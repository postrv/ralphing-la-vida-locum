//! Language-specific code antipattern detection.
//!
//! This module provides detection of code antipatterns in source files.
//! Unlike the behavioral antipattern detection in `antipatterns.rs`, this
//! module detects problematic patterns in the actual source code.
//!
//! # Supported Languages
//!
//! - Python: bare except, mutable default args, global state
//! - TypeScript/JavaScript: any type, non-null assertion, console.log
//! - Go: ignored errors, empty interface abuse, panic in library code
//! - Rust: already covered by clippy
//!
//! # Example
//!
//! ```
//! use ralph::prompt::code_antipatterns::{antipatterns_for_language, CodeAntipatternDetector};
//! use ralph::Language;
//!
//! // Get antipattern rules for Python
//! let rules = antipatterns_for_language(Language::Python);
//! assert!(!rules.is_empty());
//!
//! // Create a detector and scan code
//! let detector = CodeAntipatternDetector::new();
//! let python_code = "except:  # bare except\n    pass";
//! let findings = detector.scan_code(python_code, Language::Python);
//! assert!(!findings.is_empty());
//! ```

use crate::Language;
use serde::{Deserialize, Serialize};
use std::path::Path;

/// A rule for detecting code antipatterns.
///
/// Each rule defines a pattern (typically a regex) that matches problematic
/// code, along with metadata describing the issue and remediation.
///
/// # Example
///
/// ```
/// use ralph::prompt::code_antipatterns::CodeAntipatternRule;
///
/// let rule = CodeAntipatternRule::new(
///     "bare-except",
///     "Bare except clause",
///     r"except\s*:",
/// )
/// .with_description("Catches all exceptions including KeyboardInterrupt and SystemExit")
/// .with_remediation("Use specific exception types: except ValueError:");
///
/// assert_eq!(rule.id, "bare-except");
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CodeAntipatternRule {
    /// Unique identifier for the rule.
    pub id: String,
    /// Short name of the antipattern.
    pub name: String,
    /// Regex pattern to match.
    pub pattern: String,
    /// Description of why this is problematic.
    pub description: String,
    /// Suggested fix or remediation.
    pub remediation: String,
    /// Severity level.
    pub severity: CodeAntipatternSeverity,
    /// Languages this rule applies to.
    pub languages: Vec<Language>,
}

impl CodeAntipatternRule {
    /// Create a new antipattern rule.
    ///
    /// # Example
    ///
    /// ```
    /// use ralph::prompt::code_antipatterns::CodeAntipatternRule;
    ///
    /// let rule = CodeAntipatternRule::new(
    ///     "any-type",
    ///     "Use of any type",
    ///     r":\s*any\b",
    /// );
    /// assert_eq!(rule.id, "any-type");
    /// ```
    #[must_use]
    pub fn new(id: impl Into<String>, name: impl Into<String>, pattern: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            name: name.into(),
            pattern: pattern.into(),
            description: String::new(),
            remediation: String::new(),
            severity: CodeAntipatternSeverity::Warning,
            languages: Vec::new(),
        }
    }

    /// Set the description.
    #[must_use]
    pub fn with_description(mut self, desc: impl Into<String>) -> Self {
        self.description = desc.into();
        self
    }

    /// Set the remediation suggestion.
    #[must_use]
    pub fn with_remediation(mut self, remediation: impl Into<String>) -> Self {
        self.remediation = remediation.into();
        self
    }

    /// Set the severity.
    #[must_use]
    pub fn with_severity(mut self, severity: CodeAntipatternSeverity) -> Self {
        self.severity = severity;
        self
    }

    /// Add a language this rule applies to.
    #[must_use]
    pub fn for_language(mut self, lang: Language) -> Self {
        if !self.languages.contains(&lang) {
            self.languages.push(lang);
        }
        self
    }

    /// Set multiple languages.
    #[must_use]
    pub fn for_languages(mut self, langs: Vec<Language>) -> Self {
        self.languages = langs;
        self
    }
}

