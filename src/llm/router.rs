//! Provider Router for LLM fallback and selection.
//!
//! This module provides intelligent provider routing with automatic fallback
//! support. It enables Ralph to try multiple LLM providers in sequence when
//! one fails due to rate limiting, timeouts, or other transient errors.
//!
//! # Architecture
//!
//! The [`ProviderRouter`] wraps multiple [`LlmClient`] implementations and
//! handles provider selection and fallback logic. It supports:
//!
//! - Explicit provider selection via `--model` flag
//! - Automatic mode that tries providers in preference order
//! - Fallback on rate limits, timeouts, and connection errors
//! - Optional fallback disable via `--no-fallback`
//!
//! # Example
//!
//! ```rust,ignore
//! use ralph::llm::{ProviderRouter, ProviderSelection, LlmClient};
//!
//! let router = ProviderRouter::builder()
//!     .add_provider("claude", Box::new(claude_client))
//!     .add_provider("openai", Box::new(openai_client))
//!     .preference_order(vec!["claude", "openai"])
//!     .build();
//!
//! // Auto mode: tries providers in order
//! let response = router.run_prompt("Hello!").await?;
//!
//! // Explicit selection
//! let router = router.with_selection(ProviderSelection::Explicit("openai".to_string()));
//! ```

use crate::llm::{CompletionRequest, LlmClient, LlmResponse, ProviderCapabilities};
use anyhow::{Context, Result};
use async_trait::async_trait;
use std::collections::HashMap;
use std::sync::Arc;
use tracing::{debug, info, warn};

// =============================================================================
// Provider Selection (Phase 23.5)
// =============================================================================

/// How to select which provider to use.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum ProviderSelection {
    /// Use a specific provider by name.
    Explicit(String),
    /// Automatically select based on availability and preference.
    #[default]
    Auto,
}

impl ProviderSelection {
    /// Parse from a string (e.g., from CLI flag).
    ///
    /// # Example
    ///
    /// ```rust
    /// use ralph::llm::router::ProviderSelection;
    ///
    /// let selection: ProviderSelection = "auto".parse().unwrap();
    /// assert_eq!(selection, ProviderSelection::Auto);
    ///
    /// let explicit: ProviderSelection = "claude".parse().unwrap();
    /// assert_eq!(explicit, ProviderSelection::Explicit("claude".to_string()));
    /// ```
    #[must_use]
    pub fn parse_str(s: &str) -> Self {
        if s.eq_ignore_ascii_case("auto") {
            Self::Auto
        } else {
            Self::Explicit(s.to_lowercase())
        }
    }
}

impl std::str::FromStr for ProviderSelection {
    type Err = std::convert::Infallible;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(Self::parse_str(s))
    }
}

// =============================================================================
// Fallback Configuration (Phase 23.5)
// =============================================================================

/// Configuration for fallback behavior.
#[derive(Debug, Clone)]
pub struct FallbackConfig {
    /// Whether fallback is enabled.
    pub enabled: bool,
    /// Maximum number of providers to try.
    pub max_attempts: usize,
    /// Whether to fallback on rate limit errors.
    pub on_rate_limit: bool,
    /// Whether to fallback on timeout errors.
    pub on_timeout: bool,
    /// Whether to fallback on connection errors.
    pub on_connection_error: bool,
}

impl Default for FallbackConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            max_attempts: 3,
            on_rate_limit: true,
            on_timeout: true,
            on_connection_error: true,
        }
    }
}

impl FallbackConfig {
    /// Create a config with fallback disabled.
    #[must_use]
    pub fn disabled() -> Self {
        Self {
            enabled: false,
            ..Default::default()
        }
    }

    /// Check if the error should trigger a fallback.
    #[must_use]
    pub fn should_fallback(&self, error: &anyhow::Error) -> bool {
        if !self.enabled {
            return false;
        }

        let error_str = error.to_string().to_lowercase();

        // Check for rate limit errors
        if self.on_rate_limit
            && (error_str.contains("rate limit")
                || error_str.contains("rate_limit")
                || error_str.contains("429")
                || error_str.contains("too many requests"))
        {
            return true;
        }

        // Check for timeout errors
        if self.on_timeout
            && (error_str.contains("timeout")
                || error_str.contains("timed out")
                || error_str.contains("deadline"))
        {
            return true;
        }

        // Check for connection errors
        if self.on_connection_error
            && (error_str.contains("connection")
                || error_str.contains("network")
                || error_str.contains("unreachable")
                || error_str.contains("refused"))
        {
            return true;
        }

        false
    }
}

