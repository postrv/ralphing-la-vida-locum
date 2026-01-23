//! LLM client abstraction layer for multi-model support.
//!
//! This module provides a trait-based abstraction for LLM clients, enabling
//! Ralph to support multiple LLM backends (Claude, OpenAI, Gemini, etc.)
//! through a unified interface.
//!
//! # Architecture
//!
//! The [`LlmClient`] trait defines the core interface that all LLM clients must
//! implement. It is designed to be:
//!
//! - **Object-safe**: Supports dynamic dispatch via `Box<dyn LlmClient>`
//! - **Thread-safe**: `Send + Sync` bounds enable concurrent usage
//! - **Async-first**: Core operations are async for non-blocking I/O
//!
//! # Example
//!
//! ```rust,ignore
//! use ralph::llm::{LlmClient, ClaudeClient, MockLlmClient};
//!
//! // Use trait object for dynamic dispatch
//! let client: Box<dyn LlmClient> = Box::new(ClaudeClient::new("."));
//!
//! // Run a prompt
//! let response = client.run_prompt("Hello, world!").await?;
//!
//! // Check model capabilities
//! if client.supports_tools() {
//!     println!("Model supports tool use");
//! }
//! ```

use anyhow::Result;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU32, Ordering};
use tokio::io::AsyncWriteExt;
use tokio::process::Command as AsyncCommand;
use tracing::debug;

/// Abstraction for LLM client operations.
///
/// This trait defines the core interface for interacting with large language
/// models. Implementations can wrap specific APIs (Claude, OpenAI, etc.) while
/// providing a unified interface for the automation loop.
///
/// # Object Safety
///
/// This trait is object-safe and can be used with `Box<dyn LlmClient>` for
/// dynamic dispatch. This enables runtime model selection without generic
/// type parameters.
///
/// # Thread Safety
///
/// All implementations must be `Send + Sync` to enable concurrent usage
/// in async contexts.
///
/// # Example
///
/// ```rust,ignore
/// use ralph::llm::LlmClient;
///
/// async fn run_with_model(client: &dyn LlmClient, prompt: &str) -> Result<String> {
///     println!("Using model: {}", client.model_name());
///     client.run_prompt(prompt).await
/// }
/// ```
#[async_trait]
pub trait LlmClient: Send + Sync {
    /// Run a prompt and return the model's response.
    ///
    /// This is the core method for interacting with the LLM. The prompt
    /// is sent to the model and the response text is returned.
    ///
    /// # Arguments
    ///
    /// * `prompt` - The prompt text to send to the model
    ///
    /// # Returns
    ///
    /// The model's response as a string.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The model API is unreachable
    /// - Authentication fails
    /// - The request times out
    /// - The response cannot be parsed
    async fn run_prompt(&self, prompt: &str) -> Result<String>;

    /// Get the name of the model being used.
    ///
    /// Returns a human-readable model identifier (e.g., "claude-opus-4",
    /// "gpt-4", "gemini-pro").
    fn model_name(&self) -> &str;

    /// Check if the model supports tool/function calling.
    ///
    /// Tool support enables structured interactions where the model can
    /// request specific actions (file edits, command execution, etc.).
    fn supports_tools(&self) -> bool;
}

/// Claude Code client implementation.
///
/// Wraps the `claude` CLI tool to provide LLM capabilities through the
/// Claude API. This is the primary implementation used by Ralph.
///
/// # Example
///
/// ```rust,ignore
/// use ralph::llm::ClaudeClient;
///
/// let client = ClaudeClient::new(".");
/// let response = client.run_prompt("Hello!").await?;
/// ```
#[derive(Debug, Clone)]
pub struct ClaudeClient {
    /// Working directory for Claude CLI execution.
    project_dir: PathBuf,
    /// Model variant to use.
    model: String,
}

impl ClaudeClient {
    /// Create a new Claude client for the given project directory.
    ///
    /// # Arguments
    ///
    /// * `project_dir` - The directory where Claude should operate
    #[must_use]
    pub fn new<P: Into<PathBuf>>(project_dir: P) -> Self {
        Self {
            project_dir: project_dir.into(),
            model: "opus".to_string(),
        }
    }

    /// Set the model variant to use.
    ///
    /// # Arguments
    ///
    /// * `model` - Model name (e.g., "opus", "sonnet")
    #[must_use]
    pub fn with_model(mut self, model: &str) -> Self {
        self.model = model.to_string();
        self
    }
}

#[async_trait]
impl LlmClient for ClaudeClient {
    async fn run_prompt(&self, prompt: &str) -> Result<String> {
        let args = vec![
            "-p",
            "--dangerously-skip-permissions",
            "--model",
            &self.model,
            "--output-format",
            "text",
        ];

        debug!(
            "Running Claude with model {} ({} chars prompt)",
            self.model,
            prompt.len()
        );

        let mut child = AsyncCommand::new("claude")
            .args(&args)
            .current_dir(&self.project_dir)
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::inherit())
            .spawn()?;

        if let Some(mut stdin) = child.stdin.take() {
            stdin.write_all(prompt.as_bytes()).await?;
            stdin.flush().await?;
            drop(stdin);
        }

        let output = child.wait_with_output().await?;

        if output.status.success() {
            Ok(String::from_utf8_lossy(&output.stdout).to_string())
        } else {
            anyhow::bail!(
                "Claude process exited with code {}",
                output.status.code().unwrap_or(-1)
            )
        }
    }

    fn model_name(&self) -> &str {
        match self.model.as_str() {
            "opus" => "claude-opus-4",
            "sonnet" => "claude-sonnet-4",
            "haiku" => "claude-haiku-3.5",
            other => other,
        }
    }

    fn supports_tools(&self) -> bool {
        // Claude Code supports tool use
        true
    }
}

