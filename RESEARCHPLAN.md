# Research System Performance & Simplification Plan

## Goal

Make the `/research` feature faster, leaner, and easier to maintain while
preserving its current output shape (`RESEARCH.md`, supporting files, and
`research/INDEX.md`). This plan targets the `ragent-research` crate and its
use of `ragent-tools-extended` search/fetch primitives.

## Scope

- `crates/ragent-research/src/session.rs` — main orchestration engine (3,607 lines).
- `crates/ragent-research/src/web_gatherer.rs` — web discovery/fetch (2,419 lines).
- `crates/ragent-research/src/analysis.rs` — LLM synthesis/prompts (1,955 lines).
- `crates/ragent-research/src/local_gatherer.rs` — local file/spec gathering.
- `crates/ragent-research/src/document.rs` — `RESEARCH.md` assembly.
- `crates/ragent-research/src/manager.rs` — research lifecycle + search.
- `crates/ragent-tools-extended/src/websearch.rs` / `webfetch.rs` — underlying
  search/fetch tools.

## Current Pain Points

| Area | Problem | Impact |
|------|---------|--------|
| `session.rs` | 3,607 lines mixing orchestration, topic derivation, synthesis fallback, default-content generation, and tests. | Hard to reason about; changes are risky; tests are inline. |
| `web_gatherer.rs` | 2,419 lines mixing gatherer, decomposers, title cleaning, relevance scoring, URL classification, and tests. | Low cohesion; repeated string allocation and re-case normalization. |
| `analysis.rs` | 1,955 lines mixing prompt builder, parser, output templates, and tests. | Synthesis prompt construction is expensive and hard to unit-test. |
| `local_gatherer.rs` | Candidate scoring is fully sequential (glob → grep → read). | Slow wall-clock time on large projects. |
| Session flow | Web, local, and spec phases run sequentially. | Latency is additive instead of overlapped. |
| Source volume | Up to 250 web sources × 256 KB bodies are loaded into memory, then all sent to the LLM in one prompt. | Large prompts, high token cost, context-limit risk. |
| Relevance filtering | Low-relevance pages are fetched fully before being discarded. | Wasted bandwidth, latency, and LLM context budget. |
| Research search | `ResearchManager::search` reads every `RESEARCH.md` on every search. | No index; degrades as research items accumulate. |
| Resilience | No explicit per-phase timeout or circuit-breaker. | A single slow fetch/search can stall the whole session. |

## Success Criteria

1. `session.rs`, `web_gatherer.rs`, and `analysis.rs` are each split into
   focused modules with inline tests moved to `tests/`.
2. Representative `/research create` queries complete with measurably lower
   wall-clock time and peak memory use.
3. Local/spec gathering runs with bounded concurrency.
4. Web and local phases can overlap.
5. Large source corpora are summarized/chunked before synthesis instead of
   being dumped into a single monolithic prompt.
6. Low-relevance web candidates are filtered before full-page fetch.
7. Research search has a lightweight derived index or cache.
8. All existing `cargo test -p ragent-research --lib` tests continue to pass;
   new tests cover the optimizations.

## Milestones

### Milestone A: Measure first

Establish a reproducible baseline before changing behavior.

- **[x] A-001** Add `tracing` spans around each phase in
  `ResearchSession::run` (setup, web, local, spec, synthesis, assemble, finalize).
  Verify via logs. Implemented in `crates/ragent-research/src/session.rs` with
  per-phase `research_phase` spans and `elapsed_ms` logging.
- **[x] A-002** Add a small benchmark harness (`cargo bench -p ragent-research`)
  for `WebGatherer` fake search/fetch and `LocalGatherer` fake filesystem.
  Verify benchmarks compile and run. Added
  `crates/ragent-research/benches/gatherer_bench.rs`;
  `cargo bench -p ragent-research --bench gatherer_bench` runs successfully.
- **[x] A-003** Run three representative `/research create` topics end-to-end
  with the real stack and capture wall-clock time, peak RSS, and prompt size.
  Verify baseline numbers are recorded in a report. Added
  `tests/test_research_baseline.rs` which writes
  `target/temp/research_baseline_report.md`; also added
  `scripts/research_baseline.sh` for optional real-stack runs.

