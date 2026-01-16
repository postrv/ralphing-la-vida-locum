//! Remediation guidance generation for quality gate failures.
//!
//! This module generates context-aware prompts to help fix quality gate failures.
//! The generated prompts can be injected into Claude's context to guide fixes.
//!
//! # Example
//!
//! ```rust,ignore
//! use ralph::quality::remediation::RemediationGenerator;
//! use ralph::quality::gates::GateResult;
//!
//! let generator = RemediationGenerator::new();
//! let failures = vec![clippy_failure, test_failure];
//! let prompt = generator.generate_prompt(&failures);
//!
//! // Inject into Claude's context
//! println!("{}", prompt);
//! ```

use super::gates::{GateIssue, GateResult, IssueSeverity};

// ============================================================================
// Remediation Generator
// ============================================================================

/// Configuration for remediation generation.
#[derive(Debug, Clone)]
pub struct RemediationConfig {
    /// Maximum number of issues to include per gate.
    pub max_issues_per_gate: usize,
    /// Maximum total characters in remediation prompt.
    pub max_prompt_chars: usize,
    /// Include raw output for debugging.
    pub include_raw_output: bool,
}

impl Default for RemediationConfig {
    fn default() -> Self {
        Self {
            max_issues_per_gate: 10,
            max_prompt_chars: 5000,
            include_raw_output: false,
        }
    }
}

/// Generates remediation guidance for quality gate failures.
pub struct RemediationGenerator {
    config: RemediationConfig,
}

impl RemediationGenerator {
    /// Create a new generator with default configuration.
    #[must_use]
    pub fn new() -> Self {
        Self {
            config: RemediationConfig::default(),
        }
    }

    /// Create a new generator with custom configuration.
    #[must_use]
    pub fn with_config(config: RemediationConfig) -> Self {
        Self { config }
    }

    /// Generate a remediation prompt for the given failures.
    #[must_use]
    pub fn generate_prompt(&self, failures: &[GateResult]) -> String {
        if failures.is_empty() {
            return String::new();
        }

        let mut prompt = String::new();

        prompt.push_str("# Quality Gate Failures - Fix Before Committing\n\n");
        prompt.push_str("The following quality gates failed. Fix each issue:\n\n");

        for (i, failure) in failures.iter().enumerate() {
            if prompt.len() > self.config.max_prompt_chars {
                prompt.push_str("\n... (additional failures truncated)\n");
                break;
            }

            prompt.push_str(&format!("## {}. {} Gate\n\n", i + 1, failure.gate_name));
            prompt.push_str(&self.format_gate_failure(failure));
            prompt.push('\n');
        }

        prompt.push_str("\n---\n\n");
        prompt.push_str("After fixing, run the quality gates again before committing.\n");

        prompt
    }

    /// Format a single gate failure with its issues and guidance.
    fn format_gate_failure(&self, failure: &GateResult) -> String {
        let mut section = String::new();

        // Summary
        let error_count = failure.count_by_severity(IssueSeverity::Error);
        let warning_count = failure.count_by_severity(IssueSeverity::Warning);
        let critical_count = failure.count_by_severity(IssueSeverity::Critical);

        let mut counts = Vec::new();
        if critical_count > 0 {
            counts.push(format!("{} critical", critical_count));
        }
        if error_count > 0 {
            counts.push(format!("{} errors", error_count));
        }
        if warning_count > 0 {
            counts.push(format!("{} warnings", warning_count));
        }

        section.push_str(&format!("**Issues**: {}\n\n", counts.join(", ")));

        // List issues (limited)
        let issues_to_show: Vec<_> = failure
            .issues
            .iter()
            .take(self.config.max_issues_per_gate)
            .collect();

        for issue in &issues_to_show {
            section.push_str(&self.format_issue(issue));
        }

        if failure.issues.len() > self.config.max_issues_per_gate {
            section.push_str(&format!(
                "\n... and {} more issues. Fix the above first.\n",
                failure.issues.len() - self.config.max_issues_per_gate
            ));
        }

        // Add gate-specific guidance
        section.push_str(&self.get_gate_guidance(&failure.gate_name));

        // Optionally include raw output
        if self.config.include_raw_output && !failure.raw_output.is_empty() {
            section.push_str("\n**Raw output**:\n```\n");
            let truncated: String = failure.raw_output.chars().take(1000).collect();
            section.push_str(&truncated);
            if failure.raw_output.len() > 1000 {
                section.push_str("\n... (truncated)");
            }
            section.push_str("\n```\n");
        }

        section
    }

