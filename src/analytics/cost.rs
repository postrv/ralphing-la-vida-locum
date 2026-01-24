//! LLM cost tracking and persistence.
//!
//! This module provides cost tracking across LLM providers, enabling
//! users to monitor their spending per provider and per session.
//!
//! # Architecture
//!
//! The [`CostTracker`] accumulates costs from LLM response objects
//! and persists them to `.ralph/costs.json` for cross-session tracking.
//!
//! # Example
//!
//! ```rust,ignore
//! use ralph::analytics::cost::{CostTracker, ProviderCost};
//!
//! let mut tracker = CostTracker::new(".ralph/costs.json");
//! tracker.record_usage("claude", 1000, 500, 0.015);
//! tracker.save()?;
//!
//! let total = tracker.total_cost();
//! println!("Total cost: ${:.4}", total);
//! ```

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs::{self, File};
use std::io::{BufReader, BufWriter};
use std::path::{Path, PathBuf};

// =============================================================================
// Provider Cost Stats
// =============================================================================

/// Cost statistics for a single LLM provider.
///
/// Tracks token usage and estimated costs for a specific provider
/// (Claude, OpenAI, Ollama, etc.).
///
/// # Example
///
/// ```rust
/// use ralph::analytics::cost::ProviderCost;
///
/// let cost = ProviderCost::default();
/// assert_eq!(cost.total_input_tokens, 0);
/// assert_eq!(cost.total_output_tokens, 0);
/// assert_eq!(cost.total_cost_usd, 0.0);
/// ```
#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
pub struct ProviderCost {
    /// Total input tokens consumed.
    pub total_input_tokens: u64,
    /// Total output tokens generated.
    pub total_output_tokens: u64,
    /// Total cost in USD.
    pub total_cost_usd: f64,
    /// Number of requests made.
    pub request_count: u64,
}

impl ProviderCost {
    /// Create a new provider cost tracker.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Total tokens (input + output).
    #[must_use]
    pub fn total_tokens(&self) -> u64 {
        self.total_input_tokens + self.total_output_tokens
    }

    /// Average cost per request.
    ///
    /// Returns `None` if no requests have been made.
    #[must_use]
    pub fn avg_cost_per_request(&self) -> Option<f64> {
        if self.request_count == 0 {
            None
        } else {
            Some(self.total_cost_usd / self.request_count as f64)
        }
    }

    /// Record a usage event.
    ///
    /// # Arguments
    ///
    /// * `input_tokens` - Number of input tokens used
    /// * `output_tokens` - Number of output tokens generated
    /// * `cost_usd` - Cost in USD for this request
    pub fn record(&mut self, input_tokens: u32, output_tokens: u32, cost_usd: f64) {
        self.total_input_tokens += u64::from(input_tokens);
        self.total_output_tokens += u64::from(output_tokens);
        self.total_cost_usd += cost_usd;
        self.request_count += 1;
    }
}

// =============================================================================
// Session Cost Summary
// =============================================================================

/// Cost summary for a single session.
///
/// Tracks costs accumulated during a single Ralph session.
#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
pub struct SessionCost {
    /// Session identifier.
    pub session_id: String,
    /// Costs per provider for this session.
    pub providers: HashMap<String, ProviderCost>,
    /// Total cost for this session.
    pub total_cost_usd: f64,
}

impl SessionCost {
    /// Create a new session cost tracker.
    #[must_use]
    pub fn new(session_id: impl Into<String>) -> Self {
        Self {
            session_id: session_id.into(),
            providers: HashMap::new(),
            total_cost_usd: 0.0,
        }
    }

    /// Record a usage event for a provider.
    pub fn record(&mut self, provider: &str, input_tokens: u32, output_tokens: u32, cost_usd: f64) {
        let provider_cost = self.providers.entry(provider.to_string()).or_default();
        provider_cost.record(input_tokens, output_tokens, cost_usd);
        self.total_cost_usd += cost_usd;
    }
}

// =============================================================================
// Cost Tracker
// =============================================================================

/// Persisted cost data structure.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct CostData {
    /// Version of the cost data schema.
    pub version: u32,
    /// Cumulative costs per provider (all-time).
    pub providers: HashMap<String, ProviderCost>,
    /// Recent session costs (last 100 sessions).
    pub recent_sessions: Vec<SessionCost>,
}

