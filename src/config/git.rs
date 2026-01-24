//! Git and security-related configuration and utilities.
//!
//! This module contains patterns for detecting dangerous commands, secrets,
//! and SSH operations that should be blocked in favor of `gh` CLI usage.

use std::process::Command;

// ============================================================================
// Dangerous Command Patterns
// ============================================================================

/// Dangerous command patterns to block.
///
/// These patterns are used to detect potentially destructive commands
/// that could harm the system or project.
pub const DANGEROUS_PATTERNS: &[&str] = &[
    "rm -rf /",
    "rm -rf ~",
    "rm -rf /*",
    ":(){:|:&};:",
    "dd if=/dev/zero",
    "mkfs.",
    "> /dev/sd",
    "chmod 777",
    "chmod -R 777",
    "sudo rm",
    "sudo dd",
];

/// Secret patterns to detect in code.
///
/// These regex patterns are used to detect potential secrets, API keys,
/// and other sensitive information that should not be committed.
pub const SECRET_PATTERNS: &[&str] = &[
    r#"(?i)(api[_-]?key|apikey)\s*[:=]\s*['"][^'"]+['"]"#,
    r#"(?i)(password|passwd|pwd)\s*[:=]\s*['"][^'"]+['"]"#,
    r#"(?i)(secret|token)\s*[:=]\s*['"][^'"]+['"]"#,
    r"(?i)(aws[_-]?access[_-]?key|aws[_-]?secret)",
    r"(?i)(private[_-]?key)\s*[:=]",
    r"-----BEGIN (RSA |DSA |EC |OPENSSH )?PRIVATE KEY-----",
];

// ============================================================================
// SSH Blocking Patterns
// ============================================================================

/// SSH-related patterns that should be blocked to enforce gh CLI usage.
///
/// Ralph requires the `gh` CLI for all GitHub operations. These patterns
/// detect attempts to use SSH keys or SSH-based git operations that should
/// be replaced with `gh` CLI commands.
pub const SSH_BLOCKED_PATTERNS: &[&str] = &[
    // SSH key generation/management
    "ssh-keygen",
    "ssh-add",
    "ssh-agent",
    "eval $(ssh-agent",
    "ssh-agent -s",
    // SSH key file access
    "cat ~/.ssh",
    "cat /home/*/.ssh",
    "ls ~/.ssh",
    "ls /home/*/.ssh",
    "cat *id_rsa*",
    "cat *id_ed25519*",
    "cat *id_ecdsa*",
    "cat *id_dsa*",
    "cat *known_hosts*",
    "cat *authorized_keys*",
    // SSH directory operations
    "~/.ssh/",
    "/home/*/.ssh/",
    ".ssh/id_",
    ".ssh/config",
    // SSH connection attempts that should use gh
    "git@github.com:",
    "git clone git@",
    "git remote add * git@",
    "git remote set-url * git@",
    // Copying SSH keys
    "cp *id_rsa*",
    "cp *id_ed25519*",
    "scp *id_rsa*",
    "scp *id_ed25519*",
];

/// Check if a command attempts SSH operations.
///
/// # Arguments
///
/// * `command` - The command string to check
///
/// # Returns
///
/// `true` if the command matches any SSH blocking pattern
///
/// # Examples
///
/// ```
/// use ralph::config::git::is_ssh_command;
///
/// assert!(is_ssh_command("ssh-keygen -t rsa"));
/// assert!(is_ssh_command("git clone git@github.com:user/repo.git"));
/// assert!(!is_ssh_command("git status"));
/// assert!(!is_ssh_command("gh repo clone user/repo"));
/// ```
pub fn is_ssh_command(command: &str) -> bool {
    SSH_BLOCKED_PATTERNS.iter().any(|pattern| {
        // Handle glob patterns with * - simplified matching
        if pattern.contains('*') {
            // For patterns like "git remote add * git@", check if command contains both parts
            let parts: Vec<&str> = pattern.split('*').collect();
            if parts.len() == 2 {
                let prefix = parts[0];
                let suffix = parts[1];
                if !prefix.is_empty() && !suffix.is_empty() {
                    // Both prefix and suffix exist
                    if let Some(prefix_pos) = command.find(prefix) {
                        let rest = &command[prefix_pos + prefix.len()..];
                        return rest.contains(suffix);
                    }
                    return false;
                }
            }
        }
        command.contains(pattern)
    })
}

