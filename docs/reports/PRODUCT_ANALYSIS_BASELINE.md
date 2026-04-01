# ragent Product Analysis - Current Baseline

**Analysis Date:** March 30, 2026  
**Product Version:** 0.1.0-alpha.20  
**Analyst:** swarm-s1  
**Task ID:** s1

---

## Executive Summary

ragent is a Rust-based AI coding agent for the terminal that provides multi-provider LLM orchestration, a comprehensive tool system, and a client/server architecture compiled into a single statically-linked binary with no runtime dependencies. It was built as a learning exercise inspired by OpenCode, reimplemented in Rust with significant architectural enhancements including agent teams, custom agent support, LSP integration, and multi-agent orchestration.

**Key Metrics:**
- **Codebase Size:** ~2.5M lines of Rust code
- **Architecture:** 3-crate workspace structure
- **Maturity:** Early alpha (v0.1.0-alpha.20)
- **Core Files:** 116 Rust source files in core crate alone
- **Built-in Tools:** 60+ tools (8 basic + 15 extended + 3 sub-agent + 20+ team + 5 LSP + office/PDF tools)
- **Supported Providers:** 5 (Anthropic, OpenAI, GitHub Copilot, Ollama, Generic OpenAI)

---

## 1. Core Functionality and Capabilities

### 1.1 Primary Use Cases
1. **Interactive AI Coding Assistant** - Full-screen terminal UI with streaming chat
2. **File Operations** - Read, write, create, edit, delete files with atomic operations
3. **Shell Integration** - Execute bash commands with permission gating
4. **Code Intelligence** - LSP integration for hover, go-to-definition, find-references, diagnostics
5. **Multi-Agent Coordination** - Teams for parallel work and subagents for focused tasks
6. **Session Management** - Persistent conversation history with resume/import/export
7. **Document Processing** - Office documents (Word, Excel, PowerPoint) and PDF read/write
8. **Web Integration** - Web fetch and search capabilities

### 1.2 Agent System
**Built-in Agents:**
- `general` - General-purpose coding assistant (default)
- `coder` - Code implementation specialist
- `build` - Build, test, and debugging agent
- `plan` - Planning agent for implementation plans (read-only, subagent)
- `explore` - Codebase exploration agent (read-only, subagent)
- `title` - Session title generator (hidden subagent)
- `summary` - Session summarization (hidden subagent)

**Custom Agent Support:**
- Agent Profiles (.md) - Markdown files with JSON frontmatter
- OASF Records (.json) - Open Agentic Schema Framework standard
- Discovery paths: `~/.ragent/agents/` (user-global) and `.ragent/agents/` (project-local)
- Template variables: `{{WORKING_DIR}}`, `{{FILE_TREE}}`, `{{AGENTS_MD}}`, `{{DATE}}`
- Persistent memory scopes: user, project, or none

### 1.3 Tool Inventory

**Core Tools (8):**
- `read` - Read file contents with line-range support and section mapping
- `write` - Write content to files
- `create` - Create new files with content
- `edit` - In-place file editing with exact string replacement
- `bash` - Shell command execution with timeout and resource limits
- `grep` - Search file contents by pattern
- `glob` - Find files matching glob patterns
- `list` - Directory listing with tree output and depth control

**Extended Tools (15+):**
- `multiedit` - Atomic multi-file edits
- `patch` - Apply unified diff patches
- `webfetch` - Fetch URL content (HTML to text conversion)
- `websearch` - Web search via Tavily API
- `plan_enter` / `plan_exit` - Delegate to plan agent
- `todo_read` / `todo_write` - Session TODO management
- `rm` - File deletion
- Office document tools: `office_read`, `office_write`, `office_info`
- LibreOffice tools: `libre_read`, `libre_write`, `libre_info`
- PDF tools: `pdf_read`, `pdf_write`

**Sub-agent Tools (3):**
- `new_task` - Spawn background sub-agent tasks
- `cancel_task` - Cancel running background tasks
- `list_tasks` - List and monitor sub-agent tasks
- `wait_tasks` - Block until background tasks complete

**LSP Tools (5):**
- `lsp_hover` - Get type information and documentation
- `lsp_definition` - Find symbol definitions
- `lsp_references` - Find all usages of a symbol
- `lsp_symbols` - List all symbols in a file
- `lsp_diagnostics` - Get compiler errors and warnings

