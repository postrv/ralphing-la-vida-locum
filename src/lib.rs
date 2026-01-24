//! Ralph - Claude Code Automation Suite
//!
//! A Rust-based automation suite for running Claude Code autonomously
//! with bombproof reliability, type-checking, and memory guarantees.
//!
//! # Architecture
//!
//! The crate is organized into several modules:
//!
//! - [`checkpoint`] - Checkpoint and rollback system for quality regression prevention
//! - [`config`] - Configuration loading and validation
//! - [`error`] - Custom error types and handling
//! - [`narsil`] - narsil-mcp integration for code intelligence
//! - [`prompt`] - Dynamic prompt generation and context management
//! - [`quality`] - Quality gate enforcement and remediation
//! - [`testing`] - Testing infrastructure (traits, mocks, fixtures)
//!
//! # Example
//!
//! ```rust,ignore
//! use ralph::config::ProjectConfig;
//! use ralph::testing::{MockGitOperations, TestFixture};
//! use ralph::quality::{QualityGateEnforcer, generate_remediation_prompt};
//!
//! // Load project configuration
//! let config = ProjectConfig::load(".")?;
//!
//! // Run quality gates before committing
//! let enforcer = QualityGateEnforcer::standard(".");
//! if let Err(failures) = enforcer.can_commit() {
//!     let prompt = generate_remediation_prompt(&failures);
//!     println!("{}", prompt);
//! }
//!
//! // Use test fixtures in tests
//! let fixture = TestFixture::minimal_project();
//! ```

pub mod analytics;
pub mod audit;
pub mod bootstrap;
pub mod campaign;
pub mod changes;
pub mod checkpoint;
pub mod config;
pub mod error;
pub mod llm;
pub mod narsil;
pub mod prompt;
pub mod quality;
pub mod stagnation;
pub mod testing;
pub mod verify;

// Re-export commonly used types
pub use error::{IntoRalphError, RalphError, Result};

// Re-export config types
pub use config::{
    is_ssh_command, suggest_gh_alternative, verify_git_environment, ArrayMergeStrategy,
    ConfigLevel, ConfigLoader, ConfigLocations, ConfigSource, ConfigValidator, GitEnvironmentCheck,
    InheritanceChain, PredictorWeightsConfig, ProjectConfig, StagnationLevel, ValidationReport,
    DANGEROUS_PATTERNS, SECRET_PATTERNS, SSH_BLOCKED_PATTERNS,
};

// Re-export testing types for convenience
pub use testing::{
    ClaudeProcess, FileSystem, GitOperations, MockClaudeProcess, MockFileSystem, MockGitOperations,
    MockQualityChecker, QualityChecker, QualityGateResult,
};

// Re-export quality gate types
pub use quality::{
    generate_minimal_remediation, generate_remediation_prompt, ClippyConfig, ClippyGate,
    EnforcerConfig, EnforcerSummary, Gate, GateIssue, GatePlugin, GateResult, IssueSeverity,
    LibraryConfig, NoAllowGate, NoTodoGate, PluginConfig, PluginError, PluginExecutor,
    PluginManifest, PluginMetadata, QualityGateEnforcer, RemediationConfig, RemediationGenerator,
    SecurityGate, TestConfig, TestGate,
};

// Re-export checkpoint types
pub use checkpoint::{
    manager::{CheckpointManager, CheckpointManagerConfig},
    rollback::{RollbackManager, RollbackResult},
    Checkpoint, CheckpointDiff, CheckpointId, LanguageRegression, LintRegressionResult,
    LintRegressionSeverity, LintRegressionThresholds, QualityMetrics, RegressionThresholds,
    WarningTrend, WarningTrendDirection, WarningTrendPoint,
};

// Re-export narsil types
pub use narsil::{
    Dependency, NarsilClient, NarsilConfig, NarsilError, Reference, SecurityFinding,
    SecuritySeverity, ToolResponse,
};

// Re-export analytics types
pub use analytics::{
    AggregateStats, Analytics, AnalyticsEvent, AnalyticsUploadConfig, AnalyticsUploader,
    EventFilter, EventType, GateIssueEventData, GateResultEventData, GateStats,
    PredictorAccuracyStats, PredictorDecisionEventData, PrivacySettings, QualityMetricsSnapshot,
    QualityTrend, ReportFormat, SessionReport, SessionSummary, StructuredEvent,
    StubAnalyticsUploader, TrendData, TrendDirection, TrendMetric, TrendPoint, SCHEMA_VERSION,
};

// Re-export dashboard types (Phase 25.1)
pub use analytics::dashboard::{DashboardData, DashboardSummary, TimeRange};

// Re-export audit types
pub use audit::{
    AuditEntry, AuditEventType, AuditLogger, AuditQuery, AuditQueryResult, AuditReader,
    RepairResult, RotationConfig, VerificationResult,
};

// Re-export campaign types
pub use campaign::{
    create_campaign_api, Campaign, CampaignApi, CampaignConfig, CampaignStatus, CampaignUpdate,
    CloudCampaignApi, CloudOperationResult, LocalCampaignApi,
};

// Re-export LLM types
pub use llm::{
    create_llm_client, get_supported_models, ClaudeApiError, ClaudeClient, ClaudeModel,
    ClaudeProvider, GeminiClient, LlmClient, LlmConfig, MockLlmClient, ModelInfo, ModelStatus,
    OllamaClient, OpenAiClient, ParseClaudeModelError, RateLimitTracker,
};

// Re-export bootstrap types
pub use bootstrap::language::{Language, ParseLanguageError};
pub use bootstrap::language_detector::{DetectedLanguage, LanguageDetector};
pub use bootstrap::templates::{TemplateKind, TemplateRegistry};

// Re-export verify types
pub use verify::{
    create_verifier, CcgVerifier, MockCcgVerifier, QualityDelta, VerificationConfig,
    VerificationFinding, VerificationReport, VerificationSeverity,
};

// Re-export changes types
pub use changes::{ChangeDetector, ChangeScope};

// Re-export stagnation types
pub use stagnation::{
    PredictorStats, RiskLevel, RiskScore, RiskWeights, StatsPersistence, MIN_STATS_VERSION,
    STATS_FILENAME, STATS_VERSION,
};

// TestFixture is only available in test builds
#[cfg(test)]
pub use testing::TestFixture;
