# Ralph - Project Memory

## Project Overview

Ralph is a Rust-based Claude Code automation suite that enables autonomous, bombproof execution with TDD, quality gates, and intelligent retry mechanisms.

**Repository:** `ralphing-la-vida-locum` (open source core)
**Language:** Rust
**Build System:** Cargo

---

## Current Work: Sprint 7 - Language-Specific Quality Gates

**Goal:** Extend Ralph's quality gate system to support multiple programming languages with their native tooling.

### Key Files
- **Quality Gates:** `src/quality/gates.rs` (current Rust-only implementation)
- **Language Detection:** `src/bootstrap/language.rs`, `src/bootstrap/language_detector.rs` (Sprint 6, complete)
- **Design Document:** `further_dev_plans/ralph-multilang-bootstrap.md`
- **Implementation Plan:** `IMPLEMENTATION_PLAN.md`

### Sprint 7 Tasks
1. **7a. QualityGate Trait Refactor** - Create trait with `gates_for_language()` factory
2. **7b. Python Gates** - RuffGate, PytestGate, MypyGate, BanditGate
3. **7c. TypeScript/JS Gates** - EslintGate, JestGate, TscGate, NpmAuditGate
4. **7d. Go Gates** - GoVetGate, GolangciLintGate, GoTestGate, GovulncheckGate
5. **7e. Gate Auto-Detection** - Detect available tools and combine gates for polyglot projects

---

## GIT AUTHENTICATION (CRITICAL)

### Required Setup
Ralph requires `gh` CLI for all GitHub operations. SSH key access is **blocked**.

```bash
# Verify gh CLI is authenticated
gh auth status

# If not authenticated:
gh auth login
```

### Rules - Non-Negotiable
1. **ALWAYS** use `gh` CLI for GitHub operations
2. **NEVER** attempt SSH key operations (ssh-keygen, ssh-add, etc.)
3. **NEVER** access ~/.ssh/ directory
4. **NEVER** use git@github.com: URLs
5. Use `gh repo clone` instead of `git clone git@github.com:`

---

## PRODUCTION STANDARDS (Non-Negotiable)

### Test-Driven Development (MANDATORY - NO EXCEPTIONS)

**Tests FIRST. Implementation SECOND. Always.**

**Every task follows this cycle:**
1. **REINDEX**: Refresh narsil-mcp index before starting
2. **RED**: Write a failing test that defines expected behavior
3. **GREEN**: Write minimal code to make the test pass
4. **REFACTOR**: Clean up while keeping tests green
5. **REVIEW**: Run clippy + security scans
6. **COMMIT**: Only if ALL quality gates pass
7. **REINDEX**: Refresh narsil-mcp index after completing

**If you write implementation before tests, STOP:**
1. Delete the implementation
2. Write the test first
3. Then write minimal code to pass

### Code Quality - Zero Tolerance Policy

**FORBIDDEN PATTERNS - Never Use:**
```rust
#[allow(dead_code)]           // Wire in or delete
#[allow(unused_*)]            // Use or remove
#[allow(clippy::*)]           // Fix the issue
// TODO: ...                  // Implement now or don't merge
// FIXME: ...                 // Fix now
unimplemented!()              // Implement or remove
todo!()                       // Implement now
```

**REQUIRED PATTERNS - Always Use:**
```rust
#[must_use]                   // On functions returning values
/// # Panics                  // Document panic conditions
/// # Errors                  // Document error conditions
/// # Examples                // Provide usage examples
#[cfg(test)]                  // Keep tests in modules
```

### Quality Gates (Must Pass Before Commit)

**Start of Task:**
```
[ ] reindex                                    -> narsil-mcp index refreshed
```

**Before Commit:**
```
[ ] Tests written BEFORE implementation        -> TDD verified
[ ] cargo clippy --all-targets -- -D warnings  -> 0 warnings
[ ] cargo test                                  -> all pass
[ ] No #[allow(...)] annotations added
[ ] No TODO/FIXME comments in new code
[ ] All new public APIs documented
[ ] All new types have tests
```

**If narsil-mcp available:**
```
[ ] scan_security                              -> 0 CRITICAL/HIGH
[ ] find_injection_vulnerabilities             -> 0 findings
```

**End of Task:**
```
[ ] reindex                                    -> narsil-mcp index updated
```

---

## MCP SERVERS

### narsil-mcp (Code Intelligence - Optional)

narsil-mcp provides code intelligence but is optional. All features gracefully degrade when unavailable.

**Security (run before committing):**
```bash
scan_security           # Find vulnerabilities
find_injection_vulnerabilities  # SQL/XSS/command injection
check_cwe_top25        # CWE Top 25 checks
```

**Context Gathering:**
```bash
get_call_graph <function>   # Function relationships
find_references <symbol>    # Impact analysis
get_dependencies <path>     # Module dependencies
find_similar_code <query>   # Find related code
```

### Graceful Degradation Policy

When narsil-mcp is unavailable:
- Security gates are skipped (log warning)
- Code intelligence returns empty results
- Ralph continues to function normally

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

# Run only binary tests
cargo test --bin ralph

# Build release
cargo build --release
```

---

## PROJECT STRUCTURE

```
src/
├── bootstrap/           # Project bootstrapping
│   ├── language.rs      # Language enum (32 languages)
│   ├── language_detector.rs  # Auto-detection
│   └── mod.rs
├── quality/             # Quality gate system
│   ├── gates.rs         # Gate implementations
│   ├── enforcer.rs      # Gate orchestration
│   ├── remediation.rs   # Fix suggestions
│   └── mod.rs
├── narsil/              # narsil-mcp integration
├── checkpoint/          # Checkpoint/rollback system
├── prompt/              # Dynamic prompt generation
├── analytics/           # Session analytics
├── loop/                # Automation loop
│   ├── task_tracker.rs  # Task state machine
│   ├── retry.rs         # Intelligent retry
│   └── ...
└── lib.rs               # Public API
```

---

## KEY DESIGN DECISIONS

1. **Trait-based Gates**: Quality gates implement a common trait for extensibility
2. **Graceful Degradation**: All narsil-mcp features work when MCP unavailable
3. **Language Detection**: File extensions + manifest files with confidence scoring
4. **Sprint-Aware Tasks**: Tasks belong to sprints, preventing orphaned work

---

## QUICK REFERENCE

```bash
# Start work on a task
reindex                              # Refresh code intelligence

# Development cycle
cargo test                           # Run tests
cargo clippy --all-targets -- -D warnings  # Lint

# After completing task
reindex                              # Update code intelligence

# Verify git environment
gh auth status
```

---

## NOTES FOR AUTONOMOUS EXECUTION

- Ralph reads IMPLEMENTATION_PLAN.md each iteration to select the next task
- Tasks are prioritized top-to-bottom within each sprint
- Complete current sprint before moving to next
- Reference `further_dev_plans/ralph-multilang-bootstrap.md` for Sprint 7+ design details
- Checkpoint after successful commits to enable rollback on regression
