# Release

## Current Version: 0.1.0-alpha.105

### Added
- **Context compression pipeline** — New compression module (`ragent-agent/src/compression/`) with multi-strategy history compaction: BM25 relevance scoring, CCR (Critical Content Retention) store, aggressive/conservative/default modes, and `/compress` slash command integration.
- **Model Router Provider** — New intelligent model routing system (`ragent-llm/src/providers/router*.rs`) with 15-dimension classifier (complexity, creativity, code, reasoning, vision, etc.), automatic model selection, and configurable routing rules.
- **Compression config** — New `compression.rs` module in ragent-config for compression pipeline configuration.
- **String utilities** — New `strutil.rs` module in ragent-types for shared string helpers.
- **Spec ID scanner** — New `id_scanner.rs` in ragent-specs for extracting and tracking spec requirement/task IDs.
- **HeadroomCompress spec** — Full specification for the compression feature in `specs/HeadroomCompress/`.
- **ModelRouterProvider spec** — Full specification for the model router in `specs/ModelRouterProvider/`.
- **Config defaults fix** — Added `#[serde(skip_serializing_if)]` to `code_index.enabled`, `internal_llm.enabled`, and `tool_visibility.codeindex` so auto-generated config files don't override code-level defaults.
- **Compression indicator test** — New TUI test for compression status display.

### Changed
- **Agent system** — Refactored agent module with expanded presets and compression integration.
- **Session processor** — Added compression integration, improved tool call handling, and spec command support.
- **TUI** — Added `/compress` slash command, compression status bar indicator, and improved status bar layout.
- **Spec commands** — Major refactor of spec command handling with expanded `/spec impl` and `/spec implement` support.
- **Bedrock provider** — Refinements to credential handling and SigV4 signing.
- **Multiple tool refinements** — Updated codeindex_search, list_tasks, memory_search, office_write, and spec_list tools.
- **Test improvements** — Updated multiple test files for compatibility with new APIs and module structure.

## Previous: 0.1.0-alpha.104