/// Severity level of a code antipattern.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum CodeAntipatternSeverity {
    /// Informational - style suggestion.
    Info,
    /// Warning - should be addressed.
    Warning,
    /// Error - must be fixed before commit.
    Error,
}

impl std::fmt::Display for CodeAntipatternSeverity {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CodeAntipatternSeverity::Info => write!(f, "info"),
            CodeAntipatternSeverity::Warning => write!(f, "warning"),
            CodeAntipatternSeverity::Error => write!(f, "error"),
        }
    }
}

/// A detected code antipattern instance.
///
/// Represents a specific occurrence of an antipattern in source code.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CodeAntipatternFinding {
    /// The rule that was violated.
    pub rule_id: String,
    /// Name of the antipattern.
    pub name: String,
    /// File where the antipattern was found.
    pub file: Option<String>,
    /// Line number (1-indexed).
    pub line: u32,
    /// The matched text.
    pub matched_text: String,
    /// Description of the issue.
    pub description: String,
    /// Suggested remediation.
    pub remediation: String,
    /// Severity level.
    pub severity: CodeAntipatternSeverity,
}

impl CodeAntipatternFinding {
    /// Create a new finding from a rule match.
    #[must_use]
    pub fn from_rule(
        rule: &CodeAntipatternRule,
        line: u32,
        matched_text: impl Into<String>,
    ) -> Self {
        Self {
            rule_id: rule.id.clone(),
            name: rule.name.clone(),
            file: None,
            line,
            matched_text: matched_text.into(),
            description: rule.description.clone(),
            remediation: rule.remediation.clone(),
            severity: rule.severity,
        }
    }

    /// Set the file path.
    #[must_use]
    pub fn with_file(mut self, file: impl Into<String>) -> Self {
        self.file = Some(file.into());
        self
    }
}

/// Detector for code antipatterns.
///
/// Scans source code for language-specific antipatterns using regex matching.
#[derive(Debug, Default)]
pub struct CodeAntipatternDetector {
    /// Custom rules to use (in addition to built-in rules).
    custom_rules: Vec<CodeAntipatternRule>,
}

impl CodeAntipatternDetector {
    /// Create a new detector with default rules.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Add a custom rule.
    pub fn add_rule(&mut self, rule: CodeAntipatternRule) {
        self.custom_rules.push(rule);
    }

    /// Scan source code for antipatterns.
    ///
    /// Returns a list of findings for patterns that match in the code.
    ///
    /// # Example
    ///
    /// ```
    /// use ralph::prompt::code_antipatterns::CodeAntipatternDetector;
    /// use ralph::Language;
    ///
    /// let detector = CodeAntipatternDetector::new();
    /// let code = "except:\n    pass";
    /// let findings = detector.scan_code(code, Language::Python);
    /// // Would find "bare-except" antipattern
    /// ```
    #[must_use]
    pub fn scan_code(&self, code: &str, language: Language) -> Vec<CodeAntipatternFinding> {
        let rules = self.rules_for_language(language);
        let mut findings = Vec::new();

        for rule in rules {
            if let Ok(regex) = regex::Regex::new(&rule.pattern) {
                for (line_num, line) in code.lines().enumerate() {
                    for mat in regex.find_iter(line) {
                        findings.push(CodeAntipatternFinding::from_rule(
                            &rule,
                            (line_num + 1) as u32,
                            mat.as_str(),
                        ));
                    }
                }
            }
        }

        findings
    }

    /// Scan a file for antipatterns.
    ///
    /// Automatically detects language from file extension.
    ///
    /// # Errors
    ///
    /// Returns an error if the file cannot be read.
    pub fn scan_file(&self, path: &Path) -> std::io::Result<Vec<CodeAntipatternFinding>> {
        let Some(language) = language_from_extension(path) else {
            return Ok(Vec::new());
        };

        let code = std::fs::read_to_string(path)?;
        let mut findings = self.scan_code(&code, language);

        // Add file path to all findings
        let path_str = path.to_string_lossy().to_string();
        for finding in &mut findings {
            finding.file = Some(path_str.clone());
        }

        Ok(findings)
    }

