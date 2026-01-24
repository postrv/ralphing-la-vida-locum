//! Plan parsing utilities for task tracking.
//!
//! This module contains functions for parsing implementation plans
//! and extracting task information from markdown content.

use anyhow::{Context, Result};
use regex::Regex;

use super::{Task, TaskId, TaskTracker};

// ============================================================================
// Plan Structure Hashing
// ============================================================================

/// Compute a hash of the plan structure for change detection.
///
/// Only considers structural elements (task headers and sprint sections).
#[must_use]
pub(crate) fn compute_plan_hash(content: &str) -> String {
    // Extract only structural elements (task headers and sprint sections)
    let structural: String = content
        .lines()
        .filter(|line| {
            let trimmed = line.trim();
            trimmed.starts_with("###") || trimmed.starts_with("## Sprint")
        })
        .collect::<Vec<_>>()
        .join("\n");
    format!("{:x}", md5::compute(structural.as_bytes()))
}

// ============================================================================
// Sprint Parsing
// ============================================================================

/// Parse the current sprint from the "Current Focus" section.
///
/// Looks for patterns like "Current Focus: Sprint 7" in the content.
#[must_use]
pub(crate) fn parse_current_sprint(content: &str) -> Option<u32> {
    for line in content.lines() {
        if line.contains("Current Focus:") && line.contains("Sprint") {
            // Extract sprint number from patterns like "Sprint 7" or "Sprint 7 ("
            if let Some(sprint_idx) = line.find("Sprint ") {
                let after_sprint = &line[sprint_idx + 7..];
                let num_str: String = after_sprint
                    .chars()
                    .take_while(|c| c.is_ascii_digit())
                    .collect();
                if let Ok(num) = num_str.parse() {
                    return Some(num);
                }
            }
        }
    }
    None
}

/// Parse the sprint number from a section header.
///
/// Matches patterns like "## Sprint 7:" or "## Sprint 10:".
#[must_use]
pub(crate) fn parse_sprint_from_section(line: &str) -> Option<u32> {
    // Match "## Sprint N:" patterns
    if let Some(after_sprint) = line.strip_prefix("## Sprint ") {
        let num_str: String = after_sprint
            .chars()
            .take_while(|c| c.is_ascii_digit())
            .collect();
        return num_str.parse().ok();
    }
    None
}

// ============================================================================
// Checkbox Parsing
// ============================================================================

/// Parse checkbox mark and text into a tuple of (text, is_checked).
///
/// # Arguments
/// * `mark` - The checkbox mark character: "x", "X", or " "
/// * `text` - The checkbox text content
///
/// # Returns
/// Tuple of (text as String, is_checked as bool)
#[must_use]
pub fn parse_checkbox_match(mark: &str, text: &str) -> (String, bool) {
    let checked = mark == "x" || mark == "X";
    (text.to_string(), checked)
}

// ============================================================================
// TaskTracker Parsing Implementation
// ============================================================================

impl TaskTracker {
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
        let checkbox_re =
            Regex::new(r"^-\s+\[([ xX])\]\s+(.+)$").context("Failed to compile checkbox regex")?;

        self.focused_sprint = parse_current_sprint(content);
        self.plan_structure_hash = compute_plan_hash(content);

        let mut current_task: Option<TaskId> = None;
        let mut current_checkboxes: Vec<(String, bool)> = Vec::new();
        let mut current_sprint: Option<u32> = None;

        for line in content.lines() {
            let trimmed = line.trim();

            // Update sprint context if we hit a sprint section header
            if let Some(sprint) = parse_sprint_from_section(trimmed) {
                current_sprint = Some(sprint);
            }

            // Handle task header - save previous task's checkboxes and start new task
            if let Ok(task_id) = TaskId::parse(trimmed) {
                if let Some(ref prev_task_id) = current_task {
                    self.update_task_checkboxes(prev_task_id, &current_checkboxes);
                }
                current_checkboxes.clear();
                current_task = Some(task_id.clone());
                self.insert_or_update_task(task_id, current_sprint);
                continue;
            }

            // Handle checkbox line - only if we're inside a task
            if let (true, Some(caps)) = (current_task.is_some(), checkbox_re.captures(trimmed)) {
                let checkbox = parse_checkbox_match(&caps[1], &caps[2]);
                current_checkboxes.push(checkbox);
            }
        }

        // Save checkboxes for the last task
        if let Some(ref task_id) = current_task {
            self.update_task_checkboxes(task_id, &current_checkboxes);
        }

