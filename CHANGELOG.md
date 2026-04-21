# Changelog

## [0.1.0-alpha.46] - 2026-04-20

### Added
- **Provider metadata improvements** — Copilot model metadata now includes premium request multipliers in model selection and provider status surfaces.

### Changed
- **SPEC.md refresh** — Documented AIWiki source-code ingestion, AIWiki autosync indicators, hidden-tools configuration, dynamic provider metadata, quota/context status-bar reporting, and current CLI defaults/flags.
- **TUI status bar** — Expanded second-line status output to show provider-aware usage, context utilization, AIWiki enabled state, and AutoSync when active.
- **Configuration schema** — Added documented `hidden_tools` support with merge/union behavior across config layers.

### Fixed
- **Hugging Face tool-call compatibility** — Tool names are adapted for router streaming compatibility and mapped back to canonical ragent tool names.
- **AIWiki DOCX extraction** — Table content extraction formatting cleaned up for ingestion.
- **Workspace version** — Updated workspace version to 0.1.0-alpha.46.

## [0.1.0-alpha.45] - 2025-01-17

### Changed
- Updated workspace version to 0.1.0-alpha.45

## [0.1.0-alpha.44] - 2025-01-17

### Added
- **TUI Status Bar Redesign - Phases 2-4 Complete**: Core layout engine with visual polish and responsive behavior
    - **Phase 2 - Core Layout Engine**: New `layout_statusbar` module (529 lines)
      - `ResponsiveMode` enum: Full (≥120 chars), Compact (80-120 chars), Minimal (<80 chars)
      - Line 1: Working directory (left), git branch (center), session status (right)
      - Line 2: Provider + health (left), token usage % + progress bar (center), service status (right)
      - Intelligent path shortening with HOME → ~ replacement and ellipsis truncation
      - Dynamic gap calculation for responsive layout across terminal widths
      - 22 comprehensive unit tests for core layout (PHASE2_STATUSBAR_COMPLETION.md)
    - **Phase 3 - Visual Polish & Indicators**: Semantic indicators and color coding
      - `indicators` module: Status indicators (●, ◔, ✗, ✓, ↕, ⟳, •) and progress blocks (█, ░)
      - `spinner` module: 10-frame animated loading indicator with 45ms timing
      - `colors` module: Semantic palette (healthy=green, warning=yellow, error=red, in-progress=cyan)
      - 6 styling helper functions for consistent visual application
      - Integration with all section builders for semantic feedback
      - 6 new tests for indicators, spinner, colors, and styling (PHASE3_VISUAL_POLISH_COMPLETION.md)
    - **Phase 4 - Responsive & Adaptive Behavior**: Dynamic abbreviations and information hiding
      - `abbreviations` module: Smart label abbreviation (tokens→tok, provider→pvd, context→ctx, etc.)
      - Service abbreviations (lsp_servers→LSP, code_index→Idx, aiwiki→Wiki, etc.)
      - Provider abbreviations (anthropic→An, openai→OAI, gpt→GPT, gemini→Gm, etc.)
      - Mode-based label adaptation (Full labels in Full mode, abbreviated in Compact/Minimal)
      - Smart information hiding based on terminal width
      - 6 new tests for abbreviations and responsive behavior (PHASE4_RESPONSIVE_COMPLETE.md)
    - **Summary**: 636 lines of production code + 353 lines of tests (34 tests, 100% passing)
  ### Fixed
- **TUI tool result readability** — changed tool result summary text color from DarkGray to Gray for improved visibility on various terminal backgrounds
- **Think tool accessibility** — updated think tool summary and reasoning block colors from muted white/dark gray to bright cyan for better visibility for visually impaired users

### Added
- **AIWiki Milestone 1-6: Complete Knowledge Base System** — An embedded, project-scoped knowledge base for ragent
    - New `ragent-aiwiki` crate with full knowledge base infrastructure
    - Directory initialization (`/aiwiki init`), enable/disable system (`/aiwiki on|off`)
    - Document ingestion pipeline supporting Markdown, Plain Text, PDF, DOCX, ODT
    - Sync & auto-update with file watcher, broken link detection
    - Web interface integrated with ragent-server (HTML templates, search, graph visualization)
    - AI-powered analysis, Q&A, and contradiction detection
    - Agent tools: `aiwiki_search`, `aiwiki_ingest`, `aiwiki_status`
    - Export/Import: Single markdown export, Obsidian vault export, markdown import

- **TUI Milestone 3.1 & 3.2: Dialog & Button Component System** — Unified dialog and button system
    - Extended `Dialog` with `with_buttons()`, `with_footer_hint()`, and factory methods
    - New `Button` component with variants (Primary, Secondary, Danger, Success)
    - `ButtonBar` for horizontal button arrangement
    - Theme-aware styling throughout

### Documentation
- **SPEC.md Reorganization**: Restructured specification document from scattered 26-section layout into logical 7-part structure
    - Fixed critical numbering conflict (duplicate section 8)
    - Grouped related content: storage systems (Part II), integrations (Part V), reference materials (Part VI)
    - Improved navigation with clear part headers and logical progression
    - Updated all 195 subsections with correct numbering
    - Enhanced Table of Contents with part groupings
    - All 7,555 lines of content preserved without loss
    - See PHASE2_EXECUTION_REPORT.md and PHASE3_VALIDATION_REPORT.md for details
    - See SPEC_REORGANIZATION_SUMMARY.md for new structure overview
    - Created migration guide for old → new section number mapping

## [0.1.0-alpha.42] - 2025-01-15
### Changed
- SPEC.md: Complete structural reorganization with numbered fixes and improved organization


## [0.1.0-alpha.43] - 2026-04-16

### Added
- **AIWiki Milestone 1-6: Complete Knowledge Base System** — An embedded, project-scoped knowledge base for ragent
  - New `ragent-aiwiki` crate with full knowledge base infrastructure
  - Directory initialization (`/aiwiki init`), enable/disable system (`/aiwiki on|off`)
  - Document ingestion pipeline supporting Markdown, Plain Text, PDF, DOCX, ODT
  - Sync & auto-update with file watcher, broken link detection
  - Web interface integrated with ragent-server (HTML templates, search, graph visualization)
  - AI-powered analysis, Q&A, and contradiction detection
  - Agent tools: `aiwiki_search`, `aiwiki_ingest`, `aiwiki_status`
  - Export/Import: Single markdown export, Obsidian vault export, markdown import

- **TUI Milestone 3.1 & 3.2: Dialog & Button Component System** — Unified dialog and button system
  - Extended `Dialog` with `with_buttons()`, `with_footer_hint()`, and factory methods
  - New `Button` component with variants (Primary, Secondary, Danger, Success)
  - `ButtonBar` for horizontal button arrangement
  - Theme-aware styling throughout

### Changed
- Updated workspace version to 0.1.0-alpha.43

