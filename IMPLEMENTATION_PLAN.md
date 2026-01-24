# Ralph Self-Improvement Implementation Plan

> **Meta-Development**: This plan will be executed by Ralph to improve Ralph.
> **Methodology**: Strict TDD - Write failing test → Minimal implementation → Refactor
> **Quality Standard**: Production-grade, zero warnings, comprehensive documentation

---

## Overview

| Sprint | Focus | Effort | Risk Mitigation |
|--------|-------|--------|-----------------|
| 21 | Session Persistence & Resume | 2-3 days | Highest leverage - enables longer autonomous runs |
| 22 | File Decomposition | 2-3 days | Improves maintainability, reduces cognitive load |
| 23 | LLM Provider Abstraction | 2-3 days | Resilience through provider fallback |
| 24 | Predictor Persistence & Diagnostics | 1-2 days | Cross-session learning, faster human intervention |
| 25 | Analytics Dashboard | 1-2 days | Retrospectives, commercial demo value |
| 26 | Incremental Execution Mode | 2-3 days | Large codebase support |

**Total Estimated Effort**: 10-16 days

---

## Sprint 21: Session Persistence & Resume

**Goal**: Enable Ralph to survive crashes/restarts without losing loop state.

**Success Criteria**:
- Full loop state serialized on graceful shutdown and SIGTERM
- State restored and validated on startup
- Corrupted state files handled gracefully (fallback to fresh start)
- Zero data loss on `Ctrl+C` interruption

### 1. Phase 21.1: Session State Domain Model

**Description**: Define the unified session state structure that captures all recoverable state.

**Requirements**:
- [ ] Create `src/session/mod.rs` module with `SessionState` struct
- [ ] `SessionState` must include: `LoopState`, `TaskTracker` snapshot, `Supervisor` state, `StagnationPredictor` history, session metadata (version, timestamp, pid)
- [ ] Implement `Serialize`/`Deserialize` for `SessionState`
- [ ] Add version field for forward compatibility (reject incompatible versions gracefully)
- [ ] Write unit tests for serialization round-trip

**Test-First Requirements**:
```rust
// Tests to write BEFORE implementation:
// - test_session_state_serialization_roundtrip
// - test_session_state_version_compatibility
// - test_session_state_includes_all_components
// - test_session_state_default_is_empty
```

**Quality Gates**:
```bash
cargo test --lib session
cargo clippy --all-targets -- -D warnings
```

### 2. Phase 21.2: Session Persistence Layer

**Description**: Implement atomic file-based persistence with corruption protection.

**Requirements**:
- [ ] Create `src/session/persistence.rs` with `SessionPersistence` struct
- [ ] Implement atomic write: write to `.ralph/session.json.tmp`, then rename (prevents corruption)
- [ ] Implement `save(&self, state: &SessionState) -> Result<()>`
- [ ] Implement `load(&self) -> Result<Option<SessionState>>` (returns None if no file or corrupted)
- [ ] Add file locking to prevent concurrent Ralph instances corrupting state
- [ ] Log warnings (not errors) for corrupted files, delete and continue
- [ ] Write integration tests with tempdir

**Test-First Requirements**:
```rust
// Tests to write BEFORE implementation:
// - test_persistence_save_creates_file
// - test_persistence_load_returns_none_when_missing
// - test_persistence_atomic_write_survives_crash (simulate by checking tmp file handling)
// - test_persistence_corrupted_file_returns_none_and_logs
// - test_persistence_incompatible_version_returns_none
// - test_persistence_file_locking_prevents_concurrent_access
```

**Quality Gates**:
```bash
cargo test --lib session::persistence
cargo clippy --all-targets -- -D warnings
```

### 3. Phase 21.3: Signal Handler Integration

**Description**: Save state on SIGTERM, SIGINT, and graceful shutdown.

