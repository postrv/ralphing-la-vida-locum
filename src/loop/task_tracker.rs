//! Task-level state machine and tracking.
//!
//! This module provides fine-grained progress tracking at the individual task level,
//! enabling intelligent task selection and stagnation detection.
//!
//! # Architecture
//!
//! The task tracker maintains state for each task parsed from IMPLEMENTATION_PLAN.md:
//!
//! ```text
//! TaskTracker
//!   ├── tasks: HashMap<TaskId, Task>
//!   ├── current_task: Option<TaskId>
//!   └── config: TaskTrackerConfig
//!
//! Task
//!   ├── id: TaskId
//!   ├── state: TaskState
//!   ├── transitions: Vec<TaskTransition>
//!   └── metrics: TaskMetrics
//! ```
//!
//! # State Transitions
//!
//! ```text
//! NotStarted ──start──> InProgress ──block──> Blocked
//!     │                      │                   │
//!     │                      │ submit            │ unblock
//!     │                      ▼                   │
//!     │                  InReview ───────────────┘
//!     │                      │
//!     │                      │ complete
//!     │                      ▼
//!     └──────────────────> Complete
//! ```

use anyhow::{bail, Context, Result};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fmt;
use std::hash::Hash;

// ============================================================================
// Task Identifier
// ============================================================================

/// Unique identifier for a task parsed from the plan.
///
/// Task IDs are derived from plan section headers (e.g., "### 2. Phase 1.1: Task Domain Model").
///
/// # Example
///
/// ```
/// use ralph::r#loop::task_tracker::TaskId;
///
/// let id = TaskId::parse("### 2. Phase 1.1: Task Domain Model").unwrap();
/// assert_eq!(id.number(), 2);
/// assert_eq!(id.phase(), Some("1.1"));
/// assert_eq!(id.title(), "Task Domain Model");
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct TaskId {
    /// Task number from the plan (e.g., 2 for "### 2. Phase 1.1...")
    number: u32,
    /// Phase identifier if present (e.g., "1.1")
    phase: Option<String>,
    /// Task title without the number/phase prefix
    title: String,
    /// Original header text for display
    original: String,
}

impl TaskId {
    /// Parse a task ID from a plan header line.
    ///
    /// Supports formats:
    /// - `### 1. Task Name`
    /// - `### 2. Phase 1.1: Task Name`
    /// - `### 3. Phase 1.2: Task Name (with details)`
    ///
    /// # Errors
    ///
    /// Returns an error if the header doesn't match expected patterns.
    ///
    /// # Example
    ///
    /// ```
    /// use ralph::r#loop::task_tracker::TaskId;
    ///
    /// // Simple format
    /// let id = TaskId::parse("### 1. Setup project").unwrap();
    /// assert_eq!(id.number(), 1);
    /// assert_eq!(id.title(), "Setup project");
    ///
    /// // With phase
    /// let id = TaskId::parse("### 5. Phase 2.3: Build templates").unwrap();
    /// assert_eq!(id.number(), 5);
    /// assert_eq!(id.phase(), Some("2.3"));
    /// assert_eq!(id.title(), "Build templates");
    /// ```
    pub fn parse(header: &str) -> Result<Self> {
        // Strip leading hashes and whitespace
        let stripped = header.trim_start_matches('#').trim();

        // Pattern: "N. [Phase X.Y: ]Title"
        let parts: Vec<&str> = stripped.splitn(2, ". ").collect();
        if parts.len() != 2 {
            bail!("Invalid task header format: expected 'N. Title', got: {}", header);
        }

        let number: u32 = parts[0]
            .parse()
            .with_context(|| format!("Invalid task number in header: {}", header))?;

        let rest = parts[1];

        // Check for "Phase X.Y: " prefix
        let (phase, title) = if rest.starts_with("Phase ") {
            // Find the colon that separates phase from title
            if let Some(colon_pos) = rest.find(": ") {
                let phase_part = &rest[6..colon_pos]; // Skip "Phase "
                let title_part = rest[colon_pos + 2..].trim();
                (Some(phase_part.to_string()), title_part.to_string())
            } else {
                (None, rest.to_string())
            }
        } else {
            (None, rest.to_string())
        };

        Ok(Self {
            number,
            phase,
            title,
            original: header.to_string(),
        })
    }

    /// Create a task ID directly for testing.
    #[cfg(test)]
    pub fn new_for_test(number: u32, title: &str) -> Self {
        Self {
            number,
            phase: None,
            title: title.to_string(),
            original: format!("### {}. {}", number, title),
        }
    }

    /// Get the task number.
    #[must_use]
    pub fn number(&self) -> u32 {
        self.number
    }

    /// Get the phase identifier if present.
    #[must_use]
    pub fn phase(&self) -> Option<&str> {
        self.phase.as_deref()
    }

    /// Get the task title.
    #[must_use]
    pub fn title(&self) -> &str {
        &self.title
    }

    /// Get the original header text.
    #[must_use]
    pub fn original(&self) -> &str {
        &self.original
    }
}

impl fmt::Display for TaskId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if let Some(ref phase) = self.phase {
            write!(f, "Task {}: Phase {}: {}", self.number, phase, self.title)
        } else {
            write!(f, "Task {}: {}", self.number, self.title)
        }
    }
}

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
// Task Tracker Configuration
// ============================================================================

/// Configuration for the task tracker.
///
/// Controls thresholds and behavior for task state management.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskTrackerConfig {
    /// Maximum attempts before blocking a task
    pub max_attempts_per_task: u32,
    /// Timeout in seconds before blocking a task
    pub task_timeout_secs: u64,
    /// Number of consecutive no-progress iterations before blocking
    pub stagnation_threshold: u32,
    /// Maximum number of quality gate failures before blocking
    pub max_quality_failures: u32,
    /// Whether to auto-save state after each change
    pub auto_save: bool,
}

impl Default for TaskTrackerConfig {
    fn default() -> Self {
        Self {
            max_attempts_per_task: 5,
            task_timeout_secs: 3600, // 1 hour
            stagnation_threshold: 3,
            max_quality_failures: 3,
            auto_save: true,
        }
    }
}

impl TaskTrackerConfig {
    /// Create a new configuration with default values.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Set the maximum attempts per task.
    #[must_use]
    pub fn with_max_attempts(mut self, max: u32) -> Self {
        self.max_attempts_per_task = max;
        self
    }

    /// Set the task timeout in seconds.
    #[must_use]
    pub fn with_timeout_secs(mut self, secs: u64) -> Self {
        self.task_timeout_secs = secs;
        self
    }

    /// Set the stagnation threshold.
    #[must_use]
    pub fn with_stagnation_threshold(mut self, threshold: u32) -> Self {
        self.stagnation_threshold = threshold;
        self
    }

    /// Set the maximum quality gate failures.
    #[must_use]
    pub fn with_max_quality_failures(mut self, max: u32) -> Self {
        self.max_quality_failures = max;
        self
    }

    /// Disable auto-save (useful for testing).
    #[must_use]
    pub fn without_auto_save(mut self) -> Self {
        self.auto_save = false;
        self
    }

    /// Set auto-save behavior.
    #[must_use]
    pub fn with_auto_save(mut self, enabled: bool) -> Self {
        self.auto_save = enabled;
        self
    }
}

// ============================================================================
// Task Metrics
// ============================================================================

/// Metrics tracked for each task.
///
/// Used for progress evaluation and stagnation detection.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct TaskMetrics {
    /// Number of iterations spent on this task
    pub iterations: u32,
    /// Number of commits made while working on this task
    pub commits: u32,
    /// Number of files modified
    pub files_modified: u32,
    /// Consecutive iterations without progress
    pub no_progress_count: u32,
    /// Number of quality gate check failures
    pub quality_failures: u32,
    /// Total lines changed
    pub lines_changed: u32,
}

impl TaskMetrics {
    /// Create new empty metrics.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Record an iteration.
    pub fn record_iteration(&mut self) {
        self.iterations += 1;
    }

    /// Record a commit.
    pub fn record_commit(&mut self) {
        self.commits += 1;
    }

    /// Record progress (resets no_progress_count).
    pub fn record_progress(&mut self, files: u32, lines: u32) {
        self.files_modified += files;
        self.lines_changed += lines;
        self.no_progress_count = 0;
    }

    /// Record no progress.
    pub fn record_no_progress(&mut self) {
        self.no_progress_count += 1;
    }

    /// Record a quality gate failure.
    pub fn record_quality_failure(&mut self) {
        self.quality_failures += 1;
    }

    /// Reset quality failure count (after passing).
    pub fn reset_quality_failures(&mut self) {
        self.quality_failures = 0;
    }
}

// ============================================================================
// Task
// ============================================================================

/// A tracked task from the implementation plan.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Task {
    /// Unique identifier
    pub id: TaskId,
    /// Current state
    pub state: TaskState,
    /// Transition history
    pub transitions: Vec<TaskTransition>,
    /// Performance metrics
    pub metrics: TaskMetrics,
    /// Block reason if blocked
    pub block_reason: Option<BlockReason>,
    /// Subtasks/checkboxes (line content, checked status)
    pub checkboxes: Vec<(String, bool)>,
}

impl Task {
    /// Create a new task with the given ID.
    #[must_use]
    pub fn new(id: TaskId) -> Self {
        Self {
            id,
            state: TaskState::NotStarted,
            transitions: Vec::new(),
            metrics: TaskMetrics::new(),
            block_reason: None,
            checkboxes: Vec::new(),
        }
    }

    /// Get the completion percentage based on checkboxes.
    ///
    /// Returns 0.0 if there are no checkboxes.
    #[must_use]
    pub fn completion_percentage(&self) -> f64 {
        if self.checkboxes.is_empty() {
            return 0.0;
        }
        let checked = self.checkboxes.iter().filter(|(_, c)| *c).count();
        (checked as f64 / self.checkboxes.len() as f64) * 100.0
    }

    /// Check if the task is complete.
    #[must_use]
    pub fn is_complete(&self) -> bool {
        self.state == TaskState::Complete
    }

    /// Check if the task is blocked.
    #[must_use]
    pub fn is_blocked(&self) -> bool {
        self.state == TaskState::Blocked
    }

    /// Check if the task can be worked on.
    #[must_use]
    pub fn is_workable(&self) -> bool {
        matches!(self.state, TaskState::NotStarted | TaskState::InProgress)
    }
}

// ============================================================================
// Task Tracker (stub for Phase 1.2+)
// ============================================================================

/// Task-level progress tracker.
///
/// Maintains state for all tasks and provides task selection and progress tracking.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskTracker {
    /// All tracked tasks keyed by ID
    #[serde(with = "tasks_serde")]
    pub tasks: HashMap<TaskId, Task>,
    /// Currently active task
    pub current_task: Option<TaskId>,
    /// Configuration
    pub config: TaskTrackerConfig,
    /// When the tracker was created
    pub created_at: DateTime<Utc>,
    /// When the tracker was last modified
    pub modified_at: DateTime<Utc>,
}

/// Custom serialization for HashMap<TaskId, Task> as Vec<Task>
mod tasks_serde {
    use super::*;
    use serde::{Deserializer, Serializer};

    pub fn serialize<S>(tasks: &HashMap<TaskId, Task>, serializer: S) -> std::result::Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let tasks_vec: Vec<&Task> = tasks.values().collect();
        tasks_vec.serialize(serializer)
    }

    pub fn deserialize<'de, D>(deserializer: D) -> std::result::Result<HashMap<TaskId, Task>, D::Error>
    where
        D: Deserializer<'de>,
    {
        let tasks_vec: Vec<Task> = Vec::deserialize(deserializer)?;
        Ok(tasks_vec.into_iter().map(|t| (t.id.clone(), t)).collect())
    }
}

impl TaskTracker {
    /// Create a new task tracker with the given configuration.
    #[must_use]
    pub fn new(config: TaskTrackerConfig) -> Self {
        let now = Utc::now();
        Self {
            tasks: HashMap::new(),
            current_task: None,
            config,
            created_at: now,
            modified_at: now,
        }
    }

