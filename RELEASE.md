# Release

## Current Version: 0.1.0-alpha.38

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
- **pre-flight.sh** — Local CI check script; supports `--quick` flag

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

### Changed
- Version bump to 0.1.0-alpha.28

## Previous: 0.1.0-alpha.27

### Fixed
- CI Clippy: redundant closures in `lsp/discovery.rs`

## Previous: 0.1.0-alpha.26

### Fixed
- **LSP discover** — deduplicates across all VS code extension directories; shows version column; scrollable with ↑/↓/PgUp/PgDn
- **LSP system prompt** — only injects guidance for actually-connected servers
- **CI** — bench and test unused-mut / missing-docs warnings resolved

## Previous: 0.1.0-alpha.25

### Fixed
- **CI lint fixes** — resolved 1339 Clippy warnings/errors across 127 files workspace-wide
- Rewrote workspace lint config with priority-aware lint groups and 50+ `allow` entries
- Fixed `prompt_opt::from_str` Clippy `should_implement_trait` by implementing `std::str::FromStr`

## Previous: 0.1.0-alpha.24

### Fixed
- **Security Audit CI** — all `cargo audit` and `cargo deny check` failures resolved:
  - Upgraded criterion 0.4→0.5 (removes `atty` vulnerability)
  - Updated `rustls-webpki` and `quinn-proto` to patched versions
  - Rewrote `deny.toml` for cargo-deny ≥0.19 schema