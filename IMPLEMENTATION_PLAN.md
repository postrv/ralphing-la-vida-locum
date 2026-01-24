# Ralph Self-Improvement Implementation Plan

> **Meta-Development**: This plan will be executed by Ralph to improve Ralph.
> **Methodology**: Strict TDD - Write failing test → Minimal implementation → Refactor
> **Quality Standard**: Production-grade, zero warnings, comprehensive documentation

---

## Overview

| Sprint | Focus | Effort | Status |
|--------|-------|--------|--------|
| 23 | LLM Provider Abstraction | 2-3 days | **Current** |
| 24 | Predictor Persistence & Diagnostics | 1-2 days | Pending |
| 25 | Analytics Dashboard | 1-2 days | Pending |
| 26 | Incremental Execution Mode | 2-3 days | Pending |

**Completed Sprints**: See `docs/COMPLETED_SPRINTS.md`

**Current Test Count**: 1,777 passing

---

## Sprint 23: LLM Provider Abstraction

**Goal**: Complete the LLM abstraction layer to support multiple providers with fallback.

**Success Criteria**:
- `--model` flag works for claude, openai, gemini, ollama
- Automatic fallback on rate limit or timeout
- Provider-specific prompt formatting handled transparently
- Cost tracking per provider

**Already Complete**:
- [x] Phase 23.1: LLM Client Trait Refinement (`src/llm/mod.rs`)
- [x] Phase 23.2: Claude Provider (`src/llm/claude.rs`)

### Phase 23.2: Claude Provider (Refactor Existing) ✅

**Description**: Refactor existing Claude integration to implement the new trait.

**Requirements**:
- [x] Create `src/llm/claude.rs` implementing `LlmClient`
- [x] Move existing Claude-specific code from scattered locations
- [x] Implement proper error handling for API errors
- [x] Add rate limit detection and backoff
- [x] Support both `claude-sonnet-4-20250514` and `claude-opus-4-5-20251101`

**Test-First**:
```rust
// - test_claude_provider_implements_llm_client ✅
// - test_claude_rate_limit_detection ✅
// - test_claude_model_selection ✅
```

### Phase 23.3: Ollama Provider ✅

**Description**: Implement Ollama provider for local/free inference.

**Requirements**:
- [x] Create `src/llm/ollama.rs` implementing `LlmClient`
- [x] Auto-detect Ollama availability via `ollama list`
- [x] Support common models: llama3, codellama, mistral, deepseek-coder
- [x] Handle connection errors gracefully (Ollama not running)
- [ ] Implement streaming for long responses (future: HTTP API streaming)

**Test-First**:
```rust
// - test_ollama_provider_implements_llm_client ✅
// - test_ollama_availability_detection ✅
// - test_ollama_graceful_degradation_when_unavailable ✅
```

### Phase 23.4: OpenAI Provider

**Description**: Implement OpenAI provider.

**Requirements**:
- [ ] Create `src/llm/openai.rs` implementing `LlmClient`
- [ ] Support GPT-4o and GPT-4-turbo models
- [ ] Handle API key from environment (`OPENAI_API_KEY`)
- [ ] Implement proper error handling for API errors
- [ ] Add rate limit detection and backoff

**Test-First**:
```rust
// - test_openai_provider_implements_llm_client
// - test_openai_api_key_from_env
// - test_openai_rate_limit_detection
```

### Phase 23.5: Provider Router & Fallback

**Description**: Implement provider selection and automatic fallback.

**Requirements**:
- [ ] Create `src/llm/router.rs` with `ProviderRouter` struct
- [ ] Implement `--model` CLI flag: `claude`, `openai`, `gemini`, `ollama`, `auto`
- [ ] `auto` mode: try providers in order of preference until one succeeds
- [ ] Implement fallback on: rate limit, timeout, connection error
- [ ] Log provider switches: "Falling back from Claude to Ollama: rate limited"
- [ ] Add `--no-fallback` flag to disable automatic fallback

**Test-First**:
```rust
// - test_provider_router_selects_requested_provider
// - test_provider_router_auto_mode_tries_in_order
// - test_provider_router_fallback_on_rate_limit
// - test_provider_router_no_fallback_flag
```

### Phase 23.6: Cost Tracking

**Description**: Track LLM costs across providers.

**Requirements**:
- [ ] Add `CostTracker` to analytics module
- [ ] Track tokens in/out per provider per session
- [ ] Calculate estimated cost based on provider pricing
- [ ] Add `ralph stats` command to show cumulative costs
- [ ] Persist cost data in `.ralph/costs.json`

**Test-First**:
```rust
// - test_cost_tracker_records_tokens
// - test_cost_tracker_calculates_cost_per_provider
// - test_cost_tracker_persistence
```

---

## Sprint 24: Predictor Persistence & Diagnostics Enhancement

**Goal**: Enable cross-session learning and faster human intervention.

**Success Criteria**:
- Predictor accuracy stats persist across sessions
- Diagnostic reports include predictor breakdown
- Accuracy improves over time via adaptive weighting (optional)

**Already Complete**:
- [x] Phase 24.1: Core persistence module (`src/stagnation/persistence.rs`)

### Phase 24.1: Predictor Stats Integration (Remaining)

**Description**: Integrate predictor stats persistence with StagnationPredictor.

