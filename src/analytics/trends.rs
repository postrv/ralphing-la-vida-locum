//! Quality trend analysis and metrics tracking.
//!
//! This module provides types for capturing quality metrics over time
//! and analyzing trends for visualization and reporting.

use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

// ============================================================================
// Quality Metrics Collection
// ============================================================================

/// A timestamped snapshot of quality metrics for trend analysis.
///
/// This struct captures quality metrics at a point in time, enabling
/// historical tracking and trend analysis across sessions.
///
/// # Example
///
/// ```
/// use ralph::analytics::QualityMetricsSnapshot;
///
/// let snapshot = QualityMetricsSnapshot::new("session-123", 5)
///     .with_clippy_warnings(0)
///     .with_test_counts(42, 42, 0)
///     .with_security_issues(0);
///
/// assert!(snapshot.all_gates_passing());
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QualityMetricsSnapshot {
    /// Session ID when metrics were captured.
    pub session_id: String,
    /// Iteration number when metrics were captured.
    pub iteration: u32,
    /// Timestamp when metrics were captured.
    pub timestamp: DateTime<Utc>,
    /// Number of clippy warnings.
    pub clippy_warnings: u32,
    /// Total number of tests.
    pub test_total: u32,
    /// Number of passing tests.
    pub test_passed: u32,
    /// Number of failing tests.
    pub test_failed: u32,
    /// Number of security issues.
    pub security_issues: u32,
    /// Number of #[allow(...)] annotations.
    pub allow_annotations: u32,
    /// Optional task name being worked on.
    pub task_name: Option<String>,
}

impl QualityMetricsSnapshot {
    /// Create a new quality metrics snapshot.
    ///
    /// # Arguments
    ///
    /// * `session_id` - The session identifier
    /// * `iteration` - The iteration number
    ///
    /// # Example
    ///
    /// ```
    /// use ralph::analytics::QualityMetricsSnapshot;
    ///
    /// let snapshot = QualityMetricsSnapshot::new("session-1", 1);
    /// assert_eq!(snapshot.iteration, 1);
    /// ```
    #[must_use]
    pub fn new(session_id: impl Into<String>, iteration: u32) -> Self {
        Self {
            session_id: session_id.into(),
            iteration,
            timestamp: Utc::now(),
            clippy_warnings: 0,
            test_total: 0,
            test_passed: 0,
            test_failed: 0,
            security_issues: 0,
            allow_annotations: 0,
            task_name: None,
        }
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

    /// Set the task name.
    #[must_use]
    pub fn with_task_name(mut self, name: impl Into<String>) -> Self {
        self.task_name = Some(name.into());
        self
    }

    /// Check if all quality gates would pass with these metrics.
    #[must_use]
    pub fn all_gates_passing(&self) -> bool {
        self.clippy_warnings == 0
            && self.test_failed == 0
            && self.security_issues == 0
            && self.allow_annotations == 0
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
}

/// Direction of a quality trend.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TrendDirection {
    /// Quality is improving (fewer issues).
    Improving,
    /// Quality is stable (no significant change).
    Stable,
    /// Quality is degrading (more issues).
    Degrading,
}

/// Summary of quality trends over a time period.
///
/// # Example
///
/// ```
/// use ralph::analytics::{QualityTrend, TrendDirection};
///
/// let trend = QualityTrend::default();
/// assert_eq!(trend.overall, TrendDirection::Stable);
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QualityTrend {
    /// Overall trend direction.
    pub overall: TrendDirection,
    /// Number of snapshots analyzed.
    pub snapshot_count: usize,
    /// Average clippy warnings across period.
    pub avg_clippy_warnings: f32,
    /// Average test pass rate across period.
    pub avg_test_pass_rate: Option<f32>,
    /// Average security issues across period.
    pub avg_security_issues: f32,
    /// Change in clippy warnings (positive = more warnings = degrading).
    pub clippy_delta: i32,
    /// Change in test failures (positive = more failures = degrading).
    pub test_failures_delta: i32,
    /// Change in security issues (positive = more issues = degrading).
    pub security_delta: i32,
}

impl Default for QualityTrend {
    fn default() -> Self {
        Self {
            overall: TrendDirection::Stable,
            snapshot_count: 0,
            avg_clippy_warnings: 0.0,
            avg_test_pass_rate: None,
            avg_security_issues: 0.0,
            clippy_delta: 0,
            test_failures_delta: 0,
            security_delta: 0,
        }
    }
}

// ============================================================================
// Phase 16.3: Quality Trend Visualization Types
// ============================================================================

/// A single data point in a trend series.
///
/// Represents a timestamped value for tracking changes over time.
///
/// # Example
///
/// ```
/// use ralph::analytics::TrendPoint;
///
/// let point = TrendPoint::new(42);
/// assert_eq!(point.value, 42);
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrendPoint {
    /// When this data point was recorded.
    pub timestamp: DateTime<Utc>,
    /// The value at this point in time.
    pub value: i64,
}

impl TrendPoint {
    /// Create a new trend point with the current timestamp.
    ///
    /// # Arguments
    ///
    /// * `value` - The value for this data point
    #[must_use]
    pub fn new(value: i64) -> Self {
        Self {
            timestamp: Utc::now(),
            value,
        }
    }
}

/// The type of metric to display in a trend chart.
///
/// # Example
///
/// ```
/// use ralph::analytics::TrendMetric;
///
/// assert_eq!(TrendMetric::Warnings.label(), "Warnings");
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TrendMetric {
    /// Clippy warning count.
    Warnings,
    /// Total test count.
    TestCount,
    /// Test pass rate as percentage.
    TestPassRate,
    /// Commit count per session.
    Commits,
    /// Security issue count.
    SecurityIssues,
}

impl TrendMetric {
    /// Get the display label for this metric.
    #[must_use]
    pub fn label(&self) -> &'static str {
        match self {
            Self::Warnings => "Warnings",
            Self::TestCount => "Test Count",
            Self::TestPassRate => "Test Pass Rate (%)",
            Self::Commits => "Commits",
            Self::SecurityIssues => "Security Issues",
        }
    }
}