**Requirements**:
- [ ] Create `src/session/signals.rs` with signal handling logic
- [ ] Register handlers for SIGTERM and SIGINT (Unix) / CTRL_C_EVENT (Windows)
- [ ] On signal: save session state, then exit gracefully
- [ ] Add `--no-persist` CLI flag to disable persistence (for testing)
- [ ] Ensure signal handler doesn't panic (catch and log errors)
- [ ] Write tests using signal simulation where possible

**Test-First Requirements**:
```rust
// Tests to write BEFORE implementation:
// - test_signal_handler_registration
// - test_graceful_shutdown_saves_state
// - test_no_persist_flag_skips_save
// - test_signal_handler_error_doesnt_panic
```

**Quality Gates**:
```bash
cargo test --lib session::signals
cargo clippy --all-targets -- -D warnings
```

### 4. Phase 21.4: LoopManager Integration

**Description**: Integrate session persistence into the main loop lifecycle.

**Requirements**:
- [ ] Add `SessionPersistence` to `LoopManager` struct
- [ ] Call `persistence.load()` in `LoopManager::new()` to restore state
- [ ] Call `persistence.save()` after each iteration (debounced - max once per 30s)
- [ ] Call `persistence.save()` in `LoopManager::shutdown()` (unconditional)
- [ ] Add `--resume` CLI flag (default true) to control whether to load previous session
- [ ] Add `--fresh` CLI flag as alias for `--resume=false`
- [ ] Log session restoration: "Resuming session from <timestamp>, iteration <n>"
- [ ] Write integration tests with full loop lifecycle

**Test-First Requirements**:
```rust
// Tests to write BEFORE implementation:
// - test_loop_manager_loads_session_on_start
// - test_loop_manager_saves_session_after_iteration
// - test_loop_manager_saves_session_on_shutdown
// - test_fresh_flag_ignores_existing_session
// - test_resume_flag_restores_session
// - test_debounced_save_respects_interval
```

**Quality Gates**:
```bash
cargo test --lib loop::manager
cargo test --test integration_session  # New integration test file
cargo clippy --all-targets -- -D warnings
```

### 5. Phase 21.5: Documentation & CLI Help

**Description**: Document the session persistence feature.

**Requirements**:
- [ ] Add module-level documentation to `src/session/mod.rs`
- [ ] Update README.md with session persistence section
- [ ] Update CLAUDE.md with session recovery notes
- [ ] Update CLI help text for `--resume`, `--fresh`, `--no-persist` flags
- [ ] Add example usage in docs

**Quality Gates**:
```bash
cargo doc --no-deps
cargo test --doc
```

---

## Sprint 22: File Decomposition

**Goal**: Split oversized files into maintainable modules without changing public API.

**Success Criteria**:
- All files under 1,500 lines
- No public API changes (all re-exports preserved)
- All existing tests pass without modification
- No new clippy warnings

### 1. Phase 22.1: Extract `config` Submodules

**Description**: Split `src/config.rs` (3,362 lines) into focused submodules.

**Target Structure**:
```
src/config/
├── mod.rs           # Re-exports, SharedConfig struct (~200 lines)
├── resolution.rs    # SharedConfigResolver, inheritance logic (~800 lines)
├── validation.rs    # ConfigValidator, error types (~600 lines)
├── git.rs           # Git config detection (user.name, user.email) (~400 lines)
└── templates.rs     # Template path resolution (~400 lines)
```

**Requirements**:
- [ ] Create `src/config/` directory structure
- [ ] Move `SharedConfigResolver` and related types to `resolution.rs`
- [ ] Move `ConfigValidator` and validation logic to `validation.rs`
- [ ] Move git config detection to `git.rs`
- [ ] Move template path resolution to `templates.rs`
- [ ] Keep `SharedConfig` struct in `mod.rs` with re-exports
- [ ] Ensure all `pub use` statements maintain API compatibility
- [ ] Move tests to appropriate submodules

