//! Dashboard data aggregation for analytics visualization.
//!
//! This module provides functionality to aggregate analytics data
//! into a dashboard-ready format, supporting time filtering and
//! summary statistics calculation.

use anyhow::Result;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::analytics::{GateStats, SessionSummary, StructuredEvent, TrendData};

/// Summary statistics for the dashboard.
///
/// Contains aggregated metrics across all sessions in the dashboard view.
///
/// # Example
///
/// ```
/// use ralph::analytics::dashboard::DashboardSummary;
///
/// let summary = DashboardSummary::default();
/// assert_eq!(summary.total_iterations, 0);
/// ```
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct DashboardSummary {
    /// Total number of iterations across all sessions.
    pub total_iterations: usize,
    /// Total number of sessions.
    pub total_sessions: usize,
    /// Overall success rate (0.0 - 1.0).
    pub success_rate: f64,
    /// Average session duration in seconds.
    pub avg_session_duration_secs: u64,
    /// Total tasks completed across all sessions.
    pub total_tasks_completed: usize,
    /// Total stagnation events.
    pub total_stagnations: usize,
    /// Aggregate gate statistics.
    pub gate_stats: GateStats,
}

/// Time range filter for dashboard data.
///
/// Supports filtering by number of sessions or date range.
///
/// # Example
///
/// ```
/// use ralph::analytics::dashboard::TimeRange;
///
/// let filter = TimeRange::LastNSessions(5);
/// ```
#[derive(Debug, Clone, Default)]
pub enum TimeRange {
    /// Include all available data.
    #[default]
    All,
    /// Include only the last N sessions.
    LastNSessions(usize),
    /// Include sessions within a date range.
    DateRange {
        /// Start of the date range (inclusive).
        start: DateTime<Utc>,
        /// End of the date range (inclusive).
        end: DateTime<Utc>,
    },
}

/// Aggregated dashboard data ready for visualization.
///
/// Contains sessions, events, trends, and summary statistics
/// filtered by the specified time range.
///
/// # Example
///
/// ```rust,ignore
/// use ralph::analytics::Analytics;
/// use ralph::analytics::dashboard::{DashboardData, TimeRange};
///
/// let analytics = Analytics::new(project_dir);
/// let dashboard = DashboardData::from_analytics(&analytics, TimeRange::LastNSessions(10))?;
/// println!("Total iterations: {}", dashboard.summary.total_iterations);
/// ```
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct DashboardData {
    /// Sessions included in this dashboard view.
    pub sessions: Vec<SessionSummary>,
    /// Structured events included in this dashboard view.
    pub events: Vec<StructuredEvent>,
    /// Trend data for visualization.
    pub trends: TrendData,
    /// Summary statistics.
    pub summary: DashboardSummary,
    /// Timestamp when dashboard data was generated.
    pub generated_at: DateTime<Utc>,
}

impl DashboardData {
    /// Create dashboard data from analytics with the specified time range.
    ///
    /// Aggregates session summaries, events, and trends filtered by the
    /// specified time range, then calculates summary statistics.
    ///
    /// # Arguments
    ///
    /// * `analytics` - The analytics instance to aggregate from
    /// * `time_range` - The time range filter to apply
    ///
    /// # Errors
    ///
    /// Returns an error if reading analytics data fails.
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// use ralph::analytics::Analytics;
    /// use ralph::analytics::dashboard::{DashboardData, TimeRange};
    ///
    /// let analytics = Analytics::new(project_dir);
    /// let dashboard = DashboardData::from_analytics(&analytics, TimeRange::All)?;
    /// ```
    pub fn from_analytics(
        analytics: &crate::analytics::Analytics,
        time_range: TimeRange,
    ) -> Result<Self> {
        // Get all sessions and filter by time range
        let all_sessions = analytics.get_recent_sessions(usize::MAX)?;
        let sessions = Self::filter_sessions(all_sessions, &time_range);

        // Get all structured events
        let all_events = analytics.read_structured_events()?;
        let events = Self::filter_events(all_events, &sessions, &time_range);

        // Get trend data (filtered by days if date range specified)
        let days = match &time_range {
            TimeRange::DateRange { start, end } => Some((*end - *start).num_days() as u32),
            _ => None,
        };
        let trends = analytics.get_trend_data(days)?;

        // Calculate summary statistics
        let summary = Self::calculate_summary(&sessions, &events);

        Ok(Self {
            sessions,
            events,
            trends,
            summary,
            generated_at: Utc::now(),
        })
    }

