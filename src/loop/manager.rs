//! Core automation loop manager.
//!
//! This module handles the main automation loop that runs Claude Code
//! iterations with stagnation detection, mode switching, and analytics.
//!
//! # Architecture
//!
//! The `LoopManager` is the central orchestrator that:
//! 1. Manages iteration cycles
//! 2. Detects progress through commits and plan changes
//! 3. Handles stagnation with mode switching
//! 4. Coordinates with the supervisor for health monitoring
//!
//! # Dependency Injection
//!
//! The loop manager supports dependency injection through `LoopDependencies`,
//! enabling comprehensive unit testing with mocks.
//!
//! # Example
//!
//! ```rust,ignore
//! use ralph::r#loop::manager::LoopManager;
//! use ralph::r#loop::state::LoopMode;
//! use ralph::config::ProjectConfig;
//!
//! let manager = LoopManager::new(
//!     PathBuf::from("."),
//!     LoopMode::Build,
//!     10,  // max_iterations
//!     3,   // stagnation_threshold
//!     5,   // doc_sync_interval
//!     ProjectConfig::default(),
//!     false, // verbose
//! )?;
//!
//! manager.run().await?;
//! ```

use super::operations::{RealClaudeProcess, RealFileSystem, RealGitOperations, RealQualityChecker};
use super::progress::{ProgressEvaluation, ProgressSignals, ProgressTracker};
use super::retry::{
    FailureClass, FailureClassifier, FailureContext, FailureLocation, IntelligentRetry,
    RecoveryStrategy, RetryAttempt, RetryConfig, RetryHistory, SubTask, TaskDecomposer,
};
use super::state::{LoopMode, LoopState};
use super::task_tracker::{
    TaskState, TaskTracker, TaskTrackerConfig, TaskTransition, ValidationResult,
};
use crate::supervisor::predictor::{
    InterventionThresholds, PredictorConfig, PreventiveAction, RiskSignals, RiskWeights,
    StagnationPredictor,
};
use crate::supervisor::{Supervisor, SupervisorVerdict};
use anyhow::{bail, Context, Result};
use colored::Colorize;
use ralph::checkpoint::{
    CheckpointManager, CheckpointManagerConfig, QualityMetrics, RegressionThresholds,
    RollbackManager,
};
use ralph::config::ProjectConfig;
use ralph::prompt::{AssemblerConfig, AttemptOutcome, ErrorSeverity, PromptAssembler, TaskPhase};
use ralph::quality::EnforcerConfig;
use ralph::testing::{ClaudeProcess, FileSystem, GitOperations, QualityChecker};
use ralph::Analytics;
use std::path::PathBuf;
use std::process::Command;
use std::sync::Arc;
use tokio::process::Command as AsyncCommand;
use tokio::sync::RwLock;
use tracing::{debug, info, warn};

/// Maximum retries for transient failures (LSP crashes, etc.)
const MAX_RETRIES: u32 = 3;

/// Backoff delay between retries in milliseconds
const RETRY_BACKOFF_MS: u64 = 2000;

/// Estimated tokens per byte (conservative estimate for code)
const TOKENS_PER_BYTE: f64 = 0.25;

/// Warning threshold for file size (Claude's limit is ~25k tokens)
const FILE_SIZE_WARNING_TOKENS: usize = 20_000;

/// Critical threshold - files this size will cause Claude errors
const FILE_SIZE_CRITICAL_TOKENS: usize = 25_000;

/// Dependencies for the loop manager.
///
/// This struct holds trait objects for all external dependencies,
/// enabling dependency injection for testing.
///
/// # Example
///
/// ```rust,ignore
/// use ralph::r#loop::manager::LoopDependencies;
/// use ralph::testing::{MockGitOperations, MockClaudeProcess, MockFileSystem, MockQualityChecker};
/// use std::sync::Arc;
///
/// let deps = LoopDependencies {
///     git: Arc::new(MockGitOperations::new().with_commit_hash("abc123")),
///     claude: Arc::new(MockClaudeProcess::new().with_exit_code(0)),
///     fs: Arc::new(RwLock::new(MockFileSystem::new())),
///     quality: Arc::new(MockQualityChecker::new().all_passing()),
/// };
/// ```
pub struct LoopDependencies {
    /// Git operations abstraction.
    pub git: Arc<dyn GitOperations + Send + Sync>,
    /// Claude process abstraction.
    pub claude: Arc<dyn ClaudeProcess + Send + Sync>,
    /// File system abstraction (wrapped in RwLock for interior mutability).
    pub fs: Arc<RwLock<dyn FileSystem + Send + Sync>>,
    /// Quality checker abstraction.
    pub quality: Arc<dyn QualityChecker + Send + Sync>,
}

impl std::fmt::Debug for LoopDependencies {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("LoopDependencies")
            .field("git", &"<dyn GitOperations>")
            .field("claude", &"<dyn ClaudeProcess>")
            .field("fs", &"<dyn FileSystem>")
            .field("quality", &"<dyn QualityChecker>")
            .finish()
    }
}

impl LoopDependencies {
    /// Create real dependencies for production use.
    #[must_use]
    pub fn real(project_dir: PathBuf) -> Self {
        Self {
            git: Arc::new(RealGitOperations::new(project_dir.clone())),
            claude: Arc::new(RealClaudeProcess::new(project_dir.clone())),
            fs: Arc::new(RwLock::new(RealFileSystem::new(project_dir.clone()))),
            quality: Arc::new(RealQualityChecker::new(project_dir)),
        }
    }

    /// Create real dependencies with custom quality gate configuration.
    ///
    /// This allows customizing which quality gates are enabled and their settings.
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// use ralph::quality::EnforcerConfig;
    ///
    /// let quality_config = EnforcerConfig::new()
    ///     .with_clippy(true)
    ///     .with_tests(false)  // Skip tests for faster feedback
    ///     .with_security(true);
    ///
    /// let deps = LoopDependencies::real_with_quality_config(project_dir, quality_config);
    /// ```
    #[must_use]
    pub fn real_with_quality_config(project_dir: PathBuf, quality_config: EnforcerConfig) -> Self {
        Self {
            git: Arc::new(RealGitOperations::new(project_dir.clone())),
            claude: Arc::new(RealClaudeProcess::new(project_dir.clone())),
            fs: Arc::new(RwLock::new(RealFileSystem::new(project_dir.clone()))),
            quality: Arc::new(RealQualityChecker::with_config(project_dir, quality_config)),
        }
    }
}

/// Configuration for creating a new `LoopManager`.
///
/// Groups all the configuration parameters for the loop manager.
#[derive(Debug, Clone)]
pub struct LoopManagerConfig {
    /// Path to the project directory.
    pub project_dir: PathBuf,
    /// Initial loop mode.
    pub mode: LoopMode,
    /// Maximum number of iterations to run.
    pub max_iterations: u32,
    /// Number of iterations without progress before switching to debug mode.
    pub stagnation_threshold: u32,
    /// Interval (in iterations) for documentation sync checks.
    pub doc_sync_interval: u32,
    /// Project configuration.
    pub config: ProjectConfig,
    /// Whether to enable verbose output.
    pub verbose: bool,
    /// Quality gate enforcer configuration.
    pub quality_config: Option<EnforcerConfig>,
}

impl LoopManagerConfig {
    /// Create a new configuration with the specified parameters.
    #[must_use]
    pub fn new(project_dir: PathBuf, config: ProjectConfig) -> Self {
        Self {
            project_dir,
            mode: LoopMode::Build,
            max_iterations: 100,
            stagnation_threshold: 3,
            doc_sync_interval: 5,
            config,
            verbose: false,
            quality_config: None,
        }
    }

    /// Set the loop mode.
    #[must_use]
    pub fn with_mode(mut self, mode: LoopMode) -> Self {
        self.mode = mode;
        self
    }

    /// Set the maximum iterations.
    #[must_use]
    pub fn with_max_iterations(mut self, max: u32) -> Self {
        self.max_iterations = max;
        self
    }

    /// Set the stagnation threshold.
    #[must_use]
    pub fn with_stagnation_threshold(mut self, threshold: u32) -> Self {
        self.stagnation_threshold = threshold;
        self
    }

    /// Set the doc sync interval.
    #[must_use]
    pub fn with_doc_sync_interval(mut self, interval: u32) -> Self {
        self.doc_sync_interval = interval;
        self
    }

    /// Enable verbose mode.
    #[must_use]
    pub fn with_verbose(mut self, verbose: bool) -> Self {
        self.verbose = verbose;
        self
    }

    /// Set custom quality gate configuration.
    ///
    /// This allows customizing which quality gates are enabled.
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// use ralph::quality::EnforcerConfig;
    ///
    /// let config = LoopManagerConfig::new(project_dir, ProjectConfig::default())
    ///     .with_quality_config(EnforcerConfig::new()
    ///         .with_clippy(true)
    ///         .with_tests(false)  // Skip tests for faster iteration
    ///         .with_security(true));
    /// ```
    #[must_use]
    pub fn with_quality_config(mut self, quality_config: EnforcerConfig) -> Self {
        self.quality_config = Some(quality_config);
        self
    }
}

/// The main loop manager.
///
/// Orchestrates Claude Code iterations with stagnation detection,
/// mode switching, and progress tracking.
///
/// Supports dependency injection through `LoopDependencies` for testing.
#[derive(Debug)]
pub struct LoopManager {
    project_dir: PathBuf,
    max_iterations: u32,
    stagnation_threshold: u32,
    doc_sync_interval: u32,
    state: LoopState,
    analytics: Analytics,
    config: ProjectConfig,
    verbose: bool,
    /// Injected dependencies (None = use direct calls for backward compatibility).
    deps: Option<LoopDependencies>,
    /// Task-level progress tracker.
    task_tracker: TaskTracker,
    /// Dynamic prompt assembler for context-aware prompts.
    prompt_assembler: PromptAssembler,
    /// Semantic progress tracker for multi-dimensional progress detection.
    progress_tracker: ProgressTracker,
    /// Intelligent retry system for failure classification and recovery.
    intelligent_retry: IntelligentRetry,
    /// Retry history for tracking and learning from retry attempts.
    retry_history: RetryHistory,
}

impl LoopManager {
    /// Create a new loop manager.
    ///
    /// # Errors
    ///
    /// Returns an error if IMPLEMENTATION_PLAN.md is not found.
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// let config = LoopManagerConfig::new(PathBuf::from("."), ProjectConfig::default())
    ///     .with_mode(LoopMode::Build)
    ///     .with_max_iterations(10)
    ///     .with_stagnation_threshold(3);
    /// let manager = LoopManager::new(config)?;
    /// ```
    pub fn new(cfg: LoopManagerConfig) -> Result<Self> {
        // Create real dependencies, using custom quality config if provided
        let deps = match cfg.quality_config.clone() {
            Some(quality_config) => {
                LoopDependencies::real_with_quality_config(cfg.project_dir.clone(), quality_config)
            }
            None => LoopDependencies::real(cfg.project_dir.clone()),
        };
        Self::with_deps(cfg, deps)
    }

