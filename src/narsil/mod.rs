//! narsil-mcp integration module.
//!
//! This module provides integration with narsil-mcp for code intelligence
//! operations including security scanning, call graph analysis, and
//! reference finding.
//!
//! # Architecture
//!
//! The module uses MCP (Model Context Protocol) to communicate with narsil-mcp
//! via JSON-RPC over stdio. When narsil-mcp is not available, operations
//! gracefully degrade and return empty results.
//!
//! # Example
//!
//! ```rust,ignore
//! use ralph::narsil::{NarsilClient, NarsilConfig};
//!
//! // Create a client with default config
//! let client = NarsilClient::new(NarsilConfig::default())?;
//!
//! // Check if narsil-mcp is available
//! if client.is_available() {
//!     // Run a security scan
//!     let findings = client.scan_security(".")?;
//!     for finding in findings {
//!         println!("{}: {}", finding.severity, finding.message);
//!     }
//! }
//! ```

mod ccg;
mod client;
mod constraint_loader;
mod intelligence;

pub use ccg::{
    CcgArchitecture, CcgCache, CcgConstraint, CcgManifest, ComplianceResult, ConstraintKind,
    ConstraintSet, ConstraintSeverity, ConstraintValue, ConstraintViolation, DependencyKind,
    EntryPoint, EntryPointKind, LanguageStats, Module, ModuleDependency, PublicSymbol,
    SecuritySummary, SymbolKind, Visibility,
};
pub use client::{
    Dependency, NarsilClient, NarsilConfig, NarsilError, Reference, SecurityFinding,
    SecuritySeverity, ToolResponse,
};
pub use constraint_loader::{ConstraintLoadError, ConstraintLoader, DEFAULT_CONSTRAINTS_PATH};
pub use intelligence::CodeIntelligenceBuilder;

#[cfg(test)]
mod tests {
    use super::*;

    // =========================================================================
    // NarsilConfig Tests
    // =========================================================================

    #[test]
    fn test_narsil_config_default() {
        let config = NarsilConfig::default();
        assert!(!config.repo_path.as_os_str().is_empty());
        assert!(config.timeout_ms > 0);
    }

    #[test]
    fn test_narsil_config_builder() {
        let config = NarsilConfig::new(".")
            .with_timeout_ms(5000)
            .with_git(true)
            .with_call_graph(true);

        assert_eq!(config.timeout_ms, 5000);
        assert!(config.git_enabled);
        assert!(config.call_graph_enabled);
    }

    // =========================================================================
    // SecuritySeverity Tests
    // =========================================================================

    #[test]
    fn test_security_severity_ordering() {
        assert!(SecuritySeverity::Info < SecuritySeverity::Low);
        assert!(SecuritySeverity::Low < SecuritySeverity::Medium);
        assert!(SecuritySeverity::Medium < SecuritySeverity::High);
        assert!(SecuritySeverity::High < SecuritySeverity::Critical);
    }

    #[test]
    fn test_security_severity_is_blocking() {
        assert!(!SecuritySeverity::Info.is_blocking());
        assert!(!SecuritySeverity::Low.is_blocking());
        assert!(!SecuritySeverity::Medium.is_blocking());
        assert!(SecuritySeverity::High.is_blocking());
        assert!(SecuritySeverity::Critical.is_blocking());
    }

    #[test]
    fn test_security_severity_from_str() {
        assert_eq!(
            "critical".parse::<SecuritySeverity>().unwrap(),
            SecuritySeverity::Critical
        );
        assert_eq!(
            "HIGH".parse::<SecuritySeverity>().unwrap(),
            SecuritySeverity::High
        );
        assert_eq!(
            "Medium".parse::<SecuritySeverity>().unwrap(),
            SecuritySeverity::Medium
        );
        assert_eq!(
            "low".parse::<SecuritySeverity>().unwrap(),
            SecuritySeverity::Low
        );
        assert_eq!(
            "info".parse::<SecuritySeverity>().unwrap(),
            SecuritySeverity::Info
        );
        assert!("unknown".parse::<SecuritySeverity>().is_err());
    }

