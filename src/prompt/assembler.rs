//! Prompt Assembler - Coordinates all prompt generation components.
//!
//! This module provides a high-level assembler that combines:
//! - Template loading and management
//! - Context building from various sources
//! - Anti-pattern detection and integration
//! - Final prompt assembly
//!
//! # Example
//!
//! ```no_run
//! use ralph::prompt::assembler::PromptAssembler;
//! use ralph::prompt::context::{CurrentTaskContext, TaskPhase, SessionStats};
//!
//! let mut assembler = PromptAssembler::new();
//!
//! // Update context from loop state
//! assembler.set_current_task("2.1", "Implement feature", TaskPhase::Implementation);
//! assembler.update_session_stats(5, 2, 150);
//!
//! // Record iteration for anti-pattern detection
//! assembler.record_iteration_with_files(1, vec!["src/lib.rs".to_string()], false);
//!
//! // Build the prompt
//! let prompt = assembler.build_prompt("build").expect("should build");
//! ```

use crate::narsil::{CodeIntelligenceBuilder, NarsilClient};
use crate::prompt::antipatterns::{
    detect_quality_gate_ignoring, detect_scope_creep, AntiPatternDetector, DetectorConfig,
    IterationSummary,
};
use crate::prompt::builder::DynamicPromptBuilder;
use crate::prompt::context::{
    AntiPattern, AttemptOutcome, AttemptSummary, CurrentTaskContext, ErrorAggregator, ErrorContext,
    ErrorSeverity, GateResult, PromptContext, QualityGateStatus, SessionStats, TaskPhase,
};
use crate::prompt::templates::PromptTemplates;
use std::path::Path;

/// Configuration for the PromptAssembler.
#[derive(Debug, Clone)]
pub struct AssemblerConfig {
    /// Load templates from files if available (fallback to defaults).
    pub load_templates_from_dir: Option<String>,
    /// Anti-pattern detector configuration.
    pub detector_config: DetectorConfig,
    /// Maximum errors to include in prompt.
    pub max_errors: usize,
    /// Maximum attempt history to include.
    pub max_attempts: usize,
    /// Maximum anti-patterns to include.
    pub max_anti_patterns: usize,
}

impl Default for AssemblerConfig {
    fn default() -> Self {
        Self {
            load_templates_from_dir: None,
            detector_config: DetectorConfig::default(),
            max_errors: 10,
            max_attempts: 5,
            max_anti_patterns: 5,
        }
    }
}

impl AssemblerConfig {
    /// Create a new configuration.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Set the template directory.
    #[must_use]
    pub fn with_template_dir(mut self, dir: impl Into<String>) -> Self {
        self.load_templates_from_dir = Some(dir.into());
        self
    }

    /// Set the detector configuration.
    #[must_use]
    pub fn with_detector_config(mut self, config: DetectorConfig) -> Self {
        self.detector_config = config;
        self
    }

    /// Set the maximum errors to include.
    #[must_use]
    pub fn with_max_errors(mut self, max: usize) -> Self {
        self.max_errors = max;
        self
    }

    /// Set the maximum attempts to include.
    #[must_use]
    pub fn with_max_attempts(mut self, max: usize) -> Self {
        self.max_attempts = max;
        self
    }

    /// Set the maximum anti-patterns to include.
    #[must_use]
    pub fn with_max_anti_patterns(mut self, max: usize) -> Self {
        self.max_anti_patterns = max;
        self
    }
}

/// High-level prompt assembler that coordinates all prompt generation components.
///
/// The assembler manages:
/// - Current context (task, errors, quality status, session stats)
/// - Anti-pattern detection across iterations
/// - Attempt history for the current task
/// - Code intelligence from narsil-mcp
/// - Final prompt assembly from templates
///
/// # Example
///
/// ```no_run
/// use ralph::prompt::assembler::PromptAssembler;
/// use ralph::prompt::context::TaskPhase;
///
/// let mut assembler = PromptAssembler::new();
///
/// // Set up context
/// assembler.set_current_task("1.1", "Build feature", TaskPhase::Implementation);
/// assembler.update_session_stats(3, 1, 50);
///
/// // Build prompt
/// let prompt = assembler.build_prompt("build").expect("build succeeds");
/// assert!(prompt.contains("Build Phase"));
/// ```
pub struct PromptAssembler {
    /// Configuration.
    config: AssemblerConfig,
    /// Template-based prompt builder.
    builder: DynamicPromptBuilder,
    /// Anti-pattern detector.
    detector: AntiPatternDetector,
    /// Error aggregator for deduplication.
    error_aggregator: ErrorAggregator,
    /// Current task context.
    current_task: Option<CurrentTaskContext>,
    /// Session statistics.
    session_stats: SessionStats,
    /// Quality gate status.
    quality_status: QualityGateStatus,
    /// Attempt history for current task.
    attempts: Vec<AttemptSummary>,
    /// Historical guidance strings.
    historical_guidance: Vec<String>,
    /// Consecutive quality gate failures.
    consecutive_quality_failures: u32,
    /// Files modified in current session (for scope creep detection).
    session_files_modified: Vec<String>,
    /// Optional narsil-mcp client for code intelligence.
    narsil_client: Option<NarsilClient>,
    /// Functions to query for call graph intelligence.
    intelligence_functions: Vec<String>,
    /// Symbols to query for reference intelligence.
    intelligence_symbols: Vec<String>,
    /// Files to query for dependency intelligence.
    intelligence_files: Vec<String>,
}

impl PromptAssembler {
    /// Create a new assembler with default configuration.
    ///
    /// # Example
    ///
    /// ```no_run
    /// use ralph::prompt::assembler::PromptAssembler;
    ///
    /// let assembler = PromptAssembler::new();
    /// ```
    #[must_use]
    pub fn new() -> Self {
        Self::with_config(AssemblerConfig::default())
    }

