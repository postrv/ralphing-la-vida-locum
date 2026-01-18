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
    fn build_session_summary(&self, session_id: &str, events: &[&AnalyticsEvent]) -> SessionSummary {
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
                    if let Some(stagnation) = event.data.get("stagnation").and_then(|v| v.as_u64()) {
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

        println!(
            "\n{} Recent Sessions",
            "Analytics:".cyan().bold()
        );
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
                session.iterations,
                session.stagnations,
                session.errors
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
        let test_rates: Vec<f32> = snapshots.iter().filter_map(|s| s.test_pass_rate()).collect();
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
        let overall = Self::calculate_trend_direction(clippy_delta, test_failures_delta, security_delta);

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
    // Audit Logging Methods
    // ========================================================================

    /// Get the audit log file path.
    fn audit_file(&self) -> PathBuf {
        self.project_dir.join(".ralph/audit.jsonl")
    }

    /// Log an audit event.
    ///
    /// Records a structured audit event to the audit log for compliance
    /// and debugging purposes.
    ///
    /// # Arguments
    ///
    /// * `event` - The audit event to log
    ///
    /// # Errors
    ///
    /// Returns an error if writing to the audit log fails.
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// use ralph::analytics::{Analytics, AuditEvent, AuditAction};
    ///
    /// let analytics = Analytics::new(project_dir);
    /// let event = AuditEvent::new("session-1", AuditAction::SessionStart);
    /// analytics.log_audit_event(&event)?;
    /// ```
    pub fn log_audit_event(&self, event: &AuditEvent) -> Result<()> {
        self.ensure_dir()?;

        let mut file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(self.audit_file())?;

        let json = serde_json::to_string(&event)?;
        writeln!(file, "{}", json)?;

        Ok(())
    }

    /// Log an API action.
    ///
    /// Convenience method for logging loop manager and API operations.
    ///
    /// # Arguments
    ///
    /// * `session_id` - The session identifier
    /// * `action` - The action being performed
    /// * `details` - Human-readable description
    ///
    /// # Errors
    ///
    /// Returns an error if writing to the audit log fails.
    pub fn log_api_action(
        &self,
        session_id: &str,
        action: AuditAction,
        details: &str,
    ) -> Result<()> {
        let event = AuditEvent::new(session_id, action).with_details(details);
        self.log_audit_event(&event)
    }

    /// Log a campaign execution.
    ///
    /// Records the execution of a loop iteration or campaign, including
    /// the mode, task being worked on, and outcome.
    ///
    /// # Arguments
    ///
    /// * `session_id` - The session identifier
    /// * `mode` - The execution mode (e.g., "build", "debug")
    /// * `task_id` - Optional task identifier
    /// * `outcome` - The outcome of the execution
    ///
    /// # Errors
    ///
    /// Returns an error if writing to the audit log fails.
    pub fn log_campaign_execution(
        &self,
        session_id: &str,
        mode: &str,
        task_id: Option<&str>,
        outcome: CampaignOutcome,
    ) -> Result<()> {
        let severity = match outcome {
            CampaignOutcome::Success => AuditSeverity::Info,
            CampaignOutcome::Failure | CampaignOutcome::Stagnated => AuditSeverity::High,
            CampaignOutcome::Aborted | CampaignOutcome::Timeout => AuditSeverity::Medium,
        };

        let mut event = AuditEvent::new(session_id, AuditAction::IterationEnd)
            .with_severity(severity)
            .with_details(format!("Campaign {} completed with outcome: {}", mode, outcome))
            .with_metadata("mode", serde_json::json!(mode))
            .with_metadata("outcome", serde_json::json!(outcome.to_string()));

        if let Some(task) = task_id {
            event = event.with_task_id(task);
        }

        self.log_audit_event(&event)
    }

    /// Log a quality gate decision.
    ///
    /// Records the result of running a quality gate, including whether
    /// it passed or failed and any relevant details.
    ///
    /// # Arguments
    ///
    /// * `session_id` - The session identifier
    /// * `gate_name` - Name of the quality gate
    /// * `passed` - Whether the gate passed
    /// * `reason` - Optional reason for failure
    /// * `issues` - Optional list of specific issues
    ///
    /// # Errors
    ///
    /// Returns an error if writing to the audit log fails.
    pub fn log_quality_decision(
        &self,
        session_id: &str,
        gate_name: &str,
        passed: bool,
        reason: Option<&str>,
        issues: Option<Vec<&str>>,
    ) -> Result<()> {
        let action = if passed {
            AuditAction::QualityGatePass
        } else {
            AuditAction::QualityGateFail
        };

        let severity = if passed {
            AuditSeverity::Info
        } else {
            AuditSeverity::High
        };

        let mut event = AuditEvent::new(session_id, action)
            .with_severity(severity)
            .with_gate_name(gate_name)
            .with_metadata("passed", serde_json::json!(passed));

        if let Some(r) = reason {
            event = event.with_details(r);
        }

        if let Some(i) = issues {
            event = event.with_metadata("issues", serde_json::json!(i));
        }

        self.log_audit_event(&event)
    }

    /// Get audit events from the log.
    ///
    /// Retrieves audit events with optional filtering by session and action.
    ///
    /// # Arguments
    ///
    /// * `session_id` - Optional session ID to filter by
    /// * `action` - Optional action type to filter by
    /// * `limit` - Maximum number of events to return
    ///
    /// # Errors
    ///
    /// Returns an error if reading the audit log fails.
    pub fn get_audit_events(
        &self,
        session_id: Option<&str>,
        action: Option<AuditAction>,
        limit: usize,
    ) -> Result<Vec<AuditEvent>> {
        let file_path = self.audit_file();

        if !file_path.exists() {
            return Ok(Vec::new());
        }

        let file = File::open(&file_path).context("Failed to open audit log")?;
        let reader = BufReader::new(file);

        let mut events: Vec<AuditEvent> = reader
            .lines()
            .map_while(Result::ok)
            .filter_map(|line| serde_json::from_str::<AuditEvent>(&line).ok())
            .filter(|e| session_id.is_none_or(|sid| e.session_id == sid))
            .filter(|e| action.is_none_or(|a| e.action == a))
            .collect();

        // Sort by timestamp (most recent last for chronological order)
        events.sort_by(|a, b| a.timestamp.cmp(&b.timestamp));

        // Truncate to limit
        events.truncate(limit);

        Ok(events)
    }

    /// Export audit logs in the specified format.
    ///
    /// Exports audit events to a string in the requested format for
    /// external analysis or compliance reporting.
    ///
    /// # Arguments
    ///
    /// * `format` - The export format (JSON, JSONL, or CSV)
    /// * `session_id` - Optional session ID to filter by
    /// * `limit` - Maximum number of events to export
    ///
    /// # Errors
    ///
    /// Returns an error if reading the audit log or serialization fails.
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// use ralph::analytics::{Analytics, AuditExportFormat};
    ///
    /// let analytics = Analytics::new(project_dir);
    /// let json = analytics.export_audit_log(AuditExportFormat::Json, None, 100)?;
    /// println!("{}", json);
    /// ```
    pub fn export_audit_log(
        &self,
        format: AuditExportFormat,
        session_id: Option<&str>,
        limit: usize,
    ) -> Result<String> {
        let events = self.get_audit_events(session_id, None, limit)?;

        match format {
            AuditExportFormat::Json => {
                serde_json::to_string_pretty(&events).context("Failed to serialize to JSON")
            }
            AuditExportFormat::Jsonl => {
                let lines: Result<Vec<String>, _> = events
                    .iter()
                    .map(serde_json::to_string)
                    .collect();
                Ok(lines?.join("\n"))
            }
            AuditExportFormat::Csv => {
                let mut csv = String::new();
                // Header
                csv.push_str("timestamp,session_id,action,severity,details,gate_name,task_id\n");
                // Data rows
                for event in events {
                    let row = format!(
                        "{},{},{},{},{},{},{}\n",
                        event.timestamp.format("%Y-%m-%dT%H:%M:%S%.3fZ"),
                        Self::escape_csv(&event.session_id),
                        event.action,
                        event.severity,
                        Self::escape_csv(&event.details.unwrap_or_default()),
                        Self::escape_csv(&event.gate_name.unwrap_or_default()),
                        Self::escape_csv(&event.task_id.unwrap_or_default()),
                    );
                    csv.push_str(&row);
                }
                Ok(csv)
            }
        }
    }

    /// Escape a string for CSV output.
    fn escape_csv(s: &str) -> String {
        if s.contains(',') || s.contains('"') || s.contains('\n') {
            format!("\"{}\"", s.replace('"', "\"\""))
        } else {
            s.to_string()
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

// ============================================================================
// Audit Logging
// ============================================================================

/// Actions that can be audited.
///
/// These represent the key operations performed by Ralph that should
/// be logged for compliance, debugging, and analysis purposes.
///
/// # Example
///
/// ```
/// use ralph::analytics::AuditAction;
///
/// let action = AuditAction::SessionStart;
/// assert_eq!(action.to_string(), "session_start");
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AuditAction {
    /// A new session has started.
    SessionStart,
    /// A session has ended.
    SessionEnd,
    /// An iteration has started.
    IterationStart,
    /// An iteration has ended.
    IterationEnd,
    /// A quality gate check was run.
    QualityGateRun,
    /// A quality gate passed.
    QualityGatePass,
    /// A quality gate failed.
    QualityGateFail,
    /// A checkpoint was created.
    CheckpointCreate,
    /// A checkpoint was restored.
    CheckpointRestore,
    /// A task has started.
    TaskStart,
    /// A task completed successfully.
    TaskComplete,
    /// A task failed.
    TaskFail,
    /// A security scan was performed.
    SecurityScan,
    /// A rollback was initiated.
    RollbackInitiated,
}

impl std::fmt::Display for AuditAction {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let s = match self {
            AuditAction::SessionStart => "session_start",
            AuditAction::SessionEnd => "session_end",
            AuditAction::IterationStart => "iteration_start",
            AuditAction::IterationEnd => "iteration_end",
            AuditAction::QualityGateRun => "quality_gate_run",
            AuditAction::QualityGatePass => "quality_gate_pass",
            AuditAction::QualityGateFail => "quality_gate_fail",
            AuditAction::CheckpointCreate => "checkpoint_create",
            AuditAction::CheckpointRestore => "checkpoint_restore",
            AuditAction::TaskStart => "task_start",
            AuditAction::TaskComplete => "task_complete",
            AuditAction::TaskFail => "task_fail",
            AuditAction::SecurityScan => "security_scan",
            AuditAction::RollbackInitiated => "rollback_initiated",
        };
        write!(f, "{}", s)
    }
}

/// Severity level for audit events.
///
/// Severity indicates the importance or urgency of an audit event.
/// Higher severity events typically require more attention.
///
/// # Example
///
/// ```
/// use ralph::analytics::AuditSeverity;
///
/// assert!(AuditSeverity::Critical > AuditSeverity::High);
/// assert!(AuditSeverity::High > AuditSeverity::Medium);
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum AuditSeverity {
    /// Informational event, no action needed.
    #[default]
    Info,
    /// Low severity event.
    Low,
    /// Medium severity event.
    Medium,
    /// High severity event, attention recommended.
    High,
    /// Critical severity event, immediate attention needed.
    Critical,
}

impl std::fmt::Display for AuditSeverity {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let s = match self {
            AuditSeverity::Info => "info",
            AuditSeverity::Low => "low",
            AuditSeverity::Medium => "medium",
            AuditSeverity::High => "high",
            AuditSeverity::Critical => "critical",
        };
        write!(f, "{}", s)
    }
}

/// Outcome of a campaign execution.
///
/// Used when logging campaign executions to record the final result.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CampaignOutcome {
    /// The campaign completed successfully.
    Success,
    /// The campaign failed.
    Failure,
    /// The campaign was aborted.
    Aborted,
    /// The campaign timed out.
    Timeout,
    /// The campaign stalled without progress.
    Stagnated,
}

impl std::fmt::Display for CampaignOutcome {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let s = match self {
            CampaignOutcome::Success => "success",
            CampaignOutcome::Failure => "failure",
            CampaignOutcome::Aborted => "aborted",
            CampaignOutcome::Timeout => "timeout",
            CampaignOutcome::Stagnated => "stagnated",
        };
        write!(f, "{}", s)
    }
}

