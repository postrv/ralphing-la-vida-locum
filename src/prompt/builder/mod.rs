//! Dynamic prompt section generators and builder.
//!
//! This module provides functions to generate markdown sections from context types,
//! and a builder to assemble complete prompts.
//!
//! # Example
//!
//! ```
//! use ralph::prompt::builder::SectionBuilder;
//! use ralph::prompt::context::{CurrentTaskContext, TaskPhase};
//!
//! let task = CurrentTaskContext::new("2.1", "Create context types", TaskPhase::Testing);
//! let section = SectionBuilder::build_task_section(&task);
//! assert!(section.contains("2.1"));
//! ```

mod assembly;
pub mod sections;

// Re-export the DynamicPromptBuilder for public API
pub use assembly::DynamicPromptBuilder;

use crate::prompt::context::{
    AntiPattern, AttemptSummary, CallGraphNode, CodeIntelligenceContext, CurrentTaskContext,
    ErrorContext, ModuleDependency, QualityGateStatus, SessionStats, SymbolReference,
};

/// Section builder for generating markdown from context types.
///
/// Provides static methods to generate markdown sections for each context type.
/// This is a facade that delegates to the specialized section modules while
/// maintaining backward API compatibility.
pub struct SectionBuilder;

impl SectionBuilder {
    /// Build the task context section.
    ///
    /// # Example
    ///
    /// ```
    /// use ralph::prompt::builder::SectionBuilder;
    /// use ralph::prompt::context::{CurrentTaskContext, TaskPhase};
    ///
    /// let task = CurrentTaskContext::new("1.1", "Setup testing", TaskPhase::Implementation)
    ///     .with_completion(50)
    ///     .with_attempts(2);
    ///
    /// let section = SectionBuilder::build_task_section(&task);
    /// assert!(section.contains("## Current Task"));
    /// assert!(section.contains("1.1"));
    /// assert!(section.contains("50%"));
    /// ```
    #[must_use]
    pub fn build_task_section(task: &CurrentTaskContext) -> String {
        sections::build_task_section(task)
    }

    /// Build the error context section with frequency sorting.
    ///
    /// # Example
    ///
    /// ```
    /// use ralph::prompt::builder::SectionBuilder;
    /// use ralph::prompt::context::{ErrorContext, ErrorSeverity};
    ///
    /// let errors = vec![
    ///     ErrorContext::new("E0308", "mismatched types", ErrorSeverity::Error)
    ///         .with_occurrences(3),
    ///     ErrorContext::new("E0433", "unresolved", ErrorSeverity::Error),
    /// ];
    ///
    /// let section = SectionBuilder::build_error_section(&errors);
    /// assert!(section.contains("## Recent Errors"));
    /// assert!(section.contains("E0308"));
    /// ```
    #[must_use]
    pub fn build_error_section(errors: &[ErrorContext]) -> String {
        sections::build_error_section(errors)
    }

    /// Build the quality gate status section with status icons.
    ///
    /// # Example
    ///
    /// ```
    /// use ralph::prompt::builder::SectionBuilder;
    /// use ralph::prompt::context::{QualityGateStatus, GateResult};
    ///
    /// let status = QualityGateStatus::new()
    ///     .with_clippy(GateResult::pass())
    ///     .with_tests(GateResult::fail(vec!["test_foo failed".to_string()]))
    ///     .with_timestamp();
    ///
    /// let section = SectionBuilder::build_quality_section(&status);
    /// assert!(section.contains("## Quality Gates"));
    /// ```
    #[must_use]
    pub fn build_quality_section(status: &QualityGateStatus) -> String {
        sections::build_quality_section(status)
    }

    /// Build the session statistics section with budget warnings.
    ///
    /// # Example
    ///
    /// ```
    /// use ralph::prompt::builder::SectionBuilder;
    /// use ralph::prompt::context::SessionStats;
    ///
    /// let stats = SessionStats::new(8, 3, 250)
    ///     .with_budget(10)
    ///     .with_tasks_completed(2);
    ///
    /// let section = SectionBuilder::build_session_section(&stats);
    /// assert!(section.contains("## Session Progress"));
    /// assert!(section.contains("8"));
    /// ```
    #[must_use]
    pub fn build_session_section(stats: &SessionStats) -> String {
        sections::build_session_section(stats)
    }

    /// Build the attempt history section.
    ///
    /// # Example
    ///
    /// ```
    /// use ralph::prompt::builder::SectionBuilder;
    /// use ralph::prompt::context::{AttemptSummary, AttemptOutcome};
    ///
    /// let attempts = vec![
    ///     AttemptSummary::new(1, AttemptOutcome::TestFailure)
    ///         .with_approach("Direct implementation")
    ///         .with_error("test_foo failed"),
    ///     AttemptSummary::new(2, AttemptOutcome::CompilationError)
    ///         .with_approach("Added type annotation"),
    /// ];
    ///
    /// let section = SectionBuilder::build_attempt_section(&attempts);
    /// assert!(section.contains("## Previous Attempts"));
    /// assert!(section.contains("Attempt 1"));
    /// ```
    #[must_use]
    pub fn build_attempt_section(attempts: &[AttemptSummary]) -> String {
        sections::build_attempt_section(attempts)
    }