**Test-First Requirements**:
```rust
// Run BEFORE any changes to establish baseline:
cargo test --lib config -- --nocapture > /tmp/config_tests_before.txt

// After refactoring, diff output must be identical (except timing)
```

**Quality Gates**:
```bash
cargo test --lib config
cargo clippy --all-targets -- -D warnings
# Verify no public API changes:
cargo doc --no-deps 2>&1 | grep -i "warning.*config" && exit 1 || true
```

### 2. Phase 22.2: Extract `analytics` Submodules

**Description**: Split `src/analytics.rs` (4,048 lines) into focused submodules.

**Target Structure**:
```
src/analytics/
├── mod.rs           # Re-exports, Analytics struct (~300 lines)
├── events.rs        # Event types, EventBuilder (~800 lines)
├── session.rs       # SessionMetrics, session tracking (~600 lines)
├── trends.rs        # Trend analysis, pattern detection (~700 lines)
├── storage.rs       # JSONL file I/O, tamper detection (~500 lines)
└── reporting.rs     # Report generation, formatting (~500 lines)
```

**Requirements**:
- [ ] Create `src/analytics/` directory structure
- [ ] Move event types and builders to `events.rs`
- [ ] Move session metrics to `session.rs`
- [ ] Move trend analysis to `trends.rs`
- [ ] Move JSONL storage to `storage.rs`
- [ ] Move reporting to `reporting.rs`
- [ ] Maintain all `pub use` for API compatibility
- [ ] Move tests to appropriate submodules

**Test-First Requirements**:
```rust
// Run BEFORE any changes to establish baseline:
cargo test --lib analytics -- --nocapture > /tmp/analytics_tests_before.txt
```

**Quality Gates**:
```bash
cargo test --lib analytics
cargo clippy --all-targets -- -D warnings
```

### 3. Phase 22.3: Extract `task_tracker` Submodules

**Description**: Further split `src/loop/task_tracker/mod.rs` (3,314 lines).

**Target Structure**:
```
src/loop/task_tracker/
├── mod.rs           # Re-exports, TaskTracker struct (~500 lines)
├── parsing.rs       # Already exists - keep as is
├── persistence.rs   # Already exists - keep as is
├── state.rs         # Already exists - keep as is
├── selection.rs     # Task selection logic (~400 lines) [NEW]
├── orphan.rs        # Orphan detection logic (~300 lines) [NEW]
└── metrics.rs       # TaskMetrics, statistics (~300 lines) [NEW]
```

**Requirements**:
- [ ] Extract task selection logic to `selection.rs`
- [ ] Extract orphan detection to `orphan.rs`
- [ ] Extract metrics/statistics to `metrics.rs`
- [ ] Maintain API compatibility via re-exports
- [ ] Move relevant tests to new submodules

**Quality Gates**:
```bash
cargo test --lib loop::task_tracker
cargo clippy --all-targets -- -D warnings
```

### 4. Phase 22.4: Extract `OutputParser` Trait

**Description**: Consolidate the repeated parse_output pattern (182 occurrences) into a trait.

**Requirements**:
- [ ] Create `src/quality/parser.rs` with `OutputParser` trait
- [ ] Define trait methods: `parse_json()`, `parse_text()`, `parse_lines()`
- [ ] Implement `OutputParser` for each gate type (Rust, Python, TypeScript, Go)
- [ ] Refactor existing parse_output functions to use trait
- [ ] Add default implementations where patterns are identical
- [ ] Ensure no code duplication remains

**Test-First Requirements**:
```rust
// Tests to write BEFORE implementation:
// - test_output_parser_trait_parse_json
// - test_output_parser_trait_parse_text
// - test_rust_gate_implements_output_parser
// - test_python_gate_implements_output_parser
```

**Quality Gates**:
```bash
cargo test --lib quality
cargo clippy --all-targets -- -D warnings
```

### 5. Phase 22.5: Extract `checkpoint` Submodules

