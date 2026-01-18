# Enterprise Features Extraction Plan

**Date:** 2026-01-18
**Purpose:** Extract enterprise features to `ralph-enterprise` private repo

---

## Overview

During an autonomous loop run, Ralph implemented enterprise features from orphaned tasks. This code is production-quality (TDD, fully tested, clippy clean) but doesn't belong in the open-source Ralph repo.

---

## Features to Extract

### 1. RBAC Module (929 lines, 36 tests)

**Source:** `src/enterprise/rbac.rs`, `src/enterprise/mod.rs`
**Commit:** 4d38774

**Functionality:**
- `Role` enum with Admin, Developer, Viewer
- `Permission` enum with Read, Write, Delete, Execute, Review, Admin
- `RoleBuilder` for custom role creation
- `PermissionCheck` trait
- `can_access()` function for resource-based access control
- Full serde serialization support

**Dependencies:**
- serde, serde_json
- No internal ralph dependencies

### 2. Reporting Module (1,298 lines, 25 tests)

**Source:** `src/reporting/mod.rs`, `src/reporting/generator.rs`, `src/reporting/export.rs`
**Commit:** c7c84cd

**Functionality:**
- `ReportData` struct for aggregating quality metrics
- `ReportGenerator` for HTML dashboards with embedded CSS
- `QualityExporter` for CSV, JSON, JSONL export
- Quality score calculation (0-100)
- Session summary tables
- Trend analysis visualization

**Dependencies:**
- serde, serde_json
- chrono (for timestamps)
- No internal ralph dependencies

### 3. Quality Certification (1,072 lines, 37 tests)

**Source:** `src/reporting/certification.rs`
**Commit:** 04d1a42

**Functionality:**
- `CertificationLevel` enum (Gold/Silver/Bronze/None)
- `QualityCertification` struct with badge URL generation
- `CertificationHistory` for JSONL persistence
- `QualityCertifier` for issuing certifications
- Shields.io compatible badge URLs
- Automatic regression detection

**Dependencies:**
- serde, serde_json
- chrono
- Depends on reporting module

### 4. Audit Logging (partial analytics.rs, ~500 lines, 16 tests)

**Source:** Additions to `src/analytics.rs`
**Commit:** 6e70508

**Functionality:**
- `AuditAction` enum (14 action types)
- `AuditSeverity` enum (Info, Low, Medium, High, Critical)
- `CampaignOutcome` enum
- `AuditEvent` struct with builder pattern
- `AuditExportFormat` enum (JSON, JSONL, CSV)
- `log_audit_event()`, `log_api_action()`, `log_campaign_execution()`, `log_quality_decision()`
- `get_audit_events()` with filtering
- `export_audit_log()` with multiple formats

**Dependencies:**
- serde, serde_json
- chrono
- Depends on existing analytics types

---

## Extraction Steps

### Step 1: Create ralph-enterprise Repository

```bash
gh repo create postrv/ralph-enterprise --private --description "Enterprise features for Ralph automation suite"
cd ~/RustroverProjects
git clone https://github.com/postrv/ralph-enterprise.git
cd ralph-enterprise
```

### Step 2: Initialize Cargo Project

```bash
cargo init --lib
```

Update `Cargo.toml`:
```toml
[package]
name = "ralph-enterprise"
version = "0.1.0"
edition = "2021"
description = "Enterprise features for Ralph automation suite"
license = "Proprietary"
repository = "https://github.com/postrv/ralph-enterprise"

[dependencies]
serde = { version = "1.0", features = ["derive"] }
serde_json = "1.0"
chrono = { version = "0.4", features = ["serde"] }

[dev-dependencies]
tempfile = "3.0"
```

### Step 3: Copy Enterprise Modules

From `ralphing-la-vida-locum`:
```bash
# RBAC
mkdir -p src/enterprise
cp ../ralphing-la-vida-locum/src/enterprise/mod.rs src/enterprise/
cp ../ralphing-la-vida-locum/src/enterprise/rbac.rs src/enterprise/

# Reporting
mkdir -p src/reporting
cp ../ralphing-la-vida-locum/src/reporting/mod.rs src/reporting/
cp ../ralphing-la-vida-locum/src/reporting/generator.rs src/reporting/
cp ../ralphing-la-vida-locum/src/reporting/export.rs src/reporting/
cp ../ralphing-la-vida-locum/src/reporting/certification.rs src/reporting/
```

### Step 4: Extract Audit Logging

