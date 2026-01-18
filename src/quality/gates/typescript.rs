//! TypeScript/JavaScript-specific quality gate implementations.
//!
//! This module provides quality gates for TypeScript and JavaScript projects
//! using standard tooling:
//! - [`EslintGate`] - Runs `eslint` linter
//! - [`JestGate`] - Runs `jest` test suite (with vitest/mocha detection)
//! - [`TscGate`] - Runs `tsc` TypeScript type checker
//! - [`NpmAuditGate`] - Runs `npm audit` security scanner

use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::Result;

use super::{GateIssue, IssueSeverity, QualityGate};

// ============================================================================
// Public Factory Function
// ============================================================================

/// Returns all standard quality gates for TypeScript/JavaScript projects.
///
/// The returned gates include:
/// - ESLint (linting)
/// - Jest (unit/integration tests, with vitest/mocha fallback)
/// - Tsc (TypeScript type checking)
/// - npm audit (security scanning)
#[must_use]
pub fn typescript_gates() -> Vec<Box<dyn QualityGate>> {
    vec![
        Box::new(EslintGate::new()),
        Box::new(JestGate::new()),
        Box::new(TscGate::new()),
        Box::new(NpmAuditGate::new()),
    ]
}

// ============================================================================
// ESLint Gate
// ============================================================================

/// Quality gate that runs `eslint` linter.
///
/// ESLint is the standard JavaScript/TypeScript linter. It supports custom
/// configurations via `.eslintrc.*` files or `eslint.config.*` (flat config).
pub struct EslintGate {
    /// Additional arguments to pass to eslint.
    extra_args: Vec<String>,
}

impl EslintGate {
    /// Create a new eslint gate with default configuration.
    #[must_use]
    pub fn new() -> Self {
        Self {
            extra_args: Vec::new(),
        }
    }

    /// Create with additional arguments.
    #[must_use]
    pub fn with_args(mut self, args: Vec<String>) -> Self {
        self.extra_args = args;
        self
    }

    /// Parse ESLint JSON output into gate issues.
    fn parse_json_output(&self, stdout: &str) -> Vec<GateIssue> {
        let mut issues = Vec::new();

        // ESLint JSON format: [{filePath, messages: [{ruleId, severity, message, line, column}]}]
        if let Ok(files) = serde_json::from_str::<Vec<EslintFileResult>>(stdout) {
            for file in files {
                for msg in file.messages {
                    let severity = match msg.severity {
                        2 => IssueSeverity::Error,
                        1 => IssueSeverity::Warning,
                        _ => IssueSeverity::Info,
                    };

                    let mut issue = GateIssue::new(severity, &msg.message);

                    if let Some(line) = msg.line {
                        issue = issue.with_location(&file.file_path, line);
                        if let Some(col) = msg.column {
                            issue = issue.with_column(col);
                        }
                    } else {
                        issue.file = Some(PathBuf::from(&file.file_path));
                    }

                    if let Some(ref rule_id) = msg.rule_id {
                        issue = issue.with_code(rule_id);
                    }

                    if let Some(ref fix) = msg.fix {
                        if let Some(ref text) = fix.text {
                            issue = issue.with_suggestion(format!("Replace with: {}", text));
                        }
                    }

                    issues.push(issue);
                }
            }
        }

        issues
    }
}

impl Default for EslintGate {
    fn default() -> Self {
        Self::new()
    }
}

impl QualityGate for EslintGate {
    fn name(&self) -> &str {
        "ESLint"
    }

    fn run(&self, project_dir: &Path) -> Result<Vec<GateIssue>> {
        // Try npx eslint first (works with local installations)
        let mut args = vec!["eslint", ".", "--format=json"];
        let extra_refs: Vec<&str> = self.extra_args.iter().map(|s| s.as_str()).collect();
        args.extend(extra_refs);

        let output = Command::new("npx")
            .args(&args)
            .current_dir(project_dir)
            .output();

        match output {
            Ok(out) => {
                let stdout = String::from_utf8_lossy(&out.stdout);

                // ESLint returns exit code 1 if there are errors
                let issues = self.parse_json_output(&stdout);
                if issues.is_empty() && !out.status.success() && !stdout.is_empty() {
                    // JSON parsing failed but there was output
                    Ok(vec![GateIssue::new(
                        IssueSeverity::Error,
                        format!(
                            "ESLint reported issues: {}",
                            stdout.lines().next().unwrap_or("")
                        ),
                    )])
                } else {
                    Ok(issues)
                }
            }
            Err(_) => {
                // ESLint not available
                Ok(vec![GateIssue::new(
                    IssueSeverity::Warning,
                    "ESLint not available (run `npm install eslint`)",
                )])
            }
        }
    }

