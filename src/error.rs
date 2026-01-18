//! Custom error types for Ralph.
//!
//! This module provides structured error types that enable better
//! error handling, reporting, and recovery throughout the application.

use std::path::PathBuf;
use thiserror::Error;

/// Main error type for Ralph operations
#[derive(Error, Debug)]
pub enum RalphError {
    // =========================================================================
    // Configuration Errors
    // =========================================================================
    /// Failed to load configuration
    #[error("Configuration error: {message}")]
    Config {
        message: String,
        path: Option<PathBuf>,
    },

    /// Invalid configuration value
    #[error("Invalid configuration: {field} - {reason}")]
    InvalidConfig { field: String, reason: String },

    /// Missing required file
    #[error("Missing required file: {path}")]
    MissingFile { path: PathBuf },

    // =========================================================================
    // Loop Execution Errors
    // =========================================================================
    /// Loop execution failed
    #[error("Loop execution error: {message}")]
    Loop { message: String },

    /// Stagnation limit reached
    #[error("Stagnation limit reached after {iterations} iterations (threshold: {threshold})")]
    StagnationLimit { iterations: u32, threshold: u32 },

    /// Claude Code process failed
    #[error("Claude Code process failed with exit code {exit_code}: {message}")]
    ClaudeProcess { exit_code: i32, message: String },

    /// Maximum iterations exceeded
    #[error("Maximum iterations ({max}) exceeded without completion")]
    MaxIterations { max: u32 },

    // =========================================================================
    // Security Errors
    // =========================================================================
    /// Command blocked by security filter
    #[error("Security violation: {message}")]
    Security {
        message: String,
        command: Option<String>,
    },

    /// Dangerous command detected
    #[error("Dangerous command blocked: {pattern}")]
    DangerousCommand { pattern: String, command: String },

    /// Secret detected in code
    #[error("Secret detected in {file}: {pattern}")]
    SecretDetected { file: PathBuf, pattern: String },

    /// SSH access attempted (blocked)
    #[error("SSH access blocked - use gh CLI instead: {detail}")]
    SshBlocked { detail: String },

    // =========================================================================
    // Hook Errors
    // =========================================================================
    /// Hook execution failed
    #[error("Hook '{name}' failed: {message}")]
    Hook { name: String, message: String },

    /// Hook validation failed
    #[error("Hook validation failed: {reason}")]
    HookValidation { reason: String },

    // =========================================================================
    // Supervisor Errors
    // =========================================================================
    /// Supervisor abort
    #[error("Supervisor abort: {reason}")]
    SupervisorAbort { reason: String },

    /// Supervisor requested pause
    #[error("Supervisor pause: {reason}")]
    SupervisorPause { reason: String },

    // =========================================================================
    // Tool Errors
    // =========================================================================
    /// Missing required tool
    #[error("Missing required tool: {tool}")]
    MissingTool { tool: String },

    /// Tool execution failed
    #[error("Tool '{tool}' failed: {message}")]
    ToolExecution { tool: String, message: String },

    /// narsil-mcp not available
    #[error("narsil-mcp not available: {detail}")]
    NarsilUnavailable { detail: String },

    /// Git operation failed
    #[error("Git operation failed: {operation} - {message}")]
    Git { operation: String, message: String },

    // =========================================================================
    // Archive Errors
    // =========================================================================
    /// Archive operation failed
    #[error("Archive error: {message}")]
    Archive { message: String },

    /// Restore failed
    #[error("Failed to restore from archive: {path}")]
    RestoreFailed { path: PathBuf },

    // =========================================================================
    // Context Errors
    // =========================================================================
    /// Context building failed
    #[error("Context build error: {message}")]
    Context { message: String },

    /// Token limit exceeded
    #[error("Token limit exceeded: {actual} > {limit}")]
    TokenLimit { actual: usize, limit: usize },

