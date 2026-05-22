# Release

## Current Version: 0.1.0-alpha.87

### Fixed
- **Read tool instructions** — Clarified in AGENTS.md that `end_line` is an absolute line number, not a count or offset.
- **Remote push instructions** — Strengthened AGENTS.md guidelines to explicitly prohibit pushing without explicit user instruction.

### Changed
- **SPEC.md reorganization** — Reorganized sections into logical order (1-20), fixed numbering, added Blueprints subsection (14.7), and merged GitHub/GitLab into a single peer section (18).

## Previous: 0.1.0-alpha.86

### Added
- **Azure Resource (File) provider** — New `azure_resource` provider that reads Azure endpoint definitions from `azureresources.json` in `~/.config/ragent/` or `.ragent/`. Supports multiple resource entries with per-endpoint API keys, environment-variable-based keys, custom context windows, capability tags, and thinking configuration.
- **Azure Resource documentation** — Added `docs/userdocs/azure-resource.md` with full JSON schema, field reference, copy-pasteable example, and troubleshooting guide.
- **Azure Resource integration tests** — Added `crates/ragent-tui/tests/test_azure_resource_flow.rs` covering provider listing, persistence round-trip, stale selection cleanup, ModelInfo conversion, and backend resolution.
- **`azureresource.json` file format spec** — Added `specs/AzureResource/FILEFORMAT.md` documenting the complete `azureresources.json` schema, validation rules, and mapping to internal `ModelInfo`.

## Previous: 0.1.0-alpha.85

### Added
- **Azure test script** — Added `scripts/getresult.sh` for testing Azure AI Foundry chat completions.

## Previous: 0.1.0-alpha.83

### Fixed
- **SPEC.md fixes** — Fixed malformed benchmark runner table, corrected Team Lifecycle mermaid diagram syntax, replaced misplaced GitLab Integration section with proper content, and updated version references throughout.

## Previous: 0.1.0-alpha.82

### Added
- **azProvider fixes** — Applied fixes to Azure provider implementation.
- **`/config show`** — Added `/config show` slash command to display current configuration.

## Previous: 0.1.0-alpha.81

### Fixed
- **Azure endpoint logging** — TUI log panel now displays the full endpoint URL for Azure AI Foundry requests.

## Previous: 0.1.0-alpha.78

### Fixed
- **Azure endpoint logging** — TUI log panel now displays the full endpoint URL for Azure AI Foundry requests, not just the `[provider/model]` prefix.

## Previous: 0.1.0-alpha.77
