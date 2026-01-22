# Ralph: Strategic Vision & Architecture Analysis

**Document Type:** Strategic Analysis & Roadmap  
**Version:** 1.0  
**Date:** January 2026

---

## Executive Summary

Ralph is a sophisticated, Rust-based CLI tool for autonomous AI-assisted development. After deep analysis of the codebase (~62,000+ lines), three key insights emerge:

1. **Architectural Excellence**: Ralph's core abstractions (LoopManager, Supervisor, StagnationPredictor, QualityGateEnforcer) are exceptional. The dependency injection patterns, trait-based quality gates, and multi-factor risk scoring show genuine engineering maturity.

2. **Polyglot Foundation is Solid**: Contrary to the premise that Ralph is "Rust-only," the codebase already has substantial polyglot infrastructure—`LanguageDetector` supporting 32 languages, `gates_for_language()` factory for Python/TypeScript/Go, and `TemplateRegistry` with polyglot prompt generation.

3. **The Gap is Integration, Not Architecture**: The "polyglot gap" isn't missing code—it's incomplete wiring. Quality gates exist for Python/TypeScript/Go but aren't fully integrated into the loop's execution path. The templates exist but bootstrap doesn't fully leverage them.

**Strategic Position**: Ralph is 80% of the way to being the definitive AI coding orchestration tool. The architecture is already best-in-class. The path to dominance is finishing the integration work and then building the ecosystem flywheel.

---

## Part 1: Deep Codebase Analysis

### 1.1 Core Architecture Overview

Ralph's architecture follows a clean separation of concerns across several key modules:

```
┌─────────────────────────────────────────────────────────────────────┐
│                          CLI (main.rs)                              │
├─────────────────────────────────────────────────────────────────────┤
│                                                                     │
│  ┌─────────────┐    ┌──────────────┐    ┌──────────────────────┐   │
│  │ Bootstrap   │    │ LoopManager  │    │ Analytics            │   │
│  │             │    │              │    │                      │   │
│  │ - Language  │    │ - Iteration  │    │ - SessionSummary     │   │
│  │   Detector  │    │ - Progress   │    │ - QualityTrends      │   │
│  │ - Template  │    │ - Checkpoint │    │ - AggregateStats     │   │
│  │   Registry  │    │ - Retry      │    │                      │   │
│  └─────────────┘    └──────────────┘    └──────────────────────┘   │
│         │                  │                       │                │
│         │                  ▼                       │                │
│         │          ┌──────────────┐                │                │
│         │          │  Supervisor  │                │                │
│         │          │              │                │                │
│         │          │ - Verdicts   │◄───────────────┘                │
│         │          │ - Health     │                                 │
│         │          │ - Predictor  │                                 │
│         │          └──────────────┘                                 │
│         │                  │                                        │
│         ▼                  ▼                                        │
│  ┌─────────────────────────────────────────────────────────────┐   │
│  │                    Quality Module                            │   │
│  │                                                              │   │
│  │  ┌────────────────┐  ┌─────────────────────────────────┐    │   │
│  │  │ QualityGate    │  │ Language-Specific Gates         │    │   │
│  │  │ Trait          │──│                                 │    │   │
│  │  │                │  │ • Rust: Clippy, Test, NoAllow   │    │   │
│  │  │ • run()        │  │ • Python: Ruff, Pytest, Mypy    │    │   │
│  │  │ • name()       │  │ • TS/JS: ESLint, Jest, Tsc      │    │   │
│  │  │ • remediation()│  │ • Go: GoVet, GolangciLint       │    │   │
│  │  └────────────────┘  └─────────────────────────────────┘    │   │
│  │                                                              │   │
│  │  ┌────────────────────────────────────────────────────┐     │   │
│  │  │ QualityGateEnforcer                                │     │   │
│  │  │ • detect_available_gates()                         │     │   │
│  │  │ • can_commit() → Result<Summary, Vec<GateResult>>  │     │   │
│  │  │ • generate_remediation_prompt()                    │     │   │
│  │  └────────────────────────────────────────────────────┘     │   │
│  └─────────────────────────────────────────────────────────────┘   │
│                                                                     │
│  ┌─────────────────────────────────────────────────────────────┐   │
│  │                    Narsil Integration                        │   │
│  │                                                              │   │
│  │  • NarsilClient - MCP tool invocation                       │   │
│  │  • SecurityScanning - Vulnerability detection               │   │
│  │  • CCG (Compact Code Graph) - Architecture intelligence     │   │
│  │  • Graceful degradation when unavailable                    │   │
│  └─────────────────────────────────────────────────────────────┘   │
│                                                                     │
└─────────────────────────────────────────────────────────────────────┘
```

