//! Tests for intelligence section builders.

use super::*;
use crate::narsil::{CcgManifest, ComplianceResult, ConstraintViolation};
use crate::prompt::context::ReferenceKind;

// build_intelligence_section tests

#[test]
fn test_build_intelligence_section_empty() {
    let intel = CodeIntelligenceContext::new();
    let section = build_intelligence_section(&intel);
    assert!(section.is_empty());
}

#[test]
fn test_build_intelligence_section_unavailable() {
    // Even with data, if not marked available, should be empty
    let intel = CodeIntelligenceContext::new().with_call_graph(vec![CallGraphNode::new("foo")]);
    let section = build_intelligence_section(&intel);
    assert!(section.is_empty());
}

#[test]
fn test_build_intelligence_section_with_call_graph() {
    let intel = CodeIntelligenceContext::new()
        .with_call_graph(vec![CallGraphNode::new("process_request")
            .with_file("src/handler.rs")
            .with_callers(vec!["main".to_string(), "handle_http".to_string()])
            .with_callees(vec!["validate".to_string()])])
        .mark_available();

    let section = build_intelligence_section(&intel);

    assert!(section.contains("## Code Intelligence"));
    assert!(section.contains("process_request"));
    assert!(section.contains("main"));
    assert!(section.contains("validate"));
}

#[test]
fn test_build_intelligence_section_with_references() {
    let intel = CodeIntelligenceContext::new()
        .with_references(vec![
            SymbolReference::new("MyStruct", "src/lib.rs", 42).with_kind(ReferenceKind::Definition),
            SymbolReference::new("MyStruct", "src/main.rs", 10).with_kind(ReferenceKind::Usage),
        ])
        .mark_available();

    let section = build_intelligence_section(&intel);

    assert!(section.contains("## Code Intelligence"));
    assert!(section.contains("MyStruct"));
    assert!(section.contains("src/lib.rs:42"));
}

#[test]
fn test_build_intelligence_section_with_dependencies() {
    let intel = CodeIntelligenceContext::new()
        .with_dependencies(vec![ModuleDependency::new("src/lib.rs")
            .with_imports(vec!["std::io".to_string(), "crate::util".to_string()])
            .with_imported_by(vec!["src/main.rs".to_string()])])
        .mark_available();

    let section = build_intelligence_section(&intel);

    assert!(section.contains("## Code Intelligence"));
    assert!(section.contains("Dependencies"));
    assert!(section.contains("src/lib.rs"));
}

#[test]
fn test_build_intelligence_section_hotspots() {
    let intel = CodeIntelligenceContext::new()
        .with_call_graph(vec![CallGraphNode::new("hotspot_func")
            .with_callers(vec![
                "a".into(),
                "b".into(),
                "c".into(),
                "d".into(),
                "e".into(),
            ])
            .with_callees(vec!["x".into()])])
        .mark_available();

    let section = build_intelligence_section(&intel);

    assert!(section.contains("hotspot") || section.contains("Hotspot"));
}

#[test]
fn test_build_intelligence_section_truncates_long_lists() {
    let callers: Vec<String> = (0..20).map(|i| format!("caller_{}", i)).collect();
    let intel = CodeIntelligenceContext::new()
        .with_call_graph(vec![
            CallGraphNode::new("popular_func").with_callers(callers)
        ])
        .mark_available();

    let section = build_intelligence_section(&intel);

    // Should truncate long lists
    assert!(section.contains("...") || section.contains("more"));
}

#[test]
fn test_build_intelligence_section_more_than_8_call_graph_nodes() {
    // When there are more than 8 call graph nodes, should show "and X more"
    let nodes: Vec<CallGraphNode> = (0..12)
        .map(|i| CallGraphNode::new(format!("func_{}", i)))
        .collect();
    let intel = CodeIntelligenceContext::new()
        .with_call_graph(nodes)
        .mark_available();

    let section = build_intelligence_section(&intel);

    assert!(section.contains("### Relevant Functions"));
    assert!(section.contains("and 4 more functions")); // 12 - 8 = 4 more
}