**Requirements**:
- [ ] Load stats in `StagnationPredictor::new()`
- [ ] Save stats after each prediction verification
- [ ] Add stats summary to `ralph status` command

### Phase 24.2: Enhanced Diagnostic Reports

**Description**: Include predictor breakdown in supervisor diagnostic reports.

**Requirements**:
- [ ] Add `predictor_summary` field to `DiagnosticReport`
- [ ] Include: current risk level, factor breakdown, recent predictions, accuracy stats
- [ ] Add `preventive_actions_taken` field (what Ralph already tried)
- [ ] Format predictor data in human-readable summary
- [ ] Ensure diagnostic report is self-contained (no external lookups needed)

**Test-First**:
```rust
// - test_diagnostic_report_includes_predictor_summary
// - test_diagnostic_report_includes_preventive_actions
// - test_diagnostic_report_human_readable_format
```

### Phase 24.3: Adaptive Weight Tuning (Optional)

**Description**: Slowly adjust predictor weights based on recorded accuracy.

**Requirements**:
- [ ] Add `enable_adaptive_weights` config option (default: false)
- [ ] Track which factors contributed to correct vs incorrect predictions
- [ ] Implement simple weight adjustment: +0.1 for factors in correct predictions, -0.1 for incorrect
- [ ] Clamp weights to [0.1, 2.0] range to prevent runaway
- [ ] Add `ralph predictor tune` command to manually trigger tuning
- [ ] Log weight changes

---

## Sprint 25: Analytics Dashboard

**Goal**: Generate human-readable HTML reports from analytics data.

**Success Criteria**:
- `ralph analytics dashboard` generates standalone HTML file
- Dashboard shows: session timeline, quality trends, stagnation events, costs
- Works offline (no external dependencies in HTML)

**Already Complete**:
- [x] Phase 25.1: Dashboard Data Aggregation (`src/analytics/dashboard/mod.rs`)

### Phase 25.2: HTML Template Engine

**Description**: Simple HTML template rendering for dashboard.

**Requirements**:
- [ ] Create `src/analytics/dashboard/template.rs`
- [ ] Embed HTML template as const string (no external files)
- [ ] Use simple string substitution for data injection
- [ ] Include inline CSS (Tailwind-like utility classes)
- [ ] Include inline JavaScript for interactivity (collapsible sections, tooltips)
- [ ] No external CDN dependencies (fully offline)

**Test-First**:
```rust
// - test_template_renders_valid_html
// - test_template_substitutes_data
// - test_template_has_no_external_dependencies
```

### Phase 25.3: Chart Generation

**Description**: Generate SVG charts for visual trends.

**Requirements**:
- [ ] Create `src/analytics/dashboard/charts.rs`
- [ ] Implement simple line chart SVG generator (iterations over time)
- [ ] Implement bar chart SVG generator (quality gate pass/fail)
- [ ] Implement pie chart SVG generator (time distribution by phase)
- [ ] All charts as inline SVG (no external libraries)

**Test-First**:
```rust
// - test_line_chart_generates_valid_svg
// - test_bar_chart_generates_valid_svg
// - test_charts_handle_empty_data
```

### Phase 25.4: CLI Command Integration

**Description**: Add `ralph analytics dashboard` command.

**Requirements**:
- [ ] Add `dashboard` subcommand to `ralph analytics`
- [ ] Options: `--output <path>` (default: `.ralph/dashboard.html`), `--sessions <n>`, `--open` (open in browser)
- [ ] Generate and save HTML file
- [ ] Print path to generated file
- [ ] Optionally open in default browser

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

### Phase 26.2: Scoped Quality Gates

**Description**: Run quality gates on a subset of files.

**Requirements**:
- [ ] Add `files: Option<Vec<PathBuf>>` parameter to gate runners
- [ ] When `files` is Some, only process those files
- [ ] Maintain existing behavior when `files` is None (process all)
- [ ] Update all gate implementations (Rust, Python, TypeScript, Go)

**Test-First**:
```rust
// - test_rust_gate_scoped_to_files
// - test_python_gate_scoped_to_files
// - test_gate_processes_all_when_unscoped
```

### Phase 26.3: Scoped Context Building

**Description**: Build context from changed files + CCG neighbors.

**Requirements**:
- [ ] Add `scope: Option<ChangeScope>` to context builder
- [ ] When scoped: include changed files + their CCG neighbors (call graph)
- [ ] Use narsil-mcp `get_call_graph` to find related functions
- [ ] Graceful degradation when narsil-mcp unavailable (just use changed files)

### Phase 26.4: Scoped Task Selection

**Description**: Prioritize tasks that affect changed files.

**Requirements**:
- [ ] Add `affected_files: Option<Vec<PathBuf>>` to Task struct
- [ ] Parse affected files from task descriptions (if mentioned)
- [ ] When running scoped: prioritize tasks whose affected files overlap with changed files
- [ ] De-prioritize (but don't skip) unrelated tasks

### Phase 26.5: CLI Integration

**Description**: Add incremental execution flags to CLI.

**Requirements**:
- [ ] Add `--changed-since <commit>` flag to `ralph loop`
- [ ] Add `--files <glob>` flag to `ralph loop`
- [ ] Add `--changed` flag as shorthand for `--changed-since HEAD~1`
- [ ] Flags are mutually exclusive (error if both specified)
- [ ] Log scope at start: "Running in incremental mode: 5 files changed since abc123"

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
