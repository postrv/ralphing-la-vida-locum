//! Prompt context types for dynamic prompt generation.
//!
//! This module defines the context structures used to generate dynamic prompts.
//! Each struct captures a specific aspect of the current session state.
//!
//! # Example
//!
//! ```
//! use ralph::prompt::context::{PromptContext, CurrentTaskContext, SessionStats};
//!
//! let context = PromptContext::new()
//!     .with_session_stats(SessionStats::new(5, 2, 150));
//!
//! assert_eq!(context.session_stats.iteration_count, 5);
//! ```

use crate::narsil::{CcgArchitecture, CcgManifest, ComplianceResult, ConstraintSet};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Aggregate context for prompt generation.
///
/// This struct collects all context needed to generate a dynamic prompt,
/// including task information, errors, quality status, and session stats.
///
/// # Example
///
/// ```
/// use ralph::prompt::context::PromptContext;
///
/// let context = PromptContext::new();
/// assert!(context.current_task.is_none());
/// assert!(context.errors.is_empty());
/// ```
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct PromptContext {
    /// Current task being worked on, if any.
    pub current_task: Option<CurrentTaskContext>,
    /// Recent errors with occurrence tracking.
    pub errors: Vec<ErrorContext>,
    /// Quality gate status from recent checks.
    pub quality_status: QualityGateStatus,
    /// Session-level statistics.
    pub session_stats: SessionStats,
    /// Previous attempt summaries for the current task.
    pub attempt_summaries: Vec<AttemptSummary>,
    /// Detected anti-patterns.
    pub anti_patterns: Vec<AntiPattern>,
    /// Code intelligence context from narsil-mcp.
    pub code_intelligence: CodeIntelligenceContext,
}

impl PromptContext {
    /// Create a new empty prompt context.
    ///
    /// # Example
    ///
    /// ```
    /// use ralph::prompt::context::PromptContext;
    ///
    /// let context = PromptContext::new();
    /// assert!(context.current_task.is_none());
    /// ```
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Set the current task context.
    ///
    /// # Example
    ///
    /// ```
    /// use ralph::prompt::context::{PromptContext, CurrentTaskContext, TaskPhase};
    ///
    /// let task = CurrentTaskContext::new("1.2", "Implement parser", TaskPhase::Implementation);
    /// let context = PromptContext::new().with_current_task(task);
    /// assert!(context.current_task.is_some());
    /// ```
    #[must_use]
    pub fn with_current_task(mut self, task: CurrentTaskContext) -> Self {
        self.current_task = Some(task);
        self
    }

    /// Add an error context.
    ///
    /// # Example
    ///
    /// ```
    /// use ralph::prompt::context::{PromptContext, ErrorContext, ErrorSeverity};
    ///
    /// let error = ErrorContext::new("E0308", "mismatched types", ErrorSeverity::Error);
    /// let context = PromptContext::new().with_error(error);
    /// assert_eq!(context.errors.len(), 1);
    /// ```
    #[must_use]
    pub fn with_error(mut self, error: ErrorContext) -> Self {
        self.errors.push(error);
        self
    }

    /// Add multiple error contexts.
    #[must_use]
    pub fn with_errors(mut self, errors: Vec<ErrorContext>) -> Self {
        self.errors.extend(errors);
        self
    }

    /// Set the quality gate status.
    ///
    /// # Example
    ///
    /// ```
    /// use ralph::prompt::context::{PromptContext, QualityGateStatus};
    ///
    /// let status = QualityGateStatus::all_passing();
    /// let context = PromptContext::new().with_quality_status(status);
    /// assert!(context.quality_status.all_passed());
    /// ```
    #[must_use]
    pub fn with_quality_status(mut self, status: QualityGateStatus) -> Self {
        self.quality_status = status;
        self
    }

    /// Set the session statistics.
    ///
    /// # Example
    ///
    /// ```
    /// use ralph::prompt::context::{PromptContext, SessionStats};
    ///
    /// let stats = SessionStats::new(10, 5, 500);
    /// let context = PromptContext::new().with_session_stats(stats);
    /// assert_eq!(context.session_stats.iteration_count, 10);
    /// ```
    #[must_use]
    pub fn with_session_stats(mut self, stats: SessionStats) -> Self {
        self.session_stats = stats;
        self
    }

    /// Add an attempt summary.
    #[must_use]
    pub fn with_attempt(mut self, attempt: AttemptSummary) -> Self {
        self.attempt_summaries.push(attempt);
        self
    }

    /// Add multiple attempt summaries.
    #[must_use]
    pub fn with_attempts(mut self, attempts: Vec<AttemptSummary>) -> Self {
        self.attempt_summaries.extend(attempts);
        self
    }

    /// Add an anti-pattern.
    #[must_use]
    pub fn with_anti_pattern(mut self, pattern: AntiPattern) -> Self {
        self.anti_patterns.push(pattern);
        self
    }

    /// Add multiple anti-patterns.
    #[must_use]
    pub fn with_anti_patterns(mut self, patterns: Vec<AntiPattern>) -> Self {
        self.anti_patterns.extend(patterns);
        self
    }

    /// Set the code intelligence context.
    ///
    /// # Example
    ///
    /// ```
    /// use ralph::prompt::context::{PromptContext, CodeIntelligenceContext, CallGraphNode};
    ///
    /// let intel = CodeIntelligenceContext::new()
    ///     .with_call_graph(vec![CallGraphNode::new("foo")]);
    /// let context = PromptContext::new().with_code_intelligence(intel);
    /// assert!(context.code_intelligence.has_data());
    /// ```
    #[must_use]
    pub fn with_code_intelligence(mut self, intel: CodeIntelligenceContext) -> Self {
        self.code_intelligence = intel;
        self
    }

    /// Check if there are any critical issues requiring attention.
    ///
    /// Returns true if there are errors, failing quality gates (that have been checked),
    /// or anti-patterns.
    #[must_use]
    pub fn has_critical_issues(&self) -> bool {
        !self.errors.is_empty()
            || self.quality_status.has_failures()
            || !self.anti_patterns.is_empty()
    }

    /// Get the total error count.
    #[must_use]
    pub fn error_count(&self) -> usize {
        self.errors.len()
    }

    /// Get errors sorted by occurrence count (most frequent first).
    #[must_use]
    pub fn errors_by_frequency(&self) -> Vec<&ErrorContext> {
        let mut sorted: Vec<_> = self.errors.iter().collect();
        sorted.sort_by(|a, b| b.occurrence_count.cmp(&a.occurrence_count));
        sorted
    }
}

/// Context for the current task being worked on.
///
/// # Example
///
/// ```
/// use ralph::prompt::context::{CurrentTaskContext, TaskPhase};
///
/// let task = CurrentTaskContext::new("2.1", "Create context types", TaskPhase::Testing);
/// assert_eq!(task.task_id, "2.1");
/// assert_eq!(task.phase, TaskPhase::Testing);
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CurrentTaskContext {
    /// Task identifier (e.g., "1.2", "2.3").
    pub task_id: String,
    /// Human-readable task title.
    pub title: String,
    /// Current phase of work on this task.
    pub phase: TaskPhase,
    /// Percentage complete (0-100).
    pub completion_percentage: u8,
    /// Number of attempts on this task.
    pub attempt_count: u32,
    /// Files modified in current attempt.
    pub modified_files: Vec<String>,
    /// Known blockers for this task.
    pub blockers: Vec<String>,
    /// Dependencies that must be complete first.
    pub dependencies: Vec<String>,
}

impl CurrentTaskContext {
    /// Create a new task context.
    ///
    /// # Example
    ///
    /// ```
    /// use ralph::prompt::context::{CurrentTaskContext, TaskPhase};
    ///
    /// let task = CurrentTaskContext::new("1.1", "Setup testing", TaskPhase::Implementation);
    /// assert_eq!(task.task_id, "1.1");
    /// assert_eq!(task.attempt_count, 0);
    /// ```
    #[must_use]
    pub fn new(task_id: impl Into<String>, title: impl Into<String>, phase: TaskPhase) -> Self {
        Self {
            task_id: task_id.into(),
            title: title.into(),
            phase,
            completion_percentage: 0,
            attempt_count: 0,
            modified_files: Vec::new(),
            blockers: Vec::new(),
            dependencies: Vec::new(),
        }
    }

    /// Set completion percentage.
    #[must_use]
    pub fn with_completion(mut self, percentage: u8) -> Self {
        self.completion_percentage = percentage.min(100);
        self
    }

    /// Set attempt count.
    #[must_use]
    pub fn with_attempts(mut self, count: u32) -> Self {
        self.attempt_count = count;
        self
    }

    /// Add modified files.
    #[must_use]
    pub fn with_modified_files(mut self, files: Vec<String>) -> Self {
        self.modified_files = files;
        self
    }

    /// Add blockers.
    #[must_use]
    pub fn with_blockers(mut self, blockers: Vec<String>) -> Self {
        self.blockers = blockers;
        self
    }

    /// Add dependencies.
    #[must_use]
    pub fn with_dependencies(mut self, deps: Vec<String>) -> Self {
        self.dependencies = deps;
        self
    }

    /// Check if task is blocked.
    #[must_use]
    pub fn is_blocked(&self) -> bool {
        !self.blockers.is_empty()
    }

    /// Check if task has unmet dependencies.
    #[must_use]
    pub fn has_dependencies(&self) -> bool {
        !self.dependencies.is_empty()
    }
}

/// Phase of work on a task.
///
/// # Example
///
/// ```
/// use ralph::prompt::context::TaskPhase;
///
/// let phase = TaskPhase::Testing;
/// assert_eq!(phase.to_string(), "Testing");
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TaskPhase {
    /// Planning the implementation approach.
    Planning,
    /// Writing the implementation code.
    Implementation,
    /// Writing or fixing tests.
    Testing,
    /// Fixing quality gate failures.
    QualityFixes,
    /// Reviewing and refining.
    Review,
}

impl std::fmt::Display for TaskPhase {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TaskPhase::Planning => write!(f, "Planning"),
            TaskPhase::Implementation => write!(f, "Implementation"),
            TaskPhase::Testing => write!(f, "Testing"),
            TaskPhase::QualityFixes => write!(f, "Quality Fixes"),
            TaskPhase::Review => write!(f, "Review"),
        }
    }
}

