//! Task state types and transitions.
//!
//! This module contains the core state machine types for task tracking:
//! - [`TaskState`] - Current state of a task
//! - [`BlockReason`] - Why a task is blocked
//! - [`TaskTransition`] - Record of state transitions

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::fmt;

// ============================================================================
// Task State
// ============================================================================

/// Current state of a task in the state machine.
///
/// # State Transitions
///
/// - `NotStarted` -> `InProgress`: Task selected for work
/// - `InProgress` -> `Blocked`: Task hit a blocker
/// - `InProgress` -> `InReview`: Task submitted for quality review
/// - `Blocked` -> `InProgress`: Blocker resolved
/// - `InReview` -> `InProgress`: Review failed, needs more work
/// - `InReview` -> `Complete`: Review passed
/// - `NotStarted` -> `Complete`: Task marked complete externally
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum TaskState {
    /// Task has not been started yet
    #[default]
    NotStarted,
    /// Task is currently being worked on
    InProgress,
    /// Task is blocked and cannot proceed
    Blocked,
    /// Task is submitted for quality gate review
    InReview,
    /// Task is complete
    Complete,
}

impl fmt::Display for TaskState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            TaskState::NotStarted => write!(f, "Not Started"),
            TaskState::InProgress => write!(f, "In Progress"),
            TaskState::Blocked => write!(f, "Blocked"),
            TaskState::InReview => write!(f, "In Review"),
            TaskState::Complete => write!(f, "Complete"),
        }
    }
}

impl TaskState {
    /// Check if this state can transition to the target state.
    ///
    /// # Example
    ///
    /// ```
    /// use ralph::r#loop::task_tracker::TaskState;
    ///
    /// assert!(TaskState::NotStarted.can_transition_to(TaskState::InProgress));
    /// assert!(!TaskState::Complete.can_transition_to(TaskState::InProgress));
    /// ```
    #[must_use]
    pub fn can_transition_to(&self, target: TaskState) -> bool {
        use TaskState::*;
        matches!(
            (self, target),
            // From NotStarted
            (NotStarted, InProgress) | (NotStarted, Complete) |
            // From InProgress
            (InProgress, Blocked) | (InProgress, InReview) | (InProgress, Complete) |
            // From Blocked
            (Blocked, InProgress) | (Blocked, Complete) |
            // From InReview
            (InReview, InProgress) | (InReview, Complete)
        )
    }

    /// Check if this state represents active work.
    #[must_use]
    pub fn is_active(&self) -> bool {
        matches!(self, TaskState::InProgress | TaskState::InReview)
    }

    /// Check if this state represents a terminal state.
    #[must_use]
    pub fn is_terminal(&self) -> bool {
        matches!(self, TaskState::Complete)
    }
}

// ============================================================================
// Block Reason
// ============================================================================

/// Reason why a task is blocked.
///
/// Used to provide context for debugging and to guide recovery strategies.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum BlockReason {
    /// Exceeded maximum retry attempts without progress
    MaxAttempts {
        /// Number of attempts made
        attempts: u32,
        /// Maximum allowed attempts
        max: u32,
    },
    /// Task timed out
    Timeout {
        /// Duration in seconds before timeout
        duration_secs: u64,
    },
    /// Waiting on an external dependency
    ExternalDependency {
        /// Description of the dependency
        description: String,
    },
    /// Quality gate failed repeatedly
    QualityGateFailure {
        /// Name of the failing gate
        gate: String,
        /// Number of consecutive failures
        failures: u32,
    },
    /// Task requires manual intervention
    ManualIntervention {
        /// Reason manual intervention is needed
        reason: String,
    },
    /// Blocked by another task
    DependsOnTask {
        /// ID of the blocking task
        task_number: u32,
    },
    /// Unknown or custom block reason
    Other {
        /// Description of the block reason
        reason: String,
    },
}

impl fmt::Display for BlockReason {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            BlockReason::MaxAttempts { attempts, max } => {
                write!(f, "Exceeded max attempts ({}/{})", attempts, max)
            }
            BlockReason::Timeout { duration_secs } => {
                write!(f, "Timed out after {} seconds", duration_secs)
            }
            BlockReason::ExternalDependency { description } => {
                write!(f, "External dependency: {}", description)
            }
            BlockReason::QualityGateFailure { gate, failures } => {
                write!(f, "Quality gate '{}' failed {} times", gate, failures)
            }
            BlockReason::ManualIntervention { reason } => {
                write!(f, "Manual intervention required: {}", reason)
            }
            BlockReason::DependsOnTask { task_number } => {
                write!(f, "Blocked by task #{}", task_number)
            }
            BlockReason::Other { reason } => {
                write!(f, "{}", reason)
            }
        }
    }
}

// ============================================================================
// Task Transition
// ============================================================================

/// Record of a state transition for a task.
///
/// Provides an audit trail of task progress.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskTransition {
    /// State before the transition
    pub from: TaskState,
    /// State after the transition
    pub to: TaskState,
    /// When the transition occurred
    pub timestamp: DateTime<Utc>,
    /// Optional reason for the transition
    pub reason: Option<String>,
    /// Optional block reason (only set when transitioning to Blocked)
    pub block_reason: Option<BlockReason>,
}

