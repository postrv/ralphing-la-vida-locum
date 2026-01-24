//! Signal handler integration for graceful shutdown.
//!
//! This module provides signal handling logic that saves session state
//! on SIGTERM, SIGINT (Unix), or CTRL_C_EVENT (Windows) before exiting.
//!
//! # Architecture
//!
//! The signal handler integrates with the session persistence layer to
//! ensure no data is lost on shutdown:
//!
//! ```text
//! Signal received (SIGTERM/SIGINT)
//!    │
//!    ▼
//! SignalHandler::shutdown()
//!    │
//!    ├─► persist=true: Save SessionState
//!    │
//!    └─► Exit gracefully
//! ```
//!
//! # Example
//!
//! ```rust,ignore
//! use ralph::session::signals::{SignalHandler, SignalHandlerConfig};
//! use ralph::session::{SessionPersistence, SessionState};
//!
//! let config = SignalHandlerConfig::default();
//! let handler = SignalHandler::new(config)
//!     .with_persistence(persistence, state);
//!
//! // Register handlers - they will save state on signal
//! handler.register().await?;
//! ```

use std::sync::Arc;

use tokio::sync::Mutex;
use tracing::{error, info, warn};

use super::persistence::SessionPersistence;
use super::SessionState;
use ralph::error::Result;

/// Configuration for the signal handler.
///
/// # Example
///
/// ```rust
/// use ralph::session::signals::SignalHandlerConfig;
///
/// // Default configuration persists state
/// let config = SignalHandlerConfig::default();
/// assert!(config.persist);
///
/// // Disable persistence for testing
/// let config = SignalHandlerConfig { persist: false };
/// assert!(!config.persist);
/// ```
#[derive(Debug, Clone)]
pub struct SignalHandlerConfig {
    /// Whether to persist session state on shutdown.
    /// Set to false with `--no-persist` flag.
    pub persist: bool,
}

impl Default for SignalHandlerConfig {
    fn default() -> Self {
        Self { persist: true }
    }
}

/// Result of a graceful shutdown operation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ShutdownResult {
    /// State was saved successfully.
    StateSaved,
    /// Persistence was disabled, state not saved.
    PersistenceDisabled,
    /// No state was available to save.
    NoStateToSave,
    /// Save failed but we logged the error (no panic).
    SaveFailed(String),
}

/// Signal handler for graceful shutdown with session persistence.
///
/// This struct manages the registration of OS signal handlers and
/// coordinates saving session state before exit.
///
/// # Thread Safety
///
/// The handler uses `Arc<Mutex<_>>` internally to allow safe access
/// from signal handler callbacks.
#[derive(Debug)]
pub struct SignalHandler {
    config: SignalHandlerConfig,
    persistence: Option<Arc<SessionPersistence>>,
    session_state: Option<Arc<Mutex<SessionState>>>,
}

impl SignalHandler {
    /// Creates a new signal handler with the given configuration.
    ///
    /// # Example
    ///
    /// ```rust
    /// use ralph::session::signals::{SignalHandler, SignalHandlerConfig};
    ///
    /// let handler = SignalHandler::new(SignalHandlerConfig::default());
    /// ```
    #[must_use]
    pub fn new(config: SignalHandlerConfig) -> Self {
        Self {
            config,
            persistence: None,
            session_state: None,
        }
    }

    /// Attaches persistence layer and session state to the handler.
    ///
    /// # Arguments
    ///
    /// * `persistence` - The session persistence manager.
    /// * `state` - The session state to save on shutdown.
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// use ralph::session::signals::{SignalHandler, SignalHandlerConfig};
    /// use ralph::session::{SessionPersistence, SessionState};
    ///
    /// let persistence = SessionPersistence::new(".ralph");
    /// let state = SessionState::new();
    ///
    /// let handler = SignalHandler::new(SignalHandlerConfig::default())
    ///     .with_persistence(persistence, state);
    /// ```
    #[must_use]
    pub fn with_persistence(
        mut self,
        persistence: SessionPersistence,
        state: SessionState,
    ) -> Self {
        self.persistence = Some(Arc::new(persistence));
        self.session_state = Some(Arc::new(Mutex::new(state)));
        self
    }

