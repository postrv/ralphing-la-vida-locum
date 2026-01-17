# Repository Structure: Open Core Separation

> **Version:** 1.0
> **Last Updated:** January 2026
> **Related Documents:** [LICENSING.md](./LICENSING.md)

---

## Overview

This document defines the physical repository structure for the Ralph open core model.

**Two Repositories:**
1. `ralph` - Public, MIT licensed, drives adoption
2. `ralph-cloud` - Private, proprietary, CCIaaS monetization

---

## Repository 1: ralph (PUBLIC)

**GitHub:** `github.com/postrv/ralph`
**License:** MIT
**Visibility:** Public

### Directory Structure

```
ralph/
├── .github/
│   ├── workflows/
│   │   ├── ci.yml                  # Build, test, clippy
│   │   ├── release.yml             # Cargo publish, GitHub releases
│   │   └── security.yml            # Dependency scanning
│   ├── ISSUE_TEMPLATE/
│   ├── PULL_REQUEST_TEMPLATE.md
│   └── CODEOWNERS
│
├── src/
│   ├── main.rs                     # CLI entry point
│   ├── lib.rs                      # Library exports
│   │
│   ├── loop/                       # Core execution loop
│   │   ├── mod.rs
│   │   ├── manager.rs              # LoopManager orchestration
│   │   ├── state.rs                # LoopState, LoopMode
│   │   ├── progress.rs             # Semantic progress detection
│   │   ├── task_tracker.rs         # Task-level state machine
│   │   ├── retry.rs                # Intelligent retry logic
│   │   └── operations.rs           # Trait implementations
│   │
│   ├── quality/                    # Quality gate enforcement
│   │   ├── mod.rs
│   │   ├── gates.rs                # Individual gate implementations
│   │   ├── enforcer.rs             # QualityGateEnforcer
│   │   └── remediation.rs          # Auto-remediation prompts
│   │
│   ├── prompt/                     # Dynamic prompt generation
│   │   ├── mod.rs
│   │   ├── builder.rs              # PromptBuilder
│   │   ├── assembler.rs            # PromptAssembler
│   │   ├── context.rs              # PromptContext
│   │   ├── templates.rs            # Phase templates
│   │   └── antipatterns.rs         # Anti-pattern detection
│   │
│   ├── supervisor/                 # Health monitoring
│   │   ├── mod.rs                  # Supervisor, verdicts
│   │   └── predictor.rs            # Basic stagnation prediction
│   │
│   ├── checkpoint/                 # Git-based checkpoints
│   │   ├── mod.rs
│   │   ├── manager.rs              # CheckpointManager
│   │   └── rollback.rs             # RollbackManager
│   │
│   ├── narsil/                     # narsil-mcp integration
│   │   ├── mod.rs
│   │   ├── client.rs               # MCP tool invocation
│   │   ├── ccg.rs                  # CCG loading/parsing (L0-L2)
│   │   └── intelligence.rs         # Code intelligence queries
│   │
│   ├── testing/                    # Test infrastructure
│   │   ├── mod.rs
│   │   ├── traits.rs               # Testable traits
│   │   ├── mocks.rs                # Mock implementations
│   │   ├── fixtures.rs             # Test fixtures
│   │   └── assertions.rs           # Custom assertions
│   │
│   ├── config.rs                   # Configuration
│   ├── error.rs                    # Error types
│   ├── hooks.rs                    # Security hooks
│   ├── bootstrap.rs                # Project initialization
│   ├── context.rs                  # Context builder
│   ├── archive.rs                  # Doc archival
│   └── analytics.rs                # Local analytics
│
├── templates/                      # Bootstrap templates
│   ├── CLAUDE.md
│   ├── PROMPT_build.md
│   ├── PROMPT_debug.md
│   ├── PROMPT_plan.md
│   ├── IMPLEMENTATION_PLAN.md
│   ├── settings.json
│   ├── mcp.json
│   ├── agents/
│   │   ├── adversarial-reviewer.md
│   │   ├── security-auditor.md
│   │   └── supervisor.md
│   ├── skills/
│   │   ├── docs-sync.md
│   │   └── project-analyst.md
│   └── docs/
│       ├── architecture.md
│       ├── api.md
│       └── adr-template.md
│
├── docs/
│   ├── LICENSING.md                # Licensing strategy
│   ├── REPO_STRUCTURE.md           # This document
│   ├── CONTRIBUTING.md             # Contribution guidelines
│   ├── SECURITY.md                 # Security policy
│   └── archive/                    # Historical docs
│
├── tests/
│   ├── integration/
│   ├── fixtures/
│   └── cli_integration.rs
│
├── Cargo.toml
├── LICENSE                         # MIT License
├── README.md
├── CHANGELOG.md
└── IMPLEMENTATION_PLAN.md          # Development tracking
```