**Team Coordination Tools (20+):**
- Team lifecycle: `team_create`, `team_cleanup`
- Team spawning: `team_spawn`
- Task management: `team_task_create`, `team_task_claim`, `team_task_complete`, `team_task_list`, `team_task_assign`
- Communication: `team_message`, `team_broadcast`, `team_read_messages`
- Status: `team_status`, `team_idle`
- Plan approval: `team_submit_plan`, `team_approve_plan`
- Memory: `team_memory_read`, `team_memory_write`
- Shutdown: `team_shutdown_teammate`, `team_shutdown_ack`
- Synchronization: `team_wait`

---

## 2. Technology Stack and Architecture

### 2.1 Core Technologies
- **Language:** Rust edition 2024 (requires Rust 1.85+)
- **Async Runtime:** Tokio with full feature set
- **Database:** SQLite with bundled compilation (rusqlite)
- **HTTP Framework:** Axum 0.8 with SSE streaming
- **TUI Framework:** Ratatui 0.29 with crossterm
- **Serialization:** serde_json, serde_yaml
- **Error Handling:** anyhow, thiserror
- **Logging:** tracing, tracing-subscriber
- **HTTP Client:** reqwest with rustls-tls (no native-tls dependency)

### 2.2 Architecture Pattern

**Cargo Workspace Structure:**
```
ragent (binary) → depends on:
  ├── ragent-core    (types, storage, config, providers, tools, agents, sessions)
  ├── ragent-server  (Axum HTTP routes, SSE streaming)
  └── ragent-tui     (Ratatui terminal interface)
```

**Data Flow:**
```
User Input (TUI/HTTP)
    ↓
Event Bus (pub/sub for real-time UI updates)
    ↓
Session Processor (agent loop → LLM call → tool execution)
    ↓
├── Provider (LLM API: Anthropic, OpenAI, Copilot, Ollama, Generic)
├── Tools (60+ capabilities)
└── Storage (SQLite for sessions/messages/snapshots)
```

**Key Components:**

1. **ragent-core** (foundational library):
   - `agent/` - Agent definitions, built-in registry, OASF/profile loading
   - `config/` - Configuration loading and validation
   - `event/` - Event bus for pub/sub messaging
   - `llm/` - LLM types and tool definitions
   - `lsp/` - Language Server Protocol client
   - `mcp/` - Model Context Protocol support (stub)
   - `message/` - Message types and serialization
   - `permission/` - Permission system and rule evaluation
   - `provider/` - LLM provider implementations
   - `session/` - Session management and processor loop
   - `skill/` - Skill discovery, loading, and invocation
   - `snapshot/` - File snapshots for undo
   - `storage/` - SQLite storage layer
   - `task/` - Sub-agent task management
   - `team/` - Agent team coordination (config, mailbox, manager, tasks)
   - `tool/` - Tool implementations and registry
   - `orchestrator/` - Multi-agent orchestration (coordinator, leader election, policy)

2. **ragent-server** (HTTP/REST/SSE API):
   - REST endpoints for session CRUD, message sending, task spawning
   - SSE streaming for real-time events
   - Prompt optimization endpoint (`/opt`)
   - Multi-agent orchestration endpoints

3. **ragent-tui** (Terminal UI):
   - Full-screen ratatui interface
   - Home screen, provider setup dialog
   - Chat panel, teams panel, log panel
   - Slash-command autocomplete
   - Agent cycling and model selection
   - Streaming tool call display with step-numbered JSON
   - Mouse support and right-click context menus
   - Image attachment support (Alt+V)

### 2.3 Storage Layout

**User Data:**
- `~/.local/share/ragent/ragent.db` - SQLite database (sessions, messages, API keys)
- `~/.config/ragent/config.json` - User configuration
- `~/.ragent/agents/` - User-global custom agents
- `~/.ragent/teams/` - User-global teams
- `~/.ragent/skills/` - Personal skills

**Project Data:**
- `[PROJECT]/.ragent/agents/` - Project-local custom agents (higher priority)
- `[PROJECT]/.ragent/teams/` - Project-local teams
- `ragent.json` or `ragent.jsonc` - Project configuration

