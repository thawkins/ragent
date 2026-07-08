# Implementation Plan: Wire `/research create` options through the research system

## Goal
Make the existing `ragent research create` / `/research create` options
`--iterations N`, `--depth shallow|standard|deep`, and
`--format report|executive-summary|comparison-table|source-bibliography`
actually control research sessions.

## Scope
- `crates/ragent-research/src/session.rs` — `SessionConfig` + `ResearchSession::run`
- `crates/ragent-research/src/run_config.rs` — existing `Depth` / `OutputFormat`
- `crates/ragent-research/src/document.rs` — document assembly
- `crates/ragent-research/src/analysis.rs` — synthesis prompt builder
- `crates/ragent-research/src/cli.rs` — argument parsing
- `src/cli.rs` — top-level CLI dispatch
- `crates/ragent-tui/src/app/research.rs` — TUI `/research create` handler
- `crates/ragent-server/src/routes/research.rs` — HTTP endpoint
- tests

---

## 1. Add fields to `SessionConfig` and map depth/iterations to budgets

### 1.1 Extend `SessionConfig` (`crates/ragent-research/src/session.rs:68-108`)

Add three new fields after `fetch_concurrency`:

```rust
/// Depth preset selected via `--depth`. When `None`, the engine behaves as
/// `Depth::Standard`.
pub depth: Option<Depth>,
/// Iteration override selected via `--iterations`. When `None`, the depth
/// preset controls iteration count.
pub iterations: Option<u32>,
/// Output artifact selected via `--format`.
pub output_format: OutputFormat,
```

Update `Default` (`session.rs:110-124`) so `output_format` is `OutputFormat::Report`
and `depth`/`iterations` are `None`.

### 1.2 Add a helper to derive the engine config and source budgets

Inside `SessionConfig` add:

```rust
impl SessionConfig {
    /// Resolve the effective [`EngineConfig`] from `depth` + `iterations`.
    fn engine_config(&self) -> EngineConfig {
        let depth = self.depth.unwrap_or(Depth::Standard);
        depth.engine_config(self.iterations, depth == Depth::Deep)
    }

    /// Maximum web sources to capture for the selected depth/iteration combo.
    fn budget_web_results(&self) -> usize {
        let cfg = self.engine_config();
        (cfg.max_sources_per_question * 3).max(3)
    }

    /// Maximum local sources to capture for the selected depth.
    fn budget_local_sources(&self) -> usize {
        match self.depth.unwrap_or(Depth::Standard) {
            Depth::Shallow => 5,
            Depth::Standard => 10,
            Depth::Deep => 20,
        }
    }
}
```

### 1.3 Use the budgets in `ResearchSession::run`

At the start of the Web phase (`session.rs:1002-1032`), replace the direct
use of `config.max_web_results` with the derived budget:

```rust
let web_budget = config.max_web_results.max(config.budget_web_results());
let local_budget = config.max_local_sources.max(config.budget_local_sources());
```

Pass `web_budget` to `web.gather_with_observer(...)` and `local_budget` into
`LocalGatherConfig` (`session.rs:1045-1049`).

### 1.4 Keep `max_web_results` / `max_local_sources` as explicit overrides

When a caller explicitly populates those fields (tests, older callers), use
`max(...)` so explicit values win. When only `depth` is supplied, the budget
helper provides sensible defaults.

---

## 2. Optionally use `IterativeEngine` for multi-iteration runs

### 2.1 Give `ResearchSession` the planner/critic it needs

Add two optional fields to `ResearchSession` (`session.rs:336-342`):

```rust
planner: Option<Arc<dyn Planner>>,
critic: Option<Arc<dyn Critic>>,
```

Add builder setters (`session.rs:375-418`):

```rust
pub fn with_planner(mut self, planner: Arc<dyn Planner>) -> Self {
    self.planner = Some(planner);
    self
}
pub fn with_critic(mut self, critic: Arc<dyn Critic>) -> Self {
    self.critic = Some(critic);
    self
}
```

### 2.2 Wire planner/critic in `build_research_session`
(`crates/ragent-agent/src/research_adapter.rs:45-93`)

