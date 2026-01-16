//! Integration tests for the prompt module.
//!
//! These tests exercise the public API of the prompt module to ensure
//! all types are properly accessible and functional.

use ralph::prompt::antipatterns::{
    detect_quality_gate_ignoring, detect_scope_creep, AntiPatternDetector, DetectorConfig,
    IterationSummary,
};
use ralph::prompt::builder::{DynamicPromptBuilder, SectionBuilder};
use ralph::prompt::context::{
    AntiPattern, AntiPatternSeverity, AntiPatternType, AttemptOutcome, AttemptSummary,
    CurrentTaskContext, ErrorAggregator, ErrorContext, ErrorSeverity, GateResult, PromptContext,
    QualityGateStatus, SessionStats, TaskPhase,
};
use ralph::prompt::templates::{PromptTemplates, Template, TemplateMarker};

// ============================================================================
// Context Types Integration
// ============================================================================

#[test]
fn test_prompt_context_full_builder() {
    let context = PromptContext::new()
        .with_current_task(CurrentTaskContext::new(
            "task-1",
            "Implement feature",
            TaskPhase::Implementation,
        ))
        .with_error(ErrorContext::new(
            "E0308",
            "mismatched types",
            ErrorSeverity::Error,
        ))
        .with_session_stats(SessionStats::new(5, 2, 150).with_budget(50))
        .with_quality_status(QualityGateStatus::all_passing())
        .with_attempt(AttemptSummary::new(1, AttemptOutcome::Success))
        .with_attempts(vec![AttemptSummary::new(2, AttemptOutcome::TestFailure)])
        .with_anti_pattern(AntiPattern::new(
            AntiPatternType::EditWithoutCommit,
            "3 edits without commit",
        ))
        .with_anti_patterns(vec![AntiPattern::new(
            AntiPatternType::TestsNotRun,
            "Tests not run for 5 iterations",
        )]);

    assert!(context.current_task.is_some());
    assert_eq!(context.error_count(), 1);
    assert!(context.quality_status.all_passed());
    assert_eq!(context.attempt_summaries.len(), 2);
    assert_eq!(context.anti_patterns.len(), 2);
}

#[test]
fn test_task_context_phases() {
    let planning = CurrentTaskContext::new("1", "Plan", TaskPhase::Planning);
    let implementation = CurrentTaskContext::new("2", "Build", TaskPhase::Implementation);
    let testing = CurrentTaskContext::new("3", "Test", TaskPhase::Testing);
    let quality = CurrentTaskContext::new("4", "Fix", TaskPhase::QualityFixes);
    let review = CurrentTaskContext::new("5", "Review", TaskPhase::Review);

    assert!(matches!(planning.phase, TaskPhase::Planning));
    assert!(matches!(implementation.phase, TaskPhase::Implementation));
    assert!(matches!(testing.phase, TaskPhase::Testing));
    assert!(matches!(quality.phase, TaskPhase::QualityFixes));
    assert!(matches!(review.phase, TaskPhase::Review));
}

#[test]
fn test_error_context_full_builder() {
    let error = ErrorContext::new("E0308", "type mismatch", ErrorSeverity::Error)
        .with_location("src/lib.rs", 10)
        .with_context("Expected i32, found String")
        .with_suggested_fix("Change type to String")
        .with_occurrences(3);

    assert_eq!(error.code, "E0308");
    assert!(matches!(error.severity, ErrorSeverity::Error));
    assert!(error.context.is_some());
    assert!(error.file.is_some());
    assert!(error.is_recurring());
    assert!(error.is_critical());
}

#[test]
fn test_session_stats_full_builder() {
    let stats = SessionStats::new(10, 5, 500)
        .with_budget(50)
        .with_tasks_completed(3)
        .with_tasks_blocked(1)
        .with_files(vec!["src/lib.rs".to_string()]);

    assert_eq!(stats.iteration_count, 10);
    assert_eq!(stats.commit_count, 5);
    assert_eq!(stats.budget_used_percent(), Some(20));
    assert!(!stats.is_budget_critical());
}

