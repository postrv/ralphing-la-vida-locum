//! Session persistence and state recovery.
//!
//! This module provides the unified session state structure that captures all
//! recoverable state for Ralph's automation loop. It enables Ralph to survive
//! crashes and restarts without losing progress.
//!
//! # Architecture
//!
//! ```text
//! SessionState
//!   ├── metadata: SessionMetadata (version, timestamp, pid)
//!   ├── loop_state: LoopState (iteration, mode, stagnation)
//!   ├── task_tracker: TaskTracker (task progress)
//!   ├── supervisor: SupervisorSnapshot (health history, error tracking)
//!   └── predictor: PredictorSnapshot (prediction history)
//! ```
//!
//! # Forward Compatibility
//!
//! Session state includes a version field to handle schema evolution:
//! - Compatible versions can be loaded directly
//! - Incompatible versions are rejected gracefully (fresh start)
//!
//! # Persistence
//!
//! The [`SessionPersistence`] struct provides atomic file-based storage:
//! - Atomic writes prevent corruption on crash
//! - File locking prevents concurrent Ralph instances
//! - Corrupted files are handled gracefully (deleted with warning)

pub mod persistence;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::r#loop::state::LoopState;
use crate::r#loop::task_tracker::TaskTracker;
use crate::supervisor::{HealthMetrics, SupervisorVerdict};
use crate::supervisor::predictor::{PredictorConfig, RiskScore};

/// Current schema version for session state.
/// Increment when making breaking changes to the serialization format.
pub const SESSION_STATE_VERSION: u32 = 1;

/// Minimum supported schema version for backward compatibility.
/// Sessions with versions below this will be rejected.
pub const MIN_SUPPORTED_VERSION: u32 = 1;

/// Session metadata containing version and timing information.
///
/// # Example
///
/// ```rust
/// use ralph::session::SessionMetadata;
///
/// let metadata = SessionMetadata::new();
/// assert_eq!(metadata.version, SessionMetadata::CURRENT_VERSION);
/// ```
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SessionMetadata {
    /// Schema version for forward compatibility.
    pub version: u32,
    /// When this session was created.
    pub created_at: DateTime<Utc>,
    /// When this session was last saved.
    pub saved_at: DateTime<Utc>,
    /// Process ID that last wrote this session.
    pub pid: u32,
    /// Unique session identifier.
    pub session_id: String,
}

impl SessionMetadata {
    /// Current schema version.
    pub const CURRENT_VERSION: u32 = SESSION_STATE_VERSION;

    /// Creates new session metadata with current timestamp and PID.
    ///
    /// # Example
    ///
    /// ```rust
    /// use ralph::session::SessionMetadata;
    ///
    /// let metadata = SessionMetadata::new();
    /// assert_eq!(metadata.version, SessionMetadata::CURRENT_VERSION);
    /// assert!(metadata.pid > 0);
    /// ```
    #[must_use]
    pub fn new() -> Self {
        let now = Utc::now();
        Self {
            version: Self::CURRENT_VERSION,
            created_at: now,
            saved_at: now,
            pid: std::process::id(),
            session_id: now.timestamp_millis().to_string(),
        }
    }

    /// Updates the saved_at timestamp and PID.
    pub fn touch(&mut self) {
        self.saved_at = Utc::now();
        self.pid = std::process::id();
    }
}

impl Default for SessionMetadata {
    fn default() -> Self {
        Self::new()
    }
}

/// Serializable snapshot of Supervisor state.
///
/// Captures the essential state needed to restore supervisor behavior
/// across session restarts.
///
/// # Example
///
/// ```rust
/// use ralph::session::SupervisorSnapshot;
///
/// let snapshot = SupervisorSnapshot::default();
/// assert_eq!(snapshot.mode_switch_count, 0);
/// assert!(snapshot.last_error.is_none());
/// ```
#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
pub struct SupervisorSnapshot {
    /// Number of mode switches recorded.
    pub mode_switch_count: u32,
    /// Last recorded error message.
    pub last_error: Option<String>,
    /// Count of how many times the last error repeated.
    pub error_repeat_count: u32,
    /// Last iteration when supervisor ran a check.
    pub last_check_iteration: u32,
    /// Recent health metrics history.
    pub health_history: Vec<HealthMetrics>,
    /// Most recent supervisor verdict.
    pub last_verdict: Option<SupervisorVerdict>,
}

impl SupervisorSnapshot {
    /// Creates an empty supervisor snapshot.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Returns true if this snapshot has no meaningful state.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.mode_switch_count == 0
            && self.last_error.is_none()
            && self.error_repeat_count == 0
            && self.last_check_iteration == 0
            && self.health_history.is_empty()
            && self.last_verdict.is_none()
    }
}

