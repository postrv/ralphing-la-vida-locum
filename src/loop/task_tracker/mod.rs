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
//!
//! # Startup Validation & Orphan Detection
//!
//! The task tracker persists state to `.ralph/task_tracker.json` between sessions.
//! When the plan changes while Ralph isn't running, stale tasks can cause issues.
//!
//! To prevent Ralph from getting stuck on orphaned tasks:
//!
//! 1. **Startup Validation**: `validate_on_startup(plan)` is called before the
//!    first loop iteration. It marks any tasks not in the current plan as orphaned
//!    and clears `current_task` if it points to an orphaned task.
//!
//! 2. **Orphan Skipping**: `select_next_task()` automatically skips tasks marked
//!    as orphaned, ensuring Ralph only works on tasks that exist in the current plan.
//!
//! 3. **Mid-Session Detection**: `check_for_orphaned_tasks()` runs whenever
//!    progress is detected, catching plan changes during a session.
//!
//! This defensive system ensures Ralph gracefully handles plan changes without
//! requiring manual intervention (like deleting task_tracker.json).

mod metrics;
mod parsing;
mod persistence;
mod state;

// Re-export public types to maintain API compatibility
pub use metrics::{TaskCounts, TaskMetrics};
pub use state::{BlockReason, TaskState, TaskTransition};

use anyhow::{bail, Result};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fmt;
use std::hash::Hash;
use std::path::PathBuf;

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
    pub(crate) title: String,
    /// Original header text for display
    original: String,
    /// Sprint subsection identifier (e.g., "7a", "6b") for sprint-scoped tasks
    #[serde(default)]
    subsection: Option<String>,
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
        use anyhow::Context;

        // Strip leading hashes and whitespace
        let stripped = header.trim_start_matches('#').trim();

        // Pattern: "Na. Title" (sprint subsection) or "N. [Phase X.Y: ]Title"
        let parts: Vec<&str> = stripped.splitn(2, ". ").collect();
        if parts.len() != 2 {
            bail!(
                "Invalid task header format: expected 'N. Title', got: {}",
                header
            );
        }

        let number_part = parts[0];
        let rest = parts[1];

        // Check for subsection format (e.g., "7a", "10b")
        let (number, subsection) = if number_part
            .chars()
            .last()
            .is_some_and(|c| c.is_ascii_lowercase())
        {
            // Extract numeric part and letter suffix
            let numeric: String = number_part
                .chars()
                .take_while(|c| c.is_ascii_digit())
                .collect();
            let number: u32 = numeric
                .parse()
                .with_context(|| format!("Invalid task number in header: {}", header))?;
            (number, Some(number_part.to_string()))
        } else {
            let number: u32 = number_part
                .parse()
                .with_context(|| format!("Invalid task number in header: {}", header))?;
            (number, None)
        };

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
            subsection,
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
            subsection: None,
        }
    }

    /// Get the sprint subsection identifier if present (e.g., "7a", "6b").
    #[must_use]
    pub fn subsection(&self) -> Option<&str> {
        self.subsection.as_deref()
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
    /// Sprint number this task belongs to (e.g., 7 for Sprint 7 tasks)
    #[serde(default)]
    pub sprint: Option<u32>,
    /// Whether this task is orphaned (not in current plan)
    #[serde(default)]
    pub orphaned: bool,
    /// Files this task is expected to affect (for scoped task selection).
    ///
    /// When set, the task is only prioritized if the changed files overlap.
    /// When None, the task is considered to potentially affect any file.
    #[serde(default)]
    pub affected_files: Option<Vec<PathBuf>>,
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
            sprint: None,
            orphaned: false,
            affected_files: None,
        }
    }

    /// Create a new task with sprint affiliation.
    #[must_use]
    pub fn new_with_sprint(id: TaskId, sprint: u32) -> Self {
        Self {
            id,
            state: TaskState::NotStarted,
            transitions: Vec::new(),
            metrics: TaskMetrics::new(),
            block_reason: None,
            checkboxes: Vec::new(),
            sprint: Some(sprint),
            orphaned: false,
            affected_files: None,
        }
    }

    /// Builder method to set affected files for scoped task selection.
    ///
    /// # Example
    ///
    /// ```
    /// use ralph::r#loop::task_tracker::{Task, TaskId};
    /// use std::path::PathBuf;
    ///
    /// let task_id = TaskId::parse("### 1. Update auth").unwrap();
    /// let task = Task::new(task_id)
    ///     .with_affected_files(vec![PathBuf::from("src/auth/mod.rs")]);
    /// ```
    #[must_use]
    pub fn with_affected_files(mut self, files: Vec<PathBuf>) -> Self {
        self.affected_files = Some(files);
        self
    }

    /// Set the affected files for this task.
    ///
    /// # Example
    ///
    /// ```
    /// use ralph::r#loop::task_tracker::{Task, TaskId};
    /// use std::path::PathBuf;
    ///
    /// let task_id = TaskId::parse("### 1. Update auth").unwrap();
    /// let mut task = Task::new(task_id);
    /// task.set_affected_files(vec![PathBuf::from("src/auth/mod.rs")]);
    /// ```
    pub fn set_affected_files(&mut self, files: Vec<PathBuf>) {
        self.affected_files = Some(files);
    }

    /// Check if this task affects a given file.
    ///
    /// If `affected_files` is `None`, the task is considered to potentially
    /// affect any file (conservative approach).
    ///
    /// # Example
    ///
    /// ```
    /// use ralph::r#loop::task_tracker::{Task, TaskId};
    /// use std::path::PathBuf;
    ///
    /// let task_id = TaskId::parse("### 1. Update auth").unwrap();
    /// let task = Task::new(task_id)
    ///     .with_affected_files(vec![PathBuf::from("src/auth/mod.rs")]);
    ///
    /// assert!(task.affects_file(&PathBuf::from("src/auth/mod.rs")));
    /// assert!(!task.affects_file(&PathBuf::from("src/other.rs")));
    /// ```
    #[must_use]
    pub fn affects_file(&self, file: &PathBuf) -> bool {
        // Delegate to affects_any_file with a single-element slice
        self.affects_any_file(std::slice::from_ref(file))
    }

    /// Check if this task affects any of the given files.
    ///
    /// Returns true if:
    /// - `affected_files` is `None` (conservative approach), or
    /// - Any of the given files are in `affected_files`
    #[must_use]
    pub fn affects_any_file(&self, files: &[PathBuf]) -> bool {
        match &self.affected_files {
            Some(affected) => files.iter().any(|f| affected.contains(f)),
            None => true, // Conservative: no affected_files means potentially affects all
        }
    }

    /// Check if this task has explicit affected files that match the given files.
    ///
    /// Unlike `affects_any_file`, this returns false if `affected_files` is `None`.
    /// Use this for prioritization where explicit matches should rank higher.
    #[must_use]
    pub fn has_explicit_affected_file_match(&self, files: &[PathBuf]) -> bool {
        match &self.affected_files {
            Some(affected) => files.iter().any(|f| affected.contains(f)),
            None => false, // No explicit affected_files means no explicit match
        }
    }

    /// Get the sprint number this task belongs to.
    #[must_use]
    pub fn sprint(&self) -> Option<u32> {
        self.sprint
    }

    /// Check if this task is orphaned (not in current plan).
    #[must_use]
    pub fn is_orphaned(&self) -> bool {
        self.orphaned
    }

    /// Mark this task as orphaned.
    pub fn mark_orphaned(&mut self) {
        self.orphaned = true;
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
// Validation Result
// ============================================================================

/// Result of validating the tracker against a plan.
///
/// Used by the manager to detect when the plan structure has changed
/// and identify tasks that are no longer in the current plan.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ValidationResult {
    /// Plan structure matches the tracker
    Valid,
    /// Plan structure has changed
    PlanChanged {
        /// Tasks in tracker that are not in the current plan
        orphaned_tasks: Vec<TaskId>,
    },
}

