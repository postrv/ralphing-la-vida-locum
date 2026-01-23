# Ralph v2.0 Implementation Plan

> **Mission**: Transform Ralph into the definitive polyglot AI coding orchestration tool.
> 
> **Methodology**: TDD, quality-gated, production-ready. Every task follows RED → GREEN → REFACTOR → COMMIT.
> 
> **Current Focus: Sprint 13 (Plugin Architecture Foundation)**

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

**Goal**: Abstract Claude client to support multiple LLM backends.

### 12. Phase 12.1: LLM Client Trait ✅ COMPLETE

Define trait for LLM client abstraction.

**Test Requirements**:
- [x] Test trait defines `run_prompt(&self, prompt: &str) -> Result<String>`
- [x] Test trait defines `model_name(&self) -> &str`
- [x] Test trait is object-safe for dynamic dispatch
- [x] Test mock implementation works for testing
- [x] Test Claude implementation works (existing behavior)

**Implementation**:
- [x] Create `trait LlmClient: Send + Sync` in new `src/llm/mod.rs`
- [x] Define core methods: `run_prompt`, `model_name`, `supports_tools`
- [x] Create `ClaudeClient` implementing trait (wrap existing code)
- [x] Create `MockLlmClient` for testing

**Quality Gates**:
```bash
cargo clippy --all-targets -- -D warnings
cargo test --lib -- llm_client_trait
cargo test --lib -- claude_client
```

### 13. Phase 12.2: Model Configuration ✅ COMPLETE

Add configuration for selecting and configuring LLM backend.

**Test Requirements**:
- [x] Test model can be specified in settings.json
- [x] Test model can be overridden via CLI flag
- [x] Test invalid model name produces helpful error
- [x] Test model-specific options are validated
- [x] Test default model is Claude

**Implementation**:
- [x] Add `llm` section to `ProjectConfig`
- [x] Define `LlmConfig` with `model: String`, `api_key_env: String`, `options: Map`
- [x] Add `--model <name>` CLI flag
- [x] Implement model factory based on config

**Quality Gates**:
```bash
cargo clippy --all-targets -- -D warnings
cargo test --lib -- llm_config
cargo test --lib -- model_selection
```

### 14. Phase 12.3: OpenAI/Gemini Client Stubs ✅ COMPLETE

Create stub implementations for alternative models (implementation deferred).

**Test Requirements**:
- [x] Test OpenAI client stub exists and returns "not implemented" error
- [x] Test Gemini client stub exists and returns "not implemented" error
- [x] Test Ollama client stub exists for local models
- [x] Test client stubs are documented with implementation roadmap
- [x] Test model list includes all stub models with "coming soon" status

**Implementation**:
- [x] Create `OpenAiClient` stub with unimplemented methods
- [x] Create `GeminiClient` stub with unimplemented methods
- [x] Create `OllamaClient` stub for local models
- [x] Add `ModelStatus` enum and `get_supported_models()` for model status
- [x] Document implementation requirements in each stub

**Quality Gates**:
```bash
cargo clippy --all-targets -- -D warnings
cargo test --lib -- openai_stub
cargo test --lib -- gemini_stub
```

---

## Sprint 13: Plugin Architecture Foundation

**Goal**: Enable community-contributed quality gates via plugin system.

### 15. Phase 13.1: Gate Plugin Trait ✅ COMPLETE

Define plugin interface for external quality gates.

