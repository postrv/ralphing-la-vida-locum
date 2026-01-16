//! Loop state types and transitions.
//!
//! This module defines the state types used by the loop manager
//! to track execution progress and mode.

use chrono::Utc;
use clap::ValueEnum;
use serde::{Deserialize, Serialize};

/// Loop execution mode.
///
/// Determines which prompt file is used and how the loop behaves.
///
/// # Example
///
/// ```
/// use ralph::r#loop::state::LoopMode;
///
/// let mode = LoopMode::Build;
/// assert_eq!(mode.to_string(), "build");
/// ```
#[derive(Debug, Clone, Copy, ValueEnum, PartialEq, Eq, Serialize, Deserialize)]
pub enum LoopMode {
    /// Planning phase - create implementation plan
    Plan,
    /// Build phase - implement tasks
    Build,
    /// Debug phase - focus on blockers
    Debug,
}

impl std::fmt::Display for LoopMode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            LoopMode::Plan => write!(f, "plan"),
            LoopMode::Build => write!(f, "build"),
            LoopMode::Debug => write!(f, "debug"),
        }
    }
}

impl LoopMode {
    /// Get the prompt filename for this mode.
    ///
    /// # Example
    ///
    /// ```
    /// use ralph::r#loop::state::LoopMode;
    ///
    /// assert_eq!(LoopMode::Build.prompt_filename(), "PROMPT_build.md");
    /// ```
    #[must_use]
    pub fn prompt_filename(&self) -> String {
        format!("PROMPT_{}.md", self)
    }
}

/// State of the automation loop.
///
/// Tracks iteration count, stagnation, and progress markers.
///
/// # Example
///
/// ```
/// use ralph::r#loop::state::{LoopState, LoopMode};
///
/// let state = LoopState::new(LoopMode::Build);
/// assert_eq!(state.iteration, 0);
/// assert_eq!(state.mode, LoopMode::Build);
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoopState {
    /// Current iteration number (1-indexed after first run)
    pub iteration: u32,
    /// Number of consecutive iterations without progress
    pub stagnation_count: u32,
    /// MD5 hash of IMPLEMENTATION_PLAN.md at last check
    pub last_plan_hash: String,
    /// Git commit hash at last check
    pub last_commit_hash: String,
    /// Cumulative lines changed (for triggering analysis)
    pub cumulative_changes: u32,
    /// Current execution mode
    pub mode: LoopMode,
    /// Unique session identifier
    pub session_id: String,
}

impl LoopState {
    /// Create a new loop state with the given mode.
    ///
    /// # Example
    ///
    /// ```
    /// use ralph::r#loop::state::{LoopState, LoopMode};
    ///
    /// let state = LoopState::new(LoopMode::Plan);
    /// assert_eq!(state.mode, LoopMode::Plan);
    /// assert_eq!(state.stagnation_count, 0);
    /// ```
    #[must_use]
    pub fn new(mode: LoopMode) -> Self {
        Self {
            iteration: 0,
            stagnation_count: 0,
            last_plan_hash: String::new(),
            last_commit_hash: String::new(),
            cumulative_changes: 0,
            mode,
            session_id: Utc::now().timestamp().to_string(),
        }
    }

    /// Record progress detected (reset stagnation).
    pub fn record_progress(&mut self) {
        self.stagnation_count = 0;
    }

    /// Record no progress detected (increment stagnation).
    pub fn record_no_progress(&mut self) {
        self.stagnation_count += 1;
    }

    /// Check if stagnation threshold has been reached.
    #[must_use]
    pub fn is_stagnating(&self, threshold: u32) -> bool {
        self.stagnation_count >= threshold
    }

    /// Increment iteration counter.
    pub fn next_iteration(&mut self) {
        self.iteration += 1;
    }

    /// Switch to the given mode.
    pub fn switch_mode(&mut self, mode: LoopMode) {
        self.mode = mode;
    }

