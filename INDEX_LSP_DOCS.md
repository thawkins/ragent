# Ragent LSP Integration Documentation Index

## Quick Navigation

**New to this project? Start here:**
1. Read **README_LSP_EXPLORATION.md** (5 min) - Overview and quick example
2. Skim **LSP_QUICK_REFERENCE.md** (10 min) - Key patterns and file locations
3. Bookmark **LSP_INTEGRATION_GUIDE.md** - Reference as you code
4. Check **STRUCTURE_OVERVIEW.txt** - When you need to find something

---

## Document Purposes

### README_LSP_EXPLORATION.md
**Your entry point.** What was discovered in this exploration session.
- 📋 Overview of all 3 documents created
- 🎯 Key findings from codebase analysis
- 🏗️ Recommended hybrid integration approach
- ✅ Implementation checklist
- 💡 Quick example code
- 📊 Document statistics

**Length:** ~250 lines  
**Read Time:** 5-10 minutes  
**When to Read:** First thing, then as reference while planning

---

### LSP_QUICK_REFERENCE.md  
**Your go-to for code patterns and line numbers.**
- 📐 Architecture diagram
- 📊 4-way integration comparison table
- 📂 Key file locations with line numbers
- 💻 Code examples (tool execution flow, system prompt, events)
- 🔐 Permission system overview
- ✅ Testing checklist
- 🔧 Development workflow commands
- 📝 Files to create/modify list

**Length:** ~400 lines  
**Read Time:** 15-20 minutes (or 2 min for lookup)  
**When to Read:** While coding, for quick patterns and examples

---

### LSP_INTEGRATION_GUIDE.md
**Your comprehensive reference for every detail.**
- 🏗️ Complete crate structure (3 crates, all modules)
- 🛠️ Tool system architecture (Tool trait, registry, context)
- ⚡ Tool execution flow with code
- 💬 Slash command system (parsing, routing, menu)
- 📡 Event system (EventBus, 30+ event types)
- 📦 Session & configuration (config loading, MCP servers)
- 🎯 Agent system prompt building (7-section assembly)
- 🔌 External process tools (bash, MCP client)
- 💬 Message/context flow (session processor loop)
- 📋 4 integration approaches with pros/cons
- 📝 Configuration examples (JSON)
- 🧩 Recommended LSP tool interface
- ✅ Integration checklist

**Length:** 1100+ lines  
**Read Time:** 1-2 hours (reference, not sequential)  
**When to Read:** Deep dives, implementation details, decision-making

---

### STRUCTURE_OVERVIEW.txt
**Your map of the codebase.**
- 📂 Complete directory tree with annotations
- 📍 Module documentation with line numbers and content summaries
- 🔄 5 major data flow diagrams (config, tool execution, prompt, message, events)
- 🔗 Integration points for LSP
- 📋 Implementation order recommendations
- 🎯 Cross-referenced line numbers throughout

**Length:** 500+ lines  
**Read Time:** 30 minutes (scan as needed)  
**When to Read:** Finding files, understanding connections between modules

---

## How These Documents Were Created

### Exploration Process
1. **Crate Structure Analysis**
   - Examined Cargo.toml files
   - Found all 3 crates: ragent-core, ragent-tui, ragent-server

2. **Core Module Discovery**
   - Located tool/mod.rs (Tool trait, registry)
   - Located event/mod.rs (EventBus, events)
   - Located agent/mod.rs (agents, system prompt)
   - Located session/processor.rs (agentic loop)
   - Located config/mod.rs (configuration loading)
   - Located mcp/mod.rs (MCP integration)

3. **Tool System Analysis**
   - Studied Tool trait (name, description, parameters_schema, permission_category, execute)
   - Examined ToolRegistry (HashMap-based registry)
   - Analyzed ToolContext (session_id, working_dir, event_bus, storage, task_manager)
   - Reviewed ToolOutput (content + metadata)
   - Checked 18+ built-in tools

4. **Slash Command System**
   - Found SLASH_COMMANDS array at app.rs:159
   - Traced execute_slash_command() at app.rs:1158
   - Analyzed input.rs routing (InputAction enum)
   - Reviewed auto-complete menu logic

5. **Event System**
   - Documented EventBus (tokio broadcast)
   - Catalogued 30+ Event variants
   - Traced publish/subscribe patterns

6. **Message Flow**
   - Tracked user input → system prompt → LLM → tool execution → message storage
   - Analyzed Message/MessagePart composition
   - Reviewed ToolCallState tracking

7. **Cross-Cutting Concerns**
   - Permission gating system
   - Configuration loading (precedence order)
   - MCP server integration
   - Session management

### What's Included
✅ **All requested analyses:**
1. ✅ Crate structure (3 crates, all modules, purposes)
2. ✅ Tool system (Tool trait, ToolContext, ToolOutput, registry)
3. ✅ Slash commands (parsing, handling, /lsp examples)
4. ✅ Event system (EventBus, Event enum, 30+ types)
5. ✅ Session/config (Session struct, Config loading precedence, MCP config)
6. ✅ Agent system prompt (7-section assembly order)
7. ✅ External process tools (bash tool, MCP client, stdio communication)
8. ✅ Message/context flow (session processor loop, tool output embedding)

### What's Beyond Scope
❌ Code you'd need to write (but guides show patterns)
❌ Testing (but patterns shown from existing tests)
❌ Performance optimization (but architecture documented)
❌ Full MCP implementation details (but points to rmcp SDK)

---

## Key Numbers to Remember

