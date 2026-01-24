# Ralph Self-Improvement Implementation Plan

> **Meta-Development**: This plan will be executed by Ralph to improve Ralph.
> **Methodology**: Strict TDD - Write failing test → Minimal implementation → Refactor
> **Quality Standard**: Production-grade, zero warnings, comprehensive documentation

---

## Overview

| Sprint | Focus | Effort | Status |
|--------|-------|--------|--------|
| 23 | LLM Provider Abstraction | 2-3 days | **Complete** |
| 24 | Predictor Persistence & Diagnostics | 1-2 days | **Complete** |
| 25 | Analytics Dashboard | 1-2 days | **Complete** |
| 26 | Incremental Execution Mode | 2-3 days | **Complete** |

**Completed Sprints**: See `docs/COMPLETED_SPRINTS.md`

**Current Test Count**: 2,440 passing (1,918 lib + 522 bin)

---

## Phase 24.3: Adaptive Weight Tuning ✅

**Description**: Slowly adjust predictor weights based on recorded accuracy.

**Requirements**:
- [x] Add `enable_adaptive_weights` config option (default: false)
- [x] Track which factors contributed to correct vs incorrect predictions
- [x] Implement simple weight adjustment: +0.1 for factors in correct predictions, -0.1 for incorrect
- [x] Clamp weights to [0.1, 2.0] range to prevent runaway
- [x] Add `ralph predictor tune` command to manually trigger tuning
- [x] Log weight changes

---

## Global Quality Gates

Before ANY commit:

```bash
# Must all pass
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo test
cargo doc --no-deps

# Should pass (warn if not)
cargo deny check advisories
cargo deny check licenses
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
9. **Use narsil**: Reindex before starting work, use for code discovery.

---

## Completion Criteria

This plan is complete when:
1. All checkboxes are marked
2. All quality gates pass
3. `cargo test` shows 2,500+ tests passing
4. Documentation is updated
5. CHANGELOG.md reflects all changes