/// Error context with occurrence tracking.
///
/// Tracks how often an error has occurred and provides context for remediation.
///
/// # Example
///
/// ```
/// use ralph::prompt::context::{ErrorContext, ErrorSeverity};
///
/// let error = ErrorContext::new("E0308", "mismatched types", ErrorSeverity::Error)
///     .with_location("src/lib.rs", 42);
/// assert_eq!(error.code, "E0308");
/// assert_eq!(error.occurrence_count, 1);
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ErrorContext {
    /// Error code (e.g., "E0308", "clippy::unwrap_used").
    pub code: String,
    /// Human-readable error message.
    pub message: String,
    /// Error severity level.
    pub severity: ErrorSeverity,
    /// Number of times this error has occurred.
    pub occurrence_count: u32,
    /// File where the error occurred.
    pub file: Option<String>,
    /// Line number where the error occurred.
    pub line: Option<u32>,
    /// Suggested fix, if available.
    pub suggested_fix: Option<String>,
    /// Additional context or notes.
    pub context: Option<String>,
}

impl ErrorContext {
    /// Create a new error context.
    ///
    /// # Example
    ///
    /// ```
    /// use ralph::prompt::context::{ErrorContext, ErrorSeverity};
    ///
    /// let error = ErrorContext::new("E0433", "failed to resolve", ErrorSeverity::Error);
    /// assert_eq!(error.code, "E0433");
    /// assert_eq!(error.occurrence_count, 1);
    /// ```
    #[must_use]
    pub fn new(
        code: impl Into<String>,
        message: impl Into<String>,
        severity: ErrorSeverity,
    ) -> Self {
        Self {
            code: code.into(),
            message: message.into(),
            severity,
            occurrence_count: 1,
            file: None,
            line: None,
            suggested_fix: None,
            context: None,
        }
    }

    /// Set the file and line location.
    #[must_use]
    pub fn with_location(mut self, file: impl Into<String>, line: u32) -> Self {
        self.file = Some(file.into());
        self.line = Some(line);
        self
    }

    /// Set the occurrence count.
    #[must_use]
    pub fn with_occurrences(mut self, count: u32) -> Self {
        self.occurrence_count = count;
        self
    }

    /// Set a suggested fix.
    #[must_use]
    pub fn with_suggested_fix(mut self, fix: impl Into<String>) -> Self {
        self.suggested_fix = Some(fix.into());
        self
    }

    /// Set additional context.
    #[must_use]
    pub fn with_context(mut self, context: impl Into<String>) -> Self {
        self.context = Some(context.into());
        self
    }

    /// Increment occurrence count.
    pub fn increment(&mut self) {
        self.occurrence_count += 1;
    }

    /// Check if this is a recurring error (seen multiple times).
    #[must_use]
    pub fn is_recurring(&self) -> bool {
        self.occurrence_count > 1
    }

    /// Check if this is a critical error.
    #[must_use]
    pub fn is_critical(&self) -> bool {
        self.severity == ErrorSeverity::Error
    }
}

/// Error severity levels.
///
/// # Example
///
/// ```
/// use ralph::prompt::context::ErrorSeverity;
///
/// let severity = ErrorSeverity::Warning;
/// assert_eq!(severity.to_string(), "warning");
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum ErrorSeverity {
    /// Informational message.
    Info,
    /// Warning that should be addressed.
    Warning,
    /// Error that must be fixed.
    Error,
}

impl std::fmt::Display for ErrorSeverity {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ErrorSeverity::Info => write!(f, "info"),
            ErrorSeverity::Warning => write!(f, "warning"),
            ErrorSeverity::Error => write!(f, "error"),
        }
    }
}

/// Quality gate status from recent checks.
///
/// Tracks the pass/fail status of each quality gate.
///
/// # Example
///
/// ```
/// use ralph::prompt::context::{QualityGateStatus, GateResult};
///
/// let status = QualityGateStatus::new()
///     .with_clippy(GateResult::pass())
///     .with_tests(GateResult::fail(vec!["test_foo failed".to_string()]));
///
/// assert!(!status.all_passed());
/// ```
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct QualityGateStatus {
    /// Clippy lint check result.
    pub clippy: GateResult,
    /// Test suite result.
    pub tests: GateResult,
    /// No-allow annotation check result.
    pub no_allow: GateResult,
    /// Security scan result.
    pub security: GateResult,
    /// Documentation check result.
    pub docs: GateResult,
    /// Last check timestamp (Unix epoch seconds).
    pub last_check: Option<i64>,
}

impl QualityGateStatus {
    /// Create a new quality gate status with all gates unknown.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Create a status where all gates pass.
    #[must_use]
    pub fn all_passing() -> Self {
        Self {
            clippy: GateResult::pass(),
            tests: GateResult::pass(),
            no_allow: GateResult::pass(),
            security: GateResult::pass(),
            docs: GateResult::pass(),
            last_check: Some(chrono::Utc::now().timestamp()),
        }
    }

    /// Set clippy result.
    #[must_use]
    pub fn with_clippy(mut self, result: GateResult) -> Self {
        self.clippy = result;
        self
    }

    /// Set tests result.
    #[must_use]
    pub fn with_tests(mut self, result: GateResult) -> Self {
        self.tests = result;
        self
    }

    /// Set no-allow result.
    #[must_use]
    pub fn with_no_allow(mut self, result: GateResult) -> Self {
        self.no_allow = result;
        self
    }

    /// Set security result.
    #[must_use]
    pub fn with_security(mut self, result: GateResult) -> Self {
        self.security = result;
        self
    }

    /// Set docs result.
    #[must_use]
    pub fn with_docs(mut self, result: GateResult) -> Self {
        self.docs = result;
        self
    }

    /// Set last check timestamp to now.
    #[must_use]
    pub fn with_timestamp(mut self) -> Self {
        self.last_check = Some(chrono::Utc::now().timestamp());
        self
    }

    /// Check if all gates passed.
    #[must_use]
    pub fn all_passed(&self) -> bool {
        self.clippy.passed
            && self.tests.passed
            && self.no_allow.passed
            && self.security.passed
            && self.docs.passed
    }

    /// Check if any gates have been checked and failed.
    ///
    /// Returns false if no gates have been checked yet (last_check is None).
    #[must_use]
    pub fn has_failures(&self) -> bool {
        // Only consider failures if gates have actually been checked
        self.last_check.is_some() && !self.all_passed()
    }

    /// Get a list of failing gates.
    #[must_use]
    pub fn failing_gates(&self) -> Vec<&str> {
        let mut failing = Vec::new();
        if !self.clippy.passed {
            failing.push("clippy");
        }
        if !self.tests.passed {
            failing.push("tests");
        }
        if !self.no_allow.passed {
            failing.push("no_allow");
        }
        if !self.security.passed {
            failing.push("security");
        }
        if !self.docs.passed {
            failing.push("docs");
        }
        failing
    }

    /// Get total failure count.
    #[must_use]
    pub fn failure_count(&self) -> usize {
        let mut count = 0;
        if !self.clippy.passed {
            count += self.clippy.messages.len().max(1);
        }
        if !self.tests.passed {
            count += self.tests.messages.len().max(1);
        }
        if !self.no_allow.passed {
            count += self.no_allow.messages.len().max(1);
        }
        if !self.security.passed {
            count += self.security.messages.len().max(1);
        }
        if !self.docs.passed {
            count += self.docs.messages.len().max(1);
        }
        count
    }
}

/// Result of a single quality gate check.
///
/// # Example
///
/// ```
/// use ralph::prompt::context::GateResult;
///
/// let pass = GateResult::pass();
/// assert!(pass.passed);
///
/// let fail = GateResult::fail(vec!["error 1".to_string()]);
/// assert!(!fail.passed);
/// assert_eq!(fail.messages.len(), 1);
/// ```
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct GateResult {
    /// Whether the gate passed.
    pub passed: bool,
    /// Messages from the gate (warnings or errors).
    pub messages: Vec<String>,
}

impl GateResult {
    /// Create a passing result.
    #[must_use]
    pub fn pass() -> Self {
        Self {
            passed: true,
            messages: Vec::new(),
        }
    }

    /// Create a failing result with messages.
    #[must_use]
    pub fn fail(messages: Vec<String>) -> Self {
        Self {
            passed: false,
            messages,
        }
    }

    /// Create a result with warnings (passed but has messages).
    #[must_use]
    pub fn pass_with_warnings(messages: Vec<String>) -> Self {
        Self {
            passed: true,
            messages,
        }
    }
}

/// Session-level statistics.
///
/// Tracks overall progress within the current session.
///
/// # Example
///
/// ```
/// use ralph::prompt::context::SessionStats;
///
/// let stats = SessionStats::new(10, 3, 500);
/// assert_eq!(stats.iteration_count, 10);
/// assert_eq!(stats.commit_count, 3);
/// assert_eq!(stats.lines_changed, 500);
/// ```
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SessionStats {
    /// Number of iterations completed.
    pub iteration_count: u32,
    /// Number of commits made.
    pub commit_count: u32,
    /// Total lines changed.
    pub lines_changed: u32,
    /// Number of tasks completed.
    pub tasks_completed: u32,
    /// Number of tasks blocked.
    pub tasks_blocked: u32,
    /// Current stagnation count.
    pub stagnation_count: u32,
    /// Maximum iterations budget.
    pub max_iterations: Option<u32>,
    /// Files modified in session.
    pub files_modified: Vec<String>,
    /// Test count delta (tests added - tests removed).
    pub test_delta: i32,
}

impl SessionStats {
    /// Create new session stats.
    ///
    /// # Example
    ///
    /// ```
    /// use ralph::prompt::context::SessionStats;
    ///
    /// let stats = SessionStats::new(5, 2, 100);
    /// assert_eq!(stats.iteration_count, 5);
    /// ```
    #[must_use]
    pub fn new(iteration_count: u32, commit_count: u32, lines_changed: u32) -> Self {
        Self {
            iteration_count,
            commit_count,
            lines_changed,
            ..Default::default()
        }
    }

    /// Set tasks completed.
    #[must_use]
    pub fn with_tasks_completed(mut self, count: u32) -> Self {
        self.tasks_completed = count;
        self
    }

    /// Set tasks blocked.
    #[must_use]
    pub fn with_tasks_blocked(mut self, count: u32) -> Self {
        self.tasks_blocked = count;
        self
    }

    /// Set stagnation count.
    #[must_use]
    pub fn with_stagnation(mut self, count: u32) -> Self {
        self.stagnation_count = count;
        self
    }

    /// Set max iterations budget.
    #[must_use]
    pub fn with_budget(mut self, max: u32) -> Self {
        self.max_iterations = Some(max);
        self
    }