### Key Files

#### Cargo.toml (excerpt)
```toml
[package]
name = "ralph"
version = "0.2.0"
edition = "2021"
license = "MIT"
description = "Claude Code Automation Suite - Autonomous coding with bombproof reliability"
repository = "https://github.com/postrv/ralph"

[dependencies]
# CLI
clap = { version = "4.5", features = ["derive", "env"] }

# Async
tokio = { version = "1.43", features = ["full"] }

# Serialization
serde = { version = "1.0", features = ["derive"] }
serde_json = "1.0"

# File system
walkdir = "2.5"
ignore = "0.4"
globset = "0.4"

# Time
chrono = { version = "0.4", features = ["serde"] }

# Terminal
colored = "2.2"
indicatif = "0.17"

# Error handling
anyhow = "1.0"
thiserror = "2.0"

# Logging
tracing = "0.1"
tracing-subscriber = { version = "0.3", features = ["env-filter", "json"] }

# Regex
regex = "1.11"

# Async traits
async-trait = "0.1"
```

#### lib.rs (public API)
```rust
//! Ralph: Claude Code Automation Suite
//!
//! Autonomous coding with bombproof reliability.
//!
//! # Feature Flags
//! - `narsil`: Enable narsil-mcp integration (default: enabled)

pub mod checkpoint;
pub mod loop_manager;
pub mod narsil;
pub mod prompt;
pub mod quality;
pub mod supervisor;
pub mod testing;

// Re-export main types
pub use checkpoint::{Checkpoint, CheckpointManager, RollbackManager};
pub use loop_manager::{LoopManager, LoopState, TaskTracker};
pub use prompt::{PromptBuilder, PromptContext};
pub use quality::{QualityGateEnforcer, QualityGateResult};
pub use supervisor::{Supervisor, Verdict};
```

---

## Repository 2: ralph-cloud (PRIVATE)

**GitHub:** `github.com/postrv/ralph-cloud` (private)
**License:** Proprietary
**Visibility:** Private

### Directory Structure

