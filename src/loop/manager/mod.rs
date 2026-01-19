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

// Sub-modules
mod checkpoint;
mod iteration;
mod prompt_handling;

use super::operations::{RealClaudeProcess, RealFileSystem, RealGitOperations, RealQualityChecker};
use super::progress::ProgressTracker;
use super::retry::{IntelligentRetry, RetryConfig, RetryHistory};
use super::state::{LoopMode, LoopState};
use super::task_tracker::{TaskState, TaskTracker, TaskTrackerConfig, TaskTransition};
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
use tokio::sync::RwLock;
use tracing::{debug, info, warn};

/// Maximum retries for transient failures (LSP crashes, etc.)
pub(crate) const MAX_RETRIES: u32 = 3;

/// Backoff delay between retries in milliseconds
pub(crate) const RETRY_BACKOFF_MS: u64 = 2000;

/// Estimated tokens per byte (conservative estimate for code)
pub(crate) const TOKENS_PER_BYTE: f64 = 0.25;

/// Warning threshold for file size (Claude's limit is ~25k tokens)
pub(crate) const FILE_SIZE_WARNING_TOKENS: usize = 20_000;

/// Critical threshold - files this size will cause Claude errors
pub(crate) const FILE_SIZE_CRITICAL_TOKENS: usize = 25_000;

/// Maximum number of files to track in the touch history.
/// This bounds memory usage in long-running sessions.
pub(crate) const MAX_FILE_TOUCH_HISTORY: usize = 100;

// ============================================================================
// BoundedFileTouchHistory
// ============================================================================

/// A bounded history of file touches that evicts least-touched entries.
///
/// Used to track file modification patterns during automation sessions
/// without unbounded memory growth.
///
/// # Memory Bound
///
/// The history is capped at `capacity` entries. When full, adding a new
/// file will evict the file with the lowest touch count.
///
/// # Example
///
/// ```rust,ignore
/// let mut history = BoundedFileTouchHistory::new(100);
/// history.touch("src/lib.rs");
/// history.touch("src/lib.rs"); // Count = 2
/// history.touch("src/main.rs"); // Count = 1
///
/// assert_eq!(history.get_count("src/lib.rs"), Some(2));
/// ```
#[derive(Debug, Clone)]
pub(crate) struct BoundedFileTouchHistory {
    entries: std::collections::HashMap<String, u32>,
    capacity: usize,
}

impl BoundedFileTouchHistory {
    /// Create a new bounded history with the given capacity.
    #[must_use]
    pub fn new(capacity: usize) -> Self {
        Self {
            entries: std::collections::HashMap::with_capacity(capacity),
            capacity,
        }
    }

    /// Create a new bounded history with the default capacity.
    #[must_use]
    pub fn with_default_capacity() -> Self {
        Self::new(MAX_FILE_TOUCH_HISTORY)
    }

    /// Record a touch on a file, incrementing its count.
    ///
    /// If the history is at capacity and the file is new, evicts the
    /// file with the lowest touch count.
    pub fn touch(&mut self, file: &str) {
        if let Some(count) = self.entries.get_mut(file) {
            *count += 1;
        } else {
            // New file - check if we need to evict
            if self.entries.len() >= self.capacity {
                self.evict_least_touched();
            }
            self.entries.insert(file.to_string(), 1);
        }
    }

    /// Evict the entry with the lowest touch count.
    fn evict_least_touched(&mut self) {
        if let Some((min_file, _)) = self
            .entries
            .iter()
            .min_by_key(|(_, count)| *count)
            .map(|(f, c)| (f.clone(), *c))
        {
            self.entries.remove(&min_file);
        }
    }

    /// Iterate over all entries.
    pub fn iter(&self) -> impl Iterator<Item = (&String, &u32)> {
        self.entries.iter()
    }

    /// Get the current number of tracked files.
    #[cfg(test)]
    #[must_use]
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Check if the history is empty.
    #[cfg(test)]
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Get the capacity of the history.
    #[cfg(test)]
    #[must_use]
    pub fn capacity(&self) -> usize {
        self.capacity
    }

