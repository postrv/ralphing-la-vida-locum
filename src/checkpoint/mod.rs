//! Checkpoint and rollback system for quality regression prevention.
//!
//! This module provides the ability to create snapshots of good code states
//! and rollback when quality metrics regress beyond acceptable thresholds.
//!
//! # Architecture
//!
//! ```text
//! ┌─────────────────┐     ┌──────────────────┐     ┌─────────────────┐
//! │ CheckpointMgr   │────>│ Checkpoint       │────>│ QualityMetrics  │
//! │                 │     │                  │     │                 │
//! └─────────────────┘     └──────────────────┘     └─────────────────┘
//!         │                       │                        │
//!         v                       v                        v
//! ┌─────────────────┐     ┌──────────────────┐     ┌─────────────────┐
//! │ create/prune    │     │ git_hash + state │     │ is_worse_than   │
//! │ list/restore    │     │ task_tracker     │     │ regression_pct  │
//! └─────────────────┘     └──────────────────┘     └─────────────────┘
//! ```
//!
//! # Example
//!
//! ```rust,ignore
//! use ralph::checkpoint::{CheckpointManager, QualityMetrics, RegressionThresholds};
//!
//! let mut manager = CheckpointManager::new(".ralph/checkpoints")?;
//!
//! // Create checkpoint after successful quality gate
//! let metrics = QualityMetrics::new()
//!     .with_clippy_warnings(0)
//!     .with_test_counts(42, 42, 0);
//! let checkpoint = manager.create_checkpoint("All tests passing", metrics)?;
//!
//! // Later, check if we should rollback
//! let current_metrics = QualityMetrics::new().with_test_counts(42, 40, 2);
//! let thresholds = RegressionThresholds::default();
//! if current_metrics.is_worse_than(&checkpoint.metrics, &thresholds) {
//!     manager.rollback_to(checkpoint.id)?;
//! }
//! ```

pub mod manager;
pub mod rollback;

// Re-export manager types
pub use manager::{CheckpointManager, CheckpointManagerConfig};
// Re-export rollback types
pub use rollback::{RollbackManager, RollbackResult};

use crate::Language;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fmt;

// ============================================================================
// Checkpoint ID
// ============================================================================

/// Unique identifier for a checkpoint.
///
/// Wraps a UUID v4 string for type safety and serialization.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct CheckpointId(String);

impl CheckpointId {
    /// Create a new random checkpoint ID.
    ///
    /// # Example
    ///
    /// ```
    /// use ralph::checkpoint::CheckpointId;
    ///
    /// let id = CheckpointId::new();
    /// assert!(!id.as_str().is_empty());
    /// ```
    #[must_use]
    pub fn new() -> Self {
        Self(uuid::Uuid::new_v4().to_string())
    }

    /// Create a checkpoint ID from an existing string.
    ///
    /// # Example
    ///
    /// ```
    /// use ralph::checkpoint::CheckpointId;
    ///
    /// let id = CheckpointId::from_string("abc-123");
    /// assert_eq!(id.as_str(), "abc-123");
    /// ```
    #[must_use]
    pub fn from_string(s: impl Into<String>) -> Self {
        Self(s.into())
    }

    /// Get the ID as a string slice.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl Default for CheckpointId {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Display for CheckpointId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

// ============================================================================
// Quality Metrics
// ============================================================================

/// Snapshot of code quality metrics at a point in time.
///
/// Used to compare quality between checkpoints and detect regressions.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct QualityMetrics {
    /// Number of clippy warnings.
    pub clippy_warnings: u32,

    /// Total number of tests.
    pub test_total: u32,

    /// Number of passing tests.
    pub test_passed: u32,

    /// Number of failing tests.
    pub test_failed: u32,

    /// Number of security issues found.
    pub security_issues: u32,

    /// Number of #[allow(...)] annotations.
    pub allow_annotations: u32,

    /// Number of TODO/FIXME comments.
    pub todo_comments: u32,

    /// Optional: lines of code (for tracking code growth).
    pub lines_of_code: Option<u32>,

    /// Optional: test coverage percentage (0.0 - 100.0).
    pub test_coverage: Option<f32>,
}

impl QualityMetrics {
    /// Create new empty quality metrics.
    ///
    /// # Example
    ///
    /// ```
    /// use ralph::checkpoint::QualityMetrics;
    ///
    /// let metrics = QualityMetrics::new();
    /// assert_eq!(metrics.clippy_warnings, 0);
    /// ```
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Set clippy warning count.
    #[must_use]
    pub fn with_clippy_warnings(mut self, count: u32) -> Self {
        self.clippy_warnings = count;
        self
    }

    /// Set test counts.
    #[must_use]
    pub fn with_test_counts(mut self, total: u32, passed: u32, failed: u32) -> Self {
        self.test_total = total;
        self.test_passed = passed;
        self.test_failed = failed;
        self
    }

    /// Set security issue count.
    #[must_use]
    pub fn with_security_issues(mut self, count: u32) -> Self {
        self.security_issues = count;
        self
    }

    /// Set allow annotation count.
    #[must_use]
    pub fn with_allow_annotations(mut self, count: u32) -> Self {
        self.allow_annotations = count;
        self
    }

    /// Set TODO/FIXME comment count.
    #[must_use]
    pub fn with_todo_comments(mut self, count: u32) -> Self {
        self.todo_comments = count;
        self
    }

    /// Set lines of code.
    #[must_use]
    pub fn with_lines_of_code(mut self, loc: u32) -> Self {
        self.lines_of_code = Some(loc);
        self
    }

    /// Set test coverage percentage.
    #[must_use]
    pub fn with_test_coverage(mut self, coverage: f32) -> Self {
        self.test_coverage = Some(coverage.clamp(0.0, 100.0));
        self
    }

    /// Calculate test pass rate (0.0 - 1.0).
    ///
    /// Returns `None` if there are no tests.
    #[must_use]
    pub fn test_pass_rate(&self) -> Option<f32> {
        if self.test_total == 0 {
            None
        } else {
            Some(self.test_passed as f32 / self.test_total as f32)
        }
    }

    /// Check if metrics indicate all quality gates would pass.
    #[must_use]
    pub fn all_gates_passing(&self) -> bool {
        self.clippy_warnings == 0
            && self.test_failed == 0
            && self.security_issues == 0
            && self.allow_annotations == 0
    }