// =============================================================================
// Router Event (Phase 23.5)
// =============================================================================

/// Events emitted during routing for observability.
#[derive(Debug, Clone)]
pub enum RouterEvent {
    /// A provider was selected.
    ProviderSelected { name: String },
    /// Falling back to another provider.
    FallingBack { from: String, to: String, reason: String },
    /// All providers exhausted.
    AllProvidersExhausted { attempted: Vec<String> },
}

// =============================================================================
// Provider Router (Phase 23.5)
// =============================================================================

/// Routes requests to LLM providers with fallback support.
///
/// The router manages multiple LLM providers and handles:
/// - Provider selection based on configuration
/// - Automatic fallback on transient errors
/// - Logging of provider switches
///
/// # Thread Safety
///
/// The router is `Send + Sync` and can be safely shared across async tasks.
///
/// # Example
///
/// ```rust,ignore
/// use ralph::llm::{ProviderRouter, ProviderSelection};
///
/// let router = ProviderRouter::builder()
///     .add_provider("claude", Box::new(claude))
///     .add_provider("openai", Box::new(openai))
///     .build();
///
/// // Use auto mode (default)
/// let response = router.run_prompt("Hello").await?;
///
/// // Or select a specific provider
/// let explicit = router.clone().with_selection(ProviderSelection::Explicit("openai".to_string()));
/// let response = explicit.run_prompt("Hello").await?;
/// ```
pub struct ProviderRouter {
    /// Registered providers by name.
    providers: HashMap<String, Arc<dyn LlmClient>>,
    /// Order of preference for auto mode.
    preference_order: Vec<String>,
    /// Current selection mode.
    selection: ProviderSelection,
    /// Fallback configuration.
    fallback: FallbackConfig,
}

impl std::fmt::Debug for ProviderRouter {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ProviderRouter")
            .field("providers", &self.providers.keys().collect::<Vec<_>>())
            .field("preference_order", &self.preference_order)
            .field("selection", &self.selection)
            .field("fallback", &self.fallback)
            .finish()
    }
}

impl ProviderRouter {
    /// Create a new router builder.
    #[must_use]
    pub fn builder() -> ProviderRouterBuilder {
        ProviderRouterBuilder::new()
    }

    /// Create a clone with a different selection mode.
    #[must_use]
    pub fn with_selection(mut self, selection: ProviderSelection) -> Self {
        self.selection = selection;
        self
    }

    /// Create a clone with fallback disabled.
    #[must_use]
    pub fn with_no_fallback(mut self) -> Self {
        self.fallback.enabled = false;
        self
    }

    /// Create a clone with a custom fallback configuration.
    #[must_use]
    pub fn with_fallback_config(mut self, config: FallbackConfig) -> Self {
        self.fallback = config;
        self
    }

    /// Get the current selection mode.
    #[must_use]
    pub fn selection(&self) -> &ProviderSelection {
        &self.selection
    }

    /// Get the fallback configuration.
    #[must_use]
    pub fn fallback_config(&self) -> &FallbackConfig {
        &self.fallback
    }

    /// Check if a specific provider is registered.
    #[must_use]
    pub fn has_provider(&self, name: &str) -> bool {
        self.providers.contains_key(name)
    }

    /// Get the names of all registered providers.
    #[must_use]
    pub fn provider_names(&self) -> Vec<&str> {
        self.providers.keys().map(String::as_str).collect()
    }

    /// Get a specific provider by name.
    #[must_use]
    pub fn get_provider(&self, name: &str) -> Option<&Arc<dyn LlmClient>> {
        self.providers.get(name)
    }

    /// Get the preference order.
    #[must_use]
    pub fn preference_order(&self) -> &[String] {
        &self.preference_order
    }

