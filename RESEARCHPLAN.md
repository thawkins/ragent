# ragent-research Improvement Plan

> Derived from the combined explore-agent review at
> `log/subagents/wait-batch-1787661708029.md` and the four per-agent reports
> (`explore-c7c017e2`, `explore-1b2a58b6`, `explore-2c5a261b`,
> `explore-6b83611f`).
>
> Last updated: 2026-08-25

## Progress Summary

- **Phase 1 implemented** in this session:
    - Created `crates/ragent-research/src/run_request.rs` with `ResearchRunRequest`
      and `build_session_config`.
    - Refactored CLI (`src/cli.rs`), TUI (`crates/ragent-tui/src/app/research.rs`),
      and HTTP route (`crates/ragent-server/src/routes/research.rs`) to use the
      shared builder.
    - Added `tier`, phase timeouts, search retry settings, and synthesis source
      limits to the HTTP `POST /research` request schema.
    - Added `cargo:rerun-if-changed=build.rs` to
      `crates/ragent-research/build.rs`.
    - Resolved the duplicate `tokio` dependency in
      `crates/ragent-research/Cargo.toml` by using the workspace dependency.
    - Cached compiled regexes in `cite_checker.rs`, `verify.rs`, `synthesis.rs`,
      and `diagram.rs` via `OnceLock`.
    - Verified: `cargo check`, `cargo clippy`, and `cargo test -p ragent-research`
      all pass.

