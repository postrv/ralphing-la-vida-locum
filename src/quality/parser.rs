//! Output parsing utilities for quality gates.
//!
//! This module provides the `OutputParser` trait and common parsing utilities
//! to consolidate repeated parsing patterns across different quality gates.
//!
//! # Common Patterns
//!
//! Many linting tools output issues in standard formats:
//!
//! - **Line format**: `file:line:col: message` or `file:line: message`
//! - **JSON format**: Tool-specific JSON structures
//! - **Multi-line format**: Context spanning multiple lines (e.g., Clippy)
//!
//! # Example
//!
//! ```rust
//! use ralph::quality::parser::{OutputParser, LineFormat, parse_colon_separated_line};
//! use ralph::quality::gates::{GateIssue, IssueSeverity};
//!
//! // Parse a standard line format
//! let line = "src/main.rs:10:5: warning: unused variable";
//! let format = LineFormat::new(":").with_severity_in_message();
//! if let Some(issue) = parse_colon_separated_line(line, &format) {
//!     assert_eq!(issue.line, Some(10));
//!     assert_eq!(issue.column, Some(5));
//! }
//! ```

use super::gates::{GateIssue, IssueSeverity};

// ============================================================================
// Line Format Configuration
// ============================================================================

/// Configuration for parsing line-based output.
///
/// Many tools output issues in formats like:
/// - `file:line:col: message`
/// - `file:line: CODE message`
/// - `file(line,col): message`
///
/// This struct configures how to parse such lines.
#[derive(Debug, Clone)]
pub struct LineFormat {
    /// Separator between file, line, column (usually ":")
    pub separator: String,
    /// Default severity if not extracted from message
    pub default_severity: IssueSeverity,
    /// File extension filter (only parse lines with matching files)
    pub file_extension: Option<String>,
    /// Whether to extract severity from the message text
    pub severity_in_message: bool,
    /// Whether the format includes a column number
    pub has_column: bool,
    /// Whether the format includes an error code before the message
    pub has_code: bool,
}

impl Default for LineFormat {
    fn default() -> Self {
        Self {
            separator: ":".to_string(),
            default_severity: IssueSeverity::Error,
            file_extension: None,
            severity_in_message: false,
            has_column: true,
            has_code: false,
        }
    }
}

impl LineFormat {
    /// Create a new line format with the given separator.
    #[must_use]
    pub fn new(separator: impl Into<String>) -> Self {
        Self {
            separator: separator.into(),
            ..Default::default()
        }
    }

    /// Set the default severity for parsed issues.
    #[must_use]
    pub fn with_default_severity(mut self, severity: IssueSeverity) -> Self {
        self.default_severity = severity;
        self
    }

    /// Filter to only parse lines with files matching this extension.
    #[must_use]
    pub fn with_file_extension(mut self, ext: impl Into<String>) -> Self {
        self.file_extension = Some(ext.into());
        self
    }

    /// Indicate that severity should be extracted from the message text.
    #[must_use]
    pub fn with_severity_in_message(mut self) -> Self {
        self.severity_in_message = true;
        self
    }

    /// Indicate that the format does not include a column number.
    #[must_use]
    pub fn without_column(mut self) -> Self {
        self.has_column = false;
        self
    }

    /// Indicate that the format includes an error code before the message.
    #[must_use]
    pub fn with_code(mut self) -> Self {
        self.has_code = true;
        self
    }
}

// ============================================================================
// Parsing Functions
// ============================================================================