| Item | Value | Location |
|------|-------|----------|
| Number of crates | 3 | Cargo.toml |
| Built-in tools | 18+ | tool/*.rs |
| Event types | 30+ | event/mod.rs |
| Slash commands | 15 | app.rs:159 |
| System prompt sections | 7 | agent/mod.rs:438 |
| Config loading precedence | 5 layers | config/mod.rs:252 |
| Default tool timeout | 120 seconds | tool/bash.rs:19 |
| Max output truncation | 100 KB | tool/bash.rs:105 |

---

## Integration Decision Matrix

**Choose your approach based on needs:**

| Need | Solution | Docs | Complexity |
|------|----------|------|-----------|
| Quick agent-controlled LSP | Native Tool | QUICK_REF + GUIDE | Medium |
| Flexible external LSP | MCP Server | GUIDE §7 | Medium |
| User-triggered LSP | Slash Commands | QUICK_REF + GUIDE | Low |
| Agent-invoked LSP | Skill | GUIDE (not detailed) | High |
| All of the above | Hybrid | GUIDE §9 | High |

---

## File Locations Reference

**Understanding these 8 files is 80% of the work:**

```
1. tool/mod.rs
   └─ Tool trait, ToolRegistry, create_default_registry()
   └─ YOU REGISTER YOUR TOOL HERE

2. agent/mod.rs
   └─ build_system_prompt() - where agents learn about tools
   └─ YOU MENTION LSP CAPABILITIES HERE

3. session/processor.rs
   └─ process_message() - the main loop that calls tools
   └─ NO CHANGES NEEDED HERE

4. event/mod.rs
   └─ Event enum, EventBus
   └─ YOU MIGHT ADD LSP EVENTS HERE

5. app.rs (TUI)
   └─ SLASH_COMMANDS array, execute_slash_command()
   └─ YOU ADD /lsp_* COMMANDS HERE

6. config/mod.rs
   └─ Config struct, config loading
   └─ YOU ADD LSP CONFIG STRUCT HERE

7. message/mod.rs
   └─ Message, MessagePart, ToolCallState
   └─ NO CHANGES NEEDED (uses existing structures)

8. permission/mod.rs
   └─ Permission gating
   └─ YOU DECLARE permission_category() IN YOUR TOOL
```

---

## Implementation Path

```
1. Plan (You are here)
   ├─ Read README_LSP_EXPLORATION.md
   ├─ Skim LSP_QUICK_REFERENCE.md  
   ├─ Review STRUCTURE_OVERVIEW.txt
   └─ Decide: Native Tool vs MCP vs Slash vs Skill

2. Setup
   ├─ Create crates/ragent-core/src/tool/lsp.rs
   └─ Add dependencies to Cargo.toml

3. Core Implementation  
   ├─ Implement Tool trait for LspTool
   ├─ Register in tool/mod.rs
   └─ Test with simple LSP queries

4. Configuration
   ├─ Add LspConfig to config/mod.rs
   ├─ Load LSP servers at startup
   └─ Handle server lifecycle

5. Integration
   ├─ Add slash commands to app.rs
   ├─ Mention LSP in system prompt (agent/mod.rs)
   ├─ Add permission category
   └─ Publish LSP-specific events (optional)

6. Testing
   ├─ Unit tests for tool
   ├─ Integration tests with real LSP
   └─ TUI command tests

7. Documentation
   ├─ Update README.md
   ├─ Add LSP configuration examples
   └─ Document / commands
```

---

## Document Statistics

```
Total: 4 documents created
       ~2100 lines of documentation
       ~60 KB of content

Breakdown:
├─ LSP_INTEGRATION_GUIDE.md      1106 lines, 33 KB (comprehensive)
├─ LSP_QUICK_REFERENCE.md         400 lines, 9.3 KB (patterns)
├─ STRUCTURE_OVERVIEW.txt         500 lines, 15 KB (navigation)
└─ README_LSP_EXPLORATION.md      250 lines, 7 KB (summary)
```

---

## Common Questions Answered By...

**"How do tools work?"** → LSP_QUICK_REFERENCE.md § Tool Execution Flow  
**"Where is the agentic loop?"** → STRUCTURE_OVERVIEW.txt § session/processor.rs  
**"How do I add a tool?"** → README_LSP_EXPLORATION.md § Quick Example  
**"What events exist?"** → LSP_INTEGRATION_GUIDE.md § Event System  
**"How are slash commands handled?"** → LSP_QUICK_REFERENCE.md § Slash Commands  
**"What's the config loading order?"** → LSP_INTEGRATION_GUIDE.md § Configuration  
**"Where do I register my tool?"** → STRUCTURE_OVERVIEW.txt § tool/mod.rs  
**"How is the system prompt built?"** → LSP_INTEGRATION_GUIDE.md § System Prompt Building  
**"What about permissions?"** → LSP_QUICK_REFERENCE.md § Permission System  
**"What are the 4 integration approaches?"** → LSP_INTEGRATION_GUIDE.md § Section 9  

---

## Next Steps

1. **Now**: Read README_LSP_EXPLORATION.md (5 min)
2. **Next**: Skim LSP_QUICK_REFERENCE.md and bookmark it
3. **Then**: Review STRUCTURE_OVERVIEW.txt for the files you'll touch
4. **Finally**: Use LSP_INTEGRATION_GUIDE.md as your detailed reference while implementing

---

**Created**: March 17, 2024  
**For**: Ragent LSP Integration Planning  
**By**: Automated Codebase Exploration Agent  

