# Ralph v2.0 Implementation Plan

> **Mission**: Transform Ralph into the definitive polyglot AI coding orchestration tool.
> 
> **Methodology**: TDD, quality-gated, production-ready. Every task follows RED → GREEN → REFACTOR → COMMIT.
> 
> **Current Focus: Sprint 8 (Language-Specific Prompt Intelligence)**

---

## Overview

This plan implements the strategic roadmap to make Ralph genuinely best-in-class for Python, TypeScript, Go, and beyond. The architecture is already excellent—this work is about completing the integration.

**Phase 1**: Polyglot Gate Integration (Sprints 7-9)  
**Phase 2**: Reliability Hardening (Sprints 10-12)  
**Phase 3**: Ecosystem & Extensibility (Sprints 13-15)  
**Phase 4**: Commercial Foundation (Sprints 16-18)

---

## Sprint 7: Polyglot Quality Gate Wiring ✅ COMPLETE

**Goal**: Wire language detection into quality gate selection so `ralph loop` runs the right gates for any project.

> **Status**: All 5 phases complete (7.1-7.5). Committed 2026-01-22.

### 1. Phase 7.1: LoopDependencies Polyglot Constructor

Create `LoopDependencies::real_polyglot()` that detects languages and selects appropriate gates.

**Test Requirements**:
- [x] Test that `real_polyglot()` detects Rust project and returns ClippyGate, TestGate
- [x] Test that `real_polyglot()` detects Python project and returns RuffGate, PytestGate, MypyGate
- [x] Test that `real_polyglot()` detects TypeScript project and returns EslintGate, JestGate, TscGate
- [x] Test that `real_polyglot()` detects polyglot project and returns gates for all detected languages
- [x] Test graceful degradation when no language detected (returns empty gates, not error)

**Implementation**:
- [x] Add `real_polyglot(project_dir: PathBuf) -> Self` method to `LoopDependencies`
- [x] Call `LanguageDetector::new(&project_dir).detect()` to get languages
- [x] Filter to languages with confidence >= 0.10
- [x] Call `detect_available_gates(&project_dir, &languages)` for gate selection
- [ ] Create `RealQualityChecker::with_gates()` with selected gates (deferred to Phase 7.2)
- [x] Update `LoopManager::new()` to use `real_polyglot()` by default

**Quality Gates**:
```bash
cargo clippy --all-targets -- -D warnings  # 0 warnings
cargo test --lib -- loop_dependencies      # All pass
cargo test --lib -- real_polyglot          # All pass
```

### 2. Phase 7.2: RealQualityChecker Gate Injection

Extend `RealQualityChecker` to accept injected gates instead of hardcoded Rust gates.

**Test Requirements**:
- [x] Test `RealQualityChecker::with_gates()` stores provided gates
- [x] Test `run_gates()` executes all injected gates
- [x] Test `run_gates()` returns combined results from multiple languages
- [x] Test empty gates list returns success (no gates to fail)
- [x] Test gate execution order is deterministic

**Implementation**:
- [x] Add `gates: Vec<Box<dyn QualityGate>>` field to `RealQualityChecker`
- [x] Add `with_gates(project_dir: PathBuf, gates: Vec<Box<dyn QualityGate>>) -> Self`
- [x] Modify `run_gates()` to iterate over `self.gates` instead of hardcoded list
- [x] Ensure backward compatibility: `new()` defaults to Rust gates for existing projects

**Quality Gates**:
```bash
cargo clippy --all-targets -- -D warnings
cargo test --lib -- real_quality_checker
cargo test --lib -- with_gates
```

### 3. Phase 7.3: Gate Availability Detection ✅

Implement tool availability checking so gates only run when tools are installed.

**Test Requirements**:
- [x] Test `is_gate_available()` returns true when tool exists in PATH
- [x] Test `is_gate_available()` returns false when tool missing
- [x] Test `detect_available_gates()` filters out unavailable gates
- [x] Test Python gates check for `ruff`, `pytest`, `mypy`, `bandit`
- [x] Test TypeScript gates check for `eslint`, `jest`, `npx tsc`, `npm`
- [x] Test Go gates check for `go`, `golangci-lint`, `govulncheck`

