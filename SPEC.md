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
| `--log-level <level>` | `warn` | Logging verbosity (`trace`, `debug`, `info`, `warn`, `error`) |
| `--print-logs` | `false` | Print logs to stderr |
| `--no-tui` | `false` | Disable TUI, use plain stdout |
| `--yes` | `false` | Auto-approve all permission prompts |
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

Agents can be switched at runtime using the `/agent` slash command or by cycling with `Tab`/`Shift+Tab`.

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
| `edit` | `file:write` | Replace a specific string in a file | ✅ |
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

**Planned (not yet implemented):**

| Tool | Permission | Description | Status |
|------|-----------|-------------|--------|
| `multiedit` | `file:write` | Apply multiple edits to one or more files | 🔲 |
| `patch` | `file:write` | Apply a unified diff patch | 🔲 |
| `webfetch` | `web` | Fetch a URL and return its content | 🔲 |
| `websearch` | `web` | Perform a web search and return results | 🔲 |
| `plan_enter` | `plan` | Switch the active agent to the plan agent | 🔲 |
| `plan_exit` | `plan` | Switch back from plan agent to the previous agent | 🔲 |
| `todo_read` | `todo` | Read the current TODO list | 🔲 |
| `todo_write` | `todo` | Update the TODO list | 🔲 |

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
| `edit` | `write`, `edit`, `multiedit`, `patch` tools |
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
| `/clear` | Clear message history for the current session |
| `/compact` | Summarise and compact the conversation history |
| `/help` | Show available slash commands |
| `/log` | Toggle the log panel on/off |
| `/model` | Switch the active model on the current provider |
| `/provider` | Change the LLM provider (re-enters full setup flow) |
| `/provider_reset` | Reset a provider — prompts for selection, clears stored credentials and disables auto-detection |
| `/quit` | Exit ragent |
| `/system <prompt>` | Override the agent system prompt for the current session |
| `/tools` | List all available tools (built-in and MCP) |

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

#### Undo Granularity

| Level | Description |
|-------|-------------|
| Per-tool-call | Revert a single tool call's changes |
| Per-message | Revert all changes from one assistant message |
| Per-session | Revert all changes from the entire session |

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

---

*This specification is a living document. It will evolve as implementation progresses and requirements are refined.*