The audit logging is interleaved with existing analytics. Need to:
1. Copy relevant structs/enums to new `src/audit.rs`
2. Adapt to be standalone (remove ralph-specific dependencies)

### Step 5: Update lib.rs

```rust
pub mod enterprise;
pub mod reporting;
pub mod audit;

pub use enterprise::rbac;
pub use reporting::{ReportData, ReportGenerator, QualityExporter};
pub use reporting::certification::{CertificationLevel, QualityCertification, QualityCertifier};
pub use audit::{AuditAction, AuditEvent, AuditSeverity};
```

### Step 6: Verify Tests Pass

```bash
cargo test
cargo clippy --all-targets -- -D warnings
```

### Step 7: Initial Commit

```bash
git add -A
git commit -m "feat: Initial extraction of enterprise features from ralph

Extracted from ralphing-la-vida-locum commits:
- 4d38774: RBAC module (929 lines, 36 tests)
- c7c84cd: Reporting module (1,298 lines, 25 tests)
- 04d1a42: Quality certification (1,072 lines, 37 tests)
- 6e70508: Audit logging (~500 lines, 16 tests)

Total: 5,211 lines, 114 tests

Co-Authored-By: Claude Opus 4.5 <noreply@anthropic.com>"
git push origin main
```

---

## Revert Strategy for ralphing-la-vida-locum

After extraction, revert enterprise commits from main repo.

### Commits to Revert (in reverse order)

| Commit | Message | Action |
|--------|---------|--------|
| 32cbaf7 | can_commit() tests | KEEP (legitimate) |
| 2fabdbf | CCG constraints | DECIDE (Sprint 11 early) |
| 18cd9c2 | doctest fix | KEEP (legitimate fix) |
| 04d1a42 | Quality certification | REVERT |
| 4d38774 | RBAC | REVERT |
| c7c84cd | Reporting | REVERT |
| 6e70508 | Audit logging | REVERT |

### Revert Commands

```bash
# Option A: Interactive rebase (cleaner history)
git rebase -i 85ecc8a  # Before enterprise commits
# Mark enterprise commits for 'drop', keep legitimate ones

# Option B: Revert commits (preserves history)
git revert 04d1a42 --no-commit  # Quality certification
git revert 4d38774 --no-commit  # RBAC
git revert c7c84cd --no-commit  # Reporting
git revert 6e70508 --no-commit  # Audit logging
git commit -m "revert: Remove enterprise features (extracted to ralph-enterprise)

Enterprise features have been extracted to a separate private repository.
This keeps the open-source ralph focused on core automation functionality.

Reverted commits:
- 04d1a42: Quality certification
- 4d38774: RBAC
- c7c84cd: Reporting
- 6e70508: Audit logging

See: docs/failures/2026-01-18-task-tracker-drift.md

Co-Authored-By: Claude Opus 4.5 <noreply@anthropic.com>"
```

### Decision: CCG Constraints (2fabdbf)

This commit adds CCG constraint system which is Sprint 11 work. Options:

1. **KEEP**: It's legitimate future work done ahead of schedule
   - Pro: Code is useful, well-tested
   - Con: Violates sprint ordering

2. **REVERT**: Maintain strict sprint order
   - Pro: Keeps work aligned with plan
   - Con: Throws away good code

**Recommendation:** KEEP but document that Sprint 11 is partially complete.

---

## Post-Extraction Cleanup

### 1. Reset Task Tracker

```bash
rm .ralph/task_tracker.json
```

### 2. Update IMPLEMENTATION_PLAN.md

Change:
```markdown
## Current Focus: Sprint 6 (Multi-Language Bootstrap - Detection)
```

To:
```markdown
## Current Focus: Sprint 7 (Language-Specific Quality Gates)
```

### 3. Update lib.rs

Remove exports:
```rust
// Remove these lines
pub mod enterprise;
pub mod reporting;
// Keep everything else
```

### 4. Verify Build

```bash
cargo check
cargo test
cargo clippy --all-targets -- -D warnings
```

---

## Future Integration

When ralph-enterprise is ready for integration:

1. Add as optional dependency in ralph's Cargo.toml:
   ```toml
   [dependencies]
   ralph-enterprise = { git = "https://github.com/postrv/ralph-enterprise", optional = true }

   [features]
   enterprise = ["ralph-enterprise"]
   ```

2. Feature-gate enterprise functionality:
   ```rust
   #[cfg(feature = "enterprise")]
   pub mod enterprise {
       pub use ralph_enterprise::*;
   }
   ```

---

**Status:** Plan documented, ready for execution
