//! Session report types and formatting.
//!
//! This module provides types for generating session summary reports
//! in various formats (JSON, Markdown).

use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

// ============================================================================
// Phase 16.2: Session Summary Report Types
// ============================================================================

/// Statistics about quality gate execution.
///
/// Tracks pass/fail counts and rates for quality gates.
///
/// # Example
///
/// ```
/// use ralph::analytics::GateStats;
///
/// let stats = GateStats {
///     total_runs: 20,
///     passed: 18,
///     failed: 2,
/// };
/// assert!((stats.pass_rate() - 0.9).abs() < 0.001);
/// ```
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct GateStats {
    /// Total number of gate runs.
    pub total_runs: usize,
    /// Number of gates that passed.
    pub passed: usize,
    /// Number of gates that failed.
    pub failed: usize,
}

impl GateStats {
    /// Calculate the pass rate as a ratio (0.0 - 1.0).
    ///
    /// Returns 0.0 if no gates have been run.
    #[must_use]
    pub fn pass_rate(&self) -> f64 {
        if self.total_runs == 0 {
            0.0
        } else {
            self.passed as f64 / self.total_runs as f64
        }
    }
}

/// Output format for session reports.
///
/// # Example
///
/// ```
/// use ralph::analytics::ReportFormat;
///
/// assert_eq!(ReportFormat::Json.extension(), "json");
/// assert_eq!(ReportFormat::Markdown.extension(), "md");
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReportFormat {
    /// JSON format.
    Json,
    /// Markdown format.
    Markdown,
}

impl ReportFormat {
    /// Get the file extension for this format.
    #[must_use]
    pub fn extension(&self) -> &'static str {
        match self {
            Self::Json => "json",
            Self::Markdown => "md",
        }
    }
}

/// A comprehensive session summary report.
///
/// Contains all metrics collected during a Ralph session, suitable for
/// display in the terminal or export to JSON/Markdown.
///
/// # Example
///
/// ```
/// use ralph::analytics::{SessionReport, GateStats};
///
/// let report = SessionReport::new("session-123")
///     .with_iterations(10)
///     .with_tasks_completed(5)
///     .with_gate_stats(GateStats {
///         total_runs: 20,
///         passed: 18,
///         failed: 2,
///     })
///     .with_predictor_accuracy(0.85);
///
/// assert_eq!(report.iterations, 10);
/// assert_eq!(report.gate_stats.pass_rate(), 0.9);
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionReport {
    /// Session identifier.
    pub session_id: String,
    /// Timestamp when the report was generated.
    pub generated_at: DateTime<Utc>,
    /// Number of iterations completed.
    pub iterations: usize,
    /// Number of tasks completed.
    pub tasks_completed: usize,
    /// Number of stagnation events.
    pub stagnations: usize,
    /// Number of error events.
    pub errors: usize,
    /// Quality gate statistics.
    pub gate_stats: GateStats,
    /// Session duration in seconds.
    pub duration_seconds: Option<u64>,
    /// Predictor accuracy (0.0 - 1.0).
    pub predictor_accuracy: Option<f64>,
    /// Mode the session was run in.
    pub mode: Option<String>,
}

impl SessionReport {
    /// Create a new session report.
    ///
    /// # Arguments
    ///
    /// * `session_id` - The session identifier
    #[must_use]
    pub fn new(session_id: impl Into<String>) -> Self {
        Self {
            session_id: session_id.into(),
            generated_at: Utc::now(),
            iterations: 0,
            tasks_completed: 0,
            stagnations: 0,
            errors: 0,
            gate_stats: GateStats::default(),
            duration_seconds: None,
            predictor_accuracy: None,
            mode: None,
        }
    }

    /// Set the iteration count.
    #[must_use]
    pub fn with_iterations(mut self, count: usize) -> Self {
        self.iterations = count;
        self
    }

    /// Set the tasks completed count.
    #[must_use]
    pub fn with_tasks_completed(mut self, count: usize) -> Self {
        self.tasks_completed = count;
        self
    }

    /// Set the stagnation count.
    #[must_use]
    pub fn with_stagnations(mut self, count: usize) -> Self {
        self.stagnations = count;
        self
    }

