2 task(s) completed:

✅ **plan** (task plan-eb2):
# Research Phase 2 Implementation Plan

**Scope:** `crates/ragent-research` core refactor, correctness fixes, and test migration.  
**Source:** `RESEARCHPLAN.md` §3.2 (lines 351–368) and task-table rows **R-005**–**R-014**, **R-027**, **R-028**.  
**Output target:** `docs/research-phase2-plan.md`.

---

## 1. Goal and Success Criteria

| # | Success Criterion | How Measured |
|---|-------------------|--------------|
| 1 | `session.rs` no longer contains a single ~1,200-line method | `ResearchSession::run` reduced to a thin orchestrator that delegates to named stage helpers |
| 2 | No inline `#[cfg(test)]` modules remain in library source | `grep` across `crates/ragent-research/src/**/*.rs` finds zero `mod tests` |
| 3 | Synthesis critic outputs 1-based finding numbers | Regression test checks critic issue strings for `"Finding 1"` not `"Finding 0"` |
| 4 | Contradiction dimensions are configurable | `ContradictionConfig` exists, is wired through `SessionConfig`, and defaults preserve current behaviour |
| 5 | Reconcile conflicting-edge count is accurate | Regression test with shared sources + graph edges verifies `conflicting_edges > 0` |

---

## 2. Task Inventory

| ID | Area | Task | Priority | Dependency |
|----|------|------|----------|------------|
| R-005 | Architecture | Extract stage helpers from `ResearchSession::run` | P0 | — |
| R-006 | Architecture | Add `TierRouter::run_step_if` and convert all dispatch sites | P0 | R-005 |
| R-007 | Architecture | Restructure `SessionConfig` into nested sub-configs | P1 | R-001 (already done) |
| R-008 | Architecture | Introduce nested `SessionEvent` hierarchy | P1 | — |
| R-009 | Correctness | Fix 0-based finding numbers in `synthesis.rs` | P0 | — |
| R-010 | Correctness | Make contradiction dimensions configurable via `ContradictionConfig` / `PolarityDimension` | P0 | — |
| R-011 | Correctness | Fix reconcile conflicting-edge count to use shared source indices | P0 | R-010 |
| R-012 | Performance | Cache compiled regexes with `OnceLock` in `cite_checker`, `verify`, `synthesis`, `diagram` | P1 | — |
| R-013 | Correctness | Invert `CorpusCriticReport.balance_score` or rename to `dominance_score` | P1 | — |
| R-014 | Maintainability | Move shared polarity helpers (`source_body_text`, `depth_from_count`) into internal module | P1 | R-010 |
| R-027 | Maintainability | Convert `AnalysisEngine` from trait marker to enum or remove marker | P2 | — |
| R-028 | Maintainability | Migrate inline tests to `crates/ragent-research/tests/` | P1 | — |

---

## 3. Detailed Implementation Plan

### 3.1 R-005 — Extract stage helpers from `ResearchSession::run`

**Current state:** `ResearchSession::run` in `crates/ragent-research/src/session.rs` is ~1,170 lines (lines 889–2059) and contains setup, `--from-url`, `--from-file`, web/local gather, synthesis, every tier step, assembly, and finalization.

**Target structure:** `run` becomes a ~150-line async orchestrator that calls private async helper methods. Each helper owns one logical stage, accepts the mutable state it needs, and returns stage-specific output.

**Proposed helpers (all private `impl ResearchSession` methods):**

```rust
async fn run_setup(
    &self,
    name_str: &str,
    title: &str,
    config: &SessionConfig,
    observer: Arc<dyn SessionObserver>,
) -> Result<(ResearchName, ResearchItem, String)>;

async fn run_from_url_seed(
    &self,
    config: &SessionConfig,
    observer: Arc<dyn SessionObserver>,
) -> Result<(Vec<Source>, Vec<String>, String)>;

async fn run_from_file_seed(
    &self,
    config: &SessionConfig,
    observer: Arc<dyn SessionObserver>,
) -> Result<(Vec<Source>, Vec<String>, String)>;

async fn run_web_gather(
    &self,
    topic: &str,
    config: &SessionConfig,
    observer: Arc<dyn SessionObserver>,
) -> Result<GatherResult>;

async fn run_local_gather(
    &self,
    project_root: &Path,
    topic: &str,
    config: &SessionConfig,
    observer: Arc<dyn SessionObserver>,
) -> Result<Vec<Source>>;

async fn run_analysis_steps(
    &self,
    sources: &[Source],
    topic: &str,
    router: &mut TierRouter,
    router_observer: &TierRouterToSessionObserver,
    config: &SessionConfig,
    observer: Arc<dyn SessionObserver>,
) -> Result<AnalysisArtifacts>;

struct AnalysisArtifacts {
    contradiction_graph: Option<ContradictionGraph>,
    loci: LocusSet,
    depth_investigation: Vec<DepthInvestigation>,
    cross_locus_reconcile: CrossLocusReconcile,
    source_tensions: SourceTensions,
    evidence_digest: EvidenceDigest,
    corpus_critic: CorpusCriticReport,
    gap_fetch: GapFetchResult,
    triple_draft: TripleDraft,
}
```

**Concrete edits in `session.rs`:**
1. Add `AnalysisArtifacts` struct definition after `RunOutcome` (around line 2170).
2. Move the `--from-url` loop (lines 941–1077) into `run_from_url_seed`.
3. Move the `--from-file` loop (lines 1089–1195) into `run_from_file_seed`.
4. Move the item creation block (lines 1198–1208) into `run_setup`.
5. Move the overlapped web/local gather block (lines 1288–1432) into `run_web_gather` + `run_local_gather`.
6. Move the tier-router analysis steps (lines 1462–1639) into `run_analysis_steps`.
7. Move the synthesis + patch + cite-check + polish + readability block (lines 1672–1853) into a `run_synthesis_pipeline` helper.
8. Move assembly/finalize (lines 1855–2057) into `run_assemble_and_finalize`.

**Verification:**
- `cargo check -p ragent-research` passes.
- `cargo test -p ragent-research` passes.
- `ResearchSession::run` line count drops below 200 lines.

---

### 3.2 R-006 — Add `TierRouter::run_step_if` helper and convert dispatch sites

**Current state:** Every tier step is dispatched with the same 6-line boilerplate:

```rust
if let Some(step) = router.next_step()
    && step == RunStep::ContradictionGraph
{
    router.start_step(RunStep::ContradictionGraph, &router_observer);
    let graph = build_contradiction_graph(&sources);
    observer.on_event(SessionEvent::ContradictionGraph { ... });
    router.finish_step(RunStep::ContradictionGraph, &router_observer);
    Some(graph)
} else { None }
```