### 1.2 The Loop Manager: Heart of Ralph

The `LoopManager` implements a sophisticated "Check-Plan-Act-Verify" cycle:

```rust
// Logical Flow (simplified from src/loop/manager/mod.rs)

pub async fn run(&mut self) -> Result<()> {
    while self.state.iteration < self.max_iterations {
        // Phase 1: Progress Detection
        let plan_hash = self.get_plan_hash()?;
        let commit_hash = self.deps.git.get_latest_commit_hash()?;
        
        if plan_hash != self.state.last_plan_hash 
           || commit_hash != self.state.last_commit_hash {
            self.state.stagnation_count = 0;  // Reset on progress
        } else {
            self.state.stagnation_count += 1;
        }
        
        // Phase 2: Stagnation Prediction (BEFORE action)
        let risk_signals = RiskSignals::new()
            .with_commit_gap(self.state.stagnation_count)
            .with_file_touches(file_touch_history)
            .with_errors(recent_errors)
            .with_mode_switches(supervisor.mode_switch_count());
            
        let risk_score = predictor.risk_score(&risk_signals);
        let preventive_action = predictor.preventive_action(&risk_signals, risk_score);
        
        // Phase 3: Supervisor Health Check
        let verdict = supervisor.check_health(&metrics, &self.state)?;
        
        match verdict {
            SupervisorVerdict::Proceed => { /* continue */ },
            SupervisorVerdict::SwitchMode { target, .. } => {
                self.state.mode = target;
            },
            SupervisorVerdict::PauseForReview { reason } => {
                return self.request_human_intervention(reason);
            },
            SupervisorVerdict::Abort { reason } => {
                return Err(anyhow!("Aborted: {}", reason));
            },
        }
        
        // Phase 4: Prompt Assembly (Dynamic)
        let prompt = self.prompt_assembler.build(
            self.state.mode,
            &self.task_tracker.next_task(),
            preventive_action.guidance(),
            &antipatterns,
        )?;
        
        // Phase 5: Execute Claude
        let result = self.deps.claude.run(&prompt)?;
        
        // Phase 6: Quality Gates
        let gate_result = self.deps.quality.run_gates()?;
        
        if gate_result.all_passed() {
            self.deps.git.commit(&gate_result.summary)?;
            self.task_tracker.mark_progress()?;
        } else {
            // Feed remediation back into next iteration
            recent_errors.extend(gate_result.errors());
        }
        
        self.state.iteration += 1;
    }
}
```

**Key Insight**: The loop is already language-agnostic at its core. The `QualityChecker` trait abstracts away the specific gates, and the `PromptAssembler` can inject language-specific guidance. The architecture is ready for polyglot—the gates just need to be wired in.

### 1.3 Quality Gate System: The Binding Layer

The quality gate architecture is exceptionally well-designed with a trait-based approach:

```rust
// From src/quality/gates/mod.rs

/// Core trait for all quality gates
pub trait QualityGate: Send + Sync {
    /// Human-readable gate name
    fn name(&self) -> &str;
    
    /// Run the gate and return issues
    fn run(&self, project_dir: &Path) -> Result<GateResult>;
    
    /// Is this gate blocking? (must pass before commit)
    fn is_blocking(&self) -> bool { true }
    
    /// Generate remediation guidance for Claude
    fn remediation(&self, issues: &[GateIssue]) -> String;
}

/// Factory for language-specific gates
pub fn gates_for_language(lang: Language) -> Vec<Box<dyn QualityGate>> {
    match lang {
        Language::Rust => vec![
            Box::new(ClippyGate::new()),
            Box::new(CargoTestGate::new()),
            Box::new(NoAllowGate::new()),
            Box::new(SecurityGate::new()),
            Box::new(NoTodoGate::new()),
        ],
        Language::Python => vec![
            Box::new(RuffGate::new()),
            Box::new(PytestGate::new()),
            Box::new(MypyGate::new()),
            Box::new(BanditGate::new()),
        ],
        Language::TypeScript | Language::JavaScript => vec![
            Box::new(EslintGate::new()),
            Box::new(JestGate::new()),
            Box::new(TscGate::new()),
            Box::new(NpmAuditGate::new()),
        ],
        Language::Go => vec![
            Box::new(GoVetGate::new()),
            Box::new(GolangciLintGate::new()),
            Box::new(GoTestGate::new()),
            Box::new(GovulncheckGate::new()),
        ],
        // More languages...
        _ => vec![], // Graceful degradation
    }
}
```

**Current State Assessment**:

| Language | Detection | Gate Impls | Template | Integration |
|----------|-----------|------------|----------|-------------|
| **Rust** | ✅ Full | ✅ 5 gates | ✅ Full | ✅ Complete |
| **Python** | ✅ Full | ✅ 4 gates | ✅ Full | ⚠️ Partial |
| **TypeScript** | ✅ Full | ✅ 4 gates | ✅ Full | ⚠️ Partial |
| **JavaScript** | ✅ Full | ✅ 3 gates | ✅ Full | ⚠️ Partial |
| **Go** | ✅ Full | ✅ 4 gates | ✅ Full | ⚠️ Partial |
| **Ruby** | ✅ Full | ✅ 4 gates | ✅ Full | ⚠️ Partial |
| **Java** | ✅ Full | 🔲 Defined | 🔲 Stub | 🔲 None |
| **C#** | ✅ Full | 🔲 Defined | 🔲 Stub | 🔲 None |

The gates exist! The issue is that `detect_available_gates()` and the `RealQualityChecker` need to be fully integrated with bootstrap's language detection output.

### 1.4 Stagnation Predictor: Proactive Intelligence

The `StagnationPredictor` is particularly impressive—one of the most sophisticated components:

```rust
// From src/supervisor/predictor.rs

/// Risk factors and their default weights (sum to 1.0)
pub struct RiskWeights {
    pub commit_gap: f64,        // 0.25 - Iterations without commit
    pub file_churn: f64,        // 0.20 - Same files edited repeatedly  
    pub error_repeat: f64,      // 0.20 - Same errors occurring
    pub test_stagnation: f64,   // 0.15 - No new tests added
    pub mode_oscillation: f64,  // 0.10 - Frequent mode switches
    pub warning_growth: f64,    // 0.10 - Clippy warnings increasing
}

/// Risk levels with associated thresholds
pub enum RiskLevel {
    Low,      // 0-30: Normal operation
    Medium,   // 30-60: Caution, inject guidance
    High,     // 60-80: Intervention needed
    Critical, // 80-100: Request human review
}

impl StagnationPredictor {
    pub fn preventive_action(&self, signals: &RiskSignals, score: RiskScore) 
        -> PreventiveAction 
    {
        let level = self.risk_level(score);
        
        match level {
            RiskLevel::Low => PreventiveAction::None,
            
            RiskLevel::Medium => {
                let guidance = self.generate_unstick_guidance(&signals);
                PreventiveAction::InjectGuidance { guidance }
            },
            
            RiskLevel::High => {
                let breakdown = self.risk_breakdown(signals);
                self.high_risk_action(&breakdown, signals)
            },
            
            RiskLevel::Critical => {
                PreventiveAction::RequestReview {
                    reason: format!(
                        "Critical stagnation risk (score={:.0}). \
                         Dominant factor: {}",
                        score, 
                        breakdown.dominant_factor()
                    )
                }
            }
        }
    }
}
```

