# Ralph

[![CI](https://github.com/postrv/ralphing-la-vida-locum/actions/workflows/ci.yml/badge.svg)](https://github.com/postrv/ralphing-la-vida-locum/actions/workflows/ci.yml)

> *"Nobody ever dies wishing they'd worked more."*
> — Gareth

## Why "la-vida-locum"?

This project is named in memory of my best friend Gareth, who passed away in a mountaineering accident on Ben Nevis.

Gareth was a doctor who lived by a simple philosophy: work smart, not long. He called it "livin' la vida locum" — taking locum shifts for maximum pay to fund a life of pure adventure. And what a life it was: paragliding, rock climbing, mountaineering, ultra marathons, ice climbing, and completing Ironman triathlons while studying at Oxford.

Ralph carries that spirit forward. It's about working efficiently so you can spend less time at a keyboard and more time out in the world actually living.

---

**Claude Code Automation Suite** - Autonomous coding with bombproof reliability.

Ralph is a Rust CLI tool that orchestrates Claude Code for autonomous software development. It provides project bootstrapping, context building, quality gate enforcement, and an intelligent execution loop that runs Claude Code against an implementation plan until tasks are complete.

## Key Features

### Autonomous Execution
- **Intelligent Loop** - Run Claude Code autonomously with stagnation detection and phase-based prompts
- **Task-Level Tracking** - Fine-grained state machine for individual task progress (NotStarted → InProgress → Complete)
- **Dynamic Prompts** - Context-aware prompt assembly with CCG intelligence, antipattern injection, and remediation hints
- **Supervisor System** - "Chief Wiggum" monitors loop health with automatic intervention

### Quality Enforcement
- **Quality Gates** - Pre-commit gates: clippy, tests, no-allow annotations, no-TODO, security scans
- **Checkpoint & Rollback** - Git-based snapshots with quality metrics and automatic rollback on regression
- **TDD Methodology** - RED → GREEN → REFACTOR → CHECKPOINT → REVIEW → COMMIT

### Code Intelligence
- **narsil-mcp Integration** - Security scanning, call graph analysis, dependency tracking, CCG support
- **CCG (Compact Code Graph)** - Layered code intelligence (L0 manifest, L1 architecture) for prompt enrichment
- **Antipattern Detection** - Detects repeated file editing, missing tests, task oscillation
- **Predictive Prevention** - Pattern-based risk scoring to prevent stagnation

### Multi-Language Support
- **32 Languages** - Rust, Python, TypeScript, Go, Java, C#, C++, Swift, Kotlin, Ruby, and 22 more
- **Auto-Detection** - Detects project languages from file extensions and manifest files (Cargo.toml, package.json, etc.)
- **Polyglot Projects** - Full support for multi-language codebases with confidence scoring
- **Language-Specific Settings** - Generates appropriate `.claude/settings.json` for detected languages

### Developer Experience
- **Bootstrap** - Initialize projects with Claude Code configuration, hooks, skills, and templates
- **Context Builder** - Generate LLM-optimized context bundles respecting gitignore and token limits
- **Security Hooks** - Pre-validate commands, scan for secrets, enforce allow/deny permissions
- **Analytics** - Track sessions, iterations, events, and quality trends (test rates, warnings, security findings)
- **Constraint Verification** - Validate CCG constraints and report compliance issues

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

The bootstrap command auto-detects project languages and generates appropriate configuration:
```bash
# Shows detected languages during bootstrap
ralph --project . bootstrap
#    → Rust (primary)
#    → Python
#    → TypeScript
```

### `detect`
Detect programming languages in a project.

```bash
# Show detected languages
ralph --project . detect

# Output:
#    → Rust (primary)      # 85% confidence
#    → Python              # 72% confidence
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
ralph --project . archive run --stale-days 90 --dry-run
```

### `analytics`
Track and analyze automation sessions.

```bash
# List sessions
ralph --project . analytics sessions

# Aggregate statistics
ralph --project . analytics aggregate
```

### `config`
View and validate configuration.

```bash
# Show configuration paths
ralph --project . config paths

# Validate configuration
ralph --project . config validate
```

## Quality Gates

Ralph enforces strict quality gates before any commit:

| Gate | Checks | Failure Action |
|------|--------|----------------|
| **ClippyGate** | Zero warnings with `-D warnings` | List all warnings with locations |
| **TestGate** | All tests pass | List failing tests with output |
| **NoAllowGate** | No `#[allow(...)]` annotations | List violating files:lines |
| **NoTodoGate** | No TODO/FIXME in new code | List comments to resolve |
| **SecurityGate** | No hardcoded secrets | List detected patterns |

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

## Checkpoint & Rollback

Ralph creates quality checkpoints before risky operations and automatically rolls back on regression:

```rust
let manager = CheckpointManager::new(".")?;
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

## Supervisor System

The internal supervisor ("Chief Wiggum") monitors loop health:

| Verdict | Action |
|---------|--------|
| **PROCEED** | Continue normal execution |
| **PAUSE** | Request human review |
| **ABORT** | Stop the loop with diagnostics |
| **SWITCH_MODE** | Change to debug mode |
| **RESET** | Reset stagnation counter and retry |

### Stagnation Levels

| Level | Threshold | Action |
|-------|-----------|--------|
| **None** | 0 | Continue normally |
| **Warning** | 1x stagnation | Switch to debug mode |
| **Elevated** | 2x stagnation | Invoke supervisor |
| **Critical** | 3x stagnation | Abort with diagnostic dump |

## narsil-mcp Integration

Ralph integrates with [narsil-mcp](https://github.com/postrv/narsil-mcp) for code intelligence. All narsil-mcp features **gracefully degrade** when unavailable—Ralph continues to function normally, returning `None` or empty collections.

```rust
let client = NarsilClient::new(NarsilConfig::default());

// Security scanning
let findings = client.scan_security(".")?;

// Call graph analysis
let graph = client.get_call_graph(".", Some("main"))?;

// Dependency tracking
let deps = client.get_dependencies(".", "src/lib.rs")?;

// Reference finding
let refs = client.find_references(".", "MyStruct")?;

// CCG integration (requires narsil-mcp --features graph)
let manifest = client.get_ccg_manifest()?;      // L0 layer
let arch = client.get_ccg_architecture()?;       // L1 layer
client.export_ccg("./ccg_output")?;              // Export all layers
```

### CCG (Compact Code Graph)

Ralph supports narsil-mcp's CCG format—a layered code intelligence protocol designed for LLM context windows:

| Layer | Name | Size | Contents |
|-------|------|------|----------|
| **L0** | Manifest | ~1-2KB | Repo metadata, language stats, security summary |
| **L1** | Architecture | ~10-50KB | Module hierarchy, public API, entry points |
| **L2** | Symbol Index | Variable | Full symbol graph (N-Quads, gzipped) |

```
┌─────────────────────────────────────────────────────────────────┐
│                    CCG DATA FLOW IN RALPH                        │
├─────────────────────────────────────────────────────────────────┤
│   narsil-mcp CLI                                                 │
│   ┌────────────────────────────────┐                            │
│   │ get_ccg_manifest               │ → L0 JSON (~1-2KB)         │
│   │ export_ccg_architecture        │ → L1 JSON (~10-50KB)       │
│   │ export_ccg                     │ → All layers bundled       │
│   └──────────────┬─────────────────┘                            │
│                  ▼                                               │
│   NarsilClient → CodeIntelligenceContext → DynamicPromptBuilder  │
│                                                                  │
│   Prompt output includes:                                        │
│   • Project name and primary language                            │
│   • Symbol/file counts                                           │
│   • Security summary with severity icons                         │
│   • Entry points (limited to 5)                                  │
│   • Public API symbols (limited to 8)                            │
│   • Module structure (limited to 5)                              │
└─────────────────────────────────────────────────────────────────┘
```

**What CCG Enables:**
- **Architecture-Aware Prompts** - Claude sees module structure, public API, entry points
- **Security-Aware Development** - Security summary injected into every prompt
- **Size-Constrained Intelligence** - CCG data fits in prompt context (<1KB)

### Graceful Degradation

All narsil-mcp integration code works when narsil-mcp is unavailable:

```rust
// Check availability before expensive operations
if client.is_available() {
    let manifest = client.get_ccg_manifest()?;
    // ... use CCG data
} else {
    // Continue without CCG - Ralph still functions
}

// Or rely on Option returns
match client.get_ccg_manifest()? {
    Some(manifest) => enrich_prompt_with_ccg(&manifest),
    None => use_basic_prompt(),
}
```

## Security

Ralph implements multiple layers of security:

1. **Hardcoded Blocks** - Dangerous commands blocked unconditionally (`rm -rf /`, `chmod 777`, etc.)
2. **SSH Blocking** - SSH operations blocked; use `gh` CLI instead
3. **Project Permissions** - Allow/deny lists in `settings.json`
4. **Secret Detection** - Scans for API keys, passwords, private keys
5. **narsil-mcp Integration** - Security scanning before commits

### Git Authentication

Ralph **requires** the GitHub CLI (`gh`) for all GitHub operations. SSH key access is blocked.

```bash
# Verify authentication
gh auth status

# If not authenticated
gh auth login
```

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
│ QualityGate     │ │ PromptAssembler │ │ NarsilClient    │
│ Enforcer        │ │ (dynamic        │ │ (code           │
│ (clippy, tests, │ │  context)       │ │  intelligence)  │
│  security)      │ │                 │ │                 │
└─────────────────┘ └─────────────────┘ └─────────────────┘
```

## File Structure

```
src/
├── main.rs              # CLI entry point and command routing
├── lib.rs               # Library crate with public API
├── config.rs            # Configuration, SSH patterns, stagnation levels
├── error.rs             # Custom error types (RalphError)
│
├── checkpoint/          # Git-based checkpoint system
│   ├── mod.rs           # Core types: Checkpoint, QualityMetrics
│   ├── manager.rs       # CheckpointManager: create, compare, decide
│   └── rollback.rs      # RollbackManager: automatic regression rollback
│
├── loop/                # Autonomous execution loop
│   ├── manager.rs       # LoopManager: orchestrates iterations
│   ├── state.rs         # LoopState, LoopMode state machine
│   ├── progress.rs      # Semantic progress detection
│   ├── retry.rs         # Intelligent retry with failure classification
│   ├── task_tracker.rs  # Task-level state machine (NotStarted→Complete)
│   └── operations.rs    # Real implementations of testable traits
│
├── prompt/              # Dynamic prompt generation
│   ├── builder.rs       # PromptBuilder: fluent API
│   ├── assembler.rs     # PromptAssembler: context-aware assembly
│   ├── context.rs       # PromptContext: quality state, history
│   ├── templates.rs     # Phase-specific prompt templates
│   └── antipatterns.rs  # Antipattern detection and injection
│
├── quality/             # Quality gate enforcement
│   ├── gates.rs         # ClippyGate, TestGate, SecurityGate, etc.
│   ├── enforcer.rs      # QualityGateEnforcer: pre-commit checks
│   └── remediation.rs   # Remediation prompt generation
│
├── narsil/              # narsil-mcp integration
│   ├── client.rs        # MCP client for tool invocation
│   ├── ccg.rs           # CCG data structures (CcgManifest, CcgArchitecture)
│   ├── constraint_verifier.rs  # CCG constraint validation
│   └── intelligence.rs  # Code intelligence queries
│
├── supervisor/          # Chief Wiggum health monitoring
│   ├── mod.rs           # Supervisor: verdicts and health checks
│   └── predictor.rs     # Failure prediction heuristics
│
├── testing/             # Test infrastructure
│   ├── traits.rs        # Testable traits (GitOperations, etc.)
│   ├── mocks.rs         # Mock implementations for testing
│   ├── fixtures.rs      # Test fixtures and builders
│   └── assertions.rs    # Custom test assertions
│
├── bootstrap/           # Project bootstrapping
│   ├── mod.rs           # Bootstrap orchestration
│   ├── language.rs      # Language enum (32 languages)
│   └── language_detector.rs  # Auto-detection with confidence scoring
│
├── hooks.rs             # Security hooks and validation
├── archive.rs           # Documentation archival
├── analytics.rs         # Session tracking
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
cargo clippy --all-targets -- -D warnings
```

## Open Core Model

Ralph follows an open core licensing model:

| License | Repository | Purpose |
|---------|------------|---------|
| **MIT** | `ralph` (this repo) | CLI, quality gates, narsil-mcp integration |
| **Proprietary** | [`ralph-cloud`](https://github.com/postrv/ralph-cloud) | CCIaaS, enterprise features |

See [docs/LICENSING.md](docs/LICENSING.md) for details.

## Requirements

- Rust 1.85+ (MSRV enforced by CI)
- Claude Code 2.1.0+ (for skill hot-reload)
- GitHub CLI (`gh`) for authentication
- Optional: [narsil-mcp](https://github.com/postrv/narsil-mcp) for code intelligence (use `--features graph` for CCG support)

## License

MIT

## Credits

Built with Claude Opus 4.5.