    /// Set files modified.
    #[must_use]
    pub fn with_files(mut self, files: Vec<String>) -> Self {
        self.files_modified = files;
        self
    }

    /// Set test delta.
    #[must_use]
    pub fn with_test_delta(mut self, delta: i32) -> Self {
        self.test_delta = delta;
        self
    }

    /// Calculate progress percentage (iterations used / budget).
    #[must_use]
    pub fn budget_used_percent(&self) -> Option<u8> {
        self.max_iterations.map(|max| {
            if max == 0 {
                100
            } else {
                ((self.iteration_count as f64 / max as f64) * 100.0).min(100.0) as u8
            }
        })
    }

    /// Check if budget is nearly exhausted (>80%).
    #[must_use]
    pub fn is_budget_critical(&self) -> bool {
        self.budget_used_percent().is_some_and(|p| p > 80)
    }

    /// Check if making good progress (commits relative to iterations).
    #[must_use]
    pub fn is_progressing(&self) -> bool {
        if self.iteration_count == 0 {
            return true;
        }
        // Consider progressing if at least 1 commit per 3 iterations
        self.commit_count * 3 >= self.iteration_count
    }
}

/// Summary of a previous attempt at the current task.
///
/// # Example
///
/// ```
/// use ralph::prompt::context::{AttemptSummary, AttemptOutcome};
///
/// let attempt = AttemptSummary::new(1, AttemptOutcome::QualityGateFailed)
///     .with_approach("TDD approach")
///     .with_error("test_foo failed");
///
/// assert_eq!(attempt.attempt_number, 1);
/// assert!(!attempt.outcome.is_success());
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AttemptSummary {
    /// Attempt number (1-indexed).
    pub attempt_number: u32,
    /// Outcome of the attempt.
    pub outcome: AttemptOutcome,
    /// Approach taken in this attempt.
    pub approach: Option<String>,
    /// Key errors encountered.
    pub errors: Vec<String>,
    /// Files modified in this attempt.
    pub files_modified: Vec<String>,
    /// Duration in seconds.
    pub duration_seconds: Option<u64>,
    /// Lessons learned or notes.
    pub notes: Option<String>,
}

impl AttemptSummary {
    /// Create a new attempt summary.
    ///
    /// # Example
    ///
    /// ```
    /// use ralph::prompt::context::{AttemptSummary, AttemptOutcome};
    ///
    /// let attempt = AttemptSummary::new(2, AttemptOutcome::Success);
    /// assert_eq!(attempt.attempt_number, 2);
    /// ```
    #[must_use]
    pub fn new(attempt_number: u32, outcome: AttemptOutcome) -> Self {
        Self {
            attempt_number,
            outcome,
            approach: None,
            errors: Vec::new(),
            files_modified: Vec::new(),
            duration_seconds: None,
            notes: None,
        }
    }

    /// Set the approach taken.
    #[must_use]
    pub fn with_approach(mut self, approach: impl Into<String>) -> Self {
        self.approach = Some(approach.into());
        self
    }

    /// Add an error.
    #[must_use]
    pub fn with_error(mut self, error: impl Into<String>) -> Self {
        self.errors.push(error.into());
        self
    }

    /// Add multiple errors.
    #[must_use]
    pub fn with_errors(mut self, errors: Vec<String>) -> Self {
        self.errors.extend(errors);
        self
    }

    /// Set files modified.
    #[must_use]
    pub fn with_files(mut self, files: Vec<String>) -> Self {
        self.files_modified = files;
        self
    }

    /// Set duration.
    #[must_use]
    pub fn with_duration(mut self, seconds: u64) -> Self {
        self.duration_seconds = Some(seconds);
        self
    }

    /// Set notes.
    #[must_use]
    pub fn with_notes(mut self, notes: impl Into<String>) -> Self {
        self.notes = Some(notes.into());
        self
    }
}

/// Outcome of an attempt.
///
/// # Example
///
/// ```
/// use ralph::prompt::context::AttemptOutcome;
///
/// assert!(AttemptOutcome::Success.is_success());
/// assert!(!AttemptOutcome::CompilationError.is_success());
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AttemptOutcome {
    /// Attempt succeeded.
    Success,
    /// Compilation failed.
    CompilationError,
    /// Tests failed.
    TestFailure,
    /// Quality gate failed.
    QualityGateFailed,
    /// Attempt timed out.
    Timeout,
    /// Attempt was blocked.
    Blocked,
    /// Attempt was abandoned.
    Abandoned,
}

impl AttemptOutcome {
    /// Check if this is a successful outcome.
    #[must_use]
    pub fn is_success(&self) -> bool {
        matches!(self, AttemptOutcome::Success)
    }

    /// Check if this is a recoverable failure.
    #[must_use]
    pub fn is_recoverable(&self) -> bool {
        matches!(
            self,
            AttemptOutcome::CompilationError
                | AttemptOutcome::TestFailure
                | AttemptOutcome::QualityGateFailed
        )
    }
}

impl std::fmt::Display for AttemptOutcome {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AttemptOutcome::Success => write!(f, "Success"),
            AttemptOutcome::CompilationError => write!(f, "Compilation Error"),
            AttemptOutcome::TestFailure => write!(f, "Test Failure"),
            AttemptOutcome::QualityGateFailed => write!(f, "Quality Gate Failed"),
            AttemptOutcome::Timeout => write!(f, "Timeout"),
            AttemptOutcome::Blocked => write!(f, "Blocked"),
            AttemptOutcome::Abandoned => write!(f, "Abandoned"),
        }
    }
}

/// Detected anti-pattern with evidence.
///
/// # Example
///
/// ```
/// use ralph::prompt::context::{AntiPattern, AntiPatternType};
///
/// let pattern = AntiPattern::new(
///     AntiPatternType::EditWithoutCommit,
///     "Edited 5 files without committing",
/// );
/// assert_eq!(pattern.pattern_type, AntiPatternType::EditWithoutCommit);
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AntiPattern {
    /// Type of anti-pattern detected.
    pub pattern_type: AntiPatternType,
    /// Human-readable description.
    pub description: String,
    /// Evidence supporting the detection.
    pub evidence: Vec<String>,
    /// Severity level.
    pub severity: AntiPatternSeverity,
    /// Suggested remediation.
    pub remediation: Option<String>,
    /// Number of iterations this pattern has persisted.
    pub persistence_count: u32,
}

impl AntiPattern {
    /// Create a new anti-pattern.
    ///
    /// # Example
    ///
    /// ```
    /// use ralph::prompt::context::{AntiPattern, AntiPatternType};
    ///
    /// let pattern = AntiPattern::new(
    ///     AntiPatternType::TestsNotRun,
    ///     "Tests haven't been run in 5 iterations",
    /// );
    /// assert_eq!(pattern.severity, ralph::prompt::context::AntiPatternSeverity::Medium);
    /// ```
    #[must_use]
    pub fn new(pattern_type: AntiPatternType, description: impl Into<String>) -> Self {
        Self {
            severity: pattern_type.default_severity(),
            pattern_type,
            description: description.into(),
            evidence: Vec::new(),
            remediation: None,
            persistence_count: 1,
        }
    }

    /// Add evidence.
    #[must_use]
    pub fn with_evidence(mut self, evidence: Vec<String>) -> Self {
        self.evidence = evidence;
        self
    }

    /// Set severity override.
    #[must_use]
    pub fn with_severity(mut self, severity: AntiPatternSeverity) -> Self {
        self.severity = severity;
        self
    }

    /// Set remediation suggestion.
    #[must_use]
    pub fn with_remediation(mut self, remediation: impl Into<String>) -> Self {
        self.remediation = Some(remediation.into());
        self
    }

    /// Set persistence count.
    #[must_use]
    pub fn with_persistence(mut self, count: u32) -> Self {
        self.persistence_count = count;
        self
    }
}

/// Types of anti-patterns that can be detected.
///
/// # Example
///
/// ```
/// use ralph::prompt::context::AntiPatternType;
///
/// let pattern = AntiPatternType::ClippyNotRun;
/// assert_eq!(pattern.to_string(), "Clippy Not Run");
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum AntiPatternType {
    /// Repeatedly editing files without committing.
    EditWithoutCommit,
    /// Tests haven't been run recently.
    TestsNotRun,
    /// Clippy hasn't been run recently.
    ClippyNotRun,
    /// Oscillating between tasks without completing any.
    TaskOscillation,
    /// Same error recurring multiple times.
    RepeatingErrors,
    /// Modifying the same file repeatedly without progress.
    FileChurn,
    /// Attempting too many things at once.
    ScopeCreep,
    /// Ignoring quality gate failures.
    IgnoringQualityGates,
}

impl AntiPatternType {
    /// Get the default severity for this pattern type.
    #[must_use]
    pub fn default_severity(&self) -> AntiPatternSeverity {
        match self {
            AntiPatternType::EditWithoutCommit => AntiPatternSeverity::Medium,
            AntiPatternType::TestsNotRun => AntiPatternSeverity::Medium,
            AntiPatternType::ClippyNotRun => AntiPatternSeverity::Low,
            AntiPatternType::TaskOscillation => AntiPatternSeverity::High,
            AntiPatternType::RepeatingErrors => AntiPatternSeverity::High,
            AntiPatternType::FileChurn => AntiPatternSeverity::Medium,
            AntiPatternType::ScopeCreep => AntiPatternSeverity::Medium,
            AntiPatternType::IgnoringQualityGates => AntiPatternSeverity::High,
        }
    }
}

impl std::fmt::Display for AntiPatternType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AntiPatternType::EditWithoutCommit => write!(f, "Edit Without Commit"),
            AntiPatternType::TestsNotRun => write!(f, "Tests Not Run"),
            AntiPatternType::ClippyNotRun => write!(f, "Clippy Not Run"),
            AntiPatternType::TaskOscillation => write!(f, "Task Oscillation"),
            AntiPatternType::RepeatingErrors => write!(f, "Repeating Errors"),
            AntiPatternType::FileChurn => write!(f, "File Churn"),
            AntiPatternType::ScopeCreep => write!(f, "Scope Creep"),
            AntiPatternType::IgnoringQualityGates => write!(f, "Ignoring Quality Gates"),
        }
    }
}

/// Severity of an anti-pattern.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum AntiPatternSeverity {
    /// Informational - worth noting but not critical.
    Low,
    /// Should be addressed soon.
    Medium,
    /// Must be addressed immediately.
    High,
}