    /// Get the providers to try in order based on selection mode.
    fn get_providers_to_try(&self) -> Vec<(&str, &Arc<dyn LlmClient>)> {
        match &self.selection {
            ProviderSelection::Explicit(name) => {
                if let Some(provider) = self.providers.get(name) {
                    vec![(name.as_str(), provider)]
                } else {
                    vec![]
                }
            }
            ProviderSelection::Auto => {
                // Use preference order, then any remaining providers
                let mut result: Vec<(&str, &Arc<dyn LlmClient>)> = Vec::new();
                let mut added = std::collections::HashSet::new();

                // First add providers in preference order
                for name in &self.preference_order {
                    if let Some(provider) = self.providers.get(name) {
                        result.push((name.as_str(), provider));
                        added.insert(name.as_str());
                    }
                }

                // Then add any remaining providers
                for (name, provider) in &self.providers {
                    if !added.contains(name.as_str()) {
                        result.push((name.as_str(), provider));
                    }
                }

                result
            }
        }
    }

    /// Run a prompt with fallback support.
    async fn run_with_fallback(&self, prompt: &str) -> Result<(String, String)> {
        let providers_to_try = self.get_providers_to_try();

        if providers_to_try.is_empty() {
            anyhow::bail!("No providers available");
        }

        let max_attempts = if self.fallback.enabled {
            self.fallback.max_attempts.min(providers_to_try.len())
        } else {
            1
        };

        let mut attempted = Vec::new();
        let mut last_error: Option<anyhow::Error> = None;

        for (name, provider) in providers_to_try.iter().take(max_attempts) {
            attempted.push(name.to_string());

            debug!(provider = %name, "Attempting request with provider");

            match provider.run_prompt(prompt).await {
                Ok(response) => {
                    if attempted.len() > 1 {
                        info!(
                            provider = %name,
                            attempts = attempted.len(),
                            "Request succeeded after fallback"
                        );
                    }
                    return Ok((response, name.to_string()));
                }
                Err(error) => {
                    let should_fallback = self.fallback.should_fallback(&error);

                    if should_fallback && attempted.len() < max_attempts {
                        // Find next provider name for logging
                        let next_provider = providers_to_try
                            .get(attempted.len())
                            .map(|(n, _)| *n)
                            .unwrap_or("unknown");

                        warn!(
                            from = %name,
                            to = %next_provider,
                            reason = %error,
                            "Falling back to next provider"
                        );
                    } else if !should_fallback {
                        // Non-fallback error, return immediately
                        return Err(error);
                    }

                    last_error = Some(error);
                }
            }
        }

        // All providers exhausted
        Err(last_error.unwrap_or_else(|| anyhow::anyhow!("All providers exhausted")))
            .context(format!(
                "All {} providers failed: {:?}",
                attempted.len(),
                attempted
            ))
    }
}

// Implement Send + Sync explicitly (Arc<dyn LlmClient> is Send + Sync)
unsafe impl Send for ProviderRouter {}
unsafe impl Sync for ProviderRouter {}

#[async_trait]
impl LlmClient for ProviderRouter {
    async fn run_prompt(&self, prompt: &str) -> Result<String> {
        let (response, _provider) = self.run_with_fallback(prompt).await?;
        Ok(response)
    }

    async fn complete(&self, request: CompletionRequest) -> Result<LlmResponse> {
        let providers_to_try = self.get_providers_to_try();

        if providers_to_try.is_empty() {
            anyhow::bail!("No providers available");
        }

        let max_attempts = if self.fallback.enabled {
            self.fallback.max_attempts.min(providers_to_try.len())
        } else {
            1
        };

        let mut attempted = Vec::new();
        let mut last_error: Option<anyhow::Error> = None;

        for (name, provider) in providers_to_try.iter().take(max_attempts) {
            attempted.push(name.to_string());

            debug!(provider = %name, "Attempting completion with provider");

            match provider.complete(request.clone()).await {
                Ok(response) => {
                    return Ok(response);
                }
                Err(error) => {
                    let should_fallback = self.fallback.should_fallback(&error);

                    if should_fallback && attempted.len() < max_attempts {
                        let next_provider = providers_to_try
                            .get(attempted.len())
                            .map(|(n, _)| *n)
                            .unwrap_or("unknown");

                        warn!(
                            from = %name,
                            to = %next_provider,
                            reason = %error,
                            "Falling back to next provider"
                        );
                    } else if !should_fallback {
                        return Err(error);
                    }

                    last_error = Some(error);
                }
            }
        }

        Err(last_error.unwrap_or_else(|| anyhow::anyhow!("All providers exhausted")))
            .context(format!(
                "All {} providers failed: {:?}",
                attempted.len(),
                attempted
            ))
    }

