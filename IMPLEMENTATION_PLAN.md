# Ralph v2.0 Implementation Plan

> **Mission**: Transform Ralph into the definitive polyglot AI coding orchestration tool.
>
> **Methodology**: TDD, quality-gated, production-ready. Every task follows RED → GREEN → REFACTOR → COMMIT.
>
> **Current Focus: Sprint 16 (Analytics & Observability)**

---

## Progress Overview

| Phase | Sprints | Status |
|-------|---------|--------|
| Phase 1: Polyglot Gate Integration | 7-9 | ✅ Complete |
| Phase 2: Reliability Hardening | 10-12 | ✅ Complete |
| Phase 3: Ecosystem & Extensibility | 13-15 | ✅ Complete |
| **Phase 4: Commercial Foundation** | **16-18** | **In Progress** |

> See `docs/COMPLETED_SPRINTS.md` for detailed archive of completed work.

---

## Sprint 16: Analytics & Observability

**Goal**: Add opt-in analytics for understanding Ralph usage patterns.

### 24. Phase 16.1: Structured Event Logging ✅

Standardize event logging for analytics consumption.

**Test Requirements**:
- [x] Test events have consistent schema
- [x] Test events include timestamp, session_id, event_type
- [x] Test gate results are logged as structured events
- [x] Test predictor decisions are logged as structured events
- [x] Test events can be filtered by type

**Implementation**:
- [x] Define `EventType` enum with all event types
- [x] Add schema version to events (`SCHEMA_VERSION`)
- [x] Implement event serialization to JSON (`StructuredEvent`)
- [x] Add event filtering API (`EventFilter`)
- [x] Add structured event types (`GateResultEventData`, `PredictorDecisionEventData`)

**Quality Gates**:
```bash
cargo clippy --all-targets -- -D warnings
cargo test --lib -- analytics_event
cargo test --lib -- event_schema
```

### 25. Phase 16.2: Session Summary Report ✅

Generate detailed summary report at end of each session.

**Test Requirements**:
- [x] Test summary includes iteration count
- [x] Test summary includes tasks completed
- [x] Test summary includes gate pass/fail rates
- [x] Test summary includes predictor accuracy
- [x] Test summary can be exported as JSON/Markdown

**Implementation**:
- [x] Create `SessionReport` struct with all metrics
- [x] Collect metrics throughout session (`generate_session_report`)
- [x] Generate report on session end (`generate_session_report`)
- [x] Add `ReportFormat` enum with JSON/Markdown export
- [x] Add `to_markdown()` and `to_json()` export methods

**Quality Gates**:
```bash
cargo clippy --all-targets -- -D warnings
cargo test --lib -- session_report
cargo test --lib -- report_export
```

### 26. Phase 16.3: Quality Trend Visualization

Add command to visualize quality trends over time.

**Test Requirements**:
- [ ] Test trend shows test count over sessions
- [ ] Test trend shows warning count over sessions
- [ ] Test trend shows commit frequency
- [ ] Test trend can be output as ASCII chart
- [ ] Test trend data can be exported for external visualization

**Implementation**:
- [ ] Create `ralph analytics trends` command
- [ ] Aggregate metrics across sessions
- [ ] Implement ASCII chart rendering
- [ ] Add `--json` flag for data export
- [ ] Add `--days <n>` flag to limit time range

**Quality Gates**:
```bash
cargo clippy --all-targets -- -D warnings
cargo test --lib -- quality_trends
cargo test --test cli_analytics_trends
```

---

## Sprint 17: Enterprise Features Foundation

**Goal**: Add features required for team/enterprise usage.

### 27. Phase 17.1: Configuration Inheritance

Support configuration inheritance for team-wide defaults.

**Test Requirements**:
- [ ] Test project config inherits from user config
- [ ] Test user config inherits from system config
- [ ] Test explicit values override inherited values
- [ ] Test arrays are merged, not replaced
- [ ] Test inheritance chain is logged in verbose mode

**Implementation**:
- [ ] Define config file locations: system, user, project
- [ ] Implement config loading with inheritance
- [ ] Add merge logic for nested objects
- [ ] Add array merge strategy configuration
- [ ] Document configuration precedence

**Quality Gates**:
```bash
cargo clippy --all-targets -- -D warnings
cargo test --lib -- config_inheritance
cargo test --lib -- config_merge
```

### 28. Phase 17.2: Shared Gate Configurations

Allow gate configurations to be shared across team.

**Test Requirements**:
- [ ] Test gate config can reference external file
- [ ] Test external config is resolved relative to project root
- [ ] Test external config can be URL (future: cloud)
- [ ] Test missing external config produces clear error
- [ ] Test config validation includes external configs

**Implementation**:
- [ ] Add `extends: <path>` support in gate config
- [ ] Implement config resolution
- [ ] Add `ralph config validate` command
- [ ] Document shared config patterns
- [ ] Add examples in `examples/shared-config/`

**Quality Gates**:
```bash
cargo clippy --all-targets -- -D warnings
cargo test --lib -- shared_config
cargo test --lib -- config_validation
```

### 29. Phase 17.3: Audit Logging

Add audit log for compliance and debugging.

**Test Requirements**:
- [ ] Test audit log records all command executions
- [ ] Test audit log records all gate results
- [ ] Test audit log records all commits
- [ ] Test audit log is tamper-evident (hashed entries)
- [ ] Test audit log rotation works

**Implementation**:
- [ ] Create `AuditLogger` with append-only log
- [ ] Add entry hashing for integrity verification
- [ ] Implement log rotation by size/date
- [ ] Add `ralph audit show` command
- [ ] Add `ralph audit verify` command

**Quality Gates**:
```bash
cargo clippy --all-targets -- -D warnings
cargo test --lib -- audit_logger
cargo test --lib -- audit_integrity
```

---

## Sprint 18: Cloud Foundation (Stubs)

**Goal**: Create stubs for future cloud features without implementing backend.

### 30. Phase 18.1: Remote Analytics Upload Stub

Create opt-in analytics upload stub.

**Test Requirements**:
- [ ] Test upload is disabled by default
- [ ] Test upload can be enabled via config
- [ ] Test upload stub logs what would be sent
- [ ] Test upload respects data privacy settings
- [ ] Test upload failure doesn't affect Ralph operation

**Implementation**:
- [ ] Add `analytics.upload_enabled: bool` to config
- [ ] Create `AnalyticsUploader` trait
- [ ] Implement stub that logs to file instead of uploading
- [ ] Add data anonymization options
- [ ] Document data that would be uploaded

**Quality Gates**:
```bash
cargo clippy --all-targets -- -D warnings
cargo test --lib -- analytics_upload_stub
```

### 31. Phase 18.2: Remote Campaign API Stub

Create stub for cloud-based campaign orchestration.

**Test Requirements**:
- [ ] Test campaign API trait is defined
- [ ] Test stub returns "not available" for all methods
- [ ] Test campaign ID can be specified in config
- [ ] Test local campaigns work without cloud
- [ ] Test cloud features are clearly marked as "coming soon"

**Implementation**:
- [ ] Define `CampaignApi` trait with CRUD methods
- [ ] Implement `LocalCampaignApi` for current behavior
- [ ] Create `CloudCampaignApi` stub
- [ ] Add feature flag for cloud features
- [ ] Document cloud feature roadmap

**Quality Gates**:
```bash
cargo clippy --all-targets -- -D warnings
cargo test --lib -- campaign_api_stub
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
