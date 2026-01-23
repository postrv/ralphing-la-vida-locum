//! Section builders for dynamic prompt generation.
//!
//! This module contains functions that generate markdown sections from various
//! context types. Each section builder takes a specific context type and returns
//! a formatted markdown string.
//!
//! # Submodules
//!
//! - [`task`] - Task context section builder
//! - [`context`] - Error, quality, session, attempt, anti-pattern, and history sections
//! - [`intelligence`] - Code intelligence sections from narsil-mcp

pub mod context;
pub mod intelligence;
pub mod task;

// Re-export all section builder functions for convenient access
pub use context::{
    build_antipattern_section, build_attempt_section, build_error_section, build_history_section,
    build_language_rules_section, build_quality_section, build_session_section,
};
pub use intelligence::{
    build_call_graph_section, build_ccg_section, build_combined_intelligence_section,
    build_constraint_section, build_constraint_warnings_for, build_dependencies_section,
    build_intelligence_section, build_references_section, build_violations_section,
};
pub use task::build_task_section;
