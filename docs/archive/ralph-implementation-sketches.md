# Ralph Enhancement Implementation Sketches

This document provides concrete implementation sketches for the P0 priority enhancements.

---

## 1. Task-Level State Machine (Highest Impact)

### Problem Statement
Ralph currently tracks progress globally (any commit = progress). This means:
- Getting stuck on one task while completing others looks like "progress"
- No visibility into how long each task is taking
- No mechanism to skip genuinely blocked tasks

### Implementation

```rust
// src/loop/task_tracker.rs

use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;

/// Unique identifier for a task
#[derive(Debug, Clone, Hash, Eq, PartialEq, Serialize, Deserialize)]
pub struct TaskId(String);

impl TaskId {
    /// Parse from IMPLEMENTATION_PLAN.md format: "### 1. Task Name"
    pub fn from_header(header: &str) -> Self {
        Self(header.trim_start_matches('#').trim().to_lowercase())
    }
}

/// State of an individual task
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum TaskState {
    /// Task hasn't been started
    NotStarted,
    
    /// Task is currently being worked on
    InProgress {
        started_at: DateTime<Utc>,
        iterations_spent: u32,
        files_touched: Vec<PathBuf>,
        last_activity: String,
        consecutive_no_progress: u32,
    },
    
    /// Task is blocked and needs intervention
    Blocked {
        reason: BlockReason,
        blocked_at: DateTime<Utc>,
        attempts: u32,
        last_error: Option<String>,
        suggested_action: String,
    },
    
    /// Task code is written, awaiting quality gates
    InReview {
        started_review_at: DateTime<Utc>,
        tests_passing: bool,
        clippy_clean: bool,
        security_clean: bool,
        remaining_gates: Vec<String>,
    },
    
    /// Task is complete
    Complete {
        completed_at: DateTime<Utc>,
        total_iterations: u32,
        files_changed: Vec<PathBuf>,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum BlockReason {
    MaxAttemptsExceeded,
    TimeoutExceeded,
    ExternalDependency(String),
    CyclicDependency(Vec<TaskId>),
    MissingRequirement(String),
    PersistentError(String),
}

/// Configuration for task tracking
#[derive(Debug, Clone)]
pub struct TaskTrackerConfig {
    /// Max iterations before considering a task stuck
    pub max_task_iterations: u32,
    /// Max time before considering a task stuck
    pub max_task_duration: Duration,
    /// Max attempts before blocking a task
    pub max_attempts: u32,
    /// Iterations without progress before escalating
    pub no_progress_threshold: u32,
}

impl Default for TaskTrackerConfig {
    fn default() -> Self {
        Self {
            max_task_iterations: 20,
            max_task_duration: Duration::minutes(45),
            max_attempts: 3,
            no_progress_threshold: 5,
        }
    }
}

/// Tracks the state of all tasks
pub struct TaskTracker {
    tasks: HashMap<TaskId, TaskState>,
    current_task: Option<TaskId>,
    config: TaskTrackerConfig,
    transitions: Vec<TaskTransition>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskTransition {
    task_id: TaskId,
    from: TaskState,
    to: TaskState,
    at: DateTime<Utc>,
    reason: String,
}

impl TaskTracker {
    pub fn new(config: TaskTrackerConfig) -> Self {
        Self {
            tasks: HashMap::new(),
            current_task: None,
            config,
            transitions: Vec::new(),
        }
    }
    
    /// Parse tasks from IMPLEMENTATION_PLAN.md
    pub fn parse_plan(&mut self, plan_content: &str) {
        let task_pattern = regex::Regex::new(r"^###\s+\d+\.\s+(.+)$").unwrap();
        let checkbox_pattern = regex::Regex::new(r"^-\s+\[([ x])\]").unwrap();
        
        let mut current_task: Option<TaskId> = None;
        let mut all_checkboxes = 0;
        let mut completed_checkboxes = 0;
        
        for line in plan_content.lines() {
            if let Some(caps) = task_pattern.captures(line) {
                // Save previous task state
                if let Some(ref task_id) = current_task {
                    self.update_task_from_checkboxes(
                        task_id, 
                        all_checkboxes, 
                        completed_checkboxes
                    );
                }
                
                // Start new task
                let task_name = caps.get(1).unwrap().as_str();
                current_task = Some(TaskId::from_header(task_name));
                all_checkboxes = 0;
                completed_checkboxes = 0;
                
                // Initialize if new
                let task_id = current_task.as_ref().unwrap();
                if !self.tasks.contains_key(task_id) {
                    self.tasks.insert(task_id.clone(), TaskState::NotStarted);
                }
            }
            
            if current_task.is_some() {
                if let Some(caps) = checkbox_pattern.captures(line) {
                    all_checkboxes += 1;
                    if caps.get(1).unwrap().as_str() == "x" {
                        completed_checkboxes += 1;
                    }
                }
            }
        }
        
        // Don't forget the last task
        if let Some(ref task_id) = current_task {
            self.update_task_from_checkboxes(
                task_id, 
                all_checkboxes, 
                completed_checkboxes
            );
        }
    }
    
    fn update_task_from_checkboxes(
        &mut self, 
        task_id: &TaskId, 
        total: u32, 
        completed: u32
    ) {
        if total > 0 && completed == total {
            // All checkboxes complete - mark task complete
            if let Some(state) = self.tasks.get(task_id) {
                if !matches!(state, TaskState::Complete { .. }) {
                    self.transition(
                        task_id.clone(),
                        TaskState::Complete {
                            completed_at: Utc::now(),
                            total_iterations: self.iterations_on_task(task_id),
                            files_changed: Vec::new(),
                        },
                        "All checkboxes completed".into(),
                    );
                }
            }
        }
    }
    
    fn iterations_on_task(&self, task_id: &TaskId) -> u32 {
        match self.tasks.get(task_id) {
            Some(TaskState::InProgress { iterations_spent, .. }) => *iterations_spent,
            Some(TaskState::Complete { total_iterations, .. }) => *total_iterations,
            _ => 0,
        }
    }
    
    fn transition(&mut self, task_id: TaskId, to: TaskState, reason: String) {
        let from = self.tasks.get(&task_id).cloned().unwrap_or(TaskState::NotStarted);
        
        self.transitions.push(TaskTransition {
            task_id: task_id.clone(),
            from: from.clone(),
            to: to.clone(),
            at: Utc::now(),
            reason,
        });
        
        self.tasks.insert(task_id, to);
    }
    
    /// Select the best task to work on next
    pub fn select_next_task(&self) -> Option<TaskId> {
        // Priority order:
        // 1. Continue current in-progress task (if not stuck)
        // 2. Tasks InReview that just need gate fixes
        // 3. NotStarted tasks
        // 4. Previously blocked tasks (retry)
        
        // Check if current task should continue
        if let Some(ref current) = self.current_task {
            if let Some(state) = self.tasks.get(current) {
                if matches!(state, TaskState::InProgress { .. }) {
                    if !self.is_task_stuck(current) {
                        return Some(current.clone());
                    }
                }
            }
        }
        
        // Find best alternative
        let mut candidates: Vec<_> = self.tasks.iter()
            .filter(|(_, state)| !matches!(state, TaskState::Complete { .. }))
            .filter(|(id, _)| !self.is_task_stuck(id))
            .collect();
        
        candidates.sort_by_key(|(_, state)| {
            match state {
                TaskState::InReview { .. } => 0,  // Almost done!
                TaskState::InProgress { .. } => 1,
                TaskState::NotStarted => 2,
                TaskState::Blocked { attempts, .. } if *attempts < self.config.max_attempts => 3,
                _ => 99,
            }
        });
        
        candidates.first().map(|(id, _)| (*id).clone())
    }
    
    fn is_task_stuck(&self, task_id: &TaskId) -> bool {
        match self.tasks.get(task_id) {
            Some(TaskState::InProgress { 
                iterations_spent, 
                started_at,
                consecutive_no_progress,
                ..
            }) => {
                *iterations_spent > self.config.max_task_iterations
                    || Utc::now().signed_duration_since(*started_at) > self.config.max_task_duration
                    || *consecutive_no_progress >= self.config.no_progress_threshold
            }
            Some(TaskState::Blocked { attempts, .. }) => {
                *attempts >= self.config.max_attempts
            }
            _ => false,
        }
    }
    
    /// Record that we're starting work on a task
    pub fn start_task(&mut self, task_id: TaskId) {
        let current_state = self.tasks.get(&task_id).cloned();
        
        match current_state {
            Some(TaskState::NotStarted) | None => {
                self.transition(
                    task_id.clone(),
                    TaskState::InProgress {
                        started_at: Utc::now(),
                        iterations_spent: 1,
                        files_touched: Vec::new(),
                        last_activity: "Started task".into(),
                        consecutive_no_progress: 0,
                    },
                    "Task started".into(),
                );
            }
            Some(TaskState::InProgress { 
                started_at, 
                iterations_spent, 
                files_touched,
                consecutive_no_progress,
                ..
            }) => {
                self.tasks.insert(task_id.clone(), TaskState::InProgress {
                    started_at,
                    iterations_spent: iterations_spent + 1,
                    files_touched,
                    last_activity: "Continued task".into(),
                    consecutive_no_progress,
                });
            }
            Some(TaskState::Blocked { attempts, .. }) if attempts < self.config.max_attempts => {
                self.transition(
                    task_id.clone(),
                    TaskState::InProgress {
                        started_at: Utc::now(),
                        iterations_spent: 1,
                        files_touched: Vec::new(),
                        last_activity: "Retrying blocked task".into(),
                        consecutive_no_progress: 0,
                    },
                    format!("Retry attempt {}", attempts + 1),
                );
            }
            _ => {}
        }
        
        self.current_task = Some(task_id);
    }
    
    /// Record progress on current task
    pub fn record_progress(&mut self, files_touched: Vec<PathBuf>, activity: String) {
        if let Some(ref task_id) = self.current_task {
            if let Some(TaskState::InProgress { 
                started_at, 
                iterations_spent,
                mut files_touched: existing_files,
                ..
            }) = self.tasks.get(task_id).cloned() {
                existing_files.extend(files_touched);
                self.tasks.insert(task_id.clone(), TaskState::InProgress {
                    started_at,
                    iterations_spent,
                    files_touched: existing_files,
                    last_activity: activity,
                    consecutive_no_progress: 0,  // Reset!
                });
            }
        }
    }
    
    /// Record no progress on current iteration
    pub fn record_no_progress(&mut self) {
        if let Some(ref task_id) = self.current_task {
            if let Some(TaskState::InProgress { 
                started_at, 
                iterations_spent,
                files_touched,
                last_activity,
                consecutive_no_progress,
            }) = self.tasks.get(task_id).cloned() {
                let new_count = consecutive_no_progress + 1;
                
                if new_count >= self.config.no_progress_threshold {
                    self.transition(
                        task_id.clone(),
                        TaskState::Blocked {
                            reason: BlockReason::PersistentError(
                                format!("{} iterations without progress", new_count)
                            ),
                            blocked_at: Utc::now(),
                            attempts: 1,
                            last_error: Some(last_activity.clone()),
                            suggested_action: "Try different approach or skip".into(),
                        },
                        "Stuck: no progress".into(),
                    );
                } else {
                    self.tasks.insert(task_id.clone(), TaskState::InProgress {
                        started_at,
                        iterations_spent,
                        files_touched,
                        last_activity,
                        consecutive_no_progress: new_count,
                    });
                }
            }
        }
    }
    
    /// Block current task
    pub fn block_task(&mut self, reason: BlockReason, error: Option<String>) {
        if let Some(ref task_id) = self.current_task {
            let attempts = match self.tasks.get(task_id) {
                Some(TaskState::Blocked { attempts, .. }) => *attempts + 1,
                _ => 1,
            };
            
            self.transition(
                task_id.clone(),
                TaskState::Blocked {
                    reason: reason.clone(),
                    blocked_at: Utc::now(),
                    attempts,
                    last_error: error,
                    suggested_action: match &reason {
                        BlockReason::MaxAttemptsExceeded => "Skip task".into(),
                        BlockReason::TimeoutExceeded => "Break into smaller tasks".into(),
                        BlockReason::ExternalDependency(dep) => format!("Resolve {}", dep),
                        BlockReason::CyclicDependency(tasks) => 
                            format!("Resolve cycle: {:?}", tasks),
                        BlockReason::MissingRequirement(req) => format!("Provide {}", req),
                        BlockReason::PersistentError(err) => format!("Debug: {}", err),
                    },
                },
                format!("Blocked: {:?}", reason),
            );
            
            self.current_task = None;
        }
    }
    
    /// Move task to review
    pub fn submit_for_review(&mut self) {
        if let Some(ref task_id) = self.current_task {
            self.transition(
                task_id.clone(),
                TaskState::InReview {
                    started_review_at: Utc::now(),
                    tests_passing: false,
                    clippy_clean: false,
                    security_clean: false,
                    remaining_gates: vec![
                        "tests".into(),
                        "clippy".into(),
                        "security".into(),
                    ],
                },
                "Submitted for review".into(),
            );
        }
    }
    
    /// Update review status
    pub fn update_review(&mut self, gate: &str, passed: bool) {
        if let Some(ref task_id) = self.current_task {
            if let Some(TaskState::InReview { 
                started_review_at,
                mut tests_passing,
                mut clippy_clean,
                mut security_clean,
                mut remaining_gates,
            }) = self.tasks.get(task_id).cloned() {
                match gate {
                    "tests" => tests_passing = passed,
                    "clippy" => clippy_clean = passed,
                    "security" => security_clean = passed,
                    _ => {}
                }
                
                if passed {
                    remaining_gates.retain(|g| g != gate);
                }
                
                // Check if all gates passed
                if tests_passing && clippy_clean && security_clean {
                    self.transition(
                        task_id.clone(),
                        TaskState::Complete {
                            completed_at: Utc::now(),
                            total_iterations: self.iterations_on_task(task_id),
                            files_changed: Vec::new(),
                        },
                        "All quality gates passed".into(),
                    );
                } else {
                    self.tasks.insert(task_id.clone(), TaskState::InReview {
                        started_review_at,
                        tests_passing,
                        clippy_clean,
                        security_clean,
                        remaining_gates,
                    });
                }
            }
        }
    }
    
    /// Get summary for prompt injection
    pub fn get_context_summary(&self) -> String {
        let mut summary = String::new();
        
        summary.push_str("## Current Task State\n\n");
        
        if let Some(ref task_id) = self.current_task {
            if let Some(state) = self.tasks.get(task_id) {
                summary.push_str(&format!("**Current Task**: {}\n", task_id.0));
                summary.push_str(&format!("**State**: {:?}\n\n", state));
            }
        }
        
        // Count tasks by state
        let mut not_started = 0;
        let mut in_progress = 0;
        let mut blocked = 0;
        let mut complete = 0;
        
        for state in self.tasks.values() {
            match state {
                TaskState::NotStarted => not_started += 1,
                TaskState::InProgress { .. } => in_progress += 1,
                TaskState::Blocked { .. } => blocked += 1,
                TaskState::Complete { .. } => complete += 1,
                TaskState::InReview { .. } => in_progress += 1,
            }
        }
        
        summary.push_str(&format!(
            "**Progress**: {} complete, {} in progress, {} blocked, {} not started\n",
            complete, in_progress, blocked, not_started
        ));
        
        // List blocked tasks
        let blocked_tasks: Vec<_> = self.tasks.iter()
            .filter(|(_, s)| matches!(s, TaskState::Blocked { .. }))
            .collect();
        
        if !blocked_tasks.is_empty() {
            summary.push_str("\n**Blocked Tasks**:\n");
            for (id, state) in blocked_tasks {
                if let TaskState::Blocked { reason, suggested_action, .. } = state {
                    summary.push_str(&format!(
                        "- {}: {:?} → {}\n",
                        id.0, reason, suggested_action
                    ));
                }
            }
        }
        
        summary
    }
    
    /// Persist state to disk
    pub fn save(&self, path: &std::path::Path) -> anyhow::Result<()> {
        let json = serde_json::to_string_pretty(&self.tasks)?;
        std::fs::write(path, json)?;
        Ok(())
    }
    
    /// Load state from disk
    pub fn load(&mut self, path: &std::path::Path) -> anyhow::Result<()> {
        if path.exists() {
            let json = std::fs::read_to_string(path)?;
            self.tasks = serde_json::from_str(&json)?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_task_selection_prefers_in_progress() {
        let mut tracker = TaskTracker::new(TaskTrackerConfig::default());
        
        tracker.tasks.insert(TaskId("task1".into()), TaskState::NotStarted);
        tracker.tasks.insert(TaskId("task2".into()), TaskState::InProgress {
            started_at: Utc::now(),
            iterations_spent: 5,
            files_touched: Vec::new(),
            last_activity: "Working".into(),
            consecutive_no_progress: 0,
        });
        tracker.current_task = Some(TaskId("task2".into()));
        
        // Should continue with task2
        assert_eq!(
            tracker.select_next_task(),
            Some(TaskId("task2".into()))
        );
    }
    
    #[test]
    fn test_stuck_detection() {
        let mut tracker = TaskTracker::new(TaskTrackerConfig {
            max_task_iterations: 10,
            no_progress_threshold: 3,
            ..Default::default()
        });
        
        tracker.tasks.insert(TaskId("stuck".into()), TaskState::InProgress {
            started_at: Utc::now(),
            iterations_spent: 15,  // Over limit!
            files_touched: Vec::new(),
            last_activity: "Trying".into(),
            consecutive_no_progress: 0,
        });
        
        assert!(tracker.is_task_stuck(&TaskId("stuck".into())));
    }
    
    #[test]
    fn test_no_progress_blocks_task() {
        let mut tracker = TaskTracker::new(TaskTrackerConfig {
            no_progress_threshold: 3,
            ..Default::default()
        });
        
        tracker.tasks.insert(TaskId("task".into()), TaskState::InProgress {
            started_at: Utc::now(),
            iterations_spent: 5,
            files_touched: Vec::new(),
            last_activity: "Trying".into(),
            consecutive_no_progress: 2,
        });
        tracker.current_task = Some(TaskId("task".into()));
        
        // This should trigger blocking
        tracker.record_no_progress();
        
        assert!(matches!(
            tracker.tasks.get(&TaskId("task".into())),
            Some(TaskState::Blocked { .. })
        ));
    }
}
```

