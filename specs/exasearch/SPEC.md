---
status: draft
audit:
  - { time: 1786572767, from: "none", to: "draft", actor: "system" }
---
# Exa Search API Backend for mf_search

## Overview

This specification defines a new `mf_search` backend that integrates the
[Exa Search API](https://api.exa.ai/search) into the multi-engine web-search
pipeline. Exa is an API-key-backed search engine that returns web results with
semantic-relevance scores, highlights, and optional content extraction. It
joins the existing optional API-key backends (Tavily, LangSearch, Perplexity)
alongside the keyless backends (DuckDuckGo, Brave, OpenAlex, Wikipedia).

The backend will be implemented as an `ExaEngine` struct that implements the
`SearchEngine` trait, following the established patterns in the
`ragent-tools-extended` `masterfetch::search` module.

### Exa API Summary

- **Endpoint:** `POST https://api.exa.ai/search`
- **Authentication:** `x-api-key` header
- **Request body (JSON):**
  - `query` (string, required) — natural-language search query
  - `numResults` (integer, 1–100, default 10)
  - `type` (string, default `"auto"`) — `auto`, `fast`, `instant`, `deep-lite`, `deep`, `deep-reasoning`
  - `category` (string, optional) — `company`, `people`, `publication`, `news`, `personal site`, `financial report`
  - `includeDomains` (string[], optional, max 1200)
  - `excludeDomains` (string[], optional, max 1200)
  - `startPublishedDate` (ISO 8601, optional)
  - `endPublishedDate` (ISO 8601, optional)
  - `contents` (object, optional) — `highlights: true` to get key excerpts
- **Response body (JSON):**
  - `requestId` (string)
  - `results` (array) — each item: `id`, `url`, `title`, `score`, `publishedDate`, `author`, `highlights[]`
  - `costDollars` (object, optional)

## Requirements

### FR-001: Backend Implementation (Ubiquitous)

> The `mf_search` system **shall** include an `ExaEngine` that implements the
> `SearchEngine` trait by sending an authenticated `POST` request to
> `https://api.exa.ai/search`.

### FR-002: Configuration Field (State-Driven)

> **When** the `exa_api_key` field is present in `ragent.json` or the
> `EXA_API_KEY` environment variable is set, the `mf_search` orchestrator
> **shall** instantiate `ExaEngine` and add it as an additional backend.

### FR-003: Query Parameter Mapping (Event-Driven)

> **When** a search query is received, the `ExaEngine` **shall** map the shared
> `SearchOptions` to the Exa API request body as follows:
> - `query` — the search query verbatim, truncated to 2000 characters.
> - `numResults` — `opts.per_engine_results` clamped to 1–100.
> - `includeDomains` — populated from `opts.site` when non-empty.
> - `excludeDomains` — populated from `opts.exclude_sites` when non-empty.
> - `startPublishedDate` — derived from `opts.freshness` as an ISO 8601
>   date computed from the current date minus the freshness window.
> - `contents.highlights` — set to `true` to retrieve relevant excerpts.
> - `type` — set to `"auto"` (the default search method).

### FR-004: Response Parsing (Ubiquitous)

> The `ExaEngine` **shall** parse the JSON response at `results[]` and emit
> each item as a `RawResult` with:
> - `title` — from `results[].title`, falling back to `results[].url`.
> - `url` — from `results[].url`.
> - `snippet` — from `results[].highlights[]` joined by ` … `, truncated to
>   200 characters; if no highlights, a snippet built from the
>   `publishedDate` and `author` metadata.
> - `source` — set to `"exa"`.
> - `score` — from `results[].score` (Exa provides a 0.0–1.0 relevance
>   score).

### FR-005: Error Handling (Unwanted)

> **If** the Exa API returns a non-2xx HTTP status, the `ExaEngine` **shall
> not** panic or return `Err`; it **shall** report an `EngineReport::blocked`
> for HTTP 429 (rate-limited) or HTTP 401/403 (auth failure), and
> `EngineReport::error` for other non-2xx responses, so that the remaining
> `mf_search` backends continue to return results.

### FR-006: API Key Security (Unwanted)

> The `ExaEngine` **shall not** log or expose the Exa API key in plain text.
> The `masked_key()` method **shall** return only the first two and last two
> characters with `*` characters in between.

### FR-007: Engine Selection (Optional)

> **Where** the `engine` parameter of `mf_search` is set to `"exa"`, the
> orchestrator **shall** restrict the search to the `ExaEngine` backend only.

### FR-008: Tool Description and Schema Update (State-Driven)

> **When** the `ExaEngine` is wired, the `mf_search` tool description
> **shall** list `"exa"` among the optional API-backed engines, and the
> `parameters_schema` `engine` enum **shall** include `"exa"`.

### FR-009: Engine Status Reporting (Event-Driven)

> **When** `engine_status` is called, the `MfSearchTool` **shall** report an
> `EngineStatus` entry for Exa with `enabled` set to `true` when the API key
> is present and `failed` set to `true` when it is missing.

### FR-010: Config Overlay Merging (State-Driven)

> **When** the config overlay is merged, `exa_api_key` from the overlay
> **shall** override the base config value when the overlay value is `Some`.

### FR-011: Result Deduplication (Ubiquitous)

> The `ExaEngine` **shall** pass its raw results through
> `dedup_results_by_url` before truncating to the requested `max_results`,
> consistent with all other API-backed backends.

### FR-012: Testability (Optional)

> The `ExaEngine` **may** accept an injectable `reqwest::Client` via a
> `with_client` constructor for integration testing without network I/O.
> Request-body construction (`build_request_body`) and response parsing
> (`parse_response_json`) **shall** be pure functions that take plain inputs
> and produce plain outputs.

## Non-Functional Requirements

### NFR-001: Latency

The `ExaEngine` search **shall** complete within the shared 10-second
per-engine timeout (`ENGINE_TIMEOUT`), consistent with all other backends.

### NFR-002: No Plain-Text Keys in Logs

No log statement, error message, or diagnostic output **shall** contain the
full Exa API key. All diagnostics **shall** use the masked form.

### NFR-003: Code Style

The implementation **shall** follow the existing backend conventions:
module docblock, `ENGINE_NAME` constant, `API_URL` constant, pure helper
functions, `#[must_use]` on constructors, `#[async_trait::async_trait]` on
the trait impl.