    /// Set the error count.
    #[must_use]
    pub fn with_errors(mut self, count: usize) -> Self {
        self.errors = count;
        self
    }

    /// Set the gate statistics.
    #[must_use]
    pub fn with_gate_stats(mut self, stats: GateStats) -> Self {
        self.gate_stats = stats;
        self
    }

    /// Set the session duration in seconds.
    #[must_use]
    pub fn with_duration_seconds(mut self, seconds: u64) -> Self {
        self.duration_seconds = Some(seconds);
        self
    }

    /// Set the predictor accuracy.
    #[must_use]
    pub fn with_predictor_accuracy(mut self, accuracy: f64) -> Self {
        self.predictor_accuracy = Some(accuracy);
        self
    }

    /// Set the session mode.
    #[must_use]
    pub fn with_mode(mut self, mode: impl Into<String>) -> Self {
        self.mode = Some(mode.into());
        self
    }

    /// Export the report as JSON.
    ///
    /// # Errors
    ///
    /// Returns an error if serialization fails.
    pub fn to_json(&self) -> Result<String> {
        serde_json::to_string_pretty(self).context("Failed to serialize report to JSON")
    }

    /// Export the report as Markdown.
    #[must_use]
    pub fn to_markdown(&self) -> String {
        let mut md = String::new();

        // Header
        md.push_str("# Session Report\n\n");

        // Summary section
        md.push_str("## Summary\n\n");
        md.push_str(&format!("**Session ID:** `{}`\n\n", self.session_id));

        if let Some(mode) = &self.mode {
            md.push_str(&format!("**Mode:** {}\n\n", mode));
        }

        md.push_str(&format!("**Iterations:** {}\n\n", self.iterations));
        md.push_str(&format!(
            "**Tasks Completed:** {}\n\n",
            self.tasks_completed
        ));
        md.push_str(&format!("**Stagnations:** {}\n\n", self.stagnations));
        md.push_str(&format!("**Errors:** {}\n\n", self.errors));

        if let Some(seconds) = self.duration_seconds {
            let minutes = seconds / 60;
            let secs = seconds % 60;
            md.push_str(&format!("**Duration:** {}m {}s\n\n", minutes, secs));
        }

        // Quality Gates section
        md.push_str("## Quality Gates\n\n");
        md.push_str(&format!(
            "**Total Runs:** {}\n\n",
            self.gate_stats.total_runs
        ));
        md.push_str(&format!("**Passed:** {}\n\n", self.gate_stats.passed));
        md.push_str(&format!("**Failed:** {}\n\n", self.gate_stats.failed));
        md.push_str(&format!(
            "**Pass Rate:** {:.1}%\n\n",
            self.gate_stats.pass_rate() * 100.0
        ));

        // Performance section
        md.push_str("## Performance\n\n");

        if let Some(accuracy) = self.predictor_accuracy {
            md.push_str(&format!(
                "**Predictor Accuracy:** {:.1}%\n\n",
                accuracy * 100.0
            ));
        } else {
            md.push_str("**Predictor Accuracy:** N/A\n\n");
        }

        // Footer
        md.push_str("---\n\n");
        md.push_str(&format!(
            "*Generated at {}*\n",
            self.generated_at.format("%Y-%m-%d %H:%M:%S UTC")
        ));

        md
    }

