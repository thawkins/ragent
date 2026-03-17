# ragent — Specification

**An open-source AI coding agent built for the terminal, implemented in Rust.**

ragent is a Rust reimplementation of [OpenCode](https://github.com/anomalyco/opencode) — the open-source AI coding agent. It provides the same core capabilities (multi-provider LLM orchestration, tool execution, TUI, client/server architecture, MCP support, LSP integration) rewritten from TypeScript/Bun into idiomatic, high-performance Rust.

---

## Table of Contents

1. [Goals & Non-Goals](#1-goals--non-goals)
2. [Architecture Overview](#2-architecture-overview)
3. [Core Modules](#3-core-modules)
   - 3.1 [CLI & Entry Point](#31-cli--entry-point) ✅
   - 3.2 [Configuration](#32-configuration) ✅
   - 3.3 [Provider System](#33-provider-system) ⚠️
   - 3.4 [Agent System](#34-agent-system) ⚠️
   - 3.5 [Session Management](#35-session-management) ✅
   - 3.6 [Message Model](#36-message-model) ✅
   - 3.7 [Tool System](#37-tool-system) ✅
   - 3.8 [Permission System](#38-permission-system) ⚠️
   - 3.9 [HTTP Server](#39-http-server) ✅
   - 3.10 [Terminal UI (TUI)](#310-terminal-ui-tui) ✅
   - 3.11 [MCP Client](#311-mcp-client) ✅
   - 3.12 [LSP Integration](#312-lsp-integration) ❌
   - 3.13 [Event Bus](#313-event-bus) ✅
   - 3.14 [Storage & Database](#314-storage--database) ✅
   - 3.15 [Shell Execution](#315-shell-execution) ✅
   - 3.16 [Snapshot & Undo](#316-snapshot--undo) ✅
   - 3.17 [Hooks](#317-hooks) ❌
   - 3.18 [Custom Agents](#318-custom-agents) ⚠️
   - 3.19 [Skills](#319-skills) ❌
   - 3.20 [Persistent Memory](#320-persistent-memory) ❌
   - 3.21 [Trusted Directories](#321-trusted-directories) ❌
   - 3.22 [Codebase Indexing & Semantic Search](#322-codebase-indexing--semantic-search) ❌
   - 3.23 [Post-Edit Diagnostics](#323-post-edit-diagnostics) ❌
   - 3.24 [Task Todo List](#324-task-todo-list) ✅
   - 3.25 [Prompt Enhancement](#325-prompt-enhancement) ❌
   - 3.26 [Hierarchical Custom Instructions](#326-hierarchical-custom-instructions) ⚠️
   - 3.27 [File Ignore Patterns](#327-file-ignore-patterns) ❌
   - 3.28 [Suggested Responses](#328-suggested-responses) ❌
   - 3.29 [Session Resume & Management](#329-session-resume--management) ⚠️
   - 3.30 [Git Worktree Isolation](#330-git-worktree-isolation) ❌
   - 3.31 [Context Compaction](#331-context-compaction) ⚠️
   - 3.32 [Headless / Pipe Mode](#332-headless--pipe-mode) ⚠️
   - 3.33 [Extended Thinking & Effort Levels](#333-extended-thinking--effort-levels) ✅
   - 3.34 [@ File References](#334--file-references) ❌
4. [Data Flow](#4-data-flow)
5. [Configuration File Format](#5-configuration-file-format)
6. [Rust Crate Map](#6-rust-crate-map)
7. [Project Layout](#7-project-layout)
8. [Build & Distribution](#8-build--distribution)
9. [Testing Strategy](#9-testing-strategy)
10. [Future / Stretch Goals](#10-future--stretch-goals)

---

### Implementation Status Summary

| Status | Count | Sections |
|--------|-------|----------|
| ✅ Implemented | 15 | CLI, Config, Session Mgmt, Messages, Tools, HTTP Server, TUI, MCP, Event Bus, Storage, Shell, Snapshot, Todo List, Extended Thinking, **Skills** |
| ⚠️ Partial | 8 | Providers (4/12), Agent System (8/10 agents), Permissions (core only), Custom Agents (struct only), Instructions (basic), Session Resume (by-ID only), Compaction (manual /compact), Headless (--no-tui only) |
| ❌ Not Started | 11 | LSP, Hooks, Memory, Trusted Dirs, Codebase Indexing, Post-Edit Diagnostics, Prompt Enhancement, File Ignore, Suggested Responses, Worktree, @ References |

**Overall: 34 sections — 44% fully implemented, 24% partial, 32% not yet started**

---

## 1. Goals & Non-Goals

### Goals

| # | Goal |
|---|------|
| G1 | Feature parity with OpenCode's core CLI agent (agents, tools, providers, sessions, permissions, MCP, LSP). |
| G2 | Single statically-linked binary — no runtime dependencies (Node, Bun, Python). |
| G3 | Cross-platform: Linux (x86_64, aarch64), macOS (x86_64, aarch64), Windows (x86_64). |
| G4 | Sub-second cold start; low memory footprint. |
| G5 | Client/server architecture: a local HTTP/WebSocket server that any frontend (TUI, web, mobile) can drive. |
| G6 | Provider-agnostic: first-class support for Anthropic, OpenAI, Google, Azure, AWS Bedrock, OpenRouter, and any OpenAI-compatible endpoint. |
| G7 | Safe tool execution with a permission system that gates file writes, shell commands, and external access. |
| G8 | Configuration-file compatible with OpenCode's `opencode.json` / `opencode.jsonc` format. |
| G9 | MCP (Model Context Protocol) client for extending tool capabilities via external servers. |
| G10 | LSP integration for code intelligence (diagnostics, go-to-definition, references). |

### Non-Goals (v1)

| # | Non-Goal |
|---|----------|
| N1 | Desktop GUI (Tauri/Electron) — TUI and HTTP server only. |
| N2 | Cloud-hosted multi-tenant service — ragent is a local-first tool. |
| N3 | Plugin system via dynamic loading (`.so`/`.dll`) — MCP is the extension point. |
| N4 | Enterprise/managed config (`/etc/opencode/`) — deferred to a later release. |
| N5 | Slack or third-party chat integrations. |

---

## 2. Architecture Overview

```
┌─────────────────────────────────────────────────────────┐
│                        ragent                           │
│                                                         │
│  ┌──────────┐   ┌──────────────┐   ┌────────────────┐  │
│  │   CLI    │──▶│  HTTP Server │◀──│  TUI (ratatui) │  │
│  │ (clap)   │   │  (axum)      │   │                │  │
│  └──────────┘   └──────┬───────┘   └────────────────┘  │
│                        │                                │
│            ┌───────────┴───────────┐                    │
│            ▼                       ▼                    │
│     ┌─────────────┐       ┌──────────────┐             │
│     │  Session     │       │  Event Bus   │             │
│     │  Manager     │       │  (tokio      │             │
│     │              │       │   broadcast) │             │
│     └──────┬──────┘       └──────────────┘             │
│            │                                            │
│     ┌──────┴──────┐                                     │
│     │   Agent     │                                     │
│     │   Loop      │                                     │
│     └──────┬──────┘                                     │
│            │                                            │
│   ┌────────┼─────────┬──────────┐                       │
│   ▼        ▼         ▼          ▼                       │
│ ┌──────┐ ┌──────┐ ┌───────┐ ┌──────────┐               │
│ │ LLM  │ │Tools │ │Permis-│ │ MCP      │               │
│ │Stream│ │      │ │sions  │ │ Client   │               │
│ └──┬───┘ └──────┘ └───────┘ └──────────┘               │
│    │                                                    │
│ ┌──┴────────────────────────────────────┐               │
│ │         Provider Adapters             │               │
│ │  Anthropic │ OpenAI │ Google │ Azure  │               │
│ │  Bedrock   │ OpenRouter │ Custom     │               │
│ └───────────────────────────────────────┘               │
│                                                         │
│ ┌───────────────────────────────────────┐               │
│ │  Storage (SQLite via rusqlite)        │               │
│ └───────────────────────────────────────┘               │
└─────────────────────────────────────────────────────────┘
```

All async work runs on the **tokio** runtime. LLM responses are streamed via Server-Sent Events (SSE) / chunked HTTP. The TUI connects to the server over a local Unix socket or TCP, so the same binary can serve headless CI, interactive terminal, and remote web clients.

---

## 3. Core Modules

### 3.1 CLI & Entry Point ✅

| Aspect | Detail |
|--------|--------|
| Crate | `clap` (derive) |
| Binary name | `ragent` |
| Entry | `src/main.rs` → `src/cli/mod.rs` |

#### Subcommands

| Command | Description | Status |
|---------|-------------|--------|
| *(default)* | Launch interactive TUI session | ✅ |
| `run <prompt>` | Execute a one-shot agent run, print result, exit | ✅ |
| `serve` | Start HTTP/WebSocket server only (headless) | ✅ |
| `session list` | List saved sessions | ✅ |
| `session resume <id>` | Resume a previous session | ✅ |
| `session export <id>` | Export session to JSON | ✅ |
| `session import <file>` | Import session from JSON | ✅ |
| `auth <provider>` | Configure API key for a provider | ✅ |
| `models` | List available models across configured providers | ✅ |
| `config` | Print resolved configuration | ✅ |
| `mcp list` | List configured MCP servers and their status | ❌ |
| `upgrade` | Self-update the binary | ❌ |
| `uninstall` | Remove ragent and its data | ❌ |

#### Global Flags

| Flag | Default | Description | Status |
|------|---------|-------------|--------|
| `--config <path>` | auto-detected | Path to config file | ✅ |
| `--model <provider/model>` | from config | Override model for this run | ✅ |
| `--agent <name>` | `build` | Override default agent | ✅ |
| `-p`, `--prompt <text>` | n/a | Execute a single prompt programmatically, print result, and exit | ❌ |
| `--log-level <level>` | `warn` | Logging verbosity (`trace`, `debug`, `info`, `warn`, `error`) | ✅ |
| `--print-logs` | `false` | Print logs to stderr | ❌ |
| `--no-tui` | `false` | Disable TUI, use plain stdout | ✅ |
| `--yes` | `false` | Auto-approve all permission prompts | ✅ |
| `--allow-all-tools` | `false` | Allow all tools without manual approval | ❌ |
| `--allow-tool <spec>` | n/a | Allow a specific tool without approval (repeatable). Spec: `'shell(cmd)'`, `'write'`, or `'McpServer(tool)'` | ❌ |
| `--deny-tool <spec>` | n/a | Deny a specific tool (repeatable, overrides `--allow-tool` and `--allow-all-tools`) | ❌ |
| `--server <addr>` | n/a | Connect to an existing ragent server | ❌ |
| `--continue` | `false` | Resume the most recent session | ❌ |
| `--resume` | `false` | Open session picker to search/filter and resume a session | ❌ |
| `--from-pr <number>` | n/a | Resume or start a session linked to a pull request | ❌ |
| `--worktree <name>` | n/a | Run session in an isolated git worktree (auto-creates if needed) | ❌ |
| `--permission-mode <mode>` | `default` | Permission mode: `default`, `acceptEdits`, `dontAsk`, `bypassPermissions`, `plan` | ❌ |
| `--output-format <fmt>` | `text` | Output format for `-p` mode: `text`, `json`, `stream-json` | ❌ |

---

### 3.2 Configuration ✅

#### File Format

ragent reads `ragent.json` / `ragent.jsonc` (JSON with comments) and also supports OpenCode-compatible `opencode.json` / `opencode.jsonc` for drop-in migration.

#### Load Precedence (lowest → highest)

1. Compiled-in defaults
2. Global config: `$XDG_CONFIG_HOME/ragent/ragent.json` (or `~/.config/ragent/ragent.json`)
3. Custom path: `$RAGENT_CONFIG` environment variable
4. Project config: `./ragent.json` (or `./opencode.json`) in the working directory
5. `.ragent/` directory (instructions, agents, hooks)
6. Inline: `$RAGENT_CONFIG_CONTENT` environment variable (JSON string)
7. CLI flags (highest priority)

#### Schema

```rust
/// Top-level configuration.
pub struct Config {
    /// Display name shown in prompts.
    pub username: Option<String>,

    /// Default agent to use when starting a session.
    pub default_agent: Option<String>,

    /// Provider configurations keyed by provider ID.
    pub provider: HashMap<String, ProviderConfig>,

    /// Global permission rules.
    pub permission: PermissionRuleset,

    /// Agent definitions / overrides.
    pub agent: HashMap<String, AgentConfig>,

    /// Custom slash-commands.
    pub command: HashMap<String, CommandDef>,

    /// MCP server definitions.
    pub mcp: HashMap<String, McpServerConfig>,

    /// Additional system instructions (paths or inline strings).
    pub instructions: Vec<String>,

    /// Experimental feature flags.
    pub experimental: ExperimentalFlags,
}
```

Merging follows **deep-merge** semantics: maps are merged key-by-key, vectors are concatenated, scalars are overwritten.

---

### 3.3 Provider System ⚠️

The provider system abstracts LLM API differences behind a unified streaming interface.

#### Supported Providers

| Provider ID | SDK / Protocol | Auth | Status |
|-------------|---------------|------|--------|
| `anthropic` | Anthropic Messages API | `ANTHROPIC_API_KEY` or `ragent auth` | ✅ Implemented |
| `copilot` | OpenAI-compatible (GitHub Copilot) | Copilot OAuth token (auto-discovered) or `ragent auth` | ✅ Implemented |
| `openai` | OpenAI Chat Completions API | `OPENAI_API_KEY` or `ragent auth` | ✅ Implemented |
| `ollama` | OpenAI-compatible (local/remote) | None (optional `OLLAMA_API_KEY`) | ✅ Implemented |
| `google` | Google Generative AI API | `GOOGLE_API_KEY` | Planned |
| `azure` | Azure OpenAI (OpenAI-compatible) | `AZURE_OPENAI_API_KEY` + endpoint | Planned |
| `bedrock` | AWS Bedrock (SigV4) | AWS credentials chain | Planned |
| `openrouter` | OpenAI-compatible | `OPENROUTER_API_KEY` | Planned |
| `xai` | OpenAI-compatible | `XAI_API_KEY` | Planned |
| `mistral` | OpenAI-compatible | `MISTRAL_API_KEY` | Planned |
| `groq` | OpenAI-compatible | `GROQ_API_KEY` | Planned |
| `custom` | Any OpenAI-compatible endpoint | User-defined | Planned |

#### Model Descriptor

```rust
pub struct ModelInfo {
    pub id: String,              // e.g. "claude-sonnet-4-20250514"
    pub provider_id: String,     // e.g. "anthropic"
    pub name: String,            // Human-friendly name
    pub cost: Cost,              // { input_per_mtok, output_per_mtok }
    pub capabilities: Capabilities, // { reasoning, streaming, vision, tool_use }
    pub context_window: usize,   // Max tokens
    pub max_output: Option<usize>,
}
```

#### Streaming Interface

```rust
#[async_trait]
pub trait LlmStream {
    /// Send messages and stream back events.
    async fn chat(
        &self,
        request: ChatRequest,
    ) -> Result<Pin<Box<dyn Stream<Item = StreamEvent> + Send>>>;
}

pub enum StreamEvent {
    ReasoningStart,
    ReasoningDelta { text: String },
    ReasoningEnd,
    TextDelta { text: String },
    ToolCallStart { id: String, name: String },
    ToolCallDelta { id: String, args_json: String },
    ToolCallEnd { id: String },
    Usage { input_tokens: u64, output_tokens: u64 },
    Error { error: anyhow::Error },
    Finish { reason: FinishReason },
}

pub enum FinishReason {
    Stop,
    ToolUse,
    Length,
    ContentFilter,
}
```

Each provider implements `LlmStream`. Internally, the Anthropic adapter uses the native Messages API; all OpenAI-compatible providers share a single `OpenAiCompatibleStream` implementation parameterized by base URL and auth.

#### Ollama Provider

The Ollama provider connects to a local or remote [Ollama](https://ollama.com) server. It uses Ollama's OpenAI-compatible `/v1/chat/completions` endpoint for streaming chat completions and the `/api/tags` endpoint for dynamic model discovery.

**Key characteristics:**

| Feature | Detail |
|---------|--------|
| API endpoint | `{base_url}/v1/chat/completions` (OpenAI-compatible) |
| Model discovery | `{base_url}/api/tags` — queries available models at runtime |
| Authentication | None required for local servers; optional Bearer token via `OLLAMA_API_KEY` for remote |
| Default base URL | `http://localhost:11434` |
| Base URL override | `OLLAMA_HOST` environment variable or `provider.ollama.api.base_url` in config |
| Cost | Free (all models run locally) |
| Streaming | SSE via `data:` lines, identical to OpenAI format |
| Tool calls | Supported (model-dependent — works with llama3.x, qwen2.5, etc.) |

**Environment variables:**

| Variable | Purpose | Default |
|----------|---------|---------|
| `OLLAMA_HOST` | Ollama server URL | `http://localhost:11434` |
| `OLLAMA_API_KEY` | Optional Bearer token for authenticated remote servers | (empty — no auth) |

**Model discovery:**

The `list_ollama_models()` function queries `/api/tags` and returns `ModelInfo` for each installed model, including:
- Model ID (e.g. `llama3.2:latest`, `qwen2.5-coder:32b`)
- Human-readable display name with parameter count
- Estimated context window based on parameter size (8K–131K)

```bash
# Discover models from a running Ollama server
ragent models --provider ollama

# Discover from a remote server
ragent models --ollama-url http://remote-server:11434
```

**Configuration example:**

```jsonc
{
  "provider": {
    "ollama": {
      "api": { "base_url": "http://localhost:11434" },
      "models": {
        "llama3.2": { "name": "Llama 3.2" },
        "qwen2.5-coder:32b": { "name": "Qwen 2.5 Coder 32B" }
      }
    }
  },
  "agent": {
    "local": {
      "model": "ollama/llama3.2",
      "prompt": "You are a helpful coding assistant."
    }
  }
}
```

**Usage:**

```bash
# Use a specific Ollama model
ragent run --model ollama/llama3.2 "Explain this code"

# Use a custom agent backed by Ollama
ragent run --agent local "Write a unit test"

# Point at a remote Ollama server
OLLAMA_HOST=http://gpu-server:11434 ragent run --model ollama/deepseek-r1:70b "Refactor this"
```

#### GitHub Copilot Provider

The Copilot provider connects to the [GitHub Copilot](https://github.com/features/copilot) API using plan-specific endpoints (e.g. `https://api.individual.githubcopilot.com` for Individual/Pro plans). It uses the same OpenAI-compatible chat completions format and includes automatic token discovery via device flow, `gh` CLI, or IDE configuration.

**Key characteristics:**

| Feature | Detail |
|---------|--------|
| API endpoint | Plan-specific (e.g. `https://api.individual.githubcopilot.com/chat/completions`) |
| Model discovery | Queries available models at runtime from the plan-specific endpoint |
| Authentication | Copilot OAuth token via device flow (`ghu_*`), `gh` CLI (`gho_*`), or env var |
| Cost | Included with GitHub Copilot subscription |
| Streaming | SSE via `data:` lines, identical to OpenAI format |
| Tool calls | Supported |

**Authentication flow (priority order):**

1. `GITHUB_COPILOT_TOKEN` environment variable (highest priority)
2. DB-stored device flow token (from interactive setup)
3. `gh auth token` CLI fallback
4. Auto-discovery from `~/.config/github-copilot/apps.json` (Linux/macOS)
5. Auto-discovery from `%LOCALAPPDATA%/github-copilot/apps.json` (Windows)

When authenticating interactively, ragent uses the GitHub device flow: a one-time code is displayed that the user enters at `https://github.com/login/device`. Press `c` on the device code screen to copy the code to the clipboard.

**API base resolution:**

The Copilot token exchange (`copilot_internal/v2/token`) may return plan-specific `endpoints`. If not, ragent discovers the correct API base via `copilot_internal/user`, trying both the device flow token and the `gh` CLI token for broader scope coverage.

**Default models:**

| Model ID | Name | Context | Capabilities |
|----------|------|---------|-------------|
| `gpt-4o` | GPT-4o | 128K | streaming, vision, tool_use |
| `gpt-4o-mini` | GPT-4o Mini | 128K | streaming, vision, tool_use |
| `claude-sonnet-4` | Claude Sonnet 4 | 200K | reasoning, streaming, vision, tool_use |
| `o3-mini` | o3-mini | 200K | reasoning, streaming, tool_use |

**Usage:**

```bash
# Use Copilot with auto-discovered token
ragent run --model copilot/gpt-4o "Explain this code"

# Use Claude via Copilot
ragent run --model copilot/claude-sonnet-4 "Refactor this module"

# Explicit token
GITHUB_COPILOT_TOKEN=ghu_xxxx ragent run --model copilot/o3-mini "Write tests"
```

**Configuration example:**

```jsonc
{
  "provider": {
    "copilot": {}
  },
  "agent": {
    "copilot-agent": {
      "model": "copilot/claude-sonnet-4",
      "prompt": "You are an expert code reviewer."
    }
  }
}
```

---

### 3.4 Agent System ⚠️

Agents define *personas* — a combination of system prompt, model selection, tool access, and permission rules.

#### Agent Definition

```rust
pub struct AgentInfo {
    /// Unique identifier (e.g. "build", "plan", "general").
    pub name: String,
    /// Human-readable description.
    pub description: Option<String>,
    /// Whether this agent appears in the Tab-switch menu ("primary")
    /// or is invokable as a sub-agent ("subagent").
    pub mode: AgentMode,  // Primary | Subagent | All
    /// Whether this agent is hidden from the UI.
    pub hidden: bool,
    /// LLM sampling temperature.
    pub temperature: Option<f32>,
    /// LLM top-p sampling.
    pub top_p: Option<f32>,
    /// Override model for this agent.
    pub model: Option<ModelRef>,  // { provider_id, model_id }
    /// System prompt (can include template variables).
    pub prompt: Option<String>,
    /// Permission ruleset specific to this agent.
    pub permission: PermissionRuleset,
    /// Allowed tool groups (e.g. ["read", "edit", "command", "mcp"]).
    /// If None, all groups are available.
    pub tool_groups: Option<Vec<ToolGroup>>,
    /// Maximum tool-call iterations before stopping.
    pub max_steps: Option<u32>,
    /// Additional provider-specific options (e.g. extended_thinking).
    pub options: HashMap<String, serde_json::Value>,
}
```

#### Built-in Agents

| Name | Mode | Description | Key Permission Traits | Status |
|------|------|-------------|----------------------|--------|
| `ask` | Primary | Quick Q&A — answers questions without tools | Read-only; max 1 step | ✅ |
| `general` | Primary | General-purpose coding agent; full read/write/execute access (default) | Allows all tools; denies editing `.env*` files | ✅ |
| `build` | Subagent | Build/test agent; compile, run tests, fix errors | Full access; max 30 steps | ✅ |
| `plan` | Subagent | Read-only analysis & planning agent | Denies all edit/write tools; asks before bash | ✅ |
| `explore` | Subagent | Fast codebase search (invoked via `@explore`) | Read-only: grep, glob, list, read, bash, web | ✅ |
| `title` | Internal | Generates session titles | Hidden, no tools | ✅ |
| `summary` | Internal | Generates session summaries | Hidden, no tools | ✅ |
| `compaction` | Internal | Compresses long conversation history | Hidden, no tools | ✅ |
| `orchestrator` | Primary | Task orchestrator — decomposes complex work into subtasks and delegates to specialized agents | Read-only; delegates via `new_task` tool | ❌ |
| `debug` | Primary | Systematic debugger — methodical problem diagnosis and resolution | Full access; diagnostic-focused prompt | ❌ |

Agents can be switched at runtime using the `/agent` slash command or by cycling with `Tab`/`Shift+Tab`.

#### Tool Groups

Each agent can restrict its available tools by specifying allowed tool groups. This provides safety boundaries — e.g., the `ask` agent cannot modify files, and the `orchestrator` cannot directly execute commands.

| Group | Tools Included | Purpose |
|-------|---------------|---------|
| `read` | `read`, `list`, `glob`, `grep`, `office_read`, `office_info`, `pdf_read` | File system reading and exploration |
| `edit` | `write`, `create`, `edit`, `multiedit`, `patch`, `office_write`, `pdf_write` | File creation and modification |
| `command` | `bash` | Terminal command execution |
| `search` | `grep`, `glob`, `codebase_search` | Pattern and semantic searching |
| `mcp` | MCP tools (dynamic) | External tool integration via MCP |
| `web` | `webfetch`, `websearch` | Web access |
| `workflow` | `question`, `new_task`, `switch_agent`, `todo_read`, `todo_write` | Task management and user interaction |

If `tool_groups` is `None`, all groups are available. The `workflow` group tools (`question`, `new_task`, `switch_agent`) are always available regardless of group restrictions.

#### Orchestrator Agent & Task Delegation

The `orchestrator` agent breaks complex tasks into focused subtasks and delegates them to specialized agents. Each subtask runs in its own isolated context:

1. User submits a complex request to the orchestrator
2. Orchestrator analyzes the request and decomposes it into subtasks
3. Each subtask is created via `new_task` tool, specifying:
   - Target agent (e.g., `general` for coding, `plan` for analysis, `explore` for search)
   - Task description with all necessary context passed explicitly
   - Expected deliverable
4. Subtask runs in its own conversation context (no shared history with parent)
5. On completion, subtask returns a summary to the orchestrator
6. Orchestrator continues with remaining subtasks or synthesizes final result

This enables:
- **Context isolation**: Subtasks don't pollute each other's context windows
- **Specialized agents**: Each subtask uses the best agent for the job
- **Parallel work**: Independent subtasks can run concurrently (future)

#### Agent Resolution

Agents are merged from multiple sources (lowest → highest priority):
1. Built-in defaults (compiled in)
2. Global config `~/.config/ragent/ragent.json` → `agent.*`
3. Project config `./ragent.json` → `agent.*`
4. `.ragent/agent-*.md` files (prompt overrides)
5. CLI `--agent` flag

#### AGENTS.md Auto-Loading

On session start, `build_system_prompt()` checks for an `AGENTS.md` file in the project working directory. If found, its contents are injected into the system prompt under a "Project Guidelines" section. This applies to all multi-step agents (general, build, plan, explore) but is skipped for single-step agents (ask, title, summary, compaction).

On the first message of a session, an init exchange prompts the model to acknowledge the guidelines. The acknowledgement streams to the TUI message window as a separate assistant message before the main response begins. This init exchange is display-only — it is not stored in the conversation history or fed into subsequent LLM calls.

---

### 3.5 Session Management ✅

A **session** is a persistent conversation between the user and an agent, stored in SQLite.

```rust
pub struct Session {
    pub id: String,           // ULID
    pub title: String,
    pub project_id: String,
    pub directory: PathBuf,
    pub parent_id: Option<String>,  // For sub-agent sessions
    pub version: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub archived_at: Option<DateTime<Utc>>,
    pub summary: Option<SessionSummary>,
}

pub struct SessionSummary {
    pub additions: u32,
    pub deletions: u32,
    pub files_changed: u32,
    pub diffs: Vec<FileDiff>,
}
```

#### Session Lifecycle

1. **Create** — allocate ID, set working directory, choose agent
2. **Chat** — user sends message → agent loop runs → response stored
3. **Continue** — user sends follow-up → messages appended, agent re-enters loop
4. **Compact** — when context nears limit, compress old messages via `compaction` agent
5. **Archive** — mark session as archived (soft delete)
6. **Resume** — `ragent session resume <id>` loads the session by ID, restores the full message history and working directory, and launches the TUI in the chat screen. The `App::load_session()` method verifies the session exists, loads all persisted messages, and updates the status bar
7. **Export** — `ragent session export <id>` serializes messages to JSON on stdout
8. **Import** — `ragent session import <file>` deserializes messages from a JSON file, creates a new session in storage, and re-parents each message with a fresh ULID into the new session. Prints the new session ID on success

---

### 3.6 Message Model ✅

Messages use a **parts-based** structure supporting text, tool calls, and reasoning traces.

```rust
pub struct Message {
    pub id: String,           // ULID
    pub session_id: String,
    pub role: Role,           // User | Assistant
    pub parts: Vec<MessagePart>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

pub enum Role {
    User,
    Assistant,
}

pub enum MessagePart {
    Text {
        text: String,
    },
    ToolCall {
        tool: String,
        call_id: String,
        state: ToolCallState,
    },
    Reasoning {
        text: String,
    },
}

pub struct ToolCallState {
    pub status: ToolCallStatus, // Pending | Running | Completed | Error
    pub input: serde_json::Value,
    pub output: Option<serde_json::Value>,
    pub error: Option<String>,
    pub duration_ms: Option<u64>,
}
```

---

### 3.7 Tool System ✅

Tools are the capabilities available to agents for interacting with the filesystem, running commands, and searching code.

#### Tool Registry

Each tool implements the `Tool` trait:

```rust
#[async_trait]
pub trait Tool: Send + Sync {
    /// Unique tool name.
    fn name(&self) -> &str;

    /// Human-readable description for the LLM.
    fn description(&self) -> &str;

    /// JSON Schema for tool parameters.
    fn parameters_schema(&self) -> serde_json::Value;

    /// Permission category (e.g. "read", "edit", "bash").
    fn permission_category(&self) -> &str;

    /// Execute the tool and return output.
    async fn execute(
        &self,
        input: serde_json::Value,
        context: &ToolContext,
    ) -> Result<ToolOutput>;
}

pub struct ToolOutput {
    pub content: String,
    pub metadata: Option<serde_json::Value>,
}

pub struct ToolContext {
    pub session_id: String,
    pub working_dir: PathBuf,
    pub permission_checker: Arc<dyn PermissionChecker>,
    pub event_bus: Arc<EventBus>,
}
```

#### Built-in Tools

**Implemented (23 tools):**

| Tool | Permission | Description | Status |
|------|-----------|-------------|--------|
| `read` | `file:read` | Read file contents (with optional line range) | ✅ |
| `write` | `file:write` | Create or overwrite a file | ✅ |
| `create` | `file:write` | Create a new file, truncating if it already exists | ✅ |
| `edit` | `file:write` | Replace a specific string in a file (atomic, single match) | ✅ |
| `multiedit` | `file:write` | Apply multiple edits to one or more files atomically (all-or-nothing) | ✅ |
| `patch` | `file:write` | Apply a unified diff patch with optional fuzzy matching to one or more files | ✅ |
| `rm` | `file:write` | Delete a single file (no wildcards, explicit path required) | ✅ |
| `bash` | `bash:execute` | Execute a shell command and capture output (with timeout and signal handling) | ✅ |
| `grep` | `file:read` | Search file contents using regex patterns with context lines | ✅ |
| `glob` | `file:read` | Find files matching glob patterns (recursive) | ✅ |
| `list` | `file:read` | List directory contents with tree-like output (depth control) | ✅ |
| `question` | `question` | Ask the user a question and wait for a response (interactive) | ✅ |
| `office_read` | `file:read` | Read content from Word, Excel, or PowerPoint files (with range/sheet/slide selection) | ✅ |
| `office_write` | `file:write` | Write content to Word, Excel, or PowerPoint files (creates/overwrites) | ✅ |
| `office_info` | `file:read` | Get metadata and structural info about Office documents (format, size, counts) | ✅ |
| `pdf_read` | `file:read` | Read text and metadata from PDF files (with page range selection) | ✅ |
| `pdf_write` | `file:write` | Create PDF files from structured content (text, tables, headings, images) | ✅ |
| `webfetch` | `web` | Fetch URL content with optional HTML-to-text conversion (timeout, max length) | ✅ |
| `websearch` | `web` | Perform web search via Tavily API and return ranked results with snippets | ✅ |
| `plan_enter` | `plan` | Delegate to the plan agent for read-only codebase analysis (event-driven) | ✅ |
| `plan_exit` | `plan` | Return from plan agent to previous agent with summary result | ✅ |
| `todo_read` | `todo` | Read the session TODO list with optional status filtering (pending/in_progress/done/blocked) | ✅ |
| `todo_write` | `todo` | Add, update, remove, or clear TODO items (persistent per session) | ✅ |

**Planned (not yet implemented):**

| Tool | Permission | Description | Status | Notes |
|------|-----------|-------------|--------|-------|
| `new_task` | `workflow` | Create a subtask delegated to a specific agent with isolated context | 🔲 | Subtask delegation system |
| `switch_agent` | `workflow` | Switch the active agent for the current session | 🔲 | Agent switching within session |
| `codebase_search` | `file:read` | Semantic search across indexed codebase using embeddings | 🔲 | Tree-sitter + embedding provider |
| `generate_image` | `image` | Generate images from text prompts using AI image models | 🔲 | Vision model integration |

#### Tool Execution Flow

1. LLM emits a `tool_use` block with tool name + JSON arguments
2. Deserialize arguments against the tool's parameter schema
3. Determine permission category and file patterns involved
4. Evaluate permission rules → `Allow`, `Deny`, or `Ask`
5. If `Ask` → emit `PermissionRequested` event → TUI shows prompt → wait for reply
6. If denied → return error to LLM ("permission denied")
7. If allowed → call `tool.execute(input, context)`
8. Capture output (stdout, file contents, search results, etc.)
9. Return `ToolOutput` → serialize into the next LLM request as a tool result
10. LLM processes the result and decides whether to call another tool or respond

#### Built-in Tool Implementation Details

**File Tools (`read`, `write`, `create`, `edit`, `multiedit`, `patch`, `rm`):**

- All file tools resolve paths relative to the session's `working_dir` unless absolute
- Path safety: File operations are checked against `.ragentignore` patterns (if configured)
- Snapshot capture: The `edit`, `multiedit`, `patch`, `write`, and `create` tools create pre-execution snapshots for undo capability
- `read` tool: Supports optional `start_line` and `end_line` parameters for range selection
- `create` tool: Truncates existing files; creates parent directories automatically
- `write` tool: Overwrites or creates files; creates parent directories
- `edit` tool: Single atomic string replacement; returns error if match count ≠ 1
- `multiedit` tool: Array of edits applied atomically; validates all matches before writing (all-or-nothing)
  - Parameters: `edits` array with each element containing `path`, `old_str`, `new_str`
  - Validation: Each `old_str` must match exactly once in its target file, or entire operation fails
  - Returns: count of files modified, total edits applied, total lines changed in metadata
  - Implementation: Phase 1 reads all files, Phase 2 applies edits in-memory and validates, Phase 3 writes all files
  - Uses `replacen(..., 1)` to ensure atomic single-match replacement per edit
  - Metadata provides granular counts for TUI display of multi-file operations
- `patch` tool: Unified diff application with configurable fuzzy matching (context line tolerance)
  - Parameters: `patch` string, optional `path` override, optional `fuzz` tolerance (default 0)
  - Supports multi-file patches from `---`/`+++` headers in standard unified diff format
  - Hunk application uses context-aware line matching with fuzzy tolerance for drift-tolerant patching
  - Hunks applied in reverse order (highest line numbers first) to avoid shifting line numbers for later hunks
  - Returns: count of hunks applied, count of files modified, total lines changed in metadata
  - Parses unified diff: `@@` hunk headers with `-old_start,old_count +new_start,new_count`
  - Implements bidirectional search from target line with configurable fuzz tolerance
  - Preserves trailing newlines from original files
- `rm` tool: Explicit single-file deletion; rejects wildcards and glob patterns

**Shell Execution (`bash`):**

- Command runs with `kill_on_drop(true)` — orphan processes are cleaned up on cancellation
- Timeout: Default 120 seconds (configurable per invocation)
- Output: Combines stdout + stderr up to a limit (truncated if too large)
- Environment: Sanitized — secrets not forwarded; `RAGENT`, `RAGENT_SESSION_ID` set
- Working directory: Locked to session root (unless `external_directory` permission granted)

**Search Tools (`grep`, `glob`):**

- `grep`: Regex-based pattern matching with line number reporting and context lines
- `glob`: Recursive glob matching with depth limit (default: 100 matches before truncating)
- Both exclude files matching `.ragentignore` patterns (if configured)

**Directory Listing (`list`):**

- Tree-like output with indentation
- Configurable depth parameter (default: 2)
- Excludes ignored files (`.ragentignore`)

**Document Tools (`office_read`, `office_write`, `office_info`, `pdf_read`, `pdf_write`):**

- All document tools use format detection via file extension (`.docx`, `.xlsx`, `.pptx`, `.pdf`)
- Office tools leverage native Rust SDKs (`docx-rust` for Word, `calamine` for Excel, `ooxmlsdk` for PowerPoint)
- `office_read` / `pdf_read`: Support optional range/sheet/slide/page selection parameters
- `office_write` / `pdf_write`: Create new files or overwrite existing; output truncated at 100 KB if needed
- `office_info`: Extracts metadata (title, author, sheet/slide counts, word/row counts, etc.)
- All document tools run blocking operations in `tokio::task::spawn_blocking` to avoid blocking the async runtime

**Web Tools (`webfetch`, `websearch`):**

- `webfetch`: HTTP GET with automatic HTML-to-text conversion (via `html2text` crate)
  - Parameters: `url` (required), `format` ('raw'|'text', default 'text'), `max_length`, `timeout`
  - Follows up to 10 redirects; User-Agent set to identify ragent
  - HTML processing: Uses `html2text` crate for semantic conversion, falls back to tag-stripping on failure
  - Truncation: Truncates at character boundary (avoids splitting multibyte UTF-8 characters)
  - Returns metadata: HTTP status, content-type, content-length, final line count
  - Timeout default: 30 seconds; max content default: 50 KB
  - URL scheme validation: only http:// and https:// allowed
  - Error handling: Graceful HTTP error responses with status code and message
  - Implementation: Uses `reqwest` client with timeout, redirect policy, and custom User-Agent
- `websearch`: Uses **Tavily API** (https://api.tavily.com/search) as primary backend
  - Requires `TAVILY_API_KEY` environment variable (free keys at tavily.com)
  - Parameters: `query` (required), `num_results` (default 5, max 20)
  - Returns structured results: `{ title, url, snippet }` for each result
  - Snippet formatting: Truncated to ~200 chars with ellipsis ("…") if longer
  - Output format: Numbered list with title, URL, and snippet per result
  - Error handling: Graceful failure with helpful setup message if API key missing, auth error handling for 401/403
  - Returns metadata: query, result count, formatted line count
  - Implementation: Tavily request includes `include_answer: false` flag to focus on search results only

**Agent Delegation Tools (`plan_enter`, `plan_exit`):**

- `plan_enter`: Suspends current agent, publishes `Event::AgentSwitchRequested` with task details
  - Parameters: `task` (required, what to analyze), `context` (optional, additional info)
  - Returns metadata with `agent_switch: "plan"` flag (signals session processor to break loop)
  - Event publishing: `AgentSwitchRequested { session_id, to: "plan", task, context }`
  - Output format: Includes task description and optional context in friendly message
  - Allows TUI to switch active agent and forward task to plan agent
  - Plan agent has access only to: `read`, `grep`, `glob`, `list`, `bash` tools (read-only)
  - Plan agent max_steps: 20 (prevents runaway analysis loops)
  - Implementation: Event is detected by session processor to break agent loop
- `plan_exit`: Returns control to previous agent via `Event::AgentRestoreRequested`
  - Parameters: `summary` (required, the plan/analysis result to return)
  - Returns metadata with `agent_restore: true` flag (signals session processor to break and restore)
  - Event publishing: `AgentRestoreRequested { session_id, summary }`
  - Output format: Acknowledgement message including the summary for TUI display
  - Injects summary into conversation as tool output for previous agent to continue from
  - Only available when `plan` agent is active (enforced by tool registry filtering)
  - Implementation: Session processor detects `agent_restore` flag and pops agent stack
- Both tools implemented in same module (`tool/plan.rs`)
- Event system: Built on `tokio::sync::broadcast` channel for real-time agent switching

**TODO Management (`todo_read` / `todo_write`):**

- Session-scoped persistent storage in SQLite (`todos` table per storage)
- `todo_read` tool:
  - Parameters: `status` filter (optional: 'pending'|'in_progress'|'done'|'blocked'|'all')
  - Returns formatted markdown list with status icons:
    - ⏳ pending
    - 🔄 in_progress
    - ✅ done
    - 🚫 blocked
  - Each item displays: `icon id title [status]`
  - Optional description on next line if present
  - Metadata: count of returned items, status filter applied
- `todo_write` tool:
  - Actions: `add` | `update` | `remove` | `clear`
  - `add` action: Creates todo with auto-generated ID if not provided, default status 'pending'
    - Required: `title` (must not be empty)
    - Optional: `status`, `description`, `id`
    - ID auto-generation: `todo-{timestamp_millis % 1_000_000}`
  - `update` action: Changes title/status/description of existing todo (at least one field required)
    - Required: `id`
    - Optional: `title`, `status`, `description`
    - Fails if todo not found in current session
  - `remove` action: Deletes specific todo by ID
    - Required: `id`
    - Fails if todo not found in current session
  - `clear` action: Removes all todos for current session, returns count cleared
    - No additional parameters
  - All write actions return: summary message + updated full todo list
  - Output formatting: Uses same markdown format as `todo_read` for consistency
- Storage schema: `todos(id, session_id, title, status, description, created_at, updated_at)`
- Metadata includes: action type, new total count, error details if validation fails
- Valid statuses for write: pending, in_progress, done, blocked
- Valid statuses for read filter: pending, in_progress, done, blocked, all

**Interactive Tool (`question`):**

- Emits `PermissionRequested` event with user prompt
- Waits for response via event bus
- Returns user's text as tool output
- Used by agents to prompt for clarification, approval, or input

---

### 3.8 Permission System ⚠️

Permissions gate every tool invocation. Rules are pattern-matched against file paths and tool categories.

#### Rule Structure

```rust
pub enum PermissionAction {
    Allow,
    Deny,
    Ask,
}

pub struct PermissionRule {
    /// Permission category: "read", "edit", "bash", "web", etc.
    pub permission: String,
    /// Glob pattern for matching paths (e.g. "*.env*", "src/**/*.rs").
    pub pattern: String,
    /// Action to take when the rule matches.
    pub action: PermissionAction,
}

pub type PermissionRuleset = Vec<PermissionRule>;
```

#### Evaluation Order

1. Agent-specific rules (most specific)
2. Project config rules
3. Global config rules
4. Built-in defaults (most general)

First matching rule wins. If no rule matches, the default is `Ask`.

#### Special Permissions

| Permission | Triggers On |
|------------|------------|
| `edit` | `write`, `create`, `edit`, `multiedit`, `patch` tools |
| `bash` | `bash` tool (all shell commands) |
| `external_directory` | Any file access outside the project root |
| `doom_loop` | Agent exceeding `max_steps` iterations |
| `read` | `read`, `grep`, `glob`, `list` tools |
| `web` | `webfetch`, `websearch` tools |

#### Ask Flow (Interactive)

```
Agent requests tool "edit" on "src/main.rs"
  → PermissionChecker evaluates rules → result: Ask
  → EventBus emits PermissionRequested { id, tool, paths }
  → TUI displays: "Allow editing src/main.rs? [once / always / deny]"
  → User selects "always"
  → EventBus emits PermissionReplied { id, decision: Always }
  → PermissionChecker records "always" rule in memory for this session
  → Tool executes
```

#### Permission Modes

Permission modes provide named presets that control the approval flow across the entire session. The active mode can be set via `--permission-mode` CLI flag, `/permissions` slash command, or by cycling with `Shift+Tab` in the TUI.

| Mode | Description |
|------|-------------|
| `default` | Standard mode — ask for dangerous operations, auto-allow reads |
| `acceptEdits` | Auto-approve file edits; still ask for bash, web, and external directory access |
| `dontAsk` | Auto-approve all tool invocations within the project directory; ask for external access |
| `bypassPermissions` | No permission prompts at all (use with caution — intended for trusted automation) |
| `plan` | Read-only — deny all write/edit/bash tools; only allow exploration and planning |

#### Sandbox Settings

For additional isolation, bash commands can be restricted via a sandbox configuration that limits filesystem and network access:

```jsonc
{
  "permissions": {
    "sandbox": {
      "enabled": true,
      "writable_paths": ["./src", "./tests"],
      "readable_paths": ["/usr", "/etc"],
      "network": "allow",          // "allow" | "deny" | "local-only"
      "env_passthrough": ["HOME", "PATH", "CARGO_HOME"]
    }
  }
}
```

When sandbox mode is enabled, bash commands run with restricted filesystem access — only specified paths are writable, and network access can be limited. This is enforced at the OS level (Linux namespaces or macOS sandbox-exec) when available, falling back to path-based validation.

---

### 3.9 HTTP Server ✅

The server exposes a REST + SSE API so any client can drive ragent.

| Aspect | Detail |
|--------|--------|
| Framework | `axum` |
| Transport | HTTP/1.1 over TCP or Unix socket |
| Auth | Optional HTTP Basic Auth |
| Streaming | Server-Sent Events (SSE) for LLM output |
| Spec | OpenAPI 3.1 auto-generated |

#### Route Map

| Method | Path | Description |
|--------|------|-------------|
| `GET` | `/health` | Health check |
| `GET` | `/config` | Get resolved config |
| `PUT` | `/config` | Update config |
| `GET` | `/providers` | List providers and models |
| `PUT` | `/auth/:provider` | Set API key |
| `GET` | `/sessions` | List sessions |
| `POST` | `/sessions` | Create session |
| `GET` | `/sessions/:id` | Get session details |
| `DELETE` | `/sessions/:id` | Archive session |
| `GET` | `/sessions/:id/messages` | Get message history |
| `POST` | `/sessions/:id/messages` | Send user message (SSE response) |
| `POST` | `/sessions/:id/abort` | Abort running agent loop (archives session, publishes `SessionAborted` event) |
| `POST` | `/sessions/:id/permission/:req_id` | Reply to permission request |
| `GET` | `/sessions/:id/tasks` | List background tasks for session |
| `POST` | `/sessions/:id/tasks` | Spawn a background or sync sub-agent task |
| `GET` | `/sessions/:id/tasks/:tid` | Get task details and result |
| `DELETE` | `/sessions/:id/tasks/:tid` | Cancel a running background task |
| `GET` | `/mcp` | List MCP servers |
| `POST` | `/mcp/:id/restart` | Restart MCP server |
| `GET` | `/events` | Global SSE event stream |

#### Task Management (F13 / F14)

**Spawn Task Request**
```json
POST /sessions/{id}/tasks
Content-Type: application/json

{
  "agent": "explore",        // Agent to use
  "task": "Analyze auth module",  // Task prompt
  "background": true,        // Optional: false = sync (default), true = background
  "model": "anthropic/claude-haiku"  // Optional: override model
}
```

**Task Response**
```json
{
  "id": "550e8400-e29b-41d4-a716-446655440000",
  "parent_session_id": "abc123",
  "agent_name": "explore",
  "task_prompt": "Analyze auth module",
  "status": "running",       // running, completed, failed, or cancelled
  "result": null,            // Set on completion
  "error": null,             // Set on failure
  "created_at": "2025-03-17T00:40:05Z",
  "completed_at": null,
  "background": true
}
```

**List Tasks Response**
```json
GET /sessions/{id}/tasks
[
  { /* TaskResponse */ },
  { /* TaskResponse */ },
  ...
]
```

#### SSE Event Types

```
event: message.start
data: {"session_id":"...","message_id":"..."}

event: text.delta
data: {"text":"Hello, "}

event: reasoning.delta
data: {"text":"Let me think..."}

event: tool.start
data: {"call_id":"...","tool":"read","input":{...}}

event: tool.end
data: {"call_id":"...","output":"...","duration_ms":42}

event: permission.requested
data: {"id":"...","permission":"edit","paths":["src/main.rs"]}

event: session.aborted
data: {"session_id":"...","reason":"user_requested"}

event: usage
data: {"input_tokens":1234,"output_tokens":567}

event: message.end
data: {"finish_reason":"stop"}

event: error
data: {"message":"Rate limit exceeded","code":"rate_limit"}

event: subagent_start
data: {"session_id":"...","task_id":"...","child_session_id":"...","agent":"explore","task":"...","background":true}

event: subagent_complete
data: {"session_id":"...","task_id":"...","child_session_id":"...","summary":"...","success":true,"duration_ms":1234}

event: subagent_cancelled
data: {"session_id":"...","task_id":"..."}
```

---

### 3.10 Terminal UI (TUI) ✅

| Aspect | Detail | Status |
|--------|--------|--------|
| Crate | `ratatui` + `crossterm` | ✅ Implemented |
| Layout | Home screen on launch, transitions to chat on first message | ✅ Implemented |
| Event loop | Terminal and agent event handling via `tokio::select!` | ✅ Implemented |
| Terminal state | Raw mode, alternate screen, mouse capture with cleanup | ✅ Implemented |

#### Home Screen

On startup ragent displays a centered landing page with the following elements:
- **ASCII art logo** — Centered ragent branding
- **Input field** — Auto-expanding multi-line text input (grows as user types, text wraps within borders)
- **Status line** — Shows provider health, current model, and helpful tips
- **Tips rotator** — Displays helpful command suggestions

The home screen features auto-expanding input that resizes vertically as the user types text.

```
                                               
     ██████╗  █████╗  ██████╗ ███████╗███╗   ██╗████████╗
     ██╔══██╗██╔══██╗██╔════╝ ██╔════╝████╗  ██║╚══██╔══╝
     ██████╔╝███████║██║  ███╗█████╗  ██╔██╗ ██║   ██║   
     ██╔══██╗██╔══██║██║   ██║██╔══╝  ██║╚═██║   ██║   
     ██║  ██║██║  ██║╚██████╔╝███████╗██║ ╚████║   ██║   
     ╚═╝  ╚═╝╚═╝  ╚═╝ ╚═════╝ ╚══════╝╚═╝  ╚═══╝   ╚═╝   

        ┌─ Ask anything… ─────────────────────┐
        │ >                                    │
        └──────────────────────────────────────┘

        ● Anthropic (Claude) (env)  model: claude-sonnet-4  — use /provider to change
        ● Tip  Use /help to see available commands
        
 /home/user/project                        v0.1.0
```

If no provider is configured, the status line reads:

```
        ⚠ No provider configured — use /provider to set up
```

#### Provider Setup Dialog

The `/provider` slash command opens a modal dialog:

1. **Select Provider** — arrow keys to navigate, Enter to select:
   - Anthropic (Claude)
   - OpenAI (GPT)
   - GitHub Copilot
   - Ollama (Local)

2. **Enter API Key** — paste or type the API key (shown partially masked).
   Copilot auto-discovers from IDE config if possible.
   Ollama requires no key.

3. **Select Model** — arrow keys to browse the provider's available models,
   Enter to confirm. The list is populated from the provider's default model
   catalogue (e.g. Claude Sonnet 4 and Claude 3.5 Haiku for Anthropic).

4. **Confirmation** — success message showing the selected provider and model;
   press any key to return.

Keys are stored persistently in `~/.local/share/ragent/ragent.db` (provider_auth table)
and are used as a fallback when environment variables are not set.

#### Provider Health Indicator

Both the home screen and the chat status bar display a health indicator before the
provider/model label:

| Symbol | Colour | Meaning |
|--------|--------|---------|
| `●` | Green | Provider is reachable and responding |
| `✗` | Red | Provider is unreachable (e.g. Ollama server not running, network error) |
| `●` | Yellow | Health check in progress |

A background health check runs on startup and again after each provider setup.
For Ollama, the check queries `/api/tags`; for Copilot it verifies the token
against the models API; for API-key providers (Anthropic, OpenAI) the key
presence is treated as sufficient.

#### API Key Resolution Order

When the agent needs a provider API key, it checks in order:

1. Environment variable (e.g. `ANTHROPIC_API_KEY`)
2. Provider-specific auto-discovery (Copilot IDE config)
3. Database (`provider_auth` table, stored via `ragent auth` or the TUI dialog)

#### Chat Layout

The chat screen displays messages with real-time streaming of text, reasoning, and tool calls:

```
┌───────────────────────────────────────────────────────────────┐
│ ● ragent  session: abc123  agent: general  [ready]  ● Anthropic / claude-sonnet-4 │
├───────────────────────────────────────────────────────────────┤
│                                                 │
│  User: Build me a REST API for managing tasks   │  ← Message
│                                                 │     history
│  Assistant: I'll create a task management API.  │     (scrollable)
│  Let me start by setting up the project...      │
│                                                 │
│  ● Bash $ cargo init --name task-api            │  ← Tool call
│    └ 3 lines...                                 │     (with result)
│                                                 │
│  ● Write src/main.rs                            │  ← File write
│    └ 45 lines written to src/main.rs            │     (with path)
│                                                 │
├─────────────────────────────────────────────────┤
│ ┌─ Permission ─────────────────────────────┐    │  ← Permission
│ │ Allow editing Cargo.toml?                │    │     dialog
│ │ [y] once  [a] always  [n] deny           │    │     (modal)
│ └──────────────────────────────────────────┘    │
├─────────────────────────────────────────────────┤
│ > type your message...                     Tab ▸│  ← Input area
│                                        tokens:$ │     (multi-line, auto-expanding)
└─────────────────────────────────────────────────┘
```

**Layout components:**
- **Status bar** — Displays session ID, active agent, readiness indicator, provider health symbol (●/✗), and current model
- **Message pane** — Scrollable area showing full conversation history with messages and tool calls
- **Tool visualization** — Shows status indicator (● for done, 🔄 for running, ✗ for error), tool name, input/output summary
- **Permission dialog** — Modal overlay prompting user approval for sensitive operations
- **Input area** — Multi-line expandable text box with token counter
- **Optional log panel** — Toggle-able right-side panel (30% width) showing detailed logs

#### Tool Call Display

Tool calls in the message window use a compact, readable format:

| Element | Format | Example |
|---------|--------|---------|
| Indicator | `●` (green=done, red=error, grey=running) | `●` |
| Tool name | Capitalized | `Read`, `Write`, `Bash`, `Grep` |
| Input summary | Tool-specific, paths relative to project root | `SPEC.md`, `$ cargo build` |
| Result line | `└` prefix with count | `└ 1593 lines read` |

Tool-specific input and result summaries:

| Tool | Input Summary | Result Summary |
|------|---------------|----------------|
| `read` | relative file path | `N lines read` |
| `write` | relative file path | `N lines written to <path>` |
| `edit` | relative file path | `N lines changed` |
| `bash` | `$ <first line of command>` | `N lines...` |
| `grep` | `"pattern" in <path>` | `N lines matched` |
| `glob` | glob pattern | `N files found` |
| `list` | relative directory path | `N entries` |

#### Log Panel

When enabled (via `--log` CLI flag or the `/log` slash command), a scrollable
log panel appears on the right side of the chat area (30% width). It captures:

- **Prompts sent** — user messages submitted to the LLM
- **Tool calls** — tool invocations with call IDs, and their results with timing
- **Session events** — session creation, message start/end, agent switches
- **Token usage** — per-request and cumulative input/output token counts
- **Permissions** — requested and granted/denied permission events
- **Errors** — agent and tool errors

Each entry includes a UTC timestamp and a colour-coded level:

| Level | Label | Colour | Description |
|-------|-------|--------|-------------|
| Info  | `INF` | Blue   | General events (prompts, sessions, tokens) |
| Tool  | `TUL` | Cyan   | Tool call start and end |
| Warn  | `WRN` | Yellow | Permission requests |
| Error | `ERR` | Red    | Agent and tool errors |

The panel auto-scrolls to show the most recent entries. Toggle visibility
at runtime with `/log`.

#### Key Bindings

| Key | Action |
|-----|--------|
| `Enter` | Send message (Shift+Enter for newline) |
| `Tab` / `Shift+Tab` | Cycle between agents (general → build → plan → explore) |
| `Ctrl+C` | Abort current agent run / exit |
| `Ctrl+L` | Clear screen |
| `Esc` | Cancel current input / close dialog |
| `Up/Down` | Scroll input history |
| `PageUp/PageDown` | Scroll message pane |
| `Ctrl+PageUp/PageDown` | Scroll log panel |
| `@` | Invoke sub-agent (e.g. `@general`, `@explore`) |
| `/` | Slash commands — shows autocomplete dropdown |
| `y/a/n` | Permission dialog responses |

#### Mouse Support

The TUI supports mouse interaction through `crossterm` mouse capture:

| Interaction | Behaviour |
|-------------|-----------|
| **Scroll wheel** | Scrolls the message pane or log panel (whichever the cursor is over) |
| **Scrollbar drag** | Click-and-drag the scrollbar track on either the messages or log pane to scrub through content |
| **Text selection** | Click-and-drag to select text in any pane (messages, log, input, home input). Selected text is highlighted with a light-blue background |
| **Right-click** | Copies the current text selection to the system clipboard |

Mouse capture is disabled before leaving raw mode on exit to prevent escape sequences from leaking into the shell.

#### Scrollbars

When content overflows the visible area, vertical scrollbar widgets appear on the right edge of the messages pane and log panel. Scrollbars use `ratatui::widgets::Scrollbar` with `ScrollbarState` to reflect the current scroll position. Scrollbar tracks are draggable via mouse.

#### Implementation Status

The TUI is **fully functional** with the following architecture:

**Core Components (see `crates/ragent-tui/src/`):**
- `lib.rs` — Main entry point: `run_tui()` function manages terminal lifecycle (raw mode, alternate screen, mouse capture)
- `app.rs` — Application state and event handling (85 KB, comprehensive)
  - `ScreenMode` enum: Home vs Chat screens
  - `ProviderSetupStep` enum: Interactive provider configuration flow
  - Event handlers for terminal keys and mouse
  - Session loading, message rendering, permission dialog state
- `layout.rs` — Screen rendering and layout management (ratatui frame drawing)
- `input.rs` — Input widget with auto-expansion and history management (25 KB)
- `widgets/message_widget.rs` — Message rendering with tool calls and streaming text
- `widgets/permission_dialog.rs` — Permission request modal
- `logo.rs`, `tips.rs` — Home screen ASCII art and tip rotation

**Event Loop:**
The TUI runs a `tokio::select!` loop that:
1. Polls terminal events (keyboard, mouse) at 50ms intervals
2. Listens for agent events via the event bus (broadcast channel)
3. Renders frame on each iteration
4. Delegates events to appropriate handlers in `App`

**Screen Transitions:**
- Starts in `ScreenMode::Home` with centered logo and input
- Transitions to `ScreenMode::Chat` when first message is sent
- Agent tab switching changes the active agent without screen change

**Key Features Implemented:**
- Session resume from stored state
- Provider setup flow with device code support (Copilot)
- Health check indicator (● green, ✗ red, updating)
- Scrollbars with mouse drag support
- Text selection and clipboard copy
- Mouse scroll in message and log panes
- Auto-expanding input with text wrapping
- Slash command autocomplete dropdown
- Message enqueueing (queue messages while agent responds)
- Log panel toggle and timestamped entries
- Permission dialogs with inline feedback

Typing `/` in the input area on either the home screen or the chat screen
opens an autocomplete dropdown above the input. The list filters as you type,
and you can navigate with `↑`/`↓` arrow keys and select with `Enter`.
Press `Esc` to dismiss the menu.

| Command | Description | Implemented |
|---------|-------------|-------------|
| `/about` | Show application info, version, and authors | ✅ |
| `/agent [name]` | Switch the active agent — opens selection dialog if no name given, or switches directly to the named agent | ✅ |
| `/checkpoint [diff\|restore]` | View checkpoint diff or restore workspace to a previous checkpoint | 🔲 |
| `/clear` | Clear message history for the current session | ✅ |
| `/compact` | Summarise and compact the conversation history via the compaction agent | ✅ |
| `/context` | Show detailed token usage breakdown (input, output, cached, total, limit, percentage used) | 🔲 |
| `/help` | Show available slash commands with descriptions | ✅ |
| `/log` | Toggle the log panel on/off | ✅ |
| `/model` | Switch the active model on the current provider | ✅ |
| `/provider` | Change the LLM provider (re-enters full setup flow) | ✅ |
| `/provider_reset` | Reset a provider — prompts for selection, clears stored credentials and disables auto-detection | ✅ |
| `/quit` | Exit ragent | ✅ |
| `/system [prompt]` | Override the agent system prompt; show current prompt if no argument given | ✅ |
| `/todo` | Display the current task todo list with status indicators | ✅ |
| `/tools` | List all available tools (built-in and MCP) with parameters and descriptions | ✅ |

#### Automatic Context Compaction

When the conversation approaches **95% of the model's context window**, ragent automatically compresses the conversation history in the background:

1. Token usage is tracked per-request via provider token count responses
2. When cumulative tokens exceed 95% of the model's context limit, auto-compaction triggers
3. The conversation history is summarised by the LLM and replaced with the summary
4. The user is notified in the status bar but their workflow is not interrupted
5. Manual compaction is available via `/compact` at any time; press `Esc` to cancel

This enables virtually infinite sessions without manual context management.

#### Message Enqueueing

Users can send follow-up messages while the agent is still processing a response. Queued messages are delivered after the current response completes, allowing natural steering of the conversation without waiting for each turn to finish.

#### Inline Rejection Feedback

When a user denies a permission prompt, they can provide inline feedback explaining why. The feedback is injected into the conversation so the agent can adapt its approach without stopping entirely.

---

### 3.11 MCP Client ✅

ragent acts as an MCP (Model Context Protocol) **client**, connecting to external MCP servers that provide additional tools, resources, and prompts. The implementation uses the official [`rmcp`](https://crates.io/crates/rmcp) Rust SDK for transport, handshake, tool discovery, and tool invocation.

#### MCP Server Configuration

```jsonc
{
  "mcp": {
    "github": {
      "type": "stdio",          // "stdio" | "sse" | "http"
      "command": "npx",
      "args": ["-y", "@modelcontextprotocol/server-github"],
      "env": {
        "GITHUB_TOKEN": "${env.GITHUB_TOKEN}"
      }
    },
    "database": {
      "type": "sse",
      "url": "http://localhost:3001/sse"
    },
    "remote-api": {
      "type": "http",
      "url": "https://api.example.com/mcp"
    }
  }
}
```

#### MCP Server Lifecycle

```rust
pub enum McpStatus {
    Connected,
    Disabled,
    Failed { error: String },
    NeedsAuth,
}

pub struct McpServer {
    pub id: String,
    pub config: McpServerConfig,
    pub status: McpStatus,
    pub tools: Vec<McpToolDef>,
}
```

1. **Start** — spawn stdio child process (via `tokio::process::Command` with `ConfigureCommandExt`) or connect to an SSE/HTTP endpoint
2. **Initialize** — perform the MCP `initialize` handshake via `rmcp::ServiceExt`
3. **List Tools** — discover tools advertised by the server; supports on-demand refresh via `list_tools(force_refresh: bool)`. Tool definitions include name, description, and JSON Schema parameters
4. **Execute** — proxy tool calls from the agent to the correct MCP server via `call_tool`. Calls are auto-routed to the server that advertises the requested tool name. A configurable timeout (default 120 seconds) prevents runaway calls
5. **Reconnect** — automatic retry on transient failures
6. **Shutdown** — graceful disconnect on ragent exit

MCP connections are stored in an `Arc<RwLock<HashMap<String, McpConnection>>>` keyed by server ID, allowing concurrent access from the agent loop and HTTP endpoints.

MCP-provided tools are subject to the same permission rules as built-in tools.

---

### 3.12 LSP Integration ❌

ragent can optionally spawn and communicate with Language Server Protocol servers to provide code intelligence to the agent.

#### Supported Language Servers

| Language | Server | Detection |
|----------|--------|-----------|
| Rust | `rust-analyzer` | `Cargo.toml` |
| TypeScript / JavaScript | `typescript-language-server` | `package.json`, `tsconfig.json` |
| Python | `pylsp` or `pyright` | `pyproject.toml`, `setup.py` |
| Go | `gopls` | `go.mod` |
| C/C++ | `clangd` | `compile_commands.json`, `CMakeLists.txt` |

#### LSP Capabilities Used

| Capability | Use Case |
|------------|----------|
| `textDocument/diagnostics` | Feed compiler errors/warnings to the agent |
| `textDocument/definition` | Navigate to symbol definitions |
| `textDocument/references` | Find all references to a symbol |
| `textDocument/hover` | Get type information |
| `textDocument/completion` | (Future) code completion suggestions |

The agent can invoke LSP queries through a built-in `lsp` tool or ragent can automatically include diagnostics in the prompt context when the agent edits a file.

---

### 3.13 Event Bus ✅

The event bus is the central nervous system connecting the server, agent loop, TUI, and permission system.

```rust
pub enum Event {
    // Session events
    SessionCreated { session: Session },
    SessionUpdated { session: Session },
    SessionAborted { session_id: String, reason: String },

    // Message events
    MessageStart { session_id: String, message_id: String },
    TextDelta { session_id: String, text: String },
    ReasoningDelta { session_id: String, text: String },
    ToolCallStart { session_id: String, call_id: String, tool: String },
    ToolCallEnd { session_id: String, call_id: String, output: String },
    MessageEnd { session_id: String, message_id: String, finish_reason: FinishReason },

    // Permission events
    PermissionRequested { request: PermissionRequest },
    PermissionReplied { request_id: String, decision: PermissionDecision },

    // Agent events
    AgentSwitched { from: String, to: String },
    AgentError { session_id: String, error: String },

    // MCP events
    McpStatusChanged { server_id: String, status: McpStatus },

    // Usage events
    TokenUsage { session_id: String, input_tokens: u64, output_tokens: u64, cost_usd: f64 },
}
```

Implementation: `tokio::sync::broadcast` channel with configurable buffer size. Multiple consumers (TUI, SSE endpoint, logger) can subscribe independently.

---

### 3.14 Storage & Database ✅

| Aspect | Detail |
|--------|--------|
| Engine | SQLite via `rusqlite` (bundled) |
| Location | `$XDG_DATA_HOME/ragent/ragent.db` (or `~/.local/share/ragent/ragent.db`) |
| Migrations | Embedded SQL, run at startup |

#### Schema

```sql
CREATE TABLE sessions (
    id          TEXT PRIMARY KEY,
    title       TEXT NOT NULL DEFAULT 'New Session',
    project_id  TEXT NOT NULL,
    directory   TEXT NOT NULL,
    parent_id   TEXT REFERENCES sessions(id),
    version     TEXT NOT NULL,
    summary     TEXT,  -- JSON blob
    created_at  TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at  TEXT NOT NULL DEFAULT (datetime('now')),
    archived_at TEXT
);

CREATE TABLE messages (
    id          TEXT PRIMARY KEY,
    session_id  TEXT NOT NULL REFERENCES sessions(id),
    role        TEXT NOT NULL CHECK (role IN ('user', 'assistant')),
    parts       TEXT NOT NULL,  -- JSON array of MessagePart
    created_at  TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at  TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE INDEX idx_messages_session ON messages(session_id, created_at);

CREATE TABLE provider_auth (
    provider_id TEXT PRIMARY KEY,
    api_key     TEXT NOT NULL,        -- Encrypted at rest
    base_url    TEXT,
    updated_at  TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE TABLE mcp_servers (
    id          TEXT PRIMARY KEY,
    config      TEXT NOT NULL,         -- JSON blob
    status      TEXT NOT NULL DEFAULT 'disabled',
    updated_at  TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE TABLE snapshots (
    id          TEXT PRIMARY KEY,
    session_id  TEXT NOT NULL REFERENCES sessions(id),
    message_id  TEXT NOT NULL REFERENCES messages(id),
    data        BLOB NOT NULL,         -- Compressed tarball of changed files
    created_at  TEXT NOT NULL DEFAULT (datetime('now'))
);
```

---

### 3.15 Shell Execution ✅

The `bash` tool executes commands in a sandboxed environment.

#### Execution Model

```rust
pub struct BashTool;

impl Tool for BashTool {
    async fn execute(&self, input: Value, ctx: &ToolContext) -> Result<ToolOutput> {
        let command: String = // extract from input
        let timeout: Duration = // extract or default (120s)

        let output = tokio::process::Command::new("sh")
            .arg("-c")
            .arg(&command)
            .current_dir(&ctx.working_dir)
            .env("RAGENT", "1")
            .env("RAGENT_SESSION_ID", &ctx.session_id)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .kill_on_drop(true)
            .spawn()?
            .wait_with_output()
            .timeout(timeout)
            .await??;

        // Combine stdout + stderr, truncate if too long
        Ok(ToolOutput { content, metadata })
    }
}
```

#### Safety Features

- Commands execute with `kill_on_drop(true)` — orphan processes are cleaned up
- Configurable timeout (default 120 seconds)
- Output truncation to prevent context window overflow
- Working directory locked to project root (unless `external_directory` permission granted)
- Environment variables sanitized (secrets not forwarded)
- Permission system gates execution (default: `Ask` for all bash commands)

---

### 3.16 Snapshot & Undo ✅

Before executing edit/write/patch tools, ragent captures a snapshot of affected files so changes can be reverted.

#### Snapshot Flow

1. Agent requests `edit` on `src/main.rs`
2. Before executing, capture current contents of `src/main.rs`
3. Store snapshot in `snapshots` table (compressed)
4. Execute the edit
5. If user requests undo → restore from snapshot
6. Snapshots are associated with the message that triggered them

#### Shadow Git Checkpoints

In addition to per-file snapshots, ragent maintains a **shadow git repository** for full workspace versioning:

1. On session start, initialise a hidden shadow repo (`.ragent/.shadow-git/`) separate from the project's own git
2. Before any file modification, commit the current state as a checkpoint
3. Checkpoints capture: file content changes, new files, deleted files, renames
4. Users can compare the current workspace against any checkpoint (`/checkpoint diff`)
5. Users can restore to any checkpoint:
   - **Files only**: Revert workspace files but keep conversation history
   - **Files & conversation**: Revert both workspace and conversation to the checkpoint's point in time

Checkpoint exclusions:
- Files matching `.gitignore` and `.ragentignore` patterns
- Build artifacts, binary files, and dependencies (auto-detected)
- Files larger than 1 MB

The shadow repository is independent from the project's existing git — no GitHub account or git configuration is required.

#### Undo Granularity

| Level | Description |
|-------|-------------|
| Per-tool-call | Revert a single tool call's changes |
| Per-message | Revert all changes from one assistant message |
| Per-session | Revert all changes from the entire session |
| Per-checkpoint | Restore workspace to a specific checkpoint state |

---

### 3.17 Hooks ❌

Hooks allow users to execute custom commands at key points during agent execution, enabling validation, logging, security scanning, or workflow automation. Hooks can be shell commands, HTTP requests, prompt injections, or agent invocations.

#### Hook Events

| Hook | Trigger Point | Use Cases |
|------|---------------|-----------|
| `PreToolUse` | Before any tool executes | Validation, logging, security scanning |
| `PostToolUse` | After a tool completes successfully | Audit logging, cleanup, notification |
| `PostToolUseFailure` | After a tool fails | Error reporting, fallback logic |
| `PreMessage` | Before sending a message to the LLM | Prompt injection detection, content filtering |
| `PostMessage` | After receiving an LLM response | Response validation, metrics collection |
| `UserPromptSubmit` | When the user submits a prompt | Input validation, prompt rewriting |
| `SessionStart` | When a new session begins | Environment setup, dependency checks |
| `SessionEnd` | When a session completes | Cleanup, summary generation |
| `PreCompact` | Before context compaction | Save important context, notify user |
| `PermissionRequest` | When a permission prompt appears | Auto-approve policies, logging |
| `SubagentStart` | When a subagent is spawned | Resource tracking, scope validation |
| `SubagentStop` | When a subagent completes | Result aggregation, cleanup |
| `Notification` | When a notification-worthy event occurs | Desktop alerts, sound, external integrations |
| `WorktreeCreate` | When a git worktree is created | Non-git VCS setup, custom isolation |
| `WorktreeRemove` | When a git worktree is removed | Cleanup, branch deletion |
| `InstructionsLoaded` | After AGENTS.md / rules are loaded | Validation, transformation |
| `TaskCompleted` | When a background task finishes | Notifications, chaining |

#### Hook Types

Hooks support multiple execution backends:

| Type | Description | Example |
|------|-------------|---------|
| `command` | Shell command execution (default) | `"command": "./scripts/validate.sh"` |
| `http` | HTTP request to a URL | `"type": "http", "url": "https://hooks.example.com/audit"` |
| `prompt` | Inject text into the agent's prompt | `"type": "prompt", "content": "Remember to check tests"` |
| `agent` | Spawn a subagent to handle the event | `"type": "agent", "agent": "security-reviewer"` |

#### Configuration

Hooks are defined in `.ragent/hooks/` or in the project config:

```jsonc
{
  "hooks": {
    "PreToolUse": [
      {
        "type": "command",
        "command": "./scripts/validate-tool.sh",
        "timeout": 10,
        "matcher": { "tool_name": ["bash", "write", "edit"] }
      }
    ],
    "PostToolUse": [
      {
        "type": "command",
        "command": "./scripts/audit-log.sh",
        "timeout": 5
      }
    ],
    "Notification": [
      {
        "type": "command",
        "command": "notify-send 'ragent' '$HOOK_MESSAGE'",
        "matcher": { "type": ["permission_prompt", "idle_prompt", "task_completed"] }
      }
    ],
    "SessionStart": [
      {
        "type": "http",
        "url": "https://metrics.example.com/session-start",
        "method": "POST",
        "async": true
      }
    ]
  }
}
```

#### Hook Input / Output

Hook commands receive context via environment variables:

| Variable | Description |
|----------|-------------|
| `HOOK_EVENT` | The hook event name (e.g., `PreToolUse`) |
| `HOOK_TOOL_NAME` | Tool being invoked (for tool hooks) |
| `HOOK_TOOL_INPUT` | JSON-encoded tool input arguments |
| `HOOK_TOOL_OUTPUT` | JSON-encoded tool output (post hooks) |
| `HOOK_SESSION_ID` | Current session ID |
| `HOOK_MESSAGE` | Human-readable event description |

A non-zero exit code from a `Pre*` hook aborts the operation. Hooks marked with `"async": true` run in the background without blocking the agent.

#### Hooks in Skills and Agents

Skills and custom agents can define scoped hooks in their frontmatter/config. These hooks only fire when that skill or agent is active:

```yaml
# In SKILL.md frontmatter
hooks:
  PostToolUse:
    - type: command
      command: "./scripts/post-deploy-check.sh"
      matcher: { tool_name: ["bash"] }
```

---

### 3.18 Custom Agents ⚠️

Users can define custom specialized agents beyond the built-in presets. Custom agents allow tailoring the agent's system prompt, available tools, and permissions for specific tasks or team roles.

#### Configuration

Custom agents are defined in `.ragent/agents/` as markdown files with YAML frontmatter, or in the project config as JSON:

**Markdown format** (`.ragent/agents/frontend-expert.md`):

```markdown
---
name: frontend-expert
description: Frontend specialist following team guidelines
model: anthropic/claude-sonnet-4-20250514
tools:
  - read
  - write
  - edit
  - bash
  - grep
  - glob
max_turns: 50
memory: project
skills:
  - code-review
  - testing
isolation: none
---

You are an expert frontend engineer. Follow React best practices,
use TypeScript, and ensure all components have proper tests.
```

**JSON format** (in `ragent.json`):

```jsonc
{
  "agents": {
    "frontend-expert": {
      "description": "Frontend specialist following team guidelines",
      "prompt": "You are an expert frontend engineer. Follow React best practices...",
      "tools": ["read", "write", "edit", "bash", "grep", "glob", "list"],
      "permissions": {
        "file:write": { "glob": "src/components/**", "rule": "Allow" }
      },
      "max_turns": 50,
      "memory": "project",
      "skills": ["code-review", "testing"],
      "isolation": "none"
    }
  }
}
```

#### Advanced Agent Options

| Field | Type | Description |
|-------|------|-------------|
| `name` | string | Agent identifier (kebab-case) |
| `description` | string | Human-readable description (shown in agent picker) |
| `model` | string | Override model for this agent (`provider/model`) |
| `tools` | string[] | Allowed tool names or groups |
| `permissions` | object | Permission rules scoped to this agent |
| `max_turns` | number | Maximum tool-call iterations before stopping |
| `memory` | string | Memory scope: `"user"` (global), `"project"` (per-project), `"local"` (agent-only) |
| `skills` | string[] | Skills to preload into this agent's context |
| `isolation` | string | Execution isolation: `"none"` (default), `"worktree"` (git worktree), `"container"` |
| `hooks` | object | Hooks scoped to this agent's lifecycle |
| `permission_mode` | string | Default permission mode when this agent is active |

#### Background Execution

Custom agents can be spawned in the background for parallel execution:

```
/agent frontend-expert --background "Refactor the login component"
```

Background agents run in an isolated context and return results when complete. The parent session is notified via the `TaskCompleted` hook event.

#### Built-in Subagents

| Name | Purpose | Tools |
|------|---------|-------|
| `explore` | Fast codebase search and analysis | Read-only: grep, glob, list, read |
| `plan` | Read-only analysis and planning | Denies write tools; allows read + bash |
| `general-purpose` | Full-capability coding agent | All tools |

Custom agents appear in the agent picker (`/agent`) and can be selected via `Tab`/`Shift+Tab` cycling. The CLI automatically delegates common tasks to specialized agents when appropriate.

---

### 3.19 Skills ✅

Skills enhance the agent's ability to perform specialized tasks by bundling instructions, scripts, and resources into reusable packages. Skills follow a markdown-first format with YAML frontmatter for configuration and are fully implemented. They support both user-initiated invocation (via slash commands) and agent-initiated auto-invocation (via LLM reasoning).

#### Skill System Overview

The skill system (`ragent-core/src/skill/`) provides:
- **Discovery**: Automatic loading from project, personal, and extra directories
- **Registry**: Centralized management with scope-based priority (Bundled < Personal < Project)
- **Invocation**: User-triggered (`/name`) and agent-initiated (auto-invocation)
- **Forking**: Isolated subagent execution context for complex tasks
- **Argument substitution**: Dynamic `$ARGUMENTS` replacement and environment variable expansion

#### Skill Structure

```
.ragent/skills/
  deploy/
    SKILL.md            # Skill instructions and frontmatter (required)
    scripts/            # Helper scripts the skill can invoke
    templates/          # Template files for the agent to fill in
    examples/           # Example outputs showing expected format
    resources/          # Reference materials
```

#### Skill Definition (SKILL.md)

Skills use markdown with YAML frontmatter:

```markdown
---
name: deploy
description: Deploy the application to production
disable-model-invocation: false
user-invocable: true
allowed-tools:
  - bash
  - read
  - write
model: "anthropic/claude-sonnet-4-20250514"
context: fork
agent: general-purpose
argument-hint: "[environment]"
---

Deploy $ARGUMENTS to production:

1. Run the test suite
2. Build the release binary
3. Push to the deployment target
4. Verify the deployment succeeded
```

#### Frontmatter Reference

| Field | Type | Required | Default | Description |
|-------|------|----------|---------|-------------|
| `name` | string | No | Directory name | Unique skill identifier (lowercase, hyphens, max 64 chars) |
| `description` | string | No | None | What the skill does; used for auto-invocation matching |
| `argument-hint` | string | No | None | Hint shown during autocomplete (e.g., `[environment]`, `[issue-number]`) |
| `disable-model-invocation` | bool | No | `false` | If `true`, only user can invoke via `/name`; agent cannot auto-invoke |
| `user-invocable` | bool | No | `true` | If `false`, hidden from `/` menu; only agent can invoke |
| `allowed-tools` | string[] | No | `[]` | Tools the agent can use without permission when skill is active (single string or list) |
| `model` | string | No | None | Override model when this skill is active (e.g., `"anthropic/claude-sonnet-4-20250514"`) |
| `context` | enum | No | None | Set to `fork` to run in isolated subagent context |
| `agent` | string | No | None | Subagent type when `context: fork` (e.g., `"explore"`, `"plan"`, `"general-purpose"`) |
| `hooks` | object | No | None | Hooks scoped to this skill's lifecycle (raw YAML, stored as JSON) |

#### Skill Name Validation

Skill names must:
- Contain only lowercase ASCII letters, digits, and hyphens
- Be between 1 and 64 characters
- Match the directory name unless overridden in frontmatter

Example: `my-deploy-skill` ✅, `MySkill` ❌, `my_skill` ❌

#### Argument Substitution

Skills support dynamic argument substitution via environment variables and placeholders:

| Variable | Description | Example |
|----------|-------------|---------|
| `$ARGUMENTS` | All arguments passed when invoking the skill | `/deploy staging prod` → `staging prod` |
| `$ARGUMENTS[N]` | Specific argument by 0-based index | `$ARGUMENTS[0]` → `staging` |
| `$N` | Shorthand for `$ARGUMENTS[N]` | `$0` → `staging` |
| `${RAGENT_SESSION_ID}` | Current session ID (UUID/ULID) | Useful for logging, tracking |
| `${RAGENT_SKILL_DIR}` | Directory containing the skill's SKILL.md | Access skill-local scripts/templates |
| `${env.VAR_NAME}` | Environment variable expansion | `${env.HOME}`, `${env.GITHUB_TOKEN}` |

Example skill using arguments:

```markdown
---
name: deploy
description: Deploy to an environment
argument-hint: "[environment]"
---

Deploying to $0:

1. Load credentials from ~/.config/deploy/$0.yaml
2. Run deployment to $ARGUMENTS environment
3. Verify at $ARGUMENTS-api.example.com
4. Log results to /tmp/$RAGENT_SESSION_ID-deploy.log
```

Invocation: `/deploy staging` → `$0` becomes `staging`, `$ARGUMENTS` becomes `staging`.

#### Dynamic Context Injection

The `` !`command` `` syntax executes shell commands before the skill content is sent to the agent. Command output replaces the placeholder:

```markdown
---
name: pr-summary
description: Summarize changes in a pull request
context: fork
agent: explore
---

## Pull request context
- PR diff: !`gh pr diff`
- Changed files: !`gh pr diff --name-only`
- PR title: !`gh pr view --json title -q .title`

## Your task
Summarize this pull request in 2-3 sentences, highlighting key changes.
```

When invoked, ragent:
1. Executes each `` !`command` `` in the shell
2. Replaces the placeholder with the command's stdout
3. Sends the expanded content to the agent

Errors in dynamic commands are logged as warnings; the placeholder is replaced with an error message.

#### Skill Scopes & Priority

Skills are discovered from multiple sources with the following priority (lowest → highest):

| Scope | Path | Priority | Applies To | Note |
|-------|------|----------|------------|------|
| Bundled | Compiled into ragent | 0 (lowest) | All installations | `/simplify`, `/batch`, `/debug`, `/loop` |
| Enterprise | Managed settings | 1 | All users in organization | Future feature |
| Personal | `~/.ragent/skills/<name>/SKILL.md` | 2 | All projects for this user | Persistent across projects |
| Project | `.ragent/skills/<name>/SKILL.md` | 3 (highest) | This project only | Can override bundled/personal |

When a skill exists in multiple scopes with the same name, the **highest-priority scope wins** completely (lower scopes are ignored for that name).

Example:
- Bundled `simplify` skill exists
- User creates `~/.ragent/skills/simplify/SKILL.md` (Personal scope)
- Project creates `.ragent/skills/simplify/SKILL.md` (Project scope)
- → Project scope skill is used; bundled and personal are ignored

#### Monorepo Support

For monorepo projects, ragent automatically discovers skills in nested subdirectories:

```
project/
├── .ragent/skills/          # Root-level project skills
│   └── shared-deploy/SKILL.md
├── packages/
│   ├── frontend/
│   │   └── .ragent/skills/  # Frontend-specific skills (discovered & loaded)
│   │       └── build-frontend/SKILL.md
│   └── backend/
│       └── .ragent/skills/  # Backend-specific skills (discovered & loaded)
│           └── start-server/SKILL.md
```

All discovered skills are registered into a single `SkillRegistry` at session start. The scope resolution ensures project-level skills (including nested ones) can override personal and bundled skills.

#### Subagent Execution (Context Forking)

Skills with `context: fork` run in an **isolated subagent context**:

1. **Isolation**: No access to parent conversation history; runs independently
2. **Agent Selection**: The `agent` field specifies the execution environment:
   - `"general-purpose"` — Full tool access, multi-step reasoning
   - `"explore"` — Read-only (grep, glob, list, read, bash)
   - `"plan"` — Analysis-focused (read-only, no edits)
   - Custom agent names are also supported
3. **Context Assembly**: Skill body (with substitutions) becomes the isolated agent's prompt
4. **Result Return**: Isolated agent completes, results are summarized and returned to parent conversation
5. **History Preservation**: Parent conversation history is unaffected; the skill invocation appears as a single tool call in parent history

**Example: Forked skill for PR analysis**

```markdown
---
name: pr-analyze
description: Analyze a pull request in an isolated context
context: fork
agent: explore
argument-hint: "[pr-number]"
disable-model-invocation: false
---

Analyze PR #$0 and provide:
- Summary of changes
- Files modified
- Potential risks
- Test coverage assessment
```

When invoked by the agent: `/pr-analyze 42`
1. A new subagent (explore) is spawned with isolated context
2. The expanded skill body is sent to the explore agent
3. Explore agent can only use read-only tools
4. Results are summarized and injected as tool output in the parent conversation
5. Parent agent continues with the summary as context

#### Bundled Skills

Ragent includes 4 bundled skills (lowest priority, overridable):

| Skill | Description | Context | Agent | Invocable |
|-------|-------------|---------|-------|-----------|
| `/simplify` | Reviews recently changed files for code quality, reuse, and efficiency issues | Normal | N/A | Both user & agent |
| `/batch <instruction>` | Orchestrates large-scale parallel changes across a codebase | Fork | N/A | Both user & agent |
| `/debug [description]` | Troubleshoots current session by reading debug logs and error messages | Normal | N/A | Both user & agent |
| `/loop [interval] <prompt>` | Runs a prompt repeatedly on an interval (scheduled background tasks) | Normal | N/A | User only (`disable-model-invocation: true`) |

**Implementation notes:**
- Bundled skills are registered first, then overlaid with discovered skills
- Project-scope skills can override bundled skills by using the same name
- Bundled skill implementations are in `crates/ragent-core/src/skill/bundled.rs`

#### Skill Invocation Methods

**User Invocation:**
```bash
/deploy staging              # Invoke "deploy" skill with argument "staging"
/pr-analyze 42               # Invoke "pr-analyze" skill with "42"
/batch "Add error handling"  # Invoke "batch" skill with instruction
```

**Agent Auto-Invocation:**
```
Agent: "Let me simplify the code for you..."
→ Internally calls `/simplify` without user trigger
```

Control this with `disable-model-invocation`:

| Setting | User Can Invoke | Agent Can Invoke |
|---------|-----------------|------------------|
| (default) | Yes | Yes |
| `disable-model-invocation: true` | Yes | No |
| `user-invocable: false` | No | Yes |

#### Skill Registry API

The `SkillRegistry` (in `ragent-core/src/skill/mod.rs`) is the central manager:

```rust
/// Load all discoverable skills from working directory
pub fn load(working_dir: &Path, extra_dirs: &[String]) -> SkillRegistry;

/// Register a single skill (highest scope wins)
pub fn register(&mut self, skill: SkillInfo);

/// Lookup by name
pub fn get(&self, name: &str) -> Option<&SkillInfo>;

/// List all user-invocable skills
pub fn list_user_invocable(&self) -> Vec<&SkillInfo>;

/// List all agent-invocable skills
pub fn list_agent_invocable(&self) -> Vec<&SkillInfo>;

/// All registered skills (sorted by name)
pub fn list_all(&self) -> Vec<&SkillInfo>;
```

#### Skill File Format Details

**YAML Frontmatter parsing** (`loader.rs`):
- Splits file on opening `---` at start of file
- Parses YAML between opening and closing `---` delimiters
- Everything after closing `---` is the markdown body
- Supports both single-string and list formats for `allowed-tools`
- Converts YAML hooks to JSON for storage

**Example multiformat field**:
```yaml
# Single string
allowed-tools: bash

# List format
allowed-tools:
  - bash
  - read
  - write

# Both are valid and produce Vec<String>
```

#### Configuration Integration

Skills can be configured via `ragent.json`:

```jsonc
{
  "skill_dirs": [
    "/path/to/shared/skills",  // Extra skill directories (Personal scope)
    "$HOME/my-skills"
  ]
}
```

This allows organizations to share skills across projects without per-project setup.

#### Error Handling

- **Missing SKILL.md**: Directory is skipped with a warning
- **Invalid YAML**: Skill is skipped; error logged (e.g., "Failed to parse frontmatter")
- **Invalid name**: Validation error if name contains invalid characters, exceeds 64 chars, or is empty
- **Malformed arguments**: Substitution errors are logged; original `$VARIABLE` is kept if expansion fails

All skill loading errors are logged but don't halt the session — other skills continue loading.

#### Skill Loader Implementation Details

The skill loader (`ragent-core/src/skill/loader.rs`) is responsible for discovering, parsing, and registering skills from the filesystem. It handles multiple scopes and provides comprehensive error recovery.

**Discovery Algorithm:**

1. **Personal Skills** (`~/.ragent/skills/`)
   - Reads user's home directory via `dirs::home_dir()`
   - Scans `~/.ragent/skills/` directory (if it exists)
   - Each subdirectory with a `SKILL.md` file becomes a skill
   - Scope: `SkillScope::Personal`
   - Load order: Alphabetically by directory name

2. **Extra Directories** (from config `skill_dirs`)
   - User-configured directories passed via `extra_dirs: &[String]`
   - Treated as `SkillScope::Personal` (overridable by project skills)
   - Useful for shared organization skills or team skill libraries
   - Warnings logged if directory doesn't exist

3. **Project Skills** (`.ragent/skills/`)
   - Scans `{working_dir}/.ragent/skills/` (if it exists)
   - Scope: `SkillScope::Project`
   - Load order: Alphabetically by directory name

4. **Nested/Monorepo Skills** (`.ragent/skills/` in subdirectories)
   - Automatically discovers nested `.ragent/skills/` directories
   - Scans first-level subdirectories of `working_dir` (e.g., `packages/*/`, `services/*/`)
   - Each discovered nested directory is registered as `SkillScope::Project`
   - Enables monorepo support without explicit configuration

**File Structure for Each Skill:**

```
.ragent/skills/<skill-name>/
├── SKILL.md                 # Required (≤100 KB recommended)
├── scripts/                 # Optional
│   ├── deploy.sh
│   ├── validate.sh
│   └── ...
├── templates/               # Optional
│   ├── config.toml
│   ├── docker-compose.yml
│   └── ...
├── examples/                # Optional
│   ├── successful-output.txt
│   ├── error-case.txt
│   └── ...
├── resources/               # Optional
│   ├── reference.md
│   ├── checklists.txt
│   └── ...
└── README.md                # Optional (documentation only)
```

**Disk Footprint Considerations:**

| Component | Typical Size | Notes |
|-----------|--------------|-------|
| SKILL.md (minimal) | 0.5–5 KB | Just frontmatter + simple instructions |
| SKILL.md (typical) | 5–50 KB | Frontmatter + detailed instructions + examples |
| SKILL.md (max) | 100+ KB | Large skills with extensive documentation |
| `scripts/` directory | 10–500 KB | Helper scripts; typically bash/python files |
| `templates/` directory | 5–100 KB | Config templates, manifests, etc. |
| `examples/` directory | 10–200 KB | Example outputs, reference files |
| Single skill total | 20–500 KB | Typical skill bundle |
| Personal `~/.ragent/skills/` | 1–10 MB | All personal skills combined |
| Project `.ragent/skills/` | 1–50 MB | Project-specific skills (variable) |
| Nested `packages/*/.ragent/skills/` | 0.5–20 MB per package | Monorepo sub-package skills |

**Memory Footprint at Runtime:**

- Per `SkillInfo` struct: ~2–4 KB (name, description, frontmatter, body text in memory)
- Registry with 100 skills: ~200–400 KB
- Registry with 500 skills: ~1–2 MB
- All skills loaded at session start and kept in memory for the session lifetime

**Parsing Details:**

The `parse_skill_md()` function performs the following steps:

1. **Frontmatter Extraction** (`split_frontmatter()`)
   - Looks for opening `---` at the start of file (ignoring leading whitespace)
   - Scans line-by-line for closing `---` delimiter that appears at the start of a line
   - Returns `(frontmatter_str, body_str)` tuple
   - Handles `\n` and `\r\n` line endings transparently

2. **YAML Parsing** (via `serde_yaml`)
   - Parses frontmatter using `serde_yaml::from_str()`
   - Deserializes into `SkillFrontmatter` struct with sensible defaults
   - Handles both single-string and list formats for `allowed-tools`:
     ```yaml
     # Single string form
     allowed-tools: bash
     
     # List form
     allowed-tools:
       - bash
       - read
       - write
     ```
   - Both forms deserialize to `Vec<String>` internally

3. **Hook Conversion** (YAML to JSON)
   - Hooks are stored in YAML but converted to JSON for persistence
   - Uses `yaml_to_json()` helper which round-trips through `serde_json`
   - Ensures hooks are portable and language-agnostic

4. **Name Validation** (`validate_skill_name()`)
   - Checks for empty name (error)
   - Checks length ≤ 64 characters (error if exceeded)
   - Checks character set: lowercase ASCII letters, digits, hyphens only
   - Falls back to directory name if frontmatter omits `name` field

5. **Path Resolution**
   - Sets `skill_dir` to parent directory of `SKILL.md`
   - Sets `source_path` to absolute path of `SKILL.md`
   - Both stored for later reference (e.g., `${RAGENT_SKILL_DIR}` substitution)

**Skill Discovery Function (`discover_skills()`):**

```rust
pub fn discover_skills(
    working_dir: &Path,
    extra_dirs: &[String]
) -> Vec<SkillInfo>
```

Returns all discovered skills from all scopes (personal, extra, project, nested) as a flat list. The caller (usually `SkillRegistry::load()`) handles scope-based deduplication.

**Efficiency Considerations:**

- **Lazy Loading**: Skills are loaded into memory at session start, not on-demand
- **No Caching**: Skills are re-discovered on each session (enables dynamic updates)
- **Parallel Discovery**: Directory reads are sequential (not parallelized)
- **Early Error Handling**: Individual skill parse failures don't block loading of other skills

**Special Cases:**

1. **Empty Directories**: Subdirectories without `SKILL.md` are silently skipped
2. **Files vs. Directories**: Only directories are considered; loose files in `skills/` are ignored
3. **Symlinks**: Followed transparently (no special handling)
4. **Hidden Directories**: Discovered normally (dot-names don't exclude them)
5. **Non-existent Paths**: Extra directories that don't exist are logged as warnings; session continues

**Logging Output:**

When a skill is successfully loaded, the logger emits:
```
DEBUG: Loaded skill '<name>' from <path> (scope: <scope>)
```

On parse error:
```
WARN: Failed to parse <path>: <error details>
```

On registry load completion:
```
INFO: Skill registry loaded: <bundled_count> bundled, <discovered_count> discovered, <total_count> registered (after dedup)
```

#### Paths Reference

All paths below are relative to the user's system (not project-specific unless noted):

| Type | Path Pattern | Example | Created By | Scope |
|------|--------------|---------|-----------|-------|
| **Personal Skills** | `~/.ragent/skills/<name>/SKILL.md` | `~/.ragent/skills/deploy/SKILL.md` | User or script | Personal |
| **Extra Skill Dir** | `<configured-path>/<name>/SKILL.md` | `/shared/skills/ci-deploy/SKILL.md` | Organization | Personal |
| **Project Skills** | `.ragent/skills/<name>/SKILL.md` | `.ragent/skills/build-frontend/SKILL.md` | Version control | Project |
| **Nested Skills** | `<subdir>/.ragent/skills/<name>/SKILL.md` | `packages/api/.ragent/skills/test/SKILL.md` | Version control | Project |
| **Skill Scripts** | `.ragent/skills/<name>/scripts/<script>` | `.ragent/skills/deploy/scripts/deploy.sh` | Manual | — |
| **Skill Templates** | `.ragent/skills/<name>/templates/<file>` | `.ragent/skills/deploy/templates/docker-compose.yml` | Manual | — |
| **Skill Examples** | `.ragent/skills/<name>/examples/<file>` | `.ragent/skills/deploy/examples/success.log` | Manual | — |

**Environment Variable Substitution in Paths:**

Paths in config (e.g., `skill_dirs`) support environment variable expansion:

```jsonc
{
  "skill_dirs": [
    "$HOME/my-skills",           // Expands to user's home directory
    "${HOME}/org-skills",         // Alternative syntax
    "/opt/shared/skills",         // Absolute (no expansion needed)
    "$XDG_CONFIG_HOME/skills"     // XDG standard
  ]
}
```

**Disk Usage Audit:**

To find all skills and their total size:

```bash
# Find all SKILL.md files
find ~ -name "SKILL.md" -type f

# Calculate total size of all skills
find ~/.ragent/skills .ragent/skills -type d -name "skills" \
  -exec du -sh {} \; 2>/dev/null

# List largest skills
find ~/.ragent/skills .ragent/skills -name "SKILL.md" \
  -exec sh -c 'du -h "$1" | awk "{print \$1, \"$(dirname \"$1\" | xargs basename)\"}"' _ {} \; \
  2>/dev/null | sort -rh
```

#### How to Define and Use Skills

**Creating a project-specific skill:**

```bash
mkdir -p .ragent/skills/deploy
cat > .ragent/skills/deploy/SKILL.md << 'EOF'
---
name: deploy
description: Deploy the application to production with validation
argument-hint: "[environment]"
model: "anthropic/claude-sonnet-4-20250514"
context: fork
agent: build
allowed-tools: [bash, read]
---

# Deploy to $0

You are a deployment specialist. Perform the following steps:

1. **Validate environment**: Ensure $ARGUMENTS is a valid target
2. **Run tests**: Execute `cargo test --release`
3. **Build**: Execute `cargo build --release`
4. **Deploy**: Push to $ARGUMENTS
5. **Verify**: Run smoke tests

Target endpoint: !`aws ssm get-parameter --name /deploy/$ARGUMENTS`

Return a summary of the deployment.
EOF
```

**Using the skill:**

```bash
# In TUI, type:
/deploy production

# Or programmatically:
ragent run --agent general "/deploy staging"
```

**Agent auto-invocation:**

The agent sees the skill in the "## Available Skills" section of the system prompt and can decide to use it:

```
User: "Deploy to production"
  ↓
Agent: "I'll use the /deploy skill to handle this safely."
  ↓
Agent invokes: /deploy production
  ↓
Forked build subagent runs the deployment
  ↓
Result returned to parent agent
```

**Creating a personal skill** (available across all projects):

```bash
mkdir -p ~/.ragent/skills/my-review
cat > ~/.ragent/skills/my-review/SKILL.md << 'EOF'
---
name: my-review
description: My custom code review checklist
---

Review this code against my personal standards:
- Are variable names clear and descriptive?
- Is error handling consistent with project style?
- Are there any performance concerns?
- Is there sufficient test coverage?
EOF
```

Now available in all projects:

```bash
cd ~/project1
/my-review        # Works in project 1

cd ~/project2
/my-review        # Also works in project 2
```

**Creating a dynamic skill with context injection:**

```markdown
---
name: pr-summary
description: Summarize a pull request
argument-hint: "[pr-number]"
context: fork
agent: explore
---

## PR #$0 Summary

Current branch: !`git rev-parse --abbrev-ref HEAD`

PR Files:
!`gh pr diff --name-only --repo . $0 2>/dev/null || echo "(No PR found)"`

PR Description:
!`gh pr view $0 --json body -q .body --repo . 2>/dev/null || echo "(Could not fetch)"`

Your task: Summarize this PR in 2-3 bullet points, highlighting the main changes.
EOF
```

**Skill with inline-only invocation:**

```markdown
---
name: security-check
description: Internal security scanning (for agents only)
user-invocable: false
disable-model-invocation: false
---

Perform a comprehensive security audit...
```

Only the agent can invoke this (hidden from `/` menu, agent can auto-use it).

**Skill with user-only invocation:**

```markdown
---
name: scheduled-task
description: Run periodic checks
disable-model-invocation: true
user-invocable: true
---

Check the following at midnight:
- Build status
- Deployment health
- Test coverage
EOF
```

Only the user can invoke via `/scheduled-task` (agent cannot auto-invoke).

---

### 3.18.1 Subagents & Agent Delegation ✅

Subagents are specialized agents invoked from within a session to handle focused, isolated tasks. They differ from the primary agent in scope, context, tools, and reasoning style.

#### Built-in Subagents

| Agent | Purpose | Tools | Use Cases |
|-------|---------|-------|-----------|
| `explore` | Fast codebase search and analysis | Read-only: grep, glob, list, read, bash | Find patterns, understand architecture, code review |
| `plan` | Read-only analysis and planning | Read-only: grep, glob, list, read, bash | Design systems, plan refactoring, analyze impacts |
| `build` | Compile, test, debug | Full: bash, read, write, edit | Run tests, build projects, fix compilation errors |
| `general` | Full-capability coding | All tools | General coding, refactoring, writing features |

#### Subagent Invocation Methods

**Method 1: Tab cycling in TUI**
```
Tab / Shift+Tab
# Cycles through: general → build → plan → explore → general
```

**Method 2: Slash command**
```
/agent explore
# Opens picker dialog if no name, or switches directly
```

**Method 3: Via `context: fork` skills**
```markdown
---
name: code-analysis
context: fork
agent: explore    # Skill runs in explore subagent
---
Analyze this code...
```

The skill runs in an isolated explore subagent context, then returns the result.

**Method 4: Via `plan_enter` / `plan_exit` tools**

The agent can explicitly delegate to the plan subagent:

```
Agent: "Let me analyze the architecture..."
Tool use: plan_enter
  task: "Analyze current modularization"
  context: "We have 15 source files..."
  ↓
Plan agent enters (isolated context)
  ↓
Plan agent uses read tools: grep, list, bash
  ↓
Plan agent exits via plan_exit tool
  summary: "Architecture is tightly coupled. Suggests splitting into modules."
  ↓
Summary injected into parent conversation
  ↓
Parent agent continues: "Based on the plan analysis, I'll refactor as follows..."
```

#### Context Isolation

Subagents run in **fully isolated contexts**:

| Aspect | Subagent Context | Parent Agent |
|--------|------------------|--------------|
| Conversation history | Empty — no prior messages | Full history maintained |
| System prompt | Custom per subagent | Standard system prompt |
| Working directory | Same as parent | Same as parent |
| Tool access | Restricted per subagent | Configured tool groups |
| Session storage | Separate session in DB | Original session |
| Snapshots/undo | Own snapshot chain | Own snapshot chain |
| TODOs | Empty list | Original todos |

**Example: Plan delegation with isolation**

```
Parent (general agent) at message 15 in conversation

User: "Should we refactor this?"
  ↓
Parent agent uses plan_enter tool
  ↓
Plan subagent created (new session, empty history)
  ↓
Plan agent prompt: "Analyze whether we should refactor..."
  ↓
Plan agent: can only use grep, glob, list, read, bash (no write)
  ↓
Plan agent: "The code is tightly coupled..."
  ↓
Plan agent calls plan_exit with summary
  ↓
Parent conversation continues at message 16
  ↓
Message 16 contains: "Plan result: The code is tightly coupled..."
```

Parent agent never sees the plan agent's internal reasoning, only the summary.

#### Tool Availability by Subagent

| Tool | General | Build | Plan | Explore | Ask |
|------|---------|-------|------|---------|-----|
| read | ✅ | ✅ | ✅ | ✅ | ❌ |
| write | ✅ | ✅ | ❌ | ❌ | ❌ |
| edit | ✅ | ✅ | ❌ | ❌ | ❌ |
| patch | ✅ | ✅ | ❌ | ❌ | ❌ |
| bash | ✅ | ✅ | ✅ | ✅ | ❌ |
| grep | ✅ | ✅ | ✅ | ✅ | ❌ |
| glob | ✅ | ✅ | ✅ | ✅ | ❌ |
| list | ✅ | ✅ | ✅ | ✅ | ❌ |
| plan_enter | ✅ | ✅ | ❌ | ❌ | ❌ |
| plan_exit | ❌ | ❌ | ✅ | ❌ | ❌ |
| question | ✅ | ✅ | ✅ | ✅ | ✅ |
| webfetch / websearch | ✅ | ✅ | ✅ | ✅ | ✅ |
| todo_read / todo_write | ✅ | ✅ | ✅ | ✅ | ❌ |
| MCP tools | ✅ | ✅ | ✅ | ✅ | ❌ |

#### Agent Delegation Tools

**`plan_enter` tool:**
- **Purpose**: Pause current agent, request analysis from the plan agent
- **Permission**: `plan`
- **Parameters**:
  - `task` (required, string): What to analyze or plan
  - `context` (optional, string): Additional context for the plan agent
- **Returns**: Metadata with `agent_switch: "plan"` flag
- **Effect**: Breaks the current agent loop, publishes `AgentSwitchRequested` event
- **TUI**: Shows tool call, plan agent takes over in isolation, result returned

**`plan_exit` tool:**
- **Purpose**: Return from plan agent to previous agent with summary
- **Permission**: `plan`
- **Parameters**:
  - `summary` (required, string): Analysis/planning result to return
- **Returns**: Metadata with `agent_restore: true` flag
- **Effect**: Breaks plan agent loop, publishes `AgentRestoreRequested` event
- **TUI**: Shows plan result, previous agent continues

#### Custom Subagents

Users can define custom agents that function as specialized subagents:

```jsonc
{
  "agents": {
    "security-reviewer": {
      "description": "Security-focused code reviewer",
      "mode": "subagent",
      "prompt": "You are a security expert. Focus on vulnerabilities, authentication, data protection, and compliance.",
      "tool_groups": ["read", "question"],
      "max_steps": 50
    },
    "performance-analyst": {
      "description": "Performance and optimization specialist",
      "mode": "subagent",
      "prompt": "You are a performance engineer. Focus on bottlenecks, algorithmic efficiency, and optimization opportunities.",
      "tool_groups": ["read", "bash", "question"],
      "max_steps": 50
    }
  }
}
```

Then invoke via:
- Slash command: `/agent security-reviewer`
- Skill: `agent: security-reviewer` in `context: fork` skill
- Tab cycling if in the agent list

#### Multi-Agent Orchestration (F6) ✅

The `ragent-core` crate provides a full multi-agent orchestration layer. Multiple in-process agents can collaborate on a single job via capability-based routing, result aggregation, and event streams.

**Key components:**

| Component | Description |
|---|---|
| `AgentRegistry` | Register/unregister agents with capability tags; heartbeat and stale-pruning |
| `Coordinator` | Accepts `JobDescriptor`s, matches agents by capability, dispatches subtasks, aggregates results |
| `InProcessRouter` | Delivers messages to agent mailboxes via Tokio mpsc channels; configurable per-request timeout |
| `MetricsSnapshot` | Live counters: `active_jobs`, `completed_jobs`, `timeouts`, `errors` |

**Aggregation strategies:**

| Method | Description |
|---|---|
| `start_job_sync(desc)` | Fan-out to all matched agents; await all responses; concatenate |
| `start_job_first_success(desc)` | Try agents in order; return first non-error response; skip timeouts |
| `start_job_async(desc)` | Fire-and-forget; returns `job_id`; poll via `get_job_result(id)` or subscribe via `subscribe_job_events(id)` |

**HTTP API (ragent-server):**

| Endpoint | Description |
|---|---|
| `POST /orchestrator/start` | Start a job; body: `{"required_capabilities":["search"],"payload":"...","mode":"async"}` |
| `GET /orchestrator/metrics` | Metrics snapshot JSON |
| `GET /orchestrator/jobs/{id}` | Poll job status and result |

**Rust example:**

```rust
use ragent_core::orchestrator::{AgentRegistry, Coordinator, JobDescriptor, Responder};
use futures::future::FutureExt;
use std::sync::Arc;

let registry = AgentRegistry::new();
let r: Responder = Arc::new(|p| async move { format!("handled: {}", p) }.boxed());
registry.register("my-agent", vec!["search".to_string()], Some(r)).await;

let coord = Coordinator::new(registry);
let result = coord.start_job_sync(JobDescriptor {
    id: "job-1".to_string(),
    required_capabilities: vec!["search".to_string()],
    payload: "find TODOs".to_string(),
}).await?;
```

See `crates/ragent-core/examples/orchestration.rs` for a complete runnable example.

---

### 3.20 Persistent Memory ❌

Persistent memory allows ragent to build a lasting understanding of the project across sessions. Memory operates at two levels: **user-initiated memories** stored in the database, and **auto-memory** where the agent writes its own notes to the filesystem.

#### Memory Types

| Type | Description | Example |
|------|-------------|---------|
| Convention | Coding style preferences | "Use 4-space indentation in Rust files" |
| Pattern | Recurring code patterns | "Error handling uses `anyhow::Result` with `.context()`" |
| Preference | User preferences | "Prefer `tokio::fs` over `std::fs` for async file operations" |
| Structure | Project layout knowledge | "Tests live in `tests/` directory per crate, not inline" |

#### Database Storage

Memories are stored in the SQLite database (`memories` table) with:
- `id` — unique identifier
- `category` — convention, pattern, preference, structure
- `content` — the memory text
- `source` — file or conversation that produced it
- `created_at` — when the memory was recorded

#### Auto-Memory (Filesystem)

In addition to database-backed memories, ragent supports agent-written auto-memory files that persist across sessions:

**Project-level memory** (`.ragent/memory/`):

```
.ragent/memory/
  MEMORY.md         # Entrypoint — loaded at session start (max 200 lines)
  architecture.md   # Topic file for detailed architecture notes
  conventions.md    # Topic file for coding conventions
  gotchas.md        # Topic file for known pitfalls
```

**User-level memory** (`~/.ragent/memory/`):

```
~/.ragent/memory/
  MEMORY.md         # Global preferences loaded for all projects
  rust-patterns.md  # Cross-project topic file
```

**Rules for auto-memory:**
- `MEMORY.md` is the entrypoint — always loaded at session start and survives context compaction
- Topic files are loaded on-demand when the agent needs detailed context
- The agent creates and updates memory files autonomously as it discovers patterns
- `MEMORY.md` has a 200-line cap to avoid bloating the system prompt
- Topic files have no line cap but should stay focused

#### Usage

- Database memories are loaded at session start and injected into the system prompt
- Auto-memory `MEMORY.md` files are read from disk and injected alongside database memories
- The agent can create new memories via `memory_write` tool (database) or by writing to `.ragent/memory/` (filesystem)
- Users can review and manage memories via `/memory` slash command (browse, toggle, delete)
- Memories persist across sessions and reduce the need to repeat context
- `/memory` displays a browsable list with source, category, and toggle controls

---

### 3.21 Trusted Directories ❌

Trusted directories control where ragent can read, modify, and execute files, providing a security boundary.

#### Behaviour

1. On first launch from a directory, ragent prompts the user to confirm trust
2. Trusted directories are recorded in the settings database
3. File operations outside trusted directories require explicit permission
4. ragent should not be launched from the user's home directory (warning displayed)

#### Configuration

```jsonc
{
  "trusted_directories": [
    "/home/user/projects",
    "/home/user/work"
  ]
}
```

Trusted directory scoping is enforced by the permission system. File access outside trusted directories triggers the `external_directory` permission check.

---

### 3.22 Codebase Indexing & Semantic Search ❌

Codebase indexing enables natural-language semantic search across the entire project, complementing the existing `grep` (text matching) and `glob` (file patterns) tools.

#### Architecture

1. **Code Parsing**: Use Tree-sitter to parse source files into semantic blocks (functions, classes, methods, structs, impls)
2. **Embedding Generation**: Convert each code block into a vector embedding using a configurable embedding provider (OpenAI, Google Gemini, Ollama for local/offline)
3. **Vector Storage**: Store embeddings in an embedded vector database (e.g., `qdrant` or `hnsw` via Rust crate) for fast similarity search
4. **Search Interface**: The `codebase_search` tool accepts natural language queries and returns ranked code snippets with file paths and line numbers

#### Features

| Feature | Description |
|---------|-------------|
| Incremental indexing | Only re-index modified files (hash-based change detection) |
| File watching | Monitor workspace for changes in real-time |
| Branch awareness | Detect git branch switches and re-index as needed |
| Configurable threshold | Similarity score threshold for result relevance (0.0–1.0) |
| .gitignore / .ragentignore aware | Exclude ignored files from indexing |
| Tree-sitter fallback | Line-based chunking for unsupported file types |

#### Configuration

```jsonc
{
  "indexing": {
    "enabled": true,
    "embedding_provider": "openai",       // "openai" | "gemini" | "ollama"
    "embedding_model": "text-embedding-3-small",
    "vector_store": "embedded",           // "embedded" | "qdrant"
    "qdrant_url": "http://localhost:6333",
    "similarity_threshold": 0.4,
    "max_results": 20
  }
}
```

#### Semantic Query Examples

- "authentication middleware logic"
- "error handling for database connections"
- "how are tool permissions checked"

---

### 3.23 Post-Edit Diagnostics ❌

After file modifications, ragent can pause briefly to collect diagnostics (compiler errors, lint warnings) from the LSP before proceeding, catching errors introduced by edits immediately.

#### Flow

1. Agent executes a `write`, `edit`, or `patch` tool
2. ragent waits for a configurable delay (default: 1000 ms) for LSP diagnostics to update
3. New diagnostics (errors only, not pre-existing ones) are captured
4. If new errors are detected, they are automatically injected into the conversation as context
5. The agent can then fix the introduced errors before proceeding

#### Configuration

```jsonc
{
  "diagnostics": {
    "post_edit_check": true,
    "delay_ms": 1000,
    "severity": "error"      // "error" | "warning" | "all"
  }
}
```

This integrates with the existing LSP integration (§ 3.12) and the auto-approve system. When auto-approve is enabled for writes, the delay gives the LSP time to detect issues before the agent moves on.

---

### 3.24 Task Todo List ✅

Complex multi-step tasks are tracked via an interactive todo list that persists throughout the session, giving both the agent and user visibility into progress.

#### Features

- Agent can create, update, and complete todo items via `todo_read` / `todo_write` tools
- Todo list is displayed in the TUI status bar with progress indicator
- Each item has status: `pending` | `in_progress` | `completed`
- User can view the full todo list with `/todo` slash command
- User can edit todo items (add, remove, change status) via the TUI
- Todo state is stored in the session's SQLite database and persists across reconnections

#### Display

The TUI shows a compact summary in the status bar:

```
[TODO: 3/7 ✓] Current: Implement user auth endpoint
```

A full expanded view shows all items with status indicators:
- `○` pending
- `◐` in progress
- `●` completed

#### Agent Integration

The orchestrator agent always creates todo lists when decomposing complex tasks. Other agents create them for multi-step work. The todo list appears in the system prompt as a "REMINDERS" block, giving the agent persistent awareness of remaining work.

---

### 3.25 Prompt Enhancement ❌

An optional AI-powered prompt enhancement feature that refines the user's input before sending it to the agent, making prompts clearer, more specific, and more likely to produce high-quality results.

#### How It Works

1. User types a prompt in the input area
2. User triggers enhancement (keyboard shortcut or button)
3. ragent sends the original prompt to the LLM with an enhancement meta-prompt
4. The enhanced prompt replaces the original in the input area
5. User reviews, optionally edits, and sends

#### Features

- **Context-aware**: Can include recent conversation history for better enhancement
- **Customisable**: The enhancement meta-prompt template is user-configurable
- **Undo**: `Ctrl+Z` restores the original prompt
- **Non-blocking**: Enhancement happens asynchronously

#### Configuration

```jsonc
{
  "enhance_prompt": {
    "enabled": true,
    "use_conversation_context": true,
    "max_history_messages": 10,
    "custom_prompt": null   // null = use default; string = custom template
  }
}
```

---

### 3.26 Hierarchical Custom Instructions ⚠️

Custom instructions shape agent behaviour across multiple levels — global settings, project rules, and agent-specific (mode-specific) rules — with a clear precedence hierarchy.

#### Instruction Sources (lowest → highest priority)

| Level | Location | Scope |
|-------|----------|-------|
| Global rules directory | `~/.config/ragent/rules/` | All projects, all agents |
| Global agent-specific rules | `~/.config/ragent/rules-{agent}/` | All projects, specific agent |
| Project rules directory | `.ragent/rules/` | Current project, all agents |
| Project agent-specific rules | `.ragent/rules-{agent}/` | Current project, specific agent |
| Project rules file (fallback) | `.ragentrules` | Current project (if no rules directory) |
| AGENTS.md | `./AGENTS.md` | Current project (existing feature) |
| Config custom instructions | `ragent.json` → `custom_instructions` | Per project config |
| Enterprise / managed rules | Managed settings | All users in organization |

Rules are loaded recursively from directories, sorted alphabetically by filename, and concatenated into the system prompt. Files can be `.md`, `.txt`, or any plain text format.

#### Agent-Specific Rules

Agent-specific rules only apply when that agent is active:

```
.ragent/
├── rules/              # Applied to all agents
│   ├── 01-coding-style.md
│   └── 02-documentation.md
├── rules-general/      # Applied only to "general" agent
│   └── typescript-rules.md
├── rules-plan/         # Applied only to "plan" agent
│   └── planning-guidelines.md
└── rules-debug/        # Applied only to "debug" agent
    └── debug-workflow.md
```

#### Path-Specific Rules

Rules can be scoped to specific file paths using YAML frontmatter, so they only activate when the agent is working on matching files:

```markdown
---
paths:
  - "src/components/**/*.tsx"
  - "src/components/**/*.test.tsx"
---

When editing React components:
- Use functional components with hooks
- Always include PropTypes or TypeScript interfaces
- Co-locate test files with components
```

Path-specific rules are evaluated lazily — they load into context only when the agent reads or edits a file matching the `paths` glob patterns. This keeps the system prompt lean for monorepos with many domain-specific rules.

#### File Imports

Rules files can import content from other files using the `@path/to/file` syntax:

```markdown
# Coding Standards

Follow these conventions:

@.ragent/rules/shared/error-handling.md
@.ragent/rules/shared/naming-conventions.md
```

Imported files are resolved relative to the project root. Circular imports are detected and ignored.

#### Monorepo Filtering

For monorepos, the `rules_excludes` config option prevents loading rules from irrelevant packages:

```jsonc
{
  "rules_excludes": [
    "packages/legacy/**",
    "packages/deprecated/**"
  ]
}
```

#### System Prompt Assembly

Instructions are injected into the system prompt in this order:
1. Agent role definition
2. Enterprise / managed rules
3. Global rules
4. Global agent-specific rules
5. Project rules (filtered by path-specific scoping)
6. Project agent-specific rules
7. AGENTS.md content
8. Config custom instructions
9. Preloaded skill descriptions
10. Auto-memory (MEMORY.md)
11. Tool definitions

---

### 3.27 File Ignore Patterns ❌

A `.ragentignore` file controls which files ragent can access, modify, or include in context — analogous to `.gitignore` but for agent access control.

#### Behaviour

- File uses `.gitignore` syntax (glob patterns, negation with `!`, comments with `#`)
- The `.ragentignore` file itself is always implicitly ignored (agent cannot modify its own access rules)
- Changes to `.ragentignore` are hot-reloaded without restarting

#### Enforcement

| Tool | Enforcement |
|------|-------------|
| `read`, `office_read`, `pdf_read` | Blocked — returns "file ignored" error |
| `write`, `create`, `edit`, `multiedit`, `patch` | Blocked — returns "file ignored" error |
| `list`, `glob` | Excluded from results (or marked with 🔒) |
| `grep` | Excluded from search results |
| `bash` | File-reading commands (cat, head, tail) targeting ignored files are blocked |
| `codebase_search` | Excluded from indexing |

#### Example `.ragentignore`

```gitignore
# Secrets and credentials
.env*
config/secrets.json

# Build output
target/
dist/
node_modules/

# Large assets
*.mp4
*.zip
assets/images/

# Allow one specific env file
!.env.example
```

#### Interaction with Permissions

`.ragentignore` is enforced **in addition to** the permission system. A file can be allowed by permissions but still blocked by `.ragentignore`. The ignore file acts as a hard boundary that cannot be overridden by the agent.

---

### 3.28 Suggested Responses ❌

After each assistant message, ragent can generate context-aware follow-up suggestions that the user can select or edit, speeding up iterative workflows.

#### Behaviour

1. After the agent completes a response, it optionally generates 2–4 suggested follow-up messages
2. Suggestions appear as selectable chips below the assistant message in the TUI
3. User can: select a suggestion (sends it immediately), edit before sending, or type their own message
4. Suggestions are generated using a lightweight LLM call with recent conversation context

#### Configuration

```jsonc
{
  "suggested_responses": {
    "enabled": false,
    "max_suggestions": 3
  }
}
```

Suggestions are disabled by default to avoid unnecessary LLM calls, but can be enabled for interactive exploration sessions.

---

### 3.29 Session Resume & Management ⚠️

Session resume allows users to continue previous conversations seamlessly, maintaining full context, working directory, and agent state.

#### Resume Methods

| Method | Description |
|--------|-------------|
| `ragent --continue` | Resume the most recent session automatically |
| `ragent --resume` | Open interactive session picker with search/filter |
| `ragent --from-pr <number>` | Resume or start a session linked to a GitHub pull request |
| `ragent session resume <id>` | Resume a specific session by ID |
| `/resume` | Switch to a different session from within the TUI |

#### Session Picker

The `--resume` flag opens an interactive session picker:
- Sessions listed by most recent first with title, age, and message count
- Fuzzy search by session title or content keywords
- Keyboard navigation: `↑`/`↓` to select, `Enter` to resume, `Esc` to cancel
- Sessions are grouped: today, yesterday, this week, older

#### Session Naming

Sessions can be explicitly named for findability:
- `/name <title>` — set a human-readable name for the current session
- Auto-generated titles from the first prompt (via the `title` agent)
- Named sessions appear with their title in the session picker and session list

#### PR-Linked Sessions

When using `--from-pr <number>`, ragent:
1. Checks for an existing session linked to the PR
2. If found, resumes it with the latest PR context
3. If not found, creates a new session with the PR diff and description pre-loaded
4. Links the session to the PR for future `--from-pr` lookups

---

### 3.30 Git Worktree Isolation ❌

Git worktree isolation enables parallel ragent sessions to work on the same repository without file conflicts, by giving each session its own working copy.

#### Usage

```bash
# Create a named worktree session
ragent --worktree feature-auth

# Auto-generated worktree name
ragent --worktree auto
```

#### Behaviour

1. On `--worktree <name>`, ragent creates a new git worktree from the current branch
2. The session runs entirely within the worktree directory
3. File changes are isolated — other sessions/editors see no uncommitted changes
4. On session end:
   - If no changes were made → worktree is automatically removed
   - If changes exist → user is prompted to commit, merge, or keep the worktree

#### Subagent Worktree Isolation

Custom agents can specify `isolation: worktree` to automatically run in a dedicated worktree:

```jsonc
{
  "agents": {
    "batch-worker": {
      "description": "Parallel batch processing agent",
      "isolation": "worktree"
    }
  }
}
```

This is used by bundled skills like `/batch` to spawn multiple parallel agents, each in its own worktree.

#### Hooks

The `WorktreeCreate` and `WorktreeRemove` hook events fire when worktrees are created/removed, enabling non-git VCS support or custom cleanup logic.

---

### 3.31 Context Compaction ⚠️

Context compaction compresses the conversation history when the context window approaches its limit, preserving the most important information while freeing space.

#### Triggering

| Trigger | Description |
|---------|-------------|
| Automatic | When context usage exceeds 80% of the model's context window |
| Manual | User invokes `/compact` slash command |
| Custom prompt | `/compact <instructions>` with specific guidance on what to preserve |

#### Behaviour

1. The `compaction` agent summarizes the conversation history
2. Old messages are replaced with a compressed summary
3. **AGENTS.md and rules are re-injected from disk** (not from compressed context) — this ensures custom instructions survive compaction unchanged
4. **Auto-memory MEMORY.md is re-loaded** from disk after compaction
5. The `PreCompact` hook event fires before compaction begins
6. Tool results and file contents are aggressively summarized; key decisions and instructions are preserved

#### Configuration

```jsonc
{
  "compaction": {
    "auto": true,
    "threshold": 0.8,         // Trigger at 80% context usage
    "preserve_recent": 5      // Always keep last 5 message pairs
  }
}
```

---

### 3.32 Headless / Pipe Mode ⚠️

Headless mode enables ragent to be used programmatically in scripts, CI/CD pipelines, and other non-interactive contexts.

#### Single-Shot Execution

```bash
# Simple prompt
ragent -p "Explain the architecture of this project"

# With specific model and agent
ragent -p "Fix the failing tests" --model anthropic/claude-sonnet-4-20250514 --agent build

# Auto-approve all permissions (for CI)
ragent -p "Run cargo test and fix failures" --yes
```

#### Stdin Piping

```bash
# Pipe file contents for analysis
cat src/main.rs | ragent -p "Review this code for bugs"

# Pipe command output
cargo test 2>&1 | ragent -p "Analyze these test failures and suggest fixes"

# Pipe git diff
git diff | ragent -p "Write a commit message for these changes"
```

#### Output Formats

| Flag | Format | Description |
|------|--------|-------------|
| (default) | `text` | Plain text response |
| `--output-format json` | JSON | Structured output with metadata |
| `--output-format stream-json` | NDJSON | Newline-delimited JSON events (tool calls, text deltas) |

JSON output includes:
```jsonc
{
  "response": "The architecture follows...",
  "usage": { "input_tokens": 1234, "output_tokens": 567 },
  "tools_used": ["read", "grep"],
  "duration_ms": 4500
}
```

#### Integration Examples

```bash
# CI: Auto-fix lint errors
ragent -p "Fix all clippy warnings" --yes --output-format json

# Pre-commit hook
ragent -p "Review the staged changes for issues" --permission-mode plan < <(git diff --cached)

# Batch processing
find src -name "*.rs" | xargs -I {} ragent -p "Add missing doc comments to {}" --yes
```

---

### 3.33 Extended Thinking & Effort Levels ✅

Extended thinking gives the agent visible step-by-step reasoning, improving accuracy for complex tasks. Effort levels control how much reasoning the agent performs.

#### Effort Levels

| Level | Description | Use Case |
|-------|-------------|----------|
| `low` | Minimal reasoning, fast responses | Simple questions, quick lookups |
| `medium` | Balanced reasoning (default) | General coding tasks |
| `high` | Deep reasoning with extended thinking | Complex architecture, debugging |

#### Configuration

```jsonc
{
  "thinking": {
    "enabled": true,
    "effort": "medium",
    "max_thinking_tokens": 8192
  }
}
```

- `RAGENT_THINKING_EFFORT` environment variable overrides the effort level
- `MAX_THINKING_TOKENS` environment variable sets the maximum thinking token budget

#### TUI Integration

- Thinking text appears in a collapsible block above the response (dimmed text)
- `Ctrl+O` toggles verbose mode to always show thinking blocks
- The `ultrathink` keyword in a prompt temporarily elevates to maximum reasoning for one response

#### Per-Agent Override

Individual agents can override the thinking configuration:

```jsonc
{
  "agents": {
    "debug": {
      "options": {
        "extended_thinking": true,
        "thinking_effort": "high"
      }
    }
  }
}
```

---

### 3.34 @ File References ❌

The `@` syntax provides quick inline file and directory references in prompts, allowing users to include specific context without full paths.

#### Syntax

| Reference | Description |
|-----------|-------------|
| `@filename` | Include a file by name (fuzzy-matched against project files) |
| `@path/to/file` | Include a file by exact path |
| `@path/to/dir/` | Include a directory listing |
| `@url` | Fetch and include web page content |

#### Behaviour

1. On prompt submission, `@` references are detected and resolved
2. File contents are read and appended to the prompt as context
3. Directory references expand to a file listing (like `list` tool output)
4. Fuzzy matching suggests completions as the user types after `@`
5. The TUI shows inline autocomplete with `Tab` to accept

#### TUI Autocomplete

When the user types `@`, a dropdown appears showing:
- Recently accessed files (top)
- Files matching the typed prefix
- Directories (with trailing `/` indicator)

Navigation: `↑`/`↓` to select, `Tab` to accept, `Esc` to dismiss.

---

## 4. Data Flow

```
User Input (TUI / HTTP)
       │
       ▼
┌──────────────┐
│ Session Mgr  │── Create/load session, store user message
└──────┬───────┘
       │
       ▼
┌──────────────┐
│ Prompt Build │── Assemble: system prompt + instructions + message history
│              │   + tool definitions + workspace context
└──────┬───────┘
       │
       ▼
┌──────────────┐
│ LLM Stream   │── Send to provider API, receive streaming response
└──────┬───────┘
       │
       ├──▶ TextDelta → accumulate text → emit events
       │
       ├──▶ ReasoningDelta → accumulate reasoning → emit events
       │
       └──▶ ToolCall → ┌─────────────────────────────────┐
                        │ 1. Validate arguments            │
                        │ 2. Check permissions              │
                        │ 3. If Ask → prompt user           │
                        │ 4. Take snapshot (for edits)      │
                        │ 5. Execute tool                   │
                        │ 6. Return output to LLM           │
                        └──────────┬──────────────────────┘
                                   │
                                   ▼
                          LLM receives tool result
                          → may call more tools
                          → eventually emits final text
                                   │
                                   ▼
                        ┌─────────────────────┐
                        │ Store assistant msg  │
                        │ Update session       │
                        │ Generate title/summ  │
                        └─────────────────────┘
```

#### Doom Loop Protection

If the agent calls more than `max_steps` tools (default: 100) without producing a final response, ragent triggers the `doom_loop` permission check. If denied, the loop terminates with an error message to the LLM.

---

## 5. Configuration File Format

### Minimal `ragent.json`

```jsonc
{
  // Simplest config: just set your provider and go
  "provider": {
    "anthropic": {}
  }
}
```

### Full Example

```jsonc
{
  "username": "developer",
  "default_agent": "build",

  "provider": {
    "anthropic": {
      "env": ["ANTHROPIC_API_KEY"],
      "models": {
        "claude-sonnet-4-20250514": {
          "name": "Claude Sonnet 4",
          "cost": { "input": 3.0, "output": 15.0 }
        }
      }
    },
    "openai": {
      "env": ["OPENAI_API_KEY"]
    },
    "ollama": {
      "api": { "base_url": "http://localhost:11434/v1" },
      "models": {
        "llama3.3": {
          "name": "Llama 3.3 70B",
          "cost": { "input": 0, "output": 0 }
        }
      }
    }
  },

  "permission": {
    "*": "allow",
    "edit": {
      "*": "allow",
      "*.env*": "deny"
    },
    "bash": {
      "*": "ask"
    },
    "external_directory": {
      "*": "ask"
    }
  },

  "agent": {
    "build": {
      "model": "anthropic/claude-sonnet-4-20250514"
    },
    "architect": {
      "name": "System Architect",
      "model": "anthropic/claude-sonnet-4-20250514",
      "mode": "primary",
      "prompt": "You are a senior system architect. Focus on design patterns, scalability, and maintainability.",
      "permission": {
        "edit": { "*": "ask" }
      }
    }
  },

  "command": {
    "test": {
      "command": "cargo test",
      "description": "Run the test suite"
    },
    "lint": {
      "command": "cargo clippy --all-targets",
      "description": "Run linter"
    }
  },

  "mcp": {
    "github": {
      "type": "stdio",
      "command": "npx",
      "args": ["-y", "@modelcontextprotocol/server-github"],
      "env": {
        "GITHUB_TOKEN": "${env.GITHUB_TOKEN}"
      }
    }
  },

  "instructions": [
    "Always write idiomatic Rust code.",
    "Prefer returning Result over panicking.",
    ".ragent/instructions.md"
  ],

  "experimental": {
    "open_telemetry": false
  }
}
```

---

## 6. Rust Crate Map

| Module | Crate(s) | Purpose |
|--------|----------|---------|
| CLI | `clap` | Command-line argument parsing |
| HTTP Server | `axum`, `tower`, `tower-http` | REST API + SSE |
| TUI | `ratatui`, `crossterm` | Terminal user interface |
| Async Runtime | `tokio` | Async I/O, tasks, channels |
| HTTP Client | `reqwest` | LLM API calls, web fetch |
| JSON | `serde`, `serde_json` | Serialization/deserialization |
| Config | `serde_json`, `jsonc-parser` or `json5` | Config file parsing (with comments) |
| Database | `rusqlite` (bundled) | SQLite storage |
| Logging | `tracing`, `tracing-subscriber` | Structured logging |
| File Search | `grep-regex`, `globset`, `ignore` | ripgrep-style search |
| Diff/Patch | `similar`, `diffy` | Unified diff generation and application |
| Markdown | `termimad` or `pulldown-cmark` | Render markdown in TUI |
| Syntax Highlight | `syntect` | Code highlighting in TUI |
| UUID/ULID | `ulid` | Unique ID generation |
| MCP | Custom implementation (JSON-RPC 2.0 over stdio/SSE/HTTP) | Model Context Protocol client |
| LSP | `lsp-types`, `tokio::process` | Language Server Protocol client |
| Process | `tokio::process` | Shell command execution |
| Compression | `flate2` or `zstd` | Snapshot compression |
| Template | `minijinja` or `handlebars` | System prompt templates |
| Glob | `globset` | File pattern matching |
| Error | `anyhow`, `thiserror` | Error handling |
| Streaming | `tokio-stream`, `async-stream` | Async stream utilities |
| AWS | `aws-config`, `aws-sdk-bedrockruntime` | AWS Bedrock provider |

---

## 7. Project Layout

```
ragent/
├── Cargo.toml                  # Workspace root
├── Cargo.lock
├── SPEC.md                     # This file
├── README.md
├── LICENSE                     # MIT
├── ragent.json                 # Default/example config
│
├── crates/
│   ├── ragent-core/            # Core library (all business logic)
│   │   ├── Cargo.toml
│   │   └── src/
│   │       ├── lib.rs
│   │       ├── agent/          # Agent definitions, resolution, prompt building
│   │       │   ├── mod.rs
│   │       │   ├── builtin.rs  # Built-in agents (build, plan, general, explore)
│   │       │   └── prompt.rs   # System prompt construction
│   │       ├── config/         # Configuration loading, merging, schema
│   │       │   ├── mod.rs
│   │       │   └── schema.rs
│   │       ├── event/          # Event bus (tokio broadcast)
│   │       │   └── mod.rs
│   │       ├── llm/            # LLM streaming trait + shared utilities
│   │       │   ├── mod.rs
│   │       │   └── stream.rs
│   │       ├── mcp/            # MCP client (stdio, SSE, HTTP transports)
│   │       │   ├── mod.rs
│   │       │   ├── stdio.rs
│   │       │   ├── sse.rs
│   │       │   └── http.rs
│   │       ├── message/        # Message model, parts, serialization
│   │       │   └── mod.rs
│   │       ├── permission/     # Permission rules, evaluation, ask flow
│   │       │   └── mod.rs
│   │       ├── provider/       # Provider adapters
│   │       │   ├── mod.rs
│   │       │   ├── anthropic.rs
│   │       │   ├── copilot.rs   # GitHub Copilot (OpenAI-compatible)
│   │       │   ├── openai.rs   # Also used by OpenRouter, Groq, etc.
│   │       │   ├── ollama.rs
│   │       │   ├── google.rs
│   │       │   ├── azure.rs
│   │       │   ├── bedrock.rs
│   │       │   └── ollama.rs
│   │       ├── session/        # Session lifecycle, processor (agent loop)
│   │       │   ├── mod.rs
│   │       │   ├── processor.rs
│   │       │   └── compaction.rs
│   │       ├── snapshot/       # File snapshot and undo
│   │       │   └── mod.rs
│   │       ├── storage/        # SQLite database, migrations
│   │       │   ├── mod.rs
│   │       │   └── migrations/
│   │       └── tool/           # Tool trait, built-in tools
│   │           ├── mod.rs
│   │           ├── bash.rs
│   │           ├── edit.rs
│   │           ├── grep.rs
│   │           ├── glob.rs
│   │           ├── list.rs
│   │           ├── patch.rs
│   │           ├── question.rs
│   │           ├── read.rs
│   │           ├── webfetch.rs
│   │           ├── websearch.rs
│   │           └── write.rs
│   │
│   ├── ragent-server/          # HTTP/SSE server
│   │   ├── Cargo.toml
│   │   └── src/
│   │       ├── lib.rs
│   │       ├── routes/         # Axum route handlers
│   │       └── sse.rs          # SSE event stream
│   │
│   └── ragent-tui/             # Terminal UI
│       ├── Cargo.toml
│       └── src/
│           ├── lib.rs
│           ├── app.rs          # Application state
│           ├── input.rs        # Input handling
│           ├── layout.rs       # Screen layout
│           ├── widgets/        # Custom ratatui widgets
│           │   ├── message.rs  # Message display (with markdown)
│           │   ├── tool_call.rs # Tool call display
│           │   └── permission.rs # Permission dialog
│           └── theme.rs        # Colors, styles
│
├── src/
│   └── main.rs                 # Binary entry point (thin wrapper)
│
└── tests/
    ├── integration/            # End-to-end tests
    └── fixtures/               # Test data
```

---

## 8. Build & Distribution

### Build

```bash
# Debug build
cargo build

# Release build (optimized, stripped)
cargo build --release

# Cross-compile (via cross)
cross build --release --target aarch64-unknown-linux-musl
cross build --release --target x86_64-apple-darwin
cross build --release --target x86_64-pc-windows-msvc
```

### Binary Size Optimization

- LTO (Link-Time Optimization) enabled in release profile
- `strip = true` in Cargo.toml release profile
- `opt-level = "z"` for size optimization (or `"3"` for speed)
- `codegen-units = 1` for maximum optimization

### Distribution Channels

| Channel | Format |
|---------|--------|
| GitHub Releases | Pre-built binaries per platform |
| Homebrew | `brew install ragent` |
| Cargo | `cargo install ragent` |
| AUR | `pacman -S ragent` |
| Nix | `nix run github:user/ragent` |
| Docker | `ghcr.io/user/ragent:latest` |

---

## 9. Testing Strategy

| Layer | Approach | Crates |
|-------|----------|--------|
| Unit | Test individual functions (config parsing, permission eval, prompt building) | Built-in `#[test]` |
| Integration | Test tool execution, session lifecycle, MCP client, provider streaming (with mock HTTP) | `tokio::test`, `wiremock` |
| E2E | Full binary execution against mock LLM server | `assert_cmd`, `predicates` |
| TUI | Tests for agent switching, scrolling, session resume, slash commands, text selection | `#[test]` / `#[tokio::test]` |
| Fuzzing | Fuzz config parsing, tool input deserialization | `cargo-fuzz` |

Tests are located in `tests/` directories within each crate (not inline in source files). Current test count: **195+ tests** across `ragent-core` and `ragent-tui`.

### Mock LLM Server

A built-in mock server (feature-gated behind `#[cfg(test)]`) replays canned LLM responses including tool calls, enabling deterministic integration tests without real API calls.

---

## 10. Future / Stretch Goals

| # | Feature | Description | Status |
|---|---------|-------------|--------|
| F1 | Web UI | SolidJS-based web frontend connecting to the ragent server | ❌ |
| F2 | Mobile client | Remote control ragent from a phone via the HTTP API | ❌ |
| F3 | Plugin system | WASM-based plugin execution for custom tools | ❌ |
| F4 | ~~Git worktree isolation~~ | ~~Run each session in a separate git worktree for parallel work~~ → **Promoted to §3.30** | ❌ |
| F5 | OpenTelemetry | Trace spans for LLM calls, tool execution, and session lifecycle | ❌ |
| F6 | Multi-agent orchestration | Multiple agents collaborating on a single task | ✅ |
| F7 | Code generation benchmarks | Automated evaluation harness for measuring agent quality | ❌ |
| F8 | Enterprise features | Managed config, audit logging, SSO | ❌ |
| F9 | Voice input | Microphone input transcribed to text for hands-free coding | ❌ |
| F10 | Image/screenshot input | Vision model support for UI debugging | ❌ |
| F11 | ACP (Agent Client Protocol) | Support the open standard Agent Client Protocol for interoperability with other AI agent ecosystems | ❌ |
| F12 | `/feedback` command | Built-in user feedback submission mechanism | ❌ |
| F13 | Sub-agent spawning | Launch specialized sub-agents (e.g., explore, code-review) from within a session for focused tasks | ✅ |
| F14 | Background agents | Run multiple agent instances concurrently for parallel task execution | ✅ |
| F15 | Marketplace | Community hub for sharing and discovering custom agents, skills, and rule sets | ❌ |
| F16 | API configuration profiles | Named profiles for different API providers/models, switchable per agent or session | ❌ |
| F17 | Concurrent file operations | Parallel file reads and edits for faster multi-file workflows | ❌ |
| F18 | Model temperature control | Per-session or per-agent temperature override exposed in TUI settings | ❌ |
| F19 | Agent import/export | Export agent definitions (including rules) to portable YAML/JSON for team sharing | ❌ |
| F20 | Custom tools (user-defined) | Define project-specific tools in a scripting language that ragent can invoke | ❌ |
| F21 | Agent teams | Coordinated parallel agents with shared task queues, inter-agent messaging, and team-level progress tracking | ❌ |
| F22 | Plugin marketplace | Centralized registry for publishing/installing agents, skills, hooks, and rule sets with versioning | ❌ |
| F23 | Container sandboxing | Run agent sessions inside Docker/Podman containers for full filesystem and network isolation | ❌ |
| F24 | Output styles | Configurable response styles (explanatory, concise, educational, terse) per session or agent | ❌ |
| F25 | Scheduled tasks | `/loop` command for recurring prompts on intervals — polling deployments, babysitting PRs, periodic checks | ❌ |
| F26 | Interactive tutorials | Built-in onboarding flow teaching new users ragent features via guided tasks | ❌ |
| F27 | Session branching | Fork a session at any point to explore alternative approaches without losing the original | ❌ |
| F28 | Cost tracking & budgets | Per-session and per-project token cost tracking with configurable spending limits | ❌ |
| F29 | Model routing | Automatic model selection based on task complexity — use cheaper models for simple tasks, premium for complex | ❌ |

---

*This specification is a living document. It will evolve as implementation progresses and requirements are refined.*