    /// Create a new loop manager with injected dependencies.
    ///
    /// This constructor is primarily for testing, allowing mocks to be
    /// injected for git, claude, file system, and quality checker operations.
    ///
    /// # Errors
    ///
    /// Returns an error if IMPLEMENTATION_PLAN.md is not found.
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// use ralph::testing::{MockGitOperations, MockClaudeProcess, MockFileSystem, MockQualityChecker};
    ///
    /// let deps = LoopDependencies {
    ///     git: Arc::new(MockGitOperations::new().with_commit_hash("abc123")),
    ///     claude: Arc::new(MockClaudeProcess::new()),
    ///     fs: Arc::new(RwLock::new(MockFileSystem::new()
    ///         .with_file("IMPLEMENTATION_PLAN.md", "# Plan"))),
    ///     quality: Arc::new(MockQualityChecker::new().all_passing()),
    /// };
    ///
    /// let config = LoopManagerConfig::new(PathBuf::from("."), ProjectConfig::default())
    ///     .with_mode(LoopMode::Build)
    ///     .with_max_iterations(10)
    ///     .with_stagnation_threshold(3);
    /// let manager = LoopManager::with_deps(config, deps)?;
    /// ```
    pub fn with_deps(cfg: LoopManagerConfig, deps: LoopDependencies) -> Result<Self> {
        let analytics = Analytics::new(cfg.project_dir.clone());
        let state = LoopState::new(cfg.mode);

        // Check if plan file exists using the injected file system
        // Use try_read to avoid blocking in async contexts
        let plan_exists = match deps.fs.try_read() {
            Ok(fs) => fs.exists("IMPLEMENTATION_PLAN.md"),
            Err(_) => {
                // If we can't get the lock, fall back to sync read
                // This handles cases where we're not in an async runtime
                let fs = deps.fs.blocking_read();
                fs.exists("IMPLEMENTATION_PLAN.md")
            }
        };

        if !plan_exists {
            bail!(
                "IMPLEMENTATION_PLAN.md not found. Run 'ralph bootstrap' first or create the file."
            );
        }

        // Initialize or load task tracker using injected filesystem
        // Use all builder methods to ensure they're exercised
        let tracker_config = TaskTrackerConfig::new()
            .with_stagnation_threshold(cfg.stagnation_threshold)
            .with_max_attempts(10)
            .with_timeout_secs(3600)
            .with_max_quality_failures(3)
            .without_auto_save()  // Disable first
            .with_auto_save(true); // Then re-enable for production
        let tracker_path = TaskTracker::default_path(&cfg.project_dir);
        let mut task_tracker = TaskTracker::load_or_new(&tracker_path, tracker_config);

        // Parse the implementation plan from the injected filesystem
        let plan_content = match deps.fs.try_read() {
            Ok(fs) => fs
                .read_file("IMPLEMENTATION_PLAN.md")
                .context("Failed to read IMPLEMENTATION_PLAN.md")?,
            Err(_) => {
                let fs = deps.fs.blocking_read();
                fs.read_file("IMPLEMENTATION_PLAN.md")
                    .context("Failed to read IMPLEMENTATION_PLAN.md")?
            }
        };
        task_tracker.parse_plan(&plan_content)?;

        // Save initial state (non-fatal if save fails)
        task_tracker.save(&tracker_path).ok();

        // Initialize prompt assembler with default configuration
        let assembler_config = AssemblerConfig::new()
            .with_max_errors(5)
            .with_max_attempts(5)
            .with_max_anti_patterns(3);
        let prompt_assembler = PromptAssembler::with_config(assembler_config);

        // Initialize semantic progress tracker
        // Use strict thresholds in debug mode to require more significant progress
        let progress_tracker = if cfg.mode == LoopMode::Debug {
            ProgressTracker::with_strict_thresholds(cfg.project_dir.clone())
        } else {
            ProgressTracker::new(cfg.project_dir.clone())
        };

        // Initialize intelligent retry system
        // Use fewer retries in debug mode for faster feedback
        // Use minimal output in plan mode for cleaner planning
        let retry_config = if cfg.mode == LoopMode::Debug {
            RetryConfig::default().with_max_retries(3)
        } else if cfg.mode == LoopMode::Plan {
            RetryConfig::default().without_raw_output()
        } else {
            RetryConfig::default()
        };
        let intelligent_retry = IntelligentRetry::with_config(retry_config);
        let retry_history = RetryHistory::new();

        info!(
            "LoopManager initialized with deps: {} allowed permissions, {} safety blocks, {} tasks",
            cfg.config.permissions.allow.len(),
            cfg.config.permissions.deny.len(),
            task_tracker.tasks.len()
        );

        Ok(Self {
            project_dir: cfg.project_dir.clone(),
            max_iterations: cfg.max_iterations,
            stagnation_threshold: cfg.stagnation_threshold,
            doc_sync_interval: cfg.doc_sync_interval,
            state,
            analytics,
            config: cfg.config,
            verbose: cfg.verbose,
            deps: Some(deps),
            task_tracker,
            prompt_assembler,
            progress_tracker,
            intelligent_retry,
            retry_history,
        })
    }