impl std::fmt::Display for AntiPatternSeverity {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AntiPatternSeverity::Low => write!(f, "Low"),
            AntiPatternSeverity::Medium => write!(f, "Medium"),
            AntiPatternSeverity::High => write!(f, "High"),
        }
    }
}

// ============================================================================
// Code Intelligence Types
// ============================================================================

/// Context for code intelligence data from narsil-mcp.
///
/// Aggregates call graph, reference, and dependency information to enrich prompts
/// with code context that helps make better implementation decisions.
///
/// # Example
///
/// ```
/// use ralph::prompt::context::{CodeIntelligenceContext, CallGraphNode};
///
/// let intel = CodeIntelligenceContext::new()
///     .with_call_graph(vec![CallGraphNode::new("process_request")])
///     .mark_available();
///
/// assert!(intel.is_available);
/// assert!(intel.has_data());
/// ```
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct CodeIntelligenceContext {
    /// Call graph information for relevant functions.
    pub call_graph: Vec<CallGraphNode>,
    /// Symbol references found in the codebase.
    pub references: Vec<SymbolReference>,
    /// Module dependency information.
    pub dependencies: Vec<ModuleDependency>,
    /// Whether narsil-mcp is available and intelligence data is valid.
    pub is_available: bool,
    /// CCG L0 manifest with repository metadata and security summary.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ccg_manifest: Option<CcgManifest>,
    /// CCG L1 architecture with module hierarchy and public API.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ccg_architecture: Option<CcgArchitecture>,
    /// CCG L2 constraints that apply to the codebase.
    #[serde(default, skip_serializing_if = "ConstraintSet::is_empty")]
    pub constraints: ConstraintSet,
    /// Constraint compliance verification result.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub compliance_result: Option<ComplianceResult>,
}

impl CodeIntelligenceContext {
    /// Create a new empty code intelligence context.
    ///
    /// # Example
    ///
    /// ```
    /// use ralph::prompt::context::CodeIntelligenceContext;
    ///
    /// let intel = CodeIntelligenceContext::new();
    /// assert!(!intel.is_available);
    /// ```
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Add call graph nodes.
    #[must_use]
    pub fn with_call_graph(mut self, nodes: Vec<CallGraphNode>) -> Self {
        self.call_graph = nodes;
        self
    }

    /// Add symbol references.
    #[must_use]
    pub fn with_references(mut self, refs: Vec<SymbolReference>) -> Self {
        self.references = refs;
        self
    }

    /// Add module dependencies.
    #[must_use]
    pub fn with_dependencies(mut self, deps: Vec<ModuleDependency>) -> Self {
        self.dependencies = deps;
        self
    }

    /// Mark the context as available (narsil-mcp connected successfully).
    #[must_use]
    pub fn mark_available(mut self) -> Self {
        self.is_available = true;
        self
    }

    /// Check if any intelligence data is present.
    #[must_use]
    pub fn has_data(&self) -> bool {
        !self.call_graph.is_empty()
            || !self.references.is_empty()
            || !self.dependencies.is_empty()
            || self.ccg_manifest.is_some()
            || self.ccg_architecture.is_some()
            || !self.constraints.is_empty()
            || self.compliance_result.is_some()
    }

    /// Get the count of functions in the call graph.
    #[must_use]
    pub fn relevant_functions_count(&self) -> usize {
        self.call_graph.len()
    }

    /// Get hotspot functions (highly connected).
    #[must_use]
    pub fn hotspots(&self) -> Vec<&CallGraphNode> {
        self.call_graph.iter().filter(|n| n.is_hotspot()).collect()
    }

    /// Get definition references only.
    #[must_use]
    pub fn definitions(&self) -> Vec<&SymbolReference> {
        self.references.iter().filter(|r| r.is_definition()).collect()
    }

    /// Add CCG manifest (L0) data.
    #[must_use]
    pub fn with_ccg_manifest(mut self, manifest: CcgManifest) -> Self {
        self.ccg_manifest = Some(manifest);
        self
    }

    /// Add CCG architecture (L1) data.
    #[must_use]
    pub fn with_ccg_architecture(mut self, architecture: CcgArchitecture) -> Self {
        self.ccg_architecture = Some(architecture);
        self
    }

    /// Check if CCG data is available.
    #[must_use]
    pub fn has_ccg_data(&self) -> bool {
        self.ccg_manifest.is_some() || self.ccg_architecture.is_some()
    }

    /// Get security summary from CCG manifest if available.
    #[must_use]
    pub fn security_summary(&self) -> Option<&crate::narsil::SecuritySummary> {
        self.ccg_manifest.as_ref().map(|m| &m.security_summary)
    }

    /// Get symbol count from CCG manifest if available.
    #[must_use]
    pub fn symbol_count(&self) -> Option<u32> {
        self.ccg_manifest.as_ref().map(|m| m.symbol_count)
    }

    /// Get public API symbols from CCG architecture if available.
    #[must_use]
    pub fn public_api(&self) -> Option<&[crate::narsil::PublicSymbol]> {
        self.ccg_architecture.as_ref().map(|a| a.public_api.as_slice())
    }

    /// Get module hierarchy from CCG architecture if available.
    #[must_use]
    pub fn modules(&self) -> Option<&[crate::narsil::Module]> {
        self.ccg_architecture.as_ref().map(|a| a.modules.as_slice())
    }

    /// Add CCG constraints.
    #[must_use]
    pub fn with_constraints(mut self, constraints: ConstraintSet) -> Self {
        self.constraints = constraints;
        self
    }

    /// Check if there are any active constraints.
    #[must_use]
    pub fn has_constraints(&self) -> bool {
        !self.constraints.is_empty()
    }

    /// Add constraint compliance result.
    ///
    /// # Example
    ///
    /// ```rust
    /// use ralph::prompt::context::CodeIntelligenceContext;
    /// use ralph::narsil::ComplianceResult;
    ///
    /// let intel = CodeIntelligenceContext::new()
    ///     .with_compliance_result(ComplianceResult::passed(5));
    /// assert!(intel.has_compliance_result());
    /// ```
    #[must_use]
    pub fn with_compliance_result(mut self, result: ComplianceResult) -> Self {
        self.compliance_result = Some(result);
        self
    }

    /// Check if compliance result is available.
    #[must_use]
    pub fn has_compliance_result(&self) -> bool {
        self.compliance_result.is_some()
    }

    /// Check if there are any constraint violations.
    #[must_use]
    pub fn has_violations(&self) -> bool {
        self.compliance_result
            .as_ref()
            .is_some_and(|r| !r.compliant)
    }

    /// Get the compliance result if available.
    #[must_use]
    pub fn compliance(&self) -> Option<&ComplianceResult> {
        self.compliance_result.as_ref()
    }

    /// Get constraints that apply to a specific target (function, module, etc.).
    #[must_use]
    pub fn constraints_for_target(&self, target: &str) -> Vec<&crate::narsil::CcgConstraint> {
        self.constraints.for_target(target)
    }

    /// Check if there are blocking constraints (error or critical severity).
    #[must_use]
    pub fn has_blocking_constraints(&self) -> bool {
        self.constraints.has_blocking()
    }

    /// Generate constraint warnings for a specific target.
    ///
    /// Returns formatted warning messages for any constraints that apply to the target.
    #[must_use]
    pub fn constraint_warnings_for(&self, target: &str) -> Vec<String> {
        self.constraints
            .for_target(target)
            .iter()
            .map(|c| c.to_prompt_string())
            .collect()
    }

    /// Check if there are blocking security issues (critical or high severity).
    #[must_use]
    pub fn has_blocking_security_issues(&self) -> bool {
        self.ccg_manifest
            .as_ref()
            .is_some_and(|m| m.security_summary.critical > 0 || m.security_summary.high > 0)
    }

    /// Estimate the size of intelligence payload in bytes.
    ///
    /// Used to enforce the < 1KB constraint on intelligence section.
    #[must_use]
    pub fn estimate_payload_size(&self) -> usize {
        let mut size = 0;

        // Call graph: ~50 bytes per function
        size += self.call_graph.len() * 50;

        // References: ~40 bytes per reference
        size += self.references.len() * 40;

        // Dependencies: ~60 bytes per dependency
        size += self.dependencies.len() * 60;

        // CCG manifest: ~200 bytes if present
        if self.ccg_manifest.is_some() {
            size += 200;
        }

        // CCG architecture: varies by public API size
        if let Some(arch) = &self.ccg_architecture {
            size += arch.public_api.len() * 80;
            size += arch.modules.len() * 40;
        }

        // Constraints: ~100 bytes per constraint
        size += self.constraints.len() * 100;

        size
    }
}

/// A node in the call graph representing a function and its connections.
///
/// # Example
///
/// ```
/// use ralph::prompt::context::CallGraphNode;
///
/// let node = CallGraphNode::new("process_request")
///     .with_file("src/handler.rs")
///     .with_callers(vec!["main".to_string()])
///     .with_callees(vec!["validate".to_string()]);
///
/// assert_eq!(node.connection_count(), 2);
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CallGraphNode {
    /// Name of the function.
    pub function_name: String,
    /// File containing the function.
    pub file: Option<String>,
    /// Line number of the function definition.
    pub line: Option<u32>,
    /// Functions that call this function.
    pub callers: Vec<String>,
    /// Functions called by this function.
    pub callees: Vec<String>,
}

impl CallGraphNode {
    /// Create a new call graph node.
    ///
    /// # Example
    ///
    /// ```
    /// use ralph::prompt::context::CallGraphNode;
    ///
    /// let node = CallGraphNode::new("my_function");
    /// assert_eq!(node.function_name, "my_function");
    /// ```
    #[must_use]
    pub fn new(function_name: impl Into<String>) -> Self {
        Self {
            function_name: function_name.into(),
            file: None,
            line: None,
            callers: Vec::new(),
            callees: Vec::new(),
        }
    }

    /// Set the file path.
    #[must_use]
    pub fn with_file(mut self, file: impl Into<String>) -> Self {
        self.file = Some(file.into());
        self
    }

    /// Set the line number.
    #[must_use]
    pub fn with_line(mut self, line: u32) -> Self {
        self.line = Some(line);
        self
    }

    /// Set the callers.
    #[must_use]
    pub fn with_callers(mut self, callers: Vec<String>) -> Self {
        self.callers = callers;
        self
    }

