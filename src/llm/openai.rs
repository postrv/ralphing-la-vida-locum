//! OpenAI LLM provider implementation.
//!
//! This module provides a production-ready OpenAI client that implements the
//! [`LlmClient`] trait. It includes proper error handling, rate limit detection,
//! and support for GPT-4, GPT-4o, and o1 model variants.
//!
//! # Architecture
//!
//! The [`OpenAiProvider`] communicates with OpenAI's API via HTTP. It includes:
//!
//! - Structured error types for API errors
//! - Rate limit detection with configurable backoff
//! - Support for specific model IDs (gpt-4o, gpt-4-turbo, o1)
//! - API key management from environment variables
//!
//! # Example
//!
//! ```rust,ignore
//! use ralph::llm::{OpenAiProvider, OpenAiModel, LlmClient};
//!
//! // Create provider with GPT-4o model
//! let provider = OpenAiProvider::new(OpenAiModel::Gpt4o)
//!     .with_api_key_env("OPENAI_API_KEY");
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
use std::env;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::time::{Duration, Instant};
use thiserror::Error;
use tracing::{debug, warn};

// =============================================================================
// OpenAI Model Variants
// =============================================================================

/// Supported OpenAI model variants.
///
/// Each variant has different capabilities, pricing, and performance
/// characteristics. Use [`OpenAiModel::model_id`] to get the full model ID
/// for API calls.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
pub enum OpenAiModel {
    /// GPT-4o - most capable flagship model
    #[default]
    Gpt4o,
    /// GPT-4o Mini - fast, affordable small model
    Gpt4oMini,
    /// GPT-4 Turbo - previous generation flagship
    Gpt4Turbo,
    /// o1 - reasoning model for complex tasks
    O1,
    /// o1 Mini - smaller reasoning model
    O1Mini,
}

impl OpenAiModel {
    /// Get the full model ID for API calls.
    ///
    /// Returns the complete model identifier as used by OpenAI's API.
    ///
    /// # Example
    ///
    /// ```rust
    /// use ralph::llm::openai::OpenAiModel;
    ///
    /// assert_eq!(OpenAiModel::Gpt4o.model_id(), "gpt-4o");
    /// assert_eq!(OpenAiModel::O1.model_id(), "o1");
    /// ```
    #[must_use]
    pub const fn model_id(&self) -> &'static str {
        match self {
            Self::Gpt4o => "gpt-4o",
            Self::Gpt4oMini => "gpt-4o-mini",
            Self::Gpt4Turbo => "gpt-4-turbo",
            Self::O1 => "o1",
            Self::O1Mini => "o1-mini",
        }
    }

    /// Get the display name for this model.
    #[must_use]
    pub const fn display_name(&self) -> &'static str {
        match self {
            Self::Gpt4o => "GPT-4o",
            Self::Gpt4oMini => "GPT-4o Mini",
            Self::Gpt4Turbo => "GPT-4 Turbo",
            Self::O1 => "o1",
            Self::O1Mini => "o1 Mini",
        }
    }

    /// Get cost per million tokens (input, output) in USD.
    #[must_use]
    pub const fn cost_per_million_tokens(&self) -> (f64, f64) {
        match self {
            Self::Gpt4o => (2.50, 10.0),
            Self::Gpt4oMini => (0.15, 0.60),
            Self::Gpt4Turbo => (10.0, 30.0),
            Self::O1 => (15.0, 60.0),
            Self::O1Mini => (3.0, 12.0),
        }
    }

    /// Get the context window size for this model.
    #[must_use]
    pub const fn context_window(&self) -> u32 {
        match self {
            Self::Gpt4o | Self::Gpt4oMini => 128_000,
            Self::Gpt4Turbo => 128_000,
            Self::O1 | Self::O1Mini => 128_000,
        }
    }

    /// Get the maximum output tokens for this model.
    #[must_use]
    pub const fn max_output_tokens(&self) -> u32 {
        match self {
            Self::Gpt4o | Self::Gpt4oMini => 16_384,
            Self::Gpt4Turbo => 4_096,
            Self::O1 | Self::O1Mini => 32_768,
        }
    }

    /// Check if this model supports tool/function calling.
    #[must_use]
    pub const fn supports_tools(&self) -> bool {
        match self {
            Self::Gpt4o | Self::Gpt4oMini | Self::Gpt4Turbo => true,
            // o1 models have limited tool support
            Self::O1 | Self::O1Mini => false,
        }
    }

    /// Parse a model name string into an [`OpenAiModel`].
    ///
    /// Accepts model IDs and common variations.
    /// Returns `None` if the string is not recognized.
    ///
    /// # Example
    ///
    /// ```rust
    /// use ralph::llm::openai::OpenAiModel;
    ///
    /// assert_eq!(OpenAiModel::parse("gpt-4o"), Some(OpenAiModel::Gpt4o));
    /// assert_eq!(OpenAiModel::parse("o1"), Some(OpenAiModel::O1));
    /// assert_eq!(OpenAiModel::parse("unknown"), None);
    /// ```
    #[must_use]
    pub fn parse(s: &str) -> Option<Self> {
        let lower = s.to_lowercase();
        match lower.as_str() {
            "gpt-4o" | "gpt4o" | "4o" => Some(Self::Gpt4o),
            "gpt-4o-mini" | "gpt4o-mini" | "4o-mini" => Some(Self::Gpt4oMini),
            "gpt-4-turbo" | "gpt4-turbo" | "gpt-4" | "gpt4" => Some(Self::Gpt4Turbo),
            "o1" => Some(Self::O1),
            "o1-mini" => Some(Self::O1Mini),
            _ => None,
        }
    }
}