/// Serializable snapshot of StagnationPredictor state.
///
/// Captures prediction history for accuracy tracking across sessions.
///
/// # Example
///
/// ```rust
/// use ralph::session::PredictorSnapshot;
///
/// let snapshot = PredictorSnapshot::default();
/// assert!(snapshot.prediction_history.is_empty());
/// ```
#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
pub struct PredictorSnapshot {
    /// History of predictions: (risk_score, was_correct).
    pub prediction_history: Vec<(RiskScore, bool)>,
    /// Configuration used for predictions.
    pub config: Option<PredictorConfig>,
}

impl PredictorSnapshot {
    /// Creates an empty predictor snapshot.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Returns true if this snapshot has no prediction history.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.prediction_history.is_empty() && self.config.is_none()
    }
}

/// Unified session state capturing all recoverable loop state.
///
/// This struct contains everything needed to restore Ralph's automation
/// loop after a crash or restart.
///
/// # Version Compatibility
///
/// The session state includes a version field for forward compatibility.
/// Use [`SessionState::is_compatible_version`] to check if a loaded
/// state can be used.
///
/// # Example
///
/// ```rust
/// use ralph::session::SessionState;
///
/// let state = SessionState::new();
/// assert!(state.is_empty());
///
/// // Serialize and deserialize
/// let json = serde_json::to_string(&state).unwrap();
/// let restored: SessionState = serde_json::from_str(&json).unwrap();
/// assert!(restored.is_empty());
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionState {
    /// Session metadata (version, timestamps, PID).
    pub metadata: SessionMetadata,
    /// Loop execution state.
    pub loop_state: Option<LoopState>,
    /// Task tracker state.
    pub task_tracker: Option<TaskTracker>,
    /// Supervisor state snapshot.
    pub supervisor: SupervisorSnapshot,
    /// Predictor state snapshot.
    pub predictor: PredictorSnapshot,
}

impl SessionState {
    /// Creates a new empty session state.
    ///
    /// # Example
    ///
    /// ```rust
    /// use ralph::session::SessionState;
    ///
    /// let state = SessionState::new();
    /// assert!(state.is_empty());
    /// assert!(state.loop_state.is_none());
    /// assert!(state.task_tracker.is_none());
    /// ```
    #[must_use]
    pub fn new() -> Self {
        Self {
            metadata: SessionMetadata::new(),
            loop_state: None,
            task_tracker: None,
            supervisor: SupervisorSnapshot::new(),
            predictor: PredictorSnapshot::new(),
        }
    }

    /// Returns true if this session state is empty (no meaningful state).
    ///
    /// # Example
    ///
    /// ```rust
    /// use ralph::session::SessionState;
    /// use ralph::r#loop::state::{LoopState, LoopMode};
    ///
    /// let mut state = SessionState::new();
    /// assert!(state.is_empty());
    ///
    /// state.loop_state = Some(LoopState::new(LoopMode::Build));
    /// assert!(!state.is_empty());
    /// ```
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.loop_state.is_none()
            && self.task_tracker.is_none()
            && self.supervisor.is_empty()
            && self.predictor.is_empty()
    }

    /// Checks if a version number is compatible with this implementation.
    ///
    /// # Arguments
    ///
    /// * `version` - The version number to check.
    ///
    /// # Returns
    ///
    /// `true` if the version is compatible, `false` otherwise.
    ///
    /// # Example
    ///
    /// ```rust
    /// use ralph::session::SessionState;
    ///
    /// // Current version is always compatible
    /// assert!(SessionState::is_compatible_version(1));
    ///
    /// // Version 0 is not supported
    /// assert!(!SessionState::is_compatible_version(0));
    /// ```
    #[must_use]
    pub fn is_compatible_version(version: u32) -> bool {
        version >= MIN_SUPPORTED_VERSION && version <= SESSION_STATE_VERSION
    }

    /// Checks if this session state has a compatible version.
    ///
    /// # Example
    ///
    /// ```rust
    /// use ralph::session::SessionState;
    ///
    /// let state = SessionState::new();
    /// assert!(state.is_version_compatible());
    /// ```
    #[must_use]
    pub fn is_version_compatible(&self) -> bool {
        Self::is_compatible_version(self.metadata.version)
    }

    /// Updates the session metadata timestamp before saving.
    pub fn touch(&mut self) {
        self.metadata.touch();
    }

    /// Sets the loop state.
    pub fn set_loop_state(&mut self, state: LoopState) {
        self.loop_state = Some(state);
    }

    /// Sets the task tracker.
    pub fn set_task_tracker(&mut self, tracker: TaskTracker) {
        self.task_tracker = Some(tracker);
    }

    /// Sets the supervisor snapshot.
    pub fn set_supervisor(&mut self, snapshot: SupervisorSnapshot) {
        self.supervisor = snapshot;
    }

    /// Sets the predictor snapshot.
    pub fn set_predictor(&mut self, snapshot: PredictorSnapshot) {
        self.predictor = snapshot;
    }

    /// Gets the session ID from metadata.
    #[must_use]
    pub fn session_id(&self) -> &str {
        &self.metadata.session_id
    }

    /// Gets the schema version.
    #[must_use]
    pub fn version(&self) -> u32 {
        self.metadata.version
    }

    /// Gets when this session was created.
    #[must_use]
    pub fn created_at(&self) -> DateTime<Utc> {
        self.metadata.created_at
    }

    /// Gets when this session was last saved.
    #[must_use]
    pub fn saved_at(&self) -> DateTime<Utc> {
        self.metadata.saved_at
    }
}