/// Parse a line in the common `file:line:col: message` format.
///
/// Returns `None` if the line doesn't match the expected format.
///
/// # Arguments
///
/// * `line` - The line to parse
/// * `format` - Configuration for parsing
///
/// # Example
///
/// ```rust
/// use ralph::quality::parser::{parse_colon_separated_line, LineFormat};
///
/// let line = "src/main.rs:10:5: unused variable";
/// let format = LineFormat::default();
/// let issue = parse_colon_separated_line(line, &format).unwrap();
/// assert_eq!(issue.line, Some(10));
/// assert_eq!(issue.column, Some(5));
/// assert_eq!(issue.message, "unused variable");
/// ```
#[must_use]
pub fn parse_colon_separated_line(line: &str, format: &LineFormat) -> Option<GateIssue> {
    let sep = &format.separator;

    // Split into parts: file, line, col (optional), message
    let parts: Vec<&str> = line.splitn(4, sep.as_str()).collect();

    if parts.len() < 3 {
        return None;
    }

    let file = parts[0].trim();

    // Check file extension filter if set
    if let Some(ref ext) = format.file_extension {
        if !file.ends_with(ext.as_str()) {
            return None;
        }
    }

    // Parse line number
    let line_num: u32 = parts[1].trim().parse().ok()?;

    // Parse column and message based on format
    let (col, message_part) = if format.has_column && parts.len() >= 4 {
        // Try to parse column
        if let Ok(col_num) = parts[2].trim().parse::<u32>() {
            (Some(col_num), parts[3].trim())
        } else {
            // Column wasn't a number, treat as part of message
            let msg = if parts.len() >= 4 {
                format!("{}{}{}", parts[2], sep, parts[3]).trim().to_string()
            } else {
                parts[2].trim().to_string()
            };
            (None, msg.leak() as &str)
        }
    } else if parts.len() >= 3 {
        // No column expected or not enough parts
        (None, parts[2].trim())
    } else {
        return None;
    };

    // Extract severity from message if configured
    let (severity, message) = if format.severity_in_message {
        extract_severity_from_message(message_part, format.default_severity)
    } else {
        (format.default_severity, message_part.to_string())
    };

    // Extract code from message if configured
    let (code, final_message) = if format.has_code {
        extract_code_from_message(&message)
    } else {
        (None, message)
    };

    let mut issue = GateIssue::new(severity, final_message).with_location(file, line_num);

    if let Some(c) = col {
        issue = issue.with_column(c);
    }

    if let Some(c) = code {
        issue = issue.with_code(c);
    }

    Some(issue)
}

/// Extract severity from message text.
///
/// Looks for prefixes like "error:", "warning:", "note:" in the message.
fn extract_severity_from_message(message: &str, default: IssueSeverity) -> (IssueSeverity, String) {
    let lower = message.to_lowercase();

    if lower.starts_with("error:") || lower.starts_with("error ") {
        (
            IssueSeverity::Error,
            message[6..].trim_start_matches(':').trim().to_string(),
        )
    } else if lower.starts_with("warning:") || lower.starts_with("warning ") {
        (
            IssueSeverity::Warning,
            message[8..].trim_start_matches(':').trim().to_string(),
        )
    } else if lower.starts_with("note:") || lower.starts_with("info:") {
        (
            IssueSeverity::Info,
            message[5..].trim_start_matches(':').trim().to_string(),
        )
    } else if lower.starts_with("critical:") {
        (
            IssueSeverity::Critical,
            message[9..].trim_start_matches(':').trim().to_string(),
        )
    } else {
        (default, message.to_string())
    }
}

/// Extract an error code from the start of a message.
///
/// Looks for patterns like "E501", "W0612", "clippy::unwrap_used" at the start.
fn extract_code_from_message(message: &str) -> (Option<String>, String) {
    let trimmed = message.trim();

    // Try to find a code at the start (letters + numbers or code::name pattern)
    if let Some(space_pos) = trimmed.find(' ') {
        let potential_code = &trimmed[..space_pos];

        // Check if it looks like a code (alphanumeric with possible :: or -)
        if looks_like_error_code(potential_code) {
            return (
                Some(potential_code.to_string()),
                trimmed[space_pos + 1..].to_string(),
            );
        }
    }

    (None, message.to_string())
}

/// Check if a string looks like an error code.
fn looks_like_error_code(s: &str) -> bool {
    if s.is_empty() || s.len() > 50 {
        return false;
    }

    // Common patterns:
    // - E501, W0612 (letter + digits)
    // - clippy::unwrap_used (namespace::name)
    // - go-vet (word-word)

    let has_letter = s.chars().any(|c| c.is_alphabetic());
    let has_digit_or_sep = s.chars().any(|c| c.is_ascii_digit() || c == ':' || c == '-');

    has_letter && has_digit_or_sep
}

/// Parse multiple lines, filtering to only those matching the format.
///
/// # Example
///
/// ```rust
/// use ralph::quality::parser::{parse_lines, LineFormat};
///
/// let output = "src/main.rs:10:5: unused variable\nsome other line\nsrc/lib.rs:20:1: missing docs";
/// let format = LineFormat::default();
/// let issues = parse_lines(output, &format);
/// assert_eq!(issues.len(), 2);
/// ```
#[must_use]
pub fn parse_lines(output: &str, format: &LineFormat) -> Vec<GateIssue> {
    output
        .lines()
        .filter_map(|line| parse_colon_separated_line(line, format))
        .collect()
}

