# Release

## Current Version: 0.1.0-alpha.48

### Changed
- **Permission workflow and config updates** — Refined permission handling, TUI countdown behavior, and config parsing diagnostics.
- **Codeindex permissions** — Hardwired codeindex tools to bypass prompts for read-only local analysis.
- **Workspace maintenance** — Added new crate-level tests and refreshed specification/supporting reports for the current release.

## Previous: 0.1.0-alpha.47

### Changed
- **Crate reorganization** — Extracted foundation crates from ragent-core (Milestones 1-3 complete)
  - **ragent-types** (2,733 lines) — Core types, traits, error handling, events, messages
  - **ragent-config** (1,854 lines) — Configuration loading, permission checking, runtime lists
  - **ragent-storage** (2,818 lines) — SQLite persistence for sessions, memories, journals, snapshots, teams
  - **ragent-llm** (6,736 lines) — LLM provider implementations for 8 providers
  - Reduced ragent-core from 64,909 → 50,800 lines (-21.8%)
  - Clean dependency chain: ragent-types → ragent-config/ragent-storage/ragent-llm → ragent-core
  - All tests passing, backward compatibility maintained via re-exports
