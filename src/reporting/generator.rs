//! Report generation for quality dashboards.
//!
//! This module provides functionality to generate HTML quality reports
//! from analytics data.

use crate::analytics::{
    AggregateStats, Analytics, QualityMetricsSnapshot, QualityTrend, SessionSummary,
};
use anyhow::Result;
use chrono::{DateTime, Utc};

/// Data collected for report generation.
///
/// Aggregates sessions, quality metrics, and trends into a single
/// structure for report rendering.
///
/// # Example
///
/// ```
/// use ralph::reporting::ReportData;
///
/// let data = ReportData::new("My Project")
///     .with_sessions(vec![])
///     .with_quality_snapshots(vec![]);
///
/// assert_eq!(data.project_name, "My Project");
/// ```
#[derive(Debug, Clone)]
pub struct ReportData {
    /// Name of the project.
    pub project_name: String,
    /// Timestamp when the report was generated.
    pub generated_at: DateTime<Utc>,
    /// Session summaries.
    pub sessions: Vec<SessionSummary>,
    /// Quality metric snapshots.
    pub quality_snapshots: Vec<QualityMetricsSnapshot>,
    /// Quality trend analysis.
    pub quality_trend: Option<QualityTrend>,
    /// Aggregate statistics.
    pub aggregate_stats: Option<AggregateStats>,
}

impl ReportData {
    /// Create new report data with a project name.
    #[must_use]
    pub fn new(project_name: impl Into<String>) -> Self {
        Self {
            project_name: project_name.into(),
            generated_at: Utc::now(),
            sessions: Vec::new(),
            quality_snapshots: Vec::new(),
            quality_trend: None,
            aggregate_stats: None,
        }
    }

    /// Add session summaries to the report.
    #[must_use]
    pub fn with_sessions(mut self, sessions: Vec<SessionSummary>) -> Self {
        self.sessions = sessions;
        self
    }

    /// Add quality snapshots to the report.
    #[must_use]
    pub fn with_quality_snapshots(mut self, snapshots: Vec<QualityMetricsSnapshot>) -> Self {
        self.quality_snapshots = snapshots;
        self
    }

    /// Add quality trend to the report.
    #[must_use]
    pub fn with_trend(mut self, trend: QualityTrend) -> Self {
        self.quality_trend = Some(trend);
        self
    }

    /// Add aggregate statistics to the report.
    #[must_use]
    pub fn with_aggregate_stats(mut self, stats: AggregateStats) -> Self {
        self.aggregate_stats = Some(stats);
        self
    }

    /// Calculate an overall quality score (0-100).
    ///
    /// The score is based on the most recent quality snapshot:
    /// - Starts at 100
    /// - Deducts points for clippy warnings, test failures, and security issues
    /// - Returns `None` if no quality snapshots are available
    ///
    /// # Example
    ///
    /// ```
    /// use ralph::reporting::ReportData;
    /// use ralph::analytics::QualityMetricsSnapshot;
    ///
    /// let data = ReportData::new("Test")
    ///     .with_quality_snapshots(vec![
    ///         QualityMetricsSnapshot::new("s1", 1)
    ///             .with_clippy_warnings(0)
    ///             .with_test_counts(10, 10, 0)
    ///             .with_security_issues(0),
    ///     ]);
    ///
    /// assert!(data.quality_score().unwrap() >= 90.0);
    /// ```
    #[must_use]
    pub fn quality_score(&self) -> Option<f64> {
        let snapshot = self.quality_snapshots.first()?;

        let mut score = 100.0;

        // Deduct for clippy warnings (2 points each, max 20)
        score -= (snapshot.clippy_warnings as f64 * 2.0).min(20.0);

        // Deduct for test failures (5 points each, max 40)
        score -= (snapshot.test_failed as f64 * 5.0).min(40.0);

        // Deduct for security issues (10 points each, max 30)
        score -= (snapshot.security_issues as f64 * 10.0).min(30.0);

        // Deduct for allow annotations (1 point each, max 10)
        score -= (snapshot.allow_annotations as f64).min(10.0);

        Some(score.max(0.0))
    }
}

/// Configuration for report generation.
#[derive(Debug, Clone)]
pub struct ReportConfig {
    /// Name of the project for the report header.
    pub project_name: String,
    /// Include session summaries in the report.
    pub include_sessions: bool,
    /// Include quality metrics in the report.
    pub include_quality_metrics: bool,
    /// Include trend analysis in the report.
    pub include_trend: bool,
    /// Maximum number of sessions to include.
    pub max_sessions: usize,
    /// Maximum number of quality snapshots to include.
    pub max_quality_snapshots: usize,
}

