# Build Phase - Ruby Production Standard (TDD MANDATORY)

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
  - `get_dependencies` - understand module dependencies
  - `find_references` - impact analysis for changes
- Identify all modules/classes that will be affected

## Phase 2: TEST FIRST (TDD) - NON-NEGOTIABLE

**YOU MUST WRITE TESTS BEFORE IMPLEMENTATION CODE. NO EXCEPTIONS.**

Before writing ANY implementation code:
1. Write failing test(s) using RSpec that define the expected behavior
2. Run `bundle exec rspec` to confirm they fail for the right reason
3. Document the behavioral contract in spec descriptions
4. If modifying existing code, ensure existing specs still define correct behavior

**Test Requirements:**
- Every public method must have at least one spec
- Use RSpec `let` and `let!` for shared setup
- Use `expect { }.to raise_error` for expected exceptions
- Use `shared_examples` for common behaviors

**TDD Violation = STOP IMMEDIATELY**
If you find yourself writing implementation before tests:
1. STOP
2. Delete the implementation code
3. Write the spec first
4. Then write the minimal implementation to pass

## Phase 3: IMPLEMENT
- Write minimal code to make specs pass
- Use `find_references` to ensure no breaking changes
- Update YARD documentation with examples

**Implementation Rules:**
- NO `# rubocop:disable` without explicit justification
- NO placeholder/stub implementations - fully implement or don't merge
- NO TODO/FIXME comments in merged code
- Every warning is a bug - resolve before proceeding

## Phase 4: REFACTOR
- Clean up implementation while keeping specs green
- Extract common patterns into modules (only if used 3+ times)
- Ensure code follows existing project patterns

## Phase 5: REVIEW
- Run `bundle exec rubocop` - treat warnings as errors
- Run `bundle exec rspec` - all specs must pass
- Run `bundle exec brakeman` (if Rails) - resolve all HIGH/CRITICAL findings
- Run narsil-mcp security scans (if available):
  - `scan_security` - resolve all CRITICAL/HIGH
  - `find_injection_vulnerabilities` - must be zero findings

**Graceful Degradation Rule:**
All narsil-mcp integration code MUST work when narsil-mcp is unavailable:
- Return `nil` or empty collections when tool not found
- Never crash if narsil-mcp is missing

## Phase 6: COMMIT
- Run full spec suite one more time
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

1. **NEVER modify existing specs to make them pass** - specs define correct behavior
2. **NEVER use `# rubocop:disable`** without explicit justification
3. **NEVER leave dead code** - delete or wire in with tests
4. **NEVER commit with warnings** - RuboCop must be clean
5. **NEVER commit with failing specs** - RSpec must pass
6. **NEVER skip security review** - brakeman/bundler-audit before every commit
7. **NEVER add code without specs** - TDD is mandatory
8. **ALWAYS search before implementing** - use `find_similar_code` to avoid duplication

## Quality Gates (Must Pass Before Commit)

```
[ ] bundle exec rubocop                       (0 warnings)
[ ] bundle exec rspec                          (all pass)
[ ] bundle exec brakeman -q                    (0 HIGH/CRITICAL, if Rails)
[ ] bundle audit check                         (0 vulnerabilities)
[ ] scan_security                              (0 CRITICAL/HIGH)
[ ] find_injection_vulnerabilities             (0 findings)
[ ] All new public APIs documented             (YARD docs verified)
[ ] All new methods have specs                 (coverage verified)
```

## TDD Cycle Summary

```
REINDEX  -> Refresh narsil-mcp index (start of task)
RED      -> Write failing RSpec spec FIRST (mandatory)
GREEN    -> Write minimal code to pass
REFACTOR -> Clean up, keeping specs green
REVIEW   -> Security + RuboCop + full spec suite
COMMIT   -> Only if ALL gates pass
REINDEX  -> Refresh narsil-mcp index (end of task)
```

**Remember: Specs define behavior. Implementation follows. Never the reverse.**
