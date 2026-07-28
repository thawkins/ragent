# Release Notes

## v0.1.0-beta.14

### Fixed — TUI read tool header always shows icon, and missing path surfaces in UI

- `tool_input_summary` in `crates/ragent-tui/src/widgets/message_widget.rs` now
  renders `📄 missing path` when a `read` call lacks a `path`, keeping the file
  icon visible and signalling malformed input.
- The existing `ReadTool::execute` error "Missing required 'path' parameter"
  prompts the LLM to correct the call.
- Added a TUI test for the missing-path placeholder.

## v0.1.0-beta.13

### Changed — Version bump

- Workspace version bumped from `0.1.0-beta.12` to `0.1.0-beta.13`.
- Added JCode cost accounting and fixed tool widgets.

## v0.1.0-beta.12

### Added — Per-run cost summary (`Event::RunCostSummary`)

- At the end of every `process_user_message` turn, the session processor now
  accumulates `Event::TokenUsage` totals, calls `compute_run_cost`, and publishes
  a single `Event::RunCostSummary` on the event bus.
- The summary carries `session_id`, `model_id`, `input_tokens`,
  `output_tokens`, `total_cost_usd`, and `duration_ms`.
- Cost computation respects user-defined price overrides from `ragent.json`
  and falls back to the built-in price table.
- The TUI logs a one-line `⟡ run complete` banner and updates the
  `ragent.cost.session` telemetry counter.
- The HTTP server serializes `RunCostSummary` as SSE event type
  `run_cost_summary`.

### Changed — Version bump

- Workspace version bumped from `0.1.0-beta.11` to `0.1.0-beta.12`.
- Formatting fixes to `/research` tooling.

### Fixed — Web-source direct quotations are hard-capped to 200 characters

- Added a post-processing pass in `crates/ragent-research/src/analysis.rs` that
  mechanically truncates inline double-quoted strings and fenced code blocks
  inside each finding's `**Observation:**` paragraph to 200 characters. This
  enforces the existing prompt instruction even when the synthesis model
  ignores it, so RESEARCH.md findings no longer contain oversized web-source
  excerpts.
- The cap is applied after both clean LLM parses and mechanical fallback
  findings, and only affects the Observation paragraph so analysis text,
  cross-references, and implications are left untouched.

### Fixed — Removed redundant `mf_fetch:` prefix from `RESEARCH.md` source citations

- `parse_mf_fetch_output` in `crates/ragent-agent/src/research_adapter.rs` now
  strips the leading `mf_fetch: <url>` header block from plain-text tool output
  (cache hits, PDF pages, YouTube transcripts, and error responses) before the
  research layer stores the page body.
- The source title fallback is now taken from the real content rather than the
  `mf_fetch:` header, so the per-finding **Sources** list no longer shows the
  redundant `mf_fetch: <url> — <url>` pattern.
- Unrelated plain-text tool responses are left unchanged because the strip only
  runs when the first non-empty line starts with `mf_fetch:`.

### Fixed — `/research create` no longer crashes on html2text renderer panics

- `masterfetch::extractor::extract_markdown` now wraps the full readability →
  html2text → raw-text chain in a top-level `std::panic::catch_unwind`. If
  `html2text` panics on real-world HTML (e.g. mdBook-generated pages such as
  `rust-book.cs.brown.edu`), the extractor degrades to raw tag-stripped text
  instead of aborting the web-gatherer task or the whole ragent process.
- Added a regression test in `test_mf_extractor.rs` using a captured mdBook HTML
  fixture; it triggers the known html2text overflow and verifies the extractor
  returns a non-empty fallback result.

### Added — PDF and YouTube text extraction in `/research create`

- `mf_fetch` now extracts readable text from PDF responses instead of returning
  raw binary bytes. It detects `Content-Type: application/pdf` and `.pdf` URLs,
  runs `pdf_extract::extract_text_from_mem` in a blocking task, and surfaces the
  document title from the PDF `/Info` dictionary (with UTF-16BE BOM handling).
