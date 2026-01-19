//! Intelligent retry system for handling failures.
//!
//! This module provides failure classification, recovery strategies, and
//! intelligent retry logic to improve recovery from errors during automation.
//!
//! # Architecture
//!
//! ```text
//! FailureContext ──classify──> FailureClass ──select──> RecoveryStrategy
//!                                    │                        │
//!                                    │                        │
//!                                    ▼                        ▼
//!                           IntelligentRetry ←──────── RetryPrompt
//! ```
//!
//! # Example
//!
//! ```rust,ignore
//! use ralph::r#loop::retry::{FailureClassifier, RecoveryStrategist, IntelligentRetry};
//!
//! // Classify the failure
//! let classifier = FailureClassifier::new();
//! let failure = classifier.classify(&error_output)?;
//!
//! // Get recovery strategy
//! let strategist = RecoveryStrategist::new();
//! let strategy = strategist.select(&failure);
//!
//! // Generate retry prompt
//! let retry = IntelligentRetry::new();
//! let prompt = retry.generate_prompt(&strategy, &failure);
//! ```

use regex::Regex;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use std::time::Duration;

// ============================================================================
// Backoff Constants and Calculation
// ============================================================================

/// Base backoff delay in milliseconds for retry attempts.
pub const RETRY_BACKOFF_BASE_MS: u64 = 2000;

/// Maximum backoff delay in milliseconds.
pub const MAX_BACKOFF_MS: u64 = 30_000;

/// Multiplier for exponential backoff.
pub const BACKOFF_MULTIPLIER: u64 = 2;

/// Calculate exponential backoff delay for a given attempt number.
///
/// Uses exponential backoff with a cap at `MAX_BACKOFF_MS` to prevent
/// extremely long delays.
///
/// # Arguments
///
/// * `attempt` - The attempt number (1-indexed). First attempt uses base delay.
///
/// # Returns
///
/// A `Duration` representing the delay before the next retry.
///
/// # Example
///
/// ```rust,ignore
/// use ralph::r#loop::retry::calculate_backoff;
/// use std::time::Duration;
///
/// assert_eq!(calculate_backoff(1), Duration::from_millis(2000));  // Base
/// assert_eq!(calculate_backoff(2), Duration::from_millis(4000));  // 2x
/// assert_eq!(calculate_backoff(3), Duration::from_millis(8000));  // 4x
/// ```
#[must_use]
pub fn calculate_backoff(attempt: u32) -> Duration {
    let exponent = attempt.saturating_sub(1);
    let multiplier = BACKOFF_MULTIPLIER.saturating_pow(exponent);
    let delay = RETRY_BACKOFF_BASE_MS.saturating_mul(multiplier);
    Duration::from_millis(delay.min(MAX_BACKOFF_MS))
}

// ============================================================================
// Failure Classification
// ============================================================================

/// Classification of failure types encountered during automation.
///
/// Each class maps to a specific recovery strategy.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum FailureClass {
    /// Rust compilation error (type mismatch, borrow checker, etc.)
    CompileError,
    /// Test failure (assertion, panic in test)
    TestFailure,
    /// Clippy warning treated as error
    ClippyWarning,
    /// Missing dependency or module not found
    MissingDependency,
    /// Syntax error (parse error)
    SyntaxError,
    /// Import/use statement error
    ImportError,
    /// Lifetime or borrow checker error specifically
    LifetimeError,
    /// Trait bound not satisfied
    TraitBoundError,
    /// Type inference failed
    TypeInferenceError,
    /// Security scan finding
    SecurityFinding,
    /// Git operation failed
    GitError,
    /// External tool execution failed
    ToolError,
    /// Claude Code process failure (transient, no code change needed)
    ProcessFailure,
    /// Unknown error type
    Unknown,
}

impl FailureClass {
    /// Get a human-readable description of this failure class.
    #[must_use]
    pub fn description(&self) -> &'static str {
        match self {
            Self::CompileError => "Compilation error",
            Self::TestFailure => "Test failure",
            Self::ClippyWarning => "Clippy warning",
            Self::MissingDependency => "Missing dependency",
            Self::SyntaxError => "Syntax error",
            Self::ImportError => "Import error",
            Self::LifetimeError => "Lifetime/borrow error",
            Self::TraitBoundError => "Trait bound not satisfied",
            Self::TypeInferenceError => "Type inference failed",
            Self::SecurityFinding => "Security finding",
            Self::GitError => "Git operation failed",
            Self::ToolError => "Tool execution failed",
            Self::ProcessFailure => "Process failure",
            Self::Unknown => "Unknown error",
        }
    }

    /// Check if this failure class typically requires code changes.
    #[must_use]
    pub fn requires_code_change(&self) -> bool {
        matches!(
            self,
            Self::CompileError
                | Self::TestFailure
                | Self::ClippyWarning
                | Self::SyntaxError
                | Self::ImportError
                | Self::LifetimeError
                | Self::TraitBoundError
                | Self::TypeInferenceError
                | Self::SecurityFinding
        )
    }

    /// Check if this failure class might be transient.
    #[must_use]
    pub fn may_be_transient(&self) -> bool {
        matches!(self, Self::GitError | Self::ToolError | Self::ProcessFailure)
    }

    /// Estimate complexity of fixing this failure class (1-5).
    #[must_use]
    pub fn complexity_estimate(&self) -> u8 {
        match self {
            Self::SyntaxError => 1,
            Self::ImportError => 1,
            Self::ProcessFailure => 1,
            Self::MissingDependency => 2,
            Self::ClippyWarning => 2,
            Self::CompileError => 3,
            Self::TestFailure => 3,
            Self::TypeInferenceError => 3,
            Self::TraitBoundError => 4,
            Self::LifetimeError => 4,
            Self::SecurityFinding => 4,
            Self::GitError => 2,
            Self::ToolError => 2,
            Self::Unknown => 5,
        }
    }
}

impl std::fmt::Display for FailureClass {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.description())
    }
}

/// Location where a failure occurred.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FailureLocation {
    /// File path where the error occurred.
    pub file: PathBuf,
    /// Line number (1-indexed).
    pub line: Option<u32>,
    /// Column number (1-indexed).
    pub column: Option<u32>,
}

impl FailureLocation {
    /// Create a new failure location.
    pub fn new(file: impl Into<PathBuf>) -> Self {
        Self {
            file: file.into(),
            line: None,
            column: None,
        }
    }

    /// Add line number.
    #[must_use]
    pub fn with_line(mut self, line: u32) -> Self {
        self.line = Some(line);
        self
    }

    /// Add column number.
    #[must_use]
    pub fn with_column(mut self, column: u32) -> Self {
        self.column = Some(column);
        self
    }

    /// Format as "file:line:column" string.
    #[must_use]
    pub fn format(&self) -> String {
        let mut s = self.file.display().to_string();
        if let Some(line) = self.line {
            s.push(':');
            s.push_str(&line.to_string());
            if let Some(col) = self.column {
                s.push(':');
                s.push_str(&col.to_string());
            }
        }
        s
    }
}

impl std::fmt::Display for FailureLocation {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.format())
    }
}