    /// Build the anti-pattern detection section.
    ///
    /// # Example
    ///
    /// ```
    /// use ralph::prompt::builder::SectionBuilder;
    /// use ralph::prompt::context::{AntiPattern, AntiPatternType};
    ///
    /// let patterns = vec![
    ///     AntiPattern::new(AntiPatternType::EditWithoutCommit, "5 files edited without commit")
    ///         .with_remediation("Make incremental commits"),
    /// ];
    ///
    /// let section = SectionBuilder::build_antipattern_section(&patterns);
    /// assert!(section.contains("Detected Anti-Patterns"));
    /// ```
    #[must_use]
    pub fn build_antipattern_section(patterns: &[AntiPattern]) -> String {
        sections::build_antipattern_section(patterns)
    }

    /// Build the historical guidance section.
    ///
    /// This takes pre-formatted guidance strings (will be populated by the history module).
    ///
    /// # Example
    ///
    /// ```
    /// use ralph::prompt::builder::SectionBuilder;
    ///
    /// let guidance = vec![
    ///     "Similar task completed successfully using TDD approach".to_string(),
    ///     "Avoid direct file manipulation - use abstractions".to_string(),
    /// ];
    ///
    /// let section = SectionBuilder::build_history_section(&guidance);
    /// assert!(section.contains("## Historical Guidance"));
    /// ```
    #[must_use]
    pub fn build_history_section(guidance: &[String]) -> String {
        sections::build_history_section(guidance)
    }

    /// Build the code intelligence section from narsil-mcp data.
    ///
    /// Shows call graph, references, and dependency information to provide
    /// context for implementation decisions.
    ///
    /// # Example
    ///
    /// ```
    /// use ralph::prompt::builder::SectionBuilder;
    /// use ralph::prompt::context::{CodeIntelligenceContext, CallGraphNode};
    ///
    /// let intel = CodeIntelligenceContext::new()
    ///     .with_call_graph(vec![CallGraphNode::new("process")])
    ///     .mark_available();
    ///
    /// let section = SectionBuilder::build_intelligence_section(&intel);
    /// assert!(section.contains("## Code Intelligence"));
    /// ```
    #[must_use]
    pub fn build_intelligence_section(intel: &CodeIntelligenceContext) -> String {
        sections::build_intelligence_section(intel)
    }

    /// Build the call graph section for the intelligence output.
    ///
    /// # Returns
    ///
    /// An empty string if the call graph is empty, otherwise a formatted
    /// markdown section showing hotspots first, then regular functions.
    ///
    /// # Example
    ///
    /// ```
    /// use ralph::prompt::builder::SectionBuilder;
    /// use ralph::prompt::context::CallGraphNode;
    ///
    /// let nodes = vec![CallGraphNode::new("process").with_file("src/lib.rs").with_line(10)];
    /// let section = SectionBuilder::build_call_graph_section(&nodes);
    /// assert!(section.contains("### Relevant Functions"));
    /// ```
    #[must_use]
    pub fn build_call_graph_section(call_graph: &[CallGraphNode]) -> String {
        sections::build_call_graph_section(call_graph)
    }

    /// Build the symbol references section for the intelligence output.
    ///
    /// # Returns
    ///
    /// An empty string if references are empty, otherwise a formatted
    /// markdown section with deduplicated symbols.
    ///
    /// # Example
    ///
    /// ```
    /// use ralph::prompt::builder::SectionBuilder;
    /// use ralph::prompt::context::SymbolReference;
    ///
    /// let refs = vec![SymbolReference::new("MyType", "lib.rs", 10)];
    /// let section = SectionBuilder::build_references_section(&refs);
    /// assert!(section.contains("### Symbol References"));
    /// ```
    #[must_use]
    pub fn build_references_section(references: &[SymbolReference]) -> String {
        sections::build_references_section(references)
    }

    /// Build the dependencies section for the intelligence output.
    ///
    /// # Returns
    ///
    /// An empty string if dependencies are empty, otherwise a formatted
    /// markdown section showing module paths, imports, and importers.
    ///
    /// # Example
    ///
    /// ```
    /// use ralph::prompt::builder::SectionBuilder;
    /// use ralph::prompt::context::ModuleDependency;
    ///
    /// let deps = vec![ModuleDependency::new("src/lib.rs").with_imports(vec!["std::io".into()])];
    /// let section = SectionBuilder::build_dependencies_section(&deps);
    /// assert!(section.contains("### Dependencies"));
    /// ```
    #[must_use]
    pub fn build_dependencies_section(dependencies: &[ModuleDependency]) -> String {
        sections::build_dependencies_section(dependencies)
    }