// ============================================================================
// OutputParser Trait
// ============================================================================

/// Trait for parsing tool output into gate issues.
///
/// Quality gates can implement this trait to provide a standardized
/// parsing interface. The trait provides default implementations that
/// can be overridden for tool-specific behavior.
///
/// # Example
///
/// ```rust,ignore
/// use ralph::quality::parser::OutputParser;
/// use ralph::quality::gates::GateIssue;
///
/// struct MyLinter;
///
/// impl OutputParser for MyLinter {
///     fn parse(&self, stdout: &str, stderr: &str) -> Vec<GateIssue> {
///         // Custom parsing logic
///         Vec::new()
///     }
/// }
/// ```
pub trait OutputParser {
    /// Parse command output into issues.
    ///
    /// The default implementation tries `parse_json` first, then falls back
    /// to `parse_text` if JSON parsing returns no results.
    fn parse(&self, stdout: &str, stderr: &str) -> Vec<GateIssue> {
        // Try JSON first
        let json_issues = self.parse_json(stdout);
        if !json_issues.is_empty() {
            return json_issues;
        }

        // Fall back to text parsing
        self.parse_text(stdout, stderr)
    }

    /// Parse JSON-formatted output.
    ///
    /// Override this for tools that output JSON (e.g., ESLint with --format=json).
    /// The default implementation returns an empty vector.
    fn parse_json(&self, _stdout: &str) -> Vec<GateIssue> {
        Vec::new()
    }

    /// Parse text-formatted output.
    ///
    /// Override this for tools that output plain text.
    /// The default implementation parses lines from both stdout and stderr.
    fn parse_text(&self, stdout: &str, stderr: &str) -> Vec<GateIssue> {
        let mut issues = self.parse_text_lines(stdout);
        issues.extend(self.parse_text_lines(stderr));
        issues
    }

    /// Parse individual lines from text output.
    ///
    /// Override this or implement `line_format` for line-based parsing.
    fn parse_text_lines(&self, output: &str) -> Vec<GateIssue> {
        if let Some(format) = self.line_format() {
            parse_lines(output, &format)
        } else {
            Vec::new()
        }
    }

    /// Return the line format configuration for this parser.
    ///
    /// Override to enable automatic line-based parsing.
    fn line_format(&self) -> Option<LineFormat> {
        None
    }
}

// ============================================================================
// Pre-configured Formats
// ============================================================================

/// Standard Go output format: `file.go:line:col: message`
#[must_use]
pub fn go_line_format() -> LineFormat {
    LineFormat::new(":")
        .with_file_extension(".go")
        .with_default_severity(IssueSeverity::Error)
}

/// Standard Python output format: `file.py:line:col: CODE message`
#[must_use]
pub fn python_line_format() -> LineFormat {
    LineFormat::new(":")
        .with_file_extension(".py")
        .with_code()
        .with_default_severity(IssueSeverity::Warning)
}

