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
    /// Predictor accuracy for this session (0.0-1.0).
    pub predictor_accuracy: Option<f64>,
    /// Total time spent running quality gates in milliseconds (Phase 15.1).
    pub total_gate_execution_ms: u64,
    /// Number of times quality gates were run.
    pub gate_runs: usize,
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

    /// Log a quality gates execution event (Phase 15.1).
    ///
    /// Records timing information for quality gate execution, enabling
    /// performance tracking and optimization.
    ///
    /// # Arguments
    ///
    /// * `session` - The current session ID
    /// * `duration_ms` - Total time spent running gates in milliseconds
    /// * `gates_count` - Number of gates that were run
    /// * `passed_count` - Number of gates that passed
    /// * `parallel` - Whether gates were run in parallel
    pub fn log_gate_execution(
        &self,
        session: &str,
        duration_ms: u64,
        gates_count: usize,
        passed_count: usize,
        parallel: bool,
    ) -> Result<()> {
        self.log_event(
            session,
            "quality_gates_run",
            serde_json::json!({
                "duration_ms": duration_ms,
                "gates_count": gates_count,
                "passed_count": passed_count,
                "parallel": parallel
            }),
        )
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
            predictor_accuracy: None,
            total_gate_execution_ms: 0,
            gate_runs: 0,
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
                    // Parse predictor accuracy from session_end event
                    if let Some(accuracy) = event
                        .data
                        .get("predictor_accuracy")
                        .and_then(|v| v.as_f64())
                    {
                        summary.predictor_accuracy = Some(accuracy);
                    }
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
                "quality_gates_run" => {
                    // Track gate execution timing (Phase 15.1)
                    summary.gate_runs += 1;
                    if let Some(duration_ms) =
                        event.data.get("duration_ms").and_then(|v| v.as_u64())
                    {
                        summary.total_gate_execution_ms += duration_ms;
                    }
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

            // Display gate execution timing if available (Phase 15.1)
            if session.gate_runs > 0 {
                let avg_gate_time = session.total_gate_execution_ms / session.gate_runs as u64;
                println!(
                    "   Gate runs: {} | Total time: {}ms | Avg: {}ms",
                    session.gate_runs, session.total_gate_execution_ms, avg_gate_time
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
        let mut predictor_accuracies: Vec<f64> = Vec::new();

        for event in &events {
            sessions.insert(event.session.clone());

            match event.event.as_str() {
                "iteration" => stats.total_iterations += 1,
                "iteration_error" => stats.total_errors += 1,
                "docs_drift_detected" => stats.total_drift_events += 1,
                "stagnation" => stats.total_stagnations += 1,
                "session_end" => {
                    // Collect predictor accuracy from session_end events
                    if let Some(accuracy) = event
                        .data
                        .get("predictor_accuracy")
                        .and_then(|v| v.as_f64())
                    {
                        predictor_accuracies.push(accuracy);
                    }
                }
                "quality_gates_run" => {
                    // Collect gate execution timing (Phase 15.1)
                    stats.total_gate_runs += 1;
                    if let Some(duration_ms) =
                        event.data.get("duration_ms").and_then(|v| v.as_u64())
                    {
                        stats.total_gate_execution_ms += duration_ms;
                    }
                }
                _ => {}
            }
        }

        stats.total_sessions = sessions.len();

        // Calculate average predictor accuracy
        if !predictor_accuracies.is_empty() {
            let sum: f64 = predictor_accuracies.iter().sum();
            stats.avg_predictor_accuracy = Some(sum / predictor_accuracies.len() as f64);
        }

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

    // ========================================================================
    // Predictor Accuracy Logging
    // ========================================================================

    /// Log predictor accuracy statistics for a session.
    ///
    /// # Arguments
    ///
    /// * `session_id` - The session identifier
    /// * `stats` - The predictor accuracy statistics
    ///
    /// # Errors
    ///
    /// Returns an error if writing to the analytics file fails.
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// use ralph::analytics::{Analytics, PredictorAccuracyStats};
    ///
    /// let analytics = Analytics::new(project_dir);
    /// let stats = PredictorAccuracyStats {
    ///     total_predictions: 10,
    ///     correct_predictions: 8,
    ///     overall_accuracy: Some(0.8),
    ///     ..Default::default()
    /// };
    /// analytics.log_predictor_stats("session-1", &stats)?;
    /// ```
    pub fn log_predictor_stats(
        &self,
        session_id: &str,
        stats: &PredictorAccuracyStats,
    ) -> Result<()> {
        let data = serde_json::to_value(stats)?;
        self.log_event(session_id, "predictor_stats", data)
    }

    /// Get predictor statistics history across sessions.
    ///
    /// # Arguments
    ///
    /// * `limit` - Maximum number of records to return (most recent first).
    ///
    /// # Errors
    ///
    /// Returns an error if reading the analytics file fails.
    pub fn get_predictor_stats_history(&self, limit: usize) -> Result<Vec<PredictorAccuracyStats>> {
        let events = self.read_events()?;

        let mut stats: Vec<PredictorAccuracyStats> = events
            .into_iter()
            .filter(|e| e.event == "predictor_stats")
            .filter_map(|e| serde_json::from_value(e.data).ok())
            .collect();

        // Sort by total predictions descending (approximation of recency)
        // The events are already in file order, so we just need to reverse for newest first
        stats.reverse();
        stats.truncate(limit);

        Ok(stats)
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
    /// Average predictor accuracy across sessions that have accuracy data.
    pub avg_predictor_accuracy: Option<f64>,
    /// Total number of quality gate runs across all sessions (Phase 15.1).
    pub total_gate_runs: usize,
    /// Total time spent running quality gates in milliseconds (Phase 15.1).
    pub total_gate_execution_ms: u64,
}

// ============================================================================
// Predictor Accuracy Statistics
// ============================================================================

/// Statistics about predictor accuracy for a session.
///
/// This struct captures predictor performance metrics for analytics and
/// historical tracking.
///
/// # Example
///
/// ```
/// use ralph::analytics::PredictorAccuracyStats;
///
/// let stats = PredictorAccuracyStats {
///     total_predictions: 10,
///     correct_predictions: 8,
///     overall_accuracy: Some(0.8),
///     accuracy_low: Some(0.9),
///     accuracy_medium: Some(0.75),
///     accuracy_high: Some(0.7),
///     accuracy_critical: None,
/// };
/// assert_eq!(stats.total_predictions, 10);
/// ```
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct PredictorAccuracyStats {
    /// Total number of predictions made.
    pub total_predictions: usize,
    /// Number of correct predictions.
    pub correct_predictions: usize,
    /// Overall prediction accuracy (0.0-1.0).
    pub overall_accuracy: Option<f64>,
    /// Accuracy for low risk predictions.
    pub accuracy_low: Option<f64>,
    /// Accuracy for medium risk predictions.
    pub accuracy_medium: Option<f64>,
    /// Accuracy for high risk predictions.
    pub accuracy_high: Option<f64>,
    /// Accuracy for critical risk predictions.
    pub accuracy_critical: Option<f64>,
}

impl PredictorAccuracyStats {
    /// Returns true if any predictions were made.
    #[must_use]
    pub fn has_predictions(&self) -> bool {
        self.total_predictions > 0
    }
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

// ============================================================================
// Phase 16.1: Structured Event Logging
// ============================================================================

/// Current schema version for structured events.
///
/// Increment this when making breaking changes to the event schema.
pub const SCHEMA_VERSION: u32 = 1;

/// Type-safe event types for structured logging.
///
/// Each variant represents a specific event that can occur during
/// a Ralph session. This enables type-safe event filtering and
/// processing.
///
/// # Example
///
/// ```
/// use ralph::analytics::EventType;
///
/// let event_type = EventType::SessionStart;
/// assert_eq!(event_type.as_str(), "session_start");
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EventType {
    /// Session started.
    SessionStart,
    /// Session ended.
    SessionEnd,
    /// Iteration completed.
    Iteration,
    /// Iteration encountered an error.
    IterationError,
    /// Stagnation detected.
    Stagnation,
    /// Quality gate result.
    GateResult,
    /// Predictor made a decision.
    PredictorDecision,
    /// Documentation drift detected.
    DocsDrift,
    /// Quality metrics snapshot.
    QualityMetrics,
    /// Supervisor paused execution.
    SupervisorPause,
    /// Supervisor aborted execution.
    SupervisorAbort,
    /// Handler paused execution.
    HandlerPause,
    /// Prediction made (for accuracy tracking).
    Prediction,
    /// Predictor statistics recorded.
    PredictorStats,
    /// Quality gates run timing.
    QualityGatesRun,
}

impl EventType {
    /// Returns the string representation of the event type.
    ///
    /// # Example
    ///
    /// ```
    /// use ralph::analytics::EventType;
    ///
    /// assert_eq!(EventType::SessionStart.as_str(), "session_start");
    /// assert_eq!(EventType::GateResult.as_str(), "gate_result");
    /// ```
    #[must_use]
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::SessionStart => "session_start",
            Self::SessionEnd => "session_end",
            Self::Iteration => "iteration",
            Self::IterationError => "iteration_error",
            Self::Stagnation => "stagnation",
            Self::GateResult => "gate_result",
            Self::PredictorDecision => "predictor_decision",
            Self::DocsDrift => "docs_drift",
            Self::QualityMetrics => "quality_metrics",
            Self::SupervisorPause => "supervisor_pause",
            Self::SupervisorAbort => "supervisor_abort",
            Self::HandlerPause => "handler_pause",
            Self::Prediction => "prediction",
            Self::PredictorStats => "predictor_stats",
            Self::QualityGatesRun => "quality_gates_run",
        }
    }

    /// Returns all variants of the event type.
    ///
    /// Useful for iteration and filtering operations.
    ///
    /// # Example
    ///
    /// ```
    /// use ralph::analytics::EventType;
    ///
    /// let all = EventType::all_variants();
    /// assert!(all.contains(&EventType::SessionStart));
    /// ```
    #[must_use]
    pub fn all_variants() -> Vec<Self> {
        vec![
            Self::SessionStart,
            Self::SessionEnd,
            Self::Iteration,
            Self::IterationError,
            Self::Stagnation,
            Self::GateResult,
            Self::PredictorDecision,
            Self::DocsDrift,
            Self::QualityMetrics,
            Self::SupervisorPause,
            Self::SupervisorAbort,
            Self::HandlerPause,
            Self::Prediction,
            Self::PredictorStats,
            Self::QualityGatesRun,
        ]
    }
}

impl std::fmt::Display for EventType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

/// A structured event with consistent schema.
///
/// All events include:
/// - Schema version for forward compatibility
/// - Session ID for grouping
/// - Event type for filtering
/// - Timestamp for ordering
/// - Type-specific data payload
///
/// # Example
///
/// ```
/// use ralph::analytics::{StructuredEvent, EventType, SCHEMA_VERSION};
///
/// let event = StructuredEvent::new(
///     "session-123",
///     EventType::SessionStart,
///     serde_json::json!({"mode": "build"}),
/// );
///
/// assert_eq!(event.schema_version, SCHEMA_VERSION);
/// assert_eq!(event.event_type, EventType::SessionStart);
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StructuredEvent {
    /// Schema version for forward compatibility.
    pub schema_version: u32,
    /// Session identifier.
    pub session_id: String,
    /// Type of event.
    pub event_type: EventType,
    /// When the event occurred.
    pub timestamp: DateTime<Utc>,
    /// Event-specific data.
    pub data: serde_json::Value,
}

impl StructuredEvent {
    /// Create a new structured event.
    ///
    /// # Arguments
    ///
    /// * `session_id` - The session identifier
    /// * `event_type` - The type of event
    /// * `data` - Event-specific data
    ///
    /// # Example
    ///
    /// ```
    /// use ralph::analytics::{StructuredEvent, EventType};
    ///
    /// let event = StructuredEvent::new(
    ///     "session-1",
    ///     EventType::Iteration,
    ///     serde_json::json!({"iteration": 5}),
    /// );
    /// ```
    #[must_use]
    pub fn new(session_id: impl Into<String>, event_type: EventType, data: serde_json::Value) -> Self {
        Self {
            schema_version: SCHEMA_VERSION,
            session_id: session_id.into(),
            event_type,
            timestamp: Utc::now(),
            data,
        }
    }
}

/// Structured data for gate result events.
///
/// # Example
///
/// ```
/// use ralph::analytics::GateResultEventData;
///
/// let data = GateResultEventData {
///     gate_name: "clippy".to_string(),
///     passed: true,
///     issue_count: 0,
///     duration_ms: 1500,
///     issues: vec![],
/// };
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GateResultEventData {
    /// Name of the gate that was run.
    pub gate_name: String,
    /// Whether the gate passed.
    pub passed: bool,
    /// Number of issues found.
    pub issue_count: usize,
    /// Duration of the gate run in milliseconds.
    pub duration_ms: u64,
    /// Individual issues found.
    pub issues: Vec<GateIssueEventData>,
}

/// Structured data for individual gate issues.
///
/// # Example
///
/// ```
/// use ralph::analytics::GateIssueEventData;
///
/// let issue = GateIssueEventData {
///     severity: "error".to_string(),
///     message: "unused variable".to_string(),
///     file: Some("src/main.rs".to_string()),
///     line: Some(42),
///     code: Some("E0001".to_string()),
/// };
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GateIssueEventData {
    /// Severity level (error, warning, info).
    pub severity: String,
    /// Issue message.
    pub message: String,
    /// File where the issue was found.
    pub file: Option<String>,
    /// Line number.
    pub line: Option<u32>,
    /// Error code (e.g., E0308, clippy::unwrap_used).
    pub code: Option<String>,
}

/// Structured data for predictor decision events.
///
/// # Example
///
/// ```
/// use ralph::analytics::PredictorDecisionEventData;
///
/// let decision = PredictorDecisionEventData {
///     risk_score: 65.5,
///     risk_level: "high".to_string(),
///     action_recommended: Some("pause".to_string()),
///     contributing_factors: vec!["commit_gap: 8".to_string()],
/// };
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PredictorDecisionEventData {
    /// Risk score (0-100).
    pub risk_score: f64,
    /// Risk level (low, medium, high, critical).
    pub risk_level: String,
    /// Recommended action, if any.
    pub action_recommended: Option<String>,
    /// Factors contributing to the risk score.
    pub contributing_factors: Vec<String>,
}

/// Filter for querying structured events.
///
/// Supports filtering by event type and session ID.
///
/// # Example
///
/// ```
/// use ralph::analytics::{EventFilter, EventType};
///
/// let filter = EventFilter::new()
///     .with_event_type(EventType::GateResult)
///     .with_session_id("session-123");
/// ```
#[derive(Debug, Clone, Default)]
pub struct EventFilter {
    /// Event types to include (empty = all).
    event_types: Vec<EventType>,
    /// Session ID to filter by (None = all).
    session_id: Option<String>,
}

impl EventFilter {
    /// Create a new empty filter (matches all events).
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Add an event type to filter by.
    ///
    /// Multiple event types can be added; events matching any will be included.
    #[must_use]
    pub fn with_event_type(mut self, event_type: EventType) -> Self {
        self.event_types.push(event_type);
        self
    }

    /// Set the session ID to filter by.
    #[must_use]
    pub fn with_session_id(mut self, session_id: impl Into<String>) -> Self {
        self.session_id = Some(session_id.into());
        self
    }

    /// Check if an event matches this filter.
    #[must_use]
    pub fn matches(&self, event: &StructuredEvent) -> bool {
        // Check event type filter
        let type_matches = self.event_types.is_empty()
            || self.event_types.contains(&event.event_type);

        // Check session ID filter
        let session_matches = self.session_id.is_none()
            || self.session_id.as_ref() == Some(&event.session_id);

        type_matches && session_matches
    }
}

// ============================================================================
// Analytics Structured Event Methods
// ============================================================================

impl Analytics {
    /// Log a structured event.
    ///
    /// # Arguments
    ///
    /// * `session_id` - The session identifier
    /// * `event_type` - The type of event
    /// * `data` - Event-specific data
    ///
    /// # Errors
    ///
    /// Returns an error if writing to the analytics file fails.
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// use ralph::analytics::{Analytics, EventType};
    ///
    /// let analytics = Analytics::new(project_dir);
    /// analytics.log_structured_event(
    ///     "session-1",
    ///     EventType::SessionStart,
    ///     serde_json::json!({"mode": "build"}),
    /// )?;
    /// ```
    pub fn log_structured_event(
        &self,
        session_id: &str,
        event_type: EventType,
        data: serde_json::Value,
    ) -> Result<()> {
        let event = StructuredEvent::new(session_id, event_type, data);
        self.write_structured_event(&event)
    }

    /// Log a structured gate result event.
    ///
    /// # Arguments
    ///
    /// * `session_id` - The session identifier
    /// * `gate_data` - The gate result data
    ///
    /// # Errors
    ///
    /// Returns an error if writing to the analytics file fails.
    pub fn log_structured_gate_result(
        &self,
        session_id: &str,
        gate_data: &GateResultEventData,
    ) -> Result<()> {
        let data = serde_json::to_value(gate_data)?;
        self.log_structured_event(session_id, EventType::GateResult, data)
    }

    /// Log a structured predictor decision event.
    ///
    /// # Arguments
    ///
    /// * `session_id` - The session identifier
    /// * `decision_data` - The predictor decision data
    ///
    /// # Errors
    ///
    /// Returns an error if writing to the analytics file fails.
    pub fn log_structured_predictor_decision(
        &self,
        session_id: &str,
        decision_data: &PredictorDecisionEventData,
    ) -> Result<()> {
        let data = serde_json::to_value(decision_data)?;
        self.log_structured_event(session_id, EventType::PredictorDecision, data)
    }

    /// Write a structured event to the analytics file.
    fn write_structured_event(&self, event: &StructuredEvent) -> Result<()> {
        self.ensure_dir()?;

        let mut file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(self.structured_events_file())?;

        let json = serde_json::to_string(event)?;
        writeln!(file, "{}", json)?;

        Ok(())
    }

    /// Get the path to the structured events file.
    fn structured_events_file(&self) -> PathBuf {
        self.project_dir.join(".ralph/structured_events.jsonl")
    }

    /// Read all structured events from the analytics file.
    ///
    /// # Errors
    ///
    /// Returns an error if reading the analytics file fails.
    pub fn read_structured_events(&self) -> Result<Vec<StructuredEvent>> {
        let file_path = self.structured_events_file();

        if !file_path.exists() {
            return Ok(Vec::new());
        }

        let file = File::open(&file_path).context("Failed to open structured events file")?;
        let reader = BufReader::new(file);

        let mut events = Vec::new();
        for line in reader.lines().map_while(Result::ok) {
            if let Ok(event) = serde_json::from_str::<StructuredEvent>(&line) {
                events.push(event);
            }
        }

        Ok(events)
    }

    /// Read structured events filtered by the given filter.
    ///
    /// # Arguments
    ///
    /// * `filter` - The filter to apply
    ///
    /// # Errors
    ///
    /// Returns an error if reading the analytics file fails.
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// use ralph::analytics::{Analytics, EventFilter, EventType};
    ///
    /// let analytics = Analytics::new(project_dir);
    /// let filter = EventFilter::new()
    ///     .with_event_type(EventType::GateResult)
    ///     .with_session_id("session-1");
    /// let events = analytics.read_structured_events_filtered(&filter)?;
    /// ```
    pub fn read_structured_events_filtered(
        &self,
        filter: &EventFilter,
    ) -> Result<Vec<StructuredEvent>> {
        let events = self.read_structured_events()?;
        Ok(events.into_iter().filter(|e| filter.matches(e)).collect())
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

    // ========================================================================
    // Phase 10.2: Predictor Accuracy in Analytics Tests
    // ========================================================================

    #[test]
    fn test_session_summary_includes_predictor_accuracy() {
        let temp = TempDir::new().unwrap();
        let analytics = Analytics::new(temp.path().to_path_buf());

        // Log session with predictor accuracy
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
                "session_end",
                serde_json::json!({
                    "iterations": 10,
                    "predictor_accuracy": 0.85
                }),
            )
            .unwrap();

        let sessions = analytics.get_recent_sessions(5).unwrap();
        assert_eq!(sessions.len(), 1);
        assert_eq!(sessions[0].predictor_accuracy, Some(0.85));
    }

    #[test]
    fn test_session_summary_no_predictor_accuracy() {
        let temp = TempDir::new().unwrap();
        let analytics = Analytics::new(temp.path().to_path_buf());

        // Log session without predictor accuracy
        analytics
            .log_event(
                "session1",
                "session_start",
                serde_json::json!({"mode": "debug"}),
            )
            .unwrap();

        analytics
            .log_event("session1", "session_end", serde_json::json!({}))
            .unwrap();

        let sessions = analytics.get_recent_sessions(5).unwrap();
        assert_eq!(sessions.len(), 1);
        assert!(sessions[0].predictor_accuracy.is_none());
    }

    #[test]
    fn test_aggregate_stats_includes_avg_predictor_accuracy() {
        let temp = TempDir::new().unwrap();
        let analytics = Analytics::new(temp.path().to_path_buf());

        // Log multiple sessions with predictor accuracy
        for (i, accuracy) in [(1, 0.8), (2, 0.9), (3, 0.7)] {
            let session_id = format!("session{}", i);
            analytics
                .log_event(
                    &session_id,
                    "session_start",
                    serde_json::json!({"mode": "build"}),
                )
                .unwrap();
            analytics
                .log_event(
                    &session_id,
                    "session_end",
                    serde_json::json!({"predictor_accuracy": accuracy}),
                )
                .unwrap();
        }

        let stats = analytics.get_aggregate_stats().unwrap();
        assert_eq!(stats.total_sessions, 3);

        // Average: (0.8 + 0.9 + 0.7) / 3 = 0.8
        let avg = stats.avg_predictor_accuracy.expect("Should have average");
        assert!((avg - 0.8).abs() < 0.001, "Expected ~0.8, got {}", avg);
    }

    #[test]
    fn test_aggregate_stats_no_predictor_data() {
        let temp = TempDir::new().unwrap();
        let analytics = Analytics::new(temp.path().to_path_buf());

        // Log sessions without predictor accuracy
        analytics
            .log_event(
                "session1",
                "session_start",
                serde_json::json!({"mode": "build"}),
            )
            .unwrap();

        let stats = analytics.get_aggregate_stats().unwrap();
        assert!(stats.avg_predictor_accuracy.is_none());
    }

    #[test]
    fn test_log_predictor_stats_event() {
        let temp = TempDir::new().unwrap();
        let analytics = Analytics::new(temp.path().to_path_buf());

        let stats = PredictorAccuracyStats {
            total_predictions: 10,
            correct_predictions: 8,
            overall_accuracy: Some(0.8),
            accuracy_low: Some(0.9),
            accuracy_medium: Some(0.75),
            accuracy_high: Some(0.7),
            accuracy_critical: None,
        };

        analytics.log_predictor_stats("session1", &stats).unwrap();

        let events = analytics.read_events().unwrap();
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].event, "predictor_stats");
        assert_eq!(events[0].session, "session1");
    }

    #[test]
    fn test_get_predictor_stats_history() {
        let temp = TempDir::new().unwrap();
        let analytics = Analytics::new(temp.path().to_path_buf());

        // Log stats for multiple sessions
        for i in 1..=3 {
            let stats = PredictorAccuracyStats {
                total_predictions: i * 5,
                correct_predictions: i * 4,
                overall_accuracy: Some(0.7 + (i as f64 * 0.05)),
                accuracy_low: None,
                accuracy_medium: None,
                accuracy_high: None,
                accuracy_critical: None,
            };
            analytics
                .log_predictor_stats(&format!("session{}", i), &stats)
                .unwrap();
        }

        let history = analytics.get_predictor_stats_history(3).unwrap();
        assert_eq!(history.len(), 3);
        // Should be newest first
        assert!((history[0].overall_accuracy.unwrap() - 0.85).abs() < 0.001);
    }

    // ========================================================================
    // Phase 16.1: Structured Event Logging Tests
    // ========================================================================

    // ========================================================================
    // EventType Enum Tests
    // ========================================================================

    #[test]
    fn test_event_type_session_start_exists() {
        // EventType enum should have a SessionStart variant
        let event_type = EventType::SessionStart;
        assert_eq!(event_type.as_str(), "session_start");
    }

    #[test]
    fn test_event_type_session_end_exists() {
        let event_type = EventType::SessionEnd;
        assert_eq!(event_type.as_str(), "session_end");
    }

    #[test]
    fn test_event_type_iteration_exists() {
        let event_type = EventType::Iteration;
        assert_eq!(event_type.as_str(), "iteration");
    }

    #[test]
    fn test_event_type_stagnation_exists() {
        let event_type = EventType::Stagnation;
        assert_eq!(event_type.as_str(), "stagnation");
    }

    #[test]
    fn test_event_type_gate_result_exists() {
        let event_type = EventType::GateResult;
        assert_eq!(event_type.as_str(), "gate_result");
    }

    #[test]
    fn test_event_type_predictor_decision_exists() {
        let event_type = EventType::PredictorDecision;
        assert_eq!(event_type.as_str(), "predictor_decision");
    }

    #[test]
    fn test_event_type_iteration_error_exists() {
        let event_type = EventType::IterationError;
        assert_eq!(event_type.as_str(), "iteration_error");
    }

    #[test]
    fn test_event_type_docs_drift_exists() {
        let event_type = EventType::DocsDrift;
        assert_eq!(event_type.as_str(), "docs_drift");
    }

    #[test]
    fn test_event_type_quality_metrics_exists() {
        let event_type = EventType::QualityMetrics;
        assert_eq!(event_type.as_str(), "quality_metrics");
    }

    #[test]
    fn test_event_type_serialization() {
        let event_type = EventType::SessionStart;
        let json = serde_json::to_string(&event_type).unwrap();
        assert_eq!(json, "\"session_start\"");

        let restored: EventType = serde_json::from_str(&json).unwrap();
        assert_eq!(restored, EventType::SessionStart);
    }

    #[test]
    fn test_event_type_all_variants() {
        // Ensure all event types can be iterated
        let all_types = EventType::all_variants();
        assert!(all_types.len() >= 9); // At minimum: session_start, session_end, iteration, etc.
        assert!(all_types.contains(&EventType::SessionStart));
        assert!(all_types.contains(&EventType::GateResult));
        assert!(all_types.contains(&EventType::PredictorDecision));
    }

    // ========================================================================
    // StructuredEvent Schema Tests
    // ========================================================================

    #[test]
    fn test_structured_event_has_schema_version() {
        let event = StructuredEvent::new(
            "test-session",
            EventType::SessionStart,
            serde_json::json!({"mode": "build"}),
        );

        assert_eq!(event.schema_version, SCHEMA_VERSION);
    }

    #[test]
    fn test_structured_event_has_timestamp() {
        let before = Utc::now();
        let event = StructuredEvent::new(
            "test-session",
            EventType::SessionStart,
            serde_json::json!({}),
        );
        let after = Utc::now();

        assert!(event.timestamp >= before);
        assert!(event.timestamp <= after);
    }

    #[test]
    fn test_structured_event_has_session_id() {
        let event = StructuredEvent::new(
            "my-session-123",
            EventType::Iteration,
            serde_json::json!({}),
        );

        assert_eq!(event.session_id, "my-session-123");
    }

    #[test]
    fn test_structured_event_has_event_type() {
        let event = StructuredEvent::new(
            "test-session",
            EventType::GateResult,
            serde_json::json!({}),
        );

        assert_eq!(event.event_type, EventType::GateResult);
    }

    #[test]
    fn test_structured_event_serialization() {
        let event = StructuredEvent::new(
            "test-session",
            EventType::SessionStart,
            serde_json::json!({"mode": "build"}),
        );

        let json = serde_json::to_string(&event).unwrap();
        assert!(json.contains("\"schema_version\""));
        assert!(json.contains("\"session_id\""));
        assert!(json.contains("\"event_type\""));
        assert!(json.contains("\"timestamp\""));
        assert!(json.contains("\"data\""));

        let restored: StructuredEvent = serde_json::from_str(&json).unwrap();
        assert_eq!(restored.session_id, "test-session");
        assert_eq!(restored.event_type, EventType::SessionStart);
    }

    // ========================================================================
    // Gate Result Structured Event Tests
    // ========================================================================

    #[test]
    fn test_gate_result_event_data_structure() {
        let gate_data = GateResultEventData {
            gate_name: "clippy".to_string(),
            passed: true,
            issue_count: 0,
            duration_ms: 1500,
            issues: vec![],
        };

        assert_eq!(gate_data.gate_name, "clippy");
        assert!(gate_data.passed);
    }

    #[test]
    fn test_gate_result_event_with_issues() {
        let issue = GateIssueEventData {
            severity: "error".to_string(),
            message: "unused variable".to_string(),
            file: Some("src/main.rs".to_string()),
            line: Some(42),
            code: Some("E0001".to_string()),
        };

        let gate_data = GateResultEventData {
            gate_name: "clippy".to_string(),
            passed: false,
            issue_count: 1,
            duration_ms: 2000,
            issues: vec![issue],
        };

        assert!(!gate_data.passed);
        assert_eq!(gate_data.issues.len(), 1);
        assert_eq!(gate_data.issues[0].message, "unused variable");
    }

    #[test]
    fn test_log_gate_result_structured() {
        let temp = TempDir::new().unwrap();
        let analytics = Analytics::new(temp.path().to_path_buf());

        let gate_data = GateResultEventData {
            gate_name: "test".to_string(),
            passed: true,
            issue_count: 0,
            duration_ms: 500,
            issues: vec![],
        };

        analytics
            .log_structured_gate_result("test-session", &gate_data)
            .unwrap();

        let events = analytics.read_structured_events().unwrap();
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].event_type, EventType::GateResult);
    }

    // ========================================================================
    // Predictor Decision Structured Event Tests
    // ========================================================================

    #[test]
    fn test_predictor_decision_event_data_structure() {
        let decision_data = PredictorDecisionEventData {
            risk_score: 65.5,
            risk_level: "high".to_string(),
            action_recommended: Some("pause".to_string()),
            contributing_factors: vec![
                "commit_gap: 8 iterations".to_string(),
                "error_repeat: 3 occurrences".to_string(),
            ],
        };

        assert_eq!(decision_data.risk_score, 65.5);
        assert_eq!(decision_data.risk_level, "high");
        assert_eq!(decision_data.contributing_factors.len(), 2);
    }

    #[test]
    fn test_log_predictor_decision_structured() {
        let temp = TempDir::new().unwrap();
        let analytics = Analytics::new(temp.path().to_path_buf());

        let decision_data = PredictorDecisionEventData {
            risk_score: 45.0,
            risk_level: "medium".to_string(),
            action_recommended: None,
            contributing_factors: vec![],
        };

        analytics
            .log_structured_predictor_decision("test-session", &decision_data)
            .unwrap();

        let events = analytics.read_structured_events().unwrap();
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].event_type, EventType::PredictorDecision);
    }

    #[test]
    fn test_predictor_decision_serialization() {
        let decision_data = PredictorDecisionEventData {
            risk_score: 75.0,
            risk_level: "high".to_string(),
            action_recommended: Some("checkpoint".to_string()),
            contributing_factors: vec!["file_churn: high".to_string()],
        };

        let json = serde_json::to_string(&decision_data).unwrap();
        let restored: PredictorDecisionEventData = serde_json::from_str(&json).unwrap();

        assert_eq!(restored.risk_score, 75.0);
        assert_eq!(
            restored.action_recommended,
            Some("checkpoint".to_string())
        );
    }

    // ========================================================================
    // Event Filtering Tests
    // ========================================================================

    #[test]
    fn test_filter_events_by_single_type() {
        let temp = TempDir::new().unwrap();
        let analytics = Analytics::new(temp.path().to_path_buf());

        // Log multiple event types
        analytics
            .log_structured_event("session1", EventType::SessionStart, serde_json::json!({}))
            .unwrap();
        analytics
            .log_structured_event("session1", EventType::Iteration, serde_json::json!({}))
            .unwrap();
        analytics
            .log_structured_event("session1", EventType::Iteration, serde_json::json!({}))
            .unwrap();
        analytics
            .log_structured_event("session1", EventType::SessionEnd, serde_json::json!({}))
            .unwrap();

        let filter = EventFilter::new().with_event_type(EventType::Iteration);
        let events = analytics.read_structured_events_filtered(&filter).unwrap();

        assert_eq!(events.len(), 2);
        assert!(events
            .iter()
            .all(|e| e.event_type == EventType::Iteration));
    }

    #[test]
    fn test_filter_events_by_multiple_types() {
        let temp = TempDir::new().unwrap();
        let analytics = Analytics::new(temp.path().to_path_buf());

        analytics
            .log_structured_event("session1", EventType::SessionStart, serde_json::json!({}))
            .unwrap();
        analytics
            .log_structured_event("session1", EventType::GateResult, serde_json::json!({}))
            .unwrap();
        analytics
            .log_structured_event("session1", EventType::PredictorDecision, serde_json::json!({}))
            .unwrap();
        analytics
            .log_structured_event("session1", EventType::SessionEnd, serde_json::json!({}))
            .unwrap();

        let filter = EventFilter::new()
            .with_event_type(EventType::GateResult)
            .with_event_type(EventType::PredictorDecision);
        let events = analytics.read_structured_events_filtered(&filter).unwrap();

        assert_eq!(events.len(), 2);
    }

    #[test]
    fn test_filter_events_by_session() {
        let temp = TempDir::new().unwrap();
        let analytics = Analytics::new(temp.path().to_path_buf());

        analytics
            .log_structured_event("session1", EventType::Iteration, serde_json::json!({}))
            .unwrap();
        analytics
            .log_structured_event("session2", EventType::Iteration, serde_json::json!({}))
            .unwrap();
        analytics
            .log_structured_event("session1", EventType::Iteration, serde_json::json!({}))
            .unwrap();

        let filter = EventFilter::new().with_session_id("session1");
        let events = analytics.read_structured_events_filtered(&filter).unwrap();

        assert_eq!(events.len(), 2);
        assert!(events.iter().all(|e| e.session_id == "session1"));
    }

    #[test]
    fn test_filter_events_combined() {
        let temp = TempDir::new().unwrap();
        let analytics = Analytics::new(temp.path().to_path_buf());

        analytics
            .log_structured_event("session1", EventType::Iteration, serde_json::json!({}))
            .unwrap();
        analytics
            .log_structured_event("session1", EventType::GateResult, serde_json::json!({}))
            .unwrap();
        analytics
            .log_structured_event("session2", EventType::Iteration, serde_json::json!({}))
            .unwrap();
        analytics
            .log_structured_event("session2", EventType::GateResult, serde_json::json!({}))
            .unwrap();

        let filter = EventFilter::new()
            .with_session_id("session1")
            .with_event_type(EventType::GateResult);
        let events = analytics.read_structured_events_filtered(&filter).unwrap();

        assert_eq!(events.len(), 1);
        assert_eq!(events[0].session_id, "session1");
        assert_eq!(events[0].event_type, EventType::GateResult);
    }

    #[test]
    fn test_event_filter_default_returns_all() {
        let temp = TempDir::new().unwrap();
        let analytics = Analytics::new(temp.path().to_path_buf());

        analytics
            .log_structured_event("session1", EventType::SessionStart, serde_json::json!({}))
            .unwrap();
        analytics
            .log_structured_event("session1", EventType::Iteration, serde_json::json!({}))
            .unwrap();

        let filter = EventFilter::new();
        let events = analytics.read_structured_events_filtered(&filter).unwrap();

        assert_eq!(events.len(), 2);
    }
}