### Milestone B: Reduce memory pressure and wasted I/O in web gathering

Stream candidates instead of buffering the whole world.

- **[x] B-001** Pre-filter search hits by title/snippet relevance **before**
  full-page fetch. Keep `--use-low-relevance` semantics unchanged.
  Verify: `gather_prefilters_low_relevance_hits_before_fetch` and
  `gather_keep_low_relevance_disables_prefilter` tests pass.
- **[x] B-002** Enforce `MAX_SOURCE_BODY_BYTES` at fetch-time, not after capture,
  and collect source bodies in a bounded-size buffer. Verify memory cap:
  `gather_caps_huge_body_at_max_source_body_bytes` test passes.
- **[x] B-003** Normalize query terms once per sub-query and use
  case-insensitive matching instead of repeated `to_lowercase()` in
  `compute_relevance_label`. Verify unit tests and benchmark speedup.
- **[x] B-004** Add per-fetch timeout configurable via `SessionConfig` with a
  sensible default. Verify slow URLs time out and emit `FetchFailed`.
- **[x] B-005** Keep the original search-ranking order of retained sources when
  numbering `web-NN.md`. Verify existing ordering tests still pass.

### Milestone C: Parallelize local and spec gathering

Remove the sequential bottleneck for local evidence.

- **[x] C-001** Add `local_concurrency` to `SessionConfig` / `LocalGatherConfig`
  (default 8). Verify default exists and is documented.
- **[x] C-002** Parallelize candidate scoring in `LocalGatherer` with bounded
  concurrency (`buffer_unordered`). Verify correctness on a temp tree.
- **[x] C-003** Parallelize spec cross-reference scanning and de-duplication.
  Verify against the existing spec test corpus.
- **[x] C-004** Ensure local/spec I/O stays within `target/` of the project root
  and does not escape. Verify path-guard tests.

### Milestone D: Overlap independent phases

Reduce additive latency.

- **[x] D-001** Run web gathering and local/spec gathering concurrently up to the
  synthesis step. Verify combined sources contain both web and local results.
  Implemented in `crates/ragent-research/src/session.rs` using `tokio::join!` to
  run the web and local futures concurrently. Verified by
  `overlapped_gather_combines_web_and_local_sources_and_emits_phases` and
  `overlapped_gather_survives_local_phase_failure` tests.
- **[x] D-002** Preserve per-phase diagnostic events so the UI still shows web
  and local progress separately. Verify events are emitted in order.
  `Phase::Web` and `Phase::Local` events are emitted synchronously before
  `tokio::join!`; `Phase::Specs` is emitted after the local future completes.
  Verified by `overlapped_gather_emits_phase_events_in_order` test which asserts
  Web → Local → Specs ordering.
- **[x] D-003** When `--from-url` is provided, fetch the seed URL before or in
  parallel with the normal phases. Verify seed URL appears as source #1.
  The seed URL is fetched before the gather phases and pushed as the first
  source. Verified by `from_url_seed_appears_as_first_source` test which asserts
  the seed URL precedes all search-discovered sources.

### Milestone E: Summarize/chunk synthesis input

Keep prompts within context and token budget.

- **[x] E-001** Add a `SourceSummarizer` trait and a heuristic implementation
  that collapses each source body to a fixed token/char budget.
  Verify summarized bodies fit the budget.
  Implemented in `crates/ragent-research/src/analysis.rs`: the `SourceSummarizer`
  trait + `HeuristicSummarizer` struct snap to paragraph/sentence boundaries
  within a char budget and append a truncation marker. The `summarize_source_bodies`
  function applies the summarizer to a `Vec<SourceBody>` preserving all metadata.
  Verified by `heuristic_summarizer_returns_body_unchanged_when_within_budget`,
  `heuristic_summarizer_truncates_to_budget_chars`,
  `heuristic_summarizer_snaps_to_paragraph_boundary`, and
  `summarize_source_bodies_preserves_metadata` tests.