    /// Set the callees.
    #[must_use]
    pub fn with_callees(mut self, callees: Vec<String>) -> Self {
        self.callees = callees;
        self
    }

    /// Get total number of connections (callers + callees).
    #[must_use]
    pub fn connection_count(&self) -> usize {
        self.callers.len() + self.callees.len()
    }

    /// Check if this function is a hotspot (>= 5 connections).
    #[must_use]
    pub fn is_hotspot(&self) -> bool {
        self.connection_count() >= 5
    }
}

/// A reference to a symbol in the codebase.
///
/// # Example
///
/// ```
/// use ralph::prompt::context::{SymbolReference, ReferenceKind};
///
/// let reference = SymbolReference::new("MyStruct", "src/lib.rs", 42)
///     .with_kind(ReferenceKind::Definition);
///
/// assert!(reference.is_definition());
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SymbolReference {
    /// Name of the symbol.
    pub symbol: String,
    /// File containing the reference.
    pub file: String,
    /// Line number.
    pub line: u32,
    /// Column number.
    pub column: Option<u32>,
    /// Kind of reference.
    pub kind: ReferenceKind,
    /// Context around the reference (code snippet).
    pub context: Option<String>,
}

impl SymbolReference {
    /// Create a new symbol reference.
    ///
    /// # Example
    ///
    /// ```
    /// use ralph::prompt::context::SymbolReference;
    ///
    /// let reference = SymbolReference::new("foo", "lib.rs", 10);
    /// assert_eq!(reference.symbol, "foo");
    /// ```
    #[must_use]
    pub fn new(symbol: impl Into<String>, file: impl Into<String>, line: u32) -> Self {
        Self {
            symbol: symbol.into(),
            file: file.into(),
            line,
            column: None,
            kind: ReferenceKind::Unknown,
            context: None,
        }
    }

    /// Set the column.
    #[must_use]
    pub fn with_column(mut self, column: u32) -> Self {
        self.column = Some(column);
        self
    }

    /// Set the reference kind.
    #[must_use]
    pub fn with_kind(mut self, kind: ReferenceKind) -> Self {
        self.kind = kind;
        self
    }

    /// Set the context snippet.
    #[must_use]
    pub fn with_context(mut self, context: impl Into<String>) -> Self {
        self.context = Some(context.into());
        self
    }

    /// Check if this is a definition.
    #[must_use]
    pub fn is_definition(&self) -> bool {
        self.kind == ReferenceKind::Definition
    }
}

/// Kind of symbol reference.
///
/// # Example
///
/// ```
/// use ralph::prompt::context::ReferenceKind;
///
/// assert_eq!(ReferenceKind::Definition.to_string(), "definition");
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum ReferenceKind {
    /// Symbol definition.
    Definition,
    /// Symbol usage/read.
    Usage,
    /// Function call.
    Call,
    /// Import statement.
    Import,
    /// Unknown reference type.
    #[default]
    Unknown,
}

impl std::fmt::Display for ReferenceKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ReferenceKind::Definition => write!(f, "definition"),
            ReferenceKind::Usage => write!(f, "usage"),
            ReferenceKind::Call => write!(f, "call"),
            ReferenceKind::Import => write!(f, "import"),
            ReferenceKind::Unknown => write!(f, "unknown"),
        }
    }
}

/// Module dependency information.
///
/// # Example
///
/// ```
/// use ralph::prompt::context::ModuleDependency;
///
/// let dep = ModuleDependency::new("src/lib.rs")
///     .with_imports(vec!["std::io".to_string()])
///     .with_imported_by(vec!["src/main.rs".to_string()]);
///
/// assert_eq!(dep.total_connections(), 2);
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModuleDependency {
    /// Path to the module.
    pub module_path: String,
    /// Modules this module imports.
    pub imports: Vec<String>,
    /// Modules that import this module.
    pub imported_by: Vec<String>,
}

impl ModuleDependency {
    /// Create a new module dependency.
    ///
    /// # Example
    ///
    /// ```
    /// use ralph::prompt::context::ModuleDependency;
    ///
    /// let dep = ModuleDependency::new("src/lib.rs");
    /// assert_eq!(dep.module_path, "src/lib.rs");
    /// ```
    #[must_use]
    pub fn new(module_path: impl Into<String>) -> Self {
        Self {
            module_path: module_path.into(),
            imports: Vec::new(),
            imported_by: Vec::new(),
        }
    }

    /// Set imports.
    #[must_use]
    pub fn with_imports(mut self, imports: Vec<String>) -> Self {
        self.imports = imports;
        self
    }

    /// Set imported_by.
    #[must_use]
    pub fn with_imported_by(mut self, imported_by: Vec<String>) -> Self {
        self.imported_by = imported_by;
        self
    }

    /// Get total number of connections.
    #[must_use]
    pub fn total_connections(&self) -> usize {
        self.imports.len() + self.imported_by.len()
    }
}

/// Aggregates errors by code, merging duplicates.
///
/// # Example
///
/// ```
/// use ralph::prompt::context::{ErrorAggregator, ErrorContext, ErrorSeverity};
///
/// let mut aggregator = ErrorAggregator::new();
/// aggregator.add(ErrorContext::new("E0308", "mismatched types", ErrorSeverity::Error));
/// aggregator.add(ErrorContext::new("E0308", "mismatched types", ErrorSeverity::Error));
///
/// let errors = aggregator.into_vec();
/// assert_eq!(errors.len(), 1);
/// assert_eq!(errors[0].occurrence_count, 2);
/// ```
#[derive(Debug, Default, Clone)]
pub struct ErrorAggregator {
    errors: HashMap<String, ErrorContext>,
}

impl ErrorAggregator {
    /// Create a new error aggregator.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Add an error, merging with existing if same code.
    pub fn add(&mut self, error: ErrorContext) {
        self.errors
            .entry(error.code.clone())
            .and_modify(|e| e.increment())
            .or_insert(error);
    }

    /// Get the number of unique error codes.
    #[must_use]
    pub fn unique_count(&self) -> usize {
        self.errors.len()
    }

    /// Get the total occurrence count across all errors.
    #[must_use]
    pub fn total_occurrences(&self) -> u32 {
        self.errors.values().map(|e| e.occurrence_count).sum()
    }

    /// Convert to a vector of errors.
    #[must_use]
    pub fn into_vec(self) -> Vec<ErrorContext> {
        self.errors.into_values().collect()
    }

