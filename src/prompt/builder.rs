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

use crate::prompt::context::{
    AntiPattern, AntiPatternSeverity, AttemptOutcome, AttemptSummary, CallGraphNode,
    CodeIntelligenceContext, CurrentTaskContext, ErrorContext, PromptContext, QualityGateStatus,
    SessionStats,
};
use crate::prompt::templates::{PromptTemplates, TemplateMarker};
use std::collections::HashMap;

/// Section builder for generating markdown from context types.
///
/// Provides static methods to generate markdown sections for each context type.
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
        let mut lines = vec![
            "## Current Task".to_string(),
            String::new(),
            format!("**Task:** {} - {}", task.task_id, task.title),
            format!("**Phase:** {}", task.phase),
            format!("**Progress:** {}%", task.completion_percentage),
        ];

        if task.attempt_count > 0 {
            lines.push(format!("**Attempts:** {}", task.attempt_count));
        }

        if !task.modified_files.is_empty() {
            lines.push(String::new());
            lines.push("**Modified Files:**".to_string());
            for file in &task.modified_files {
                lines.push(format!("- `{}`", file));
            }
        }

        if !task.blockers.is_empty() {
            lines.push(String::new());
            lines.push("**⚠️ Blockers:**".to_string());
            for blocker in &task.blockers {
                lines.push(format!("- {}", blocker));
            }
        }

        if !task.dependencies.is_empty() {
            lines.push(String::new());
            lines.push("**Dependencies (must complete first):**".to_string());
            for dep in &task.dependencies {
                lines.push(format!("- {}", dep));
            }
        }

        lines.push(String::new());
        lines.join("\n")
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
        if errors.is_empty() {
            return String::new();
        }

        // Sort by occurrence count (most frequent first)
        let mut sorted: Vec<_> = errors.iter().collect();
        sorted.sort_by(|a, b| b.occurrence_count.cmp(&a.occurrence_count));

        let mut lines = vec![
            "## Recent Errors".to_string(),
            String::new(),
            format!("**Total unique errors:** {}", errors.len()),
            String::new(),
        ];

        for error in sorted.iter().take(10) {
            // Limit to top 10
            let severity_icon = match error.severity {
                crate::prompt::context::ErrorSeverity::Error => "🔴",
                crate::prompt::context::ErrorSeverity::Warning => "🟡",
                crate::prompt::context::ErrorSeverity::Info => "🔵",
            };

            let recurrence = if error.is_recurring() {
                format!(" (×{})", error.occurrence_count)
            } else {
                String::new()
            };

            lines.push(format!(
                "{} **{}**{}: {}",
                severity_icon, error.code, recurrence, error.message
            ));

            if let (Some(file), Some(line)) = (&error.file, error.line) {
                lines.push(format!("   📍 `{}:{}`", file, line));
            }

            if let Some(fix) = &error.suggested_fix {
                lines.push(format!("   💡 {}", fix));
            }
        }

        if errors.len() > 10 {
            lines.push(String::new());
            lines.push(format!("... and {} more errors", errors.len() - 10));
        }

        lines.push(String::new());
        lines.join("\n")
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
    /// assert!(section.contains("✅") || section.contains("❌"));
    /// ```
    #[must_use]
    pub fn build_quality_section(status: &QualityGateStatus) -> String {
        // Don't show if never checked
        if status.last_check.is_none() {
            return String::new();
        }

        let mut lines = vec!["## Quality Gates".to_string(), String::new()];

        // Clippy
        let clippy_icon = if status.clippy.passed { "✅" } else { "❌" };
        lines.push(format!("{} **Clippy**", clippy_icon));
        if !status.clippy.messages.is_empty() {
            for msg in status.clippy.messages.iter().take(3) {
                lines.push(format!("   - {}", msg));
            }
            if status.clippy.messages.len() > 3 {
                lines.push(format!(
                    "   ... and {} more",
                    status.clippy.messages.len() - 3
                ));
            }
        }

        // Tests
        let tests_icon = if status.tests.passed { "✅" } else { "❌" };
        lines.push(format!("{} **Tests**", tests_icon));
        if !status.tests.messages.is_empty() {
            for msg in status.tests.messages.iter().take(3) {
                lines.push(format!("   - {}", msg));
            }
            if status.tests.messages.len() > 3 {
                lines.push(format!(
                    "   ... and {} more",
                    status.tests.messages.len() - 3
                ));
            }
        }

        // No-allow annotations
        let no_allow_icon = if status.no_allow.passed { "✅" } else { "❌" };
        lines.push(format!("{} **No #[allow] annotations**", no_allow_icon));
        if !status.no_allow.messages.is_empty() {
            for msg in status.no_allow.messages.iter().take(3) {
                lines.push(format!("   - {}", msg));
            }
        }

        // Security
        let security_icon = if status.security.passed { "✅" } else { "❌" };
        lines.push(format!("{} **Security scan**", security_icon));
        if !status.security.messages.is_empty() {
            for msg in status.security.messages.iter().take(3) {
                lines.push(format!("   - {}", msg));
            }
        }

        // Docs
        let docs_icon = if status.docs.passed { "✅" } else { "❌" };
        lines.push(format!("{} **Documentation**", docs_icon));
        if !status.docs.messages.is_empty() {
            for msg in status.docs.messages.iter().take(3) {
                lines.push(format!("   - {}", msg));
            }
        }

        lines.push(String::new());
        lines.join("\n")
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
        let mut lines = vec!["## Session Progress".to_string(), String::new()];

        lines.push(format!("**Iterations:** {}", stats.iteration_count));
        lines.push(format!("**Commits:** {}", stats.commit_count));
        lines.push(format!("**Lines changed:** {}", stats.lines_changed));

        if stats.tasks_completed > 0 || stats.tasks_blocked > 0 {
            lines.push(format!(
                "**Tasks:** {} completed, {} blocked",
                stats.tasks_completed, stats.tasks_blocked
            ));
        }

        if stats.test_delta != 0 {
            let delta_str = if stats.test_delta > 0 {
                format!("+{}", stats.test_delta)
            } else {
                format!("{}", stats.test_delta)
            };
            lines.push(format!("**Test count delta:** {}", delta_str));
        }

        if let Some(budget_percent) = stats.budget_used_percent() {
            lines.push(String::new());

            if budget_percent >= 90 {
                lines.push(format!(
                    "🔴 **Budget critical:** {}% used ({}/{} iterations)",
                    budget_percent,
                    stats.iteration_count,
                    stats.max_iterations.unwrap_or(0)
                ));
                lines.push("   ⚠️ Prioritize completing current task or commit progress!".to_string());
            } else if budget_percent >= 80 {
                lines.push(format!(
                    "🟡 **Budget warning:** {}% used ({}/{} iterations)",
                    budget_percent,
                    stats.iteration_count,
                    stats.max_iterations.unwrap_or(0)
                ));
            } else {
                lines.push(format!(
                    "🟢 **Budget:** {}% used ({}/{} iterations)",
                    budget_percent,
                    stats.iteration_count,
                    stats.max_iterations.unwrap_or(0)
                ));
            }
        }

        if stats.stagnation_count > 0 {
            lines.push(String::new());
            if stats.stagnation_count >= 3 {
                lines.push(format!(
                    "🔴 **Stagnation alert:** {} iterations without progress",
                    stats.stagnation_count
                ));
                lines.push("   Consider: Is the current approach working? Try a different strategy.".to_string());
            } else {
                lines.push(format!(
                    "🟡 **Note:** {} iteration(s) without progress",
                    stats.stagnation_count
                ));
            }
        }

        if !stats.is_progressing() && stats.iteration_count > 3 {
            lines.push(String::new());
            lines.push("⚠️ **Low commit rate** - Consider making smaller, incremental commits.".to_string());
        }

        lines.push(String::new());
        lines.join("\n")
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
        if attempts.is_empty() {
            return String::new();
        }

        let mut lines = vec![
            "## Previous Attempts".to_string(),
            String::new(),
            "Learn from these previous attempts:".to_string(),
            String::new(),
        ];

        for attempt in attempts {
            let outcome_icon = match attempt.outcome {
                AttemptOutcome::Success => "✅",
                AttemptOutcome::CompilationError => "🔴",
                AttemptOutcome::TestFailure => "🟡",
                AttemptOutcome::QualityGateFailed => "🟠",
                AttemptOutcome::Timeout => "⏱️",
                AttemptOutcome::Blocked => "🚫",
                AttemptOutcome::Abandoned => "⚪",
            };

            lines.push(format!(
                "### Attempt {} - {} {}",
                attempt.attempt_number, outcome_icon, attempt.outcome
            ));

            if let Some(approach) = &attempt.approach {
                lines.push(format!("**Approach:** {}", approach));
            }

            if !attempt.errors.is_empty() {
                lines.push("**Errors encountered:**".to_string());
                for error in attempt.errors.iter().take(5) {
                    lines.push(format!("- {}", error));
                }
            }

            if !attempt.files_modified.is_empty() {
                lines.push(format!(
                    "**Files touched:** {}",
                    attempt.files_modified.join(", ")
                ));
            }

            if let Some(notes) = &attempt.notes {
                lines.push(format!("**Notes:** {}", notes));
            }

            lines.push(String::new());
        }

        // Add insights based on patterns
        let failed_attempts: Vec<_> = attempts.iter().filter(|a| !a.outcome.is_success()).collect();
        if failed_attempts.len() >= 2 {
            lines.push("### ⚠️ Pattern Analysis".to_string());

            // Check for repeated errors
            let mut error_counts: HashMap<&str, usize> = HashMap::new();
            for attempt in &failed_attempts {
                for error in &attempt.errors {
                    *error_counts.entry(error.as_str()).or_insert(0) += 1;
                }
            }

            let repeated: Vec<_> = error_counts
                .iter()
                .filter(|(_, count)| **count > 1)
                .collect();
            if !repeated.is_empty() {
                lines.push("**Recurring errors (try a different approach):**".to_string());
                for (error, count) in repeated.iter().take(3) {
                    lines.push(format!("- {} (×{})", error, count));
                }
            }

            lines.push(String::new());
        }

        lines.join("\n")
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
    /// assert!(section.contains("## ⚠️ Detected Anti-Patterns"));
    /// ```
    #[must_use]
    pub fn build_antipattern_section(patterns: &[AntiPattern]) -> String {
        if patterns.is_empty() {
            return String::new();
        }

        let mut lines = vec!["## ⚠️ Detected Anti-Patterns".to_string(), String::new()];

        // Sort by severity (high first)
        let mut sorted: Vec<_> = patterns.iter().collect();
        sorted.sort_by(|a, b| b.severity.cmp(&a.severity));

        for pattern in sorted {
            let severity_icon = match pattern.severity {
                AntiPatternSeverity::High => "🔴",
                AntiPatternSeverity::Medium => "🟡",
                AntiPatternSeverity::Low => "🔵",
            };

            lines.push(format!(
                "### {} {} ({})",
                severity_icon, pattern.pattern_type, pattern.severity
            ));
            lines.push(pattern.description.clone());

            if !pattern.evidence.is_empty() {
                lines.push("**Evidence:**".to_string());
                for evidence in pattern.evidence.iter().take(5) {
                    lines.push(format!("- {}", evidence));
                }
            }

            if let Some(remediation) = &pattern.remediation {
                lines.push(format!("**💡 Remediation:** {}", remediation));
            }

            if pattern.persistence_count > 1 {
                lines.push(format!(
                    "⚠️ This pattern has persisted for {} iterations",
                    pattern.persistence_count
                ));
            }

            lines.push(String::new());
        }

        lines.join("\n")
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
        if guidance.is_empty() {
            return String::new();
        }

        let mut lines = vec![
            "## Historical Guidance".to_string(),
            String::new(),
            "Based on previous sessions:".to_string(),
            String::new(),
        ];

        for item in guidance.iter().take(5) {
            lines.push(format!("- {}", item));
        }

        lines.push(String::new());
        lines.join("\n")
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
        // Show section if narsil-mcp is available with data, OR if we have constraints
        // (constraints can be loaded from project config without narsil-mcp)
        let has_narsil_data = intel.is_available
            && (!intel.call_graph.is_empty()
                || !intel.references.is_empty()
                || !intel.dependencies.is_empty());
        let has_constraints = intel.has_constraints();

        if !has_narsil_data && !has_constraints {
            return String::new();
        }

        let mut lines = vec!["## Code Intelligence".to_string(), String::new()];

        // Call graph section
        if !intel.call_graph.is_empty() {
            lines.push("### Relevant Functions".to_string());
            lines.push(String::new());

            // Show hotspots first with special formatting
            let hotspots: Vec<_> = intel.call_graph.iter().filter(|n| n.is_hotspot()).collect();
            if !hotspots.is_empty() {
                lines.push("**🔥 Hotspots (highly connected):**".to_string());
                for node in hotspots.iter().take(3) {
                    Self::format_call_graph_node(node, &mut lines, true);
                }
                lines.push(String::new());
            }

            // Show other functions
            let regular: Vec<_> = intel.call_graph.iter().filter(|n| !n.is_hotspot()).collect();
            if !regular.is_empty() {
                lines.push("**Functions:**".to_string());
                for node in regular.iter().take(5) {
                    Self::format_call_graph_node(node, &mut lines, false);
                }
            }

            if intel.call_graph.len() > 8 {
                lines.push(format!("... and {} more functions", intel.call_graph.len() - 8));
            }
            lines.push(String::new());
        }

        // References section
        if !intel.references.is_empty() {
            lines.push("### Symbol References".to_string());
            lines.push(String::new());

            // Group by symbol
            let mut symbols_seen: std::collections::HashSet<&str> = std::collections::HashSet::new();
            for reference in intel.references.iter().take(10) {
                if symbols_seen.insert(&reference.symbol) {
                    let location = format!("{}:{}", reference.file, reference.line);
                    let kind_str = format!(" ({})", reference.kind);
                    lines.push(format!("- `{}` at `{}`{}", reference.symbol, location, kind_str));
                }
            }

            if intel.references.len() > 10 {
                lines.push(format!("... and {} more references", intel.references.len() - 10));
            }
            lines.push(String::new());
        }

        // Dependencies section
        if !intel.dependencies.is_empty() {
            lines.push("### Dependencies".to_string());
            lines.push(String::new());

            for dep in intel.dependencies.iter().take(5) {
                lines.push(format!("**`{}`**", dep.module_path));
                if !dep.imports.is_empty() {
                    let imports_preview: Vec<_> = dep.imports.iter().take(5).collect();
                    let imports_str = imports_preview.iter().map(|s| s.as_str()).collect::<Vec<_>>().join(", ");
                    if dep.imports.len() > 5 {
                        lines.push(format!("  - Imports: {} ... (+{} more)", imports_str, dep.imports.len() - 5));
                    } else {
                        lines.push(format!("  - Imports: {}", imports_str));
                    }
                }
                if !dep.imported_by.is_empty() {
                    let importers_preview: Vec<_> = dep.imported_by.iter().take(3).collect();
                    let importers_str = importers_preview.iter().map(|s| s.as_str()).collect::<Vec<_>>().join(", ");
                    if dep.imported_by.len() > 3 {
                        lines.push(format!("  - Used by: {} ... (+{} more)", importers_str, dep.imported_by.len() - 3));
                    } else {
                        lines.push(format!("  - Used by: {}", importers_str));
                    }
                }
            }
            lines.push(String::new());
        }

        // Constraints section
        if intel.has_constraints() {
            let constraint_section = intel.constraints.to_prompt_section();
            if !constraint_section.is_empty() {
                lines.push(constraint_section);
                lines.push(String::new());
            }
        }

        // Violations section (if compliance result shows violations)
        if intel.has_violations() {
            let violations_section = Self::build_violations_section(intel);
            if !violations_section.is_empty() {
                lines.push(violations_section);
                lines.push(String::new());
            }
        }

        lines.join("\n")
    }

    /// Format a call graph node for display.
    fn format_call_graph_node(node: &CallGraphNode, lines: &mut Vec<String>, is_hotspot: bool) {
        let location = match (&node.file, node.line) {
            (Some(f), Some(l)) => format!(" (`{}:{}`)", f, l),
            (Some(f), None) => format!(" (`{}`)", f),
            _ => String::new(),
        };

        let prefix = if is_hotspot { "  🔥 " } else { "  - " };
        lines.push(format!("{}`{}`{}", prefix, node.function_name, location));

        // Show callers (limited)
        if !node.callers.is_empty() {
            let callers_preview: Vec<_> = node.callers.iter().take(5).collect();
            let callers_str = callers_preview.iter().map(|s| format!("`{}`", s)).collect::<Vec<_>>().join(", ");
            if node.callers.len() > 5 {
                lines.push(format!("    ← Called by: {} ... (+{} more)", callers_str, node.callers.len() - 5));
            } else {
                lines.push(format!("    ← Called by: {}", callers_str));
            }
        }

        // Show callees (limited)
        if !node.callees.is_empty() {
            let callees_preview: Vec<_> = node.callees.iter().take(5).collect();
            let callees_str = callees_preview.iter().map(|s| format!("`{}`", s)).collect::<Vec<_>>().join(", ");
            if node.callees.len() > 5 {
                lines.push(format!("    → Calls: {} ... (+{} more)", callees_str, node.callees.len() - 5));
            } else {
                lines.push(format!("    → Calls: {}", callees_str));
            }
        }
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
        // Don't show if not available or no CCG data
        if !intel.is_available || !intel.has_ccg_data() {
            return String::new();
        }

        let mut lines = Vec::new();

        // L0: Manifest section - basic project info
        if let Some(manifest) = &intel.ccg_manifest {
            lines.push("## Project Overview".to_string());
            lines.push(String::new());

            // Project name and primary language
            if let Some(lang) = &manifest.primary_language {
                lines.push(format!("**Project:** {} ({})", manifest.name, lang));
            } else {
                lines.push(format!("**Project:** {}", manifest.name));
            }

            // Symbol count
            lines.push(format!("**Symbols:** {} across {} files", manifest.symbol_count, manifest.file_count));

            // Security summary (if there are any issues)
            let sec = &manifest.security_summary;
            let total = sec.critical + sec.high + sec.medium + sec.low;
            if total > 0 {
                lines.push(String::new());
                lines.push("**Security Summary:**".to_string());
                if sec.critical > 0 {
                    lines.push(format!("  🔴 Critical: {}", sec.critical));
                }
                if sec.high > 0 {
                    lines.push(format!("  🟠 High: {}", sec.high));
                }
                if sec.medium > 0 {
                    lines.push(format!("  🟡 Medium: {}", sec.medium));
                }
                if sec.low > 0 {
                    lines.push(format!("  🔵 Low: {}", sec.low));
                }
            }

            lines.push(String::new());
        }

        // L1: Architecture section - module hierarchy and public API
        if let Some(arch) = &intel.ccg_architecture {
            // Entry points
            if !arch.entry_points.is_empty() {
                lines.push("### Entry Points".to_string());
                for entry in arch.entry_points.iter().take(5) {
                    let location = entry.file.display();
                    if let Some(line) = entry.line {
                        lines.push(format!("- `{}` ({}) at `{}:{}`", entry.name, entry.kind, location, line));
                    } else {
                        lines.push(format!("- `{}` ({}) in `{}`", entry.name, entry.kind, location));
                    }
                }
                if arch.entry_points.len() > 5 {
                    lines.push(format!("... and {} more entry points", arch.entry_points.len() - 5));
                }
                lines.push(String::new());
            }

            // Public API (limit to 8 symbols to stay concise)
            if !arch.public_api.is_empty() {
                lines.push("### Public API".to_string());
                for symbol in arch.public_api.iter().take(8) {
                    let kind_str = format!("{}", symbol.kind);
                    if let Some(sig) = &symbol.signature {
                        lines.push(format!("- `{}` ({}) - {}", symbol.name, kind_str, sig));
                    } else {
                        lines.push(format!("- `{}` ({})", symbol.name, kind_str));
                    }
                }
                if arch.public_api.len() > 8 {
                    lines.push(format!("... and {} more public symbols", arch.public_api.len() - 8));
                }
                lines.push(String::new());
            }

            // Module structure (limit to 5 modules)
            if !arch.modules.is_empty() {
                lines.push("### Module Structure".to_string());
                for module in arch.modules.iter().take(5) {
                    let children_count = module.children.len();
                    if children_count > 0 {
                        lines.push(format!("- `{}` ({} submodules)", module.name, children_count));
                    } else {
                        lines.push(format!("- `{}`", module.name));
                    }
                }
                if arch.modules.len() > 5 {
                    lines.push(format!("... and {} more modules", arch.modules.len() - 5));
                }
                lines.push(String::new());
            }
        }

        lines.join("\n")
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
        if !intel.is_available || !intel.has_constraints() {
            return String::new();
        }

        intel.constraints.to_prompt_section()
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
        if !intel.is_available || !intel.has_constraints() {
            return String::new();
        }

        let constraints = intel.constraints_for_target(target);
        if constraints.is_empty() {
            return String::new();
        }

        let mut lines = vec![
            format!("### Constraints for `{}`", target),
            String::new(),
        ];

        for constraint in constraints.iter().take(5) {
            lines.push(constraint.to_prompt_string());
        }

        if constraints.len() > 5 {
            lines.push(format!("\n*...and {} more constraints*", constraints.len() - 5));
        }

        // Add suggestion for constraint compliance
        lines.push(String::new());
        lines.push("**When modifying this code:**".to_string());
        lines.push("- Ensure changes comply with the above constraints".to_string());
        lines.push("- Use small, focused functions to maintain low complexity".to_string());
        lines.push("- Add tests to verify constraint compliance".to_string());

        lines.push(String::new());
        lines.join("\n")
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
        let Some(result) = intel.compliance() else {
            return String::new();
        };

        if result.compliant {
            return String::new();
        }

        let mut lines = vec![
            "### ⚠️ Constraint Violations".to_string(),
            String::new(),
            format!(
                "**{} violation(s)** found that need to be addressed:",
                result.violations.len()
            ),
            String::new(),
        ];

        for (i, violation) in result.violations.iter().enumerate().take(10) {
            let location = match (&violation.file, violation.line) {
                (Some(f), Some(l)) => format!(" at `{}:{}`", f, l),
                (Some(f), None) => format!(" in `{}`", f),
                _ => String::new(),
            };

            lines.push(format!(
                "{}. **`{}`**{}: {}",
                i + 1,
                violation.target,
                location,
                violation.message
            ));

            if let Some(ref suggestion) = violation.suggestion {
                lines.push(format!("   - 💡 *{}*", suggestion));
            }
        }

        if result.violations.len() > 10 {
            lines.push(format!(
                "\n*...and {} more violations*",
                result.violations.len() - 10
            ));
        }

        lines.push(String::new());
        lines.push("**Action required:** Fix these violations before proceeding.".to_string());
        lines.push(String::new());

        lines.join("\n")
    }

    /// Build a combined intelligence section with both call graph and CCG data.
    ///
    /// This combines `build_intelligence_section` and `build_ccg_section` into
    /// a single coherent section, respecting the < 1KB size constraint.
    #[must_use]
    pub fn build_combined_intelligence_section(intel: &CodeIntelligenceContext, max_bytes: usize) -> String {
        if !intel.is_available {
            return String::new();
        }

        let mut sections = Vec::new();

        // Add CCG section first (overview)
        let ccg_section = Self::build_ccg_section(intel);
        if !ccg_section.is_empty() {
            sections.push(ccg_section);
        }

        // Add constraints section (important for guiding implementation)
        let constraint_section = Self::build_constraint_section(intel);
        if !constraint_section.is_empty() {
            sections.push(constraint_section);
        }

        // Add intelligence section (call graph, refs, deps)
        let intel_section = Self::build_intelligence_section(intel);
        if !intel_section.is_empty() {
            sections.push(intel_section);
        }

        let combined = sections.join("\n");

        // Enforce size limit
        if combined.len() > max_bytes {
            // Truncate with indicator
            let truncated = &combined[..max_bytes.saturating_sub(50)];
            // Find last newline to avoid cutting mid-line
            if let Some(pos) = truncated.rfind('\n') {
                format!("{}\n\n... (intelligence section truncated to {}B)", &combined[..pos], max_bytes)
            } else {
                format!("{}... (truncated)", &combined[..max_bytes.saturating_sub(20)])
            }
        } else {
            combined
        }
    }
}