There are ~15 such blocks in `session.rs`.

**Target API in `tier_router.rs`:**

```rust
impl TierRouter {
    /// Run `f` if `next_step()` returns `expected`. Handles start/finish events.
    /// Returns `Some(f())` when the step ran, `None` when it was not next or skipped.
    pub async fn run_step_if<T, F, Fut>(
        &mut self,
        expected: RunStep,
        observer: &dyn TierRouterObserver,
        f: F,
    ) -> Option<T>
    where
        F: FnOnce() -> Fut,
        Fut: std::future::Future<Output = T>;
}
```

Because `start_step`/`finish_step` emit observer events and the closure body often emits `SessionEvent`s, the helper must accept an async closure. To avoid a lifetime mess, prefer a synchronous version for non-async work plus an async version:

```rust
pub fn run_step_if<T>(
    &mut self,
    expected: RunStep,
    observer: &dyn TierRouterObserver,
    f: impl FnOnce() -> T,
) -> Option<T>;

pub async fn run_step_if_async<T>(
    &mut self,
    expected: RunStep,
    observer: &dyn TierRouterObserver,
    f: impl AsyncFnOnce() -> T,
) -> Option<T>;
```

*Decision needed:* Rust 2024 edition supports `AsyncFnOnce` traits. If the project is already on edition 2024, use the single `async fn run_step_if` signature with `impl AsyncFnOnce() -> T`.

**Concrete conversions in `session.rs`:**

Replace the `ContradictionGraph` block with:

```rust
let contradiction_graph = router
    .run_step_if(RunStep::ContradictionGraph, &router_observer, || async {
        let graph = build_contradiction_graph(&sources, config.analysis.contradiction.clone());
        observer.on_event(SessionEvent::Analysis {
            analysis: AnalysisEvent::ContradictionGraph {
                sources_scanned: sources.len(),
                edges: graph.edges.clone(),
            },
        });
        graph
    })
    .await;
```

Do the same for `LociAnalysis`, `DepthInvestigation`, `CrossLocusReconcile`, `SourceTensions`, `EvidenceDigest`, `CorpusCritic`, `GapFetch`, `TripleDraft`, `Synthesize`, `Critics`, `Patcher`, `CiteCheck`, `Polish`, `ReadabilityAudit`.

**Verification:**
- Count of `if let Some(step) = router.next_step()` strings in `session.rs` drops from ~15 to 0.
- All tests pass; no behavioural change.

---

### 3.3 R-007 — Restructure `SessionConfig` into nested sub-configs

**Current state:** `SessionConfig` is a flat struct with ~35 fields (lines 163–299 of `session.rs`). It mixes topic inputs, I/O paths, gathering knobs, resilience knobs, output format, tier/depth, and OA recovery.

**Target grouping:**

```rust
pub struct SessionConfig {
    pub input: InputConfig,
    pub output: OutputConfig,
    pub web: WebConfig,
    pub local: LocalConfig,
    pub analysis: AnalysisConfig,
    pub resilience: ResilienceConfig,
    pub engine: EngineConfig, // renamed from depth/iterations/tier
}

pub struct InputConfig {
    pub topic: String,
    pub from_urls: Vec<String>,
    pub from_files: Vec<PathBuf>,
    pub sources_dir: Option<PathBuf>,
}

pub struct OutputConfig {
    pub template: Option<String>,
    pub output_format: OutputFormat,
}

pub struct WebConfig {
    pub max_web_results: usize,
    pub fetch_concurrency: usize,
    pub fetch_timeout_secs: u64,
    pub use_low_relevance: bool,
    pub disable_scholarly: bool,
    pub use_pdf_web_sources: bool,
    pub open_access_recovery: bool,
    pub contact_email: Option<String>,
    pub oa_min_full_text_chars: usize,
}

pub struct LocalConfig {
    pub max_local_sources: usize,
    pub disable_local: bool,
    pub disable_specs: bool,
    pub local_concurrency: usize,
}

pub struct AnalysisConfig {
    pub max_synthesis_sources: Option<usize>,
    pub contradiction: ContradictionConfig,
}

pub struct ResilienceConfig {
    pub web_phase_timeout_secs: Option<u64>,
    pub local_phase_timeout_secs: Option<u64>,
    pub search_max_retries: u32,
    pub search_retry_base_delay_ms: u64,
    pub search_circuit_breaker_threshold: u32,
}

pub struct EngineConfig {
    pub depth: Option<Depth>,
    pub iterations: Option<u32>,
    pub tier: Tier,
}
```

*Note:* This conflicts with the existing `crate::engine::EngineConfig`. Rename the session-level one to `RunEngineConfig` or move `crate::engine::EngineConfig` to `crate::engine::IterativeEngineConfig`.

**Concrete edits:**
1. In `session.rs`, replace the flat `SessionConfig` struct with the nested version.
2. Implement `Default` for each sub-config.
3. Update `impl SessionConfig` methods `engine_config`, `budget_web_results`, `budget_local_sources` to read nested fields.
4. Update `run_request.rs` `build_session_config` to populate nested fields.
5. Update every `config.*` access site in `session.rs`, `cli.rs`, TUI, and server.
6. Keep serde compatibility by adding `#[serde(flatten)]` on each nested field if serialised externally.

**Verification:**
- `cargo check -p ragent-research` passes.
- `cargo check --workspace` passes.
- Existing CLI/TUI/HTTP tests pass.
- Add a regression test that builds `SessionConfig::default()` and asserts each nested default matches the old flat default.

---

### 3.4 R-008 — Introduce nested `SessionEvent` hierarchy

**Current state:** `SessionEvent` has 30+ flat variants (lines 403–696). Front-ends must match many unrelated branches.

**Target hierarchy:**

