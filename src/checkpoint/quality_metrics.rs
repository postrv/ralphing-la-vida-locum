//! Quality metrics snapshot for checkpoint comparison.
//!
//! This module provides the `QualityMetrics` type for capturing code quality
//! metrics at a point in time, enabling regression detection and comparison.

use serde::{Deserialize, Serialize};

use super::thresholds::{LintRegressionResult, LintRegressionThresholds, RegressionThresholds};

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
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

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
}
