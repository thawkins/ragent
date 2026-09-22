# OpenAlex Search Backend — Implementation Plan

This plan implements the requirements in `SPEC.md` for spec `openalex`.
The strategy mirrors the existing Tavily and LangSearch backends:

1. Implement `OpenAlexEngine` inside the existing `SearchEngine` trait.
2. Add pure request-builder and response-parser functions (unit-testable
   without network).
3. Wire the engine into `mf_search` as an always-on keyless backend.
4. Resolve an optional polite-pool `mailto` from config/env.
5. Add tests, update docs, and verify no regressions.

## Tasks

| ID | Title | Requirement | Effort | Priority | Status | Dependencies |
|----|-------|-------------|--------|----------|--------|--------------|
| T-001 | Create `openalex.rs` module + `OpenAlexEngine` struct implementing `SearchEngine` | FR-002, FR-010, FR-012, NFR-001, NFR-005 | M | Critical | completed | — |
| T-002 | Implement `build_request` — map `SearchOptions` → OpenAlex query params (`search`, `filter`, `per_page`, `page`/`cursor`, `mailto`) | FR-004, FR-005, FR-008, FR-012 | M | High | completed | T-001 |
| T-003 | Implement `parse_response` — parse `results[]` into `RawResult` rows with scholarly metadata | FR-001, FR-010, FR-011, NFR-002 | M | High | completed | T-001 |
| T-004 | Handle non-2xx / 429 / parse errors as `engine_blocked` reports | FR-006, FR-009 | S | High | completed | T-001 |
| T-005 | Resolve optional `mailto` from `OPENALEX_EMAIL` env / `openalex_email` config field | FR-007, NFR-003 | S | Medium | completed | T-001 |
| T-006 | Add `openalex_email: Option<String>` config field to `ragent-config` | FR-007, NFR-003 | S | Medium | completed | T-005 |
| T-007 | Wire `OpenAlexEngine` into `MfSearchTool::build_orchestrator` as always-on keyless backend | FR-003 | S | Critical | completed | T-001, T-005 |
| T-008 | Update `MfSearchTool::engine_status` to report `openalex` as always-enabled | FR-003 | S | Medium | completed | T-007 |
| T-009 | Add unit tests for `build_request` and `parse_response` (pure functions, no network) | NFR-002 | M | High | completed | T-002, T-003 |
| T-010 | Add `#[ignore]` integration test hitting live `https://api.openalex.org/works?search=���` | NFR-001, FR-006 | M | Medium | completed | T-009 |
| T-011 | Update `masterfetch/search/mod.rs` module header + `mf_search` tool description | NFR-004 | S | Low | completed | T-007 |
| T-012 | Update README feature list and `masterfetch` docs for OpenAlex backend | NFR-004 | S | Low | completed | T-007 |
| T-013 | Run `cargo fmt`, `cargo clippy`, `cargo test` and verify no regressions | NFR-005 | S | High | completed | T-001–T-012 |
## Task Details

### T-001 — Create `OpenAlexEngine`

- Add `crates/ragent-tools-extended/src/masterfetch/search/openalex.rs`.
- Implement `SearchEngine` for `OpenAlexEngine` with `name() -> "openalex"`.
- Store optional `mailto` email; no API key required (keyless backend).
- Use the shared masterfetch HTTP client from `crate::masterfetch::http`.
- Re-use `dedup_results_by_url` and truncate to `opts.max_results`.
- No `unsafe` code; no `.unwrap()` on user-facing paths (NFR-005).

### T-002 — Request builder

- Pure `pub fn build_request(query, opts, mailto) -> (String, Vec<(String,String)>)`
  returning the URL and query-parameter pairs.
- Map `SearchOptions.max_results` → `per_page` clamped to 1–200.
- Map `SearchOptions.page` → `page` (1-indexed); fall back to `cursor=*`
  when `page` exceeds the 10k basic-pagination limit (FR-008).
- `site` filter → `filter=primary_location.source.host_organization:<domain>`
  (FR-004).
- `freshness` (non-`Any`) → `from_publication_date`/`to_publication_date`
  date range (FR-005).
- Append `mailto=<email>` when present (FR-007).

### T-003 — Response parser

- Pure `pub fn parse_response(value: &serde_json::Value) -> Vec<RawResult>`.
- Iterate `results[]`; for each work extract:
  - `title` → `title`
  - `primary_location.landing_page_url` (fallback: `id` URI, then DOI URL)
    → `url`
  - abstract (reconstructed from inverted index or `abstract_inverted_index`)
    stripped of HTML and truncated to ~200 chars → `snippet`
  - `relevance_score` (normalised to 0.0–1.0) → `score`
- Set `source = "openalex"`.
- Scholarly metadata (DOI, publication year, citation count, OA URL,
  source display name) surfaced via the snippet or via future `RawResult`
  extension (FR-001).

### T-004 — Error handling

- Non-2xx → `EngineReport::error` or `blocked`.
- HTTP 429 → `EngineReport::blocked("openalex", "rate-limited")` (FR-009).
- Unparseable JSON → `EngineReport::error("openalex", "…")` (FR-006).
- Never return `Err` for engine-level failures (catch-and-return pattern).

### T-005 — Polite-pool email

- Read `OPENALEX_EMAIL` env var, then `openalex_email` config field.
- Mask in diagnostics (NFR-003).
- Append as `mailto=` query param when non-empty (FR-007).

### T-006 — Config field

- Add `openalex_email: Option<String>` to `ragent_config::Config` with
  `#[serde(default, skip_serializing_if = "Option::is_none")]`.
- Document precedence: env var > config field.

### T-007 — Orchestrator wiring

- In `MfSearchTool::build_orchestrator`, push
  `Arc::new(OpenAlexEngine::new(mailto))` into the engines vector
  unconditionally (always-on keyless backend, FR-003).
- Place after DuckDuckGo/Brave, before optional keyed backends.

### T-008 — Engine status

- Add `openalex` to `MfSearchTool::engine_status` as always `enabled`,
  `in_use`, never `failed`.

### T-009–T-013 — Tests & docs

- Unit tests for `build_request` and `parse_response` (NFR-002).
- `#[ignore]` live integration test (NFR-001, FR-006).
- Update module header, tool description, README, and masterfetch docs
  (NFR-004).
- Final `cargo fmt` + `cargo clippy` + `cargo test` pass (NFR-005).