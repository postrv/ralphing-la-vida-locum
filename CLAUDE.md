# Project Memory - Automation Suite Enabled

## Environment
- Claude Code 2.1.0+ with skill hot-reload
- Opus 4.5 exclusively (Max x20)
- narsil-mcp for code intelligence and security
- Ralph automation suite for autonomous execution

## Current Work: Enterprise Features Foundation

Sprint 17 focuses on enterprise features: configuration inheritance, shared gate configs, and audit logging.

- **Implementation Plan**: `IMPLEMENTATION_PLAN.md`
- **Completed Work**: `docs/COMPLETED_SPRINTS.md`
- **Quality Gates**: `src/quality/gates/`
- **Analytics**: `src/analytics.rs`

---

## GIT AUTHENTICATION (CRITICAL)

### Required Setup
Ralph requires `gh` CLI for all GitHub operations. SSH key access is **blocked**.

```bash
# Verify gh CLI is authenticated
gh auth status

# If not authenticated, run:
gh auth login

# For workflow scope (needed for .github/workflows/):
gh auth refresh -s workflow
```

### Rules - Non-Negotiable
1. **ALWAYS** use `gh` CLI for GitHub operations
2. **NEVER** attempt SSH key operations (ssh-keygen, ssh-add, etc.)
3. **NEVER** access ~/.ssh/ directory
4. **NEVER** use git@github.com: URLs
5. Use `gh auth status` to verify authentication
6. Use `gh repo clone` instead of `git clone git@github.com:`

### Command Mappings
```
# Instead of:                          # Use:
git clone git@github.com:user/repo     gh repo clone user/repo
ssh-keygen                             (not needed - gh handles auth)
cat ~/.ssh/id_rsa                      (blocked - use gh auth)
git remote add origin git@...          gh repo set-default
```

### If Authentication Issues
```bash
# Re-authenticate
gh auth login

# Add workflow scope for CI files
gh auth refresh -s workflow

# Check status
gh auth status

# Test with
gh api user
```

---

## PRODUCTION STANDARDS (Non-Negotiable)

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

### Test-Driven Development (MANDATORY - NO EXCEPTIONS)

**Tests FIRST. Implementation SECOND. Always.**

**Every change follows this cycle:**
1. **REINDEX**: `reindex` - refresh narsil-mcp index before starting
2. **RED**: Write a failing test that defines expected behavior
3. **GREEN**: Write minimal code to make the test pass
4. **REFACTOR**: Clean up while keeping tests green
5. **REVIEW**: Run clippy + security scans
6. **COMMIT**: Only if ALL quality gates pass
7. **REINDEX**: `reindex` - refresh narsil-mcp index after completing

**If you write implementation before tests, STOP:**
1. Delete the implementation
2. Write the test first
3. Then write minimal code to pass

**Test Coverage Requirements:**
- Every public function: at least 1 unit test
- Every public type: exercised in integration tests
- Every error path: tested
- Every edge case: empty inputs, boundaries, overflow

### Clippy Configuration
Run with warnings as errors:
```bash
cargo clippy --all-targets -- -D warnings
```

If clippy warns about:
- `dead_code` -> Delete the code or add tests that use it
- `unused_*` -> Remove the unused item
- `similar_names` -> Rename for clarity
- `cast_*` -> Use safe conversion patterns

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

## PROJECT STRUCTURE

```
src/
├── bootstrap/           # Project bootstrapping & language detection
├── quality/             # Quality gate system
├── narsil/              # narsil-mcp integration (optional)
├── checkpoint/          # Checkpoint/rollback system
├── prompt/              # Dynamic prompt generation
│   └── builder/         # Section builders (split module)
├── analytics.rs         # Session analytics, events, trends
├── llm/                 # Model abstraction layer
├── plugin/              # Plugin architecture
├── loop/                # Automation loop
│   ├── manager/         # Loop manager (split module)
│   ├── task_tracker/    # Task state machine (split module)
│   ├── retry.rs         # Intelligent retry
│   └── progress.rs      # Progress tracking
└── lib.rs               # Public API
```

---

## MCP SERVERS

### narsil-mcp (Code Intelligence - Optional)

narsil-mcp is optional. All Ralph features gracefully degrade when unavailable.

**Security (run before committing, if available):**
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
get_function_hotspots       # High-impact functions
```

**Refactoring:**
```bash
get_import_graph
find_circular_imports
find_dead_code
```

### Graceful Degradation Policy

When narsil-mcp is unavailable:
- Security gates are skipped (log warning)
- Code intelligence returns empty results
- Ralph continues to function normally

---

## STAGNATION HANDLING

Ralph monitors for stagnation and will escalate:

| Level | Threshold | Action |
|-------|-----------|--------|
| Warning | 1x | Switch to debug mode |
| Elevated | 2x | Invoke supervisor |
| Critical | 3x | Abort with diagnostics |

**If in stagnation:**
1. Check `IMPLEMENTATION_PLAN.md` for blocked tasks
2. Run `cargo test` to identify failing tests
3. Run `cargo clippy` to find warnings
4. Check git status for uncommitted changes
5. Use narsil-mcp to understand the codebase

---

## QUALITY GATES (Pre-Commit Checklist)

**Start of Task:**
```
[ ] reindex                                    -> narsil-mcp index refreshed
```

**Mandatory (always enforced):**
```
[ ] Tests written BEFORE implementation        -> TDD verified
[ ] cargo clippy --all-targets -- -D warnings  -> 0 warnings
[ ] cargo test                                  -> all pass
[ ] No #[allow(...)] annotations added
[ ] No TODO/FIXME comments in new code
[ ] All new public APIs documented
[ ] All new types have tests
[ ] gh auth status                              -> authenticated
```

**Optional (if narsil-mcp available):**
```
[ ] scan_security                              -> 0 CRITICAL/HIGH
[ ] find_injection_vulnerabilities             -> 0 findings
```

**End of Task:**
```
[ ] reindex                                    -> narsil-mcp index updated
```

---

## SUPERVISOR (Chief Wiggum)

The supervisor agent monitors loop health and can intervene:

- **PROCEED**: Continue normal execution
- **PAUSE**: Request human review
- **ABORT**: Stop the loop with diagnostics
- **SWITCH_MODE**: Change to debug mode
- **RESET**: Reset stagnation and retry

The supervisor checks:
- Test pass rate (abort if < 50%)
- Clippy warnings (pause if > 20)
- Time since last commit (switch mode if > 15 iterations)
- Repeating errors (reset after 2 repeats)
- Mode oscillation (abort if > 4 switches)

---

## QUICK REFERENCE

```bash
# Start automation
ralph --project . loop --max-iterations 50

# Debug stagnation
ralph --project . loop --phase debug --max-iterations 10

# Check configuration
ralph config validate

# View analytics
ralph analytics sessions --last 5

# Verify git environment
gh auth status
```
