# Release

## Current Version: 0.1.0-alpha.106

### Added
- **Microsoft Foundry Local provider integration** — Added first-class support for Microsoft Foundry Local as a local LLM provider, including provider setup dialog visibility, `[local]` badge rendering, status-bar abbreviation, health checks, and configuration option merging (`auto_start`, `device`, `models_path`).
- **Headroom compression lifecycle events** — New `Event::CompressionStarted` and `Event::CompressionFinished` events carry per-iteration compression statistics and are surfaced in the TUI status bar and SSE stream.

### Changed
- **Per-iteration compression visibility** — Automatic Headroom compression now publishes start/finish events, activates the status-bar "compressing" indicator, and refreshes the `ctx:` display immediately with the compressed token count.
- **Workspace version** — Bumped to `0.1.0-alpha.106`.

### Fixed
- **Context window display lag after compression** — The status-bar context percentage now updates as soon as compression finishes instead of waiting for the LLM response.

## Previous Release: 0.1.0-alpha.105
- **Bedrock provider** — Refinements to credential handling and SigV4 signing.
- **Multiple tool refinements** — Updated codeindex_search, list_tasks, memory_search, office_write, and spec_list tools.
- **Test improvements** — Updated multiple test files for compatibility with new APIs and module structure.

## Previous: 0.1.0-alpha.104