```rust
pub enum SessionEvent {
    Lifecycle(LifecycleEvent),
    Gather(GatherEvent),
    Analysis(AnalysisEvent),
    Synthesis(SynthesisEvent),
    Audit(AuditEvent),
    Done(DoneEvent),
}

pub enum LifecycleEvent {
    Phase { phase: SessionPhase },
    ConfigSnapshot { output_format: String, depth: Option<String>, iterations: Option<u32>, tier: Option<String>, from_urls: Vec<String>, from_files: Vec<String> },
}

pub enum GatherEvent {
    QueriesDecomposed { queries: Vec<String> },
    WebCaptured { url: String, title: String, search_tool: String, search_engine: String, body_preview: String, language: String, oa_recovery: Option<Box<crate::open_access::RecoveredOpenAccess>> },
    FromUrlBodyPreview { url: String, body_preview: String },
    FromFileBodyPreview { path: String, body_preview: String },
    LocalCaptured { path: String, score: usize },
    SpecCaptured { spec_id: String },
    WebSearchFailed { error: String },
    WebFetchFailed { url: String, error: String },
    SourceFailed { source: Option<String>, error: String },
}

pub enum AnalysisEvent {
    PlanUpdated { sub_questions: Vec<String> },
    SubQuestionStatusChanged { id: String, status: String },
    ContradictionGraph { edges: Vec<ContradictionEdge>, sources_scanned: usize },
    LociAnalysis { loci: LocusSet, sources_scanned: usize },
    DepthInvestigation { investigations: Vec<DepthInvestigation> },
    CrossLocusReconcile { reconcile: CrossLocusReconcile },
    SourceTensions { tensions: SourceTensions },
    EvidenceDigest { digest: EvidenceDigest },
    CorpusCritic { report: CorpusCriticReport },
    GapFetch { result: GapFetchResult },
    TripleDraft { draft: TripleDraft },
}

pub enum SynthesisEvent {
    SynthesizeResult { outcome: SynthesizeOutcome, detail: Option<String> },
    SurgicalPatch { result: PatchResult },
    Polish { result: PolishResult },
}

pub enum AuditEvent {
    CriticResult { score: Option<u32>, gaps: Vec<String> },
    VerificationResult { passed: bool, issues: Vec<String> },
    CiteCheck { result: CitationCheckResult },
    ReadabilityAudit { result: ReadabilityAudit },
    SynthesisAudit { audit: SynthesisAudit },
}

pub enum DoneEvent {
    RunSummary { total_sources: usize, pdf_count: usize, youtube_count: usize, excluded_count: usize },
}
```

**Compatibility strategy:** Because TUI, CLI, and server pattern-match on `SessionEvent`, introduce the nested enum under a new name and keep deprecated top-level aliases during transition, OR update all three front-ends in the same PR.

Recommended: update all front-ends in one pass since the task explicitly requires "Observers compile and match only needed branches."

**Concrete edits:**
1. Replace `SessionEvent` enum in `session.rs` with the nested structure.
2. Update every `observer.on_event(SessionEvent::Variant { ... })` in `session.rs` to the new nested form.
3. Update `GatherEventForwarder::on_event` to emit `SessionEvent::Gather(GatherEvent::*)`.
4. Update `TierRouterToSessionObserver` (in `tier_router.rs`) to convert step events to `SessionEvent::Lifecycle`.
5. Update `crates/ragent-research/src/cli.rs` `render_session_event_json` to match nested structure.
6. Update TUI (`crates/ragent-tui/src/app/research.rs`) match arms.
7. Update server (`crates/ragent-server/src/routes/research.rs`) event streaming.

**Verification:**
- `cargo check --workspace` passes.
- `cargo test --workspace` passes.
- TUI/CLI/server integration tests pass.

---

### 3.5 R-009 — Fix 0-based finding numbers in `synthesis.rs`

**Current state:** `evidence_critic` in `crates/ragent-research/src/synthesis.rs` uses `idx` directly:

```rust
for (idx, finding) in analysis.findings.iter().enumerate() {
    ...
    issues.push(format!("Finding {idx} cites out-of-range source #{n} ..."));
    issues.push(format!("Finding {idx} does not cite any source"));
}
```

**Fix:** Change every `Finding {idx}` to `Finding {}` with `idx + 1`.

**Concrete edits in `synthesis.rs` (lines 273–295):**

```rust
let display_idx = idx + 1;
issues.push(format!(
    "Finding {display_idx} cites out-of-range source #{n} (only {} sources available)",
    sources.len()
));
issues.push(format!("Finding {display_idx} does not cite any source"));
gaps.push(format!("Add a supporting source citation to finding {display_idx}"));
```

Also update `readability_critic` messages at lines 349, 358 if they use `idx` for finding numbers.

**Verification:**
- Add regression test in `crates/ragent-research/tests/test_synthesis.rs`:

```rust
#[test]
fn evidence_critic_uses_one_based_finding_numbers() {
    let sources = vec![web_source("body")];
    let analysis = AnalysisResult {
        findings: vec![valid_finding(1), valid_finding(2)],
        ..Default::default()
    };
    let report = evidence_critic(&sources, &analysis);
    assert!(report.issues.iter().any(|i| i.contains("Finding 1")));
    assert!(!report.issues.iter().any(|i| i.contains("Finding 0")));
}
```

---

### 3.6 R-010 — Make contradiction dimensions configurable via `ContradictionConfig` / `PolarityDimension`

**Current state:** `POLARITY_DIMENSIONS` is a hard-coded `const` array in `contradiction.rs` (lines 126–212) with medical/tech dimensions.

**Target design in `contradiction.rs`:**

```rust
/// Configurable polarity dimension.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct PolarityDimension {
    pub name: String,
    pub positive_tokens: Vec<String>,
    pub negative_tokens: Vec<String>,
    pub positive_label: String,
    pub negative_label: String,
}

impl PolarityDimension {
    pub fn new(
        name: impl Into<String>,
        positives: &[&str],
        negatives: &[&str],
        positive_label: impl Into<String>,
        negative_label: impl Into<String>,
    ) -> Self { ... }
}

/// Configuration for contradiction detection.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ContradictionConfig {
    pub dimensions: Vec<PolarityDimension>,
    pub min_body_len: usize,
}

impl Default for ContradictionConfig {
    fn default() -> Self {
        Self {
            dimensions: vec![
                PolarityDimension::new("effect", &["improves", "benefits", ...], &["worsens", ...], "benefit / risk reduction", "harm / risk increase"),
                // ... all six current dimensions ...
            ],
            min_body_len: 20,
        }
    }
}
```

**Change `build_contradiction_graph` signature:**

```rust
pub fn build_contradiction_graph(
    sources: &[Source],
    config: ContradictionConfig,
) -> ContradictionGraph
```

**Remove hard-coded `POLARITY_DIMENSIONS`, `positive_label`, `negative_label` free functions.** Replace with methods on `PolarityDimension`.

**Concrete edits in `contradiction.rs`:**
1. Add `PolarityDimension` and `ContradictionConfig` structs.
2. Convert the six hard-coded dimensions into `Default` dimensions.
3. Update `build_contradiction_graph` to accept `config` and iterate `config.dimensions`.
4. Update all internal label generation to use dimension labels from config.
5. Update `lib.rs` re-exports.

**Wiring through `SessionConfig`:** Add `pub contradiction: ContradictionConfig` to the new `AnalysisConfig` nested struct. Default it.

**Verification:**
- Add test in `crates/ragent-research/tests/test_contradiction.rs`:

```rust
#[test]
fn custom_dimension_detects_non_medical_contradiction() {
    let dim = PolarityDimension::new("price", &["cheaper", "lower price"], &["expensive", "higher price"], "cheaper", "more expensive");
    let cfg = ContradictionConfig { dimensions: vec![dim], min_body_len: 5 };
    let sources = vec![
        web_source(1, "Product A is cheaper."),
        web_source(2, "Product A is more expensive."),
    ];
    let graph = build_contradiction_graph(&sources, cfg);
    assert_eq!(graph.edges.len(), 1);
    assert_eq!(graph.edges[0].dimension, "price");
}
```

---

### 3.7 R-011 — Fix reconcile conflicting-edge count to use shared source indices

**Current state:** `build_cross_locus_reconcile` in `reconcile.rs` (lines 170–182) only counts an edge as conflicting if its `dimension` matches one of the two locus labels:

```rust
&& (a.label.to_lowercase() == e.dimension.to_lowercase()
    || b.label.to_lowercase() == e.dimension.to_lowercase())
```

This almost never matches because locus labels are derived from corpus keywords, not contradiction dimensions.

**Fix:** Count any contradiction-graph edge whose *both* source indices are in the shared source set for the locus pair. Remove the dimension-label filter.

```rust
let conflicting_edges = graph
    .map(|g| {
        g.edges
            .iter()
            .filter(|e| {
                shared.contains(&e.claim_a.source_index)
                    && shared.contains(&e.claim_b.source_index)
            })
            .count()
    })
    .unwrap_or(0);
```

**Concrete edits in `reconcile.rs`:**
1. Replace the filter closure at lines 174–179 with the shared-source-only check.
2. Update the `note` text to say "shared sources also hold opposing claims".

**Verification:**
- Add regression test in `crates/ragent-research/tests/test_reconcile.rs`:

```rust
#[test]
fn reconcile_counts_conflicting_edges_between_shared_sources() {
    let loci = LocusSet {
        loci: vec![
            locus("performance", &[1, 2]),
            locus("cost", &[2, 3]),
        ],
    };
    let mut graph = ContradictionGraph::empty();
    graph.add_edge(edge(1, 2, "performance")); // both shared between performance and cost
    let reconcile = build_cross_locus_reconcile(&loci, Some(&graph), 3);
    let pair = &reconcile.pairs[0];
    assert_eq!(pair.shared_sources, 1); // source 2
    assert!(pair.conflicting_edges > 0, "expected conflicting edge between shared sources");
}
```

---

### 3.8 R-012 — Cache compiled regexes with `OnceLock`

**Current state:** `synthesis.rs` already uses `OnceLock` for `CITATION_RE`. Other modules may still recompile. Verify and add caches in `cite_checker.rs`, `verify.rs`, and `diagram.rs`.

**Concrete edits per module:**

```rust
// cite_checker.rs
static CITE_RE: OnceLock<Regex> = OnceLock::new();
fn cite_re() -> &'static Regex {
    CITE_RE.get_or_init(|| Regex::new(r"\[#(\d+)\]").expect("valid regex"))
}
```

```rust
// verify.rs
static KEYWORD_SPLIT_RE: OnceLock<Regex> = OnceLock::new();
fn keyword_split_re() -> &'static Regex { ... }
```

```rust
// diagram.rs
static FINDING_RE: OnceLock<Regex> = OnceLock::new();
fn finding_re() -> &'static Regex { ... }
```

**Verification:**
- `cargo clippy -p ragent-research` with lints for `Regex::new` in loops passes.
- Add micro-test asserting the same regex pointer is returned on repeated calls.

---

### 3.9 R-013 — Invert `CorpusCriticReport.balance_score` or rename to `dominance_score`

**Current state:** Unknown exact semantics; inspect `corpus_critic.rs`. If `balance_score` currently rewards imbalance, either invert the formula or rename the field to `dominance_score` and update all renderers.

**Concrete steps:**
1. Read `crates/ragent-research/src/corpus_critic.rs` to confirm semantics.
2. If imbalance currently scores high → rename field to `dominance_score`, update document rendering, tests, and JSON schema.
3. If the intent is balance but formula is inverted → invert formula (`100 - current`) and update tests.

**Verification:**
- Tests in `corpus_critic.rs` (moved to `tests/`) assert expected direction.
- Golden-file tests if any.

---

### 3.10 R-014 — Move shared polarity helpers into internal module

**Current state:** `source_body_text` is duplicated in `contradiction.rs` and likely used in `locus.rs`/`reconcile.rs`. `depth_from_count` is in `reconcile.rs` but needed by `locus.rs`.

**Target:** Create `crates/ragent-research/src/source_utils.rs` (private module) containing:

```rust
pub(crate) fn source_body_text(source: &Source) -> String;
pub(crate) fn depth_from_count(n: usize) -> DepthLevel;
```

**Concrete edits:**
1. Create `source_utils.rs` with the two helpers.
2. Replace `contradiction.rs` local `source_body_text` with a call to `crate::source_utils::source_body_text`.
3. Replace `reconcile.rs` local `depth_from_count` with a call to `crate::source_utils::depth_from_count`.
4. Update any other internal callers (`locus.rs`, `digest.rs`, etc.) to use the shared helper.
5. Do **not** add `source_utils` to `lib.rs` public exports.

**Verification:**
- `cargo check -p ragent-research` passes.
- Tests pass.

---

### 3.11 R-027 — Convert `AnalysisEngine` from trait marker to enum or remove marker

**Current state:** `AnalysisEngine` trait has an `is_noop_marker()` method solely so `ResearchSession::analysis_is_noop()` can detect `NoopAnalysisEngine`. This leaks implementation detail into the public trait.

**Target options:**
1. **Preferred:** Add an enum wrapper:

```rust
pub enum AnalysisEngineKind {
    Llm(Arc<LlmAnalysisEngine>),
    Noop,
}
```

2. **Alternative:** Remove the marker and make `ResearchSession` store `Option<Arc<LlmAnalysisEngine>>` plus a separate `use_analysis: bool` flag.

**Concrete edits in `analysis.rs`:**
1. Introduce `AnalysisEngineKind` enum or make `AnalysisEngine` not require `is_noop_marker`.
2. Update `ResearchSession` to store `analysis: AnalysisEngineKind` instead of `Arc<dyn AnalysisEngine>`.
3. Update `ResearchSession::new` and `analysis_is_noop()`.
4. Remove `is_noop_marker` from the trait.
5. Update `lib.rs` re-exports.

**Verification:**
- `cargo check --workspace` passes.
- All tests pass.

---

### 3.12 R-028 — Migrate inline tests to `crates/ragent-research/tests/`

**Current inline test modules to migrate:**

