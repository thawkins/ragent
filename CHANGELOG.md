# Changelog

## Unreleased

## Version: 1.0.36

### Changed

- Bumped workspace version to 1.0.36.
- `cargo audit` reports 9 pre-existing allowed warnings (unmaintained/unsound
  crates); no new security issues introduced.

### Fixed

- Finance provider selection now surfaces the configured paid provider
  (TwelveData/Alpha Vantage) instead of silently falling back to Yahoo when the
  paid provider fails for a symbol or endpoint. A new `finance.yahoo_fallback`
  option controls this; it defaults to `false` when a paid provider is
  configured, and `true` for the free Yahoo provider. The rate-limit error
  message now mentions TwelveData alongside Alpha Vantage.
- Removed undocumented `#[allow(dead_code)]` const placeholders in
  `crates/ragent-tools-extended/src/finance/tools/mod.rs`, resolving the
  dead-code reason check CI job failure.

## Version: 1.0.35

### Changed

- Bumped workspace version to 1.0.35.
- Updated `h2` crate to 0.4.16 to resolve RUSTSEC-2026-0258.
- `cargo audit` reports 9 pre-existing allowed warnings (unmaintained/unsound
  crates); no new security issues introduced.

## Version: 1.0.34

### Fixed

- Start-of-turn context compaction now uses the same provider-reported input
  token count shown in the TUI status bar. The previous turn's reported usage
  is persisted in the per-session state cache, preventing the trigger from
  falling back to the tool-heavy local estimate and firing when the displayed
  usage percentage is well below the 70 % floor. Emergency overflow and pre-send
  compaction paths also persist the compressed-token estimate so the next turn's
  usage percentage remains accurate after compaction.
- Added debug logging for every pre-send compaction trigger evaluation so users
  can inspect `effective_tokens`, `threshold`, `context_window`, and
  `last_reported_input_tokens` when diagnosing compaction behaviour.

### Changed

- Bumped workspace version to 1.0.34.
- `cargo audit` reports 9 pre-existing allowed warnings (unmaintained/unsound
  crates); no new security issues introduced.

## Version: 1.0.33

### Changed

- Fixed rustfmt formatting in `crates/ragent-tools-core/src/apply_patch.rs`.
- Bumped workspace version to 1.0.33.
- `cargo audit` reports 9 pre-existing allowed warnings (unmaintained/unsound
  crates); no new security issues introduced.

## Version: 1.0.32

### Changed

- Bumped workspace version to 1.0.32.
- `cargo audit` reports 9 pre-existing allowed warnings (unmaintained/unsound
  crates); no new security issues introduced.

## Version: 1.0.31

### Fixed

- Background shell tasks spawned with the `bg` tool now emit
  `BackgroundTaskUpdated` and `BackgroundTaskCompleted` events with the
  correct owning `session_id`, so the TUI Agents panel updates and removes
  rows as tasks finish instead of leaving them stuck in the `running` state.

### Changed

- Removed the legacy `/todo` and `/todos` slash-command aliases. The Tasks
  side panel and task service are now accessed through `/task` (toggle panel
  or subcommands) and `/tasks` (list task items). The TUI internal task-panel
  identifiers (`show_todo`, `todo_area`, `SelectionPane::Todo`,
  `InputAction::ToggleTodo`, etc.) have been renamed to task-specific names.
- Updated `QUICKSTART.md` to point to the Agents panel for background tasks
  and `/task list` for session tasks.

### Fixed — CI clippy and dead-code lint failures

- Fixed `clippy::needless_raw_string_hashes` warnings in
  `crates/ragent-agent/src/goal/mod.rs`, `crates/ragent-agent/src/template/mod.rs`,
  and `crates/ragent-tui/src/app/slash.rs` by removing unnecessary `#` delimiters
  from raw string literals that don't contain double quotes.
- Fixed `clippy::vec_init_then_push` warning in
  `crates/ragent-agent/src/template/mod.rs` by converting `Vec::new()` + `push`
  pattern to a `vec![]` macro.
- Fixed `clippy::useless_borrows_in_formatting` warning in `src/main.rs` by
  removing a redundant `&` borrow on a format argument.
- Added explanatory `// reason:` comments to all 8 undocumented
  `#[allow(dead_code)]` attributes in
  `crates/ragent-agent/src/session/archive.rs` `CronEventExport` struct,
  resolving the dead-code reason check CI job failure.

### Changed

- Bumped workspace version to 1.0.31.
- `cargo audit` reports 9 pre-existing allowed warnings (unmaintained/unsound
  crates); no new security issues introduced.

## Version: 1.0.30

### Added — Panic hook with full log capture (ragent binary)

- Installed a custom `std::panic` hook at the very start of `main` that
  captures every panic and writes a full report to `log/panic-*.log` in
  the project working directory.
- Each panic log includes: UTC timestamp, PID, executable path, working
  directory, panic location (file:line:column), panic message, full
  command-line arguments, `RUST_BACKTRACE` / `RUST_LIB_BACKTRACE`
  environment values, and a complete backtrace captured via
  `std::backtrace::Backtrace::force_capture` (always captured regardless
  of `RUST_BACKTRACE` setting).
- The hook chains to the default Rust panic hook after writing the file,
  preserving stderr output for terminal users.
- Added `src/panic_hook.rs` module with `install()`, `log_dir()`,
  `panic_log_path()`, and `write_panic_log()` functions, plus unit tests
  for path format and log directory resolution.
- Added `chrono` to the root `Cargo.toml` `[dependencies]`.

### Changed — Edit and edit-log improvements (ragent-tools-core)

- Hardened `edit`, `multiedit`, `apply_patch`, and `replace` matching with
  improved fallback cascades, whitespace-flexible matching, and
  indent-normalised retry logic to reduce spurious match failures.
- Enhanced `edit_log` tracking for better auditability of applied edits.
- Expanded test coverage in `test_edit`, `test_edit_integration`,
  `test_edit_smoke`, `test_multiedit`, `test_multiedit_helpers`, and
  `test_apply_patch` to cover the new matching behaviour.

### Changed — Research system (ragent-research)

- Added new hyperresearch modules: `chapter`, `cite_checker`,
  `contradiction`, `corpus_critic`, `digest`, `locus`, `open_access`,
  `patcher`, `readability`, `reconcile`, `run_manifest`, `source_vault`,
  `synthesis`, and `tier_router`.
- Extended `cli`, `document`, `session`, `web_gatherer`, `source`,
  `manager`, `run_config`, and `item` with tier-based research pipeline
  support.
- Added `test_hyperresearch_manual` and `source_vault` tests.

### Changed — Configuration (ragent-config)

- Added `research.open_access_recovery` and `research.contact_email`
  config fields.
- Added `test_research_config` test.

### Changed — Other

- Updated `openai_responses` provider handling.
- Updated `ragent-server` routes and `ragent-storage` storage helpers.
- Updated TUI research progress rendering and tests.
- Removed obsolete docs: `CODE_QUALITY_IMPLEMENTED.md`,
  `edit-matching-improvements.md`, `O365_TOOL.md`, and
  `research-options-wiring-plan.md`.
- Added `docs/howtos/research.md`.
- Bumped workspace version to 1.0.30.
- `cargo audit` reports 9 pre-existing allowed warnings (unmaintained/unsound
  crates); no new security issues introduced.

## Version: 1.0.29

### Added — Hyperresearch integration

- `/research --tier light|full|dissertation` selects research depth; default is `full` (T-001, FR-001).
- `RunManifest` with `RunStep`/`StepStatus` tracks every run step and supports resuming an interrupted run via `/research resume <run_tag>` (T-002, FR-007).
- Persistent source vault under `.ragent/research_vault/<run_tag>/` with SQLite index and raw content files; vault sources are reused before any new web search (T-003, T-004, FR-002, FR-003, FR-009).
- Tier router implements the full 16-step pipeline for `full`, a trimmed pipeline for `light`, and dissertation chaptering for `dissertation` (T-005, FR-005, FR-008, FR-013).
- Width sweep aggregates results from all configured `mf_search` backends in parallel (T-006).
- Deterministic contradiction graph and cross-locus reconcile/source tensions are rendered in `RESEARCH.md` (T-007, T-009).
- Deterministic loci analysis and depth investigation surface recurring dimensions and shallow evidence (T-008).
- Deterministic corpus critic, gap-fill fetch, and surgical patcher refine the draft before citation checking (T-010, T-013).
- Deterministic evidence digest and triple draft feed synthesis and a 4-critic audit (T-011, T-012).
- Citation checker verifies every `[#N]` marker and closes the failure gate with `CITATION_VERIFICATION_FAILED` markers when a source is unsupported (T-014, FR-006, FR-014).
- Deterministic polish and readability audit run before final assembly (T-015).
- Open-access recovery via Unpaywall and Europe PMC, with source license/version disclosure in `RESEARCH.md` frontmatter and supporting-file notes (T-017, T-019, FR-010, FR-015).
- `research.open_access_recovery` and `research.contact_email` config fields (T-018, FR-011, FR-012).
- Sufficient-source check skips web search when the vault already holds enough sources for the requested tier (T-021, FR-016).

### Tests — Hyperresearch integration

- Added `crates/ragent-research/tests/test_hyperresearch_manual.rs` with manual verification test cases covering tier step selection, resume-from-manifest, rendered contradiction/tensions/cite-check sections, sufficient-source skip, and OA-recovery disclosure.

### Changed

- Bumped workspace version to 1.0.29.
- `cargo audit` reports 9 pre-existing allowed warnings (unmaintained/unsound crates); no new security issues introduced.

## Version: 1.0.28-beta

### Added — Spec-Driven Development (SDD) back-fill

Back-fills missing Spec-Driven Development capabilities from GitHub's
`spec-kit/spec-driven.md` into ragent's `/spec` feature set. All new
capabilities are opt-in via configuration flags (FR-019) and backward
compatible with existing spec directories (FR-018). See
`specs/reqeng/SPEC.md` for the full specification and `specs/reqeng/PLAN.md`
for the implementation plan with gap-resolution tracking (FR-020).

#### New `/spec` subcommands (ragent-specs)

- **`/spec specify <specname> <feature>`** — New `SpecCommand::Specify`
  variant and parser (T-001, FR-001). Creates a `SPEC.md` with structured
  requirements, user stories, and acceptance criteria without
  simultaneously generating a `PLAN.md`, separating the specification
  stage from the planning stage.
- **`/spec plan <spec-id> <tech-context>`** — New `SpecCommand::Plan`
  variant with technology-context argument (T-003, FR-004). Generates
  (or regenerates) `PLAN.md` from an existing `SPEC.md` using the
  provided technology context as guidance. Coexists with the existing
  `/spec update` and `/spec add` commands.
- **`/spec tasks <spec-id>`** — New `SpecCommand::Tasks` variant and
  parser (T-005, FR-005). Generates a `TASKS.md` file containing an
  ordered task list derived from the existing `PLAN.md`.

#### `[NEEDS CLARIFICATION]` marker support (ragent-specs)

- New `detect_clarification_markers` function and `ClarificationMarker`
  struct (T-007, FR-002). Case-insensitive regex detects
  `[NEEDS CLARIFICATION: <question>]` markers in `SPEC.md` content and
  returns each with its 1-based line number and captured question text.
- New validation `Category::Clarification` for reporting unresolved
  clarification markers.

#### Quality checklists in templates (ragent-specs)

- `SpecTemplate::generate_with_checklist` (T-010, FR-006) — Embeds an
  optional `## Quality Checklist` section in `SPEC.md` covering
  requirement completeness, testability, and absence of speculative
  features. Existing `generate` and `generate_with_research` delegate
  with `include_checklist = false`, preserving existing output.
- `PlanTemplate::generate_with_checklist` (T-011, FR-006) — Embeds an
  optional `## Quality Checklist` section in `PLAN.md` covering
  requirement traceability, testability, and absence of speculative
  tasks. Existing `generate` delegates with `include_checklist = false`.

#### Constitution artifact (ragent-specs)

- New `crates/ragent-specs/src/constitution.rs` module (T-013, FR-007)
  implementing `Constitution`, `Article`, and `Amendment` structs with
  `parse_constitution` parser. Parses `CONSTITUTION.md` files containing
  immutable architectural principles (`## Article N: Title` headings)
  and an optional `## Amendment Log` table. `Constitution::empty()`
  returns a no-articles value for backward compatibility (FR-018).
- Exported `Constitution`, `Article`, `Amendment`, and
  `parse_constitution` from the crate root.

#### Consistency validation — ambiguity detection (ragent-specs)

- New `detect_ambiguity` function, `AmbiguityIssue` struct, and
  `AmbiguityKind` enum (T-026, FR-015). Detects vague terms (e.g.,
  "maybe", "possibly", "might") and undefined cross-references in
  `SPEC.md` content.
- New validation `Category::Ambiguity` for reporting ambiguous language.

#### `FEEDBACK.md` file support (ragent-specs)

- New `FeedbackTemplate` struct with `generate(title)` method (T-031,
  FR-017). Produces a default `FEEDBACK.md` template with a feedback
  notes table (Date | Source | Note) and an advisory notice.
- `Spec` struct gains a `feedback_md: String` field and
  `feedback_md_path()` method. `SpecIo::discover_specs`, `read_spec`,
  and `write_spec` load and persist `FEEDBACK.md` following the same
  pattern as `REVIEW.md` — loaded if present, written only when
  non-empty (FR-018 backward compatibility).
- Exported `FeedbackTemplate` from the crate root.

#### SDD configuration flags (ragent-config)

- New `SddConfig` struct (T-035, FR-019) with 13 opt-in boolean flags
  gating SDD capabilities: `clarification_markers` (FR-002),
  `quality_checklists` (FR-006), `constitution` (FR-007),
  `phase_minus_one_gates` (FR-008), `branch_per_spec` (FR-009),
  `research_artifacts` (FR-010), `data_model` (FR-011), `contracts`
  (FR-012), `quickstart` (FR-013), `test_first_ordering` (FR-014),
  `consistency_checks` (FR-015), `amendment_process` (FR-016), and
  `feedback_loop` (FR-017).