impl Default for ReportConfig {
    fn default() -> Self {
        Self {
            project_name: "Project".to_string(),
            include_sessions: true,
            include_quality_metrics: true,
            include_trend: true,
            max_sessions: 10,
            max_quality_snapshots: 50,
        }
    }
}

/// Generator for quality reports.
///
/// Creates HTML reports from analytics data.
///
/// # Example
///
/// ```rust,ignore
/// use ralph::reporting::ReportGenerator;
/// use ralph::Analytics;
///
/// let analytics = Analytics::new(project_dir);
/// let generator = ReportGenerator::new(&analytics);
/// let html = generator.generate_html()?;
/// ```
pub struct ReportGenerator<'a> {
    analytics: &'a Analytics,
    /// Configuration for report generation.
    pub config: ReportConfig,
}

impl<'a> ReportGenerator<'a> {
    /// Create a new report generator with default configuration.
    #[must_use]
    pub fn new(analytics: &'a Analytics) -> Self {
        Self {
            analytics,
            config: ReportConfig::default(),
        }
    }

    /// Create a report generator with custom configuration.
    #[must_use]
    pub fn with_config(analytics: &'a Analytics, config: ReportConfig) -> Self {
        Self { analytics, config }
    }

    /// Collect all data needed for the report.
    ///
    /// # Errors
    ///
    /// Returns an error if reading analytics data fails.
    pub fn collect_data(&self) -> Result<ReportData> {
        let mut data = ReportData::new(&self.config.project_name);

        if self.config.include_sessions {
            data.sessions = self.analytics.get_recent_sessions(self.config.max_sessions)?;
        }

        if self.config.include_quality_metrics {
            data.quality_snapshots = self
                .analytics
                .get_quality_metrics_history(None, self.config.max_quality_snapshots)?;
        }

        if self.config.include_trend {
            if let Ok(trend) = self.analytics.get_quality_trend(None, 10) {
                data.quality_trend = Some(trend);
            }
        }

        if let Ok(stats) = self.analytics.get_aggregate_stats() {
            data.aggregate_stats = Some(stats);
        }

        Ok(data)
    }

    /// Generate an HTML report.
    ///
    /// Creates a self-contained HTML document with embedded CSS
    /// for displaying quality metrics and session data.
    ///
    /// # Errors
    ///
    /// Returns an error if collecting data fails.
    pub fn generate_html(&self) -> Result<String> {
        let data = self.collect_data()?;
        Ok(render_html_report(&data))
    }
}

/// Escape HTML special characters.
fn escape_html(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#39;")
}