    /// Create an assembler with custom configuration.
    ///
    /// # Example
    ///
    /// ```no_run
    /// use ralph::prompt::assembler::{AssemblerConfig, PromptAssembler};
    ///
    /// let config = AssemblerConfig::new()
    ///     .with_max_errors(5)
    ///     .with_max_attempts(3);
    ///
    /// let assembler = PromptAssembler::with_config(config);
    /// ```
    #[must_use]
    pub fn with_config(config: AssemblerConfig) -> Self {
        let templates = if let Some(ref dir) = config.load_templates_from_dir {
            PromptTemplates::load_or_defaults(dir).unwrap_or_else(|_| PromptTemplates::with_defaults())
        } else {
            PromptTemplates::with_defaults()
        };

        Self {
            detector: AntiPatternDetector::with_config(config.detector_config.clone()),
            config,
            builder: DynamicPromptBuilder::new(templates),
            error_aggregator: ErrorAggregator::new(),
            current_task: None,
            session_stats: SessionStats::new(0, 0, 0),
            quality_status: QualityGateStatus::new(),
            attempts: Vec::new(),
            historical_guidance: Vec::new(),
            consecutive_quality_failures: 0,
            session_files_modified: Vec::new(),
            narsil_client: None,
            intelligence_functions: Vec::new(),
            intelligence_symbols: Vec::new(),
            intelligence_files: Vec::new(),
        }
    }

    /// Create an assembler with a NarsilClient for code intelligence.
    ///
    /// # Example
    ///
    /// ```ignore
    /// use ralph::prompt::assembler::PromptAssembler;
    /// use ralph::narsil::{NarsilClient, NarsilConfig};
    ///
    /// let config = NarsilConfig::new(".");
    /// let client = NarsilClient::new(config).unwrap();
    /// let assembler = PromptAssembler::with_narsil_client(client);
    /// ```
    #[must_use]
    pub fn with_narsil_client(client: NarsilClient) -> Self {
        let mut assembler = Self::new();
        assembler.narsil_client = Some(client);
        assembler
    }

    /// Load templates from a directory.
    ///
    /// # Errors
    ///
    /// Returns an error if the directory cannot be read.
    pub fn load_templates(&mut self, dir: impl AsRef<Path>) -> anyhow::Result<()> {
        let templates = PromptTemplates::load_or_defaults(dir)?;
        self.builder = DynamicPromptBuilder::new(templates);
        Ok(())
    }

    // =========================================================================
    // Task Context Management
    // =========================================================================

    /// Set the current task context.
    ///
    /// This clears any previous attempt history for the old task.
    ///
    /// # Example
    ///
    /// ```no_run
    /// use ralph::prompt::assembler::PromptAssembler;
    /// use ralph::prompt::context::TaskPhase;
    ///
    /// let mut assembler = PromptAssembler::new();
    /// assembler.set_current_task("2.1", "Implement feature", TaskPhase::Implementation);
    /// ```
    pub fn set_current_task(&mut self, task_id: &str, title: &str, phase: TaskPhase) {
        // If switching tasks, clear attempt history
        if let Some(ref current) = self.current_task {
            if current.task_id != task_id {
                self.attempts.clear();
            }
        }

        self.current_task = Some(CurrentTaskContext::new(task_id, title, phase));
    }

    /// Set the current task with full context.
    pub fn set_current_task_full(&mut self, task: CurrentTaskContext) {
        // If switching tasks, clear attempt history
        if let Some(ref current) = self.current_task {
            if current.task_id != task.task_id {
                self.attempts.clear();
            }
        }

        self.current_task = Some(task);
    }

    /// Update the current task's completion percentage.
    pub fn update_task_completion(&mut self, percentage: u8) {
        if let Some(ref mut task) = self.current_task {
            task.completion_percentage = percentage.min(100);
        }
    }

    /// Update the current task's modified files.
    pub fn update_task_files(&mut self, files: Vec<String>) {
        if let Some(ref mut task) = self.current_task {
            task.modified_files = files.clone();
        }
        // Also track for scope creep detection
        for file in files {
            if !self.session_files_modified.contains(&file) {
                self.session_files_modified.push(file);
            }
        }
    }

    /// Add a blocker to the current task.
    pub fn add_task_blocker(&mut self, blocker: String) {
        if let Some(ref mut task) = self.current_task {
            if !task.blockers.contains(&blocker) {
                task.blockers.push(blocker);
            }
        }
    }

    /// Clear the current task.
    pub fn clear_current_task(&mut self) {
        self.current_task = None;
    }

    /// Get a reference to the current task.
    #[must_use]
    pub fn current_task(&self) -> Option<&CurrentTaskContext> {
        self.current_task.as_ref()
    }

    // =========================================================================
    // Session Stats Management
    // =========================================================================

    /// Update session statistics.
    ///
    /// # Example
    ///
    /// ```no_run
    /// use ralph::prompt::assembler::PromptAssembler;
    ///
    /// let mut assembler = PromptAssembler::new();
    /// assembler.update_session_stats(5, 2, 150);
    /// ```
    pub fn update_session_stats(&mut self, iterations: u32, commits: u32, lines_changed: u32) {
        self.session_stats = SessionStats::new(iterations, commits, lines_changed);
    }

    /// Set the maximum iterations budget.
    pub fn set_budget(&mut self, max_iterations: u32) {
        self.session_stats = self.session_stats.clone().with_budget(max_iterations);
    }

    /// Update stagnation count.
    pub fn set_stagnation(&mut self, count: u32) {
        self.session_stats = self.session_stats.clone().with_stagnation(count);
    }