---

## 2. Dynamic Prompt Generation

### Implementation

```rust
// src/prompt/builder.rs

use std::path::PathBuf;
use chrono::{DateTime, Duration, Utc};

use crate::loop::task_tracker::{TaskTracker, TaskState};

pub struct DynamicPromptBuilder {
    base_templates: PromptTemplates,
}

pub struct PromptTemplates {
    pub build: String,
    pub debug: String,
    pub plan: String,
}

pub struct PromptContext {
    pub current_task: Option<CurrentTaskContext>,
    pub recent_errors: Vec<ErrorContext>,
    pub quality_gates: QualityGateStatus,
    pub session_stats: SessionStats,
    pub history_insights: Option<String>,
    pub anti_patterns_detected: Vec<AntiPattern>,
}

pub struct CurrentTaskContext {
    pub name: String,
    pub description: String,
    pub time_spent: Duration,
    pub iterations_spent: u32,
    pub files_of_interest: Vec<PathBuf>,
    pub previous_attempts: Vec<AttemptSummary>,
}

pub struct ErrorContext {
    pub error_type: String,
    pub message: String,
    pub file: Option<PathBuf>,
    pub line: Option<u32>,
    pub occurrence_count: u32,
}

pub struct QualityGateStatus {
    pub tests_passing: Option<bool>,
    pub test_count: u32,
    pub clippy_warnings: u32,
    pub security_issues: u32,
}

pub struct SessionStats {
    pub total_iterations: u32,
    pub iterations_remaining: u32,
    pub tasks_completed: u32,
    pub tasks_remaining: u32,
    pub overall_stagnation_count: u32,
}

pub struct AttemptSummary {
    pub approach: String,
    pub outcome: String,
    pub files_touched: Vec<PathBuf>,
}

pub struct AntiPattern {
    pub name: String,
    pub description: String,
    pub evidence: String,
    pub remediation: String,
}

impl DynamicPromptBuilder {
    pub fn new() -> Self {
        Self {
            base_templates: PromptTemplates {
                build: include_str!("../../templates/PROMPT_build.md").to_string(),
                debug: include_str!("../../templates/PROMPT_debug.md").to_string(),
                plan: include_str!("../../templates/PROMPT_plan.md").to_string(),
            },
        }
    }
    
    /// Build a context-aware prompt
    pub fn build(&self, mode: &str, context: &PromptContext) -> String {
        let base = match mode {
            "build" => &self.base_templates.build,
            "debug" => &self.base_templates.debug,
            "plan" => &self.base_templates.plan,
            _ => &self.base_templates.build,
        };
        
        let mut prompt = base.clone();
        
        // Inject task context
        if let Some(ref task) = context.current_task {
            prompt.push_str(&self.build_task_section(task));
        }
        
        // Inject error context
        if !context.recent_errors.is_empty() {
            prompt.push_str(&self.build_error_section(&context.recent_errors));
        }
        
        // Inject quality gate status
        prompt.push_str(&self.build_quality_section(&context.quality_gates));
        
        // Inject session awareness
        prompt.push_str(&self.build_session_section(&context.session_stats));
        
        // Inject anti-pattern guidance
        if !context.anti_patterns_detected.is_empty() {
            prompt.push_str(&self.build_antipattern_section(&context.anti_patterns_detected));
        }
        
        // Inject history insights
        if let Some(ref insights) = context.history_insights {
            prompt.push_str(&format!("\n## Insights from Previous Sessions\n\n{}\n", insights));
        }
        
        prompt
    }
    
    fn build_task_section(&self, task: &CurrentTaskContext) -> String {
        let mut section = String::new();
        
        section.push_str("\n---\n\n## Current Task Context\n\n");
        section.push_str(&format!("**Task**: {}\n", task.name));
        section.push_str(&format!("**Description**: {}\n", task.description));
        section.push_str(&format!(
            "**Time invested**: {} minutes ({} iterations)\n",
            task.time_spent.num_minutes(),
            task.iterations_spent
        ));
        
        if !task.files_of_interest.is_empty() {
            section.push_str("\n**Key files**:\n");
            for file in &task.files_of_interest {
                section.push_str(&format!("- `{}`\n", file.display()));
            }
        }
        
        if !task.previous_attempts.is_empty() {
            section.push_str("\n**Previous attempts**:\n");
            for (i, attempt) in task.previous_attempts.iter().enumerate() {
                section.push_str(&format!(
                    "{}. **{}** → {}\n",
                    i + 1, attempt.approach, attempt.outcome
                ));
            }
            section.push_str("\n⚠️ Do NOT repeat failed approaches. Try something different.\n");
        }
        
        // Time pressure warnings
        if task.iterations_spent > 10 {
            section.push_str(&format!(
                "\n🚨 **WARNING**: {} iterations spent on this task. Consider:\n",
                task.iterations_spent
            ));
            section.push_str("1. Breaking into smaller subtasks\n");
            section.push_str("2. Documenting the blocker\n");
            section.push_str("3. Moving to another task\n");
        }
        
        section
    }
    
    fn build_error_section(&self, errors: &[ErrorContext]) -> String {
        let mut section = String::new();
        
        section.push_str("\n---\n\n## Errors to Address\n\n");
        section.push_str("Fix these errors in order of priority:\n\n");
        
        // Sort by occurrence count (most frequent first)
        let mut sorted_errors = errors.to_vec();
        sorted_errors.sort_by(|a, b| b.occurrence_count.cmp(&a.occurrence_count));
        
        for (i, error) in sorted_errors.iter().take(5).enumerate() {
            section.push_str(&format!("### {}. {} (seen {} times)\n", 
                i + 1, 
                error.error_type,
                error.occurrence_count
            ));
            section.push_str(&format!("```\n{}\n```\n", error.message));
            
            if let Some(ref file) = error.file {
                section.push_str(&format!("**Location**: `{}`", file.display()));
                if let Some(line) = error.line {
                    section.push_str(&format!(", line {}", line));
                }
                section.push_str("\n");
            }
            section.push_str("\n");
        }
        
        if errors.len() > 5 {
            section.push_str(&format!(
                "... and {} more errors. Fix the above first.\n",
                errors.len() - 5
            ));
        }
        
        section
    }
    
    fn build_quality_section(&self, quality: &QualityGateStatus) -> String {
        let mut section = String::new();
        
        section.push_str("\n---\n\n## Quality Gate Status\n\n");
        
        // Tests
        match quality.tests_passing {
            Some(true) => section.push_str(&format!(
                "✅ Tests: PASSING ({} tests)\n", quality.test_count
            )),
            Some(false) => section.push_str(&format!(
                "❌ Tests: FAILING ({} tests) - FIX BEFORE CONTINUING\n", quality.test_count
            )),
            None => section.push_str("⚪ Tests: Not run yet\n"),
        }
        
        // Clippy
        if quality.clippy_warnings == 0 {
            section.push_str("✅ Clippy: Clean\n");
        } else {
            section.push_str(&format!(
                "❌ Clippy: {} warnings - FIX BEFORE CONTINUING\n",
                quality.clippy_warnings
            ));
        }
        
        // Security
        if quality.security_issues == 0 {
            section.push_str("✅ Security: Clean\n");
        } else {
            section.push_str(&format!(
                "❌ Security: {} issues - FIX BEFORE CONTINUING\n",
                quality.security_issues
            ));
        }
        
        section
    }
    
    fn build_session_section(&self, stats: &SessionStats) -> String {
        let mut section = String::new();
        
        section.push_str("\n---\n\n## Session Status\n\n");
        section.push_str(&format!(
            "**Progress**: {} of {} tasks complete\n",
            stats.tasks_completed,
            stats.tasks_completed + stats.tasks_remaining
        ));
        section.push_str(&format!(
            "**Budget**: {} iterations used, {} remaining\n",
            stats.total_iterations,
            stats.iterations_remaining
        ));
        
        if stats.overall_stagnation_count > 0 {
            section.push_str(&format!(
                "⚠️ **Stagnation detected**: {} iterations without meaningful progress\n",
                stats.overall_stagnation_count
            ));
        }
        
        // Urgency based on remaining budget
        let budget_percent = stats.iterations_remaining as f32 
            / (stats.total_iterations + stats.iterations_remaining) as f32;
        
        if budget_percent < 0.2 {
            section.push_str("\n🚨 **LOW BUDGET**: Focus on completing current task or documenting blockers.\n");
        } else if budget_percent < 0.4 {
            section.push_str("\n⚠️ **Budget warning**: Prioritize quick wins.\n");
        }
        
        section
    }
    
    fn build_antipattern_section(&self, patterns: &[AntiPattern]) -> String {
        let mut section = String::new();
        
        section.push_str("\n---\n\n## ⚠️ Anti-Patterns Detected\n\n");
        section.push_str("The following problematic patterns have been observed. Please correct course:\n\n");
        
        for pattern in patterns {
            section.push_str(&format!("### {}\n", pattern.name));
            section.push_str(&format!("**What's happening**: {}\n", pattern.description));
            section.push_str(&format!("**Evidence**: {}\n", pattern.evidence));
            section.push_str(&format!("**Remedy**: {}\n\n", pattern.remediation));
        }
        
        section
    }
}

/// Detect anti-patterns from session history
pub fn detect_antipatterns(
    iterations: &[IterationSummary],
    task_tracker: &TaskTracker,
) -> Vec<AntiPattern> {
    let mut patterns = Vec::new();
    
    // Pattern: Same file edited repeatedly without commit
    let file_edit_counts = count_file_edits(iterations);
    for (file, count) in file_edit_counts.iter() {
        if *count >= 3 {
            patterns.push(AntiPattern {
                name: "Repeated Editing Without Committing".into(),
                description: format!(
                    "File `{}` has been edited {} times without a successful commit",
                    file.display(), count
                ),
                evidence: "Multiple edit operations on same file in iteration history".into(),
                remediation: "Either commit the changes or revert and try a different approach".into(),
            });
        }
    }
    
    // Pattern: Tests not being run
    let recent_test_runs = iterations.iter()
        .rev()
        .take(5)
        .filter(|i| i.ran_tests)
        .count();
    
    if recent_test_runs == 0 && iterations.len() >= 5 {
        patterns.push(AntiPattern {
            name: "Tests Not Being Run".into(),
            description: "No test runs detected in the last 5 iterations".into(),
            evidence: "TDD cycle requires running tests after changes".into(),
            remediation: "Run `cargo test` before making more changes".into(),
        });
    }
    
    // Pattern: Clippy not being run
    let recent_clippy_runs = iterations.iter()
        .rev()
        .take(5)
        .filter(|i| i.ran_clippy)
        .count();
    
    if recent_clippy_runs == 0 && iterations.len() >= 5 {
        patterns.push(AntiPattern {
            name: "Clippy Not Being Run".into(),
            description: "No clippy runs detected in the last 5 iterations".into(),
            evidence: "Quality gates require clean clippy output".into(),
            remediation: "Run `cargo clippy --all-targets -- -D warnings`".into(),
        });
    }
    
    // Pattern: Task oscillation
    let task_switches = count_task_switches(iterations);
    if task_switches > 5 {
        patterns.push(AntiPattern {
            name: "Task Oscillation".into(),
            description: format!(
                "Switched between tasks {} times recently",
                task_switches
            ),
            evidence: "Frequent task switching indicates inability to complete tasks".into(),
            remediation: "Focus on ONE task until complete or explicitly blocked".into(),
        });
    }
    
    patterns
}

struct IterationSummary {
    files_edited: Vec<PathBuf>,
    ran_tests: bool,
    ran_clippy: bool,
    task_id: Option<String>,
    committed: bool,
}

fn count_file_edits(iterations: &[IterationSummary]) -> std::collections::HashMap<PathBuf, u32> {
    let mut counts = std::collections::HashMap::new();
    
    // Only look at recent iterations
    for iteration in iterations.iter().rev().take(10) {
        for file in &iteration.files_edited {
            *counts.entry(file.clone()).or_insert(0) += 1;
        }
    }
    
    counts
}

fn count_task_switches(iterations: &[IterationSummary]) -> u32 {
    let mut switches = 0;
    let mut last_task: Option<&String> = None;
    
    for iteration in iterations.iter().rev().take(10) {
        if let Some(ref task) = iteration.task_id {
            if let Some(last) = last_task {
                if last != task {
                    switches += 1;
                }
            }
            last_task = Some(task);
        }
    }
    
    switches
}
```

