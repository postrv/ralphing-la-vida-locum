//! Quality reporting and export functionality.
//!
//! This module provides comprehensive reporting capabilities for Ralph,
//! including HTML dashboards, CSV/JSON exports, and aggregated quality reports.
//!
//! # Features
//!
//! - **HTML Reports**: Self-contained HTML quality dashboards
//! - **Data Export**: CSV and JSON export for quality metrics
//! - **Report Aggregation**: Combine sessions, quality metrics, and trends
//!
//! # Example
//!
//! ```rust,ignore
//! use ralph::reporting::{ReportGenerator, ReportConfig, ExportFormat};
//! use ralph::Analytics;
//!
//! let analytics = Analytics::new(project_dir);
//! let generator = ReportGenerator::new(&analytics);
//!
//! // Generate HTML report
//! let html = generator.generate_html()?;
//!
//! // Export quality metrics as JSON
//! let json = generator.export_quality_metrics(ExportFormat::Json)?;
//! ```

mod certification;
mod export;
mod generator;

pub use certification::{
    CertificationHistory, CertificationLevel, CertificationMetrics, QualityCertification,
    QualityCertifier,
};
pub use export::{ExportFormat, QualityExporter};
pub use generator::{ReportConfig, ReportData, ReportGenerator};

#[cfg(test)]
mod tests {
    use super::*;
    use crate::analytics::{
        Analytics, QualityMetricsSnapshot, SessionSummary, AggregateStats, QualityTrend,
        TrendDirection,
    };
    use tempfile::TempDir;

    fn create_test_analytics() -> (TempDir, Analytics) {
        let temp_dir = TempDir::new().unwrap();
        let analytics = Analytics::new(temp_dir.path().to_path_buf());
        (temp_dir, analytics)
    }

    // ========================================================================
    // ReportData Tests
    // ========================================================================

    #[test]
    fn test_report_data_new_creates_empty_report() {
        let data = ReportData::new("Test Project");

        assert_eq!(data.project_name, "Test Project");
        assert!(data.sessions.is_empty());
        assert!(data.quality_snapshots.is_empty());
        assert!(data.quality_trend.is_none());
    }

    #[test]
    fn test_report_data_with_sessions() {
        let sessions = vec![
            SessionSummary {
                session_id: "session-1".to_string(),
                started_at: None,
                ended_at: None,
                mode: Some("build".to_string()),
                iterations: 5,
                stagnations: 1,
                errors: 0,
                docs_drift_events: 0,
                duration_minutes: Some(30),
            },
        ];

        let data = ReportData::new("Test Project")
            .with_sessions(sessions.clone());

        assert_eq!(data.sessions.len(), 1);
        assert_eq!(data.sessions[0].session_id, "session-1");
    }

    #[test]
    fn test_report_data_with_quality_snapshots() {
        let snapshots = vec![
            QualityMetricsSnapshot::new("session-1", 1)
                .with_clippy_warnings(0)
                .with_test_counts(42, 42, 0),
        ];

        let data = ReportData::new("Test Project")
            .with_quality_snapshots(snapshots);

        assert_eq!(data.quality_snapshots.len(), 1);
        assert!(data.quality_snapshots[0].all_gates_passing());
    }

    #[test]
    fn test_report_data_with_trend() {
        let trend = QualityTrend {
            overall: TrendDirection::Improving,
            snapshot_count: 5,
            avg_clippy_warnings: 0.5,
            avg_test_pass_rate: Some(0.95),
            avg_security_issues: 0.0,
            clippy_delta: -2,
            test_failures_delta: -1,
            security_delta: 0,
        };

        let data = ReportData::new("Test Project")
            .with_trend(trend);

        assert!(data.quality_trend.is_some());
        assert_eq!(data.quality_trend.unwrap().overall, TrendDirection::Improving);
    }

    #[test]
    fn test_report_data_with_aggregate_stats() {
        let stats = AggregateStats {
            total_sessions: 10,
            total_iterations: 100,
            total_errors: 5,
            total_stagnations: 8,
            total_drift_events: 2,
        };

        let data = ReportData::new("Test Project")
            .with_aggregate_stats(stats);

        assert!(data.aggregate_stats.is_some());
        assert_eq!(data.aggregate_stats.unwrap().total_sessions, 10);
    }

