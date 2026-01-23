# Ralph v2.0 Implementation Plan

> **Mission**: Transform Ralph into the definitive polyglot AI coding orchestration tool.
> 
> **Methodology**: TDD, quality-gated, production-ready. Every task follows RED → GREEN → REFACTOR → COMMIT.
> 
> **Current Focus: Sprint 15 (Performance & Reliability)**

---

## Overview

This plan implements the strategic roadmap to make Ralph genuinely best-in-class for Python, TypeScript, Go, and beyond. The architecture is already excellent—this work is about completing the integration.

**Phase 1**: Polyglot Gate Integration (Sprints 7-9)
**Phase 2**: Reliability Hardening (Sprints 10-12)
**Phase 3**: Ecosystem & Extensibility (Sprints 13-15)
**Phase 4**: Commercial Foundation (Sprints 16-18)

---

## Sprint 7: Polyglot Quality Gate Wiring ✅ COMPLETE

> **Completed 2026-01-22**: All 5 phases (7.1-7.5). Added `LoopDependencies::real_polyglot()`, `RealQualityChecker::with_gates()`, gate availability detection, `PolyglotGateResult`, and `ralph detect` command.

---

## Sprint 8: Language-Specific Prompt Intelligence ✅ COMPLETE

> **Completed 2026-01-23**: All 3 phases (8.1-8.3). Added language-aware `PromptAssembler`, code antipattern detection for Python/TypeScript/Go, and `ClaudeMdGenerator` for dynamic CLAUDE.md generation.

---

## Sprint 9: Polyglot Orchestration & Validation ✅ COMPLETE

> **Completed 2026-01-23**: All 3 phases (9.1-9.3). Added weighted gate scoring with `GateWeightConfig`, context window language prioritization with `ContextPrioritizer`, and 22 end-to-end polyglot integration tests.

---

## Sprint 10: Predictor Action Enforcement ✅ COMPLETE

> **Completed 2026-01-23**: All 3 phases (10.1-10.3). Added `PreventiveActionHandler`, predictor accuracy tracking with `PredictionStatistics`, and dynamic risk weight tuning with `WeightPreset` enum and `--predictor-profile` CLI flag.

---

## Sprint 11: Enhanced Checkpoint System ✅ COMPLETE

> **Completed 2026-01-23**: All 3 phases (11.1-11.3). Added `metrics_by_language` to `Checkpoint`, `LanguageRegression` struct, `LintRegressionSeverity` enum with tiered thresholds, `WarningTrend` tracking, `CheckpointDiff` struct, `CheckpointManager::diff()`, and `ralph checkpoint diff` CLI command with JSON output.

---

## Sprint 12: Model Abstraction Layer ✅ COMPLETE

> **Completed 2026-01-23**: All 3 phases (12.1-12.3). Added `LlmClient` trait, `LlmConfig` configuration, model factory, and `OpenAiClient`/`GeminiClient`/`OllamaClient` stubs with `ModelStatus` enum.

---

## Sprint 13: Plugin Architecture Foundation ✅ COMPLETE

> **Completed 2026-01-23**: All 3 phases (13.1-13.3). Added `GatePlugin` trait with metadata and timeout configuration, `PluginLoader` for discovery from `~/.ralph/plugins/` and project `.ralph/plugins/`, and example `rubocop-gate` plugin demonstrating plugin development.

---

## Sprint 14: Documentation & Examples ✅ COMPLETE

> **Completed 2026-01-23**: All 3 phases (14.1-14.3). Added quickstart guides for Python, TypeScript, and Go projects, example polyglot fullstack project with Next.js and FastAPI, and comprehensive gate development guide for custom quality gates.

---

## Sprint 15: Performance & Reliability

**Goal**: Ensure Ralph performs well on large polyglot projects.

### 21. Phase 15.1: Gate Execution Parallelization ✅ COMPLETE

Run independent gates in parallel for faster feedback.

**Test Requirements**:
- [x] Test independent gates run concurrently
- [x] Test gate results are collected correctly
- [x] Test parallel execution respects timeout
- [x] Test failure in one gate doesn't cancel others
- [x] Test parallelization is configurable (can be disabled)