**Description**: Split `src/checkpoint/mod.rs` (3,083 lines).

**Target Structure**:
```
src/checkpoint/
├── mod.rs           # Re-exports, Checkpoint struct (~400 lines)
├── verification.rs  # Checkpoint verification (~500 lines) [NEW]
├── diff.rs          # Diff generation and analysis (~500 lines) [NEW]
├── storage.rs       # Checkpoint file I/O (~400 lines) [NEW]
└── rollback.rs      # Rollback operations (~400 lines) [NEW]
```

**Requirements**:
- [ ] Extract verification logic to `verification.rs`
- [ ] Extract diff logic to `diff.rs`
- [ ] Extract storage to `storage.rs`
- [ ] Extract rollback to `rollback.rs`
- [ ] Maintain API compatibility

**Quality Gates**:
```bash
cargo test --lib checkpoint
cargo clippy --all-targets -- -D warnings
```

---

## Sprint 23: LLM Provider Abstraction

**Goal**: Complete the LLM abstraction layer to support multiple providers with fallback.

**Success Criteria**:
- `--model` flag works for claude, openai, gemini, ollama
- Automatic fallback on rate limit or timeout
- Provider-specific prompt formatting handled transparently
- Cost tracking per provider

### 1. Phase 23.1: LLM Client Trait Refinement

**Description**: Refine the `LlmClient` trait for multi-provider support.

**Requirements**:
- [x] Create/update `src/llm/mod.rs` with refined `LlmClient` trait
- [x] Add methods: `complete()`, `complete_with_retry()`, `available()`, `cost_per_token()`
- [x] Add `ProviderCapabilities` struct (supports_streaming, max_context, etc.)
- [x] Add `LlmResponse` struct with token counts, latency, cost
- [x] Write comprehensive trait documentation

**Test-First Requirements**:
```rust
// Tests to write BEFORE implementation:
// - test_llm_client_trait_complete
// - test_llm_response_includes_token_counts
// - test_provider_capabilities_defaults
```

**Quality Gates**:
```bash
cargo test --lib llm
cargo clippy --all-targets -- -D warnings
```

### 2. Phase 23.2: Claude Provider (Refactor Existing)

**Description**: Refactor existing Claude integration to implement the new trait.

**Requirements**:
- [ ] Create `src/llm/claude.rs` implementing `LlmClient`
- [ ] Move existing Claude-specific code from scattered locations
- [ ] Implement proper error handling for API errors
- [ ] Add rate limit detection and backoff
- [ ] Support both `claude-sonnet-4-20250514` and `claude-opus-4-5-20251101`

**Test-First Requirements**:
```rust
// Tests to write BEFORE implementation:
// - test_claude_provider_implements_llm_client
// - test_claude_rate_limit_detection
// - test_claude_model_selection
```

**Quality Gates**:
```bash
cargo test --lib llm::claude
cargo clippy --all-targets -- -D warnings
```

### 3. Phase 23.3: Ollama Provider

**Description**: Implement Ollama provider for local/free inference.

**Requirements**:
- [ ] Create `src/llm/ollama.rs` implementing `LlmClient`
- [ ] Auto-detect Ollama availability via `ollama list`
- [ ] Support common models: llama3, codellama, mistral, deepseek-coder
- [ ] Handle connection errors gracefully (Ollama not running)
- [ ] Implement streaming for long responses

**Test-First Requirements**:
```rust
// Tests to write BEFORE implementation:
// - test_ollama_provider_implements_llm_client
// - test_ollama_availability_detection
// - test_ollama_graceful_degradation_when_unavailable
```

**Quality Gates**:
```bash
cargo test --lib llm::ollama
cargo clippy --all-targets -- -D warnings
```

### 4. Phase 23.4: OpenAI Provider

**Description**: Implement OpenAI provider.