    async fn available(&self) -> bool {
        // Check if any provider is available
        for provider in self.providers.values() {
            if provider.available().await {
                return true;
            }
        }
        false
    }

    fn model_name(&self) -> &str {
        // Return the first provider's model name
        match &self.selection {
            ProviderSelection::Explicit(name) => {
                if let Some(provider) = self.providers.get(name) {
                    return provider.model_name();
                }
            }
            ProviderSelection::Auto => {
                if let Some(name) = self.preference_order.first() {
                    if let Some(provider) = self.providers.get(name) {
                        return provider.model_name();
                    }
                }
            }
        }
        "router"
    }

    fn supports_tools(&self) -> bool {
        // Return the first provider's tool support
        match &self.selection {
            ProviderSelection::Explicit(name) => {
                if let Some(provider) = self.providers.get(name) {
                    return provider.supports_tools();
                }
            }
            ProviderSelection::Auto => {
                if let Some(name) = self.preference_order.first() {
                    if let Some(provider) = self.providers.get(name) {
                        return provider.supports_tools();
                    }
                }
            }
        }
        false
    }

    fn capabilities(&self) -> ProviderCapabilities {
        match &self.selection {
            ProviderSelection::Explicit(name) => {
                if let Some(provider) = self.providers.get(name) {
                    return provider.capabilities();
                }
            }
            ProviderSelection::Auto => {
                if let Some(name) = self.preference_order.first() {
                    if let Some(provider) = self.providers.get(name) {
                        return provider.capabilities();
                    }
                }
            }
        }
        ProviderCapabilities::default()
    }

    fn cost_per_token(&self) -> (f64, f64) {
        match &self.selection {
            ProviderSelection::Explicit(name) => {
                if let Some(provider) = self.providers.get(name) {
                    return provider.cost_per_token();
                }
            }
            ProviderSelection::Auto => {
                if let Some(name) = self.preference_order.first() {
                    if let Some(provider) = self.providers.get(name) {
                        return provider.cost_per_token();
                    }
                }
            }
        }
        (0.0, 0.0)
    }
}

// =============================================================================
// Provider Router Builder (Phase 23.5)
// =============================================================================

/// Builder for creating a [`ProviderRouter`].
#[derive(Default)]
pub struct ProviderRouterBuilder {
    providers: HashMap<String, Arc<dyn LlmClient>>,
    preference_order: Vec<String>,
    selection: ProviderSelection,
    fallback: FallbackConfig,
}

impl ProviderRouterBuilder {
    /// Create a new builder.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Add a provider to the router.
    #[must_use]
    pub fn add_provider(mut self, name: &str, provider: Box<dyn LlmClient>) -> Self {
        self.providers.insert(name.to_lowercase(), Arc::from(provider));
        self
    }

    /// Add a provider wrapped in Arc.
    #[must_use]
    pub fn add_provider_arc(mut self, name: &str, provider: Arc<dyn LlmClient>) -> Self {
        self.providers.insert(name.to_lowercase(), provider);
        self
    }

    /// Set the preference order for auto mode.
    #[must_use]
    pub fn preference_order(mut self, order: Vec<&str>) -> Self {
        self.preference_order = order.into_iter().map(|s| s.to_lowercase()).collect();
        self
    }

    /// Set the selection mode.
    #[must_use]
    pub fn selection(mut self, selection: ProviderSelection) -> Self {
        self.selection = selection;
        self
    }

    /// Set the fallback configuration.
    #[must_use]
    pub fn fallback(mut self, config: FallbackConfig) -> Self {
        self.fallback = config;
        self
    }

    /// Disable fallback.
    #[must_use]
    pub fn no_fallback(mut self) -> Self {
        self.fallback.enabled = false;
        self
    }

    /// Build the router.
    ///
    /// # Panics
    ///
    /// Panics if no providers have been added.
    #[must_use]
    pub fn build(self) -> ProviderRouter {
        assert!(!self.providers.is_empty(), "At least one provider is required");

        // If no preference order, use providers in insertion order
        let preference_order = if self.preference_order.is_empty() {
            self.providers.keys().cloned().collect()
        } else {
            self.preference_order
        };

        ProviderRouter {
            providers: self.providers,
            preference_order,
            selection: self.selection,
            fallback: self.fallback,
        }
    }
}