#[test]
fn test_error_aggregator() {
    let mut aggregator = ErrorAggregator::new();
    aggregator.add(ErrorContext::new(
        "E0308",
        "type error",
        ErrorSeverity::Error,
    ));
    aggregator.add(ErrorContext::new(
        "E0308",
        "type error",
        ErrorSeverity::Error,
    ));
    aggregator.add(ErrorContext::new(
        "E0433",
        "unresolved import",
        ErrorSeverity::Error,
    ));

    assert_eq!(aggregator.total_occurrences(), 3);
    assert_eq!(aggregator.unique_count(), 2);

    let errors = aggregator.sorted_by_frequency();
    assert_eq!(errors.len(), 2); // Aggregated to 2 unique codes
    // E0308 should be first (2 occurrences)
    assert_eq!(errors[0].occurrence_count, 2);
}

// ============================================================================
// Templates Integration
// ============================================================================

#[test]
fn test_template_marker_roundtrip() {
    for marker in [
        TemplateMarker::TaskContext,
        TemplateMarker::ErrorContext,
        TemplateMarker::QualityStatus,
        TemplateMarker::SessionStats,
        TemplateMarker::AttemptHistory,
        TemplateMarker::AntiPatterns,
        TemplateMarker::HistoricalGuidance,
        TemplateMarker::CustomSection,
    ] {
        let tag = marker.tag();
        let parsed = TemplateMarker::from_tag(tag);
        assert!(parsed.is_some(), "Failed to parse tag: {}", tag);
    }
}

#[test]
fn test_templates_with_defaults() {
    let templates = PromptTemplates::with_defaults();

    assert!(templates.get_template("build").is_some());
    assert!(templates.get_template("debug").is_some());
    assert!(templates.get_template("plan").is_some());
    assert!(templates.has_template("build"));

    let modes = templates.modes();
    assert_eq!(modes.len(), 3);

    let markers = templates.markers();
    assert!(!markers.is_empty());
}

#[test]
fn test_template_substitution() {
    let template = Template::new("Hello {{TASK_CONTEXT}}!");
    let result = template.substitute(TemplateMarker::TaskContext, "World");

    let content = result.content();
    assert!(content.contains("World"));
}

// ============================================================================
// Builder Integration
// ============================================================================

#[test]
fn test_section_builder_all_sections() {
    let task_ctx = CurrentTaskContext::new("1", "Test", TaskPhase::Implementation);
    let section = SectionBuilder::build_task_section(&task_ctx);
    assert!(section.contains("## Current Task"));

    let error_ctx = ErrorContext::new("E0308", "type error", ErrorSeverity::Error);
    let errors = vec![error_ctx];
    let section = SectionBuilder::build_error_section(&errors);
    assert!(section.contains("## Recent Errors"));

    let quality = QualityGateStatus::all_passing();
    let section = SectionBuilder::build_quality_section(&quality);
    assert!(section.contains("## Quality Gates"));

    let stats = SessionStats::new(10, 5, 500);
    let section = SectionBuilder::build_session_section(&stats);
    assert!(section.contains("## Session Progress"));

    let attempt = AttemptSummary::new(1, AttemptOutcome::Success);
    let attempts = vec![attempt];
    let section = SectionBuilder::build_attempt_section(&attempts);
    assert!(section.contains("## Previous Attempts"));

    let pattern = AntiPattern::new(AntiPatternType::EditWithoutCommit, "test");
    let patterns = vec![pattern];
    let section = SectionBuilder::build_antipattern_section(&patterns);
    assert!(section.contains("## ⚠️ Detected Anti-Patterns"));
}