/// Detailed context about a failure.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FailureContext {
    /// Classification of the failure.
    pub class: FailureClass,
    /// Primary error message.
    pub message: String,
    /// Error code if available (e.g., "E0308", "clippy::unwrap_used").
    pub code: Option<String>,
    /// Location where the failure occurred.
    pub location: Option<FailureLocation>,
    /// Raw error output for reference.
    pub raw_output: String,
    /// Suggested fix from compiler/tool.
    pub suggestion: Option<String>,
    /// Related errors (e.g., caused by / causes).
    pub related: Vec<String>,
    /// Number of times this exact failure has occurred.
    pub occurrence_count: u32,
    /// Labels/tags for grouping similar failures.
    pub labels: Vec<String>,
}

impl FailureContext {
    /// Create a new failure context.
    pub fn new(class: FailureClass, message: impl Into<String>) -> Self {
        Self {
            class,
            message: message.into(),
            code: None,
            location: None,
            raw_output: String::new(),
            suggestion: None,
            related: Vec::new(),
            occurrence_count: 1,
            labels: Vec::new(),
        }
    }

    /// Add an error code.
    #[must_use]
    pub fn with_code(mut self, code: impl Into<String>) -> Self {
        self.code = Some(code.into());
        self
    }

    /// Add a location.
    #[must_use]
    pub fn with_location(mut self, location: FailureLocation) -> Self {
        self.location = Some(location);
        self
    }

    /// Add raw output.
    #[must_use]
    pub fn with_raw_output(mut self, output: impl Into<String>) -> Self {
        self.raw_output = output.into();
        self
    }

    /// Add a suggested fix.
    #[must_use]
    pub fn with_suggestion(mut self, suggestion: impl Into<String>) -> Self {
        self.suggestion = Some(suggestion.into());
        self
    }

    /// Add a related error message.
    #[must_use]
    pub fn with_related(mut self, related: impl Into<String>) -> Self {
        self.related.push(related.into());
        self
    }

    /// Increment occurrence count.
    pub fn increment_occurrence(&mut self) {
        self.occurrence_count += 1;
    }

    /// Add a label.
    #[must_use]
    pub fn with_label(mut self, label: impl Into<String>) -> Self {
        self.labels.push(label.into());
        self
    }

    /// Check if this failure is recurring (seen multiple times).
    #[must_use]
    pub fn is_recurring(&self) -> bool {
        self.occurrence_count > 1
    }

    /// Get a brief summary suitable for logging.
    #[must_use]
    pub fn summary(&self) -> String {
        let mut parts = vec![format!("[{}]", self.class)];
        if let Some(ref code) = self.code {
            parts.push(format!("[{}]", code));
        }
        parts.push(self.message.clone());
        if let Some(ref loc) = self.location {
            parts.push(format!("at {}", loc));
        }
        if self.occurrence_count > 1 {
            parts.push(format!("(x{})", self.occurrence_count));
        }
        parts.join(" ")
    }
}

// ============================================================================
// Failure Classifier
// ============================================================================

/// Classifies failures based on error output analysis.
pub struct FailureClassifier {
    /// Compiled regex patterns for classification.
    patterns: Vec<(Regex, FailureClass)>,
}

impl FailureClassifier {
    /// Create a new classifier with default patterns.
    #[must_use]
    pub fn new() -> Self {
        // Patterns ordered from most specific to least specific
        // More specific patterns must come BEFORE more general ones
        let patterns = vec![
            // Process failures (transient, Claude CLI issues)
            (r"No messages returned", FailureClass::ProcessFailure),
            (r"process crashed", FailureClass::ProcessFailure),
            (r"connection timed out", FailureClass::ProcessFailure),
            // Import errors (must come before generic compile errors)
            (r"unresolved import", FailureClass::ImportError),
            (r"cannot find .+ in this scope", FailureClass::ImportError),
            // Lifetime/borrow specific (must come before generic compile errors)
            (
                r"borrowed value does not live long enough",
                FailureClass::LifetimeError,
            ),
            (r"cannot borrow .+ as mutable", FailureClass::LifetimeError),
            (r"cannot move out of", FailureClass::LifetimeError),
            (r"lifetime .+ required", FailureClass::LifetimeError),
            (r"does not live long enough", FailureClass::LifetimeError),
            // Trait bounds (must come before generic compile errors)
            (
                r"the trait .+ is not implemented",
                FailureClass::TraitBoundError,
            ),
            (
                r"trait bound .+ is not satisfied",
                FailureClass::TraitBoundError,
            ),
            (r"doesn't implement", FailureClass::TraitBoundError),
            // Type inference (must come before generic compile errors)
            (r"type annotations needed", FailureClass::TypeInferenceError),
            (r"cannot infer type", FailureClass::TypeInferenceError),
            // Test failures (specific patterns)
            (r"test .+ \.\.\. FAILED", FailureClass::TestFailure),
            (r"panicked at", FailureClass::TestFailure),
            (r"assertion .+ failed", FailureClass::TestFailure),
            (r"left: .+\n.+right:", FailureClass::TestFailure),
            // Clippy warnings (before generic warnings)
            (r"clippy::", FailureClass::ClippyWarning),
            (
                r"warning: .+\n.+= note: `#\[warn",
                FailureClass::ClippyWarning,
            ),
            // Security findings
            (r"security vulnerability", FailureClass::SecurityFinding),
            (r"CVE-", FailureClass::SecurityFinding),
            (r"RUSTSEC-", FailureClass::SecurityFinding),
            // Missing dependencies
            (r"can't find crate", FailureClass::MissingDependency),
            (r"could not find .+ in", FailureClass::MissingDependency),
            (r"unresolved dependency", FailureClass::MissingDependency),
            // Git errors
            (r"fatal: .+", FailureClass::GitError),
            (r"error: failed to push", FailureClass::GitError),
            (r"CONFLICT", FailureClass::GitError),
            // Tool errors
            (r"command .+ failed", FailureClass::ToolError),
            (r"tool .+ not found", FailureClass::ToolError),
            // Generic compile errors (last - most general)
            (r"error\[E\d+\]:", FailureClass::CompileError),
            (r"no method named .+ found", FailureClass::CompileError),
            (r"mismatched types", FailureClass::CompileError),
            // Syntax errors (general patterns)
            (r"expected .+, found .+", FailureClass::SyntaxError),
            (r"unexpected token", FailureClass::SyntaxError),
            (r"expected identifier", FailureClass::SyntaxError),
        ];

        let compiled: Vec<_> = patterns
            .into_iter()
            .filter_map(|(pattern, class)| Regex::new(pattern).ok().map(|re| (re, class)))
            .collect();

        Self { patterns: compiled }
    }

    /// Classify an error output string.
    ///
    /// Returns the most specific failure class that matches.
    pub fn classify(&self, output: &str) -> FailureContext {
        let mut matched_class = FailureClass::Unknown;
        let mut matched_message = output.lines().next().unwrap_or("Unknown error").to_string();

        // Try each pattern, preferring more specific matches
        for (regex, class) in &self.patterns {
            if regex.is_match(output) {
                // Extract the matching line as the message
                for line in output.lines() {
                    if regex.is_match(line) {
                        matched_message = line.to_string();
                        break;
                    }
                }
                matched_class = *class;
                break; // Use first match (patterns are ordered by specificity)
            }
        }

        let mut context =
            FailureContext::new(matched_class, matched_message).with_raw_output(output);

        // Try to extract error code
        if let Some(code) = self.extract_error_code(output) {
            context = context.with_code(code);
        }

        // Try to extract location
        if let Some(location) = self.extract_location(output) {
            context = context.with_location(location);
        }

        // Try to extract suggestion
        if let Some(suggestion) = self.extract_suggestion(output) {
            context = context.with_suggestion(suggestion);
        }

        // Extract related "note:" lines
        for related in self.extract_related_notes(output) {
            context = context.with_related(related);
        }

        // Add labels based on failure characteristics
        context = self.add_labels(context);

        context
    }