    /// Scan multiple files, filtering by language.
    ///
    /// Only scans files that match the given languages.
    pub fn scan_files_for_languages(
        &self,
        files: &[&Path],
        languages: &[Language],
    ) -> Vec<CodeAntipatternFinding> {
        let mut findings = Vec::new();

        for path in files {
            if let Some(lang) = language_from_extension(path) {
                if languages.contains(&lang) {
                    if let Ok(file_findings) = self.scan_file(path) {
                        findings.extend(file_findings);
                    }
                }
            }
        }

        findings
    }

    /// Get all rules applicable to a language.
    fn rules_for_language(&self, language: Language) -> Vec<CodeAntipatternRule> {
        let builtin = antipatterns_for_language(language);
        let mut rules = builtin;

        // Add custom rules that apply to this language
        for rule in &self.custom_rules {
            if rule.languages.is_empty() || rule.languages.contains(&language) {
                rules.push(rule.clone());
            }
        }

        rules
    }
}

/// Get the language from a file extension.
///
/// Returns `None` if the extension is not recognized.
#[must_use]
pub fn language_from_extension(path: &Path) -> Option<Language> {
    let ext = path.extension()?.to_str()?;
    match ext {
        "py" | "pyi" | "pyw" => Some(Language::Python),
        "ts" | "tsx" | "mts" | "cts" => Some(Language::TypeScript),
        "js" | "jsx" | "mjs" | "cjs" => Some(Language::JavaScript),
        "go" => Some(Language::Go),
        "rs" => Some(Language::Rust),
        "java" => Some(Language::Java),
        "kt" | "kts" => Some(Language::Kotlin),
        "cs" => Some(Language::CSharp),
        "rb" | "rake" => Some(Language::Ruby),
        "php" => Some(Language::Php),
        _ => None,
    }
}

/// Get antipattern rules for a specific language.
///
/// Returns the built-in antipattern detection rules for the given language.
///
/// # Example
///
/// ```
/// use ralph::prompt::code_antipatterns::antipatterns_for_language;
/// use ralph::Language;
///
/// let python_rules = antipatterns_for_language(Language::Python);
/// assert!(python_rules.iter().any(|r| r.id == "bare-except"));
///
/// let ts_rules = antipatterns_for_language(Language::TypeScript);
/// assert!(ts_rules.iter().any(|r| r.id == "any-type"));
///
/// let go_rules = antipatterns_for_language(Language::Go);
/// assert!(go_rules.iter().any(|r| r.id == "ignored-error"));
/// ```
#[must_use]
pub fn antipatterns_for_language(language: Language) -> Vec<CodeAntipatternRule> {
    match language {
        Language::Python => python_antipatterns(),
        Language::TypeScript | Language::JavaScript => typescript_antipatterns(),
        Language::Go => go_antipatterns(),
        _ => Vec::new(), // Other languages rely on their linters
    }
}

/// Python-specific antipattern rules.
fn python_antipatterns() -> Vec<CodeAntipatternRule> {
    vec![
        CodeAntipatternRule::new("bare-except", "Bare except clause", r"except\s*:")
            .with_description(
                "Catches all exceptions including KeyboardInterrupt and SystemExit, \
             which can make programs hard to stop and hide bugs.",
            )
            .with_remediation(
                "Use specific exception types: except ValueError: or except Exception:",
            )
            .with_severity(CodeAntipatternSeverity::Error)
            .for_language(Language::Python),
        CodeAntipatternRule::new(
            "mutable-default-arg",
            "Mutable default argument",
            r"def\s+\w+\s*\([^)]*(?:\[\s*\]|\{\s*\})\s*(?:=[^,\)]+)?\s*[,\)]",
        )
        .with_description(
            "Default mutable arguments like [] or {} are shared across calls, \
             causing unexpected behavior when modified.",
        )
        .with_remediation("Use None as default and create the mutable object inside the function")
        .with_severity(CodeAntipatternSeverity::Error)
        .for_language(Language::Python),
        CodeAntipatternRule::new("global-statement", "Global statement", r"\bglobal\s+\w+")
            .with_description(
                "Global variables make code harder to test and reason about. \
             They can cause unexpected side effects.",
            )
            .with_remediation(
                "Pass values as function arguments or use a class to encapsulate state",
            )
            .with_severity(CodeAntipatternSeverity::Warning)
            .for_language(Language::Python),
    ]
}