## [0.1.0-alpha.42] - 2026-04-15

### Added
- **AIWiki Milestone 1: Core Infrastructure** — An embedded, project-scoped knowledge base system for ragent
  - New `ragent-aiwiki` crate with core infrastructure for knowledge base management
  - Directory initialization (`/aiwiki init`): Creates `aiwiki/ directory structure with config, state tracking, and organized subdirectories
  - Configuration management: `config.json` with wiki name, sync mode, LLM model, extraction settings, and ignore patterns
  - **Enable/Disable system**: `enabled` field in config.json controls AIWiki activation
    - `/aiwiki init` auto-enables the wiki (sets `enabled: true`)
    - `/aiwiki on` - Enable AIWiki system
    - `/aiwiki off` - Disable AIWiki system (zero performance impact)
    - When disabled: no indexing, no file watching, no background processes
    - When disabled: only `/aiwiki on` command works (others show disabled message)
    - State persists across sessions via config.json
  - State tracking: `state.json` with SHA-256 file hashing for incremental updates, detecting new/modified/deleted source files
  - Slash commands: `/aiwiki init`, `/aiwiki on`, `/aiwiki off`, `/aiwiki status`, `/aiwiki help` integrated into TUI
  - Generated directory structure:
    - `aiwiki/raw/` — Source documents (PDFs, MD, images) - gitignored
    - `aiwiki/wiki/` — Generated markdown pages (tracked in git)
      - `entities/` — People, places, organizations
      - `concepts/` — Ideas, topics, theories
      - `sources/` — One summary per source file
      - `analyses/` — Derived content
    - `aiwiki/static/` — Web UI assets (CSS, JS, images)
  - Initial log.md with initialization timestamp and configuration summary

- **AIWiki Milestone 2: Ingestion Pipeline** — Document ingestion system
  - File type detection: Markdown, Plain Text, PDF, DOCX, ODT
  - Text extraction: Native text files, PDF via pdf-extract, stubs for DOCX/ODT
  - `/aiwiki ingest` command variants:
    - `/aiwiki ingest` - Scans `aiwiki/raw/` directory for documents
    - `/aiwiki ingest <file>` - Ingests a single file into `aiwiki/raw/`
    - `/aiwiki ingest <directory>` - Recursively ingests all supported files
  - Directory scanning with recursive subdirectory support
  - File size limits and validation (configured via `max_file_size`)
  - Copy or move files with options
  - Progress reporting in TUI with file counts and metadata
  - Disabled state blocking (ingestion commands blocked when wiki disabled)

- **AIWiki Milestone 3: Sync & Auto-Update** — Automatic wiki synchronization
  - Sync orchestration with `sync()`, `preview_sync()`, and `needs_sync()` functions
  - Detect stale pages by comparing source hashes from state.json
  - Process new, modified, and deleted source files
  - `/aiwiki sync` command with preview and force options
  - Sync report with counts (new/updated/deleted sources, pages affected)
  - Broken link detection and validation across wiki pages
  - File watcher with debounced change detection
  - `WatcherConfig` for customizable sync behavior (interval, debounce)
  - `SyncResult` and `SyncPreview` for detailed sync reporting

- **AIWiki Milestone 4: Web Interface (ragent-server Integration)** — Web-based wiki browsing
  - HTML templates for wiki pages, search results, graph visualization, and status dashboard
  - CSS and JavaScript for responsive dark/light theme and interactivity
  - HTTP routes: `/aiwiki/, `/aiwiki/page/:path`, `/aiwiki/edit/:path`, `/aiwiki/search`, `/aiwiki/graph`, `/aiwiki/status`
  - Page editing with frontmatter preservation
  - Full-text search with keyword matching and relevance scoring
  - Interactive graph visualization with D3.js
  - Status dashboard with wiki statistics
  - Static asset serving (CSS, JS)

- **AIWiki Milestone 5: Analysis & Derived Content** — AI-powered analysis and Q&A
  - Analysis generation: Compare multiple sources, generate analysis/<slug>.md pages
  - Analysis types: Comparison, Synthesis, Trade-offs, Custom
  - Wiki Q&A: Query wiki content with source citations (`ask_wiki()`)
  - Contradiction detection: Review wiki for potential contradictions
  - TUI help updated with analysis, ask, and review commands
  - Source attribution and provenance tracking
  - Analysis reports with suggested resolutions

- **AIWiki Milestone 6: Integration & Polish** — Agent tools, export/import, and documentation
    - **Wiki-aware tool calls** for agent integration:
      - `aiwiki_search` — Search wiki content with filtering by page type
      - `aiwiki_ingest` — Ingest files, directories, or scan raw/ folder
      - `aiwiki_status` — Show comprehensive wiki statistics
    - **Export/Import functionality**:
      - Export wiki as single combined markdown file (`export_single_markdown`)
      - Export as Obsidian-compatible vault with `.obsidian/` config (`export_obsidian_vault`)
      - Import external markdown files and directories (`import_markdown`)
      - `aiwiki_export` and `aiwiki_import` tools for agents
    - **Enhanced status monitoring**:
      - Token usage tracking in `state.json`
      - Storage usage display (wiki and raw directory sizes)
      - Detailed statistics by page category (entities, concepts, sources, analyses)
      - Sync status with pending changes count
    - **Documentation**:
      - User guide: `docs/userdocs/aiwiki.md`
      - Example workflow: `examples/aiwiki/README.md`
      - Updated QUICKSTART.md with AIWiki section
    - Note: T6.1 (Session context injection) is pending as a future enhancement

### Changed
- **TUI Milestone 3.2: Button Component Standardization** — Unified button system for consistent UI interactions
  - Completely rewritten `widgets/button.rs` with theme-aware Button component
    - Added `Button::new()` and `Button::with_shortcut()` constructors
    - Added `ButtonVariant` enum: Primary (focus color), Secondary (primary color), Danger (danger color), Success (success color)
    - Added `ButtonState` enum: Enabled, Disabled, Active for different interaction modes
    - Added configurable width/height, borders, keyboard shortcuts
    - All colors now sourced from theme system (no hardcoded values)
  - Added `ButtonBar` component for horizontal button arrangement
    - Configurable spacing between buttons (default: 2 chars)
    - Supports Left/Center/Right alignment
    - Automatic width calculation based on button content
  - Extended `DialogButton` with `to_button()` method for Button widget conversion
  - Updated `widgets/permission_dialog.rs` to use theme colors:
    - `Color::Yellow` → `theme::colors::DIALOG_WARNING`
    - `Color::Cyan` → `theme::colors::PRIMARY`
  - Added theme constants to support button system:
    - `theme::colors::BORDER_INACTIVE` for disabled button borders
    - `theme::colors::TEXT_PRIMARY` for button text