    /// Update tasks completed/blocked counts.
    pub fn update_task_counts(&mut self, completed: u32, blocked: u32) {
        self.session_stats = self
            .session_stats
            .clone()
            .with_tasks_completed(completed)
            .with_tasks_blocked(blocked);
    }

    /// Increment iteration count.
    pub fn increment_iteration(&mut self) {
        self.session_stats.iteration_count += 1;
    }

    /// Increment commit count.
    pub fn increment_commits(&mut self) {
        self.session_stats.commit_count += 1;
    }

    /// Add to lines changed.
    pub fn add_lines_changed(&mut self, lines: u32) {
        self.session_stats.lines_changed += lines;
    }

    /// Get session stats.
    #[must_use]
    pub fn session_stats(&self) -> &SessionStats {
        &self.session_stats
    }

    // =========================================================================
    // Error Management
    // =========================================================================

    /// Add an error to the aggregator.
    ///
    /// Errors are deduplicated and frequency is tracked.
    ///
    /// # Example
    ///
    /// ```
    /// use ralph::prompt::assembler::PromptAssembler;
    /// use ralph::prompt::context::ErrorSeverity;
    ///
    /// let mut assembler = PromptAssembler::new();
    /// assembler.add_error("E0308", "mismatched types", ErrorSeverity::Error);
    /// assembler.add_error("E0308", "mismatched types", ErrorSeverity::Error); // Deduplicated
    /// ```
    pub fn add_error(&mut self, code: &str, message: &str, severity: ErrorSeverity) {
        self.error_aggregator
            .add(ErrorContext::new(code, message, severity));
    }

    /// Add an error with full context.
    pub fn add_error_context(&mut self, error: ErrorContext) {
        self.error_aggregator.add(error);
    }

    /// Clear all errors.
    pub fn clear_errors(&mut self) {
        self.error_aggregator = ErrorAggregator::new();
    }

    /// Get the number of unique errors.
    #[must_use]
    pub fn error_count(&self) -> usize {
        self.error_aggregator.unique_count()
    }

    // =========================================================================
    // Quality Gate Management
    // =========================================================================

    /// Update quality gate status for clippy.
    pub fn update_clippy_status(&mut self, passed: bool, messages: Vec<String>) {
        let result = if passed {
            GateResult::pass()
        } else {
            GateResult::fail(messages)
        };
        self.quality_status = self.quality_status.clone().with_clippy(result).with_timestamp();
        self.update_quality_failure_streak(passed);
    }

    /// Update quality gate status for tests.
    pub fn update_test_status(&mut self, passed: bool, messages: Vec<String>) {
        let result = if passed {
            GateResult::pass()
        } else {
            GateResult::fail(messages)
        };
        self.quality_status = self.quality_status.clone().with_tests(result).with_timestamp();
        self.update_quality_failure_streak(passed);
    }

    /// Update quality gate status for security scan.
    pub fn update_security_status(&mut self, passed: bool, messages: Vec<String>) {
        let result = if passed {
            GateResult::pass()
        } else {
            GateResult::fail(messages)
        };
        self.quality_status = self.quality_status.clone().with_security(result).with_timestamp();
        self.update_quality_failure_streak(passed);
    }

    /// Set all quality gates as passing.
    pub fn set_all_quality_passing(&mut self) {
        self.quality_status = QualityGateStatus::all_passing();
        self.consecutive_quality_failures = 0;
    }

    /// Get quality gate status.
    #[must_use]
    pub fn quality_status(&self) -> &QualityGateStatus {
        &self.quality_status
    }

    fn update_quality_failure_streak(&mut self, passed: bool) {
        if passed {
            self.consecutive_quality_failures = 0;
        } else {
            self.consecutive_quality_failures += 1;
        }
    }

    // =========================================================================
    // Attempt History Management
    // =========================================================================

    /// Record a new attempt for the current task.
    ///
    /// # Example
    ///
    /// ```no_run
    /// use ralph::prompt::assembler::PromptAssembler;
    /// use ralph::prompt::context::{AttemptOutcome, TaskPhase};
    ///
    /// let mut assembler = PromptAssembler::new();
    /// assembler.set_current_task("1.1", "Test", TaskPhase::Testing);
    /// assembler.record_attempt(AttemptOutcome::TestFailure, Some("TDD approach"), vec!["test_foo failed".to_string()]);
    /// ```
    pub fn record_attempt(
        &mut self,
        outcome: AttemptOutcome,
        approach: Option<&str>,
        errors: Vec<String>,
    ) {
        let attempt_number = self.attempts.len() as u32 + 1;
        let mut summary = AttemptSummary::new(attempt_number, outcome);

        if let Some(approach_str) = approach {
            summary = summary.with_approach(approach_str);
        }

        for error in errors {
            summary = summary.with_error(&error);
        }

        // Include modified files from current task
        if let Some(ref task) = self.current_task {
            summary = summary.with_files(task.modified_files.clone());
        }

        self.attempts.push(summary);

        // Update task attempt count
        if let Some(ref mut task) = self.current_task {
            task.attempt_count = self.attempts.len() as u32;
        }
    }

    /// Clear attempt history.
    pub fn clear_attempts(&mut self) {
        self.attempts.clear();
        if let Some(ref mut task) = self.current_task {
            task.attempt_count = 0;
        }
    }

    /// Get the number of attempts.
    #[must_use]
    pub fn attempt_count(&self) -> usize {
        self.attempts.len()
    }

    // =========================================================================
    // Anti-Pattern Detection
    // =========================================================================

