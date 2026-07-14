# Changelog

## Unreleased

## Version: 0.1.0-alpha.145

### Added — Router Provider TUI Setup (spec: routeui)

- **Interactive Model Router configuration panel** reachable via `/provider` → `Model Router`
  or `/provider router`. Users can select multiple already-configured concrete providers,
  assign provider/model pairs to the four routing tiers (`SIMPLE`, `MEDIUM`, `COMPLEX`,
  `REASONING`), and save the cluster to `ragent.json`.
- **Router setup state machine** added to `ProviderSetupStep` with `SetupRouter` and
  `SelectRouterModel` variants, reusing the existing provider-setup overlay.
- **Two-pane router UI** — provider multi-selection list on the left, four bucket columns
  on the right, with keyboard navigation (Tab, arrows, Space, Enter, Delete, Ctrl+S, Esc).
- **Model picker dialog** for choosing which model from a selected provider is assigned
  to the active tier bucket.
- **Persistence and validation** — `Ctrl+S` saves `provider.router.tiers` to `ragent.json`,
  enables the router, preserves existing classifier weights/boundaries, rejects recursive
  router-to-router assignments, and requires at least one non-empty tier.
- **`/provider show` now renders the router cluster** with each tier and its assigned
  provider/model entries.
- **Status bar label** displays `"Model Router"` when the router virtual provider is active.
- **Spec and tests** — added `specs/routeui/SPEC.md`/`PLAN.md` and
  `crates/ragent-tui/tests/test_router_setup.rs` covering the provider helper, state
  defaults, report rendering, and picker list invariants.

### Changed

- **Workspace version** — Bumped to `0.1.0-alpha.145`.
- **`ragent-llm` Provider trait** now requires `as_any_static()` so callers can
  downcast concrete providers (e.g. to inspect the router's enabled state).

## Version: 0.1.0-alpha.144

### Changed

- **Workspace version** — Bumped to `0.1.0-alpha.144`.

## Version: 0.1.0-alpha.143

### Fixed — Scroll optimizations and edit tool follow-up

- **Workspace version bumped** to `0.1.0-alpha.143`.
- **Scroll optimizations** — additional TUI scrolling fixes and improvements.
- **Edit/MultiEdit reliability follow-up** — continued hardening of the tolerant matcher and batch normalization fallback introduced in `0.1.0-alpha.142`.

## Version: 0.1.0-alpha.142

### Fixed — Edit/MultiEdit tool matcher reliability

- **Single-file `edit` now uses the tolerant seven-pass matcher.** It accepts common
  whitespace and line-ending differences (CRLF vs LF, trailing/leading spaces,
  indentation drift) while still requiring a unique match. This exceeds the strict
  byte-for-byte behavior of Claude Code's `Edit` tool, which is known to fail
  frequently on real-world files.
- **`multi_edit` batch normalization fallback.** Each batch edit tries strict exact
  match first; if that fails with `NotFound`, a controlled fallback strips CRLF and
  trailing whitespace per line and retries. Indentation and internal whitespace
  are preserved so batch edits remain deterministic.
- **Improved match-failure diagnostics.** Error messages now report the last
  attempted tolerance pass (e.g. `trailing-ws`, `batch-normalized`) and the closest
  near-match line when one can be identified, giving the model concrete guidance.
- **Dry-run mode for `edit` and `multi_edit`.** Pass `"dry_run": true` to resolve
  matches and preview snippets without writing any files.
- **Updated agent instructions and `AGENTS.md`** to describe the tolerant matching,
  recommend context blocks and `dry_run`, and document the `write`-fallback pattern.
- **Amended `specs/editrenewal/SPEC.md`** (FR-004, FR-009, FR-011) to reflect the
  new matcher behavior.
- **Files changed:** `crates/ragent-tools-core/src/edit.rs`,
  `crates/ragent-tools-core/src/multiedit.rs`,
  `crates/ragent-tools-core/src/replace.rs`,
  `crates/ragent-tools-core/tests/test_edit_integration.rs`,
  `crates/ragent-tools-core/tests/test_multiedit.rs`,
  `crates/ragent-tools-core/tests/test_multiedit_helpers.rs`,
  `crates/ragent-agent/src/agent/mod.rs`,
  `assets/config/AGENTS.md`, `specs/editrenewal/SPEC.md`.

## Version: 0.1.0-alpha.141

### Added — Research findings now render with a headline

- **Finding headings combine number and a short headline.** `RESEARCH.md` findings
  now render as `### Finding N — <headline>` instead of a bare `### Finding N`.
- **Headline comes from a new `**Headline:**` paragraph.** The synthesis prompt now
  asks the LLM to start each finding with a `**Headline:**` paragraph of at most
  15 words summarising the observation. If the LLM omits the headline, the
  assembler falls back to the first 15 words of the observation.
- **Backward compatibility.** Old findings without a `**Headline:**` paragraph
  still produce a sensible heading from the observation text.
- **Files changed:** `crates/ragent-research/src/analysis.rs`,
  `crates/ragent-research/src/document.rs`,
  `crates/ragent-research/src/session.rs`, plus updated tests in
  `crates/ragent-research/src/document.rs`,
  `crates/ragent-research/src/analysis.rs`,
  `crates/ragent-research/src/session.rs`,
  `crates/ragent-research/tests/test_research_create_synthesis.rs`,
  `crates/ragent-research/tests/test_template_merge.rs`.

## Version: 0.1.0-alpha.140

### Added — `/research open` markdown viewer panel

- **TUI panel renders RESEARCH.md content.** Running `/research open <name>`
  now opens a full-screen overlay that reads the item's `RESEARCH.md` (minus
  YAML frontmatter), strips control characters, and renders the markdown body
  directly in the TUI instead of only printing metadata to the chat log.
- **Mermaid diagram support.** Fenced ` ```mermaid ` blocks are detected and
  rendered with a header label (`[Mermaid diagram — rendered as text below]`)
  so the user knows the diagram source is present even though the terminal
  cannot draw vector graphics.
- **Image placeholders.** Markdown image syntax (`![alt](src)`) is converted
  into a colored placeholder such as `[Image: alt (100x50)]`. Local PNG/JPEG
  files are inspected for dimensions without decoding pixels; remote images
  and missing files still show the alt text and path/URL.
- **Link rendering.** Inline links are shown as `[text](url)` with the URL
  styled underlined/cyan. The panel footer notes that terminal links are plain
  text (browsers/terminals with OSC-8 are not yet supported).
- **Navigation.** The panel supports scrolling with `PageUp` / `PageDown`
  and jumps to start/end with `Ctrl+PageUp` / `Ctrl+PageDown`. `Esc` closes
  the viewer.
- **Mouse support.** The panel responds to mouse-wheel scroll and closes when
  clicking outside its bounds.
- **Files changed:** `crates/ragent-tui/src/app/{state,init,event_handler,
  input_handler,research,helpers}.rs`, `crates/ragent-tui/src/{input,layout}.rs`,
  `crates/ragent-tui/src/app.rs`, `crates/ragent-types/src/event/mod.rs`,
  `crates/ragent-server/src/sse.rs`.
- **New tests** — `crates/ragent-tui/tests/test_research_viewer.rs` (18 tests)
  covering headings, code blocks, mermaid labels, image placeholders, link
  rendering, bullet lists, the footer note, `Esc`/click close, keyboard
  scrolling, and mouse-wheel scrolling.

### Added — `/research` parallel web capture

### Fixed — `/research` TUI screen corruption

- **Sanitize external strings before display.** URL, page title, error, and
  `--from-url` body-preview text are now passed through `sanitize_for_display`
  in `crates/ragent-tui/src/research_progress.rs` and
  `crates/ragent-tui/src/app/event_handler.rs`. This strips ANSI escape
  sequences and control characters (`\x00`–`\x1F` except `\n` and `\t`) that
  could leak into the TUI from fetched page content or HTTP errors and appear
  as garbage glyphs (e.g. `%???`) on the left side of the research progress
  panel.
- **Bypass lossy markdown→HTML→text pipeline for research progress.** Messages
  starting with `🔬 Research Progress` are now rendered as plain text in
  `crates/ragent-tui/src/app/models.rs` instead of being converted by
  `pulldown_cmark` + `html2text`. This prevents the converter from replacing
  valid Unicode icons and line indentation with artifacts and keeps the
  pre-formatted log list intact.
- **New tests** — `test_research_progress_sanitize.rs` (7 tests) covering ANSI
  stripping, control-char removal, newline/tab preservation, sanitized
  `WebCaptured`/`WebFetchFailed` encoding, and the research-progress
  plain-text bypass.

### Added — `/research` parallel web capture

- **Concurrent page fetching in `/research create`.** The web-gathering
  fetch phase in `crates/ragent-research/src/web_gatherer.rs` now issues
  candidate page fetches concurrently via `futures::stream::buffer_unordered`
  instead of sequentially `await`-ing each URL in turn. The default
  concurrency limit is **10** (`DEFAULT_FETCH_CONCURRENCY`), configurable per
  run with the new `--fetch-concurrently N` CLI flag (and the matching
  `fetch_concurrency` field on `SessionConfig`, the `fetch_concurrency` JSON
  body field on `POST /research`, and the
  `WebGatherer::with_fetch_concurrency` builder).
- **Ordering preserved.** `SourceCaptured` / `FetchFailed` observer events
  still fire in fetch-completion order (so the TUI renders pages as they
  arrive), but the returned `sources` vector is re-sorted into the original
  search-ranking order so `web-NN.md` supporting-file names keep tracking
  hit position rather than completion timing.
- **New tests** — `gather_fetches_pages_concurrently_up_to_fetch_concurrency`
  (proves the high-water mark of in-flight fetches matches
  `fetch_concurrency`), `with_fetch_concurrency_clamps_zero_to_one`, and
  `default_fetch_concurrency_is_ten` in
  `crates/ragent-research/src/web_gatherer.rs`; plus CLI parse tests for
  `--fetch-concurrently` in `crates/ragent-research/src/cli.rs`.

### Fixed — `/research --from-url` topic derivation

- **Prefer cleaned page titles, fall back to cleaned body.** When
  `/research create --from-url <URL>` is used, the research topic is now
  derived from the extracted page title first. Site-brand tokens (e.g.
  `InfoQ`), common nav words (`Homepage`, `Articles`), title-tag separators
  (`|`, `-`, `/`), and glued-together words (`HomepageArticlesLarge`) are
  stripped, so titles like `InfoQ HomepageArticlesLarge Concept Models: a
  Paradigm Shift in AI Reasoning` become `Large Concept Models: a Paradigm
  Shift in AI Reasoning`. If the title is missing or too generic, the first
  substantive sentence of the cleaned page body is used. This fixes sparse
  or concatenated topics without falling back to a bare URL.
- **New unit tests** — `clean_site_title_strips_site_brand_and_nav_prefixes`,
  `derive_topic_prefers_cleaned_title_over_body`,
  `derive_topic_falls_back_to_body_when_title_is_generic`, and
  `split_glued_words_splits_camel_case_and_acronyms` in
  `crates/ragent-research/src/session.rs`.

## Version: 0.1.0-alpha.139

### Changed

- **Workspace version** — Bumped to `0.1.0-alpha.139`. Updates the `/research`
  subsystem: analysis prompts are rebuilt on the evidence-based
  `research/researchanalysis` guidance, the `--from-url` seed now uses the
  `readability-rs` crate for HTML extraction, and a new `FromUrlNoUsableBody`
  outcome prevents unrelated topics from nav links.

### Added — `/research` improvements

- **`--from-url` uses `readability-rs` for HTML extraction.** The custom
  ARC90-style readability module in `crates/ragent-research/src/readability.rs`
  has been removed. The `webfetch` tool now runs `readability::extract` on HTML
  responses; if the crate cannot produce a substantive article body, it falls
  back to `html2text`. The extracted page title is still captured in source
  metadata, but it is **never** used as the research topic.
- **Improved `/research create` synthesis prompt.** The analysis prompt (in
  `crates/ragent-research/src/analysis.rs`) now applies the evidence-based
  prompt-engineering guidance from `research/researchanalysis` (20 synthesized
  findings from 91 web sources): a versioned, composable
  `SynthesisPromptBuilder` replaces the monolithic `build_synthesis_prompt`;
  a mandatory `Sources Cited / Date Spread` paragraph; recency-weighting
  rules; deterministic mechanical fallback via `analyze_with_outcome` /
  `AnalysisOutcome`; and `--template` merge clarification in `document.rs`.
- **New tests** — `crates/ragent-research/tests/test_research_create_synthesis.rs`
  and `crates/ragent-research/tests/test_template_merge.rs`.

### Fixed

- **Research `--from-url` no longer derives unrelated topics from nav links.**
  The topic is now derived only from the fetched page body. If the body
  contains no usable article text, the session stops and reports
  `FromUrlNoUsableBody` instead of falling back to the page title or URL.
  This prevents cases like an OpenAI deep-research URL producing
  Foundation-framework queries.

### Fixed — `/research` TUI progress widget

- **Each `/research create` run now gets its own progress widget.** Previously
  the TUI held a single `Option<ResearchProgress>` tracker, so starting a new
  research run overwrote the progress log of any earlier run, making it
  impossible to see the results of older requests. The state field is now a
  `Vec<ResearchProgress>` and each run is matched by name, so every run keeps
  its own self-updating `🔬 Research Progress — \`<name>\`` message in the
  window and older runs stay visible alongside the latest one.
- **`--from-url` body preview in the progress widget.** When
  `/research create --from-url <URL>` is used, the progress widget now shows
  the first ~200 characters of the extracted article body (via a new
  `SessionEvent::FromUrlBodyPreview` event emitted right after the primary
  fetch succeeds), so you can see exactly what content the topic was derived
  from.
- **Decomposed queries render as soon as they are generated.** The
  `GatherEvent::QueriesDecomposed` event is now forwarded immediately by
  `GatherEventForwarder` (previously it was dropped and the session re-emitted
  `QueriesDecomposed` only after the whole web gather returned). The
  duplicated post-gather emission has been removed. Sub-queries now appear in
  the progress widget before the parallel searches complete.
- **Successfully retrieved URLs render inline during the gather.** A new
  `GatherEvent::SourceCaptured { url, title }` event is emitted by
  `WebGatherer` each time a page fetch succeeds, and is forwarded as
  `SessionEvent::WebCaptured` so the progress widget shows each captured URL
  as it arrives. Previously only `WebFetchFailed` events were surfaced during
  the gather; successful captures were only shown in a batch at the end.