    /// Format a single issue for display.
    fn format_issue(&self, issue: &GateIssue) -> String {
        let mut line = String::new();

        // Severity indicator
        let indicator = match issue.severity {
            IssueSeverity::Critical => "🚨",
            IssueSeverity::Error => "❌",
            IssueSeverity::Warning => "⚠️",
            IssueSeverity::Info => "ℹ️",
        };

        line.push_str(&format!("{} ", indicator));

        // Code if present
        if let Some(ref code) = issue.code {
            line.push_str(&format!("[{}] ", code));
        }

        // Message
        line.push_str(&issue.message);

        // Location
        if let Some(ref file) = issue.file {
            line.push_str(&format!("\n   at `{}", file.display()));
            if let Some(ln) = issue.line {
                line.push_str(&format!(":{}", ln));
                if let Some(col) = issue.column {
                    line.push_str(&format!(":{}", col));
                }
            }
            line.push('`');
        }

        // Suggestion
        if let Some(ref suggestion) = issue.suggestion {
            line.push_str(&format!("\n   💡 {}", suggestion));
        }

        line.push_str("\n\n");
        line
    }

    /// Get gate-specific remediation guidance.
    fn get_gate_guidance(&self, gate_name: &str) -> String {
        match gate_name {
            "Clippy" => r#"
### How to Fix Clippy Issues

1. Read each warning/error carefully
2. Apply the suggested fix OR justify why it's wrong
3. **NEVER use `#[allow(...)]` to silence warnings**
4. Run `cargo clippy --fix` for auto-fixable issues
5. For complex warnings, understand the underlying issue

Common patterns:
- `unused_*` → Remove unused code or prefix with `_`
- `unwrap_used` → Use `?` or handle the error properly
- `clone` warnings → Consider references instead
"#
            .to_string(),

            "Tests" => r#"
### How to Fix Test Failures

1. Read the failing test to understand expected behavior
2. **Fix the implementation, NOT the test**
3. Tests define correct behavior - trust them
4. Run `cargo test <test_name>` to verify your fix
5. If a test is genuinely wrong, explain why before changing it

Debug steps:
- Add `println!` or `dbg!()` to trace values
- Check test assertions for exact expectations
- Verify test setup and teardown
"#
            .to_string(),

            "NoAllow" => r#"
### How to Fix #[allow] Violations

1. Remove ALL `#[allow(...)]` annotations
2. Fix the underlying issue the annotation was hiding
3. If code is unused, **delete it**
4. If code is legitimately needed, write tests that use it

The #[allow] annotation is forbidden because it hides problems.
Always fix the root cause instead of suppressing warnings.
"#
            .to_string(),

            "Security" => r#"
### How to Fix Security Issues

1. Review each security finding carefully
2. **CRITICAL/HIGH issues must be fixed before commit**
3. Update vulnerable dependencies with `cargo update`
4. For code issues, apply the suggested remediation

Common fixes:
- Input validation and sanitization
- Use secure defaults
- Avoid unsafe code unless necessary
- Keep dependencies up to date
"#
            .to_string(),

            "NoTodo" => r#"
### How to Fix TODO/FIXME Comments

1. Either implement the TODO now, OR
2. Remove the code with the TODO if it's not needed
3. If blocked, document in IMPLEMENTATION_PLAN.md, not in code

TODOs in code indicate incomplete work. Either complete the work
or track it properly in the project plan.
"#
            .to_string(),

            _ => format!("Fix the issues reported by the {} gate.\n", gate_name),
        }
    }

    /// Generate a minimal remediation prompt focusing on the most critical issues.
    #[must_use]
    pub fn generate_minimal_prompt(&self, failures: &[GateResult]) -> String {
        if failures.is_empty() {
            return String::new();
        }

        let mut prompt = String::new();

        prompt.push_str("## Quality Issues to Fix\n\n");

        // Collect critical issues first
        let critical_issues: Vec<_> = failures
            .iter()
            .flat_map(|f| f.issues.iter().filter(|i| i.severity == IssueSeverity::Critical))
            .take(3)
            .collect();

        if !critical_issues.is_empty() {
            prompt.push_str("### 🚨 Critical (fix immediately)\n\n");
            for issue in critical_issues {
                prompt.push_str(&format!("- {}\n", issue.message));
            }
            prompt.push('\n');
        }

        // Then errors
        let error_issues: Vec<_> = failures
            .iter()
            .flat_map(|f| f.issues.iter().filter(|i| i.severity == IssueSeverity::Error))
            .take(5)
            .collect();

        if !error_issues.is_empty() {
            prompt.push_str("### ❌ Errors\n\n");
            for issue in error_issues {
                prompt.push_str(&format!("- {}\n", issue.message));
            }
        }

        prompt
    }
}

