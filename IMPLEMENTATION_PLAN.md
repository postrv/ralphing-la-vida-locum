# Ralph v2.0 Implementation Plan

> **Mission**: Transform Ralph into the definitive polyglot AI coding orchestration tool.
>
> **Methodology**: TDD, quality-gated, production-ready. Every task follows RED → GREEN → REFACTOR → COMMIT.
>
> **Current Focus: Sprint 19 (CLI Commands)**

---

## Progress Overview

| Phase | Sprints | Status |
|-------|---------|--------|
| Phase 1: Polyglot Gate Integration | 7-9 | ✅ Complete |
| Phase 2: Reliability Hardening | 10-12 | ✅ Complete |
| Phase 3: Ecosystem & Extensibility | 13-15 | ✅ Complete |
| Phase 4: Commercial Foundation | 16-17 | ✅ Complete |
| Phase 5: Cloud & CLI | 18-19 | ✅ Sprint 18 Complete |
| **Current Sprint** | **19** | **In Progress** |

> See `docs/COMPLETED_SPRINTS.md` for detailed archive of completed work.

---

## Sprint 19: CLI Commands

**Goal**: Add CLI commands for config validation, audit management, and verification.

> ✅ **Completed**: 19.1 Config Validate, 19.2 Audit Show, 19.3 Audit Verify (see `docs/COMPLETED_SPRINTS.md`)

### ~~35. Phase 19.3: Audit Verify Command~~ ✅ Complete

Add `ralph audit verify` command.

**Test Requirements**:
- [x] Test verifies hash chain integrity
- [x] Test reports first corrupted entry
- [x] Test succeeds on valid log
- [x] Test fails on tampered log
- [x] Test outputs verification report

**Implementation**:
- [x] Add verification to `AuditLogger::verify()` (already existed)
- [x] Add `ralph audit verify` subcommand to CLI
- [x] Add `--repair` flag to truncate at corruption
- [x] Add JSON report output
- [x] Document in CLI help

**Quality Gates**:
```bash
cargo clippy --all-targets -- -D warnings
cargo test --lib -- audit_verify
```

### ~~36. Phase 19.4: Verify Mock Command~~ ✅ Complete

Add `ralph verify --mock` command (deferred from Sprint 18.3).

**Test Requirements**:
- [x] Test `ralph verify` with `--mock` flag
- [x] Test outputs verification report in JSON format
- [x] Test outputs verification report in Markdown format
- [x] Test integrates with existing `MockCcgVerifier`
- [x] Test help text describes verification purpose

**Implementation**:
- [x] Add `ralph verify` subcommand to CLI
- [x] Add `--mock` flag to use `MockCcgVerifier`
- [x] Add `--json` flag for JSON output
- [x] Add `--markdown` flag for Markdown output
- [x] Add `--output <file>` flag to write report to file
- [x] Document in CLI help

**Quality Gates**:
```bash
cargo clippy --all-targets -- -D warnings
cargo test --test cli_integration verify_command
```

---

## Notes for Claude

- **Reindex at phase boundaries**: Run `reindex` (narsil-mcp) at the START and END of each phase to ensure code intelligence is current.
- **Always run quality gates before committing**: `cargo clippy --all-targets -- -D warnings && cargo test`
- **Follow TDD strictly**: Write failing test first, then minimal implementation, then refactor.
- **One task at a time**: Complete current task before starting next. Mark checkboxes as you go.
- **If stuck**: Re-read the task requirements. Check existing implementations for patterns. Ask for clarification via IMPLEMENTATION_PLAN.md update.
- **Commit messages**: Use conventional commits format: `feat(quality): add polyglot gate result aggregation`
- **Documentation**: Update relevant docs when adding public APIs.

### Phase Workflow

```
START OF PHASE:
  reindex                    # Refresh narsil-mcp index

DURING PHASE:
  RED → GREEN → REFACTOR → COMMIT (repeat per task)

END OF PHASE:
  reindex                    # Update narsil-mcp index
  Update checkboxes in this plan
```

---

## References

- Strategy Document: `RALPH_STRATEGY.md`
- Completed Work: `docs/COMPLETED_SPRINTS.md`
- Architecture: `src/lib.rs` module documentation
- Quality Gate Trait: `src/quality/gates/mod.rs`
- Language Detector: `src/bootstrap/language_detector.rs`
- Stagnation Predictor: `src/supervisor/predictor.rs`
