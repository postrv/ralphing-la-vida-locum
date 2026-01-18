//! Export functionality for quality metrics and session data.
//!
//! Provides CSV, JSON, and JSONL export formats for quality data.

use crate::analytics::{QualityMetricsSnapshot, SessionSummary};
use anyhow::{Context, Result};
use std::str::FromStr;

/// Export format for quality data.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExportFormat {
    /// JSON array format.
    Json,
    /// JSON Lines format (one JSON object per line).
    Jsonl,
    /// CSV format with headers.
    Csv,
}

impl ExportFormat {
    /// Get the file extension for this format.
    #[must_use]
    pub fn extension(&self) -> &'static str {
        match self {
            ExportFormat::Json => "json",
            ExportFormat::Jsonl => "jsonl",
            ExportFormat::Csv => "csv",
        }
    }
}

impl FromStr for ExportFormat {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "json" => Ok(ExportFormat::Json),
            "jsonl" | "ndjson" => Ok(ExportFormat::Jsonl),
            "csv" => Ok(ExportFormat::Csv),
            _ => Err(anyhow::anyhow!(
                "Invalid export format: {}. Valid formats: json, jsonl, csv",
                s
            )),
        }
    }
}

/// Exporter for quality metrics and session data.
///
/// # Example
///
/// ```rust,ignore
/// use ralph::reporting::{QualityExporter, ExportFormat};
/// use ralph::analytics::QualityMetricsSnapshot;
///
/// let snapshots = vec![QualityMetricsSnapshot::new("s1", 1)];
/// let json = QualityExporter::export(&snapshots, ExportFormat::Json)?;
/// ```
pub struct QualityExporter;

impl QualityExporter {
    /// Export quality metrics snapshots to the specified format.
    ///
    /// # Arguments
    ///
    /// * `snapshots` - Quality metrics snapshots to export
    /// * `format` - Output format (JSON, JSONL, or CSV)
    ///
    /// # Errors
    ///
    /// Returns an error if serialization fails.
    pub fn export(snapshots: &[QualityMetricsSnapshot], format: ExportFormat) -> Result<String> {
        match format {
            ExportFormat::Json => Self::export_json(snapshots),
            ExportFormat::Jsonl => Self::export_jsonl(snapshots),
            ExportFormat::Csv => Self::export_csv(snapshots),
        }
    }

    /// Export session summaries to the specified format.
    ///
    /// # Arguments
    ///
    /// * `sessions` - Session summaries to export
    /// * `format` - Output format (JSON, JSONL, or CSV)
    ///
    /// # Errors
    ///
    /// Returns an error if serialization fails.
    pub fn export_sessions(sessions: &[SessionSummary], format: ExportFormat) -> Result<String> {
        match format {
            ExportFormat::Json => Self::export_sessions_json(sessions),
            ExportFormat::Jsonl => Self::export_sessions_jsonl(sessions),
            ExportFormat::Csv => Self::export_sessions_csv(sessions),
        }
    }

    fn export_json(snapshots: &[QualityMetricsSnapshot]) -> Result<String> {
        serde_json::to_string_pretty(snapshots).context("Failed to serialize to JSON")
    }

    fn export_jsonl(snapshots: &[QualityMetricsSnapshot]) -> Result<String> {
        let lines: Result<Vec<String>, _> = snapshots.iter().map(serde_json::to_string).collect();
        Ok(lines?.join("\n"))
    }

    fn export_csv(snapshots: &[QualityMetricsSnapshot]) -> Result<String> {
        let mut csv = String::new();

        // Header
        csv.push_str(
            "timestamp,session_id,iteration,clippy_warnings,test_total,test_passed,test_failed,security_issues,allow_annotations,task_name\n",
        );

        // Data rows
        for snapshot in snapshots {
            let timestamp = snapshot.timestamp.format("%Y-%m-%dT%H:%M:%S%.3fZ");
            let task_name = snapshot
                .task_name
                .as_ref()
                .map(|s| escape_csv(s))
                .unwrap_or_default();

            let row = format!(
                "{},{},{},{},{},{},{},{},{},{}\n",
                timestamp,
                escape_csv(&snapshot.session_id),
                snapshot.iteration,
                snapshot.clippy_warnings,
                snapshot.test_total,
                snapshot.test_passed,
                snapshot.test_failed,
                snapshot.security_issues,
                snapshot.allow_annotations,
                task_name
            );
            csv.push_str(&row);
        }

        Ok(csv)
    }

    fn export_sessions_json(sessions: &[SessionSummary]) -> Result<String> {
        serde_json::to_string_pretty(sessions).context("Failed to serialize sessions to JSON")
    }

    fn export_sessions_jsonl(sessions: &[SessionSummary]) -> Result<String> {
        let lines: Result<Vec<String>, _> = sessions.iter().map(serde_json::to_string).collect();
        Ok(lines?.join("\n"))
    }

    fn export_sessions_csv(sessions: &[SessionSummary]) -> Result<String> {
        let mut csv = String::new();

        // Header
        csv.push_str(
            "session_id,started_at,ended_at,mode,iterations,stagnations,errors,docs_drift_events,duration_minutes\n",
        );

        // Data rows
        for session in sessions {
            let started_at = session
                .started_at
                .map(|t| t.format("%Y-%m-%dT%H:%M:%S%.3fZ").to_string())
                .unwrap_or_default();
            let ended_at = session
                .ended_at
                .map(|t| t.format("%Y-%m-%dT%H:%M:%S%.3fZ").to_string())
                .unwrap_or_default();
            let mode = session
                .mode
                .as_ref()
                .map(|s| escape_csv(s))
                .unwrap_or_default();
            let duration = session
                .duration_minutes
                .map(|d| d.to_string())
                .unwrap_or_default();

            let row = format!(
                "{},{},{},{},{},{},{},{},{}\n",
                escape_csv(&session.session_id),
                started_at,
                ended_at,
                mode,
                session.iterations,
                session.stagnations,
                session.errors,
                session.docs_drift_events,
                duration
            );
            csv.push_str(&row);
        }

        Ok(csv)
    }
}

/// Escape a string for CSV output.
fn escape_csv(s: &str) -> String {
    if s.contains(',') || s.contains('"') || s.contains('\n') || s.contains('\r') {
        format!("\"{}\"", s.replace('"', "\"\""))
    } else {
        s.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_escape_csv_simple() {
        assert_eq!(escape_csv("hello"), "hello");
    }

    #[test]
    fn test_escape_csv_with_comma() {
        assert_eq!(escape_csv("hello,world"), "\"hello,world\"");
    }

    #[test]
    fn test_escape_csv_with_quotes() {
        assert_eq!(escape_csv("say \"hello\""), "\"say \"\"hello\"\"\"");
    }

    #[test]
    fn test_escape_csv_with_newline() {
        assert_eq!(escape_csv("line1\nline2"), "\"line1\nline2\"");
    }
}