impl Default for SessionState {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::r#loop::state::{LoopMode, LoopState};
    use crate::r#loop::task_tracker::{TaskTracker, TaskTrackerConfig};

    #[test]
    fn test_session_state_serialization_roundtrip() {
        // Create a session state with all components populated
        let mut state = SessionState::new();
        state.set_loop_state(LoopState::new(LoopMode::Build));
        state.set_task_tracker(TaskTracker::new(TaskTrackerConfig::default()));
        state.supervisor.mode_switch_count = 3;
        state.supervisor.last_error = Some("test error".to_string());
        state.predictor.prediction_history.push((42.5, true));

        // Serialize to JSON
        let json = serde_json::to_string_pretty(&state).expect("serialization should succeed");

        // Deserialize back
        let restored: SessionState =
            serde_json::from_str(&json).expect("deserialization should succeed");

        // Verify all fields roundtrip correctly
        assert_eq!(restored.metadata.version, state.metadata.version);
        assert!(restored.loop_state.is_some());
        assert_eq!(
            restored.loop_state.as_ref().unwrap().mode,
            LoopMode::Build
        );
        assert!(restored.task_tracker.is_some());
        assert_eq!(restored.supervisor.mode_switch_count, 3);
        assert_eq!(
            restored.supervisor.last_error,
            Some("test error".to_string())
        );
        assert_eq!(restored.predictor.prediction_history.len(), 1);
        assert!((restored.predictor.prediction_history[0].0 - 42.5).abs() < f64::EPSILON);
        assert!(restored.predictor.prediction_history[0].1);
    }

    #[test]
    fn test_session_state_version_compatibility() {
        // Current version is compatible
        assert!(SessionState::is_compatible_version(SESSION_STATE_VERSION));
        assert!(SessionState::is_compatible_version(1));

        // Version 0 is not supported
        assert!(!SessionState::is_compatible_version(0));

        // Future versions are not compatible (forward compatibility boundary)
        assert!(!SessionState::is_compatible_version(SESSION_STATE_VERSION + 1));
        assert!(!SessionState::is_compatible_version(100));

        // Test on actual state
        let state = SessionState::new();
        assert!(state.is_version_compatible());
    }

    #[test]
    fn test_session_state_includes_all_components() {
        let state = SessionState::new();

        // Verify structure has all required fields
        let _ = &state.metadata;
        let _ = &state.metadata.version;
        let _ = &state.metadata.created_at;
        let _ = &state.metadata.saved_at;
        let _ = &state.metadata.pid;
        let _ = &state.metadata.session_id;
        let _ = &state.loop_state;
        let _ = &state.task_tracker;
        let _ = &state.supervisor;
        let _ = &state.supervisor.mode_switch_count;
        let _ = &state.supervisor.last_error;
        let _ = &state.supervisor.error_repeat_count;
        let _ = &state.supervisor.health_history;
        let _ = &state.predictor;
        let _ = &state.predictor.prediction_history;

        // All components are accessible and have expected types
        assert_eq!(state.metadata.version, SESSION_STATE_VERSION);
        assert!(state.metadata.pid > 0);
        assert!(!state.metadata.session_id.is_empty());
    }

    #[test]
    fn test_session_state_default_is_empty() {
        let state = SessionState::new();
        assert!(state.is_empty());
        assert!(state.loop_state.is_none());
        assert!(state.task_tracker.is_none());
        assert!(state.supervisor.is_empty());
        assert!(state.predictor.is_empty());

        // Default implementation should also be empty
        let default_state = SessionState::default();
        assert!(default_state.is_empty());
    }

