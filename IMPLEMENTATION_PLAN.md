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

### 33. Phase 19.1: Config Validate Command ✅

Add `ralph config validate` command.

**Test Requirements**:
- [x] Test validates project config syntax
- [x] Test validates inheritance chain resolution
- [x] Test validates extends references exist
- [x] Test reports missing required fields
- [x] Test exits with appropriate error codes

**Implementation**:
- [x] Add `ConfigValidator` struct
- [x] Implement validation for all config sections
- [x] Add `ralph config validate` subcommand to CLI
- [x] Add `--verbose` flag for detailed output
- [x] Add `--json` flag for machine-readable output
- [x] Document in CLI help

**Quality Gates**:
```bash
cargo clippy --all-targets -- -D warnings
cargo test --lib -- config_validate
```

### 34. Phase 19.2: Audit Show Command

Add `ralph audit show` command.

**Test Requirements**:
- [ ] Test displays recent audit entries
- [ ] Test supports `--limit N` flag
- [ ] Test supports `--since <datetime>` filter
- [ ] Test supports `--type <event_type>` filter
- [ ] Test outputs JSON with `--json` flag

**Implementation**:
- [ ] Add `AuditReader` for querying log
- [ ] Implement filter/limit logic
- [ ] Add `ralph audit show` subcommand to CLI
- [ ] Add formatted table output
- [ ] Document in CLI help

**Quality Gates**:
```bash
cargo clippy --all-targets -- -D warnings
cargo test --lib -- audit_show
```

### 35. Phase 19.3: Audit Verify Command

Add `ralph audit verify` command.

**Test Requirements**:
- [ ] Test verifies hash chain integrity
- [ ] Test reports first corrupted entry
- [ ] Test succeeds on valid log
- [ ] Test fails on tampered log
- [ ] Test outputs verification report

**Implementation**:
- [ ] Add verification to `AuditLogger::verify()`
- [ ] Add `ralph audit verify` subcommand to CLI
- [ ] Add `--repair` flag to truncate at corruption
- [ ] Add JSON report output
- [ ] Document in CLI help

**Quality Gates**:
```bash
cargo clippy --all-targets -- -D warnings
cargo test --lib -- audit_verify
```

### 36. Phase 19.4: Verify Mock Command

Add `ralph verify --mock` command (deferred from Sprint 18.3).

**Test Requirements**:
- [ ] Test `ralph verify` with `--mock` flag
- [ ] Test outputs verification report in JSON format
- [ ] Test outputs verification report in Markdown format
- [ ] Test integrates with existing `MockCcgVerifier`
- [ ] Test help text describes verification purpose

**Implementation**:
- [ ] Add `ralph verify` subcommand to CLI
- [ ] Add `--mock` flag to use `MockCcgVerifier`
- [ ] Add `--json` flag for JSON output
- [ ] Add `--output <file>` flag to write report to file
- [ ] Document in CLI help

**Quality Gates**:
```bash
cargo clippy --all-targets -- -D warnings
cargo test --lib -- verify_command
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
