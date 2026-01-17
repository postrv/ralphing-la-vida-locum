//! NarsilClient implementation for MCP tool invocation.
//!
//! This module provides the core client for communicating with narsil-mcp
//! via command-line invocation. It handles spawning the process, sending
//! requests, and parsing responses.

use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::process::Command;
use std::str::FromStr;
use thiserror::Error;

// ============================================================================
// Error Types
// ============================================================================

/// Errors that can occur during narsil-mcp operations.
#[derive(Error, Debug)]
pub enum NarsilError {
    /// narsil-mcp is not available (not installed or not in PATH).
    #[error("narsil-mcp unavailable: {0}")]
    Unavailable(String),

    /// Operation timed out.
    #[error("narsil-mcp operation timed out after {0}ms")]
    Timeout(u64),

    /// Failed to parse response.
    #[error("Failed to parse narsil-mcp response: {0}")]
    ParseError(String),

    /// Tool invocation failed.
    #[error("Tool invocation failed: {0}")]
    ToolError(String),

    /// IO error during communication.
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
}

impl NarsilError {
    /// Check if this error is recoverable (operation can be retried or skipped).
    pub fn is_recoverable(&self) -> bool {
        matches!(self, Self::Unavailable(_) | Self::Timeout(_))
    }
}

// ============================================================================
// Configuration
// ============================================================================

/// Configuration for the NarsilClient.
#[derive(Debug, Clone)]
pub struct NarsilConfig {
    /// Path to the repository to analyze.
    pub repo_path: PathBuf,
    /// Timeout for operations in milliseconds.
    pub timeout_ms: u64,
    /// Enable git integration.
    pub git_enabled: bool,
    /// Enable call graph analysis.
    pub call_graph_enabled: bool,
    /// Path to the narsil-mcp binary.
    pub binary_path: PathBuf,
}

impl Default for NarsilConfig {
    fn default() -> Self {
        Self {
            repo_path: PathBuf::from("."),
            timeout_ms: 30000,
            git_enabled: true,
            call_graph_enabled: true,
            binary_path: PathBuf::from("narsil-mcp"),
        }
    }
}

impl NarsilConfig {
    /// Create a new config with the specified repository path.
    pub fn new(repo_path: impl Into<PathBuf>) -> Self {
        Self {
            repo_path: repo_path.into(),
            ..Default::default()
        }
    }

    /// Set the operation timeout in milliseconds.
    #[must_use]
    pub fn with_timeout_ms(mut self, timeout_ms: u64) -> Self {
        self.timeout_ms = timeout_ms;
        self
    }

    /// Enable or disable git integration.
    #[must_use]
    pub fn with_git(mut self, enabled: bool) -> Self {
        self.git_enabled = enabled;
        self
    }

    /// Enable or disable call graph analysis.
    #[must_use]
    pub fn with_call_graph(mut self, enabled: bool) -> Self {
        self.call_graph_enabled = enabled;
        self
    }

    /// Set the path to the narsil-mcp binary.
    #[must_use]
    pub fn with_binary_path(mut self, path: impl Into<PathBuf>) -> Self {
        self.binary_path = path.into();
        self
    }
}

// ============================================================================
// Security Types
// ============================================================================

/// Severity level for security findings.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SecuritySeverity {
    /// Informational finding.
    Info,
    /// Low severity issue.
    Low,
    /// Medium severity issue.
    Medium,
    /// High severity issue - blocks commits.
    High,
    /// Critical security vulnerability - must fix immediately.
    Critical,
}

impl SecuritySeverity {
    /// Check if this severity level blocks commits.
    pub fn is_blocking(&self) -> bool {
        matches!(self, Self::High | Self::Critical)
    }
}

impl std::fmt::Display for SecuritySeverity {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Info => write!(f, "INFO"),
            Self::Low => write!(f, "LOW"),
            Self::Medium => write!(f, "MEDIUM"),
            Self::High => write!(f, "HIGH"),
            Self::Critical => write!(f, "CRITICAL"),
        }
    }
}

impl FromStr for SecuritySeverity {
    type Err = NarsilError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "critical" => Ok(Self::Critical),
            "high" => Ok(Self::High),
            "medium" => Ok(Self::Medium),
            "low" => Ok(Self::Low),
            "info" => Ok(Self::Info),
            _ => Err(NarsilError::ParseError(format!(
                "Unknown severity level: {}",
                s
            ))),
        }
    }
}

