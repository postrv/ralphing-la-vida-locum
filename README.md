# Ralph

> *"Nobody ever dies wishing they'd worked more."*
> — Gareth

## Why "la-vida-locum"?

This project is named in memory of my best friend Gareth, who passed away in a mountaineering accident on Ben Nevis.

Gareth was a doctor who lived by a simple philosophy: work smart, not long. He called it "livin' la vida locum" — taking locum shifts for maximum pay to fund a life of pure adventure. And what a life it was: paragliding, rock climbing, mountaineering, ultra marathons, ice climbing, and completing Ironman triathlons while studying at Oxford.

Ralph carries that spirit forward. It's about working efficiently so you can spend less time at a keyboard and more time out in the world actually living.

---

**Claude Code Automation Suite** - Autonomous coding with bombproof reliability.

Ralph is a Rust CLI tool that orchestrates Claude Code for autonomous software development. It provides project bootstrapping, context building, security validation, and an execution loop that runs Claude Code against an implementation plan until tasks are complete.

## Features

- **Bootstrap** - Initialize a project with Claude Code configuration, hooks, skills, and templates
- **Context Builder** - Generate LLM-optimized context bundles respecting gitignore and token limits
- **Execution Loop** - Run Claude Code autonomously with stagnation detection and phase-based prompts
- **Quality Gate Enforcer** - Pre-commit gates: clippy, tests, no-allow annotations, no-TODO, security scans
- **Checkpoint & Rollback** - Git-based snapshots with quality metrics tracking and automatic rollback on regression
- **Task-Level Tracking** - Fine-grained state machine for individual task progress (NotStarted → InProgress → Complete)
- **Security Hooks** - Pre-validate commands, scan for secrets, enforce allow/deny permissions
- **Dynamic Prompts** - Context-aware prompt assembly with antipattern injection and remediation hints
- **Documentation Archive** - Manage stale documentation without polluting context
- **Analytics** - Track sessions, iterations, and events for analysis

## Installation

```bash
# Clone the repository
git clone https://github.com/postrv/ralphing-la-vida-locum.git
cd ralphing-la-vida-locum

# Build release binary
cargo build --release

# Install to PATH (optional)
cp target/release/ralph ~/.local/bin/
# or
cargo install --path .
```

## Quick Start

### 1. Bootstrap a Project

```bash
ralph --project /path/to/your/project bootstrap
```

This creates:
```
your-project/
├── .claude/
│   ├── CLAUDE.md           # Project memory for Claude Code
│   ├── settings.json       # Permissions and hook configuration
│   ├── mcp.json            # MCP server configuration
│   ├── skills/             # Custom skills (docs-sync, project-analyst)
│   └── agents/             # Subagents (adversarial-reviewer, security-auditor)
├── docs/
│   ├── architecture.md     # Architecture documentation template
│   ├── api.md              # API documentation template
│   └── decisions/          # ADR templates
├── IMPLEMENTATION_PLAN.md  # Task tracking for the loop
└── PROMPT_*.md             # Phase prompts (plan, build, debug)
```

### 2. Create Your Implementation Plan

Edit `IMPLEMENTATION_PLAN.md` with your tasks:

```markdown
# Implementation Plan

## Current Sprint

- [ ] Add user authentication endpoint
- [ ] Create database migrations
- [ ] Write integration tests
```

### 3. Run the Loop

```bash
# Plan phase (5 iterations)
ralph --project . loop --phase plan --max-iterations 5

# Build phase (autonomous execution)
ralph --project . loop --phase build --max-iterations 50

# With verbose output
ralph --verbose --project . loop --phase build --max-iterations 10
```

## Commands

### `bootstrap`
Initialize automation suite in a project directory.

```bash
ralph --project /path/to/project bootstrap
```

### `context`
Build LLM context from project files.

```bash
# Generate context file
ralph --project . context --output context.txt

# Show stats only
ralph --project . context --stats-only

# Limit tokens
ralph --project . context --max-tokens 50000
```

### `loop`
Run the autonomous execution loop.

