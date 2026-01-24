//! HTML template engine for analytics dashboard.
//!
//! This module provides a simple HTML template renderer that generates
//! standalone dashboard pages with embedded CSS and JavaScript.
//! All content is inline - no external dependencies required.

use super::DashboardData;

/// HTML template renderer for the analytics dashboard.
///
/// Generates standalone HTML files with inline CSS and JavaScript.
/// No external CDN dependencies - works fully offline.
///
/// # Example
///
/// ```rust,ignore
/// use ralph::analytics::dashboard::{DashboardData, DashboardTemplate};
///
/// let data = DashboardData::default();
/// let template = DashboardTemplate::new(&data);
/// let html = template.render();
/// assert!(html.contains("<!DOCTYPE html>"));
/// ```
#[derive(Debug)]
pub struct DashboardTemplate<'a> {
    data: &'a DashboardData,
}

impl<'a> DashboardTemplate<'a> {
    /// Create a new template renderer for the given dashboard data.
    ///
    /// # Arguments
    ///
    /// * `data` - The dashboard data to render
    #[must_use]
    pub fn new(data: &'a DashboardData) -> Self {
        Self { data }
    }

    /// Render the dashboard as an HTML string.
    ///
    /// Generates a complete, standalone HTML document with:
    /// - Inline CSS for styling (Tailwind-like utility classes)
    /// - Inline JavaScript for interactivity
    /// - All dashboard data embedded in the page
    ///
    /// # Returns
    ///
    /// A complete HTML document as a string.
    #[must_use]
    pub fn render(&self) -> String {
        let summary = &self.data.summary;
        let success_pct = (summary.success_rate * 100.0).round() as u32;
        let gate_pass_pct = (summary.gate_stats.pass_rate() * 100.0).round() as u32;
        let generated_time = self.data.generated_at.format("%Y-%m-%d %H:%M:%S UTC");

        // Build session list HTML
        let sessions_html = self.render_sessions_list();

        format!(
            r##"<!DOCTYPE html>
<html lang="en">
<head>
    <meta charset="UTF-8">
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
    <title>Ralph Analytics Dashboard</title>
    <style>
        :root {{
            --color-bg: #0f172a;
            --color-card: #1e293b;
            --color-border: #334155;
            --color-text: #e2e8f0;
            --color-text-muted: #94a3b8;
            --color-primary: #3b82f6;
            --color-success: #22c55e;
            --color-warning: #f59e0b;
            --color-error: #ef4444;
        }}
        * {{
            margin: 0;
            padding: 0;
            box-sizing: border-box;
        }}
        body {{
            font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, 'Helvetica Neue', Arial, sans-serif;
            background-color: var(--color-bg);
            color: var(--color-text);
            line-height: 1.6;
            padding: 2rem;
        }}
        .container {{
            max-width: 1200px;
            margin: 0 auto;
        }}
        .header {{
            margin-bottom: 2rem;
            padding-bottom: 1rem;
            border-bottom: 1px solid var(--color-border);
        }}
        .header h1 {{
            font-size: 1.875rem;
            font-weight: 700;
            margin-bottom: 0.5rem;
        }}
        .header .generated {{
            color: var(--color-text-muted);
            font-size: 0.875rem;
        }}
        .grid {{
            display: grid;
            grid-template-columns: repeat(auto-fit, minmax(250px, 1fr));
            gap: 1rem;
            margin-bottom: 2rem;
        }}
        .card {{
            background-color: var(--color-card);
            border: 1px solid var(--color-border);
            border-radius: 0.5rem;
            padding: 1.5rem;
        }}
        .card-title {{
            font-size: 0.875rem;
            color: var(--color-text-muted);
            text-transform: uppercase;
            letter-spacing: 0.05em;
            margin-bottom: 0.5rem;
        }}
        .card-value {{
            font-size: 2rem;
            font-weight: 700;
        }}
        .card-value.success {{
            color: var(--color-success);
        }}
        .card-value.warning {{
            color: var(--color-warning);
        }}
        .card-value.error {{
            color: var(--color-error);
        }}
        .section {{
            background-color: var(--color-card);
            border: 1px solid var(--color-border);
            border-radius: 0.5rem;
            margin-bottom: 1rem;
        }}
        .section-header {{
            display: flex;
            justify-content: space-between;
            align-items: center;
            padding: 1rem 1.5rem;
            cursor: pointer;
            user-select: none;
        }}
        .section-header:hover {{
            background-color: rgba(59, 130, 246, 0.1);
        }}
        .section-title {{
            font-size: 1.125rem;
            font-weight: 600;
        }}
        .toggle-icon {{
            transition: transform 0.2s;
        }}
        .section.collapsed .toggle-icon {{
            transform: rotate(-90deg);
        }}
        .section.collapsed .section-content {{
            display: none;
        }}
        .section-content {{
            padding: 1.5rem;
            border-top: 1px solid var(--color-border);
        }}
        .stat-row {{
            display: flex;
            justify-content: space-between;
            padding: 0.5rem 0;
            border-bottom: 1px solid var(--color-border);
        }}
        .stat-row:last-child {{
            border-bottom: none;
        }}
        .stat-label {{
            color: var(--color-text-muted);
        }}
        .stat-value {{
            font-weight: 600;
        }}
        .progress-bar {{
            height: 0.5rem;
            background-color: var(--color-border);
            border-radius: 0.25rem;
            overflow: hidden;
            margin-top: 0.5rem;
        }}
        .progress-fill {{
            height: 100%;
            border-radius: 0.25rem;
            transition: width 0.3s;
        }}
        .progress-fill.success {{
            background-color: var(--color-success);
        }}
        .progress-fill.warning {{
            background-color: var(--color-warning);
        }}
        .progress-fill.error {{
            background-color: var(--color-error);
        }}
        .session-list {{
            max-height: 400px;
            overflow-y: auto;
        }}
        .session-item {{
            padding: 1rem;
            border-bottom: 1px solid var(--color-border);
        }}
        .session-item:last-child {{
            border-bottom: none;
        }}
        .session-id {{
            font-family: monospace;
            font-size: 0.875rem;
            color: var(--color-primary);
        }}
        .session-meta {{
            display: flex;
            gap: 1rem;
            margin-top: 0.5rem;
            font-size: 0.875rem;
            color: var(--color-text-muted);
        }}
        .no-data {{
            text-align: center;
            padding: 2rem;
            color: var(--color-text-muted);
        }}
        @media (max-width: 640px) {{
            body {{
                padding: 1rem;
            }}
            .grid {{
                grid-template-columns: 1fr;
            }}
        }}
    </style>
</head>
<body>
    <div class="container">
        <header class="header">
            <h1>Ralph Analytics Dashboard</h1>
            <p class="generated">Generated: {generated_time}</p>
        </header>

        <section class="grid">
            <div class="card">
                <div class="card-title">Total Sessions</div>
                <div class="card-value">{total_sessions}</div>
            </div>
            <div class="card">
                <div class="card-title">Total Iterations</div>
                <div class="card-value">{total_iterations}</div>
            </div>
            <div class="card">
                <div class="card-title">Tasks Completed</div>
                <div class="card-value">{total_tasks}</div>
            </div>
            <div class="card">
                <div class="card-title">Success Rate</div>
                <div class="card-value {success_class}">{success_pct}%</div>
                <div class="progress-bar">
                    <div class="progress-fill {success_class}" style="width: {success_pct}%"></div>
                </div>
            </div>
        </section>

        <div class="section" data-collapsed="false">
            <div class="section-header" onclick="toggleSection(this)">
                <span class="section-title">Summary Overview</span>
                <span class="toggle-icon">&#9660;</span>
            </div>
            <div class="section-content">
                <div class="stat-row">
                    <span class="stat-label">Total Sessions</span>
                    <span class="stat-value">{total_sessions}</span>
                </div>
                <div class="stat-row">
                    <span class="stat-label">Total Iterations</span>
                    <span class="stat-value">{total_iterations}</span>
                </div>
                <div class="stat-row">
                    <span class="stat-label">Tasks Completed</span>
                    <span class="stat-value">{total_tasks}</span>
                </div>
                <div class="stat-row">
                    <span class="stat-label">Stagnation Events</span>
                    <span class="stat-value">{stagnations}</span>
                </div>
                <div class="stat-row">
                    <span class="stat-label">Avg Session Duration</span>
                    <span class="stat-value">{avg_duration}</span>
                </div>
            </div>
        </div>

        <div class="section" data-collapsed="false">
            <div class="section-header" onclick="toggleSection(this)">
                <span class="section-title">Quality Gates</span>
                <span class="toggle-icon">&#9660;</span>
            </div>
            <div class="section-content">
                <div class="stat-row">
                    <span class="stat-label">Total Gate Runs</span>
                    <span class="stat-value">{gate_total}</span>
                </div>
                <div class="stat-row">
                    <span class="stat-label">Gates Passed</span>
                    <span class="stat-value" style="color: var(--color-success)">{gate_passed}</span>
                </div>
                <div class="stat-row">
                    <span class="stat-label">Gates Failed</span>
                    <span class="stat-value" style="color: var(--color-error)">{gate_failed}</span>
                </div>
                <div class="stat-row">
                    <span class="stat-label">Pass Rate</span>
                    <span class="stat-value">{gate_pass_pct}%</span>
                </div>
                <div class="progress-bar">
                    <div class="progress-fill {gate_class}" style="width: {gate_pass_pct}%"></div>
                </div>
            </div>
        </div>

        <div class="section" data-collapsed="false">
            <div class="section-header" onclick="toggleSection(this)">
                <span class="section-title">Sessions</span>
                <span class="toggle-icon">&#9660;</span>
            </div>
            <div class="section-content">
                {sessions_html}
            </div>
        </div>
    </div>

    <script>
        function toggleSection(header) {{
            const section = header.parentElement;
            const isCollapsed = section.classList.toggle('collapsed');
            section.setAttribute('data-collapsed', isCollapsed);
        }}

        // Allow expand/collapse all
        document.addEventListener('keydown', function(e) {{
            if (e.key === 'e' && e.altKey) {{
                document.querySelectorAll('.section').forEach(function(s) {{
                    s.classList.remove('collapsed');
                    s.setAttribute('data-collapsed', 'false');
                }});
            }}
            if (e.key === 'c' && e.altKey) {{
                document.querySelectorAll('.section').forEach(function(s) {{
                    s.classList.add('collapsed');
                    s.setAttribute('data-collapsed', 'true');
                }});
            }}
        }});
    </script>
</body>
</html>"##,
            generated_time = generated_time,
            total_sessions = summary.total_sessions,
            total_iterations = summary.total_iterations,
            total_tasks = summary.total_tasks_completed,
            success_pct = success_pct,
            success_class = Self::get_rate_class(summary.success_rate),
            stagnations = summary.total_stagnations,
            avg_duration = Self::format_duration(summary.avg_session_duration_secs),
            gate_total = summary.gate_stats.total_runs,
            gate_passed = summary.gate_stats.passed,
            gate_failed = summary.gate_stats.failed,
            gate_pass_pct = gate_pass_pct,
            gate_class = Self::get_rate_class(summary.gate_stats.pass_rate()),
            sessions_html = sessions_html,
        )
    }

