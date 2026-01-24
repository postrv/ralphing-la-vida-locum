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
| 26 | Incremental Execution Mode | 2-3 days | **Current** |

**Completed Sprints**: See `docs/COMPLETED_SPRINTS.md`

**Current Test Count**: 1,942 passing

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
- [x] Create `src/analytics/dashboard/template.rs`
- [x] Embed HTML template as const string (no external files)
- [x] Use simple string substitution for data injection
- [x] Include inline CSS (Tailwind-like utility classes)
- [x] Include inline JavaScript for interactivity (collapsible sections, tooltips)
- [x] No external CDN dependencies (fully offline)

**Test-First**:
```rust
// - test_template_renders_valid_html
// - test_template_substitutes_data
// - test_template_has_no_external_dependencies
```

### Phase 25.3: Chart Generation

**Description**: Generate SVG charts for visual trends.

**Requirements**:
- [x] Create `src/analytics/dashboard/charts.rs`
- [x] Implement simple line chart SVG generator (iterations over time)
- [x] Implement bar chart SVG generator (quality gate pass/fail)
- [x] Implement pie chart SVG generator (time distribution by phase)
- [x] All charts as inline SVG (no external libraries)

**Test-First**:
```rust
// - test_line_chart_generates_valid_svg
// - test_bar_chart_generates_valid_svg
// - test_charts_handle_empty_data
```

### Phase 25.4: CLI Command Integration

**Description**: Add `ralph analytics dashboard` command.

**Requirements**:
- [x] Add `dashboard` subcommand to `ralph analytics`
- [x] Options: `--output <path>` (default: `.ralph/dashboard.html`), `--sessions <n>`, `--open` (open in browser)
- [x] Generate and save HTML file
- [x] Print path to generated file
- [x] Optionally open in default browser

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
- [x] Add `files: Option<Vec<PathBuf>>` parameter to gate runners
- [x] When `files` is Some, only process those files
- [x] Maintain existing behavior when `files` is None (process all)
- [x] Update all gate implementations (Rust, Python, TypeScript, Go)

**Test-First**:
```rust
// - test_rust_gate_scoped_to_files ✓
// - test_python_gate_scoped_to_files ✓
// - test_gate_processes_all_when_unscoped ✓
```

### Phase 26.3: Scoped Context Building

**Description**: Build context from changed files + CCG neighbors.

**Requirements**:
- [x] Add `scope: Option<ChangeScope>` to context builder
- [x] When scoped: include changed files + their CCG neighbors (call graph)
- [x] Use narsil-mcp `get_call_graph` to find related functions
- [x] Graceful degradation when narsil-mcp unavailable (just use changed files)

**Test-First**:
```rust
// - test_change_scope_new_empty ✓
// - test_change_scope_with_files ✓
// - test_change_scope_from_detector ✓
// - test_builder_for_scope_exists ✓
// - test_builder_for_scope_graceful_degradation ✓
```

### Phase 26.4: Scoped Task Selection

**Description**: Prioritize tasks that affect changed files.

**Requirements**:
- [x] Add `affected_files: Option<Vec<PathBuf>>` to Task struct
- [x] Parse affected files from task descriptions (if mentioned)
- [x] When running scoped: prioritize tasks whose affected files overlap with changed files
- [x] De-prioritize (but don't skip) unrelated tasks

**Test-First**:
```rust
// - test_task_affected_files_default_is_none ✓
// - test_task_with_affected_files ✓
// - test_task_set_affected_files ✓
// - test_task_affects_file_when_matching ✓
// - test_task_affects_file_when_not_matching ✓
// - test_task_affects_file_when_no_affected_files ✓
// - test_task_affects_any_file_with_matches ✓
// - test_task_affects_any_file_without_matches ✓
// - test_task_affects_any_file_when_no_affected_files ✓
// - test_task_has_explicit_affected_file_match ✓
// - test_task_has_explicit_affected_file_match_returns_false_when_none ✓
// - test_select_next_task_scoped_prioritizes_matching_tasks ✓
// - test_select_next_task_scoped_falls_back_to_normal_selection ✓
// - test_select_next_task_scoped_respects_in_progress_priority ✓
// - test_select_next_task_scoped_with_empty_scope ✓
// - test_select_next_task_scoped_handles_tasks_without_affected_files ✓
// - test_extract_file_paths_from_simple_text ✓
// - test_extract_file_paths_multiple_files ✓
// - test_extract_file_paths_no_files ✓
// - test_extract_file_paths_in_backticks ✓
// - test_extract_file_paths_various_extensions ✓
// - test_parse_affected_files_from_checkboxes ✓
// - test_parse_affected_files_from_title ✓
// - test_parse_affected_files_deduplicates ✓
```

### Phase 26.5: CLI Integration

**Description**: Add incremental execution flags to CLI.

**Requirements**:
- [x] Add `--changed-since <commit>` flag to `ralph loop`
- [ ] Add `--files <glob>` flag to `ralph loop`
- [ ] Add `--changed` flag as shorthand for `--changed-since HEAD~1`
- [ ] Flags are mutually exclusive (error if both specified)
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

## Pending CLI Integration

These CLI flags were implemented in the library but need CLI wiring:

- [ ] `--model` flag: `claude`, `openai`, `gemini`, `ollama`, `auto`
- [ ] `--no-fallback` flag to disable automatic provider fallback

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