/// Aggregated trend data across sessions.
///
/// Contains multiple trend series suitable for visualization and analysis.
///
/// # Example
///
/// ```
/// use ralph::analytics::{TrendData, TrendMetric};
///
/// let trend_data = TrendData::default();
/// let chart = trend_data.render_ascii_chart(TrendMetric::Warnings, 40, 10);
/// ```
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct TrendData {
    /// Warning count trend (clippy warnings over time).
    pub warning_count_points: Vec<TrendPoint>,
    /// Test count trend (total tests over time).
    pub test_count_points: Vec<TrendPoint>,
    /// Test pass rate trend (percentage over time).
    pub test_pass_rate_points: Vec<TrendPoint>,
    /// Commit count trend (commits per session).
    pub commit_points: Vec<TrendPoint>,
    /// Security issue count trend.
    pub security_issue_points: Vec<TrendPoint>,
}

impl TrendData {
    /// Export trend data as JSON.
    ///
    /// # Errors
    ///
    /// Returns an error if JSON serialization fails.
    ///
    /// # Example
    ///
    /// ```
    /// use ralph::analytics::TrendData;
    ///
    /// let trend_data = TrendData::default();
    /// let json = trend_data.to_json().unwrap();
    /// assert!(json.contains("warning_count_points"));
    /// ```
    pub fn to_json(&self) -> Result<String> {
        serde_json::to_string_pretty(self).context("Failed to serialize trend data to JSON")
    }

    /// Get the data points for a specific metric.
    #[must_use]
    pub fn points_for_metric(&self, metric: TrendMetric) -> &[TrendPoint] {
        match metric {
            TrendMetric::Warnings => &self.warning_count_points,
            TrendMetric::TestCount => &self.test_count_points,
            TrendMetric::TestPassRate => &self.test_pass_rate_points,
            TrendMetric::Commits => &self.commit_points,
            TrendMetric::SecurityIssues => &self.security_issue_points,
        }
    }