- All flags default to `false` (opt-in). `SddConfig::merge` uses OR
  semantics — a flag enabled in either base or overlay stays enabled.
- Serialized under the `sdd` key in `ragent.json` with
  `skip_serializing_if` on each field; an all-false `sdd` block is
  omitted entirely from serialized output.
- `Config::merge` wires `base.sdd.merge(&overlay.sdd)`.
- 12 integration tests in
  `crates/ragent-config/tests/test_sdd_config.rs` covering defaults,
  parsing, serialization, and merge semantics.
- Exported `SddConfig` from the `ragent-config` crate root.

#### Gap resolution tracking (specs/reqeng)

- `specs/reqeng/PLAN.md` Gap Resolution Tracking section enhanced with
  a Status column (T-037, FR-020) showing per-gap resolution state
  (✅ Resolved / ⏳ Partial / ⬜ Not started), a progress summary, and a
  completed-task summary table linking each completed task to its gap
  and deliverable.

### Changed — ragent-tui slash command handling

- `crates/ragent-tui/src/app/slash.rs` match arms extended to cover
  `SpecCommand::Specify`, `SpecCommand::Plan`, and `SpecCommand::Tasks`
  variants, fixing a non-exhaustive match that would have prevented
  compilation once the new variants were added.

### Tests — SDD back-fill

- `crates/ragent-specs/tests/test_templates.rs` — 10 new tests for
  `SpecTemplate::generate_with_checklist`, `PlanTemplate::generate_with_checklist`,
  and `FeedbackTemplate::generate`.
- `crates/ragent-specs/tests/test_spec_io.rs` — 7 new tests for
  `FEEDBACK.md` load, read, write, discover, and path resolution.
- `crates/ragent-specs/tests/test_slash_spec.rs` — 6 new tests for
  `SpecCommand::Specify`, `SpecCommand::Plan`, and `SpecCommand::Tasks`
  parsing and help text.
- `crates/ragent-specs/tests/inline/validate.rs` — 12 new tests for
  `detect_clarification_markers` and `detect_ambiguity`.
- `crates/ragent-config/tests/test_sdd_config.rs` — 12 new tests for
  `SddConfig` defaults, parsing, serialization, and merge semantics.
- `crates/ragent-specs/src/constitution.rs` — 13 inline tests for
  constitution parsing (articles, amendments, em-dash headings,
  multiline bodies, path resolution).

## Version: 1.0.27

### Added — Exa Search API backend for `mf_search`

- New **Exa** search engine added to `mf_search`, configured via
  `exa_api_key` in `ragent.json` (global or project) or the
  `EXA_API_KEY` environment variable; the environment variable takes
  precedence. The key is masked in diagnostics and never logged.
- New module
  `crates/ragent-tools-extended/src/masterfetch/search/exa.rs`
  implementing the `ExaEngine` backend.
- The `mf_search` `engine` parameter now accepts `"exa"` to restrict
  searches to the Exa backend only. The JSON schema `engine` enum and
  tool description updated to include `exa`.
- `MfSearchTool::resolve_search_keys` now returns a 4-tuple including
  the Exa key; `build_orchestrator` and `engine_status` wired to
  include the Exa engine when a key is present.
- `Config::exa_api_key` field added to `ragent-config` with
  `#[serde(default, skip_serializing_if = "Option::is_none")]` and
  merge support in `Config::merge`.
- New tests `test_mf_exa.rs` and extended `test_mf_search_tool.rs`
  covering orchestrator wiring, engine selection, engine status, and
  schema enum for Exa.

### Changed — Research document `Search Engine Summary` section

- `crates/ragent-research/src/document.rs` now renders a
  **Search Engine Summary** table in `RESEARCH.md` (both Report and
  IMRaD layouts) showing, per backend engine, the number of web
  sources acquired broken down by media type (pages, PDFs, videos).
  The section is emitted only when at least one web source has a
  non-empty `search_engine` field, so skeletons and pre-gathering
  documents remain unchanged.
- `crates/ragent-research/src/source.rs` gains a
  `Source::search_engine()` accessor returning the comma-separated
  backend engine list for `Source::Web` (empty string for other
  variants).
- New unit tests for `render_search_engine_summary` covering
  multi-engine source splitting, media-type counting, empty-state,
  and section ordering in both Report and IMRaD layouts.

### Changed — Documentation and statistics

- README.md updated to mention Exa in the MasterFetch feature list,
  recent highlights, and `engine` parameter enum.
- STATS.md updated with current line/file/test counts reflecting the
  new Exa module and research document changes.

## Version: 1.0.26

### Added — OpenAlex and Wikipedia search backends for `mf_search`

- New **OpenAlex** keyless search backend queries the scholarly-works
  catalog (papers, articles, datasets). Results include title, authors,
  publication year, venue, citation count, open-access URL, and relevance
  score. Set `OPENALEX_EMAIL` in the environment or `ragent.json` to join
  the polite pool.
- New **Wikipedia** keyless search backend queries the English Wikipedia
  REST API for encyclopedia-style summaries. Results include the page
  title, extract, and canonical URL.
- Both backends run in parallel with the existing DuckDuckGo and Brave
  engines by default; optional LangSearch / Tavily / Perplexity
  API-backed engines continue to be supported when configured.
- The `mf_search` tool gains an `engine` parameter to restrict the
  search to a single backend
  (`duckduckgo` / `brave` / `openalex` / `wikipedia` / `langsearch` /
  `tavily` / `perplexity`).
- New modules `crates/ragent-tools-extended/src/masterfetch/search/openalex.rs`
  and `.../wikipedia.rs` implementing the backends.
- New tests `test_mf_openalex.rs`, `test_mf_openalex_live.rs`,
  `test_mf_wikipedia.rs`, `test_mf_wikipedia_live.rs` covering unit and
  live integration paths.

### Changed — Search consensus and relevancy adjustments

- The consensus merge in `mf_search` now weights per-engine relevance
  scores and `fetch_relevance` signals more evenly, reducing
  DuckDuckGo-dominated ranking when multiple backends contribute.
- `mf_search` `engine.rs` and `search/mod.rs` refactored to share a
  common `SearchResult` builder path so all backends produce consistent
  field sets (title, url, snippet, score, source engine, extra metadata).
- The search-tool JSON schema now documents the `engine` enum and
  the `per_engine_results` cap.

### Changed — Research web-gatherer enhancements

- `crates/ragent-research/src/web_gatherer.rs` extended to handle the
  new backend result shapes and to pass through OpenAlex/Wikipedia
  metadata into `RESEARCH.md` source entries.
- `crates/ragent-research/src/cli.rs` and `session.rs` updated for the
  new gatherer flow.
- `crates/ragent-server/src/routes/research.rs` and
  `crates/ragent-tui/src/app/research.rs` wired to the updated research
  session.
- New `--use-low-relevance` style flags consolidated in the research
  CLI.

### Fixed — Compaction loop and processor stability

- `crates/ragent-agent/src/session/loop_steps.rs` simplified (removed
  ~100 lines of duplicated nudge logic) to fix repeated
  post-compaction continuation nudges across loop iterations.
- `crates/ragent-agent/src/session/processor.rs` updated to thread the
  `last_task_completed_at` guard so autopilot auto-continue is
  suppressed after `agent_complete`.
- `crates/ragent-agent/tests/test_compaction_integration.rs` updated
  to match the simplified loop.

### Changed — Config and CLI

- `crates/ragent-config/src/config.rs` extended with the new search
  backend option fields.
- `src/cli.rs` and `crates/ragent-research/src/cli.rs` updated for
  the new research/search flags.

## Version: 1.0.25

### Fixed — cargo-deny CI and security audit

- Add RUSTSEC-2026-0253 (lru `LruCache::pop()` use-after-free) to `deny.toml`
  `[advisories].ignore` — transitive via ratatui 0.29 / tantivy 0.22; ragent does
  not call `pop()`; upgrade blocked by ratatui's lru 0.12 pin
- Add `--ignore RUSTSEC-2026-0253` to the `cargo audit` step in
  `security-audit.yml`, keeping it in sync with `deny.toml`
- Pin `cargo-deny` to `0.18.12` in the CI workflow to avoid
  `bug[unresolved-workspace-dependency]` false-positives that appeared in later
  cargo-deny versions when resolving root-crate `{ workspace = true }` deps

## Version: 1.0.24

### Added — `/spec update` subcommand and TESTPLAN.md artifact

- New `/spec update <spec-id>` sub-command re-reads the existing
  `specs/<spec-id>/SPEC.md` and regenerates `PLAN.md` and `TESTPLAN.md`
  to match the current requirements. `SPEC.md` is not modified. Existing
  task IDs in `PLAN.md` are preserved where unchanged.
  - Validates the spec ID and that the spec directory / `SPEC.md` exist.
  - Guards against updating archived specs (returns an error).
  - On not-found, lists available specs to aid discovery.
  - Delegates to the `explore` agent by default, falling back to the
    current agent. Dispatches generation via `process_message` and
    publishes `AgentError` on failure.
  - `SpecCommand::Update` variant added to `crates/ragent-specs` with
    `build_update_status`, `build_update_message`, `build_update_log`,
    and `build_update_prompt` helpers.
- `/spec create` now generates a third artefact, `TESTPLAN.md`, alongside
  `SPEC.md` and `PLAN.md` in every new spec directory. The file is a
  human-readable **manual** test plan with a `## Test Cases` section, each
  test case having a `TC-NNN` ID, title, preconditions, step-by-step
  instructions, test data to enter, and expected results. When the feature
  involves UI navigation, the plan enumerates every navigation step and
  the exact data to enter. It may optionally include `## Prerequisites`
  and `## Cleanup` sections. It does **not** contain automated test code,
  `#[test]` functions, or `cargo test` references.
- The `/spec create` status message, user-facing message, and log entry
  now mention `TESTPLAN.md` so testers know a manual test plan was
  produced.
- `/spec add` now runs a second phase after the incremental requirement
  addition that fully regenerates `PLAN.md` and `TESTPLAN.md` from the
  updated `SPEC.md` (same logic as `/spec update`), so add operations stay
  consistent with the latest requirements.
- Tests added for the new behaviour: `test_slash_spec_update_missing_spec_id_shows_usage_error`,
  `test_spec_jtbd`, and `TESTPLAN.md` assertions in the create test.

### Changed — AGENTS.md project guidelines

- Added rule 6: **No unsafe code.**
- Added rule 7: **No `.unwrap()` on user-facing paths.**

### Removed — Stale planning files

- Deleted obsolete root planning documents that were superseded by the
  `specs/` workflow: `ALPLAN.md`, `CUTPLAN.md`, `EDITPLAN.md`,
  `RESEARCHPLAN.md`, `TOOLPLAN.md`.
- Removed a stale unversioned `build` artifact from the repository root.

## Version: 1.0.23

### Added — Jobs-To-Be-Done analysis for specs

- New `/spec jtbd <specname>` sub-command performs a Jobs-To-Be-Done
  analysis of an existing spec's `SPEC.md` and writes the result to
  `specs/<specname>/JTBD.md`. Supports `--force` to overwrite an existing
  analysis and `--agent <name>` to select the analysis agent.
- Added parsing, dispatch, and tests for the new `Jtbd` spec command
  variant in `crates/ragent-specs`.

### Changed — Mandatory readability extraction in the research web-gather phase

- Every HTML page captured as a research web source must now have been
  extracted by the `readability-rs` crate. Pages where readability fails
  — and would previously have been accepted via the silent html2text /
  raw tag-strip fallbacks — are rejected with a
  `readability extraction failed …` fetch error and skipped by the
  gatherer. PDF and YouTube sources bypass readability by design and are
  unaffected.
- `mf_fetch` now reports which stage of the extraction chain produced a
  page via a new `extraction_method` metadata signal
  (`readability` / `html2text` / `raw_text` / …); the signal is also
  recorded in the masterfetch content cache (new
  `fetch_cache.extraction_method` column) so cached responses keep it.
- The legacy `webfetch` path (which does not report the extraction
  stage) is verified by re-running `readability-rs` directly on the raw
  HTML so the guarantee is enforced rather than trusted.

### Fixed — YouTube transcript capture in research web-gather

- `mf_fetch` now parses real YouTube watch pages correctly: the
  `ytInitialPlayerResponse` object is extracted with a brace-balanced,
  string-aware scanner instead of the previous `(\{.*?\});` regex (which
  broke on nested braces and on `}` characters inside JSON strings), and
  the caption track list is read from the real
  `captions.playerCaptionsTracklistRenderer.captionTracks` location
  (with the legacy flat `captions.captionTracks` layout kept as a
  fallback). Watch-page transcription data is therefore actually
  recovered instead of erroring on every real video page.
