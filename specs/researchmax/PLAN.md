# Concept and finding output limits — Implementation Plan

This plan implements the requirements in `SPEC.md` for spec `researchmax`.
The overall strategy is:

1. Define the default limits and the reverse-relevance ordering helper so both
   concepts and findings share one ranking implementation.
2. Extend the analysis config and request plumbing with the two limits.
3. Apply the finding cap after synthesis and the concept cap after extraction,
   with contiguous renumbering.
4. Expose `--max-concepts` / `--max-findings` on the CLI, hand parser, TUI, and
   server, plus `ragent.json` defaults.
5. Add tests, update docs, and verify no regressions.

## Tasks

| ID | Title | Requirement | Effort | Priority | Status | Dependencies |
|----|-------|-------------|--------|----------|--------|--------------|
| T-001 | Define default limits and shared reverse-relevance ordering helper | FR-001, FR-002, FR-005, NFR-001 | M | Critical | completed | — |
| T-002 | Add`max_concepts` and `max_findings` to `AnalysisConfig` | FR-008, FR-014 | S | High | completed | T-001 |
| T-003 | Cap and renumber findings inside`AnalysisResult` merge/synthesis path | FR-003, FR-010, FR-020, FR-022 | M | Critical | completed | T-001, T-002 |
| T-004 | Cap and renumber generated concepts before section assembly | FR-004, FR-011, FR-018 | M | Critical | completed | T-001, T-002 |
| T-005 | Parameterise the concept-extraction prompt with the effective limit | FR-001, FR-015 | S | Medium | completed | T-001 |
| T-006 | Plumb`max_concepts`/`max_findings` through `ResearchRunRequest` and `build_session_config` | FR-012, FR-014 | M | High | completed | T-002 |
| T-007 | Add`--max-concepts` / `--max-findings` to the research hand parser and help | FR-006, FR-007, FR-019, NFR-003 | M | High | completed | T-006 |
| T-008 | Add`--max-concepts` / `--max-findings` to the root clap CLI | FR-006, FR-007, FR-019 | S | High | completed | T-007 |
| T-009 | Add`research.max_concepts` / `research.max_findings` to `ragent.json` config | FR-008, FR-017 | M | High | completed | T-002 |
| T-010 | Add HTTP`max_concepts`/`max_findings` fields and invocation summary parity | FR-012, FR-013 | S | Medium | completed | T-006 |
| T-011 | Update TUI completion and parameter hints for the two flags | FR-006, FR-007, NFR-003 | S | Medium | completed | T-007 |
| T-012 | Add tests for ordering, truncation, zero/unbounded, and config defaults | FR-001, FR-003, FR-004, FR-015, FR-016, FR-022, NFR-002 | M | High | completed | T-003, T-004, T-006, T-009 |
| T-013 | Add CLI/hand-parser/server round-trip tests for the new arguments | FR-006, FR-007, FR-012, FR-013, FR-019 | M | High | completed | T-007, T-008, T-010 |
| T-014 | Update docs (`research.md`, `slashcommands/research.md`, `/research help`) and CHANGELOG | FR-008, NFR-003 | S | Low | completed | T-007, T-011 |
| T-015 | Run`cargo fmt`, `cargo clippy`, and `cargo test`; verify acceptance criteria | NFR-002, NFR-004, All | M | High | completed | T-001, T-002, T-003, T-004, T-005, T-006, T-007, T-008, T-009, T-010, T-011, T-012, T-013, T-014 |
## Task Details

### T-001 — Define default limits and shared reverse-relevance ordering helper

- Add `pub const DEFAULT_MAX_CONCEPTS: usize = 5;` and
  `pub const DEFAULT_MAX_FINDINGS: usize = 20;` in `crates/ragent-research/src/lib.rs`
  (or `session.rs`, next to the other research defaults).
- Add one helper (for example `rank_entries_by_reverse_relevance`) that accepts
  a list of `(body, cited_ranks)` entries and sorts by
  `(max cited rank desc, cited count desc, original index asc)`.
- Add a citation-extraction routine that parses `[#N]` and `web-NN` markers out
  of a text body and maps them to source ranks via the existing
  `Source::relevance_rank`.
- Entries with no recognized citations receive rank 5 (FR-005).

### T-002 — Add `max_concepts` and `max_findings` to `AnalysisConfig`

- Add `pub max_concepts: usize` and `pub max_findings: usize` to
  `AnalysisConfig` in `session.rs`, defaulting to the constants from T-001.
- Update every `AnalysisConfig` literal in the crate and tests; prefer adding a
  `Default` impl usage so future fields do not require churn.
- Document `0` = unbounded (FR-016).

### T-003 — Cap and renumber findings inside `AnalysisResult`

- After synthesis returns `AnalysisResult`, order `findings` by reverse
  relevance using the T-001 helper, then truncate to `max_findings` (unless 0).