## Version: 0.1.0-alpha.138

### Changed

- **Workspace version** — Bumped to `0.1.0-alpha.138`. Continued code
  duplication removal tracked in `DUPPLAN.md`, following the
  `0.1.0-alpha.137` milestone which reduced `cargo dupes` exact-duplicate
  groups from 385 → 340 and exact-dup lines from 15,573 → 11,775 across
  milestones A–K.

## Version: 0.1.0-alpha.137

### Removed — Code duplication (DUPPLAN.md Milestones A–K)

**Summary:** Across 11 milestones (A–K), the `cargo dupes` duplication
metrics improved as follows:

| Metric | Before | After | Improvement |
|--------|--------|-------|-------------|
| Exact-duplicate groups | 385 | 340 | −45 (−11.7%) |
| Exact-dup lines | 15,573 | 11,775 | −3,798 (−24.4%) |
| Near-duplicate groups | 142 | 144 | +2 (new minor groups) |
| Exact duplication % | 10.9% | 8.4% | −2.5 pp |
| Total lines analysed | 142,348 | ~139,700 | ~2,600 removed |

**Milestones completed:**

| Milestone | Description | Lines removed | Groups eliminated |
|-----------|-------------|---------------|-------------------|
| A | Dead-code VCS tool copies | 2,461 | 50, 51, 53, 102 |
| B | `resolve_path` extraction | ~136 | 1 |
| C | Parser boilerplate (`build_qname`, `create_parser`, `parse_tree`) | ~190 | 8, 10, 16 |
| D | `not_available` codeindex fallback | ~50 | 22 |
| E | `resource.rs` triple-copy | ~60 | 32 |
| F | `strip_tags` unification | ~15 | 121 (near) |
| G | TUI `make_app` test helper | ~950 | 2, 34 |
| H | `MockStorage` test helper | ~90 | 49, 60, 65, 67 |
| I | `setup` / `setup_workspace` helpers | ~40 | 33, 37 |
| J | Accepted duplications documented | 0 (comments) | — |
| K | Verification & regression baseline | 0 | — |
| **Total** | | **~4,000** | **~20 groups** |

See `DUPPLAN.md` for the full plan and per-milestone details.
Pre-refactor baseline: `docs/reports/dupes-baseline.txt`.
Post-refactor report: `docs/reports/dupes-final.txt`.

### Removed — Code duplication (DUPPLAN.md Milestones A & B)

- **Milestone A — Dead-code VCS tool copies removed (2,461 lines).** The
  `ragent-agent` crate held five full verbatim copies of GitHub/GitLab tool
  implementations that already live canonically in `ragent-tools-vcs` and are
  registered via the `ExtractedVcsToolAdapter`. The local copies were never
  referenced. Deleted `github_issues.rs`, `github_prs.rs`, `gitlab_issues.rs`,
  `gitlab_mrs.rs`, and `gitlab_pipelines.rs` from
  `crates/ragent-agent/src/tool/` and removed their `pub mod` declarations from
  `tool/mod.rs`. Added `scripts/check-vcs-duplication.sh` CI guard (wired into
  `pre-flight.sh`) to prevent regressions. Eliminates `cargo dupes` exact-dup
  groups 50, 51, 53 and near-dup group 102 (cross-crate). Stats: 385 → 355
  exact groups, 15,573 → 13,347 exact-dup lines.

- **Milestone B — `resolve_path` extraction (18 → 1 copy, ~136 lines).** The
  identical 8-line `resolve_path` helper was copy-pasted across 16 files in
  `ragent-tools-core/src/` and 2 in `ragent-tools-extended/src/` (the largest
  single exact-duplicate group in the codebase). Extracted the canonical
  implementation into `crates/ragent-tools-core/src/path_util.rs` and replaced
  all 16 core copies with `use super::path_util::resolve_path;`. Replaced the
  2 extended copies (`libreoffice_common.rs`, `office_common.rs`) with
  `pub use ragent_tools_core::path_util::resolve_path;` re-exports. Fixed the
  two `#[path]`-based test files (`test_edit.rs`, `test_multiedit_helpers.rs`)
  to add a `path_util` shim module. `cargo fix` cleaned up now-unused
  `Path`/`PathBuf` imports. Eliminates `cargo dupes` exact-dup group 1. Stats:
  355 → 354 exact groups, 13,347 → 13,203 exact-dup lines.

- **Milestone E — `resource.rs` triple-copy (3 → 1 copy, ~60 lines).** The
  process/tool concurrency semaphore module was duplicated three times:
  `ragent-types/src/resource.rs` (canonical, with tests),
  `ragent-agent/src/resource.rs` (near-identical, only added a `#[cfg(test)]`
  block), and `ragent-tools-core/src/lib.rs` (inline `pub mod resource` with a
  subset). Deleted `ragent-agent/src/resource.rs` and replaced `pub mod
  resource;` in `lib.rs` with `pub use ragent_types::resource;`. Deleted the
  inline `pub mod resource { ... }` block in `ragent-tools-core/src/lib.rs` and
  replaced it with `pub use ragent_types::resource;`. Both crates already
  depended on `ragent-types`. All call sites (`processor.rs:1014`,
  `context.rs:235`, `bash.rs:1051`) continue to resolve via the re-exports.
  The tests that were in `ragent-agent/src/resource.rs` are already covered by
  `ragent-types/tests/test_resource.rs`. Eliminates `cargo dupes` exact-dup
  group 32. Stats: 349 → 347 exact groups, 12,947 → 12,866 exact-dup lines.

- **Milestone F — `strip_tags` unification (2 → 1 copy).** The `strip_tags`
  HTML-tag-stripping helper was duplicated in `ragent-tools-extended/src/
  webfetch.rs` and `ragent-research/src/web_date.rs` (near-dup group 121). The
  two variants differed in behaviour: `web_date.rs` pushed a space on `<` to
  prevent words merging across tag boundaries (e.g. `"foo<b>bar"` → `"foo
  bar"`), while `webfetch.rs` did not (producing `"foobar"`). Adopted the
  space-pushing variant as the canonical implementation in a new
  `ragent-types/src/html.rs` module (both crates depend on `ragent-types` but
  `ragent-research` does not depend on `ragent-tools-extended`). Replaced the
  `webfetch.rs` definition with `pub use ragent_types::html::strip_tags;` and
  the `web_date.rs` definition with `use ragent_types::html::strip_tags;`. The
  `webfetch.rs` internal `extract_text` helper now calls the imported
  `strip_tags`, inheriting the improved space-on-`<` behaviour. Eliminates
  `cargo dupes` near-dup group 121.

- **Milestone G — TUI `make_app` test helper (27 → 1 shared + ~10 variant
  copies, ~900 lines removed).** The `make_app()` function — a ~45-line `App`
  constructor wiring up `EventBus`, `Storage::open_in_memory()`,
  `SessionProcessor`, and `App::new(...)` — was copy-pasted across 27 files
  (24 test files + 3 bench files; `cargo dupes` groups 2 and 34, the
  second-largest duplication in the codebase). Extracted the canonical
  `pub fn make_app() -> App` into `crates/ragent-tui/tests/support/mod.rs`.
  Replaced the standard copy in 18 test files and 3 bench files with
  `#[path = "support/mod.rs"] mod support;` + `support::make_app()` calls.
  Left ~9 files with variant signatures or flags (`make_app(event_bus)`,
  `make_app_with_storage(storage)`, `make_app_with_manager()`, and files
  passing `true` as the debug flag to `App::new`) as local definitions — these
  have legitimately different behaviour and cannot use the shared helper.
  `cargo fix` cleaned up now-unused imports across all modified files.
  Eliminates `cargo dupes` exact-dup groups 2 and 34. Stats: 347 → 341 exact
  groups, 12,866 → ~11,900 exact-dup lines.

- **Milestone H — `MockStorage` / `DemoStorage` test helpers (4 → 1 shared +
  1 documented example, ~90 lines removed).** An in-memory `StorageBackend`
  mock was duplicated verbatim across 4 files in `ragent-tools-extended/`
  (`cargo dupes` groups 49, 60, 65, 67 — each group was one trait method:
  `get_todos`, `create_todo`, `update_todo`, `clear_todos`). Extracted the
  canonical `MockStorage` struct + its `StorageBackend` impl into
  `crates/ragent-tools-extended/tests/support/mock_storage.rs`. Replaced the
  local `MockStorage` definitions in the 3 test files (`test_todo_demo.rs`,
  `test_todo_lifecycle.rs`, `test_todo_status_change.rs`) with
  `#[path = "support/mock_storage.rs"] mod mock_storage;` +
  `use mock_storage::MockStorage;`. Left `DemoStorage` in
  `examples/todo_cycle.rs` as a documented example variant (with a comment
  pointing to the shared module) so the example remains self-contained.
  `cargo fix` cleaned up now-unused imports. Eliminates `cargo dupes`
  exact-dup groups 49, 60, 65, 67.

- **Milestone I — `setup` / `setup_workspace` test helpers (10 → 2 shared +
  0 local, ~40 lines removed).** Two temp-directory setup helpers were
  duplicated across 10 test modules. `setup_workspace() -> (TempDir,
  PathBuf)` (5 copies in `ragent-team/tests/`, `cargo dupes` group 37) was
  extracted into `crates/ragent-team/tests/support/mod.rs` and included via
  `#[path = "support/mod.rs"] mod support;` in all 5 test files.
  `setup() -> TempDir` (5 copies across `ragent-agent/src/memory/` and
  `ragent-tools-extended/src/memory/` inline `#[cfg(test)]` modules, group 33)
  was extracted into `memory/test_helpers.rs` in each crate with
  `#[cfg(test)] mod test_helpers;` in `memory/mod.rs`. Each test module's
  local `fn setup` was replaced with `use super::test_helpers::setup_temp_dir;`
  and `setup()` calls updated to `setup_temp_dir()`. Eliminates `cargo dupes`
  exact-dup groups 33 and 37.

- **Milestone J — Accepted duplications documented (comment-only, 0 lines
  changed).** Added `// NOTE: intentional duplication — see DUPPLAN.md
  Milestone J` comments above the Tier-3 accepted duplicate groups so future
  readers don't attempt to "fix" them. Comments added to: `mock_llm_client.rs`
  (group 30, `as_str`), `store.rs` (group 40, `IndexStore` accessors),
  `read.rs` (group 80, `detect_python_sections`/`detect_go_sections`),
  `anthropic.rs` (group 28, streaming closures), `session.rs` (group 44,
  `LocalTool::grep` no-op impls), `bigcodebench.rs` (group 47,
  `BenchSuiteAdapter::build_prompt`), `gradle.rs` (group 58,
  `LanguageParser::parse`), and `knowledge_graph.rs` (group 13, `From` impls).
  No behaviour change; `cargo dupes` numbers unchanged.

- **Milestone D — `not_available` codeindex fallback (6 → 1 copy, ~50
  lines).** Six codeindex tool files in `ragent-tools-extended/src/` each
  defined a structurally-identical `fn not_available() -> ToolOutput` fallback
  for the "code index disabled" message (the messages and `fallback_tools`
  metadata differed per-tool). Extracted a parameterised
  `pub(crate) fn codeindex_not_available(fallback_hint: &str, fallback_tools: &[&str])`
  into the existing `codeindex_utils.rs` module and replaced all 6 local
  definitions with one-line calls. `codeindex_status.rs` was unified to use the
  same `fallback_tools` metadata shape (its redundant `"enabled": false` field
  was dropped since `"error": "codeindex_disabled"` already signals the state).
  `cargo fix` cleaned up now-unused `json!` imports. Eliminates `cargo dupes`
  exact-dup group 22. Stats: 350 → 349 exact groups, 13,013 → 12,971 exact-dup
  lines.

- **Milestone C — Code-index parser boilerplate (3 groups eliminated, ~190
  lines).** The tree-sitter parser subsystem in `ragent-codeindex/src/parser/`
  repeated three boilerplate patterns across 7–10 language files. Extracted
  `pub fn build_qname(scope, name, sep)` into a new `parser/util.rs` and
  replaced all 10 local copies (`build_qname` / `build_qualified` /
  `build_qualified_name`) with thin `#[inline]` delegating wrappers that pass
  the per-language separator (`"::"`, `"."`, or `":"`). Defined a
  `tree_sitter_parser!` declarative macro in `parser/util.rs` that generates
  the uniform `create_parser()` + `parse_tree()` pair, and applied it to all 9
  language parsers that use the standard pattern (gradle, cmake, go,
  gradle_kts, hcl, openscad, python, maven, rust). The `go.rs` and `python.rs`
  `LanguageParser::parse` impls were updated to call `Self::parse_tree(source)`
  (matching the other 7 files) so the macro-generated `parse_tree` is used.
  `cargo fix` cleaned up now-unused `Parser`/`Tree` imports. Excluded
  `typescript.rs` (variant `create_parser(&self)` with match-on-variant) and
  `c_cpp.rs` (inline parse, no separate methods) per the plan. Eliminates
  `cargo dupes` exact-dup groups 8, 10, and 16. Stats: 354 → 350 exact groups,
  13,203 → 13,013 exact-dup lines.

### Added

- **`/init config` slash command** — New subcommand of `/init` that creates a
  default `ragent.json` file in the global config directory
  (`~/.config/ragent/ragent.json` on Linux,
  `~/Library/Application Support/ragent/ragent.json` on macOS,
  `%APPDATA%\ragent\ragent.json` on Windows). If a global config already
  exists, the command reports its path and makes no changes. The default config
  is serialised from `Config::default()` and contains all default settings ready
  to edit. Autocomplete suggestions and parameter hints updated for `/init`.

### Changed

- **Workspace version** — Bumped to `0.1.0-alpha.137`. Follow-up to
  `0.1.0-alpha.136`, which added web source publication dates to the
  `/research` slash command (`RESEARCH.md` References Index **Published**
  column, per-finding `**Source date range:**`, and the
  `ragent_research::extract_published_at` helper).

## Version: 0.1.0-alpha.136

### Added

- **Research source publication dates** — The `/research` slash command now
  captures the publication date of each web source and surfaces it in the
  `RESEARCH.md` output and references. A new `**Source date range:**` line
  under each finding summarises the earliest–latest publication dates of its
  cited web sources, so the relative age of the evidence is visible at a
  glance. Dates are parsed best-effort from JSON-LD `datePublished`, article
  meta tags (`article:published_time`, `pubdate`, `dc.date`, etc.), `<time>`
  elements, and a visible-text fallback; any failure leaves the date as `—`
  without aborting the research run. The References Index table gained a
  **Published** column, supporting files show `Published (UTC)`, and the new
  `ragent_research::extract_published_at` helper is re-exported for
  `ragent-agent`'s best-effort raw-HTML fetch. Older `RESEARCH.md` files
  remain loadable via `#[serde(default)]` on the new optional field.