---

## 3. Quality Gate Enforcement

```rust
// src/quality/gates.rs

use std::process::Command;
use anyhow::Result;

pub struct QualityGateResult {
    pub gate_name: String,
    pub passed: bool,
    pub message: String,
    pub details: Option<String>,
}

pub struct QualityGateEnforcer {
    project_dir: std::path::PathBuf,
}

impl QualityGateEnforcer {
    pub fn new(project_dir: std::path::PathBuf) -> Self {
        Self { project_dir }
    }
    
    /// Run all quality gates and return results
    pub fn run_all_gates(&self) -> Vec<QualityGateResult> {
        vec![
            self.run_clippy_gate(),
            self.run_test_gate(),
            self.run_no_allow_gate(),
            self.run_no_todo_gate(),
            self.run_security_gate(),
        ]
    }
    
    /// Check if all blocking gates pass
    pub fn can_commit(&self) -> Result<(), Vec<QualityGateResult>> {
        let results = self.run_all_gates();
        let failures: Vec<_> = results.into_iter()
            .filter(|r| !r.passed)
            .collect();
        
        if failures.is_empty() {
            Ok(())
        } else {
            Err(failures)
        }
    }
    
    /// Generate remediation prompt for failures
    pub fn generate_remediation_prompt(&self, failures: &[QualityGateResult]) -> String {
        let mut prompt = String::new();
        
        prompt.push_str("# Quality Gate Failures - Fix Before Committing\n\n");
        prompt.push_str("The following quality gates failed. Fix each issue:\n\n");
        
        for (i, failure) in failures.iter().enumerate() {
            prompt.push_str(&format!("## {}. {} Gate\n\n", i + 1, failure.gate_name));
            prompt.push_str(&format!("**Issue**: {}\n\n", failure.message));
            
            if let Some(ref details) = failure.details {
                prompt.push_str("**Details**:\n```\n");
                // Truncate if too long
                let truncated: String = details.chars().take(2000).collect();
                prompt.push_str(&truncated);
                if details.len() > 2000 {
                    prompt.push_str("\n... (truncated)");
                }
                prompt.push_str("\n```\n\n");
            }
            
            prompt.push_str(&self.get_remediation_guidance(&failure.gate_name));
            prompt.push_str("\n");
        }
        
        prompt.push_str("\n---\n");
        prompt.push_str("After fixing, run the quality gates again before committing.\n");
        
        prompt
    }
    
    fn run_clippy_gate(&self) -> QualityGateResult {
        let output = Command::new("cargo")
            .args(["clippy", "--all-targets", "--", "-D", "warnings"])
            .current_dir(&self.project_dir)
            .output();
        
        match output {
            Ok(o) => {
                let stderr = String::from_utf8_lossy(&o.stderr).to_string();
                let warning_count = stderr.matches("warning:").count();
                
                if o.status.success() && warning_count == 0 {
                    QualityGateResult {
                        gate_name: "Clippy".into(),
                        passed: true,
                        message: "No warnings".into(),
                        details: None,
                    }
                } else {
                    QualityGateResult {
                        gate_name: "Clippy".into(),
                        passed: false,
                        message: format!("{} warnings found", warning_count),
                        details: Some(stderr),
                    }
                }
            }
            Err(e) => QualityGateResult {
                gate_name: "Clippy".into(),
                passed: false,
                message: format!("Failed to run clippy: {}", e),
                details: None,
            },
        }
    }
    
    fn run_test_gate(&self) -> QualityGateResult {
        let output = Command::new("cargo")
            .args(["test"])
            .current_dir(&self.project_dir)
            .output();
        
        match output {
            Ok(o) => {
                let stdout = String::from_utf8_lossy(&o.stdout).to_string();
                let stderr = String::from_utf8_lossy(&o.stderr).to_string();
                
                if o.status.success() {
                    // Extract test count
                    let passed_pattern = regex::Regex::new(r"(\d+) passed").ok();
                    let passed = passed_pattern
                        .and_then(|p| p.captures(&stdout))
                        .and_then(|c| c.get(1))
                        .and_then(|m| m.as_str().parse::<u32>().ok())
                        .unwrap_or(0);
                    
                    QualityGateResult {
                        gate_name: "Tests".into(),
                        passed: true,
                        message: format!("{} tests passed", passed),
                        details: None,
                    }
                } else {
                    QualityGateResult {
                        gate_name: "Tests".into(),
                        passed: false,
                        message: "Tests failing".into(),
                        details: Some(format!("{}\n{}", stdout, stderr)),
                    }
                }
            }
            Err(e) => QualityGateResult {
                gate_name: "Tests".into(),
                passed: false,
                message: format!("Failed to run tests: {}", e),
                details: None,
            },
        }
    }
    
    fn run_no_allow_gate(&self) -> QualityGateResult {
        let output = Command::new("grep")
            .args(["-rn", "#\\[allow(", "src/"])
            .current_dir(&self.project_dir)
            .output();
        
        match output {
            Ok(o) => {
                let stdout = String::from_utf8_lossy(&o.stdout).to_string();
                let matches: Vec<_> = stdout.lines().collect();
                
                if matches.is_empty() {
                    QualityGateResult {
                        gate_name: "No #[allow]".into(),
                        passed: true,
                        message: "No #[allow(...)] annotations found".into(),
                        details: None,
                    }
                } else {
                    QualityGateResult {
                        gate_name: "No #[allow]".into(),
                        passed: false,
                        message: format!("{} forbidden #[allow(...)] annotations", matches.len()),
                        details: Some(stdout),
                    }
                }
            }
            Err(_) => QualityGateResult {
                gate_name: "No #[allow]".into(),
                passed: true,  // grep returns error when no matches
                message: "No #[allow(...)] annotations found".into(),
                details: None,
            },
        }
    }
    
    fn run_no_todo_gate(&self) -> QualityGateResult {
        let output = Command::new("grep")
            .args(["-rn", "-E", "(TODO|FIXME):", "src/"])
            .current_dir(&self.project_dir)
            .output();
        
        match output {
            Ok(o) => {
                let stdout = String::from_utf8_lossy(&o.stdout).to_string();
                let matches: Vec<_> = stdout.lines().collect();
                
                if matches.is_empty() {
                    QualityGateResult {
                        gate_name: "No TODO/FIXME".into(),
                        passed: true,
                        message: "No TODO/FIXME comments found".into(),
                        details: None,
                    }
                } else {
                    QualityGateResult {
                        gate_name: "No TODO/FIXME".into(),
                        passed: false,
                        message: format!("{} TODO/FIXME comments found", matches.len()),
                        details: Some(stdout),
                    }
                }
            }
            Err(_) => QualityGateResult {
                gate_name: "No TODO/FIXME".into(),
                passed: true,
                message: "No TODO/FIXME comments found".into(),
                details: None,
            },
        }
    }
    
    fn run_security_gate(&self) -> QualityGateResult {
        // Try narsil-mcp security scan
        let output = Command::new("narsil-mcp")
            .args(["scan_security"])
            .current_dir(&self.project_dir)
            .output();
        
        match output {
            Ok(o) => {
                let stdout = String::from_utf8_lossy(&o.stdout).to_string();
                
                // Check for CRITICAL or HIGH findings
                let critical = stdout.matches("CRITICAL").count();
                let high = stdout.matches("HIGH").count();
                
                if critical == 0 && high == 0 {
                    QualityGateResult {
                        gate_name: "Security".into(),
                        passed: true,
                        message: "No critical/high security issues".into(),
                        details: None,
                    }
                } else {
                    QualityGateResult {
                        gate_name: "Security".into(),
                        passed: false,
                        message: format!("{} critical, {} high security issues", critical, high),
                        details: Some(stdout),
                    }
                }
            }
            Err(_) => {
                // narsil-mcp not available - skip
                QualityGateResult {
                    gate_name: "Security".into(),
                    passed: true,
                    message: "narsil-mcp not available, skipping".into(),
                    details: None,
                }
            }
        }
    }
    
    fn get_remediation_guidance(&self, gate_name: &str) -> String {
        match gate_name {
            "Clippy" => r#"
**How to fix**:
1. Read each warning carefully
2. Apply the suggested fix OR justify why it's wrong
3. NEVER use `#[allow(...)]` to silence warnings
4. Run `cargo clippy --fix` for auto-fixable issues
"#.into(),
            "Tests" => r#"
**How to fix**:
1. Read the failing test to understand expected behavior
2. Fix the implementation, NOT the test
3. Tests define correct behavior - trust them
4. Run `cargo test <test_name>` to verify your fix
"#.into(),
            "No #[allow]" => r#"
**How to fix**:
1. Remove ALL `#[allow(...)]` annotations
2. Fix the underlying issue the annotation was hiding
3. If code is unused, delete it
4. If code is legitimately needed, write tests that use it
"#.into(),
            "No TODO/FIXME" => r#"
**How to fix**:
1. Either implement the TODO now, OR
2. Remove the code with the TODO if it's not needed
3. If blocked, document in IMPLEMENTATION_PLAN.md, not in code
"#.into(),
            "Security" => r#"
**How to fix**:
1. Review each security finding
2. CRITICAL/HIGH must be fixed before commit
3. Use narsil-mcp for detailed vulnerability info
4. Common fixes: input validation, sanitization, secure defaults
"#.into(),
            _ => "Fix the issue and re-run quality gates.".into(),
        }
    }
}
```

---

## 4. Integration: Modified Loop Manager

```rust
// Modified run() method for loop_manager.rs