    /// Get errors sorted by occurrence count.
    #[must_use]
    pub fn sorted_by_frequency(self) -> Vec<ErrorContext> {
        let mut errors: Vec<_> = self.errors.into_values().collect();
        errors.sort_by(|a, b| b.occurrence_count.cmp(&a.occurrence_count));
        errors
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // PromptContext tests

    #[test]
    fn test_prompt_context_new() {
        let context = PromptContext::new();
        assert!(context.current_task.is_none());
        assert!(context.errors.is_empty());
        assert!(context.attempt_summaries.is_empty());
        assert!(context.anti_patterns.is_empty());
    }

    #[test]
    fn test_prompt_context_with_current_task() {
        let task = CurrentTaskContext::new("1.1", "Test task", TaskPhase::Implementation);
        let context = PromptContext::new().with_current_task(task);
        assert!(context.current_task.is_some());
        assert_eq!(context.current_task.unwrap().task_id, "1.1");
    }

    #[test]
    fn test_prompt_context_with_errors() {
        let error1 = ErrorContext::new("E0308", "type mismatch", ErrorSeverity::Error);
        let error2 = ErrorContext::new("E0433", "unresolved", ErrorSeverity::Error);
        let context = PromptContext::new()
            .with_error(error1)
            .with_error(error2);
        assert_eq!(context.errors.len(), 2);
    }

    #[test]
    fn test_prompt_context_has_critical_issues() {
        let context = PromptContext::new();
        assert!(!context.has_critical_issues());

        let context_with_error =
            PromptContext::new().with_error(ErrorContext::new("E0308", "err", ErrorSeverity::Error));
        assert!(context_with_error.has_critical_issues());
    }

    #[test]
    fn test_prompt_context_errors_by_frequency() {
        let error1 =
            ErrorContext::new("E0308", "type mismatch", ErrorSeverity::Error).with_occurrences(5);
        let error2 =
            ErrorContext::new("E0433", "unresolved", ErrorSeverity::Error).with_occurrences(2);
        let error3 =
            ErrorContext::new("E0599", "method not found", ErrorSeverity::Error).with_occurrences(10);

        let context = PromptContext::new().with_errors(vec![error1, error2, error3]);
        let sorted = context.errors_by_frequency();

        assert_eq!(sorted[0].code, "E0599"); // 10 occurrences
        assert_eq!(sorted[1].code, "E0308"); // 5 occurrences
        assert_eq!(sorted[2].code, "E0433"); // 2 occurrences
    }

    // CurrentTaskContext tests

    #[test]
    fn test_current_task_context_new() {
        let task = CurrentTaskContext::new("2.1", "Create context", TaskPhase::Testing);
        assert_eq!(task.task_id, "2.1");
        assert_eq!(task.title, "Create context");
        assert_eq!(task.phase, TaskPhase::Testing);
        assert_eq!(task.completion_percentage, 0);
        assert_eq!(task.attempt_count, 0);
    }

    #[test]
    fn test_current_task_context_builders() {
        let task = CurrentTaskContext::new("1.1", "Task", TaskPhase::Implementation)
            .with_completion(75)
            .with_attempts(3)
            .with_modified_files(vec!["src/lib.rs".to_string()])
            .with_blockers(vec!["Dependency missing".to_string()])
            .with_dependencies(vec!["0.1".to_string()]);

        assert_eq!(task.completion_percentage, 75);
        assert_eq!(task.attempt_count, 3);
        assert_eq!(task.modified_files.len(), 1);
        assert!(task.is_blocked());
        assert!(task.has_dependencies());
    }

    #[test]
    fn test_current_task_context_completion_capped() {
        let task = CurrentTaskContext::new("1.1", "Task", TaskPhase::Implementation)
            .with_completion(150); // Over 100
        assert_eq!(task.completion_percentage, 100);
    }

    // TaskPhase tests

    #[test]
    fn test_task_phase_display() {
        assert_eq!(TaskPhase::Planning.to_string(), "Planning");
        assert_eq!(TaskPhase::Implementation.to_string(), "Implementation");
        assert_eq!(TaskPhase::Testing.to_string(), "Testing");
        assert_eq!(TaskPhase::QualityFixes.to_string(), "Quality Fixes");
        assert_eq!(TaskPhase::Review.to_string(), "Review");
    }

    // ErrorContext tests

    #[test]
    fn test_error_context_new() {
        let error = ErrorContext::new("E0308", "mismatched types", ErrorSeverity::Error);
        assert_eq!(error.code, "E0308");
        assert_eq!(error.message, "mismatched types");
        assert_eq!(error.severity, ErrorSeverity::Error);
        assert_eq!(error.occurrence_count, 1);
    }

    #[test]
    fn test_error_context_with_location() {
        let error = ErrorContext::new("E0308", "error", ErrorSeverity::Error)
            .with_location("src/lib.rs", 42);
        assert_eq!(error.file, Some("src/lib.rs".to_string()));
        assert_eq!(error.line, Some(42));
    }

    #[test]
    fn test_error_context_increment() {
        let mut error = ErrorContext::new("E0308", "error", ErrorSeverity::Error);
        assert_eq!(error.occurrence_count, 1);
        error.increment();
        assert_eq!(error.occurrence_count, 2);
        assert!(error.is_recurring());
    }

    #[test]
    fn test_error_context_is_critical() {
        let error = ErrorContext::new("E0308", "error", ErrorSeverity::Error);
        assert!(error.is_critical());

        let warning = ErrorContext::new("W0001", "warning", ErrorSeverity::Warning);
        assert!(!warning.is_critical());
    }

    // ErrorSeverity tests

    #[test]
    fn test_error_severity_ordering() {
        assert!(ErrorSeverity::Info < ErrorSeverity::Warning);
        assert!(ErrorSeverity::Warning < ErrorSeverity::Error);
    }

    #[test]
    fn test_error_severity_display() {
        assert_eq!(ErrorSeverity::Info.to_string(), "info");
        assert_eq!(ErrorSeverity::Warning.to_string(), "warning");
        assert_eq!(ErrorSeverity::Error.to_string(), "error");
    }

    // QualityGateStatus tests

    #[test]
    fn test_quality_gate_status_new() {
        let status = QualityGateStatus::new();
        assert!(!status.clippy.passed);
        assert!(!status.tests.passed);
    }

    #[test]
    fn test_quality_gate_status_all_passing() {
        let status = QualityGateStatus::all_passing();
        assert!(status.all_passed());
        assert!(status.last_check.is_some());
    }

    #[test]
    fn test_quality_gate_status_failing_gates() {
        let status = QualityGateStatus::new()
            .with_clippy(GateResult::pass())
            .with_tests(GateResult::fail(vec!["test failed".to_string()]))
            .with_no_allow(GateResult::pass())
            .with_security(GateResult::fail(vec!["vuln found".to_string()]))
            .with_docs(GateResult::pass());

        let failing = status.failing_gates();
        assert_eq!(failing.len(), 2);
        assert!(failing.contains(&"tests"));
        assert!(failing.contains(&"security"));
    }

    #[test]
    fn test_quality_gate_status_failure_count() {
        let status = QualityGateStatus::new()
            .with_clippy(GateResult::fail(vec!["w1".into(), "w2".into()]))
            .with_tests(GateResult::pass())
            .with_no_allow(GateResult::pass())
            .with_security(GateResult::pass())
            .with_docs(GateResult::pass());

        assert_eq!(status.failure_count(), 2);
    }

    // GateResult tests

    #[test]
    fn test_gate_result_pass() {
        let result = GateResult::pass();
        assert!(result.passed);
        assert!(result.messages.is_empty());
    }

    #[test]
    fn test_gate_result_fail() {
        let result = GateResult::fail(vec!["error 1".into(), "error 2".into()]);
        assert!(!result.passed);
        assert_eq!(result.messages.len(), 2);
    }

    #[test]
    fn test_gate_result_pass_with_warnings() {
        let result = GateResult::pass_with_warnings(vec!["warning".into()]);
        assert!(result.passed);
        assert_eq!(result.messages.len(), 1);
    }

    // SessionStats tests

    #[test]
    fn test_session_stats_new() {
        let stats = SessionStats::new(10, 3, 500);
        assert_eq!(stats.iteration_count, 10);
        assert_eq!(stats.commit_count, 3);
        assert_eq!(stats.lines_changed, 500);
    }

    #[test]
    fn test_session_stats_budget_used_percent() {
        let stats = SessionStats::new(5, 2, 100).with_budget(10);
        assert_eq!(stats.budget_used_percent(), Some(50));
    }

    #[test]
    fn test_session_stats_budget_critical() {
        let stats = SessionStats::new(9, 2, 100).with_budget(10);
        assert!(stats.is_budget_critical());

        let stats2 = SessionStats::new(5, 2, 100).with_budget(10);
        assert!(!stats2.is_budget_critical());
    }

    #[test]
    fn test_session_stats_is_progressing() {
        // Good progress: 3 commits in 9 iterations (1 commit per 3 iterations)
        let good = SessionStats::new(9, 3, 100);
        assert!(good.is_progressing());

        // Poor progress: 1 commit in 10 iterations
        let poor = SessionStats::new(10, 1, 100);
        assert!(!poor.is_progressing());
    }

    // AttemptSummary tests

    #[test]
    fn test_attempt_summary_new() {
        let attempt = AttemptSummary::new(1, AttemptOutcome::Success);
        assert_eq!(attempt.attempt_number, 1);
        assert!(attempt.outcome.is_success());
    }

    #[test]
    fn test_attempt_summary_builders() {
        let attempt = AttemptSummary::new(2, AttemptOutcome::TestFailure)
            .with_approach("TDD")
            .with_errors(vec!["test_foo failed".into()])
            .with_files(vec!["src/lib.rs".into()])
            .with_duration(300)
            .with_notes("Need to fix assertion");

        assert_eq!(attempt.approach, Some("TDD".to_string()));
        assert_eq!(attempt.errors.len(), 1);
        assert_eq!(attempt.files_modified.len(), 1);
        assert_eq!(attempt.duration_seconds, Some(300));
        assert!(attempt.notes.is_some());
    }

    // AttemptOutcome tests

    #[test]
    fn test_attempt_outcome_is_success() {
        assert!(AttemptOutcome::Success.is_success());
        assert!(!AttemptOutcome::TestFailure.is_success());
        assert!(!AttemptOutcome::Blocked.is_success());
    }

    #[test]
    fn test_attempt_outcome_is_recoverable() {
        assert!(AttemptOutcome::CompilationError.is_recoverable());
        assert!(AttemptOutcome::TestFailure.is_recoverable());
        assert!(AttemptOutcome::QualityGateFailed.is_recoverable());
        assert!(!AttemptOutcome::Timeout.is_recoverable());
        assert!(!AttemptOutcome::Blocked.is_recoverable());
    }

    #[test]
    fn test_attempt_outcome_display() {
        assert_eq!(AttemptOutcome::Success.to_string(), "Success");
        assert_eq!(AttemptOutcome::CompilationError.to_string(), "Compilation Error");
    }

    // AntiPattern tests

    #[test]
    fn test_anti_pattern_new() {
        let pattern = AntiPattern::new(
            AntiPatternType::EditWithoutCommit,
            "Edited 5 files without committing",
        );
        assert_eq!(pattern.pattern_type, AntiPatternType::EditWithoutCommit);
        assert_eq!(pattern.severity, AntiPatternSeverity::Medium);
        assert_eq!(pattern.persistence_count, 1);
    }

    #[test]
    fn test_anti_pattern_builders() {
        let pattern = AntiPattern::new(AntiPatternType::RepeatingErrors, "Same error 3 times")
            .with_evidence(vec!["E0308 at line 10".into()])
            .with_severity(AntiPatternSeverity::High)
            .with_remediation("Fix the type mismatch")
            .with_persistence(3);

        assert_eq!(pattern.evidence.len(), 1);
        assert_eq!(pattern.severity, AntiPatternSeverity::High);
        assert!(pattern.remediation.is_some());
        assert_eq!(pattern.persistence_count, 3);
    }

    // AntiPatternType tests

    #[test]
    fn test_anti_pattern_type_default_severity() {
        assert_eq!(
            AntiPatternType::TaskOscillation.default_severity(),
            AntiPatternSeverity::High
        );
        assert_eq!(
            AntiPatternType::ClippyNotRun.default_severity(),
            AntiPatternSeverity::Low
        );
        assert_eq!(
            AntiPatternType::EditWithoutCommit.default_severity(),
            AntiPatternSeverity::Medium
        );
    }

    #[test]
    fn test_anti_pattern_type_display() {
        assert_eq!(AntiPatternType::EditWithoutCommit.to_string(), "Edit Without Commit");
        assert_eq!(AntiPatternType::TestsNotRun.to_string(), "Tests Not Run");
        assert_eq!(AntiPatternType::RepeatingErrors.to_string(), "Repeating Errors");
    }

    // AntiPatternSeverity tests

    #[test]
    fn test_anti_pattern_severity_ordering() {
        assert!(AntiPatternSeverity::Low < AntiPatternSeverity::Medium);
        assert!(AntiPatternSeverity::Medium < AntiPatternSeverity::High);
    }

    #[test]
    fn test_anti_pattern_severity_display() {
        assert_eq!(AntiPatternSeverity::Low.to_string(), "Low");
        assert_eq!(AntiPatternSeverity::Medium.to_string(), "Medium");
        assert_eq!(AntiPatternSeverity::High.to_string(), "High");
    }

    // ErrorAggregator tests

    #[test]
    fn test_error_aggregator_new() {
        let aggregator = ErrorAggregator::new();
        assert_eq!(aggregator.unique_count(), 0);
    }

    #[test]
    fn test_error_aggregator_merges_duplicates() {
        let mut aggregator = ErrorAggregator::new();
        aggregator.add(ErrorContext::new("E0308", "type mismatch", ErrorSeverity::Error));
        aggregator.add(ErrorContext::new("E0308", "type mismatch", ErrorSeverity::Error));
        aggregator.add(ErrorContext::new("E0433", "unresolved", ErrorSeverity::Error));

        assert_eq!(aggregator.unique_count(), 2);
        assert_eq!(aggregator.total_occurrences(), 3);
    }

    #[test]
    fn test_error_aggregator_sorted_by_frequency() {
        let mut aggregator = ErrorAggregator::new();
        // Add E0308 twice
        aggregator.add(ErrorContext::new("E0308", "error", ErrorSeverity::Error));
        aggregator.add(ErrorContext::new("E0308", "error", ErrorSeverity::Error));
        // Add E0433 once
        aggregator.add(ErrorContext::new("E0433", "error", ErrorSeverity::Error));
        // Add E0599 three times
        aggregator.add(ErrorContext::new("E0599", "error", ErrorSeverity::Error));
        aggregator.add(ErrorContext::new("E0599", "error", ErrorSeverity::Error));
        aggregator.add(ErrorContext::new("E0599", "error", ErrorSeverity::Error));

        let sorted = aggregator.sorted_by_frequency();
        assert_eq!(sorted[0].code, "E0599"); // 3
        assert_eq!(sorted[1].code, "E0308"); // 2
        assert_eq!(sorted[2].code, "E0433"); // 1
    }

    // Serialization tests

    #[test]
    fn test_prompt_context_serialize() {
        let context = PromptContext::new()
            .with_session_stats(SessionStats::new(5, 2, 100));

        let json = serde_json::to_string(&context).unwrap();
        assert!(json.contains("\"iteration_count\":5"));
    }

    #[test]
    fn test_prompt_context_deserialize() {
        let json = r#"{
            "current_task": null,
            "errors": [],
            "quality_status": {
                "clippy": {"passed": true, "messages": []},
                "tests": {"passed": true, "messages": []},
                "no_allow": {"passed": true, "messages": []},
                "security": {"passed": true, "messages": []},
                "docs": {"passed": true, "messages": []},
                "last_check": null
            },
            "session_stats": {
                "iteration_count": 10,
                "commit_count": 5,
                "lines_changed": 200,
                "tasks_completed": 2,
                "tasks_blocked": 0,
                "stagnation_count": 0,
                "max_iterations": null,
                "files_modified": [],
                "test_delta": 0
            },
            "attempt_summaries": [],
            "anti_patterns": [],
            "code_intelligence": {
                "call_graph": [],
                "references": [],
                "dependencies": [],
                "is_available": false
            }
        }"#;

        let context: PromptContext = serde_json::from_str(json).unwrap();
        assert_eq!(context.session_stats.iteration_count, 10);
        assert!(context.quality_status.all_passed());
    }