| Source file | `mod tests` line | Approx tests | New test file |
|-------------|------------------|--------------|---------------|
| `session.rs` | 2231 | ~30 | `tests/test_session.rs` |
| `contradiction.rs` | 377 | 7 | `tests/test_contradiction.rs` |
| `synthesis.rs` | 481 | 9 | `tests/test_synthesis.rs` |
| `reconcile.rs` | 321 | 6 | `tests/test_reconcile.rs` |
| `tier_router.rs` | 311 | 7 | `tests/test_tier_router.rs` |

**Migration rules from `AGENTS.md`:**
- Public-API tests: `use ragent_research::module::Item;`.
- Private-item tests: widen tested items to `pub(crate)` and re-import source module via `#[path = "../src/<module>.rs"] mod <module>;` with shims for `super::` and `crate::`.

**Concrete steps:**
1. Create `crates/ragent-research/tests/` directory.
2. Move each inline `mod tests` into a new file.
3. For tests touching private helpers:
   - Make those helpers `pub(crate)` where necessary.
   - In the test file, declare `#[path = "../src/<module>.rs"] mod inner;` and add `use inner::*;`.
   - Provide shims: `mod crate { pub use crate::*; }` may be needed for `crate::` references inside the source file when it is imported as a module.
4. Remove all `#[cfg(test)] mod tests` blocks from library source.
5. Run `cargo test -p ragent-research`.

**Special handling for `session.rs` tests:** Many tests use private test structs (`FakeSearch`, `FakeFetch`, etc.) that are only used in tests. Move them into `tests/test_session.rs` as well.

**Verification:**
- `grep -R "mod tests" crates/ragent-research/src` returns nothing.
- `cargo test -p ragent-research` passes.

---

## 4. File-by-File Edit Summary

| File | Lines touched | Nature of change |
|------|---------------|------------------|
| `crates/ragent-research/src/session.rs` | 1–4528 | Reorganise `SessionConfig`, `SessionEvent`, extract helpers, remove inline tests |
| `crates/ragent-research/src/tier_router.rs` | 120–280 | Add `run_step_if` / `run_step_if_async`, remove inline tests |
| `crates/ragent-research/src/contradiction.rs` | 1–517 | Add `PolarityDimension`, `ContradictionConfig`, parameterise builder, remove inline tests |
| `crates/ragent-research/src/synthesis.rs` | 200–360 | Fix 0-based numbers, remove inline tests |
| `crates/ragent-research/src/reconcile.rs` | 142–220 | Fix conflicting-edge filter, remove inline tests |
| `crates/ragent-research/src/corpus_critic.rs` | TBD | Invert balance or rename field |
| `crates/ragent-research/src/cite_checker.rs` | TBD | `OnceLock` regex cache |
| `crates/ragent-research/src/verify.rs` | TBD | `OnceLock` regex cache |
| `crates/ragent-research/src/diagram.rs` | TBD | `OnceLock` regex cache |
| `crates/ragent-research/src/analysis.rs` | TBD | Remove `is_noop_marker`, add enum or option |
| `crates/ragent-research/src/source_utils.rs` | new | Shared `source_body_text`, `depth_from_count` |
| `crates/ragent-research/src/lib.rs` | 81–167 | Update re-exports |
| `crates/ragent-research/src/run_request.rs` | TBD | Build nested `SessionConfig` |
| `crates/ragent-research/src/cli.rs` | TBD | Update `render_session_event_json`, config access |
| `crates/ragent-tui/src/app/research.rs` | TBD | Update `SessionEvent` matches |
| `crates/ragent-server/src/routes/research.rs` | TBD | Update event streaming, config access |
| `crates/ragent-research/tests/*.rs` | new | Migrated + new regression tests |

---

## 5. Proposed `SessionEvent` Nesting (final shape)

```rust
pub enum SessionEvent {
    Lifecycle(LifecycleEvent),
    Gather(GatherEvent),
    Analysis(AnalysisEvent),
    Synthesis(SynthesisEvent),
    Audit(AuditEvent),
    Done(DoneEvent),
}
```

Front-end match example:

```rust
match event {
    SessionEvent::Lifecycle(LifecycleEvent::Phase { phase }) => { ... }
    SessionEvent::Gather(GatherEvent::WebCaptured { url, .. }) => { ... }
    SessionEvent::Analysis(AnalysisEvent::ContradictionGraph { edges, .. }) => { ... }
    SessionEvent::Synthesis(SynthesisEvent::SynthesizeResult { outcome, .. }) => { ... }
    SessionEvent::Audit(AuditEvent::CiteCheck { result }) => { ... }
    SessionEvent::Done(DoneEvent::RunSummary { total_sources, .. }) => { ... }
}
```

---

## 6. Proposed `SessionConfig` Nesting (final shape)

```rust
pub struct SessionConfig {
    pub input: InputConfig,
    pub output: OutputConfig,
    pub web: WebConfig,
    pub local: LocalConfig,
    pub analysis: AnalysisConfig,
    pub resilience: ResilienceConfig,
    pub engine: RunEngineConfig,
}

pub struct AnalysisConfig {
    pub max_synthesis_sources: Option<usize>,
    pub contradiction: ContradictionConfig,
}
```

---

## 7. Dependencies and Ordering

```text
R-007 (nested SessionConfig)
    → unblocks R-010 (ContradictionConfig placement)
R-010
    → unblocks R-011 (reconcile fix needs graph built with config)
    → unblocks R-014 (shared helpers used by new contradiction code)
R-005 (extract helpers)
    → unblocks R-006 (run_step_if applied to helpers)
R-008 (nested SessionEvent)
    → must be done together with TUI/server updates
R-009 / R-012 / R-013 / R-027
    → can proceed in parallel with above
R-028 (test migration)
    → should follow each source refactor so migrated tests still pass
```

**Suggested order:**
1. R-007 + R-010 (config restructure + configurable dimensions)
2. R-014 (shared helpers)
3. R-011 (reconcile fix)
4. R-005 + R-006 (stage helper extraction + `run_step_if`)
5. R-008 (nested events) with front-end updates
6. R-009, R-012, R-013, R-027 in parallel
7. R-028 migrate all inline tests last

---

## 8. Testing Strategy

| What | How |
|------|-----|
| Compilation | `cargo check --workspace` after each task |
| Unit tests | `cargo test -p ragent-research` |
| Lint | `cargo clippy --workspace -- -D warnings` |
| Format | `cargo fmt --check` |
| Regression for R-009 | `test_synthesis_one_based_finding_numbers` |
| Regression for R-010 | `test_custom_contradiction_dimension` |
| Regression for R-011 | `test_reconcile_counts_conflicting_edges` |
| Regression for R-007 | `test_session_config_nested_defaults_match_flat_defaults` |
| Integration | Existing `session_runs_end_to_end_and_writes_document` migrated to `tests/test_session.rs` |

---

## 9. Risks and Mitigations

