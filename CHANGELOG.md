# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/),
and this project adheres to [Semantic Versioning](https://semver.org/).

## [0.1.0-alpha.2] - 2025-07-25

### Added
- `/provider_reset` slash command with interactive provider selection UI
- Persistent provider disable flag — reset providers stay disabled across restarts
- Clipboard copy support on Copilot device code screen (`c` key, Linux-aware via `arboard`)
- Storage methods: `delete_provider_auth()`, `delete_setting()` with full test coverage
- `discover_api_base_multi_source()` for robust Copilot API base resolution
- VS Code-compatible headers for Copilot chat API (fixes 400 errors on plan-specific endpoints)

### Fixed
- Copilot "Unknown model" error — DB-stored device flow token now prioritised over `gh` CLI token
- Copilot API base URL resolution uses plan-specific endpoint (`api.individual.githubcopilot.com`)
- Provider reset now properly persists by storing a disabled flag in settings

### Changed
- `CopilotDeviceFlowComplete` event now carries `api_base` field
- Token exchange returns `TokenExchangeResult` with optional endpoints
- `resolve_api_key` for Copilot checks DB-stored token first, then falls back to other sources

## [0.1.0-alpha.1] - 2026-03-09

### Added
- TUI home screen with ASCII logo, random tips, and centered prompt
- Interactive provider setup dialog (select provider → enter API key → choose model)
- Provider health indicator (green/yellow/red) on home and chat screens
- Slash-command autocomplete dropdown (`/agent`, `/model`, `/provider`)
- Agent cycling with Tab/Shift+Tab across non-hidden agents
- `ask` agent — quick Q&A without tools (single-step, no project context)
- Settings key-value table in SQLite for persisting user preferences
- Input history navigation with Up/Down arrow keys
- API key resolution from database (fallback after env vars and auto-discovery)
- Extended thinking control forwarded to all providers via agent options
- `SessionManager::storage()` accessor for direct storage access
- Messages now persisted to database on send and receive

### Changed
- Default agent changed from `build` to `general`
- `build` agent demoted from Primary to Subagent mode
- Improved error handling in `SessionProcessor` — errors now emit both `AgentError` and `MessageEnd` events so the TUI always resets
- Single-step agents omit tool definitions from LLM requests
- Copilot provider falls through to database key check instead of hard-failing
- API key error messages now suggest `ragent auth` command

### Fixed
- TUI no longer hangs when provider or API key is missing (error events always emitted)
- `resolve_api_key` iterates env var list by reference (avoids move)

## [0.1.0-alpha.0] - 2026-03-01

### Added
- Initial project scaffolding with Cargo workspace (3 crates)
- Core library (ragent-core): agent, config, event, llm, mcp, message, permission, provider, session, snapshot, storage, tool modules
- Provider adapters for Anthropic and OpenAI with SSE streaming
- GitHub Copilot provider with auto token discovery
- Ollama provider for local/remote LLM support
- 8 built-in tools: read, write, edit, bash, grep, glob, list, question
- Permission system with glob-based rule matching
- SQLite storage for sessions, messages, and provider auth
- HTTP server (ragent-server) with REST + SSE endpoints via axum
- Terminal UI (ragent-tui) with ratatui
- CLI entry point with clap (run, serve, session, auth, models, config commands)
- Event bus for real-time internal pub/sub
- File snapshot/restore for undo support
- Workspace-wide lint configuration (clippy pedantic, nursery, missing_docs)