    /// Check if these metrics are worse than a reference, given thresholds.
    ///
    /// Returns `true` if any metric has regressed beyond acceptable bounds.
    ///
    /// # Example
    ///
    /// ```
    /// use ralph::checkpoint::{QualityMetrics, RegressionThresholds};
    ///
    /// let baseline = QualityMetrics::new()
    ///     .with_clippy_warnings(0)
    ///     .with_test_counts(10, 10, 0);
    ///
    /// let current = QualityMetrics::new()
    ///     .with_clippy_warnings(5)
    ///     .with_test_counts(10, 8, 2);
    ///
    /// let thresholds = RegressionThresholds::default();
    /// assert!(current.is_worse_than(&baseline, &thresholds));
    /// ```
    #[must_use]
    pub fn is_worse_than(
        &self,
        baseline: &QualityMetrics,
        thresholds: &RegressionThresholds,
    ) -> bool {
        // Check absolute regressions
        if self.clippy_warnings > baseline.clippy_warnings + thresholds.max_clippy_increase {
            return true;
        }

        if self.test_failed > baseline.test_failed + thresholds.max_test_failures_increase {
            return true;
        }

        if self.security_issues > baseline.security_issues + thresholds.max_security_increase {
            return true;
        }

        if self.allow_annotations > baseline.allow_annotations + thresholds.max_allow_increase {
            return true;
        }

        // Check percentage regressions
        if let (Some(baseline_rate), Some(current_rate)) =
            (baseline.test_pass_rate(), self.test_pass_rate())
        {
            let regression_pct = (baseline_rate - current_rate) * 100.0;
            if regression_pct > thresholds.max_test_pass_rate_drop_pct {
                return true;
            }
        }

        // Check coverage regression
        if let (Some(baseline_cov), Some(current_cov)) =
            (baseline.test_coverage, self.test_coverage)
        {
            let coverage_drop = baseline_cov - current_cov;
            if coverage_drop > thresholds.max_coverage_drop_pct {
                return true;
            }
        }

        false
    }

    /// Calculate the regression percentage compared to baseline.
    ///
    /// Returns a value >= 0 where higher means worse regression.
    /// 0 means equal or better than baseline.
    #[must_use]
    pub fn regression_score(&self, baseline: &QualityMetrics) -> f32 {
        let mut score = 0.0;

        // Clippy regression (each warning = 5 points)
        if self.clippy_warnings > baseline.clippy_warnings {
            score += (self.clippy_warnings - baseline.clippy_warnings) as f32 * 5.0;
        }

        // Test failure regression (each failure = 10 points)
        if self.test_failed > baseline.test_failed {
            score += (self.test_failed - baseline.test_failed) as f32 * 10.0;
        }

        // Security regression (each issue = 20 points)
        if self.security_issues > baseline.security_issues {
            score += (self.security_issues - baseline.security_issues) as f32 * 20.0;
        }

        // Allow annotation regression (each = 3 points)
        if self.allow_annotations > baseline.allow_annotations {
            score += (self.allow_annotations - baseline.allow_annotations) as f32 * 3.0;
        }

        // Test pass rate drop (1 point per 1% drop)
        if let (Some(baseline_rate), Some(current_rate)) =
            (baseline.test_pass_rate(), self.test_pass_rate())
        {
            if current_rate < baseline_rate {
                score += (baseline_rate - current_rate) * 100.0;
            }
        }

        score
    }

    /// Format a summary of the metrics.
    #[must_use]
    pub fn summary(&self) -> String {
        let mut parts = Vec::new();

        if self.clippy_warnings > 0 {
            parts.push(format!("{} clippy warnings", self.clippy_warnings));
        }

        parts.push(format!(
            "{}/{} tests passing",
            self.test_passed, self.test_total
        ));

        if self.test_failed > 0 {
            parts.push(format!("{} failing", self.test_failed));
        }

        if self.security_issues > 0 {
            parts.push(format!("{} security issues", self.security_issues));
        }

        if self.allow_annotations > 0 {
            parts.push(format!("{} #[allow]", self.allow_annotations));
        }

        if let Some(cov) = self.test_coverage {
            parts.push(format!("{:.1}% coverage", cov));
        }

        parts.join(", ")
    }
}

// ============================================================================
// Regression Thresholds
// ============================================================================

/// Thresholds for determining when quality has regressed too much.
///
/// These values define acceptable bounds for metric changes.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RegressionThresholds {
    /// Maximum allowed increase in clippy warnings.
    pub max_clippy_increase: u32,

    /// Maximum allowed increase in test failures.
    pub max_test_failures_increase: u32,

    /// Maximum allowed increase in security issues.
    pub max_security_increase: u32,

    /// Maximum allowed increase in `#[allow]` annotations.
    pub max_allow_increase: u32,

    /// Maximum allowed drop in test pass rate (percentage points).
    pub max_test_pass_rate_drop_pct: f32,

    /// Maximum allowed drop in test coverage (percentage points).
    pub max_coverage_drop_pct: f32,

    /// Minimum regression score to trigger rollback.
    pub rollback_threshold_score: f32,
}

impl Default for RegressionThresholds {
    fn default() -> Self {
        Self {
            max_clippy_increase: 0,           // Zero tolerance for new warnings
            max_test_failures_increase: 0,    // Zero tolerance for new failures
            max_security_increase: 0,         // Zero tolerance for security issues
            max_allow_increase: 0,            // Zero tolerance for #[allow]
            max_test_pass_rate_drop_pct: 5.0, // Allow 5% drop (for test refactoring)
            max_coverage_drop_pct: 5.0,       // Allow 5% coverage drop
            rollback_threshold_score: 50.0,   // Rollback if regression score >= 50
        }
    }
}

impl RegressionThresholds {
    /// Create new thresholds with default values.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Create strict thresholds (zero tolerance).
    #[must_use]
    pub fn strict() -> Self {
        Self {
            max_clippy_increase: 0,
            max_test_failures_increase: 0,
            max_security_increase: 0,
            max_allow_increase: 0,
            max_test_pass_rate_drop_pct: 0.0,
            max_coverage_drop_pct: 0.0,
            rollback_threshold_score: 10.0,
        }
    }

    /// Create lenient thresholds (for development/exploration).
    #[must_use]
    pub fn lenient() -> Self {
        Self {
            max_clippy_increase: 5,
            max_test_failures_increase: 2,
            max_security_increase: 0, // Still strict on security
            max_allow_increase: 3,
            max_test_pass_rate_drop_pct: 10.0,
            max_coverage_drop_pct: 10.0,
            rollback_threshold_score: 100.0,
        }
    }

    /// Set maximum clippy warning increase.
    #[must_use]
    pub fn with_max_clippy_increase(mut self, count: u32) -> Self {
        self.max_clippy_increase = count;
        self
    }

    /// Set maximum test failure increase.
    #[must_use]
    pub fn with_max_test_failures_increase(mut self, count: u32) -> Self {
        self.max_test_failures_increase = count;
        self
    }