    #[test]
    fn test_error_context_serialize_roundtrip() {
        let error = ErrorContext::new("E0308", "mismatched types", ErrorSeverity::Error)
            .with_location("src/lib.rs", 42)
            .with_occurrences(3)
            .with_suggested_fix("Change type to String");

        let json = serde_json::to_string(&error).unwrap();
        let restored: ErrorContext = serde_json::from_str(&json).unwrap();

        assert_eq!(restored.code, "E0308");
        assert_eq!(restored.occurrence_count, 3);
        assert_eq!(restored.file, Some("src/lib.rs".to_string()));
    }

    #[test]
    fn test_anti_pattern_serialize_roundtrip() {
        let pattern = AntiPattern::new(AntiPatternType::TaskOscillation, "Switching tasks")
            .with_evidence(vec!["Task 1 -> Task 2 -> Task 1".into()])
            .with_persistence(5);

        let json = serde_json::to_string(&pattern).unwrap();
        let restored: AntiPattern = serde_json::from_str(&json).unwrap();

        assert_eq!(restored.pattern_type, AntiPatternType::TaskOscillation);
        assert_eq!(restored.persistence_count, 5);
    }

    // ==========================================================================
    // CodeIntelligenceContext tests
    // ==========================================================================

    #[test]
    fn test_code_intelligence_context_new() {
        let intel = CodeIntelligenceContext::new();
        assert!(intel.call_graph.is_empty());
        assert!(intel.references.is_empty());
        assert!(intel.dependencies.is_empty());
        assert!(!intel.is_available);
    }

    #[test]
    fn test_code_intelligence_context_with_call_graph() {
        let nodes = vec![
            CallGraphNode::new("process_request")
                .with_callers(vec!["handle_http".to_string()])
                .with_callees(vec!["validate_input".to_string(), "execute".to_string()]),
        ];
        let intel = CodeIntelligenceContext::new()
            .with_call_graph(nodes)
            .mark_available();

        assert_eq!(intel.call_graph.len(), 1);
        assert_eq!(intel.call_graph[0].function_name, "process_request");
        assert!(intel.is_available);
    }

    #[test]
    fn test_code_intelligence_context_with_references() {
        let refs = vec![
            SymbolReference::new("MyStruct", "src/lib.rs", 42)
                .with_kind(ReferenceKind::Definition),
            SymbolReference::new("MyStruct", "src/main.rs", 10)
                .with_kind(ReferenceKind::Usage),
        ];
        let intel = CodeIntelligenceContext::new().with_references(refs);

        assert_eq!(intel.references.len(), 2);
        assert_eq!(intel.references[0].symbol, "MyStruct");
    }

    #[test]
    fn test_code_intelligence_context_with_dependencies() {
        let deps = vec![
            ModuleDependency::new("src/lib.rs")
                .with_imports(vec!["std::io".to_string(), "crate::util".to_string()])
                .with_imported_by(vec!["src/main.rs".to_string()]),
        ];
        let intel = CodeIntelligenceContext::new().with_dependencies(deps);

        assert_eq!(intel.dependencies.len(), 1);
        assert_eq!(intel.dependencies[0].imports.len(), 2);
    }

    #[test]
    fn test_code_intelligence_context_has_data() {
        let empty = CodeIntelligenceContext::new();
        assert!(!empty.has_data());

        let with_graph = CodeIntelligenceContext::new()
            .with_call_graph(vec![CallGraphNode::new("foo")]);
        assert!(with_graph.has_data());
    }

    #[test]
    fn test_code_intelligence_context_relevant_functions_count() {
        let intel = CodeIntelligenceContext::new()
            .with_call_graph(vec![
                CallGraphNode::new("foo"),
                CallGraphNode::new("bar"),
            ]);
        assert_eq!(intel.relevant_functions_count(), 2);
    }

    // ==========================================================================
    // CallGraphNode tests
    // ==========================================================================

    #[test]
    fn test_call_graph_node_new() {
        let node = CallGraphNode::new("process_data");
        assert_eq!(node.function_name, "process_data");
        assert!(node.callers.is_empty());
        assert!(node.callees.is_empty());
        assert!(node.file.is_none());
    }

    #[test]
    fn test_call_graph_node_builders() {
        let node = CallGraphNode::new("process_data")
            .with_file("src/processor.rs")
            .with_line(42)
            .with_callers(vec!["main".to_string(), "handle_request".to_string()])
            .with_callees(vec!["validate".to_string()]);

        assert_eq!(node.file, Some("src/processor.rs".to_string()));
        assert_eq!(node.line, Some(42));
        assert_eq!(node.callers.len(), 2);
        assert_eq!(node.callees.len(), 1);
    }

    #[test]
    fn test_call_graph_node_connection_count() {
        let node = CallGraphNode::new("foo")
            .with_callers(vec!["a".to_string(), "b".to_string()])
            .with_callees(vec!["c".to_string()]);

        assert_eq!(node.connection_count(), 3);
    }

    #[test]
    fn test_call_graph_node_is_hotspot() {
        let low_connections = CallGraphNode::new("foo")
            .with_callers(vec!["a".to_string()]);
        assert!(!low_connections.is_hotspot());

        let high_connections = CallGraphNode::new("bar")
            .with_callers(vec!["a".into(), "b".into(), "c".into(), "d".into(), "e".into()])
            .with_callees(vec!["x".into()]);
        assert!(high_connections.is_hotspot());
    }

    // ==========================================================================
    // SymbolReference tests
    // ==========================================================================

    #[test]
    fn test_symbol_reference_new() {
        let reference = SymbolReference::new("MyStruct", "src/lib.rs", 42);
        assert_eq!(reference.symbol, "MyStruct");
        assert_eq!(reference.file, "src/lib.rs");
        assert_eq!(reference.line, 42);
        assert_eq!(reference.kind, ReferenceKind::Unknown);
    }

    #[test]
    fn test_symbol_reference_builders() {
        let reference = SymbolReference::new("foo", "src/lib.rs", 10)
            .with_column(5)
            .with_kind(ReferenceKind::Call)
            .with_context("let result = foo();");

        assert_eq!(reference.column, Some(5));
        assert_eq!(reference.kind, ReferenceKind::Call);
        assert_eq!(reference.context, Some("let result = foo();".to_string()));
    }

    #[test]
    fn test_symbol_reference_is_definition() {
        let def = SymbolReference::new("foo", "lib.rs", 1)
            .with_kind(ReferenceKind::Definition);
        assert!(def.is_definition());

        let usage = SymbolReference::new("foo", "main.rs", 10)
            .with_kind(ReferenceKind::Usage);
        assert!(!usage.is_definition());
    }

    // ==========================================================================
    // ReferenceKind tests
    // ==========================================================================

    #[test]
    fn test_reference_kind_display() {
        assert_eq!(ReferenceKind::Definition.to_string(), "definition");
        assert_eq!(ReferenceKind::Usage.to_string(), "usage");
        assert_eq!(ReferenceKind::Call.to_string(), "call");
        assert_eq!(ReferenceKind::Import.to_string(), "import");
        assert_eq!(ReferenceKind::Unknown.to_string(), "unknown");
    }

    // ==========================================================================
    // ModuleDependency tests
    // ==========================================================================

    #[test]
    fn test_module_dependency_new() {
        let dep = ModuleDependency::new("src/lib.rs");
        assert_eq!(dep.module_path, "src/lib.rs");
        assert!(dep.imports.is_empty());
        assert!(dep.imported_by.is_empty());
    }

    #[test]
    fn test_module_dependency_builders() {
        let dep = ModuleDependency::new("src/lib.rs")
            .with_imports(vec!["std::io".to_string(), "crate::util".to_string()])
            .with_imported_by(vec!["src/main.rs".to_string()]);

        assert_eq!(dep.imports.len(), 2);
        assert_eq!(dep.imported_by.len(), 1);
    }

    #[test]
    fn test_module_dependency_total_connections() {
        let dep = ModuleDependency::new("mod.rs")
            .with_imports(vec!["a".into(), "b".into()])
            .with_imported_by(vec!["c".into()]);

        assert_eq!(dep.total_connections(), 3);
    }

    // ==========================================================================
    // PromptContext with CodeIntelligence tests
    // ==========================================================================

    #[test]
    fn test_prompt_context_with_code_intelligence() {
        let intel = CodeIntelligenceContext::new()
            .with_call_graph(vec![CallGraphNode::new("foo")])
            .mark_available();

        let context = PromptContext::new().with_code_intelligence(intel);

        assert!(context.code_intelligence.is_available);
        assert!(context.code_intelligence.has_data());
    }

