//! Iteration execution and progress tracking methods for `LoopManager`.
//!
//! This module contains methods related to running Claude Code iterations,
//! tracking progress, and managing the intelligent retry system.

use super::{mode_to_prompt_name, truncate_prompt, LoopManager};
use crate::r#loop::progress::{ProgressEvaluation, ProgressSignals};
use crate::r#loop::retry::{
    FailureClass, FailureClassifier, FailureContext, FailureLocation, RecoveryStrategy,
    RetryAttempt, SubTask, TaskDecomposer,
};
use anyhow::Result;
use std::process::Command;
use tokio::io::AsyncWriteExt;
use tokio::process::Command as AsyncCommand;
use tracing::debug;

impl LoopManager {
    /// Run a single Claude Code iteration.
    pub(crate) async fn run_claude_iteration(&self) -> Result<i32> {
        const MAX_PROMPT_LENGTH: usize = 12000;

        let mode_name = mode_to_prompt_name(&self.state.mode);
        let prompt = self.build_iteration_prompt(mode_name)?;

        // Truncate if needed
        let original_len = prompt.len();
        let prompt = truncate_prompt(prompt, MAX_PROMPT_LENGTH);
        if prompt.len() < original_len {
            tracing::warn!(
                "Prompt truncated from {} to {} chars",
                original_len,
                prompt.len()
            );
        }

        debug!(
            "Running Claude Code with prompt for mode {} ({} chars)",
            mode_name,
            prompt.len()
        );

        // Use injected Claude process if available
        if let Some(deps) = &self.deps {
            return deps.claude.run_iteration(&prompt).await;
        }

        // Fallback to direct command execution
        self.run_claude_command(&prompt).await
    }

    /// Run the claude command with the given prompt.
    pub(crate) async fn run_claude_command(&self, prompt: &str) -> Result<i32> {
        let args = vec!["-p", "--dangerously-skip-permissions", "--model", "opus"];

        let claude_md = ralph::config::ProjectConfig::claude_md_path(&self.project_dir);
        if claude_md.exists() {
            debug!("Using CLAUDE.md from {}", claude_md.display());
        }

        let mut child = AsyncCommand::new("claude")
            .args(&args)
            .current_dir(&self.project_dir)
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::inherit())
            .stderr(std::process::Stdio::inherit())
            .spawn()?;

        if let Some(mut stdin) = child.stdin.take() {
            stdin.write_all(prompt.as_bytes()).await?;
            stdin.flush().await?;
            drop(stdin);
        }