    /// Set rollback threshold score.
    #[must_use]
    pub fn with_rollback_threshold(mut self, score: f32) -> Self {
        self.rollback_threshold_score = score;
        self
    }
}

// ============================================================================
// Language Regression
// ============================================================================

/// Result of per-language regression analysis.
///
/// Contains information about whether a specific language has regressed
/// and details about the regression.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LanguageRegression {
    /// The language being analyzed.
    pub language: Language,

    /// Whether this language has a regression.
    pub has_regression: bool,

    /// The regression score (0 = no regression, higher = worse).
    pub regression_score: f32,

    /// Specific metrics that regressed.
    pub regressed_metrics: Vec<String>,

    /// Summary of the regression.
    pub summary: String,
}

impl LanguageRegression {
    /// Create a new language regression result indicating no regression.
    #[must_use]
    pub fn no_regression(language: Language) -> Self {
        Self {
            language,
            has_regression: false,
            regression_score: 0.0,
            regressed_metrics: Vec::new(),
            summary: String::new(),
        }
    }

    /// Create a new language regression result with detected regression.
    #[must_use]
    pub fn with_regression(
        language: Language,
        score: f32,
        regressed_metrics: Vec<String>,
        summary: String,
    ) -> Self {
        Self {
            language,
            has_regression: true,
            regression_score: score,
            regressed_metrics,
            summary,
        }
    }
}

// ============================================================================
// Checkpoint
// ============================================================================

/// A snapshot of code state at a specific point in time.
///
/// Captures git commit hash, quality metrics, and optional task tracker state.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Checkpoint {
    /// Unique identifier for this checkpoint.
    pub id: CheckpointId,

    /// When the checkpoint was created.
    pub created_at: DateTime<Utc>,

    /// Human-readable description of why checkpoint was created.
    pub description: String,

    /// Git commit hash at time of checkpoint.
    pub git_hash: String,

    /// Git branch name.
    pub git_branch: String,

    /// Quality metrics at time of checkpoint (aggregated).
    pub metrics: QualityMetrics,

    /// Per-language quality metrics for polyglot projects.
    ///
    /// Maps each detected language to its specific quality metrics,
    /// enabling language-specific regression detection and reporting.
    #[serde(default)]
    pub metrics_by_language: HashMap<Language, QualityMetrics>,

    /// Serialized task tracker state (if available).
    pub task_tracker_state: Option<String>,

    /// Iteration number when checkpoint was created.
    pub iteration: u32,

    /// Whether this checkpoint has been marked as known-good (verified).
    pub verified: bool,

    /// Optional tags for categorization.
    pub tags: Vec<String>,

    /// List of files modified since the previous checkpoint.
    pub files_modified: Vec<String>,
}

impl Checkpoint {
    /// Create a new checkpoint.
    ///
    /// # Arguments
    ///
    /// * `description` - Human-readable description
    /// * `git_hash` - Git commit hash
    /// * `git_branch` - Git branch name
    /// * `metrics` - Quality metrics at this point
    /// * `iteration` - Current iteration number
    ///
    /// # Example
    ///
    /// ```
    /// use ralph::checkpoint::{Checkpoint, QualityMetrics};
    ///
    /// let checkpoint = Checkpoint::new(
    ///     "All tests passing",
    ///     "abc123",
    ///     "main",
    ///     QualityMetrics::new().with_test_counts(10, 10, 0),
    ///     5,
    /// );
    /// assert_eq!(checkpoint.description, "All tests passing");
    /// assert!(!checkpoint.verified);
    /// ```
    #[must_use]
    pub fn new(
        description: impl Into<String>,
        git_hash: impl Into<String>,
        git_branch: impl Into<String>,
        metrics: QualityMetrics,
        iteration: u32,
    ) -> Self {
        Self {
            id: CheckpointId::new(),
            created_at: Utc::now(),
            description: description.into(),
            git_hash: git_hash.into(),
            git_branch: git_branch.into(),
            metrics,
            metrics_by_language: HashMap::new(),
            task_tracker_state: None,
            iteration,
            verified: false,
            tags: Vec::new(),
            files_modified: Vec::new(),
        }
    }

    /// Attach task tracker state to checkpoint.
    #[must_use]
    pub fn with_task_tracker_state(mut self, state: impl Into<String>) -> Self {
        self.task_tracker_state = Some(state.into());
        self
    }

    /// Mark checkpoint as verified (known-good).
    #[must_use]
    pub fn mark_verified(mut self) -> Self {
        self.verified = true;
        self
    }

    /// Add a tag to the checkpoint.
    #[must_use]
    pub fn with_tag(mut self, tag: impl Into<String>) -> Self {
        self.tags.push(tag.into());
        self
    }

    /// Add multiple tags.
    #[must_use]
    pub fn with_tags(mut self, tags: impl IntoIterator<Item = impl Into<String>>) -> Self {
        self.tags.extend(tags.into_iter().map(Into::into));
        self
    }

    /// Set the list of files modified since the previous checkpoint.
    ///
    /// # Example
    ///
    /// ```
    /// use ralph::checkpoint::{Checkpoint, QualityMetrics};
    ///
    /// let checkpoint = Checkpoint::new(
    ///     "Fixed bug",
    ///     "abc123",
    ///     "main",
    ///     QualityMetrics::new(),
    ///     1,
    /// ).with_files_modified(vec!["src/lib.rs".to_string(), "src/main.rs".to_string()]);
    ///
    /// assert_eq!(checkpoint.files_modified.len(), 2);
    /// ```
    #[must_use]
    pub fn with_files_modified(mut self, files: Vec<String>) -> Self {
        self.files_modified = files;
        self
    }

    /// Add a single file to the modified list.
    #[must_use]
    pub fn with_file_modified(mut self, file: impl Into<String>) -> Self {
        self.files_modified.push(file.into());
        self
    }

    /// Check if checkpoint has a specific tag.
    #[must_use]
    pub fn has_tag(&self, tag: &str) -> bool {
        self.tags.iter().any(|t| t == tag)
    }

    /// Get age of checkpoint in seconds.
    #[must_use]
    pub fn age_seconds(&self) -> i64 {
        (Utc::now() - self.created_at).num_seconds()
    }

    /// Format a summary line for display.
    #[must_use]
    pub fn summary(&self) -> String {
        let verified_marker = if self.verified { " ✓" } else { "" };
        let tags_str = if self.tags.is_empty() {
            String::new()
        } else {
            format!(" [{}]", self.tags.join(", "))
        };

        format!(
            "{}:{:.8} - {} (iter {}){}{}",
            self.id, self.git_hash, self.description, self.iteration, verified_marker, tags_str
        )
    }

