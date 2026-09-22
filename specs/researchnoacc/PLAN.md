# Research scholarly-engine exclusion — Implementation Plan

This plan implements the requirements in `SPEC.md` for spec `researchnoacc`.
The overall strategy is:

1. Add a reusable academic-engine vocabulary and an orchestrator exclusion
   primitive in `ragent-tools-extended`.
2. Expose `exclude_engines` on the `mf_search` tool.
3. Plumb an exclusion parameter through the research `WebSearchTool` trait and
   the gatherer, driven by `--no-papers` / config.
4. Fix the `--no-papers` spelling across CLI, TUI, and server.
5. Add tests, update docs, and verify no regressions.

## Tasks

| ID | Title | Requirement | Effort | Priority | Status | Dependencies |
|----|-------|-------------|--------|----------|--------|--------------|
| T-001 | Define academic engine-name constants and classification helper | FR-003, FR-015 | S | High | completed | — |
| T-002 | Add `SearchOrchestrator::exclude_engines` method | FR-002, FR-011 | S | Critical | completed | — |
| T-003 | Add `exclude_engines` to `mf_search` schema and execution | FR-001, FR-007, FR-008 | M | Critical | completed | T-001, T-002 |
| T-004 | Handle the all-engines-excluded case explicitly | FR-014, NFR-004 | S | High | completed | T-003 |
| T-005 | Add orchestrator and `mf_search` exclusion tests | FR-001, FR-002, FR-008, FR-014, NFR-003 | M | High | completed | T-003, T-004 |
| T-006 | Extend research `WebSearchTool` trait with an exclusion parameter | FR-004 | M | Critical | completed | T-003 |
| T-007 | Translate research exclusion to `mf_search` `exclude_engines` in the adapter | FR-004, FR-006 | M | Critical | completed | T-006 |
| T-008 | Drive engine exclusion from `WebGatherer.disable_scholarly` | FR-006, FR-010, NFR-001 | M | Critical | completed | T-006, T-007 |
| T-009 | Add `research.exclude_academic_engines` config with per-run precedence | FR-012 | M | High | completed | T-008 |
| T-010 | Fix `--no-papers` / `--no-scholarly` flag spelling on root CLI and hand parser | FR-005 | S | High | completed | — |
| T-011 | Add `no_scholarly` field to `POST /research` and invocation summary | FR-009 | S | High | completed | T-010 |
| T-012 | Update TUI completion and help text for `--no-papers` | FR-005 | S | Medium | completed | T-010 |
| T-013 | Add research gatherer tests for engine-level exclusion | FR-006, FR-010, NFR-003 | M | High | completed | T-008 |
| T-014 | Update docs (`research.md`, `config.md`, `mf_search` description) and CHANGELOG | NFR-004 | S | Low | completed | T-003, T-009, T-011 |
| T-015 | Run `cargo fmt`, `cargo clippy`, and `cargo test`; verify acceptance criteria | NFR-002, All | M | High | completed | T-001–T-014 |
## Task Details

### T-001 — Define academic engine-name constants and classification helper

- Add `pub const ENGINE_OPENALEX: &str = "openalex";` (and any sibling names
  needed) plus an `is_academic_engine(name: &str) -> bool` helper in the
  masterfetch search module.
- Reference the constant from the research classification
  (`is_scholarly_hit` in `web_gatherer.rs`) so the string literal exists once.

### T-002 — Add `SearchOrchestrator::exclude_engines`

- Add `exclude_engines(&self, names: &[&str]) -> SearchOrchestrator` next to
  `select_engine` in `search/mod.rs`, filtering `self.engines` by `name()`.
- Return a new orchestrator with an empty cache (matches `select_engine`).

### T-003 — Add `exclude_engines` to `mf_search` schema and execution

- Extend `parameters_schema` in `search_tool.rs` with the `exclude_engines`
  array.
- In `execute`, parse it and call `orchestrator.exclude_engines(...)`.
- Ignore unknown names (FR-008).

### T-004 — Handle the all-engines-excluded case explicitly

- When the filtered orchestrator has zero engines, return an explicit result
  stating all engines were excluded; do not dispatch requests.
- Reflect the skip in metadata/logs (NFR-004).

### T-005 — Add orchestrator and `mf_search` exclusion tests

- Unit-test `exclude_engines` (removes named engines, keeps others, unknown
  names ignored, empty list is identity).
- Test `mf_search` with `exclude_engines: ["openalex"]` returns no openalex
  rows and with all engines excluded returns the explicit result.

### T-006 — Extend research `WebSearchTool` trait

- Add an exclusion parameter to `WebSearchTool::search` (or a companion method)
  carrying engine names to skip.
- Update the trait's default/in-memory implementations and test doubles.

### T-007 — Translate research exclusion to `mf_search` `exclude_engines`

- In the production adapter that builds the `mf_search` call, map the exclusion
  parameter onto the tool's `exclude_engines` argument.

### T-008 — Drive engine exclusion from `WebGatherer.disable_scholarly`

- When `disable_scholarly` is set, pass the academic engine names through the
  trait to the search call so OpenAlex is never queried.
- Keep the existing hit-level filter as defence in depth.

### T-009 — Add `research.exclude_academic_engines` config

- Add the boolean to the research config block with the precedence chain:
  flag wins, else config, else off.

### T-010 — Fix the flag spelling

- Reconcile the root CLI (`--no-scholarly`) and the hand parser
  (`--no-papers`); accept `--no-papers` everywhere (add a visible alias where
  needed) so the documented flag works.

### T-011 — `POST /research` parity

- Add a `no_scholarly: Option<bool>` field to `CreateResearchRequest` and emit
  the flag in `invocation_summary`, mirroring `oa_recovery`.

### T-012 — TUI completion and help

- Ensure `/research create` completion and help list `--no-papers`.

### T-013 — Research gatherer tests

- Assert that with `disable_scholarly` set, the search tool receives the
  academic exclusion and no openalex engine is queried, while non-academic
  results still gather.

### T-014 — Documentation

- Update `docs/howtos/research.md`, `docs/howtos/config.md`, the `mf_search`
  tool description, and `CHANGELOG.md`.

### T-015 — Verification

- Run `cargo fmt`, `cargo clippy`, and `cargo test` for every touched crate.
- Walk the acceptance criteria in `SPEC.md` and confirm each.