    /// Extract error code from output (e.g., "E0308").
    fn extract_error_code(&self, output: &str) -> Option<String> {
        // Match error[E0308] pattern
        let re = Regex::new(r"error\[(E\d+)\]").ok()?;
        re.captures(output)
            .and_then(|caps| caps.get(1))
            .map(|m| m.as_str().to_string())
    }

    /// Extract file location from error output.
    fn extract_location(&self, output: &str) -> Option<FailureLocation> {
        // Match patterns like "  --> src/lib.rs:123:45"
        let re = Regex::new(r"-->\s*([^:]+):(\d+):(\d+)").ok()?;
        if let Some(caps) = re.captures(output) {
            let file = caps.get(1)?.as_str();
            let line: u32 = caps.get(2)?.as_str().parse().ok()?;
            let column: u32 = caps.get(3)?.as_str().parse().ok()?;
            return Some(
                FailureLocation::new(file)
                    .with_line(line)
                    .with_column(column),
            );
        }

        // Try simpler pattern "src/lib.rs:123"
        let re = Regex::new(r"([a-zA-Z0-9_/\-\.]+\.rs):(\d+)").ok()?;
        if let Some(caps) = re.captures(output) {
            let file = caps.get(1)?.as_str();
            let line: u32 = caps.get(2)?.as_str().parse().ok()?;
            return Some(FailureLocation::new(file).with_line(line));
        }

        None
    }

    /// Extract suggested fix from compiler output.
    fn extract_suggestion(&self, output: &str) -> Option<String> {
        // Match "help: " suggestions
        let re = Regex::new(r"help:\s*(.+)").ok()?;
        re.captures(output)
            .and_then(|caps| caps.get(1))
            .map(|m| m.as_str().to_string())
    }

    /// Extract related "note:" messages from compiler output.
    fn extract_related_notes(&self, output: &str) -> Vec<String> {
        let mut notes = Vec::new();
        if let Ok(re) = Regex::new(r"note:\s*(.+)") {
            for caps in re.captures_iter(output) {
                if let Some(note) = caps.get(1) {
                    notes.push(note.as_str().to_string());
                }
            }
        }
        notes
    }

    /// Add appropriate labels to a failure context based on its characteristics.
    fn add_labels(&self, mut context: FailureContext) -> FailureContext {
        // Add severity label based on failure class
        if context.class.requires_code_change() {
            context = context.with_label("requires-code-change");
        }
        if context.class.may_be_transient() {
            context = context.with_label("may-be-transient");
        }

        // Add complexity label
        let complexity = context.class.complexity_estimate();
        if complexity <= 2 {
            context = context.with_label("simple");
        } else if complexity >= 4 {
            context = context.with_label("complex");
        }

        context
    }
}

impl Default for FailureClassifier {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================================
// Recovery Strategies
// ============================================================================

/// Strategy for recovering from a failure.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum RecoveryStrategy {
    /// Fix the issue in isolation, focusing only on the failing code.
    IsolatedFix,
    /// Write or fix tests first to understand expected behavior.
    TestFirst,
    /// Decompose the task into smaller sub-tasks.
    Decompose,
    /// Gather more context before attempting a fix.
    GatherContext,
    /// Retry the same operation (for transient failures).
    SimpleRetry,
    /// Escalate to debug mode with more verbose output.
    EscalateDebug,
    /// Rollback recent changes and try a different approach.
    RollbackAndRetry,
    /// Skip this task and move to another.
    SkipTask,
}

impl RecoveryStrategy {
    /// Get a human-readable description of this strategy.
    #[must_use]
    pub fn description(&self) -> &'static str {
        match self {
            Self::IsolatedFix => "Focus on fixing the specific error in isolation",
            Self::TestFirst => "Write or examine tests to understand expected behavior",
            Self::Decompose => "Break down the task into smaller, manageable sub-tasks",
            Self::GatherContext => "Gather more context about the codebase before fixing",
            Self::SimpleRetry => "Retry the operation (may be transient failure)",
            Self::EscalateDebug => "Switch to debug mode for more detailed output",
            Self::RollbackAndRetry => "Rollback recent changes and try a different approach",
            Self::SkipTask => "Skip this task and proceed to another",
        }
    }

    /// Check if this strategy requires prompt modification.
    #[must_use]
    pub fn modifies_prompt(&self) -> bool {
        !matches!(self, Self::SimpleRetry | Self::SkipTask)
    }

    /// Get the maximum retries recommended for this strategy.
    #[must_use]
    pub fn max_retries(&self) -> u32 {
        match self {
            Self::SimpleRetry => 3,
            Self::IsolatedFix => 3,
            Self::TestFirst => 2,
            Self::Decompose => 1,
            Self::GatherContext => 2,
            Self::EscalateDebug => 1,
            Self::RollbackAndRetry => 1,
            Self::SkipTask => 0,
        }
    }
}

impl std::fmt::Display for RecoveryStrategy {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.description())
    }
}

/// Selects recovery strategies based on failure context.
pub struct RecoveryStrategist {
    /// Strategy history to avoid repeating ineffective strategies.
    history: HashMap<String, Vec<RecoveryStrategy>>,
}

impl RecoveryStrategist {
    /// Create a new strategist.
    #[must_use]
    pub fn new() -> Self {
        Self {
            history: HashMap::new(),
        }
    }

    /// Select the best recovery strategy for a failure.
    pub fn select(&mut self, failure: &FailureContext) -> RecoveryStrategy {
        // Get key for tracking history
        let key = self.failure_key(failure);

        // Get previously tried strategies (clone to avoid borrow issues)
        let tried = self.history.get(&key).cloned().unwrap_or_default();

        // Select strategy based on failure class and history
        let strategy = self.select_for_class(failure, &tried);

        // Record the strategy
        self.history.entry(key).or_default().push(strategy);

        strategy
    }

    /// Generate a unique key for a failure (for history tracking).
    fn failure_key(&self, failure: &FailureContext) -> String {
        let mut key = format!("{:?}", failure.class);
        if let Some(ref code) = failure.code {
            key.push(':');
            key.push_str(code);
        }
        if let Some(ref loc) = failure.location {
            key.push(':');
            key.push_str(&loc.file.display().to_string());
        }
        key
    }

