//! Checkpoint difference computation and visualization.
//!
//! This module provides the `CheckpointDiff` type for computing and analyzing
//! differences between checkpoints, enabling quality trend visualization.

use serde::{Deserialize, Serialize};
use std::fmt;

use super::{Checkpoint, CheckpointId};

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
        let files_changed: Vec<String> =
            from_files.union(&to_files).map(|s| (*s).clone()).collect();

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
        lines.push(format!("  Tests total:     {:+}", self.test_total_delta));
        lines.push(format!("  Tests passed:    {:+}", self.test_passed_delta));
        lines.push(format!("  Tests failed:    {:+}", self.test_failed_delta));
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
    use crate::checkpoint::QualityMetrics;

    // ------------------------------------------------------------------------
    // Phase 11.3: Checkpoint Diff Visualization Tests
    // ------------------------------------------------------------------------

    #[test]
    fn test_checkpoint_diff_shows_test_count_changes() {
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
        let from = Checkpoint::new("Before", "abc123", "main", QualityMetrics::new(), 1);
        let to = Checkpoint::new("After", "def456", "main", QualityMetrics::new(), 2);

        let diff = CheckpointDiff::compute(&from, &to);

        // Diff should capture which checkpoints were compared
        assert_eq!(diff.from_id, from.id);
        assert_eq!(diff.to_id, to.id);
    }

    #[test]
    fn test_checkpoint_diff_shows_security_issue_changes() {
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