/// Standard TypeScript output format: `file.ts(line,col): message`
#[must_use]
pub fn typescript_line_format() -> LineFormat {
    // Note: TSC uses different format, this is a simplified version
    LineFormat::new(":")
        .with_default_severity(IssueSeverity::Error)
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    // ------------------------------------------------------------------------
    // LineFormat Tests
    // ------------------------------------------------------------------------

    #[test]
    fn test_line_format_default() {
        let format = LineFormat::default();
        assert_eq!(format.separator, ":");
        assert_eq!(format.default_severity, IssueSeverity::Error);
        assert!(format.has_column);
        assert!(!format.has_code);
        assert!(!format.severity_in_message);
        assert!(format.file_extension.is_none());
    }

    #[test]
    fn test_line_format_builder() {
        let format = LineFormat::new("|")
            .with_default_severity(IssueSeverity::Warning)
            .with_file_extension(".rs")
            .with_severity_in_message()
            .with_code()
            .without_column();

        assert_eq!(format.separator, "|");
        assert_eq!(format.default_severity, IssueSeverity::Warning);
        assert_eq!(format.file_extension, Some(".rs".to_string()));
        assert!(format.severity_in_message);
        assert!(format.has_code);
        assert!(!format.has_column);
    }

    // ------------------------------------------------------------------------
    // parse_colon_separated_line Tests
    // ------------------------------------------------------------------------

    #[test]
    fn test_parse_standard_line_with_col() {
        let line = "src/main.rs:10:5: unused variable `x`";
        let format = LineFormat::default();
        let issue = parse_colon_separated_line(line, &format).unwrap();

        assert_eq!(issue.file, Some(PathBuf::from("src/main.rs")));
        assert_eq!(issue.line, Some(10));
        assert_eq!(issue.column, Some(5));
        assert_eq!(issue.message, "unused variable `x`");
        assert_eq!(issue.severity, IssueSeverity::Error);
    }

    #[test]
    fn test_parse_line_without_col() {
        let line = "src/lib.rs:20: missing documentation";
        let format = LineFormat::default().without_column();
        let issue = parse_colon_separated_line(line, &format).unwrap();

        assert_eq!(issue.file, Some(PathBuf::from("src/lib.rs")));
        assert_eq!(issue.line, Some(20));
        assert!(issue.column.is_none());
        assert_eq!(issue.message, "missing documentation");
    }

    #[test]
    fn test_parse_line_with_severity_extraction() {
        let line = "src/main.rs:10:5: warning: unused variable";
        let format = LineFormat::default().with_severity_in_message();
        let issue = parse_colon_separated_line(line, &format).unwrap();

        assert_eq!(issue.severity, IssueSeverity::Warning);
        assert_eq!(issue.message, "unused variable");
    }

    #[test]
    fn test_parse_line_with_code_extraction() {
        let line = "src/main.py:10:5: E501 line too long";
        let format = LineFormat::default().with_code();
        let issue = parse_colon_separated_line(line, &format).unwrap();

        assert_eq!(issue.code, Some("E501".to_string()));
        assert_eq!(issue.message, "line too long");
    }

    #[test]
    fn test_parse_line_with_file_filter() {
        let go_line = "main.go:10:5: error here";
        let py_line = "main.py:10:5: error here";

        let format = LineFormat::default().with_file_extension(".go");

        assert!(parse_colon_separated_line(go_line, &format).is_some());
        assert!(parse_colon_separated_line(py_line, &format).is_none());
    }

    #[test]
    fn test_parse_line_returns_none_for_invalid() {
        let format = LineFormat::default();

        assert!(parse_colon_separated_line("not a valid line", &format).is_none());
        assert!(parse_colon_separated_line("only:one:part", &format).is_none());
        assert!(parse_colon_separated_line("file:notanumber:5: msg", &format).is_none());
    }

    // ------------------------------------------------------------------------
    // extract_severity_from_message Tests
    // ------------------------------------------------------------------------

    #[test]
    fn test_extract_severity_error() {
        let (sev, msg) = extract_severity_from_message("error: something bad", IssueSeverity::Info);
        assert_eq!(sev, IssueSeverity::Error);
        assert_eq!(msg, "something bad");
    }

    #[test]
    fn test_extract_severity_warning() {
        let (sev, msg) =
            extract_severity_from_message("warning: be careful", IssueSeverity::Error);
        assert_eq!(sev, IssueSeverity::Warning);
        assert_eq!(msg, "be careful");
    }

    #[test]
    fn test_extract_severity_info() {
        let (sev, msg) = extract_severity_from_message("note: just fyi", IssueSeverity::Error);
        assert_eq!(sev, IssueSeverity::Info);
        assert_eq!(msg, "just fyi");
    }

    #[test]
    fn test_extract_severity_none() {
        let (sev, msg) = extract_severity_from_message("just a message", IssueSeverity::Warning);
        assert_eq!(sev, IssueSeverity::Warning);
        assert_eq!(msg, "just a message");
    }

    // ------------------------------------------------------------------------
    // extract_code_from_message Tests
    // ------------------------------------------------------------------------

    #[test]
    fn test_extract_code_with_code() {
        let (code, msg) = extract_code_from_message("E501 line too long");
        assert_eq!(code, Some("E501".to_string()));
        assert_eq!(msg, "line too long");
    }

    #[test]
    fn test_extract_code_with_namespaced_code() {
        let (code, msg) = extract_code_from_message("clippy::unwrap_used consider using expect");
        assert_eq!(code, Some("clippy::unwrap_used".to_string()));
        assert_eq!(msg, "consider using expect");
    }

    #[test]
    fn test_extract_code_no_code() {
        let (code, msg) = extract_code_from_message("just a regular message");
        assert!(code.is_none());
        assert_eq!(msg, "just a regular message");
    }

    // ------------------------------------------------------------------------
    // parse_lines Tests
    // ------------------------------------------------------------------------

    #[test]
    fn test_parse_lines_multiple() {
        let output = "src/a.rs:10:5: error one\nignored line\nsrc/b.rs:20:1: error two";
        let format = LineFormat::default();
        let issues = parse_lines(output, &format);

        assert_eq!(issues.len(), 2);
        assert_eq!(issues[0].message, "error one");
        assert_eq!(issues[1].message, "error two");
    }

    #[test]
    fn test_parse_lines_empty() {
        let output = "";
        let format = LineFormat::default();
        let issues = parse_lines(output, &format);
        assert!(issues.is_empty());
    }

    #[test]
    fn test_parse_lines_no_matches() {
        let output = "this is not\na valid format\nat all";
        let format = LineFormat::default();
        let issues = parse_lines(output, &format);
        assert!(issues.is_empty());
    }

    // ------------------------------------------------------------------------
    // OutputParser Trait Tests
    // ------------------------------------------------------------------------

    struct TestParser {
        format: Option<LineFormat>,
    }

    impl OutputParser for TestParser {
        fn line_format(&self) -> Option<LineFormat> {
            self.format.clone()
        }
    }

    #[test]
    fn test_output_parser_with_line_format() {
        let parser = TestParser {
            format: Some(LineFormat::default()),
        };

        let stdout = "src/main.rs:10:5: test error";
        let issues = parser.parse(stdout, "");

        assert_eq!(issues.len(), 1);
        assert_eq!(issues[0].message, "test error");
    }

    #[test]
    fn test_output_parser_without_line_format() {
        let parser = TestParser { format: None };

        let stdout = "src/main.rs:10:5: test error";
        let issues = parser.parse(stdout, "");

        assert!(issues.is_empty());
    }

    #[test]
    fn test_output_parser_parses_stderr() {
        let parser = TestParser {
            format: Some(LineFormat::default()),
        };

        let stderr = "src/lib.rs:5:1: stderr error";
        let issues = parser.parse("", stderr);

        assert_eq!(issues.len(), 1);
        assert_eq!(issues[0].message, "stderr error");
    }

    #[test]
    fn test_output_parser_combines_stdout_stderr() {
        let parser = TestParser {
            format: Some(LineFormat::default()),
        };

        let stdout = "src/a.rs:1:1: from stdout";
        let stderr = "src/b.rs:2:2: from stderr";
        let issues = parser.parse(stdout, stderr);

        assert_eq!(issues.len(), 2);
    }

    // ------------------------------------------------------------------------
    // Pre-configured Format Tests
    // ------------------------------------------------------------------------

    #[test]
    fn test_go_line_format() {
        let format = go_line_format();
        assert_eq!(format.file_extension, Some(".go".to_string()));
        assert_eq!(format.default_severity, IssueSeverity::Error);
    }

    #[test]
    fn test_python_line_format() {
        let format = python_line_format();
        assert_eq!(format.file_extension, Some(".py".to_string()));
        assert!(format.has_code);
        assert_eq!(format.default_severity, IssueSeverity::Warning);
    }

    #[test]
    fn test_typescript_line_format() {
        let format = typescript_line_format();
        assert_eq!(format.default_severity, IssueSeverity::Error);
    }

    // ------------------------------------------------------------------------
    // looks_like_error_code Tests
    // ------------------------------------------------------------------------

    #[test]
    fn test_looks_like_error_code_valid() {
        assert!(looks_like_error_code("E501"));
        assert!(looks_like_error_code("W0612"));
        assert!(looks_like_error_code("clippy::unwrap_used"));
        assert!(looks_like_error_code("go-vet"));
    }

    #[test]
    fn test_looks_like_error_code_invalid() {
        assert!(!looks_like_error_code(""));
        assert!(!looks_like_error_code("just"));
        assert!(!looks_like_error_code("123"));
        assert!(!looks_like_error_code("a".repeat(100).as_str()));
    }
}
