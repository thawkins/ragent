# TODO — Unimplemented & Stub Functions

> Auto-generated from codebase scan. Each task tracks a function or handler
> that is currently a stub, placeholder, or has an incomplete implementation.

---

## Summary

| Status | Count |
|--------|-------|
| Stub (no-op / fake response) | 4 |
| Placeholder (empty match arm) | 2 |
| TODO comment (partial impl) | 2 |
| **Total** | **8** |

---

## Tasks

### MCP Client — `crates/ragent-core/src/mcp/mod.rs`

The entire MCP (Model Context Protocol) module is stubbed out. All three
public methods return fake success without performing real work.

#### TODO-001: Implement `McpClient::connect()`
- **File:** `crates/ragent-core/src/mcp/mod.rs` — line 43
- **Signature:** `pub async fn connect(&mut self, id: &str, config: McpServerConfig) -> Result<()>`
- **Current behavior:** Pushes an empty server record, logs `"(stub)"`, returns `Ok(())`.
- **Required behavior:** Open a stdio or HTTP transport to the MCP server,
  perform the `initialize` handshake, discover available tools, and populate
  the server's `tools` vector.
- **Priority:** High
- **Blocked by:** —

#### TODO-002: Implement `McpClient::list_tools()`
- **File:** `crates/ragent-core/src/mcp/mod.rs` — line 56
- **Signature:** `pub fn list_tools(&self) -> Vec<McpToolDef>`
- **Current behavior:** Returns a flat list from `self.servers`, which is
  always empty because `connect()` never populates tools.
- **Required behavior:** Query each connected server for its current tool
  manifest and return the aggregated list. May also need caching/refresh logic.
- **Priority:** High
- **Blocked by:** TODO-001

#### TODO-003: Implement `McpClient::call_tool()`
- **File:** `crates/ragent-core/src/mcp/mod.rs` — line 64
- **Signature:** `pub async fn call_tool(&self, _server_id: &str, _tool_name: &str, _input: Value) -> Result<Value>`
- **Current behavior:** Logs a warning and returns an empty JSON object `{}`.
- **Required behavior:** Route the tool invocation to the correct MCP server,
  send the input payload, wait for the result, and return the server's
  response. Handle timeouts and errors.
- **Priority:** High
- **Blocked by:** TODO-001

---

### HTTP Server — `crates/ragent-server/src/routes/mod.rs`

#### TODO-004: Implement `abort_session()`
- **File:** `crates/ragent-server/src/routes/mod.rs` — line 248
- **Signature:** `async fn abort_session(Path(_id): Path<String>) -> impl IntoResponse`
- **Current behavior:** Ignores the session ID (prefixed `_id`), returns
  `{"ok": true}` unconditionally.
- **Required behavior:** Cancel any in-progress message processing for the
  session, mark the session as aborted in storage, publish a session-aborted
  event on the event bus, and clean up associated resources.
- **Priority:** High
- **Blocked by:** —

---

### TUI Application — `crates/ragent-tui/src/app.rs`

#### TODO-005: Implement agent switching
- **File:** `crates/ragent-tui/src/app.rs` — line 62
- **Match arm:** `InputAction::SwitchAgent`
- **Current behavior:** Empty block with comment `// Cycle through agents — placeholder`.
- **Required behavior:** Cycle through the list of available agents (coder,
  task, architect, ask, debug, code-review, custom), update `self.agent`,
  and display the new active agent in the status bar.
- **Priority:** Medium
- **Blocked by:** —

#### TODO-006: Implement slash command handling
- **File:** `crates/ragent-tui/src/app.rs` — line 65
- **Match arm:** `InputAction::SlashCommand(_cmd)`
- **Current behavior:** Empty block with comment `// Handle slash commands — placeholder`.
- **Required behavior:** Parse and dispatch slash commands such as `/clear`,
  `/compact`, `/model <name>`, `/agent <name>`, `/system <prompt>`,
  `/quit`, `/help`. Update app state accordingly and provide user feedback.
- **Priority:** Medium
- **Blocked by:** —

---

### CLI Binary — `src/main.rs`

#### TODO-007: Implement session resume
- **File:** `src/main.rs` — line 222
- **Match arm:** `SessionCommands::Resume { id }`
- **Current behavior:** Prints `"Resuming session {id}..."` then does nothing.
  Has `// TODO: implement resume with TUI` comment.
- **Required behavior:** Load the session and its message history from
  storage, initialize the TUI with the existing conversation context, and
  resume interactive mode so the user can continue the session.
- **Priority:** Medium
- **Blocked by:** —

#### TODO-008: Implement session import persistence
- **File:** `src/main.rs` — line 231
- **Match arm:** `SessionCommands::Import { file }`
- **Current behavior:** Reads the file, deserializes messages into
  `_messages` (unused variable), prints confirmation, but never persists
  the messages to the database. Has `// TODO: store imported messages` comment.
- **Required behavior:** Create a new session in storage, iterate over the
  deserialized messages, insert each into the database via
  `storage.insert_message()`, and report the number of messages imported.
- **Priority:** Medium
- **Blocked by:** —

---

## Dependency Graph

```
TODO-001  (McpClient::connect)
  └─► TODO-002  (McpClient::list_tools)
  └─► TODO-003  (McpClient::call_tool)

TODO-004  (abort_session)       — independent
TODO-005  (agent switching)     — independent
TODO-006  (slash commands)      — independent
TODO-007  (session resume)      — independent
TODO-008  (session import)      — independent
```

---

## Priority Order

1. **TODO-001** — MCP connect (unlocks TODO-002 and TODO-003)
2. **TODO-002** — MCP list_tools
3. **TODO-003** — MCP call_tool
4. **TODO-004** — Server abort_session (returns false success)
5. **TODO-005** — TUI agent switching
6. **TODO-006** — TUI slash commands
7. **TODO-007** — CLI session resume
8. **TODO-008** — CLI session import persistence