/// TypeScript/JavaScript-specific antipattern rules.
fn typescript_antipatterns() -> Vec<CodeAntipatternRule> {
    vec![
        CodeAntipatternRule::new("any-type", "Use of any type", r":\s*any\b")
            .with_description(
                "Using 'any' bypasses TypeScript's type checking, \
             defeating the purpose of using TypeScript.",
            )
            .with_remediation(
                "Use specific types, generics, or 'unknown' if the type is truly unknown",
            )
            .with_severity(CodeAntipatternSeverity::Warning)
            .for_language(Language::TypeScript),
        CodeAntipatternRule::new(
            "non-null-assertion",
            "Non-null assertion operator",
            r"\w+\s*!(?:\.|(?:\[))",
        )
        .with_description(
            "The non-null assertion (!) tells TypeScript to ignore potential null/undefined, \
             which can cause runtime errors.",
        )
        .with_remediation("Use optional chaining (?.) or add proper null checks")
        .with_severity(CodeAntipatternSeverity::Warning)
        .for_language(Language::TypeScript),
        CodeAntipatternRule::new(
            "console-log",
            "Console.log statement",
            r"\bconsole\.log\s*\(",
        )
        .with_description(
            "Console.log statements should be removed before production. \
             Use a proper logging library instead.",
        )
        .with_remediation("Remove console.log or use a configurable logging library")
        .with_severity(CodeAntipatternSeverity::Info)
        .for_language(Language::TypeScript)
        .for_language(Language::JavaScript),
    ]
}

/// Go-specific antipattern rules.
fn go_antipatterns() -> Vec<CodeAntipatternRule> {
    vec![
        CodeAntipatternRule::new(
            "ignored-error",
            "Ignored error return",
            r"[^,]\s*,\s*_\s*(?::)?=\s*\w+\s*\(",
        )
        .with_description(
            "Ignoring error returns can hide bugs and make debugging difficult. \
             Go's error handling is explicit for a reason.",
        )
        .with_remediation("Handle the error: check it, wrap it, or return it")
        .with_severity(CodeAntipatternSeverity::Error)
        .for_language(Language::Go),
        CodeAntipatternRule::new(
            "empty-interface",
            "Empty interface parameter",
            r"func\s+\w+\s*\([^)]*\binterface\s*\{\s*\}",
        )
        .with_description(
            "Using interface{} (or any) loses type safety. \
             Consider using generics or a more specific interface.",
        )
        .with_remediation("Use generics with type constraints or define a specific interface")
        .with_severity(CodeAntipatternSeverity::Warning)
        .for_language(Language::Go),
        CodeAntipatternRule::new("panic-in-library", "Panic in library code", r"\bpanic\s*\(")
            .with_description(
                "Libraries should return errors, not panic. \
             Panics make the calling code's error handling impossible.",
            )
            .with_remediation("Return an error instead of panicking")
            .with_severity(CodeAntipatternSeverity::Warning)
            .for_language(Language::Go),
    ]
}

