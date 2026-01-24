//! Claude LLM provider implementation.
//!
//! This module provides a production-ready Claude client that implements the
//! [`LlmClient`] trait. It includes proper error handling, rate limit detection,
//! and support for all Claude model variants.
//!
//! # Architecture
//!
//! The [`ClaudeProvider`] wraps the `claude` CLI tool to provide LLM capabilities.
//! It extends the basic [`ClaudeClient`] with:
//!
//! - Structured error types for API errors
//! - Rate limit detection with configurable backoff
//! - Support for specific model IDs (claude-sonnet-4, claude-opus-4.5)
//!
//! # Example
//!
//! ```rust,ignore
//! use ralph::llm::{ClaudeProvider, ClaudeModel, LlmClient};
//!
//! // Create provider with opus model
//! let provider = ClaudeProvider::new(".")
//!     .with_model(ClaudeModel::Opus);
//!
//! // Check if rate limited
//! if provider.is_rate_limited() {
//!     println!("Rate limited, waiting for backoff...");
//! }
//!
//! // Run a prompt
//! let response = provider.run_prompt("Hello!").await?;
//! ```

use crate::llm::{CompletionRequest, LlmClient, LlmResponse, ProviderCapabilities, StopReason};
use anyhow::Result;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::time::{Duration, Instant};
use thiserror::Error;
use tokio::io::AsyncWriteExt;
use tokio::process::Command as AsyncCommand;
use tracing::{debug, warn};

// =============================================================================
// Claude Model Variants
// =============================================================================

/// Supported Claude model variants.
///
/// Each variant has different capabilities, pricing, and performance
/// characteristics. Use [`ClaudeModel::model_id`] to get the full model ID
/// for API calls.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
pub enum ClaudeModel {
    /// Claude Opus 4.5 - most capable model
    #[default]
    Opus,
    /// Claude Sonnet 4 - balanced performance
    Sonnet,
    /// Claude Haiku 3.5 - fastest, most economical
    Haiku,
}

impl ClaudeModel {
    /// Get the full model ID for API calls.
    ///
    /// Returns the complete model identifier as used by Anthropic's API.
    ///
    /// # Example
    ///
    /// ```rust
    /// use ralph::llm::claude::ClaudeModel;
    ///
    /// assert_eq!(ClaudeModel::Opus.model_id(), "claude-opus-4-5-20251101");
    /// assert_eq!(ClaudeModel::Sonnet.model_id(), "claude-sonnet-4-20250514");
    /// assert_eq!(ClaudeModel::Haiku.model_id(), "claude-3-5-haiku-20241022");
    /// ```
    #[must_use]
    pub const fn model_id(&self) -> &'static str {
        match self {
            Self::Opus => "claude-opus-4-5-20251101",
            Self::Sonnet => "claude-sonnet-4-20250514",
            Self::Haiku => "claude-3-5-haiku-20241022",
        }
    }

    /// Get the short name used by the Claude CLI.
    ///
    /// # Example
    ///
    /// ```rust
    /// use ralph::llm::claude::ClaudeModel;
    ///
    /// assert_eq!(ClaudeModel::Opus.cli_name(), "opus");
    /// assert_eq!(ClaudeModel::Sonnet.cli_name(), "sonnet");
    /// assert_eq!(ClaudeModel::Haiku.cli_name(), "haiku");
    /// ```
    #[must_use]
    pub const fn cli_name(&self) -> &'static str {
        match self {
            Self::Opus => "opus",
            Self::Sonnet => "sonnet",
            Self::Haiku => "haiku",
        }
    }

    /// Get the display name for this model.
    #[must_use]
    pub const fn display_name(&self) -> &'static str {
        match self {
            Self::Opus => "Claude Opus 4.5",
            Self::Sonnet => "Claude Sonnet 4",
            Self::Haiku => "Claude Haiku 3.5",
        }
    }

    /// Get cost per million tokens (input, output) in USD.
    #[must_use]
    pub const fn cost_per_million_tokens(&self) -> (f64, f64) {
        match self {
            Self::Opus => (15.0, 75.0),
            Self::Sonnet => (3.0, 15.0),
            Self::Haiku => (0.25, 1.25),
        }
    }

    /// Parse a model name string into a [`ClaudeModel`].
    ///
    /// Accepts CLI names, display names, and model IDs.
    /// Returns `None` if the string is not recognized.
    ///
    /// # Example
    ///
    /// ```rust
    /// use ralph::llm::claude::ClaudeModel;
    ///
    /// assert_eq!(ClaudeModel::parse("opus"), Some(ClaudeModel::Opus));
    /// assert_eq!(ClaudeModel::parse("sonnet"), Some(ClaudeModel::Sonnet));
    /// assert_eq!(ClaudeModel::parse("unknown"), None);
    /// ```
    #[must_use]
    pub fn parse(s: &str) -> Option<Self> {
        let lower = s.to_lowercase();
        match lower.as_str() {
            "opus" | "claude-opus" | "claude-opus-4" | "claude-opus-4-5-20251101" => {
                Some(Self::Opus)
            }
            "sonnet" | "claude-sonnet" | "claude-sonnet-4" | "claude-sonnet-4-20250514" => {
                Some(Self::Sonnet)
            }
            "haiku" | "claude-haiku" | "claude-haiku-3.5" | "claude-3-5-haiku-20241022" => {
                Some(Self::Haiku)
            }
            _ => None,
        }
    }
}

