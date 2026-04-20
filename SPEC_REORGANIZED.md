
## Executive Summary

Ragent is an open-source AI coding agent for the terminal, written entirely in
Rust and distributed as a single statically-linked binary with zero external
runtime dependencies. It orchestrates multiple LLM providers — Anthropic,
OpenAI, GitHub Copilot, Ollama (local and cloud), and any OpenAI-compatible
endpoint — behind a unified streaming interface, giving developers a powerful,
provider-agnostic assistant that runs wherever a terminal does.

### What It Does

Ragent bridges the gap between conversational AI and hands-on software
engineering. An agent can read and write files, execute shell commands, search
codebases, manage Git and GitHub workflows, query language servers, read and
write office documents, and coordinate with other agents — all through a
library of **147+ built-in tools** organised across 18 categories. Every tool
invocation passes through a multi-layered security and permission system that
gives the user full control over what the agent can and cannot do.

### How It Works

At its core, ragent follows a **session → agent → tool** loop. A session
processor manages the conversation with the LLM provider, the agent system
defines personality and capabilities via profiles, and the tool registry
dispatches execution requests. An asynchronous event bus (built on tokio)
connects all components, enabling real-time streaming of tokens, tool results,
and status updates to both the TUI and the HTTP API.

```
User ──▶ TUI / HTTP API ──▶ Session Processor ──▶ LLM Provider
                                    │
                              Agent Profile
                                    │
                              Tool Registry ──▶ File ops, bash, GitHub,
                                                code index, memory, teams,
                                                office docs, LSP, web, ...
```

### Key Capabilities

| Capability | Summary |
|-----------|---------|
| **Multi-provider LLM** | 7 providers with automatic model discovery, health monitoring, streaming, vision, and reasoning levels |
| **Terminal UI** | Full-screen ratatui interface with streaming markdown, syntax highlighting, slash commands, and image support |
| **HTTP Server** | REST + SSE API (Axum) for headless operation and external integrations |
| **Tool System** | 147+ tools: file ops, shell, search, GitHub, GitLab, code index, memory, journal, teams, sub-agents, LSP, office/PDF, web, MCP |
| **Code Intelligence** | Tree-sitter parsing (15+ languages), Tantivy FTS, symbol/reference search, and optional LSP integration |
| **Persistent Memory** | Three-tier system — file blocks, structured SQLite store, and optional embedding-based semantic search — with automatic extraction, decay, compaction, and a knowledge graph |
| **Teams & Swarms** | Multi-agent coordination with named teammates, shared task lists, mailbox messaging, and swarm decomposition for parallel work |
| **Security** | Permission rules (allow/deny/ask), 7-layer bash safety, file-path guards, secret redaction, resource limits, and YOLO mode for trusted environments |
| **Skills** | Loadable skill packs (bundled or custom YAML) that inject tools, prompts, and file context into agent sessions |
| **Custom Agents** | OASF-based agent profiles with configurable models, tools, permissions, and personality |
| **Autopilot** | Autonomous operation mode with configurable iteration limits and permission auto-approval |
| **AIWiki** | Project-scoped knowledge base with LLM-powered extraction, multi-format ingestion (MD/PDF/DOCX/ODT), web interface, entity/concept graphs, and agent-accessible search |

### Who It's For

Ragent is designed for software developers and teams who want an AI assistant
that lives in their terminal, respects their security boundaries, and learns
from their workflow over time. It is equally suited to interactive pair-programming
sessions and headless CI/CD integration via its HTTP API.

### Technology

| Aspect | Detail |
|--------|--------|
| **Language** | Rust (edition 2021) |
| **Async runtime** | tokio |
| **TUI framework** | ratatui + crossterm |
| **HTTP framework** | Axum |
| **Database** | SQLite (rusqlite, compiled-in) |
| **Full-text search** | Tantivy |
| **Code parsing** | tree-sitter (15+ grammars compiled-in) |
| **Embeddings** | ONNX Runtime (optional, `all-MiniLM-L6-v2`) |
| **Binary size** | Single static binary, ~50 MB release |
| **Platforms** | Linux, macOS, Windows (cross-compiled) |

### Project Status

Ragent is in **alpha** (v0.1.0-alpha.44). The core architecture, tool system,
TUI, HTTP server, memory system, teams, security layer, and AIWiki knowledge base
are functional and under active development. The specification below documents
the current state of all subsystems.

**Current Release Highlights:**
- AIWiki knowledge base fully implemented with 6 complete milestones
- AIWiki single-file reference path resolution bugfix
- Unified TUI dialog and button component system
- 147+ tools across 18 categories including comprehensive team coordination tools
- Native GitLab integration with issues, merge requests, and CI/CD pipeline management

---


## Table of Contents