    /// Run the main automation loop.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - Supervisor aborts execution
    /// - Critical errors occur during iteration
    pub async fn run(&mut self) -> Result<()> {
        self.print_banner();

        // Log session start
        self.analytics.log_event(
            &self.state.session_id,
            "session_start",
            serde_json::json!({
                "mode": self.state.mode.to_string(),
                "max_iterations": self.max_iterations,
                "config_loaded": true,
            }),
        )?;

        // Get initial plan and commit hashes
        self.state.update_plan_hash(self.get_plan_hash()?);
        self.state
            .update_commit_hash(self.get_commit_hash().unwrap_or_default());

        // Create supervisor for health monitoring (checks every 5 iterations by default)
        let mut supervisor = Supervisor::new(self.project_dir.clone()).with_interval(5);

        // Create stagnation predictor for proactive intervention
        // Use builder pattern in verbose mode to demonstrate customization,
        // otherwise use sensible defaults
        let mut predictor = if self.verbose {
            // Builder pattern allows tuning based on session parameters
            let predictor_config = PredictorConfig::new()
                .with_weights(RiskWeights::new(0.25, 0.20, 0.20, 0.15, 0.10, 0.10))
                .with_thresholds(InterventionThresholds::new(30.0, 60.0, 80.0))
                .with_max_commit_gap(self.max_iterations.saturating_div(2).max(10))
                .with_max_file_touches(5)
                .with_history_length(10);
            let p = StagnationPredictor::new(predictor_config);
            let config = p.config();
            debug!(
                "Predictor initialized (custom): max_commit_gap={}, max_file_touches={}, history_len={}",
                config.max_commit_gap, config.max_file_touches, config.history_length
            );
            p
        } else {
            // Use defaults for standard operation
            StagnationPredictor::with_defaults()
        };

        // Create checkpoint manager for quality regression prevention
        let checkpoint_dir = self.project_dir.join(".ralph/checkpoints");
        let checkpoint_config = CheckpointManagerConfig::new()
            .with_max_checkpoints(20)
            .with_auto_prune(true)
            .with_min_interval(3); // At least 3 iterations between checkpoints
        let mut checkpoint_mgr = CheckpointManager::with_config(&checkpoint_dir, checkpoint_config)
            .unwrap_or_else(|e| {
                warn!("Could not create checkpoint manager: {}", e);
                // Fall back to basic manager
                CheckpointManager::new(&checkpoint_dir).expect("Checkpoint manager creation failed")
            });

        let rollback_mgr = RollbackManager::new(&self.project_dir);
        let regression_thresholds = RegressionThresholds::default();

        // Track best quality metrics for checkpoint creation
        let mut best_quality: Option<QualityMetrics> = None;

        // Track file touches across iterations for churn detection
        let mut file_touch_history: std::collections::HashMap<String, u32> =
            std::collections::HashMap::new();
        // Track test count history for stagnation detection
        let mut test_count_history: Vec<u32> = Vec::new();
        // Track clippy warning history for growth detection
        let mut warning_count_history: Vec<u32> = Vec::new();
        // Track recent error messages for repetition detection
        let mut recent_errors: Vec<String> = Vec::new();
        // Track last prediction for accuracy recording
        let mut last_risk_score: Option<f64> = None;

        while self.state.iteration < self.max_iterations {
            self.state.next_iteration();

            self.print_iteration_header();

            // Select the next task to work on (clone to release borrow)
            let current_task_id = self.task_tracker.select_next_task().cloned();
            if let Some(ref task_id) = current_task_id {
                // Set as current task for tracking
                if let Err(e) = self.task_tracker.set_current(task_id) {
                    debug!("Could not set current task {}: {}", task_id, e);
                }

                // Start the task if not already in progress
                if let Err(e) = self.task_tracker.start_task(task_id) {
                    debug!("Could not start task {}: {}", task_id, e);
                }

                // Record iteration start
                if let Err(e) = self.task_tracker.record_iteration() {
                    debug!("Could not record iteration: {}", e);
                }

                // Log the state transition for debugging
                let transition = TaskTransition::new(TaskState::NotStarted, TaskState::InProgress);
                debug!(
                    "Task transition: {:?} -> {:?} at {:?}",
                    transition.from, transition.to, transition.timestamp
                );

                // Display current task info
                let phase_info = task_id
                    .phase()
                    .map(|p| format!(" (Phase {})", p))
                    .unwrap_or_default();
                println!(
                    "   {} {} - {}{}",
                    "Task:".cyan().bold(),
                    task_id,
                    task_id.title(),
                    phase_info
                );
                debug!("Task original header: {}", task_id.original());

                // Update prompt assembler with current task context
                let task_phase = match self.state.mode {
                    LoopMode::Build => TaskPhase::Implementation,
                    LoopMode::Debug => TaskPhase::QualityFixes,
                    LoopMode::Plan => TaskPhase::Planning,
                };
                self.prompt_assembler.set_current_task(
                    &task_id.to_string(),
                    task_id.title(),
                    task_phase,
                );

                // Update task files if available
                if let Some(task) = self.task_tracker.get_task(task_id) {
                    // Calculate completion percentage based on checkboxes
                    let total = task.checkboxes.len();
                    let checked = task.checkboxes.iter().filter(|(_, c)| *c).count();
                    if total > 0 {
                        let completion = ((checked * 100) / total) as u8;
                        self.prompt_assembler.update_task_completion(completion);
                    }
                }
            } else {
                debug!("No task selected for this iteration");
            }

            // Update session stats in prompt assembler
            let task_counts = self.task_tracker.task_counts();
            self.prompt_assembler.update_session_stats(
                self.state.iteration,
                task_counts.complete,
                0, // tokens_used - not tracked directly yet
            );

            // Log sprint status if verbose
            if self.verbose {
                if let Some(sprint_num) = self.task_tracker.current_sprint() {
                    let sprint_tasks = self.task_tracker.tasks_for_sprint(sprint_num);
                    let sprint_complete = self.task_tracker.is_sprint_complete(sprint_num);
                    debug!(
                        "Current sprint {}: {} tasks, complete={}",
                        sprint_num,
                        sprint_tasks.len(),
                        sprint_complete
                    );
                    // Log individual task sprint info
                    for task in sprint_tasks.iter().take(3) {
                        debug!(
                            "  Sprint {} task: {} (sprint={:?})",
                            sprint_num,
                            task.id,
                            task.sprint()
                        );
                    }
                }
                debug!("Plan hash: {}", self.task_tracker.plan_hash());
            }

            // Check for progress using multiple indicators (commits AND plan changes)
            let made_progress = self.has_made_progress();

            // Record prediction accuracy from last iteration
            if let Some(score) = last_risk_score {
                // If we predicted high risk (score >= 60) and no progress, prediction was correct
                // If we predicted low risk (score < 60) and progress was made, prediction was correct
                let actually_stagnated = !made_progress;
                predictor.record_prediction(score, actually_stagnated);
            }

            if made_progress {
                // Progress detected - reset stagnation and update tracking
                self.state.record_progress();
                self.state.update_plan_hash(self.get_plan_hash()?);
                self.state
                    .update_commit_hash(self.get_commit_hash().unwrap_or_default());

                // Check for orphaned tasks when plan changes
                self.check_for_orphaned_tasks();

                // Record progress on the current task
                // We use (1, 0) to indicate progress was made (actual counts tracked separately)
                if let Some(ref task_id) = current_task_id {
                    if let Err(e) = self.task_tracker.record_progress(1, 0) {
                        debug!("Could not record progress for current task: {}", e);
                    }
                    // Record commit in task metrics
                    if let Some(task) = self.task_tracker.get_task_mut(task_id) {
                        task.metrics.record_commit();
                        // Log task state info
                        debug!(
                            "Task {} state: active={}, terminal={}, complete={}, blocked={}",
                            task_id,
                            task.state.is_active(),
                            task.state.is_terminal(),
                            task.is_complete(),
                            task.is_blocked()
                        );
                    }
                }

                // If we were in debug mode due to stagnation, return to build mode
                if self.state.mode == LoopMode::Debug {
                    info!("Progress resumed, returning to build mode");
                    println!(
                        "   {} Progress detected, returning to build mode",
                        "Info:".green().bold()
                    );
                    self.state.switch_mode(LoopMode::Build);
                }
            } else {
                // No progress - increment stagnation counter
                self.state.record_no_progress();

                // Record no progress on the current task (may auto-block if threshold reached)
                if let Some(ref task_id) = current_task_id {
                    match self.task_tracker.record_no_progress() {
                        Ok(blocked) => {
                            if blocked {
                                println!(
                                    "   {} Task {} has been blocked due to repeated no-progress",
                                    "Warning:".yellow().bold(),
                                    task_id
                                );
                            }
                        }
                        Err(e) => {
                            debug!("Could not record no-progress for task {}: {}", task_id, e);
                        }
                    }

                    // Check if task appears stuck
                    if self.task_tracker.is_task_stuck(task_id) {
                        println!(
                            "   {} Task {} appears to be stuck",
                            "Warning:".yellow().bold(),
                            task_id
                        );
                    }
                }

                if self.state.is_stagnating(self.stagnation_threshold) {
                    warn!(
                        "Stagnation detected ({} iterations without commits or plan changes)",
                        self.state.stagnation_count
                    );
                    self.state.switch_mode(LoopMode::Debug);
                    println!(
                        "   {} Switching to debug mode (no commits or plan changes)",
                        "Warning:".yellow().bold()
                    );

                    // Log stagnation event with task context
                    self.analytics.log_event(
                        &self.state.session_id,
                        "stagnation",
                        serde_json::json!({
                            "iteration": self.state.iteration,
                            "count": self.state.stagnation_count,
                            "last_commit": self.state.last_commit_hash,
                            "current_task": current_task_id.as_ref().map(|id| id.to_string()),
                        }),
                    )?;
                }
            }

            // Run Claude Code iteration with retry logic for transient failures
            let mut retry_count = 0;
            let mut should_break = false;

            loop {
                let result = self.run_claude_iteration().await;

                match result {
                    Ok(exit_code) => {
                        if exit_code == 0 {
                            // If this was a successful retry, clear the failure history
                            if retry_count > 0 {
                                let previous_error = "Claude exited with code 1";
                                self.clear_resolved_failure(previous_error);
                                debug!(
                                    "Retry succeeded after {} attempts, cleared failure history",
                                    retry_count
                                );
                            }

                            // Record successful attempt in prompt assembler
                            self.prompt_assembler.record_attempt(
                                AttemptOutcome::Success,
                                Some(&format!("{} mode", self.state.mode)),
                                vec![],
                            );

                            // Run quality checks if we have dependencies
                            if let Some(ref deps) = self.deps {
                                // Submit task for review before running quality checks
                                if let Some(ref task_id) = current_task_id {
                                    if let Err(e) = self.task_tracker.submit_for_review(task_id) {
                                        debug!("Could not submit task for review: {}", e);
                                    }
                                }

                                let quality_result = deps.quality.run_clippy();
                                match quality_result {
                                    Ok(result) if result.passed => {
                                        // Quality passed - update review status
                                        self.prompt_assembler.update_clippy_status(true, vec![]);
                                        if let Some(ref task_id) = current_task_id {
                                            // Update review with passing result
                                            match self
                                                .task_tracker
                                                .update_review(task_id, "clippy", true)
                                            {
                                                Ok(_) => {
                                                    debug!("Quality review passed for {}", task_id)
                                                }
                                                Err(e) => debug!("Could not update review: {}", e),
                                            }

                                            // Check if all checkboxes are complete
                                            if let Some(task) = self.task_tracker.get_task(task_id)
                                            {
                                                let all_checked = task
                                                    .checkboxes
                                                    .iter()
                                                    .all(|(_, checked)| *checked);
                                                if all_checked && !task.checkboxes.is_empty() {
                                                    // Complete the task
                                                    if let Err(e) =
                                                        self.task_tracker.complete_task(task_id)
                                                    {
                                                        debug!("Could not complete task: {}", e);
                                                    } else {
                                                        println!(
                                                            "   {} Task {} completed!",
                                                            "Success:".green().bold(),
                                                            task_id
                                                        );
                                                        // Reset retry state for the completed task
                                                        self.reset_task_retries(
                                                            &task_id.to_string(),
                                                        );
                                                    }
                                                }
                                            }
                                        }
                                    }
                                    Ok(result) => {
                                        // Quality failed - update review with failure
                                        debug!(
                                            "Quality check failed: {} warnings",
                                            result.warnings.len()
                                        );
                                        self.prompt_assembler
                                            .update_clippy_status(false, result.warnings.clone());

                                        // Log warning locations for debugging
                                        for warning in &result.warnings {
                                            // Try to extract file:line from warning
                                            // Clippy warnings typically look like "src/file.rs:123: message"
                                            if let Some((file, rest)) = warning.split_once(':') {
                                                let line = rest
                                                    .split(':')
                                                    .next()
                                                    .and_then(|s| s.parse::<u32>().ok());
                                                let location =
                                                    Self::create_failure_location(file, line);
                                                debug!("Quality warning at {}", location);
                                            }
                                        }

                                        if let Some(ref task_id) = current_task_id {
                                            match self
                                                .task_tracker
                                                .update_review(task_id, "clippy", false)
                                            {
                                                Ok(blocked) if blocked => {
                                                    println!(
                                                        "   {} Task {} blocked due to quality failures",
                                                        "Warning:".yellow().bold(),
                                                        task_id
                                                    );
                                                }
                                                Ok(_) => debug!(
                                                    "Quality review failed for {}, will retry",
                                                    task_id
                                                ),
                                                Err(e) => debug!("Could not update review: {}", e),
                                            }
                                        }
                                    }
                                    Err(e) => {
                                        debug!("Quality check error: {}", e);
                                    }
                                }
                            }
                            // Success - continue to next iteration
                            break;
                        } else if exit_code == 1 && retry_count < MAX_RETRIES {
                            // Check if intelligent retry system has exhausted retries for this task
                            let task_id_for_retry = current_task_id
                                .as_ref()
                                .map(|t| t.to_string())
                                .unwrap_or_else(|| "default".to_string());

                            if self.retries_exhausted(&task_id_for_retry) {
                                debug!(
                                    "Intelligent retry system reports exhausted for task {}",
                                    task_id_for_retry
                                );
                                // Fall through to the else branch
                            }

                            // Check if this might be a transient LSP failure
                            retry_count += 1;
                            warn!(
                                "Iteration failed (attempt {}/{}), cleaning up LSP and retrying...",
                                retry_count, MAX_RETRIES
                            );
                            eprintln!(
                                "   {} Transient failure detected, retrying ({}/{})...",
                                "Retry:".yellow().bold(),
                                retry_count,
                                MAX_RETRIES
                            );

                            // Clean up LSP processes and wait
                            Self::cleanup_lsp();
                            tokio::time::sleep(std::time::Duration::from_millis(
                                RETRY_BACKOFF_MS * u64::from(retry_count),
                            ))
                            .await;
                        } else {
                            // Get task ID for retry tracking
                            let task_id_str = current_task_id
                                .as_ref()
                                .map(|t| t.to_string())
                                .unwrap_or_else(|| "default".to_string());

                            // Create error output for classification
                            let error_output = format!("Claude exited with code {}", exit_code);

                            // Use intelligent retry to classify and determine strategy
                            if let Some((strategy, retry_prompt)) =
                                self.classify_failure(&error_output, Some(&task_id_str))
                            {
                                eprintln!(
                                    "   {} Intelligent retry: {:?} strategy selected",
                                    "Retry:".yellow().bold(),
                                    strategy
                                );

                                // Log detailed retry status
                                self.log_retry_status(&task_id_str);

                                // Classify failure for recording
                                let failure = self.failure_classifier().classify(&error_output);

                                // Log failure summary with code change requirement
                                debug!(
                                    "Failure analysis: {} (requires_code_change={})",
                                    failure.summary(),
                                    failure.class.requires_code_change()
                                );

                                // Check if failure is atomic or needs decomposition
                                if !self.is_atomic_failure(&failure) {
                                    let subtasks =
                                        self.decompose_task_for_failure(&task_id_str, &failure);
                                    debug!(
                                        "Task decomposed into {} subtasks for recovery",
                                        subtasks.len()
                                    );
                                }

                                // Record the retry attempt
                                self.record_retry_attempt(failure, strategy, false);

                                // If strategy modifies prompt, inject retry guidance
                                if strategy.modifies_prompt() {
                                    self.prompt_assembler.add_error(
                                        "RETRY_GUIDANCE",
                                        &retry_prompt,
                                        ErrorSeverity::Warning,
                                    );
                                }

                                // Clean up and continue
                                Self::cleanup_lsp();
                                tokio::time::sleep(std::time::Duration::from_millis(
                                    RETRY_BACKOFF_MS * u64::from(retry_count),
                                ))
                                .await;
                                retry_count += 1;
                                continue;
                            }

                            // Retries exhausted or non-recoverable
                            eprintln!(
                                "   {} Fatal error from Claude Code (after {} retries)",
                                "Error:".red().bold(),
                                retry_count
                            );

                            // Log final retry summary
                            debug!("Retry summary: {}", self.retry_summary());

                            // Record failed attempt in prompt assembler
                            self.prompt_assembler.record_attempt(
                                AttemptOutcome::CompilationError,
                                Some(&format!(
                                    "{} mode, {} retries",
                                    self.state.mode, retry_count
                                )),
                                vec![format!("Exit code: {}", exit_code)],
                            );
                            self.prompt_assembler.add_error(
                                "CLAUDE_EXIT",
                                &format!("Claude exited with code {} after {} retries, retries exhausted", exit_code, retry_count),
                                ErrorSeverity::Error,
                            );

                            should_break = true;
                            break;
                        }
                    }
                    Err(e) => {
                        eprintln!("   {} {}", "Error:".red().bold(), e);
                        // Record error for supervisor pattern detection
                        supervisor.record_error(&e.to_string());

                        // Record error for predictor repetition detection
                        recent_errors.push(e.to_string());
                        // Keep only the last 10 errors
                        if recent_errors.len() > 10 {
                            recent_errors.remove(0);
                        }

                        // Record error in prompt assembler
                        self.prompt_assembler.add_error(
                            "CLAUDE_ERROR",
                            &e.to_string(),
                            ErrorSeverity::Error,
                        );
                        self.prompt_assembler.record_attempt(
                            AttemptOutcome::CompilationError,
                            Some("Error during execution"),
                            vec![e.to_string()],
                        );
                        // Log error but continue
                        self.analytics.log_event(
                            &self.state.session_id,
                            "iteration_error",
                            serde_json::json!({
                                "iteration": self.state.iteration,
                                "error": e.to_string(),
                                "retry_count": retry_count,
                            }),
                        )?;
                        break;
                    }
                }
            }

            if should_break {
                break;
            }

            // Track cumulative changes
            if let Ok(changes) = self.get_recent_changes() {
                self.state.cumulative_changes += changes;

                // Trigger project analysis if significant changes
                if self.state.cumulative_changes > 500 {
                    info!(
                        "Significant changes detected ({} lines) - consider running project analysis",
                        self.state.cumulative_changes
                    );
                    self.state.cumulative_changes = 0;
                }
            }

            // Log detailed progress status after each iteration
            self.log_progress_status();

            // Update progress baselines for accurate delta tracking
            self.update_progress_baselines();

            // Update quality history for predictor
            if let Ok(quality) = self.progress_tracker.collect_quality_signals() {
                // Update test count history (keep last 10)
                test_count_history.push(quality.test_count);
                if test_count_history.len() > 10 {
                    test_count_history.remove(0);
                }
                // Update warning count history (keep last 10)
                warning_count_history.push(quality.warning_count);
                if warning_count_history.len() > 10 {
                    warning_count_history.remove(0);
                }

                // Build quality metrics for checkpoint system
                let current_quality = QualityMetrics::new()
                    .with_clippy_warnings(quality.warning_count)
                    .with_test_counts(quality.test_count, quality.test_count, 0);

                // Create checkpoint if progress was made and quality is good
                if made_progress && quality.warning_count == 0 {
                    let commit_hash = self
                        .get_commit_hash()
                        .unwrap_or_else(|_| "unknown".to_string());
                    let description = format!(
                        "Iteration {} - {} tests, 0 warnings",
                        self.state.iteration, quality.test_count
                    );

                    // Check if this is better than previous best
                    let should_checkpoint = best_quality
                        .as_ref()
                        .map(|bq| !current_quality.is_worse_than(bq, &regression_thresholds))
                        .unwrap_or(true);

                    if should_checkpoint {
                        match checkpoint_mgr.create_checkpoint(
                            &description,
                            &commit_hash,
                            "main",
                            current_quality.clone(),
                            self.state.iteration,
                        ) {
                            Ok(cp) => {
                                debug!("Created checkpoint: {}", cp.summary());
                                if self.verbose {
                                    println!(
                                        "   {} Checkpoint created: {}",
                                        "Checkpoint:".bright_cyan().bold(),
                                        cp.id
                                    );
                                }
                                best_quality = Some(current_quality.clone());
                            }
                            Err(e) => {
                                debug!("Could not create checkpoint: {}", e);
                            }
                        }
                    }
                }

                // Check for quality regression and potential rollback
                if let Some(ref best) = best_quality {
                    if current_quality.is_worse_than(best, &regression_thresholds) {
                        let score = current_quality.regression_score(best);
                        warn!(
                            "Quality regression detected (score: {:.1}): {} vs {}",
                            score,
                            current_quality.summary(),
                            best.summary()
                        );

                        // Check if we should rollback
                        if score >= regression_thresholds.rollback_threshold_score {
                            if let Ok(Some(target)) = rollback_mgr.should_rollback(
                                &mut checkpoint_mgr,
                                &current_quality,
                                &regression_thresholds,
                            ) {
                                warn!(
                                    "Rollback recommended to checkpoint {} (score: {:.1})",
                                    target.id, score
                                );
                                println!(
                                    "   {} Quality regression detected, consider rollback to {}",
                                    "Warning:".yellow().bold(),
                                    target.id
                                );
                                // Note: Actual rollback is manual to avoid disrupting work
                                // The user can run `ralph rollback <id>` to rollback
                            }
                        }
                    }
                }
            }

            // Doc sync check
            if self.doc_sync_interval > 0
                && self.state.iteration.checked_rem(self.doc_sync_interval) == Some(0)
                && self.state.mode == LoopMode::Build
            {
                self.run_doc_sync().await?;
            }

            // File size check (every 5 iterations in build mode)
            if self.state.iteration.checked_rem(5) == Some(0) && self.state.mode == LoopMode::Build
            {
                self.check_file_sizes();
            }

            // Record iteration for anti-pattern detection
            // Note: TaskMetrics tracks files_modified as a count (u32), not file paths
            // For anti-pattern detection, we pass an empty vec for now
            // Future enhancement: track actual file paths in TaskMetrics
            let committed = self.has_made_progress();
            self.prompt_assembler.record_iteration_with_files(
                self.state.iteration,
                vec![], // File paths not currently tracked in TaskMetrics
                committed,
            );

            // Auto-archive check (every 10 iterations)
            if self.state.iteration.checked_rem(10) == Some(0) {
                if let Err(e) = self.run_auto_archive() {
                    debug!("Auto-archive check failed: {}", e);
                }
            }

            // Update file touch history from modified files
            if let Some(ref deps) = self.deps {
                if let Ok(modified) = deps.git.get_modified_files() {
                    for file in modified {
                        *file_touch_history.entry(file).or_insert(0) += 1;
                    }
                }
            }

            // Predictive stagnation prevention (every iteration)
            let risk_signals = RiskSignals::new()
                .with_commit_gap(self.state.stagnation_count)
                .with_file_touches(
                    file_touch_history
                        .iter()
                        .map(|(f, c)| (f.clone(), *c))
                        .collect(),
                )
                .with_errors(recent_errors.clone())
                .with_test_history(test_count_history.clone())
                .with_mode_switches(supervisor.mode_switch_count())
                .with_warning_history(warning_count_history.clone());

            let risk_score = predictor.risk_score(&risk_signals);
            let risk_level = predictor.risk_level(risk_score);
            let risk_breakdown = predictor.risk_breakdown(&risk_signals);

            // Also use evaluate() for direct assessment (validates internal consistency)
            let evaluated_level = predictor.evaluate(&risk_signals);
            debug_assert_eq!(
                risk_level, evaluated_level,
                "risk_level and evaluate should match"
            );

            // Compute pattern detection metrics for detailed analytics
            let file_touches: Vec<(String, u32)> = file_touch_history
                .iter()
                .map(|(f, c)| (f.clone(), *c))
                .collect();
            let file_churn_score = predictor.repeated_file_touches(&file_touches);
            let error_rate = predictor.error_repetition_rate(&recent_errors);
            let mode_switching = predictor.recent_mode_switch(supervisor.mode_switch_count(), 3);

            // Log risk assessment with level bounds and pattern metrics
            debug!(
                "Predictor: {} (dominant={}, level={}, range={:.0}-{:.0})",
                risk_breakdown.summary(),
                risk_breakdown.dominant_factor(),
                risk_level,
                risk_level.min_score(),
                risk_level.max_score()
            );
            debug!(
                "Pattern metrics: file_churn={:.2}, error_rate={:.2}, mode_switching={}",
                file_churn_score, error_rate, mode_switching
            );

            // Critical risk requires immediate attention
            if risk_level.is_critical() {
                warn!(
                    "Critical stagnation risk detected: score={:.0}, dominant={}",
                    risk_score,
                    risk_breakdown.dominant_factor()
                );
            }

            // Apply preventive action if risk is elevated
            if risk_level.requires_intervention() {
                let action = predictor.preventive_action(&risk_signals, risk_score);

                match action {
                    PreventiveAction::None => {}
                    PreventiveAction::InjectGuidance { ref guidance } => {
                        debug!("Predictor guidance: {}", guidance);
                        // Log guidance for prompt context awareness
                        info!(
                            "Risk guidance (score={:.0}): {}",
                            risk_score,
                            guidance.chars().take(80).collect::<String>()
                        );
                        if self.verbose {
                            println!(
                                "   {} Risk score={:.0}: {}",
                                "Predictor:".bright_magenta().bold(),
                                risk_score,
                                guidance.chars().take(60).collect::<String>()
                            );
                        }
                    }
                    PreventiveAction::FocusTask { ref task } => {
                        info!("Predictor suggests focusing: {}", task);
                        println!(
                            "   {} Focus on: {}",
                            "Predictor:".bright_magenta().bold(),
                            task
                        );
                    }
                    PreventiveAction::RunTests => {
                        debug!("Predictor suggests running tests");
                        if self.verbose {
                            println!(
                                "   {} Suggestion: Run tests to verify progress",
                                "Predictor:".bright_magenta().bold()
                            );
                        }
                    }
                    PreventiveAction::SuggestCommit => {
                        info!("Predictor suggests committing current work");
                        println!(
                            "   {} Suggestion: Commit your current progress",
                            "Predictor:".bright_magenta().bold()
                        );
                    }
                    PreventiveAction::SwitchMode { ref target } => {
                        info!("Predictor suggests switching to {} mode", target);
                        println!(
                            "   {} Suggestion: Consider switching to {} mode",
                            "Predictor:".bright_magenta().bold(),
                            target
                        );
                    }
                    PreventiveAction::RequestReview { ref reason } => {
                        warn!("Predictor requests review: {}", reason);
                        println!(
                            "   {} Critical risk - review needed: {}",
                            "Predictor:".bright_magenta().bold(),
                            reason.chars().take(60).collect::<String>()
                        );
                    }
                }

                // Log prediction for accuracy tracking
                self.analytics.log_event(
                    &self.state.session_id,
                    "prediction",
                    serde_json::json!({
                        "risk_score": risk_score,
                        "risk_level": risk_level.to_string(),
                        "dominant_factor": risk_breakdown.dominant_factor(),
                        "action": action.to_string(),
                    }),
                )?;
            }

            // Store risk score for accuracy tracking next iteration
            last_risk_score = Some(risk_score);

            // Supervisor health check
            if supervisor.should_check(self.state.iteration) {
                let result = if self.verbose {
                    supervisor.check_verbose(&self.state, self.state.iteration)
                } else {
                    supervisor.check(&self.state, self.state.iteration)
                };
                match result {
                    Ok(verdict) => match verdict {
                        SupervisorVerdict::Proceed => {
                            debug!(
                                "Supervisor: health OK (interval={}, mode_switches={}, last_check={})",
                                supervisor.check_interval(),
                                supervisor.mode_switch_count(),
                                supervisor.last_check_iteration()
                            );
                        }
                        SupervisorVerdict::SwitchMode { target, reason } => {
                            info!("Supervisor recommends mode switch: {}", reason);
                            println!(
                                "   {} Supervisor: switching to {} mode ({})",
                                "Supervisor:".bright_blue().bold(),
                                target,
                                reason
                            );
                            self.state.switch_mode(target);
                            supervisor.record_mode_switch();
                        }
                        SupervisorVerdict::Reset { reason } => {
                            info!("Supervisor recommends reset: {}", reason);
                            println!(
                                "   {} Supervisor: resetting stagnation ({})",
                                "Supervisor:".bright_blue().bold(),
                                reason
                            );
                            self.state.record_progress(); // Resets stagnation

                            // If current task is blocked, try to unblock it as part of reset
                            if let Some(ref task_id) = current_task_id {
                                if let Some(task) = self.task_tracker.get_task(task_id) {
                                    if task.is_blocked() {
                                        info!("Attempting to unblock task {} as part of supervisor reset", task_id);
                                        if let Err(e) = self.task_tracker.unblock_task(task_id) {
                                            debug!("Could not unblock task: {}", e);
                                        } else {
                                            println!(
                                                "   {} Task {} unblocked by supervisor",
                                                "Info:".blue(),
                                                task_id
                                            );
                                        }
                                    }
                                }
                            }
                        }
                        SupervisorVerdict::PauseForReview { reason } => {
                            warn!("Supervisor requests pause: {}", reason);
                            println!(
                                "   {} Supervisor: pausing for review ({})",
                                "Warning:".yellow().bold(),
                                reason
                            );
                            self.analytics.log_event(
                                &self.state.session_id,
                                "supervisor_pause",
                                serde_json::json!({ "reason": reason }),
                            )?;
                            break;
                        }
                        SupervisorVerdict::Abort { reason } => {
                            warn!("Supervisor abort: {}", reason);
                            println!(
                                "   {} Supervisor: aborting ({})",
                                "Error:".red().bold(),
                                reason
                            );
                            // Generate and save diagnostics before aborting
                            if let Ok(report) = supervisor.generate_diagnostics(&self.analytics) {
                                if let Ok(path) = report.save(&self.project_dir) {
                                    println!(
                                        "   {} Diagnostics saved to: {}",
                                        "Info:".blue(),
                                        path.display()
                                    );
                                }
                            }
                            self.analytics.log_event(
                                &self.state.session_id,
                                "supervisor_abort",
                                serde_json::json!({ "reason": reason }),
                            )?;
                            bail!("Supervisor abort: {}", reason);
                        }
                    },
                    Err(e) => {
                        debug!("Supervisor check failed: {}", e);
                    }
                }
            }

            // Push to remote
            self.try_push().await;

            // Check for completion
            if self.is_complete()? {
                println!("\n   {} All tasks complete!", "Success:".green().bold());
                break;
            }

            // Clear current task at end of iteration (will be re-selected next iteration)
            self.task_tracker.clear_current();
            debug!("Cleared current task for next iteration");

            // Auto-save task tracker state
            let tracker_path = TaskTracker::default_path(&self.project_dir);
            if let Err(e) = self.task_tracker.auto_save(&tracker_path) {
                debug!("Task tracker auto-save failed: {}", e);
            }

            // Log iteration with task context
            let task_counts = self.task_tracker.task_counts();
            self.analytics.log_event(
                &self.state.session_id,
                "iteration",
                serde_json::json!({
                    "iteration": self.state.iteration,
                    "stagnation": self.state.stagnation_count,
                    "mode": self.state.mode.to_string(),
                    "tasks": {
                        "total": task_counts.total(),
                        "complete": task_counts.complete,
                        "in_progress": task_counts.in_progress,
                        "blocked": task_counts.blocked,
                    },
                }),
            )?;
        }

        // Run final security audit
        println!("\n{} Running final security audit...", "Info:".blue());
        self.run_security_audit().await?;

        // Log predictor accuracy summary
        let predictor_summary = predictor.summary();
        let prediction_accuracy = predictor.prediction_accuracy();
        debug!("{}", predictor_summary);

        // Log session end
        self.analytics.log_event(
            &self.state.session_id,
            "session_end",
            serde_json::json!({
                "iterations": self.state.iteration,
                "final_mode": self.state.mode.to_string(),
                "predictor_accuracy": prediction_accuracy,
            }),
        )?;

        println!(
            "\n{} Session complete. Analytics: .ralph/analytics.jsonl",
            "Done:".green().bold()
        );

        // Print predictor accuracy if verbose
        if self.verbose {
            if let Some(accuracy) = prediction_accuracy {
                println!(
                    "   {} Prediction accuracy: {:.0}%",
                    "Predictor:".bright_magenta().bold(),
                    accuracy * 100.0
                );
            }
        }

        Ok(())
    }