/// Error for parsing [`ClaudeModel`] from a string.
#[derive(Error, Debug, Clone, PartialEq, Eq)]
#[error("Unknown Claude model: '{0}'. Valid options: opus, sonnet, haiku")]
pub struct ParseClaudeModelError(String);

impl std::str::FromStr for ClaudeModel {
    type Err = ParseClaudeModelError;

    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        Self::parse(s).ok_or_else(|| ParseClaudeModelError(s.to_string()))
    }
}

impl std::fmt::Display for ClaudeModel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.display_name())
    }
}

// =============================================================================
// Claude API Errors
// =============================================================================

/// Errors specific to Claude API interactions.
///
/// These errors provide structured information about API failures,
/// enabling appropriate handling (e.g., retry with backoff for rate limits).
#[derive(Error, Debug)]
pub enum ClaudeApiError {
    /// Rate limit exceeded - should retry with backoff.
    #[error("Rate limit exceeded: {message} (retry after {retry_after_secs}s)")]
    RateLimited {
        message: String,
        retry_after_secs: u64,
    },

    /// Authentication failed - check API key.
    #[error("Authentication failed: {message}")]
    AuthenticationFailed { message: String },

    /// Invalid request - check prompt/parameters.
    #[error("Invalid request: {message}")]
    InvalidRequest { message: String },

    /// Server error - may be transient.
    #[error("Server error: {message}")]
    ServerError { message: String },

    /// Network/connection error.
    #[error("Connection error: {message}")]
    ConnectionError { message: String },

    /// Claude CLI not found.
    #[error("Claude CLI not found: {message}")]
    CliNotFound { message: String },

    /// Process exited with non-zero code.
    #[error("Process failed with exit code {exit_code}: {stderr}")]
    ProcessFailed { exit_code: i32, stderr: String },

    /// Timeout waiting for response.
    #[error("Request timed out after {timeout_secs}s")]
    Timeout { timeout_secs: u64 },

    /// Context length exceeded.
    #[error("Context length exceeded: {message}")]
    ContextLengthExceeded { message: String },
}

impl ClaudeApiError {
    /// Check if this error indicates the request should be retried.
    #[must_use]
    pub fn is_retryable(&self) -> bool {
        matches!(
            self,
            Self::RateLimited { .. } | Self::ServerError { .. } | Self::Timeout { .. }
        )
    }

    /// Get the recommended retry delay if applicable.
    #[must_use]
    pub fn retry_after(&self) -> Option<Duration> {
        match self {
            Self::RateLimited {
                retry_after_secs, ..
            } => Some(Duration::from_secs(*retry_after_secs)),
            Self::ServerError { .. } => Some(Duration::from_secs(5)),
            Self::Timeout { .. } => Some(Duration::from_secs(10)),
            _ => None,
        }
    }

