//! Quality gate enforcement module.
//!
//! This module provides comprehensive code quality checking through gates:
//!
//! - [`gates`] - Individual quality gate implementations
//! - [`enforcer`] - Gate orchestration and enforcement
//! - [`remediation`] - Remediation guidance generation
//!
//! # Architecture
//!
//! ```text
//! ┌─────────────────────┐
//! │ QualityGateEnforcer │
//! │                     │
//! │  - run_all()        │
//! │  - can_commit()     │
//! └─────────┬───────────┘
//!           │
//!     ┌─────┼─────┬─────┬─────┐
//!     │     │     │     │     │
//!     ▼     ▼     ▼     ▼     ▼
//! ┌──────┐ ┌───┐ ┌─────┐ ┌────┐ ┌──────┐
//! │Clippy│ │Test│ │NoAllow│ │Sec │ │NoTodo│
//! │Gate  │ │Gate│ │Gate │ │Gate│ │Gate  │
//! └──────┘ └───┘ └─────┘ └────┘ └──────┘
//!           │
//!           ▼
//! ┌─────────────────────┐
//! │ RemediationGenerator│
//! │                     │
//! │  generate_prompt()  │
//! └─────────────────────┘
//! ```
//!
//! # Example
//!
//! ```rust,ignore
//! use ralph::quality::{QualityGateEnforcer, generate_remediation_prompt};
//!
//! let enforcer = QualityGateEnforcer::standard("/path/to/project");
//!
//! match enforcer.can_commit() {
//!     Ok(()) => {
//!         println!("All quality gates passed!");
//!     }
//!     Err(failures) => {
//!         let prompt = generate_remediation_prompt(&failures);
//!         println!("{}", prompt);
//!     }
//! }
//! ```
//!
//! # Available Gates
//!
//! | Gate | Purpose | Blocking |
//! |------|---------|----------|
//! | `ClippyGate` | Runs `cargo clippy` with warnings as errors | Yes |
//! | `TestGate` | Runs `cargo test` and verifies pass rate | Yes |
//! | `NoAllowGate` | Checks for `#[allow(...)]` annotations | Yes |
//! | `SecurityGate` | Runs security scans (cargo-audit) | Yes |
//! | `NoTodoGate` | Checks for TODO/FIXME comments | No |
//!
//! # Configuration
//!
//! Gates can be individually configured:
//!
//! ```rust,ignore
//! use ralph::quality::{EnforcerConfig, QualityGateEnforcer};
//!
//! let config = EnforcerConfig::new()
//!     .with_clippy(true)
//!     .with_tests(true)
//!     .with_no_allow(true)
//!     .with_security(false)  // Skip security for speed
//!     .with_fail_fast(true); // Stop on first failure
//!
//! let enforcer = QualityGateEnforcer::with_config("/path/to/project", config);
//! ```

pub mod enforcer;
pub mod gates;
pub mod plugin;
pub mod remediation;

// Re-export commonly used types from gates
pub use gates::{
    detect_changed_languages, ClippyConfig, ClippyGate, Gate, GateIssue, GateResult,
    GateWeightConfig, IssueSeverity, NoAllowGate, NoTodoGate, PolyglotGateResult, SecurityGate,
    TestConfig, TestGate,
};

// Re-export enforcer types
pub use enforcer::{EnforcerConfig, EnforcerSummary, QualityGateEnforcer};

// Re-export remediation types
pub use remediation::{
    generate_minimal_remediation, generate_remediation_prompt, RemediationConfig,
    RemediationGenerator,
};

// Re-export plugin types
pub use plugin::{
    GatePlugin, LibraryConfig, PluginConfig, PluginError, PluginExecutor, PluginManifest,
    PluginMetadata,
};