**Requirements**:
- [ ] Create `src/llm/openai.rs` implementing `LlmClient`
- [ ] Support GPT-4o and GPT-4-turbo models
- [ ] Handle API key from environment (`OPENAI_API_KEY`)
- [ ] Implement proper error handling for API errors
- [ ] Add rate limit detection and backoff

**Test-First Requirements**:
```rust
// Tests to write BEFORE implementation:
// - test_openai_provider_implements_llm_client
// - test_openai_api_key_from_env
// - test_openai_rate_limit_detection
```

**Quality Gates**:
```bash
cargo test --lib llm::openai
cargo clippy --all-targets -- -D warnings
```

### 5. Phase 23.5: Provider Router & Fallback

**Description**: Implement provider selection and automatic fallback.

**Requirements**:
- [ ] Create `src/llm/router.rs` with `ProviderRouter` struct
- [ ] Implement `--model` CLI flag: `claude`, `openai`, `gemini`, `ollama`, `auto`
- [ ] `auto` mode: try providers in order of preference until one succeeds
- [ ] Implement fallback on: rate limit, timeout, connection error
- [ ] Log provider switches: "Falling back from Claude to Ollama: rate limited"
- [ ] Add `--no-fallback` flag to disable automatic fallback

**Test-First Requirements**:
```rust
// Tests to write BEFORE implementation:
// - test_provider_router_selects_requested_provider
// - test_provider_router_auto_mode_tries_in_order
// - test_provider_router_fallback_on_rate_limit
// - test_provider_router_no_fallback_flag
```

**Quality Gates**:
```bash
cargo test --lib llm::router
cargo test --lib llm
cargo clippy --all-targets -- -D warnings
```

### 6. Phase 23.6: Cost Tracking

**Description**: Track LLM costs across providers.

**Requirements**:
- [ ] Add `CostTracker` to analytics module
- [ ] Track tokens in/out per provider per session
- [ ] Calculate estimated cost based on provider pricing
- [ ] Add `ralph stats` command to show cumulative costs
- [ ] Persist cost data in `.ralph/costs.json`

**Test-First Requirements**:
```rust
// Tests to write BEFORE implementation:
// - test_cost_tracker_records_tokens
// - test_cost_tracker_calculates_cost_per_provider
// - test_cost_tracker_persistence
```

**Quality Gates**:
```bash
cargo test --lib llm
cargo test --lib analytics
cargo clippy --all-targets -- -D warnings
```

---

## Sprint 24: Predictor Persistence & Diagnostics Enhancement

**Goal**: Enable cross-session learning and faster human intervention.

**Success Criteria**:
- Predictor accuracy stats persist across sessions
- Diagnostic reports include predictor breakdown
- Accuracy improves over time via adaptive weighting (optional)

### 1. Phase 24.1: Predictor Stats Persistence

**Description**: Persist stagnation predictor accuracy statistics.

**Requirements**:
- [ ] Create `src/stagnation/persistence.rs`
- [ ] Define `PredictorStats` struct: predictions made, accuracy by risk level, factor weights
- [ ] Implement save/load to `.ralph/predictor_stats.json`
- [ ] Load stats in `StagnationPredictor::new()`
- [ ] Save stats after each prediction verification
- [ ] Add stats summary to `ralph status` command

**Test-First Requirements**:
```rust
// Tests to write BEFORE implementation:
// - test_predictor_stats_serialization
// - test_predictor_stats_persistence_roundtrip
// - test_predictor_loads_stats_on_init
// - test_predictor_saves_stats_after_verification
```

**Quality Gates**:
```bash
cargo test --lib stagnation
cargo clippy --all-targets -- -D warnings
```

### 2. Phase 24.2: Enhanced Diagnostic Reports

**Description**: Include predictor breakdown in supervisor diagnostic reports.

**Requirements**:
- [ ] Add `predictor_summary` field to `DiagnosticReport`
- [ ] Include: current risk level, factor breakdown, recent predictions, accuracy stats
- [ ] Add `preventive_actions_taken` field (what Ralph already tried)
- [ ] Format predictor data in human-readable summary
- [ ] Ensure diagnostic report is self-contained (no external lookups needed)