#[test]
fn test_build_intelligence_section_more_than_10_references() {
    // When there are more than 10 references, should show "and X more"
    let refs: Vec<SymbolReference> = (0..15)
        .map(|i| {
            SymbolReference::new(format!("symbol_{}", i), format!("file_{}.rs", i), i as u32)
                .with_kind(ReferenceKind::Usage)
        })
        .collect();
    let intel = CodeIntelligenceContext::new()
        .with_references(refs)
        .mark_available();

    let section = build_intelligence_section(&intel);

    assert!(section.contains("### Symbol References"));
    assert!(section.contains("and 5 more references")); // 15 - 10 = 5 more
}

#[test]
fn test_build_intelligence_section_dependencies_with_many_imports() {
    // Dependencies with > 5 imports should show "+X more"
    let imports: Vec<String> = (0..8).map(|i| format!("import_{}", i)).collect();
    let dep = ModuleDependency::new("src/lib.rs").with_imports(imports);
    let intel = CodeIntelligenceContext::new()
        .with_dependencies(vec![dep])
        .mark_available();

    let section = build_intelligence_section(&intel);

    assert!(section.contains("### Dependencies"));
    assert!(section.contains("src/lib.rs"));
    assert!(section.contains("+3 more")); // 8 - 5 = 3 more
}

#[test]
fn test_build_intelligence_section_dependencies_with_many_importers() {
    // Dependencies with > 3 imported_by should show "+X more"
    let imported_by: Vec<String> = (0..6).map(|i| format!("user_{}.rs", i)).collect();
    let dep = ModuleDependency::new("src/lib.rs").with_imported_by(imported_by);
    let intel = CodeIntelligenceContext::new()
        .with_dependencies(vec![dep])
        .mark_available();

    let section = build_intelligence_section(&intel);

    assert!(section.contains("### Dependencies"));
    assert!(section.contains("Used by:"));
    assert!(section.contains("+3 more")); // 6 - 3 = 3 more
}

#[test]
fn test_build_intelligence_section_combined_all_sections() {
    // Test that all sections appear together correctly
    let intel = CodeIntelligenceContext::new()
        .with_call_graph(vec![CallGraphNode::new("func_a")
            .with_file("src/a.rs")
            .with_line(10)
            .with_callers(vec!["main".to_string()])])
        .with_references(vec![
            SymbolReference::new("MyType", "src/types.rs", 25).with_kind(ReferenceKind::Definition)
        ])
        .with_dependencies(vec![
            ModuleDependency::new("src/lib.rs").with_imports(vec!["std::io".to_string()])
        ])
        .mark_available();

    let section = build_intelligence_section(&intel);

    // Should have all sections
    assert!(section.contains("## Code Intelligence"));
    assert!(section.contains("### Relevant Functions"));
    assert!(section.contains("### Symbol References"));
    assert!(section.contains("### Dependencies"));

    // Check order: functions should come before references, which should come before deps
    let funcs_pos = section.find("### Relevant Functions").unwrap();
    let refs_pos = section.find("### Symbol References").unwrap();
    let deps_pos = section.find("### Dependencies").unwrap();
    assert!(funcs_pos < refs_pos);
    assert!(refs_pos < deps_pos);
}

#[test]
fn test_build_intelligence_section_hotspots_shown_first() {
    // Hotspots (>= 5 connections) should appear before regular functions
    let hotspot = CallGraphNode::new("hotspot_func").with_callers(vec![
        "a".into(),
        "b".into(),
        "c".into(),
        "d".into(),
        "e".into(),
    ]);
    let regular = CallGraphNode::new("regular_func").with_callers(vec!["x".into()]);

    let intel = CodeIntelligenceContext::new()
        .with_call_graph(vec![regular, hotspot]) // Regular first in input
        .mark_available();

    let section = build_intelligence_section(&intel);

    // Hotspots section should appear before regular functions section
    assert!(section.contains("Hotspots"));
    let hotspot_pos = section.find("hotspot_func").unwrap();
    let regular_pos = section.find("regular_func").unwrap();
    assert!(hotspot_pos < regular_pos); // Hotspot should appear first
}

