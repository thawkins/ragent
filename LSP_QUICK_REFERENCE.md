# Ragent LSP Integration - Quick Reference

## Architecture at a Glance

```
Input Flow:
  User Message → Slash Command / Regular Input
    ↓
  Session Processor (processor.rs line 77)
    ↓
  System Prompt Injected (agent/mod.rs line 438)
    ↓
  LLM Request with Available Tools
    ↓
  Tool Execution (tool.rs: Tool trait)
    ↓
  Event Bus Publishing (event.rs)
    ↓
  TUI Update / Message Storage
```

## 3 Crates
1. **ragent-core** - All business logic (agents, tools, LLM, session, permission, storage, MCP)
2. **ragent-tui** - Terminal UI (app.rs has slash commands, input.rs has keyboard handling)
3. **ragent-server** - HTTP API server

## 4 Ways to Add LSP Support

| Option | Where | Pros | Cons |
|--------|-------|------|------|
| **Native Tool** | `/tool/lsp.rs` + register in `create_default_registry()` | Fast, full control | More code to maintain |
| **MCP Server** | Config `mcp.*.type = "stdio"` | Flexible, reusable | Extra process overhead |
| **Slash Command** | `app.rs` SLASH_COMMANDS + `execute_slash_command()` | User-friendly | Limited for agent automation |
| **Skill** | `skills/lsp_*.toml` | Agent-invocable, modular | Need skill system knowledge |

## Key File Locations

```
Core Logic:
  tool/mod.rs              → Tool trait, ToolRegistry, ToolContext, ToolOutput
  agent/mod.rs             → AgentInfo, system prompt building (line 438)
  event/mod.rs             → EventBus, Event enum (30+ event types)
  session/processor.rs     → Main agentic loop (line 77)
  message/mod.rs           → Message, MessagePart, ToolCallState
  config/mod.rs            → Config loading, MCP/LSP config schema
  mcp/mod.rs               → McpClient, MCP server connections
  provider/mod.rs          → LLM provider abstractions

TUI:
  app.rs                   → App state, slash commands (line 159: SLASH_COMMANDS)
  app.rs                   → execute_slash_command() at line 1158
  input.rs                 → InputAction enum, keyboard routing

CLI:
  src/main.rs              → Entry point (line 141: main fn)
```

## Tool Execution Flow

```rust
// 1. Tool registered in registry
pub fn create_default_registry() -> ToolRegistry {
    let mut registry = ToolRegistry::new();
    registry.register(Arc::new(MyLspTool));  // ← Add here
    registry
}

// 2. Tool definition
#[async_trait::async_trait]
impl Tool for MyLspTool {
    fn name(&self) -> &str { "lsp_query" }
    fn parameters_schema(&self) -> Value { /* ... */ }
    fn permission_category(&self) -> &str { "code:query" }  // For permissions
    
    async fn execute(&self, input: Value, ctx: &ToolContext) -> Result<ToolOutput> {
        // ctx.session_id, ctx.working_dir, ctx.event_bus available
        Ok(ToolOutput {
            content: "symbol found at line 42".to_string(),
            metadata: Some(json!({"line": 42, "col": 5})),
        })
    }
}

// 3. Tool gets called by LLM, output embedded in conversation
// 4. Events published (ToolCallStart, ToolCallEnd)
// 5. TUI updates with tool results
```

## System Prompt Injection

```rust
// File: session/processor.rs, line 172
let file_tree = build_file_tree(&working_dir, 2);
let skill_registry = crate::skill::SkillRegistry::load(&working_dir, &skill_dirs);
let system_prompt = build_system_prompt(agent, &working_dir, &file_tree, Some(&skill_registry));

// Order of sections (agent/mod.rs line 409-550):
// 1. Agent role definition
// 2. Working directory path
// 3. Project file tree (first 2 levels)
// 4. AGENTS.md project guidelines (if exists)
// 5. Available skills (agent-invocable only)
// 6. Tool usage guidelines
// 7. File reading best practices
```

## Event System

```rust
// Publish from tool
ctx.event_bus.publish(Event::ToolCallStart {
    session_id: "session-1".to_string(),
    call_id: "call-123".to_string(),
    tool: "lsp_query".to_string(),
});

// Event types (event/mod.rs line 35)
Event::ToolCallStart { session_id, call_id, tool }
Event::ToolCallEnd { session_id, call_id, tool, error, duration_ms }
Event::TextDelta { session_id, text }
Event::PermissionRequested { session_id, request_id, permission, description }
Event::AgentError { session_id, error }
// ... 30+ more variants

// TUI subscribes
let mut rx = self.event_bus.subscribe();
while let Ok(event) = rx.recv().await {
    // Update UI based on event
}
```

## Configuration Loading

```json
{
  "mcp": {
    "rust-analyzer": {
      "type": "stdio",
      "command": "rust-analyzer",
      "args": [],
      "env": {}
    }
  },
  "lsp": {
    "enabled": true,
    "servers": {
      "rust-analyzer": {
        "type": "stdio",
        "command": "rust-analyzer"
      }
    }
  }
}
```