    /// Record an iteration summary for anti-pattern detection.
    ///
    /// # Example
    ///
    /// ```no_run
    /// use ralph::prompt::assembler::PromptAssembler;
    ///
    /// let mut assembler = PromptAssembler::new();
    /// assembler.record_iteration_with_files(1, vec!["src/lib.rs".to_string()], false);
    /// ```
    pub fn record_iteration_with_files(
        &mut self,
        iteration: u32,
        files_modified: Vec<String>,
        committed: bool,
    ) {
        let mut summary = IterationSummary::new(iteration).with_files_modified(files_modified);

        if committed {
            summary = summary.with_commit();
        }

        if let Some(ref task) = self.current_task {
            summary = summary.with_task(&task.task_id);
        }

        self.detector.add_iteration(summary);
    }

    /// Record a full iteration summary.
    pub fn record_iteration(&mut self, summary: IterationSummary) {
        self.detector.add_iteration(summary);
    }

    /// Clear anti-pattern detection history.
    pub fn clear_detector(&mut self) {
        self.detector.clear();
    }

    /// Get the number of iterations recorded.
    #[must_use]
    pub fn iteration_count(&self) -> usize {
        self.detector.iteration_count()
    }

    // =========================================================================
    // Historical Guidance
    // =========================================================================

    /// Add historical guidance.
    pub fn add_guidance(&mut self, guidance: String) {
        if !self.historical_guidance.contains(&guidance) {
            self.historical_guidance.push(guidance);
        }
    }

    /// Set historical guidance.
    pub fn set_guidance(&mut self, guidance: Vec<String>) {
        self.historical_guidance = guidance;
    }

    /// Clear historical guidance.
    pub fn clear_guidance(&mut self) {
        self.historical_guidance.clear();
    }

    // =========================================================================
    // Code Intelligence Management
    // =========================================================================

    /// Check if a NarsilClient is configured.
    #[must_use]
    pub fn has_narsil_client(&self) -> bool {
        self.narsil_client.is_some()
    }

    /// Add a function to query for call graph intelligence.
    ///
    /// When building the prompt, this function's callers and callees
    /// will be included in the code intelligence context.
    ///
    /// # Example
    ///
    /// ```
    /// use ralph::prompt::assembler::PromptAssembler;
    ///
    /// let mut assembler = PromptAssembler::new();
    /// assembler.add_intelligence_function("process_request");
    /// ```
    pub fn add_intelligence_function(&mut self, function: impl Into<String>) {
        let func = function.into();
        if !self.intelligence_functions.contains(&func) {
            self.intelligence_functions.push(func);
        }
    }

    /// Add a symbol to query for reference intelligence.
    ///
    /// When building the prompt, all references to this symbol
    /// will be included in the code intelligence context.
    ///
    /// # Example
    ///
    /// ```no_run
    /// use ralph::prompt::assembler::PromptAssembler;
    ///
    /// let mut assembler = PromptAssembler::new();
    /// assembler.add_intelligence_symbol("MyStruct");
    /// ```
    pub fn add_intelligence_symbol(&mut self, symbol: impl Into<String>) {
        let sym = symbol.into();
        if !self.intelligence_symbols.contains(&sym) {
            self.intelligence_symbols.push(sym);
        }
    }

    /// Add a file to query for dependency intelligence.
    ///
    /// When building the prompt, this file's imports and importers
    /// will be included in the code intelligence context.
    ///
    /// # Example
    ///
    /// ```
    /// use ralph::prompt::assembler::PromptAssembler;
    ///
    /// let mut assembler = PromptAssembler::new();
    /// assembler.add_intelligence_file("src/lib.rs");
    /// ```
    pub fn add_intelligence_file(&mut self, file: impl Into<String>) {
        let f = file.into();
        if !self.intelligence_files.contains(&f) {
            self.intelligence_files.push(f);
        }
    }

    /// Get the list of functions to query for intelligence.
    #[must_use]
    pub fn intelligence_functions(&self) -> &[String] {
        &self.intelligence_functions
    }

    /// Get the list of symbols to query for intelligence.
    #[must_use]
    pub fn intelligence_symbols(&self) -> &[String] {
        &self.intelligence_symbols
    }

    /// Get the list of files to query for intelligence.
    #[must_use]
    pub fn intelligence_files(&self) -> &[String] {
        &self.intelligence_files
    }

    /// Clear all intelligence queries.
    pub fn clear_intelligence_queries(&mut self) {
        self.intelligence_functions.clear();
        self.intelligence_symbols.clear();
        self.intelligence_files.clear();
    }

    /// Build code intelligence context using the configured NarsilClient.
    fn build_code_intelligence(&self) -> crate::prompt::context::CodeIntelligenceContext {
        let Some(ref client) = self.narsil_client else {
            return crate::prompt::context::CodeIntelligenceContext::new();
        };

        // Convert owned Strings to &str slices for the builder API
        let functions: Vec<&str> = self.intelligence_functions.iter().map(|s| s.as_str()).collect();
        let symbols: Vec<&str> = self.intelligence_symbols.iter().map(|s| s.as_str()).collect();
        let files: Vec<&str> = self.intelligence_files.iter().map(|s| s.as_str()).collect();

        let builder = CodeIntelligenceBuilder::new(client)
            .for_functions(&functions)
            .for_symbols(&symbols)
            .for_files(&files);

        // Build returns a Result, unwrap with default on error
        builder.build().unwrap_or_else(|_| crate::prompt::context::CodeIntelligenceContext::new())
    }

    // =========================================================================
    // Prompt Building
    // =========================================================================