    /// Update the plan hash for progress tracking.
    pub fn update_plan_hash(&mut self, hash: String) {
        self.last_plan_hash = hash;
    }

    /// Update the commit hash for progress tracking.
    pub fn update_commit_hash(&mut self, hash: String) {
        self.last_commit_hash = hash;
    }
}

impl Default for LoopState {
    fn default() -> Self {
        Self::new(LoopMode::Build)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_loop_state_new() {
        let state = LoopState::new(LoopMode::Plan);
        assert_eq!(state.mode, LoopMode::Plan);
        assert_eq!(state.iteration, 0);
        assert_eq!(state.stagnation_count, 0);
        assert!(state.last_plan_hash.is_empty());
        assert!(state.last_commit_hash.is_empty());
    }

    #[test]
    fn test_loop_mode_display() {
        assert_eq!(LoopMode::Plan.to_string(), "plan");
        assert_eq!(LoopMode::Build.to_string(), "build");
        assert_eq!(LoopMode::Debug.to_string(), "debug");
    }

    #[test]
    fn test_loop_mode_prompt_filename() {
        assert_eq!(LoopMode::Build.prompt_filename(), "PROMPT_build.md");
        assert_eq!(LoopMode::Debug.prompt_filename(), "PROMPT_debug.md");
        assert_eq!(LoopMode::Plan.prompt_filename(), "PROMPT_plan.md");
    }

    #[test]
    fn test_loop_state_default() {
        let state = LoopState::default();
        assert_eq!(state.mode, LoopMode::Build);
        assert_eq!(state.iteration, 0);
    }

    #[test]
    fn test_record_progress() {
        let mut state = LoopState::new(LoopMode::Build);
        state.stagnation_count = 5;
        state.record_progress();
        assert_eq!(state.stagnation_count, 0);
    }

    #[test]
    fn test_record_no_progress() {
        let mut state = LoopState::new(LoopMode::Build);
        state.record_no_progress();
        state.record_no_progress();
        assert_eq!(state.stagnation_count, 2);
    }

    #[test]
    fn test_is_stagnating() {
        let mut state = LoopState::new(LoopMode::Build);
        assert!(!state.is_stagnating(3));

        state.stagnation_count = 3;
        assert!(state.is_stagnating(3));

        state.stagnation_count = 5;
        assert!(state.is_stagnating(3));
    }

    #[test]
    fn test_next_iteration() {
        let mut state = LoopState::new(LoopMode::Build);
        assert_eq!(state.iteration, 0);

        state.next_iteration();
        assert_eq!(state.iteration, 1);

        state.next_iteration();
        assert_eq!(state.iteration, 2);
    }

    #[test]
    fn test_switch_mode() {
        let mut state = LoopState::new(LoopMode::Build);
        state.switch_mode(LoopMode::Debug);
        assert_eq!(state.mode, LoopMode::Debug);
    }

    #[test]
    fn test_update_hashes() {
        let mut state = LoopState::new(LoopMode::Build);

        state.update_plan_hash("abc123".to_string());
        assert_eq!(state.last_plan_hash, "abc123");

        state.update_commit_hash("def456".to_string());
        assert_eq!(state.last_commit_hash, "def456");
    }

    #[test]
    fn test_loop_state_serialize() {
        let state = LoopState::new(LoopMode::Build);
        let json = serde_json::to_string(&state).unwrap();
        assert!(json.contains("\"mode\":\"Build\""));
        assert!(json.contains("\"iteration\":0"));
    }

    #[test]
    fn test_loop_state_deserialize() {
        let json = r#"{
            "iteration": 5,
            "stagnation_count": 2,
            "last_plan_hash": "abc",
            "last_commit_hash": "def",
            "cumulative_changes": 100,
            "mode": "Debug",
            "session_id": "12345"
        }"#;

        let state: LoopState = serde_json::from_str(json).unwrap();
        assert_eq!(state.iteration, 5);
        assert_eq!(state.stagnation_count, 2);
        assert_eq!(state.mode, LoopMode::Debug);
    }
}