/// Export format for audit logs.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AuditExportFormat {
    /// JSON array format.
    Json,
    /// JSON Lines format (one JSON object per line).
    Jsonl,
    /// CSV format with headers.
    Csv,
}

/// An audit event for compliance and debugging.
///
/// Audit events provide a detailed record of all significant actions
/// performed by Ralph, enabling compliance auditing, debugging, and
/// analysis of automation runs.
///
/// # Example
///
/// ```
/// use ralph::analytics::{AuditEvent, AuditAction, AuditSeverity};
///
/// let event = AuditEvent::new("session-123", AuditAction::QualityGatePass)
///     .with_severity(AuditSeverity::Info)
///     .with_gate_name("clippy")
///     .with_details("All warnings resolved");
///
/// assert_eq!(event.action, AuditAction::QualityGatePass);
/// assert_eq!(event.gate_name, Some("clippy".to_string()));
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditEvent {
    /// Timestamp when the event occurred.
    pub timestamp: DateTime<Utc>,
    /// Session ID this event belongs to.
    pub session_id: String,
    /// The action that was performed.
    pub action: AuditAction,
    /// Severity of this event.
    pub severity: AuditSeverity,
    /// Human-readable details about the event.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub details: Option<String>,
    /// Name of the quality gate (if applicable).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub gate_name: Option<String>,
    /// Task ID (if applicable).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub task_id: Option<String>,
    /// Additional metadata as key-value pairs.
    #[serde(default, skip_serializing_if = "std::collections::HashMap::is_empty")]
    pub metadata: std::collections::HashMap<String, serde_json::Value>,
}