**Test-First Requirements**:
```rust
// Tests to write BEFORE implementation:
// - test_diagnostic_report_includes_predictor_summary
// - test_diagnostic_report_includes_preventive_actions
// - test_diagnostic_report_human_readable_format
```

**Quality Gates**:
```bash
cargo test --lib supervisor
cargo clippy --all-targets -- -D warnings
```

### 3. Phase 24.3: Adaptive Weight Tuning (Optional)

**Description**: Slowly adjust predictor weights based on recorded accuracy.

**Requirements**:
- [ ] Add `enable_adaptive_weights` config option (default: false)
- [ ] Track which factors contributed to correct vs incorrect predictions
- [ ] Implement simple weight adjustment: +0.1 for factors in correct predictions, -0.1 for incorrect
- [ ] Clamp weights to [0.1, 2.0] range to prevent runaway
- [ ] Add `ralph predictor tune` command to manually trigger tuning
- [ ] Log weight changes

**Test-First Requirements**:
```rust
// Tests to write BEFORE implementation:
// - test_adaptive_weights_increase_on_correct_prediction
// - test_adaptive_weights_decrease_on_incorrect
// - test_adaptive_weights_clamped_to_range
// - test_adaptive_weights_disabled_by_default
```

**Quality Gates**:
```bash
cargo test --lib stagnation
cargo clippy --all-targets -- -D warnings
```

---

## Sprint 25: Analytics Dashboard

**Goal**: Generate human-readable HTML reports from analytics data.

**Success Criteria**:
- `ralph analytics dashboard` generates standalone HTML file
- Dashboard shows: session timeline, quality trends, stagnation events, costs
- Works offline (no external dependencies in HTML)

### 1. Phase 25.1: Dashboard Data Aggregation

**Description**: Aggregate analytics data into dashboard-ready format.

**Requirements**:
- [ ] Create `src/analytics/dashboard/mod.rs`
- [ ] Define `DashboardData` struct: sessions, events, trends, summary
- [ ] Implement aggregation from JSONL event stream
- [ ] Support time range filtering: last N sessions, date range
- [ ] Calculate summary statistics: total iterations, success rate, avg session duration

**Test-First Requirements**:
```rust
// Tests to write BEFORE implementation:
// - test_dashboard_data_aggregation_from_events
// - test_dashboard_data_time_filtering
// - test_dashboard_data_summary_statistics
```

**Quality Gates**:
```bash
cargo test --lib analytics::dashboard
cargo clippy --all-targets -- -D warnings
```

### 2. Phase 25.2: HTML Template Engine

**Description**: Simple HTML template rendering for dashboard.

**Requirements**:
- [ ] Create `src/analytics/dashboard/template.rs`
- [ ] Embed HTML template as const string (no external files)
- [ ] Use simple string substitution for data injection
- [ ] Include inline CSS (Tailwind-like utility classes)
- [ ] Include inline JavaScript for interactivity (collapsible sections, tooltips)
- [ ] No external CDN dependencies (fully offline)

**Test-First Requirements**:
```rust
// Tests to write BEFORE implementation:
// - test_template_renders_valid_html
// - test_template_substitutes_data
// - test_template_has_no_external_dependencies
```

**Quality Gates**:
```bash
cargo test --lib analytics::dashboard::template
cargo clippy --all-targets -- -D warnings
```

### 3. Phase 25.3: Chart Generation

**Description**: Generate SVG charts for visual trends.

**Requirements**:
- [ ] Create `src/analytics/dashboard/charts.rs`
- [ ] Implement simple line chart SVG generator (iterations over time)
- [ ] Implement bar chart SVG generator (quality gate pass/fail)
- [ ] Implement pie chart SVG generator (time distribution by phase)
- [ ] All charts as inline SVG (no external libraries)

