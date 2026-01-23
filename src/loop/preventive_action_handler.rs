//! Preventive action handler for the automation loop.
//!
//! This module converts `PreventiveAction` recommendations from the
//! `StagnationPredictor` into concrete loop behavior changes.
//!
//! # Architecture
//!
//! ```text
//! ┌────────────────────┐     ┌───────────────────────┐     ┌─────────────┐
//! │StagnationPredictor │────>│PreventiveActionHandler│────>│ LoopManager │
//! │                    │     │                       │     │             │
//! │ risk_score()       │     │ handle()              │     │ state       │
//! │ preventive_action()│     │                       │     │ prompt_asm  │
//! └────────────────────┘     └───────────────────────┘     │ task_tracker│
//!                                                          └─────────────┘
//! ```
//!
//! # Example
//!
//! ```rust,ignore
//! use ralph::r#loop::preventive_action_handler::{PreventiveActionHandler, HandlerResult};
//! use ralph::supervisor::predictor::PreventiveAction;
//!
//! let handler = PreventiveActionHandler::new();
//! let action = PreventiveAction::InjectGuidance {
//!     guidance: "Consider committing your work".into(),
//! };
//!
//! let result = handler.handle(action, &mut context)?;
//! assert_eq!(result, HandlerResult::Continue);
//! ```

use crate::r#loop::state::LoopMode;
use crate::supervisor::predictor::PreventiveAction;
use anyhow::Result;
use tracing::{debug, info, warn};

/// Result of handling a preventive action.
///
/// Indicates whether the loop should continue or pause for user review.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HandlerResult {
    /// Continue loop execution normally.
    Continue,
    /// Pause and return control to the user for review.
    PauseForReview,
}

impl HandlerResult {
    /// Returns true if the loop should pause.
    #[must_use]
    pub fn should_pause(&self) -> bool {
        matches!(self, Self::PauseForReview)
    }
}

/// Context required for handling preventive actions.
///
/// This trait abstracts the parts of `LoopManager` needed by the handler,
/// enabling easier testing with mocks.
pub trait HandlerContext {
    /// Add guidance to the prompt assembler.
    fn add_guidance(&mut self, guidance: String);

    /// Set focus on a specific task by description.
    ///
    /// Returns an error if the task cannot be found.
    fn focus_task(&mut self, task_description: &str) -> Result<()>;

    /// Run quality gate tests only (not clippy/lint).
    ///
    /// Returns true if tests passed, false otherwise.
    fn run_tests_only(&self) -> Result<bool>;

    /// Suggest a commit if quality gates pass.
    ///
    /// Returns true if a commit suggestion was made, false if gates failed.
    fn suggest_commit(&self) -> Result<bool>;

    /// Switch the loop to a different mode.
    fn switch_mode(&mut self, target: LoopMode);

    /// Get the current loop mode.
    fn current_mode(&self) -> LoopMode;
}

/// Handler for converting predictor actions into loop behavior.
///
/// The handler receives `PreventiveAction` values from the `StagnationPredictor`
/// and applies them to the loop through the `HandlerContext` trait.
#[derive(Debug, Clone, Default)]
pub struct PreventiveActionHandler {
    /// Count of actions handled in this session.
    actions_handled: u32,
    /// Count of guidance injections.
    guidance_count: u32,
    /// Count of mode switches triggered.
    mode_switches: u32,
    /// Count of review requests.
    review_requests: u32,
}