/// Get alternative gh command for blocked SSH operation.
///
/// Provides helpful suggestions for replacing SSH-based git operations
/// with `gh` CLI equivalents.
///
/// # Arguments
///
/// * `command` - The blocked SSH command
///
/// # Returns
///
/// `Some(String)` with the suggested alternative, or `None` if no suggestion available
///
/// # Examples
///
/// ```
/// use ralph::config::git::suggest_gh_alternative;
///
/// let alt = suggest_gh_alternative("git clone git@github.com:user/repo.git");
/// assert!(alt.is_some());
/// assert!(alt.unwrap().contains("gh repo clone"));
/// ```
pub fn suggest_gh_alternative(command: &str) -> Option<String> {
    // git@github.com:user/repo.git -> gh repo clone user/repo
    if command.contains("git clone git@github.com:") {
        if let Some(repo_part) = command.split("git@github.com:").nth(1) {
            let repo = repo_part.trim_end_matches(".git").trim();
            return Some(format!("gh repo clone {}", repo));
        }
    }

    // git remote add origin git@... -> gh repo set-default
    if command.contains("git remote") && command.contains("git@github.com:") {
        return Some("Use 'gh repo set-default' instead of git remote with SSH URL".to_string());
    }

    // ssh-keygen suggestions
    if command.contains("ssh-keygen") || command.contains("ssh-add") {
        return Some(
            "gh CLI handles authentication - no SSH keys needed. Run 'gh auth login'".to_string(),
        );
    }

    None
}

// ============================================================================
// Git Environment Verification
// ============================================================================

/// Result of git environment verification.
///
/// This struct contains the results of checking whether the git environment
/// is properly configured for Ralph, including `gh` CLI authentication status.
#[derive(Debug)]
pub struct GitEnvironmentCheck {
    /// gh CLI is installed
    pub gh_installed: bool,
    /// gh CLI is authenticated
    pub gh_authenticated: bool,
    /// git is installed
    pub git_installed: bool,
    /// Current git user
    pub git_user: Option<String>,
    /// Current git email
    pub git_email: Option<String>,
    /// SSH agent running (warning if true - should use gh)
    pub ssh_agent_running: bool,
    /// Errors encountered
    pub errors: Vec<String>,
    /// Warnings
    pub warnings: Vec<String>,
}

impl GitEnvironmentCheck {
    /// Check if environment is ready for Ralph.
    ///
    /// Returns `true` if git is installed, gh CLI is installed, and gh CLI is authenticated.
    #[must_use]
    pub fn is_ready(&self) -> bool {
        self.gh_installed && self.gh_authenticated && self.git_installed
    }
}