impl AuditEvent {
    /// Create a new audit event.
    ///
    /// # Arguments
    ///
    /// * `session_id` - The session this event belongs to
    /// * `action` - The action being logged
    ///
    /// # Example
    ///
    /// ```
    /// use ralph::analytics::{AuditEvent, AuditAction};
    ///
    /// let event = AuditEvent::new("session-1", AuditAction::SessionStart);
    /// assert_eq!(event.action, AuditAction::SessionStart);
    /// ```
    #[must_use]
    pub fn new(session_id: impl Into<String>, action: AuditAction) -> Self {
        Self {
            timestamp: Utc::now(),
            session_id: session_id.into(),
            action,
            severity: AuditSeverity::default(),
            details: None,
            gate_name: None,
            task_id: None,
            metadata: std::collections::HashMap::new(),
        }
    }

    /// Set the severity level.
    #[must_use]
    pub fn with_severity(mut self, severity: AuditSeverity) -> Self {
        self.severity = severity;
        self
    }

    /// Set the details message.
    #[must_use]
    pub fn with_details(mut self, details: impl Into<String>) -> Self {
        self.details = Some(details.into());
        self
    }

    /// Set the gate name.
    #[must_use]
    pub fn with_gate_name(mut self, name: impl Into<String>) -> Self {
        self.gate_name = Some(name.into());
        self
    }