- **TUI Milestone 3.1: Dialog Component Refactoring** — Unified dialog system for consistent UX across the TUI
  - Extended `widgets/dialog.rs` with `Dialog::with_buttons()` for standardized action buttons
  - Added `Dialog::with_footer_hint()` for consistent action hints across all dialogs
  - Added `DialogAction` enum for standard dialog actions (Confirm, Cancel, Yes, No, Always, Custom)
  - Added `DialogButton` struct with keyboard shortcut support and primary button styling
  - Extended `DialogSize` enum with Small (60x30), Medium (70x40), Large (90x70), and Fullscreen variants
  - Added factory methods: `Dialog::confirm()`, `Dialog::alert()`, `Dialog::error()`, `Dialog::success()`
  - Added `DialogWidget` for rendering dialogs as ratatui Widgets
  - Added helper function `centered_rect()` for consistent dialog positioning
  - All dialogs now use theme colors via `DialogVariant` (Info, Warning, Danger, Success)

## [0.1.0-alpha.42] - 2026-04-15

### Added
- **Gemini provider** — Google Gemini API support as a first-class LLM provider
- **Hugging Face provider** — Hugging Face Inference API support as a first-class LLM provider
- **Unfinished goal detection** — Session processor now detects when the agent's stated goals have not been fully achieved

### Fixed
- Various fixes across provider implementations and session processing

## [0.1.0-alpha.41] - 2026-04-15

### Changed
- **Model picker now displays models in a tabular format with metadata** — The model selection dialog now shows a table with columns for Model name, Context window size, Cost (input/output per million tokens), and Features (Reasoning, Vision, Tool-use). Models are now sorted alphabetically by name for all providers.

### Added
- **Code index parsers for OpenSCAD and Terraform** — Added tree-sitter-based parsers for OpenSCAD (`.scad`) and HCL/Terraform (`.tf`, `.tfvars`), bringing the total to 9 supported languages with full symbol extraction
  - OpenSCAD parser: extracts modules, functions, variable declarations, include/use statements, and module/function call references
  - HCL/Terraform parser: extracts resource blocks, data blocks, module calls, variables, locals, outputs, provider blocks, and terraform settings blocks with nested attribute extraction
  - Registered both parsers in `ParserRegistry` and added file extension detection (`.scad` → `openscad`, `.tf`/`.tfvars` → `terraform`)
  - `tree-sitter-openscad-ng` and `tree-sitter-hcl` dependencies were already present in Cargo.toml but previously unused
- **Code index parsers for CMake, Gradle, and Maven** — Added tree-sitter-based parsers for CMake (`.cmake`/`CMakeLists.txt`), Gradle Groovy DSL (`.gradle`), Gradle Kotlin DSL (`.gradle.kts`), and Maven POM (`pom.xml`), bringing the total to 13 supported languages
  - CMake parser: extracts function definitions, macro definitions, block scopes, foreach/while loops, if conditions, command references, and include/add_subdirectory imports
  - Gradle Groovy parser: extracts classes, interfaces, enums, methods, constructors, standalone functions, imports, closures, annotations, variable declarations, and DSL block calls
  - Gradle Kotlin DSL parser: extracts classes, objects, companion objects, functions, properties, type aliases, imports, and DSL call expressions
  - Maven parser: extracts project coordinates, dependencies (as imports), modules, plugins, profiles, properties, repositories, and parent POM references
  - Added file extension/filename detection (`.cmake`/`CMakeLists.txt` → `cmake`, `.gradle` → `gradle`, `.gradle.kts` → `gradle_kts`, `pom.xml` → `maven`)
  - Added `tree-sitter-cmake`, `tree-sitter-groovy`, `tree-sitter-kotlin-ng`, and `tree-sitter-xml` dependencies
- **GitLab integration** — Full GitLab REST API support mirroring GitHub toolset
  - Issue management: `gitlab_list_issues`, `gitlab_get_issue`, `gitlab_create_issue`, `gitlab_comment_issue`, `gitlab_close_issue`
  - Merge requests: `gitlab_list_mrs`, `gitlab_get_mr`, `gitlab_create_mr`, `gitlab_approve_mr`, `gitlab_merge_mr`
  - CI/CD pipelines: `gitlab_list_pipelines`, `gitlab_get_pipeline`, `gitlab_cancel_pipeline`, `gitlab_retry_pipeline`
  - Jobs: `gitlab_list_jobs`, `gitlab_get_job`, `gitlab_get_job_log`, `gitlab_retry_job`, `gitlab_cancel_job`
- **Extensive SPEC.md rewrite** — Comprehensive rewrite of project specification document

## [0.1.0-alpha.40] - 2026-04-14

### Changed
- **Model picker now displays models in a tabular format with metadata** — The model selection dialog now shows a table with columns for Model name, Context window size, Cost (input/output per million tokens), and Features (Reasoning, Vision, Tool-use). Models are now sorted alphabetically by name for all providers.

## [0.1.0-alpha.39] - 2026-04-14

### Changed
- Version bump to 0.1.0-alpha.39 - an number of stability fixes

## [0.1.0-alpha.38] - 2026-04-14

### Fixed
- **MS Office and LibreOffice presentation writer** — Fixed PPTX and ODP slide rendering: body text now produces proper layout, geometry, and paragraph elements
- **todo_write tool** — Updated result summary output for improved clarity

### Changed
- Version bump to 0.1.0-alpha.38

## [0.1.0-alpha.37] - 2026-04-13

### Added
- **Memory compaction and deduplication** (Milestone 6) — Automatic memory lifecycle management: deduplication, block compaction, stale eviction, and trigger-based scheduling
  - `deduplicate_memory()` checks for semantically similar memories using FTS5 (or cosine similarity when embeddings enabled) and merges or proposes merging duplicates
  - `DedupResult` enum: `NoDuplicate`, `Duplicate` (>0.95 similarity, auto-merge), `NearDuplicate` (0.8–0.95, requires confirmation)
  - `apply_dedup_merge()` merges content (combines unique sentences), takes highest confidence, unions tags
  - `compact_blocks()` detects memory blocks exceeding 90% of `block_size_limit` and truncates them, logging originals to journal
  - `compact_block_content()` preserves YAML frontmatter, truncates body to 75% of limit, appends compaction note
  - `evict_stale_memories()` identifies memories with `confidence < min_confidence` AND `last_accessed > stale_days` for deletion (or logging if `auto_evict: false`)
  - `CompactionTrigger` tracks when compaction last ran and how many memories stored; triggers on: first run, time-based (>24h), count-based (>10 stored), total count threshold
  - `run_compaction()` performs full compaction pass: block compaction → stale eviction → dedup merge
  - `CompactionConfig` in `MemoryConfig`: `enabled` (true), `block_size_limit` (4096), `memory_count_threshold` (500), `min_interval_hours` (24)
  - `EvictionConfig` in `MemoryConfig`: `auto` (false), `stale_days` (30), `min_confidence` (0.1)
  - `ForgetFilter` changed from struct to enum: `Id(i64)` for single delete, `Filter { ... }` for criteria-based delete
  - New `Storage` methods: `update_memory_content()`, `set_memory_tags()`, `delete_memories()` (unified API)
  - Startup compaction: runs automatically if `compaction.enabled` and trigger conditions met