    /// Build a prompt for the given mode.
    ///
    /// Assembles all context, detects anti-patterns, and generates the final prompt.
    ///
    /// # Errors
    ///
    /// Returns an error if the template for the given mode doesn't exist.
    ///
    /// # Example
    ///
    /// ```no_run
    /// use ralph::prompt::assembler::PromptAssembler;
    ///
    /// let assembler = PromptAssembler::new();
    /// let prompt = assembler.build_prompt("build").expect("build succeeds");
    /// assert!(prompt.contains("Build Phase"));
    /// ```
    pub fn build_prompt(&self, mode: &str) -> anyhow::Result<String> {
        let context = self.build_context();
        self.builder.build(mode, &context)
    }

    /// Build a prompt with a custom context override.
    ///
    /// Useful when you want to supply additional context beyond what the assembler tracks.
    pub fn build_prompt_with_context(
        &self,
        mode: &str,
        context: &PromptContext,
    ) -> anyhow::Result<String> {
        self.builder.build(mode, context)
    }

    /// Build the PromptContext from current state.
    ///
    /// This is exposed for testing and advanced use cases.
    #[must_use]
    pub fn build_context(&self) -> PromptContext {
        let mut context = PromptContext::new();

        // Add current task
        if let Some(ref task) = self.current_task {
            context = context.with_current_task(task.clone());
        }

        // Add errors (sorted by frequency) - clone aggregator to preserve state
        let errors = self.error_aggregator.clone().sorted_by_frequency();
        for error in errors.into_iter().take(self.config.max_errors) {
            context = context.with_error(error);
        }

        // Add quality status
        context = context.with_quality_status(self.quality_status.clone());

        // Add session stats
        context = context.with_session_stats(self.session_stats.clone());

        // Add attempt history (most recent first for limiting)
        let attempts: Vec<_> = self
            .attempts
            .iter()
            .rev()
            .take(self.config.max_attempts)
            .rev()
            .cloned()
            .collect();
        context = context.with_attempts(attempts);

        // Add anti-patterns from snapshot detection (doesn't modify detector state)
        let mut patterns = self.detect_anti_patterns_snapshot();
        patterns.truncate(self.config.max_anti_patterns);
        context = context.with_anti_patterns(patterns);

        // Add code intelligence from narsil-mcp
        let intelligence = self.build_code_intelligence();
        context = context.with_code_intelligence(intelligence);

        context
    }

    /// Detect anti-patterns from current state (snapshot, doesn't modify detector).
    fn detect_anti_patterns_snapshot(&self) -> Vec<AntiPattern> {
        let mut patterns = Vec::new();

        // Quality gate ignoring
        if let Some(pattern) =
            detect_quality_gate_ignoring(&self.quality_status, self.consecutive_quality_failures)
        {
            patterns.push(pattern);
        }

        // Scope creep
        if let Some(pattern) = detect_scope_creep(&self.session_files_modified, 5) {
            patterns.push(pattern);
        }

        patterns
    }

    /// Run full anti-pattern detection.
    ///
    /// This modifies the internal detector state and returns detected patterns.
    pub fn detect_anti_patterns(&mut self) -> Vec<AntiPattern> {
        let mut patterns = self.detector.detect();

        // Add quality gate ignoring
        if let Some(pattern) =
            detect_quality_gate_ignoring(&self.quality_status, self.consecutive_quality_failures)
        {
            patterns.push(pattern);
        }

        // Add scope creep
        if let Some(pattern) = detect_scope_creep(&self.session_files_modified, 5) {
            patterns.push(pattern);
        }

        patterns
    }

    // =========================================================================
    // State Reset
    // =========================================================================

    /// Reset all state for a new session.
    pub fn reset(&mut self) {
        self.error_aggregator = ErrorAggregator::new();
        self.current_task = None;
        self.session_stats = SessionStats::new(0, 0, 0);
        self.quality_status = QualityGateStatus::new();
        self.attempts.clear();
        self.historical_guidance.clear();
        self.consecutive_quality_failures = 0;
        self.session_files_modified.clear();
        self.detector.clear();
        self.clear_intelligence_queries();
    }

    /// Reset only the iteration-specific state (keeps session stats).
    pub fn reset_iteration(&mut self) {
        // Keep: session_stats, current_task (task continues), detector history
        // Clear: errors (fresh for this iteration)
        self.error_aggregator = ErrorAggregator::new();
    }
}

impl std::fmt::Debug for PromptAssembler {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PromptAssembler")
            .field("config", &self.config)
            .field("current_task", &self.current_task)
            .field("session_stats", &self.session_stats)
            .field("quality_status", &self.quality_status)
            .field("attempts", &self.attempts)
            .field("historical_guidance", &self.historical_guidance)
            .field("consecutive_quality_failures", &self.consecutive_quality_failures)
            .field("session_files_modified", &self.session_files_modified)
            .field("has_narsil_client", &self.narsil_client.is_some())
            .field("intelligence_functions", &self.intelligence_functions)
            .field("intelligence_symbols", &self.intelligence_symbols)
            .field("intelligence_files", &self.intelligence_files)
            .finish_non_exhaustive()
    }
}

impl Default for PromptAssembler {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // AssemblerConfig tests

    #[test]
    fn test_assembler_config_default() {
        let config = AssemblerConfig::default();
        assert!(config.load_templates_from_dir.is_none());
        assert_eq!(config.max_errors, 10);
        assert_eq!(config.max_attempts, 5);
    }

    #[test]
    fn test_assembler_config_builders() {
        let config = AssemblerConfig::new()
            .with_template_dir("/templates")
            .with_max_errors(5)
            .with_max_attempts(3)
            .with_max_anti_patterns(2);

        assert_eq!(
            config.load_templates_from_dir,
            Some("/templates".to_string())
        );
        assert_eq!(config.max_errors, 5);
        assert_eq!(config.max_attempts, 3);
        assert_eq!(config.max_anti_patterns, 2);
    }

    // PromptAssembler creation tests

