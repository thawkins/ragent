# Release

## Current Version: 0.1.0-alpha.36

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
- **LSP discover** — deduplicates across all VS Code extension directories; shows version column; scrollable with ↑/↓/PgUp/PgDn
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
  - Added `.cargo/audit.toml` for transitive advisory ignores
- **Tool hallucination** — system prompt now includes exact list of available tool names to prevent models hallucinating tool names like `search` instead of `grep`

### Added
- **`/bash` slash command** — manage dynamic bash allowlist/denylist persisted in `ragent.json`:
  - `add allow <cmd>` / `add deny <pattern>` / `remove allow|deny <entry>` / `show` / `help`
  - `--global` flag writes to `~/.config/ragent/ragent.json`
  - Allowlist exempts banned commands without YOLO mode; denylist adds extra rejection patterns
  - Lists loaded at startup and on `/reload`

### Fixed
- CI `cargo check -D warnings` failures: unused `finish_reason` variable, missing doc comments on GitHub tool structs and team config field


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

### Carried from 0.1.0-alpha.19
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