    fn remediation(&self, issues: &[GateIssue]) -> String {
        let error_count = issues
            .iter()
            .filter(|i| i.severity == IssueSeverity::Error)
            .count();
        let warning_count = issues
            .iter()
            .filter(|i| i.severity == IssueSeverity::Warning)
            .count();

        format!(
            r#"## ESLint Issues

Found {} errors and {} warnings.

**How to fix:**
1. Run `npx eslint . --fix` to auto-fix many issues
2. For remaining issues, see the ESLint rule documentation
3. Run `npx eslint .` to verify all fixed

**Common fixes:**
- Unused variables: remove or use the variable
- Missing semicolons: add or configure your style
- Prefer const: use const for variables that aren't reassigned
"#,
            error_count, warning_count
        )
    }
}

// ============================================================================
// Jest Gate
// ============================================================================

/// Quality gate that runs `jest` test suite.
///
/// Supports fallback to vitest or mocha if jest is not available.
pub struct JestGate {
    /// Additional arguments to pass to jest.
    extra_args: Vec<String>,
}

impl JestGate {
    /// Create a new jest gate with default configuration.
    #[must_use]
    pub fn new() -> Self {
        Self {
            extra_args: Vec::new(),
        }
    }

    /// Create with additional arguments.
    #[must_use]
    pub fn with_args(mut self, args: Vec<String>) -> Self {
        self.extra_args = args;
        self
    }

    /// Parse Jest JSON output into gate issues.
    fn parse_json_output(&self, stdout: &str) -> Vec<GateIssue> {
        let mut issues = Vec::new();

        if let Ok(result) = serde_json::from_str::<JestResult>(stdout) {
            for test_result in result.test_results {
                if test_result.status == "failed" {
                    for assertion in test_result.assertion_results {
                        if assertion.status == "failed" {
                            let message = assertion
                                .failure_messages
                                .first()
                                .map(|s| s.lines().next().unwrap_or(s).to_string())
                                .unwrap_or_else(|| "Test failed".to_string());

                            let mut issue =
                                GateIssue::new(IssueSeverity::Error, format!("{}: {}", assertion.title, message))
                                    .with_code("jest_failure");

                            issue.file = Some(PathBuf::from(&test_result.name));
                            issues.push(issue);
                        }
                    }
                }
            }
        }

        issues
    }

    /// Parse plain text test output (fallback).
    fn parse_text_output(&self, stdout: &str, stderr: &str) -> Vec<GateIssue> {
        let mut issues = Vec::new();
        let combined = format!("{}\n{}", stdout, stderr);

        // Look for FAIL lines
        for line in combined.lines() {
            if line.starts_with("FAIL ") || line.contains("✕") || line.contains("✗") {
                issues.push(
                    GateIssue::new(IssueSeverity::Error, line.trim().to_string())
                        .with_code("test_failure"),
                );
            }
        }

        issues
    }

    /// Try running vitest as fallback.
    fn run_vitest(&self, project_dir: &Path) -> Result<Vec<GateIssue>> {
        let output = Command::new("npx")
            .args(["vitest", "run", "--reporter=verbose"])
            .current_dir(project_dir)
            .output();

        match output {
            Ok(out) => {
                if out.status.success() {
                    Ok(Vec::new())
                } else {
                    let stdout = String::from_utf8_lossy(&out.stdout);
                    let stderr = String::from_utf8_lossy(&out.stderr);
                    let issues = self.parse_text_output(&stdout, &stderr);
                    if issues.is_empty() {
                        Ok(vec![GateIssue::new(
                            IssueSeverity::Error,
                            "Vitest failed (run `npx vitest` for details)",
                        )
                        .with_code("vitest_failure")])
                    } else {
                        Ok(issues)
                    }
                }
            }
            Err(_) => self.run_mocha(project_dir),
        }
    }