/// Mock LLM client for testing.
///
/// Provides controllable behavior for unit tests without making actual
/// API calls. Thread-safe for use in async contexts.
///
/// # Example
///
/// ```rust,ignore
/// use ralph::llm::MockLlmClient;
///
/// let client = MockLlmClient::new()
///     .with_response("Test response")
///     .with_model_name("mock-model");
///
/// assert_eq!(client.run_prompt("test").await.unwrap(), "Test response");
/// ```
#[derive(Debug)]
pub struct MockLlmClient {
    /// Response to return from `run_prompt`.
    response: String,
    /// Error to return (if set).
    error: Option<String>,
    /// Model name to return.
    model: String,
    /// Whether tools are supported.
    tools_supported: bool,
    /// Count of prompt calls.
    call_count: AtomicU32,
    /// Number of calls to fail before succeeding.
    fail_count: AtomicU32,
    /// Error message for fail_count failures.
    fail_error: Option<String>,
}

impl Clone for MockLlmClient {
    fn clone(&self) -> Self {
        Self {
            response: self.response.clone(),
            error: self.error.clone(),
            model: self.model.clone(),
            tools_supported: self.tools_supported,
            call_count: AtomicU32::new(self.call_count.load(Ordering::SeqCst)),
            fail_count: AtomicU32::new(self.fail_count.load(Ordering::SeqCst)),
            fail_error: self.fail_error.clone(),
        }
    }
}

impl Default for MockLlmClient {
    fn default() -> Self {
        Self {
            response: String::new(),
            error: None,
            model: "mock-llm".to_string(),
            tools_supported: false,
            call_count: AtomicU32::new(0),
            fail_count: AtomicU32::new(0),
            fail_error: None,
        }
    }
}

impl MockLlmClient {
    /// Create a new mock client with default values.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Set the response to return.
    #[must_use]
    pub fn with_response(mut self, response: &str) -> Self {
        self.response = response.to_string();
        self
    }

    /// Configure the mock to return an error.
    #[must_use]
    pub fn with_error(mut self, error: &str) -> Self {
        self.error = Some(error.to_string());
        self
    }

    /// Set the model name.
    #[must_use]
    pub fn with_model_name(mut self, name: &str) -> Self {
        self.model = name.to_string();
        self
    }

    /// Set whether tools are supported.
    #[must_use]
    pub fn with_tools_support(mut self, supported: bool) -> Self {
        self.tools_supported = supported;
        self
    }

    /// Configure the mock to fail the first N calls, then succeed.
    #[must_use]
    pub fn with_fail_count(mut self, count: u32, error: &str) -> Self {
        self.fail_count = AtomicU32::new(count);
        self.fail_error = Some(error.to_string());
        self
    }

    /// Get the number of times `run_prompt` was called.
    pub fn call_count(&self) -> u32 {
        self.call_count.load(Ordering::SeqCst)
    }
}

#[async_trait]
impl LlmClient for MockLlmClient {
    async fn run_prompt(&self, _prompt: &str) -> Result<String> {
        self.call_count.fetch_add(1, Ordering::SeqCst);

        // Check fail_count first
        let current_fail_count = self.fail_count.load(Ordering::SeqCst);
        if current_fail_count > 0 {
            self.fail_count.fetch_sub(1, Ordering::SeqCst);
            if let Some(ref fail_error) = self.fail_error {
                anyhow::bail!("{}", fail_error)
            } else {
                anyhow::bail!("Mock failure")
            }
        }

        // Check permanent error
        if let Some(ref error) = self.error {
            anyhow::bail!("{}", error)
        }

        Ok(self.response.clone())
    }

    fn model_name(&self) -> &str {
        &self.model
    }

    fn supports_tools(&self) -> bool {
        self.tools_supported
    }
}

// =============================================================================
// LLM Configuration (Phase 12.2)
// =============================================================================

/// Configuration for LLM backend selection and options.
///
/// This configuration is typically loaded from the `llm` section of
/// `.claude/settings.json` and can be overridden via CLI flags.
///
/// # Example settings.json
///
/// ```json
/// {
///   "llm": {
///     "model": "claude",
///     "api_key_env": "ANTHROPIC_API_KEY",
///     "options": {
///       "variant": "opus"
///     }
///   }
/// }
/// ```
///
/// # Supported Models
///
/// - `claude`: Claude models via Anthropic API (default)
/// - `openai`: OpenAI models (coming soon)
/// - `gemini`: Google Gemini models (coming soon)
/// - `ollama`: Local models via Ollama (coming soon)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LlmConfig {
    /// The LLM backend to use.
    ///
    /// Valid values: "claude", "openai", "gemini", "ollama".
    /// Default: "claude".
    #[serde(default = "default_model")]
    pub model: String,

    /// Environment variable name containing the API key.
    ///
    /// Default: "ANTHROPIC_API_KEY" for Claude.
    #[serde(default = "default_api_key_env")]
    pub api_key_env: String,

    /// Model-specific options.
    ///
    /// For Claude:
    /// - `variant`: Model variant ("opus", "sonnet", "haiku"). Default: "opus".
    ///
    /// For OpenAI:
    /// - `variant`: Model variant ("gpt-4", "gpt-4o", "o1"). Default: "gpt-4o".
    ///
    /// For Gemini:
    /// - `variant`: Model variant ("pro", "flash"). Default: "pro".
    ///
    /// For Ollama:
    /// - `model_name`: Local model name. Required.
    /// - `host`: Ollama host URL. Default: "http://localhost:11434".
    #[serde(default)]
    pub options: HashMap<String, serde_json::Value>,
}

fn default_model() -> String {
    "claude".to_string()
}

fn default_api_key_env() -> String {
    "ANTHROPIC_API_KEY".to_string()
}

impl Default for LlmConfig {
    fn default() -> Self {
        Self {
            model: default_model(),
            api_key_env: default_api_key_env(),
            options: HashMap::new(),
        }
    }
}

impl LlmConfig {
    /// Validate the LLM configuration.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The model name is not recognized
    /// - Model-specific options are invalid
    pub fn validate(&self) -> std::result::Result<(), String> {
        // Validate model name
        let valid_models = ["claude", "openai", "gemini", "ollama"];
        if !valid_models.contains(&self.model.as_str()) {
            return Err(format!(
                "Invalid model '{}'. Valid options: {}",
                self.model,
                valid_models.join(", ")
            ));
        }

        // Validate model-specific options
        match self.model.as_str() {
            "claude" => self.validate_claude_options()?,
            "openai" => self.validate_openai_options()?,
            "gemini" => self.validate_gemini_options()?,
            "ollama" => self.validate_ollama_options()?,
            _ => {} // Already validated above
        }

        Ok(())
    }