/// Verify the git environment is properly configured for Ralph.
///
/// Checks for:
/// - git installation
/// - gh CLI installation and authentication
/// - git user configuration
/// - SSH agent status (warns if running)
///
/// # Returns
///
/// A `GitEnvironmentCheck` struct with the results of all checks.
///
/// # Examples
///
/// ```no_run
/// use ralph::config::git::verify_git_environment;
///
/// let check = verify_git_environment();
/// if !check.is_ready() {
///     for error in &check.errors {
///         eprintln!("Error: {}", error);
///     }
/// }
/// ```
#[must_use]
pub fn verify_git_environment() -> GitEnvironmentCheck {
    let mut check = GitEnvironmentCheck {
        gh_installed: false,
        gh_authenticated: false,
        git_installed: false,
        git_user: None,
        git_email: None,
        ssh_agent_running: false,
        errors: Vec::new(),
        warnings: Vec::new(),
    };

    // Check git installed
    match Command::new("git").args(["--version"]).output() {
        Ok(output) if output.status.success() => {
            check.git_installed = true;
        }
        _ => {
            check.errors.push("git not installed".to_string());
        }
    }

    // Get git user config
    if let Ok(output) = Command::new("git").args(["config", "user.name"]).output() {
        if output.status.success() {
            check.git_user = Some(String::from_utf8_lossy(&output.stdout).trim().to_string());
        }
    }

    if let Ok(output) = Command::new("git").args(["config", "user.email"]).output() {
        if output.status.success() {
            check.git_email = Some(String::from_utf8_lossy(&output.stdout).trim().to_string());
        }
    }

    // Check gh CLI installed
    match Command::new("gh").args(["--version"]).output() {
        Ok(output) if output.status.success() => {
            check.gh_installed = true;
        }
        _ => {
            check
                .errors
                .push("gh CLI not installed - required for GitHub operations".to_string());
        }
    }

    // Check gh CLI authenticated
    if check.gh_installed {
        match Command::new("gh").args(["auth", "status"]).output() {
            Ok(output) if output.status.success() => {
                check.gh_authenticated = true;
            }
            Ok(output) => {
                let stderr = String::from_utf8_lossy(&output.stderr);
                check.errors.push(format!(
                    "gh CLI not authenticated - run 'gh auth login': {}",
                    stderr.trim()
                ));
            }
            Err(e) => {
                check
                    .errors
                    .push(format!("Failed to check gh auth status: {}", e));
            }
        }
    }

    // Check if SSH agent is running (warning - should use gh instead)
    if std::env::var("SSH_AUTH_SOCK").is_ok() {
        check.ssh_agent_running = true;
        check
            .warnings
            .push("SSH agent detected - Ralph prefers gh CLI for GitHub operations".to_string());
    }

    check
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_dangerous_patterns() {
        assert!(DANGEROUS_PATTERNS.contains(&"rm -rf /"));
        assert!(DANGEROUS_PATTERNS.contains(&"chmod 777"));
    }

    #[test]
    fn test_secret_patterns_valid_regex() {
        for pattern in SECRET_PATTERNS {
            assert!(
                regex::Regex::new(pattern).is_ok(),
                "Invalid pattern: {}",
                pattern
            );
        }
    }

    // ============================================================================
    // SSH Blocking Tests
    // ============================================================================

    #[test]
    fn test_ssh_patterns_blocked() {
        assert!(is_ssh_command("ssh-keygen -t rsa"));
        assert!(is_ssh_command("ssh-add ~/.ssh/id_rsa"));
        assert!(is_ssh_command("ssh-agent -s"));
        assert!(is_ssh_command("cat ~/.ssh/id_rsa"));
        assert!(is_ssh_command("ls ~/.ssh"));
        assert!(is_ssh_command("git clone git@github.com:user/repo.git"));
        assert!(is_ssh_command(
            "git remote add origin git@github.com:user/repo"
        ));
    }

    #[test]
    fn test_non_ssh_commands_allowed() {
        assert!(!is_ssh_command("git status"));
        assert!(!is_ssh_command("gh repo clone user/repo"));
        assert!(!is_ssh_command("cat README.md"));
        assert!(!is_ssh_command("ls -la"));
        assert!(!is_ssh_command("git push origin main"));
    }

    #[test]
    fn test_suggest_gh_alternative_clone() {
        let alt = suggest_gh_alternative("git clone git@github.com:user/repo.git");
        assert!(alt.is_some());
        let suggestion = alt.unwrap();
        assert!(suggestion.contains("gh repo clone"));
        assert!(suggestion.contains("user/repo"));
    }

    #[test]
    fn test_suggest_gh_alternative_keygen() {
        let alt = suggest_gh_alternative("ssh-keygen -t ed25519");
        assert!(alt.is_some());
        let suggestion = alt.unwrap();
        assert!(suggestion.contains("gh auth login"));
    }

    #[test]
    fn test_suggest_gh_alternative_remote() {
        let alt = suggest_gh_alternative("git remote add origin git@github.com:user/repo");
        assert!(alt.is_some());
        let suggestion = alt.unwrap();
        assert!(suggestion.contains("gh repo set-default"));
    }

    #[test]
    fn test_suggest_gh_alternative_none() {
        let alt = suggest_gh_alternative("git status");
        assert!(alt.is_none());
    }

    // ============================================================================
    // Git Environment Check Tests
    // ============================================================================

    #[test]
    fn test_git_environment_check_is_ready() {
        let check = GitEnvironmentCheck {
            gh_installed: true,
            gh_authenticated: true,
            git_installed: true,
            git_user: Some("test".to_string()),
            git_email: Some("test@example.com".to_string()),
            ssh_agent_running: false,
            errors: Vec::new(),
            warnings: Vec::new(),
        };
        assert!(check.is_ready());
    }

    #[test]
    fn test_git_environment_check_not_ready_no_gh() {
        let check = GitEnvironmentCheck {
            gh_installed: false,
            gh_authenticated: false,
            git_installed: true,
            git_user: None,
            git_email: None,
            ssh_agent_running: false,
            errors: vec!["gh CLI not installed".to_string()],
            warnings: Vec::new(),
        };
        assert!(!check.is_ready());
    }

    #[test]
    fn test_git_environment_check_not_ready_no_auth() {
        let check = GitEnvironmentCheck {
            gh_installed: true,
            gh_authenticated: false,
            git_installed: true,
            git_user: None,
            git_email: None,
            ssh_agent_running: false,
            errors: vec!["gh CLI not authenticated".to_string()],
            warnings: Vec::new(),
        };
        assert!(!check.is_ready());
    }
}
