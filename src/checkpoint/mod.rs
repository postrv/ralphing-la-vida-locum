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

    /// Check for lint warning regression with tiered severity.
    ///
    /// Returns a `LintRegressionResult` indicating whether the warning count
    /// has regressed, and if so, at what severity level.
    ///
    /// # Example
    ///
    /// ```
    /// use ralph::checkpoint::{QualityMetrics, LintRegressionThresholds, LintRegressionSeverity};
    ///
    /// let baseline = QualityMetrics::new().with_clippy_warnings(0);
    /// let current = QualityMetrics::new().with_clippy_warnings(5);
    /// let thresholds = LintRegressionThresholds::default();
    ///
    /// let result = current.check_lint_regression(&baseline, &thresholds);
    /// assert_eq!(result.severity, LintRegressionSeverity::Warning);
    /// assert_eq!(result.warning_delta, 5);
    /// ```
    #[must_use]
    pub fn check_lint_regression(
        &self,
        baseline: &QualityMetrics,
        thresholds: &LintRegressionThresholds,
    ) -> LintRegressionResult {
        let current = self.clippy_warnings;
        let baseline_count = baseline.clippy_warnings;

        // No regression if warnings stayed same or decreased
        if current <= baseline_count {
            return LintRegressionResult::no_regression(baseline_count, current);
        }

        let delta = current - baseline_count;

        // Determine severity based on thresholds
        if delta > thresholds.rollback_threshold {
            LintRegressionResult::rollback(baseline_count, current)
        } else if delta > thresholds.warning_threshold {
            LintRegressionResult::warning(baseline_count, current)
        } else {
            // Delta is within warning threshold, no action needed
            LintRegressionResult::no_regression(baseline_count, current)
        }
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
// Lint Regression Detection (Phase 11.2)
// ============================================================================

/// Severity level of a lint warning regression.
///
/// Used to differentiate between minor increases that warrant a warning
/// and large increases that should trigger an automatic rollback.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum LintRegressionSeverity {
    /// No regression detected (warnings stayed same or decreased).
    #[default]
    None,

    /// Minor regression that warrants a warning but not a rollback.
    ///
    /// This typically means 1-3 new warnings appeared.
    Warning,

    /// Major regression that should trigger an automatic rollback.
    ///
    /// This typically means many new warnings appeared (10+).
    Rollback,
}

impl fmt::Display for LintRegressionSeverity {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::None => write!(f, "none"),
            Self::Warning => write!(f, "warning"),
            Self::Rollback => write!(f, "rollback"),
        }
    }
}

/// Direction of a warning trend over time.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum WarningTrendDirection {
    /// Warnings are increasing over time.
    Increasing,

    /// Warnings are decreasing over time.
    Decreasing,

    /// Warnings are stable (no significant change).
    Stable,

    /// Not enough data points to determine trend.
    #[default]
    Unknown,
}

impl fmt::Display for WarningTrendDirection {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Increasing => write!(f, "increasing"),
            Self::Decreasing => write!(f, "decreasing"),
            Self::Stable => write!(f, "stable"),
            Self::Unknown => write!(f, "unknown"),
        }
    }
}

/// Configurable thresholds for lint warning regression detection.
///
/// These thresholds define when a lint warning increase should produce
/// a warning vs. trigger an automatic rollback.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LintRegressionThresholds {
    /// Maximum increase in warnings before a warning is issued.
    ///
    /// If the increase is <= this value, severity is `None`.
    /// If the increase is > this value but <= `rollback_threshold`, severity is `Warning`.
    pub warning_threshold: u32,

    /// Maximum increase in warnings before a rollback is triggered.
    ///
    /// If the increase is > this value, severity is `Rollback`.
    pub rollback_threshold: u32,
}

impl Default for LintRegressionThresholds {
    fn default() -> Self {
        Self {
            warning_threshold: 3,   // Allow up to 3 new warnings without action
            rollback_threshold: 10, // Rollback if 10+ new warnings
        }
    }
}

impl LintRegressionThresholds {
    /// Create new thresholds with default values.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Create strict thresholds (zero tolerance for warnings).
    #[must_use]
    pub fn strict() -> Self {
        Self {
            warning_threshold: 0,
            rollback_threshold: 3,
        }
    }

    /// Create lenient thresholds (for development/exploration).
    #[must_use]
    pub fn lenient() -> Self {
        Self {
            warning_threshold: 10,
            rollback_threshold: 25,
        }
    }

    /// Set the warning threshold.
    #[must_use]
    pub fn with_warning_threshold(mut self, threshold: u32) -> Self {
        self.warning_threshold = threshold;
        self
    }

    /// Set the rollback threshold.
    #[must_use]
    pub fn with_rollback_threshold(mut self, threshold: u32) -> Self {
        self.rollback_threshold = threshold;
        self
    }
}

/// Result of checking for lint warning regression.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LintRegressionResult {
    /// Severity of the regression.
    pub severity: LintRegressionSeverity,

    /// Number of warnings added (0 if improved or unchanged).
    pub warning_delta: u32,

    /// Baseline warning count.
    pub baseline_count: u32,

    /// Current warning count.
    pub current_count: u32,

    /// Human-readable message describing the result.
    pub message: String,
}