    fn validate_claude_options(&self) -> std::result::Result<(), String> {
        if let Some(variant) = self.options.get("variant") {
            if let Some(variant_str) = variant.as_str() {
                let valid_variants = ["opus", "sonnet", "haiku"];
                if !valid_variants.contains(&variant_str) {
                    return Err(format!(
                        "Invalid Claude variant '{}'. Valid options: {}",
                        variant_str,
                        valid_variants.join(", ")
                    ));
                }
            } else {
                return Err("Claude variant must be a string".to_string());
            }
        }
        Ok(())
    }

    fn validate_openai_options(&self) -> std::result::Result<(), String> {
        if let Some(variant) = self.options.get("variant") {
            if let Some(variant_str) = variant.as_str() {
                let valid_variants = ["gpt-4", "gpt-4o", "gpt-4o-mini", "o1", "o1-mini"];
                if !valid_variants.contains(&variant_str) {
                    return Err(format!(
                        "Invalid OpenAI variant '{}'. Valid options: {}",
                        variant_str,
                        valid_variants.join(", ")
                    ));
                }
            }
        }
        Ok(())
    }

    fn validate_gemini_options(&self) -> std::result::Result<(), String> {
        if let Some(variant) = self.options.get("variant") {
            if let Some(variant_str) = variant.as_str() {
                let valid_variants = ["pro", "flash", "ultra"];
                if !valid_variants.contains(&variant_str) {
                    return Err(format!(
                        "Invalid Gemini variant '{}'. Valid options: {}",
                        variant_str,
                        valid_variants.join(", ")
                    ));
                }
            }
        }
        Ok(())
    }

    fn validate_ollama_options(&self) -> std::result::Result<(), String> {
        // Ollama requires model_name in options
        // But we allow empty options for validation (will fail at runtime)
        Ok(())
    }

    /// Get the Claude variant from options, defaulting to "opus".
    #[must_use]
    pub fn claude_variant(&self) -> &str {
        self.options
            .get("variant")
            .and_then(|v| v.as_str())
            .unwrap_or("opus")
    }
}

// =============================================================================
// Model Status and Info (Phase 12.3)
// =============================================================================

/// Status of a model implementation.
///
/// Indicates whether a model backend is fully implemented or coming soon.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ModelStatus {
    /// Model is fully implemented and available for use.
    Available,
    /// Model is planned but not yet implemented.
    ComingSoon,
}

impl ModelStatus {
    /// Check if the model is available for use.
    #[must_use]
    pub fn is_available(&self) -> bool {
        matches!(self, Self::Available)
    }
}

/// Information about a supported model.
///
/// Provides metadata about each model backend including its current
/// implementation status, description, and available variants.
#[derive(Debug, Clone)]
pub struct ModelInfo {
    /// Model identifier (e.g., "claude", "openai").
    pub name: &'static str,
    /// Human-readable description.
    pub description: &'static str,
    /// Implementation status.
    pub status: ModelStatus,
    /// Available model variants.
    pub variants: &'static [&'static str],
    /// Default API key environment variable.
    pub default_api_key_env: &'static str,
}

/// Get information about all supported models.
///
/// Returns a list of all model backends that Ralph supports or will support,
/// including their current implementation status.
///
/// # Example
///
/// ```rust
/// use ralph::llm::get_supported_models;
///
/// let models = get_supported_models();
/// for model in &models {
///     println!("{}: {:?}", model.name, model.status);
/// }
/// ```
#[must_use]
pub fn get_supported_models() -> Vec<ModelInfo> {
    vec![
        ModelInfo {
            name: "claude",
            description: "Anthropic Claude models via Claude Code CLI",
            status: ModelStatus::Available,
            variants: &["opus", "sonnet", "haiku"],
            default_api_key_env: "ANTHROPIC_API_KEY",
        },
        ModelInfo {
            name: "openai",
            description: "OpenAI GPT models (coming soon)",
            status: ModelStatus::ComingSoon,
            variants: &["gpt-4", "gpt-4o", "gpt-4o-mini", "o1", "o1-mini"],
            default_api_key_env: "OPENAI_API_KEY",
        },
        ModelInfo {
            name: "gemini",
            description: "Google Gemini models (coming soon)",
            status: ModelStatus::ComingSoon,
            variants: &["pro", "flash", "ultra"],
            default_api_key_env: "GOOGLE_API_KEY",
        },
        ModelInfo {
            name: "ollama",
            description: "Local models via Ollama (coming soon)",
            status: ModelStatus::ComingSoon,
            variants: &["llama3", "mistral", "codellama", "custom"],
            default_api_key_env: "",
        },
    ]
}

// =============================================================================
// OpenAI Client Stub (Phase 12.3)
// =============================================================================

/// OpenAI client stub for GPT models.
///
/// **Status: Coming Soon**
///
/// This is a placeholder implementation for OpenAI API integration.
/// Currently, calling `run_prompt` will return an error indicating
/// that the implementation is not yet available.
///
/// # Implementation Roadmap
///
/// When implemented, this client will:
///
/// 1. Use the `OPENAI_API_KEY` environment variable for authentication
/// 2. Support GPT-4, GPT-4o, and o1 model variants
/// 3. Handle tool/function calling via OpenAI's function calling API
/// 4. Support streaming responses for real-time output
///
/// # Example
///
/// ```rust,ignore
/// use ralph::llm::OpenAiClient;
///
/// // Create client for GPT-4o
/// let client = OpenAiClient::new("gpt-4o");
///
/// // Currently returns an error - not yet implemented
/// let result = client.run_prompt("Hello!").await;
/// assert!(result.is_err());
/// ```
#[derive(Debug, Clone)]
pub struct OpenAiClient {
    /// Model variant (e.g., "gpt-4", "gpt-4o", "o1").
    model: String,
    /// API key environment variable name.
    api_key_env: String,
}