    #[test]
    fn test_assembler_new() {
        let assembler = PromptAssembler::new();
        assert!(assembler.current_task().is_none());
        assert_eq!(assembler.error_count(), 0);
        assert_eq!(assembler.attempt_count(), 0);
    }

    #[test]
    fn test_assembler_with_config() {
        let config = AssemblerConfig::new().with_max_errors(3);
        let assembler = PromptAssembler::with_config(config);
        assert_eq!(assembler.config.max_errors, 3);
    }

    // Task context tests

    #[test]
    fn test_set_current_task() {
        let mut assembler = PromptAssembler::new();
        assembler.set_current_task("2.1", "Implement feature", TaskPhase::Implementation);

        let task = assembler.current_task().unwrap();
        assert_eq!(task.task_id, "2.1");
        assert_eq!(task.title, "Implement feature");
        assert!(matches!(task.phase, TaskPhase::Implementation));
    }

    #[test]
    fn test_update_task_completion() {
        let mut assembler = PromptAssembler::new();
        assembler.set_current_task("1.1", "Test", TaskPhase::Testing);
        assembler.update_task_completion(75);

        assert_eq!(assembler.current_task().unwrap().completion_percentage, 75);
    }

    #[test]
    fn test_update_task_completion_caps_at_100() {
        let mut assembler = PromptAssembler::new();
        assembler.set_current_task("1.1", "Test", TaskPhase::Testing);
        assembler.update_task_completion(150);

        assert_eq!(assembler.current_task().unwrap().completion_percentage, 100);
    }

    #[test]
    fn test_update_task_files() {
        let mut assembler = PromptAssembler::new();
        assembler.set_current_task("1.1", "Test", TaskPhase::Testing);
        assembler.update_task_files(vec!["src/lib.rs".to_string()]);

        assert_eq!(
            assembler.current_task().unwrap().modified_files,
            vec!["src/lib.rs"]
        );
    }

    #[test]
    fn test_add_task_blocker() {
        let mut assembler = PromptAssembler::new();
        assembler.set_current_task("1.1", "Test", TaskPhase::Testing);
        assembler.add_task_blocker("Dependency not available".to_string());

        assert_eq!(
            assembler.current_task().unwrap().blockers,
            vec!["Dependency not available"]
        );
    }

    #[test]
    fn test_task_switch_clears_attempts() {
        let mut assembler = PromptAssembler::new();

        // Set task and record an attempt
        assembler.set_current_task("1.1", "First", TaskPhase::Testing);
        assembler.record_attempt(AttemptOutcome::TestFailure, None, vec![]);
        assert_eq!(assembler.attempt_count(), 1);

        // Switch to different task - attempts should be cleared
        assembler.set_current_task("2.1", "Second", TaskPhase::Implementation);
        assert_eq!(assembler.attempt_count(), 0);
    }

    #[test]
    fn test_same_task_preserves_attempts() {
        let mut assembler = PromptAssembler::new();

        // Set task and record an attempt
        assembler.set_current_task("1.1", "First", TaskPhase::Testing);
        assembler.record_attempt(AttemptOutcome::TestFailure, None, vec![]);

        // Re-set same task - attempts should be preserved
        assembler.set_current_task("1.1", "First Updated", TaskPhase::Testing);
        assert_eq!(assembler.attempt_count(), 1);
    }

    // Session stats tests

    #[test]
    fn test_update_session_stats() {
        let mut assembler = PromptAssembler::new();
        assembler.update_session_stats(5, 2, 150);

        assert_eq!(assembler.session_stats().iteration_count, 5);
        assert_eq!(assembler.session_stats().commit_count, 2);
        assert_eq!(assembler.session_stats().lines_changed, 150);
    }

    #[test]
    fn test_set_budget() {
        let mut assembler = PromptAssembler::new();
        assembler.update_session_stats(5, 2, 150);
        assembler.set_budget(10);

        assert_eq!(assembler.session_stats().max_iterations, Some(10));
    }

    #[test]
    fn test_increment_iteration() {
        let mut assembler = PromptAssembler::new();
        assembler.update_session_stats(5, 2, 150);
        assembler.increment_iteration();

        assert_eq!(assembler.session_stats().iteration_count, 6);
    }

    #[test]
    fn test_increment_commits() {
        let mut assembler = PromptAssembler::new();
        assembler.update_session_stats(5, 2, 150);
        assembler.increment_commits();

        assert_eq!(assembler.session_stats().commit_count, 3);
    }

    // Error tests

    #[test]
    fn test_add_error() {
        let mut assembler = PromptAssembler::new();
        assembler.add_error("E0308", "mismatched types", ErrorSeverity::Error);

        assert_eq!(assembler.error_count(), 1);
    }

    #[test]
    fn test_errors_are_deduplicated() {
        let mut assembler = PromptAssembler::new();
        assembler.add_error("E0308", "mismatched types", ErrorSeverity::Error);
        assembler.add_error("E0308", "mismatched types", ErrorSeverity::Error);

        assert_eq!(assembler.error_count(), 1);
    }

    #[test]
    fn test_clear_errors() {
        let mut assembler = PromptAssembler::new();
        assembler.add_error("E0308", "mismatched types", ErrorSeverity::Error);
        assembler.clear_errors();

        assert_eq!(assembler.error_count(), 0);
    }

    // Quality gate tests

    #[test]
    fn test_update_clippy_status_passing() {
        let mut assembler = PromptAssembler::new();
        assembler.update_clippy_status(true, vec![]);

        assert!(assembler.quality_status().clippy.passed);
    }