**Team Storage Structure:**
```
~/.ragent/teams/{team-name}/
    config.json          # Team metadata and member list
    tasks.json           # Shared task list (file-locked)
    mailbox/
        {agent-id}.json  # Per-agent message queue
```

---

## 3. User-Facing Features and Workflows

### 3.1 Terminal UI (TUI) Features
- **Home Screen:** Session list, provider health indicators, quick actions
- **Chat Panel:** Streaming messages with markdown rendering, code blocks, tables
- **Teams Panel:** Team status, member list, task overview
- **Log Panel:** Real-time tracing output with step-numbered tool calls
- **Input Widget:** Multi-line editing, history (↑/↓), slash-command autocomplete
- **Keybindings:**
  - `?` - Show keybindings help panel
  - `p` - Provider setup dialog
  - `m` - Model selector
  - `/agent` - Agent picker
  - `Alt+V` - Attach image from clipboard or file URI
  - `F6` - Multi-agent orchestration panel
  - `F9` - MCP server discovery panel
  - `F13` - Background agents list
  - `F14` - Spawn background agent

### 3.2 Slash Commands
- `/agents` - List loaded agents
- `/agent` - Open agent picker
- `/team create <name>` - Create team
- `/team open <name>` - Re-open existing team
- `/team close` - Close active team
- `/team delete <name>` - Delete team
- `/team clear` - Reset team state
- `/team tasks` - Show tabular task list
- `/team message <to> <msg>` - Send direct message
- `/mcp discover` - Scan for installed MCP servers
- `/opt <method> <prompt>` - Transform prompt into structured framework

### 3.3 CLI Commands
```bash
ragent                           # Launch TUI
ragent run "prompt"              # One-shot prompt
ragent serve --port 9100         # Start HTTP server only
ragent session list              # List all sessions
ragent session resume <id>       # Resume session in TUI
ragent session import <file>     # Import session from JSON
ragent session export <id>       # Export session to JSON
ragent auth <provider> <key>     # Store API key
ragent models                    # List available models
ragent models --ollama           # List Ollama models
ragent config                    # Show resolved configuration
```

### 3.4 Prompt Optimization
12 built-in prompt transformation methods:
- `co_star` - Context, Objective, Scope, Task, Action, Result
- `crispe` - Context, Role, Intent, Steps, Persona, Examples
- `cot` - Chain-of-Thought reasoning
- `draw` - Image generation prompts
- `rise` - Role, Intent, Scope, Examples
- `o1_style` - Stylized creative tokens
- `meta` - Meta prompting
- `variational` - VARI with multiple candidates
- `q_star` - Q* iterative refinement
- `openai` - OpenAI adapter
- `claude` - Anthropic Claude adapter
- `microsoft` - Microsoft Azure AI adapter

### 3.5 Skills System
- **Bundled Skills:** `/simplify`, `/debug`
- **Discovery:** `~/.ragent/skills/` and `.ragent/skills/`
- **Invocation:** `/skillname [args]` in TUI or via CLI
- **Context:** Skills receive working directory, session info, recent files
- **Custom Skills:** Shell scripts or executables with YAML metadata

---

## 4. Integration Points and APIs

### 4.1 LLM Providers
1. **Anthropic Claude:**
   - Models: claude-sonnet-4-20250514, claude-3-5-haiku-latest
   - Features: Native tool calling, streaming, image support
   - Auth: ANTHROPIC_API_KEY env var or stored key

2. **OpenAI:**
   - Models: gpt-4o, gpt-4o-mini, gpt-3.5-turbo, o1-preview, o1-mini
   - Features: Function calling, streaming, vision
   - Auth: OPENAI_API_KEY env var or stored key

3. **GitHub Copilot:**
   - Models: gpt-4o, gpt-4, gpt-3.5-turbo, claude-3.5-sonnet, o1-preview, o1-mini
   - Features: Auto-discovery of token, reasoning level selection
   - Auth: Auto-discovered from VS Code/JetBrains or GITHUB_COPILOT_TOKEN
   - Unique: Plan-based quota tracking, request cost multiplier display