    // ------------------------------------------------------------------------
    // Language-Aware Quality Metrics (Phase 11.1)
    // ------------------------------------------------------------------------

    /// Set per-language quality metrics for polyglot projects.
    ///
    /// # Example
    ///
    /// ```rust
    /// use ralph::checkpoint::{Checkpoint, QualityMetrics};
    /// use ralph::Language;
    /// use std::collections::HashMap;
    ///
    /// let mut metrics_by_lang = HashMap::new();
    /// metrics_by_lang.insert(
    ///     Language::Rust,
    ///     QualityMetrics::new().with_test_counts(50, 50, 0),
    /// );
    ///
    /// let checkpoint = Checkpoint::new("Multi-lang", "abc", "main", QualityMetrics::new(), 1)
    ///     .with_language_metrics(metrics_by_lang);
    ///
    /// assert_eq!(checkpoint.metrics_by_language.len(), 1);
    /// ```
    #[must_use]
    pub fn with_language_metrics(
        mut self,
        metrics: HashMap<Language, QualityMetrics>,
    ) -> Self {
        self.metrics_by_language = metrics;
        self
    }

    /// Add quality metrics for a single language.
    ///
    /// # Example
    ///
    /// ```rust
    /// use ralph::checkpoint::{Checkpoint, QualityMetrics};
    /// use ralph::Language;
    ///
    /// let checkpoint = Checkpoint::new("Single lang", "abc", "main", QualityMetrics::new(), 1)
    ///     .with_metrics_for_language(
    ///         Language::Python,
    ///         QualityMetrics::new().with_test_counts(30, 30, 0),
    ///     );
    ///
    /// assert!(checkpoint.metrics_by_language.contains_key(&Language::Python));
    /// ```
    #[must_use]
    pub fn with_metrics_for_language(mut self, language: Language, metrics: QualityMetrics) -> Self {
        self.metrics_by_language.insert(language, metrics);
        self
    }

    /// Analyze per-language regressions compared to a baseline checkpoint.
    ///
    /// Returns a map of languages to their regression status. Only languages
    /// present in both checkpoints are compared.
    ///
    /// # Example
    ///
    /// ```rust
    /// use ralph::checkpoint::{Checkpoint, QualityMetrics, RegressionThresholds};
    /// use ralph::Language;
    /// use std::collections::HashMap;
    ///
    /// let mut baseline_metrics = HashMap::new();
    /// baseline_metrics.insert(Language::Rust, QualityMetrics::new().with_clippy_warnings(0));
    ///
    /// let mut current_metrics = HashMap::new();
    /// current_metrics.insert(Language::Rust, QualityMetrics::new().with_clippy_warnings(5));
    ///
    /// let baseline = Checkpoint::new("Baseline", "abc", "main", QualityMetrics::new(), 1)
    ///     .with_language_metrics(baseline_metrics);
    /// let current = Checkpoint::new("Current", "def", "main", QualityMetrics::new(), 2)
    ///     .with_language_metrics(current_metrics);
    ///
    /// let regressions = current.language_regressions(&baseline, &RegressionThresholds::default());
    /// assert!(regressions.get(&Language::Rust).unwrap().has_regression);
    /// ```
    #[must_use]
    pub fn language_regressions(
        &self,
        baseline: &Checkpoint,
        thresholds: &RegressionThresholds,
    ) -> HashMap<Language, LanguageRegression> {
        let mut results = HashMap::new();

        // Check each language in the current checkpoint
        for (language, current_metrics) in &self.metrics_by_language {
            // Only compare if the language exists in baseline
            if let Some(baseline_metrics) = baseline.metrics_by_language.get(language) {
                let regression = Self::analyze_language_regression(
                    *language,
                    current_metrics,
                    baseline_metrics,
                    thresholds,
                );
                results.insert(*language, regression);
            }
            // Languages not in baseline are new additions, not regressions
        }

        results
    }

    /// Check if any language has a regression compared to baseline.
    #[must_use]
    pub fn has_any_language_regression(
        &self,
        baseline: &Checkpoint,
        thresholds: &RegressionThresholds,
    ) -> bool {
        self.language_regressions(baseline, thresholds)
            .values()
            .any(|r| r.has_regression)
    }

    /// Generate a human-readable report of per-language regressions.
    ///
    /// Returns a formatted string listing all languages and their regression status.
    #[must_use]
    pub fn language_regression_report(
        &self,
        baseline: &Checkpoint,
        thresholds: &RegressionThresholds,
    ) -> String {
        let regressions = self.language_regressions(baseline, thresholds);

        if regressions.is_empty() {
            return "No per-language metrics to compare.".to_string();
        }

        let mut lines = Vec::new();
        lines.push("Per-Language Regression Report:".to_string());
        lines.push("─".repeat(40));

        // Sort languages for consistent output
        let mut sorted_langs: Vec<_> = regressions.keys().collect();
        sorted_langs.sort_by_key(|l| format!("{}", l));

        for language in sorted_langs {
            let regression = &regressions[language];
            let status = if regression.has_regression {
                format!("⚠ REGRESSION (score: {:.1})", regression.regression_score)
            } else {
                "✓ OK".to_string()
            };

            lines.push(format!("{}: {}", language, status));

            if regression.has_regression && !regression.regressed_metrics.is_empty() {
                for metric in &regression.regressed_metrics {
                    lines.push(format!("  - {}", metric));
                }
            }
        }

        lines.push("─".repeat(40));
        lines.join("\n")
    }

