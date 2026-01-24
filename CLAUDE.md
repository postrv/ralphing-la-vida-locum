# Ralph - Claude Code Automation Suite

## Overview

Ralph is an automation suite for Claude Code that enables autonomous code improvement loops with quality gates, stagnation detection, and intelligent task management.

## Key Components

- **Implementation Plan**: `IMPLEMENTATION_PLAN.md` - Define sprints and phases for Ralph to execute
- **Quality Gates**: `src/quality/gates/` - Automated checks (clippy, tests, security, etc.)
- **Analytics**: `src/analytics/` - Session tracking and performance metrics
- **Loop Manager**: `src/loop/` - Automation loop with progress detection

---

## Quick Start

```bash
# Run automation loop
ralph --project . loop --max-iterations 20

# Start fresh (ignore existing session)
ralph --project . loop --fresh

# Run in debug mode
ralph --project . loop --phase debug

# View analytics
ralph analytics sessions --last 5

# Generate dashboard
ralph analytics dashboard --open
```

---

## Configuration

Ralph uses `ralph.toml` for configuration:

```toml
[quality]
gates = ["clippy", "tests", "no_allow", "no_todo"]

[loop]
max_iterations = 50
stagnation_threshold = 5

[supervisor]
enabled = true
```

---

## Quality Gates

Before ANY commit:

```bash
# Must all pass
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo test
cargo doc --no-deps
```

---

## Development Standards

### Code Quality - Zero Tolerance

**Forbidden Patterns:**
```rust
#[allow(dead_code)]           // Wire in or delete
#[allow(unused_*)]            // Use or remove
#[allow(clippy::*)]           // Fix the issue
// TODO: ...                  // Implement now or don't merge
// FIXME: ...                 // Fix now
unimplemented!()              // Implement or remove
todo!()                       // Implement now
```

**Required Patterns:**
```rust
#[must_use]                   // On functions returning values
/// # Panics                  // Document panic conditions
/// # Errors                  // Document error conditions
```

### Test-Driven Development

**Every change follows this cycle:**
1. **RED**: Write a failing test that defines expected behavior
2. **GREEN**: Write minimal code to make the test pass
3. **REFACTOR**: Clean up while keeping tests green

---

## Session Persistence

Ralph automatically saves session state to `.ralph/session.json` for crash recovery.

- Sessions auto-resume on restart (use `--fresh` to start clean)
- State is saved after each iteration and on graceful shutdown
- Corrupted sessions are automatically deleted

---

## Stagnation Detection

Ralph monitors for stagnation and will escalate:

| Level | Threshold | Action |
|-------|-----------|--------|
| Warning | 1x | Switch to debug mode |
| Elevated | 2x | Invoke supervisor |
| Critical | 3x | Abort with diagnostics |

---

## Predictor & Adaptive Weights

Ralph includes a stagnation predictor that can learn from prediction accuracy:

```bash
# View predictor statistics
ralph predictor stats

# Tune weights based on prediction history
ralph predictor tune

# Dry run (preview changes)
ralph predictor tune --dry-run
```

---

## Project Structure

```
src/
├── analytics/           # Session analytics and dashboard
├── bootstrap/           # Project detection and setup
├── checkpoint/          # Checkpoint/rollback system
├── changes/             # Incremental execution support
├── llm/                 # LLM provider abstraction
├── loop/                # Automation loop
│   ├── manager/         # Loop orchestration
│   └── task_tracker/    # Task state machine
├── plugin/              # Plugin architecture
├── prompt/              # Dynamic prompt generation
├── quality/             # Quality gate system
│   └── gates/           # Individual gate implementations
├── session/             # Session persistence
├── supervisor/          # Health monitoring
│   └── predictor.rs     # Stagnation prediction
└── lib.rs               # Public API
```