- `mf_fetch` now extracts timestamped captions from YouTube watch pages. It
  parses the embedded `ytInitialPlayerResponse` JSON, selects the best caption
  track (default, then English, then first available), fetches the caption XML,
  and formats each caption with a `[MM:SS]` timestamp.
- PDF and YouTube `mf_fetch` outputs set `page_type: pdf` / `page_type: youtube`
  and include `content_type` in metadata so the research layer classifies and
  counts them correctly.
- Updated `WebSourceKind::YouTube` documentation to reflect that transcript
  extraction is now implemented.
- `WebGatherer::fetch_url_as_source` now classifies `--from-url` seed sources by
  their fetched `content_type` / URL instead of always reporting `media_type: page`.
- Added `crates/ragent-tools-extended/tests/test_mf_pdf.rs` and
  `crates/ragent-tools-extended/tests/test_mf_youtube.rs` covering PDF text/title
  extraction, YouTube caption parsing, and end-to-end transcript fetch against a
  local mock caption server.
- Added `ragent-research` tests verifying that `fetch_url_as_source` and
  `gather_with_observer` classify and count PDF and YouTube sources correctly.

### Added — `/research create` richer fetch metadata and PDF/YouTube counters

- Research web gathering now prefers the `mf_fetch` tool over legacy `webfetch`.
  `mf_fetch` returns a structured envelope with `content_type`, `page_type`, and
  `metadata.title`, giving the research layer reliable media classification and
  better page titles.
- Added `WebFetchedPage.content_type` and `WebFetchedPage.page_type` to carry
  the richer `mf_fetch` metadata through the gatherer.
  - Added `Source::Web` fields `content_type`, `page_type`, and `media_type`
    (`"page" | "pdf" | "youtube"`) with serde defaults so older `RESEARCH.md`
    files remain backward compatible.
  - Added `Source::media_type()` and a new **Media** column in the
    `RESEARCH.md` References Index table so every source shows its classified
    media type (`page`, `pdf`, `youtube`) alongside the existing Type column.
    Non-web sources render `—`.
- New `WebSourceKind` enum and `classify_web_source()` helper classify sources
  from `Content-Type: application/pdf`, `.pdf` URLs, and YouTube hosts.
- `GatherResult`, `SessionEvent::Done`, `RunOutcome`, and the TUI
  `ResearchProgress` tracker now carry `pdf_count` and `youtube_count`.
- CLI `ragent research create` prints recovered PDF and YouTube counts in the
  completion summary (e.g. "created research/foo (5 sources, 2 PDFs, 1 YouTube
  video)").
- TUI `/research create` progress summary now shows PDF and YouTube counts both in
  the live tracker render and in the status-bar completion message.
- Research event JSON output (`render_session_event_json`) includes
  `pdf_count` and `youtube_count` in the `done` payload.
- Added `url` crate dependency to `ragent-research` for host-based YouTube
  classification.

## v0.1.0-beta.11

### Changed — Version bump

- Workspace version bumped from `0.1.0-beta.10` to `0.1.0-beta.11`.
- Moved Tavily search backend into the `mf_search` multi-engine framework.

## v0.1.0-beta.10

### Added
- Tavily search backend migrated into the `mf_search` multi-engine framework; it now runs in parallel with DuckDuckGo, Brave, and optional LangSearch.
- New `TavilyEngine` implementing the `SearchEngine` trait in `ragent-tools-extended`.
- `mf_search` description updated to mention Tavily and both optional API keys.
- Research system (`ragent-research`) now depends on `ragent-tools-extended` and its web-gathering adapter prefers `mf_search`, falling back to legacy `websearch`.
- New `parse_mf_search_metadata` helper maps `mf_search` JSON metadata into research-layer `WebSearchHit` rows while preserving `search_tool` and `search_engine` provenance.
- TUI now recognises `mf_search` in tool input/result summaries.
- Fixed missing `tempfile` and `calamine` dev-dependencies in `ragent-bench` so the workspace test suite compiles.

### Changed
- Legacy `websearch` tool docs now clarify it is retained for direct agent use and backwards compatibility; research workflows prefer `mf_search`.

### Fixed
- `WebSearchHit` provenance fields are now populated consistently across `websearch`, `mf_search`, and research output.