    /// Try running mocha as fallback.
    fn run_mocha(&self, project_dir: &Path) -> Result<Vec<GateIssue>> {
        let output = Command::new("npx")
            .args(["mocha", "--reporter", "spec"])
            .current_dir(project_dir)
            .output();

        match output {
            Ok(out) => {
                if out.status.success() {
                    Ok(Vec::new())
                } else {
                    let stdout = String::from_utf8_lossy(&out.stdout);
                    let stderr = String::from_utf8_lossy(&out.stderr);
                    let issues = self.parse_text_output(&stdout, &stderr);
                    if issues.is_empty() {
                        Ok(vec![GateIssue::new(
                            IssueSeverity::Error,
                            "Mocha failed (run `npx mocha` for details)",
                        )
                        .with_code("mocha_failure")])
                    } else {
                        Ok(issues)
                    }
                }
            }
            Err(_) => Ok(vec![GateIssue::new(
                IssueSeverity::Warning,
                "No test runner available (install jest, vitest, or mocha)",
            )]),
        }
    }
}

impl Default for JestGate {
    fn default() -> Self {
        Self::new()
    }
}

impl QualityGate for JestGate {
    fn name(&self) -> &str {
        "Jest"
    }

    fn run(&self, project_dir: &Path) -> Result<Vec<GateIssue>> {
        let mut args = vec!["jest", "--json", "--passWithNoTests"];
        let extra_refs: Vec<&str> = self.extra_args.iter().map(|s| s.as_str()).collect();
        args.extend(extra_refs);

        let output = Command::new("npx")
            .args(&args)
            .current_dir(project_dir)
            .output();

        match output {
            Ok(out) => {
                let stdout = String::from_utf8_lossy(&out.stdout);

                if out.status.success() {
                    Ok(Vec::new())
                } else {
                    let issues = self.parse_json_output(&stdout);
                    if issues.is_empty() {
                        let stderr = String::from_utf8_lossy(&out.stderr);
                        let text_issues = self.parse_text_output(&stdout, &stderr);
                        if text_issues.is_empty() {
                            Ok(vec![GateIssue::new(
                                IssueSeverity::Error,
                                "Jest failed (run `npx jest` for details)",
                            )
                            .with_code("jest_failure")])
                        } else {
                            Ok(text_issues)
                        }
                    } else {
                        Ok(issues)
                    }
                }
            }
            Err(_) => {
                // Jest not available - try vitest
                self.run_vitest(project_dir)
            }
        }
    }

    fn remediation(&self, issues: &[GateIssue]) -> String {
        let failed_tests: Vec<_> = issues
            .iter()
            .filter(|i| {
                i.code.as_deref() == Some("jest_failure")
                    || i.code.as_deref() == Some("vitest_failure")
                    || i.code.as_deref() == Some("mocha_failure")
                    || i.code.as_deref() == Some("test_failure")
            })
            .filter_map(|i| i.file.as_ref().map(|f| f.display().to_string()))
            .collect();

        format!(
            r#"## Test Failures

{} test(s) failed.

**Failed test files:**
{}

**How to fix:**
1. Run `npx jest --verbose` to see detailed failure output
2. Fix each failing test
3. Run `npx jest` to verify all pass
"#,
            issues.len(),
            if failed_tests.is_empty() {
                "- (unable to determine specific files)".to_string()
            } else {
                failed_tests
                    .iter()
                    .map(|t| format!("- {}", t))
                    .collect::<Vec<_>>()
                    .join("\n")
            }
        )
    }
}

// ============================================================================
// Tsc Gate
// ============================================================================

/// Quality gate that runs `tsc` TypeScript type checker.
pub struct TscGate {
    /// Additional arguments to pass to tsc.
    extra_args: Vec<String>,
}

impl TscGate {
    /// Create a new tsc gate with default configuration.
    #[must_use]
    pub fn new() -> Self {
        Self {
            extra_args: Vec::new(),
        }
    }