| Risk | Impact | Mitigation |
|------|--------|------------|
| Nested `SessionEvent` breaks TUI/server compile | High | Update all three front-ends in the same PR; no intermediate merge |
| Nested `SessionConfig` breaks serde saved sessions | Medium | Add `#[serde(flatten)]` or a manual migration deserialization |
| Removing `AnalysisEngine` trait marker breaks downstream custom engines | Medium | Keep trait but make `is_noop_marker` optional/default false; prefer enum approach |
| Large `session.rs` refactor introduces subtle ordering bugs | High | Keep tests green after each extracted helper; do not change logic while moving code |

---

## 10. Deliverable Checklist

- [ ] `docs/research-phase2-plan.md` created (this document).
- [ ] All R-005–R-014, R-027, R-028 tasks have concrete file/line targets.
- [ ] Function signatures and struct designs specified.
- [ ] `SessionEvent` nesting hierarchy documented.
- [ ] `SessionConfig` nesting documented.
- [ ] `ContradictionConfig` / `PolarityDimension` design documented.
- [ ] Test migration strategy and regression tests documented.
- [ ] Dependencies and ordering documented.

---

---

✅ **explore** (task explore-):
Hi I'm RAgent and I have read Agents.md. Rust language instructions loaded. I'll explore the ragent-research crate systematically and report the Phase 2 findings without making changes.# ragent-research Phase 2 Implementation Findings Report

## Executive Summary

The `ragent-research` crate is functional but carries significant technical debt concentrated in `session.rs`, where the main orchestration method has grown to ~1,170 lines. The Phase 2 work identified in `RESEARCHPLAN.md` (R-005 through R-014, R-027, R-028) falls into four groups:

1. **Architecture / maintainability** — mega-method decomposition, god-struct split, event variant simplification, public API cleanup.
2. **Correctness / user-visible bugs** — 0-based synthesis finding numbers, hard-coded contradiction dimensions, broken cross-locus reconcile edge counting.
3. **Analysis / critic improvements** — trait marker clarity, balance-score naming.
4. **Build hygiene** — migration of inline tests to external files.

All findings below are ranked by user-facing impact and implementation risk, with exact file locations and concrete fix instructions. No code was modified during this exploration.

---

## High Impact

### H-1. `ResearchSession::run` is a single mega-method coupling all stage dispatch
- **File:** `crates/ragent-research/src/session.rs`
- **Lines:** `889`–`2058` (the `pub async fn run` body spans ~1,170 lines)
- **Observation:** Every pipeline stage (gather, contradiction graph, loci, reconcile, synthesis, patcher, cite check, polish, readability audit, assembly) is inlined inside one method. The same `if let Some(step) = router.next_step() && step == RunStep::X { router.start_step(...); ...; router.finish_step(...) }` boilerplate repeats ~15 times. Error handling, observer events, source mutation, and tier-router bookkeeping are all interleaved.
- **Concrete fix (R-005):**
  - Extract a `SessionStage` enum/trait and a `StageContext` struct holding `&mut Vec<Source>`, `&mut ResearchItem`, `&SessionConfig`, `&TierRouterObserver`, etc.
  - Move each stage body into its own `async fn run_<stage>(ctx: &mut StageContext, ...) -> Result<StageOutput>` helper under a new `session/stages.rs` submodule.
  - Keep only orchestration in `run`: setup, stage dispatch loop, and final assembly.
  - Introduce a small `StageRunner` that wraps `router.start_step` / `finish_step` / `skip_step` so each stage implementation only calls `runner.run(step, work_fn).await`.
- **Verification:** `cargo test -p ragent-research` must pass; each extracted helper should be unit-testable with fake observers.

### H-2. `SessionConfig` is a "god struct" mixing unrelated concerns
- **File:** `crates/ragent-research/src/session.rs`
- **Lines:** `163`–`366`
- **Observation:** `SessionConfig` contains topic inputs, I/O paths, gathering knobs (`max_web_results`, `max_local_sources`), resilience knobs (`search_max_retries`, `web_phase_timeout_secs`), output format, depth/iterations, tier, and OA recovery settings all in one struct.
- **Concrete fix (R-006):**
  - Split into four focused structs:
    - `TopicInput { topic, from_urls, from_files, sources_dir, template }`
    - `GatherBudget { max_web_results, max_local_sources, use_low_relevance, use_pdf_web_sources, ... }`
    - `ResilienceConfig { fetch_concurrency, fetch_timeout_secs, web_phase_timeout_secs, local_phase_timeout_secs, search_max_retries, ... }`
    - `SynthesisConfig { output_format, depth, iterations, tier, max_synthesis_sources, ... }`
  - Keep `SessionConfig` as a thin aggregate with `impl SessionConfig { fn topic_input(&self) -> TopicInput; ... }` so existing call sites change minimally in Phase 2.
- **Verification:** Existing tests still construct `SessionConfig` and the aggregate still round-trips to the helper methods (`engine_config`, `budget_web_results`, etc.).

### H-3. `SessionEvent` has 30+ variants, forcing large `match` arms
- **File:** `crates/ragent-research/src/session.rs`
- **Lines:** `403`–`696`
- **Observation:** Variants include `Phase`, `QueriesDecomposed`, `WebCaptured`, `FromUrlBodyPreview`, `FromFileBodyPreview`, `LocalCaptured`, `SpecCaptured`, `WebSearchFailed`, `WebFetchFailed`, `SynthesizeResult`, `PlanUpdated`, `SubQuestionStatusChanged`, `SourceFailed`, `CriticResult`, `VerificationResult`, `IterationCompleted`, `FollowUpQueries`, `ContradictionGraph`, `LociAnalysis`, `DepthInvestigation`, `CrossLocusReconcile`, `SourceTensions`, `EvidenceDigest`, `CorpusCritic`, `GapFetch`, `TripleDraft`, `SurgicalPatch`, `CiteCheck`, `Polish`, `ReadabilityAudit`, `RunStep`, `Done`, and nested variants.
- **Concrete fix (R-007):**
  - Group into a smaller set of composite events:
    - `Progress { phase: SessionPhase, detail: ProgressDetail }`
    - `SourceCaptured { kind: SourceKind, payload: SourceCapturedPayload }`
    - `AnalysisArtifact { kind: ArtifactKind, payload: serde_json::Value }` for graph/loci/reconcile/digest/critic/etc.
    - `Failure { kind: FailureKind, message: String }`
    - `Done { summary: RunSummary }`
  - Provide a compatibility layer so front-ends can migrate incrementally.
- **Verification:** TUI/CLI renderers compile and tests observing specific events still pass via the compatibility layer or updated assertions.