**Test Requirements**:
- [x] Test plugin trait extends QualityGate trait
- [x] Test plugin defines metadata: name, version, author
- [x] Test plugin can be loaded from shared library
- [x] Test plugin errors are isolated (don't crash Ralph)
- [x] Test plugin timeout prevents hanging

**Implementation**:
- [x] Create `trait GatePlugin: QualityGate` with metadata methods
- [x] Define plugin manifest format (TOML)
- [x] Implement safe plugin loading with error isolation
- [x] Add plugin timeout configuration

**Quality Gates**:
```bash
cargo clippy --all-targets -- -D warnings
cargo test --lib -- gate_plugin_trait
```

### 16. Phase 13.2: Plugin Discovery and Loading ✅ COMPLETE

Implement plugin discovery from standard locations.

**Test Requirements**:
- [x] Test plugins discovered from `~/.ralph/plugins/`
- [x] Test plugins discovered from project `.ralph/plugins/`
- [x] Test plugin manifest is validated before loading
- [x] Test duplicate plugin names produce warning
- [x] Test plugin load failures are logged but don't stop Ralph

**Implementation**:
- [x] Create `PluginLoader` struct
- [x] Implement directory scanning for plugin manifests
- [x] Implement manifest validation
- [x] Load plugins via `libloading` crate (manifest discovery, library loading deferred)
- [x] Add `ralph plugins list` command

**Quality Gates**:
```bash
cargo clippy --all-targets -- -D warnings
cargo test --lib -- plugin_discovery
cargo test --lib -- plugin_loading
```

### 17. Phase 13.3: Example Plugin: RuboCop Gate

Create example Ruby plugin to demonstrate plugin system.

**Test Requirements**:
- [ ] Test RuboCop plugin compiles as shared library
- [ ] Test plugin runs `rubocop` command
- [ ] Test plugin parses RuboCop JSON output
- [ ] Test plugin produces GateIssue list
- [ ] Test plugin provides remediation guidance

**Implementation**:
- [ ] Create `examples/plugins/rubocop-gate/` directory
- [ ] Implement `RubocopGatePlugin` struct
- [ ] Create plugin manifest `plugin.toml`
- [ ] Add build instructions to README
- [ ] Document as plugin development template

**Quality Gates**:
```bash
cargo clippy --all-targets -- -D warnings
cd examples/plugins/rubocop-gate && cargo build
cargo test --lib -- rubocop_plugin
```

---

## Sprint 14: Documentation & Examples

**Goal**: Create comprehensive documentation for polyglot usage.

### 18. Phase 14.1: Polyglot Quick Start Guide

Write quick start guide for Python, TypeScript, and Go projects.

**Test Requirements**:
- [ ] Test Python quick start commands work on fresh project
- [ ] Test TypeScript quick start commands work on fresh project
- [ ] Test Go quick start commands work on fresh project
- [ ] Test all code examples compile/run
- [ ] Test documentation renders correctly on GitHub

**Implementation**:
- [ ] Create `docs/quickstart-python.md`
- [ ] Create `docs/quickstart-typescript.md`
- [ ] Create `docs/quickstart-go.md`
- [ ] Add code examples with expected output
- [ ] Link from main README

**Quality Gates**:
```bash
# Documentation tests (markdown lint)
npx markdownlint docs/*.md
# Example validation (manual review)
```

### 19. Phase 14.2: Example Polyglot Project

Create complete example project demonstrating polyglot features.

**Test Requirements**:
- [ ] Test example project has Next.js frontend
- [ ] Test example project has FastAPI backend
- [ ] Test `ralph bootstrap` works on example
- [ ] Test `ralph loop --max-iterations 3` completes successfully
- [ ] Test example includes sample IMPLEMENTATION_PLAN.md

**Implementation**:
- [ ] Create `examples/polyglot-fullstack/`
- [ ] Add Next.js app with TypeScript
- [ ] Add FastAPI app with Python
- [ ] Add shared OpenAPI types
- [ ] Add README with walkthrough
- [ ] Add sample implementation plan

**Quality Gates**:
```bash
cd examples/polyglot-fullstack && npm install && npm run lint
cd examples/polyglot-fullstack/backend && pip install -r requirements.txt && ruff check .
```

### 20. Phase 14.3: Gate Development Guide

Document how to create custom quality gates.

**Test Requirements**:
- [ ] Test guide explains QualityGate trait
- [ ] Test guide includes complete code example
- [ ] Test guide explains testing strategies
- [ ] Test guide covers plugin vs built-in development
- [ ] Test example code compiles

**Implementation**:
- [ ] Create `docs/developing-gates.md`
- [ ] Document trait requirements with examples
- [ ] Document testing patterns
- [ ] Document contribution process
- [ ] Add inline code examples

**Quality Gates**:
```bash
npx markdownlint docs/developing-gates.md
# Code example extraction and compilation (manual)
```

---

## Sprint 15: Performance & Reliability

**Goal**: Ensure Ralph performs well on large polyglot projects.

### 21. Phase 15.1: Gate Execution Parallelization

Run independent gates in parallel for faster feedback.

**Test Requirements**:
- [ ] Test independent gates run concurrently
- [ ] Test gate results are collected correctly
- [ ] Test parallel execution respects timeout
- [ ] Test failure in one gate doesn't cancel others
- [ ] Test parallelization is configurable (can be disabled)

**Implementation**:
- [ ] Add `--parallel-gates` flag (default: true)
- [ ] Use `tokio::spawn` for concurrent gate execution
- [ ] Implement result collection with `futures::join_all`
- [ ] Add per-gate timeout with cancellation
- [ ] Add total gate execution timing to analytics

**Quality Gates**:
```bash
cargo clippy --all-targets -- -D warnings
cargo test --lib -- parallel_gates
cargo test --lib -- gate_timeout
```

### 22. Phase 15.2: Incremental Gate Execution

Only run gates for changed languages/files when possible.

**Test Requirements**:
- [ ] Test only Python gates run when only .py files changed
- [ ] Test only TypeScript gates run when only .ts files changed
- [ ] Test all gates run on first iteration
- [ ] Test config/manifest changes trigger full gate run
- [ ] Test incremental mode is configurable

**Implementation**:
- [ ] Add `--incremental-gates` flag (default: true)
- [ ] Detect changed files via `git diff --name-only`
- [ ] Map file extensions to languages
- [ ] Skip gates for unchanged languages
- [ ] Force full run on manifest file changes

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