    #[test]
    fn test_security_severity_display() {
        assert_eq!(SecuritySeverity::Critical.to_string(), "CRITICAL");
        assert_eq!(SecuritySeverity::High.to_string(), "HIGH");
        assert_eq!(SecuritySeverity::Medium.to_string(), "MEDIUM");
        assert_eq!(SecuritySeverity::Low.to_string(), "LOW");
        assert_eq!(SecuritySeverity::Info.to_string(), "INFO");
    }

    // =========================================================================
    // SecurityFinding Tests
    // =========================================================================

    #[test]
    fn test_security_finding_new() {
        let finding = SecurityFinding::new(
            SecuritySeverity::High,
            "SQL injection vulnerability",
            "src/db.rs",
        );

        assert_eq!(finding.severity, SecuritySeverity::High);
        assert_eq!(finding.message, "SQL injection vulnerability");
        assert_eq!(finding.file.to_str().unwrap(), "src/db.rs");
        assert!(finding.line.is_none());
        assert!(finding.rule_id.is_none());
    }

    #[test]
    fn test_security_finding_builder() {
        let finding = SecurityFinding::new(SecuritySeverity::Critical, "XSS vulnerability", "src/web.rs")
            .with_line(42)
            .with_rule_id("CWE-79")
            .with_suggestion("Use HTML escaping");

        assert_eq!(finding.line, Some(42));
        assert_eq!(finding.rule_id, Some("CWE-79".to_string()));
        assert_eq!(finding.suggestion, Some("Use HTML escaping".to_string()));
    }

    #[test]
    fn test_security_finding_is_blocking() {
        let high = SecurityFinding::new(SecuritySeverity::High, "test", "file.rs");
        let medium = SecurityFinding::new(SecuritySeverity::Medium, "test", "file.rs");

        assert!(high.is_blocking());
        assert!(!medium.is_blocking());
    }

    // =========================================================================
    // ToolResponse Tests
    // =========================================================================

    #[test]
    fn test_tool_response_success() {
        let response = ToolResponse::success(serde_json::json!({"key": "value"}));
        assert!(response.is_success());
        assert!(response.error.is_none());
        assert_eq!(response.result["key"], "value");
    }

    #[test]
    fn test_tool_response_error() {
        let response = ToolResponse::error("Tool not found");
        assert!(!response.is_success());
        assert_eq!(response.error, Some("Tool not found".to_string()));
    }

    // =========================================================================
    // NarsilClient Tests
    // =========================================================================

    #[test]
    fn test_narsil_client_new_with_config() {
        let config = NarsilConfig::new(".");
        let client = NarsilClient::new(config);

        // Client should be created even if narsil-mcp is not available
        assert!(client.is_ok());
    }

    #[test]
    fn test_narsil_client_is_available_returns_false_when_not_installed() {
        // This test verifies graceful handling when narsil-mcp is not installed
        let config = NarsilConfig::new(".")
            .with_binary_path("/nonexistent/narsil-mcp");
        let client = NarsilClient::new(config).unwrap();

        assert!(!client.is_available());
    }

    #[test]
    fn test_narsil_client_scan_security_returns_empty_when_unavailable() {
        let config = NarsilConfig::new(".")
            .with_binary_path("/nonexistent/narsil-mcp");
        let client = NarsilClient::new(config).unwrap();

        // Should return empty vec, not an error
        let result = client.scan_security();
        assert!(result.is_ok());
        assert!(result.unwrap().is_empty());
    }

    // =========================================================================
    // NarsilError Tests
    // =========================================================================

    #[test]
    fn test_narsil_error_display() {
        let err = NarsilError::Unavailable("not installed".to_string());
        assert!(err.to_string().contains("not installed"));

        let err = NarsilError::Timeout(5000);
        assert!(err.to_string().contains("5000"));

        let err = NarsilError::ParseError("invalid json".to_string());
        assert!(err.to_string().contains("invalid json"));
    }

    #[test]
    fn test_narsil_error_is_recoverable() {
        assert!(NarsilError::Unavailable("test".to_string()).is_recoverable());
        assert!(NarsilError::Timeout(1000).is_recoverable());
        assert!(!NarsilError::ParseError("test".to_string()).is_recoverable());
    }
}