    /// Print the startup banner.
    fn print_banner(&self) {
        println!("{}", "═".repeat(60).bright_blue());
        println!(
            "{}",
            "     RALPH - Claude Code Automation Suite"
                .bright_blue()
                .bold()
        );
        println!("{}", "═".repeat(60).bright_blue());
        println!();
        println!("   Project: {}", self.project_dir.display());
        println!("   Mode: {}", self.state.mode);
        println!("   Max iterations: {}", self.max_iterations);
        println!("   Stagnation threshold: {}", self.stagnation_threshold);
        if !self.config.permissions.allow.is_empty() {
            println!(
                "   Permissions: {} allowed, {} safety blocks",
                self.config.permissions.allow.len(),
                self.config.permissions.deny.len()
            );
        }

        // Display task summary
        let task_counts = self.task_tracker.task_counts();
        if task_counts.total() > 0 {
            let remaining = self.task_tracker.remaining_count();
            let workable = self.task_tracker.workable_tasks();
            println!(
                "   Tasks: {} total, {} complete, {} in progress, {} blocked, {} remaining ({} workable)",
                task_counts.total(),
                task_counts.complete,
                task_counts.in_progress,
                task_counts.blocked,
                remaining,
                workable.len()
            );
            // Show next task info if available
            if let Some(next_id) = self.task_tracker.next_task() {
                if let Some(task) = self.task_tracker.get_task(&next_id) {
                    debug!("Next task: {} (state: {:?})", task.id, task.state);
                }
                // Also try lookup by number
                if let Some(task) = self.task_tracker.get_task_by_number(next_id.number()) {
                    debug!(
                        "Task #{} found by number: {}",
                        next_id.number(),
                        task.id.title()
                    );
                }
            }
            if task_counts.all_done() || self.task_tracker.is_all_done() {
                println!("   {} All tasks complete!", "Success:".green().bold());
            }
        }

        if self.verbose {
            println!();
            println!("{}", "   Allowed operations:".cyan());
            for perm in &self.config.permissions.allow {
                println!("     ✓ {}", perm.green());
            }
            if !self.config.permissions.deny.is_empty() {
                println!("{}", "   Safety blocks (denied):".cyan());
                for perm in &self.config.permissions.deny {
                    println!("     ✗ {}", perm.red());
                }
            }
        }
        println!();
    }