- Failed fetches reported by `mf_fetch` metadata (`error` field or
  `content_ok = false` — e.g. "no caption tracks available for this
  YouTube video") now abort the research fetch adapter with an explicit
  error. The gatherer surfaces them as a `FetchFailed` event carrying
  the real reason and suppresses the video outright; previously the
  placeholder `[YouTube transcript extraction failed: …]` text was kept
  as the page body and silently suppressed by the
  "extracted content too short" gate, hiding why videos were dropped.

## Version: 1.0.22

### Fixed — Time-sensitive `test_parse_natural_time_5pm_tomorrow` CI failure

- Replaced the fragile assertion `parsed.next_due > now + 20 hours` in
  `crates/ragent-types/src/cron.rs` with a robust date-based check.
  The old assertion failed when CI ran after ~3pm UTC because "5pm
  tomorrow" could be as little as ~18 hours away. The new assertions
  verify that the result is in the future and on a later calendar day,
  which is what "tomorrow" actually means.

## Version: 1.0.21

### Fixed — CI clippy failure

- Added `#[allow(clippy::too_many_arguments)]` to
  `log_cron_execution` in `crates/ragent-tools-core/src/cron_log.rs`.
  The function takes 8 arguments (clippy's default threshold is 7),
  which caused the CI clippy gate (`-D warnings`) to fail on the
  v1.0.20 release commit.

## Version: 1.0.20

### Added — LLM-callable cron tools

- New `cron_add`, `cron_remove`, `cron_list`, `cron_enable`, and
  `cron_disable` tools registered in the default tool registry, giving
  the model direct access to the cron scheduler without going through
  the TUI slash-command surface.
- All cron tools read/write through the `Storage` layer, mirroring the
  `/cron` slash-command handlers.
- TUI log-panel rendering added for cron tool inputs and results
  (⏰ summaries in `message_widget`).

### Added — `/cron` slash-command enhancements

- `/cron add` now takes a positional `cronname` as the first argument:
  `/cron add <cronname> <agent> <schedule> "<prompt>"`. The cronname
  becomes the event ID.
- New `/cron enable <event_id>` and `/cron disable <event_id>` commands
  to toggle events without removing them.
- New `/cron detail <event_id>` command showing every stored field
  including the full, untruncated prompt.
- `/cron help` updated with parameter tables, schedule examples, and
  natural-language timestamp documentation.

### Added — Natural-language timestamp parsing

- `parse_timestamp` now accepts human-friendly time shortcuts resolved
  against the user's local timezone in addition to ISO-8601:
  - `5pm` / `5PM` — next 5pm (today or tomorrow)
  - `5:30pm` / `5:30 pm` — 12-hour clock with minutes
  - `17:00` — 24-hour clock
  - `5am tomorrow` / `5pm today` — explicit day offset
- 12-hour edge cases handled: `12pm` = noon, `12am` = midnight, `13pm`
  rejected as invalid.
- Added 13 unit tests covering natural-language parsing, case
  insensitivity, 24-hour conversion, and invalid inputs.

### Fixed — Sub-agent and background-agent model resolution

- Background agents and sub-agents (`new_agent`) now use the user's
  persisted `selected_model` setting from Storage instead of falling
  back to `Config::default()` and `resolve_default_model`, which
  typically picked Anthropic even when no API key was configured.
- Agent resolution now uses `resolve_agent_with_customs_and_model` with
  the cached config (`load_config_cached`) and the provider registry,
  matching the TUI's model resolution path.
- Explicit model overrides (`--model` / `model` parameter) still take
  precedence; the persisted setting is only applied when no override is
  present and the agent's model is not pinned.

### Tests

- `crates/ragent-agent/tests/test_cron_tools.rs` — 6 tests covering the
  LLM-callable cron tool surface.
- `crates/ragent-tui/tests/test_cron_add_positional.rs` — tests for the
  positional `/cron add <cronname> ...` parser.
- 13 new natural-language timestamp tests in `ragent-types/src/cron.rs`.

## Version: 1.0.19

### Added — Cron capability

- Added agent cron scheduling system (`/cron` slash-command family) with
  one-shot (`at <timestamp>`), repeating (`from <timestamp> every
  <duration>`), and interval (`every <duration>`) schedule forms.
- `cron_events` table persisted to SQLite via the existing `Storage` layer
  with migration support.
- Background scheduler ticks every 30 seconds, evaluating and firing
  enabled events whose `next_due` has passed.
- Execution outcomes (`success`, `error`, `skipped`) logged as JSONL to
  `<working_dir>/log/cron-<timestamp>.jsonl`.
- Duration parser supporting `m`, `h`, `d`, `w`, `mo` units with
  plural/long-form aliases.
- Comprehensive unit test coverage: duration parser (21 tests), schedule
  parser (53 tests), storage round-trip (12 tests), `every` no-start
  computation (6 tests), and past-start advancement (14 tests).

## Version: 1.0.18

### Added — Agent cron system (`/cron`)

- New cron scheduling system that lets users schedule agent runs with a
  designated agent type and an initial prompt.
- Three schedule forms supported:
  - `at <timestamp>` — one-shot, fires once at the specified time.
  - `from <timestamp> every <duration>` — repeating, first fire at the
    given timestamp, then at each interval.
  - `every <duration>` — repeating with no explicit start; first fire is
    `duration` from now.
- Duration parser supports `m` (minutes), `h` (hours), `d` (days),
  `w` (weeks), `mo` (months = 30 days) with plural/long-form aliases
  (`mins`, `hrs`, `days`, `wks`, `months`).
- Events persisted to SQLite via the existing `Storage` layer; `cron_events`
  table with migration.
- Background scheduler ticks every 30 seconds on a non-blocking tokio
  task while the TUI session is running, evaluates all enabled events,
  and fires those whose `next_due` has passed.
- `/cron` slash-command family:
  - `/cron add <agent> <schedule> "<prompt>"` — schedule a new event.
  - `/cron remove <event_id>` — remove a scheduled event.
  - `/cron list` — list all events with human-readable schedule
    descriptions.
  - `/cron log [event_id]` — show execution log (optionally filtered).
  - `/cron help` — show usage.
- Every execution is logged as a JSONL line to
  `<working_dir>/log/cron-<timestamp>.jsonl`, mirroring the edit-log
  convention. Each entry records event id, agent type, prompt, outcome
  (`"success"`, `"error"`, or `"skipped"`), and timestamp.
- Disabled events are skipped with a `"skipped"` outcome.
- Past-start timestamps for `from <ts> every <d>` are advanced by whole
  duration intervals until `next_due` is in the future.
- Added unit tests for duration parser (21 tests), schedule parser
  (53 tests), storage round-trip (12 tests), `every <d>` no-start
  `next_due` computation (6 tests), and `from <past> every <d>` past-start
  advancement (14 tests).

### Added — Perplexity Sonar backend for `mf_search`

- Added a new `perplexity` search backend (`PerplexityEngine`) that queries the
  Perplexity Sonar API, wired into the MasterFetch search orchestrator alongside
  the existing DuckDuckGo, Brave, LangSearch, and Tavily engines.
- New `perplexity_api_key` field in `ragent_config::Config` (with merge support
  across global/project config files). When present (or when the
  `PERPLEXITY_API_KEY` environment variable is set), `mf_search` includes the
  Perplexity backend as an additional engine. The key is masked in diagnostics
  and never logged.
- Added integration tests for the Perplexity backend wiring and API-key
  resolution.

### Added — Edit-log per-tool success/failure analysis

- `EditLogAnalysis` now tracks per-tool success and failure counts via
  `success_by_tool` and `failure_by_tool` maps.
- New `tools_sorted()` helper returns all tools that have logged operations.
- New `failure_success_ratio_pct_for(tool)` helper computes the failed-to-succeeded
  ratio as a percentage for a given tool (returns `0.0` when the tool has no
  succeeded operations).
- The `edit_log_analyse` function now counts successful operations (previously
  skipped) and records them per tool, enabling richer edit-log summaries.
- Added tests verifying per-tool counts and ratio calculations.

### Changed — Miscellaneous

- Removed unused setup code from TUI app initialization (`init.rs`).
- TUI slash-command and tool-display rendering updated for the new search
  backend and edit-log analysis fields.
- Updated existing `mf_search` tool tests for the new three-tuple key
  resolution signature.

## Version: 1.0.17

### Added — Optional `collapse_whitespace` matching for `edit` / `multi_edit`

- The `edit` and `multi_edit` tools now accept an optional
  `collapse_whitespace` boolean (default `false`, i.e. the existing
  byte-for-byte strict matcher is unchanged). When `true`, backslash escapes
  (`\t`, `\n`, `\r`, `\\`) in `old_string` are decoded and every run of
  whitespace matches a non-empty run of whitespace in the file, so collapsed
  indentation or alignment whitespace no longer causes spurious "old_string
  not found" failures. Uniqueness and the whole-batch atomicity of
  `multi_edit` are preserved in flexible mode.
- New public helpers in `ragent_tools_core::replace`: `decode_escapes` and
  `find_flexible_replacement_range` (two-lane matching: exact lane wins when
  unique; whitespace-tolerant scan otherwise; ambiguous hits rejected).
- `edit` result metadata now includes `collapse_whitespace: true` when the
  relaxed matcher was used, and failure messages mention the flexible mode.

### Added — Persistent edit-log toggle with Alt+E and status-bar indicator

- Added `edit_log` boolean field to `ragent_config::Config`; defaults to `false`
  and merges across global/project config files.
- New `ragent_config::edit_log` module provides `is_enabled`, `set_enabled`,
  `sync_from_config`, `persist_edit_log`, and `toggle_persist` — mirroring the
  YOLO persistence helpers.
- `ragent_tools_core::edit_log::is_edit_log_enabled` now delegates to the
  shared `ragent_config::edit_log` runtime flag, ensuring the TUI, tools, and
  status bar all see the same state.
- TUI `Alt+E` toggles edit logging; the new state is persisted to
  `.ragent/ragent.json` and a status/log message confirms the change.
- TUI `/editlog on|off` now persists the new state to `ragent.json` instead of
  changing the runtime flag only.
- Status bar line 2 shows an `EditLog:{✓|✗}` indicator next to `AutoPilot`,
  reflecting the current persisted state.
- Added tests for `Alt+E` toggle + indicator and `/editlog` persistence.

### Fixed — Clippy warnings across workspace

- Removed unused underscore-prefixed bindings in `session_ops.rs`.
- Added `#![allow(clippy::redundant_pub_crate)]` to `crates/ragent-tui/src/app/tests.rs`.
- Allowed `clippy::await_holding_lock` on edit-log integration tests that
  intentionally serialise process-wide state with a static mutex.
- Reduced `LocalEmbeddingInner::Ready` variant size by boxing the tokenizer.
- Collapsed nested `if let` in `LocalEmbeddingProvider::embed`.
- Added `#[allow(dead_code)]` to edit-log analysis APIs that are public but not
  yet consumed in every compilation unit.

### Fixed — Repeated "Context compression skipped" notices

- Added a per-turn `compaction_attempted_this_turn` flag to `LoopState` and the
  agent-loop orchestrator.
- The pre-send compaction path now sets the flag before invoking the runner and
  suppresses further compaction attempts for the rest of the turn, even when the
  runner bails with a "Context compression skipped" notice.
- Both emergency-overflow compaction paths now use the same flag, so a skipped
  emergency compaction is not retried either.
- `compressed_this_turn` is still tracked separately so the post-compaction
  continuation nudge only fires after a successful compression.
- Added `test_pre_send_compaction_skipped_notice_emitted_once_per_turn` to
  verify that only one skipped notice is published per turn.

### Changed — Model-independent context-compaction trigger

- `CompactionConfig::default().threshold` is now `0.7` (70 % of the model's
  context window) instead of `None`. This makes the default pre-send compaction
  trigger independent of the model's absolute context size.
- `CompactionConfig::default().buffer` is now a fraction of the context window
  (`0.10`, i.e. 10 %) instead of an absolute `20_000` tokens.
- `CompactionConfig::default().keep.tokens` is now a fraction of the context
  window (`0.20`, i.e. 20 %) instead of an absolute `8_000` tokens.
- `compaction_threshold()` and `select()` compute absolute token budgets from
  these fractions using the resolved `context_window`, so compaction behavior
  scales automatically with the model in use.
- The agent-loop pre-send estimator (`compaction_threshold`) now enforces a
  70 % context-window floor, so automatic compaction never fires on routine
  prompts that fill less than 70 % of the available context.
- The TUI pre-send compaction check (`should_auto_compact_before_send`) now
  uses the shared `compaction_threshold()` estimator instead of a private 92 %
  fallback, keeping the TUI and server/agent paths consistent.
- Updated `SPEC.md`, `QUICKSTART.md`, and `README.md` compaction examples to
  show the new `threshold`, `buffer`, and `keep.tokens` fraction fields.
- Adjusted compaction config and runner tests to reflect fraction-based
  defaults and the fact that legacy `compression.auto_threshold` only fills an
  explicit `threshold: null`.

## Version: 1.0.15

### Fixed — CI formatting and multi_edit edit-log tests

- Ran `cargo fmt` to fix formatting in `crates/ragent-tools-core/tests/test_multiedit.rs`.
- Fix flaky `multi_edit` edit-log tests (commit `3fb331c`).
- Incremental release bump to 1.0.15.

## Version: 1.0.14

### Changed — version bump and security audit

- Incremental release bump to 1.0.14.
- `cargo audit` reports 7 allowed warnings (unmaintained/unsound crates) that are
  suppressed via existing project policy; no new high-severity advisories introduced.

## Version: 1.0.13

### Added — Edit-operation audit logging

- New `ragent-tools-core/src/edit_log.rs` provides `log_edit_operation`, a
  process-wide atomic `EDIT_LOG_ENABLED` flag, and helpers `is_edit_log_enabled`,
  `set_edit_log_enabled`, and `clear_edit_logs`.
- `edit` and `multi_edit` now log every outcome path (success, dry-run preview,
  no-change rejection, create/mkdir/write errors, stale-file rejection, and
  match failures) to `<working_dir>/log/edits-<timestamp>.jsonl` when enabled.
- TUI `/editlog` slash command added with subcommands `on|off|status|show|help`.
- `EditTool` and `MultiEditTool` instrument all branches; `create_file` now
  receives the `dry_run` flag so create paths are also logged.
- Tests added for successful edits, dry-run previews, failures, disabled logging,
  and multi_edit batch logging.

## Version: 1.0.12

### Changed — update security audits

- Incremental release bump; security audit clean via `.cargo/audit.toml`
  suppressions for advisories blocked by upstream compatibility.

### Changed — research finding heading format

- `/research` `RESEARCH.md` findings now render with an extra blank line above,
  a bold finding number (`### **Finding N** —`), and the same `###` heading
  level to preserve outline structure.

## Version: 1.0.11

### Changed — update research system

- Incremental release bump and changelog update for the research subsystem.

### Security — audited and accepted transitive dependency advisories

- Ran `cargo audit`; 7 high-severity advisories were reported. After review,
  the following are suppressed in `deny.toml` because a fix requires major
  upstream crate upgrades that are out of scope for this release:
  - `RUSTSEC-2026-0187` (`lopdf` stack overflow): blocked by `pdf-extract`
    0.10 and `printpdf` 0.9; trusted local PDF input.
  - `RUSTSEC-2026-0194` / `RUSTSEC-2026-0195` (`quick-xml` DoS): blocked by
    `ooxmlsdk` 0.3 and `calamine` 0.34; trusted local Office/Spreadsheet input.
  - `RUSTSEC-2026-0235` (`rkyv` archive validation): blocked by
    `rust_decimal` 1.42.1 via `spreadsheet-ods`; not used with untrusted input.

## Version: 1.0.10

### Changed — updated /research functionality

- `ragent-research` crate updated with improved synthesis, gathering, and session
  pipeline changes (see details below).

## Version: 1.0.9

### Added — Low-relevance web-source filter in `/research` gathering

- `WebGatherer` now computes a deterministic relevance label for every
  fetched candidate and drops sources labelled **Low** or **Very low** before
  they are added to the research state.
- The relevance score is based only on the query, title, snippet, and URL, so
  it adds zero LLM cost. Common English stopwords are stripped from the query
  so question-style queries (e.g. "What is Rust?") are not penalised for
  missing auxiliary words.
- Skipped sources still emit a `GatherEvent::FetchFailed` diagnostic with the
  reason `relevance too low (...)`, preserving the index slot so `web-NN.md`
  numbering remains stable.
- Added unit test `gather_filters_low_relevance_hits_and_notifies_observer`.

### Fixed — Mermaid findings diagram now renders when labels contain quotes or backticks

- `crates/ragent-research/src/diagram.rs::escape_mermaid_label` now replaces
  both `"` and `` ` `` with `'` instead of trying to escape them.
  Mermaid's double-quoted node labels (`F["..."]`) do **not** support
  backslash-escaped quotes or backticks, so the previous `\"` output and any
  inline code spans caused a parser error (visible in GitHub/GitLab previews and
  `mermaid-cli`).
- Updated the quote-escaping unit test and added a new regression test covering
  backtick-laden labels such as `` `rlms` ``.
- Verified the generated diagram renders with `@mermaid-js/mermaid-cli`.

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

## Version: 1.0.8

### Added

- macOS Apple Silicon (M-series) packaging support.
  - New `scripts/macos-pkg.sh` builds a flat `.pkg` installer from an `aarch64-apple-darwin` release binary.
  - New `packaging/macos/scripts/preinstall` and `postinstall` scripts install the binary to `/usr/local/lib/ragent/ragent` and symlink it to `/usr/local/bin/ragent` so `ragent` is on the default PATH.
  - New `build-macos-arm64` job in `.github/workflows/release.yml` runs on `macos-15`, builds the arm64 binary, creates `ragent-{version}-macos-arm64.pkg`, and uploads the binary and package as release artifacts.
  - Release job now downloads and publishes the macOS artifacts alongside Linux and Windows assets.
  - The `.pkg` is intentionally unsigned; it is suitable for local installation and enterprise distribution. Apple Developer ID signing and notarization can be added later if required.

## Version: 1.0.7

### Fixed

- Added `authors.workspace = true` to the root `ragent` package in `Cargo.toml` so the per-package manifest inherits the workspace authors. This fixes the `cargo wix` step of the Windows release build, which requires an `authors` field.
- Removed the temporary `$env:CARGO_PKG_AUTHORS` override from `.github/workflows/release.yml`; the workflow now relies on the corrected Cargo.toml manifest.

### Changed

- Incremented workspace version to 1.0.6.

## Version: 1.0.5

### Fixed

- Windows release build in `.github/workflows/release.yml`:
  - Added a separate `cargo wix init --package ragent` step to generate the WiX `wix/main.wxs` source file before building the `.msi` package.

### Changed

- Incremented workspace version to 1.0.5.

## Version: 1.0.4

### Fixed

- Windows release build fixed in `.github/workflows/release.yml`:
  - Removed unsupported `cargo wix init --no-build` call (cargo-wix 0.3.9 does not accept `--no-build` on the `init` subcommand).
  - Added `--package ragent` so `cargo wix` works inside a Cargo workspace.
  - Stage the already-built `target/x86_64-pc-windows-msvc/release/ragent.exe` at `target/release/ragent.exe` before invoking `cargo wix --no-build`, matching the layout that cargo-wix expects.

### Changed

- Incremented workspace version to 1.0.4.

## Version: 1.0.3

### Added

- Windows x86_64 build and `.msi` installer packaging added to the release pipeline (`.github/workflows/release.yml`).
  - New `build-windows` job builds `ragent.exe` and creates a WiX-based MSI.
  - Release job now combines Linux and Windows artifacts for GitHub Releases.
  - Added `[package.metadata.wix]` section in `Cargo.toml` with a stable upgrade GUID.
  - Added Windows MSVC static C-runtime flags in `.cargo/config.toml` so the installed binary does not require the VC++ redistributable.
  - Added `/wix/` to `.gitignore` for generated WiX source files.

### Changed

- Incremented workspace version to 1.0.3.

## Version: 1.0.2

### Changed

- Incremented workspace version to 1.0.2.
- Updated `.github/workflows/security-audit.yml` and `deny.toml` ignore list to include all current transitive/direct dependency advisories so `cargo audit` stays green.
- Release pipeline now treats the dependency-audit job as informational (`continue-on-error: true`) so unmaintained crates do not block a release.

## Version: 1.0.0

### Changed

- Removed pre-release beta label and reset stable version to 1.0.0.
- Future releases will increment the patch (last) digit.

## Version: 0.1.0-beta.41

### Fixed

- Fixed CI failures after the v0.1.0-beta.40 release:
  - Restored pub(crate) visibility in ragent-tui/src/app/helpers.rs and added #![allow(clippy::redundant_pub_crate)] so it passes both the -D unreachable_pub dead-code lint and Clippy.
  - Added RUSTSEC-2026-0235 (rkyv) to deny.toml and .github/workflows/security-audit.yml ignores, matching existing transitive-dependency treatment.

### Changed

- Incremented workspace version to 0.1.0-beta.41.

## Version: 0.1.0-beta.40

### Fixed

- Resolved build/clippy warnings in `ragent-tui`:
  - Added `#[allow(dead_code)]` to `App::is_router_enabled` and the unused status helper methods (`set_status_info`, `set_status_success`, `set_status_warning`, `set_status_error`).
  - Added a doc comment to `App::execute_slash_command_inner`.
  - Changed redundant `pub(crate)` visibility to `pub` in the private `app::helpers` module to satisfy `clippy::redundant-pub-crate`.

### Changed

- Incremented workspace version to 0.1.0-beta.40.

## Version: 0.1.0-beta.39

### Changed

- Incremented workspace version to 0.1.0-beta.39.

## Version: 0.1.0-beta.38

### Added

- Strict exact-byte matching for `edit`, `multi_edit`, and `apply_patch` tools (EDITPLAN.md):
  - Replaced whitespace-tolerant/line-normalized replacement logic in `ragent-tools-core` with exact-byte `find_exact_replacement_range` and `find_exact_batch_edit` helpers.
  - Removed fallback heuristic replace in `replace.rs`; tools now return a clear single error listing the first non-matching old_string.
  - Added `test_edit_smoke.rs` automated smoke test (T-014) covering exact-match success and failure paths.
  - Added `EDITPLAN.md` and completion report `docs/reports/editplan-m1-completion.md` tracking milestones M1–M3.

### Fixed

- Fixed GitHub Actions Clippy failures caused by the stable toolchain upgrading to Rust 1.97.0.
  - Removed redundant `Arc::from(client)` wrapping and closure-style `Arc::clone` in `crates/ragent-agent/src/session/loop_steps.rs`.
  - Replaced `needless_collect` and `filter().next()` patterns in `crates/ragent-agent/tests/inline/skill_loader.rs` with `!any(...)`.
  - Removed `let _ =` on unit timeout await in `crates/ragent-agent/tests/test_bg_service.rs`.
  - Switched single-character `contains` checks to `char` patterns in `crates/ragent-agent/tests/test_instruction_includes.rs`.
  - Replaced `Default::default()` followed by field assignment with struct-expression initialization in `ragent-telemetry` tests and `src/instruments.rs`.
  - Removed unnecessary `to_path_buf()` in `crates/ragent-tools-core/tests/test_open.rs`.
  - Removed clone-to-slice in `crates/ragent-llm/src/providers/tool_cache.rs`.
  - Added missing blank line before top-level doc comment in `crates/ragent-tui/src/widgets/message_widget.rs`.
  - Restored public visibility for API symbols used by external integration tests (TUI `App` methods, `MessageWidget`, and message-widget helpers) that became private after a previous pub(crate) sweep.
- Fixed flaky TUI test `test_telemetry_setup_context_menu_paste_writes_active_field` that failed in GitHub Actions CI with `X11 server connection timed out`.
  - Added a thread-local test-only clipboard override (`ClipboardTestOverrideGuard`) in `crates/ragent-tui/src/clipboard.rs` so paste tests can run on headless runners without a display server.
  - Updated the telemetry paste test to use the override instead of writing to the real system clipboard.
- Hardened YOLO-mode test isolation to remove parallel-test races in `test_alt_y_toggles_yolo_mode_and_status_bar_indicator` and `test_slash_yolo_toggles_and_persists`.
  - `enter_temp_config_dir()` now primes a project-local `.ragent/ragent.json` with a known `yolo: false` state before toggling.
  - Decoupled the in-memory YOLO flag from `Config::load()`; added `ragent_config::yolo::sync_from_config()` for explicit startup sync and called it from `src/main.rs`. This prevents unrelated config reloads from racing with an in-flight toggle during parallel tests.
  - Updated `ragent-config/tests/test_yolo_persistence.rs` to call the new explicit sync helper.

### Changed

- Incremented workspace version to 0.1.0-beta.38.

## Version: 0.1.0-beta.37

### Added

- TUI clipboard remediation (CUTPLAN.md Milestones 1–5):
  - New `crates/ragent-tui/src/clipboard.rs` module is the single source of truth for `arboard` text and image clipboard operations.
  - `InputField::paste_text_from_clipboard` (with `paste_clipboard` alias) and `App::{get,set}_clipboard` now delegate to shared helpers.
  - Device-flow user-code copy in `input.rs` uses the shared helper.
  - `App::handle_paste_text` strips `\r` and replaces active keyboard/mouse selections; used by Ctrl+V, terminal bracketed paste, and context-menu Paste.
  - Context-menu Paste in provider setup now supports `TelemetrySetup` alongside `EnterKey` and `GitLabSetup`.
  - Clipboard image temp files are written under `<cwd>/target/temp/` as `ragent_paste_*.png` with Unix permissions `0o600`, encoded directly from the borrowed pixel buffer (no `to_vec()` copy).
  - TUI startup prunes orphaned `ragent_paste_*.png` files older than 24 hours.
  - `App::paste_image_from_clipboard` is now `pub(crate)`; it warns when a clipboard-resolved image path lies outside the working directory or home directory while still attaching the file.
  - User-facing docs (`QUICKSTART.md` and `TUI-QUICKSTART.md`) updated to describe text selection, Copy/Cut/Paste, right-click context menu, terminal bracketed paste, and `Alt+V` image paste.
  - Tests added in `crates/ragent-tui/tests/test_clipboard.rs` and extended in `tests/test_clipboard_tempfile.rs` and `tests/test_slash_commands.rs`.

### Changed

- Incremented workspace version to 0.1.0-beta.37.
- Reviewed and cleaned up project guidelines in `AGENTS.md`:
  - Renamed the agent acknowledgement section and removed legacy memory tool names (`memory_read`, `memory_write`, `memory_replace`, `memory_search`, `memory_migrate`) from the available-tools list.
  - Added an explicit "Tool Use — Critical Instructions" callout instructing immediate tool invocation without narrative preamble.
  - Added `RELEASE.md` to the approved root documentation exceptions list.
  - Removed project-specific requirement identifiers from the test migration guidance.
- Deleted the stale `assets/config/AGENTS.md` copy to eliminate drift from the canonical root guidelines.

## Version: 0.1.0-beta.36

### Changed

- Incremented workspace version to 0.1.0-beta.36.
- Security remediation planning: created `SECPLAN.md` with P0–P3 risk register,
  concrete file/line references, and milestone-based remediation roadmap.

## Version: 0.1.0-beta.35

### Added

- New `TOOLS.md` root documentation file listing all available agent tools.
- Updated `team_create` tool input schema to support an optional `context` field
  for richer teammate creation prompts.

### Improved

- Tool system-prompt sufficiency audit and remediation across all 142 registered
  tools (T-001–T-004 in `TOOLPLAN.md`). Every tool now has a description of at
  least 120 characters, explicit required-parameter callouts, and a strict
  JSON schema with `"additionalProperties": false` to reject hallucinated
  parameter names. System-prompt guidance sections were added/updated for VCS
  safety, codeindex usage, memory tools, team tools, ask-user tools, and
  file-reading best practices.

## Version: 0.1.0-beta.34

### Added

- `/actionloop` slash command (with `help` and `clip` subcommands) that reports
  agent action-loop average timings from the profiler, sorted by descending
  average elapsed time so hotspots are visible at a glance.
- `github_get_actions` tool input/result summaries in the TUI message widget,
  showing the inspected run count and any failed runs.
- `CompactionConfig.threshold` percentage-based trigger (0.0–1.0). When set
  (e.g. `0.8` = 80%), compaction fires at `context_window * threshold`; when
  `None` the buffer-based `context_window - max(output_tokens, buffer)` model
  is used. The legacy `compression.auto_threshold` value is migrated into this
  field so existing configurations keep their trigger point.
- New documentation: `ALPLAN.md` (agent-loop performance remediation plan with
  hotspots H1–H4 and rollout order) and `docs/agentorch.md` (component-by-component
  agent loop / orchestrator internals with exact `file:line` references).

### Changed

- Incremented workspace version to 0.1.0-beta.34.
- `Storage::open` now enables `journal_mode=WAL` and a 5s `busy_timeout` so a
  background writer (e.g. the startup FTS warm-up) no longer serialises
  concurrent readers behind it and stalls `get_setting`/`detect_provider`.
- `warm_message_search_index` rebuilds the FTS index inside a single transaction
  (one fsync instead of one per row), making the startup warm-up effectively free.
- Loop-step P-3 now treats non-positive provider-reported context windows
  (notably the virtual Model Router reporting `0`) as "unknown" and falls back
  to the 128k default so compaction never fires on the first turn.
- The TUI pre-send compaction check now honours the configured
  `compaction.threshold` percentage when present, matching the server-side
  estimator; otherwise it falls back to the conservative 92% hard limit.
- `detect_provider` now runs a fast pass that defers the Copilot `gh auth token`
  CLI subprocess until no provider is found cheaply, so startup provider
  detection never blocks on a cold keyring / slow `gh`.
- Fixed a corrupted UTF-8 box-drawing comment in `message_widget.rs` (mojibake
  from a prior commit) and removed redundant double-gating in the raw-args
  fallback per the `/simplify` review.

### Fixed

- `test_worker_indexes_changed_file` now polls for worker batch processing
  instead of a fixed sleep, making it robust on slow/saturated CI runners.

### Tests

- Added `crates/ragent-storage/tests/test_wal_warmup.rs` asserting WAL mode is
  active and that a background warm-up writer does not block a concurrent reader.
- Added compaction threshold percentage / legacy-alias migration tests in
  `ragent-config` and `ragent-agent` compaction estimator tests.
- Added `/actionloop` slash-command tests (help, no-samples hint, clip, timings).
- Added `github_get_actions` tool summary tests and provider-detection
  fast-path (defer `gh` CLI) tests.

## Version: 0.1.0-beta.33

### Changed

- Incremented workspace version to 0.1.0-beta.33.
- Removed the `agentgrep` structure-aware code search tool and its unused
  dependencies (`grep-regex`, `grep-searcher`, `ignore`, `glob` from
  `ragent-tools-extended`) because it duplicated the `codeindex_*` toolset.
- Updated `README.md`, `QUICKSTART.md`, `TUI-QUICKSTART.md`, `SPEC.md`, and
  `docs/JCODEPLAN.md` to remove `agentgrep` references.

## Version: 0.1.0-beta.32

### Changed

- Incremented workspace version to 0.1.0-beta.32.
- Updated CI workflow.

## Version: 0.1.0-beta.31

### Fixed

- TUI tool-call summaries now always show parameters by falling back to the raw
  tool args when the expected JSON keys (`path`, `command`, etc.) are missing.
  This keeps bash/read/write/create/edit inputs visible even when providers emit
  unexpected field names.
- Fixed rustfmt indentation issue in `crates/ragent-tools-extended/src/masterfetch/security.rs`.

## Version: 0.1.0-beta.30

### Changed

- Incremented workspace version to 0.1.0-beta.30.
- Updated CI workflow and performed multiple codebase hygiene updates.

## Version: 0.1.0-beta.29

### Changed

- Removed legacy memory system (file-block memory modules, old structured-memory
  storage, migration helpers, and cross-project import/export code) and replaced
  it with the new structured-memory store backed by `ragent-storage`.
- Fixed the memory panel in the TUI after the memory-system refactor so it
  continues to browse and render stored memories correctly.

## Version: 0.1.0-beta.28

### Changed

- Fixed Build and Release workflow by granting `contents: write` permission so
  `softprops/action-gh-release@v2` can create GitHub releases.

### Fixed — CI Check & Test

- Reverted the `check-and-test` job to debug builds (`cargo check/test
  --workspace`) and removed the accidental `--release` flags that caused the
  runner to run out of memory while linking the release test binary.
- Added an 8 GiB swapfile step and a 45-minute job timeout to give the debug
  build more headroom and prevent runaway jobs.

## Version: 0.1.0-beta.27

### Changed

- Optimized CI runners: moved `check-and-test` to `ubuntu-latest-4-cores`, disabled
  debuginfo in dev/test profiles, and added `free-disk-space` cleanup to reduce
  disk pressure during builds.

## Version: 0.1.0-beta.26

### Changed

- Incremented workspace version to `0.1.0-beta.26`.

## Version: 0.1.0-beta.25

### Changed — CI package builds

- Disabled `.rpm` package builds in the release workflow; only the plain binary
  is now published. Both Debian and RPM packaging are temporarily disabled
  while the packaging build paths are reviewed.

## Version: 0.1.0-beta.24

### Changed — CI package builds

- Disabled `.deb` package builds in the release workflow; the `.rpm` package and
  plain binary remain published. Debian packaging is temporarily disabled while
  the `cargo-deb` build path is reviewed.

## Version: 0.1.0-beta.23

### Added — CI package builds

- Added `.github/workflows/release.yml` that triggers on `v*` tags and builds
  `ragent` for `x86_64-unknown-linux-gnu` on `ubuntu-latest`.
- CI installs `cargo-deb` and `cargo-generate-rpm`, then runs `cargo deb` and
  `cargo generate-rpm` against the release binary.
- The release body is populated from the matching section of `CHANGELOG.md`
  (extracted via `awk`) and published with `softprops/action-gh-release@v2`.
- Assets published to the GitHub Release include:
  - `ragent-<version>-x86_64.deb`
  - `ragent-<version>-x86_64.rpm`
  - the plain `ragent` binary
- Root `Cargo.toml` now carries `[package.metadata.deb]` and
  `[package.metadata.generate-rpm]` metadata so the generated packages
  install the binary to `/usr/bin/ragent` and ship `README.md`, `LICENSE`,
  and `CHANGELOG.md` to `/usr/share/doc/ragent/`.

### Changed — OpenTelemetry updated to 0.28

- Bumped `opentelemetry`, `opentelemetry_sdk`, and `opentelemetry-otlp` to
  0.28 across `crates/ragent-telemetry` and adapted to the breaking API
  changes (new `Resource` builder, `PeriodicReader` signature,
  `InMemoryMetricExporter` relocation, `MetricReader` return types).

## Version: 0.1.0-beta.22

### Added — `/provider` always allows editing the API key

- The `/provider` slash command now opens the provider picker with
  `force_key_entry: true`, so selecting an already-configured key-based
  provider shows the `EnterKey` dialog instead of skipping straight to the
  model list. This lets users update an existing API key without removing
  and re-adding the provider.
- The `EnterKey` dialog pre-fills the key field with the existing stored key
  (`App::provider_api_key`) so the user can edit it rather than re-entering
  from scratch.
- The API-key and GitLab token fields are now displayed **unmasked** so the
  user can verify the full value, and the dialog is widened (80×30) so the
  full key (≥ 48 chars) is visible.

### Changed — `/model` jumps straight to the model list for configured providers

- When a provider is already configured, `/model` now skips the provider
  picker and jumps directly to model discovery / the model list for that
  provider. The provider picker is only shown when no provider is configured.
  Special-cases: `azure_resource` opens the resource-file picker, `router`
  opens the cluster setup UI.

### Added — Research `--use-low-relevance` flag

- `ragent research create <name> --use-low-relevance` (and the TUI / HTTP
  equivalents) retains every fetched web page regardless of its
  query-match relevance score, disabling the default filter that discards
  "Low"/"Very low" sources. Plumbed through `SessionConfig::use_low_relevance`,
  `WebGatherer::with_keep_low_relevance`, the CLI, TUI slash handler, and the
  `POST /research` HTTP route.

## Version: 0.1.0-beta.21

### Fixed — Compaction user feedback & resilience

- Compaction bail paths (empty head, prompt-overflow, LLM summarisation failure,
  empty summary) now publish an `Event::AgentNotice` so the TUI and HTTP clients
  show "Context compression skipped/failed: …" instead of silently bailing.
- Compaction warning logs now use `warn!` with the bail reason and carry the
  session id, making skipped compressions visible in diagnostics.

### Fixed — Post-compaction continuation nudge

- After a successful compaction the session loop injects a continuation nudge
  so the agent resumes its task instead of stopping. The `compaction_nudged`
  flag is now threaded across loop iterations (no longer reset to `false`
  each turn), preventing repeated nudges. Integration tests updated to expect
  the extra post-compaction continuation request.

### Fixed — Autopilot auto-continue after task completion

- Added `App::last_task_completed_at` timestamp set when a `TaskCompleted`
  event arrives. `poll_autopilot_continue` now suppresses the auto-continue
  and disables autopilot when the agent already signalled completion, so
  autopilot no longer keeps re-prompting after `agent_complete`.
- The `FinishReason` handler also guards against re-entering autopilot
  continue when a `TaskCompleted` was already consumed this turn.

### Added — Router downstream-model status bar

- The TUI status bar now shows the actual downstream model and tier for the
  router virtual provider: `Model Router ({provider}:{model}) / {tier}`
  instead of the static `Model Router / router` label. New
  `router_current_model` field captures the last routed downstream model,
  surfaced via `Event::RouterTierSelected`. New test
  `test_router_status_bar_label_shows_downstream_model_and_tier`.

### Added — Autopilot status indicator in status bar

- Status bar line 2 now shows `AutoPilot:✓` (green) when autopilot is active
  and `AutoPilot:✗` (red) when disabled, giving immediate visual feedback.

### Fixed — Router terminal-signal guarantee

- `RouterClient::chat` now wraps the downstream stream so that if the
  provider ends without emitting a `StreamEvent::Finish`, a synthetic
  `Finish { reason: Stop }` is injected. This guarantees the session loop
  always observes a terminal event per LLM call, preventing infinite loops
  on provider protocol drift.

### Fixed — Skill discovery test isolation

- Skill discovery/registry tests now filter by `SkillScope` (Project vs
  Personal) and assert against `bundled_count()` rather than the total
  registry length, so the tests no longer break when bundled or personal
  skills are present alongside project skills.

### Fixed — Doctest build breakages

- Updated doctests in `session::permissions` (marked `ignore` since the
  helper is `pub(crate)`) and `tool::ToolRegistry` (switched example from
  `ReadTool` to `PlanEnterTool` and added the missing fields to the
  `ToolContext` doc example) so the crate's doctests compile again.

### Fixed — Research progress config `from_file` field in tests

- `test_research_progress_config` now sets the new `from_file: None` field
  introduced by the `--from-file` research feature.

## Version: 0.1.0-beta.20

### Added — Research support for local file topics (`--from-file`)

- `ragent research create <name> --from-file <PATH>` (and `/research create
  --from-file` in the TUI) extracts a local document and uses its content as
  the research subject in place of an explicit topic. Supported formats: PDF,
  DOCX, XLSX, PPTX, ODT, ODS, ODP, TXT, and MD. The extracted content becomes
  the primary source; web search still runs using the derived topic.
- When no explicit topic is given, a concise topic and clean title are derived
  from the extracted document body via the optional LLM summarizer
  (`summarize_subject`), falling back to the heuristic
  `derive_topic_from_url_body` scraper when no LLM is configured.
- New `document_extract` module in `ragent-tools-extended` performs the text
  extraction (including a direct PDF fast path extended in `libreoffice_read`).
- New `SessionEvent::FromFileBodyPreview` surfaces a ~200-char preview of the
  extracted text so the TUI and HTTP clients can show what content was used
  to derive the topic; `/research` TUI progress panel shows the `from-file`
  path in the header.
- `derive_title_full` picks the file path as the item-title fallback after
  topic and URL.
- `SessionConfig::from_file` plumbed through the research adapter, manager,
  server routes, and CLI; `--from-url`, `--from-file`, and explicit topics
  are mutually combinable.

### Fixed — Control-character sanitisation in research documents

- `strip_control_chars` (new public helper in `ragent-research::item`)
  removes C0/C1 control characters and BOM from all research document fields
  (summary, findings, cross-references, open questions, queries) before
  rendering into `RESEARCH.md`, so model output or raw PDF extraction can no
  longer corrupt the document with binary garbage.
- Analysis parsing (`parse_subject_summary`, fallback findings rescue) now
  sanitises model JSON before extraction.
- New `test_control_char_sanitization` test suite covers the rendering path.

## Version: 0.1.0-beta.19

### Fixed — Startup messages

- Added trailing newlines to TUI startup status messages (code index
  enabled/disabled/failed and the "Ready" banner) so subsequent output starts
  on a clean line instead of being appended to the same line.

## Version: 0.1.0-beta.18

### Fixed — Startup blocking issues

- MCP server connections now happen in a background `tokio::spawn` task instead
  of sequentially on the main task, eliminating the 5–15 s startup stall when one
  or more MCP servers are slow to start.
- Code-index startup (open + watcher + initial `full_reindex`) now runs in a
  background task and wires into `App` state via an mpsc channel, so the TUI
  event loop starts immediately and the index becomes available when ready.
- Provider health check (including Copilot token resolution via `gh auth token`)
  now runs entirely inside its spawned async task instead of blocking the TUI
  render loop during startup.
- `App::backfill_model_ctx_window` no longer calls synchronous model discovery
  (`sync_discover_models`) at startup; only cached/default metadata is consulted.
- The first printable keystroke after the run-cost banner is no longer
  swallowed — non-character keys still just dismiss the banner, but a plain
  character clears the banner and falls through to normal input so the first
  typed character is not lost.

### Added — Startup timing instrumentation

- New `StartupTimings` type (`crates/ragent-types/src/startup.rs`) records the
  wall-clock duration of every instrumented startup stage (CLI parse, config
  load, storage open, provider/tool registries, TUI init, session create, code
  index, MCP, etc.).
- New `/startup` TUI slash command renders an aligned stage/time table so users
  can identify which stages contribute most to perceived startup latency.
- Stages recorded inside `App::new()` are merged into the main timings
  collector via `StartupTimings::merge_stages`.

### Changed — Compaction prompt cap and reuse

- `MAX_COMPACTION_PROMPT_CHARS` reduced from 120 000 to 60 000 chars (~15 k
  tokens) to keep the LLM summarisation call tractable while still giving the
  model enough context for a useful summary. The verbatim recent tail
  (`keep_tokens`) is preserved regardless of this cap.
- `select()` now pre-serialises the head transcript and computes the original
  token cost, so `compact()` reuses them instead of re-serialising every message
  a second time.

### Added — Post-compaction continuation nudge

- When context compaction runs and the LLM responds without tool calls, a
  one-time user nudge is injected so the agent resumes its in-progress task
  rather than letting the loop stop prematurely.

### Added — Copilot `gh` CLI token cache

- `find_gh_cli_token` caches its result in a process-wide `OnceLock` so the
  `gh auth token` subprocess is spawned at most once per session.

### Changed — Code-index performance

- SQLite store now sets `WAL` journal mode, `synchronous = NORMAL`, and
  `temp_store = MEMORY` pragmas for dramatically faster writes.
- `get_file_symbols` queries directly by `file_id` instead of loading all
  symbols and filtering in Rust.
- Reindex chunk yield reduced from 5 ms to 1 ms for tighter throughput.
- Reindex now logs per-phase timing (scan, diff, apply, fts_sync, total).

## Version: 0.1.0-beta.17

### Added — `@<path>` directive for modular instruction files

- Instruction files (`AGENTS.md`, `CLAUDE.md`, `.ragent.md`, `INSTRUCTIONS.md`)
  now support a C/C++ `#include`-style mechanism for modularity. A line of
  the form `@docs/conventions.md` (or `@"path with spaces.md"`) — with the `@`
  in the first column — is replaced in-place by the contents of the referenced
  file before the content is loaded into the system prompt.
- A leading `@@` is an escape sequence that collapses to a single literal `@`
  character, so lines that start with `@` can be written verbatim without
  triggering an include.
- Paths resolve relative to the directory of the file containing the
  directive; includes are transitive (included files are themselves
  expanded). Cycle detection (visited-path set) and a depth cap
  (`MAX_INCLUDE_DEPTH = 16`) prevent infinite loops. Absolute paths and
  `../` escapes outside the working dir / global ragent data dir are
  rejected with an inline marker comment; missing or unreadable files emit
  a marker comment rather than failing. Implemented in
  `crates/ragent-agent/src/agent/mod.rs` (`expand_includes`), wired into
  `collect_agents_md_content_with_discovery`. Tests in
  `crates/ragent-agent/tests/test_instruction_includes.rs`.

### Changed — `@include <path>` → `@<path>` include syntax

- The instruction-file include directive now uses `@<path>` (the `@` sigil
  followed directly by the path) instead of `@include <path>`. The `@` must
  appear in the first column of the line. The `@@` escape sequence is new.
  Existing instruction files using `@include path/to/file.md` must be updated
  to `@path/to/file.md`.

### Completed — Open/reveal and remaining UX tools — JCODEPLAN M10

- Confirmed `open` tool implementation in `crates/ragent-tools-core/src/open.rs`
  (T-090 through T-094): cross-platform `xdg-open`/`open`/`start` wrapper,
  `reveal` action for opening a target's parent directory, URL scheme
  allowlist validation (`http`, `https`, `mailto`, `file`), and integration
  tests in `crates/ragent-tools-core/tests/test_open.rs`.
- `open` is registered via `create_core_registry()` and surfaced automatically
  in the agent default registry under the `shell:execute` permission category.
- `docs/JCODEPLAN.md` updated to mark M10 tasks complete and added
  `docs/reports/jcodeplan-m10-completion.md`.

### Added — Durable initiatives and skill management — JCODEPLAN M8

- New `initiative` tool in `crates/ragent-agent/src/tool/initiative.rs`
  (T-070) managing durable, project-scoped goals with milestones. Actions:
  `create`, `read`, `update`, `checkpoint`, `list`, `close`. `checkpoint`
  marks a milestone complete (recording `completed_at`), bumps overall
  progress, and appends a timestamped `Checkpoint:` note to the description.
  `close` supports `completed` (auto-fills progress to 100) and `abandoned`
  (keeps recorded progress). Registered in `create_default_registry()` under
  the `storage:write` permission category (T-073).
- New `initiatives` SQLite table + CRUD (`create_initiative`,
  `get_initiative`, `list_initiatives`, `update_initiative`,
  `delete_initiative`) in `ragent-storage`, with `InitiativeMilestone` /
  `InitiativeRow` types re-exported via `crate::storage` (T-070).
- `## Active Initiatives` system-prompt section injected on every turn in
  `session/loop_steps.rs`, listing active initiatives with progress and the
  next pending milestones so the agent stays aware of long-term goals across
  sessions and compaction (T-070 "surface in system prompt").
- New `skill_manage` tool in `crates/ragent-agent/src/tool/skill_manage.rs`
  (T-071). Actions: `list` (with `scope` filter + optional `include_bodies`),
  `read` (processed prompt with `$ARGUMENTS` substitution), `load` (discover
  + return prompt — injects skills added/edited after session start),
  `reload` (clear caches, re-discover, report added/removed skills).
- New `SkillInfo::clear_body_cache()` async helper used by the `reload`
  action to drop on-disk `SKILL.md` caches without a session restart.
- Migration SQL block in `ragent-storage` re-indented consistently to prevent
  future edit collisions.
- Tests (T-072): 26 tests in `crates/ragent-agent/tests/test_initiative.rs`
  (tool identity, schema, create/read/update/checkpoint/list/close, duplicate
  and invalid-slug rejection, cross-session visibility, per-project
  isolation, empty-storage graceful error, storage round-trip, prompt
  section), 12 tests in `crates/ragent-agent/tests/test_skill_manage.rs`
  (bundled + project discovery, scope filter, arg substitution, unknown-skill
  listing, load injects prompt, reload picks up added skills + edited
  bodies), 7 tests in `crates/ragent-storage/tests/test_initiatives.rs`
  (table creation, field round-trip, status filter, closed_at lifecycle,
  malformed-JSON fallback).
- Documented both tools in `SPEC.md` §19B.

### Added — Gmail and messaging channel tools — JCODEPLAN M7

- New `gmail` tool in `crates/ragent-tools-extended/src/gmail.rs` (T-060)
  providing Gmail search/read/draft/send via the Gmail REST API with OAuth2
  tokens stored encrypted in the SQLite credential store (`SqliteTokenStore`),
  plus `auth`/`status`/`logout` management actions and automatic
  refresh-token exchange + retry on HTTP 401.
- New `send_channel_message` tool in
  `crates/ragent-tools-extended/src/channels.rs` (T-061) sending short
  messages to Telegram (bot API) and Discord (incoming webhook), with
  `send` (target `telegram`/`discord`/`all`) and `status` actions.
- New config schema: `gmail` block (`client_id`, `client_secret` with `env:`
  indirection) and `channels` block (`enabled`, `telegram`, `discord`) in
  `ragent-config`. Client credential precedence: auth args → stored tokens →
  config → `GMAIL_CLIENT_ID`/`GMAIL_CLIENT_SECRET` env vars.
- Both tools use the `network:send` permission category and degrade
  gracefully with honest errors and `next_action` hints when unconfigured.
- Registered both tools in `create_extended_registry()` (T-063), surfaced in
  the agent automatically via `register_extracted_extended_tools`.
- Mocked-backend integration tests (T-062):
  `crates/ragent-tools-extended/tests/test_gmail.rs` (19 tests, axum mock
  server, encrypted store round-trip, refresh exchange, RFC 2822 wire check)
  and `crates/ragent-tools-extended/tests/test_channels.rs` (20 tests, mock
  Telegram/Discord fanout, config merge, env indirection).
- Documented both tools in `SPEC.md` §19A.

## Version: 0.1.0-beta.16

### Added — Conversation and cross-session search tools — JCODEPLAN M5

- New `conversation_search` tool in `crates/ragent-agent/src/tool/conversation_search.rs`
  provides keyword search, turn-range retrieval, and statistics for the current
  session. Modes: `keyword` (default), `turn_range`, `stats`.
- New `session_search` tool in `crates/ragent-agent/src/tool/session_search.rs`
  performs ranked full-text search across all stored sessions with filters for
  date range, working directory, role, per-session limits, and optional
  surrounding context.
- Session message FTS5 index (`messages_fts`) and optional embedding cache
  (`messages_embedding`) in `ragent-storage`, with `store_message_embedding`,
  `get_message_embedding`, and `search_messages_by_embedding` helpers.
- `warm_message_search_index()` is called on startup to rebuild the FTS index
  in a background blocking task.
- New `ConversationSearched` and `SessionSearched` event variants, with SSE
  serialization in `crates/ragent-server/src/sse.rs`.
- Added integration tests in `crates/ragent-agent/tests/test_conversation_search.rs`,
  `crates/ragent-agent/tests/test_session_search.rs`, and
  `crates/ragent-storage/tests/test_message_embeddings.rs`.
- Registered both tools in `create_default_registry()` under the Memory category.

## Version: 0.1.0-beta.15

### Added — Browser automation tool (`browser`) — JCODEPLAN M4

- New `browser` tool in `crates/ragent-tools-extended/src/browser/` providing
  Chrome DevTools Protocol (CDP) browser automation with 14 actions: `open`,
  `snapshot`, `click`, `type`, `fill_form`, `select`, `wait`, `eval`,
  `scroll`, `upload`, `press`, `screenshot`, `status`, `setup`.
- CDP WebSocket client (`browser/cdp.rs`) with JSON-RPC command/response
  correlation, event fan-out via broadcast channel, and graceful degradation
  when no browser is available.
- Browser launcher (`browser/launch.rs`) with platform-specific Chrome/
  Chromium binary detection (Linux, macOS, Windows) and headless launch
  via `--remote-debugging-port`.
- Action handlers (`browser/actions.rs`) implementing each action using CDP
  domains (Page, DOM, Runtime, Input, Network).
- `BrowserConfig` in `ragent-config` with `cdp_endpoint` and
  `default_headless` fields, configurable in `ragent.json` under the
  `browser` key.
- `browser` tool-visibility switch added to `ToolVisibilityConfig` —
  toggle via `/tools browser on|off`.
- TUI `/tools` slash command updated to include `browser` and `masterfetch`
  in the valid switches list.
- Added `tokio-tungstenite` workspace dependency for WebSocket support.
- Added integration tests in
  `crates/ragent-tools-extended/tests/test_browser.rs` (37 tests covering
  tool identity, schema, graceful degradation, config, visibility, CDP types,
  and conditional live CDP tests).
- Registered as `browser` in `create_extended_registry()`.

## Version: 0.1.0-beta.14

### Added — Codex-style patch tool (`apply_patch`)

- New `apply_patch` tool in `crates/ragent-tools-core/src/apply_patch.rs` parses
  `*** Begin Patch` / `*** End Patch` envelopes with `*** Add File:`,
  `*** Delete File:`, and `*** Update File:` operations. Update hunks use `@@`
  headers with context (` `), add (`+`), and remove (`-`) lines.
- Supports file moves via `*** Move to:` inside an update block, including
  rename-and-edit in a single patch.
- All operations are validated before any file is written; paths are resolved
  relative to the working directory and canonical containment is enforced.
- Includes `dry_run` parameter to preview changes without writing files.
- Added integration tests in `crates/ragent-tools-core/tests/test_apply_patch.rs`.
- Registered in `create_core_registry()`.

### Added — Open/reveal tool (`open`)

- New `open` tool in `crates/ragent-tools-core/src/open.rs` opens files, folders,
  and URLs using the platform default handler (`xdg-open` on Linux, `open` on
  macOS, `start` on Windows).
- Supports `open`, `reveal` (parent directory), and `url` actions. URL schemes
  are validated against an allowlist (`http`, `https`, `mailto`, `file`).
- Paths are resolved relative to the working directory and checked for root
  containment.
- Added integration tests in `crates/ragent-tools-core/tests/test_open.rs`.
- Registered in `create_core_registry()`.

### Fixed — `agentgrep` clippy warnings

- Cleaned up `agentgrep.rs` to satisfy `-D warnings`:
  replaced `map_or(false, ...)` with `is_some_and`, iterated map keys directly,
  simplified sorts, and flattened a glob loop.

### Fixed — TUI read tool header uses pending args when `ToolCallStart` is dropped

- In `crates/ragent-tui/src/app/event_handler.rs`, the `Event::ToolCallBatch`
  fallback now applies any previously-stored `pending_tool_args` to a missing
  tool-call part after creating it. This fixes the case where the broadcast
  bridge drops `ToolCallStart` but `ToolCallArgs` was already queued: the TUI
  widget header was showing `📄 missing path` even though the args JSON
  contained a valid `path`.
- `update_tool_call_input` is reused to merge the pending JSON into the newly
  created part's `state.input`, so `tool_input_summary` can render the correct
  read path in the header.
- Added regression tests
  `test_tool_call_batch_applies_pending_args_when_start_dropped` and
  `test_tool_call_batch_does_not_overwrite_existing_input` in
  `crates/ragent-tui/src/app/tests.rs`.

### Fixed — TUI read tool header always shows icon, and missing path surfaces in UI

- `tool_input_summary` in `crates/ragent-tui/src/widgets/message_widget.rs` no
  longer returns an empty string for `read` calls with a missing/empty `path`.
- When `path` is absent the header now renders `📄 missing path`, keeping the
  file icon visible and clearly signalling the malformed input to the model.
- Added `test_input_summary_read_tool_missing_path_shows_placeholder` in
  `crates/ragent-tui/tests/test_tool_display.rs`.
- The underlying `ReadTool::execute` already errors with
  "Missing required 'path' parameter", so the LLM receives an actionable
  diagnostic prompting it to correct the call.

## Version: 0.1.0-beta.13

### Changed — Version bump

- Workspace version bumped from `0.1.0-beta.12` to `0.1.0-beta.13`.
- Added JCode cost accounting and fixed tool widgets.

## Version: 0.1.0-beta.12

### Added — Research completion reports excluded web sources

- `ragent-research` now counts web pages that were fetched but excluded due to
  low relevance (`excluded_count`).
- `GatherResult`, `RunOutcome`, and `SessionEvent::Done` all carry the new
  `excluded_count` field so the information flows from the gatherer through the
  session to observers.
- `WebFetchedPage` now includes an optional `language` field populated by the
  `mf_fetch` layer and propagated into `Source::Web`.
- CLI final output updated to
  `Done: N sources (PDF P, YouTube Y, X excluded)` and the JSON event now
  includes `excluded_count`.
- TUI `/research create` progress now decodes `excluded_count`, passes it to
  `ResearchProgress::finish`, and includes it in both the rendered markdown log
  and the final status-bar message.

### Added — Per-run cost summary (`Event::RunCostSummary`)

- At the end of every `process_user_message` turn, the session processor now
  accumulates `Event::TokenUsage` totals, calls `compute_run_cost`, and publishes
  a single `Event::RunCostSummary` on the event bus.
- The summary carries `session_id`, `model_id`, `input_tokens`,
  `output_tokens`, `total_cost_usd`, and `duration_ms`.
- Cost computation respects user-defined price overrides from `ragent.json`
  (`Config::prices`) via `merged_prices` and falls back to the built-in price
  table; unknown models count tokens with zero cost.
- The TUI logs a one-line `⟡ run complete` banner on `Event::RunCostSummary`
  and updates the `ragent.cost.session` telemetry counter.
- The TUI now also renders a transient one-line
  `⟡ run complete · {in}+{out} tokens · ${cost} · {dur}s` banner overlay
  (FR-012, T-013) on `Event::RunCostSummary`, dismissed on the next keypress,
  while the full summary (model id + millisecond duration) is logged to the
  log panel.
  - Added TUI tests `crates/ragent-tui/tests/test_run_cost_banner.rs` covering
    banner population, log content, cross-session filtering, keypress dismissal,
    and default state.
  - Run-cost summaries are now persisted in a dedicated `run_cost_summaries`
    SQLite table (FR-018, T-024) so they can be retrieved for `--include-cost`
    exports, but are **omitted** from the default session export JSON.
  - `session export` CLI command now accepts a `--include-cost` flag; when set,
    the export JSON is wrapped as `{ "messages": [...], "cost_summaries": [...] }`
    with per-run cost records (`input_tokens`, `output_tokens`,
    `total_cost_usd`, `duration_ms`, `model_id`, `created_at`). Without the
    flag, only the messages array is exported (no cost data).
  - `RunCostSummaryRow` derives `Serialize`/`Deserialize` for JSON export and
    is re-exported from `ragent-storage` and `ragent-agent::storage`.
  - The session processor persists each `RunCostSummary` via
    `spawn_blocking` (non-blocking) alongside publishing the event.
  - Added storage tests `crates/ragent-storage/tests/test_run_cost_summaries.rs`
    covering round-trip persistence, session scoping, JSON serialization, and
    default-vs-opt-in export separation.
  - Extended `crates/ragent-agent/tests/test_run_cost_summary.rs` to assert the
    summary is persisted in storage after `process_message` completes.
  - The HTTP server serializes `RunCostSummary` as SSE event type
    `run_cost_summary`.
  - Added integration test `crates/ragent-agent/tests/test_run_cost_summary.rs`
    and SSE serialization test `test_run_cost_summary`.

### Changed — Version bump

- Workspace version bumped from `0.1.0-beta.11` to `0.1.0-beta.12`.
- Formatting fixes to `/research` tooling.

### Fixed — Web-source direct quotations are hard-capped to 200 characters

- Added a post-processing pass in `crates/ragent-research/src/analysis.rs` that
  mechanically truncates inline double-quoted strings and fenced code blocks
  inside each finding's `**Observation:**` paragraph to 200 characters. This
  enforces the existing prompt instruction even when the synthesis model
  ignores it, so RESEARCH.md findings no longer contain oversized web-source
  excerpts.
- The cap is applied after both clean LLM parses and mechanical fallback
  findings, and only affects the Observation paragraph so analysis text,
  cross-references, and implications are left untouched.

### Fixed — Removed redundant `mf_fetch:` prefix from `RESEARCH.md` source citations

- `parse_mf_fetch_output` in `crates/ragent-agent/src/research_adapter.rs` now
  strips the leading `mf_fetch: <url>` header block from plain-text tool output
  (cache hits, PDF pages, YouTube transcripts, and error responses) before the
  research layer stores the page body.
- The source title fallback is now taken from the real content rather than the
  `mf_fetch:` header, so the per-finding **Sources** list no longer shows the
  redundant `mf_fetch: <url> — <url>` pattern.
- Unrelated plain-text tool responses are left unchanged because the strip only
  runs when the first non-empty line starts with `mf_fetch:`.

### Fixed — `/research create` no longer crashes on html2text renderer panics

- `masterfetch::extractor::extract_markdown` now wraps the full readability →
  html2text → raw-text chain in a top-level `std::panic::catch_unwind`. If
  `html2text` panics on real-world HTML (e.g. mdBook-generated pages such as
  `rust-book.cs.brown.edu`), the extractor degrades to raw tag-stripped text
  instead of aborting the web-gatherer task or the whole ragent process.
- Added a regression test in `test_mf_extractor.rs` using a captured mdBook HTML
  fixture; it triggers the known html2text overflow and verifies the extractor
  returns a non-empty fallback result.

### Added — PDF and YouTube text extraction in `/research create`

- `mf_fetch` now extracts readable text from PDF responses instead of returning
  raw binary bytes. It detects `Content-Type: application/pdf` and `.pdf` URLs,
  runs `pdf_extract::extract_text_from_mem` in a blocking task, and surfaces the
  document title from the PDF `/Info` dictionary (with UTF-16BE BOM handling).
- `mf_fetch` now extracts timestamped captions from YouTube watch pages. It
  parses the embedded `ytInitialPlayerResponse` JSON, selects the best caption
  track (default, then English, then first available), fetches the caption XML,
  and formats each caption with a `[MM:SS]` timestamp.
- PDF and YouTube `mf_fetch` outputs set `page_type: pdf` / `page_type: youtube`
  and include `content_type` in metadata so the research layer classifies and
  counts them correctly.
- Updated `WebSourceKind::YouTube` documentation to reflect that transcript
  extraction is now implemented.
- `WebGatherer::fetch_url_as_source` now classifies `--from-url` seed sources by
  their fetched `content_type` / URL instead of always reporting `media_type: page`.
- Added `crates/ragent-tools-extended/tests/test_mf_pdf.rs` and
  `crates/ragent-tools-extended/tests/test_mf_youtube.rs` covering PDF text/title
  extraction, YouTube caption parsing, and end-to-end transcript fetch against a
  local mock caption server.
- Added `ragent-research` tests verifying that `fetch_url_as_source` and
  `gather_with_observer` classify and count PDF and YouTube sources correctly.

### Added — `/research create` richer fetch metadata and PDF/YouTube counters

- Research web gathering now prefers the `mf_fetch` tool over legacy `webfetch`.
  `mf_fetch` returns a structured envelope with `content_type`, `page_type`, and
  `metadata.title`, giving the research layer reliable media classification and
  better page titles.
- Added `WebFetchedPage.content_type` and `WebFetchedPage.page_type` to carry
  the richer `mf_fetch` metadata through the gatherer.
  - Added `Source::Web` fields `content_type`, `page_type`, and `media_type`
    (`"page" | "pdf" | "youtube"`) with serde defaults so older `RESEARCH.md`
    files remain backward compatible.
  - Added `Source::media_type()` and a new **Media** column in the
    `RESEARCH.md` References Index table so every source shows its classified
    media type (`page`, `pdf`, `youtube`) alongside the existing Type column.
    Non-web sources render `—`.
- New `WebSourceKind` enum and `classify_web_source()` helper classify sources
  from `Content-Type: application/pdf`, `.pdf` URLs, and YouTube hosts.
- `GatherResult`, `SessionEvent::Done`, `RunOutcome`, and the TUI
  `ResearchProgress` tracker now carry `pdf_count` and `youtube_count`.
- CLI `ragent research create` prints recovered PDF and YouTube counts in the
  completion summary (e.g. "created research/foo (5 sources, 2 PDFs, 1 YouTube
  video)").
- TUI `/research create` progress summary now shows PDF and YouTube counts both in
  the live tracker render and in the status-bar completion message.
- Research event JSON output (`render_session_event_json`) includes
  `pdf_count` and `youtube_count` in the `done` payload.
- Added `url` crate dependency to `ragent-research` for host-based YouTube
  classification.

## Version: 0.1.0-beta.11

### Changed — Version bump

- Workspace version bumped from `0.1.0-beta.10` to `0.1.0-beta.11`.
- Moved Tavily search backend into the `mf_search` multi-engine framework.

## Version: 0.1.0-beta.9

### Added — TUI `/websearch` diagnostics

- New TUI slash command `/websearch show` lists every configured web-search
  backend (DuckDuckGo, Brave, LangSearch, Tavily) with `enabled`, `in_use`, and
  `failed` status columns.
- `/websearch help` prints usage and subcommand help.
- Command is registered in the TUI slash menu with autocomplete suggestions for
  `show` and `help`.
- `MfSearchTool::engine_status()` exposes the same status table programmatically
  so the TUI and tests share one source of truth.
- SPEC.md slash-command table updated with `/websearch show|help` and `/webapi`.

### Added — Tavily backend for `mf_search` and research migration

- New `TavilyEngine` in `crates/ragent-tools-extended/src/masterfetch/search/tavily.rs`
  implementing the `SearchEngine` trait. It calls `https://api.tavily.com/search`
  with `Authorization: Bearer {key}`, maps `SearchOptions` to Tavily JSON fields
  (`query` truncated to 400 chars, `max_results` clamped 1–20, `include_answer: false`),
  parses the `results` array, and reports non-2xx responses as `engine_blocked`.
- `mf_search` now includes the Tavily backend automatically when `tavily_api_key`
  is configured in `ragent.json` or `TAVILY_API_KEY` is set; it runs in parallel
  with the existing DuckDuckGo, Brave, and optional LangSearch backends.
- `mf_search` description updated to mention Tavily and both optional API keys.
- Legacy `websearch` tool is retained for direct agent use and now documents that
  research workflows prefer `mf_search`.
- `ragent-research` now depends on `ragent-tools-extended` so the research layer
  can understand the `mf_search` metadata shape.
- `AgentWebSearchTool` now prefers the `mf_search` tool when available and falls
  back to `websearch`, mapping structured metadata and plain-text output into
  `WebSearchHit` while preserving `search_tool` and `search_engine` provenance.
- New helper `parse_mf_search_metadata` converts `mf_search` JSON metadata into
  research-layer hits, falling back from `search_engine` to `source` to the
  tool name.
- TUI `tool_input_summary` and `tool_result_summary` now handle `mf_search` like
  `websearch`, and the tool-category doc comment lists `mf_search`.

### Added — Research source provenance

- `WebSearchHit` and `Source::Web` now carry `search_tool` and `search_engine`
  fields so every web source produced by the research system records *which*
  search tool (e.g. `mf_search`, `websearch`) and backend engine(s) (e.g.
  `tavily`, `duckduckgo, brave`) discovered the URL.
- `AgentWebSearchTool` populates provenance from the `websearch` tool's
  structured metadata and from the text fallback parser; the `websearch`
  `SearchResult` now emits `search_tool`/`search_engine` defaults, and
  `mf_search` metadata includes them per result.
- `WebGatherer` propagates provenance into `Source::Web`, and `GatherEvent::SourceCaptured`
  forwards it so both the non-iterative and iterative research engines surface it.
- `SessionEvent::WebCaptured` now includes `search_tool`/`search_engine`; the CLI
  JSON renderer and TUI progress encoder display provenance in capture lines
  (e.g. `captured https://example.com via websearch (tavily) — Title`).
- `ResearchIo::render_references_index` adds **Search tool** and **Engine**
  columns to the References Index table in `RESEARCH.md`.
- All new fields use `serde(default)` so existing `RESEARCH.md` files and older
  metadata load without migration.

### Fixed

- `/websearch test` no longer panics from nested Tokio runtime. The TUI slash
  command now runs the async `MfSearchTool::engine_test()` inside
  `tokio::task::block_in_place`, matching the pattern used by `/spec validate`.

## Version: 0.1.0-beta.9
### Added — LangSearch backend for `mf_search`

- New optional `langsearch_api_key` top-level config field in `ragent.json`.
  When set, the key is merged across global/project/env config layers and
  serialised back only when explicitly present (defaults omit the key).
- New `LangSearchEngine` in `crates/ragent-tools-extended/src/masterfetch/search/langsearch.rs`
  implementing the `SearchEngine` trait. It calls `https://api.langsearch.com/v1/web-search`
  with `Authorization: Bearer {key}`, maps `SearchOptions` to LangSearch JSON
  fields (`query`, `count` clamped 1–10, `freshness`, `summary: true`), parses
  `data.webPages.value`, and reports non-2xx responses as `engine_blocked`.
- `mf_search` now includes the LangSearch backend automatically when a key is
  configured; existing keyless DuckDuckGo and Brave backends continue to work
  when no key is present.
- API key is masked in diagnostics and never logged or surfaced in error
  messages.
- Tests: request/response mapping and key masking unit tests, config
  merge/load/serialise tests, and an `#[ignore]`-gated live API test.

## Version: 0.1.0-beta.8

### Changed — Version bump

- Workspace version bumped from `0.1.0-beta.7` to `0.1.0-beta.8`.
- `cargo check` passes cleanly with the new version.

## Version: 0.1.0-beta.7

### Changed — Version bump

- Workspace version bumped from `0.1.0-beta.5` to `0.1.0-beta.6`.
- `cargo check` passes cleanly with the new version.

## Version: 0.1.0-beta.5

### Added — Live telemetry reconfiguration, agent metric recording, and sudo askpass broker
- `/telemetry on|off` now reconfigures the live `TelemetrySubsystem` in place
  (shuts down the meter provider on `off`, builds a fresh one on `on`) so the
  toggle takes effect immediately instead of requiring a restart. The
  subsystem's runtime state is held behind a `parking_lot::Mutex` and the
  provider wrapped in `Arc` for safe interior mutability.
- New `ragent-agent` telemetry module (`LlmRecorder`, `SessionRecorder`,
  `ToolRecorder`) records LLM call duration, tool invocation counts/durations,
  session start/end, and agent-loop timing into the telemetry subsystem.
  `SessionProcessor` is wired to the subsystem via an `Arc<TelemetrySubsystem>`.
- New `askpass` module in `ragent-tools-core` routes `sudo` password prompts
  through ragent's interactive question dialog instead of hanging on the
  controlling tty. The bash tool now detaches stdin (`Stdio::null()`) and sets
  `SUDO_ASKPASS` environment variables when a broker is active.
- `ragent-telemetry` re-exports `LlmRecorder`, `SessionRecorder`, and
  `ToolRecorder` for cross-crate use.
- `ShutdownGuard` keeps the meter provider alive for the process lifetime and
  flushes pending metrics on normal or panic exit paths.
- Telemetry panel rendering and `/telemetry` slash-command code reformatted
  (indentation and trailing-newline fixes).

## Version: 0.1.0-beta.4

### Added — Telemetry panel styling and release tooling

- Telemetry metric type labels now render in bold blue for better visual
  distinction in the ALT-O Telemetry panel.
- Automated release skill increments workspace version, updates release notes,
  and tags the repository.

## Version: 0.1.0-beta.3

### Added — Context-window compaction and `/config save`

- New context-window compaction pipeline replacing the Headroom-based compression
  scheme. Includes `compaction` config block, `/compact` slash command (with
  `/compress` alias), `CompactionStarted/CompactionFinished` events, and
  Unicode-safe truncation.
- `/config save` and `/config list` slash commands for backing up and restoring
  global `ragent.json`.
- Updates to telemetry counters and TUI wiring.

### Removed — Headroom dependency, CCR store, and compression pipeline

- Dropped the `headroom-core` git dependency, deleted the `compression` modules,
  removed CCR markers and the `headroom_retrieve` bridge, and added a legacy
  `compression` → `compaction` config alias.

## Version: 0.1.0-beta.2

### Added — Telemetry (OTEL) and ALT-O Telemetry panel

- OpenTelemetry metrics export (`/telemetry` slash command family: `help`, `on`, `off`, `setup`, `counters`) for managing OTLP endpoints, protocol, export interval, timeout, and an internal Prometheus port.
- TUI **ALT-O Telemetry panel** for live OpenTelemetry metrics and counter inspection.
- Configuration schema and TUI wiring for telemetry settings in `ragent.json`.

## Version: 0.1.0-beta.1

### Changed — Transition to beta channel

- Workspace version bumped from `0.1.0-alpha.147` to `0.1.0-beta.1`, marking
  the transition from the alpha pre-release channel to the beta pre-release
  channel.

### Added

- TUI `/telemetry` slash command family (`help`, `on`, `off`, `setup`,
  `counters`) for managing OpenTelemetry metrics export, including a full
  multi-field setup dialog for endpoint, protocol, export interval, timeout,
  and internal Prometheus port.

## Version: 0.1.0-alpha.147

### Fixed — Model Router no longer forces a vision model for text-only follow-ups

- `extract_attachments()` in `crates/ragent-llm/src/providers/router_client.rs` now
  scans **only the most recent user message** for image/video attachments instead of
  the entire conversation history.
- Previously, once an image was sent in a conversation, every subsequent prompt
  was treated as `requires_vision`, which caused the router to keep selecting a
  vision-capable model even when the current user prompt had no attachment.
- Now a text-only follow-up correctly re-classifies and selects the first model
  in the resolved tier (e.g. a non-vision `glm-5.2` listed above a vision variant).
- Added regression test
  `test_router_text_followup_after_image_uses_non_vision_model` in
  `crates/ragent-llm/tests/test_router_client.rs`.

### Fixed — Selecting Model Router in the provider picker now opens the router setup UI

- In the provider setup dialog (`/provider`), choosing **Model Router** when it
  was already marked as configured previously fell through to the generic model
  picker, which showed a useless single-entry "Model Router" list and offered no
  way to reconfigure the cluster. The provider picker now detects this case and
  opens the **Model Router cluster setup panel** instead.
- The empty-cluster guard is preserved: if no concrete providers are configured,
  the picker stays open with a warning so the user can set up a downstream provider
  first.
- Added a regression test in `crates/ragent-tui/tests/test_router_setup.rs`
  (`test_provider_picker_already_configured_router_opens_setup_router`).

### Fixed — Model Router classification now appears in the TUI log panel

- `RouterProvider` now overrides the `Provider::set_event_bus` trait default so
  that the TUI's `provider_registry.set_event_bus_all()` call actually reaches
  the router. Previously the router kept its event bus as `None` even though
  other providers were wired, so `Event::RouterClassification` was never
  published and the classification/bucket/model selection stayed invisible in
  the Logging Window.
- The router now also publishes classification events via a plain
  `tracing::info!()` record, so the summary appears in the log panel regardless
  of whether the event-bus bridge is active.

### Fixed — Model Router no longer defaults to Anthropic for Medium/Complex/Reasoning tiers

- `default_tier_config()` now uses local-first `ollama` models for every tier:
  - `SIMPLE`: `qwen3:0.6b`, `llama3.2`
  - `MEDIUM`: `qwen3:1.7b`, `qwen2.5:7b`
  - `COMPLEX`: `qwen3:4b`, `qwen2.5:14b`
  - `REASONING`: `qwq:32b`, `deepseek-r1:14b`
- This removes the silent `anthropic` fallbacks for `MEDIUM`, `COMPLEX`, and
  `REASONING` that produced 401 Unauthorized errors when no Anthropic API key
  was configured.
- If the resolved tier has no configured models, the router now falls back to
  higher tiers first, then lower tiers, before giving up. This keeps a
  partially configured cluster (e.g. only `SIMPLE` defined) working for prompts
  that classify into any other bucket.

### Fixed — Default model no longer hard-wires Anthropic/Claude

- `create_default_registry()` now registers **local/self-hosted providers first**
  (`ollama`, `ollama_cloud`, `generic_openai`, `huggingface`, `azure_resource`,
  `azure_foundry`, `copilot`) before cloud providers (`openai`, `gemini`, `xai`,
  `anthropic`, `bedrock`). The built-in `RouterProvider` remains last.
- As a result, when no model is explicitly selected the fallback resolves to the
  first available *local* provider/model (e.g. `ollama/...`) instead of
  `anthropic/claude-sonnet-4-20250514`.
- This affects all paths that call `resolve_default_model` /
  `resolve_agent_with_model`: TUI startup, `ragent run`, `ragent serve`,
  `POST /sessions/{id}/messages`, and the AGENTS.md init exchange.
- Router built-in tiers are left unchanged but are only active when the user
  explicitly enables the router (`/router on` or custom `provider.router` config).

### Added — Model Router classification logging and virtual model discovery

- The router now logs the **classified prompt**, **selected bucket/tier**,
  **selected downstream model**, **composite score**, and **active classifier
  dimensions** every time it routes a request. This information is published as
  an `Event::RouterClassification` so it appears in the TUI log panel even when
  the tracing filter is set to the default `warn` level.
- `RouterProvider` now exposes a single virtual model (`Model Router`), so the
  model picker and discovery flow show the router as a valid selection instead
  of reporting "No models are currently available for this provider".

### Added — Model Router save confirmation dialog

- **Ctrl+S in the Model Router setup dialog now opens a confirmation modal**
  instead of saving immediately. The modal shows the number of tier entries that
  will be saved and prompts the user to press Enter to confirm or Esc to cancel.
- **Confirming the dialog** persists the draft cluster to `ragent.json`,
  enables the router, and selects `router/router` as the active model.
- **Cancelling the dialog** clears the pending save and returns to the router
  setup dialog without writing to disk.
- **New tests** in `crates/ragent-tui/tests/test_router_save_dialog.rs` cover
  both confirming and cancelling the save confirmation dialog.

### Fixed — Model Router save confirmation dialog visibility

- The router save confirmation modal is now rendered **after** the router setup
  dialog so it appears on top instead of being painted over. Previously the
  confirmation was invisible because `render_provider_setup_dialog` was drawn
  later and covered the modal, which also meant users could not confirm the
  save and the router cluster was not persisted.
- Added a regression test in `crates/ragent-tui/tests/test_router_setup.rs`
  (`test_router_save_confirmation_renders_above_setup_dialog`) that renders
  both dialogs together and asserts the save confirmation title and hint are
  visible.

### Fixed — Router provider no longer demands an API key

- `SessionProcessor::resolve_api_key` now returns an empty key immediately for
  the virtual `router` provider. The router delegates authentication to its
  downstream providers, so it does not require its own API key. This prevents the
  spurious `"No API key found for provider 'router'"` error when the Model
  Router is selected.

### Fixed — "error decoding response body" retry loop for local/OpenAI-compatible providers

- **OpenAI-compatible providers (OpenAI, Azure Foundry, Generic OpenAI) now detect
  an empty/malformed SSE body immediately** and emit a clear, non-retryable error
  (`"... returned an empty/malformed event stream ... model is not loaded"`) instead
  of the raw reqwest `"error decoding response body"` diagnostic. The transform fires
  when a chunk decode failure happens before any events have been yielded and the HTTP
  status was successful, which is the typical signature of a local model that is not
  loaded or an endpoint that returned a non-stream body.
- **Ollama provider applies the same empty/malformed stream detection** in its SSE
  parser, using captured response status and content-type to produce a clear local
  model error message.
- **Retry policy no longer retries raw `"error decoding response body"` before any
  output has been received.** `should_retry_stream_error` now treats that specific
  early decode failure as fatal, while still preserving partial output if the error
  occurs mid-stream.
- **TUI status-bar label now reflects the provider actually handling the request.**
  It shows `"Model Router"` only when the active model ref points to the `router`
  provider; if the router is enabled but a concrete model is still selected, the
  label falls back to that concrete provider's name.

### Changed — Model Router setup UI layout and model-property display

- **Router cluster buckets now render as a 2×2 grid** (two rows of two buckets)
  instead of a single row of four columns, improving readability on narrower
  terminals. The four tiers are laid out in ascending complexity order:
  `SIMPLE` | `MEDIUM` on the top row, `COMPLEX` | `REASONING` on the bottom row.
- **Bucket titles now show the full tier name** (e.g. `SIMPLE`, `REASONING`)
  instead of the previous single-character abbreviation (`S`, `M`, `C`, `R`).
- **Retained model properties are displayed inside each bucket.** Each assigned
  model entry now renders its context window, feature flags (`R`/`V`/`T`),
  thinking levels, cost tier/multiplier, and registry cost estimate, in addition
  to the `provider / model` label. Properties are resolved at render time via a
  new `App::router_model_picker_entry` helper that prefers cached/discovered
  model metadata and falls back to the provider registry's default catalog.
- **Router model picker upgraded to a full properties table.** The
  `SelectRouterModel` dialog now renders the same `Model | Context | Cost |
  Thinking | Features` table used by the standard model picker, so users can
  compare model properties before assigning a sub-model to a tier bucket.
- **New tests** added to `crates/ragent-tui/tests/test_router_setup.rs` covering
  the full tier-name bucket titles, retained property rendering, and the
  picker property-column rendering.

## Version: 0.1.0-alpha.146

### Added — Model router baseline support with TUI

- Baseline model router support with TUI setup flow for selecting providers and
  assigning them to routing tiers (`SIMPLE`, `MEDIUM`, `COMPLEX`, `REASONING`).

### Changed

- **Workspace version** — Bumped to `0.1.0-alpha.146`.

### Removed — Rig framework integration

- **Deleted the `ragent-rig` workspace crate** and the entire Rig (`rig-core`)
  integration, including Rig-backed providers, embeddings, vector stores,
  conversation-memory policies, semantic code index, semantic memory, and the
  Rig-backed research augmentor.
- **Removed Rig-only abstraction points:**
  - `crates/ragent-agent/src/session/semantic_handles.rs`
  - `crates/ragent-research/src/semantic.rs`
  - `crates/ragent-research/tests/test_research_semantic.rs`
- **Removed Rig-specific documentation and specs:**
  - `docs/howtos/rig-integration.md`
  - `docs/reports/rig-interface-audit.md`
  - `docs/reports/rig-binary-size-compile-time-impact.md`
  - `specs/rig/SPEC.md`
  - `specs/rig/PLAN.md`
- **Removed generated Rig research artifacts** under `research/rig/`.
- **Native providers and native memory/code-index/search remain unchanged;**
  `memory_search` and `codeindex_search` continue to use their existing
  non-Rig implementations.

## Version: 0.1.0-alpha.145

### Added — Router Provider TUI Setup (spec: routeui)

- **Interactive Model Router configuration panel** reachable via `/provider` → `Model Router`
  or `/provider router`. Users can select multiple already-configured concrete providers,
  assign provider/model pairs to the four routing tiers (`SIMPLE`, `MEDIUM`, `COMPLEX`,
  `REASONING`), and save the cluster to `ragent.json`.
- **Router setup state machine** added to `ProviderSetupStep` with `SetupRouter` and
  `SelectRouterModel` variants, reusing the existing provider-setup overlay.
- **Two-pane router UI** — provider multi-selection list on the left, four bucket columns
  on the right, with keyboard navigation (Tab, arrows, Space, Enter, Ctrl+S, Esc).
- **Model picker dialog** for choosing which model from a selected provider is assigned
  to the active tier bucket.
- **Persistence and validation** — `Ctrl+S` saves `provider.router.tiers` to `ragent.json`,
  enables the router, preserves existing classifier weights/boundaries, rejects recursive
  router-to-router assignments, and requires at least one non-empty tier.
- **Bucket reordering** — `Ctrl+↑` / `Ctrl+↓` moves the selected model within a tier.
- **Cost estimates** — each bucket entry shows a per-model `~$/M` estimate when pricing
  metadata is available from the provider registry.
- **`/provider show` now renders the router cluster** with each tier and its assigned
  provider/model entries.
- **Status bar label** displays `"Model Router"` when the router virtual provider is active.
- **Spec and tests** — added `specs/routeui/SPEC.md`/`PLAN.md` and
  `crates/ragent-tui/tests/test_router_setup.rs` covering the provider helper, state
  machine, input handling, persistence, validation, `/provider` integration, and report
  rendering.

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
    `AgentManager.has_pending_background` AtomicBool skips drain scans;
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
- **Task tool family guidance** — Added a dedicated `## Task Tool Family` section to every primary agent's system prompt that clearly distinguishes `agent_complete` (autonomous loop signal — only takes `summary`) from `team_task_complete` (team workflow — only takes `team_name` + `task_id`).  The `agent_complete`, `team_task_complete`, and `new_agent` tool descriptions and JSON schemas now explicitly warn against the most common parameter-confusion mistakes and reject unknown keys via `additionalProperties: false`.  `agent_complete` and `list_agents` are now hardwired auto-approved so the agent can always finish or inspect background tasks without a permission prompt.

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
- **Multiple tool refinements** — Updated codeindex_search, list_agents, memory_search, office_write, and spec_list tools.
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