/// A security finding from narsil-mcp scan.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SecurityFinding {
    /// Severity of the finding.
    pub severity: SecuritySeverity,
    /// Description of the security issue.
    pub message: String,
    /// File where the issue was found.
    pub file: PathBuf,
    /// Line number (if available).
    pub line: Option<u32>,
    /// Rule ID that triggered this finding.
    pub rule_id: Option<String>,
    /// Suggested fix.
    pub suggestion: Option<String>,
}

impl SecurityFinding {
    /// Create a new security finding.
    pub fn new(severity: SecuritySeverity, message: impl Into<String>, file: impl Into<PathBuf>) -> Self {
        Self {
            severity,
            message: message.into(),
            file: file.into(),
            line: None,
            rule_id: None,
            suggestion: None,
        }
    }

    /// Set the line number.
    #[must_use]
    pub fn with_line(mut self, line: u32) -> Self {
        self.line = Some(line);
        self
    }

    /// Set the rule ID.
    #[must_use]
    pub fn with_rule_id(mut self, rule_id: impl Into<String>) -> Self {
        self.rule_id = Some(rule_id.into());
        self
    }

    /// Set the suggestion.
    #[must_use]
    pub fn with_suggestion(mut self, suggestion: impl Into<String>) -> Self {
        self.suggestion = Some(suggestion.into());
        self
    }

    /// Check if this finding blocks commits.
    pub fn is_blocking(&self) -> bool {
        self.severity.is_blocking()
    }
}

// ============================================================================
// Tool Response
// ============================================================================

/// Response from a narsil-mcp tool invocation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolResponse {
    /// The result data (if successful).
    pub result: serde_json::Value,
    /// Error message (if failed).
    pub error: Option<String>,
}

impl ToolResponse {
    /// Create a successful response.
    pub fn success(result: serde_json::Value) -> Self {
        Self { result, error: None }
    }

    /// Create an error response.
    pub fn error(message: impl Into<String>) -> Self {
        Self {
            result: serde_json::Value::Null,
            error: Some(message.into()),
        }
    }

    /// Check if this is a successful response.
    pub fn is_success(&self) -> bool {
        self.error.is_none()
    }
}

// ============================================================================
// NarsilClient
// ============================================================================

/// Client for invoking narsil-mcp tools.
///
/// The client gracefully handles the case where narsil-mcp is not available
/// by returning empty results instead of errors.
pub struct NarsilClient {
    config: NarsilConfig,
    available: bool,
}

impl NarsilClient {
    /// Create a new NarsilClient with the given configuration.
    ///
    /// This will check if narsil-mcp is available and cache the result.
    ///
    /// # Errors
    ///
    /// Returns an error if there's a problem initializing the client.
    /// Note: narsil-mcp not being available is NOT an error - use
    /// `is_available()` to check.
    pub fn new(config: NarsilConfig) -> Result<Self, NarsilError> {
        let available = Self::check_availability(&config.binary_path);
        Ok(Self { config, available })
    }

    /// Check if narsil-mcp is available.
    pub fn is_available(&self) -> bool {
        self.available
    }

    /// Run a security scan on the configured repository.
    ///
    /// Returns an empty vector if narsil-mcp is not available.
    ///
    /// # Errors
    ///
    /// Returns an error if the scan fails for reasons other than
    /// narsil-mcp not being available.
    pub fn scan_security(&self) -> Result<Vec<SecurityFinding>, NarsilError> {
        if !self.available {
            return Ok(Vec::new());
        }

        self.invoke_scan_security()
    }

    /// Run a security scan with a custom severity threshold.
    ///
    /// Returns an empty vector if narsil-mcp is not available.
    pub fn scan_security_with_threshold(
        &self,
        threshold: SecuritySeverity,
    ) -> Result<Vec<SecurityFinding>, NarsilError> {
        let findings = self.scan_security()?;
        Ok(findings
            .into_iter()
            .filter(|f| f.severity >= threshold)
            .collect())
    }

    /// Get the call graph for a function.
    ///
    /// Returns None if narsil-mcp is not available or the function is not found.
    pub fn get_call_graph(&self, function: &str) -> Result<Option<serde_json::Value>, NarsilError> {
        if !self.available {
            return Ok(None);
        }

        self.invoke_get_call_graph(function)
    }

    /// Find all references to a symbol.
    ///
    /// Returns an empty vector if narsil-mcp is not available.
    pub fn find_references(&self, symbol: &str) -> Result<Vec<Reference>, NarsilError> {
        if !self.available {
            return Ok(Vec::new());
        }

        self.invoke_find_references(symbol)
    }