impl CostData {
    /// Current schema version.
    pub const CURRENT_VERSION: u32 = 1;

    /// Create new cost data.
    #[must_use]
    pub fn new() -> Self {
        Self {
            version: Self::CURRENT_VERSION,
            providers: HashMap::new(),
            recent_sessions: Vec::new(),
        }
    }

    /// Total cost across all providers.
    #[must_use]
    pub fn total_cost(&self) -> f64 {
        self.providers.values().map(|p| p.total_cost_usd).sum()
    }

    /// Total tokens across all providers.
    #[must_use]
    pub fn total_tokens(&self) -> u64 {
        self.providers.values().map(|p| p.total_tokens()).sum()
    }
}

/// LLM cost tracker with persistence.
///
/// Tracks token usage and costs across providers and sessions,
/// persisting data to `.ralph/costs.json`.
///
/// # Example
///
/// ```rust,ignore
/// use ralph::analytics::cost::CostTracker;
///
/// let mut tracker = CostTracker::new("/path/to/project");
/// tracker.start_session("session-001")?;
/// tracker.record_usage("claude", 1000, 500, Some(0.015));
/// tracker.end_session()?;
/// tracker.save()?;
///
/// println!("Total cost: ${:.4}", tracker.total_cost());
/// ```
#[derive(Debug)]
pub struct CostTracker {
    /// Path to the costs.json file.
    costs_file: PathBuf,
    /// Loaded cost data.
    data: CostData,
    /// Current session (if active).
    current_session: Option<SessionCost>,
    /// Whether data has been modified since last save.
    dirty: bool,
}

impl CostTracker {
    /// Maximum number of recent sessions to keep.
    pub const MAX_RECENT_SESSIONS: usize = 100;

    /// Create a new cost tracker for a project directory.
    ///
    /// # Arguments
    ///
    /// * `project_dir` - Project directory containing `.ralph/`
    ///
    /// # Errors
    ///
    /// Returns an error if the costs file exists but cannot be parsed.
    pub fn new(project_dir: impl AsRef<Path>) -> Result<Self> {
        let costs_file = project_dir.as_ref().join(".ralph/costs.json");
        let data = if costs_file.exists() {
            let file = File::open(&costs_file).context("Failed to open costs file")?;
            let reader = BufReader::new(file);
            serde_json::from_reader(reader).context("Failed to parse costs file")?
        } else {
            CostData::new()
        };

        Ok(Self {
            costs_file,
            data,
            current_session: None,
            dirty: false,
        })
    }

    /// Create an in-memory cost tracker (for testing).
    #[must_use]
    pub fn in_memory() -> Self {
        Self {
            costs_file: PathBuf::from("/dev/null"),
            data: CostData::new(),
            current_session: None,
            dirty: false,
        }
    }

    /// Start a new session.
    ///
    /// # Arguments
    ///
    /// * `session_id` - Unique identifier for the session
    ///
    /// # Panics
    ///
    /// Panics if a session is already active. Call `end_session()` first.
    pub fn start_session(&mut self, session_id: impl Into<String>) {
        assert!(
            self.current_session.is_none(),
            "Session already active. Call end_session() first."
        );
        self.current_session = Some(SessionCost::new(session_id));
    }

    /// Record a usage event.
    ///
    /// # Arguments
    ///
    /// * `provider` - Provider name (e.g., "claude", "openai", "ollama")
    /// * `input_tokens` - Number of input tokens
    /// * `output_tokens` - Number of output tokens
    /// * `cost_usd` - Estimated cost in USD (None for free providers like Ollama)
    pub fn record_usage(
        &mut self,
        provider: &str,
        input_tokens: u32,
        output_tokens: u32,
        cost_usd: Option<f64>,
    ) {
        let cost = cost_usd.unwrap_or(0.0);

        // Update cumulative provider stats
        let provider_cost = self.data.providers.entry(provider.to_string()).or_default();
        provider_cost.record(input_tokens, output_tokens, cost);

        // Update current session if active
        if let Some(session) = &mut self.current_session {
            session.record(provider, input_tokens, output_tokens, cost);
        }

        self.dirty = true;
    }

    /// End the current session.
    ///
    /// Finalizes the session and adds it to recent sessions history.
    pub fn end_session(&mut self) {
        if let Some(session) = self.current_session.take() {
            // Only keep recent sessions
            if self.data.recent_sessions.len() >= Self::MAX_RECENT_SESSIONS {
                self.data.recent_sessions.remove(0);
            }
            self.data.recent_sessions.push(session);
            self.dirty = true;
        }
    }