4. **Ollama:**
   - Models: Any locally available model (llama3.2, codellama, etc.)
   - Features: Local inference, no API key required
   - Auth: None required
   - Config: OLLAMA_HOST env var or default localhost:11434

5. **Generic OpenAI:**
   - Compatible with any OpenAI-compatible API
   - Configurable base URL in ragent.json
   - Auth: GENERIC_OPENAI_API_KEY

### 4.2 HTTP/REST API
**Base URL:** `http://localhost:9100` (default)
**Authentication:** Bearer token (printed on server start)

**Endpoints:**
- `GET /config` - Get resolved configuration
- `GET /providers` - List available providers
- `GET /sessions` - List all sessions
- `POST /sessions` - Create new session
- `GET /sessions/{id}` - Get session details
- `DELETE /sessions/{id}` - Archive session
- `POST /sessions/{id}/message` - Send message
- `POST /sessions/{id}/abort` - Abort running session
- `POST /sessions/{id}/permission/{req_id}` - Reply to permission request
- `GET /sessions/{id}/tasks` - List sub-agent tasks
- `POST /sessions/{id}/tasks` - Spawn new task
- `GET /sessions/{id}/tasks/{task_id}` - Get task details
- `POST /sessions/{id}/tasks/{task_id}/cancel` - Cancel task
- `GET /events` - SSE stream for real-time events
- `POST /opt` - Prompt optimization endpoint
- Orchestration endpoints (M5): job management, metrics, leader election

### 4.3 LSP Integration
**Supported Languages:** Rust (rust-analyzer), extensible via config

**Configuration:**
```json
{
  "lsp": {
    "rust": {
      "command": "/path/to/rust-analyzer",
      "args": [],
      "extensions": ["rs"],
      "disabled": false
    }
  }
}
```

**Capabilities:**
- Hover for type info and docs
- Go-to-definition
- Find references
- List symbols (document outline)
- Diagnostics (errors, warnings, hints)
- Workspace folders support

### 4.4 MCP (Model Context Protocol)
**Status:** Stub implementation, in progress

**Features:**
- Auto-discovery of installed MCP servers
- Scans PATH, npm global packages, Claude Desktop, Cline configs
- Recognizes 18 known MCP servers
- TUI discovery panel (F9)

---

## 5. Strengths and Unique Selling Points

### 5.1 Technical Strengths
1. **Single Binary Distribution:** Zero runtime dependencies, statically linked
2. **Rust Performance:** Fast, memory-safe, concurrent by design
3. **Multi-Provider Flexibility:** Switch between Anthropic, OpenAI, Copilot, Ollama seamlessly
4. **Rich Tool Ecosystem:** 60+ built-in tools covering files, shell, web, office, LSP, teams
5. **Robust Permission System:** Fine-grained control with allow/deny rules and pattern matching
6. **Atomic Operations:** Multiedit, patch, and snapshot/undo prevent partial failures
7. **Event-Driven Architecture:** Real-time UI updates via event bus, SSE for HTTP clients
8. **File Locking:** Concurrent edits safe across multiple agents via file-level locks

### 5.2 User Experience Strengths
1. **Interactive TUI:** Full-screen terminal UI with mouse support, markdown rendering, streaming
2. **Session Persistence:** Resume conversations across sessions with full history
3. **Agent Teams:** Unique multi-agent coordination with shared tasks and peer messaging
4. **Custom Agents:** Easy agent definition via markdown profiles or OASF JSON
5. **Skills System:** Extensible agent capabilities via shell scripts with metadata
6. **Prompt Optimization:** Instant prompt transformation into 12 frameworks, no LLM call needed
7. **Image Support:** Attach screenshots and visuals via Alt+V
8. **Background Agents:** Spawn parallel sub-agents without blocking main session

### 5.3 Developer Strengths
1. **HTTP/REST API:** Drive agent from any client, not just TUI
2. **SSE Streaming:** Real-time event stream for custom frontends
3. **OpenCode Compatibility:** Config format compatible with opencode.json
4. **LSP Integration:** Code intelligence queries via standard protocol
5. **OASF Standard:** Custom agents follow open specification
6. **Comprehensive Testing:** 70+ integration tests across all crates
7. **Modular Architecture:** Clean crate boundaries, extensible provider/tool traits