After constructing `analysis`, build planner/critic and pass them into
`ResearchSession::new`:

```rust
let planner: Arc<dyn Planner> = match (provider_registry.clone(), active_model.clone()) {
    (Some(reg), Some(m)) => {
        let api_key = storage.as_deref()
            .and_then(|s| s.get_provider_auth(&m.provider_id).ok().flatten());
        let base_url = resolve_base_url(&m.provider_id, storage.as_deref(), config.as_deref());
        Arc::new(LlmPlanner::new(reg, &m.provider_id, &m.model_id)
            .with_api_key(api_key)
            .with_base_url(base_url))
    }
    _ => Arc::new(HeuristicPlanner::new()),
};
let critic: Arc<dyn Critic> = Arc::new(SimpleCritic);

ResearchSession::new(manager, web, local, analysis)
    .with_planner(planner)
    .with_critic(critic)
```

### 2.3 Add the iterative branch inside `ResearchSession::run`

After the `--from-url` pre-step and item creation (`session.rs:980-990`), and
before the single-pass Web phase, decide which path to take:

```rust
let engine_cfg = config.engine_config();
let use_iterative = engine_cfg.max_iterations > 1;
```

If `use_iterative`, run the iterative engine. Extract a helper
`run_iterative_pass` in `session.rs`:

```rust
async fn run_iterative_pass(
    &self,
    topic: &str,
    config: &SessionConfig,
    observer: Arc<dyn SessionObserver>,
) -> Result<(Vec<Source>, Vec<String>, u32)> {
    let planner = self.planner.clone()
        .unwrap_or_else(|| Arc::new(HeuristicPlanner::new()));
    let critic = self.critic.clone()
        .unwrap_or_else(|| Arc::new(SimpleCritic));
    let engine = IterativeEngine::new(
        planner,
        self.web.clone(),
        self.analysis.clone(),
        critic,
        config.engine_config(),
    );
    let state = engine.run(topic, observer.clone()).await?;
    let queries: Vec<String> = state.plan.sub_questions.iter().map(|s| s.question.clone()).collect();
    Ok((state.sources, queries, state.iteration_count))
}
```

Then, in `run`, after the iterative pass finishes, still run the Local and Spec
phases and append their sources to the same `sources` vector. Synthesize from the
merged corpus.

### 2.4 Single-pass path stays unchanged (but uses budgets)

When `engine_cfg.max_iterations == 1`, keep the existing Web/Local/Spec/Synthesize
flow but use the derived `web_budget` / `local_budget` from §1.3.

### 2.5 Synthesize and assemble from either path

Both paths end with the same `sources`/`web_queries` variables, so the existing
Synthesize and Assemble code (`session.rs:1110-1208`) is reused unchanged.

---

## 3. Use `OutputFormat` to influence document assembly and synthesis prompts

### 3.1 Add `output_format` to `ResearchDocument`
(`crates/ragent-research/src/document.rs:80-103`)

```rust
/// Output artifact this document was requested as.
pub output_format: OutputFormat,
```

### 3.2 Add `output_format` to `SynthesisPromptConfig`
(`crates/ragent-research/src/analysis.rs:315-332`)

```rust
pub output_format: Option<OutputFormat>,
```

Make it settable through `SynthesisPromptBuilder`:

```rust
pub fn output_format(mut self, fmt: OutputFormat) -> Self {
    self.config.output_format = Some(fmt);
    self
}
```

### 3.3 Tailor the prompt template per format
(`analysis.rs:427-523`)

In `render_output_template`, branch on `config.output_format` before emitting
the default report instructions. For example:

```rust
match config.output_format {
    Some(OutputFormat::ExecutiveSummary) => {
        out.push_str("## Summary\nA very concise executive summary in 2-3 sentences.\n\n");
        out.push_str("## Findings\nAt most 5 high-level findings. Keep each finding to one compact paragraph per required label.\n\n");
    }
    Some(OutputFormat::ComparisonTable) => {
        out.push_str("## Summary\nOne-paragraph overview of the entities being compared.\n\n");
        out.push_str("## Comparison Table\nA markdown table with columns: Entity | Key strengths | Key weaknesses | Best for | Sources.\n\n");
        out.push_str("## Findings\n3-7 findings that explain the comparison and cite sources with [#N].\n\n");
    }
    Some(OutputFormat::SourceBibliography) => {
        out.push_str("## Summary\nOne paragraph summarizing the corpus.\n\n");
        out.push_str("## Findings\nAn annotated bibliography: one entry per major source, describing its contribution and citing [#N].\n\n");
    }
    _ => { /* existing report template */ }
}
```

Keep the four required labels (`Observation`, `Analysis`, `Cross-reference / Dependencies`,
`Implication`) for every format, but make the *volume* and *emphasis* instructions
format-specific.

### 3.4 Wire the format into `LlmAnalysisEngine`

Add a `with_output_format` setter to `LlmAnalysisEngine` (`analysis.rs:171-209`):

```rust
pub fn with_output_format(mut self, fmt: Option<OutputFormat>) -> Self {
    self.output_format = fmt;
    self
}
```

Store the field on `LlmAnalysisEngine` and use it in `stream_synthesis`
(`analysis.rs:239-303`):

```rust
let prompt = SynthesisPromptBuilder::new(topic)
    .sources(sources)
    .output_format(self.output_format.unwrap_or(OutputFormat::Report))
    .build();
```

Update the legacy `build_synthesis_prompt` free function to route through the
builder as well, passing `OutputFormat::Report` so legacy behavior is preserved.

### 3.5 Pass `output_format` through `ResearchDocument` into assembly

When building `ResearchDocument` in `ResearchSession::run`
(`session.rs:1168-1207`), set:

```rust
output_format: config.output_format,
```

For now `assemble_document` (`document.rs:137-259`) keeps all eight sections,
but the content coming from the analysis engine is already shaped by the prompt.
Future work could add format-aware section ordering here.

### 3.6 Add `format` to frontmatter

In `assemble_document`, include the requested format in the YAML frontmatter
through the existing `doc.item.render_frontmatter()` path, or add a dedicated
field to `ResearchItem` if needed. A lightweight first step is to append a
`requested_format` key to the frontmatter in `ResearchItem::render_frontmatter`
(`crates/ragent-research/src/item.rs`).

---

## 4. Wire TUI and CLI so parsed options reach `SessionConfig`

### 4.1 `ResearchCliCommand` already parses the strings
(`crates/ragent-research/src/cli.rs:145-256`)

The parser already produces `iterations`, `depth`, `format` as `Option<String>`.
No change required there.

### 4.2 Top-level CLI dispatch (`src/cli.rs:139-311`)

In `handle_research_command`, convert the parsed strings to typed values and
populate `SessionConfig`:

```rust
let depth = depth.as_deref().and_then(Depth::parse);
let output_format = format.as_deref().map(|s| {
    OutputFormat::parse(s).unwrap_or_else(|| {
        eprintln!("ragent-research: warning: unknown format '{s}', using 'report'");
        OutputFormat::Report
    })
}).unwrap_or(OutputFormat::Report);

let config = SessionConfig {
    topic: topic.clone(),
    from_url,
    sources_dir: sources_dir.map(PathBuf::from),
    template,
    disable_local: !use_local,
    disable_specs: !use_specs,
    fetch_concurrency: fetch_concurrency.unwrap_or(ragent_research::DEFAULT_FETCH_CONCURRENCY),
    depth,
    iterations,
    output_format,
    ..SessionConfig::default()
};
```

Remove the `iterations: _, depth: _, format: _` wildcard destructuring in the
second `ResearchCliCommand::Create` match arm (`src/cli.rs:268-279`).

### 4.3 TUI handler (`crates/ragent-tui/src/app/research.rs:51-157`)

In the `ResearchCliCommand::Create` match arm, currently `iterations`, `depth`,
and `format` are bound to `_`. Bind them and convert:

```rust
ResearchCliCommand::Create {
    name,
    topic,
    from_url,
    iterations,
    depth,
    format,
    sources_dir,
    template,
    fetch_concurrency,
    use_local,
    use_specs,
} => {
    let depth = depth.as_deref().and_then(Depth::parse);
    let output_format = format.as_deref()
        .and_then(OutputFormat::parse)
        .unwrap_or(OutputFormat::Report);
    // ... existing title logic ...
    let config = SessionConfig {
        topic: topic.clone(),
        from_url,
        sources_dir: sources_dir.map(std::path::PathBuf::from),
        template,
        disable_local: !use_local,
        disable_specs: !use_specs,
        fetch_concurrency: fetch_concurrency.unwrap_or(ragent_research::DEFAULT_FETCH_CONCURRENCY),
        depth,
        iterations,
        output_format,
        ..SessionConfig::default()
    };
    // ... rest unchanged ...
}
```

### 4.4 HTTP endpoint (`crates/ragent-server/src/routes/research.rs:95-131`)

Extend `CreateResearchRequest`:

```rust
#[serde(default)]
depth: Option<String>,
#[serde(default)]
iterations: Option<u32>,
#[serde(default)]
format: Option<String>,
```

Map them in `create_research`:

```rust
let depth = req.depth.as_deref().and_then(Depth::parse);
let output_format = req.format.as_deref()
    .and_then(OutputFormat::parse)
    .unwrap_or(OutputFormat::Report);

let config = SessionConfig {
    topic: req.topic.clone(),
    from_url: req.from_url.clone(),
    sources_dir: req.sources_dir.map(PathBuf::from),
    template: req.template,
    disable_local: !req.use_local,
    disable_specs: !req.use_specs,
    fetch_concurrency: req.fetch_concurrency.unwrap_or(ragent_research::DEFAULT_FETCH_CONCURRENCY),
    depth,
    iterations: req.iterations,
    output_format,
    ..SessionConfig::default()
};
```

### 4.5 Update help text
(`crates/ragent-research/src/cli.rs:311-346`)

Ensure `build_help_message()` documents the defaults:

```text
--iterations N          override iteration count (default: from --depth)
--depth shallow|standard|deep  research thoroughness (default: standard)
--format report|executive-summary|comparison-table|source-bibliography
                        output artifact (default: report)
```

---

## 5. Tests

### 5.1 `run_config` mapping tests (`crates/ragent-research/src/run_config.rs:129-170`)

Add tests verifying depth/iteration budgets:

```rust
#[test]
fn shallow_session_config_has_small_budgets() {
    let cfg = SessionConfig {
        depth: Some(Depth::Shallow),
        ..SessionConfig::default()
    };
    assert_eq!(cfg.engine_config().max_iterations, 1);
    assert_eq!(cfg.budget_web_results(), 6); // 2 * 3
    assert_eq!(cfg.budget_local_sources(), 5);
}

#[test]
fn deep_session_config_has_large_budgets() {
    let cfg = SessionConfig {
        depth: Some(Depth::Deep),
        ..SessionConfig::default()
    };
    assert_eq!(cfg.engine_config().max_iterations, 5);
    assert!(cfg.engine_config().force_deeper);
    assert_eq!(cfg.budget_web_results(), 15); // 5 * 3
    assert_eq!(cfg.budget_local_sources(), 20);
}

#[test]
fn iterations_override_beats_depth_preset() {
    let cfg = SessionConfig {
        depth: Some(Depth::Shallow),
        iterations: Some(4),
        ..SessionConfig::default()
    };
    assert_eq!(cfg.engine_config().max_iterations, 4);
}
```

Place these in `session.rs` because the helper lives there, or add a small
`SessionConfig` test module in `session.rs`.

### 5.2 Prompt-format tests (`crates/ragent-research/src/analysis.rs:1140-`)

Add tests that assert each `OutputFormat` appears in the built prompt and that
the default report prompt is unchanged:

```rust
#[test]
fn output_format_executive_summary_appears_in_prompt() {
    let sources = vec![src_body(1, None)];
    let prompt = SynthesisPromptBuilder::new("rust async")
        .sources(&sources)
        .output_format(OutputFormat::ExecutiveSummary)
        .build();
    assert!(prompt.contains("very concise executive summary"));
}

#[test]
fn output_format_comparison_table_includes_table_request() {
    let sources = vec![src_body(1, None)];
    let prompt = SynthesisPromptBuilder::new("rust async")
        .sources(&sources)
        .output_format(OutputFormat::ComparisonTable)
        .build();
    assert!(prompt.contains("Comparison Table"));
    assert!(prompt.contains("markdown table"));
}
```

### 5.3 CLI parsing tests already exist
(`crates/ragent-research/src/cli.rs:844-865`)

`parse_create_with_iterations_depth_format` already covers the hand parser.
No change required.

### 5.4 Integration tests

Update the existing integration tests to exercise format-specific output shapes.

In `crates/ragent-research/tests/test_research_create_synthesis.rs:157-168`, add
a variant that sets `output_format: OutputFormat::ExecutiveSummary` and asserts
that the generated `RESEARCH.md` is shorter / contains a compact summary.

In `crates/ragent-research/tests/test_research_integration.rs:256-261`, pass
`depth: Some(Depth::Shallow)` and `output_format: OutputFormat::Report` to ensure
the new fields do not break the single-pass flow.

### 5.5 Iterative-engine integration test

Add a new test in `crates/ragent-research/tests/test_research_iterative.rs`:

```rust
#[tokio::test]
async fn multi_iteration_session_gathers_more_sources() {
    // Build a ResearchSession with planner + critic and a fake web search
    // that returns one hit per sub-question.
    let cfg = SessionConfig {
        topic: "rust async".into(),
        depth: Some(Depth::Deep),
        iterations: Some(2),
        disable_local: true,
        disable_specs: true,
        ..SessionConfig::default()
    };
    // run and assert > 0 sources and iterative events emitted.
}
```

Use the fake `WebSearchTool`/`WebFetchTool` patterns from
`test_research_create_synthesis.rs`.

---

## 6. Implementation order

1. Add `depth`, `iterations`, `output_format` to `SessionConfig` and the budget helpers.
2. Add `output_format` to `SynthesisPromptConfig`/`ResearchDocument` and update the prompt builder.
3. Add `with_output_format` to `LlmAnalysisEngine` and use the builder in `stream_synthesis`.
4. Add optional `planner`/`critic` to `ResearchSession` and the iterative branch.
5. Wire `build_research_session` to supply planner + critic.
6. Update `src/cli.rs`, `crates/ragent-tui/src/app/research.rs`, and `crates/ragent-server/src/routes/research.rs` to populate the new fields.
7. Update tests and help text.
8. Run `cargo check --workspace`, `cargo build --workspace --tests`, and
   `cargo test -p ragent-research`.

---

## 7. Risks / decisions

- **Default behavior:** With no flags, `depth=None` and `iterations=None`, so
  `engine_config()` returns `Depth::Standard` (3 iterations by the helper's
  definition). The current single-pass default must stay unchanged for the
  common case. To avoid changing behavior for users who do not pass the new
  flags, only take the iterative branch when `config.iterations.is_some() ||
  config.depth == Some(Depth::Deep)`. Shallow/Standard without an explicit
  `--iterations` remain single-pass.
- **LLM planner requirement:** The iterative branch uses `LlmPlanner` when an
  active model is available; otherwise it falls back to `HeuristicPlanner`.
- **Document sections:** All eight `RESEARCH.md` sections remain present for
  every format; the format only shapes the synthesis instructions.


---

## Implementation Log

**Date:** 2026-07-07
**Status:** Implemented and verified

### Summary of changes

Wired the `/research create` option flags (`--iterations`, `--depth`, `--format`) through the research system as described in this plan, with the safety tweak that the iterative engine is only used when `--iterations` is explicitly set OR `--depth deep` is set.

### Files modified