    /// Parse tasks from an implementation plan.
    ///
    /// Extracts task headers (### N. Title) and checkboxes (- [ ] or - [x])
    /// from markdown content. Updates existing tasks without losing state.
    ///
    /// # Example
    ///
    /// ```
    /// use ralph::r#loop::task_tracker::{TaskTracker, TaskTrackerConfig};
    ///
    /// let mut tracker = TaskTracker::new(TaskTrackerConfig::default());
    /// let plan = r#"
    /// ## Tasks
    ///
    /// ### 1. Setup project
    /// - [x] Create directory structure
    /// - [ ] Initialize git repo
    ///
    /// ### 2. Phase 1.1: Implement feature
    /// - [ ] Write tests
    /// - [ ] Write implementation
    /// "#;
    ///
    /// tracker.parse_plan(plan).unwrap();
    /// assert_eq!(tracker.tasks.len(), 2);
    /// ```
    pub fn parse_plan(&mut self, content: &str) -> Result<()> {
        use regex::Regex;

        // Task header pattern: ### N. [Phase X.Y: ]Title
        let header_re = Regex::new(r"^###\s+(\d+)\.\s+(.+)$")
            .context("Failed to compile header regex")?;

        // Checkbox pattern: - [ ] or - [x] followed by text
        let checkbox_re = Regex::new(r"^-\s+\[([ xX])\]\s+(.+)$")
            .context("Failed to compile checkbox regex")?;

        let mut current_task: Option<TaskId> = None;
        let mut current_checkboxes: Vec<(String, bool)> = Vec::new();

        for line in content.lines() {
            let trimmed = line.trim();

            // Check for task header
            if header_re.is_match(trimmed) {
                // Save checkboxes for previous task
                if let Some(ref task_id) = current_task {
                    self.update_task_checkboxes(task_id, &current_checkboxes);
                }
                current_checkboxes.clear();

                // Parse new task header
                if let Ok(task_id) = TaskId::parse(trimmed) {
                    current_task = Some(task_id.clone());

                    // Insert task if it doesn't exist
                    if !self.tasks.contains_key(&task_id) {
                        self.tasks.insert(task_id.clone(), Task::new(task_id));
                    }
                }
            }
            // Check for checkbox under current task
            else if let Some(caps) = checkbox_re.captures(trimmed) {
                if current_task.is_some() {
                    let checked = &caps[1] == "x" || &caps[1] == "X";
                    let text = caps[2].to_string();
                    current_checkboxes.push((text, checked));
                }
            }
        }

        // Save checkboxes for last task
        if let Some(ref task_id) = current_task {
            self.update_task_checkboxes(task_id, &current_checkboxes);
        }

        self.modified_at = Utc::now();
        Ok(())
    }

    /// Update checkboxes for a task without losing other state.
    fn update_task_checkboxes(&mut self, task_id: &TaskId, checkboxes: &[(String, bool)]) {
        if let Some(task) = self.tasks.get_mut(task_id) {
            task.checkboxes = checkboxes.to_vec();
        }
    }

    /// Get tasks that are ready to be worked on.
    ///
    /// Returns tasks in order of priority:
    /// 1. InProgress tasks (resume what we started)
    /// 2. NotStarted tasks in order
    #[must_use]
    pub fn workable_tasks(&self) -> Vec<&Task> {
        let mut in_progress: Vec<&Task> = self
            .tasks
            .values()
            .filter(|t| t.state == TaskState::InProgress)
            .collect();

        let mut not_started: Vec<&Task> = self
            .tasks
            .values()
            .filter(|t| t.state == TaskState::NotStarted)
            .collect();

        // Sort by task number
        in_progress.sort_by_key(|t| t.id.number());
        not_started.sort_by_key(|t| t.id.number());

        // Combine: in-progress first, then not started
        in_progress.extend(not_started);
        in_progress
    }

    /// Get count of tasks by state.
    #[must_use]
    pub fn task_counts(&self) -> TaskCounts {
        let mut counts = TaskCounts::default();
        for task in self.tasks.values() {
            match task.state {
                TaskState::NotStarted => counts.not_started += 1,
                TaskState::InProgress => counts.in_progress += 1,
                TaskState::Blocked => counts.blocked += 1,
                TaskState::InReview => counts.in_review += 1,
                TaskState::Complete => counts.complete += 1,
            }
        }
        counts
    }

    /// Get total completion percentage across all tasks.
    ///
    /// Weighted by checkbox completion per task.
    #[must_use]
    pub fn overall_completion(&self) -> f64 {
        if self.tasks.is_empty() {
            return 0.0;
        }

        // Count completed tasks as 100%, others by their checkbox progress
        let total: f64 = self
            .tasks
            .values()
            .map(|t| {
                if t.state == TaskState::Complete {
                    100.0
                } else {
                    t.completion_percentage()
                }
            })
            .sum();

        total / self.tasks.len() as f64
    }

    /// Get a task by its ID.
    #[must_use]
    pub fn get_task(&self, id: &TaskId) -> Option<&Task> {
        self.tasks.get(id)
    }

    /// Get a mutable task by its ID.
    pub fn get_task_mut(&mut self, id: &TaskId) -> Option<&mut Task> {
        self.tasks.get_mut(id)
    }

    /// Get a task by its number.
    #[must_use]
    pub fn get_task_by_number(&self, number: u32) -> Option<&Task> {
        self.tasks.values().find(|t| t.id.number() == number)
    }

    // ========================================================================
    // State Machine Operations
    // ========================================================================

    /// Start working on a task.
    ///
    /// Transitions task from NotStarted to InProgress.
    /// Also sets current_task if no task is currently active.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - Task doesn't exist
    /// - Task is not in a state that can transition to InProgress
    ///
    /// # Example
    ///
    /// ```
    /// use ralph::r#loop::task_tracker::{TaskTracker, TaskTrackerConfig, TaskId, TaskState};
    ///
    /// let mut tracker = TaskTracker::new(TaskTrackerConfig::default());
    /// tracker.parse_plan("### 1. My task\n- [ ] Item").unwrap();
    ///
    /// let task_id = TaskId::parse("### 1. My task").unwrap();
    /// tracker.start_task(&task_id).unwrap();
    ///
    /// let task = tracker.get_task(&task_id).unwrap();
    /// assert_eq!(task.state, TaskState::InProgress);
    /// ```
    pub fn start_task(&mut self, task_id: &TaskId) -> Result<()> {
        let task = self.tasks.get_mut(task_id).ok_or_else(|| {
            anyhow::anyhow!("Task not found: {}", task_id)
        })?;

        if !task.state.can_transition_to(TaskState::InProgress) {
            bail!(
                "Cannot start task {}: current state is {}",
                task_id,
                task.state
            );
        }

        let transition = TaskTransition::with_reason(
            task.state,
            TaskState::InProgress,
            "Task started",
        );
        task.transitions.push(transition);
        task.state = TaskState::InProgress;

        // Set as current task if none is set
        if self.current_task.is_none() {
            self.current_task = Some(task_id.clone());
        }

        self.modified_at = Utc::now();
        Ok(())
    }

    /// Record progress on the current task.
    ///
    /// Updates task metrics with files modified and lines changed.
    /// Resets the no-progress counter.
    ///
    /// # Errors
    ///
    /// Returns an error if no task is currently active.
    pub fn record_progress(&mut self, files: u32, lines: u32) -> Result<()> {
        let task_id = self.current_task.clone().ok_or_else(|| {
            anyhow::anyhow!("No active task to record progress for")
        })?;

        let task = self.tasks.get_mut(&task_id).ok_or_else(|| {
            anyhow::anyhow!("Current task not found: {}", task_id)
        })?;

        task.metrics.record_progress(files, lines);
        self.modified_at = Utc::now();
        Ok(())
    }

    /// Record no progress on the current task.
    ///
    /// Increments the no-progress counter. If threshold is exceeded,
    /// blocks the task automatically.
    ///
    /// # Errors
    ///
    /// Returns an error if no task is currently active.
    pub fn record_no_progress(&mut self) -> Result<bool> {
        let task_id = self.current_task.clone().ok_or_else(|| {
            anyhow::anyhow!("No active task to record no-progress for")
        })?;

        let threshold = self.config.stagnation_threshold;

        let task = self.tasks.get_mut(&task_id).ok_or_else(|| {
            anyhow::anyhow!("Current task not found: {}", task_id)
        })?;

        task.metrics.record_no_progress();
        task.metrics.record_iteration();

        let should_block = task.metrics.no_progress_count >= threshold;
        self.modified_at = Utc::now();

        if should_block {
            // Auto-block the task - borrow of `task` ends after extracting the count
            let attempts = task.metrics.no_progress_count;
            let block_reason = BlockReason::MaxAttempts {
                attempts,
                max: threshold,
            };
            self.block_task(&task_id, block_reason)?;
        }

        Ok(should_block)
    }

    /// Record an iteration on the current task without progress determination.
    ///
    /// # Errors
    ///
    /// Returns an error if no task is currently active.
    pub fn record_iteration(&mut self) -> Result<()> {
        let task_id = self.current_task.clone().ok_or_else(|| {
            anyhow::anyhow!("No active task")
        })?;

        let task = self.tasks.get_mut(&task_id).ok_or_else(|| {
            anyhow::anyhow!("Current task not found: {}", task_id)
        })?;

        task.metrics.record_iteration();
        self.modified_at = Utc::now();
        Ok(())
    }

    /// Block a task.
    ///
    /// Transitions task to Blocked state with a reason.
    /// Clears current_task if it was the blocked task.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - Task doesn't exist
    /// - Task cannot transition to Blocked
    pub fn block_task(&mut self, task_id: &TaskId, reason: BlockReason) -> Result<()> {
        let task = self.tasks.get_mut(task_id).ok_or_else(|| {
            anyhow::anyhow!("Task not found: {}", task_id)
        })?;

        if !task.state.can_transition_to(TaskState::Blocked) {
            bail!(
                "Cannot block task {}: current state is {}",
                task_id,
                task.state
            );
        }

        let transition = TaskTransition::blocked(task.state, reason.clone());
        task.transitions.push(transition);
        task.state = TaskState::Blocked;
        task.block_reason = Some(reason);

        // Clear current task if this was it
        if self.current_task.as_ref() == Some(task_id) {
            self.current_task = None;
        }

        self.modified_at = Utc::now();
        Ok(())
    }

    /// Unblock a task.
    ///
    /// Transitions task from Blocked back to InProgress.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - Task doesn't exist
    /// - Task is not blocked
    pub fn unblock_task(&mut self, task_id: &TaskId) -> Result<()> {
        let task = self.tasks.get_mut(task_id).ok_or_else(|| {
            anyhow::anyhow!("Task not found: {}", task_id)
        })?;

        if task.state != TaskState::Blocked {
            bail!("Task {} is not blocked (state: {})", task_id, task.state);
        }

        let transition = TaskTransition::with_reason(
            TaskState::Blocked,
            TaskState::InProgress,
            "Task unblocked",
        );
        task.transitions.push(transition);
        task.state = TaskState::InProgress;
        task.block_reason = None;
        task.metrics.no_progress_count = 0; // Reset counter

        self.modified_at = Utc::now();
        Ok(())
    }

    /// Submit task for quality review.
    ///
    /// Transitions task from InProgress to InReview.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - Task doesn't exist
    /// - Task is not InProgress
    pub fn submit_for_review(&mut self, task_id: &TaskId) -> Result<()> {
        let task = self.tasks.get_mut(task_id).ok_or_else(|| {
            anyhow::anyhow!("Task not found: {}", task_id)
        })?;

        if task.state != TaskState::InProgress {
            bail!(
                "Cannot submit task {} for review: current state is {}",
                task_id,
                task.state
            );
        }

        let transition = TaskTransition::with_reason(
            TaskState::InProgress,
            TaskState::InReview,
            "Submitted for review",
        );
        task.transitions.push(transition);
        task.state = TaskState::InReview;

        self.modified_at = Utc::now();
        Ok(())
    }

    /// Update review status for a task.
    ///
    /// If gate passed, resets quality failures. If failed, increments.
    /// May auto-block if max quality failures exceeded.
    ///
    /// # Errors
    ///
    /// Returns an error if task doesn't exist or is not InReview.
    pub fn update_review(&mut self, task_id: &TaskId, gate: &str, passed: bool) -> Result<bool> {
        let max_failures = self.config.max_quality_failures;

        let task = self.tasks.get_mut(task_id).ok_or_else(|| {
            anyhow::anyhow!("Task not found: {}", task_id)
        })?;

        if task.state != TaskState::InReview {
            bail!("Task {} is not in review (state: {})", task_id, task.state);
        }

        if passed {
            task.metrics.reset_quality_failures();
            self.modified_at = Utc::now();
            return Ok(false);
        }

        // Gate failed
        task.metrics.record_quality_failure();
        let failures = task.metrics.quality_failures;
        let should_block = failures >= max_failures;

        if should_block {
            // Auto-block due to quality failures
            let block_reason = BlockReason::QualityGateFailure {
                gate: gate.to_string(),
                failures,
            };
            let transition = TaskTransition::blocked(TaskState::InReview, block_reason.clone());
            task.transitions.push(transition);
            task.state = TaskState::Blocked;
            task.block_reason = Some(block_reason);

            // Clear current task if this was it
            if self.current_task.as_ref() == Some(task_id) {
                self.current_task = None;
            }
        } else {
            // Return to InProgress to try again
            let transition = TaskTransition::with_reason(
                TaskState::InReview,
                TaskState::InProgress,
                &format!("Review failed ({}), retrying", gate),
            );
            task.transitions.push(transition);
            task.state = TaskState::InProgress;
        }

        self.modified_at = Utc::now();
        Ok(should_block)
    }