    /// Render an ASCII chart for the specified metric.
    ///
    /// # Arguments
    ///
    /// * `metric` - The metric to chart
    /// * `width` - Chart width in characters
    /// * `height` - Chart height in lines
    ///
    /// # Example
    ///
    /// ```
    /// use ralph::analytics::{TrendData, TrendMetric, TrendPoint};
    ///
    /// let mut trend_data = TrendData::default();
    /// trend_data.warning_count_points.push(TrendPoint::new(10));
    /// trend_data.warning_count_points.push(TrendPoint::new(5));
    ///
    /// let chart = trend_data.render_ascii_chart(TrendMetric::Warnings, 40, 10);
    /// assert!(chart.contains("Warnings"));
    /// ```
    #[must_use]
    pub fn render_ascii_chart(&self, metric: TrendMetric, width: usize, height: usize) -> String {
        let points = self.points_for_metric(metric);

        if points.is_empty() {
            return format!("{}\n\nNo data available.", metric.label());
        }

        // Reverse to get chronological order (oldest to newest for chart)
        let mut chronological: Vec<_> = points.iter().collect();
        chronological.reverse();

        let values: Vec<i64> = chronological.iter().map(|p| p.value).collect();

        let min_val = *values.iter().min().unwrap_or(&0);
        let max_val = *values.iter().max().unwrap_or(&0);
        let range = if max_val == min_val {
            1
        } else {
            max_val - min_val
        };

        let mut chart = String::new();

        // Title
        chart.push_str(&format!("{}\n", metric.label()));
        chart.push_str(&"─".repeat(width));
        chart.push('\n');

        // Y-axis labels and chart area
        let effective_height = height.saturating_sub(3); // Reserve space for title, axis, legend
        let effective_width = width.saturating_sub(8); // Reserve space for Y-axis labels

        for row in 0..effective_height {
            let y_value = max_val - (row as i64 * range / effective_height.max(1) as i64);
            chart.push_str(&format!("{:>6} │", y_value));

            for col in 0..effective_width {
                let data_idx = col * values.len() / effective_width.max(1);
                if data_idx < values.len() {
                    let val = values[data_idx];
                    let val_height =
                        ((val - min_val) * effective_height as i64 / range.max(1)) as usize;
                    let row_from_bottom = effective_height.saturating_sub(1) - row;

                    if val_height >= row_from_bottom {
                        chart.push('█');
                    } else {
                        chart.push(' ');
                    }
                } else {
                    chart.push(' ');
                }
            }
            chart.push('\n');
        }

        // X-axis
        chart.push_str(&format!("{:>6} └", ""));
        chart.push_str(&"─".repeat(effective_width));
        chart.push('\n');

        // Legend
        chart.push_str(&format!(
            "        {} points | Range: {} - {}\n",
            values.len(),
            min_val,
            max_val
        ));

        chart
    }
}