    /// Select strategy based on failure class and previously tried strategies.
    fn select_for_class(
        &self,
        failure: &FailureContext,
        tried: &[RecoveryStrategy],
    ) -> RecoveryStrategy {
        // If failure may be transient and we haven't tried SimpleRetry
        if failure.class.may_be_transient() && !tried.contains(&RecoveryStrategy::SimpleRetry) {
            return RecoveryStrategy::SimpleRetry;
        }

        // Strategy selection based on failure class
        let candidates = match failure.class {
            FailureClass::CompileError | FailureClass::SyntaxError => vec![
                RecoveryStrategy::IsolatedFix,
                RecoveryStrategy::GatherContext,
                RecoveryStrategy::EscalateDebug,
            ],
            FailureClass::TestFailure => vec![
                RecoveryStrategy::TestFirst,
                RecoveryStrategy::IsolatedFix,
                RecoveryStrategy::GatherContext,
            ],
            FailureClass::ClippyWarning => vec![
                RecoveryStrategy::IsolatedFix,
                RecoveryStrategy::GatherContext,
            ],
            FailureClass::LifetimeError | FailureClass::TraitBoundError => vec![
                RecoveryStrategy::GatherContext,
                RecoveryStrategy::IsolatedFix,
                RecoveryStrategy::Decompose,
            ],
            FailureClass::TypeInferenceError => vec![
                RecoveryStrategy::IsolatedFix,
                RecoveryStrategy::GatherContext,
            ],
            FailureClass::ImportError | FailureClass::MissingDependency => vec![
                RecoveryStrategy::IsolatedFix,
                RecoveryStrategy::GatherContext,
            ],
            FailureClass::SecurityFinding => vec![
                RecoveryStrategy::IsolatedFix,
                RecoveryStrategy::GatherContext,
                RecoveryStrategy::TestFirst,
            ],
            FailureClass::GitError | FailureClass::ToolError | FailureClass::ProcessFailure => vec![
                RecoveryStrategy::SimpleRetry,
                RecoveryStrategy::EscalateDebug,
            ],
            FailureClass::Unknown => vec![
                RecoveryStrategy::GatherContext,
                RecoveryStrategy::IsolatedFix,
                RecoveryStrategy::EscalateDebug,
            ],
        };

        // Return first candidate not already tried
        for candidate in candidates {
            if !tried.contains(&candidate) {
                return candidate;
            }
        }

        // If all strategies tried, check if recurring
        if failure.is_recurring() {
            return RecoveryStrategy::Decompose;
        }

        // Default fallbacks
        if !tried.contains(&RecoveryStrategy::RollbackAndRetry) {
            RecoveryStrategy::RollbackAndRetry
        } else {
            RecoveryStrategy::SkipTask
        }
    }

    /// Clear history for a specific failure key.
    pub fn clear_history(&mut self, failure: &FailureContext) {
        let key = self.failure_key(failure);
        self.history.remove(&key);
    }

    /// Clear all history.
    pub fn clear_all_history(&mut self) {
        self.history.clear();
    }
}

impl Default for RecoveryStrategist {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================================
// Intelligent Retry
// ============================================================================

/// Configuration for intelligent retry behavior.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RetryConfig {
    /// Maximum total retries per task.
    pub max_retries_per_task: u32,
    /// Maximum retries for the same failure.
    pub max_retries_same_failure: u32,
    /// Include raw error output in retry prompt.
    pub include_raw_output: bool,
    /// Maximum characters of raw output to include.
    pub max_raw_output_chars: usize,
}

impl Default for RetryConfig {
    fn default() -> Self {
        Self {
            max_retries_per_task: 5,
            max_retries_same_failure: 2,
            include_raw_output: true,
            max_raw_output_chars: 2000,
        }
    }
}

impl RetryConfig {
    /// Create a new config with custom max retries per task.
    #[must_use]
    pub fn with_max_retries(mut self, max: u32) -> Self {
        self.max_retries_per_task = max;
        self
    }

    /// Create a new config with raw output disabled.
    #[must_use]
    pub fn without_raw_output(mut self) -> Self {
        self.include_raw_output = false;
        self
    }
}

/// Coordinates intelligent retry behavior.
pub struct IntelligentRetry {
    config: RetryConfig,
    classifier: FailureClassifier,
    strategist: RecoveryStrategist,
    retry_count: HashMap<String, u32>,
}

impl std::fmt::Debug for IntelligentRetry {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("IntelligentRetry")
            .field("config", &self.config)
            .field("retry_count", &self.retry_count)
            .finish_non_exhaustive()
    }
}

impl IntelligentRetry {
    /// Create a new intelligent retry handler.
    #[must_use]
    pub fn new() -> Self {
        Self {
            config: RetryConfig::default(),
            classifier: FailureClassifier::new(),
            strategist: RecoveryStrategist::new(),
            retry_count: HashMap::new(),
        }
    }

    /// Create with custom configuration.
    #[must_use]
    pub fn with_config(config: RetryConfig) -> Self {
        Self {
            config,
            classifier: FailureClassifier::new(),
            strategist: RecoveryStrategist::new(),
            retry_count: HashMap::new(),
        }
    }

    /// Process a failure and determine retry strategy.
    ///
    /// Returns `Some((strategy, prompt))` if retry should be attempted,
    /// `None` if max retries exceeded.
    pub fn process_failure(
        &mut self,
        error_output: &str,
        task_id: Option<&str>,
    ) -> Option<(RecoveryStrategy, String)> {
        // Classify the failure
        let mut failure = self.classifier.classify(error_output);

        // Check retry limits
        let key = task_id.unwrap_or("default").to_string();
        let count = self.retry_count.entry(key.clone()).or_insert(0);
        *count += 1;

        // Track failure occurrence count for this classification
        failure.increment_occurrence();

        // Select recovery strategy
        let strategy = self.strategist.select(&failure);

        // Check against both global limit and strategy-specific limit
        let strategy_max = strategy.max_retries();
        if *count > self.config.max_retries_per_task || *count > strategy_max {
            return None;
        }

        // Generate retry prompt
        let prompt = self.generate_prompt(&strategy, &failure);

        Some((strategy, prompt))
    }