    /// Complete a task.
    ///
    /// Transitions task to Complete state.
    /// Clears current_task if it was this task.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - Task doesn't exist
    /// - Task cannot transition to Complete
    pub fn complete_task(&mut self, task_id: &TaskId) -> Result<()> {
        let task = self.tasks.get_mut(task_id).ok_or_else(|| {
            anyhow::anyhow!("Task not found: {}", task_id)
        })?;

        if !task.state.can_transition_to(TaskState::Complete) {
            bail!(
                "Cannot complete task {}: current state is {}",
                task_id,
                task.state
            );
        }

        let transition = TaskTransition::with_reason(
            task.state,
            TaskState::Complete,
            "Task completed",
        );
        task.transitions.push(transition);
        task.state = TaskState::Complete;

        // Clear current task if this was it
        if self.current_task.as_ref() == Some(task_id) {
            self.current_task = None;
        }

        self.modified_at = Utc::now();
        Ok(())
    }

    /// Get the current active task.
    #[must_use]
    pub fn current(&self) -> Option<&Task> {
        self.current_task.as_ref().and_then(|id| self.tasks.get(id))
    }

    /// Set the current task (without state transition).
    ///
    /// # Errors
    ///
    /// Returns an error if the task doesn't exist.
    pub fn set_current(&mut self, task_id: &TaskId) -> Result<()> {
        if !self.tasks.contains_key(task_id) {
            bail!("Task not found: {}", task_id);
        }
        self.current_task = Some(task_id.clone());
        self.modified_at = Utc::now();
        Ok(())
    }

    /// Clear the current task.
    pub fn clear_current(&mut self) {
        self.current_task = None;
        self.modified_at = Utc::now();
    }

    // ========================================================================
    // Task Selection Algorithm
    // ========================================================================

    /// Select the next task to work on.
    ///
    /// Priority order:
    /// 1. Currently active task (if any and still workable)
    /// 2. InProgress tasks (resume what we started)
    /// 3. NotStarted tasks (start fresh)
    ///
    /// Returns None if all tasks are complete or blocked.
    ///
    /// # Example
    ///
    /// ```
    /// use ralph::r#loop::task_tracker::{TaskTracker, TaskTrackerConfig};
    ///
    /// let mut tracker = TaskTracker::new(TaskTrackerConfig::default());
    /// tracker.parse_plan("### 1. Task one\n### 2. Task two").unwrap();
    ///
    /// // First selection returns task 1
    /// let next = tracker.select_next_task().unwrap();
    /// assert_eq!(next.number(), 1);
    /// ```
    #[must_use]
    pub fn select_next_task(&self) -> Option<&TaskId> {
        // Priority 1: Current task if still workable
        if let Some(ref current_id) = self.current_task {
            if let Some(task) = self.tasks.get(current_id) {
                if task.is_workable() {
                    return Some(current_id);
                }
            }
        }

        // Priority 2: In-progress tasks by number
        let mut in_progress: Vec<&TaskId> = self
            .tasks
            .iter()
            .filter(|(_, t)| t.state == TaskState::InProgress)
            .map(|(id, _)| id)
            .collect();
        in_progress.sort_by_key(|id| id.number());
        if let Some(id) = in_progress.first() {
            return Some(id);
        }

        // Priority 3: Not-started tasks by number
        let mut not_started: Vec<&TaskId> = self
            .tasks
            .iter()
            .filter(|(_, t)| t.state == TaskState::NotStarted)
            .map(|(id, _)| id)
            .collect();
        not_started.sort_by_key(|id| id.number());
        if let Some(id) = not_started.first() {
            return Some(id);
        }

        None
    }

    /// Check if a task is stuck based on multiple criteria.
    ///
    /// A task is considered stuck if any of:
    /// - No progress for `stagnation_threshold` iterations
    /// - Quality gate failures approaching limit
    /// - Been in the same state for too long
    ///
    /// # Example
    ///
    /// ```
    /// use ralph::r#loop::task_tracker::{TaskTracker, TaskTrackerConfig, TaskId};
    ///
    /// let mut tracker = TaskTracker::new(TaskTrackerConfig::default().with_stagnation_threshold(3));
    /// tracker.parse_plan("### 1. Test task").unwrap();
    ///
    /// let task_id = TaskId::parse("### 1. Test task").unwrap();
    /// assert!(!tracker.is_task_stuck(&task_id)); // Not stuck initially
    /// ```
    #[must_use]
    pub fn is_task_stuck(&self, task_id: &TaskId) -> bool {
        let task = match self.tasks.get(task_id) {
            Some(t) => t,
            None => return false,
        };

        // Check 1: High no-progress count (approaching threshold)
        let stagnation_warning = self.config.stagnation_threshold.saturating_sub(1);
        if task.metrics.no_progress_count >= stagnation_warning {
            return true;
        }

        // Check 2: Quality failures approaching limit
        let quality_warning = self.config.max_quality_failures.saturating_sub(1);
        if task.metrics.quality_failures >= quality_warning {
            return true;
        }

        // Check 3: Too many iterations without completion
        // Heuristic: > 10 iterations for a single task suggests trouble
        if task.metrics.iterations > 10 && task.state == TaskState::InProgress {
            return true;
        }

        false
    }

    /// Get stuckness details for a task.
    ///
    /// Returns None if task is not stuck, otherwise returns diagnostic info.
    #[must_use]
    pub fn get_stuck_reason(&self, task_id: &TaskId) -> Option<String> {
        let task = self.tasks.get(task_id)?;

        let mut reasons = Vec::new();

        let stagnation_warning = self.config.stagnation_threshold.saturating_sub(1);
        if task.metrics.no_progress_count >= stagnation_warning {
            reasons.push(format!(
                "no progress for {} iterations (threshold: {})",
                task.metrics.no_progress_count,
                self.config.stagnation_threshold
            ));
        }

        let quality_warning = self.config.max_quality_failures.saturating_sub(1);
        if task.metrics.quality_failures >= quality_warning {
            reasons.push(format!(
                "{} quality gate failures (max: {})",
                task.metrics.quality_failures,
                self.config.max_quality_failures
            ));
        }

        if task.metrics.iterations > 10 && task.state == TaskState::InProgress {
            reasons.push(format!(
                "{} iterations without completion",
                task.metrics.iterations
            ));
        }

        if reasons.is_empty() {
            None
        } else {
            Some(reasons.join("; "))
        }
    }

    /// Generate a context summary for prompt injection.
    ///
    /// Provides current task state, progress metrics, and any warnings
    /// for use in dynamic prompt generation.
    ///
    /// # Example
    ///
    /// ```
    /// use ralph::r#loop::task_tracker::{TaskTracker, TaskTrackerConfig};
    ///
    /// let mut tracker = TaskTracker::new(TaskTrackerConfig::default());
    /// tracker.parse_plan("### 1. Setup\n- [ ] Item").unwrap();
    ///
    /// let summary = tracker.get_context_summary();
    /// assert!(summary.contains("Task 1"));
    /// ```
    #[must_use]
    pub fn get_context_summary(&self) -> String {
        let mut lines = Vec::new();

        // Header
        lines.push("## Current Task Status".to_string());
        lines.push(String::new());

        // Overall progress
        let counts = self.task_counts();
        lines.push(format!(
            "**Overall Progress**: {}/{} tasks complete ({:.0}%)",
            counts.complete,
            counts.total(),
            self.overall_completion()
        ));

        // Current task
        if let Some(current) = self.current() {
            lines.push(String::new());
            lines.push(format!("**Current Task**: {}", current.id));
            lines.push(format!("- State: {}", current.state));
            lines.push(format!("- Iterations: {}", current.metrics.iterations));
            lines.push(format!("- Checkbox progress: {:.0}%", current.completion_percentage()));

            if current.metrics.no_progress_count > 0 {
                lines.push(format!(
                    "- No progress for {} iteration(s)",
                    current.metrics.no_progress_count
                ));
            }

            if current.metrics.quality_failures > 0 {
                lines.push(format!(
                    "- Quality gate failures: {}",
                    current.metrics.quality_failures
                ));
            }

            // Warnings
            if let Some(ref current_id) = self.current_task {
                if self.is_task_stuck(current_id) {
                    lines.push(String::new());
                    lines.push("**WARNING**: Task appears stuck".to_string());
                    if let Some(reason) = self.get_stuck_reason(current_id) {
                        lines.push(format!("- {}", reason));
                    }
                }
            }
        } else {
            lines.push(String::new());
            lines.push("**No current task selected**".to_string());
        }

        // Blocked tasks
        let blocked: Vec<&Task> = self.tasks.values()
            .filter(|t| t.state == TaskState::Blocked)
            .collect();
        if !blocked.is_empty() {
            lines.push(String::new());
            lines.push(format!("**Blocked Tasks**: {} task(s)", blocked.len()));
            for task in blocked.iter().take(3) {
                if let Some(ref reason) = task.block_reason {
                    lines.push(format!("- {}: {}", task.id, reason));
                }
            }
        }

        lines.join("\n")
    }

    /// Get the next workable task ID, or None if all are done/blocked.
    ///
    /// This is a convenience method that combines `select_next_task`
    /// with checking for completion.
    #[must_use]
    pub fn next_task(&self) -> Option<TaskId> {
        self.select_next_task().cloned()
    }

    /// Check if all workable tasks are done.
    #[must_use]
    pub fn is_all_done(&self) -> bool {
        self.tasks.values().all(|t| {
            t.state == TaskState::Complete || t.state == TaskState::Blocked
        })
    }

    /// Get count of remaining tasks (not complete, not blocked).
    #[must_use]
    pub fn remaining_count(&self) -> usize {
        self.tasks.values()
            .filter(|t| t.state != TaskState::Complete && t.state != TaskState::Blocked)
            .count()
    }

    // ========================================================================
    // Persistence
    // ========================================================================

    /// Save the tracker state to a JSON file.
    ///
    /// Creates parent directories if they don't exist.
    ///
    /// # Arguments
    ///
    /// * `path` - Path to the JSON file
    ///
    /// # Example
    ///
    /// ```no_run
    /// use ralph::r#loop::task_tracker::{TaskTracker, TaskTrackerConfig};
    /// use std::path::Path;
    ///
    /// let tracker = TaskTracker::new(TaskTrackerConfig::default());
    /// tracker.save(Path::new(".ralph/task_tracker.json")).unwrap();
    /// ```
    pub fn save(&self, path: &std::path::Path) -> Result<()> {
        // Create parent directories if needed
        if let Some(parent) = path.parent() {
            if !parent.exists() {
                std::fs::create_dir_all(parent)
                    .with_context(|| format!("Failed to create directory: {}", parent.display()))?;
            }
        }

        let json = serde_json::to_string_pretty(self)
            .context("Failed to serialize task tracker")?;

        std::fs::write(path, json)
            .with_context(|| format!("Failed to write task tracker to: {}", path.display()))?;

        Ok(())
    }

    /// Load tracker state from a JSON file.
    ///
    /// Returns a new tracker if the file doesn't exist.
    ///
    /// # Arguments
    ///
    /// * `path` - Path to the JSON file
    /// * `config` - Configuration to use if file doesn't exist
    ///
    /// # Example
    ///
    /// ```no_run
    /// use ralph::r#loop::task_tracker::{TaskTracker, TaskTrackerConfig};
    /// use std::path::Path;
    ///
    /// let tracker = TaskTracker::load(
    ///     Path::new(".ralph/task_tracker.json"),
    ///     TaskTrackerConfig::default(),
    /// ).unwrap();
    /// ```
    pub fn load(path: &std::path::Path, config: TaskTrackerConfig) -> Result<Self> {
        if !path.exists() {
            return Ok(Self::new(config));
        }

        let json = std::fs::read_to_string(path)
            .with_context(|| format!("Failed to read task tracker from: {}", path.display()))?;

        let tracker: Self = serde_json::from_str(&json)
            .with_context(|| format!("Failed to deserialize task tracker from: {}", path.display()))?;

        Ok(tracker)
    }

    /// Load tracker state, or create new if file doesn't exist or is corrupted.
    ///
    /// This is more lenient than `load()` - it will create a fresh tracker
    /// if the file is corrupted instead of returning an error.
    ///
    /// # Arguments
    ///
    /// * `path` - Path to the JSON file
    /// * `config` - Configuration to use for new tracker
    pub fn load_or_new(path: &std::path::Path, config: TaskTrackerConfig) -> Self {
        match Self::load(path, config.clone()) {
            Ok(tracker) => tracker,
            Err(_) => Self::new(config),
        }
    }