```bash
ralph --project . loop \
  --phase build \
  --max-iterations 50 \
  --stagnation-threshold 3
```

Options:
- `--phase`: plan, build, or debug
- `--max-iterations`: Maximum loop iterations
- `--stagnation-threshold`: Iterations without progress before escalation
- `--no-commit`: Disable auto-commits

### `hook`
Security hooks for command validation.

```bash
# Validate a command
ralph hook validate "git status"       # OK
ralph hook validate "rm -rf /"         # Blocked

# Run a specific hook
ralph hook run security-filter "npm install"

# Scan file for secrets
ralph hook scan ./config.json
```

### `archive`
Manage documentation archival.

```bash
# Show archive stats
ralph --project . archive stats

# List stale documents (>90 days)
ralph --project . archive list-stale --stale-days 90

# Archive stale docs
ralph --project . archive run --stale-days 90

# Dry run (preview)
ralph --project . archive run --stale-days 90 --dry-run
```

### `analytics`
Track and analyze automation sessions.

```bash
# List sessions
ralph --project . analytics sessions

# Aggregate statistics
ralph --project . analytics aggregate

# Log an event
ralph --project . analytics log \
  --session my-session \
  --event iteration \
  --data '{"phase": "build"}'
```

### `analyze`
Generate project analysis artifacts.

```bash
ralph --project . analyze --output-dir ./analysis
```

### `config`
View and validate configuration.

```bash
# Show configuration paths
ralph --project . config paths

# Validate configuration
ralph --project . config validate

# Show current settings
ralph --project . config show --json
```

## Configuration

### `.claude/settings.json`

```json
{
  "respectGitignore": true,
  "permissions": {
    "allow": [
      "Bash(git *)",
      "Bash(npm *)",
      "Bash(cargo *)"
    ],
    "deny": [
      "Bash(rm -rf *)"
    ]
  },
  "hooks": {
    "PreToolUse": [{
      "matcher": "Bash",
      "hooks": [{
        "type": "command",
        "command": "ralph hook run security-filter"
      }]
    }]
  }
}
```

### Permission Patterns

```
Bash(*)           # Allow all bash commands
Bash(git *)       # Allow git commands only
Bash(npm install) # Allow specific command
```

Priority: deny list > allow list > default allow (if allow list empty)

Hardcoded dangerous patterns are **always** blocked regardless of config:
- `rm -rf /`, `rm -rf ~`, `rm -rf /*`
- `chmod 777`, `chmod -R 777`
- `dd if=/dev/zero`, `mkfs.*`
- Fork bombs, etc.

## Security

Ralph implements multiple layers of security:

1. **Hardcoded Blocks** - Dangerous commands are blocked unconditionally
2. **SSH Blocking** - SSH operations are blocked; use `gh` CLI instead
3. **Project Permissions** - Allow/deny lists in `settings.json`
4. **Secret Detection** - Scans for API keys, passwords, private keys
5. **narsil-mcp Integration** - Run `scan_security` before commits

### Git Authentication

Ralph **requires** the GitHub CLI (`gh`) for all GitHub operations. SSH key access is blocked.

```bash
# Verify authentication
gh auth status

# If not authenticated
gh auth login
```

**Blocked SSH patterns include:**
- `ssh-keygen`, `ssh-add`, `ssh-agent`
- `~/.ssh/` directory access
- `git@github.com:` URLs
- SSH key file access (`id_rsa`, `id_ed25519`, etc.)

**Use these alternatives:**
```
# Instead of:                          Use:
git clone git@github.com:user/repo     gh repo clone user/repo
ssh-keygen                             gh auth login
git remote add origin git@...          gh repo set-default
```

### Secret Patterns Detected

- API keys: `api_key = "..."`
- Passwords: `password = "..."`
- AWS credentials: `aws_access_key_id`
- Private keys: `-----BEGIN RSA PRIVATE KEY-----`

## Two-Tier Analysis

Ralph supports a two-tier analysis approach:

### Project-Level Analysis
For architecture decisions and strategic planning:
```bash
ralph --project . analyze
# Upload ./analysis/context-*.txt to web LLM
```

### Implementation-Level Execution
For file-by-file changes with `ralph loop`:
```bash
ralph --project . loop --phase build --max-iterations 50
```

## Supervisor (Chief Wiggum)

Ralph includes an internal supervisor system ("Chief Wiggum") that monitors loop health and can intervene when problems are detected.

### Verdicts

The supervisor can issue these verdicts:

| Verdict | Action |
|---------|--------|
| **PROCEED** | Continue normal execution |
| **PAUSE** | Request human review |
| **ABORT** | Stop the loop with diagnostics |
| **SWITCH_MODE** | Change to debug mode |
| **RESET** | Reset stagnation counter and retry |

### Health Checks

The supervisor monitors:
- **Test pass rate** - Aborts if < 50%
- **Clippy warnings** - Pauses if > 20
- **Time since last commit** - Switches to debug mode after 15 iterations
- **Repeating errors** - Resets after 2 consecutive repeats
- **Mode oscillation** - Aborts if > 4 mode switches

### Stagnation Levels

Ralph tracks stagnation and escalates automatically:

| Level | Threshold | Action |
|-------|-----------|--------|
| **None** | 0 | Continue normally |
| **Warning** | 1x stagnation | Switch to debug mode |
| **Elevated** | 2x stagnation | Invoke supervisor |
| **Critical** | 3x stagnation | Abort with diagnostic dump |

When stagnation is detected, Ralph:
1. Checks `IMPLEMENTATION_PLAN.md` for blocked tasks
2. Runs `cargo test` to identify failures
3. Runs `cargo clippy` to find warnings
4. Generates a diagnostic report to `.ralph/diagnostics/`

## Quality Gates

Ralph enforces strict quality gates via the `QualityGateEnforcer`:

```rust
let enforcer = QualityGateEnforcer::standard(".");
match enforcer.can_commit() {
    Ok(summary) => println!("Ready to commit: {}", summary),
    Err(failures) => {
        let prompt = generate_remediation_prompt(&failures);
        // Feed prompt back to Claude for fixes
    }
}
```

### Gate Types

| Gate | Checks | Failure Action |
|------|--------|----------------|
| **ClippyGate** | Zero warnings with `-D warnings` | List all warnings with locations |
| **TestGate** | All tests pass | List failing tests with output |
| **NoAllowGate** | No `#[allow(...)]` annotations | List violating files:lines |
| **NoTodoGate** | No TODO/FIXME in new code | List comments to resolve |
| **SecurityGate** | No hardcoded secrets | List detected patterns |

### Checkpoint & Rollback

Ralph creates quality checkpoints before risky operations:

```rust
let manager = CheckpointManager::new(".", config);
let checkpoint = manager.create_checkpoint("Before refactor")?;

// ... Claude makes changes ...

if manager.has_regression(&checkpoint)? {
    let rollback = RollbackManager::new(".")?;
    rollback.rollback_to(&checkpoint)?;
}
```

Checkpoints track:
- Git commit SHA
- Test pass count / fail count
- Clippy warning count
- Files modified since last checkpoint

### Test-Driven Development

Ralph follows TDD methodology:
1. **RED** - Write a failing test that defines expected behavior
2. **GREEN** - Write minimal code to make the test pass
3. **REFACTOR** - Clean up while keeping tests green
4. **CHECKPOINT** - Create quality checkpoint
5. **REVIEW** - Run quality gates + security scans
6. **COMMIT** - Only if ALL gates pass, else rollback

## Architecture

