//! Ralph - Claude Code Automation Suite
//!
//! A Rust-based automation suite for running Claude Code autonomously
//! with bombproof reliability, type-checking, and memory guarantees.

pub mod config;
pub mod error;

// Re-export commonly used types
pub use error::{IntoRalphError, RalphError, Result};

// Re-export config types
pub use config::{
    is_ssh_command, suggest_gh_alternative, verify_git_environment, GitEnvironmentCheck,
    ProjectConfig, StagnationLevel, DANGEROUS_PATTERNS, SECRET_PATTERNS, SSH_BLOCKED_PATTERNS,
};