    /// Get dependencies for a file.
    ///
    /// Returns an empty vector if narsil-mcp is not available.
    pub fn get_dependencies(&self, path: &str) -> Result<Vec<Dependency>, NarsilError> {
        if !self.available {
            return Ok(Vec::new());
        }

        self.invoke_get_dependencies(path)
    }

    // =========================================================================
    // Private Methods
    // =========================================================================

    fn check_availability(binary_path: &PathBuf) -> bool {
        Command::new(binary_path)
            .arg("--version")
            .output()
            .is_ok_and(|output| output.status.success())
    }

    fn invoke_scan_security(&self) -> Result<Vec<SecurityFinding>, NarsilError> {
        let output = Command::new(&self.config.binary_path)
            .arg("scan")
            .arg("--repo")
            .arg(&self.config.repo_path)
            .arg("--format")
            .arg("json")
            .output()?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(NarsilError::ToolError(format!(
                "scan_security failed: {}",
                stderr
            )));
        }

        let stdout = String::from_utf8_lossy(&output.stdout);
        self.parse_security_findings(&stdout)
    }

    fn parse_security_findings(&self, output: &str) -> Result<Vec<SecurityFinding>, NarsilError> {
        // Try to parse as JSON array of findings
        if output.trim().is_empty() {
            return Ok(Vec::new());
        }

        // Parse the JSON response
        let value: serde_json::Value =
            serde_json::from_str(output).map_err(|e| NarsilError::ParseError(e.to_string()))?;

        // Handle different response formats
        let findings_array = if let Some(findings) = value.get("findings") {
            findings.as_array()
        } else if value.is_array() {
            value.as_array()
        } else {
            return Ok(Vec::new());
        };

        let Some(findings_array) = findings_array else {
            return Ok(Vec::new());
        };

        let mut findings = Vec::new();
        for item in findings_array {
            if let Some(finding) = self.parse_single_finding(item) {
                findings.push(finding);
            }
        }

        Ok(findings)
    }

    fn parse_single_finding(&self, value: &serde_json::Value) -> Option<SecurityFinding> {
        let severity_str = value.get("severity")?.as_str()?;
        let severity = severity_str.parse().ok()?;
        let message = value.get("message")?.as_str()?.to_string();
        let file = value.get("file")?.as_str().map(PathBuf::from)?;

        let mut finding = SecurityFinding::new(severity, message, file);

        if let Some(line) = value.get("line").and_then(|v| v.as_u64()) {
            finding = finding.with_line(line as u32);
        }

        if let Some(rule_id) = value.get("rule_id").and_then(|v| v.as_str()) {
            finding = finding.with_rule_id(rule_id);
        }

        if let Some(suggestion) = value.get("suggestion").and_then(|v| v.as_str()) {
            finding = finding.with_suggestion(suggestion);
        }

        Some(finding)
    }

    fn invoke_get_call_graph(
        &self,
        function: &str,
    ) -> Result<Option<serde_json::Value>, NarsilError> {
        let output = Command::new(&self.config.binary_path)
            .arg("call-graph")
            .arg("--repo")
            .arg(&self.config.repo_path)
            .arg("--function")
            .arg(function)
            .arg("--format")
            .arg("json")
            .output()?;

        if !output.status.success() {
            // Function not found is not an error, just return None
            return Ok(None);
        }

        let stdout = String::from_utf8_lossy(&output.stdout);
        if stdout.trim().is_empty() {
            return Ok(None);
        }

        let value: serde_json::Value =
            serde_json::from_str(&stdout).map_err(|e| NarsilError::ParseError(e.to_string()))?;

        Ok(Some(value))
    }

    fn invoke_find_references(&self, symbol: &str) -> Result<Vec<Reference>, NarsilError> {
        let output = Command::new(&self.config.binary_path)
            .arg("find-references")
            .arg("--repo")
            .arg(&self.config.repo_path)
            .arg("--symbol")
            .arg(symbol)
            .arg("--format")
            .arg("json")
            .output()?;

        if !output.status.success() {
            return Ok(Vec::new());
        }

        let stdout = String::from_utf8_lossy(&output.stdout);
        self.parse_references(&stdout)
    }

    fn parse_references(&self, output: &str) -> Result<Vec<Reference>, NarsilError> {
        if output.trim().is_empty() {
            return Ok(Vec::new());
        }

        let value: serde_json::Value =
            serde_json::from_str(output).map_err(|e| NarsilError::ParseError(e.to_string()))?;

        let refs_array = if let Some(refs) = value.get("references") {
            refs.as_array()
        } else if value.is_array() {
            value.as_array()
        } else {
            return Ok(Vec::new());
        };

        let Some(refs_array) = refs_array else {
            return Ok(Vec::new());
        };

        let mut references = Vec::new();
        for item in refs_array {
            if let Some(reference) = self.parse_single_reference(item) {
                references.push(reference);
            }
        }

        Ok(references)
    }

    fn parse_single_reference(&self, value: &serde_json::Value) -> Option<Reference> {
        let file = value.get("file")?.as_str().map(PathBuf::from)?;
        let line = value.get("line")?.as_u64()? as u32;

        let mut reference = Reference::new(file, line);

        if let Some(column) = value.get("column").and_then(|v| v.as_u64()) {
            reference = reference.with_column(column as u32);
        }

        if let Some(context) = value.get("context").and_then(|v| v.as_str()) {
            reference = reference.with_context(context);
        }

        if let Some(kind) = value.get("kind").and_then(|v| v.as_str()) {
            reference = reference.with_kind(kind);
        }

        Some(reference)
    }

    fn invoke_get_dependencies(&self, path: &str) -> Result<Vec<Dependency>, NarsilError> {
        let output = Command::new(&self.config.binary_path)
            .arg("dependencies")
            .arg("--repo")
            .arg(&self.config.repo_path)
            .arg("--path")
            .arg(path)
            .arg("--format")
            .arg("json")
            .output()?;

        if !output.status.success() {
            return Ok(Vec::new());
        }

        let stdout = String::from_utf8_lossy(&output.stdout);
        self.parse_dependencies(&stdout)
    }

    fn parse_dependencies(&self, output: &str) -> Result<Vec<Dependency>, NarsilError> {
        if output.trim().is_empty() {
            return Ok(Vec::new());
        }

        let value: serde_json::Value =
            serde_json::from_str(output).map_err(|e| NarsilError::ParseError(e.to_string()))?;

        let deps_array = if let Some(deps) = value.get("dependencies") {
            deps.as_array()
        } else if let Some(deps) = value.get("imports") {
            deps.as_array()
        } else if value.is_array() {
            value.as_array()
        } else {
            return Ok(Vec::new());
        };

        let Some(deps_array) = deps_array else {
            return Ok(Vec::new());
        };

        let mut dependencies = Vec::new();
        for item in deps_array {
            if let Some(dep) = self.parse_single_dependency(item) {
                dependencies.push(dep);
            }
        }

        Ok(dependencies)
    }

    fn parse_single_dependency(&self, value: &serde_json::Value) -> Option<Dependency> {
        // Handle both string format and object format
        if let Some(path) = value.as_str() {
            return Some(Dependency::new(path));
        }

        let path = value.get("path")?.as_str()?;
        let mut dep = Dependency::new(path);

        if let Some(kind) = value.get("kind").and_then(|v| v.as_str()) {
            dep = dep.with_kind(kind);
        }

        Some(dep)
    }
}