### Fixed

- **Workspace version** — Bumped to `0.1.0-alpha.136`.

## Version: 0.1.0-alpha.135

### Fixed

- **Workspace version** — Bumped to `0.1.0-alpha.135`.
- **fix ci** — Resolved GitHub Actions "Check and Test" failure caused by the `0.1.0-alpha.132` scrollbar drag math regression in Memory/TODO panels. Reverted the `top_based` inversion in `apply_scrollbar_drag()` and updated the Memory panel tests to use the bottom-based offset convention consistent with Messages/Log/Profile.

## Version: 0.1.0-alpha.134

### Fixed

- **Scrollbar drag math regression** — Reverted the `top_based` inversion introduced for Memory/TODO panels in `0.1.0-alpha.132` and updated the Memory panel tests to match the bottom-based offset convention used by the rest of the TUI.

## Version: 0.1.0-alpha.133

### Changed

- **Workspace version** — Bumped to `0.1.0-alpha.133`.
- **fix tests that depend on untracked files** — Pointed `ragent-specs` real-project integration tests at the self-contained fixture under `crates/ragent-specs/tests/fixtures/testspec` so they no longer rely on the untracked `specs/` directory.

## Version: 0.1.0-alpha.132

### Changed

- **Workspace version** — Bumped to `0.1.0-alpha.132`.
- **fix thumb scrolls** — TUI thumb/srollbar scrolling improvements.

## Version: 0.1.0-alpha.131

### Changed

- **Workspace version** — Bumped to `0.1.0-alpha.131`.
- **CI/CD updates and formatting fixes** — Applied GitHub Actions workflow
  maintenance and repository formatting improvements.

## Version: 0.1.0-alpha.130

### Changed

- **Workspace version** — Bumped to `0.1.0-alpha.130`.
- **TODO panel** — Implemented a third side panel (Alt+T) in `ragent-tui`
  that renders the session's TODO items from `ragent-storage`. The panel
  follows the existing log/profile side-panel pattern with mutual
  exclusion, text selection, scrollbar drag, and a `/todo` slash alias.
  All 12 plan tasks (T-001…T-012) and 8 acceptance criteria from the
  `todopanel` spec are satisfied.
- **Agentic-loop performance upgrade** — Implemented all six milestones
  (A–F) of `PERFPLAN.md`, covering 26 findings (P-1…P-26) plus 5
  measurement/gating tasks (F-1…F-5). Highlights:
  - Deleted inline nudge recomputation; single `set_step` call; verified
    empty-buffer stall guard (`handle_no_tool_decision`).
  - `LoopState.chat_messages` is now `Arc<Vec<ChatMessage>>` with
    `Arc::make_mut` for cheap clones; tool-definition bytes cached on
    `SessionProcessor`; one `ToolContext` per step; hoisted reusable Vecs;
    `text_buffer` moved via `mem::take`.
  - `get_messages` routed through `storage_op`; cached config keyed by
    file mtimes; `build_turn_chat_messages` returns the context window;
    `TaskManager.has_pending_background` AtomicBool skips drain scans;
    interim-save hash uses `serde_json::to_vec` bytes.
  - `ToolsSent` published only on step 1; added `Event::ToolCallBatch` +
    `ToolCallBatchEntry` and SSE forwarding; tool-result preview scan
    capped at 400 bytes.
  - Consolidated emergency-compression call sites; verified async history
    reads; short-circuit when `last_reported_input_tokens > 0`; added
    `cached_spec_section` to `SystemPromptCache` keyed by
    `(spec_id, spec.modified_at)` with `/spec activate` invalidation.
  - Added `MockLlmClient`/`MockLlmScript` in `ragent-bench`, criterion
    `agent_loop` benchmarks, baseline report, `/perf` TUI alias, and
    `scripts/check-bench-regression.sh` CI guard wired into `pre-flight.sh`.

### Fixed

- **Tool-result preview char-boundary panic** — Replaced the three manual
  200-byte preview builders in `crates/ragent-agent/src/session/processor.rs`
  (`response_preview`, `batch_content`, and `result_preview`) with
  `ragent_types::truncate_bytes`, which steps back from a byte cut point to
  the previous valid UTF-8 character boundary. This prevents the panic
  `end byte index 400 is not a char boundary; it is inside '\u{2014}'` when
  a fixed 400-byte scan landed in the middle of a multi-byte em dash.
  Added `test_truncate_bytes_em_dash_at_400_boundary` in
  `crates/ragent-types/tests/test_strutil.rs` to guard the exact scenario.

## Version: 0.1.0-alpha.129

### Changed

- **Workspace version** — Bumped to `0.1.0-alpha.129`.
- **Compression made permanent** — Removed the `compression` and
  `compression-ml` Cargo feature flags across the workspace.
  `headroom-core` is now an unconditional dependency of `ragent-agent`,
  and the context-compression pipeline is always compiled in. Specific
  changes:
    - `Cargo.toml` (workspace root): removed `compression` and
      `compression-ml` features; `default` is now empty.
    - `crates/ragent-agent/Cargo.toml`: removed `compression` and
      `compression-ml` feature definitions; `headroom-core` is no longer
      `optional`.
    - `crates/ragent-tui/Cargo.toml`: removed the `compression` feature
      passthrough.
    - `crates/ragent-agent/src/compression/mod.rs`: dropped all
      `#[cfg(feature = "compression")]` gates; `is_available()` now
      always returns `true`.
    - `crates/ragent-agent/src/lib.rs`,
      `crates/ragent-agent/src/session/{mod,history,loop_steps,processor}.rs`:
      removed every `#[cfg(feature = "compression")]` /
      `#[cfg(not(feature = "compression"))]` guard and the dead-code
      markers that existed only to silence the disabled-feature build.
    - `crates/ragent-agent/tests/test_compression_pipeline.rs` and
      `crates/ragent-agent/benches/agent_loop.rs`: removed the
      feature-gated `#[cfg(...)]` attributes on tests and benchmarks.
    - `crates/ragent-config/src/compression.rs`: updated doc comment for
      `CompressorConfig.prose` (no longer references the
      `compression-ml` feature).

## Version: 0.1.0-alpha.128

### Changed

- **Workspace version** — Bumped to `0.1.0-alpha.128`.
- **Warning remediation** — Eliminated all 279 compiler warnings across the
  workspace (build, tests, benches, and examples now compile with zero
  warnings under `--all-features`). Fixes applied:
  - Removed ~270 unused imports across `ragent-tui` app submodules (init,
    compress, bench, swarm, research, models, slash, input_handler,
    event_handler, session_ops) left over from the `app.rs` split (M5).
  - Added `///` doc comments to 62 previously-undocumented `pub` /
    `pub(crate)` methods and associated functions across `ragent-tui` app
    submodules and `ragent-agent` `session/processor.rs` to satisfy
    `-W missing-docs`.
  - Gated the `is_token_overflow_error_message` import in
    `session/loop_steps.rs` behind `#[cfg(feature = "compression")]` and
    added `#[allow(unused_variables)]` / `#[allow(unused_mut)]` on
    feature-conditional parameters in `build_turn_chat_messages`.
  - Removed unused `std::sync::Arc` and `clap::Subcommand` imports from
    `src/cli.rs`.
  - Removed unused `ragent_prompt_opt::Completer` import from
    `app/swarm.rs` and `tool::TeamManagerInterface` from
    `app/input_handler.rs`.
  - Deleted dead duplicate `#[cfg(test)]` test functions in
    `app/models.rs` and `app/session_ops.rs` (the canonical `#[test]`
    versions live in `app/tests.rs`).
  - Deleted the dead `test_app` helper in `app/helpers.rs` (the canonical
    copy lives in `app/tests.rs`) and its `#[cfg(test)]` import block.
  - Removed redundant `use super::*;` from `app/tests.rs` and the
    `router_modifiers` inline test file.

## [Unreleased] — REMPLAN.md Structural Remediation (M1–M10)

### Refactoring

Completed a 10-milestone structural remediation plan that deduplicated types,
eliminated source copies, broke dependency cycles, split overlarge files,
removed dead code, migrated inline tests, and cleaned up repository hygiene.

- **M1** — Foundation type consolidation: `Message`, `Permission*`, and LLM
  primitive types each have exactly one canonical definition. Guard test added.
- **M2** — Eliminate duplicate `Storage`: `ragent-storage` is the sole impl;
  `ragent-agent` re-exports it via a 27-line shim.
- **M3** — Break `ragent-agent`↔`ragent-team` `#[path]` cycle: Team sources
  moved into `ragent-agent`; `ragent-team` is a thin re-export shim. 27
  `#[path]` attributes eliminated.
- **M4** — Retire the `ragent_core` alias: 470+ `ragent_core::` →
  `ragent_agent::` references rewritten across 63 files.
- **M5** — Split `ragent-tui/src/app.rs`: 15,332 → 28 lines; methods
  distributed across 12+ submodules.
- **M6** — Split `session/processor.rs`: 4,503 → 2,911 lines; 4 sibling
  modules extracted; 22 inline tests moved to external test file.
- **M7** — Remove dead code & compat shims: `predictive.rs` (454 lines) and
  `message/pool.rs` (168 lines) deleted; 4 `pub mod config {}` shims
  collapsed.
- **M8** — Migrate inline tests to `tests/`: 373 tests moved to
  `tests/inline/`; CI guard script added (baseline 109).
- **M9** — Repository hygiene: Stray files and output dirs untracked;
  `docs/howtoos/` → `docs/howtos/`; `src/main.rs` split (1,223 → 905 lines).
- **M10** — Final verification & docs: All structural-defect checks pass.
  Completion report at `docs/reports/remplan-completion.md`.

## Version: 0.1.0-alpha.127

### Changed
- **Workspace version** — Bumped to `0.1.0-alpha.127`.
- **Dead-code removal** — Audited every `#[allow(dead_code)]` site across the
  workspace and removed ~579 net lines of genuinely unreachable code.
  Removed items include: `cleanup_unused_locks` and the whole
  `ragent-agent/src/tool/file_lock.rs` module (a duplicate of the
  `ragent-tools-core` file-lock); `get_attribute` (maven parser);
  `orch_metrics` HTTP handler (route never registered); the deprecated
  `render_status_bar` v1 and `render_plan_approval_dialog` (superseded by
  v2 / widget-based rendering); `GREP_PATTERNS` (predictive); seven unused
  style helpers (`style_healthy`, `style_warning`, `style_error`,
  `style_info`, `style_healthy_bold`, `style_warning_bold`,
  `style_error_bold`); the standalone `AzureFoundry::discover_models`
  method; `FailedToolCall.timestamp`; `FindDiag.pass` / `FindDiag.closest_line`;
  `ShellType::program`; and `resolve_base_url` (research analysis). Stale
  `#[allow(dead_code)]` attributes were also removed from items that are
  actually used (`SAFE_COMMANDS`, `char_wrap`, `push_log_no_agent`).
- **`/spec impl` sequential driver** — Fixed the bug where `/spec impl`
  only ran the first task. The compound single-prompt injection was
  replaced with an event-driven outer loop that dispatches one task per
  agent turn, checks the task status via `SpecManager` after each turn,
  and advances to the next task (or stops with a resumable message) on
  `Event::MessageEnd`. Added `SpecImplRunner::task_prompt`,
  `build_single_task_prompt`, `total_to_execute`, and `task_id_at` APIs
  plus a new `SpecImplState` TUI state struct.

## Version: 0.1.0-alpha.126

### Changed
- **Workspace version** — Bumped to `0.1.0-alpha.126`.
- **Compression pipeline fixes** — Addressed issues in the context
  compression pipeline (`/compress`) so it behaves correctly under the
  updated agent loop.
- **Test updates** — Refreshed and repaired tests across multiple crates
  to track API and behaviour changes from the compression work and
  related refactors.
- **Warning fixes** — Resolved compiler/clippy warnings surfaced by the
  latest tool and config changes.

## Version: 0.1.0-alpha.125

### Changed
- **Workspace version** — Bumped to `0.1.0-alpha.125`.

### Added — Edit Tool Renewal (editrenewal spec)
- **Renewed `edit` tool** — The single-file `edit` tool now aligns with Claude
  Code's `Edit` semantics. It uses strict exact-match replacement (FR-004),
  canonical parameter names `file_path`/`old_string`/`new_string` (FR-001),
  create/update/delete operations via empty `old_string`/`new_string`
  (FR-006), no-change rejection (FR-007), stale-file detection when a read
  timestamp has been recorded (FR-003), and returns a line-numbered result
  snippet with ≥4 lines of context (FR-008). Legacy parameter names
  (`path`/`old_str`/`new_str`) are still accepted and emit a
  `deprecation_warning` in the output metadata.
- **`multi_edit` tool** — The atomic batch edit tool formerly known as
  `multiedit` is now registered as `multi_edit` (FR-009). Each edit in the
  `edits` array uses `file_path`/`old_string`/`new_string` and is validated
  with the strict exact-match matcher. Overlap detection and atomic rollback
  are preserved. Stale-file detection is applied per file in the batch
  (FR-003/FR-009).
- **Legacy `multiedit` alias** — The old `multiedit` tool name remains
  registered as a deprecated alias that forwards to `multi_edit`, normalises
  legacy parameter names, and emits a `deprecation_warning` (FR-012).
- **Read-timestamp tracking** — `ToolContext` now carries a shared
  `read_timestamps` map (FR-003). The `read` tool records each file's mtime;
  `edit` and `multi_edit` consult it to reject stale-file edits. Plumbed
  through `SessionProcessor` and all tool-context construction sites.
- **Migration guide** — `docs/editrenewal-migration.md` documents the new
  tools, the strict matching semantics, and the migration path from legacy
  parameter names.

### Tests
- `crates/ragent-tools-core/tests/test_edit_integration.rs` — 17 tests
  covering exact match, strict rejection of whitespace mismatches, multiple
  matches, NotFound, create/delete operations, no-change rejection,
  stale-file detection, snippet generation, canonical vs legacy params, and
  read-then-edit integration.
- `crates/ragent-tools-core/tests/test_multiedit.rs` — 18 tests covering
  cross-file batches, overlap rejection, atomic rollback, JSON-order
  independence, strict-match acceptance/rejection, stale-file rejection in
  batches, and canonical/legacy parameter names.
