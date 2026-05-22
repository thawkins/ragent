# Release

## Current Version: 0.1.0-alpha.89

### Changed
- **README.md rebuilt** — Rewrote from scratch to reflect the current specification. Expanded feature list to ~111 tools across 15 categories, corrected provider list (10 providers), added missing systems (memory, spec management, skills, teams/swarm, autopilot, MCP client, config error reporting), updated architecture table with all 15 crates, and refreshed project status.
- **STATS.md updated** — Complete rewrite showing project-wide metrics (175,840 lines, 468 files, 1,670 tests) and a per-crate breakdown with file counts, line counts, test files, descriptions, ASCII bar chart, and architecture ratios.
- **SPEC.md cover page added** — Inserted a styled HTML cover page with title, author, version, date, and repository link at the top of the specification.

## Previous: 0.1.0-alpha.88

### Fixed
- **Context compaction bug** — Fixed `compact_history_with_atomic_tool_calls` which was breaking the compaction loop prematurely when the oldest message was part of a tool call pair, preventing any trimming. Now uses prefix sums and a proper scan to find the correct truncation point while preserving atomic tool call pairs.

### Changed
- **SPEC.md audit and corrections** — Comprehensive review of SPEC.md against actual codebase implementation. Corrected tool counts (File Ops 14, Execution 3, Memory 8, Team 20), removed non-existent tools (`execute_python`, `file_ops_tool`, dead aliases), fixed version string to `alpha.88`, added alpha.87/alpha.88 to Appendix A, replaced MCP stub (§19) and Auto-Update stub (§20) with real documentation, expanded §5.2 config schema to include `tool_visibility`, `dirs`, `bash`, `gitlab`, `internal_llm`, `stream`, added missing slash commands (`/config show`, `/dirs`, `/profile`, `/theme`, `/status`, `/mouse`) to §6.2, and expanded §5.3 environment variable table with all provider-specific keys.

## Previous: 0.1.0-alpha.87

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
