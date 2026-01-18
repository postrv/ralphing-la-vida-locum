//! Code intelligence section builders.
//!
//! Generates markdown sections for code intelligence data from narsil-mcp,
//! including call graphs, symbol references, dependencies, CCG data,
//! constraints, and constraint violations.

use crate::prompt::context::{
    CallGraphNode, CodeIntelligenceContext, ModuleDependency, SymbolReference,
};

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
    let has_narsil_data = intel.is_available
        && (!intel.call_graph.is_empty()
            || !intel.references.is_empty()
            || !intel.dependencies.is_empty());

    if !has_narsil_data && !intel.has_constraints() {
        return String::new();
    }

    let mut sections = vec!["## Code Intelligence".to_string(), String::new()];

    // Collect all non-empty sections
    push_if_not_empty(&mut sections, build_call_graph_section(&intel.call_graph));
    push_if_not_empty(&mut sections, build_references_section(&intel.references));
    push_if_not_empty(
        &mut sections,
        build_dependencies_section(&intel.dependencies),
    );
    push_section_with_newline(&mut sections, intel.constraints.to_prompt_section());
    push_section_with_newline(&mut sections, build_violations_section(intel));

    sections.join("\n")
}

/// Push a section to the list if it's not empty.
fn push_if_not_empty(sections: &mut Vec<String>, section: String) {
    if !section.is_empty() {
        sections.push(section);
    }
}

/// Push a section with trailing newline if not empty.
fn push_section_with_newline(sections: &mut Vec<String>, section: String) {
    if !section.is_empty() {
        sections.push(section);
        sections.push(String::new());
    }
}

