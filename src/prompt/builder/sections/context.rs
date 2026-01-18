//! Context section builders.
//!
//! Generates markdown sections for error, quality, session, attempt,
//! anti-pattern, and historical guidance contexts.

use std::collections::HashMap;

use crate::prompt::context::{
    AntiPattern, AntiPatternSeverity, AttemptOutcome, AttemptSummary, ErrorContext,
    QualityGateStatus, SessionStats,
};

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
            crate::prompt::context::ErrorSeverity::Error => "\u{1f534}",
            crate::prompt::context::ErrorSeverity::Warning => "\u{1f7e1}",
            crate::prompt::context::ErrorSeverity::Info => "\u{1f535}",
        };

        let recurrence = if error.is_recurring() {
            format!(" (\u{00d7}{})", error.occurrence_count)
        } else {
            String::new()
        };

        lines.push(format!(
            "{} **{}**{}: {}",
            severity_icon, error.code, recurrence, error.message
        ));

        if let (Some(file), Some(line)) = (&error.file, error.line) {
            lines.push(format!("   \u{1f4cd} `{}:{}`", file, line));
        }

        if let Some(fix) = &error.suggested_fix {
            lines.push(format!("   \u{1f4a1} {}", fix));
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
/// ```
#[must_use]
pub fn build_quality_section(status: &QualityGateStatus) -> String {
    // Don't show if never checked
    if status.last_check.is_none() {
        return String::new();
    }

    let mut lines = vec!["## Quality Gates".to_string(), String::new()];

    // Clippy
    let clippy_icon = if status.clippy.passed {
        "\u{2705}"
    } else {
        "\u{274c}"
    };
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
    let tests_icon = if status.tests.passed {
        "\u{2705}"
    } else {
        "\u{274c}"
    };
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
    let no_allow_icon = if status.no_allow.passed {
        "\u{2705}"
    } else {
        "\u{274c}"
    };
    lines.push(format!("{} **No #[allow] annotations**", no_allow_icon));
    if !status.no_allow.messages.is_empty() {
        for msg in status.no_allow.messages.iter().take(3) {
            lines.push(format!("   - {}", msg));
        }
    }

    // Security
    let security_icon = if status.security.passed {
        "\u{2705}"
    } else {
        "\u{274c}"
    };
    lines.push(format!("{} **Security scan**", security_icon));
    if !status.security.messages.is_empty() {
        for msg in status.security.messages.iter().take(3) {
            lines.push(format!("   - {}", msg));
        }
    }

    // Docs
    let docs_icon = if status.docs.passed {
        "\u{2705}"
    } else {
        "\u{274c}"
    };
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
                "\u{1f534} **Budget critical:** {}% used ({}/{} iterations)",
                budget_percent,
                stats.iteration_count,
                stats.max_iterations.unwrap_or(0)
            ));
            lines.push(
                "   \u{26a0}\u{fe0f} Prioritize completing current task or commit progress!"
                    .to_string(),
            );
        } else if budget_percent >= 80 {
            lines.push(format!(
                "\u{1f7e1} **Budget warning:** {}% used ({}/{} iterations)",
                budget_percent,
                stats.iteration_count,
                stats.max_iterations.unwrap_or(0)
            ));
        } else {
            lines.push(format!(
                "\u{1f7e2} **Budget:** {}% used ({}/{} iterations)",
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
                "\u{1f534} **Stagnation alert:** {} iterations without progress",
                stats.stagnation_count
            ));
            lines.push(
                "   Consider: Is the current approach working? Try a different strategy."
                    .to_string(),
            );
        } else {
            lines.push(format!(
                "\u{1f7e1} **Note:** {} iteration(s) without progress",
                stats.stagnation_count
            ));
        }
    }

    if !stats.is_progressing() && stats.iteration_count > 3 {
        lines.push(String::new());
        lines.push(
            "\u{26a0}\u{fe0f} **Low commit rate** - Consider making smaller, incremental commits."
                .to_string(),
        );
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
            AttemptOutcome::Success => "\u{2705}",
            AttemptOutcome::CompilationError => "\u{1f534}",
            AttemptOutcome::TestFailure => "\u{1f7e1}",
            AttemptOutcome::QualityGateFailed => "\u{1f7e0}",
            AttemptOutcome::Timeout => "\u{23f1}\u{fe0f}",
            AttemptOutcome::Blocked => "\u{1f6ab}",
            AttemptOutcome::Abandoned => "\u{26aa}",
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
    let failed_attempts: Vec<_> = attempts
        .iter()
        .filter(|a| !a.outcome.is_success())
        .collect();
    if failed_attempts.len() >= 2 {
        lines.push("### \u{26a0}\u{fe0f} Pattern Analysis".to_string());

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
                lines.push(format!("- {} (\u{00d7}{})", error, count));
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
/// assert!(section.contains("Detected Anti-Patterns"));
/// ```
#[must_use]
pub fn build_antipattern_section(patterns: &[AntiPattern]) -> String {
    if patterns.is_empty() {
        return String::new();
    }

    let mut lines = vec![
        "## \u{26a0}\u{fe0f} Detected Anti-Patterns".to_string(),
        String::new(),
    ];

    // Sort by severity (high first)
    let mut sorted: Vec<_> = patterns.iter().collect();
    sorted.sort_by(|a, b| b.severity.cmp(&a.severity));

    for pattern in sorted {
        let severity_icon = match pattern.severity {
            AntiPatternSeverity::High => "\u{1f534}",
            AntiPatternSeverity::Medium => "\u{1f7e1}",
            AntiPatternSeverity::Low => "\u{1f535}",
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
            lines.push(format!("**\u{1f4a1} Remediation:** {}", remediation));
        }

        if pattern.persistence_count > 1 {
            lines.push(format!(
                "\u{26a0}\u{fe0f} This pattern has persisted for {} iterations",
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::prompt::context::{AntiPatternType, ErrorSeverity, GateResult};

    // build_error_section tests

    #[test]
    fn test_build_error_section_empty() {
        let errors: Vec<ErrorContext> = vec![];
        let section = build_error_section(&errors);
        assert!(section.is_empty());
    }

    #[test]
    fn test_build_error_section_single() {
        let errors = vec![ErrorContext::new(
            "E0308",
            "mismatched types",
            ErrorSeverity::Error,
        )];
        let section = build_error_section(&errors);

        assert!(section.contains("## Recent Errors"));
        assert!(section.contains("E0308"));
        assert!(section.contains("mismatched types"));
    }

    #[test]
    fn test_build_error_section_with_location() {
        let errors = vec![ErrorContext::new("E0308", "error", ErrorSeverity::Error)
            .with_location("src/lib.rs", 42)];
        let section = build_error_section(&errors);

        assert!(section.contains("`src/lib.rs:42`"));
    }

    #[test]
    fn test_build_error_section_with_suggested_fix() {
        let errors = vec![ErrorContext::new("E0308", "error", ErrorSeverity::Error)
            .with_suggested_fix("Change type to String")];
        let section = build_error_section(&errors);

        assert!(section.contains("Change type to String"));
    }

    #[test]
    fn test_build_error_section_recurring() {
        let errors =
            vec![ErrorContext::new("E0308", "error", ErrorSeverity::Error).with_occurrences(5)];
        let section = build_error_section(&errors);

        assert!(section.contains("5)")); // Recurrence indicator
    }

    #[test]
    fn test_build_error_section_sorted_by_frequency() {
        let errors = vec![
            ErrorContext::new("E0001", "less frequent", ErrorSeverity::Error).with_occurrences(1),
            ErrorContext::new("E0002", "most frequent", ErrorSeverity::Error).with_occurrences(10),
            ErrorContext::new("E0003", "medium", ErrorSeverity::Error).with_occurrences(5),
        ];
        let section = build_error_section(&errors);

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
        let section = build_error_section(&errors);

        // Should contain all three severity levels
        assert!(section.contains("E0001"));
        assert!(section.contains("W0001"));
        assert!(section.contains("I0001"));
    }

    // build_quality_section tests

    #[test]
    fn test_build_quality_section_unchecked() {
        let status = QualityGateStatus::new(); // No timestamp
        let section = build_quality_section(&status);
        assert!(section.is_empty());
    }

    #[test]
    fn test_build_quality_section_all_passing() {
        let status = QualityGateStatus::all_passing();
        let section = build_quality_section(&status);

        assert!(section.contains("## Quality Gates"));
        assert!(section.contains("**Clippy**"));
        assert!(section.contains("**Tests**"));
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

        let section = build_quality_section(&status);

        assert!(section.contains("**Clippy**"));
        assert!(section.contains("**Tests**"));
        assert!(section.contains("test_foo failed"));
    }

    // build_session_section tests

    #[test]
    fn test_build_session_section_basic() {
        let stats = SessionStats::new(5, 2, 150);
        let section = build_session_section(&stats);

        assert!(section.contains("## Session Progress"));
        assert!(section.contains("**Iterations:** 5"));
        assert!(section.contains("**Commits:** 2"));
        assert!(section.contains("**Lines changed:** 150"));
    }

    #[test]
    fn test_build_session_section_with_budget() {
        let stats = SessionStats::new(8, 3, 200).with_budget(10);
        let section = build_session_section(&stats);

        assert!(section.contains("80%")); // 8/10 = 80%
    }

    #[test]
    fn test_build_session_section_budget_critical() {
        let stats = SessionStats::new(9, 3, 200).with_budget(10);
        let section = build_session_section(&stats);

        assert!(section.contains("Budget critical"));
    }

    #[test]
    fn test_build_session_section_with_stagnation() {
        let stats = SessionStats::new(10, 2, 100).with_stagnation(4);
        let section = build_session_section(&stats);

        assert!(section.contains("Stagnation alert"));
        assert!(section.contains("4 iterations without progress"));
    }

    #[test]
    fn test_build_session_section_low_commit_rate() {
        let stats = SessionStats::new(10, 1, 100); // 1 commit in 10 iterations
        let section = build_session_section(&stats);

        assert!(section.contains("Low commit rate"));
    }

    #[test]
    fn test_build_session_section_test_delta() {
        let stats = SessionStats::new(5, 2, 100).with_test_delta(5);
        let section = build_session_section(&stats);

        assert!(section.contains("**Test count delta:** +5"));

        let stats_neg = SessionStats::new(5, 2, 100).with_test_delta(-3);
        let section_neg = build_session_section(&stats_neg);

        assert!(section_neg.contains("**Test count delta:** -3"));
    }

    // build_attempt_section tests

    #[test]
    fn test_build_attempt_section_empty() {
        let attempts: Vec<AttemptSummary> = vec![];
        let section = build_attempt_section(&attempts);
        assert!(section.is_empty());
    }

    #[test]
    fn test_build_attempt_section_single() {
        let attempts = vec![AttemptSummary::new(1, AttemptOutcome::TestFailure)
            .with_approach("TDD approach")
            .with_error("test_foo failed")];

        let section = build_attempt_section(&attempts);

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

        let section = build_attempt_section(&attempts);

        assert!(section.contains("Success"));
        assert!(section.contains("Compilation Error"));
        assert!(section.contains("Test Failure"));
    }

    #[test]
    fn test_build_attempt_section_pattern_analysis() {
        let attempts = vec![
            AttemptSummary::new(1, AttemptOutcome::TestFailure).with_error("recurring error"),
            AttemptSummary::new(2, AttemptOutcome::TestFailure).with_error("recurring error"),
        ];

        let section = build_attempt_section(&attempts);

        assert!(section.contains("Pattern Analysis"));
        assert!(section.contains("Recurring errors"));
    }

    // build_antipattern_section tests

    #[test]
    fn test_build_antipattern_section_empty() {
        let patterns: Vec<AntiPattern> = vec![];
        let section = build_antipattern_section(&patterns);
        assert!(section.is_empty());
    }

    #[test]
    fn test_build_antipattern_section_single() {
        let patterns = vec![
            AntiPattern::new(AntiPatternType::EditWithoutCommit, "5 files edited")
                .with_remediation("Make incremental commits"),
        ];

        let section = build_antipattern_section(&patterns);

        assert!(section.contains("Detected Anti-Patterns"));
        assert!(section.contains("Edit Without Commit"));
        assert!(section.contains("5 files edited"));
        assert!(section.contains("Make incremental commits"));
    }

    #[test]
    fn test_build_antipattern_section_severity_sorted() {
        let patterns = vec![
            AntiPattern::new(AntiPatternType::ClippyNotRun, "low")
                .with_severity(AntiPatternSeverity::Low),
            AntiPattern::new(AntiPatternType::TaskOscillation, "high")
                .with_severity(AntiPatternSeverity::High),
            AntiPattern::new(AntiPatternType::TestsNotRun, "medium")
                .with_severity(AntiPatternSeverity::Medium),
        ];

        let section = build_antipattern_section(&patterns);

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

        let section = build_antipattern_section(&patterns);

        assert!(section.contains("**Evidence:**"));
        assert!(section.contains("E0308 at line 10"));
    }

    #[test]
    fn test_build_antipattern_section_persistence() {
        let patterns = vec![
            AntiPattern::new(AntiPatternType::FileChurn, "Churn detected").with_persistence(5),
        ];

        let section = build_antipattern_section(&patterns);

        assert!(section.contains("persisted for 5 iterations"));
    }

    // build_history_section tests

    #[test]
    fn test_build_history_section_empty() {
        let guidance: Vec<String> = vec![];
        let section = build_history_section(&guidance);
        assert!(section.is_empty());
    }

    #[test]
    fn test_build_history_section_with_items() {
        let guidance = vec![
            "TDD approach worked well".to_string(),
            "Avoid direct manipulation".to_string(),
        ];

        let section = build_history_section(&guidance);

        assert!(section.contains("## Historical Guidance"));
        assert!(section.contains("TDD approach worked well"));
        assert!(section.contains("Avoid direct manipulation"));
    }
}