#[test]
fn test_build_intelligence_section_call_graph_node_location_format() {
    // Test the location format for call graph nodes
    let node_with_both = CallGraphNode::new("func_full")
        .with_file("src/handler.rs")
        .with_line(42);
    let node_with_file_only = CallGraphNode::new("func_file").with_file("src/other.rs");
    let node_without_location = CallGraphNode::new("func_bare");

    let intel = CodeIntelligenceContext::new()
        .with_call_graph(vec![
            node_with_both,
            node_with_file_only,
            node_without_location,
        ])
        .mark_available();

    let section = build_intelligence_section(&intel);

    // Full location should show file:line
    assert!(section.contains("`src/handler.rs:42`"));
    // File only should show just file
    assert!(section.contains("`src/other.rs`"));
}

#[test]
fn test_build_intelligence_section_references_deduplicated_by_symbol() {
    // References should be deduplicated by symbol name
    let refs = vec![
        SymbolReference::new("DuplicateSymbol", "file1.rs", 10),
        SymbolReference::new("DuplicateSymbol", "file2.rs", 20), // Same symbol, different file
        SymbolReference::new("UniqueSymbol", "file3.rs", 30),
    ];
    let intel = CodeIntelligenceContext::new()
        .with_references(refs)
        .mark_available();

    let section = build_intelligence_section(&intel);

    // Should only show DuplicateSymbol once (first occurrence at file1.rs:10)
    assert!(section.contains("`DuplicateSymbol` at `file1.rs:10`"));
    // Should show UniqueSymbol
    assert!(section.contains("`UniqueSymbol` at `file3.rs:30`"));
    // Count occurrences of DuplicateSymbol - should be 1
    let count = section.matches("DuplicateSymbol").count();
    assert_eq!(count, 1, "DuplicateSymbol should appear only once");
}

// build_call_graph_section helper tests

#[test]
fn test_build_call_graph_section_empty() {
    let section = build_call_graph_section(&[]);
    assert!(section.is_empty());
}

#[test]
fn test_build_call_graph_section_with_regular_functions() {
    let nodes = vec![
        CallGraphNode::new("func_a").with_file("a.rs").with_line(10),
        CallGraphNode::new("func_b").with_file("b.rs").with_line(20),
    ];
    let section = build_call_graph_section(&nodes);

    assert!(section.contains("### Relevant Functions"));
    assert!(section.contains("func_a"));
    assert!(section.contains("func_b"));
    assert!(section.contains("`a.rs:10`"));
}

#[test]
fn test_build_call_graph_section_with_hotspots() {
    let hotspot = CallGraphNode::new("hot_func").with_callers(vec![
        "a".into(),
        "b".into(),
        "c".into(),
        "d".into(),
        "e".into(),
    ]);
    let regular = CallGraphNode::new("normal_func");

    let section = build_call_graph_section(&[hotspot, regular]);

    assert!(section.contains("Hotspots"));
    assert!(section.contains("hot_func"));
    assert!(section.contains("normal_func"));
    // Hotspot should appear before regular
    let hot_pos = section.find("hot_func").unwrap();
    let regular_pos = section.find("normal_func").unwrap();
    assert!(hot_pos < regular_pos);
}

#[test]
fn test_build_call_graph_section_truncates_at_8() {
    let nodes: Vec<CallGraphNode> = (0..12)
        .map(|i| CallGraphNode::new(format!("fn_{}", i)))
        .collect();
    let section = build_call_graph_section(&nodes);

    assert!(section.contains("and 4 more functions"));
}

// build_references_section helper tests

#[test]
fn test_build_references_section_empty() {
    let section = build_references_section(&[]);
    assert!(section.is_empty());
}