    /// Auto-save the tracker if auto_save is enabled.
    ///
    /// Call this after mutations if auto-save behavior is desired.
    ///
    /// # Arguments
    ///
    /// * `path` - Path to save to
    pub fn auto_save(&self, path: &std::path::Path) -> Result<()> {
        if self.config.auto_save {
            self.save(path)?;
        }
        Ok(())
    }

    /// Get the default persistence path for a project.
    ///
    /// Returns `.ralph/task_tracker.json` relative to the project root.
    #[must_use]
    pub fn default_path(project_dir: &std::path::Path) -> std::path::PathBuf {
        project_dir.join(".ralph").join("task_tracker.json")
    }
}

/// Summary counts of tasks by state.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct TaskCounts {
    /// Tasks not yet started
    pub not_started: u32,
    /// Tasks currently in progress
    pub in_progress: u32,
    /// Tasks that are blocked
    pub blocked: u32,
    /// Tasks under review
    pub in_review: u32,
    /// Completed tasks
    pub complete: u32,
}

impl TaskCounts {
    /// Get total task count.
    #[must_use]
    pub fn total(&self) -> u32 {
        self.not_started + self.in_progress + self.blocked + self.in_review + self.complete
    }

    /// Check if all tasks are done (complete or blocked).
    #[must_use]
    pub fn all_done(&self) -> bool {
        self.not_started == 0 && self.in_progress == 0 && self.in_review == 0
    }
}

impl Default for TaskTracker {
    fn default() -> Self {
        Self::new(TaskTrackerConfig::default())
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    // ========================================================================
    // TaskId Tests
    // ========================================================================

    #[test]
    fn test_task_id_parse_simple() {
        let id = TaskId::parse("### 1. Setup project").unwrap();
        assert_eq!(id.number(), 1);
        assert_eq!(id.phase(), None);
        assert_eq!(id.title(), "Setup project");
    }

    #[test]
    fn test_task_id_parse_with_phase() {
        let id = TaskId::parse("### 5. Phase 2.3: Build templates").unwrap();
        assert_eq!(id.number(), 5);
        assert_eq!(id.phase(), Some("2.3"));
        assert_eq!(id.title(), "Build templates");
    }

    #[test]
    fn test_task_id_parse_with_phase_and_details() {
        let id = TaskId::parse("### 10. Phase 3.1: Quality Gate Abstractions").unwrap();
        assert_eq!(id.number(), 10);
        assert_eq!(id.phase(), Some("3.1"));
        assert_eq!(id.title(), "Quality Gate Abstractions");
    }

    #[test]
    fn test_task_id_parse_preserves_original() {
        let header = "### 2. Phase 1.1: Task Domain Model";
        let id = TaskId::parse(header).unwrap();
        assert_eq!(id.original(), header);
    }

    #[test]
    fn test_task_id_parse_invalid_no_number() {
        let result = TaskId::parse("### Setup project");
        assert!(result.is_err());
    }

    #[test]
    fn test_task_id_parse_invalid_no_dot() {
        let result = TaskId::parse("### 1 Setup project");
        assert!(result.is_err());
    }

    #[test]
    fn test_task_id_display_without_phase() {
        let id = TaskId::parse("### 1. Setup project").unwrap();
        assert_eq!(id.to_string(), "Task 1: Setup project");
    }

    #[test]
    fn test_task_id_display_with_phase() {
        let id = TaskId::parse("### 5. Phase 2.3: Build templates").unwrap();
        assert_eq!(id.to_string(), "Task 5: Phase 2.3: Build templates");
    }

    #[test]
    fn test_task_id_equality() {
        let id1 = TaskId::parse("### 1. Test task").unwrap();
        let id2 = TaskId::parse("### 1. Test task").unwrap();
        assert_eq!(id1, id2);
    }

    #[test]
    fn test_task_id_hash_map_key() {
        let mut map = HashMap::new();
        let id = TaskId::parse("### 1. Test task").unwrap();
        map.insert(id.clone(), "value");
        assert_eq!(map.get(&id), Some(&"value"));
    }

    #[test]
    fn test_task_id_serialize() {
        let id = TaskId::parse("### 2. Phase 1.1: Test").unwrap();
        let json = serde_json::to_string(&id).unwrap();
        assert!(json.contains("\"number\":2"));
        assert!(json.contains("\"phase\":\"1.1\""));
        assert!(json.contains("\"title\":\"Test\""));
    }

    #[test]
    fn test_task_id_deserialize() {
        let json = r####"{"number":3,"phase":"2.1","title":"Build","original":"### 3. Phase 2.1: Build"}"####;
        let id: TaskId = serde_json::from_str(json).unwrap();
        assert_eq!(id.number(), 3);
        assert_eq!(id.phase(), Some("2.1"));
        assert_eq!(id.title(), "Build");
    }

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
        let reason = BlockReason::MaxAttempts { attempts: 5, max: 5 };
        assert_eq!(reason.to_string(), "Exceeded max attempts (5/5)");
    }

    #[test]
    fn test_block_reason_timeout_display() {
        let reason = BlockReason::Timeout { duration_secs: 3600 };
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
        let reason = BlockReason::MaxAttempts { attempts: 3, max: 5 };
        let json = serde_json::to_string(&reason).unwrap();
        assert!(json.contains("MaxAttempts"));
        assert!(json.contains("\"attempts\":3"));
        assert!(json.contains("\"max\":5"));
    }