    /// Create with additional arguments.
    #[must_use]
    pub fn with_args(mut self, args: Vec<String>) -> Self {
        self.extra_args = args;
        self
    }

    /// Parse tsc output into gate issues.
    fn parse_output(&self, stdout: &str) -> Vec<GateIssue> {
        let mut issues = Vec::new();

        // tsc output format: path.ts(10,5): error TS2322: Message
        for line in stdout.lines() {
            if let Some(issue) = Self::parse_tsc_line(line) {
                issues.push(issue);
            }
        }

        issues
    }

    /// Parse a single tsc output line.
    fn parse_tsc_line(line: &str) -> Option<GateIssue> {
        // Format: path/to/file.ts(10,5): error TS2322: Message
        let paren_pos = line.find('(')?;
        let file = &line[..paren_pos];

        let loc_end = line.find(')')?;
        let loc = &line[paren_pos + 1..loc_end];
        let parts: Vec<&str> = loc.split(',').collect();
        let row: u32 = parts.first()?.parse().ok()?;
        let col: u32 = parts.get(1).and_then(|s| s.parse().ok()).unwrap_or(1);

        let rest = &line[loc_end + 2..]; // Skip "): "

        // Parse severity and code
        let (severity, code, message) = if let Some(err_pos) = rest.find("error TS") {
            let after_error = &rest[err_pos + 6..];
            let colon_pos = after_error.find(':')?;
            let code = after_error[..colon_pos].to_string();
            let message = after_error[colon_pos + 1..].trim().to_string();
            (IssueSeverity::Error, Some(code), message)
        } else if let Some(warn_pos) = rest.find("warning TS") {
            let after_warn = &rest[warn_pos + 8..];
            let colon_pos = after_warn.find(':')?;
            let code = after_warn[..colon_pos].to_string();
            let message = after_warn[colon_pos + 1..].trim().to_string();
            (IssueSeverity::Warning, Some(code), message)
        } else {
            return None;
        };

        let mut issue = GateIssue::new(severity, message)
            .with_location(file, row)
            .with_column(col);

        if let Some(c) = code {
            issue = issue.with_code(c);
        }

        Some(issue)
    }
}

impl Default for TscGate {
    fn default() -> Self {
        Self::new()
    }
}

impl QualityGate for TscGate {
    fn name(&self) -> &str {
        "TypeScript"
    }

    fn run(&self, project_dir: &Path) -> Result<Vec<GateIssue>> {
        let mut args = vec!["tsc", "--noEmit"];
        let extra_refs: Vec<&str> = self.extra_args.iter().map(|s| s.as_str()).collect();
        args.extend(extra_refs);

        let output = Command::new("npx")
            .args(&args)
            .current_dir(project_dir)
            .output();

        match output {
            Ok(out) => {
                let stdout = String::from_utf8_lossy(&out.stdout);

                if out.status.success() {
                    Ok(Vec::new())
                } else {
                    let issues = self.parse_output(&stdout);
                    if issues.is_empty() && !stdout.is_empty() {
                        Ok(vec![GateIssue::new(
                            IssueSeverity::Error,
                            "TypeScript reported type errors (run `npx tsc --noEmit` for details)",
                        )])
                    } else {
                        Ok(issues)
                    }
                }
            }
            Err(_) => {
                // TypeScript not installed - this is non-blocking for JS projects
                Ok(Vec::new())
            }
        }
    }

    fn is_blocking(&self) -> bool {
        true
    }

    fn remediation(&self, issues: &[GateIssue]) -> String {
        let error_count = issues
            .iter()
            .filter(|i| i.severity == IssueSeverity::Error)
            .count();

        format!(
            r#"## TypeScript Type Errors

Found {} type errors.

**How to fix:**
1. Run `npx tsc --noEmit` to see all type errors
2. Add proper type annotations to fix inference issues
3. Use `// @ts-ignore` sparingly for false positives
4. Consider using `any` type temporarily for complex migrations

**Common fixes:**
- Type 'X' is not assignable: ensure types match or add type assertions
- Property does not exist: check spelling or extend the interface
- Could not find declaration file: install @types/package-name
"#,
            error_count
        )
    }
}