#[test]
fn test_build_references_section_basic() {
    let refs = vec![
        SymbolReference::new("MyType", "lib.rs", 10).with_kind(ReferenceKind::Definition),
        SymbolReference::new("OtherType", "main.rs", 20).with_kind(ReferenceKind::Usage),
    ];
    let section = build_references_section(&refs);

    assert!(section.contains("### Symbol References"));
    assert!(section.contains("`MyType` at `lib.rs:10`"));
    assert!(section.contains("`OtherType` at `main.rs:20`"));
}

#[test]
fn test_build_references_section_truncates_at_10() {
    let refs: Vec<SymbolReference> = (0..15)
        .map(|i| SymbolReference::new(format!("sym_{}", i), format!("file_{}.rs", i), i as u32))
        .collect();
    let section = build_references_section(&refs);

    assert!(section.contains("and 5 more references"));
}

#[test]
fn test_build_references_section_deduplicates() {
    let refs = vec![
        SymbolReference::new("Same", "file1.rs", 1),
        SymbolReference::new("Same", "file2.rs", 2), // Duplicate symbol
        SymbolReference::new("Different", "file3.rs", 3),
    ];
    let section = build_references_section(&refs);

    // Same should only appear once (first occurrence)
    let count = section.matches("Same").count();
    assert_eq!(count, 1);
}

// build_dependencies_section helper tests

#[test]
fn test_build_dependencies_section_empty() {
    let section = build_dependencies_section(&[]);
    assert!(section.is_empty());
}

#[test]
fn test_build_dependencies_section_basic() {
    let deps = vec![ModuleDependency::new("src/lib.rs")
        .with_imports(vec!["std::io".into()])
        .with_imported_by(vec!["main.rs".into()])];
    let section = build_dependencies_section(&deps);

    assert!(section.contains("### Dependencies"));
    assert!(section.contains("src/lib.rs"));
    assert!(section.contains("std::io"));
    assert!(section.contains("main.rs"));
}

#[test]
fn test_build_dependencies_section_truncates_imports() {
    let imports: Vec<String> = (0..8).map(|i| format!("import_{}", i)).collect();
    let deps = vec![ModuleDependency::new("mod.rs").with_imports(imports)];
    let section = build_dependencies_section(&deps);

    assert!(section.contains("+3 more")); // 8 - 5 = 3
}

#[test]
fn test_build_dependencies_section_truncates_importers() {
    let imported_by: Vec<String> = (0..6).map(|i| format!("user_{}.rs", i)).collect();
    let deps = vec![ModuleDependency::new("mod.rs").with_imported_by(imported_by)];
    let section = build_dependencies_section(&deps);

    assert!(section.contains("+3 more")); // 6 - 3 = 3
}

// Constraint section tests

#[test]
fn test_build_constraint_section_empty() {
    let intel = CodeIntelligenceContext::new().mark_available();
    let section = build_constraint_section(&intel);
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
    let section = build_constraint_section(&intel);
    assert!(section.is_empty());
}

#[test]
fn test_build_constraint_section_with_constraints() {
    use crate::narsil::{CcgConstraint, ConstraintKind, ConstraintSet, ConstraintValue};

    let constraints = ConstraintSet::new()
        .with_constraint(
            CcgConstraint::new(
                "max-complexity",
                ConstraintKind::MaxComplexity,
                "Keep it simple",
            )
            .with_value(ConstraintValue::Number(10)),
        )
        .with_constraint(
            CcgConstraint::new("max-lines", ConstraintKind::MaxLines, "Short functions")
                .with_value(ConstraintValue::Number(50)),
        );

    let intel = CodeIntelligenceContext::new()
        .with_constraints(constraints)
        .mark_available();

    let section = build_constraint_section(&intel);

    assert!(section.contains("Active Constraints"));
    assert!(section.contains("maxComplexity"));
    assert!(section.contains("maxLines"));
}

#[test]
fn test_build_constraint_warnings_for_empty() {
    let intel = CodeIntelligenceContext::new().mark_available();
    let warnings = build_constraint_warnings_for(&intel, "some_function");
    assert!(warnings.is_empty());
}

