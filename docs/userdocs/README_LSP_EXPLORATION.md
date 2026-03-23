# Ragent LSP Integration Exploration - Summary

## Overview
This document summarizes the comprehensive exploration of the Ragent codebase for LSP (Language Server Protocol) integration planning. Three detailed documents have been created to guide your implementation.

## Documents Created

### 1. **LSP_INTEGRATION_GUIDE.md** (1100+ lines, 33 KB)
The most comprehensive reference covering:
- Complete crate structure with file paths and dependencies
- Detailed tool system architecture (Tool trait, ToolRegistry, ToolContext)
- Tool execution flow and registration pattern
- Slash command system (SLASH_COMMANDS array, routing logic)
- Event system with all 30+ event types
- Session and configuration management with config loading precedence
- Agent system prompt building (7-section assembly order)
- Existing bash tool and MCP client as external process examples
- Message/context flow through the agentic loop
- 4 different integration approach options
- Full configuration examples (JSON)
- Integration checklist and recommended LSP tool interface

**Use this for:** Deep understanding and detailed implementation guidance

### 2. **LSP_QUICK_REFERENCE.md** (400+ lines, 9 KB)
Fast lookup reference with:
- Architecture flow diagram
- 3-crate overview
- 4-way comparison table for LSP integration approaches
- Key file locations with line numbers
- Tool execution flow (code snippet)
- System prompt injection details
- Event system quick examples
- Configuration structure
- Slash command patterns
- Message composition
- Permission system overview
- Recommended integration steps
- Testing checklist
- Files to create/modify
- Development workflow commands

**Use this for:** Quick answers, copy-paste code examples, finding specific line numbers

### 3. **STRUCTURE_OVERVIEW.txt** (500+ lines, 15 KB)
Visual project structure with:
- Complete directory tree with file paths
- Module documentation showing key line numbers and content
- 5 major data flows (config loading, tool execution, prompt building, message flow, event publishing)
- Integration points for LSP
- Recommended implementation order
- Cross-referenced line numbers throughout

**Use this for:** Navigation, understanding file organization, finding related code sections

## Key Findings

### Crate Architecture
```
ragent-core (Business logic)
  ├── tool/mod.rs - Tool trait & registry
  ├── agent/mod.rs - Agent definitions & system prompt
  ├── session/processor.rs - Agentic loop
  ├── event/mod.rs - Event bus
  ├── config/mod.rs - Configuration
  ├── mcp/mod.rs - MCP server integration
  └── message/mod.rs - Message types

ragent-tui (Terminal UI)
  ├── app.rs - Slash commands (line 159)
  ├── input.rs - Keyboard routing
  └── tests/test_slash_commands.rs

ragent-server (HTTP API)
  └── routes/mod.rs
```

### Tool System
- **Tool trait** in `tool/mod.rs` line 108
- **ToolRegistry** for managing tools by name
- **ToolContext** provides session_id, working_dir, event_bus, storage, task_manager
- **ToolOutput** contains content string + optional metadata JSON
- Built-in tools: bash, read, write, edit, grep, glob, webfetch, office_*, pdf_*

### Event System
- **EventBus** using tokio broadcast channels
- **30+ event types**: SessionCreated, MessageStart, TextDelta, ToolCallStart/End, PermissionRequested, etc.
- Tools publish events directly: `ctx.event_bus.publish(Event::ToolCallStart {...})`
- TUI subscribes and updates UI based on events

### Slash Command System
- **SLASH_COMMANDS** array at line 159 in app.rs
- **execute_slash_command()** at line 1158 in app.rs
- Currently: /about, /agent, /clear, /compact, /help, /log, /model, /provider, /quit, /resume, /system, /tools, /skills, /cancel
- Auto-complete menu with up/down/enter navigation

### System Prompt Building
- **build_system_prompt()** at line 438 in agent/mod.rs
- 7-section assembly:
  1. Agent role definition
  2. Working directory path
  3. Project file tree (first 2 levels)
  4. AGENTS.md project guidelines (if present)
  5. Available skills from skill registry
  6. Tool usage guidelines
  7. File reading best practices

### Configuration
- **Config precedence**: global (~/.config/ragent/ragent.json) → project (./ragent.json) → env var → inline JSON
- **MCP servers** already configurable in config.mcp section
- **LSP configuration** would be added similarly

### Session Processing Loop
```
process_message(session_id, user_text, agent, cancel_flag)
  1. Store user message
  2. Load history
  3. Build system prompt
  4. Stream LLM response
  5. For each tool call:
     - Check permissions
     - Execute tool
     - Emit ToolCallEnd event
     - Embed output in conversation
  6. Loop until finish_reason != ToolUse
  7. Store assistant message
  8. Publish MessageEnd event
```

## Recommended LSP Integration Approach

### Best: Hybrid (Native Tool + MCP)

**Phase 1: Native Tool**
- Implement `/crates/ragent-core/src/tool/lsp.rs`
- Register in `create_default_registry()`
- Provides: goto_definition, find_references, hover, completion, diagnostics, document_symbols, workspace_symbols
- Configuration in `config/mod.rs` with LSP server definitions

**Phase 2: MCP Wrapper** (optional)
- Wrap external LSP servers as MCP servers
- Reuse existing MCP infrastructure
- Allow agent-driven discovery of available tools

**Phase 3: Agent Awareness** (optional)
- Update system prompt to mention LSP
- Add LSP-specific agents (e.g., "code-analyst")
- Integrate LSP results into skill system

## Implementation Checklist

