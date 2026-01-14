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
}