    /// Generate a retry prompt for the given strategy and failure.
    #[must_use]
    pub fn generate_prompt(&self, strategy: &RecoveryStrategy, failure: &FailureContext) -> String {
        let mut prompt = String::new();

        // Header
        prompt.push_str("## Retry: Fix Previous Failure\n\n");

        // Failure summary
        prompt.push_str("### Failure Details\n\n");
        prompt.push_str(&format!("**Type**: {}\n", failure.class));
        prompt.push_str(&format!("**Message**: {}\n", failure.message));

        if let Some(ref code) = failure.code {
            prompt.push_str(&format!("**Code**: {}\n", code));
        }

        if let Some(ref location) = failure.location {
            prompt.push_str(&format!("**Location**: {}\n", location));
        }

        if let Some(ref suggestion) = failure.suggestion {
            prompt.push_str(&format!("\n**Compiler Suggestion**: {}\n", suggestion));
        }

        prompt.push('\n');

        // Strategy-specific instructions
        prompt.push_str("### Recovery Strategy\n\n");
        prompt.push_str(&format!("**Approach**: {}\n\n", strategy));

        match strategy {
            RecoveryStrategy::IsolatedFix => {
                prompt.push_str("Focus ONLY on fixing this specific error:\n");
                prompt.push_str("1. Read the error message carefully\n");
                prompt.push_str("2. Navigate to the exact location\n");
                prompt.push_str("3. Make the minimal change needed to fix it\n");
                prompt.push_str("4. Do NOT refactor or improve surrounding code\n");
            }
            RecoveryStrategy::TestFirst => {
                prompt.push_str("Understand expected behavior through tests:\n");
                prompt.push_str("1. Find and read the failing test\n");
                prompt.push_str("2. Understand what behavior it expects\n");
                prompt.push_str("3. Fix the implementation to match expectations\n");
                prompt.push_str("4. Run the specific test to verify\n");
            }
            RecoveryStrategy::Decompose => {
                prompt.push_str("This task may be too complex. Break it down:\n");
                prompt.push_str("1. Identify the smallest piece you can fix\n");
                prompt.push_str("2. Fix just that piece and commit\n");
                prompt.push_str("3. Then move to the next piece\n");
            }
            RecoveryStrategy::GatherContext => {
                prompt.push_str("Gather more context before fixing:\n");
                prompt.push_str("1. Read related code files\n");
                prompt.push_str("2. Check how similar code handles this\n");
                prompt.push_str("3. Understand the types and traits involved\n");
                prompt.push_str("4. Then attempt the fix with full understanding\n");
            }
            RecoveryStrategy::SimpleRetry => {
                prompt.push_str("This may be a transient failure. Simply retry the operation.\n");
            }
            RecoveryStrategy::EscalateDebug => {
                prompt.push_str("Enable verbose output for debugging:\n");
                prompt.push_str("1. Run with --verbose or RUST_BACKTRACE=1\n");
                prompt.push_str("2. Add debug logging if needed\n");
                prompt.push_str("3. Examine the detailed output\n");
            }
            RecoveryStrategy::RollbackAndRetry => {
                prompt.push_str("Recent changes may have caused issues:\n");
                prompt.push_str("1. Consider reverting the last change\n");
                prompt.push_str("2. Try a different approach\n");
                prompt.push_str("3. Make smaller, incremental changes\n");
            }
            RecoveryStrategy::SkipTask => {
                prompt.push_str("This task should be skipped for now.\n");
                prompt.push_str("Move on to the next task in the plan.\n");
            }
        }

        // Raw output if configured
        if self.config.include_raw_output && !failure.raw_output.is_empty() {
            prompt.push_str("\n### Error Output\n\n```\n");
            let truncated: String = failure
                .raw_output
                .chars()
                .take(self.config.max_raw_output_chars)
                .collect();
            prompt.push_str(&truncated);
            if failure.raw_output.len() > self.config.max_raw_output_chars {
                prompt.push_str("\n... (truncated)");
            }
            prompt.push_str("\n```\n");
        }

        prompt
    }

    /// Reset retry count for a task.
    pub fn reset_task(&mut self, task_id: &str) {
        self.retry_count.remove(task_id);
        self.strategist.clear_all_history();
    }

    /// Clear history for a specific failure (call when failure is resolved).
    pub fn clear_failure_history(&mut self, failure: &FailureContext) {
        self.strategist.clear_history(failure);
    }

    /// Get current retry count for a task.
    #[must_use]
    pub fn retry_count(&self, task_id: &str) -> u32 {
        self.retry_count.get(task_id).copied().unwrap_or(0)
    }

    /// Check if retries are exhausted for a task.
    #[must_use]
    pub fn retries_exhausted(&self, task_id: &str) -> bool {
        self.retry_count(task_id) >= self.config.max_retries_per_task
    }

    /// Get a reference to the classifier.
    #[must_use]
    pub fn classifier(&self) -> &FailureClassifier {
        &self.classifier
    }

    /// Get a summary of retry state.
    #[must_use]
    pub fn summary(&self) -> String {
        let total_retries: u32 = self.retry_count.values().sum();
        let tasks_with_retries = self.retry_count.len();
        format!(
            "{} total retries across {} tasks",
            total_retries, tasks_with_retries
        )
    }
}

impl Default for IntelligentRetry {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================================
// Task Decomposition
// ============================================================================

/// A decomposed sub-task derived from a parent task.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SubTask {
    /// Description of the sub-task.
    pub description: String,
    /// Estimated complexity (1-5).
    pub complexity: u8,
    /// Files likely involved.
    pub files: Vec<PathBuf>,
    /// Dependencies on other sub-tasks.
    pub depends_on: Vec<usize>,
}

impl SubTask {
    /// Create a new sub-task.
    pub fn new(description: impl Into<String>) -> Self {
        Self {
            description: description.into(),
            complexity: 1,
            files: Vec::new(),
            depends_on: Vec::new(),
        }
    }

    /// Set complexity.
    #[must_use]
    pub fn with_complexity(mut self, complexity: u8) -> Self {
        self.complexity = complexity.min(5);
        self
    }

    /// Add a file.
    #[must_use]
    pub fn with_file(mut self, file: impl Into<PathBuf>) -> Self {
        self.files.push(file.into());
        self
    }

    /// Add a dependency.
    #[must_use]
    pub fn depends_on(mut self, index: usize) -> Self {
        self.depends_on.push(index);
        self
    }
}

/// Decomposes complex tasks into smaller sub-tasks.
pub struct TaskDecomposer;

impl TaskDecomposer {
    /// Create a new decomposer.
    #[must_use]
    pub fn new() -> Self {
        Self
    }

    /// Decompose a task based on failure context.
    ///
    /// Returns a list of sub-tasks to tackle the failure incrementally.
    #[must_use]
    pub fn decompose(&self, task_description: &str, failure: &FailureContext) -> Vec<SubTask> {
        let mut subtasks = Vec::new();

        // Add context-gathering sub-task if failure is complex
        if failure.class.complexity_estimate() >= 3 {
            subtasks.push(SubTask::new(format!(
                "Read and understand the code around {}",
                failure
                    .location
                    .as_ref()
                    .map(|l| l.format())
                    .unwrap_or_else(|| "the error".to_string())
            )));
        }

        // Add specific fix sub-task
        match failure.class {
            FailureClass::TestFailure => {
                subtasks.push(SubTask::new(
                    "Read the failing test to understand expected behavior",
                ));
                subtasks.push(
                    SubTask::new("Identify what the implementation should do differently")
                        .depends_on(subtasks.len() - 1),
                );
                subtasks.push(
                    SubTask::new("Make the minimal fix to pass the test")
                        .depends_on(subtasks.len() - 1),
                );
            }
            FailureClass::LifetimeError | FailureClass::TraitBoundError => {
                subtasks.push(SubTask::new(
                    "Understand the type signatures and lifetimes involved",
                ));
                subtasks.push(
                    SubTask::new("Check if the approach needs restructuring")
                        .depends_on(subtasks.len() - 1),
                );
                subtasks.push(
                    SubTask::new("Apply the fix with correct lifetime/trait bounds")
                        .depends_on(subtasks.len() - 1),
                );
            }
            FailureClass::CompileError => {
                let mut fix_task =
                    SubTask::new(format!("Fix the compilation error: {}", failure.message));
                // Add file context if available
                if let Some(ref loc) = failure.location {
                    fix_task = fix_task.with_file(loc.file.clone());
                }
                subtasks.push(fix_task);
                if let Some(ref suggestion) = failure.suggestion {
                    subtasks.push(
                        SubTask::new(format!("Apply suggested fix: {}", suggestion))
                            .with_complexity(1),
                    );
                }
            }
            _ => {
                // Generic decomposition
                subtasks.push(SubTask::new(format!(
                    "Fix the {} issue",
                    failure.class.description()
                )));
            }
        }

        // Add verification sub-task
        subtasks.push(
            SubTask::new(format!("Verify the fix for: {}", task_description))
                .depends_on(subtasks.len() - 1),
        );

        subtasks
    }

