# Project Memory - Automation Suite Enabled

## Environment
- Claude Code 2.1.0+ with skill hot-reload
- Opus 4.5 exclusively (Max x20)
- narsil-mcp for code intelligence and security
- Ralph automation suite for autonomous execution

## Current Work: Multi-Language Bootstrap

Ralph is being enhanced to support all 32 languages in narsil-mcp. Key areas:

- **Language Detection**: `src/bootstrap/language.rs`, `src/bootstrap/language_detector.rs`
- **Quality Gates**: `src/quality/gates/` (trait-based, per-language)
- **Templates**: `src/templates/{rust,python,typescript,go,java,...}/`
- **Reference Design**: `further_dev_plans/ralph-multilang-bootstrap.md`

When implementing, follow the existing patterns in Rust code and adapt for new languages.

---

## GIT AUTHENTICATION (CRITICAL)

### Required Setup
Ralph requires `gh` CLI for all GitHub operations. SSH key access is **blocked**.

```bash
# Verify gh CLI is authenticated
gh auth status

# If not authenticated, run:
gh auth login
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

### Test-Driven Development (Mandatory)

**Every change follows this cycle:**
1. **RED**: Write a failing test that defines expected behavior
2. **GREEN**: Write minimal code to make the test pass
3. **REFACTOR**: Clean up while keeping tests green
4. **REVIEW**: Run clippy + security scans
5. **COMMIT**: Only if ALL quality gates pass

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
- `dead_code` → Delete the code or add tests that use it
- `unused_*` → Remove the unused item
- `similar_names` → Rename for clarity
- `cast_*` → Use safe conversion patterns

---

## TWO-TIER ANALYSIS

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

**CCG (Code Context Graph) - Requires `--features graph`:**
```bash
get_ccg_manifest            # L0: Manifest (~1-2KB JSON-LD)
export_ccg_architecture     # L1: Architecture (~10-50KB JSON-LD)
export_ccg_index            # L2: Symbol Index (N-Quads gzipped)
export_ccg                  # Export all layers to directory
query_ccg <sparql>          # Query CCG via SPARQL
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
- CCG queries return None
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

## DOCUMENTATION RULES

- New public API → update docs/api.md
- Architecture change → update docs/architecture.md
- Major decision → create docs/decisions/XXX-*.md
- Stale docs (90+ days) → archive to .archive/docs/
- NEVER delete docs, always archive

---

## SECURITY REQUIREMENTS

- All CRITICAL/HIGH findings must be resolved before commit
- No hardcoded secrets (use environment variables)
- All user input must be validated
- No `unsafe` blocks without explicit justification
- Run narsil-mcp security scans before every commit

---

## QUALITY GATES (Pre-Commit Checklist)

**Mandatory (always enforced):**
```
[ ] cargo clippy --all-targets -- -D warnings  → 0 warnings
[ ] cargo test                                  → all pass
[ ] No #[allow(...)] annotations added
[ ] No TODO/FIXME comments in new code
[ ] All new public APIs documented
[ ] All new types have tests
[ ] gh auth status                              → authenticated
```

**Optional (if narsil-mcp available):**
```
[ ] scan_security                              → 0 CRITICAL/HIGH
[ ] find_injection_vulnerabilities             → 0 findings
```

---

## SUBAGENT USAGE

- **docs-sync**: After significant code changes
- **project-analyst**: Before major features
- **security-auditor**: For security-sensitive changes
- **adversarial-reviewer**: Before marking complex tasks complete
- **supervisor**: Invoked automatically at elevated stagnation

---

## ARCHIVE POLICY

Files in `.archive/` are excluded from all context.
Retrieve with: `grep -r "keyword" .archive/`

---

## CONTEXT BUILDING

Generate full project context: `ralph context -o context.txt`

---

## TEST BASELINE TRACKING

Track test counts in IMPLEMENTATION_PLAN.md. Test count must never decrease.
Warning count must stay at 0. Any commit that violates this is BLOCKED.

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
ralph loop build --max-iterations 50

# Debug stagnation
ralph loop debug --max-iterations 10

# Check configuration
ralph config validate

# View analytics
ralph analytics sessions --last 5

# Build context
ralph context -o context.txt

# Verify git environment
gh auth status
```