    #[test]
    fn test_session_state_not_empty_with_loop_state() {
        let mut state = SessionState::new();
        assert!(state.is_empty());

        state.set_loop_state(LoopState::new(LoopMode::Build));
        assert!(!state.is_empty());
    }

    #[test]
    fn test_session_state_not_empty_with_task_tracker() {
        let mut state = SessionState::new();
        assert!(state.is_empty());

        state.set_task_tracker(TaskTracker::new(TaskTrackerConfig::default()));
        assert!(!state.is_empty());
    }

    #[test]
    fn test_session_state_not_empty_with_supervisor_state() {
        let mut state = SessionState::new();
        assert!(state.is_empty());

        state.supervisor.mode_switch_count = 1;
        assert!(!state.is_empty());
    }

    #[test]
    fn test_session_state_not_empty_with_predictor_state() {
        let mut state = SessionState::new();
        assert!(state.is_empty());

        state.predictor.prediction_history.push((50.0, false));
        assert!(!state.is_empty());
    }

    #[test]
    fn test_session_metadata_new() {
        let metadata = SessionMetadata::new();
        assert_eq!(metadata.version, SESSION_STATE_VERSION);
        assert!(metadata.pid > 0);
        assert!(!metadata.session_id.is_empty());
        // created_at and saved_at should be close to now
        let now = Utc::now();
        let diff = now.signed_duration_since(metadata.created_at);
        assert!(diff.num_seconds() < 1);
    }

    #[test]
    fn test_session_metadata_touch() {
        let mut metadata = SessionMetadata::new();
        let original_saved_at = metadata.saved_at;

        // Small delay to ensure time difference
        std::thread::sleep(std::time::Duration::from_millis(10));

        metadata.touch();

        // saved_at should be updated
        assert!(metadata.saved_at > original_saved_at);
        // created_at should remain unchanged
        assert_eq!(metadata.created_at, metadata.created_at);
    }

    #[test]
    fn test_supervisor_snapshot_is_empty() {
        let snapshot = SupervisorSnapshot::new();
        assert!(snapshot.is_empty());

        let mut snapshot_with_count = SupervisorSnapshot::new();
        snapshot_with_count.mode_switch_count = 1;
        assert!(!snapshot_with_count.is_empty());

        let mut snapshot_with_error = SupervisorSnapshot::new();
        snapshot_with_error.last_error = Some("error".to_string());
        assert!(!snapshot_with_error.is_empty());

        let mut snapshot_with_history = SupervisorSnapshot::new();
        snapshot_with_history.health_history.push(HealthMetrics::default());
        assert!(!snapshot_with_history.is_empty());
    }

    #[test]
    fn test_predictor_snapshot_is_empty() {
        let snapshot = PredictorSnapshot::new();
        assert!(snapshot.is_empty());

        let mut snapshot_with_history = PredictorSnapshot::new();
        snapshot_with_history.prediction_history.push((30.0, true));
        assert!(!snapshot_with_history.is_empty());
    }

    #[test]
    fn test_session_state_touch() {
        let mut state = SessionState::new();
        let original_saved_at = state.saved_at();

        std::thread::sleep(std::time::Duration::from_millis(10));

        state.touch();
        assert!(state.saved_at() > original_saved_at);
    }

    #[test]
    fn test_session_state_getters() {
        let state = SessionState::new();

        assert!(!state.session_id().is_empty());
        assert_eq!(state.version(), SESSION_STATE_VERSION);
        let _ = state.created_at();
        let _ = state.saved_at();
    }

    #[test]
    fn test_deserialize_incompatible_version() {
        // Create JSON with an incompatible version
        let json = r#"{
            "metadata": {
                "version": 999,
                "created_at": "2024-01-01T00:00:00Z",
                "saved_at": "2024-01-01T00:00:00Z",
                "pid": 12345,
                "session_id": "test-session"
            },
            "loop_state": null,
            "task_tracker": null,
            "supervisor": {
                "mode_switch_count": 0,
                "last_error": null,
                "error_repeat_count": 0,
                "last_check_iteration": 0,
                "health_history": [],
                "last_verdict": null
            },
            "predictor": {
                "prediction_history": [],
                "config": null
            }
        }"#;

        // Should deserialize successfully (we can read the format)
        let state: SessionState = serde_json::from_str(json).expect("should parse");

        // But version check should fail
        assert!(!state.is_version_compatible());
        assert!(!SessionState::is_compatible_version(999));
    }
}