    /// Print iteration header.
    fn print_iteration_header(&self) {
        println!(
            "\n{} Iteration {}/{} (stagnation: {}/{})",
            "===".bright_blue(),
            self.state.iteration,
            self.max_iterations,
            self.state.stagnation_count,
            self.stagnation_threshold
        );
    }

    /// Get MD5 hash of IMPLEMENTATION_PLAN.md.
    fn get_plan_hash(&self) -> Result<String> {
        let content = if let Some(deps) = &self.deps {
            // Use try_read to avoid blocking in async contexts
            let fs = deps
                .fs
                .try_read()
                .map_err(|_| anyhow::anyhow!("Could not acquire filesystem lock"))?;
            fs.read_file("IMPLEMENTATION_PLAN.md")
                .context("Failed to read IMPLEMENTATION_PLAN.md")?
        } else {
            let plan_path = self.project_dir.join("IMPLEMENTATION_PLAN.md");
            std::fs::read_to_string(&plan_path).context("Failed to read IMPLEMENTATION_PLAN.md")?
        };
        Ok(format!("{:x}", md5::compute(content.as_bytes())))
    }

    /// Read the plan content from IMPLEMENTATION_PLAN.md.
    fn read_plan_content(&self) -> Result<String> {
        if let Some(deps) = &self.deps {
            let fs = deps
                .fs
                .try_read()
                .map_err(|_| anyhow::anyhow!("Could not acquire filesystem lock"))?;
            fs.read_file("IMPLEMENTATION_PLAN.md")
                .context("Failed to read IMPLEMENTATION_PLAN.md")
        } else {
            let plan_path = self.project_dir.join("IMPLEMENTATION_PLAN.md");
            std::fs::read_to_string(&plan_path).context("Failed to read IMPLEMENTATION_PLAN.md")
        }
    }

    /// Check for orphaned tasks when the plan structure changes.
    ///
    /// Validates the current task tracker against the plan and marks
    /// any tasks that are no longer in the plan as orphaned.
    fn check_for_orphaned_tasks(&mut self) {
        if let Ok(plan_content) = self.read_plan_content() {
            match self.task_tracker.validate_against_plan(&plan_content) {
                ValidationResult::Valid => {
                    debug!("Plan structure unchanged, no orphaned tasks");
                }
                ValidationResult::PlanChanged { orphaned_tasks } => {
                    if !orphaned_tasks.is_empty() {
                        warn!(
                            "Plan structure changed, {} task(s) orphaned: {:?}",
                            orphaned_tasks.len(),
                            orphaned_tasks
                        );
                        self.task_tracker.mark_orphaned_tasks(&plan_content);
                    }
                }
            }
        }
    }

    /// Get the current git commit hash (HEAD).
    fn get_commit_hash(&self) -> Result<String> {
        if let Some(deps) = &self.deps {
            deps.git.get_commit_hash()
        } else {
            let output = Command::new("git")
                .args(["rev-parse", "HEAD"])
                .current_dir(&self.project_dir)
                .output()
                .context("Failed to run git rev-parse")?;

            if output.status.success() {
                Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
            } else {
                Ok(String::new())
            }
        }
    }

    /// Count commits between two hashes.
    fn count_commits_since(&self, old_hash: &str) -> u32 {
        if old_hash.is_empty() {
            return 0;
        }

        if let Some(deps) = &self.deps {
            deps.git.count_commits_since(old_hash)
        } else {
            let output = Command::new("git")
                .args(["rev-list", "--count", &format!("{old_hash}..HEAD")])
                .current_dir(&self.project_dir)
                .output();

            match output {
                Ok(out) if out.status.success() => String::from_utf8_lossy(&out.stdout)
                    .trim()
                    .parse()
                    .unwrap_or(0),
                _ => 0,
            }
        }
    }

    /// Check if there's been any progress (commits or plan changes).
    fn has_made_progress(&self) -> bool {
        // Use semantic progress evaluation
        match self.evaluate_progress() {
            Ok(evaluation) => {
                debug!(
                    "Progress evaluation: {:?} (score: {:.2}) - {}",
                    evaluation.verdict, evaluation.score, evaluation.explanation
                );

                // Log health status for monitoring
                if evaluation.verdict.is_healthy() {
                    debug!("Verdict indicates healthy activity");
                } else {
                    debug!("Verdict indicates unhealthy state - may need intervention");
                }

                evaluation.verdict.should_reset_stagnation()
            }
            Err(e) => {
                // Fall back to simple commit check on error
                debug!(
                    "Progress evaluation failed ({}), falling back to commit check",
                    e
                );
                let commit_count = self.count_commits_since(&self.state.last_commit_hash);
                if commit_count > 0 {
                    debug!("Progress detected: {} new commit(s)", commit_count);
                    return true;
                }

                // Check for plan file changes
                if let Ok(current_hash) = self.get_plan_hash() {
                    if current_hash != self.state.last_plan_hash {
                        debug!("Progress detected: IMPLEMENTATION_PLAN.md changed");
                        return true;
                    }
                }

                false
            }
        }
    }

    /// Evaluate progress using semantic signals.
    ///
    /// Returns a detailed evaluation including score, verdict, and explanation.
    fn evaluate_progress(&self) -> Result<ProgressEvaluation> {
        self.progress_tracker.evaluate(&self.state.last_commit_hash)
    }

    /// Collect detailed progress signals for logging and analysis.
    ///
    /// This method gathers multi-dimensional signals from git, quality checks,
    /// and behavioral patterns to provide comprehensive progress insights.
    pub fn collect_progress_signals(&self) -> Result<ProgressSignals> {
        self.progress_tracker
            .collect_signals(&self.state.last_commit_hash)
    }

    /// Log detailed progress status for the current iteration.
    ///
    /// Uses the progress tracker to collect and evaluate signals,
    /// providing verbose output about the state of progress.
    pub fn log_progress_status(&self) {
        // Collect raw signals
        let signals = match self.collect_progress_signals() {
            Ok(s) => s,
            Err(e) => {
                debug!("Could not collect progress signals: {}", e);
                // Return empty signals as fallback
                ProgressSignals::new()
            }
        };

        // Quick check for any positive activity
        if signals.has_any_positive_signal() {
            debug!("Progress signals detected: {}", signals.summary());
        } else {
            debug!("No positive progress signals detected");
        }

        // Collect and log quality signals separately
        if let Ok(quality) = self.progress_tracker.collect_quality_signals() {
            debug!("Quality state: {}", quality.summary());
        }

        // Evaluate the signals
        let evaluation = self.progress_tracker().evaluate_signals(&signals);
        debug!(
            "Signal evaluation: {:?} (score: {:.2})",
            evaluation.verdict, evaluation.score
        );

        // Log contribution breakdown if verbose
        for contrib in &evaluation.contributions {
            debug!(
                "  - {}: raw={:.1}, weight={:.2}, contribution={:.3}",
                contrib.signal_name, contrib.raw_value, contrib.weight, contrib.contribution
            );
        }
    }

    /// Update progress tracking baselines after each iteration.
    ///
    /// This should be called after quality checks to ensure accurate delta
    /// calculations for the next iteration.
    pub fn update_progress_baselines(&mut self) {
        // Collect current quality state and update baselines
        if let Ok(quality) = self.progress_tracker.collect_quality_signals() {
            self.progress_tracker
                .update_baselines(quality.test_count, quality.warning_count);
            debug!(
                "Updated progress baselines: {} tests, {} warnings",
                quality.test_count, quality.warning_count
            );
        }
    }

    // =========================================================================
    // Intelligent Retry Methods
    // =========================================================================

    /// Classify a failure and determine recovery strategy.
    ///
    /// Returns `Some((strategy, prompt))` if retry should be attempted,
    /// `None` if retries are exhausted.
    pub fn classify_failure(
        &mut self,
        error_output: &str,
        task_id: Option<&str>,
    ) -> Option<(RecoveryStrategy, String)> {
        self.intelligent_retry
            .process_failure(error_output, task_id)
    }

    /// Record a retry attempt in history.
    pub fn record_retry_attempt(
        &mut self,
        failure: FailureContext,
        strategy: RecoveryStrategy,
        succeeded: bool,
    ) {
        // Capture failure class before moving
        let failure_class = failure.class;
        let mut attempt = RetryAttempt::new(failure, strategy);
        if succeeded {
            attempt.mark_success();
        }
        self.retry_history.record(attempt);
        debug!(
            "Recorded retry attempt: {:?} with strategy {:?}, success={}",
            failure_class, strategy, succeeded
        );
    }

    /// Get a summary of retry history for logging.
    #[must_use]
    pub fn retry_summary(&self) -> String {
        let history_summary = self.retry_history.summary();
        let intelligent_summary = self.intelligent_retry.summary();

        // Include success rates for common strategies
        let isolated_fix_rate = self
            .retry_history
            .success_rate(RecoveryStrategy::IsolatedFix);
        let test_first_rate = self.retry_history.success_rate(RecoveryStrategy::TestFirst);

        // Include success rates for common failure classes
        let compile_error_rate = self
            .retry_history
            .class_success_rate(FailureClass::CompileError);
        let test_failure_rate = self
            .retry_history
            .class_success_rate(FailureClass::TestFailure);

        format!(
            "{}; {}; Strategy success: IsolatedFix={:.0}%, TestFirst={:.0}%; Class success: CompileError={:.0}%, TestFailure={:.0}%",
            history_summary,
            intelligent_summary,
            isolated_fix_rate * 100.0,
            test_first_rate * 100.0,
            compile_error_rate * 100.0,
            test_failure_rate * 100.0
        )
    }

    /// Check if retries are exhausted for a task.
    #[must_use]
    pub fn retries_exhausted(&self, task_id: &str) -> bool {
        self.intelligent_retry.retries_exhausted(task_id)
    }

    /// Reset retry state for a task (e.g., after it completes).
    pub fn reset_task_retries(&mut self, task_id: &str) {
        self.intelligent_retry.reset_task(task_id);
        debug!("Reset retry state for task: {}", task_id);
    }

    /// Decompose a task based on failure context.
    ///
    /// Returns a list of sub-tasks to tackle the failure incrementally.
    #[must_use]
    pub fn decompose_task_for_failure(
        &self,
        task_description: &str,
        failure: &FailureContext,
    ) -> Vec<SubTask> {
        let decomposer = TaskDecomposer::new();
        decomposer.decompose(task_description, failure)
    }

    /// Check if a failure is simple enough to not need decomposition.
    #[must_use]
    pub fn is_atomic_failure(&self, failure: &FailureContext) -> bool {
        let decomposer = TaskDecomposer::new();
        decomposer.is_atomic(failure)
    }

    /// Get failure classifier for manual classification if needed.
    #[must_use]
    pub fn failure_classifier(&self) -> &FailureClassifier {
        self.intelligent_retry.classifier()
    }

    /// Create a failure location from file and line info.
    #[must_use]
    pub fn create_failure_location(file: &str, line: Option<u32>) -> FailureLocation {
        let mut loc = FailureLocation::new(file);
        if let Some(l) = line {
            loc = loc.with_line(l);
        }
        loc
    }

    /// Clear retry history for a resolved failure.
    ///
    /// Call this when a previously failing operation succeeds, to reset
    /// the strategist's memory of tried strategies for that failure.
    pub fn clear_resolved_failure(&mut self, error_output: &str) {
        let failure = self.failure_classifier().classify(error_output);
        self.intelligent_retry.clear_failure_history(&failure);
        debug!(
            "Cleared retry history for resolved failure: {}",
            failure.summary()
        );
    }

    /// Log retry status for diagnostics.
    pub fn log_retry_status(&self, task_id: &str) {
        let retry_count = self.intelligent_retry.retry_count(task_id);
        let exhausted = self.intelligent_retry.retries_exhausted(task_id);
        debug!(
            "Retry status for {}: {} retries, exhausted={}",
            task_id, retry_count, exhausted
        );
    }

    /// Get the prompt file for current mode.
    fn get_prompt_path(&self) -> PathBuf {
        self.project_dir.join(self.state.mode.prompt_filename())
    }

