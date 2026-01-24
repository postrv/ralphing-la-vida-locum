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

**Current Test Count**: 2,430 passing (1,918 lib + 512 bin)

---

## Sprint 26: Incremental Execution Mode

**Goal**: Run Ralph on changed files only for large codebase support.

**Success Criteria**:
- `--changed-since <commit>` runs gates only on changed files
- `--files <glob>` explicitly specifies files to process
- Task selection prioritizes tasks affecting changed files
- 10x+ speedup on large repos with small changes

**Already Complete**:
- [x] Phase 26.1: Change Detection (`src/changes/mod.rs`)
- [x] Phase 26.2: Scoped Quality Gates (`run_scoped()` on all gates)
- [x] Phase 26.3: Scoped Context Building (`ChangeScope`, CCG neighbors)
- [x] Phase 26.4: Scoped Task Selection (`affected_files`, prioritization)

### Phase 26.5: CLI Integration ✅

**Description**: Add incremental execution flags to CLI.

**Requirements**:
- [x] Add `--changed-since <commit>` flag to `ralph loop`
- [x] Add `--files <glob>` flag to `ralph loop`
- [x] Add `--changed` flag as shorthand for `--changed-since HEAD~1`
- [x] Flags are mutually exclusive (error if both specified)
- [x] Log scope at start: "Running in incremental mode: 5 files changed since abc123"

---

## Optional: Phase 24.3 - Adaptive Weight Tuning

**Description**: Slowly adjust predictor weights based on recorded accuracy.

**Requirements**:
- [ ] Add `enable_adaptive_weights` config option (default: false)
- [ ] Track which factors contributed to correct vs incorrect predictions
- [ ] Implement simple weight adjustment: +0.1 for factors in correct predictions, -0.1 for incorrect
- [ ] Clamp weights to [0.1, 2.0] range to prevent runaway
- [ ] Add `ralph predictor tune` command to manually trigger tuning
- [ ] Log weight changes

---

## Pending CLI Integration ✅

These CLI flags were implemented in the library and are now wired to CLI:

- [x] `--model` flag: `claude`, `openai`, `gemini`, `ollama`, `auto`
  - Model variant (opus/sonnet/haiku) is read from config and passed to Claude CLI
  - `RealClaudeProcess::with_model()` constructor added
- [x] `--no-fallback` flag to disable automatic provider fallback
  - Flag added to CLI, logs warning when used
  - Currently a no-op until ProviderRouter is integrated into LoopManager

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
