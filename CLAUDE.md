# Ralph - Project Guidelines

## Project Overview

Ralph is a Rust-based Claude Code automation suite that enables autonomous execution with TDD, quality gates, and intelligent retry mechanisms.

**Language:** Rust
**Build System:** Cargo

---

## BUILD & TEST COMMANDS

```bash
# Check compilation
cargo check

# Run clippy (warnings as errors)
cargo clippy --all-targets -- -D warnings

# Run all tests
cargo test

# Run only library tests
cargo test --lib

# Build release
cargo build --release
```

---

## CODE QUALITY STANDARDS

### Test-Driven Development

Tests first, implementation second.

### Quality Gates (Before Commit)

```
[ ] cargo clippy --all-targets -- -D warnings  -> 0 warnings
[ ] cargo test                                  -> all pass
```

### Forbidden Patterns

```rust
#[allow(dead_code)]           // Wire in or delete
#[allow(unused_*)]            // Use or remove
#[allow(clippy::*)]           // Fix the issue
todo!()                       // Implement now
```

---

## PROJECT STRUCTURE

```
src/
├── bootstrap/           # Project bootstrapping & language detection
├── quality/             # Quality gate system
├── narsil/              # narsil-mcp integration (optional)
├── checkpoint/          # Checkpoint/rollback system
├── prompt/              # Dynamic prompt generation
│   └── builder/         # Section builders (split module)
├── analytics/           # Session analytics
├── loop/                # Automation loop
│   ├── manager/         # Loop manager (split module)
│   ├── task_tracker/    # Task state machine (split module)
│   ├── retry.rs         # Intelligent retry
│   └── progress.rs      # Progress tracking
└── lib.rs               # Public API
```