    // ==========================================================================
    // Serialization tests for CodeIntelligence types
    // ==========================================================================

    #[test]
    fn test_call_graph_node_serialize_roundtrip() {
        let node = CallGraphNode::new("process")
            .with_file("src/lib.rs")
            .with_callers(vec!["main".to_string()]);

        let json = serde_json::to_string(&node).unwrap();
        let restored: CallGraphNode = serde_json::from_str(&json).unwrap();

        assert_eq!(restored.function_name, "process");
        assert_eq!(restored.callers.len(), 1);
    }

    #[test]
    fn test_code_intelligence_context_serialize_roundtrip() {
        let intel = CodeIntelligenceContext::new()
            .with_call_graph(vec![CallGraphNode::new("foo")])
            .with_references(vec![SymbolReference::new("Bar", "lib.rs", 10)])
            .mark_available();

        let json = serde_json::to_string(&intel).unwrap();
        let restored: CodeIntelligenceContext = serde_json::from_str(&json).unwrap();

        assert!(restored.is_available);
        assert_eq!(restored.call_graph.len(), 1);
        assert_eq!(restored.references.len(), 1);
    }

    // ==========================================================================
    // CCG integration tests
    // ==========================================================================

    #[test]
    fn test_code_intelligence_context_with_ccg_manifest() {
        use crate::narsil::{CcgManifest, SecuritySummary};

        let manifest = CcgManifest::new("test-repo", "/path/to/repo")
            .with_counts(100, 500)
            .with_security_summary(SecuritySummary {
                critical: 0,
                high: 1,
                medium: 5,
                low: 10,
            });

        let intel = CodeIntelligenceContext::new()
            .with_ccg_manifest(manifest)
            .mark_available();

        assert!(intel.has_data());
        assert!(intel.has_ccg_data());
        assert_eq!(intel.symbol_count(), Some(500));
        assert!(intel.has_blocking_security_issues()); // high > 0
    }

    #[test]
    fn test_code_intelligence_context_with_ccg_architecture() {
        use crate::narsil::{CcgArchitecture, PublicSymbol, SymbolKind, Module};

        let arch = CcgArchitecture::new()
            .with_public_symbol(PublicSymbol::new("process_request", SymbolKind::Function))
            .with_public_symbol(PublicSymbol::new("MyStruct", SymbolKind::Struct))
            .with_module(Module::new("lib", "src/lib.rs"));

        let intel = CodeIntelligenceContext::new()
            .with_ccg_architecture(arch)
            .mark_available();

        assert!(intel.has_data());
        assert!(intel.has_ccg_data());
        assert_eq!(intel.public_api().map(|api| api.len()), Some(2));
        assert_eq!(intel.modules().map(|m| m.len()), Some(1));
    }

    #[test]
    fn test_code_intelligence_context_security_summary() {
        use crate::narsil::{CcgManifest, SecuritySummary};

        // No security issues
        let safe_manifest = CcgManifest::new("safe-repo", "/path")
            .with_security_summary(SecuritySummary {
                critical: 0,
                high: 0,
                medium: 2,
                low: 5,
            });

        let safe_intel = CodeIntelligenceContext::new()
            .with_ccg_manifest(safe_manifest);

        assert!(!safe_intel.has_blocking_security_issues());

        // Critical issues
        let unsafe_manifest = CcgManifest::new("unsafe-repo", "/path")
            .with_security_summary(SecuritySummary {
                critical: 1,
                high: 0,
                medium: 0,
                low: 0,
            });

        let unsafe_intel = CodeIntelligenceContext::new()
            .with_ccg_manifest(unsafe_manifest);

        assert!(unsafe_intel.has_blocking_security_issues());
    }

    #[test]
    fn test_code_intelligence_context_estimate_payload_size() {
        use crate::narsil::{CcgManifest, CcgArchitecture, PublicSymbol, SymbolKind, Module};

        // Empty context
        let empty = CodeIntelligenceContext::new();
        assert_eq!(empty.estimate_payload_size(), 0);

        // With call graph
        let with_graph = CodeIntelligenceContext::new()
            .with_call_graph(vec![CallGraphNode::new("foo"), CallGraphNode::new("bar")]);
        assert_eq!(with_graph.estimate_payload_size(), 100); // 2 * 50

        // With CCG data
        let with_ccg = CodeIntelligenceContext::new()
            .with_ccg_manifest(CcgManifest::new("test", "/path"))
            .with_ccg_architecture(
                CcgArchitecture::new()
                    .with_public_symbol(PublicSymbol::new("foo", SymbolKind::Function))
                    .with_module(Module::new("lib", "src/lib.rs"))
            );

        // 200 (manifest) + 80 (1 symbol) + 40 (1 module) = 320
        assert_eq!(with_ccg.estimate_payload_size(), 320);
    }

    #[test]
    fn test_code_intelligence_context_has_data_with_ccg() {
        use crate::narsil::CcgManifest;

        // Only CCG manifest counts as data
        let intel = CodeIntelligenceContext::new()
            .with_ccg_manifest(CcgManifest::new("test", "/path"));

        assert!(intel.has_data());
        assert!(intel.has_ccg_data());
    }

    #[test]
    fn test_code_intelligence_context_ccg_serialize_roundtrip() {
        use crate::narsil::{CcgManifest, SecuritySummary};

        let manifest = CcgManifest::new("test-repo", "/path/to/repo")
            .with_counts(50, 200)
            .with_security_summary(SecuritySummary {
                critical: 0,
                high: 0,
                medium: 1,
                low: 2,
            });

        let intel = CodeIntelligenceContext::new()
            .with_ccg_manifest(manifest)
            .mark_available();

        let json = serde_json::to_string(&intel).unwrap();
        let restored: CodeIntelligenceContext = serde_json::from_str(&json).unwrap();

        assert!(restored.is_available);
        assert!(restored.has_ccg_data());
        assert_eq!(restored.symbol_count(), Some(200));
    }

    // ==========================================================================
    // CodeIntelligenceContext Constraint Tests
    // ==========================================================================

    #[test]
    fn test_code_intelligence_context_with_constraints() {
        use crate::narsil::{CcgConstraint, ConstraintKind, ConstraintSet, ConstraintValue};

        let constraints = ConstraintSet::new()
            .with_constraint(
                CcgConstraint::new("max-complexity", ConstraintKind::MaxComplexity, "Keep it simple")
                    .with_value(ConstraintValue::Number(10)),
            );

        let intel = CodeIntelligenceContext::new().with_constraints(constraints);

        assert!(intel.has_constraints());
        assert!(intel.has_data());
    }

    #[test]
    fn test_code_intelligence_context_constraints_for_target() {
        use crate::narsil::{CcgConstraint, ConstraintKind, ConstraintSet};

        let constraints = ConstraintSet::new()
            .with_constraint(
                CcgConstraint::new("c1", ConstraintKind::MaxComplexity, "For core")
                    .with_target("core::*"),
            )
            .with_constraint(CcgConstraint::new("c2", ConstraintKind::MaxLines, "Global"));

        let intel = CodeIntelligenceContext::new().with_constraints(constraints);

        // core::process matches both wildcards and global
        let core_constraints = intel.constraints_for_target("core::process");
        assert_eq!(core_constraints.len(), 2);

        // api::handler only matches global
        let api_constraints = intel.constraints_for_target("api::handler");
        assert_eq!(api_constraints.len(), 1);
    }

    #[test]
    fn test_code_intelligence_context_has_blocking_constraints() {
        use crate::narsil::{CcgConstraint, ConstraintKind, ConstraintSet, ConstraintSeverity};

        let non_blocking = ConstraintSet::new()
            .with_constraint(CcgConstraint::new("c1", ConstraintKind::MaxComplexity, "Test"));

        let intel = CodeIntelligenceContext::new().with_constraints(non_blocking);
        assert!(!intel.has_blocking_constraints());

        let blocking = ConstraintSet::new()
            .with_constraint(
                CcgConstraint::new("c1", ConstraintKind::MaxComplexity, "Blocking")
                    .with_severity(ConstraintSeverity::Error),
            );

        let intel = CodeIntelligenceContext::new().with_constraints(blocking);
        assert!(intel.has_blocking_constraints());
    }

    #[test]
    fn test_code_intelligence_context_constraint_warnings_for() {
        use crate::narsil::{CcgConstraint, ConstraintKind, ConstraintSet, ConstraintValue};

        let constraints = ConstraintSet::new()
            .with_constraint(
                CcgConstraint::new("max-complexity", ConstraintKind::MaxComplexity, "Keep it simple")
                    .with_target("process")
                    .with_value(ConstraintValue::Number(10)),
            );

        let intel = CodeIntelligenceContext::new().with_constraints(constraints);

        let warnings = intel.constraint_warnings_for("process");
        assert_eq!(warnings.len(), 1);
        assert!(warnings[0].contains("maxComplexity"));
    }

    #[test]
    fn test_code_intelligence_context_estimate_payload_size_with_constraints() {
        use crate::narsil::{CcgConstraint, ConstraintKind, ConstraintSet};

        let constraints = ConstraintSet::new()
            .with_constraint(CcgConstraint::new("c1", ConstraintKind::MaxComplexity, "Test1"))
            .with_constraint(CcgConstraint::new("c2", ConstraintKind::MaxLines, "Test2"));

        let intel = CodeIntelligenceContext::new().with_constraints(constraints);

        // Should include constraint size (~100 bytes per constraint)
        let size = intel.estimate_payload_size();
        assert!(size >= 200);
    }

    #[test]
    fn test_code_intelligence_context_constraint_serialize_roundtrip() {
        use crate::narsil::{CcgConstraint, ConstraintKind, ConstraintSet, ConstraintValue};

        let constraints = ConstraintSet::new()
            .with_constraint(
                CcgConstraint::new("max-complexity", ConstraintKind::MaxComplexity, "Keep it simple")
                    .with_value(ConstraintValue::Number(10)),
            );

        let intel = CodeIntelligenceContext::new()
            .with_constraints(constraints)
            .mark_available();

        let json = serde_json::to_string(&intel).unwrap();
        let restored: CodeIntelligenceContext = serde_json::from_str(&json).unwrap();

        assert!(restored.is_available);
        assert!(restored.has_constraints());
        assert_eq!(restored.constraints.len(), 1);
    }
}