    /// Check if a task is atomic (shouldn't be decomposed).
    #[must_use]
    pub fn is_atomic(&self, failure: &FailureContext) -> bool {
        // Simple failures don't need decomposition
        failure.class.complexity_estimate() <= 2 && !failure.is_recurring()
    }
}

impl Default for TaskDecomposer {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================================
// Retry History
// ============================================================================

/// Record of a retry attempt.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RetryAttempt {
    /// The failure that triggered the retry.
    pub failure: FailureContext,
    /// Strategy used for this retry.
    pub strategy: RecoveryStrategy,
    /// Whether this retry succeeded.
    pub succeeded: bool,
    /// Timestamp of the attempt.
    pub timestamp: chrono::DateTime<chrono::Utc>,
}

impl RetryAttempt {
    /// Create a new retry attempt record.
    pub fn new(failure: FailureContext, strategy: RecoveryStrategy) -> Self {
        Self {
            failure,
            strategy,
            succeeded: false,
            timestamp: chrono::Utc::now(),
        }
    }

    /// Mark this attempt as successful.
    pub fn mark_success(&mut self) {
        self.succeeded = true;
    }
}

/// Tracks retry history for learning and analysis.
#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct RetryHistory {
    /// All retry attempts.
    attempts: Vec<RetryAttempt>,
}

impl RetryHistory {
    /// Create a new history.
    #[must_use]
    pub fn new() -> Self {
        Self {
            attempts: Vec::new(),
        }
    }

    /// Record a retry attempt.
    pub fn record(&mut self, attempt: RetryAttempt) {
        self.attempts.push(attempt);
    }

    /// Get success rate for a strategy.
    #[must_use]
    pub fn success_rate(&self, strategy: RecoveryStrategy) -> f64 {
        let matching: Vec<_> = self
            .attempts
            .iter()
            .filter(|a| a.strategy == strategy)
            .collect();

        if matching.is_empty() {
            return 0.0;
        }

        let successes = matching.iter().filter(|a| a.succeeded).count();
        successes as f64 / matching.len() as f64
    }

    /// Get success rate for a failure class.
    #[must_use]
    pub fn class_success_rate(&self, class: FailureClass) -> f64 {
        let matching: Vec<_> = self
            .attempts
            .iter()
            .filter(|a| a.failure.class == class)
            .collect();

        if matching.is_empty() {
            return 0.0;
        }

        let successes = matching.iter().filter(|a| a.succeeded).count();
        successes as f64 / matching.len() as f64
    }

    /// Get the total number of attempts.
    #[must_use]
    pub fn total_attempts(&self) -> usize {
        self.attempts.len()
    }

    /// Get the number of successful attempts.
    #[must_use]
    pub fn successful_attempts(&self) -> usize {
        self.attempts.iter().filter(|a| a.succeeded).count()
    }

