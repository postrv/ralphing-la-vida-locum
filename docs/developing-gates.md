# Developing Quality Gates

This guide explains how to create custom quality gates for Ralph. Quality gates are checks that run before commits to ensure code meets project standards.

## Table of Contents

- [Overview](#overview)
- [Key Concepts](#key-concepts)
- [Built-in Gates vs Plugins](#built-in-gates-vs-plugins)
- [Implementing the QualityGate Trait](#implementing-the-qualitygate-trait)
- [Implementing the GatePlugin Trait](#implementing-the-gateplugin-trait)
- [Testing Strategies](#testing-strategies)
- [Complete Examples](#complete-examples)
- [Contributing a Gate](#contributing-a-gate)

---

## Overview

Ralph's quality gate system provides a pluggable architecture for code quality enforcement. Gates check specific aspects of code quality and return structured results that Ralph uses for:

- **Commit blocking** - Preventing commits when issues are found
- **Remediation guidance** - Generating fix suggestions for Claude Code
- **Analytics tracking** - Recording quality trends over time

### Architecture

```
                      ┌─────────────────────┐
                      │ QualityGateEnforcer │
                      │                     │
                      │  - run_all()        │
                      │  - can_commit()     │
                      └─────────┬───────────┘
                                │
          ┌─────────────────────┼─────────────────────┐
          │                     │                     │
          ▼                     ▼                     ▼
   ┌─────────────┐      ┌─────────────┐      ┌─────────────┐
   │ Built-in    │      │ Built-in    │      │  External   │
   │ Gates       │      │ Gates       │      │  Plugins    │
   │             │      │             │      │             │
   │ - Clippy    │      │ - Ruff      │      │ - RuboCop   │
   │ - Tests     │      │ - Pytest    │      │ - Custom    │
   │ - NoAllow   │      │ - Mypy      │      │             │
   └─────────────┘      └─────────────┘      └─────────────┘
          │                     │                     │
          └─────────────────────┼─────────────────────┘
                                ▼
                      ┌─────────────────────┐
                      │RemediationGenerator │
                      │                     │
                      │  generate_prompt()  │
                      └─────────────────────┘
```

---

## Key Concepts

### GateIssue

A `GateIssue` represents a single problem found by a gate:

```rust
use ralph::quality::gates::{GateIssue, IssueSeverity};

// Basic issue
let issue = GateIssue::new(IssueSeverity::Error, "Unused variable 'x'");

// Issue with full context
let issue = GateIssue::new(IssueSeverity::Warning, "Line too long (120 > 100)")
    .with_location("src/lib.rs", 42)
    .with_column(100)
    .with_code("E501")
    .with_suggestion("Break line into multiple statements");
```

### IssueSeverity

Severity levels determine whether issues block commits:

| Severity   | Blocking | Description                        |
|------------|----------|------------------------------------|
| `Info`     | No       | Informational, doesn't block       |
| `Warning`  | No       | Should fix, but doesn't block      |
| `Error`    | **Yes**  | Must fix before commit             |
| `Critical` | **Yes**  | Security issue, fix immediately    |

```rust
use ralph::quality::gates::IssueSeverity;

let severity = IssueSeverity::Error;
assert!(severity.is_blocking()); // true

let severity = IssueSeverity::Warning;
assert!(!severity.is_blocking()); // false
```

### GateResult

A `GateResult` summarizes the output of running a gate:

```rust
use ralph::quality::gates::{GateResult, GateIssue, IssueSeverity};

// Passing result
let result = GateResult::pass("MyGate");

// Failing result with issues
let issues = vec![
    GateIssue::new(IssueSeverity::Error, "Syntax error"),
];
let result = GateResult::fail("MyGate", issues)
    .with_output("Raw tool output here")
    .with_duration(150); // milliseconds
```

---

## Built-in Gates vs Plugins

Ralph supports two ways to add quality gates:

| Aspect            | Built-in Gates                    | Plugin Gates                     |
|-------------------|-----------------------------------|----------------------------------|
| **Location**      | `src/quality/gates/`              | External shared library          |
| **Distribution**  | Part of Ralph binary              | Separate package                 |
| **Use case**      | Core language support             | Custom/third-party tools         |
| **Compilation**   | Compiled with Ralph               | Compiled separately              |
| **Discovery**     | Automatic via `gates_for_language`| Via plugin loader from directory |

**Choose built-in** when:
- Adding support for a major language (Python, Go, etc.)
- The gate is generally useful to all Ralph users
- You want to contribute to Ralph core

**Choose plugin** when:
- Adding a niche or company-specific tool
- Rapid iteration without rebuilding Ralph
- Distributing separately from Ralph

---

## Implementing the QualityGate Trait

The `QualityGate` trait is the core interface for all quality gates.

### Trait Definition

```rust
pub trait QualityGate: Send + Sync {
    /// Returns the display name of this gate.
    fn name(&self) -> &str;

    /// Runs the quality gate check on the given project.
    fn run(&self, project_dir: &Path) -> Result<Vec<GateIssue>>;

    /// Returns whether this gate blocks commits on failure.
    fn is_blocking(&self) -> bool {
        true // Default: blocking
    }

    /// Generates remediation guidance for the given issues.
    fn remediation(&self, issues: &[GateIssue]) -> String;

    /// Returns the name of the external tool required (if any).
    fn required_tool(&self) -> Option<&str> {
        None // Default: no external tool
    }
}
```

### Minimal Implementation

```rust
use std::path::Path;
use anyhow::Result;
use ralph::quality::gates::{QualityGate, GateIssue, IssueSeverity};

/// A simple gate that checks for TODO comments.
pub struct TodoCheckerGate;

impl QualityGate for TodoCheckerGate {
    fn name(&self) -> &str {
        "TodoChecker"
    }

    fn run(&self, project_dir: &Path) -> Result<Vec<GateIssue>> {
        let mut issues = Vec::new();

        // Walk through source files and check for TODOs
        for entry in walkdir::WalkDir::new(project_dir)
            .into_iter()
            .filter_map(|e| e.ok())
            .filter(|e| e.path().extension().map_or(false, |ext| ext == "rs"))
        {
            let content = std::fs::read_to_string(entry.path())?;
            for (line_num, line) in content.lines().enumerate() {
                if line.contains("TODO") || line.contains("FIXME") {
                    issues.push(
                        GateIssue::new(IssueSeverity::Warning, "Found TODO comment")
                            .with_location(entry.path(), (line_num + 1) as u32)
                    );
                }
            }
        }

        Ok(issues)
    }

    fn is_blocking(&self) -> bool {
        false // TODOs are warnings, not blocking
    }

    fn remediation(&self, issues: &[GateIssue]) -> String {
        format!(
            "## TODO Comments Found\n\n\
             Found {} TODO/FIXME comments.\n\n\
             **Resolution:**\n\
             - Complete the TODO items, or\n\
             - Remove outdated TODOs, or\n\
             - Convert to tracked issues\n",
            issues.len()
        )
    }
}
```

### External Tool Integration

For gates that wrap external tools (linters, test runners):

```rust
use std::path::Path;
use std::process::Command;
use anyhow::{Context, Result};
use ralph::quality::gates::{QualityGate, GateIssue, IssueSeverity};

/// A gate that runs an external linter.
pub struct ExternalLinterGate;

impl QualityGate for ExternalLinterGate {
    fn name(&self) -> &str {
        "ExternalLinter"
    }

    fn run(&self, project_dir: &Path) -> Result<Vec<GateIssue>> {
        // Run the external tool
        let output = Command::new("my-linter")
            .arg("--format=json")
            .current_dir(project_dir)
            .output()
            .context("Failed to execute my-linter. Is it installed?")?;

        let stdout = String::from_utf8_lossy(&output.stdout);

        // Parse the output (tool-specific)
        if output.status.success() {
            Ok(Vec::new())
        } else {
            self.parse_output(&stdout)
        }
    }

    fn remediation(&self, issues: &[GateIssue]) -> String {
        format!(
            "## Linter Issues\n\n\
             Found {} issues.\n\n\
             Run `my-linter --fix` to auto-fix.\n",
            issues.len()
        )
    }

    fn required_tool(&self) -> Option<&str> {
        Some("my-linter") // Ralph checks this is available before running
    }
}

impl ExternalLinterGate {
    fn parse_output(&self, output: &str) -> Result<Vec<GateIssue>> {
        // Parse tool-specific JSON/text output into GateIssues
        // This is highly dependent on the tool's output format
        Ok(vec![])
    }
}
```

---

## Implementing the GatePlugin Trait

For external plugins, implement both `QualityGate` and `GatePlugin`:

### Trait Definition

```rust
pub trait GatePlugin: QualityGate {
    /// Returns metadata about this plugin.
    fn metadata(&self) -> PluginMetadata;

    /// Returns the maximum execution time.
    fn timeout(&self) -> Duration {
        Duration::from_secs(60)
    }

    /// Called when the plugin is loaded.
    fn on_load(&self) -> Result<()> {
        Ok(())
    }

    /// Called when the plugin is unloaded.
    fn on_unload(&self) {
        // Default: no cleanup
    }

    /// Apply configuration to the plugin.
    fn configure(&mut self, config: &PluginConfig) -> Result<()> {
        Ok(())
    }
}
```

### PluginMetadata

Every plugin must provide metadata:

```rust
use ralph::quality::plugin::PluginMetadata;

let metadata = PluginMetadata::new("my-gate", "1.0.0", "Your Name")
    .with_description("Description of what this gate checks")
    .with_homepage("https://github.com/you/my-gate")
    .with_license("MIT");
```

### Plugin Manifest (plugin.toml)

Plugins require a manifest file:

```toml
[plugin]
name = "my-custom-gate"
version = "1.0.0"
author = "Your Name"
description = "A custom quality gate for XYZ"
homepage = "https://github.com/you/my-gate"
license = "MIT"

[library]
# Path to shared library (relative to plugin.toml)
path = "target/release/libmy_gate.dylib"
# Entry point function name
entry_point = "create_gate_plugin"

[config]
# Maximum execution time
timeout = "60s"
# Whether enabled by default
enabled = true
```

### Entry Point Function

Plugins must export a C-compatible entry point:

```rust
/// Entry point for dynamic loading.
///
/// # Safety
///
/// Caller must use `Box::from_raw` to take ownership of the returned pointer.
#[no_mangle]
pub extern "C" fn create_gate_plugin() -> *mut MyPlugin {
    Box::into_raw(Box::new(MyPlugin::new()))
}
```

### Complete Plugin Example

See [examples/plugins/rubocop-gate/](../examples/plugins/rubocop-gate/) for a complete plugin implementation.

---

## Testing Strategies

### Unit Tests

Test your gate logic in isolation:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_gate_name() {
        let gate = MyGate::new();
        assert_eq!(gate.name(), "MyGate");
    }

    #[test]
    fn test_parse_output_empty() {
        let gate = MyGate::new();
        let issues = gate.parse_output("").unwrap();
        assert!(issues.is_empty());
    }

    #[test]
    fn test_parse_output_with_errors() {
        let json = r#"{"errors": [{"message": "test", "line": 1}]}"#;
        let gate = MyGate::new();
        let issues = gate.parse_output(json).unwrap();

        assert_eq!(issues.len(), 1);
        assert_eq!(issues[0].severity, IssueSeverity::Error);
        assert_eq!(issues[0].line, Some(1));
    }

    #[test]
    fn test_remediation_empty() {
        let gate = MyGate::new();
        let guidance = gate.remediation(&[]);
        assert!(guidance.contains("No issues"));
    }

    #[test]
    fn test_remediation_with_issues() {
        let gate = MyGate::new();
        let issues = vec![
            GateIssue::new(IssueSeverity::Error, "Test error"),
        ];
        let guidance = gate.remediation(&issues);

        assert!(guidance.contains("1"));
        assert!(guidance.contains("error"));
    }
}
```

### Integration Tests

Test against real projects (use temp directories):

```rust
#[cfg(test)]
mod integration_tests {
    use super::*;
    use tempfile::TempDir;
    use std::fs;

    #[test]
    fn test_gate_on_clean_project() {
        let temp = TempDir::new().unwrap();
        fs::write(temp.path().join("clean.rs"), "fn main() {}").unwrap();

        let gate = MyGate::new();
        let issues = gate.run(temp.path()).unwrap();

        assert!(issues.is_empty());
    }

    #[test]
    fn test_gate_detects_issues() {
        let temp = TempDir::new().unwrap();
        fs::write(temp.path().join("bad.rs"), "fn main() { TODO: fix this }").unwrap();

        let gate = MyGate::new();
        let issues = gate.run(temp.path()).unwrap();

        assert!(!issues.is_empty());
    }
}
```

### Testing External Tools

For gates that require external tools:

```rust
#[test]
fn test_required_tool() {
    let gate = ExternalGate::new();
    assert_eq!(gate.required_tool(), Some("external-tool"));
}

#[test]
fn test_run_without_tool() {
    // Skip if tool not installed
    if which::which("external-tool").is_err() {
        eprintln!("Skipping test: external-tool not installed");
        return;
    }

    let temp = TempDir::new().unwrap();
    let gate = ExternalGate::new();
    let result = gate.run(temp.path());

    assert!(result.is_ok());
}
```

### Testing Plugins

Test the plugin entry point:

```rust
#[test]
fn test_plugin_entry_point() {
    let ptr = create_gate_plugin();
    assert!(!ptr.is_null());

    unsafe {
        let plugin = Box::from_raw(ptr);
        assert_eq!(plugin.name(), "MyPlugin");
        assert_eq!(plugin.metadata().version, "1.0.0");
    }
}

#[test]
fn test_plugin_implements_traits() {
    let plugin = MyPlugin::new();

    // Can use as QualityGate
    let gate: &dyn QualityGate = &plugin;
    assert!(gate.is_blocking());

    // Can use as GatePlugin
    let gate_plugin: &dyn GatePlugin = &plugin;
    assert!(gate_plugin.timeout() > Duration::ZERO);
}
```

---

## Complete Examples

### Example 1: Built-in Gate for a Language

Add a new gate for a language in `src/quality/gates/`:

```rust
// src/quality/gates/ruby.rs

//! Ruby-specific quality gate implementations.

use std::path::Path;
use std::process::Command;
use anyhow::{Context, Result};

use super::{GateIssue, IssueSeverity, QualityGate};

/// Returns all standard quality gates for Ruby projects.
#[must_use]
pub fn ruby_gates() -> Vec<Box<dyn QualityGate>> {
    vec![
        Box::new(RubocopGate::new()),
        Box::new(RspecGate::new()),
    ]
}

/// Quality gate that runs RuboCop linter.
pub struct RubocopGate {
    extra_args: Vec<String>,
}

impl RubocopGate {
    #[must_use]
    pub fn new() -> Self {
        Self { extra_args: Vec::new() }
    }
}

impl Default for RubocopGate {
    fn default() -> Self {
        Self::new()
    }
}

impl QualityGate for RubocopGate {
    fn name(&self) -> &str {
        "RuboCop"
    }

    fn run(&self, project_dir: &Path) -> Result<Vec<GateIssue>> {
        let output = Command::new("rubocop")
            .args(["--format", "json"])
            .current_dir(project_dir)
            .output()
            .context("Failed to execute rubocop")?;

        let stdout = String::from_utf8_lossy(&output.stdout);
        self.parse_output(&stdout)
    }

    fn remediation(&self, issues: &[GateIssue]) -> String {
        format!(
            "## RuboCop Issues\n\n\
             Found {} offenses.\n\n\
             Run `rubocop -a` to auto-correct.\n",
            issues.len()
        )
    }

    fn required_tool(&self) -> Option<&str> {
        Some("rubocop")
    }
}

impl RubocopGate {
    fn parse_output(&self, json: &str) -> Result<Vec<GateIssue>> {
        // Parse RuboCop JSON output...
        Ok(vec![])
    }
}
```

### Example 2: External Plugin

See [examples/plugins/rubocop-gate/](../examples/plugins/rubocop-gate/) for a complete example showing:

- Plugin structure (`Cargo.toml`, `lib.rs`, `plugin.toml`)
- JSON output parsing
- Severity mapping
- Remediation guidance
- Comprehensive tests

---

## Contributing a Gate

### For Built-in Gates

1. **Create the gate module** in `src/quality/gates/<language>.rs`

2. **Implement the `QualityGate` trait** following patterns from existing gates

3. **Add factory function** returning `Vec<Box<dyn QualityGate>>`

4. **Register in `gates_for_language()`** in `src/quality/gates/mod.rs`:
   ```rust
   pub fn gates_for_language(lang: Language) -> Vec<Box<dyn QualityGate>> {
       match lang {
           Language::Ruby => ruby::ruby_gates(),
           // ... other languages
       }
   }
   ```

5. **Add tests** with full coverage

6. **Run quality gates**:
   ```bash
   cargo clippy --all-targets -- -D warnings
   cargo test
   ```

7. **Submit a PR** with:
   - Description of the gate and what it checks
   - Link to the external tool documentation
   - Test coverage summary

### For Plugin Gates

1. **Create plugin crate** following [examples/plugins/rubocop-gate/](../examples/plugins/rubocop-gate/)

2. **Implement `QualityGate` and `GatePlugin` traits**

3. **Create `plugin.toml` manifest**

4. **Export entry point function**

5. **Add comprehensive tests**

6. **Build and install**:
   ```bash
   cargo build --release
   mkdir -p ~/.ralph/plugins/my-gate
   cp target/release/libmy_gate.dylib ~/.ralph/plugins/my-gate/
   cp plugin.toml ~/.ralph/plugins/my-gate/
   ```

7. **Test with Ralph**:
   ```bash
   ralph --project . detect  # Should show your plugin
   ```

### Code Style

- Follow existing patterns in `src/quality/gates/`
- Use builder pattern for configuration (`.with_*()` methods)
- Document public APIs with `///` comments
- Include `# Example` and `# Panics` sections in docs
- No `#[allow(...)]` annotations - fix warnings at source
- All public functions must have tests

---

## Reference

### Types

| Type | Module | Description |
|------|--------|-------------|
| `QualityGate` | `ralph::quality::gates` | Core gate trait |
| `GatePlugin` | `ralph::quality::plugin` | Plugin extension trait |
| `GateIssue` | `ralph::quality::gates` | Single issue found |
| `IssueSeverity` | `ralph::quality::gates` | Issue severity level |
| `GateResult` | `ralph::quality::gates` | Gate execution result |
| `PluginMetadata` | `ralph::quality::plugin` | Plugin identification |
| `PluginConfig` | `ralph::quality::plugin` | Plugin configuration |
| `PluginManifest` | `ralph::quality::plugin` | TOML manifest structure |

### Existing Gates

| Gate | Language | Tool | Description |
|------|----------|------|-------------|
| `ClippyGate` | Rust | cargo clippy | Lint checking |
| `TestGate` | Rust | cargo test | Test runner |
| `NoAllowGate` | Rust | (built-in) | Checks for `#[allow]` |
| `SecurityGate` | Rust | narsil-mcp | Security scanning |
| `NoTodoGate` | All | (built-in) | Checks for TODOs |
| `RuffGate` | Python | ruff/flake8 | Lint checking |
| `PytestGate` | Python | pytest | Test runner |
| `MypyGate` | Python | mypy | Type checking |
| `BanditGate` | Python | bandit | Security scanning |
| `EslintGate` | TypeScript | eslint | Lint checking |
| `TypeCheckGate` | TypeScript | tsc | Type checking |
| `JestGate` | TypeScript | jest/vitest | Test runner |
| `GolintGate` | Go | golangci-lint | Lint checking |
| `GoTestGate` | Go | go test | Test runner |
| `GoSecGate` | Go | gosec | Security scanning |

---

## Further Reading

- [Ralph README](../README.md) - Project overview
- [Quick Start: Python](quickstart-python.md) - Python project setup
- [Quick Start: TypeScript](quickstart-typescript.md) - TypeScript project setup
- [Quick Start: Go](quickstart-go.md) - Go project setup
- [Example Plugin](../examples/plugins/rubocop-gate/) - Complete plugin example