impl LintRegressionResult {
    /// Create a result indicating no regression.
    #[must_use]
    pub fn no_regression(baseline: u32, current: u32) -> Self {
        Self {
            severity: LintRegressionSeverity::None,
            warning_delta: 0,
            baseline_count: baseline,
            current_count: current,
            message: if current < baseline {
                format!(
                    "Lint warnings improved: {} → {} (-{})",
                    baseline,
                    current,
                    baseline - current
                )
            } else {
                format!("Lint warnings unchanged: {}", current)
            },
        }
    }

    /// Create a result indicating a warning-level regression.
    #[must_use]
    pub fn warning(baseline: u32, current: u32) -> Self {
        let delta = current.saturating_sub(baseline);
        Self {
            severity: LintRegressionSeverity::Warning,
            warning_delta: delta,
            baseline_count: baseline,
            current_count: current,
            message: format!(
                "Lint warning increase detected: {} → {} (+{}). Consider fixing before proceeding.",
                baseline, current, delta
            ),
        }
    }

    /// Create a result indicating a rollback-level regression.
    #[must_use]
    pub fn rollback(baseline: u32, current: u32) -> Self {
        let delta = current.saturating_sub(baseline);
        Self {
            severity: LintRegressionSeverity::Rollback,
            warning_delta: delta,
            baseline_count: baseline,
            current_count: current,
            message: format!(
                "Critical lint warning increase: {} → {} (+{}). Automatic rollback recommended.",
                baseline, current, delta
            ),
        }
    }

    /// Check if this result indicates any regression (warning or rollback).
    #[must_use]
    pub fn has_regression(&self) -> bool {
        self.severity != LintRegressionSeverity::None
    }

    /// Check if this result should trigger a rollback.
    #[must_use]
    pub fn should_rollback(&self) -> bool {
        self.severity == LintRegressionSeverity::Rollback
    }
}

/// A single data point in a warning trend.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WarningTrendPoint {
    /// Checkpoint ID this data point came from.
    pub checkpoint_id: CheckpointId,

    /// Iteration number of the checkpoint.
    pub iteration: u32,

    /// Warning count at this point.
    pub warning_count: u32,
}

/// Tracks lint warning counts across multiple checkpoints.
///
/// Used to analyze trends in code quality over time.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct WarningTrend {
    /// Warning counts at each checkpoint, ordered by iteration.
    pub data_points: Vec<WarningTrendPoint>,
}

impl WarningTrend {
    /// Create a new empty warning trend.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Create a warning trend from a list of checkpoints.
    ///
    /// Checkpoints are sorted by iteration number.
    #[must_use]
    pub fn from_checkpoints(checkpoints: &[Checkpoint]) -> Self {
        let mut data_points: Vec<WarningTrendPoint> = checkpoints
            .iter()
            .map(|cp| WarningTrendPoint {
                checkpoint_id: cp.id.clone(),
                iteration: cp.iteration,
                warning_count: cp.metrics.clippy_warnings,
            })
            .collect();

        // Sort by iteration
        data_points.sort_by_key(|p| p.iteration);

        Self { data_points }
    }

    /// Get the warning count from the first checkpoint.
    #[must_use]
    pub fn first_count(&self) -> Option<u32> {
        self.data_points.first().map(|p| p.warning_count)
    }

    /// Get the warning count from the last checkpoint.
    #[must_use]
    pub fn last_count(&self) -> Option<u32> {
        self.data_points.last().map(|p| p.warning_count)
    }

    /// Get the maximum warning count across all checkpoints.
    #[must_use]
    pub fn max_count(&self) -> u32 {
        self.data_points
            .iter()
            .map(|p| p.warning_count)
            .max()
            .unwrap_or(0)
    }

    /// Get the minimum warning count across all checkpoints.
    #[must_use]
    pub fn min_count(&self) -> u32 {
        self.data_points
            .iter()
            .map(|p| p.warning_count)
            .min()
            .unwrap_or(0)
    }

    /// Calculate the overall direction of the trend.
    #[must_use]
    pub fn direction(&self) -> WarningTrendDirection {
        if self.data_points.len() < 2 {
            return WarningTrendDirection::Unknown;
        }

        // Compare last third to first third for trend direction
        let len = self.data_points.len();
        let first_third_end = len / 3;
        let last_third_start = len - len / 3;

        // Handle small arrays
        let (first_third_end, last_third_start) = if len <= 3 {
            (1, len - 1)
        } else {
            (first_third_end.max(1), last_third_start.min(len - 1))
        };

        let first_avg: f64 = self.data_points[..first_third_end]
            .iter()
            .map(|p| p.warning_count as f64)
            .sum::<f64>()
            / first_third_end as f64;

        let last_avg: f64 = self.data_points[last_third_start..]
            .iter()
            .map(|p| p.warning_count as f64)
            .sum::<f64>()
            / (len - last_third_start) as f64;

        let diff = last_avg - first_avg;

        // Use a threshold of 0.5 to account for noise
        if diff > 0.5 {
            WarningTrendDirection::Increasing
        } else if diff < -0.5 {
            WarningTrendDirection::Decreasing
        } else {
            WarningTrendDirection::Stable
        }
    }