#[test]
fn test_build_constraint_warnings_for_targeted() {
    use crate::narsil::{CcgConstraint, ConstraintKind, ConstraintSet, ConstraintValue};

    let constraints = ConstraintSet::new()
        .with_constraint(
            CcgConstraint::new(
                "max-complexity",
                ConstraintKind::MaxComplexity,
                "Keep it simple",
            )
            .with_target("process_request")
            .with_value(ConstraintValue::Number(10)),
        )
        .with_constraint(
            CcgConstraint::new(
                "no-direct-calls",
                ConstraintKind::NoDirectCalls,
                "Use interfaces",
            )
            .with_target("database::*"),
        );

    let intel = CodeIntelligenceContext::new()
        .with_constraints(constraints)
        .mark_available();

    // process_request should show the complexity constraint
    let warnings = build_constraint_warnings_for(&intel, "process_request");
    assert!(warnings.contains("process_request"));
    assert!(warnings.contains("maxComplexity"));
    assert!(!warnings.contains("noDirectCalls"));

    // database::query should show the noDirectCalls constraint
    let db_warnings = build_constraint_warnings_for(&intel, "database::query");
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

    let warnings = build_constraint_warnings_for(&intel, "any_function");

    assert!(warnings.contains("When modifying this code"));
    assert!(warnings.contains("comply with"));
}

#[test]
fn test_build_combined_intelligence_section_with_constraints() {
    use crate::narsil::{CcgConstraint, ConstraintKind, ConstraintSet, ConstraintValue};

    let constraints = ConstraintSet::new().with_constraint(
        CcgConstraint::new("c1", ConstraintKind::MaxComplexity, "Keep simple")
            .with_value(ConstraintValue::Number(10)),
    );

    let intel = CodeIntelligenceContext::new()
        .with_ccg_manifest(CcgManifest::new("test", ".").with_counts(10, 50))
        .with_constraints(constraints)
        .mark_available();

    let section = build_combined_intelligence_section(&intel, 2048);

    // Should include both CCG overview and constraints
    assert!(section.contains("Project Overview") || section.contains("project"));
    assert!(section.contains("Active Constraints") || section.contains("maxComplexity"));
}

#[test]
fn test_build_violations_section_empty_when_compliant() {
    let intel = CodeIntelligenceContext::new()
        .with_compliance_result(ComplianceResult::passed(5))
        .mark_available();

    let section = build_violations_section(&intel);
    assert!(section.is_empty());
}

#[test]
fn test_build_violations_section_empty_when_no_result() {
    let intel = CodeIntelligenceContext::new().mark_available();

    let section = build_violations_section(&intel);
    assert!(section.is_empty());
}

#[test]
fn test_build_violations_section_with_violations() {
    let result = ComplianceResult::failed(
        vec![ConstraintViolation::new(
            "max-complexity",
            "process_data",
            "Complexity 15 exceeds maximum of 10",
        )
        .with_location("src/handler.rs", 42)
        .with_suggestion("Break into smaller functions")],
        1,
    );

    let intel = CodeIntelligenceContext::new()
        .with_compliance_result(result)
        .mark_available();

    let section = build_violations_section(&intel);

    assert!(section.contains("Constraint Violations"));
    assert!(section.contains("process_data"));
    assert!(section.contains("src/handler.rs:42"));
    assert!(section.contains("Complexity 15"));
    assert!(section.contains("Break into smaller functions"));
    assert!(section.contains("Action required"));
}

#[test]
fn test_build_violations_section_multiple_violations() {
    let result = ComplianceResult::failed(
        vec![
            ConstraintViolation::new("max-complexity", "func1", "Complexity 15 exceeds max"),
            ConstraintViolation::new("max-lines", "func2", "Function has 200 lines, max is 100"),
        ],
        2,
    );

    let intel = CodeIntelligenceContext::new()
        .with_compliance_result(result)
        .mark_available();

    let section = build_violations_section(&intel);

    assert!(section.contains("2 violation(s)"));
    assert!(section.contains("func1"));
    assert!(section.contains("func2"));
}