The predictor tracks accuracy of its predictions over time, allowing for self-tuning. This is genuinely sophisticated ML-adjacent engineering.

### 1.5 Language Detection: Already Comprehensive

The `LanguageDetector` supports 32 languages with confidence scoring:

```rust
// From src/bootstrap/language_detector.rs

impl LanguageDetector {
    pub fn detect(&self) -> Vec<DetectedLanguage> {
        let mut scores: HashMap<Language, f32> = HashMap::new();
        
        // Weight 1: Manifest files (strong signal)
        for lang in Language::all() {
            for manifest in lang.manifest_files() {
                if self.path.join(manifest).exists() {
                    *scores.entry(lang).or_default() += 100.0;
                }
            }
        }
        
        // Weight 2: File extension counts
        for entry in WalkDir::new(&self.path) {
            if let Some(ext) = entry.extension() {
                for lang in Language::all() {
                    if lang.extensions().contains(&ext) {
                        *scores.entry(lang).or_default() += 1.0;
                    }
                }
            }
        }
        
        // Normalize and rank
        let total: f32 = scores.values().sum();
        scores.into_iter()
            .map(|(lang, score)| DetectedLanguage {
                language: lang,
                confidence: score / total,
                primary: false, // Set later
            })
            .filter(|d| d.confidence > 0.05) // 5% threshold
            .sorted_by(|a, b| b.confidence.partial_cmp(&a.confidence).unwrap())
            .enumerate()
            .map(|(i, mut d)| { d.primary = i == 0; d })
            .collect()
    }
    
    /// Check if project uses multiple significant languages (>10% each)
    pub fn is_polyglot(&self) -> bool {
        self.detect()
            .iter()
            .filter(|d| d.confidence >= 0.10)
            .count() >= 2
    }
}
```

**The infrastructure is there**. The detection works. The issue is that bootstrap doesn't fully utilize the detection results to configure the loop's quality gates.

---

## Part 2: Competitive Analysis & Market Position

### 2.1 The AI Coding Tool Landscape

| Tool | Strengths | Weaknesses | vs. Ralph |
|------|-----------|------------|-----------|
| **Cursor** | IDE integration, fast edits | No autonomous loop, no quality gates | Ralph: autonomous + enforced quality |
| **Aider** | Multi-model, git-aware | Basic quality checks, no predictor | Ralph: superior stagnation handling |
| **Continue** | Open source, extensible | IDE-bound, no loop orchestration | Ralph: standalone, loop-native |
| **Devin** | Full autonomy, web access | Closed, expensive, black box | Ralph: open core, transparent |
| **GitHub Copilot Workspace** | GitHub integration | Early, limited customization | Ralph: mature, configurable |

### 2.2 Ralph's Unique Differentiators

1. **Quality-Gate-First Architecture**: Ralph is the only tool that makes quality gates a first-class orchestration primitive. Others bolt on linting; Ralph builds around it.

2. **Predictive Stagnation Prevention**: The multi-factor risk model with preventive actions is unique. Others detect stagnation reactively; Ralph predicts and prevents.

3. **Checkpoint/Rollback System**: Full git-based checkpointing with quality regression detection. No other tool offers automatic rollback on test/lint regression.

4. **Graceful Degradation**: Every advanced feature (narsil-mcp, CCG, security scanning) degrades gracefully. Ralph works standalone but becomes more powerful with integrations.

5. **Open Core Model**: Clear separation between MIT-licensed CLI and potential commercial CCIaaS. This enables community growth while preserving monetization.

---

## Part 3: Strategic Roadmap

### 3.1 Guiding Principles

1. **Complete Before Extend**: Finish polyglot integration before adding new features.
2. **Prove with Polyglot**: Validate on a real Next.js + FastAPI project end-to-end.
3. **Community Before Cloud**: Build adoption through open-source excellence first.
4. **Reliability is the Product**: Ralph's brand is "bombproof"—never compromise it.