```
ralph-cloud/
├── .github/
│   ├── workflows/
│   │   ├── ci.yml                  # Build, test
│   │   ├── deploy-staging.yml      # Deploy to staging
│   │   └── deploy-prod.yml         # Deploy to production
│   └── CODEOWNERS
│
├── src/
│   ├── main.rs                     # Service entry point
│   ├── lib.rs                      # Library (for testing)
│   │
│   ├── cciaas/                     # Continuous Code Improvement as a Service
│   │   ├── mod.rs
│   │   ├── orchestrator.rs         # Multi-project orchestration
│   │   ├── campaign.rs             # Refactoring campaigns
│   │   ├── scheduler.rs            # Job scheduling (cron, triggers)
│   │   ├── executor.rs             # Sandboxed execution environment
│   │   └── workers.rs              # Background job workers
│   │
│   ├── verification/               # CCG-backed verification
│   │   ├── mod.rs
│   │   ├── definition_of_done.rs   # DoD spec parsing & verification
│   │   ├── ccg_diff.rs             # narsil-cloud CCG diff client
│   │   ├── constraints.rs          # Architectural constraint checking
│   │   ├── certification.rs        # Quality certification generation
│   │   └── badges.rs               # shields.io-style badge generation
│   │
│   ├── quality/                    # Enhanced quality tracking
│   │   ├── mod.rs
│   │   ├── trends.rs               # Quality trend analysis
│   │   ├── regression.rs           # Regression detection
│   │   ├── metrics.rs              # Aggregate quality metrics
│   │   ├── scoring.rs              # Quality score calculation
│   │   └── reports.rs              # PDF/HTML report generation
│   │
│   ├── intelligence/               # Advanced AI features
│   │   ├── mod.rs
│   │   ├── learning.rs             # Cross-session learning
│   │   ├── patterns.rs             # Failure pattern database
│   │   ├── prediction.rs           # ML-based stagnation prediction
│   │   ├── recommendations.rs      # Improvement recommendations
│   │   └── embeddings.rs           # Code similarity via embeddings
│   │
│   ├── multi_agent/                # Advanced orchestration
│   │   ├── mod.rs
│   │   ├── agents.rs               # Agent types (Planner, Implementer, etc.)
│   │   ├── handoff.rs              # Agent handoff logic
│   │   ├── coordination.rs         # Multi-agent coordination
│   │   └── prompts.rs              # Agent-specific prompts
│   │
│   ├── enterprise/                 # Enterprise features
│   │   ├── mod.rs
│   │   ├── teams.rs                # Team/organization management
│   │   ├── projects.rs             # Multi-project management
│   │   ├── sso.rs                  # SAML/OIDC integration
│   │   ├── audit.rs                # Compliance audit logging
│   │   ├── rbac.rs                 # Role-based access control
│   │   └── quotas.rs               # Usage quotas/limits
│   │
│   ├── api/                        # REST/GraphQL API
│   │   ├── mod.rs
│   │   ├── routes.rs               # Route definitions
│   │   ├── handlers/
│   │   │   ├── campaigns.rs        # Campaign management
│   │   │   ├── projects.rs         # Project management
│   │   │   ├── quality.rs          # Quality metrics
│   │   │   └── webhooks.rs         # Webhook handlers
│   │   ├── auth.rs                 # JWT/API key authentication
│   │   ├── middleware.rs           # Request middleware
│   │   └── errors.rs               # API error types
│   │
│   ├── dashboard/                  # Analytics dashboard backend
│   │   ├── mod.rs
│   │   ├── quality.rs              # Quality dashboards
│   │   ├── activity.rs             # Activity tracking
│   │   ├── insights.rs             # AI-generated insights
│   │   └── exports.rs              # Data exports
│   │
│   ├── integrations/               # External integrations
│   │   ├── mod.rs
│   │   ├── github.rs               # GitHub App integration
│   │   ├── gitlab.rs               # GitLab integration
│   │   ├── narsil_cloud.rs         # narsil-cloud API client
│   │   └── notifications.rs        # Slack, email, etc.
│   │
│   └── billing/                    # Subscription management
│       ├── mod.rs
│       ├── stripe.rs               # Stripe integration
│       ├── metering.rs             # Usage metering
│       └── plans.rs                # Plan definitions
│
├── frontend/                       # Dashboard UI (optional)
│   ├── src/
│   └── package.json
│
├── infrastructure/
│   ├── terraform/
│   │   ├── main.tf
│   │   ├── cloudflare.tf           # Cloudflare config
│   │   ├── database.tf             # PostgreSQL/Supabase
│   │   ├── storage.tf              # R2/S3 for artifacts
│   │   └── secrets.tf              # Secret management
│   └── kubernetes/
│       ├── deployment.yaml
│       ├── service.yaml
│       └── ingress.yaml
│
├── migrations/                     # Database migrations
│   └── *.sql
│
├── tests/
│   ├── integration/
│   └── e2e/
│
├── Cargo.toml
├── LICENSE                         # Proprietary license
└── README.md                       # Internal docs
```

### Key Files