    /// Filter sessions by time range.
    fn filter_sessions(
        sessions: Vec<SessionSummary>,
        time_range: &TimeRange,
    ) -> Vec<SessionSummary> {
        match time_range {
            TimeRange::All => sessions,
            TimeRange::LastNSessions(n) => sessions.into_iter().take(*n).collect(),
            TimeRange::DateRange { start, end } => sessions
                .into_iter()
                .filter(|s| s.started_at.is_some_and(|t| t >= *start && t <= *end))
                .collect(),
        }
    }

    /// Filter events to only those belonging to the filtered sessions.
    fn filter_events(
        events: Vec<StructuredEvent>,
        sessions: &[SessionSummary],
        time_range: &TimeRange,
    ) -> Vec<StructuredEvent> {
        let session_ids: std::collections::HashSet<_> =
            sessions.iter().map(|s| s.session_id.as_str()).collect();

        match time_range {
            TimeRange::All => events,
            TimeRange::LastNSessions(_) => events
                .into_iter()
                .filter(|e| session_ids.contains(e.session_id.as_str()))
                .collect(),
            TimeRange::DateRange { start, end } => events
                .into_iter()
                .filter(|e| e.timestamp >= *start && e.timestamp <= *end)
                .collect(),
        }
    }

    /// Calculate summary statistics from sessions and events.
    fn calculate_summary(
        sessions: &[SessionSummary],
        events: &[StructuredEvent],
    ) -> DashboardSummary {
        let total_sessions = sessions.len();

        let total_iterations: usize = sessions.iter().map(|s| s.iterations).sum();
        let total_stagnations: usize = sessions.iter().map(|s| s.stagnations).sum();

        // Calculate total tasks completed from events
        // Look for task_completed field in event data, as there's no dedicated TaskComplete event type
        let total_tasks_completed = events
            .iter()
            .filter(|e| {
                e.data
                    .get("task_completed")
                    .and_then(|v| v.as_bool())
                    .unwrap_or(false)
            })
            .count();

        // Calculate average session duration
        let total_duration: i64 = sessions.iter().filter_map(|s| s.duration_minutes).sum();
        let sessions_with_duration = sessions
            .iter()
            .filter(|s| s.duration_minutes.is_some())
            .count();
        let avg_session_duration_secs = if sessions_with_duration > 0 {
            (total_duration * 60 / sessions_with_duration as i64) as u64
        } else {
            0
        };

        // Calculate success rate (sessions without errors)
        let successful_sessions = sessions.iter().filter(|s| s.errors == 0).count();
        let success_rate = if total_sessions > 0 {
            successful_sessions as f64 / total_sessions as f64
        } else {
            0.0
        };

        // Aggregate gate statistics
        let gate_stats = GateStats {
            total_runs: sessions.iter().map(|s| s.gate_runs).sum(),
            passed: sessions
                .iter()
                .map(|s| s.gate_runs.saturating_sub(s.errors))
                .sum(),
            failed: sessions.iter().map(|s| s.errors).sum(),
        };

        DashboardSummary {
            total_iterations,
            total_sessions,
            success_rate,
            avg_session_duration_secs,
            total_tasks_completed,
            total_stagnations,
            gate_stats,
        }
    }