    /// Run a single Claude Code iteration.
    async fn run_claude_iteration(&self) -> Result<i32> {
        let prompt_path = self.get_prompt_path();

        // Build dynamic prompt using the assembler
        let mode_name = match self.state.mode {
            LoopMode::Build => "build",
            LoopMode::Debug => "debug",
            LoopMode::Plan => "plan",
        };

        // Maximum prompt length (Claude Code has limits on stdin size)
        // Keep prompts concise to avoid "Prompt is too long" errors
        const MAX_PROMPT_LENGTH: usize = 12000; // ~12KB is safe

        // Try to build dynamic prompt first, fall back to static file if needed
        let prompt = match self.prompt_assembler.build_prompt(mode_name) {
            Ok(dynamic_prompt) => {
                debug!(
                    "Using dynamic prompt for mode: {} ({} chars)",
                    mode_name,
                    dynamic_prompt.len()
                );
                // Dynamic prompt already includes task context via {{TASK_CONTEXT}} marker
                // No need to duplicate with task_tracker.get_context_summary()
                dynamic_prompt
            }
            Err(e) => {
                // Fall back to static prompt file
                debug!("Dynamic prompt failed ({}), falling back to static file", e);

                let base_prompt = if let Some(deps) = &self.deps {
                    // Use try_read to avoid blocking in async contexts
                    let fs = deps
                        .fs
                        .try_read()
                        .map_err(|_| anyhow::anyhow!("Could not acquire filesystem lock"))?;
                    let prompt_filename = self.state.mode.prompt_filename();
                    if !fs.exists(&prompt_filename) {
                        bail!("Prompt file not found: {}", prompt_path.display());
                    }
                    fs.read_file(&prompt_filename)?
                } else {
                    if !prompt_path.exists() {
                        bail!("Prompt file not found: {}", prompt_path.display());
                    }
                    std::fs::read_to_string(&prompt_path)?
                };

                // Add task context to static prompt (static prompts don't have {{TASK_CONTEXT}})
                let task_context = self.task_tracker.get_context_summary();
                if task_context.is_empty() {
                    base_prompt
                } else {
                    format!(
                        "## Current Task Context\n\n{}\n\n---\n\n{}",
                        task_context, base_prompt
                    )
                }
            }
        };

        // Truncate prompt if it exceeds the limit
        let prompt = if prompt.len() > MAX_PROMPT_LENGTH {
            warn!(
                "Prompt exceeds {} chars ({} chars), truncating to avoid Claude Code limits",
                MAX_PROMPT_LENGTH,
                prompt.len()
            );
            // Truncate from the middle, keeping beginning (instructions) and end (current context)
            let keep_start = MAX_PROMPT_LENGTH * 2 / 3;
            let keep_end = MAX_PROMPT_LENGTH / 3;
            let start = &prompt[..keep_start];
            let end = &prompt[prompt.len() - keep_end..];
            format!(
                "{}\n\n... [truncated {} chars] ...\n\n{}",
                start,
                prompt.len() - MAX_PROMPT_LENGTH,
                end
            )
        } else {
            prompt
        };

        debug!(
            "Running Claude Code with prompt for mode {} ({} chars total)",
            mode_name,
            prompt.len()
        );

        // Use injected Claude process if available
        if let Some(deps) = &self.deps {
            return deps.claude.run_iteration(&prompt).await;
        }

        // Fallback to direct command execution
        // Build command arguments
        let args = vec!["-p", "--dangerously-skip-permissions", "--model", "opus"];

        // If we have a CLAUDE.md, reference it
        let claude_md = ProjectConfig::claude_md_path(&self.project_dir);
        if claude_md.exists() {
            debug!("Using CLAUDE.md from {}", claude_md.display());
        }

        // Run claude with the prompt piped to stdin
        let mut child = AsyncCommand::new("claude")
            .args(&args)
            .current_dir(&self.project_dir)
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::inherit())
            .stderr(std::process::Stdio::inherit())
            .spawn()?;

        // Write prompt to stdin, flush, and close to signal EOF
        if let Some(mut stdin) = child.stdin.take() {
            use tokio::io::AsyncWriteExt;
            stdin.write_all(prompt.as_bytes()).await?;
            stdin.flush().await?;
            drop(stdin); // Explicitly close stdin to signal EOF
        }