impl PreventiveActionHandler {
    /// Create a new handler with default configuration.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Handle a preventive action from the stagnation predictor.
    ///
    /// # Arguments
    ///
    /// * `action` - The preventive action to handle.
    /// * `context` - The loop context to apply changes to.
    ///
    /// # Returns
    ///
    /// * `Ok(HandlerResult::Continue)` - Action handled, continue loop.
    /// * `Ok(HandlerResult::PauseForReview)` - Action requires user review.
    /// * `Err(...)` - Action handling failed.
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// let handler = PreventiveActionHandler::new();
    /// let result = handler.handle(
    ///     PreventiveAction::SwitchMode { target: "debug".into() },
    ///     &mut context,
    /// )?;
    /// ```
    pub fn handle(
        &mut self,
        action: PreventiveAction,
        context: &mut dyn HandlerContext,
    ) -> Result<HandlerResult> {
        self.actions_handled += 1;

        match action {
            PreventiveAction::None => {
                debug!("PreventiveAction::None - no action needed");
                Ok(HandlerResult::Continue)
            }

            PreventiveAction::InjectGuidance { guidance } => {
                info!(
                    "Injecting guidance: {}",
                    &guidance[..guidance.len().min(80)]
                );
                context.add_guidance(guidance);
                self.guidance_count += 1;
                Ok(HandlerResult::Continue)
            }

            PreventiveAction::FocusTask { task } => {
                info!("Focusing on task: {}", task);
                context.focus_task(&task)?;
                Ok(HandlerResult::Continue)
            }

            PreventiveAction::RunTests => {
                info!("Running tests as preventive action");
                match context.run_tests_only() {
                    Ok(passed) => {
                        if passed {
                            debug!("Tests passed");
                        } else {
                            debug!("Tests failed - will retry");
                        }
                        Ok(HandlerResult::Continue)
                    }
                    Err(e) => {
                        warn!("Test execution failed: {}", e);
                        // Don't fail the loop, just continue
                        Ok(HandlerResult::Continue)
                    }
                }
            }

            PreventiveAction::SuggestCommit => {
                info!("Suggesting commit");
                match context.suggest_commit() {
                    Ok(committed) => {
                        if committed {
                            debug!("Commit suggestion accepted - gates passed");
                        } else {
                            debug!("Commit suggestion deferred - gates failed");
                        }
                        Ok(HandlerResult::Continue)
                    }
                    Err(e) => {
                        warn!("Commit suggestion failed: {}", e);
                        Ok(HandlerResult::Continue)
                    }
                }
            }

            PreventiveAction::SwitchMode { target } => {
                let new_mode = parse_loop_mode(&target);
                let current = context.current_mode();

                if current != new_mode {
                    info!("Switching mode: {} -> {}", current, new_mode);
                    context.switch_mode(new_mode);
                    self.mode_switches += 1;
                } else {
                    debug!("Already in {} mode, no switch needed", new_mode);
                }
                Ok(HandlerResult::Continue)
            }

            PreventiveAction::RequestReview { reason } => {
                warn!("Review requested: {}", reason);
                self.review_requests += 1;
                Ok(HandlerResult::PauseForReview)
            }
        }
    }

    /// Returns the total number of actions handled.
    #[must_use]
    pub fn actions_handled(&self) -> u32 {
        self.actions_handled
    }

    /// Returns the number of guidance injections.
    #[must_use]
    pub fn guidance_count(&self) -> u32 {
        self.guidance_count
    }

    /// Returns the number of mode switches triggered.
    #[must_use]
    pub fn mode_switches(&self) -> u32 {
        self.mode_switches
    }

    /// Returns the number of review requests.
    #[must_use]
    pub fn review_requests(&self) -> u32 {
        self.review_requests
    }

    /// Returns a summary of handler activity.
    #[must_use]
    pub fn summary(&self) -> String {
        format!(
            "Actions: {} total, {} guidance, {} mode switches, {} review requests",
            self.actions_handled, self.guidance_count, self.mode_switches, self.review_requests
        )
    }

    /// Reset handler statistics.
    pub fn reset(&mut self) {
        self.actions_handled = 0;
        self.guidance_count = 0;
        self.mode_switches = 0;
        self.review_requests = 0;
    }
}