#[test]
fn test_dynamic_prompt_builder() {
    let templates = PromptTemplates::with_defaults();
    let builder = DynamicPromptBuilder::new(templates);

    let context = PromptContext::new()
        .with_current_task(CurrentTaskContext::new("1", "Test", TaskPhase::Implementation))
        .with_session_stats(SessionStats::new(5, 2, 100));

    let prompt = builder.build("build", &context);
    assert!(prompt.is_ok());

    let content = prompt.unwrap();
    assert!(!content.is_empty());
}

// ============================================================================
// Anti-Pattern Detection Integration
// ============================================================================

#[test]
fn test_iteration_summary_builders() {
    let summary = IterationSummary::new(1)
        .with_files_modified(vec!["src/lib.rs".to_string()])
        .with_commit()
        .with_tests_run()
        .with_clippy_run()
        .with_task("task-1")
        .with_errors(vec!["E0308".to_string()])
        .with_exit_code(0)
        .without_commit();

    assert_eq!(summary.iteration, 1);
    assert!(!summary.committed);
    assert!(summary.tests_run);
}

#[test]
fn test_detector_config() {
    let config = DetectorConfig {
        edit_without_commit_threshold: 5,
        tests_not_run_threshold: 10,
        clippy_not_run_threshold: 10,
        task_oscillation_threshold: 6,
        error_repetition_threshold: 3,
        file_churn_threshold: 8,
    };

    let detector = AntiPatternDetector::with_config(config);
    assert_eq!(detector.iteration_count(), 0);
}

#[test]
fn test_anti_pattern_detector_full_cycle() {
    let mut detector = AntiPatternDetector::new();

    // Add iterations that trigger patterns
    for i in 1..=5 {
        detector.add_iteration(
            IterationSummary::new(i).with_files_modified(vec!["src/lib.rs".to_string()]),
        );
    }

    let patterns = detector.detect();
    // Should detect edit without commit
    assert!(patterns
        .iter()
        .any(|p| p.pattern_type == AntiPatternType::EditWithoutCommit));

    detector.clear();
    assert_eq!(detector.iteration_count(), 0);
}

#[test]
fn test_helper_functions() {
    let status = QualityGateStatus::new()
        .with_tests(GateResult::fail(vec!["test failed".to_string()]))
        .with_clippy(GateResult::pass())
        .with_no_allow(GateResult::pass())
        .with_security(GateResult::pass())
        .with_docs(GateResult::pass())
        .with_timestamp();

    let pattern = detect_quality_gate_ignoring(&status, 5);
    assert!(pattern.is_some());

    let files = vec![
        "src/a/mod.rs".to_string(),
        "src/b/mod.rs".to_string(),
        "src/c/mod.rs".to_string(),
        "src/d/mod.rs".to_string(),
    ];
    let pattern = detect_scope_creep(&files, 3);
    assert!(pattern.is_some());
}

#[test]
fn test_anti_pattern_severity_levels() {
    let high = AntiPattern::new(AntiPatternType::TaskOscillation, "test")
        .with_severity(AntiPatternSeverity::High);
    let medium = AntiPattern::new(AntiPatternType::EditWithoutCommit, "test")
        .with_severity(AntiPatternSeverity::Medium);
    let low = AntiPattern::new(AntiPatternType::ClippyNotRun, "test")
        .with_severity(AntiPatternSeverity::Low);

    assert!(matches!(high.severity, AntiPatternSeverity::High));
    assert!(matches!(medium.severity, AntiPatternSeverity::Medium));
    assert!(matches!(low.severity, AntiPatternSeverity::Low));
}

// ============================================================================
// Prompt Assembler Integration
// ============================================================================

use ralph::prompt::assembler::{AssemblerConfig, PromptAssembler};

