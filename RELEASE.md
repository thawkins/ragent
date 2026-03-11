# Release

## Current Version: 0.1.0-alpha.4

### Added (since 0.1.0-alpha.3)
- 8 new tools: `multiedit`, `patch`, `webfetch`, `websearch`, `plan_enter`, `plan_exit`, `todo_read`, `todo_write` (21 tools total)
- Agent delegation system — `plan_enter`/`plan_exit` tools enable switching to a planning agent and back, with agent stack management
- Web tools — `webfetch` fetches URLs and converts HTML to markdown; `websearch` queries Tavily API
- TODO persistence — `todo_read`/`todo_write` tools with SQLite-backed session-scoped TODOs
- `multiedit` tool for applying multiple edits to a single file atomically
- `patch` tool for applying unified diff patches to files
- `Storage` layer extended with `todos` table, full CRUD, and `clear_todos()`
- `ToolContext` now carries optional `Storage` handle for tools that need database access
- Event variants: `AgentSwitchRequested`, `AgentRestoreRequested` for agent delegation
- TUI: agent stack with push/pop, pending plan task/restore handling on `MessageEnd`
- TUI display summaries for all 8 new tools (input + result lines)
- SSE serialization for new event variants
- `todo` permission rule (Allow) in default agent permissions

### Fixed (since 0.1.0-alpha.3)
- Processor breaks agent loop on `agent_switch` or `agent_restore` metadata to support delegation
- `event_matches_session()` updated for exhaustive matching of new event variants
