# Tavily Search Backend Migration — Implementation Plan

This plan implements the requirements in `SPEC.md` for spec
`tavmove`. The overall strategy is:

1. Implement `TavilyEngine` inside the existing `SearchEngine` trait.
2. Wire the engine into `mf_search`.
3. Convert `WebSearchTool` into a thin wrapper that delegates to
   `mf_search` while preserving its legacy schema and output.
4. Update the research gatherer adapter to consume `mf_search`.
5. Add tests, update docs, and verify no regressions.

## Tasks

| ID | Title | Requirement | Effort | Priority | Status | Dependencies |
|----|-------|-------------|--------|----------|--------|--------------|
| T-001 | Create `TavilyEngine` struct implementing `SearchEngine` | FR-001, FR-003, FR-004, FR-005, FR-012 | M | Critical | completed | — |
| T-002 | Add Tavily request-body builder and response parser unit tests | FR-003, FR-004, NFR-003 | S | High | completed | T-001 |
| T-003 | Wire `TavilyEngine` into `MfSearchTool::build_orchestrator` | FR-002, FR-011, NFR-004 | S | Critical | completed | T-001 |
| T-004 | Add `mf_search` orchestrator tests verifying Tavily inclusion / absence | FR-002, FR-011, NFR-003 | S | High | completed | T-003 |
| T-005 | Refactor `WebSearchTool` to delegate to `MfSearchTool` | FR-006, FR-007, FR-008 | M | Critical | completed | T-003 |
| T-006 | Preserve `websearch` JSON metadata shape and provenance fields | FR-008 | S | High | completed | T-005 |
| T-007 | Add integration tests for `websearch` wrapper output and metadata | FR-006, FR-007, FR-008, NFR-003 | M | High | completed | T-005, T-006 |
| T-008 | Update research adapter to call `mf_search` for web gathering | FR-009, FR-010 | M | Critical | completed | T-003, T-006 |
| T-009 | Add research adapter unit tests for `mf_search` metadata parsing | FR-010, NFR-003 | S | High | completed | T-008 |
| T-010 | Update `mf_search` tool description and README feature list | NFR-004 | S | Low | completed | T-003 |
| T-011 | Run `cargo test`, `cargo clippy`, and `cargo fmt` | NFR-002 | S | High | completed | T-001–T-009 |
| T-012 | Review spec coverage and mark requirements verified | All | S | Medium | completed | T-011 |
## Task Details

### T-001 — Create `TavilyEngine`

- Add
  `crates/ragent-tools-extended/src/masterfetch/search/tavily.rs`.
- Implement `SearchEngine` for `TavilyEngine` with `name() ->
  "tavily"`.
- Read API key from constructor argument (the orchestrator builder
  will supply it from env / config).
- Implement request body construction, JSON parsing, and
  `EngineReport::blocked` / `error` handling.
- Use the existing key-masking helper (or replicate the LangSearch
  masking logic) to avoid logging the key.
- Re-use `dedup_results_by_url` and truncate to `opts.max_results`
  before returning the report.

### T-002 — Tavily request / parser tests

- Add `crates/ragent-tools-extended/tests/test_mf_tavily.rs`.
- Test request body maps `query`, `max_results`, and default
  `search_depth` / `include_answer` correctly.
- Test parser extracts `title`, `url`, and `content`/`snippet`.
- Test `EngineReport::blocked` for missing or invalid keys and
  non-2xx responses using a mock HTTP client (inject via
  `with_client`).

### T-003 — Wire Tavily into `mf_search`

- Update `MfSearchTool::build_orchestrator` to read `tavily_api_key`
  from `ctx.config` or `TAVILY_API_KEY` env, and append
  `TavilyEngine::new(key)` to the engines vector when a non-empty key
  is present.
- Keep LangSearch conditional logic unchanged.

### T-004 — Orchestrator backend tests

- Extend `crates/ragent-tools-extended/tests/test_mf_search_tool.rs`
  with tests asserting that a Tavily key adds the `"tavily"` engine,
  an empty key omits it, and a missing key omits it.
- Add an integration test with mocked engines to verify that a
  blocked Tavily report does not prevent other engines from returning
  results (FR-011).

### T-005 — Refactor `WebSearchTool`

- Change `WebSearchTool::execute` to:
  1. Read / validate `query` and `num_results`.
  2. Build or reuse a `ToolContext` that carries the same
     `config` as the caller.
  3. Build `SearchOptions::new(num_results)`.
  4. Call `MfSearchTool::build_orchestrator(ctx).search(...).await`.
  5. Format the merged results as the old human-readable text.
- Keep `parameters_schema` unchanged.
- Keep the tool name as `websearch`.

### T-006 — Preserve metadata shape

- Ensure the JSON metadata emitted by the new `websearch` wrapper
  contains a `results` array matching the old `SearchResult` shape.
- Map each `ConsensusResult` back to `{title, url, snippet,
  search_tool: "websearch", search_engine}` where `search_engine` is
  the consensus `source` string.

### T-007 — `websearch` integration tests

- Add / extend tests in
  `crates/ragent-tools-extended/tests/test_websearch.rs` (or similar)
  to verify:
  - empty / missing query still errors;
  - output text contains result titles and URLs;
  - metadata `results` array is parseable;
  - blocked Tavily still returns DuckDuckGo / Brave results.

### T-008 — Research adapter migration

- In `crates/ragent-agent/src/research_adapter.rs`:
  - Introduce a new `AgentMfSearchTool` implementing the
    `WebSearchTool` trait, or adapt `AgentWebSearchTool` to call
    `mf_search`.
  - Map `mf_search` metadata into `Vec<WebSearchHit>` with
    `search_tool = "mf_search"` and `search_engine` as a comma-separated
    list of backend names from the consensus source.
- Update `build_web_gatherer` to use the `mf_search` tool from the
  registry instead of `websearch`.
- Keep `parse_websearch_output` for backward compatibility but prefer
  metadata.

### T-009 ��� Research adapter tests

- Add unit tests for mapping `mf_search` metadata to `WebSearchHit`.
- Verify `search_tool` and `search_engine` fields are populated.
- Keep existing `parse_websearch_output` tests.

### T-010 — Documentation updates

- Update `mf_search` description in
  `crates/ragent-tools-extended/src/masterfetch/tools/search_tool.rs`.
- Update `README.md` feature list if it currently says Tavily is only
  for `websearch`.

### T-011 — Verification

- Run `cargo test -p ragent-tools-extended`,
  `cargo test -p ragent-agent`, and `cargo test -p ragent-research`.
- Run `cargo clippy --workspace` and `cargo fmt --check`.
- Ensure no new compiler warnings.

### T-012 — Spec coverage review

- Use `spec_coverage` for `tavmove` once implementation is complete.
- Update task statuses in this plan and transition the spec to
  `implemented`.

## Acceptance Criteria

- `mf_search` returns Tavily results when a Tavily key is configured.
- `mf_search` still works without a Tavily key.
- The `websearch` tool remains available, returns merged results, and
  preserves its schema.
- Research web-gathering uses `mf_search` and continues to produce
  `Source::Web` entries with correct provenance fields.
- All CI tests pass and no new warnings are introduced.