**Implementation**:
- [x] Add `--parallel-gates` flag (default: true)
- [x] Use `tokio::spawn` for concurrent gate execution
- [x] Implement result collection with `futures::join_all`
- [x] Add per-gate timeout with cancellation
- [x] Add total gate execution timing to analytics

**Quality Gates**:
```bash
cargo clippy --all-targets -- -D warnings
cargo test --lib -- parallel_gates
cargo test --lib -- gate_timeout
```

### 22. Phase 15.2: Incremental Gate Execution ✅

Only run gates for changed languages/files when possible.

**Test Requirements**:
- [x] Test only Python gates run when only .py files changed
- [x] Test only TypeScript gates run when only .ts files changed
- [x] Test all gates run on first iteration
- [x] Test config/manifest changes trigger full gate run
- [x] Test incremental mode is configurable

**Implementation**:
- [x] Add `--incremental-gates` flag (default: true)
- [x] Detect changed files via `git diff --name-only`
- [x] Map file extensions to languages
- [x] Skip gates for unchanged languages
- [x] Force full run on manifest file changes

**Quality Gates**:
```bash
cargo clippy --all-targets -- -D warnings
cargo test --lib -- incremental_gates
cargo test --lib -- changed_file_detection
```

### 23. Phase 15.3: Benchmark Suite

Create benchmark suite for performance regression detection.

**Test Requirements**:
- [ ] Test benchmark measures gate execution time
- [ ] Test benchmark measures language detection time
- [ ] Test benchmark measures context building time
- [ ] Test benchmark produces machine-readable output
- [ ] Test benchmark can compare against baseline

**Implementation**:
- [ ] Add `benches/` directory with Criterion benchmarks
- [ ] Create benchmark for each major subsystem
- [ ] Add baseline recording: `cargo bench -- --save-baseline main`
- [ ] Add comparison: `cargo bench -- --baseline main`
- [ ] Add CI job for performance regression detection

**Quality Gates**:
```bash
cargo clippy --all-targets -- -D warnings
cargo bench --no-run  # Verify benchmarks compile
```

---

## Sprint 16: Analytics & Observability

**Goal**: Add opt-in analytics for understanding Ralph usage patterns.

### 24. Phase 16.1: Structured Event Logging

Standardize event logging for analytics consumption.

**Test Requirements**:
- [ ] Test events have consistent schema
- [ ] Test events include timestamp, session_id, event_type
- [ ] Test gate results are logged as structured events
- [ ] Test predictor decisions are logged as structured events
- [ ] Test events can be filtered by type

**Implementation**:
- [ ] Define `AnalyticsEvent` enum with all event types
- [ ] Add schema version to events
- [ ] Implement event serialization to JSON
- [ ] Add event filtering API
- [ ] Update all subsystems to emit structured events

**Quality Gates**:
```bash
cargo clippy --all-targets -- -D warnings
cargo test --lib -- analytics_event
cargo test --lib -- event_schema
```

### 25. Phase 16.2: Session Summary Report

Generate detailed summary report at end of each session.

**Test Requirements**:
- [ ] Test summary includes iteration count
- [ ] Test summary includes tasks completed
- [ ] Test summary includes gate pass/fail rates
- [ ] Test summary includes predictor accuracy
- [ ] Test summary can be exported as JSON/Markdown

**Implementation**:
- [ ] Create `SessionReport` struct with all metrics
- [ ] Collect metrics throughout session
- [ ] Generate report on session end
- [ ] Add `--report <format>` flag for export
- [ ] Display summary in terminal on completion

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

## Completion Criteria

When all sprints are complete and all checkboxes are checked, proceed to the next phase or request further direction.

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
- Architecture: `src/lib.rs` module documentation
- Quality Gate Trait: `src/quality/gates/mod.rs`
- Language Detector: `src/bootstrap/language_detector.rs`
- Stagnation Predictor: `src/supervisor/predictor.rs`
