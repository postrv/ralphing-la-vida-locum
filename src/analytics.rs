//! Analytics and logging for the automation suite.
//!
//! This module handles session tracking, event logging, and
//! performance analytics in JSONL format.

use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use colored::Colorize;
use serde::{Deserialize, Serialize};
use std::fs::{self, File, OpenOptions};
use std::io::{BufRead, BufReader, Write};
use std::path::PathBuf;

/// An analytics event
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnalyticsEvent {
    pub session: String,
    pub event: String,
    #[serde(rename = "ts")]
    pub timestamp: DateTime<Utc>,
    #[serde(flatten)]
    pub data: serde_json::Value,
}

/// A session summary
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionSummary {
    pub session_id: String,
    pub started_at: Option<DateTime<Utc>>,
    pub ended_at: Option<DateTime<Utc>>,
    pub mode: Option<String>,
    pub iterations: usize,
    pub stagnations: usize,
    pub errors: usize,
    pub docs_drift_events: usize,
    pub duration_minutes: Option<i64>,
}

/// Analytics manager
#[derive(Debug)]
pub struct Analytics {
    project_dir: PathBuf,
}

impl Analytics {
    /// Create a new analytics manager
    pub fn new(project_dir: PathBuf) -> Self {
        Self { project_dir }
    }

    /// Get the analytics file path
    fn analytics_file(&self) -> PathBuf {
        self.project_dir.join(".ralph/analytics.jsonl")
    }

    /// Ensure analytics directory exists
    fn ensure_dir(&self) -> Result<()> {
        let dir = self.project_dir.join(".ralph");
        if !dir.exists() {
            fs::create_dir_all(&dir)?;
        }
        Ok(())
    }

    /// Log an event
    pub fn log_event(&self, session: &str, event: &str, data: serde_json::Value) -> Result<()> {
        self.ensure_dir()?;

        let analytics_event = AnalyticsEvent {
            session: session.to_string(),
            event: event.to_string(),
            timestamp: Utc::now(),
            data,
        };

        let mut file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(self.analytics_file())?;

        let json = serde_json::to_string(&analytics_event)?;
        writeln!(file, "{}", json)?;

        Ok(())
    }

    /// Read all events from the analytics file
    pub fn read_events(&self) -> Result<Vec<AnalyticsEvent>> {
        let file_path = self.analytics_file();

        if !file_path.exists() {
            return Ok(Vec::new());
        }

        let file = File::open(&file_path).context("Failed to open analytics file")?;
        let reader = BufReader::new(file);

        let mut events = Vec::new();
        for line in reader.lines().map_while(Result::ok) {
            if let Ok(event) = serde_json::from_str::<AnalyticsEvent>(&line) {
                events.push(event);
            }
        }

        Ok(events)
    }

    /// Clear all analytics data
    pub fn clear(&self) -> Result<()> {
        let file_path = self.analytics_file();
        if file_path.exists() {
            fs::remove_file(&file_path)?;
        }
        Ok(())
    }

    /// Get recent session summaries
    pub fn get_recent_sessions(&self, count: usize) -> Result<Vec<SessionSummary>> {
        let events = self.read_events()?;

        // Group events by session
        let mut sessions: std::collections::HashMap<String, Vec<&AnalyticsEvent>> =
            std::collections::HashMap::new();

        for event in &events {
            sessions
                .entry(event.session.clone())
                .or_default()
                .push(event);
        }

        // Build summaries
        let mut summaries: Vec<SessionSummary> = sessions
            .into_iter()
            .map(|(session_id, events)| self.build_session_summary(&session_id, &events))
            .collect();

        // Sort by start time (most recent first)
        summaries.sort_by(|a, b| b.started_at.cmp(&a.started_at));

        // Take the requested number
        summaries.truncate(count);

        Ok(summaries)
    }