### H-4. Synthesis critic reports use 0-based finding numbers
- **File:** `crates/ragent-research/src/synthesis.rs`
- **Lines:** `273`–`295`
- **Observation:** The evidence critic iterates `for (idx, finding) in analysis.findings.iter().enumerate()` and emits messages like `"Finding {idx} does not cite any source"` and `"Finding {idx} cites out-of-range source #{n}"`. Because `idx` is 0-based, this contradicts the 1-based source numbering used everywhere else (`[#1]` corresponds to `findings[0]`).
- **Concrete fix (R-010):**
  - Change `idx` to `idx + 1` in all critic messages in `synthesis.rs`.
  - Lines to edit:
    - `283`: `"Finding {idx} cites out-of-range source #{n}"` → `"Finding {} cites out-of-range source #{n}", idx + 1`
    - `293`: `"Finding {idx} does not cite any source"` → `"Finding {} does not cite any source", idx + 1`
    - `294`: `"Add a supporting source citation to finding {idx}"` → `"Add a supporting source citation to finding {}", idx + 1`
- **Verification:** Add a test asserting that critic issues for findings use 1-based numbers.

### H-5. Contradiction detector is hard-coded to six medical/tech dimensions
- **File:** `crates/ragent-research/src/contradiction.rs`
- **Lines:** `126`–`212`
- **Observation:** `POLARITY_DIMENSIONS` contains only `effect`, `mortality`, `performance`, `cost`, `adoption`, `safety` with biomedical/technology polarity tokens. For topics like history, law, literature, or finance the detector finds no contradictions, contradicting the README's general-purpose claims.
- **Concrete fix (R-011):**
  - Replace the static table with a topic-aware dimension generator:
    - Add `fn derive_dimensions(topic: &str, sources: &[Source]) -> Vec<PolarityDimension>` that extracts candidate dimensions from the loci (`crate::locus::LocusSet`) and from the topic itself.
    - Define a small default set of generic polarity pairs (`increases`/`decreases`, `benefits`/`harms`, `supports`/`opposes`, `effective`/`ineffective`, `safe`/`unsafe`) applicable to any topic.
    - Merge locus keywords into the polarity dimension list so contradictions are detected across the actual research dimensions.
  - Keep `ContradictionGraph` and `ContradictionEdge` unchanged; only the input dimensions change.
- **Verification:** Test with a non-medical topic (e.g., "Rust async vs Go goroutines performance") and assert at least one contradiction edge is produced when opposing claims exist.

### H-6. Cross-locus reconcile counts conflicting edges against dimension labels, not locus labels
- **File:** `crates/ragent-research/src/reconcile.rs`
- **Lines:** `170`–`182`
- **Observation:** In `build_cross_locus_reconcile`, the filter is:
  ```rust
  (a.label.to_lowercase() == e.dimension.to_lowercase()
   || b.label.to_lowercase() == e.dimension.to_lowercase())
  ```
  `e.dimension` comes from the contradiction graph's keyword (e.g., `performance`), while `a.label` / `b.label` are locus labels (e.g., `async runtime performance` or `tokio scheduler`). These almost never match, so `conflicting_edges` is almost always `0`.
- **Concrete fix (R-012):**
  - Change the condition to substring containment or semantic overlap:
    ```rust
    let dim_lower = e.dimension.to_lowercase();
    a.label.to_lowercase().contains(&dim_lower)
        || b.label.to_lowercase().contains(&dim_lower)
        || dim_lower.contains(&a.label.to_lowercase())
        || dim_lower.contains(&b.label.to_lowercase())
    ```
  - Cache the lowercase labels outside the edge loop to avoid O(n·m) repeated allocations.
- **Verification:** Add a test where a locus label contains a contradiction dimension substring and assert `conflicting_edges > 0`.

---

## Medium Impact

### M-1. `lib.rs` exposes too many implementation details
- **File:** `crates/ragent-research/src/lib.rs`
- **Lines:** `81`–`167`
- **Observation:** `lib.rs` re-exports 80+ items from 35 modules, including internals such as `TierRouterToSessionObserver`, `build_surgical_patches`, `chunk_source_bodies`, `local_body_path`, and `extract_published_at`. This makes future refactoring risky because downstream crates (TUI, server, CLI) may depend on internal helpers.
- **Concrete fix (R-008):**
  - Audit every `pub use` and classify as:
    - **Public API** — keep exported (e.g., `ResearchManager`, `ResearchSession`, `ResearchItem`, `Source`, `Tier`, `OutputFormat`, `ResearchRunRequest`, `build_session_config`).
    - **Crate-internal** — remove from `lib.rs` and access via `crate::module::Item` inside the crate (e.g., `build_surgical_patches`, `local_body_path`, `TierRouterToSessionObserver`).
    - **Test-only** — gate behind `#[cfg(test)]`.
  - Add a `pub mod internal` opt-in module for advanced callers if truly needed.
- **Verification:** `cargo check --workspace` must fail only on expected downstream call sites; update those call sites to use the public API.

### M-2. `AnalysisEngine` trait still has a fragile `is_noop_marker` hack
- **File:** `crates/ragent-research/src/analysis.rs`
- **Lines:** `84`–`116`
- **Observation:** The trait provides `fn is_noop_marker(&self) -> bool` specifically so `session.rs` can detect `NoopAnalysisEngine` without downcasting. This is a code smell and leaks implementation detail into the trait.
- **Concrete fix (R-009):**
  - Make `analyze_with_outcome` return a dedicated `AnalysisProvenance` enum: `Llm`, `LlmWithFallback`, `Heuristic`, `NoEngine`.
  - Remove `is_noop_marker` and the corresponding branch in `session.rs` line `1697`.
  - Map the provenance directly to `SynthesizeOutcome`.
- **Verification:** `synthesize_result_event_emitted_when_no_llm` test still passes, now asserting `SynthesizeOutcome::NoLlm` is derived from `AnalysisProvenance::NoEngine`.

### M-3. `corpus_critic.rs` `balance_score` is actually a dominance score
- **File:** `crates/ragent-research/src/corpus_critic.rs`
- **Lines:** `207`–`215`
- **Observation:** `balance_score` is computed as `(max * 100) / total`, which is `100` when the corpus is entirely one perspective and `50` when perfectly balanced. The field name and documentation say "balance," but higher values mean less balance.
- **Concrete fix (R-014):**
  - Rename `balance_score` to `dominance_score` and update the doc comment, or
  - Invert the metric so `100` means perfectly balanced:
    ```rust
    let balance_score = if positive_count + negative_count == 0 {
        50
    } else {
        let min = positive_count.min(negative_count);
        let total = positive_count + negative_count;
        ((min * 100) / total).min(100) as u32
    };
    ```
  - Update `document.rs` rendering logic that interprets this field.