### 3.2 Strategic Phases

```
┌─────────────────────────────────────────────────────────────────────┐
│ PHASE 1: POLYGLOT COMPLETION (Weeks 1-4)                            │
│                                                                     │
│ Goal: Make Ralph genuinely best-in-class for Python, TS, Go        │
│                                                                     │
│ • Wire detect_available_gates() into LoopManager                   │
│ • Integrate language detection → quality gate selection            │
│ • Test gates end-to-end for Python, TypeScript, Go                 │
│ • Generate language-appropriate prompts dynamically                │
│ • Validate on real polyglot project (Next.js + FastAPI)            │
│                                                                     │
│ Success Metric: `ralph loop` works on Python project with same     │
│ reliability as Rust                                                 │
└─────────────────────────────────────────────────────────────────────┘
                                    │
                                    ▼
┌─────────────────────────────────────────────────────────────────────┐
│ PHASE 2: RELIABILITY HARDENING (Weeks 5-8)                          │
│                                                                     │
│ Goal: Make Ralph the most reliable AI coding tool, period          │
│                                                                     │
│ • Full predictor integration with preventive actions               │
│ • Context window management with language-aware prioritization     │
│ • Enhanced checkpoint system with per-language metrics             │
│ • Regression detection for lint warnings, not just tests           │
│ • Improved error classification for retry logic                    │
│                                                                     │
│ Success Metric: <5% stagnation rate on 100-iteration runs          │
└─────────────────────────────────────────────────────────────────────┘
                                    │
                                    ▼
┌─────────────────────────────────────────────────────────────────────┐
│ PHASE 3: ECOSYSTEM & ADOPTION (Weeks 9-16)                          │
│                                                                     │
│ Goal: Build community momentum and establish Ralph as the standard │
│                                                                     │
│ • Plugin architecture for community gates (Ruby, PHP, etc.)        │
│ • Template marketplace for skills/agents                           │
│ • Comprehensive polyglot examples and documentation                │
│ • Model abstraction (Claude, GPT-4o, Gemini, local)               │
│ • Integration guides for popular frameworks                        │
│                                                                     │
│ Success Metric: 1000+ GitHub stars, 50+ community contributions    │
└─────────────────────────────────────────────────────────────────────┘
                                    │
                                    ▼
┌─────────────────────────────────────────────────────────────────────┐
│ PHASE 4: COMMERCIAL FOUNDATION (Weeks 17-24)                        │
│                                                                     │
│ Goal: Lay groundwork for CCIaaS without compromising open core     │
│                                                                     │
│ • Analytics upload (opt-in) for quality trend tracking             │
│ • Remote campaign orchestration API                                │
│ • CCG-diff verification for "provable improvement"                 │
│ • Team features: shared configurations, quality baselines          │
│ • Enterprise hooks: SSO, audit logging, compliance reports         │
│                                                                     │
│ Success Metric: 10 design partners, validated pricing model        │
└─────────────────────────────────────────────────────────────────────┘
```

---

## Part 4: Detailed Technical Recommendations

### 4.1 Immediate: Wire Polyglot Gates into Loop

**The Gap**: `LoopManager` creates `RealQualityChecker` without language information:

```rust
// Current (src/loop/manager/mod.rs)
impl LoopDependencies {
    pub fn real(project_dir: PathBuf) -> Self {
        Self {
            git: Arc::new(RealGitOperations::new(project_dir.clone())),
            claude: Arc::new(RealClaudeProcess::new(project_dir.clone())),
            fs: Arc::new(RwLock::new(RealFileSystem::new(project_dir.clone()))),
            quality: Arc::new(RealQualityChecker::new(project_dir)), // ← No language info!
        }
    }
}
```

**The Fix**:

```rust
// Proposed
impl LoopDependencies {
    pub fn real_polyglot(project_dir: PathBuf) -> Self {
        // 1. Detect languages
        let detector = LanguageDetector::new(&project_dir);
        let languages: Vec<Language> = detector.detect()
            .into_iter()
            .filter(|d| d.confidence >= 0.10)
            .map(|d| d.language)
            .collect();
        
        // 2. Get available gates for detected languages
        let gates = detect_available_gates(&project_dir, &languages);
        
        // 3. Create checker with language-specific gates
        let quality = RealQualityChecker::with_gates(project_dir.clone(), gates);
        
        Self {
            git: Arc::new(RealGitOperations::new(project_dir.clone())),
            claude: Arc::new(RealClaudeProcess::new(project_dir.clone())),
            fs: Arc::new(RwLock::new(RealFileSystem::new(project_dir.clone()))),
            quality: Arc::new(quality),
        }
    }
}
```

### 4.2 Prompt Assembly: Language-Aware Sections

**Current State**: `PROMPT_build.md` has hardcoded Rust commands.

**Fix**: The `TemplateRegistry::get_polyglot_prompt()` already exists—ensure bootstrap and the loop use it:

```rust
// In PromptAssembler
pub fn build(&self, mode: LoopMode, languages: &[Language], task: &Task) -> String {
    let registry = TemplateRegistry::new();
    
    let base_prompt = if languages.len() > 1 {
        registry.get_polyglot_prompt(TemplateKind::PromptBuild, languages)
    } else {
        registry.get(TemplateKind::PromptBuild, languages[0])
    };
    
    // Inject task, antipatterns, remediation as before
    self.inject_dynamic_sections(base_prompt, task)
}
```

### 4.3 Predictor Integration: Act on Preventive Actions

**Current State**: Predictor calculates `PreventiveAction` but it's not fully acted upon.

**Fix**: Wire actions into the loop:

```rust
// In LoopManager::run()
let action = predictor.preventive_action(&risk_signals, risk_score);

match action {
    PreventiveAction::InjectGuidance { guidance } => {
        prompt_extras.push(guidance);
    },
    PreventiveAction::FocusTask { task } => {
        self.task_tracker.force_focus(&task);
    },
    PreventiveAction::RunTests => {
        // Force a test run before proceeding
        self.deps.quality.run_tests_only()?;
    },
    PreventiveAction::SuggestCommit => {
        // If we have passing gates but haven't committed, do so now
        if self.deps.quality.gates_passing()? {
            self.deps.git.commit("Checkpoint: predictor suggested commit")?;
        }
    },
    PreventiveAction::SwitchMode { target } => {
        self.state.mode = target.parse()?;
    },
    PreventiveAction::RequestReview { reason } => {
        return self.pause_for_human_review(&reason);
    },
    PreventiveAction::None => {},
}
```

### 4.4 Quality Gate Result Aggregation

For polyglot projects, aggregate results across languages:

```rust
pub struct PolyglotGateResult {
    pub by_language: HashMap<Language, Vec<GateResult>>,
    pub blocking_failures: Vec<GateResult>,
    pub warnings: Vec<GateResult>,
}

impl PolyglotGateResult {
    pub fn can_commit(&self) -> bool {
        self.blocking_failures.is_empty()
    }
    
    pub fn summary(&self) -> String {
        let mut parts = vec![];
        for (lang, results) in &self.by_language {
            let passed = results.iter().filter(|r| r.passed()).count();
            let total = results.len();
            parts.push(format!("{}: {}/{}", lang, passed, total));
        }
        parts.join(", ")
    }
}
```

---

## Part 5: Success Metrics & Validation

### 5.1 Phase 1 Validation: Polyglot Project Test

**Test Project**: A real-world polyglot setup:
- Frontend: Next.js + TypeScript
- Backend: FastAPI + Python
- Shared types: Generated from OpenAPI

**Acceptance Criteria**:
1. `ralph bootstrap` detects both TypeScript and Python
2. `ralph loop --phase build` runs ESLint, Jest, Tsc for frontend changes
3. `ralph loop --phase build` runs Ruff, Pytest, Mypy for backend changes
4. Quality gate failures produce appropriate remediation prompts
5. Commits only happen when all relevant gates pass

### 5.2 Reliability Metrics