// ============================================================================
// Task Tracker
// ============================================================================

/// Task-level progress tracker.
///
/// Maintains state for all tasks and provides task selection and progress tracking.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskTracker {
    /// All tracked tasks keyed by ID
    #[serde(with = "persistence::tasks_serde")]
    pub tasks: HashMap<TaskId, Task>,
    /// Currently active task
    pub current_task: Option<TaskId>,
    /// Configuration
    pub config: TaskTrackerConfig,
    /// When the tracker was created
    pub created_at: DateTime<Utc>,
    /// When the tracker was last modified
    pub modified_at: DateTime<Utc>,
    /// Hash of the plan structure for invalidation detection
    #[serde(default)]
    pub(crate) plan_structure_hash: String,
    /// Current sprint number from "Current Focus" section
    #[serde(default)]
    pub(crate) focused_sprint: Option<u32>,
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
            plan_structure_hash: String::new(),
            focused_sprint: None,
        }
    }

    /// Get the hash of the plan structure.
    #[must_use]
    pub fn plan_hash(&self) -> &str {
        &self.plan_structure_hash
    }

    /// Get the current sprint from the "Current Focus" section.
    #[must_use]
    pub fn current_sprint(&self) -> Option<u32> {
        self.focused_sprint
    }

    /// Check if a sprint is complete (all tasks done or blocked).
    #[must_use]
    pub fn is_sprint_complete(&self, sprint: u32) -> bool {
        let sprint_tasks: Vec<_> = self
            .tasks
            .values()
            .filter(|t| t.sprint == Some(sprint))
            .collect();

        if sprint_tasks.is_empty() {
            return false;
        }

        sprint_tasks
            .iter()
            .all(|t| t.state == TaskState::Complete || t.state == TaskState::Blocked)
    }

    /// Get all tasks for a specific sprint.
    #[must_use]
    pub fn tasks_for_sprint(&self, sprint: u32) -> Vec<&Task> {
        self.tasks
            .values()
            .filter(|t| t.sprint == Some(sprint))
            .collect()
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
        let task = self
            .tasks
            .get_mut(task_id)
            .ok_or_else(|| anyhow::anyhow!("Task not found: {}", task_id))?;

        if !task.state.can_transition_to(TaskState::InProgress) {
            bail!(
                "Cannot start task {}: current state is {}",
                task_id,
                task.state
            );
        }

        let transition =
            TaskTransition::with_reason(task.state, TaskState::InProgress, "Task started");
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
        let task_id = self
            .current_task
            .clone()
            .ok_or_else(|| anyhow::anyhow!("No active task to record progress for"))?;

        let task = self
            .tasks
            .get_mut(&task_id)
            .ok_or_else(|| anyhow::anyhow!("Current task not found: {}", task_id))?;

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
        let task_id = self
            .current_task
            .clone()
            .ok_or_else(|| anyhow::anyhow!("No active task to record no-progress for"))?;

        let threshold = self.config.stagnation_threshold;

        let task = self
            .tasks
            .get_mut(&task_id)
            .ok_or_else(|| anyhow::anyhow!("Current task not found: {}", task_id))?;

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
        let task_id = self
            .current_task
            .clone()
            .ok_or_else(|| anyhow::anyhow!("No active task"))?;

        let task = self
            .tasks
            .get_mut(&task_id)
            .ok_or_else(|| anyhow::anyhow!("Current task not found: {}", task_id))?;

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
        let task = self
            .tasks
            .get_mut(task_id)
            .ok_or_else(|| anyhow::anyhow!("Task not found: {}", task_id))?;

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
        let task = self
            .tasks
            .get_mut(task_id)
            .ok_or_else(|| anyhow::anyhow!("Task not found: {}", task_id))?;

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
        let task = self
            .tasks
            .get_mut(task_id)
            .ok_or_else(|| anyhow::anyhow!("Task not found: {}", task_id))?;

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

        let task = self
            .tasks
            .get_mut(task_id)
            .ok_or_else(|| anyhow::anyhow!("Task not found: {}", task_id))?;

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
        let task = self
            .tasks
            .get_mut(task_id)
            .ok_or_else(|| anyhow::anyhow!("Task not found: {}", task_id))?;

        if !task.state.can_transition_to(TaskState::Complete) {
            bail!(
                "Cannot complete task {}: current state is {}",
                task_id,
                task.state
            );
        }

        let transition =
            TaskTransition::with_reason(task.state, TaskState::Complete, "Task completed");
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
        // Priority 1: Current task if still workable and not orphaned
        if let Some(ref current_id) = self.current_task {
            if let Some(task) = self.tasks.get(current_id) {
                if task.is_workable() && !task.is_orphaned() {
                    // Also check sprint boundary
                    if self.is_task_in_current_sprint(task) {
                        return Some(current_id);
                    }
                }
            }
        }

        // Priority 2: In-progress tasks from current sprint (not orphaned)
        let mut in_progress: Vec<&TaskId> = self
            .tasks
            .iter()
            .filter(|(_, t)| {
                t.state == TaskState::InProgress
                    && !t.is_orphaned()
                    && self.is_task_in_current_sprint(t)
            })
            .map(|(id, _)| id)
            .collect();
        in_progress.sort_by_key(|id| (id.number(), id.subsection().unwrap_or("")));
        if let Some(id) = in_progress.first() {
            return Some(id);
        }

        // Priority 3: Not-started tasks from current sprint (not orphaned)
        let mut not_started: Vec<&TaskId> = self
            .tasks
            .iter()
            .filter(|(_, t)| {
                t.state == TaskState::NotStarted
                    && !t.is_orphaned()
                    && self.is_task_in_current_sprint(t)
            })
            .map(|(id, _)| id)
            .collect();
        not_started.sort_by_key(|id| (id.number(), id.subsection().unwrap_or("")));
        if let Some(id) = not_started.first() {
            return Some(id);
        }

        None
    }

    /// Select the next task to work on with scoped prioritization.
    ///
    /// When a `ChangeScope` is provided, tasks that affect the changed files
    /// are prioritized over tasks that don't. This enables incremental execution
    /// where Ralph focuses on tasks relevant to recent changes.
    ///
    /// Priority order:
    /// 1. Current task if still workable, not orphaned, and in current sprint
    /// 2. In-progress tasks from current sprint that affect changed files
    /// 3. Not-started tasks from current sprint that affect changed files
    /// 4. In-progress tasks from current sprint (fallback)
    /// 5. Not-started tasks from current sprint (fallback)
    ///
    /// # Arguments
    ///
    /// * `scope` - The change scope defining which files have changed
    ///
    /// # Example
    ///
    /// ```
    /// use ralph::r#loop::task_tracker::{TaskTracker, TaskTrackerConfig, TaskId};
    /// use ralph::changes::ChangeScope;
    /// use std::path::PathBuf;
    ///
    /// let mut tracker = TaskTracker::new(TaskTrackerConfig::default());
    /// tracker.parse_plan("### 1. Update auth\n### 2. Update db").unwrap();
    ///
    /// let scope = ChangeScope::from_files(vec![PathBuf::from("src/db/mod.rs")]);
    /// let next = tracker.select_next_task_scoped(&scope);
    /// ```
    #[must_use]
    pub fn select_next_task_scoped(&self, scope: &ralph::changes::ChangeScope) -> Option<&TaskId> {
        let changed_files = scope.changed_files();

        // Priority 1: Current task if still workable and not orphaned
        if let Some(ref current_id) = self.current_task {
            if let Some(task) = self.tasks.get(current_id) {
                if task.is_workable() && !task.is_orphaned() && self.is_task_in_current_sprint(task)
                {
                    return Some(current_id);
                }
            }
        }

        // If no changed files, fall back to normal selection
        if changed_files.is_empty() {
            return self.select_next_task();
        }

        // Priority 2: In-progress tasks with EXPLICIT affected_files that match changed files
        let mut in_progress_explicit: Vec<&TaskId> = self
            .tasks
            .iter()
            .filter(|(_, t)| {
                t.state == TaskState::InProgress
                    && !t.is_orphaned()
                    && self.is_task_in_current_sprint(t)
                    && t.has_explicit_affected_file_match(changed_files)
            })
            .map(|(id, _)| id)
            .collect();
        // Log debug info about conservative matching for tracing
        #[cfg(debug_assertions)]
        for (id, task) in &self.tasks {
            if task.affects_any_file(changed_files)
                && !task.has_explicit_affected_file_match(changed_files)
            {
                tracing::trace!(
                    "Task {} has conservative match (no explicit affected_files)",
                    id
                );
            }
            // Check single file match for completeness
            if let Some(first_file) = changed_files.first() {
                if task.affects_file(first_file) {
                    tracing::trace!("Task {} affects file {:?}", id, first_file);
                }
            }
        }
        in_progress_explicit.sort_by_key(|id| (id.number(), id.subsection().unwrap_or("")));
        if let Some(id) = in_progress_explicit.first() {
            return Some(id);
        }

        // Priority 3: Not-started tasks with EXPLICIT affected_files that match changed files
        let mut not_started_explicit: Vec<&TaskId> = self
            .tasks
            .iter()
            .filter(|(_, t)| {
                t.state == TaskState::NotStarted
                    && !t.is_orphaned()
                    && self.is_task_in_current_sprint(t)
                    && t.has_explicit_affected_file_match(changed_files)
            })
            .map(|(id, _)| id)
            .collect();
        not_started_explicit.sort_by_key(|id| (id.number(), id.subsection().unwrap_or("")));
        if let Some(id) = not_started_explicit.first() {
            return Some(id);
        }

        // Priority 4: Fall back to in-progress tasks (no explicit match requirement)
        let mut in_progress: Vec<&TaskId> = self
            .tasks
            .iter()
            .filter(|(_, t)| {
                t.state == TaskState::InProgress
                    && !t.is_orphaned()
                    && self.is_task_in_current_sprint(t)
            })
            .map(|(id, _)| id)
            .collect();
        in_progress.sort_by_key(|id| (id.number(), id.subsection().unwrap_or("")));
        if let Some(id) = in_progress.first() {
            return Some(id);
        }

        // Priority 5: Fall back to not-started tasks (no explicit match requirement)
        let mut not_started: Vec<&TaskId> = self
            .tasks
            .iter()
            .filter(|(_, t)| {
                t.state == TaskState::NotStarted
                    && !t.is_orphaned()
                    && self.is_task_in_current_sprint(t)
            })
            .map(|(id, _)| id)
            .collect();
        not_started.sort_by_key(|id| (id.number(), id.subsection().unwrap_or("")));
        if let Some(id) = not_started.first() {
            return Some(id);
        }

        None
    }

    /// Check if a task belongs to the current sprint or has no sprint assigned.
    fn is_task_in_current_sprint(&self, task: &Task) -> bool {
        match (self.focused_sprint, task.sprint) {
            // If no current sprint set, accept all tasks
            (None, _) => true,
            // If task has no sprint assigned, accept it
            (_, None) => true,
            // If task sprint matches current sprint, accept it
            (Some(current), Some(task_sprint)) => current == task_sprint,
        }
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
                task.metrics.no_progress_count, self.config.stagnation_threshold
            ));
        }

        let quality_warning = self.config.max_quality_failures.saturating_sub(1);
        if task.metrics.quality_failures >= quality_warning {
            reasons.push(format!(
                "{} quality gate failures (max: {})",
                task.metrics.quality_failures, self.config.max_quality_failures
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
            lines.push(format!(
                "- Checkbox progress: {:.0}%",
                current.completion_percentage()
            ));

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
        let blocked: Vec<&Task> = self
            .tasks
            .values()
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

    /// Get the next workable task ID with optional scoped prioritization.
    ///
    /// When `scope` is provided and has changed files, tasks that explicitly
    /// affect those files are prioritized. This enables incremental execution
    /// where Ralph focuses on tasks relevant to recent changes.
    ///
    /// # Arguments
    ///
    /// * `scope` - Optional change scope for scoped prioritization
    ///
    /// # Example
    ///
    /// ```
    /// use ralph::r#loop::task_tracker::{TaskTracker, TaskTrackerConfig, TaskId};
    /// use ralph::changes::ChangeScope;
    /// use std::path::PathBuf;
    ///
    /// let mut tracker = TaskTracker::new(TaskTrackerConfig::default());
    /// tracker.parse_plan("### 1. Task A\n### 2. Task B").unwrap();
    ///
    /// // Without scope - normal selection
    /// let next = tracker.next_task_with_scope(None);
    ///
    /// // With scope - scoped prioritization
    /// let scope = ChangeScope::from_files(vec![PathBuf::from("src/main.rs")]);
    /// let next = tracker.next_task_with_scope(Some(&scope));
    /// ```
    #[must_use]
    pub fn next_task_with_scope(
        &self,
        scope: Option<&ralph::changes::ChangeScope>,
    ) -> Option<TaskId> {
        match scope {
            Some(s) if s.has_changes() => self.select_next_task_scoped(s).cloned(),
            _ => self.select_next_task().cloned(),
        }
    }

    /// Check if all workable tasks are done.
    #[must_use]
    pub fn is_all_done(&self) -> bool {
        self.tasks
            .values()
            .all(|t| t.state == TaskState::Complete || t.state == TaskState::Blocked)
    }

    /// Get count of remaining tasks (not complete, not blocked).
    #[must_use]
    pub fn remaining_count(&self) -> usize {
        self.tasks
            .values()
            .filter(|t| t.state != TaskState::Complete && t.state != TaskState::Blocked)
            .count()
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
    use std::collections::HashMap;

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

    #[test]
    fn test_task_id_parse_with_subsection() {
        // Format: ### 7a. Task Title (sprint task with subsection)
        let id = TaskId::parse("### 7a. QualityGate Trait Refactor").unwrap();
        assert_eq!(id.subsection(), Some("7a"));
        assert_eq!(id.title(), "QualityGate Trait Refactor");
    }

    #[test]
    fn test_task_id_parse_numeric_only() {
        // Format: ### 1. Task Title (simple numeric task)
        let id = TaskId::parse("### 1. Setup project").unwrap();
        assert_eq!(id.subsection(), None);
        assert_eq!(id.number(), 1);
    }

    #[test]
    fn test_task_id_subsection_various_formats() {
        // Test various subsection formats
        let id_a = TaskId::parse("### 6a. Language Enum").unwrap();
        assert_eq!(id_a.subsection(), Some("6a"));

        let id_b = TaskId::parse("### 6b. Language Detector").unwrap();
        assert_eq!(id_b.subsection(), Some("6b"));

        let id_c = TaskId::parse("### 10a. Polyglot Prompt").unwrap();
        assert_eq!(id_c.subsection(), Some("10a"));
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
        task.checkboxes = vec![("Item 1".to_string(), true), ("Item 2".to_string(), true)];
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
    // TaskTracker Tests
    // ========================================================================

    #[test]
    fn test_task_tracker_new() {
        let config = TaskTrackerConfig::default();
        let tracker = TaskTracker::new(config.clone());

        assert!(tracker.tasks.is_empty());
        assert!(tracker.current_task.is_none());
        assert_eq!(
            tracker.config.max_attempts_per_task,
            config.max_attempts_per_task
        );
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
    // Workable Tasks Tests
    // ========================================================================

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
        tracker
            .get_task_mut(&TaskId::parse("### 2. Second").unwrap())
            .unwrap()
            .state = TaskState::InProgress;
        tracker
            .get_task_mut(&TaskId::parse("### 3. Third").unwrap())
            .unwrap()
            .state = TaskState::Complete;
        tracker
            .get_task_mut(&TaskId::parse("### 4. Fourth").unwrap())
            .unwrap()
            .state = TaskState::InProgress;

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
        tracker
            .get_task_mut(&TaskId::parse("### 1. Task 1").unwrap())
            .unwrap()
            .state = TaskState::Complete;
        tracker
            .get_task_mut(&TaskId::parse("### 2. Task 2").unwrap())
            .unwrap()
            .state = TaskState::InProgress;
        tracker
            .get_task_mut(&TaskId::parse("### 3. Task 3").unwrap())
            .unwrap()
            .state = TaskState::Blocked;
        tracker
            .get_task_mut(&TaskId::parse("### 4. Task 4").unwrap())
            .unwrap()
            .state = TaskState::InReview;
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

        tracker
            .get_task_mut(&TaskId::parse("### 1. Task 1").unwrap())
            .unwrap()
            .state = TaskState::Complete;
        tracker
            .get_task_mut(&TaskId::parse("### 2. Task 2").unwrap())
            .unwrap()
            .state = TaskState::Blocked;

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
        tracker
            .get_task_mut(&TaskId::parse("### 2. Complete task").unwrap())
            .unwrap()
            .state = TaskState::Complete;

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
        tracker
            .block_task(
                &task_id,
                BlockReason::Other {
                    reason: "Test block".to_string(),
                },
            )
            .unwrap();

        // Now start should succeed (transition from Blocked -> InProgress)
        tracker.start_task(&task_id).unwrap();
        assert_eq!(
            tracker.get_task(&task_id).unwrap().state,
            TaskState::InProgress
        );
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
        let mut tracker =
            TaskTracker::new(TaskTrackerConfig::default().with_stagnation_threshold(3));
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

        let reason = BlockReason::Other {
            reason: "Test".to_string(),
        };
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
        tracker
            .block_task(
                &task_id,
                BlockReason::Other {
                    reason: "Test".to_string(),
                },
            )
            .unwrap();

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
        let mut tracker =
            TaskTracker::new(TaskTrackerConfig::default().with_max_quality_failures(3));
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
        let mut tracker =
            TaskTracker::new(TaskTrackerConfig::default().with_max_quality_failures(2));
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
        assert_eq!(
            tracker.get_task(&task_id).unwrap().state,
            TaskState::InProgress
        );

        // Record some progress
        tracker.record_progress(3, 50).unwrap();

        // Submit for review
        tracker.submit_for_review(&task_id).unwrap();
        assert_eq!(
            tracker.get_task(&task_id).unwrap().state,
            TaskState::InReview
        );

        // Review passes
        tracker.update_review(&task_id, "tests", true).unwrap();

        // Complete
        tracker.complete_task(&task_id).unwrap();
        assert_eq!(
            tracker.get_task(&task_id).unwrap().state,
            TaskState::Complete
        );

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
        tracker
            .block_task(
                &task_id,
                BlockReason::Other {
                    reason: "Test".to_string(),
                },
            )
            .unwrap();
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
    // Task Selection Tests
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
        tracker
            .parse_plan("### 1. Task 1\n### 2. Task 2\n### 3. Task 3")
            .unwrap();

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
        tracker
            .parse_plan("### 1. Task 1\n### 2. Task 2\n### 3. Task 3")
            .unwrap();

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
        tracker
            .block_task(
                &task1_id,
                BlockReason::Other {
                    reason: "X".to_string(),
                },
            )
            .unwrap();
        tracker.start_task(&task2_id).unwrap();
        tracker
            .block_task(
                &task2_id,
                BlockReason::Other {
                    reason: "Y".to_string(),
                },
            )
            .unwrap();

        assert!(tracker.select_next_task().is_none());
    }

    #[test]
    fn test_is_task_stuck_not_stuck_initially() {
        let mut tracker =
            TaskTracker::new(TaskTrackerConfig::default().with_stagnation_threshold(3));
        tracker.parse_plan("### 1. Test task").unwrap();

        let task_id = TaskId::parse("### 1. Test task").unwrap();
        assert!(!tracker.is_task_stuck(&task_id));
    }

    #[test]
    fn test_is_task_stuck_approaching_stagnation() {
        let mut tracker =
            TaskTracker::new(TaskTrackerConfig::default().with_stagnation_threshold(3));
        tracker.parse_plan("### 1. Test task").unwrap();

        let task_id = TaskId::parse("### 1. Test task").unwrap();
        tracker.start_task(&task_id).unwrap();

        // Two no-progress iterations = approaching threshold (3-1=2)
        tracker
            .get_task_mut(&task_id)
            .unwrap()
            .metrics
            .no_progress_count = 2;

        assert!(tracker.is_task_stuck(&task_id));
    }

    #[test]
    fn test_is_task_stuck_approaching_quality_failures() {
        let mut tracker =
            TaskTracker::new(TaskTrackerConfig::default().with_max_quality_failures(3));
        tracker.parse_plan("### 1. Test task").unwrap();

        let task_id = TaskId::parse("### 1. Test task").unwrap();
        tracker.start_task(&task_id).unwrap();

        // Two quality failures = approaching threshold (3-1=2)
        tracker
            .get_task_mut(&task_id)
            .unwrap()
            .metrics
            .quality_failures = 2;

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
        let mut tracker =
            TaskTracker::new(TaskTrackerConfig::default().with_stagnation_threshold(5));
        tracker.parse_plan("### 1. Test task").unwrap();

        let task_id = TaskId::new_for_test(1, "Test task");
        assert!(tracker.get_stuck_reason(&task_id).is_none());
    }

    #[test]
    fn test_get_stuck_reason_stagnation() {
        let mut tracker =
            TaskTracker::new(TaskTrackerConfig::default().with_stagnation_threshold(3));
        tracker.parse_plan("### 1. Test task").unwrap();

        let task_id = TaskId::new_for_test(1, "Test task");
        tracker.start_task(&task_id).unwrap();
        tracker
            .get_task_mut(&task_id)
            .unwrap()
            .metrics
            .no_progress_count = 2;

        let reason = tracker.get_stuck_reason(&task_id).unwrap();
        assert!(reason.contains("no progress for 2 iterations"));
        assert!(reason.contains("threshold: 3"));
    }

    #[test]
    fn test_get_stuck_reason_quality_failures() {
        let mut tracker =
            TaskTracker::new(TaskTrackerConfig::default().with_max_quality_failures(3));
        tracker.parse_plan("### 1. Test task").unwrap();

        let task_id = TaskId::new_for_test(1, "Test task");
        tracker.start_task(&task_id).unwrap();
        tracker
            .get_task_mut(&task_id)
            .unwrap()
            .metrics
            .quality_failures = 2;

        let reason = tracker.get_stuck_reason(&task_id).unwrap();
        assert!(reason.contains("2 quality gate failures"));
        assert!(reason.contains("max: 3"));
    }

    #[test]
    fn test_get_stuck_reason_multiple_reasons() {
        let mut tracker = TaskTracker::new(
            TaskTrackerConfig::default()
                .with_stagnation_threshold(3)
                .with_max_quality_failures(3),
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
        tracker
            .get_task_mut(&task_id)
            .unwrap()
            .metrics
            .no_progress_count = 2;
        tracker
            .get_task_mut(&task_id)
            .unwrap()
            .metrics
            .quality_failures = 1;

        let summary = tracker.get_context_summary();
        assert!(summary.contains("No progress for 2 iteration(s)"));
        assert!(summary.contains("Quality gate failures: 1"));
    }

    #[test]
    fn test_get_context_summary_shows_stuck_warning() {
        let mut tracker =
            TaskTracker::new(TaskTrackerConfig::default().with_stagnation_threshold(3));
        tracker.parse_plan("### 1. Test task").unwrap();

        let task_id = TaskId::parse("### 1. Test task").unwrap();
        tracker.start_task(&task_id).unwrap();
        tracker
            .get_task_mut(&task_id)
            .unwrap()
            .metrics
            .no_progress_count = 2;

        let summary = tracker.get_context_summary();
        assert!(summary.contains("**WARNING**: Task appears stuck"));
    }

    #[test]
    fn test_get_context_summary_shows_blocked_tasks() {
        let mut tracker = TaskTracker::default();
        tracker.parse_plan("### 1. Task 1\n### 2. Task 2").unwrap();

        let task1_id = TaskId::parse("### 1. Task 1").unwrap();
        tracker.start_task(&task1_id).unwrap();
        tracker
            .block_task(
                &task1_id,
                BlockReason::ExternalDependency {
                    description: "Waiting for API".to_string(),
                },
            )
            .unwrap();

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
        tracker
            .block_task(
                &task_id,
                BlockReason::Other {
                    reason: "X".to_string(),
                },
            )
            .unwrap();

        assert!(tracker.is_all_done()); // Blocked counts as "done" (no more work possible)
    }

    #[test]
    fn test_is_all_done_mixed() {
        let mut tracker = TaskTracker::default();
        tracker
            .parse_plan("### 1. Task 1\n### 2. Task 2\n### 3. Task 3")
            .unwrap();

        let task1_id = TaskId::parse("### 1. Task 1").unwrap();
        let task2_id = TaskId::parse("### 2. Task 2").unwrap();

        tracker.start_task(&task1_id).unwrap();
        tracker.complete_task(&task1_id).unwrap();
        tracker.start_task(&task2_id).unwrap();
        tracker
            .block_task(
                &task2_id,
                BlockReason::Other {
                    reason: "X".to_string(),
                },
            )
            .unwrap();
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
        tracker
            .parse_plan("### 1. Task 1\n### 2. Task 2\n### 3. Task 3")
            .unwrap();
        assert_eq!(tracker.remaining_count(), 3);
    }

    #[test]
    fn test_remaining_count_excludes_complete() {
        let mut tracker = TaskTracker::default();
        tracker
            .parse_plan("### 1. Task 1\n### 2. Task 2\n### 3. Task 3")
            .unwrap();

        let task1_id = TaskId::parse("### 1. Task 1").unwrap();
        tracker.start_task(&task1_id).unwrap();
        tracker.complete_task(&task1_id).unwrap();

        assert_eq!(tracker.remaining_count(), 2);
    }

    #[test]
    fn test_remaining_count_excludes_blocked() {
        let mut tracker = TaskTracker::default();
        tracker
            .parse_plan("### 1. Task 1\n### 2. Task 2\n### 3. Task 3")
            .unwrap();

        let task1_id = TaskId::parse("### 1. Task 1").unwrap();
        tracker.start_task(&task1_id).unwrap();
        tracker
            .block_task(
                &task1_id,
                BlockReason::Other {
                    reason: "X".to_string(),
                },
            )
            .unwrap();

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
        tracker
            .block_task(
                &task2_id,
                BlockReason::Other {
                    reason: "X".to_string(),
                },
            )
            .unwrap();

        assert_eq!(tracker.remaining_count(), 0);
    }

    // ========================================================================
    // Sprint Tests
    // ========================================================================

    #[test]
    fn test_select_next_task_respects_current_sprint() {
        let plan = r#"
## Current Focus: Sprint 7 (Quality Gates)

## Sprint 6: Detection
### 6a. Language Enum
- [x] Complete

## Sprint 7: Quality Gates
### 7a. QualityGate Trait
- [ ] Incomplete
"#;
        let mut tracker = TaskTracker::default();
        tracker.parse_plan(plan).unwrap();

        // Even though Sprint 6 task exists, Sprint 7 is current focus
        let selected = tracker.select_next_task();
        if let Some(id) = selected {
            let task = tracker.get_task(id).unwrap();
            assert_eq!(task.sprint(), Some(7));
        }
    }

    #[test]
    fn test_is_sprint_complete_true_when_all_done() {
        let plan = r#"
## Current Focus: Sprint 7

## Sprint 7: Quality Gates
### 7a. Task A
- [x] Done

### 7b. Task B
- [x] Done
"#;
        let mut tracker = TaskTracker::default();
        tracker.parse_plan(plan).unwrap();

        // Mark all tasks complete
        let task_a = TaskId::parse("### 7a. Task A").unwrap();
        let task_b = TaskId::parse("### 7b. Task B").unwrap();
        tracker.start_task(&task_a).unwrap();
        tracker.complete_task(&task_a).unwrap();
        tracker.start_task(&task_b).unwrap();
        tracker.complete_task(&task_b).unwrap();

        assert!(tracker.is_sprint_complete(7));
    }

    #[test]
    fn test_is_sprint_complete_false_when_incomplete() {
        let plan = r#"
## Current Focus: Sprint 7

## Sprint 7: Quality Gates
### 7a. Task A
- [ ] Not done
"#;
        let mut tracker = TaskTracker::default();
        tracker.parse_plan(plan).unwrap();

        assert!(!tracker.is_sprint_complete(7));
    }

    #[test]
    fn test_sprint_tasks_returns_only_sprint_tasks() {
        let plan = r#"
## Sprint 6: Detection
### 6a. Detect Languages
- [ ] Item

## Sprint 7: Quality Gates
### 7a. QualityGate Trait
- [ ] Item

### 7b. Python Gates
- [ ] Item
"#;
        let mut tracker = TaskTracker::default();
        tracker.parse_plan(plan).unwrap();

        let sprint_7_tasks = tracker.tasks_for_sprint(7);
        assert_eq!(sprint_7_tasks.len(), 2);
        for task in sprint_7_tasks {
            assert_eq!(task.sprint(), Some(7));
        }
    }

    #[test]
    fn test_select_next_task_within_sprint_boundary() {
        let plan = r#"
## Current Focus: Sprint 7

## Sprint 6: Detection (Complete)
### 6a. Old Task
- [x] Done

## Sprint 7: Quality Gates
### 7a. Current Task
- [ ] Not done

## Sprint 8: Future Sprint
### 8a. Future Task
- [ ] Not done
"#;
        let mut tracker = TaskTracker::default();
        tracker.parse_plan(plan).unwrap();

        // Should select from Sprint 7, not Sprint 8
        let selected = tracker.select_next_task();
        assert!(selected.is_some());

        let id = selected.unwrap();
        let task = tracker.get_task(id).unwrap();
        assert_eq!(task.sprint(), Some(7));
        assert_ne!(task.sprint(), Some(8)); // Must not jump to future sprint
    }

    #[test]
    fn test_select_next_task_skips_orphaned() {
        let plan_v1 = r#"
## Current Focus: Sprint 7

## Sprint 6: Old Sprint
### 6a. Old Task
- [ ] Item

## Sprint 7: Current Sprint
### 7a. Current Task
- [ ] Item
"#;
        let plan_v2 = r#"
## Current Focus: Sprint 7

## Sprint 7: Current Sprint
### 7a. Current Task
- [ ] Item
"#;

        let mut tracker = TaskTracker::default();
        tracker.parse_plan(plan_v1).unwrap();

        // Mark orphaned tasks
        tracker.mark_orphaned_tasks(plan_v2);

        // Select should skip orphaned task 6a
        let selected = tracker.select_next_task();
        if let Some(id) = selected {
            assert_eq!(id.subsection(), Some("7a"));
        }
    }

    // ========================================================================
    // Startup Validation Tests (Sprint 20)
    // ========================================================================

    #[test]
    fn test_validate_on_startup_marks_orphaned_tasks() {
        // Simulate session 1: Parse plan v1 with two tasks
        let plan_v1 = r#"
## Current Sprint
### 1. Task One
- [ ] Item

### 2. Task Two
- [ ] Item
"#;
        let mut tracker = TaskTracker::default();
        tracker.parse_plan(plan_v1).unwrap();

        // Set Task Two as current
        let task_two = TaskId::parse("### 2. Task Two").unwrap();
        tracker.set_current(&task_two).unwrap();
        assert!(tracker.current_task.is_some());

        // Simulate session 2: Plan changed, Task Two removed
        let plan_v2 = r#"
## Current Sprint
### 1. Task One
- [ ] Item

### 3. Task Three
- [ ] Item
"#;

        // Call validate_on_startup with the new plan
        tracker.validate_on_startup(plan_v2);

        // Task Two should now be orphaned
        let task = tracker.get_task(&task_two).unwrap();
        assert!(task.is_orphaned(), "Task Two should be marked as orphaned");

        // current_task should be cleared because it was orphaned
        assert!(
            tracker.current_task.is_none(),
            "current_task should be cleared when orphaned"
        );
    }

    #[test]
    fn test_validate_on_startup_keeps_valid_current_task() {
        let plan = r#"
## Current Sprint
### 1. Task One
- [ ] Item
"#;
        let mut tracker = TaskTracker::default();
        tracker.parse_plan(plan).unwrap();

        let task_one = TaskId::parse("### 1. Task One").unwrap();
        tracker.set_current(&task_one).unwrap();

        // Validate with same plan - current task should remain
        tracker.validate_on_startup(plan);

        assert!(
            tracker.current_task.is_some(),
            "current_task should remain when not orphaned"
        );
        assert_eq!(tracker.current_task.as_ref().unwrap(), &task_one);
    }

    #[test]
    fn test_clear_current_task_if_orphaned() {
        let plan_v1 = r#"
## Current Sprint
### 1. Old Task
- [ ] Item
"#;
        let mut tracker = TaskTracker::default();
        tracker.parse_plan(plan_v1).unwrap();

        let old_task = TaskId::parse("### 1. Old Task").unwrap();
        tracker.set_current(&old_task).unwrap();

        // Mark the task as orphaned
        let plan_v2 = r#"
## Current Sprint
### 2. New Task
- [ ] Item
"#;
        tracker.mark_orphaned_tasks(plan_v2);

        // Clear current task if orphaned
        let was_cleared = tracker.clear_current_task_if_orphaned();

        assert!(
            was_cleared,
            "Should return true when current task was orphaned and cleared"
        );
        assert!(tracker.current_task.is_none());
    }

    #[test]
    fn test_clear_current_task_if_orphaned_noop_when_valid() {
        let plan = r#"
## Current Sprint
### 1. Valid Task
- [ ] Item
"#;
        let mut tracker = TaskTracker::default();
        tracker.parse_plan(plan).unwrap();

        let task = TaskId::parse("### 1. Valid Task").unwrap();
        tracker.set_current(&task).unwrap();

        // Task is not orphaned
        let was_cleared = tracker.clear_current_task_if_orphaned();

        assert!(
            !was_cleared,
            "Should return false when current task is not orphaned"
        );
        assert!(tracker.current_task.is_some());
    }

    #[test]
    fn test_clear_current_task_if_orphaned_noop_when_no_current() {
        let plan = r#"
## Current Sprint
### 1. Task
- [ ] Item
"#;
        let mut tracker = TaskTracker::default();
        tracker.parse_plan(plan).unwrap();

        // No current task set
        let was_cleared = tracker.clear_current_task_if_orphaned();

        assert!(!was_cleared, "Should return false when no current task");
    }

    // ========================================================================
    // Defensive Task Selection Tests (Sprint 20, Phase 20.2)
    // ========================================================================

    #[test]
    fn test_task_exists_in_plan_returns_true_for_present_task() {
        let plan = r#"
## Current Sprint
### 1. Task One
- [ ] Item

### 2. Task Two
- [ ] Item
"#;
        let task_id = TaskId::parse("### 1. Task One").unwrap();
        let tracker = TaskTracker::default();

        assert!(
            tracker.task_exists_in_plan(&task_id, plan),
            "Task One should exist in the plan"
        );
    }

    #[test]
    fn test_task_exists_in_plan_returns_false_for_absent_task() {
        let plan = r#"
## Current Sprint
### 1. Task One
- [ ] Item
"#;
        let task_id = TaskId::parse("### 2. Missing Task").unwrap();
        let tracker = TaskTracker::default();

        assert!(
            !tracker.task_exists_in_plan(&task_id, plan),
            "Missing Task should not exist in the plan"
        );
    }

    #[test]
    fn test_task_exists_in_plan_handles_sprint_subsection_format() {
        let plan = r#"
## Sprint 7: Quality Gates
### 7a. QualityGate Trait
- [ ] Item
"#;
        let task_id = TaskId::parse("### 7a. QualityGate Trait").unwrap();
        let tracker = TaskTracker::default();

        assert!(
            tracker.task_exists_in_plan(&task_id, plan),
            "Sprint subsection task should exist in the plan"
        );
    }

    #[test]
    fn test_select_next_task_uses_orphan_flag_correctly() {
        // This test verifies that select_next_task relies on the orphan flag
        // set by validate_on_startup, which is the defensive mechanism
        let plan_v1 = r#"
## Current Sprint
### 1. Old Task
- [ ] Item

### 2. New Task
- [ ] Item
"#;
        let plan_v2 = r#"
## Current Sprint
### 2. New Task
- [ ] Item
"#;

        let mut tracker = TaskTracker::default();
        tracker.parse_plan(plan_v1).unwrap();

        // Set Old Task as current
        let old_task = TaskId::parse("### 1. Old Task").unwrap();
        tracker.set_current(&old_task).unwrap();

        // Simulate startup with changed plan
        tracker.validate_on_startup(plan_v2);

        // select_next_task should NOT return the orphaned old task
        let selected = tracker.select_next_task();
        assert!(selected.is_some(), "Should find a task");

        let selected_id = selected.unwrap();
        // Should select Task 2, not the orphaned Task 1
        assert!(
            selected_id.title().contains("New Task"),
            "Should select New Task, not the orphaned Old Task"
        );
    }

    // ========================================================================
    // Phase 26.4: Scoped Task Selection Tests (TDD - Written First)
    // ========================================================================

    #[test]
    fn test_task_affected_files_default_is_none() {
        let task_id = TaskId::parse("### 1. Test Task").unwrap();
        let task = Task::new(task_id);

        assert!(
            task.affected_files.is_none(),
            "New task should have no affected files by default"
        );
    }

    #[test]
    fn test_task_with_affected_files() {
        let task_id = TaskId::parse("### 1. Test Task").unwrap();
        let files = vec![
            std::path::PathBuf::from("src/main.rs"),
            std::path::PathBuf::from("src/lib.rs"),
        ];
        let task = Task::new(task_id).with_affected_files(files.clone());

        assert_eq!(
            task.affected_files,
            Some(files),
            "Task should have the specified affected files"
        );
    }

    #[test]
    fn test_task_set_affected_files() {
        let task_id = TaskId::parse("### 1. Test Task").unwrap();
        let mut task = Task::new(task_id);

        let files = vec![std::path::PathBuf::from("src/main.rs")];
        task.set_affected_files(files.clone());

        assert_eq!(
            task.affected_files,
            Some(files),
            "set_affected_files should update the affected files"
        );
    }

    #[test]
    fn test_task_affects_file_when_matching() {
        let task_id = TaskId::parse("### 1. Test Task").unwrap();
        let files = vec![
            std::path::PathBuf::from("src/main.rs"),
            std::path::PathBuf::from("src/lib.rs"),
        ];
        let task = Task::new(task_id).with_affected_files(files);

        assert!(
            task.affects_file(&std::path::PathBuf::from("src/main.rs")),
            "Task should affect a file in its affected_files list"
        );
    }

    #[test]
    fn test_task_affects_file_when_not_matching() {
        let task_id = TaskId::parse("### 1. Test Task").unwrap();
        let files = vec![std::path::PathBuf::from("src/main.rs")];
        let task = Task::new(task_id).with_affected_files(files);

        assert!(
            !task.affects_file(&std::path::PathBuf::from("src/other.rs")),
            "Task should not affect a file not in its affected_files list"
        );
    }

    #[test]
    fn test_task_affects_file_when_no_affected_files() {
        let task_id = TaskId::parse("### 1. Test Task").unwrap();
        let task = Task::new(task_id);

        // When no affected_files specified, task is considered to potentially affect all files
        assert!(
            task.affects_file(&std::path::PathBuf::from("src/any.rs")),
            "Task with no affected_files should match any file (conservative)"
        );
    }

    #[test]
    fn test_task_affects_any_file_with_matches() {
        let task_id = TaskId::parse("### 1. Test Task").unwrap();
        let files = vec![
            std::path::PathBuf::from("src/main.rs"),
            std::path::PathBuf::from("src/lib.rs"),
        ];
        let task = Task::new(task_id).with_affected_files(files);

        let check_files = vec![
            std::path::PathBuf::from("src/lib.rs"),
            std::path::PathBuf::from("src/other.rs"),
        ];

        assert!(
            task.affects_any_file(&check_files),
            "Task should affect any file when at least one matches"
        );
    }

    #[test]
    fn test_task_affects_any_file_without_matches() {
        let task_id = TaskId::parse("### 1. Test Task").unwrap();
        let files = vec![std::path::PathBuf::from("src/main.rs")];
        let task = Task::new(task_id).with_affected_files(files);

        let check_files = vec![
            std::path::PathBuf::from("src/other.rs"),
            std::path::PathBuf::from("src/another.rs"),
        ];

        assert!(
            !task.affects_any_file(&check_files),
            "Task should not affect any file when none match"
        );
    }

    #[test]
    fn test_task_affects_any_file_when_no_affected_files() {
        let task_id = TaskId::parse("### 1. Test Task").unwrap();
        let task = Task::new(task_id);

        let check_files = vec![std::path::PathBuf::from("src/any.rs")];

        // Conservative: no affected_files means potentially affects all
        assert!(
            task.affects_any_file(&check_files),
            "Task with no affected_files should match any files (conservative)"
        );
    }

    #[test]
    fn test_task_has_explicit_affected_file_match() {
        let task_id = TaskId::parse("### 1. Test Task").unwrap();
        let files = vec![std::path::PathBuf::from("src/main.rs")];
        let task = Task::new(task_id).with_affected_files(files);

        let check_files = vec![std::path::PathBuf::from("src/main.rs")];
        assert!(
            task.has_explicit_affected_file_match(&check_files),
            "Task with explicit affected_files should match"
        );
    }

    #[test]
    fn test_task_has_explicit_affected_file_match_returns_false_when_none() {
        let task_id = TaskId::parse("### 1. Test Task").unwrap();
        let task = Task::new(task_id); // No affected_files set

        let check_files = vec![std::path::PathBuf::from("src/any.rs")];

        // Unlike affects_any_file, this should return false when affected_files is None
        assert!(
            !task.has_explicit_affected_file_match(&check_files),
            "Task without affected_files should NOT have explicit match"
        );
    }

    #[test]
    fn test_select_next_task_scoped_prioritizes_matching_tasks() {
        use ralph::changes::ChangeScope;

        let plan = r#"
## Current Sprint
### 1. Update auth module
- [ ] Item

### 2. Update database module
- [ ] Item
"#;

        let mut tracker = TaskTracker::default();
        tracker.parse_plan(plan).unwrap();

        // Set affected files for tasks
        let task1_id = TaskId::parse("### 1. Update auth module").unwrap();
        let task2_id = TaskId::parse("### 2. Update database module").unwrap();

        tracker
            .get_task_mut(&task1_id)
            .unwrap()
            .set_affected_files(vec![std::path::PathBuf::from("src/auth/mod.rs")]);
        tracker
            .get_task_mut(&task2_id)
            .unwrap()
            .set_affected_files(vec![std::path::PathBuf::from("src/db/mod.rs")]);

        // Create a scope with only db files changed
        let scope = ChangeScope::from_files(vec![std::path::PathBuf::from("src/db/mod.rs")]);

        // Task 2 should be prioritized because it affects changed files
        let selected = tracker.select_next_task_scoped(&scope);
        assert!(selected.is_some(), "Should find a task");

        let selected_id = selected.unwrap();
        assert!(
            selected_id.title().contains("database"),
            "Should prioritize task affecting changed files"
        );
    }

    #[test]
    fn test_select_next_task_scoped_falls_back_to_normal_selection() {
        use ralph::changes::ChangeScope;

        let plan = r#"
## Current Sprint
### 1. First task
- [ ] Item

### 2. Second task
- [ ] Item
"#;

        let mut tracker = TaskTracker::default();
        tracker.parse_plan(plan).unwrap();

        // Set affected files for tasks that don't match the scope
        let task1_id = TaskId::parse("### 1. First task").unwrap();
        let task2_id = TaskId::parse("### 2. Second task").unwrap();

        tracker
            .get_task_mut(&task1_id)
            .unwrap()
            .set_affected_files(vec![std::path::PathBuf::from("src/a.rs")]);
        tracker
            .get_task_mut(&task2_id)
            .unwrap()
            .set_affected_files(vec![std::path::PathBuf::from("src/b.rs")]);

        // Scope has files not matching any task
        let scope = ChangeScope::from_files(vec![std::path::PathBuf::from("src/other.rs")]);

        // Should fall back to normal priority (task 1 first by number)
        let selected = tracker.select_next_task_scoped(&scope);
        assert!(
            selected.is_some(),
            "Should find a task even without matches"
        );

        let selected_id = selected.unwrap();
        assert!(
            selected_id.title().contains("First"),
            "Should fall back to normal order when no match"
        );
    }

    #[test]
    fn test_select_next_task_scoped_respects_in_progress_priority() {
        use ralph::changes::ChangeScope;

        let plan = r#"
## Current Sprint
### 1. First task
- [ ] Item

### 2. Second task
- [ ] Item
"#;

        let mut tracker = TaskTracker::default();
        tracker.parse_plan(plan).unwrap();

        // Start task 1 (make it in-progress)
        let task1_id = TaskId::parse("### 1. First task").unwrap();
        let task2_id = TaskId::parse("### 2. Second task").unwrap();
        tracker.start_task(&task1_id).unwrap();

        // Task 2 affects changed files, but Task 1 is in-progress
        tracker
            .get_task_mut(&task1_id)
            .unwrap()
            .set_affected_files(vec![std::path::PathBuf::from("src/a.rs")]);
        tracker
            .get_task_mut(&task2_id)
            .unwrap()
            .set_affected_files(vec![std::path::PathBuf::from("src/changed.rs")]);

        let scope = ChangeScope::from_files(vec![std::path::PathBuf::from("src/changed.rs")]);

        // In-progress task should still be prioritized (current task continuation)
        let selected = tracker.select_next_task_scoped(&scope);
        assert!(selected.is_some());

        let selected_id = selected.unwrap();
        assert!(
            selected_id.title().contains("First"),
            "In-progress task should be prioritized over scoped matches"
        );
    }

    #[test]
    fn test_select_next_task_scoped_with_empty_scope() {
        use ralph::changes::ChangeScope;

        let plan = r#"
## Current Sprint
### 1. Task One
- [ ] Item
"#;

        let mut tracker = TaskTracker::default();
        tracker.parse_plan(plan).unwrap();

        let scope = ChangeScope::new(); // Empty scope

        // Should fall back to normal selection
        let selected = tracker.select_next_task_scoped(&scope);
        assert!(selected.is_some(), "Should find a task with empty scope");
    }

    #[test]
    fn test_select_next_task_scoped_handles_tasks_without_affected_files() {
        use ralph::changes::ChangeScope;

        let plan = r#"
## Current Sprint
### 1. Task without affected files
- [ ] Item

### 2. Task with affected files
- [ ] Item
"#;

        let mut tracker = TaskTracker::default();
        tracker.parse_plan(plan).unwrap();

        // Only set affected_files for task 2
        let task2_id = TaskId::parse("### 2. Task with affected files").unwrap();
        tracker
            .get_task_mut(&task2_id)
            .unwrap()
            .set_affected_files(vec![std::path::PathBuf::from("src/specific.rs")]);

        // Scope matches task 2's affected files
        let scope = ChangeScope::from_files(vec![std::path::PathBuf::from("src/specific.rs")]);

        // Task 2 should be prioritized because it explicitly affects the changed file
        let selected = tracker.select_next_task_scoped(&scope);
        assert!(selected.is_some());

        let selected_id = selected.unwrap();
        assert!(
            selected_id.title().contains("with affected"),
            "Should prioritize task with explicit affected_files match"
        );
    }
}