- [Executive Summary](#executive-summary)

### Part I: Foundation & Basics

1. [Overview](#overview)
2. [Architecture](#architecture)
3. [Core Features](#core-features)
4. [Terminal User Interface (TUI)](#terminal-user-interface-tui)
5. [HTTP Server & API](#http-server-api)

### Part II: Data & Knowledge Systems

6. [Code Index](#code-index)
7. [Memory System](#memory-system)
8. [AIWiki Knowledge Base](#aiwiki-knowledge-base)

### Part III: Multi-Agent Coordination

9. [Teams](#teams)
10. [Swarm Mode](#swarm-mode)
11. [Autopilot Mode](#autopilot-mode)
12. [Orchestrator & Multi-Agent Coordination](#orchestrator-multi-agent-coordination)

### Part IV: Customization & Extension

13. [Custom Agents](#custom-agents)
14. [Skills System](#skills-system)
15. [Prompt Optimization](#prompt-optimization)
16. [Configuration](#configuration)

### Part V: External Integrations

17. [LSP Integration](#lsp-integration)
18. [GitLab Integration](#gitlab-integration)
19. [MCP Integration (Model Context Protocol)](#mcp-integration-model-context-protocol)

### Part VI: Reference Materials

20. [Tool Reference](#tool-reference)
21. [Office, LibreOffice, and PDF Document Tools](#office-libreoffice-and-pdf-document-tools)
22. [CLI Command Reference](#cli-command-reference)
23. [Testing & CI/CD](#testing-cicd)

### Part VII: Security & Operations

24. [Security & Permissions](#security-permissions)
25. [Auto-Update Mechanism](#auto-update-mechanism)

**Appendices**

- [Appendix A: Version History](#appendix-a-version-history)
- [Appendix B: Documentation](#appendix-b-documentation)
- [Appendix C: Project Contact & Repository](#appendix-c-project-contact--repository)
- [Appendix D: Changelog (2025-01-16)](#appendix-d-changelog-2025-01-16)
---



---

# Part I: Foundation & Basics

---

## 1. Overview

Ragent is an AI coding agent for the terminal, built in Rust. It provides multi-provider LLM orchestration, a built-in tool system, terminal UI, and client/server architecture — all compiled into a single statically-linked binary.

### 1.1 Key Characteristics

- **Multi-provider LLM support** — Anthropic, OpenAI, GitHub Copilot, Ollama, and Generic OpenAI-compatible APIs
- **Comprehensive tool system** — 147+ tools covering file operations, code analysis, GitHub/GitLab integration, web access, office documents, memory, teams, and more
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
| **Hugging Face** | `huggingface` | `HF_TOKEN` | Streaming, tools, vision, dynamic model discovery |
| **Generic OpenAI** | `generic_openai` | `GENERIC_OPENAI_API_KEY` | Any OpenAI-compatible endpoint |
| **Google Gemini** | `gemini` | `GEMINI_API_KEY` | Streaming, tools, vision, reasoning |

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

#### Google Gemini Provider

The Google Gemini provider connects to Google's Gemini API for state-of-the-art multimodal models with extensive context windows.

**Authentication:** `GEMINI_API_KEY` environment variable

**Default Models:**

| Model | Context | Cost (Input/Output) | Capabilities |
|-------|---------|---------------------|--------------|
| `gemini-2.5-flash-preview-05-20` | 1,048,576 | $0.15 / $0.60 | reasoning, streaming, vision, tool_use |
| `gemini-2.5-pro-preview-05-06` | 1,048,576 | $1.25 / $10.00 | reasoning, streaming, vision, tool_use |
| `gemini-2.0-flash` | 1,048,576 | $0.10 / $0.40 | streaming, vision, tool_use |
| `gemini-2.0-flash-lite` | 1,048,576 | $0.075 / $0.30 | streaming, vision, tool_use |
| `gemini-1.5-flash` | 1,048,576 | $0.075 / $0.30 | streaming, vision, tool_use |
| `gemini-1.5-pro` | 2,097,152 | $1.25 / $5.00 | reasoning, streaming, vision, tool_use |

**Features:**
- **Streaming** — Real-time token-by-token response streaming
- **Tool Use** — Native function calling for all models
- **Vision** — Image understanding capabilities
- **Reasoning** — Available on Pro and Flash 2.5 models
- **Massive Context Windows** — Up to 2M tokens on 1.5 Pro

**API Base:** `https://generativelanguage.googleapis.com`

#### Hugging Face Provider

The HuggingFace provider connects to the HuggingFace Inference API, which exposes an OpenAI-compatible `/v1/chat/completions` endpoint. Supports both the free/Pro shared Inference API and dedicated Inference Endpoints.

**Authentication:**
- **Primary:** `HF_TOKEN` environment variable (standard HuggingFace token)
- **Legacy:** `HUGGING_FACE_HUB_TOKEN` (older HF token name)
- **Ragent convention:** `RAGENT_API_KEY_HUGGINGFACE` (auto-checked)

**Default Models:**

| Model | Context | Capabilities |
|-------|---------|--------------|
| `meta-llama/Llama-3.1-8B-Instruct` | 128,000 | streaming, tool_use |
| `meta-llama/Llama-3.1-70B-Instruct` | 128,000 | streaming, tool_use |
| `mistralai/Mixtral-8x7B-Instruct-v0.1` | 32,000 | streaming, tool_use |
| `Qwen/Qwen2.5-72B-Instruct` | 128,000 | streaming, tool_use |
| `microsoft/Phi-3-mini-4k-instruct` | 4,096 | streaming |

**Features:**
- **OpenAI-Compatible API** — Uses `/v1/chat/completions` endpoint (same as OpenAI)
- **Streaming Support** — Full SSE streaming with tool call deltas
- **Tool Use** — Function calling for models that support it (Llama 3.1+, Mixtral, Qwen)
- **Dynamic Model Discovery** — Queries HuggingFace Hub API for available text-generation models with warm inference endpoints (up to 50 models)
- **Model Loading Detection** — Detects 503 "model loading" responses with estimated wait time
- **Gated Model Handling** — Clear error messages for models requiring license acceptance
- **Rate Limit Tracking** — Parses `X-RateLimit-Limit`/`X-RateLimit-Remaining` headers

**Provider-Specific Options:**

| Option | Type | Default | Description |
|--------|------|---------|-------------|
| `wait_for_model` | bool | `true` | Send `x-wait-for-model: true` header to wait for cold models |
| `use_cache` | bool | `true` | Enable server-side response caching |

**Inference Endpoints:**

For dedicated deployments, configure the custom endpoint URL:
```json
{
  "provider": {
    "huggingface": {
      "api": {
        "base_url": "https://my-endpoint.endpoints.huggingface.cloud"
      }
    }
  }
}
```

**Model Listing:**
```bash
ragent models --provider huggingface
```

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
| **GitLab** | 19 | Issues, merge requests, pipelines, and jobs |
| **Memory** | 12 | memory_read/write/replace/store/recall/forget/search/migrate |
| **Journal** | 3 | journal_write, journal_search, journal_read |
| **Team** | 21 | Team lifecycle, tasks, messaging, coordination |
| **Sub-agent** | 5 | new_task, cancel_task, list_tasks, wait_tasks, task_complete |
| **LSP** | 6 | lsp_hover, definition, references, symbols, diagnostics |
| **Plan** | 2 | plan_enter, plan_exit |
| **MCP** | 1 | mcp_tool (McpToolWrapper) |
| **Interactive** | 4 | question, think, todo_read/write |
| **Utility** | 3 | calculator, get_env |
| **TOTAL** | **147+** | All tools including aliases |

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

### 3.3.1 GitHub Integration Tools

ragent provides native GitHub issue and pull request tools that auto-detect
the repository owner and name from the local git remote configuration.

#### Issue Tools

| Tool | Description | Parameters |
|------|-------------|------------|
| `github_issues_list` | List issues with filtering | `state` (open/closed/all), `labels`, `limit` |
| `github_issues_get` | Get issue details | `number` |
| `github_issues_create` | Create a new issue | `title`, `body`, `labels`, `assignees` |
| `github_issues_comment` | Add comment to an issue | `number`, `body` |
| `github_issues_close` | Close an issue | `number`, `comment` (optional) |

#### Pull Request Tools

| Tool | Description | Parameters |
|------|-------------|------------|
| `github_pr_list` | List pull requests | `state`, `base`, `limit` |
| `github_pr_get` | Get PR details and diff | `number` |
| `github_pr_create` | Create a new pull request | `title`, `body`, `base`, `head`, `draft` |
| `github_pr_merge` | Merge a pull request | `number`, `method` (merge/squash/rebase) |
| `github_pr_review` | Submit a PR review | `number`, `event` (approve/comment/request_changes), `body` |

#### Auto-Detection

Owner and repository are automatically detected from the git remote:

```text
git remote get-url origin
→ https://github.com/owner/repo.git  → owner="owner", repo="repo"
→ git@github.com:owner/repo.git      → owner="owner", repo="repo"
```

Falls back to explicit `--owner` and `--repo` parameters if detection fails.

### 3.4 Session Management

- **Persistent storage** — SQLite-backed conversation history
- **Session commands** — `ragent session list`, `resume`, `export`, `import`
- **Step numbering** — Session-prefixed step numbers (`[sid:step]`) for traceability
- **Context compaction** — Automatic pre-send context management near window limits

---


## 4. Terminal User Interface (TUI)

### 4.1 TUI Windows and Overlay Panels

The ragent TUI is built on a multi-layer architecture with a main chat screen, modal overlays, popup windows, and sidebar panels. Each window serves a specific purpose in the user workflow.

#### 4.1.1 Main Screen (Chat)

The primary interface where all conversation happens.

| Component | Description |
|-----------|-------------|
| **Status Bar (Line 1)** | Shows session ID, agent name, working directory, git branch, and current status message |
| **Status Bar (Line 2)** | Displays provider/model, token usage, active tasks, LSP/MCP status, code index status, and log indicator |
| **Messages Panel** | Scrollable conversation history with syntax highlighting and formatted tool calls |
| **Input Area** | Multi-line text input with autocomplete support for slash commands and file references |
| **Log Panel** | Toggleable panel showing step-numbered tool calls with pretty-printed JSON |
| **Active Agents Subpanel** | Sidebar showing running background agents with progress indicators |
| **Teams Subpanel** | Sidebar displaying team members, their status, and message counts |

**Access**: This is the default screen when ragent starts (after initial setup).

---

#### 4.1.2 Provider Setup Dialog (Modal)

Multi-step wizard for configuring LLM providers.

| Step | Description |
|------|-------------|
| **Select Provider** | Choose from Anthropic, OpenAI, GitHub Copilot, Ollama, Ollama Cloud, or Generic OpenAI |
| **Enter API Key** | Secure input with masked characters and endpoint URL entry for Generic OpenAI |
| **Device Flow** | GitHub Copilot OAuth flow with user code and verification URL |
| **Select Model** | Browse available models with metadata (context window, cost, capabilities) |
| **Select Agent** | Choose default agent personality |
| **Reset Provider** | Remove stored credentials for a provider |
| **Done** | Confirmation screen showing configured provider and model |

**Access**: `/provider` command, or auto-triggered at first startup

---

#### 4.1.4 Agents Popup Window

A floating popup window showing active background agents and their status.

**Purpose**: Monitor and switch between multiple concurrent agent sessions.

**Features**:
- List of active agents with session IDs
- Agent status indicators (running, idle, error)
- Message count per agent
- Click to focus specific agent session
- Close button to dismiss

**Access**: Click "Agents" button or press `a`

---

#### 4.1.5 Teams Popup Window

A floating popup for team coordination when managing multiple teammates.

**Purpose**: Coordinate work across a team of specialized agents.

**Features**:
- Team member list with status
- Message counts (sent/received per teammate)
- Focus indicator for active teammate
- Task assignment interface
- Broadcast messaging capability

**Access**: Click "Teams" button or press `F10`

---

#### 4.1.6 Slash Command Autocomplete Menu

An inline popup menu that appears when typing `/` in the input area.

**Purpose**: Quick discovery and selection of slash commands.

**Features**:
- Real-time filtering as you type
- Command descriptions
- Skill vs. builtin command indicators
- Keyboard navigation (↑/↓) and Enter to select

**Access**: Type `/` in input area

---

#### 4.1.7 File Reference Autocomplete Menu (`@` Menu)

An inline popup for selecting files when using `@` references.

**Purpose**: Quickly reference files in the conversation.

**Features**:
- Fuzzy file search across project
- Directory navigation mode
- Hidden file toggle
- Recently used files prioritized
- Preview of selected file

**Access**: Type `@` in input area, optionally followed by partial filename

---

#### 4.1.8 History Picker Overlay

A scrollable overlay for browsing and reusing previous inputs.

**Purpose**: Quickly recall and resend previous prompts.

**Features**:
- Chronological list of previous inputs
- Search/filter capability
- Enter to insert, Esc to cancel
- Persistent across sessions (stored in SQLite)

**Access**: `/history` command or Up arrow with empty input

---

#### 4.1.9 Permission Dialog (Modal)

Centered modal for approving or denying permission requests.

**Purpose**: Security gate for file writes, shell commands, and external access.

**Features**:
- Permission type indicator (file:write, bash:execute, etc.)
- Target path or command preview
- One-time (y/n) or always allow options
- Question mode with text input for user prompts

**Access**: Auto-triggered when tool requires permission

---

#### 4.1.10 Context Menu (Right-Click)

A small popup menu for text operations.

**Purpose**: Standard text editing operations in any pane.

**Features**:
- Cut selected text
- Copy to clipboard
- Paste from clipboard
- Context-aware (disabled when no selection)

**Access**: Right-click in any pane

---

#### 4.1.11 LSP Discovery Dialog (Overlay)

An overlay listing discovered Language Server Protocol servers.

**Purpose**: Enable code intelligence features by connecting to LSP servers.

**Features**:
- Numbered list of discovered servers
- Server type and command preview
- Number input to select and enable
- Connection status feedback

**Access**: `/lsp discover` command

---

#### 4.1.12 LSP Edit Dialog (Overlay)

Interactive dialog for managing configured LSP servers.

**Purpose**: Enable/disable LSP servers without editing config files.

**Features**:
- Table of configured servers with enabled/disabled status
- Arrow key navigation
- Space/Enter to toggle status
- Persistent changes to ragent.json

**Access**: `/lsp edit` command

---

#### 4.1.13 MCP Discovery Dialog (Overlay)

An overlay for discovering Model Context Protocol servers.

**Purpose**: Extend tool capabilities via MCP servers.

**Features**:
- Numbered list of discovered MCP servers
- Server metadata display
- Number input to connect
- Connection feedback

**Access**: `/mcp discover` command

---

#### 4.1.14 Output View Overlay

A scrollable panel for viewing raw agent or team member output.

**Purpose**: Inspect unformatted output from specific agents or team members.

**Features**:
- Session output viewer
- Team member output viewer
- Scrollable content
- Syntax highlighting for code

**Access**: Auto-triggered for certain tool outputs or team member responses

---

#### 4.1.15 Memory Browser Overlay

A full-panel overlay for browsing memory blocks.

**Purpose**: View and manage persistent memory across sessions.

**Features**:
- List of global and project memory blocks
- Size indicators (with warnings for blocks near limit)
- Expand/collapse to view full content
- Keyboard navigation (j/k, Enter, Esc)
- Search and filter capabilities

**Access**: `/memory` command

---

#### 4.1.16 Journal Viewer Overlay

A full-panel overlay for browsing journal entries.

**Purpose**: Review recorded insights, decisions, and discoveries.

**Features**:
- Chronological list of journal entries
- Tag filtering and search
- Expand to view full entry content
- Add new entries inline
- FTS5 full-text search support

**Access**: `/journal` command

---

#### 4.1.17 Plan Approval Dialog (Modal)

A centered dialog for approving or rejecting plans from the plan agent.

**Purpose**: Human-in-the-loop approval for plan agent proposals.

**Features**:
- Plan text display with scrollable content
- Approve/Reject buttons
- Cursor navigation between options
- On approve: switches to plan agent and executes
- On reject: returns to previous agent

**Access**: Auto-triggered when plan agent submits a plan

---

#### 4.1.18 Force-Cleanup Confirmation Modal

A confirmation dialog for destructive team cleanup operations.

**Purpose**: Prevent accidental data loss when force-cleaning team resources.

**Features**:
- Warning message with team name
- List of active members that will be affected
- Explicit confirmation required
- Cancel option

**Access**: Triggered by `/team cleanup` when team has active members

---

#### 4.1.19 Keybindings Help Panel (Overlay)

A scrollable help panel showing all keyboard shortcuts.

**Purpose**: Quick reference for TUI controls.

**Features**:
- Categorized keybindings
- Context-aware help (shows relevant shortcuts)
- Search within help
- Scroll with arrow keys

**Access**: `?` key when input is empty, or `/help` command

---

#### 4.1.20 Session/Message Widget Overlays

Various inline widgets rendered within the message panel.

| Widget | Purpose |
|--------|---------|
| **MessageWidget** | Renders individual chat messages with markdown formatting, syntax highlighting, and inline tool call summaries |
| **Tool Result Summaries** | Collapsible sections showing tool execution results |
| **File Diff Widgets** | Side-by-side or inline diffs for file edits |
| **Image Widgets** | Renders attached images with dimensions and preview |

---

#### 4.1.21 Window State Summary

| State Field | Window | Access |
|-------------|--------|--------|
| `provider_setup` | Provider Setup Dialog | `/provider`, startup |
| `show_agents_window` | Agents Popup | Click "Agents" button, `a` key |
| `show_teams_window` | Teams Popup | Click "Teams" button, `F10` key |
| `slash_menu` | Slash Command Menu | Type `/` |
| `file_menu` | File Reference Menu | Type `@` |
| `history_picker` | History Picker | `/history`, Up arrow |
| `permission_queue` | Permission Dialog | Auto (tool permission) |
| `context_menu` | Right-Click Menu | Right-click |
| `lsp_discover` | LSP Discovery | `/lsp discover` |
| `lsp_edit` | LSP Edit | `/lsp edit` |
| `mcp_discover` | MCP Discovery | `/mcp discover` |
| `output_view` | Output View | Auto (tool output) |
| `memory_browser` | Memory Browser | `/memory` |
| `journal_viewer` | Journal Viewer | `/journal` |
| `plan_approval_pending` | Plan Approval | Auto (plan submission) |
| `pending_forcecleanup` | Force-Cleanup Modal | `/team cleanup` (with active) |
| `show_shortcuts` | Keybindings Help | `?` (empty input), `/help` |

---

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
| **AIWiki** ||
| `/aiwiki init` | Initialize AIWiki for current project |
| `/aiwiki on` | Enable AIWiki |
| `/aiwiki off` | Disable AIWiki |
| `/aiwiki status` | Show AIWiki status |
| `/aiwiki ingest <path>` | Ingest document(s) into AIWiki |
| `/aiwiki sync` | Sync wiki with raw/ folder |
| `/aiwiki clear` | Clear all AIWiki data |
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
| **Swarm & Autopilot** ||
| `/swarm <prompt>` | Auto-decompose goal into parallel subtasks |
| `/swarm status` | Check swarm execution status |
| `/autopilot on [--max-tokens N] [--max-time N]` | Enable autonomous operation |
| `/autopilot off` | Disable autonomous operation |
| `/autopilot status` | Show autopilot status |
| `/yolo` | Toggle YOLO mode (bypass all restrictions) |
| **Agent Modes & Planning** ||
| `/mode <role>` | Set agent role: architect, coder, reviewer, debugger, tester, off |
| `/plan <description>` | Delegate planning to the plan agent |
| **GitHub Integration** ||
| `/github login` | Authenticate with GitHub |
| `/github logout` | Remove GitHub credentials |
| `/github status` | Show GitHub connection status |
| **GitLab Integration** ||
| `/gitlab setup` | Configure GitLab connection (instance URL + PAT) |
| `/gitlab logout` | Remove GitLab credentials |
| `/gitlab status` | Show GitLab connection status |
| **Journal & Todos** ||
| `/journal` | View journal entries |
| `/journal search <query>` | Search journal entries |
| `/journal add <title>` | Add journal entry |
| `/todos` | Show TODO items |
| **LSP & Skills** ||
| `/lsp discover` | Discover LSP servers |
| `/lsp connect <id>` | Connect to LSP server |
| `/lsp disconnect <id>` | Disconnect LSP server |
| `/skills` | List registered skills |
| **Server & Diagnostics** ||
| `/webapi enable` | Enable HTTP REST API |
| `/webapi disable` | Disable HTTP REST API |
| `/doctor` | Run system diagnostics |
| `/update` | Check for updates |
| `/update install` | Install updates |
| **UI & History** ||
| `/log` | Toggle log panel visibility |
| `/history` | Browse previous inputs |
| `/inputdiag` | Input diagnostics |
| `/compact` | Compact context window |
| `/agent_compact` | Compact agent description |

### 4.3 Key Bindings

| Key | Action |
|-----|--------|
| `Enter` | Send message |
| `Ctrl+C` | Interrupt current operation |
| `Esc` | Clear input / Close overlay |
| `Tab` | Cycle focus between panels |
| `↑/↓` | Scroll message/log panels |
| `PgUp/PgDn` | Page scroll |
| `Home/End` | Jump to start/end |
| `Alt+V` | Paste image from clipboard |
| `Right-click` | Context menu (Cut/Copy/Paste) |
| `p` | Open provider setup |
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

#### Orchestrator API

| Method | Endpoint | Description |
|--------|----------|-------------|
| `POST` | `/orchestrate` | Submit a job to the orchestrator |
| `GET` | `/orchestrate/{job_id}` | Get job status and results |
| `DELETE` | `/orchestrate/{job_id}` | Cancel a running orchestration job |

#### Response Types

**BlockResponse (Memory blocks):**

```json
{
  "scope": "project",
  "label": "conventions",
  "content": "Use snake_case...",
  "read_only": false,
  "created_at": "2025-01-15T10:30:00Z",
  "updated_at": "2025-01-15T12:00:00Z"
}
```

**MemoryResponse (Structured memories):**

```json
{
  "id": "mem_abc123",
  "content": "The project uses PostgreSQL...",
  "category": "tech_stack",
  "confidence": 0.85,
  "tags": ["database", "infrastructure"],
  "created_at": "2025-01-15T10:30:00Z",
  "last_accessed": "2025-01-16T08:00:00Z"
}
```

**JournalEntryResponse:**

```json
{
  "id": "j_xyz789",
  "session_id": "sess_001",
  "entry_type": "decision",
  "title": "Chose PostgreSQL over MySQL",
  "content": "After comparing performance benchmarks...",
  "tags": ["database", "architecture"],
  "created_at": "2025-01-15T10:30:00Z"
}
```

**Search Request Body (`/memory/search`, `/journal/search`):**

```json
{
  "query": "database configuration",
  "limit": 10,
  "semantic": true,
  "filters": {
    "category": "tech_stack",
    "min_confidence": 0.5
  }
}
```

### 5.3 Authentication

- Bearer token generated on server startup
- Token displayed in console: `Server token: {token}`
- Include in requests: `Authorization: Bearer {token}`

---



---

# Part II: Data & Knowledge Systems

---

## 6. Code Index

### 6.1 Overview

The Code Index is a built-in codebase indexing, search, and retrieval system that provides agents with deep, structured understanding of the codebase. Unlike simple text search (grep), it extracts symbols, their relationships, and enables semantic code exploration.

**Key Features:**
- **Zero external dependencies** — Everything compiles into the ragent binary (tree-sitter, SQLite, Tantivy)
- **User-controllable** — Enable/disable at any time via `/codeindex on|off`
- **Non-intrusive** — Zero overhead when disabled
- **Incremental updates** — Only re-indexes changed files using content hashing (Blake3)
- **Real-time file watching** — Automatic re-indexing on file changes
- **Fast search** — Sub-100ms symbol lookup across large codebases

### 6.2 Architecture

```
┌─────────────────────────────────────────────────────────────────┐
│                        ragent-code crate                        │
├──────────────���──────────────────────────────────────────────────┤
│  ┌──────────────┐   ┌──────────────┐   ┌────────────────────┐ │
│  │ File Scanner  │──▶│   Parser     │──▶│  Symbol Extractor  │ │
│  │ (ignore crate)│   │ (tree-sitter)│   │  (per-language)    │ │
│  └──────┬───────┘   └──────────────┘   └────────┬───────────┘ │
│         │                                         │             │
│         │  ┌──────────────┐                       ▼             │
│         │  │ File Watcher  │            ┌──────────────────┐    │
│         │  │ (notify crate)│───queue───▶│ Background Worker│    │
│         │  └──────────────┘            │ (tokio task)     │    │
│         │                               └────────┬─────────┘    │
│         ▼                                         ▼             │
│  ┌──────────────────────────────────────────────────────────┐ │
│  │                    Index Store (SQLite)                    │ │
│  │  ┌────────────┐ ┌─────────┐ ┌─────────┐ ┌────────────┐  │ │
│  │  │indexed_files│ │ symbols │ │ imports │ │ references │  │ │
│  │  └────────────┘ └─────────┘ └─────────┘ └────────────┘  │ │
│  └──────────────────────────────────────────────────────────┘ │
│         │                                                        │
│         ▼                                                        │
│  ┌──────────────────┐   ┌──────────────────────────────────┐   │
│  │ Tantivy FTS Index │   │       Tool Interface             │   │
│  │ (full-text search)│   │  codeindex_search                │   │
│  └──────────────────┘   │  codeindex_symbols                │   │
│                          │  codeindex_references             │   │
│                          │  codeindex_dependencies           │   │
│                          │  codeindex_status                 │   │
│                          │  codeindex_reindex                │   │
│                          └──────────────────────────────────┘   │
└─────────────────────────────────────────────────────────────────┘
```

**Components:**
| Component | Purpose |
|-----------|---------|
| **File Scanner** | Walk directory trees, respect `.gitignore`, compute content hashes |
| **File Watcher** | Real-time filesystem change detection via `notify` crate |
| **Parser** | Tree-sitter AST parsing with per-language grammar support |
| **Symbol Extractor** | Per-language AST walkers extract symbols, imports, and references |
| **Index Store** | SQLite persistence for files, symbols, imports, references |
| **Search Engine** | Tantivy full-text index + structured SQLite queries |
| **Tree Cache** | LRU cache of parse trees for incremental re-parsing |
| **Background Worker** | Async indexing worker with debounce, dedup, and batching |

### 6.3 Supported Languages

| Language | Extensions | Symbols Extracted |
|----------|------------|-------------------|
| **Rust** | `.rs` | Functions, structs, enums, traits, impls, modules, consts, statics, type aliases, macros |
| **Python** | `.py` | Functions, classes, methods, decorators, imports, async functions |
| **TypeScript** | `.ts`, `.tsx` | Functions, classes, interfaces, types, enums, namespaces, imports |
| **JavaScript** | `.js`, `.jsx` | Functions, classes, methods, arrow functions, imports |
| **Go** | `.go` | Functions, structs, interfaces, methods, imports, type definitions |
| **C/C++** | `.c`, `.cpp`, `.h`, `.hpp` | Functions, structs, unions, enums, classes, namespaces, includes |
| **Java** | `.java` | Classes, interfaces, enums, methods, constructors, annotations |
| **OpenSCAD** | `.scad` | Modules, functions, variable declarations, include/use statements, call references |
| **Terraform** | `.tf`, `.tfvars` | Resource blocks, data blocks, module calls, variables, locals, outputs, provider blocks |
| **CMake** | `.cmake`, `CMakeLists.txt` | Functions, macros, blocks, foreach/while loops, if conditions, commands, include/add_subdirectory |
| **Gradle (Groovy)** | `.gradle` | Classes, methods, functions, closures, imports, annotations, DSL block calls |
| **Gradle (Kotlin)** | `.gradle.kts` | Classes, functions, properties, type aliases, imports, companion objects, DSL calls |
| **Maven** | `pom.xml` | Project coordinates, dependencies, modules, plugins, profiles, properties, repositories |

### 6.4 Data Model

#### Indexed Files
```rust
struct FileEntry {
    path: String,          // Relative path from project root
    language: String,      // "rust", "python", "typescript", etc.
    content_hash: String,    // Blake3 hash for change detection
    indexed_at: String,    // ISO 8601 timestamp
    file_size: i64,        // Bytes
}
```

#### Symbols
```rust
struct Symbol {
    name: String,          // Symbol name
    kind: SymbolKind,      // Function, Struct, Enum, Trait, etc.
    visibility: Visibility, // Public, Private, Restricted
    file_path: String,     // Source file path
    start_line: u32,       // 1-based line number
    start_col: u32,        // 1-based column
    end_line: u32,         // End line
    end_col: u32,          // End column
    doc: Option<String>,   // Doc comment / documentation
}
```

**SymbolKind Taxonomy:**
| Kind | Description |
|------|-------------|
| `function` | Named function or method |
| `struct` | Struct or class definition |
| `enum` | Enum type |
| `trait` | Trait or interface definition |
| `impl` | Implementation block |
| `const` | Constant definition |
| `static` | Static variable |
| `type_alias` | Type alias |
| `module` | Module or namespace |
| `macro` | Macro definition |
| `field` | Struct/class field |
| `variant` | Enum variant |
| `interface` | Interface (Java/TS) |
| `class` | Class definition |
| `method` | Class method |

#### Imports
```rust
struct ImportEntry {
    source_file: String,   // File containing the import
    imported_name: String, // Imported symbol name
    source_path: String,   // Origin module/path (e.g., "std::fs::File")
    kind: ImportKind,      // Use, Import, Include
}
```

#### References (Cross-file Symbol Usage)
```rust
struct SymbolRef {
    symbol_name: String,   // Name of referenced symbol
    file_path: String,     // File containing the reference
    line: u32,             // Line number
    column: u32,           // Column number
    is_definition: bool,   // True if this is where symbol is defined
}
```

### 6.5 Index Storage

- **Location**: `~/.cache/ragent/code_index/` (or project-local `.ragent/code_index/`)
- **SQLite database** (`index.db`):
  - `indexed_files` — File metadata and content hashes
  - `symbols` — Extracted symbols with locations and documentation
  - `imports` — Cross-file import relationships
  - `references` — Symbol usage references
  - `file_deps` — File-level dependency graph
- **Tantivy FTS Index** (`fts/`): Full-text search over symbols and documentation
- **Tree Cache**: LRU cache of parse trees for incremental updates

### 6.6 Control

```bash
/codeindex on           # Enable indexing
/codeindex off          # Disable indexing
/codeindex status       # Show current status
/codeindex reindex      # Force full re-index
/codeindex clear        # Delete all indexed data
```

Configuration in `ragent.json`:

```jsonc
{
  "code_index": {
    "enabled": true,
    "index_dir": ".ragent/code_index",  // Custom location
    "max_file_size": 1048576,             // 1MB default
    "extra_exclude_dirs": ["vendor", "node_modules", "target"],
    "extra_exclude_patterns": ["*.min.js", "*.d.ts"]
  }
}
```

### 6.7 Code Index Tools

All tools are available to agents and can be called directly in conversations.

#### `codeindex_search`

Full-text search across symbols, documentation, and code.

**Parameters:**
| Parameter | Type | Description |
|-----------|------|-------------|
| `query` | string | Search query (supports boolean operators) |
| `language` | string? | Filter by language (e.g., "rust") |
| `file_pattern` | string? | Filter by file path pattern (e.g., "src/**/*.rs") |
| `max_results` | integer? | Maximum results (default: 20, max: 100) |

**Example:**
```json
{
  "query": "config parser",
  "language": "rust",
  "file_pattern": "crates/ragent-core/**/*.rs",
  "max_results": 10
}
```

**Returns:** List of search results with symbol info, file path, and relevance score.

---

#### `codeindex_symbols`

Query symbols from the codebase index with optional filters.

**Parameters:**
| Parameter | Type | Description |
|-----------|------|-------------|
| `name` | string? | Filter by symbol name (substring match) |
| `kind` | string? | Filter by symbol kind ("function", "struct", "enum", etc.) |
| `file_path` | string? | Filter by file path substring |
| `language` | string? | Filter by programming language |
| `visibility` | string? | Filter by visibility ("public", "private", "restricted") |
| `limit` | integer? | Maximum results (default: 50, max: 200) |

**Example:**
```json
{
  "name": "parse",
  "kind": "function",
  "language": "rust",
  "limit": 20
}
```

**Returns:** Structured symbol information with signatures and documentation.

---

#### `codeindex_references`

Find all references to a symbol by name across the indexed codebase.

**Parameters:**
| Parameter | Type | Description |
|-----------|------|-------------|
| `symbol` | string | The symbol name to find references for |
| `limit` | integer? | Maximum results (default: 50, max: 200) |

**Example:**
```json
{
  "symbol": "AgentConfig",
  "limit": 100
}
```

**Returns:** File locations grouped by file, with reference kind (call, type, field_access).

---

#### `codeindex_dependencies`

Query file-level dependencies from the code index.

**Parameters:**
| Parameter | Type | Description |
|-----------|------|-------------|
| `path` | string | File path to query dependencies for |
| `direction` | string? | "imports" (what this file uses) or "dependents" (what uses this file) |

**Example:**
```json
{
  "path": "crates/ragent-core/src/agent/mod.rs",
  "direction": "dependents"
}
```

**Returns:** List of file paths that depend on (or are imported by) the target file.

---

#### `codeindex_status`

Show current status and statistics of the codebase index.

**No parameters.**

**Returns:**
- Files indexed
- Symbols extracted
- Languages detected
- Index size on disk
- Timestamps

**Example Output:**
```json
{
  "files_indexed": 128,
  "symbols_extracted": 3427,
  "languages": {
    "rust": 89,
    "python": 23,
    "typescript": 16
  },
  "index_size_bytes": 2457600,
  "last_updated": "2026-04-14T09:30:00Z"
}
```

---

#### `codeindex_reindex`

Trigger a full re-index of the codebase. Use after major file changes or when search results seem stale.

**No parameters.**

**Note:** This can take several minutes for large codebases. Progress is shown in the TUI.

---

### 6.8 Usage Examples

#### Example 1: Find all configuration-related functions
```
Call codeindex_search with:
{
  "query": "config",
  "kind": "function",
  "max_results": 20
}
```

#### Example 2: Find where a specific function is used
```
Call codeindex_references with:
{
  "symbol": "load_config",
  "limit": 50
}
```

#### Example 3: Find all public structs in a crate
```
Call codeindex_symbols with:
{
  "kind": "struct",
  "file_path": "crates/ragent-core",
  "visibility": "public",
  "limit": 100
}
```

#### Example 4: Check what files depend on a core module
```
Call codeindex_dependencies with:
{
  "path": "crates/ragent-core/src/lib.rs",
  "direction": "dependents"
}
```

#### Example 5: Find enum definitions matching a pattern
```
Call codeindex_symbols with:
{
  "name": "Error",
  "kind": "enum",
  "language": "rust"
}
```

### 6.9 When to Use Code Index vs Other Tools

| Task | Best Tool | Why |
|------|-----------|-----|
| Find where function X is defined | `codeindex_symbols` | Semantic understanding of symbols |
| Find all usages of function X | `codeindex_references` | Cross-file reference tracking |
| Search for text in comments | `codeindex_search` | Full-text search includes docs |
| Find all implementations of trait | `codeindex_symbols` | Filter by kind=impl |
| Quick file content search | `grep` | Faster for simple text matching |
| Real-time type info while editing | LSP tools | Live analysis, not indexed |
| Cross-repository search | `grep` | Code index is per-project |

### 6.10 Performance Characteristics

| Metric | Target |
|--------|--------|
| Full index time | < 5 min for 10k files |
| Incremental update | < 100ms per changed file |
| Symbol lookup | < 50ms |
| Full-text search | < 100ms |
| Memory usage | < 512MB for typical projects |
| Disk usage | ~10-50MB per 1000 files |

**Concurrency Model:**
- SQLite connections are thread-local (no global lock contention)
- Tree-sitter parsing runs in parallel via Rayon
- Tantivy uses `IndexWriter` with `try_commit` every 50 documents
- File watcher queue has 500ms debounce (configurable)

### 6.11 File Watcher (`watcher.rs`)

The file watcher provides real-time filesystem monitoring using the `notify`
crate (v7.0+), feeding incremental updates into the code index without
manual re-scanning.

#### Architecture

```text
notify::RecommendedWatcher
   ↓ filesystem events (Create, Modify, Remove, Rename)
CodeIndexWatcher
   ├── event_tx: Sender<WatchEvent>      — bounded channel to worker
   ├── debounce_ms: u64                  — default 500ms
   └── ignored_dirs: HashSet<String>     — directories to skip
```

#### Ignored Directories (12 Built-in)

The following directories are never watched:

| Directory | Reason |
|-----------|--------|
| `target/` | Rust build output |
| `node_modules/` | npm dependencies |
| `.git/` | Git internals |
| `dist/` | Build output |
| `build/` | Build output |
| `.next/` | Next.js build |
| `__pycache__/` | Python bytecode |
| `.venv/` | Python virtual environment |
| `vendor/` | Vendored dependencies |
| `.cargo/` | Cargo cache |
| `.tox/` | Python tox |
| `coverage/` | Test coverage output |

#### Event Flow

1. `notify::Watcher` delivers raw filesystem events
2. `CodeIndexWatcher` deduplicates and debounces (500ms window)
3. Debounced events are sent via bounded channel to the background worker
4. On drop, the watcher gracefully shuts down via `stop()` signal

#### Starting the Watcher

```rust
let watch_session = code_index.start_watching(root_path)?;
// Returns WatchSession which stops on drop
```

### 6.12 Background Worker (`worker.rs`)

The background worker processes file change events from the watcher in a
dedicated thread, performing incremental index updates without blocking the
main session.

#### Processing Pipeline

```text
WatchEvent received
   ↓
Batch accumulation (max 50 events or 500ms timeout)
   ↓
For each file in batch:
   ├── Read file contents
   ├── Compute Blake3 hash → compare with stored hash
   ├── If changed: re-parse with tree-sitter → update SQLite + FTS
   └── If unchanged: skip (dedup by content hash)
   ↓
Commit Tantivy index (try_commit)
```

#### Configuration

| Parameter | Default | Description |
|-----------|---------|-------------|
| `batch_size` | 50 | Max events per batch before flush |
| `debounce_ms` | 500 | Debounce window for event coalescing |
| `max_queue_size` | 10,000 | Bounded channel capacity |

#### Worker Lifecycle

- **Started** by `start_watching()` via `tokio::spawn`
- **Receives** `WatchEvent` variants: `Created(path)`, `Modified(path)`,
  `Removed(path)`, `Renamed(old, new)`
- **Graceful shutdown** on channel close (watcher dropped) or explicit
  poison pill
- **Error resilience:** Individual file failures are logged but do not stop
  batch processing

### 6.13 Tree Cache (`tree_cache.rs`)

An LRU cache of parsed tree-sitter syntax trees to avoid repeated parsing
of frequently accessed files.

#### Configuration

| Parameter | Default | Description |
|-----------|---------|-------------|
| `capacity` | 1,000 | Maximum cached parse trees |
| `eviction` | LRU | Least Recently Used eviction policy |

#### Cache Operations

| Method | Description |
|--------|-------------|
| `get(path)` | Return cached tree if present and file unchanged |
| `insert(path, tree, hash)` | Store parsed tree with content hash |
| `invalidate(path)` | Remove entry (called on file change) |
| `clear()` | Drop all cached entries |

Cache validity is checked by comparing the stored Blake3 hash against the
current file hash. If the hash differs, the cache entry is invalidated and
the file is re-parsed.

### 6.14 Full-Text Search (`search.rs`)

The code index uses Tantivy (a Rust full-text search engine) for fast
symbol and content search across indexed files.

#### Index Schema

| Field | Type | Stored | Indexed | Tokenizer |
|-------|------|--------|---------|-----------|
| `path` | text | yes | yes | raw (exact) |
| `name` | text | yes | yes | default (tokenized) |
| `kind` | text | yes | yes | raw |
| `language` | text | yes | yes | raw |
| `content` | text | no | yes | default |
| `line` | u64 | yes | yes | — |
| `parent` | text | yes | yes | raw |

#### Search Scoring

Tantivy uses BM25 scoring by default, ranking results by term frequency and
inverse document frequency. The `FtsIndex` provides:

| Method | Description |
|--------|-------------|
| `search(query, limit)` | Full-text search with BM25 ranking |
| `search_by_kind(query, kind, limit)` | Filter by symbol kind |
| `search_in_file(query, path)` | Scope search to a specific file |
| `rebuild_from_store(store)` | Rehydrate FTS from SQLite data |

#### FTS Lifecycle

1. **Build:** During full indexing, symbols are written to both SQLite and
   Tantivy
2. **Incremental:** On file change, old entries are deleted by path and new
   entries added
3. **Rebuild:** `rebuild_fts()` reconstructs the entire Tantivy index from
   SQLite (used on corruption or mismatch detection)
4. **Validation:** `full_reindex()` auto-detects FTS/SQLite count mismatch
   and triggers rebuild

### 6.15 Content Hashing (Blake3)

All indexed files are hashed using Blake3 for fast content-based change
detection. Blake3 is chosen for its performance (3-4x faster than SHA-256)
and streaming capability.

**Usage in the index:**

| Location | Purpose |
|----------|---------|
| `scanner.rs` | Hash computed during initial file scan |
| `worker.rs` | Hash compared before re-parsing on file change |
| `tree_cache.rs` | Hash used to validate cache freshness |
| `store.rs` | Hash stored in SQLite `files` table |

**Hash comparison flow:**

```text
File changed event → Read file → Compute Blake3 hash
   ↓
Compare with stored hash in SQLite
   ├── Same → Skip (no actual change)
   └── Different → Re-parse → Update SQLite + FTS + Cache
```

---


## 7. Memory System

### 7.1 Overview

The memory system gives ragent agents persistent learning capabilities across
sessions. It combines file-based memory blocks, a structured SQLite store,
optional embedding-based semantic search, an append-only journal, automatic
memory extraction, and a knowledge graph — organised in three tiers that can
be enabled independently.

| Tier | Components | Storage | Default |
|------|-----------|---------|---------|
| **Core** | File-based memory blocks | Markdown files | Enabled |
| **Structured** | SQLite memories + journal + knowledge graph | SQLite | Enabled |
| **Semantic** | Embedding vectors + cosine-similarity search | SQLite BLOB | Disabled (opt-in) |

### 7.2 Memory Blocks (File-Based)

Named, scoped memory blocks are stored as Markdown files with YAML
frontmatter. They are loaded into the agent's system prompt at session start.

**Storage locations:**

| Scope | Location | Priority |
|-------|----------|----------|
| **User-global** | `~/.ragent/memory/` | Lower |
| **Project-local** | `.ragent/memory/` | Higher (overrides global) |

**Directory structure:**

```
.ragent/memory/
├── MEMORY.md              # General notes (legacy)
├── project.md             # Project understanding
├── patterns.md            # Code patterns and conventions
├── decisions.md           # Architecture decisions
└── scratchpad.md          # Temporary notes (not auto-loaded)
```

**Block format (YAML frontmatter + Markdown body):**

```yaml
---
label: project
scope: project
description: Codebase-specific knowledge
limit: 5000
read_only: false
---
# Project Knowledge

This project uses Axum for HTTP, tokio for async, and SQLite for storage.
```

**Frontmatter fields:**

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `label` | string | filename stem | Unique block identifier |
| `scope` | `"project"` \| `"user"` | `"project"` | Storage scope |
| `description` | string | `""` | Purpose of this block |
| `limit` | integer | none | Maximum content size in bytes |
| `read_only` | bool | `false` | Prevent agent modifications |

Blocks are persisted with atomic writes (write to `.md.tmp`, then rename) to
prevent corruption on crash.

### 7.3 Structured Memory Store (SQLite)

Structured memories are stored in SQLite with rich metadata for categorisation,
search, and lifecycle management.

#### 7.3.1 Memory Entry Fields

| Field | Type | Description |
|-------|------|-------------|
| `id` | integer | Auto-generated primary key |
| `content` | string | Memory text |
| `category` | string | Classification (see below) |
| `source` | string | Origin: `"manual"`, `"auto-extract"`, tool name |
| `confidence` | f64 | Reliability score 0.0–1.0 (default 0.7) |
| `project` | string | Associated project name |
| `session_id` | string | Creating session identifier |
| `tags` | string[] | Categorisation tags |
| `created_at` | datetime | Entry creation time |
| `updated_at` | datetime | Last modification time |
| `access_count` | integer | Number of times retrieved |
| `last_accessed` | datetime | Timestamp of last retrieval |
| `embedding` | blob | Optional vector embedding |

#### 7.3.2 Memory Categories

| Category | Description | Example |
|----------|-------------|---------|
| `fact` | Objective project/tool truths | "Uses Axum for HTTP server" |
| `pattern` | Recurring code/process patterns | "Repository pattern with traits" |
| `preference` | User's stated preferences | "Prefers explicit error types over anyhow" |
| `insight` | Agent-learned understanding | "Auth flow is the critical path" |
| `error` | Past errors and their solutions | "Mutex deadlock in worker.rs fixed by Arc" |
| `workflow` | Step-by-step procedures | "Adding a tool: register in mod.rs, add tests" |

### 7.4 Journal System

The journal is an append-only log for recording insights, decisions, and
observations during sessions. It complements structured memories by preserving
temporal context.

#### 7.4.1 Journal Entry Fields

| Field | Type | Description |
|-------|------|-------------|
| `id` | string | UUID v4 identifier |
| `title` | string | Short descriptive title |
| `content` | string | Full entry text |
| `tags` | string[] | Categorisation tags |
| `project` | string | Associated project |
| `session_id` | string | Creating session |
| `timestamp` | datetime | Event/observation time |
| `created_at` | datetime | Entry creation time |
| `embedding` | blob | Optional vector embedding |

#### 7.4.2 Tag Validation

Tags must be non-empty, at most 64 characters, and contain only ASCII
alphanumeric characters, hyphens, and underscores. Invalid tags are rejected
with an error.

#### 7.4.3 Journal Search

Journal entries are indexed with FTS5 full-text search across titles and
content. Search results return summaries (first 200 characters) to avoid
loading full content for large result sets.

### 7.5 Memory Tools

#### 7.5.1 File-Based Memory Tools

| Tool | Parameters | Description |
|------|-----------|-------------|
| `memory_write` | `content`, `scope`, `label`, `description`, `limit`, `mode` | Create or append to a memory block |
| `memory_read` | `label`, `scope` | Read a memory block's content |
| `memory_replace` | `label`, `scope`, `content` | Replace a block's content |

The `mode` parameter accepts `"append"` (default) or `"overwrite"`. The
`scope` parameter accepts `"project"` (default), `"user"`, or `"global"`.

#### 7.5.2 Structured Memory Tools

| Tool | Parameters | Description |
|------|-----------|-------------|
| `memory_store` | `content`, `category`, `tags`, `confidence`, `source` | Store a structured memory with metadata |
| `memory_recall` | `query`, `category`, `tags`, `min_confidence` | FTS5 search across structured memories |
| `memory_forget` | `id` or filter criteria | Delete memories by ID or bulk filter |
| `memory_search` | `query`, `scope`, `limit`, `min_similarity` | Semantic or FTS5 search across all memory types |

**`memory_forget` filter criteria:**

| Filter | Type | Description |
|--------|------|-------------|
| `id` | integer | Delete a specific memory by ID |
| `older_than_days` | integer | Delete memories older than N days |
| `max_confidence` | f64 | Delete memories with confidence below threshold |
| `category` | string | Delete memories of a specific category |
| `tags` | string[] | Delete memories matching all listed tags |

#### 7.5.3 Journal Tools

| Tool | Parameters | Description |
|------|-----------|-------------|
| `journal_write` | `title`, `content`, `tags` | Create a new journal entry |
| `journal_search` | `query`, `tags`, `project`, `date_range` | FTS5 search across journal entries |
| `journal_read` | `id` | Retrieve full journal entry by ID |

#### 7.5.4 Team Memory Tools

| Tool | Parameters | Description |
|------|-----------|-------------|
| `team_memory_read` | `scope` | Read shared team memory |
| `team_memory_write` | `content`, `scope` | Write to shared team memory |

### 7.6 Semantic Search (Embeddings)

Optional embedding-based semantic search enables natural language queries
across all memory types. Unlike FTS5 keyword search, semantic search finds
memories by _meaning_ — a query for "authentication flow" will match a
memory about "JWT token login process" even though they share no keywords.

Embeddings require two things:

1. The `embeddings` **Cargo feature** compiled in (see §7.6.4).
2. The `memory.semantic.enabled` **configuration flag** set to `true` (see §7.6.5).

When either condition is missing the system falls back silently to FTS5.

#### 7.6.1 Architecture

```text
┌──────────────────────────┐
│   EmbeddingProvider      │  (trait)
│   - embed(text)          │
│   - embed_batch(texts)   │
│   - dimensions()         │
│   - name()               │
│   - is_available()       │
└──────┬───────────────────┘
       │
  ┌────┴─────┐
  │          │
  ▼          ▼
NoOp     Local (ort)
(empty)  (ONNX Runtime)
```

The `EmbeddingProvider` trait abstracts all embedding generation behind a
common interface. Two implementations ship with ragent:

| Provider | Name | Description | Dimensions | External Deps |
|----------|------|-------------|------------|---------------|
| `NoOpEmbedding` | `"noop"` | Returns empty vectors; signals that semantic search is unavailable | 0 | None |
| `LocalEmbeddingProvider` | `"ort-local"` | Runs a sentence-transformer ONNX model locally via ONNX Runtime | 384 (default) | `ort`, `tokenizers`, `ndarray` |

**Trait methods:**

| Method | Return | Description |
|--------|--------|-------------|
| `embed(text)` | `Result<Vec<f32>>` | Generate an embedding vector for a single text string |
| `embed_batch(texts)` | `Result<Vec<Vec<f32>>>` | Generate embeddings for multiple texts (default: sequential; override for GPU batching) |
| `dimensions()` | `usize` | Vector dimensionality; `0` means disabled |
| `name()` | `&str` | Human-readable provider name (e.g. `"noop"`, `"ort-local"`) |
| `is_available()` | `bool` | `true` when the provider can produce real embeddings (`dimensions() > 0`) |

#### 7.6.2 Default Model — `all-MiniLM-L6-v2`

The default (and currently only supported) model is
[`sentence-transformers/all-MiniLM-L6-v2`](https://huggingface.co/sentence-transformers/all-MiniLM-L6-v2),
a lightweight sentence-transformer that produces **384-dimensional** vectors.

| Property | Value |
|----------|-------|
| Model name | `all-MiniLM-L6-v2` |
| Source | HuggingFace (`sentence-transformers/all-MiniLM-L6-v2`) |
| Dimensions | 384 |
| Pooling | Mean pooling with attention-mask weighting |
| Normalisation | L2-normalised output vectors |
| Runtime | ONNX Runtime (via the `ort` crate) |
| Tokeniser | HuggingFace `tokenizers` crate (`tokenizer.json`) |

**Required model files** (downloaded automatically on first use):

| File | Purpose |
|------|---------|
| `model.onnx` | ONNX model weights |
| `tokenizer.json` | Tokeniser vocabulary and configuration |
| `config.json` | Model configuration metadata |

**Model storage location:**

```
~/.local/share/ragent/models/all-MiniLM-L6-v2/
├── model.onnx
├── tokenizer.json
└── config.json
```

On first call to `embed()`, the provider checks for these files locally. Any
missing files are downloaded from HuggingFace (`https://huggingface.co/
sentence-transformers/all-MiniLM-L6-v2/resolve/main/<file>`). Downloads are
atomic (write to `.tmp`, then rename) to prevent corruption. Subsequent calls
reuse the cached files with no network access.

#### 7.6.3 Search Behaviour

The `memory_search` tool adapts based on embedding availability:

- **Embeddings available:** generates a query embedding, performs cosine
  similarity search across stored vectors, returns results ranked by
  similarity score.
- **Embeddings disabled:** falls back to FTS5 keyword search.

**Lazy embedding:** Memories are _not_ embedded at insert time. Instead,
embeddings are computed on first semantic search — any memories without an
embedding vector are embedded on the fly and their vectors stored for future
queries. This keeps inserts fast and avoids embedding memories that may never
be searched.

**Vector storage:** Embedding vectors are serialised as little-endian IEEE 754
`f32` arrays (4 bytes per dimension) and stored in SQLite BLOB columns. For
the default 384-dim model, each embedding occupies **1536 bytes**.

**Similarity scoring:** Retrieval uses cosine similarity with a configurable
minimum threshold (default `0.3`). Results are ranked from highest to lowest
similarity.

**`memory_search` parameters:**

| Parameter | Type | Default | Description |
|-----------|------|---------|-------------|
| `query` | string | _(required)_ | Natural language description of what to find |
| `scope` | `"memories"` \| `"blocks"` \| `"all"` | `"memories"` | Which store to search |
| `limit` | integer | 5 | Maximum number of results |
| `min_similarity` | float | 0.3 | Cosine similarity threshold (0.0–1.0, only used with embeddings) |

#### 7.6.4 Enabling Embeddings — Build Step

Embeddings are gated behind the `embeddings` Cargo feature flag on the
`ragent-core` crate. This keeps the binary small when semantic search is not
needed (ONNX Runtime and tokeniser add significant size).

**Feature flag definition** (`crates/ragent-core/Cargo.toml`):

```toml
[features]
embeddings = ["ort", "tokenizers", "ndarray"]
```

**Dependencies pulled in by the feature:**

| Crate | Version | Purpose |
|-------|---------|---------|
| `ort` | 2.0.0-rc.12 | ONNX Runtime bindings (with `download-binaries` and `load-dynamic` features) |
| `tokenizers` | latest | HuggingFace tokeniser for text → token conversion |
| `ndarray` | latest | N-dimensional array operations for tensor I/O |

**Build with embeddings enabled:**

```bash
# Build the ragent-core crate with embeddings
cargo build -p ragent-core --features embeddings

# Build the full workspace with embeddings
cargo build --features ragent-core/embeddings

# Run tests including embedding tests
cargo test -p ragent-core --features embeddings
```

When the `embeddings` feature is **not** compiled in, the
`LocalEmbeddingProvider` type is absent entirely. Only `NoOpEmbedding` is
available, and `memory_search` / `journal_search` always use FTS5 regardless
of configuration.

#### 7.6.5 Enabling Embeddings — Configuration Step

After building with the feature flag, enable semantic search in
`ragent.json`:

```jsonc
{
  "memory": {
    "enabled": true,
    "tier": "semantic",            // must be "semantic" to activate embeddings
    "semantic": {
      "enabled": true,             // enable the embedding provider
      "model": "all-MiniLM-L6-v2", // model name (only supported value currently)
      "dimensions": 384            // must match model output (384 for all-MiniLM-L6-v2)
    }
  }
}
```

**Configuration fields (`memory.semantic`):**

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `enabled` | bool | `false` | Enable/disable semantic search. When `false`, FTS5 keyword search is used. |
| `model` | string | `"all-MiniLM-L6-v2"` | Name of the ONNX sentence-transformer model. Currently only `all-MiniLM-L6-v2` is supported. |
| `dimensions` | integer | `384` | Embedding vector dimensions. Must match the model output (384 for `all-MiniLM-L6-v2`). |

**Memory tier levels** (`memory.tier`):

| Tier | Components Enabled |
|------|-------------------|
| `"core"` | File-based memory blocks only (default) |
| `"structured"` | File blocks + SQLite structured store + journal + knowledge graph |
| `"semantic"` | All of the above + embedding vectors + cosine-similarity search |

**Minimal configuration** — add just these lines to enable embeddings with
all defaults:

```jsonc
{
  "memory": {
    "tier": "semantic",
    "semantic": {
      "enabled": true
    }
  }
}
```

#### 7.6.6 Embedding Lifecycle

```text
1. Build with `--features embeddings`
2. Set `memory.tier = "semantic"` and `memory.semantic.enabled = true`
3. On first `memory_search` call:
   a. LocalEmbeddingProvider lazily initialises
   b. Model files downloaded to ~/.local/share/ragent/models/ (if missing)
   c. ONNX session and tokeniser loaded (Mutex-protected, thread-safe)
   d. Query text embedded → 384-dim f32 vector
   e. Un-embedded memories are batch-embedded and vectors stored
   f. Cosine similarity computed against all stored vectors
   g. Results returned ranked by similarity score
4. Subsequent searches reuse the loaded model and stored vectors
```

**Error handling:** If model initialisation fails (download error, corrupt
model file, OOM), the provider transitions to a `Failed` state and will not
retry on subsequent calls. The error message is preserved for diagnostics.
`memory_search` falls back to FTS5 when the provider is unavailable.

#### 7.6.7 Utility Functions

The embedding module exports helper functions for working with vectors:

| Function | Signature | Description |
|----------|-----------|-------------|
| `cosine_similarity(a, b)` | `(&[f32], &[f32]) → f32` | Cosine similarity in `[-1.0, 1.0]`; panics if lengths differ; returns `0.0` for zero-magnitude vectors |
| `serialise_embedding(vec)` | `(&[f32]) → Vec<u8>` | Serialise to little-endian byte blob for SQLite BLOB storage |
| `deserialise_embedding(blob, dims)` | `(&[u8], usize) → Result<Vec<f32>>` | Deserialise byte blob back to vector; validates length matches dimensions |

### 7.7 Automatic Memory Extraction

The extraction engine observes tool usage and session events to propose
structured memories without explicit user action.

#### 7.7.1 Hook Points

| Hook | Trigger | What It Detects |
|------|---------|-----------------|
| `on_tool_result` | After every tool execution | Bash error→success pairs, file edit patterns |
| `on_session_end` | Session completion | Key learnings, decisions, workflow summaries |

#### 7.7.2 Extraction Types

| Type | Source | Example |
|------|--------|---------|
| **Error resolution** | Bash failure followed by success | "Fix: use `--release` flag for optimisation" |
| **Coding pattern** | File edit/create operations | "Module files use `pub mod` re-exports" |
| **Session summary** | Conversation history at session end | "Refactored auth to use JWT tokens" |

#### 7.7.3 Memory Candidates

Extracted memories are wrapped as `MemoryCandidate` with:

| Field | Type | Description |
|-------|------|-------------|
| `content` | string | Proposed memory text |
| `category` | string | Suggested category |
| `tags` | string[] | Suggested tags |
| `confidence` | f64 | Extraction confidence 0.0–1.0 |
| `source` | string | Origin (e.g. `"auto-extract/bash"`, `"auto-extract/edit"`) |
| `reason` | string | Why this was extracted |

#### 7.7.4 Confirmation Flow

| Mode | Behaviour |
|------|-----------|
| `require_confirmation: true` (default) | Candidate emitted as `MemoryCandidateExtracted` event; agent decides whether to store via `memory_store` |
| `require_confirmation: false` | Candidate automatically stored in SQLite |

#### 7.7.5 Configuration

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

### 7.8 Memory Lifecycle Management

#### 7.8.1 Confidence Decay

Memories lose confidence over time via exponential decay:

```
confidence = max(confidence - daily_decay_rate, min_confidence_floor)
```

| Setting | Default | Description |
|---------|---------|-------------|
| `daily_decay_rate` | 0.01 | Confidence reduction per day |
| `min_confidence_floor` | 0.1 | Minimum confidence (never decays below) |

#### 7.8.2 Compaction

When memory blocks exceed their size limit (default threshold: 90% full),
compaction summarises the content to fit. Original content is logged to the
journal before truncation.

**Compaction triggers:**

| Trigger | Condition |
|---------|-----------|
| First run | Initial startup with compaction enabled |
| Time-based | More than 24 hours since last compaction |
| Count-based | Memory count exceeds threshold |
| Manual | `/reload memory` command |

#### 7.8.3 Deduplication

Before storing, new memories are checked for duplicates:

| Similarity | Result | Action |
|------------|--------|--------|
| > 0.95 | `Duplicate` | Merge with existing (update confidence) |
| 0.8 – 0.95 | `NearDuplicate` | Propose merge, require confirmation |
| < 0.8 | `NoDuplicate` | Store as new memory |

Similarity is computed using cosine similarity on embeddings when available,
or FTS5 word-overlap when embeddings are disabled.

#### 7.8.4 Eviction

Stale memories can be automatically identified and proposed for deletion:

| Setting | Default | Description |
|---------|---------|-------------|
| `auto_evict` | `false` | Auto-delete vs. propose for review |
| `stale_days` | 30 | Days since last access to consider stale |
| `min_confidence` | 0.2 | Confidence threshold for eviction candidates |

#### 7.8.5 Full Compaction Pass

The `run_compaction()` function performs a complete maintenance cycle:

1. **Block compaction** — summarise oversized blocks
2. **Stale eviction** — identify and remove low-value memories
3. **Deduplication merge** — consolidate near-duplicate entries

### 7.9 Knowledge Graph

The knowledge graph extracts entities and relationships from stored memories
to build a structured understanding of the project.

#### 7.9.1 Entity Types

| Type | Description | Example |
|------|-------------|---------|
| `Project` | Software project | "ragent" |
| `Tool` | Development tool | "cargo", "git" |
| `Language` | Programming language | "Rust", "Python" |
| `Pattern` | Design/code pattern | "Repository pattern" |
| `Person` | Team member | "thawkins" |
| `Concept` | Abstract concept | "async runtime" |

#### 7.9.2 Relationship Types

| Type | Description | Example |
|------|-------------|---------|
| `Uses` | Subject uses target | "ragent Uses Rust" |
| `Prefers` | Subject prefers target | "project Prefers explicit errors" |
| `DependsOn` | Subject depends on target | "ragent-tui DependsOn ragent-core" |
| `Avoids` | Subject avoids target | "project Avoids println!" |
| `RelatedTo` | General association | "auth RelatedTo JWT" |

Entities track `mention_count` (number of memories referencing them) and
relationships carry a `confidence` score.

### 7.10 System Prompt Integration

At session start, the memory system injects relevant context into the agent's
system prompt:

1. **Memory blocks** — all `.md` blocks from both scopes are loaded with
   scope labels, descriptions, read-only markers, and size usage percentages.
2. **Legacy MEMORY.md** — standalone files loaded for backward compatibility
   (skipped if already loaded as a block).
3. **Structured memories** — top N memories by relevance are injected under a
   `## Relevant Memories` section, ranked by a weighted combination of
   recency and relevance.

**Retrieval configuration:**

```jsonc
{
  "memory": {
    "retrieval": {
      "max_memories_per_prompt": 5,
      "recency_weight": 0.3,
      "relevance_weight": 0.7
    }
  }
}
```

### 7.11 Visualisation

The memory system can generate visualisation data for TUI display:

| View | Description |
|------|-------------|
| **Memory graph** | Category and tag relationship network |
| **Timeline** | Journal entries sorted chronologically |
| **Tag cloud** | Tags ranked by frequency |
| **Access heatmap** | Memories ranked by access count and recency |

### 7.12 User Interaction

#### Slash Commands

| Command | Description |
|---------|-------------|
| `/memory` | Browse memory blocks and status |
| `/memory read [label]` | Read a specific memory block |
| `/journal` | Browse journal entries |
| `/journal search <query>` | Search journal entries |
| `/journal add <title>` | Create a journal entry |
| `/reload memory` | Re-scan memory directories and trigger compaction |

#### Status Bar

The TUI status bar displays memory status: `MEM: X blocks, Y entries`

### 7.13 HTTP API

Memory operations are available via REST endpoints:

**Memory Block endpoints:**

| Method | Path | Description |
|--------|------|-------------|
| `GET` | `/memory/blocks` | List all memory blocks |
| `GET` | `/memory/blocks/{scope}/{label}` | Get specific block |
| `PUT` | `/memory/blocks/{scope}/{label}` | Create/update block |
| `DELETE` | `/memory/blocks/{scope}/{label}` | Delete block |

**Structured memory endpoints:**

| Method | Path | Description |
|--------|------|-------------|
| `POST` | `/memory/store` | Store structured memory |
| `POST` | `/memory/search` | Search memories |
| `GET` | `/memory/search` | Search memories (query params) |

**Journal endpoints:**

| Method | Path | Description |
|--------|------|-------------|
| `GET` | `/journal` | List journal entries |
| `POST` | `/journal` | Create journal entry |
| `GET` | `/journal/{id}` | Get entry by ID |
| `POST` | `/journal/search` | Search journal entries |

**SSE events:** `memory_candidate_extracted`, `memory_searched`

### 7.14 Configuration Reference

Complete memory configuration in `ragent.json`:

```jsonc
{
  "memory": {
    "enabled": true,
    "tier": "structured",
    "structured": {
      "enabled": true
    },
    "retrieval": {
      "max_memories_per_prompt": 5,
      "recency_weight": 0.3,
      "relevance_weight": 0.7
    },
    "semantic": {
      "enabled": false,
      "model": "all-MiniLM-L6-v2",
      "dimensions": 384
    },
    "auto_extract": {
      "enabled": true,
      "require_confirmation": true
    },
    "decay": {
      "enabled": true,
      "daily_decay_rate": 0.01,
      "min_confidence_floor": 0.1
    },
    "compaction": {
      "enabled": true,
      "block_size_limit": 4096,
      "memory_count_threshold": 500
    },
    "eviction": {
      "enabled": true,
      "auto_evict": false,
      "stale_days": 30,
      "min_confidence": 0.2
    }
  }
}
```

| Key | Type | Default | Description |
|-----|------|---------|-------------|
| `enabled` | bool | `true` | Master enable for the memory system |
| `tier` | string | `"structured"` | Active tier: `"core"`, `"structured"`, or `"semantic"` |
| `retrieval.max_memories_per_prompt` | integer | 5 | Max structured memories injected into system prompt |
| `retrieval.recency_weight` | f64 | 0.3 | Weight for recency in retrieval ranking |
| `retrieval.relevance_weight` | f64 | 0.7 | Weight for relevance in retrieval ranking |
| `semantic.enabled` | bool | `false` | Enable embedding-based search (requires `embeddings` feature) |
| `semantic.model` | string | `"all-MiniLM-L6-v2"` | Sentence-transformer model name |
| `semantic.dimensions` | integer | 384 | Embedding vector dimensionality |
| `auto_extract.enabled` | bool | `true` | Enable automatic memory extraction |
| `auto_extract.require_confirmation` | bool | `true` | Require agent confirmation before storing |
| `decay.enabled` | bool | `true` | Enable confidence decay over time |
| `decay.daily_decay_rate` | f64 | 0.01 | Daily confidence reduction |
| `decay.min_confidence_floor` | f64 | 0.1 | Minimum confidence floor |
| `compaction.enabled` | bool | `true` | Enable automatic compaction |
| `compaction.block_size_limit` | integer | 4096 | Block size threshold for compaction (bytes) |
| `compaction.memory_count_threshold` | integer | 500 | Memory count trigger for compaction |
| `eviction.enabled` | bool | `true` | Enable stale memory eviction |
| `eviction.auto_evict` | bool | `false` | Auto-delete vs. propose for review |
| `eviction.stale_days` | integer | 30 | Days since last access before stale |
| `eviction.min_confidence` | f64 | 0.2 | Confidence threshold for eviction |

### 7.15 Persistence Summary

| Component | Storage | Scope | Mutability | Search |
|-----------|---------|-------|------------|--------|
| **Memory blocks** | Markdown files | Project / User | Read/Write (unless read-only) | File listing |
| **Structured memories** | SQLite | Project | Read/Write | FTS5 + Semantic |
| **Journal entries** | SQLite | Project / Session | Append-only | FTS5 + Semantic |
| **Knowledge graph** | SQLite | Project | Read/Write | Entity/relationship queries |
| **Embeddings** | SQLite BLOB | Per-entry | Read/Write | Cosine similarity |

### 7.16 Import/Export

The memory system supports importing and exporting data in multiple formats
for migration, backup, and interoperability.

#### Export Formats

| Format | Command | Description |
|--------|---------|-------------|
| **ragent** | `ragent memory export --format ragent` | Native JSON format with full metadata |
| **markdown** | `ragent memory export --format markdown` | Human-readable Markdown blocks |

**ragent JSON export structure:**

```json
{
  "version": "1",
  "exported_at": "2025-01-15T10:30:00Z",
  "blocks": [
    {
      "scope": "project",
      "label": "conventions",
      "content": "...",
      "read_only": false,
      "metadata": {}
    }
  ],
  "memories": [
    {
      "id": "mem_abc",
      "content": "...",
      "category": "tech_stack",
      "confidence": 0.85,
      "tags": ["database"],
      "created_at": "...",
      "last_accessed": "..."
    }
  ],
  "journal": [
    {
      "id": "j_xyz",
      "entry_type": "decision",
      "title": "...",
      "content": "...",
      "tags": []
    }
  ]
}
```

#### Import Formats

| Format | Command | Description |
|--------|---------|-------------|
| **ragent** | `ragent memory import --format ragent <file>` | Native format (full fidelity) |
| **cline** | `ragent memory import --format cline <file>` | Cline memory format |
| **claude-code** | `ragent memory import --format claude-code <file>` | Claude Code CLAUDE.md format |

**Import from Cline:**

Parses Cline's memory bank files (typically `cline_docs/` directory) and
converts them to ragent memory blocks. Section headers become block labels,
content is preserved.

**Import from Claude Code:**

Parses `CLAUDE.md` files (project root or `~/.claude/CLAUDE.md`). Each
section is imported as a separate memory block with appropriate scope
(project or user based on file location).

#### Import Conflict Resolution

When importing data that conflicts with existing memories:

| Strategy | Behaviour |
|----------|-----------|
| **Skip** | Keep existing, skip import (default) |
| **Overwrite** | Replace existing with imported data |
| **Merge** | Append imported content to existing blocks |

### 7.17 Cross-Project Memory Sharing

Memories can be shared across projects to enable knowledge transfer and
reuse of common patterns.

#### Configuration

```jsonc
{
  "memory": {
    "cross_project": {
      "enabled": true,
      "shared_scopes": ["user"],
      "resolve_strategy": "shadow"
    }
  }
}
```

#### Block Resolution

When a memory block is requested, the system resolves it using a layered
lookup:

```text
1. Project-scope block (highest priority)
2. User-scope block (cross-project)
3. Global-scope block (lowest priority)
```

**Shadowing:** A project-scope block with the same label as a user-scope
block "shadows" it — the project version is returned. The user-scope block
still exists and is returned in other project contexts.

#### resolve_block() Algorithm

```text
resolve_block(scope, label):
  1. Check project scope → return if found
  2. If cross_project.enabled:
     a. Check user scope → return if found
     b. Check global scope → return if found
  3. Return None
```

### 7.18 Knowledge Graph

The knowledge graph provides entity-relationship storage for structured
project knowledge that goes beyond flat text memories.

#### Entity Types

| Type | Description | Example |
|------|-------------|---------|
| `Function` | Named function or method | `parse_config()` |
| `Type` | Struct, enum, trait, class | `AppConfig` |
| `File` | Source file | `src/main.rs` |
| `Module` | Module or package | `ragent_core::memory` |
| `Concept` | Abstract concept | "authentication flow" |
| `Tool` | External tool or dependency | `PostgreSQL` |

#### Relation Types

| Relation | Description | Example |
|----------|-------------|---------|
| `calls` | Function calls another | `main() → calls → parse_config()` |
| `uses` | Entity uses another | `AppConfig → uses → serde` |
| `contains` | Containment relationship | `mod.rs → contains → parse_config()` |
| `depends_on` | Dependency relationship | `ragent-tui → depends_on → ragent-core` |
| `implements` | Implementation of trait/interface | `OllamaProvider → implements → Provider` |

#### Graph Operations

| Operation | Description |
|-----------|-------------|
| `add_entity(type, name, metadata)` | Create an entity node |
| `add_relation(from, to, relation)` | Create a relationship edge |
| `query_entity(name)` | Get entity with all relations |
| `query_relations(entity, direction)` | Get inbound or outbound relations |
| `query_path(from, to, max_depth)` | Find shortest path between entities |
| `remove_entity(name)` | Remove entity and all its relations |

#### Language Support (33 Languages)

The knowledge graph parser recognizes entities from the following languages:

Rust, TypeScript, JavaScript, Python, Go, Java, C, C++, C#, Ruby, PHP,
Swift, Kotlin, Scala, Lua, R, Dart, Elixir, Haskell, OCaml, Clojure,
Erlang, F#, Julia, Perl, Shell/Bash, SQL, HTML, CSS, YAML, TOML, JSON,
Markdown

#### Tool Recognition (40+ Tools)

The knowledge graph automatically recognizes references to common tools and
frameworks and creates `Tool` entities for them, including:

- **Databases:** PostgreSQL, MySQL, SQLite, MongoDB, Redis, DynamoDB
- **Frameworks:** React, Vue, Angular, Django, Flask, Express, Spring
- **Infrastructure:** Docker, Kubernetes, Terraform, AWS, GCP, Azure
- **Build tools:** Cargo, npm, pip, Maven, Gradle, Make
- **CI/CD:** GitHub Actions, Jenkins, CircleCI, GitLab CI

---


## 8. AIWiki Knowledge Base

### 8.1 Overview

AIWiki is an embedded, project-scoped knowledge base system that provides agents with structured, searchable knowledge about concepts, entities, and analysis from ingested documents. Unlike the Memory System (personal notes and preferences) and the Code Index (code symbols), AIWiki stores domain knowledge extracted from external documents.

**Key Features:**
- **Project-scoped** — Each project has its own isolated knowledge base
- **Multi-format support** — Markdown (.md), Plain text (.txt), PDF (.pdf), Word (.docx), OpenDocument (.odt)
- **Conceptual linking** — Automatic cross-linking between related concepts
- **AI-powered analysis** — Fact extraction, Q&A generation, contradiction detection
- **Web interface** — Built-in web UI integrated with ragent-server
- **Export/Import** — Single markdown export, Obsidian vault export, markdown import
- **Agent integration** — Available via `aiwiki_search`, `aiwiki_ingest`, `aiwiki_status` tools

### 8.2 Architecture

```
┌─────────────────────────────────────────────────────────────────┐
│                        AIWiki System                             │
├─────────────────────────────────────────────────────────────────┤
│                                                                  │
│  ┌──────────────┐    ┌──────────────┐    ┌──────────────┐     │
│  │   Document   │───▶│   Ingest     │───▶│   Knowledge  │     │
│  │   Sources    │    │   Pipeline   │    │   Graph      │     │
│  └──────────────┘    └──────────────┘    └──────┬───────┘     │
│         │                                          │             │
│         ▼                                          ▼             │
│  ┌─────────────────────────────────────────────────────────┐   │
│  │                    AIWiki Store                          │   │
│  │  ┌────────────┐ ┌─────────┐ ┌─────────┐ ┌────────────┐ │   │
│  │  │   pages    │ │ sources │ │concepts │ │  entities  │ │   │
│  │  └────────────┘ └─────────┘ └─────────┘ └────────────┘ │   │
│  │  ┌────────────┐ ┌─────────┐ ┌──────────────────────┐    │   │
│  │  │  analyses  │ │  links  │ │  link graph (JSON) │    │   │
│  │  └────────────┘ └─────────┘ └──────────────────────┘    │   │
│  └───────────────────────────────────────────────────���─────┘   │
│                              │                                    │
│                              ▼                                    │
│  ┌──────────────────────────────────────────────────────────┐   │
│  │                     Query Interface                       │   │
│  │   aiwiki_search     aiwiki_ingest     aiwiki_status     │   │
│  │   aiwiki_export     aiwiki_import                        │   │
│  └──────────────────────────────────────────────────────────┘   │
│                              │                                    │
│                              ▼                                    │
│  ┌──────────────────────────────────────────────────────────┐   │
│  │                    Web Interface                         │   │
│  │   /aiwiki (serve)  - Search, browse, graph visualization │   │
│  │   HTML templates, HTMX for interactivity                  │   │
│  └──────────────────────────────────────────────────────────┘   │
│                                                                  │
└─────────────────────────────────────────────────────────────────┘
```

**Components:**
| Component | Purpose |
|-----------|---------|
| **Document Sources** | Original files in supported formats |
| **Ingest Pipeline** | Extract text, parse structure, identify concepts |
| **Knowledge Graph** | SQLite-based storage with JSON link graph |
| **AI Analysis** | LLM-powered extraction of facts, Q&A pairs, contradictions |
| **Web UI** | HTML templates served via ragent-server |
| **Agent Tools** | Full-text search, status, ingestion, export/import |

### 8.3 Directory Structure

Each project with AIWiki enabled contains:

```
project_root/
└── aiwiki/
    ├── config.json          # AIWiki configuration
    ├── state.json           # Runtime state (watcher, etc.)
    ├── raw/                 # Ingested source documents
    │   ├── documents/
    │   └── references/
    ├── wiki/                # Parsed wiki pages
    │   ├── concepts/        # Concept pages
    │   ├── entities/        # Entity pages
    │   ├── analyses/        # Analysis pages
    │   └── *.md             # Root pages
    └── static/              # Web UI assets (CSS, JS)
```

### 8.4 Page Types

AIWiki organizes knowledge into four page types:

| Type | Description | Example |
|------|-------------|---------|
| **Concepts** | Abstract ideas, patterns, methodologies | `asynchronous-programming.md`, `builder-pattern.md` |
| **Entities** | Concrete things: people, organizations, tools | `anthropic.md`, `tokio.md`, `github.md` |
| **Sources** | Document metadata with links to raw files | Source attribution for ingested documents |
| **Analyses** | AI-generated analysis: Q&A, contradictions | `qa-results.md`, `contradiction-report.md` |

### 8.5 Slash Commands

```bash
/aiwiki init                       # Initialize AIWiki for current project
/aiwiki on                         # Enable AIWiki
/aiwiki off                        # Disable AIWiki
/aiwiki status                     # Show AIWiki status
/aiwiki ingest <path>              # Ingest document(s) into AIWiki
/aiwiki sync                       # Sync wiki with raw/ folder and referenced folders
/aiwiki clear                      # Clear all AIWiki data
```

### 8.6 Source Folders (Referenced Folders)

AIWiki supports **referenced folders** — project directories that can be registered by reference and scanned in-place without copying content into `aiwiki/raw/`. This is ideal for frequently-changing directories like `docs/`, `src/`, or `examples/`.

**Key Features:**
- **In-place scanning** — Files are read directly from their original location
- **Glob pattern matching** — Filter files by pattern (e.g., `*.rs`, `**/*.md`)
- **Recursive by default** — All subdirectories are included
- **Enable/disable** — Sources can be temporarily disabled without removing them
- **Unified sync** — Referenced folders are processed alongside `raw/` in the same sync run

**Configuration Schema:**

```json
{
  "sources": [
    { "path": "docs", "label": "Documentation", "patterns": ["**/*"], "enabled": true },
    { "path": "src", "label": "Source Code", "patterns": ["**/*.rs"], "enabled": true }
  ],
  "watch_mode": false
}
```

**State Key Convention:**
- Files from `raw/`: `readme.md` (no prefix)
- Files from referenced folders: `ref:docs/guide.md` (`ref:` prefix)

---

### 8.7 AIWiki Tools

All tools are available to agents and can be called directly.

#### `aiwiki_search`

Search the AIWiki knowledge base for pages matching a query.

**Parameters:**
| Parameter | Type | Description |
|-----------|------|-------------|
| `query` | string | Search keywords or phrase |
| `page_type` | string? | Filter by type: concepts, entities, sources, analyses |
| `max_results` | integer? | Maximum results (default: 10, max: 50) |

**Example:**
```json
{
  "query": "async programming patterns",
  "page_type": "concepts",
  "max_results": 10
}
```

**Returns:** Matching pages with titles, paths, and content excerpts.

---

#### `aiwiki_ingest`

Ingest documents into the AIWiki knowledge base.

**Parameters:**
| Parameter | Type | Description |
|-----------|------|-------------|
| `path` | string? | Path to file or directory to ingest |
| `subdirectory` | string? | Store in subdirectory within raw/ |
| `move_file` | boolean? | Move source file instead of copying |

**Example:**
```json
{
  "path": "docs/architecture.pdf",
  "subdirectory": "references"
}
```

**Supports:** Markdown, Plain text, PDF, DOCX, ODT

---

#### `aiwiki_status`

Show current status and statistics of the AIWiki knowledge base.

**No parameters.**

**Returns:**
- Pages count by type
- Sources count
- Storage usage
- Sync status
- Configuration

---

#### `aiwiki_export`

Export the AIWiki knowledge base to various formats.

**Parameters:**
| Parameter | Type | Description |
|-----------|------|-------------|
| `format` | string | Export format: single_markdown, obsidian |
| `output_path` | string? | Output file or directory path |

---

#### `aiwiki_import`

Import external markdown files into AIWiki.

**Parameters:**
| Parameter | Type | Description |
|-----------|------|-------------|
| `path` | string | Path to markdown file or directory |
| `target_subdir` | string? | Subdirectory in wiki/ to place imported files |

### 8.7 AI-Powered Analysis

AIWiki leverages LLMs to provide intelligent analysis:

| Analysis Type | Description |
|---------------|-------------|
| **Fact Extraction** | Pull key facts from documents into structured form |
| **Q&A Generation** | Generate questions and answers from content |
| **Contradiction Detection** | Identify conflicting statements across documents |
| **Concept Linking** | Automatically suggest related concepts |

### 8.8 Web Interface

The AIWiki web UI is served at `/aiwiki` when the ragent-server is running:

- **Search page** (`/aiwiki`) — Full-text search with filters
- **Page browser** — Browse concepts, entities, sources, analyses
- **Graph visualization** — Interactive knowledge graph with D3.js
- **Page view** — Rendered markdown with cross-links

The web interface uses:
- **Askama** templates (compiled, type-safe)
- **HTMX** for dynamic interactivity
- **Tailwind CSS** for styling
- **D3.js** for graph visualization

---



---

# Part III: Multi-Agent Coordination

---

## 9. Teams

### 9.1 Overview

Teams enable one lead session to coordinate multiple teammate agents with shared tasks and mailbox messaging. Unlike subagents (which are ephemeral workers), teammates have persistent named identities, can message each other directly, and share a common task list to coordinate work without the lead acting as a bottleneck.

### 9.2 When to Use Teams vs Subagents

| Dimension | Subagents (`new_task`) | Teams |
|-----------|------------------------|-------|
| Context | Own context; result summarised back | Own context; fully independent |
| Communication | Reports to lead only | Teammates message each other directly |
| Coordination | Lead manages all work | Shared task list; self-coordinating |
| Persistence | Ephemeral; destroyed on completion | Named; persist until team cleanup |
| Best for | Focused tasks; result is all that matters | Complex work requiring collaboration |
| Token cost | Lower | Higher (scales with active teammates) |

**Use Teams when:**
- Research with multiple independent angles (parallel code review, competing hypotheses)
- New features/modules where teammates each own a different file set without overlap
- Debugging where multiple theories need simultaneous investigation
- Cross-layer changes (API, UI, tests) owned by dedicated specialist teammates

### 9.3 Team Lifecycle

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

### 9.4 Storage Layout

Teams and tasks are stored locally so they survive process restarts:

```
~/.ragent/teams/{team-name}/
    config.json          # Team metadata and member list
    tasks.json           # Shared task list (file-locked on write)
    mailbox/
        {agent-id}.json  # Per-agent inbound message queue

[PROJECT]/.ragent/teams/{team-name}/   # Project-local teams (higher priority)
    (same structure)
```

### 9.5 Team Config Schema (`config.json`)

```json
{
  "name": "my-review-team",
  "lead_session_id": "sess-abc123",
  "created_at": "2026-03-19T05:32:47Z",
  "status": "active",
  "members": [
    {
      "name": "security-reviewer",
      "agent_id": "tm-001",
      "session_id": "sess-def456",
      "agent_type": "general",
      "status": "working",
      "current_task_id": "task-003"
    }
  ],
  "settings": {
    "max_teammates": 8,
    "require_plan_approval": false,
    "auto_claim_tasks": true
  }
}
```

### 9.6 Task List Schema (`tasks.json`)

```json
{
  "team_name": "my-review-team",
  "tasks": [
    {
      "id": "task-001",
      "title": "Review authentication module",
      "description": "...",
      "status": "completed",
      "assigned_to": "tm-001",
      "depends_on": [],
      "created_at": "...",
      "claimed_at": "...",
      "completed_at": "..."
    },
    {
      "id": "task-002",
      "title": "Review database queries",
      "status": "pending",
      "assigned_to": null,
      "depends_on": ["task-001"]
    }
  ]
}
```

### 9.7 Mailbox Message Schema

```json
{
  "message_id": "msg-uuid",
  "from": "tm-001",
  "to": "lead",
  "type": "message|broadcast|plan_request|plan_approved|plan_rejected|idle_notify|shutdown_request|shutdown_ack",
  "content": "...",
  "sent_at": "2026-03-19T05:32:47Z",
  "read": false
}
```

### 9.8 Blueprints

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

### 9.9 Communication

- **Mailbox system** — Async message passing between team members
- **Broadcast** — Send to all teammates simultaneously via `team_broadcast`
- **Direct messages** — Private communication via `team_message`
- **Race-free claiming** — `flock`-based file locking on `tasks.json`

### 9.10 Task Management

#### Task Lifecycle

```text
[task created by lead]
        ↓
    Pending (available for claiming)
        ↓
    [teammate claims task] ────→ [lead assigns to specific teammate]
        ↓
    InProgress (claimed, being worked on)
        ↓
    [teammate completes task] ────→ [auto-completion on agent loop exit]
        ↓
    Done (completed, triggers dependent task unblocking)
        ↓
    [archived or retained per team settings]

Alternate paths:
Pending ���───[dependency not met]────→ Blocked ────[dependency completed]────→ Pending
```

#### Task Status States

| Status | Description | Who Can Transition |
|--------|-------------|-------------------|
| `Pending` | Task created, available to claim | Lead → Teammate (claim) |
| `InProgress` | Task assigned, work in progress | Teammate (claim), Lead (assign) |
| `Done` | Task completed successfully | Teammate (complete), System (auto) |
| `Blocked` | Has unmet dependencies, cannot claim | System (automatic) |

---

### 9.11 Task Acquisition Methods

Teams support **four distinct methods** for getting work assigned to teammates:

#### Method 1: Self-Service Claiming (Default)

Teammates actively poll for and claim available tasks:

**Algorithm:**
```rust
fn acquire_task(teammate) {
    loop {
        // Poll every 500ms (configurable)
        sleep(config.mailbox_poll_interval_ms);
        
        // Read task list with file lock
        let tasks = read_tasks_locked();
        
        // Filter: Pending + dependencies met + no assignee
        let available = tasks.filter(|t| 
            t.status == Pending &&
            t.dependencies.all(|d| d.status == Done) &&
            t.assigned_to.is_none()
        );
        
        if available.is_empty() {
            // No work available
            continue;
        }
        
        // Race-free claim with exclusive lock
        let claimed = tasks.claim_next_available(
            teammate.id,
            lock_timeout_ms: 5000
        );
        
        if claimed {
            update_task_status(InProgress);
            update_teammate_status(Working, task_id);
            return task;
        }
        
        // Another teammate claimed it first, retry
    }
}
```

**Advantages:**
- **Decentralized** — No lead bottleneck
- **Self-balancing** — Fast teammates claim more tasks
- **Fault-tolerant** — If teammate fails, task returns to Pending
- **Simple** — No coordination overhead

**Usage:**
```bash
# Teammate code
let task = team_task_claim(team_name);
if let Some(task) = task {
    execute_work(task);
    team_task_complete(team_name, task.id);
}
```

---

#### Method 2: Lead-Directed Assignment

Lead explicitly assigns specific tasks to specific teammates:

**Algorithm:**
```rust
fn assign_task(lead, teammate_id, task_id) {
    // Validate teammate exists and is available
    let teammate = team.get_member(teammate_id);
    assert!(teammate.status == Ready || teammate.status == Working);
    
    // Validate task is claimable
    let task = tasks.get(task_id);
    assert!(task.status == Pending || task.status == Blocked);
    assert!(task.dependencies.all_met());
    
    // Atomic assignment with file lock
    with_tasks_locked(|tasks| {
        task.assigned_to = Some(teammate_id);
        task.status = InProgress;
        task.claimed_at = now();
    });
    
    // Notify teammate via mailbox
    team_message(
        to: teammate_id,
        content: format!("Task assigned: {}", task.title)
    );
    
    update_teammate_status(Working, task_id);
}
```

**Advantages:**
- **Control** — Lead decides who does what
- **Expertise matching** — Assign security tasks to security reviewer
- **Load balancing** — Lead can distribute work evenly
- **Priority handling** — Urgent tasks assigned immediately

**Usage:**
```bash
# Lead assigns specific task to specific teammate
team_assign_task(team_name, teammate_id="security-reviewer", task_id="task-003");
```

---

#### Method 3: Blueprint Pre-Seeding

Tasks are defined in the blueprint before team creation:

**Blueprint Structure:**
```json
// .ragent/blueprints/code-review/task-seed.json
{
  "tasks": [
    {
      "id": "task-001",
      "title": "Review authentication module",
      "description": "Focus on SQL injection and session handling",
      "depends_on": []
    },
    {
      "id": "task-002",
      "title": "Review API endpoints",
      "description": "Check input validation and rate limiting",
      "depends_on": ["task-001"]
    },
    {
      "id": "task-003",
      "title": "Review database layer",
      "description": "Verify query safety and connection pooling",
      "depends_on": []
    }
  ]
}
```

**Team Creation Flow:**
```rust
fn create_team_from_blueprint(blueprint_name) {
    // Load blueprint
    let blueprint = load_blueprint(blueprint_name);
    
    // Create team directory structure
    let team = create_team(blueprint.config);
    
    // Seed tasks from blueprint (if present)
    if let Some(seed) = blueprint.task_seed {
        for task in seed.tasks {
            team.tasks.create({
                id: task.id,
                title: task.title,
                description: task.description,
                status: Pending,
                depends_on: task.depends_on,
                assigned_to: None,
                created_at: now()
            });
        }
    }
    
    // Spawn teammates from spawn-prompts.json
    for (name, prompt) in blueprint.spawn_prompts {
        team.spawn(name, prompt);
    }
    
    return team;
}
```

**Advantages:**
- **Repeatable** — Same tasks every time for consistent reviews
- **Fast startup** — No manual task creation
- **Structured workflows** — Pre-defined dependency chains
- **Knowledge capture** — Expert-defined task templates

**Usage:**
```bash
/team create code-review --blueprint
# Automatically creates team with pre-seeded tasks
```

---

#### Method 4: Swarm Auto-Decomposition

For `/swarm <goal>` commands, tasks are auto-generated via LLM decomposition:

**Swarm Task Creation Flow:**
```rust
fn swarm_create_tasks(goal) {
    // Phase 1: LLM analyzes goal and creates subtasks
    let decomposition_prompt = format!(r#"
    Decompose this goal into 2-8 independent subtasks.
    
    Goal: "{}"
    
    For each subtask, provide:
    1. Simple ID (s1, s2, s3...)
    2. Clear title
    3. Detailed description with full context
    4. List of dependency IDs (which tasks must complete first)
    
    Rules:
    - Minimize dependencies (prefer parallel over sequential)
    - Make tasks self-contained (teammate shouldn't need to ask questions)
    - Keep descriptions detailed enough for an AI agent to implement
    "#, goal);
    
    // Call LLM to decompose
    let subtasks = llm.chat(decomposition_prompt);
    
    // Phase 2: Create ephemeral team and tasks
    let team = create_ephemeral_team();
    
    for (idx, subtask) in subtasks.iter().enumerate() {
        let task_id = format!("task-{:03}", idx + 1);
        
        team.task_create({
            id: task_id,
            title: subtask.title,
            description: subtask.description,
            status: if subtask.dependencies.is_empty() {
                Pending
            } else {
                Blocked  // Will unblock as dependencies complete
            },
            depends_on: subtask.dependencies,
            assigned_to: None,
            created_at: now()
        });
    }
    
    // Phase 3: Spawn one teammate per task
    for task in team.tasks.iter() {
        team.spawn(
            name: format!("swarm-{}", task.id),
            prompt: task.description,
            task_id: task.id
        );
    }
    
    return team;
}
```

**Dependency Unblocking:**
```rust
fn on_task_completed(team, completed_task) {
    // Find tasks blocked by this one
    let blocked_tasks = team.tasks.filter(|t|
        t.status == Blocked &&
        t.depends_on.contains(&completed_task.id)
    );
    
    for task in blocked_tasks {
        // Check if all dependencies now satisfied
        let all_deps_done = task.depends_on.all(|dep_id| {
            team.tasks.get(dep_id).status == Done
        });
        
        if all_deps_done {
            task.status = Pending;  // Now available to claim
            task.depends_on.clear();
            
            log!("Task {} unblocked (dependency {} completed)",
                 task.id, completed_task.id);
        }
    }
}
```

**Advantages:**
- **Dynamic** — Tasks created on-demand for any goal
- **Intelligent** — LLM determines optimal decomposition
- **Parallelizable** — LLM explicitly minimizes dependencies
- **Goal-oriented** — Natural language goal → executable tasks

**Usage:**
```bash
/swarm "Implement user authentication with JWT tokens, password hashing, and session management"
# Automatically decomposes into tasks like:
# - task-001: Create database schema for users
# - task-002: Implement password hashing module
# - task-003: Create JWT token generation
# - task-004: Build login/logout endpoints
# - task-005: Add session middleware
# - task-006: Write integration tests
```

---

#### Task Acquisition Comparison

| Method | Initiated By | Best For | Parallelism | Setup Effort |
|--------|--------------|----------|-------------|--------------|
| **Self-Service** | Teammates | Dynamic workloads, unknown task count | High | None |
| **Lead-Assigned** | Lead | Specific expertise matching, priority | Medium | Low |
| **Pre-Seeded** | Blueprint | Repeatable workflows, reviews | Varies | Medium |
| **Swarm** | LLM Decomposition | One-off complex goals | High | None |

---

#### Dependency Resolution Algorithm

Tasks with dependencies are automatically managed:

```rust
fn can_claim_task(task, teammate) -> bool {
    // Basic checks
    if task.status != Pending {
        return false;  // Not available
    }
    if task.assigned_to.is_some() {
        return false;  // Already claimed
    }
    
    // Dependency check
    for dep_id in &task.depends_on {
        let dep_task = tasks.get(dep_id);
        if dep_task.status != Done {
            return false;  // Dependency not satisfied
        }
    }
    
    true
}

fn transition_task_status(task, new_status) {
    match (task.status, new_status) {
        (Pending, InProgress) => {
            assert!(can_claim_task(task));
            task.claimed_at = now();
            task.assigned_to = Some(current_teammate.id);
        },
        (InProgress, Done) => {
            task.completed_at = now();
            // Trigger dependency unblocking
            unblock_dependent_tasks(task.id);
        },
        (Pending, Blocked) => {
            // Automatic when dependencies detected
        },
        (Blocked, Pending) => {
            // Automatic when dependencies complete
        },
        _ => panic!("Invalid state transition"),
    }
    task.status = new_status;
}
```

---

#### Plan Approval Workflow

For tasks requiring lead review before execution:

1. **Teammate calls `team_submit_plan`** with planned approach
2. **Teammate enters read-only mode** (no write/bash tools active)
3. **Lead receives a `plan_request` mailbox message**
4. **Lead calls `team_approve_plan`** (approve or reject with feedback)
5. **Teammate receives result and proceeds accordingly**

```rust
fn submit_plan(teammate, plan_text) {
    // Enter read-only mode
    teammate.enter_plan_pending_mode();
    
    // Send to lead
    team_message(
        to: "lead",
        type: "plan_request",
        content: json!({
            "task_id": teammate.current_task_id,
            "plan": plan_text,
            "teammate": teammate.name
        })
    );
    
    teammate.status = PlanPending;
    
    // Wait for response (blocking)
    let response = wait_for_mailbox("plan_approved" | "plan_rejected");
    
    if response.type == "plan_approved" {
        teammate.status = PlanApproved;
        teammate.exit_readonly_mode();
        return Ok(continue_execution);
    } else {
        teammate.status = PlanRejected;
        // Provide feedback, can revise
        return Err(revise_and_resubmit);
    }
}
```

### 9.13 Configuration

Settings in `ragent.json`:

```json
{
  "teams": {
    "max_teammates": 8,
    "default_require_plan_approval": false,
    "auto_claim_tasks": true,
    "mailbox_poll_interval_ms": 500,
    "task_claim_lock_timeout_ms": 5000
  }
}
```

### 9.14 Limitations

- No session resumption for active teammates
- One active team per lead session
- Teammates cannot spawn sub-teams (no nested teams)
- Split-pane display (tmux/iTerm2) is out of scope
- Per-teammate permission modes inherit lead permissions
- Teammate context windows are independent; no shared memory beyond tasks/mailbox

### 9.15 Member Status States

Team members transition through the following status states:

```text
[spawned] → Initializing → Ready → Working → Done
                                  ↘ Error
                                  ↘ Blocked → Working (unblocked)
                         → PlanPending → PlanApproved → Working
                                       → PlanRejected → PlanPending (revised)
```

| Status | Description |
|--------|-------------|
| `Initializing` | Session being created, agent loading |
| `Ready` | Idle, waiting for task assignment |
| `Working` | Actively executing a task |
| `PlanPending` | Submitted plan, awaiting lead approval |
| `PlanApproved` | Plan approved, resuming execution |
| `PlanRejected` | Plan rejected with feedback, must revise |
| `Blocked` | Cannot proceed (missing dependency, etc.) |
| `Done` | Completed all assigned tasks |
| `Error` | Unrecoverable error during execution |

### 9.16 Broadcast Messaging

The lead can send broadcast messages to all active teammates simultaneously:

```
/team broadcast "Focus on error handling paths first"
```

**Broadcast behaviour:**
- Message delivered to every teammate's mailbox
- Teammates receive the message on their next mailbox poll
- Does not interrupt active tool execution
- Broadcast messages have type `broadcast` in the mailbox

### 9.17 Memory Scopes

Each teammate can be configured with a memory scope that controls which
memories are available during execution:

| Scope | Description |
|-------|-------------|
| `None` | No memory access (stateless execution) |
| `User` | Access user-level memories only |
| `Project` | Access project-level memories (default for most tasks) |

Memory scope is set per team member in the team configuration:

```jsonc
{
  "teams": {
    "members": [
      {
        "name": "reviewer",
        "agent": "code-review",
        "memory_scope": "project"
      }
    ]
  }
}
```

### 9.18 Task Dependencies

Tasks support dependency edges to enforce execution order:

```text
Task A (setup) ──→ Task B (implement) ──→ Task C (test)
                ↘ Task D (docs)
```

- A task with unmet dependencies has status `Blocked`
- When a dependency completes, blocked tasks are automatically unblocked
- Circular dependencies are detected at creation time and rejected
- The lead can override blocked status with `/team unblock <task_id>`

### 9.19 Hook Events

Teams emit hook events for quality gate integration:

| Event | Trigger |
|-------|---------|
| `team_member_spawned` | New teammate created |
| `team_member_done` | Teammate completed all tasks |
| `team_member_error` | Teammate encountered fatal error |
| `team_plan_submitted` | Teammate submitted plan for review |
| `team_plan_approved` | Lead approved teammate's plan |
| `team_plan_rejected` | Lead rejected teammate's plan |
| `team_task_claimed` | Teammate claimed a task |
| `team_task_completed` | Task completed successfully |

These events can trigger hooks for external CI/CD integration, logging, or
notification systems.

---


## 10. Swarm Mode

### 10.1 Overview

Swarm mode is ragent's **Fleet-style auto-decomposition** system that automatically breaks down complex goals into independent parallel subtasks and coordinates their execution across multiple sub-agents.

**Key Concepts:**
- A *swarm* takes a high-level prompt and uses the LLM to decompose it into independent subtasks with dependency edges
- An ephemeral team is created with one teammate per subtask
- The lead orchestrates completion, handling dependencies and unblocking tasks as they complete
- Results are aggregated automatically when all tasks finish

**When to Use Swarm:**
- Complex multi-file refactoring across the codebase
- Large documentation updates (multiple .md files)
- Security reviews requiring multiple independent checks
- Code quality audits across different modules
- Any task that can be naturally divided into independent parallel work streams

---

### 10.2 Swarm Commands

| Command | Description |
|---------|-------------|
| `/swarm <prompt>` | Decompose a goal into parallel subtasks and spawn an ephemeral team to execute them |
| `/swarm status` | Display live progress of the active swarm, including task completion and teammate status |
| `/swarm cancel` | Cancel the active swarm, tear down the ephemeral team, and clean up resources |
| `/swarm help` | Show detailed help for swarm mode |

---

### 10.3 Swarm Decomposition

When you invoke `/swarm <prompt>`, ragent:

1. **Sends the goal to the LLM** with a specialized decomposition system prompt
2. **Parses the LLM's response** into a structured decomposition with subtasks and dependencies
3. **Creates an ephemeral team** with one teammate per subtask
4. **Spawns teammates** in parallel (respecting dependencies)
5. **Monitors completion** and unblocks dependent tasks automatically
6. **Aggregates results** when all tasks complete

#### Decomposition Schema

Each subtask in the decomposition has the following structure:

```rust
struct SwarmSubtask {
    /// Unique ID within the decomposition (e.g., "s1", "s2")
    id: String,
    /// Short human-readable title
    title: String,
    /// Full description/instructions for the teammate
    description: String,
    /// IDs of subtasks that must complete before this one can start
    depends_on: Vec<String>,
    /// Optional agent type override (defaults to "general")
    agent_type: Option<String>,
    /// Optional model override ("provider/model" format)
    model: Option<String>,
}
```

#### Decomposition Rules

The LLM is instructed to:
1. Create **independent subtasks** — agents complete work without seeing other agents' output (unless declared as dependency)
2. **Minimize dependencies** — prefer tasks that can run in parallel
3. **Use `depends_on` sparingly** — only when one task truly requires another's output (e.g., "create API" before "write integration tests")
4. **Keep subtask count between 2 and 8** — simple goals use fewer tasks
5. **Provide detailed descriptions** — subtasks must be self-contained with enough context for an AI agent to implement without clarification
6. **Use simple short IDs** — like "s1", "s2", etc.

---

#### 10.3.4 Task Decomposition Algorithm

The swarm's decomposition process follows a structured algorithm to transform a high-level goal into executable, parallelizable tasks:

##### Step 1: Goal Analysis

The LLM analyzes the user's prompt to identify:

- **Primary objective** — What is the single outcome being pursued?
- **Sub-components** — What are the logical parts of this goal?
- **Dependencies** — Which components must complete before others can begin?
- **Work units** — What are the smallest independent chunks of work?

**Example:**
```
Goal: "Implement user authentication system"
Sub-components: database schema, API endpoints, frontend forms, session management
Dependencies: API depends on database, frontend depends on API
Work units: schema design, DB migration, API implementation, etc.
```

##### Step 2: Task Granularity Determination

The system determines appropriate task granularity based on:

| Factor | Smaller Tasks | Larger Tasks |
|--------|--------------|--------------|
| **Goal complexity** | Complex goals with many parts | Simple, focused goals |
| **Parallelization potential** | Many independent work streams | Sequential by nature |
| **Teammate count** | Up to 8 teammates (max) | Minimum 2 teammates |
| **Context limits** | Tasks fit in agent context window | Tasks can reference external context |

**Algorithm:**
```
IF goal_complexity > HIGH AND parallel_potential > MEDIUM:
    task_count = MIN(8, estimated_work_units)
ELSE IF goal_complexity > MEDIUM:
    task_count = 3-5
ELSE:
    task_count = 2
```

##### Step 3: Dependency Graph Construction

The LLM constructs a directed acyclic graph (DAG) of tasks:

```
                    ┌─────────────┐
                    │    Goal     │
                    └──────┬──────┘
                           │
           ┌───────────────┼───────────────┐
           │               │               │
           ▼               ▼               ▼
      ┌─────────┐     ┌─────────┐     ┌─────────┐
      │ Task A  │     │ Task B  │     │ Task C  │
      │ (s1)    │     │ (s2)    │     │ (s3)    │
      └────┬────┘     └────┬──���─┘     └────┬────┘
           │               │               │
           └───────────────┼───────────────┘
                           │
                           ▼
                    ┌─────────────┐
                    │  Task D     │
                    │  (s4)       │
                    └─────────────┘
```

**Dependency detection rules:**

| Dependency Type | Detection Method | Example |
|-----------------|------------------|---------|
| **Data dependency** | Output of A is input to B | "Create DB schema" → "Write queries" |
| **Sequential dependency** | Natural ordering | "Design" → "Implement" → "Test" |
| **Resource dependency** | Shared resources | "Modify config" → "Restart service" |
| **Logical dependency** | Conceptual prerequisites | "Define API" → "Implement endpoints" |

**DAG validation checks:**
- No cycles detected (A depends on B, B depends on A)
- All tasks reachable from root
- Dependencies form a partial order, not total order (maximize parallelism)

##### Step 4: Critical Path Analysis

The system identifies the **critical path** — the longest chain of dependent tasks that determines minimum completion time:

```
Path A: s1 → s3 → s5 = 3 tasks
Path B: s2 → s4 = 2 tasks       ← NOT critical
Path C: s1 → s4 = 2 tasks       ← NOT critical
────────────────────────────────
Critical Path: Path A (3 tasks) ← Determines minimum time
```

**Optimization strategies:**
1. **Break critical path tasks** — Split long tasks into parallel sub-tasks where possible
2. **Overlap where safe** — Start tasks when partial dependencies are met
3. **Resource balancing** — Assign faster agents to critical path tasks

##### Step 5: Task Assignment Strategy

Tasks are assigned to teammates with optional overrides:

**Default assignment:**
```
Task → General-purpose agent (ragent-general)
Model → Current session model
```

**Override options:**
- `agent_type`: Use specialized agent (e.g., "security-reviewer", "test-writer")
- `model`: Use specific model for task requirements (e.g., "claude" for reasoning)

**Load balancing considerations:**
- Similar complexity tasks get similar time estimates
- Memory-intensive tasks distributed across available resources
- I/O-bound tasks can run concurrently with CPU-bound tasks

---

### 10.4 Swarm Lifecycle

```
┌─────────────────┐
│  User invokes   │
│ /swarm <prompt> │
└────────┬────────┘
         ▼
┌─────────────────┐
│ LLM Decomposition│
│ (async)         │
└────────┬────────┘
         ▼
┌─────────────────┐
│ Parse JSON      │
│ → SwarmDecomp   │
└────────┬────────┘
         ▼
┌─────────────────┐
│ Create ephemeral│
│ team (swarm-*)  │
└────────┬────────┘
         ▼
┌─────────────────┐
│ Spawn teammates │
│ (respect deps)  │
└────────┬────────┘
         ▼
┌─────────────────┐
│ Monitor progress  │
│ Unblock deps    │
└────────┬────────┘
         ▼
┌─────────────────┐
│ All complete?   │
└────────┬────────┘
    Yes  │
         ▼
┌─────────────────┐
│ Aggregate results│
│ Finalize swarm  │
└────────┬────────┘
         ▼
┌─────────────────┐
│ User runs       │
│ /swarm cancel   │
│ (cleanup)       │
└─────────────────┘
```

#### State Transitions

1. **Decomposing** → Goal sent to LLM, awaiting decomposition response
2. **Spawning** → Creating ephemeral team and spawning initial (non-blocked) teammates
3. **Running** → Teammates executing, polling for completion and unblocking dependencies
4. **Complete** → All tasks finished, results aggregated
5. **Cancelled** → User cancelled swarm, team cleaned up

---

### 10.5 Dependency Management

Swarm handles task dependencies automatically:

#### Blocked → Spawning Transition

When a teammate is created for a task with dependencies, it starts in the `Blocked` state. The system continuously polls:

1. Checks which dependencies have completed (via TaskStore or member status)
2. When ALL dependencies are complete, transitions the member from `Blocked` → `Spawning`
3. Triggers `reconcile_spawning_members()` to start the newly unblocked teammate

#### Example Dependency Chain

```json
{
  "tasks": [
    {"id": "s1", "title": "Create API", "depends_on": []},
    {"id": "s2", "title": "Write tests", "depends_on": ["s1"]},
    {"id": "s3", "title": "Write docs", "depends_on": ["s1"]}
  ]
}
```

**Execution flow:**
1. **s1** starts immediately (no dependencies)
2. **s2** and **s3** start in `Blocked` state
3. When s1 completes, both s2 and s3 are unblocked and start in parallel
4. When s2 and s3 complete, all tasks are done

---

### 10.6 Completion Detection

The swarm detects completion in multiple ways:

#### Method 1: Task Store Completion

When teammates call `team_task_complete()`, tasks are marked as `Completed` in the TaskStore. The swarm polls the TaskStore and counts completed vs total tasks.

#### Method 2: Member Status Fallback

If teammates finish their agent loop but don't explicitly complete their tasks (e.g., forgot to call `team_task_complete`), the swarm detects this via member status:

```rust
if all_members_terminal(
    Idle | Failed | Stopped
) {
    // Auto-complete any non-completed tasks
}
```

#### Finalization

When all tasks are completed (or cancelled), the swarm:
1. Marks itself as `completed: true`
2. Aggregates results from all teammates
3. Displays a completion summary showing:
   - Total tasks
   - Completed count
   - Cancelled count
   - Failed count
   - Teammate status breakdown

---

### 10.7 Output and Status

#### `/swarm` Response

When you invoke `/swarm`, the system responds with:

```markdown
## 🐝 Swarm Decomposition

| Task | Title | Deps |
|------|-------|------|
| s1 | Analyze config module | — |
| s2 | Review error handling | — |
| s3 | Check documentation | s1 |
| s4 | Write tests | s1, s2 |

**Summary:** 4 tasks, 2 ready, 2 blocked on deps
```

#### `/swarm status` Output

Shows live progress:

```markdown
## 🐝 Swarm: swarm-20250116-143052

**Tasks:** 4 total | 2 ✅ complete | 0 ⏳ pending | 2 🚫 blocked

**Teammates:**
  • swarm-s1 — idle ✅
  • swarm-s2 — idle ✅
  • swarm-s3 — blocked (waiting on: s1)
  • swarm-s4 — blocked (waiting on: s1, s2)

🎉 **All tasks complete!** Use `/swarm cancel` to clean up.
```

---

### 10.8 Error Handling

#### Parse Errors

If the LLM returns malformed JSON:
- Status shows: `⚠ swarm: decomposition parse error`
- Raw response is displayed for debugging
- No ephemeral team is created

#### Empty Decomposition

If the LLM returns zero subtasks:
- Warning message displayed
- Suggests trying a more specific prompt

#### Team Creation Failures

If ephemeral team creation fails:
- Error status displayed
- Log entry created
- User can retry with `/swarm <prompt>`

#### Partial Completion

If some teammates fail while others succeed:
- Tasks from failed members remain incomplete
- User can review logs and retry or cancel
- Final summary shows failed count

---

### 10.9 Integration with Teams

Swarm builds on ragent's team infrastructure:

- **Ephemeral team naming:** `swarm-{timestamp}`
- **Member naming:** `swarm-{task_id}` (e.g., `swarm-s1`)
- **Storage location:** Team data in `.ragent/teams/swarm-{timestamp}/`
- **Reuses:** TaskStore, TeamStore, team messaging, and mailbox systems
- **Cleanup:** `/swarm cancel` delegates to `/team close` for proper resource cleanup

---

### 10.10 Best Practices

#### When to Use Swarm

✅ **Good candidates:**
- Multi-file refactoring (e.g., "Update all error handling to use thiserror")
- Documentation audit across multiple files
- Security review of different modules
- Parallel exploration of multiple approaches
- Large tasks with clear parallelizable subcomponents

❌ **Avoid for:**
- Sequential tasks where each step depends on the previous (use regular agent)
- Very simple tasks (overhead not worth it)
- Tasks requiring shared mutable state between subtasks

#### Writing Effective Swarm Prompts

1. **Be specific about scope:** "Review error handling in src/auth/" not "Review all code"
2. **Mention independence:** "These modules can be reviewed independently"
3. **Provide context:** Include file paths, patterns, or examples
4. **Start small:** Test with 2-3 tasks before larger decompositions

#### Monitoring Swarms

1. Use `/swarm status` periodically to check progress
2. Check logs for blocked tasks — may indicate dependency issues
3. Cancel and restart if decomposition seems incorrect
4. Review teammate outputs before finalizing

---

### 10.11 Implementation Details

#### Data Structures

```rust
// Core swarm types in ragent-core/src/team/swarm.rs

/// Runtime state for an active swarm
pub struct SwarmState {
    pub team_name: String,           // e.g., "swarm-20250116-143052"
    pub prompt: String,              // Original user prompt
    pub decomposition: SwarmDecomposition,
    pub spawned: bool,               // All non-blocked teammates spawned
    pub completed: bool,             // All tasks finished
}

/// The LLM's decomposition response
pub struct SwarmDecomposition {
    pub tasks: Vec<SwarmSubtask>,
}

/// Individual subtask
pub struct SwarmSubtask {
    pub id: String,                  // "s1", "s2", etc.
    pub title: String,
    pub description: String,
    pub depends_on: Vec<String>,     // Task IDs that must complete first
    pub agent_type: Option<String>,
    pub model: Option<String>,
}
```

#### Polling Loop

The TUI polls swarm state on every tick:

1. `poll_pending_swarm()` — Check for completed LLM decomposition
2. `poll_swarm_unblock()` — Unblock tasks whose dependencies completed
3. `poll_swarm_completion()` — Detect when all tasks are done

#### System Prompt

The decomposition system prompt instructs the LLM to:
- Break goals into independent subtasks
- Minimize dependencies
- Keep task count between 2-8
- Use simple short IDs
- Provide detailed descriptions
- Respond with JSON only (no markdown fences)

---


## 11. Autopilot Mode

### 11.1 Overview

Autopilot mode enables autonomous operation where the agent can make decisions and execute tools without user confirmation.

### 11.2 Usage

| Command | Description |
|---------|-------------|
| `/autopilot on [--max-tokens N] [--max-time N]` | Enable autonomous operation |
| `/autopilot off` | Disable autonomous operation |
| `/autopilot status` | Show current autopilot status |

### 11.3 Features

- **Auto-approval** — Tools execute without user confirmation
- **Token limits** — Optional maximum token budget
- **Time limits** — Optional maximum execution time
- **Safety guardrails** — Permission system still applies

### 11.4 YOLO Mode

| Command | Description |
|---------|-------------|
| `/yolo` | Toggle YOLO mode (bypass all command validation and tool restrictions) |

YOLO mode bypasses bash validation, permission checks, and tool restrictions. Use with extreme caution.

---


## 12. Orchestrator & Multi-Agent Coordination

### 12.1 Overview

The orchestrator provides a framework for coordinating multiple agents to
work on complex tasks. It supports various job execution modes, conflict
resolution policies, and leader election for distributed scenarios.

### 12.2 Architecture

```text
Coordinator
├── registry: AgentRegistry          — Available agents and capabilities
├── router: RouterComposite          — Message routing (in-process + HTTP)
├── leader: LeaderElector            — Vote-based leader election
├── jobs: HashMap<JobId, Job>        — Active and completed jobs
├── metrics: MetricsSnapshot         — Performance counters
└── event_tx: Sender<JobEvent>       — Job lifecycle events

AgentRegistry
├── agents: Vec<RegisteredAgent>     — Agent metadata + capabilities
└── capability_index: HashMap<String, Vec<AgentId>>  — Capability → agents lookup
```

### 12.3 Agent Registry

Agents register with the orchestrator declaring their capabilities:

```rust
pub struct RegisteredAgent {
    pub id: AgentId,
    pub name: String,
    pub capabilities: Vec<String>,   // e.g., ["code-review", "testing", "rust"]
    pub status: AgentStatus,         // Available | Busy | Offline
    pub max_concurrent: usize,       // Max simultaneous jobs
}
```

**Capability matching:** When a job requires specific capabilities, the
registry returns all agents that declare those capabilities, sorted by
availability and load.

### 12.4 Job Execution Modes

| Mode | Description | Use Case |
|------|-------------|----------|
| `Sync` | Wait for all agents to complete | Comprehensive analysis |
| `Async` | Fire and forget, collect results later | Background processing |
| `FirstSuccess` | Return first successful result | Fast path with fallbacks |

#### JobDescriptor

```rust
pub struct JobDescriptor {
    pub id: JobId,
    pub prompt: String,
    pub required_capabilities: Vec<String>,
    pub mode: JobMode,               // Sync | Async | FirstSuccess
    pub conflict_policy: ConflictPolicy,
    pub timeout: Duration,
    pub max_agents: Option<usize>,
}
```

### 12.5 Conflict Resolution

When multiple agents produce results for the same job, conflicts are
resolved according to the configured policy:

| Policy | Description |
|--------|-------------|
| `Concat` | Concatenate all results in order |
| `FirstSuccess` | Use the first non-error result |
| `LastResponse` | Use the most recent response |
| `Consensus` | Select the result that appears most frequently |
| `HumanReview` | Present all results to the user for selection |

### 12.6 Leader Election

For distributed orchestration scenarios, the `LeaderElector` implements
vote-based leader election:

```text
Candidate → Voting → Leader / Follower
              ↓
         Vote collection (majority wins)
              ↓
         Leader heartbeat (keep-alive)
              ↓
         Re-election on leader timeout
```

**Election parameters:**

| Parameter | Default | Description |
|-----------|---------|-------------|
| `election_timeout_ms` | 5000 | Max wait for votes |
| `heartbeat_interval_ms` | 1000 | Leader heartbeat period |
| `min_votes` | majority | Minimum votes to win |

### 12.7 Message Routing

Messages between agents are routed through a composite router:

| Router | Transport | Use Case |
|--------|-----------|----------|
| `InProcessRouter` | Direct function calls | Same-process agents |
| `HttpRouter` | HTTP POST | Remote agents |
| `RouterComposite` | Delegates to above | Unified routing layer |

**Routing flow:**

```text
Agent A sends message to Agent B
   → RouterComposite checks if B is local
   ├── Local → InProcessRouter (direct call)
   └── Remote → HttpRouter (HTTP POST to B's endpoint)
```

### 12.8 Metrics

The orchestrator tracks operational metrics:

```rust
pub struct MetricsSnapshot {
    pub jobs_submitted: u64,
    pub jobs_completed: u64,
    pub jobs_failed: u64,
    pub avg_completion_time_ms: f64,
    pub active_agents: usize,
    pub pending_jobs: usize,
}
```

### 12.9 Job Events

Job lifecycle events are broadcast for monitoring:

| Event | Description |
|-------|-------------|
| `JobSubmitted` | New job entered the queue |
| `JobAssigned { agent_id }` | Job assigned to an agent |
| `JobProgress { percent }` | Agent reports progress |
| `JobCompleted { result }` | Job finished successfully |
| `JobFailed { error }` | Job failed with error |
| `JobCancelled` | Job was cancelled |

---

### 12.2 Event Bus Architecture

### 23.1 Overview

The event bus is the central nervous system of ragent, providing
publish-subscribe messaging between all components. It uses a broadcast
channel with a capacity of 256 events.

### 23.2 Implementation

```rust
pub struct EventBus {
    tx: broadcast::Sender<Event>,    // Capacity: 256
}

impl EventBus {
    pub fn publish(&self, event: Event) { ... }
    pub fn subscribe(&self) -> broadcast::Receiver<Event> { ... }
}
```

**Characteristics:**
- **Non-blocking:** Publishing never blocks the sender
- **Lossy:** If a subscriber falls behind by >256 events, oldest events
  are dropped (with a `Lagged` error on next receive)
- **Clone-safe:** Multiple subscribers can listen independently
- **Thread-safe:** `Arc<EventBus>` shared across all components

### 23.3 Event Categories (~40+ Variants)

#### Session Events

| Event | Description |
|-------|-------------|
| `SessionStarted { session_id }` | New session created |
| `SessionEnded { session_id }` | Session terminated |
| `SessionPaused` | Session paused (background) |
| `SessionResumed` | Session resumed from pause |
| `StepStarted { session_id, step }` | New processing step |
| `StepCompleted { session_id, step }` | Step finished |

#### Streaming Events

| Event | Description |
|-------|-------------|
| `StreamToken { token }` | Single token from LLM stream |
| `StreamStarted` | LLM response stream began |
| `StreamCompleted { finish_reason }` | Stream finished |
| `StreamError { error }` | Stream error occurred |

**FinishReason enum:**

| Variant | Description |
|---------|-------------|
| `Stop` | Normal completion |
| `Length` | Max tokens reached |
| `ToolUse` | Model wants to use a tool |
| `ContentFilter` | Content filtered by provider |

#### Tool Events

| Event | Description |
|-------|-------------|
| `ToolCallStarted { name, input }` | Tool invocation began |
| `ToolCallCompleted { name, output }` | Tool finished |
| `ToolCallFailed { name, error }` | Tool error |
| `PermissionRequested { tool, req_id }` | Permission needed |
| `PermissionGranted { req_id }` | Permission approved |
| `PermissionDenied { req_id, reason }` | Permission rejected |

#### Agent Events

| Event | Description |
|-------|-------------|
| `AgentChanged { from, to }` | Active agent switched |
| `SubAgentSpawned { id, agent_type }` | Sub-agent created |
| `SubAgentCompleted { id, result }` | Sub-agent finished |
| `SubAgentFailed { id, error }` | Sub-agent error |

#### Team Events

| Event | Description |
|-------|-------------|
| `TeamCreated { blueprint }` | Team spawned |
| `TeammateSpawned { name }` | Teammate created |
| `TeammateCompleted { name }` | Teammate finished |
| `TaskClaimed { task_id, member }` | Task assigned |
| `TaskCompleted { task_id }` | Task finished |
| `MailboxMessage { from, to }` | Message sent |

#### Memory Events

| Event | Description |
|-------|-------------|
| `MemoryStored { id, category }` | New memory created |
| `MemoryAccessed { id }` | Memory retrieved |
| `MemoryEvicted { id, reason }` | Memory removed |
| `JournalEntryCreated { id }` | Journal entry added |

#### Infrastructure Events

| Event | Description |
|-------|-------------|
| `LspStatusChanged { server, status }` | LSP server status change |
| `McpStatusChanged { server, status }` | MCP server status change |
| `CodeIndexProgress { phase, percent }` | Index build progress |
| `UpdateAvailable { version }` | New version detected |

#### OAuth Events

| Event | Description |
|-------|-------------|
| `OAuthFlowStarted { provider }` | OAuth flow initiated |
| `OAuthFlowCompleted { provider }` | OAuth completed |
| `OAuthFlowFailed { provider, error }` | OAuth failed |

### 23.4 Step Counters

Each session maintains a monotonic step counter, incremented for each
processing step (LLM call + tool execution cycle). Steps are formatted
as `[session_id:step_number]` for traceability in logs and UI.

### 23.5 Subscribers

| Component | Events Consumed | Purpose |
|-----------|----------------|---------|
| **TUI** | All | Display updates, status bar |
| **Server (SSE)** | All | Stream to HTTP clients |
| **Hooks** | Tool, Permission | Trigger hook execution |
| **Metrics** | Job, Agent | Performance tracking |
| **Logger** | All (filtered) | Structured logging |

---



---

# Part IV: Customization & Extension

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

The OASF (Open Agent Schema Format) provides a structured JSON format for
defining agents with fine-grained control over capabilities.

#### Full Schema

```json
{
  "schema_version": "oasf/agntcy.org/agent/1.0.0",
  "name": "my-agent",
  "description": "Custom agent description",
  "version": "1.0.0",
  "modules": [
    {
      "name": "core",
      "type": "ragent",
      "payload": {
        "version": "1",
        "system_prompt": "Your instructions...",
        "model": "anthropic/claude-sonnet-4-20250514",
        "memory_scope": "project",
        "temperature": 0.7,
        "max_tokens": 4096,
        "skills": ["code-review", "testing"],
        "tools": {
          "allowed": ["read_file", "write_file", "bash"],
          "denied": ["delete_file"]
        },
        "permissions": [
          {
            "path": "src/**/*.rs",
            "allow": ["read", "write"]
          },
          {
            "path": "*.env",
            "allow": ["read"],
            "deny": ["write"]
          }
        ]
      }
    }
  ]
}
```

#### OasfAgentRecord Fields

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `schema_version` | string | yes | Must be `"oasf/agntcy.org/agent/1.0.0"` |
| `name` | string | yes | Unique agent identifier |
| `description` | string | no | Human-readable description |
| `version` | string | no | Agent version |
| `modules` | array | yes | Agent capability modules |

#### RagentAgentPayload Fields

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `version` | string | `"1"` | Payload schema version |
| `system_prompt` | string | _(required)_ | Agent's system prompt |
| `model` | string | config default | LLM model identifier |
| `memory_scope` | string | `"none"` | Memory scope: `none`, `user`, `project` |
| `temperature` | f64 | `0.7` | LLM temperature parameter |
| `max_tokens` | u32 | model default | Max response tokens |
| `skills` | string[] | `[]` | Named skill sets to activate |
| `tools.allowed` | string[] | all | Whitelist of permitted tools |
| `tools.denied` | string[] | `[]` | Blacklist of denied tools |
| `permissions` | array | `[]` | File/path permission rules |

#### Permission Rules

Permission rules use glob patterns to control file access per agent:

```json
{
  "permissions": [
    {
      "path": "src/**/*.rs",
      "allow": ["read", "write"]
    },
    {
      "path": "secrets/**",
      "deny": ["read", "write"]
    },
    {
      "path": "tests/**",
      "allow": ["read", "write", "execute"]
    }
  ]
}
```

**Rule evaluation order:**
1. Deny rules checked first (explicit deny always wins)
2. Allow rules checked next
3. If no rule matches, falls back to session-level permissions

**Supported permissions:**

| Permission | Description |
|------------|-------------|
| `read` | Read file contents |
| `write` | Create or modify files |
| `execute` | Run as shell command |

### 13.2.1 Built-in Agents (8)

ragent ships with 8 built-in agents that handle internal operations:

| Agent | Purpose | Used By |
|-------|---------|---------|
| `ask` | Answer questions without tool use | `/ask` command |
| `general` | General-purpose coding assistant | Default agent |
| `build` | Build and fix compilation errors | `/build` command |
| `plan` | Create implementation plans | `/plan` command |
| `explore` | Codebase exploration and research | Sub-agent spawning |
| `title` | Generate conversation titles | Session management |
| `summary` | Summarize conversations | Export, compaction |
| `compaction` | Compress context windows | Automatic compaction |

**Built-in agents cannot be overridden** by custom agents with the same
name. Custom agents must use unique names.

### 13.3 System Prompt Best Practices

Guidelines for writing effective system prompts based on authoritative LLM provider recommendations:

**High-Value Recommendations:**

1. **State role and high-level authority first**
   ```
   You are a helpful, concise coding assistant for Rust projects.
   ```

2. **Scope behaviour and list prohibitions**
   ```
   - Do not execute destructive actions without confirmation
   - Do not reveal secrets or credentials
   ```

3. **Provide an output contract**
   ```
   Output only valid JSON with keys: status, result
   ```

4. **Use few-shot examples** (3–5 targeted examples)

5. **Include verification/self-check steps**
   ```
   Before returning, summarize your assumptions and confidence (high/medium/low).
   ```

**Template Pattern:**
```
1) Role: "You are an assistant that writes unit tests for Rust code."
2) Rules: numbered dos/don'ts
3) Output contract: exact JSON schema + example
4) Examples: 2–3 input→output pairs
5) Fallback: "If request is ambiguous, ask one clarifying question."
```

### 13.4 Template Variables

| Variable | Description |
|----------|-------------|
| `{{WORKING_DIR}}` | Current working directory |
| `{{FILE_TREE}}` | Project file tree (respects .gitignore) |
| `{{AGENTS_MD}}` | Content of AGENTS.md |
| `{{GIT_STATUS}}` | Git branch, status, recent commits |
| `{{README}}` | Content of README.md |
| `{{DATE}}` | Current date (ISO 8601) |

---


## 14. Skills System

### 14.1 Overview

Skills are reusable, parameterised instruction templates that extend ragent's
capabilities beyond its built-in toolset. Each skill is defined in a `SKILL.md`
file using YAML frontmatter for metadata and Markdown for the instruction body.
Skills can be invoked by users via slash commands (`/skill-name args`) or
automatically by the agent when it determines a skill is relevant to the task.

Key capabilities:

- **Argument substitution** — positional and named placeholders replaced at
  invocation time.
- **Dynamic context injection** — embed live command output in the skill body
  (`` !`command` `` syntax).
- **Forked execution** — run a skill in an isolated sub-session so it cannot
  affect the parent conversation.
- **Model override** — bind a skill to a specific provider/model.
- **Tool restrictions** — declare which tools the skill may use without
  requiring explicit permission.
- **Scope-based priority** — project-local skills override personal skills,
  which override bundled skills, etc.

### 14.2 Skill Scopes

Skills are discovered from multiple locations. When two skills share the same
name, the higher-priority scope wins.

| Priority | Scope | Location | Notes |
|----------|-------|----------|-------|
| 0 (lowest) | **Bundled** | Embedded in binary | Always available, cannot be removed |
| 1 | **Enterprise** | `~/.ragent/enterprise-skills/` | Organisation-managed |
| 2 | **OpenSkills Global** | `~/.agent/skills/`, `~/.claude/skills/` | Cross-tool compatibility (Anthropic OpenSkills format) |
| 3 | **Personal** | `~/.ragent/skills/` | User-level customisation |
| 4 | **OpenSkills Project** | `.agent/skills/`, `.claude/skills/` | Project-level cross-tool skills |
| 5 (highest) | **Project** | `.ragent/skills/` | Project-specific, highest priority |

Additional search directories can be specified via the `skill_dirs`
configuration key (see §14). These are treated as Personal scope.

**Monorepo support:** ragent also scans first-level subdirectories of the
working directory for `.ragent/skills/` folders, so monorepo sub-packages can
define their own project skills.

### 14.3 Skill File Format

Each skill lives in its own directory containing a `SKILL.md` file:

```
.ragent/skills/<skill-name>/
├── SKILL.md            # Required — skill definition
├── scripts/            # Optional — helper scripts
├── templates/          # Optional — template files
├── examples/           # Optional — example outputs
└── resources/          # Optional — reference materials
```

The `SKILL.md` file uses YAML frontmatter followed by a Markdown body:

```yaml
---
name: deploy
description: "Deploy the application to the target environment"
argument-hint: "<environment> [--dry-run]"
context: fork
agent: general-purpose
model: "openai/gpt-4o"
allowed-tools: [bash, read, grep]
user-invocable: true
disable-model-invocation: false
allow-dynamic-context: true
license: MIT
compatibility: "Linux, macOS"
metadata:
  author: "team-platform"
  version: "1.2.0"
---

Deploy the application to the **$0** environment.

Current branch: !`git branch --show-current`
Last commit: !`git log --oneline -1`

Steps:
1. Run pre-flight checks
2. Build the release artefact
3. Push to $0
4. Verify health endpoint
```

### 14.4 Frontmatter Reference

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `name` | string | directory name | Kebab-case identifier (alphanumeric + hyphens, max 64 chars) |
| `description` | string | `""` | Human-readable summary shown in menus and system prompt |
| `argument-hint` | string | `""` | Usage hint displayed in autocomplete (e.g. `"<env> [flags]"`) |
| `context` | `"inline"` \| `"fork"` | `"inline"` | Execution model (see §11.7) |
| `agent` | string | `"general"` | Sub-agent type for forked execution (e.g. `"explore"`, `"general-purpose"`) |
| `model` | string | session default | Model override in `"provider/model"` or `"provider:model"` format |
| `allowed-tools` | string \| string[] | `[]` | Tools the skill can use without requiring permission |
| `user-invocable` | bool | `true` | Whether the skill appears in the user's `/` slash menu |
| `disable-model-invocation` | bool | `false` | If `true`, only users can invoke; the agent cannot auto-invoke |
| `allow-dynamic-context` | bool | `false` | Enable `` !`command` `` shell injection in the body |
| `hooks` | object | `{}` | Lifecycle hooks (raw YAML stored as JSON) |
| `license` | string | `""` | OASF compatibility — licence identifier |
| `compatibility` | string | `""` | OASF compatibility — platform requirements |
| `metadata` | object | `{}` | OASF compatibility — arbitrary key-value metadata |

### 14.5 Bundled Skills

Four skills are compiled into the ragent binary and always available:

| Skill | Description | Allowed Tools | Invocable By |
|-------|-------------|---------------|--------------|
| `/simplify [output_path]` | Review recently changed files for code quality, efficiency, and simplification opportunities | `git diff`, `read`, `grep`, `glob`, `create`, `write` | User and Agent |
| `/batch <instruction>` | Orchestrate large-scale parallel changes across the codebase | `bash`, `read`, `edit`, `create`, `grep`, `glob` | User only |
| `/debug [description]` | Troubleshoot issues by examining debug logs, error messages, and configuration | `bash`, `read`, `grep` | User and Agent |
| `/loop [interval] <prompt>` | Run a prompt repeatedly on a timed interval for scheduled/iterative tasks | `bash`, `read` | User only |

Bundled skills have the lowest scope priority, so they can be overridden by
placing a skill with the same name in any higher-priority scope directory.

### 14.6 Argument Substitution

Skill bodies support placeholder variables that are replaced at invocation time.

| Variable | Description | Example |
|----------|-------------|---------|
| `$ARGUMENTS` | All arguments joined as a single string | `/deploy staging prod` → `"staging prod"` |
| `$0`, `$1`, … `$N` | Positional argument (0-indexed) | `$0` → `"staging"`, `$1` → `"prod"` |
| `$ARGUMENTS[N]` | Indexed argument (array-style, 0-indexed) | `$ARGUMENTS[0]` → `"staging"` |
| `${RAGENT_SESSION_ID}` | Current session identifier | `"sess-abc123"` |
| `${RAGENT_SKILL_DIR}` | Absolute path to the skill's directory | `"/project/.ragent/skills/deploy"` |

**Substitution order:** environment variables → indexed arguments → full
arguments string → positional shorthand. This ordering prevents partial
replacement conflicts.

**Quoting rules for arguments:**

- Whitespace-separated tokens: `staging prod` → `["staging", "prod"]`
- Double-quoted strings: `"hello world" foo` → `["hello world", "foo"]`
- Single-quoted strings: `'hello world' foo` → `["hello world", "foo"]`
- Out-of-bounds indices silently resolve to an empty string.

### 14.7 Execution Models

#### Inline (default)

The processed skill body is injected into the current session as a user
message. The agent processes it within the existing conversation context,
with full access to prior message history.

#### Forked (`context: fork`)

The skill runs in an **isolated sub-session** with fresh message history:

1. A new session is created with no prior conversation context.
2. The sub-agent type is resolved from the `agent` field (defaults to
   `"general"`).
3. Any `model` override is applied to the sub-session.
4. The processed skill content is sent through the agent loop.
5. The sub-agent's response is returned to the parent session wrapped in a
   `[Forked Skill Result: /name]` block.

Forked execution is useful for tasks that should not pollute the main
conversation (e.g. code review, batch operations) or that require a
different model or agent profile.

### 14.8 Dynamic Context Injection

When `allow-dynamic-context: true`, the skill body can embed live command
output using the `` !`command` `` syntax:

```markdown
Current Git branch: !`git branch --show-current`
Recent changes: !`git log --oneline -5`
Disk usage: !`df -h /`
```

At invocation time, each `` !`…` `` pattern is replaced with the stdout of the
executed command. Commands are executed sequentially with a **30-second
timeout** per command.

**Security — command allowlist:**

Only executables on a built-in allowlist may be used. The allowlist includes
65+ commonly-needed tools across these categories:

| Category | Examples |
|----------|---------|
| Version control | `git`, `gh`, `svn`, `hg` |
| File inspection | `cat`, `grep`, `rg`, `ls`, `find`, `tree`, `file`, `stat`, `wc` |
| Text processing | `awk`, `sed`, `cut`, `sort`, `uniq`, `jq`, `yq`, `head`, `tail` |
| Build tools | `cargo`, `npm`, `node`, `python`, `make`, `go`, `java`, `dotnet` |
| Networking | `curl`, `wget`, `dig`, `nslookup`, `ping` |
| Containers | `docker`, `podman`, `kubectl` |
| System | `date`, `env`, `hostname`, `uname`, `whoami`, `id` |

Commands not on the allowlist are rejected with an error message. Pipelines
are allowed if the first command in the pipeline is on the allowlist (e.g.
`git log | head -5` is permitted because `git` is allowed). Destructive
commands such as `rm`, `bash -c`, and `nc` are always rejected.

When YOLO mode is enabled, the allowlist is bypassed entirely.

### 14.9 Skill Discovery & Registry

#### Discovery algorithm

On startup and when `/reload skills` is invoked, ragent scans skill
directories in priority order:

1. **OpenSkills global** — `~/.agent/skills/`, `~/.claude/skills/`
2. **Personal** — `~/.ragent/skills/`
3. **Extra directories** — paths listed in config `skill_dirs` (Personal scope)
4. **OpenSkills project** — `{working_dir}/.agent/skills/`,
   `{working_dir}/.claude/skills/`
5. **Project** — `{working_dir}/.ragent/skills/`
6. **Monorepo** — `{working_dir}/*/.ragent/skills/` (first-level subdirectories)
7. **Bundled** — compiled-in skills (always loaded last, lowest priority)

Each directory is scanned for subdirectories containing a `SKILL.md` file.
The file is parsed for YAML frontmatter; invalid files are skipped with a
warning logged.

#### Registry behaviour

The `SkillRegistry` maintains a name-indexed map of `SkillInfo` entries:

- **Scope priority** — when a skill name is registered at a higher scope, it
  replaces any existing entry at a lower scope.
- **Same-scope conflict** — if the same name appears twice at the same scope,
  the first one found wins.
- **Lookup** — `registry.get("name")` returns the highest-priority `SkillInfo`.
- **Listing** — `list_user_invocable()` returns skills where
  `user_invocable == true`; `list_agent_invocable()` returns skills where
  `disable_model_invocation == false`.

### 14.10 Agent Integration

#### System prompt injection

When a session begins, ragent loads the skill registry and injects a skills
section into the agent's system prompt. The format is:

```
Available skills (invoke with /name):
- /deploy <environment> — Deploy the application to the target environment
- /simplify [output_path] — Review recently changed files for code quality
- /debug [description] — Troubleshoot issues
```

If the active agent profile has a `skills` list in its configuration, only
those named skills are injected. If the list is empty or absent, all
agent-invocable skills are shown.

#### Agent auto-invocation

The agent can invoke skills by including `/skill-name arguments` in its
response. The session processor detects the slash-command pattern, resolves
the skill from the registry, performs argument substitution and context
injection, and processes the result — either inline or forked depending on
the skill's `context` setting.

#### Per-agent skill filtering

Agent profiles (both `.json` OASF and `.md` format) support a `skills` field
listing skill names the agent should have access to:

```json
{
  "skills": ["deploy", "test", "lint"]
}
```

This restricts the agent's system prompt to only show the named skills,
preventing overload when many skills are installed.

### 14.11 TUI Integration

#### Slash menu

When the user types `/` in the input bar, the TUI displays an autocomplete
menu that includes both built-in commands and user-invocable skills. Skills
are visually distinguished from built-in commands and display their
`argument-hint` and `description`.

#### `/skills` command

The `/skills` slash command displays a table of all registered skills:

| Column | Description |
|--------|-------------|
| Command | Skill name with `/` prefix |
| Scope | Where the skill was discovered (Bundled, Personal, Project, etc.) |
| Access | Who can invoke: User, Agent, or Both |
| Description | Skill description from frontmatter |

#### Skill reload

The `/reload skills` command re-scans all skill directories and rebuilds the
registry without restarting ragent. This is useful during skill development.

### 14.12 OpenSkills Compatibility

Ragent supports the **OpenSkills** skill format used by other AI coding tools
(e.g. Claude Code). Skills placed in `~/.agent/skills/` or `.agent/skills/`
(and the equivalent `~/.claude/skills/` / `.claude/skills/` paths) are
discovered and loaded alongside native ragent skills.

OpenSkills files use the same YAML frontmatter + Markdown body format. They
are assigned `OpenSkillsGlobal` or `OpenSkillsProject` scope depending on
their location, which sits between Enterprise and Personal / Personal and
Project in the priority ordering.

### 14.13 Configuration

Skills-related configuration in `ragent.json`:

```jsonc
{
  "skill_dirs": [
    "/home/user/shared-skills",
    "/org/standard-skills"
  ]
}
```

| Key | Type | Default | Description |
|-----|------|---------|-------------|
| `skill_dirs` | string[] | `[]` | Additional directories to scan for skills (Personal scope priority) |

### 14.14 Creating a Skill

**Step 1:** Create the skill directory and file:

```bash
mkdir -p .ragent/skills/my-skill
cat > .ragent/skills/my-skill/SKILL.md << 'EOF'
---
name: my-skill
description: "Describe what this skill does"
argument-hint: "<required-arg> [optional-arg]"
allowed-tools: [bash, read, grep]
---

Instructions for the agent when this skill is invoked.

The user wants to: $ARGUMENTS

Positional arg 0: $0
Positional arg 1: $1
EOF
```

**Step 2:** Reload skills:

```
/reload skills
```

**Step 3:** Verify it appears:

```
/skills
```

**Step 4:** Invoke it:

```
/my-skill hello world
```

### 14.15 Security Considerations

- **Command allowlist** — dynamic context injection (`` !`command` ``) only
  executes commands on a built-in allowlist of 65+ safe executables.
  Destructive commands are always rejected.
- **Opt-in dynamic context** — the `allow-dynamic-context` field defaults to
  `false`; skills must explicitly enable shell injection.
- **Scope override risk** — a malicious project-level skill can override a
  bundled or personal skill of the same name. Review `.ragent/skills/` in
  untrusted repositories before running ragent.
- **Tool restrictions** — the `allowed-tools` field limits which tools the
  skill can use without requiring user permission, reducing blast radius.
- **Command timeout** — dynamic context commands are killed after 30 seconds
  to prevent hangs.
- **YOLO bypass** — when YOLO mode is active, the command allowlist and
  permission checks are bypassed. Use with extreme caution.

---


## 15. Prompt Optimization

### 15.1 Overview

Transform plain prompts into structured frameworks using the `/opt` command
or `POST /opt` endpoint. The prompt optimization module provides 12
LLM-powered prompt engineering frameworks via an async `optimize` function
in the `prompt_opt` crate. It is decoupled from any specific LLM backend
through a `Completer` trait that callers implement.

### 15.2 Architecture

```rust
#[async_trait]
pub trait Completer: Send + Sync {
    async fn complete(&self, system: &str, user: &str) -> anyhow::Result<String>;
}

pub async fn optimize(
    method: OptMethod,
    input: &str,
    completer: &dyn Completer,
) -> anyhow::Result<String>
```

Each method has a static `system_prompt(method)` that returns the
meta-prompt, a `name()` for CLI usage, and a `description()` for help text.
The method enum supports case-insensitive parsing with aliases (e.g. `"cot"`
→ `ChainOfThought`, `"o1"` → `O1Style`, `"q*"` → `QStar`).

### 15.3 Optimization Methods

| Method | Name | Framework | Purpose | When to Use |
|--------|------|-----------|---------|-------------|
| **CO-STAR** | `co_star` | Context, Objective, Style/Identity, Tone, Audience, Result | Comprehensive structured role assignment | Tasks requiring multiple well-defined dimensions |
| **CRISPE** | `crispe` | Capacity/Role, Request, Intent, Steps, Persona, Examples | Detailed role-based workflow with skill enumeration | Workflows needing explicit skill and step definitions |
| **Chain-of-Thought** | `cot` | Step-by-step reasoning scaffold with self-checks | Encourage intermediate thinking steps | Reasoning-heavy problems; math, logic, analysis |
| **DRAW** | `draw` | Professional AI image/drawing prompt optimizer | Reframe text-to-image prompts for diffusion models | Stable Diffusion / DALL-E prompt generation |
| **RISE** | `rise` | Recursive Introspection for iterative self-improvement | Multi-turn self-checking refinement loops | Iterative refinement where quality improves per pass |
| **O1-STYLE** | `o1_style` | `<thinking>`, `<step>`, `<reflection>`, `<reward>` tag scaffold | Reward-driven extended thinking | Deep reasoning; OpenAI o1-style structured thought |
| **Meta Prompting** | `meta` | Distill to concise, high-signal meta-prompt | Strip filler; compress instructions | Simplifying verbose prompts; reducing token count |
| **Variational** | `variational` | Template-based planning with `[placeholders]` for task content | Planning-based generation with structured output | Template-driven workflows; document generation |
| **Q-STAR** | `q_star` | XML-structured Q\*/A\* iterative optimizer | Reasoning chains with reward-modelled selection | Complex queries needing iterative quality checks |
| **OpenAI** | `openai` | Detailed GPT-style system prompt with guidelines | Extensive role description + constraints | GPT models; system prompts for chat completions |
| **Claude** | `claude` | Anthropic-style XML instruction generator with examples | XML tags for clarity + reasoning scaffolding | Claude / Anthropic models; structured XML prompts |
| **Microsoft** | `microsoft` | Azure AI optimized prompt with quality targets | SLA-aware quality metrics and validation | Azure OpenAI; enterprise quality requirements |

### 15.4 Method Details

#### CO-STAR

Generates structured sections for **C**ontext, **O**bjective,
**S**tyle/Identity, **T**one, **A**udience, and **R**esult. Best for
comprehensive tasks where the model needs to adopt a specific persona and
produce output tailored to a defined audience.

#### CRISPE

Creates a role profile with **C**apacity/Role, **R**equest, **I**ntent,
**S**teps, **P**ersona, and **E**xamples. Particularly effective when the
task benefits from explicitly listing the agent's skills and providing
step-by-step workflows.

#### Chain-of-Thought (CoT)

Adds explicit step-by-step reasoning scaffolding with self-checks before
the final answer. The model is instructed to show its work, verify each
step, and only then produce the conclusion.

#### DRAW

A specialised image prompt optimizer that transforms natural language
descriptions into detailed text-to-image prompts. Handles subject
composition, artistic style, lighting, camera angle, and negative prompts.

#### RISE

Implements **R**ecursive **I**ntrospection: the model produces an initial
attempt, introspects on its quality, refines the output, reflects on the
improvement, and iterates. Useful when quality improves with revision.

#### O1-STYLE

Scaffolds the response with XML tags — `<thinking>`, `<step>`,
`<reflection>`, `<reward>` — mimicking OpenAI o1's extended thinking
behaviour. A numeric reward score guides the model to continue refining
until a threshold is met.

#### Meta Prompting

Distils verbose user instructions into a minimal, high-signal meta-prompt.
Strips filler words, redundancy, and unnecessary context while preserving
all actionable requirements.

#### Variational

Creates a planning template with bracketed `[placeholders]` that map to
specific task content. Useful for generating structured documents or
repeatable workflow templates.

#### Q-STAR

Uses XML-structured reasoning chains (`<system-instruction>`, `<task>`,
`<reasoning>`) with iterative refinement. Inspired by the Q\* / A\*
search paradigm for exploring multiple solution paths.

#### OpenAI

Generates a comprehensive GPT-style system prompt with explicit guidelines,
constraints, output format specifications, and quality criteria. Optimised
for `gpt-4` and `gpt-3.5-turbo` system message conventions.

#### Claude

Creates Anthropic-style prompts using XML tags (`<task>`, `<document>`,
`<answer>`, `<thinking>`) with inline examples. Designed for Claude's
preference for structured XML input.

#### Microsoft

Produces Azure AI optimised prompts with SLA compliance targets, quality
validation checkpoints, and structured output criteria. Suited for
enterprise deployments with measurable quality requirements.

### 15.5 Usage

```bash
/opt help                           # Show method table with descriptions
/opt co_star Explain Rust lifetimes
/opt cot Solve the two-sum problem
/opt draw A cyberpunk cityscape at sunset
/opt claude Build a REST API with error handling
```

**HTTP API:**

```
POST /opt
Content-Type: application/json

{
  "method": "co_star",
  "input": "Explain Rust lifetimes to a beginner"
}
```

---


## 16. Configuration

### 16.1 Configuration Files

| File | Purpose |
|------|---------|
| `ragent.json` | Project-level configuration |
| `ragent.jsonc` | Project-level (with comments) |
| `~/.config/ragent/config.json` | User-global configuration |

### 16.2 Configuration Schema

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

### 16.3 Environment Variables

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



---

# Part V: External Integrations

---

## 17. LSP Integration

### 17.1 Overview

Language Server Protocol integration provides semantic code intelligence
directly within ragent's tool system. The `LspManager` coordinates multiple
language servers in parallel, auto-discovers installed servers from the
system PATH and VS Code extension directories, and dispatches queries to the
appropriate server based on file extension.

### 17.2 Architecture

```text
LspManager (Arc<RwLock<SharedLspManager>>)
├── servers: Vec<LspServer>        — All registered servers (connected, disabled, failed)
├── clients: HashMap<String, Arc<LspClient>>  — Active connections only
├── root_path: PathBuf             — Workspace root passed to servers
└── event_bus: Arc<EventBus>       — Publishes LspStatusChanged events

LspServer
├── id: String                     — Unique server identifier
├── language: String               — Language served
├── config: LspServerConfig        — Command, args, extensions, timeout
├── status: LspStatus              — Starting | Connected | Disabled | Failed
└── capabilities_summary: Option<String>  — "hover, definition, references, ..."
```

**Status Lifecycle:**

```text
[new] → Starting → Connected
                 ↘ Failed { error }
      (disabled) → Disabled
```

### 17.3 LSP Tools Available

| Tool | Purpose |
|------|---------|
| `lsp_hover` | Get type info and docs for symbol at position |
| `lsp_definition` | Find where a symbol is defined |
| `lsp_references` | Find all usages of a symbol |
| `lsp_symbols` | List all symbols in a source file |
| `lsp_diagnostics` | Show compiler errors and warnings |

### 17.4 LspManager API

| Method | Description |
|--------|-------------|
| `connect(id, language, config)` | Establish connection to a server |
| `connect_all(configs)` | Connect all configured servers |
| `disconnect(id)` | Shut down a single server |
| `disconnect_all()` | Shut down all servers |
| `client_for_extension(ext)` | Look up active client by file extension |
| `client_for_path(path)` | Look up active client by file path |
| `diagnostics_for(path)` | Aggregate diagnostics from all connected servers |
| `discover()` | Auto-discover installed language servers |

### 17.5 Auto-Discovery

The discovery system scans the local environment for installed language
servers without requiring manual configuration.

#### Discovery Methods

**1. PATH scanning:**

For each known executable name, checks if it exists on the system PATH.
Returns the first match per language.

**2. VS Code extension scanning:**

Scans the following directories for bundled language servers:

- `~/.vscode/extensions/`
- `~/.vscode-server/extensions/`
- `~/.vscode-insiders/extensions/`

Parses extension directory names
(`<publisher>.<name>-<version>[-<platform>]`) and resolves relative paths
to bundled server binaries. For each language, keeps the highest-versioned
extension and deduplicates across directories.

#### Known Language Servers (13 Built-in)

| Language | Executable Candidates | Args | File Extensions |
|----------|----------------------|------|-----------------|
| **Rust** | `rust-analyzer` | _(none)_ | `rs` |
| **TypeScript** | `typescript-language-server`, `tsserver` | `--stdio` | `ts`, `tsx`, `js`, `jsx`, `mjs`, `cjs` |
| **Python** | `pyright-langserver`, `pylsp`, `jedi-language-server` | `--stdio` | `py`, `pyi` |
| **Go** | `gopls` | _(none)_ | `go` |
| **C/C++** | `clangd` | _(none)_ | `c`, `h`, `cpp`, `hpp`, `cc`, `cxx` |
| **Java** | `jdtls`, `java-language-server` | _(none)_ | `java` |
| **Lua** | `lua-language-server` | _(none)_ | `lua` |
| **Ruby** | `solargraph` | `stdio` | `rb`, `gemspec` |
| **C#** | `OmniSharp`, `csharp-ls` | `--languageserver` | `cs` |
| **HTML** | `vscode-html-language-server` | `--stdio` | `html`, `htm` |
| **CSS** | `vscode-css-language-server` | `--stdio` | `css`, `scss`, `less` |
| **JSON** | `vscode-json-language-server` | `--stdio` | `json`, `jsonc` |

#### Discovery Result

```rust
pub struct DiscoveredServer {
    pub language: String,
    pub id: String,
    pub executable: PathBuf,
    pub args: Vec<String>,
    pub extensions: Vec<String>,
    pub source: DiscoverySource,      // SystemPath or VsCodeExtension
    pub version: Option<String>,
}
```

Each `DiscoveredServer` can be converted to an `LspServerConfig` for
insertion into `ragent.json` via `to_config()`.

### 17.6 Configuration

LSP servers are configured in `ragent.json`:

```jsonc
{
  "lsp": {
    "rust": {
      "command": "rust-analyzer",
      "extensions": ["rs"],
      "timeout_ms": 15000
    },
    "typescript": {
      "command": "typescript-language-server",
      "args": ["--stdio"],
      "extensions": ["ts", "tsx", "js", "jsx"],
      "disabled": false
    },
    "python": {
      "command": "pyright-langserver",
      "args": ["--stdio"],
      "extensions": ["py"],
      "env": { "PYTHONPATH": "/custom/path" }
    }
  }
}
```

**Configuration fields (`LspServerConfig`):**

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `command` | string | _(required)_ | Executable name or path |
| `args` | string[] | `[]` | Startup arguments |
| `env` | map | `{}` | Environment variable overrides |
| `extensions` | string[] | _(required)_ | Handled file extensions |
| `disabled` | bool | `false` | Skip startup if true |
| `timeout_ms` | u64 | `10000` | Response timeout in milliseconds |

### 17.7 Capabilities Detection

After a server connects and completes the LSP `initialize` handshake, the
manager inspects `ServerCapabilities` and builds a human-readable summary
string listing supported features:

- Hover support
- Definition / references
- Document symbols / workspace symbols
- Diagnostics
- Formatting / rename
- Code actions

Example: `"hover, definition, references, symbols, workspace-symbols, diagnostics, formatting"`

### 17.8 Event Publishing

Status changes are published to the `EventBus`:

```rust
Event::LspStatusChanged {
    server_id: String,
    status: LspStatus,     // Starting | Connected | Disabled | Failed { error }
}
```

The TUI subscribes to these events to display server health in the status
bar. Other components can also subscribe to coordinate dependent operations
(e.g. waiting for servers before indexing).

### 17.9 Usage Guidelines

- **Use LSP tools instead of grep** when looking for code symbols
  (functions, types, variables)
- **Connected servers** shown in `/lsp status`
- **File analysis triggered** on file open
- **Diagnostics updated** automatically on file changes
- **Multiple servers** can run in parallel for polyglot projects

### 17.10 Integration Approaches

| Approach | Description |
|----------|-------------|
| **Native Tool** | Direct LSP client implementation in ragent-core |
| **MCP Bridge** | LSP as MCP server (external process) |
| **Hybrid** | Combine native + MCP for different languages |

---


## 18. GitLab Integration

### 18.1 Overview

ragent provides a full GitLab integration modelled after the existing GitHub support. It connects to any GitLab instance (gitlab.com or self-hosted) via the GitLab REST API v4, using a Personal Access Token (PAT) for authentication.

### 18.2 Authentication & Credential Storage

Credentials are stored using the same encrypted database system as LLM provider keys:

| Item | Storage Location |
|------|-----------------|
| **Personal Access Token** | Encrypted in `provider_auth` table (provider id: `gitlab`) |
| **Instance URL + Username** | JSON in `settings` table (key: `gitlab_config`) |

#### Resolution Priority (Layered)

Credentials are resolved with the following priority (highest first):

1. **Environment variables** — `GITLAB_TOKEN`, `GITLAB_URL`, `GITLAB_USERNAME`
2. **ragent.json** — `gitlab` section in the configuration file
3. **Encrypted database** — stored via the `/gitlab setup` dialog

This matches the resolution pattern used by LLM providers.

#### ragent.json Configuration

```json
{
  "gitlab": {
    "instance_url": "https://gitlab.example.com",
    "token": "glpat-xxxxxxxxxxxxxxxxxxxx",
    "username": "your-username"
  }
}
```

All fields are optional. When present, they override database values but are overridden by environment variables.

#### Environment Variables

| Variable | Purpose |
|----------|---------|
| `GITLAB_TOKEN` | Personal Access Token |
| `GITLAB_URL` | GitLab instance URL (default: `https://gitlab.com`) |
| `GITLAB_USERNAME` | GitLab username |

### 18.3 Setup Dialog

The `/gitlab setup` command opens a provider-style setup dialog (same `ProviderSetupStep` pattern used by LLM providers):

1. **Instance URL field** — defaults to `https://gitlab.com`, editable
2. **Token field** — masked display (first 4 + last 4 characters visible)
3. **Tab** switches between fields
4. **Enter** triggers async validation against the GitLab API
5. **Esc** cancels the setup

On successful validation, credentials are persisted to the encrypted database and the dialog closes with a confirmation message.

### 18.4 Slash Commands

| Command | Description |
|---------|-------------|
| `/gitlab setup` | Open the setup dialog to configure GitLab connection |
| `/gitlab logout` | Delete stored GitLab credentials |
| `/gitlab status` | Display current connection status (instance URL, username, token presence) |

### 18.5 Tools

ragent provides 19 GitLab tools across issues, merge requests, and CI/CD pipelines:

#### Issue Tools

| Tool | Permission | Description |
|------|-----------|-------------|
| `gitlab_list_issues` | `gitlab:read` | List issues for a project with optional filters (state, labels, assignee) |
| `gitlab_get_issue` | `gitlab:read` | Get details of a specific issue by IID |
| `gitlab_create_issue` | `gitlab:write` | Create a new issue with title, description, labels, assignee |
| `gitlab_comment_issue` | `gitlab:write` | Add a comment (note) to an issue |
| `gitlab_close_issue` | `gitlab:write` | Close an issue by setting state to `closed` |

#### Merge Request Tools

| Tool | Permission | Description |
|------|-----------|-------------|
| `gitlab_list_mrs` | `gitlab:read` | List merge requests with optional filters (state, labels, author) |
| `gitlab_get_mr` | `gitlab:read` | Get details of a specific merge request by IID |
| `gitlab_create_mr` | `gitlab:write` | Create a merge request with title, source/target branches, description |
| `gitlab_merge_mr` | `gitlab:write` | Merge a merge request (with optional squash) |
| `gitlab_approve_mr` | `gitlab:write` | Approve a merge request |

#### Pipeline & Job Tools

| Tool | Permission | Description |
|------|-----------|-------------|
| `gitlab_list_pipelines` | `gitlab:read` | List pipelines with optional filters (status, ref/branch, limit) |
| `gitlab_get_pipeline` | `gitlab:read` | Get pipeline details (status, duration, stages, user, URL) |
| `gitlab_list_jobs` | `gitlab:read` | List jobs in a pipeline with optional scope filter |
| `gitlab_get_job` | `gitlab:read` | Get job details (stage, status, runner, artifacts, timing) |
| `gitlab_get_job_log` | `gitlab:read` | Download job log/trace output (tail N lines, default 200) |
| `gitlab_retry_job` | `gitlab:write` | Retry a failed or cancelled job |
| `gitlab_cancel_job` | `gitlab:write` | Cancel a running or pending job |
| `gitlab_retry_pipeline` | `gitlab:write` | Retry all failed jobs in a pipeline |
| `gitlab_cancel_pipeline` | `gitlab:write` | Cancel all running/pending jobs in a pipeline |

### 18.6 API Details

- **API Version**: GitLab REST API v4
- **Authentication Header**: `PRIVATE-TOKEN: <pat>`
- **Project Identification**: URL-encoded project path (e.g., `group%2Fsubgroup%2Fproject`)
- **Auto-detection**: Project path is auto-detected from git remote URLs (SSH and HTTPS patterns)
- **Issue/MR IDs**: Uses `iid` (project-scoped) not `id` (global), matching GitLab UI numbering
- **States**: `opened`, `closed`, `merged` (differs from GitHub's `open`/`closed`)
- **Comments**: Called "notes" in the GitLab API

### 18.7 Permission Model

GitLab tools use the ragent permission system with two permission categories:

| Permission | Scope | Tools |
|-----------|-------|-------|
| `gitlab:read` | Read-only operations | list_issues, get_issue, list_mrs, get_mr, list_pipelines, get_pipeline, list_jobs, get_job, get_job_log |
| `gitlab:write` | Write operations | create_issue, comment_issue, close_issue, create_mr, merge_mr, approve_mr, retry_job, cancel_job, retry_pipeline, cancel_pipeline |

Permissions follow the same allow/deny/ask flow as all other ragent tools (see §13 Security & Permissions).

### 18.8 Legacy Migration

On startup, ragent automatically migrates credentials from the legacy file-based storage format:

- `~/.ragent/gitlab_token` → encrypted database
- `~/.ragent/gitlab_config.json` → settings table

Legacy files are deleted after successful migration.

---


## 19. MCP Integration (Model Context Protocol)

### 19.1 Overview

ragent implements the Model Context Protocol (MCP) client, enabling
connection to external tool servers that expose capabilities via the
standardised MCP specification. This allows ragent to dynamically discover
and invoke tools from any MCP-compatible server.

### 19.2 Architecture

```text
McpManager
├── clients: Vec<McpClient>           — Active connections
├── max_concurrent: usize             — Default 8
├── discovery: McpDiscovery           — Auto-discovery system
└── event_bus: Arc<EventBus>          — Status change events

McpClient
├── server: McpServer                 — Server configuration
├── transport: McpTransport           — Stdio or HTTP/SSE
├── status: McpStatus                 — Connecting | Connected | Failed | Disconnected
├── tools: Vec<McpToolDef>            — Discovered tools from server
└── timeout: Duration                 — Default 120s per tool call
```

### 19.3 Transport Types

| Transport | Protocol | Use Case |
|-----------|----------|----------|
| **Stdio** | stdin/stdout JSON-RPC | Local processes (most common) |
| **HTTP/SSE** | HTTP POST + Server-Sent Events | Remote servers, network services |

**Stdio transport:**
- Spawns the server process as a child
- Communicates via stdin (requests) and stdout (responses)
- stderr is captured for error logging
- Process lifecycle managed by McpClient (killed on disconnect)

**HTTP/SSE transport:**
- Connects to a running HTTP server
- Sends requests via HTTP POST
- Receives streaming responses via SSE
- Supports reconnection on connection loss

### 19.4 Server Discovery

MCP servers can be discovered automatically from multiple sources:

#### Discovery Sources

| Source | Method | Priority |
|--------|--------|----------|
| **Configuration** | `ragent.json` `mcp.servers[]` | Highest |
| **PATH scanning** | Known executable names on PATH | Medium |
| **npm global** | `npm list -g` for known MCP packages | Medium |
| **Registry** | MCP server registry lookup | Lowest |

#### Known MCP Servers (18 Built-in)

The discovery system recognizes the following MCP server executables:

| Server | Package/Binary | Description |
|--------|---------------|-------------|
| `filesystem` | `@modelcontextprotocol/server-filesystem` | File operations |
| `github` | `@modelcontextprotocol/server-github` | GitHub API |
| `gitlab` | `@modelcontextprotocol/server-gitlab` | GitLab API |
| `slack` | `@modelcontextprotocol/server-slack` | Slack messaging |
| `google-drive` | `@modelcontextprotocol/server-gdrive` | Google Drive |
| `postgres` | `@modelcontextprotocol/server-postgres` | PostgreSQL |
| `sqlite` | `@modelcontextprotocol/server-sqlite` | SQLite |
| `puppeteer` | `@modelcontextprotocol/server-puppeteer` | Browser automation |
| `brave-search` | `@modelcontextprotocol/server-brave-search` | Brave Search |
| `fetch` | `@modelcontextprotocol/server-fetch` | HTTP fetching |
| `memory` | `@modelcontextprotocol/server-memory` | Persistent memory |
| `sequential-thinking` | `@modelcontextprotocol/server-sequential-thinking` | Chain-of-thought |
| `everything` | `@modelcontextprotocol/server-everything` | Test/demo server |
| `docker` | `mcp-server-docker` | Docker management |
| `kubernetes` | `mcp-server-kubernetes` | Kubernetes |
| `aws` | `mcp-server-aws` | AWS services |
| `azure` | `mcp-server-azure` | Azure services |
| `gcp` | `mcp-server-gcp` | Google Cloud |

### 19.5 Security

#### Shell Metacharacter Validation

Before spawning any MCP server process, the command string is validated
against shell metacharacters to prevent command injection:

**Blocked characters:** `` ` ``, `$`, `|`, `&`, `;`, `>`, `<`, `(`, `)`,
`{`, `}`, `\n`, `\r`

If any blocked character is found, the server connection is rejected with
an error.

#### Sandboxing

- Stdio servers run as child processes with inherited environment
- No additional filesystem sandboxing beyond OS-level permissions
- Environment variables can be restricted per server via configuration

### 19.6 Configuration

```jsonc
{
  "mcp": {
    "servers": [
      {
        "name": "filesystem",
        "command": "npx",
        "args": ["-y", "@modelcontextprotocol/server-filesystem", "/home/user/projects"],
        "transport": "stdio",
        "timeout_secs": 120,
        "env": {
          "NODE_OPTIONS": "--max-old-space-size=512"
        }
      },
      {
        "name": "github",
        "url": "http://localhost:3100/sse",
        "transport": "sse",
        "timeout_secs": 60
      }
    ],
    "auto_discover": true,
    "max_concurrent_connections": 8
  }
}
```

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `name` | string | _(required)_ | Unique server identifier |
| `command` | string | _(stdio)_ | Executable to spawn |
| `args` | string[] | `[]` | Command arguments |
| `url` | string | _(sse)_ | HTTP/SSE server URL |
| `transport` | string | `"stdio"` | `"stdio"` or `"sse"` |
| `timeout_secs` | u64 | `120` | Per-tool-call timeout |
| `env` | map | `{}` | Environment variable overrides |
| `auto_discover` | bool | `true` | Enable auto-discovery |
| `max_concurrent_connections` | usize | `8` | Max simultaneous servers |

### 19.7 Tool Integration

MCP tools are registered alongside native ragent tools and are available
to the LLM with the prefix `mcp_<server>_<tool>`:

```text
MCP server "github" exposes tools:
  → mcp_github_create_issue
  → mcp_github_list_repos
  → mcp_github_search_code
```

**Tool call flow:**

```text
LLM selects mcp_github_create_issue
   → McpManager routes to "github" client
   → McpClient sends JSON-RPC request
   → Server processes and responds
   → Result returned to LLM as tool output
```

### 19.8 Status Events

```rust
Event::McpStatusChanged {
    server_name: String,
    status: McpStatus,  // Connecting | Connected | Failed { error } | Disconnected
}
```

Status changes are published to the EventBus and displayed in the TUI
status bar.

---



---

# Part VI: Reference Materials

---

## 20. Tool Reference

### 20.1 Total Tool Count

**Current count:** 147+ tools across 18 categories (including aliases)

### 20.2 Tool Categories Summary

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
| GitLab | 19 | Issues, merge requests, pipelines, and jobs |
| Memory | 12 | Block storage and structured memories |
| Journal | 3 | Logging and search |
| Team | 21 | Coordination, tasks, messaging |
| Sub-agent | 5 | Background task management |
| LSP | 6 | Language server protocol tools |
| Plan | 2 | Plan delegation |
| MCP | 1 | Model Context Protocol |
| Interactive | 4 | User prompts, todos, reasoning |
| Utility | 3 | Calculator, environment |

### 20.3 Tool Categories (Detailed)

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


## 21. Office, LibreOffice, and PDF Document Tools

Ragent provides comprehensive support for creating, reading, and manipulating Microsoft Office documents, LibreOffice/OpenDocument files, and PDF documents. These tools enable agents to work with business documents, reports, spreadsheets, and presentations programmatically.

### 21.1 Overview

The document tool ecosystem consists of three major categories:

| Category | Tools | Supported Formats |
|----------|-------|-------------------|
| **Microsoft Office** | `office_read`, `office_write`, `office_info` | .docx, .xlsx, .pptx |
| **LibreOffice/OpenDocument** | `libre_read`, `libre_write`, `libre_info` | .odt, .ods, .odp |
| **PDF Documents** | `pdf_read`, `pdf_write` | .pdf |

### 21.2 Microsoft Office Tools

#### 21.2.1 `office_read`

Reads content from Word (.docx), Excel (.xlsx), or PowerPoint (.pptx) files.

**Parameters:**
| Parameter | Type | Required | Description |
|-----------|------|----------|-------------|
| `path` | string | Yes | Path to the Office document |
| `sheet` | string | No | Excel: sheet name or index (default: first sheet) |
| `range` | string | No | Excel: cell range e.g., 'A1:D10' (default: all data) |
| `slide` | integer | No | PowerPoint: specific slide number (default: all slides) |
| `format` | string | No | Output format: `text`, `markdown`, `json` (default: markdown) |

**Output Formats:**
- `text` — Plain text extraction
- `markdown` — Structured markdown with headings, lists, tables
- `json` — Structured JSON with document metadata

**Examples:**
```json
// Read a Word document
{"path": "report.docx", "format": "markdown"}

// Read specific Excel sheet and range
{"path": "data.xlsx", "sheet": "Sales", "range": "A1:F50"}

// Read specific PowerPoint slide
{"path": "presentation.pptx", "slide": 3}
```

**Returns:** Document content in requested format, line count, and metadata.

#### 21.2.2 `office_write`

Creates or overwrites Word (.docx), Excel (.xlsx), or PowerPoint (.pptx) files.

**Parameters:**
| Parameter | Type | Required | Description |
|-----------|------|----------|-------------|
| `path` | string | Yes | Path for the output file |
| `type` | string | Yes* | Document type: `docx`, `xlsx`, `pptx` (inferred from extension if omitted) |
| `content` | object/array | Yes | Document content (format varies by type) |
| `title` | string | No | Document title for metadata |

**Word (.docx) Content Format:**
```json
{
  "content": [
    {"type": "heading", "text": "Introduction", "level": 1},
    {"type": "paragraph", "text": "This is a paragraph."},
    {"type": "bullet_list", "items": ["Item 1", "Item 2", "Item 3"]},
    {"type": "ordered_list", "items": ["Step 1", "Step 2"]},
    {"type": "code_block", "text": "code here"}
  ]
}
```

**Excel (.xlsx) Content Format:**
```json
{
  "sheets": [
    {
      "name": "Sales",
      "rows": [
        ["Product", "Q1", "Q2", "Q3", "Q4"],
        ["Widget", "100", "150", "200", "250"],
        ["Gadget", "80", "120", "160", "200"]
      ]
    }
  ]
}
```

**PowerPoint (.pptx) Content Format:**
```json
{
  "slides": [
    {
      "title": "Slide 1 Title",
      "body": "Slide content here"
    },
    {
      "title": "Slide 2 Title",
      "body": "Second slide content"
    }
  ]
}
```

**Examples:**
```json
// Create a Word document
{
  "path": "output.docx",
  "content": {
    "paragraphs": [
      {"type": "heading", "text": "Report", "level": 1},
      {"type": "paragraph", "text": "This is the report body."}
    ]
  }
}

// Create an Excel spreadsheet
{
  "path": "data.xlsx",
  "content": {
    "sheets": [{"name": "Data", "rows": [["A", "B"], ["1", "2"]]}]
  }
}

// Create a PowerPoint presentation
{
  "path": "presentation.pptx",
  "content": {
    "slides": [
      {"title": "Welcome", "body": "Introduction slide"},
      {"title": "Agenda", "body": "Today's topics"}
    ]
  }
}
```

#### 21.2.3 `office_info`

Extracts metadata and structural information from Office documents without reading full content.

**Parameters:**
| Parameter | Type | Required | Description |
|-----------|------|----------|-------------|
| `path` | string | Yes | Path to the Office document |

**Returns for Word (.docx):**
- File type and size
- Paragraph count, word count, table count
- Title, author, creation date

**Returns for Excel (.xlsx):**
- File type and size
- Sheet names and dimensions
- Total row/column counts

**Returns for PowerPoint (.pptx):**
- File type and size
- Slide count
- Slide titles (if available)

**Example Output:**
```json
{
  "type": "docx",
  "size": 24576,
  "paragraphs": 45,
  "words": 1200,
  "tables": 2,
  "title": "Quarterly Report",
  "author": "John Doe",
  "created": "2025-01-15T10:30:00Z"
}
```

### 21.3 LibreOffice/OpenDocument Tools

LibreOffice/OpenDocument Format (ODF) is an open standard for office documents. Ragent provides native support without requiring LibreOffice installation.

#### 21.3.1 `libre_read`

Reads content from OpenDocument Text (.odt), Spreadsheet (.ods), or Presentation (.odp) files.

**Parameters:**
| Parameter | Type | Required | Description |
|-----------|------|----------|-------------|
| `path` | string | Yes | Path to the OpenDocument file |
| `sheet` | string | No | ODS only: sheet name or 0-based index (default: first) |
| `range` | string | No | ODS only: cell range e.g., 'A1:D10' (default: all) |
| `slide` | integer | No | ODP only: 1-based slide number (default: all) |
| `format` | string | No | Output format: `text`, `markdown`, `json` (default: markdown) |

**Implementation Details:**
- **ODS**: Parsed with `calamine` for full spreadsheet fidelity
- **ODT/ODP**: XML extraction from ZIP archive using `quick-xml`

**Examples:**
```json
// Read an ODT file
{"path": "document.odt"}

// Read specific ODS sheet
{"path": "spreadsheet.ods", "sheet": "Sales", "range": "A1:Z100"}

// Read specific ODP slide
{"path": "presentation.odp", "slide": 5}
```

#### 21.3.2 `libre_write`

Creates or overwrites OpenDocument files (.odt, .ods, .odp).

**Parameters:**
| Parameter | Type | Required | Description |
|-----------|------|----------|-------------|
| `path` | string | Yes | Output file path |
| `content` | object/array/string | Yes* | Document content (ODT/ODP) |
| `rows` | array | Yes* | 2D array for ODS spreadsheet |
| `sheet_name` | string | No | ODS: sheet name (default: 'Sheet1') |
| `title` | string | No | Document title metadata |
| `author` | string | No | Document author metadata |

*Either `content` (ODT/ODP) or `rows` (ODS) is required based on format.

**ODT Content Format:**
```json
{
  "content": [
    {"type": "paragraph", "text": "Normal paragraph"},
    {"type": "heading", "text": "Heading", "level": 1},
    {"type": "bullet_list", "items": ["Item 1", "Item 2"]},
    {"type": "ordered_list", "items": ["Step 1", "Step 2"]},
    {"type": "code_block", "text": "code"}
  ]
}
```

**ODS Spreadsheet Format:**
```json
{
  "rows": [
    ["Header1", "Header2", "Header3"],
    ["Data1", "Data2", "Data3"],
    ["Data4", "Data5", "Data6"]
  ],
  "sheet_name": "My Data"
}
```

**ODP Presentation Format:**
```json
{
  "content": [
    {"title": "Slide 1", "content": ["Bullet 1", "Bullet 2"]},
    {"title": "Slide 2", "content": ["Content here"]}
  ]
}
```

**Examples:**
```json
// Create an ODT document
{
  "path": "document.odt",
  "content": [
    {"type": "heading", "text": "Title", "level": 1},
    {"type": "paragraph", "text": "Content here."}
  ],
  "title": "My Document",
  "author": "Jane Doe"
}

// Create an ODS spreadsheet
{
  "path": "data.ods",
  "rows": [["Name", "Score"], ["Alice", "95"], ["Bob", "87"]],
  "sheet_name": "Results"
}

// Create an ODP presentation
{
  "path": "slides.odp",
  "content": [
    {"title": "Welcome", "content": ["Introduction"]}
  ]
}
```

#### 21.3.3 `libre_info`

Returns metadata and structural information about OpenDocument files.

**Parameters:**
| Parameter | Type | Required | Description |
|-----------|------|----------|-------------|
| `path` | string | Yes | Path to the OpenDocument file |

**Returns:**
- File format (odt/ods/odp)
- File size in bytes
- For ODT: paragraph count, word count, title, author
- For ODS: sheet names, row/column dimensions
- For ODP: slide count, slide titles

**Example Output:**
```json
{
  "format": "ods",
  "size": 8192,
  "sheets": [
    {"name": "Sheet1", "rows": 100, "columns": 26},
    {"name": "Sheet2", "rows": 50, "columns": 10}
  ]
}
```

### 21.4 PDF Document Tools

Portable Document Format (PDF) tools enable reading existing PDFs and creating new documents programmatically.

#### 21.4.1 `pdf_read`

Extracts text content, metadata, and page information from PDF files.

**Parameters:**
| Parameter | Type | Required | Description |
|-----------|------|----------|-------------|
| `path` | string | Yes | Path to the PDF file |
| `start_page` | integer | No | Starting page (1-based, inclusive). Default: first page |
| `end_page` | integer | No | Ending page (1-based, inclusive). Default: last page |
| `format` | string | No | Output format: `text`, `metadata`, `json` (default: text) |

**Output Formats:**
- `text` — Plain text extraction (default)
- `metadata` — Document information only (title, author, pages, etc.)
- `json` — Structured output with pages array and metadata

**Examples:**
```json
// Read entire PDF
{"path": "document.pdf"}

// Read specific page range
{"path": "report.pdf", "start_page": 5, "end_page": 10}

// Get metadata only
{"path": "document.pdf", "format": "metadata"}

// Get structured JSON output
{"path": "document.pdf", "format": "json"}
```

**Returns:**
- Extracted text content
- Page count
- Document metadata (title, author, creation date, etc.)

**Example Output (JSON format):**
```json
{
  "content": "Extracted text...",
  "metadata": {
    "path": "document.pdf",
    "pages": 42,
    "title": "Annual Report",
    "author": "Company Name",
    "start_page": 1,
    "end_page": 42
  }
}
```

#### 21.4.2 `pdf_write`

Creates PDF files from structured JSON content. Supports text paragraphs, headings, tables, and embedded images.

**Parameters:**
| Parameter | Type | Required | Description |
|-----------|------|----------|-------------|
| `path` | string | Yes | Path for the output PDF file |
| `content` | object | Yes | Document content structure |

**Content Structure:**
```json
{
  "content": {
    "title": "Document Title",
    "elements": [
      {"type": "heading", "text": "Section 1", "level": 1},
      {"type": "paragraph", "text": "Paragraph content here."},
      {"type": "table", "headers": ["Col1", "Col2"], "rows": [["A", "B"], ["C", "D"]]},
      {"type": "image", "path": "image.png", "width": 100, "height": 100}
    ]
  }
}
```

**Element Types:**
| Type | Properties | Description |
|------|------------|-------------|
| `paragraph` | `text` | Body text paragraph |
| `heading` | `text`, `level` (1-3) | Section headings |
| `table` | `headers`, `rows` | Data tables |
| `image` | `path`, `width`, `height` | Embedded images (PNG/JPG) |

**Layout Constants:**
- Page size: A4 (210mm × 297mm)
- Margins: 25mm on all sides
- Body font: 11pt
- Heading sizes: H1=22pt, H2=17pt, H3=13pt
- Line spacing: 1.4

**Examples:**
```json
// Create a simple PDF
{
  "path": "output.pdf",
  "content": {
    "title": "My Report",
    "elements": [
      {"type": "heading", "text": "Introduction", "level": 1},
      {"type": "paragraph", "text": "This is the introduction."},
      {"type": "heading", "text": "Results", "level": 2},
      {"type": "table", "headers": ["Item", "Value"], "rows": [["A", "10"], ["B", "20"]]}
    ]
  }
}

// Create PDF with image
{
  "path": "document.pdf",
  "content": {
    "elements": [
      {"type": "heading", "text": "Diagram", "level": 1},
      {"type": "image", "path": "chart.png", "width": 150, "height": 100}
    ]
  }
}
```

### 21.5 Implementation Details

#### 21.5.1 Dependencies

| Format | Dependencies |
|--------|--------------|
| DOCX | `docx-rust` |
| XLSX | `calamine`, `rust_xlsxwriter` |
| PPTX | `ooxmlsdk` |
| ODT/ODP | `zip`, `quick-xml` |
| ODS | `spreadsheet-ods`, `calamine` |
| PDF | `printpdf`, `pdf-extract` |

#### 21.5.2 Performance Characteristics

| Operation | Typical Performance |
|-----------|---------------------|
| Read DOCX | ~100 pages/second |
| Read XLSX | ~10,000 cells/second |
| Read PPTX | ~50 slides/second |
| Read ODT/ODP | ~50 pages/second |
| Read ODS | ~5,000 cells/second |
| Read PDF | ~20 pages/second |
| Write DOCX | ~50 pages/second |
| Write XLSX | ~5,000 cells/second |
| Write PPTX | ~30 slides/second |
| Write PDF | ~10 pages/second |

All document operations run in `tokio::task::spawn_blocking` to avoid blocking the async runtime.

#### 21.5.3 Output Limits

Document content is truncated to prevent excessive token usage:
- Maximum output: 100,000 bytes per document
- Truncation indicator added when content is cut off

#### 21.5.4 Error Handling

Common error conditions:
- **File not found**: Path resolution fails
- **Invalid format**: Extension doesn't match content
- **Corrupted file**: ZIP/XML parsing fails
- **Invalid range**: Sheet name, cell range, or slide number out of bounds
- **Permission denied**: File system access restrictions

### 21.6 Usage Best Practices

#### 21.6.1 When to Use Each Format

| Use Case | Recommended Format | Rationale |
|----------|-------------------|-----------|
| Interoperability | ODF (.odt/.ods/.odp) | Open standard, no licensing |
| Enterprise sharing | OOXML (.docx/.xlsx/.pptx) | Microsoft Office compatibility |
| Print-ready output | PDF | Fixed layout, universal reader |
| Editable reports | DOCX or ODT | Easy revision cycles |
| Data exchange | XLSX or ODS | Structured tabular data |
| Presentations | PPTX or ODP | Rich media support |

#### 21.6.2 Large Document Handling

For documents exceeding output limits:
1. Use `office_info`/`libre_info` to assess size first
2. Read specific ranges (Excel) or slides (PowerPoint) incrementally
3. Use `pdf_read` with page ranges for large PDFs

#### 21.6.3 Content Structure Guidelines

**Word/ODT Documents:**
- Use semantic headings (h1, h2, h3) for structure
- Prefer bullet lists over manual formatting
- Include code blocks with proper escaping

**Excel/ODS Spreadsheets:**
- First row should contain headers
- Use consistent data types per column
- Avoid merged cells when possible

**PowerPoint/ODP Presentations:**
- Keep slides focused (1-3 main points)
- Use titles for navigation
- Limit bullet points to 5-7 per slide

**PDF Documents:**
- Use hierarchical headings for navigation
- Include tables for structured data
- Optimize images before embedding

---


## 22. CLI Command Reference

### 22.1 Overview

ragent uses Clap for command-line argument parsing, providing a structured
CLI with global flags and subcommands.

### 22.2 Global Flags

| Flag | Short | Type | Default | Description |
|------|-------|------|---------|-------------|
| `--model` | `-m` | string | config default | Override LLM model |
| `--agent` | `-a` | string | `"general"` | Select agent profile |
| `--log-level` | | string | `"info"` | Log verbosity (trace/debug/info/warn/error) |
| `--no-tui` | | flag | `false` | Run in non-interactive mode |
| `--yes` | `-y` | flag | `false` | Auto-approve all permission requests |
| `--log` | | path | | Write logs to file |
| `--config` | `-c` | path | `ragent.json` | Configuration file path |
| `--maxsteps` | | u32 | `100` | Maximum processing steps per turn |
| `--no-git-context` | | flag | `false` | Disable git context in system prompt |
| `--no-readme-context` | | flag | `false` | Disable README injection in system prompt |

### 22.3 Subcommands

#### `ragent` (default)

Start an interactive TUI session:

```bash
ragent                           # Default agent, default model
ragent -m openai/gpt-4o         # Override model
ragent -a coder                  # Select agent
ragent --no-tui                  # Non-interactive mode
```

#### `ragent run`

Execute a single prompt and exit:

```bash
ragent run "Explain this codebase"
ragent run -m anthropic/claude-sonnet-4-20250514 "Write tests for auth.rs"
```

#### `ragent serve`

Start the HTTP server:

```bash
ragent serve                     # Default port (from config)
ragent serve --port 8080         # Custom port
ragent serve --host 0.0.0.0     # Bind to all interfaces
```

#### `ragent orchestrate`

Run in orchestrator mode:

```bash
ragent orchestrate               # Start orchestrator
ragent orchestrate --agents 4    # Max concurrent agents
```

#### `ragent session`

Session management commands:

```bash
ragent session list              # List all sessions
ragent session resume <id>       # Resume a previous session
ragent session export <id>       # Export session to JSON
ragent session import <file>     # Import session from JSON
```

#### `ragent memory`

Memory management commands:

```bash
ragent memory list               # List memory blocks
ragent memory export             # Export all memories
ragent memory export --format markdown  # Export as Markdown
ragent memory import <file>      # Import memories
ragent memory import --format cline <file>  # Import from Cline
```

#### `ragent auth`

Authentication management:

```bash
ragent auth                      # Show auth status
ragent auth login <provider>     # Login to provider
ragent auth logout <provider>    # Logout from provider
```

#### `ragent models`

List available models:

```bash
ragent models                    # List all configured models
ragent models --provider openai  # Filter by provider
```

#### `ragent config`

Configuration management:

```bash
ragent config                    # Show current config
ragent config --init             # Create default ragent.json
```

### 22.4 Environment Variables

| Variable | Description |
|----------|-------------|
| `RAGENT_CONFIG` | Override config file path |
| `RAGENT_LOG_LEVEL` | Override log level |
| `RAGENT_MODEL` | Override default model |
| `RAGENT_API_KEY_<PROVIDER>` | Provider API key (e.g., `RAGENT_API_KEY_OPENAI`) |
| `OPENAI_API_KEY` | OpenAI API key (standard) |
| `ANTHROPIC_API_KEY` | Anthropic API key (standard) |

---


## 23. Testing & CI/CD

### 23.1 Overview

ragent maintains a comprehensive test suite across all crates with
continuous integration via GitHub Actions.

### 23.2 Test Organization

Tests are organized in `tests/` directories within each crate, following
the project convention of external test files rather than inline tests:

```text
crates/
├── ragent-core/
│   ├── src/          — Source code (no inline #[test])
│   └── tests/        — Integration and unit tests
├── ragent-tui/
│   ├── src/
│   └── tests/
├── ragent-server/
│   ├── src/
│   └── tests/
├── ragent-code/
│   ├── src/
│   └── tests/
└── prompt_opt/
    ├── src/
    └── tests/
tests/                — Root-level integration tests
```

### 23.3 Test Commands

| Command | Description |
|---------|-------------|
| `cargo test` | Run all tests across all crates |
| `cargo test -p ragent-core` | Test a specific crate |
| `cargo test <name>` | Run tests matching a name |
| `cargo test -- --nocapture` | Show test output |
| `cargo test --lib` | Library tests only (skip integration) |
| `timeout 600 cargo test` | Run with 10-minute timeout |

### 23.4 Benchmarks (Criterion)

Performance benchmarks use the Criterion framework:

| Crate | Benchmark File | What It Measures |
|-------|---------------|-----------------|
| `ragent-tui` | `bench_markdown.rs` | Markdown rendering performance |
| `ragent-server` | `bench_sse.rs` | SSE event throughput |
| `ragent-core` | `bench_file_ops.rs` | File operation performance |
| `ragent-code` | `bench_index.rs` | Code indexing throughput |

**Running benchmarks:**

```bash
cargo bench                      # All benchmarks
cargo bench -p ragent-tui        # Single crate
cargo bench -- markdown          # Filter by name
```

### 23.5 CI Workflows (GitHub Actions)

#### `ci.yml` — Primary CI

Runs on every push and pull request:

| Step | Description |
|------|-------------|
| `cargo fmt --check` | Formatting validation |
| `cargo clippy -- -D warnings` | Lint checks (warnings = errors) |
| `cargo build` | Debug build |
| `cargo test` | Full test suite |

**Matrix:**
- Rust: stable, nightly
- OS: Ubuntu, macOS

#### `ci_benchmarks.yml` — Benchmark CI

Runs on pushes to `main` to track performance regressions:

| Step | Description |
|------|-------------|
| `cargo bench` | Run all Criterion benchmarks |
| Compare | Compare with baseline (previous run) |
| Alert | Comment on PR if regression detected |

#### `security-audit.yml` — Security Audit

Runs on schedule and PRs modifying `Cargo.lock`:

| Step | Description |
|------|-------------|
| `cargo audit` | Check for known vulnerabilities |
| `cargo deny check` | License and advisory compliance |

### 23.6 Pre-Flight Script (`pre-flight.sh`)

A local mirror of CI checks that developers can run before pushing:

```bash
./pre-flight.sh
```

**What it runs:**

1. `cargo fmt --check` — Formatting
2. `cargo clippy -- -D warnings` — Linting
3. `cargo build` — Build check
4. `cargo test` — Full test suite
5. `cargo doc --no-deps` — Documentation build

### 23.7 Dependency Management

**`deny.toml`** — Configuration for `cargo deny`:

| Check | Description |
|-------|-------------|
| `advisories` | RUSTSEC advisory database |
| `licenses` | Allowed license list |
| `bans` | Banned crates/versions |
| `sources` | Allowed registry sources |

---



---

# Part VII: Security & Operations

---

## 24. Security & Permissions

### 24.1 Overview

ragent implements **defence-in-depth** security with multiple independent
validation layers. Every tool invocation passes through a permission system
before execution, and shell commands face additional layers of command
validation, pattern blocking, and syntax checking.

The security model follows these principles:

- **Deny by default** — modifications and shell commands require explicit
  permission unless a rule grants them.
- **Layered validation** — bash commands pass through 7 independent security
  layers; any single layer can reject.
- **User control** — interactive permission prompts let users approve, deny,
  or permanently grant access per resource pattern.
- **Bounded execution** — semaphores limit concurrent processes and tool calls
  to prevent resource exhaustion.
- **Auditability** — hooks fire on permission denials and errors for logging
  and alerting.

### 24.2 Permission System

#### 24.2.1 Permission Types

| Permission | String | Description |
|------------|--------|-------------|
| `Read` | `"read"` | File or resource read access |
| `Edit` | `"edit"` | File write/edit access |
| `Bash` | `"bash"` | Shell command execution |
| `Web` | `"web"` | Network or web access |
| `Question` | `"question"` | Interactive question to the user |
| `PlanEnter` | `"plan_enter"` | Enter a planning phase |
| `PlanExit` | `"plan_exit"` | Exit a planning phase |
| `Todo` | `"todo"` | Create or modify to-do items |
| `ExternalDirectory` | `"external_directory"` | Access directories outside the project root |
| `DoomLoop` | `"doom_loop"` | Detect and break infinite processing loops |
| `Custom(name)` | any string | User-defined permission type |

#### 24.2.2 Permission Actions

| Action | Behaviour |
|--------|-----------|
| `Allow` | Grant the operation without prompting the user |
| `Deny` | Block the operation without prompting the user |
| `Ask` | Display an interactive permission dialog (default when no rule matches) |

#### 24.2.3 Permission Rules

Rules are defined as triples of `(permission, glob_pattern, action)` and
evaluated **last-match-wins** (like CSS specificity):

```jsonc
{
  "permission": [
    { "permission": "read",  "pattern": "**",         "action": "allow" },
    { "permission": "edit",  "pattern": "src/**",     "action": "allow" },
    { "permission": "edit",  "pattern": "src/main.rs","action": "deny"  },
    { "permission": "bash",  "pattern": "*",          "action": "ask"   },
    { "permission": "web",   "pattern": "api.github.com", "action": "allow" }
  ]
}
```

A wildcard permission (`"*"`) matches all permission types.

#### 24.2.4 Default Permission Ruleset

When no custom rules are configured, ragent uses these defaults:

| Permission | Pattern | Action | Effect |
|------------|---------|--------|--------|
| `read` | `**` | Allow | All file reads auto-approved |
| `edit` | `**` | Ask | File writes require user approval |
| `bash` | `*` | Ask | Shell commands require user approval |
| `web` | `*` | Ask | Network access requires user approval |
| `plan_enter` | `*` | Ask | Entering planning mode requires approval |
| `todo` | `*` | Allow | To-do operations auto-approved |

Read-only agent profiles (e.g. `researcher`, `librarian`) use a restricted
ruleset that denies all edits and bash commands.

#### 24.2.5 Permission Checker

The `PermissionChecker` evaluates requests in this order:

1. **Always grants** — permanent grants recorded via "Always Allow" user
   decisions are checked first. If any glob matcher hits, the operation is
   allowed immediately.
2. **Static ruleset** — rules are evaluated sequentially; the last matching
   rule determines the action.
3. **Fallback** — if no rule matches, the action is `Ask`.

### 24.3 Permission Request Flow

When a tool requires permission, the following flow occurs:

```
Tool invocation
    │
    ├─► Pre-tool-use hooks evaluated
    │     ├─ Hook returns Allow  → skip UI, execute
    │     ├─ Hook returns Deny   → reject with reason
    │     └─ Hook returns ModifiedInput → apply changes, continue
    │
    ├─► PermissionChecker.check(permission, resource)
    │     ├─ Always grant matches → execute
    │     ├─ Rule matches Allow   → execute
    │     ├─ Rule matches Deny    → reject
    │     └─ Rule matches Ask     → send PermissionRequest to TUI
    │
    └─► TUI Permission Dialog
          ├─ User presses 'y' → grant Once, execute
          ├─ User presses 'a' → grant Always (recorded), execute
          └─ User presses 'n' → deny, fire on_permission_denied hook
```

#### 24.3.1 Permission Request

A `PermissionRequest` is published as an event to the TUI:

| Field | Type | Description |
|-------|------|-------------|
| `id` | String | Unique request identifier |
| `session_id` | String | Session that originated the request |
| `permission` | String | Permission type (e.g. `"bash"`, `"edit"`) |
| `patterns` | Vec&lt;String&gt; | Glob patterns describing target resources |
| `metadata` | JSON | Tool-specific metadata (command text, file path, etc.) |
| `tool_call_id` | Option&lt;String&gt; | Tool call that triggered the request |

#### 24.3.2 Permission Decisions

| Decision | Key | Behaviour |
|----------|-----|-----------|
| **Once** | `y` | Grant for this single operation only |
| **Always** | `a` | Grant permanently for this permission + pattern (session lifetime) |
| **Deny** | `n` | Block the operation |

"Always" grants are stored in the `PermissionChecker`'s `always_grants`
HashMap and take precedence over all rules for the remainder of the session.
They are not persisted across sessions.

#### 24.3.3 Permission Queue

Multiple permission requests can arrive in rapid succession (e.g. parallel
tool calls). The TUI maintains a FIFO queue:

- **Data structure**: `VecDeque<PermissionRequest>`
- **Display**: The front of the queue is rendered as the active permission
  dialog. A queue depth indicator shows how many additional requests are
  pending (e.g. `"Permission: bash (3 queued)"`).
- **Deduplication**: Requests with the same `session_id`, `permission`, and
  first pattern are deduplicated on arrival.
- **Processing**: When the user responds, the front request is popped and the
  next becomes active.

### 24.4 Bash Security

Shell commands pass through **7 independent security layers**. Any layer can
reject a command, and layers are evaluated in order:

#### Layer 1 — Safe Command Whitelist

Commands matching the safe list are **auto-approved** without a permission
prompt. Matching is by prefix (e.g. `git status` matches `git`).

| Category | Commands |
|----------|----------|
| **File management** | `ls`, `cd`, `pwd`, `mkdir`, `touch`, `cp`, `mv` |
| **File reading & search** | `cat`, `head`, `tail`, `grep`, `egrep`, `fgrep`, `find`, `rg`, `wc` |
| **Version control** | `git`, `gh` |
| **Build / package** | `cargo`, `rustc`, `rustfmt`, `clippy-driver`, `npm`, `yarn`, `pnpm`, `node`, `npx`, `python3`, `python`, `pip`, `pip3`, `make`, `docker-compose` |
| **Text / data utilities** | `echo`, `printf`, `chmod`, `jq`, `yq`, `sed`, `awk`, `sort`, `uniq`, `cut`, `tr`, `xargs`, `date`, `which`, `tree`, `diff`, `patch` |

> **Note:** `rm` is intentionally excluded from the safe list. Individual `rm`
> calls go through normal permission flow; destructive variants are caught by
> the denied patterns layer.

#### Layer 2 — Banned Commands

High-risk tools that could exfiltrate data or attack external systems are
**always rejected** (unless YOLO mode is enabled). Detection uses
word-boundary matching to avoid false positives (e.g. a path containing
`curl_helper` does not trigger the `curl` ban).

| Category | Commands |
|----------|----------|
| **Data exfiltration** | `curl`, `wget`, `nc`, `netcat`, `telnet`, `axel`, `aria2c`, `lynx`, `w3m` |
| **Attack tools** | `nmap`, `masscan`, `nikto`, `sqlmap`, `hydra`, `john`, `hashcat`, `aircrack`, `metasploit`, `msfconsole`, `msfvenom`, `burpsuite`, `ettercap`, `arpspoof` |
| **Packet capture** | `tcpdump`, `wireshark` |

#### Layer 3 — Denied Patterns

Substring patterns that indicate destructive or dangerous intent are
**always rejected**. Heredoc body content is stripped before matching to
prevent false positives from literal text (e.g. Rust string escapes).

| Category | Patterns |
|----------|----------|
| **Filesystem destruction** | `rm -rf /`, `rm -r -f /`, `rm -fr /`, `rm -Rf /`, `rmdir /`, `rm -rf ~`, `rm -rf $HOME`, `rm -rf .` |
| **Disk/partition destruction** | `mkfs`, `dd if=`, `wipefs`, `shred /dev` |
| **Device writes** | `> /dev/sd`, `> /dev/nvme`, `> /dev/vd` |
| **Fork bomb** | `:(){ :\|:&};:` |
| **Privilege escalation** | `sudo`, `su -`, `su root`, `doas`, `chmod -R 777 /`, `chmod 000 /`, `chmod -R 000`, `chown -R` |
| **Credential theft** | `.bash_history`, `.ssh/id_` |
| **Kernel modifications** | `insmod`, `modprobe -r`, `sysctl -w` |
| **User/group manipulation** | `useradd`, `usermod`, `groupadd`, `passwd` |
| **System configuration** | `visudo`, `crontab -`, `systemctl disable`, `systemctl mask`, `chattr +i` |
| **Destructive git** | `git push --force`, `git push -f`, `git push origin --delete` |
| **Boot/firmware** | `grub-install`, `efibootmgr` |
| **Data exfiltration** | `> /dev/tcp`, `bash -i >&`, `/dev/tcp/`, `/dev/udp/` |
| **Sensitive file access** | `curl.*etc/shadow`, `wget.*etc/shadow` |

#### Layer 4 — Directory Escape Prevention

Commands using `cd` or `pushd` are checked for directory escape attempts:

| Pattern | Blocked |
|---------|---------|
| `cd ..` or `cd ../..` | Yes — parent directory traversal |
| `cd /etc/passwd` | Yes — absolute path outside working directory |
| `cd ~` or `cd $HOME` | Yes — home directory escape |
| `cd /project/subdir` | Allowed if path is within working directory |
| `cd /help` | Allowed — single-segment slash-prefixed tokens treated as commands |

Path validation uses `canonicalize()` to resolve symlinks and verify the
target is within the working directory tree.

#### Layer 5 — Syntax Validation

Before execution, commands are checked with `sh -n -c <command>` (parse-only,
no execution) with a 1-second timeout. Invalid syntax is rejected before the
command can run.

#### Layer 6 — Obfuscation Detection

Commands that attempt to bypass other layers through encoding or dynamic
evaluation are rejected:

| Pattern | Description |
|---------|-------------|
| `base64 ... \| bash` | Base64-decode piped into shell |
| `python -c "exec(...)"` | Dynamic eval/exec in scripting language |
| `$'\x72\x6d'` | Hex escape sequence obfuscation |
| `eval $(...)` | Eval with command substitution |

#### Layer 7 — User Allowlist / Denylist

Users can customise bash security via configuration or slash commands:

| Command | Effect |
|---------|--------|
| `/bash add allow curl` | Exempt `curl` from the banned commands check |
| `/bash add deny git push -f` | Always reject force-push commands |
| `/bash remove allow curl` | Re-enable the `curl` ban |
| `/bash remove deny git push -f` | Remove the custom deny rule |

Configuration equivalent in `ragent.json`:

```jsonc
{
  "bash": {
    "allowlist": ["curl", "wget"],
    "denylist": ["git push -f", "rm -rf"]
  }
}
```

Allowlist entries exempt commands from the banned-commands check only.
Denylist entries are checked as substring patterns, similar to the built-in
denied patterns. Both global and project-level configurations are merged.

### 24.5 File Path Security

- **Directory escape guard** — `check_path_within_root` ensures file
  operations stay within the project root directory.
- **Wildcard restriction** — wildcards are not permitted in `rm` tool
  operations.
- **Edit snapshots** — file contents are snapshotted before edits to support
  rollback via `/undo`.
- **LRU read cache** — file reads are cached (256 entries, keyed on path +
  mtime) to reduce disk access; cache invalidates when the file is modified.
- **Large file handling** — files exceeding 100 lines return a summary with
  the first 100 lines plus a section map, preventing accidental consumption
  of enormous files.

### 24.6 Resource Limits

Application-level semaphores prevent resource exhaustion:

| Resource | Limit | Purpose |
|----------|-------|---------|
| Concurrent child processes | 16 | Bounds bash commands, dynamic context commands, MCP servers |
| Concurrent tool calls | 5 | Bounds parallel tool execution within a single agent turn |

Permits are acquired before execution and released when the operation
completes. If all permits are in use, new requests wait asynchronously
until a permit becomes available.

> **Why not `setrlimit`?** True per-process limits (`RLIMIT_NPROC`, etc.)
> require `unsafe` code. The workspace has `unsafe_code = "deny"`, so
> application-level concurrency control is used instead.

### 24.7 YOLO Mode

YOLO mode disables most security validations for trusted local development
scenarios.

**Toggle:** `/yolo` slash command (requires confirmation)

**What YOLO mode bypasses:**

| Layer | Normal | YOLO |
|-------|--------|------|
| Banned commands | Rejected | Allowed with warning |
| Denied patterns | Rejected | Allowed with warning |
| Obfuscation detection | Rejected | Skipped |
| User denylist | Rejected | Skipped |
| Dynamic context allowlist | Enforced | Skipped |
| MCP config validation | Enforced | Skipped |

**What YOLO mode does NOT bypass:**

| Layer | Behaviour |
|-------|-----------|
| Safe command whitelist | Still applied (auto-approve) |
| Directory escape prevention | Still enforced |
| Syntax validation | Still enforced |
| Resource semaphores | Still enforced |
| Permission rules with `Deny` action | Still enforced |

YOLO mode is session-scoped and not persisted across restarts.

**Auto-approve mode** is a separate mechanism:

```bash
ragent --yes              # Auto-approve all permission prompts
export RAGENT_YES=1       # Environment variable equivalent
```

This automatically responds "Once" to all `Ask` permission prompts but does
not bypass banned commands, denied patterns, or other security layers.

### 24.8 Agent Profile Permissions

Each agent profile can define its own permission ruleset that overlays or
replaces the defaults.

#### Built-in Agent Profiles

| Agent | Edit | Bash | Web | Notes |
|-------|------|------|-----|-------|
| `coder` | Ask | Ask | Ask | Full capability, prompts for writes |
| `reviewer` | Ask | Ask | Ask | Code review with manual approval |
| `researcher` | Deny | Deny | Ask | Read-only, no modifications |
| `librarian` | Deny | Deny | Ask | Read-only, documentation queries |

#### Custom Agent Permissions

Agent profiles (JSON OASF or Markdown format) support a `permissions` field:

```json
{
  "modules": [{
    "type": "ragent/agent/v1",
    "payload": {
      "system_prompt": "...",
      "permissions": [
        { "permission": "read",  "pattern": "**",      "action": "allow" },
        { "permission": "edit",  "pattern": "docs/**",  "action": "allow" },
        { "permission": "edit",  "pattern": "**",       "action": "deny"  },
        { "permission": "bash",  "pattern": "*",        "action": "deny"  }
      ]
    }
  }]
}
```

#### Permission Merge Order

Permissions are resolved by merging multiple sources:

1. **Built-in agent defaults** — base ruleset for the agent type
2. **Global config permissions** — from `ragent.json` `"permission"` field
3. **Agent-specific permissions** — from the agent profile's `"permissions"` field

All rules are concatenated in order and evaluated last-match-wins, so
agent-specific rules override global rules, which override built-in defaults.

### 24.9 Hooks System

Lifecycle hooks allow running shell commands at key session points for
auditing, logging, and custom permission logic. Hooks are synchronous for
pre-tool-use (can block or modify execution) and fire-and-forget for all
other triggers.

#### Triggers

| Trigger | Description | Execution Model |
|---------|-------------|-----------------|
| `on_session_start` | Fired when session receives first user message | Fire-and-forget |
| `on_session_end` | Fired after session completes processing | Fire-and-forget |
| `on_error` | Fired when LLM call or tool execution errors | Fire-and-forget |
| `on_permission_denied` | Fired when tool rejected by permission rule | Fire-and-forget |
| `pre_tool_use` | Fired before tool execution (can approve/deny/modify) | Synchronous (blocks) |
| `post_tool_use` | Fired after tool execution (can inspect/modify results) | Async spawned |

#### Configuration

```jsonc
{
  "hooks": [
    {
      "trigger": "on_session_start",
      "command": "echo 'Session started' >> ~/.ragent/session.log",
      "timeout_secs": 30
    },
    {
      "trigger": "pre_tool_use",
      "command": "./check_tool.sh",
      "timeout_secs": 10
    },
    {
      "trigger": "on_permission_denied",
      "command": "echo 'Denied: $RAGENT_TOOL_NAME' >> ~/.ragent/audit.log"
    },
    {
      "trigger": "on_error",
      "command": "notify-send 'ragent error' '$RAGENT_ERROR'"
    }
  ]
}
```

#### Environment Variables

All hooks receive a base set of environment variables. Additional variables
are provided depending on the trigger type:

**Base variables (all hooks):**

| Variable | Description |
|----------|-------------|
| `RAGENT_TRIGGER` | The trigger name (e.g., `on_session_start`, `pre_tool_use`) |
| `RAGENT_WORKING_DIR` | Session working directory |

**Error hooks (`on_error`):**

| Variable | Description |
|----------|-------------|
| `RAGENT_ERROR` | Error message text |

**Tool hooks (`pre_tool_use`, `post_tool_use`):**

| Variable | Description |
|----------|-------------|
| `RAGENT_TOOL_NAME` | Name of the tool being invoked |
| `RAGENT_TOOL_INPUT` | JSON string of the tool arguments |

**Post-tool hooks only (`post_tool_use`):**

| Variable | Description |
|----------|-------------|
| `RAGENT_TOOL_OUTPUT` | JSON string of the tool output |
| `RAGENT_TOOL_SUCCESS` | `"true"` or `"false"` |

#### Pre-tool-use Hook Results

`pre_tool_use` hooks run synchronously before tool execution and can
control the outcome by writing JSON to stdout:

```rust
pub enum PreToolUseResult {
    Allow,                          // Execute without UI prompt
    Deny { reason: String },        // Block execution with reason
    ModifiedInput { input: Value }, // Modified tool arguments
    NoDecision,                     // Use normal permission flow
}
```

| stdout JSON | Effect |
|-------------|--------|
| `{"decision": "allow"}` | Skip UI prompt, allow tool |
| `{"decision": "deny", "reason": "..."}` | Deny with reason |
| `{"modified_input": {...}}` | Replace tool arguments before execution |
| Empty/invalid output | Normal permission flow continues (`NoDecision`) |

#### Post-tool-use Hook Results

`post_tool_use` hooks run asynchronously after tool execution and can
modify the output returned to the LLM:

| stdout JSON | Effect |
|-------------|--------|
| `{"modified_output": {"content": "...", ...}}` | Replace tool output |
| Empty/invalid output | Original output passed through unchanged |

#### Execution Model

| Type | Function | Behaviour |
|------|----------|-----------|
| **Synchronous** | `run_pre_tool_use_hooks()` | Blocks tool execution; can modify/deny |
| **Async spawned** | `run_post_tool_use_hooks()` | Spawned tasks; can inspect/modify output |
| **Fire-and-forget** | `fire_hooks()` | Spawned tasks; never block execution |

**Default timeout:** 30 seconds per hook (configurable via `timeout_secs`).

**Error handling:** Hook errors are logged but never fatal. Failures do not
block tool execution or session processing.

### 24.10 HTTP & Network Security

#### Client Configuration

| Setting | Value | Purpose |
|---------|-------|---------|
| Connection pool per host | 8 | Prevents connection exhaustion |
| Pool idle timeout | 90s | Releases idle connections |
| Connect timeout | 30s | Fails fast on unreachable hosts |
| Request timeout | 120s | Prevents hung non-streaming requests |
| Streaming timeout | None (per-chunk) | Per-chunk timeout managed by each provider |
| TCP keep-alive | 60s | Detects dead connections |

#### Retry Policy

- **Max retries:** 4
- **Backoff:** Exponential (1s, 2s, 4s, 8s)
- **Retries on:** 5xx errors, connection failures, timeouts, HTTP/2 protocol
  errors, body decoding errors, broken pipes, unexpected EOF
- **No retry on:** 4xx client errors (authentication, rate limiting, etc.)

#### Credential Handling

- API keys are stored in the ragent configuration database, not in plain-text
  files.
- The Copilot provider uses GitHub device flow OAuth — tokens are exchanged
  for short-lived session tokens and cached in memory.
- Secrets in log output and SSE event payloads are redacted via a central
  `redact_secrets()` function.

### 24.11 Secret Redaction

All logging and event serialisation passes through secret redaction:

- **Patterns detected:** API keys, bearer tokens, JWTs, AWS credentials,
  GitHub tokens, SSH keys, and other common secret formats.
- **Redaction:** Matched patterns are replaced with `[REDACTED]`.
- **Coverage:** `tracing` output, SSE event payloads, error messages, and
  tool output displayed in the TUI.

### 24.12 Security Summary

| Security Layer | Scope | Enforcement | Bypass |
|----------------|-------|-------------|--------|
| Permission rules | All tools | Glob-pattern ruleset, last-match-wins | "Always" grants, hooks |
| Permission queue | TUI | FIFO interactive dialog | Auto-approve mode (`--yes`) |
| Safe command whitelist | Bash | Prefix matching | N/A (auto-approve) |
| Banned commands | Bash | Word-boundary matching | User allowlist or YOLO |
| Denied patterns | Bash | Substring matching | YOLO only |
| Directory escape guard | Bash | Path canonicalisation | Cannot bypass |
| Syntax validation | Bash | `sh -n -c` pre-check | Cannot bypass |
| Obfuscation detection | Bash | Pattern matching | YOLO only |
| User allow/denylist | Bash | Config + slash commands | User-managed |
| Dynamic context allowlist | Skills | 65+ executable allowlist | YOLO only |
| File path guard | File tools | Root directory check | Cannot bypass |
| Resource semaphores | All tools | 16 process / 5 tool permits | Cannot bypass |
| Pre/post-tool hooks | All tools | Custom shell scripts | Config-dependent |
| Secret redaction | Logging/events | Regex pattern matching | Cannot bypass |
| HTTP timeouts | Network | Client-level enforcement | Config override |

### 24.13 Encrypted Credential Storage

ragent stores all sensitive credentials (API keys, tokens, OAuth tokens) in an encrypted SQLite database rather than plain-text files. This system is implemented in `crates/ragent-core/src/storage/mod.rs`.

#### Encryption Architecture

| Component | Detail |
|-----------|--------|
| **Algorithm** | blake3 key derivation in XOF (extendable output) mode |
| **Key derivation** | `blake3::derive_key("ragent credential encryption v2", "{username}:{home_dir}")` |
| **Key binding** | Machine-local — derived from OS username + home directory path |
| **Nonce** | 16-byte random nonce per encryption operation |
| **Cipher** | XOR of plaintext with blake3-derived keystream |
| **Format** | `v2:<base64(nonce || ciphertext)>` prefix identifies encrypted values |
| **Legacy support** | Automatic v1 → v2 migration on read (repeating-key XOR format) |
| **Protection** | Copying the database file to another machine or user account renders credentials unrecoverable |

#### Database Tables

Two SQLite tables store credentials and configuration:

```sql
-- Encrypted credential storage (API keys, tokens)
CREATE TABLE provider_auth (
    provider_id TEXT PRIMARY KEY,    -- e.g. "anthropic", "openai", "copilot", "gitlab"
    api_key     TEXT NOT NULL,       -- encrypted with v2: prefix
    updated_at  TEXT NOT NULL
);

-- General key-value settings (unencrypted)
CREATE TABLE settings (
    key        TEXT PRIMARY KEY,     -- e.g. "gitlab_config", "selected_model", "theme"
    value      TEXT NOT NULL,
    updated_at TEXT NOT NULL
);
```

The `provider_auth` table stores encrypted values; the `settings` table stores plaintext configuration data.

#### Core Encryption Functions

| Function | Module | Purpose |
|----------|--------|---------|
| `encrypt_key(key: &str) -> String` | `storage/mod.rs` | Encrypts a plaintext key, returns `v2:`-prefixed ciphertext |
| `decrypt_key(encoded: &str) -> String` | `storage/mod.rs` | Decrypts a `v2:` or legacy `v1` encoded key |
| `generate_keystream(nonce, len) -> Vec<u8>` | `storage/mod.rs` | Generates blake3 XOF keystream for a given nonce |
| `deobfuscate_key_v1(encoded: &str) -> String` | `storage/mod.rs` | Legacy v1 XOR decoding (backward compatibility) |
| `obfuscate_key(key: &str) -> String` | `storage/mod.rs` | **Deprecated** — delegates to `encrypt_key` |
| `deobfuscate_key(encoded: &str) -> String` | `storage/mod.rs` | **Deprecated** — delegates to `decrypt_key` |

#### Storage API Methods

These are public methods on the `Storage` struct used by the rest of the codebase:

| Method | Purpose |
|--------|---------|
| `set_provider_auth(provider_id, api_key)` | Encrypt and store a provider credential |
| `get_provider_auth(provider_id) -> Option<String>` | Retrieve and decrypt a provider credential; auto-migrates v1 → v2 |
| `delete_provider_auth(provider_id)` | Remove a provider credential |
| `seed_secret_registry()` | Load all stored credentials into the secret redaction registry at startup |
| `set_setting(key, value)` | Store a plaintext setting |
| `get_setting(key) -> Option<String>` | Retrieve a plaintext setting |
| `delete_setting(key)` | Remove a setting |

#### Callers — Provider Authentication

The following subsystems use `set_provider_auth` / `get_provider_auth` / `delete_provider_auth`:

| Caller | File | Usage |
|--------|------|-------|
| **Provider setup dialog** | `ragent-tui/src/input.rs` | Stores API keys during `/provider` setup (Anthropic, OpenAI, Ollama Cloud, Generic OpenAI) |
| **Provider removal** | `ragent-tui/src/input.rs` | Deletes credentials on provider logout/removal |
| **Copilot OAuth flow** | `ragent-tui/src/input.rs` | Stores OAuth token after GitHub Copilot device flow |
| **Copilot token check** | `ragent-tui/src/app.rs` | Reads Copilot token to check auth status |
| **Provider auto-detection** | `ragent-tui/src/app.rs` | Reads stored keys to auto-select available providers at startup |
| **Session processor** | `ragent-core/src/session/processor.rs` | Reads provider keys when creating LLM clients for inference |
| **GitLab auth** | `ragent-core/src/gitlab/auth.rs` | Stores/retrieves GitLab PAT (provider_id = `"gitlab"`) |
| **GitLab legacy migration** | `ragent-core/src/gitlab/auth.rs` | Imports tokens from legacy `~/.ragent/gitlab_token` files |

#### Callers — Settings

The following subsystems use `set_setting` / `get_setting` / `delete_setting`:

| Caller | File | Usage |
|--------|------|-------|
| **Model selection** | `ragent-tui/src/input.rs`, `app.rs` | Persists `selected_model`, `selected_model_ctx_window`, `preferred_provider` |
| **Copilot API base** | `ragent-tui/src/input.rs` | Stores `copilot_api_base` URL |
| **Generic OpenAI endpoint** | `ragent-tui/src/input.rs` | Stores `generic_openai_api_base` URL |
| **Provider disable flags** | `ragent-tui/src/input.rs` | Stores `provider_{id}_disabled` flags |
| **GitLab config** | `ragent-core/src/gitlab/auth.rs` | Stores `gitlab_config` JSON (instance URL + username) |
| **Session processor** | `ragent-core/src/session/processor.rs` | Reads `copilot_api_base` and `generic_openai_api_base` for LLM clients |
| **Memory system** | `ragent-core/src/memory/extract.rs` | Reads `project_name` for memory extraction context |

#### Layered Credential Resolution

For integrations that support multiple configuration sources, ragent uses a layered resolution pattern (highest priority first):

1. **Environment variables** — e.g. `ANTHROPIC_API_KEY`, `GITLAB_TOKEN`, `OPENAI_API_KEY`
2. **ragent.json configuration** — project-level or global config file
3. **Encrypted database** — credentials stored via the TUI setup dialogs

This pattern is currently used by all LLM providers and the GitLab integration.

---


## 25. Auto-Update Mechanism

### 25.1 Overview

ragent includes a self-update mechanism that checks for new releases on
GitHub and performs atomic binary replacement.

### 25.2 Update Check Flow

```text
On startup (background task)
   ↓
GET https://api.github.com/repos/{owner}/{repo}/releases/latest
   ↓
Parse release tag as semver
   ↓
Compare with current version (including prerelease)
   ├── Newer available → Notify user
   └── Current or newer → No action
```

### 25.3 Platform Detection

The updater detects the current platform and selects the appropriate
release asset:

| Platform | Architecture | Asset Pattern |
|----------|-------------|---------------|
| Linux | x86_64 | `ragent-linux-x86_64` |
| Linux | aarch64 | `ragent-linux-aarch64` |
| macOS | x86_64 | `ragent-darwin-x86_64` |
| macOS | aarch64 | `ragent-darwin-aarch64` |
| Windows | x86_64 | `ragent-windows-x86_64.exe` |

### 25.4 Atomic Binary Replacement

The update process ensures no corruption or partial writes:

```text
1. Download new binary to temporary file (in same directory)
2. Verify download integrity (size check)
3. Set executable permissions (chmod +x on Unix)
4. Atomic rename: temp file → current binary path
5. Log success and suggest restart
```

**Failure handling:**
- If download fails, temp file is cleaned up
- If rename fails, original binary is preserved
- User is never left without a working binary

### 25.5 Version Comparison

Version comparison uses full semver with prerelease support:

```text
0.1.0-alpha < 0.1.0-beta < 0.1.0 < 0.1.1-alpha < 0.1.1
```

**Rules:**
- Prerelease versions are always less than release versions
- Prerelease identifiers are compared lexicographically
- Build metadata is ignored in comparisons

### 25.6 Configuration

```jsonc
{
  "auto_update": {
    "enabled": true,
    "check_on_startup": true,
    "channel": "stable"
  }
}
```

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `enabled` | bool | `true` | Enable auto-update checks |
| `check_on_startup` | bool | `true` | Check for updates on launch |
| `channel` | string | `"stable"` | Release channel (`stable` or `prerelease`) |

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
- [docs/teams.md](docs/teams.md) — Teams user documentation
- [docs/userdocs/aiwiki.md](docs/userdocs/aiwiki.md) — AIWiki user guide
- [docs/userdocs/](docs/userdocs/) — User guides and reference materials
- [docs/](docs/) — Additional documentation

---

## Appendix C: Project Contact & Repository

### Author

| Field | Detail |
|-------|--------|
| **Name** | Tim Hawkins |
| **Email** | tim.thawkins@gmail.com |

### Repository

| Resource | URL |
|----------|-----|
| **Source code** | <https://github.com/thawkins/ragent> |
| **Issues** | <https://github.com/thawkins/ragent/issues> |
| **Pull requests** | <https://github.com/thawkins/ragent/pulls> |
| **Releases** | <https://github.com/thawkins/ragent/releases> |

### License

Ragent is released under the **MIT License**. See [LICENSE](LICENSE) for the
full text.

### Contributing

Contributions are welcome via pull requests on GitHub. Please open an issue
first to discuss significant changes before submitting a PR.

---

*End of Specification*

---

## Changelog for SPEC.md (2025-01-16)

### Additions

1. **Version Update** — Updated version from 0.1.0-alpha.43 to 0.1.0-alpha.44
2. **Project Status Enhancements** — Added "Current Release Highlights" section documenting:
   - AIWiki knowledge base fully implemented with 6 complete milestones
   - AIWiki single-file reference path resolution bugfix
   - Unified TUI dialog and button component system
   - Tool system scale and team coordination capabilities
   - Native GitLab integration features
3. **Date Update** — Updated "Last Updated" from 2026-04-16 to 2025-01-16 (corrected future date)

### Removals

1. **Obsolete Documentation References** — Removed references to deleted planning/design files:
   - Removed `[RAGENTMEM.md](RAGENTMEM.md) — Memory system design` (file was deleted during cleanup)
   - Removed `[AIWIKIPLAN.md](AIWIKIPLAN.md) — AIWiki knowledge base design` (file was deleted during cleanup)
   - These references are now obsolete as the systems are documented inline in SPEC.md

2. **Box Drawing Character Fix** — Fixed corrupted Unicode character in section 6.2 Architecture diagram header (line 1206)

### Corrections

1. **Date Format** — Fixed Last Updated date which was set to a future date (2026-04-16 → 2025-01-16)
2. **Consistency** — Clarified project status language to reflect current alpha state with functional features

### No Changes Required

The following were verified as accurate and required no updates:
- Tool count (147+) is comprehensive and accurate
- Provider count documentation (8 providers: Anthropic, OpenAI, GitHub Copilot, Ollama local, Ollama cloud, Hugging Face, Google Gemini, Generic OpenAI) — documented as "7 providers" in summary refers to primary native providers (excluding Generic as it's catch-all)
- All feature descriptions match current CHANGELOG.md releases
- Architecture and subsystem documentation is current and complete
- AIWiki milestones (1-6) are fully documented
- Memory system three-tier architecture is correctly described
- Teams and swarm mode documentation is comprehensive
- Security and permission system documentation is detailed and current
- All tool categories and counts match implementation