- **Verification:** Add a unit test for the two extremes: monoculture → 0 balance, 50/50 → 100 balance.

### M-4. Tier router step transitions are manually repeated in `session.rs`
- **File:** `crates/ragent-research/src/session.rs`
- **Lines:** `1462`–`1837` (each stage block repeats `router.start_step`, `router.finish_step`)
- **Observation:** Every stage inlines the same start/finish pattern. The only varying parts are the `RunStep` variant and the work closure.
- **Concrete fix (R-006 / R-005 overlap):**
  - Add a helper in `tier_router.rs`:
    ```rust
    impl TierRouter {
        pub fn run_step<F>(
            &mut self,
            step: RunStep,
            observer: &dyn TierRouterObserver,
            work: F,
        ) where F: FnOnce() -> ...
        ```
  - Or, as part of the `SessionStage` refactor (H-1), each stage returns `StageResult` and the runner handles router transitions.
- **Verification:** Existing tier-router tests still pass; stage ordering unchanged.

---

## Lower Impact

### L-1. Inline `#[cfg(test)]` test module in `session.rs` is ~2,300 lines
- **File:** `crates/ragent-research/src/session.rs`
- **Lines:** `2231`–`4528`
- **Observation:** The test module dominates the file. Per project guidelines, tests should live in `tests/` directories, not inline in source files.
- **Concrete fix (R-027):**
  - Move tests to `crates/ragent-research/tests/test_session.rs`.
  - For tests that need private items, widen those items to `pub(crate)` or use `#[path = "../src/session.rs"] mod session;` in the test file with the shim pattern described in the project guidelines.
  - Keep only extremely tight unit helpers (e.g., `select_top_relevance_sources` quick tests) near the function if absolutely necessary.
- **Verification:** `cargo test -p ragent-research` passes; `cargo test --lib` no longer runs these integration-style tests.

### L-2. `tier_router.rs` observer helpers are test-only but not clearly separated
- **File:** `crates/ragent-research/src/tier_router.rs`
- **Lines:** `38`–`62`
- **Observation:** `CollectingTierRouterObserver` is marked `#[cfg(test)]` and uses `.lock().unwrap()` inside test code, which is fine but mixes test infrastructure with production code.
- **Concrete fix (R-027):**
  - Move `CollectingTierRouterObserver` to `crates/ragent-research/tests/support/tier_router_observer.rs` or keep it but convert `unwrap()` to explicit handling for consistency.
- **Verification:** `cargo test -p ragent-research` passes.

### L-3. `run_config.rs` `Depth` and `Tier` parsing does not preserve unknown values for diagnostics
- **File:** `crates/ragent-research/src/run_config.rs`
- **Lines:** `40`–`71`, `91`–`125`
- **Observation:** `OutputFormat::parse` and `Tier::parse` silently return `None` for unknown strings, which makes HTTP 400 responses generic.
- **Concrete fix (R-028, if applicable):**
  - Change `FromStr` implementations to return a structured error that lists valid values, e.g.:
    ```rust
    pub enum FormatParseError { Unknown(String), Empty }
    ```
  - Propagate this through `ResearchRunRequest` validation so HTTP callers get actionable messages.
- **Verification:** HTTP route tests for invalid `tier`/`format` receive the improved error message.

### L-4. `item.rs` frontmatter parser uses custom YAML scalar parsing
- **File:** `crates/ragent-research/src/item.rs`
- **Lines:** `435`–`482`
- **Observation:** `parse_frontmatter_label` and `unquote_yaml_scalar` reimplement a subset of YAML. This is a latent correctness risk for edge cases (multiline, escaped quotes, flow style).
- **Concrete fix (out of scope for R-005–R-014 but worth noting):**
  - Migrate to `serde_yaml` for the full frontmatter block, guarded by the existing `yaml` feature flag.
  - Keep the custom parser as a fallback only.
- **Verification:** Round-trip tests for complex titles and queries still pass.

---

## Suggested Implementation Order

1. **Start with correctness bugs** (H-4, H-5, H-6) — small, isolated, user-visible.
2. **Refactor `ResearchSession::run`** (H-1) — highest risk; do it after the small bugs to keep the diff reviewable.
3. **Split `SessionConfig`** (H-2) — naturally follows the stage extraction.
4. **Simplify `SessionEvent`** (H-3) — can be done in parallel with H-1/H-2 if compatibility layer is preserved.
5. **Clean `lib.rs` exports** (M-1) and **fix `AnalysisEngine` trait** (M-2) — architectural polish.
6. **Fix `corpus_critic` balance score** (M-3).
7. **Migrate tests out of `session.rs`** (L-1) — best done after H-1 stabilizes the module.

---

## Files to Touch for Each Requirement

| Requirement | Primary Files | Notes |
|---|---|---|
| R-005 — session refactor | `session.rs`, new `session/stages.rs` | Extract `run` into stage helpers. |
| R-006 — config split | `session.rs`, `run_config.rs` | Split `SessionConfig` into focused structs. |
| R-007 — event simplification | `session.rs`, `cli.rs`, TUI/server renderers | Introduce composite events with compat layer. |
| R-008 — lib.rs cleanup | `lib.rs`, downstream call sites | Remove internal re-exports. |
| R-009 — analysis engine trait | `analysis.rs`, `session.rs` | Replace `is_noop_marker` with provenance enum. |
| R-010 — synthesis numbering | `synthesis.rs` | `idx + 1` in critic messages. |
| R-011 — contradiction dimensions | `contradiction.rs`, `locus.rs` | Topic-aware dimension generator. |
| R-012 — reconcile edge count | `reconcile.rs` | Substring match on dimension vs. locus label. |
| R-013 — (if R-013 corresponds to another area, no direct match was found in the inspected files) | — | Verify against task table. |
| R-014 — corpus critic balance | `corpus_critic.rs`, `document.rs` | Rename or invert metric. |
| R-027 — test migration | `session.rs` → `tests/test_session.rs` | Move inline tests. |
| R-028 — config diagnostics | `run_config.rs`, `run_request.rs`, server route | Structured parse errors. |

---

## Verification Checklist (before marking Phase 2 complete)

- [ ] `cargo check -p ragent-research` clean.
- [ ] `cargo clippy -p ragent-research` clean.
- [ ] `cargo test -p ragent-research` passes.
- [ ] `cargo test --workspace` passes (after lib.rs export changes).
- [ ] TUI `/research create` smoke test still emits expected progress events.
- [ ] HTTP `POST /research` with invalid `tier` returns actionable error.
- [ ] Non-medical topic produces contradiction edges when claims oppose.

This report covers the 10 requested source files and maps each finding to the concrete Phase 2 changes required by `RESEARCHPLAN.md` items R-005 through R-014, R-027, and R-028.

---

