//! Task section builder.
//!
//! Generates markdown sections for task context.

use crate::prompt::context::CurrentTaskContext;

/// Build the task context section.
///
/// # Example
///
/// ```
/// use ralph::prompt::builder::SectionBuilder;
/// use ralph::prompt::context::{CurrentTaskContext, TaskPhase};
///
/// let task = CurrentTaskContext::new("1.1", "Setup testing", TaskPhase::Implementation)
///     .with_completion(50)
///     .with_attempts(2);
///
/// let section = SectionBuilder::build_task_section(&task);
/// assert!(section.contains("## Current Task"));
/// assert!(section.contains("1.1"));
/// assert!(section.contains("50%"));
/// ```
#[must_use]
pub fn build_task_section(task: &CurrentTaskContext) -> String {
    let mut lines = vec![
        "## Current Task".to_string(),
        String::new(),
        format!("**Task:** {} - {}", task.task_id, task.title),
        format!("**Phase:** {}", task.phase),
        format!("**Progress:** {}%", task.completion_percentage),
    ];

    if task.attempt_count > 0 {
        lines.push(format!("**Attempts:** {}", task.attempt_count));
    }

    if !task.modified_files.is_empty() {
        lines.push(String::new());
        lines.push("**Modified Files:**".to_string());
        for file in &task.modified_files {
            lines.push(format!("- `{}`", file));
        }
    }

    if !task.blockers.is_empty() {
        lines.push(String::new());
        lines.push("**\u{26a0}\u{fe0f} Blockers:**".to_string());
        for blocker in &task.blockers {
            lines.push(format!("- {}", blocker));
        }
    }

    if !task.dependencies.is_empty() {
        lines.push(String::new());
        lines.push("**Dependencies (must complete first):**".to_string());
        for dep in &task.dependencies {
            lines.push(format!("- {}", dep));
        }
    }

    lines.push(String::new());
    lines.join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::prompt::context::TaskPhase;

    #[test]
    fn test_build_task_section_basic() {
        let task =
            CurrentTaskContext::new("2.1", "Create context types", TaskPhase::Implementation);
        let section = build_task_section(&task);

        assert!(section.contains("## Current Task"));
        assert!(section.contains("2.1"));
        assert!(section.contains("Create context types"));
        assert!(section.contains("Implementation"));
        assert!(section.contains("0%")); // Default completion
    }

    #[test]
    fn test_build_task_section_with_progress() {
        let task = CurrentTaskContext::new("1.1", "Task", TaskPhase::Testing)
            .with_completion(75)
            .with_attempts(3);

        let section = build_task_section(&task);

        assert!(section.contains("75%"));
        assert!(section.contains("**Attempts:** 3"));
    }

    #[test]
    fn test_build_task_section_with_files() {
        let task = CurrentTaskContext::new("1.1", "Task", TaskPhase::Implementation)
            .with_modified_files(vec!["src/lib.rs".to_string(), "src/main.rs".to_string()]);

        let section = build_task_section(&task);

        assert!(section.contains("**Modified Files:**"));
        assert!(section.contains("`src/lib.rs`"));
        assert!(section.contains("`src/main.rs`"));
    }

    #[test]
    fn test_build_task_section_with_blockers() {
        let task = CurrentTaskContext::new("1.1", "Task", TaskPhase::Implementation)
            .with_blockers(vec!["Dependency not available".to_string()]);

        let section = build_task_section(&task);

        assert!(section.contains("Blockers:"));
        assert!(section.contains("Dependency not available"));
    }

    #[test]
    fn test_build_task_section_with_dependencies() {
        let task = CurrentTaskContext::new("1.2", "Task", TaskPhase::Implementation)
            .with_dependencies(vec!["1.1".to_string()]);

        let section = build_task_section(&task);

        assert!(section.contains("Dependencies (must complete first):"));
        assert!(section.contains("1.1"));
    }
}