**Test-First Requirements**:
```rust
// Tests to write BEFORE implementation:
// - test_line_chart_generates_valid_svg
// - test_bar_chart_generates_valid_svg
// - test_charts_handle_empty_data
```

**Quality Gates**:
```bash
cargo test --lib analytics::dashboard::charts
cargo clippy --all-targets -- -D warnings
```

### 4. Phase 25.4: CLI Command Integration

**Description**: Add `ralph analytics dashboard` command.

**Requirements**:
- [ ] Add `dashboard` subcommand to `ralph analytics`
- [ ] Options: `--output <path>` (default: `.ralph/dashboard.html`), `--sessions <n>`, `--open` (open in browser)
- [ ] Generate and save HTML file
- [ ] Print path to generated file
- [ ] Optionally open in default browser

**Test-First Requirements**:
```rust
// Tests to write BEFORE implementation:
// - test_dashboard_command_creates_file
// - test_dashboard_command_respects_output_path
// - test_dashboard_command_filters_sessions
```

**Quality Gates**:
```bash
cargo test --lib cli
cargo test --test integration_dashboard
cargo clippy --all-targets -- -D warnings
```

---

## Sprint 26: Incremental Execution Mode

**Goal**: Run Ralph on changed files only for large codebase support.

**Success Criteria**:
- `--changed-since <commit>` runs gates only on changed files
- `--files <glob>` explicitly specifies files to process
- Task selection prioritizes tasks affecting changed files
- 10x+ speedup on large repos with small changes

### 1. Phase 26.1: Change Detection

**Description**: Detect changed files since a commit or in working tree.

**Requirements**:
- [ ] Create `src/changes/mod.rs` with `ChangeDetector` struct
- [ ] Implement `changed_since(commit: &str) -> Result<Vec<PathBuf>>`
- [ ] Implement `changed_in_working_tree() -> Result<Vec<PathBuf>>`
- [ ] Support filtering by file extension / glob pattern
- [ ] Handle renamed files correctly

**Test-First Requirements**:
```rust
// Tests to write BEFORE implementation:
// - test_change_detector_finds_modified_files
// - test_change_detector_finds_added_files
// - test_change_detector_handles_renames
// - test_change_detector_filters_by_extension
```

**Quality Gates**:
```bash
cargo test --lib changes
cargo clippy --all-targets -- -D warnings
```

### 2. Phase 26.2: Scoped Quality Gates

**Description**: Run quality gates on a subset of files.

**Requirements**:
- [ ] Add `files: Option<Vec<PathBuf>>` parameter to gate runners
- [ ] When `files` is Some, only process those files
- [ ] Maintain existing behavior when `files` is None (process all)
- [ ] Update all gate implementations (Rust, Python, TypeScript, Go)

**Test-First Requirements**:
```rust
// Tests to write BEFORE implementation:
// - test_rust_gate_scoped_to_files
// - test_python_gate_scoped_to_files
// - test_gate_processes_all_when_unscoped
```

**Quality Gates**:
```bash
cargo test --lib quality::gates
cargo clippy --all-targets -- -D warnings
```

### 3. Phase 26.3: Scoped Context Building

**Description**: Build context from changed files + CCG neighbors.

**Requirements**:
- [ ] Add `scope: Option<ChangeScope>` to context builder
- [ ] When scoped: include changed files + their CCG neighbors (call graph)
- [ ] Use narsil-mcp `get_call_graph` to find related functions
- [ ] Graceful degradation when narsil-mcp unavailable (just use changed files)

**Test-First Requirements**:
```rust
// Tests to write BEFORE implementation:
// - test_context_builder_scoped_includes_changed_files
// - test_context_builder_scoped_includes_ccg_neighbors
// - test_context_builder_scoped_degrades_without_narsil
```

**Quality Gates**:
```bash
cargo test --lib context
cargo clippy --all-targets -- -D warnings
```