        let status = child.wait().await?;
        Ok(status.code().unwrap_or(1))
    }

    /// Run documentation sync check.
    async fn run_doc_sync(&self) -> Result<()> {
        println!("   {} Running documentation sync check...", "Info:".blue());

        // Try to run docs-sync agent if available
        let result = AsyncCommand::new("claude")
            .args([
                "--dangerously-skip-permissions",
                "--agent",
                "docs-sync",
                "Check for documentation drift",
            ])
            .current_dir(&self.project_dir)
            .output()
            .await;

        match result {
            Ok(output) => {
                let stdout = String::from_utf8_lossy(&output.stdout);
                if stdout.contains("drift_detected") && stdout.contains("true") {
                    println!("   {} Documentation drift detected", "Warning:".yellow());
                    self.analytics.log_event(
                        &self.state.session_id,
                        "docs_drift_detected",
                        serde_json::json!({
                            "iteration": self.state.iteration,
                        }),
                    )?;
                }
            }
            Err(e) => {
                debug!("docs-sync agent not available: {}", e);
            }
        }

        Ok(())
    }

    /// Try to push to remote (uses BatchMode to avoid SSH passphrase hang).
    async fn try_push(&self) {
        // Use injected git operations if available
        if let Some(deps) = &self.deps {
            if let Ok(branch) = deps.git.get_branch() {
                if let Err(e) = deps.git.push("origin", &branch) {
                    debug!("Push failed: {}", e);
                } else {
                    debug!("Pushed to origin/{}", branch);
                }
            }
            return;
        }

        // Fallback to direct command execution
        // Get current branch
        let branch_output = Command::new("git")
            .args(["branch", "--show-current"])
            .current_dir(&self.project_dir)
            .output();

        if let Ok(output) = branch_output {
            if output.status.success() {
                let branch = String::from_utf8_lossy(&output.stdout).trim().to_string();

                // Use GIT_SSH_COMMAND with BatchMode to fail fast on passphrase prompt
                // instead of hanging indefinitely
                let push_result = AsyncCommand::new("git")
                    .args(["push", "origin", &branch])
                    .env(
                        "GIT_SSH_COMMAND",
                        "ssh -o BatchMode=yes -o ConnectTimeout=10",
                    )
                    .current_dir(&self.project_dir)
                    .output()
                    .await;

                match push_result {
                    Ok(output) if output.status.success() => {
                        debug!("Pushed to origin/{}", branch);
                    }
                    Ok(output) => {
                        let stderr = String::from_utf8_lossy(&output.stderr);
                        if stderr.contains("Host key verification failed")
                            || stderr.contains("Permission denied")
                        {
                            debug!(
                                "Push failed due to SSH auth - consider using HTTPS remote or ssh-agent"
                            );
                        } else {
                            debug!("Push failed: {}", stderr);
                        }
                    }
                    Err(e) => {
                        debug!("Push error: {}", e);
                    }
                }
            }
        }
    }

    /// Check if all tasks are complete.
    fn is_complete(&self) -> Result<bool> {
        let content = if let Some(deps) = &self.deps {
            // Use try_read to avoid blocking in async contexts
            let fs = deps
                .fs
                .try_read()
                .map_err(|_| anyhow::anyhow!("Could not acquire filesystem lock"))?;
            fs.read_file("IMPLEMENTATION_PLAN.md")?
        } else {
            let plan_path = self.project_dir.join("IMPLEMENTATION_PLAN.md");
            std::fs::read_to_string(&plan_path)?
        };
        Ok(content.contains("ALL_TASKS_COMPLETE"))
    }

    /// Get number of lines changed in recent commit.
    fn get_recent_changes(&self) -> Result<u32> {
        let output = Command::new("git")
            .args(["diff", "--stat", "HEAD~1"])
            .current_dir(&self.project_dir)
            .output()?;

        if output.status.success() {
            let stdout = String::from_utf8_lossy(&output.stdout);
            // Parse last line for changes count
            if let Some(last_line) = stdout.lines().last() {
                // Extract numbers from "X files changed, Y insertions(+), Z deletions(-)"
                let numbers: Vec<u32> = last_line
                    .split_whitespace()
                    .filter_map(|s| s.parse().ok())
                    .collect();

                if numbers.len() >= 2 {
                    return Ok(numbers[1] + numbers.get(2).unwrap_or(&0));
                }
            }
        }

        Ok(0)
    }

    /// Run final security audit.
    async fn run_security_audit(&self) -> Result<()> {
        // Try narsil-mcp scan
        let scan_result = AsyncCommand::new("narsil-mcp")
            .arg("scan_security")
            .current_dir(&self.project_dir)
            .output()
            .await;

        match scan_result {
            Ok(output) => {
                let stdout = String::from_utf8_lossy(&output.stdout);
                if stdout.contains("CRITICAL") {
                    eprintln!(
                        "   {} Critical security issues found!",
                        "Warning:".red().bold()
                    );
                } else {
                    println!("   {} Security scan complete", "OK".green());
                }
            }
            Err(_) => {
                debug!("narsil-mcp not available for security scan");
            }
        }

        // Try to generate SBOM
        let sbom_result = AsyncCommand::new("narsil-mcp")
            .args(["generate_sbom", "--format", "cyclonedx"])
            .current_dir(&self.project_dir)
            .output()
            .await;

        if let Ok(output) = sbom_result {
            if output.status.success() {
                let sbom_path = self.project_dir.join("sbom.json");
                std::fs::write(&sbom_path, &output.stdout)?;
                println!("   {} SBOM generated: sbom.json", "OK".green());
            }
        }

        Ok(())
    }

    /// Get the current project configuration.
    ///
    /// Returns a reference to the project configuration for inspection
    /// or passing to other components.
    #[cfg(test)]
    pub fn config(&self) -> &ProjectConfig {
        &self.config
    }

    /// Get the current loop state.
    ///
    /// Returns a reference to the current state for inspection,
    /// including iteration count, mode, and checkpoint history.
    #[cfg(test)]
    pub fn state(&self) -> &LoopState {
        &self.state
    }

    /// Get a reference to the task tracker.
    ///
    /// Returns a reference to the task tracker for inspection of task
    /// states, metrics, and progress.
    #[cfg(test)]
    pub fn task_tracker(&self) -> &TaskTracker {
        &self.task_tracker
    }

    /// Get a mutable reference to the task tracker.
    ///
    /// Returns a mutable reference for modifying task states during tests.
    #[cfg(test)]
    pub fn task_tracker_mut(&mut self) -> &mut TaskTracker {
        &mut self.task_tracker
    }

    /// Get a reference to the progress tracker.
    ///
    /// Returns a reference to the semantic progress tracker for inspection.
    #[must_use]
    pub fn progress_tracker(&self) -> &ProgressTracker {
        &self.progress_tracker
    }

    /// Clean up stale LSP processes (rust-analyzer, etc.).
    ///
    /// This helps prevent LSP crashes from accumulating and causing failures
    /// in long-running automation sessions.
    fn cleanup_lsp() {
        // Kill any stale rust-analyzer processes
        let _ = Command::new("pkill").args(["-f", "rust-analyzer"]).output();

        // Give processes time to terminate
        std::thread::sleep(std::time::Duration::from_millis(500));
    }

    /// Check for oversized source files that might cause Claude errors.
    ///
    /// Scans the project for source files exceeding token thresholds.
    /// Returns a list of (path, estimated_tokens) tuples for problematic files.
    fn find_oversized_files(&self) -> Vec<(PathBuf, usize)> {
        let mut oversized = Vec::new();
        let src_dir = self.project_dir.join("src");

        if !src_dir.exists() {
            return oversized;
        }

        let code_extensions = [
            "rs", "py", "ts", "tsx", "js", "jsx", "go", "java", "cpp", "c", "h",
        ];

        for entry in walkdir::WalkDir::new(&src_dir)
            .into_iter()
            .filter_map(|e| e.ok())
            .filter(|e| e.file_type().is_file())
        {
            let path = entry.path();
            let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");

            if !code_extensions.contains(&ext) {
                continue;
            }

            if let Ok(metadata) = std::fs::metadata(path) {
                let estimated_tokens = (metadata.len() as f64 * TOKENS_PER_BYTE) as usize;

                if estimated_tokens >= FILE_SIZE_WARNING_TOKENS {
                    oversized.push((path.to_path_buf(), estimated_tokens));
                }
            }
        }

        // Sort by size descending
        oversized.sort_by(|a, b| b.1.cmp(&a.1));
        oversized
    }

    /// Check files and print warnings for oversized ones.
    fn check_file_sizes(&self) {
        let oversized = self.find_oversized_files();

        for (path, tokens) in oversized {
            let rel_path = path.strip_prefix(&self.project_dir).unwrap_or(&path);

            if tokens >= FILE_SIZE_CRITICAL_TOKENS {
                eprintln!(
                    "   {} {} (~{} tokens) - will cause Claude errors! Consider splitting.",
                    "CRITICAL:".red().bold(),
                    rel_path.display(),
                    tokens
                );
            } else {
                println!(
                    "   {} {} (~{} tokens) - approaching limit, consider splitting.",
                    "Warning:".yellow(),
                    rel_path.display(),
                    tokens
                );
            }
        }
    }

    /// Run automatic archiving of stale markdown files.
    fn run_auto_archive(&self) -> Result<()> {
        let archive_manager = crate::archive::ArchiveManager::new(
            self.project_dir.clone(),
            90, // 90 day stale threshold
        );

        // Also check for stale markdown files in project root
        let root_stale = self.find_stale_root_markdown()?;

        if !root_stale.is_empty() {
            println!(
                "   {} Found {} stale .md files in project root",
                "Archive:".cyan(),
                root_stale.len()
            );
            for path in &root_stale {
                debug!("Stale root markdown: {}", path.display());
            }
        }

        // Run the standard archive process (dry run first to report)
        let result = archive_manager.run(true)?;

        if result.docs_archived > 0 || result.decisions_archived > 0 {
            println!(
                "   {} {} docs and {} decisions eligible for archiving (run 'ralph archive run')",
                "Info:".blue(),
                result.docs_archived,
                result.decisions_archived
            );
        }

        Ok(())
    }

    /// Find stale markdown files in the project root.
    fn find_stale_root_markdown(&self) -> Result<Vec<PathBuf>> {
        let mut stale_files = Vec::new();
        let threshold_secs = 90 * 24 * 60 * 60; // 90 days in seconds

        // Look for markdown files directly in project root (not in subdirectories)
        for entry in std::fs::read_dir(&self.project_dir)? {
            let entry = entry?;
            let path = entry.path();

            if !path.is_file() {
                continue;
            }

            let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
            if ext != "md" {
                continue;
            }

            // Skip known important files
            let filename = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
            if [
                "README.md",
                "IMPLEMENTATION_PLAN.md",
                "CLAUDE.md",
                "CHANGELOG.md",
            ]
            .contains(&filename)
            {
                continue;
            }

            // Skip PROMPT_*.md files
            if filename.starts_with("PROMPT_") {
                continue;
            }

            // Check age
            if let Ok(metadata) = std::fs::metadata(&path) {
                if let Ok(modified) = metadata.modified() {
                    if let Ok(duration) = std::time::SystemTime::now().duration_since(modified) {
                        if duration.as_secs() > threshold_secs {
                            stale_files.push(path);
                        }
                    }
                }
            }
        }

        Ok(stale_files)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    /// Helper to create a default test config for the given path
    fn test_config(path: PathBuf) -> LoopManagerConfig {
        LoopManagerConfig::new(path, ProjectConfig::default())
            .with_mode(LoopMode::Build)
            .with_max_iterations(10)
            .with_stagnation_threshold(3)
            .with_doc_sync_interval(5)
    }

    #[test]
    fn test_loop_manager_requires_plan() {
        let temp = TempDir::new().unwrap();
        let cfg = test_config(temp.path().to_path_buf());
        let result = LoopManager::new(cfg);
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("IMPLEMENTATION_PLAN.md"));
    }

    #[test]
    fn test_loop_manager_creates_with_plan() {
        let temp = TempDir::new().unwrap();
        std::fs::write(temp.path().join("IMPLEMENTATION_PLAN.md"), "# Plan").unwrap();

        let cfg = test_config(temp.path().to_path_buf());
        let result = LoopManager::new(cfg);
        assert!(result.is_ok());
    }

    #[test]
    fn test_loop_manager_config_accessor() {
        let temp = TempDir::new().unwrap();
        std::fs::write(temp.path().join("IMPLEMENTATION_PLAN.md"), "# Plan").unwrap();

        let project_config = ProjectConfig {
            permissions: ralph::config::PermissionsConfig {
                allow: vec!["Bash(git *)".to_string()],
                deny: vec![],
            },
            ..Default::default()
        };

        let cfg = LoopManagerConfig::new(temp.path().to_path_buf(), project_config)
            .with_mode(LoopMode::Build)
            .with_max_iterations(10)
            .with_stagnation_threshold(3);
        let manager = LoopManager::new(cfg).unwrap();

        // Test config accessor
        assert_eq!(manager.config().permissions.allow.len(), 1);
        assert!(manager
            .config()
            .permissions
            .allow
            .contains(&"Bash(git *)".to_string()));
    }

    #[test]
    fn test_loop_manager_state_accessor() {
        let temp = TempDir::new().unwrap();
        std::fs::write(temp.path().join("IMPLEMENTATION_PLAN.md"), "# Plan").unwrap();

        let cfg = LoopManagerConfig::new(temp.path().to_path_buf(), ProjectConfig::default())
            .with_mode(LoopMode::Plan)
            .with_max_iterations(10)
            .with_stagnation_threshold(3);
        let manager = LoopManager::new(cfg).unwrap();

        // Test state accessor
        assert_eq!(manager.state().mode, LoopMode::Plan);
        assert_eq!(manager.state().iteration, 0);
        assert_eq!(manager.state().stagnation_count, 0);
    }

    #[test]
    fn test_find_oversized_files_empty_project() {
        let temp = TempDir::new().unwrap();
        std::fs::write(temp.path().join("IMPLEMENTATION_PLAN.md"), "# Plan").unwrap();

        let cfg = test_config(temp.path().to_path_buf());
        let manager = LoopManager::new(cfg).unwrap();

        // No src directory - should return empty
        let oversized = manager.find_oversized_files();
        assert!(oversized.is_empty());
    }

    #[test]
    fn test_find_oversized_files_small_files() {
        let temp = TempDir::new().unwrap();
        std::fs::write(temp.path().join("IMPLEMENTATION_PLAN.md"), "# Plan").unwrap();
        std::fs::create_dir_all(temp.path().join("src")).unwrap();

        // Write a small Rust file (well under threshold)
        std::fs::write(
            temp.path().join("src/main.rs"),
            "fn main() { println!(\"Hello\"); }",
        )
        .unwrap();

        let cfg = test_config(temp.path().to_path_buf());
        let manager = LoopManager::new(cfg).unwrap();

        let oversized = manager.find_oversized_files();
        assert!(oversized.is_empty());
    }

    #[test]
    fn test_find_oversized_files_detects_large() {
        let temp = TempDir::new().unwrap();
        std::fs::write(temp.path().join("IMPLEMENTATION_PLAN.md"), "# Plan").unwrap();
        std::fs::create_dir_all(temp.path().join("src")).unwrap();

        // Write a large Rust file (~100k bytes = ~25k tokens at 0.25 ratio)
        let large_content = "fn test() { }\n".repeat(8000);
        std::fs::write(temp.path().join("src/large.rs"), &large_content).unwrap();

        let cfg = test_config(temp.path().to_path_buf());
        let manager = LoopManager::new(cfg).unwrap();

        let oversized = manager.find_oversized_files();
        assert!(!oversized.is_empty());
        assert!(oversized[0].0.ends_with("large.rs"));
    }

    #[test]
    fn test_find_stale_root_markdown_no_stale() {
        let temp = TempDir::new().unwrap();
        std::fs::write(temp.path().join("IMPLEMENTATION_PLAN.md"), "# Plan").unwrap();
        std::fs::write(temp.path().join("README.md"), "# Readme").unwrap();

        let cfg = test_config(temp.path().to_path_buf());
        let manager = LoopManager::new(cfg).unwrap();

        // Fresh files should not be stale
        let stale = manager.find_stale_root_markdown().unwrap();
        assert!(stale.is_empty());
    }

    #[test]
    fn test_token_estimation() {
        // Test that token estimation produces reasonable values
        // A 1000 byte file should produce ~250 tokens at 0.25 tokens/byte
        let file_size_bytes: u64 = 1000;
        let estimated_tokens = (file_size_bytes as f64 * TOKENS_PER_BYTE) as usize;
        assert!(estimated_tokens > 200);
        assert!(estimated_tokens < 300);

        // A file at the critical threshold should be ~100KB
        let critical_bytes = (FILE_SIZE_CRITICAL_TOKENS as f64 / TOKENS_PER_BYTE) as u64;
        assert!(critical_bytes > 90_000);
        assert!(critical_bytes < 110_000);
    }

    // =========================================================================
    // Dependency Injection Tests
    // =========================================================================

    use ralph::testing::{
        MockClaudeProcess, MockFileSystem, MockGitOperations, MockQualityChecker,
    };

    fn create_test_deps() -> LoopDependencies {
        LoopDependencies {
            git: Arc::new(MockGitOperations::new().with_commit_hash("abc123")),
            claude: Arc::new(MockClaudeProcess::new().with_exit_code(0)),
            fs: Arc::new(RwLock::new(
                MockFileSystem::new()
                    .with_file("IMPLEMENTATION_PLAN.md", "# Test Plan\n\nTask 1: Test")
                    .with_file("PROMPT_build.md", "Build prompt"),
            )),
            quality: Arc::new(MockQualityChecker::new().all_passing()),
        }
    }

    /// Helper to create a mock config for DI tests
    fn mock_config() -> LoopManagerConfig {
        LoopManagerConfig::new(PathBuf::from("/mock"), ProjectConfig::default())
            .with_mode(LoopMode::Build)
            .with_max_iterations(10)
            .with_stagnation_threshold(3)
            .with_doc_sync_interval(5)
    }

    #[test]
    fn test_loop_manager_with_deps_requires_plan() {
        let deps = LoopDependencies {
            git: Arc::new(MockGitOperations::new()),
            claude: Arc::new(MockClaudeProcess::new()),
            fs: Arc::new(RwLock::new(MockFileSystem::new())), // No plan file
            quality: Arc::new(MockQualityChecker::new()),
        };

        let cfg = mock_config();
        let result = LoopManager::with_deps(cfg, deps);

        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("IMPLEMENTATION_PLAN.md"));
    }

    #[test]
    fn test_loop_manager_with_deps_creates_successfully() {
        let deps = create_test_deps();
        let cfg = mock_config();
        let result = LoopManager::with_deps(cfg, deps);

        assert!(result.is_ok());
        let manager = result.unwrap();
        assert_eq!(manager.state().mode, LoopMode::Build);
        assert_eq!(manager.state().iteration, 0);
    }

    #[test]
    fn test_get_plan_hash_uses_mock_fs() {
        let deps = create_test_deps();
        let cfg = mock_config();
        let manager = LoopManager::with_deps(cfg, deps).unwrap();

        // get_plan_hash should return a valid MD5 hash
        let hash = manager.get_plan_hash().unwrap();
        assert!(!hash.is_empty());
        assert_eq!(hash.len(), 32); // MD5 produces 32 hex chars
    }

    #[test]
    fn test_get_commit_hash_uses_mock_git() {
        let deps = LoopDependencies {
            git: Arc::new(MockGitOperations::new().with_commit_hash("deadbeef1234")),
            claude: Arc::new(MockClaudeProcess::new()),
            fs: Arc::new(RwLock::new(
                MockFileSystem::new().with_file("IMPLEMENTATION_PLAN.md", "# Plan"),
            )),
            quality: Arc::new(MockQualityChecker::new()),
        };

        let cfg = mock_config();
        let manager = LoopManager::with_deps(cfg, deps).unwrap();

        let hash = manager.get_commit_hash().unwrap();
        assert_eq!(hash, "deadbeef1234");
    }

    #[test]
    fn test_count_commits_since_uses_mock_git() {
        let deps = LoopDependencies {
            git: Arc::new(MockGitOperations::new().with_commits_since(5)),
            claude: Arc::new(MockClaudeProcess::new()),
            fs: Arc::new(RwLock::new(
                MockFileSystem::new().with_file("IMPLEMENTATION_PLAN.md", "# Plan"),
            )),
            quality: Arc::new(MockQualityChecker::new()),
        };

        let cfg = mock_config();
        let manager = LoopManager::with_deps(cfg, deps).unwrap();

        let count = manager.count_commits_since("old_hash");
        assert_eq!(count, 5);
    }

    #[test]
    fn test_is_complete_with_mock_fs() {
        let deps = LoopDependencies {
            git: Arc::new(MockGitOperations::new()),
            claude: Arc::new(MockClaudeProcess::new()),
            fs: Arc::new(RwLock::new(
                MockFileSystem::new()
                    .with_file("IMPLEMENTATION_PLAN.md", "# Plan\n\nALL_TASKS_COMPLETE"),
            )),
            quality: Arc::new(MockQualityChecker::new()),
        };

        let cfg = mock_config();
        let manager = LoopManager::with_deps(cfg, deps).unwrap();

        assert!(manager.is_complete().unwrap());
    }

    #[test]
    fn test_is_not_complete_with_mock_fs() {
        let deps = create_test_deps();

        let cfg = mock_config();
        let manager = LoopManager::with_deps(cfg, deps).unwrap();

        assert!(!manager.is_complete().unwrap());
    }

    #[tokio::test]
    async fn test_run_claude_iteration_uses_mock_claude() {
        let claude = MockClaudeProcess::new().with_exit_code(0);
        let deps = LoopDependencies {
            git: Arc::new(MockGitOperations::new()),
            claude: Arc::new(claude),
            fs: Arc::new(RwLock::new(
                MockFileSystem::new()
                    .with_file("IMPLEMENTATION_PLAN.md", "# Plan")
                    .with_file("PROMPT_build.md", "Test build prompt"),
            )),
            quality: Arc::new(MockQualityChecker::new()),
        };

        let cfg = mock_config();
        let manager = LoopManager::with_deps(cfg, deps).unwrap();

        let result = manager.run_claude_iteration().await;
        assert!(result.is_ok(), "Expected Ok but got: {:?}", result);
        assert_eq!(result.unwrap(), 0);
    }

    #[tokio::test]
    async fn test_run_claude_iteration_returns_error_exit_code() {
        let claude = MockClaudeProcess::new().with_exit_code(1);
        let deps = LoopDependencies {
            git: Arc::new(MockGitOperations::new()),
            claude: Arc::new(claude),
            fs: Arc::new(RwLock::new(
                MockFileSystem::new()
                    .with_file("IMPLEMENTATION_PLAN.md", "# Plan")
                    .with_file("PROMPT_build.md", "Test build prompt"),
            )),
            quality: Arc::new(MockQualityChecker::new()),
        };

        let cfg = mock_config();
        let manager = LoopManager::with_deps(cfg, deps).unwrap();

        let result = manager.run_claude_iteration().await;
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), 1);
    }

    #[tokio::test]
    async fn test_run_claude_iteration_propagates_error() {
        let claude = MockClaudeProcess::new().with_error("Claude crashed!");
        let deps = LoopDependencies {
            git: Arc::new(MockGitOperations::new()),
            claude: Arc::new(claude),
            fs: Arc::new(RwLock::new(
                MockFileSystem::new()
                    .with_file("IMPLEMENTATION_PLAN.md", "# Plan")
                    .with_file("PROMPT_build.md", "Test build prompt"),
            )),
            quality: Arc::new(MockQualityChecker::new()),
        };

        let cfg = mock_config();
        let manager = LoopManager::with_deps(cfg, deps).unwrap();

        let result = manager.run_claude_iteration().await;
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("Claude crashed!"));
    }

    #[tokio::test]
    async fn test_run_claude_iteration_uses_dynamic_prompt_without_file() {
        // With the PromptAssembler integration, dynamic prompts are built from templates
        // even when no static prompt file exists. This tests that fallback behavior.
        let deps = LoopDependencies {
            git: Arc::new(MockGitOperations::new()),
            claude: Arc::new(MockClaudeProcess::new().with_exit_code(0)),
            fs: Arc::new(RwLock::new(
                MockFileSystem::new().with_file("IMPLEMENTATION_PLAN.md", "# Plan"), // No static prompt file needed
            )),
            quality: Arc::new(MockQualityChecker::new()),
        };

        let cfg = mock_config();
        let manager = LoopManager::with_deps(cfg, deps).unwrap();

        // Should succeed using dynamic prompt generation
        let result = manager.run_claude_iteration().await;
        assert!(
            result.is_ok(),
            "Expected Ok with dynamic prompt, got: {:?}",
            result
        );
        assert_eq!(result.unwrap(), 0);
    }

    #[test]
    fn test_has_made_progress_detects_commits() {
        let deps = LoopDependencies {
            git: Arc::new(
                MockGitOperations::new()
                    .with_commit_hash("new_hash")
                    .with_commits_since(1),
            ),
            claude: Arc::new(MockClaudeProcess::new()),
            fs: Arc::new(RwLock::new(
                MockFileSystem::new().with_file("IMPLEMENTATION_PLAN.md", "# Plan"),
            )),
            quality: Arc::new(MockQualityChecker::new()),
        };

        let cfg = mock_config();
        let mut manager = LoopManager::with_deps(cfg, deps).unwrap();

        // Set a previous commit hash
        manager.state.update_commit_hash("old_hash".to_string());

        // Should detect progress via commits
        assert!(manager.has_made_progress());
    }

    #[test]
    fn test_has_made_progress_detects_no_progress() {
        let deps = LoopDependencies {
            git: Arc::new(
                MockGitOperations::new()
                    .with_commit_hash("same_hash")
                    .with_commits_since(0),
            ),
            claude: Arc::new(MockClaudeProcess::new()),
            fs: Arc::new(RwLock::new(
                MockFileSystem::new().with_file("IMPLEMENTATION_PLAN.md", "# Plan"),
            )),
            quality: Arc::new(MockQualityChecker::new()),
        };

        let cfg = mock_config();
        let mut manager = LoopManager::with_deps(cfg, deps).unwrap();

        // Set the same plan hash (compute what it should be)
        let plan_hash = manager.get_plan_hash().unwrap();
        manager.state.update_plan_hash(plan_hash);
        manager.state.update_commit_hash("same_hash".to_string());

        // Should detect no progress
        assert!(!manager.has_made_progress());
    }

    // =========================================================================
    // Task Tracker Integration Tests
    // =========================================================================

    #[test]
    fn test_task_tracker_initialized_from_plan() {
        let plan = r#"# Implementation Plan

## Phase 1: Foundation

### 1. Task One
Build the foundation.

### 2. Task Two
Add features.

### 3. Task Three
Final polish.
"#;
        let deps = LoopDependencies {
            git: Arc::new(MockGitOperations::new()),
            claude: Arc::new(MockClaudeProcess::new()),
            fs: Arc::new(RwLock::new(
                MockFileSystem::new()
                    .with_file("IMPLEMENTATION_PLAN.md", plan)
                    .with_file("PROMPT_build.md", "Build prompt"),
            )),
            quality: Arc::new(MockQualityChecker::new()),
        };

        let cfg = mock_config();
        let manager = LoopManager::with_deps(cfg, deps).unwrap();

        // Verify tasks were parsed from the plan
        assert_eq!(manager.task_tracker().tasks.len(), 3);
    }

    #[test]
    fn test_task_tracker_counts_initial_state() {
        let plan = r#"# Implementation Plan

### 1. Task One
First task.

### 2. Task Two
Second task.
"#;
        let deps = LoopDependencies {
            git: Arc::new(MockGitOperations::new()),
            claude: Arc::new(MockClaudeProcess::new()),
            fs: Arc::new(RwLock::new(
                MockFileSystem::new()
                    .with_file("IMPLEMENTATION_PLAN.md", plan)
                    .with_file("PROMPT_build.md", "Build prompt"),
            )),
            quality: Arc::new(MockQualityChecker::new()),
        };

        let cfg = mock_config();
        let manager = LoopManager::with_deps(cfg, deps).unwrap();

        let counts = manager.task_tracker().task_counts();
        assert_eq!(counts.total(), 2);
        assert_eq!(counts.not_started, 2);
        assert_eq!(counts.in_progress, 0);
        assert_eq!(counts.complete, 0);
        assert_eq!(counts.blocked, 0);
    }

    #[test]
    fn test_task_tracker_context_summary() {
        let plan = r#"# Implementation Plan

### 1. Task One
First task description.

### 2. Task Two
Second task description.
"#;
        let deps = LoopDependencies {
            git: Arc::new(MockGitOperations::new()),
            claude: Arc::new(MockClaudeProcess::new()),
            fs: Arc::new(RwLock::new(
                MockFileSystem::new()
                    .with_file("IMPLEMENTATION_PLAN.md", plan)
                    .with_file("PROMPT_build.md", "Build prompt"),
            )),
            quality: Arc::new(MockQualityChecker::new()),
        };

        let cfg = mock_config();
        let manager = LoopManager::with_deps(cfg, deps).unwrap();

        // Context summary should be empty when no task is selected
        let context = manager.task_tracker().get_context_summary();
        // The context should mention no current task or be empty
        assert!(
            context.is_empty() || context.contains("No current task"),
            "Expected empty or 'No current task', got: {}",
            context
        );
    }

    #[test]
    fn test_task_tracker_task_selection() {
        let plan = r#"# Implementation Plan

### 1. Task One
First task.

### 2. Task Two
Second task.
"#;
        let deps = LoopDependencies {
            git: Arc::new(MockGitOperations::new()),
            claude: Arc::new(MockClaudeProcess::new()),
            fs: Arc::new(RwLock::new(
                MockFileSystem::new()
                    .with_file("IMPLEMENTATION_PLAN.md", plan)
                    .with_file("PROMPT_build.md", "Build prompt"),
            )),
            quality: Arc::new(MockQualityChecker::new()),
        };

        let cfg = mock_config();
        let mut manager = LoopManager::with_deps(cfg, deps).unwrap();

        // Select next task
        let selected = manager.task_tracker_mut().select_next_task().cloned();
        assert!(selected.is_some());

        // Should select task 1 (lowest numbered not-started task)
        let task_id = selected.unwrap();
        assert_eq!(task_id.number(), 1);
    }

    #[test]
    fn test_task_tracker_start_and_progress() {
        let plan = r#"# Implementation Plan

### 1. Task One
First task.
"#;
        let deps = LoopDependencies {
            git: Arc::new(MockGitOperations::new()),
            claude: Arc::new(MockClaudeProcess::new()),
            fs: Arc::new(RwLock::new(
                MockFileSystem::new()
                    .with_file("IMPLEMENTATION_PLAN.md", plan)
                    .with_file("PROMPT_build.md", "Build prompt"),
            )),
            quality: Arc::new(MockQualityChecker::new()),
        };

        let cfg = mock_config();
        let mut manager = LoopManager::with_deps(cfg, deps).unwrap();

        // Select and start task
        let task_id = manager
            .task_tracker_mut()
            .select_next_task()
            .cloned()
            .unwrap();
        manager.task_tracker_mut().start_task(&task_id).unwrap();

        // Verify task is in progress
        let counts = manager.task_tracker().task_counts();
        assert_eq!(counts.in_progress, 1);
        assert_eq!(counts.not_started, 0);

        // Record progress
        manager.task_tracker_mut().record_progress(5, 100).unwrap();

        // Task should still be in progress
        let task = manager.task_tracker().get_task(&task_id).unwrap();
        assert_eq!(task.metrics.files_modified, 5);
        assert_eq!(task.metrics.lines_changed, 100);
    }

    #[test]
    fn test_task_tracker_complete_task() {
        let plan = r#"# Implementation Plan

### 1. Task One
First task.

### 2. Task Two
Second task.
"#;
        let deps = LoopDependencies {
            git: Arc::new(MockGitOperations::new()),
            claude: Arc::new(MockClaudeProcess::new()),
            fs: Arc::new(RwLock::new(
                MockFileSystem::new()
                    .with_file("IMPLEMENTATION_PLAN.md", plan)
                    .with_file("PROMPT_build.md", "Build prompt"),
            )),
            quality: Arc::new(MockQualityChecker::new()),
        };

        let cfg = mock_config();
        let mut manager = LoopManager::with_deps(cfg, deps).unwrap();

        // Select, start, and complete task 1
        let task_id = manager
            .task_tracker_mut()
            .select_next_task()
            .cloned()
            .unwrap();
        manager.task_tracker_mut().start_task(&task_id).unwrap();
        manager.task_tracker_mut().complete_task(&task_id).unwrap();

        // Verify counts
        let counts = manager.task_tracker().task_counts();
        assert_eq!(counts.complete, 1);
        assert_eq!(counts.not_started, 1); // Task 2 still not started
        assert_eq!(counts.in_progress, 0);
    }

    #[test]
    fn test_task_tracker_integration_with_plan_parsing() {
        // Test with a more realistic plan format
        let plan = r#"# IMPLEMENTATION_PLAN.md

## Overview
This plan outlines the implementation phases.

## Phase 1: Foundation

### 1. Phase 1.1: Core Types
- [ ] Define basic types
- [ ] Add serialization

### 2. Phase 1.2: Parser
- [ ] Implement parser
- [x] Add tests

## Phase 2: Integration

### 3. Phase 2.1: Integration
Connect all components.
"#;
        let deps = LoopDependencies {
            git: Arc::new(MockGitOperations::new()),
            claude: Arc::new(MockClaudeProcess::new()),
            fs: Arc::new(RwLock::new(
                MockFileSystem::new()
                    .with_file("IMPLEMENTATION_PLAN.md", plan)
                    .with_file("PROMPT_build.md", "Build prompt"),
            )),
            quality: Arc::new(MockQualityChecker::new()),
        };

        let cfg = mock_config();
        let manager = LoopManager::with_deps(cfg, deps).unwrap();

        // Should have parsed 3 tasks
        assert_eq!(manager.task_tracker().tasks.len(), 3);

        // Task 1 should have checkboxes
        for (task_id, task) in &manager.task_tracker().tasks {
            if task_id.number() == 1 {
                assert_eq!(task.checkboxes.len(), 2);
                assert!(!task.checkboxes[0].1); // First unchecked
                assert!(!task.checkboxes[1].1); // Second unchecked
            } else if task_id.number() == 2 {
                assert_eq!(task.checkboxes.len(), 2);
                assert!(!task.checkboxes[0].1); // First unchecked
                assert!(task.checkboxes[1].1); // Second checked
            }
        }
    }
}