**Implementation**:
- [x] Add `fn required_tool(&self) -> Option<&str>` to `QualityGate` trait
- [x] Implement `is_gate_available(gate: &dyn QualityGate) -> bool` using `which`
- [x] Update `detect_available_gates()` to filter using availability check
- [x] Add logging when gates are skipped due to missing tools

**Quality Gates**:
```bash
cargo clippy --all-targets -- -D warnings
cargo test --lib -- gate_available
cargo test --lib -- detect_available
```

### 4. Phase 7.4: PolyglotGateResult Aggregation ✅

Create result type that aggregates gate results across multiple languages.

**Test Requirements**:
- [x] Test `PolyglotGateResult::can_commit()` returns true when all gates pass
- [x] Test `PolyglotGateResult::can_commit()` returns false when any blocking gate fails
- [x] Test `PolyglotGateResult::summary()` shows per-language breakdown
- [x] Test `PolyglotGateResult::blocking_failures()` returns only blocking failures
- [x] Test `PolyglotGateResult::warnings()` returns non-blocking issues

**Implementation**:
- [x] Create `PolyglotGateResult` struct with `by_language: HashMap<Language, Vec<GateResult>>`
- [x] Add `blocking_failures()` and `warnings()` methods (computed from by_language)
- [x] Implement `can_commit(&self) -> bool`
- [x] Implement `summary(&self) -> String` with per-language gate counts
- [x] Implement `remediation_prompt(&self) -> String` for Claude feedback

**Quality Gates**:
```bash
cargo clippy --all-targets -- -D warnings
cargo test --lib -- polyglot_gate_result
```

### 5. Phase 7.5: CLI Integration ✅

Update CLI commands to display polyglot gate information.

**Test Requirements**:
- [x] Test `ralph detect` shows all detected languages with confidence
- [x] Test `ralph bootstrap` reports detected languages during setup
- [x] Test `ralph loop` logs which gates are being run
- [x] Test `ralph loop --verbose` shows per-language gate results

**Implementation**:
- [x] Add `detect` command to show all detected languages with confidence
- [x] Add `--show-gates` flag to detect command to show gate availability per language
- [x] Update `bootstrap` to log detected languages and selected gates
- [x] Add gate count to loop banner output
- [x] Add detailed gate list in verbose mode

**Quality Gates**:
```bash
cargo clippy --all-targets -- -D warnings
cargo test --test cli_integration
```

---

## Sprint 8: Language-Specific Prompt Intelligence

**Goal**: Generate prompts that include language-appropriate TDD guidance, antipatterns, and quality rules.

### 6. Phase 8.1: PromptAssembler Language Awareness

Extend `PromptAssembler` to accept detected languages and generate appropriate prompts.

**Test Requirements**:
- [ ] Test assembler includes Rust quality rules for Rust projects
- [ ] Test assembler includes Python quality rules for Python projects
- [ ] Test assembler includes TypeScript quality rules for TypeScript projects
- [ ] Test polyglot projects get combined rules with clear separation
- [ ] Test language-specific TDD patterns are included

**Implementation**:
- [ ] Add `languages: Vec<Language>` field to `AssemblerConfig`
- [ ] Create `get_language_rules(lang: Language) -> String` helper
- [ ] Modify `build()` to inject language-specific rules into prompt
- [ ] Use `TemplateRegistry::get_polyglot_prompt()` for multi-language projects

**Quality Gates**:
```bash
cargo clippy --all-targets -- -D warnings
cargo test --lib -- prompt_assembler
cargo test --lib -- language_rules
```

### 7. Phase 8.2: Language-Specific Antipattern Detection

Add antipattern detection rules for Python, TypeScript, and Go.

**Test Requirements**:
- [ ] Test Python antipatterns: bare except, mutable default args, global state
- [ ] Test TypeScript antipatterns: any type, non-null assertion, console.log
- [ ] Test Go antipatterns: ignored errors, empty interface abuse, panic in library
- [ ] Test antipattern detector accepts language parameter
- [ ] Test polyglot projects check antipatterns for changed file types only

