# Implementation Plan — langsearch backend for `mf_search`

This plan implements the `langsearch` spec. All work is confined to the `ragent-config` and `ragent-tools-extended` crates.

## Tasks

| ID | Title | Requirement | Effort | Priority | Status | Dependencies |
|----|-------|-------------|--------|----------|--------|--------------|
| T-001 | Add `langsearch_api_key` to `Config` | FR-002, FR-006, FR-008 | S | High | completed | — |
| T-002 | Merge `langsearch_api_key` across config layers | FR-002 | S | High | completed | T-001 |
| T-003 | Create `LangSearchEngine` struct and request builder | FR-001, FR-003 | M | High | completed | — |
| T-004 | Implement `SearchEngine` trait for `LangSearchEngine` | FR-001, FR-005 | M | High | completed | T-003 |
| T-005 | Parse LangSearch response JSON into `RawResult`s | FR-001, FR-007 | M | High | completed | T-004 |
| T-006 | Wire `LangSearchEngine` into `SearchOrchestrator` conditionally | FR-004, FR-006, FR-010 | M | High | completed | T-004, T-002 |
| T-007 | Update `mf_search` tool description and docs | FR-009 | S | Low | completed | T-006 |
| T-008 | Add unit tests for request body mapping | FR-003 | S | High | completed | T-003 |
| T-009 | Add unit tests for response parsing | FR-001, FR-005 | S | High | completed | T-005 |
| T-010 | Add config merge/load tests for `langsearch_api_key` | FR-002, FR-006 | S | High | completed | T-002 |
| T-011 | Add `#[ignore]` integration test for live LangSearch API | FR-001, FR-005 | S | Medium | completed | T-006 |
| T-012 | Update `ragent-tools-extended` module exports | FR-001 | S | High | completed | T-004 |
## Task details

### T-001 — Add `langsearch_api_key` to `Config`

In `crates/ragent-config/src/config.rs`, add `pub langsearch_api_key: Option<String>` next to `tavily_api_key` with a matching doc comment. The field should use `#[serde(default)]` and `#[serde(skip_serializing_if = "Option::is_none")]` so empty keys are omitted from serialised output.

### T-002 — Merge `langsearch_api_key` across config layers

In `Config::merge`, add the same overlay logic used for `tavily_api_key`:

```rust
if overlay.langsearch_api_key.is_some() {
    base.langsearch_api_key = overlay.langsearch_api_key;
}
```

### T-003 — Create `LangSearchEngine` struct and request builder

Create `crates/ragent-tools-extended/src/masterfetch/search/langsearch.rs` containing:

- `LangSearchEngine { api_key: String, client: Option<reqwest::Client> }`
- `impl LangSearchEngine { pub fn new(api_key: String), pub fn with_client(...) }`
- `build_request_body(query, opts) -> serde_json::Value` that maps `SearchOptions` to the LangSearch JSON body.

### T-004 — Implement `SearchEngine` trait for `LangSearchEngine`

Implement `fn name(&self) -> &str { "langsearch" }` and `async fn search(...) -> EngineReport`. The method must:

1. Build the request body.
2. Send the POST with the Bearer token.
3. On 2xx, read JSON.
4. On non-2xx, return `EngineReport::blocked(name, status_text)`.
5. On network/parse errors, return `EngineReport::error(name, error_message)` with `engine_blocked = true`.

### T-005 — Parse LangSearch response JSON into `RawResult`s

Add a pure function `parse_response_json(value: &Value) -> Vec<RawResult>` that walks `data.webPages.value` and maps each entry to `RawResult`. Use `summary` when present, else `snippet`. Unit-test with fixture JSON.

### T-006 — Wire `LangSearchEngine` into `SearchOrchestrator` conditionally

Change `SearchOrchestrator::new()` so it accepts an optional API key (or refactor the tool to build the orchestrator with `with_engines`). The simplest approach: make `SearchOrchestrator::new()` accept an `Option<String>` langsearch key and prepend the engine when the key is present. Update `MfSearchTool::execute` to pass the key from `ToolContext` config.

If `ToolContext` does not currently expose config, add a helper to read `langsearch_api_key` from the existing config reference, or thread it through.

### T-007 — Update `mf_search` tool description and docs

Update the description string in `MfSearchTool` to note that supplying a LangSearch API key improves result quality. Do not change the JSON schema or required parameters.

### T-008 — Add unit tests for request body mapping

In `langsearch.rs` inline tests or in `crates/ragent-tools-extended/tests/test_mf_search_langsearch.rs`:

- Assert `day` maps to `"oneDay"`.
- Assert `max_results > 10` is clamped to `10`.
- Assert `summary` is always `true`.
- Assert `query` is preserved exactly.

### T-009 — Add unit tests for response parsing

- Test parsing of a full LangSearch response fixture.
- Test empty `webPages.value` returns empty `Vec`.
- Test missing `summary` falls back to `snippet`.
- Test non-2xx response produces `engine_blocked = true`.

### T-010 — Add config merge/load tests for `langsearch_api_key`

In `crates/ragent-config/tests/test_config_save.rs` or a new test file:

- Load a JSON config containing `langsearch_api_key`, verify the field is populated.
- Load global + project configs with different keys, verify project wins.
- Save and reload; verify the key round-trips and is masked/omitted when absent.

### T-011 — Add `#[ignore]` integration test for live LangSearch API

In `crates/ragent-tools-extended/tests/test_mf_search_langsearch_integration.rs`:

- Read `LANGSEARCH_API_KEY` from the environment.
- Build `LangSearchEngine::new(key)` and call `search` with a simple query.
- Assert at least one result is returned when the key is valid.
- Mark `#[ignore]` so CI does not run it without a key.

### T-012 — Update `ragent-tools-extended` module exports

In `crates/ragent-tools-extended/src/masterfetch/search/mod.rs`:

- Add `pub mod langsearch;`
- Re-export `LangSearchEngine` if useful for tests / callers.

## Acceptance criteria

1. `cargo test -p ragent-config` and `cargo test -p ragent-tools-extended` pass.
2. `mf_search` without a key behaves exactly as before (DuckDuckGo + Brave).
3. `mf_search` with `langsearch_api_key` configured includes LangSearch results in the consensus merge.
4. The API key is read from `ragent.json`, survives config merge, and is not logged.
5. All new code is documented with `///` docblocks and module-level `//!` comments.