/// Error for parsing [`OpenAiModel`] from a string.
#[derive(Error, Debug, Clone, PartialEq, Eq)]
#[error(
    "Unknown OpenAI model: '{0}'. Valid options: gpt-4o, gpt-4o-mini, gpt-4-turbo, o1, o1-mini"
)]
pub struct ParseOpenAiModelError(String);

impl std::str::FromStr for OpenAiModel {
    type Err = ParseOpenAiModelError;

    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        Self::parse(s).ok_or_else(|| ParseOpenAiModelError(s.to_string()))
    }
}

impl std::fmt::Display for OpenAiModel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.display_name())
    }
}

// =============================================================================
// OpenAI API Errors
// =============================================================================

/// Errors specific to OpenAI API interactions.
///
/// These errors provide structured information about API failures,
/// enabling appropriate handling (e.g., retry with backoff for rate limits).
#[derive(Error, Debug)]
pub enum OpenAiApiError {
    /// Rate limit exceeded - should retry with backoff.
    #[error("Rate limit exceeded: {message} (retry after {retry_after_secs}s)")]
    RateLimited {
        message: String,
        retry_after_secs: u64,
    },

    /// Authentication failed - check API key.
    #[error("Authentication failed: {message}")]
    AuthenticationFailed { message: String },

    /// API key not found in environment.
    #[error("API key not found in environment variable '{env_var}'")]
    ApiKeyNotFound { env_var: String },

    /// Invalid request - check prompt/parameters.
    #[error("Invalid request: {message}")]
    InvalidRequest { message: String },

    /// Server error - may be transient.
    #[error("Server error: {message}")]
    ServerError { message: String },

    /// Network/connection error.
    #[error("Connection error: {message}")]
    ConnectionError { message: String },

    /// Timeout waiting for response.
    #[error("Request timed out after {timeout_secs}s")]
    Timeout { timeout_secs: u64 },

    /// Context length exceeded.
    #[error("Context length exceeded: {message}")]
    ContextLengthExceeded { message: String },

    /// Invalid response from API.
    #[error("Invalid API response: {message}")]
    InvalidResponse { message: String },
}

impl OpenAiApiError {
    /// Check if this error indicates the request should be retried.
    #[must_use]
    pub fn is_retryable(&self) -> bool {
        matches!(
            self,
            Self::RateLimited { .. }
                | Self::ServerError { .. }
                | Self::Timeout { .. }
                | Self::ConnectionError { .. }
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
            Self::ConnectionError { .. } => Some(Duration::from_secs(2)),
            _ => None,
        }
    }

