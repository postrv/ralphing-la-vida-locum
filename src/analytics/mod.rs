//! Analytics and logging for the automation suite.
//!
//! This module handles session tracking, event logging, and
//! performance analytics in JSONL format.

pub mod cost;
pub mod dashboard;
pub mod events;
pub mod reporting;
pub mod session;
pub mod storage;
pub mod trends;

pub use cost::{CostData, CostTracker, ProviderCost, SessionCost};
pub use events::{
    EventFilter, EventType, GateIssueEventData, GateResultEventData, PredictorDecisionEventData,
    StructuredEvent, SCHEMA_VERSION,
};
pub use reporting::{GateStats, ReportFormat, SessionReport};
pub use session::{AggregateStats, PredictorAccuracyStats};
pub use storage::{AnalyticsUploadConfig, AnalyticsUploader, PrivacySettings, StubAnalyticsUploader};
pub use trends::{
    calculate_trend_direction, QualityMetricsSnapshot, QualityTrend, TrendData, TrendDirection,
    TrendMetric, TrendPoint,
};

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
            calculate_trend_direction(clippy_delta, test_failures_delta, security_delta);

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

    // ========================================================================
    // Phase 16.2: Session Report Generation
    // ========================================================================

    /// Generate a comprehensive session report from logged events.
    ///
    /// Reads all structured events for the specified session and aggregates
    /// them into a summary report.
    ///
    /// # Arguments
    ///
    /// * `session_id` - The session identifier to generate the report for
    ///
    /// # Errors
    ///
    /// Returns an error if reading events fails.
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// use ralph::analytics::Analytics;
    ///
    /// let analytics = Analytics::new(project_dir);
    /// let report = analytics.generate_session_report("session-123")?;
    /// println!("{}", report.to_markdown());
    /// ```
    pub fn generate_session_report(&self, session_id: &str) -> Result<SessionReport> {
        let filter = EventFilter::new().with_session_id(session_id);
        let events = self.read_structured_events_filtered(&filter)?;

        let mut report = SessionReport::new(session_id);
        let mut started_at: Option<DateTime<Utc>> = None;
        let mut ended_at: Option<DateTime<Utc>> = None;

        for event in &events {
            match event.event_type {
                EventType::SessionStart => {
                    started_at = Some(event.timestamp);
                    if let Some(mode) = event.data.get("mode").and_then(|v| v.as_str()) {
                        report = report.with_mode(mode);
                    }
                }
                EventType::SessionEnd => {
                    ended_at = Some(event.timestamp);
                    // Extract predictor accuracy from session end event
                    if let Some(accuracy) = event
                        .data
                        .get("predictor_accuracy")
                        .and_then(|v| v.as_f64())
                    {
                        report = report.with_predictor_accuracy(accuracy);
                    }
                    // Extract tasks completed from session end event
                    if let Some(tasks) = event.data.get("tasks_completed").and_then(|v| v.as_u64())
                    {
                        report = report.with_tasks_completed(tasks as usize);
                    }
                }
                EventType::Iteration => {
                    report.iterations += 1;
                    // Check for stagnation in iteration data
                    if let Some(stagnation) = event.data.get("stagnation").and_then(|v| v.as_u64())
                    {
                        if stagnation > 0 {
                            report.stagnations += 1;
                        }
                    }
                }
                EventType::IterationError => {
                    report.errors += 1;
                }
                EventType::Stagnation => {
                    report.stagnations += 1;
                }
                EventType::GateResult => {
                    report.gate_stats.total_runs += 1;
                    if let Some(passed) = event.data.get("passed").and_then(|v| v.as_bool()) {
                        if passed {
                            report.gate_stats.passed += 1;
                        } else {
                            report.gate_stats.failed += 1;
                        }
                    }
                }
                _ => {}
            }
        }

        // Calculate duration if we have both start and end times
        if let (Some(start), Some(end)) = (started_at, ended_at) {
            let duration = end - start;
            report = report.with_duration_seconds(duration.num_seconds() as u64);
        }

        Ok(report)
    }

    // ========================================================================
    // Phase 16.3: Quality Trend Visualization
    // ========================================================================

    /// Get aggregated trend data across sessions.
    ///
    /// Collects quality metrics from all sessions and aggregates them into
    /// trend data suitable for visualization and export.
    ///
    /// # Arguments
    ///
    /// * `days` - Optional number of days to include. `None` means all data.
    ///
    /// # Errors
    ///
    /// Returns an error if reading the analytics file fails.
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// use ralph::analytics::Analytics;
    ///
    /// let analytics = Analytics::new(project_dir);
    /// let trend_data = analytics.get_trend_data(Some(30))?; // Last 30 days
    /// println!("{}", trend_data.render_ascii_chart(TrendMetric::Warnings, 60, 15));
    /// ```
    pub fn get_trend_data(&self, days: Option<u32>) -> Result<TrendData> {
        let cutoff = days.map(|d| Utc::now() - chrono::Duration::days(i64::from(d)));

        // Get quality metrics snapshots
        let all_snapshots = self.get_quality_metrics_history(None, usize::MAX)?;

        // Filter by time range if specified
        let snapshots: Vec<_> = if let Some(cutoff_time) = cutoff {
            all_snapshots
                .into_iter()
                .filter(|s| s.timestamp >= cutoff_time)
                .collect()
        } else {
            all_snapshots
        };

        // Build trend data from snapshots (snapshots are newest-first from get_quality_metrics_history)
        let mut trend_data = TrendData::default();

        for snapshot in &snapshots {
            trend_data.warning_count_points.push(TrendPoint {
                timestamp: snapshot.timestamp,
                value: snapshot.clippy_warnings as i64,
            });

            trend_data.test_count_points.push(TrendPoint {
                timestamp: snapshot.timestamp,
                value: snapshot.test_total as i64,
            });

            if let Some(rate) = snapshot.test_pass_rate() {
                trend_data.test_pass_rate_points.push(TrendPoint {
                    timestamp: snapshot.timestamp,
                    value: (rate * 100.0) as i64, // Store as percentage
                });
            }

            trend_data.security_issue_points.push(TrendPoint {
                timestamp: snapshot.timestamp,
                value: snapshot.security_issues as i64,
            });
        }

        // Get commit data from structured events
        let events = self.read_structured_events()?;
        let commit_events: Vec<_> = events
            .into_iter()
            .filter(|e| e.event_type == EventType::SessionEnd)
            .filter(|e| cutoff.is_none_or(|c| e.timestamp >= c))
            .collect();

        for event in commit_events {
            if let Some(commits) = event.data.get("commits").and_then(|v| v.as_i64()) {
                trend_data.commit_points.push(TrendPoint {
                    timestamp: event.timestamp,
                    value: commits,
                });
            }
        }

        // Sort commit points newest first
        trend_data
            .commit_points
            .sort_by(|a, b| b.timestamp.cmp(&a.timestamp));

        Ok(trend_data)
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
        let event =
            StructuredEvent::new("test-session", EventType::GateResult, serde_json::json!({}));

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
        assert_eq!(restored.action_recommended, Some("checkpoint".to_string()));
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
        assert!(events.iter().all(|e| e.event_type == EventType::Iteration));
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
            .log_structured_event(
                "session1",
                EventType::PredictorDecision,
                serde_json::json!({}),
            )
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

    // ========================================================================
    // Phase 16.2: Session Summary Report Tests
    // ========================================================================

    #[test]
    fn test_session_report_includes_iteration_count() {
        let report = SessionReport::new("test-session").with_iterations(10);

        assert_eq!(report.session_id, "test-session");
        assert_eq!(report.iterations, 10);
    }

    #[test]
    fn test_session_report_includes_tasks_completed() {
        let report = SessionReport::new("test-session").with_tasks_completed(5);

        assert_eq!(report.tasks_completed, 5);
    }

    #[test]
    fn test_session_report_includes_gate_pass_fail_rates() {
        let report = SessionReport::new("test-session").with_gate_stats(GateStats {
            total_runs: 20,
            passed: 18,
            failed: 2,
        });

        assert_eq!(report.gate_stats.total_runs, 20);
        assert_eq!(report.gate_stats.passed, 18);
        assert_eq!(report.gate_stats.failed, 2);
        assert!((report.gate_stats.pass_rate() - 0.9).abs() < 0.001);
    }

    #[test]
    fn test_session_report_includes_predictor_accuracy() {
        let report = SessionReport::new("test-session").with_predictor_accuracy(0.85);

        assert_eq!(report.predictor_accuracy, Some(0.85));
    }

    #[test]
    fn test_session_report_export_json() {
        let report = SessionReport::new("test-session")
            .with_iterations(10)
            .with_tasks_completed(5)
            .with_gate_stats(GateStats {
                total_runs: 20,
                passed: 18,
                failed: 2,
            })
            .with_predictor_accuracy(0.85);

        let json = report.to_json().unwrap();

        // to_string_pretty adds spaces after colons
        assert!(json.contains("\"session_id\": \"test-session\""));
        assert!(json.contains("\"iterations\": 10"));
        assert!(json.contains("\"tasks_completed\": 5"));
        assert!(json.contains("\"predictor_accuracy\": 0.85"));
    }

    #[test]
    fn test_session_report_export_markdown() {
        let report = SessionReport::new("test-session")
            .with_iterations(10)
            .with_tasks_completed(5)
            .with_gate_stats(GateStats {
                total_runs: 20,
                passed: 18,
                failed: 2,
            })
            .with_predictor_accuracy(0.85);

        let markdown = report.to_markdown();

        assert!(markdown.contains("# Session Report"));
        assert!(markdown.contains("test-session"));
        assert!(markdown.contains("10")); // iterations
        assert!(markdown.contains("5")); // tasks completed
        assert!(markdown.contains("90.0%")); // pass rate
        assert!(markdown.contains("85.0%")); // predictor accuracy
    }

    #[test]
    fn test_gate_stats_pass_rate_zero_runs() {
        let stats = GateStats {
            total_runs: 0,
            passed: 0,
            failed: 0,
        };

        assert!(stats.pass_rate().is_nan() || stats.pass_rate() == 0.0);
    }

    #[test]
    fn test_gate_stats_pass_rate_all_passed() {
        let stats = GateStats {
            total_runs: 10,
            passed: 10,
            failed: 0,
        };

        assert!((stats.pass_rate() - 1.0).abs() < 0.001);
    }

    #[test]
    fn test_gate_stats_pass_rate_all_failed() {
        let stats = GateStats {
            total_runs: 10,
            passed: 0,
            failed: 10,
        };

        assert!((stats.pass_rate() - 0.0).abs() < 0.001);
    }

    #[test]
    fn test_session_report_builder_chain() {
        let report = SessionReport::new("my-session")
            .with_iterations(15)
            .with_tasks_completed(3)
            .with_stagnations(2)
            .with_errors(1)
            .with_duration_seconds(3600)
            .with_predictor_accuracy(0.75);

        assert_eq!(report.session_id, "my-session");
        assert_eq!(report.iterations, 15);
        assert_eq!(report.tasks_completed, 3);
        assert_eq!(report.stagnations, 2);
        assert_eq!(report.errors, 1);
        assert_eq!(report.duration_seconds, Some(3600));
        assert_eq!(report.predictor_accuracy, Some(0.75));
    }

    #[test]
    fn test_session_report_serialization_roundtrip() {
        let report = SessionReport::new("session-123")
            .with_iterations(20)
            .with_tasks_completed(8)
            .with_gate_stats(GateStats {
                total_runs: 50,
                passed: 45,
                failed: 5,
            })
            .with_predictor_accuracy(0.9);

        let json = serde_json::to_string(&report).unwrap();
        let restored: SessionReport = serde_json::from_str(&json).unwrap();

        assert_eq!(restored.session_id, "session-123");
        assert_eq!(restored.iterations, 20);
        assert_eq!(restored.tasks_completed, 8);
        assert_eq!(restored.gate_stats.total_runs, 50);
        assert_eq!(restored.predictor_accuracy, Some(0.9));
    }

    #[test]
    fn test_session_report_from_events() {
        let temp = TempDir::new().unwrap();
        let analytics = Analytics::new(temp.path().to_path_buf());

        // Log session start
        analytics
            .log_structured_event(
                "session1",
                EventType::SessionStart,
                serde_json::json!({"mode": "build"}),
            )
            .unwrap();

        // Log iterations
        for i in 1..=5 {
            analytics
                .log_structured_event(
                    "session1",
                    EventType::Iteration,
                    serde_json::json!({"iteration": i}),
                )
                .unwrap();
        }

        // Log gate results
        let gate_pass = GateResultEventData {
            gate_name: "clippy".to_string(),
            passed: true,
            issue_count: 0,
            duration_ms: 1000,
            issues: vec![],
        };
        analytics
            .log_structured_gate_result("session1", &gate_pass)
            .unwrap();

        let gate_fail = GateResultEventData {
            gate_name: "test".to_string(),
            passed: false,
            issue_count: 2,
            duration_ms: 2000,
            issues: vec![],
        };
        analytics
            .log_structured_gate_result("session1", &gate_fail)
            .unwrap();

        // Log session end with predictor accuracy
        analytics
            .log_structured_event(
                "session1",
                EventType::SessionEnd,
                serde_json::json!({"predictor_accuracy": 0.8}),
            )
            .unwrap();

        // Generate report from events
        let report = analytics.generate_session_report("session1").unwrap();

        assert_eq!(report.session_id, "session1");
        assert_eq!(report.iterations, 5);
        assert_eq!(report.gate_stats.total_runs, 2);
        assert_eq!(report.gate_stats.passed, 1);
        assert_eq!(report.gate_stats.failed, 1);
        assert_eq!(report.predictor_accuracy, Some(0.8));
    }

    #[test]
    fn test_session_report_from_events_with_tasks() {
        let temp = TempDir::new().unwrap();
        let analytics = Analytics::new(temp.path().to_path_buf());

        // Log session with task completions
        analytics
            .log_structured_event("session1", EventType::SessionStart, serde_json::json!({}))
            .unwrap();

        // Log task completions as custom events
        for i in 1..=3 {
            analytics
                .log_structured_event(
                    "session1",
                    EventType::Iteration,
                    serde_json::json!({
                        "iteration": i,
                        "task_completed": true
                    }),
                )
                .unwrap();
        }

        // One iteration without task completion
        analytics
            .log_structured_event(
                "session1",
                EventType::Iteration,
                serde_json::json!({
                    "iteration": 4,
                    "task_completed": false
                }),
            )
            .unwrap();

        analytics
            .log_structured_event(
                "session1",
                EventType::SessionEnd,
                serde_json::json!({"tasks_completed": 3}),
            )
            .unwrap();

        let report = analytics.generate_session_report("session1").unwrap();

        assert_eq!(report.iterations, 4);
        assert_eq!(report.tasks_completed, 3);
    }

    #[test]
    fn test_session_report_markdown_formatting() {
        let report = SessionReport::new("test-session")
            .with_iterations(10)
            .with_tasks_completed(3)
            .with_stagnations(1)
            .with_errors(0)
            .with_gate_stats(GateStats {
                total_runs: 15,
                passed: 14,
                failed: 1,
            })
            .with_duration_seconds(1800)
            .with_predictor_accuracy(0.92);

        let markdown = report.to_markdown();

        // Check structure
        assert!(markdown.contains("# Session Report"));
        assert!(markdown.contains("## Summary"));
        assert!(markdown.contains("## Quality Gates"));
        assert!(markdown.contains("## Performance"));

        // Check content
        assert!(markdown.contains("**Session ID:**"));
        assert!(markdown.contains("**Iterations:**"));
        assert!(markdown.contains("**Tasks Completed:**"));
        assert!(markdown.contains("**Pass Rate:**"));
    }

    #[test]
    fn test_session_report_json_is_valid() {
        let report = SessionReport::new("test-session")
            .with_iterations(5)
            .with_tasks_completed(2);

        let json = report.to_json().unwrap();

        // Should be valid JSON that can be parsed back
        let value: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert!(value.is_object());
        assert_eq!(value["session_id"], "test-session");
    }

    #[test]
    fn test_report_format_enum() {
        assert_eq!(ReportFormat::Json.extension(), "json");
        assert_eq!(ReportFormat::Markdown.extension(), "md");
    }

    #[test]
    fn test_session_report_export_to_format() {
        let report = SessionReport::new("test-session").with_iterations(5);

        let json_output = report.export(ReportFormat::Json).unwrap();
        assert!(json_output.contains("\"session_id\""));

        let markdown_output = report.export(ReportFormat::Markdown).unwrap();
        assert!(markdown_output.contains("# Session Report"));
    }

    // ========================================================================
    // Phase 16.3: Quality Trend Visualization Tests
    // ========================================================================

    #[test]
    fn test_trend_data_shows_test_count_over_sessions() {
        let temp = TempDir::new().unwrap();
        let analytics = Analytics::new(temp.path().to_path_buf());

        // Log quality metrics for multiple sessions
        analytics
            .log_quality_metrics(
                &QualityMetricsSnapshot::new("session-1", 1).with_test_counts(100, 95, 5),
            )
            .unwrap();
        analytics
            .log_quality_metrics(
                &QualityMetricsSnapshot::new("session-2", 1).with_test_counts(110, 108, 2),
            )
            .unwrap();

        let trend_data = analytics.get_trend_data(None).unwrap();

        assert_eq!(trend_data.test_count_points.len(), 2);
        // Newest first
        assert_eq!(trend_data.test_count_points[0].value, 110);
        assert_eq!(trend_data.test_count_points[1].value, 100);
    }

    #[test]
    fn test_trend_data_shows_warning_count_over_sessions() {
        let temp = TempDir::new().unwrap();
        let analytics = Analytics::new(temp.path().to_path_buf());

        // Log quality metrics with decreasing warnings
        analytics
            .log_quality_metrics(
                &QualityMetricsSnapshot::new("session-1", 1).with_clippy_warnings(10),
            )
            .unwrap();
        std::thread::sleep(std::time::Duration::from_millis(10));
        analytics
            .log_quality_metrics(
                &QualityMetricsSnapshot::new("session-2", 1).with_clippy_warnings(5),
            )
            .unwrap();
        std::thread::sleep(std::time::Duration::from_millis(10));
        analytics
            .log_quality_metrics(
                &QualityMetricsSnapshot::new("session-3", 1).with_clippy_warnings(2),
            )
            .unwrap();

        let trend_data = analytics.get_trend_data(None).unwrap();

        assert_eq!(trend_data.warning_count_points.len(), 3);
        // Newest first
        assert_eq!(trend_data.warning_count_points[0].value, 2);
        assert_eq!(trend_data.warning_count_points[2].value, 10);
    }

    #[test]
    fn test_trend_data_shows_commit_frequency() {
        let temp = TempDir::new().unwrap();
        let analytics = Analytics::new(temp.path().to_path_buf());

        // Log session events with commits
        analytics
            .log_structured_event(
                "session-1",
                EventType::SessionEnd,
                serde_json::json!({"commits": 3}),
            )
            .unwrap();
        std::thread::sleep(std::time::Duration::from_millis(10));
        analytics
            .log_structured_event(
                "session-2",
                EventType::SessionEnd,
                serde_json::json!({"commits": 5}),
            )
            .unwrap();

        let trend_data = analytics.get_trend_data(None).unwrap();

        assert_eq!(trend_data.commit_points.len(), 2);
        assert_eq!(trend_data.commit_points[0].value, 5);
        assert_eq!(trend_data.commit_points[1].value, 3);
    }

    #[test]
    fn test_trend_data_ascii_chart_output() {
        let temp = TempDir::new().unwrap();
        let analytics = Analytics::new(temp.path().to_path_buf());

        // Log some metrics
        for i in 1..=5 {
            analytics
                .log_quality_metrics(
                    &QualityMetricsSnapshot::new(format!("session-{}", i), 1)
                        .with_clippy_warnings(10 - i),
                )
                .unwrap();
            std::thread::sleep(std::time::Duration::from_millis(10));
        }

        let trend_data = analytics.get_trend_data(None).unwrap();
        let chart = trend_data.render_ascii_chart(TrendMetric::Warnings, 40, 10);

        // Chart should contain expected elements
        assert!(chart.contains("Warnings")); // Title
        assert!(chart.lines().count() >= 5); // Has multiple lines
    }

    #[test]
    fn test_trend_data_json_export() {
        let temp = TempDir::new().unwrap();
        let analytics = Analytics::new(temp.path().to_path_buf());

        analytics
            .log_quality_metrics(
                &QualityMetricsSnapshot::new("session-1", 1)
                    .with_clippy_warnings(5)
                    .with_test_counts(50, 48, 2),
            )
            .unwrap();

        let trend_data = analytics.get_trend_data(None).unwrap();
        let json = trend_data.to_json().unwrap();

        // Should be valid JSON
        let value: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert!(value.is_object());
        assert!(value.get("warning_count_points").is_some());
        assert!(value.get("test_count_points").is_some());
    }

    #[test]
    fn test_trend_data_filtered_by_days() {
        let temp = TempDir::new().unwrap();
        let analytics = Analytics::new(temp.path().to_path_buf());

        // Log a recent metric
        analytics
            .log_quality_metrics(
                &QualityMetricsSnapshot::new("session-recent", 1).with_clippy_warnings(3),
            )
            .unwrap();

        // Filter to last 7 days (recent data should be included)
        let trend_data = analytics.get_trend_data(Some(7)).unwrap();

        assert!(!trend_data.warning_count_points.is_empty());
    }

    #[test]
    fn test_trend_point_structure() {
        let point = TrendPoint::new(5);
        assert_eq!(point.value, 5);
        assert!(point.timestamp <= Utc::now());
    }

    #[test]
    fn test_trend_data_empty_when_no_data() {
        let temp = TempDir::new().unwrap();
        let analytics = Analytics::new(temp.path().to_path_buf());

        let trend_data = analytics.get_trend_data(None).unwrap();

        assert!(trend_data.warning_count_points.is_empty());
        assert!(trend_data.test_count_points.is_empty());
        assert!(trend_data.commit_points.is_empty());
    }

    #[test]
    fn test_trend_metric_enum() {
        assert_eq!(TrendMetric::Warnings.label(), "Warnings");
        assert_eq!(TrendMetric::TestCount.label(), "Test Count");
        assert_eq!(TrendMetric::Commits.label(), "Commits");
        assert_eq!(TrendMetric::TestPassRate.label(), "Test Pass Rate (%)");
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

    // ========================================================================
    // Analytics Upload Stub Tests (Phase 18.1)
    // ========================================================================

    #[test]
    fn test_analytics_upload_config_disabled_by_default() {
        let config = AnalyticsUploadConfig::default();
        assert!(!config.upload_enabled);
    }

    #[test]
    fn test_analytics_upload_config_can_be_enabled() {
        let config = AnalyticsUploadConfig {
            upload_enabled: true,
            ..Default::default()
        };
        assert!(config.upload_enabled);
    }

    #[test]
    fn test_analytics_uploader_stub_logs_to_file() {
        let temp = TempDir::new().unwrap();
        let log_path = temp.path().join("upload_log.jsonl");

        let config = AnalyticsUploadConfig {
            upload_enabled: true,
            endpoint_url: "https://example.com/analytics".to_string(),
            log_file: Some(log_path.clone()),
            privacy: PrivacySettings {
                anonymize_session_ids: false, // Disable for this test
                exclude_event_data: false,
                include_only_aggregates: false,
            },
        };

        let uploader = StubAnalyticsUploader::new(config);

        // Create a sample event to upload
        let event = AnalyticsEvent {
            session: "test-session".to_string(),
            event: "test_event".to_string(),
            timestamp: Utc::now(),
            data: serde_json::json!({"key": "value"}),
        };

        uploader.upload(&[event]).unwrap();

        // Verify the log file contains what would have been uploaded
        let log_content = std::fs::read_to_string(&log_path).unwrap();
        assert!(log_content.contains("test-session"));
        assert!(log_content.contains("test_event"));
        assert!(log_content.contains("STUB: Would upload"));
    }

    #[test]
    fn test_analytics_uploader_respects_privacy_settings() {
        let temp = TempDir::new().unwrap();
        let log_path = temp.path().join("upload_log.jsonl");

        let config = AnalyticsUploadConfig {
            upload_enabled: true,
            log_file: Some(log_path.clone()),
            privacy: PrivacySettings {
                anonymize_session_ids: true,
                exclude_event_data: true,
                include_only_aggregates: false,
            },
            ..Default::default()
        };

        let uploader = StubAnalyticsUploader::new(config);

        let event = AnalyticsEvent {
            session: "my-unique-session-id-12345".to_string(),
            event: "test_event".to_string(),
            timestamp: Utc::now(),
            data: serde_json::json!({"sensitive": "data"}),
        };

        uploader.upload(&[event]).unwrap();

        let log_content = std::fs::read_to_string(&log_path).unwrap();

        // Session ID should be anonymized (hashed)
        assert!(!log_content.contains("my-unique-session-id-12345"));

        // Event data should be excluded
        assert!(!log_content.contains("sensitive"));
    }

    #[test]
    fn test_analytics_uploader_failure_does_not_affect_operation() {
        // Create uploader with invalid log path to simulate failure
        let config = AnalyticsUploadConfig {
            upload_enabled: true,
            log_file: Some(PathBuf::from("/nonexistent/path/that/cannot/be/written")),
            ..Default::default()
        };

        let uploader = StubAnalyticsUploader::new(config);

        let event = AnalyticsEvent {
            session: "test-session".to_string(),
            event: "test_event".to_string(),
            timestamp: Utc::now(),
            data: serde_json::json!({}),
        };

        // upload_graceful should return Ok even on failure
        let result = uploader.upload_graceful(&[event]);
        assert!(result.is_ok());
    }

    #[test]
    fn test_analytics_uploader_skips_when_disabled() {
        let temp = TempDir::new().unwrap();
        let log_path = temp.path().join("upload_log.jsonl");

        let config = AnalyticsUploadConfig {
            upload_enabled: false, // Disabled
            log_file: Some(log_path.clone()),
            ..Default::default()
        };

        let uploader = StubAnalyticsUploader::new(config);

        let event = AnalyticsEvent {
            session: "test-session".to_string(),
            event: "test_event".to_string(),
            timestamp: Utc::now(),
            data: serde_json::json!({}),
        };

        uploader.upload(&[event]).unwrap();

        // Log file should not exist since upload is disabled
        assert!(!log_path.exists());
    }

    #[test]
    fn test_analytics_uploader_trait_stub_implements() {
        let config = AnalyticsUploadConfig::default();
        let uploader = StubAnalyticsUploader::new(config);

        // Verify it implements the trait (compile-time check via dyn)
        let _boxed: Box<dyn AnalyticsUploader> = Box::new(uploader);
    }

    #[test]
    fn test_privacy_settings_default() {
        let settings = PrivacySettings::default();

        // Default should be privacy-preserving
        assert!(settings.anonymize_session_ids);
        assert!(!settings.exclude_event_data);
        assert!(!settings.include_only_aggregates);
    }

    #[test]
    fn test_analytics_upload_config_serialization() {
        let config = AnalyticsUploadConfig {
            upload_enabled: true,
            endpoint_url: "https://analytics.example.com".to_string(),
            log_file: Some(PathBuf::from("/tmp/analytics.log")),
            privacy: PrivacySettings {
                anonymize_session_ids: true,
                exclude_event_data: false,
                include_only_aggregates: true,
            },
        };

        let json = serde_json::to_string(&config).unwrap();
        let restored: AnalyticsUploadConfig = serde_json::from_str(&json).unwrap();

        assert_eq!(restored.upload_enabled, config.upload_enabled);
        assert_eq!(restored.endpoint_url, config.endpoint_url);
        assert_eq!(
            restored.privacy.include_only_aggregates,
            config.privacy.include_only_aggregates
        );
    }
}