// ============================================================================
// Reference Type
// ============================================================================

/// A reference to a symbol in the codebase.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Reference {
    /// File containing the reference.
    pub file: PathBuf,
    /// Line number.
    pub line: u32,
    /// Column number (if available).
    pub column: Option<u32>,
    /// Context around the reference.
    pub context: Option<String>,
    /// Kind of reference (call, import, definition, etc.).
    pub kind: Option<String>,
}

impl Reference {
    /// Create a new reference.
    pub fn new(file: impl Into<PathBuf>, line: u32) -> Self {
        Self {
            file: file.into(),
            line,
            column: None,
            context: None,
            kind: None,
        }
    }

    /// Set the column number.
    #[must_use]
    pub fn with_column(mut self, column: u32) -> Self {
        self.column = Some(column);
        self
    }

    /// Set the context.
    #[must_use]
    pub fn with_context(mut self, context: impl Into<String>) -> Self {
        self.context = Some(context.into());
        self
    }

    /// Set the kind.
    #[must_use]
    pub fn with_kind(mut self, kind: impl Into<String>) -> Self {
        self.kind = Some(kind.into());
        self
    }
}

// ============================================================================
// Dependency Type
// ============================================================================

/// A dependency of a file or module.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Dependency {
    /// Path to the dependency.
    pub path: PathBuf,
    /// Kind of dependency (import, use, etc.).
    pub kind: Option<String>,
}