    /// Parse error from HTTP status code and response body.
    pub fn from_response(status_code: u16, body: &str) -> Self {
        let body_lower = body.to_lowercase();

        match status_code {
            429 => {
                // Rate limited
                let retry_after = Self::extract_retry_after(body).unwrap_or(60);
                Self::RateLimited {
                    message: body.to_string(),
                    retry_after_secs: retry_after,
                }
            }
            401 => Self::AuthenticationFailed {
                message: body.to_string(),
            },
            400 => {
                if body_lower.contains("context_length") || body_lower.contains("maximum") {
                    Self::ContextLengthExceeded {
                        message: body.to_string(),
                    }
                } else {
                    Self::InvalidRequest {
                        message: body.to_string(),
                    }
                }
            }
            500..=599 => Self::ServerError {
                message: body.to_string(),
            },
            _ => Self::InvalidResponse {
                message: format!("HTTP {}: {}", status_code, body),
            },
        }
    }

    /// Extract retry-after seconds from error response.
    fn extract_retry_after(body: &str) -> Option<u64> {
        // Try to find patterns like "retry after 60s" or "retry-after: 60"
        let patterns = [
            r"retry.?after[:\s]+(\d+)",
            r"wait[:\s]+(\d+)",
            r"(\d+)\s*seconds?",
        ];

        for pattern in patterns {
            if let Ok(re) = regex::Regex::new(pattern) {
                if let Some(caps) = re.captures(&body.to_lowercase()) {
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

/// Tracks rate limit state and implements exponential backoff for OpenAI.
#[derive(Debug)]
pub struct OpenAiRateLimitTracker {
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

impl Default for OpenAiRateLimitTracker {
    fn default() -> Self {
        Self::new()
    }
}

impl OpenAiRateLimitTracker {
    /// Create a new rate limit tracker with default settings.
    #[must_use]
    pub fn new() -> Self {
        Self {
            is_limited: AtomicBool::new(false),
            limit_expires_at: AtomicU64::new(0),
            backoff_multiplier: AtomicU64::new(1),
            base_backoff_secs: 5,
            max_backoff_secs: 300,
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
            "OpenAI rate limited for {}s (backoff multiplier: {})",
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
// OpenAI API Request/Response Types
// =============================================================================

/// Message in an OpenAI chat completion request.
#[derive(Debug, Clone, Serialize, Deserialize)]
struct ChatMessage {
    role: String,
    content: String,
}

/// Request body for OpenAI chat completions API.
#[derive(Debug, Serialize)]
struct ChatCompletionRequest {
    model: String,
    messages: Vec<ChatMessage>,
    #[serde(skip_serializing_if = "Option::is_none")]
    max_tokens: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    temperature: Option<f32>,
}

/// Usage information from API response.
#[derive(Debug, Deserialize)]
struct Usage {
    prompt_tokens: u32,
    completion_tokens: u32,
    // total_tokens is part of the API response but we calculate from prompt + completion
    #[serde(default)]
    _total_tokens: u32,
}

/// Choice in API response.
#[derive(Debug, Deserialize)]
struct Choice {
    message: ChatMessage,
    // finish_reason is part of the API response but not used
    #[serde(default)]
    _finish_reason: Option<String>,
}

/// Response from OpenAI chat completions API.
#[derive(Debug, Deserialize)]
struct ChatCompletionResponse {
    choices: Vec<Choice>,
    usage: Option<Usage>,
    // model is part of the API response but not used
    #[serde(default)]
    _model: String,
}

// =============================================================================
// OpenAI Provider
// =============================================================================

/// Production-ready OpenAI LLM provider.
///
/// Implements the [`LlmClient`] trait with:
/// - HTTP API calls to OpenAI's chat completions endpoint
/// - Proper error handling and structured error types
/// - Rate limit detection with exponential backoff
/// - Support for GPT-4, GPT-4o, and o1 model variants
///
/// # Example
///
/// ```rust,ignore
/// use ralph::llm::{OpenAiProvider, OpenAiModel, LlmClient};
///
/// let provider = OpenAiProvider::new(OpenAiModel::Gpt4o);
///
/// // Provider automatically handles rate limits
/// let response = provider.run_prompt("Hello!").await?;
/// ```
#[derive(Debug)]
pub struct OpenAiProvider {
    /// Model variant to use.
    model: OpenAiModel,
    /// Environment variable name for API key.
    api_key_env: String,
    /// Rate limit tracker.
    rate_limiter: OpenAiRateLimitTracker,
    /// Request timeout in seconds.
    timeout_secs: u64,
    /// API base URL.
    api_base: String,
}

impl OpenAiProvider {
    /// Default timeout for requests (2 minutes).
    pub const DEFAULT_TIMEOUT_SECS: u64 = 120;

    /// Default API base URL.
    pub const DEFAULT_API_BASE: &'static str = "https://api.openai.com/v1";

    /// Default API key environment variable.
    pub const DEFAULT_API_KEY_ENV: &'static str = "OPENAI_API_KEY";

    /// Create a new OpenAI provider with the specified model.
    ///
    /// # Arguments
    ///
    /// * `model` - The model to use
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// use ralph::llm::{OpenAiProvider, OpenAiModel};
    ///
    /// let provider = OpenAiProvider::new(OpenAiModel::Gpt4o);
    /// ```
    #[must_use]
    pub fn new(model: OpenAiModel) -> Self {
        Self {
            model,
            api_key_env: Self::DEFAULT_API_KEY_ENV.to_string(),
            rate_limiter: OpenAiRateLimitTracker::new(),
            timeout_secs: Self::DEFAULT_TIMEOUT_SECS,
            api_base: Self::DEFAULT_API_BASE.to_string(),
        }
    }

    /// Set the model variant to use.
    #[must_use]
    pub fn with_model(mut self, model: OpenAiModel) -> Self {
        self.model = model;
        self
    }

    /// Set the model by name string.
    ///
    /// Falls back to GPT-4o if the name is not recognized.
    #[must_use]
    pub fn with_model_name(mut self, name: &str) -> Self {
        self.model = OpenAiModel::parse(name).unwrap_or_default();
        self
    }

    /// Set the environment variable name for the API key.
    #[must_use]
    pub fn with_api_key_env(mut self, env_var: &str) -> Self {
        self.api_key_env = env_var.to_string();
        self
    }

    /// Set the request timeout.
    #[must_use]
    pub fn with_timeout(mut self, timeout_secs: u64) -> Self {
        self.timeout_secs = timeout_secs;
        self
    }

    /// Set a custom API base URL (for Azure OpenAI or proxies).
    #[must_use]
    pub fn with_api_base(mut self, api_base: &str) -> Self {
        self.api_base = api_base.to_string();
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
    pub fn model(&self) -> OpenAiModel {
        self.model
    }

    /// Get the API key from the environment.
    fn get_api_key(&self) -> Result<String, OpenAiApiError> {
        env::var(&self.api_key_env).map_err(|_| OpenAiApiError::ApiKeyNotFound {
            env_var: self.api_key_env.clone(),
        })
    }

    /// Reset rate limit state (for testing).
    pub fn reset_rate_limit(&self) {
        self.rate_limiter.reset();
    }

    /// Execute an API request and return the result.
    async fn execute_request(&self, prompt: &str) -> Result<(String, u32, u32), OpenAiApiError> {
        // Check rate limit before proceeding
        if self.is_rate_limited() {
            let remaining = self.rate_limit_remaining().unwrap_or_default();
            return Err(OpenAiApiError::RateLimited {
                message: "Rate limit in effect".to_string(),
                retry_after_secs: remaining.as_secs(),
            });
        }

        let api_key = self.get_api_key()?;
        let url = format!("{}/chat/completions", self.api_base);

        let request_body = ChatCompletionRequest {
            model: self.model.model_id().to_string(),
            messages: vec![ChatMessage {
                role: "user".to_string(),
                content: prompt.to_string(),
            }],
            max_tokens: Some(self.model.max_output_tokens()),
            temperature: Some(0.7),
        };

        debug!(
            "Sending request to OpenAI {} ({} chars prompt)",
            self.model.display_name(),
            prompt.len()
        );

        // Use tokio's TCP client for HTTP request
        // Since we don't have reqwest, we'll use a simple HTTP implementation
        let response = self
            .make_http_request(&url, &api_key, &request_body)
            .await?;

        self.rate_limiter.record_success();

        let content = response
            .choices
            .first()
            .map(|c| c.message.content.clone())
            .unwrap_or_default();

        let (input_tokens, output_tokens) = response
            .usage
            .map(|u| (u.prompt_tokens, u.completion_tokens))
            .unwrap_or(((prompt.len() / 4) as u32, (content.len() / 4) as u32));

        Ok((content, input_tokens, output_tokens))
    }

    /// Make HTTP request to OpenAI API.
    ///
    /// Uses curl as a subprocess since we don't have reqwest as a dependency.
    async fn make_http_request(
        &self,
        url: &str,
        api_key: &str,
        body: &ChatCompletionRequest,
    ) -> Result<ChatCompletionResponse, OpenAiApiError> {
        let body_json =
            serde_json::to_string(body).map_err(|e| OpenAiApiError::InvalidRequest {
                message: format!("Failed to serialize request: {}", e),
            })?;

        // Use curl as a subprocess for HTTPS requests
        let output = tokio::process::Command::new("curl")
            .args([
                "-s",
                "-X",
                "POST",
                url,
                "-H",
                &format!("Authorization: Bearer {}", api_key),
                "-H",
                "Content-Type: application/json",
                "-d",
                &body_json,
                "--max-time",
                &self.timeout_secs.to_string(),
            ])
            .output()
            .await
            .map_err(|e| OpenAiApiError::ConnectionError {
                message: format!("Failed to execute curl: {}", e),
            })?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            if stderr.contains("timed out") || stderr.contains("timeout") {
                return Err(OpenAiApiError::Timeout {
                    timeout_secs: self.timeout_secs,
                });
            }
            return Err(OpenAiApiError::ConnectionError {
                message: format!("curl failed: {}", stderr),
            });
        }

        let response_body = String::from_utf8_lossy(&output.stdout);

        // Check for error response
        if let Ok(error_response) = serde_json::from_str::<serde_json::Value>(&response_body) {
            if let Some(error) = error_response.get("error") {
                let message = error
                    .get("message")
                    .and_then(|m| m.as_str())
                    .unwrap_or("Unknown error");
                let error_type = error.get("type").and_then(|t| t.as_str()).unwrap_or("");

                if error_type.contains("rate_limit") || message.contains("rate limit") {
                    let retry_after = OpenAiApiError::extract_retry_after(message).unwrap_or(60);
                    self.rate_limiter.record_rate_limit(Some(retry_after));
                    return Err(OpenAiApiError::RateLimited {
                        message: message.to_string(),
                        retry_after_secs: retry_after,
                    });
                }
                if error_type.contains("authentication") || message.contains("API key") {
                    return Err(OpenAiApiError::AuthenticationFailed {
                        message: message.to_string(),
                    });
                }
                return Err(OpenAiApiError::InvalidRequest {
                    message: message.to_string(),
                });
            }
        }

        serde_json::from_str(&response_body).map_err(|e| OpenAiApiError::InvalidResponse {
            message: format!("Failed to parse response: {} - Body: {}", e, response_body),
        })
    }
}

#[async_trait]
impl LlmClient for OpenAiProvider {
    async fn run_prompt(&self, prompt: &str) -> Result<String> {
        let (content, _, _) = self
            .execute_request(prompt)
            .await
            .map_err(|e| anyhow::anyhow!("{}", e))?;
        Ok(content)
    }

    async fn complete(&self, request: CompletionRequest) -> Result<LlmResponse> {
        let start = Instant::now();

        let (content, input_tokens, output_tokens) = self.execute_request(&request.prompt).await?;

        let latency_ms = start.elapsed().as_millis() as u64;

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
        // Check if API key is available
        self.get_api_key().is_ok()
    }

    fn model_name(&self) -> &str {
        self.model.model_id()
    }

    fn supports_tools(&self) -> bool {
        self.model.supports_tools()
    }

    fn capabilities(&self) -> ProviderCapabilities {
        ProviderCapabilities::default()
            .with_streaming(true)
            .with_tool_use(self.model.supports_tools())
            .with_max_context(self.model.context_window())
            .with_max_output(self.model.max_output_tokens())
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
// Tests (TDD - Phase 23.4)
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    // =========================================================================
    // OpenAiModel Tests
    // =========================================================================

    /// Test OpenAiModel provides correct model IDs.
    #[test]
    fn test_openai_model_ids() {
        assert_eq!(OpenAiModel::Gpt4o.model_id(), "gpt-4o");
        assert_eq!(OpenAiModel::Gpt4oMini.model_id(), "gpt-4o-mini");
        assert_eq!(OpenAiModel::Gpt4Turbo.model_id(), "gpt-4-turbo");
        assert_eq!(OpenAiModel::O1.model_id(), "o1");
        assert_eq!(OpenAiModel::O1Mini.model_id(), "o1-mini");
    }

    /// Test OpenAiModel parse method.
    #[test]
    fn test_openai_model_parse() {
        // Standard names
        assert_eq!(OpenAiModel::parse("gpt-4o"), Some(OpenAiModel::Gpt4o));
        assert_eq!(
            OpenAiModel::parse("gpt-4o-mini"),
            Some(OpenAiModel::Gpt4oMini)
        );
        assert_eq!(
            OpenAiModel::parse("gpt-4-turbo"),
            Some(OpenAiModel::Gpt4Turbo)
        );
        assert_eq!(OpenAiModel::parse("o1"), Some(OpenAiModel::O1));
        assert_eq!(OpenAiModel::parse("o1-mini"), Some(OpenAiModel::O1Mini));

        // Case insensitive
        assert_eq!(OpenAiModel::parse("GPT-4O"), Some(OpenAiModel::Gpt4o));

        // Shorthand
        assert_eq!(OpenAiModel::parse("4o"), Some(OpenAiModel::Gpt4o));

        // Unknown
        assert_eq!(OpenAiModel::parse("unknown"), None);
        assert_eq!(OpenAiModel::parse("claude"), None);
    }

    /// Test OpenAiModel default is Gpt4o.
    #[test]
    fn test_openai_model_default() {
        assert_eq!(OpenAiModel::default(), OpenAiModel::Gpt4o);
    }

    /// Test OpenAiModel cost per million tokens.
    #[test]
    fn test_openai_model_costs() {
        let (gpt4o_in, gpt4o_out) = OpenAiModel::Gpt4o.cost_per_million_tokens();
        assert!((gpt4o_in - 2.50).abs() < f64::EPSILON);
        assert!((gpt4o_out - 10.0).abs() < f64::EPSILON);

        let (mini_in, mini_out) = OpenAiModel::Gpt4oMini.cost_per_million_tokens();
        assert!((mini_in - 0.15).abs() < f64::EPSILON);
        assert!((mini_out - 0.60).abs() < f64::EPSILON);

        let (o1_in, o1_out) = OpenAiModel::O1.cost_per_million_tokens();
        assert!((o1_in - 15.0).abs() < f64::EPSILON);
        assert!((o1_out - 60.0).abs() < f64::EPSILON);
    }

    /// Test OpenAiModel tool support.
    #[test]
    fn test_openai_model_tool_support() {
        assert!(OpenAiModel::Gpt4o.supports_tools());
        assert!(OpenAiModel::Gpt4oMini.supports_tools());
        assert!(OpenAiModel::Gpt4Turbo.supports_tools());
        // o1 models have limited tool support
        assert!(!OpenAiModel::O1.supports_tools());
        assert!(!OpenAiModel::O1Mini.supports_tools());
    }

    // =========================================================================
    // OpenAiApiError Tests
    // =========================================================================

    /// Test rate limit error detection from response.
    #[test]
    fn test_openai_rate_limit_detection() {
        let body = "Rate limit exceeded. Please retry after 60 seconds.";
        let error = OpenAiApiError::from_response(429, body);

        match error {
            OpenAiApiError::RateLimited {
                retry_after_secs, ..
            } => {
                assert_eq!(retry_after_secs, 60);
            }
            _ => panic!("Expected RateLimited error, got {:?}", error),
        }

        assert!(error.is_retryable());
        assert!(error.retry_after().is_some());
    }

    /// Test authentication error detection.
    #[test]
    fn test_openai_auth_error_detection() {
        let body = "Invalid API key provided";
        let error = OpenAiApiError::from_response(401, body);

        assert!(matches!(error, OpenAiApiError::AuthenticationFailed { .. }));
        assert!(!error.is_retryable());
    }

    /// Test server error detection and retryability.
    #[test]
    fn test_openai_server_error_detection() {
        let body = "Internal server error";
        let error = OpenAiApiError::from_response(500, body);

        assert!(matches!(error, OpenAiApiError::ServerError { .. }));
        assert!(error.is_retryable());
    }

    /// Test context length error detection.
    #[test]
    fn test_openai_context_length_error() {
        let body = "This model's maximum context length is 128000 tokens";
        let error = OpenAiApiError::from_response(400, body);

        assert!(matches!(
            error,
            OpenAiApiError::ContextLengthExceeded { .. }
        ));
        assert!(!error.is_retryable());
    }

    // =========================================================================
    // Rate Limit Tracker Tests
    // =========================================================================

    /// Test rate limit tracker initial state.
    #[test]
    fn test_openai_rate_limit_tracker_initial_state() {
        let tracker = OpenAiRateLimitTracker::new();
        assert!(!tracker.is_rate_limited());
        assert!(tracker.time_until_ready().is_none());
    }

    /// Test rate limit recording.
    #[test]
    fn test_openai_rate_limit_tracker_record_limit() {
        let tracker = OpenAiRateLimitTracker::new();
        tracker.record_rate_limit(Some(1)); // 1 second

        assert!(tracker.is_rate_limited());
        assert!(tracker.time_until_ready().is_some());

        // Wait for expiry
        std::thread::sleep(std::time::Duration::from_millis(1100));
        assert!(!tracker.is_rate_limited());
    }

    /// Test rate limit reset.
    #[test]
    fn test_openai_rate_limit_tracker_reset() {
        let tracker = OpenAiRateLimitTracker::new();
        tracker.record_rate_limit(Some(60));
        assert!(tracker.is_rate_limited());

        tracker.reset();
        assert!(!tracker.is_rate_limited());
    }

    // =========================================================================
    // OpenAiProvider Tests (Required by Implementation Plan)
    // =========================================================================

    /// Test that OpenAiProvider implements LlmClient trait.
    #[test]
    fn test_openai_provider_implements_llm_client() {
        fn assert_llm_client<T: LlmClient>() {}
        assert_llm_client::<OpenAiProvider>();
    }

    /// Test that OpenAiProvider is Send + Sync.
    #[test]
    fn test_openai_provider_is_send_sync() {
        fn assert_send_sync<T: Send + Sync>() {}
        assert_send_sync::<OpenAiProvider>();
    }

    /// Test that OpenAiProvider can be used as a trait object.
    #[test]
    fn test_openai_provider_as_trait_object() {
        let provider: Box<dyn LlmClient> = Box::new(OpenAiProvider::new(OpenAiModel::Gpt4o));
        assert_eq!(provider.model_name(), "gpt-4o");
    }

    /// Test OpenAiProvider creation with default settings.
    #[test]
    fn test_openai_provider_creation() {
        let provider = OpenAiProvider::new(OpenAiModel::Gpt4o);
        assert_eq!(provider.model(), OpenAiModel::Gpt4o);
        assert_eq!(provider.model_name(), "gpt-4o");
    }

    /// Test OpenAiProvider model selection.
    #[test]
    fn test_openai_provider_model_selection() {
        let gpt4o = OpenAiProvider::new(OpenAiModel::Gpt4o);
        assert_eq!(gpt4o.model_name(), "gpt-4o");

        let mini = OpenAiProvider::new(OpenAiModel::Gpt4oMini);
        assert_eq!(mini.model_name(), "gpt-4o-mini");

        let o1 = OpenAiProvider::new(OpenAiModel::O1);
        assert_eq!(o1.model_name(), "o1");
    }

    /// Test OpenAiProvider with model name string.
    #[test]
    fn test_openai_provider_with_model_name() {
        let provider = OpenAiProvider::new(OpenAiModel::Gpt4o).with_model_name("o1");
        assert_eq!(provider.model(), OpenAiModel::O1);

        // Unknown name falls back to Gpt4o
        let provider = OpenAiProvider::new(OpenAiModel::Gpt4o).with_model_name("unknown");
        assert_eq!(provider.model(), OpenAiModel::Gpt4o);
    }

    /// Test OpenAiProvider API key environment variable configuration.
    #[test]
    fn test_openai_provider_api_key_from_env() {
        let provider = OpenAiProvider::new(OpenAiModel::Gpt4o);
        assert_eq!(provider.api_key_env, "OPENAI_API_KEY");

        let custom = OpenAiProvider::new(OpenAiModel::Gpt4o).with_api_key_env("MY_OPENAI_KEY");
        assert_eq!(custom.api_key_env, "MY_OPENAI_KEY");
    }

    /// Test OpenAiProvider capabilities.
    #[test]
    fn test_openai_provider_capabilities() {
        let gpt4o = OpenAiProvider::new(OpenAiModel::Gpt4o);
        let caps = gpt4o.capabilities();

        assert!(caps.supports_streaming);
        assert!(caps.supports_tool_use);
        assert_eq!(caps.max_context_tokens, 128_000);
        assert_eq!(caps.max_output_tokens, 16_384);

        // o1 doesn't support tools
        let o1 = OpenAiProvider::new(OpenAiModel::O1);
        let caps = o1.capabilities();
        assert!(!caps.supports_tool_use);
    }

    /// Test OpenAiProvider cost per token.
    #[test]
    fn test_openai_provider_cost_per_token() {
        let gpt4o = OpenAiProvider::new(OpenAiModel::Gpt4o);
        let (input, output) = gpt4o.cost_per_token();
        assert!((input * 1_000_000.0 - 2.50).abs() < f64::EPSILON);
        assert!((output * 1_000_000.0 - 10.0).abs() < f64::EPSILON);
    }

    /// Test OpenAiProvider rate limit awareness.
    #[test]
    fn test_openai_provider_rate_limit_awareness() {
        let provider = OpenAiProvider::new(OpenAiModel::Gpt4o);

        assert!(!provider.is_rate_limited());

        // Simulate rate limit (internal access for testing)
        provider.rate_limiter.record_rate_limit(Some(1));
        assert!(provider.is_rate_limited());

        provider.reset_rate_limit();
        assert!(!provider.is_rate_limited());
    }

    /// Test OpenAiProvider timeout configuration.
    #[test]
    fn test_openai_provider_timeout_config() {
        let provider = OpenAiProvider::new(OpenAiModel::Gpt4o);
        assert_eq!(provider.timeout_secs, OpenAiProvider::DEFAULT_TIMEOUT_SECS);

        let custom = OpenAiProvider::new(OpenAiModel::Gpt4o).with_timeout(60);
        assert_eq!(custom.timeout_secs, 60);
    }

    /// Test OpenAiProvider custom API base.
    #[test]
    fn test_openai_provider_custom_api_base() {
        let provider =
            OpenAiProvider::new(OpenAiModel::Gpt4o).with_api_base("https://my-proxy.example.com");
        assert_eq!(provider.api_base, "https://my-proxy.example.com");
    }

    /// Test OpenAiProvider available() returns false when API key not set.
    #[tokio::test]
    async fn test_openai_provider_available_without_api_key() {
        // Create provider with a non-existent env var
        let provider =
            OpenAiProvider::new(OpenAiModel::Gpt4o).with_api_key_env("NONEXISTENT_API_KEY_XYZ");

        // Should return false because API key is not available
        let available = provider.available().await;
        assert!(!available);
    }
}