**Implementation**:
- [ ] Add `antipatterns_for_language(lang: Language) -> Vec<AntipatternRule>`
- [ ] Create antipattern rules for Python, TypeScript, Go
- [ ] Modify antipattern detector to filter by file extension
- [ ] Add antipattern results to remediation prompt

**Quality Gates**:
```bash
cargo clippy --all-targets -- -D warnings
cargo test --lib -- antipattern
cargo test --lib -- python_antipattern
cargo test --lib -- typescript_antipattern
```

### 8. Phase 8.3: Dynamic CLAUDE.md Generation

Generate project-specific CLAUDE.md with detected languages and their rules.

**Test Requirements**:
- [ ] Test generated CLAUDE.md includes detected primary language
- [ ] Test generated CLAUDE.md includes quality gate commands for each language
- [ ] Test generated CLAUDE.md includes TDD methodology for each language
- [ ] Test polyglot CLAUDE.md has clear sections per language
- [ ] Test regeneration preserves user customizations (marked sections)

**Implementation**:
- [ ] Create `ClaudeMdGenerator` struct with language list
- [ ] Implement `generate(&self) -> String` using `TemplateRegistry`
- [ ] Add user customization markers: `<!-- USER_CUSTOM_START -->` / `<!-- USER_CUSTOM_END -->`
- [ ] Update bootstrap to use generator instead of static template

**Quality Gates**:
```bash
cargo clippy --all-targets -- -D warnings
cargo test --lib -- claude_md_generator
```

---

## Sprint 9: Polyglot Orchestration & Validation

**Goal**: Validate end-to-end polyglot functionality on real projects.

### 9. Phase 9.1: Weighted Gate Scoring

Implement weighted scoring for polyglot gate results based on change scope.

**Test Requirements**:
- [ ] Test gates for changed files are weighted higher
- [ ] Test unchanged language gates contribute less to overall score
- [ ] Test blocking failures always block regardless of weight
- [ ] Test weights are configurable via settings.json
- [ ] Test default weights: changed files 1.0, unchanged 0.3

**Implementation**:
- [ ] Add `weight: f64` field to gate execution context
- [ ] Compute weights based on `git diff --name-only` file extensions
- [ ] Modify `PolyglotGateResult::can_commit()` to use weighted scoring
- [ ] Add `gate_weights` section to `settings.json` schema

**Quality Gates**:
```bash
cargo clippy --all-targets -- -D warnings
cargo test --lib -- weighted_scoring
cargo test --lib -- gate_weights
```

### 10. Phase 9.2: Context Window Language Prioritization

Prioritize context inclusion based on detected languages and changed files.

**Test Requirements**:
- [ ] Test changed files are always included in context
- [ ] Test primary language files are prioritized over secondary
- [ ] Test context respects token limits while maximizing relevant content
- [ ] Test test files are included when related source files change
- [ ] Test config files (Cargo.toml, package.json) included when relevant

**Implementation**:
- [ ] Add `prioritize_by_language(files: Vec<Path>, languages: Vec<Language>) -> Vec<Path>`
- [ ] Implement relevance scoring: changed > primary > secondary > other
- [ ] Update context builder to use prioritization
- [ ] Add `context_priority` configuration options

**Quality Gates**:
```bash
cargo clippy --all-targets -- -D warnings
cargo test --lib -- context_priority
cargo test --lib -- language_prioritization
```

### 11. Phase 9.3: End-to-End Polyglot Integration Test

Create comprehensive integration test with real polyglot project.

**Test Requirements**:
- [ ] Test `ralph bootstrap` on Next.js + FastAPI project
- [ ] Test language detection finds TypeScript and Python
- [ ] Test `ralph loop --max-iterations 1` runs correct gates
- [ ] Test gate failures produce appropriate remediation
- [ ] Test commits only happen when all relevant gates pass

**Implementation**:
- [ ] Create test fixture: `tests/fixtures/polyglot-nextjs-fastapi/`
- [ ] Add minimal Next.js frontend with TypeScript
- [ ] Add minimal FastAPI backend with Python
- [ ] Write integration test exercising full bootstrap → loop → commit cycle
- [ ] Add CI workflow for polyglot integration tests

