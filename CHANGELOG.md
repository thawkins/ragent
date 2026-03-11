# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/),
and this project adheres to [Semantic Versioning](https://semver.org/).

## [0.1.0-alpha.5] - 2026-03-11

### Added
- `create` tool — create a new file with content, truncating if it already exists; creates parent directories as needed (22 tools total)
- Slash command output headers — all slash commands (`/about`, `/help`, `/system`, `/tools`) now prefix output with `From: /<command>` for clarity
- Each slash command now produces a separate message block with its own indicator dot

### Fixed
- Slash command output truncation — messages panel scroll calculation now uses `Paragraph::line_count()` to account for word-wrapped lines instead of logical line count
- Slash command viewport not scrolling to bottom — `scroll_offset` now resets to 0 when any slash command is executed
- `ratatui` `unstable-rendered-line-info` feature enabled for accurate wrapped-line measurement

### Changed
- Tool registry now contains 22 tools (up from 21)
- TUI display summaries added for `create` tool (input path + result line count)

## [0.1.0-alpha.4] - 2026-03-11

### Added
- `multiedit` tool — apply multiple edits to a single file atomically with line-based targeting
- `patch` tool — apply unified diff patches to files with fuzzy matching
- `webfetch` tool — fetch URLs and convert HTML to clean markdown via `htmd`
- `websearch` tool — web search via Tavily API with structured results (titles, URLs, snippets)
- `plan_enter` tool — delegate a task to the planning agent via event-driven agent switching
- `plan_exit` tool — return from planning agent to previous agent with summary injection
- `todo_read` tool — list session-scoped TODO items with optional status filter
- `todo_write` tool — add, update, remove, or clear TODO items with persistent SQLite storage
- Agent delegation architecture: `AgentSwitchRequested` / `AgentRestoreRequested` events, agent stack in TUI, pending action dispatch on `MessageEnd`
- `ToolContext.storage` field (`Option<Arc<Storage>>`) for tools needing database access
- `todos` table in Storage with full CRUD (`create_todo`, `get_todos`, `update_todo`, `delete_todo`, `clear_todos`)
- `todo` permission rule (Allow) in default agent permissions
- TUI display summaries for all 8 new tools (input descriptions + result summaries)
- SSE serialization for `AgentSwitchRequested` and `AgentRestoreRequested` events
- 111 new tests across 7 test files (538 total)

### Changed
- Tool registry now contains 21 tools (up from 13)
- Processor detects `agent_switch` / `agent_restore` metadata in tool results and breaks agent loop

## [0.1.0-alpha.3] - 2026-03-10

### Added
- AGENTS.md auto-loading on session start — project guidelines are injected into the system prompt for all multi-step agents (general, build, plan, explore); excluded for ask and internal utility agents
- AGENTS.md init exchange — model acknowledges project guidelines with a visible greeting in the message window on first message
- TUI tool call display improvements:
  - Tool names capitalized (Read, Write, Bash, Grep, etc.)
  - File paths shown relative to project root instead of absolute
  - Result summary lines with "└" prefix (e.g., "└ 72 lines read", "└ 45 lines written to INDEX.md")
  - Per-message-part spacing for visual separation
- INDEX.md document index with summaries of all root-level markdown files
- `content_line_count` field on `ToolResult` event for accurate line counts
- `force_new_message` flag in TUI to separate init exchange from main response
- History reconstruction now generates `tool_result` messages for `/compact` compatibility

### Fixed
- `/compact` slash command "tool_use ids without tool_result" error — `history_to_chat_messages()` now injects synthetic user messages with `ToolResult` parts for each assistant tool call
- Read tool line count showing truncated count (e.g., "5 lines" for a 1593-line file) — now uses full content line count before truncation
- Write tool showing "1 line written" regardless of actual content — now uses metadata `lines` field from tool output
- Write tool missing filename in display — `ToolCallArgs` event now sends full JSON args instead of truncated 200-char preview
- AGENTS.md init exchange no longer interferes with tool call decoding — uses isolated message list without the user's actual message
- Tool input parsing for write/edit tools with large content — full args JSON sent to TUI for proper field extraction

### Changed
- `ToolCallArgs` event now carries full args JSON (truncation moved to log display only)
- `content_line_count` computation uses tool metadata `lines` field when available, falls back to result content line count
- `build_system_prompt()` loads AGENTS.md from working directory for multi-step agents
- `history_to_chat_messages()` rewritten from iterator `.map()` to imperative loop with tool result injection

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
