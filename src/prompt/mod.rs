//! Dynamic prompt generation module.
//!
//! This module contains components for generating context-aware prompts:
//!
//! - [`context`] - Prompt context aggregation and types
//! - [`templates`] - Base template system with markers
//! - [`builder`] - Dynamic section generators and prompt builder
//! - [`antipatterns`] - Anti-pattern detection
//! - [`assembler`] - High-level prompt assembly coordinator
//!
//! # Architecture
//!
//! The prompt module builds dynamic prompts by combining:
//! 1. Base templates from PROMPT_{mode}.md files
//! 2. Current task context from the task tracker
//! 3. Error context from recent failures
//! 4. Quality gate status from recent checks
//! 5. Historical patterns from the learning system
//!
//! # Example
//!
//! ```
//! use ralph::prompt::assembler::PromptAssembler;
//! use ralph::prompt::context::TaskPhase;
//!
//! let mut assembler = PromptAssembler::new();
//! assembler.set_current_task("2.1", "Implement feature", TaskPhase::Implementation);
//! assembler.update_session_stats(5, 2, 150);
//!
//! let prompt = assembler.build_prompt("build").expect("should build");
//! assert!(prompt.contains("Build Phase"));
//! ```
//!
//! # Module Structure
//!
//! ```text
//! ┌─────────────────────────────────────────────┐
//! │              PromptAssembler                │
//! │  (coordinates all components)               │
//! └─────────────────────┬───────────────────────┘
//!                       │
//!        ┌──────────────┼──────────────┐
//!        │              │              │
//!        v              v              v
//! ┌──────────────┐ ┌─────────┐ ┌─────────────────┐
//! │ DynamicPrompt│ │PromptTe-│ │ AntiPattern     │
//! │ Builder      │ │ mplates │ │ Detector        │
//! └──────┬───────┘ └────┬────┘ └────────┬────────┘
//!        │              │               │
//!        v              v               v
//! ┌──────────────┐ ┌─────────┐ ┌─────────────────┐
//! │ Section      │ │Template │ │ Iteration       │
//! │ Builder      │ │         │ │ Summary         │
//! └──────────────┘ └─────────┘ └─────────────────┘
//! ```

pub mod antipatterns;
pub mod assembler;
pub mod builder;
pub mod claude_md_generator;
pub mod code_antipatterns;
pub mod context;
pub mod templates;

// Re-export commonly used types for convenience
pub use antipatterns::{AntiPatternDetector, DetectorConfig, IterationSummary};
pub use assembler::{AssemblerConfig, PromptAssembler};
pub use builder::{DynamicPromptBuilder, SectionBuilder};
pub use code_antipatterns::{
    antipatterns_for_language, format_findings_for_prompt, language_from_extension,
    CodeAntipatternDetector, CodeAntipatternFinding, CodeAntipatternRule, CodeAntipatternSeverity,
};
pub use context::{
    AntiPattern, AntiPatternSeverity, AntiPatternType, AttemptOutcome, AttemptSummary,
    CurrentTaskContext, ErrorAggregator, ErrorContext, ErrorSeverity, GateResult, PromptContext,
    QualityGateStatus, SessionStats, TaskPhase,
};
pub use claude_md_generator::{
    extract_user_custom_sections, ClaudeMdGenerator, USER_CUSTOM_END, USER_CUSTOM_START,
};
pub use templates::{PromptTemplates, Template, TemplateMarker};