/// Format a call graph node for display.
fn format_call_graph_node(node: &CallGraphNode, lines: &mut Vec<String>, is_hotspot: bool) {
    let location = match (&node.file, node.line) {
        (Some(f), Some(l)) => format!(" (`{}:{}`)", f, l),
        (Some(f), None) => format!(" (`{}`)", f),
        _ => String::new(),
    };

    let prefix = if is_hotspot { "  \u{1f525} " } else { "  - " };
    lines.push(format!("{}`{}`{}", prefix, node.function_name, location));

    // Show callers (limited)
    if !node.callers.is_empty() {
        let callers_preview: Vec<_> = node.callers.iter().take(5).collect();
        let callers_str = callers_preview
            .iter()
            .map(|s| format!("`{}`", s))
            .collect::<Vec<_>>()
            .join(", ");
        if node.callers.len() > 5 {
            lines.push(format!(
                "    \u{2190} Called by: {} ... (+{} more)",
                callers_str,
                node.callers.len() - 5
            ));
        } else {
            lines.push(format!("    \u{2190} Called by: {}", callers_str));
        }
    }

    // Show callees (limited)
    if !node.callees.is_empty() {
        let callees_preview: Vec<_> = node.callees.iter().take(5).collect();
        let callees_str = callees_preview
            .iter()
            .map(|s| format!("`{}`", s))
            .collect::<Vec<_>>()
            .join(", ");
        if node.callees.len() > 5 {
            lines.push(format!(
                "    \u{2192} Calls: {} ... (+{} more)",
                callees_str,
                node.callees.len() - 5
            ));
        } else {
            lines.push(format!("    \u{2192} Calls: {}", callees_str));
        }
    }
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
    if call_graph.is_empty() {
        return String::new();
    }

    let mut lines = vec!["### Relevant Functions".to_string(), String::new()];

    // Show hotspots first with special formatting
    let hotspots: Vec<_> = call_graph.iter().filter(|n| n.is_hotspot()).collect();
    if !hotspots.is_empty() {
        lines.push("**\u{1f525} Hotspots (highly connected):**".to_string());
        for node in hotspots.iter().take(3) {
            format_call_graph_node(node, &mut lines, true);
        }
        lines.push(String::new());
    }

    // Show other functions
    let regular: Vec<_> = call_graph.iter().filter(|n| !n.is_hotspot()).collect();
    if !regular.is_empty() {
        lines.push("**Functions:**".to_string());
        for node in regular.iter().take(5) {
            format_call_graph_node(node, &mut lines, false);
        }
    }

    if call_graph.len() > 8 {
        lines.push(format!("... and {} more functions", call_graph.len() - 8));
    }
    lines.push(String::new());

    lines.join("\n")
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
    if references.is_empty() {
        return String::new();
    }

    let mut lines = vec!["### Symbol References".to_string(), String::new()];

    // Group by symbol (show first occurrence only)
    let mut symbols_seen: std::collections::HashSet<&str> = std::collections::HashSet::new();
    for reference in references.iter().take(10) {
        if symbols_seen.insert(&reference.symbol) {
            let location = format!("{}:{}", reference.file, reference.line);
            let kind_str = format!(" ({})", reference.kind);
            lines.push(format!(
                "- `{}` at `{}`{}",
                reference.symbol, location, kind_str
            ));
        }
    }

    if references.len() > 10 {
        lines.push(format!("... and {} more references", references.len() - 10));
    }
    lines.push(String::new());

    lines.join("\n")
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
    if dependencies.is_empty() {
        return String::new();
    }

    let mut lines = vec!["### Dependencies".to_string(), String::new()];

    for dep in dependencies.iter().take(5) {
        lines.push(format!("**`{}`**", dep.module_path));
        if !dep.imports.is_empty() {
            let imports_str = dep
                .imports
                .iter()
                .take(5)
                .map(String::as_str)
                .collect::<Vec<_>>()
                .join(", ");
            if dep.imports.len() > 5 {
                lines.push(format!(
                    "  - Imports: {} ... (+{} more)",
                    imports_str,
                    dep.imports.len() - 5
                ));
            } else {
                lines.push(format!("  - Imports: {}", imports_str));
            }
        }
        if !dep.imported_by.is_empty() {
            let importers_str = dep
                .imported_by
                .iter()
                .take(3)
                .map(String::as_str)
                .collect::<Vec<_>>()
                .join(", ");
            if dep.imported_by.len() > 3 {
                lines.push(format!(
                    "  - Used by: {} ... (+{} more)",
                    importers_str,
                    dep.imported_by.len() - 3
                ));
            } else {
                lines.push(format!("  - Used by: {}", importers_str));
            }
        }
    }
    lines.push(String::new());

    lines.join("\n")
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
        lines.push(format!(
            "**Symbols:** {} across {} files",
            manifest.symbol_count, manifest.file_count
        ));

        // Security summary (if there are any issues)
        let sec = &manifest.security_summary;
        let total = sec.critical + sec.high + sec.medium + sec.low;
        if total > 0 {
            lines.push(String::new());
            lines.push("**Security Summary:**".to_string());
            if sec.critical > 0 {
                lines.push(format!("  \u{1f534} Critical: {}", sec.critical));
            }
            if sec.high > 0 {
                lines.push(format!("  \u{1f7e0} High: {}", sec.high));
            }
            if sec.medium > 0 {
                lines.push(format!("  \u{1f7e1} Medium: {}", sec.medium));
            }
            if sec.low > 0 {
                lines.push(format!("  \u{1f535} Low: {}", sec.low));
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
                    lines.push(format!(
                        "- `{}` ({}) at `{}:{}`",
                        entry.name, entry.kind, location, line
                    ));
                } else {
                    lines.push(format!(
                        "- `{}` ({}) in `{}`",
                        entry.name, entry.kind, location
                    ));
                }
            }
            if arch.entry_points.len() > 5 {
                lines.push(format!(
                    "... and {} more entry points",
                    arch.entry_points.len() - 5
                ));
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
                lines.push(format!(
                    "... and {} more public symbols",
                    arch.public_api.len() - 8
                ));
            }
            lines.push(String::new());
        }

        // Module structure (limit to 5 modules)
        if !arch.modules.is_empty() {
            lines.push("### Module Structure".to_string());
            for module in arch.modules.iter().take(5) {
                let children_count = module.children.len();
                if children_count > 0 {
                    lines.push(format!(
                        "- `{}` ({} submodules)",
                        module.name, children_count
                    ));
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

    let mut lines = vec![format!("### Constraints for `{}`", target), String::new()];

    for constraint in constraints.iter().take(5) {
        lines.push(constraint.to_prompt_string());
    }

    if constraints.len() > 5 {
        lines.push(format!(
            "\n*...and {} more constraints*",
            constraints.len() - 5
        ));
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
        "### \u{26a0}\u{fe0f} Constraint Violations".to_string(),
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
            lines.push(format!("   - \u{1f4a1} *{}*", suggestion));
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
pub fn build_combined_intelligence_section(
    intel: &CodeIntelligenceContext,
    max_bytes: usize,
) -> String {
    if !intel.is_available {
        return String::new();
    }

    let mut sections = Vec::new();

    // Add CCG section first (overview)
    let ccg_section = build_ccg_section(intel);
    if !ccg_section.is_empty() {
        sections.push(ccg_section);
    }

    // Add constraints section (important for guiding implementation)
    let constraint_section = build_constraint_section(intel);
    if !constraint_section.is_empty() {
        sections.push(constraint_section);
    }

    // Add intelligence section (call graph, refs, deps)
    let intel_section = build_intelligence_section(intel);
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
            format!(
                "{}\n\n... (intelligence section truncated to {}B)",
                &combined[..pos],
                max_bytes
            )
        } else {
            format!(
                "{}... (truncated)",
                &combined[..max_bytes.saturating_sub(20)]
            )
        }
    } else {
        combined
    }
}

#[cfg(test)]
#[path = "intelligence_tests.rs"]
mod tests;