    #[test]
    fn test_report_data_quality_score() {
        // Good quality - all gates passing
        let data = ReportData::new("Test")
            .with_quality_snapshots(vec![
                QualityMetricsSnapshot::new("s1", 1)
                    .with_clippy_warnings(0)
                    .with_test_counts(10, 10, 0)
                    .with_security_issues(0),
            ]);

        let score = data.quality_score();
        assert!(score.is_some());
        assert!(score.unwrap() >= 90.0); // High score for passing

        // Poor quality - many issues
        let data = ReportData::new("Test")
            .with_quality_snapshots(vec![
                QualityMetricsSnapshot::new("s1", 1)
                    .with_clippy_warnings(10)
                    .with_test_counts(10, 5, 5)
                    .with_security_issues(2),
            ]);

        let score = data.quality_score();
        assert!(score.is_some());
        assert!(score.unwrap() < 50.0); // Low score for issues
    }

    // ========================================================================
    // ReportGenerator Tests
    // ========================================================================

    #[test]
    fn test_report_generator_new() {
        let (_temp, analytics) = create_test_analytics();
        let generator = ReportGenerator::new(&analytics);

        assert!(generator.config.include_sessions);
        assert!(generator.config.include_quality_metrics);
    }

    #[test]
    fn test_report_generator_with_config() {
        let (_temp, analytics) = create_test_analytics();
        let config = ReportConfig {
            project_name: "Custom Project".to_string(),
            include_sessions: false,
            include_quality_metrics: true,
            include_trend: true,
            max_sessions: 5,
            max_quality_snapshots: 20,
        };

        let generator = ReportGenerator::with_config(&analytics, config);

        assert_eq!(generator.config.project_name, "Custom Project");
        assert!(!generator.config.include_sessions);
    }

    #[test]
    fn test_report_generator_collect_data_empty() {
        let (_temp, analytics) = create_test_analytics();
        let generator = ReportGenerator::new(&analytics);

        let data = generator.collect_data().unwrap();

        assert!(data.sessions.is_empty());
        assert!(data.quality_snapshots.is_empty());
    }

    #[test]
    fn test_report_generator_generate_html_structure() {
        let (_temp, analytics) = create_test_analytics();
        let generator = ReportGenerator::new(&analytics);

        let html = generator.generate_html().unwrap();

        // Should contain basic HTML structure
        assert!(html.contains("<!DOCTYPE html>"));
        assert!(html.contains("<html"));
        assert!(html.contains("</html>"));
        assert!(html.contains("<head>"));
        assert!(html.contains("<body>"));

        // Should contain report title
        assert!(html.contains("Quality Report"));

        // Should be self-contained (CSS included)
        assert!(html.contains("<style>"));
    }

    #[test]
    fn test_report_generator_html_includes_metrics() {
        let (_temp, analytics) = create_test_analytics();

        // Log some quality metrics
        let snapshot = QualityMetricsSnapshot::new("test-session", 1)
            .with_clippy_warnings(2)
            .with_test_counts(50, 48, 2)
            .with_security_issues(0);
        analytics.log_quality_metrics(&snapshot).unwrap();

        let generator = ReportGenerator::new(&analytics);
        let html = generator.generate_html().unwrap();

        // Should include metrics in report
        assert!(html.contains("Clippy"));
        assert!(html.contains("Test"));
    }

    #[test]
    fn test_report_generator_html_escapes_content() {
        let (_temp, analytics) = create_test_analytics();
        let config = ReportConfig {
            project_name: "<script>alert('xss')</script>".to_string(),
            ..Default::default()
        };

        let generator = ReportGenerator::with_config(&analytics, config);
        let html = generator.generate_html().unwrap();

        // Should escape HTML in project name
        assert!(!html.contains("<script>alert"));
        assert!(html.contains("&lt;script&gt;") || html.contains("script")); // Escaped or removed
    }

    // ========================================================================
    // ExportFormat Tests
    // ========================================================================

    #[test]
    fn test_export_format_from_str() {
        assert_eq!("json".parse::<ExportFormat>().unwrap(), ExportFormat::Json);
        assert_eq!("csv".parse::<ExportFormat>().unwrap(), ExportFormat::Csv);
        assert_eq!("jsonl".parse::<ExportFormat>().unwrap(), ExportFormat::Jsonl);

        assert!("invalid".parse::<ExportFormat>().is_err());
    }

    #[test]
    fn test_export_format_extension() {
        assert_eq!(ExportFormat::Json.extension(), "json");
        assert_eq!(ExportFormat::Csv.extension(), "csv");
        assert_eq!(ExportFormat::Jsonl.extension(), "jsonl");
    }