/// Calculate the overall trend direction from deltas.
#[must_use]
pub fn calculate_trend_direction(
    clippy_delta: i32,
    test_failures_delta: i32,
    security_delta: i32,
) -> TrendDirection {
    // Weight security most heavily, then test failures, then clippy
    let weighted_score = security_delta * 3 + test_failures_delta * 2 + clippy_delta;

    if weighted_score < -1 {
        TrendDirection::Improving
    } else if weighted_score > 1 {
        TrendDirection::Degrading
    } else {
        TrendDirection::Stable
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ========================================================================
    // QualityMetricsSnapshot Tests
    // ========================================================================

    #[test]
    fn test_quality_metrics_snapshot_new() {
        let snapshot = QualityMetricsSnapshot::new("test-session", 5);

        assert_eq!(snapshot.session_id, "test-session");
        assert_eq!(snapshot.iteration, 5);
        assert_eq!(snapshot.clippy_warnings, 0);
        assert_eq!(snapshot.test_total, 0);
        assert_eq!(snapshot.test_passed, 0);
        assert_eq!(snapshot.test_failed, 0);
        assert_eq!(snapshot.security_issues, 0);
        assert_eq!(snapshot.allow_annotations, 0);
        assert!(snapshot.task_name.is_none());
    }

    #[test]
    fn test_quality_metrics_snapshot_builder() {
        let snapshot = QualityMetricsSnapshot::new("session-1", 3)
            .with_clippy_warnings(2)
            .with_test_counts(100, 98, 2)
            .with_security_issues(1)
            .with_allow_annotations(0)
            .with_task_name("Fix bug");

        assert_eq!(snapshot.clippy_warnings, 2);
        assert_eq!(snapshot.test_total, 100);
        assert_eq!(snapshot.test_passed, 98);
        assert_eq!(snapshot.test_failed, 2);
        assert_eq!(snapshot.security_issues, 1);
        assert_eq!(snapshot.allow_annotations, 0);
        assert_eq!(snapshot.task_name, Some("Fix bug".to_string()));
    }

    #[test]
    fn test_quality_metrics_snapshot_all_gates_passing_true() {
        let snapshot = QualityMetricsSnapshot::new("session", 1)
            .with_clippy_warnings(0)
            .with_test_counts(50, 50, 0)
            .with_security_issues(0)
            .with_allow_annotations(0);

        assert!(snapshot.all_gates_passing());
    }

    #[test]
    fn test_quality_metrics_snapshot_all_gates_passing_false() {
        // Test each failure condition
        let snapshot1 = QualityMetricsSnapshot::new("session", 1).with_clippy_warnings(1);
        assert!(!snapshot1.all_gates_passing());

        let snapshot2 = QualityMetricsSnapshot::new("session", 1).with_test_counts(10, 9, 1);
        assert!(!snapshot2.all_gates_passing());

        let snapshot3 = QualityMetricsSnapshot::new("session", 1).with_security_issues(1);
        assert!(!snapshot3.all_gates_passing());

        let snapshot4 = QualityMetricsSnapshot::new("session", 1).with_allow_annotations(1);
        assert!(!snapshot4.all_gates_passing());
    }

    #[test]
    fn test_quality_metrics_snapshot_test_pass_rate() {
        let snapshot = QualityMetricsSnapshot::new("session", 1).with_test_counts(100, 95, 5);

        assert_eq!(snapshot.test_pass_rate(), Some(0.95));
    }

    #[test]
    fn test_quality_metrics_snapshot_test_pass_rate_no_tests() {
        let snapshot = QualityMetricsSnapshot::new("session", 1);
        assert!(snapshot.test_pass_rate().is_none());
    }

    #[test]
    fn test_quality_metrics_snapshot_serialization() {
        let snapshot = QualityMetricsSnapshot::new("session-1", 5)
            .with_clippy_warnings(3)
            .with_test_counts(100, 97, 3)
            .with_security_issues(1)
            .with_task_name("Implement feature");

        let json = serde_json::to_string(&snapshot).unwrap();
        let restored: QualityMetricsSnapshot = serde_json::from_str(&json).unwrap();

        assert_eq!(restored.session_id, "session-1");
        assert_eq!(restored.iteration, 5);
        assert_eq!(restored.clippy_warnings, 3);
        assert_eq!(restored.test_total, 100);
        assert_eq!(restored.test_passed, 97);
        assert_eq!(restored.test_failed, 3);
        assert_eq!(restored.security_issues, 1);
        assert_eq!(restored.task_name, Some("Implement feature".to_string()));
    }

    // ========================================================================
    // TrendDirection and QualityTrend Tests
    // ========================================================================

    #[test]
    fn test_quality_trend_default() {
        let trend = QualityTrend::default();

        assert_eq!(trend.overall, TrendDirection::Stable);
        assert_eq!(trend.snapshot_count, 0);
        assert_eq!(trend.avg_clippy_warnings, 0.0);
        assert!(trend.avg_test_pass_rate.is_none());
        assert_eq!(trend.avg_security_issues, 0.0);
        assert_eq!(trend.clippy_delta, 0);
        assert_eq!(trend.test_failures_delta, 0);
        assert_eq!(trend.security_delta, 0);
    }

    #[test]
    fn test_trend_direction_calculation() {
        // Improving: negative weighted score
        assert_eq!(
            calculate_trend_direction(-3, -2, -1),
            TrendDirection::Improving
        );

        // Degrading: positive weighted score
        assert_eq!(
            calculate_trend_direction(3, 2, 1),
            TrendDirection::Degrading
        );

        // Stable: weighted score near zero
        assert_eq!(
            calculate_trend_direction(0, 0, 0),
            TrendDirection::Stable
        );

        // Security issues weighted heavily
        assert_eq!(
            calculate_trend_direction(0, 0, 2),
            TrendDirection::Degrading
        );
    }

    // ========================================================================
    // TrendPoint Tests
    // ========================================================================

    #[test]
    fn test_trend_point_structure() {
        let point = TrendPoint::new(5);
        assert_eq!(point.value, 5);
        assert!(point.timestamp <= Utc::now());
    }

    // ========================================================================
    // TrendMetric Tests
    // ========================================================================

    #[test]
    fn test_trend_metric_enum() {
        assert_eq!(TrendMetric::Warnings.label(), "Warnings");
        assert_eq!(TrendMetric::TestCount.label(), "Test Count");
        assert_eq!(TrendMetric::Commits.label(), "Commits");
        assert_eq!(TrendMetric::TestPassRate.label(), "Test Pass Rate (%)");
        assert_eq!(TrendMetric::SecurityIssues.label(), "Security Issues");
    }

    // ========================================================================
    // TrendData Tests
    // ========================================================================

    #[test]
    fn test_trend_data_empty_when_no_data() {
        let trend_data = TrendData::default();

        assert!(trend_data.warning_count_points.is_empty());
        assert!(trend_data.test_count_points.is_empty());
        assert!(trend_data.commit_points.is_empty());
    }

    #[test]
    fn test_ascii_chart_handles_empty_data() {
        let trend_data = TrendData::default();
        let chart = trend_data.render_ascii_chart(TrendMetric::Warnings, 40, 10);

        assert!(chart.contains("No data"));
    }

    #[test]
    fn test_ascii_chart_handles_single_point() {
        let mut trend_data = TrendData::default();
        trend_data.warning_count_points.push(TrendPoint::new(5));

        let chart = trend_data.render_ascii_chart(TrendMetric::Warnings, 40, 10);

        // Should still render something useful
        assert!(!chart.is_empty());
    }

    #[test]
    fn test_trend_data_serialization_roundtrip() {
        let mut trend_data = TrendData::default();
        trend_data.warning_count_points.push(TrendPoint::new(10));
        trend_data.warning_count_points.push(TrendPoint::new(5));
        trend_data.test_count_points.push(TrendPoint::new(100));

        let json = trend_data.to_json().unwrap();
        let restored: TrendData = serde_json::from_str(&json).unwrap();

        assert_eq!(restored.warning_count_points.len(), 2);
        assert_eq!(restored.test_count_points.len(), 1);
    }

    #[test]
    fn test_trend_data_points_for_metric() {
        let mut trend_data = TrendData::default();
        trend_data.warning_count_points.push(TrendPoint::new(10));
        trend_data.test_count_points.push(TrendPoint::new(100));
        trend_data.security_issue_points.push(TrendPoint::new(0));

        assert_eq!(trend_data.points_for_metric(TrendMetric::Warnings).len(), 1);
        assert_eq!(trend_data.points_for_metric(TrendMetric::TestCount).len(), 1);
        assert_eq!(trend_data.points_for_metric(TrendMetric::SecurityIssues).len(), 1);
        assert_eq!(trend_data.points_for_metric(TrendMetric::Commits).len(), 0);
    }

    #[test]
    fn test_trend_data_ascii_chart_with_data() {
        let mut trend_data = TrendData::default();
        trend_data.warning_count_points.push(TrendPoint::new(10));
        trend_data.warning_count_points.push(TrendPoint::new(5));
        trend_data.warning_count_points.push(TrendPoint::new(2));

        let chart = trend_data.render_ascii_chart(TrendMetric::Warnings, 40, 10);

        // Chart should contain expected elements
        assert!(chart.contains("Warnings")); // Title
        assert!(chart.lines().count() >= 5); // Has multiple lines
    }
}