- [ ] Review `tool/mod.rs` and `agent/mod.rs` for patterns
- [ ] Create `tool/lsp.rs` with LspTool struct
- [ ] Add LSP config struct to `config/mod.rs`
- [ ] Register tool in `create_default_registry()`
- [ ] Implement LSP client (stdio-based, tokio)
- [ ] Add LSP-specific events to `event/mod.rs` (optional)
- [ ] Add `/lsp_*` slash commands to `app.rs`
- [ ] Update agent prompt in `agent/mod.rs` to mention LSP
- [ ] Add tests in `tests/test_lsp_integration.rs`
- [ ] Handle permission gating (tool returns permission_category)
- [ ] Test with real LSP servers (rust-analyzer, pylsp, etc.)
- [ ] Document in README

## Code Structure Pattern

All three documents show the same pattern for implementation:

```rust
// 1. Define tool in tool/lsp.rs
#[async_trait::async_trait]
impl Tool for LspTool {
    fn name(&self) -> &str { "lsp_query" }
    fn permission_category(&self) -> &str { "code:query" }
    async fn execute(&self, input: Value, ctx: &ToolContext) -> Result<ToolOutput> {
        // Use ctx.session_id, ctx.working_dir, ctx.event_bus
        Ok(ToolOutput { content: "...", metadata: Some(...) })
    }
}

// 2. Register in tool/mod.rs
pub fn create_default_registry() -> ToolRegistry {
    let mut registry = ToolRegistry::new();
    registry.register(Arc::new(LspTool));
    registry
}

// 3. Add slash command in app.rs
pub const SLASH_COMMANDS: &[SlashCommandDef] = &[
    SlashCommandDef { trigger: "lsp_symbol", description: "..." },
];

// 4. Handle in execute_slash_command()
"lsp_symbol" => {
    // Your logic here
}

// 5. Events published automatically by processor loop
ctx.event_bus.publish(Event::ToolCallStart { ... });
```

## Key Technical Details

1. **Async/Tokio Throughout**
   - All I/O is async
   - Tools use `async fn execute()`
   - Session processor uses tokio::time::timeout()

2. **Permission Gating**
   - Each tool declares `permission_category()`
   - PermissionChecker publishes PermissionRequested event
   - TUI intercepts user y/n/a keys
   - Permission reply event enables/denies execution

3. **Event-Driven Architecture**
   - Tools publish ToolCallStart/End
   - TUI subscribes to event stream
   - Message storage persists results
   - Complete audit trail maintained

4. **Message Composition**
   - Messages contain multiple MessageParts
   - ToolCall parts track execution state
   - Tool output stored in ToolCallState.output
   - Full history available for context

5. **Configuration Flexibility**
   - Multiple LSP servers in config
   - Per-server settings and environments
   - Disabled/Failed status handling
   - Timeout configuration

## Testing Guidance

From `crates/ragent-tui/tests/test_slash_commands.rs`:
```rust
#[test]
fn test_lsp_symbol_query() {
    let mut app = make_app();
    app.session_id = Some("test-session".to_string());
    
    app.execute_slash_command("/lsp_symbol main");
    
    assert!(!app.messages.is_empty());
    // Verify tool output in messages
    // Check events published
    // Confirm status updated
}
```

## Next Steps

1. **Read** LSP_QUICK_REFERENCE.md for quick patterns
2. **Skim** LSP_INTEGRATION_GUIDE.md sections as needed
3. **Reference** STRUCTURE_OVERVIEW.txt while coding
4. **Start** with creating `tool/lsp.rs` - the pattern is clear
5. **Test** incrementally with one LSP capability at a time
6. **Iterate** with real LSP servers (rust-analyzer is easiest)

## File Statistics

| Document | Size | Lines | Best For |
|----------|------|-------|----------|
| LSP_INTEGRATION_GUIDE.md | 33 KB | 1100+ | Comprehensive reference |
| LSP_QUICK_REFERENCE.md | 9.3 KB | 400+ | Quick lookup, examples |
| STRUCTURE_OVERVIEW.txt | 15 KB | 500+ | Navigation, organization |
| README_LSP_EXPLORATION.md | This file | 250+ | Quick summary |

---

## Quick Example: Adding an LSP Tool

Based on the exploration, here's the minimal implementation:

```rust
// crates/ragent-core/src/tool/lsp.rs
use async_trait::async_trait;
use serde_json::{Value, json};
use crate::tool::{Tool, ToolContext, ToolOutput};

pub struct LspQueryTool;

#[async_trait]
impl Tool for LspQueryTool {
    fn name(&self) -> &str { "lsp" }
    fn description(&self) -> &str { "Query language server for code intelligence" }
    fn parameters_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "action": { "type": "string", "enum": ["definition", "references", "hover"] },
                "file": { "type": "string" },
                "line": { "type": "integer" },
                "column": { "type": "integer" },
                "symbol": { "type": "string" }
            },
            "required": ["action"]
        })
    }
    fn permission_category(&self) -> &str { "code:query" }
    
    async fn execute(&self, input: Value, ctx: &ToolContext) -> Result<ToolOutput> {
        let action = input["action"].as_str().unwrap_or("definition");
        // Implementation here
        Ok(ToolOutput {
            content: format!("LSP query for {}", action),
            metadata: None,
        })
    }
}
```

Then in `tool/mod.rs`:
```rust
pub fn create_default_registry() -> ToolRegistry {
    let mut registry = ToolRegistry::new();
    // ... other tools ...
    registry.register(Arc::new(lsp::LspQueryTool)); // Add this
    registry
}
```

That's it! The tool is now available to agents.

---

**Created**: 2024
**Project**: Ragent AI Coding Agent
**Exploration Scope**: Comprehensive structural analysis for LSP integration planning