    /// Set the task ID.
    #[must_use]
    pub fn with_task_id(mut self, id: impl Into<String>) -> Self {
        self.task_id = Some(id.into());
        self
    }

    /// Add metadata key-value pair.
    #[must_use]
    pub fn with_metadata(mut self, key: impl Into<String>, value: serde_json::Value) -> Self {
        self.metadata.insert(key.into(), value);
        self
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
            .log_event("test-session", "test_event", serde_json::json!({"foo": "bar"}))
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
            .log_event("session1", "session_start", serde_json::json!({"mode": "build"}))
            .unwrap();

        analytics
            .log_event("session1", "iteration", serde_json::json!({"stagnation": 0}))
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
    // Audit Logging Tests (TDD - tests written first)
    // ========================================================================

    #[test]
    fn test_audit_action_display() {
        assert_eq!(AuditAction::SessionStart.to_string(), "session_start");
        assert_eq!(AuditAction::SessionEnd.to_string(), "session_end");
        assert_eq!(AuditAction::IterationStart.to_string(), "iteration_start");
        assert_eq!(AuditAction::IterationEnd.to_string(), "iteration_end");
        assert_eq!(AuditAction::QualityGateRun.to_string(), "quality_gate_run");
        assert_eq!(AuditAction::QualityGatePass.to_string(), "quality_gate_pass");
        assert_eq!(AuditAction::QualityGateFail.to_string(), "quality_gate_fail");
        assert_eq!(AuditAction::CheckpointCreate.to_string(), "checkpoint_create");
        assert_eq!(AuditAction::CheckpointRestore.to_string(), "checkpoint_restore");
        assert_eq!(AuditAction::TaskStart.to_string(), "task_start");
        assert_eq!(AuditAction::TaskComplete.to_string(), "task_complete");
        assert_eq!(AuditAction::TaskFail.to_string(), "task_fail");
        assert_eq!(AuditAction::SecurityScan.to_string(), "security_scan");
        assert_eq!(AuditAction::RollbackInitiated.to_string(), "rollback_initiated");
    }

    #[test]
    fn test_audit_severity_ordering() {
        assert!(AuditSeverity::Critical > AuditSeverity::High);
        assert!(AuditSeverity::High > AuditSeverity::Medium);
        assert!(AuditSeverity::Medium > AuditSeverity::Low);
        assert!(AuditSeverity::Low > AuditSeverity::Info);
    }

    #[test]
    fn test_audit_event_new() {
        let event = AuditEvent::new("session-1", AuditAction::SessionStart)
            .with_details("Starting build mode session");

        assert_eq!(event.session_id, "session-1");
        assert_eq!(event.action, AuditAction::SessionStart);
        assert_eq!(event.details, Some("Starting build mode session".to_string()));
        assert_eq!(event.severity, AuditSeverity::Info);
    }

    #[test]
    fn test_audit_event_builder() {
        let event = AuditEvent::new("session-1", AuditAction::QualityGateFail)
            .with_severity(AuditSeverity::High)
            .with_details("Clippy gate failed with 5 warnings")
            .with_gate_name("clippy")
            .with_task_id("task-123")
            .with_metadata("warnings", serde_json::json!(5));

        assert_eq!(event.action, AuditAction::QualityGateFail);
        assert_eq!(event.severity, AuditSeverity::High);
        assert_eq!(event.gate_name, Some("clippy".to_string()));
        assert_eq!(event.task_id, Some("task-123".to_string()));
        assert_eq!(event.metadata.get("warnings"), Some(&serde_json::json!(5)));
    }

    #[test]
    fn test_audit_event_serialization() {
        let event = AuditEvent::new("session-1", AuditAction::TaskComplete)
            .with_severity(AuditSeverity::Info)
            .with_task_id("task-42")
            .with_details("Task completed successfully");

        let json = serde_json::to_string(&event).unwrap();
        let restored: AuditEvent = serde_json::from_str(&json).unwrap();

        assert_eq!(restored.session_id, event.session_id);
        assert_eq!(restored.action, event.action);
        assert_eq!(restored.severity, event.severity);
        assert_eq!(restored.task_id, event.task_id);
        assert_eq!(restored.details, event.details);
    }

    #[test]
    fn test_log_audit_event() {
        let temp = TempDir::new().unwrap();
        let analytics = Analytics::new(temp.path().to_path_buf());

        let event = AuditEvent::new("session-1", AuditAction::SessionStart)
            .with_details("Build mode started");

        analytics.log_audit_event(&event).unwrap();

        let events = analytics.get_audit_events(None, None, 100).unwrap();
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].action, AuditAction::SessionStart);
    }

    #[test]
    fn test_log_api_action() {
        let temp = TempDir::new().unwrap();
        let analytics = Analytics::new(temp.path().to_path_buf());

        analytics
            .log_api_action("session-1", AuditAction::IterationStart, "Iteration 1 started")
            .unwrap();

        let events = analytics.get_audit_events(None, None, 100).unwrap();
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].action, AuditAction::IterationStart);
        assert_eq!(events[0].details, Some("Iteration 1 started".to_string()));
    }

    #[test]
    fn test_log_campaign_execution() {
        let temp = TempDir::new().unwrap();
        let analytics = Analytics::new(temp.path().to_path_buf());

        analytics
            .log_campaign_execution(
                "session-1",
                "build",
                Some("task-1"),
                CampaignOutcome::Success,
            )
            .unwrap();

        let events = analytics.get_audit_events(None, None, 100).unwrap();
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].action, AuditAction::IterationEnd);
    }

    #[test]
    fn test_log_quality_decision() {
        let temp = TempDir::new().unwrap();
        let analytics = Analytics::new(temp.path().to_path_buf());

        // Log a passing gate
        analytics
            .log_quality_decision("session-1", "clippy", true, None, None)
            .unwrap();

        // Log a failing gate
        analytics
            .log_quality_decision(
                "session-1",
                "test",
                false,
                Some("3 tests failed"),
                Some(vec!["test_foo", "test_bar", "test_baz"]),
            )
            .unwrap();

        let events = analytics.get_audit_events(None, None, 100).unwrap();
        assert_eq!(events.len(), 2);
        assert_eq!(events[0].action, AuditAction::QualityGatePass);
        assert_eq!(events[0].gate_name, Some("clippy".to_string()));
        assert_eq!(events[1].action, AuditAction::QualityGateFail);
        assert_eq!(events[1].gate_name, Some("test".to_string()));
    }

    #[test]
    fn test_get_audit_events_filtered_by_session() {
        let temp = TempDir::new().unwrap();
        let analytics = Analytics::new(temp.path().to_path_buf());

        analytics
            .log_api_action("session-1", AuditAction::SessionStart, "Session 1")
            .unwrap();
        analytics
            .log_api_action("session-2", AuditAction::SessionStart, "Session 2")
            .unwrap();

        let events = analytics
            .get_audit_events(Some("session-1"), None, 100)
            .unwrap();
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].session_id, "session-1");
    }

    #[test]
    fn test_get_audit_events_filtered_by_action() {
        let temp = TempDir::new().unwrap();
        let analytics = Analytics::new(temp.path().to_path_buf());

        analytics
            .log_api_action("session-1", AuditAction::SessionStart, "Start")
            .unwrap();
        analytics
            .log_quality_decision("session-1", "clippy", true, None, None)
            .unwrap();
        analytics
            .log_quality_decision("session-1", "test", false, Some("Failed"), None)
            .unwrap();

        let events = analytics
            .get_audit_events(None, Some(AuditAction::QualityGateFail), 100)
            .unwrap();
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].action, AuditAction::QualityGateFail);
    }

    #[test]
    fn test_export_audit_log_json() {
        let temp = TempDir::new().unwrap();
        let analytics = Analytics::new(temp.path().to_path_buf());

        analytics
            .log_api_action("session-1", AuditAction::SessionStart, "Started")
            .unwrap();
        analytics
            .log_quality_decision("session-1", "clippy", true, None, None)
            .unwrap();

        let json = analytics.export_audit_log(AuditExportFormat::Json, None, 100).unwrap();

        // Should be valid JSON array
        let parsed: Vec<AuditEvent> = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.len(), 2);
    }

    #[test]
    fn test_export_audit_log_csv() {
        let temp = TempDir::new().unwrap();
        let analytics = Analytics::new(temp.path().to_path_buf());

        analytics
            .log_api_action("session-1", AuditAction::SessionStart, "Started")
            .unwrap();
        analytics
            .log_quality_decision("session-1", "clippy", true, None, None)
            .unwrap();

        let csv = analytics.export_audit_log(AuditExportFormat::Csv, None, 100).unwrap();

        // Should have header line and two data lines
        let lines: Vec<&str> = csv.lines().collect();
        assert_eq!(lines.len(), 3); // header + 2 events
        assert!(lines[0].contains("timestamp"));
        assert!(lines[0].contains("session_id"));
        assert!(lines[0].contains("action"));
    }

    #[test]
    fn test_export_audit_log_jsonl() {
        let temp = TempDir::new().unwrap();
        let analytics = Analytics::new(temp.path().to_path_buf());

        analytics
            .log_api_action("session-1", AuditAction::SessionStart, "Started")
            .unwrap();
        analytics
            .log_api_action("session-1", AuditAction::SessionEnd, "Ended")
            .unwrap();

        let jsonl = analytics.export_audit_log(AuditExportFormat::Jsonl, None, 100).unwrap();

        // Each line should be valid JSON
        let lines: Vec<&str> = jsonl.lines().collect();
        assert_eq!(lines.len(), 2);
        for line in lines {
            serde_json::from_str::<AuditEvent>(line).unwrap();
        }
    }

    #[test]
    fn test_audit_log_respects_limit() {
        let temp = TempDir::new().unwrap();
        let analytics = Analytics::new(temp.path().to_path_buf());

        for i in 0..10 {
            analytics
                .log_api_action("session-1", AuditAction::IterationStart, &format!("Iter {}", i))
                .unwrap();
        }

        let events = analytics.get_audit_events(None, None, 5).unwrap();
        assert_eq!(events.len(), 5);
    }

    #[test]
    fn test_audit_event_metadata() {
        let temp = TempDir::new().unwrap();
        let analytics = Analytics::new(temp.path().to_path_buf());

        let event = AuditEvent::new("session-1", AuditAction::SecurityScan)
            .with_metadata("findings_count", serde_json::json!(3))
            .with_metadata("severity", serde_json::json!("high"));

        analytics.log_audit_event(&event).unwrap();

        let events = analytics.get_audit_events(None, None, 100).unwrap();
        assert_eq!(events[0].metadata.get("findings_count"), Some(&serde_json::json!(3)));
        assert_eq!(events[0].metadata.get("severity"), Some(&serde_json::json!("high")));
    }
}