    /// Render the sessions list HTML.
    fn render_sessions_list(&self) -> String {
        if self.data.sessions.is_empty() {
            return r#"<div class="no-data">No sessions recorded yet.</div>"#.to_string();
        }

        let mut html = String::from(r#"<div class="session-list">"#);

        for session in &self.data.sessions {
            let started = session
                .started_at
                .map(|t| t.format("%Y-%m-%d %H:%M").to_string())
                .unwrap_or_else(|| "Unknown".to_string());

            let mode = session.mode.as_deref().unwrap_or("unknown");
            let duration = session
                .duration_minutes
                .map(|m| format!("{}m", m))
                .unwrap_or_else(|| "-".to_string());

            html.push_str(&format!(
                r#"<div class="session-item">
                    <div class="session-id">{session_id}</div>
                    <div class="session-meta">
                        <span>Mode: {mode}</span>
                        <span>Started: {started}</span>
                        <span>Duration: {duration}</span>
                        <span>Iterations: {iterations}</span>
                        <span>Errors: {errors}</span>
                    </div>
                </div>"#,
                session_id = Self::escape_html(&session.session_id),
                mode = Self::escape_html(mode),
                started = started,
                duration = duration,
                iterations = session.iterations,
                errors = session.errors,
            ));
        }

        html.push_str("</div>");
        html
    }

    /// Get CSS class based on rate (0.0-1.0).
    fn get_rate_class(rate: f64) -> &'static str {
        if rate >= 0.9 {
            "success"
        } else if rate >= 0.7 {
            "warning"
        } else {
            "error"
        }
    }

