# Release

## Current Version: 0.1.0-alpha.102

### Added
- **Windows shell support for BashTool** — The `BashTool` now runs on Windows with automatic shell discovery (Git Bash preferred, PowerShell fallback). All 7 security layers remain active regardless of platform. Windows-specific directory-escape detection blocks `C:\`, `D:\`, and `\` paths. PowerShell syntax validation is skipped (PowerShell self-validates at runtime). State files are stored in `%LOCALAPPDATA%\ragent\shell\` on Windows.

### Changed
- **Refactored `is_directory_escape_attempt`** — Split into `is_directory_escape_attempt` (public, calls inner) and `is_directory_escape_attempt_inner` (testable inner function with explicit `on_windows` parameter). This enables testing Windows-specific path detection on any platform.
- **Shell discovery caching** — Added `OnceLock`-based process-global shell type cache so shell discovery only runs once per process.

### Fixed
- **Directory escape test** — `test_directory_escape_absolute` now uses `tempfile::tempdir()` for a real filesystem path, avoiding `canonicalize()` hangs on nonexistent paths.

## Previous: 0.1.0-alpha.101

### Fixed
- fixed AGENTS.md load path