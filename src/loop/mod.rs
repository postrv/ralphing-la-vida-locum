//! Loop management module.
//!
//! This module contains the core automation loop components:
//!
//! - [`manager`] - Main loop manager that orchestrates iterations
//! - [`state`] - Loop state tracking and transitions
//! - [`operations`] - Real implementations of testable traits
//! - [`task_tracker`] - Task-level progress tracking and state machine
//! - [`progress`] - Semantic progress detection
//! - [`retry`] - Intelligent retry logic with failure classification
//! - [`preventive_action_handler`] - Convert predictor actions to loop behavior
//!
//! # Architecture
//!
//! The loop module follows a state machine pattern where the `LoopManager`
//! coordinates Claude Code iterations while tracking progress and handling
//! stagnation.
//!
//! ```text
//! ┌─────────────┐     ┌──────────────┐     ┌─────────────┐
//! │LoopManager  │────>│ LoopState    │────>│ TaskTracker │
//! │             │     │              │     │             │
//! └─────────────┘     └──────────────┘     └─────────────┘
//!       │                    │                    │
//!       v                    v                    v
//! ┌─────────────┐     ┌──────────────┐     ┌─────────────┐
//! │ Claude      │     │ Progress     │     │Intelligent  │
//! │ Process     │     │ Detector     │     │   Retry     │
//! └─────────────┘     └──────────────┘     └─────────────┘
//! ```

pub mod manager;
pub mod operations;
pub mod preventive_action_handler;
pub mod progress;
pub mod retry;
pub mod state;
pub mod task_tracker;

// Re-exports for convenience
pub use manager::{LoopManager, LoopManagerConfig};
pub use state::LoopMode;