impl OpenAiClient {
    /// Create a new OpenAI client stub.
    ///
    /// # Arguments
    ///
    /// * `model` - The model variant to use (e.g., "gpt-4o", "o1")
    #[must_use]
    pub fn new(model: &str) -> Self {
        Self {
            model: model.to_string(),
            api_key_env: "OPENAI_API_KEY".to_string(),
        }
    }

    /// Set the API key environment variable.
    #[must_use]
    pub fn with_api_key_env(mut self, env_var: &str) -> Self {
        self.api_key_env = env_var.to_string();
        self
    }
}

#[async_trait]
impl LlmClient for OpenAiClient {
    async fn run_prompt(&self, _prompt: &str) -> Result<String> {
        anyhow::bail!(
            "OpenAI model '{}' is not yet implemented (coming soon). \
            To use Ralph now, switch to Claude with --model claude or \
            set \"model\": \"claude\" in .claude/settings.json. \
            \n\nImplementation tracking: https://github.com/anthropics/ralph/issues/openai",
            self.model
        )
    }

    fn model_name(&self) -> &str {
        &self.model
    }

    fn supports_tools(&self) -> bool {
        // OpenAI supports function calling, but stub doesn't implement it
        false
    }
}

// =============================================================================
// Gemini Client Stub (Phase 12.3)
// =============================================================================

/// Google Gemini client stub.
///
/// **Status: Coming Soon**
///
/// This is a placeholder implementation for Google's Gemini API integration.
/// Currently, calling `run_prompt` will return an error indicating
/// that the implementation is not yet available.
///
/// # Implementation Roadmap
///
/// When implemented, this client will:
///
/// 1. Use the `GOOGLE_API_KEY` environment variable for authentication
/// 2. Support Gemini Pro, Flash, and Ultra variants
/// 3. Handle tool use via Gemini's function calling API
/// 4. Support multimodal inputs (future)
///
/// # Example
///
/// ```rust,ignore
/// use ralph::llm::GeminiClient;
///
/// // Create client for Gemini Pro
/// let client = GeminiClient::new("pro");
///
/// // Currently returns an error - not yet implemented
/// let result = client.run_prompt("Hello!").await;
/// assert!(result.is_err());
/// ```
#[derive(Debug, Clone)]
pub struct GeminiClient {
    /// Model variant (e.g., "pro", "flash", "ultra").
    variant: String,
    /// API key environment variable name.
    api_key_env: String,
}

impl GeminiClient {
    /// Create a new Gemini client stub.
    ///
    /// # Arguments
    ///
    /// * `variant` - The model variant to use (e.g., "pro", "flash")
    #[must_use]
    pub fn new(variant: &str) -> Self {
        Self {
            variant: variant.to_string(),
            api_key_env: "GOOGLE_API_KEY".to_string(),
        }
    }

    /// Set the API key environment variable.
    #[must_use]
    pub fn with_api_key_env(mut self, env_var: &str) -> Self {
        self.api_key_env = env_var.to_string();
        self
    }
}

#[async_trait]
impl LlmClient for GeminiClient {
    async fn run_prompt(&self, _prompt: &str) -> Result<String> {
        anyhow::bail!(
            "Gemini model 'gemini-{}' is not yet implemented (coming soon). \
            To use Ralph now, switch to Claude with --model claude or \
            set \"model\": \"claude\" in .claude/settings.json. \
            \n\nImplementation tracking: https://github.com/anthropics/ralph/issues/gemini",
            self.variant
        )
    }

    fn model_name(&self) -> &str {
        // Return the full model name (e.g., "gemini-pro")
        // We can't return a dynamically created string, so we match variants
        match self.variant.as_str() {
            "pro" => "gemini-pro",
            "flash" => "gemini-flash",
            "ultra" => "gemini-ultra",
            _ => "gemini-unknown",
        }
    }

    fn supports_tools(&self) -> bool {
        // Gemini supports function calling, but stub doesn't implement it
        false
    }
}

// =============================================================================
// Ollama Client Stub (Phase 12.3)
// =============================================================================

/// Ollama client stub for local models.
///
/// **Status: Coming Soon**
///
/// This is a placeholder implementation for Ollama integration, enabling
/// Ralph to work with locally-hosted LLMs. Currently, calling `run_prompt`
/// will return an error indicating that the implementation is not yet available.
///
/// # Implementation Roadmap
///
/// When implemented, this client will:
///
/// 1. Connect to Ollama server at configurable host (default: `localhost:11434`)
/// 2. Support any model available in the local Ollama installation
/// 3. Work offline without requiring API keys
/// 4. Support custom quantization and model configurations
///
/// # Example
///
/// ```rust,ignore
/// use ralph::llm::OllamaClient;
///
/// // Create client for llama3
/// let client = OllamaClient::new("llama3", None);
///
/// // Create client with custom host
/// let client = OllamaClient::new("codellama", Some("http://192.168.1.100:11434"));
///
/// // Currently returns an error - not yet implemented
/// let result = client.run_prompt("Hello!").await;
/// assert!(result.is_err());
/// ```
#[derive(Debug, Clone)]
pub struct OllamaClient {
    /// Model name (e.g., "llama3", "mistral", "codellama").
    model: String,
    /// Ollama server host URL.
    host: String,
}

impl OllamaClient {
    /// Default Ollama host.
    pub const DEFAULT_HOST: &'static str = "http://localhost:11434";

    /// Create a new Ollama client stub.
    ///
    /// # Arguments
    ///
    /// * `model` - The model name to use (e.g., "llama3", "codellama")
    /// * `host` - Optional custom host URL. Defaults to `http://localhost:11434`
    #[must_use]
    pub fn new(model: &str, host: Option<&str>) -> Self {
        Self {
            model: model.to_string(),
            host: host.unwrap_or(Self::DEFAULT_HOST).to_string(),
        }
    }

    /// Get the configured host URL.
    #[must_use]
    pub fn host(&self) -> &str {
        &self.host
    }
}

#[async_trait]
impl LlmClient for OllamaClient {
    async fn run_prompt(&self, _prompt: &str) -> Result<String> {
        anyhow::bail!(
            "Ollama model '{}' at '{}' is not yet implemented (coming soon). \
            To use Ralph now, switch to Claude with --model claude or \
            set \"model\": \"claude\" in .claude/settings.json. \
            \n\nImplementation tracking: https://github.com/anthropics/ralph/issues/ollama",
            self.model,
            self.host
        )
    }