    // =========================================================================
    // Wrapped Errors
    // =========================================================================
    /// IO error wrapper
    #[error(transparent)]
    Io(#[from] std::io::Error),

    /// JSON error wrapper
    #[error(transparent)]
    Json(#[from] serde_json::Error),

    /// Generic error wrapper
    #[error(transparent)]
    Other(#[from] anyhow::Error),
}

impl RalphError {
    // =========================================================================
    // Constructor helpers
    // =========================================================================

    /// Create a configuration error
    pub fn config(message: impl Into<String>) -> Self {
        Self::Config {
            message: message.into(),
            path: None,
        }
    }

    /// Create a configuration error with path
    pub fn config_with_path(message: impl Into<String>, path: PathBuf) -> Self {
        Self::Config {
            message: message.into(),
            path: Some(path),
        }
    }

    /// Create a security error
    pub fn security(message: impl Into<String>) -> Self {
        Self::Security {
            message: message.into(),
            command: None,
        }
    }

    /// Create a security error with command
    pub fn security_with_command(message: impl Into<String>, command: impl Into<String>) -> Self {
        Self::Security {
            message: message.into(),
            command: Some(command.into()),
        }
    }

    /// Create a loop error
    pub fn loop_error(message: impl Into<String>) -> Self {
        Self::Loop {
            message: message.into(),
        }
    }

    /// Create a hook error
    pub fn hook(name: impl Into<String>, message: impl Into<String>) -> Self {
        Self::Hook {
            name: name.into(),
            message: message.into(),
        }
    }

    /// Create a git error
    pub fn git(operation: impl Into<String>, message: impl Into<String>) -> Self {
        Self::Git {
            operation: operation.into(),
            message: message.into(),
        }
    }

    // =========================================================================
    // Classification helpers
    // =========================================================================

    /// Check if this error is recoverable
    pub fn is_recoverable(&self) -> bool {
        matches!(
            self,
            Self::Loop { .. }
                | Self::Hook { .. }
                | Self::HookValidation { .. }
                | Self::Git { .. }
                | Self::ToolExecution { .. }
        )
    }

    /// Check if this error requires human intervention
    pub fn requires_human(&self) -> bool {
        matches!(
            self,
            Self::SupervisorPause { .. }
                | Self::Security { .. }
                | Self::DangerousCommand { .. }
                | Self::SecretDetected { .. }
                | Self::SshBlocked { .. }
        )
    }

    /// Check if this error is fatal (should abort loop)
    pub fn is_fatal(&self) -> bool {
        matches!(
            self,
            Self::SupervisorAbort { .. }
                | Self::StagnationLimit { .. }
                | Self::MaxIterations { .. }
                | Self::MissingFile { .. }
                | Self::MissingTool { .. }
        )
    }

    /// Get error code for exit status
    pub fn exit_code(&self) -> i32 {
        match self {
            Self::Security { .. } | Self::DangerousCommand { .. } | Self::SshBlocked { .. } => 2,
            Self::StagnationLimit { .. } => 3,
            Self::SupervisorAbort { .. } => 4,
            Self::SupervisorPause { .. } => 5,
            Self::MissingFile { .. } | Self::MissingTool { .. } => 6,
            Self::Config { .. } | Self::InvalidConfig { .. } => 7,
            _ => 1,
        }
    }
}

/// Type alias for Ralph results
pub type Result<T> = std::result::Result<T, RalphError>;

/// Extension trait for converting anyhow errors to RalphError
pub trait IntoRalphError<T> {
    fn into_ralph_config(self) -> Result<T>;
    fn into_ralph_loop(self) -> Result<T>;
    fn into_ralph_hook(self, name: &str) -> Result<T>;
}

impl<T, E: Into<anyhow::Error>> IntoRalphError<T> for std::result::Result<T, E> {
    fn into_ralph_config(self) -> Result<T> {
        self.map_err(|e| RalphError::config(e.into().to_string()))
    }

    fn into_ralph_loop(self) -> Result<T> {
        self.map_err(|e| RalphError::loop_error(e.into().to_string()))
    }

    fn into_ralph_hook(self, name: &str) -> Result<T> {
        self.map_err(|e| RalphError::hook(name, e.into().to_string()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_error_display() {
        let err = RalphError::StagnationLimit {
            iterations: 15,
            threshold: 5,
        };
        assert!(err.to_string().contains("15"));
        assert!(err.to_string().contains("5"));
    }

    #[test]
    fn test_is_recoverable() {
        assert!(RalphError::loop_error("test").is_recoverable());
        assert!(RalphError::hook("test", "error").is_recoverable());
        assert!(!RalphError::SupervisorAbort {
            reason: "test".into()
        }
        .is_recoverable());
    }

    #[test]
    fn test_is_fatal() {
        assert!(RalphError::SupervisorAbort {
            reason: "test".into()
        }
        .is_fatal());
        assert!(RalphError::StagnationLimit {
            iterations: 15,
            threshold: 5
        }
        .is_fatal());
        assert!(!RalphError::loop_error("test").is_fatal());
    }

    #[test]
    fn test_requires_human() {
        assert!(RalphError::security("blocked").requires_human());
        assert!(RalphError::SshBlocked {
            detail: "test".into()
        }
        .requires_human());
        assert!(!RalphError::loop_error("test").requires_human());
    }

    #[test]
    fn test_exit_codes() {
        assert_eq!(RalphError::security("test").exit_code(), 2);
        assert_eq!(
            RalphError::StagnationLimit {
                iterations: 15,
                threshold: 5
            }
            .exit_code(),
            3
        );
        assert_eq!(RalphError::config("test").exit_code(), 7);
    }

    #[test]
    fn test_constructor_helpers() {
        let err = RalphError::security_with_command("blocked", "rm -rf /");
        if let RalphError::Security { command, .. } = err {
            assert_eq!(command, Some("rm -rf /".to_string()));
        } else {
            panic!("Wrong error variant");
        }
    }

    #[test]
    fn test_config_with_path() {
        let path = PathBuf::from("/test/path.toml");
        let err = RalphError::config_with_path("failed to parse", path.clone());
        if let RalphError::Config {
            message,
            path: opt_path,
        } = err
        {
            assert_eq!(message, "failed to parse");
            assert_eq!(opt_path, Some(path));
        } else {
            panic!("Wrong error variant");
        }
    }

    #[test]
    fn test_git_error() {
        let err = RalphError::git("push", "authentication failed");
        if let RalphError::Git { operation, message } = err {
            assert_eq!(operation, "push");
            assert_eq!(message, "authentication failed");
        } else {
            panic!("Wrong error variant");
        }
    }

    #[test]
    fn test_hook_error() {
        let err = RalphError::hook("pre-commit", "script returned non-zero");
        if let RalphError::Hook { name, message } = err {
            assert_eq!(name, "pre-commit");
            assert_eq!(message, "script returned non-zero");
        } else {
            panic!("Wrong error variant");
        }
    }

    #[test]
    fn test_into_ralph_error_trait() {
        let result: std::result::Result<(), std::io::Error> = Err(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            "file not found",
        ));

        let ralph_result = result.into_ralph_config();
        assert!(ralph_result.is_err());

        if let Err(RalphError::Config { message, .. }) = ralph_result {
            assert!(message.contains("file not found"));
        } else {
            panic!("Wrong error variant after conversion");
        }
    }

    #[test]
    fn test_error_from_io() {
        let io_err = std::io::Error::new(std::io::ErrorKind::PermissionDenied, "access denied");
        let ralph_err: RalphError = io_err.into();
        assert!(matches!(ralph_err, RalphError::Io(_)));
        assert!(ralph_err.to_string().contains("access denied"));
    }
}