    /// Build a CCG (Compact Code Graph) section for prompt enrichment.
    ///
    /// Extracts key information from CCG L0 (manifest) and L1 (architecture)
    /// to provide architectural context in prompts. Keeps output concise.
    ///
    /// # Example
    ///
    /// ```
    /// use ralph::prompt::builder::SectionBuilder;
    /// use ralph::prompt::context::CodeIntelligenceContext;
    /// use ralph::narsil::{CcgManifest, SecuritySummary};
    ///
    /// let manifest = CcgManifest::new("my-project", "/path/to/project")
    ///     .with_counts(50, 200)
    ///     .with_security_summary(SecuritySummary { critical: 0, high: 0, medium: 1, low: 3 });
    ///
    /// let intel = CodeIntelligenceContext::new()
    ///     .with_ccg_manifest(manifest)
    ///     .mark_available();
    ///
    /// let section = SectionBuilder::build_ccg_section(&intel);
    /// assert!(section.contains("## Project Overview"));
    /// ```
    #[must_use]
    pub fn build_ccg_section(intel: &CodeIntelligenceContext) -> String {
        sections::build_ccg_section(intel)
    }

    /// Build a constraints section showing active code constraints.
    ///
    /// Displays constraints from CCG L2 that apply to the codebase. This helps
    /// the model understand architectural boundaries and quality requirements.
    ///
    /// # Example
    ///
    /// ```
    /// use ralph::prompt::builder::SectionBuilder;
    /// use ralph::prompt::context::CodeIntelligenceContext;
    /// use ralph::narsil::{ConstraintSet, CcgConstraint, ConstraintKind, ConstraintValue};
    ///
    /// let constraints = ConstraintSet::new()
    ///     .with_constraint(
    ///         CcgConstraint::new("max-complexity", ConstraintKind::MaxComplexity, "Keep functions simple")
    ///             .with_value(ConstraintValue::Number(10)),
    ///     );
    ///
    /// let intel = CodeIntelligenceContext::new()
    ///     .with_constraints(constraints)
    ///     .mark_available();
    ///
    /// let section = SectionBuilder::build_constraint_section(&intel);
    /// assert!(section.contains("Active Constraints"));
    /// ```
    #[must_use]
    pub fn build_constraint_section(intel: &CodeIntelligenceContext) -> String {
        sections::build_constraint_section(intel)
    }

    /// Build a constraint warning section for a specific target.
    ///
    /// Shows warnings for constraints that apply to a specific function or module.
    /// Use this when the user is working on constrained code.
    ///
    /// # Example
    ///
    /// ```
    /// use ralph::prompt::builder::SectionBuilder;
    /// use ralph::prompt::context::CodeIntelligenceContext;
    /// use ralph::narsil::{ConstraintSet, CcgConstraint, ConstraintKind, ConstraintValue};
    ///
    /// let constraints = ConstraintSet::new()
    ///     .with_constraint(
    ///         CcgConstraint::new("max-complexity", ConstraintKind::MaxComplexity, "Keep simple")
    ///             .with_target("process_request")
    ///             .with_value(ConstraintValue::Number(10)),
    ///     );
    ///
    /// let intel = CodeIntelligenceContext::new()
    ///     .with_constraints(constraints)
    ///     .mark_available();
    ///
    /// let warnings = SectionBuilder::build_constraint_warnings_for(&intel, "process_request");
    /// assert!(warnings.contains("maxComplexity"));
    /// ```
    #[must_use]
    pub fn build_constraint_warnings_for(intel: &CodeIntelligenceContext, target: &str) -> String {
        sections::build_constraint_warnings_for(intel, target)
    }

    /// Build a violations section showing constraint compliance failures.
    ///
    /// Displays violations from constraint verification, helping the model
    /// understand what code needs to be fixed to comply with constraints.
    ///
    /// # Example
    ///
    /// ```
    /// use ralph::prompt::builder::SectionBuilder;
    /// use ralph::prompt::context::CodeIntelligenceContext;
    /// use ralph::narsil::{ComplianceResult, ConstraintViolation};
    ///
    /// let result = ComplianceResult::failed(
    ///     vec![
    ///         ConstraintViolation::new("max-complexity", "process_data", "Complexity 15 exceeds max 10")
    ///     ],
    ///     1,
    /// );
    ///
    /// let intel = CodeIntelligenceContext::new()
    ///     .with_compliance_result(result)
    ///     .mark_available();
    ///
    /// let section = SectionBuilder::build_violations_section(&intel);
    /// assert!(section.contains("Constraint Violations"));
    /// assert!(section.contains("process_data"));
    /// ```
    #[must_use]
    pub fn build_violations_section(intel: &CodeIntelligenceContext) -> String {
        sections::build_violations_section(intel)
    }

    /// Build a combined intelligence section with both call graph and CCG data.
    ///
    /// This combines `build_intelligence_section` and `build_ccg_section` into
    /// a single coherent section, respecting the < 1KB size constraint.
    #[must_use]
    pub fn build_combined_intelligence_section(
        intel: &CodeIntelligenceContext,
        max_bytes: usize,
    ) -> String {
        sections::build_combined_intelligence_section(intel, max_bytes)
    }
}