// ============================================================================
// npm Audit Gate
// ============================================================================

/// Quality gate that runs `npm audit` security scanner.
pub struct NpmAuditGate {
    /// Minimum severity to report.
    severity_threshold: NpmSeverity,
    /// Additional arguments to pass to npm audit.
    extra_args: Vec<String>,
}

/// npm audit severity levels.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum NpmSeverity {
    /// Low severity issues.
    Low,
    /// Moderate severity issues.
    Moderate,
    /// High severity issues.
    High,
    /// Critical severity issues.
    Critical,
}

impl NpmAuditGate {
    /// Create a new npm audit gate with default configuration.
    #[must_use]
    pub fn new() -> Self {
        Self {
            severity_threshold: NpmSeverity::Moderate,
            extra_args: Vec::new(),
        }
    }

    /// Set the minimum severity threshold.
    #[must_use]
    pub fn with_threshold(mut self, threshold: NpmSeverity) -> Self {
        self.severity_threshold = threshold;
        self
    }

    /// Create with additional arguments.
    #[must_use]
    pub fn with_args(mut self, args: Vec<String>) -> Self {
        self.extra_args = args;
        self
    }

    /// Parse npm audit JSON output into gate issues.
    fn parse_json_output(&self, stdout: &str) -> Vec<GateIssue> {
        let mut issues = Vec::new();

        if let Ok(report) = serde_json::from_str::<NpmAuditReport>(stdout) {
            for (name, vuln) in report.vulnerabilities {
                let severity = match vuln.severity.to_lowercase().as_str() {
                    "critical" => IssueSeverity::Critical,
                    "high" => IssueSeverity::Error,
                    "moderate" => IssueSeverity::Warning,
                    "low" => IssueSeverity::Info,
                    _ => IssueSeverity::Info,
                };

                // Filter by threshold
                let npm_sev = match vuln.severity.to_lowercase().as_str() {
                    "critical" => NpmSeverity::Critical,
                    "high" => NpmSeverity::High,
                    "moderate" => NpmSeverity::Moderate,
                    _ => NpmSeverity::Low,
                };

                if npm_sev < self.severity_threshold {
                    continue;
                }

                let message = format!(
                    "{}: {} (via {})",
                    name,
                    vuln.via
                        .first()
                        .map(|v| v.title.as_deref().unwrap_or("unknown"))
                        .unwrap_or("unknown vulnerability"),
                    vuln.via
                        .iter()
                        .filter_map(|v| v.name.as_deref())
                        .collect::<Vec<_>>()
                        .join(" -> ")
                );

                let mut issue = GateIssue::new(severity, message);

                if let Some(first_via) = vuln.via.first() {
                    if let Some(ref url) = first_via.url {
                        issue = issue.with_suggestion(format!("See: {}", url));
                    }
                }

                if vuln.fix_available {
                    issue = issue.with_code("npm-fix-available");
                }

                issues.push(issue);
            }
        }

        issues
    }
}

impl Default for NpmAuditGate {
    fn default() -> Self {
        Self::new()
    }
}

impl QualityGate for NpmAuditGate {
    fn name(&self) -> &str {
        "npm-audit"
    }

    fn run(&self, project_dir: &Path) -> Result<Vec<GateIssue>> {
        let mut args = vec!["audit", "--json"];
        let extra_refs: Vec<&str> = self.extra_args.iter().map(|s| s.as_str()).collect();
        args.extend(extra_refs);

        let output = Command::new("npm")
            .args(&args)
            .current_dir(project_dir)
            .output();

        match output {
            Ok(out) => {
                let stdout = String::from_utf8_lossy(&out.stdout);

                // npm audit returns exit code 1 if there are vulnerabilities
                let issues = self.parse_json_output(&stdout);
                Ok(issues)
            }
            Err(_) => {
                // npm not available
                Ok(vec![GateIssue::new(
                    IssueSeverity::Warning,
                    "npm not available",
                )])
            }
        }
    }