- `crates/ragent-tools-core/tests/test_read_tool.rs` — 2 new tests for
  read-timestamp recording.
- `crates/ragent-agent/tests/test_editrenewal_aliases.rs` — 6 tests
  verifying the registry exposes `edit`, `multi_edit`, and the `multiedit`
  alias, and that descriptions/schemas carry the canonical parameter names
  and deprecation signalling.

## Version: 0.1.0-alpha.124

### Changed
- **Workspace version** — Bumped to `0.1.0-alpha.124`.

## Version: 0.1.0-alpha.122

### Fixed — `/help` and `/skills` slash output no longer collapses to a single paragraph
- **`/help` table preserves per-line layout in the TUI** — The `/help` slash
  command now wraps its command/skill listing in a bare fenced code block
  (` ```\n … \n``` `) so the markdown → HTML → text pipeline does not reflow
  every row into one paragraph. Each command/skill stays on its own line and
  column alignment is preserved instead of being mangled.
- **`/skills` table preserves per-line layout in the TUI** — Same fix
  applied to the `/skills` listing: the registered-skills table is wrapped in
  a bare fenced code block, and `try_extract_research_code_block` now detects
  the block via the generic `From: /<cmd>` prefix (not just `From: /research`)
  so `/skills` benefits from the same verbatim rendering path.