        let status = child.wait().await?;
        Ok(status.code().unwrap_or(1))
    }

    /// Clean up stale LSP processes (rust-analyzer, etc.).
    ///
    /// This helps prevent LSP crashes from accumulating and causing failures
    /// in long-running automation sessions.
    pub(crate) fn cleanup_lsp() {
        // Kill any stale rust-analyzer processes
        let _ = Command::new("pkill").args(["-f", "rust-analyzer"]).output();

        // Give processes time to terminate
        std::thread::sleep(std::time::Duration::from_millis(500));
    }

    // =========================================================================
    // Progress Tracking Methods
    // =========================================================================

    /// Check if there's been any progress (commits or plan changes).
    pub(crate) fn has_made_progress(&self) -> bool {
        // Use semantic progress evaluation
        match self.evaluate_progress() {
            Ok(evaluation) => {
                debug!(
                    "Progress evaluation: {:?} (score: {:.2}) - {}",
                    evaluation.verdict, evaluation.score, evaluation.explanation
                );

                // Log health status for monitoring
                if evaluation.verdict.is_healthy() {
                    debug!("Verdict indicates healthy activity");
                } else {
                    debug!("Verdict indicates unhealthy state - may need intervention");
                }

                evaluation.verdict.should_reset_stagnation()
            }
            Err(e) => {
                // Fall back to simple commit check on error
                debug!(
                    "Progress evaluation failed ({}), falling back to commit check",
                    e
                );
                let commit_count = self.count_commits_since(&self.state.last_commit_hash);
                if commit_count > 0 {
                    debug!("Progress detected: {} new commit(s)", commit_count);
                    return true;
                }

                // Check for plan file changes
                if let Ok(current_hash) = self.get_plan_hash() {
                    if current_hash != self.state.last_plan_hash {
                        debug!("Progress detected: IMPLEMENTATION_PLAN.md changed");
                        return true;
                    }
                }

                false
            }
        }
    }

    /// Evaluate progress using semantic signals.
    ///
    /// Returns a detailed evaluation including score, verdict, and explanation.
    pub(crate) fn evaluate_progress(&self) -> Result<ProgressEvaluation> {
        self.progress_tracker.evaluate(&self.state.last_commit_hash)
    }

    /// Collect detailed progress signals for logging and analysis.
    ///
    /// This method gathers multi-dimensional signals from git, quality checks,
    /// and behavioral patterns to provide comprehensive progress insights.
    pub fn collect_progress_signals(&self) -> Result<ProgressSignals> {
        self.progress_tracker
            .collect_signals(&self.state.last_commit_hash)
    }

    /// Log detailed progress status for the current iteration.
    ///
    /// Uses the progress tracker to collect and evaluate signals,
    /// providing verbose output about the state of progress.
    pub fn log_progress_status(&self) {
        // Collect raw signals
        let signals = match self.collect_progress_signals() {
            Ok(s) => s,
            Err(e) => {
                debug!("Could not collect progress signals: {}", e);
                // Return empty signals as fallback
                ProgressSignals::new()
            }
        };

        // Quick check for any positive activity
        if signals.has_any_positive_signal() {
            debug!("Progress signals detected: {}", signals.summary());
        } else {
            debug!("No positive progress signals detected");
        }

        // Collect and log quality signals separately
        if let Ok(quality) = self.progress_tracker.collect_quality_signals() {
            debug!("Quality state: {}", quality.summary());
        }

        // Evaluate the signals
        let evaluation = self.progress_tracker().evaluate_signals(&signals);
        debug!(
            "Signal evaluation: {:?} (score: {:.2})",
            evaluation.verdict, evaluation.score
        );

        // Log contribution breakdown if verbose
        for contrib in &evaluation.contributions {
            debug!(
                "  - {}: raw={:.1}, weight={:.2}, contribution={:.3}",
                contrib.signal_name, contrib.raw_value, contrib.weight, contrib.contribution
            );
        }
    }

    /// Update progress tracking baselines after each iteration.
    ///
    /// This should be called after quality checks to ensure accurate delta
    /// calculations for the next iteration.
    pub fn update_progress_baselines(&mut self) {
        // Collect current quality state and update baselines
        if let Ok(quality) = self.progress_tracker.collect_quality_signals() {
            self.progress_tracker
                .update_baselines(quality.test_count, quality.warning_count);
            debug!(
                "Updated progress baselines: {} tests, {} warnings",
                quality.test_count, quality.warning_count
            );
        }
    }

    // =========================================================================
    // Intelligent Retry Methods
    // =========================================================================

    /// Classify a failure and determine recovery strategy.
    ///
    /// Returns `Some((strategy, prompt))` if retry should be attempted,
    /// `None` if retries are exhausted.
    pub fn classify_failure(
        &mut self,
        error_output: &str,
        task_id: Option<&str>,
    ) -> Option<(RecoveryStrategy, String)> {
        self.intelligent_retry
            .process_failure(error_output, task_id)
    }

    /// Record a retry attempt in history.
    pub fn record_retry_attempt(
        &mut self,
        failure: FailureContext,
        strategy: RecoveryStrategy,
        succeeded: bool,
    ) {
        // Capture failure class before moving
        let failure_class = failure.class;
        let mut attempt = RetryAttempt::new(failure, strategy);
        if succeeded {
            attempt.mark_success();
        }
        self.retry_history.record(attempt);
        debug!(
            "Recorded retry attempt: {:?} with strategy {:?}, success={}",
            failure_class, strategy, succeeded
        );
    }

    /// Get a summary of retry history for logging.
    #[must_use]
    pub fn retry_summary(&self) -> String {
        let history_summary = self.retry_history.summary();
        let intelligent_summary = self.intelligent_retry.summary();

        // Include success rates for common strategies
        let isolated_fix_rate = self
            .retry_history
            .success_rate(RecoveryStrategy::IsolatedFix);
        let test_first_rate = self.retry_history.success_rate(RecoveryStrategy::TestFirst);

        // Include success rates for common failure classes
        let compile_error_rate = self
            .retry_history
            .class_success_rate(FailureClass::CompileError);
        let test_failure_rate = self
            .retry_history
            .class_success_rate(FailureClass::TestFailure);

        format!(
            "{}; {}; Strategy success: IsolatedFix={:.0}%, TestFirst={:.0}%; Class success: CompileError={:.0}%, TestFailure={:.0}%",
            history_summary,
            intelligent_summary,
            isolated_fix_rate * 100.0,
            test_first_rate * 100.0,
            compile_error_rate * 100.0,
            test_failure_rate * 100.0
        )
    }

    /// Check if retries are exhausted for a task.
    #[must_use]
    pub fn retries_exhausted(&self, task_id: &str) -> bool {
        self.intelligent_retry.retries_exhausted(task_id)
    }

    /// Reset retry state for a task (e.g., after it completes).
    pub fn reset_task_retries(&mut self, task_id: &str) {
        self.intelligent_retry.reset_task(task_id);
        debug!("Reset retry state for task: {}", task_id);
    }

    /// Decompose a task based on failure context.
    ///
    /// Returns a list of sub-tasks to tackle the failure incrementally.
    #[must_use]
    pub fn decompose_task_for_failure(
        &self,
        task_description: &str,
        failure: &FailureContext,
    ) -> Vec<SubTask> {
        let decomposer = TaskDecomposer::new();
        decomposer.decompose(task_description, failure)
    }

    /// Check if a failure is simple enough to not need decomposition.
    #[must_use]
    pub fn is_atomic_failure(&self, failure: &FailureContext) -> bool {
        let decomposer = TaskDecomposer::new();
        decomposer.is_atomic(failure)
    }

    /// Get failure classifier for manual classification if needed.
    #[must_use]
    pub fn failure_classifier(&self) -> &FailureClassifier {
        self.intelligent_retry.classifier()
    }

    /// Create a failure location from file and line info.
    #[must_use]
    pub fn create_failure_location(file: &str, line: Option<u32>) -> FailureLocation {
        let mut loc = FailureLocation::new(file);
        if let Some(l) = line {
            loc = loc.with_line(l);
        }
        loc
    }

    /// Clear retry history for a resolved failure.
    ///
    /// Call this when a previously failing operation succeeds, to reset
    /// the strategist's memory of tried strategies for that failure.
    pub fn clear_resolved_failure(&mut self, error_output: &str) {
        let failure = self.failure_classifier().classify(error_output);
        self.intelligent_retry.clear_failure_history(&failure);
        debug!(
            "Cleared retry history for resolved failure: {}",
            failure.summary()
        );
    }

    /// Log retry status for diagnostics.
    pub fn log_retry_status(&self, task_id: &str) {
        let retry_count = self.intelligent_retry.retry_count(task_id);
        let exhausted = self.intelligent_retry.retries_exhausted(task_id);
        debug!(
            "Retry status for {}: {} retries, exhausted={}",
            task_id, retry_count, exhausted
        );
    }
}