impl Dependency {
    /// Create a new dependency.
    pub fn new(path: impl Into<PathBuf>) -> Self {
        Self {
            path: path.into(),
            kind: None,
        }
    }

    /// Set the kind of dependency.
    #[must_use]
    pub fn with_kind(mut self, kind: impl Into<String>) -> Self {
        self.kind = Some(kind.into());
        self
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_security_findings_empty() {
        let config = NarsilConfig::new(".");
        let client = NarsilClient::new(config).unwrap();

        let findings = client.parse_security_findings("").unwrap();
        assert!(findings.is_empty());
    }

    #[test]
    fn test_parse_security_findings_json_array() {
        let config = NarsilConfig::new(".");
        let client = NarsilClient::new(config).unwrap();

        let json = r#"[
            {
                "severity": "high",
                "message": "SQL injection",
                "file": "src/db.rs",
                "line": 42,
                "rule_id": "CWE-89"
            }
        ]"#;

        let findings = client.parse_security_findings(json).unwrap();
        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].severity, SecuritySeverity::High);
        assert_eq!(findings[0].message, "SQL injection");
        assert_eq!(findings[0].line, Some(42));
        assert_eq!(findings[0].rule_id, Some("CWE-89".to_string()));
    }

    #[test]
    fn test_parse_security_findings_wrapped_format() {
        let config = NarsilConfig::new(".");
        let client = NarsilClient::new(config).unwrap();

        let json = r#"{
            "findings": [
                {
                    "severity": "critical",
                    "message": "Command injection",
                    "file": "src/exec.rs",
                    "suggestion": "Use parameterized commands"
                }
            ]
        }"#;

        let findings = client.parse_security_findings(json).unwrap();
        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].severity, SecuritySeverity::Critical);
        assert_eq!(
            findings[0].suggestion,
            Some("Use parameterized commands".to_string())
        );
    }

    #[test]
    fn test_parse_references_json_array() {
        let config = NarsilConfig::new(".");
        let client = NarsilClient::new(config).unwrap();

        let json = r#"[
            {
                "file": "src/main.rs",
                "line": 10,
                "column": 5,
                "kind": "call"
            }
        ]"#;

        let refs = client.parse_references(json).unwrap();
        assert_eq!(refs.len(), 1);
        assert_eq!(refs[0].file.to_str().unwrap(), "src/main.rs");
        assert_eq!(refs[0].line, 10);
        assert_eq!(refs[0].column, Some(5));
        assert_eq!(refs[0].kind, Some("call".to_string()));
    }

    #[test]
    fn test_parse_dependencies_string_format() {
        let config = NarsilConfig::new(".");
        let client = NarsilClient::new(config).unwrap();

        let json = r#"["std::io", "crate::util"]"#;

        let deps = client.parse_dependencies(json).unwrap();
        assert_eq!(deps.len(), 2);
        assert_eq!(deps[0].path.to_str().unwrap(), "std::io");
        assert_eq!(deps[1].path.to_str().unwrap(), "crate::util");
    }

    #[test]
    fn test_parse_dependencies_object_format() {
        let config = NarsilConfig::new(".");
        let client = NarsilClient::new(config).unwrap();

        let json = r#"{
            "imports": [
                {"path": "std::io", "kind": "use"},
                {"path": "crate::util", "kind": "mod"}
            ]
        }"#;

        let deps = client.parse_dependencies(json).unwrap();
        assert_eq!(deps.len(), 2);
        assert_eq!(deps[0].kind, Some("use".to_string()));
        assert_eq!(deps[1].kind, Some("mod".to_string()));
    }

    #[test]
    fn test_reference_builder() {
        let reference = Reference::new("src/lib.rs", 42)
            .with_column(10)
            .with_context("fn foo()")
            .with_kind("definition");

        assert_eq!(reference.line, 42);
        assert_eq!(reference.column, Some(10));
        assert_eq!(reference.context, Some("fn foo()".to_string()));
        assert_eq!(reference.kind, Some("definition".to_string()));
    }

    #[test]
    fn test_dependency_builder() {
        let dep = Dependency::new("std::collections").with_kind("use");

        assert_eq!(dep.path.to_str().unwrap(), "std::collections");
        assert_eq!(dep.kind, Some("use".to_string()));
    }
}