### 5.4 Unique Features vs. Competitors
1. **Agent Teams:** No other terminal AI agent has coordinated multi-agent teams with peer messaging
2. **Rust Implementation:** Most competitors are Python/TypeScript, ragent is fast and memory-safe
3. **Copilot Integration:** Direct integration with GitHub Copilot, auto-discovers token
4. **Prompt Optimization:** Built-in prompt framework transformations without LLM calls
5. **Office Document Support:** Native Word/Excel/PowerPoint read/write, rare in AI agents
6. **Multi-Agent Orchestration:** Leader election, pluggable transport, conflict resolution policies
7. **Persistent Memory:** Agent-specific memory scopes (user-global, project-local)

---

## 6. Current Limitations and Gaps

### 6.1 Maturity Limitations
1. **Alpha Status:** v0.1.0-alpha.20, API instability expected
2. **Documentation:** Fragmented across README, QUICKSTART, docs/ folder
3. **Error Messages:** Some cryptic errors, especially in team coordination
4. **UX Polish:** TUI occasionally has rendering glitches or cursor positioning issues

### 6.2 Functional Gaps
1. **MCP Support:** Stub implementation, not fully functional yet
2. **Windows Support:** Primarily tested on Linux/macOS, Windows may have issues
3. **Context Window Management:** Basic compaction, could be smarter about pruning
4. **Model Switching:** Can't switch models mid-session without restart
5. **Undo/Redo:** Snapshot system exists but no interactive undo UI
6. **Search in Chat:** No search within session message history
7. **Multi-Project Support:** No workspace concept for multiple projects

### 6.3 Integration Gaps
1. **IDE Integration:** No VS Code extension, JetBrains plugin, or editor integrations
2. **Git Integration:** No direct git operations, relies on bash tool
3. **Testing Framework:** No built-in test execution or test result parsing
4. **Debugging Tools:** No interactive debugger integration
5. **Database Tools:** No native SQL query tools (relies on bash)
6. **Cloud Services:** No AWS/GCP/Azure CLI integrations

### 6.4 Performance Limitations
1. **Large Files:** Reading files >10K lines can be slow, no streaming read
2. **Concurrent Agents:** No hard limit, but performance degrades with many active teammates
3. **Database Locks:** SQLite can bottleneck under high concurrent load
4. **Memory Usage:** Each agent/teammate holds full context, memory scales linearly

### 6.5 Security Considerations
1. **Permission System:** Works but could be more granular (no per-directory quotas)
2. **API Key Storage:** Keys stored in SQLite database, not OS keychain
3. **Shell Command Execution:** bash tool is powerful but risky, needs sandboxing options
4. **Network Access:** webfetch and websearch have no rate limiting or domain blocking
5. **File Access:** No chroot or containerization for file operations

### 6.6 Known Bugs/Issues
1. **Team Cleanup:** Force cleanup can leave orphaned processes
2. **LSP Timeout:** Some LSP servers don't respond to shutdown gracefully
3. **Clipboard:** Image paste doesn't work on all terminal emulators
4. **Markdown Rendering:** Some table rendering edge cases in TUI
5. **Context Compaction:** Aggressive compaction can lose important context

---

## 7. Architecture Insights

### 7.1 Design Patterns
1. **Event-Driven:** Event bus for decoupled communication between components
2. **Trait-Based Polymorphism:** Provider, Tool, LlmClient all use trait objects
3. **Registry Pattern:** ProviderRegistry, ToolRegistry for dynamic dispatch
4. **Builder Pattern:** Agent and tool configuration use builders
5. **Repository Pattern:** Storage layer abstracts SQLite details
6. **Pub/Sub:** Event bus implements observer pattern for UI updates

### 7.2 Concurrency Model
1. **Tokio Runtime:** Full async/await throughout core logic
2. **Arc/Mutex:** Shared state wrapped in Arc, protected with async-aware locks
3. **Channels:** mpsc/broadcast channels for task communication
4. **File Locks:** fs2 crate for advisory file locking on shared resources
5. **Database:** Single SQLite connection, no connection pooling (potential bottleneck)