    /// Build a session summary from events
    fn build_session_summary(
        &self,
        session_id: &str,
        events: &[&AnalyticsEvent],
    ) -> SessionSummary {
        let mut summary = SessionSummary {
            session_id: session_id.to_string(),
            started_at: None,
            ended_at: None,
            mode: None,
            iterations: 0,
            stagnations: 0,
            errors: 0,
            docs_drift_events: 0,
            duration_minutes: None,
        };

        for event in events {
            match event.event.as_str() {
                "session_start" => {
                    summary.started_at = Some(event.timestamp);
                    if let Some(mode) = event.data.get("mode").and_then(|v| v.as_str()) {
                        summary.mode = Some(mode.to_string());
                    }
                }
                "session_end" => {
                    summary.ended_at = Some(event.timestamp);
                }
                "iteration" => {
                    summary.iterations += 1;
                    if let Some(stagnation) = event.data.get("stagnation").and_then(|v| v.as_u64())
                    {
                        if stagnation > 0 {
                            summary.stagnations += 1;
                        }
                    }
                }
                "iteration_error" => {
                    summary.errors += 1;
                }
                "docs_drift_detected" => {
                    summary.docs_drift_events += 1;
                }
                _ => {}
            }
        }

        // Calculate duration
        if let (Some(start), Some(end)) = (summary.started_at, summary.ended_at) {
            summary.duration_minutes = Some((end - start).num_minutes());
        }

        summary
    }

    /// Print a summary of sessions
    pub fn print_summary(&self, sessions: &[SessionSummary], detailed: bool) {
        if sessions.is_empty() {
            println!("No sessions found.");
            return;
        }

        println!("\n{} Recent Sessions", "Analytics:".cyan().bold());
        println!("{}", "─".repeat(60));

        for (i, session) in sessions.iter().enumerate() {
            let started = session
                .started_at
                .map(|t| t.format("%Y-%m-%d %H:%M").to_string())
                .unwrap_or_else(|| "Unknown".to_string());

            let duration = session
                .duration_minutes
                .map(|d| format!("{}m", d))
                .unwrap_or_else(|| "Running".to_string());

            let mode = session.mode.as_deref().unwrap_or("unknown");

            println!(
                "\n{} Session {} ({})",
                format!("[{}]", i + 1).bright_blue(),
                &session.session_id[..8.min(session.session_id.len())],
                started
            );
            println!("   Mode: {} | Duration: {}", mode, duration);
            println!(
                "   Iterations: {} | Stagnations: {} | Errors: {}",
                session.iterations, session.stagnations, session.errors
            );

            if session.docs_drift_events > 0 {
                println!(
                    "   {} Docs drift events: {}",
                    "Warning:".yellow(),
                    session.docs_drift_events
                );
            }

            if detailed {
                // Could add more detailed event breakdown here
            }
        }

        println!("\n{}", "─".repeat(60));
    }

    /// Get aggregate statistics
    pub fn get_aggregate_stats(&self) -> Result<AggregateStats> {
        let events = self.read_events()?;

        let mut stats = AggregateStats::default();

        let mut sessions = std::collections::HashSet::new();

        for event in &events {
            sessions.insert(event.session.clone());

            match event.event.as_str() {
                "iteration" => stats.total_iterations += 1,
                "iteration_error" => stats.total_errors += 1,
                "docs_drift_detected" => stats.total_drift_events += 1,
                "stagnation" => stats.total_stagnations += 1,
                _ => {}
            }
        }

        stats.total_sessions = sessions.len();

        Ok(stats)
    }

    // ========================================================================
    // Quality Metrics Collection
    // ========================================================================

    /// Log a quality metrics snapshot.
    ///
    /// Records the quality metrics at a point in time for trend analysis.
    ///
    /// # Arguments
    ///
    /// * `snapshot` - The quality metrics snapshot to log
    ///
    /// # Errors
    ///
    /// Returns an error if writing to the analytics file fails.
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// use ralph::analytics::{Analytics, QualityMetricsSnapshot};
    ///
    /// let analytics = Analytics::new(project_dir);
    /// let snapshot = QualityMetricsSnapshot::new("session-1", 5)
    ///     .with_clippy_warnings(0)
    ///     .with_test_counts(42, 42, 0);
    ///
    /// analytics.log_quality_metrics(&snapshot)?;
    /// ```
    pub fn log_quality_metrics(&self, snapshot: &QualityMetricsSnapshot) -> Result<()> {
        let data = serde_json::to_value(snapshot)?;
        self.log_event(&snapshot.session_id, "quality_metrics", data)
    }

    /// Get quality metrics history for a session or all sessions.
    ///
    /// # Arguments
    ///
    /// * `session_id` - Optional session ID to filter by. If `None`, returns all metrics.
    /// * `limit` - Maximum number of snapshots to return (most recent first).
    ///
    /// # Errors
    ///
    /// Returns an error if reading the analytics file fails.
    pub fn get_quality_metrics_history(
        &self,
        session_id: Option<&str>,
        limit: usize,
    ) -> Result<Vec<QualityMetricsSnapshot>> {
        let events = self.read_events()?;

        let mut snapshots: Vec<QualityMetricsSnapshot> = events
            .into_iter()
            .filter(|e| e.event == "quality_metrics")
            .filter(|e| session_id.is_none_or(|sid| e.session == sid))
            .filter_map(|e| serde_json::from_value(e.data).ok())
            .collect();

        // Sort by timestamp descending (most recent first)
        snapshots.sort_by(|a, b| b.timestamp.cmp(&a.timestamp));
        snapshots.truncate(limit);

        Ok(snapshots)
    }