    fn model_name(&self) -> &str {
        &self.model
    }

    fn supports_tools(&self) -> bool {
        // Ollama supports some function calling, but stub doesn't implement it
        false
    }
}

// =============================================================================
// LLM Client Factory (Phase 12.2 + 12.3)
// =============================================================================

/// Create an LLM client based on configuration.
///
/// This factory function creates the appropriate LLM client implementation
/// based on the model specified in the configuration.
///
/// # Arguments
///
/// * `config` - The LLM configuration specifying which model to use
/// * `project_dir` - The project directory for the client to operate in
///
/// # Returns
///
/// A boxed `LlmClient` trait object for the configured model.
///
/// # Errors
///
/// Returns an error if:
/// - The model is not yet implemented (OpenAI, Gemini, Ollama)
/// - The configuration is invalid
///
/// # Example
///
/// ```rust,ignore
/// use ralph::llm::{LlmConfig, create_llm_client};
///
/// let config = LlmConfig::default();
/// let client = create_llm_client(&config, Path::new("."))?;
/// ```
pub fn create_llm_client(
    config: &LlmConfig,
    project_dir: &Path,
) -> Result<Box<dyn LlmClient>> {
    // Validate configuration first
    config.validate().map_err(|e| anyhow::anyhow!("{}", e))?;

    match config.model.as_str() {
        "claude" => {
            let variant = config.claude_variant();
            let client = ClaudeClient::new(project_dir).with_model(variant);
            Ok(Box::new(client))
        }
        "openai" => {
            // Return stub client - will error on run_prompt but allows testing
            let variant = config
                .options
                .get("variant")
                .and_then(|v| v.as_str())
                .unwrap_or("gpt-4o");
            let client = OpenAiClient::new(variant)
                .with_api_key_env(&config.api_key_env);
            Ok(Box::new(client))
        }
        "gemini" => {
            // Return stub client - will error on run_prompt but allows testing
            let variant = config
                .options
                .get("variant")
                .and_then(|v| v.as_str())
                .unwrap_or("pro");
            let client = GeminiClient::new(variant)
                .with_api_key_env(&config.api_key_env);
            Ok(Box::new(client))
        }
        "ollama" => {
            // Return stub client - will error on run_prompt but allows testing
            let model = config
                .options
                .get("model_name")
                .and_then(|v| v.as_str())
                .unwrap_or("llama3");
            let host = config
                .options
                .get("host")
                .and_then(|v| v.as_str());
            let client = OllamaClient::new(model, host);
            Ok(Box::new(client))
        }
        other => {
            anyhow::bail!(
                "Unknown model '{}'. Valid options: claude, openai, gemini, ollama",
                other
            )
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // =========================================================================
    // LlmConfig Tests (Phase 12.2)
    // =========================================================================

    /// Test model can be specified in settings - LlmConfig default values.
    #[test]
    fn test_llm_config_default_model_is_claude() {
        let config = LlmConfig::default();
        assert_eq!(config.model, "claude");
        assert_eq!(config.api_key_env, "ANTHROPIC_API_KEY");
    }

    /// Test LlmConfig can be created with custom model.
    #[test]
    fn test_llm_config_with_custom_model() {
        let config = LlmConfig {
            model: "openai".to_string(),
            api_key_env: "OPENAI_API_KEY".to_string(),
            options: std::collections::HashMap::new(),
        };
        assert_eq!(config.model, "openai");
        assert_eq!(config.api_key_env, "OPENAI_API_KEY");
    }

    /// Test LlmConfig can store model-specific options.
    #[test]
    fn test_llm_config_with_options() {
        let mut options = std::collections::HashMap::new();
        options.insert("variant".to_string(), serde_json::json!("opus"));
        options.insert("temperature".to_string(), serde_json::json!(0.7));

        let config = LlmConfig {
            model: "claude".to_string(),
            api_key_env: "ANTHROPIC_API_KEY".to_string(),
            options,
        };

        assert_eq!(config.options.get("variant"), Some(&serde_json::json!("opus")));
        assert_eq!(config.options.get("temperature"), Some(&serde_json::json!(0.7)));
    }

    /// Test LlmConfig can be deserialized from JSON.
    #[test]
    fn test_llm_config_deserialize_from_json() {
        let json = r#"{
            "model": "claude",
            "api_key_env": "MY_API_KEY",
            "options": {
                "variant": "sonnet"
            }
        }"#;

        let config: LlmConfig = serde_json::from_str(json).unwrap();
        assert_eq!(config.model, "claude");
        assert_eq!(config.api_key_env, "MY_API_KEY");
        assert_eq!(config.options.get("variant"), Some(&serde_json::json!("sonnet")));
    }

    /// Test LlmConfig deserialize with missing fields uses defaults.
    #[test]
    fn test_llm_config_deserialize_partial() {
        // Only specify model, others should use defaults
        let json = r#"{"model": "gemini"}"#;
        let config: LlmConfig = serde_json::from_str(json).unwrap();

        assert_eq!(config.model, "gemini");
        assert_eq!(config.api_key_env, "ANTHROPIC_API_KEY"); // default
        assert!(config.options.is_empty());
    }

    /// Test LlmConfig deserialize empty object uses all defaults.
    #[test]
    fn test_llm_config_deserialize_empty() {
        let json = r#"{}"#;
        let config: LlmConfig = serde_json::from_str(json).unwrap();

        assert_eq!(config.model, "claude");
        assert_eq!(config.api_key_env, "ANTHROPIC_API_KEY");
    }

    /// Test LlmConfig serializes correctly.
    #[test]
    fn test_llm_config_serialize() {
        let config = LlmConfig::default();
        let json = serde_json::to_string(&config).unwrap();

        assert!(json.contains(r#""model":"claude""#));
        assert!(json.contains(r#""api_key_env":"ANTHROPIC_API_KEY""#));
    }

    /// Test invalid model name produces helpful error during validation.
    #[test]
    fn test_llm_config_validate_invalid_model() {
        let config = LlmConfig {
            model: "unknown_model_xyz".to_string(),
            api_key_env: "API_KEY".to_string(),
            options: std::collections::HashMap::new(),
        };

        let result = config.validate();
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(err.contains("unknown_model_xyz"));
        assert!(err.contains("claude") || err.contains("Valid"));
    }

    /// Test valid model names pass validation.
    #[test]
    fn test_llm_config_validate_valid_models() {
        let valid_models = ["claude", "openai", "gemini", "ollama"];

        for model_name in valid_models {
            let config = LlmConfig {
                model: model_name.to_string(),
                api_key_env: "API_KEY".to_string(),
                options: std::collections::HashMap::new(),
            };
            assert!(config.validate().is_ok(), "Model '{}' should be valid", model_name);
        }
    }

    /// Test model-specific options are validated for Claude.
    #[test]
    fn test_llm_config_validate_claude_options() {
        let mut options = std::collections::HashMap::new();
        options.insert("variant".to_string(), serde_json::json!("opus"));

        let config = LlmConfig {
            model: "claude".to_string(),
            api_key_env: "ANTHROPIC_API_KEY".to_string(),
            options,
        };

        assert!(config.validate().is_ok());
    }

    /// Test invalid Claude variant produces error.
    #[test]
    fn test_llm_config_validate_invalid_claude_variant() {
        let mut options = std::collections::HashMap::new();
        options.insert("variant".to_string(), serde_json::json!("invalid_variant"));

        let config = LlmConfig {
            model: "claude".to_string(),
            api_key_env: "ANTHROPIC_API_KEY".to_string(),
            options,
        };

        let result = config.validate();
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("variant"));
    }

    // =========================================================================
    // Model Factory Tests (Phase 12.2)
    // =========================================================================

    /// Test model factory creates ClaudeClient for claude model.
    #[test]
    fn test_create_llm_client_claude() {
        let config = LlmConfig::default();
        let project_dir = std::path::PathBuf::from(".");

        let client = create_llm_client(&config, &project_dir).unwrap();
        assert!(client.model_name().contains("claude"));
        assert!(client.supports_tools());
    }

    /// Test model factory with claude variant option.
    #[test]
    fn test_create_llm_client_claude_with_variant() {
        let mut options = std::collections::HashMap::new();
        options.insert("variant".to_string(), serde_json::json!("sonnet"));

        let config = LlmConfig {
            model: "claude".to_string(),
            api_key_env: "ANTHROPIC_API_KEY".to_string(),
            options,
        };
        let project_dir = std::path::PathBuf::from(".");

        let client = create_llm_client(&config, &project_dir).unwrap();
        assert!(client.model_name().contains("sonnet"));
    }

    /// Test model factory returns error for invalid model name.
    #[test]
    fn test_create_llm_client_invalid_model() {
        let config = LlmConfig {
            model: "invalid_model_xyz".to_string(),
            api_key_env: "API_KEY".to_string(),
            options: std::collections::HashMap::new(),
        };
        let project_dir = std::path::PathBuf::from(".");

        let result = create_llm_client(&config, &project_dir);
        match result {
            Ok(_) => panic!("Expected error for invalid model name"),
            Err(e) => {
                let err = e.to_string();
                assert!(
                    err.contains("Unknown model") || err.contains("Invalid model"),
                    "Error should mention invalid model: {}",
                    err
                );
            }
        }
    }

    // =========================================================================
    // LlmClient Trait Tests
    // =========================================================================

    /// Test that trait defines `run_prompt(&self, prompt: &str) -> Result<String>`.
    #[tokio::test]
    async fn test_llm_client_trait_defines_run_prompt() {
        let client = MockLlmClient::new().with_response("test response");
        let result = client.run_prompt("test prompt").await;
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "test response");
    }

    /// Test that trait defines `model_name(&self) -> &str`.
    #[test]
    fn test_llm_client_trait_defines_model_name() {
        let client = MockLlmClient::new().with_model_name("test-model");
        assert_eq!(client.model_name(), "test-model");
    }

    /// Test that trait defines `supports_tools(&self) -> bool`.
    #[test]
    fn test_llm_client_trait_defines_supports_tools() {
        let client_no_tools = MockLlmClient::new();
        assert!(!client_no_tools.supports_tools());

        let client_with_tools = MockLlmClient::new().with_tools_support(true);
        assert!(client_with_tools.supports_tools());
    }

    /// Test that trait is object-safe for dynamic dispatch.
    #[tokio::test]
    async fn test_llm_client_trait_is_object_safe() {
        // This test verifies object safety by using `Box<dyn LlmClient>`
        let client: Box<dyn LlmClient> =
            Box::new(MockLlmClient::new().with_response("boxed response"));

        // All trait methods must work through the trait object
        let result = client.run_prompt("test").await;
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "boxed response");

        assert_eq!(client.model_name(), "mock-llm");
        assert!(!client.supports_tools());
    }

    /// Test that trait objects can be stored in collections.
    #[test]
    fn test_llm_client_trait_object_in_collection() {
        let clients: Vec<Box<dyn LlmClient>> = vec![
            Box::new(MockLlmClient::new().with_model_name("model-a")),
            Box::new(MockLlmClient::new().with_model_name("model-b")),
        ];

        assert_eq!(clients[0].model_name(), "model-a");
        assert_eq!(clients[1].model_name(), "model-b");
    }

    /// Test that trait is Send + Sync (required for async contexts).
    #[test]
    fn test_llm_client_is_send_sync() {
        fn assert_send_sync<T: Send + Sync>() {}
        assert_send_sync::<MockLlmClient>();
        assert_send_sync::<ClaudeClient>();
    }

    // =========================================================================
    // MockLlmClient Tests
    // =========================================================================

    /// Test mock implementation works for testing.
    #[tokio::test]
    async fn test_mock_llm_client_basic_usage() {
        let client = MockLlmClient::new()
            .with_response("Hello from mock")
            .with_model_name("mock-v1");

        let response = client.run_prompt("test").await.unwrap();
        assert_eq!(response, "Hello from mock");
        assert_eq!(client.model_name(), "mock-v1");
    }

    /// Test mock call counting.
    #[tokio::test]
    async fn test_mock_llm_client_call_count() {
        let client = MockLlmClient::new();
        assert_eq!(client.call_count(), 0);

        client.run_prompt("test1").await.unwrap();
        assert_eq!(client.call_count(), 1);

        client.run_prompt("test2").await.unwrap();
        assert_eq!(client.call_count(), 2);
    }

    /// Test mock error handling.
    #[tokio::test]
    async fn test_mock_llm_client_error() {
        let client = MockLlmClient::new().with_error("API rate limited");

        let result = client.run_prompt("test").await;
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("API rate limited"));
    }

    /// Test mock fail count for retry testing.
    #[tokio::test]
    async fn test_mock_llm_client_fail_count() {
        let client = MockLlmClient::new()
            .with_fail_count(2, "Connection timeout")
            .with_response("success");

        // First two calls fail
        assert!(client.run_prompt("test").await.is_err());
        assert!(client.run_prompt("test").await.is_err());

        // Third call succeeds
        let result = client.run_prompt("test").await;
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "success");

        assert_eq!(client.call_count(), 3);
    }

    /// Test mock tools support configuration.
    #[tokio::test]
    async fn test_mock_llm_client_tools_support() {
        let client_no_tools = MockLlmClient::new();
        assert!(!client_no_tools.supports_tools());

        let client_with_tools = MockLlmClient::new().with_tools_support(true);
        assert!(client_with_tools.supports_tools());
    }

    /// Test mock default values.
    #[test]
    fn test_mock_llm_client_default() {
        let client = MockLlmClient::default();
        assert_eq!(client.model_name(), "mock-llm");
        assert!(!client.supports_tools());
        assert_eq!(client.call_count(), 0);
    }

    /// Test mock clone preserves state.
    #[tokio::test]
    async fn test_mock_llm_client_clone() {
        let client = MockLlmClient::new()
            .with_response("cloned response")
            .with_model_name("cloned-model");

        let cloned = client.clone();
        assert_eq!(cloned.model_name(), "cloned-model");

        let response = cloned.run_prompt("test").await.unwrap();
        assert_eq!(response, "cloned response");
    }

    // =========================================================================
    // ClaudeClient Tests
    // =========================================================================

    /// Test Claude client creation.
    #[test]
    fn test_claude_client_creation() {
        let client = ClaudeClient::new(".");
        assert_eq!(client.model_name(), "claude-opus-4");
        assert!(client.supports_tools());
    }

    /// Test Claude client with different models.
    #[test]
    fn test_claude_client_model_variants() {
        let opus = ClaudeClient::new(".").with_model("opus");
        assert_eq!(opus.model_name(), "claude-opus-4");

        let sonnet = ClaudeClient::new(".").with_model("sonnet");
        assert_eq!(sonnet.model_name(), "claude-sonnet-4");

        let haiku = ClaudeClient::new(".").with_model("haiku");
        assert_eq!(haiku.model_name(), "claude-haiku-3.5");

        let custom = ClaudeClient::new(".").with_model("custom-model");
        assert_eq!(custom.model_name(), "custom-model");
    }

    /// Test Claude client always supports tools.
    #[test]
    fn test_claude_client_supports_tools() {
        let client = ClaudeClient::new(".");
        assert!(client.supports_tools());
    }

    /// Test Claude client project directory configuration.
    #[test]
    fn test_claude_client_project_dir() {
        let client = ClaudeClient::new("/tmp/test-project");
        assert_eq!(client.project_dir, PathBuf::from("/tmp/test-project"));
    }

    /// Test Claude client is Send + Sync.
    #[test]
    fn test_claude_client_send_sync() {
        fn assert_send_sync<T: Send + Sync>() {}
        assert_send_sync::<ClaudeClient>();
    }

    /// Test Claude client can be used as trait object.
    #[test]
    fn test_claude_client_as_trait_object() {
        let client: Box<dyn LlmClient> = Box::new(ClaudeClient::new("."));
        assert_eq!(client.model_name(), "claude-opus-4");
        assert!(client.supports_tools());
    }

    // =========================================================================
    // OpenAI Client Stub Tests (Phase 12.3)
    // =========================================================================

    /// Test OpenAI client stub exists and can be created.
    #[test]
    fn test_openai_client_stub_exists() {
        let client = OpenAiClient::new("gpt-4o");
        assert!(client.model_name().contains("gpt"));
    }

    /// Test OpenAI client stub returns "not implemented" error.
    #[tokio::test]
    async fn test_openai_client_stub_returns_not_implemented() {
        let client = OpenAiClient::new("gpt-4o");
        let result = client.run_prompt("test").await;
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(
            err.contains("not yet implemented") || err.contains("coming soon"),
            "Error should indicate not implemented: {}",
            err
        );
    }

    /// Test OpenAI client stub model variants.
    #[test]
    fn test_openai_client_stub_model_variants() {
        let gpt4 = OpenAiClient::new("gpt-4");
        assert_eq!(gpt4.model_name(), "gpt-4");

        let gpt4o = OpenAiClient::new("gpt-4o");
        assert_eq!(gpt4o.model_name(), "gpt-4o");

        let o1 = OpenAiClient::new("o1");
        assert_eq!(o1.model_name(), "o1");
    }

    /// Test OpenAI client stub is Send + Sync.
    #[test]
    fn test_openai_client_stub_send_sync() {
        fn assert_send_sync<T: Send + Sync>() {}
        assert_send_sync::<OpenAiClient>();
    }

    /// Test OpenAI client stub can be used as trait object.
    #[test]
    fn test_openai_client_stub_as_trait_object() {
        let client: Box<dyn LlmClient> = Box::new(OpenAiClient::new("gpt-4o"));
        assert!(client.model_name().contains("gpt"));
    }

    // =========================================================================
    // Gemini Client Stub Tests (Phase 12.3)
    // =========================================================================

    /// Test Gemini client stub exists and can be created.
    #[test]
    fn test_gemini_client_stub_exists() {
        let client = GeminiClient::new("pro");
        assert!(client.model_name().contains("gemini"));
    }

    /// Test Gemini client stub returns "not implemented" error.
    #[tokio::test]
    async fn test_gemini_client_stub_returns_not_implemented() {
        let client = GeminiClient::new("pro");
        let result = client.run_prompt("test").await;
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(
            err.contains("not yet implemented") || err.contains("coming soon"),
            "Error should indicate not implemented: {}",
            err
        );
    }

    /// Test Gemini client stub model variants.
    #[test]
    fn test_gemini_client_stub_model_variants() {
        let pro = GeminiClient::new("pro");
        assert_eq!(pro.model_name(), "gemini-pro");

        let flash = GeminiClient::new("flash");
        assert_eq!(flash.model_name(), "gemini-flash");

        let ultra = GeminiClient::new("ultra");
        assert_eq!(ultra.model_name(), "gemini-ultra");
    }

    /// Test Gemini client stub is Send + Sync.
    #[test]
    fn test_gemini_client_stub_send_sync() {
        fn assert_send_sync<T: Send + Sync>() {}
        assert_send_sync::<GeminiClient>();
    }

    /// Test Gemini client stub can be used as trait object.
    #[test]
    fn test_gemini_client_stub_as_trait_object() {
        let client: Box<dyn LlmClient> = Box::new(GeminiClient::new("pro"));
        assert!(client.model_name().contains("gemini"));
    }

    // =========================================================================
    // Ollama Client Stub Tests (Phase 12.3)
    // =========================================================================

    /// Test Ollama client stub exists and can be created.
    #[test]
    fn test_ollama_client_stub_exists() {
        let client = OllamaClient::new("llama3", None);
        assert!(client.model_name().contains("llama3"));
    }

    /// Test Ollama client stub returns "not implemented" error.
    #[tokio::test]
    async fn test_ollama_client_stub_returns_not_implemented() {
        let client = OllamaClient::new("llama3", None);
        let result = client.run_prompt("test").await;
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(
            err.contains("not yet implemented") || err.contains("coming soon"),
            "Error should indicate not implemented: {}",
            err
        );
    }

    /// Test Ollama client stub with custom host.
    #[test]
    fn test_ollama_client_stub_with_custom_host() {
        let client = OllamaClient::new("llama3", Some("http://localhost:11434"));
        assert_eq!(client.host(), "http://localhost:11434");

        let client_default = OllamaClient::new("llama3", None);
        assert_eq!(client_default.host(), OllamaClient::DEFAULT_HOST);
    }

    /// Test Ollama client stub is Send + Sync.
    #[test]
    fn test_ollama_client_stub_send_sync() {
        fn assert_send_sync<T: Send + Sync>() {}
        assert_send_sync::<OllamaClient>();
    }

    /// Test Ollama client stub can be used as trait object.
    #[test]
    fn test_ollama_client_stub_as_trait_object() {
        let client: Box<dyn LlmClient> = Box::new(OllamaClient::new("llama3", None));
        assert!(client.model_name().contains("llama"));
    }

    // =========================================================================
    // Model Status and List Tests (Phase 12.3)
    // =========================================================================

    /// Test ModelStatus enum has correct variants.
    #[test]
    fn test_model_status_variants() {
        let available = ModelStatus::Available;
        let coming_soon = ModelStatus::ComingSoon;

        assert!(available.is_available());
        assert!(!coming_soon.is_available());
    }

    /// Test get_supported_models returns all models with correct status.
    #[test]
    fn test_get_supported_models_includes_all() {
        let models = get_supported_models();

        // Should include Claude (available)
        let claude = models.iter().find(|m| m.name == "claude");
        assert!(claude.is_some());
        assert!(claude.unwrap().status.is_available());

        // Should include OpenAI (coming soon)
        let openai = models.iter().find(|m| m.name == "openai");
        assert!(openai.is_some());
        assert!(!openai.unwrap().status.is_available());

        // Should include Gemini (coming soon)
        let gemini = models.iter().find(|m| m.name == "gemini");
        assert!(gemini.is_some());
        assert!(!gemini.unwrap().status.is_available());

        // Should include Ollama (coming soon)
        let ollama = models.iter().find(|m| m.name == "ollama");
        assert!(ollama.is_some());
        assert!(!ollama.unwrap().status.is_available());
    }

    /// Test ModelInfo contains documentation.
    #[test]
    fn test_model_info_has_documentation() {
        let models = get_supported_models();

        for model in &models {
            assert!(!model.description.is_empty(), "Model {} should have description", model.name);
            assert!(!model.variants.is_empty(), "Model {} should have variants", model.name);
        }
    }

    // =========================================================================
    // Updated Factory Tests (Phase 12.3)
    // =========================================================================

    /// Test factory can create OpenAI stub when enabled.
    #[test]
    fn test_create_llm_client_openai_stub() {
        let config = LlmConfig {
            model: "openai".to_string(),
            api_key_env: "OPENAI_API_KEY".to_string(),
            options: std::collections::HashMap::new(),
        };
        let project_dir = std::path::PathBuf::from(".");

        // Factory should return a stub client (not an error)
        let client = create_llm_client(&config, &project_dir).unwrap();
        assert!(client.model_name().contains("gpt"));
    }

    /// Test factory can create Gemini stub when enabled.
    #[test]
    fn test_create_llm_client_gemini_stub() {
        let config = LlmConfig {
            model: "gemini".to_string(),
            api_key_env: "GOOGLE_API_KEY".to_string(),
            options: std::collections::HashMap::new(),
        };
        let project_dir = std::path::PathBuf::from(".");

        // Factory should return a stub client (not an error)
        let client = create_llm_client(&config, &project_dir).unwrap();
        assert!(client.model_name().contains("gemini"));
    }

    /// Test factory can create Ollama stub when enabled.
    #[test]
    fn test_create_llm_client_ollama_stub() {
        let mut options = std::collections::HashMap::new();
        options.insert("model_name".to_string(), serde_json::json!("llama3"));

        let config = LlmConfig {
            model: "ollama".to_string(),
            api_key_env: String::new(),
            options,
        };
        let project_dir = std::path::PathBuf::from(".");

        // Factory should return a stub client (not an error)
        let client = create_llm_client(&config, &project_dir).unwrap();
        assert!(client.model_name().contains("llama"));
    }
}