### 7.3 Error Handling
1. **anyhow::Result:** Used for application-level errors with context
2. **thiserror:** Custom error types for domain-specific failures
3. **No Panics:** Strict linting against unwrap/expect in production code
4. **Error Propagation:** `?` operator throughout, errors bubble to session processor

### 7.4 Testing Strategy
1. **Integration Tests:** 70+ tests in crates/*/tests/
2. **Unit Tests:** Inline tests relocated to tests/ folders per AGENTS.md
3. **Fixtures:** Test fixtures in tests/fixtures/
4. **Benchmarks:** Criterion benchmarks in crates/*/benches/
5. **Prompts:** Test prompts in tests/prompts/

---

## 8. Competitive Positioning

### 8.1 Similar Products
1. **Aider** - Python, git-focused, simpler architecture
2. **Cursor** - Commercial IDE, proprietary
3. **GitHub Copilot CLI** - Chat interface for git/terminal
4. **OpenCode** - TypeScript, inspiration for ragent
5. **Continue.dev** - VS Code extension, IDE-centric
6. **Claude Code** - Web-based, Anthropic's official tool

### 8.2 ragent Differentiators
1. **Rust Performance:** 10-100x faster startup and tool execution than Python/JS
2. **Agent Teams:** Unique parallel multi-agent coordination
3. **Terminal Native:** Not an IDE plugin, works anywhere
4. **Multi-Provider:** Easy switching between 5 providers
5. **Office Docs:** Native document processing, rare in competitors
6. **Open Source:** MIT licensed, full transparency
7. **Single Binary:** No Python/Node.js runtime required

---

## 9. Development Activity

### 9.1 Recent Changes (Last 3 Versions)
**v0.1.0-alpha.20 (Current):**
- Input improvements
- Generic OpenAI provider support

**v0.1.0-alpha.19:**
- Teams UX enhancements (open/close/delete/clear)
- Context compaction near window limits
- Copilot reasoning level selection
- Model cost multiplier display

**v0.1.0-alpha.18:**
- Custom agents support (profiles + OASF)
- Agent teams coordination APIs
- Task claiming and messaging tools

### 9.2 Development Velocity
- **Rapid Evolution:** 20 alpha releases in short timeframe
- **Feature-Driven:** Each release adds significant new capabilities
- **Breaking Changes:** Alpha status means API instability
- **Documentation Lag:** Features added faster than docs updated

---

## 10. Dependencies and Tech Debt

### 10.1 Key Dependencies
- **tokio:** Async runtime (37 transitive deps)
- **ratatui:** TUI framework (10 deps)
- **axum:** HTTP server (25 deps)
- **rusqlite:** SQLite (bundled, minimal deps)
- **reqwest:** HTTP client (30 deps with rustls)
- **tracing:** Logging (15 deps)

**Total Dependency Count:** ~200 crates in full tree

### 10.2 Tech Debt Areas
1. **TODO Comments:** Scattered throughout codebase for API improvements
2. **Type Variance:** Some `serde_json::Value` usage instead of typed structs
3. **Error Handling:** Some error messages lack context
4. **Test Coverage:** Uneven, some modules under-tested
5. **Documentation:** DOCBLOCK comments incomplete in places
6. **Code Duplication:** Some repeated patterns in tool implementations

---

## Conclusion

ragent is a sophisticated, high-performance AI coding agent that stands out for its Rust implementation, multi-agent team coordination, and rich tool ecosystem. It's in early alpha but demonstrates strong architectural foundations and unique capabilities compared to competitors. The main challenges are maturity (documentation, UX polish, bug fixes) and completing incomplete features (MCP, IDE integrations). The agent teams capability is a significant innovation not seen in competing products.

**Primary Strengths:**
- Performance and reliability (Rust)
- Unique agent teams feature
- Rich tool set (60+ tools)
- Multi-provider flexibility
- Strong foundation for extensibility

**Primary Gaps:**
- Documentation fragmentation
- Alpha-level stability
- MCP incomplete
- Limited IDE integration
- Some UX rough edges

**Recommended Next Steps:**
1. Stabilize core APIs and release beta
2. Complete MCP implementation
3. Consolidate documentation
4. Build IDE extensions (VS Code, JetBrains)
5. Add workspace/multi-project support
6. Enhance context window management
7. Improve error messages and debugging tools