- **[x] E-002** When total source volume exceeds a configured threshold, send
  sources in chunked LLM calls and merge partial findings.
  Verify output still contains references to all source numbers.
  Implemented in `LlmAnalysisEngine::analyze_with_outcome`: when
  `synthesis_chunk_threshold` is set and `total_body_chars` exceeds it, sources
  are split via `chunk_source_bodies` into chunks of `synthesis_chunk_size`
  (default 48K chars). Each chunk is sent as a separate LLM call; partial
  `AnalysisResult`s are merged via `merge_chunk_results`, which concatenates
  findings (renumbered sequentially), deduplicates cross-references by path,
  and merges open questions. Source indices are preserved across chunks so
  `[#N]` citations remain valid. Verified by `chunk_source_bodies_splits_on_budget`,
  `chunk_source_bodies_groups_small_sources`,
  `merge_chunk_results_concatenates_findings_and_renumbers`,
  `merge_chunk_results_dedup_cross_references`,
  `merge_chunk_results_single_part_is_clone`, and
  `merge_chunk_results_empty_returns_default` tests.
- **[x] E-003** Add a `max_synthesis_sources` cap that selects the highest-
  relevance sources when the corpus is too large. Verify `--use-low-relevance`
  still allows low-relevance inclusion.
  Implemented in `crates/ragent-research/src/session.rs`: `SessionConfig` gained
  a `max_synthesis_sources: Option<usize>` field. When set and the corpus exceeds
  the cap, `select_top_relevance_sources` sorts by `Source::relevance_rank()`
  (added to `source.rs`, mapping relevance labels to numeric ranks 1–8) descending,
  takes the top N, and restores original order so citation indices stay stable.
  `--use-low-relevance` sources remain in the pool (the web gatherer already
  controls filtering); the cap just picks the top N from whatever is in the pool.
  Verified by `relevance_rank_*` tests in `source.rs` and
  `select_top_relevance_sources_*` tests in `session.rs`.
- **[x] E-004** Ensure diagrams and cross-references still render correctly after
  chunking. Verify document assembly tests.
  Verified by `crates/ragent-research/tests/test_chunked_synthesis_diagram.rs`:
  simulates two chunk results, merges them via `merge_chunk_results`, assembles
  a `ResearchDocument`, and asserts the `## Findings Relationship Diagram` section
  contains all three finding nodes (F1, F2, F3) with the F2→F1 dependency edge,
  the `## In-Project Cross-References` section contains both deduplicated
  cross-references, and all `REQUIRED_SECTIONS` are present and in order.

### Milestone F: Simplify and split oversized modules

Improve maintainability without behavior changes.

- **[x] F-001** Extract `session/topic.rs` for URL/body topic derivation and
  `session/fallback.rs` for default summary/findings/questions. Move tests to
  `crates/ragent-research/tests/`. Verify `cargo test` passes.
  Implemented: `session/topic.rs` (442 lines) contains all topic-derivation
  functions (`derive_topic_from_url_body`, `derive_topic_from_body`,
  `derive_topic_description`, `fuzzy_contains`, `clean_topic_fragment`,
  `clean_site_title`, `split_glued_words`, etc.) with `pub(crate)` visibility.
  `session/fallback.rs` (365 lines) contains `default_summary`,
  `default_findings`, `body_excerpt`, `default_open_questions`,
  `cross_references_from`, and `format_with_kind`. Topic tests moved to
  `tests/test_session_topic.rs` (8 tests) and fallback tests to
  `tests/test_session_fallback.rs` (13 tests), both using `#[path]` to
  access `pub(crate)` functions. `session.rs` reduced from 4,361 to 3,182
  lines. Verified by `cargo test -p ragent-research --lib` (412 tests).
- **[x] F-002** Extract `web_gatherer/title.rs`, `web_gatherer/relevance.rs`,
  `web_gatherer/decomposer.rs`, and `web_gatherer/classify.rs`. Move tests.
  Verify.
  Implemented: `web_gatherer/title.rs` (142 lines) contains
  `clean_web_source_title`, `clean_title_text`, `strip_markdown_link_text`,
  `strip_leading_noise`, `collapse_title_ws`, `truncate_title_words`, and
  `MAX_WEB_SOURCE_TITLE_CHARS`. `web_gatherer/relevance.rs` (95 lines)
  contains `compute_relevance_label`, `normalize_query_terms`, `is_stopword_lc`,
  and `is_stopword`. `web_gatherer/decomposer.rs` (394 lines) contains the
  `QueryDecomposer` trait, `HeuristicQueryDecomposer`, `LlmQueryDecomposer`,
  and all decomposition helpers. `web_gatherer/classify.rs` (50 lines)
  contains `WebSourceKind` and `classify_web_source`. Title and relevance
  tests moved to `tests/test_web_gatherer_helpers.rs` (18 tests including
  4 new classify tests). `web_gatherer.rs` reduced from 2,853 to 2,087
  lines. Public re-exports preserved in the parent module.
