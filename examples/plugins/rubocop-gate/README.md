# RuboCop Quality Gate Plugin

A Ralph quality gate plugin that runs [RuboCop](https://rubocop.org/) on Ruby projects.

This example plugin demonstrates how to create external quality gates for Ralph. Use it as a template for developing your own plugins.

## Features

- Runs RuboCop with JSON output format
- Parses offenses into structured `GateIssue` results
- Maps RuboCop severities to Ralph's issue severity levels
- Provides remediation guidance with auto-fix instructions
- Includes specific guidance for common RuboCop cops

## Prerequisites

- Rust toolchain (for building the plugin)
- RuboCop installed and available in PATH

```bash
gem install rubocop
```

## Building

```bash
# From this directory
cargo build --release

# The compiled plugin will be at:
#   target/release/librubocop_gate.dylib (macOS)
#   target/release/librubocop_gate.so (Linux)
#   target/release/rubocop_gate.dll (Windows)
```

## Installation

Copy the compiled library and manifest to your plugins directory:

```bash
# User-wide installation
mkdir -p ~/.ralph/plugins/rubocop-gate
cp target/release/librubocop_gate.dylib ~/.ralph/plugins/rubocop-gate/
cp plugin.toml ~/.ralph/plugins/rubocop-gate/

# Or project-specific installation
mkdir -p .ralph/plugins/rubocop-gate
cp target/release/librubocop_gate.dylib .ralph/plugins/rubocop-gate/
cp plugin.toml .ralph/plugins/rubocop-gate/
```

## Verifying Installation

```bash
ralph plugins list
```

You should see:
```
Installed Plugins:
========================================
rubocop-gate v1.0.0
by Ralph Community
Ruby linting via RuboCop - checks Ruby code against community style guide
License: MIT
```

## Usage

Once installed, Ralph will automatically use the RuboCop gate when running quality checks on Ruby projects:

```bash
ralph detect  # Shows available gates including rubocop-gate
ralph loop    # Runs quality gates including RuboCop
```

## Configuration

Edit `plugin.toml` to customize:

```toml
[config]
# Timeout for RuboCop execution
timeout = "2m"

# Enable/disable the plugin
enabled = true
```

---

## Plugin Development Guide

Use this plugin as a template for creating your own quality gates.

### Step 1: Create Your Plugin Crate

```bash
cargo new --lib my-gate
cd my-gate
```

Add to `Cargo.toml`:
```toml
[lib]
crate-type = ["cdylib", "rlib"]

[dependencies]
ralph = "2.0"  # Or path dependency for development
anyhow = "1.0"
serde = { version = "1.0", features = ["derive"] }
serde_json = "1.0"
```

### Step 2: Implement the Traits

Your plugin must implement two traits:

1. **`QualityGate`** - Core check logic:
   - `name()` - Display name for the gate
   - `run(&self, project_dir: &Path) -> Result<Vec<GateIssue>>` - Run the check
   - `remediation(&self, issues: &[GateIssue]) -> String` - Generate fix guidance
   - `required_tool()` - Optional: specify required external tool

2. **`GatePlugin`** - Plugin metadata:
   - `metadata()` - Return `PluginMetadata` (name, version, author, etc.)
   - `timeout()` - Maximum execution time
   - `on_load()` - Optional: initialization hook
   - `configure(&mut self, config: &PluginConfig)` - Optional: apply config

### Step 3: Export the Entry Point

```rust
#[no_mangle]
pub extern "C" fn create_gate_plugin() -> *mut dyn GatePlugin {
    let plugin = MyGatePlugin::new();
    Box::into_raw(Box::new(plugin))
}
```

### Step 4: Create the Manifest

Create `plugin.toml`:
```toml
[plugin]
name = "my-gate"
version = "1.0.0"
author = "Your Name"
description = "Description of what your gate checks"

[library]
path = "target/release/libmy_gate.dylib"
entry_point = "create_gate_plugin"

[config]
timeout = "60s"
enabled = true
```

### Step 5: Build and Install

```bash
cargo build --release
mkdir -p ~/.ralph/plugins/my-gate
cp target/release/libmy_gate.dylib ~/.ralph/plugins/my-gate/
cp plugin.toml ~/.ralph/plugins/my-gate/
```

### Example: Minimal Plugin

```rust
use ralph::quality::gates::{GateIssue, IssueSeverity, QualityGate};
use ralph::quality::plugin::{GatePlugin, PluginMetadata};
use std::path::Path;
use std::time::Duration;
use anyhow::Result;

pub struct MyGate;

impl QualityGate for MyGate {
    fn name(&self) -> &str { "MyGate" }

    fn run(&self, project_dir: &Path) -> Result<Vec<GateIssue>> {
        // Run your check here
        // Return Vec::new() for pass, or issues for failure
        Ok(vec![])
    }

    fn remediation(&self, issues: &[GateIssue]) -> String {
        format!("Fix {} issues", issues.len())
    }
}

impl GatePlugin for MyGate {
    fn metadata(&self) -> PluginMetadata {
        PluginMetadata::new("my-gate", "1.0.0", "Author")
    }

    fn timeout(&self) -> Duration {
        Duration::from_secs(60)
    }
}

#[no_mangle]
pub extern "C" fn create_gate_plugin() -> *mut dyn GatePlugin {
    Box::into_raw(Box::new(MyGate))
}
```

## Testing

Run the plugin's test suite:

```bash
cargo test
```

## License

MIT License - see [LICENSE](LICENSE) for details.