Precedence (config/mod.rs line 252):
1. Compiled defaults
2. `~/.config/ragent/ragent.json` (global)
3. `./ragent.json` (project)
4. `$RAGENT_CONFIG` env var (file path)
5. `$RAGENT_CONFIG_CONTENT` env var (inline JSON)

## Slash Commands

```rust
// Define in app.rs around line 159
pub const SLASH_COMMANDS: &[SlashCommandDef] = &[
    SlashCommandDef {
        trigger: "lsp_symbol",
        description: "Find symbol definition",
    },
];

// Handle in execute_slash_command() at line 1158
"lsp_symbol" => {
    let args = stripped.split_once(char::is_whitespace)
        .map(|(_, a)| a.trim())
        .unwrap_or("");
    
    // Your LSP logic here
    self.append_assistant_text(&format!("Symbol: {}", args));
    self.status = "symbol found".to_string();
}
```

## Message Composition

```rust
pub struct Message {
    pub id: String,
    pub session_id: String,
    pub role: Role,              // User or Assistant
    pub parts: Vec<MessagePart>, // Can have multiple parts
    pub created_at: DateTime<Utc>,
}

pub enum MessagePart {
    Text { text: String },
    ToolCall {
        tool: String,
        call_id: String,
        state: ToolCallState { status, input, output, error, duration_ms },
    },
    Reasoning { text: String },
}

// Example: Tool output as message part
let tool_result = ToolCallState {
    status: ToolCallStatus::Completed,
    input: json!({"symbol": "main", "file": "src/main.rs"}),
    output: Some(json!({
        "definition": "fn main() { ... }",
        "line": 10,
        "file": "src/main.rs"
    })),
    error: None,
    duration_ms: Some(5),
};
```

## Permission System

```rust
// Tools declare permission categories
impl Tool for MyTool {
    fn permission_category(&self) -> &str {
        "code:query"  // or "file:read", "bash:execute", etc.
    }
}

// Checked before execution
pub struct PermissionChecker {
    rules: Vec<PermissionRule>,
}

// If denied or in "Ask" mode, publishes event
Event::PermissionRequested {
    session_id: "s1".to_string(),
    request_id: "req-123".to_string(),
    permission: "code:query".to_string(),
    description: "Query LSP for symbol definition".to_string(),
}

// User replies with y/n/a (all)
Event::PermissionReplied {
    session_id: "s1".to_string(),
    request_id: "req-123".to_string(),
    allowed: true,
}
```

## Integration Recommendations

**Step 1: Start Simple**
- Implement LSP as a native Tool in `/tool/lsp.rs`
- Register in `create_default_registry()`
- Test with a single LSP capability (e.g., goto_definition)

**Step 2: Add Configuration**
- Extend `Config` in `/config/mod.rs` with `lsp` section
- Load LSP server configs at startup
- Connect to configured servers in Tool::new() or on-demand

**Step 3: Integrate with Agent Prompts**
- Update `build_system_prompt()` to mention LSP availability
- Add LSP context to specific agents (e.g., "explore" agent)

**Step 4: Add Slash Commands** (optional)
- `/lsp_symbol <name>` - Find symbol
- `/lsp_hover <file>:<line>:<col>` - Get hover info
- `/lsp_connect <server>` - Connect to LSP server

**Step 5: MCP Integration** (optional)
- Wrap LSP in MCP server wrapper
- Let agents discover LSP tools via MCP

## Testing Checklist

- [ ] LSP tool executes and returns JSON output
- [ ] Tool results appear in message history
- [ ] Events published (ToolCallStart, ToolCallEnd)
- [ ] Permissions respected (Ask/Allow/Deny)
- [ ] TUI displays tool output
- [ ] Configuration loading works
- [ ] Multiple LSP servers co-exist
- [ ] Slash commands work
- [ ] Handles LSP server crashes gracefully
- [ ] Timeouts respected (default 120s)

## Files You'll Likely Touch

```
NEW:
  crates/ragent-core/src/tool/lsp.rs          (LSP tool implementation)
  tests/test_lsp_integration.rs               (LSP tests)

MODIFIED:
  crates/ragent-core/src/tool/mod.rs          (register LSP tool)
  crates/ragent-core/src/config/mod.rs        (add LSP config struct)
  crates/ragent-core/src/agent/mod.rs         (mention LSP in system prompt)
  crates/ragent-tui/src/app.rs                (add /lsp slash commands)
  Cargo.toml                                   (add LSP dependencies: lsp-types, jsonrpc, etc.)

OPTIONAL:
  crates/ragent-core/src/event/mod.rs         (add LSP-specific events)
  crates/ragent-core/src/message/mod.rs       (if structuring LSP results specially)
```

## Development Workflow

```bash
# Run tests
cargo test test_lsp

# Run single tool test
cargo test tool::lsp::tests::test_goto_definition

# Run TUI with logging
RUST_LOG=debug cargo run -- --log

# Run headless
cargo run -- run "what functions define the lsp_query symbol?"

# Check formatting
cargo fmt --check

# Lint
cargo clippy
```

---

**See full guide: `/home/thawkins/Projects/ragent/LSP_INTEGRATION_GUIDE.md`**
