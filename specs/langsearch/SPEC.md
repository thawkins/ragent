---
status: draft
audit:
  - { time: 1784888079, from: "none", to: "draft", actor: "system" }
---
# Specification — langsearch backend for `mf_search`

## Introduction

`mf_search` currently runs a keyless multi-engine search pipeline using DuckDuckGo and Brave HTML scraping. This specification adds **LangSearch** (`https://api.langsearch.com/v1/web-search`) as an optional, API-key-powered backend. Users who supply a `langsearch_api_key` in `ragent.json` get higher-quality, AI-optimised web results merged into the existing consensus ranking.

## Goals

1. Provide a `LangSearchEngine` backend that plugs into the existing `SearchEngine` trait and `SearchOrchestrator`.
2. Allow the API key to be stored in `ragent.json` (project or global) with the same precedence rules as other config values.
3. Map LangSearch request/response semantics (`query`, `freshness`, `count`, `summary`) onto existing `mf_search` parameters without changing the public tool schema.
4. Keep the tool keyless-by-default: when no key is configured, LangSearch is simply skipped and the existing backends continue to work.

## Requirements

### Ubiquitous requirements

**FR-001** — The system shall add a `LangSearchEngine` struct in `crates/ragent-tools-extended/src/masterfetch/search/langsearch.rs` that implements the existing `SearchEngine` trait.

**FR-002** — The `LangSearchEngine` shall send an authenticated `POST` request to `https://api.langsearch.com/v1/web-search` with `Content-Type: application/json` and `Authorization: Bearer {key}`.

**FR-003** — The `LangSearchEngine` shall translate `SearchOptions` into the LangSearch JSON body: `query`, `count` (clamped to 1–10), and `freshness` (`day`→`oneDay`, `week`→`oneWeek`, `month`→`oneMonth`, `year`→`oneYear`, `any`→`noLimit`).

### Event-driven requirements

**FR-004** — When a config load completes and `langsearch_api_key` is present, the `SearchOrchestrator` used by `mf_search` shall include `LangSearchEngine` alongside DuckDuckGo and Brave.

**FR-005** — When a LangSearch API request returns a non-2xx status, the engine shall return an `EngineReport` with `engine_blocked` set to `true` and the HTTP status captured in the `error` field.

### State-driven requirements

**FR-006** — While the `langsearch_api_key` config field is `None`, the system shall not instantiate or query `LangSearchEngine`.

**FR-007** — When the `mf_search` tool is invoked and the LangSearch backend is enabled, the system shall request `summary: true` so that LangSearch returns its full `summary` field.

### Optional requirements

**FR-008** — The system may, when serialising the config, write `langsearch_api_key` back to `ragent.json` if it was explicitly loaded from that file.

**FR-009** — The `mf_search` tool description may be updated to mention that an optional LangSearch API key improves result quality, but the tool remains usable without a key.

### Unwanted requirements

**FR-010** — The system shall not require a LangSearch API key for `mf_search` to function; existing keyless backends must continue to operate when the key is absent.

**FR-011** — The system shall not log or surface the full API key in error messages, traces, or serialised config diagnostics.

## Configuration schema

The top-level `Config` struct gains a new optional string field:

```jsonc
{
  "langsearch_api_key": "ls-..."
}
```

- `langsearch_api_key` is loaded from global config, project config, `RAGENT_CONFIG`, and `RAGENT_CONFIG_CONTENT` with normal precedence.
- It is merged like `tavily_api_key`: an overlay value overrides the base value.
- It is serialised only when explicitly present in the loaded source file (default config generation omits the key).

## Data mapping

### Request body

| LangSearch field | Source |
|------------------|--------|
| `query`          | `mf_search.query` |
| `count`          | `min(max_results, 10)` |
| `freshness`      | `SearchOptions.freshness` mapping |
| `summary`        | hard-coded `true` |

### Response parsing

The engine parses `data.webPages.value` and emits one `RawResult` per item:

- `title`  → `name`
- `url`    → `url`
- `snippet`→ `summary` if present, otherwise `snippet`
- `source` → `"langsearch"`
- `score`  → `None` (consensus merger will derive the score)

## Error handling

- Network / timeout errors are captured in `EngineReport.error` with `engine_blocked = true`.
- Missing key at runtime results in the engine not being registered, so it contributes nothing.
- Invalid JSON responses are treated as a parse error in the engine report, not as a tool-level failure.

## Security and privacy

- The API key is treated as a credential: it is not printed, not traced, and masked in diagnostics.
- The key is stored only in user-controlled config files, not in snapshots, sessions, or logs.

## Out of scope

- LangSearch Rerank API integration.
- Per-provider model discovery or quota reporting.
- A dedicated `langsearch_api_key` environment variable (only config file storage is required).