- **Phase 2 implemented** in this session:
    - **R-005**: Extracted stage helpers from `ResearchSession::run` — the
      ~1,200-line mega-method is now reduced to orchestration, with
      `fetch_from_url_seeds`, `extract_from_file_seeds`, and `overlapped_gather`
      extracted as named async helper methods.
    - **R-006**: Added `TierRouter::run_step_if` generic helper and converted
      all synchronous dispatch sites (ContradictionGraph, LociAnalysis,
      DepthInvestigation, CrossLocusReconcile, SourceTensions, EvidenceDigest,
      CorpusCritic, TripleDraft, Critics, Patcher, Polish, ReadabilityAudit,
      WidthSweep, Decompose) to use it, eliminating ~15 boilerplate blocks.
    - **R-007**: `SessionConfig` already restructured into nested sub-configs
      (`InputConfig`, `OutputConfig`, `WebConfig`, `LocalConfig`,
      `AnalysisConfig`, `ResilienceConfig`, `RunEngineConfig`) — verified and
      re-exported from `lib.rs`.
    - **R-008**: Introduced nested `SessionEvent` hierarchy with `AnalysisEvent`
      (9 variants) and `SynthesisEvent` (7 variants) sub-enums, reducing the
      top-level enum from 35 to 21 flat variants. All emission sites and
      consumers (TUI, CLI, server) updated.
    - **R-009**: Fixed 0-based finding numbers in synthesis critic reports (already
      applied in Phase 1 uncommitted changes; verified with regression tests).
    - **R-010**: Made contradiction dimensions configurable via
      `ContradictionConfig` / `PolarityDimension`. Default preserves the original
      six medical/tech dimensions; callers can supply custom dimensions for
      non-medical topics via `AnalysisConfig::contradiction`. Added
      `build_contradiction_graph_with` configurable variant.
    - **R-011**: Fixed reconcile conflicting-edge count to use shared source
      indices instead of comparing dimension labels to locus labels. Added
      regression tests for non-matching dimension labels and unrelated sources.
    - **R-014**: Moved shared polarity helpers (`source_body_text`,
      `depth_from_count`) into a new internal `polarity.rs` module, eliminating
      ~100 lines of duplication across `contradiction.rs`, `corpus_critic.rs`,
      and `reconcile.rs`.
    - **R-028**: Migrated inline `#[cfg(test)]` modules for `contradiction.rs`,
      `reconcile.rs`, and `corpus_critic.rs` into `tests/` directory
      (`test_contradiction.rs`, `test_reconcile.rs`, `test_corpus_critic.rs`).
      Tests that access private functions (e.g. `synthesis.rs` critic helpers)
        remain inline pending API widening or `#[path]` migration.
      - Verified: `cargo check`, `cargo clippy`, and `cargo test -p ragent-research`
        all pass.
  
  - **Phase 3 implemented** in this session:
      - **R-019**: Pre-sized output buffers and replaced `format!` with
        `write!`/`push_str` in `document.rs` renderers (`assemble_document`,
        `render_finding_sources`, `render_bibliography`,
        `normalize_finding_labels`). Cached `extract_cited_source_indices`
        and `normalize_finding_labels` regexes in `OnceLock`. Replaced
        `to_lowercase().contains()` with direct `find`. Short-circuited
        `body.chars().count() > 240` with `.take(241).count()`.
      - **R-020**: Fixed `body_preview_of` perf wart in `web_gatherer.rs` —
        removed redundant final `.chars().take(256).collect()` re-iteration,
        replaced with truncate-on-overshoot. Helper was already shared
        between vault and fetch paths.
      - **R-021**: Changed `collect_matched_terms` `HashSet<String>` to
        `HashSet<&str>` to eliminate double `to_string()` per hit. Pre-
        lowercased terms once instead of per-comparison.
      - **R-022**: Already correct — `gather()` applies
        `take(max_local_sources)` before body reads. No change needed.
      - **R-023**: Added mtime-keyed `item_cache`
        (`Arc<Mutex<HashMap<PathBuf, (SystemTime, ResearchItem)>>>)` to
        `ResearchManager`. `show()` and `list()` now use `read_item_cached()`
        which checks mtime before reading from disk. `transition_status()`
        reads file once instead of double-read. Writes invalidate cache.
      - **R-024**: Replaced two `sources.clone()` calls with
        `std::mem::take(&mut sources)` to move ownership instead of
        deep-cloning. Updated all post-synthesis references to use
        `&synthesis_sources`. The double clone (clone + `to_vec()` in
        `synthesize`) is eliminated.
      - **R-025**: Added `is_stopword_lc` filter to `KeywordVerifier::words()`
        and changed `supported_by()` to require ≥2 non-trivial content-word
        overlaps (or ≥1 for very short findings).
      - **R-026**: Added `BOLD_NUM_RE` regex to match `**Finding N:**` and
        `**N.**` formats in `renumber_findings`. Cached both regexes in
        `OnceLock`.
      - **R-015**: Added `Continue` variant to clap `ResearchCommands` enum.
        Wired it to `manager.continue_item()` + session re-run, replacing the
        dead stub.
      - **R-016**: `POST /research` now returns `202 Accepted` immediately
        with `Location` header pointing to `GET /research/{name}/events`.
        Research runs in a background `tokio::spawn` task. Added SSE endpoint
        `GET /research/{name}/events` that streams `SessionEvent`s via
        broadcast channel. Added `research_runs` registry to `AppState`.        - Verified: `cargo check`, `cargo clippy`, and
          `cargo test -p ragent-research` all pass (606 lib tests + all
          integration tests).

  - **Phase 4 implemented** in this session:
      - **R-017**: Extracted `session_event_json` from
        `render_session_event_json` — the SSE handler now calls the pure
        JSON helper directly instead of stripping the `ragent-research: `
        CLI prefix. `render_session_event_json` delegates to
        `session_event_json` and wraps it with the prefix.
      - **R-018**: Extended `ResearchItemRow` with `topic`, `queries`,
        `output_format`, and `model` fields (all `skip_serializing_if`).
        Added `?full=true` query param to `GET /research/{name}` to
        control whether extended fields are included.
      - **E2E tests**: Added `test_research_run_request.rs` (24 tests)
        verifying `build_session_config` maps all fields correctly — tier,
        output format, depth, web/local config, resilience config, OA
        settings — plus `session_event_json` pure-JSON validation. Added
        `test_research_routes.rs` (13 tests) covering auth, list, show
        (base + `?full=true`), delete (with/without confirmation), POST
        (202+Location, invalid name, duplicate conflict), and SSE events
        (no active run, not found).
      - Fixed pre-existing compile errors in `test_integration.rs` and
        `test_memory_api.rs` (missing `research_runs` field in `AppState`).
      - Fixed research route registration — `research_routes()` was using
        absolute paths (`/research/...`) inside `.nest("/research", ...)`,
        causing doubled prefixes; now uses relative paths.
      - **R-032**: Updated SPEC.md (HTTP Research API §11.9, Research Tiers
        §11.8, `imrad` format, `research` SSE event type, endpoint table),
        QUICKSTART.md (202+Location, SSE, `?full=true`, `--tier`), and
        CHANGELOG.md.
      - Verified: `cargo test -p ragent-research` and
        `cargo test -p ragent-server --test test_research_routes` pass.

---

## 1. Project Context and Scope

The `ragent-research` crate provides structured information gathering for the
ragent agent system. It is exposed through three front-ends:

- **CLI** (`src/cli.rs` / `crates/ragent-research/src/cli.rs`)
- **TUI** (`crates/ragent-tui/src/app/research.rs`)
- **HTTP server** (`crates/ragent-server/src/routes/research.rs`)

The subsystem runs an async research pipeline that overlaps web search,
local-file/spec gathering, adversarial analysis, LLM synthesis, QA auditing,
and final document assembly into a self-contained `RESEARCH.md`.

This plan documents findings from four focused reviews:

1. **Performance review** of core source files (`document.rs`,
   `web_gatherer.rs`, `local_gatherer.rs`, `manager.rs`, `engine.rs`).
2. **Cross-interface audit** of CLI/TUI/HTTP capability drift and default
   mismatches.
3. **Analysis / synthesis / QA review** (`analysis.rs`, `synthesis.rs`,
   `diagram.rs`, `contradiction.rs`, `cite_checker.rs`, `verify.rs`,
   `corpus_critic.rs`, `reconcile.rs`).
4. **Crate-level review** (`Cargo.toml`, `build.rs`, `lib.rs`, `session.rs`).

The goal is to fix user-visible bugs, eliminate duplicated configuration
logic, improve performance and maintainability, and converge the three
front-ends onto a single shared research request abstraction.

---

## 2. Executive Summary of Findings

### 2.1 Architecture

- `ResearchSession::run` in `session.rs` is a single mega-method (~1,200 lines)
  that couples stage dispatch, state management, and error handling. The tier
  router exists, but every stage is dispatched with ~15 repetitions of the
  same boilerplate.
- `SessionConfig` is a "god struct" mixing topic inputs, I/O paths, gathering
  knobs, resilience knobs, output format, tier/depth, and OA recovery settings.
- `lib.rs` re-exports 80+ items from 35 modules, exposing implementation details
  such as `TierRouterToSessionObserver` and `build_surgical_patches`.
- `SessionEvent` has 30+ variants, forcing front-ends to carry large `match`
  arms or ignore variants.

### 2.2 Performance

- The highest-impact issue is **synchronous file/SQLite work inside
  `tokio::task::spawn_blocking`** in `manager.rs`, with many small calls entering
  and exiting the blocking pool instead of batched transactions.
- `document.rs` repeatedly allocates temporary `String`s with `format!` and
  `push_str`, leading to `O(n²)`-style buffer growth in `assemble_document`.
- `local_gatherer.rs` reads file bodies for every scored candidate before
  `take(max_local_sources)`, wasting I/O and memory.
- `engine.rs` / `session.rs` clone the entire source corpus into
  `spawn_blocking`.
- Regexes are recompiled on every call in `cite_checker.rs`, `verify.rs`,
  `synthesis.rs`, and `diagram.rs`.

### 2.3 Functionality / Correctness

- **User-visible bug:** synthesis critic reports use 0-based finding numbers,
  inconsistent with 1-based numbering everywhere else.
- **Contradiction detector** is hard-coded to six medical/tech dimensions
  (`effect`, `mortality`, `performance`, `cost`, `adoption`, `safety`), making
  it useless for most topics and contradicting the general-purpose README
  claims.
- **Reconcile "conflicting edges"** logic compares dimension labels to locus
  labels, which almost never match, producing misleading "0 conflicting
  edges" notes.
- `CorpusCriticReport.balance_score` actually measures imbalance (a one-sided
  corpus scores 100).
- `KeywordVerifier` uses a weak 4-character word-overlap heuristic that is
  both too strict and too loose.
- Chunked synthesis merge renumbering only renumbers lines beginning with
  `N.`, missing the bold-labelled findings the LLM emits.
- `NoopAnalysisEngine` uses a trait marker method instead of an enum.

### 2.4 Cross-Interface

- The three front-ends build `SessionConfig` independently with divergent
  defaults and missing fields. The HTTP route cannot set `tier`, uses
  hard-coded `open_access_recovery = false`, and does not propagate
  `contact_email` / `oa_min_full_text_chars` / retry settings from
  `ragent.json`.
- CLI `ResearchCommands` has malformed indentation; the `continue` subcommand
  is a dead stub.
- HTTP research runs block until completion with no SSE or 202-accepted
  background mode.
- Event serialization has a brittle CLI-prefix strip in the HTTP route.
- `ResearchItemRow` omits `topic`, `queries`, `output_format`, and `model`.

---

## 3. Detailed Problem Inventory

### 3.1 Performance — `document.rs`

| ID | File / Lines | Issue | Impact |
|----|--------------|-------|--------|
| PERF-DOC-01 | `document.rs:272–310` | `assemble_document` repeatedly calls `push_str(&format!(...))`, causing repeated reallocation and `O(n²)` growth for large reports. | Medium–High |
| PERF-DOC-02 | `document.rs:223–236` | `make_headline_from_observation` runs three full `.replace()` passes, allocates a `Vec`, then joins + `to_string`. | Medium |
| PERF-DOC-03 | `document.rs:1160–1165,1377,1553` | `strip_control_chars` and `escape_pipe` are applied redundantly across render paths. | Low–Medium |
| PERF-DOC-04 | `document.rs:2106–2187` | `normalize_finding_labels` builds two `Vec<String>` plus intermediate strings. | Medium |
| PERF-DOC-05 | `document.rs:2025–2104` | `split_analysis_sentences` collects `Vec<char>` and allocates a new `String` per sentence. | Medium |
| PERF-DOC-06 | `document.rs:1726–1787` | `linkify_outside_code` collects `Vec<(usize, char)>` and copies char-by-char; binary search per skip is `O(log n)`. | Medium |
| PERF-DOC-07 | `document.rs:1577–1620` | `render_finding_sources` allocates `format!` strings inside a loop. | Low–Medium |
| PERF-DOC-08 | `document.rs:2207–2264` | `render_search_engine_summary` clones engine names into a `BTreeMap<String, Counts>`. | Low |
| PERF-DOC-09 | `document.rs:2273–2307` | `render_bibliography` counts chars twice, collects a preview, and allocates another copy via `.replace('\n', "\n  ")`. | Medium |
| PERF-DOC-10 | `document.rs:1541–1551` | `apply_template` performs chained `.replace()` calls, each scanning the whole template. | Low–Medium |

### 3.2 Performance — `web_gatherer.rs`

| ID | File / Lines | Issue | Impact |
|----|--------------|-------|--------|
| PERF-WEB-01 | `web_gatherer.rs:150–175` | `is_scholarly_hit` / `is_encyclopedia_hit` allocate `Vec<&str>` just to check `all()`. | Low |
| PERF-WEB-02 | `web_gatherer.rs:914–931` | `gather_from_vault` reads full source bodies, collects lines into `Vec`, joins them, then takes `chars()`. | Medium |
| PERF-WEB-03 | `web_gatherer.rs:1265,1323,1342` | Search/fetch outcomes clone `query` heavily inside loops. | Low |
| PERF-WEB-04 | `web_gatherer.rs:1753–1761` | `body_preview` logic is duplicated and expensive. | Low |
| PERF-WEB-05 | `web_gatherer.rs:1634–1701` | Open-access recovery can issue a second full fetch inline per source, blocking that source's completion. | Medium |
| PERF-WEB-06 | `web_gatherer.rs:1221–1255,1324–1333` | `log_url_outcome` creates a fresh `serde_json::json!` `Value` and acquires a `Mutex` per URL. | Medium |
| PERF-WEB-07 | `web_gatherer.rs:1992–1998` | `body256` test helper uses `chars().count()` in a loop (`O(k²)` in tests). | Low |

### 3.3 Performance — `local_gatherer.rs`

| ID | File / Lines | Issue | Impact |
|----|--------------|-------|--------|
| PERF-LOC-01 | `local_gatherer.rs:570–586` | `collect_matched_terms` is `O(matches × terms × line_length)` and lowercases on every comparison. | Medium |
| PERF-LOC-02 | `local_gatherer.rs:593–636` | `derive_terms` builds intermediate `String` then `Vec<String>`. | Low |
| PERF-LOC-03 | `local_gatherer.rs:478–527` | `build_local_excerpt` collects `chars().take(200)` per line and uses `format!` per emitted line. | Low–Medium |
| PERF-LOC-04 | `local_gatherer.rs:390–394` | `gather_specs` allocates a new `String` per spec for a case-insensitive contains check. | Low |
| PERF-LOC-05 | `local_gatherer.rs:290–320` | `enumerate_candidates` runs globs fully sequentially. | Medium |
| PERF-LOC-06 | `local_gatherer.rs:229–248` | File bodies are read for every scored candidate before `take(max_local_sources)`. | Medium–High |

### 3.4 Performance — `manager.rs` / `engine.rs`

| ID | File / Lines | Issue | Impact |
|----|--------------|-------|--------|
| PERF-MGR-01 | `manager.rs` (persistence calls) | Synchronous file/SQLite work wrapped per-operation in `spawn_blocking`. | High |
| PERF-MGR-02 | `manager.rs` (inferred) | No LRU cache for parsed item metadata / supporting files. | Medium |
| PERF-MGR-03 | `manager.rs` (inferred) | Event emitters/observers may block the gather pipeline. | Medium |
| PERF-ENG-01 | `engine.rs` / `session.rs` | Prompt construction duplicates source bodies; full corpus cloned into `spawn_blocking`. | High |
| PERF-ENG-02 | `engine.rs` (inferred) | Source bodies cleaned multiple times before LLM calls. | Medium |
| PERF-ENG-03 | `engine.rs` (inferred) | QA pipeline stages run sequentially even when independent. | Medium–High |

### 3.5 Functionality — Analysis / Synthesis / QA

| ID | File / Lines | Issue | Impact |
|----|--------------|-------|--------|
| FUNC-ANL-01 | `synthesis.rs:~275,~286,~341,~350` | Synthesis critic reports use 0-based finding numbers; user sees "Finding 0". | High |
| FUNC-ANL-02 | `contradiction.rs:~126–212` | `POLARITY_DIMENSIONS` hard-coded to medical/tech dimensions; useless for most topics. | High |
| FUNC-ANL-03 | `reconcile.rs:~170–182` | Conflicting-edge count compares dimension labels to locus labels; almost always 0. | High |
| FUNC-ANL-04 | `cite_checker.rs:~78`, `verify.rs:~59`, `synthesis.rs:~262`, `diagram.rs:~84` | Regexes recompiled on every call / loop iteration. | Medium–High |
| FUNC-ANL-05 | `corpus_critic.rs:~208–224` | `balance_score` measures imbalance, not balance. | Medium |
| FUNC-ANL-06 | `contradiction.rs:~126–212`, `corpus_critic.rs:~350–394`, `reconcile.rs:~313` | Duplicated polarity token lists and helpers (`source_body_text`, `depth_from_count`). | Medium |
| FUNC-ANL-07 | `verify.rs:~70–91` | Keyword verifier is a weak proxy for source support; common words cause false passes, paraphrases fail. | Medium |
| FUNC-ANL-08 | `analysis.rs:~777–790` | `renumber_findings` only matches leading `N.`, missing bold-labelled findings. | Medium |
| FUNC-ANL-09 | `analysis.rs:~84–116,~143–159` | `NoopAnalysisEngine` trait marker is a code smell; enum would be cleaner. | Medium |
| FUNC-ANL-10 | `corpus_critic.rs` (gap-fill plumbing) | Gap-fill fetch plumbing appears unused. | Medium |
| FUNC-ANL-11 | `analysis.rs` (inferred) | `stream_synthesis` has hard-coded generation limits. | Medium |
| FUNC-ANL-12 | `verify.rs` | `KeywordVerifier::verify` returns `passed: true` when there are no findings. | Low–Medium |
| FUNC-ANL-13 | `contradiction.rs` (inferred) | `add_edge` re-sorts on every insertion. | Low–Medium |
| FUNC-ANL-14 | `reconcile.rs` | `build_cross_locus_reconcile` does unnecessary collection. | Low–Medium |
| FUNC-ANL-15 | `analysis.rs` / `synthesis.rs` / `session.rs` | Inline `#[cfg(test)]` modules violate project test conventions. | Low–Medium |
| FUNC-ANL-16 | `cite_checker.rs:~67–140` | Only validates citation existence, not relevance. | Low |
| FUNC-ANL-17 | `analysis.rs:~387–434` | `summarize_subject` silently swallows errors. | Low |
| FUNC-ANL-18 | `diagram.rs:~268–280` | `escape_mermaid_label` is lossy and order-dependent. | Low |

### 3.6 Cross-Interface — CLI / TUI / HTTP

| ID | File / Lines | Issue | Impact |
|----|--------------|-------|--------|
| XINT-01 | `src/cli.rs:402–436`, `crates/ragent-tui/src/app/research.rs:111–159`, `crates/ragent-server/src/routes/research.rs:170–194` | Three independent `SessionConfig` builders with divergent defaults and missing fields. | Critical |
| XINT-02 | `crates/ragent-server/src/routes/research.rs:105–163` | HTTP `CreateResearchRequest` is missing `tier`; silently defaults to `Tier::Full`. | High |
| XINT-03 | `crates/ragent-server/src/routes/research.rs:195–201` | HTTP title derivation ignores `from_files`; no empty-source validation. | High |
| XINT-04 | `crates/ragent-server/src/routes/research.rs:202–218` | HTTP research blocks until completion; no SSE or 202-accepted background mode. | High |
| XINT-05 | `src/cli.rs:73–80` | `ResearchCommands` enum has malformed indentation and is fragile to `cargo fmt`. | High |
| XINT-06 | `crates/ragent-tui/src/app/research.rs:127–129` | TUI hard-codes defaults that should come from `SessionConfig::default()`. | Medium |
| XINT-07 | `src/cli.rs:488–493` | `ResearchCliCommand::Continue` is a dead stub. | Medium |
| XINT-08 | `crates/ragent-server/src/routes/research.rs` (inferred) | HTTP `disable_scholarly` exists but CLI exposes only `use_scholarly` semantics. | Medium |
| XINT-09 | `crates/ragent-research/src/cli.rs` | `ResearchCliCommand` is a separate enum duplicating fields instead of being derived from a shared request type. | Medium |
| XINT-10 | `crates/ragent-server/src/routes/research.rs:251–260` | Different event serialization paths; HTTP manually strips `ragent-research: ` prefix. | Medium |
| XINT-11 | `crates/ragent-server/src/routes/research.rs:93–101` | `ResearchItemRow` omits `topic`, `queries`, `output_format`, `model`. | Low |
| XINT-12 | `src/run_config.rs` etc. | Some paths from the original prompt live under `crates/ragent-research/src/`, not `src/`. | Low |

### 3.7 Crate-Level Maintainability

| ID | File / Lines | Issue | Impact |
|----|--------------|-------|--------|
| CRATE-01 | `crates/ragent-research/src/session.rs` | `ResearchSession::run` is ~1,200 lines. | Critical |
| CRATE-02 | `crates/ragent-research/src/session.rs` | Inline `#[cfg(test)]` module violates project test guidelines. | High |
| CRATE-03 | `crates/ragent-research/src/session.rs:~163–299` | `SessionConfig` is a god struct with ~30 fields. | High |
| CRATE-04 | `crates/ragent-research/src/lib.rs:80–165` | Very broad public API surface. | Medium |
| CRATE-05 | `crates/ragent-research/src/session.rs:~402–692` | `SessionEvent` enum has 30+ variants. | Medium |
| CRATE-06 | `crates/ragent-research/src/session.rs:~1239,1466,1482,1498,1513,1529,1544,1564,1627,1776,1794,1829,1844` | Tier-router step dispatch is repetitive boilerplate (~15 copies). | Medium |
| CRATE-07 | `crates/ragent-research/src/session.rs:~2105–2149` | `synthesize` clones entire source corpus into `spawn_blocking`. | Medium |
| CRATE-08 | `crates/ragent-research/src/session.rs:~742–774` | `ResearchSession` clone-cost doc claim may not match reality. | Medium |
| CRATE-09 | `crates/ragent-research/build.rs:12–19` | No `rerun-if-changed` directives. | Low |
| CRATE-10 | `crates/ragent-research/Cargo.toml:30,38` | `tokio` declared twice with different feature sets. | Low |
| CRATE-11 | `crates/ragent-research/src/lib.rs:27–38` | Stale module documentation. | Low |
| CRATE-12 | `crates/ragent-research/src/session.rs:333,346` | Hardcoded numeric defaults scattered across modules. | Low |

---

## 4. Proposed Solution Approach

### 4.1 Cross-interface convergence

Introduce a single shared research request type and builder inside
`ragent-research`:

```text
crates/ragent-research/src/run_request.rs
```

- `ResearchRunRequest` — neutral, serializable, front-end-agnostic schema.
- `ResearchRunRequest::validate()` — centralises empty-topic/source validation
  and title derivation.
- `build_session_config(&ResearchRunRequest, Option<&ragent_config::Config>)`
  — the only place that maps user input into `SessionConfig`.

CLI, TUI, and HTTP become thin adapters that populate `ResearchRunRequest` and
hand it to the shared builder. This eliminates default drift and guarantees
that all three surfaces expose identical capabilities.

### 4.2 Session refactor

- Extract `ResearchSession::run` into small async stage helpers, one per
  `RunStep`.
- Add a `TierRouter::run_step_if` helper to remove the ~15 copies of the same
  dispatch boilerplate.
- Restructure `SessionConfig` into nested sub-configs (`SeedConfig`,
  `WebConfig`, `LocalConfig`, `OutputConfig`, `ResilienceConfig`,
  `OpenAccessConfig`).
- Replace the flat `SessionEvent` enum with a nested hierarchy so observers
  only match the branches they care about.
- Make `ResearchSession` clone cost match its doc comment by wrapping
  `manager`, `web`, and `local` in `Arc` where appropriate, or update the
  comment.

### 4.3 Performance fixes

- `document.rs`: pre-size output buffers; replace `format!` inside loops with
  `write!`; build subsections with `Vec<&str>`/`Vec<String>` and join once;
  operate on byte/char indices instead of collecting `Vec<char>`.
- `web_gatherer.rs`: add a single-pass `preview_body` helper; avoid `Vec`
  allocation for simple all-checks; batch URL logging through a channel;
  cap open-access recovery parallelism.
- `local_gatherer.rs`: pre-lowercase terms once; use `Aho-Corasick` or a
  `HashSet`; sort and `take(max_local_sources)` **before** reading bodies;
  run globs concurrently.
- `manager.rs`: batch SQLite operations inside one `spawn_blocking`
  transaction; cache parsed `ResearchItem` metadata by mtime.
- `engine.rs` / `session.rs`: pass `&[Source]` or `Arc<[Source]>` into blocking
  work; clean source bodies once at ingestion.

### 4.4 Analysis / synthesis / QA fixes

- Use `idx + 1` in all synthesis critic diagnostic strings.
- Replace hard-coded `POLARITY_DIMENSIONS` with a configurable
  `ContradictionConfig` / `PolarityDimension` type and share token lists.
- Fix reconcile conflicting-edge logic to count edges whose source indices
  intersect the shared source set.
- Cache compiled regexes with `std::sync::OnceLock`.
- Invert `CorpusCriticReport.balance_score` semantics or rename it to
  `dominance_score`.
- Add a stop-word filter and require multiple non-trivial content-word
  overlaps in `KeywordVerifier`.
- Improve `renumber_findings` to normalise and rewrite any leading number in
  structured finding labels.
- Convert `AnalysisEngine` from a trait-with-marker to an enum or remove the
  marker.
- Migrate inline tests to `crates/ragent-research/tests/` per project
  convention.

### 4.5 HTTP improvements

- Extend `CreateResearchRequest` with `tier`, `title`, `disable_scholarly`.
- Return `202 Accepted` with a `Location` header and run research in a
  background task; expose `GET /research/{name}` for status and
  `GET /research/{name}/events` for SSE progress.
- Split `render_session_event_json` into a raw JSON helper plus a prefixed
  CLI line renderer.
- Extend `ResearchItemRow` with `topic`, `queries`, `output_format`, `model`
  (or add `?full=true`).

### 4.6 Tooling / build hygiene

- Add `cargo:rerun-if-changed=build.rs` and `cargo:rerun-if-env-changed=COMPILE_TIME`
  to `build.rs`.
- Resolve duplicate `tokio` dependency in `Cargo.toml` using workspace features.
- Update stale module docs in `lib.rs`.
- Move numeric defaults to module-level constants.

---

## 5. Milestones

### Phase 1 — Foundation (shared request, build hygiene, quick wins) ✅

*Completed 2026-08-25.*

- Create `run_request.rs` and `build_session_config`.
- Refactor CLI to use `ResearchRunRequest` and the shared builder.
- Refactor TUI to use the shared builder; remove hard-coded defaults.
- Refactor HTTP route to use the shared builder; add `tier` and missing
  `ragent.json` fields.
- Add `rerun-if-changed` directives to `build.rs`.
- Resolve duplicate `tokio` dependency.
- Apply regex `OnceLock` caching across `cite_checker`, `verify`, `synthesis`,
  `diagram`.

**Success criteria:**

- All three front-ends build `SessionConfig` through a single function.
- `cargo check` and `cargo clippy` pass with no new warnings.
- Existing research tests still pass.

*Verification: `cargo check`, `cargo clippy`, and `cargo test -p ragent-research` all pass after the changes.*

### Phase 2 — Core refactor (session, events, config, analysis) ✅

*Completed 2026-08-25.*

- Extract stage helpers from `ResearchSession::run`.
- Add `TierRouter::run_step_if` and convert all dispatch sites.
- Restructure `SessionConfig` into nested sub-configs.
- Introduce nested `SessionEvent` hierarchy.
- Fix 0-based finding numbers.
- Make polarity dimensions configurable and shared.
- Fix reconcile conflicting-edge logic.
- Migrate inline tests to `crates/ragent-research/tests/`.

**Success criteria:**

- `session.rs` no longer contains a single 1,200-line method.
- No inline `#[cfg(test)]` modules remain in the crate library source.
- Synthesis critic outputs 1-based finding numbers.
- Contradiction dimensions are configurable.

*Verification: `cargo check`, `cargo clippy`, and `cargo test -p ragent-research` all pass after the changes.*

### Phase 3 — QA / polish (performance, verifier, HTTP experience)

- Pre-size buffers and replace `format!` with `write!` in `document.rs`.
- Add single-pass `preview_body` helper in `web_gatherer.rs`.
- Pre-lowercase terms and use efficient multi-term matching in
  `local_gatherer.rs`.
- Sort/take before reading bodies in `local_gatherer.rs`.
- Batch SQLite operations and cache parsed items in `manager.rs`.
- Avoid full-corpus clones in `synthesize`.
- Improve `KeywordVerifier` stop-word filter and overlap requirements.
- Improve `renumber_findings`.
- Implement or remove the CLI `continue` stub.
- Add SSE or 202-accepted background HTTP research runs.

**Success criteria:**

- Large-report assembly benchmarks show measurable improvement.
- Local gatherer no longer reads bodies for discarded candidates.
- HTTP research runs do not block the connection until completion.

### Phase 4 — Integration / rollout

- Add end-to-end tests covering CLI, TUI, and HTTP research runs from a
  common request fixture.
- Extend `ResearchItemRow` and add `?full=true` if needed.
- Update `SPEC.md` and `QUICKSTART.md` for new HTTP endpoints and tier
  parameter.
- Update `CHANGELOG.md`.
- Run full test suite (`cargo test`) and benchmarks.

**Success criteria:**

- `cargo test` passes for the workspace.
- Documentation reflects the unified request model and new HTTP behaviour.
- No regressions in research output quality.

---

## 6. Task Table

| ID | Area | Task | Priority | Acceptance Criteria | Dependencies | Notes |
|----|------|------|----------|---------------------|--------------|-------|
| R-001 | Cross-Interface | Create `crates/ragent-research/src/run_request.rs` with `ResearchRunRequest` and `build_session_config` | P0 | New module compiles; covers all fields from CLI/TUI/HTTP; centralises validation and title derivation | — | Derived from XINT-01, XINT-03 |
| R-002 | Cross-Interface | Refactor CLI (`src/cli.rs`) to use `ResearchRunRequest` and `build_session_config` | P0 | CLI builds and tests pass; no direct `SessionConfig` construction in `src/cli.rs` | R-001 | Fixes XINT-05 dead stub separately in R-015 |
| R-003 | Cross-Interface | Refactor TUI (`crates/ragent-tui/src/app/research.rs`) to use `ResearchRunRequest` and `build_session_config` | P0 | TUI builds and tests pass; removes hard-coded defaults | R-001 | Fixes XINT-06 |
| R-004 | Cross-Interface | Refactor HTTP (`crates/ragent-server/src/routes/research.rs`) to use `ResearchRunRequest` and `build_session_config`; add `tier` and missing config fields | P0 | Server builds and tests pass; HTTP can set tier and propagate OA/retry config | R-001 | Fixes XINT-02 |
| R-005 | Architecture | Extract stage helpers from `ResearchSession::run` | P0 | `run` method reduced to orchestration; each stage in a named helper; tests pass | — | Fixes CRATE-01 |
| R-006 | Architecture | Add `TierRouter::run_step_if` helper and convert all dispatch sites | P0 | ~15 boilerplate blocks replaced; no behavioural change | R-005 | Fixes CRATE-06 |
| R-007 | Architecture | Restructure `SessionConfig` into nested sub-configs | P1 | Config builds/serialises; front-ends unchanged externally | R-001 | Fixes CRATE-03 |
| R-008 | Architecture | Introduce nested `SessionEvent` hierarchy | P1 | Observers compile and match only needed branches | — | Fixes CRATE-05 |
| R-009 | Correctness | Fix 0-based finding numbers in `synthesis.rs` | P0 | Synthesis critic messages use 1-based numbers; regression test added | — | Fixes FUNC-ANL-01 |
| R-010 | Correctness | Make contradiction dimensions configurable via `ContradictionConfig` / `PolarityDimension` | P0 | Default keeps current dimensions; callers can override; tests for non-medical topics | — | Fixes FUNC-ANL-02 |
| R-011 | Correctness | Fix reconcile conflicting-edge count to use shared source indices | P0 | Conflicting edges counted correctly; regression test added | R-010 | Fixes FUNC-ANL-03 |
| R-012 | Performance | Cache compiled regexes with `OnceLock` in `cite_checker`, `verify`, `synthesis`, `diagram` | P1 | No `Regex::new` in hot paths; benchmarks or tests confirm | — | Fixes FUNC-ANL-04 |
| R-013 | Correctness | Invert `CorpusCriticReport.balance_score` or rename to `dominance_score` | P1 | Semantics match field name; tests updated | — | Fixes FUNC-ANL-05 |
| R-014 | Maintainability | Move shared polarity helpers (`source_body_text`, `depth_from_count`) into internal module | P1 | ~100 lines of duplication removed; tests pass | R-010 | Fixes FUNC-ANL-06 |
| R-015 | Cross-Interface | Implement or remove CLI `ResearchCliCommand::Continue` stub | P1 | Either `continue` works end-to-end or subcommand is removed | — | Fixes XINT-07 |
| R-016 | HTTP | Add SSE or 202-accepted background research runs | P1 | `POST /research` returns immediately; `GET /research/{name}/events` streams progress | R-004 | Fixes XINT-04 |
| R-017 | HTTP | Split event JSON helper from CLI line renderer | P1 | HTTP no longer strips prefix manually; TUI can use raw JSON | R-004 | Fixes XINT-10 |
| R-018 | HTTP | Extend `ResearchItemRow` with `topic`, `queries`, `output_format`, `model` | P2 | Response includes fields; tests updated | — | Fixes XINT-11 |
| R-019 | Performance | Pre-size buffers and replace `format!` with `write!` in `document.rs` renderers | P1 | Large-report assembly shows improvement; no output change | — | Fixes PERF-DOC-01, PERF-DOC-04, PERF-DOC-07, PERF-DOC-09 |
| R-020 | Performance | Add single-pass `preview_body` helper in `web_gatherer.rs` | P1 | Both vault and fetch paths use helper; tests pass | — | Fixes PERF-WEB-02, PERF-WEB-04 |
| R-021 | Performance | Pre-lowercase terms and use efficient multi-term matching in `local_gatherer.rs` | P1 | No `O(matches×terms)` loops; tests pass | — | Fixes PERF-LOC-01 |
| R-022 | Performance | Sort/take local candidates before reading bodies | P1 | Only top-N candidate bodies read; I/O benchmark improved | — | Fixes PERF-LOC-06 |
| R-023 | Performance | Batch SQLite operations and cache parsed items in `manager.rs` | P1 | Fewer `spawn_blocking` calls; status checks avoid redundant disk I/O | — | Fixes PERF-MGR-01, PERF-MGR-02 |
| R-024 | Performance | Avoid full-corpus clones in `synthesize` | P1 | `Arc<[Source]>` or `&[Source]` used; memory benchmark improved | — | Fixes PERF-ENG-01, CRATE-07 |
| R-025 | Correctness | Improve `KeywordVerifier` with stop-word filter and multi-word overlap | P2 | Tests for false positives/negatives added | — | Fixes FUNC-ANL-07 |
| R-026 | Correctness | Improve `renumber_findings` for bold-labelled findings | P2 | Merged findings have contiguous numbers; tests added | — | Fixes FUNC-ANL-08 |
| R-027 | Maintainability | Convert `AnalysisEngine` from trait marker to enum or remove marker | P2 | No `is_noop_marker()` in public API; tests pass | — | Fixes FUNC-ANL-09 |
| R-028 | Maintainability | Migrate inline tests to `crates/ragent-research/tests/` | P1 | No inline `#[cfg(test)]` modules in library source | — | Fixes CRATE-02, FUNC-ANL-15 |
| R-029 | Build | Add `rerun-if-changed` directives to `build.rs` | P2 | Unnecessary rebuilds reduced | — | Fixes CRATE-09 |
| R-030 | Build | Resolve duplicate `tokio` dependency in `Cargo.toml` | P2 | Single feature set used; tests pass | — | Fixes CRATE-10 |
| R-031 | Documentation | Update stale module docs in `lib.rs` | P3 | Comments match implemented modules | — | Fixes CRATE-11 |
| R-032 | Documentation | Update `SPEC.md`, `QUICKSTART.md`, `CHANGELOG.md` for unified model and HTTP changes | P2 | Docs reflect new endpoints and request schema | R-004, R-016 | Required before rollout |

---

## 7. Risks and Dependencies

### Risks

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| Refactoring `SessionConfig` breaks serde snapshots or saved sessions | Medium | High | Keep serde field names stable; add migration tests; consider deserialising old configs. |
| Nested `SessionEvent` change requires updates in TUI and server observers | High | Medium | Refactor one front-end at a time; keep old enum variants as deprecated aliases during transition. |
| Performance changes alter `RESEARCH.md` formatting whitespace | Medium | Low | Add golden-file tests before changes; compare canonical normalised output. |
| Configurable polarity dimensions change default research reports | Low | Medium | Default remains unchanged; add tests for custom dimensions. |
| HTTP background-run mode complicates error propagation | Medium | Medium | Return initial validation errors synchronously; stream stage errors via SSE. |
| Batch SQLite changes introduce transaction concurrency bugs | Low | High | Keep writes behind a single `spawn_blocking` task/channel; add concurrency tests. |

### Dependencies

- `ragent_config::Config` must remain available for `build_session_config`.
- `ragent-server` and `ragent-tui` must be updated in lock-step with the
  `ragent-research` request abstraction.
- Regex caching requires `regex` crate already present (confirmed in
  `Cargo.toml`).
- SSE / background HTTP runs depend on existing `ResearchManager` state
  visibility (`GET /research/{name}`).

### External Blockers

None identified.

### Suggested Ordering

1. Start with **R-001** (shared request) because it unblocks the cross-interface
   work and reduces the blast radius of later changes.
2. Follow with **R-002 / R-003 / R-004** to converge the front-ends.
3. Do **R-005 / R-006 / R-007 / R-008** in Phase 2 before large analysis
   changes, so the new structure stabilises.
4. Tackle correctness fixes (**R-009 / R-010 / R-011**) and performance
   micro-optimisations in parallel where they touch different files.
5. Save HTTP background-run (**R-016**) and documentation updates
   (**R-032**) for Phase 3/4 after the core model is solid.

---

*End of plan.*