    #[test]
    fn test_update_clippy_status_failing() {
        let mut assembler = PromptAssembler::new();
        assembler.update_clippy_status(false, vec!["warning: unused".to_string()]);

        assert!(!assembler.quality_status().clippy.passed);
        assert_eq!(
            assembler.quality_status().clippy.messages,
            vec!["warning: unused"]
        );
    }

    #[test]
    fn test_quality_failure_streak() {
        let mut assembler = PromptAssembler::new();

        assembler.update_test_status(false, vec![]);
        assert_eq!(assembler.consecutive_quality_failures, 1);

        assembler.update_test_status(false, vec![]);
        assert_eq!(assembler.consecutive_quality_failures, 2);

        assembler.update_test_status(true, vec![]);
        assert_eq!(assembler.consecutive_quality_failures, 0);
    }

    // Attempt tests

    #[test]
    fn test_record_attempt() {
        let mut assembler = PromptAssembler::new();
        assembler.set_current_task("1.1", "Test", TaskPhase::Testing);
        assembler.record_attempt(
            AttemptOutcome::TestFailure,
            Some("TDD approach"),
            vec!["test_foo failed".to_string()],
        );

        assert_eq!(assembler.attempt_count(), 1);
        assert_eq!(assembler.current_task().unwrap().attempt_count, 1);
    }

    #[test]
    fn test_clear_attempts() {
        let mut assembler = PromptAssembler::new();
        assembler.set_current_task("1.1", "Test", TaskPhase::Testing);
        assembler.record_attempt(AttemptOutcome::TestFailure, None, vec![]);
        assembler.clear_attempts();

        assert_eq!(assembler.attempt_count(), 0);
        assert_eq!(assembler.current_task().unwrap().attempt_count, 0);
    }

    // Anti-pattern detection tests

    #[test]
    fn test_record_iteration_with_files() {
        let mut assembler = PromptAssembler::new();
        assembler.record_iteration_with_files(1, vec!["src/lib.rs".to_string()], false);

        assert_eq!(assembler.iteration_count(), 1);
    }

    #[test]
    fn test_clear_detector() {
        let mut assembler = PromptAssembler::new();
        assembler.record_iteration_with_files(1, vec!["src/lib.rs".to_string()], false);
        assembler.clear_detector();

        assert_eq!(assembler.iteration_count(), 0);
    }

    // Historical guidance tests

    #[test]
    fn test_add_guidance() {
        let mut assembler = PromptAssembler::new();
        assembler.add_guidance("Use TDD approach".to_string());

        assert_eq!(assembler.historical_guidance.len(), 1);
    }

    #[test]
    fn test_guidance_deduplication() {
        let mut assembler = PromptAssembler::new();
        assembler.add_guidance("Use TDD approach".to_string());
        assembler.add_guidance("Use TDD approach".to_string());

        assert_eq!(assembler.historical_guidance.len(), 1);
    }

    // Prompt building tests

    #[test]
    fn test_build_prompt_build_mode() {
        let assembler = PromptAssembler::new();
        let prompt = assembler.build_prompt("build").unwrap();

        assert!(prompt.contains("Build Phase"));
    }

    #[test]
    fn test_build_prompt_debug_mode() {
        let assembler = PromptAssembler::new();
        let prompt = assembler.build_prompt("debug").unwrap();

        assert!(prompt.contains("Debug Phase"));
    }

    #[test]
    fn test_build_prompt_plan_mode() {
        let assembler = PromptAssembler::new();
        let prompt = assembler.build_prompt("plan").unwrap();

        assert!(prompt.contains("Plan Phase"));
    }

    #[test]
    fn test_build_prompt_unknown_mode() {
        let assembler = PromptAssembler::new();
        let result = assembler.build_prompt("unknown");

        assert!(result.is_err());
    }

    #[test]
    fn test_build_prompt_includes_task() {
        let mut assembler = PromptAssembler::new();
        assembler.set_current_task("2.1", "Test Feature", TaskPhase::Implementation);

        let prompt = assembler.build_prompt("build").unwrap();

        assert!(prompt.contains("2.1"));
        assert!(prompt.contains("Test Feature"));
    }

    #[test]
    fn test_build_prompt_includes_errors() {
        let mut assembler = PromptAssembler::new();
        assembler.add_error("E0308", "mismatched types", ErrorSeverity::Error);

        let prompt = assembler.build_prompt("build").unwrap();

        assert!(prompt.contains("E0308"));
    }

    #[test]
    fn test_build_prompt_includes_session_stats() {
        let mut assembler = PromptAssembler::new();
        assembler.update_session_stats(5, 2, 150);

        let prompt = assembler.build_prompt("build").unwrap();

        assert!(prompt.contains("Session Progress"));
    }

    // Context building tests

    #[test]
    fn test_build_context() {
        let mut assembler = PromptAssembler::new();
        assembler.set_current_task("1.1", "Test", TaskPhase::Testing);
        assembler.add_error("E0308", "error", ErrorSeverity::Error);
        assembler.update_session_stats(3, 1, 50);

        let context = assembler.build_context();

        assert!(context.current_task.is_some());
        assert_eq!(context.error_count(), 1);
    }

    // Reset tests

    #[test]
    fn test_reset() {
        let mut assembler = PromptAssembler::new();
        assembler.set_current_task("1.1", "Test", TaskPhase::Testing);
        assembler.add_error("E0308", "error", ErrorSeverity::Error);
        assembler.update_session_stats(3, 1, 50);
        assembler.record_attempt(AttemptOutcome::TestFailure, None, vec![]);

        assembler.reset();

        assert!(assembler.current_task().is_none());
        assert_eq!(assembler.error_count(), 0);
        assert_eq!(assembler.attempt_count(), 0);
        assert_eq!(assembler.session_stats().iteration_count, 0);
    }

