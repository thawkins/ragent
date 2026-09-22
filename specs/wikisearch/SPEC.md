---
status: draft
audit:
  - { time: 1786551648, from: "none", to: "draft", actor: "system" }
---
# Wikipedia Search Backend for `mf_search`

## Background

The `mf_search` tool already runs a multi-engine keyless search pipeline
(DuckDuckGo + Brave + OpenAlex, plus optional LangSearch / Tavily /
Perplexity API-backed backends) behind the `SearchEngine` trait. Each
backend is implemented as a small adapter module under
`crates/ragent-tools-extended/src/masterfetch/search/` and plugged into
`SearchOrchestrator` by `MfSearchTool::build_orchestrator`.

[Wikipedia](https://www.wikipedia.org) exposes a public REST API at
`https://en.wikipedia.org/api/rest_v1/` with, among others, the
**page/summary** endpoint:

- `GET /page/summary/{title}` — returns a concise summary of a Wikipedia
  page as JSON: `title`, `extract` (plain-text summary), `description`,
  `content_urls.desktop.page` (the human-facing article URL),
  `thumbnail.source`, `lang`, `timestamp`, and page metadata.

The endpoint requires a descriptive `User-Agent` header (requests with no
or a generic user agent receive HTTP 403). It is **unauthenticated** — no
API key is required — making it a keyless backend like DuckDuckGo, Brave,
and OpenAlex.

Because the page/summary endpoint takes a *page title* (not a free-text
query), the backend first resolves the user's query to candidate page
titles using the MediaWiki Action API's `list=search` facility
(`GET /w/api.php?action=query&list=search&srsearch=<query>&format=json`),
then fetches a summary for each resolved title from the page/summary
endpoint. This two-step flow gives `mf_search` first-class encyclopedia
coverage: results include a plain-language extract, the canonical article
URL, a short description, and (when available) a thumbnail URL.

## Scope

In scope:

- New `wikipedia` module under
  `crates/ragent-tools-extended/src/masterfetch/search/`.
- A `WikipediaEngine` struct implementing the existing `SearchEngine`
  trait.
- A two-step query flow: resolve query → candidate titles via the
  MediaWiki Action API, then fetch a summary per title via the REST
  page/summary endpoint.
- Mapping of shared `SearchOptions` to Wikipedia query parameters.
- Parsing of both the Action API search response and the page/summary
  JSON into `RawResult` rows.
- Wiring the engine into `MfSearchTool::build_orchestrator` as an
  always-on keyless backend (no config key needed).
- A descriptive `User-Agent` header on every outbound request.
- Documentation updates.

Out of scope:

- Changes to the `SearchEngine` trait or consensus merge algorithm.
- Full article HTML extraction (`/page/html`) or media-list endpoints.
- Multi-language selection UI (the backend targets `en.wikipedia.org`;
  a future enhancement may parameterize the language subdomain).
- Authentication via Wikimedia API keys / OAuth.

## Requirements

### Functional Requirements

FR-001 (ubiquitous) — The `mf_search` tool shall expose encyclopedia
metadata (plain-text extract, short description, canonical article URL,
and, when available, thumbnail URL and last-revision timestamp) for each
Wikipedia result it returns.

FR-002 (ubiquitous) — The `WikipediaEngine` shall implement the existing
`SearchEngine` trait so that Wikipedia participates in the parallel
`mf_search` consensus pipeline alongside the other backends.

FR-003 (event-driven) — When the `mf_search` tool builds its
orchestrator, the system shall include the `WikipediaEngine` as an
always-on keyless backend, requiring no API key, environment variable,
or configuration entry from the user.

FR-004 (state-driven) — While a non-empty `site` filter is present in
`SearchOptions`, the `WikipediaEngine` shall restrict resolved
candidate titles to those whose article URL host matches the requested
domain; when no `site` filter is present, the engine shall issue an
unfiltered search.

FR-005 (event-driven) — When resolving a query to candidate page titles,
the `WikipediaEngine` shall request at most `opts.max_results` titles
(clamped to the MediaWiki search API's 1–500 result limit) from the
Action API and shall fetch a page/summary for each resolved title, so
that the number of summaries retrieved never exceeds the requested
result cap.

FR-006 (event-driven) — When the Wikipedia API returns a non-2xx HTTP
status or an unparseable body, the `WikipediaEngine` shall report an
`EngineReport` with `engine_blocked = true` (or `error`) so the
remaining `mf_search` backends continue to return results.

FR-007 (ubiquitous) — Every HTTP request issued by the
`WikipediaEngine` shall carry a descriptive `User-Agent` header
identifying the ragent application, so the backend does not receive
HTTP 403 from Wikipedia's robot policy.

FR-008 (unwanted) — If the Wikipedia API returns HTTP 429 (rate
limited) or HTTP 403 (blocked / missing user-agent), the
`WikipediaEngine` shall report `EngineReport::blocked("wikipedia",
"rate-limited")` (or `"blocked"`) rather than propagating an `Err`, so
consensus merging can mark the engine as blocked and the other
backends still contribute.

FR-009 (ubiquitous) — Each `RawResult` produced by the
`WikipediaEngine` shall set `source` to `"wikipedia"`, populate `url`
with the page's `content_urls.desktop.page` value, and populate
`snippet` with the `extract` field truncated to approximately 300
characters.

FR-010 (optional) — Where the page/summary response provides a
`description` field, the `WikipediaEngine` may prepend it to the result
snippet to give the agent additional disambiguation context.

FR-011 (optional) — Where the page/summary response provides a
`thumbnail.source` field, the `WikipediaEngine` may surface the
thumbnail URL in the result snippet so downstream rendering can display
a preview image.

FR-012 (state-driven) — While the `freshness` filter is `Any` or the
resolved titles have no usable `timestamp`, the `WikipediaEngine` shall
not apply a date filter; the backend does not support time-scoped
Wikipedia search and shall ignore `freshness` without error.

### Non-Functional Requirements

NFR-001 — The `WikipediaEngine` shall complete its requests within the
shared masterfetch HTTP client timeout so it does not stall the parallel
`mf_search` pipeline beyond the existing 15-second end-to-end budget.

NFR-002 — Request construction and response parsing shall be implemented
as pure functions that accept plain inputs and return plain outputs,
enabling unit tests without network I/O (matching the OpenAlex / Tavily
testability pattern).

NFR-003 — The new module shall be documented in the `mf_search` tool
description, the README feature list, and the
`masterfetch/search/mod.rs` module header, consistent with the existing
backend modules.

NFR-004 — The implementation shall introduce no `unsafe` code and no
`.unwrap()` on user-facing paths, per project guidelines.

NFR-005 — The summary-fetch step shall issue requests concurrently
(using `futures::join_all` or equivalent) so that retrieving summaries
for multiple titles does not serialise and exceed the search time
budget.