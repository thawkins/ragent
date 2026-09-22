# Wikipedia Search Backend — Implementation Plan

This plan implements the requirements in `SPEC.md` for spec `wikisearch`.
The strategy mirrors the existing OpenAlex backend:

1. Implement `WikipediaEngine` inside the existing `SearchEngine` trait.
2. Add a pure request-builder for the Action API title-resolution step
   and a pure parser for the page/summary JSON response (unit-testable
   without network).
3. Wire the engine into `mf_search` as an always-on keyless backend.
4. Set a descriptive `User-Agent` header on every request (FR-007).
5. Add tests, update docs, and verify no regressions.

## Tasks

| ID | Title | Requirement | Effort | Priority | Status | Dependencies |
|----|-------|-------------|--------|----------|--------|--------------|
| T-001 | Create `wikipedia.rs` module + `WikipediaEngine` struct implementing `SearchEngine` | FR-002, FR-009, NFR-001, NFR-004 | M | Critical | completed | — |
| T-002 | Implement `build_search_request` — map `SearchOptions` → MediaWiki Action API `list=search` query params (`srsearch`, `srlimit`, `format=json`) | FR-005 | S | High | completed | T-001 |
| T-003 | Implement `build_summary_url` — construct the REST `page/summary/{title}` URL for a resolved title (URL-encoded) | FR-009, NFR-002 | S | High | completed | T-001 |
| T-004 | Implement `parse_search_response` — parse the Action API `query.search[]` array into a list of candidate titles | FR-005, NFR-002 | M | High | completed | T-001 |
| T-005 | Implement `parse_summary_response` — parse one page/summary JSON into a `RawResult` (`title`, `extract`, `content_urls.desktop.page`, optional `description`, optional `thumbnail.source`) | FR-001, FR-009, FR-010, FR-011, NFR-002 | M | High | completed | T-001 |
| T-006 | Implement the two-step `search` flow: resolve titles → fetch summaries concurrently → assemble `EngineReport`; apply `site` filtering | FR-004, FR-005, NFR-005 | L | Critical | completed | T-002, T-003, T-004, T-005 |
| T-007 | Set a descriptive `User-Agent` header on every outbound request; handle HTTP 403 / 429 as `EngineReport::blocked` | FR-006, FR-007, FR-008 | S | High | completed | T-001 |
| T-008 | Handle non-2xx / unparseable bodies as `engine_blocked` / `error` reports so other backends continue | FR-006, FR-008 | S | High | completed | T-001 |
| T-009 | Wire `WikipediaEngine` into `MfSearchTool::build_orchestrator` as an always-on keyless backend (after OpenAlex, before optional keyed backends) | FR-003 | S | Critical | completed | T-001 |
| T-010 | Update `MfSearchTool::engine_status` to report `wikipedia` as always `enabled` and `in_use`, never `failed` | FR-003 | S | Medium | completed | T-009 |
| T-011 | Add unit tests for `build_search_request`, `parse_search_response`, and `parse_summary_response` (pure functions, no network) | NFR-002 | M | High | completed | T-002, T-004, T-005 |
| T-012 | Add `#[ignore]` integration test hitting live `https://en.wikipedia.org` with a real query | NFR-001, FR-006 | M | Medium | completed | T-011 |
| T-013 | Update `masterfetch/search/mod.rs` module header + `mf_search` tool description to mention the Wikipedia backend | NFR-003 | S | Low | completed | T-009 |
| T-014 | Update README feature list and `masterfetch` docs for the Wikipedia backend | NFR-003 | S | Low | completed | T-009 |
| T-015 | Run `cargo fmt`, `cargo clippy`, `cargo test` and verify no regressions | NFR-004 | S | High | completed | T-001–T-014 |
## Task Details

### T-001 — Create `WikipediaEngine`

- Add `crates/ragent-tools-extended/src/masterfetch/search/wikipedia.rs`.
- Register the module in `masterfetch/search/mod.rs`.
- Implement `SearchEngine` for `WikipediaEngine` with `name() -> "wikipedia"`.
- Store an injectable `reqwest::Client` (for testing); when `None`, use
  the shared masterfetch client from `crate::masterfetch::http`.
- No API key required (keyless backend).
- No `unsafe` code; no `.unwrap()` on user-facing paths (NFR-004).

### T-002 — Action API search request builder

- Pure `pub fn build_search_request(query, opts) -> (String, Vec<(String,String)>)`
  returning the MediaWiki Action API URL and query-parameter pairs.
- Map `SearchOptions.max_results` → `srlimit` clamped to 1–500.
- Set `action=query`, `list=search`, `format=json`, `srsearch=<query>`.
- Always include a `srprop=snippet` so the search step returns a snippet
  (used as a fallback when a page/summary fetch fails).

### T-003 — Summary URL builder

- Pure `pub fn build_summary_url(title) -> String` that URL-encodes the
  title and returns
  `https://en.wikipedia.org/api/rest_v1/page/summary/{encoded_title}`.

### T-004 — Search response parser

- Pure `pub fn parse_search_response(value: &serde_json::Value) -> Vec<String>`
  returning the list of candidate page titles from `query.search[].title`.
- Skip entries with no title field.

### T-005 — Summary response parser

- Pure `pub fn parse_summary_response(value: &serde_json::Value) -> Option<RawResult>`.
- Returns `None` when the response is missing `title` or `extract`.
- Sets `source = "wikipedia"`.
- `url` ← `content_urls.desktop.page` (fall back to the constructed
  article URL from the title).
- `snippet` ← `extract`, prepended with `description` when present
  (FR-010), truncated to ~300 chars (FR-009).
- Append the thumbnail URL to the snippet when `thumbnail.source` is
  present (FR-011).

### T-006 — Two-step search flow

- Call the Action API search to resolve up to `opts.max_results`
  candidate titles (T-002, T-004).
- If `site` is set, filter resolved titles whose article URL host does
  not match (FR-004).
- For each remaining title, build the summary URL (T-003) and fetch
  concurrently with `futures::join_all` (NFR-005).
- Parse each summary (T-005); collect successful `RawResult`s.
- Truncate the result vector to `opts.max_results`.
- Return `EngineReport::ok("wikipedia", results)`.

### T-007 — User-Agent and blocked handling

- Set `User-Agent: ragent/<version> (https://github.com/thawkins/ragent)`
  on every request (FR-007).
- HTTP 403 → `EngineReport::blocked("wikipedia", "blocked")`.
- HTTP 429 → `EngineReport::blocked("wikipedia", "rate-limited")`
  (FR-008).

### T-008 — Generic error handling

- Other non-2xx → `EngineReport::error` or `blocked`.
- Unparseable JSON → `EngineReport::error("wikipedia", "…")` (FR-006).
- Never return `Err` for engine-level failures (catch-and-return
  pattern, matching the other backends).

### T-009 — Orchestrator wiring

- In `MfSearchTool::build_orchestrator`, push
  `Arc::new(WikipediaEngine::new())` into the engines vector
  unconditionally (always-on keyless backend, FR-003).
- Place after OpenAlex, before optional keyed backends.

### T-010 — Engine status

- Add `wikipedia` to `MfSearchTool::engine_status` as always `enabled`,
  `in_use`, never `failed`.

### T-011–T-015 — Tests & docs

- Unit tests for the pure functions (T-011).
- A `#[ignore]` live integration test (T-012).
- Module-header and tool-description updates (T-013).
- README and docs updates (T-014).
- `cargo fmt` / `cargo clippy` / `cargo test` verification (T-015).