- `crates/ragent-research/src/session.rs`
  - Added `depth`, `iterations`, `output_format` to `SessionConfig`.
  - Added `engine_config()`, `budget_web_results()`, `budget_local_sources()` helpers.
  - Derived web/local budgets are used in single-pass `ResearchSession::run`.
  - Added optional `planner`/`critic` fields and `with_planner`/`with_critic` setters to `ResearchSession`.
  - Added `run_iterative_pass` helper and guarded iterative branch on `config.iterations.is_some() || config.depth == Some(Depth::Deep)`.
  - Set `output_format` on the assembled `ResearchDocument`.
  - Added unit tests for budgets and the explicit-flags guard.


- `crates/ragent-research/src/analysis.rs`
  - Added `output_format` to `SynthesisPromptConfig` and `output_format()` setter on `SynthesisPromptBuilder`.
  - Branched `render_output_template` for `ExecutiveSummary`, `ComparisonTable`, `SourceBibliography`.
  - Added `with_output_format` to `LlmAnalysisEngine`; `stream_synthesis` uses the builder with the configured format.
  - Updated legacy `build_synthesis_prompt` to route through the builder with `OutputFormat::Report`.
  - Added unit tests for format-specific prompts.

- `crates/ragent-research/src/document.rs`
  - Added `output_format` to `ResearchDocument`; updated test helpers.

- `crates/ragent-research/src/item.rs`
  - Added `output_format` field to `ResearchItem`; persisted as `requested_format` in frontmatter and parsed back.

- `crates/ragent-research/src/manager.rs`
  - Added `EngineRunFailed` variant for iterative engine failures; updated `render_document_for` helper to supply `output_format`.

- `crates/ragent-research/src/cli.rs`
  - Updated `build_help_message` defaults text.

- `crates/ragent-research/src/web_gatherer.rs`
  - Made `DEFAULT_MAX_WEB_RESULTS` and `DEFAULT_FETCH_CONCURRENCY` public, added `with_fetch_concurrency` to `WebGatherer`, and switched the fetch phase to bounded concurrent fetches.

- `src/cli.rs`
  - Removed `_` destructuring of `iterations`/`depth`/`format`; parsed and populated `SessionConfig`.

- `crates/ragent-tui/src/app/research.rs`
  - Removed `_` destructuring; parsed and populated `SessionConfig`.

- `crates/ragent-server/src/routes/research.rs`
  - Extended `CreateResearchRequest` with `depth`, `iterations`, `format` and mapped them to `SessionConfig`.

- `crates/ragent-agent/src/research_adapter.rs`
  - Wired `LlmPlanner`/`SimpleCritic` into `ResearchSession::new` via `with_planner`/`with_critic`.

- Tests
  - `crates/ragent-research/tests/test_research_create_synthesis.rs`: added `OutputFormat` import and a new integration test for `ExecutiveSummary` frontmatter.
  - `crates/ragent-research/tests/test_research_integration.rs`: updated `SessionConfig` construction and added `depth`/`output_format` fields.
  - `crates/ragent-research/tests/test_template_merge.rs`: added `output_format` to test document.

### Verification

- `cargo check --workspace` — passed
- `cargo build --workspace --tests` — passed
- `cargo build --release` — passed
- `cargo clippy --workspace` — passed (clean)
- `cargo test -p ragent-research` — passed (337 unit tests, 16 integration/doc tests)
- `cargo test -p ragent-agent --lib` — passed (316 tests)
- `cargo test -p ragent-tui --lib` — passed (60 tests)
- `cargo test -p ragent-server --test test_integration` — passed (16 tests)
- `cargo fmt --check` — passed

**Note:** `cargo test -p ragent-team --test test_m4_delivery` still fails with 4 pre-existing failures introduced in `0.1.0-alpha.138` (the shared `setup_workspace` helper returns `.ragent/teams` as the working directory, but `find_team_dir` expects the project root). These failures are unrelated to the research option wiring.

### Notes / residual items

- The iterative engine branch is only taken when the user explicitly opts in via `--iterations N` or `--depth deep`. This preserves the existing single-pass default for `ragent research create` without flags and for `--depth shallow|standard`.
- The `build_synthesis_prompt` free function is now used only by the backward-compat test and is marked `#[allow(dead_code)]` to keep the warning clean.
- All eight `RESEARCH.md` sections remain present regardless of format; the synthesis prompt shapes the content.
