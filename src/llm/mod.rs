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
            anyhow::bail!(
                "OpenAI model support is not yet implemented (coming soon). \
                Use --model claude or set \"model\": \"claude\" in settings.json"
            )
        }
        "gemini" => {
            anyhow::bail!(
                "Gemini model support is not yet implemented (coming soon). \
                Use --model claude or set \"model\": \"claude\" in settings.json"
            )
        }
        "ollama" => {
            anyhow::bail!(
                "Ollama model support is not yet implemented (coming soon). \
                Use --model claude or set \"model\": \"claude\" in settings.json"
            )
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

    /// Test model factory returns error for unsupported model.
    #[test]
    fn test_create_llm_client_unsupported_model() {
        let config = LlmConfig {
            model: "openai".to_string(),
            api_key_env: "OPENAI_API_KEY".to_string(),
            options: std::collections::HashMap::new(),
        };
        let project_dir = std::path::PathBuf::from(".");

        let result = create_llm_client(&config, &project_dir);
        match result {
            Ok(_) => panic!("Expected error for unsupported model"),
            Err(e) => {
                let err = e.to_string();
                assert!(
                    err.contains("not yet implemented") || err.contains("coming soon"),
                    "Error should mention model not implemented: {}",
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
}