- **`try_extract_research_code_block` generalised to any `From: /<cmd>`
  response** — Only **bare** fences (a line containing exactly three
  backticks followed by a newline) are recognised. Responses that use
  language-tagged fences (e.g. `/tools show` emits multiple ` ```text `
  blocks) are not intercepted and continue to flow through the normal
  markdown pipeline.

### Tests
- New unit tests in `crates/ragent-tui/src/app.rs::tests`:
  - `test_try_extract_research_code_block_handles_skills_output` — extracts
    the bare fenced block from a `/skills` response and verifies the skill
    rows remain on separate lines.
  - `test_try_extract_research_code_block_handles_help_output` — same check
    for a `/help` response with both command and skills sections.
  - `test_try_extract_research_code_block_returns_none_for_non_slash_text` —
    ensures the helper still returns `None` for plain text that happens to
    contain a fenced block.
  - `test_render_markdown_to_ascii_preserves_skills_table_lines` — renders
    a `/skills` table through `render_markdown_to_ascii` and asserts the two
    skill lines are not collapsed into one sentence.
  - `test_render_markdown_to_ascii_preserves_help_command_lines` — same
    end-to-end check for `/help` output.

## Version: 0.1.0-alpha.121

### Fixed — `/research` now actually analyses the gathered sources
- **Supporting files contain the captured body, not a placeholder** — `Source::Web`,
  `Source::Local`, and `Source::Other` gained an inline `body: String` field. The
  `WebGatherer` now passes the fetched page text into `Source::Web.body`; the
  `LocalGatherer` reads each candidate file and writes a context-aware excerpt
  (matching lines plus one line on either side) into `Source::Local.body`.
  `render_supporting_file` and the synthesis engine both consume the inline body,
  so `research/<name>/sources/web-NN.md` and `local-NN.md` now contain the
  actual evidence (with `▶` markers for exact matches and a ` ` marker for
  context) instead of the legacy `(see WebGatherer for the captured body)`
  placeholder. Old `RESEARCH.md` files without the new field deserialize with
  `body == ""` thanks to `#[serde(default)]`.
- **Local source relevance note is now informative** — The previous "X keyword
  match(es) for research topic" string has been replaced by a note that names
  the matched keywords (truncated to 3, e.g. `…(+N)` for the tail) and a 120-char
  snippet of the first matching line. Driven by the new
  `LocalGatherer::build_relevance_note` and `collect_matched_terms` helpers.
- **Mechanical fallback summary/findings are useful, not skeletal** — When no
  LLM synthesis is available (CLI, or TUI without an active model, or LLM call
  failed), the default `Summary` now names the captured web titles and local
  file paths grouped by type, the default `Findings` is one bullet per source
  with a 240-char excerpt, and the default `Open Questions` suggests concrete
  gaps and re-running with a configured LLM. The Summary is also transparent
  that no LLM analysis was applied.
- **Synthesis errors are visible, not silently swallowed** — The
  `ResearchSession::run` synthesize step now matches on the outcome and emits a
  `SessionEvent::SynthesizeResult { outcome, detail }` whose outcome is one of
  `Llm`, `FallbackEmpty`, `FallbackError`, `NoLlm`. Failures are logged at
  `error` level (not `warn`) and the message bubbles through to the TUI
  progress tracker and the CLI JSON emitter. Also fixed a latent bug where
  `analysis_is_noop` always returned `false` because `Any::type_id` on a trait
  object returns the trait object's `TypeId`, not the underlying concrete
  type's — replaced with a small `is_noop_marker()` trait method that
  `NoopAnalysisEngine` overrides to `true`.
- **CLI now wires up the local gatherer** — `ragent research create` used to
  call `ResearchSession::new(manager, None, None, NoopAnalysisEngine)`,
  producing a `RESEARCH.md` with 0 sources regardless of project contents. It
  now uses the new `ragent_research::cli::FsLocalTool` (a filesystem-backed
  `LocalTool`) so the CLI produces useful output without API keys. Web search
  and LLM synthesis still require credentials and remain off in the CLI.

### Added
- **`ragent_research::cli::FsLocalTool`** — Public filesystem-backed
  implementation of `LocalTool` for CLI use. Walks the project root,
  greps line-by-line, reads files, and lists `specs/<id>` directories.
  Skips `research/`, `target/`, `.git/`, `node_modules/`, and dot-prefixed
  directories so the gatherer doesn't index its own previous outputs.
- **`ragent_research::session::SynthesizeOutcome` enum** —
  `Llm | FallbackEmpty | FallbackError | NoLlm` so callers can attribute the
  resulting `RESEARCH.md` summary to the path that produced it.
- **`SessionEvent::SynthesizeResult`** — New event emitted after the
  synthesis phase with the outcome and an optional detail string.
- **`ragent_research::analysis::AnalysisEngine::is_noop_marker()`** — Default
  trait method (`false`) overridden by `NoopAnalysisEngine` (`true`).
- **`LocalGatherer::build_relevance_note`, `build_local_excerpt`,
  `collect_matched_terms`, `MAX_LOCAL_EXCERPT_LINES`** — Public helpers used
  by both the gatherer and the synthesis fallback.
- **`Source::body()` and `Source::has_body()`** — Accessor helpers on
  `Source` for the new body field.

### Changed
- **`Source` enum** — `Source::Web`, `Source::Local`, `Source::Other` gained a
  `body: String` field with `#[serde(default)]`. All call sites (gatherers,
  tests, fixture files) updated accordingly.
- **`LocalGatherer::score_candidates`** — Now returns
  `(LocalCandidate, Vec<GrepMatch>, Vec<String>)` so the caller has the
  matched terms and per-line hits needed to build the relevance note and
  excerpt.
- **`ResearchSession::synthesize`** — Prefers the inline `Source::body`
  field over reading from the on-disk supporting file (the latter still
  works as a fallback for items loaded from older `RESEARCH.md` files).

### Tests
- **199 unit tests + 8 integration tests for `ragent-research`** (up from
    181 + 7) covering body propagation, new relevance notes, mechanical
    fallback content, `SynthesizeResult` event emission, and the new
    `FsLocalTool`.

## Version: 0.1.0-alpha.120 (unreleased)
## Version: 0.1.0-alpha.119 (unreleased)

### Added — LLM-driven synthesis for `/research create`
- **`/research create` now analyzes sources with the active LLM** — The TUI research
  session gained a `Synthesize` phase between gathering and assembly. When an
  active provider/model is configured, `ResearchSession` sends the captured
  source bodies (web pages and local excerpts) to the LLM with a structured
  prompt requesting `## Summary`, numbered `## Findings` with `[#N]` citations,
  `## In-Project Cross-References`, and `## Open Questions`. The response is parsed
  and used to populate `RESEARCH.md`. If the LLM is unavailable, misconfigured, or
  returns empty output, the session falls back to the existing mechanical
  summary/findings.
- **New `ragent-research::analysis` module** — Introduces `AnalysisEngine`,
  `NoopAnalysisEngine`, `LlmAnalysisEngine`, `SourceBody`, and
  `AnalysisResult` so callers can plug in alternative analysis implementations.
- **Updated TUI wiring** — `build_research_session` in
  `crates/ragent-tui/src/research_adapter.rs` now accepts an optional
  `ProviderRegistry` and active `ModelRef` and constructs an `LlmAnalysisEngine`
  when both are present. CLI and HTTP research creation endpoints continue to use
  `NoopAnalysisEngine`, preserving their current behaviour.
- **Progress tracking** — `SessionPhase::Synthesize` and the research progress
  tracker now display the synthesis phase in the TUI log.

### Changed
- **`ragent-research` dependencies** — Added `ragent-llm`, `ragent-config`, and
  `ragent-storage` (dev-only for tests) so the crate can build LLM clients and
  accept provider auth without leaking storage types across the public API.
- **Research system specification** — `specs/researchsystem/SPEC.md` updated with
  FR-021 (AI-Driven Source Synthesis) and FR-022 (Graceful Degradation of
  Synthesis), plus a new Research System section in the top-level `SPEC.md`.

## Version: 0.1.0-alpha.116 (unreleased)

### Fixed — persistence and performance of agent loop
- **Fix persistence and improve performance of agent loop** — Addressed
  persistence-related issues in the session/cache and storage layers and
  reduced overhead in the session processor hot path. Bumps workspace version
  to `0.1.0-alpha.116`.

## Version: 0.1.0-alpha.114 (unreleased)

### Changed — eliminate duplicated team tool source
- **Single source of truth for team coordination tools** — The 20 team tool
  files (`team_approve_plan`, `team_assign_task`, `team_broadcast`,
  `team_cleanup`, `team_create`, `team_idle`, `team_memory_read`,
  `team_memory_write`, `team_message`, `team_read_messages`,
  `team_shutdown_ack`, `team_shutdown_teammate`, `team_spawn`, `team_status`,
  `team_submit_plan`, `team_task_claim`, `team_task_complete`,
  `team_task_create`, `team_task_list`, `team_wait`) previously existed as
  byte-for-byte identical copies in both `crates/ragent-agent/src/tool/` and
  `crates/ragent-team/src/tools/`. Every COMMSPLAN fix had to be applied twice
  and re-synced with `cp`. The `ragent-agent` copies have been deleted and
  `crates/ragent-agent/src/tool/mod.rs` now compiles each tool from the
  canonical `crates/ragent-team/src/tools/team_*.rs` file via `#[path]`
  includes — the same mechanism already used for the team runtime modules in
  `crates/ragent-agent/src/team/mod.rs`. Edits to the team tools now only need
  to be made in one place. The CI guard `scripts/check-team-duplication.sh`
  was extended to reject any re-introduced physical `team_*.rs` copy under
  `crates/ragent-agent/src/tool/` and to verify every tool has a `#[path]`
  include.

### Added — COMMSPLAN Milestone 4 (message delivery semantics)
- **Read-vs-processed split in mailbox consumption (M4-T1)** — `Mailbox` gained
  `peek_unread()` (returns unread messages **without** marking them read) and
  `acknowledge(message_id)` (the explicit "I processed this" ack, semantically
  `mark_read`). `team_read_messages` now peeks, builds its output, and only
  acknowledges each message once the `ToolOutput` is ready — so a failure
  mid-build leaves the messages unread and they are redelivered on the next
  call (at-least-once semantics). `drain_unread` is kept for the mailbox poll
  loop, which treats event publishing as the processing step. As part of this,
  `mark_read` / `acknowledge` now return `changed` instead of `pos.is_some()`,
  making acknowledge idempotent (a second ack of an already-read message
  reports `false`), matching the documented contract.
- **`team_assign_task` notifies the assigned teammate (M4-T2)** — After
  updating `tasks.json`, the tool pushes a `MailboxMessage` to the assignee's
  mailbox so they are notified immediately instead of having to poll
  `team_task_list` / `team_task_claim`. The notification outcome is reported
  in the tool output (`Notification: delivered` / `failed: …`). The tool also
  rejects assignment to `Stopped` / `Failed` teammates up front.
- **`team_broadcast` reports per-recipient results (M4-T3)** — The
  early-return `?` on the first failure is replaced with a loop that collects
  `Result` per recipient, so a failure on one teammate no longer aborts
  delivery to the rest. The tool output includes `succeeded` and `failed`
  arrays (with per-failure error text) in the JSON metadata.
- **`team_message` validates recipient state (M4-T4)** — Before pushing, the
  tool loads `TeamStore` and rejects messages to `Stopped` / `Failed`
  teammates and to unknown agent IDs, so the sender gets an error instead of a
  false success. Messages to `lead` and active teammates are delivered as
  before.
- **`team_read_messages` output schema fixed (M4-T5)** — The JSON metadata
  now serialises `message_type` via `serde_json::to_value` (snake_case,
  matching the on-disk `#[serde(rename_all = "snake_case")]` format) instead
  of `format!("{:?}", …)` (PascalCase), and includes the `to` and `read`
  fields. The human-readable text now shows `To:` and the snake_case type.
- **Delivery regression tests (M4-T1..T5)** — New `ragent-team` integration
  test suite `tests/test_m4_delivery.rs` (12 tests) covering peek/ack
  idempotence, redelivery-on-no-ack, assign-task notification, dead-assignee
  rejection, per-recipient broadcast results, stopped/unknown recipient
  rejection, and the snake_case `team_read_messages` schema.

### Added — COMMSPLAN Milestone 3 (team liveness, shutdown, idle signalling)
- **`team_wait` subscribes before reading team state (M3-T1)** — The event-bus
  receiver is now created *before* the initial `TeamStore::load` so a teammate
  that goes idle or fails between the store read and the wait loop is captured
  rather than missed. A pre-loop `try_recv` drain reconciles any events that
  arrived during the store scan into the `waiting_for` set.
- **`team_wait` handles `TeammateFailed` (M3-T2)** — A failed teammate is now
  removed from the waiting set on receipt of `Event::TeammateFailed`, so the
  lead no longer waits the full 300 s timeout for an agent that will never
  become idle.
- **`team_wait` re-checks disk state on timeout (M3-T3)** — Before returning a
  timeout, `team_wait` reloads the team store and treats any member whose
  on-disk status is `Idle`, `Failed`, or `Stopped` as finished. This recovers
  terminal state when an `EventBus` event was dropped (buffer full / no
  subscribers) but the teammate legitimately reached a terminal state on disk.
- **`team_idle` publishes `Event::TeammateIdle` (M3-T4)** — After marking the
  member `Idle` on disk, the tool now publishes `TeammateIdle` on the event
  bus so `team_wait` and the TUI/SSE observe the transition even when the
  mailbox poll loop does not deliver an `IdleNotify` message. The lead session
  id is derived from the on-disk team config.
- **Unified shutdown path (M3-T5/T6)** — `TeamManagerInterface` gained a
  `shutdown_teammate(agent_id, graceful)` method, implemented once on
  `TeamManager` and used by both the `team_shutdown_teammate` tool and
  internal callers (`shutdown_all`, the TUI teardown paths). Graceful shutdown
  marks the member `ShuttingDown` and pushes a `ShutdownRequest` without
  forcing cancel; immediate shutdown sets the agent-loop and poll-loop cancel
  flags, deregisters the mailbox notifier, pushes a `ShutdownRequest` as a
  fallback, and marks the member `Stopped`. The tool gained an `immediate`
  parameter (default `false`) and falls back to a disk-only path when no
  `TeamManager` is wired into the context.
- **Lifecycle regression tests (M3-T7)** — New `ragent-team` integration test
  suite `tests/test_m3_lifecycle.rs` covering all four required scenarios:
  (a) teammate fails while lead is in `team_wait`, (b) teammate goes idle
  before `team_wait` starts, (c) `EventBus` event dropped but disk state
  correct, (d) `team_idle` publishes `TeammateIdle` and `team_shutdown_teammate`
  marks `ShuttingDown` (graceful) / `Stopped` (immediate).

### Added
- **Unified whitespace-tolerant replacement matcher** — `edit`, `multiedit`,
  and `memory_replace` now share a single seven-pass matcher in the new
  `ragent_tools_core::replace` module (`find_replacement_range` /
  `find_replacement_range_diag`). The matcher tolerates CRLF line endings,
  trailing/leading whitespace differences, collapsed-whitespace (tabs vs
  spaces, double spaces, mixed indentation), blank-line edge differences, and
  final-newline mismatches — eliminating `old_str not found` failures caused
  by common LLM output quirks. `memory_replace` previously used exact-only
  `String::matches`/`replacen` and would fail on the same whitespace quirks
  that `edit` already handled; it now behaves identically to `edit`.
- **`stream.initial_response_timeout_secs` config knob** — New optional
  field on `StreamConfig` (default `300`) that bounds how long the HTTP
  client waits for the **first byte** of a streaming response.  This is
  distinct from `stream.timeout_secs` (default `120`), which now exclusively
  governs the gap between subsequent stream deltas.  Cloud-hosted models
  (Ollama Cloud, Bedrock, Copilot, Azure AI Foundry) routinely need
  30-90 s for cold-start, which the previous shared 120 s timeout was
  insufficient to absorb once 4-5 swarm teammates started hammering the
  same provider concurrently.

### Changed
- **`multiedit` overlap detection & ordering** — `MultiEditTool::execute` now
  resolves every edit against the **original** file content (so byte ranges
  are stable), pairwise-checks edits on the same file for intersecting byte
  ranges (rejecting with a clear error naming the edit indices and file path),
  and applies non-overlapping edits highest-end-offset-first so the JSON input
  order no longer matters. Touching ranges (`a.end == b.start`) are allowed.
- **`multiedit` / `edit` diagnostics** — `NotFound` errors from `multiedit`
  now name the edit index, the file path, the last matching pass attempted
  (e.g. `collapsed`, `final-newline`), and a best-effort closest-line hint,
  via the new `FindDiag` / `find_replacement_range_diag` API. The original
  `find_replacement_range` remains as a thin wrapper so `edit` and
  `memory_replace` are unaffected.
- **Relative indentation preservation** — `reindent_with` now uses the
  **common** leading whitespace of all matched file lines (via
  `common_leading_ws`) rather than just the first line's full indentation,
  and leaves blank lines untouched so no trailing whitespace is introduced.
- **Swarm teammate retry backoff is now exponential with jitter** —
  `teammate_retry_backoff(attempt)` (in `ragent-team`, used by
  `TeamManager::spawn_teammate_internal`) replaced the previous linear
  `500 ms × attempt` schedule with an exponential curve
  (`1 s, 2 s, 4 s, 8 s` for attempts 1-4) plus up to 500 ms of clock-
  derived jitter, capped at 30 s.  The previous linear schedule caused
  every teammate that failed at the same moment to retry in lockstep,
  re-triggering the same upstream rate-limit / cold-start pressure on
  cloud LLMs (Ollama Cloud, Bedrock, Copilot).  The new helper is exposed
  publicly for downstream tooling and is covered by 4 integration tests
  in `crates/ragent-team/tests/test_teammate_retry_backoff.rs`.

### Fixed
- **Team subsystem data-loss races (COMMSPLAN Milestone 1)** —
  `Mailbox`, `TaskStore`, and `TeamStore` now serialise all mutating disk
  operations on a stable companion lock file (`*.json.lock`) using `fs2`
  advisory locks.  The lock is held across the full read-modify-write cycle
  and the atomic rename of a uniquely-named temp file, eliminating the
  previous TOCTOU window where concurrent writers released `flock` before
  the data reached disk.  Temp file names now include a UUID so concurrent
  writers cannot collide on a shared `.tmp` path.  Added regression tests
  `parent_mailbox_concurrent_push`, `parent_task_store_concurrent_claims`,
  and in-process threaded variants in
  `crates/ragent-team/tests/test_concurrent_store_writes.rs`.

  - **Team implementation unified (COMMSPLAN Milestone 2)** —
    `crates/ragent-agent/src/team/` now contains only `mod.rs`, which
    source-includes the implementation from `crates/ragent-team/src/team/`
    via `#[path]` attributes.  The duplicated local copies of
    `config.rs`, `mailbox.rs`, `manager.rs`, `store.rs`, `swarm.rs`,
    `task.rs`, and the previous `store.rs` `#[path]` wrapper have been
    removed, so fixes such as the Milestone 1 lock-file changes apply to
    both crates from a single source.  A `MemoryScope::as_str()` helper was
    added so that `TeamManager` can compare memory scopes across the
    source-inclusion boundary.  Added `docs/team-unification-decision.md`
    documenting why `#[path]` is used (the existing Cargo dependency cycle
    between `ragent-agent` and `ragent-team`) and the future path to a real
    crate dependency.  Added `scripts/check-team-duplication.sh` CI guard
    that fails if local team source files reappear in `ragent-agent`.

### Fixed
- **`old_str not found` on blank-line / final-newline edge differences** —
  Added blank-line normalisation (pass 6) and final-newline normalisation
  (pass 7) passes to the matcher, handling `str::lines()` inconsistencies
  around leading/trailing blank lines and trailing `\n` disagreements in
  either direction.
- **Collapsed-whitespace false `MultipleMatches`** — When collapsed matching
  yields multiple candidates, the matcher now prefers the candidate whose
  per-line leading whitespace is closest (smallest total char-length
  distance) to the needle's, rather than hard-erroring. Ties still error
  with `MultipleMatches`.
- **Swarm synthesis task timing out on cloud LLM providers** — When the
  final `/swarm` subtask (typically the architect agent) started after the
  other 3-4 parallel teammates had already consumed provider rate-limit
  budget, Ollama Cloud and similar hosted services would frequently fail
  to deliver the first byte within the configured 120 s
  `stream.timeout_secs`, surfacing as repeated `Ollama Cloud chat request
  timed out` warnings and, on the final synthesis task, an unrecoverable
  stall.  The fix splits the conflated timeout field into
  `initial_response_timeout_secs` (300 s, forwarded to providers as
  `ChatRequest.stream_timeout_secs`) and `timeout_secs` (120 s, used only
  for per-event stall detection).  Paired with the new exponential
  teammate retry backoff, the final synthesis task now completes on the
  first attempt instead of exhausting retries on cold-start latency.

## Version: 0.1.0-alpha.113 (unreleased)

### Fixed
- **`/research create` now wires real gatherers in the TUI** — Previously `handle_research_command` built every session with `ResearchSession::new(manager, None, None)`, so web search and local cross-referencing were skipped and every research item had zero sources.  The TUI now builds a `ResearchSession` backed by the existing agent tool registry (`websearch`/`webfetch` for the web phase, `glob`/`grep`/`read`/`list` for the local/spec phase) via new adapters in `crates/ragent-tui/src/research_adapter.rs`.
- **`/research create` now reports completion in the TUI** — The TUI makes sure a session exists before spawning the research task, so the `TuiResearchObserver` `AgentNotice` events are routed to the active session.  The status bar now updates to the `Done` message instead of staying stuck on "research: writing …".
- **`/research list|show|search` output no longer distorts columns** — `render_markdown_to_ascii` now bypasses the markdown→HTML→text and ASCII-table-normalisation pipeline for `/research` responses that contain a fenced code block, returning the already-formatted plain text directly.  This keeps the fixed-width tables aligned and readable.
- **Research keyword matching improved** — `derive_terms()` in `ragent-research` now strips ASCII punctuation (except apostrophes in contractions) and splits on internal punctuation such as `/` and `,`, so topics like `"async/await, tokio!"` produce usable terms.  `LocalGatherer::gather_specs()` now actually filters specs by the research topic and falls back to all specs only when fewer than three relevant specs are found.

## Version: 0.1.0-alpha.113

### Fixed
- **Research-system spec status overstated (second occurrence)** — `specs/researchsystem/SPEC.md` was again tagged `status: implemented` with a three-step audit trail even though only six of the 56 plan tasks had shipped (T-001/T-002/T-003 foundational types, T-005 crate scaffold, T-004 `ResearchItem`, T-014 web-gatherer, T-016 local-gatherer, T-040 plan-dep parser, T-045 path-traversal rejection, T-051 name-validation tests). The frontmatter is now `status: in_progress` with a two-step audit (`none → draft → in_progress`) that correctly reflects "framework implemented, integration pending". The remaining ~50 tasks (manager CRUD, session orchestrator, supporting-file writers, `RESEARCH.md` assembler, References Index generator, `INDEX.md` cache, TUI slash-command wiring, CLI/HTTP endpoints, spec-integration glue, benchmarks, user docs) remain `pending` and will be promoted in subsequent releases.

## Version: 0.1.0-alpha.112

### Fixed
- **Research-system spec status overstated** — `specs/researchsystem/SPEC.md` was committed with `status: implemented` and a three-step audit trail even though only four of the 22 plan tasks were actually completed (T-001 `ResearchName`, T-002 `ResearchStatus`, T-003 `Source`, T-005 crate scaffold). The frontmatter is now `status: draft` with a single `none → draft` audit transition, and the remaining 18 tasks remain `pending` in `specs/researchsystem/PLAN.md` until the gathering engine, TUI slash command, CLI/HTTP endpoints, and spec integration land in follow-up releases.

## Version: 0.1.0-alpha.111

### Changed
- **`ask_user` tool promoted from alias to standalone** — The previously-delegating `ask_user` tool in `crates/ragent-agent/src/tool/aliases.rs` now publishes `Event::QuestionRequested` and awaits `Event::QuestionAnswered` directly via the event bus. The standalone `question` tool has been deleted from `ragent-agent`, `ragent-tools-core`, and the TUI question-dialog widget module; question rendering is now driven entirely by the existing TUI event handler in `ragent-tui/src/app.rs`.
- **`ask_user` supports multiple-choice** — The optional `options` array parameter renders a selectable list in the TUI question dialog; omitting `options` keeps the previous free-text input behaviour. The tool description and JSON schema now document the new parameter, and `permission_category` is reported as `ask_user` (was `question`).
- **Permission auto-approval key renamed** — `check_permission_with_prompt`'s hardwired always-allow list now matches `ask_user` (was `question`); the corresponding unit test was renamed accordingly.

### Added
- **`ragent-research` crate scaffold** — New workspace member under `crates/ragent-research/` providing `ResearchName` (validated, URL-safe identifier newtype with FR-002 validation), `Source` (Web/Local/Spec/Other enum backing the references index), and `ResearchStatus` (draft/in-progress/complete/archived covering FR-013). Depends only on `ragent-types` and common workspace deps so it can be reused by both the TUI and HTTP layers once the manager/session/io modules are added in follow-up releases.
- **Research system spec + plan** — New `specs/researchsystem/SPEC.md` and `specs/researchsystem/PLAN.md` describing the `/research` slash command, directory conventions, information-gathering session, references index, and integration with the existing spec workflow.

## Version: 0.1.0-alpha.110

### Removed
- **Internal LLM subsystem removed** — The embedded local LLM (Candle GGUF + Foundry Local + LiteRT-LM), the `/internal-llm` slash command family, the TUI `InternalLLM` chat overlay panel, the `InternalLlmConfig` block, the `internal_llm` Cargo feature flag, and all related test files have been removed. Compaction now always uses the provider-compaction fallback. Session titles default to empty (no longer auto-generated). Memory extraction no longer has an LLM prefilter step. The `internal_llm` key in `ragent.json` is silently ignored. This supersedes the prior Foundry Local internal-LLM backend and the LiteRT-LM backend switch work.

## Version: 0.1.0-alpha.109

### Added
- **In-process Microsoft Foundry Local backend** — New `FoundryLocalInProcClient` in `crates/ragent-llm/src/providers/foundry_local_inproc_client.rs` loads and runs Foundry Local models inside the ragent process via the `foundry-local-sdk` native core, bypassing the local web service.  Supports model alias resolution, download progress events, device selection (`auto`/`cpu`/`gpu`/`npu`), temperature/max_tokens, tools, and full `StreamEvent` translation (text, tool calls, usage, finish reason).
- **`in_process` provider option** — `provider.foundry_local.in_process` (default `false`) selects the in-process backend; when unset or `false` the existing web-service path is preserved.
- **`RAGENT_FOUNDRY_LOCAL_FORCE_WEB` escape hatch** — Set this environment variable to `1` or `true` to force the web-service path even when `in_process: true` is configured.
- **TUI foundry-mode indicator** — `/internal-llm show` now displays whether the main Foundry Local provider is configured for `in-process` or `web-service` inference when the internal LLM backend is `foundry`.

### Changed
- **Foundry Local provider routing** — `FoundryLocalProvider::create_client()` now branches on the resolved `in_process` flag, returning either `FoundryLocalInProcClient` or the existing `FoundryLocalClient`.
- **Device validation** — `provider.foundry_local.device` values are now validated and rejected if not one of `auto`, `cpu`, `gpu`, or `npu`.
- **Foundry Local documentation** — Updated `PROVIDERS.md` and `SPEC.md` with in-process mode configuration, environment escape hatch, and internal-LLM notes.

### Fixed
- **HuggingFace provider discovery failed** — The HuggingFace `/v1/models` router endpoint is public and now works without an API token; discovery no longer errors out immediately when `HF_TOKEN` is unset.  Added `HUGGING_FACE_HUB_TOKEN` as a recognised token source for consistency with the TUI configured-provider detection.  When dynamic discovery fails or returns no models, the TUI now falls back to the provider's static default catalog instead of showing an empty "No models are currently available" dialog.  Empty discovery results are no longer cached, preventing a transient failure from permanently hiding the default models.
- **Task tool family guidance** — Added a dedicated `## Task Tool Family` section to every primary agent's system prompt that clearly distinguishes `task_complete` (autonomous loop signal — only takes `summary`) from `team_task_complete` (team workflow — only takes `team_name` + `task_id`).  The `task_complete`, `team_task_complete`, and `new_task` tool descriptions and JSON schemas now explicitly warn against the most common parameter-confusion mistakes and reject unknown keys via `additionalProperties: false`.  `task_complete` and `list_tasks` are now hardwired auto-approved so the agent can always finish or inspect background tasks without a permission prompt.

## Version: 0.1.0-alpha.108

### Added
- **Foundry Local internal-LLM backend** — New `FoundryLocalExecutor` in `ragent-agent/src/internal_llm/foundry_executor.rs` routes internal-LLM requests through Microsoft Foundry Local instead of the Candle-based embedded runtime. The `/internal-llm foundry` and `/internal-llm embedded` slash commands switch between backends at runtime, and `from_config()` now dispatches on `config.backend` (`"foundry"`/`"foundry_local"` vs default candle).

### Changed
- **Internal LLM backend routing** — `InternalLlmService::from_config()` now selects the executor based on the configured backend name, supporting both Candle (`embedded`) and Foundry Local (`foundry`/`foundry_local`) paths.
- **TUI /internal-llm commands** — Added `foundry` and `embedded` subcommands to the `/internal-llm` slash command for switching backends. Updated autocomplete list, help text, and slash-command definition.
- **Compiled backends display** — Replaced litertlm feature-flag detection with Foundry Local availability check (`is_foundry_local_available()`) in the TUI show/info panel.
- **Workspace version** — Bumped to `0.1.0-alpha.108`.

### Fixed
- **Microsoft Foundry Local empty SSE stream after model preparation** — `wait_for_model_ready` previously polled `/v1/models`, which lists downloaded/cataloged models and may report a model before it is actually loaded into memory. Chatting with an unloaded model caused the empty/malformed event-stream error seen in the TUI. The readiness check now polls the web service's `/models/loaded` endpoint (the authoritative "loaded into memory" signal) and falls back to `/v1/models` only on older services. The model load id is now taken directly from the SDK's full variant id, making load requests more robust across catalog versions.

## Version: 0.1.0-alpha.107

### Fixed
- **Compression pipeline threshold gating** — Added `should_compress` and `should_compress_chat_messages` checks before invoking the full compression pipeline, preventing unnecessary overhead and unconditional UI events when the conversation is well within the context window. The initial-history compression and per-iteration compression now both gate on the configured `auto_threshold` (default 0.80) before running. Added 2 new unit tests for the chat-messages threshold helper.

### Changed
- **Workspace version** — Bumped to `0.1.0-alpha.107`.

## Version: 0.1.0-alpha.106

### Added
- **Microsoft Foundry Local provider integration** — Added first-class support for Microsoft Foundry Local as a local LLM provider, including provider setup dialog visibility, `[local]` badge rendering, status-bar abbreviation, health checks, and configuration option merging (`auto_start`, `device`, `models_path`).
- **Headroom compression lifecycle events** — New `Event::CompressionStarted` and `Event::CompressionFinished` events are published by the session processor and consumed by the TUI and SSE stream. They carry `original_tokens`, `compressed_tokens`, `compression_ratio`, and `did_compress` so observers can show live progress and update context-window displays immediately.

### Changed
- **Workspace version** — Bumped to `0.1.0-alpha.106`.
- **Per-iteration compression visibility** — The agent loop now publishes `CompressionStarted`/`CompressionFinished` around every automatic Headroom compression run. The TUI sets `compress_in_progress` while the pipeline is active, so the existing status-bar "compressing" indicator actually appears during automatic compression, and the `ctx:` display is refreshed with the post-compression token count as soon as the run completes.
- **SSE event coverage** — `ragent-server` now serializes `compression_started` and `compression_finished` SSE events.

### Fixed
- **Context window display lag after compression** — `last_input_tokens` was only updated when the provider returned token usage, so the status-bar context percentage stayed at the pre-compression value until the LLM response arrived. The TUI now updates `last_input_tokens` directly from `CompressionFinished`, keeping the `ctx:` display in sync with the actual request size.

## Version: 0.1.0-alpha.105

### Added
- **Context compression pipeline** — New compression module (`ragent-agent/src/compression/`) with multi-strategy history compaction: BM25 relevance scoring, CCR (Critical Content Retention) store, aggressive/conservative/default modes, and `/compress` slash command integration.
- **Model Router Provider** — New intelligent model routing system (`ragent-llm/src/providers/router*.rs`) with 15-dimension classifier (complexity, creativity, code, reasoning, vision, image_attachment, etc.), automatic model selection, and configurable routing rules.
- **Compression config** — New `compression.rs` module in ragent-config for compression pipeline configuration.
- **String utilities** — New `strutil.rs` module in ragent-types for shared string helpers.
- **Spec ID scanner** — New `id_scanner.rs` in ragent-specs for extracting and tracking spec requirement/task IDs.
- **HeadroomCompress spec** — Full specification for the compression feature in `specs/HeadroomCompress/`.
- **ModelRouterProvider spec** — Full specification for the model router in `specs/ModelRouterProvider/`.
- **Config defaults fix** — Added `#[serde(skip_serializing_if)]` to `code_index.enabled`, `internal_llm.enabled`, and `tool_visibility.codeindex` so auto-generated config files don't override code-level defaults. Added 10 regression tests.
- **Compression indicator test** — New TUI test for compression status display.

### Changed
- **Agent system** — Refactored agent module with expanded presets and compression integration.
- **Session processor** — Added compression integration, improved tool call handling, and spec command support.
- **TUI** — Added `/compress` slash command, compression status bar indicator, and improved status bar layout.
- **Spec commands** — Major refactor of spec command handling with expanded `/spec impl` and `/spec implement` support.
- **Bedrock provider** — Refinements to credential handling and SigV4 signing.
- **Multiple tool refinements** — Updated codeindex_search, list_tasks, memory_search, office_write, and spec_list tools.
- **Test improvements** — Updated multiple test files for compatibility with new APIs and module structure.
- **TUI toggle persistence** — `/codeindex`, `/internal-llm`, and `/tools` toggles now save to a project-local `.ragent/ragent.json` (creating the directory if needed) instead of falling back to the global config. They also skip writes when the target value has not changed, avoiding unnecessary file churn.
- **YOLO mode persistence** — YOLO mode state is now saved to the config file (`yolo: true/false`) and restored on startup. Toggling via `/yolo` or the `InputAction::ToggleYolo` keybinding persists the new state immediately.

### Fixed
- **Microsoft Foundry Local visibility** — The provider was already registered in the LLM layer and listed by `ragent models`, but was missing from the TUI provider setup dialog and the user-facing provider list. Added `foundry_local` to `PROVIDER_LIST`, the provider setup/reset flows, configured-provider detection, health checks, and status-bar abbreviation, and rendered a `[local]` badge alongside Ollama. Also merged `provider.foundry_local` options (`auto_start`, `device`, `models_path`) from `ragent.json` into the client creation path. Updated README.md and SPEC.md to include Microsoft Foundry Local.
- **TUI provider picker scrolling** — The provider setup dialog used a fixed 50% height and rendered all providers in a single paragraph, so on typical 24-row terminals the bottom of the list (including Microsoft Foundry Local) was clipped off-screen and unreachable. The dialog is now taller (capped at 22 rows) and the provider list scrolls to keep the selected item visible, with a "(more providers below)" hint when the list overflows.
- **Per-iteration context compression** — The Headroom compression pipeline was only run once at the start of an agent run, so the LLM request payload kept growing as the agent loop appended assistant tool uses and tool results each turn. Once the payload exceeded the model's context window, the provider returned an error and the task failed. The agent loop now re-runs compression before every LLM call when the configured `auto_threshold` (default 0.80) is exceeded, satisfying FR-005. Added `compress_chat_messages` round-trip helpers and 6 unit tests in `ragent-agent/src/compression/pipeline.rs`.
- **Config defaults for CodeIndex and InternalLLM** — When `Config::load()` created a default config file (no existing config), it serialised all fields including default values like `"enabled": true` for `code_index` and `"enabled": false` for `internal_llm`. These explicit values then overrode any future code-level default changes. Added `#[serde(skip_serializing_if)]` annotations to `code_index.enabled`, `internal_llm.enabled`, and `tool_visibility.codeindex` so default values are omitted from serialised output, allowing code-level defaults to take effect automatically. Added 10 regression tests in `test_code_index_config.rs`.
- **Foundry Local always compiled** — Removed the empty `foundry-local` feature flag from the root `Cargo.toml` defaults. The `foundry-local-sdk` dependency in `ragent-llm` is already unconditional, so the Microsoft Foundry Local provider is now always present with no compile-time gate.

## Version: 0.1.0-alpha.104

### Added
- **Amazon Bedrock provider** — Full AWS Bedrock support with SigV4 request signing (no AWS SDK dependency), dual API clients (Anthropic Messages API for Claude models, Converse API for all other models), 9 default models, short alias mapping, `@bedrock` suffix stripping, `ListFoundationModels` discovery, and credential resolution chain (env vars → AWS profile INI → session tokens).
- **xAI Grok provider** — New `xai.rs` provider for the xAI Grok API, registered in the default provider registry.
- **Spec implementation commands** — `/spec impl` and `/spec implement` slash commands for spec lifecycle: generates implementation plans, tracks progress against requirements, and runs implementation tasks.

### Changed
- **Copilot provider improvements** — Updated copilot.rs with refinements to the GitHub Copilot provider.
- **HuggingFace provider improvements** — Updated huggingface.rs with refinements.
- **Provider registry** — Added Bedrock and xAI to `create_default_registry()`.
- **Spec module expanded** — New `impl_runner.rs` and `plan_parser.rs` modules in ragent-specs.

## Version: 0.1.0-alpha.103

### Fixed
- **Bash syntax validation on Windows** — `validate_bash_syntax()` previously used a hardcoded `sh -n -c` command, which fails on Windows because `sh` is not available. Now uses the discovered shell program: `bash -n -c` on Unix, the Git Bash executable path on Windows (Git Bash), and skips validation entirely for PowerShell. This eliminates the "program not found" error when running bash commands on Windows 11.

## Version: 0.1.0-alpha.102

### Added
- **Windows shell support for BashTool** — The `BashTool` now runs on Windows with automatic shell discovery (Git Bash preferred, PowerShell fallback). All 7 security layers remain active regardless of platform. Windows-specific directory-escape detection blocks `C:\`, `D:\`, and `\` paths. PowerShell syntax validation is skipped (PowerShell self-validates at runtime). State files are stored in `%LOCALAPPDATA%\ragent\shell\` on Windows.

### Changed
- **Refactored `is_directory_escape_attempt`** — Split into `is_directory_escape_attempt` (public, calls inner) and `is_directory_escape_attempt_inner` (testable inner function with explicit `on_windows` parameter). This enables testing Windows-specific path detection on any platform.
- **Shell discovery caching** — Added `OnceLock`-based process-global shell type cache so shell discovery only runs once per process.

### Fixed
- **Directory escape test** — `test_directory_escape_absolute` now uses `tempfile::tempdir()` for a real filesystem path, avoiding `canonicalize()` hangs on nonexistent paths.

## Version: 0.1.0-alpha.101

### Fixed
- fixed AGENTS.md load path

## Version: 0.1.0-alpha.100

### Added
- **Sub-agent suspend/resume/kill lifecycle** — New `suspend_task()`, `resume_task()`, and `kill_task()` methods on the task manager. Sub-agents can now be paused, resumed, or forcibly terminated with a 10-second force-kill escalation timeout. New `TaskStatus::Suspended` and `TaskStatus::Terminating` states, plus `SubagentSuspended`, `SubagentResumed`, and `SubagentKilled` events.
- **Teammate suspend/resume events** — `TeammateSuspended` and `TeammateResumed` events for team coordination, enabling lead agents to pause and resume teammates.
- **Enhanced active-agents panel** — TUI active agents panel now shows per-agent status (running/suspended), supports suspend/resume/kill actions, and renders agent step counts and elapsed time.
- **Enhanced teams panel** — TUI teams panel shows teammate statuses, suspend/resume buttons, and per-teammate progress indicators.
- **SSE events for sub-agent lifecycle** — Server-sent events now stream `SubagentSuspended`, `SubagentResumed`, `SubagentKilled`, `TeammateSuspended`, and `TeammateResumed` event types.

### Changed
- **Permission check indentation fix** — Re-indented `check_permission_with_prompt()` to correct a long-standing indentation issue.
- **AGENTS.md discovery sorting** — Improved instruction file priority sorting with properly formatted ordering logic.
- **Azure Resource provider refactoring** — Cleaned up `azure_resource.rs` provider implementation.
- **HTTP client retry logic** — Updated `execute_with_retry` in `http_client.rs`.

### Fixed
- **Instruction file discovery priority bug** — `collect_agents_md_content_with_discovery()` in `agent/mod.rs` was incorrectly calculating file depth (missing `saturating_sub(1)`), causing `AGENTS.md` in the project root to have depth=1 instead of depth=0. This meant root instruction files were treated as subdirectories, and the sorting step mixed root, global, and subdirectory candidates together. Now root files are correctly identified (depth=0), each priority tier is sorted independently, and concatenated in strict order: project root → global directory → project subdirectories. Added integration test `test_root_agents_md_beats_subdirectory_agents_md` to verify the fix.

## Version: 0.1.0-alpha.99

### Changed
- **updated splash screen text**

## Version: 0.1.0-alpha.98

### Added
- **Azure Resource Provider API type switch** — Added `api_type` field to `azureresources.json` entries. When set to `"anthropic"`, requests are routed to `{endpoint}/anthropic/v1/messages` using the Anthropic Messages API format with Azure-style `api-key` authentication. When set to `"openai"` or omitted, the existing OpenAI-compatible path (`{endpoint}/openai/v1/chat/completions`) is used.  
  - New `AzureAnthropicClient` wrapper in `azure_resource.rs` reuses `AnthropicClient` body construction and SSE parsing but sends `api-key` header instead of `x-api-key`.
  - `AzureResourceProvider::create_client` now branches on `api_type` by looking up the model ID in the cached entries.
  - TUI `SelectAzureResource` flow now persists `azure_resource` as the provider (instead of `azure_foundry`) and stores `azure_resource_api_base` alongside `azure_resource_last_selection`.
  - Session processor `resolve_api_key` and base-URL resolution now handle `azure_resource` provider directly.
  - Added unit tests for parser validation (`test_api_type_openai_accepted`, `test_api_type_anthropic_accepted`, `test_api_type_missing_defaults_to_openai`, `test_api_type_invalid_skipped_with_warning`).
  - Added integration tests for `create_client` branching (`test_azure_anthropic_create_client_branches_correctly`, `test_azure_openai_branch_unchanged`).
  - Updated `specs/AzureResource/FILEFORMAT.md` with `api_type` documentation.

## Version: 0.1.0-alpha.97

### Changed
- **add rate limiting logic**

## Version: 0.1.0-alpha.96

### Changed
- **fix for azure resource provider**
- **Azure AI Foundry / Azure Resource 429 rate-limit retry** — HTTP 429 (Too Many Requests) responses from Azure AI Foundry and Azure Resource endpoints are now treated as retryable. The `execute_with_retry` helper in `http_client.rs` has been updated to:
  - Detect `429 Too Many Requests` status code and automatically retry up to 4 times
  - Respect the `Retry-After` response header when present (integer seconds format)
  - Fall back to exponential backoff (2ˢ, 4ˢ, 8ˢ, 16ˢ) when no `Retry-After` header is provided
  - Log each retry attempt with the delay duration for transparency
- **AzureFoundryClient now uses `execute_with_retry`** — The chat request in `azure_foundry.rs` now routes through the retry-aware `execute_with_retry` path, so transient 429 errors will be handled automatically instead of surfacing as immediate failures.

### Changed
- **YOLO mode fixes** — Fixed YOLO mode permission bypass logic.
- **AGENTS.md search and inclusion order** — Updated AGENTS.md discovery and inclusion order handling.

## Version: 0.1.0-alpha.93

### Changed
- **Built-in agents no longer hardcode Anthropic Claude** — All 18 built-in agent definitions (`ask`, `general`, `build`, `plan`, `explore`, `title`, `summary`, `compaction`, `rust-coder`, `python-coder`, `go-coder`, `typescript-coder`, `java-coder`, `cpp-coder`, `csharp-coder`, `swift-coder`, `database-agent`, `frontend-agent`) previously hardcoded `model: Some(ModelRef { provider_id: "anthropic", model_id: "claude-sonnet-4-20250514" })`. They now default to `model: None` and auto-resolve the first available model from the provider registry at runtime. This means agents automatically use whatever provider/model the user has configured via `/provider`, `/model`, or `--model` instead of always falling back to Claude.

### Added
- **`resolve_default_model()` helper** — Scans the provider registry and returns the first model from the first provider, used when an agent has no explicit model binding.
- **`resolve_agent_with_model()` / `resolve_agent_with_customs_and_model()`** — Wrappers around `resolve_agent()` / `resolve_agent_with_customs()` that ensure the returned agent always has a model by falling back to `resolve_default_model()` when needed.

### Fixed
- **TUI initial agent setup** — `App::new()` now calls `resolve_default_model()` on the initial agent so startup works even when no model was previously persisted in storage.
- **TUI agent switching** — `apply_selected_model_and_thinking()` now falls back to `resolve_default_model()` when both `selected_model` and `agent.model` are `None`.
- **Server message handler** — `POST /sessions/{id}/messages` now uses `resolve_agent_with_model()` instead of `resolve_agent()` so server requests also auto-resolve the default model.

## Version: 0.1.0-alpha.91

### Fixed
- **TUI 5-minute stall / frozen ESC** — `refresh_memory_stats()` was doing synchronous file I/O (`load_all_blocks` reads ALL memory blocks from disk) + SQLite query on every single event-loop tick (~50 ms) with zero debouncing. When many memory blocks exist or SQLite has lock contention, the entire async runtime blocks for seconds, preventing keyboard events from being processed (ESC appears frozen). Added 5-second debounce to `refresh_memory_stats()` matching the pattern used by `refresh_code_index_stats()`. Also added 2-second debounces to `poll_swarm_unblock()` and `poll_swarm_completion()` which were doing filesystem I/O (`TeamStore::load_by_name`, `TaskStore::open`) on every tick.
- **Question dialog not rendering** — `Event::QuestionRequested` handler in `app.rs` was missing `self.needs_redraw = true`, causing the question dialog to never appear on screen until an unrelated input or event triggered a redraw. Added the flag so the dialog renders immediately when a `question` tool call arrives.

### Changed
- **Agent loop optimization** — Optimize the agent loop to prevent stalls.

## Version: 0.1.0-alpha.90

### Added
- **Git tool summaries in TUI** — `tool_input_summary` and `tool_output_summary` in `message_widget.rs` now provide human-readable summaries for all 16 git tools (`git_add`, `git_branch`, `git_checkout`, `git_cherry_pick`, `git_clone`, `git_commit`, `git_diff`, `git_fetch`, `git_log`, `git_merge`, `git_pull`, `git_push`, `git_remote`, `git_reset`, `git_show`, `git_stash`), showing actions like "🌿 add -A", "🌿 commit --amend", "🌿 merge feature-branch", etc.
- **GitHub tool summaries in TUI** — Added summaries for all 10 GitHub tools (`github_list_issues`, `github_get_issue`, `github_create_issue`, `github_comment_issue`, `github_close_issue`, `github_list_prs`, `github_get_pr`, `github_create_pr`, `github_merge_pr`, `github_review_pr`), displaying actions like "📋 issue #42 created", "📋 PR #7 merged", etc.
- **GitLab tool summaries in TUI** — Added summaries for all 14 GitLab tools (`gitlab_list_projects`, `gitlab_get_project`, `gitlab_list_issues`, `gitlab_get_issue`, `gitlab_create_issue`, `gitlab_close_issue`, `gitlab_list_prs`, `gitlab_get_pr`, `gitlab_create_pr`, `gitlab_get_pipeline`, `gitlab_list_jobs`, `gitlab_get_job`, `gitlab_retry_job`, `gitlab_cancel_job`), displaying actions like "🦊 project retrieved", "🦊 issue #5 created", "🦊 pipeline #3", etc.
- **Tool output summaries** — `tool_output_summary` function extended to cover git, GitHub, and GitLab tools with descriptive output strings.

## Version: 0.1.0-alpha.89

### Changed
- **README.md rebuilt** — Rewrote from scratch to reflect the current specification. Expanded feature list to ~111 tools across 15 categories, corrected provider list (10 providers), added missing systems (memory, spec management, skills, teams/swarm, autopilot, MCP client, config error reporting), updated architecture table with all 15 crates, and refreshed project status.
- **STATS.md updated** — Complete rewrite showing project-wide metrics (175,840 lines, 468 files, 1,670 tests) and a per-crate breakdown with file counts, line counts, test files, descriptions, ASCII bar chart, and architecture ratios.
- **SPEC.md cover page** — Added styled HTML cover page with title, author, version, date, and repository link.

## Version: 0.1.0-alpha.88

### Fixed
- **Context compaction bug** — Fixed `compact_history_with_atomic_tool_calls` which was breaking the compaction loop prematurely when the oldest message was part of a tool call pair, preventing any trimming. Now uses prefix sums and a proper scan to find the correct truncation point while preserving atomic tool call pairs.

### Changed
- **SPEC.md audit and corrections** — Comprehensive review of SPEC.md against actual codebase implementation. Corrected tool counts (File Ops 14, Execution 3, Memory 8, Team 20), removed non-existent tools (`execute_python`, `file_ops_tool`, dead aliases), fixed version string to `alpha.88`, added alpha.87/alpha.88 to Appendix A, replaced MCP stub (§19) and Auto-Update stub (§20) with real documentation, expanded §5.2 config schema to include `tool_visibility`, `dirs`, `bash`, `gitlab`, `internal_llm`, `stream`, added missing slash commands (`/config show`, `/dirs`, `/profile`, `/theme`, `/status`, `/mouse`) to §6.2, and expanded §5.3 environment variable table with all provider-specific keys.

## Version: 0.1.0-alpha.87

### Fixed
- **Read tool instructions** — Clarified in AGENTS.md that `end_line` is an absolute line number, not a count or offset.
- **Remote push instructions** — Strengthened AGENTS.md guidelines to explicitly prohibit pushing without explicit user instruction.

### Changed
- **SPEC.md reorganization** — Reorganized sections into logical order (1-20), fixed numbering, added Blueprints subsection (14.7), and merged GitHub/GitLab into a single peer section (18).

## Version: 0.1.0-alpha.86

### Added
- **Azure Resource (File) provider** — New `azure_resource` provider that reads Azure endpoint definitions from `azureresources.json` in `~/.config/ragent/` or `.ragent/`. Supports multiple resource entries with per-endpoint API keys, environment-variable-based keys, custom context windows, capability tags, and thinking configuration.
- **Azure Resource documentation** — Added `docs/userdocs/azure-resource.md` with full JSON schema, field reference, copy-pasteable example, and troubleshooting guide.
- **Azure Resource integration tests** — Added `crates/ragent-tui/tests/test_azure_resource_flow.rs` covering provider listing, persistence round-trip, stale selection cleanup, ModelInfo conversion, and backend resolution.

## Version: 0.1.0-alpha.85

### Changed
- **Version bump** — Incremented pre-release version for release.

## Version: 0.1.0-alpha.84

### Added
- **Azure test script** — Added `scripts/getresult.sh` for testing Azure AI Foundry chat completions.

## Version: 0.1.0-alpha.83

### Fixed
- **SPEC.md fixes** — Fixed malformed benchmark runner table, corrected Team Lifecycle mermaid diagram syntax, replaced misplaced GitLab Integration section with proper content, and updated version references throughout.

## Version: 0.1.0-alpha.82

### Added
- **azProvider fixes** — Applied fixes to Azure provider implementation.
- **`/config show`** — Added `/config show` slash command to display current configuration.

## Version: 0.1.0-alpha.81

### Fixed
- **Azure endpoint logging** — TUI log panel now displays the full endpoint URL for Azure AI Foundry requests.

## Version: 0.1.0-alpha.78

### Fixed
- **Azure endpoint logging** — TUI log panel now displays the full endpoint URL for Azure AI Foundry requests, not just the `[provider/model]` prefix.

## Version: 0.1.0-alpha.77

### Added
- **Azure endpoint logging** — Azure AI Foundry provider now logs the resolved endpoint via `tracing::info!` when connecting.

## Version: 0.1.0-alpha.76

### Added
- **Azure AI Foundry provider** — New `azure_foundry` provider for Microsoft Azure AI Foundry models. Supports OpenAI-compatible endpoints with `api-key` header authentication, dynamic model discovery, streaming chat completions, tool calling, vision, and reasoning levels (o1, o3-mini). Configurable via `AZURE_AI_FOUNDRY_API_KEY` and `AZURE_AI_FOUNDRY_BASE` environment variables or `ragent.json`.

## Version: 0.1.0-alpha.75

### Fixed
- **SPEC.md mermaid diagrams** — Fixed 2 diagrams (Figure 1 and Figure 7) where closing fences/`end` keywords were on the same line as node definitions, which broke rendering. All 14 diagrams now pass syntax validation and render correctly.

## Version: 0.1.0-alpha.73

### Fixed
- **/model selection** — Fixed `/model` selection handling.

## Version: 0.1.0-alpha.72

### Added
- **gen-spec-pdf.sh script** — New `scripts/gen-spec-pdf.sh` for converting Markdown specifications (with Mermaid diagrams) to PDF using Pandoc and Chromium.

### Changed
- **SPEC.md updates** — Removed LSP references and added a dedicated Spec Management section documenting the spec tool suite.

## Version: 0.1.0-alpha.71

### Added
- **Startup ASCII art banner** — The TUI now displays an ASCII art rendering of the application name on startup, followed by the version number and the exact date/time the binary was compiled.

## Version: 0.1.0-alpha.70

### Changed
- **Update concurrency** — Improved concurrency handling across the codebase.
- **Fix todos** — Resolved outstanding todo issues.

## Version: 0.1.0-alpha.68

### Added
- **`/codeindex` language filtering** — The `/codeindex` slash command now supports an optional `lang` parameter to filter code index results by programming language (e.g., `/codeindex lang rust`).

### Changed
- **Benchmark data cleanup** — Removed unused benchmark dataset files from `benches/data/` across multiple languages and suites, significantly reducing repository size.

## Version: 0.1.0-alpha.67

### Changed
- **Version bump** — Incremented to 0.1.0-alpha.67.

## Version: 0.1.0-alpha.66

### Changed
- **Version bump** — Incremented to 0.1.0-alpha.66.

## Version: 0.1.0-alpha.65

### Changed
- **Version bump** — Incremented to 0.1.0-alpha.65.

## Version: 0.1.0-alpha.64

### Changed
- **Version bump** — Incremented to 0.1.0-alpha.64.

## Version: 0.1.0-alpha.63

### Changed
- **Version bump** — Incremented to 0.1.0-alpha.63.

## Version: 0.1.0-alpha.62

### Changed
- **Version bump** — Incremented to 0.1.0-alpha.62.

## Version: 0.1.0-alpha.61

### Added
- **Instruction file discovery logging** — New `InstructionFileDiscovery` struct and `collect_agents_md_content_with_discovery()` function track which AGENTS.md-style files were found and where. Logs discovery summary via tracing and emits `AgentNotice` events for visibility.

### Changed
- **AgentNotice display** — TUI now displays `AgentNotice` events in the message window ("📋 **Agent Notice**" prefix) in addition to the status bar, making instruction file discovery visible to users.
- **Improved formatting** — AGENTS.md acknowledgment messages now include a newline separator for better readability.

## Version: 0.1.0-alpha.60

### Added
- **Global AGENTS.md search path** — Extended `collect_agents_md_content()` to search `~/.local/share/ragent/` for instruction files. Falls back to global files only when no local project files exist.

### Changed
- **AGENTS.md precedence** — Local project instruction files now completely replace global files, rather than being appended to them. This enables cleaner project-specific overrides of global guidelines.

## Version: 0.1.0-alpha.59

### Changed
- **Version bump** — Incremented to 0.1.0-alpha.59.

## Version: 0.1.0-alpha.58

### Changed
- **Version bump** — Incremented to 0.1.0-alpha.58.

## Version: 0.1.0-alpha.57

### Changed
- **Version bump** — Incremented to 0.1.0-alpha.57.

## Version: 0.1.0-alpha.56

### Added
- **Multilingual benchmark suites** — Added benchmark test files for Go, Java, JavaScript/TypeScript, Python, and Ruby to expand language coverage for the benchmark system.

## Version: 0.1.0-alpha.55

### Fixed
- **Permission milestone test fixes** — Fixed failing unit tests in `test_permission_system.rs` related to context window limits, message ordering, and compact history behaviour. Removed brittle assertions and added robust assertions for actual system state.

## Version: 0.1.0-alpha.54

### Fixed
- **Permission dialog timeout** — Fixed permission dialog timeout from 30 seconds to 120 seconds in `processor.rs`. Added `created_at` and `timeout_secs` fields to `PermissionRequest` struct in `permission/mod.rs`.

### Changed
- **Permission dialog countdown timer** — Implemented live countdown timer in permission dialog title in `crates/ragent-tui/src/input.rs`.

## Version: 0.1.0-alpha.53

### Fixed
- **Permission dialog live update** — Fixed permission dialog countdown not visually decrementing by changing main event loop to always redraw.

## Version: 0.1.0-alpha.52

### Changed
- **Bash safe command display** — Changed `SAFE_COMMANDS` from private to `pub const` in `bash.rs` and updated `/bash show` TUI command to display the built-in safe command list.

## Version: 0.1.0-alpha.51

### Changed
- **Bash permission command name extraction** — Added `extract_command_name()` helper in `processor.rs` to extract just the first word from a bash command before permission checking. Modified bash permission check loop to use command names.

## Version: 0.1.0-alpha.50

### Changed
- **Bash denylist word-boundary matching** — Split `DENIED_PATTERNS` into `DENIED_COMMANDS` (word-boundary matched via command name extraction) and `DENIED_PATTERNS` (substring matched). Added `extract_command_names()` helper in `bash.rs`.

## Version: 0.1.0-alpha.49

### Added
- **Permission dialog countdown** — Added countdown timer to permission approval dialog in TUI.
- **Config parse error enhancement** — Improved config file parser to show clear, actionable errors.
- **Codeindex hardwired permissions** — Made codeindex tools always allowed without permission checks.

## Version: 0.1.0-alpha.48

### Fixed
- **Permission milestones** — Fixed various issues in permission system and bash security layers.

## Version: 0.1.0-alpha.47

### Changed
- **Crate reorganisation** — Extracted `ragent-types`, `ragent-config`, `ragent-storage`, and `ragent-llm` from `ragent-core`.

## Version: 0.1.0-alpha.46

### Added
- **Permission system** — Implemented core permission system with 20 passing tests.

## Version: 0.1.0-alpha.45

### Added
- **Bash security** — Implemented 7-layer bash security system.

## Version: 0.1.0-alpha.44

### Added
- **Permission dialog** — Added permission approval dialog with timeout.

## Version: 0.1.0-alpha.43

### Added
- **Permission checker** — Implemented permission checker with allow/deny/ask rules.

## Version: 0.1.0-alpha.42

### Added
- **Permission rules** — Added permission rule evaluation with last-match-wins semantics.

## Version: 0.1.0-alpha.41

### Added
- **Permission request flow** — Implemented permission request flow with EventBus integration.

## Version: 0.1.0-alpha.40

### Added
- **Permission system foundation** — Added Permission enum, PermissionAction, PermissionRule, and PermissionChecker.

## Version: 0.1.0-alpha.39

### Added
- **Code index tools** — Added `codeindex_search`, `codeindex_symbols`, `codeindex_references`, `codeindex_dependencies`, `codeindex_status`, and `codeindex_reindex` tools.

## Version: 0.1.0-alpha.38

### Added
- **Code index** — Implemented codebase indexing with tree-sitter parsing and Tantivy FTS.

## Version: 0.1.0-alpha.37

### Added
- **Memory system** — Implemented three-tier memory system with file blocks, structured SQLite store, and semantic search.

## Version: 0.1.0-alpha.36

### Added
- **Teams** — Implemented multi-agent coordination with named teammates and shared task lists.

## Version: 0.1.0-alpha.35

### Added
- **Swarm mode** — Implemented swarm decomposition for parallel task execution.

## Version: 0.1.0-alpha.34

### Added
- **Autopilot mode** — Implemented autonomous operation mode.

## Version: 0.1.0-alpha.33

### Added
- **Custom agents** — Implemented OASF-based custom agent profiles.

## Version: 0.1.0-alpha.32

### Added
- **Skills system** — Implemented loadable skill packs.

## Version: 0.1.0-alpha.31

### Added
- **Prompt optimization** — Implemented `/opt` slash command with 12 methods.

## Version: 0.1.0-alpha.30

### Added
- **MCP client** — Implemented Model Context Protocol client support.

## Version: 0.1.0-alpha.29

### Added
- **Background agents** — Implemented sub-agent spawning and management.

## Version: 0.1.0-alpha.28

### Added
- **Event bus** — Implemented internal tokio pub/sub for real-time UI updates.

## Version: 0.1.0-alpha.27

### Added
- **Snapshot & undo** — Implemented file snapshots before edits.

## Version: 0.1.0-alpha.26

### Added
- **Project guidelines** — Implemented auto-loading of `AGENTS.md` from project root.

## Version: 0.1.0-alpha.25

### Added
- **Agent presets** — Implemented coder, task, architect, ask, debug, code-review agents.

## Version: 0.1.0-alpha.24

### Added
- **Permission system** — Implemented configurable permission rules.

## Version: 0.1.0-alpha.23

### Added
- **Session management** — Implemented persistent conversation history in SQLite.

## Version: 0.1.0-alpha.22

### Added
- **HTTP server** — Implemented axum-based REST + SSE API.

## Version: 0.1.0-alpha.21

### Added
- **Terminal UI** — Implemented full-screen ratatui interface.

## Version: 0.1.0-alpha.20

### Added
- **Tool system** — Implemented core tool registry and dispatch.

## Version: 0.1.0-alpha.19

### Added
- **GitHub integration** — Implemented GitHub tools for issues and PRs.

## Version: 0.1.0-alpha.18

### Added
- **GitLab integration** — Implemented GitLab tools for issues, MRs, and pipelines.

## Version: 0.1.0-alpha.17

### Added
- **Office tools** — Implemented office_read, office_write, office_info, libre_read, libre_write, libre_info.

## Version: 0.1.0-alpha.16

### Added
- **PDF tools** — Implemented pdf_read and pdf_write.

## Version: 0.1.0-alpha.15

### Added
- **Web tools** — Implemented webfetch, websearch, and http_request.

## Version: 0.1.0-alpha.14

### Added
- **Bash tool** — Implemented bash execution with security restrictions.

## Version: 0.1.0-alpha.13

### Added
- **File tools** — Implemented read, write, create, edit, multiedit, patch, copy_file, move_file, rm, mkdir, append_file, file_info, diff_files, glob, and list.

## Version: 0.1.0-alpha.12

### Added
- **Provider system** — Implemented Anthropic, OpenAI, and Ollama providers.

## Version: 0.1.0-alpha.11

### Added
- **Configuration** — Implemented ragent.json configuration loading.

## Version: 0.1.0-alpha.10

### Added
- **Storage** — Implemented SQLite-backed storage.

## Version: 0.1.0-alpha.9

### Added
- **Event system** — Implemented EventBus with tokio broadcast channels.

## Version: 0.1.0-alpha.8

### Added
- **Message system** — Implemented chat message types and serialization.

## Version: 0.1.0-alpha.7

### Added
- **LLM client** — Implemented HTTP client for LLM providers.

## Version: 0.1.0-alpha.6

### Added
- **Types** — Implemented shared types and IDs.

## Version: 0.1.0-alpha.5

### Added
- **CLI** — Implemented clap-based CLI with run, serve, session, auth, models, and config commands.

## Version: 0.1.0-alpha.4

### Added
- **TUI** — Implemented ratatui terminal interface.

## Version: 0.1.0-alpha.3

### Added
- **Server** — Implemented axum HTTP server with REST + SSE endpoints.

## Version: 0.1.0-alpha.2

### Added
- **Tools** — Implemented core tool system.

## Version: 0.1.0-alpha.1

### Added
- **Initial project scaffolding** — Created Cargo workspace with core crates.

## Version: 0.1.0-alpha.0

### Added
- **Initial commit** — Project created.

## [Unreleased]

### researchprompt — improved `/research create` synthesis prompt

The `/research create` analysis prompt (in `crates/ragent-research/src/analysis.rs`)
now applies the evidence-based prompt-engineering guidance from
`research/researchanalysis` (20 synthesized findings from 91 web sources):

- **Versioned, composable prompt builder** (`SynthesisPromptBuilder` +
  `SynthesisPromptConfig`) replaces the monolithic `build_synthesis_prompt`
  string concatenation. The legacy free function is preserved as a thin
  wrapper with byte-identical default output, so existing callers are
  unchanged (FR-001, T-002).
- **Mandatory `Sources Cited / Date Spread` paragraph** (FR-003, T-003). When
  enabled, every finding must end with a fifth labeled paragraph listing its
  `[#N]` citations and the earliest/latest publication dates among the cited
  web sources, plus a sentence on how the date range affects confidence. The
  per-source block in the prompt gains a `Published (UTC):` line so the model
  can quote real dates instead of inventing them.
- **Recency-weighting rule** (FR-004, T-004). When enabled, the prompt
  instructs the model to prefer more recently published sources, note
  conflicts between older and newer sources, and down-weight anonymous/undated
  pages.
- **Deterministic mechanical fallback** (FR-005, FR-006, T-005, T-010). The
  `AnalysisEngine` trait now exposes `analyze_with_outcome` returning
  `(AnalysisResult, AnalysisOutcome)`. When the LLM response is empty,
  unparseable, missing required labels, or missing citations, the parser
  returns `AnalysisOutcome::FallbackEmpty` and a mechanically-extracted set
  of findings that always contains at least one finding (with the spec's
  `(findings could not be structured — see below)` placeholder wording and
  the raw model output preserved in a fenced block). `session.rs` maps
  `AnalysisOutcome` to the user-facing `SynthesizeOutcome`, and provider
  errors still surface as `SynthesizeOutcome::FallbackError`.
- **Template merge** (FR-007, T-006). `document.rs::assemble_document`
  clarifies that a `--template` body is MERGED with the standard sections
  (prepended), never a replacement for the Findings section or its four
  required labeled paragraphs. The prompt builder also carries a
  `template_body` knob so the model is told template sections augment the
  structured findings rather than replace them.
- **Few-shot exemplars** (FR-008, T-007). The prompt builder appends up to
  two short exemplar findings (gated on `config.few_shot_examples`) to
  calibrate the exact label structure, `[#N]` citations, and
  `Sources Cited / Date Spread` paragraph.
- **Configurable persona** (FR-009, T-008). `LlmAnalysisEngine::with_persona`
  overrides the default `"You are a careful research analyst..."` system
  message verbatim.
- **Citation/date validation** (FR-010, T-009). On a clean LLM parse, every
  `[#N]` citation is cross-checked against the captured source indices; out
  of-range citations are rewritten inline to `[#N?] (out of range — not in
  source list)` and logged via `tracing::warn`. Explicit publication dates
  inside a `Sources Cited / Date Spread` paragraph that don't match any
  cited source's `published_at` are rewritten to `(unsupported date)`.

### Added — researchprompt configuration surface

- `SynthesisPromptConfig` (`pub(crate)`) with `audience_scope`, `recency_rule`,
  `date_spread_paragraph`, `few_shot_examples`, `persona`, and
  `template_body` knobs.
- `AnalysisOutcome` enum (`Llm`, `FallbackEmpty`, `FallbackError`) re-exported
  from `ragent_research`.
- `SourceBody.published_at: Option<DateTime<Utc>>` field, populated by
  `build_source_bodies` from `Source::published_at`, so the synthesis prompt
  can quote real publication dates.
- `LlmAnalysisEngine::with_persona(Option<String>)` builder.
- `ragent.json` keys documented (SPEC.md → "Research Configuration",
  QUICKSTART.md → "Research prompt configuration"):
  `research.few_shot` (bool) and `research.analysis_persona` (string). Both
  are opt-in; wiring them from config into the engine is tracked as a
  follow-up.

### Tests

- `crates/ragent-research/src/analysis.rs` (inline `mod tests`): +14 tests
  covering the builder (default byte-identity, four required labels,
  date-spread paragraph, recency rule, few-shot append, few-shot cap),
  `parse_analysis_response_with_outcome` (clean Llm, empty/no-findings/
  missing-labels fallback), `mechanical_fallback_findings` (non-empty
  guarantee, placeholder wording), and `validate_citations_and_dates`
  (out-of-range citation, unsupported date, valid-finding untouched).
- `crates/ragent-research/tests/test_template_merge.rs`: 3 tests for FR-007
  template merge (template + standard sections coexist; required labels
  survive merge; no-template regression guard).
- `crates/ragent-research/tests/test_research_create_synthesis.rs`: 3
  integration tests for FR-005/FR-006 (malformed → FallbackEmpty +
  placeholder findings; well-formed → Llm + verbatim findings; Noop → NoLlm
  + mechanical findings).
- `crates/ragent-research/tests/test_research_create_synthesis.rs` (T-012)
  exercises the full `ResearchSession::run` pipeline with mock
  `AnalysisEngine` implementations (no real LLM provider required).

### Verification

- `cargo check --workspace` — green.
- `cargo test -p ragent-research` — 298 lib + 3 template-merge + 3 synthesis
  integration + 1 doc test pass.
- `cargo clippy -p ragent-research --lib` — clean.
- `cargo fmt -p ragent-research -- --check` — clean.
- Default-config prompt is byte-identical to the pre-refactor
  `build_synthesis_prompt` output (regression guard).