#### Cargo.toml
```toml
[package]
name = "ralph-cloud"
version = "0.1.0"
edition = "2021"
license = "LicenseRef-Proprietary"
publish = false                     # Never publish to crates.io

[dependencies]
# Open source core as git dependency
ralph = { git = "https://github.com/postrv/ralph", tag = "v0.2.0" }

# Web framework
axum = "0.7"
tower = "0.4"
tower-http = { version = "0.5", features = ["cors", "trace"] }

# Database
sqlx = { version = "0.7", features = ["postgres", "runtime-tokio-native-tls", "chrono", "uuid"] }

# Authentication
jsonwebtoken = "9"

# Async
tokio = { version = "1", features = ["full"] }

# Serialization
serde = { version = "1.0", features = ["derive"] }
serde_json = "1.0"

# HTTP client (for narsil-cloud)
reqwest = { version = "0.12", features = ["json"] }

# Background jobs
apalis = "0.4"

# Billing
stripe-rust = "0.24"

# Observability
tracing = "0.1"
tracing-subscriber = { version = "0.3", features = ["env-filter", "json"] }
opentelemetry = "0.22"

# Error handling
anyhow = "1.0"
thiserror = "2.0"
```

#### Using ralph crate
```rust
// src/cciaas/orchestrator.rs
use ralph::loop_manager::{LoopManager, LoopState};
use ralph::quality::QualityGateEnforcer;
use ralph::checkpoint::CheckpointManager;

/// CCIaaS Orchestrator - runs Ralph loops with enterprise features
pub struct Orchestrator {
    config: OrchestratorConfig,
    db: DatabasePool,
    narsil_cloud: NarsilCloudClient,
}

impl Orchestrator {
    /// Execute a CCIaaS campaign
    pub async fn execute_campaign(&self, campaign: &Campaign) -> Result<CampaignResult> {
        // Create Ralph loop manager (uses open source core)
        let mut loop_manager = LoopManager::new(campaign.project_path.clone());

        // Add enterprise instrumentation
        loop_manager.on_iteration(|state| {
            self.log_iteration(&campaign.id, state).await;
        });

        // Run with CCG verification
        let result = loop_manager.run().await?;

        // Verify against Definition of Done (proprietary)
        let verification = self.verify_against_dod(&campaign, &result).await?;

        // Generate quality certification (proprietary)
        if verification.passed {
            self.issue_certification(&campaign).await?;
        }

        Ok(CampaignResult { result, verification })
    }

    /// Verify changes against CCG Definition of Done
    async fn verify_against_dod(
        &self,
        campaign: &Campaign,
        result: &LoopResult,
    ) -> Result<Verification> {
        // Call narsil-cloud CCG diff API
        let baseline_ccg = self.load_baseline_ccg(&campaign).await?;
        let current_ccg = self.narsil_cloud.generate_ccg(&campaign.project_path).await?;

        let diff = self.narsil_cloud.compute_diff(&baseline_ccg, &current_ccg).await?;

        // Check against DoD constraints
        let constraints = self.load_dod_constraints(&campaign).await?;
        let violations = constraints.check(&diff);

        Ok(Verification {
            passed: violations.is_empty(),
            violations,
            quality_delta: diff.quality_score_delta(),
        })
    }
}
```

---

## Migration Path

### Phase 1: Current State (ralph monorepo)

Everything is currently in `ralph`. No changes yet.

### Phase 2: Prepare for Split

1. **Ensure clean boundaries** - Core loop, quality gates, prompts are fully open
2. **narsil integration** - Complete narsil-mcp client in open source
3. **Document public API** - `lib.rs` exports clearly defined

### Phase 3: Create ralph-cloud

1. Create private `ralph-cloud` repository
2. Add `ralph` as git dependency
3. Implement CCIaaS orchestrator
4. Implement narsil-cloud integration for CCG diff
5. Set up CI/CD for deployment

### Phase 4: Maintain Separation

1. All new open source features go to `ralph`
2. All commercial features go to `ralph-cloud`
3. `ralph-cloud` always depends on a tagged version of `ralph`
4. Releases coordinated but independent

---

## Branching Strategy

### ralph (Public)

```
main                    # Stable, releases tagged here
├── develop             # Integration branch
├── feature/task-*      # Feature branches
├── feature/quality-*
└── feature/narsil-*
```

### ralph-cloud (Private)