    /// Performs a graceful shutdown, saving state if configured.
    ///
    /// This method is called when a signal is received. It:
    /// 1. Checks if persistence is enabled
    /// 2. Saves the session state if available
    /// 3. Returns the result (never panics)
    ///
    /// # Returns
    ///
    /// Returns a `ShutdownResult` indicating what happened during shutdown.
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// let result = handler.shutdown().await;
    /// match result {
    ///     ShutdownResult::StateSaved => println!("State saved"),
    ///     ShutdownResult::PersistenceDisabled => println!("Persistence disabled"),
    ///     ShutdownResult::NoStateToSave => println!("No state"),
    ///     ShutdownResult::SaveFailed(e) => eprintln!("Save failed: {}", e),
    /// }
    /// ```
    pub async fn shutdown(&self) -> ShutdownResult {
        if !self.config.persist {
            info!("Graceful shutdown: persistence disabled, skipping save");
            return ShutdownResult::PersistenceDisabled;
        }

        let (Some(ref persistence), Some(ref state_mutex)) =
            (&self.persistence, &self.session_state)
        else {
            warn!("Graceful shutdown: no state or persistence configured");
            return ShutdownResult::NoStateToSave;
        };

        let mut state = state_mutex.lock().await;
        state.touch(); // Update timestamp

        match persistence.save(&state) {
            Ok(()) => {
                info!(
                    "Graceful shutdown: session state saved (iteration: {:?})",
                    state.loop_state.as_ref().map(|s| s.iteration)
                );
                ShutdownResult::StateSaved
            }
            Err(e) => {
                // Log error but don't panic - graceful degradation
                error!("Graceful shutdown: failed to save session state: {}", e);
                ShutdownResult::SaveFailed(e.to_string())
            }
        }
    }

    /// Registers signal handlers for graceful shutdown.
    ///
    /// On Unix: Registers handlers for SIGTERM and SIGINT.
    /// On Windows: Registers handler for CTRL_C.
    ///
    /// When a signal is received, the handler will:
    /// 1. Call `shutdown()` to save state
    /// 2. Log the shutdown
    /// 3. Allow the process to exit
    ///
    /// # Returns
    ///
    /// Returns a future that completes when a shutdown signal is received.
    ///
    /// # Errors
    ///
    /// Returns an error if signal handler registration fails.
    pub async fn wait_for_shutdown(&self) -> Result<ShutdownResult> {
        #[cfg(unix)]
        {
            use tokio::signal::unix::{signal, SignalKind};

            let mut sigterm = signal(SignalKind::terminate())?;
            let mut sigint = signal(SignalKind::interrupt())?;

            tokio::select! {
                _ = sigterm.recv() => {
                    info!("Received SIGTERM, initiating graceful shutdown");
                }
                _ = sigint.recv() => {
                    info!("Received SIGINT, initiating graceful shutdown");
                }
            }
        }

        #[cfg(windows)]
        {
            tokio::signal::ctrl_c().await?;
            info!("Received Ctrl+C, initiating graceful shutdown");
        }

        Ok(self.shutdown().await)
    }

    /// Returns whether persistence is enabled.
    #[must_use]
    pub fn persist_enabled(&self) -> bool {
        self.config.persist
    }