    /// Calculate quality trend from historical metrics.
    ///
    /// Analyzes the quality metrics over time to determine if quality
    /// is improving, stable, or degrading.
    ///
    /// # Arguments
    ///
    /// * `session_id` - Optional session ID to filter by.
    /// * `limit` - Number of recent snapshots to analyze.
    ///
    /// # Errors
    ///
    /// Returns an error if reading the analytics file fails.
    pub fn get_quality_trend(
        &self,
        session_id: Option<&str>,
        limit: usize,
    ) -> Result<QualityTrend> {
        let snapshots = self.get_quality_metrics_history(session_id, limit)?;

        if snapshots.is_empty() {
            return Ok(QualityTrend::default());
        }

        let count = snapshots.len();

        // Calculate averages
        let total_clippy: u32 = snapshots.iter().map(|s| s.clippy_warnings).sum();
        let total_security: u32 = snapshots.iter().map(|s| s.security_issues).sum();

        let avg_clippy = total_clippy as f32 / count as f32;
        let avg_security = total_security as f32 / count as f32;

        // Calculate average test pass rate (only for snapshots with tests)
        let test_rates: Vec<f32> = snapshots
            .iter()
            .filter_map(|s| s.test_pass_rate())
            .collect();
        let avg_test_pass_rate = if test_rates.is_empty() {
            None
        } else {
            Some(test_rates.iter().sum::<f32>() / test_rates.len() as f32)
        };

        // Calculate deltas (comparing oldest to newest)
        // snapshots are sorted newest first, so first = newest, last = oldest
        let (clippy_delta, test_failures_delta, security_delta) = if count >= 2 {
            let newest = &snapshots[0];
            let oldest = &snapshots[count - 1];

            (
                newest.clippy_warnings as i32 - oldest.clippy_warnings as i32,
                newest.test_failed as i32 - oldest.test_failed as i32,
                newest.security_issues as i32 - oldest.security_issues as i32,
            )
        } else {
            (0, 0, 0)
        };

        // Determine overall trend
        let overall =
            Self::calculate_trend_direction(clippy_delta, test_failures_delta, security_delta);

        Ok(QualityTrend {
            overall,
            snapshot_count: count,
            avg_clippy_warnings: avg_clippy,
            avg_test_pass_rate,
            avg_security_issues: avg_security,
            clippy_delta,
            test_failures_delta,
            security_delta,
        })
    }

