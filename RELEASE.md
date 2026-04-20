# Release

## Current Version: 0.1.0-alpha.45

### Changed

- Updated workspace version to 0.1.0-alpha.45
## Previous: 0.1.0-alpha.44

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
      - See individual phase completion reports for detailed documentation

### Changed
- Updated workspace version to 0.1.0-alpha.44

## Previous: 0.1.0-alpha.43

### Added
- **AIWiki Milestone 1-6: Complete Knowledge Base System** — An embedded, project-scoped knowledge base for ragent
    - New `aiwiki` crate with full knowledge base infrastructure
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

## Previous: 0.1.0-alpha.42
### Added
- **Gemini provider** — Google Gemini API support as a first-class LLM provider
- **Hugging Face provider** — Hugging Face Inference API support as a first-class LLM provider
- **Unfinished goal detection** — Session processor now detects when the agent's stated goals have not been fully achieved

### Fixed
- Various fixes across provider implementations and session processing

## Previous: 0.1.0-alpha.40

### Changed
- **Model picker now displays models in a tabular format with metadata** — The model selection dialog now shows a table with columns for Model name, Context window size, Cost (input/output per million tokens), and Features (Reasoning, Vision, Tool-use). Models are now sorted alphabetically by name for all providers.

### Added
- **GitLab integration** — Full GitLab REST API support mirroring GitHub toolset (issues, MRs, CI/CD pipelines, jobs)
- **Code index parsers for CMake, Gradle, Maven, OpenSCAD, and Terraform** — 13 total languages now supported
- **Extensive SPEC.md rewrite** — Comprehensive rewrite of project specification document

## Previous: 0.1.0-alpha.39

### Fixed
- **MS Office and LibreOffice presentation writer** — Fixed PPTX and ODP slide rendering: body text now produces proper layout, geometry, and paragraph elements
- **todo_write tool** — Updated result summary output for improved clarity

### Changed
- Version bump to 0.1.0-alpha.38

## Previous: 0.1.0-alpha.37

### Added
- **Code index multi-language support** — 7 languages: Rust, Python, TypeScript/JavaScript (TS/TSX/JS/JSX), Go, C/C++, Java via tree-sitter parsers
- **Code index benchmarks** — Criterion-based performance benchmarks for parse, store, search, and full index operations
- **Code index config persistence** — `code_index` section in `ragent.json` for enabled state, file size limits, and exclude patterns

### Fixed
- Fixed `test_registry_total_tool_count` assertion (105 tools)

### Changed
- Version bump to 0.1.0-alpha.37

## Previous: 0.1.0-alpha.36

### Fixed
- Comprehensive test & lint cleanup across the entire workspace (43 files, 1,709 tests all passing)
- Added `#[serial]` to bash tool tests to eliminate flaky process permit contention
- Fixed webfetch test metadata field names, slash command keystroke test, push_log doctest
- Resolved all clippy warnings (single_char_pattern, needless_collect, unchecked_time_subtraction, etc.)

### Changed
- Updated dependencies: clap 4.6, tokio 1.51, tracing-subscriber 0.3.23, uuid 1.23
- Version bump to 0.1.0-alpha.36

## Previous: 0.1.0-alpha.35

### Added
- **pre-flight.sh** �� Local CI check script; supports `--quick` flag

### Fixed
- Fixed lint errors in test files (unused variables, imports, mut warnings, missing docs)

## Previous: 0.1.0-alpha.34

### Fixed
- **Security: RUSTSEC-2026-0097** — Updated `rand` to 0.9.3 (0.9.2 was still affected)

### Changed
- Version bump to 0.1.0-alpha.34

## Previous: 0.1.0-alpha.33

### Fixed
- **Security: RUSTSEC-2026-0097** — Upgraded `rand` from 0.8 to 0.9 to fix unsound advisory
- **Ollama Cloud context window** — Now fetches actual context window size via `/api/show` API endpoint; also detects vision capability

### Changed
- Updated `rand` API calls for 0.9 compatibility
- Version bump to 0.1.0-alpha.33

## Previous: 0.1.0-alpha.31

### Added
- **SECPLAN.md** — Comprehensive security remediation plan consolidating findings from 5 existing security audit documents covering Critical (P0), High (P1), Medium (P2), and Low (P3) issues

### Changed
- Version bump to 0.1.0-alpha.31

## Previous: 0.1.0-alpha.30

### Added
- **Alt+L keybinding** — Toggle the log panel visibility on/off (previously only available via `/log` command)

### Changed
- Version bump to 0.1.0-alpha.30

## Previous: 0.1.0-alpha.29
