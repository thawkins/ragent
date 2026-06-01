# Release

## Current Version: 0.1.0-alpha.103

### Fixed
- **Bash syntax validation on Windows** — `validate_bash_syntax()` now uses the discovered shell program (`bash` on Unix, Git Bash executable path on Windows) instead of a hardcoded `sh -n -c` that fails on Windows. PowerShell syntax validation is still skipped (PowerShell self-validates at runtime).

## Previous: 0.1.0-alpha.102

### Added
- **Windows shell support for BashTool** — The `BashTool` now runs on Windows with automatic shell discovery (Git Bash preferred, PowerShell fallback). All 7 security layers remain active regardless of platform.

### Changed
- **Refactored `is_directory_escape_attempt`** — Split into testable inner function with explicit `on_windows` parameter.
- **Shell discovery caching** — Added `OnceLock`-based process-global shell type cache.

### Fixed
- **Directory escape test** — Uses `tempfile::tempdir()` for real filesystem paths.