/// Format findings for inclusion in a remediation prompt.
///
/// Returns a markdown-formatted string summarizing the code antipatterns found.
#[must_use]
pub fn format_findings_for_prompt(findings: &[CodeAntipatternFinding]) -> String {
    if findings.is_empty() {
        return String::new();
    }

    let mut output = String::from("## Code Antipatterns Detected\n\n");
    output.push_str("The following antipatterns were found in the changed files:\n\n");

    // Group by severity
    let errors: Vec<_> = findings
        .iter()
        .filter(|f| f.severity == CodeAntipatternSeverity::Error)
        .collect();
    let warnings: Vec<_> = findings
        .iter()
        .filter(|f| f.severity == CodeAntipatternSeverity::Warning)
        .collect();
    let infos: Vec<_> = findings
        .iter()
        .filter(|f| f.severity == CodeAntipatternSeverity::Info)
        .collect();

    if !errors.is_empty() {
        output.push_str("### ❌ Errors (Must Fix)\n\n");
        for finding in errors {
            output.push_str(&format_finding(finding));
        }
        output.push('\n');
    }

    if !warnings.is_empty() {
        output.push_str("### ⚠️ Warnings (Should Fix)\n\n");
        for finding in warnings {
            output.push_str(&format_finding(finding));
        }
        output.push('\n');
    }

    if !infos.is_empty() {
        output.push_str("### ℹ️ Info (Consider)\n\n");
        for finding in infos {
            output.push_str(&format_finding(finding));
        }
    }

    output
}