    #[test]
    fn test_block_reason_deserialize() {
        let json = r#"{"Timeout":{"duration_secs":1800}}"#;
        let reason: BlockReason = serde_json::from_str(json).unwrap();
        assert!(matches!(reason, BlockReason::Timeout { duration_secs: 1800 }));
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
        let block_reason = BlockReason::MaxAttempts { attempts: 5, max: 5 };
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

    // ========================================================================
    // TaskTrackerConfig Tests
    // ========================================================================

    #[test]
    fn test_task_tracker_config_default() {
        let config = TaskTrackerConfig::default();
        assert_eq!(config.max_attempts_per_task, 5);
        assert_eq!(config.task_timeout_secs, 3600);
        assert_eq!(config.stagnation_threshold, 3);
        assert_eq!(config.max_quality_failures, 3);
        assert!(config.auto_save);
    }

    #[test]
    fn test_task_tracker_config_builder() {
        let config = TaskTrackerConfig::new()
            .with_max_attempts(10)
            .with_timeout_secs(7200)
            .with_stagnation_threshold(5)
            .with_max_quality_failures(2)
            .without_auto_save();

        assert_eq!(config.max_attempts_per_task, 10);
        assert_eq!(config.task_timeout_secs, 7200);
        assert_eq!(config.stagnation_threshold, 5);
        assert_eq!(config.max_quality_failures, 2);
        assert!(!config.auto_save);
    }

    #[test]
    fn test_task_tracker_config_serialize() {
        let config = TaskTrackerConfig::default();
        let json = serde_json::to_string(&config).unwrap();
        assert!(json.contains("\"max_attempts_per_task\":5"));
        assert!(json.contains("\"auto_save\":true"));
    }

    #[test]
    fn test_task_tracker_config_deserialize() {
        let json = r#"{"max_attempts_per_task":10,"task_timeout_secs":1800,"stagnation_threshold":2,"max_quality_failures":1,"auto_save":false}"#;
        let config: TaskTrackerConfig = serde_json::from_str(json).unwrap();
        assert_eq!(config.max_attempts_per_task, 10);
        assert_eq!(config.task_timeout_secs, 1800);
        assert!(!config.auto_save);
    }

    // ========================================================================
    // TaskMetrics Tests
    // ========================================================================

    #[test]
    fn test_task_metrics_default() {
        let metrics = TaskMetrics::default();
        assert_eq!(metrics.iterations, 0);
        assert_eq!(metrics.commits, 0);
        assert_eq!(metrics.files_modified, 0);
        assert_eq!(metrics.no_progress_count, 0);
        assert_eq!(metrics.quality_failures, 0);
        assert_eq!(metrics.lines_changed, 0);
    }

    #[test]
    fn test_task_metrics_record_iteration() {
        let mut metrics = TaskMetrics::new();
        metrics.record_iteration();
        metrics.record_iteration();
        assert_eq!(metrics.iterations, 2);
    }

    #[test]
    fn test_task_metrics_record_commit() {
        let mut metrics = TaskMetrics::new();
        metrics.record_commit();
        assert_eq!(metrics.commits, 1);
    }

    #[test]
    fn test_task_metrics_record_progress_resets_no_progress() {
        let mut metrics = TaskMetrics::new();
        metrics.record_no_progress();
        metrics.record_no_progress();
        assert_eq!(metrics.no_progress_count, 2);

        metrics.record_progress(3, 100);
        assert_eq!(metrics.no_progress_count, 0);
        assert_eq!(metrics.files_modified, 3);
        assert_eq!(metrics.lines_changed, 100);
    }

    #[test]
    fn test_task_metrics_quality_failures() {
        let mut metrics = TaskMetrics::new();
        metrics.record_quality_failure();
        metrics.record_quality_failure();
        assert_eq!(metrics.quality_failures, 2);

        metrics.reset_quality_failures();
        assert_eq!(metrics.quality_failures, 0);
    }

    // ========================================================================
    // Task Tests
    // ========================================================================

    #[test]
    fn test_task_new() {
        let id = TaskId::parse("### 1. Test task").unwrap();
        let task = Task::new(id.clone());

        assert_eq!(task.id, id);
        assert_eq!(task.state, TaskState::NotStarted);
        assert!(task.transitions.is_empty());
        assert!(task.block_reason.is_none());
        assert!(task.checkboxes.is_empty());
    }

    #[test]
    fn test_task_completion_percentage_empty() {
        let id = TaskId::parse("### 1. Test task").unwrap();
        let task = Task::new(id);
        assert_eq!(task.completion_percentage(), 0.0);
    }

    #[test]
    fn test_task_completion_percentage_partial() {
        let id = TaskId::parse("### 1. Test task").unwrap();
        let mut task = Task::new(id);
        task.checkboxes = vec![
            ("Item 1".to_string(), true),
            ("Item 2".to_string(), false),
            ("Item 3".to_string(), true),
            ("Item 4".to_string(), false),
        ];
        assert_eq!(task.completion_percentage(), 50.0);
    }

    #[test]
    fn test_task_completion_percentage_all_complete() {
        let id = TaskId::parse("### 1. Test task").unwrap();
        let mut task = Task::new(id);
        task.checkboxes = vec![
            ("Item 1".to_string(), true),
            ("Item 2".to_string(), true),
        ];
        assert_eq!(task.completion_percentage(), 100.0);
    }

    #[test]
    fn test_task_is_complete() {
        let id = TaskId::parse("### 1. Test task").unwrap();
        let mut task = Task::new(id);
        assert!(!task.is_complete());

        task.state = TaskState::Complete;
        assert!(task.is_complete());
    }

    #[test]
    fn test_task_is_blocked() {
        let id = TaskId::parse("### 1. Test task").unwrap();
        let mut task = Task::new(id);
        assert!(!task.is_blocked());

        task.state = TaskState::Blocked;
        assert!(task.is_blocked());
    }

    #[test]
    fn test_task_is_workable() {
        let id = TaskId::parse("### 1. Test task").unwrap();
        let mut task = Task::new(id);

        assert!(task.is_workable()); // NotStarted

        task.state = TaskState::InProgress;
        assert!(task.is_workable());

        task.state = TaskState::Blocked;
        assert!(!task.is_workable());

        task.state = TaskState::InReview;
        assert!(!task.is_workable());

        task.state = TaskState::Complete;
        assert!(!task.is_workable());
    }

    // ========================================================================
    // TaskTracker Tests (basic structure)
    // ========================================================================

    #[test]
    fn test_task_tracker_new() {
        let config = TaskTrackerConfig::default();
        let tracker = TaskTracker::new(config.clone());

        assert!(tracker.tasks.is_empty());
        assert!(tracker.current_task.is_none());
        assert_eq!(tracker.config.max_attempts_per_task, config.max_attempts_per_task);
    }

    #[test]
    fn test_task_tracker_default() {
        let tracker = TaskTracker::default();
        assert!(tracker.tasks.is_empty());
        assert!(tracker.current_task.is_none());
    }

    #[test]
    fn test_task_tracker_serialize() {
        let tracker = TaskTracker::default();
        let json = serde_json::to_string(&tracker).unwrap();
        // Tasks is serialized as a Vec (array), not a HashMap (object)
        assert!(json.contains("\"tasks\":[]"));
        assert!(json.contains("\"current_task\":null"));
    }

    #[test]
    fn test_task_tracker_deserialize() {
        // Tasks is deserialized from a Vec (array), not a HashMap (object)
        let json = r#"{"tasks":[],"current_task":null,"config":{"max_attempts_per_task":5,"task_timeout_secs":3600,"stagnation_threshold":3,"max_quality_failures":3,"auto_save":true},"created_at":"2024-01-01T00:00:00Z","modified_at":"2024-01-01T00:00:00Z"}"#;
        let tracker: TaskTracker = serde_json::from_str(json).unwrap();
        assert!(tracker.tasks.is_empty());
        assert!(tracker.current_task.is_none());
    }

    // ========================================================================
    // Plan Parser Tests
    // ========================================================================

    #[test]
    fn test_parse_plan_empty() {
        let mut tracker = TaskTracker::default();
        tracker.parse_plan("").unwrap();
        assert!(tracker.tasks.is_empty());
    }

    #[test]
    fn test_parse_plan_no_tasks() {
        let mut tracker = TaskTracker::default();
        let plan = r#"
# Implementation Plan

This is the introduction.

## Overview

Some overview text.
"#;
        tracker.parse_plan(plan).unwrap();
        assert!(tracker.tasks.is_empty());
    }

    #[test]
    fn test_parse_plan_single_task() {
        let mut tracker = TaskTracker::default();
        let plan = r#"
## Tasks

### 1. Setup project
- [ ] Create directory
- [ ] Initialize git
"#;
        tracker.parse_plan(plan).unwrap();
        assert_eq!(tracker.tasks.len(), 1);

        let task = tracker.get_task_by_number(1).unwrap();
        assert_eq!(task.id.title(), "Setup project");
        assert_eq!(task.checkboxes.len(), 2);
        assert_eq!(task.checkboxes[0].0, "Create directory");
        assert!(!task.checkboxes[0].1);
        assert_eq!(task.checkboxes[1].0, "Initialize git");
        assert!(!task.checkboxes[1].1);
    }

    #[test]
    fn test_parse_plan_multiple_tasks() {
        let mut tracker = TaskTracker::default();
        let plan = r#"
## Tasks

### 1. Phase 1.1: Setup
- [x] Done item

### 2. Phase 1.2: Build
- [ ] Not done item

### 3. Phase 2.1: Test
- [ ] Item 1
- [x] Item 2
- [ ] Item 3
"#;
        tracker.parse_plan(plan).unwrap();
        assert_eq!(tracker.tasks.len(), 3);

        // Check task 1
        let task1 = tracker.get_task_by_number(1).unwrap();
        assert_eq!(task1.id.phase(), Some("1.1"));
        assert_eq!(task1.checkboxes.len(), 1);
        assert!(task1.checkboxes[0].1); // checked

        // Check task 2
        let task2 = tracker.get_task_by_number(2).unwrap();
        assert_eq!(task2.id.phase(), Some("1.2"));
        assert_eq!(task2.checkboxes.len(), 1);
        assert!(!task2.checkboxes[0].1); // unchecked

        // Check task 3
        let task3 = tracker.get_task_by_number(3).unwrap();
        assert_eq!(task3.id.phase(), Some("2.1"));
        assert_eq!(task3.checkboxes.len(), 3);
    }

    #[test]
    fn test_parse_plan_checkbox_uppercase_x() {
        let mut tracker = TaskTracker::default();
        let plan = r#"
### 1. Test task
- [X] Checked with uppercase X
- [x] Checked with lowercase x
- [ ] Not checked
"#;
        tracker.parse_plan(plan).unwrap();

        let task = tracker.get_task_by_number(1).unwrap();
        assert!(task.checkboxes[0].1); // uppercase X
        assert!(task.checkboxes[1].1); // lowercase x
        assert!(!task.checkboxes[2].1); // not checked
    }

    #[test]
    fn test_parse_plan_incremental_preserves_state() {
        let mut tracker = TaskTracker::default();

        // First parse
        let plan1 = r#"
### 1. Task one
- [ ] Item 1
"#;
        tracker.parse_plan(plan1).unwrap();

        // Modify task state
        if let Some(task) = tracker.get_task_mut(&TaskId::parse("### 1. Task one").unwrap()) {
            task.state = TaskState::InProgress;
            task.metrics.iterations = 5;
        }

        // Re-parse with updated checkboxes
        let plan2 = r#"
### 1. Task one
- [x] Item 1
- [ ] Item 2
"#;
        tracker.parse_plan(plan2).unwrap();

        // Verify state was preserved
        let task = tracker.get_task_by_number(1).unwrap();
        assert_eq!(task.state, TaskState::InProgress);
        assert_eq!(task.metrics.iterations, 5);
        // But checkboxes were updated
        assert_eq!(task.checkboxes.len(), 2);
        assert!(task.checkboxes[0].1); // Item 1 now checked
    }

    #[test]
    fn test_parse_plan_completion_percentage() {
        let mut tracker = TaskTracker::default();
        let plan = r#"
### 1. Half done
- [x] Done 1
- [x] Done 2
- [ ] Not done 1
- [ ] Not done 2

### 2. All done
- [x] A
- [x] B

### 3. None done
- [ ] X
- [ ] Y
"#;
        tracker.parse_plan(plan).unwrap();

        let task1 = tracker.get_task_by_number(1).unwrap();
        assert_eq!(task1.completion_percentage(), 50.0);

        let task2 = tracker.get_task_by_number(2).unwrap();
        assert_eq!(task2.completion_percentage(), 100.0);

        let task3 = tracker.get_task_by_number(3).unwrap();
        assert_eq!(task3.completion_percentage(), 0.0);
    }

    #[test]
    fn test_workable_tasks_priority_order() {
        let mut tracker = TaskTracker::default();
        let plan = r#"
### 1. First
### 2. Second
### 3. Third
### 4. Fourth
"#;
        tracker.parse_plan(plan).unwrap();

        // Set various states
        tracker.get_task_mut(&TaskId::parse("### 2. Second").unwrap())
            .unwrap().state = TaskState::InProgress;
        tracker.get_task_mut(&TaskId::parse("### 3. Third").unwrap())
            .unwrap().state = TaskState::Complete;
        tracker.get_task_mut(&TaskId::parse("### 4. Fourth").unwrap())
            .unwrap().state = TaskState::InProgress;

        let workable = tracker.workable_tasks();

        // Should have 3 workable tasks (not task 3 which is complete)
        assert_eq!(workable.len(), 3);

        // In-progress tasks first (2, 4), then not started (1)
        assert_eq!(workable[0].id.number(), 2);
        assert_eq!(workable[1].id.number(), 4);
        assert_eq!(workable[2].id.number(), 1);
    }

    #[test]
    fn test_task_counts() {
        let mut tracker = TaskTracker::default();
        let plan = r#"
### 1. Task 1
### 2. Task 2
### 3. Task 3
### 4. Task 4
### 5. Task 5
"#;
        tracker.parse_plan(plan).unwrap();

        // Set various states
        tracker.get_task_mut(&TaskId::parse("### 1. Task 1").unwrap())
            .unwrap().state = TaskState::Complete;
        tracker.get_task_mut(&TaskId::parse("### 2. Task 2").unwrap())
            .unwrap().state = TaskState::InProgress;
        tracker.get_task_mut(&TaskId::parse("### 3. Task 3").unwrap())
            .unwrap().state = TaskState::Blocked;
        tracker.get_task_mut(&TaskId::parse("### 4. Task 4").unwrap())
            .unwrap().state = TaskState::InReview;
        // Task 5 stays NotStarted

        let counts = tracker.task_counts();
        assert_eq!(counts.not_started, 1);
        assert_eq!(counts.in_progress, 1);
        assert_eq!(counts.blocked, 1);
        assert_eq!(counts.in_review, 1);
        assert_eq!(counts.complete, 1);
        assert_eq!(counts.total(), 5);
        assert!(!counts.all_done());
    }

    #[test]
    fn test_task_counts_all_done() {
        let mut tracker = TaskTracker::default();
        let plan = r#"
### 1. Task 1
### 2. Task 2
"#;
        tracker.parse_plan(plan).unwrap();

        tracker.get_task_mut(&TaskId::parse("### 1. Task 1").unwrap())
            .unwrap().state = TaskState::Complete;
        tracker.get_task_mut(&TaskId::parse("### 2. Task 2").unwrap())
            .unwrap().state = TaskState::Blocked;

        let counts = tracker.task_counts();
        assert!(counts.all_done());
    }

    #[test]
    fn test_overall_completion() {
        let mut tracker = TaskTracker::default();
        let plan = r#"
### 1. Half done
- [x] Done
- [ ] Not done

### 2. Complete task
- [x] Done

### 3. Not started
- [ ] Not done
"#;
        tracker.parse_plan(plan).unwrap();

        // Mark task 2 as complete
        tracker.get_task_mut(&TaskId::parse("### 2. Complete task").unwrap())
            .unwrap().state = TaskState::Complete;

        // Task 1: 50% checkbox completion
        // Task 2: 100% (Complete state)
        // Task 3: 0% checkbox completion
        // Average: (50 + 100 + 0) / 3 = 50%
        let completion = tracker.overall_completion();
        assert!((completion - 50.0).abs() < 0.01);
    }

    #[test]
    fn test_overall_completion_empty() {
        let tracker = TaskTracker::default();
        assert_eq!(tracker.overall_completion(), 0.0);
    }

    // ========================================================================
    // TaskCounts Tests
    // ========================================================================

    #[test]
    fn test_task_counts_default() {
        let counts = TaskCounts::default();
        assert_eq!(counts.total(), 0);
        assert!(counts.all_done());
    }

    #[test]
    fn test_task_counts_serialize() {
        let counts = TaskCounts {
            not_started: 1,
            in_progress: 2,
            blocked: 3,
            in_review: 4,
            complete: 5,
        };
        let json = serde_json::to_string(&counts).unwrap();
        assert!(json.contains("\"not_started\":1"));
        assert!(json.contains("\"complete\":5"));
    }

    // ========================================================================
    // State Machine Tests
    // ========================================================================

    #[test]
    fn test_start_task() {
        let mut tracker = TaskTracker::default();
        tracker.parse_plan("### 1. Test task\n- [ ] Item").unwrap();

        let task_id = TaskId::parse("### 1. Test task").unwrap();
        tracker.start_task(&task_id).unwrap();

        let task = tracker.get_task(&task_id).unwrap();
        assert_eq!(task.state, TaskState::InProgress);
        assert_eq!(tracker.current_task, Some(task_id.clone()));
        assert_eq!(task.transitions.len(), 1);
        assert_eq!(task.transitions[0].from, TaskState::NotStarted);
        assert_eq!(task.transitions[0].to, TaskState::InProgress);
    }

    #[test]
    fn test_start_task_not_found() {
        let mut tracker = TaskTracker::default();
        let task_id = TaskId::parse("### 1. Missing task").unwrap();

        let result = tracker.start_task(&task_id);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("not found"));
    }