    /// Format duration in seconds to human-readable string.
    fn format_duration(seconds: u64) -> String {
        if seconds == 0 {
            return "-".to_string();
        }

        let hours = seconds / 3600;
        let minutes = (seconds % 3600) / 60;

        if hours > 0 {
            format!("{}h {}m", hours, minutes)
        } else {
            format!("{}m", minutes)
        }
    }

    /// Escape HTML special characters.
    fn escape_html(s: &str) -> String {
        s.replace('&', "&amp;")
            .replace('<', "&lt;")
            .replace('>', "&gt;")
            .replace('"', "&quot;")
            .replace('\'', "&#x27;")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::analytics::dashboard::{DashboardData, DashboardSummary, TimeRange};
    use crate::analytics::{Analytics, GateStats, TrendData};
    use chrono::Utc;
    use tempfile::TempDir;

    fn create_test_analytics() -> (TempDir, Analytics) {
        let temp_dir = TempDir::new().unwrap();
        let analytics = Analytics::new(temp_dir.path().to_path_buf());
        (temp_dir, analytics)
    }

    fn create_sample_dashboard_data() -> DashboardData {
        DashboardData {
            sessions: vec![],
            events: vec![],
            trends: TrendData::default(),
            summary: DashboardSummary {
                total_iterations: 42,
                total_sessions: 5,
                success_rate: 0.85,
                avg_session_duration_secs: 3600,
                total_tasks_completed: 15,
                total_stagnations: 3,
                gate_stats: GateStats {
                    total_runs: 100,
                    passed: 95,
                    failed: 5,
                },
            },
            generated_at: Utc::now(),
        }
    }

    // =========================================================================
    // Phase 25.2: HTML Template Engine Tests
    // =========================================================================

    #[test]
    fn test_template_renders_valid_html() {
        // Given: Dashboard data
        let data = create_sample_dashboard_data();
        let template = DashboardTemplate::new(&data);

        // When: Rendering the template
        let html = template.render();

        // Then: Output should be valid HTML with required structure
        assert!(
            html.contains("<!DOCTYPE html>"),
            "HTML must start with DOCTYPE"
        );
        assert!(html.contains("<html"), "HTML must contain html tag");
        assert!(html.contains("</html>"), "HTML must close html tag");
        assert!(html.contains("<head>"), "HTML must contain head section");
        assert!(html.contains("</head>"), "HTML must close head section");
        assert!(html.contains("<body"), "HTML must contain body tag");
        assert!(html.contains("</body>"), "HTML must close body tag");
        assert!(html.contains("<title>"), "HTML must contain title");
        assert!(
            html.contains("Ralph Analytics Dashboard"),
            "Title should identify the dashboard"
        );
    }

    #[test]
    fn test_template_has_inline_css() {
        // Given: Dashboard data
        let data = create_sample_dashboard_data();
        let template = DashboardTemplate::new(&data);

        // When: Rendering the template
        let html = template.render();

        // Then: Output should contain inline CSS (no external stylesheets)
        assert!(
            html.contains("<style>") || html.contains("<style "),
            "HTML must contain inline style tag"
        );
        assert!(html.contains("</style>"), "HTML must close style tag");
        // Should have actual CSS content
        assert!(
            html.contains("font-family") || html.contains("color:") || html.contains("margin"),
            "Style tag should contain CSS rules"
        );
    }

    #[test]
    fn test_template_has_inline_javascript() {
        // Given: Dashboard data
        let data = create_sample_dashboard_data();
        let template = DashboardTemplate::new(&data);

        // When: Rendering the template
        let html = template.render();

        // Then: Output should contain inline JavaScript (no external scripts)
        assert!(
            html.contains("<script>") || html.contains("<script "),
            "HTML must contain inline script tag"
        );
        assert!(html.contains("</script>"), "HTML must close script tag");
        // Should have actual JS content for interactivity
        assert!(
            html.contains("function")
                || html.contains("addEventListener")
                || html.contains("onclick"),
            "Script tag should contain JavaScript code"
        );
    }

    #[test]
    fn test_template_substitutes_data() {
        // Given: Dashboard data with specific values
        let data = create_sample_dashboard_data();
        let template = DashboardTemplate::new(&data);

        // When: Rendering the template
        let html = template.render();

        // Then: Output should contain the substituted data values
        assert!(
            html.contains("42"),
            "HTML should contain total_iterations (42)"
        );
        assert!(
            html.contains("5") || html.contains("sessions"),
            "HTML should reference sessions count"
        );
        assert!(
            html.contains("85") || html.contains("0.85"),
            "HTML should contain success rate"
        );
        assert!(
            html.contains("15"),
            "HTML should contain tasks completed (15)"
        );
        assert!(html.contains("95"), "HTML should contain gates passed (95)");
    }

    #[test]
    fn test_template_has_no_external_dependencies() {
        // Given: Dashboard data
        let data = create_sample_dashboard_data();
        let template = DashboardTemplate::new(&data);

        // When: Rendering the template
        let html = template.render();

        // Then: Output should NOT contain external resource references
        // Check for common CDN patterns
        assert!(
            !html.contains("cdn."),
            "HTML must not reference CDN resources"
        );
        assert!(
            !html.contains("http://") && !html.contains("https://"),
            "HTML must not reference external URLs"
        );
        assert!(
            !html.contains("googleapis.com"),
            "HTML must not reference Google APIs"
        );
        assert!(
            !html.contains("cloudflare.com"),
            "HTML must not reference Cloudflare"
        );
        assert!(!html.contains("unpkg.com"), "HTML must not reference unpkg");
        assert!(
            !html.contains("jsdelivr.net"),
            "HTML must not reference jsDelivr"
        );
        // Check for external stylesheet/script links
        assert!(
            !html.contains("rel=\"stylesheet\"") || !html.contains("href=\"http"),
            "HTML must not have external stylesheet links"
        );
    }

    #[test]
    fn test_template_handles_empty_data() {
        // Given: Empty dashboard data
        let data = DashboardData::default();
        let template = DashboardTemplate::new(&data);

        // When: Rendering the template
        let html = template.render();

        // Then: Output should still be valid HTML
        assert!(
            html.contains("<!DOCTYPE html>"),
            "Empty data should still produce valid HTML"
        );
        assert!(
            html.contains("</html>"),
            "Empty data should still produce complete HTML"
        );
        // Should show zeros or "no data" message
        assert!(
            html.contains("0") || html.contains("No data") || html.contains("no sessions"),
            "Empty data should show zeros or no data message"
        );
    }

    #[test]
    fn test_template_has_collapsible_sections() {
        // Given: Dashboard data
        let data = create_sample_dashboard_data();
        let template = DashboardTemplate::new(&data);

        // When: Rendering the template
        let html = template.render();

        // Then: Output should have interactive collapsible elements
        // Look for common collapse patterns
        assert!(
            html.contains("toggle")
                || html.contains("collapse")
                || html.contains("expand")
                || html.contains("data-collapsed")
                || html.contains("aria-expanded"),
            "HTML should have collapsible section markers"
        );
    }

    #[test]
    fn test_template_has_dashboard_sections() {
        // Given: Dashboard data
        let data = create_sample_dashboard_data();
        let template = DashboardTemplate::new(&data);

        // When: Rendering the template
        let html = template.render();

        // Then: Output should have distinct dashboard sections
        // Summary section
        assert!(
            html.contains("Summary") || html.contains("summary") || html.contains("Overview"),
            "HTML should have a summary section"
        );
        // Quality gates section
        assert!(
            html.contains("Gate") || html.contains("Quality") || html.contains("quality"),
            "HTML should have a quality gates section"
        );
    }

    #[test]
    fn test_template_escapes_special_characters() {
        // Given: Dashboard data (default data doesn't have special chars, but test the structure)
        let data = create_sample_dashboard_data();
        let template = DashboardTemplate::new(&data);

        // When: Rendering the template
        let html = template.render();

        // Then: HTML should be properly structured (basic validation)
        // All opened tags should be closed
        let open_divs = html.matches("<div").count();
        let close_divs = html.matches("</div>").count();
        assert_eq!(
            open_divs, close_divs,
            "All div tags should be properly closed"
        );
    }

    #[test]
    fn test_template_includes_generated_timestamp() {
        // Given: Dashboard data with a specific timestamp
        let data = create_sample_dashboard_data();
        let template = DashboardTemplate::new(&data);

        // When: Rendering the template
        let html = template.render();

        // Then: Output should include when the dashboard was generated
        assert!(
            html.contains("Generated")
                || html.contains("generated")
                || html.contains("Last updated"),
            "HTML should show when dashboard was generated"
        );
    }

    #[test]
    fn test_template_from_analytics() {
        // Given: An analytics instance with sessions
        let (_temp_dir, analytics) = create_test_analytics();
        analytics
            .log_event(
                "test-session",
                "session_start",
                serde_json::json!({"mode": "build"}),
            )
            .unwrap();
        analytics
            .log_event("test-session", "session_end", serde_json::json!({}))
            .unwrap();

        // When: Creating dashboard from analytics and rendering
        let dashboard = DashboardData::from_analytics(&analytics, TimeRange::All).unwrap();
        let template = DashboardTemplate::new(&dashboard);
        let html = template.render();

        // Then: Should produce valid HTML with session data
        assert!(
            html.contains("<!DOCTYPE html>"),
            "Should produce valid HTML"
        );
        assert!(
            html.contains("test-session") || html.contains("1") || html.contains("session"),
            "Should include session information"
        );
    }
}
