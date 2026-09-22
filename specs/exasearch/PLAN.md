# Implementation Plan — Exa Search API Backend

**Spec ID:** exasearch
**Status:** draft

## Tasks

| ID | Title | Requirement | Effort | Priority | Status | Dependencies |
|----|-------|-------------|--------|----------|--------|--------------|
| T-001 | Add `exa_api_key` config field to `Config` struct | FR-002, FR-010 | S | High | completed | — |
| T-002 | Add `exa_api_key` overlay-merge logic in `merge_config` | FR-010 | S | High | completed | T-001 |
| T-003 | Create `crates/ragent-tools-extended/src/masterfetch/search/exa.rs` module with `ExaEngine` struct, constants, and constructors | FR-001, FR-006 | M | Critical | completed | — |
| T-004 | Implement `build_request_body` pure function (query → Exa JSON body) | FR-003 | M | Critical | completed | T-003 |
| T-005 | Implement `parse_response_json` pure function (Exa JSON → `Vec<RawResult>`) | FR-004, FR-011 | M | Critical | completed | T-003 |
| T-006 | Implement `SearchEngine::search` for `ExaEngine` (HTTP call, error handling, dedup) | FR-001, FR-005, FR-011, NFR-001 | M | Critical | completed | T-004, T-005 |
| T-007 | Implement `mask_key` and `masked_key` helper | FR-006, NFR-002 | S | High | completed | T-003 |
| T-008 | Implement `truncate_query` and `truncate_snippet` helpers | FR-003, FR-004 | S | Medium | completed | T-003 |
| T-009 | Implement `freshness_to_date_range` helper for Exa date parameters | FR-003 | S | Medium | completed | T-003 |
| T-010 | Register `pub mod exa` in `search/mod.rs` | FR-001 | S | Critical | completed | T-003 |
| T-011 | Wire `ExaEngine` into `MfSearchTool::build_orchestrator` with key resolution from env + config | FR-002 | S | High | completed | T-003, T-001 |
| T-012 | Add `resolve_exa_key` helper in `search_tool.rs` | FR-002 | S | High | completed | T-011 |
| T-013 | Update `engine_status` to include Exa `EngineStatus` entry | FR-009 | S | Medium | completed | T-012 |
| T-014 | Update `mf_search` tool description string to mention `"exa"` | FR-008 | S | Medium | completed | T-011 |
| T-015 | Update `parameters_schema` `engine` enum to include `"exa"` | FR-008 | S | Medium | completed | T-011 |
| T-016 | Add unit tests for `build_request_body` | FR-003, FR-012 | M | High | completed | T-004 |
| T-017 | Add unit tests for `parse_response_json` | FR-004, FR-012 | M | High | completed | T-005 |
| T-018 | Add unit tests for `mask_key` | FR-006, NFR-002 | S | High | completed | T-007 |
| T-019 | Add unit tests for `truncate_query` and `truncate_snippet` | FR-003, FR-004 | S | Medium | completed | T-008 |
| T-020 | Add unit tests for `freshness_to_date_range` | FR-003 | S | Medium | completed | T-009 |
| T-021 | Add live integration test (`#[ignore]`) for `ExaEngine::search` against real API | FR-001, NFR-001 | M | Low | completed | T-006 |
| T-022 | Add orchestrator integration test verifying `ExaEngine` is wired when key is present | FR-002, FR-007 | S | High | completed | T-011 |
| T-023 | Run `cargo fmt` and `cargo clippy` to ensure clean build | NFR-003 | S | Medium | completed | T-001–T-022 |
| T-024 | Update README.md `mf_search` backend list to include Exa | — | S | Low | completed | T-011 |