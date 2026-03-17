# ragent — Implementation Status

**Status as of: 2025 Development Cycle**

This document tracks the implementation status of all features described in SPEC.md. Each feature is marked with a status indicator:
- ✅ **Implemented** — Feature is complete and tested
- ⚠️ **Partial** — Feature is partially implemented or in progress
- 🔲 **Planned** — Feature is not yet started but planned
- ❌ **Not Started** — Feature not yet implemented

---

## Table of Contents

1. [Core Modules Status](#core-modules-status)
2. [Detailed Feature Implementation](#detailed-feature-implementation)
3. [Tools Implementation Status](#tools-implementation-status)
4. [Provider Support Status](#provider-support-status)
5. [Summary Statistics](#summary-statistics)

---

## Core Modules Status

### 3.1 CLI & Entry Point ✅

| Feature | Status | Notes |
|---------|--------|-------|
| Default interactive TUI launch | ✅ | Working, transitions to chat on first message |
| `run <prompt>` subcommand | ✅ | One-shot execution, prints result and exits |
| `serve` subcommand | ✅ | Headless HTTP/WebSocket server mode |
| `session list` | ✅ | Lists saved sessions with metadata |
| `session resume <id>` | ✅ | Resumes specific session, restores history and directory |
| `session export <id>` | ✅ | Exports session to JSON |
| `session import <file>` | ✅ | Imports session from JSON file |
| `auth <provider>` | ✅ | Configures API keys for providers |
| `models` | ✅ | Lists available models across providers |
| `config` | ✅ | Prints resolved configuration |
| `mcp list` | ❌ | Not implemented |
| `upgrade` | ❌ | Self-update binary feature not implemented |
| `uninstall` | ❌ | Uninstall feature not implemented |
| `--config <path>` | ✅ | Config file path override |
| `--model <provider/model>` | ✅ | Model override for run |
| `--agent <name>` | ✅ | Agent selection override |
| `-p`, `--prompt <text>` | ❌ | Single prompt execution not fully implemented |
| `--log-level <level>` | ✅ | Logging verbosity control |
| `--print-logs` | ❌ | Print logs to stderr not implemented |
| `--no-tui` | ✅ | Headless mode with plain stdout |
| `--yes` | ✅ | Auto-approve all permission prompts |
| `--allow-all-tools` | ❌ | Not implemented |
| `--allow-tool <spec>` | ❌ | Not implemented |
| `--deny-tool <spec>` | ❌ | Not implemented |
| `--server <addr>` | ❌ | Connect to existing ragent server not implemented |
| `--continue` | ❌ | Resume most recent session flag not implemented |
| `--resume` | ❌ | Session picker not implemented |
| `--from-pr <number>` | ❌ | GitHub PR linking not implemented |
| `--worktree <name>` | ❌ | Git worktree isolation not implemented |
| `--permission-mode <mode>` | ❌ | Permission mode selection not implemented |
| `--output-format <fmt>` | ❌ | JSON/stream-json output formats not implemented |

---

### 3.2 Configuration ✅

| Feature | Status | Notes |
|---------|--------|-------|
| `ragent.json` file format | ✅ | Fully supported with comments |
| `ragent.jsonc` format | ✅ | JSON with comments supported |
| OpenCode config compatibility | ✅ | Reads `opencode.json`/`opencode.jsonc` |
| Configuration merging | ✅ | Deep-merge semantics implemented |
| `username` field | ✅ | Display name configuration |
| `default_agent` field | ✅ | Default agent selection |
| `provider` configuration | ✅ | Provider definitions with model lists |
| `permission` ruleset | ✅ | Global permission rules |
| `agent` definitions | ✅ | Custom agent configuration |
| `command` definitions | ✅ | Custom slash-commands |
| `mcp` server configuration | ✅ | MCP server definitions |
| `instructions` field | ✅ | System instructions from paths/inline |
| `experimental` flags | ✅ | Feature flag support |
| Load precedence | ✅ | Correct priority order implemented |
| `$XDG_CONFIG_HOME/ragent/ragent.json` | ✅ | Global config location |
| `$RAGENT_CONFIG` env var | ✅ | Custom config path via environment |
| `.ragent/` directory loading | ⚠️ | Partial — directory exists but not all subdirs loaded |
| `$RAGENT_CONFIG_CONTENT` env var | ❌ | Inline JSON string not supported |

---

### 3.3 Provider System ⚠️

| Provider | Status | Notes |
|----------|--------|-------|
| Anthropic | ✅ | Fully implemented with native API |
| OpenAI | ✅ | Chat Completions API fully working |
| GitHub Copilot | ✅ | Device flow + auto-discovery implemented |
| Ollama | ✅ | OpenAI-compatible with model discovery |
| Google Generative AI | 🔲 | Planned, not started |
| Azure OpenAI | 🔲 | Planned, not started |
| AWS Bedrock | 🔲 | Planned, not started |
| OpenRouter | 🔲 | Planned, not started |
| XAI | 🔲 | Planned, not started |
| Mistral | 🔲 | Planned, not started |
| Groq | 🔲 | Planned, not started |
| Custom OpenAI-compatible | 🔲 | Planned, not started |

**Provider Features:**
| Feature | Status | Notes |
|---------|--------|-------|
| Model descriptor struct | ✅ | ModelInfo with cost, capabilities, context window |
| Streaming interface (LlmStream trait) | ✅ | Async streaming with events |
| StreamEvent enum | ✅ | Text, reasoning, tool calls, usage, errors |
| Provider health checking | ✅ | Background health checks with visual indicators |
| Token cost tracking | ⚠️ | Partial — tracked but not all providers have costs |
| Model discovery | ✅ | Available for Anthropic, OpenAI, Ollama, Copilot |

---

### 3.4 Agent System ⚠️

**Built-in Agents:**

| Agent | Status | Notes |
|-------|--------|-------|
| `ask` | ✅ | Read-only Q&A agent implemented |
| `general` | ✅ | General-purpose with full access |
| `build` | ✅ | Build/test agent, max 30 steps |
| `plan` | ✅ | Read-only planning agent |
| `explore` | ✅ | Fast codebase search agent |
| `title` | ✅ | Session title generation |
| `summary` | ✅ | Session summary generation |
| `compaction` | ✅ | History compression agent |
| `orchestrator` | ❌ | Task orchestrator not implemented |
| `debug` | ❌ | Systematic debugger not implemented |

**Agent Features:**

| Feature | Status | Notes |
|---------|--------|-------|
| Agent definition struct (AgentInfo) | ✅ | Complete implementation |
| Agent mode (Primary/Subagent/All) | ✅ | Mode system working |
| Agent temperature/top_p | ✅ | Sampling parameters supported |
| Agent model override | ✅ | Per-agent model selection |
| Agent prompt override | ✅ | Custom system prompts |
| Agent permissions | ✅ | Agent-specific permission rules |
| Tool groups | ✅ | Tool access control groups |
| Max steps | ✅ | Loop iteration limit |
| Tool group restrictions | ✅ | `read`, `edit`, `command`, `search`, `mcp`, `web`, `workflow` |
| Agent resolution/merging | ✅ | Proper priority order for config sources |
| Agent switching (Tab/Shift+Tab) | ✅ | TUI agent cycling |
| Agent switching via slash command | ✅ | `/agent` command implemented |
| AGENTS.md auto-loading | ✅ | Project guidelines auto-injection |
| Init exchange for guidelines | ✅ | Acknowledgement prompt shown to user |

---

### 3.5 Session Management ✅

| Feature | Status | Notes |
|---------|--------|-------|
| Session creation | ✅ | ULID generation, metadata setup |
| Session persistence (SQLite) | ✅ | Full storage and retrieval |
| Session title | ✅ | Auto-generated or user-set |
| Session directory | ✅ | Working directory tracking |
| Session metadata | ✅ | Created_at, updated_at, archived_at |
| Session summary | ✅ | Statistics tracking (additions, deletions, files_changed) |
| Session resume | ✅ | Load by ID with full context restoration |
| Session export (JSON) | ✅ | Serialize to JSON with new ULIDs |
| Session import (JSON) | ✅ | Deserialize from JSON file |
| Session archiving | ✅ | Soft delete functionality |
| Message history | ✅ | Full conversation persistence |
| Parent session tracking | ✅ | For sub-agent sessions |

---

### 3.6 Message Model ✅

| Feature | Status | Notes |
|---------|--------|-------|
| Message struct | ✅ | ULID ID, role, parts, timestamps |
| Message parts (Text) | ✅ | Text message parts |
| Message parts (ToolCall) | ✅ | Tool invocation with call_id and state |
| Message parts (Reasoning) | ✅ | Extended thinking text support |
| ToolCallState | ✅ | Status, input, output, error, duration tracking |
| Message serialization | ✅ | JSON storage in database |
| Message history reconstruction | ✅ | Proper message ordering and persistence |

---

### 3.7 Tool System ✅

**Total Tools Implemented: 26**

#### File Tools

| Tool | Status | Notes |
|------|--------|-------|
| `read` | ✅ | Read with optional line range |
| `write` | ✅ | Create/overwrite files |
| `create` | ✅ | Create new file, truncate if exists |
| `edit` | ✅ | Single atomic string replacement |
| `multiedit` | ✅ | Multiple atomic edits per file |
| `patch` | ✅ | Unified diff application with fuzzy matching |
| `rm` | ✅ | Single file deletion (no wildcards) |

#### Search Tools

| Tool | Status | Notes |
|------|--------|-------|
| `grep` | ✅ | Regex pattern search with context lines |
| `glob` | ✅ | Recursive file pattern matching |

#### Directory Tools

| Tool | Status | Notes |
|------|--------|-------|
| `list` | ✅ | Tree-like directory listing with depth |

#### Shell Tools

| Tool | Status | Notes |
|------|--------|-------|
| `bash` | ✅ | Command execution with timeout and signal handling |

#### Document Tools

| Tool | Status | Notes |
|------|--------|-------|
| `office_read` | ✅ | Word/Excel/PowerPoint content reading |
| `office_write` | ✅ | Document creation/modification |
| `office_info` | ✅ | Document metadata extraction |
| `pdf_read` | ✅ | PDF text and metadata extraction |
| `pdf_write` | ✅ | PDF creation with text, tables, images |

#### Web Tools

| Tool | Status | Notes |
|------|--------|-------|
| `webfetch` | ✅ | HTTP GET with HTML-to-text conversion |
| `websearch` | ✅ | Tavily API-based search |

#### Interactive Tools

| Tool | Status | Notes |
|------|--------|-------|
| `question` | ✅ | Prompt user for input |

#### Task Management Tools

| Tool | Status | Notes |
|------|--------|-------|
| `todo_read` | ✅ | Read session TODO list with filtering |
| `todo_write` | ✅ | Add/update/remove/clear TODO items |

#### Agent Delegation Tools

| Tool | Status | Notes |
|------|--------|-------|
| `plan_enter` | ✅ | Switch to plan agent with task context |
| `plan_exit` | ✅ | Return from plan agent with summary |

#### Planned Tools

| Tool | Status | Notes |
|------|--------|-------|
| `new_task` | 🔲 | Subtask delegation not implemented |
| `switch_agent` | 🔲 | Agent switching tool not implemented |
| `codebase_search` | ❌ | Semantic search via embeddings not implemented |
| `generate_image` | 🔲 | Image generation not implemented |

---

### 3.8 Permission System ⚠️

| Feature | Status | Notes |
|---------|--------|-------|
| Permission rules structure | ✅ | Allow/Deny/Ask actions |
| Permission categories | ✅ | read, edit, bash, web, external_directory, doom_loop |
| Rule evaluation order | ✅ | Agent → Project → Global → Built-in defaults |
| Permission check on tool invocation | ✅ | Evaluated before execution |
| Ask flow (interactive) | ✅ | Permission dialog with once/always/deny options |
| Permission modes | ⚠️ | Partial — `default` and `plan` work; others need flags |
| Sandbox configuration | ❌ | Sandbox settings not implemented |
| Path-based permissions | ✅ | Glob patterns for file access control |
| Tool-specific permissions | ✅ | Categories per tool type |
| Memory of permission decisions | ✅ | Session-local rule recording |
| External directory permission | ✅ | Enforced at execution time |

---

### 3.9 HTTP Server ✅

| Feature | Status | Notes |
|---------|--------|-------|
| HTTP server framework (axum) | ✅ | REST + SSE API |
| TCP/Unix socket transport | ✅ | Both supported |
| Basic authentication | ⚠️ | Partial support |
| OpenAPI spec generation | ❌ | Not auto-generated |
| `/health` endpoint | ✅ | Health check |
| `/config` endpoint | ✅ | GET/PUT config |
| `/providers` endpoint | ✅ | List providers and models |
| `/auth/:provider` endpoint | ✅ | Set API key |
| `/sessions` endpoint | ✅ | CRUD operations |
| `/sessions/:id/messages` endpoint | ✅ | Message history and submission |
| `/sessions/:id/abort` endpoint | ✅ | Abort running session |
| `/sessions/:id/permission/:req_id` endpoint | ✅ | Permission replies |
| `/mcp` endpoint | ✅ | MCP server listing |
| `/mcp/:id/restart` endpoint | ✅ | MCP server restart |
| `/events` endpoint | ✅ | Global SSE event stream |
| SSE events | ✅ | All event types properly streamed |
| Error handling | ✅ | Proper HTTP error responses |

---

### 3.10 Terminal UI (TUI) ✅

#### Core Features

| Feature | Status | Notes |
|---------|--------|-------|
| Home screen with ASCII logo | ✅ | Centered landing page |
| Auto-expanding input | ✅ | Multi-line input that grows with text |
| Chat screen layout | ✅ | Full message and input layout |
| Message rendering | ✅ | Text, tool calls, reasoning blocks |
| Scrollable message pane | ✅ | Full conversation history visible |
| Tool call display | ✅ | Compact format with status indicators |
| Permission dialogs | ✅ | Modal prompts for approval |
| Input area | ✅ | Multi-line with history navigation |
| Status bar | ✅ | Shows session, agent, provider, model |
| Log panel | ✅ | Timestamped event logging |

#### Slash Commands

| Command | Status | Notes |
|---------|--------|-------|
| `/about` | ✅ | Application info |
| `/agent [name]` | ✅ | Agent selection |
| `/checkpoint` | 🔲 | Not implemented |
| `/clear` | ✅ | Clear message history |
| `/compact` | ✅ | Manual context compaction |
| `/context` | 🔲 | Token usage breakdown not shown |
| `/help` | ✅ | Command list and descriptions |
| `/log` | ✅ | Toggle log panel |
| `/model` | ✅ | Model selection |
| `/provider` | ✅ | Provider setup dialog |
| `/provider_reset` | ✅ | Clear provider credentials |
| `/quit` | ✅ | Exit application |
| `/system [prompt]` | ✅ | Override system prompt |
| `/todo` | ✅ | Display todo list |
| `/tools` | ✅ | List available tools |

#### Key Bindings

| Key | Status | Notes |
|-----|--------|-------|
| `Enter` | ✅ | Send message |
| `Shift+Enter` | ✅ | Newline in message |
| `Tab` / `Shift+Tab` | ✅ | Cycle between agents |
| `Ctrl+C` | ✅ | Abort/exit |
| `Ctrl+L` | ✅ | Clear screen |
| `Esc` | ✅ | Cancel input/close dialog |
| `Up/Down` | ✅ | Input history navigation |
| `PageUp/PageDown` | ✅ | Message pane scroll |
| `Ctrl+PageUp/PageDown` | ✅ | Log panel scroll |
| `@` | ✅ | Sub-agent invocation |
| `/` | ✅ | Slash command with autocomplete |
| `y/a/n` | ✅ | Permission dialog responses |

#### Mouse Support

| Feature | Status | Notes |
|---------|--------|-------|
| Scroll wheel | ✅ | Scrolls active pane |
| Scrollbar drag | ✅ | Click-drag scrolling |
| Text selection | ✅ | Click-drag to select |
| Right-click copy | ✅ | Copy selection to clipboard |

#### Additional Features

| Feature | Status | Notes |
|---------|--------|-------|
| Provider health indicator | ✅ | Visual status before provider name |
| Provider setup dialog | ✅ | Interactive flow with key entry |
| Session resume | ✅ | Loads previous session state |
| Auto-expanding input | ✅ | Grows as user types |
| Message enqueueing | ✅ | Queue messages during agent processing |
| Scrollbars | ✅ | Visible in messages and log panes |
| Context compaction trigger | ✅ | Auto-compacts at 95% of context window |
| Inline permission feedback | ✅ | User can explain denial reasons |

---

### 3.11 MCP Client ✅

| Feature | Status | Notes |
|---------|--------|-------|
| MCP server configuration | ✅ | stdio, SSE, HTTP transports |
| Server lifecycle management | ✅ | Spawn, initialize, list tools, execute, reconnect, shutdown |
| Tool discovery | ✅ | Lists tools from MCP servers |
| Tool execution | ✅ | Proxy calls to correct server |
| Timeout handling | ✅ | Configurable per-call timeout |
| Status tracking | ✅ | Connected/Disabled/Failed/NeedsAuth states |
| Concurrent access | ✅ | Arc<RwLock<HashMap>> for safe sharing |
| Permission integration | ✅ | MCP tools subject to permission rules |
| Auto-reconnection | ✅ | Retry on transient failures |

---

### 3.12 LSP Integration ❌

| Feature | Status | Notes |
|---------|--------|-------|
| Language server support | ❌ | Not implemented |
| Rust (rust-analyzer) | ❌ | Not integrated |
| TypeScript (typescript-language-server) | ❌ | Not integrated |
| Python (pylsp/pyright) | ❌ | Not integrated |
| Go (gopls) | ❌ | Not integrated |
| C/C++ (clangd) | ❌ | Not integrated |
| Diagnostics integration | ❌ | Not implemented |
| Definition navigation | ❌ | Not implemented |
| References finding | ❌ | Not implemented |
| Hover information | ❌ | Not implemented |
| LSP tool invocation | ❌ | Not implemented |

---

### 3.13 Event Bus ✅

| Feature | Status | Notes |
|---------|--------|-------|
| Event enum | ✅ | All event types defined |
| Broadcast channel (tokio) | ✅ | tokio::sync::broadcast implementation |
| Session events | ✅ | Created, Updated, Aborted |
| Message events | ✅ | Start, TextDelta, ReasoningDelta, ToolCall, End |
| Permission events | ✅ | Requested, Replied |
| Agent events | ✅ | Switched, Error |
| MCP events | ✅ | StatusChanged |
| Token usage events | ✅ | Tracked and emitted |
| Event subscription | �� | Multiple independent consumers |
| Event publishing | ✅ | Throughout agent loop and tool execution |

---

### 3.14 Storage & Database ✅

| Feature | Status | Notes |
|---------|--------|-------|
| SQLite engine | ✅ | rusqlite with bundled library |
| Database location | ✅ | `$XDG_DATA_HOME/ragent/ragent.db` |
| Schema migrations | ✅ | Embedded SQL run at startup |
| Sessions table | ✅ | Complete schema |
| Messages table | ✅ | With JSON parts storage |
| Provider auth table | ✅ | Stores API keys (not encrypted) |
| MCP servers table | ✅ | Server configuration storage |
| Snapshots table | ✅ | File snapshot storage with compression |
| Todos table | ✅ | Task/todo item storage |
| Index optimization | ✅ | Created on messages(session_id, created_at) |

---

### 3.15 Shell Execution ✅

| Feature | Status | Notes |
|---------|--------|-------|
| Bash command execution | ✅ | Via tokio::process::Command |
| Async execution | ✅ | Non-blocking with timeout |
| Timeout handling | ✅ | Configurable (default 120s) |
| Signal handling | ✅ | kill_on_drop(true) for cleanup |
| Output capture | ✅ | stdout + stderr combined |
| Working directory | ✅ | Locked to session root by default |
| Environment variables | ✅ | RAGENT and RAGENT_SESSION_ID set |
| Sanitization | ✅ | Secrets not forwarded |
| Output truncation | ✅ | Truncated if too large |
| External directory permission | ✅ | Enforced before execution |

---

### 3.16 Snapshot & Undo ✅

| Feature | Status | Notes |
|---------|--------|-------|
| Pre-execution snapshot | ✅ | Captured before edit/write/patch |
| Snapshot storage | ✅ | Compressed in SQLite |
| Snapshot association | ✅ | Linked to message_id |
| File compression | ✅ | Zstd or flate2 compression |
| Undo by message | ✅ | Restore single message changes |
| Shadow git repository | ⚠️ | Infrastructure in place, not fully tested |
| Checkpoint creation | ⚠️ | Partial implementation |
| Checkpoint diff | 🔲 | `/checkpoint diff` not implemented |
| Checkpoint restore | 🔲 | `/checkpoint restore` not implemented |
| .ragentignore integration | ⚠️ | Pattern matching but enforcement incomplete |

---

### 3.17 Hooks ❌

All hook events and types are planned but not implemented:

| Hook Type | Status |
|-----------|--------|
| PreToolUse | ❌ |
| PostToolUse | ❌ |
| PostToolUseFailure | ❌ |
| PreMessage | ❌ |
| PostMessage | ❌ |
| UserPromptSubmit | ❌ |
| SessionStart | ❌ |
| SessionEnd | ❌ |
| PreCompact | ❌ |
| PermissionRequest | ❌ |
| SubagentStart | ❌ |
| SubagentStop | ❌ |
| Notification | ❌ |
| WorktreeCreate | ❌ |
| WorktreeRemove | ❌ |
| InstructionsLoaded | ❌ |
| TaskCompleted | ❌ |

---

### 3.18 Custom Agents ⚠️

| Feature | Status | Notes |
|---------|--------|-------|
| Custom agent struct | ✅ | AgentConfig supports all fields |
| Markdown format (`.ragent/agents/`) | 🔲 | Directory not loaded |
| JSON format in config | ✅ | Fully supported in ragent.json |
| Agent name and description | ✅ | Configurable |
| Model override | ✅ | Per-agent model selection |
| Tool restrictions | ✅ | Allowed tools configurable |
| Permission scoping | ✅ | Agent-specific permission rules |
| Max turns | ✅ | max_steps configurable |
| Memory scope | ❌ | Not implemented |
| Skills loading | ❌ | Skills not implemented |
| Isolation modes | ❌ | Worktree/container isolation not implemented |
| Background execution | ❌ | Not implemented |
| Hooks in agents | ❌ | Not implemented |

---

### 3.19 Skills ❌

All skill features are not implemented:

| Feature | Status | Notes |
|---------|--------|-------|
| Skill structure (`.ragent/skills/`) | ❌ | Not implemented |
| Skill markdown format | ❌ | Not parsed |
| YAML frontmatter | ❌ | Not processed |
| Skill arguments | ❌ | Not supported |
| Dynamic context injection | ❌ | Not supported |
| Subagent execution | ❌ | Not implemented |
| Skill scopes | ❌ | Not implemented |
| Skill marketplace | ❌ | Not implemented |
| Built-in skills | ❌ | Not implemented |

---

### 3.20 Persistent Memory ❌

All memory features are not implemented:

| Feature | Status | Notes |
|---------|--------|-------|
| Memory database table | ❌ | Not implemented |
| Memory categories | ❌ | Not supported |
| Auto-memory files | ❌ | Not supported |
| MEMORY.md loading | ❌ | Not implemented |
| Topic files | ❌ | Not implemented |
| Memory write tool | ❌ | Not implemented |
| Memory management UI | ❌ | Not implemented |

---

### 3.21 Trusted Directories ❌

| Feature | Status | Notes |
|---------|--------|-------|
| Trust prompt on first launch | ❌ | Not implemented |
| Trusted directory tracking | ❌ | Not stored |
| External directory enforcement | ⚠️ | Partial — permission check exists but no trust mechanism |
| Configuration | ❌ | Config option not used |
| Permission integration | ⚠️ | `external_directory` permission exists but trust not enforced |

---

### 3.22 Codebase Indexing & Semantic Search ❌

| Feature | Status | Notes |
|---------|--------|-------|
| Code parsing (Tree-sitter) | ❌ | Not implemented |
| Embedding generation | ❌ | Not implemented |
| Vector storage | ❌ | Not implemented |
| Semantic search tool | ❌ | Not implemented |
| Incremental indexing | ❌ | Not implemented |
| File watching | ❌ | Not implemented |
| Branch awareness | ❌ | Not implemented |
| `.gitignore` awareness | ❌ | Not implemented |

---

### 3.23 Post-Edit Diagnostics ❌

| Feature | Status | Notes |
|---------|--------|-------|
| LSP diagnostic collection | ❌ | Requires LSP integration first |
| Post-edit delay | ❌ | Not implemented |
| Error detection | ❌ | Not implemented |
| Error injection | ❌ | Not implemented |
| Configuration | ❌ | Not configurable |

---

### 3.24 Task Todo List ✅

| Feature | Status | Notes |
|---------|--------|-------|
| Todo item creation | ✅ | Via `todo_write` tool |
| Todo item status | ✅ | pending, in_progress, done, blocked |
| Todo persistence | ✅ | SQLite storage per session |
| Todo display in TUI | ✅ | Status bar summary |
| `/todo` slash command | ✅ | Full list display |
| `todo_read` tool | ✅ | Read with status filtering |
| `todo_write` tool | ✅ | Add/update/remove/clear operations |
| Todo editing in TUI | ⚠️ | Display only, not editable in UI yet |
| Progress indicator | ✅ | Shown in status bar |

---

### 3.25 Prompt Enhancement ❌

| Feature | Status | Notes |
|---------|--------|-------|
| Prompt enhancement feature | ❌ | Not implemented |
| Enhancement UI trigger | ❌ | Not in TUI |
| Context-aware enhancement | ❌ | Not implemented |
| Undo for enhancement | ❌ | Not implemented |
| Configuration | ❌ | Not configurable |

---

### 3.26 Hierarchical Custom Instructions ⚠️

| Feature | Status | Notes |
|---------|--------|-------|
| `~/.config/ragent/rules/` | 🔲 | Directory structure exists but not loaded |
| `.ragent/rules/` | 🔲 | Directory structure exists but not loaded |
| Agent-specific rules | ✅ | Via agent-specific config works |
| Path-specific rules | ❌ | Not implemented |
| File imports (@filename) | ❌ | Not implemented |
| AGENTS.md auto-loading | ✅ | Project guidelines auto-injected |
| Init exchange | ✅ | Acknowledgement prompt shown |
| Monorepo filtering | ❌ | Not implemented |
| System prompt assembly order | ✅ | Proper precedence implemented for available features |

---

### 3.27 File Ignore Patterns ❌

| Feature | Status | Notes |
|---------|--------|-------|
| `.ragentignore` file parsing | ❌ | Not implemented |
| Glob pattern support | ❌ | Not implemented |
| Negation patterns | ❌ | Not implemented |
| File enforcement | ❌ | Not enforced in tools |
| Tool integration | ❌ | read/write/bash not filtering by .ragentignore |
| Codebase search | ❌ | Not enforced (feature also not implemented) |

---

### 3.28 Suggested Responses ❌

| Feature | Status | Notes |
|---------|--------|-------|
| Suggestion generation | ❌ | Not implemented |
| TUI display | ❌ | Not shown in UI |
| User selection | ❌ | Not interactive |
| Edit before sending | ❌ | Not supported |
| Configuration | ❌ | Not configurable |

---

### 3.29 Session Resume & Management ⚠️

| Feature | Status | Notes |
|---------|--------|-------|
| `--continue` flag | ❌ | Not implemented |
| `--resume` flag | ❌ | Not implemented |
| `--from-pr` flag | ❌ | Not implemented |
| `ragent session resume <id>` | ✅ | Fully working |
| Session picker | 🔲 | Not implemented |
| Session search/filter | 🔲 | Not implemented |
| Session grouping | 🔲 | Not implemented |
| `/resume` slash command | 🔲 | Not implemented |
| Session naming | 🔲 | Not implemented |
| `/name` slash command | 🔲 | Not implemented |
| PR linking | ❌ | Not implemented |
| Auto-title generation | ✅ | Via `title` agent |

---

### 3.30 Git Worktree Isolation ❌

| Feature | Status | Notes |
|---------|--------|-------|
| `--worktree <name>` flag | ❌ | Not implemented |
| Worktree creation | ❌ | Not implemented |
| Worktree cleanup | ❌ | Not implemented |
| Worktree prompt | ❌ | Not implemented |
| Agent worktree isolation | ❌ | Not implemented |
| Hooks for worktree events | ❌ | Not implemented |

---

### 3.31 Context Compaction ⚠️

| Feature | Status | Notes |
|---------|--------|-------|
| Automatic compaction | ✅ | Triggers at 95% context usage |
| Manual compaction | ✅ | `/compact` slash command |
| Compaction agent | ✅ | Dedicated `compaction` agent |
| Custom prompt | 🔲 | `/compact <instructions>` not fully supported |
| Rules re-injection | ✅ | AGENTS.md reloaded after compaction |
| Memory re-loading | ⚠️ | Partial — MEMORY.md concept exists but not fully implemented |
| Configuration | ✅ | `compaction.threshold` configurable |
| Threshold | ✅ | Configurable percentage of context window |
| Preserve recent messages | ✅ | Last N message pairs preserved |

---

### 3.32 Headless / Pipe Mode ⚠️

| Feature | Status | Notes |
|---------|--------|-------|
| `--no-tui` mode | ✅ | Works for plain stdout output |
| `-p` / `--prompt` flag | ⚠️ | Partially working in `run` command |
| Stdin piping | ❌ | Not supported |
| `--output-format text` | ✅ | Default output |
| `--output-format json` | ❌ | Not implemented |
| `--output-format stream-json` | ❌ | Not implemented |
| Auto-approve for CI | ✅ | `--yes` flag works |
| CI integration examples | 🔲 | Not documented/tested |

---

### 3.33 Extended Thinking & Effort Levels ✅

| Feature | Status | Notes |
|---------|--------|-------|
| Extended thinking support | ✅ | Reasoning blocks in messages |
| Thinking text display | ✅ | Shown in chat window (collapsible) |
| Effort levels | ✅ | low/medium/high configurable |
| Max thinking tokens | ✅ | Per-model limits respected |
| `RAGENT_THINKING_EFFORT` env | ✅ | Environment variable override |
| `MAX_THINKING_TOKENS` env | ✅ | Budget override |
| Per-agent override | ✅ | Agent-specific thinking config |
| TUI toggle (Ctrl+O) | ✅ | Show/hide thinking blocks |
| Ultrathink keyword | ✅ | Prompt modifier for maximum reasoning |

---

### 3.34 @ File References ❌

| Feature | Status | Notes |
|---------|--------|-------|
| `@filename` syntax | ❌ | Not implemented |
| `@path/to/file` syntax | ❌ | Not implemented |
| `@path/to/dir/` syntax | ❌ | Not implemented |
| `@url` syntax | ❌ | Not implemented |
| Fuzzy matching | ❌ | Not implemented |
| TUI autocomplete | ❌ | Not implemented |
| Inline file inclusion | ❌ | Not implemented |

---

### 3.35 Sub-agents & Background Agents ✅ (F13 & F14)

| Feature | Status | Notes |
|---------|--------|-------|
| TaskManager | ✅ | Central registry for spawning and tracking sub-agent tasks |
| `new_task` tool | ✅ | Agent-invocable tool for spawning sub-agents (sync and background) |
| `cancel_task` tool | ✅ | Tool for cancelling running background tasks |
| `list_tasks` tool | ✅ | Tool for querying task status and results |
| Background execution | ✅ | Async spawning via tokio, non-blocking parent loop |
| Result injection | ✅ | Background task results injected as system messages into parent session |
| Task events | ✅ | SubagentStart, SubagentComplete, SubagentCancelled events published |
| Concurrency limits | ✅ | Configurable max_background_agents (default: 4) and timeout (default: 3600s) |
| TUI status bar | ✅ | Shows running task count in status bar |
| `/tasks` command | ✅ | Lists all active and completed tasks with details |
| `/cancel` command | ✅ | Cancels tasks by ID prefix |
| REST API endpoints | ✅ | POST/GET/DELETE /sessions/{id}/tasks endpoints |
| Task ownership | ✅ | Tasks scoped to parent session (403 Forbidden on violation) |
| SSE streaming | ✅ | Task lifecycle events streamed via /events endpoint |

---

## Tools Implementation Status

Total built-in tools: **26 implemented**, **4 planned**

### Fully Implemented Tools (26)

1. `read` ✅
2. `write` ✅
3. `create` ✅
4. `edit` ✅
5. `multiedit` ✅
6. `patch` ✅
7. `rm` ✅
8. `bash` ✅
9. `grep` ✅
10. `glob` ✅
11. `list` ✅
12. `question` ✅
13. `office_read` ✅
14. `office_write` ✅
15. `office_info` ✅
16. `pdf_read` ✅
17. `pdf_write` ✅
18. `webfetch` ✅
19. `websearch` ✅
20. `plan_enter` ✅
21. `plan_exit` ✅
22. `todo_read` ✅
23. `todo_write` ✅

### Planned Tools (4)

1. `new_task` 🔲 - Subtask delegation
2. `switch_agent` 🔲 - Agent switching within session
3. `codebase_search` ❌ - Semantic search (requires indexing)
4. `generate_image` 🔲 - Image generation

---

## Provider Support Status

### Fully Implemented Providers (4/12)

1. **Anthropic** ✅
   - Models: Claude 3, Claude 3.5 Sonnet, Claude Sonnet 4, etc.
   - Features: Streaming, tool use, extended thinking
   
2. **OpenAI** ✅
   - Models: GPT-4o, GPT-4o mini, GPT-4 Turbo, etc.
   - Features: Streaming, tool use, vision
   
3. **GitHub Copilot** ✅
   - Device flow auth, auto-discovery
   - Models: GPT-4o, Claude Sonnet 4, o3-mini
   - Features: Streaming, tool use
   
4. **Ollama** ✅
   - Local/remote LLM support
   - Model discovery via /api/tags
   - Features: Streaming, OpenAI-compatible

### Planned Providers (8/12)

1. Google Generative AI 🔲
2. Azure OpenAI 🔲
3. AWS Bedrock 🔲
4. OpenRouter 🔲
5. XAI 🔲
6. Mistral 🔲
7. Groq 🔲
8. Custom OpenAI-compatible 🔲

---

## Summary Statistics

### Implementation Coverage

- **Total Sections in SPEC.md**: 35 core modules/sections
- **Fully Implemented**: 15 (43%)
- **Partially Implemented**: 8 (23%)
- **Planned/Not Started**: 12 (34%)

### By Category

| Category | Implemented | Partial | Not Started | Total |
|----------|------------|---------|-------------|-------|
| Core Systems | 7 | 1 | 0 | 8 |
| Agent System | 8 | 2 | 2 | 12 |
| Tools | 26 | 0 | 4 | 30 |
| Providers | 4 | 0 | 8 | 12 |
| Features | 14 | 5 | 12 | 31 |
| **TOTAL** | **59** | **8** | **26** | **93** |

### Key Strengths

✅ **Production-Ready Features:**
- Core session management and conversation persistence
- 26 fully-working tools (including sub-agent management tools)
- 4 LLM providers (Anthropic, OpenAI, Copilot, Ollama)
- Full TUI with all essential commands and keybindings
- Permission system with interactive approval flow
- HTTP server with SSE streaming
- MCP client for extensibility
- Extended thinking / reasoning support
- Todo list management
- Context auto-compaction
- Sub-agent spawning and background task execution (F13, F14)

### Key Gaps

❌ **Not Yet Implemented:**
- LSP integration
- Skills system
- Hooks
- Persistent memory
- Semantic codebase search
- Git worktree isolation
- File ignore patterns (.ragentignore)
- Suggested responses
- Trusted directories
- 8 additional LLM providers

⚠️ **Partially Implemented:**
- Custom agents (config works, markdown files not loaded)
- Session resume (by-ID works, --continue/--resume flags missing)
- Context compaction (automatic works, manual features incomplete)
- Hierarchical custom instructions (AGENTS.md works, rules/ dirs not loaded)

---

## Next Steps for Developers

Based on this implementation status, recommended priority areas are:

1. **High Priority (needed for v1.0):**
   - LSP integration for code intelligence
   - File ignore patterns (.ragentignore)
   - `.ragent/rules/` directory loading for hierarchical instructions
   - Session picker UI (--resume / --continue flags)

2. **Medium Priority (nice-to-have):**
   - Additional LLM providers (Google, Azure, Bedrock)
   - Skills system
   - Persistent memory
   - Hooks system
   - Git worktree isolation

3. **Lower Priority (v1.1+):**
   - Codebase semantic search
   - Suggested responses
   - Trusted directories
   - Web UI
   - Plugin system

---

*Last Updated: Development in progress*
*For real-time status, refer to SPEC.md and the codebase*