    /// Export the report in the specified format.
    ///
    /// # Arguments
    ///
    /// * `format` - The output format
    ///
    /// # Errors
    ///
    /// Returns an error if JSON serialization fails.
    pub fn export(&self, format: ReportFormat) -> Result<String> {
        match format {
            ReportFormat::Json => self.to_json(),
            ReportFormat::Markdown => Ok(self.to_markdown()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ========================================================================
    // GateStats Tests
    // ========================================================================

    #[test]
    fn test_gate_stats_pass_rate_zero_runs() {
        let stats = GateStats {
            total_runs: 0,
            passed: 0,
            failed: 0,
        };
        assert_eq!(stats.pass_rate(), 0.0);
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
        assert_eq!(stats.pass_rate(), 0.0);
    }

    #[test]
    fn test_gate_stats_pass_rate_mixed() {
        let stats = GateStats {
            total_runs: 20,
            passed: 18,
            failed: 2,
        };
        assert!((stats.pass_rate() - 0.9).abs() < 0.001);
    }

    // ========================================================================
    // ReportFormat Tests
    // ========================================================================

    #[test]
    fn test_report_format_enum() {
        assert_eq!(ReportFormat::Json.extension(), "json");
        assert_eq!(ReportFormat::Markdown.extension(), "md");
    }

    // ========================================================================
    // SessionReport Tests
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
            total_runs: 10,
            passed: 8,
            failed: 2,
        });

        assert_eq!(report.gate_stats.total_runs, 10);
        assert_eq!(report.gate_stats.passed, 8);
        assert!((report.gate_stats.pass_rate() - 0.8).abs() < 0.001);
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

        let json = report.export(ReportFormat::Json).unwrap();

        assert!(json.contains("\"session_id\""));
        assert!(json.contains("\"iterations\""));
        assert!(json.contains("\"gate_stats\""));
    }

    #[test]
    fn test_session_report_export_markdown() {
        let report = SessionReport::new("test-session")
            .with_iterations(10)
            .with_mode("build");

        let markdown = report.export(ReportFormat::Markdown).unwrap();

        assert!(markdown.contains("# Session Report"));
        assert!(markdown.contains("**Iterations:** 10"));
        assert!(markdown.contains("**Mode:** build"));
    }

    #[test]
    fn test_session_report_builder_chain() {
        let report = SessionReport::new("chain-test")
            .with_iterations(5)
            .with_tasks_completed(3)
            .with_stagnations(1)
            .with_errors(0)
            .with_duration_seconds(300)
            .with_predictor_accuracy(0.9)
            .with_mode("test");

        assert_eq!(report.session_id, "chain-test");
        assert_eq!(report.iterations, 5);
        assert_eq!(report.tasks_completed, 3);
        assert_eq!(report.stagnations, 1);
        assert_eq!(report.errors, 0);
        assert_eq!(report.duration_seconds, Some(300));
        assert_eq!(report.predictor_accuracy, Some(0.9));
        assert_eq!(report.mode, Some("test".to_string()));
    }

    #[test]
    fn test_session_report_serialization_roundtrip() {
        let report = SessionReport::new("roundtrip-test")
            .with_iterations(10)
            .with_gate_stats(GateStats {
                total_runs: 5,
                passed: 4,
                failed: 1,
            });

        let json = report.to_json().unwrap();
        let restored: SessionReport = serde_json::from_str(&json).unwrap();

        assert_eq!(restored.session_id, "roundtrip-test");
        assert_eq!(restored.iterations, 10);
        assert_eq!(restored.gate_stats.total_runs, 5);
    }

    #[test]
    fn test_session_report_markdown_formatting() {
        let report = SessionReport::new("md-test")
            .with_iterations(10)
            .with_gate_stats(GateStats {
                total_runs: 10,
                passed: 9,
                failed: 1,
            })
            .with_duration_seconds(125);

        let markdown = report.to_markdown();

        // Check structure
        assert!(markdown.contains("# Session Report"));
        assert!(markdown.contains("## Summary"));
        assert!(markdown.contains("## Quality Gates"));
        assert!(markdown.contains("## Performance"));

        // Check content
        assert!(markdown.contains("**Pass Rate:** 90.0%"));
        assert!(markdown.contains("**Duration:** 2m 5s"));
    }

    #[test]
    fn test_session_report_json_is_valid() {
        let report = SessionReport::new("json-test").with_iterations(5);

        let json = report.to_json().unwrap();

        // Should be valid JSON
        let value: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert!(value.is_object());
        assert_eq!(value["session_id"], "json-test");
        assert_eq!(value["iterations"], 5);
    }

    #[test]
    fn test_session_report_export_to_format() {
        let report = SessionReport::new("test-session").with_iterations(5);

        let json_output = report.export(ReportFormat::Json).unwrap();
        assert!(json_output.contains("\"session_id\""));

        let markdown_output = report.export(ReportFormat::Markdown).unwrap();
        assert!(markdown_output.contains("# Session Report"));
    }
}