**Quality Gates**:
```bash
cargo clippy --all-targets -- -D warnings
cargo test --test polyglot_integration
```

---

## Sprint 10: Predictor Action Enforcement

**Goal**: Wire `StagnationPredictor` preventive actions into the loop for proactive intervention.

### 12. Phase 10.1: PreventiveAction Handler

Implement handler that converts predictor actions into loop behavior.

**Test Requirements**:
- [ ] Test `InjectGuidance` adds text to prompt extras
- [ ] Test `FocusTask` sets task tracker to specific task
- [ ] Test `RunTests` triggers quality gate test-only run
- [ ] Test `SuggestCommit` triggers commit when gates pass
- [ ] Test `SwitchMode` changes loop mode
- [ ] Test `RequestReview` pauses loop and returns control to user
- [ ] Test `None` action has no effect

**Implementation**:
- [ ] Create `PreventiveActionHandler` struct
- [ ] Implement `handle(&self, action: PreventiveAction, manager: &mut LoopManager) -> Result<()>`
- [ ] Wire handler into main loop after predictor evaluation
- [ ] Add action execution logging

**Quality Gates**:
```bash
cargo clippy --all-targets -- -D warnings
cargo test --lib -- preventive_action_handler
cargo test --lib -- action_inject_guidance
cargo test --lib -- action_switch_mode
```

### 13. Phase 10.2: Predictor Accuracy Tracking

Track and report predictor accuracy over time for self-improvement.

**Test Requirements**:
- [ ] Test prediction recording stores score and outcome
- [ ] Test accuracy calculation: correct / total predictions
- [ ] Test accuracy reported in session summary
- [ ] Test accuracy persists across sessions (analytics)
- [ ] Test accuracy breakdown by risk level

**Implementation**:
- [ ] Add `prediction_history: Vec<(RiskScore, bool)>` to predictor
- [ ] Implement `record_prediction(score: f64, actually_stagnated: bool)`
- [ ] Implement `prediction_accuracy() -> Option<f64>`
- [ ] Add accuracy to `AnalyticsEvent::SessionComplete`
- [ ] Add accuracy trend to aggregate stats

**Quality Gates**:
```bash
cargo clippy --all-targets -- -D warnings
cargo test --lib -- predictor_accuracy
cargo test --lib -- prediction_history
```

### 14. Phase 10.3: Dynamic Risk Weight Tuning

Allow risk weights to be tuned based on project characteristics.

**Test Requirements**:
- [ ] Test custom weights can be specified in settings.json
- [ ] Test weights are normalized to sum to 1.0
- [ ] Test weight changes affect risk score calculation
- [ ] Test preset weight profiles: "conservative", "aggressive", "balanced"
- [ ] Test weight validation rejects invalid configurations

**Implementation**:
- [ ] Add `predictor_weights` section to `ProjectConfig`
- [ ] Load custom weights in `StagnationPredictor::new()`
- [ ] Add weight presets as static configurations
- [ ] Add CLI flag: `--predictor-profile <preset>`

**Quality Gates**:
```bash
cargo clippy --all-targets -- -D warnings
cargo test --lib -- predictor_weights
cargo test --lib -- weight_presets
```

---

## Sprint 11: Enhanced Checkpoint System

**Goal**: Extend checkpoint system with per-language quality metrics and smarter regression detection.

### 15. Phase 11.1: Language-Aware Quality Metrics

Track quality metrics separately for each detected language.

**Test Requirements**:
- [ ] Test checkpoint stores per-language test counts
- [ ] Test checkpoint stores per-language lint warning counts
- [ ] Test checkpoint stores per-language coverage (if available)
- [ ] Test regression detection works per-language
- [ ] Test rollback considers language-specific thresholds

**Implementation**:
- [ ] Add `metrics_by_language: HashMap<Language, QualityMetrics>` to `Checkpoint`
- [ ] Collect metrics from each language's gates during checkpoint creation
- [ ] Update `has_regression()` to check per-language metrics
- [ ] Add language to regression report output