### 4. Phase 26.4: Scoped Task Selection

**Description**: Prioritize tasks that affect changed files.

**Requirements**:
- [ ] Add `affected_files: Option<Vec<PathBuf>>` to Task struct
- [ ] Parse affected files from task descriptions (if mentioned)
- [ ] When running scoped: prioritize tasks whose affected files overlap with changed files
- [ ] De-prioritize (but don't skip) unrelated tasks

**Test-First Requirements**:
```rust
// Tests to write BEFORE implementation:
// - test_task_selection_prioritizes_affected_tasks
// - test_task_selection_includes_unrelated_tasks_at_lower_priority
```

**Quality Gates**:
```bash
cargo test --lib loop::task_tracker
cargo clippy --all-targets -- -D warnings
```

### 5. Phase 26.5: CLI Integration

**Description**: Add incremental execution flags to CLI.

**Requirements**:
- [ ] Add `--changed-since <commit>` flag to `ralph loop`
- [ ] Add `--files <glob>` flag to `ralph loop`
- [ ] Add `--changed` flag as shorthand for `--changed-since HEAD~1`
- [ ] Flags are mutually exclusive (error if both specified)
- [ ] Log scope at start: "Running in incremental mode: 5 files changed since abc123"

**Test-First Requirements**:
```rust
// Tests to write BEFORE implementation:
// - test_cli_changed_since_flag
// - test_cli_files_flag
// - test_cli_flags_mutually_exclusive
```

**Quality Gates**:
```bash
cargo test --lib cli
cargo test --test integration_incremental
cargo clippy --all-targets -- -D warnings
```

---

## Global Quality Gates

Before ANY commit in ANY sprint:

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

---

## Progress Tracking

### Sprint 21: Session Persistence
- [ ] Phase 21.1: Session State Domain Model
- [ ] Phase 21.2: Session Persistence Layer
- [ ] Phase 21.3: Signal Handler Integration
- [ ] Phase 21.4: LoopManager Integration
- [ ] Phase 21.5: Documentation & CLI Help

### Sprint 22: File Decomposition
- [ ] Phase 22.1: Extract `config` Submodules
- [ ] Phase 22.2: Extract `analytics` Submodules
- [ ] Phase 22.3: Extract `task_tracker` Submodules
- [ ] Phase 22.4: Extract `OutputParser` Trait
- [ ] Phase 22.5: Extract `checkpoint` Submodules

### Sprint 23: LLM Provider Abstraction
- [x] Phase 23.1: LLM Client Trait Refinement
- [ ] Phase 23.2: Claude Provider
- [ ] Phase 23.3: Ollama Provider
- [ ] Phase 23.4: OpenAI Provider
- [ ] Phase 23.5: Provider Router & Fallback
- [ ] Phase 23.6: Cost Tracking

### Sprint 24: Predictor Persistence & Diagnostics
- [ ] Phase 24.1: Predictor Stats Persistence
- [ ] Phase 24.2: Enhanced Diagnostic Reports
- [ ] Phase 24.3: Adaptive Weight Tuning

### Sprint 25: Analytics Dashboard
- [ ] Phase 25.1: Dashboard Data Aggregation
- [ ] Phase 25.2: HTML Template Engine
- [ ] Phase 25.3: Chart Generation
- [ ] Phase 25.4: CLI Command Integration

### Sprint 26: Incremental Execution
- [ ] Phase 26.1: Change Detection
- [ ] Phase 26.2: Scoped Quality Gates
- [ ] Phase 26.3: Scoped Context Building
- [ ] Phase 26.4: Scoped Task Selection
- [ ] Phase 26.5: CLI Integration

---

## Completion Criteria

This plan is complete when:
1. All checkboxes are marked
2. All quality gates pass
3. `cargo test` shows 2,500+ tests passing (current: 2,195)
4. Documentation is updated
5. CHANGELOG.md reflects all changes