    fn remediation(&self, issues: &[GateIssue]) -> String {
        let critical = issues
            .iter()
            .filter(|i| i.severity == IssueSeverity::Critical)
            .count();
        let high = issues
            .iter()
            .filter(|i| i.severity == IssueSeverity::Error)
            .count();
        let fixable = issues
            .iter()
            .filter(|i| i.code.as_deref() == Some("npm-fix-available"))
            .count();

        format!(
            r#"## npm Audit Security Issues

Found {} critical and {} high severity vulnerabilities.
{} can be automatically fixed.

**How to fix:**
1. Run `npm audit fix` to automatically fix compatible vulnerabilities
2. Run `npm audit fix --force` to fix breaking changes (use with caution)
3. For remaining issues, manually update or replace vulnerable packages

**Manual fixes:**
- Check each vulnerability URL for specific remediation steps
- Consider alternative packages if no fix is available
- Add to allow-list only if vulnerability doesn't apply to your use case
"#,
            critical,
            high,
            fixable
        )
    }
}

// ============================================================================
// JSON Parsing Structures
// ============================================================================

/// ESLint file result structure.
#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct EslintFileResult {
    file_path: String,
    messages: Vec<EslintMessage>,
}

/// ESLint message structure.
#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct EslintMessage {
    severity: u8,
    message: String,
    line: Option<u32>,
    column: Option<u32>,
    rule_id: Option<String>,
    fix: Option<EslintFix>,
}

/// ESLint fix suggestion.
#[derive(Debug, serde::Deserialize)]
struct EslintFix {
    text: Option<String>,
}

/// Jest result structure.
#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct JestResult {
    test_results: Vec<JestTestResult>,
}

/// Jest test result.
#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct JestTestResult {
    name: String,
    status: String,
    assertion_results: Vec<JestAssertionResult>,
}

/// Jest assertion result.
#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct JestAssertionResult {
    title: String,
    status: String,
    failure_messages: Vec<String>,
}

/// npm audit report structure.
#[derive(Debug, serde::Deserialize)]
struct NpmAuditReport {
    vulnerabilities: std::collections::HashMap<String, NpmVulnerability>,
}

/// npm vulnerability entry.
#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct NpmVulnerability {
    severity: String,
    via: Vec<NpmVia>,
    fix_available: bool,
}

/// npm via entry (describes vulnerability source).
#[derive(Debug, serde::Deserialize)]
struct NpmVia {
    name: Option<String>,
    title: Option<String>,
    url: Option<String>,
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    // =========================================================================
    // EslintGate tests
    // =========================================================================

    #[test]
    fn test_eslint_gate_name() {
        let gate = EslintGate::new();
        assert_eq!(gate.name(), "ESLint");
    }

    #[test]
    fn test_eslint_gate_is_blocking() {
        let gate = EslintGate::new();
        assert!(gate.is_blocking());
    }

    #[test]
    fn test_eslint_gate_default() {
        let gate = EslintGate::default();
        assert_eq!(gate.name(), "ESLint");
    }

    #[test]
    fn test_eslint_gate_remediation() {
        let gate = EslintGate::new();
        let issues = vec![
            GateIssue::new(IssueSeverity::Error, "error"),
            GateIssue::new(IssueSeverity::Warning, "warning"),
        ];
        let remediation = gate.remediation(&issues);
        assert!(remediation.contains("ESLint"));
        assert!(remediation.contains("1 errors"));
        assert!(remediation.contains("1 warnings"));
    }

    #[test]
    fn test_eslint_parse_json_output() {
        let gate = EslintGate::new();

        let json = r#"[
            {
                "filePath": "src/main.ts",
                "messages": [
                    {
                        "severity": 2,
                        "message": "Unexpected console statement",
                        "line": 10,
                        "column": 5,
                        "ruleId": "no-console",
                        "fix": {"text": "// console.log()"}
                    },
                    {
                        "severity": 1,
                        "message": "Prefer const",
                        "line": 15,
                        "column": 1,
                        "ruleId": "prefer-const",
                        "fix": null
                    }
                ]
            }
        ]"#;

        let issues = gate.parse_json_output(json);
        assert_eq!(issues.len(), 2);