    /// Analyze regression for a single language.
    fn analyze_language_regression(
        language: Language,
        current: &QualityMetrics,
        baseline: &QualityMetrics,
        thresholds: &RegressionThresholds,
    ) -> LanguageRegression {
        let mut regressed_metrics = Vec::new();

        // Check clippy/lint warnings
        if current.clippy_warnings > baseline.clippy_warnings + thresholds.max_clippy_increase {
            regressed_metrics.push(format!(
                "lint warnings: {} → {} (+{})",
                baseline.clippy_warnings,
                current.clippy_warnings,
                current.clippy_warnings - baseline.clippy_warnings
            ));
        }

        // Check test failures
        if current.test_failed > baseline.test_failed + thresholds.max_test_failures_increase {
            regressed_metrics.push(format!(
                "test failures: {} → {} (+{})",
                baseline.test_failed,
                current.test_failed,
                current.test_failed - baseline.test_failed
            ));
        }

        // Check security issues
        if current.security_issues > baseline.security_issues + thresholds.max_security_increase {
            regressed_metrics.push(format!(
                "security issues: {} → {} (+{})",
                baseline.security_issues,
                current.security_issues,
                current.security_issues - baseline.security_issues
            ));
        }

        // Check test pass rate
        if let (Some(baseline_rate), Some(current_rate)) =
            (baseline.test_pass_rate(), current.test_pass_rate())
        {
            let drop_pct = (baseline_rate - current_rate) * 100.0;
            if drop_pct > thresholds.max_test_pass_rate_drop_pct {
                regressed_metrics.push(format!(
                    "test pass rate: {:.1}% → {:.1}% (-{:.1}%)",
                    baseline_rate * 100.0,
                    current_rate * 100.0,
                    drop_pct
                ));
            }
        }

        // Check coverage
        if let (Some(baseline_cov), Some(current_cov)) =
            (baseline.test_coverage, current.test_coverage)
        {
            let drop = baseline_cov - current_cov;
            if drop > thresholds.max_coverage_drop_pct {
                regressed_metrics.push(format!(
                    "coverage: {:.1}% → {:.1}% (-{:.1}%)",
                    baseline_cov, current_cov, drop
                ));
            }
        }

        if regressed_metrics.is_empty() {
            LanguageRegression::no_regression(language)
        } else {
            let score = current.regression_score(baseline);
            let summary = format!(
                "{} has {} regressed metric(s)",
                language,
                regressed_metrics.len()
            );
            LanguageRegression::with_regression(language, score, regressed_metrics, summary)
        }
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    // ------------------------------------------------------------------------
    // CheckpointId tests
    // ------------------------------------------------------------------------

    #[test]
    fn test_checkpoint_id_new_creates_unique_ids() {
        let id1 = CheckpointId::new();
        let id2 = CheckpointId::new();
        assert_ne!(id1, id2);
    }

    #[test]
    fn test_checkpoint_id_from_str() {
        let id = CheckpointId::from_string("test-id-123");
        assert_eq!(id.as_str(), "test-id-123");
        assert_eq!(format!("{}", id), "test-id-123");
    }

    #[test]
    fn test_checkpoint_id_default_creates_new() {
        let id = CheckpointId::default();
        assert!(!id.as_str().is_empty());
    }

    // ------------------------------------------------------------------------
    // QualityMetrics tests
    // ------------------------------------------------------------------------

    #[test]
    fn test_quality_metrics_new_has_zero_defaults() {
        let metrics = QualityMetrics::new();
        assert_eq!(metrics.clippy_warnings, 0);
        assert_eq!(metrics.test_total, 0);
        assert_eq!(metrics.test_passed, 0);
        assert_eq!(metrics.test_failed, 0);
        assert_eq!(metrics.security_issues, 0);
        assert_eq!(metrics.allow_annotations, 0);
        assert_eq!(metrics.todo_comments, 0);
        assert!(metrics.lines_of_code.is_none());
        assert!(metrics.test_coverage.is_none());
    }

    #[test]
    fn test_quality_metrics_builder_pattern() {
        let metrics = QualityMetrics::new()
            .with_clippy_warnings(3)
            .with_test_counts(100, 98, 2)
            .with_security_issues(1)
            .with_allow_annotations(5)
            .with_todo_comments(10)
            .with_lines_of_code(5000)
            .with_test_coverage(85.5);

        assert_eq!(metrics.clippy_warnings, 3);
        assert_eq!(metrics.test_total, 100);
        assert_eq!(metrics.test_passed, 98);
        assert_eq!(metrics.test_failed, 2);
        assert_eq!(metrics.security_issues, 1);
        assert_eq!(metrics.allow_annotations, 5);
        assert_eq!(metrics.todo_comments, 10);
        assert_eq!(metrics.lines_of_code, Some(5000));
        assert_eq!(metrics.test_coverage, Some(85.5));
    }

    #[test]
    fn test_quality_metrics_test_coverage_clamped() {
        let metrics = QualityMetrics::new().with_test_coverage(150.0);
        assert_eq!(metrics.test_coverage, Some(100.0));

        let metrics2 = QualityMetrics::new().with_test_coverage(-10.0);
        assert_eq!(metrics2.test_coverage, Some(0.0));
    }

    #[test]
    fn test_quality_metrics_test_pass_rate_with_tests() {
        let metrics = QualityMetrics::new().with_test_counts(100, 95, 5);
        assert_eq!(metrics.test_pass_rate(), Some(0.95));
    }

    #[test]
    fn test_quality_metrics_test_pass_rate_no_tests() {
        let metrics = QualityMetrics::new();
        assert!(metrics.test_pass_rate().is_none());
    }

    #[test]
    fn test_quality_metrics_all_gates_passing_true() {
        let metrics = QualityMetrics::new()
            .with_clippy_warnings(0)
            .with_test_counts(50, 50, 0)
            .with_security_issues(0)
            .with_allow_annotations(0);

        assert!(metrics.all_gates_passing());
    }

    #[test]
    fn test_quality_metrics_all_gates_passing_false_clippy() {
        let metrics = QualityMetrics::new()
            .with_clippy_warnings(1)
            .with_test_counts(50, 50, 0);

        assert!(!metrics.all_gates_passing());
    }

    #[test]
    fn test_quality_metrics_all_gates_passing_false_tests() {
        let metrics = QualityMetrics::new()
            .with_clippy_warnings(0)
            .with_test_counts(50, 49, 1);

        assert!(!metrics.all_gates_passing());
    }

    #[test]
    fn test_quality_metrics_is_worse_than_clippy_regression() {
        let baseline = QualityMetrics::new().with_clippy_warnings(0);
        let current = QualityMetrics::new().with_clippy_warnings(5);
        let thresholds = RegressionThresholds::default();

        assert!(current.is_worse_than(&baseline, &thresholds));
    }

    #[test]
    fn test_quality_metrics_is_worse_than_test_regression() {
        let baseline = QualityMetrics::new().with_test_counts(100, 100, 0);
        let current = QualityMetrics::new().with_test_counts(100, 95, 5);
        let thresholds = RegressionThresholds::default();

        assert!(current.is_worse_than(&baseline, &thresholds));
    }

    #[test]
    fn test_quality_metrics_is_worse_than_within_tolerance() {
        let baseline = QualityMetrics::new()
            .with_clippy_warnings(0)
            .with_test_counts(100, 100, 0);
        let current = QualityMetrics::new()
            .with_clippy_warnings(0)
            .with_test_counts(100, 100, 0);
        let thresholds = RegressionThresholds::default();

        assert!(!current.is_worse_than(&baseline, &thresholds));
    }

    #[test]
    fn test_quality_metrics_is_worse_than_lenient_thresholds() {
        let baseline = QualityMetrics::new().with_clippy_warnings(0);
        let current = QualityMetrics::new().with_clippy_warnings(3);
        let thresholds = RegressionThresholds::lenient();

        assert!(!current.is_worse_than(&baseline, &thresholds));
    }

    #[test]
    fn test_quality_metrics_regression_score_no_regression() {
        let baseline = QualityMetrics::new()
            .with_clippy_warnings(5)
            .with_test_counts(100, 95, 5);
        let current = QualityMetrics::new()
            .with_clippy_warnings(3) // Better
            .with_test_counts(100, 98, 2); // Better

        assert_eq!(current.regression_score(&baseline), 0.0);
    }

    #[test]
    fn test_quality_metrics_regression_score_with_regression() {
        let baseline = QualityMetrics::new()
            .with_clippy_warnings(0)
            .with_test_counts(100, 100, 0);
        let current = QualityMetrics::new()
            .with_clippy_warnings(2)
            .with_test_counts(100, 95, 5);

        let score = current.regression_score(&baseline);
        // 2 warnings * 5 = 10
        // 5 failures * 10 = 50
        // 5% pass rate drop = 5
        // Total = 65
        assert_eq!(score, 65.0);
    }

    #[test]
    fn test_quality_metrics_summary() {
        let metrics = QualityMetrics::new()
            .with_clippy_warnings(2)
            .with_test_counts(50, 48, 2)
            .with_security_issues(1)
            .with_test_coverage(85.0);

        let summary = metrics.summary();
        assert!(summary.contains("2 clippy warnings"));
        assert!(summary.contains("48/50 tests passing"));
        assert!(summary.contains("2 failing"));
        assert!(summary.contains("1 security issues"));
        assert!(summary.contains("85.0% coverage"));
    }

    // ------------------------------------------------------------------------
    // RegressionThresholds tests
    // ------------------------------------------------------------------------

    #[test]
    fn test_regression_thresholds_default() {
        let thresholds = RegressionThresholds::default();
        assert_eq!(thresholds.max_clippy_increase, 0);
        assert_eq!(thresholds.max_test_failures_increase, 0);
        assert_eq!(thresholds.max_security_increase, 0);
        assert_eq!(thresholds.rollback_threshold_score, 50.0);
    }

    #[test]
    fn test_regression_thresholds_strict() {
        let thresholds = RegressionThresholds::strict();
        assert_eq!(thresholds.max_test_pass_rate_drop_pct, 0.0);
        assert_eq!(thresholds.rollback_threshold_score, 10.0);
    }

    #[test]
    fn test_regression_thresholds_lenient() {
        let thresholds = RegressionThresholds::lenient();
        assert_eq!(thresholds.max_clippy_increase, 5);
        assert_eq!(thresholds.max_test_failures_increase, 2);
        assert_eq!(thresholds.rollback_threshold_score, 100.0);
    }

    #[test]
    fn test_regression_thresholds_builder_pattern() {
        let thresholds = RegressionThresholds::new()
            .with_max_clippy_increase(3)
            .with_max_test_failures_increase(1)
            .with_rollback_threshold(75.0);

        assert_eq!(thresholds.max_clippy_increase, 3);
        assert_eq!(thresholds.max_test_failures_increase, 1);
        assert_eq!(thresholds.rollback_threshold_score, 75.0);
    }

    // ------------------------------------------------------------------------
    // Checkpoint tests
    // ------------------------------------------------------------------------

    #[test]
    fn test_checkpoint_new() {
        let metrics = QualityMetrics::new().with_test_counts(10, 10, 0);
        let checkpoint = Checkpoint::new("Test checkpoint", "abc123def", "main", metrics, 5);

        assert_eq!(checkpoint.description, "Test checkpoint");
        assert_eq!(checkpoint.git_hash, "abc123def");
        assert_eq!(checkpoint.git_branch, "main");
        assert_eq!(checkpoint.iteration, 5);
        assert!(!checkpoint.verified);
        assert!(checkpoint.tags.is_empty());
        assert!(checkpoint.task_tracker_state.is_none());
    }

    #[test]
    fn test_checkpoint_with_task_tracker_state() {
        let checkpoint = Checkpoint::new("Test", "abc123", "main", QualityMetrics::new(), 1)
            .with_task_tracker_state(r#"{"tasks": []}"#);

        assert_eq!(
            checkpoint.task_tracker_state,
            Some(r#"{"tasks": []}"#.to_string())
        );
    }

    #[test]
    fn test_checkpoint_mark_verified() {
        let checkpoint =
            Checkpoint::new("Test", "abc", "main", QualityMetrics::new(), 1).mark_verified();

        assert!(checkpoint.verified);
    }

    #[test]
    fn test_checkpoint_with_tags() {
        let checkpoint = Checkpoint::new("Test", "abc", "main", QualityMetrics::new(), 1)
            .with_tag("release")
            .with_tags(vec!["stable", "v1.0"]);

        assert!(checkpoint.has_tag("release"));
        assert!(checkpoint.has_tag("stable"));
        assert!(checkpoint.has_tag("v1.0"));
        assert!(!checkpoint.has_tag("beta"));
    }

    #[test]
    fn test_checkpoint_with_files_modified() {
        let checkpoint = Checkpoint::new("Bug fix", "def456", "main", QualityMetrics::new(), 2)
            .with_files_modified(vec!["src/lib.rs".to_string(), "src/main.rs".to_string()]);

        assert_eq!(checkpoint.files_modified.len(), 2);
        assert!(checkpoint
            .files_modified
            .contains(&"src/lib.rs".to_string()));
        assert!(checkpoint
            .files_modified
            .contains(&"src/main.rs".to_string()));
    }

    #[test]
    fn test_checkpoint_with_file_modified_single() {
        let checkpoint = Checkpoint::new("Fix", "abc", "main", QualityMetrics::new(), 1)
            .with_file_modified("src/lib.rs")
            .with_file_modified("src/test.rs");

        assert_eq!(checkpoint.files_modified.len(), 2);
        assert_eq!(checkpoint.files_modified[0], "src/lib.rs");
        assert_eq!(checkpoint.files_modified[1], "src/test.rs");
    }

    #[test]
    fn test_checkpoint_files_modified_empty_by_default() {
        let checkpoint = Checkpoint::new("Test", "abc", "main", QualityMetrics::new(), 1);
        assert!(checkpoint.files_modified.is_empty());
    }

    #[test]
    fn test_checkpoint_summary() {
        let checkpoint = Checkpoint::new(
            "All tests pass",
            "abc123def",
            "main",
            QualityMetrics::new(),
            5,
        )
        .mark_verified()
        .with_tag("milestone");

        let summary = checkpoint.summary();
        assert!(summary.contains("abc123de"));
        assert!(summary.contains("All tests pass"));
        assert!(summary.contains("iter 5"));
        assert!(summary.contains("✓"));
        assert!(summary.contains("milestone"));
    }

    #[test]
    fn test_checkpoint_serialization_roundtrip() {
        let metrics = QualityMetrics::new()
            .with_clippy_warnings(2)
            .with_test_counts(50, 48, 2);

        let checkpoint = Checkpoint::new("Test", "abc123", "main", metrics, 3)
            .with_task_tracker_state("{}")
            .with_tag("test");

        let json = serde_json::to_string(&checkpoint).expect("serialize");
        let restored: Checkpoint = serde_json::from_str(&json).expect("deserialize");

        assert_eq!(restored.id, checkpoint.id);
        assert_eq!(restored.description, checkpoint.description);
        assert_eq!(restored.git_hash, checkpoint.git_hash);
        assert_eq!(restored.metrics.clippy_warnings, 2);
        assert_eq!(restored.tags, vec!["test"]);
    }

    // ------------------------------------------------------------------------
    // Phase 11.1: Language-Aware Quality Metrics Tests
    // ------------------------------------------------------------------------

    #[test]
    fn test_checkpoint_stores_per_language_test_counts() {
        use crate::Language;

        let mut metrics_by_lang = std::collections::HashMap::new();
        metrics_by_lang.insert(
            Language::Rust,
            QualityMetrics::new().with_test_counts(50, 50, 0),
        );
        metrics_by_lang.insert(
            Language::Python,
            QualityMetrics::new().with_test_counts(30, 28, 2),
        );

        let checkpoint = Checkpoint::new("Multi-lang tests", "abc123", "main", QualityMetrics::new(), 1)
            .with_language_metrics(metrics_by_lang);

        assert_eq!(checkpoint.metrics_by_language.len(), 2);

        let rust_metrics = checkpoint.metrics_by_language.get(&Language::Rust).unwrap();
        assert_eq!(rust_metrics.test_total, 50);
        assert_eq!(rust_metrics.test_passed, 50);
        assert_eq!(rust_metrics.test_failed, 0);

        let python_metrics = checkpoint.metrics_by_language.get(&Language::Python).unwrap();
        assert_eq!(python_metrics.test_total, 30);
        assert_eq!(python_metrics.test_passed, 28);
        assert_eq!(python_metrics.test_failed, 2);
    }

    #[test]
    fn test_checkpoint_stores_per_language_lint_warning_counts() {
        use crate::Language;

        let mut metrics_by_lang = std::collections::HashMap::new();
        metrics_by_lang.insert(
            Language::Rust,
            QualityMetrics::new().with_clippy_warnings(0),
        );
        metrics_by_lang.insert(
            Language::TypeScript,
            QualityMetrics::new().with_clippy_warnings(5), // ESLint warnings
        );
        metrics_by_lang.insert(
            Language::Python,
            QualityMetrics::new().with_clippy_warnings(3), // Ruff/flake8 warnings
        );

        let checkpoint = Checkpoint::new("Multi-lang lint", "def456", "main", QualityMetrics::new(), 2)
            .with_language_metrics(metrics_by_lang);

        assert_eq!(checkpoint.metrics_by_language.len(), 3);

        let rust_metrics = checkpoint.metrics_by_language.get(&Language::Rust).unwrap();
        assert_eq!(rust_metrics.clippy_warnings, 0);

        let ts_metrics = checkpoint.metrics_by_language.get(&Language::TypeScript).unwrap();
        assert_eq!(ts_metrics.clippy_warnings, 5);

        let py_metrics = checkpoint.metrics_by_language.get(&Language::Python).unwrap();
        assert_eq!(py_metrics.clippy_warnings, 3);
    }

    #[test]
    fn test_checkpoint_stores_per_language_coverage() {
        use crate::Language;

        let mut metrics_by_lang = std::collections::HashMap::new();
        metrics_by_lang.insert(
            Language::Rust,
            QualityMetrics::new().with_test_coverage(85.5),
        );
        metrics_by_lang.insert(
            Language::Python,
            QualityMetrics::new().with_test_coverage(92.0),
        );

        let checkpoint = Checkpoint::new("Coverage checkpoint", "ghi789", "main", QualityMetrics::new(), 3)
            .with_language_metrics(metrics_by_lang);

        let rust_metrics = checkpoint.metrics_by_language.get(&Language::Rust).unwrap();
        assert_eq!(rust_metrics.test_coverage, Some(85.5));

        let python_metrics = checkpoint.metrics_by_language.get(&Language::Python).unwrap();
        assert_eq!(python_metrics.test_coverage, Some(92.0));
    }

    #[test]
    fn test_checkpoint_per_language_metrics_empty_by_default() {
        let checkpoint = Checkpoint::new("Default", "abc", "main", QualityMetrics::new(), 1);
        assert!(checkpoint.metrics_by_language.is_empty());
    }

    #[test]
    fn test_checkpoint_add_single_language_metrics() {
        use crate::Language;

        let checkpoint = Checkpoint::new("Single lang", "abc", "main", QualityMetrics::new(), 1)
            .with_metrics_for_language(
                Language::Go,
                QualityMetrics::new()
                    .with_test_counts(20, 19, 1)
                    .with_clippy_warnings(2),
            );

        assert_eq!(checkpoint.metrics_by_language.len(), 1);
        let go_metrics = checkpoint.metrics_by_language.get(&Language::Go).unwrap();
        assert_eq!(go_metrics.test_total, 20);
        assert_eq!(go_metrics.clippy_warnings, 2);
    }

    #[test]
    fn test_per_language_regression_detection_single_language() {
        use crate::Language;

        let mut baseline_metrics = std::collections::HashMap::new();
        baseline_metrics.insert(
            Language::Rust,
            QualityMetrics::new().with_clippy_warnings(0).with_test_counts(50, 50, 0),
        );

        let mut current_metrics = std::collections::HashMap::new();
        current_metrics.insert(
            Language::Rust,
            QualityMetrics::new().with_clippy_warnings(5).with_test_counts(50, 48, 2),
        );

        let baseline = Checkpoint::new("Baseline", "abc", "main", QualityMetrics::new(), 1)
            .with_language_metrics(baseline_metrics);
        let current = Checkpoint::new("Current", "def", "main", QualityMetrics::new(), 2)
            .with_language_metrics(current_metrics);

        let thresholds = RegressionThresholds::default();
        let regressions = current.language_regressions(&baseline, &thresholds);

        assert!(regressions.contains_key(&Language::Rust));
        assert!(regressions.get(&Language::Rust).unwrap().has_regression);
    }

    #[test]
    fn test_per_language_regression_detection_multiple_languages() {
        use crate::Language;

        let mut baseline_metrics = std::collections::HashMap::new();
        baseline_metrics.insert(
            Language::Rust,
            QualityMetrics::new().with_clippy_warnings(0),
        );
        baseline_metrics.insert(
            Language::Python,
            QualityMetrics::new().with_clippy_warnings(0),
        );

        let mut current_metrics = std::collections::HashMap::new();
        current_metrics.insert(
            Language::Rust,
            QualityMetrics::new().with_clippy_warnings(0), // No regression
        );
        current_metrics.insert(
            Language::Python,
            QualityMetrics::new().with_clippy_warnings(10), // Regression
        );

        let baseline = Checkpoint::new("Baseline", "abc", "main", QualityMetrics::new(), 1)
            .with_language_metrics(baseline_metrics);
        let current = Checkpoint::new("Current", "def", "main", QualityMetrics::new(), 2)
            .with_language_metrics(current_metrics);

        let thresholds = RegressionThresholds::default();
        let regressions = current.language_regressions(&baseline, &thresholds);

        // Rust should have no regression
        assert!(!regressions.get(&Language::Rust).unwrap().has_regression);
        // Python should have regression
        assert!(regressions.get(&Language::Python).unwrap().has_regression);
    }

    #[test]
    fn test_per_language_regression_detection_new_language() {
        use crate::Language;

        let mut baseline_metrics = std::collections::HashMap::new();
        baseline_metrics.insert(
            Language::Rust,
            QualityMetrics::new().with_test_counts(50, 50, 0),
        );

        let mut current_metrics = std::collections::HashMap::new();
        current_metrics.insert(
            Language::Rust,
            QualityMetrics::new().with_test_counts(50, 50, 0),
        );
        // New language added
        current_metrics.insert(
            Language::Python,
            QualityMetrics::new().with_test_counts(10, 10, 0),
        );

        let baseline = Checkpoint::new("Baseline", "abc", "main", QualityMetrics::new(), 1)
            .with_language_metrics(baseline_metrics);
        let current = Checkpoint::new("Current", "def", "main", QualityMetrics::new(), 2)
            .with_language_metrics(current_metrics);

        let thresholds = RegressionThresholds::default();
        let regressions = current.language_regressions(&baseline, &thresholds);

        // New language without baseline should not count as regression
        // Python was added in current but not in baseline, so it shouldn't be in regressions
        assert!(
            !regressions.contains_key(&Language::Python)
                || !regressions[&Language::Python].has_regression
        );
    }

    #[test]
    fn test_has_any_language_regression() {
        use crate::Language;

        let mut baseline_metrics = std::collections::HashMap::new();
        baseline_metrics.insert(
            Language::Rust,
            QualityMetrics::new().with_test_counts(50, 50, 0),
        );

        let mut current_metrics = std::collections::HashMap::new();
        current_metrics.insert(
            Language::Rust,
            QualityMetrics::new().with_test_counts(50, 45, 5), // Regression
        );

        let baseline = Checkpoint::new("Baseline", "abc", "main", QualityMetrics::new(), 1)
            .with_language_metrics(baseline_metrics);
        let current = Checkpoint::new("Current", "def", "main", QualityMetrics::new(), 2)
            .with_language_metrics(current_metrics);

        let thresholds = RegressionThresholds::default();
        assert!(current.has_any_language_regression(&baseline, &thresholds));
    }

    #[test]
    fn test_no_language_regression_when_all_improved() {
        use crate::Language;

        let mut baseline_metrics = std::collections::HashMap::new();
        baseline_metrics.insert(
            Language::Rust,
            QualityMetrics::new().with_clippy_warnings(5).with_test_counts(50, 48, 2),
        );

        let mut current_metrics = std::collections::HashMap::new();
        current_metrics.insert(
            Language::Rust,
            QualityMetrics::new().with_clippy_warnings(2).with_test_counts(50, 50, 0), // Improved
        );

        let baseline = Checkpoint::new("Baseline", "abc", "main", QualityMetrics::new(), 1)
            .with_language_metrics(baseline_metrics);
        let current = Checkpoint::new("Current", "def", "main", QualityMetrics::new(), 2)
            .with_language_metrics(current_metrics);

        let thresholds = RegressionThresholds::default();
        assert!(!current.has_any_language_regression(&baseline, &thresholds));
    }

    #[test]
    fn test_language_regression_report_format() {
        use crate::Language;

        let mut baseline_metrics = std::collections::HashMap::new();
        baseline_metrics.insert(
            Language::Rust,
            QualityMetrics::new().with_clippy_warnings(0).with_test_counts(50, 50, 0),
        );
        baseline_metrics.insert(
            Language::Python,
            QualityMetrics::new().with_clippy_warnings(0),
        );

        let mut current_metrics = std::collections::HashMap::new();
        current_metrics.insert(
            Language::Rust,
            QualityMetrics::new().with_clippy_warnings(3).with_test_counts(50, 48, 2),
        );
        current_metrics.insert(
            Language::Python,
            QualityMetrics::new().with_clippy_warnings(5),
        );

        let baseline = Checkpoint::new("Baseline", "abc", "main", QualityMetrics::new(), 1)
            .with_language_metrics(baseline_metrics);
        let current = Checkpoint::new("Current", "def", "main", QualityMetrics::new(), 2)
            .with_language_metrics(current_metrics);

        let thresholds = RegressionThresholds::default();
        let report = current.language_regression_report(&baseline, &thresholds);

        // Report should include language names
        assert!(report.contains("Rust"));
        assert!(report.contains("Python"));
    }

    #[test]
    fn test_checkpoint_serialization_with_language_metrics() {
        use crate::Language;

        let mut metrics_by_lang = std::collections::HashMap::new();
        metrics_by_lang.insert(
            Language::Rust,
            QualityMetrics::new().with_clippy_warnings(0).with_test_counts(50, 50, 0),
        );
        metrics_by_lang.insert(
            Language::Python,
            QualityMetrics::new().with_test_coverage(85.0),
        );

        let checkpoint = Checkpoint::new("Serialization test", "abc123", "main", QualityMetrics::new(), 5)
            .with_language_metrics(metrics_by_lang);

        let json = serde_json::to_string(&checkpoint).expect("serialize");
        let restored: Checkpoint = serde_json::from_str(&json).expect("deserialize");

        assert_eq!(restored.metrics_by_language.len(), 2);
        assert!(restored.metrics_by_language.contains_key(&Language::Rust));
        assert!(restored.metrics_by_language.contains_key(&Language::Python));

        let rust_metrics = restored.metrics_by_language.get(&Language::Rust).unwrap();
        assert_eq!(rust_metrics.test_total, 50);
    }
}
