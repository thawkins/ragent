# ragent-tools-extended

Extended document, web, task, and codeindex tools for ragent, the MasterFetch
web content extraction engine, and the project-scaffold / architecture-document
engines behind `/new` and `/spec govcreate`.

## Workspace Dependencies

- ragent-codeindex
- ragent-config
- ragent-tools-core
- ragent-types

## External Dependencies

- tokio, async-trait, futures, serde, serde_json, anyhow, thiserror, tracing, chrono, uuid
- reqwest (with cookies/gzip/deflate/blocking), regex, url, rusqlite, rustc-hash
- html2text, readability-rs, lingua
- quick-xml, printpdf, pdf-extract, lopdf

Dev-dependencies: tempfile, axum, wiremock.

## Features

- `default` — empty (no optional deps)
- `test-ssrf-bypass` — integration-test only; compiles in the SSRF escape hatch for loopback tests.
- `headless-browser` — opt-in; registers `mf_screenshot` while an engine binary resolves on `PATH`.

## Public API (crate root)

### Re-exported types

- **ToolOutput** (struct) — Result of a tool execution with content and optional metadata.
- **ToolContext** (struct) — Execution context passed to each tool (session_id, working_dir, event_bus, storage, code_index, config, read_timestamps, canonical_cache, allowed_roots).
- **ToolRegistry** (struct) — Tool registry managing available tools by name (`new`, `register`, `get`, `contains`, `definitions`).
- **Tool** (trait, async) — A tool an agent can invoke; provides name, description, parameters_schema, permission_category, and async execute.
- **create_extended_registry** (fn) — Creates a `ToolRegistry` with all extended tools registered.
- **check_path_within_root** (fn) — Re-exported path validation helper.

### Modules

- **archdoc** — Architecture-document content handling for `/spec govcreate` (spec `govdoc`).
- **codeindex_search** — `CodeIndexSearchTool` (`codeindex_search`).
- **codeindex_status** — `CodeIndexStatusTool` (`codeindex_status`).
- **codeindex_symbols** — `CodeIndexSymbolsTool` (`codeindex_symbols`).
- **codeindex_references** — `CodeIndexReferencesTool` (`codeindex_references`).
- **codeindex_dependencies** — `CodeIndexDependenciesTool` (`codeindex_dependencies`).
- **codeindex_reindex** — `CodeIndexReindexTool` (`codeindex_reindex`).
- **codeindex_godnodes** — `CodeIndexGodnodesTool` (`codeindex_godnodes`).
- **codeindex_path** — `CodeIndexPathTool` (`codeindex_path`).
- **codeindex_explain** — `CodeIndexExplainTool` (`codeindex_explain`).
- **codeindex_communities** — `CodeIndexCommunitiesTool` (`codeindex_communities`).
- **document_extract** — `DocumentFormat` (enum), `detect_document_format`, `ExtractedDocument`, `extract_file_as_markdown`.
- **http_request** — `HttpRequestTool` (`http_request`).
- **masterfetch** — MasterFetch web content engine. Types: `PageType`, `SourceType`, `PageMetadata`, `EnvelopeSignals`, `FetchResult`, `SearchResult`, `CrawlPage`. Tools: `MfFetchTool`, `MfCrawlTool`, `MfSearchTool`, `MfScreenshotTool`, `MfCacheClearTool`, `MfVersionTool`. Submodules: `cache`, `envelope`, `extractor`, `focus`, `http`, `language`, `links`, `metadata`, `pdf`, `robots`, `search`, `security`, `urlnorm`, `youtube`, `crawl` (`CrawlOrchestrator`, `CrawlConfig`, `CrawlResult`).
- **pdf_common** — `MAX_OUTPUT_BYTES`, `truncate_output`, and the `resolve_path` re-export shared by the PDF tools.
- **pdf_read** — `PdfReadTool` (`pdf_read`), `read_pdf`.
- **pdf_write** — `PdfWriteTool` (`pdf_write`).
- **project_scaffold** — Project scaffolding engine for the `/new` command (spec `newproj`).
- **task** — Session-scoped task management: `TaskCreateTool`, `TaskUpdateTool`, `TaskGetTool`, `TaskListTool`.
- **webfetch** — `WebFetchTool` (`webfetch`).
- **websearch** — `WebSearchTool` (`websearch`), `SearchResult`, `hits_from_metadata`.
- **storage** (inline) — `TaskRow`, `MemoryRow`, `StorageBackend` (trait), `Storage` (type alias).
- **event** (inline) — Compatibility re-export of `ragent_types::event::{Event, EventBus}`.