/// Parse a string into a `LoopMode`.
///
/// Defaults to `Debug` if the string is unrecognized.
fn parse_loop_mode(s: &str) -> LoopMode {
    match s.to_lowercase().as_str() {
        "build" => LoopMode::Build,
        "plan" => LoopMode::Plan,
        "debug" => LoopMode::Debug,
        _ => LoopMode::Debug,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::RefCell;

    /// Mock context for testing the handler.
    struct MockContext {
        guidance: RefCell<Vec<String>>,
        focused_task: RefCell<Option<String>>,
        tests_result: bool,
        commit_result: bool,
        mode: RefCell<LoopMode>,
        test_run_count: RefCell<u32>,
        commit_run_count: RefCell<u32>,
    }

    impl MockContext {
        fn new() -> Self {
            Self {
                guidance: RefCell::new(Vec::new()),
                focused_task: RefCell::new(None),
                tests_result: true,
                commit_result: true,
                mode: RefCell::new(LoopMode::Build),
                test_run_count: RefCell::new(0),
                commit_run_count: RefCell::new(0),
            }
        }

        fn with_tests_result(mut self, passed: bool) -> Self {
            self.tests_result = passed;
            self
        }

        fn with_commit_result(mut self, success: bool) -> Self {
            self.commit_result = success;
            self
        }

        fn with_mode(self, mode: LoopMode) -> Self {
            *self.mode.borrow_mut() = mode;
            self
        }
    }

    impl HandlerContext for MockContext {
        fn add_guidance(&mut self, guidance: String) {
            self.guidance.borrow_mut().push(guidance);
        }

        fn focus_task(&mut self, task_description: &str) -> Result<()> {
            *self.focused_task.borrow_mut() = Some(task_description.to_string());
            Ok(())
        }

        fn run_tests_only(&self) -> Result<bool> {
            *self.test_run_count.borrow_mut() += 1;
            Ok(self.tests_result)
        }

        fn suggest_commit(&self) -> Result<bool> {
            *self.commit_run_count.borrow_mut() += 1;
            Ok(self.commit_result)
        }

        fn switch_mode(&mut self, target: LoopMode) {
            *self.mode.borrow_mut() = target;
        }

        fn current_mode(&self) -> LoopMode {
            *self.mode.borrow()
        }
    }

    // =========================================================================
    // Test: InjectGuidance adds text to prompt extras
    // =========================================================================

    #[test]
    fn test_action_inject_guidance_adds_to_prompt() {
        let mut handler = PreventiveActionHandler::new();
        let mut context = MockContext::new();

        let action = PreventiveAction::InjectGuidance {
            guidance: "Consider committing your work".to_string(),
        };

        let result = handler.handle(action, &mut context).unwrap();

        assert_eq!(result, HandlerResult::Continue);
        assert_eq!(context.guidance.borrow().len(), 1);
        assert_eq!(
            context.guidance.borrow()[0],
            "Consider committing your work"
        );
        assert_eq!(handler.guidance_count(), 1);
    }

    #[test]
    fn test_action_inject_guidance_multiple() {
        let mut handler = PreventiveActionHandler::new();
        let mut context = MockContext::new();

        handler
            .handle(
                PreventiveAction::InjectGuidance {
                    guidance: "First guidance".to_string(),
                },
                &mut context,
            )
            .unwrap();
        handler
            .handle(
                PreventiveAction::InjectGuidance {
                    guidance: "Second guidance".to_string(),
                },
                &mut context,
            )
            .unwrap();

        assert_eq!(context.guidance.borrow().len(), 2);
        assert_eq!(handler.guidance_count(), 2);
        assert_eq!(handler.actions_handled(), 2);
    }

    // =========================================================================
    // Test: FocusTask sets task tracker to specific task
    // =========================================================================

    #[test]
    fn test_action_focus_task_sets_task() {
        let mut handler = PreventiveActionHandler::new();
        let mut context = MockContext::new();

        let action = PreventiveAction::FocusTask {
            task: "Fix the authentication bug".to_string(),
        };

        let result = handler.handle(action, &mut context).unwrap();

        assert_eq!(result, HandlerResult::Continue);
        assert_eq!(
            *context.focused_task.borrow(),
            Some("Fix the authentication bug".to_string())
        );
    }

    // =========================================================================
    // Test: RunTests triggers quality gate test-only run
    // =========================================================================

    #[test]
    fn test_action_run_tests_triggers_test_run() {
        let mut handler = PreventiveActionHandler::new();
        let mut context = MockContext::new();

        let result = handler
            .handle(PreventiveAction::RunTests, &mut context)
            .unwrap();

        assert_eq!(result, HandlerResult::Continue);
        assert_eq!(*context.test_run_count.borrow(), 1);
    }

    #[test]
    fn test_action_run_tests_continues_on_failure() {
        let mut handler = PreventiveActionHandler::new();
        let mut context = MockContext::new().with_tests_result(false);

        let result = handler
            .handle(PreventiveAction::RunTests, &mut context)
            .unwrap();

        // Should continue even if tests fail
        assert_eq!(result, HandlerResult::Continue);
        assert_eq!(*context.test_run_count.borrow(), 1);
    }

    // =========================================================================
    // Test: SuggestCommit triggers commit when gates pass
    // =========================================================================

    #[test]
    fn test_action_suggest_commit_when_gates_pass() {
        let mut handler = PreventiveActionHandler::new();
        let mut context = MockContext::new().with_commit_result(true);

        let result = handler
            .handle(PreventiveAction::SuggestCommit, &mut context)
            .unwrap();

        assert_eq!(result, HandlerResult::Continue);
        assert_eq!(*context.commit_run_count.borrow(), 1);
    }

    #[test]
    fn test_action_suggest_commit_deferred_when_gates_fail() {
        let mut handler = PreventiveActionHandler::new();
        let mut context = MockContext::new().with_commit_result(false);

        let result = handler
            .handle(PreventiveAction::SuggestCommit, &mut context)
            .unwrap();

        // Should continue even if commit is deferred
        assert_eq!(result, HandlerResult::Continue);
        assert_eq!(*context.commit_run_count.borrow(), 1);
    }

    // =========================================================================
    // Test: SwitchMode changes loop mode
    // =========================================================================

    #[test]
    fn test_action_switch_mode_changes_mode() {
        let mut handler = PreventiveActionHandler::new();
        let mut context = MockContext::new().with_mode(LoopMode::Build);

        let action = PreventiveAction::SwitchMode {
            target: "debug".to_string(),
        };

        let result = handler.handle(action, &mut context).unwrap();

        assert_eq!(result, HandlerResult::Continue);
        assert_eq!(context.current_mode(), LoopMode::Debug);
        assert_eq!(handler.mode_switches(), 1);
    }

    #[test]
    fn test_action_switch_mode_no_change_if_same() {
        let mut handler = PreventiveActionHandler::new();
        let mut context = MockContext::new().with_mode(LoopMode::Debug);

        let action = PreventiveAction::SwitchMode {
            target: "debug".to_string(),
        };

        let result = handler.handle(action, &mut context).unwrap();

        assert_eq!(result, HandlerResult::Continue);
        assert_eq!(context.current_mode(), LoopMode::Debug);
        // Mode switch count should be 0 because mode didn't change
        assert_eq!(handler.mode_switches(), 0);
    }

    #[test]
    fn test_action_switch_mode_to_build() {
        let mut handler = PreventiveActionHandler::new();
        let mut context = MockContext::new().with_mode(LoopMode::Debug);

        let action = PreventiveAction::SwitchMode {
            target: "build".to_string(),
        };

        handler.handle(action, &mut context).unwrap();

        assert_eq!(context.current_mode(), LoopMode::Build);
    }

    #[test]
    fn test_action_switch_mode_to_plan() {
        let mut handler = PreventiveActionHandler::new();
        let mut context = MockContext::new().with_mode(LoopMode::Build);

        let action = PreventiveAction::SwitchMode {
            target: "plan".to_string(),
        };

        handler.handle(action, &mut context).unwrap();

        assert_eq!(context.current_mode(), LoopMode::Plan);
    }

    // =========================================================================
    // Test: RequestReview pauses loop and returns control to user
    // =========================================================================

    #[test]
    fn test_action_request_review_pauses_loop() {
        let mut handler = PreventiveActionHandler::new();
        let mut context = MockContext::new();

        let action = PreventiveAction::RequestReview {
            reason: "Critical stagnation detected".to_string(),
        };

        let result = handler.handle(action, &mut context).unwrap();

        assert_eq!(result, HandlerResult::PauseForReview);
        assert!(result.should_pause());
        assert_eq!(handler.review_requests(), 1);
    }

    // =========================================================================
    // Test: None action has no effect
    // =========================================================================

    #[test]
    fn test_action_none_has_no_effect() {
        let mut handler = PreventiveActionHandler::new();
        let mut context = MockContext::new();

        let result = handler
            .handle(PreventiveAction::None, &mut context)
            .unwrap();

        assert_eq!(result, HandlerResult::Continue);
        assert!(context.guidance.borrow().is_empty());
        assert!(context.focused_task.borrow().is_none());
        assert_eq!(*context.test_run_count.borrow(), 0);
        assert_eq!(*context.commit_run_count.borrow(), 0);
        // Actions handled should still increment
        assert_eq!(handler.actions_handled(), 1);
    }

    // =========================================================================
    // Handler statistics and utility tests
    // =========================================================================

    #[test]
    fn test_handler_statistics() {
        let mut handler = PreventiveActionHandler::new();
        let mut context = MockContext::new();

        // Perform various actions
        handler
            .handle(PreventiveAction::None, &mut context)
            .unwrap();
        handler
            .handle(
                PreventiveAction::InjectGuidance {
                    guidance: "test".into(),
                },
                &mut context,
            )
            .unwrap();
        handler
            .handle(
                PreventiveAction::SwitchMode {
                    target: "debug".into(),
                },
                &mut context,
            )
            .unwrap();

        assert_eq!(handler.actions_handled(), 3);
        assert_eq!(handler.guidance_count(), 1);
        assert_eq!(handler.mode_switches(), 1);
        assert_eq!(handler.review_requests(), 0);
    }

    #[test]
    fn test_handler_summary() {
        let mut handler = PreventiveActionHandler::new();
        let mut context = MockContext::new();

        handler
            .handle(
                PreventiveAction::InjectGuidance {
                    guidance: "test".into(),
                },
                &mut context,
            )
            .unwrap();

        let summary = handler.summary();
        assert!(summary.contains("1 total"));
        assert!(summary.contains("1 guidance"));
    }

    #[test]
    fn test_handler_reset() {
        let mut handler = PreventiveActionHandler::new();
        let mut context = MockContext::new();

        handler
            .handle(
                PreventiveAction::InjectGuidance {
                    guidance: "test".into(),
                },
                &mut context,
            )
            .unwrap();
        handler
            .handle(
                PreventiveAction::RequestReview {
                    reason: "test".into(),
                },
                &mut context,
            )
            .unwrap();

        assert_eq!(handler.actions_handled(), 2);

        handler.reset();

        assert_eq!(handler.actions_handled(), 0);
        assert_eq!(handler.guidance_count(), 0);
        assert_eq!(handler.review_requests(), 0);
    }

    #[test]
    fn test_handler_result_should_pause() {
        assert!(!HandlerResult::Continue.should_pause());
        assert!(HandlerResult::PauseForReview.should_pause());
    }

    #[test]
    fn test_parse_loop_mode() {
        assert_eq!(parse_loop_mode("build"), LoopMode::Build);
        assert_eq!(parse_loop_mode("BUILD"), LoopMode::Build);
        assert_eq!(parse_loop_mode("plan"), LoopMode::Plan);
        assert_eq!(parse_loop_mode("PLAN"), LoopMode::Plan);
        assert_eq!(parse_loop_mode("debug"), LoopMode::Debug);
        assert_eq!(parse_loop_mode("DEBUG"), LoopMode::Debug);
        // Unknown defaults to Debug
        assert_eq!(parse_loop_mode("unknown"), LoopMode::Debug);
    }
}
