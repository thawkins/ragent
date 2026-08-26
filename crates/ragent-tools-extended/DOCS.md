# ragent-tools-extended

Extended document, web, memory, codeindex, finance, and browser tools for
ragent. Implements ~30 tool modules and the MasterFetch web content extraction
engine.

## Workspace Dependencies

- ragent-codeindex
- ragent-config
- ragent-storage
- ragent-tools-core
- ragent-types

## External Dependencies

- tokio, async-trait, futures, serde, serde_json, anyhow, thiserror, tracing, chrono, rand, uuid, dirs
- reqwest (with cookies/gzip/deflate), tokio-tungstenite, regex, percent-encoding, url, rusqlite
- html2text, readability-rs, base64, lingua
- docx-rust, calamine, rust_xlsxwriter, ooxmlsdk, zip, quick-xml, spreadsheet-ods, printpdf, pdf-extract, lopdf
- yfinance-rs, paft-decimal, paft-money
- ort, tokenizers, ndarray (optional, `embeddings` feature)

Dev-dependencies: tempfile, axum.

## Features

- `default` — empty (no optional deps)
- `embeddings` — enables local ONNX embedding model support (`ort`, `tokenizers`, `ndarray`)

## Public API (crate root)

### Re-exported types

- **ToolOutput** (struct) — Result of a tool execution with content and optional metadata.
- **ToolContext** (struct) — Execution context passed to each tool (session_id, working_dir, event_bus, storage, code_index, config, read_timestamps).
- **ToolRegistry** (struct) — Tool registry managing available tools by name with hidden-tool filtering.
- **Tool** (trait, async) — A tool an agent can invoke; provides name, description, parameters_schema, permission_category, and async execute.
- **create_extended_registry** (fn) — Creates a `ToolRegistry` with all extended tools registered.
- **check_path_within_root** (fn) — Re-exported path validation helper.

### Modules

- **browser** — `BrowserTool` (`browser`) — Chrome DevTools Protocol automation. Submodules: `actions`, `cdp` (`CdpConnection`, `TargetInfo`, `VersionInfo`, `CdpError`), `launch` (`find_browser_binary`).
- **channels** — `SendChannelMessageTool` (`send_channel_message`), `resolve_secret`.
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
- **finance** — Provider-agnostic finance data. `FinanceProvider` (trait), `Quote`, `OhlcvBar`, `Fundamentals`, `CurrencyRate`, `OptionContract`, `SearchResult`, `RecommendationPeriod`, `QuoteCache`, `RateLimiter`. Providers: `YahooFinanceProvider`, `PaidProvider`, `TwelveDataProvider`. Tools: `StockQuoteTool`, `StockHistoryTool`, `StockFundamentalsTool`, `CurrencyRateTool`, `CurrencyHistoryTool`, `StockSearchTool`, `StockOptionsTool`, `StockRecommendationsTool`.
- **gmail** — `GmailTool` (`gmail`), `GmailTokens`, `TokenStore` (trait), `SqliteTokenStore`, `GmailResolvedConfig`.
- **http_request** — `HttpRequestTool` (`http_request`).
- **libreoffice_common** / **libreoffice_info** / **libreoffice_read** / **libreoffice_write** — ODF document tools: `LibreInfoTool`, `LibreReadTool`, `LibreWriteTool`; helpers `read_odt`, `read_ods`, `read_odp`.
- **masterfetch** — MasterFetch web content engine. Types: `PageType`, `SourceType`, `PageMetadata`, `EnvelopeSignals`, `FetchResult`, `SearchResult`, `CrawlPage`. Tools: `MfFetchTool`, `MfCrawlTool`, `MfSearchTool`, `MfScreenshotTool`, `MfCacheClearTool`, `MfVersionTool`. Submodules: `cache`, `envelope`, `extractor`, `focus`, `http`, `language`, `links`, `metadata`, `pdf`, `robots`, `search`, `security`, `urlnorm`, `youtube`, `crawl` (`CrawlOrchestrator`, `CrawlConfig`, `CrawlResult`).
- **office_common** / **office_info** / **office_read** / **office_write** — Office (OOXML) tools: `OfficeInfoTool`, `OfficeReadTool`, `OfficeWriteTool`; helpers `read_docx`, `read_xlsx`, `read_pptx`.
- **pdf_read** — `PdfReadTool` (`pdf_read`), `read_pdf`.
- **pdf_write** — `PdfWriteTool` (`pdf_write`).
- **task** — Session-scoped task management: `TaskCreateTool`, `TaskUpdateTool`, `TaskGetTool`, `TaskListTool`.
- **webfetch** — `WebFetchTool` (`webfetch`).
- **websearch** — `WebSearchTool` (`websearch`), `SearchResult`, `hits_from_metadata`.
- **memory::embedding** — `EmbeddingProvider` (trait), `NoOpEmbedding`, `LocalEmbeddingProvider` (feature-gated), `cosine_similarity`, `serialise_embedding`, `deserialise_embedding`, `SimilarityResult`.
- **storage** (inline) — `TaskRow`, `MemoryRow`, `EmbeddingMatch`, `StorageBackend` (trait), `Storage` (type alias).
- **event** (inline) — Compatibility re-export of `ragent_types::event::{Event, EventBus}`.