/// Dynamic prompt builder that assembles complete prompts from templates and context.
///
/// # Example
///
/// ```
/// use ralph::prompt::builder::DynamicPromptBuilder;
/// use ralph::prompt::context::{PromptContext, SessionStats};
/// use ralph::prompt::templates::PromptTemplates;
///
/// let templates = PromptTemplates::with_defaults();
/// let builder = DynamicPromptBuilder::new(templates);
///
/// let context = PromptContext::new()
///     .with_session_stats(SessionStats::new(5, 2, 150));
///
/// let prompt = builder.build("build", &context);
/// assert!(prompt.is_ok());
/// ```
#[derive(Debug)]
pub struct DynamicPromptBuilder {
    templates: PromptTemplates,
}

impl DynamicPromptBuilder {
    /// Create a new builder with the given templates.
    #[must_use]
    pub fn new(templates: PromptTemplates) -> Self {
        Self { templates }
    }

    /// Build a complete prompt for the given mode and context.
    ///
    /// # Errors
    ///
    /// Returns an error if the template for the given mode doesn't exist.
    ///
    /// # Example
    ///
    /// ```
    /// use ralph::prompt::builder::DynamicPromptBuilder;
    /// use ralph::prompt::context::PromptContext;
    /// use ralph::prompt::templates::PromptTemplates;
    ///
    /// let templates = PromptTemplates::with_defaults();
    /// let builder = DynamicPromptBuilder::new(templates);
    ///
    /// let result = builder.build("build", &PromptContext::new());
    /// assert!(result.is_ok());
    /// ```
    pub fn build(&self, mode: &str, context: &PromptContext) -> anyhow::Result<String> {
        let template = self
            .templates
            .get_template(mode)
            .ok_or_else(|| anyhow::anyhow!("Template not found for mode: {}", mode))?;

        let mut substitutions = HashMap::new();

        // Build each section and add to substitutions
        if let Some(task) = &context.current_task {
            let section = SectionBuilder::build_task_section(task);
            substitutions.insert(TemplateMarker::TaskContext, section);
        } else {
            substitutions.insert(TemplateMarker::TaskContext, String::new());
        }

        let error_section = SectionBuilder::build_error_section(&context.errors);
        substitutions.insert(TemplateMarker::ErrorContext, error_section);

        let quality_section = SectionBuilder::build_quality_section(&context.quality_status);
        substitutions.insert(TemplateMarker::QualityStatus, quality_section);

        let session_section = SectionBuilder::build_session_section(&context.session_stats);
        substitutions.insert(TemplateMarker::SessionStats, session_section);

        let attempt_section = SectionBuilder::build_attempt_section(&context.attempt_summaries);
        substitutions.insert(TemplateMarker::AttemptHistory, attempt_section);

        let antipattern_section =
            SectionBuilder::build_antipattern_section(&context.anti_patterns);
        substitutions.insert(TemplateMarker::AntiPatterns, antipattern_section);

        // Code intelligence section
        let intelligence_section =
            SectionBuilder::build_intelligence_section(&context.code_intelligence);
        substitutions.insert(TemplateMarker::CodeIntelligence, intelligence_section);

        // Historical guidance placeholder (populated by history module)
        substitutions.insert(TemplateMarker::HistoricalGuidance, String::new());

        // Custom section placeholder
        substitutions.insert(TemplateMarker::CustomSection, String::new());

        // Apply all substitutions
        let substitution_refs: HashMap<TemplateMarker, &str> = substitutions
            .iter()
            .map(|(k, v)| (*k, v.as_str()))
            .collect();

        let result = template.substitute_all(&substitution_refs);

        // Clean up any remaining markers and extra newlines
        let cleaned = result.remove_unreplaced_markers();

        Ok(cleaned.content().to_string())
    }