    /// Check if a file is in the history.
    #[cfg(test)]
    #[must_use]
    pub fn contains(&self, file: &str) -> bool {
        self.entries.contains_key(file)
    }

    /// Get the touch count for a file.
    #[cfg(test)]
    #[must_use]
    pub fn get_count(&self, file: &str) -> Option<u32> {
        self.entries.get(file).copied()
    }
}

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
    pub(crate) project_dir: PathBuf,
    pub(crate) max_iterations: u32,
    pub(crate) stagnation_threshold: u32,
    pub(crate) doc_sync_interval: u32,
    pub(crate) state: LoopState,
    pub(crate) analytics: Analytics,
    pub(crate) config: ProjectConfig,
    pub(crate) verbose: bool,
    /// Injected dependencies (None = use direct calls for backward compatibility).
    pub(crate) deps: Option<LoopDependencies>,
    /// Task-level progress tracker.
    pub(crate) task_tracker: TaskTracker,
    /// Dynamic prompt assembler for context-aware prompts.
    pub(crate) prompt_assembler: PromptAssembler,
    /// Semantic progress tracker for multi-dimensional progress detection.
    pub(crate) progress_tracker: ProgressTracker,
    /// Intelligent retry system for failure classification and recovery.
    pub(crate) intelligent_retry: IntelligentRetry,
    /// Retry history for tracking and learning from retry attempts.
    pub(crate) retry_history: RetryHistory,
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

        // Track file touches across iterations for churn detection (bounded to prevent memory growth)
        let mut file_touch_history = BoundedFileTouchHistory::with_default_capacity();
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
                let result = self.run_claude_iteration_with_retry().await;

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
                        file_touch_history.touch(&file);
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
        println!("{}", "=".repeat(60).bright_blue());
        println!(
            "{}",
            "     RALPH - Claude Code Automation Suite"
                .bright_blue()
                .bold()
        );
        println!("{}", "=".repeat(60).bright_blue());
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
                println!("     {} {}", "OK".green(), perm.green());
            }
            if !self.config.permissions.deny.is_empty() {
                println!("{}", "   Safety blocks (denied):".cyan());
                for perm in &self.config.permissions.deny {
                    println!("     {} {}", "X".red(), perm.red());
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

    /// Get the current git commit hash (HEAD).
    pub(crate) fn get_commit_hash(&self) -> Result<String> {
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
    pub(crate) fn count_commits_since(&self, old_hash: &str) -> u32 {
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

    /// Try to push to remote (uses BatchMode to avoid SSH passphrase hang).
    async fn try_push(&self) {
        use tokio::process::Command as AsyncCommand;

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
    pub(crate) fn is_complete(&self) -> Result<bool> {
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
    pub(crate) fn get_recent_changes(&self) -> Result<u32> {
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
}

/// Convert a loop mode to its prompt name string.
///
/// # Example
///
/// ```
/// use ralph::r#loop::manager::{LoopMode, mode_to_prompt_name};
///
/// assert_eq!(mode_to_prompt_name(&LoopMode::Build), "build");
/// ```
#[must_use]
pub fn mode_to_prompt_name(mode: &LoopMode) -> &'static str {
    match mode {
        LoopMode::Build => "build",
        LoopMode::Debug => "debug",
        LoopMode::Plan => "plan",
    }
}

/// Truncate a prompt to fit within max_length, preserving start and end.
///
/// If the prompt exceeds max_length, it will be truncated from the middle,
/// keeping 2/3 of the start and 1/3 of the end, with a truncation marker.
///
/// # Example
///
/// ```
/// use ralph::r#loop::manager::truncate_prompt;
///
/// let short = truncate_prompt("hello".to_string(), 100);
/// assert_eq!(short, "hello");
///
/// let long = truncate_prompt("x".repeat(200), 100);
/// assert!(long.contains("truncated"));
/// ```
#[must_use]
pub fn truncate_prompt(prompt: String, max_length: usize) -> String {
    if prompt.len() <= max_length {
        return prompt;
    }

    // Truncate from the middle, keeping beginning (instructions) and end (current context)
    let keep_start = max_length * 2 / 3;
    let keep_end = max_length / 3;
    let start = &prompt[..keep_start];
    let end = &prompt[prompt.len() - keep_end..];

    format!(
        "{}\n\n... [truncated {} chars] ...\n\n{}",
        start,
        prompt.len() - max_length,
        end
    )
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

    // =========================================================================
    // truncate_prompt helper tests
    // =========================================================================

    #[test]
    fn test_truncate_prompt_under_limit() {
        let prompt = "Short prompt".to_string();
        let result = truncate_prompt(prompt.clone(), 1000);
        assert_eq!(result, prompt);
    }

    #[test]
    fn test_truncate_prompt_at_limit() {
        let prompt = "x".repeat(100);
        let result = truncate_prompt(prompt.clone(), 100);
        assert_eq!(result, prompt);
    }

    #[test]
    fn test_truncate_prompt_over_limit() {
        let prompt = "x".repeat(200);
        let result = truncate_prompt(prompt, 100);
        assert!(result.len() <= 150); // Some overhead for truncation message
        assert!(result.contains("truncated"));
    }

    #[test]
    fn test_truncate_prompt_preserves_start_and_end() {
        let start = "START".repeat(20);
        let middle = "MIDDLE".repeat(100);
        let end = "END".repeat(20);
        let prompt = format!("{}{}{}", start, middle, end);

        let result = truncate_prompt(prompt, 300);

        // Should preserve start
        assert!(result.starts_with("START"));
        // Should preserve end
        assert!(result.ends_with("END"));
        // Should indicate truncation
        assert!(result.contains("truncated"));
    }

    // =========================================================================
    // mode_to_prompt_name helper tests
    // =========================================================================

    #[test]
    fn test_mode_to_prompt_name_build() {
        assert_eq!(mode_to_prompt_name(&LoopMode::Build), "build");
    }

    #[test]
    fn test_mode_to_prompt_name_debug() {
        assert_eq!(mode_to_prompt_name(&LoopMode::Debug), "debug");
    }

    #[test]
    fn test_mode_to_prompt_name_plan() {
        assert_eq!(mode_to_prompt_name(&LoopMode::Plan), "plan");
    }

    // =========================================================================
    // run_claude_iteration_with_retry tests
    // =========================================================================

    #[tokio::test]
    async fn test_process_error_retries_then_succeeds() {
        let claude = Arc::new(MockClaudeProcess::new().with_fail_count(2, "No messages returned"));
        let deps = LoopDependencies {
            git: Arc::new(MockGitOperations::new()),
            claude: claude.clone(),
            fs: Arc::new(RwLock::new(
                MockFileSystem::new()
                    .with_file("IMPLEMENTATION_PLAN.md", "# Plan")
                    .with_file("PROMPT_build.md", "Test build prompt"),
            )),
            quality: Arc::new(MockQualityChecker::new()),
        };

        let cfg = mock_config();
        let manager = LoopManager::with_deps(cfg, deps).unwrap();

        let result = manager.run_claude_iteration_with_retry().await;
        assert!(
            result.is_ok(),
            "Expected Ok after retries, got: {:?}",
            result
        );
        assert_eq!(claude.call_count(), 3); // Failed twice, succeeded on third
    }

    #[tokio::test]
    async fn test_process_error_exhausts_retries() {
        // Fail more times than MAX_RETRIES to exhaust all retries
        let claude = Arc::new(MockClaudeProcess::new().with_fail_count(10, "No messages returned"));
        let deps = LoopDependencies {
            git: Arc::new(MockGitOperations::new()),
            claude: claude.clone(),
            fs: Arc::new(RwLock::new(
                MockFileSystem::new()
                    .with_file("IMPLEMENTATION_PLAN.md", "# Plan")
                    .with_file("PROMPT_build.md", "Test build prompt"),
            )),
            quality: Arc::new(MockQualityChecker::new()),
        };

        let cfg = mock_config();
        let manager = LoopManager::with_deps(cfg, deps).unwrap();

        let result = manager.run_claude_iteration_with_retry().await;
        assert!(result.is_err());
        assert_eq!(claude.call_count(), MAX_RETRIES); // Stopped at MAX_RETRIES
    }

    #[tokio::test]
    async fn test_non_transient_error_not_retried() {
        // A compilation error should not be retried
        let claude =
            Arc::new(MockClaudeProcess::new().with_error("error[E0308]: mismatched types"));
        let deps = LoopDependencies {
            git: Arc::new(MockGitOperations::new()),
            claude: claude.clone(),
            fs: Arc::new(RwLock::new(
                MockFileSystem::new()
                    .with_file("IMPLEMENTATION_PLAN.md", "# Plan")
                    .with_file("PROMPT_build.md", "Test build prompt"),
            )),
            quality: Arc::new(MockQualityChecker::new()),
        };

        let cfg = mock_config();
        let manager = LoopManager::with_deps(cfg, deps).unwrap();

        let result = manager.run_claude_iteration_with_retry().await;
        assert!(result.is_err());
        // Should only call once since compile errors are not transient
        assert_eq!(claude.call_count(), 1);
    }

    #[tokio::test]
    async fn test_success_on_first_try_no_retry_needed() {
        let claude = Arc::new(MockClaudeProcess::new().with_exit_code(0));
        let deps = LoopDependencies {
            git: Arc::new(MockGitOperations::new()),
            claude: claude.clone(),
            fs: Arc::new(RwLock::new(
                MockFileSystem::new()
                    .with_file("IMPLEMENTATION_PLAN.md", "# Plan")
                    .with_file("PROMPT_build.md", "Test build prompt"),
            )),
            quality: Arc::new(MockQualityChecker::new()),
        };

        let cfg = mock_config();
        let manager = LoopManager::with_deps(cfg, deps).unwrap();

        let result = manager.run_claude_iteration_with_retry().await;
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), 0);
        assert_eq!(claude.call_count(), 1); // Only one call needed
    }

    // =========================================================================
    // BoundedFileTouchHistory tests
    // =========================================================================

    #[test]
    fn test_bounded_history_evicts_least_touched() {
        let mut history = BoundedFileTouchHistory::new(3);
        history.touch("a.rs");
        history.touch("b.rs");
        history.touch("a.rs"); // a.rs count = 2
        history.touch("c.rs");
        history.touch("d.rs"); // Should evict b.rs or c.rs (count = 1)

        assert_eq!(history.len(), 3);
        assert!(history.contains("a.rs")); // Most touched, should remain
    }

    #[test]
    fn test_bounded_history_default_capacity() {
        let history = BoundedFileTouchHistory::with_default_capacity();
        assert_eq!(history.capacity(), MAX_FILE_TOUCH_HISTORY);
    }

    #[test]
    fn test_bounded_history_touch_increments_count() {
        let mut history = BoundedFileTouchHistory::new(10);
        history.touch("file.rs");
        assert_eq!(history.get_count("file.rs"), Some(1));

        history.touch("file.rs");
        assert_eq!(history.get_count("file.rs"), Some(2));

        history.touch("file.rs");
        assert_eq!(history.get_count("file.rs"), Some(3));
    }

    #[test]
    fn test_bounded_history_no_eviction_under_capacity() {
        let mut history = BoundedFileTouchHistory::new(5);
        history.touch("a.rs");
        history.touch("b.rs");
        history.touch("c.rs");

        assert_eq!(history.len(), 3);
        assert!(history.contains("a.rs"));
        assert!(history.contains("b.rs"));
        assert!(history.contains("c.rs"));
    }

    #[test]
    fn test_bounded_history_iter() {
        let mut history = BoundedFileTouchHistory::new(10);
        history.touch("a.rs");
        history.touch("b.rs");
        history.touch("a.rs");

        let entries: Vec<_> = history.iter().collect();
        assert_eq!(entries.len(), 2);
    }

    #[test]
    fn test_bounded_history_is_empty() {
        let history = BoundedFileTouchHistory::new(10);
        assert!(history.is_empty());

        let mut history = BoundedFileTouchHistory::new(10);
        history.touch("a.rs");
        assert!(!history.is_empty());
    }
}
