---
status: draft
audit:
  - { time: 1786545617, from: "none", to: "draft", actor: "system" }
---
# OpenAlex Search Backend for `mf_search`

## Background

The `mf_search` tool already runs a multi-engine keyless search pipeline
(DuckDuckGo + Brave, plus optional LangSearch / Tavily / Perplexity
API-backed backends) behind the `SearchEngine` trait. Each backend is
implemented as a small adapter module under
`crates/ragent-tools-extended/src/masterfetch/search/` and plugged into
`SearchOrchestrator` by `MfSearchTool::build_orchestrator`.

[OpenAlex](https://openalex.org) is a fully-open catalog of scholarly works,
authors, sources, institutions, topics, publishers, and funders. Its REST
API at `https://api.openalex.org` provides:

- `/works?search=<query>` — full-text search across ~240M scholarly works,
  ranked by `relevance_score`.
- `/works?filter=<field:value,…>` — structured filtering (publication year,
  open access, cited-by count, type, institution, …).
- `?per_page=` — 1–200 results per request (default 25).
- `?page=` — basic pagination (up to 10,000 results); `?cursor=*` for deep
  paging beyond that.
- `?select=id,doi,display_name,…` — field projection.
- No authentication is required; a `mailto=` query parameter is the
  recommended polite-pool identifier and raises the daily request limit.
  The documented daily limit is 100,000 requests/day for unauthenticated
  traffic.

Adding an OpenAlex backend gives `mf_search` first-class academic /
scholarly coverage: results include DOIs, publication years, citation
counts, open-access URLs, and journal/source metadata that general web
engines do not surface. Because OpenAlex requires no API key, the backend
is keyless (like DuckDuckGo and Brave) and participates in the existing
parallel consensus pipeline without any user configuration.

## Scope

In scope:

- New `openalex` module under
  `crates/ragent-tools-extended/src/masterfetch/search/`.
- An `OpenAlexEngine` struct implementing the existing `SearchEngine` trait.
- Mapping of shared `SearchOptions` to OpenAlex query parameters.
- Parsing of the OpenAlex JSON response (`results[]`) into `RawResult` rows.
- Wiring the engine into `MfSearchTool::build_orchestrator` as an always-on
  keyless backend (no config key needed).
- Documentation updates.

Out of scope:

- Changes to the `SearchEngine` trait or consensus merge algorithm.
- Semantic/vector search (`/find/works`) — uses a separate endpoint with
  different rate limits; deferred.
- Aggregation (`group_by`) and snapshot download.
- A separate `openalex` CLI command.

## Requirements

### Functional Requirements

FR-001 (ubiquitous) — The `mf_search` tool shall expose scholarly metadata
(relevance score, DOI, publication year, citation count, open-access URL,
and source/journal display name) for each academic result it returns.

FR-002 (ubiquitous) — The `OpenAlexEngine` shall implement the existing
`SearchEngine` trait so that OpenAlex participates in the parallel
`mf_search` consensus pipeline alongside the other backends.

FR-003 (event-driven) — When the `mf_search` tool builds its orchestrator,
the system shall include the `OpenAlexEngine` as an always-on keyless
backend, requiring no API key, environment variable, or configuration
entry from the user.

FR-004 (state-driven) — While a non-empty `site` filter is present in
`SearchOptions`, the `OpenAlexEngine` shall translate it into a
`primary_location.source.host_organization` filter so results are scoped
to the requested publisher/repository domain; when no `site` filter is
present, the engine shall issue an unfiltered `search` request.

FR-005 (state-driven) — While a `freshness` filter other than `Any` is
present in `SearchOptions`, the `OpenAlexEngine` shall translate it into a
`from_publication_date` / `to_publication_date` date range derived from
the freshness window (Day = last 24h, Week = last 7 days, Month = last
30 days, Year = last 365 days); when `freshness` is `Any`, no date filter
shall be applied.

FR-006 (event-driven) — When the OpenAlex API returns a non-2xx HTTP
status or an unparseable body, the `OpenAlexEngine` shall report an
`EngineReport` with `engine_blocked = true` (or `error`) so the remaining
`mf_search` backends continue to return results.

FR-007 (optional) — Where configured, the `OpenAlexEngine` may append a
`mailto=<email>` query parameter (resolved from an `openalex_email`
config field or `OPENALEX_EMAIL` environment variable) to participate in
the OpenAlex polite pool and benefit from the higher daily rate limit.

FR-008 (optional) — The `OpenAlexEngine` may map `SearchOptions.page` to
the OpenAlex `page` parameter for basic pagination (0-indexed internally,
translated to OpenAlex's 1-indexed `page`); when `page` exceeds the
documented 10,000-result basic-pagination limit, the engine shall fall
back to `cursor=*` deep-paging semantics.

FR-009 (unwanted) — If the OpenAlex API returns HTTP 429 (rate limited),
the `OpenAlexEngine` shall report `EngineReport::blocked("openalex",
"rate-limited")` rather than propagating an `Err`, so consensus merging
can mark the engine as blocked and the other backends still contribute.

FR-010 (ubiquitous) — Each `RawResult` produced by the `OpenAlexEngine`
shall set `source` to `"openalex"`, populate `url` with the work's
`primary_location.landing_page_url` (falling back to the OpenAlex `id`
URI, then to any DOI URL), and populate `snippet` with a truncated
abstract (stripped of HTML), capped at approximately 200 characters.

FR-011 (event-driven) — When the OpenAlex response provides a
`relevance_score`, the `OpenAlexEngine` shall populate the `RawResult`
`score` field with the relevance score normalised to the 0.0–1.0 range
used by the consensus ranker.

FR-012 (ubiquitous) — The `OpenAlexEngine` shall request at most
`opts.max_results` results (clamped to OpenAlex's 1–200 `per_page`
range) and shall truncate its returned `RawResult` vector to
`opts.max_results` before returning the `EngineReport`.

### Non-Functional Requirements

NFR-001 — The `OpenAlexEngine` shall complete its HTTP request within the
shared masterfetch HTTP client timeout so it does not stall the parallel
`mf_search` pipeline beyond the existing 15-second end-to-end budget.

NFR-002 — Request construction and response parsing shall be implemented
as pure functions that accept plain inputs and return plain outputs,
enabling unit tests without network I/O (matching the Tavily / LangSearch
testability pattern).

NFR-003 — The OpenAlex API key/email (when configured) shall never be
logged in plain text; only masked representations may appear in
diagnostics.

NFR-004 — The new module shall be documented in the `mf_search` tool
description, the README feature list, and the `masterfetch/search/mod.rs`
module header, consistent with the existing backend modules.

NFR-005 — The implementation shall introduce no `unsafe` code and no
`.unwrap()` on user-facing paths, per project guidelines.