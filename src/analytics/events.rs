//! Structured event types for analytics logging.
//!
//! This module provides type-safe event definitions for the analytics system.
//! All events follow a consistent schema with versioning for forward compatibility.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

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
    pub fn new(
        session_id: impl Into<String>,
        event_type: EventType,
        data: serde_json::Value,
    ) -> Self {
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
        let type_matches =
            self.event_types.is_empty() || self.event_types.contains(&event.event_type);

        // Check session ID filter
        let session_matches =
            self.session_id.is_none() || self.session_id.as_ref() == Some(&event.session_id);

        type_matches && session_matches
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ============================================================================
    // EventType Tests
    // ============================================================================

    #[test]
    fn test_event_type_session_start_exists() {
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
        let event_type = EventType::GateResult;
        let json = serde_json::to_string(&event_type).unwrap();
        assert_eq!(json, "\"gate_result\"");

        let parsed: EventType = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed, EventType::GateResult);
    }

    #[test]
    fn test_event_type_all_variants() {
        let all = EventType::all_variants();
        assert!(all.contains(&EventType::SessionStart));
        assert!(all.contains(&EventType::SessionEnd));
        assert!(all.contains(&EventType::Iteration));
        assert!(all.contains(&EventType::IterationError));
        assert!(all.contains(&EventType::Stagnation));
        assert!(all.contains(&EventType::GateResult));
        assert!(all.contains(&EventType::PredictorDecision));
        assert_eq!(all.len(), 15);
    }

    // ============================================================================
    // StructuredEvent Tests
    // ============================================================================

    #[test]
    fn test_structured_event_has_schema_version() {
        let event = StructuredEvent::new(
            "test-session",
            EventType::SessionStart,
            serde_json::json!({}),
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
            EventType::SessionStart,
            serde_json::json!({}),
        );
        assert_eq!(event.session_id, "my-session-123");
    }

    #[test]
    fn test_structured_event_has_event_type() {
        let event = StructuredEvent::new("test", EventType::GateResult, serde_json::json!({}));
        assert_eq!(event.event_type, EventType::GateResult);
    }

    #[test]
    fn test_structured_event_serialization() {
        let event = StructuredEvent::new(
            "session-1",
            EventType::Iteration,
            serde_json::json!({"iteration": 5}),
        );

        let json = serde_json::to_string(&event).unwrap();
        assert!(json.contains("\"schema_version\":1"));
        assert!(json.contains("\"session_id\":\"session-1\""));
        assert!(json.contains("\"event_type\":\"iteration\""));
        assert!(json.contains("\"iteration\":5"));

        let parsed: StructuredEvent = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.session_id, "session-1");
        assert_eq!(parsed.event_type, EventType::Iteration);
    }

    // ============================================================================
    // GateResultEventData Tests
    // ============================================================================

    #[test]
    fn test_gate_result_event_data_structure() {
        let data = GateResultEventData {
            gate_name: "test_gate".to_string(),
            passed: true,
            issue_count: 0,
            duration_ms: 100,
            issues: vec![],
        };

        assert_eq!(data.gate_name, "test_gate");
        assert!(data.passed);
        assert_eq!(data.issue_count, 0);
    }

    #[test]
    fn test_gate_result_event_with_issues() {
        let issue = GateIssueEventData {
            severity: "warning".to_string(),
            message: "unused variable".to_string(),
            file: Some("src/lib.rs".to_string()),
            line: Some(42),
            code: Some("W0001".to_string()),
        };

        let data = GateResultEventData {
            gate_name: "clippy".to_string(),
            passed: false,
            issue_count: 1,
            duration_ms: 250,
            issues: vec![issue],
        };

        assert!(!data.passed);
        assert_eq!(data.issues.len(), 1);
        assert_eq!(data.issues[0].severity, "warning");
    }

    // ============================================================================
    // PredictorDecisionEventData Tests
    // ============================================================================

    #[test]
    fn test_predictor_decision_event_data_structure() {
        let decision = PredictorDecisionEventData {
            risk_score: 75.5,
            risk_level: "high".to_string(),
            action_recommended: Some("pause".to_string()),
            contributing_factors: vec!["factor1".to_string(), "factor2".to_string()],
        };

        assert!((decision.risk_score - 75.5).abs() < f64::EPSILON);
        assert_eq!(decision.risk_level, "high");
        assert_eq!(decision.action_recommended, Some("pause".to_string()));
        assert_eq!(decision.contributing_factors.len(), 2);
    }

    #[test]
    fn test_predictor_decision_serialization() {
        let decision = PredictorDecisionEventData {
            risk_score: 50.0,
            risk_level: "medium".to_string(),
            action_recommended: None,
            contributing_factors: vec![],
        };

        let json = serde_json::to_string(&decision).unwrap();
        let parsed: PredictorDecisionEventData = serde_json::from_str(&json).unwrap();

        assert!((parsed.risk_score - 50.0).abs() < f64::EPSILON);
        assert_eq!(parsed.risk_level, "medium");
    }

    // ============================================================================
    // EventFilter Tests
    // ============================================================================

    #[test]
    fn test_filter_events_by_single_type() {
        let event1 = StructuredEvent::new("s1", EventType::SessionStart, serde_json::json!({}));
        let event2 = StructuredEvent::new("s1", EventType::GateResult, serde_json::json!({}));

        let filter = EventFilter::new().with_event_type(EventType::GateResult);

        assert!(!filter.matches(&event1));
        assert!(filter.matches(&event2));
    }

    #[test]
    fn test_filter_events_by_multiple_types() {
        let event1 = StructuredEvent::new("s1", EventType::SessionStart, serde_json::json!({}));
        let event2 = StructuredEvent::new("s1", EventType::GateResult, serde_json::json!({}));
        let event3 = StructuredEvent::new("s1", EventType::Iteration, serde_json::json!({}));

        let filter = EventFilter::new()
            .with_event_type(EventType::SessionStart)
            .with_event_type(EventType::GateResult);

        assert!(filter.matches(&event1));
        assert!(filter.matches(&event2));
        assert!(!filter.matches(&event3));
    }

    #[test]
    fn test_filter_events_by_session() {
        let event1 = StructuredEvent::new("session-1", EventType::Iteration, serde_json::json!({}));
        let event2 = StructuredEvent::new("session-2", EventType::Iteration, serde_json::json!({}));

        let filter = EventFilter::new().with_session_id("session-1");

        assert!(filter.matches(&event1));
        assert!(!filter.matches(&event2));
    }

    #[test]
    fn test_filter_events_combined() {
        let event1 =
            StructuredEvent::new("session-1", EventType::GateResult, serde_json::json!({}));
        let event2 =
            StructuredEvent::new("session-2", EventType::GateResult, serde_json::json!({}));
        let event3 = StructuredEvent::new("session-1", EventType::Iteration, serde_json::json!({}));

        let filter = EventFilter::new()
            .with_event_type(EventType::GateResult)
            .with_session_id("session-1");

        assert!(filter.matches(&event1));
        assert!(!filter.matches(&event2)); // Wrong session
        assert!(!filter.matches(&event3)); // Wrong type
    }

    #[test]
    fn test_event_filter_default_returns_all() {
        let event =
            StructuredEvent::new("any-session", EventType::Stagnation, serde_json::json!({}));
        let filter = EventFilter::new();
        assert!(filter.matches(&event));
    }
}