    /// Get the total cost across all providers.
    #[must_use]
    pub fn total_cost(&self) -> f64 {
        self.data.total_cost()
    }

    /// Get the total tokens used across all providers.
    #[must_use]
    pub fn total_tokens(&self) -> u64 {
        self.data.total_tokens()
    }

    /// Get costs for a specific provider.
    #[must_use]
    pub fn provider_cost(&self, provider: &str) -> Option<&ProviderCost> {
        self.data.providers.get(provider)
    }

    /// Get all provider costs.
    #[must_use]
    pub fn all_providers(&self) -> &HashMap<String, ProviderCost> {
        &self.data.providers
    }

    /// Get recent session costs.
    #[must_use]
    pub fn recent_sessions(&self) -> &[SessionCost] {
        &self.data.recent_sessions
    }

    /// Get the current session cost (if active).
    #[must_use]
    pub fn current_session(&self) -> Option<&SessionCost> {
        self.current_session.as_ref()
    }

    /// Check if data has been modified since last save.
    #[must_use]
    pub fn is_dirty(&self) -> bool {
        self.dirty
    }

    /// Save cost data to disk.
    ///
    /// # Errors
    ///
    /// Returns an error if the file cannot be written.
    pub fn save(&mut self) -> Result<()> {
        if !self.dirty {
            return Ok(());
        }

        // Ensure directory exists
        if let Some(parent) = self.costs_file.parent() {
            fs::create_dir_all(parent).context("Failed to create .ralph directory")?;
        }

        let file = File::create(&self.costs_file).context("Failed to create costs file")?;
        let writer = BufWriter::new(file);
        serde_json::to_writer_pretty(writer, &self.data).context("Failed to write costs file")?;

        self.dirty = false;
        Ok(())
    }

    /// Format a cost summary for display.
    #[must_use]
    pub fn format_summary(&self) -> String {
        let mut lines = Vec::new();
        lines.push("=== LLM Cost Summary ===".to_string());
        lines.push(String::new());

        // Total
        lines.push(format!("Total Cost: ${:.4}", self.total_cost()));
        lines.push(format!(
            "Total Tokens: {} ({} requests)",
            self.total_tokens(),
            self.data
                .providers
                .values()
                .map(|p| p.request_count)
                .sum::<u64>()
        ));
        lines.push(String::new());

        // Per provider
        if !self.data.providers.is_empty() {
            lines.push("By Provider:".to_string());
            let mut providers: Vec<_> = self.data.providers.iter().collect();
            providers.sort_by(|a, b| b.1.total_cost_usd.partial_cmp(&a.1.total_cost_usd).unwrap());

            for (name, cost) in providers {
                lines.push(format!(
                    "  {}: ${:.4} ({} tokens, {} requests)",
                    name,
                    cost.total_cost_usd,
                    cost.total_tokens(),
                    cost.request_count
                ));
            }
        }

        lines.join("\n")
    }
}