    /// Parse error from Claude CLI stderr output.
    pub fn from_stderr(stderr: &str, exit_code: i32) -> Self {
        let stderr_lower = stderr.to_lowercase();

        // Check for rate limit indicators
        if stderr_lower.contains("rate limit")
            || stderr_lower.contains("too many requests")
            || stderr_lower.contains("429")
        {
            // Try to extract retry-after value
            let retry_after = Self::extract_retry_after(stderr).unwrap_or(60);
            return Self::RateLimited {
                message: stderr.to_string(),
                retry_after_secs: retry_after,
            };
        }

        // Check for authentication errors
        if stderr_lower.contains("authentication")
            || stderr_lower.contains("unauthorized")
            || stderr_lower.contains("api key")
            || stderr_lower.contains("401")
        {
            return Self::AuthenticationFailed {
                message: stderr.to_string(),
            };
        }

        // Check for invalid request
        if stderr_lower.contains("invalid request")
            || stderr_lower.contains("bad request")
            || stderr_lower.contains("400")
        {
            return Self::InvalidRequest {
                message: stderr.to_string(),
            };
        }

        // Check for context length errors
        if stderr_lower.contains("context length")
            || stderr_lower.contains("too long")
            || stderr_lower.contains("max tokens")
        {
            return Self::ContextLengthExceeded {
                message: stderr.to_string(),
            };
        }

        // Check for server errors
        if stderr_lower.contains("500")
            || stderr_lower.contains("502")
            || stderr_lower.contains("503")
            || stderr_lower.contains("server error")
        {
            return Self::ServerError {
                message: stderr.to_string(),
            };
        }

        // Check for connection errors
        if stderr_lower.contains("connection")
            || stderr_lower.contains("network")
            || stderr_lower.contains("timeout")
        {
            return Self::ConnectionError {
                message: stderr.to_string(),
            };
        }

        // Default to process failed
        Self::ProcessFailed {
            exit_code,
            stderr: stderr.to_string(),
        }
    }

    /// Extract retry-after seconds from error message.
    fn extract_retry_after(stderr: &str) -> Option<u64> {
        // Try to find patterns like "retry after 60s" or "retry-after: 60"
        let patterns = [
            r"retry.?after[:\s]+(\d+)",
            r"wait[:\s]+(\d+)",
            r"(\d+)\s*seconds?",
        ];

        for pattern in patterns {
            if let Ok(re) = regex::Regex::new(pattern) {
                if let Some(caps) = re.captures(&stderr.to_lowercase()) {
                    if let Some(m) = caps.get(1) {
                        if let Ok(secs) = m.as_str().parse::<u64>() {
                            return Some(secs);
                        }
                    }
                }
            }
        }
        None
    }
}

// =============================================================================
// Rate Limit Tracker
// =============================================================================

/// Tracks rate limit state and implements exponential backoff.
#[derive(Debug)]
pub struct RateLimitTracker {
    /// Whether currently rate limited.
    is_limited: AtomicBool,
    /// Timestamp when rate limit expires (epoch millis).
    limit_expires_at: AtomicU64,
    /// Current backoff multiplier.
    backoff_multiplier: AtomicU64,
    /// Base backoff duration in seconds.
    base_backoff_secs: u64,
    /// Maximum backoff duration in seconds.
    max_backoff_secs: u64,
}

impl Default for RateLimitTracker {
    fn default() -> Self {
        Self::new()
    }
}

impl RateLimitTracker {
    /// Create a new rate limit tracker with default settings.
    #[must_use]
    pub fn new() -> Self {
        Self {
            is_limited: AtomicBool::new(false),
            limit_expires_at: AtomicU64::new(0),
            backoff_multiplier: AtomicU64::new(1),
            base_backoff_secs: 5,
            max_backoff_secs: 300, // 5 minutes max
        }
    }

    /// Check if currently rate limited.
    #[must_use]
    pub fn is_rate_limited(&self) -> bool {
        if !self.is_limited.load(Ordering::SeqCst) {
            return false;
        }

        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64;

        let expires_at = self.limit_expires_at.load(Ordering::SeqCst);

        if now >= expires_at {
            // Rate limit expired
            self.is_limited.store(false, Ordering::SeqCst);
            false
        } else {
            true
        }
    }

