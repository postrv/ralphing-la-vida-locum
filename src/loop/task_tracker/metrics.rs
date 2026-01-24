//! Task metrics and statistics types.
//!
//! This module provides types for tracking task performance metrics
//! and aggregate task counts.

use serde::{Deserialize, Serialize};

// ============================================================================
// Task Metrics
// ============================================================================

/// Metrics tracking for a single task.
///
/// Records performance data like iterations spent, commits made,
/// and quality gate failures to detect stagnation.
///
/// # Example
///
/// ```
/// use ralph::r#loop::task_tracker::TaskMetrics;
///
/// let mut metrics = TaskMetrics::new();
/// metrics.record_iteration();
/// metrics.record_progress(2, 50);
/// assert_eq!(metrics.iterations, 1);
/// assert_eq!(metrics.files_modified, 2);
/// ```
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct TaskMetrics {
    /// Number of iterations spent on this task
    pub iterations: u32,
    /// Number of commits made while working on this task
    pub commits: u32,
    /// Number of files modified
    pub files_modified: u32,
    /// Consecutive iterations without progress
    pub no_progress_count: u32,
    /// Number of quality gate check failures
    pub quality_failures: u32,
    /// Total lines changed
    pub lines_changed: u32,
}

impl TaskMetrics {
    /// Create new empty metrics.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Record an iteration.
    pub fn record_iteration(&mut self) {
        self.iterations += 1;
    }

    /// Record a commit.
    pub fn record_commit(&mut self) {
        self.commits += 1;
    }

    /// Record progress (resets no_progress_count).
    pub fn record_progress(&mut self, files: u32, lines: u32) {
        self.files_modified += files;
        self.lines_changed += lines;
        self.no_progress_count = 0;
    }

    /// Record no progress.
    pub fn record_no_progress(&mut self) {
        self.no_progress_count += 1;
    }

    /// Record a quality gate failure.
    pub fn record_quality_failure(&mut self) {
        self.quality_failures += 1;
    }

    /// Reset quality failure count (after passing).
    pub fn reset_quality_failures(&mut self) {
        self.quality_failures = 0;
    }
}

// ============================================================================
// Task Counts
// ============================================================================

/// Aggregate task counts by state.
///
/// Provides a snapshot of how many tasks are in each state,
/// useful for progress tracking and completion detection.
///
/// # Example
///
/// ```
/// use ralph::r#loop::task_tracker::TaskCounts;
///
/// let counts = TaskCounts {
///     not_started: 5,
///     in_progress: 2,
///     blocked: 0,
///     in_review: 1,
///     complete: 10,
/// };
/// assert_eq!(counts.total(), 18);
/// assert!(!counts.all_done());
/// ```
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct TaskCounts {
    /// Tasks not yet started
    pub not_started: u32,
    /// Tasks currently in progress
    pub in_progress: u32,
    /// Tasks that are blocked
    pub blocked: u32,
    /// Tasks under review
    pub in_review: u32,
    /// Completed tasks
    pub complete: u32,
}

impl TaskCounts {
    /// Get total task count.
    #[must_use]
    pub fn total(&self) -> u32 {
        self.not_started + self.in_progress + self.blocked + self.in_review + self.complete
    }

    /// Check if all tasks are done (complete or blocked).
    #[must_use]
    pub fn all_done(&self) -> bool {
        self.not_started == 0 && self.in_progress == 0 && self.in_review == 0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ========================================================================
    // TaskMetrics Tests
    // ========================================================================

    #[test]
    fn test_task_metrics_default() {
        let metrics = TaskMetrics::default();
        assert_eq!(metrics.iterations, 0);
        assert_eq!(metrics.commits, 0);
        assert_eq!(metrics.files_modified, 0);
        assert_eq!(metrics.no_progress_count, 0);
        assert_eq!(metrics.quality_failures, 0);
        assert_eq!(metrics.lines_changed, 0);
    }

    #[test]
    fn test_task_metrics_record_iteration() {
        let mut metrics = TaskMetrics::new();
        metrics.record_iteration();
        metrics.record_iteration();
        assert_eq!(metrics.iterations, 2);
    }

    #[test]
    fn test_task_metrics_record_commit() {
        let mut metrics = TaskMetrics::new();
        metrics.record_commit();
        assert_eq!(metrics.commits, 1);
    }

    #[test]
    fn test_task_metrics_record_progress_resets_no_progress() {
        let mut metrics = TaskMetrics::new();
        metrics.record_no_progress();
        metrics.record_no_progress();
        assert_eq!(metrics.no_progress_count, 2);

        metrics.record_progress(3, 100);
        assert_eq!(metrics.no_progress_count, 0);
        assert_eq!(metrics.files_modified, 3);
        assert_eq!(metrics.lines_changed, 100);
    }

    #[test]
    fn test_task_metrics_quality_failures() {
        let mut metrics = TaskMetrics::new();
        metrics.record_quality_failure();
        metrics.record_quality_failure();
        assert_eq!(metrics.quality_failures, 2);

        metrics.reset_quality_failures();
        assert_eq!(metrics.quality_failures, 0);
    }

    // ========================================================================
    // TaskCounts Tests
    // ========================================================================

    #[test]
    fn test_task_counts_default() {
        let counts = TaskCounts::default();
        assert_eq!(counts.total(), 0);
        assert!(counts.all_done());
    }

    #[test]
    fn test_task_counts_serialize() {
        let counts = TaskCounts {
            not_started: 1,
            in_progress: 2,
            blocked: 3,
            in_review: 4,
            complete: 5,
        };
        let json = serde_json::to_string(&counts).unwrap();
        assert!(json.contains("\"not_started\":1"));
        assert!(json.contains("\"complete\":5"));
    }

    #[test]
    fn test_task_counts_total() {
        let counts = TaskCounts {
            not_started: 1,
            in_progress: 2,
            blocked: 3,
            in_review: 4,
            complete: 5,
        };
        assert_eq!(counts.total(), 15);
    }

    #[test]
    fn test_task_counts_all_done_true() {
        let counts = TaskCounts {
            not_started: 0,
            in_progress: 0,
            blocked: 5,
            in_review: 0,
            complete: 10,
        };
        assert!(counts.all_done());
    }

    #[test]
    fn test_task_counts_all_done_false_in_progress() {
        let counts = TaskCounts {
            not_started: 0,
            in_progress: 1,
            blocked: 0,
            in_review: 0,
            complete: 5,
        };
        assert!(!counts.all_done());
    }

    #[test]
    fn test_task_counts_all_done_false_not_started() {
        let counts = TaskCounts {
            not_started: 1,
            in_progress: 0,
            blocked: 0,
            in_review: 0,
            complete: 5,
        };
        assert!(!counts.all_done());
    }
}
