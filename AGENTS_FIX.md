# AGENTS_FIX.md — Missing & Incomplete Code Tasks

This document is a comprehensive list of all agent implementations that have missing, incomplete, or stub code that needs to be completed. Tasks are organized by priority and blocked dependencies.

---

## Summary

| Status | Count |
|--------|-------|
| **Critical** (Priority 0) | 3 |
| **High** (Priority 1) | 3 |
| **Medium** (Priority 2) | 2 |
| **Total Incomplete** | **8** |

---

## Task Breakdown

### Critical Priority Tasks (P0)

#### TASK-001: Implement MCP Client — `connect()` method
- **File**: `crates/ragent-core/src/mcp/mod.rs` — line 56
- **Crate**: ragent-core
- **Signature**: `pub async fn connect(&mut self, id: &str, config: McpServerConfig) -> anyhow::Result<()>`
- **Current State**: Stub — registers server without actual connection
- **Required Behavior**:
  - Open stdio or HTTP transport to MCP server
  - Perform `initialize` handshake per MCP spec
  - Discover available tools from server
  - Populate `server.tools` vector with tool definitions
  - Handle transport-specific errors (connection refused, timeout, etc.)
- **Testing**: Mock MCP server with stdio transport; verify tools are discovered
- **Blocks**: TASK-002, TASK-003
- **Depends On**: —

#### TASK-002: Implement MCP Client — `list_tools()` method
- **File**: `crates/ragent-core/src/mcp/mod.rs` — line 69
- **Crate**: ragent-core
- **Signature**: `pub fn list_tools(&self) -> Vec<McpToolDef>`
- **Current State**: Stub — returns always-empty list (tools never populated by `connect()`)
- **Required Behavior**:
  - Query each connected server for current tool manifest
  - Aggregate results from all servers
  - Implement optional caching/refresh logic
  - Return consolidated list of all available tools
- **Testing**: After TASK-001 completes, mock multi-server setup
- **Blocks**: Session processor tool resolution
- **Depends On**: TASK-001

#### TASK-003: Implement MCP Client — `call_tool()` method
- **File**: `crates/ragent-core/src/mcp/mod.rs` — line 77
- **Crate**: ragent-core
- **Signature**: `pub async fn call_tool(&self, _server_id: &str, _tool_name: &str, _input: Value) -> anyhow::Result<Value>`
- **Current State**: Stub — logs warning and returns empty JSON object
- **Required Behavior**:
  - Route tool invocation to correct MCP server
  - Serialize input payload per tool schema
  - Send request and await response
  - Handle timeouts, protocol errors, and invalid responses
  - Return server's result as JSON value
- **Testing**: Mock MCP server returning test payloads; verify roundtrip
- **Blocks**: Session processor tool execution
- **Depends On**: TASK-001

---

### High Priority Tasks (P1)

#### TASK-004: Implement HTTP Server — `abort_session()` endpoint
- **File**: `crates/ragent-server/src/routes/mod.rs` — around line 245
- **Crate**: ragent-server
- **Signature**: `async fn abort_session(Path(_id): Path<String>) -> impl IntoResponse`
- **Current State**: Stub — ignores session ID, returns `{"ok": true}` unconditionally
- **Required Behavior**:
  - Extract session ID from path parameter (currently ignored)
  - Cancel any in-progress message processing via cancellation token or semaphore
  - Mark session as aborted in storage via `storage.archive_session(&id)`
  - Publish `Event::SessionAborted { session_id, reason }` on event bus
  - Clean up any associated resources (temp files, tool contexts, etc.)
  - Return success or error JSON response with proper status code
- **Testing**: Create session, send message, abort mid-processing, verify storage reflects abort
- **Blocks**: —
- **Depends On**: —

#### TASK-005: Implement TUI — agent switching
- **File**: `crates/ragent-tui/src/app.rs` — line ~65
- **Crate**: ragent-tui
- **Match Arm**: `InputAction::SwitchAgent`
- **Current State**: Empty block with comment `// Cycle through agents — placeholder`
- **Required Behavior**:
  - Maintain list of available agents from `ragent_core::agent::create_builtin_agents()`
  - Track current agent index in `App` struct
  - On `SwitchAgent` action, increment index (wrap at end)
  - Update `self.agent` to new `AgentInfo`
  - Display new agent name in status bar
  - Optionally show toast: "Switched to: {agent_name}"
- **Testing**: Verify cycling through all agents; wrap-around behavior
- **Blocks**: —
- **Depends On**: —

#### TASK-006: Implement TUI — slash command parsing & dispatch
- **File**: `crates/ragent-tui/src/app.rs` — line ~70
- **Crate**: ragent-tui
- **Match Arm**: `InputAction::SlashCommand(_cmd)`
- **Current State**: Empty block with comment `// Handle slash commands — placeholder`
- **Required Behavior**:
  - Parse user input starting with `/` as slash commands
  - Support commands:
    - `/clear` — clear message history for session
    - `/compact` — trigger session compaction via summary agent
    - `/model <provider>/<name>` — switch LLM model
    - `/agent <name>` — switch active agent
    - `/system <prompt>` — override system prompt
    - `/quit` — exit TUI gracefully
    - `/help` — show command help
  - Update app state based on command (agent, model, system prompt)
  - Provide user feedback via log entry or toast
  - Handle invalid commands with error message
- **Testing**: Execute each command; verify state changes; invalid command handling
- **Blocks**: —
- **Depends On**: —

---

### Medium Priority Tasks (P2)