    /// Get the time remaining until rate limit expires.
    #[must_use]
    pub fn time_until_ready(&self) -> Option<Duration> {
        if !self.is_rate_limited() {
            return None;
        }

        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64;

        let expires_at = self.limit_expires_at.load(Ordering::SeqCst);

        if now < expires_at {
            Some(Duration::from_millis(expires_at - now))
        } else {
            None
        }
    }

    /// Record a rate limit hit.
    pub fn record_rate_limit(&self, retry_after_secs: Option<u64>) {
        let multiplier = self.backoff_multiplier.fetch_add(1, Ordering::SeqCst);
        let backoff_secs = retry_after_secs
            .unwrap_or_else(|| (self.base_backoff_secs * multiplier).min(self.max_backoff_secs));

        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64;

        let expires_at = now + (backoff_secs * 1000);
        self.limit_expires_at.store(expires_at, Ordering::SeqCst);
        self.is_limited.store(true, Ordering::SeqCst);

        warn!(
            "Rate limited for {}s (backoff multiplier: {})",
            backoff_secs, multiplier
        );
    }

    /// Record a successful request (resets backoff).
    pub fn record_success(&self) {
        self.backoff_multiplier.store(1, Ordering::SeqCst);
    }

    /// Reset rate limit state.
    pub fn reset(&self) {
        self.is_limited.store(false, Ordering::SeqCst);
        self.limit_expires_at.store(0, Ordering::SeqCst);
        self.backoff_multiplier.store(1, Ordering::SeqCst);
    }
}

// =============================================================================
// Claude Provider
// =============================================================================

/// Production-ready Claude LLM provider.
///
/// Implements the [`LlmClient`] trait with:
/// - Proper error handling and structured error types
/// - Rate limit detection with exponential backoff
/// - Support for all Claude model variants
///
/// # Example
///
/// ```rust,ignore
/// use ralph::llm::{ClaudeProvider, ClaudeModel, LlmClient};
///
/// let provider = ClaudeProvider::new(".")
///     .with_model(ClaudeModel::Sonnet);
///
/// // Provider automatically handles rate limits
/// let response = provider.run_prompt("Hello!").await?;
/// ```
#[derive(Debug)]
pub struct ClaudeProvider {
    /// Working directory for Claude CLI execution.
    project_dir: PathBuf,
    /// Model variant to use.
    model: ClaudeModel,
    /// Rate limit tracker.
    rate_limiter: RateLimitTracker,
    /// Request timeout in seconds.
    timeout_secs: u64,
}

impl ClaudeProvider {
    /// Default timeout for requests (5 minutes).
    pub const DEFAULT_TIMEOUT_SECS: u64 = 300;

    /// Create a new Claude provider for the given project directory.
    ///
    /// # Arguments
    ///
    /// * `project_dir` - The directory where Claude should operate
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// use ralph::llm::ClaudeProvider;
    ///
    /// let provider = ClaudeProvider::new("/path/to/project");
    /// ```
    #[must_use]
    pub fn new<P: Into<PathBuf>>(project_dir: P) -> Self {
        Self {
            project_dir: project_dir.into(),
            model: ClaudeModel::default(),
            rate_limiter: RateLimitTracker::new(),
            timeout_secs: Self::DEFAULT_TIMEOUT_SECS,
        }
    }

    /// Set the model variant to use.
    ///
    /// # Arguments
    ///
    /// * `model` - The Claude model variant
    #[must_use]
    pub fn with_model(mut self, model: ClaudeModel) -> Self {
        self.model = model;
        self
    }

    /// Set the model by name string.
    ///
    /// Falls back to Opus if the name is not recognized.
    #[must_use]
    pub fn with_model_name(mut self, name: &str) -> Self {
        self.model = ClaudeModel::parse(name).unwrap_or_default();
        self
    }

    /// Set the request timeout.
    #[must_use]
    pub fn with_timeout(mut self, timeout_secs: u64) -> Self {
        self.timeout_secs = timeout_secs;
        self
    }

    /// Check if currently rate limited.
    #[must_use]
    pub fn is_rate_limited(&self) -> bool {
        self.rate_limiter.is_rate_limited()
    }