    // ========================================================================
    // QualityExporter Tests
    // ========================================================================

    #[test]
    fn test_quality_exporter_export_json() {
        let snapshots = vec![
            QualityMetricsSnapshot::new("s1", 1)
                .with_clippy_warnings(0)
                .with_test_counts(10, 10, 0),
            QualityMetricsSnapshot::new("s1", 2)
                .with_clippy_warnings(1)
                .with_test_counts(10, 9, 1),
        ];

        let json = QualityExporter::export(&snapshots, ExportFormat::Json).unwrap();

        // Should be valid JSON array
        let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert!(parsed.is_array());
        assert_eq!(parsed.as_array().unwrap().len(), 2);
    }

    #[test]
    fn test_quality_exporter_export_jsonl() {
        let snapshots = vec![
            QualityMetricsSnapshot::new("s1", 1),
            QualityMetricsSnapshot::new("s1", 2),
        ];

        let jsonl = QualityExporter::export(&snapshots, ExportFormat::Jsonl).unwrap();

        // Should have two lines
        let lines: Vec<&str> = jsonl.lines().collect();
        assert_eq!(lines.len(), 2);

        // Each line should be valid JSON
        for line in lines {
            assert!(serde_json::from_str::<serde_json::Value>(line).is_ok());
        }
    }

    #[test]
    fn test_quality_exporter_export_csv() {
        let snapshots = vec![
            QualityMetricsSnapshot::new("session-1", 1)
                .with_clippy_warnings(2)
                .with_test_counts(50, 48, 2)
                .with_security_issues(1),
        ];

        let csv = QualityExporter::export(&snapshots, ExportFormat::Csv).unwrap();

        // Should have header row
        let lines: Vec<&str> = csv.lines().collect();
        assert!(lines.len() >= 2);

        // Header should contain expected columns
        let header = lines[0];
        assert!(header.contains("session_id"));
        assert!(header.contains("iteration"));
        assert!(header.contains("clippy_warnings"));
        assert!(header.contains("test_passed"));
        assert!(header.contains("test_failed"));
        assert!(header.contains("security_issues"));
    }

    #[test]
    fn test_quality_exporter_csv_escapes_values() {
        let mut snapshot = QualityMetricsSnapshot::new("session,with,commas", 1);
        snapshot.task_name = Some("task \"with\" quotes".to_string());

        let csv = QualityExporter::export(&[snapshot], ExportFormat::Csv).unwrap();

        // Should properly escape the values
        assert!(csv.contains("\"session,with,commas\"") || csv.contains("session_with_commas"));
    }

    #[test]
    fn test_quality_exporter_export_empty() {
        let snapshots: Vec<QualityMetricsSnapshot> = vec![];

        let json = QualityExporter::export(&snapshots, ExportFormat::Json).unwrap();
        assert_eq!(json.trim(), "[]");

        let jsonl = QualityExporter::export(&snapshots, ExportFormat::Jsonl).unwrap();
        assert!(jsonl.is_empty());

        let csv = QualityExporter::export(&snapshots, ExportFormat::Csv).unwrap();
        // Should still have header
        assert!(csv.contains("session_id"));
    }

    #[test]
    fn test_quality_exporter_export_sessions_json() {
        let sessions = vec![
            SessionSummary {
                session_id: "s1".to_string(),
                started_at: None,
                ended_at: None,
                mode: Some("build".to_string()),
                iterations: 5,
                stagnations: 0,
                errors: 0,
                docs_drift_events: 0,
                duration_minutes: Some(10),
            },
        ];

        let json = QualityExporter::export_sessions(&sessions, ExportFormat::Json).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();

        assert!(parsed.is_array());
        assert_eq!(parsed[0]["session_id"], "s1");
    }

    #[test]
    fn test_quality_exporter_export_sessions_csv() {
        let sessions = vec![
            SessionSummary {
                session_id: "s1".to_string(),
                started_at: None,
                ended_at: None,
                mode: Some("build".to_string()),
                iterations: 5,
                stagnations: 1,
                errors: 0,
                docs_drift_events: 0,
                duration_minutes: Some(10),
            },
        ];

        let csv = QualityExporter::export_sessions(&sessions, ExportFormat::Csv).unwrap();

        assert!(csv.contains("session_id"));
        assert!(csv.contains("mode"));
        assert!(csv.contains("iterations"));
        assert!(csv.contains("s1"));
    }
}