/// Render the HTML report from collected data.
fn render_html_report(data: &ReportData) -> String {
    let project_name = escape_html(&data.project_name);
    let generated_at = data.generated_at.format("%Y-%m-%d %H:%M:%S UTC");

    let quality_score = data
        .quality_score()
        .map(|s| format!("{:.0}", s))
        .unwrap_or_else(|| "N/A".to_string());

    let score_class = match data.quality_score() {
        Some(s) if s >= 80.0 => "score-good",
        Some(s) if s >= 50.0 => "score-warning",
        Some(_) => "score-bad",
        None => "score-none",
    };

    let sessions_html = render_sessions_section(&data.sessions);
    let metrics_html = render_metrics_section(&data.quality_snapshots);
    let trend_html = render_trend_section(&data.quality_trend);
    let stats_html = render_stats_section(&data.aggregate_stats);

    format!(
        r#"<!DOCTYPE html>
<html lang="en">
<head>
    <meta charset="UTF-8">
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
    <title>Quality Report - {project_name}</title>
    <style>
        :root {{
            --primary: #2563eb;
            --success: #16a34a;
            --warning: #d97706;
            --danger: #dc2626;
            --bg: #f8fafc;
            --card-bg: #ffffff;
            --text: #1e293b;
            --text-muted: #64748b;
            --border: #e2e8f0;
        }}
        * {{
            box-sizing: border-box;
            margin: 0;
            padding: 0;
        }}
        body {{
            font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, Oxygen, Ubuntu, sans-serif;
            background: var(--bg);
            color: var(--text);
            line-height: 1.6;
            padding: 2rem;
        }}
        .container {{
            max-width: 1200px;
            margin: 0 auto;
        }}
        header {{
            text-align: center;
            margin-bottom: 2rem;
            padding-bottom: 1rem;
            border-bottom: 2px solid var(--border);
        }}
        h1 {{
            font-size: 2rem;
            margin-bottom: 0.5rem;
        }}
        .subtitle {{
            color: var(--text-muted);
            font-size: 0.875rem;
        }}
        .score-card {{
            display: flex;
            justify-content: center;
            margin: 1.5rem 0;
        }}
        .score {{
            font-size: 3rem;
            font-weight: bold;
            padding: 1rem 2rem;
            border-radius: 0.5rem;
        }}
        .score-good {{ background: #dcfce7; color: var(--success); }}
        .score-warning {{ background: #fef3c7; color: var(--warning); }}
        .score-bad {{ background: #fee2e2; color: var(--danger); }}
        .score-none {{ background: var(--border); color: var(--text-muted); }}
        section {{
            background: var(--card-bg);
            border-radius: 0.5rem;
            padding: 1.5rem;
            margin-bottom: 1.5rem;
            box-shadow: 0 1px 3px rgba(0,0,0,0.1);
        }}
        h2 {{
            font-size: 1.25rem;
            margin-bottom: 1rem;
            padding-bottom: 0.5rem;
            border-bottom: 1px solid var(--border);
        }}
        table {{
            width: 100%;
            border-collapse: collapse;
        }}
        th, td {{
            padding: 0.75rem;
            text-align: left;
            border-bottom: 1px solid var(--border);
        }}
        th {{
            background: var(--bg);
            font-weight: 600;
        }}
        .metric {{
            display: inline-block;
            padding: 0.25rem 0.75rem;
            border-radius: 0.25rem;
            margin-right: 0.5rem;
            font-size: 0.875rem;
        }}
        .metric-good {{ background: #dcfce7; color: var(--success); }}
        .metric-bad {{ background: #fee2e2; color: var(--danger); }}
        .empty-message {{
            color: var(--text-muted);
            font-style: italic;
            text-align: center;
            padding: 2rem;
        }}
        .trend-indicator {{
            display: inline-flex;
            align-items: center;
            gap: 0.5rem;
        }}
        .trend-improving {{ color: var(--success); }}
        .trend-degrading {{ color: var(--danger); }}
        .trend-stable {{ color: var(--text-muted); }}
        footer {{
            text-align: center;
            color: var(--text-muted);
            font-size: 0.75rem;
            margin-top: 2rem;
            padding-top: 1rem;
            border-top: 1px solid var(--border);
        }}
    </style>
</head>
<body>
    <div class="container">
        <header>
            <h1>Quality Report</h1>
            <p class="subtitle">{project_name} - Generated {generated_at}</p>
            <div class="score-card">
                <div class="score {score_class}">{quality_score}</div>
            </div>
        </header>

        {stats_html}
        {trend_html}
        {metrics_html}
        {sessions_html}

        <footer>
            Generated by Ralph - Claude Code Automation Suite
        </footer>
    </div>
</body>
</html>"#
    )
}

fn render_sessions_section(sessions: &[SessionSummary]) -> String {
    if sessions.is_empty() {
        return r#"<section>
            <h2>Sessions</h2>
            <p class="empty-message">No session data available.</p>
        </section>"#
            .to_string();
    }

    let rows: String = sessions
        .iter()
        .map(|s| {
            let started = s
                .started_at
                .map(|t| t.format("%Y-%m-%d %H:%M").to_string())
                .unwrap_or_else(|| "-".to_string());
            let duration = s
                .duration_minutes
                .map(|d| format!("{}m", d))
                .unwrap_or_else(|| "-".to_string());
            let mode = s.mode.as_deref().unwrap_or("-");

            format!(
                r#"<tr>
                    <td>{}</td>
                    <td>{}</td>
                    <td>{}</td>
                    <td>{}</td>
                    <td>{}</td>
                    <td>{}</td>
                </tr>"#,
                escape_html(&s.session_id[..8.min(s.session_id.len())]),
                escape_html(&started),
                escape_html(mode),
                s.iterations,
                s.stagnations,
                escape_html(&duration)
            )
        })
        .collect();

    format!(
        r#"<section>
            <h2>Sessions</h2>
            <table>
                <thead>
                    <tr>
                        <th>ID</th>
                        <th>Started</th>
                        <th>Mode</th>
                        <th>Iterations</th>
                        <th>Stagnations</th>
                        <th>Duration</th>
                    </tr>
                </thead>
                <tbody>
                    {rows}
                </tbody>
            </table>
        </section>"#
    )
}

fn render_metrics_section(snapshots: &[QualityMetricsSnapshot]) -> String {
    if snapshots.is_empty() {
        return r#"<section>
            <h2>Quality Metrics</h2>
            <p class="empty-message">No quality metrics available.</p>
        </section>"#
            .to_string();
    }

    let rows: String = snapshots
        .iter()
        .take(10) // Show latest 10
        .map(|s| {
            let timestamp = s.timestamp.format("%Y-%m-%d %H:%M").to_string();
            let clippy_class = if s.clippy_warnings == 0 {
                "metric-good"
            } else {
                "metric-bad"
            };
            let test_class = if s.test_failed == 0 {
                "metric-good"
            } else {
                "metric-bad"
            };
            let security_class = if s.security_issues == 0 {
                "metric-good"
            } else {
                "metric-bad"
            };

            format!(
                r#"<tr>
                    <td>{}</td>
                    <td>{}</td>
                    <td><span class="metric {}">{}</span></td>
                    <td><span class="metric {}">{}/{}</span></td>
                    <td><span class="metric {}">{}</span></td>
                </tr>"#,
                escape_html(&timestamp),
                s.iteration,
                clippy_class,
                s.clippy_warnings,
                test_class,
                s.test_passed,
                s.test_total,
                security_class,
                s.security_issues
            )
        })
        .collect();

    format!(
        r#"<section>
            <h2>Quality Metrics</h2>
            <table>
                <thead>
                    <tr>
                        <th>Timestamp</th>
                        <th>Iteration</th>
                        <th>Clippy</th>
                        <th>Tests</th>
                        <th>Security</th>
                    </tr>
                </thead>
                <tbody>
                    {rows}
                </tbody>
            </table>
        </section>"#
    )
}

fn render_trend_section(trend: &Option<QualityTrend>) -> String {
    let Some(t) = trend else {
        return String::new();
    };

    let (trend_class, trend_text) = match t.overall {
        crate::analytics::TrendDirection::Improving => ("trend-improving", "Improving"),
        crate::analytics::TrendDirection::Degrading => ("trend-degrading", "Degrading"),
        crate::analytics::TrendDirection::Stable => ("trend-stable", "Stable"),
    };

    let pass_rate = t
        .avg_test_pass_rate
        .map(|r| format!("{:.1}%", r * 100.0))
        .unwrap_or_else(|| "-".to_string());

    format!(
        r#"<section>
            <h2>Quality Trend</h2>
            <p class="trend-indicator {trend_class}">
                <strong>Overall:</strong> {trend_text}
            </p>
            <table>
                <tr>
                    <th>Metric</th>
                    <th>Average</th>
                    <th>Change</th>
                </tr>
                <tr>
                    <td>Clippy Warnings</td>
                    <td>{:.1}</td>
                    <td>{:+}</td>
                </tr>
                <tr>
                    <td>Test Pass Rate</td>
                    <td>{}</td>
                    <td>{:+} failures</td>
                </tr>
                <tr>
                    <td>Security Issues</td>
                    <td>{:.1}</td>
                    <td>{:+}</td>
                </tr>
            </table>
        </section>"#,
        t.avg_clippy_warnings,
        t.clippy_delta,
        pass_rate,
        t.test_failures_delta,
        t.avg_security_issues,
        t.security_delta
    )
}

fn render_stats_section(stats: &Option<AggregateStats>) -> String {
    let Some(s) = stats else {
        return String::new();
    };

    format!(
        r#"<section>
            <h2>Aggregate Statistics</h2>
            <table>
                <tr>
                    <td>Total Sessions</td>
                    <td><strong>{}</strong></td>
                </tr>
                <tr>
                    <td>Total Iterations</td>
                    <td><strong>{}</strong></td>
                </tr>
                <tr>
                    <td>Total Errors</td>
                    <td><strong>{}</strong></td>
                </tr>
                <tr>
                    <td>Total Stagnations</td>
                    <td><strong>{}</strong></td>
                </tr>
            </table>
        </section>"#,
        s.total_sessions, s.total_iterations, s.total_errors, s.total_stagnations
    )
}