```
main                    # Production
├── staging             # Pre-production testing
├── develop             # Integration
├── feature/cciaas-*    # CCIaaS features
├── feature/enterprise-*
└── feature/dashboard-*
```

---

## Integration Points

### ralph → narsil-mcp

```rust
// ralph/src/narsil/client.rs

/// Client for narsil-mcp MCP tools
pub struct NarsilClient {
    // ... implementation
}

impl NarsilClient {
    /// Run security scan via narsil-mcp
    pub async fn scan_security(&self) -> Result<SecurityReport> {
        // Invoke MCP tool
    }

    /// Get call graph for a function
    pub async fn get_call_graph(&self, function: &str) -> Result<CallGraph> {
        // Invoke MCP tool
    }

    /// Load CCG manifest (L0)
    pub async fn load_ccg_manifest(&self, path: &Path) -> Result<CcgManifest> {
        // Parse JSON-LD
    }
}
```

### ralph-cloud → narsil-cloud

```rust
// ralph-cloud/src/integrations/narsil_cloud.rs

/// Client for narsil-cloud proprietary APIs
pub struct NarsilCloudClient {
    api_key: String,
    base_url: String,
}

impl NarsilCloudClient {
    /// Generate full L3 CCG
    pub async fn generate_l3_ccg(&self, repo: &str) -> Result<Ccg> {
        // POST /api/ccg/generate
    }

    /// Compute CCG diff
    pub async fn compute_diff(&self, before: &Ccg, after: &Ccg) -> Result<CcgDiff> {
        // POST /api/ccg/diff
    }

    /// Verify constraints
    pub async fn verify_constraints(
        &self,
        diff: &CcgDiff,
        constraints: &Constraints,
    ) -> Result<VerificationResult> {
        // POST /api/ccg/verify
    }
}
```

---

## CI/CD Configuration

### ralph CI

```yaml
# .github/workflows/ci.yml
name: CI

on: [push, pull_request]

jobs:
  test:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable
      - run: cargo test --all-features
      - run: cargo clippy --all-targets -- -D warnings

  release:
    if: startsWith(github.ref, 'refs/tags/')
    needs: test
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable
      - run: cargo publish
        env:
          CARGO_REGISTRY_TOKEN: ${{ secrets.CRATES_IO_TOKEN }}
```

### ralph-cloud CI

```yaml
# .github/workflows/ci.yml
name: CI

on: [push, pull_request]

jobs:
  test:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable
      - uses: webfactory/ssh-agent@v0.8.0
        with:
          ssh-private-key: ${{ secrets.RALPH_DEPLOY_KEY }}
      - run: cargo test

  deploy-staging:
    needs: test
    if: github.ref == 'refs/heads/staging'
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - run: ./deploy.sh staging

  deploy-prod:
    needs: test
    if: github.ref == 'refs/heads/main'
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - run: ./deploy.sh prod
```

---

## Secrets Management

| Secret | Location | Purpose |
|--------|----------|---------|
| `CRATES_IO_TOKEN` | ralph | Publishing to crates.io |
| `RALPH_DEPLOY_KEY` | ralph-cloud | Access to private ralph repo |
| `DATABASE_URL` | ralph-cloud | PostgreSQL connection |
| `STRIPE_SECRET_KEY` | ralph-cloud | Billing |
| `NARSIL_CLOUD_API_KEY` | ralph-cloud | narsil-cloud integration |
| `GITHUB_APP_PRIVATE_KEY` | ralph-cloud | GitHub App integration |

---

## Checklist for Split

- [ ] Ensure MIT LICENSE file is in ralph root
- [ ] Complete narsil-mcp integration in open source
- [ ] Document public API in lib.rs
- [ ] Create ralph-cloud private repository
- [ ] Set up deploy keys for cross-repo dependency
- [ ] Configure CI/CD for both repositories
- [ ] Update ralph README with "Commercial features" section
- [ ] Create CONTRIBUTING.md for open source contributions
- [ ] Set up issue templates for feature requests
- [ ] Configure Dependabot for both repos
- [ ] Set up release automation
- [ ] Create landing page for CCIaaS