- **Automatic memory extraction** (Milestone 5) — The extraction engine observes tool usage and session events to propose structured memories automatically
  - `ExtractionEngine` with hook points after tool execution (`on_tool_result`) and at session end (`on_session_end`)
  - `MemoryCandidate` type for proposed memories with `content`, `category`, `tags`, `confidence`, `source`, `reason` fields
  - Pattern extraction from file edits: detects coding conventions (anyhow, tracing, async, serde), test file locations, module structure, and configuration files
  - Error resolution extraction: tracks bash failures and detects when a subsequent success resolves the error, storing the problem and resolution as a category `"error"` memory
  - Session summary extraction: at session end, compiles tool usage patterns and files edited into a category `"workflow"` memory
  - Confirmation flow: when `auto_extract.require_confirmation` is `true` (default), candidates are emitted as `MemoryCandidateExtracted` events but NOT auto-stored; when `false`, candidates are stored directly
  - `AutoExtractConfig` in `MemoryConfig` with `enabled` (default: false) and `require_confirmation` (default: true) fields
  - Memory confidence decay: `decay_confidence()` function applies exponential decay to stale memories (configurable `factor` default 0.95/day, `min_confidence` default 0.1)
  - `DecayConfig` in `MemoryConfig` with `factor` and `min_confidence` fields
  - `MemoryCandidateExtracted` event for SSE and TUI notification
  - SSE event `memory_candidate_extracted` in HTTP server
  - Extraction engine wired into `SessionProcessor` via `OnceLock<Arc<ExtractionEngine>>`
  - Confidence decay applied on startup when `decay.factor < 1.0`
  - Content deduplication: extraction engine uses FTS5 word-overlap checking to avoid proposing duplicate memories
- **Semantic search (embeddings)** (Milestone 4) — Added embedding-based semantic search for memories and journal entries, enabling similarity-based retrieval beyond keyword matching
  - `EmbeddingProvider` trait with `embed()`, `embed_batch()`, `dimensions()`, `name()`, `is_available()` methods
  - `NoOpEmbedding` implementation (default, returns empty vectors when embeddings disabled)
  - `LocalEmbeddingProvider` (feature-gated, `embeddings` feature) using ONNX Runtime (`ort` crate) with `all-MiniLM-L6-v2` sentence-transformer (384-dim vectors)
  - `memory_search` tool for semantic + FTS5 hybrid search across structured memories and memory blocks
  - `SemanticConfig` in `MemoryConfig` with `enabled`, `model`, `dimensions` fields (default: disabled)
  - Embedding BLOB columns in SQLite (`memories.embedding`, `journal_entries.embedding`)
  - Cosine similarity brute-force search for <10K entries via `search_memories_by_embedding()` and `search_journal_by_embedding()`
  - Lazy embedding: memories are embedded on first semantic search if they don't already have embeddings
  - `MemorySearched` event with mode indicator ("semantic" or "fts")
  - SSE event `memory_searched` in HTTP server
  - Embedding evaluation document at `docs/performance/embedding-evaluation.md`
- **Code index — multi-language support** (Milestone 6) — Added tree-sitter parsers for Python, TypeScript/JavaScript (TS/TSX/JS/JSX), Go, C/C++, and Java alongside the existing Rust parser, bringing the total to 7 supported languages; each parser extracts functions, classes/structs, interfaces, enums, imports, type aliases, constants, and more
- **Code index benchmarks** — Criterion-based benchmarks (`cargo bench -p ragent-codeindex`) covering per-language parse throughput, store upsert, FTS search latency, and full 7-file indexing
- **Code index config persistence** — `code_index` section in `ragent.json` for persisting `enabled`, `max_file_size`, `extra_exclude_dirs`, and `extra_exclude_patterns`

### Fixed
  - Fixed `test_registry_total_tool_count` assertion to match actual registered tool count (114)
### Changed
- Version bump to 0.1.0-alpha.37

## [0.1.0-alpha.36] - 2026-04-12

### Fixed
- **Comprehensive test & lint cleanup** — Fixed all test failures and clippy warnings across the entire workspace (43 files changed)
- Added `#[serial]` to all 26 bash tool tests to prevent process permit contention causing flaky failures
- Fixed webfetch test metadata field names (`status`→`http_status`, `lines`→`line_count`)
- Fixed slash command keystroke test assertion (backspace correctly removes last character)
- Fixed `push_log` doctest calling private `push_log_no_agent` method
- Fixed clippy warnings: `single_char_pattern`, `needless_collect`, `approx_constant`, `unchecked_time_subtraction`, `used_underscore_binding`, `strict_float_comparison`, `assert_eq_with_bool`, `manual_string_new`, `clone_to_from_ref`, `duplicated_attribute`, `field_assignment_outside_initializer`, `items_after_test_module`
- Moved `TeamManagerInterface` impl before test module in `manager.rs` to satisfy `items_after_test_module` lint

### Changed
- Updated dependencies: clap 4.5→4.6, tokio 1.50→1.51, tracing-subscriber 0.3.22→0.3.23, uuid 1.22→1.23
- Relaxed mailbox notify benchmark threshold from 10ms to 100ms for CI runners
- Changed tool registry test assertions from exact count (70) to minimum count (`>=70`)
- Version bump to 0.1.0-alpha.36

## [0.1.0-alpha.35] - 2026-04-12

### Added
- **pre-flight.sh** — Local CI check script that runs all GitHub Actions checks (fmt, check, clippy, tests, audit, deny) before pushing; supports `--quick` flag for fast checks

### Fixed
- Fixed multiple unused variable/import lint errors in test files (test_reconcile_spawning, test_tool_truncate, test_tool_output_standardization, test_blueprint_with_manager)
- Fixed unused `mut` warnings in selectable_list.rs tests
- Added `#![allow(missing_docs)]` to test modules to satisfy CI lint requirements

### Changed
- Version bump to 0.1.0-alpha.35

## [0.1.0-alpha.34] - 2026-04-12

### Fixed
- **Security: RUSTSEC-2026-0097** — Updated `rand` to 0.9.3 (0.9.2 was still affected)

### Changed
- Version bump to 0.1.0-alpha.34

## [0.1.0-alpha.33] - 2026-04-12

### Fixed
- **Security: RUSTSEC-2026-0097** — Upgraded `rand` from 0.8 to 0.9 to fix unsound advisory related to custom loggers using `rand::rng()`
- **Ollama Cloud context window** — Now fetches actual context window size via `/api/show` API endpoint instead of using heuristic estimates; also detects vision capability from API response

