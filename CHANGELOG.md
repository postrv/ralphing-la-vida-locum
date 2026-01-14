# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added

- **Custom error types** - `RalphError` enum with 18 error variants for structured error handling
  - `IntoRalphError` extension trait for converting standard errors
  - Classification helpers: `is_recoverable()`, `requires_human()`, `is_fatal()`
  - Exit code mapping for proper CLI behavior
- **Supervisor system (Chief Wiggum)** - Loop health monitoring with automatic intervention
  - `SupervisorVerdict` enum: Proceed, PauseForReview, Abort, SwitchMode, Reset
  - `HealthMetrics` for test pass rate, clippy warnings, iteration tracking
  - `DiagnosticReport` for debugging stalled loops
  - Pattern detection for mode oscillation, repeating errors
- **Stagnation levels** - Multi-tier escalation for handling stuck loops
  - `StagnationLevel` enum: None, Warning, Elevated, Critical
  - Automatic mode switching at Warning level
  - Supervisor invocation at Elevated level
  - Abort with diagnostics at Critical level
- **SSH blocking** - Enforce `gh` CLI usage for all GitHub operations
  - 33 SSH-related patterns blocked
  - `is_ssh_command()` for pattern matching
  - `suggest_gh_alternative()` for helpful CLI suggestions
  - `GitEnvironmentCheck` for verifying proper authentication

### Changed

- **Session init hook** - Enhanced with environment verification
  - git, gh CLI, and narsil-mcp checks
  - Project state checks (uncommitted changes, branch, task progress)
  - Analytics logging with warnings count
- **Security filter hook** - Expanded with SSH blocking
  - SSH key operations blocked (keygen, agent, add)
  - SSH key file access blocked (~/.ssh/, id_rsa, etc.)
  - Git SSH URLs blocked (git@github.com:)
  - Warning patterns for risky but allowed commands
- **CLAUDE.md template** - Added new sections
  - GIT AUTHENTICATION section with rules and command mappings
  - STAGNATION HANDLING section with level table
  - SUPERVISOR section with verdict explanations
  - QUICK REFERENCE section with common commands

### Security

- **SSH blocking enforced** - All SSH operations blocked, gh CLI required
- **SECURITY.md policy** - Vulnerability reporting and security features documented
- **CI security audit** - `cargo audit` and secret scanning in CI workflow

### Documentation

- **README updated** - Supervisor, stagnation, quality gates documented
- **SECURITY.md added** - Security policy and vulnerability reporting

### CI/CD

- **Comprehensive CI workflow** - Added `.github/workflows/ci.yml`
  - Forbidden pattern checks (#[allow], TODO/FIXME)
  - Security audit with cargo-audit
  - Integration tests for bootstrap, config, hooks
  - Multi-platform builds (Linux, macOS)
  - Release job for tagged versions

## [0.1.0] - 2024-XX-XX

### Added

- **Bootstrap command** - Initialize projects with Claude Code configuration, hooks, skills, and 22 templates
- **Context builder** - Generate LLM-optimized context bundles respecting gitignore and token limits
- **Execution loop** - Run Claude Code autonomously with configurable phases (plan, build, debug)
- **Stagnation detection** - Monitor git commits to detect when progress stalls
- **Security hooks** - Pre-validate commands against 46 dangerous patterns
- **Secret scanning** - Detect API keys, passwords, and private keys in files
- **Documentation archive** - Manage stale documentation lifecycle
- **Analytics** - Track sessions, iterations, and events in JSONL format
- **File size monitoring** - Auto-archive integration for large files

### Security

- Hardcoded blocking of dangerous commands (rm -rf /, chmod 777, fork bombs, etc.)
- Allow/deny permission lists in settings.json
- Secret detection with 6 regex patterns
