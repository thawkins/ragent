# Ragent Specification

> **Version:** 0.1.0-alpha.38  
> **Last Updated:** 2026-01-15

---

## Table of Contents

1. [Overview](#1-overview)
2. [Architecture](#2-architecture)
3. [Core Features](#3-core-features)
4. [Terminal User Interface (TUI)](#4-terminal-user-interface-tui)
5. [HTTP Server & API](#5-http-server--api)
6. [Code Index](#6-code-index)
7. [Memory System](#7-memory-system)
8. [Teams](#8-teams)
9. [Skills System](#9-skills-system)
10. [Prompt Optimization](#10-prompt-optimization)
11. [Security & Permissions](#11-security--permissions)
12. [Configuration](#12-configuration)
13. [Custom Agents](#13-custom-agents)
14. [Tool Reference](#14-tool-reference)

---

## 1. Overview

Ragent is an AI coding agent for the terminal, built in Rust. It provides multi-provider LLM orchestration, a built-in tool system, terminal UI, and client/server architecture — all compiled into a single statically-linked binary.

### 1.1 Key Characteristics

- **Multi-provider LLM support** — Anthropic, OpenAI, GitHub Copilot, Ollama, and Generic OpenAI-compatible APIs
- **Comprehensive tool system** — 128+ tools covering file operations, code analysis, GitHub integration, web access, office documents, memory, teams, and more
- **Built-in TUI** — Full-screen ratatui interface with streaming chat, slash commands, and real-time updates
- **HTTP server** — REST + SSE API for external integrations
- **Zero external dependencies** — Self-contained binary with SQLite, Tantivy, and tree-sitter compiled in

---

## 2. Architecture

```
┌─────────────────────────────────────────────────────────────────┐
│                        User Interface                            │
│  ┌──────────────┐              ┌──────────────┐                  │
│  │     TUI      │              │ HTTP Server  │                  │
│  │  (ratatui)   │              │   (axum)     │                  │
│  └──────┬───────┘              └──────┬───────┘                  │
└─────────┼─────────────────────────────┼────────────────────────┘
          │                             │
          ▼                             ▼
┌─────────────────────────────────────────────────────────────────┐
│                      Event Bus (tokio)                           │
└──────────────────────────┬──────────────────────────────────────┘
                           │
          ┌────────────────┼────────────────┐
          ▼                ▼                ▼
┌──────────────┐  ┌──────────────┐  ┌──────────────┐
│   Session    │  │   Agent      │  │    Tool      │
│  Processor   │  │   System     │  │   Registry   │
└──────┬───────┘  └──────────────┘  └──────┬───────┘
       │                                   │
       ▼                                   ▼
┌──────────────┐  ┌───────────��──┐  ┌──────────────┐
│   Provider   │  │   Storage    │  │  Background  │
│   (LLM API)  │  │  (SQLite)    │  │    Agents    │
└──────────────┘  └──────────────┘  └──────────────┘
```

### 2.1 Workspace Crates

| Crate | Purpose |
|-------|---------|
| `ragent-core` | Types, storage, config, providers, tools, agents, sessions, event bus |
| `ragent-code` | Codebase indexing: tree-sitter parsing, SQLite store, Tantivy FTS, file watcher |
| `ragent-server` | Axum HTTP routes, SSE streaming |
| `ragent-tui` | Ratatui terminal interface |
| `prompt_opt` | Prompt optimization transformations |

---

## 3. Core Features

### 3.1 LLM Providers

#### Supported Providers

| Provider | ID | Authentication | Features |
|----------|-----|---------------|----------|
| **Anthropic** | `anthropic` | `ANTHROPIC_API_KEY` | Streaming, tools, vision, reasoning |
| **OpenAI** | `openai` | `OPENAI_API_KEY` | Streaming, tools, vision |
| **GitHub Copilot** | `copilot` | Auto-discovered from VS Code | Streaming, tools, vision, reasoning levels |
| **Ollama** | `ollama` | No key required | Local models, streaming |
| **Ollama Cloud** | `ollama_cloud` | `OLLAMA_API_KEY` | Remote Ollama servers, dynamic model discovery, vision |
| **Generic OpenAI** | `generic_openai` | `GENERIC_OPENAI_API_KEY` | Any OpenAI-compatible endpoint |
| **Google Gemini** | `gemini` *(planned)* | `GEMINI_API_KEY` *(planned)* | Streaming, tools, vision |

#### Provider Features

- **Health indicators** — Real-time connectivity status (● green/✗ red/● yellow)
- **Model discovery** — Automatic model listing from provider APIs
- **Vision support** — Image attachments for supported models
- **Reasoning levels** — Copilot reasoning level selection (low/medium/high/none)
- **Context window display** — Status bar shows context utilization percentage
- **Extended thinking** — Anthropic extended thinking/reasoning support
- **Usage tracking** — Token usage and cost tracking

#### Anthropic Models

| Model | Context | Max Output | Capabilities |
|-------|---------|------------|--------------|
| `claude-sonnet-4-20250514` | 200,000 | 64,000 | reasoning, streaming, vision, tool_use |
| `claude-3-5-haiku-latest` | 200,000 | 8,192 | streaming, vision, tool_use |

#### OpenAI Models

| Model | Context | Max Output | Capabilities |
|-------|---------|------------|--------------|
| `gpt-4o` | 128,000 | 16,384 | streaming, vision, tool_use |
| `gpt-4o-mini` | 128,000 | 16,384 | streaming, vision, tool_use |

#### Ollama Cloud Provider

The Ollama Cloud provider connects to remote Ollama servers using native `/api/chat` and `/api/tags` endpoints with Bearer token authentication.

**Configuration:**
- **Environment Variable:** `OLLAMA_API_KEY` — API key for authenticated Ollama Cloud instances
- **Default Endpoint:** `https://ollama.com`
- **Custom Endpoint:** Configurable via `base_url` in `ragent.json`

**Features:**
- **Dynamic Model Discovery** — Automatically fetches available models from `/api/tags` endpoint
- **Context Window Detection** — Queries `/api/show` to retrieve actual context length from model metadata
- **Vision Capability Detection** — Automatically detects vision support from model capabilities
- **Streaming Support** — Native SSE streaming via `/api/chat` endpoint
- **Tool Support** — Compatible with Ollama tool-calling format

**Model Listing:**
```bash
ragent models --provider ollama_cloud
```

**Configuration Example (`ragent.json`):**
```json
{
  "provider": {
    "ollama_cloud": {
      "apiKey": "ollama_api_key_here",
      "models": {
        "llama3.2": { "max_tokens": 8192 }
      }
    }
  }
}
```

#### Ollama (Local) Provider

The local Ollama provider connects to self-hosted Ollama instances (no authentication required for local servers).

**Configuration:**
- **Environment Variable:** `OLLAMA_HOST` (optional) — Custom server URL (default: `http://127.0.0.1:11434`)
- **No API Key Required** — Local Ollama servers run without authentication

**Features:**
- **Local Model Execution** — Run models on local hardware (CPU/GPU)
- **Dynamic Discovery** — Lists locally available models via `/api/tags`
- **OpenAI-Compatible API** — Uses `/v1/chat/completions` endpoint
- **Streaming Support** — Full SSE streaming

**Model Listing:**
```bash
ragent models --provider ollama
```

#### Google Gemini Provider *(Planned)*

Support for Google Gemini models is planned for a future release.

**Planned Features:**
- **Authentication:** `GEMINI_API_KEY` environment variable
- **Models:** Gemini 1.5 Pro, Gemini 1.5 Flash
- **Capabilities:** Streaming, tool use, vision
- **API:** Google AI Gemini API or Vertex AI

### 3.2 Tool System

#### File Operations Tools (26)

| Tool | Purpose |
|------|---------|
| `read` | Read file contents with line range support |
| `write` | Create new files |
| `edit` | Replace text in existing files |
| `create` | Create new file (alternative to write) |
| `rm` | Delete single files |
| `move_file` | Move/rename files and directories |
| `copy_file` | Copy files to new location |
| `mkdir` | Create directories (mkdir -p) |
| `append_file` | Append text to end of file |
| `file_info` | Get metadata (size, mtime, type) |
| `diff_files` | Compare two files |
| `glob` | Find files matching glob patterns |
| `list` | List directory contents |
| `multiedit` | Atomic multi-file edits |
| `patch` | Apply unified diff patches |
| `str_replace_editor` | Multi-command file editor |
| `file_ops_tool` | Combined file operations |

#### File Operation Aliases

The following are aliases for commonly requested operations:

| Alias | Maps To |
|-------|---------|
| `view_file`, `read_file`, `get_file_contents`, `open_file` | `read` |
| `list_files`, `list_directory` | `list` |
| `find_files` | `glob` |
| `replace_in_file`, `update_file` | `edit` |
| `search`, `search_in_repo`, `file_search` | `grep` |

#### Execution Tools (10)

| Tool | Purpose |
|------|---------|
| `bash` | Execute shell commands with security restrictions |
| `bash_reset` | Reset bash shell state |
| `execute_python` | Run Python code snippets |
| `run_code` / `execute_code` / `execute_bash` / `run_shell_command` / `run_terminal_cmd` | Aliases for bash/code execution |

#### Interactive Tools (3)

| Tool | Purpose |
|------|---------|
| `question` / `ask_user` | Interactive user prompts |
| `think` | Record reasoning notes (no-op) |
| `todo_read` | Read TODO items |
| `todo_write` | Manage TODO items |

#### Utility Tools (3)

| Tool | Purpose |
|------|---------|
| `calculator` | Evaluate mathematical expressions |
| `get_env` | Read environment variables |

### 3.2.1 Tool System Categories Summary

| Category | Count | Description |
|----------|-------|-------------|
| **File Operations** | 26 | read, write, edit, create, rm, move, copy, mkdir, append, diff, multiedit, patch, etc. |
| **Execution** | 10 | bash, bash_reset, execute_python, aliases |
| **Search** | 4 | grep and aliases |
| **Web** | 3 | webfetch, websearch, http_request |
| **Office** | 6 | office_read/write/info, libre_read/write/info |
| **PDF** | 2 | pdf_read, pdf_write |
| **Code Index** | 6 | codeindex_search, symbols, references, dependencies, status, reindex |
| **GitHub** | 10 | Issues and PR management |
| **Memory** | 12 | memory_read/write/replace/store/recall/forget/search/migrate |
| **Journal** | 3 | journal_write, journal_search, journal_read |
| **Team** | 21 | Team lifecycle, tasks, messaging, coordination |
| **Sub-agent** | 5 | new_task, cancel_task, list_tasks, wait_tasks, task_complete |
| **LSP** | 6 | lsp_hover, definition, references, symbols, diagnostics |
| **Plan** | 2 | plan_enter, plan_exit |
| **MCP** | 1 | mcp_tool (McpToolWrapper) |
| **Interactive** | 4 | question, think, todo_read/write |
| **Utility** | 3 | calculator, get_env |
| **TOTAL** | **128+** | All tools including aliases |

#### Team Tools (21)

| Tool | Purpose |
|------|---------|
| `team_create` | Create new team |
| `team_spawn` | Spawn teammate agent |
| `team_cleanup` | Cleanup team resources |
| `team_status` | Get team status |
| `team_idle` | Signal idle state |
| `team_task_create` | Create team task |
| `team_task_claim` | Claim task to work on |
| `team_task_complete` | Mark task complete |
| `team_task_list` | List team tasks |
| `team_assign_task` | Assign task to specific teammate |
| `team_message` | Send message to team member |
| `team_broadcast` | Broadcast to all teammates |
| `team_read_messages` | Read mailbox messages |
| `team_shutdown_teammate` | Request teammate shutdown |
| `team_shutdown_ack` | Acknowledge shutdown request |
| `team_submit_plan` | Submit plan for approval |
| `team_approve_plan` | Approve teammate plan |
| `team_wait` | Wait for teammates to complete |
| `team_memory_read` | Read team memory |
| `team_memory_write` | Write to team memory |

### 3.3 Agent System

#### Built-in Agents

| Agent | Purpose | Tool Groups |
|-------|---------|-------------|
| `general` | General-purpose assistant | All tools |
| `coder` | Code-focused tasks | File, bash, search |
| `task` | Task execution | File, bash |
| `architect` | Design and planning | All tools |
| `ask` | Question answering | Read-only tools |
| `debug` | Debugging assistance | File, bash, search |
| `code-review` | Code review | Read, diff, github |
| `orchestrator` | Multi-agent coordination | All tools |

#### Agent Features

- **Custom agents** — User-defined agents via JSON (OASF format) or Markdown profiles
- **Template variables** — Dynamic injection of context (`{{WORKING_DIR}}`, `{{FILE_TREE}}`, `{{AGENTS_MD}}`, `{{GIT_STATUS}}`, `{{README}}`)
- **Permission rules** — Per-agent access control for file paths and commands
- **Memory scoping** — Project-level and user-level memory for agents

### 3.4 Session Management

- **Persistent storage** — SQLite-backed conversation history
- **Session commands** — `ragent session list`, `resume`, `export`, `import`
- **Step numbering** — Session-prefixed step numbers (`[sid:step]`) for traceability
- **Context compaction** — Automatic pre-send context management near window limits

---

## 4. Terminal User Interface (TUI)

### 4.1 Screens

| Screen | Key | Purpose |
|--------|-----|---------|
| **Home** | `Esc` | Provider selection, recent sessions, quick actions |
| **Chat** | — | Main conversation interface |
| **Agents** | `a` | Agent selection and management |
| **Log** | `l` / `Alt+L` | Tool call history with pretty-printed JSON |
| **MCP** | `F9` | MCP server discovery and management |
| **Teams** | `F10` | Team coordination panel |
| **Help** | `?` | Keybindings reference |

### 4.2 Slash Commands

| Command | Purpose |
|---------|---------|
| **Core** ||
| `/about` | Show application info, version, and authors |
| `/help` | Show available slash commands |
| `/quit`, `/exit` | Exit ragent |
| **Session & Agent** ||
| `/agent <name>` | Switch to specific agent |
| `/agents` | List all agents (built-in and custom) |
| `/clear` | Clear conversation history |
| `/compact` | Summarize and compact conversation history |
| `/resume` | Resume agent from halted state |
| `/system <prompt>` | Override agent system prompt |
| **Provider & Model** ||
| `/model` | Switch active model on current provider |
| `/provider` | Change LLM provider |
| `/provider_reset` | Reset provider and remove stored credentials |
| `/llmstats` | Show LLM response time and token throughput |
| `/cost` | Show token usage and estimated cost |
| **Context & Config** ||
| `/context refresh` | Clear cached file tree, git status, README |
| `/browse_refresh` | Refresh @ file-picker project index |
| `/reload [all\|config\|mcp\|skills\|agents]` | Reload customizations |
| `/init` | Analyze project and write to PROJECT_ANALYSIS.md |
| **Tasks** ||
| `/tasks` | List active background tasks |
| `/cancel_task <id>` | Cancel a background task |
| `/abort` | Abort current running agent |
| **Tools** ||
| `/tools` | List available tools with parameters |
| `/bash allow <cmd>` | Add command to bash allowlist |
| `/bash deny <cmd>` | Add command to bash denylist |
| `/bash reset` | Reset bash shell state |
| **Code Index** ||
| `/codeindex on\|off` | Toggle code indexing |
| `/codeindex reindex` | Force full re-index |
| `/codeindex status` | Show index status |
| **Memory** ||
| `/memory` | Open memory browser |
| **Team** ||
| `/team create <name>` | Create new team |
| `/team open <name>` | Open existing team |
| `/team close` | Close team session |
| `/team delete <name>` | Delete team |
| `/team clear` | Clear team state |
| `/team tasks` | Show team tasks table |
| `/team status` | Show team status |
| `/team message <to> <content>` | Send message to teammate |
| `/team broadcast <content>` | Broadcast to all teammates |
| `/team spawn <agent>` | Spawn teammate agent |
| `/team cleanup` | Cleanup team resources |
| **MCP** ||
| `/mcp discover` | Discover MCP servers |
| `/mcp list` | List connected MCP servers |
| `/mcp call <server> <tool>` | Call MCP tool |
| **Optimization** ||
| `/opt <method> <prompt>` | Optimize prompt |
| `/opt help` | Show optimization methods |
| **Diagnostics** ||
| `/doctor` | Run diagnostics |
| `/update` | Check for updates |
| **UI** ||
| `/log` | Toggle log panel visibility |
| `/compact` | Compact context window |
| `/agent_compact` | Compact agent description |

### 4.3 Key Bindings

| Key | Action |
|-----|--------|
| `Enter` | Send message |
| `Ctrl+C` | Interrupt current operation |
| `Esc` | Return to home screen |
| `Tab` | Cycle focus between panels |
| `↑/↓` | Scroll message/log panels |
| `PgUp/PgDn` | Page scroll |
| `Home/End` | Jump to start/end |
| `Alt+V` | Paste image from clipboard |
| `Right-click` | Context menu (Cut/Copy/Paste) |
| `p` (home) | Open provider setup |
| `r` (home) | Resume previous session |
| `?` (empty input) | Show keybindings help |

### 4.4 TUI Features

- **Streaming responses** — Real-time token streaming from LLM
- **Step-numbered tool calls** — Cross-session tool call correlation
- **Pretty-printed JSON** — Formatted tool parameters in log panel
- **Image attachments** — Visual support with clipboard paste
- **Mouse support** — Full mouse interaction
- **Auto-complete** — Slash command and agent name completion

---

## 5. HTTP Server & API

### 5.1 Server Commands

```bash
ragent serve              # Start server on default port (9100)
ragent serve --port 8080  # Custom port
```

### 5.2 API Endpoints

#### Health & Status

| Method | Endpoint | Description |
|--------|----------|-------------|
| `GET` | `/health` | Health check - returns "ok" |

#### Configuration & Providers

| Method | Endpoint | Description |
|--------|----------|-------------|
| `GET` | `/config` | Get current application configuration |
| `GET` | `/providers` | List configured provider IDs |

#### Sessions

| Method | Endpoint | Description |
|--------|----------|-------------|
| `GET` | `/sessions` | List all sessions |
| `POST` | `/sessions` | Create new session |
| `GET` | `/sessions/{id}` | Get session details |
| `DELETE` | `/sessions/{id}` | Archive/delete a session |
| `GET` | `/sessions/{id}/messages` | Get messages for a session |
| `POST` | `/sessions/{id}/messages` | Send message (returns SSE stream) |
| `POST` | `/sessions/{id}/abort` | Abort an active session |
| `POST` | `/sessions/{id}/permission/{req_id}` | Reply to a permission request |

#### Tasks (Background Agents)

| Method | Endpoint | Description |
|--------|----------|-------------|
| `GET` | `/sessions/{id}/tasks` | List tasks for a session |
| `POST` | `/sessions/{id}/tasks` | Spawn a new background task |
| `GET` | `/sessions/{id}/tasks/{tid}` | Get task details |
| `DELETE` | `/sessions/{id}/tasks/{tid}` | Cancel a task |

#### Server-Sent Events (SSE)

| Method | Endpoint | Description |
|--------|----------|-------------|
| `GET` | `/events` | Global SSE event stream (all sessions) |
| `GET` | `/sessions/{id}/messages` | Session-specific SSE stream |

#### Agents

| Method | Endpoint | Description |
|--------|----------|-------------|
| `GET` | `/agents` | List available agents |
| `GET` | `/agents/{name}` | Get agent details |

#### Tools

| Method | Endpoint | Description |
|--------|----------|-------------|
| `GET` | `/tools` | List available tools |
| `POST` | `/tools/{name}` | Execute tool |

#### Prompt Optimization

| Method | Endpoint | Description |
|--------|----------|-------------|
| `POST` | `/opt` | Optimize prompt (requires Bearer token) |

#### Memory API

| Method | Endpoint | Description |
|--------|----------|-------------|
| `GET` | `/memory/blocks` | List memory blocks |
| `GET` | `/memory/blocks/{scope}/{label}` | Get specific block |
| `PUT` | `/memory/blocks/{scope}/{label}` | Create/update block |
| `DELETE` | `/memory/blocks/{scope}/{label}` | Delete block |
| `POST` | `/memory/store` | Store structured memory |
| `POST` | `/memory/search` | Search memories |
| `GET` | `/memory/search` | Search memories (query params) |

#### Journal API

| Method | Endpoint | Description |
|--------|----------|-------------|
| `GET` | `/journal` | List journal entries |
| `POST` | `/journal` | Create journal entry |
| `GET` | `/journal/{id}` | Get entry by ID |
| `POST` | `/journal/search` | Search journal entries |

### 5.3 Authentication

- Bearer token generated on server startup
- Token displayed in console: `Server token: {token}`
- Include in requests: `Authorization: Bearer {token}`

---

## 6. Code Index

### 6.1 Overview

Automatic codebase indexing with tree-sitter parsing, full-text search via Tantivy, and incremental updates via file watcher.

### 6.2 Supported Languages

| Language | Extensions | Symbols Extracted |
|----------|------------|-------------------|
| **Rust** | `.rs` | Functions, structs, enums, traits, impls, modules, consts |
| **Python** | `.py` | Functions, classes, methods, decorators, imports |
| **TypeScript** | `.ts`, `.tsx` | Functions, classes, interfaces, types, imports |
| **JavaScript** | `.js`, `.jsx` | Functions, classes, methods, imports |
| **Go** | `.go` | Functions, structs, interfaces, methods, imports |
| **C/C++** | `.c`, `.cpp`, `.h`, `.hpp` | Functions, structs, enums, classes, includes |
| **Java** | `.java` | Classes, interfaces, enums, methods, imports |

### 6.3 Index Storage

- **SQLite database** — Symbols, imports, references, file metadata
- **Tantivy FTS** — Full-text search index
- **Tree cache** — LRU cache of parse trees for incremental updates
- **Content hashing** — Blake3 hashes for change detection

### 6.4 Control

```bash
/codeindex on    # Enable indexing
/codeindex off   # Disable indexing
```

Configuration in `ragent.json`:

```jsonc
{
  "code_index": {
    "enabled": true,
    "max_file_size": 1048576,
    "extra_exclude_dirs": ["vendor", "node_modules"],
    "extra_exclude_patterns": ["*.min.js"]
  }
}
```

---

## 7. Memory System

### 7.1 Memory Types

| Type | Scope | Purpose |
|------|-------|---------|
| **Working Memory** | Session | Active conversation context |
| **Episodic Memory** | Persistent | Past interactions with embeddings |
| **Semantic Memory** | Persistent | Facts, concepts, relationships |
| **Procedural Memory** | Persistent | How-to knowledge, patterns |

### 7.2 Memory Blocks

Named, scoped memory blocks stored in:
- `~/.ragent/memory/` — User-global
- `.ragent/memory/` — Project-local (higher priority)

Block format (YAML frontmatter + Markdown):

```yaml
---
label: project
scope: project
description: Codebase-specific knowledge
limit: 5000
read_only: false
---
# Content here...
```

### 7.3 Automatic Memory Extraction

The extraction engine observes tool usage and session events to propose structured memories:

- **Pattern extraction** — Coding conventions from file edits
- **Error resolution** — Problem-solution pairs from bash failures
- **Session summaries** — Workflow patterns from tool usage

Configuration:

```jsonc
{
  "memory": {
    "auto_extract": {
      "enabled": true,
      "require_confirmation": true
    }
  }
}
```

### 7.4 Semantic Search

Optional embedding-based semantic search:

```jsonc
{
  "memory": {
    "semantic": {
      "enabled": true,
      "model": "all-MiniLM-L6-v2",
      "dimensions": 384
    }
  }
}
```

Requires `embeddings` feature and ONNX Runtime.

### 7.5 Memory Lifecycle Management

- **Compaction** — Automatic block size management
- **Deduplication** — Semantic similarity detection and merging
- **Eviction** — Stale memory cleanup based on confidence and age

---

## 8. Teams

### 8.1 Overview

Teams enable one lead session to coordinate multiple teammate agents with shared tasks and mailbox messaging.

### 8.2 Team Lifecycle

| Phase | Command | Description |
|-------|---------|-------------|
| Create | `/team create <name>` | Create new team |
| Open | `/team open <name>` | Re-open existing team |
| Spawn | `team_spawn` | Add teammates |
| Tasks | `team_task_create` | Create shared tasks |
| Work | `team_task_claim` | Teammates claim tasks |
| Complete | `team_task_complete` | Mark tasks done |
| Close | `/team close` | Close team session |
| Cleanup | `/team cleanup` | Remove team resources |

### 8.3 Blueprints

Pre-configured team templates stored in:
- `~/.ragent/blueprints/` — User-global
- `.ragent/blueprints/` — Project-local

Blueprint structure:

```
blueprint-name/
├── config.json          # Team configuration
├── spawn-prompts.json   # Teammate spawn prompts
└── task-seed.json       # Initial tasks (optional)
```

### 8.4 Communication

- **Mailbox system** — Async message passing between team members
- **Broadcast** — Send to all teammates simultaneously
- **Direct messages** — Private communication

### 8.5 Task Management

- **Race-free claiming** — File-lock based task assignment
- **Dependencies** — Tasks can depend on other tasks
- **Status tracking** — Pending, InProgress, Done, Blocked

---

## 9. Skills System

### 9.1 Overview

Reusable skill definitions using YAML frontmatter-based `SKILL.md` format.

### 9.2 Skill Scopes

| Scope | Location | Priority |
|-------|----------|----------|
| **Bundled** | Embedded in binary | Lowest |
| **Enterprise** | `~/.ragent/skills/` | Medium |
| **Personal** | `~/.ragent/personal-skills/` | High |
| **Project** | `.ragent/skills/` | Highest |

### 9.3 Bundled Skills

| Skill | Purpose |
|-------|---------|
| `simplify` | Review code for quality and efficiency |
| `batch` | Execute batch operations |
| `debug` | Troubleshoot issues |
| `loop` | Iterative task execution |

### 9.4 Skill Format

```yaml
---
name: skill-name
description: What this skill does
context: inline  # or "fork" for subagent
arguments:
  - name: arg1
    description: Argument description
---
Skill body with $ARGUMENTS substitution
```

### 9.5 Argument Substitution

- `$ARGUMENTS` — All arguments
- `$0`, `$1`, `$N` — Nth argument (0-indexed)
- `$ARGUMENTS[N]` — Nth argument (array style)
- `${RAGENT_SESSION_ID}` — Current session ID
- `${RAGENT_SKILL_DIR}` — Skill directory path
- `` !`command` `` — Dynamic context injection via shell

---

## 10. Prompt Optimization

### 10.1 Overview

Transform plain prompts into structured frameworks using `/opt` command or `POST /opt` endpoint.

### 10.2 Optimization Methods

| Method | Description |
|--------|-------------|
| `co_star` | Context, Objective, Scope, Task, Action, Result |
| `crispe` | Context, Role, Intent, Steps, Persona, Examples |
| `cot` | Chain-of-Thought step-by-step reasoning |
| `draw` | Image prompt: subject, style, details, negatives |
| `rise` | Role, Intent, Scope, Examples |
| `o1_style` | Stylized creative tokens and constraints |
| `meta` | Meta Prompting — generate internal prompt |
| `variational` | VARI — multiple prompt candidates |
| `q_star` | Q* — iterative query refinement |
| `openai` | OpenAI/GPT system+user adapter |
| `claude` | Anthropic Claude adapter |
| `microsoft` | Microsoft Azure AI adapter |

### 10.3 Usage

```bash
/opt help                           # Show method table
/opt co_star Explain Rust lifetimes
/opt cot Solve the two-sum problem
```

---

## 11. Security & Permissions

### 11.1 Permission System

Configurable rules that gate file writes, shell commands, and external access:

```jsonc
{
  "permissions": [
    { "permission": "file:write", "pattern": "src/**", "action": "allow" },
    { "permission": "bash:execute", "pattern": "rm -rf /", "action": "deny" }
  ]
}
```

### 11.2 Bash Security

- **Banned commands** — curl, wget, nc, telnet, etc. (14+ tools)
- **Denied patterns** — sudo, privilege escalation, /dev/tcp exfiltration
- **Allowlist** — User-defined exemptions
- **Denylist** — User-defined blocks
- **Directory escape guard** — Prevents `cd` to parent or absolute paths
- **Syntax validation** — `sh -n -c` pre-check before execution

### 11.3 File Path Security

- `check_path_within_root` — Directory escape guard
- Wildcards not allowed in `rm` tool
- Snapshots before edits for rollback

### 11.4 Auto-approve Mode

```bash
ragent --yes              # Auto-approve all permissions
export RAGENT_YES=1     # Environment variable
```

---

## 12. Configuration

### 12.1 Configuration Files

| File | Purpose |
|------|---------|
| `ragent.json` | Project-level configuration |
| `ragent.jsonc` | Project-level (with comments) |
| `~/.config/ragent/config.json` | User-global configuration |

### 12.2 Configuration Schema

```jsonc
{
  "provider": {
    "anthropic": {
      "env": ["ANTHROPIC_API_KEY"],
      "api": { "max_tokens": 8192 }
    },
    "openai": { /* ... */ },
    "copilot": { /* ... */ },
    "ollama": { /* ... */ },
    "generic_openai": { /* ... */ }
  },
  "defaultAgent": "coder",
  "permissions": [],
  "skill_dirs": [],
  "code_index": {
    "enabled": true,
    "max_file_size": 1048576
  },
  "memory": {
    "auto_extract": { "enabled": false, "require_confirmation": true },
    "semantic": { "enabled": false, "dimensions": 384 },
    "compaction": { "enabled": true, "block_size_limit": 4096 },
    "eviction": { "auto": false, "stale_days": 30 }
  },
  "bash": {
    "allowlist": [],
    "denylist": []
  },
  "hooks": [
    { "trigger": "on_session_start", "command": "echo 'Session started'" }
  ]
}
```

### 12.3 Environment Variables

| Variable | Purpose |
|----------|---------|
| `ANTHROPIC_API_KEY` | Anthropic API key |
| `OPENAI_API_KEY` | OpenAI API key |
| `GENERIC_OPENAI_API_KEY` | Generic OpenAI-compatible key |
| `GITHUB_COPILOT_TOKEN` | GitHub Copilot token |
| `OLLAMA_HOST` | Ollama server URL |
| `RAGENT_LOG_LEVEL` | Log level (trace/debug/info/warn/error) |
| `RAGENT_YES` | Auto-approve all permissions |

---

## 13. Custom Agents

### 13.1 Profile Format (Markdown)

Location: `~/.ragent/agents/` or `.ragent/agents/`

```markdown
---
name: my-agent
description: Custom agent description
model: anthropic/claude-sonnet-4-20250514
memory_scope: project
---
# System Prompt

Your custom instructions here...
```

### 13.2 OASF Format (JSON)

Location: Same directories as above

```json
{
  "schema_version": "oasf/agntcy.org/agent/1.0.0",
  "name": "my-agent",
  "description": "Custom agent description",
  "model": { "provider": "anthropic", "id": "claude-sonnet-4-20250514" },
  "ragent": {
    "version": "1",
    "system_prompt": "Your instructions...",
    "memory_scope": "project",
    "permissions": []
  }
}
```

### 13.3 Template Variables

| Variable | Description |
|----------|-------------|
| `{{WORKING_DIR}}` | Current working directory |
| `{{FILE_TREE}}` | Project file tree (respects .gitignore) |
| `{{AGENTS_MD}}` | Content of AGENTS.md |
| `{{GIT_STATUS}}` | Git branch, status, recent commits |
| `{{README}}` | Content of README.md |
| `{{DATE}}` | Current date (ISO 8601) |

---

## 14. Tool Reference

### 14.1 Total Tool Count

**Current count:** 128+ tools across 17 categories (including aliases)

### 14.2 Tool Categories Summary

| Category | Count | Description |
|----------|-------|-------------|
| File Operations | 28 | Core file manipulation and advanced operations |
| Execution | 10 | Shell commands, Python execution |
| Search | 4 | Text/code search and aliases |
| Web | 3 | HTTP requests and web search |
| Office | 6 | Word/Excel/PowerPoint and LibreOffice |
| PDF | 2 | PDF read/write |
| Code Index | 6 | Symbol search, references, dependencies |
| GitHub | 10 | Issues and PR management |
| Memory | 12 | Block storage and structured memories |
| Journal | 3 | Logging and search |
| Team | 21 | Coordination, tasks, messaging |
| Sub-agent | 5 | Background task management |
| LSP | 6 | Language server protocol tools |
| Plan | 2 | Plan delegation |
| MCP | 1 | Model Context Protocol |
| Interactive | 4 | User prompts, todos, reasoning |
| Utility | 3 | Calculator, environment |

### 14.3 Tool Categories (Detailed)

| Category | Tools |
|----------|-------|
| **File Operations** | read, write, edit, create, rm, move_file, copy_file, mkdir, append_file, file_info, diff_files, glob, list, multiedit, patch, str_replace_editor, file_ops_tool, plus aliases |
| **Execution** | bash, bash_reset, execute_python, and aliases |
| **Search** | grep and aliases |
| **Web** | webfetch, websearch, http_request |
| **Office** | office_read, office_write, office_info, libre_read, libre_write, libre_info |
| **PDF** | pdf_read, pdf_write |
| **Code Index** | codeindex_search, codeindex_symbols, codeindex_references, codeindex_dependencies, codeindex_status, codeindex_reindex |
| **GitHub** | github_list_issues, github_get_issue, github_create_issue, github_comment_issue, github_close_issue, github_list_prs, github_get_pr, github_create_pr, github_merge_pr, github_review_pr |
| **Memory** | memory_read, memory_write, memory_replace, memory_store, memory_recall, memory_search, memory_forget, memory_migrate, team_memory_read, team_memory_write |
| **Journal** | journal_write, journal_search, journal_read |
| **Team** | team_create, team_spawn, team_cleanup, team_status, team_idle, team_task_create, team_task_claim, team_task_complete, team_task_list, team_assign_task, team_message, team_broadcast, team_read_messages, team_shutdown_teammate, team_shutdown_ack, team_submit_plan, team_approve_plan, team_wait, team_memory_read, team_memory_write |
| **Sub-agent** | new_task, cancel_task, list_tasks, wait_tasks, task_complete |
| **LSP** | lsp_hover, lsp_definition, lsp_references, lsp_symbols, lsp_diagnostics |
| **Plan** | plan_enter, plan_exit |
| **MCP** | mcp_call (via McpToolWrapper) |
| **Interactive** | question, think, todo_read, todo_write |
| **Utility** | calculator, get_env, file_info |

---

## Appendix A: Version History

See [CHANGELOG.md](CHANGELOG.md) for complete version history.

---

## Appendix B: Documentation

- [README.md](README.md) — Project overview
- [QUICKSTART.md](QUICKSTART.md) — Getting started guide
- [INSTALLATION_GUIDE.md](INSTALLATION_GUIDE.md) — Detailed installation
- [AGENTS.md](AGENTS.md) — Agent guidelines (loaded into prompts)
- [CODEINDEX.md](CODEINDEX.md) — Code indexing design
- [RAGENTMEM.md](RAGENTMEM.md) — Memory system design
- [docs/](docs/) — Additional documentation

---

*End of Specification*
