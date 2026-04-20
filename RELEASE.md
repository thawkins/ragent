# Release

## Current Version: 0.1.0-alpha.46

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

## Previous: 0.1.0-alpha.45

### Changed
- Updated workspace version to 0.1.0-alpha.45