// =============================================================================
// Tests (TDD - Phase 23.5)
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::llm::MockLlmClient;

    // =========================================================================
    // ProviderSelection Tests
    // =========================================================================

    #[test]
    fn test_provider_selection_parse_str_auto() {
        assert_eq!(ProviderSelection::parse_str("auto"), ProviderSelection::Auto);
        assert_eq!(ProviderSelection::parse_str("AUTO"), ProviderSelection::Auto);
        assert_eq!(ProviderSelection::parse_str("Auto"), ProviderSelection::Auto);
    }

    #[test]
    fn test_provider_selection_parse_str_explicit() {
        assert_eq!(
            ProviderSelection::parse_str("claude"),
            ProviderSelection::Explicit("claude".to_string())
        );
        assert_eq!(
            ProviderSelection::parse_str("OpenAI"),
            ProviderSelection::Explicit("openai".to_string())
        );
    }

    #[test]
    fn test_provider_selection_from_str_trait() {
        let auto: ProviderSelection = "auto".parse().unwrap();
        assert_eq!(auto, ProviderSelection::Auto);

        let claude: ProviderSelection = "claude".parse().unwrap();
        assert_eq!(claude, ProviderSelection::Explicit("claude".to_string()));
    }

    #[test]
    fn test_provider_selection_default_is_auto() {
        assert_eq!(ProviderSelection::default(), ProviderSelection::Auto);
    }

    // =========================================================================
    // FallbackConfig Tests
    // =========================================================================

    #[test]
    fn test_fallback_config_default() {
        let config = FallbackConfig::default();
        assert!(config.enabled);
        assert!(config.on_rate_limit);
        assert!(config.on_timeout);
        assert!(config.on_connection_error);
    }

    #[test]
    fn test_fallback_config_disabled() {
        let config = FallbackConfig::disabled();
        assert!(!config.enabled);
    }

    #[test]
    fn test_fallback_should_fallback_rate_limit() {
        let config = FallbackConfig::default();

        let rate_limit_error = anyhow::anyhow!("Rate limit exceeded, retry after 60s");
        assert!(config.should_fallback(&rate_limit_error));

        let error_429 = anyhow::anyhow!("HTTP 429: Too Many Requests");
        assert!(config.should_fallback(&error_429));
    }

    #[test]
    fn test_fallback_should_fallback_timeout() {
        let config = FallbackConfig::default();

        let timeout_error = anyhow::anyhow!("Request timed out after 120s");
        assert!(config.should_fallback(&timeout_error));
    }

    #[test]
    fn test_fallback_should_fallback_connection() {
        let config = FallbackConfig::default();

        let connection_error = anyhow::anyhow!("Connection refused");
        assert!(config.should_fallback(&connection_error));
    }

    #[test]
    fn test_fallback_should_not_fallback_auth_error() {
        let config = FallbackConfig::default();

        let auth_error = anyhow::anyhow!("Invalid API key");
        assert!(!config.should_fallback(&auth_error));
    }

    #[test]
    fn test_fallback_should_not_fallback_when_disabled() {
        let config = FallbackConfig::disabled();

        let rate_limit_error = anyhow::anyhow!("Rate limit exceeded");
        assert!(!config.should_fallback(&rate_limit_error));
    }

    // =========================================================================
    // ProviderRouter Builder Tests
    // =========================================================================

    #[test]
    fn test_provider_router_builder_creates_router() {
        let mock = MockLlmClient::new().with_response("test");
        let router = ProviderRouter::builder()
            .add_provider("mock", Box::new(mock))
            .build();

        assert!(router.has_provider("mock"));
    }

    #[test]
    fn test_provider_router_builder_preference_order() {
        let mock1 = MockLlmClient::new().with_model_name("mock1");
        let mock2 = MockLlmClient::new().with_model_name("mock2");

        let router = ProviderRouter::builder()
            .add_provider("provider1", Box::new(mock1))
            .add_provider("provider2", Box::new(mock2))
            .preference_order(vec!["provider2", "provider1"])
            .build();

        assert_eq!(router.preference_order()[0], "provider2");
        assert_eq!(router.preference_order()[1], "provider1");
    }

    #[test]
    #[should_panic(expected = "At least one provider is required")]
    fn test_provider_router_builder_panics_without_providers() {
        let _router = ProviderRouter::builder().build();
    }

    // =========================================================================
    // Required Tests from Implementation Plan
    // =========================================================================

    /// Test: test_provider_router_selects_requested_provider
    #[tokio::test]
    async fn test_provider_router_selects_requested_provider() {
        let claude_mock = MockLlmClient::new()
            .with_response("claude response")
            .with_model_name("claude");
        let openai_mock = MockLlmClient::new()
            .with_response("openai response")
            .with_model_name("openai");

        let router = ProviderRouter::builder()
            .add_provider("claude", Box::new(claude_mock))
            .add_provider("openai", Box::new(openai_mock))
            .preference_order(vec!["claude", "openai"])
            .selection(ProviderSelection::Explicit("openai".to_string()))
            .build();

        let response = router.run_prompt("test").await.unwrap();
        assert_eq!(response, "openai response");
    }

    /// Test: test_provider_router_auto_mode_tries_in_order
    #[tokio::test]
    async fn test_provider_router_auto_mode_tries_in_order() {
        let first_mock = MockLlmClient::new()
            .with_response("first response")
            .with_model_name("first");
        let second_mock = MockLlmClient::new()
            .with_response("second response")
            .with_model_name("second");

        let router = ProviderRouter::builder()
            .add_provider("first", Box::new(first_mock))
            .add_provider("second", Box::new(second_mock))
            .preference_order(vec!["first", "second"])
            .selection(ProviderSelection::Auto)
            .build();

        // Should use first provider in auto mode
        let response = router.run_prompt("test").await.unwrap();
        assert_eq!(response, "first response");
    }

    /// Test: test_provider_router_fallback_on_rate_limit
    #[tokio::test]
    async fn test_provider_router_fallback_on_rate_limit() {
        // First provider fails with rate limit
        let rate_limited = MockLlmClient::new()
            .with_error("Rate limit exceeded, retry after 60s")
            .with_model_name("rate-limited");
        let fallback = MockLlmClient::new()
            .with_response("fallback response")
            .with_model_name("fallback");

        let router = ProviderRouter::builder()
            .add_provider("primary", Box::new(rate_limited))
            .add_provider("fallback", Box::new(fallback))
            .preference_order(vec!["primary", "fallback"])
            .selection(ProviderSelection::Auto)
            .build();

        // Should fall back to second provider
        let response = router.run_prompt("test").await.unwrap();
        assert_eq!(response, "fallback response");
    }

    /// Test: test_provider_router_no_fallback_flag
    #[tokio::test]
    async fn test_provider_router_no_fallback_flag() {
        // First provider fails with rate limit
        let rate_limited = MockLlmClient::new()
            .with_error("Rate limit exceeded")
            .with_model_name("rate-limited");
        let fallback = MockLlmClient::new()
            .with_response("fallback response")
            .with_model_name("fallback");

        let router = ProviderRouter::builder()
            .add_provider("primary", Box::new(rate_limited))
            .add_provider("fallback", Box::new(fallback))
            .preference_order(vec!["primary", "fallback"])
            .selection(ProviderSelection::Auto)
            .no_fallback() // Disable fallback
            .build();

        // Should NOT fall back, should return error
        let result = router.run_prompt("test").await;
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("Rate limit"));
    }

    // =========================================================================
    // Additional Router Tests
    // =========================================================================

    #[tokio::test]
    async fn test_provider_router_implements_llm_client() {
        fn assert_llm_client<T: LlmClient>() {}
        assert_llm_client::<ProviderRouter>();
    }

    #[test]
    fn test_provider_router_is_send_sync() {
        fn assert_send_sync<T: Send + Sync>() {}
        assert_send_sync::<ProviderRouter>();
    }

    #[tokio::test]
    async fn test_provider_router_no_providers_returns_error() {
        // Note: We can't test this directly with builder since it panics
        // Instead test with explicit selection of non-existent provider
        let mock = MockLlmClient::new().with_response("test");
        let router = ProviderRouter::builder()
            .add_provider("mock", Box::new(mock))
            .selection(ProviderSelection::Explicit("nonexistent".to_string()))
            .build();

        let result = router.run_prompt("test").await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_provider_router_fallback_on_timeout() {
        let timeout_provider = MockLlmClient::new()
            .with_error("Request timed out after 120s")
            .with_model_name("timeout");
        let fallback = MockLlmClient::new()
            .with_response("success")
            .with_model_name("fallback");

        let router = ProviderRouter::builder()
            .add_provider("primary", Box::new(timeout_provider))
            .add_provider("fallback", Box::new(fallback))
            .preference_order(vec!["primary", "fallback"])
            .build();

        let response = router.run_prompt("test").await.unwrap();
        assert_eq!(response, "success");
    }

    #[tokio::test]
    async fn test_provider_router_fallback_on_connection_error() {
        let connection_error = MockLlmClient::new()
            .with_error("Connection refused")
            .with_model_name("unavailable");
        let fallback = MockLlmClient::new()
            .with_response("success")
            .with_model_name("fallback");

        let router = ProviderRouter::builder()
            .add_provider("primary", Box::new(connection_error))
            .add_provider("fallback", Box::new(fallback))
            .preference_order(vec!["primary", "fallback"])
            .build();

        let response = router.run_prompt("test").await.unwrap();
        assert_eq!(response, "success");
    }

    #[tokio::test]
    async fn test_provider_router_no_fallback_on_auth_error() {
        let auth_error = MockLlmClient::new()
            .with_error("Invalid API key")
            .with_model_name("invalid-key");
        let fallback = MockLlmClient::new()
            .with_response("success")
            .with_model_name("fallback");

        let router = ProviderRouter::builder()
            .add_provider("primary", Box::new(auth_error))
            .add_provider("fallback", Box::new(fallback))
            .preference_order(vec!["primary", "fallback"])
            .build();

        // Auth errors should NOT trigger fallback
        let result = router.run_prompt("test").await;
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("Invalid API key"));
    }

    #[test]
    fn test_provider_router_model_name_reflects_selection() {
        let claude_mock = MockLlmClient::new().with_model_name("claude-opus-4");
        let openai_mock = MockLlmClient::new().with_model_name("gpt-4o");

        // Auto mode - uses first in preference
        let auto_router = ProviderRouter::builder()
            .add_provider("claude", Box::new(claude_mock.clone()))
            .add_provider("openai", Box::new(openai_mock.clone()))
            .preference_order(vec!["claude", "openai"])
            .selection(ProviderSelection::Auto)
            .build();

        assert_eq!(auto_router.model_name(), "claude-opus-4");

        // Explicit mode - uses selected
        let explicit_router = ProviderRouter::builder()
            .add_provider("claude", Box::new(claude_mock))
            .add_provider("openai", Box::new(openai_mock))
            .selection(ProviderSelection::Explicit("openai".to_string()))
            .build();

        assert_eq!(explicit_router.model_name(), "gpt-4o");
    }

    #[tokio::test]
    async fn test_provider_router_available_checks_all_providers() {
        let available_mock = MockLlmClient::new(); // No error = available
        let unavailable_mock = MockLlmClient::new().with_error("Service unavailable");

        let router = ProviderRouter::builder()
            .add_provider("available", Box::new(available_mock))
            .add_provider("unavailable", Box::new(unavailable_mock))
            .build();

        // Should return true if any provider is available
        assert!(router.available().await);
    }

    #[test]
    fn test_provider_router_with_selection_creates_new_config() {
        let mock = MockLlmClient::new();

        let router = ProviderRouter::builder()
            .add_provider("mock", Box::new(mock))
            .selection(ProviderSelection::Auto)
            .build();

        assert_eq!(router.selection(), &ProviderSelection::Auto);

        let explicit_router = router.with_selection(ProviderSelection::Explicit("mock".to_string()));
        assert_eq!(
            explicit_router.selection(),
            &ProviderSelection::Explicit("mock".to_string())
        );
    }

    #[tokio::test]
    async fn test_provider_router_max_attempts_limits_fallback() {
        let fail1 = MockLlmClient::new().with_error("Rate limit exceeded");
        let fail2 = MockLlmClient::new().with_error("Rate limit exceeded");
        let success = MockLlmClient::new().with_response("success");

        let router = ProviderRouter::builder()
            .add_provider("fail1", Box::new(fail1))
            .add_provider("fail2", Box::new(fail2))
            .add_provider("success", Box::new(success))
            .preference_order(vec!["fail1", "fail2", "success"])
            .fallback(FallbackConfig {
                max_attempts: 2, // Only try 2 providers
                ..Default::default()
            })
            .build();

        // Should fail because max_attempts=2 and third provider (success) won't be tried
        let result = router.run_prompt("test").await;
        assert!(result.is_err());
    }
}