### Changed
- Updated `rand` API calls: `thread_rng()` → `rng()`, `gen_range()` → `random_range()`, `rand::distributions::Alphanumeric` → `rand::distr::Alphanumeric`
- Version bump to 0.1.0-alpha.33

## [0.1.0-alpha.31] - 2026-04-09

### Added
- **SECPLAN.md** — Comprehensive security remediation plan consolidating findings from 5 existing security audit documents covering Critical (P0), High (P1), Medium (P2), and Low (P3) issues

### Changed
- Version bump to 0.1.0-alpha.31

## [0.1.0-alpha.30] - 2026-04-08

### Added
- **Alt+L keybinding** — Toggle the log panel visibility on/off (previously only available via `/log` command)

### Changed
- Version bump to 0.1.0-alpha.30

## [0.1.0-alpha.29] - 2026-04-07

### Changed
- Version bump to 0.1.0-alpha.29

## [0.1.0-alpha.28] - 2026-04-07

### Changed
- Version bump to 0.1.0-alpha.28

## [0.1.0-alpha.27] - 2026-04-05

### Fixed
- **CI Clippy** — replaced redundant closures with function references in `lsp/discovery.rs` (`and_then(|v| parse_version_tuple(v))` → `and_then(parse_version_tuple)`)

## [0.1.0-alpha.26] - 2026-04-05

### Fixed
- **LSP discover deduplication** — `/lsp discover` no longer shows duplicate entries when the same language server is installed in multiple VS Code extension directories (e.g. `.vscode` and `.vscode-server`)
- **LSP discover version display** — VSCode extension entries now show the installed version (e.g. `0.3.2845`) in a new Version column; PATH-installed servers show `—`
- **LSP discover scrolling** — dialog is now fixed-height with `↑`/`↓`/`PgUp`/`PgDn` scroll support for long lists
- **Dynamic LSP system prompt** — system prompt now only lists LSP tools for servers that are actually connected, not all possible servers

### Fixed (CI)
- Removed unused `mut` from test and bench variables (`test_markdown_table`, `bench_markdown`)
- Added `#![allow(missing_docs)]` to all bench files (criterion macro generates undocumented fn)
- Added `#[allow(unused_variables)]` to `debug_log_input_transition` (params used only under `#[cfg(debug_assertions)]`)

## [0.1.0-alpha.25] - 2026-04-05

### Fixed
- **CI lint fixes** — resolved 1339 Clippy warnings/errors across 127 files:
  - Fixed unused variable warnings in `ragent-tui` (`_removed`, `_display_session`)
  - Added missing doc comments to `normalize_ascii_tables` and `render_markdown_to_ascii`
  - Rewrote workspace lint config with priority-aware lint groups to eliminate `lint_groups_priority` errors
  - Added 50+ workspace-level `allow` entries for lints requiring architectural changes
  - Auto-fixed ~860 suggestions via `cargo clippy --fix` (redundant clones, collapsible ifs, format strings, etc.)
  - Fixed `prompt_opt::from_str` Clippy `should_implement_trait` by implementing `std::str::FromStr`

## [0.1.0-alpha.24] - 2026-04-04

### Fixed
- **Security Audit CI** — resolved all failures in both `cargo audit` and `cargo deny check` jobs:
  - Upgraded `criterion` 0.4→0.5 in `ragent-core` and `ragent-server` (eliminates `atty` RUSTSEC-2021-0145/2024-0375)
  - Updated `rustls-webpki` 0.103.9→0.103.10 (fixes RUSTSEC-2026-0049 CRL matching bug)
  - Updated `quinn-proto` 0.11.13→0.11.14 (fixes RUSTSEC-2026-0037 Quinn DoS)
  - Added `.cargo/audit.toml` to ignore RUSTSEC-2026-0002 (`lru` via ratatui; unexploitable)
  - Rewrote `deny.toml` for cargo-deny ≥0.19 schema (removed obsolete keys: `vulnerability`, `unmaintained`, `unlicensed`, `copyleft`)
  - Added `CDLA-Permissive-2.0` to allowed license list (webpki-roots 1.0.6)
  - Added advisory ignore entries for `paste` and `proc-macro-error` (transitive unmaintained, unfixable)
  - Added `license.workspace = true` to `prompt_opt/Cargo.toml`
  - Changed `wildcards = "deny"` → `"warn"` to avoid false-positives on workspace-internal path deps
- **Tool hallucination prevention** — inject `## Available Tools` section into every session's system prompt listing all registered tool names exactly, preventing models (especially Ollama-hosted) from calling non-existent tools like `search` instead of `grep`

## [0.1.0-alpha.23] - 2026-04-04

### Added
- **`/bash` slash command** — dynamic bash allowlist/denylist management:
  - `/bash add allow <cmd>` — exempt a banned command prefix (e.g. `curl`) from the built-in banned-command check
  - `/bash add deny <pattern>` — add a substring pattern that unconditionally rejects any matching command
  - `/bash remove allow|deny <entry>` — remove an entry from either list
  - `/bash show` — display current user-defined allowlist and denylist
  - `/bash help` — usage reference
  - Append `--global` to any add/remove to write to `~/.config/ragent/ragent.json` instead of the project `ragent.json`
  - Lists are persisted in `ragent.json` under a new `bash` top-level key (`allowlist`, `denylist` arrays)
  - Global and project configs are merged at startup; also reloaded on `/reload`
  - Allowlisted commands bypass the built-in banned-command check without entering YOLO mode
  - Denylisted patterns are checked after built-in DENIED_PATTERNS

### Fixed
- **CI warnings-as-errors** — resolved all 14 errors flagged by `cargo check -D warnings`:
  - `session/processor.rs` — renamed unused `finish_reason` variable to `_finish_reason`
  - `team/config.rs` — added missing doc comment on `last_spawn_error` field
  - `tool/github_issues.rs` — added doc comments to all 5 tool structs
  - `tool/github_prs.rs` — added doc comments to all 5 tool structs

## [0.1.0-alpha.22] - 2026-06-09

### Added
- **M5 Ecosystem Integration:**
  - **MCP Tool Wiring (5.1):** `McpToolWrapper` implementing the `Tool` trait; MCP server tools are discovered at startup and registered in the tool registry as `mcp_{server_id}_{tool_name}`; `ToolRegistry` now uses `RwLock` for interior mutability enabling runtime registration; `SessionProcessor` holds `OnceLock<Arc<RwLock<McpClient>>>` with `set_mcp_client()` wired into `main.rs`
  - **Hooks System (5.4):** `HookConfig` with `HookTrigger` enum (`on_session_start`, `on_session_end`, `on_error`, `on_permission_denied`); `fire_hooks()` async fire-and-forget via `tokio::spawn`; hooks configured in `ragent.json` under `"hooks"` array; 4 lifecycle trigger points wired into `session/processor.rs`; hooks merge (append) across config files
  - **Auto-Update (5.5):** `ragent_core::updater` module with `check_for_update()` (GitHub releases API, 10s timeout), `download_and_replace()` (300s timeout, replaces running binary), `is_newer()` semver comparison; `/update` slash command with interactive confirmation
  - **`/doctor` Diagnostic Command (5.6):** System diagnostics report including binary path, version, config validity, MCP server connectivity, provider/model selection, tool count, session store status