    /// Build a prompt with custom sections.
    ///
    /// Allows adding additional sections not covered by the standard context.
    pub fn build_with_custom(
        &self,
        mode: &str,
        context: &PromptContext,
        custom_sections: &HashMap<TemplateMarker, String>,
    ) -> anyhow::Result<String> {
        let template = self
            .templates
            .get_template(mode)
            .ok_or_else(|| anyhow::anyhow!("Template not found for mode: {}", mode))?;

        let mut substitutions = HashMap::new();

        // Build standard sections
        if let Some(task) = &context.current_task {
            substitutions.insert(TemplateMarker::TaskContext, SectionBuilder::build_task_section(task));
        } else {
            substitutions.insert(TemplateMarker::TaskContext, String::new());
        }

        substitutions.insert(
            TemplateMarker::ErrorContext,
            SectionBuilder::build_error_section(&context.errors),
        );
        substitutions.insert(
            TemplateMarker::QualityStatus,
            SectionBuilder::build_quality_section(&context.quality_status),
        );
        substitutions.insert(
            TemplateMarker::SessionStats,
            SectionBuilder::build_session_section(&context.session_stats),
        );
        substitutions.insert(
            TemplateMarker::AttemptHistory,
            SectionBuilder::build_attempt_section(&context.attempt_summaries),
        );
        substitutions.insert(
            TemplateMarker::AntiPatterns,
            SectionBuilder::build_antipattern_section(&context.anti_patterns),
        );
        substitutions.insert(
            TemplateMarker::CodeIntelligence,
            SectionBuilder::build_intelligence_section(&context.code_intelligence),
        );
        substitutions.insert(TemplateMarker::HistoricalGuidance, String::new());
        substitutions.insert(TemplateMarker::CustomSection, String::new());

        // Override with custom sections
        for (marker, content) in custom_sections {
            substitutions.insert(*marker, content.clone());
        }

        let substitution_refs: HashMap<TemplateMarker, &str> = substitutions
            .iter()
            .map(|(k, v)| (*k, v.as_str()))
            .collect();

        let result = template.substitute_all(&substitution_refs);
        let cleaned = result.remove_unreplaced_markers();

        Ok(cleaned.content().to_string())
    }