        self.modified_at = chrono::Utc::now();
        Ok(())
    }

    /// Update checkboxes for a task without losing other state.
    pub(crate) fn update_task_checkboxes(
        &mut self,
        task_id: &TaskId,
        checkboxes: &[(String, bool)],
    ) {
        if let Some(task) = self.tasks.get_mut(task_id) {
            task.checkboxes = checkboxes.to_vec();
        }
    }

    /// Insert a new task or update an existing task's sprint affiliation.
    ///
    /// If the task doesn't exist, creates it with the given sprint.
    /// If the task exists but has no sprint set, updates it with the given sprint.
    /// If the task exists and already has a sprint, preserves the existing sprint.
    pub fn insert_or_update_task(&mut self, task_id: TaskId, current_sprint: Option<u32>) {
        if let Some(task) = self.tasks.get_mut(&task_id) {
            // Update sprint if not already set
            if task.sprint.is_none() && current_sprint.is_some() {
                task.sprint = current_sprint;
            }
        } else {
            // Create new task with sprint
            let task = match current_sprint {
                Some(sprint) => Task::new_with_sprint(task_id.clone(), sprint),
                None => Task::new(task_id.clone()),
            };
            self.tasks.insert(task_id, task);
        }
    }

    /// Validate the tracker against a plan to detect structural changes.
    #[must_use]
    pub fn validate_against_plan(&self, plan: &str) -> super::ValidationResult {
        let current_hash = compute_plan_hash(plan);
        if current_hash == self.plan_structure_hash {
            super::ValidationResult::Valid
        } else {
            let orphaned = self.find_orphaned_tasks(plan);
            super::ValidationResult::PlanChanged {
                orphaned_tasks: orphaned,
            }
        }
    }

    /// Find tasks in the tracker that are not in the current plan.
    #[must_use]
    pub fn find_orphaned_tasks(&self, plan: &str) -> Vec<TaskId> {
        let header_re = Regex::new(r"^###\s+(\d+[a-z]?)\.\s+(.+)$").unwrap();

        // Collect all task titles from the plan
        let plan_tasks: std::collections::HashSet<String> = plan
            .lines()
            .filter_map(|line| {
                let trimmed = line.trim();
                if header_re.is_match(trimmed) {
                    TaskId::parse(trimmed).ok().map(|id| id.title().to_string())
                } else {
                    None
                }
            })
            .collect();

        // Find tasks in tracker not in plan
        self.tasks
            .keys()
            .filter(|id| !plan_tasks.contains(id.title()))
            .cloned()
            .collect()
    }

    /// Mark tasks as orphaned if they are not in the given plan.
    pub fn mark_orphaned_tasks(&mut self, plan: &str) {
        let orphaned_ids = self.find_orphaned_tasks(plan);
        for id in orphaned_ids {
            if let Some(task) = self.tasks.get_mut(&id) {
                task.mark_orphaned();
            }
        }
        self.modified_at = chrono::Utc::now();
    }

    /// Validate task tracker state against the current plan on startup.
    ///
    /// This method should be called at the beginning of a new session to ensure
    /// the persisted task tracker state is consistent with the current plan.
    ///
    /// It performs the following operations:
    /// 1. Marks any tasks not in the plan as orphaned
    /// 2. Clears `current_task` if it points to an orphaned task
    ///
    /// This prevents Ralph from getting stuck on stale/removed tasks after
    /// the plan changes between sessions.
    ///
    /// # Example
    ///
    /// ```
    /// use ralph::r#loop::task_tracker::{TaskTracker, TaskTrackerConfig};
    ///
    /// let mut tracker = TaskTracker::new(TaskTrackerConfig::default());
    /// let plan = "### 1. Task One\n- [ ] Item";
    /// tracker.validate_on_startup(plan);
    /// ```
    pub fn validate_on_startup(&mut self, plan: &str) {
        // First, mark any orphaned tasks
        self.mark_orphaned_tasks(plan);

        // Then, clear current_task if it's orphaned
        let _ = self.clear_current_task_if_orphaned();
    }

    /// Clear the current task if it is marked as orphaned.
    ///
    /// Returns `true` if the current task was cleared, `false` otherwise.
    ///
    /// This is useful when the plan changes and the previously-selected task
    /// no longer exists.
    ///
    /// # Example
    ///
    /// ```
    /// use ralph::r#loop::task_tracker::{TaskTracker, TaskTrackerConfig};
    ///
    /// let mut tracker = TaskTracker::new(TaskTrackerConfig::default());
    /// // ... parse plan, set current, mark orphaned ...
    /// let was_cleared = tracker.clear_current_task_if_orphaned();
    /// ```
    #[must_use]
    pub fn clear_current_task_if_orphaned(&mut self) -> bool {
        let should_clear = self
            .current_task
            .as_ref()
            .and_then(|id| self.tasks.get(id))
            .is_some_and(|task| task.is_orphaned());

        if should_clear {
            self.current_task = None;
            self.modified_at = chrono::Utc::now();
            true
        } else {
            false
        }
    }

    /// Check if a task exists in the given plan content.
    ///
    /// This method checks if the task's title appears as a valid task header
    /// in the plan content. It's useful for defensive validation to ensure
    /// a task is still present before working on it.
    ///
    /// Note: This method is primarily for testing defensive task selection
    /// behavior. In production, the orphan flag system handles this via
    /// `validate_on_startup()` and `mark_orphaned_tasks()`.
    #[cfg(test)]
    #[must_use]
    pub fn task_exists_in_plan(&self, task_id: &TaskId, plan: &str) -> bool {
        let header_re = Regex::new(r"^###\s+(\d+[a-z]?)\.\s+(.+)$").unwrap();

        plan.lines().any(|line| {
            let trimmed = line.trim();
            if header_re.is_match(trimmed) {
                if let Ok(parsed_id) = TaskId::parse(trimmed) {
                    parsed_id.title() == task_id.title()
                } else {
                    false
                }
            } else {
                false
            }
        })
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::r#loop::task_tracker::{TaskState, TaskTrackerConfig, ValidationResult};

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

    // ========================================================================
    // Checkbox Parsing Tests
    // ========================================================================

    #[test]
    fn test_parse_checkbox_match_lowercase_x() {
        let (text, checked) = parse_checkbox_match("x", "Some task");
        assert!(checked);
        assert_eq!(text, "Some task");
    }

    #[test]
    fn test_parse_checkbox_match_uppercase_x() {
        let (text, checked) = parse_checkbox_match("X", "Another task");
        assert!(checked);
        assert_eq!(text, "Another task");
    }

    #[test]
    fn test_parse_checkbox_match_unchecked() {
        let (text, checked) = parse_checkbox_match(" ", "Pending task");
        assert!(!checked);
        assert_eq!(text, "Pending task");
    }

    // ========================================================================
    // Sprint Parsing Tests
    // ========================================================================

    #[test]
    fn test_task_tracker_parses_current_focus_sprint() {
        let plan = r#"
## Current Focus: Sprint 7 (Language-Specific Quality Gates)

### 7a. Task A
"#;
        let mut tracker = TaskTracker::default();
        tracker.parse_plan(plan).unwrap();

        assert_eq!(tracker.current_sprint(), Some(7));
    }

    #[test]
    fn test_task_tracker_stores_sprint_affiliation() {
        let plan = r#"
## Current Focus: Sprint 7 (Language-Specific Quality Gates)

## Sprint 7: Language-Specific Quality Gates

### 7a. QualityGate Trait Refactor
- [ ] Create QualityGate trait

### 7b. Python Quality Gates
- [ ] Implement RuffGate
"#;
        let mut tracker = TaskTracker::default();
        tracker.parse_plan(plan).unwrap();

        let task_a = TaskId::parse("### 7a. QualityGate Trait Refactor").unwrap();
        let task = tracker.get_task(&task_a).unwrap();
        assert_eq!(task.sprint(), Some(7));
    }

    // ========================================================================
    // Hash and Validation Tests
    // ========================================================================

    #[test]
    fn test_task_tracker_computes_plan_hash() {
        let plan = "### 1. Task A\n### 2. Task B";
        let mut tracker = TaskTracker::default();
        tracker.parse_plan(plan).unwrap();

        let hash = tracker.plan_hash();
        assert!(!hash.is_empty());
    }

    #[test]
    fn test_task_tracker_hash_changes_with_structure() {
        let plan1 = "### 1. Task A\n### 2. Task B";
        let plan2 = "### 1. Task A\n### 2. Task B\n### 3. Task C";

        let mut tracker1 = TaskTracker::default();
        tracker1.parse_plan(plan1).unwrap();

        let mut tracker2 = TaskTracker::default();
        tracker2.parse_plan(plan2).unwrap();

        assert_ne!(tracker1.plan_hash(), tracker2.plan_hash());
    }

    #[test]
    fn test_task_tracker_hash_stable_for_same_structure() {
        let plan = "### 1. Task A\n### 2. Task B";

        let mut tracker1 = TaskTracker::default();
        tracker1.parse_plan(plan).unwrap();

        let mut tracker2 = TaskTracker::default();
        tracker2.parse_plan(plan).unwrap();

        assert_eq!(tracker1.plan_hash(), tracker2.plan_hash());
    }

    #[test]
    fn test_validate_against_plan_detects_change() {
        let plan_v1 = "### 1. Task A\n### 2. Task B";
        let plan_v2 = "### 1. Task A\n### 3. New Task";

        let mut tracker = TaskTracker::default();
        tracker.parse_plan(plan_v1).unwrap();

        let result = tracker.validate_against_plan(plan_v2);
        assert!(matches!(result, ValidationResult::PlanChanged { .. }));
    }

    #[test]
    fn test_validate_against_plan_valid_when_unchanged() {
        let plan = "### 1. Task A\n### 2. Task B";

        let mut tracker = TaskTracker::default();
        tracker.parse_plan(plan).unwrap();

        let result = tracker.validate_against_plan(plan);
        assert!(matches!(result, ValidationResult::Valid));
    }

    // ========================================================================
    // Orphaned Task Tests
    // ========================================================================

    #[test]
    fn test_find_orphaned_tasks_detects_orphans() {
        let plan_v1 = "### 1. Task A\n### 2. Task B\n### 3. Enterprise Feature";
        let plan_v2 = "### 1. Task A\n### 2. Task B"; // Task 3 removed

        let mut tracker = TaskTracker::default();
        tracker.parse_plan(plan_v1).unwrap();

        let orphaned = tracker.find_orphaned_tasks(plan_v2);
        assert_eq!(orphaned.len(), 1);
        assert_eq!(orphaned[0].title(), "Enterprise Feature");
    }

    #[test]
    fn test_find_orphaned_tasks_empty_when_all_present() {
        let plan = "### 1. Task A\n### 2. Task B";

        let mut tracker = TaskTracker::default();
        tracker.parse_plan(plan).unwrap();

        let orphaned = tracker.find_orphaned_tasks(plan);
        assert!(orphaned.is_empty());
    }

    #[test]
    fn test_task_marked_as_orphaned() {
        let plan_v1 = "### 1. Old Task\n### 2. Current Task";
        let plan_v2 = "### 2. Current Task"; // Task 1 removed

        let mut tracker = TaskTracker::default();
        tracker.parse_plan(plan_v1).unwrap();

        tracker.mark_orphaned_tasks(plan_v2);

        let old_task = TaskId::parse("### 1. Old Task").unwrap();
        let task = tracker.get_task(&old_task).unwrap();
        assert!(task.is_orphaned());
    }

    // ========================================================================
    // Insert/Update Task Tests
    // ========================================================================

    #[test]
    fn test_insert_or_update_task_new_task_with_sprint() {
        let mut tracker = TaskTracker::new(TaskTrackerConfig::default());
        let task_id = TaskId::new_for_test(1, "Test task");

        tracker.insert_or_update_task(task_id.clone(), Some(7));

        let task = tracker.get_task(&task_id).unwrap();
        assert_eq!(task.sprint(), Some(7));
    }

    #[test]
    fn test_insert_or_update_task_new_task_without_sprint() {
        let mut tracker = TaskTracker::new(TaskTrackerConfig::default());
        let task_id = TaskId::new_for_test(2, "No sprint task");

        tracker.insert_or_update_task(task_id.clone(), None);

        let task = tracker.get_task(&task_id).unwrap();
        assert_eq!(task.sprint(), None);
    }

    #[test]
    fn test_insert_or_update_task_existing_task_preserves_sprint() {
        let mut tracker = TaskTracker::new(TaskTrackerConfig::default());
        let task_id = TaskId::new_for_test(3, "Existing task");

        // First insert with sprint 5
        tracker.insert_or_update_task(task_id.clone(), Some(5));

        // Second call should NOT override existing sprint
        tracker.insert_or_update_task(task_id.clone(), Some(9));

        let task = tracker.get_task(&task_id).unwrap();
        assert_eq!(task.sprint(), Some(5)); // Original sprint preserved
    }

    #[test]
    fn test_insert_or_update_task_existing_task_sets_sprint_if_none() {
        let mut tracker = TaskTracker::new(TaskTrackerConfig::default());
        let task_id = TaskId::new_for_test(4, "Task without sprint");

        // First insert without sprint
        tracker.insert_or_update_task(task_id.clone(), None);
        assert_eq!(tracker.get_task(&task_id).unwrap().sprint(), None);

        // Second call should set sprint since it was None
        tracker.insert_or_update_task(task_id.clone(), Some(8));

        let task = tracker.get_task(&task_id).unwrap();
        assert_eq!(task.sprint(), Some(8));
    }
}
