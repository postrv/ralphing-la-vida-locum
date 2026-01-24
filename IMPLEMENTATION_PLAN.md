# Implementation Plan Template

> **Meta-Development**: This plan will be executed by Ralph to improve your project.
> **Methodology**: Strict TDD - Write failing test → Minimal implementation → Refactor
> **Quality Standard**: Production-grade, zero warnings, comprehensive documentation

---

## Overview

| Sprint | Focus | Effort | Status |
|--------|-------|--------|--------|
| 1 | Example Sprint | 1-2 days | **In Progress** |

---

## Sprint 1: Example Sprint

**Goal**: Describe the sprint goal here.

**Success Criteria**:
- Criterion 1
- Criterion 2
- Criterion 3

### Phase 1.1: First Phase

**Description**: Describe what this phase accomplishes.

**Requirements**:
- [ ] Requirement 1
- [ ] Requirement 2
- [ ] Requirement 3

### Phase 1.2: Second Phase

**Description**: Describe what this phase accomplishes.

**Requirements**:
- [ ] Requirement 1
- [ ] Requirement 2

---

## Global Quality Gates

Before ANY commit:

```bash
# Must all pass
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo test
cargo doc --no-deps
```

---

## Notes for Ralph (Self-Execution)

1. **Follow TDD religiously**: Write the failing test first. Watch it fail. Then implement.
2. **One task at a time**: Complete and commit each phase before starting the next.
3. **Run quality gates before committing**: All gates must pass.
4. **Update this plan**: Mark checkboxes as you complete requirements.
5. **If blocked**: Document the blocker, mark task as blocked, move to next task.
6. **Commit messages**: Use conventional commits (`feat:`, `fix:`, `refactor:`, `test:`, `docs:`).
7. **No dead code**: Remove any commented-out code or unused functions.
8. **No TODOs in committed code**: Either do it now or create a task for later.

---

## Completion Criteria

This plan is complete when:
1. All checkboxes are marked
2. All quality gates pass
3. Documentation is updated
4. CHANGELOG.md reflects all changes