    /// Get a reference to the templates.
    #[must_use]
    pub fn templates(&self) -> &PromptTemplates {
        &self.templates
    }
}

impl Default for DynamicPromptBuilder {
    fn default() -> Self {
        Self::new(PromptTemplates::with_defaults())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::prompt::context::*;

    // SectionBuilder::build_task_section tests

    #[test]
    fn test_build_task_section_basic() {
        let task = CurrentTaskContext::new("2.1", "Create context types", TaskPhase::Implementation);
        let section = SectionBuilder::build_task_section(&task);

        assert!(section.contains("## Current Task"));
        assert!(section.contains("2.1"));
        assert!(section.contains("Create context types"));
        assert!(section.contains("Implementation"));
        assert!(section.contains("0%")); // Default completion
    }

    #[test]
    fn test_build_task_section_with_progress() {
        let task = CurrentTaskContext::new("1.1", "Task", TaskPhase::Testing)
            .with_completion(75)
            .with_attempts(3);

        let section = SectionBuilder::build_task_section(&task);

        assert!(section.contains("75%"));
        assert!(section.contains("**Attempts:** 3"));
    }

    #[test]
    fn test_build_task_section_with_files() {
        let task = CurrentTaskContext::new("1.1", "Task", TaskPhase::Implementation)
            .with_modified_files(vec!["src/lib.rs".to_string(), "src/main.rs".to_string()]);

        let section = SectionBuilder::build_task_section(&task);

        assert!(section.contains("**Modified Files:**"));
        assert!(section.contains("`src/lib.rs`"));
        assert!(section.contains("`src/main.rs`"));
    }

    #[test]
    fn test_build_task_section_with_blockers() {
        let task = CurrentTaskContext::new("1.1", "Task", TaskPhase::Implementation)
            .with_blockers(vec!["Dependency not available".to_string()]);

        let section = SectionBuilder::build_task_section(&task);

        assert!(section.contains("⚠️ Blockers:"));
        assert!(section.contains("Dependency not available"));
    }

    #[test]
    fn test_build_task_section_with_dependencies() {
        let task = CurrentTaskContext::new("1.2", "Task", TaskPhase::Implementation)
            .with_dependencies(vec!["1.1".to_string()]);

        let section = SectionBuilder::build_task_section(&task);

        assert!(section.contains("Dependencies (must complete first):"));
        assert!(section.contains("1.1"));
    }

    // SectionBuilder::build_error_section tests

    #[test]
    fn test_build_error_section_empty() {
        let errors: Vec<ErrorContext> = vec![];
        let section = SectionBuilder::build_error_section(&errors);
        assert!(section.is_empty());
    }

    #[test]
    fn test_build_error_section_single() {
        let errors = vec![ErrorContext::new(
            "E0308",
            "mismatched types",
            ErrorSeverity::Error,
        )];
        let section = SectionBuilder::build_error_section(&errors);

        assert!(section.contains("## Recent Errors"));
        assert!(section.contains("E0308"));
        assert!(section.contains("mismatched types"));
        assert!(section.contains("🔴")); // Error icon
    }

    #[test]
    fn test_build_error_section_with_location() {
        let errors = vec![
            ErrorContext::new("E0308", "error", ErrorSeverity::Error)
                .with_location("src/lib.rs", 42),
        ];
        let section = SectionBuilder::build_error_section(&errors);

        assert!(section.contains("📍 `src/lib.rs:42`"));
    }

    #[test]
    fn test_build_error_section_with_suggested_fix() {
        let errors = vec![
            ErrorContext::new("E0308", "error", ErrorSeverity::Error)
                .with_suggested_fix("Change type to String"),
        ];
        let section = SectionBuilder::build_error_section(&errors);

        assert!(section.contains("💡 Change type to String"));
    }

    #[test]
    fn test_build_error_section_recurring() {
        let errors = vec![
            ErrorContext::new("E0308", "error", ErrorSeverity::Error).with_occurrences(5),
        ];
        let section = SectionBuilder::build_error_section(&errors);

        assert!(section.contains("(×5)")); // Recurrence indicator
    }

    #[test]
    fn test_build_error_section_sorted_by_frequency() {
        let errors = vec![
            ErrorContext::new("E0001", "less frequent", ErrorSeverity::Error).with_occurrences(1),
            ErrorContext::new("E0002", "most frequent", ErrorSeverity::Error).with_occurrences(10),
            ErrorContext::new("E0003", "medium", ErrorSeverity::Error).with_occurrences(5),
        ];
        let section = SectionBuilder::build_error_section(&errors);

        // E0002 should appear before E0003 which should appear before E0001
        let e1_pos = section.find("E0001").unwrap();
        let e2_pos = section.find("E0002").unwrap();
        let e3_pos = section.find("E0003").unwrap();

        assert!(e2_pos < e3_pos);
        assert!(e3_pos < e1_pos);
    }

    #[test]
    fn test_build_error_section_severity_icons() {
        let errors = vec![
            ErrorContext::new("E0001", "error", ErrorSeverity::Error),
            ErrorContext::new("W0001", "warning", ErrorSeverity::Warning),
            ErrorContext::new("I0001", "info", ErrorSeverity::Info),
        ];
        let section = SectionBuilder::build_error_section(&errors);

        assert!(section.contains("🔴")); // Error
        assert!(section.contains("🟡")); // Warning
        assert!(section.contains("🔵")); // Info
    }

    // SectionBuilder::build_quality_section tests

    #[test]
    fn test_build_quality_section_unchecked() {
        let status = QualityGateStatus::new(); // No timestamp
        let section = SectionBuilder::build_quality_section(&status);
        assert!(section.is_empty());
    }

    #[test]
    fn test_build_quality_section_all_passing() {
        let status = QualityGateStatus::all_passing();
        let section = SectionBuilder::build_quality_section(&status);

        assert!(section.contains("## Quality Gates"));
        assert!(section.contains("✅ **Clippy**"));
        assert!(section.contains("✅ **Tests**"));
    }

    #[test]
    fn test_build_quality_section_with_failures() {
        let status = QualityGateStatus::new()
            .with_clippy(GateResult::pass())
            .with_tests(GateResult::fail(vec!["test_foo failed".to_string()]))
            .with_no_allow(GateResult::pass())
            .with_security(GateResult::pass())
            .with_docs(GateResult::pass())
            .with_timestamp();

        let section = SectionBuilder::build_quality_section(&status);

        assert!(section.contains("✅ **Clippy**"));
        assert!(section.contains("❌ **Tests**"));
        assert!(section.contains("test_foo failed"));
    }

    // SectionBuilder::build_session_section tests

    #[test]
    fn test_build_session_section_basic() {
        let stats = SessionStats::new(5, 2, 150);
        let section = SectionBuilder::build_session_section(&stats);

        assert!(section.contains("## Session Progress"));
        assert!(section.contains("**Iterations:** 5"));
        assert!(section.contains("**Commits:** 2"));
        assert!(section.contains("**Lines changed:** 150"));
    }

    #[test]
    fn test_build_session_section_with_budget() {
        let stats = SessionStats::new(8, 3, 200).with_budget(10);
        let section = SectionBuilder::build_session_section(&stats);

        assert!(section.contains("80%")); // 8/10 = 80%
        assert!(section.contains("🟡")); // Warning (>80%)
    }

    #[test]
    fn test_build_session_section_budget_critical() {
        let stats = SessionStats::new(9, 3, 200).with_budget(10);
        let section = SectionBuilder::build_session_section(&stats);

        assert!(section.contains("🔴")); // Critical
        assert!(section.contains("Budget critical"));
    }

    #[test]
    fn test_build_session_section_with_stagnation() {
        let stats = SessionStats::new(10, 2, 100).with_stagnation(4);
        let section = SectionBuilder::build_session_section(&stats);

        assert!(section.contains("Stagnation alert"));
        assert!(section.contains("4 iterations without progress"));
    }

    #[test]
    fn test_build_session_section_low_commit_rate() {
        let stats = SessionStats::new(10, 1, 100); // 1 commit in 10 iterations
        let section = SectionBuilder::build_session_section(&stats);

        assert!(section.contains("Low commit rate"));
    }

    #[test]
    fn test_build_session_section_test_delta() {
        let stats = SessionStats::new(5, 2, 100).with_test_delta(5);
        let section = SectionBuilder::build_session_section(&stats);

        assert!(section.contains("**Test count delta:** +5"));

        let stats_neg = SessionStats::new(5, 2, 100).with_test_delta(-3);
        let section_neg = SectionBuilder::build_session_section(&stats_neg);

        assert!(section_neg.contains("**Test count delta:** -3"));
    }

    // SectionBuilder::build_attempt_section tests

    #[test]
    fn test_build_attempt_section_empty() {
        let attempts: Vec<AttemptSummary> = vec![];
        let section = SectionBuilder::build_attempt_section(&attempts);
        assert!(section.is_empty());
    }

    #[test]
    fn test_build_attempt_section_single() {
        let attempts = vec![AttemptSummary::new(1, AttemptOutcome::TestFailure)
            .with_approach("TDD approach")
            .with_error("test_foo failed")];

        let section = SectionBuilder::build_attempt_section(&attempts);

        assert!(section.contains("## Previous Attempts"));
        assert!(section.contains("### Attempt 1"));
        assert!(section.contains("Test Failure"));
        assert!(section.contains("TDD approach"));
        assert!(section.contains("test_foo failed"));
    }

    #[test]
    fn test_build_attempt_section_outcome_icons() {
        let attempts = vec![
            AttemptSummary::new(1, AttemptOutcome::Success),
            AttemptSummary::new(2, AttemptOutcome::CompilationError),
            AttemptSummary::new(3, AttemptOutcome::TestFailure),
        ];

        let section = SectionBuilder::build_attempt_section(&attempts);

        assert!(section.contains("✅")); // Success
        assert!(section.contains("🔴")); // CompilationError
        assert!(section.contains("🟡")); // TestFailure
    }

    #[test]
    fn test_build_attempt_section_pattern_analysis() {
        let attempts = vec![
            AttemptSummary::new(1, AttemptOutcome::TestFailure)
                .with_error("recurring error"),
            AttemptSummary::new(2, AttemptOutcome::TestFailure)
                .with_error("recurring error"),
        ];

        let section = SectionBuilder::build_attempt_section(&attempts);

        assert!(section.contains("Pattern Analysis"));
        assert!(section.contains("Recurring errors"));
    }

    // SectionBuilder::build_antipattern_section tests

    #[test]
    fn test_build_antipattern_section_empty() {
        let patterns: Vec<AntiPattern> = vec![];
        let section = SectionBuilder::build_antipattern_section(&patterns);
        assert!(section.is_empty());
    }

    #[test]
    fn test_build_antipattern_section_single() {
        let patterns = vec![
            AntiPattern::new(AntiPatternType::EditWithoutCommit, "5 files edited")
                .with_remediation("Make incremental commits"),
        ];

        let section = SectionBuilder::build_antipattern_section(&patterns);

        assert!(section.contains("## ⚠️ Detected Anti-Patterns"));
        assert!(section.contains("Edit Without Commit"));
        assert!(section.contains("5 files edited"));
        assert!(section.contains("Make incremental commits"));
    }

    #[test]
    fn test_build_antipattern_section_severity_sorted() {
        let patterns = vec![
            AntiPattern::new(AntiPatternType::ClippyNotRun, "low").with_severity(AntiPatternSeverity::Low),
            AntiPattern::new(AntiPatternType::TaskOscillation, "high")
                .with_severity(AntiPatternSeverity::High),
            AntiPattern::new(AntiPatternType::TestsNotRun, "medium")
                .with_severity(AntiPatternSeverity::Medium),
        ];

        let section = SectionBuilder::build_antipattern_section(&patterns);

        // High should appear before Medium which should appear before Low
        let high_pos = section.find("Task Oscillation").unwrap();
        let medium_pos = section.find("Tests Not Run").unwrap();
        let low_pos = section.find("Clippy Not Run").unwrap();

        assert!(high_pos < medium_pos);
        assert!(medium_pos < low_pos);
    }

    #[test]
    fn test_build_antipattern_section_with_evidence() {
        let patterns = vec![
            AntiPattern::new(AntiPatternType::RepeatingErrors, "Same error 3x")
                .with_evidence(vec!["E0308 at line 10".to_string()]),
        ];

        let section = SectionBuilder::build_antipattern_section(&patterns);

        assert!(section.contains("**Evidence:**"));
        assert!(section.contains("E0308 at line 10"));
    }

    #[test]
    fn test_build_antipattern_section_persistence() {
        let patterns = vec![
            AntiPattern::new(AntiPatternType::FileChurn, "Churn detected").with_persistence(5),
        ];

        let section = SectionBuilder::build_antipattern_section(&patterns);

        assert!(section.contains("persisted for 5 iterations"));
    }

    // SectionBuilder::build_history_section tests

    #[test]
    fn test_build_history_section_empty() {
        let guidance: Vec<String> = vec![];
        let section = SectionBuilder::build_history_section(&guidance);
        assert!(section.is_empty());
    }

    #[test]
    fn test_build_history_section_with_items() {
        let guidance = vec![
            "TDD approach worked well".to_string(),
            "Avoid direct manipulation".to_string(),
        ];

        let section = SectionBuilder::build_history_section(&guidance);

        assert!(section.contains("## Historical Guidance"));
        assert!(section.contains("TDD approach worked well"));
        assert!(section.contains("Avoid direct manipulation"));
    }

    // DynamicPromptBuilder tests

    #[test]
    fn test_dynamic_prompt_builder_new() {
        let builder = DynamicPromptBuilder::default();
        assert!(builder.templates().has_template("build"));
        assert!(builder.templates().has_template("debug"));
        assert!(builder.templates().has_template("plan"));
    }

    #[test]
    fn test_dynamic_prompt_builder_build_basic() {
        let builder = DynamicPromptBuilder::default();
        let context = PromptContext::new();

        let result = builder.build("build", &context);
        assert!(result.is_ok());

        let prompt = result.unwrap();
        assert!(prompt.contains("Build Phase"));
    }

    #[test]
    fn test_dynamic_prompt_builder_build_with_task() {
        let builder = DynamicPromptBuilder::default();
        let context = PromptContext::new().with_current_task(CurrentTaskContext::new(
            "2.1",
            "Test task",
            TaskPhase::Implementation,
        ));

        let result = builder.build("build", &context);
        assert!(result.is_ok());

        let prompt = result.unwrap();
        assert!(prompt.contains("2.1"));
        assert!(prompt.contains("Test task"));
    }

    #[test]
    fn test_dynamic_prompt_builder_build_with_errors() {
        let builder = DynamicPromptBuilder::default();
        let context = PromptContext::new()
            .with_error(ErrorContext::new("E0308", "mismatched", ErrorSeverity::Error));

        let result = builder.build("build", &context);
        assert!(result.is_ok());

        let prompt = result.unwrap();
        assert!(prompt.contains("E0308"));
    }

    #[test]
    fn test_dynamic_prompt_builder_build_unknown_mode() {
        let builder = DynamicPromptBuilder::default();
        let context = PromptContext::new();

        let result = builder.build("unknown", &context);
        assert!(result.is_err());
    }

    #[test]
    fn test_dynamic_prompt_builder_build_debug_mode() {
        let builder = DynamicPromptBuilder::default();
        let context = PromptContext::new();

        let result = builder.build("debug", &context);
        assert!(result.is_ok());

        let prompt = result.unwrap();
        assert!(prompt.contains("Debug Phase"));
    }

    #[test]
    fn test_dynamic_prompt_builder_build_plan_mode() {
        let builder = DynamicPromptBuilder::default();
        let context = PromptContext::new();

        let result = builder.build("plan", &context);
        assert!(result.is_ok());

        let prompt = result.unwrap();
        assert!(prompt.contains("Plan Phase"));
    }

    #[test]
    fn test_dynamic_prompt_builder_removes_unreplaced_markers() {
        let builder = DynamicPromptBuilder::default();
        let context = PromptContext::new(); // No errors, no task

        let result = builder.build("build", &context);
        assert!(result.is_ok());

        let prompt = result.unwrap();
        // Should not contain raw markers
        assert!(!prompt.contains("{{TASK_CONTEXT}}"));
        assert!(!prompt.contains("{{ERROR_CONTEXT}}"));
    }

    #[test]
    fn test_dynamic_prompt_builder_with_custom_sections() {
        let builder = DynamicPromptBuilder::default();
        let context = PromptContext::new();

        let mut custom = HashMap::new();
        custom.insert(
            TemplateMarker::CustomSection,
            "## Custom\n\nCustom content".to_string(),
        );

        let result = builder.build_with_custom("build", &context, &custom);
        assert!(result.is_ok());
    }

    // ==========================================================================
    // SectionBuilder::build_intelligence_section tests
    // ==========================================================================

    #[test]
    fn test_build_intelligence_section_empty() {
        let intel = CodeIntelligenceContext::new();
        let section = SectionBuilder::build_intelligence_section(&intel);
        assert!(section.is_empty());
    }

    #[test]
    fn test_build_intelligence_section_unavailable() {
        // Even with data, if not marked available, should be empty
        let intel = CodeIntelligenceContext::new()
            .with_call_graph(vec![CallGraphNode::new("foo")]);
        let section = SectionBuilder::build_intelligence_section(&intel);
        assert!(section.is_empty());
    }

    #[test]
    fn test_build_intelligence_section_with_call_graph() {
        let intel = CodeIntelligenceContext::new()
            .with_call_graph(vec![
                CallGraphNode::new("process_request")
                    .with_file("src/handler.rs")
                    .with_callers(vec!["main".to_string(), "handle_http".to_string()])
                    .with_callees(vec!["validate".to_string()]),
            ])
            .mark_available();

        let section = SectionBuilder::build_intelligence_section(&intel);

        assert!(section.contains("## Code Intelligence"));
        assert!(section.contains("process_request"));
        assert!(section.contains("main"));
        assert!(section.contains("validate"));
    }

    #[test]
    fn test_build_intelligence_section_with_references() {
        let intel = CodeIntelligenceContext::new()
            .with_references(vec![
                SymbolReference::new("MyStruct", "src/lib.rs", 42)
                    .with_kind(ReferenceKind::Definition),
                SymbolReference::new("MyStruct", "src/main.rs", 10)
                    .with_kind(ReferenceKind::Usage),
            ])
            .mark_available();

        let section = SectionBuilder::build_intelligence_section(&intel);

        assert!(section.contains("## Code Intelligence"));
        assert!(section.contains("MyStruct"));
        assert!(section.contains("src/lib.rs:42"));
    }

    #[test]
    fn test_build_intelligence_section_with_dependencies() {
        let intel = CodeIntelligenceContext::new()
            .with_dependencies(vec![
                ModuleDependency::new("src/lib.rs")
                    .with_imports(vec!["std::io".to_string(), "crate::util".to_string()])
                    .with_imported_by(vec!["src/main.rs".to_string()]),
            ])
            .mark_available();

        let section = SectionBuilder::build_intelligence_section(&intel);

        assert!(section.contains("## Code Intelligence"));
        assert!(section.contains("Dependencies"));
        assert!(section.contains("src/lib.rs"));
    }

    #[test]
    fn test_build_intelligence_section_hotspots() {
        let intel = CodeIntelligenceContext::new()
            .with_call_graph(vec![
                CallGraphNode::new("hotspot_func")
                    .with_callers(vec!["a".into(), "b".into(), "c".into(), "d".into(), "e".into()])
                    .with_callees(vec!["x".into()]),
            ])
            .mark_available();

        let section = SectionBuilder::build_intelligence_section(&intel);

        assert!(section.contains("hotspot") || section.contains("Hotspot"));
    }

    #[test]
    fn test_build_intelligence_section_truncates_long_lists() {
        let callers: Vec<String> = (0..20).map(|i| format!("caller_{}", i)).collect();
        let intel = CodeIntelligenceContext::new()
            .with_call_graph(vec![
                CallGraphNode::new("popular_func")
                    .with_callers(callers),
            ])
            .mark_available();

        let section = SectionBuilder::build_intelligence_section(&intel);

        // Should truncate long lists
        assert!(section.contains("...") || section.contains("more"));
    }

    // ==========================================================================
    // DynamicPromptBuilder with code intelligence tests
    // ==========================================================================

    #[test]
    fn test_dynamic_prompt_builder_with_code_intelligence() {
        let builder = DynamicPromptBuilder::default();
        let intel = CodeIntelligenceContext::new()
            .with_call_graph(vec![CallGraphNode::new("test_func")])
            .mark_available();
        let context = PromptContext::new().with_code_intelligence(intel);

        let result = builder.build("build", &context);
        assert!(result.is_ok());

        let prompt = result.unwrap();
        assert!(prompt.contains("test_func") || prompt.contains("Code Intelligence"));
    }

    // ==========================================================================
    // SectionBuilder constraint section tests
    // ==========================================================================

    #[test]
    fn test_build_constraint_section_empty() {
        let intel = CodeIntelligenceContext::new().mark_available();
        let section = SectionBuilder::build_constraint_section(&intel);
        assert!(section.is_empty());
    }

    #[test]
    fn test_build_constraint_section_not_available() {
        use crate::narsil::{CcgConstraint, ConstraintKind, ConstraintSet, ConstraintValue};

        let constraints = ConstraintSet::new().with_constraint(
            CcgConstraint::new("c1", ConstraintKind::MaxComplexity, "Test")
                .with_value(ConstraintValue::Number(10)),
        );

        // Not marked as available - should return empty
        let intel = CodeIntelligenceContext::new().with_constraints(constraints);
        let section = SectionBuilder::build_constraint_section(&intel);
        assert!(section.is_empty());
    }

    #[test]
    fn test_build_constraint_section_with_constraints() {
        use crate::narsil::{CcgConstraint, ConstraintKind, ConstraintSet, ConstraintValue};

        let constraints = ConstraintSet::new()
            .with_constraint(
                CcgConstraint::new("max-complexity", ConstraintKind::MaxComplexity, "Keep it simple")
                    .with_value(ConstraintValue::Number(10)),
            )
            .with_constraint(
                CcgConstraint::new("max-lines", ConstraintKind::MaxLines, "Short functions")
                    .with_value(ConstraintValue::Number(50)),
            );

        let intel = CodeIntelligenceContext::new()
            .with_constraints(constraints)
            .mark_available();

        let section = SectionBuilder::build_constraint_section(&intel);

        assert!(section.contains("Active Constraints"));
        assert!(section.contains("maxComplexity"));
        assert!(section.contains("maxLines"));
    }

    #[test]
    fn test_build_constraint_warnings_for_empty() {
        let intel = CodeIntelligenceContext::new().mark_available();
        let warnings = SectionBuilder::build_constraint_warnings_for(&intel, "some_function");
        assert!(warnings.is_empty());
    }

    #[test]
    fn test_build_constraint_warnings_for_targeted() {
        use crate::narsil::{CcgConstraint, ConstraintKind, ConstraintSet, ConstraintValue};

        let constraints = ConstraintSet::new()
            .with_constraint(
                CcgConstraint::new("max-complexity", ConstraintKind::MaxComplexity, "Keep it simple")
                    .with_target("process_request")
                    .with_value(ConstraintValue::Number(10)),
            )
            .with_constraint(
                CcgConstraint::new("no-direct-calls", ConstraintKind::NoDirectCalls, "Use interfaces")
                    .with_target("database::*"),
            );

        let intel = CodeIntelligenceContext::new()
            .with_constraints(constraints)
            .mark_available();

        // process_request should show the complexity constraint
        let warnings = SectionBuilder::build_constraint_warnings_for(&intel, "process_request");
        assert!(warnings.contains("process_request"));
        assert!(warnings.contains("maxComplexity"));
        assert!(!warnings.contains("noDirectCalls"));

        // database::query should show the noDirectCalls constraint
        let db_warnings = SectionBuilder::build_constraint_warnings_for(&intel, "database::query");
        assert!(db_warnings.contains("database::query"));
        assert!(db_warnings.contains("noDirectCalls"));
    }

    #[test]
    fn test_build_constraint_warnings_includes_guidance() {
        use crate::narsil::{CcgConstraint, ConstraintKind, ConstraintSet, ConstraintValue};

        let constraints = ConstraintSet::new().with_constraint(
            CcgConstraint::new("c1", ConstraintKind::MaxComplexity, "Test")
                .with_value(ConstraintValue::Number(10)),
        );

        let intel = CodeIntelligenceContext::new()
            .with_constraints(constraints)
            .mark_available();

        let warnings = SectionBuilder::build_constraint_warnings_for(&intel, "any_function");

        assert!(warnings.contains("When modifying this code"));
        assert!(warnings.contains("comply with"));
    }

    #[test]
    fn test_build_combined_intelligence_section_with_constraints() {
        use crate::narsil::{CcgConstraint, CcgManifest, ConstraintKind, ConstraintSet, ConstraintValue};

        let constraints = ConstraintSet::new().with_constraint(
            CcgConstraint::new("c1", ConstraintKind::MaxComplexity, "Keep simple")
                .with_value(ConstraintValue::Number(10)),
        );

        let intel = CodeIntelligenceContext::new()
            .with_ccg_manifest(CcgManifest::new("test", ".").with_counts(10, 50))
            .with_constraints(constraints)
            .mark_available();

        let section = SectionBuilder::build_combined_intelligence_section(&intel, 2048);

        // Should include both CCG overview and constraints
        assert!(section.contains("Project Overview") || section.contains("project"));
        assert!(section.contains("Active Constraints") || section.contains("maxComplexity"));
    }

    #[test]
    fn test_build_violations_section_empty_when_compliant() {
        use crate::narsil::ComplianceResult;

        let intel = CodeIntelligenceContext::new()
            .with_compliance_result(ComplianceResult::passed(5))
            .mark_available();

        let section = SectionBuilder::build_violations_section(&intel);
        assert!(section.is_empty());
    }

    #[test]
    fn test_build_violations_section_empty_when_no_result() {
        let intel = CodeIntelligenceContext::new().mark_available();

        let section = SectionBuilder::build_violations_section(&intel);
        assert!(section.is_empty());
    }

    #[test]
    fn test_build_violations_section_with_violations() {
        use crate::narsil::{ComplianceResult, ConstraintViolation};

        let result = ComplianceResult::failed(
            vec![
                ConstraintViolation::new(
                    "max-complexity",
                    "process_data",
                    "Complexity 15 exceeds maximum of 10",
                )
                .with_location("src/handler.rs", 42)
                .with_suggestion("Break into smaller functions"),
            ],
            1,
        );

        let intel = CodeIntelligenceContext::new()
            .with_compliance_result(result)
            .mark_available();

        let section = SectionBuilder::build_violations_section(&intel);

        assert!(section.contains("Constraint Violations"));
        assert!(section.contains("process_data"));
        assert!(section.contains("src/handler.rs:42"));
        assert!(section.contains("Complexity 15"));
        assert!(section.contains("Break into smaller functions"));
        assert!(section.contains("Action required"));
    }

    #[test]
    fn test_build_violations_section_multiple_violations() {
        use crate::narsil::{ComplianceResult, ConstraintViolation};

        let result = ComplianceResult::failed(
            vec![
                ConstraintViolation::new(
                    "max-complexity",
                    "func1",
                    "Complexity 15 exceeds max",
                ),
                ConstraintViolation::new(
                    "max-lines",
                    "func2",
                    "Function has 200 lines, max is 100",
                ),
            ],
            2,
        );

        let intel = CodeIntelligenceContext::new()
            .with_compliance_result(result)
            .mark_available();

        let section = SectionBuilder::build_violations_section(&intel);

        assert!(section.contains("2 violation(s)"));
        assert!(section.contains("func1"));
        assert!(section.contains("func2"));
    }
}