    /// Check if the trend shows improvement (decreasing warnings).
    #[must_use]
    pub fn is_improving(&self) -> bool {
        // Compare last two points if available
        if self.data_points.len() >= 2 {
            let last = self.data_points.last().unwrap().warning_count;
            let second_last = self.data_points[self.data_points.len() - 2].warning_count;
            last < second_last
        } else {
            false
        }
    }

    /// Get a summary string describing the trend.
    #[must_use]
    pub fn summary(&self) -> String {
        if self.data_points.is_empty() {
            return "No warning data available".to_string();
        }

        let direction = self.direction();
        let first = self.first_count().unwrap_or(0);
        let last = self.last_count().unwrap_or(0);

        format!(
            "Warning trend: {} → {} ({}, {} checkpoints)",
            first,
            last,
            direction,
            self.data_points.len()
        )
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
// Checkpoint Diff (Phase 11.3)
// ============================================================================

/// Represents the difference between two checkpoints.
///
/// Captures deltas in quality metrics and file changes between checkpoints,
/// enabling visualization of quality trends over time.
///
/// # Example
///
/// ```rust
/// use ralph::checkpoint::{Checkpoint, CheckpointDiff, QualityMetrics};
///
/// let from = Checkpoint::new(
///     "Before",
///     "abc123",
///     "main",
///     QualityMetrics::new().with_test_counts(50, 45, 5),
///     1,
/// );
/// let to = Checkpoint::new(
///     "After",
///     "def456",
///     "main",
///     QualityMetrics::new().with_test_counts(55, 54, 1),
///     2,
/// );
///
/// let diff = CheckpointDiff::compute(&from, &to);
/// assert_eq!(diff.test_total_delta, 5);
/// assert!(diff.is_improvement());
/// ```
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CheckpointDiff {
    /// ID of the "from" checkpoint.
    pub from_id: CheckpointId,

    /// ID of the "to" checkpoint.
    pub to_id: CheckpointId,

    /// Change in total test count (to - from).
    pub test_total_delta: i32,

    /// Change in passing test count (to - from).
    pub test_passed_delta: i32,

    /// Change in failing test count (to - from).
    pub test_failed_delta: i32,

    /// Change in clippy/lint warning count (to - from).
    pub clippy_warnings_delta: i32,

    /// Change in security issue count (to - from).
    pub security_issues_delta: i32,

    /// Change in allow annotation count (to - from).
    pub allow_annotations_delta: i32,

    /// Change in test coverage (to - from), if both checkpoints have coverage data.
    pub coverage_delta: Option<f32>,

    /// Files added in "to" that were not in "from".
    pub files_added: Vec<String>,

    /// Files removed from "from" that are not in "to".
    pub files_removed: Vec<String>,

    /// Files present in both checkpoints.
    pub files_unchanged: Vec<String>,

    /// All files that changed (union of files_modified from both checkpoints).
    pub files_changed: Vec<String>,

    /// Iteration delta (to - from).
    pub iteration_delta: i32,
}

impl CheckpointDiff {
    /// Compute the difference between two checkpoints.
    ///
    /// Calculates deltas for all quality metrics and analyzes file changes.
    ///
    /// # Arguments
    ///
    /// * `from` - The baseline checkpoint
    /// * `to` - The checkpoint to compare against the baseline
    ///
    /// # Returns
    ///
    /// A `CheckpointDiff` containing all computed deltas.
    #[must_use]
    pub fn compute(from: &Checkpoint, to: &Checkpoint) -> Self {
        // Calculate metric deltas
        let test_total_delta = to.metrics.test_total as i32 - from.metrics.test_total as i32;
        let test_passed_delta = to.metrics.test_passed as i32 - from.metrics.test_passed as i32;
        let test_failed_delta = to.metrics.test_failed as i32 - from.metrics.test_failed as i32;
        let clippy_warnings_delta =
            to.metrics.clippy_warnings as i32 - from.metrics.clippy_warnings as i32;
        let security_issues_delta =
            to.metrics.security_issues as i32 - from.metrics.security_issues as i32;
        let allow_annotations_delta =
            to.metrics.allow_annotations as i32 - from.metrics.allow_annotations as i32;

        // Calculate coverage delta
        let coverage_delta = match (from.metrics.test_coverage, to.metrics.test_coverage) {
            (Some(from_cov), Some(to_cov)) => Some(to_cov - from_cov),
            _ => None,
        };

        // Analyze file changes
        let from_files: std::collections::HashSet<_> = from.files_modified.iter().collect();
        let to_files: std::collections::HashSet<_> = to.files_modified.iter().collect();

        let files_added: Vec<String> = to_files
            .difference(&from_files)
            .map(|s| (*s).clone())
            .collect();
        let files_removed: Vec<String> = from_files
            .difference(&to_files)
            .map(|s| (*s).clone())
            .collect();
        let files_unchanged: Vec<String> = from_files
            .intersection(&to_files)
            .map(|s| (*s).clone())
            .collect();
        let files_changed: Vec<String> = from_files
            .union(&to_files)
            .map(|s| (*s).clone())
            .collect();

        let iteration_delta = to.iteration as i32 - from.iteration as i32;

        Self {
            from_id: from.id.clone(),
            to_id: to.id.clone(),
            test_total_delta,
            test_passed_delta,
            test_failed_delta,
            clippy_warnings_delta,
            security_issues_delta,
            allow_annotations_delta,
            coverage_delta,
            files_added,
            files_removed,
            files_unchanged,
            files_changed,
            iteration_delta,
        }
    }

    /// Check if there are no quality changes between checkpoints.
    #[must_use]
    pub fn is_unchanged(&self) -> bool {
        self.test_total_delta == 0
            && self.test_passed_delta == 0
            && self.test_failed_delta == 0
            && self.clippy_warnings_delta == 0
            && self.security_issues_delta == 0
            && self.allow_annotations_delta == 0
            && self.coverage_delta.is_none_or(|d| d.abs() < 0.01)
    }

    /// Check if the diff represents an improvement in code quality.
    ///
    /// An improvement is defined as:
    /// - Fewer or equal warnings
    /// - Fewer or equal test failures
    /// - More or equal passing tests
    /// - No increase in security issues
    /// - At least one metric actually improved
    #[must_use]
    pub fn is_improvement(&self) -> bool {
        // Must not have regressions
        let no_regressions = self.clippy_warnings_delta <= 0
            && self.test_failed_delta <= 0
            && self.security_issues_delta <= 0
            && self.allow_annotations_delta <= 0;

        // Must have at least one improvement
        let has_improvement = self.clippy_warnings_delta < 0
            || self.test_failed_delta < 0
            || self.test_passed_delta > 0
            || self.security_issues_delta < 0
            || self.allow_annotations_delta < 0
            || self.coverage_delta.is_some_and(|d| d > 0.0);

        no_regressions && has_improvement
    }

    /// Check if the diff represents a regression in code quality.
    ///
    /// A regression is defined as:
    /// - More warnings, OR
    /// - More test failures, OR
    /// - More security issues
    #[must_use]
    pub fn is_regression(&self) -> bool {
        self.clippy_warnings_delta > 0
            || self.test_failed_delta > 0
            || self.security_issues_delta > 0
            || self.coverage_delta.is_some_and(|d| d < -5.0) // >5% coverage drop
    }

    /// Generate a human-readable summary of the diff.
    #[must_use]
    pub fn summary(&self) -> String {
        let mut parts = Vec::new();

        // Test changes
        if self.test_total_delta != 0 || self.test_passed_delta != 0 {
            let delta_str = if self.test_total_delta >= 0 {
                format!("+{}", self.test_total_delta)
            } else {
                format!("{}", self.test_total_delta)
            };
            let passed_str = if self.test_passed_delta >= 0 {
                format!("+{}", self.test_passed_delta)
            } else {
                format!("{}", self.test_passed_delta)
            };
            parts.push(format!(
                "tests: {} total ({} passed)",
                delta_str, passed_str
            ));
        }

        // Warning changes
        if self.clippy_warnings_delta != 0 {
            let delta_str = if self.clippy_warnings_delta >= 0 {
                format!("+{}", self.clippy_warnings_delta)
            } else {
                format!("{}", self.clippy_warnings_delta)
            };
            parts.push(format!("warnings: {}", delta_str));
        }

        // Security issue changes
        if self.security_issues_delta != 0 {
            let delta_str = if self.security_issues_delta >= 0 {
                format!("+{}", self.security_issues_delta)
            } else {
                format!("{}", self.security_issues_delta)
            };
            parts.push(format!("security issues: {}", delta_str));
        }

        // Coverage changes
        if let Some(coverage_d) = self.coverage_delta {
            if coverage_d.abs() >= 0.01 {
                let delta_str = if coverage_d >= 0.0 {
                    format!("+{:.1}%", coverage_d)
                } else {
                    format!("{:.1}%", coverage_d)
                };
                parts.push(format!("coverage: {}", delta_str));
            }
        }

        // File changes
        if !self.files_added.is_empty() || !self.files_removed.is_empty() {
            let added = self.files_added.len();
            let removed = self.files_removed.len();
            parts.push(format!("files: +{} -{}", added, removed));
        }

        if parts.is_empty() {
            "No changes".to_string()
        } else {
            parts.join(", ")
        }
    }

    /// Format a detailed diff report.
    #[must_use]
    pub fn detailed_report(&self) -> String {
        let mut lines = Vec::new();

        lines.push("Checkpoint Diff Report".to_string());
        lines.push("─".repeat(50));
        lines.push(format!("From: {}", self.from_id));
        lines.push(format!("To:   {}", self.to_id));
        lines.push(format!("Iterations: {}", self.iteration_delta));
        lines.push(String::new());

        lines.push("Quality Metrics:".to_string());
        lines.push(format!(
            "  Tests total:     {:+}",
            self.test_total_delta
        ));
        lines.push(format!(
            "  Tests passed:    {:+}",
            self.test_passed_delta
        ));
        lines.push(format!(
            "  Tests failed:    {:+}",
            self.test_failed_delta
        ));
        lines.push(format!(
            "  Clippy warnings: {:+}",
            self.clippy_warnings_delta
        ));
        lines.push(format!(
            "  Security issues: {:+}",
            self.security_issues_delta
        ));

        if let Some(cov) = self.coverage_delta {
            lines.push(format!("  Coverage:        {:+.1}%", cov));
        }

        lines.push(String::new());
        lines.push("File Changes:".to_string());
        lines.push(format!("  Added:     {}", self.files_added.len()));
        lines.push(format!("  Removed:   {}", self.files_removed.len()));
        lines.push(format!("  Unchanged: {}", self.files_unchanged.len()));

        if !self.files_added.is_empty() {
            lines.push(String::new());
            lines.push("  Files Added:".to_string());
            for f in &self.files_added {
                lines.push(format!("    + {}", f));
            }
        }

        if !self.files_removed.is_empty() {
            lines.push(String::new());
            lines.push("  Files Removed:".to_string());
            for f in &self.files_removed {
                lines.push(format!("    - {}", f));
            }
        }

        lines.push(String::new());
        lines.push("─".repeat(50));

        let status = if self.is_improvement() {
            "IMPROVEMENT"
        } else if self.is_regression() {
            "REGRESSION"
        } else if self.is_unchanged() {
            "UNCHANGED"
        } else {
            "MIXED"
        };
        lines.push(format!("Status: {}", status));

        lines.join("\n")
    }
}

impl fmt::Display for CheckpointDiff {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.summary())
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

    // ------------------------------------------------------------------------
    // Phase 11.2: Lint Warning Regression Detection Tests
    // ------------------------------------------------------------------------

    #[test]
    fn test_lint_regression_increase_triggers_warning() {
        use super::{LintRegressionSeverity, LintRegressionThresholds};

        let baseline = QualityMetrics::new().with_clippy_warnings(0);
        let current = QualityMetrics::new().with_clippy_warnings(5); // 5 > warning_threshold(3)
        let thresholds = LintRegressionThresholds::default();

        let result = current.check_lint_regression(&baseline, &thresholds);

        assert_eq!(result.severity, LintRegressionSeverity::Warning);
        assert!(result.warning_delta > 0);
    }

    #[test]
    fn test_lint_regression_threshold_is_configurable() {
        use super::{LintRegressionSeverity, LintRegressionThresholds};

        let baseline = QualityMetrics::new().with_clippy_warnings(0);

        // With strict threshold (0 tolerance), even 2 warnings triggers rollback
        let current_small = QualityMetrics::new().with_clippy_warnings(2);
        let strict = LintRegressionThresholds::new()
            .with_warning_threshold(0)
            .with_rollback_threshold(1);
        let result_strict = current_small.check_lint_regression(&baseline, &strict);
        assert_eq!(result_strict.severity, LintRegressionSeverity::Rollback);

        // With lenient threshold (5 warning, 20 rollback), 7 is a warning, 3 is acceptable
        let current_medium = QualityMetrics::new().with_clippy_warnings(7);
        let lenient = LintRegressionThresholds::new()
            .with_warning_threshold(5)
            .with_rollback_threshold(20);
        let result_lenient = current_medium.check_lint_regression(&baseline, &lenient);
        assert_eq!(result_lenient.severity, LintRegressionSeverity::Warning);

        // 3 warnings with lenient thresholds should be acceptable (None)
        let current_small = QualityMetrics::new().with_clippy_warnings(3);
        let result_acceptable = current_small.check_lint_regression(&baseline, &lenient);
        assert_eq!(result_acceptable.severity, LintRegressionSeverity::None);
    }

    #[test]
    fn test_lint_regression_small_increase_produces_warning_not_rollback() {
        use super::{LintRegressionSeverity, LintRegressionThresholds};

        let baseline = QualityMetrics::new().with_clippy_warnings(5);
        let current = QualityMetrics::new().with_clippy_warnings(10); // +5 increase (> warning=3, <= rollback=10)
        let thresholds = LintRegressionThresholds::default(); // default: warning=3, rollback=10

        let result = current.check_lint_regression(&baseline, &thresholds);

        // 5 warning increase should produce Warning, not Rollback
        assert_eq!(result.severity, LintRegressionSeverity::Warning);
        assert_eq!(result.warning_delta, 5);
    }

    #[test]
    fn test_lint_regression_large_increase_triggers_rollback() {
        use super::{LintRegressionSeverity, LintRegressionThresholds};

        let baseline = QualityMetrics::new().with_clippy_warnings(0);
        let current = QualityMetrics::new().with_clippy_warnings(15); // +15 increase
        let thresholds = LintRegressionThresholds::default(); // default: rollback=10

        let result = current.check_lint_regression(&baseline, &thresholds);

        // 15 warning increase should trigger Rollback
        assert_eq!(result.severity, LintRegressionSeverity::Rollback);
        assert_eq!(result.warning_delta, 15);
    }

    #[test]
    fn test_lint_regression_no_increase_is_none() {
        use super::{LintRegressionSeverity, LintRegressionThresholds};

        let baseline = QualityMetrics::new().with_clippy_warnings(5);
        let current = QualityMetrics::new().with_clippy_warnings(5);
        let thresholds = LintRegressionThresholds::default();

        let result = current.check_lint_regression(&baseline, &thresholds);

        assert_eq!(result.severity, LintRegressionSeverity::None);
        assert_eq!(result.warning_delta, 0);
    }

    #[test]
    fn test_lint_regression_improvement_is_none() {
        use super::{LintRegressionSeverity, LintRegressionThresholds};

        let baseline = QualityMetrics::new().with_clippy_warnings(10);
        let current = QualityMetrics::new().with_clippy_warnings(5); // Improved!
        let thresholds = LintRegressionThresholds::default();

        let result = current.check_lint_regression(&baseline, &thresholds);

        // Improvement should not be flagged as regression
        assert_eq!(result.severity, LintRegressionSeverity::None);
        assert_eq!(result.warning_delta, 0); // Delta is 0 when improved
    }

    #[test]
    fn test_warning_trend_tracking_across_checkpoints() {
        use super::WarningTrend;

        // Create a series of checkpoints with varying warning counts
        let checkpoints = vec![
            Checkpoint::new("CP1", "abc1", "main", QualityMetrics::new().with_clippy_warnings(0), 1),
            Checkpoint::new("CP2", "abc2", "main", QualityMetrics::new().with_clippy_warnings(2), 2),
            Checkpoint::new("CP3", "abc3", "main", QualityMetrics::new().with_clippy_warnings(3), 3),
            Checkpoint::new("CP4", "abc4", "main", QualityMetrics::new().with_clippy_warnings(1), 4),
        ];

        let trend = WarningTrend::from_checkpoints(&checkpoints);

        // Verify trend data
        assert_eq!(trend.data_points.len(), 4);
        assert_eq!(trend.first_count(), Some(0));
        assert_eq!(trend.last_count(), Some(1));
        assert_eq!(trend.max_count(), 3);
        assert_eq!(trend.min_count(), 0);
        assert!(trend.is_improving()); // 0 -> 1 overall is slight increase, but 3 -> 1 at end is improving
    }

    #[test]
    fn test_warning_trend_direction() {
        use super::{WarningTrendDirection, WarningTrend};

        // Consistently increasing trend
        let increasing = vec![
            Checkpoint::new("CP1", "a", "main", QualityMetrics::new().with_clippy_warnings(0), 1),
            Checkpoint::new("CP2", "b", "main", QualityMetrics::new().with_clippy_warnings(5), 2),
            Checkpoint::new("CP3", "c", "main", QualityMetrics::new().with_clippy_warnings(10), 3),
        ];
        let trend = WarningTrend::from_checkpoints(&increasing);
        assert_eq!(trend.direction(), WarningTrendDirection::Increasing);

        // Consistently decreasing trend
        let decreasing = vec![
            Checkpoint::new("CP1", "a", "main", QualityMetrics::new().with_clippy_warnings(10), 1),
            Checkpoint::new("CP2", "b", "main", QualityMetrics::new().with_clippy_warnings(5), 2),
            Checkpoint::new("CP3", "c", "main", QualityMetrics::new().with_clippy_warnings(0), 3),
        ];
        let trend = WarningTrend::from_checkpoints(&decreasing);
        assert_eq!(trend.direction(), WarningTrendDirection::Decreasing);

        // Stable trend
        let stable = vec![
            Checkpoint::new("CP1", "a", "main", QualityMetrics::new().with_clippy_warnings(5), 1),
            Checkpoint::new("CP2", "b", "main", QualityMetrics::new().with_clippy_warnings(5), 2),
            Checkpoint::new("CP3", "c", "main", QualityMetrics::new().with_clippy_warnings(5), 3),
        ];
        let trend = WarningTrend::from_checkpoints(&stable);
        assert_eq!(trend.direction(), WarningTrendDirection::Stable);
    }

    #[test]
    fn test_warning_trend_empty_checkpoints() {
        use super::{WarningTrendDirection, WarningTrend};

        let empty: Vec<Checkpoint> = vec![];
        let trend = WarningTrend::from_checkpoints(&empty);

        assert!(trend.data_points.is_empty());
        assert_eq!(trend.first_count(), None);
        assert_eq!(trend.last_count(), None);
        assert_eq!(trend.direction(), WarningTrendDirection::Unknown);
    }

    #[test]
    fn test_warning_trend_single_checkpoint() {
        use super::{WarningTrendDirection, WarningTrend};

        let single = vec![
            Checkpoint::new("CP1", "abc", "main", QualityMetrics::new().with_clippy_warnings(5), 1),
        ];
        let trend = WarningTrend::from_checkpoints(&single);

        assert_eq!(trend.data_points.len(), 1);
        assert_eq!(trend.first_count(), Some(5));
        assert_eq!(trend.last_count(), Some(5));
        assert_eq!(trend.direction(), WarningTrendDirection::Unknown); // Need 2+ points
    }

    #[test]
    fn test_lint_regression_thresholds_serialization() {
        use super::LintRegressionThresholds;

        let thresholds = LintRegressionThresholds::new()
            .with_warning_threshold(5)
            .with_rollback_threshold(15);

        let json = serde_json::to_string(&thresholds).expect("serialize");
        let restored: LintRegressionThresholds = serde_json::from_str(&json).expect("deserialize");

        assert_eq!(restored.warning_threshold, 5);
        assert_eq!(restored.rollback_threshold, 15);
    }

    #[test]
    fn test_lint_regression_result_has_message() {
        use super::LintRegressionThresholds;

        let baseline = QualityMetrics::new().with_clippy_warnings(0);
        let current = QualityMetrics::new().with_clippy_warnings(5);
        let thresholds = LintRegressionThresholds::default();

        let result = current.check_lint_regression(&baseline, &thresholds);

        // Result should have a human-readable message
        assert!(!result.message.is_empty());
        assert!(result.message.contains("5")); // Should mention the count
    }

    // ------------------------------------------------------------------------
    // Phase 11.3: Checkpoint Diff Visualization Tests
    // ------------------------------------------------------------------------

    #[test]
    fn test_checkpoint_diff_shows_test_count_changes() {
        use super::CheckpointDiff;

        let from = Checkpoint::new(
            "Before",
            "abc123",
            "main",
            QualityMetrics::new().with_test_counts(50, 48, 2),
            1,
        );
        let to = Checkpoint::new(
            "After",
            "def456",
            "main",
            QualityMetrics::new().with_test_counts(55, 54, 1),
            2,
        );

        let diff = CheckpointDiff::compute(&from, &to);

        // Test total increased by 5
        assert_eq!(diff.test_total_delta, 5);
        // Tests passed increased by 6
        assert_eq!(diff.test_passed_delta, 6);
        // Test failures decreased by 1
        assert_eq!(diff.test_failed_delta, -1);
    }

    #[test]
    fn test_checkpoint_diff_shows_lint_warning_changes() {
        use super::CheckpointDiff;

        let from = Checkpoint::new(
            "Before",
            "abc123",
            "main",
            QualityMetrics::new().with_clippy_warnings(10),
            1,
        );
        let to = Checkpoint::new(
            "After",
            "def456",
            "main",
            QualityMetrics::new().with_clippy_warnings(3),
            2,
        );

        let diff = CheckpointDiff::compute(&from, &to);

        // Warnings decreased by 7
        assert_eq!(diff.clippy_warnings_delta, -7);
    }

    #[test]
    fn test_checkpoint_diff_shows_files_modified_between_checkpoints() {
        use super::CheckpointDiff;

        let from = Checkpoint::new("Before", "abc123", "main", QualityMetrics::new(), 1)
            .with_files_modified(vec!["src/lib.rs".to_string(), "src/main.rs".to_string()]);

        let to = Checkpoint::new("After", "def456", "main", QualityMetrics::new(), 2)
            .with_files_modified(vec![
                "src/lib.rs".to_string(),
                "src/new.rs".to_string(),
                "tests/test.rs".to_string(),
            ]);

        let diff = CheckpointDiff::compute(&from, &to);

        // Should capture files from the "to" checkpoint that differ from "from"
        assert!(!diff.files_changed.is_empty());
        // Files in 'to' that weren't in 'from' should show as added
        assert!(diff.files_added.contains(&"src/new.rs".to_string()));
        assert!(diff.files_added.contains(&"tests/test.rs".to_string()));
        // Files in 'from' that aren't in 'to' should show as removed
        assert!(diff.files_removed.contains(&"src/main.rs".to_string()));
        // Files in both should show as unchanged (or in files_changed if logic requires)
        assert!(diff.files_unchanged.contains(&"src/lib.rs".to_string()));
    }

    #[test]
    fn test_checkpoint_diff_is_json_serializable() {
        use super::CheckpointDiff;

        let from = Checkpoint::new(
            "Before",
            "abc123",
            "main",
            QualityMetrics::new()
                .with_test_counts(50, 50, 0)
                .with_clippy_warnings(5),
            1,
        );
        let to = Checkpoint::new(
            "After",
            "def456",
            "main",
            QualityMetrics::new()
                .with_test_counts(60, 58, 2)
                .with_clippy_warnings(3),
            2,
        );

        let diff = CheckpointDiff::compute(&from, &to);

        // Should be serializable to JSON
        let json = serde_json::to_string(&diff).expect("serialize to JSON");
        assert!(!json.is_empty());

        // Should be deserializable back
        let restored: CheckpointDiff = serde_json::from_str(&json).expect("deserialize from JSON");
        assert_eq!(restored.test_total_delta, diff.test_total_delta);
        assert_eq!(restored.clippy_warnings_delta, diff.clippy_warnings_delta);
    }

    #[test]
    fn test_checkpoint_diff_captures_checkpoint_ids() {
        use super::CheckpointDiff;

        let from = Checkpoint::new("Before", "abc123", "main", QualityMetrics::new(), 1);
        let to = Checkpoint::new("After", "def456", "main", QualityMetrics::new(), 2);

        let diff = CheckpointDiff::compute(&from, &to);

        // Diff should capture which checkpoints were compared
        assert_eq!(diff.from_id, from.id);
        assert_eq!(diff.to_id, to.id);
    }

    #[test]
    fn test_checkpoint_diff_shows_security_issue_changes() {
        use super::CheckpointDiff;

        let from = Checkpoint::new(
            "Before",
            "abc123",
            "main",
            QualityMetrics::new().with_security_issues(5),
            1,
        );
        let to = Checkpoint::new(
            "After",
            "def456",
            "main",
            QualityMetrics::new().with_security_issues(2),
            2,
        );

        let diff = CheckpointDiff::compute(&from, &to);

        // Security issues decreased by 3
        assert_eq!(diff.security_issues_delta, -3);
    }

    #[test]
    fn test_checkpoint_diff_shows_coverage_changes() {
        use super::CheckpointDiff;

        let from = Checkpoint::new(
            "Before",
            "abc123",
            "main",
            QualityMetrics::new().with_test_coverage(75.0),
            1,
        );
        let to = Checkpoint::new(
            "After",
            "def456",
            "main",
            QualityMetrics::new().with_test_coverage(85.5),
            2,
        );

        let diff = CheckpointDiff::compute(&from, &to);

        // Coverage increased by 10.5
        assert!(
            (diff.coverage_delta.unwrap() - 10.5).abs() < 0.01,
            "Coverage delta should be ~10.5, got {:?}",
            diff.coverage_delta
        );
    }

    #[test]
    fn test_checkpoint_diff_with_no_changes() {
        use super::CheckpointDiff;

        let metrics = QualityMetrics::new()
            .with_test_counts(50, 50, 0)
            .with_clippy_warnings(0);

        let from = Checkpoint::new("Before", "abc123", "main", metrics.clone(), 1);
        let to = Checkpoint::new("After", "def456", "main", metrics, 2);

        let diff = CheckpointDiff::compute(&from, &to);

        assert_eq!(diff.test_total_delta, 0);
        assert_eq!(diff.test_passed_delta, 0);
        assert_eq!(diff.test_failed_delta, 0);
        assert_eq!(diff.clippy_warnings_delta, 0);
        assert!(diff.is_unchanged());
    }

    #[test]
    fn test_checkpoint_diff_improvement_indicator() {
        use super::CheckpointDiff;

        // Improvement: fewer warnings, more passing tests
        let from = Checkpoint::new(
            "Before",
            "abc123",
            "main",
            QualityMetrics::new()
                .with_test_counts(50, 45, 5)
                .with_clippy_warnings(10),
            1,
        );
        let to = Checkpoint::new(
            "After",
            "def456",
            "main",
            QualityMetrics::new()
                .with_test_counts(50, 50, 0)
                .with_clippy_warnings(0),
            2,
        );

        let diff = CheckpointDiff::compute(&from, &to);

        assert!(diff.is_improvement());
    }

    #[test]
    fn test_checkpoint_diff_regression_indicator() {
        use super::CheckpointDiff;

        // Regression: more warnings, fewer passing tests
        let from = Checkpoint::new(
            "Before",
            "abc123",
            "main",
            QualityMetrics::new()
                .with_test_counts(50, 50, 0)
                .with_clippy_warnings(0),
            1,
        );
        let to = Checkpoint::new(
            "After",
            "def456",
            "main",
            QualityMetrics::new()
                .with_test_counts(50, 45, 5)
                .with_clippy_warnings(10),
            2,
        );

        let diff = CheckpointDiff::compute(&from, &to);

        assert!(diff.is_regression());
    }

    #[test]
    fn test_checkpoint_diff_summary_format() {
        use super::CheckpointDiff;

        let from = Checkpoint::new(
            "Before",
            "abc123",
            "main",
            QualityMetrics::new()
                .with_test_counts(50, 45, 5)
                .with_clippy_warnings(10),
            1,
        );
        let to = Checkpoint::new(
            "After",
            "def456",
            "main",
            QualityMetrics::new()
                .with_test_counts(55, 54, 1)
                .with_clippy_warnings(3),
            2,
        );

        let diff = CheckpointDiff::compute(&from, &to);

        let summary = diff.summary();
        assert!(!summary.is_empty());
        // Summary should mention key changes
        assert!(summary.contains("tests") || summary.contains("test"));
        assert!(summary.contains("warning") || summary.contains("clippy"));
    }
}
