# Build Phase - PHP Production Standard (TDD MANDATORY)

## CRITICAL: This is a TDD-First, Production-Quality Codebase

**Every change MUST follow Test-Driven Development:**
1. Write failing tests FIRST - before ANY implementation code
2. Tests define the contract - implementation follows
3. No exceptions, no shortcuts, no "I'll add tests later"

**Production standard means:**
- Zero linting warnings, zero dead code, zero TODOs
- Every public method tested and documented
- Security scanned before every commit

---

## Phase 0: REINDEX (Start of Task)
**Before starting any task, refresh narsil-mcp index:**
```
reindex
```
This ensures code intelligence reflects the current codebase state.

---

## Phase 1: PLAN
- Read IMPLEMENTATION_PLAN.md
- Select highest-priority incomplete task
- **Context Gathering (narsil-mcp - optional, degrades gracefully):**
  - `get_call_graph` - understand method relationships
  - `get_dependencies` - understand namespace dependencies
  - `find_references` - impact analysis for changes
- Identify all classes/namespaces that will be affected

## Phase 2: TEST FIRST (TDD) - NON-NEGOTIABLE

**YOU MUST WRITE TESTS BEFORE IMPLEMENTATION CODE. NO EXCEPTIONS.**

Before writing ANY implementation code:
1. Write failing test(s) using PHPUnit that define the expected behavior
2. Run `./vendor/bin/phpunit` to confirm they fail for the right reason
3. Document the behavioral contract in test annotations
4. If modifying existing code, ensure existing tests still define correct behavior

**Test Requirements:**
- Every public method must have at least one test
- Use PHPUnit data providers for multiple test cases
- Use `expectException()` for expected exceptions
- Use mocks and stubs for external dependencies

**TDD Violation = STOP IMMEDIATELY**
If you find yourself writing implementation before tests:
1. STOP
2. Delete the implementation code
3. Write the test first
4. Then write the minimal implementation to pass

## Phase 3: IMPLEMENT
- Write minimal code to make tests pass
- Use `find_references` to ensure no breaking changes
- Update PHPDoc with @param, @return, @throws

**Implementation Rules:**
- NO `@phpstan-ignore` without explicit justification
- NO `// @codeCoverageIgnore` to skip coverage
- NO placeholder/stub implementations - fully implement or don't merge
- NO TODO/FIXME comments in merged code
- Every warning is a bug - resolve before proceeding

## Phase 4: REFACTOR
- Clean up implementation while keeping tests green
- Extract common patterns into traits/services (only if used 3+ times)
- Ensure code follows existing project patterns

## Phase 5: REVIEW
- Run `./vendor/bin/phpstan analyse` - treat warnings as errors
- Run `./vendor/bin/phpcs` - coding standards must pass
- Run `./vendor/bin/phpunit` - all tests must pass
- Run `composer audit` - resolve all vulnerability findings
- Run narsil-mcp security scans (if available):
  - `scan_security` - resolve all CRITICAL/HIGH
  - `find_injection_vulnerabilities` - must be zero findings

**Graceful Degradation Rule:**
All narsil-mcp integration code MUST work when narsil-mcp is unavailable:
- Return `null` or empty arrays when tool not found
- Never crash if narsil-mcp is missing

## Phase 6: COMMIT
- Run full test suite one more time
- If ALL checks pass: `git add -A && git commit -m "feat: [description]"`
- Update IMPLEMENTATION_PLAN.md marking task complete
- If ANY check fails: DO NOT COMMIT - fix issues first

## Phase 7: REINDEX (End of Task)
**After completing a task, refresh narsil-mcp index:**
```
reindex
```
This ensures the next task starts with accurate code intelligence.

## Hard Rules (Violations = Immediate Stop)

1. **NEVER modify existing tests to make them pass** - tests define correct behavior
2. **NEVER use `@phpstan-ignore`** without explicit justification
3. **NEVER leave dead code** - delete or wire in with tests
4. **NEVER commit with warnings** - PHPStan/PHPCS must be clean
5. **NEVER commit with failing tests** - PHPUnit must pass
6. **NEVER skip security review** - composer audit before every commit
7. **NEVER add code without tests** - TDD is mandatory
8. **ALWAYS search before implementing** - use `find_similar_code` to avoid duplication

## Quality Gates (Must Pass Before Commit)

```
[ ] ./vendor/bin/phpstan analyse              (0 errors)
[ ] ./vendor/bin/phpcs                         (0 violations)
[ ] ./vendor/bin/phpunit                       (all pass)
[ ] composer audit                             (0 vulnerabilities)
[ ] scan_security                              (0 CRITICAL/HIGH)
[ ] find_injection_vulnerabilities             (0 findings)
[ ] All new public APIs documented             (PHPDoc verified)
[ ] All new methods have tests                 (coverage verified)
```

## TDD Cycle Summary

```
REINDEX  -> Refresh narsil-mcp index (start of task)
RED      -> Write failing PHPUnit test FIRST (mandatory)
GREEN    -> Write minimal code to pass
REFACTOR -> Clean up, keeping tests green
REVIEW   -> Security + PHPStan + PHPCS + full test suite
COMMIT   -> Only if ALL gates pass
REINDEX  -> Refresh narsil-mcp index (end of task)
```

**Remember: Tests define behavior. Implementation follows. Never the reverse.**