impl TaskTransition {
    /// Create a new transition record.
    ///
    /// # Example
    ///
    /// ```
    /// use ralph::r#loop::task_tracker::{TaskTransition, TaskState};
    ///
    /// let transition = TaskTransition::new(TaskState::NotStarted, TaskState::InProgress);
    /// assert_eq!(transition.from, TaskState::NotStarted);
    /// assert_eq!(transition.to, TaskState::InProgress);
    /// ```
    #[must_use]
    pub fn new(from: TaskState, to: TaskState) -> Self {
        Self {
            from,
            to,
            timestamp: Utc::now(),
            reason: None,
            block_reason: None,
        }
    }

    /// Create a transition with a reason.
    #[must_use]
    pub fn with_reason(from: TaskState, to: TaskState, reason: &str) -> Self {
        Self {
            from,
            to,
            timestamp: Utc::now(),
            reason: Some(reason.to_string()),
            block_reason: None,
        }
    }

    /// Create a blocking transition with a block reason.
    #[must_use]
    pub fn blocked(from: TaskState, block_reason: BlockReason) -> Self {
        Self {
            from,
            to: TaskState::Blocked,
            timestamp: Utc::now(),
            reason: Some(block_reason.to_string()),
            block_reason: Some(block_reason),
        }
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    // ========================================================================
    // TaskState Tests
    // ========================================================================

    #[test]
    fn test_task_state_default() {
        let state = TaskState::default();
        assert_eq!(state, TaskState::NotStarted);
    }

    #[test]
    fn test_task_state_display() {
        assert_eq!(TaskState::NotStarted.to_string(), "Not Started");
        assert_eq!(TaskState::InProgress.to_string(), "In Progress");
        assert_eq!(TaskState::Blocked.to_string(), "Blocked");
        assert_eq!(TaskState::InReview.to_string(), "In Review");
        assert_eq!(TaskState::Complete.to_string(), "Complete");
    }

    #[test]
    fn test_task_state_can_transition_from_not_started() {
        assert!(TaskState::NotStarted.can_transition_to(TaskState::InProgress));
        assert!(TaskState::NotStarted.can_transition_to(TaskState::Complete));
        assert!(!TaskState::NotStarted.can_transition_to(TaskState::Blocked));
        assert!(!TaskState::NotStarted.can_transition_to(TaskState::InReview));
    }

    #[test]
    fn test_task_state_can_transition_from_in_progress() {
        assert!(TaskState::InProgress.can_transition_to(TaskState::Blocked));
        assert!(TaskState::InProgress.can_transition_to(TaskState::InReview));
        assert!(TaskState::InProgress.can_transition_to(TaskState::Complete));
        assert!(!TaskState::InProgress.can_transition_to(TaskState::NotStarted));
    }

    #[test]
    fn test_task_state_can_transition_from_blocked() {
        assert!(TaskState::Blocked.can_transition_to(TaskState::InProgress));
        assert!(TaskState::Blocked.can_transition_to(TaskState::Complete));
        assert!(!TaskState::Blocked.can_transition_to(TaskState::NotStarted));
        assert!(!TaskState::Blocked.can_transition_to(TaskState::InReview));
    }

    #[test]
    fn test_task_state_can_transition_from_in_review() {
        assert!(TaskState::InReview.can_transition_to(TaskState::InProgress));
        assert!(TaskState::InReview.can_transition_to(TaskState::Complete));
        assert!(!TaskState::InReview.can_transition_to(TaskState::NotStarted));
        assert!(!TaskState::InReview.can_transition_to(TaskState::Blocked));
    }

    #[test]
    fn test_task_state_complete_is_terminal() {
        assert!(!TaskState::Complete.can_transition_to(TaskState::NotStarted));
        assert!(!TaskState::Complete.can_transition_to(TaskState::InProgress));
        assert!(!TaskState::Complete.can_transition_to(TaskState::Blocked));
        assert!(!TaskState::Complete.can_transition_to(TaskState::InReview));
    }

    #[test]
    fn test_task_state_is_active() {
        assert!(!TaskState::NotStarted.is_active());
        assert!(TaskState::InProgress.is_active());
        assert!(!TaskState::Blocked.is_active());
        assert!(TaskState::InReview.is_active());
        assert!(!TaskState::Complete.is_active());
    }

    #[test]
    fn test_task_state_is_terminal() {
        assert!(!TaskState::NotStarted.is_terminal());
        assert!(!TaskState::InProgress.is_terminal());
        assert!(!TaskState::Blocked.is_terminal());
        assert!(!TaskState::InReview.is_terminal());
        assert!(TaskState::Complete.is_terminal());
    }

    #[test]
    fn test_task_state_serialize() {
        let json = serde_json::to_string(&TaskState::InProgress).unwrap();
        assert_eq!(json, "\"InProgress\"");
    }

    #[test]
    fn test_task_state_deserialize() {
        let state: TaskState = serde_json::from_str("\"Blocked\"").unwrap();
        assert_eq!(state, TaskState::Blocked);
    }

    // ========================================================================
    // BlockReason Tests
    // ========================================================================

    #[test]
    fn test_block_reason_max_attempts_display() {
        let reason = BlockReason::MaxAttempts {
            attempts: 5,
            max: 5,
        };
        assert_eq!(reason.to_string(), "Exceeded max attempts (5/5)");
    }

    #[test]
    fn test_block_reason_timeout_display() {
        let reason = BlockReason::Timeout {
            duration_secs: 3600,
        };
        assert_eq!(reason.to_string(), "Timed out after 3600 seconds");
    }

    #[test]
    fn test_block_reason_external_dependency_display() {
        let reason = BlockReason::ExternalDependency {
            description: "API key needed".to_string(),
        };
        assert_eq!(reason.to_string(), "External dependency: API key needed");
    }

    #[test]
    fn test_block_reason_quality_gate_display() {
        let reason = BlockReason::QualityGateFailure {
            gate: "clippy".to_string(),
            failures: 3,
        };
        assert_eq!(reason.to_string(), "Quality gate 'clippy' failed 3 times");
    }

    #[test]
    fn test_block_reason_manual_intervention_display() {
        let reason = BlockReason::ManualIntervention {
            reason: "Need code review".to_string(),
        };
        assert_eq!(
            reason.to_string(),
            "Manual intervention required: Need code review"
        );
    }

    #[test]
    fn test_block_reason_depends_on_task_display() {
        let reason = BlockReason::DependsOnTask { task_number: 5 };
        assert_eq!(reason.to_string(), "Blocked by task #5");
    }

    #[test]
    fn test_block_reason_other_display() {
        let reason = BlockReason::Other {
            reason: "Custom reason".to_string(),
        };
        assert_eq!(reason.to_string(), "Custom reason");
    }

    #[test]
    fn test_block_reason_serialize() {
        let reason = BlockReason::MaxAttempts {
            attempts: 3,
            max: 5,
        };
        let json = serde_json::to_string(&reason).unwrap();
        assert!(json.contains("MaxAttempts"));
        assert!(json.contains("\"attempts\":3"));
        assert!(json.contains("\"max\":5"));
    }

    #[test]
    fn test_block_reason_deserialize() {
        let json = r#"{"Timeout":{"duration_secs":1800}}"#;
        let reason: BlockReason = serde_json::from_str(json).unwrap();
        assert!(matches!(
            reason,
            BlockReason::Timeout {
                duration_secs: 1800
            }
        ));
    }

    // ========================================================================
    // TaskTransition Tests
    // ========================================================================

    #[test]
    fn test_task_transition_new() {
        let transition = TaskTransition::new(TaskState::NotStarted, TaskState::InProgress);
        assert_eq!(transition.from, TaskState::NotStarted);
        assert_eq!(transition.to, TaskState::InProgress);
        assert!(transition.reason.is_none());
        assert!(transition.block_reason.is_none());
    }

    #[test]
    fn test_task_transition_with_reason() {
        let transition =
            TaskTransition::with_reason(TaskState::InProgress, TaskState::Complete, "All done");
        assert_eq!(transition.from, TaskState::InProgress);
        assert_eq!(transition.to, TaskState::Complete);
        assert_eq!(transition.reason, Some("All done".to_string()));
    }

    #[test]
    fn test_task_transition_blocked() {
        let block_reason = BlockReason::MaxAttempts {
            attempts: 5,
            max: 5,
        };
        let transition = TaskTransition::blocked(TaskState::InProgress, block_reason.clone());
        assert_eq!(transition.from, TaskState::InProgress);
        assert_eq!(transition.to, TaskState::Blocked);
        assert!(transition.reason.is_some());
        assert_eq!(transition.block_reason, Some(block_reason));
    }

    #[test]
    fn test_task_transition_timestamp_is_recent() {
        let before = Utc::now();
        let transition = TaskTransition::new(TaskState::NotStarted, TaskState::InProgress);
        let after = Utc::now();

        assert!(transition.timestamp >= before);
        assert!(transition.timestamp <= after);
    }

    #[test]
    fn test_task_transition_serialize() {
        let transition = TaskTransition::new(TaskState::NotStarted, TaskState::InProgress);
        let json = serde_json::to_string(&transition).unwrap();
        assert!(json.contains("\"from\":\"NotStarted\""));
        assert!(json.contains("\"to\":\"InProgress\""));
    }

    #[test]
    fn test_task_transition_deserialize() {
        let json = r#"{"from":"InProgress","to":"Complete","timestamp":"2024-01-01T00:00:00Z","reason":"Done","block_reason":null}"#;
        let transition: TaskTransition = serde_json::from_str(json).unwrap();
        assert_eq!(transition.from, TaskState::InProgress);
        assert_eq!(transition.to, TaskState::Complete);
        assert_eq!(transition.reason, Some("Done".to_string()));
    }
}