**Quality Gates**:
```bash
cargo clippy --all-targets -- -D warnings
cargo test --lib -- language_aware_metrics
cargo test --lib -- per_language_regression
```

### 16. Phase 11.2: Lint Warning Regression Detection

Add regression detection for lint warning counts, not just test failures.

**Test Requirements**:
- [ ] Test lint warning increase triggers regression warning
- [ ] Test lint warning threshold is configurable
- [ ] Test small increases (1-2 warnings) produce warning, not rollback
- [ ] Test large increases trigger automatic rollback
- [ ] Test warning trend tracking across checkpoints

**Implementation**:
- [ ] Add `lint_warning_count: u32` to `QualityMetrics`
- [ ] Add `max_warning_increase: u32` to `RegressionThresholds`
- [ ] Update `has_regression()` to check warning delta
- [ ] Add warning trend to checkpoint comparison

**Quality Gates**:
```bash
cargo clippy --all-targets -- -D warnings
cargo test --lib -- lint_regression
cargo test --lib -- warning_threshold
```

### 17. Phase 11.3: Checkpoint Diff Visualization

Add ability to visualize quality changes between checkpoints.

**Test Requirements**:
- [ ] Test diff shows test count changes
- [ ] Test diff shows lint warning changes
- [ ] Test diff shows files modified between checkpoints
- [ ] Test diff output is machine-readable (JSON option)
- [ ] Test diff can compare arbitrary checkpoint IDs

**Implementation**:
- [ ] Create `CheckpointDiff` struct with delta fields
- [ ] Implement `CheckpointManager::diff(id1: &str, id2: &str) -> CheckpointDiff`
- [ ] Add `ralph checkpoint diff <id1> <id2>` command
- [ ] Add `--json` flag for machine-readable output

**Quality Gates**:
```bash
cargo clippy --all-targets -- -D warnings
cargo test --lib -- checkpoint_diff
cargo test --test cli_checkpoint_diff
```

---

## Sprint 12: Model Abstraction Layer

**Goal**: Abstract Claude client to support multiple LLM backends.

### 18. Phase 12.1: LLM Client Trait

Define trait for LLM client abstraction.

**Test Requirements**:
- [ ] Test trait defines `run_prompt(&self, prompt: &str) -> Result<String>`
- [ ] Test trait defines `model_name(&self) -> &str`
- [ ] Test trait is object-safe for dynamic dispatch
- [ ] Test mock implementation works for testing
- [ ] Test Claude implementation works (existing behavior)

**Implementation**:
- [ ] Create `trait LlmClient: Send + Sync` in new `src/llm/mod.rs`
- [ ] Define core methods: `run_prompt`, `model_name`, `supports_tools`
- [ ] Create `ClaudeClient` implementing trait (wrap existing code)
- [ ] Create `MockLlmClient` for testing

**Quality Gates**:
```bash
cargo clippy --all-targets -- -D warnings
cargo test --lib -- llm_client_trait
cargo test --lib -- claude_client
```

### 19. Phase 12.2: Model Configuration

Add configuration for selecting and configuring LLM backend.

**Test Requirements**:
- [ ] Test model can be specified in settings.json
- [ ] Test model can be overridden via CLI flag
- [ ] Test invalid model name produces helpful error
- [ ] Test model-specific options are validated
- [ ] Test default model is Claude

**Implementation**:
- [ ] Add `llm` section to `ProjectConfig`
- [ ] Define `LlmConfig` with `model: String`, `api_key_env: String`, `options: Map`
- [ ] Add `--model <name>` CLI flag
- [ ] Implement model factory based on config

**Quality Gates**:
```bash
cargo clippy --all-targets -- -D warnings
cargo test --lib -- llm_config
cargo test --lib -- model_selection
```

### 20. Phase 12.3: OpenAI/Gemini Client Stubs

Create stub implementations for alternative models (implementation deferred).

**Test Requirements**:
- [ ] Test OpenAI client stub exists and returns "not implemented" error
- [ ] Test Gemini client stub exists and returns "not implemented" error
- [ ] Test Ollama client stub exists for local models
- [ ] Test client stubs are documented with implementation roadmap
- [ ] Test model list includes all stub models with "coming soon" status