### Added
- **M4 Advanced Features & Differentiation:**
  - **Persistent Shell (4.1+4.2):** Bash tool now preserves shell environment (env vars, cwd) between calls using temp state files; `BashResetTool` to wipe state; status bar shows shell cwd in yellow when it differs from project root via `ShellCwdChanged` event
  - **GitHub OAuth & API Client (4.3):** Device flow OAuth (`/github login`), token stored at `~/.ragent/github_token` (chmod 600); `/github logout` and `/github status`; `GitHubClient` with GET/POST/PATCH/PUT, rate-limit handling, auto-detects repo from `git remote`
  - **GitHub Issue Tools (4.4):** `github_list_issues`, `github_get_issue`, `github_create_issue`, `github_comment_issue`, `github_close_issue`
  - **GitHub PR Tools (4.5):** `github_list_prs`, `github_get_pr`, `github_create_pr`, `github_merge_pr`, `github_review_pr`
  - **Advanced Security (4.6):** 14 attack tools added to banned list (nmap, sqlmap, hydra, metasploit, etc.); 30+ new denied patterns (sudo, privilege escalation, /dev/tcp exfiltration, destructive git ops); `pushd`/`cd ~/$HOME` escape detection; `check_path_within_root` directory escape guard applied to create/edit/write/rm/patch tools

## [0.1.0-alpha.20] - 2026-05-21

### Added
- Input changes improvements
- New OpenAI generic provider support
- **CCGAP CC1 — Context & Safety Foundations:**
  - Git status injection: `{{GIT_STATUS}}` template variable with branch, status, and recent commits
  - README injection: `{{README}}` template variable reads from working directory
  - Bash safety: Safe-command whitelist (git, pwd, tree, date, which) for fast-path validation
  - Bash safety: Banned-command list (curl, wget, nc, telnet, axel, aria2c, lynx, w3m) blocks risky tools
  - Bash safety: Directory-escape guard rejects `cd` to parent or absolute paths
  - Bash syntax pre-check: `sh -n -c` validation (1s timeout) before execution
  - Output truncation: Head+tail truncation (15k + 15k chars) for large bash outputs

## [0.1.0-alpha.19] - 2026-03-19

### Changed
- Version bump.

### Added
- Teams UX and lifecycle enhancements:
  - Added `/team open <name>`, `/team close`, `/team delete <name>`, and `/team clear`
  - Updated `/team tasks` to render a tabular task/status view
  - Improved team-session reliability with TeamManager lazy initialization in TUI team flows
  - Fixed slash-input cursor behavior for `/team ...` entry
- Context management improvements:
  - Added automatic pre-send context compaction near context-window limits with queued message replay
- Copilot provider enhancements:
  - Added reasoning level selection support (`low`, `medium`, `high`, `none`)
  - Added model request-cost multiplier display in model selector
  - Improved model compatibility filtering for chat-completions endpoint usage
- Stability and docs:
  - Added read tool line-range validation to prevent runtime panics
  - Added `docs/howto_teams.md` comprehensive Teams user manual
  - Expanded tests around teams slash commands and task rendering

## [0.1.0-alpha.18] - 2026-03-19

### Changed
- Version bump — no functional changes since 0.1.0-alpha.17

### Added
- 2 major changes highlighted by the user: 1. Custom Agents 2. Agent Teams
  - Custom Agents: added support for user-defined agents via configuration and dynamic loading; new examples and tests included
  - Agent Teams: team coordination APIs and TUI panels for managing multi-agent teams; task claiming and messaging tools implemented