    #[test]
    fn test_start_task_invalid_transition() {
        let mut tracker = TaskTracker::default();
        tracker.parse_plan("### 1. Test task").unwrap();

        let task_id = TaskId::parse("### 1. Test task").unwrap();
        tracker.get_task_mut(&task_id).unwrap().state = TaskState::Complete;

        let result = tracker.start_task(&task_id);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("Cannot start"));
    }

    #[test]
    fn test_start_blocked_task() {
        let mut tracker = TaskTracker::default();
        tracker.parse_plan("### 1. Test task").unwrap();

        let task_id = TaskId::parse("### 1. Test task").unwrap();

        // Block the task first
        tracker.start_task(&task_id).unwrap();
        tracker.block_task(&task_id, BlockReason::Other {
            reason: "Test block".to_string(),
        }).unwrap();

        // Now start should succeed (transition from Blocked -> InProgress)
        tracker.start_task(&task_id).unwrap();
        assert_eq!(tracker.get_task(&task_id).unwrap().state, TaskState::InProgress);
    }

    #[test]
    fn test_record_progress() {
        let mut tracker = TaskTracker::default();
        tracker.parse_plan("### 1. Test task").unwrap();

        let task_id = TaskId::parse("### 1. Test task").unwrap();
        tracker.start_task(&task_id).unwrap();

        tracker.record_progress(5, 100).unwrap();

        let task = tracker.get_task(&task_id).unwrap();
        assert_eq!(task.metrics.files_modified, 5);
        assert_eq!(task.metrics.lines_changed, 100);
        assert_eq!(task.metrics.no_progress_count, 0);
    }

    #[test]
    fn test_record_progress_no_current_task() {
        let mut tracker = TaskTracker::default();
        let result = tracker.record_progress(1, 10);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("No active task"));
    }

    #[test]
    fn test_record_no_progress() {
        let mut tracker = TaskTracker::new(TaskTrackerConfig::default().with_stagnation_threshold(3));
        tracker.parse_plan("### 1. Test task").unwrap();

        let task_id = TaskId::parse("### 1. Test task").unwrap();
        tracker.start_task(&task_id).unwrap();

        // Record no progress twice, should not block
        let blocked1 = tracker.record_no_progress().unwrap();
        assert!(!blocked1);
        let blocked2 = tracker.record_no_progress().unwrap();
        assert!(!blocked2);

        // Third time should auto-block
        let blocked3 = tracker.record_no_progress().unwrap();
        assert!(blocked3);

        let task = tracker.get_task(&task_id).unwrap();
        assert_eq!(task.state, TaskState::Blocked);
        assert!(tracker.current_task.is_none()); // Cleared
    }

    #[test]
    fn test_block_task() {
        let mut tracker = TaskTracker::default();
        tracker.parse_plan("### 1. Test task").unwrap();

        let task_id = TaskId::parse("### 1. Test task").unwrap();
        tracker.start_task(&task_id).unwrap();

        let reason = BlockReason::ExternalDependency {
            description: "Waiting for API".to_string(),
        };
        tracker.block_task(&task_id, reason).unwrap();

        let task = tracker.get_task(&task_id).unwrap();
        assert_eq!(task.state, TaskState::Blocked);
        assert!(task.block_reason.is_some());
        assert!(tracker.current_task.is_none());
    }

    #[test]
    fn test_block_task_invalid_transition() {
        let mut tracker = TaskTracker::default();
        tracker.parse_plan("### 1. Test task").unwrap();

        let task_id = TaskId::parse("### 1. Test task").unwrap();
        // Task is NotStarted, cannot transition to Blocked directly

        let reason = BlockReason::Other { reason: "Test".to_string() };
        let result = tracker.block_task(&task_id, reason);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("Cannot block"));
    }

    #[test]
    fn test_unblock_task() {
        let mut tracker = TaskTracker::default();
        tracker.parse_plan("### 1. Test task").unwrap();

        let task_id = TaskId::parse("### 1. Test task").unwrap();
        tracker.start_task(&task_id).unwrap();

        // Block it
        tracker.block_task(&task_id, BlockReason::Other {
            reason: "Test".to_string(),
        }).unwrap();

        // Unblock it
        tracker.unblock_task(&task_id).unwrap();

        let task = tracker.get_task(&task_id).unwrap();
        assert_eq!(task.state, TaskState::InProgress);
        assert!(task.block_reason.is_none());
        assert_eq!(task.metrics.no_progress_count, 0);
    }

    #[test]
    fn test_unblock_non_blocked_task() {
        let mut tracker = TaskTracker::default();
        tracker.parse_plan("### 1. Test task").unwrap();

        let task_id = TaskId::parse("### 1. Test task").unwrap();
        tracker.start_task(&task_id).unwrap();

        let result = tracker.unblock_task(&task_id);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("not blocked"));
    }

    #[test]
    fn test_submit_for_review() {
        let mut tracker = TaskTracker::default();
        tracker.parse_plan("### 1. Test task").unwrap();

        let task_id = TaskId::parse("### 1. Test task").unwrap();
        tracker.start_task(&task_id).unwrap();
        tracker.submit_for_review(&task_id).unwrap();

        let task = tracker.get_task(&task_id).unwrap();
        assert_eq!(task.state, TaskState::InReview);
    }

    #[test]
    fn test_submit_for_review_not_in_progress() {
        let mut tracker = TaskTracker::default();
        tracker.parse_plan("### 1. Test task").unwrap();

        let task_id = TaskId::parse("### 1. Test task").unwrap();
        // Task is NotStarted

        let result = tracker.submit_for_review(&task_id);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("Cannot submit"));
    }

    #[test]
    fn test_update_review_passed() {
        let mut tracker = TaskTracker::default();
        tracker.parse_plan("### 1. Test task").unwrap();

        let task_id = TaskId::parse("### 1. Test task").unwrap();
        tracker.start_task(&task_id).unwrap();
        tracker.submit_for_review(&task_id).unwrap();

        let blocked = tracker.update_review(&task_id, "clippy", true).unwrap();
        assert!(!blocked);

        let task = tracker.get_task(&task_id).unwrap();
        // Still in review after passing - need to complete separately
        assert_eq!(task.state, TaskState::InReview);
        assert_eq!(task.metrics.quality_failures, 0);
    }

    #[test]
    fn test_update_review_failed_returns_to_in_progress() {
        let mut tracker = TaskTracker::new(TaskTrackerConfig::default().with_max_quality_failures(3));
        tracker.parse_plan("### 1. Test task").unwrap();

        let task_id = TaskId::parse("### 1. Test task").unwrap();
        tracker.start_task(&task_id).unwrap();
        tracker.submit_for_review(&task_id).unwrap();

        // First failure - returns to InProgress
        let blocked = tracker.update_review(&task_id, "clippy", false).unwrap();
        assert!(!blocked);

        let task = tracker.get_task(&task_id).unwrap();
        assert_eq!(task.state, TaskState::InProgress);
        assert_eq!(task.metrics.quality_failures, 1);
    }

    #[test]
    fn test_update_review_max_failures_blocks() {
        let mut tracker = TaskTracker::new(TaskTrackerConfig::default().with_max_quality_failures(2));
        tracker.parse_plan("### 1. Test task").unwrap();

        let task_id = TaskId::parse("### 1. Test task").unwrap();

        // First round
        tracker.start_task(&task_id).unwrap();
        tracker.submit_for_review(&task_id).unwrap();
        tracker.update_review(&task_id, "clippy", false).unwrap();

        // Second round
        tracker.submit_for_review(&task_id).unwrap();
        let blocked = tracker.update_review(&task_id, "clippy", false).unwrap();
        assert!(blocked);

        let task = tracker.get_task(&task_id).unwrap();
        assert_eq!(task.state, TaskState::Blocked);
        assert!(matches!(
            task.block_reason,
            Some(BlockReason::QualityGateFailure { .. })
        ));
    }

    #[test]
    fn test_complete_task() {
        let mut tracker = TaskTracker::default();
        tracker.parse_plan("### 1. Test task").unwrap();

        let task_id = TaskId::parse("### 1. Test task").unwrap();
        tracker.start_task(&task_id).unwrap();
        tracker.complete_task(&task_id).unwrap();

        let task = tracker.get_task(&task_id).unwrap();
        assert_eq!(task.state, TaskState::Complete);
        assert!(tracker.current_task.is_none());
    }

    #[test]
    fn test_complete_task_from_review() {
        let mut tracker = TaskTracker::default();
        tracker.parse_plan("### 1. Test task").unwrap();

        let task_id = TaskId::parse("### 1. Test task").unwrap();
        tracker.start_task(&task_id).unwrap();
        tracker.submit_for_review(&task_id).unwrap();
        tracker.complete_task(&task_id).unwrap();

        let task = tracker.get_task(&task_id).unwrap();
        assert_eq!(task.state, TaskState::Complete);
    }

    #[test]
    fn test_complete_task_already_complete() {
        let mut tracker = TaskTracker::default();
        tracker.parse_plan("### 1. Test task").unwrap();

        let task_id = TaskId::parse("### 1. Test task").unwrap();
        tracker.start_task(&task_id).unwrap();
        tracker.complete_task(&task_id).unwrap();

        // Second complete should fail
        let result = tracker.complete_task(&task_id);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("Cannot complete"));
    }

    #[test]
    fn test_current_task() {
        let mut tracker = TaskTracker::default();
        tracker.parse_plan("### 1. Test task").unwrap();

        assert!(tracker.current().is_none());

        let task_id = TaskId::parse("### 1. Test task").unwrap();
        tracker.start_task(&task_id).unwrap();

        let current = tracker.current().unwrap();
        assert_eq!(current.id.number(), 1);
    }

    #[test]
    fn test_set_current() {
        let mut tracker = TaskTracker::default();
        tracker.parse_plan("### 1. Task 1\n### 2. Task 2").unwrap();

        let task_id = TaskId::parse("### 2. Task 2").unwrap();
        tracker.set_current(&task_id).unwrap();

        assert_eq!(tracker.current_task, Some(task_id));
    }

    #[test]
    fn test_set_current_not_found() {
        let mut tracker = TaskTracker::default();
        let task_id = TaskId::parse("### 1. Missing").unwrap();

        let result = tracker.set_current(&task_id);
        assert!(result.is_err());
    }

    #[test]
    fn test_clear_current() {
        let mut tracker = TaskTracker::default();
        tracker.parse_plan("### 1. Test task").unwrap();

        let task_id = TaskId::parse("### 1. Test task").unwrap();
        tracker.start_task(&task_id).unwrap();
        assert!(tracker.current_task.is_some());

        tracker.clear_current();
        assert!(tracker.current_task.is_none());
    }

    #[test]
    fn test_full_task_lifecycle() {
        let mut tracker = TaskTracker::default();
        tracker.parse_plan("### 1. Full lifecycle task").unwrap();

        let task_id = TaskId::parse("### 1. Full lifecycle task").unwrap();

        // Start
        tracker.start_task(&task_id).unwrap();
        assert_eq!(tracker.get_task(&task_id).unwrap().state, TaskState::InProgress);

        // Record some progress
        tracker.record_progress(3, 50).unwrap();

        // Submit for review
        tracker.submit_for_review(&task_id).unwrap();
        assert_eq!(tracker.get_task(&task_id).unwrap().state, TaskState::InReview);

        // Review passes
        tracker.update_review(&task_id, "tests", true).unwrap();

        // Complete
        tracker.complete_task(&task_id).unwrap();
        assert_eq!(tracker.get_task(&task_id).unwrap().state, TaskState::Complete);

        // Check transition history
        let task = tracker.get_task(&task_id).unwrap();
        assert!(task.transitions.len() >= 3);
    }

    #[test]
    fn test_transition_history_preserved() {
        let mut tracker = TaskTracker::default();
        tracker.parse_plan("### 1. Test task").unwrap();

        let task_id = TaskId::parse("### 1. Test task").unwrap();

        tracker.start_task(&task_id).unwrap();
        tracker.block_task(&task_id, BlockReason::Other { reason: "Test".to_string() }).unwrap();
        tracker.unblock_task(&task_id).unwrap();
        tracker.submit_for_review(&task_id).unwrap();
        tracker.complete_task(&task_id).unwrap();

        let task = tracker.get_task(&task_id).unwrap();
        assert_eq!(task.transitions.len(), 5);

        // Verify transition sequence
        assert_eq!(task.transitions[0].to, TaskState::InProgress);
        assert_eq!(task.transitions[1].to, TaskState::Blocked);
        assert_eq!(task.transitions[2].to, TaskState::InProgress);
        assert_eq!(task.transitions[3].to, TaskState::InReview);
        assert_eq!(task.transitions[4].to, TaskState::Complete);
    }

    // ========================================================================
    // Task Selection Algorithm Tests
    // ========================================================================

    #[test]
    fn test_select_next_task_returns_current_if_workable() {
        let mut tracker = TaskTracker::default();
        tracker.parse_plan("### 1. Task 1\n### 2. Task 2").unwrap();

        let task1_id = TaskId::parse("### 1. Task 1").unwrap();
        tracker.start_task(&task1_id).unwrap();

        // Current task should be returned
        let next = tracker.select_next_task().unwrap();
        assert_eq!(next.number(), 1);
    }

    #[test]
    fn test_select_next_task_skips_non_workable_current() {
        let mut tracker = TaskTracker::default();
        tracker.parse_plan("### 1. Task 1\n### 2. Task 2").unwrap();

        let task1_id = TaskId::parse("### 1. Task 1").unwrap();
        let task2_id = TaskId::parse("### 2. Task 2").unwrap();

        tracker.start_task(&task1_id).unwrap();
        tracker.complete_task(&task1_id).unwrap();
        tracker.start_task(&task2_id).unwrap();
        tracker.set_current(&task1_id).unwrap(); // Force current to completed task

        // Should skip completed current and return task 2 (InProgress)
        let next = tracker.select_next_task().unwrap();
        assert_eq!(next.number(), 2);
    }

    #[test]
    fn test_select_next_task_prefers_in_progress() {
        let mut tracker = TaskTracker::default();
        tracker.parse_plan("### 1. Task 1\n### 2. Task 2\n### 3. Task 3").unwrap();

        let task2_id = TaskId::parse("### 2. Task 2").unwrap();
        tracker.start_task(&task2_id).unwrap();
        tracker.clear_current(); // Clear current to test priority logic

        // Task 2 is InProgress, Task 1 & 3 are NotStarted
        let next = tracker.select_next_task().unwrap();
        assert_eq!(next.number(), 2); // InProgress takes priority
    }

    #[test]
    fn test_select_next_task_in_progress_by_number() {
        let mut tracker = TaskTracker::default();
        tracker.parse_plan("### 1. Task 1\n### 2. Task 2\n### 3. Task 3").unwrap();

        let task2_id = TaskId::parse("### 2. Task 2").unwrap();
        let task3_id = TaskId::parse("### 3. Task 3").unwrap();

        tracker.start_task(&task3_id).unwrap();
        tracker.clear_current();
        tracker.start_task(&task2_id).unwrap();
        tracker.clear_current();

        // Both 2 and 3 are InProgress, should return 2 (lower number)
        let next = tracker.select_next_task().unwrap();
        assert_eq!(next.number(), 2);
    }

    #[test]
    fn test_select_next_task_not_started_fallback() {
        let mut tracker = TaskTracker::default();
        tracker.parse_plan("### 1. Task 1\n### 2. Task 2").unwrap();

        // All tasks NotStarted, should return task 1
        let next = tracker.select_next_task().unwrap();
        assert_eq!(next.number(), 1);
    }

    #[test]
    fn test_select_next_task_none_when_all_done() {
        let mut tracker = TaskTracker::default();
        tracker.parse_plan("### 1. Task 1\n### 2. Task 2").unwrap();

        let task1_id = TaskId::parse("### 1. Task 1").unwrap();
        let task2_id = TaskId::parse("### 2. Task 2").unwrap();

        tracker.start_task(&task1_id).unwrap();
        tracker.complete_task(&task1_id).unwrap();
        tracker.start_task(&task2_id).unwrap();
        tracker.complete_task(&task2_id).unwrap();

        assert!(tracker.select_next_task().is_none());
    }

    #[test]
    fn test_select_next_task_none_when_all_blocked() {
        let mut tracker = TaskTracker::default();
        tracker.parse_plan("### 1. Task 1\n### 2. Task 2").unwrap();

        let task1_id = TaskId::parse("### 1. Task 1").unwrap();
        let task2_id = TaskId::parse("### 2. Task 2").unwrap();

        tracker.start_task(&task1_id).unwrap();
        tracker.block_task(&task1_id, BlockReason::Other { reason: "X".to_string() }).unwrap();
        tracker.start_task(&task2_id).unwrap();
        tracker.block_task(&task2_id, BlockReason::Other { reason: "Y".to_string() }).unwrap();

        assert!(tracker.select_next_task().is_none());
    }

    #[test]
    fn test_is_task_stuck_not_stuck_initially() {
        let mut tracker = TaskTracker::new(TaskTrackerConfig::default().with_stagnation_threshold(3));
        tracker.parse_plan("### 1. Test task").unwrap();

        let task_id = TaskId::parse("### 1. Test task").unwrap();
        assert!(!tracker.is_task_stuck(&task_id));
    }

    #[test]
    fn test_is_task_stuck_approaching_stagnation() {
        let mut tracker = TaskTracker::new(TaskTrackerConfig::default().with_stagnation_threshold(3));
        tracker.parse_plan("### 1. Test task").unwrap();

        let task_id = TaskId::parse("### 1. Test task").unwrap();
        tracker.start_task(&task_id).unwrap();

        // Two no-progress iterations = approaching threshold (3-1=2)
        tracker.get_task_mut(&task_id).unwrap().metrics.no_progress_count = 2;

        assert!(tracker.is_task_stuck(&task_id));
    }

    #[test]
    fn test_is_task_stuck_approaching_quality_failures() {
        let mut tracker = TaskTracker::new(TaskTrackerConfig::default().with_max_quality_failures(3));
        tracker.parse_plan("### 1. Test task").unwrap();

        let task_id = TaskId::parse("### 1. Test task").unwrap();
        tracker.start_task(&task_id).unwrap();

        // Two quality failures = approaching threshold (3-1=2)
        tracker.get_task_mut(&task_id).unwrap().metrics.quality_failures = 2;

        assert!(tracker.is_task_stuck(&task_id));
    }

    #[test]
    fn test_is_task_stuck_too_many_iterations() {
        let mut tracker = TaskTracker::default();
        tracker.parse_plan("### 1. Test task").unwrap();

        let task_id = TaskId::parse("### 1. Test task").unwrap();
        tracker.start_task(&task_id).unwrap();

        // 11 iterations without completion
        tracker.get_task_mut(&task_id).unwrap().metrics.iterations = 11;

        assert!(tracker.is_task_stuck(&task_id));
    }

    #[test]
    fn test_is_task_stuck_many_iterations_but_not_in_progress() {
        let mut tracker = TaskTracker::default();
        tracker.parse_plan("### 1. Test task").unwrap();

        let task_id = TaskId::parse("### 1. Test task").unwrap();
        tracker.start_task(&task_id).unwrap();
        tracker.get_task_mut(&task_id).unwrap().metrics.iterations = 11;
        tracker.complete_task(&task_id).unwrap();

        // Not stuck because it's complete (not InProgress)
        assert!(!tracker.is_task_stuck(&task_id));
    }

    #[test]
    fn test_is_task_stuck_nonexistent_task() {
        let tracker = TaskTracker::default();
        let task_id = TaskId::parse("### 1. Missing").unwrap();
        assert!(!tracker.is_task_stuck(&task_id)); // Returns false for nonexistent
    }

    #[test]
    fn test_get_stuck_reason_none_when_not_stuck() {
        let mut tracker = TaskTracker::new(TaskTrackerConfig::default().with_stagnation_threshold(5));
        tracker.parse_plan("### 1. Test task").unwrap();

        let task_id = TaskId::new_for_test(1, "Test task");
        assert!(tracker.get_stuck_reason(&task_id).is_none());
    }

    #[test]
    fn test_get_stuck_reason_stagnation() {
        let mut tracker = TaskTracker::new(TaskTrackerConfig::default().with_stagnation_threshold(3));
        tracker.parse_plan("### 1. Test task").unwrap();

        let task_id = TaskId::new_for_test(1, "Test task");
        tracker.start_task(&task_id).unwrap();
        tracker.get_task_mut(&task_id).unwrap().metrics.no_progress_count = 2;

        let reason = tracker.get_stuck_reason(&task_id).unwrap();
        assert!(reason.contains("no progress for 2 iterations"));
        assert!(reason.contains("threshold: 3"));
    }

    #[test]
    fn test_get_stuck_reason_quality_failures() {
        let mut tracker = TaskTracker::new(TaskTrackerConfig::default().with_max_quality_failures(3));
        tracker.parse_plan("### 1. Test task").unwrap();

        let task_id = TaskId::new_for_test(1, "Test task");
        tracker.start_task(&task_id).unwrap();
        tracker.get_task_mut(&task_id).unwrap().metrics.quality_failures = 2;

        let reason = tracker.get_stuck_reason(&task_id).unwrap();
        assert!(reason.contains("2 quality gate failures"));
        assert!(reason.contains("max: 3"));
    }

    #[test]
    fn test_get_stuck_reason_multiple_reasons() {
        let mut tracker = TaskTracker::new(
            TaskTrackerConfig::default()
                .with_stagnation_threshold(3)
                .with_max_quality_failures(3)
        );
        tracker.parse_plan("### 1. Test task").unwrap();

        let task_id = TaskId::new_for_test(1, "Test task");
        tracker.start_task(&task_id).unwrap();

        let task = tracker.get_task_mut(&task_id).unwrap();
        task.metrics.no_progress_count = 2;
        task.metrics.quality_failures = 2;
        task.metrics.iterations = 15;

        let reason = tracker.get_stuck_reason(&task_id).unwrap();
        assert!(reason.contains("no progress"));
        assert!(reason.contains("quality gate failures"));
        assert!(reason.contains("iterations without completion"));
        // Reasons are joined with "; "
        assert!(reason.contains("; "));
    }

    #[test]
    fn test_get_context_summary_no_current_task() {
        let tracker = TaskTracker::default();
        let summary = tracker.get_context_summary();

        assert!(summary.contains("## Current Task Status"));
        assert!(summary.contains("**No current task selected**"));
    }

    #[test]
    fn test_get_context_summary_with_current_task() {
        let mut tracker = TaskTracker::default();
        tracker.parse_plan("### 1. Test task\n- [ ] Item").unwrap();

        let task_id = TaskId::parse("### 1. Test task").unwrap();
        tracker.start_task(&task_id).unwrap();

        let summary = tracker.get_context_summary();
        assert!(summary.contains("**Current Task**: Task 1"));
        assert!(summary.contains("- State: In Progress")); // Note: space in "In Progress"
        assert!(summary.contains("- Iterations: 0"));
    }

    #[test]
    fn test_get_context_summary_shows_progress_warnings() {
        let mut tracker = TaskTracker::default();
        tracker.parse_plan("### 1. Test task").unwrap();

        let task_id = TaskId::parse("### 1. Test task").unwrap();
        tracker.start_task(&task_id).unwrap();
        tracker.get_task_mut(&task_id).unwrap().metrics.no_progress_count = 2;
        tracker.get_task_mut(&task_id).unwrap().metrics.quality_failures = 1;

        let summary = tracker.get_context_summary();
        assert!(summary.contains("No progress for 2 iteration(s)"));
        assert!(summary.contains("Quality gate failures: 1"));
    }

    #[test]
    fn test_get_context_summary_shows_stuck_warning() {
        let mut tracker = TaskTracker::new(TaskTrackerConfig::default().with_stagnation_threshold(3));
        tracker.parse_plan("### 1. Test task").unwrap();

        let task_id = TaskId::parse("### 1. Test task").unwrap();
        tracker.start_task(&task_id).unwrap();
        tracker.get_task_mut(&task_id).unwrap().metrics.no_progress_count = 2;

        let summary = tracker.get_context_summary();
        assert!(summary.contains("**WARNING**: Task appears stuck"));
    }

    #[test]
    fn test_get_context_summary_shows_blocked_tasks() {
        let mut tracker = TaskTracker::default();
        tracker.parse_plan("### 1. Task 1\n### 2. Task 2").unwrap();

        let task1_id = TaskId::parse("### 1. Task 1").unwrap();
        tracker.start_task(&task1_id).unwrap();
        tracker.block_task(&task1_id, BlockReason::ExternalDependency {
            description: "Waiting for API".to_string(),
        }).unwrap();

        let summary = tracker.get_context_summary();
        assert!(summary.contains("**Blocked Tasks**: 1 task(s)"));
        assert!(summary.contains("Task 1"));
        assert!(summary.contains("Waiting for API"));
    }

    #[test]
    fn test_get_context_summary_overall_progress() {
        let mut tracker = TaskTracker::default();
        tracker.parse_plan("### 1. Task 1\n### 2. Task 2").unwrap();

        let task1_id = TaskId::parse("### 1. Task 1").unwrap();
        tracker.start_task(&task1_id).unwrap();
        tracker.complete_task(&task1_id).unwrap();

        let summary = tracker.get_context_summary();
        assert!(summary.contains("**Overall Progress**: 1/2 tasks complete"));
    }

    #[test]
    fn test_next_task_returns_cloned_id() {
        let mut tracker = TaskTracker::default();
        tracker.parse_plan("### 1. Test task").unwrap();

        let next = tracker.next_task().unwrap();
        assert_eq!(next.number(), 1);
    }

    #[test]
    fn test_next_task_none_when_done() {
        let mut tracker = TaskTracker::default();
        tracker.parse_plan("### 1. Test task").unwrap();

        let task_id = TaskId::parse("### 1. Test task").unwrap();
        tracker.start_task(&task_id).unwrap();
        tracker.complete_task(&task_id).unwrap();

        assert!(tracker.next_task().is_none());
    }

    #[test]
    fn test_is_all_done_empty() {
        let tracker = TaskTracker::default();
        assert!(tracker.is_all_done()); // No tasks = all done
    }

    #[test]
    fn test_is_all_done_false_with_pending() {
        let mut tracker = TaskTracker::default();
        tracker.parse_plan("### 1. Test task").unwrap();
        assert!(!tracker.is_all_done());
    }

    #[test]
    fn test_is_all_done_true_when_complete() {
        let mut tracker = TaskTracker::default();
        tracker.parse_plan("### 1. Test task").unwrap();

        let task_id = TaskId::parse("### 1. Test task").unwrap();
        tracker.start_task(&task_id).unwrap();
        tracker.complete_task(&task_id).unwrap();

        assert!(tracker.is_all_done());
    }

    #[test]
    fn test_is_all_done_true_when_blocked() {
        let mut tracker = TaskTracker::default();
        tracker.parse_plan("### 1. Test task").unwrap();

        let task_id = TaskId::parse("### 1. Test task").unwrap();
        tracker.start_task(&task_id).unwrap();
        tracker.block_task(&task_id, BlockReason::Other { reason: "X".to_string() }).unwrap();

        assert!(tracker.is_all_done()); // Blocked counts as "done" (no more work possible)
    }

    #[test]
    fn test_is_all_done_mixed() {
        let mut tracker = TaskTracker::default();
        tracker.parse_plan("### 1. Task 1\n### 2. Task 2\n### 3. Task 3").unwrap();

        let task1_id = TaskId::parse("### 1. Task 1").unwrap();
        let task2_id = TaskId::parse("### 2. Task 2").unwrap();

        tracker.start_task(&task1_id).unwrap();
        tracker.complete_task(&task1_id).unwrap();
        tracker.start_task(&task2_id).unwrap();
        tracker.block_task(&task2_id, BlockReason::Other { reason: "X".to_string() }).unwrap();
        // Task 3 still NotStarted

        assert!(!tracker.is_all_done());
    }

    #[test]
    fn test_remaining_count_empty() {
        let tracker = TaskTracker::default();
        assert_eq!(tracker.remaining_count(), 0);
    }

    #[test]
    fn test_remaining_count_all_pending() {
        let mut tracker = TaskTracker::default();
        tracker.parse_plan("### 1. Task 1\n### 2. Task 2\n### 3. Task 3").unwrap();
        assert_eq!(tracker.remaining_count(), 3);
    }

    #[test]
    fn test_remaining_count_excludes_complete() {
        let mut tracker = TaskTracker::default();
        tracker.parse_plan("### 1. Task 1\n### 2. Task 2\n### 3. Task 3").unwrap();

        let task1_id = TaskId::parse("### 1. Task 1").unwrap();
        tracker.start_task(&task1_id).unwrap();
        tracker.complete_task(&task1_id).unwrap();

        assert_eq!(tracker.remaining_count(), 2);
    }

    #[test]
    fn test_remaining_count_excludes_blocked() {
        let mut tracker = TaskTracker::default();
        tracker.parse_plan("### 1. Task 1\n### 2. Task 2\n### 3. Task 3").unwrap();

        let task1_id = TaskId::parse("### 1. Task 1").unwrap();
        tracker.start_task(&task1_id).unwrap();
        tracker.block_task(&task1_id, BlockReason::Other { reason: "X".to_string() }).unwrap();

        assert_eq!(tracker.remaining_count(), 2);
    }

    #[test]
    fn test_remaining_count_includes_in_progress() {
        let mut tracker = TaskTracker::default();
        tracker.parse_plan("### 1. Task 1\n### 2. Task 2").unwrap();

        let task1_id = TaskId::parse("### 1. Task 1").unwrap();
        tracker.start_task(&task1_id).unwrap();

        // Task 1 InProgress, Task 2 NotStarted - both count as remaining
        assert_eq!(tracker.remaining_count(), 2);
    }

    #[test]
    fn test_remaining_count_includes_in_review() {
        let mut tracker = TaskTracker::default();
        tracker.parse_plan("### 1. Task 1\n### 2. Task 2").unwrap();

        let task1_id = TaskId::parse("### 1. Task 1").unwrap();
        tracker.start_task(&task1_id).unwrap();
        tracker.submit_for_review(&task1_id).unwrap();

        // Task 1 InReview, Task 2 NotStarted - both count as remaining
        assert_eq!(tracker.remaining_count(), 2);
    }

    #[test]
    fn test_remaining_count_zero_when_all_done() {
        let mut tracker = TaskTracker::default();
        tracker.parse_plan("### 1. Task 1\n### 2. Task 2").unwrap();

        let task1_id = TaskId::parse("### 1. Task 1").unwrap();
        let task2_id = TaskId::parse("### 2. Task 2").unwrap();

        tracker.start_task(&task1_id).unwrap();
        tracker.complete_task(&task1_id).unwrap();
        tracker.start_task(&task2_id).unwrap();
        tracker.block_task(&task2_id, BlockReason::Other { reason: "X".to_string() }).unwrap();

        assert_eq!(tracker.remaining_count(), 0);
    }

    // ========================================================================
    // Persistence Tests
    // ========================================================================

    #[test]
    fn test_save_and_load_roundtrip() {
        let temp_dir = tempfile::TempDir::new().unwrap();
        let path = temp_dir.path().join("tracker.json");

        // Create and populate a tracker
        let mut original = TaskTracker::default();
        original.parse_plan("### 1. Task one\n- [ ] Item").unwrap();

        let task_id = TaskId::parse("### 1. Task one").unwrap();
        original.start_task(&task_id).unwrap();
        original.record_progress(5, 100).unwrap();

        // Save it
        original.save(&path).unwrap();
        assert!(path.exists());

        // Load it back
        let loaded = TaskTracker::load(&path, TaskTrackerConfig::default()).unwrap();

        // Verify state preserved
        assert_eq!(loaded.tasks.len(), original.tasks.len());
        let loaded_task = loaded.get_task(&task_id).unwrap();
        assert_eq!(loaded_task.state, TaskState::InProgress);
        assert_eq!(loaded_task.metrics.files_modified, 5);
        assert_eq!(loaded_task.metrics.lines_changed, 100);
    }

    #[test]
    fn test_save_creates_directories() {
        let temp_dir = tempfile::TempDir::new().unwrap();
        let path = temp_dir.path().join("nested").join("dirs").join("tracker.json");

        let tracker = TaskTracker::default();
        tracker.save(&path).unwrap();

        assert!(path.exists());
    }

    #[test]
    fn test_load_nonexistent_returns_new() {
        let temp_dir = tempfile::TempDir::new().unwrap();
        let path = temp_dir.path().join("nonexistent.json");

        let config = TaskTrackerConfig::default().with_stagnation_threshold(10);
        let tracker = TaskTracker::load(&path, config).unwrap();

        assert!(tracker.tasks.is_empty());
        assert_eq!(tracker.config.stagnation_threshold, 10);
    }

    #[test]
    fn test_load_corrupted_returns_error() {
        let temp_dir = tempfile::TempDir::new().unwrap();
        let path = temp_dir.path().join("corrupted.json");

        // Write invalid JSON
        std::fs::write(&path, "not valid json {{{").unwrap();

        let result = TaskTracker::load(&path, TaskTrackerConfig::default());
        assert!(result.is_err());
    }

    #[test]
    fn test_load_or_new_with_corrupted() {
        let temp_dir = tempfile::TempDir::new().unwrap();
        let path = temp_dir.path().join("corrupted.json");

        // Write invalid JSON
        std::fs::write(&path, "not valid json {{{").unwrap();

        let config = TaskTrackerConfig::default().with_stagnation_threshold(7);
        let tracker = TaskTracker::load_or_new(&path, config);

        // Should return a new tracker, not error
        assert!(tracker.tasks.is_empty());
        assert_eq!(tracker.config.stagnation_threshold, 7);
    }

    #[test]
    fn test_load_or_new_with_valid() {
        let temp_dir = tempfile::TempDir::new().unwrap();
        let path = temp_dir.path().join("valid.json");

        // Create and save a tracker
        let mut original = TaskTracker::default();
        original.parse_plan("### 1. Test").unwrap();
        original.save(&path).unwrap();

        // Load it back with different config
        let config = TaskTrackerConfig::default().with_stagnation_threshold(99);
        let tracker = TaskTracker::load_or_new(&path, config);

        // Should load the existing tracker, not create new
        assert_eq!(tracker.tasks.len(), 1);
    }

    #[test]
    fn test_auto_save_when_enabled() {
        let temp_dir = tempfile::TempDir::new().unwrap();
        let path = temp_dir.path().join("auto.json");

        let config = TaskTrackerConfig::default().with_auto_save(true);
        let tracker = TaskTracker::new(config);

        tracker.auto_save(&path).unwrap();
        assert!(path.exists());
    }

    #[test]
    fn test_auto_save_when_disabled() {
        let temp_dir = tempfile::TempDir::new().unwrap();
        let path = temp_dir.path().join("auto.json");

        let config = TaskTrackerConfig::default().with_auto_save(false);
        let tracker = TaskTracker::new(config);

        tracker.auto_save(&path).unwrap();
        assert!(!path.exists());
    }

    #[test]
    fn test_default_path() {
        let project_dir = std::path::Path::new("/home/user/project");
        let path = TaskTracker::default_path(project_dir);

        assert_eq!(path.to_string_lossy(), "/home/user/project/.ralph/task_tracker.json");
    }

    #[test]
    fn test_save_preserves_all_state() {
        let temp_dir = tempfile::TempDir::new().unwrap();
        let path = temp_dir.path().join("full.json");

        // Create a tracker with complex state
        let config = TaskTrackerConfig::default()
            .with_stagnation_threshold(5)
            .with_max_quality_failures(3);
        let mut tracker = TaskTracker::new(config);

        tracker.parse_plan(r#"
### 1. Phase 1.1: Setup
- [x] Create directories
- [ ] Configure tools

### 2. Phase 1.2: Build
- [ ] Write code

### 3. Phase 2.1: Test
- [ ] Write tests
"#).unwrap();

        // Add state to multiple tasks
        let task1_id = TaskId::parse("### 1. Phase 1.1: Setup").unwrap();
        let task2_id = TaskId::parse("### 2. Phase 1.2: Build").unwrap();
        let task3_id = TaskId::parse("### 3. Phase 2.1: Test").unwrap();

        tracker.start_task(&task1_id).unwrap();
        tracker.record_progress(2, 50).unwrap();
        tracker.submit_for_review(&task1_id).unwrap();
        tracker.complete_task(&task1_id).unwrap();

        tracker.start_task(&task2_id).unwrap();
        tracker.block_task(&task2_id, BlockReason::ExternalDependency {
            description: "Waiting for API".to_string(),
        }).unwrap();

        // Save and reload
        tracker.save(&path).unwrap();
        let loaded = TaskTracker::load(&path, TaskTrackerConfig::default()).unwrap();

        // Verify task 1 state
        let t1 = loaded.get_task(&task1_id).unwrap();
        assert_eq!(t1.state, TaskState::Complete);
        assert_eq!(t1.metrics.files_modified, 2);
        assert!(t1.transitions.len() >= 3);

        // Verify task 2 state
        let t2 = loaded.get_task(&task2_id).unwrap();
        assert_eq!(t2.state, TaskState::Blocked);
        assert!(matches!(t2.block_reason, Some(BlockReason::ExternalDependency { .. })));

        // Verify task 3 state (untouched)
        let t3 = loaded.get_task(&task3_id).unwrap();
        assert_eq!(t3.state, TaskState::NotStarted);

        // Verify config was saved
        assert_eq!(loaded.config.stagnation_threshold, 5);
        assert_eq!(loaded.config.max_quality_failures, 3);
    }

    #[test]
    fn test_save_preserves_timestamps() {
        let temp_dir = tempfile::TempDir::new().unwrap();
        let path = temp_dir.path().join("timestamps.json");

        let tracker = TaskTracker::default();
        let original_created = tracker.created_at;

        tracker.save(&path).unwrap();
        let loaded = TaskTracker::load(&path, TaskTrackerConfig::default()).unwrap();

        assert_eq!(loaded.created_at, original_created);
    }

    #[test]
    fn test_persistence_roundtrip_with_block_reasons() {
        let temp_dir = tempfile::TempDir::new().unwrap();
        let path = temp_dir.path().join("blocks.json");

        let mut tracker = TaskTracker::default();
        tracker.parse_plan("### 1. Task\n### 2. Task\n### 3. Task").unwrap();

        // Create different block reasons
        let t1 = TaskId::parse("### 1. Task").unwrap();
        let t2 = TaskId::parse("### 2. Task").unwrap();
        let t3 = TaskId::parse("### 3. Task").unwrap();

        tracker.start_task(&t1).unwrap();
        tracker.block_task(&t1, BlockReason::MaxAttempts { attempts: 5, max: 5 }).unwrap();

        tracker.start_task(&t2).unwrap();
        tracker.block_task(&t2, BlockReason::QualityGateFailure {
            gate: "clippy".to_string(),
            failures: 3,
        }).unwrap();

        tracker.start_task(&t3).unwrap();
        tracker.block_task(&t3, BlockReason::DependsOnTask { task_number: 1 }).unwrap();

        // Save and reload
        tracker.save(&path).unwrap();
        let loaded = TaskTracker::load(&path, TaskTrackerConfig::default()).unwrap();

        // Verify block reasons
        let l1 = loaded.get_task(&t1).unwrap();
        assert!(matches!(l1.block_reason, Some(BlockReason::MaxAttempts { attempts: 5, max: 5 })));

        let l2 = loaded.get_task(&t2).unwrap();
        if let Some(BlockReason::QualityGateFailure { gate, failures }) = &l2.block_reason {
            assert_eq!(gate, "clippy");
            assert_eq!(*failures, 3);
        } else {
            panic!("Expected QualityGateFailure block reason");
        }

        let l3 = loaded.get_task(&t3).unwrap();
        assert!(matches!(l3.block_reason, Some(BlockReason::DependsOnTask { task_number: 1 })));
    }
}
