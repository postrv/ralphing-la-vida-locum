# Ralph Licensing Strategy: Open Core Model

> **Version:** 1.0
> **Last Updated:** January 2026
> **Related Documents:** [NARSIL_BACKEND_MASTER_PLAN.md](https://github.com/postrv/narsil-mcp/docs/NARSIL_BACKEND_MASTER_PLAN.md), [CCG-SPEC.md](https://github.com/postrv/narsil-mcp/docs/ccg-spec.md)

---

## Executive Summary

Ralph follows an **Open Core** licensing model with a unique positioning:

- **Open Source Core** (MIT): The CLI tool, execution loop, quality gates, and narsil-mcp integration
- **Commercial Extension** (Proprietary): **Continuous Code Improvement as a Service (CCIaaS)** - enterprise-grade autonomous coding with quality guarantees

### The Enterprise Value Proposition

> **"AI-assisted coding that provably improves code quality over time, backed by verifiable Code Context Graphs."**

Enterprises fear that widespread AI code generation will lead to:
- Accumulated technical debt
- Inconsistent code quality
- Loss of architectural integrity
- Security regressions
- Inability to verify AI changes

Ralph Cloud (CCIaaS) directly addresses these fears with:
- **CCG-backed "Definition of Done"** - Changes verified against architectural requirements
- **Quality trend tracking** - Proof that code quality improves over time
- **Regression prevention** - Automatic rollback on quality degradation
- **Audit trails** - Full traceability of every AI-assisted change
- **Quality certification** - Badges proving code meets standards

---

## The Virtuous Cycle: Ralph + narsil-mcp + CCG

```
┌─────────────────────────────────────────────────────────────────────┐
│                    CONTINUOUS CODE IMPROVEMENT CYCLE                 │
├─────────────────────────────────────────────────────────────────────┤
│                                                                      │
│   ┌──────────────┐         ┌──────────────┐         ┌────────────┐ │
│   │   RALPH      │ uses    │  NARSIL-MCP  │ generates│    CCG     │ │
│   │   (Executor) │────────►│  (Intelligence)────────►│ (Ground    │ │
│   │              │         │              │         │  Truth)    │ │
│   └──────┬───────┘         └──────────────┘         └─────┬──────┘ │
│          │                                                │        │
│          │                                                │        │
│          │ ┌──────────────────────────────────────────────┘        │
│          │ │                                                        │
│          ▼ ▼                                                        │
│   ┌──────────────┐                                                  │
│   │ CCG DIFF     │◄─────── "Does this change meet our Definition   │
│   │ VERIFICATION │         of Done? Does it improve the codebase?" │
│   └──────┬───────┘                                                  │
│          │                                                          │
│          ▼                                                          │
│   ┌──────────────┐                                                  │
│   │ HIGH-CONF.   │ = Enterprise trust in AI-assisted development   │
│   │ CHANGES      │                                                  │
│   └──────────────┘                                                  │
│                                                                      │
└─────────────────────────────────────────────────────────────────────┘
```

**Each component reinforces the others:**
1. **Ralph** drives adoption of **narsil-mcp** (needs code intelligence to work well)
2. **narsil-mcp** generates **CCGs** (creates ground truth for code quality)
3. **CCGs** enable **CCG Diff** (verifiable "Definition of Done")
4. **CCG Diff** enables **high-confidence changes** (enterprise trust)
5. **High-confidence changes** drive **Ralph** adoption (developers want it)

---

## Repository Structure

### Public Repository: `ralph` (MIT License)

**Purpose:** Drive adoption, build community, establish Ralph as the standard for Claude Code automation.

```
ralph/                              # MIT License - PUBLIC on GitHub
├── src/
│   ├── main.rs                     # CLI entry point
│   ├── lib.rs                      # Library crate
│   │
│   ├── loop/                       # Execution loop
│   │   ├── manager.rs              # LoopManager orchestration
│   │   ├── state.rs                # Loop state machine
│   │   ├── progress.rs             # Progress detection
│   │   ├── task_tracker.rs         # Task-level state machine
│   │   └── retry.rs                # Intelligent retry logic
│   │
│   ├── quality/                    # Quality gates
│   │   ├── gates.rs                # Clippy, tests, security gates
│   │   ├── enforcer.rs             # Gate enforcement
│   │   └── remediation.rs          # Auto-fix prompts
│   │
│   ├── prompt/                     # Dynamic prompts
│   │   ├── builder.rs              # Context-aware prompt generation
│   │   ├── templates.rs            # Phase-specific templates
│   │   ├── context.rs              # Prompt context assembly
│   │   └── antipatterns.rs         # Anti-pattern detection
│   │
│   ├── supervisor/                 # Health monitoring
│   │   ├── mod.rs                  # Supervisor verdicts
│   │   └── predictor.rs            # Basic stagnation prediction
│   │
│   ├── checkpoint/                 # Rollback system
│   │   ├── manager.rs              # Checkpoint creation
│   │   └── rollback.rs             # Regression rollback
│   │
│   ├── narsil/                     # narsil-mcp integration
│   │   ├── mod.rs
│   │   ├── client.rs               # MCP tool invocation
│   │   ├── ccg.rs                  # CCG loading/parsing
│   │   └── intelligence.rs         # Code intelligence queries
│   │
│   ├── hooks.rs                    # Security hooks
│   ├── bootstrap.rs                # Project initialization
│   ├── context.rs                  # Context builder
│   ├── archive.rs                  # Doc archival
│   ├── analytics.rs                # Local analytics
│   └── config.rs                   # Configuration
│
├── templates/                      # Bootstrap templates
│   ├── CLAUDE.md
│   ├── PROMPT_*.md
│   ├── agents/                     # Community agents
│   ├── skills/                     # Community skills
│   └── IMPLEMENTATION_PLAN.md
│
├── docs/
│   ├── LICENSING.md                # This document
│   ├── REPO_STRUCTURE.md           # Physical separation guide
│   └── CONTRIBUTING.md             # Contribution guidelines
│
├── tests/
├── Cargo.toml
├── LICENSE                         # MIT License
└── README.md
```

### Private Repository: `ralph-cloud` (Proprietary)

**Purpose:** Continuous Code Improvement as a Service (CCIaaS) - monetization through quality guarantees.

```
ralph-cloud/                        # Proprietary - PRIVATE
├── src/
│   ├── main.rs                     # Service entry point
│   ├── lib.rs                      # Library (for testing)
│   │
│   ├── cciaas/                     # Core CCIaaS engine
│   │   ├── mod.rs
│   │   ├── orchestrator.rs         # Multi-project orchestration
│   │   ├── campaign.rs             # Refactoring campaigns
│   │   ├── scheduler.rs            # Job scheduling
│   │   └── executor.rs             # Sandboxed execution
│   │
│   ├── verification/               # CCG-backed verification
│   │   ├── mod.rs
│   │   ├── definition_of_done.rs   # DoD verification against CCG
│   │   ├── ccg_diff.rs             # Integration with narsil-cloud CCG diff
│   │   ├── constraints.rs          # Architectural constraint checking
│   │   └── certification.rs        # Quality certification generation
│   │
│   ├── quality/                    # Enhanced quality tracking
│   │   ├── mod.rs
│   │   ├── trends.rs               # Quality trend analysis
│   │   ├── regression.rs           # Regression detection
│   │   ├── metrics.rs              # Quality metrics aggregation
│   │   └── reports.rs              # Quality reports generation
│   │
│   ├── intelligence/               # Advanced AI features
│   │   ├── mod.rs
│   │   ├── learning.rs             # Cross-session learning
│   │   ├── patterns.rs             # Failure pattern database
│   │   ├── prediction.rs           # ML-based stagnation prediction
│   │   └── recommendations.rs      # Improvement recommendations
│   │
│   ├── multi_agent/                # Advanced orchestration
│   │   ├── mod.rs
│   │   ├── agents.rs               # Specialized agent types
│   │   ├── handoff.rs              # Agent handoff logic
│   │   └── coordination.rs         # Multi-agent coordination
│   │
│   ├── enterprise/                 # Enterprise features
│   │   ├── mod.rs
│   │   ├── teams.rs                # Team/org management
│   │   ├── sso.rs                  # SAML/OIDC integration
│   │   ├── audit.rs                # Compliance audit logging
│   │   ├── rbac.rs                 # Role-based access control
│   │   └── quotas.rs               # Usage quotas
│   │
│   ├── api/                        # REST/GraphQL API
│   │   ├── mod.rs
│   │   ├── handlers.rs             # API handlers
│   │   ├── auth.rs                 # API authentication
│   │   └── webhooks.rs             # GitHub/GitLab webhooks
│   │
│   ├── dashboard/                  # Analytics dashboard
│   │   ├── mod.rs
│   │   ├── quality.rs              # Quality dashboards
│   │   ├── activity.rs             # Activity tracking
│   │   └── insights.rs             # AI-generated insights
│   │
│   └── billing/                    # Subscription management
│       ├── mod.rs
│       ├── stripe.rs               # Stripe integration
│       └── metering.rs             # Usage metering
│
├── infrastructure/
│   ├── terraform/
│   └── kubernetes/
│
├── Cargo.toml
├── LICENSE                         # Proprietary
└── README.md
```

---

## What's Open vs. Commercial

### Open Source (MIT) - `ralph`

| Component | Description | Rationale |
|-----------|-------------|-----------|
| **Execution Loop** | LoopManager, state machine, progress detection | Core value, hooks users |
| **Quality Gates** | Clippy, tests, security, no-allow, no-todo | Everyone needs this |
| **Task Tracker** | Task-level state machine | Essential for adoption |
| **Dynamic Prompts** | Context-aware prompt generation | Better results = more users |
| **Checkpoint/Rollback** | Local git-based snapshots | Safety feature |
| **Supervisor** | Basic health monitoring, verdicts | Loop stability |
| **narsil-mcp Integration** | Tool invocation, CCG parsing | **Drives narsil-mcp adoption** |
| **Bootstrap** | Project initialization, templates | Onboarding |
| **Security Hooks** | Command validation, secret detection | Security baseline |
| **Local Analytics** | Session tracking, JSONL output | Developer insights |
| **Community Templates** | Agents, skills, prompts | Community contributions |

### Commercial (Proprietary) - `ralph-cloud`

| Component | Description | Rationale |
|-----------|-------------|-----------|
| **CCIaaS Orchestrator** | Multi-project autonomous improvement | The product |
| **CCG "Definition of Done"** | Verify changes against CCG constraints | High-value enterprise |
| **Quality Trend Tracking** | Prove quality improves over time | Enterprise differentiation |
| **Regression Prevention** | Automatic rollback on degradation | Enterprise trust |
| **Quality Certification** | Badges proving code meets standards | Marketing + compliance |
| **Cross-Session Learning** | Learn from all sessions/projects | Competitive moat |
| **ML Stagnation Prediction** | Prevent problems before they happen | Premium intelligence |
| **Failure Pattern Database** | Shared knowledge across users | Network effects |
| **Multi-Agent Orchestration** | Specialized agents (planner, reviewer, etc.) | Advanced capability |
| **Refactoring Campaigns** | Scheduled, incremental refactoring | Unique offering |
| **Enterprise Features** | SSO, audit, RBAC, teams | Table stakes for enterprise |
| **API & Webhooks** | CI/CD integration | Automation |
| **Analytics Dashboard** | Quality insights, trends, reports | Decision support |
| **SLA & Support** | Guaranteed uptime, priority support | Enterprise contracts |

---

## CCIaaS: Continuous Code Improvement as a Service

### The Problem

Enterprises have legitimate concerns about AI-assisted development:

1. **Quality Drift**: AI generates code faster than humans can review
2. **Architectural Erosion**: Incremental changes violate design principles
3. **Technical Debt Accumulation**: AI takes shortcuts that compound over time
4. **Security Regressions**: AI may introduce vulnerabilities
5. **Compliance Risk**: Cannot prove AI changes meet standards

### The Solution: CCG-Backed Quality Assurance

```
┌─────────────────────────────────────────────────────────────────────┐
│                     CCIaaS QUALITY ASSURANCE LOOP                    │
├─────────────────────────────────────────────────────────────────────┤
│                                                                      │
│  1. BASELINE                                                         │
│     ┌─────────────────┐                                              │
│     │ Generate        │ narsil-mcp creates baseline CCG              │
│     │ Baseline CCG    │ (L0: manifest, L1: architecture, L2: index) │
│     └────────┬────────┘                                              │
│              │                                                        │
│  2. DEFINE CONSTRAINTS                                               │
│     ┌────────▼────────┐                                              │
│     │ Define "DoD"    │ Architect specifies:                         │
│     │ (Definition     │ - noDirectCalls(UI → Database)              │
│     │  of Done)       │ - maxCyclomaticComplexity(10)               │
│     └────────┬────────┘ - requireTests(coverage >= 80%)              │
│              │                                                        │
│  3. AUTONOMOUS IMPROVEMENT                                           │
│     ┌────────▼────────┐                                              │
│     │ Ralph Executes  │ Ralph runs autonomous coding session         │
│     │ Changes         │ - Quality gates enforced locally            │
│     └────────┬────────┘ - Commits only if gates pass                 │
│              │                                                        │
│  4. CCG DIFF VERIFICATION                                            │
│     ┌────────▼────────┐                                              │
│     │ Verify Against  │ narsil-cloud computes CCG diff:              │
│     │ DoD via CCG     │ - Does new CCG satisfy constraints?         │
│     │ Diff            │ - Did architecture improve or degrade?      │
│     └────────┬────────┘ - Are there new violations?                  │
│              │                                                        │
│  5. CERTIFICATION                                                    │
│     ┌────────▼────────┐                                              │
│     │ Issue Quality   │ If all checks pass:                          │
│     │ Certificate     │ - Stamp change as "CCG Verified"            │
│     └────────┬────────┘ - Update quality trend metrics               │
│              │            - Issue certification badge                 │
│              ▼                                                        │
│     ┌─────────────────┐                                              │
│     │ PROVABLE        │ Enterprise can demonstrate:                  │
│     │ QUALITY         │ "AI changes meet architectural standards"   │
│     │ IMPROVEMENT     │ "Code quality trend is positive"            │
│     └─────────────────┘ "No security regressions introduced"         │
│                                                                      │
└─────────────────────────────────────────────────────────────────────┘
```

### Pricing Tiers

| Tier | Price | Features |
|------|-------|----------|
| **Open Source** | Free | Local execution, basic quality gates, narsil-mcp integration |
| **Pro** | $49/user/mo | CCG verification, quality trends, API access |
| **Team** | $149/user/mo | Pro + team management, shared learning, dashboards |
| **Enterprise** | Custom | Team + SSO, audit logs, RBAC, dedicated support, SLA |

### Enterprise Success Metrics

Customers can track and prove:

1. **Quality Score Trend**: Weighted metric combining test coverage, clippy cleanliness, security issues, cyclomatic complexity
2. **Architectural Compliance**: % of changes that pass CCG constraint verification
3. **Regression Rate**: How often changes require rollback
4. **Time to Quality**: How quickly code reaches "shippable" state
5. **AI Confidence Index**: % of AI changes that pass all verification without intervention

---

## Dependency Relationship

```
┌─────────────────────────────────────────────────────────────────────┐
│                       RALPH-CLOUD (Proprietary)                      │
│                                                                      │
│  ┌──────────────┐  ┌──────────────┐  ┌──────────────┐              │
│  │ CCIaaS       │  │ Enterprise   │  │ Dashboard    │              │
│  │ Orchestrator │  │ Features     │  │ & Analytics  │              │
│  └──────────────┘  └──────────────┘  └──────────────┘              │
│          │                 │                 │                       │
│          └─────────────────┼─────────────────┘                       │
│                            │                                         │
│                            ▼ depends on                              │
│  ┌──────────────────────────────────────────────────────────────┐  │
│  │                    RALPH (MIT, git dependency)                 │  │
│  └──────────────────────────────────────────────────────────────┘  │
│                            │                                         │
│                            │ uses                                    │
│                            ▼                                         │
│  ┌──────────────────────────────────────────────────────────────┐  │
│  │                  NARSIL-CLOUD (Proprietary)                    │  │
│  │  CCG Diff | L3 Generation | Quality Certification              │  │
│  └──────────────────────────────────────────────────────────────┘  │
└─────────────────────────────────────────────────────────────────────┘
                              │
                              │ depends on
                              ▼
┌─────────────────────────────────────────────────────────────────────┐
│                       NARSIL-MCP (MIT, public)                       │
│                                                                      │
│  76+ MCP Tools | CCG L0-L2 | Local Intelligence | Security Scans   │
└─────────────────────────────────────────────────────────────────────┘
                              │
                              │ uses
                              ▼
┌─────────────────────────────────────────────────────────────────────┐
│                         RALPH (MIT, public)                          │
│                                                                      │
│  Loop | Quality Gates | Task Tracker | Prompts | narsil-mcp Client  │
└─────────────────────────────────────────────────────────────────────┘
```

**Cross-selling synergy:**
- Ralph users need narsil-mcp for code intelligence → drives narsil-mcp adoption
- narsil-mcp users who want CCG diff need narsil-cloud → drives narsil-cloud revenue
- Ralph users who want quality verification need ralph-cloud → drives ralph-cloud revenue
- Both cloud products integrate → bundle pricing opportunities

---

## Implementation Sprints by License

### Sprint License Key

| Badge | Meaning |
|-------|---------|
| 🟢 **OPEN** | MIT licensed, public repository |
| 🟡 **SPLIT** | Core functionality MIT, advanced features proprietary |
| 🔴 **COMMERCIAL** | Proprietary, ralph-cloud only |

### Sprint Mapping

| Sprint | Focus | License |
|--------|-------|---------|
| **1** | Task-Level State Machine | 🟢 OPEN |
| **2** | Dynamic Prompt Generation | 🟢 OPEN |
| **3** | Quality Gate Enforcement | 🟢 OPEN |
| **4** | Checkpoint/Rollback | 🟢 OPEN |
| **5** | narsil-mcp Integration | 🟢 OPEN |
| **6** | CCG-Aware Prompts | 🟡 SPLIT (basic=open, DoD verification=commercial) |
| **7** | CCIaaS Orchestrator | 🔴 COMMERCIAL |
| **8** | Quality Trend Analytics | 🔴 COMMERCIAL |
| **9** | Enterprise Features | 🔴 COMMERCIAL |
| **10** | Multi-Agent Orchestration | 🔴 COMMERCIAL |
| **11** | Cross-Session Learning | 🔴 COMMERCIAL |

---

## Differentiation from Competitors

### vs. GitHub Copilot Workspace
- **Copilot**: One-shot code generation
- **Ralph**: Continuous autonomous improvement with quality verification

### vs. Cursor / Aider
- **Cursor/Aider**: Interactive coding assistants
- **Ralph**: Fully autonomous execution with rollback and quality gates

### vs. DIY Claude Code Scripts
- **DIY**: Manual loop, no quality enforcement
- **Ralph**: Production-ready orchestration, narsil-mcp integration, CCG verification

### vs. Enterprise Code Quality Tools (SonarQube, etc.)
- **SonarQube**: Scans code, reports issues
- **Ralph CCIaaS**: Scans code AND fixes issues autonomously with verified quality

### Unique Value Proposition

> "The only autonomous coding system that can **prove** its changes improve code quality, backed by verifiable Code Context Graphs and architectural constraint verification."

---

## License Text

### ralph LICENSE (MIT)

```
MIT License

Copyright (c) 2025 Laurence Shouldice

Permission is hereby granted, free of charge, to any person obtaining a copy
of this software and associated documentation files (the "Software"), to deal
in the Software without restriction, including without limitation the rights
to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
copies of the Software, and to permit persons to whom the Software is
furnished to do so, subject to the following conditions:

The above copyright notice and this permission notice shall be included in all
copies or substantial portions of the Software.

THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
SOFTWARE.
```

### ralph-cloud LICENSE (Proprietary)

```
Copyright (c) 2025 Laurence Shouldice. All Rights Reserved.

This software and associated documentation files (the "Software") are
proprietary and confidential. Unauthorized copying, modification, distribution,
or use of this Software, via any medium, is strictly prohibited.

The Software is licensed, not sold. Use of the Software requires a valid
commercial license agreement.

For licensing inquiries, contact: [licensing email]
```

---

## FAQ

### Can I use Ralph open source commercially?

**Yes.** The MIT license allows commercial use. You get the CLI, quality gates, narsil-mcp integration, and local analytics.

### What do I get with Ralph Cloud (CCIaaS)?

- CCG-backed "Definition of Done" verification
- Proof that code quality improves over time
- Multi-project orchestration
- Enterprise features (SSO, audit, RBAC)
- Quality certification badges
- API and CI/CD integration
- Premium support and SLA

### How does Ralph relate to narsil-mcp?

Ralph uses narsil-mcp for code intelligence. The open source version integrates with narsil-mcp's 76+ tools. The commercial version integrates with narsil-cloud for CCG diff verification.

### Can I contribute to Ralph?

**Yes!** Contributions to the open source repository are welcome. See CONTRIBUTING.md.

---

## Contact

- **Open Source Issues:** github.com/postrv/ralph/issues
- **Commercial Inquiries:** [sales email]
- **Security Reports:** [security email]