fn format_finding(finding: &CodeAntipatternFinding) -> String {
    let location = match &finding.file {
        Some(file) => format!("{}:{}", file, finding.line),
        None => format!("line {}", finding.line),
    };

    format!(
        "- **{}** at `{}`\n  - Matched: `{}`\n  - {}\n  - Fix: {}\n\n",
        finding.name, location, finding.matched_text, finding.description, finding.remediation
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    // ============================================================
    // CodeAntipatternRule tests
    // ============================================================

    #[test]
    fn test_rule_creation() {
        let rule = CodeAntipatternRule::new("test-id", "Test Rule", r"\btest\b");

        assert_eq!(rule.id, "test-id");
        assert_eq!(rule.name, "Test Rule");
        assert_eq!(rule.pattern, r"\btest\b");
        assert!(rule.description.is_empty());
        assert!(rule.remediation.is_empty());
        assert_eq!(rule.severity, CodeAntipatternSeverity::Warning);
    }

    #[test]
    fn test_rule_with_builders() {
        let rule = CodeAntipatternRule::new("test", "Test", "pattern")
            .with_description("A test description")
            .with_remediation("Fix it like this")
            .with_severity(CodeAntipatternSeverity::Error)
            .for_language(Language::Python);

        assert_eq!(rule.description, "A test description");
        assert_eq!(rule.remediation, "Fix it like this");
        assert_eq!(rule.severity, CodeAntipatternSeverity::Error);
        assert!(rule.languages.contains(&Language::Python));
    }

    #[test]
    fn test_rule_for_multiple_languages() {
        let rule = CodeAntipatternRule::new("test", "Test", "pattern")
            .for_language(Language::TypeScript)
            .for_language(Language::JavaScript);

        assert_eq!(rule.languages.len(), 2);
        assert!(rule.languages.contains(&Language::TypeScript));
        assert!(rule.languages.contains(&Language::JavaScript));
    }

    #[test]
    fn test_rule_for_languages_vec() {
        let rule = CodeAntipatternRule::new("test", "Test", "pattern")
            .for_languages(vec![Language::Python, Language::Ruby]);

        assert_eq!(rule.languages.len(), 2);
    }

    // ============================================================
    // Python antipattern tests
    // ============================================================

    #[test]
    fn test_python_antipatterns_exist() {
        let rules = antipatterns_for_language(Language::Python);
        assert!(!rules.is_empty(), "Python should have antipattern rules");
    }

    #[test]
    fn test_python_bare_except_rule() {
        let rules = antipatterns_for_language(Language::Python);
        let rule = rules.iter().find(|r| r.id == "bare-except");
        assert!(rule.is_some(), "Should have bare-except rule");

        let rule = rule.unwrap();
        assert_eq!(rule.severity, CodeAntipatternSeverity::Error);
    }

    #[test]
    fn test_python_bare_except_detection() {
        let detector = CodeAntipatternDetector::new();
        let code = "try:\n    foo()\nexcept:\n    pass";
        let findings = detector.scan_code(code, Language::Python);

        assert!(!findings.is_empty(), "Should detect bare except");
        assert!(
            findings.iter().any(|f| f.rule_id == "bare-except"),
            "Should find bare-except rule violation"
        );
    }

    #[test]
    fn test_python_specific_except_not_flagged() {
        let detector = CodeAntipatternDetector::new();
        let code = "try:\n    foo()\nexcept ValueError:\n    pass";
        let findings = detector.scan_code(code, Language::Python);

        assert!(
            !findings.iter().any(|f| f.rule_id == "bare-except"),
            "Specific except should not be flagged"
        );
    }

    #[test]
    fn test_python_mutable_default_arg_rule() {
        let rules = antipatterns_for_language(Language::Python);
        let rule = rules.iter().find(|r| r.id == "mutable-default-arg");
        assert!(rule.is_some(), "Should have mutable-default-arg rule");
    }

    #[test]
    fn test_python_global_statement_detection() {
        let detector = CodeAntipatternDetector::new();
        let code = "def foo():\n    global counter\n    counter += 1";
        let findings = detector.scan_code(code, Language::Python);

        assert!(
            findings.iter().any(|f| f.rule_id == "global-statement"),
            "Should detect global statement"
        );
    }

    // ============================================================
    // TypeScript antipattern tests
    // ============================================================

    #[test]
    fn test_typescript_antipatterns_exist() {
        let rules = antipatterns_for_language(Language::TypeScript);
        assert!(
            !rules.is_empty(),
            "TypeScript should have antipattern rules"
        );
    }

    #[test]
    fn test_typescript_any_type_rule() {
        let rules = antipatterns_for_language(Language::TypeScript);
        let rule = rules.iter().find(|r| r.id == "any-type");
        assert!(rule.is_some(), "Should have any-type rule");
    }

    #[test]
    fn test_typescript_any_type_detection() {
        let detector = CodeAntipatternDetector::new();
        let code = "function foo(x: any): void {}";
        let findings = detector.scan_code(code, Language::TypeScript);

        assert!(
            findings.iter().any(|f| f.rule_id == "any-type"),
            "Should detect any type usage"
        );
    }

    #[test]
    fn test_typescript_specific_type_not_flagged() {
        let detector = CodeAntipatternDetector::new();
        let code = "function foo(x: string): void {}";
        let findings = detector.scan_code(code, Language::TypeScript);

        assert!(
            !findings.iter().any(|f| f.rule_id == "any-type"),
            "Specific types should not be flagged"
        );
    }

    #[test]
    fn test_typescript_non_null_assertion_rule() {
        let rules = antipatterns_for_language(Language::TypeScript);
        let rule = rules.iter().find(|r| r.id == "non-null-assertion");
        assert!(rule.is_some(), "Should have non-null-assertion rule");
    }

    #[test]
    fn test_typescript_non_null_assertion_detection() {
        let detector = CodeAntipatternDetector::new();
        let code = "const len = value!.length";
        let findings = detector.scan_code(code, Language::TypeScript);

        assert!(
            findings.iter().any(|f| f.rule_id == "non-null-assertion"),
            "Should detect non-null assertion"
        );
    }

    #[test]
    fn test_typescript_console_log_detection() {
        let detector = CodeAntipatternDetector::new();
        let code = "console.log('debug info')";
        let findings = detector.scan_code(code, Language::TypeScript);

        assert!(
            findings.iter().any(|f| f.rule_id == "console-log"),
            "Should detect console.log"
        );
    }

    // ============================================================
    // Go antipattern tests
    // ============================================================

    #[test]
    fn test_go_antipatterns_exist() {
        let rules = antipatterns_for_language(Language::Go);
        assert!(!rules.is_empty(), "Go should have antipattern rules");
    }

    #[test]
    fn test_go_ignored_error_rule() {
        let rules = antipatterns_for_language(Language::Go);
        let rule = rules.iter().find(|r| r.id == "ignored-error");
        assert!(rule.is_some(), "Should have ignored-error rule");

        let rule = rule.unwrap();
        assert_eq!(rule.severity, CodeAntipatternSeverity::Error);
    }

    #[test]
    fn test_go_ignored_error_detection() {
        let detector = CodeAntipatternDetector::new();
        let code = "result, _ := doSomething()";
        let findings = detector.scan_code(code, Language::Go);

        assert!(
            findings.iter().any(|f| f.rule_id == "ignored-error"),
            "Should detect ignored error"
        );
    }

    #[test]
    fn test_go_empty_interface_rule() {
        let rules = antipatterns_for_language(Language::Go);
        let rule = rules.iter().find(|r| r.id == "empty-interface");
        assert!(rule.is_some(), "Should have empty-interface rule");
    }

    #[test]
    fn test_go_empty_interface_detection() {
        let detector = CodeAntipatternDetector::new();
        let code = "func process(data interface{}) error {";
        let findings = detector.scan_code(code, Language::Go);

        assert!(
            findings.iter().any(|f| f.rule_id == "empty-interface"),
            "Should detect empty interface usage"
        );
    }

    #[test]
    fn test_go_panic_detection() {
        let detector = CodeAntipatternDetector::new();
        let code = "panic(\"something went wrong\")";
        let findings = detector.scan_code(code, Language::Go);

        assert!(
            findings.iter().any(|f| f.rule_id == "panic-in-library"),
            "Should detect panic usage"
        );
    }

    // ============================================================
    // Detector with language parameter tests
    // ============================================================

    #[test]
    fn test_detector_accepts_language_parameter() {
        let detector = CodeAntipatternDetector::new();

        // Should work with any language
        let _ = detector.scan_code("code", Language::Python);
        let _ = detector.scan_code("code", Language::TypeScript);
        let _ = detector.scan_code("code", Language::Go);
        let _ = detector.scan_code("code", Language::Rust);
    }

    #[test]
    fn test_detector_returns_empty_for_rust() {
        // Rust antipatterns are handled by clippy, not this detector
        let detector = CodeAntipatternDetector::new();
        let code = "fn main() { todo!() }";
        let findings = detector.scan_code(code, Language::Rust);

        assert!(
            findings.is_empty(),
            "Rust should have no antipattern rules in this detector"
        );
    }

    #[test]
    fn test_detector_different_rules_per_language() {
        let python_rules = antipatterns_for_language(Language::Python);
        let ts_rules = antipatterns_for_language(Language::TypeScript);
        let go_rules = antipatterns_for_language(Language::Go);

        // Ensure rules are different per language
        assert!(python_rules.iter().any(|r| r.id == "bare-except"));
        assert!(!ts_rules.iter().any(|r| r.id == "bare-except"));
        assert!(!go_rules.iter().any(|r| r.id == "bare-except"));

        assert!(ts_rules.iter().any(|r| r.id == "any-type"));
        assert!(!python_rules.iter().any(|r| r.id == "any-type"));

        assert!(go_rules.iter().any(|r| r.id == "ignored-error"));
        assert!(!python_rules.iter().any(|r| r.id == "ignored-error"));
    }

    // ============================================================
    // Polyglot file filtering tests
    // ============================================================

    #[test]
    fn test_language_from_extension() {
        assert_eq!(
            language_from_extension(Path::new("file.py")),
            Some(Language::Python)
        );
        assert_eq!(
            language_from_extension(Path::new("file.ts")),
            Some(Language::TypeScript)
        );
        assert_eq!(
            language_from_extension(Path::new("file.tsx")),
            Some(Language::TypeScript)
        );
        assert_eq!(
            language_from_extension(Path::new("file.js")),
            Some(Language::JavaScript)
        );
        assert_eq!(
            language_from_extension(Path::new("file.go")),
            Some(Language::Go)
        );
        assert_eq!(
            language_from_extension(Path::new("file.rs")),
            Some(Language::Rust)
        );
    }

    #[test]
    fn test_language_from_extension_unknown() {
        assert_eq!(language_from_extension(Path::new("file.unknown")), None);
        assert_eq!(language_from_extension(Path::new("file")), None);
    }

    #[test]
    fn test_scan_files_filters_by_language() {
        // This tests that only files matching the specified languages are scanned
        let detector = CodeAntipatternDetector::new();

        // Create temp files would be needed for a full test
        // For now, test the logic with empty input
        let files: Vec<&Path> = vec![];
        let findings = detector.scan_files_for_languages(&files, &[Language::Python]);
        assert!(findings.is_empty());
    }

    // ============================================================
    // Finding formatting tests
    // ============================================================

    #[test]
    fn test_finding_from_rule() {
        let rule = CodeAntipatternRule::new("test", "Test Rule", "pattern")
            .with_description("Test description")
            .with_remediation("Fix it")
            .with_severity(CodeAntipatternSeverity::Error);

        let finding = CodeAntipatternFinding::from_rule(&rule, 42, "matched text");

        assert_eq!(finding.rule_id, "test");
        assert_eq!(finding.name, "Test Rule");
        assert_eq!(finding.line, 42);
        assert_eq!(finding.matched_text, "matched text");
        assert_eq!(finding.severity, CodeAntipatternSeverity::Error);
    }

    #[test]
    fn test_finding_with_file() {
        let rule = CodeAntipatternRule::new("test", "Test", "pattern");
        let finding = CodeAntipatternFinding::from_rule(&rule, 1, "text").with_file("src/main.py");

        assert_eq!(finding.file, Some("src/main.py".to_string()));
    }

    #[test]
    fn test_format_findings_empty() {
        let findings: Vec<CodeAntipatternFinding> = vec![];
        let output = format_findings_for_prompt(&findings);
        assert!(output.is_empty());
    }

    #[test]
    fn test_format_findings_with_content() {
        let rule = CodeAntipatternRule::new("test", "Test Rule", "pattern")
            .with_description("Description")
            .with_remediation("Fix it")
            .with_severity(CodeAntipatternSeverity::Error);

        let finding = CodeAntipatternFinding::from_rule(&rule, 10, "bad code").with_file("test.py");

        let output = format_findings_for_prompt(&[finding]);

        assert!(output.contains("Code Antipatterns Detected"));
        assert!(output.contains("Test Rule"));
        assert!(output.contains("test.py:10"));
        assert!(output.contains("bad code"));
    }

    #[test]
    fn test_format_findings_groups_by_severity() {
        let error_rule = CodeAntipatternRule::new("error", "Error Rule", "pattern")
            .with_severity(CodeAntipatternSeverity::Error);
        let warning_rule = CodeAntipatternRule::new("warning", "Warning Rule", "pattern")
            .with_severity(CodeAntipatternSeverity::Warning);

        let error_finding = CodeAntipatternFinding::from_rule(&error_rule, 1, "e");
        let warning_finding = CodeAntipatternFinding::from_rule(&warning_rule, 2, "w");

        let output = format_findings_for_prompt(&[error_finding, warning_finding]);

        assert!(output.contains("Errors (Must Fix)"));
        assert!(output.contains("Warnings (Should Fix)"));
    }

    // ============================================================
    // Custom rules tests
    // ============================================================

    #[test]
    fn test_detector_with_custom_rule() {
        let mut detector = CodeAntipatternDetector::new();

        let custom_rule = CodeAntipatternRule::new("custom", "Custom Rule", r"FIXME")
            .for_language(Language::Python);

        detector.add_rule(custom_rule);

        let code = "# FIXME: this needs work";
        let findings = detector.scan_code(code, Language::Python);

        assert!(
            findings.iter().any(|f| f.rule_id == "custom"),
            "Should detect custom rule"
        );
    }

    // ============================================================
    // Severity display tests
    // ============================================================

    #[test]
    fn test_severity_display() {
        assert_eq!(CodeAntipatternSeverity::Info.to_string(), "info");
        assert_eq!(CodeAntipatternSeverity::Warning.to_string(), "warning");
        assert_eq!(CodeAntipatternSeverity::Error.to_string(), "error");
    }

    #[test]
    fn test_severity_ordering() {
        assert!(CodeAntipatternSeverity::Info < CodeAntipatternSeverity::Warning);
        assert!(CodeAntipatternSeverity::Warning < CodeAntipatternSeverity::Error);
    }
}