**Implementation**:
- [ ] Create `OpenAiClient` stub with unimplemented methods
- [ ] Create `GeminiClient` stub with unimplemented methods
- [ ] Create `OllamaClient` stub for local models
- [ ] Add feature flags for optional model support
- [ ] Document implementation requirements in each stub

**Quality Gates**:
```bash
cargo clippy --all-targets -- -D warnings
cargo test --lib -- openai_stub
cargo test --lib -- gemini_stub
```

---

## Sprint 13: Plugin Architecture Foundation

**Goal**: Enable community-contributed quality gates via plugin system.

### 21. Phase 13.1: Gate Plugin Trait

Define plugin interface for external quality gates.

**Test Requirements**:
- [ ] Test plugin trait extends QualityGate trait
- [ ] Test plugin defines metadata: name, version, author
- [ ] Test plugin can be loaded from shared library
- [ ] Test plugin errors are isolated (don't crash Ralph)
- [ ] Test plugin timeout prevents hanging

**Implementation**:
- [ ] Create `trait GatePlugin: QualityGate` with metadata methods
- [ ] Define plugin manifest format (TOML)
- [ ] Implement safe plugin loading with error isolation
- [ ] Add plugin timeout configuration

**Quality Gates**:
```bash
cargo clippy --all-targets -- -D warnings
cargo test --lib -- gate_plugin_trait
```

### 22. Phase 13.2: Plugin Discovery and Loading

Implement plugin discovery from standard locations.

**Test Requirements**:
- [ ] Test plugins discovered from `~/.ralph/plugins/`
- [ ] Test plugins discovered from project `.ralph/plugins/`
- [ ] Test plugin manifest is validated before loading
- [ ] Test duplicate plugin names produce warning
- [ ] Test plugin load failures are logged but don't stop Ralph

**Implementation**:
- [ ] Create `PluginLoader` struct
- [ ] Implement directory scanning for plugin manifests
- [ ] Implement manifest validation
- [ ] Load plugins via `libloading` crate
- [ ] Add `ralph plugins list` command

**Quality Gates**:
```bash
cargo clippy --all-targets -- -D warnings
cargo test --lib -- plugin_discovery
cargo test --lib -- plugin_loading
```

### 23. Phase 13.3: Example Plugin: RuboCop Gate

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

### 24. Phase 14.1: Polyglot Quick Start Guide

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

### 25. Phase 14.2: Example Polyglot Project

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

### 26. Phase 14.3: Gate Development Guide

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

### 27. Phase 15.1: Gate Execution Parallelization

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

### 28. Phase 15.2: Incremental Gate Execution

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

### 29. Phase 15.3: Benchmark Suite

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

### 30. Phase 16.1: Structured Event Logging

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

### 31. Phase 16.2: Session Summary Report

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

### 32. Phase 16.3: Quality Trend Visualization

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

### 33. Phase 17.1: Configuration Inheritance

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

### 34. Phase 17.2: Shared Gate Configurations

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

### 35. Phase 17.3: Audit Logging

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

### 36. Phase 18.1: Remote Analytics Upload Stub

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

### 37. Phase 18.2: Remote Campaign API Stub

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

### 38. Phase 18.3: CCG-Diff Verification Stub

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

1. **Reindex at phase boundaries**: Run `reindex` (narsil-mcp) at the START and END of each phase to ensure code intelligence is current.

2. **Always run quality gates before committing**: `cargo clippy --all-targets -- -D warnings && cargo test`

3. **Follow TDD strictly**: Write failing test first, then minimal implementation, then refactor.

4. **One task at a time**: Complete current task before starting next. Mark checkboxes as you go.

5. **If stuck**: Re-read the task requirements. Check existing implementations for patterns. Ask for clarification via IMPLEMENTATION_PLAN.md update.

6. **Commit messages**: Use conventional commits format: `feat(quality): add polyglot gate result aggregation`

7. **Documentation**: Update relevant docs when adding public APIs.

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