        assert_eq!(issues[0].severity, IssueSeverity::Error);
        assert!(issues[0].message.contains("console"));
        assert_eq!(issues[0].code, Some("no-console".to_string()));
        assert_eq!(issues[0].line, Some(10));
        assert_eq!(issues[0].column, Some(5));

        assert_eq!(issues[1].severity, IssueSeverity::Warning);
        assert_eq!(issues[1].line, Some(15));
    }

    // =========================================================================
    // JestGate tests
    // =========================================================================

    #[test]
    fn test_jest_gate_name() {
        let gate = JestGate::new();
        assert_eq!(gate.name(), "Jest");
    }

    #[test]
    fn test_jest_gate_is_blocking() {
        let gate = JestGate::new();
        assert!(gate.is_blocking());
    }

    #[test]
    fn test_jest_gate_default() {
        let gate = JestGate::default();
        assert_eq!(gate.name(), "Jest");
    }

    #[test]
    fn test_jest_parse_json_output() {
        let gate = JestGate::new();

        let json = r#"{
            "testResults": [
                {
                    "name": "tests/foo.test.ts",
                    "status": "failed",
                    "assertionResults": [
                        {
                            "title": "should add numbers",
                            "status": "failed",
                            "failureMessages": ["Expected 3 but got 2\n    at line 10"]
                        },
                        {
                            "title": "should subtract numbers",
                            "status": "passed",
                            "failureMessages": []
                        }
                    ]
                }
            ]
        }"#;

        let issues = gate.parse_json_output(json);
        assert_eq!(issues.len(), 1);

        assert_eq!(issues[0].severity, IssueSeverity::Error);
        assert!(issues[0].message.contains("should add numbers"));
        assert!(issues[0].message.contains("Expected 3 but got 2"));
        assert_eq!(issues[0].file, Some(PathBuf::from("tests/foo.test.ts")));
    }

    #[test]
    fn test_jest_parse_text_output() {
        let gate = JestGate::new();

        let stdout = r#"
FAIL src/foo.test.ts
  ✕ should work (5ms)
"#;

        let issues = gate.parse_text_output(stdout, "");
        assert!(!issues.is_empty());
    }

    #[test]
    fn test_jest_gate_remediation() {
        let gate = JestGate::new();
        let issues = vec![GateIssue::new(IssueSeverity::Error, "Test failed")
            .with_code("jest_failure")];
        let remediation = gate.remediation(&issues);
        assert!(remediation.contains("Test Failures"));
        assert!(remediation.contains("1 test(s) failed"));
    }

    // =========================================================================
    // TscGate tests
    // =========================================================================

    #[test]
    fn test_tsc_gate_name() {
        let gate = TscGate::new();
        assert_eq!(gate.name(), "TypeScript");
    }

    #[test]
    fn test_tsc_gate_is_blocking() {
        let gate = TscGate::new();
        assert!(gate.is_blocking());
    }

    #[test]
    fn test_tsc_gate_default() {
        let gate = TscGate::default();
        assert_eq!(gate.name(), "TypeScript");
    }

    #[test]
    fn test_tsc_parse_output() {
        let gate = TscGate::new();

        let output = r#"src/main.ts(10,5): error TS2322: Type 'string' is not assignable to type 'number'.
src/utils.ts(20,1): error TS2304: Cannot find name 'foo'."#;

        let issues = gate.parse_output(output);
        assert_eq!(issues.len(), 2);

        assert_eq!(issues[0].severity, IssueSeverity::Error);
        assert!(issues[0].message.contains("Type 'string'"));
        assert_eq!(issues[0].code, Some("TS2322".to_string()));
        assert_eq!(issues[0].line, Some(10));
        assert_eq!(issues[0].column, Some(5));

        assert_eq!(issues[1].severity, IssueSeverity::Error);
        assert!(issues[1].message.contains("Cannot find name"));
        assert_eq!(issues[1].code, Some("TS2304".to_string()));
        assert_eq!(issues[1].line, Some(20));
    }

    #[test]
    fn test_tsc_gate_remediation() {
        let gate = TscGate::new();
        let issues = vec![GateIssue::new(IssueSeverity::Error, "type error")];
        let remediation = gate.remediation(&issues);
        assert!(remediation.contains("TypeScript"));
        assert!(remediation.contains("1 type errors"));
    }

    // =========================================================================
    // NpmAuditGate tests
    // =========================================================================

    #[test]
    fn test_npm_audit_gate_name() {
        let gate = NpmAuditGate::new();
        assert_eq!(gate.name(), "npm-audit");
    }

    #[test]
    fn test_npm_audit_gate_is_blocking() {
        let gate = NpmAuditGate::new();
        assert!(gate.is_blocking());
    }

    #[test]
    fn test_npm_audit_gate_default() {
        let gate = NpmAuditGate::default();
        assert_eq!(gate.name(), "npm-audit");
    }

    #[test]
    fn test_npm_audit_parse_json_output() {
        let gate = NpmAuditGate::new();

        let json = r#"{
            "vulnerabilities": {
                "lodash": {
                    "severity": "high",
                    "via": [
                        {
                            "name": "lodash",
                            "title": "Prototype Pollution",
                            "url": "https://npmjs.com/advisories/1234"
                        }
                    ],
                    "fixAvailable": true
                },
                "axios": {
                    "severity": "moderate",
                    "via": [
                        {
                            "name": "axios",
                            "title": "SSRF vulnerability",
                            "url": "https://npmjs.com/advisories/5678"
                        }
                    ],
                    "fixAvailable": false
                }
            }
        }"#;

        let issues = gate.parse_json_output(json);
        assert_eq!(issues.len(), 2);

        // High severity should come through
        let high_issue = issues.iter().find(|i| i.severity == IssueSeverity::Error);
        assert!(high_issue.is_some());
        let high = high_issue.unwrap();
        assert!(high.message.contains("lodash"));
        assert!(high.message.contains("Prototype Pollution"));
    }

    #[test]
    fn test_npm_audit_severity_threshold() {
        let gate = NpmAuditGate::new().with_threshold(NpmSeverity::High);

        let json = r#"{
            "vulnerabilities": {
                "pkg-high": {
                    "severity": "high",
                    "via": [{"name": "pkg", "title": "High issue", "url": null}],
                    "fixAvailable": true
                },
                "pkg-moderate": {
                    "severity": "moderate",
                    "via": [{"name": "pkg", "title": "Moderate issue", "url": null}],
                    "fixAvailable": false
                }
            }
        }"#;

        let issues = gate.parse_json_output(json);
        // Only HIGH severity should be included
        assert_eq!(issues.len(), 1);
        assert!(issues[0].message.contains("High issue"));
    }

    #[test]
    fn test_npm_audit_gate_remediation() {
        let gate = NpmAuditGate::new();
        let issues = vec![
            GateIssue::new(IssueSeverity::Critical, "critical issue"),
            GateIssue::new(IssueSeverity::Error, "high issue")
                .with_code("npm-fix-available"),
        ];
        let remediation = gate.remediation(&issues);
        assert!(remediation.contains("npm Audit"));
        assert!(remediation.contains("1 critical"));
        assert!(remediation.contains("1 high"));
        assert!(remediation.contains("1 can be automatically fixed"));
    }

    // =========================================================================
    // typescript_gates() factory tests
    // =========================================================================

    #[test]
    fn test_typescript_gates_returns_all_gates() {
        let gates = typescript_gates();
        let names: Vec<_> = gates.iter().map(|g| g.name()).collect();

        assert!(names.contains(&"ESLint"));
        assert!(names.contains(&"Jest"));
        assert!(names.contains(&"TypeScript"));
        assert!(names.contains(&"npm-audit"));
    }

    #[test]
    fn test_typescript_gates_are_send_sync() {
        let gates = typescript_gates();
        fn assert_send_sync<T: Send + Sync>(_: &T) {}
        for gate in &gates {
            assert_send_sync(gate);
        }
    }

    #[test]
    fn test_typescript_gates_have_remediation() {
        let gates = typescript_gates();
        for gate in &gates {
            let issues = vec![GateIssue::new(IssueSeverity::Error, "test error")];
            let remediation = gate.remediation(&issues);
            assert!(
                !remediation.is_empty(),
                "Gate {} should provide remediation",
                gate.name()
            );
        }
    }
}