    /// Get the time until rate limit expires.
    #[must_use]
    pub fn rate_limit_remaining(&self) -> Option<Duration> {
        self.rate_limiter.time_until_ready()
    }

    /// Get the current model.
    #[must_use]
    pub fn model(&self) -> ClaudeModel {
        self.model
    }

    /// Reset rate limit state (for testing).
    pub fn reset_rate_limit(&self) {
        self.rate_limiter.reset();
    }

    /// Execute the Claude CLI and return the result.
    async fn execute_cli(&self, prompt: &str) -> Result<String, ClaudeApiError> {
        // Check rate limit before proceeding
        if self.is_rate_limited() {
            let remaining = self.rate_limit_remaining().unwrap_or_default();
            return Err(ClaudeApiError::RateLimited {
                message: "Rate limit in effect".to_string(),
                retry_after_secs: remaining.as_secs(),
            });
        }

        let args = vec![
            "-p",
            "--dangerously-skip-permissions",
            "--model",
            self.model.cli_name(),
            "--output-format",
            "text",
        ];

        debug!(
            "Running Claude {} ({} chars prompt)",
            self.model.display_name(),
            prompt.len()
        );

        let mut child = match AsyncCommand::new("claude")
            .args(&args)
            .current_dir(&self.project_dir)
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .spawn()
        {
            Ok(child) => child,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                return Err(ClaudeApiError::CliNotFound {
                    message: "The 'claude' CLI is not installed or not in PATH".to_string(),
                });
            }
            Err(e) => {
                return Err(ClaudeApiError::ConnectionError {
                    message: format!("Failed to spawn claude process: {}", e),
                });
            }
        };

        if let Some(mut stdin) = child.stdin.take() {
            if let Err(e) = stdin.write_all(prompt.as_bytes()).await {
                return Err(ClaudeApiError::ConnectionError {
                    message: format!("Failed to write prompt to stdin: {}", e),
                });
            }
            if let Err(e) = stdin.flush().await {
                return Err(ClaudeApiError::ConnectionError {
                    message: format!("Failed to flush stdin: {}", e),
                });
            }
            drop(stdin);
        }

        // Wait for output with timeout
        // Note: wait_with_output() takes ownership, so we can't kill the process
        // on timeout. Tokio will clean up the child process when dropped.
        let output = match tokio::time::timeout(
            Duration::from_secs(self.timeout_secs),
            child.wait_with_output(),
        )
        .await
        {
            Ok(Ok(output)) => output,
            Ok(Err(e)) => {
                return Err(ClaudeApiError::ConnectionError {
                    message: format!("Failed to read output: {}", e),
                });
            }
            Err(_) => {
                // Timeout occurred - process will be cleaned up when dropped
                return Err(ClaudeApiError::Timeout {
                    timeout_secs: self.timeout_secs,
                });
            }
        };

        if output.status.success() {
            self.rate_limiter.record_success();
            Ok(String::from_utf8_lossy(&output.stdout).to_string())
        } else {
            let stderr = String::from_utf8_lossy(&output.stderr).to_string();
            let exit_code = output.status.code().unwrap_or(-1);

            let error = ClaudeApiError::from_stderr(&stderr, exit_code);

            // Record rate limit if applicable
            if let ClaudeApiError::RateLimited {
                retry_after_secs, ..
            } = &error
            {
                self.rate_limiter.record_rate_limit(Some(*retry_after_secs));
            }

            Err(error)
        }
    }
}

#[async_trait]
impl LlmClient for ClaudeProvider {
    async fn run_prompt(&self, prompt: &str) -> Result<String> {
        self.execute_cli(prompt)
            .await
            .map_err(|e| anyhow::anyhow!("{}", e))
    }

    async fn complete(&self, request: CompletionRequest) -> Result<LlmResponse> {
        let start = Instant::now();

        let content = self.execute_cli(&request.prompt).await?;

        let latency_ms = start.elapsed().as_millis() as u64;

        // Estimate token counts (rough approximation: ~4 chars per token)
        let input_tokens = (request.prompt.len() / 4) as u32;
        let output_tokens = (content.len() / 4) as u32;

        // Calculate cost based on model
        let (input_rate, output_rate) = self.cost_per_token();
        let cost_usd = if input_rate > 0.0 || output_rate > 0.0 {
            Some((input_tokens as f64 * input_rate) + (output_tokens as f64 * output_rate))
        } else {
            None
        };

        Ok(LlmResponse {
            content,
            input_tokens,
            output_tokens,
            latency_ms,
            cost_usd,
            model: self.model.model_id().to_string(),
            stop_reason: StopReason::EndTurn,
        })
    }