impl Default for RemediationGenerator {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================================
// Convenience Functions
// ============================================================================

/// Generate a remediation prompt for the given failures using default settings.
#[must_use]
pub fn generate_remediation_prompt(failures: &[GateResult]) -> String {
    RemediationGenerator::new().generate_prompt(failures)
}

/// Generate a minimal remediation prompt for the given failures.
#[must_use]
pub fn generate_minimal_remediation(failures: &[GateResult]) -> String {
    RemediationGenerator::new().generate_minimal_prompt(failures)
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    fn make_clippy_failure() -> GateResult {
        GateResult::fail(
            "Clippy",
            vec![
                GateIssue::new(IssueSeverity::Warning, "unused variable: `x`")
                    .with_location("src/lib.rs", 10)
                    .with_code("unused_variables")
                    .with_suggestion("prefix with underscore: `_x`"),
                GateIssue::new(IssueSeverity::Error, "mismatched types")
                    .with_location("src/main.rs", 20)
                    .with_code("E0308"),
            ],
        )
    }

    fn make_test_failure() -> GateResult {
        GateResult::fail(
            "Tests",
            vec![GateIssue::new(IssueSeverity::Error, "test_add failed")
                .with_code("test_failure")],
        )
    }

    fn make_no_allow_failure() -> GateResult {
        GateResult::fail(
            "NoAllow",
            vec![GateIssue::new(
                IssueSeverity::Error,
                "Forbidden #[allow(dead_code)] annotation",
            )
            .with_location("src/lib.rs", 5)
            .with_code("no_allow")],
        )
    }

    #[test]
    fn test_generator_empty_failures() {
        let generator = RemediationGenerator::new();
        let prompt = generator.generate_prompt(&[]);
        assert!(prompt.is_empty());
    }

    #[test]
    fn test_generator_single_failure() {
        let generator = RemediationGenerator::new();
        let failures = vec![make_clippy_failure()];
        let prompt = generator.generate_prompt(&failures);

        assert!(prompt.contains("Quality Gate Failures"));
        assert!(prompt.contains("Clippy Gate"));
        assert!(prompt.contains("unused variable"));
        assert!(prompt.contains("mismatched types"));
        assert!(prompt.contains("How to Fix Clippy Issues"));
    }

    #[test]
    fn test_generator_multiple_failures() {
        let generator = RemediationGenerator::new();
        let failures = vec![make_clippy_failure(), make_test_failure(), make_no_allow_failure()];
        let prompt = generator.generate_prompt(&failures);

        assert!(prompt.contains("Clippy Gate"));
        assert!(prompt.contains("Tests Gate"));
        assert!(prompt.contains("NoAllow Gate"));
    }

    #[test]
    fn test_generator_includes_suggestions() {
        let generator = RemediationGenerator::new();
        let failures = vec![make_clippy_failure()];
        let prompt = generator.generate_prompt(&failures);

        assert!(prompt.contains("prefix with underscore"));
    }

    #[test]
    fn test_generator_includes_locations() {
        let generator = RemediationGenerator::new();
        let failures = vec![make_clippy_failure()];
        let prompt = generator.generate_prompt(&failures);

        assert!(prompt.contains("src/lib.rs:10"));
        assert!(prompt.contains("src/main.rs:20"));
    }

    #[test]
    fn test_minimal_prompt() {
        let generator = RemediationGenerator::new();
        let failures = vec![
            GateResult::fail(
                "Security",
                vec![GateIssue::new(
                    IssueSeverity::Critical,
                    "SQL injection vulnerability",
                )],
            ),
            make_clippy_failure(),
        ];

        let prompt = generator.generate_minimal_prompt(&failures);

        assert!(prompt.contains("Critical"));
        assert!(prompt.contains("SQL injection"));
        assert!(prompt.contains("Errors"));
    }

    #[test]
    fn test_config_limits_issues() {
        let config = RemediationConfig {
            max_issues_per_gate: 1,
            ..Default::default()
        };
        let generator = RemediationGenerator::with_config(config);

        let failure = GateResult::fail(
            "Test",
            vec![
                GateIssue::new(IssueSeverity::Error, "error 1"),
                GateIssue::new(IssueSeverity::Error, "error 2"),
                GateIssue::new(IssueSeverity::Error, "error 3"),
            ],
        );

        let prompt = generator.generate_prompt(&[failure]);

        assert!(prompt.contains("error 1"));
        assert!(prompt.contains("2 more issues"));
    }

    #[test]
    fn test_gate_specific_guidance() {
        let generator = RemediationGenerator::new();

        let clippy = generator.get_gate_guidance("Clippy");
        assert!(clippy.contains("cargo clippy --fix"));
        assert!(clippy.contains("NEVER use `#[allow"));

        let tests = generator.get_gate_guidance("Tests");
        assert!(tests.contains("Fix the implementation"));
        assert!(tests.contains("NOT the test"));

        let no_allow = generator.get_gate_guidance("NoAllow");
        assert!(no_allow.contains("Remove ALL"));

        let security = generator.get_gate_guidance("Security");
        assert!(security.contains("CRITICAL/HIGH"));
    }

    #[test]
    fn test_convenience_functions() {
        let failures = vec![make_clippy_failure()];

        let full = generate_remediation_prompt(&failures);
        assert!(full.contains("Quality Gate Failures"));

        let minimal = generate_minimal_remediation(&failures);
        assert!(minimal.contains("Quality Issues"));
    }
}