    /// Calculate the overall trend direction from deltas.
    fn calculate_trend_direction(
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
}

/// Aggregate statistics across all sessions
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct AggregateStats {
    pub total_sessions: usize,
    pub total_iterations: usize,
    pub total_errors: usize,
    pub total_stagnations: usize,
    pub total_drift_events: usize,
}

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

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_log_and_read_event() {
        let temp = TempDir::new().unwrap();
        let analytics = Analytics::new(temp.path().to_path_buf());

        analytics
            .log_event(
                "test-session",
                "test_event",
                serde_json::json!({"foo": "bar"}),
            )
            .unwrap();

        let events = analytics.read_events().unwrap();
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].session, "test-session");
        assert_eq!(events[0].event, "test_event");
    }

    #[test]
    fn test_session_summary() {
        let temp = TempDir::new().unwrap();
        let analytics = Analytics::new(temp.path().to_path_buf());

        analytics
            .log_event(
                "session1",
                "session_start",
                serde_json::json!({"mode": "build"}),
            )
            .unwrap();

        analytics
            .log_event(
                "session1",
                "iteration",
                serde_json::json!({"stagnation": 0}),
            )
            .unwrap();

        analytics
            .log_event("session1", "session_end", serde_json::json!({}))
            .unwrap();

        let sessions = analytics.get_recent_sessions(5).unwrap();
        assert_eq!(sessions.len(), 1);
        assert_eq!(sessions[0].iterations, 1);
        assert_eq!(sessions[0].mode, Some("build".to_string()));
    }

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

    // ========================================================================
    // Quality Metrics Logging Tests
    // ========================================================================

    #[test]
    fn test_log_quality_metrics() {
        let temp = TempDir::new().unwrap();
        let analytics = Analytics::new(temp.path().to_path_buf());

        let snapshot = QualityMetricsSnapshot::new("test-session", 1)
            .with_clippy_warnings(0)
            .with_test_counts(42, 42, 0);

        analytics.log_quality_metrics(&snapshot).unwrap();

        let events = analytics.read_events().unwrap();
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].event, "quality_metrics");
        assert_eq!(events[0].session, "test-session");
    }

    #[test]
    fn test_log_multiple_quality_metrics() {
        let temp = TempDir::new().unwrap();
        let analytics = Analytics::new(temp.path().to_path_buf());

        // Log metrics for multiple iterations
        for i in 1..=5 {
            let snapshot = QualityMetricsSnapshot::new("session-1", i)
                .with_clippy_warnings(5 - i) // Improving trend
                .with_test_counts(50, 45 + i, 5 - i);
            analytics.log_quality_metrics(&snapshot).unwrap();
        }

        let events = analytics.read_events().unwrap();
        assert_eq!(events.len(), 5);
    }

    // ========================================================================
    // Quality Metrics History Tests
    // ========================================================================

    #[test]
    fn test_get_quality_metrics_history_empty() {
        let temp = TempDir::new().unwrap();
        let analytics = Analytics::new(temp.path().to_path_buf());

        let history = analytics.get_quality_metrics_history(None, 10).unwrap();
        assert!(history.is_empty());
    }

    #[test]
    fn test_get_quality_metrics_history_all_sessions() {
        let temp = TempDir::new().unwrap();
        let analytics = Analytics::new(temp.path().to_path_buf());

        // Log metrics for two sessions
        analytics
            .log_quality_metrics(
                &QualityMetricsSnapshot::new("session-1", 1).with_clippy_warnings(2),
            )
            .unwrap();
        analytics
            .log_quality_metrics(
                &QualityMetricsSnapshot::new("session-2", 1).with_clippy_warnings(3),
            )
            .unwrap();

        let history = analytics.get_quality_metrics_history(None, 10).unwrap();
        assert_eq!(history.len(), 2);
    }

    #[test]
    fn test_get_quality_metrics_history_filtered_by_session() {
        let temp = TempDir::new().unwrap();
        let analytics = Analytics::new(temp.path().to_path_buf());

        analytics
            .log_quality_metrics(
                &QualityMetricsSnapshot::new("session-1", 1).with_clippy_warnings(2),
            )
            .unwrap();
        analytics
            .log_quality_metrics(
                &QualityMetricsSnapshot::new("session-1", 2).with_clippy_warnings(1),
            )
            .unwrap();
        analytics
            .log_quality_metrics(
                &QualityMetricsSnapshot::new("session-2", 1).with_clippy_warnings(5),
            )
            .unwrap();

        let history = analytics
            .get_quality_metrics_history(Some("session-1"), 10)
            .unwrap();
        assert_eq!(history.len(), 2);
        assert!(history.iter().all(|s| s.session_id == "session-1"));
    }

    #[test]
    fn test_get_quality_metrics_history_respects_limit() {
        let temp = TempDir::new().unwrap();
        let analytics = Analytics::new(temp.path().to_path_buf());

        for i in 1..=10 {
            analytics
                .log_quality_metrics(&QualityMetricsSnapshot::new("session", i))
                .unwrap();
        }

        let history = analytics.get_quality_metrics_history(None, 3).unwrap();
        assert_eq!(history.len(), 3);
    }

    // ========================================================================
    // Quality Trend Tests
    // ========================================================================

    #[test]
    fn test_get_quality_trend_empty() {
        let temp = TempDir::new().unwrap();
        let analytics = Analytics::new(temp.path().to_path_buf());

        let trend = analytics.get_quality_trend(None, 10).unwrap();

        assert_eq!(trend.overall, TrendDirection::Stable);
        assert_eq!(trend.snapshot_count, 0);
    }

    #[test]
    fn test_get_quality_trend_improving() {
        let temp = TempDir::new().unwrap();
        let analytics = Analytics::new(temp.path().to_path_buf());

        // Log improving metrics (fewer warnings over time)
        // Note: we need to log them in chronological order, then they'll be sorted newest-first
        analytics
            .log_quality_metrics(
                &QualityMetricsSnapshot::new("session", 1)
                    .with_clippy_warnings(5)
                    .with_test_counts(50, 45, 5),
            )
            .unwrap();

        // Small delay to ensure different timestamps
        std::thread::sleep(std::time::Duration::from_millis(10));

        analytics
            .log_quality_metrics(
                &QualityMetricsSnapshot::new("session", 2)
                    .with_clippy_warnings(2)
                    .with_test_counts(50, 48, 2),
            )
            .unwrap();

        std::thread::sleep(std::time::Duration::from_millis(10));

        analytics
            .log_quality_metrics(
                &QualityMetricsSnapshot::new("session", 3)
                    .with_clippy_warnings(0)
                    .with_test_counts(50, 50, 0),
            )
            .unwrap();

        let trend = analytics.get_quality_trend(None, 10).unwrap();

        assert_eq!(trend.overall, TrendDirection::Improving);
        assert_eq!(trend.snapshot_count, 3);
        assert!(trend.clippy_delta < 0); // Fewer warnings = negative delta
        assert!(trend.test_failures_delta < 0); // Fewer failures = negative delta
    }

    #[test]
    fn test_get_quality_trend_degrading() {
        let temp = TempDir::new().unwrap();
        let analytics = Analytics::new(temp.path().to_path_buf());

        // Log degrading metrics (more issues over time)
        analytics
            .log_quality_metrics(
                &QualityMetricsSnapshot::new("session", 1)
                    .with_clippy_warnings(0)
                    .with_test_counts(50, 50, 0)
                    .with_security_issues(0),
            )
            .unwrap();

        std::thread::sleep(std::time::Duration::from_millis(10));

        analytics
            .log_quality_metrics(
                &QualityMetricsSnapshot::new("session", 2)
                    .with_clippy_warnings(3)
                    .with_test_counts(50, 47, 3)
                    .with_security_issues(1),
            )
            .unwrap();

        let trend = analytics.get_quality_trend(None, 10).unwrap();

        assert_eq!(trend.overall, TrendDirection::Degrading);
        assert!(trend.clippy_delta > 0);
        assert!(trend.test_failures_delta > 0);
        assert!(trend.security_delta > 0);
    }

    #[test]
    fn test_get_quality_trend_stable() {
        let temp = TempDir::new().unwrap();
        let analytics = Analytics::new(temp.path().to_path_buf());

        // Log stable metrics (no change)
        for i in 1..=3 {
            analytics
                .log_quality_metrics(
                    &QualityMetricsSnapshot::new("session", i)
                        .with_clippy_warnings(2)
                        .with_test_counts(50, 48, 2),
                )
                .unwrap();
            std::thread::sleep(std::time::Duration::from_millis(10));
        }

        let trend = analytics.get_quality_trend(None, 10).unwrap();

        assert_eq!(trend.overall, TrendDirection::Stable);
        assert_eq!(trend.clippy_delta, 0);
        assert_eq!(trend.test_failures_delta, 0);
    }

    #[test]
    fn test_get_quality_trend_averages() {
        let temp = TempDir::new().unwrap();
        let analytics = Analytics::new(temp.path().to_path_buf());

        analytics
            .log_quality_metrics(
                &QualityMetricsSnapshot::new("session", 1)
                    .with_clippy_warnings(4)
                    .with_test_counts(100, 90, 10),
            )
            .unwrap();

        std::thread::sleep(std::time::Duration::from_millis(10));

        analytics
            .log_quality_metrics(
                &QualityMetricsSnapshot::new("session", 2)
                    .with_clippy_warnings(2)
                    .with_test_counts(100, 95, 5),
            )
            .unwrap();

        let trend = analytics.get_quality_trend(None, 10).unwrap();

        // Average clippy: (4 + 2) / 2 = 3.0
        assert_eq!(trend.avg_clippy_warnings, 3.0);
        // Average test pass rate: (0.9 + 0.95) / 2 = 0.925
        assert!((trend.avg_test_pass_rate.unwrap() - 0.925).abs() < 0.001);
    }

    #[test]
    fn test_trend_direction_calculation() {
        // Improving: negative weighted score
        assert_eq!(
            Analytics::calculate_trend_direction(-3, -2, -1),
            TrendDirection::Improving
        );

        // Degrading: positive weighted score
        assert_eq!(
            Analytics::calculate_trend_direction(3, 2, 1),
            TrendDirection::Degrading
        );

        // Stable: weighted score near zero
        assert_eq!(
            Analytics::calculate_trend_direction(0, 0, 0),
            TrendDirection::Stable
        );

        // Security issues weighted heavily
        assert_eq!(
            Analytics::calculate_trend_direction(0, 0, 2),
            TrendDirection::Degrading
        );
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
}