| Metric | Current (Rust) | Target (Polyglot) |
|--------|----------------|-------------------|
| Stagnation Rate | <3% | <5% |
| False Positive Commits | 0% | <1% |
| Gate Execution Time | <30s | <45s |
| Predictor Accuracy | ~70% | >80% |

### 5.3 Adoption Metrics (Phase 3)

| Metric | Month 1 | Month 3 | Month 6 |
|--------|---------|---------|---------|
| GitHub Stars | 500 | 1,500 | 5,000 |
| Weekly Active Users | 100 | 500 | 2,000 |
| Community PRs | 10 | 50 | 200 |
| Languages Supported | 6 | 10 | 15 |

---

## Part 6: Risk Analysis & Mitigations

### 6.1 Technical Risks

| Risk | Probability | Impact | Mitigation |
|------|-------------|--------|------------|
| Gate false negatives | Medium | High | Extensive test suites for each gate |
| Context window overflow | Medium | Medium | Language-aware prioritization |
| Claude API changes | Low | High | Abstract Claude client, enable multi-model |
| Performance degradation | Medium | Medium | Benchmark suite, caching |

### 6.2 Strategic Risks

| Risk | Probability | Impact | Mitigation |
|------|-------------|--------|------------|
| Competitor leap | Medium | High | Move fast, community moat |
| Model commoditization | High | Medium | Value is in orchestration, not model |
| Adoption stalls | Medium | High | Documentation investment, examples |
| Open source fragmentation | Low | Medium | Clear contribution guidelines |

---

## Part 7: Architectural Decisions Record

### ADR-001: Polyglot Gate Wiring Strategy

**Context**: Quality gates exist for multiple languages but aren't integrated into the loop.

**Decision**: Wire language detection results into `LoopDependencies::real_polyglot()` at loop initialization time, not dynamically per-iteration.

**Rationale**: 
- Language composition rarely changes mid-session
- Initialization-time detection is simpler and more testable
- Gates can be cached/warmed at startup

**Consequences**:
- Mixed-language commits will run all relevant gates
- Gate availability is checked once, not repeatedly
- New language files require loop restart (acceptable tradeoff)

### ADR-002: Predictor Action Enforcement

**Context**: Predictor calculates actions but doesn't enforce them.

**Decision**: Predictor actions become soft recommendations injected into prompts, not hard constraints.

**Rationale**:
- Hard constraints could block legitimate progress
- LLM can incorporate guidance intelligently
- Preserves human-in-the-loop for critical decisions

**Consequences**:
- `InjectGuidance` adds text to prompt, doesn't force behavior
- `SuggestCommit` is a hint, not automatic
- `RequestReview` is the only hard stop

### ADR-003: Template Composition Strategy

**Context**: Polyglot projects need combined prompts from multiple language templates.

**Decision**: Use `TemplateRegistry::get_polyglot_prompt()` which concatenates language-specific sections with a polyglot header.

**Rationale**:
- Simpler than runtime template merging
- Each language section is self-contained
- Header establishes context for mixed-language work

**Consequences**:
- Prompt length grows with language count (monitor context)
- Duplicate guidance possible (acceptable, adds emphasis)
- Clear structure for Claude to parse

---

## Conclusion: The Path to Dominance

Ralph is not a tool that needs to be rebuilt—it's a tool that needs to be finished. The architecture is exceptional. The abstractions are clean. The quality gate system is best-in-class. The stagnation predictor is genuinely innovative.

**The 20% remaining work**:
1. Wire polyglot gates into the loop (2 weeks)
2. Validate end-to-end on real polyglot project (1 week)
3. Full predictor action enforcement (1 week)
4. Documentation and examples (ongoing)

**The destiny**: Ralph becomes the de-facto standard for AI-assisted development because it's the only tool that provably improves code quality over time, works reliably across languages, and respects developers' existing toolchains.

The spirit of "livin' la vida locum" demands nothing less—work efficiently so you can live fully.

---

*Document prepared for strategic planning. Next step: Build the phased TDD implementation plan for Phase 1.*
