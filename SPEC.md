# ragent — Specification

**An open-source AI coding agent built for the terminal, implemented in Rust.**

ragent is a Rust reimplementation of [OpenCode](https://github.com/anomalyco/opencode) — the open-source AI coding agent. It provides the same core capabilities (multi-provider LLM orchestration, tool execution, TUI, client/server architecture, MCP support, LSP integration) rewritten from TypeScript/Bun into idiomatic, high-performance Rust.

---

## Table of Contents

1. [Goals & Non-Goals](#1-goals--non-goals)
2. [Architecture Overview](#2-architecture-overview)
3. [Core Modules](#3-core-modules)
   - 3.1 [CLI & Entry Point](#31-cli--entry-point)
   - 3.2 [Configuration](#32-configuration)
   - 3.3 [Provider System](#33-provider-system)
   - 3.4 [Agent System](#34-agent-system)
   - 3.5 [Session Management](#35-session-management)
   - 3.6 [Message Model](#36-message-model)
   - 3.7 [Tool System](#37-tool-system)
   - 3.8 [Permission System](#38-permission-system)
   - 3.9 [HTTP Server](#39-http-server)
   - 3.10 [Terminal UI (TUI)](#310-terminal-ui-tui)
   - 3.11 [MCP Client](#311-mcp-client)
   - 3.12 [LSP Integration](#312-lsp-integration)
   - 3.13 [Event Bus](#313-event-bus)
   - 3.14 [Storage & Database](#314-storage--database)
   - 3.15 [Shell Execution](#315-shell-execution)
   - 3.16 [Snapshot & Undo](#316-snapshot--undo)
   - 3.17 [Hooks](#317-hooks)
   - 3.18 [Custom Agents](#318-custom-agents)
   - 3.19 [Skills](#319-skills)
   - 3.20 [Persistent Memory](#320-persistent-memory)
   - 3.21 [Trusted Directories](#321-trusted-directories)
   - 3.22 [Codebase Indexing & Semantic Search](#322-codebase-indexing--semantic-search)
   - 3.23 [Post-Edit Diagnostics](#323-post-edit-diagnostics)
   - 3.24 [Task Todo List](#324-task-todo-list)
   - 3.25 [Prompt Enhancement](#325-prompt-enhancement)
   - 3.26 [Hierarchical Custom Instructions](#326-hierarchical-custom-instructions)
   - 3.27 [File Ignore Patterns](#327-file-ignore-patterns)
   - 3.28 [Suggested Responses](#328-suggested-responses)
4. [Data Flow](#4-data-flow)
5. [Configuration File Format](#5-configuration-file-format)
6. [Rust Crate Map](#6-rust-crate-map)
7. [Project Layout](#7-project-layout)
8. [Build & Distribution](#8-build--distribution)
9. [Testing Strategy](#9-testing-strategy)
10. [Future / Stretch Goals](#10-future--stretch-goals)

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

### 3.1 CLI & Entry Point

| Aspect | Detail |
|--------|--------|
| Crate | `clap` (derive) |
| Binary name | `ragent` |
| Entry | `src/main.rs` → `src/cli/mod.rs` |

#### Subcommands

| Command | Description |
|---------|-------------|
| *(default)* | Launch interactive TUI session |
| `run <prompt>` | Execute a one-shot agent run, print result, exit |
| `serve` | Start HTTP/WebSocket server only (headless) |
| `session list` | List saved sessions |
| `session resume <id>` | Resume a previous session |
| `session export <id>` | Export session to JSON |
| `session import <file>` | Import session from JSON |
| `auth <provider>` | Configure API key for a provider |
| `models` | List available models across configured providers |
| `config` | Print resolved configuration |
| `mcp list` | List configured MCP servers and their status |
| `upgrade` | Self-update the binary |
| `uninstall` | Remove ragent and its data |

#### Global Flags

| Flag | Default | Description |
|------|---------|-------------|
| `--config <path>` | auto-detected | Path to config file |
| `--model <provider/model>` | from config | Override model for this run |
| `--agent <name>` | `build` | Override default agent |
| `-p`, `--prompt <text>` | n/a | Execute a single prompt programmatically, print result, and exit |
| `--log-level <level>` | `warn` | Logging verbosity (`trace`, `debug`, `info`, `warn`, `error`) |
| `--print-logs` | `false` | Print logs to stderr |
| `--no-tui` | `false` | Disable TUI, use plain stdout |
| `--yes` | `false` | Auto-approve all permission prompts |
| `--allow-all-tools` | `false` | Allow all tools without manual approval |
| `--allow-tool <spec>` | n/a | Allow a specific tool without approval (repeatable). Spec: `'shell(cmd)'`, `'write'`, or `'McpServer(tool)'` |
| `--deny-tool <spec>` | n/a | Deny a specific tool (repeatable, overrides `--allow-tool` and `--allow-all-tools`) |
| `--server <addr>` | n/a | Connect to an existing ragent server |

---

### 3.2 Configuration

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

### 3.3 Provider System

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

### 3.4 Agent System

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

| Name | Mode | Description | Key Permission Traits |
|------|------|-------------|----------------------|
| `ask` | Primary | Quick Q&A — answers questions without tools | Read-only; max 1 step |
| `general` | Primary | General-purpose coding agent; full read/write/execute access (default) | Allows all tools; denies editing `.env*` files |
| `build` | Subagent | Build/test agent; compile, run tests, fix errors | Full access; max 30 steps |
| `plan` | Subagent | Read-only analysis & planning agent | Denies all edit/write tools; asks before bash |
| `explore` | Subagent | Fast codebase search (invoked via `@explore`) | Read-only: grep, glob, list, read, bash, web |
| `title` | Internal | Generates session titles | Hidden, no tools |
| `summary` | Internal | Generates session summaries | Hidden, no tools |
| `compaction` | Internal | Compresses long conversation history | Hidden, no tools |
| `orchestrator` | Primary | Task orchestrator — decomposes complex work into subtasks and delegates to specialized agents | Read-only; delegates via `new_task` tool |
| `debug` | Primary | Systematic debugger — methodical problem diagnosis and resolution | Full access; diagnostic-focused prompt |

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

### 3.5 Session Management

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

### 3.6 Message Model

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

### 3.7 Tool System

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

**Implemented:**

| Tool | Permission | Description | Status |
|------|-----------|-------------|--------|
| `read` | `file:read` | Read file contents (with optional line range) | ✅ |
| `write` | `file:write` | Create or overwrite a file | ✅ |
| `create` | `file:write` | Create a new file, truncating if it already exists | ✅ |
| `edit` | `file:write` | Replace a specific string in a file | ✅ |
| `multiedit` | `file:write` | Apply multiple edits to one or more files atomically | ✅ |
| `bash` | `bash:execute` | Execute a shell command and capture output | ✅ |
| `grep` | `file:read` | Search file contents using string matching | ✅ |
| `glob` | `file:read` | Find files matching glob patterns | ✅ |
| `list` | `file:read` | List directory contents (with depth control) | ✅ |
| `question` | `question` | Ask the user a question and wait for a response | ✅ |
| `office_read` | `file:read` | Read content from Word, Excel, or PowerPoint files | ✅ |
| `office_write` | `file:write` | Write content to Word, Excel, or PowerPoint files | ✅ |
| `office_info` | `file:read` | Get metadata about Office documents | ✅ |
| `pdf_read` | `file:read` | Read text and metadata from PDF files | ✅ |
| `pdf_write` | `file:write` | Create PDF files from structured content | ✅ |
| `patch` | `file:write` | Apply a unified diff patch to one or more files | ✅ |
| `webfetch` | `web` | Fetch URL content with HTML-to-text conversion | ✅ |

**Planned (not yet implemented):**

| Tool | Permission | Description | Status |
|------|-----------|-------------|--------|
| `websearch` | `web` | Perform a web search and return results | ✅ |
| `plan_enter` | `plan` | Switch the active agent to the plan agent | ✅ |
| `plan_exit` | `plan` | Switch back from plan agent to the previous agent | ✅ |
| `todo_read` | `todo` | Read the current TODO list | ✅ |
| `todo_write` | `todo` | Update the TODO list | ✅ |
| `new_task` | `workflow` | Create a subtask delegated to a specific agent with isolated context | 🔲 |
| `switch_agent` | `workflow` | Switch the active agent for the current session | 🔲 |
| `codebase_search` | `file:read` | Semantic search across indexed codebase using embeddings | 🔲 |
| `generate_image` | `image` | Generate images from text prompts using AI image models | 🔲 |

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

---

### 3.8 Permission System

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

---

### 3.9 HTTP Server

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
| `GET` | `/mcp` | List MCP servers |
| `POST` | `/mcp/:id/restart` | Restart MCP server |
| `GET` | `/events` | Global SSE event stream |

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
```

---

### 3.10 Terminal UI (TUI)

| Aspect | Detail |
|--------|--------|
| Crate | `ratatui` + `crossterm` |
| Layout | Home screen on launch, transitions to chat on first message |

#### Home Screen

On startup ragent displays a centered landing page:

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

```
┌───────────────────────────────────────────────────────────────┐
│ ● ragent  session: abc123  agent: general  [ready]  ● Ollama (Local) / qwen3:latest │
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
│                                        tokens:$ │     (multi-line)
└─────────────────────────────────────────────────┘
```

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

#### Auto-expanding Input

The input widget on both the home screen and the chat screen automatically expands vertically as the user types. Text wraps within the input borders, and the cursor position accounts for wrapped lines. The input height is computed dynamically based on the text length and the available inner width.

#### Slash Commands

Typing `/` in the input area on either the home screen or the chat screen
opens an autocomplete dropdown above the input. The list filters as you type,
and you can navigate with `↑`/`↓` arrow keys and select with `Enter`.
Press `Esc` to dismiss the menu.

| Command | Description |
|---------|-------------|
| `/agent [name]` | Switch the active agent — opens selection dialog if no name given, or switches directly to the named agent |
| `/checkpoint [diff|restore]` | View checkpoint diff or restore workspace to a previous checkpoint |
| `/clear` | Clear message history for the current session |
| `/compact` | Summarise and compact the conversation history |
| `/context` | Show detailed token usage breakdown (input, output, cached, total, limit, percentage used) |
| `/help` | Show available slash commands |
| `/log` | Toggle the log panel on/off |
| `/model` | Switch the active model on the current provider |
| `/provider` | Change the LLM provider (re-enters full setup flow) |
| `/provider_reset` | Reset a provider — prompts for selection, clears stored credentials and disables auto-detection |
| `/quit` | Exit ragent |
| `/system <prompt>` | Override the agent system prompt for the current session |
| `/todo` | Display the current task todo list with status indicators |
| `/tools` | List all available tools (built-in and MCP) with parameters |

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

### 3.11 MCP Client

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

### 3.12 LSP Integration

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

### 3.13 Event Bus

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

### 3.14 Storage & Database

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

### 3.15 Shell Execution

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

### 3.16 Snapshot & Undo

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

### 3.17 Hooks

Hooks allow users to execute custom shell commands at key points during agent execution, enabling validation, logging, security scanning, or workflow automation.

#### Hook Types

| Hook | Trigger Point | Use Cases |
|------|---------------|-----------|
| `pre-tool` | Before any tool executes | Validation, logging, security scanning |
| `post-tool` | After a tool completes | Audit logging, cleanup, notification |
| `pre-message` | Before sending a message to the LLM | Prompt injection detection, content filtering |
| `post-message` | After receiving an LLM response | Response validation, metrics collection |
| `session-start` | When a new session begins | Environment setup, dependency checks |
| `session-end` | When a session completes | Cleanup, summary generation |

#### Configuration

Hooks are defined in `.ragent/hooks/` or in the project config:

```jsonc
{
  "hooks": {
    "pre-tool": {
      "command": "./scripts/validate-tool.sh",
      "timeout": 10,
      "tools": ["bash", "write", "edit"]   // optional: only run for these tools
    },
    "post-tool": {
      "command": "./scripts/audit-log.sh",
      "timeout": 5
    }
  }
}
```

Hook commands receive tool name, arguments, and context via environment variables. A non-zero exit code from a `pre-*` hook aborts the operation.

---

### 3.18 Custom Agents

Users can define custom specialized agents beyond the built-in presets. Custom agents allow tailoring the agent's system prompt, available tools, and permissions for specific tasks or team roles.

#### Configuration

Custom agents are defined in `.ragent/agents/` as YAML or JSON files, or in the project config:

```jsonc
{
  "agents": {
    "frontend-expert": {
      "description": "Frontend specialist following team guidelines",
      "prompt": "You are an expert frontend engineer. Follow React best practices...",
      "tools": ["read", "write", "edit", "bash", "grep", "glob", "list"],
      "permissions": {
        "file:write": { "glob": "src/components/**", "rule": "Allow" }
      }
    }
  }
}
```

Custom agents appear in the agent picker (`/agent`) and can be selected via `Tab`/`Shift+Tab` cycling. The CLI automatically delegates common tasks to specialized agents when appropriate.

---

### 3.19 Skills

Skills enhance the agent's ability to perform specialized tasks by bundling instructions, scripts, and resources into reusable packages.

#### Skill Structure

```
.ragent/skills/
  deploy/
    skill.json          # Skill metadata and instructions
    scripts/            # Helper scripts the skill can invoke
    resources/          # Reference materials, templates, examples
```

#### Skill Definition

```jsonc
{
  "name": "deploy",
  "description": "Deploy the application to production",
  "instructions": "Follow the deployment checklist: run tests, build release, deploy to staging, verify, promote to production.",
  "tools": ["bash"],
  "scripts": {
    "deploy": "./scripts/deploy.sh",
    "rollback": "./scripts/rollback.sh"
  }
}
```

Skills are automatically loaded from `.ragent/skills/` and their instructions are injected into the system prompt when the agent invokes them.

---

### 3.20 Persistent Memory

Persistent memory allows ragent to build a lasting understanding of the project across sessions. Memories are facts about coding conventions, patterns, preferences, and project structure that the agent learns over time.

#### Memory Types

| Type | Description | Example |
|------|-------------|---------|
| Convention | Coding style preferences | "Use 4-space indentation in Rust files" |
| Pattern | Recurring code patterns | "Error handling uses `anyhow::Result` with `.context()`" |
| Preference | User preferences | "Prefer `tokio::fs` over `std::fs` for async file operations" |
| Structure | Project layout knowledge | "Tests live in `tests/` directory per crate, not inline" |

#### Storage

Memories are stored in the SQLite database (`memories` table) with:
- `id` — unique identifier
- `category` — convention, pattern, preference, structure
- `content` — the memory text
- `source` — file or conversation that produced it
- `created_at` — when the memory was recorded

#### Usage

- Memories are loaded at session start and injected into the system prompt
- The agent can create new memories when it discovers patterns via a `memory_write` tool
- Users can review and manage memories via `/memory` slash command
- Memories persist across sessions and reduce the need to repeat context

---

### 3.21 Trusted Directories

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

### 3.22 Codebase Indexing & Semantic Search

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

### 3.23 Post-Edit Diagnostics

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

### 3.24 Task Todo List

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

### 3.25 Prompt Enhancement

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

### 3.26 Hierarchical Custom Instructions

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

#### System Prompt Assembly

Instructions are injected into the system prompt in this order:
1. Agent role definition
2. Global rules
3. Global agent-specific rules
4. Project rules
5. Project agent-specific rules
6. AGENTS.md content
7. Config custom instructions
8. Tool definitions

---

### 3.27 File Ignore Patterns

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

### 3.28 Suggested Responses

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

| # | Feature | Description |
|---|---------|-------------|
| F1 | Web UI | SolidJS-based web frontend connecting to the ragent server |
| F2 | Mobile client | Remote control ragent from a phone via the HTTP API |
| F3 | Plugin system | WASM-based plugin execution for custom tools |
| F4 | Git worktree isolation | Run each session in a separate git worktree for parallel work |
| F5 | OpenTelemetry | Trace spans for LLM calls, tool execution, and session lifecycle |
| F6 | Multi-agent orchestration | Multiple agents collaborating on a single task |
| F7 | Code generation benchmarks | Automated evaluation harness for measuring agent quality |
| F8 | Enterprise features | Managed config, audit logging, SSO |
| F9 | Voice input | Microphone input transcribed to text for hands-free coding |
| F10 | Image/screenshot input | Vision model support for UI debugging |
| F11 | ACP (Agent Client Protocol) | Support the open standard Agent Client Protocol for interoperability with other AI agent ecosystems |
| F12 | `/feedback` command | Built-in user feedback submission mechanism |
| F13 | Sub-agent spawning | Launch specialized sub-agents (e.g., explore, code-review) from within a session for focused tasks |
| F14 | Background agents | Run multiple agent instances concurrently for parallel task execution |
| F15 | Marketplace | Community hub for sharing and discovering custom agents, skills, and rule sets |
| F16 | API configuration profiles | Named profiles for different API providers/models, switchable per agent or session |
| F17 | Concurrent file operations | Parallel file reads and edits for faster multi-file workflows |
| F18 | Model temperature control | Per-session or per-agent temperature override exposed in TUI settings |
| F19 | Agent import/export | Export agent definitions (including rules) to portable YAML/JSON for team sharing |
| F20 | Custom tools (user-defined) | Define project-specific tools in a scripting language that ragent can invoke |

---

*This specification is a living document. It will evolve as implementation progresses and requirements are refined.*
