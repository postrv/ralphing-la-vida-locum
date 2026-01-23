# Ralph v2.0 Implementation Plan

> **Mission**: Transform Ralph into the definitive polyglot AI coding orchestration tool.
>
> **Methodology**: TDD, quality-gated, production-ready. Every task follows RED → GREEN → REFACTOR → COMMIT.
>
> **Current Focus: Sprint 18 (Cloud Foundation Stubs) or Sprint 19 (CLI Commands)**

---

## Progress Overview

| Phase | Sprints | Status |
|-------|---------|--------|
| Phase 1: Polyglot Gate Integration | 7-9 | ✅ Complete |
| Phase 2: Reliability Hardening | 10-12 | ✅ Complete |
| Phase 3: Ecosystem & Extensibility | 13-15 | ✅ Complete |
| Phase 4: Commercial Foundation | 16-17 | ✅ Complete |
| **Phase 5: Cloud & CLI** | **18-19** | **In Progress** |

> See `docs/COMPLETED_SPRINTS.md` for detailed archive of completed work.

---

## Sprint 18: Cloud Foundation (Stubs)

**Goal**: Create stubs for future cloud features without implementing backend.

### 30. Phase 18.1: Remote Analytics Upload Stub ✅

Create opt-in analytics upload stub.

**Test Requirements**:
- [x] Test upload is disabled by default
- [x] Test upload can be enabled via config
- [x] Test upload stub logs what would be sent
- [x] Test upload respects data privacy settings
- [x] Test upload failure doesn't affect Ralph operation

**Implementation**:
- [x] Add `analytics.upload_enabled: bool` to config
- [x] Create `AnalyticsUploader` trait
- [x] Implement stub that logs to file instead of uploading
- [x] Add data anonymization options
- [x] Document data that would be uploaded

**Quality Gates**:
```bash
cargo clippy --all-targets -- -D warnings
cargo test --lib -- analytics_upload_stub
```

### 31. Phase 18.2: Remote Campaign API Stub ✅

Create stub for cloud-based campaign orchestration.

**Test Requirements**:
- [x] Test campaign API trait is defined
- [x] Test stub returns "not available" for all methods
- [x] Test campaign ID can be specified in config
- [x] Test local campaigns work without cloud
- [x] Test cloud features are clearly marked as "coming soon"

**Implementation**:
- [x] Define `CampaignApi` trait with CRUD methods
- [x] Implement `LocalCampaignApi` for current behavior
- [x] Create `CloudCampaignApi` stub
- [x] Add feature flag for cloud features
- [x] Document cloud feature roadmap

**Quality Gates**:
```bash
cargo clippy --all-targets -- -D warnings
cargo test --lib -- campaign::
```

### 32. Phase 18.3: CCG-Diff Verification Stub

Create stub for provable quality improvement verification.

**Test Requirements**:
- [ ] Test CCG-diff trait is defined
- [ ] Test stub returns mock "quality improved" result
- [ ] Test CCG integration points are documented
- [ ] Test narsil-mcp hooks are prepared
- [ ] Test verification report format is defined

**Implementation**:
- [ ] Define `CcgVerifier` trait
- [ ] Implement mock verifier for development
- [ ] Document CCG integration requirements
- [ ] Define verification report JSON schema
- [ ] Add `ralph verify --mock` command

**Quality Gates**:
```bash
cargo clippy --all-targets -- -D warnings
cargo test --lib -- ccg_verifier_stub
```

---

## Sprint 19: CLI Commands (Deferred from Sprint 17)

**Goal**: Add CLI commands for config validation and audit management.

### 33. Phase 19.1: Config Validate Command

Add `ralph config validate` command.

**Test Requirements**:
- [ ] Test validates project config syntax
- [ ] Test validates inheritance chain resolution
- [ ] Test validates extends references exist
- [ ] Test reports missing required fields
- [ ] Test exits with appropriate error codes

**Implementation**:
- [ ] Add `ConfigValidator` struct
- [ ] Implement validation for all config sections
- [ ] Add `ralph config validate` subcommand to CLI
- [ ] Add `--verbose` flag for detailed output
- [ ] Document in CLI help

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
