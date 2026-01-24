//! Regression thresholds and lint analysis types.
//!
//! This module provides types for configuring and detecting quality regressions
//! in checkpoint comparisons. Includes both general quality thresholds and
//! specific lint warning analysis.

use crate::Language;
use serde::{Deserialize, Serialize};
use std::fmt;

// ============================================================================
// Regression Thresholds
// ============================================================================

/// Thresholds for determining when quality has regressed too much.
///
/// These values define acceptable bounds for metric changes.
///
/// # Example
///
/// ```
/// use ralph::checkpoint::RegressionThresholds;
///
/// let thresholds = RegressionThresholds::strict();
/// assert_eq!(thresholds.max_clippy_increase, 0);
/// ```
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

// ============================================================================
// Warning Trend
// ============================================================================

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

#[cfg(test)]
mod tests {
    use super::*;

    // ========================================================================
    // RegressionThresholds Tests
    // ========================================================================

    #[test]
    fn test_regression_thresholds_default() {
        let thresholds = RegressionThresholds::default();
        assert_eq!(thresholds.max_clippy_increase, 0);
        assert_eq!(thresholds.max_test_failures_increase, 0);
        assert_eq!(thresholds.max_security_increase, 0);
    }

    #[test]
    fn test_regression_thresholds_strict() {
        let thresholds = RegressionThresholds::strict();
        assert_eq!(thresholds.max_clippy_increase, 0);
        assert_eq!(thresholds.rollback_threshold_score, 10.0);
    }

    #[test]
    fn test_regression_thresholds_lenient() {
        let thresholds = RegressionThresholds::lenient();
        assert_eq!(thresholds.max_clippy_increase, 5);
        assert_eq!(thresholds.max_test_failures_increase, 2);
    }

    #[test]
    fn test_regression_thresholds_builder() {
        let thresholds = RegressionThresholds::new()
            .with_max_clippy_increase(5)
            .with_max_test_failures_increase(2)
            .with_rollback_threshold(75.0);

        assert_eq!(thresholds.max_clippy_increase, 5);
        assert_eq!(thresholds.max_test_failures_increase, 2);
        assert_eq!(thresholds.rollback_threshold_score, 75.0);
    }

    // ========================================================================
    // LanguageRegression Tests
    // ========================================================================

    #[test]
    fn test_language_regression_no_regression() {
        let result = LanguageRegression::no_regression(Language::Rust);
        assert!(!result.has_regression);
        assert_eq!(result.regression_score, 0.0);
        assert!(result.regressed_metrics.is_empty());
    }

    #[test]
    fn test_language_regression_with_regression() {
        let result = LanguageRegression::with_regression(
            Language::Rust,
            25.0,
            vec!["clippy_warnings".to_string()],
            "Clippy warnings increased".to_string(),
        );
        assert!(result.has_regression);
        assert_eq!(result.regression_score, 25.0);
        assert_eq!(result.regressed_metrics.len(), 1);
    }

    // ========================================================================
    // LintRegressionSeverity Tests
    // ========================================================================

    #[test]
    fn test_lint_regression_severity_display() {
        assert_eq!(format!("{}", LintRegressionSeverity::None), "none");
        assert_eq!(format!("{}", LintRegressionSeverity::Warning), "warning");
        assert_eq!(format!("{}", LintRegressionSeverity::Rollback), "rollback");
    }

    #[test]
    fn test_lint_regression_severity_default() {
        let severity: LintRegressionSeverity = Default::default();
        assert_eq!(severity, LintRegressionSeverity::None);
    }

    // ========================================================================
    // WarningTrendDirection Tests
    // ========================================================================

    #[test]
    fn test_warning_trend_direction_display() {
        assert_eq!(
            format!("{}", WarningTrendDirection::Increasing),
            "increasing"
        );
        assert_eq!(
            format!("{}", WarningTrendDirection::Decreasing),
            "decreasing"
        );
        assert_eq!(format!("{}", WarningTrendDirection::Stable), "stable");
        assert_eq!(format!("{}", WarningTrendDirection::Unknown), "unknown");
    }

    // ========================================================================
    // LintRegressionThresholds Tests
    // ========================================================================

    #[test]
    fn test_lint_regression_thresholds_default() {
        let thresholds = LintRegressionThresholds::default();
        assert_eq!(thresholds.warning_threshold, 3);
        assert_eq!(thresholds.rollback_threshold, 10);
    }

    #[test]
    fn test_lint_regression_thresholds_strict() {
        let thresholds = LintRegressionThresholds::strict();
        assert_eq!(thresholds.warning_threshold, 0);
        assert_eq!(thresholds.rollback_threshold, 3);
    }

    #[test]
    fn test_lint_regression_thresholds_lenient() {
        let thresholds = LintRegressionThresholds::lenient();
        assert_eq!(thresholds.warning_threshold, 10);
        assert_eq!(thresholds.rollback_threshold, 25);
    }

    #[test]
    fn test_lint_regression_thresholds_builder() {
        let thresholds = LintRegressionThresholds::new()
            .with_warning_threshold(5)
            .with_rollback_threshold(15);

        assert_eq!(thresholds.warning_threshold, 5);
        assert_eq!(thresholds.rollback_threshold, 15);
    }

    // ========================================================================
    // LintRegressionResult Tests
    // ========================================================================

    #[test]
    fn test_lint_regression_result_no_regression() {
        let result = LintRegressionResult::no_regression(10, 8);
        assert_eq!(result.severity, LintRegressionSeverity::None);
        assert_eq!(result.warning_delta, 0);
        assert!(!result.has_regression());
        assert!(!result.should_rollback());
        assert!(result.message.contains("improved"));
    }

    #[test]
    fn test_lint_regression_result_unchanged() {
        let result = LintRegressionResult::no_regression(10, 10);
        assert_eq!(result.severity, LintRegressionSeverity::None);
        assert!(result.message.contains("unchanged"));
    }

    #[test]
    fn test_lint_regression_result_warning() {
        let result = LintRegressionResult::warning(10, 14);
        assert_eq!(result.severity, LintRegressionSeverity::Warning);
        assert_eq!(result.warning_delta, 4);
        assert!(result.has_regression());
        assert!(!result.should_rollback());
        assert!(result.message.contains("+4"));
    }

    #[test]
    fn test_lint_regression_result_rollback() {
        let result = LintRegressionResult::rollback(10, 25);
        assert_eq!(result.severity, LintRegressionSeverity::Rollback);
        assert_eq!(result.warning_delta, 15);
        assert!(result.has_regression());
        assert!(result.should_rollback());
        assert!(result.message.contains("Critical"));
    }
}