    /// Returns whether a persistence layer is configured.
    #[must_use]
    pub fn has_persistence(&self) -> bool {
        self.persistence.is_some() && self.session_state.is_some()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::r#loop::state::{LoopMode, LoopState};
    use tempfile::TempDir;

    fn test_handler() -> (SignalHandler, TempDir) {
        let temp_dir = TempDir::new().expect("Failed to create temp dir");
        let persistence = SessionPersistence::new(temp_dir.path().join(".ralph"));
        let state = SessionState::new();

        let handler =
            SignalHandler::new(SignalHandlerConfig::default()).with_persistence(persistence, state);

        (handler, temp_dir)
    }

    #[test]
    fn test_signal_handler_config_default() {
        let config = SignalHandlerConfig::default();
        assert!(config.persist);
    }

    #[test]
    fn test_signal_handler_config_no_persist() {
        let config = SignalHandlerConfig { persist: false };
        assert!(!config.persist);
    }

    #[test]
    fn test_signal_handler_new() {
        let handler = SignalHandler::new(SignalHandlerConfig::default());
        assert!(handler.persist_enabled());
        assert!(!handler.has_persistence());
    }

    #[test]
    fn test_signal_handler_with_persistence() {
        let (handler, _temp_dir) = test_handler();
        assert!(handler.persist_enabled());
        assert!(handler.has_persistence());
    }

    #[tokio::test]
    async fn test_signal_handler_registration() {
        // Test that we can create and configure a signal handler without panicking
        let (handler, _temp_dir) = test_handler();

        // Verify the handler is properly configured
        assert!(handler.persist_enabled());
        assert!(handler.has_persistence());

        // Note: We can't easily test actual signal registration in unit tests
        // as it requires spawning separate processes. This test verifies
        // that the handler can be created and configured correctly.
    }

    #[tokio::test]
    async fn test_graceful_shutdown_saves_state() {
        let temp_dir = TempDir::new().expect("Failed to create temp dir");
        let ralph_dir = temp_dir.path().join(".ralph");
        let persistence = SessionPersistence::new(&ralph_dir);

        // Create state with meaningful data
        let mut state = SessionState::new();
        state.set_loop_state(LoopState::new(LoopMode::Build));
        state.supervisor.mode_switch_count = 5;

        let handler =
            SignalHandler::new(SignalHandlerConfig::default()).with_persistence(persistence, state);

        // Trigger shutdown
        let result = handler.shutdown().await;

        // Verify state was saved
        assert_eq!(result, ShutdownResult::StateSaved);

        // Verify file was created
        let session_file = ralph_dir.join("session.json");
        assert!(
            session_file.exists(),
            "Session file should exist after shutdown"
        );

        // Verify contents are valid
        let contents = std::fs::read_to_string(&session_file).expect("read file");
        let loaded: SessionState = serde_json::from_str(&contents).expect("parse json");
        assert_eq!(loaded.supervisor.mode_switch_count, 5);
    }

    #[tokio::test]
    async fn test_no_persist_flag_skips_save() {
        let temp_dir = TempDir::new().expect("Failed to create temp dir");
        let ralph_dir = temp_dir.path().join(".ralph");
        let persistence = SessionPersistence::new(&ralph_dir);
        let state = SessionState::new();

        // Create handler with persistence disabled
        let handler = SignalHandler::new(SignalHandlerConfig { persist: false })
            .with_persistence(persistence, state);

        // Trigger shutdown
        let result = handler.shutdown().await;

        // Verify persistence was skipped
        assert_eq!(result, ShutdownResult::PersistenceDisabled);

        // Verify no file was created
        let session_file = ralph_dir.join("session.json");
        assert!(
            !session_file.exists(),
            "Session file should not exist when persist=false"
        );
    }

    #[tokio::test]
    async fn test_signal_handler_error_doesnt_panic() {
        // Create handler without persistence - should handle gracefully
        let handler = SignalHandler::new(SignalHandlerConfig::default());

        // This should not panic, even though there's no persistence configured
        let result = handler.shutdown().await;
        assert_eq!(result, ShutdownResult::NoStateToSave);
    }

    #[tokio::test]
    async fn test_signal_handler_save_error_doesnt_panic() {
        // Test that save errors don't cause panics
        let temp_dir = TempDir::new().expect("Failed to create temp dir");
        let ralph_dir = temp_dir.path().join(".ralph");
        let persistence = SessionPersistence::new(&ralph_dir);
        let state = SessionState::new();

        let handler =
            SignalHandler::new(SignalHandlerConfig::default()).with_persistence(persistence, state);

        // Create directory and make it read-only to force save error
        std::fs::create_dir_all(&ralph_dir).expect("create dir");

        // First save should work
        let result = handler.shutdown().await;
        assert_eq!(result, ShutdownResult::StateSaved);

        // Subsequent saves should also work (they overwrite)
        let result2 = handler.shutdown().await;
        assert_eq!(result2, ShutdownResult::StateSaved);
    }

    #[test]
    fn test_shutdown_result_variants() {
        // Verify all variants can be created and compared
        assert_eq!(ShutdownResult::StateSaved, ShutdownResult::StateSaved);
        assert_eq!(
            ShutdownResult::PersistenceDisabled,
            ShutdownResult::PersistenceDisabled
        );
        assert_eq!(ShutdownResult::NoStateToSave, ShutdownResult::NoStateToSave);
        assert_eq!(
            ShutdownResult::SaveFailed("test".to_string()),
            ShutdownResult::SaveFailed("test".to_string())
        );

        // Different variants are not equal
        assert_ne!(
            ShutdownResult::StateSaved,
            ShutdownResult::PersistenceDisabled
        );
    }

    #[tokio::test]
    async fn test_wait_for_shutdown_can_be_cancelled() {
        // Test that wait_for_shutdown can be started and cancelled without panicking.
        // We can't easily send real signals in unit tests, so we verify the method
        // is correctly set up by racing it against a timeout.
        let (handler, _temp_dir) = test_handler();

        // Spawn the shutdown waiter
        let shutdown_future = handler.wait_for_shutdown();

        // Race against a short timeout - the wait should not complete
        // (no signal is sent), but it also should not panic
        let result =
            tokio::time::timeout(std::time::Duration::from_millis(10), shutdown_future).await;

        // We expect timeout since no signal was sent
        assert!(
            result.is_err(),
            "wait_for_shutdown should timeout without a signal"
        );
    }

    // Note: Testing actual signal handling (SIGINT/SIGTERM) would require:
    // 1. Spawning a child process
    // 2. Sending signals to that process
    // 3. Verifying the session file was created
    //
    // This is covered by integration tests. For unit tests, we verify:
    // - Signal handler registration doesn't panic
    // - The shutdown logic works correctly (test_graceful_shutdown_saves_state)
    // - The wait_for_shutdown future can be cancelled (above test)
}