// =============================================================================
// Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    // -------------------------------------------------------------------------
    // ProviderCost Tests
    // -------------------------------------------------------------------------

    #[test]
    fn test_provider_cost_default() {
        let cost = ProviderCost::default();
        assert_eq!(cost.total_input_tokens, 0);
        assert_eq!(cost.total_output_tokens, 0);
        assert_eq!(cost.total_cost_usd, 0.0);
        assert_eq!(cost.request_count, 0);
    }

    #[test]
    fn test_provider_cost_record() {
        let mut cost = ProviderCost::new();
        cost.record(1000, 500, 0.015);

        assert_eq!(cost.total_input_tokens, 1000);
        assert_eq!(cost.total_output_tokens, 500);
        assert_eq!(cost.total_cost_usd, 0.015);
        assert_eq!(cost.request_count, 1);
    }

    #[test]
    fn test_provider_cost_accumulates() {
        let mut cost = ProviderCost::new();
        cost.record(1000, 500, 0.015);
        cost.record(2000, 1000, 0.030);

        assert_eq!(cost.total_input_tokens, 3000);
        assert_eq!(cost.total_output_tokens, 1500);
        assert_eq!(cost.total_cost_usd, 0.045);
        assert_eq!(cost.request_count, 2);
    }

    #[test]
    fn test_provider_cost_total_tokens() {
        let mut cost = ProviderCost::new();
        cost.record(1000, 500, 0.0);

        assert_eq!(cost.total_tokens(), 1500);
    }

    #[test]
    fn test_provider_cost_avg_cost_per_request() {
        let mut cost = ProviderCost::new();
        assert_eq!(cost.avg_cost_per_request(), None);

        cost.record(1000, 500, 0.010);
        cost.record(2000, 1000, 0.020);

        let avg = cost.avg_cost_per_request().unwrap();
        assert!((avg - 0.015).abs() < f64::EPSILON);
    }

    // -------------------------------------------------------------------------
    // SessionCost Tests
    // -------------------------------------------------------------------------

    #[test]
    fn test_session_cost_new() {
        let session = SessionCost::new("test-session");
        assert_eq!(session.session_id, "test-session");
        assert!(session.providers.is_empty());
        assert_eq!(session.total_cost_usd, 0.0);
    }

    #[test]
    fn test_session_cost_record() {
        let mut session = SessionCost::new("test");
        session.record("claude", 1000, 500, 0.015);
        session.record("openai", 500, 250, 0.008);

        assert_eq!(session.providers.len(), 2);
        assert_eq!(session.total_cost_usd, 0.023);
        assert!(session.providers.contains_key("claude"));
        assert!(session.providers.contains_key("openai"));
    }

    // -------------------------------------------------------------------------
    // CostData Tests
    // -------------------------------------------------------------------------

    #[test]
    fn test_cost_data_new() {
        let data = CostData::new();
        assert_eq!(data.version, CostData::CURRENT_VERSION);
        assert!(data.providers.is_empty());
        assert!(data.recent_sessions.is_empty());
    }

    #[test]
    fn test_cost_data_total_cost() {
        let mut data = CostData::new();
        data.providers
            .insert("claude".to_string(), ProviderCost::default());
        data.providers.get_mut("claude").unwrap().total_cost_usd = 0.50;
        data.providers
            .insert("openai".to_string(), ProviderCost::default());
        data.providers.get_mut("openai").unwrap().total_cost_usd = 0.30;

        assert!((data.total_cost() - 0.80).abs() < f64::EPSILON);
    }

    // -------------------------------------------------------------------------
    // CostTracker Tests
    // -------------------------------------------------------------------------

    #[test]
    fn test_cost_tracker_in_memory() {
        let tracker = CostTracker::in_memory();
        assert_eq!(tracker.total_cost(), 0.0);
        assert_eq!(tracker.total_tokens(), 0);
        assert!(!tracker.is_dirty());
    }

    #[test]
    fn test_cost_tracker_record_usage() {
        let mut tracker = CostTracker::in_memory();
        tracker.record_usage("claude", 1000, 500, Some(0.015));

        assert_eq!(tracker.total_cost(), 0.015);
        assert_eq!(tracker.total_tokens(), 1500);
        assert!(tracker.is_dirty());
    }

    #[test]
    fn test_cost_tracker_record_free_provider() {
        let mut tracker = CostTracker::in_memory();
        tracker.record_usage("ollama", 1000, 500, None);

        assert_eq!(tracker.total_cost(), 0.0);
        assert_eq!(tracker.total_tokens(), 1500);
        assert_eq!(tracker.provider_cost("ollama").unwrap().request_count, 1);
    }

    #[test]
    fn test_cost_tracker_multiple_providers() {
        let mut tracker = CostTracker::in_memory();
        tracker.record_usage("claude", 1000, 500, Some(0.015));
        tracker.record_usage("openai", 2000, 1000, Some(0.025));
        tracker.record_usage("ollama", 500, 250, None);

        assert_eq!(tracker.total_cost(), 0.040);
        assert_eq!(tracker.total_tokens(), 5250);
        assert_eq!(tracker.all_providers().len(), 3);
    }

    #[test]
    fn test_cost_tracker_session_lifecycle() {
        let mut tracker = CostTracker::in_memory();

        tracker.start_session("session-001");
        tracker.record_usage("claude", 1000, 500, Some(0.015));
        tracker.end_session();

        assert!(tracker.current_session().is_none());
        assert_eq!(tracker.recent_sessions().len(), 1);
        assert_eq!(tracker.recent_sessions()[0].session_id, "session-001");
        assert_eq!(tracker.recent_sessions()[0].total_cost_usd, 0.015);
    }

    #[test]
    fn test_cost_tracker_session_isolation() {
        let mut tracker = CostTracker::in_memory();

        // Session 1
        tracker.start_session("session-001");
        tracker.record_usage("claude", 1000, 500, Some(0.015));
        tracker.end_session();

        // Session 2
        tracker.start_session("session-002");
        tracker.record_usage("openai", 2000, 1000, Some(0.025));
        tracker.end_session();

        assert_eq!(tracker.recent_sessions().len(), 2);
        assert_eq!(tracker.recent_sessions()[0].total_cost_usd, 0.015);
        assert_eq!(tracker.recent_sessions()[1].total_cost_usd, 0.025);

        // Cumulative totals
        assert_eq!(tracker.total_cost(), 0.040);
    }

    #[test]
    #[should_panic(expected = "Session already active")]
    fn test_cost_tracker_double_start_panics() {
        let mut tracker = CostTracker::in_memory();
        tracker.start_session("session-001");
        tracker.start_session("session-002"); // Should panic
    }

    #[test]
    fn test_cost_tracker_max_recent_sessions() {
        let mut tracker = CostTracker::in_memory();

        // Add MAX + 1 sessions
        for i in 0..=CostTracker::MAX_RECENT_SESSIONS {
            tracker.start_session(format!("session-{:03}", i));
            tracker.record_usage("claude", 100, 50, Some(0.001));
            tracker.end_session();
        }

        // Should keep only MAX sessions
        assert_eq!(
            tracker.recent_sessions().len(),
            CostTracker::MAX_RECENT_SESSIONS
        );

        // First session should be removed
        assert_eq!(tracker.recent_sessions()[0].session_id, "session-001");
    }

    #[test]
    fn test_cost_tracker_persistence() {
        let temp_dir = TempDir::new().unwrap();
        let project_dir = temp_dir.path();

        // Create and populate tracker
        {
            let mut tracker = CostTracker::new(project_dir).unwrap();
            tracker.start_session("test-session");
            tracker.record_usage("claude", 1000, 500, Some(0.015));
            tracker.end_session();
            tracker.save().unwrap();
        }

        // Load and verify
        {
            let tracker = CostTracker::new(project_dir).unwrap();
            assert_eq!(tracker.total_cost(), 0.015);
            assert_eq!(tracker.total_tokens(), 1500);
            assert_eq!(tracker.recent_sessions().len(), 1);
        }
    }

    #[test]
    fn test_cost_tracker_format_summary() {
        let mut tracker = CostTracker::in_memory();
        tracker.record_usage("claude", 10000, 5000, Some(0.150));
        tracker.record_usage("openai", 5000, 2500, Some(0.075));
        tracker.record_usage("ollama", 2000, 1000, None);

        let summary = tracker.format_summary();

        assert!(summary.contains("Total Cost: $0.2250"));
        assert!(summary.contains("Total Tokens: 25500"));
        assert!(summary.contains("claude: $0.1500"));
        assert!(summary.contains("openai: $0.0750"));
        assert!(summary.contains("ollama: $0.0000"));
    }

    #[test]
    fn test_cost_tracker_provider_cost_lookup() {
        let mut tracker = CostTracker::in_memory();
        tracker.record_usage("claude", 1000, 500, Some(0.015));

        let claude_cost = tracker.provider_cost("claude");
        assert!(claude_cost.is_some());
        assert_eq!(claude_cost.unwrap().total_cost_usd, 0.015);

        let unknown_cost = tracker.provider_cost("unknown");
        assert!(unknown_cost.is_none());
    }

    #[test]
    fn test_cost_tracker_no_save_when_clean() {
        let temp_dir = TempDir::new().unwrap();
        let project_dir = temp_dir.path();

        // Create empty tracker and save
        let mut tracker = CostTracker::new(project_dir).unwrap();
        assert!(!tracker.is_dirty());

        // Save should be a no-op
        tracker.save().unwrap();

        // File should not be created
        assert!(!project_dir.join(".ralph/costs.json").exists());
    }

    #[test]
    fn test_cost_tracker_dirty_after_record() {
        let mut tracker = CostTracker::in_memory();
        assert!(!tracker.is_dirty());

        tracker.record_usage("claude", 100, 50, Some(0.001));
        assert!(tracker.is_dirty());
    }
}
