---
status: draft
audit:
  - { time: 1784925148, from: "none", to: "draft", actor: "system" }
---
# Tavily Search Backend Migration

## Background

The project currently has two separate web-search tools:

- `websearch` — a single-backend tool that calls the [Tavily](https://tavily.com/)
  search API when a `TAVILY_API_KEY` environment variable or
  `tavily_api_key` config value is present.
- `mf_search` — a multi-engine, keyless search tool that runs
  `DuckDuckGo`, `Brave`, and optionally `LangSearch` in parallel, then
  merges, deduplicates, ranks, and reports cross-engine consensus.

This dual implementation means:

1. Tavily results are **not** merged with other engines.
2. The `websearch` tool cannot benefit from consensus ranking or
   graceful degradation when Tavily is unavailable.
3. The research system’s `WebGatherer` can only use `websearch`
   (Tavily) today, leaving its web-gathering phase on a single provider
   instead of the robust parallel backend.

The goal of this migration is to:

- Add a `TavilyEngine` implementation of the existing
  `SearchEngine` trait so Tavily participates in the parallel
  `mf_search` pipeline.
- Repurpose the `websearch` tool so it becomes a compatibility wrapper
  around `mf_search` (still exposing the simpler `query` +
  `num_results` schema and still accepting a Tavily API key).
- Update the research system’s web-gathering adapter to call
  `mf_search` directly so research benefits from the parallel backends.

## Scope

In scope:

- New `tavily` module under
  `crates/ragent-tools-extended/src/masterfetch/search/`.
- Updates to `MfSearchTool::build_orchestrator` to add the Tavily
  backend when a Tavily API key is configured.
- Refactoring of `WebSearchTool` to delegate through
  `MfSearchTool` / `SearchOrchestrator` while keeping the same public
  tool name, schema, and output shape.
- Update `ragent-agent/src/research_adapter.rs` to build the research
  `WebGatherer` with a `mf_search`-backed search tool.
- Tests and documentation updates.

Out of scope:

- Changing the agent tool registry layout.
- Removing the `websearch` tool name (it remains for backward
  compatibility).
- Altering the `mf_search` schema or consensus merge algorithm beyond
  adding a new backend.

## Requirements

### Functional Requirements

FR-001 — The system shall provide a `TavilyEngine` struct that implements
`SearchEngine` so Tavily becomes a first-class backend inside the
`mf_search` parallel pipeline.

FR-002 — When a Tavily API key is available, `mf_search` shall include the
Tavily backend alongside DuckDuckGo and Brave, running all enabled
backends in parallel.

FR-003 — `TavilyEngine` shall translate the shared `SearchOptions` into
the Tavily JSON request body, including the query, `max_results`
(clamped to Tavily limits), and optional `search_depth` /
`include_answer` defaults that preserve existing `websearch` behaviour.

FR-004 — `TavilyEngine` shall parse the Tavily JSON response into
`RawResult` rows with `source` set to `"tavily"` and normalise / dedupe
results the same way the other backends do.

FR-005 — If the Tavily API key is missing, expired, or the Tavily API
returns a non-2xx response, `TavilyEngine` shall report the failure as an
`EngineReport::blocked` entry so the other `mf_search` backends can still
return results.

FR-006 — The `websearch` tool shall continue to expose the same name,
permission category (`web`), and parameters schema (`query`, optional
`num_results`) after migration.

FR-007 — When the `websearch` tool receives a request, it shall forward
the query to `mf_search` (via `MfSearchTool::build_orchestrator`), convert
the `num_results` parameter into `SearchOptions::max_results`, and return
the merged results formatted as human-readable text with the same
provenance fields as before.

FR-008 — The `websearch` tool’s JSON metadata shall continue to include a
`results` array whose items contain `title`, `url`, `snippet`,
`search_tool`, and `search_engine` fields so existing research-system
parsers keep working.

FR-009 — The research system’s `WebGatherer` adapter
(`AgentWebSearchTool`) shall call the `mf_search` tool instead of the
`websearch` tool so research web-gathering uses the parallel backend.

FR-010 — The research system shall map `mf_search` result metadata into
`WebSearchHit` rows, preserving `url`, `title`, `snippet`,
`search_tool = "mf_search"`, and a comma-separated `search_engine` list
reflecting the consensus `source` field.

FR-011 — When the Tavily backend is enabled but unavailable, `mf_search`
shall still return results from DuckDuckGo and Brave and report
`tavily` in the `engine_blocked` list rather than failing the entire
request.

FR-012 — Tavily API keys shall be read from the `TAVILY_API_KEY`
environment variable or the `tavily_api_key` config field and shall never
be logged in plain text in diagnostics.

### Non-Functional Requirements

NFR-001 — The migration shall not increase the `websearch` happy-path
latency by more than 200 ms end-to-end for a single-backend Tavily-only
scenario in CI benchmarks.

NFR-002 — All new code shall follow the existing module documentation,
error-handling, and test-visibility conventions in
`ragent-tools-extended`.

NFR-003 — The `TavilyEngine` and `WebSearchTool` changes shall be unit
and integration-testable without making live Tavily API calls.

NFR-004 — The `mf_search` tool description shall be updated to mention
that Tavily is an optional parallel backend when a key is configured.

## References

- `crates/ragent-tools-extended/src/websearch.rs`
- `crates/ragent-tools-extended/src/masterfetch/tools/search_tool.rs`
- `crates/ragent-tools-extended/src/masterfetch/search/engine.rs`
- `crates/ragent-tools-extended/src/masterfetch/search/langsearch.rs`
- `crates/ragent-tools-extended/src/masterfetch/search/mod.rs`
- `crates/ragent-agent/src/research_adapter.rs`
- `crates/ragent-research/src/web_gatherer.rs`
- `crates/ragent-config/src/config.rs`