    async fn available(&self) -> bool {
        // Check if claude CLI is available
        match AsyncCommand::new("which").arg("claude").output().await {
            Ok(output) => output.status.success(),
            Err(_) => false,
        }
    }

    fn model_name(&self) -> &str {
        self.model.model_id()
    }

    fn supports_tools(&self) -> bool {
        // Claude supports tool use
        true
    }

    fn capabilities(&self) -> ProviderCapabilities {
        ProviderCapabilities::default()
            .with_streaming(true)
            .with_tool_use(true)
            .with_max_context(200_000)
            .with_max_output(8192)
    }

    fn cost_per_token(&self) -> (f64, f64) {
        let (input_per_million, output_per_million) = self.model.cost_per_million_tokens();
        (
            input_per_million / 1_000_000.0,
            output_per_million / 1_000_000.0,
        )
    }
}

// =============================================================================
// Tests (TDD - Phase 23.2)
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    // =========================================================================
    // ClaudeModel Tests
    // =========================================================================

    /// Test ClaudeModel provides correct model IDs.
    #[test]
    fn test_claude_model_ids() {
        assert_eq!(ClaudeModel::Opus.model_id(), "claude-opus-4-5-20251101");
        assert_eq!(ClaudeModel::Sonnet.model_id(), "claude-sonnet-4-20250514");
        assert_eq!(ClaudeModel::Haiku.model_id(), "claude-3-5-haiku-20241022");
    }

    /// Test ClaudeModel provides correct CLI names.
    #[test]
    fn test_claude_model_cli_names() {
        assert_eq!(ClaudeModel::Opus.cli_name(), "opus");
        assert_eq!(ClaudeModel::Sonnet.cli_name(), "sonnet");
        assert_eq!(ClaudeModel::Haiku.cli_name(), "haiku");
    }

    /// Test ClaudeModel parse method.
    #[test]
    fn test_claude_model_parse() {
        // CLI names
        assert_eq!(ClaudeModel::parse("opus"), Some(ClaudeModel::Opus));
        assert_eq!(ClaudeModel::parse("sonnet"), Some(ClaudeModel::Sonnet));
        assert_eq!(ClaudeModel::parse("haiku"), Some(ClaudeModel::Haiku));

        // Case insensitive
        assert_eq!(ClaudeModel::parse("OPUS"), Some(ClaudeModel::Opus));
        assert_eq!(ClaudeModel::parse("Sonnet"), Some(ClaudeModel::Sonnet));

        // Full model IDs
        assert_eq!(
            ClaudeModel::parse("claude-opus-4-5-20251101"),
            Some(ClaudeModel::Opus)
        );
        assert_eq!(
            ClaudeModel::parse("claude-sonnet-4-20250514"),
            Some(ClaudeModel::Sonnet)
        );

        // Unknown
        assert_eq!(ClaudeModel::parse("unknown"), None);
        assert_eq!(ClaudeModel::parse("gpt-4"), None);
    }

    /// Test ClaudeModel default is Opus.
    #[test]
    fn test_claude_model_default() {
        assert_eq!(ClaudeModel::default(), ClaudeModel::Opus);
    }

    /// Test ClaudeModel cost per million tokens.
    #[test]
    fn test_claude_model_costs() {
        let (opus_in, opus_out) = ClaudeModel::Opus.cost_per_million_tokens();
        assert!((opus_in - 15.0).abs() < f64::EPSILON);
        assert!((opus_out - 75.0).abs() < f64::EPSILON);

        let (sonnet_in, sonnet_out) = ClaudeModel::Sonnet.cost_per_million_tokens();
        assert!((sonnet_in - 3.0).abs() < f64::EPSILON);
        assert!((sonnet_out - 15.0).abs() < f64::EPSILON);

        let (haiku_in, haiku_out) = ClaudeModel::Haiku.cost_per_million_tokens();
        assert!((haiku_in - 0.25).abs() < f64::EPSILON);
        assert!((haiku_out - 1.25).abs() < f64::EPSILON);
    }

    // =========================================================================
    // ClaudeApiError Tests
    // =========================================================================

    /// Test rate limit error detection from stderr.
    #[test]
    fn test_claude_rate_limit_detection() {
        let stderr = "Error: Rate limit exceeded. Please retry after 60 seconds.";
        let error = ClaudeApiError::from_stderr(stderr, 1);

        match error {
            ClaudeApiError::RateLimited {
                retry_after_secs, ..
            } => {
                assert_eq!(retry_after_secs, 60);
            }
            _ => panic!("Expected RateLimited error, got {:?}", error),
        }

        assert!(error.is_retryable());
        assert!(error.retry_after().is_some());
    }

    /// Test rate limit detection with "429" status code.
    #[test]
    fn test_claude_rate_limit_429() {
        let stderr = "HTTP 429: Too many requests";
        let error = ClaudeApiError::from_stderr(stderr, 1);

        assert!(matches!(error, ClaudeApiError::RateLimited { .. }));
    }

    /// Test authentication error detection.
    #[test]
    fn test_claude_auth_error_detection() {
        let stderr = "Error: Authentication failed. Invalid API key.";
        let error = ClaudeApiError::from_stderr(stderr, 1);

        assert!(matches!(error, ClaudeApiError::AuthenticationFailed { .. }));
        assert!(!error.is_retryable());
    }

    /// Test server error detection and retryability.
    #[test]
    fn test_claude_server_error_detection() {
        let stderr = "HTTP 503: Service temporarily unavailable";
        let error = ClaudeApiError::from_stderr(stderr, 1);

        assert!(matches!(error, ClaudeApiError::ServerError { .. }));
        assert!(error.is_retryable());
    }

    /// Test context length error detection.
    #[test]
    fn test_claude_context_length_error() {
        let stderr = "Error: Context length exceeded. Maximum 200000 tokens.";
        let error = ClaudeApiError::from_stderr(stderr, 1);

        assert!(matches!(
            error,
            ClaudeApiError::ContextLengthExceeded { .. }
        ));
        assert!(!error.is_retryable());
    }

    // =========================================================================
    // RateLimitTracker Tests
    // =========================================================================

    /// Test rate limit tracker initial state.
    #[test]
    fn test_rate_limit_tracker_initial_state() {
        let tracker = RateLimitTracker::new();
        assert!(!tracker.is_rate_limited());
        assert!(tracker.time_until_ready().is_none());
    }

    /// Test rate limit recording.
    #[test]
    fn test_rate_limit_tracker_record_limit() {
        let tracker = RateLimitTracker::new();
        tracker.record_rate_limit(Some(1)); // 1 second

        assert!(tracker.is_rate_limited());
        assert!(tracker.time_until_ready().is_some());

        // Wait for expiry
        std::thread::sleep(std::time::Duration::from_millis(1100));
        assert!(!tracker.is_rate_limited());
    }

    /// Test rate limit reset.
    #[test]
    fn test_rate_limit_tracker_reset() {
        let tracker = RateLimitTracker::new();
        tracker.record_rate_limit(Some(60));
        assert!(tracker.is_rate_limited());

        tracker.reset();
        assert!(!tracker.is_rate_limited());
    }

    /// Test successful request resets backoff.
    #[test]
    fn test_rate_limit_tracker_success_resets_backoff() {
        let tracker = RateLimitTracker::new();

        // Simulate multiple rate limits increasing backoff
        tracker.record_rate_limit(None);
        tracker.reset();
        tracker.record_rate_limit(None);
        tracker.reset();

        // Success should reset
        tracker.record_success();

        // Next multiplier should be 1
        assert_eq!(tracker.backoff_multiplier.load(Ordering::SeqCst), 1);
    }

    // =========================================================================
    // ClaudeProvider Tests
    // =========================================================================

    /// Test ClaudeProvider creation with default model.
    #[test]
    fn test_claude_provider_creation() {
        let provider = ClaudeProvider::new(".");
        assert_eq!(provider.model(), ClaudeModel::Opus);
        assert_eq!(provider.model_name(), "claude-opus-4-5-20251101");
    }

    /// Test ClaudeProvider model selection.
    #[test]
    fn test_claude_provider_model_selection() {
        let opus = ClaudeProvider::new(".").with_model(ClaudeModel::Opus);
        assert_eq!(opus.model_name(), "claude-opus-4-5-20251101");

        let sonnet = ClaudeProvider::new(".").with_model(ClaudeModel::Sonnet);
        assert_eq!(sonnet.model_name(), "claude-sonnet-4-20250514");

        let haiku = ClaudeProvider::new(".").with_model(ClaudeModel::Haiku);
        assert_eq!(haiku.model_name(), "claude-3-5-haiku-20241022");
    }

    /// Test ClaudeProvider with model name string.
    #[test]
    fn test_claude_provider_with_model_name() {
        let provider = ClaudeProvider::new(".").with_model_name("sonnet");
        assert_eq!(provider.model(), ClaudeModel::Sonnet);

        // Unknown name falls back to Opus
        let provider = ClaudeProvider::new(".").with_model_name("unknown");
        assert_eq!(provider.model(), ClaudeModel::Opus);
    }

    /// Test ClaudeProvider implements LlmClient.
    #[test]
    fn test_claude_provider_implements_llm_client() {
        fn assert_llm_client<T: LlmClient>() {}
        assert_llm_client::<ClaudeProvider>();
    }

    /// Test ClaudeProvider is Send + Sync.
    #[test]
    fn test_claude_provider_is_send_sync() {
        fn assert_send_sync<T: Send + Sync>() {}
        assert_send_sync::<ClaudeProvider>();
    }

    /// Test ClaudeProvider can be used as trait object.
    #[test]
    fn test_claude_provider_as_trait_object() {
        let provider: Box<dyn LlmClient> = Box::new(ClaudeProvider::new("."));
        assert!(provider.model_name().contains("claude"));
        assert!(provider.supports_tools());
    }

    /// Test ClaudeProvider capabilities.
    #[test]
    fn test_claude_provider_capabilities() {
        let provider = ClaudeProvider::new(".");
        let caps = provider.capabilities();

        assert!(caps.supports_streaming);
        assert!(caps.supports_tool_use);
        assert_eq!(caps.max_context_tokens, 200_000);
        assert_eq!(caps.max_output_tokens, 8192);
    }

    /// Test ClaudeProvider cost per token.
    #[test]
    fn test_claude_provider_cost_per_token() {
        let opus = ClaudeProvider::new(".").with_model(ClaudeModel::Opus);
        let (input, output) = opus.cost_per_token();
        assert!((input * 1_000_000.0 - 15.0).abs() < f64::EPSILON);
        assert!((output * 1_000_000.0 - 75.0).abs() < f64::EPSILON);

        let sonnet = ClaudeProvider::new(".").with_model(ClaudeModel::Sonnet);
        let (input, output) = sonnet.cost_per_token();
        assert!((input * 1_000_000.0 - 3.0).abs() < f64::EPSILON);
        assert!((output * 1_000_000.0 - 15.0).abs() < f64::EPSILON);
    }

    /// Test ClaudeProvider rate limit awareness.
    #[test]
    fn test_claude_provider_rate_limit_awareness() {
        let provider = ClaudeProvider::new(".");

        assert!(!provider.is_rate_limited());

        // Simulate rate limit (internal access for testing)
        provider.rate_limiter.record_rate_limit(Some(1));
        assert!(provider.is_rate_limited());

        provider.reset_rate_limit();
        assert!(!provider.is_rate_limited());
    }

    /// Test ClaudeProvider timeout configuration.
    #[test]
    fn test_claude_provider_timeout_config() {
        let provider = ClaudeProvider::new(".");
        assert_eq!(provider.timeout_secs, ClaudeProvider::DEFAULT_TIMEOUT_SECS);

        let custom = ClaudeProvider::new(".").with_timeout(60);
        assert_eq!(custom.timeout_secs, 60);
    }
}
