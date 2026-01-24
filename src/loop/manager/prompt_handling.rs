//! Prompt building and plan handling methods for `LoopManager`.
//!
//! This module contains methods related to:
//! - Building iteration prompts (dynamic and static)
//! - Reading and hashing the implementation plan
//! - Checking for orphaned tasks when plan changes

use super::LoopManager;
use crate::r#loop::task_tracker::ValidationResult;
use anyhow::{bail, Context, Result};
use std::path::PathBuf;
use tracing::{debug, warn};

impl LoopManager {
    /// Build the prompt for a Claude Code iteration.
    ///
    /// Tries dynamic prompt generation first, falling back to static file.
    pub(crate) fn build_iteration_prompt(&self, mode_name: &str) -> Result<String> {
        // Try to build dynamic prompt first, fall back to static file if needed
        match self.prompt_assembler.build_prompt(mode_name) {
            Ok(dynamic_prompt) => {
                debug!(
                    "Using dynamic prompt for mode: {} ({} chars)",
                    mode_name,
                    dynamic_prompt.len()
                );
                Ok(dynamic_prompt)
            }
            Err(e) => {
                // Fall back to static prompt file
                debug!("Dynamic prompt failed ({}), falling back to static file", e);
                self.build_static_prompt()
            }
        }
    }

    /// Build a static prompt from the PROMPT_*.md file.
    fn build_static_prompt(&self) -> Result<String> {
        let prompt_path = self.get_prompt_path();

        let base_prompt = if let Some(deps) = &self.deps {
            let fs = deps
                .fs
                .try_read()
                .map_err(|_| anyhow::anyhow!("Could not acquire filesystem lock"))?;
            let prompt_filename = self.state.mode.prompt_filename();
            if !fs.exists(&prompt_filename) {
                bail!("Prompt file not found: {}", prompt_path.display());
            }
            fs.read_file(&prompt_filename)?
        } else {
            if !prompt_path.exists() {
                bail!("Prompt file not found: {}", prompt_path.display());
            }
            std::fs::read_to_string(&prompt_path)?
        };

        // Add task context to static prompt (static prompts don't have {{TASK_CONTEXT}})
        let task_context = self.task_tracker.get_context_summary();
        if task_context.is_empty() {
            Ok(base_prompt)
        } else {
            Ok(format!(
                "## Current Task Context\n\n{}\n\n---\n\n{}",
                task_context, base_prompt
            ))
        }
    }

    /// Get the path to the current mode's prompt file.
    fn get_prompt_path(&self) -> PathBuf {
        self.project_dir.join(self.state.mode.prompt_filename())
    }

    /// Get an MD5 hash of the implementation plan.
    pub(crate) fn get_plan_hash(&self) -> Result<String> {
        let content = if let Some(deps) = &self.deps {
            let fs = deps
                .fs
                .try_read()
                .map_err(|_| anyhow::anyhow!("Could not acquire filesystem lock"))?;
            fs.read_file("IMPLEMENTATION_PLAN.md")
                .context("Failed to read IMPLEMENTATION_PLAN.md")?
        } else {
            let plan_path = self.project_dir.join("IMPLEMENTATION_PLAN.md");
            std::fs::read_to_string(&plan_path).context("Failed to read IMPLEMENTATION_PLAN.md")?
        };
        Ok(format!("{:x}", md5::compute(content.as_bytes())))
    }

    /// Read the plan content from IMPLEMENTATION_PLAN.md.
    fn read_plan_content(&self) -> Result<String> {
        if let Some(deps) = &self.deps {
            let fs = deps
                .fs
                .try_read()
                .map_err(|_| anyhow::anyhow!("Could not acquire filesystem lock"))?;
            fs.read_file("IMPLEMENTATION_PLAN.md")
                .context("Failed to read IMPLEMENTATION_PLAN.md")
        } else {
            let plan_path = self.project_dir.join("IMPLEMENTATION_PLAN.md");
            std::fs::read_to_string(&plan_path).context("Failed to read IMPLEMENTATION_PLAN.md")
        }
    }

    /// Validate task tracker state against the current plan on startup.
    ///
    /// This should be called at the beginning of a session to ensure the
    /// persisted task tracker state is consistent with the current plan.
    /// It marks orphaned tasks and clears the current_task if it's stale.
    ///
    /// This prevents Ralph from getting stuck on stale/removed tasks when
    /// the plan changes between sessions.
    pub(crate) fn validate_task_tracker_on_startup(&mut self) {
        if let Ok(plan_content) = self.read_plan_content() {
            self.task_tracker.validate_on_startup(&plan_content);
            debug!("Task tracker startup validation complete");
        }
    }

    /// Check for orphaned tasks when the plan structure changes.
    ///
    /// Validates the current task tracker against the plan and marks
    /// any tasks that are no longer in the plan as orphaned.
    pub(crate) fn check_for_orphaned_tasks(&mut self) {
        if let Ok(plan_content) = self.read_plan_content() {
            match self.task_tracker.validate_against_plan(&plan_content) {
                ValidationResult::Valid => {
                    debug!("Plan structure unchanged, no orphaned tasks");
                }
                ValidationResult::PlanChanged { orphaned_tasks } => {
                    if !orphaned_tasks.is_empty() {
                        warn!(
                            "Plan structure changed, {} task(s) orphaned: {:?}",
                            orphaned_tasks.len(),
                            orphaned_tasks
                        );
                        self.task_tracker.mark_orphaned_tasks(&plan_content);
                    }
                }
            }
        }
    }
}