    /// Get a summary of retry history.
    #[must_use]
    pub fn summary(&self) -> String {
        let total = self.total_attempts();
        let successes = self.successful_attempts();
        let rate = if total > 0 {
            (successes as f64 / total as f64) * 100.0
        } else {
            0.0
        };
        format!(
            "{} retry attempts ({} successful, {:.1}% success rate)",
            total, successes, rate
        )
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    // -------------------------------------------------------------------------
    // FailureClass tests
    // -------------------------------------------------------------------------

    #[test]
    fn test_failure_class_description() {
        assert_eq!(
            FailureClass::CompileError.description(),
            "Compilation error"
        );
        assert_eq!(FailureClass::TestFailure.description(), "Test failure");
        assert_eq!(
            FailureClass::LifetimeError.description(),
            "Lifetime/borrow error"
        );
    }

    #[test]
    fn test_failure_class_requires_code_change() {
        assert!(FailureClass::CompileError.requires_code_change());
        assert!(FailureClass::TestFailure.requires_code_change());
        assert!(!FailureClass::GitError.requires_code_change());
        assert!(!FailureClass::ToolError.requires_code_change());
    }

    #[test]
    fn test_failure_class_may_be_transient() {
        assert!(FailureClass::GitError.may_be_transient());
        assert!(FailureClass::ToolError.may_be_transient());
        assert!(!FailureClass::CompileError.may_be_transient());
    }

    #[test]
    fn test_failure_class_complexity() {
        assert!(
            FailureClass::SyntaxError.complexity_estimate()
                < FailureClass::LifetimeError.complexity_estimate()
        );
        assert!(
            FailureClass::ImportError.complexity_estimate()
                < FailureClass::TraitBoundError.complexity_estimate()
        );
    }

    #[test]
    fn test_process_failure_is_transient() {
        assert!(FailureClass::ProcessFailure.may_be_transient());
        assert!(!FailureClass::ProcessFailure.requires_code_change());
        assert_eq!(FailureClass::ProcessFailure.complexity_estimate(), 1);
    }

    #[test]
    fn test_classifier_detects_no_messages_returned() {
        let classifier = FailureClassifier::new();
        let ctx = classifier.classify("No messages returned");
        assert_eq!(ctx.class, FailureClass::ProcessFailure);
    }

    #[test]
    fn test_classifier_detects_process_crashed() {
        let classifier = FailureClassifier::new();
        let ctx = classifier.classify("The process crashed unexpectedly");
        assert_eq!(ctx.class, FailureClass::ProcessFailure);
    }

    #[test]
    fn test_classifier_detects_connection_timed_out() {
        let classifier = FailureClassifier::new();
        let ctx = classifier.classify("connection timed out while waiting for response");
        assert_eq!(ctx.class, FailureClass::ProcessFailure);
    }

    // -------------------------------------------------------------------------
    // Exponential backoff tests
    // -------------------------------------------------------------------------

    #[test]
    fn test_exponential_backoff_calculation() {
        use std::time::Duration;
        assert_eq!(calculate_backoff(1), Duration::from_millis(2000)); // Base
        assert_eq!(calculate_backoff(2), Duration::from_millis(4000)); // 2x
        assert_eq!(calculate_backoff(3), Duration::from_millis(8000)); // 4x
        assert_eq!(calculate_backoff(5), Duration::from_millis(30000)); // Capped at max
    }

    #[test]
    fn test_backoff_first_attempt_is_base() {
        use std::time::Duration;
        assert_eq!(calculate_backoff(1), Duration::from_millis(RETRY_BACKOFF_BASE_MS));
    }

    #[test]
    fn test_backoff_caps_at_max() {
        use std::time::Duration;
        // Even with very high attempt number, should never exceed max
        assert_eq!(calculate_backoff(100), Duration::from_millis(MAX_BACKOFF_MS));
    }

    // -------------------------------------------------------------------------
    // FailureLocation tests
    // -------------------------------------------------------------------------

    #[test]
    fn test_failure_location_format() {
        let loc = FailureLocation::new("src/lib.rs");
        assert_eq!(loc.format(), "src/lib.rs");

        let loc = FailureLocation::new("src/lib.rs").with_line(42);
        assert_eq!(loc.format(), "src/lib.rs:42");

        let loc = FailureLocation::new("src/lib.rs")
            .with_line(42)
            .with_column(10);
        assert_eq!(loc.format(), "src/lib.rs:42:10");
    }

    // -------------------------------------------------------------------------
    // FailureContext tests
    // -------------------------------------------------------------------------

    #[test]
    fn test_failure_context_creation() {
        let ctx = FailureContext::new(FailureClass::CompileError, "mismatched types")
            .with_code("E0308")
            .with_location(FailureLocation::new("src/lib.rs").with_line(10));

        assert_eq!(ctx.class, FailureClass::CompileError);
        assert_eq!(ctx.message, "mismatched types");
        assert_eq!(ctx.code, Some("E0308".to_string()));
        assert!(ctx.location.is_some());
    }

    #[test]
    fn test_failure_context_is_recurring() {
        let mut ctx = FailureContext::new(FailureClass::CompileError, "error");
        assert!(!ctx.is_recurring());

        ctx.increment_occurrence();
        assert!(ctx.is_recurring());
    }

    #[test]
    fn test_failure_context_summary() {
        let ctx = FailureContext::new(FailureClass::CompileError, "mismatched types")
            .with_code("E0308")
            .with_location(FailureLocation::new("src/lib.rs").with_line(10));

        let summary = ctx.summary();
        assert!(summary.contains("Compilation error"));
        assert!(summary.contains("E0308"));
        assert!(summary.contains("mismatched types"));
        assert!(summary.contains("src/lib.rs:10"));
    }

    // -------------------------------------------------------------------------
    // FailureClassifier tests
    // -------------------------------------------------------------------------

    #[test]
    fn test_classifier_compile_error() {
        let classifier = FailureClassifier::new();
        let output = r#"error[E0308]: mismatched types
   --> src/lib.rs:10:5
    |
10  |     let x: u32 = "hello";
    |            ---   ^^^^^^^ expected `u32`, found `&str`
    |            |
    |            expected due to this"#;

        let ctx = classifier.classify(output);
        assert_eq!(ctx.class, FailureClass::CompileError);
        assert_eq!(ctx.code, Some("E0308".to_string()));
        assert!(ctx.location.is_some());
        let loc = ctx.location.unwrap();
        assert_eq!(loc.line, Some(10));
    }

    #[test]
    fn test_classifier_test_failure() {
        let classifier = FailureClassifier::new();
        let output = "test tests::test_add ... FAILED\n\nthread 'tests::test_add' panicked at 'assertion failed'";

        let ctx = classifier.classify(output);
        assert_eq!(ctx.class, FailureClass::TestFailure);
    }

    #[test]
    fn test_classifier_lifetime_error() {
        let classifier = FailureClassifier::new();
        let output = "error: borrowed value does not live long enough";

        let ctx = classifier.classify(output);
        assert_eq!(ctx.class, FailureClass::LifetimeError);
    }

    #[test]
    fn test_classifier_trait_bound_error() {
        let classifier = FailureClassifier::new();
        let output = "error: the trait `Display` is not implemented for `MyType`";

        let ctx = classifier.classify(output);
        assert_eq!(ctx.class, FailureClass::TraitBoundError);
    }

    #[test]
    fn test_classifier_import_error() {
        let classifier = FailureClassifier::new();
        let output = "error[E0432]: unresolved import `foo::bar`";

        let ctx = classifier.classify(output);
        assert_eq!(ctx.class, FailureClass::ImportError);
    }

    #[test]
    fn test_classifier_clippy_warning() {
        let classifier = FailureClassifier::new();
        let output = "warning: clippy::unwrap_used\n  --> src/lib.rs:5:10";

        let ctx = classifier.classify(output);
        assert_eq!(ctx.class, FailureClass::ClippyWarning);
    }

    #[test]
    fn test_classifier_git_error() {
        let classifier = FailureClassifier::new();
        let output = "fatal: not a git repository";

        let ctx = classifier.classify(output);
        assert_eq!(ctx.class, FailureClass::GitError);
    }

    #[test]
    fn test_classifier_unknown() {
        let classifier = FailureClassifier::new();
        let output = "some random error message that doesn't match any pattern";

        let ctx = classifier.classify(output);
        assert_eq!(ctx.class, FailureClass::Unknown);
    }

    #[test]
    fn test_classifier_extracts_suggestion() {
        let classifier = FailureClassifier::new();
        let output = "error[E0308]: mismatched types\nhelp: try using `as` to convert";

        let ctx = classifier.classify(output);
        assert_eq!(
            ctx.suggestion,
            Some("try using `as` to convert".to_string())
        );
    }

    // -------------------------------------------------------------------------
    // RecoveryStrategy tests
    // -------------------------------------------------------------------------

    #[test]
    fn test_recovery_strategy_description() {
        assert!(!RecoveryStrategy::IsolatedFix.description().is_empty());
        assert!(!RecoveryStrategy::TestFirst.description().is_empty());
    }

    #[test]
    fn test_recovery_strategy_modifies_prompt() {
        assert!(RecoveryStrategy::IsolatedFix.modifies_prompt());
        assert!(RecoveryStrategy::TestFirst.modifies_prompt());
        assert!(!RecoveryStrategy::SimpleRetry.modifies_prompt());
        assert!(!RecoveryStrategy::SkipTask.modifies_prompt());
    }

    #[test]
    fn test_recovery_strategy_max_retries() {
        assert!(RecoveryStrategy::SimpleRetry.max_retries() > 0);
        assert_eq!(RecoveryStrategy::SkipTask.max_retries(), 0);
    }

    // -------------------------------------------------------------------------
    // RecoveryStrategist tests
    // -------------------------------------------------------------------------

    #[test]
    fn test_strategist_selects_for_transient() {
        let mut strategist = RecoveryStrategist::new();
        let failure = FailureContext::new(FailureClass::GitError, "connection failed");

        let strategy = strategist.select(&failure);
        assert_eq!(strategy, RecoveryStrategy::SimpleRetry);
    }

    #[test]
    fn test_strategist_selects_for_test_failure() {
        let mut strategist = RecoveryStrategist::new();
        let failure = FailureContext::new(FailureClass::TestFailure, "assertion failed");

        let strategy = strategist.select(&failure);
        assert_eq!(strategy, RecoveryStrategy::TestFirst);
    }

    #[test]
    fn test_strategist_selects_for_compile_error() {
        let mut strategist = RecoveryStrategist::new();
        let failure = FailureContext::new(FailureClass::CompileError, "mismatched types");

        let strategy = strategist.select(&failure);
        assert_eq!(strategy, RecoveryStrategy::IsolatedFix);
    }

    #[test]
    fn test_strategist_avoids_repeating() {
        let mut strategist = RecoveryStrategist::new();
        let failure = FailureContext::new(FailureClass::CompileError, "mismatched types");

        let first = strategist.select(&failure);
        let second = strategist.select(&failure);

        assert_ne!(first, second);
    }

    #[test]
    fn test_strategist_clears_history() {
        let mut strategist = RecoveryStrategist::new();
        let failure = FailureContext::new(FailureClass::CompileError, "mismatched types");

        let first = strategist.select(&failure);
        strategist.clear_history(&failure);
        let after_clear = strategist.select(&failure);

        assert_eq!(first, after_clear);
    }

    // -------------------------------------------------------------------------
    // IntelligentRetry tests
    // -------------------------------------------------------------------------

    #[test]
    fn test_intelligent_retry_process_failure() {
        let mut retry = IntelligentRetry::new();
        let output = "error[E0308]: mismatched types";

        let result = retry.process_failure(output, Some("task1"));
        assert!(result.is_some());

        let (strategy, prompt) = result.unwrap();
        assert!(matches!(
            strategy,
            RecoveryStrategy::IsolatedFix | RecoveryStrategy::GatherContext
        ));
        assert!(prompt.contains("Retry"));
        assert!(prompt.contains("Failure Details"));
    }

    #[test]
    fn test_intelligent_retry_respects_max_retries() {
        let config = RetryConfig::default().with_max_retries(2);
        let mut retry = IntelligentRetry::with_config(config);
        let output = "error: something failed";

        // First two retries should work
        assert!(retry.process_failure(output, Some("task1")).is_some());
        assert!(retry.process_failure(output, Some("task1")).is_some());

        // Third should fail
        assert!(retry.process_failure(output, Some("task1")).is_none());
    }

    #[test]
    fn test_intelligent_retry_reset_task() {
        let mut retry = IntelligentRetry::new();
        let output = "error: something failed";

        retry.process_failure(output, Some("task1"));
        assert_eq!(retry.retry_count("task1"), 1);

        retry.reset_task("task1");
        assert_eq!(retry.retry_count("task1"), 0);
    }

    #[test]
    fn test_intelligent_retry_retries_exhausted() {
        let config = RetryConfig::default().with_max_retries(1);
        let mut retry = IntelligentRetry::with_config(config);

        assert!(!retry.retries_exhausted("task1"));

        retry.process_failure("error", Some("task1"));
        assert!(retry.retries_exhausted("task1"));
    }

    #[test]
    fn test_intelligent_retry_summary() {
        let mut retry = IntelligentRetry::new();
        retry.process_failure("error", Some("task1"));
        retry.process_failure("error", Some("task2"));

        let summary = retry.summary();
        assert!(summary.contains("2 total retries"));
        assert!(summary.contains("2 tasks"));
    }

    #[test]
    fn test_generate_prompt_includes_error_output() {
        let retry = IntelligentRetry::new();
        let failure = FailureContext::new(FailureClass::CompileError, "mismatched types")
            .with_raw_output("full error output here");

        let prompt = retry.generate_prompt(&RecoveryStrategy::IsolatedFix, &failure);
        assert!(prompt.contains("full error output here"));
    }

    #[test]
    fn test_generate_prompt_without_raw_output() {
        let config = RetryConfig::default().without_raw_output();
        let retry = IntelligentRetry::with_config(config);
        let failure = FailureContext::new(FailureClass::CompileError, "mismatched types")
            .with_raw_output("full error output here");

        let prompt = retry.generate_prompt(&RecoveryStrategy::IsolatedFix, &failure);
        assert!(!prompt.contains("full error output here"));
    }

    // -------------------------------------------------------------------------
    // SubTask tests
    // -------------------------------------------------------------------------

    #[test]
    fn test_subtask_creation() {
        let subtask = SubTask::new("Fix the error")
            .with_complexity(3)
            .with_file("src/lib.rs")
            .depends_on(0);

        assert_eq!(subtask.description, "Fix the error");
        assert_eq!(subtask.complexity, 3);
        assert_eq!(subtask.files.len(), 1);
        assert_eq!(subtask.depends_on, vec![0]);
    }

    #[test]
    fn test_subtask_complexity_capped() {
        let subtask = SubTask::new("Task").with_complexity(10);
        assert_eq!(subtask.complexity, 5); // Capped at 5
    }

    // -------------------------------------------------------------------------
    // TaskDecomposer tests
    // -------------------------------------------------------------------------

    #[test]
    fn test_decomposer_for_test_failure() {
        let decomposer = TaskDecomposer::new();
        let failure = FailureContext::new(FailureClass::TestFailure, "assertion failed");

        let subtasks = decomposer.decompose("Implement feature X", &failure);
        assert!(!subtasks.is_empty());
        assert!(subtasks.iter().any(|s| s.description.contains("test")));
    }

    #[test]
    fn test_decomposer_for_complex_failure() {
        let decomposer = TaskDecomposer::new();
        let failure = FailureContext::new(FailureClass::LifetimeError, "borrowed value")
            .with_location(FailureLocation::new("src/lib.rs").with_line(10));

        let subtasks = decomposer.decompose("Fix lifetime", &failure);
        assert!(!subtasks.is_empty());
        // Should have context-gathering step for complex errors
        assert!(subtasks
            .iter()
            .any(|s| s.description.contains("Read") || s.description.contains("Understand")));
    }

    #[test]
    fn test_decomposer_is_atomic() {
        let decomposer = TaskDecomposer::new();

        let simple = FailureContext::new(FailureClass::SyntaxError, "missing semicolon");
        assert!(decomposer.is_atomic(&simple));

        let complex = FailureContext::new(FailureClass::LifetimeError, "borrowed value");
        assert!(!decomposer.is_atomic(&complex));
    }

    // -------------------------------------------------------------------------
    // RetryHistory tests
    // -------------------------------------------------------------------------

    #[test]
    fn test_retry_history_record() {
        let mut history = RetryHistory::new();
        let failure = FailureContext::new(FailureClass::CompileError, "error");
        let attempt = RetryAttempt::new(failure, RecoveryStrategy::IsolatedFix);

        history.record(attempt);
        assert_eq!(history.total_attempts(), 1);
    }

    #[test]
    fn test_retry_history_success_rate() {
        let mut history = RetryHistory::new();

        // Add successful attempt
        let mut success = RetryAttempt::new(
            FailureContext::new(FailureClass::CompileError, "error1"),
            RecoveryStrategy::IsolatedFix,
        );
        success.mark_success();
        history.record(success);

        // Add failed attempt
        let failure = RetryAttempt::new(
            FailureContext::new(FailureClass::CompileError, "error2"),
            RecoveryStrategy::IsolatedFix,
        );
        history.record(failure);

        assert!((history.success_rate(RecoveryStrategy::IsolatedFix) - 0.5).abs() < 0.01);
    }

    #[test]
    fn test_retry_history_class_success_rate() {
        let mut history = RetryHistory::new();

        let mut attempt = RetryAttempt::new(
            FailureContext::new(FailureClass::TestFailure, "test failed"),
            RecoveryStrategy::TestFirst,
        );
        attempt.mark_success();
        history.record(attempt);

        assert!((history.class_success_rate(FailureClass::TestFailure) - 1.0).abs() < 0.01);
        assert!((history.class_success_rate(FailureClass::CompileError) - 0.0).abs() < 0.01);
    }

    #[test]
    fn test_retry_history_summary() {
        let mut history = RetryHistory::new();
        let mut attempt = RetryAttempt::new(
            FailureContext::new(FailureClass::CompileError, "error"),
            RecoveryStrategy::IsolatedFix,
        );
        attempt.mark_success();
        history.record(attempt);

        let summary = history.summary();
        assert!(summary.contains("1 retry attempts"));
        assert!(summary.contains("1 successful"));
        assert!(summary.contains("100.0%"));
    }
}