#[test]
fn test_prompt_assembler_full_workflow() {
    let mut assembler = PromptAssembler::new();

    // Set up context
    assembler.set_current_task("2.1", "Implement dynamic prompts", TaskPhase::Implementation);
    assembler.update_task_completion(50);
    assembler.update_task_files(vec!["src/prompt/mod.rs".to_string()]);

    // Add session stats
    assembler.update_session_stats(5, 2, 150);
    assembler.set_budget(10);

    // Add an error
    assembler.add_error("E0308", "mismatched types", ErrorSeverity::Error);

    // Update quality status
    assembler.update_clippy_status(true, vec![]);
    assembler.update_test_status(false, vec!["test_foo failed".to_string()]);

    // Record an attempt
    assembler.record_attempt(
        AttemptOutcome::TestFailure,
        Some("TDD approach"),
        vec!["test_foo failed".to_string()],
    );

    // Record iteration
    assembler.record_iteration_with_files(1, vec!["src/prompt/mod.rs".to_string()], false);

    // Build prompt
    let prompt = assembler.build_prompt("build").expect("should build");

    // Verify all components are present
    assert!(prompt.contains("2.1"));
    assert!(prompt.contains("Implement dynamic prompts"));
    assert!(prompt.contains("Session Progress"));
    assert!(prompt.contains("E0308"));
}

#[test]
fn test_prompt_assembler_config() {
    let config = AssemblerConfig::new()
        .with_max_errors(3)
        .with_max_attempts(2)
        .with_max_anti_patterns(1);

    let assembler = PromptAssembler::with_config(config);
    assert_eq!(assembler.error_count(), 0);
}

#[test]
fn test_prompt_assembler_task_switching() {
    let mut assembler = PromptAssembler::new();

    // Set first task and record an attempt
    assembler.set_current_task("1.1", "First task", TaskPhase::Implementation);
    assembler.record_attempt(AttemptOutcome::TestFailure, None, vec![]);
    assert_eq!(assembler.attempt_count(), 1);

    // Switch to second task - attempts should be cleared
    assembler.set_current_task("2.1", "Second task", TaskPhase::Implementation);
    assert_eq!(assembler.attempt_count(), 0);
}

#[test]
fn test_prompt_assembler_error_deduplication() {
    let mut assembler = PromptAssembler::new();

    assembler.add_error("E0308", "mismatched types", ErrorSeverity::Error);
    assembler.add_error("E0308", "mismatched types", ErrorSeverity::Error);
    assembler.add_error("E0433", "unresolved import", ErrorSeverity::Error);

    // Should have 2 unique errors (E0308 deduplicated)
    assert_eq!(assembler.error_count(), 2);
}

#[test]
fn test_prompt_assembler_quality_failure_streak() {
    let mut assembler = PromptAssembler::new();

    assembler.update_test_status(false, vec!["failed".to_string()]);
    assembler.update_test_status(false, vec!["failed".to_string()]);
    assembler.update_test_status(true, vec![]); // Reset streak

    // Streak was reset on pass
    assert!(assembler.quality_status().tests.passed);
}

#[test]
fn test_prompt_assembler_reset() {
    let mut assembler = PromptAssembler::new();

    // Set up some state
    assembler.set_current_task("1.1", "Test", TaskPhase::Testing);
    assembler.add_error("E0308", "error", ErrorSeverity::Error);
    assembler.update_session_stats(5, 2, 100);

    // Reset
    assembler.reset();

    // Verify everything is cleared
    assert!(assembler.current_task().is_none());
    assert_eq!(assembler.error_count(), 0);
    assert_eq!(assembler.session_stats().iteration_count, 0);
}

#[test]
fn test_prompt_assembler_all_modes() {
    let assembler = PromptAssembler::new();

    // Build mode
    let build_prompt = assembler.build_prompt("build").expect("build should succeed");
    assert!(build_prompt.contains("Build Phase"));

    // Debug mode
    let debug_prompt = assembler.build_prompt("debug").expect("debug should succeed");
    assert!(debug_prompt.contains("Debug Phase"));

    // Plan mode
    let plan_prompt = assembler.build_prompt("plan").expect("plan should succeed");
    assert!(plan_prompt.contains("Plan Phase"));

    // Unknown mode should fail
    let result = assembler.build_prompt("unknown");
    assert!(result.is_err());
}