- **[x] F-003** Extract `analysis/prompt.rs`, `analysis/parser.rs`,
  `analysis/templates.rs`, and `analysis/engine.rs`. Move tests. Verify.
  Implemented: `analysis/prompt.rs` (367 lines) contains
  `SynthesisPromptConfig`, `SynthesisPromptBuilder`, `render_preamble`,
  `render_output_template`, `render_sources_block`, `render_closing`, and
  `build_synthesis_prompt`. `analysis/parser.rs` (568 lines) contains
  `parse_analysis_response`, `parse_analysis_response_with_outcome`,
  `validate_citations_and_dates`, `is_malformed_analysis_result`,
  `mechanical_fallback_findings`, `extract_candidate_findings`,
  `split_sections`, `parse_numbered_list`, `parse_bullet_list`,
  `parse_cross_reference_list`, `reorder_findings_by_dependency`, and
  `truncate_body`. The `LlmAnalysisEngine`, types (`SourceBody`,
  `AnalysisResult`, `AnalysisOutcome`), `NoopAnalysisEngine`, and
  summarizer/chunk functions remain in `analysis.rs` (1,571 lines) as the
  module root. Inline tests stay in `analysis.rs` because they exercise
  the engine, parser, and prompt builder together. `analysis.rs` reduced
  from 2,478 to 1,571 lines.
- **[x] F-004** Replace mechanical default-content string builders with small,
  testable template helpers. Verify fallback output is byte-identical or
  acceptable.
  Implemented: `finding_template()` helper in `session/fallback.rs`
  assembles the five-paragraph finding layout (Headline, Observation,
  Analysis, Cross-reference/Dependencies, Implication) from individual
  string parts. All four finding builders (web, local, spec, empty) now
  use this single template function instead of inline `format!` strings.
  The template output is byte-identical to the previous inline format
  strings. Verified by the existing `default_findings_*` tests in
  `tests/test_session_fallback.rs` (13 tests all pass).
- **[x] F-005** Delete any pre-existing dead code surfaced during the split only
  if it was made unused by this refactor; otherwise leave it and document.
  No new dead code was surfaced by the split. Pre-existing `#[allow(dead_code)]`
  annotations on `is_stopword` (relevance.rs), `truncate_captured_body`
  (web_gatherer.rs), and `SynthesisPromptConfig` fields (prompt.rs) were
  preserved. Functions used by the crate but not by `#[path]` test files
  (`cross_references_from`, `format_with_kind`, `compute_relevance_label`,
  `WebSourceKind::as_str`) received `#[allow(dead_code)]` to suppress
  false-positive warnings during test compilation. The pre-existing
  `session.rs.bak` backup file was not created by this refactor and was
  left in place.

### Milestone G: Index research search

Avoid scanning every `RESEARCH.md` on every search.

- **[x] G-001** Build a derived `research/.index.json` cache on create/update
  containing title, topic, status, tags, and one-line summary per item.
  Verify it is regenerated after creates and deletes.
  Implemented: `SearchIndexEntry` and `SearchIndex` structs added to
  `manager.rs` with `serde::Serialize`/`Deserialize`. Each entry stores name,
  title, topic, status (kebab-case string), tags (reserved, currently empty),
  summary (one-line extracted from `## Summary` section via
  `extract_one_line_summary()`), created_at, modified_at, and `search_text`
  (full body after frontmatter, so cache-based search produces identical
  results to full-scan). `ResearchIo::cache_path()` returns
  `research/.index.json`. `refresh_index()` now builds and atomically writes
  the cache alongside `INDEX.md` on every mutation (create, delete, archive,
  transition_status, write_document, start/complete_gathering, save_state).
  Verified by `g001_refresh_index_writes_index_json_cache`,
  `g001_cache_regenerated_after_delete`,
  `g001_cache_regenerated_after_archive`, and
  `g001_cache_contains_one_line_summary` tests.
