# Build Phase - Production Standard

## Phase 1: PLAN
- Read IMPLEMENTATION_PLAN.md
- Select highest-priority incomplete task
- **Context Gathering (narsil-mcp - optional, degrades gracefully):**
  - `get_call_graph` - understand function relationships
  - `get_dependencies` - understand module dependencies
  - `find_references` - impact analysis for changes
  - `get_ccg_manifest` - get codebase overview (if CCG available)
  - `export_ccg_architecture` - understand public API surface
- Identify all types/functions that will be affected

**Current Focus (Sprint 5 Tasks 4-5):**
If working on CCG integration, the narsil-mcp tools to implement are:
- `get_ccg_manifest` → returns L0 JSON-LD (~1-2KB)
- `export_ccg_architecture` → returns L1 JSON-LD (~10-50KB)
- `export_ccg` → exports all layers to directory
- These require narsil-mcp built with `--features graph`

## Phase 2: TEST FIRST (TDD)
**Before writing ANY implementation code:**
1. Write failing test(s) that define the expected behavior
2. Run tests to confirm they fail for the right reason
3. Document the behavioral contract in test comments
4. If modifying existing code, ensure existing tests still define correct behavior

**Test Requirements:**
- Every public function must have at least one test
- Every public type must be exercised in integration tests
- Edge cases must be tested (empty inputs, errors, boundaries)
- Use `#[should_panic]` for expected panic paths

## Phase 3: IMPLEMENT
- Write minimal code to make tests pass
- Use `find_references` to ensure no breaking changes
- Update inline documentation with `# Examples` and `# Panics`

**Implementation Rules:**
- NO `#[allow(...)]` annotations - fix warnings at source
- NO `#[dead_code]` - if it exists, it must be tested and used
- NO placeholder/stub implementations - fully implement or don't merge
- NO TODO/FIXME comments in merged code
- Every warning is a bug - resolve before proceeding

## Phase 4: REFACTOR
- Clean up implementation while keeping tests green
- Extract common patterns into helpers (only if used 3+ times)
- Ensure code follows existing project patterns

## Phase 5: REVIEW
- Run `cargo clippy --all-targets -- -D warnings` (treat warnings as errors)
- Run `cargo test` (all tests must pass)
- Run narsil-mcp security scans (if available):
  - `scan_security` - resolve all CRITICAL/HIGH
  - `find_injection_vulnerabilities` - must be zero findings
  - `check_cwe_top25` - review any new findings
- Check documentation drift - update docs/ if API changed

**Graceful Degradation Rule:**
All narsil-mcp integration code MUST work when narsil-mcp is unavailable:
- Return `None` or empty collections when tool not found
- Use `NarsilClient::is_available()` to check before invoking
- Never panic if narsil-mcp is missing
- Log at debug level when degrading, not error level

## Phase 6: COMMIT
- Run full test suite one more time
- If ALL checks pass: `git add -A && git commit -m "feat: [description]"`
- Update IMPLEMENTATION_PLAN.md marking task complete
- If ANY check fails: DO NOT COMMIT - fix issues first

## Hard Rules (Violations = Immediate Stop)

1. **NEVER modify existing tests to make them pass** - tests define correct behavior
2. **NEVER use #[allow(...)]** - fix the underlying issue
3. **NEVER leave dead code** - delete or wire in with tests
4. **NEVER commit with warnings** - `cargo clippy` must be clean
5. **NEVER commit with failing tests** - all tests must pass
6. **NEVER skip security review** - scan_security before every commit
7. **NEVER add code without tests** - TDD is mandatory
8. **ALWAYS search before implementing** - use `find_similar_code` to avoid duplication

## Quality Gates (Must Pass Before Commit)

```
[ ] cargo clippy --all-targets -- -D warnings  (0 warnings)
[ ] cargo test                                  (all pass)
[ ] scan_security                               (0 CRITICAL/HIGH)
[ ] find_injection_vulnerabilities              (0 findings)
[ ] All new public APIs documented              (docs/api.md updated)
[ ] All new types have tests                    (coverage verified)
```

## TDD Cycle Summary

```
RED    -> Write failing test
GREEN  -> Write minimal code to pass
REFACTOR -> Clean up, keeping tests green
REVIEW -> Security + clippy + full test suite
COMMIT -> Only if ALL gates pass
```