- Renumber surviving findings contiguously using the existing
  `renumber_findings` logic so `Finding N` labels stay sequential (FR-010).
- Guarantee at least one finding survives when the model/fallback produced any
  (FR-020); never pad when fewer are available (FR-022).
- Apply the same pass in the supervisor finalization path so both session modes
  behave identically.

### T-004 — Cap and renumber generated concepts before section assembly

- In `extract_concepts_inner` / `concepts_section_for_research`, split the
  normalized response into `###` concept sections, order them by reverse
  relevance using the T-001 helper, keep the top `max_concepts` (unless 0), and
  renumber the surviving headings contiguously from 1.
- Preserve the existing behaviour of returning `None` when no concept sections
  exist (FR-018).
- Keep the `## Concepts` / `### Concepts` heading structure unchanged (FR-009).

### T-005 — Parameterise the concept-extraction prompt with the effective limit

- Replace the hardcoded "up to 20 core concepts" sentence in
  `CONCEPT_EXTRACTION_PROMPT_TEMPLATE` with a placeholder (for example
  `[MAX_CONCEPTS]`) substituted at call time.
- Keep a default substitution so direct callers of the template still compile;
  update the existing prompt test to assert the new max is injected.

### T-006 — Plumb the limits through `ResearchRunRequest` and `build_session_config`

- Add `pub max_concepts: Option<usize>` and `pub max_findings: Option<usize>` to
  `ResearchRunRequest` in `run_request.rs`.
- In `build_session_config`, resolve each to flag value → config value →
  built-in default and write the result into `AnalysisConfig` (FR-012, FR-014).

### T-007 — Add the flags to the research hand parser and help

- Add `--max-concepts` and `--max-findings` to `parse_create` in
  `crates/ragent-research/src/cli.rs`, including them in the value-taking
  argument list, the parse match arms, the returned variant fields, and the
  `build_help_message` flag list (NFR-003).
- Reject malformed values with a clear error rather than silently defaulting
  (FR-019).

### T-008 — Add the flags to the root clap CLI

- Add `max_concepts: Option<usize>` and `max_findings: Option<usize>` fields
  with `#[arg(long = "max-concepts")]` / `#[arg(long = "max-findings")]` to the
  root `ResearchCommands::Create` variant in `src/cli.rs`.
- Forward them into `ResearchCliCommand::Create` alongside the other
  `max_*` fields.

### T-009 — Add the config keys to `ragent.json`

- Add `max_concepts` (default 5) and `max_findings` (default 20) to
  `ResearchConfig` in `crates/ragent-config/src/config.rs` with
  `#[serde(default = "...")]` accessors.
- Update `ResearchConfig::is_empty` and the config merge and tests so the keys
  round-trip and are omitted from serialization at their defaults (FR-008).

### T-010 — Add HTTP fields and invocation-summary parity

- Add `max_concepts` / `max_findings` (optional, serde default) to
  `CreateResearchRequest` in `crates/ragent-server/src/routes/research.rs`.
- Forward them in `to_run_request()` and emit
  `--max-concepts N` / `--max-findings N` in `invocation_summary()` only when
  set, so the recorded invocation replays (FR-012, FR-013).

### T-011 — Update TUI completion and parameter hints

- Add `--max-concepts` and `--max-findings` to the `/research` suggestion list
  in `crates/ragent-tui/src/app/slash.rs` and the parameter hint in
  `session_ops.rs`.

### T-012 — Add ordering/truncation/config tests

- Unit tests for the ordering helper: reverse relevance, tie-break by cited
  count then original order, unknown-citation default rank, empty list.
- Tests that findings and concepts are truncated to the limit, renumbered
  contiguously, and keep the highest-relevance entries.
- Tests for `0` = unbounded and for "fewer entries than the limit" (no padding).
- Config tests for the new `research.*` defaults and override precedence.

### T-013 — Add CLI/parser/server round-trip tests

- Root CLI test asserting both flags parse and a malformed value fails.
- Hand-parser tests for `parse_create` accepting the flags and for the help
  text listing them.
- Server tests asserting `max_concepts`/`max_findings` reach
  `ResearchRunRequest` and appear in (and replay through) the invocation summary.

### T-014 — Update docs and CHANGELOG

- Document `--max-concepts` / `--max-findings`, their defaults (5 / 20),
  ordering semantics, and the `0` = unbounded rule in
  `docs/howtos/research.md` and `docs/howtos/slashcommands/research.md`.
- Add an Unreleased `Added` entry to `CHANGELOG.md`.

### T-015 — Verify

- Run `cargo fmt --check`, `cargo clippy --workspace --all-targets`, and the
  `ragent-research`, `ragent-config`, `ragent-server`, and `ragent-tui` test
  suites, then confirm each SPEC acceptance criterion 1–6.