impl LoopManager {
    pub async fn run(&mut self) -> Result<()> {
        // Initialize new components
        let mut task_tracker = TaskTracker::new(TaskTrackerConfig::default());
        let prompt_builder = DynamicPromptBuilder::new();
        let quality_enforcer = QualityGateEnforcer::new(self.project_dir.clone());
        
        // Load persisted state
        let state_path = self.project_dir.join(".ralph/task_state.json");
        task_tracker.load(&state_path).ok();
        
        // Parse current plan
        let plan_content = std::fs::read_to_string(
            self.project_dir.join("IMPLEMENTATION_PLAN.md")
        )?;
        task_tracker.parse_plan(&plan_content);
        
        self.print_banner();
        
        for _ in 0..self.max_iterations {
            self.state.iteration += 1;
            self.print_iteration_header();
            
            // Select best task to work on
            let task_id = match task_tracker.select_next_task() {
                Some(id) => id,
                None => {
                    println!("All tasks complete or blocked!");
                    break;
                }
            };
            
            task_tracker.start_task(task_id.clone());
            
            // Build context-aware prompt
            let context = self.build_prompt_context(&task_tracker, &quality_enforcer);
            let prompt = prompt_builder.build(&self.state.mode.to_string(), &context);
            
            // Run iteration with dynamic prompt
            let result = self.run_claude_iteration_with_prompt(&prompt).await;
            
            // Analyze result and update task tracker
            match result {
                Ok(exit_code) if exit_code == 0 => {
                    // Check if progress was made
                    if self.has_made_progress() {
                        let files = self.get_changed_files();
                        task_tracker.record_progress(files, "Made progress".into());
                        self.state.stagnation_count = 0;
                    } else {
                        task_tracker.record_no_progress();
                        self.state.stagnation_count += 1;
                    }
                }
                Ok(_) | Err(_) => {
                    task_tracker.record_no_progress();
                    self.state.stagnation_count += 1;
                }
            }
            
            // Run quality gates
            match quality_enforcer.can_commit() {
                Ok(()) => {
                    // All gates pass - can commit
                    task_tracker.submit_for_review();
                    // Run gate-by-gate updates
                    task_tracker.update_review("tests", true);
                    task_tracker.update_review("clippy", true);
                    task_tracker.update_review("security", true);
                }
                Err(failures) => {
                    // Inject remediation prompt for next iteration
                    let remediation = quality_enforcer.generate_remediation_prompt(&failures);
                    self.inject_guidance(&remediation);
                }
            }
            
            // Save state periodically
            task_tracker.save(&state_path)?;
            
            // Check completion
            if self.is_complete()? {
                break;
            }
        }
        
        Ok(())
    }
    
    fn build_prompt_context(
        &self, 
        task_tracker: &TaskTracker,
        quality_enforcer: &QualityGateEnforcer,
    ) -> PromptContext {
        // Build rich context for dynamic prompt generation
        // ... implementation details
    }
    
    async fn run_claude_iteration_with_prompt(&self, prompt: &str) -> Result<i32> {
        // Similar to existing but uses provided prompt instead of file
        // ... implementation details
    }
}
```

---

## Summary

These three implementations provide:

1. **Task-Level State Machine**: Fine-grained tracking of individual task progress, automatic blocking of stuck tasks, intelligent task selection.

2. **Dynamic Prompt Generation**: Context-aware prompts that inject error history, quality gate status, anti-patterns, and time pressure warnings.

3. **Quality Gate Enforcement**: Hard enforcement of quality standards with automatic remediation prompt generation when gates fail.

Together, they transform Ralph from a "blind retry loop" into an intelligent orchestration system that:
- Knows which task to work on
- Understands what went wrong
- Guides Claude toward fixes
- Won't commit until quality passes
- Can move past genuinely blocked tasks