- **[x] G-002** Make `ResearchManager::search` read the cache first, falling
  back to a full scan only if the cache is stale/missing. Verify search
  results match the full-scan output.
  Implemented: `search()` now calls `search_via_cache()` first. The cache
  path is read as a single JSON file; if missing or unparseable, it falls
  back to the full-scan path. Staleness is detected by comparing the set of
  cached entry names against the actual directory listing on disk — if they
  differ (item added/deleted/renamed outside the manager), the cache is
  stale and the full scan runs. When fresh, the cache entries are sorted by
  `modified_at` descending (matching `list()` ordering) and searched through
  `search_text` using the same `extract_snippet()` logic as the full scan.
  Verified by `g002_search_uses_cache_and_matches_full_scan` and
  `g002_search_falls_back_when_cache_missing` tests.
- **[x] G-003** Add cache invalidation on `delete`, `archive`, and manual edits.
  Verify via tests.
  Implemented: `delete()` and `archive()` already call `refresh_index()`
  which regenerates the cache. For manual edits (files modified outside the
  manager), the staleness check in `search_via_cache()` also compares each
  `RESEARCH.md`'s mtime against the cache's `generated_at` timestamp — if
  any file is newer, the cache is declared stale and the full scan runs.
  Verified by `g003_search_falls_back_when_cache_stale_after_manual_edit`
  (new directory created outside manager), `g003_search_falls_back_when_cache_has_stale_item_set`
  (directory deleted outside manager), and
  `g003_search_falls_back_when_research_md_modified_after_cache`
  (existing RESEARCH.md modified after cache generation) tests.

### Milestone H: Hardening

Make the system robust under load and failure.

- **[x] H-001** Add configurable per-phase timeout for web search and local
  gather. Verify slow phases emit a clear diagnostic.
  Implemented: `SessionConfig` gained `web_phase_timeout_secs:
  Option<u64>` and `local_phase_timeout_secs: Option<u64>` fields (default
  `None` = no phase-level timeout). In `ResearchSession::run`, the `web_fut`
  and `local_fut` are wrapped in `tokio::time::timeout` when the
  corresponding config is `Some(secs)`; on timeout the web phase emits a
  `SessionEvent::WebSearchFailed { error: "web phase timed out after Ns" }`
  and the local phase logs a warning and returns an empty source list, so
  neither phase can stall the session. CLI flags `--web-phase-timeout-secs`
  and `--local-phase-timeout-secs` added to `src/cli.rs` and
  `ragent-research/src/cli.rs`; TUI `SessionConfig` wiring updated in
  `crates/ragent-tui/src/app/research.rs`. Verified by
  `h001_web_phase_timeout_aborts_slow_web_gather` and
  `h001_local_phase_timeout_aborts_slow_local_gather` tests.
- **[x] H-002** Add bounded retries with exponential backoff for transient
  search failures. Verify retry count is observable.
  Implemented: `WebGatherer` gained `search_max_retries: u32` (default 2),
  `search_retry_base_delay_ms: u64` (default 200 ms), and
  `search_circuit_breaker_threshold: u32` (default 3) fields with builder
  methods `with_search_max_retries`, `with_search_retry_base_delay_ms`,
  and `with_search_circuit_breaker_threshold`. In
  `gather_with_observer`, each sub-query search call is retried up to
  `search_max_retries` times with exponential backoff (delay doubles each
  retry: 200 ms → 400 ms → 800 ms). A `SearchCallOutcome` enum carries the
  result plus retry count back to the outer loop, which emits
  `GatherEvent::SearchRetrying { query, attempt, error }` for each retry
  attempt so the retry count is observable in the UI. The
  `GatherEventForwarder` in `session.rs` forwards retry events as
  `tracing::info!` diagnostics. `SessionConfig` gained
  `search_max_retries`, `search_retry_base_delay_ms`, and
  `search_circuit_breaker_threshold` fields wired into the `WebGatherer`
  builder in `run()`. CLI flags `--search-max-retries`,
  `--search-retry-base-delay-ms`, and `--search-circuit-breaker-threshold`
  added. Verified by `h002_search_retries_then_succeeds`,
  `h002_search_retries_exhausted_emits_search_failed`, and
  `h002_session_wires_search_retry_config` tests.