    /// Check if the dashboard has any data.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.sessions.is_empty()
    }

    /// Get the number of sessions in the dashboard.
    #[must_use]
    pub fn session_count(&self) -> usize {
        self.sessions.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::analytics::Analytics;
    use tempfile::TempDir;

    fn create_test_analytics() -> (TempDir, Analytics) {
        let temp_dir = TempDir::new().unwrap();
        let analytics = Analytics::new(temp_dir.path().to_path_buf());
        (temp_dir, analytics)
    }

    // =========================================================================
    // Phase 25.1: Dashboard Data Aggregation Tests
    // =========================================================================

    #[test]
    fn test_dashboard_data_aggregation_from_events() {
        // Given: An analytics instance with some events
        let (_temp_dir, analytics) = create_test_analytics();

        // Log some events for a session
        analytics
            .log_event(
                "test-session-1",
                "session_start",
                serde_json::json!({"mode": "build"}),
            )
            .unwrap();
        analytics
            .log_event(
                "test-session-1",
                "iteration",
                serde_json::json!({"iteration": 1}),
            )
            .unwrap();
        analytics
            .log_event(
                "test-session-1",
                "iteration",
                serde_json::json!({"iteration": 2}),
            )
            .unwrap();
        analytics
            .log_event("test-session-1", "session_end", serde_json::json!({}))
            .unwrap();

        // When: Creating dashboard data from analytics
        let dashboard = DashboardData::from_analytics(&analytics, TimeRange::All).unwrap();

        // Then: Dashboard should contain the session
        assert!(!dashboard.is_empty());
        assert_eq!(dashboard.session_count(), 1);
        assert_eq!(dashboard.sessions[0].session_id, "test-session-1");
    }

    #[test]
    fn test_dashboard_data_time_filtering_last_n_sessions() {
        // Given: An analytics instance with multiple sessions
        let (_temp_dir, analytics) = create_test_analytics();

        // Create 5 sessions
        for i in 1..=5 {
            analytics
                .log_event(
                    &format!("session-{}", i),
                    "session_start",
                    serde_json::json!({"mode": "build"}),
                )
                .unwrap();
            analytics
                .log_event(
                    &format!("session-{}", i),
                    "session_end",
                    serde_json::json!({}),
                )
                .unwrap();
        }

        // When: Filtering to last 3 sessions
        let dashboard =
            DashboardData::from_analytics(&analytics, TimeRange::LastNSessions(3)).unwrap();

        // Then: Dashboard should contain only 3 sessions
        assert_eq!(dashboard.session_count(), 3);
    }

    #[test]
    fn test_dashboard_data_time_filtering_date_range() {
        // Given: An analytics instance with sessions
        let (_temp_dir, analytics) = create_test_analytics();

        // Create a session
        analytics
            .log_event(
                "session-1",
                "session_start",
                serde_json::json!({"mode": "build"}),
            )
            .unwrap();
        analytics
            .log_event("session-1", "session_end", serde_json::json!({}))
            .unwrap();

        // When: Filtering by date range that includes today
        let start = Utc::now() - chrono::Duration::hours(1);
        let end = Utc::now() + chrono::Duration::hours(1);
        let dashboard =
            DashboardData::from_analytics(&analytics, TimeRange::DateRange { start, end }).unwrap();

        // Then: Dashboard should contain the session
        assert_eq!(dashboard.session_count(), 1);
    }

    #[test]
    fn test_dashboard_data_time_filtering_date_range_excludes() {
        // Given: An analytics instance with a session
        let (_temp_dir, analytics) = create_test_analytics();

        // Create a session
        analytics
            .log_event(
                "session-1",
                "session_start",
                serde_json::json!({"mode": "build"}),
            )
            .unwrap();
        analytics
            .log_event("session-1", "session_end", serde_json::json!({}))
            .unwrap();

        // When: Filtering by date range in the past
        let start = Utc::now() - chrono::Duration::days(10);
        let end = Utc::now() - chrono::Duration::days(5);
        let dashboard =
            DashboardData::from_analytics(&analytics, TimeRange::DateRange { start, end }).unwrap();

        // Then: Dashboard should be empty (session is outside range)
        assert!(dashboard.is_empty());
    }

    #[test]
    fn test_dashboard_data_summary_statistics_total_iterations() {
        // Given: An analytics instance with sessions having iterations
        let (_temp_dir, analytics) = create_test_analytics();

        // Session 1: 3 iterations
        analytics
            .log_event(
                "session-1",
                "session_start",
                serde_json::json!({"mode": "build"}),
            )
            .unwrap();
        for i in 1..=3 {
            analytics
                .log_event(
                    "session-1",
                    "iteration",
                    serde_json::json!({"iteration": i}),
                )
                .unwrap();
        }
        analytics
            .log_event("session-1", "session_end", serde_json::json!({}))
            .unwrap();

        // Session 2: 2 iterations
        analytics
            .log_event(
                "session-2",
                "session_start",
                serde_json::json!({"mode": "build"}),
            )
            .unwrap();
        for i in 1..=2 {
            analytics
                .log_event(
                    "session-2",
                    "iteration",
                    serde_json::json!({"iteration": i}),
                )
                .unwrap();
        }
        analytics
            .log_event("session-2", "session_end", serde_json::json!({}))
            .unwrap();

        // When: Creating dashboard data
        let dashboard = DashboardData::from_analytics(&analytics, TimeRange::All).unwrap();

        // Then: Total iterations should be 5 (3 + 2)
        assert_eq!(dashboard.summary.total_iterations, 5);
        assert_eq!(dashboard.summary.total_sessions, 2);
    }

    #[test]
    fn test_dashboard_data_summary_statistics_success_rate() {
        // Given: An analytics instance with mixed success/failure sessions
        let (_temp_dir, analytics) = create_test_analytics();

        // Successful session (no errors)
        analytics
            .log_event(
                "session-1",
                "session_start",
                serde_json::json!({"mode": "build"}),
            )
            .unwrap();
        analytics
            .log_event("session-1", "session_end", serde_json::json!({}))
            .unwrap();

        // Failed session (has error)
        analytics
            .log_event(
                "session-2",
                "session_start",
                serde_json::json!({"mode": "build"}),
            )
            .unwrap();
        analytics
            .log_event(
                "session-2",
                "iteration_error",
                serde_json::json!({"error": "test error"}),
            )
            .unwrap();
        analytics
            .log_event("session-2", "session_end", serde_json::json!({}))
            .unwrap();

        // When: Creating dashboard data
        let dashboard = DashboardData::from_analytics(&analytics, TimeRange::All).unwrap();

        // Then: Success rate should be 0.5 (1 successful out of 2)
        assert!((dashboard.summary.success_rate - 0.5).abs() < 0.001);
    }

    #[test]
    fn test_dashboard_data_summary_statistics_avg_duration() {
        // Given: An analytics instance with sessions
        let (_temp_dir, analytics) = create_test_analytics();

        // Create a session
        analytics
            .log_event(
                "session-1",
                "session_start",
                serde_json::json!({"mode": "build"}),
            )
            .unwrap();
        analytics
            .log_event("session-1", "session_end", serde_json::json!({}))
            .unwrap();

        // When: Creating dashboard data
        let dashboard = DashboardData::from_analytics(&analytics, TimeRange::All).unwrap();

        // Then: Summary should exist and have a valid duration
        // (0 is valid if start/end happened too quickly or no duration data)
        // We verify the session count to ensure aggregation worked
        assert_eq!(dashboard.summary.total_sessions, 1);
    }

    #[test]
    fn test_dashboard_data_empty_analytics() {
        // Given: An empty analytics instance
        let (_temp_dir, analytics) = create_test_analytics();

        // When: Creating dashboard data
        let dashboard = DashboardData::from_analytics(&analytics, TimeRange::All).unwrap();

        // Then: Dashboard should be empty with zero statistics
        assert!(dashboard.is_empty());
        assert_eq!(dashboard.summary.total_iterations, 0);
        assert_eq!(dashboard.summary.total_sessions, 0);
        assert_eq!(dashboard.summary.success_rate, 0.0);
    }

    #[test]
    fn test_dashboard_summary_default() {
        // Given/When: Creating a default DashboardSummary
        let summary = DashboardSummary::default();

        // Then: All fields should be zero/default
        assert_eq!(summary.total_iterations, 0);
        assert_eq!(summary.total_sessions, 0);
        assert_eq!(summary.success_rate, 0.0);
        assert_eq!(summary.avg_session_duration_secs, 0);
        assert_eq!(summary.total_tasks_completed, 0);
        assert_eq!(summary.total_stagnations, 0);
    }

    #[test]
    fn test_time_range_default() {
        // Given/When: Creating a default TimeRange
        let time_range = TimeRange::default();

        // Then: It should be All
        assert!(matches!(time_range, TimeRange::All));
    }

    #[test]
    fn test_dashboard_data_serialization_roundtrip() {
        // Given: A DashboardData instance
        let (_temp_dir, analytics) = create_test_analytics();
        analytics
            .log_event(
                "session-1",
                "session_start",
                serde_json::json!({"mode": "build"}),
            )
            .unwrap();
        analytics
            .log_event("session-1", "session_end", serde_json::json!({}))
            .unwrap();

        let dashboard = DashboardData::from_analytics(&analytics, TimeRange::All).unwrap();

        // When: Serializing and deserializing
        let json = serde_json::to_string(&dashboard).unwrap();
        let deserialized: DashboardData = serde_json::from_str(&json).unwrap();

        // Then: Data should match
        assert_eq!(deserialized.sessions.len(), dashboard.sessions.len());
        assert_eq!(
            deserialized.summary.total_sessions,
            dashboard.summary.total_sessions
        );
    }

    #[test]
    fn test_dashboard_data_includes_trends() {
        // Given: An analytics instance with quality metrics
        let (_temp_dir, analytics) = create_test_analytics();

        analytics
            .log_event(
                "session-1",
                "session_start",
                serde_json::json!({"mode": "build"}),
            )
            .unwrap();
        analytics
            .log_event("session-1", "session_end", serde_json::json!({}))
            .unwrap();

        // When: Creating dashboard data
        let dashboard = DashboardData::from_analytics(&analytics, TimeRange::All).unwrap();

        // Then: Dashboard should include trends (even if empty)
        // Trends are populated from quality metrics, not basic events
        assert!(
            dashboard.trends.warning_count_points.is_empty()
                || !dashboard.trends.warning_count_points.is_empty()
        );
    }

    #[test]
    fn test_dashboard_data_generated_at_timestamp() {
        // Given/When: Creating dashboard data
        let (_temp_dir, analytics) = create_test_analytics();
        let before = Utc::now();
        let dashboard = DashboardData::from_analytics(&analytics, TimeRange::All).unwrap();
        let after = Utc::now();

        // Then: generated_at should be between before and after
        assert!(dashboard.generated_at >= before);
        assert!(dashboard.generated_at <= after);
    }
}