```
┌─────────────────────────────────────────────────────────────────┐
│                         LoopManager                             │
│  ┌─────────────┐  ┌─────────────┐  ┌─────────────────────────┐ │
│  │ TaskTracker │  │ Supervisor  │  │ CheckpointManager       │ │
│  │ (state      │  │ (health     │  │ (snapshots + rollback)  │ │
│  │  machine)   │  │  verdicts)  │  │                         │ │
│  └─────────────┘  └─────────────┘  └─────────────────────────┘ │
└─────────────────────────────────────────────────────────────────┘
         │                 │                     │
         v                 v                     v
┌─────────────────┐ ┌─────────────────┐ ┌─────────────────┐
│ QualityGate     │ │ PromptAssembler │ │ RollbackManager │
│ Enforcer        │ │ (dynamic        │ │ (automatic      │
│ (clippy, tests, │ │  context)       │ │  regression     │
│  security)      │ │                 │ │  prevention)    │
└─────────────────┘ └─────────────────┘ └─────────────────┘
```

## File Structure

```
src/
├── main.rs              # CLI entry point and command routing
├── lib.rs               # Library crate with public API
├── config.rs            # Configuration, SSH patterns, stagnation levels
├── error.rs             # Custom error types (RalphError)
├── context.rs           # Context builder for LLM input
│
├── checkpoint/          # Git-based checkpoint system
│   ├── mod.rs           # Core types: Checkpoint, QualityMetrics
│   ├── manager.rs       # CheckpointManager: create, compare, decide
│   └── rollback.rs      # RollbackManager: automatic regression rollback
│
├── loop/                # Autonomous execution loop
│   ├── mod.rs           # Module exports
│   ├── manager.rs       # LoopManager: orchestrates iterations
│   ├── state.rs         # LoopState, LoopMode state machine
│   ├── progress.rs      # Semantic progress detection
│   ├── retry.rs         # Intelligent retry with failure classification
│   ├── task_tracker.rs  # Task-level state machine (NotStarted→Complete)
│   └── operations.rs    # Real implementations of testable traits
│
├── prompt/              # Dynamic prompt generation
│   ├── mod.rs           # Module exports
│   ├── builder.rs       # PromptBuilder: fluent API
│   ├── assembler.rs     # PromptAssembler: context-aware assembly
│   ├── context.rs       # PromptContext: quality state, history
│   ├── templates.rs     # Phase-specific prompt templates
│   └── antipatterns.rs  # Antipattern detection and injection
│
├── quality/             # Quality gate enforcement
│   ├── mod.rs           # Module exports and Gate trait
│   ├── gates.rs         # ClippyGate, TestGate, SecurityGate, etc.
│   ├── enforcer.rs      # QualityGateEnforcer: pre-commit checks
│   └── remediation.rs   # Remediation prompt generation
│
├── supervisor/          # Chief Wiggum health monitoring
│   ├── mod.rs           # Supervisor: verdicts and health checks
│   └── predictor.rs     # Failure prediction heuristics
│
├── testing/             # Test infrastructure
│   ├── mod.rs           # Module exports
│   ├── traits.rs        # Testable traits (GitOperations, etc.)
│   ├── mocks.rs         # Mock implementations for testing
│   ├── fixtures.rs      # Test fixtures and builders
│   └── assertions.rs    # Custom test assertions
│
├── hooks.rs             # Security hooks and validation
├── archive.rs           # Documentation archival
├── analytics.rs         # Session tracking
├── bootstrap.rs         # Project initialization
└── templates/           # Bootstrap templates
```

## Development

```bash
# Run tests
cargo test

# Run with verbose logging
RUST_LOG=debug cargo run -- --verbose --project . config paths

# Build release
cargo build --release

# Check for warnings
cargo clippy
```

### Test Coverage

- **351+ unit tests** across all modules
- **68 doc tests** for API documentation
- Full coverage of:
  - Quality gates (clippy, tests, security, no-allow, no-todo)
  - Checkpoint creation, comparison, and rollback
  - Task-level state machine transitions
  - Dynamic prompt assembly and antipattern injection
  - Security validation, SSH blocking, stagnation handling
  - Supervisor verdicts and health monitoring
  - Mock-based testing with dependency injection

## Requirements

- Rust 1.70+
- Claude Code 2.1.0+ (for skill hot-reload)
- Optional: [narsil-mcp](https://github.com/postrv/narsil-mcp) for code intelligence

## License

MIT

## Credits

Built with Claude Opus 4.5.