    #[test]
    fn test_reset_iteration() {
        let mut assembler = PromptAssembler::new();
        assembler.set_current_task("1.1", "Test", TaskPhase::Testing);
        assembler.add_error("E0308", "error", ErrorSeverity::Error);
        assembler.update_session_stats(3, 1, 50);

        assembler.reset_iteration();

        // Task and stats preserved
        assert!(assembler.current_task().is_some());
        assert_eq!(assembler.session_stats().iteration_count, 3);
        // Errors cleared
        assert_eq!(assembler.error_count(), 0);
    }

    // Integration tests

    #[test]
    fn test_full_workflow() {
        let mut assembler = PromptAssembler::new();

        // Set up task
        assembler.set_current_task("2.1", "Implement feature", TaskPhase::Implementation);

        // Record some progress
        assembler.update_session_stats(3, 1, 150);
        assembler.set_budget(10);

        // Record an iteration
        assembler.record_iteration_with_files(1, vec!["src/lib.rs".to_string()], false);

        // Add an error
        assembler.add_error("E0308", "mismatched types", ErrorSeverity::Error);

        // Update quality
        assembler.update_test_status(false, vec!["test_foo failed".to_string()]);

        // Record an attempt
        assembler.record_attempt(
            AttemptOutcome::TestFailure,
            Some("Direct implementation"),
            vec!["test_foo failed".to_string()],
        );

        // Build prompt
        let prompt = assembler.build_prompt("build").unwrap();

        // Verify all components are present
        assert!(prompt.contains("2.1"));
        assert!(prompt.contains("E0308"));
        assert!(prompt.contains("Session Progress"));
    }

    #[test]
    fn test_scope_creep_detection() {
        let mut assembler = PromptAssembler::new();

        // Modify files across many directories
        assembler.update_task_files(vec!["src/a/mod.rs".to_string()]);
        assembler.update_task_files(vec!["src/b/mod.rs".to_string()]);
        assembler.update_task_files(vec!["src/c/mod.rs".to_string()]);
        assembler.update_task_files(vec!["src/d/mod.rs".to_string()]);
        assembler.update_task_files(vec!["src/e/mod.rs".to_string()]);

        let patterns = assembler.detect_anti_patterns_snapshot();

        assert!(patterns
            .iter()
            .any(|p| p.pattern_type == crate::prompt::context::AntiPatternType::ScopeCreep));
    }

    // =========================================================================
    // Code Intelligence Integration Tests
    // =========================================================================

    #[test]
    fn test_assembler_without_intelligence_has_empty_code_intelligence() {
        let assembler = PromptAssembler::new();
        let context = assembler.build_context();

        // Without a NarsilClient, code intelligence should be unavailable
        assert!(!context.code_intelligence.is_available);
        assert!(!context.code_intelligence.has_data());
    }

    #[test]
    fn test_assembler_add_function_for_intelligence() {
        let mut assembler = PromptAssembler::new();

        // Add a function to query for intelligence
        assembler.add_intelligence_function("process_request");
        assembler.add_intelligence_function("validate_input");

        assert_eq!(assembler.intelligence_functions().len(), 2);
    }

    #[test]
    fn test_assembler_add_symbol_for_intelligence() {
        let mut assembler = PromptAssembler::new();

        // Add a symbol to query for references
        assembler.add_intelligence_symbol("MyStruct");

        assert_eq!(assembler.intelligence_symbols().len(), 1);
    }

    #[test]
    fn test_assembler_add_file_for_intelligence() {
        let mut assembler = PromptAssembler::new();

        // Add a file to query for dependencies
        assembler.add_intelligence_file("src/lib.rs");

        assert_eq!(assembler.intelligence_files().len(), 1);
    }

    #[test]
    fn test_assembler_clear_intelligence_queries() {
        let mut assembler = PromptAssembler::new();

        assembler.add_intelligence_function("foo");
        assembler.add_intelligence_symbol("Bar");
        assembler.add_intelligence_file("src/lib.rs");

        assembler.clear_intelligence_queries();

        assert!(assembler.intelligence_functions().is_empty());
        assert!(assembler.intelligence_symbols().is_empty());
        assert!(assembler.intelligence_files().is_empty());
    }

    #[test]
    fn test_assembler_with_narsil_client() {
        use crate::narsil::{NarsilClient, NarsilConfig};

        // Create a client (will be unavailable since narsil-mcp not installed)
        let config = NarsilConfig::new(".");
        let client = NarsilClient::new(config).unwrap();

        let assembler = PromptAssembler::with_narsil_client(client);

        // The assembler should now have a client configured
        assert!(assembler.has_narsil_client());
    }

    #[test]
    fn test_assembler_build_context_with_narsil_client() {
        use crate::narsil::{NarsilClient, NarsilConfig};

        // Create a client (unavailable in test environment)
        let config = NarsilConfig::new(".");
        let client = NarsilClient::new(config).unwrap();

        let mut assembler = PromptAssembler::with_narsil_client(client);
        assembler.add_intelligence_function("test_function");

        let context = assembler.build_context();

        // Even with unavailable client, the field should be properly initialized
        // (is_available will be false since narsil-mcp isn't installed in test)
        // The key is that build_context properly tries to gather intelligence
        assert!(context.code_intelligence.call_graph.is_empty() || !context.code_intelligence.is_available);
    }

    #[test]
    fn test_assembler_reset_clears_intelligence() {
        let mut assembler = PromptAssembler::new();

        assembler.add_intelligence_function("foo");
        assembler.add_intelligence_symbol("Bar");

        assembler.reset();

        assert!(assembler.intelligence_functions().is_empty());
        assert!(assembler.intelligence_symbols().is_empty());
        assert!(assembler.intelligence_files().is_empty());
    }
}