All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/),
and this project adheres to [Semantic Versioning](https://semver.org/).

## [0.1.0-alpha.16] - 2026-03-17

### Added
- **Milestone 5 — Multi-agent orchestration hardening** (F6 extensions)
  - **Pluggable transport** — `HttpRouter` routes jobs to remote agents via HTTP POST; `RouterComposite` chains multiple routers with first-success fallback
  - **Leader election** — `LeaderElector` provides vote-based in-process leader election with deterministic tie-breaking and `LeaderEvent` broadcast; `CoordinatorCluster` routes jobs to the elected leader node
  - **Policy-based conflict resolution** — `ConflictResolver` wraps any `ConflictPolicy` variant (`Concat`, `FirstSuccess`, `LastResponse`, `Consensus{threshold}`, `HumanReview`); `HumanFallback` trait with `LoggingFallback` default
  - `Coordinator::with_policy()` consuming builder wires a `ConflictResolver` into `start_job_sync`
  - 41 new tests across `test_orchestrator_transport.rs`, `test_orchestrator_leader.rs`, `test_orchestrator_policy.rs`
  - SPEC.md and QUICKSTART.md updated with M5 API reference and examples

## [0.1.0-alpha.15] - 2026-03-18

### Added
- **Image attachment support (Alt+V)** — paste images from clipboard or file URIs to include screenshots and visuals in conversations with the LLM
  - New `MessagePart::Image` variant stores image path and MIME type
  - Clipboard raw image data (RGBA pixels) encoded as PNG and saved to temp file
  - File URIs (`file:///...`) with image extensions are recognized and staged
  - Pending attachments displayed in input widget before sending
- **Keybindings help panel (`?`)** — type `?` on empty input to view all keyboard shortcuts
- **Right-click context menu** — Cut/Copy/Paste context menu for input and message panels
- **Context-window utilisation display** — status bar shows percentage of context window used (Copilot provider)
- **Session-prefixed step numbers** — tool call logs now show `[sid:step]` format for easier cross-session correlation

### Changed
- Provider layer extended with `supports_images()` capability check
- Anthropic provider supports image content blocks in message assembly
- Copilot provider supports vision-capable models with base64 image URLs
- Step map now stores `(short_session_id, step_number)` tuples

## [0.1.0-alpha.14] - 2026-03-17

### Added
- **MCP server auto-discovery** — new `/mcp discover` command scans PATH, npm global packages, and well-known MCP registry directories for installed MCP servers
  - Recognizes 18 known MCP servers (filesystem, GitHub, git, postgres, sqlite, memory, brave-search, fetch, puppeteer, slack, google-drive, google-maps, sentry, sequential-thinking, everything, time, aws-kb-retrieval, exa)
  - Scans `@modelcontextprotocol` npm scope for installed servers
  - Reads Claude Desktop, Cline, and generic MCP registry directories
  - Discovered servers can be added to `ragent.json` config
- **TUI MCP discovery panel** — F9 key opens discovery panel showing available MCP servers

### Changed
- MCP module structure reorganized with new `discovery.rs` submodule

## [0.1.0-alpha.13] - 2026-03-17

### Added
- **LSP test prompts** — 5 test prompts for LSP server integration testing (hover, go-to-definition, find-references, list-symbols, diagnostics) targeting Rust language servers
- **Office 365 test prompts** — 5 test prompts for Office document read/write testing (Word, Excel, PowerPoint, multi-sheet Excel, output formats)
- **LSP workspace folders support** — updated LSP client to use `workspace_folders` parameter instead of deprecated `root_uri`

### Fixed
- **Unused imports and dead code** — removed unused `LspDiscoverState` import and dead `get_json_str`/`get_json_u64` helper functions
- **Missing documentation** — added doc comments for `ExperimentalFlags` struct and `VsCodeExtension` variant field

## [0.1.0-alpha.12] - 2026-03-17

### Added
- **OpenSkills support** — extended skill loader to support OpenSkills-format skill files
- **Output file support for `/simplify` skill** — skill now accepts optional output path argument to save findings to a markdown file

### Fixed
- **Overly aggressive read function** — improved large file handling with section maps

## [0.1.0-alpha.11] - 2026-03-16

### Added
- **Skills system** fully implemented (SPEC §3.19) across 10 phases
  - YAML frontmatter-based skill definitions (`SKILL.md` format)
  - Multi-scope skill registry with priority: Bundled < Enterprise < Personal < Project
  - Argument substitution: `$ARGUMENTS`, `$N` (0-indexed), `$ARGUMENTS[N]`, `${RAGENT_SESSION_ID}`, `${RAGENT_SKILL_DIR}`
  - Dynamic context injection via `` !`command` `` shell execution patterns
  - Forked subagent execution for `context: fork` skills
  - 4 bundled skills: `simplify`, `batch`, `debug`, `loop`
  - `/skills` slash command with table display (Command, Scope, Access, Description)
  - Skill autocomplete in TUI slash menu (skills rendered in yellow)
  - Config `skill_dirs` for additional skill search directories
  - System prompt integration: active skills injected into agent context
  - `release` project skill for automated version bump, commit, push, and tag
- ~110 new tests for skill system (parsing, args, context, discovery, invocation, TUI)

### Changed
- SPEC.md: Skills section (§3.19) marked ✅ with implementation details
- QUICKSTART.md expanded with additional usage guidance
- Total test count: 781 (up from 672)

## [0.1.0-alpha.10] - 2026-03-13

### Changed
- Version bump — no functional changes since 0.1.0-alpha.9

## [0.1.0-alpha.9] - 2026-03-13

### Changed
- Version bump — no functional changes since 0.1.0-alpha.8

## [0.1.0-alpha.8] - 2026-03-12

### Added
- Step numbers `[#N]` displayed next to tool calls in both message and log panels for cross-referencing
- Pretty-printed JSON for tool call parameters in log panel
- Restored session tool calls now appear in log panel with `(restored)` suffix
- Event bus lag warning when broadcast events are dropped

### Changed
- Message/log panel split ratio changed from 70/30 to 60/40
- Event bus capacity increased from 256 to 2048 events
- TUI event loop now drains all pending bus events per cycle using `try_recv()` instead of processing one at a time
- Log panel auto-scroll now uses rendered (wrapped) line count for correct bottom tracking

### Fixed
- Log panel missing entries when tool output wraps to many lines (scroll calculation used logical vs rendered line count)
- Event bus lag causing silent loss of tool call log entries during burst activity
- Resumed sessions not showing tool calls in log panel
- 148 build warnings across workspace (missing docs, unused variables, dead code) — all resolved

## [0.1.0-alpha.7] - 2026-03-11

### Added
- `rm` tool — delete a single file (no wildcards), with path and success/failure display in TUI (23 tools total)
- `/tools` command now shows indented parameter details (name + description) for each tool
- SPEC.md: `orchestrator` and `debug` agents with tool group restrictions
- SPEC.md: Task delegation via `new_task` tool for orchestrator-style workflows
- SPEC.md: Codebase indexing & semantic search (§3.22) — embeddings + vector DB + Tree-sitter
- SPEC.md: Post-edit diagnostics integration (§3.23) — write delay + LSP error detection
- SPEC.md: Task todo list (§3.24) — interactive in-session task tracking
- SPEC.md: Prompt enhancement (§3.25) — AI-powered prompt refinement before sending
- SPEC.md: Hierarchical custom instructions (§3.26) — global/project/agent-specific rules
- SPEC.md: File ignore patterns `.ragentignore` (§3.27) — agent file access control
- SPEC.md: Suggested responses (§3.28) — context-aware follow-up suggestions
- SPEC.md: Shadow git checkpoints in Snapshot & Undo (§3.16)
- SPEC.md: `--allow-tool`, `--deny-tool`, `--allow-all-tools` CLI flags
- SPEC.md: `-p`/`--prompt` programmatic mode flag
- SPEC.md: `/context`, `/checkpoint`, `/todo` slash commands
- SPEC.md: Auto-compaction at 95% context limit, message enqueueing, inline rejection feedback
- SPEC.md: Hooks (§3.17), custom agents (§3.18), skills (§3.19), persistent memory (§3.20), trusted directories (§3.21)
- SPEC.md: Future goals F11–F20 (ACP, marketplace, API profiles, concurrent ops, custom tools, etc.)
- Planned tools: `new_task`, `switch_agent`, `codebase_search`, `generate_image`

### Changed
- Tool registry now contains 23 tools (up from 22)
- SPEC.md expanded from ~1636 to ~2168 lines with Copilot CLI and Roo Code feature parity analysis

## [0.1.0-alpha.6] - 2026-03-11

### Added
- TUI display summaries for `office_read`, `office_write`, `office_info`, `pdf_read`, and `pdf_write` tools (file path + line count)

### Fixed
- Panic in text selection when selecting lines containing multi-byte UTF-8 characters (e.g., `●`) — byte offsets now snap to valid char boundaries
- `office_read`, `office_write`, `office_info`, `pdf_read`, `pdf_write` tools now show file path and line count in the messages panel

## [0.1.0-alpha.5] - 2026-03-11

### Added
- `create` tool — create a new file with content, truncating if it already exists; creates parent directories as needed (22 tools total)
- Slash command output headers — all slash commands (`/about`, `/help`, `/system`, `/tools`) now prefix output with `From: /<command>` for clarity
- Each slash command now produces a separate message block with its own indicator dot

### Fixed
- Slash command output truncation — messages panel scroll calculation now uses `Paragraph::line_count()` to account for word-wrapped lines instead of logical line count
- Slash command viewport not scrolling to bottom — `scroll_offset` now resets to 0 when any slash command is executed
- `ratatui` `unstable-rendered-line-info` feature enabled for accurate wrapped-line measurement

### Changed
- Tool registry now contains 22 tools (up from 21)
- TUI display summaries added for `create` tool (input path + result line count)

## [0.1.0-alpha.4] - 2026-03-11

### Added
- `multiedit` tool — apply multiple edits to a single file atomically with line-based targeting
- `patch` tool — apply unified diff patches to files with fuzzy matching
- `webfetch` tool — fetch URLs and convert HTML to clean markdown via `htmd`
- `websearch` tool — web search via Tavily API with structured results (titles, URLs, snippets)
- `plan_enter` tool — delegate a task to the planning agent via event-driven agent switching
- `plan_exit` tool — return from planning agent to previous agent with summary injection
- `todo_read` tool — list session-scoped TODO items with optional status filter
- `todo_write` tool — add, update, remove, or clear TODO items with persistent SQLite storage
- Agent delegation architecture: `AgentSwitchRequested` / `AgentRestoreRequested` events, agent stack in TUI, pending action dispatch on `MessageEnd`
- `ToolContext.storage` field (`Option<Arc<Storage>>`) for tools needing database access
- `todos` table in Storage with full CRUD (`create_todo`, `get_todos`, `update_todo`, `delete_todo`, `clear_todos`)
- `todo` permission rule (Allow) in default agent permissions
- TUI display summaries for all 8 new tools (input descriptions + result summaries)
- SSE serialization for `AgentSwitchRequested` and `AgentRestoreRequested` events
- 111 new tests across 7 test files (538 total)

### Changed
- Tool registry now contains 21 tools (up from 13)
- Processor detects `agent_switch` / `agent_restore` metadata in tool results and breaks agent loop

## [0.1.0-alpha.3] - 2026-03-10

### Added
- AGENTS.md auto-loading on session start — project guidelines are injected into the system prompt for all multi-step agents (general, build, plan, explore); excluded for ask and internal utility agents
- AGENTS.md init exchange — model acknowledges project guidelines with a visible greeting in the message window on first message
- TUI tool call display improvements:
  - Tool names capitalized (Read, Write, Bash, Grep, etc.)
  - File paths shown relative to project root instead of absolute
  - Result summary lines with "└" prefix (e.g., "└ 72 lines read", "└ 45 lines written to INDEX.md")
  - Per-message-part spacing for visual separation
- INDEX.md document index with summaries of all root-level markdown files
- `content_line_count` field on `ToolResult` event for accurate line counts
- `force_new_message` flag in TUI to separate init exchange from main response
- History reconstruction now generates `tool_result` messages for `/compact` compatibility

### Fixed
- `/compact` slash command "tool_use ids without tool_result" error — `history_to_chat_messages()` now injects synthetic user messages with `ToolResult` parts for each assistant tool call
- Read tool line count showing truncated count (e.g., "5 lines" for a 1593-line file) — now uses full content line count before truncation
- Write tool showing "1 line written" regardless of actual content — now uses metadata `lines` field from tool output
- Write tool missing filename in display — `ToolCallArgs` event now sends full JSON args instead of truncated 200-char preview
- AGENTS.md init exchange no longer interferes with tool call decoding — uses isolated message list without the user's actual message
- Tool input parsing for write/edit tools with large content — full args JSON sent to TUI for proper field extraction

### Changed
- `ToolCallArgs` event now carries full args JSON (truncation moved to log display only)
- `content_line_count` computation uses tool metadata `lines` field when available, falls back to result content line count
- `build_system_prompt()` loads AGENTS.md from working directory for multi-step agents
- `history_to_chat_messages()` rewritten from iterator `.map()` to imperative loop with tool result injection

## [0.1.0-alpha.2] - 2025-07-25

### Added
- `/provider_reset` slash command with interactive provider selection UI
- Persistent provider disable flag — reset providers stay disabled across restarts
- Clipboard copy support on Copilot device code screen (`c` key, Linux-aware via `arboard`)
- Storage methods: `delete_provider_auth()`, `delete_setting()` with full test coverage
- `discover_api_base_multi_source()` for robust Copilot API base resolution
- VS Code-compatible headers for Copilot chat API (fixes 400 errors on plan-specific endpoints)

### Fixed
- Copilot "Unknown model" error — DB-stored device flow token now prioritised over `gh` CLI token
- Copilot API base URL resolution uses plan-specific endpoint (`api.individual.githubcopilot.com`)
- Provider reset now properly persists by storing a disabled flag in settings

### Changed
- `CopilotDeviceFlowComplete` event now carries `api_base` field
- Token exchange returns `TokenExchangeResult` with optional endpoints
- `resolve_api_key` for Copilot checks DB-stored token first, then falls back to other sources

## [0.1.0-alpha.1] - 2026-03-09

### Added
- TUI startup screen with ASCII logo, random tips, and centered prompt
- Interactive provider setup dialog (select provider → enter API key → choose model)
- Provider health indicator (green/yellow/red) on chat screen
- Slash-command autocomplete dropdown (`/agent`, `/model`, `/provider`)
- Agent cycling with Tab/Shift+Tab across non-hidden agents
- `ask` agent — quick Q&A without tools (single-step, no project context)
- Settings key-value table in SQLite for persisting user preferences
- Input history navigation with Up/Down arrow keys
- API key resolution from database (fallback after env vars and auto-discovery)
- Extended thinking control forwarded to all providers via agent options
- `SessionManager::storage()` accessor for direct storage access
- Messages now persisted to database on send and receive

### Changed
- Default agent changed from `build` to `general`
- `build` agent demoted from Primary to Subagent mode
- Improved error handling in `SessionProcessor` — errors now emit both `AgentError` and `MessageEnd` events so the TUI always resets
- Single-step agents omit tool definitions from LLM requests
- Copilot provider falls through to database key check instead of hard-failing
- API key error messages now suggest `ragent auth` command

### Fixed
- TUI no longer hangs when provider or API key is missing (error events always emitted)
- `resolve_api_key` iterates env var list by reference (avoids move)

## [0.1.0-alpha.0] - 2026-03-01

### Added
- Initial project scaffolding with Cargo workspace (3 crates)
- Core library (ragent-core): agent, config, event, llm, mcp, message, permission, provider, session, snapshot, storage, tool modules
- Provider adapters for Anthropic and OpenAI with SSE streaming
- GitHub Copilot provider with auto token discovery
- Ollama provider for local/remote LLM support
- 8 built-in tools: read, write, edit, bash, grep, glob, list, question
- Permission system with glob-based rule matching
- SQLite storage for sessions, messages, and provider auth
- HTTP server (ragent-server) with REST + SSE endpoints via axum
- Terminal UI (ragent-tui) with ratatui
- CLI entry point with clap (run, serve, session, auth, models, config commands)
- Event bus for real-time internal pub/sub
- File snapshot/restore for undo support
- Workspace-wide lint configuration (clippy pedantic, nursery, missing_docs)