#### TASK-007: Implement CLI — session resume
- **File**: `src/main.rs` — around line 267
- **Crate**: ragent (binary root)
- **Match Arm**: `SessionCommands::Resume { id }`
- **Current State**: Logs message, then does nothing; has `// TODO: implement resume with TUI` comment
- **Required Behavior**:
  - Load session from storage via `session_manager.get_session(&id)` or similar
  - Load message history via `storage.get_messages(&id)`
  - Initialize TUI with existing conversation context
  - Restore scroll position, agent, and model from session metadata
  - Resume interactive TUI mode so user can continue the session
- **Testing**: Create session, send messages, export, close; then resume and verify history
- **Blocks**: —
- **Depends On**: —

#### TASK-008: Implement CLI — session import persistence
- **File**: `src/main.rs` — around line 276
- **Crate**: ragent (binary root)
- **Match Arm**: `SessionCommands::Import { file }`
- **Current State**: Reads JSON, deserializes into `_messages` (unused), prints confirmation, but never saves to DB
- **Required Behavior**:
  - Read and deserialize messages from file
  - Create new session in storage via `session_manager.create_session(&dir)`
  - Iterate over messages and insert each via `storage.insert_message(&session_id, &message)`
  - Report number of messages imported to user
  - Handle file not found, invalid JSON, and other errors gracefully
- **Testing**: Export session → import → resume and verify messages are restored
- **Blocks**: —
- **Depends On**: —

---

## Dependency Graph

```
TASK-001 (MCP::connect)
  ├─→ TASK-002 (MCP::list_tools)
  └─→ TASK-003 (MCP::call_tool)

TASK-004 (abort_session)     — independent
TASK-005 (agent switching)   — independent
TASK-006 (slash commands)    — independent
TASK-007 (session resume)    — independent
TASK-008 (session import)    — independent
```

---

## Implementation Order

**Recommended priority for completion:**

1. **TASK-004** — `abort_session()` (independent, straightforward, high-impact for robustness)
2. **TASK-001** — `McpClient::connect()` (unblocks downstream MCP tasks)
3. **TASK-002** — `McpClient::list_tools()` (depends on TASK-001)
4. **TASK-003** — `McpClient::call_tool()` (depends on TASK-001)
5. **TASK-007** — Session resume (independent, medium complexity)
6. **TASK-008** — Session import (independent, medium complexity)
7. **TASK-005** — Agent switching (independent, low complexity)
8. **TASK-006** — Slash commands (independent, high complexity)

---

## Testing Strategy

For each task, ensure:

1. **Unit tests** in `crates/<crate>/tests/` with mocks where applicable
2. **Integration tests** that verify the feature works end-to-end
3. **Regression tests** to ensure changes don't break existing functionality
4. Run `cargo test --workspace` to verify all tests pass
5. Run `cargo clippy` and `cargo fmt` to ensure code quality

### MCP Testing

Since MCP requires a server for testing, consider:
- Using a mock in-memory MCP server for unit tests
- Creating a test fixture with a simple stdio-based server
- Mocking `McpServer::connect()` in session processor tests

### HTTP Endpoint Testing

For `abort_session()`:
- Mock the storage and event bus
- Create a test server with `axum::test` utilities
- Verify response codes and JSON structure

---

## Code Quality Checklist

Before marking a task complete, verify:

- [ ] Code follows AGENTS.md style guide (4 spaces, 100-char max, snake_case, etc.)
- [ ] All public types have `///` doc comments with parameters and return values
- [ ] All modules have `//!` doc comments
- [ ] No `println!` / `eprintln!` — use `tracing` instead
- [ ] No `unwrap()` on fallible operations — use `?` or proper error handling
- [ ] Error messages are clear and actionable
- [ ] Tests are in `crates/<crate>/tests/` (not inline in source)
- [ ] Tests follow naming convention: `test_<component>_<scenario>`
- [ ] `cargo clippy --workspace` has no new warnings
- [ ] `cargo fmt --check` passes

---

## Notes for Implementation

### MCP Tasks (TASK-001, TASK-002, TASK-003)

The Model Context Protocol is a standard for extending LLM capabilities via external tools. Implement support for:

- **Stdio transport**: Most common; spawn subprocess and communicate via stdin/stdout
- **HTTP transport**: Optional; for remote servers
- **Tool discovery**: Parse tool manifests in `initialize` response
- **Error handling**: Network errors, timeouts, invalid responses should be logged and recovered gracefully

Reference implementation: [MCP Specification](https://spec.modelcontextprotocol.io/)

### Session Management (TASK-007, TASK-008)

Sessions encapsulate a conversation context (working directory, message history, agent, model). Ensure:

- Session data is persisted in SQLite via `Storage`
- Session state is preserved across restarts
- Session metadata (title, created_at, updated_at) is maintained

### TUI Features (TASK-005, TASK-006)

The TUI uses `ratatui` for rendering. When implementing:

- Use `LogEntry` for user feedback
- Update `App::agent` or `App::model` when switching
- Validate input before dispatching commands
- Show errors clearly in the message area

---

## Related Documentation

- **AGENTS.md**: Agent requirements and guidelines
- **SPEC.md**: System architecture and design
- **docs/TODO.md**: Additional unimplemented functions (related but different scope)
- **docs/CODE_CLEANUP.md**: Code quality improvements (separate from missing features)

---

## Revision History

| Date | Author | Change |
|------|--------|--------|
| Initial | Agent | Created task list from code scan |