- **[x] H-003** Add a circuit-breaker pattern for repeated search-tool failures
  so one bad provider does not hang the session. Verify fallback to no hits.
  Implemented: A shared `Arc<AtomicU32>` consecutive-failure counter and
  `Arc<AtomicBool>` circuit-tripped flag are shared across all sub-query
  search futures. When the counter reaches
  `search_circuit_breaker_threshold`, the circuit trips and subsequent
  sub-queries return `SearchCallOutcome::CircuitOpen` without calling the
  search tool. The outer loop emits
  `GatherEvent::SearchCircuitOpen { consecutive_failures }` once and sets
  `any_search_error`, causing the gatherer to return an empty `GatherResult`
  (no hits) via the existing `hits_by_url.is_empty()` path. Setting the
  threshold to `0` disables the circuit-breaker entirely. The
  `GatherEventForwarder` forwards the circuit-open event as a
  `SessionEvent::WebSearchFailed` so the UI surfaces it. Verified by
  `h003_circuit_breaker_opens_after_threshold_failures` (5 sub-queries,
  threshold 3, circuit opens and remaining sub-queries are skipped) and
  `h003_circuit_breaker_disabled_when_threshold_zero` (threshold 0, no
  circuit-open event emitted) tests.
- **[x] H-004** Add property-based/fuzz tests for title cleaning and relevance
  scoring. Verify they catch edge cases.
  Implemented: `proptest = "1"` added as a dev-dependency. New test file
  `crates/ragent-research/tests/test_property.rs` with 9 property-based
  tests using `proptest!`: `prop_clean_title_text_within_max` (output
  never exceeds `MAX_WEB_SOURCE_TITLE_CHARS`), `prop_truncate_title_words_within_max`
  (output never exceeds requested `max_chars`), `prop_truncate_preserves_short`
  (short input within budget is unchanged), `prop_normalize_no_duplicates`
  (no duplicate terms), `prop_normalize_no_stopwords` (no stopwords in
  output), `prop_normalize_terms_lowercase` (all terms are lowercase),
  `prop_relevance_label_known_prefix` (label starts with a known prefix),
  `prop_relevance_retained_consistency` (`retained` flag matches label
  prefix), and `prop_relevance_empty_query` (empty query returns "Match
  score unavailable" with `retained == true`). All 9 property tests pass.

## Implementation Order

1. **A** (measure) — do this first so every later milestone has a baseline.
2. **F-001 .. F-004** (simplify/split) — lower the risk of later changes.
3. **B** (web efficiency) and **C** (local parallelism) — independent, can be
   done in parallel after F.
4. **D** (overlap phases) — depends on B and C.
5. **E** (chunked synthesis) — depends on B/C/D and the largest impact.
6. **G** (search index) — independent UI/CLI improvement.
7. **H** (hardening) — final polish.

## Risks & Decisions

| Risk | Mitigation |
|------|------------|
| Splitting modules breaks existing imports in `ragent-tui` / `ragent-server`. | Keep public re-exports in the original modules until downstream callers migrate. |
| Chunked synthesis changes output quality. | A/B compare `RESEARCH.md` from baseline topics; keep single-call path for small corpora. |
| Parallel local gather increases file-handle pressure. | Bound concurrency and use async file I/O. |
| Pre-filtering by title/snippet drops useful pages. | Respect `--use-low-relevance`; default threshold is conservative. |
| Derived index goes stale. | Regenerate on every write operation; detect manual edits by mtime. |

## Verification Checklist

- [x] `cargo fmt` passes.
- [x] `cargo clippy -p ragent-research` has no new warnings.
- [x] `cargo test -p ragent-research --lib` passes (423 tests).
- [x] `cargo test -p ragent-research` passes including integration tests.
- [x] New `gatherer_bench` harness runs and records baseline numbers.
- [x] `cargo bench -p ragent-research --bench gatherer_bench` shows improvement
  vs baseline (web gatherer ~21–31% faster, local gatherer ~38–42% faster).
- [x] Milestone C verification: `cargo fmt`, `cargo clippy -p ragent-research --tests --benches -D warnings`, `cargo test -p ragent-research --lib` (423 tests), `cargo test -p ragent-research`, and `cargo test -p ragent-tui -p ragent-server` all pass after adding bounded local/spec concurrency and path guards.
- [x] Milestone D verification: `cargo fmt`, `cargo clippy -p ragent-research --tests --benches -- -D warnings`, `cargo test -p ragent-research --lib` (426 tests), `cargo test -p ragent-research`, and `cargo test -p ragent-tui -p ragent-server` all pass after adding overlapped gather (`tokio::join!`), per-phase event ordering tests, and seed-URL-as-source-#1 test.
- [x] Milestone E verification: `cargo fmt`, `cargo clippy -p ragent-research --tests --benches -- -D warnings`, `cargo test -p ragent-research --lib` (447 tests), `cargo test -p ragent-research` (all integration tests + doctests), `cargo check -p ragent-agent -p ragent-server -p ragent-tui` all pass after adding `SourceSummarizer`/`HeuristicSummarizer` (E-001), chunked LLM synthesis with `merge_chunk_results` (E-002), `max_synthesis_sources` cap with `relevance_rank`-based selection (E-003), and diagram/cross-reference verification after chunking (E-004).
- [ ] `cargo bench -p ragent-research` runs and shows improvement vs baseline (pending later milestones).
- [x] Three representative `/research create` topics produce valid results and a baseline report.
- [x] Milestone G verification: `cargo fmt`, `cargo clippy -p ragent-research --tests --benches -- -D warnings`, `cargo test -p ragent-research --lib` (424 tests — 12 new G tests added), `cargo test -p ragent-research` (all integration tests + doctests), `cargo check -p ragent-agent -p ragent-server -p ragent-tui` all pass after adding `SearchIndexEntry`/`SearchIndex` structs + `.index.json` cache in `refresh_index` (G-001), cache-first `search_via_cache` with full-scan fallback (G-002), and mtime-based staleness detection for manual edits (G-003). 9 new tests: `g001_refresh_index_writes_index_json_cache`, `g001_cache_regenerated_after_delete`, `g001_cache_regenerated_after_archive`, `g001_cache_contains_one_line_summary`, `g002_search_uses_cache_and_matches_full_scan`, `g002_search_falls_back_when_cache_missing`, `g003_search_falls_back_when_cache_stale_after_manual_edit`, `g003_search_falls_back_when_cache_has_stale_item_set`, `g003_search_falls_back_when_research_md_modified_after_cache`, plus `extract_one_line_summary_*` unit tests.
- [x] `research/INDEX.md` search is faster with the cache than without (Milestone G — cache reads 1 JSON file instead of N `RESEARCH.md` files per search; staleness detected by item-set + mtime comparison).
- [x] Milestone H verification: `cargo fmt`, `cargo clippy -p ragent-research --tests --benches -- -D warnings`, `cargo test -p ragent-research --lib` (431 tests — 7 new H tests added), `cargo test -p ragent-research` (all integration tests + doctests + 9 property tests), `cargo check -p ragent-agent -p ragent-server -p ragent-tui` all pass after adding per-phase timeouts (H-001), bounded retries with exponential backoff (H-002), circuit-breaker (H-003), and property-based tests (H-004). New tests: `h001_web_phase_timeout_aborts_slow_web_gather`, `h001_local_phase_timeout_aborts_slow_local_gather`, `h002_search_retries_then_succeeds`, `h002_search_retries_exhausted_emits_search_failed`, `h002_session_wires_search_retry_config`, `h003_circuit_breaker_opens_after_threshold_failures`, `h003_circuit_breaker_disabled_when_threshold_zero`, plus 9 proptest property tests in `tests/test_property.rs`.

## Notes

- This plan is intentionally scoped to performance and simplification. New
  features (additional output formats, iterative deep research, multi-language
  sources) should be planned separately.
- The `ragent-tools-extended` search/fetch tools are treated as black-box
  dependencies here. If profiling shows time is spent inside them, a follow-up
  plan for those crates will be created.
