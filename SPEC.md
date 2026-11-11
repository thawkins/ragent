<div style="page-break-after: always; text-align: center; padding-top: 15em;">

<h1 style="font-size: 3em; margin-bottom: 0.2em;">ragent</h1>
<h2 style="font-size: 1.5em; font-weight: normal; color: #555; margin-top: 0;">Technical Specification</h2>        <p style="margin-top: 4em; font-size: 1.1em;">        <strong>Version:</strong> 1.0.168</p>
        <p style="font-size: 1.1em;">
          <strong>Date:</strong> 2026-11-09
      </p>
  <p style="font-size: 1.1em;">
    <strong>Author:</strong> Tim Hawkins &lt;tim.thawkins@gmail.com&gt;
  </p>
<p style="margin-top: 6em; font-size: 0.9em; color: #777;">
    <a href="https://github.com/thawkins/ragent">github.com/thawkins/ragent</a>
  </p>

</div>
---

## Executive Summary

Ragent is an open-source AI coding agent for the terminal, written entirely in
Rust and distributed as a single statically-linked binary with zero external
runtime dependencies. It orchestrates multiple LLM providers — Anthropic,
OpenAI, GitHub Copilot, Google Gemini, Hugging Face, Ollama (local and cloud), xAI
Grok, Generic OpenAI-compatible endpoints, Azure AI Foundry, Azure Resource (File),
Amazon Bedrock, and a Model Router provider — behind a unified streaming interface,
giving developers a powerful,
provider-agnostic assistant that runs wherever a terminal does.

### What It Does

Ragent bridges the gap between conversational AI and hands-on software
engineering. An agent can read and write files, execute shell commands, search
codebases, manage Git and GitHub workflows, query language servers, read PDF
documents, and coordinate with other agents — all through a
large built-in tool library organised across multiple categories. Every tool
invocation passes through a multi-layered security and permission system that
gives the user full control over what the agent can and cannot do.

### How It Works

At its core, ragent follows a **session → agent → tool** loop. A session
processor manages the conversation with the LLM provider, the agent system
defines personality and capabilities via profiles, and the tool registry
dispatches execution requests. An asynchronous event bus (built on tokio)
connects all components, enabling real-time streaming of tokens, tool results,
and status updates to both the TUI and the HTTP API.

```mermaid
graph LR
    User[User] --> TUI[TUI / HTTP API]
    TUI --> SP[Session Processor]
    SP --> LLM[LLM Provider]
    SP --> AP[Agent Profile]
    SP --> TR[Tool Registry]
          TR --> Tools[File ops, Bash, GitHub,<br/>code index, memory, teams,<br/>PDF, web, ...]

```

**Figure 1:** Core Execution Loop — High-level session/agent/tool flow

### Key Capabilities

| Capability | Summary |
|-----------|---------|
| **Multi-provider LLM** | Every provider registered in `ragent-llm` (`crates/ragent-llm/src/providers/`), with automatic model discovery, health monitoring, streaming, vision, and reasoning levels |
| **Terminal UI** | Full-screen ratatui interface with streaming markdown, syntax highlighting, slash commands, and image support |
| **HTTP Server** | REST + SSE API (Axum) for headless operation and external integrations |
| **Tool System** | Broad tool coverage across file ops, shell, search, GitHub, GitLab, code index, memory, teams, sub-agents, PDF, web, and MCP |
| **Code Intelligence** | Tree-sitter parsing (15+ languages), Tantivy FTS, symbol/reference search, and code index queries |
| **Persistent Memory** | Two-tier system — file blocks and a structured SQLite store with FTS5 recall — with automatic extraction, decay, compaction, and a knowledge graph |
| **Teams & Swarms** | Multi-agent coordination with named teammates, shared task lists, mailbox messaging, and swarm decomposition for parallel work |
| **Security** | Permission rules (allow/deny/ask), 7-layer bash safety, file-path guards, secret redaction, resource limits, and YOLO mode for trusted environments |
| **Skills** | Loadable skill packs (bundled or custom YAML) that inject tools, prompts, and file context into agent sessions |
| **Custom Agents** | OASF-based agent profiles with configurable models, tools, permissions, and personality |
| **Autopilot** | Autonomous operation mode with configurable iteration limits and permission auto-approval |

### Who It's For

Ragent is designed for software developers and teams who want an AI assistant
that lives in their terminal, respects their security boundaries, and learns
from their workflow over time. It is equally suited to interactive pair-programming
sessions and headless CI/CD integration via its HTTP API.

### Technology

| Aspect | Detail |
|--------|--------|
| **Language** | Rust (edition 2024) |
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

Ragent is in **beta**. The core architecture, tool system,
TUI, HTTP server, memory system, spec management, skills system, research system,
multi-agent coordination, security layer, telemetry, code index semantic graph,
plugin system, connector system (§19C), execution backends (§19D), the Agent
Client Protocol client and server (§19E), the automation service (§19F), and
release packaging are functional and under active development. The specification
below documents the current state of all subsystems.

---


## Table of Contents

- [Executive Summary](#executive-summary)

### Part I: Foundation & Basics

- [1. Overview](#1-overview)
- [2. Architecture](#2-architecture)
- [3. Core Features](#3-core-features)
- [4. Security & Permissions](#4-security--permissions)
- [5. Configuration](#5-configuration)

### Part II: User Interface & API

- [6. Terminal User Interface (TUI)](#6-terminal-user-interface-tui)
- [7. HTTP Server & API](#7-http-server--api)

### Part III: Data & Knowledge Systems

- [8. Code Index](#8-code-index)
- [9. Memory System](#9-memory-system)
- [10. Spec Management](#10-spec-management)
- [11. Research System](#11-research-system)
- [11A. Project Scaffolding (`/new`)](#11a-project-scaffolding-new)

### Part IV: Agent Customization & Extension

- [11. Custom Agents](#11-custom-agents)
- [12. Skills System](#12-skills-system)

### Part V: Multi-Agent Coordination

- [14. Teams](#14-teams)
- [15. Swarm Mode](#15-swarm-mode)
- [16. Autopilot Mode](#16-autopilot-mode)
- [16A. Cron Scheduling](#16a-cron-scheduling)
- [17. Orchestrator & Multi-Agent Coordination](#17-orchestrator--multi-agent-coordination)

### Part VI: External Integrations

- [18. GitHub & GitLab Integration](#18-github--gitlab-integration)
- [19. MCP Integration (Model Context Protocol)](#19-mcp-integration-model-context-protocol)
- [19B. Durable Initiatives & Skill Management](#19b-durable-initiatives--skill-management)
- [19C. Connector System (Claude-Connector-Equivalent Integrations over MCP)](#19c-connector-system-claude-connector-equivalent-integrations-over-mcp)
- [19D. Execution Backends (`local` / `docker` / `podman` / `remote`)](#19d-execution-backends-local--docker--podman--remote)
- [19E. Agent Client Protocol (`ragent-agent/src/acp/`)](#19e-agent-client-protocol-ragent-agentsrcacp)
- [19F. Automation Service (`ragent-agent/src/automation/`)](#19f-automation-service-ragent-agentsrcautomation)

### Part VII: Operations & Reference

- [20. Auto-Update Mechanism](#20-auto-update-mechanism)
- [21. Harness Enhancements](#21-harness-enhancements)

**Appendices**

- [Appendix B: Documentation](#appendix-b-documentation)
- [Appendix C: Project Contact & Repository](#appendix-c-project-contact--repository)
- [Research Configuration](#research-configuration)

### List of Diagrams

| # | Diagram | Section | Description |
|---|---------|---------|-------------|
| 1 | [Core Execution Loop](#how-it-works) | Executive Summary | High-level session/agent/tool flow |
| 2 | [System Architecture](#2-architecture) | Architecture | Full crate and component topology |
| 3 | [Crate Dependency Graph](#22-crate-dependency-graph) | Workspace Crates | Inter-crate dependency relationships |
| 4 | [Event Bus Flow](#23-event-bus-flow) | Architecture | Internal pub/sub message routing |
| 5 | [Session & Tool Execution Flow](#35-session--tool-execution-flow) | Core Features | LLM call → permission → tool dispatch loop |
| 6 | [Provider Selection Flow](#36-provider-selection-flow) | Core Features | Multi-provider routing and health checks |
| 7 | [TUI Component Architecture](#63-tui-component-architecture) | Terminal User Interface | UI layout and event wiring |
| 8 | [HTTP API Request Flow](#75-http-api-request-flow) | HTTP Server & API | REST + SSE lifecycle |
| 9 | [Code Index Pipeline](#83-architecture) | Code Index | File scan → parse → index → search |
| 10 | [Permission Security Layers](#41-permission-security-layers) | Security & Permissions | 5-layer defense-in-depth |
| 11 | [Bash Security — 7 Layers](#42-bash-security--7-layers) | Security & Permissions | Bash command defense flow |
| 12 | [Permission Request Flow](#43-permission-request-flow) | Security & Permissions | From tool call to user decision |
| 13 | [Permission Rules Evaluation](#44-permission-rules-evaluation) | Security & Permissions | Rule matching and resolution |
| 14 | [Agent Execution Loop Phases](#37-agent-execution-loop-phases) | Core Features | One complete turn from input to response |

---



---

# Part I: Foundation & Basics

---

## 1. Overview

Ragent is an AI coding agent for the terminal, built in Rust. It provides multi-provider LLM orchestration, a built-in tool system, terminal UI, and client/server architecture — all compiled into a single statically-linked binary.

### 1.1 Key Characteristics

| Characteristic | Description |
|----------------|-------------|
| **Single binary** | Statically linked, zero runtime dependencies beyond OS libraries |
| **Multi-provider** | 13 first-class LLM provider IDs with auto-discovery and health checks |
| **Tool-rich** | A tool registry grouped into permission categories; `ragent --dry-run --json run x` reports the live total (`mf_screenshot` is engine-gated, spec `openhands` FR-029) |
| **Local-first** | SQLite, Tantivy, and tree-sitter compiled in; no external services required |
| **Streaming** | Real-time token, tool, and event streaming via TUI and HTTP SSE |
| **Extensible** | Custom agents, skills, MCP servers, and provider modules |
| **Secure by default** | Permission system, bash safety, file guards, secret redaction |

### 1.2 Terminology

| Term | Meaning |
|------|---------|
| **Session** | A single conversation/run with an agent, persisted to SQLite |
| **Agent Profile** | A named configuration of model, system prompt, tools, and permissions |
| **Tool** | A typed, invokable capability (file ops, shell, search, etc.) |
| **Event Bus** | Tokio broadcast channel used for cross-component real-time updates |
| **Code Index** | Tree-sitter + Tantivy index of the project codebase |
| **Memory** | Two-tier persistence: file blocks and structured SQLite (FTS5 recall) |
| **Skill** | A YAML pack that injects tools, prompts, and file context |
| **Team** | A named multi-agent workspace with shared tasks and mailbox messaging |
| **Swarm** | Parallel task decomposition across spawned agents |
| **Autopilot** | Autonomous mode with auto-approval and iteration limits |
| **Initiative** | Durable project-scoped goal with milestones |
| **Telemetry** | OpenTelemetry metrics and log export for operator observability |

---

## 2. Architecture

Ragent is organised as a Cargo workspace of focused crates. Each crate owns a
single concern, and the top-level `src/main.rs` wires them together behind a clap
CLI.

```mermaid
graph TB
    subgraph "Workspace Crates"
        A[ragent-agent]
        B[ragent-bench]
        C[ragent-codeindex]
        D[ragent-config]
        E[ragent-llm]
        G[ragent-server]
        H[ragent-specs]
        I[ragent-storage]
        K[ragent-tools-core]
        L[ragent-tools-extended]
        M[ragent-tools-vcs]
        N[ragent-tui]
        O[ragent-types]
        P[ragent-research]
        S[ragent-surface]
    end

    Q[src/main.rs CLI entry]
    Q --> A
    Q --> N
    Q --> G
    A --> O
    A --> D
    A --> E
    A --> K
    A --> L
    A --> J
    A --> H
    A --> P
    N --> A
    N --> O
    G --> A
    G --> O
    J --> O
    K --> O
    L --> O
    M --> O
    C --> O
    I --> O
    E --> O
    D --> O
    B --> O
    H --> O
    P --> O
```

**Figure 2:** System Architecture — Full crate and component topology

### 2.1 Workspace Crates

| Crate | Responsibility | Approx. `src/` Lines |
|-------|----------------|---------------|
| `ragent-types` | Shared IDs, events, messages, errors, sanitisation | ~8,000 |
| `ragent-config` | Configuration loading, defaults, permission rules | ~8,900 |
| `ragent-connectors` | Connector descriptors, store/ledger, catalogue providers, MCP bridge, `/connectors` surface | ~10,500 |
| `ragent-storage` | SQLite persistence, snapshots, encrypted credentials | ~7,700 |
| `ragent-llm` | Provider clients and model/provider registry | ~18,800 |
| `ragent-tools-core` | File, shell, search, and utility tools | ~12,300 |
| `ragent-tools-extended` | Memory, code index, PDF, web tools | ~29,900 |
| `ragent-tools-vcs` | GitHub and GitLab tool surface | ~9,000 |
| `ragent-agent` | Session processor, agent resolution, tool registry, memory, MCP; team runtime + the `team_*` tools (former `ragent-team` shim folded in) | ~77,300 |
| `ragent-codeindex` | Tree-sitter parsing, SQLite/Tantivy index, file watcher | ~14,200 |
| `ragent-server` | Axum HTTP routes and SSE streaming | ~6,100 |
| `ragent-tui` | Ratatui terminal interface | ~72,300 |
| `ragent-bench` | Criterion benchmarks shared between TUI and CLI | ~6,800 |
| `ragent-specs` | Spec lifecycle management, SDD artifact generation, consistency validation, constitution parsing | ~9,100 |
| `ragent-research` | Research types, gatherers, and plan-dep parser | ~39,000 |
| `ragent-plugins` | Plugin discovery, dialect manifests, sandboxed JS runtime, lifecycle, skills/MCP/commands/agents/hooks bridges, `/plugins` surface | ~11,400 |
| `ragent-surface` | Shared surface helpers for the plugin/connector families: attribution, subcommand tokeniser, store-dir resolution, harness step model, schema samples | ~1,200 |
| `ragent-telemetry` | OpenTelemetry metrics export subsystem | ~6,400 |
| `ragent-hyperresearch` | Deep-research engine: persistent vault, fetch/scholarly providers, claims, graph, MCP and HTTP surfaces | ~1,200 |

### 2.2 Crate Dependency Graph

```mermaid
graph TD
    T[ragent-types]
    D[ragent-config]
    S[ragent-storage]
    L[ragent-llm]
    TC[ragent-tools-core]
    TE[ragent-tools-extended]
    TV[ragent-tools-vcs]
    A[ragent-agent]
    CI[ragent-codeindex]
    SV[ragent-server]
    TU[ragent-tui]
    SP[ragent-specs]
    RS[ragent-research]
    B[ragent-bench]
    PL[ragent-plugins]
    CN[ragent-connectors]
    SU[ragent-surface]

    T --> D
    T --> S
    T --> L
    T --> TC
    T --> TE
    T --> TV
    T --> CI
    T --> SP
    T --> RS
    T --> CN
    D --> L
    D --> A
    D --> TU
    D --> CN
    S --> A
    S --> TU
    L --> A
    L --> TU
    L --> SV
    TC --> A
    TE --> A
    TV --> A
    A --> TU
    A --> SV
    CI --> A
    SP --> A
    RS --> A
    B --> TU
    B --> SV
    PL --> A
    PL --> TU
    PL --> SU
    CN --> TU
    CN --> SU
```

**Figure 3:** Crate Dependency Graph — Inter-crate dependency relationships

Per-crate `fn`-definition counts (lexical lower bounds, re-measured 2026-10-21):
`ragent-tui` 6,054 · `ragent-agent` 4,630 · `ragent-tools-extended` 2,956 ·
`ragent-research` 2,513 · `ragent-codeindex` 1,130 · `ragent-llm` 1,102 ·
`ragent-tools-core` 1,001 · `ragent-plugins` 991 · `ragent-specs` 981 ·
`ragent-tools-vcs` 941 · `ragent-connectors` 907 · `ragent-config` 787 ·
`ragent-types` 590 · `ragent-storage` 585 · `ragent-telemetry` 550 ·
`ragent-bench` 412 · `ragent-server` 363 · root `ragent` binary 159 ·
`ragent-surface` 114 — 26,768 workspace-wide (8,985 in `src/`, 17,667 in
`tests/`, 66% test code).

### 2.3 Event Bus Flow

The event bus is a tokio broadcast channel (`tokio::sync::broadcast`). Every
component can publish and subscribe to strongly-typed events.

```mermaid
graph LR
    SP[Session Processor] --> EB[Event Bus]
    TUI[TUI] --> EB
    HTTP[HTTP Server] --> EB
    Tools[Tool Executors] --> EB
    EB --> SP
    EB --> TUI
    EB --> HTTP
    EB --> Storage[Storage / Memory]
    EB --> Teams[Team Manager]
```

**Figure 4:** Event Bus Flow — Internal pub/sub message routing

Key event types include:

| Event | Purpose |
|-------|---------|
| `AssistantChunk` | Streaming LLM token or reasoning block |
| `ToolRequested` / `ToolExecuted` | Tool call lifecycle |
| `PermissionRequested` / `PermissionReplied` | Permission dialog flow |
| `QuestionRequested` / `QuestionAnswered` | User question tool |
| `AgentNotice` | Non-blocking status/information message |
| `CompressionStarted` / `CompressionFinished` | Context compaction lifecycle (auto pre-send summarisation and emergency overflow compaction) |
| `TeammateIdle` / `TeammateFailed` / `TeammateResumed` | Team coordination |
| `SubagentSpawned` / `SubagentCompleted` / `SubagentKilled` | Background agents |
| `SubagentSuspended` / `SubagentResumed` | Cooperative suspension of a background sub-agent (spec `agentui` FR-035) |
| `RouterClassification` / `RouterTierSelected` | Model-router routing decisions |
| `RunCostSummary` | Per-run token/cost summary emitted at session run end |
| `FromFileBodyPreview` | Preview of text extracted from a `--from-file` research seed |
| `SynthesizeResult` | Research LLM synthesis outcome (success/fallback/error) |

---

## 3. Core Features

### 3.1 LLM Providers

Ragent supports multiple providers through a common trait interface. Providers
are registered in a runtime registry, and the session processor selects one
based on the active agent/model.

#### Supported Providers

| Provider | Local/Cloud | Discovery | Streaming | Vision | Tools | Notes |
|----------|-------------|-----------|-----------|--------|-------|-------|
| `anthropic` | Cloud | Static catalog | Yes | Yes | Yes | Claude models, thinking blocks |
| `openai` | Cloud | Static + dynamic | Yes | Yes | Yes | GPT-4o, o1, etc. |
| `copilot` | Cloud | Static | Yes | No | Yes | GitHub Copilot token-based |
| `gemini` | Cloud | Static + dynamic | Yes | Yes | Yes | Google Gemini, up to 2M context |
| `huggingface` | Cloud | Dynamic | Yes | Yes | Some | HF Inference API |
| `ollama` | Local | Dynamic | Yes | Yes | Yes | Local Ollama server |
| `ollama_cloud` | Cloud | Dynamic | Yes | Yes | Yes | Managed Ollama endpoints |
| `generic_openai` | Either | Config-driven | Yes | Yes | Yes | Any OpenAI-compatible endpoint |
| `azure_foundry` | Cloud | Dynamic | Yes | Yes | Yes | Microsoft Azure AI Foundry |
| `azure_resource` | Cloud | File-based | Yes | Yes | Yes | Reads `azureresources.json` |
| `xai` | Cloud | Static | Yes | Yes | Yes | xAI Grok API |
| `bedrock` | Cloud | Dynamic | Yes | Yes | Yes | AWS Bedrock, SigV4 signing |
| `openrouter` | Cloud | Dynamic | Yes | Yes | Yes | OpenAI-compatible model aggregator; single API key unlocks 100+ upstream models |
| `router` | Cloud | Static | Yes | Yes | Yes | Model Router with 15-dimension classifier |

(The registry exposes 14 provider IDs; the `router` is a virtual provider that
selects a downstream model based on request characteristics.)

#### Provider Features

| Feature | Description |
|---------|-------------|
| **Health checks** | `Provider::health()` returns `Healthy`, `Degraded`, or `Unhealthy` |
| **Model discovery** | `Provider::discover_models()` returns available models with capabilities |
| **Streaming** | All providers emit `StreamEvent::Chunk` and `StreamEvent::Done` |
| **Tool calling** | Providers that support function calling receive tool schemas |
| **Vision** | Image inputs passed when the model reports vision capability |
| **Reasoning levels** | Anthropic/OpenAI thinking blocks map to `low`/`medium`/`high` |
| **Model routing** | `router` provider classifies prompts and picks a downstream provider/model/tier |
| **OpenRouter integration** | `openrouter` provider discovers and streams from 100+ OpenAI-compatible models with a single API key |

#### Anthropic Models

| Model | Context | Notes |
|-------|---------|-------|
| `claude-sonnet-4-20250514` | 200k | Default reasoning model |
| `claude-opus-4-20250514` | 200k | High capability |
| `claude-haiku-4-20250514` | 200k | Fast/cheap |

#### OpenAI Models

| Model | Context | Notes |
|-------|---------|-------|
| `gpt-4o` | 128k | General purpose, vision |
| `o3-mini` | 200k | Reasoning |
| `o1` | 200k | High reasoning |

#### Ollama Cloud Provider

- Endpoint configured via `OLLAMA_CLOUD_HOST` or `ragent.json`
- Dynamic model discovery against `/api/tags`
- Supports vision models when the Ollama model reports `vision`

#### Ollama (Local) Provider

- Default endpoint `http://localhost:11434`
- Dynamic discovery; no API key required
- Vision support depends on the pulled model

#### Google Gemini Provider

- API key via `GEMINI_API_KEY`
- Massive context windows (up to 2M tokens)
- Discovery via Gemini models API

#### Hugging Face Provider

- Token via `HF_TOKEN` or `HUGGING_FACE_HUB_TOKEN`
- Public `/v1/models` discovery; falls back to static catalog on empty results
- Rate-limit tracking

#### Azure AI Foundry Provider

- API key via `AZURE_AI_FOUNDRY_API_KEY` (or `AZURE_AI_FOUNDRY_API_KEY`)
- Base URL via `AZURE_AI_FOUNDRY_BASE`
- OpenAI-compatible endpoints with `api-key` header
- Dynamic model discovery, tool calling, vision, reasoning levels
- HTTP 429 retries with `Retry-After` and exponential backoff

#### Azure Resource (File) Provider

- Reads endpoint definitions from `azureresources.json` in `~/.config/ragent/` or `.ragent/`
- Supports per-endpoint API keys, custom context windows, capability tags, thinking config
- Can route to Anthropic Messages (`/anthropic/v1/messages`) or OpenAI (`/openai/v1/chat/completions`) API shape via `api_type`

#### OpenRouter Provider

- API key via `OPENROUTER_API_KEY` or `ragent auth openrouter <key>`
- Single `GET /api/v1/models` discovery call returns all 100+ upstream models
- OpenAI-compatible `POST /api/v1/chat/completions` streaming with tool-call and reasoning deltas
- Per-model metadata (context window, pricing, vision/reasoning flags) mapped from discovery
- Supports vendor-slug model ids such as `openrouter/anthropic/claude-sonnet-4`

#### Unified Tool-Calling Wire Path (spec `uniformtools` FR-002..FR-004)

Every provider encodes tool definitions, tool results and streamed tool-call
deltas through one declared contract, so no provider module builds its own tool
JSON. A client holds a `ToolWireProfile` - nine required decisions, no `Default`
and no optional field - and dispatches on it:

| Field | Values | What it decides |
|-------|--------|-----------------|
| `tool_envelope` | `openai_function`, `anthropic_tool`, `gemini_function_declaration`, `bedrock_tool_spec`, `responses_flat_function` | the wrapper `wire::encode::encode_tools` puts a definition in |
| `result_carrier` | `tool_role_message`, `user_tool_result_block`, `gemini_function_response`, `responses_function_call_output` | where `encode_message` puts a tool result |
| `id_field` | a JSON key such as `tool_call_id`, or `synthesised` | which field a call id occupies |
| `args_encoding` | `json_string`, `json_object` | whether accumulated arguments are a string or an object |
| `stream_keying` | `by_index`, `by_call_id` | how `wire::decode::ToolCallAccumulator` attributes a delta |
| `supports_parallel` | `yes` / `no` | whether the step may dispatch concurrently (see the gate below) |
| `supports_is_error` | `yes` / `no` | native error flag, or the `ERROR: ` prefix |
| `echo_reasoning` | `yes` / `no` | whether a signed reasoning block is replayed |
| `synthetic_ids` | `yes` / `no` | whether the harness mints the call id |

The six families (`openai_chat`, `anthropic_messages`, `gemini`,
`bedrock_converse`, `openai_responses`, `ollama_native`) each have one `const`
profile, and `wire::profile_table::PROVIDER_WIRE` names the family of every
registered provider id; `profile_for` is the one lookup the encoders, the
accumulator, the parallel gate and the wire matrix share. There is no catch-all
row: an id with no declared contract fails the coverage assertion in
`crates/ragent-llm/tests/test_wire_profile_table.rs` at build time, so a
provider cannot join the build without stating its tool-calling terms.

`ragent --wire-matrix` prints that contract per registered provider as one
aligned ASCII table (ten columns - the provider id plus the nine fields, one row
per id, sorted, no blank or `-` cell) and exits without contacting a provider or
executing a tool; a registered id with no row is reported by name and exits
non-zero rather than printing a hole in the table.

### 3.2 Tool System

The tool system is the primary way agents interact with the world. Each tool
has a JSON schema, a permission category, and an async `execute` method.

#### File Operations Tools

| Tool | Purpose |
|------|---------|
| `apply_patch` | Apply a Codex-style patch with add/delete/update and file moves |
| `read` | Read file contents with line-range support |
| `Write` / `create` | Create or overwrite a file |
| `Edit` | Replace one exact string occurrence; optional `collapse_whitespace` relaxes matching (whitespace-run collapse + `\t`/`\n`/`\r`/`\\` escape decoding) |
| `multiedit` | Apply multiple edits atomically across one or more files; each edit honours its own `collapse_whitespace` flag |
| `patch` | Apply a unified diff |
| `rm` | Delete a single file |
| `move` / `move_file` | Move or rename a file/directory |
| `copy` / `copy_file` | Copy a file |
| `mkdir` / `make_directory` | Create directories |
| `append_to_file` | Append text to the end of a file |
| `file_info` | File/directory metadata |
| `diff_files` | Unified diff between files or strings |
| `Glob` | Find files matching a glob pattern |
| `list` | List directory contents |

#### File Operation Aliases

| Alias | Maps to |
|-------|---------|
| `read_file` | `read` |
| `write_file` | `Write` |
| `update_file` | `Write` |
| `delete_file` | `rm` |
| `apply_patch` | `patch` |
| `multiedit` | `multi_edit` (legacy parameter normalisation) |

#### Execution Tools

| Tool | Purpose |
|------|---------|
| `Bash` | Run a shell command with 7-layer safety |
| `run_code` | Alias for `Bash`; accepts `code`/`command` |
| `bash_reset` | Reset persistent shell state |
| `calculator` | Evaluate mathematical expressions |
| `open` | Open/reveal files, folders, or URLs in the desktop environment |

#### Interactive Tools

| Tool | Purpose |
|------|---------|
| `AskUserQuestion` | Ask the user a question (text or multiple-choice, with an optional default used on timeout) |
| `think` | Record a reasoning note |

#### Task Management Tools

| Tool | Purpose |
|------|---------|
| `TaskCreate` | Create a session-scoped task with subject, description, optional owner, active_form, metadata, and blocked_by dependencies |
| `TaskUpdate` | Update a task's status (pending, in_progress, completed), subject, owner, metadata, or dependencies; auto-evaluates blocked tasks on completion |
| `TaskGet` | Retrieve the full record of a single task by ID |
| `TaskList` | List all session tasks, optionally filtered by status |

#### Utility Tools

| Tool | Purpose |
|------|---------|
| `get_env` | Read non-sensitive environment variables |
| `calculator` | Evaluate mathematical expressions |
| `ragent_info` | Report the running ragent version, build time, git commit, compiler, and execution details (pid, parent pid, start time, uptime, executable, working directory, memory, thread count) |
| `os_info` | Report read-only host OS/hardware introspection (OS identity and distro, CPU, GPU adapters and graphics-API versions, physical hardware, memory, uptime, process environment); also `/osinfo show` |
| `tool_info` | Return a JSON-encoded dump of the tool registry (name, description, parameters schema, permission category, source, hidden state) |
| `commands_info` | Return a JSON-encoded catalog of every slash command (trigger, description, subcommands, flags) including plugin-contributed commands |

### 3.2.1 Tool System Categories Summary

| Category | Tools |
|----------|-------|
| File operations | `read`, `Write`/`create`/`update_file`, `Edit`, `multi_edit`/`multiedit`, `apply_patch`, `patch`, `rm`, `move`, `copy`, `mkdir`, `append`, `file_info`, `diff`, `Glob`, `list` |
| Shell / execution | `Bash`, `run_code`, `bash_reset`, `calculator`, `open` |
| Search | `Grep`, `codeindex_*` |
| Web / MasterFetch | `webfetch`, `websearch`, `http_request`, `mf_fetch`, `mf_crawl`, `mf_search`, `mf_cache_clear`, `mf_version` (+ `mf_screenshot`, engine-gated per FR-029) |
| Memory | `memory_read`, `memory_write`, `memory_replace`, `memory_store`, `memory_recall`, `memory_forget`, `memory_search`, `memory_migrate`, `conversation_search`, `session_search` |
| Code index | `codeindex_search`, `codeindex_symbols`, `codeindex_references`, `codeindex_dependencies`, `codeindex_status`, `codeindex_reindex` |
| Code graph | `codeindex_explain`, `codeindex_path`, `codeindex_communities`, `codeindex_godnodes` |
| Teams | `team_*` lifecycle/task/message tools |
| Sub-agents | `new_agent`, `cancel_agent`, `list_agents`, `wait_agents`, `agent_complete` |
| VCS | Git local, GitHub, and GitLab issue/PR/MR/pipeline tools |
| PDF | `pdf_read/write` |
| MCP | `mcp_tool` |
| Planning | `EnterPlanMode`, `ExitPlanMode` |
| Background tasks | `bg` |
| Initiatives / skills | `initiative`, `Skill` |
| Interactive | `AskUserQuestion`, `think` |
| Task management | `TaskCreate`, `TaskUpdate`, `TaskGet`, `TaskList` |
| Utility | `get_env`, `calculator`, `ragent_info`, `os_info`, `tool_info`, `commands_info` |

#### Team Tools

| Tool | Purpose |
|------|---------|
| `team_create` | Create a named team |
| `team_spawn` | Spawn a teammate for a single task |
| `team_status` | Team status: members with their last five status transitions, undelivered mailbox messages, plans awaiting a verdict, and hook failures |
| `team_message` | Send a direct message |
| `team_broadcast` | Message all active teammates |
| `team_read_messages` | Read unread mailbox messages |
| `team_memory_read` / `team_memory_write` | Team-scoped memory access |
| `team_submit_plan` | Submit a plan for lead approval |
| `team_approve_plan` | Lead approve/reject a plan |
| `team_task_create` | Add a team task |
| `team_task_claim` | Claim a task |
| `team_task_complete` | Mark a team task done (owner or lead only) |
| `team_task_cancel` | Lead withdraws a pending/in-progress task |
| `team_assign_task` | Lead assigns a task directly |
| `team_idle` | Signal no more work |
| `team_shutdown_teammate` | Request teammate shutdown |
| `team_shutdown_ack` | Acknowledge shutdown request |
| `team_wait` | Block until teammates finish |
| `team_cleanup` | Delete team on-disk state |

#### Background Shell Task Tool

| Tool | Purpose |
|------|---------|
| `bg` | Spawn, monitor, tail, and cancel long-running shell commands as **shell tasks** (never sub-agents, whose vocabulary is the execution contract). Actions: `spawn`, `list`, `status`, `output`, `tail`, `cancel`, `wait`, `cleanup`. `session_id` on `list`/`cleanup` must name the calling session - a foreign id is refused (`reason_code: wrong_session`) and deletes nothing. |

#### Agent Search Tool

| Tool | Purpose |
|------|---------|

#### Durable Initiatives & Skill Management Tools

| Tool | Purpose |
|------|---------|
| `initiative` | Create, checkpoint, list, and close durable project-scoped goals with milestones. |
| `Skill` | List, read, load, and reload skill packs at runtime. |

### 3.2.1 Tool-Call Wire Path and the One Dispatch Pipeline (spec `uniformtools`)

Four rules apply to every provider and every turn, whichever surface drove it.

**A provider declares its contract, it does not build tool JSON.** The nine
profile fields in the section above are the contract
(`ragent_llm::provider::wire::ToolWireProfile`), and one encoder each reads them:
`encode_tools` for the outbound definition list (its bytes are cached per
`(ToolFormat, fingerprint)` and a cache hit is byte-identical to the miss),
`encode_message` for a persisted message - a tool result lands in the profile's
carrier, keyed by the profile's id field - and `wire::decode::ToolCallAccumulator`
for streamed deltas, which attributes a fragment by the frame's content-block or
choice index (or by call id) rather than to whichever call opened last. A
provider whose stream supplies no id has one minted as
`<provider>_<counter>_<name>`, deterministic across the step, because a call id
is replayed in the next request and a fabricated one that changes shape between
requests breaks result pairing. Two calls carrying the same id in one response
are reconciled by occurrence order and left as two calls; neither is dropped.
Tool-use argument JSON is decoded by one shared helper,
`ragent_types::tool_args::decode_tool_arguments`, which accepts a JSON object, a
JSON string that decodes to one, and empty or whitespace text (as `{}`), is
called at most once per call per step, and is not called in the provider layer at
all. A decode failure produces one refusal value
(`session::tool_input_error::ToolInputError`) that owns its sentence, its record
status and therefore its `is_error` flag, so the same failure cannot be worded or
flagged two ways.

**One ordered dispatch pipeline serves both execution modes.** A step's tool
calls - sequential or concurrent - are prepared into one list of slots in the
provider's order (a future for a call that runs, a settled outcome for one that
is refused) and drained in a single ordered pass, so
`experimental.parallel_tool_calls` changes the concurrency of a step and never
the history it records. The stages are named once in `session::dispatch`:
decode arguments, validate schema, unknown-tool check, repeat guard, pre-tool
hooks, permission verdict, backend dispatch, result normalisation.

**An unknown name never executes.** A call whose name resolves to no registered
tool returns a corrective tool result naming what was asked for and the closest
registered name when one is within the shared similarity threshold - the one
ranking in `session::tool_suggestion` - and no tool runs. A call whose
*arguments* fail to decode or validate is refused the same way, before
dispatch, and the refusal is flagged as an error so the model can correct it.

**A tool result is never indistinguishable from a success.** The canonical
`ContentPart::ToolResult` carries `is_error`; providers with native support
transmit the flag, and a profile without it prefixes the text with `ERROR: `, so
a failed run and a refused call reach the model behind the same marker. A
result is appended only for a call whose permission verdict resolved, and a
provider that reports a tool-use finish while the step parsed no complete call is
treated as a truncation rather than a clean answer (FR-022).

### 3.2.2 Completion and Suggestion Surfaces (FR-033, spec `uniformtools`)

Every surface that offers a tool name is built from the tool **registry**, which is
the only catalogue of names: the slash-command and tool-name menus, the
did-you-mean candidate pool, and the lists printed by `--dry-run`, `tool_info` and
`commands_info`. A retired `before` spelling appears in none of them, and typing one
produces no completion entry - only the ordinary unknown-tool refusal with the
did-you-mean hint when a live name is close.

| Surface | Source |
|---------|--------|
| did-you-mean pool | `session::tool_suggestion::suggest_tool_name` over `ToolRegistry::list` (one implementation, also re-exported by `session::dispatch`) |
| `tool_info` | the `ToolContext::tool_registry` it was handed |
| `--dry-run` tools section | `ToolRegistry::list` + each tool's `permission_category()` |
| system-prompt tool reference | `ToolRegistry::definitions()` |
| slash-command subcommand menu | one table per command; `/tools` reads `command_catalog::TOOLS_SUBS` |
| `commands_info` | `command_catalog::COMMAND_CATALOG` (`/tools` publishes `TOOLS_SUBS`) |
| `@` file picker | project **paths** only - this field offers no tool name and FR-033 does not add one |

The `/tools` row is the one subcommand table with four readers: the autocomplete menu,
the parameter hint, the handler (its help page and every usage warning are generated
from it), and `commands_info`. Adding a `/tools` form means adding it to `TOOLS_SUBS`
and nowhere else.

### 3.2.3 Tool Surface Agreement (FR-030, spec `uniformtools`)

The registered tool surface is grouped into permission families (the `tools`
section of `ragent --dry-run --json run x` reports the live total and per-family
counts), and every enumeration of it is checked against the registry rather than
against a remembered number:

| Enumeration | What it reads | Guard |
|-------------|---------------|-------|
| `ToolRegistry::list`, `definitions`, `all_tools` | the registry keys and tool objects | `test_fr030_surface_agreement.rs` case 1 |
| the sorted name list and its digest | the FR-023 `after` spellings (13 uppercase) | case 2 |
| `--dry-run` `tools` section (human and `--json`) | `list()` + `permission_category()`, `total` then one row per family | cases 3 and 4 |
| `tool_info` | `ToolContext::tool_registry.all_tools()` | case 5 |
| `commands_info` | `command_catalog::COMMAND_CATALOG` (`/tools` publishes `TOOLS_SUBS`) | case 6 |
| outbound provider `ChatRequest.tools` | `definitions()`, via `get_cached_tool_definitions` | case 7 |
| `/tools list` | `definitions()` | TUI case 1 |
| `/bug-report` `Tools registered` line | `list().len()` | TUI case 2 |

`/tools list` reports every tool the registry holds - the same set `definitions()`
returns and the same set sent to the provider - so its row count is the tool count.
The family is the leading segment of `permission_category()`, never the tool name:
`team_wait` reports `agent:spawn`, so the split is `agent` 5 / `team` 20 and a
name-prefix reading gives the wrong 3 / 21 for the same 150 total. (`agent` grew
4 -> 5 when `list_agents` moved from `none` to `agent:read`, spec `agentssetup`
FR-005; the family count stayed 21 because the category was re-spelled inside an
existing family.)

### 3.3 Agent System

Agents are typed profiles that define what model, system prompt, tools, and
permissions a session uses.

#### Parallel Tool Dispatch Gate (FR-010, spec `uniformtools`)

Each provider family declares a tool-calling wire profile, and one of its fields
is `supports_parallel` - whether the provider can carry several tool calls in one
step. The agent loop reads that field once per step, before any call is prepared:
when the user's `experimental.parallel_tool_calls` is on **and** the active
provider's profile does not permit concurrency, the flag is demoted to `false`
for the step and one `tracing::info!` names the provider and the reason. Every
call of that step then runs through the sequential arm of the one ordered
dispatch pipeline, which since the pipeline consolidation is the same pipeline
with the same recorded order - so the gate changes the concurrency of a step and
never the history it produces.

Two boundaries are part of the contract. The gate is a **demotion only**: `true`
means concurrency is permitted, never switched on, and only the user's own opt-in
enables it, so a provider that supports parallel calls is untouched. And an id
with **no** declared profile is gated too (the condition is
`is_none_or(|p| !p.supports_parallel)`), because a profile table with no
catch-all means an unnamed provider declares nothing rather than inheriting
agreement. The `router` provider is gated by its own row and not by the
downstream provider its classification picks - that selection happens inside the
router's `chat`, after the decision.

#### Reasoning Echo (FR-011, spec `uniformtools`)

Anthropic's extended thinking attaches an opaque **signature** to every signed
`thinking` block and requires that block, signature included, to be replayed
unchanged on the turn that answers a tool call. The block is therefore stored as
one canonical part with two halves kept apart - the reasoning text and the
provider's signature over it - and a profile field decides whether it is
replayed:

- **Recording.** The streamed `ReasoningSignature` frame is kept in its own
  field beside the reasoning text (never appended to it, or it would be shown to
  the user and lost as a signature), and both halves are written to the saved
  `MessagePart::Reasoning`. A retry clears the signature with the text it signed,
  because a provider verifies the pair together and a fresh block carrying the
  previous attempt's signature is rejected rather than merely unverified.
- **Replaying within the turn.** The tool phase writes the reasoning part into
  the step's assistant message *above* its text, so it precedes both the text and
  the `tool_use` blocks of that turn. This is not the same path as a history
  rebuild: the request that answers the call is built from the in-turn message
  list, so a block that exists only in the persisted record would arrive one turn
  too late.
- **Omitting.** `echo_reasoning` is declared per family (`true` for Anthropic
  Messages, `false` for the other five). The encoder writes
  `{"type":"thinking","thinking":<text>}` plus `signature` - and writes no
  `signature` key at all for a block recorded without one, rather than a `null` -
  only where the profile declares it. Every other family omits the part, and that
  omission is a declared term of the contract rather than the silent drop it was
  before.

Neither history converter consults `echo_reasoning`: the omission is a property
of the wire contract, so deciding it anywhere but the encoder would re-implement
the profile table.

#### Unresolved Approvals Block the Result (FR-013, spec `uniformtools`)

A tool call's permission verdict is a state, not an event. While it is
unresolved - the interactive prompt is open and nobody has answered, a client has
not replied, a checkpoint timer has not fired - the call has no outcome, and the
model must not be handed one: a turn that continued on an unapproved call's
output would have acted without approval while reading as if it had been allowed.

That state is tracked rather than assumed. `session::pending::PendingVerdicts`
holds, per session, the call ids currently at a prompt. The dispatch pipeline's
permission stage marks a call immediately before it prompts and settles it the
moment the verdict resolves; a timeout is a denial, so it settles too, and an
`Ask` that somehow survives the layer is refused as an error rather than reported
as a success. Before the tool phase appends anything it asks the same tracker,
through `result_is_appendable`. On an unsettled verdict the call's
`MessagePart::ToolCall` record is still written - status `Pending`, no output, no
error, so the provider's `tool_use` is never orphaned - but no
`ContentPart::ToolResult` is appended and one warning names the call, because a
silently dropped result would hide a wedged verdict.

Three details are part of the contract. The tracker is keyed by **session and
call**, because call ids are not globally unique (a provider restarts its
`call_1` every response, and sessions run concurrently), so one session's verdict
can never unblock another's call. A **second mark needs no second settle**: the
bash path raises one prompt per sub-command under a single call id, and the
verdict is one verdict. And a **denial is a resolved verdict**, so a denial's
error observation is appended normally - FR-013 forbids appending an unresolved
call's output, not a refusal.

#### Built-in Agents

| Agent | Role |
|-------|------|
| `ask` | Quick Q&A agent with read-only permissions (tools stay available; the prompt asks it to answer directly) |
| `general` | General-purpose coding agent |
| `build` | Build and test agent with full tool access |
| `plan` | Planning agent that creates implementation plans |
| `explore` | Exploration agent for understanding codebases |
| `rust-coder` / `python-coder` / `typescript-coder` | Language coding specialists |
| `fastapi-agent` | FastAPI project specialist |
| `security-auditor` | Security code reviewer (OWASP Top 10, CWE) |
| `test-writer` | Test generation specialist |
| `documenter` | Documentation specialist |
| `devops-agent` / `database-agent` / `frontend-agent` | DevOps, database and frontend specialists |
| `title` / `summary` | Hidden helper agents (session titles, summaries) |

#### Agent Features

- **Model binding** — Each agent can specify a `provider/model` or auto-resolve the first available model
- **Tool surface** — Every registered tool is advertised to the model: the
  request's `tools` array is `definitions()` and the system prompt lists the same
  set. A tool is absent only when the subsystem that registers it is inactive
  (the code index registers its ten tools only while enabled), so there is no
  per-family filter to configure.
- **Permissions** — Per-agent permission rules merge with global config
- **Custom prompts** — Markdown/OASF profiles in `~/.ragent/agents/` or `.ragent/agents/`

### 3.3.1 Git Platform Integrations

| Platform | Coverage |
|----------|----------|
| **GitHub** | Issues, pull requests, comments, reviews, pipelines |
| **GitLab** | Issues, merge requests, pipelines, jobs, projects |

Authentication uses environment tokens (`GITHUB_TOKEN`, `GITLAB_TOKEN`) or
configured credentials, with auto-detection from the local git remote.

### 3.5 Session & Tool Execution Flow

```mermaid
graph LR
    Input[User Input] --> SP[Session Processor]
    SP --> Agent[Resolve Agent + Model]
    SP --> LLM[LLM Call]
    LLM --> ToolCall[Tool Call Request]
    ToolCall --> Permission[Permission Check]
    Permission -->|ask| UI[User Prompt]
    UI --> Decision[Allow / Deny / Always]
    Decision --> Execute[Tool Execution]
    Execute --> Result[Tool Result]
    Result --> SP
    SP --> Output[Assistant Response]
```

**Figure 5:** Session & Tool Execution Flow — LLM call → permission → tool dispatch loop

### 3.6 Provider Selection Flow

```mermaid
graph TD
    Agent[Agent Profile] --> ModelRef[ModelRef or None]
    ModelRef -->|explicit| Registry[Provider Registry]
    ModelRef -->|None| DefaultResolver[Resolve first available model]
    CLI[--model override] --> Registry
    Config[default_provider] --> Registry
    Registry --> Health[Health Check]
    Health -->|healthy| Use[Use Provider]
    Health -->|unhealthy| Fallback[Fallback / Error]
```

**Figure 6:** Provider Selection Flow — Multi-provider routing and health checks

### 3.7 Agent Execution Loop Phases

```mermaid
graph LR
    A[Start Turn] --> B[Build Context]
          B --> C[Compact if Over Threshold]    C --> D[Send to LLM]
    D --> E[Stream Tokens / Tool Calls]
    E --> F[Execute Tools]
    F --> G[Record Results]
    G --> H[End Turn]
    H -->|next turn| A
```

**Figure 7:** Agent Execution Loop Phases — One complete turn from input to response

#### Phase Descriptions

| Phase | Responsibility |
|-------|----------------|
| Build Context | Assemble system prompt, memories, code index, conversation history |
| Compact | Summarise conversation history when estimated request tokens exceed the configured percentage of the model's context window (`threshold`, default 70%) or, when `threshold` is null, `context_window - max(output_tokens, compaction.buffer)` (FR-003); emergency compaction on provider context-overflow errors (FR-004) |
| LLM Call | Send request to selected provider |
| Stream | Emit assistant chunks and tool call events |
| Execute Tools | Check permissions, run tools, record results |
| Record | Append assistant message + tool results to SQLite |

Between steps the loop also runs two gates:

- **Sub-agent suspension gate (spec `agentui` FR-035)** — a child session
  parked by `AgentManager::suspend_task` waits here (a 200 ms poll that
  breaks on the cancel flag) until `resume_task` clears its flag or the turn
  is cancelled; sessions not spawned by the manager skip the gate. The
  suspension is cooperative: the LLM call or tool execution in flight when
  the request lands finishes first. A separate hard watchdog kills any
  single tool call and terminates the run when it stalls past
  `TOOL_WATCHDOG_TIMEOUT` (5000 s; see `session/processor.rs`).
- **Text-format tool-call recovery** — when the step produced no native tool
  calls, the loop (in `session/loop_steps.rs`, before callers save the
  buffers) extracts pending calls from the text buffer first and then the
  reasoning buffer, blanking any recovered spans so the narration is not
  re-sent. The recogniser accepts the Qwen private-tag JSON dialect, the XML
  `function=` dialect, the plain `<toolcall>` pair (including a slash-dropped
  close-tag variant that produces two identical tags), the underscore-spelled
  `tool_call` pair, and a bare tool-call JSON object/array; ordinary prose
  never matches, and recovered calls flow through the normal dispatch
  pipeline (loop restrictions, hooks, permission checks, registry).

### 3.8 Autonomous Loop Tools

| Tool | Use |
|------|-----|
| `new_agent` | Spawn a background sub-agent (optional `detached: true` for a fire-and-forget task excluded from `list_agents`/`wait_agents` whose result is not injected back) |
| `cancel_agent` | Request cancellation of a running sub-agent; reports `cancel_requested` plus the task's current status, never a terminal state it has not reached |
| `list_agents` | Inspect active sub-agents (reports `limit`/`occupancy`/`headroom`, the `waiter_count` of a task in-flight `wait_agents` callers hold - shown only when greater than zero - and its `status` filter accepts every declared status: `running`, `suspended`, `completed`, `failed`, `cancelled`, `timed_out`) |
| `wait_agents` | Block until sub-agents finish |
| `agent_complete` | **Terminal signal** that ends the autonomous loop |

`agent_complete` takes **only** `summary`. `team_task_complete` takes `team_name` + `task_id`.

**`/cancel <prefix>` delegates to the same registry.** The slash command is not a
separate cancellation path: it resolves the prefix against the Agents window's
`active_tasks` projection, then routes a single match to
`AgentManager::cancel_agent` - the method behind `cancel_agent` - and reports the
*request*, never an outcome it has not seen (FR-023). It does not remove the row:
the entry stays `running` until the agent loop observes the cooperative cancel flag
at its next step boundary, and the row is retired by the `SubagentCancelled` event
or the registry reconciliation, so the window cannot show a cancellation that did
not happen. A prefix matching more than one live task is refused with every match
listed as `<short id> (<agent>, <status>)` (FR-024); a prefix matching none reports
`No task matching '<prefix>'` and lists the live ids, or says plainly that there are
none (FR-041). A prefix matching one *finished* task reports
`Task <short id> is already <status>` rather than a cancellation. Only `running`,
`suspended` and `terminating` count as live; `completed`, `failed`, `cancelled` and
`timed_out` are never offered as cancel targets. The benchmark-first dispatch
(`/cancel <prefix>` against an active `/bench` run) is unchanged and still takes
precedence over sub-agent tasks. Covered by
`test_cancel_slash::cancel_requests_through_the_registry_and_keeps_the_row`,
`::cancel_refuses_an_ambiguous_prefix_and_lists_the_matches`,
`::cancel_names_the_live_ids_when_no_task_matches` and
`::cancel_reports_the_registry_status_of_a_finished_task`.

### 3.9 Failure Capture: Panic Reports, Crash Dumps, and the stderr Spool

Three complementary mechanisms cover the three ways a ragent process can die.

| Failure | Captured by | Where the evidence lands |
|---------|-------------|--------------------------|
| Unwinding panic (in-process) | `panic_hook::install` — writes the payload, location, environment, and a forced backtrace, then chains to the default hook | `log/panics/panic-<timestamp>.log` |
| Abort — stack overflow, `SIGKILL`, `std::process::abort` | OS core dump (no in-process hook can run) | `coredumpctl` entry / `core_pattern` file; detection marker at `log/panics/last-crash.json` |
| Raw stderr while the TUI owns the alternate screen | `stderr_spool::install` — fd-2 redirection drained into a truncating spool | `log/logwindow/stderr-<timestamp>.log` plus the log panel |

**Stack overflows cannot be trapped in-process.** The Rust runtime prints
`thread '...' has overflowed its stack` and calls `abort()`; nothing after that
point runs. ragent therefore does not try. Instead, `src/crash_dump.rs` stamps
`log/panics/last-crash.json` at startup (pid, exe, args, cwd, thread,
`status: running`, and a platform-specific core-dump retrieval hint), overwrites
it with `clean exit` on a normal return, and — when the next start finds a
still-`running` record whose pid is gone — prints a warning naming the marker,
the dead pid, and the host's core-dump command. The hint is `coredumpctl
list/info/debug <pid>` when `/proc/sys/kernel/core_pattern` pipes into a helper
(typical `systemd-coredump` hosts), otherwise the `core_pattern` file path or
the macOS `/cores` convention. A clean exit never reports.

**The stderr spool** exists because the TUI paints the alternate screen: the
default panic hook, `eprintln!`, and C-library diagnostics would be overwritten
by the next frame. In TUI mode the binary replaces fd 2 with the write end of a
pipe and drains it on a dedicated thread into
`ragent_types::stderr_spool::Spool` (`SPOOL_MAX_LINES = 1000`); the TUI mirrors
each chunk into the log panel as `stderr: <line>`. The spool counts newlines per
write and, once the 1000-line cap is exceeded, rewrites the file with only its
newest 1000 lines — the rewrite is skipped entirely for a write containing no
newline, so the common case costs one append. Non-TUI modes (`--no-tui`,
headless `run`, `serve`) leave stderr attached to the terminal.

---

## 4. Security & Permissions

### 4.1 Permission Security Layers

```mermaid
graph TD
    ToolCall[Tool Call] --> Hardwired[Hardwired Rules]
    Hardwired -->|codeindex / agent_complete / list_agents| Allow[Always Allow]
    Hardwired -->|others| Rules[Configured Rules]
    Rules --> Match[Last Match Wins]
    Match -->|allow| Allow
    Match -->|deny| Deny
    Match -->|ask / no match| Prompt[User Prompt]
    Prompt -->|always / once| Allow
    Prompt -->|deny| Deny
    Prompt -->|timeout| Deny
```

**Figure 10:** Permission Security Layers — 5-layer defense-in-depth

### 4.2 Bash Security — 7 Layers

```mermaid
graph TD
    Cmd[Command String] --> L1[Layer 1: Safe Command Whitelist]
    L1 -->|safe| Auto[Auto-approve]
    L1 -->|not safe| L2[Layer 2: Banned Commands]
    L2 -->|banned| Reject
    L2 -->|ok| L3[Layer 3: Denied Patterns]
    L3 -->|denied| Reject
    L3 -->|ok| L4[Layer 4: Directory Escape Check]
    L4 -->|escape| Reject
    L4 -->|ok| L5[Layer 5: Syntax Validation]
    L5 -->|invalid| Reject
    L5 -->|ok| L6[Layer 6: Obfuscation Detection]
    L6 -->|obfuscated| Reject
    L6 -->|ok| L7[Layer 7: User Allow/Deny Lists]
    L7 -->|deny| Reject
    L7 -->|allow/ok| Permission[Permission Check]
```

**Figure 11:** Bash Security — 7 Layers — Bash command defense flow

### 4.3 Permission Request Flow

```mermaid
sequenceDiagram
    participant Tool as Tool Executor
    participant SP as Session Processor
    participant Bus as Event Bus
    participant UI as TUI / HTTP API
    participant User as User
    Tool->>SP: request permission
    SP->>Bus: PermissionRequested
    Bus->>UI: render dialog
    User->>UI: decision (allow/deny/always)
    UI->>Bus: PermissionReplied
    Bus->>SP: resume execution
```

**Figure 12:** Permission Request Flow — From tool call to user decision

Permission requests include:

| Field | Description |
|-------|-------------|
| `id` | Unique request ID |
| `permission` | Permission type (`file:write`, `bash`, etc.) |
| `action` | Requested operation description |
| `path` | Target path (for file operations) |
| `command` | Command string (for `Bash`) |
| `created_at` | Unix timestamp |
| `timeout_secs` | Timeout before auto-deny (default 120 s) |

### 4.4 Permission Rules Evaluation

Beyond the rule evaluation below, a **tool-repeat guard** (FR-044) sits on
every dispatched call: five consecutive identical calls (same tool + same
argument hash) pass through untouched; the sixth raises a `tool:repeat`
permission prompt in interactive runs and is auto-denied with a corrective
observation in unattended runs (subagent, `--yes`, YOLO). A different call
or a user "allow" resets the consecutive counter.

```mermaid
graph LR
    Rules[Rules List] --> Iterate[Evaluate in order]
    Iterate --> Match{Glob Match?}
    Match -->|yes| Record[Record Match]
    Match -->|no| Next[Next Rule]
    Record --> Next
    Next --> More{More Rules?}
    More -->|yes| Iterate
    More -->|no| Last[Last Match Wins]
    Last --> Action[Allow / Deny / Ask]
```

**Figure 13:** Permission Rules Evaluation — Rule matching and resolution

Rules are evaluated top-to-bottom; the last matching rule determines the action.
Built-in default rules include:

| Default | Action |
|---------|--------|
| `read` | allow |
| `Edit`, `Bash`, `web`, `EnterPlanMode` | ask |
| `todo` | allow |

### 4.5 File Path Guards

- File-write tools must target paths inside the working directory unless the user
  explicitly allows escaping.
- Path normalization resolves `..` and symlinks before permission checks.
- The `dirs` config block can whitelist additional writable roots via `allowlist`/`denylist` glob patterns.
- `dirs.allowed_roots` declares *additional* directory roots that are valid for path escape checking; the session's `working_dir` is always a valid root, and files under it or under any listed root are reachable by the `read`/`Write`/`list`/`Grep` tools (FR-017). The `allowed_roots` list extends the working directory; it never replaces it.

### 4.6 Secret Redaction

Environment variables matching `*KEY*`, `*SECRET*`, `*TOKEN*`, `*PASSWORD*`
are redacted from tool outputs and logs.

### 4.6a Path and Argument-Injection Guards

Each guard is a named helper rather than an inline check, so every call site
shares one implementation.

| Guard | Where it lives | What it rejects |
|-------|----------------|-----------------|
| `validate_team_name` | `ragent-agent::team::store` | anything outside `^[a-z0-9][a-z0-9-]{0,63}$`; called by `TeamStore::create`, `find_team_dir`, `find_team_dir_cached`, `team_cleanup`, and the `/team create`/`/team delete` slash commands |
| Team-name slugify (`slugify_team_name`) | `ragent-agent::team::store` | blueprint/timestamp-derived names are slugified before use so generation can never produce an invalid name |
| `is_safe_git_argument` | `ragent-plugins::add` | a `git+` ref or subpath that is not `[A-Za-z0-9._/-]+`, begins with `-`, or contains `..`; `git_run` also passes `--` before every positional |
| `sanitize_declared_id` / `confined_dest_dir` | `ragent-plugins::manifest` / `add` | a manifest `id` that is not a single `Component::Normal` matching `[A-Za-z0-9._-]+` (no separators, no `..`, not absolute) |
| `check_path_within_allowed_roots_cached` | `ragent-tools-core` | a `multi_edit` target outside the workspace — the same containment check `Edit`/`read`/`Write` apply |

Two further classes are handled in place rather than by a helper:

- **Blueprint seed execution** — every tool named by `.ragent/blueprints/teams/<bp>/task-seed.json`
  or `spawn-prompts.json` is dispatched through `dispatch_seed_tool`, which consults
  the session permission checker: a category that is not explicitly `Allow` (or
  `Ask`-with-no-checker) refuses the seed and reports why, so a repository-supplied
  blueprint cannot reach `Bash`/`Write`/`rm` unpermissioned.
- **`multi_edit` containment** — see the table row above; the check runs for every
  edit in the batch before any file is read or written, so a mixed batch is atomic
  and writes nothing.

### 4.6b Host and Sandbox Guards

Each guard is a named helper or an explicit bound.

| Guard | Where it lives | What it enforces |
|-------|----------------|------------------|
| `hook_approved` on `PreToolUseResult::Allow` | `ragent-agent::hooks` / `session::processor` | a `pre_tool_use` hook approval is distinguishable from "no hook"; it may satisfy an `Ask` but never an explicit `Deny`. Plugin-contributed `pre_tool_use` hooks are dropped by `merge_hook_configs` |
| `unpack_archive_checked` | `ragent-agent::session::archive` | session-archive import rejects escaping entry paths and caps entries (10 000) and total decompressed bytes (512 MiB); `manifest.session_directory` is accepted only inside the working directory |
| `exec_guard` (`is_allowed_fixture_program`) | `ragent-bench` | a benchmark fixture may only spawn an allowlisted toolchain program (bare name, no path, no shell) |
| `index_file` containment | `ragent-codeindex` | the watcher-driven path canonicalises and refuses anything outside the project root, refuses symlinks, and enforces `max_file_size` |
| Project-config privilege stripping | `ragent-config` | the cwd `.ragent/ragent.json` may not enable YOLO, remove a global deny rule, or widen `allowed_roots`/bash allow-lists |
| `x-goog-api-key` header | `ragent-llm::providers::gemini` | the Gemini key is sent as a header, registered with the secret registry, and never interpolated into a logged URL |
| `redirect::Policy::none()` | `ragent-llm::providers` | authenticated LLM requests do not follow redirects, so a non-standard auth header cannot be replayed cross-host |
| `ResearchIo::template_path` -> `Option` | `ragent-research::io` | the `--template` name must be a single `[A-Za-z0-9_-]` component; an invalid name yields no template plus a warning |
| `redact_config_secrets` | `ragent-server::routes` | `GET /config` (`get_config`) replaces every credential value before the response is returned |
| `check_path_within_root` on `from_files` | `ragent-research` / `ragent-server` | research file seeds are canonicalised and rejected outside the project root |
| ChaCha20-Poly1305 `v3:` credential store | `ragent-storage` | a per-install 0600 key file, an authenticated construction with a fresh nonce, and a hard error on a corrupt row (no empty-string downgrade) |
| `sender_session_id` on `MailboxMessage` | `ragent-agent::team::mailbox` | the authenticated sender session id is recorded; a message claiming `from: "lead"` without one is rejected at the write boundary |
| `tls-roots` OTLP transport | `ragent-telemetry` | the gRPC exporter is compiled with TLS, and a non-loopback `https://` endpoint is refused when TLS is unavailable |
| Telemetry privilege stripping | `ragent-config` | telemetry enablement and endpoint overrides come from the user-global config only |
| OTLP export health monitor | `ragent-telemetry` | every export outcome is classified and counted; 5 consecutive failures disable export instead of retrying a dead endpoint, and each failure surfaces as an `Agent Notice` with the masked endpoint |
| Blocking OTLP exporter | `ragent-telemetry::subsystem::build_provider` | the OTLP exporter uses the blocking reqwest client (`reqwest-blocking-client`) under the default `PeriodicReader`, so an export needs no Tokio reactor and `flush` is safe from any thread |
| Thread-aware TUI panic hook | `ragent-tui::run_tui` | terminal teardown runs only when the panicking thread is the UI thread, so a background-worker panic cannot destroy the alternate screen |
| Safe-command allowlist + in-crate calculator | `ragent-tools-core` | the allowlist is read-only commands only, interpreters always go through the permission gate, banned tools are consulted first, and `calculator` evaluates arithmetic in-process (no `python3 -c`) |
| VCS argument hardening | `ragent-tools-vcs` | `reject_option_like` on every ref/branch/tag/remote/path, `--` before positionals, `validate_clone_directory` confines the clone target, and a `gitlab:<host>` host is accepted only when it matches the configured instance |

### 4.6c Network and Secret Bounds

The recurring shapes are a **byte cap**, a **page/entry budget**, a **clamped
delay**, and **origin-scoped credential attachment**.

| Guard | Where it lives | What it enforces |
|-------|----------------|------------------|
| `read_body_capped` / MCP body cap | `ragent-agent::mcp::http` | MCP HTTP responses are read under a fixed byte cap |
| Benchmark download caps | `ragent-bench::data` | client timeout, streamed byte cap, pagination page/record budget, and `contained_join` for manifest `case_file`/`relative_path` |
| `MAX_RETRY_AFTER` / `MAX_SSE_BUFFER_BYTES` / `MAX_ERROR_BODY_BYTES` | `ragent-llm::providers::http_client` | `Retry-After` is clamped to 30 s, the SSE accumulation buffer aborts the stream past 1 MiB, and error bodies are read under 64 KiB |
| `MAX_TREE_REQUESTS` / `MAX_TREE_ENTRIES` / `MAX_JOB_PAGES` | `ragent-tools-vcs` | the GitHub tree walk and the GitLab jobs pagination are bounded |
| `read_body_capped` / `MAX_FETCH_BODY_BYTES` / `MAX_CRAWL_PAGE_BYTES` | `ragent-tools-extended::masterfetch` | responses are streamed under a byte budget; `crawl_urls` is SSRF-validated at the tool boundary |
| Credential env seeding | binary startup | every credential env var is registered with the secret registry before any log can be written |
| `redact_secrets` on tool output | `ragent-tools-core::bash`, `ragent-tools-vcs::github_actions` | bash partial/completed output and CI log excerpts are redacted before they reach the model or the session store |
| `is_allowed_request_header` | `ragent-tools-extended::http_request` | only `accept`, `accept-language`, `content-type` and `user-agent` may be set; routing, hop-by-hop and credential headers are refused |
| `mask_secret` | `ragent-tui::layout` | the provider-setup dialog masks the API key and the GitLab PAT |
| `check_path_within_workspace` | `ragent-tools-extended::ToolContext` | `pdf_write` confines its output path |
| `build_exclude_matcher` / `clear_index_dir` | `ragent-codeindex` | the configured exclusion globs are applied and the FTS recovery wipe refuses a symlinked directory |
| Scratch-dir confinement (`bash_scratch_dir`, `restrict_to_owner`) | `ragent-tools-core::bash` | the wrapper script and `export -p` state file live in a 0700 directory as 0600 files |
| `MAX_CANDIDATES_PER_REF` / `MAX_EDGES_PER_PASS` | `ragent-codeindex::graph::edges` | edge derivation carries a per-reference fan-out cap and a global edge budget |
| `MAX_GAP_EXPANSION` / `MAX_CONTRADICTION_REQUIREMENTS` | `ragent-specs::validate` | numbering-gap expansion and contradiction detection are bounded |
| `MAX_MESSAGE_BYTES` / `validate_hook_command` / `HOOK_TIMEOUT` | `ragent-agent::team` | mailbox messages are size-capped, team hook commands are validated, hooks time out at 30 s with a process-group kill, and feedback is capped |
| `positive_duration_secs` / checked arithmetic | `ragent-types::cron` | `CronSchedule` returns `None` instead of panicking on an invariant violation and uses `checked_mul` / `saturating_mul` |
| `SPOOL_MAX_BYTES` | `ragent-types::stderr_spool` | the spool enforces a byte ceiling on every write, including newline-free ones |
| `split_bash_command` substitution depth | `ragent-agent::session::permissions` | a `;`/`&&` inside `$( ... )` does not split the command into a separately-judged sub-command |
| `same_origin_as` | `ragent-tools-vcs::github` | the Bearer token is attached only when the resolved URL matches the configured API origin |

### 4.6d Defence in Depth

The same guard shapes at the remaining call sites: an explicit **cap/budget**, a
**containment check**, a **char-safe split**, or an **owner-only file mode**.

| Guard | Where it lives | What it enforces |
|-------|----------------|------------------|
| `unpack_archive_checked` (`ARCHIVE_MAX_ENTRIES`, `ARCHIVE_MAX_TOTAL_BYTES`) | `ragent-agent::session::archive` | session-archive import refuses an entry-count or total-decompressed-size overrun and rejects an entry path that escapes the extraction directory |
| `MAX_BENCH_SAMPLES` | `ragent-bench::model` | `--samples` is clamped to 100 before both the `Vec` pre-allocation and the generation loop |
| `MAX_DOWNLOAD_BYTES` / `MAX_HUMANEVALPACK_PAGES` / `MAX_HUMANEVALPACK_ROWS` | `ragent-bench::data` | a dataset download is streamed under a 64 MiB cap and the pagination loop carries a page and row budget |
| `base_url_override` run note | `ragent-bench::runner` | the resolved provider base URL is kept out of the shareable workbook and run-state sidecar |
| `contained_join` | `ragent-bench::data` | manifest `case_file` and `relative_path` are joined inside the canonicalised data root |
| read-then-check size | `ragent-codeindex::scanner` | the stored byte size is derived from the bytes actually read, closing the stat/read TOCTOU past `max_file_size` |
| `build_exclude_matcher` | `ragent-codeindex::scanner` | `ScanConfig::extra_exclude_patterns` is compiled into a glob matcher and applied during the walk |
| `clear_index_dir` symlink refusal | `ragent-codeindex::search` | `FtsIndex::open_or_create` refuses a symlinked index directory before the recovery wipe and surfaces deletion failures |
| `BUILTIN_DENYLIST` merge | `ragent-config::dir_lists` | the built-in system-directory patterns are merged into the enforced denylist, not merely advertised |
| parse-diagnostic redaction | `ragent-config::config`, `src/main.rs` | the echoed problematic source line is passed through `redact_secrets` before it is printed or logged |
| `is_contained_relative_path` | `ragent-plugins::manifest` | `entry` / `main` / `server.entry` must be a contained relative path inside the plugin root |
| `https_only_redirects` (shared) | `ragent-plugins::store_fetch` → `add` | the package download stops any non-`https` redirect |
| `is_single_normal_component` | `ragent-plugins::marketplace` | the wrapper-directory name derived from the install source must be a single normal component |
| `neutralise_fenced_body` | `ragent-research::document` | backtick runs are broken and `#### Source [#N]` / `**Sources:**` lines escaped before a fetched body enters a fenced synthesis prompt |
| `mask_url_credentials` | `ragent-research::document` → `gather_log` | recorded gather-log URLs drop userinfo and query strings |
| session-bound permission reply | `ragent-agent::session::permissions` | an `Event::PermissionReplied` is honoured only when its `session_id` matches the session awaiting the request |
| `internal_error_response` | `ragent-server::routes` | internal error detail is logged; the client receives `internal server error` |
| `SpecId` validating `Deserialize` | `ragent-specs::spec` | every deserialisation routes through `SpecId::new`, so a traversal id cannot be constructed |
| `write_govcreate_spec` re-validation | `ragent-specs::commands` | the public write API validates the id before joining it into the output path |
| `/spec create` / `/spec specify` name validation | `ragent-specs::commands` | an invalid spec name is a usage error |
| char-boundary diff split | `ragent-storage::snapshot` | `apply_unified_diff` splits on the first char, so a multibyte diff line cannot panic |
| `is_safe_memory_component` | `ragent-agent::team::config` | `resolve_memory_dir` rejects a multi-component or `..` agent name |
| `CardinalityCache::resolve` sanitisation | `ragent-telemetry::cardinality` | values are passed through `sanitize_attr_value` inside `resolve` and a poisoned lock fails closed to the unknown bucket |
| `parse_alog_run_id_yes` validation | `ragent-tui::app::helpers` | `/alog export` accepts only `[A-Za-z0-9_-]+` run ids |
| log-window spool mode | `ragent-tui::app::session_ops` | the spool is created 0600 and the mode is re-asserted on every open |
| `bash_scratch_dir` private fallback | `ragent-tools-core::bash` | a failed private-dir creation falls back to a process-private 0700 directory, never to shared temp |
| tree-walk budget | `ragent-tools-vcs::github` | the GitHub recursive tree walk carries a `MAX_TREE_REQUESTS`/`MAX_TREE_ENTRIES` budget, matching the GitLab equivalent |
| `redacted_event_debug` / SSE device-code masking | `ragent-server::sse` | credential-bearing events expose presence-only fields on the SSE stream and a redacted rendering for log sites |
| `CrawlFetcher` SSRF contract | `ragent-tools-extended::masterfetch::crawl` | the SSRF obligation is stated on the trait and validated at the `mf_crawl` tool boundary |

### 4.6e Shared Guards and CI Gates

Every mitigation in 4.6a-4.6d is one of a small number of guard *shapes*. Each
shape has a single implementation so a new call site cannot silently weaken it,
and CI gates fail a build that reintroduces a class.

**`ragent_types::guard`** (re-exported from the crate root, and as
`ragent_tools_core::guard` for the tools crates) owns:

| Helper | Enforces |
|--------|----------|
| `reject_option_like(value, label)` | non-empty and no leading `-`, so a `git` operand cannot be parsed as an option |
| `is_safe_operand(value)` | the predicate form: `[A-Za-z0-9._/-]+` and no leading `-` |
| `validate_identifier(value, label)` | `^[A-Za-z0-9._-]+$`, at most `MAX_IDENTIFIER_LEN` (64) bytes, no `.`/`..`/separators |
| `validate_relative_component(value, label)` | relative, no `\`, no `..`, no Windows drive or UNC prefix |
| `contained_join(root, relative, label)` | the above plus a canonicalised-root check that catches a symlink escape; it canonicalises the **deepest existing ancestor** of the joined path, so a not-yet-created leaf cannot escape through a symlinked intermediate component |
| `reject_symlink_components(path, label)` | no component of `path` (walking upward) is a symlink - refuses to write or delete through one |
| `reject_symlink_escape(path, root, label)` | `path` is not a symlink and canonicalises inside `root` |
| `open_owner_only(path)` | creates/truncates `path` owner-only (`0o600`), so a secret-bearing temp file is not world-readable |
| `clamp_retry_after(hint)` | the 1 s floor and `MAX_RETRY_AFTER` (30 s) ceiling on a server-supplied delay |
| `cap_read(bytes, max)` | a UTF-8-boundary-safe prefix of a buffered read |

Call-site-preserving adapters delegate to it:
`ragent-tools-vcs::git::reject_option_like`,
`ragent-plugins::add::is_safe_git_argument`, and
`ragent-bench::data::contained_join`. Regression suite:
`crates/ragent-types/tests/test_shared_guards.rs`.

**One redaction implementation.** `ragent-agent`, `ragent-storage`, and
`ragent-tools-core` re-export `ragent_types::sanitize` instead of holding a
second secret registry, so a credential registered anywhere is masked
everywhere: `GET /config`, telemetry attributes, log lines, SSE payloads, and
tool output all call the same `redact_secrets`.

`Event` does not derive `Debug`. Its hand-written impl renders through an
internal `DebugProxy` that reports credential-bearing fields
(`CopilotDeviceFlowComplete.token`, `CopilotDeviceFlowStartResult.device_code`)
as presence flags and then pipes the whole rendering through `redact_secrets`,
so `tracing::debug!("{event:?}")` cannot print an OAuth credential. Regression
suite: `crates/ragent-types/tests/test_unified_redaction.rs`.

**CI gates** (job `security-guards` in `.github/workflows/ci.yml`, and
`pre-flight.sh`). Each gate has a `--self-test` that seeds a violation and
asserts the gate fails it:

| Gate | Command | Fails when |
|------|---------|-----------|
| Redaction tests | `cargo test -p ragent-types --test all test_unified_redaction::` | a second registry or a leaking `Event` `Debug` appears |
| File-tool containment | `scripts/check-file-tool-containment.sh` | a registered file tool does not call the `allowed_roots`-aware `check_path_within_allowed_roots_cached` helper (a working-dir-only `check_path_within_root_cached` call fails the gate) |
| Panic-free production paths | `scripts/check-security-unwraps.sh` | a file exceeds its `.unwrap()`/`.expect()` baseline in `scripts/security-unwrap-baseline.txt` |
| One guard per rule | `scripts/check-shared-guards.sh` | a crate re-defines a shared guard or a second secret registry |
| Git-argument guard | `scripts/check-vcs-duplication.sh` | the leading-dash check is re-derived or the shared delegation is dropped |

The panic-free gate is a **shrink-only baseline** gate: existing panicking calls
are recorded per file in `scripts/security-unwrap-baseline.txt`, and every *new*
occurrence is rejected. An intentional call is exempted with a same-line
`// no-panic-ok: <reason>` comment, which makes it visible in review.

### 4.6f Containment, Redaction and Failure Reporting

**Startup and retry bounds.**

- **Crash-marker ordering.** `async_main` reads
  `crash_dump::previous_unclean_exit` *before* stamping the new `running`
  marker, so the "previous session exited without unwinding" warning reports
  the earlier process rather than the live one.
- **Search-retry bound.** `WebGatherer::with_search_max_retries` clamps
  its argument to `MAX_SEARCH_RETRIES` (10) and the backoff uses
  `checked_shl` capped by `MAX_SEARCH_RETRY_DELAY_MS` (60 s), so any
  CLI-supplied `--search-max-retries` value is safe.

**Containment and argument-injection guards.**

| Guard | Where it lives | What it rejects |
|-------|----------------|-----------------|
| `git::reject_option_like` on every operand | `ragent-tools-vcs::git` | a leading-dash operand to any git tool, including the `git_checkout` branch/source, every `git_cherry_pick` commit, every `git_add` path, and the `git_remote` name/url |
| `GitHubClient::validate_repo_segment` | `ragent-tools-vcs::github` | an `owner`/`repo` that is not `[A-Za-z0-9._~-]+`, so it cannot escape the request path |
| `resolve_url` for `fetch_readme` | `ragent-tools-vcs::github` | a request to the public origin when a GitHub Enterprise `base_url` is configured, so the Bearer token goes only to the configured API |
| `check_path_within_allowed_roots_cached` in every file tool | `ragent-tools-core` | a path outside both `working_dir` and the `allowed_roots` entries; `diff_files`, `file_info`, `Glob`, `open`, and `apply_patch` accept the same paths as every other file tool |
| `masterfetch::http::read_body_capped` / `read_bytes_capped` | `ragent-tools-extended` | an unbounded response body from search/robots/youtube reads (16 MiB ceiling; 512 KiB for small text bodies) |
| `write_govcreate_spec` target-folder containment | `ragent-specs::commands` | a `target-folder` that climbs out of the invoking root, checked lexically *before* any directory is created |
| `merged_dir_lists_for` / `builtin_only_dir_lists` | `ragent-config::dir_lists` | a config-load failure keeps the mandatory built-in system-directory denylist instead of returning an empty one |
| plugin `content_digest` | `ragent-plugins::add` | an install with no integrity record: a SHA-256 over the staged tree is recorded in the store ledger, so a substituted archive or checkout is detectable |
| aggregate archive cap (`MAX_EXTRACTED_BYTES`) | `ragent-plugins::add` | a decompression bomb made of many just-under-the-per-entry-cap entries |
| symlink refusal in `copy_dir_recursive` | `ragent-plugins::add` | a symlink inside a source directory is refused, not dereferenced and copied into the store |
| `read_body_capped` on every provider error path | `ragent-llm` | an unbounded 4xx/5xx error body; malformed SSE frames are logged rather than silently dropped |

**Redaction and file permissions.**

- `Config` does not derive `Debug`; a hand-written impl renders a JSON form
  routed through `ragent_types::sanitize::redact_secrets`, so an API key,
  GitLab token, bot token, webhook URL, or client secret cannot be printed.
- Crash markers, panic reports, and the stderr spool are created owner-only
  (`0o600`), and the `/bug-report` dump is written `0o600` too. The crash
  record's argv is redacted before serialisation, because
  `ragent auth openai sk-...` would otherwise persist a live credential to a
  world-readable file.

**Failure reporting.**

- Storage: the memory access-count bump logs its failure; the
  `conversation_stats` counts propagate errors instead of `unwrap_or(0)`; a legacy
  nonce that cannot be converted is an error, never a decrypt under an
  all-zero key.
- Server: HTTP 500 responses route through `internal_error_response`, so a
  SQLite or filesystem error string never reaches a caller; the
  `store_memory` fetch-after-write propagates its failure instead of answering
  `201` with an empty body.
- Agent: a sub-agent report that cannot be persisted to
  `log/subagents/<task-id>.md` warns with the task id and target directory,
  instead of silently reporting `output_file: None`.
- Telemetry: `ToolRecorder`, `SessionRecorder`, `CoordinatorRecorder`, and
  `CompressionRecorder` honour the FR-027 per-metric toggles, and
  `InstrumentRegistry::noop()` builds from an explicit locally-owned provider
  rather than the process-global one, so a disabled subsystem cannot export.

**Regression suites.** `crates/ragent-research/tests/test_search_retry_clamp.rs`,
`crates/ragent-tools-vcs/tests/test_ms0_git_arg_guards.rs`,
`crates/ragent-config/tests/test_ms0_config_guards.rs`,
`crates/ragent-plugins/tests/test_add.rs`,
`crates/ragent-specs/tests/test_govcreate_authoring.rs`,
`crates/ragent-telemetry/tests/test_metric_toggles.rs` (all wired into the
`ms0-regression` CI job).

### 4.6g Standards Gates

**Inline-test gate.** `scripts/check-inline-tests.sh` is a wrapper
over `scripts/check_inline_tests.py`, which distinguishes the two `#[cfg(test)]`
shapes:

| Shape | Verdict |
|-------|---------|
| `#[cfg(test)] #[path = ".../tests/inline/<name>.rs"] mod x;` | allowed (external body) |
| `#[cfg(test)] mod x { ... }` | rejected (inline body) |

The scanner covers `crates/*/src` **and** root `src/`, keeps a shrink-only
baseline of `0`, and ships a `--self-test` that proves an inline body fails the
gate while a `#[path]` hook passes. It runs in the `security-guards` CI job and
in `pre-flight.sh`.

**Production unwraps.** Constant-regex `expect`/`unwrap` sites that remain in
production code carry `// INVARIANT:` or `no-panic-ok` markers; see
`scripts/check-security-unwraps.sh` in 4.6e.

### 4.7 YOLO Mode

YOLO mode bypasses Layers 2, 3, and 6 of bash security and auto-approves all
permissions. It is intended for trusted local environments and is persisted
to `ragent.json`.

```bash
/yolo        # toggle in TUI
--yes        # CLI flag
```

#### 4.7.1 Automatic sub-agent YOLO scope

An **unattended sub-agent run** - any run with `agent.mode == Subagent`,
whether spawned by `new_agent` (foreground, background or detached), `/spawn`,
cron or the server - gets a YOLO-equivalent prompt scope **for its write-capable
tools only**, regardless of the global `/yolo` flag. A sub-agent has no attached
user, so an interactive prompt is not a safeguard but a stall: the call would
wait out the watchdog and the run would be lost.

The scope is a property of the run, resolved once per turn from the tool
surface that run was offered:

| Question | Answer | Where |
|---|---|---|
| Is the run unattended? | `agent.mode == AgentMode::Subagent` | `session/processor.rs`, threaded as `ToolDispatchContext::agent_is_subagent` |
| Can the called tool write? | its `permission_category()` is not in `READ_ONLY_PERMISSION_CATEGORIES` | `session/permissions.rs::is_write_capable_permission_category` |
| What does that buy? | the interactive **prompt** is skipped, exactly where the global flag skips it | `session/permissions.rs::check_permission_with_prompt` |

The membership set is built per turn from the run's model-facing tool surface
(`SessionProcessor::write_capable_tools`), so a read-only run (`explore`, a plan
reviewer, a loop restricted to reads) yields an empty set and keeps the ordinary
policy, and a name the run was never offered cannot be waved through.

`READ_ONLY_PERMISSION_CATEGORIES` is the whole answer to "can a run offered this
tool mutate the workspace?", and its failure mode is silent in the unsafe
direction: a category the list forgets is treated as **write-capable**, so a pure
inspection sub-agent is handed the YOLO write scope instead of the ordinary
prompt policy. The list therefore names every read category the registry actually
declares - `file:read`, `codeindex`, `web`, `web:read`, `network`, `model`,
`model:read`, `shell:open`, `memory:read`, `spec:read`, `skill:read`,
`git:read`, `github:read`, `gitlab:read`, `cron:read`, `agent:read`, `team:read`,
`task`, `system` - and deliberately excludes the mutating `team:tasks`, which
`team_task_claim` and `team_task_complete` share with the read-only
`team_task_list`. `tests/test_read_only_permission_categories.rs` enforces three
strengths of assertion: every tool in the default registry declares a *known*
category (a typo such as `git:reed` is invisible to the classifier and fails
loudly here), the named read surfaces classify read-only, and the named write
surfaces still classify write-capable.

**Containment.** The scope is narrower than the global flag and widens nothing:

- the primary session never gets it - the predicate is gated on the run being a
  sub-agent;
- only write-capable tools are covered; every read a sub-agent makes stays under
  the ordinary policy;
- the loop tool-set restriction, the path guards, the 7-layer bash
  `is_safe_command` stage (which runs *before* the prompt), the tool-repeat guard
  and every pre-tool hook still apply;
- an explicit policy `Deny` still denies, and a forced destructive-action
  checkpoint still prompts (FR-024) - the automatic scope suppresses a prompt,
  never a refusal.

Because isolation between a sub-agent and its parent is by process, the
process-wide parts of YOLO (bash denied patterns, dynamic-context allowlist, MCP
config validation) are still governed by the single global flag and are never
entered by a sub-agent alone. The automatic scope adds no process-wide state: it
is a per-turn function of the run's own tool surface.

---

## 5. Configuration

### 5.1 Configuration Sources

Ragent loads configuration from (highest priority first):

1. `--config <PATH>` CLI argument (replaces the layered load below with that single file)
2. `RAGENT_CONFIG_CONTENT` environment variable (inline JSON)
3. `RAGENT_CONFIG` environment variable (path to a config file)
4. `.ragent/ragent.json` in the working directory
5. `~/.config/ragent/ragent.json`
6. Built-in defaults

The format is compatible with OpenCode's `opencode.json`.

**Project overlays are untrusted (SEC-001/SEC-002/SEC-005).** A project-local
file may contribute the ordinary sections, but `Config::merge_project` strips the
privileged keys before the merge and logs each rejected key at `warn!`:

| Rejected project key | Reason |
|---|---|
| `yolo`, `permission` allow-rules, bash/dirs lists | would auto-approve or widen the sandbox |
| `telemetry` | could point the OTLP endpoint at an attacker host |
| `hooks` | runs `sh -c` outside the permission system |
| `openai` | enables the inbound OpenAI-compatible surface |
| `mcp` | a stdio server spawns a subprocess with no prompt (RCE) |
| `acp` | an ACP agent spawns verbatim, and the section controls the editor-facing server (RCE) |
| `provider.<id>.api` | redirects the provider credential and prompt stream to an attacker endpoint |

These keys are honoured only from the user-global config. The rest of a project
provider entry (`thinking`, `env`, `models`) still merges.

### 5.2 Configuration Schema

```jsonc
{
  "provider": {
    "anthropic": {
      "env": ["ANTHROPIC_API_KEY"],
      "thinking": { "enabled": true, "level": "low" },
      "models": {
        "claude-sonnet-4-20250514": {
          "thinking": { "enabled": true, "level": "high", "budget_tokens": 16000 }
        }
      }
    },
    "router": {
      "enabled": true,
      "tiers": {
        "SIMPLE": { "models": [{"provider": "ollama", "model": "phi4"}] },
        "MEDIUM": { "models": [{"provider": "anthropic", "model": "claude-sonnet-4-20250514"}] },
        "COMPLEX": { "models": [{"provider": "openai", "model": "gpt-4o"}] },
        "REASONING": { "models": [{"provider": "anthropic", "model": "claude-opus-4-20250514"}] }
      }
    }
    // azure_resource: keys come from each entry's `api_key_env` in azureresources.json
  },
  "defaultAgent": "general",
  "permission": [
    { "permission": "file:write", "pattern": "src/**", "action": "allow" }
  ],
  "memory": {
    "enabled": true,
    "structured": { "enabled": true },
    "semantic": { "enabled": false, "dimensions": 384 }
  },
  // OpenCode-derived summarisation compaction (recommended).
  // Compaction triggers when estimated request tokens exceed the configured
  // percentage of the model's context window. `threshold: 0.7` means the
  // summarisation runs once usage crosses 70 % of the available context,
  // regardless of the model's absolute context size. When `threshold` is
  // null, the fraction-based fallback `context_window - max(output_tokens,
  // buffer)` is used instead.
  "compaction": {
    "auto": true,
    "threshold": 0.7,
    "buffer": 0.10,
    "keep": { "tokens": 0.20 }
  },
  // Token-efficient GCF encoding of JSON tool results (default off; the
  // section is omitted entirely while disabled).
  "gcf": {
    "enabled": false
  },
  // Session-message retention (opt-in; the section is omitted entirely while
  // disabled). When enabled, messages older than `message_days` are deleted at
  // startup and by `/db prune` / `ragent db prune`. Session rows are kept, so
  // an expired transcript stays listed and resumable but reads back empty.
  // `message_days` must be >= 1: a zero-day window would empty the store on
  // every start and is rejected by `validate()`.
  "storage": {
    "retention": {
      "enabled": false,
      "message_days": 14
    }
  },
  // Tool-name alias resolution (spec `uniformtools` FR-015). `aliases: true`
  // enables the built-in table of provider spellings; an object adds explicit
  // `"<provider spelling>": "<ragent tool>"` entries on top of it. An entry
  // naming a retired ragent spelling is a load error (FR-031). `expose_alias_names`
  // additionally advertises each spelling in the outbound tool list, and
  // `wire_trace` dumps each step's redacted request body under
  // `target/temp/wire-trace/` (FR-017). Omitted entirely while all off.
  "tools": {
    "aliases": { "WebFetch": "webfetch" },
    "expose_alias_names": false,
    "wire_trace": false
  },
  "yolo": false,
  "stream": {
    "timeout_secs": 120,
    "initial_response_timeout_secs": 300
  },
  // Connector subsystem (spec `connectors`). `enabled: false` makes the whole
  // subsystem inert (no discovery, catalogue fetch, or connection).
  "connectors": {
    "enabled": true,
    "store_dir": null,
    "stores": {
      "timeout_ms": 10000,
      "max_index_bytes": 2097152,
      "cache_ttl_secs": 3600
    },
    "credentials": {}
  },
  "dirs": {
    "allowlist": ["src/**", "tests/**"],
    "denylist": ["secrets/**", ".env"],
    "allowed_roots": ["/home/alice/shared"]
  },
  // UI internationalisation (spec `openhands` FR-027). Opt-in and omitted
  // entirely while disabled: user-facing strings (the status-bar labels, the
  // `/help` header, and the connector-catalogue footer) then render in English.
  "i18n": {
    "enabled": true,
    "locale": "fr"
  },
  // Execution backend (spec `openhands` FR-001, FR-026). Omitted = `local` (host
  // execution, FR-019). A `podman`/`docker` label runs shell/file/search tools
  // inside a container whose workspace is a named volume (never a host bind
  // mount). `backends` declares the registry entries the container backend reads.
  // A `credentials` list names encrypted-store credentials to inject as container
  // environment variables at spawn time (FR-010); the value is never in this file.
  "execution_backend": "podman",
  "backends": [
    { "id": "sandbox", "name": "Podman sandbox", "kind": "podman",
      "image": "ghcr.io/yourorg/ragent-sandbox:latest", "workspace": "/projects",
      "credentials": ["EXAMPLE_API_KEY"] }
  ],
  // LLM security analyzer (spec `openhands` FR-006, FR-017). Opt-in; a
  // tightening-only layer over the static permission rules (a `deny` is hard,
  // an `allow` never overrides an explicit policy `deny`, and a failure degrades
  // to the normal interactive prompt).
  "security_analyzer": {
    "enabled": true,
    "model": { "provider_id": "ollama", "model_id": "qwen2.5:1.5b" }
  },
  // OpenAI-compatible inbound surface (spec `openhands` FR-012, FR-024, FR-032).
  // Omitted the /v1 endpoints stay behind the native bearer-token check. When
  // `enabled` is true, a `token` (or the ambient `RAGENT_TOKEN`) is required or
  // the server refuses to start the surface (FR-032).
  "openai": {
    "enabled": true,
    "token": "sk-ragent-local"
  }
}
```

### 5.3 Environment Variables

| Variable | Provider / Purpose |
|----------|--------------------|
| `ANTHROPIC_API_KEY` | Anthropic |
| `OPENAI_API_KEY` | OpenAI |
| `GITHUB_TOKEN` | GitHub / Copilot |
| `GEMINI_API_KEY` | Google Gemini |
| `HF_TOKEN`, `HUGGING_FACE_HUB_TOKEN` | Hugging Face |
| `AZURE_AI_FOUNDRY_API_KEY`, `AZURE_AI_FOUNDRY_BASE` | Azure AI Foundry |
| per-entry `api_key_env` (in `azureresources.json`) | Azure Resource |
| `AWS_ACCESS_KEY_ID`, `AWS_SECRET_ACCESS_KEY`, `AWS_SESSION_TOKEN`, `AWS_PROFILE`, `AWS_REGION` | Amazon Bedrock |
| `XAI_API_KEY` | xAI Grok |
| `LANGSEARCH_API_KEY` | Web search (LangSearch) |
| `TAVILY_API_KEY` | Web search (Tavily) |

### 5.4 Compaction Configuration

The `compaction` block controls OpenCode-derived summarisation-based context-window
compaction, which replaces the older Headroom `compression` scheme. It satisfies
FR-008 (auto toggle) and FR-011 (user-overridable threshold, buffer, keep-tokens,
and summary output-token values).

```jsonc
{
  "compaction": {
    // Enable automatic pre-send summarisation. When false, only emergency
    // overflow compaction runs on provider context-overflow errors (FR-008).
    "auto": true,
    // Fraction of the model's context window at which automatic pre-send
    // compaction triggers (0.0–1.0). Default: 0.7 (70 %). This is the
    // recommended, model-independent trigger: 70 % of a 32k window is 22.4k
    // tokens, while 70 % of a 200k window is 140k tokens. The threshold is
    // never below 70 %, even when a fraction-based fallback is used.
    "threshold": 0.7,
    // Response/safety buffer as a fraction of the model's context window.
    // Only used when `threshold` is null. Compaction then triggers when
    // estimated request tokens exceed context_window - max(output_tokens,
    // buffer). Default: 0.10 (10 %, FR-011).
    "buffer": 0.10,
    // Recent conversation turns kept verbatim after compaction.
    "keep": {
      // Fraction of the context window reserved for recent user/assistant/tool turns to preserve.
      // Default: 0.20 (20 %, FR-011).
      "tokens": 0.20
    },
    // Optional dedicated fast/cheap model for the summarisation call
    // (avoids paying the session model's latency for compaction).
    "model": { "provider_id": "ollama", "model_id": "qwen2.5:1.5b" },
    // Maximum tokens requested for the compaction summary output.
    // Default: 1500 (was 4096; smaller budget = proportionally lower latency).
    "summary_tokens": 1500,
    // Truncation limit per tool output serialised into the summarisation prompt.
    // Default: 2000 characters.
    "tool_output_max_chars": 2000
  }
}
```

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `auto` | bool | `true` | Enable automatic pre-send compaction (FR-008) |
| `threshold` | f64 | `0.7` | Fraction of context window that triggers compaction; model-independent |
| `buffer` | f64 | `0.10` | Fallback response/safety buffer as a fraction of the context window when `threshold` is null (FR-011) |
| `keep.tokens` | f64 | `0.20` | Fraction of context window reserved for recent turns preserved verbatim (FR-011) |
| `model` | object | none | Optional fast/cheap model override for the summarisation call (`/compact` and `compact_session` only); the session's primary model is used when unset. The pre-send auto-compaction path always uses the session's primary model and publishes an `AgentNotice` when an override is configured |
| `summary_tokens` | usize | `1500` | Maximum tokens requested for the compaction summary output (FR-011) |
| `tool_output_max_chars` | usize | `2000` | Truncation limit for each tool output serialised into the summarisation prompt |

The compaction trigger threshold is always raised to at least 70 % of the
model's context window, so automatic pre-send compaction never fires on routine
prompts that fill less than 70 % of the available context. Emergency overflow
compaction is not subject to this floor.

The compaction summary output length defaults to `1500` tokens (configurable
via `compaction.summary_tokens`) and tool outputs are truncated to `2000`
characters (`compaction.tool_output_max_chars`) before being serialised into
the summarisation prompt. The prompt itself is capped adaptively at
`min(60,000 chars, context_window * 4 / 2)` so small local models are not
swamped. The summarisation stream has a hard 180-second overall cap plus a
60-second per-chunk stall timeout, and progress is reported every 10 seconds
with the elapsed time and character count received so far. When
`compaction.model` is set, the `/compact` path builds a dedicated client for
the override model instead of reusing the session's primary model. The
pre-send auto-compaction path always uses the session's primary model and
publishes an `AgentNotice` when an override is configured.

The legacy Headroom `compression` block (`compression.enabled`,
`compression.mode`, `compression.auto_threshold`) is no longer parsed: `Config`
has no `compression` field, so the key is silently ignored. Use `compaction`.

Trigger the one-shot compaction manually with the `/compact` slash command
(`/compress` is a deprecated alias — FR-009).

### 5.5 Thinking Configuration

Thinking/reasoning is configured per model:

```jsonc
{
  "provider": {
    "anthropic": {
      "models": {
        "claude-sonnet-4-20250514": {
          "thinking": { "enabled": true, "level": "medium" }
        }
      }
    }
  }
}
```

Levels: `auto`, `off`, `low`, `medium`, `high`. `budget_tokens` is optional.

### 5.6 Message Retention (`storage.retention`)

Ragent keeps the whole transcript forever unless the operator opts in to
age-based expiry. The `storage` section makes that opt-in explicit:

```jsonc
{ "storage": { "retention": { "enabled": true, "message_days": 14 } } }
```

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `retention.enabled` | `bool` | `false` | Run age-based expiry at startup and permit `/db prune` / `ragent db prune`. |
| `retention.message_days` | `u32` | `14` | Window, in days, a message survives. Must be `>= 1`. |

Rules:

- **Off by default, and omitted while off.** With no `storage` section no
  message is ever expired, and a `ragent.json` that never set it round-trips
  byte-identically (`skip_serializing_if = StorageConfig::is_default`).
- **A zero-day window is rejected** by `RetentionConfig::validate()`, because it
  would empty the store on every start - an error the operator can see beats a
  silent wipe.
- **Sessions are kept.** `Storage::prune_messages_older_than` deletes rows from
  `messages_fts`, `message_fts_keys` and `messages` in **one transaction**, but
  never from `sessions`. An expired transcript stays listed and resumable; it
  reads back empty.
- **The cutoff is fallible.** `chrono::Duration::checked_sub_signed` (SEC-049)
  means an extreme window returns an error instead of panicking inside the
  startup path.
- **`--dry-run` cannot disagree with the prune.** `Storage::count_messages_older_than`
  uses the same cutoff computation and the same predicate and deletes nothing.
- **The file does not shrink.** `DELETE` returns pages to SQLite's freelist;
  `ragent.db` keeps its size until a `VACUUM`, which requires exclusive access.

Surfaces: the startup pass (on `spawn_blocking`, so it never stalls startup),
`ragent db prune [--days N] [--dry-run]`, and `/db prune [--days N] [--dry-run]`
in the TUI. A real TUI prune asks for confirmation and the dialog opens on `No`,
so a stray `Enter` deletes nothing.

See `docs/sqlite.md` for the storage-level detail (pragma table and the
retention section).

### 5.7 Telemetry (`telemetry.otel`)

Observability export is opt-in and disabled by default. The `telemetry.otel`
block configures an OTLP exporter for **metrics** and **logs** under one master
switch: turning telemetry on exports both.

```jsonc
{ "telemetry": { "otel": {
    "enabled": true,
    "endpoint": "http://localhost:4318",
    "protocol": "http",
    "export_interval_seconds": 30,
    "export_timeout_seconds": 10,
    "logs": { "enabled": true, "level": "warn" }
} } }
```

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `otel.enabled` | `bool` | `false` | Master switch for the whole subsystem. |
| `otel.endpoint` | `String` | `"http://localhost:4318"` | Collector **base** URL (`http://` or `https://`). |
| `otel.protocol` | `OtelProtocol` | `http` | `http` or `grpc`. |
| `otel.export_interval_seconds` | `u64` | `30` | Batch interval shared by both signals. |
| `otel.export_timeout_seconds` | `u64` | `10` | Per-export request timeout shared by both signals. |
| `otel.cardinality_limit` | `usize` | `1000` | Distinct attribute combinations per metric before overflow into the `unknown` bucket. |
| `otel.logs.enabled` | `bool` | `false` | Ship `tracing` events over the OTLP logs signal. Written by `/telemetry on|off` alongside `otel.enabled`. |
| `otel.logs.level` | `String` | `"info"` | `all`/`trace`/`debug`/`info`/`warn`/`error`; a floor for the **remote** export only (`all` exports every level the layer observes). |

Rules:

- **The endpoint is a base URL, and ragent resolves the signal path itself.**
  `otlp_http_signal_endpoint` appends `/v1/metrics` or `/v1/logs` (trailing
  slashes collapsed, query string and fragment preserved), keeps an existing
  base path (`https://gw.example.com/otel`), and returns a value that already
  names a signal path (`.../v1/metrics`) unchanged, so both documented forms
  reach a stock collector. Passing the configured value straight to
  `opentelemetry-otlp`'s `with_endpoint` posts to it verbatim, which a stock
  collector answers with `404`.
- **The logs signal composes, it does not redirect.** A
  `tracing_subscriber::Layer` (`ragent_telemetry::logs::TracingLogsLayer`) is
  installed beside the existing sinks, so the TUI log panel (or stderr in a
  headless run) keeps every event `--log-level` admits and only the remote
  export is filtered by `logs.level`.
- **One switch, both signals.** `/telemetry on` enables metrics **and** logs
  (writing `otel.logs.enabled: true` beside `otel.enabled: true`) and
  `/telemetry off` stops both; there is no separate logs on/off command. Log
  export still requires `otel.enabled: true`, so a hand-edited config with
  `logs.enabled: true` and `otel.enabled: false` records the intent and ships
  nothing until telemetry is on (FR-040).
- **A logs failure never takes metrics down.** An exporter that cannot be built
  is logged at `warn` and the metric signal keeps running (FR-041).
- **Both signals are flushed and shut down together**, so the last records of a
  run are not lost in the batch buffer.
- **Every exported body and attribute is guarded.** Bodies pass through
  `ragent_types::sanitize::redact_secrets` (over 16 KiB truncated with a
  `... [truncated]` marker) and attributes through the metric sensitive-data
  guard.
- **Export failures are visible.** `ragent_telemetry::export_health` classifies
  every metric export outcome; the TUI turns each into an `Agent Notice` naming
  the consecutive-failure count, the reason and the masked endpoint, and five
  consecutive failures switch telemetry off (persisting `enabled: false`)
  rather than retrying a dead collector forever.
- **A failure streak belongs to one pipeline generation.** `ExportHealth`
  carries a generation number bumped by `begin_generation` on every
  reconfigure; each `ObservedMetricExporter` records the generation it was built
  in, and a result from an earlier generation is forwarded unchanged but neither
  counted nor announced, so a retired pipeline's own final export cannot reset
  or extend the live streak.
- **A reconfigure never blocks the caller.** The live pipeline is detached into
  a `RetiringPipeline` (Prometheus handle aborted, logs cell cleared, providers
  taken) under the runtime lock and shut down on a named background thread
  (`otel-retire`) once the lock is released, so `/telemetry off` against a dead
  collector does not stall the TUI for the export timeout. `shutdown()` joins
  the outstanding retire threads outside any lock, so an exit right after
  `/telemetry off` still delivers the final export.
- **Both signals fail independently.** `flush()` and `shutdown()` attempt the
  logs signal and the metric signal regardless of the other's error (the first
  error is still returned), so an unhealthy logs endpoint cannot drop the final
  metric batch.
- **The switch persists and reports honestly.** `/telemetry on|off` return a
  user-facing error - reported as `[err]` - when the stored block is invalid,
  the save fails, or the live subsystem refuses the change, instead of printing
  `[ok]` unconditionally; an invalid enabled block is refused before it is
  written. Saving removes the deprecated `experimental.open_telemetry` alias so
  the state survives a restart, and creates the user-global config directory
  when it is absent (without it the lock file cannot be created and every save
  fails on a fresh install).
- **`/telemetry setup` merges, it does not rebuild.** The dialog covers only
  endpoint, protocol, interval, timeout, internal port and log level, so the
  draft is built with `..stored_block` and prefilled from the same block; the
  fields it cannot show (`service_name`, `resource_attributes`, the per-metric
  toggles, `cardinality_limit`) survive a save. A changed `cardinality_limit`
  applies on reconfigure (the cardinality cache is rebuilt), not at the next
  restart. The protocol toggle follows the transport with the endpoint's OTLP
  default port (4318 for HTTP, 4317 for gRPC), matching the URL authority only
  (the authority ends at `/`, `?` or `#`), so a custom port and a path, query or
  fragment containing the digits are left alone.
- **The gRPC transport needs TLS off-loopback.** A cleartext non-loopback gRPC
  endpoint is refused when TLS is unavailable.

Surfaces: `/telemetry help|on|off|setup|counters` plus `/telemetry logs [<level>]`
(the level only - enabling and disabling is `/telemetry on|off`) in the TUI, the
`Logs level` row in `/telemetry setup` (an empty field keeps the default
`info`; saving the dialog turns both signals on), `/telemetry counters` and the
telemetry panel, and `docs/howtos/config.md` for the per-field reference.
`App::telemetry_logs_state` supplies the one four-state label (`shipping`,
`configured but not attached`, `off (logs signal disabled)`, `off (telemetry is
disabled)`) that `/telemetry logs` and the panel both render, and `/telemetry
logs warning` stores the canonical `warn`. The telemetry panel reads the live
subsystem's config rather than `Config::load()`, so it costs no disk I/O per
frame and describes this run; it opens with a `Status` block naming whether
export is on, the log-signal state at its level, and the export-failure streak
once any failure has been recorded. `OtelConfig::validate` reports an empty
endpoint once (the URL check is chained under the non-empty check).

See `specs/otel/` for FR-036..FR-042 and NFR-008.

### 5.8 GCF Encoding Configuration

The `gcf` block controls GCF (Graph Compact Format) encoding of eligible JSON
tool results in the LLM view. GCF is a token-efficient, lossless
key=value text format: when enabled, tool results of at least 200 characters
that parse as a JSON object or array are re-encoded inside a
`[BEGIN GCF generic]` ... `[END GCF]` labelled block, but only when the GCF
output is at least 10 % smaller than the raw JSON (results that save nothing
stay raw). Any encode failure falls back to the raw JSON, so the model always
receives the full information content (FR-007). TUI rendering, the activity
log, memory extraction, hooks, and compaction keep seeing the raw JSON; only
the model-facing copy is re-encoded (FR-005). While GCF is enabled the system
prompt also gains a short reading primer so the model can decode the block
format (FR-006).

```jsonc
{
  // Default off; the whole section is omitted from the saved file while
  // disabled (skip_serializing_if default rule, FR-001).
  "gcf": {
    "enabled": false
  },
  "storage": {
    "retention": { "enabled": false, "message_days": 14 }
  }
}
```

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `enabled` | bool | `false` | Enable GCF encoding of eligible JSON tool results in the LLM view (FR-004) |

Toggle at runtime with `/gcf on` / `/gcf off` (persists to the loaded config
source), inspect with `/gcf show`, and view usage with `/gcf help` (FR-002,
FR-003). `Alt+G` toggles GCF with the same persist semantics, and a status-bar
indicator on line 2 (compression-clamp icon, left of the codeindex icon) shows
the enabled/disabled state.

---

## 6. Terminal User Interface (TUI)

### 6.1 TUI Layout

The TUI is a ratatui full-screen interface with these panels:

| Panel | Purpose |
|-------|---------|
| Chat | Conversation stream with markdown and syntax highlighting |
| Log | Step-numbered tool calls with pretty-printed JSON |
| Status bar | Provider/model, context usage, compression indicator, YOLO state; service states use plain ASCII text tags (`CDX:`, `LOG:`, `AUTO:`, `EDIT:`, `TELE:`, `YOLO:`, `GCF:`, `BUSY: `), and message-window tool categories use ASCII marker prefixes (`[file]`, `[dir]`, `[search]`, `[exec]`, `[git]`, `[team]`, ...) |
| Teammate strip | One row of pills under the status bar, one per active teammate (status glyph, clipped name, focused highlight, `Alt+^v:cycle` hint); omitted when no team is active (spec `agentui` FR-027) |
| Agents window | `Agents`-button popup listing the primary agent and an ancestor-indented sub-agent tree with status, elapsed time, step count, `[stop]`/`[x]` status glyphs, and a background-shell section; openable from the keyboard with `Alt+A` (the chord resolves to `InputAction::ToggleAgentsWindow` and calls the same `App::toggle_agents_window` the button click does, so both share one visibility path, one no-subject no-op and one redraw flag - spec `agentssetup` FR-022); keyboard-first: a block cursor (`Up`/`Down`/`PgUp`/`PgDn`) selects a row, `Space` toggles suspend/resume, `Del` arms the kill confirmation, `Enter` opens the row's output view, and clicking a row selects it; the FR-077 key legend sits in the footer (spec `agentui` FR-006..FR-016) |
| Teams window | `Teams`-button popup listing the lead plus every teammate with focus markers, status words from `MemberStatus::display_label()`, model column (inherited global model suppressed), and claimed/done/sent/recv counters; the block cursor plus `Enter`/`Space`/`Del` drive focus, suspend and shutdown; clicking a teammate selects it (spec `agentui` FR-017..FR-026) |
| Input | Command-line with slash-command autocomplete |
| Permission dialog | Modal approval dialog with live countdown (120 s) |
| Question dialog | Modal user question (free-text or multiple-choice) |
| Row-action dialog | Yes/No confirmation for destructive row actions (sub-agent kill, background-shell cancel, teammate suspend/shutdown) drawn over the windows, keyboard and mouse answered (spec `agentui` T-009) |

### 6.2 Slash Commands

| Command | Description |
|---------|-------------|
| `/quit` / `/exit` | Exit ragent |
| `/provider` | Open provider setup dialog (always allows editing the stored key) |
| `/model` | Select model (jumps to model list when a provider is already configured) |
| `/agent` | Select agent |
| `/agents` | List loaded agents and diagnostics |
| `/websearch show` | Show web-search engine diagnostics (enabled / in-use / failed) |
| `/websearch help` | Show `/websearch` subcommand help |
| `/websearch test` | Test configured web-search backends |
| `/webapi enable\|disable\|help` | Manage the HTTP REST API |
| `/tools` | List every registered tool: `/tools list` (alias `show`) prints the registry with each tool's source and description, `/tools help` prints help |
| `/codeindex on\|off` | Enable/disable code index |
| `/codeindex lang <language>` | Filter code index by language |
| `/gcf on\|off\|show\|help` | Toggle GCF token-efficient tool-result encoding; `show` reports state and source |
| `/compact` | Summarise and compact the conversation history (one-shot LLM summarisation; FR-009). `/compress` is a deprecated alias |
| `/memory` | Structured-memory panel and summary: `/memory` / `/memory show` (same), `/memory clear` (a `Yes`/`No` dialog clears this project's memories), `/memory help` |
| `/db` | Session database: `/db prune [--days N] [--dry-run]` deletes messages older than the retention window (the session row is kept; a real prune arms a `Yes`/`No` dialog that opens on `No`), `/db help` (also the bare form) |
| `/yolo` | Toggle YOLO mode (`on`/`off` set and persist the state, `status` reports it, a bare `/yolo` toggles; the toggle writes the user-global config, or the `--config` file when one was loaded) |
| `/task` | Toggle the TASKS side panel (also Alt+T); `/task list`, `/task help` |
| `/team create <name>` | Create a team |
| `/team open <name>` | Re-open existing team |
| `/team close` | Close current team |
| `/team clear` | Reset current team state |
| `/team delete <name>` | Delete team on disk |
| `/team cleanup` | Tear down current team |
| `/team message ...` | Send team message |
| `/swarm <prompt>` | Decompose prompt into parallel subtasks |
| `/swarm status` | Show active swarm tasks |
| `/swarm kill` | Cancel active swarm |
| `/spawn <agent> <prompt...>` | Launch a detached fire-and-forget background sub-agent from the input line (spec `spawnagent`); not listed by `list_agents`, not awaitable via `wait_agents`, its result never injected back into the chat |
| `/autopilot on\|off` | Toggle autonomous mode |
| `/spec create\|specify\|plan\|tasks\|update\|add\|feedback\|jtbd\|list\|search\|show\|validate\|status\|task\|impl\|coverage\|activate\|deactivate\|delete` | Spec lifecycle and SDD commands |
| `/plugins list\|add\|remove\|enable\|disable\|test\|stores\|marketplace\|help` | Manage sandboxed Codex/Claude plugins; `/plugins list [--verbose]` opens an interactive panel (the plugin table pinned at the top - a fixed column heading and separator - above a scrollable block-cursor list, with the capability blocks listed at the end of the same body; `Up`/`Down`/`Home`/`End` move the cursor, `Space` toggles the highlighted plugin's enabled state (enable/disable, reporting the outcome on a feedback line), and `Esc` closes); `/plugins test` runs an isolated harness; `/plugins codex [query] [--category <name>] [--refresh]` and `/plugins claude [query] [--category <name>] [--refresh]` browse each store's official marketplace with a left-hand category navigator (`ALL` plus the distinct categories the fetched index declares) operated by keyboard and mouse, its own document shape normalised by that store's `StoreProvider`, and each row showing the plugin id, version, the marketplace it came from, and the description; `/plugins stores [--check]` reports each store's effective endpoint and its source, and with `--check` also contacts each store to report availability and plugin count; `/plugins marketplace show` lists the marketplaces configured for both stores in one section each (Codex, Claude) as a name/url/description table; `/plugins marketplace <codex|claude> add <source> [--force]|remove <name>|list` manages additional marketplaces (from an `owner/repo` slug, an SSH remote, an `https://` URL, or a local directory) in the user-global `plugins.stores.marketplaces`, which the two browse panels fetch and merge into the one panel; `/plugins add` accepts a `git+<https-url>#<ref>[:<subpath>]` git source; CLI parity via `ragent plugins` |
| `/connectors list\|claude\|add\|remove\|enable\|disable\|connect\|disconnect\|auth\|test\|stores\|marketplace\|help` | Manage MCP-backed connectors (named integrations carrying a category, an auth shape, and one or more MCP servers); `/connectors claude [query] [--category <name>] [--refresh]` opens the interactive catalogue browser (the single catalogue surface), which merges the base catalogue endpoint and every marketplace added with `/connectors marketplace claude add`, shows each row's id, category, the marketplace it came from, and its description, and resolves blank descriptions from each entry's own repository manifest off-loop with a `loading details...` title hint; `/connectors marketplace show` lists the marketplaces configured for the Claude catalogue and `/connectors marketplace claude list\|add <source> [--force]\|remove <name>` manages them in the user-global `connectors.stores.marketplaces`; `/connectors test` runs an isolated connect-and-invoke harness; `/connectors stores [--check]` reports each catalogue endpoint's provenance; `/connectors add` accepts a catalogue id, a local directory, a local `.zip`/`.tar.gz`, or an `https://` package URL; CLI parity via `ragent connectors` (spec `connectors`, section 19C) |
| `/backend [<kind>]` | Open the execution-backend switcher (active backend + each backend's health) or switch to a kind/id directly; `/backend help`. See section 19D (spec `openhands` FR-008) |
| `/security on\|off\|status\|revert\|help` | LLM security-analyzer mode (spec `openhands` FR-006, FR-017). `security-analyzer` is accepted as an alias. See section 19D.7 |
| `/automation [list]\|runs <id>\|run <id>\|help` | Manage the automation service (spec `openhands` FR-013, FR-014, FR-018). See section 19F |
| `/i18n on\|off\|status\|list\|<locale>\|help` | UI internationalisation: enable translation, set the locale, list installed catalogs (spec `openhands` FR-027) |
| `/queue list\|clear\|next\|help` | Inspect the message input queue (messages and slash commands submitted while the agent executes; spec `inputqueue` FR-013/FR-017 amendment) |
| `/toolchain list [lang] [--json]` | Language toolchain report |
| `/blueprints help\|list` | List installed team blueprints |
| `/skill help\|list\|<name> [args]` | Load or inspect skill packs: list them, or load and invoke `<name>` |
| `/osinfo show [--no-probe]\|help` | Read-only host OS/hardware report; routes through the `os_info` tool's collector/renderer (spec `osinfo`) |
| `/research create\|list\|show\|search\|cluster\|archive\|delete\|update` | Research commands; `create` supports `--from-file`, `--from-url`, `--use-low-relevance`, `--papers`, `--oa-enable`/`--no-oa`, `--max-queries N`, `--max-concepts N`, `--max-findings N`, `--url-cloak`, `--age <SPAN>` |
| `/config show` | Show resolved configuration |
| `/config save` | Snapshot global `ragent.json` to `saves/` (atomic, timestamped) |
| `/config list` | Interactive picker to restore a saved backup |
| `/init config` | Create a default `ragent.json` in the global config directory |
| `/startup` | Show per-stage startup timing instrumentation |
| `/prompt help\|primary [agent]\|subagent [agent]\|list\|<agent>` | Read-only agent system-prompt inspector (no LLM call) |
| `/telemetry help\|on\|off\|setup\|counters\|logs` | Manage OpenTelemetry metrics and log export |
| `/dirs` | Show configured writable directories |
| `/profile` / `/status` / `/mouse` | UI preferences |
| `/skill` / `/skills` | Load or inspect skill packs |
| `/mcp [status]\|config\|discover\|connect <id>\|disconnect <id>` | MCP server status, discovery location report, and enable/disable |
| `/update` / `/update install` | Auto-update (reserved; not implemented) |

### 6.3 TUI Component Architecture

```mermaid
graph LR
    App[App State] --> EventLoop[Event Loop]
    EventLoop --> Terminal[Terminal]
    App --> Chat[Chat Widget]
    App --> Log[Log Widget]
    App --> Input[Input Widget]
    App --> Status[Status Bar]
    App --> Windows[Agents / Teams Windows + Output View Legend]
    App --> Dialogs[Permission / Question Dialogs]
    EventBus --> App
```

**Figure 7:** TUI Component Architecture — UI layout and event wiring

### 6.5 Plugin-Store Category and Marketplace Navigators

The plugin-store browse panel opened by `/plugins codex` and `/plugins claude`
(spec `catnav`, `pluginstores` FR-052) splits its inner area into a left-hand
navigator column and the existing result column, and the navigator column is split
**vertically** into a **category navigator** (upper half) and a **marketplace
navigator** (lower half). Each navigator lists the `ALL` sentinel first, then the
distinct non-empty values the fetched store index declares (sorted
case-insensitively, folding case-only duplicates): the categories from each entry's
`category`, and the marketplaces from each entry's `marketplace` label. An index that
declares no category or no marketplace renders `ALL` only for the corresponding
navigator and still browses normally (FR-018). Every navigator row appends the number
of loaded entries under that value as `Name (N)`, counted over the full fetched index
so the count is stable while a query or filter narrows the result list; the `ALL` row
counts the whole index (FR-053). The active category and marketplace are
shown in the panel title (`-- category <name> -- marketplace <name>`) and painted in a
distinct colour; the row under each navigator's cursor carries a block cursor
independent of the result-list cursor.

The category navigator additionally carries an **`Installed` pseudo-category** in
row 1, directly below `ALL` and above the index's own categories (spec `pluginstores`
FR-054). Selecting it narrows the results to the plugins already present in the store
scan, matched on the plugin id rather than on a declared category, so an installed
plugin that declares no category still appears; the row's `Name (N)` count is the
number of loaded entries the scan matches. It composes with the query and marketplace
filters like any other navigator row, and `c` clears back to `ALL`.

The panel has three focusable panes and is fully operable by keyboard: `Tab`/`BackTab`
cycle focus forward/backward through the result list, the category navigator, and the
marketplace navigator; `Up`/`Down` move the focused pane's cursor (moving a navigator
cursor applies the highlighted value on the same keypress and scrolls that navigator to
keep the cursor visible); `Enter` over a navigator
applies the value and hands focus back to the result list (never installing),
and `c` clears the focused navigator's filter to `ALL`. Mouse input is an additive
convenience: a
left-click on a navigator row selects and applies it and focuses that pane, a
wheel over a navigator scrolls its viewport without changing the active filter,
and a left-click on a result row moves the result cursor while keeping the active
filters. Every mouse path is a convenience over the keyboard path, so a terminal
without mouse reporting still reaches every value (FR-023), and no navigator
path ever installs, downloads, or executes plugin JavaScript (FR-024, FR-025).

Selecting a category or a marketplace re-derives the visible result set from the full
fetched index
in memory, resets the result cursor to the first survivor, and issues no network
request (FR-014, FR-028); the category and marketplace predicates compose with each
other and with the free-text query
(a result is visible only when it matches all active filters). An entry with a blank
marketplace is visible only under `ALL`. A value no fetched entry
declares is never presented as a row and a selection that vanishes after a re-fetch
falls back to `ALL` (FR-027). A `--category <name>` launch argument holds the
requested filter until the index lands, then validates it against the derived rows
exactly like an in-panel selection, reporting an unknown name in the footer while
leaving every row visible (FR-022).

Each result row renders the plugin id, its version, **the marketplace the entry came
from**, and its description, and the panel clamps to a 140-column ceiling (a third wider
than the original 100) so the marketplace column has room. The marketplace column names
the added marketplace an entry was fetched from (the `name` it was added under, matched by
resolved URL), or the store label (`Codex`/`Claude`) for the store's base endpoint, so a
merged multi-marketplace panel shows each row's source (FR-048). The marketplace and
description columns share the flexible remainder, so a longer marketplace name is shown
in full when the terminal has room and clipped without pushing the fixed id/version
columns or the one-column installed lane off the row when it does not. That lane sits
beside the `> ` cursor gutter and carries a bold green `*` on a row already installed;
every other row paints a blank there so the columns stay aligned.

The panel's bottom inner row carries a **search box** (FR-051): a leading `search:` label,
the live filter field, and a right-aligned `[x] clear` button. The field mirrors the
browser's query byte-for-byte, so the box and the result filter cannot drift; a query
wider than the field is clipped from the left so the newest characters stay visible. A
left-click on the clear button clears the search filter through the same in-memory path
as the keyboard editor, restoring the full result set while leaving the active category
untouched; the button is inert when the query is already empty. A row too narrow to hold
both the label and the button drops the button rather than squeezing the field below a
usable width. The panel is framed with the same Unicode box-drawing borders every other
panel uses, over a plain-ASCII body.

A vendor marketplace is a *listing* rather than a native index: the Codex catalogue
carries neither a version nor a description, and the Claude catalogue carries a
description but only sometimes a version. After the index lands, the panel resolves each
plugin's `version` and `description` from the plugin's **own manifest**, fetched off the
event loop from the raw URL derived from its install source when that source is a
`github.com` git source - `https://raw.githubusercontent.com/<owner>/<repo>/<ref>/<subpath>/<manifest>`,
with the store's dialect-appropriate manifest name (`.codex-plugin/plugin.json` for
Codex, `.claude-plugin/plugin.json` for Claude) tried first and a bare `plugin.json` as a
fallback. The provider transform (`transform_vendor_entry`) reads what the listing
declares, including the entry's `interface.displayName`/`shortDescription`/
`longDescription`, and leaves an absent version blank rather than fabricating a `0`. The
enrichment (`ragent-plugins/src/store_enrich.rs`) is best-effort and never fatal (a
missing manifest, a transport failure, non-JSON bytes, or an oversized body yields no
update), runs on a bounded thread pool, caches each URL's result (including a negative
result) across opens, and is injectable so the tests run offline. The title shows
`loading details...` while it runs; a version that stays unknown renders an explicit `-`
rather than a blank cell, and enrichment never overwrites a value the index declared.

The result list itself is rendered in ascending alphabetical order of plugin id,
case-insensitively with a byte-wise tiebreak, derived from the id alone rather than the
order the store index declared the entries in or the order the configured marketplaces
were merged (FR-050). The sort is applied when the index lands, before the filter is
derived, so a free-text query and an active category both narrow an already-sorted set
and the survivors keep their alphabetical order. One comparator is shared by every store
surface (`compare_store_ids`/`sort_store_entries` in `ragent-plugins`), mirroring the
connector system's `compare_connector_ids`. The navigator's category rows keep their own
case-insensitive sort, and a case-insensitive category collision (for example `Utility`
and `utility`) keeps the first spelling the index declares, so the result sort never
relabels a category.

### 6.4 Permission Dialog Countdown

The permission dialog displays a live countdown:

- Format: `M:SS` (e.g., `1:45`)
- Shows `EXPIRED` when the 120-second timeout is reached
- The event loop redraws continuously so the timer decrements without requiring keyboard input

---

## 7. HTTP Server & API

### 7.1 Starting the Server

```bash
ragent serve --addr 127.0.0.1:3000   # --addr defaults to 127.0.0.1:3000
```

### 7.2 REST Endpoints

| Method | Endpoint | Description |
|--------|----------|-------------|
| GET | `/health` | Health check |
| GET | `/openapi.json` | OpenAPI 3.1 document (public, no auth) |
| POST | `/sessions` | Create a session |
| GET | `/sessions` | List sessions |
| GET | `/sessions/{id}` | Get session details |
| POST | `/sessions/{id}/messages` | Send a message (returns SSE stream) |
| GET | `/events` | SSE event stream |
| GET | `/providers` | List providers |
| GET | `/v1/models` | OpenAI-compatible model list (spec `openhands` FR-003) |
| POST | `/v1/chat/completions` | OpenAI-compatible chat completion; `stream:false` returns JSON, `stream:true` returns `chat.completion.chunk` SSE (spec `openhands` FR-003, FR-011, FR-012) |
| POST | `/auto/{id}` | Public automation webhook ingress: enqueue a run with the request body as context (spec `openhands` FR-013) |
| GET | `/automation` | List configured automations (bearer) |
| GET | `/automation/runs/{id}` | List an automation's run history (bearer) |
| POST | `/automation/{id}/run` | Enqueue a manual run now (bearer) |
| GET | `/research` | List research items |
| POST | `/research` | Create + run a research gathering session (returns `202 Accepted` with `Location` header) |
| GET | `/research/{name}` | Show one research item (supports `?full=true` for extended metadata) |
| DELETE | `/research/{name}` | Delete a research item (requires `?confirm=delete-{name}`) |
| GET | `/research/{name}/events` | SSE stream of live research events for a background run |

#### 7.2.1 OpenAPI Contract and Typed Client (spec `openhands` FR-007)

The full REST surface is described by an OpenAPI 3.1 document served at
`GET /openapi.json`. The document is rendered from a single route table
(`crates/ragent-server/src/routes/openapi.rs::ROUTES`) so it cannot drift from the
served routes; a companion integration test asserts the table and the router agree
in both directions (it scrapes every `.route(...)`/`.nest(...)` literal from
`src/routes/` and probes every documented parameterless `GET` through the live
router). Every protected operation declares the native bearer `security` scheme;
`/openapi.json` itself is served without the token because the document carries no
runtime data or secrets.

The same route table generates a dependency-free typed TypeScript client, checked
in at `docs/openapi/ragent-client.d.ts` and reproduced by:

```bash
ragent openapi              # print the OpenAPI 3.1 document
ragent openapi --client     # print the generated typed TypeScript client
```

For `ragent openapi`, tracing is routed to stderr so the machine-readable stdout
stays clean.

MCP servers configured with the `http` transport (`"type": "http"`, `"url"`)
connect over Streamable-HTTP request/response alongside the existing `stdio`
transport (FR-025).

A server entry's `"type"` is optional and defaults to `"stdio"`, matching the
MCP convention every other client follows: a command-launched server may omit
it, and a missing key loads rather than failing. The same default is applied on
every path that parses a server definition — the `mcp` section of `ragent.json`,
plugin `mcpServers` bridges, and connector `servers`/`remote` entries — so a
definition that declares no transport is a stdio server everywhere. Serde still
serializes an explicit `"type"`, so a config written back names its transport.

### 7.3 SSE Events

The server streams the following event types:

| Event | Description |
|-------|-------------|
| `assistant_chunk` | Token or reasoning block |
| `tool_requested` / `tool_executed` | Tool lifecycle |
| `permission_requested` / `permission_replied` | Permission flow |
| `question_requested` / `question_answered` | Question tool |
| `compression_started` / `compression_finished` | Context compaction lifecycle |
| `subagent_*` / `teammate_*` | Multi-agent lifecycle |
| `agent_notice` | Status messages |
| `research` | Research session events (phase, web capture, synthesis, analysis, done) — streamed via `GET /research/{name}/events` |

### 7.4 Authentication

The HTTP API uses Bearer token authentication:

```bash
curl -H "Authorization: Bearer $RAGENT_TOKEN" ...
```

`RAGENT_TOKEN` is read from the environment. If unset, the server may allow
local requests without authentication depending on build configuration.

The OpenAI-compatible surface (`/v1/models`, `/v1/chat/completions`) uses the
same bearer token as the native API (FR-024) - ragent runs one server token. The
`openai.token` config key (or the ambient `RAGENT_TOKEN`) supplies it. When the
surface is explicitly enabled (`openai.enabled: true`) without a token, the
server **refuses to start** rather than serve the surface unauthenticated
(FR-032).

#### 7.4.1 OpenAI-compatible surface (spec `openhands` FR-003, FR-011, FR-012, FR-038)

`POST /v1/chat/completions` bridges the last user message to one
`SessionProcessor` turn and answers in the OpenAI dialect:

- `stream:false` returns one `chat.completion` JSON object (FR-011).
- `stream:true` returns a `text/event-stream` of `chat.completion.chunk` events -
  an opening `delta.role = "assistant"` chunk, one chunk per assistant
  `TextDelta`, and a terminal chunk carrying the mapped `finish_reason`
  (`stop` / `length` / `tool_calls` / `content_filter`) - terminated by a
  `data: [DONE]` sentinel (FR-012).

Both paths run every tool call through the same permission layer as a native
request (`auto_approve` is never forced); a tool needing confirmation raises the
ordinary `PermissionRequested` prompt and, with no interactive client attached,
times out to a denial (FR-038).

### 7.5 HTTP API Request Flow

```mermaid
sequenceDiagram
    participant Client
    participant Axum
    participant SP as Session Processor
    participant LLM
    Client->>Axum: POST /sessions/{id}/messages
    Axum->>SP: dispatch message
    SP->>LLM: streaming request
    LLM-->>SP: chunks / tool calls
    SP-->>Axum: events
    Axum-->>Client: SSE stream
```

**Figure 8:** HTTP API Request Flow — REST + SSE lifecycle

---

# Part III: Data & Knowledge Systems

---

## 8. Code Index

### 8.1 Overview

The code index provides fast, local, read-only code intelligence. It is built
on tree-sitter parsing and Tantivy full-text search, with a SQLite metadata
store.

### 8.2 Supported Languages

Rust, Python, TypeScript/JavaScript, Go, C/C++, Java, OpenSCAD, Terraform,
CMake, Gradle, Maven, and more (15+ grammars compiled-in).

### 8.3 Architecture

```mermaid
graph LR
    Scan[File Scanner] --> Parse[Tree-sitter Parser]
    Parse --> Store[(SQLite Metadata)]
    Parse --> Index[Tantivy FTS Index]
    Query[Tool Query] --> Index
    Query --> Store
```

**Figure 9:** Code Index Pipeline — File scan → parse → index → search

### 8.4 Code Index Tools

| Tool | Purpose |
|------|---------|
| `codeindex_search` | Full-text search across indexed code |
| `codeindex_symbols` | Search for symbols by name |
| `codeindex_references` | Find references to a symbol |
| `codeindex_dependencies` | Query file-level imports/dependents |
| `codeindex_status` | Show index status |
| `codeindex_reindex` | Trigger a full re-index |

All code index tools are **hardwired always-allowed** because they are read-only
and local-only.

### 8.5 Semantic Code Graph

The code index also maintains a **semantic code graph** — a typed edge graph
over indexed symbols that captures relationships such as `calls`, `imports`,
`inherits`, `references`, `mixes_in`, and `implements`. Edges are either
`EXTRACTED` (from tree-sitter parse data) or `INFERRED` (heuristic).

The graph is built on demand via `/codeindex graph build` (or
`/codeindex graph lang <language>` for a per-language subgraph) and persisted
in the `graph_edges` SQLite table. Graph builds run on a dedicated OS thread
(`codeindex-graph-build`) via `CodeIndex::spawn_graph_build`, so the TUI event
loop stays responsive while the derivation runs; a double-build guard refuses
overlapping builds.

Graph builds use a phased lock discipline: the store mutex is taken only for
two brief windows - a read-only snapshot of the derivation inputs (symbols,
refs, imports, files) and the final single-transaction persist into
`graph_edges`. The CPU-heavy derivation runs with no locks held, so FTS search
and the other store readers remain available while the graph builds; only the
short write transaction touches the `graph_edges` table.

#### Graph Tools

| Tool | Purpose |
|------|---------|
| `codeindex_godnodes` | Top-N most-connected symbols (highest degree) |
| `codeindex_path` | Shortest path (by hop count) between two symbols |
| `codeindex_explain` | Node metadata and incoming/outgoing edges for a symbol |
| `codeindex_communities` | Community detection via label propagation |

All graph tools use non-blocking `try_*` variants and return a
`codeindex_busy` response when the index is locked (e.g. during a reindex).
Graph name resolution (`codeindex_path`, `codeindex_explain`) prefers exact
name matches over the underlying substring query and ranks definition kinds
(struct/function/trait/class/enum/interface) above impl/module containers, so
a trait named `CachedSessionProcessor` no longer shadows the struct
`SessionProcessor`.

#### Graph Status

`/codeindex show` reports graph-level statistics alongside index stats:
edge count (total, extracted, inferred), node count, per-kind edge counts,
and community count. While a graph build is in flight, `/codeindex show`
reports `**Graph:** building...` with per-file progress (`done/total`), and
the `codeindex_status` tool reports `graph_state: "building"` plus
`graph_building`/`graph_done`/`graph_total` metadata. `codeindex_status`
never blocks on the store lock: when the lock is held it returns an immediate
busy report built from the lock-free progress atomics (metadata `busy: true`,
`error: "codeindex_busy"`). The `IndexStats` type carries `graph_total_edges`/
`graph_nodes`/`graph_communities` so status surfaces can report "graph built"
vs "not built" without a separate graph query.

### 8.6 Code Index Status Indicators

The TUI status bar (second line, top-right) shows two bold busy indicators
while the code index works:

- `IDX` glyph (`idx`) -- the FTS/store indexes are being updated (a reindex
  holds the store/FTS locks or the reindex progress counters are active).
- `GRAPH` glyph (`graph`) -- the semantic edge graph is being (re)built.

Both busy tags render in every responsive mode and are polled lock-free from
`CodeIndex` atomics (`reindex_progress()` / `graph_busy()` /
`graph_build_progress()`) by `App::refresh_code_index_stats`, so they stay
live even while the index mutexes are held by the background work.

Attribution rule: when the store lock is unavailable (`try_status()` returns
`None`) and `graph_busy` is set, the lock is attributed to the graph build --
the `idx` indicator is not latched and is cleared if stale, so the two
indicators track their own phases and never vanish simultaneously at build
completion.

### 8.7 Incremental Updates

A file watcher detects changes and incrementally updates the index. Language
filtering is available via `/codeindex lang <language>`.

---

## 9. Memory System

### 9.1 Three Tiers

| Tier | Storage | Use Case |
|------|---------|----------|
| **File blocks** | Markdown files in `.ragent/memory/` or `~/.ragent/memory/` | Long-form notes, patterns, preferences |
| **Structured store** | SQLite via `memory_store` / `memory_recall` | Facts, errors, workflows with tags/confidence |

Recall over the structured store is full-text (SQLite FTS5); there is no
embedding-based similarity tier.

### 9.2 Memory Operations

| Tool | Purpose |
|------|---------|
| `memory_read` | Read a memory block file |
| `memory_write` | Write/append to a memory block file |
| `memory_replace` | Replace a string in a named block |
| `memory_store` | Store a structured memory entry |
| `memory_recall` | Full-text search structured memories |
| `memory_search` | Semantic/keyword search across memories |
| `memory_forget` | Delete memories by filter |
| `memory_migrate` | Split a flat MEMORY.md into blocks |
| `conversation_search` | Keyword/turn-range/stats search over the current session |
| `session_search` | Cross-session full-text search with filters and context |

### 9.3 Automatic Extraction

After each assistant turn the system can extract:

- Project facts and patterns
- Errors and their resolutions
- User preferences
- Workflows and standard operating procedures

Extracted memories are tagged with category, confidence, and source.

### 9.4 Decay, Compaction, and Knowledge Graph

- Memories decay in relevance over time unless accessed or reinforced.
- Compaction summarises old memories.
- A lightweight knowledge graph links related memories by tag and reference.

---

## 10. Spec Management

### 10.1 Overview

Ragent includes a built-in specification lifecycle for tracking features,
requirements, and implementation tasks.

### 10.2 Directory Layout

Specs live in `specs/<SpecId>/`, one directory per spec (`ls -d specs/*/` lists
them). Only
`SPEC.md` is required: a directory holding just a `SPEC.md` is discoverable and
readable, and is a valid target for `/spec update`, `/spec plan` and
`/spec validate`. Every other file in the layout below is optional and derived.
The newest is `specs/agentui/` — a draft spec for the Agent and Team Member
panel (the `Agents` and `Teams` popups, the teammate strip, and their live
event plumbing):

```text
specs/
└── testspec/
    ├── SPEC.md          # EARS requirements and status (required)
    ├── PLAN.md          # Implementation tasks and Phase -1 gates
    ├── TASKS.md         # Ordered task list extracted from PLAN.md
    ├── TESTPLAN.md      # Manual test-plan with TC-NNN test cases
    ├── CONSTITUTION.md  # Optional: project constitution with nine articles
    ├── data-model.md    # Optional: data-model artifact (gated by sdd.data_model)
    ├── contracts/       # Optional: API contracts directory (gated by sdd.contracts)
    ├── quickstart.md    # Key validation scenarios derived from SPEC.md
    ├── FEEDBACK.md      # Optional: production feedback notes
    ├── JTBD.md          # Optional: Jobs-To-Be-Done analysis
    └── REPORT.md        # Optional audit/completion report
```

### 10.3 Spec Status Lifecycle

```mermaid
graph LR
    draft[draft] --> in_review[in_review]
    in_review --> draft
    in_review --> approved[approved]
    approved --> in_progress[in_progress]
    in_progress --> implemented[implemented]
    implemented --> verified[verified]
    implemented -->|reopen| in_progress
    verified --> archived[archived]
    archived --> draft
```

### 10.4 Spec Tools

| Tool | Purpose |
|------|---------|
| `spec_list` | List specifications |
| `spec_read` | Read a spec by ID |
| `spec_search` | Search specs by keyword |
| `spec_coverage` | Generate requirement coverage report |
| `spec_task_update` | Update a plan task status; mirrors the status into the session task tracker (creates the tracker task if none exists, updates it if it does) |

### 10.5 Slash Commands

| Command | Purpose |
|---------|---------|
| `/spec create <name> <title> [--from-research <name>] [--folder <path>]` | Create a new spec (SPEC.md + PLAN.md + TESTPLAN.md). `--folder` targets another project's `specs/` root (used by `/spec reverse --folder` chaining) |
| `/spec specify <name> <feature> [--from-research <name>]` | Generate SPEC.md only (EARS spec with `[NEEDS CLARIFICATION]` markers); optionally creates a git branch when `sdd.branch_per_spec` is enabled |
| `/spec plan <name> <tech-context>` | Generate or regenerate PLAN.md from existing SPEC.md using technology context as guidance |
| `/spec tasks <name>` | Generate TASKS.md (ordered task list from PLAN.md) and quickstart.md (validation scenarios from SPEC.md) |
| `/spec update <specname>` | Re-read `SPEC.md` and regenerate `PLAN.md` + `TESTPLAN.md` (preserves unchanged task IDs) |
| `/spec add <id> <requirement>` | Add an incremental requirement to an existing spec |
| `/spec feedback <spec-id> <note>` | Append a production feedback note to `FEEDBACK.md` |
| `/spec impl <id>` / `/spec implement <id>` | Transition spec to `in_progress` and generate plan |
| `/spec jtbd <specname> [--force] [--agent <name>]` | Perform JTBD analysis and write `JTBD.md` in the spec folder |
| `/spec list [--status <status>] [--prefix <prefix>] [--archived]` | List specs with optional filtering; `--status=<s>` is accepted too, and an unknown status or flag, a flag followed by another option, or an empty `--status=` is an error instead of a full listing |
| `/spec search <query>` | Search specs by keyword |
| `/spec show <id>` | Read a spec |
| `/spec validate [specname]` | Validate EARS compliance, clarification markers, and consistency (ambiguity, contradiction, gap detection) |
| `/spec status <id> [<new-status>]` | Show or transition spec status |
| `/spec task <id> [<task-id>] [<new-status>]` | List, show, or update task status |
| `/spec coverage <id>` | Generate requirement coverage report |
| `/spec activate <id>` | Activate a spec for context injection into agent prompts |
| `/spec deactivate` | Deactivate the currently active spec |
| `/spec delete <id> [--yes]` | Delete a spec directory |

### 10.6 Research Linkage

Specs can reference research outputs via the `--from-research` flag on
`/spec create` and `/spec specify`. When provided, the spec's `SPEC.md`
receives a `research:` key in its YAML frontmatter linking to the research
artifact, and a `## Related Research` section is injected into the body
summarising the linked research output.

### 10.7 SDD Configuration Flags

The following configuration flags in `ragent.json` under the `sdd` key control
opt-in Spec-Driven Development capabilities:

| Flag | Default | Purpose |
|------|---------|---------|
| `branch_per_spec` | `false` | Create a `spec/<name>` git branch during `/spec specify` |
| `data_model` | `false` | Generate `data-model.md` during `/spec plan` |
| `contracts` | `false` | Generate `contracts/` directory during `/spec plan` |
| `feedback_loop` | `false` | Surface `FEEDBACK.md` notes during `/spec plan` regeneration |

### 10.8 Consistency Validation

`/spec validate` performs three categories of consistency checks in addition
to EARS compliance:

- **Ambiguity detection** — flags vague terms (e.g. "fast", "efficient") and
  undefined references in requirement text.
- **Contradiction detection** — identifies conflicting requirements that
  specify opposing constraints.
- **Gap detection** — identifies requirements lacking acceptance criteria.

Consistency warnings are included in the validation report alongside EARS
errors and `[NEEDS CLARIFICATION]` marker warnings.

### 10.9 Clarification Markers

During `/spec specify`, the LLM is instructed to insert
`[NEEDS CLARIFICATION]` markers for ambiguous or underspecified requirements.
These markers are detected during validation and block the `approved` status
transition until resolved.

### 10.10 Production Feedback Loop

`/spec feedback <spec-id> <note>` appends advisory notes to
`specs/<spec-id>/FEEDBACK.md`. When the `sdd.feedback_loop` config flag is
enabled, these notes are automatically surfaced in the `/spec plan` prompt
so that production feedback (metrics, incidents, user reports) informs plan
regeneration.

### 10.10a Task Status Symbols and Coverage Report

Task statuses carry shared ASCII symbols from `TaskStatus::symbol()`:
`[wait]` pending, `[sync]` in-progress, `[ok]` completed, `[stop]` blocked.
The `/spec coverage <id>` command and the `spec_coverage` agent tool both
render the requirement-coverage report through a single shared
`Spec::coverage_report()` implementation (overall coverage percentage, a
per-requirement `[ok]`/`[  ]` line with linked-task completion counts, and a
per-task line with status symbol and linked requirement IDs), so the two
consumers can never drift apart.

### 10.10b Spec File Write Semantics

Spec files are written atomically (unique temp file, flush + sync, rename), and
auxiliary review files follow a clear-on-empty rule: when `REVIEW.md` or
`FEEDBACK.md` content becomes empty the file is deleted rather than written as
an empty placeholder. Frontmatter regeneration via `update_frontmatter`
preserves keys the lifecycle manager does not model (e.g. `research:`
linkage), quoting values that contain YAML-special characters.

### 10.10c Automatic Task Completion on Spec-Directory Writes

When an agent turn writes files inside the active spec's directory
(`specs/<id>/`) and the session has an active spec, tasks in the `in_progress`
state are automatically advanced to `completed` with a completion timestamp.
The heuristic is guarded by `writes_in_spec_dir`: only file-writing tools
(`Write`, `Edit`, `multiedit`, `patch`, `apply_patch`, `create`,
`append_to_file`) whose resolved target path sits inside the spec directory
count. Writes elsewhere in the workspace (docs, scratch files, snapshots)
never complete spec tasks, preventing unrelated edits from silently corrupting
`PLAN.md`.

### 10.10d Session Task Tracker Sync

`spec_task_update` calls are mirrored into the session task tracker: a
tracker task tagged with the same `spec_id`/`spec_task_id` metadata is
updated when it exists, or created (seeded from the PLAN.md task row,
status mapped from the spec task status) when it does not. The
`/spec impl` driver no longer pre-creates milestone tracker rows; the
tracker is populated lazily by the agent's `spec_task_update` calls.

### 10.10e Task Completion Timestamps

When a task is marked `completed` (via `/spec task` or `spec_task_update`),
its completion time is written into the PLAN.md Status cell as RFC 3339
UTC, e.g. `| completed (2026-11-08T10:12:00Z) |`. No extra column is added,
so 6- and 7-column task tables keep parsing; a bare `completed` (older
plans) or an unparseable stamp reads as completed with no time. Moving the
task to any other status drops the stamp. `/spec task <id> <task-id>` shows
the time in the local time zone; `spec_read` reports it in UTC.

### 10.10f Spec Persistence Guarantees

The `SPEC.md`/`PLAN.md` round-trip preserves everything the writer does not own:

- **Frontmatter.** A comment line (`# ...`) inside a `reviewers`, `research` or
  `audit` list is not a top-level key, so it no longer truncates the list and
  the writer no longer copies orphaned items (`io::is_frontmatter_key_line`).
  The block closes only on a line that is exactly `---`, so a `---` inside a
  value does not cut it short. Keys the lifecycle manager does not model are
  kept on every save, the audit trail is read back rather than regenerated, and
  multi-line `research:` lists parse.
- **Task table.** Empty cells stay empty, so a blank Requirement cell does not
  shift the columns after it. The em/en dash placeholders the row formatter
  writes read back as "none" instead of a requirement or dependency literally
  named as a dash. A status update rewrites the matching rows in place (the
  trailing newline preserved) instead of replacing everything between the table
  and the next heading.
- **Rewrite scope.** A rewrite touches exactly what `PlanParser` reads as tasks:
  the first task header in the Tasks section (a legacy `| Task ID | Title |`
  header is normalised to seven columns) and data rows with at least six cells.
  An unrelated table that merely mentions a task ID, such as a three-column
  review table, is left untouched.
- **Discovery.** One unreadable spec is reported and skipped; it no longer
  aborts discovery for every other spec.
- **Status writes.** `/spec impl` reports a failed write to `implemented` as an
  error and leaves `SPEC.md` at its previous status instead of reading the
  mutated in-memory status back as success. `/spec status` reports the
  transition it performed (it previously compared the status against itself),
  deleting the active spec deactivates it, and `/spec deactivate` invalidates
  the cached spec section of the system prompt.

### 10.11 Constitutional Amendment Process

When a spec directory contains a `CONSTITUTION.md`, amendments require
explicit rationale documentation, a backwards-compatibility assessment, and
a dated changelog entry within the file. The constitution parser validates
the nine-article structure and the amendment process.

### 10.12 Project Scaffolding (`/new`)

The `/new` slash command scaffolds a new project in the current working
directory (spec `newproj`); see section 11A for the full behaviour. The
pipeline is flag validation -> empty-directory guard -> no-silent-overwrite
file emission -> git init + initial commit -> optional GitHub/GitLab remote
create + push with failure containment -> summary report. In the TUI the
blocking emit/git/remote steps run on a worker thread while streamed
progress lines update a single in-place progress message.

---

# Part III: Data & Knowledge Systems (continued)

## 11. Research System

### 11.1 Overview

The `/research` slash command and `ragent research` CLI create structured
research items under `research/<name>/`. Each item contains captured sources
(web pages, local files, prior specs) and one or more dated report documents
(`RESEARCH-<YYYY-MM-DD>.md`; an item created by an older ragent keeps its
legacy undated `RESEARCH.md`).

### 11.2 Source Gathering

A research session runs in phases:

1. **Setup** — validate name and create the skeleton report
   `RESEARCH-<YYYY-MM-DD>.md` (named for the item's creation date).
2. **Web** — issue `websearch` queries and fetch pages via `webfetch`. Every
   HTML page captured as a web source must have been extracted by the
   `readability-rs` crate (the research web-gather phase verifies the
   `extraction_method` signal reported by `mf_fetch`, or re-runs readability
   on the raw HTML when the legacy `webfetch` tool is used). Pages where
   readability extraction failed — and would only be available via the
   html2text / raw tag-strip fallbacks — are rejected with a
   `readability extraction failed …` fetch error instead of being accepted.
   `github.com/{owner}/{repo}/blob/…` file-view URLs are rewritten to their
   `raw.githubusercontent.com` counterparts before fetching, since the blob
   page is GitHub application chrome that readability cannot extract as an
   article. PDF and YouTube sources bypass readability entirely by design,
   as do non-HTML content types (e.g. `text/plain` served by
   `raw.githubusercontent.com`) — their bodies are the verbatim document
   content, so no fallback extraction is involved. YouTube
   sources are captured as the video's caption transcript: `mf_fetch`
   parses the watch page's `ytInitialPlayerResponse` (brace-balanced,
   string-aware JSON extraction), reads the caption tracks from
   `captions.playerCaptionsTracklistRenderer.captionTracks`, and fetches
   the default (or first English) track. A fetch that fails at the tool
   level (e.g. `mf_fetch` reporting `content_ok = false` or an `error`,
   such as "no caption tracks available for this YouTube video") is
   rejected as an explicit `FetchFailed` event carrying the real reason —
   placeholder error text never enters the research corpus as a source
   body.
3. **Local** — scan project files with `Glob`/`Grep`/`read`. Each captured
   file produces an excerpt showing the matching lines plus one line of
   context on either side, and a `relevance` note that names the matched
   keyword(s) and the first matching line.
4. **Specs** — cross-reference prior specs under `specs/`.
5. **Synthesize** — (TUI only, when an active model is configured) send the
   captured source bodies to the LLM and ask for Summary, Findings,
   In-Project Cross-References, and Open Questions. The session emits a
   `SynthesizeResult` event so the UI can distinguish LLM-synthesized output
   from the mechanical fallback.
6. **Assemble** — combine frontmatter, analysis, and References Index.
7. **Finalize** — mark the item `Complete` and refresh `research/INDEX.md`.

Supporting files under `research/<name>/sources/` (e.g. `web-01.md`,
`local-02.md`) contain the **actual** captured body — web pages render into
plain text, local files become keyword-anchored excerpts — so a reader can
audit the evidence without re-running the search.

### 11.2.1 Execution Modes and Per-Page Summarisation

The `--mode` flag selects the high-level research strategy while reusing
the same gathering and synthesis pipeline. Supported modes are:

| Mode | Description |
|------|-------------|
| `tiered` | Default single adversarial pipeline. |
| `supervisor` | Supervisor/researcher graph: a lead supervisor spawns parallel sub-topic researchers (FR-009). |
| `competitive` | Supervisor graph specialised for comparing a set of entities. Defaults to `--format comparison-table` unless an explicit `--format` is supplied (FR-014). |

`--max-concurrent-research-units N` caps the number of parallel
researcher agents in `supervisor` and `competitive` modes. The default is
5 and can be changed in `ragent.json` under `research.supervisor.max_concurrent_research_units`.

`--summarization-model <provider:model>` selects a lightweight model for
per-page webpage summaries. When omitted, the active default model is
used with a short-output limit. This is independent of the per-phase
overrides `--research-model`, `--compression-model`, and `--final-report-model` (FR-013).

`--no-clarify` (default) and `--clarify` control whether the pipeline
asks a single clarifying question when the topic looks ambiguous (FR-005,
FR-017). Clarification is disabled by default; `--clarify` opts in.
`--brief <TEXT>` supplies an explicit research brief used as
the mission statement for synthesis (FR-004).

`--evaluate` appends a deterministic self-evaluation scorecard
(quality, relevance, groundedness, completeness, structure) to the
assembled `RESEARCH.md` (FR-008, FR-015). It can also be enabled by
default via `research.evaluate.enabled: true`.

### 11.3 AI-Driven Synthesis

When the TUI builds a `ResearchSession` with an active provider/model, an
`LlmAnalysisEngine` is wired in. The engine sends a single prompt containing
truncated source bodies and requests structured markdown sections. The result
is parsed and used to populate the final `RESEARCH.md`.

If the LLM call fails or returns empty output, the session falls back to a
**mechanical digest** that is still useful:

- **Summary** — names the captured web titles (top 3), local file paths
  (top 3), and cross-referenced specs, and is transparent that no LLM
  analysis was applied.
- **Findings** — one bullet per source with the matched keywords, an
  excerpt, and a relevance note.
- **In-Project Cross-References** — table of local files with relevance
  notes.
- **Open Questions** — suggests gaps (missing web sources, missing local
  files, etc.) and recommends re-running with a configured LLM.

Synthesis failures are logged at `error` level (not `warn`) and surfaced via
the `SynthesizeResult { outcome: FallbackError, detail }` event so the user
can see why the LLM path failed.
### 11.4 Output Document

Every report (`RESEARCH-<YYYY-MM-DD>.md`) contains:

- YAML frontmatter with `name`, `title`, `status`, `created`, `modified`, `sources`
  (the `title` is named by the LLM from the generated executive summary when the
  synthesis came from the model - seeded with the research topic, normalised by
  `clean_generated_title` and capped at `RESEARCH_TITLE_MAX_CHARS` on a word
  boundary - and otherwise a reduced-length version of the `## Summary`
  content capped at 80 characters, so the headline reflects the synthesis
  rather than the original prompt) plus an optional `invocation` line recording
  the verbatim front-end command (`ragent research create ...` from the CLI,
  `/research ...` from the TUI, or a `POST /research` summary from the HTTP
  API) so the
  `/research update` command can replay the run
  - `# Title:` heading
  - `## Topic`
  - `## Summary`
  - `## Findings` (numbered)
  - `## In-Project Cross-References` (table)
  - `## Open Questions`
  - `## References Index` (table)

  Section order in the report layout: Topic → Search Queries → Executive
  Summary → **Top 10 Implications → Open Questions** → Findings → Findings
  Relationship Diagram → In-Project Cross-References → References Index.
  Open Questions sits directly under Top 10 Implications so unresolved gaps
  surface immediately after the ranked consequences.

  The `newsletter` format (§11.7) replaces that layout with the reader-facing
  subset only: Topic → Executive Summary → Top 10 Implications → Open Questions
  → Concepts → Findings → Findings Relationship Diagram. The corpus quality
  scoreboard, research brief, search-query/engine/provider blocks, Data Quality
  & Consistency, every QA render section, In-Project Cross-References and the
  References Index are not emitted, so the artifact reads as a newsletter
  rather than an audit trail. The `CORPA` companion is unaffected - it is
  written for every format.

  The quality-assurance render sections (Contradiction Graph, Loci Analysis,
  Depth Investigation, Cross-Locus Reconcile, Source Tensions, Synthesis
  Audit, Corpus Critic) are written to a per-research companion file
  `research/<name>/CORPA-<YYYY-MM-DD>.md` instead of the report. The companion
  is created alongside the skeleton and rewritten on every `write_document`;
  it ends with a `## Sources Reference` copy of the References Index table so
  `[#N]` source indices resolve in both files.

  #### Dated output names

  Both files carry the run's ISO date, so a `/research update` re-run (which
  replays the recorded invocation and rewrites the report) keeps the earlier
  run's document instead of overwriting it:

  - The **primary** report is `RESEARCH-<created_at date>.md`, where
    `created_at` is taken from the item's frontmatter. It is what
    `ResearchManager::create` publishes, so an item answers the same path
    before and after its first run.
  - A later run writes `RESEARCH-<run date>.md`, so the same item accumulates
    one report per run date and a same-day re-run produces a new file.
  - **Resolution** (`ResearchIo::resolve_research_md` /
    `resolve_corpa_md`) checks the canonical creation-dated name with one
    `try_exists`, then falls back to a directory scan
    (`find_research_md` / `find_corpa_md`) that accepts the lexicographically
    greatest dated name - the ISO prefix sorts chronologically, so that is the
    most recent run - and otherwise the legacy undated
    `RESEARCH.md` / `CORPA.md` (`LEGACY_RESEARCH_FILE_NAME` /
    `LEGACY_CORPA_FILE_NAME`). No command joins a hardcoded file name; every
    read surface (`ResearchManager::{list, show_validated, set_status, search,
    complete_gathering, write_document, continue_item}`, `ragent research open`
    and the TUI `/research open`) resolves the report first.

  When an LLM is configured, `/research create` also runs a concept-extraction
  step (spec researchcluster) after synthesis: the gathered corpus is assembled
  into a context-bounded payload whose blocks are headed `--- [#N] Title ---`
  with `N` equal to the References Index position, and the model returns a
  numbered concept list (`### N. Name` subsections with `[#N]` citations).
  The normalized list is rendered as `## Concepts` directly above `## Findings`
  (report layout) or `### Concepts` directly above `### Findings` inside
  Results (IMRaD layout). When no LLM engine is wired, or the extraction call
  fails or returns no sections, the section is omitted entirely and the run
  completes normally; the step is surfaced in the UI as a `concepts` `RunStep`
  event (started / completed / skipped / failed).

### 11.4.1 Diff Deck (`/research deck`)

`/research deck <name>` (CLI: `ragent research deck <name>`, spec `diffdeck`)
consumes an item's *series* of dated reports as a change narrative: it selects
two of them, diffs them element by element, and writes a single Markdown slide
deck. The deck is **deterministic and LLM-free** - a pure function of the two
input reports - so the same pair always yields byte-identical output, the run
costs no tokens, and it works with no provider configured.

**File naming.** The deck is written into the item directory as
`DECK-<YYYY-MM-DD>.md`, dated by the UTC day the deck was **generated** (never
by either contributing report), using the same atomic write-then-rename path as
every other research artifact. Regenerating on a later day therefore adds a file
beside the earlier deck instead of overwriting it. `--out <path>` redirects the
output (creating missing parent directories); `--force` overwrites an existing
target, which is otherwise refused by name. A `DECK-*.md` file is not a report:
the report resolver and `ReportSeries` ignore it.

**Report pair.** By default a deck covers the two most recent dated reports,
rendered older-as-baseline / newer-as-current. `--from <YYYY-MM-DD>` and
`--to <YYYY-MM-DD>` name the pair explicitly; each must match a
`RESEARCH-<stamp>.md`, and supplying only one pairs it with its *adjacent*
dated report (the next older for `--to`, the next newer for `--from`). An
explicit pair must be ordered older-then-newer. A legacy undated `RESEARCH.md`
is the oldest report of the series - baseline only - so an item whose only
report is `RESEARCH.md` is refused specifically as needing its first *dated*
report.

**Coverage window.** The cover slide carries the research title, the generation
date, the research id, and the two contributing report dates as the coverage
window `**Coverage:** <older-date> to <newer-date>` (a legacy side reads
`legacy`), plus counts of element slides, change items and unchanged items. The
window is what makes a stored deck self-describing months later.

**Composition.** Markdown slides separated by `---` rules, in fixed order: one
cover slide, then one slide per changed element in the fixed element precedence
(Executive Summary, Concepts, Findings, Cross-References, References), then one
closing summary slide reporting the element-slide count, the per-status counts
and the element names that produced a slide. The **Concepts** element is the
deck's key grouping and emits one slide per concept: the concept's name, its
status, its definition, then the findings that belong to it - each with its own
status tag, headline and observation/analysis paragraph. Every finding no
concept claimed (the grouping is at most three per concept) is rendered on an
`Other Findings` slide, so each finding appears on the deck exactly once. The
report's `## Top 10 Implications` and `## Open Questions` are not deck elements
and produce no slide; `--elements` neither accepts nor lists them. The other
element slides each carry **every** item of the element from both reports - the
changed items and the unchanged ones - so a slide is a complete picture of that
element, not only its deltas. A pair that changed nothing still writes a deck,
with an explicit `_No changes detected between the two reports._` notice in
place of the element slides. `--include-old` additionally emits a slide for an
element carrying only unchanged items.

**Status tags.** Every rendered item carries exactly one tag from the closed set:

| Tag | Definition |
| --- | --- |
| `NEW` | The item exists only in the newer report. |
| `ADDED` | The item exists only in the newer report **and** cites at least one source the older report did not cite. |
| `UPDATE` | The item exists in both reports with changed condensed text. |
| `OLD` | The item exists in both reports with unchanged condensed text. |

Items are matched by section-scoped normalised identity: Findings by their
extracted headline (so a reworded headline is `NEW`), Concepts by their declared
name (the `### <n>. <name>` heading minus its ordinal), and every other element
by its normalised text with citations, Markdown emphasis, backticks and
whitespace runs removed (so cosmetic drift is `UPDATE`, not a `NEW` plus a
silent deletion).
Each item is condensed to one bullet, capped at 200 characters on a word
boundary with a trailing ellipsis, and has Markdown-significant characters
escaped so text containing `|`, backticks or a leading `#` cannot break the
slide structure.

**Safety.** No network request, LLM call or write outside the research root is
performed. Every refusal (unknown item, fewer than two reports, bad or unknown
`--from`/`--to` date listing the dates that do exist, inverted pair, unknown
`--elements` value listing the accepted ones, unreadable / non-UTF-8 /
section-less report, existing target without `--force`) is decided before the
destination is written, and leaves the item directory byte-identical - an
existing deck file included. The diff and render stages are pure functions over
in-memory strings; the implementation contains no `unsafe` code and no
`unwrap()`/`expect()` on a production path.

### 11.5 Slash Commands

| Command | Purpose |
|---------|---------|
| `/research create ... --mode tiered|supervisor|competitive` | Select research execution strategy |
| `/research create ... --summarization-model <provider:model>` | Use a lightweight model for per-page webpage summaries |
| `/research create ... --research-model <provider:model>` | Model used by sub-topic research agents |
| `/research create ... --compression-model <provider:model>` | Model used to compress intermediate findings |
| `/research create ... --final-report-model <provider:model>` | Model used to write the final report |
| `/research create ... --max-concurrent-research-units N` | Limit parallel researchers in supervisor/competitive modes |
| `/research create ... --papers` | Include academically-classified backends (OpenAlex) in the search sweep. Scholarly engines are excluded by default; config `research.exclude_academic_engines` persists the exclusion |
| `/research create ... --oa-enable` / `--no-oa` | Force open-access recovery on/off for the run, overriding `research.open_access_recovery` |
| `/research create ... --max-queries N` | Cap the number of search queries produced by the topic-decomposition step (default 20, or `research.max_queries`; `0` = unbounded, subject to the decomposer's hard upper bound of 20). Bounds the LLM request, the heuristic fallback, and the queries the gatherer searches |
| `/research create ... --max-concepts N` | Cap the `## Concepts` list (default 5; `0` = unbounded); entries are ordered most-relevant-first before truncation |
| `/research create ... --max-findings N` | Cap the `## Findings` list (default 20; `0` = unbounded); entries are ordered most-relevant-first before truncation |
| `/research create ... --url-cloak` | Emit web source URLs in `Sources` bullets and the `References Index` (and `CORPA.md` `Sources Reference`) as defanged plain text (`hxxps://host[.]tld/…` in a code span) instead of clickable links |
| `/research create ... --age <SPAN>` | Reject web sources published longer ago than `SPAN` (`30d`, `6m` = 30 days/month, `2y` = 365 days/year). A source with no publication date is rejected too, because the caller asked for recent material. The window's start date (`now - span`, ISO `YYYY-MM-DD`) is appended to every decomposed search query as `after:YYYY-MM-DD` (`SEARCH_QUERY_AGE_OPERATOR`) so the engines steer towards material inside the window, and the same cut-off instant serves the post-fetch rejection test (`ExclusionReason::TooOld`, the `age` column of the progress table). Off by default; recorded in frontmatter and replayed by `/research update`. |
| `/research create ... --evaluate` | Append a deterministic quality scorecard to `RESEARCH.md` |
| `/research create ... --brief <TEXT>` | Provide an explicit research brief |
| `/research create ... --clarify` | Ask the single clarifying question (disabled by default) |
| `/research create <name> <topic>` | Gather sources and write `RESEARCH.md` |
| `/research create <name> <topic> --iterations N --depth shallow|standard|deep --format ...` | Iterative research with controls |
| `/research create <name> --from-url <URL>` | Fetch the URL, use it as the research subject and capture it as the primary source |
| `/research create <name> --from-file <PATH>` | Extract a local document and use it as the research subject |
| `/research create ... --use-low-relevance` | Retain low-relevance web sources |
| `/research continue <name> [message]` | Add the follow-up to the item's plan and re-run the session on it |
| `/research update <name>` | Replay the recorded invocation for a run and write a new dated report (`RESEARCH-<run date>.md`); the earlier run's report is kept |
| `/research list` | List research items |
| `/research open <name>` | Show the path to the item's report (the newest dated `RESEARCH-<date>.md`, or the legacy `RESEARCH.md`) |
| `/research show <name> [--json]` | Show metadata and sources (each labelled with its own kind) |
| `/research search <query> [--json]` | Search across every item's resolved report file |
| `/research delete <name> --yes` | Delete a research item (the TUI requires `--yes`; the CLI prompts) |
| `/research archive <name>` | Archive a research item |
| `/research config` | Print the effective `research` settings as JSON |
| `/research deck <name>` | Write `DECK-<YYYY-MM-DD>.md` for the item: a deterministic, LLM-free slide deck diffing two of its dated reports (§11.4.1) |
| `/research deck <name> --from <date> --to <date>` | Diff the named report pair instead of the two most recent; one alone pairs with its adjacent dated report |
| `/research deck <name> --elements <list>` | Restrict the deck to named elements (case-insensitive; slide label or report heading); unknown names are refused |
| `/research deck <name> --include-old` | Also emit slides for elements that changed nothing |
| `/research deck <name> --out <path>` / `--force` | Write elsewhere (parent dirs created) / overwrite an existing target |

### 11.6 Iterative Research Loop (`researchext`)

The research engine supports an iterative, stateful loop:

1. **Plan** — decompose the topic into focused sub-questions.
2. **Gather** — capture sources for pending sub-questions in parallel.
3. **Synthesize** — produce structured findings from evidence.
4. **Verify** — ensure each claim traces back to a supporting source.
5. **Critique** — score the result and detect missing-link gaps.
6. **Iterate or stop** — continue until complete, out of budget, or no longer improving.

State is persisted to `research/<name>/state.json` so sessions can be resumed with
`/research continue`. The loop emits structured events for plan updates, sub-question
status changes, source captures/failures, critic scores, verification results,
follow-up queries, and iteration completion.

### 11.7 Output Formats

The `--format` flag selects the artifact produced at the end of a session:

| Format | Description |
|---|---|
| `report` | Full multi-section `RESEARCH.md` (default). |
| `executive-summary` | One-page summary. |
| `comparison-table` | Comparison table across key entities. |
| `source-bibliography` | Standalone bibliography of all captured sources. |
| `imrad` | Introduction, Methods, Results, Discussion structure for academic-style reports. |
| `newsletter` | Short, reader-facing newsletter: frontmatter, title, topic, executive summary, top 10 implications, open questions, concepts, findings, and the findings relationship diagram. Every audit-trail section is omitted. |

`newsletter` accepts only the canonical spelling (no aliases) and persists as
`output_format: newsletter` in the item frontmatter. Its layout is the five
narrative sections plus the diagram - it never renders the corpus quality
scoreboard, the research brief, the search queries / engine summary / provider
request blocks, Data Quality & Consistency, the QA artifact sections,
In-Project Cross-References or the References Index. Sections whose data was
not produced still render their placeholder, so the document's shape is stable
across a partially-populated corpus.

### 11.8 Research Tiers

The `--tier` flag selects the depth of analysis:

| Tier | Description |
|---|---|
| `light` | Minimal analysis — web gathering + mechanical digest only. |
| `full` | Full analysis pipeline — contradiction graph, loci analysis, reconcile, corpus critic, synthesis, QA (default). |
| `dissertation` | Extended pipeline — includes depth investigations, source tensions, evidence digest, and width sweeps. |

### 11.9 HTTP Research API

The research system is exposed via REST endpoints (all auth-protected):

- `GET /research` — list all research items as JSON (`{items: [...], count: N}`).
- `POST /research` — create + run a research session. Returns `202 Accepted`
  immediately with a `Location` header pointing to `GET /research/{name}/events`.
  The research run executes in a background `tokio::spawn` task.
- `GET /research/{name}` — show a single research item. Pass `?full=true` to
  include extended metadata fields (`topic`, `queries`, `output_format`,
  `model`, `mode`, `summarization_model`, `evaluate`, `brief`).
- `DELETE /research/{name}?confirm=delete-{name}` — delete a research item
  (requires confirmation token).
- `GET /research/{name}/events` — SSE stream of live research events. When no
  active run exists but the item is on disk, returns a JSON status blob. Each
  SSE event is a JSON object `{"kind": "...", "payload": {...}}` with event
  type `research`.

The `POST /research` request body mirrors `ResearchRunRequest`:
`name`, `topic`, `title`, `from_urls`, `from_files`, `sources_dir`,
`template`, `depth`, `tier`, `iterations`, `format`, `mode`,
`summarization_model`, `research_model`, `compression_model`,
`final_report_model`, `max_concurrent_research_units`, `use_local`,
`use_specs`, `use_low_relevance`, `papers`, `exclude_academic_engines`,
`open_access_recovery`, `use_pdf`, `evaluate`,
`brief`, `clarify`, `fetch_concurrency`, `local_concurrency`,
`fetch_timeout_secs`, `web_phase_timeout_secs`,
`local_phase_timeout_secs`, `search_max_retries`, `search_retry_base_delay_ms`,
`max_web_results`, `max_search_calls`,
`max_concepts`, `max_findings`,
`max_local_sources`,
`max_synthesis_sources`. `web_phase_timeout_secs` (CLI `--web-time`) defaults
to 180 seconds (`DEFAULT_WEB_PHASE_TIMEOUT_SECS`); a value of `0` disables the
deadline and allows the web phase to run to natural completion (FR-007). When
the web-gathering phase exceeds the deadline the run ingests everything gathered
so far and proceeds to analysis/synthesis with the partial source set, emitting
a single `web_deadline` `RunStep` diagnostic that carries the effective deadline
and the number of sources captured (FR-004). No new search is started
after the deadline: the decomposer call and each sub-query search-result wait
are deadline-bounded (FR-008). The fetch stage is never gated on or cancelled by
the deadline: every candidate produced by the search stage is fetched to
completion and in-flight fetches always run to completion (or their own
`fetch_timeout_secs` cap). At the
start of a web-gathering phase the system emits a `web_phase_start` `RunStep`
carrying the remaining effective deadline in seconds; the TUI renders this as a
live `web:M:SS` countdown in the status-bar wait segment, computed from a stored
wall-clock `Instant` at render time and updated at least once per second while
the phase is active (FR-009, FR-010, FR-013). The same deadline is applied to
every iteration of the iterative research engine (FR-006).

Web-search quota controls: `max_search_calls` (CLI `--max-search-calls N`)
places a hard cap on the total web-search calls a single run may issue. The
cap is shared via `Arc` across every supervisor/competitive researcher and
every gather pass (main gather, engine iterations, gap-fill), so N parallel
researchers draw from one per-run pool; when the cap is reached remaining
sub-queries are skipped, the run proceeds with the sources gathered so far,
and a `search_budget` `RunStep` diagnostic reports the used/limit counts.
Within a supervisor/competitive run, a query-result cache keyed on the
normalized query text serves identical sub-queries (common when per-entity
researchers decompose the same comparison dimensions) from the cache instead
of re-issuing paid search calls. In competitive mode the researcher count is
capped at `max_concurrent_research_units` (one sub-topic per extracted
entity, truncated to the cap). Without an explicit `--max-web-results`, the
web-source budget is derived from `--depth` (shallow 6 / standard 9 / deep 15
sources per the depth preset) instead of defaulting to the 500-source
ceiling; `--depth` therefore bounds web volume by default.

---

## 11A. Project Scaffolding (`/new`)

The `/new` slash command (and the `ragent new` CLI subcommand / `ragent run
"/new ..."` surface) scaffolds a new project in the current working
directory (spec `newproj`). The pipeline is: flag validation (`--language`
rust/python/go/typescript, `--type` library/cmdline/tui/gui/webapp, optional
`--stack`, mutually exclusive `--github`/`--gitlab`) -> empty-directory
guard (refuses non-empty targets; ragent artifacts `.ragent/`, `log/`,
`target/` are allowlisted) -> file emission with no silent overwrite
(existing files are reported and left untouched) -> local git init + initial
commit -> optional GitHub/GitLab remote create + push with failure
containment (a failed remote step never undoes the local scaffold; an
existing remote repository is reused instead of failing) -> summary report
(created layout, generated files, language/type/stack, git outcome, remote
status).

Generated content: the ragent workspace (`.ragent/config.json`,
`.ragent/agents/README.md`, `specs/README.md`, `log/`, language-specific
`.gitignore`, `AGENTS.md` stub), a runnable hello-world artifact set from
per-language recipes (`Cargo.toml`/`src/main.rs`, `pyproject.toml`/`main.py`,
`go.mod`/`main.go`, `package.json`/`src/main.ts`; `library` types swap the
binary entry for `src/lib.rs` / `src/<name>/__init__.py` / `<name>.go` /
`src/index.ts`), optional stack overlays (Rust: `axum`, `warp`, `raylib`;
unknown stacks warn and fall back to the base layout), and the documentation
scaffold (`README.md`, `QUICKSTART.md`, `STATS.md`, `docs/`) rendered from
the same recipe data as the code artifacts. In the TUI the command runs as a
foreground operation with streamed progress lines in the message window (a
worker thread performs the blocking emit/git/remote steps; no sub-agent and
no LLM turn is involved). `/new help` (and bare `/new`) prints a usage page
whose language/type value lists are derived from the scaffolder's own
registries. See `docs/howtos/newproj.md` for the user manual.

The TUI and CLI surfaces share one pipeline engine:
`project_scaffold::plan_and_emit()` in
`crates/ragent-tools-extended/src/project_scaffold/mod.rs` performs flag
validation, the empty-directory guard, file emission, stack overlay, and
git init, returning the scaffold summary plus the stack note; each surface
then attaches only its own git/hosting outcome lines to the streamed
progress. The GitHub and GitLab flows share a
`register_origin_and_push(root, url)` helper in `project_scaffold/remote.rs`.

---

# Part IV: Agent Customization & Extension

---

## 11. Custom Agents

### 11.1 Storage Locations

Custom agents can be defined in JSON using the Open Agentic Schema Framework
(OASF):

- `~/.ragent/agents/` — user-global (legacy location)
- `~/.config/ragent/agents/` — user-global (XDG config location, higher priority)
- `.ragent/agents/` — project-local (highest priority)

### 11.2 Agent Schema

```json
{
  "name": "rust-reviewer",
  "description": "Reviews Rust code for idioms and safety",
  "version": "1.0.0",
  "schema_version": "0.7.0",
  "modules": [{
    "type": "ragent/agent/v1",
    "payload": {
      "system_prompt": "You are a meticulous Rust reviewer...",
      "model": "anthropic:claude-sonnet-4-20250514",
      "permissions": [
        { "permission": "edit", "pattern": "crates/**", "action": "allow" }
      ]
    }
  }]
}
```

`version` and `schema_version` are required, and the record must carry exactly
one `ragent/agent/v1` module whose `payload` holds the agent definition
(`system_prompt`, `mode`, `max_steps`, `model`, `skills`, `permissions`, ...).

### 11.3 Template Variables

System prompts and file contexts support variables:

| Variable | Expansion |
|----------|-----------|
| `{{WORKING_DIR}}` | Current working directory |
| `{{FILE_TREE}}` | Project file tree |
| `{{AGENTS_MD}}` | Discovered AGENTS.md content |
| `{{DATE}}` | Current UTC date |

### 11.4 Loading and Diagnostics

Use `/agents` to list loaded agents, view diagnostics, and confirm custom
profiles are parsed correctly. Custom agents display a `[custom]` badge in the
agent picker.

---

## 12. Skills System

### 12.1 Overview

Skills are loadable YAML packs that inject tools, system prompts, and file
context into an agent session. They are useful for domain-specific workflows
(e.g., Rust embedded development, Terraform infrastructure).

At startup the agent builds a `SkillCatalog` — a lightweight, metadata-only
list of skill names, descriptions, and trigger phrases. The catalog is cheap to
produce and lets the agent advertise available skills without loading full
skill bodies into the system prompt. Full `SkillInfo` bodies are loaded on
demand when the skill is invoked (`SkillRegistry::catalog()` for discovery,
`SkillRegistry::get()` for activation).

### 12.2 Skill Locations

- Bundled skills in `assets/skills/`
- Custom skills in `~/.ragent/skills/` or `.ragent/skills/`
- OpenSkills in `~/.agent/skills/`, `~/.agents/skills/`, and `~/.claude/skills/`
  (global), plus `.agent/skills/`, `.agents/skills/`, and `.claude/skills/`
  (project), each scanned for `<skill>/SKILL.md` under the
  `openskills-global` / `openskills-project` scopes

### 12.3 Skill Format

```yaml
---
name: rust-embedded
description: You are an expert in embedded Rust (no_std, RTIC, embassy).
allowed-tools:
  - read
  - Edit
  - Bash
  - codeindex_search
---

The skill body follows the closing `---` and is what the model receives.
```

### 12.4 Activation

Skills can be loaded via slash command: `/skills` (or `/skill` / `/skill list`)
lists the registered packs, `/skill <name> [args]` loads and invokes a named
pack, and a bare `/<name>` trigger invokes a user-invocable pack directly.
AgentSkills packs discovered under `.agents/skills/` (project) and
`~/.agents/skills/` (user) appear in the list and are loadable by their
frontmatter name; on a name clash the existing higher-scope pack is preferred.

### 12.5 Tool restrictions (`allowed-tools`)

A `SKILL.md` frontmatter `allowed-tools` key (a single string or a list) limits
the tools a skill run may call. Entries resolve by **case-insensitive** name
comparison against the `ToolRegistry`:
`tool::build_allowed_tool_set` stores each entry under the *registry's* own
spelling (`allowed_tools::resolve_allowed_tool_entry`), so two spellings of one
tool cannot become two keys in the allowed set, and the per-step filter compares
with `allowed_tools::entry_matches_tool` (`eq_ignore_ascii_case`).
Because names are ASCII identifiers, ASCII folding is sufficient and cannot fold
two distinct names together.

The pseudo entry **`any`** (any case, `allowed_tools::ANY_TOOL_ENTRY`) names every
tool: the allowed set is returned empty, which every consumer already reads as
"no allowlist, hide nothing", so the restriction is exactly as if the key were
omitted. It is honoured without consulting the registry, so it never appears in
the unresolved report and does not depend on a registered tool of that name.

There is deliberately **no translation layer**. Because the tool rename adopted
the Anthropic spellings (`AskUserQuestion`, `Bash`, `Edit`, `Glob`, `Grep`,
`EnterPlanMode`, `ExitPlanMode`, `Skill`, `TaskCreate`, `TaskGet`, `TaskList`,
`TaskUpdate`, `Write`), a manifest written for Claude Code resolves on the
registry lookup itself. Nothing consults the `(before, after)` rename table, so a
retired spelling that names a *different* tool is unresolved rather than mapped,
and `Read` - rated too close to `read` and not adopted - is unresolved rather than
silently served by `read`. Case folding resolves *one* tool; it never resolves a
different one.

Every surface that renders a tool list routes through `tool::is_allowed_tool`, so
the case-insensitive rule holds in the sub-agent filters
(`session/processor.rs`, `session/loop_steps.rs`) and the loop `tool_set` path as
well as the primary prompt.

An entry that matches no registered tool is reported once per `(entry, declarer)`
by `tool::allowed_tools::report_unresolved_allowed_tools_once`, naming the entry
and the declaring skill, from the three places a skill's list becomes a
restriction: `Skill` (`action=read|load`), the TUI `/skill` (and bare `/<name>`)
trigger, and the forked-skill resolver. The per-turn prompt path deliberately does
not report - it visits every discovered manifest, including plugin skills the
session never runs, and a per-turn scan adds cost for an entry that constrains
nothing that turn (NFR-006).

---

## 14. Teams

### 14.1 Core Concepts

A **team** is a named workspace with:

- A roster of teammates (each with an agent type and task)
- A shared task list stored in `tasks.json`
- A per-member mailbox stored in `mailbox.json`
- A team configuration in `team.json`

### 14.2 Team Lifecycle

```mermaid
graph LR
    Create[team_create] --> Spawn[team_spawn]
    Spawn --> Work[Tasks + Messages]
    Work --> Idle[team_idle]
    Work --> Shutdown[team_shutdown_teammate]
    Idle --> Cleanup[team_cleanup]
    Shutdown --> Cleanup
```

Mailbox writes are validated at the boundary: a message authored by the lead
(the `team_shutdown_teammate` shutdown request, lead-to-teammate chat) must
carry the lead session id (`MailboxMessage::with_sender_session`,
SEC-ragent-team-003), so a lead-authored message pushed without one is rejected
at the write boundary rather than silently forwarded.

### 14.3 Team Tools

| Tool | Purpose |
|------|---------|
| `team_create` | Create team |
| `team_spawn` | Spawn teammate |
| `team_status` | Team status (status history, undelivered messages, pending plans, hook failures) |
| `team_message` | Direct message |
| `team_broadcast` | Broadcast to active teammates |
| `team_read_messages` | Read mailbox |
| `team_memory_read` / `team_memory_write` | Team memory access |
| `team_submit_plan` | Submit plan for approval |
| `team_approve_plan` | Lead approve/reject plan |
| `team_task_create` | Add task |
| `team_task_claim` | Claim task |
| `team_task_complete` | Complete task (owner or lead only) |
| `team_task_cancel` | Cancel a pending/in-progress task (lead only) |
| `team_assign_task` | Lead assigns task |
| `team_idle` | Signal idle |
| `team_shutdown_teammate` | Shutdown teammate (graceful/immediate) |
| `team_shutdown_ack` | Acknowledge shutdown |
| `team_wait` | Block until teammates settle |
| `team_cleanup` | Remove team state |

**Caller checks.** Every team tool except `team_create` first resolves who is
calling within the team named by `team_name` (`tool::team_caller`). It uses
the session's team context when that names the team. Otherwise it falls back
to the team's `config.json`: the caller is the lead when the session is
`lead_session_id`, and a teammate when the session is a member's `session_id`.
A session that is neither is refused. `team_spawn`, `team_task_create`,
`team_assign_task`, `team_approve_plan`, `team_shutdown_teammate`,
`team_cleanup` and `team_wait` are lead-only. `team_wait` without `team_name`
waits on the caller's own team.

**Teammate turns.** A teammate's agent loop stays alive between turns. After
each turn it runs another one when the member holds an auto-claimed task, or
when it has unread mail it has not yet been prompted about. Otherwise it waits.
New mail, an auto-claim, a resume and a graceful shutdown request all wake it.
Suspended, blocked and plan-pending members are not woken
(`TeamManager::next_teammate_turn`).

**Roster writes.** Every read-modify-write of a team's `config.json` goes
through `TeamStore::update`, which holds the exclusive lock from read to write.

**Team runtime.** Before tool dispatch, the session processor binds a
`TeamManager` for the team its session leads, in every entry point (TUI,
`run`, `serve`, ACP). See `SessionProcessor::ensure_team_manager`. It reconciles
members queued in `Spawning` and starts the watchdog. A `team_spawn` that runs
before the runtime is bound is refused with an instruction to retry. The
processor holds the runtime in a rebindable `TeamManagerSlot`. A manager whose
team is gone is unbound, atomically. The processor keeps a live manager of
another team rather than replacing it, so concurrent sessions do not fight over
the slot. The TUI rebinds when its active team changes. A manager refuses to spawn into a team it does not drive.
`team_shutdown_teammate` and `team_task_cancel` fall back to acting on disk when
the bound manager drives another team.

### 14.4 Team Slash Commands

| Command | Purpose |
|---------|---------|
| `/team create <name>` | Create a new team |
| `/team open <name>` | Open an existing team |
| `/team close` | Close current team |
| `/team clear` | Reset current team state |
| `/team delete <name>` | Delete a team |
| `/team cleanup` | Tear down current team |
| `/team message <to> <text>` | Send direct message |

### 14.5 Task Dependencies

Tasks can declare `depends_on` task IDs. A task cannot be claimed until all its
dependencies are completed.

### 14.6 Plan Approval Workflow

1. Teammate calls `team_submit_plan`.
2. Lead reviews and calls `team_approve_plan` (approved=true/false).
3. On approval, teammate proceeds with implementation.

### 14.7 Blueprints

#### Blueprint Storage Locations

- `~/.ragent/blueprints/teams/<name>/`
- `~/.config/ragent/blueprints/teams/<name>/` (higher priority than the legacy location)
- `.ragent/blueprints/teams/<name>/` (highest priority)

#### Blueprint Directory Structure

```
<blueprint>/
├── spawn-prompts.json   # Agent type definitions
├── task-seed.json       # Initial task list
└── context.md           # Shared context
```

#### spawn-prompts.json Format

```json
{
  "agents": [
    {
      "name": "security-reviewer",
      "agent_type": "general",
      "prompt": "Review the target files for security issues..."
    }
  ]
}
```

#### task-seed.json Format

```json
{
  "tasks": [
    { "title": "Scan for injection risks", "description": "..." }
  ]
}
```

#### Blueprint Slash Commands

| Command | Purpose |
|---------|---------|
| `/team create <name> --blueprint <bp>` | Create team from blueprint |

#### Work Context Propagation

When a team is created with a blueprint, the work context is prepended to every
teammate's spawn prompt. This context includes target directories, expected
outputs, and any constraints.

#### Blueprint Best Practices

- Keep prompts under ~500 words.
- Reference files by path rather than pasting content.
- Spawn one teammate per independent work item.
- Use `/team_wait` after spawning to avoid racing ahead.

### 14.8 Team state machines and their on-disk record

Four rules the team runtime now enforces rather than merely documents (spec
`teamssetup`). Each is tightening-only: no capability is removed, and every
refusal is visible in the event stream and the tool result rather than silent.

#### 14.8.1 One member-status writer

`TeamManager::set_member_status` is the single writer of a member's status. It
validates the move against the member status transition table, applies the
destination status's side effects, persists the roster once, appends one record
to the team's `status-history.jsonl`, and publishes exactly one
`Event::TeammateStatusChanged { from, to, cause }`. Nine direct
`member.status = ...` sites were consolidated onto it, so a transition cannot be
persisted without being logged and cannot be logged without being published.

`MemberStatus` exposes the predicates every guard now uses instead of
re-spelling a status list: `is_terminal` (`Stopped`, `Failed` - no session
behind it, only a respawn revives it; `ShuttingDown` is deliberately **not**
terminal because its session still exists and is awaiting a shutdown ack),
`is_parked` (`Idle`, `PlanPending`, `Blocked`, `Suspended` - alive but not
attached to work, so time spent there is not evidence of a wedge and the
watchdog must skip it) and `is_live` (`Spawning`, `Working`, `ShuttingDown`).

`status-history.jsonl` carries one JSON object per line
(`{timestamp, agent_id, name, from, to, cause}`), so `team_status` can answer
"why did this teammate stop?" *after* the events that answer it have gone past -
a tool call is not a listener. The file is append-only and bounded: before an
append that would pass `MAX_HISTORY_BYTES` it is renamed whole to
`status-history.jsonl.1` and the new record starts a fresh live file.
`team_status` renders the last `STATUS_HISTORY_LIMIT` (5) transitions per member,
oldest first, under a `Status history` block beneath each roster row.

#### 14.8.2 One task-status transition table

`TaskStatus::can_transition_to` and `transition_task` are the single writer of
`Task::status`, so the four claim paths and both completion paths cannot
disagree about which moves are possible, and an illegal move is refused with a
message naming the task, its actual status, and the table row that forbids it.

`TaskList::validate()` runs before every persisted write (`TaskStore::write_locked`,
the one function every list passes through, cached or not) and refuses duplicate
task ids, a `depends_on` edge naming no task in the list, and a dependency cycle
(detected by a three-colour iterative DFS), naming the offending task, the
missing id plus the ids that exist, or the closing chain
(`task-001 -> task-002 -> task-001`). The refusal is prefixed
`refusing to write invalid <path>: ...` and leaves `tasks.json`
byte-identical, so a task that could never be claimed is never persisted.

#### 14.8.3 Completion requires ownership, adoption requires consent

`team_task_complete` accepts a completion only from the task's `assigned_to`
agent or the lead; a task assigned elsewhere is refused and **not**
re-assigned, with `rejection_reason` set to `not_owner`, `not_claimed` (still
`pending`) or `already_completed`, and the current `owner` reported alongside.
Every refusal leaves the task list untouched. A `TeamTaskCompletionRejected`
event carries the same facts, so a remote consumer sees the refusal and the rule
that fired.

A second session that opens a team directory must take the team over explicitly
(`AdoptTeam::Adopt`, i.e. `adopt: true`); without it the session gets an error
naming the owning session rather than quietly competing for the same task list.
A successful adoption publishes `TeamAdopted` with the previous lead session id
and the task ids the adoption reset.

#### 14.8.4 Hook failures are recorded, never mistaken for approval

A configured quality-gate hook that cannot run - its command fails
`validate_hook_command`, fails to spawn, or exceeds the 30-second
`HOOK_TIMEOUT` - appends one JSON object per line to the team's
`hook-failures.jsonl` (`{timestamp, event, hook, reason, detail}`, with `reason`
one of `invalid`, `spawn_failed`, `timeout`). The file is append-only and
bounded: at 1 MiB it is rotated whole to `hook-failures.jsonl.1` (replacing the
previous generation). Recording is best-effort, so the ledger can never change a
hook's verdict - but the count is surfaced in `/team show <name>` and
`team_status`, so an inert hook is never mistaken for one that ran and approved.

#### 14.8.5 Parked members and blocked exits

The watchdog's eligibility test is `MemberStatus::is_live()`, so a parked member
is never failed for taking no progress, and a team whose config cannot be read
yields "not live" rather than being failed on a guess. `resume` accepts a
`Blocked` member as well as a `Suspended` one, and when a task completes every
`Blocked` member whose last unmet dependency is now satisfied is moved to `idle`
automatically and published as `TeammateResumed`, so the common case needs no
operator verb. `team_wait` reports parked members distinctly instead of waiting
out its clock on them, and `team_cleanup`'s guard blocks only on live or
shutting-down members (parked and terminal members never block; `force: true` is
the only override, and `/team cleanup` and `/team delete` share the guard).

---

## 15. Swarm Mode

### 15.1 How Swarm Works

The `/swarm <prompt>` slash command decomposes a user request into parallel
subtasks and spawns multiple agents to work on them concurrently. A synthesis
agent combines the outputs into a final answer.

### 15.2 Swarm Slash Commands

| Command | Purpose |
|---------|---------|
| `/swarm <prompt>` | Decompose and run parallel swarm |
| `/swarm status` | Show active swarm tasks |
| `/swarm kill` | Cancel active swarm |

### 15.3 Decomposition Format

Swarm decomposition returns a JSON or Markdown list of subtasks:

```json
{
  "tasks": [
    { "id": "1", "agent": "explore", "prompt": "Find all auth-related files" },
    { "id": "2", "agent": "code-review", "prompt": "Review auth logic for issues" }
  ]
}
```

### 15.4 Swarm State

Swarm state is tracked in memory and via the team store when teammates are
used. The TUI active-agents panel shows per-agent status and progress.

### 15.5 Limitations

- Swarm requires a provider with enough throughput for parallel calls.
- Cloud providers may hit rate limits; exponential retry backoff mitigates this.
- Synthesis depends on the quality of subtask outputs.

---

## 16. Autopilot Mode

### 16.1 How Autopilot Works

Autopilot runs the agent loop without requiring user approval for each step. It
is useful for long-running, low-risk tasks.

### 16.2 Autopilot Slash Commands

| Command | Purpose |
|---------|---------|
| `/autopilot on` | Enable autopilot |
| `/autopilot off` | Disable autopilot |
| `/autopilot on --max-tokens N` | Enable with token budget |
| `/autopilot on --max-time N` | Enable with time budget (seconds) |

### 16.3 Safety Limits

| Limit | Default | Description |
|-------|---------|-------------|
| Max iterations | 50 | Hard cap on tool/turn loops |
| Max tokens | configurable | Total token budget |
| Max time | configurable | Wall-clock budget |
| Permissions | auto-approve | Only within allowed rule set |

### 16.4 Completion Signalling

Autopilot ends when:

- The agent calls `agent_complete(summary)`.
- A safety limit is reached.
- The user sends `/autopilot off` or `Esc`.

### 16.4.1 `new_agent` parameter contract

`new_agent` spawns a sub-agent with a bounded task. The sub-agent MUST finish
with `agent_complete(summary)` as its final action — every sub-agent run,
without exception (including failed or empty runs, where the summary describes
what was attempted). The sub-agent system prompt carries a mandatory
"Sub-Agent Completion Protocol" section enforcing this; a run that ends
without the call leaves the task entry running and stalls `wait_agents`.
The mandatory section title itself is "Sub-Agent Completion Protocol
(MANDATORY - HARD REQUIREMENT)" and the per-turn `agent_complete` tool
description repeats the requirement so it is visible in the tool
reference as well.

A separate "Deliverable Enforcement (FILE-WRITE TASKS)" section requires a
sub-agent whose prompt names a file to call `Write`/`create` and verify the
file BEFORE `agent_complete`; an `agent_complete` whose promised file was
never written is a failed run. Every completed sub-agent run (detached or
not) also persists its FULL output to `log/subagents/<task-id>.md`, and the
`SubagentComplete` event carries the real loop `finish_reason`
(`stop`/`truncation`/`length`/`cancelled`/`error`) recorded by the session
loop so a provider-side cut is reported as truncation, not a healthy finish.
A provider that reports a **tool-use finish with no parsed tool call** is
treated the same way (spec `uniformtools` FR-022): the step is reclassified as
`Truncation` so the text-format tool-call recovery gets its chance, and a
reply with no recoverable markup ends the run with the "reply may be
incomplete" notice instead of being presented as a clean answer.
`new_agent` selects exactly one of three execution contracts from
`background` and `detached` (spec `agentssetup` FR-001, FR-003): *foreground*
(`background` absent or `false`, blocking, response returned inline),
*background* (`background: true, detached: false`) and *detached*
(`background: true, detached: true`). The contract is derived once into an
`Execution` enum so no code path can silently degrade a detached request. A
call with `detached: true` and `background` absent or `false` is **refused**
with an error naming both parameters and the two legal combinations: there is
no detached-foreground contract. A detached task is excluded from
`list_agents` and `running_background_count`,
cannot be awaited with `wait_agents`, and its completion is reaped without a
chat injection while remaining visible in the Agents panel and
`tasks_snapshot`.

The Agents panel paints that contract per row (spec `agentssetup` FR-020). The
`Event::SubagentStart` variant carries `detached` alongside `background`, the
task registry fills both from the entry it actually spawned, and the TUI stores
them on the painted row, so the panel's type column reads `fg`, `bg` or
`detached` from the registry instead of inferring it from `background` (which a
detached run also sets). The flag travels to remote consumers in the SSE
`subagent_start` payload for the same reason.

**The system prompt teaches the same contract (FR-025).** The `## Sub-Agent
Spawning` block a *primary* agent receives carries four named teaching
obligations, in the tool descriptions' vocabulary rather than a paraphrase of
it, so a model that plans a batch is not left to infer the rules from a refusal:

- **Three execution contracts.** Each is named with the `background` /
  `detached` shape that selects it (`background` absent or `false`;
  `background: true, detached: false`; `background: true, detached: true`), the
  background contract's three consequences are stated (listed by `list_agents`,
  awaitable with `wait_agents`, injected on completion) against the detached
  contract's inverse (fire-and-forget), and the illegal combination is named as
  **refused** rather than degraded.
- **The limit, its occupancy, and how to free a slot.** The block states the
  configured limit (substituted for the `MAX_BG_TASKS` placeholder from
  `experimental.max_background_agents`, so the prose and the enforcement cannot
  drift), that occupancy counts `running`, `suspended` **and** `terminating`
  background tasks while never counting a detached or foreground one, and the
  cancel-to-free rule in those words: *to free a slot, CANCEL - do not suspend*,
  because suspending frees no headroom (FR-030). It also says a cancel is only
  *requested* when `cancel_agent` returns, with the slot freeing when the
  registry reaches the terminal `cancelled` status (FR-013), and points at
  `list_agents`' `limit` / `occupancy` / `headroom` report, which describes the
  whole session even under a `status` filter (FR-011, FR-035).
- **The await contract.** `wait_agents` blocks until the awaited tasks settle;
  `task_ids` and `timeout_secs` are both named; every settled task reports its
  canonical registry status (`completed`, `failed`, `cancelled`, `timed_out`)
  so a cancelled task is never reported as a failure (FR-014); `suspended` is
  *not* settled and `timed_out` *is* (FR-015, FR-017); and a detached task can
  never be awaited (FR-044) while a foreground task is not awaited at all
  because it returned inline.
- **The durable report path.** `log/subagents/<task-id>.md`, surfaced as
  `output_file` in the `wait_agents` output, its metadata `results` entries and
  the `list_agents` table, plus the absence case: a finished task with no durable
  copy is reported as in-memory only rather than having the field omitted
  (FR-018, FR-032), so a caller never reads a path that does not exist.

**Durable-copy absence is a reported fact, not a missing field (FR-018, FR-032,
FR-043).** `persist_task_output` returns `None` when the report cannot be written
(an unwritable or read-only `log/subagents/`, a full disk, a failed rename). The
task still completes with its result in memory, so before this change the caller
saw only the *absence* of `output_file` and could not tell "the write failed and
the only copy is in memory" from "this task has no report" - a model reading the
first shape as the second `read`s a path that was never created and loses the
findings when the text is truncated. Both surfaces now name the condition through
one shared constant, `ragent_agent::tool::wait_agents::IN_MEMORY_REPORT_NOTE`:

- `list_agents` list rows print `-> [memory] <status> - report is <note>` where a
  persisted task prints `-> [file] Full report: <path>`, and the detail view
  prints `Output File: none - <note>` instead of the `Output File (full
  untruncated report)` heading it prints for a real path (FR-032).
- `wait_agents` prints `[memory] No durable copy: <note>` in the content where a
  persisted task prints `[file] Full report: ...`, and its `results` entries
  carry `report_persisted: false` plus `report_location: "in_memory_only"` (with
  `output_file` present and `null`, so a presence check cannot mistake the
  absence for a shape change); the `tasks` roll-up repeats the flag (FR-043).
- Only a *settled* task reports the verdict: a running or suspended task has not
  had a write attempted yet, so neither surface claims one failed. The verdict
  is derived in `durable_copy_absence_note`, which both tools call, so the two
  cannot drift into describing the same condition differently.

**A detached task is refused by contract, not reported as unknown (FR-044).** A
detached task (`new_agent` with `background: true, detached: true`) is
fire-and-forget: `AgentManager::list_agents` excludes it, `wait_agents`' wait set
is seeded from that listing, and nothing ever injects its completion into the
session - so a wait on its id can never settle. Before this change such an id
fell through the settled-set scan, the wait set came out empty, and the caller
was told "No running background tasks to wait for": the tool described the *one*
thing that was not wrong (the id is real) and named no way forward.

`wait_agents` now checks every requested id against the whole registry -
`list_agents` cannot be asked, precisely because it hides detached entries -
before it subscribes to the event bus or counts a waiter, so a refused call
leaves the registry exactly as it found it. A detached id returns an error
carrying the machine-readable reason `task_detached` (`REASON_TASK_DETACHED`)
whose message names, in one pass: the task (short id, full id, agent name), the
contract as the spawn shape that produced it (`background: true, detached:
true`), why a wait cannot settle (excluded from `list_agents`, never awaited,
never injected), and the durable report file `log/subagents/<task-id>.md` - the
recorded `output_file` when the run has finished, the expected path when it has
not - together with the `read` tool call that recovers it. It ends by naming
both ways forward: `list_agents` for the tasks this session *can* await, and
`new_agent(background: true, detached: false)` for a sub-agent whose result
should come back here. The word "unknown" appears nowhere in it, which is the
distinction the requirement is about.

The refusal is scoped to `detached: true`: an ordinary background task is
awaitable exactly as before, and a detached id listed alongside an awaitable one
refuses the whole call rather than awaiting the half it can see.

The block is scoped to `AgentMode::Primary`; a sub-agent receives the completion
protocol instead and cannot spawn. Because the block is rendered prose it cannot
be guarded by FR-016's schema-scan, so
`test_subagent_prompt_contract::the_prompt_uses_the_same_vocabulary_as_the_tool_descriptions`
asserts agreement in both directions against the *live* `description()` strings
rather than a hand-copied word list: a term the descriptions teach and the prompt
does not (or vice versa) is a build failure. The four teaching obligations are
pinned by `::the_prompt_teaches_the_three_execution_contracts`,
`::the_prompt_teaches_the_limit_and_how_a_slot_is_freed`,
`::the_prompt_teaches_the_await_contract` and
`::the_prompt_teaches_the_durable_report_path`, and the primary-only scoping by
`::the_contract_block_is_scoped_to_primary_agents`.

Every tool in the family (`new_agent`, `cancel_agent`, `list_agents`,
`wait_agents`) and `bg` declares a closed parameter set
(`additionalProperties: false`) and enforces it through one shared validator,
`ragent_agent::tool::param_schema::validate_tool_input` (spec `agentssetup`
FR-004, FR-038). A call carrying a property the schema does not declare is
refused with an error naming every unknown key and listing the declared ones,
instead of being silently ignored - a misspelled `backgound: true` no longer
degrades to the foreground contract. The check runs first in each tool's
`execute`, before any `AgentManager` / service lookup, so a bad call shape is
reported as a bad call shape rather than as a missing environment. `bg`
additionally refuses a *declared* property its selected action does not read
(`lines` on `status`, `timeout` on `spawn`), and `wait_agents` refuses a
`task_ids` element that is not a string instead of dropping it, so every
declared parameter is honoured or refused (FR-002).

The optional `model` override accepts one documented spelling set
(`provider/model` and `provider:model`), and every surface that documents it
says the same thing (FR-006). `provider/model` is **preferred**: the resolver
tries `/` before `:`, and a string carrying neither separator is ignored with a
warning rather than guessed at. The four surfaces are the `model` property's
schema description, the `new_agent` tool description, the primary-agent system
prompt, and `README.md`; `test_model_override_spellings.rs` pins the first
three so drift becomes a build failure, and the same test asserts the two
documented shapes are the ones the resolver accepts.

Every precondition failure in the family is an **error**, not a successful
result whose content is prose (FR-009): a caller cannot mistake "you may not do
this" for an answer. In particular, `new_agent` called from a session with an
active team context is refused with the machine-readable reason
`team_context_active` (FR-045, FR-009). The refusal names the team and this
session's role and keeps the previous guidance text verbatim: a lead is pointed
at `team_spawn`, `team_task_create`, `team_assign_task` and `team_message`; a
teammate at `team_read_messages`, `team_task_claim`, `team_task_complete` and
`team_idle`. No sub-agent task starts.

**Reason codes (FR-010, NFR-005).** Every sub-agent tool failure a caller could
branch on carries a stable machine-readable code beside its human message, so a
caller never has to pattern-match English prose:

| Code | Raised by | Condition |
|------|-----------|-----------|
| `limit_reached` | `new_agent` (via the registry) | the background limit is full |
| `contract_refused` | `new_agent` | `detached: true` without `background: true` |
| `team_context_active` | `new_agent` | a team context owns this session |
| `invalid_timeout_secs` | `new_agent` | `timeout_secs` is not a positive integer |
| `manager_unavailable` | all four | the tool ran where no `AgentManager` is wired |
| `task_not_found` | `list_agents`, `cancel_agent` | the id names no task for this session |
| `wrong_session` | `cancel_agent` | the task exists but belongs to another session |
| `cancel_failed` | `cancel_agent` | the registry rejected the cancel request |
| `invalid_status_filter` | `list_agents` | `status` is outside the declared set |
| `no_tasks` | `list_agents` | the listing matched nothing |
| `invalid_task_ids` | `wait_agents` | `task_ids` is not an array of strings |
| `wait_timed_out` / `wait_settled` | `wait_agents` | the wait outcome |
| `unknown_action` / `parameter_not_accepted` / `service_unavailable` | `bg` | the shell-task tool's own refusals |

Codes travel in the shape the failure takes. An **error** carries it at the head
of its message (`reason_code: <code>: <explanation>`), read back with
`tool::param_schema::reason_in`; a failure reported as a **result** (the
`cancel_agent` unknown-task case, an empty listing, the wait outcome) carries it
as `metadata.reason`. The task registry prefixes its own code with
`param_schema::prefixed` and `new_agent` lifts it into the tool-error envelope,
so one code is visible whichever layer refused. `bg`'s unknown-action refusal
also names the supported action set, and every refusal names at least one
supported next action (NFR-005).

**The limit is reported, not just enforced (FR-011, FR-029, FR-035, NFR-003).**
`AgentManager::limit_report()` returns one
`BackgroundLimitReport { limit, occupancy, headroom, occupying_ids }` from the
same `DashMap` scan the spawn path already performs, counting through the single
`occupies_limit` predicate that also defines `running_background_count`, so a
listing and a spawn refusal can never disagree and no storage read is added.
Occupancy counts `running` **and** `suspended` background tasks (a suspended task
keeps its wall-clock budget and can resume, FR-030), excludes `detached` ones, and
is manager-wide because the limit is. `headroom` saturates at zero, so a limit
lowered below the occupancy never reports a negative number of free slots.
Suspending a task therefore frees **no** headroom: the spawn refusal for a full
limit names the suspended task among the occupying ids and says
`suspending does not free a slot`, so cancellation - not suspension - is the
supported way to free one early.

`list_agents` carries `limit`, `occupancy`, `headroom` and `occupying_task_ids`
in the metadata of **every** exit path - populated listing, empty listing, detail
hit and detail miss - and prints one content line,
`Background limit: 1/2 slots in use, headroom 1. Occupying tasks: <id>, ...`
(the `Occupying tasks` clause appears only while a slot is in use), so a model
reading only prose still sees the headroom. The figures are computed **before** the
`status` filter is applied, so a filtered listing describes the whole session
rather than the subset it shows (FR-035). When `new_agent` is refused for the
limit, the error names the limit, the occupancy, the headroom and the occupying
task ids as well as its `limit_reached` code (FR-029), so the caller can cancel a
specific task instead of guessing. The system prompt's concurrency block points at
the headroom for the same reason. Covered by
`test_task_registry_bookkeeping::limit_report_counts_running_and_suspended_background_tasks`,
`::spawn_refusal_names_limit_occupancy_and_occupying_tasks`,
`test_list_agents_tool::limit_occupancy_and_headroom_are_reported`,
`::limit_report_composes_with_a_status_filter` and
`::detail_mode_reports_the_limit_on_hit_and_miss`, and
`test_task_suspend_resume::suspended_task_still_occupies_the_limit` drives the
real `suspend_task` transition and asserts occupancy is unchanged by it.

`waiter_count` - the registry's tally of in-flight `wait_agents` callers - is
reported per task by `list_agents` (FR-012): in a listing row
(`-> N active waiter(s)`), in the `task_id` detail view (`Active Waiters: N`)
and in the detail metadata (`waiter_count`), and on the matching Agents-panel row
as a `[wN]` suffix. It is reported **only when greater than zero** and only for a
task that has not finished, so a task nobody awaits never paints a `0` that reads
as a defect and a completed task's leftover tally is not mistaken for a live
queue. The Agents panel takes the count from the registry snapshot it already
merges, so a suspended task keeps showing its queued waiters. Covered by
`test_list_agents_tool::detail_mode_reports_the_waiter_count_of_an_awaited_live_task`,
`::detail_mode_omits_the_waiter_count_when_nobody_is_waiting`,
`::detail_mode_drops_the_waiter_count_after_the_wait_returns`,
`::list_mode_marks_only_the_awaited_live_task`,
`::a_finished_task_never_reports_waiters`,
`test_agents_window_table::an_awaited_live_row_paints_its_waiter_count`,
`::an_unawaited_row_paints_no_waiter_marker` and
`::a_suspended_awaited_row_still_paints_its_waiters`. The waiter marker is fitted
into the label column, so
`::the_waiter_marker_does_not_move_the_pinned_controls` asserts the `[stop]` and
`[x]` columns stay pinned after it.

Every settled task carries a `status` string that is the canonical
`TaskStatus::as_str()` label the registry recorded - `completed`, `failed`,
`cancelled` or `timed_out` - in `wait_agents` content (`[...] **<agent>** (task
<id>, <status>)`), in its `results[]` entries and in its `tasks[]` entries
(FR-007, FR-014). The status is taken from the registry entry, **not** re-derived
from the `success` boolean: a task that was cancelled is reported as `cancelled`
and one that failed as `failed`, so the two terminal states never collapse into
one label. The same rule applies on the event path - a `SubagentComplete` carrying
`success: false` whose entry is `Cancelled` reports `cancelled`, because the
registry, not the event, is what distinguishes them. A cancelled task publishes
`SubagentCancelled` rather than `SubagentComplete`, so it reaches the wait only
through the registry scan; that asymmetry is exactly why the label is sourced
from the entry. `success` is kept alongside `status` as its boolean refinement
(and always agrees with it) so callers written against the older metadata shape
keep working. The implicit wait seeds every settled background task, matching on
`TaskStatus::is_terminal()` instead of a hardcoded status list, so a task that hit
its per-call time bound (FR-017) is reported like one that was cancelled or
failed rather than silently vanishing from the count. Covered by
`test_wait_agents_results_meta::test_wait_agents_reports_cancelled_not_failed`,
`::test_wait_agents_status_uses_the_canonical_task_status_vocabulary`,
`::test_wait_agents_keeps_success_boolean_as_a_refinement` and
`::test_wait_agents_event_path_prefers_registry_status_over_the_event_boolean`.

**One definition of "finished" (`TaskStatus::is_terminal`).** `Completed`,
`Failed`, `Cancelled` and `TimedOut` are terminal; `Running`, `Suspended` and
`Terminating` are not. Reaping (`drain_completed`), result injection, kill
refusal, both wait tools and the TUI Agents panel all consult that one predicate,
so a new status cannot be classified as finished on one path and live on another
(`ragent_agent::tool::wait_agents::is_settled` is a thin delegate to it).
`drain_completed` previously treated anything that was not `Running` as settled,
which injected a *suspended* or *terminating* task as an empty completion and then
removed it from the registry - a later `resume_task` failed with "not found" and
the real result was lost. It now skips a non-terminal entry entirely.

The same predicate closes three adjacent lifecycle holes:

- `kill_task` refuses a `timed_out` task instead of re-killing a run that already
  ended;
- `suspend_task` and `resume_task` check and set the status under one guard, so a
  task that finished between the check and the write is not overwritten back to
  `Suspended`, and the park flag is stored under that same guard;
- `cancel_all` also cancels suspended tasks, and a finished foreground
  (`spawn_sync`) entry is marked reported and reaped by the next drain instead of
  living for the process lifetime. `spawn_sync` returns the snapshot taken under
  the completion guard, so a concurrent drain cannot reap the entry it just
  reported and turn a successful run into "vanished after completion".

`new_agent`'s foreground metadata reports the registry status rather than a
hard-coded `completed`, so a foreground run that failed is not reported as a
success.

**A settled task keeps its result across a reaping race.** `Event::SubagentComplete`
carries the terminal `status` and `output_file` alongside `success`, so the
Agents panel renders the real outcome and the durable report path without a
second registry lookup that could race with the drain having removed the entry.
A failed run logs `[err] Task failed`.

**The background drain flag is not cleared while work is outstanding.**
`AgentManager::has_pending_background` (the flag that lets the completion drain
skip a scan) is cleared only when no background task is live *and* no other
session has a pending result, and every background completion re-arms it, so a
result is never stranded by a scan that ran while a sibling was still working.

A task that has **not** settled when the wait returns is reported by its real
state too, so a parked task is never described as running (FR-015). The
partial-result headline separates the two: `N task(s) still running: <ids>` names
only tasks that are actually working, and `N task(s) suspended (parked - nothing
progresses until they are resumed or cancelled): <ids>` names the parked ones; a
wait holding only running tasks keeps the original wording. The metadata splits
the same way, with `still_running` / `still_running_count` carrying only live
work and `suspended` / `suspended_count` carrying the parked ids, while
`unsettled` lists every unsettled task with its canonical `status` (`suspended`
for a parked one). This required two corrections inside the wait: the
pre-subscription collection and the periodic scan both treated "not `Running`"
as "settled", which collected a parked task as an empty completion, and
`AgentManager::increment_waiter` refused a non-`Running` task, which is what made
the parked task look collectable - it now increments for `Running` **and**
`Suspended`, because a wait parked on a suspended task is an active waiter
(FR-030, and the `waiter_count` FR-012 reports). Covered by
`test_wait_agents_suspended::test_wait_agents_reports_suspended_task_as_suspended`,
`::test_wait_agents_unsettled_entries_carry_the_suspended_status`,
`::test_wait_agents_keeps_a_running_task_out_of_the_suspended_set` and
`::test_wait_agents_mixed_running_and_suspended_are_reported_separately`.

The four tools read and write one object (`AgentManager::tasks`), so they carry
one permission taxonomy (spec `agentssetup` FR-005) - three categories over one
registry, never `none`:

| Tool | `permission_category()` | Why |
|------|-------------------------|-----|
| `new_agent` | `agent:spawn` | the only action that creates a task |
| `cancel_agent` | `agent:control` | a control action on a task that already exists |
| `wait_agents` | `agent:control` | the same control action, blocking instead of terminal |
| `list_agents` | `agent:read` | a read-only observation of the registry |

`cancel_agent` previously reported `agent:spawn`, which conflated "start work"
with "stop work": a policy could not grant spawning while denying cancellation.
`list_agents` previously reported `none`, which short-circuits the permission
layer entirely (`processor.rs` returns before any rule lookup), so a policy
written against its category never ran - and a read of another agent's work is
something a policy must be able to deny without denying the spawn. The
classification tightened and the interactive outcome did not: all four remain in
`AUTO_APPROVED_AGENT_TOOLS`, so no sub-agent call gains a prompt. The `--dry-run`
family table keeps its count (the `agent` family grows 4 -> 5, the family total
stays 21), and `test_agent_permission_taxonomy.rs` pins all four categories as
literals so a regression cannot agree with itself.

### 16.4.2 Live sub-agent output overlay

The TUI output-view overlay (click a row in the Agents panel, or `Alt+A` /
the Agents popup) renders the target session's persisted transcript from
SQLite, so a running sub-agent's steps are visible mid-run, not only after
completion. Two mechanisms keep the overlay in step with the run:

- the session loop's interim assistant save persists completed tool-call
  parts into the child session's message row as each step finishes (the
  M-008 save gate is a total-parts count, so a tool-only step still saves;
  FTS stays untouched until the final save), and
- the overlay's line-cache generation key mixes in the same per-session
  step / tool-call counters the Agents panel shows, so the cache rebuilds
  whenever the agent advances and otherwise reuses the rendered rows
  (PERF-048 invariant).

Before the counter mix, the generation key (message count + `edit_seq`,
neither of which changes mid-run) kept the overlay frozen at whatever it
rendered when opened; before the total-parts gate, a tool-only step skipped
the interim save entirely, so nothing new was visible until the final save.

### 16.5 Status Display

Autopilot state is shown in the status bar as `AUTOPILOT` with iteration and
token counters.

---

## 16A. Cron Scheduling

Ragent supports scheduling agent runs with a cron-like system. Users can
schedule one-shot or repeating events, each with a designated agent type and an
initial prompt. Events are persisted to SQLite and evaluated by a background
scheduler while the TUI session is running.

### 16A.1 Schedule Grammar

Three schedule forms are supported:

| Form | Behaviour | Example |
|------|-----------|---------|
| `at <timestamp>` | One-shot. Fires once at the specified time. | `/cron add nightly general at 2025-01-15T09:00 "Run tests"` |
| `from <timestamp> every <duration>` | Repeating. First fire at the timestamp, then every duration. | `/cron add nightly general from 2025-01-15T09:00 every 30m "Run tests"` |
| `every <duration>` | Repeating, no explicit start. First fire is duration from now. | `/cron add nightly general every 2h "Run tests"` |

Durations are a positive integer + unit:

| Unit | Meaning | Aliases |
|------|---------|---------|
| `m` | minutes | `min`, `mins` |
| `h` | hours | `hr`, `hrs` |
| `d` | days | `day`, `days` |
| `w` | weeks | `wk`, `wks` |
| `mo` | months (30 days) | `month`, `months` |

Timestamps accept ISO-8601 (e.g. `2025-01-15T09:00:00Z` or
`2025-01-15T09:00:00+02:00`) or natural-language shortcuts resolved against
the user's local timezone:

| Shortcut | Meaning |
|----------|---------|
| `5pm` | Next 5pm (today if not yet passed, else tomorrow) |
| `5:30pm` / `5:30 pm` | Next 5:30pm |
| `17:00` | Next 17:00 (24-hour clock) |
| `5am tomorrow` | 5am on the following day |
| `12pm` | Noon |
| `12am` | Midnight |

### 16A.2 Slash Commands

| Command | Description |
|---------|-------------|
| `/cron add <cronname> <agent> <schedule> "<prompt>"` | Schedule a new event (`cronname` becomes the event ID) |
| `/cron remove <event_id>` | Remove a scheduled event |
| `/cron enable <event_id>` | Enable a previously disabled event |
| `/cron disable <event_id>` | Disable an event (skipped by the scheduler) |
| `/cron list` | List all events with human-readable schedule descriptions |
| `/cron detail <event_id>` | Show full details of a single event (untruncated prompt) |
| `/cron log [event_id]` | Show execution log (optionally filtered by event id) |
| `/cron help` | Show usage |

The model can also manage cron events directly via the LLM-callable tools
`cron_add`, `cron_remove`, `cron_list`, `cron_enable`, and `cron_disable`.

### 16A.3 Scheduler

A background scheduler task runs while the TUI session is active, ticking every
30 seconds. On each tick, it queries all enabled events whose `next_due` has
passed and spawns a background agent run for each. For repeating events,
`next_due` is advanced by one duration interval. For one-shot events, the event
is marked as fired.

### 16A.4 Execution Logging

Every execution is logged as a single JSON line appended to
`<working_dir>/log/cron-<timestamp>.jsonl`, mirroring the edit-log convention.
Each entry records:

- Event id
- Agent type
- Prompt
- Outcome (`"success"`, `"error"`, or `"skipped"`)
- Error message (if any)
- Completion timestamp

### 16A.5 Guards

- **Disabled events** are skipped with a `"skipped"` outcome.
- **Unknown agent types** are logged with an `"error"` outcome.
- **Double-fire prevention**: if a repeating event's previous execution is
  still running, the current due cycle is skipped and logged as `"skipped"`.

### 16A.6 Past-Start Advancement

When a `from <timestamp> every <duration>` event has a start timestamp in the
past, `next_due` is advanced by whole duration intervals until it is strictly in
the future. The original `start_at` timestamp is preserved unchanged.

---

## 17. Orchestrator & Multi-Agent Coordination

### 17.1 Core Components

| Component | Responsibility |
|-----------|----------------|
| `Coordinator` | High-level task decomposition and dispatch |
| `AgentPool` | Manages active sub-agents |
| `ConflictResolver` | Detects and resolves conflicting edits |
| `TransportAdapters` | Bridges TUI, HTTP, and mailbox transports |

### 17.2 Coordinator API

The coordinator exposes methods for:

- `decompose(prompt)` → list of subtasks
- `dispatch(subtasks)` → spawn agents
- `collect(results)` → aggregate outputs
- `synthesize(results)` → final response

### 17.3 HTTP API Endpoints

Orchestrator operations are available via `/sessions/{id}/messages` when the
session is configured with the `orchestrator` agent.

### 17.4 Conflict Resolution

When multiple agents edit the same file, the conflict resolver:

1. Detects overlapping edits.
2. Applies non-overlapping edits highest-end-offset-first.
3. Reports conflicts for manual resolution.

### 17.5 Transport Adapters

| Adapter | Use |
|---------|-----|
| TUI | Real-time UI updates |
| HTTP SSE | Remote clients |
| Mailbox | Team teammate notifications |

### 17.6 Current Status

The orchestrator is functional for swarm decomposition and simple parallel
workflows. Advanced conflict resolution across arbitrary file sets is under
active development.

---

# Part VI: External Integrations

---

## 18. GitHub & GitLab Integration

### 18.1 GitHub Authentication & Slash Commands

Authentication uses `GITHUB_TOKEN`. GitHub Enterprise is supported via the
`github_api_base` config.

### 18.2 GitHub Issue Tools

| Tool | Purpose |
|------|---------|
| `github_list_issues` | List repository issues |
| `github_get_issue` | Read an issue |
| `github_create_issue` | Create an issue |
| `github_update_issue` | Update an issue |
| `github_close_issue` | Close an issue |

### 18.3 GitHub Pull Request Tools

| Tool | Purpose |
|------|---------|
| `github_list_prs` | List PRs |
| `github_get_pr` | Read a PR |
| `github_create_pr` | Create a PR |
| `github_merge_pr` | Merge a PR |
| `github_close_pr` | Close a PR |

### 18.4 GitHub Auto-Detection

The local git remote is inspected for `github.com` or a configured GitHub
Enterprise host. When detected, GitHub tools are exposed automatically.

### 18.5 GitLab Authentication & Slash Commands

Authentication uses `GITLAB_TOKEN` or `gitlab.com` OAuth. Self-managed GitLab
is supported via `gitlab_api_base`.

### 18.6 GitLab Issue Tools

| Tool | Purpose |
|------|---------|
| `gitlab_list_issues` | List project issues |
| `gitlab_get_issue` | Read an issue |
| `gitlab_create_issue` | Create an issue |
| `gitlab_close_issue` | Close an issue |

### 18.7 GitLab Merge Request Tools

| Tool | Purpose |
|------|---------|
| `gitlab_list_prs` | List MRs |
| `gitlab_get_pr` | Read an MR |
| `gitlab_create_pr` | Create an MR |

### 18.8 GitLab CI/CD Pipeline Tools

| Tool | Purpose |
|------|---------|
| `gitlab_get_pipeline` | Read a pipeline |
| `gitlab_list_jobs` | List pipeline jobs |
| `gitlab_get_job` | Read a job |
| `gitlab_retry_job` | Retry a job |
| `gitlab_cancel_job` | Cancel a job |

### 18.9 GitLab Project Tools

| Tool | Purpose |
|------|---------|
| `gitlab_list_projects` | List accessible projects |
| `gitlab_get_project` | Read project metadata |

### 18.10 GitLab Auto-Detection

GitLab tools are auto-enabled when a `gitlab.com` or configured self-managed
host is detected in the git remote.

---

## 19. MCP Integration (Model Context Protocol)

### 19.1 Discovery

Ragent can discover MCP servers automatically. Known server types include:

- `filesystem`
- `git`
- `github`
- `fetch`
- `sqlite`
- `postgres`
- `brave-search`
- `everything`
- `puppeteer`

### 19.2 Configuration

MCP servers are configured in `ragent.json`:

```jsonc
{
  "mcp": {
    "servers": [
      {
        "name": "fetch",
        "command": "uvx",
        "args": ["mcp-server-fetch"]
      }
    ]
  }
}
```

### 19.3 Tool Bridging

MCP tools are wrapped as `mcp_tool` and exposed to the agent with dynamically
generated JSON schemas. The wrapper handles:

- stdio transport
- request/response correlation
- error translation

### 19.4 Status

MCP support is functional for stdio and HTTP servers. Auto-discovery and
configuration editing are supported via `/mcp` slash commands.

#### 19.4.1 Server enable/disable state

Whether an MCP server is actually started is a persisted, user-owned choice
rather than an implicit side effect of being listed in `ragent.json`. The choice
lives in a **global ledger** (`<global state dir>/mcp_state.json`) shared by the
connect path, the permission gate, and the `/mcp` and `/plugins` surfaces:

- A server id **absent** from the ledger is enabled, so a newly added server -
  written into `ragent.json` or bridged from a plugin's `mcpServers` section -
  starts enabled with no extra step.
- `mcp.<id>.disabled: true` in `ragent.json` always disables the server (the
  config flag is the harder switch), even if the ledger says otherwise.
- A disabled server is registered with `McpStatus::Disabled` and **no child
  process is spawned**; `/mcp` still lists it and can re-enable it.
- `/mcp connect <id>` enables and connects a server live, registering its tools
  immediately; `/mcp disconnect <id>` disables and disconnects it live. Both
  persist the choice globally, so it survives a restart and applies to every
  project.
- `McpToolWrapper::execute` refuses to call a disabled server's tools even when
  the tool is still registered from an earlier connection, naming the command
  that re-enables it.

#### 19.4.1a Removing a definition (`/mcp remove`)

`/mcp remove <id>` is the destructive counterpart of connect/disconnect: it
disconnects the server and deletes its entry from the `mcp` section of the
**user-global** `ragent.json`, then clears the server's enable-ledger row so a
re-added server starts enabled again rather than inheriting a stale disable. The
session-side disconnect runs *before* the file write, so a failed write leaves
the running state consistent with the file; a server that was not connected
simply has nothing to disconnect.

The edit goes through `Config::edit_global_config(edit)`, which reads the file as
raw `serde_json::Value`, hands the top-level object to the caller's closure, and
writes it back only when the closure reports a change. It exists because
serialising the *merged* config would drop every key this build does not model,
so an unrelated key in the operator's file would be lost by an edit that only
meant to touch `mcp`. Every other key survives, an `mcp` object emptied by the
removal is deleted rather than left as `{}`, and the load cache is invalidated so
the next `Config::load` sees the new value.

**A bridged server is refused.** A server whose definition lives in an installed
plugin's `mcpServers` (or the external `mcp.json` that entry names) or in a
connector's `connector.json` belongs to that owner: pruning it would leave the
owner installed declaring a server list it no longer has, which the next bridge
scan reports as drift. The refusal names the owning command - `/plugins remove
<id>` or `/connectors remove <id>` - while `/mcp disconnect <id>` still stops the
server without uninstalling its owner. Both owners resolve through one
`app::mcp_bridge` module (`merged_mcp_servers`, `bridged_servers`,
`resolve_config`, `find_bridged`, `is_bridged_source`), which `/mcp`,
`/mcp config`, `/mcp discover` and `/mcp connect` all share, so a
connector-bridged id is accepted everywhere a plugin-bridged one is.

#### 19.4.2 Transports, sessions, and startup reporting

A server is reached over the transport its config declares. Only the bundled
MongoDB server gets a default: ragent does **not** start a second copy of a
server it expects to be long-lived, so the shipped `mongodb` plugin, whose
`mcp.json` declares no transport, is bridged to `McpTransport::Http` at
`http://127.0.0.1:3000/mcp`. If a server is already listening there it is
adopted; otherwise the HTTP endpoint (not a stdio child) is what the server is
started as. Every other plugin entry and every `ragent.json` entry keeps the
transport it declares (default `stdio`).

The HTTP client performs the MCP `initialize` handshake before `tools/list` and
replays an issued `mcp-session-id` on every later request, because a sessionful
Streamable-HTTP server (the MongoDB server on its 2025-era path) rejects
requests on a session it did not open. Replies may be bare JSON-RPC or an
`event: message` SSE frame; both are accepted, and a frame whose JSON payload
is split across multiple `data:` lines is re-joined with `\n` per the SSE
grammar rather than truncated to the first line.

A stdio child is never adoptable - its pipes are private to the parent that
spawned it - so a config declaring `stdio` always results in a fresh child.
A stdio child leads its **own process group** (`process_group(0)` at spawn, the
same pattern as the bash-timeout harness), because launcher commands (`npx`,
`npm`, `bunx`) fork the real server (`node`) and exit immediately - killing
only the recorded launcher pid would orphan the process actually holding the
stdio pipes. Process teardown (`shutdown`) therefore SIGKILLs the *group*
(`killpg`) after attempting the graceful rmcp `cancel()` — the kill is
unconditional and ESRCH-safe, so a connection that `/mcp disable` already
tore down needs no special-casing — and drains the connections map atomically
so a second ragent instance starting at the same
time never reaps a child another instance is still using. Before spawning,
`connect` sweeps `/proc` (Unix only) for **orphaned** earlier
copies of the same `command` + `args` and SIGKILLs them, because an orphaned
stdio server can no longer serve anyone yet keeps holding file locks, ports it
also binds, API rate-limit slots, and memory. A process counts as orphaned only
after being re-parented away from its spawner: `/proc/<pid>/stat` field 4
(`Ppid`) equals `1`, or the parent is the per-user service manager
(`systemd --user`, matched on `/proc/<parent>/comm` or argv[0]). A same-command
process whose own parent is still alive belongs to a *running* ragent instance
and is never killed, so concurrent sessions sharing one MCP stdio command do
not sever each other's pipes (`Transport closed`). Desktop sessions re-parent a
dead ragent's launcher tree to the user manager rather than to init, so the
`Ppid == 1` check alone missed exactly the orphans this sweep exists to reap.
Matching is per process: any supervisor-reparented process whose own command
line matches is swept, and its process group is signalled, so the launcher
(`npm exec mongodb-mcp-server@<3>`) and the `node` child it forked are both
reclaimed even though only the child carries the package token. The
current process is always
excluded, matching keys on the executable basename (`/usr/bin/npx` == `npx`)
plus, for launcher commands (`npx`, `npm`, `bunx`, `pnpx`), the package token
with any `@<version>` specifier stripped - so `npx -y mongodb-mcp-server@<3`
and the node child it spawns (whose argv only mentions the package inside the
`node_modules` path) are both swept, while an unrelated `npx -y other-server`
and any same-named process outside the package tree survive. A launcher
invocation with no positional package argument matches nothing (killing every
`npx` on the machine would be a bug, not cleanup). When the stdio command line
declares an HTTP port (`--httpPort <n>` / `--port <n>`), that port is probed
with a short-timeout `initialize` first and a live MCP server there is adopted
instead of spawning a child; a bare TCP listener that fails the handshake is
rejected as not-MCP.

The startup connect loop publishes the shared `McpClient` **before** connecting
the first server, so every reader (the tool registry, `/mcp`, the startup
report) sees the live handle from the beginning; a one-shot run no longer races
the loop. Startup does not block on the loop: the TUI adopts whatever state the
loop has published, then prints its one-shot per-server report off the loop's
completion sentinel (an empty-id `McpStatusChanged { status: "initialized" }`
event), once, after every server has settled. The report prints one `[mcp]` line
per server — `Starting <id> (<transport>)` plus `Connected ... via <transport>`,
or `Skipping ... (<transport>, disabled)`, or `Failed to connect ... : <reason>`,
or `... needs authentication` — where `<transport>` is the config's declared wire
protocol (`stdio` / `sse` / `http`, via `Display for McpTransport`).
`report_mcp_startup` latches its own one-shot; a `try_read` miss (the connect loop
holds the client write lock across a connect) defers the report without latching,
so the next sentinel retries it. Every `connect` failure path — a config rejected
by `validate_mcp_config`, a closed spawn semaphore, or a failed handshake —
records the server as `McpStatus::Failed { error }` before returning, so a startup
error is reported alongside the servers that came up instead of only being logged.
The connect loop publishes this client-derived status (`connected`, else
`failed: <reason>`) on its per-server `McpStatusChanged` event, so `/mcp` shows the
reason as well.

The TUI reconciles the session tool registry against the live client whenever a
connect settles or a server is enabled/disabled: a registered MCP tool whose
`mcp_<sanitized-server>_` name prefix no longer matches any Connected server is
removed (`ToolRegistry::remove_all`), so a multi-segment id like
`mongodb.mongodb` (`mcp_mongodb_mongodb_*`) is recognised by prefix match
rather than by its first `_`-segment, and disabling a server leaves no dead
wrappers for the model to call. The registered name itself comes from one
place, `McpToolWrapper::ragent_name_for`, which the wrapper constructor and the
TUI both use.

#### 19.4.3 Discovery-location report (`/mcp config`)

`/mcp config` reports **where** ragent looks for MCP server definitions, how many
it finds at each place, and the individual servers discovered. It is backed by
`McpClient::discover_locations`, which returns a `Vec<McpDiscoveryLocation>`
instead of the flat, de-duplicated server list `discover` yields. Each location
carries a label, the scanned path (absent for the `PATH` and npm-global rows),
whether the path exists, and the definitions found there, before de-duplication.

The scanned locations, in order, are: `PATH` (the known server executables),
npm global `node_modules`, and each registry directory — the ragent-native
`~/.config/ragent/servers/` first so its definitions win de-duplication, then
`~/.claude/mcp-servers/`, `~/.cline/mcp-servers/`, `~/.mcp/servers/`,
`<config>/mcp/servers/`, and `<config>/claude/mcp-servers/`. The command renders
a markdown table of every location (label, path with a `(missing)` marker for an
absent directory, and a count), a total, and a second table of the discovered
servers (id, the location it came from, its launch command, and the file that
declares it). `discover` is
unchanged: it flattens the location results and de-duplicates by id.

The **declaring file** column is what tells a `ragent.json` entry apart from a
bridged one before `/mcp remove` is run (§19.4.1a): a `PATH`/npm/registry
discovery reports `-` (it has no definition file until it is adopted), while a
plugin- or connector-bridged server reports the absolute path of the manifest
that declares it - the plugin manifest, the external `mcp.json` its `mcpServers`
entry names (resolved and confinement-checked against the plugin root), or the
connector's `connector.json`. The discover dialog carries the same information in
its `Source / Defined in` column, and its width ceiling is 170 columns so an
absolute path is not clipped away. `/mcp` prints the same mapping as a separate
`Defined in:` block plus a `Managed by:` block naming each bridged server's owner,
because a seventh table column wrapped every header cell past the message width.


---

## 19B. Durable Initiatives & Skill Management

### 19B.1 Initiative Tool (`initiative`)

The `initiative` tool manages durable, cross-session goals with milestone
tracking. Unlike session-scoped `todo_*` items, initiatives are persisted in
the project-scoped `initiatives` SQLite table and survive compaction, session
restarts, and machine reboots. Any session running in the same working
directory sees the same initiatives.

Actions: `create`, `read`, `update`, `checkpoint`, `list`, `close`.

```jsonc
initiative action="create" id="api-v2" title="Ship API v2"
           milestones=["design","implement","deprecate v1"]
initiative action="checkpoint" id="api-v2" milestone="ms-1" progress=33
           note="Design doc merged"
initiative action="list"                       // active only (default)
initiative action="list" status="all"          // includes closed
initiative action="close" id="api-v2" status="completed"
```

`checkpoint` marks a milestone complete (recording `completed_at`), bumps the
overall progress percentage, and appends the free-text `note` as a timestamped
`Checkpoint:` line in the description so there is a durable audit trail.
`close` requires `status` of `completed` (auto-fills progress to 100) or
`abandoned` (keeps the recorded progress).

Active initiatives are injected into the system prompt on every turn under
`## Active Initiatives`, listing id, progress, and the next pending milestones
so the agent stays aware of long-term goals across turns and sessions.

Registered under the `storage:write` permission category.

### 19B.2 Skill Management Tool (`Skill`)

The `Skill` tool exposes runtime control of the skill registry
(SPEC §12) without requiring a session restart.

Actions:

- `list` — enumerate registered skills (name, scope, invocation flags,
  description). Optional `scope` filter; `include_bodies=true` also prints each
  skill's prompt body.
- `read` — return one skill's fully processed prompt body. `arguments` are
  substituted into `$ARGUMENTS`-style placeholders, exactly as a `/skill`
  invocation would.
- `load` — (re)discover skills from disk and return the named skill's prompt,
  the same content the model receives on `/skill` invocation. Use it to
  "inject" a skill added or edited after session start.
- `reload` — drop cached skill bodies, re-discover from disk, and report
  which skills were added/removed since the previous scan, plus the bundled
  baseline count.

```jsonc
Skill action="list"
Skill action="read" name="rust-error-handling" arguments="ctx"
Skill action="load" name="rust-error-handling"
Skill action="reload"
```

Registered under the `skill:manage` permission category.

---

## 19C. Connector System (Claude-Connector-Equivalent Integrations over MCP)

Spec: `specs/connectors/` (`SPEC.md` FR-001..FR-041, NFR-001..003; `PLAN.md`
T-001..T-021; `TESTPLAN.md`). Crate: `crates/ragent-connectors` (17th workspace
crate).

A **connector** is a named, user-facing integration (Google Drive, Slack,
GitHub, Git, Postgres, Puppeteer, and the wider community catalogue) that reaches
an external system through the **Model Context Protocol (MCP)**. A connector is
a catalogue entry carrying a display name, a category, an authentication
requirement, and one or more MCP servers; the MCP server remains the transport
ragent already speaks (`McpClient`, §19). The connector system is therefore a
**catalogue and management layer** over machinery that already exists (the MCP
client, the durable `mcp_state.json` enable ledger, and the encrypted credential
store), adding the connector abstraction and one coherent surface.

### 19C.1 Descriptor model

A connector is a normalised `ConnectorDescriptor` holding its id, display name,
category, one or more server definitions, an authentication shape, and an
optional credential *name*. Validation (FR-002, FR-025) rejects a descriptor
that cannot be expressed as one or more MCP servers (an unexpressible transport
such as `grpc` is recorded with the label `server transport` rather than treated
as fatal). A malformed catalogue entry is skipped and counted, never fatal
(FR-031).

### 19C.2 Store and enable ledger

The connector store is discovered under `.ragent/connectors/` (project),
falling back to `~/.config/ragent/connectors/` (user-global) (FR-001), with an
optional `connectors.store_dir` override. A connector install is
**path-traversal-safe**: an archive member that escapes the store root (for
example `../../escape.json`) is refused (FR-029), and a non-`https` package URL
is refused (FR-028). A freshly installed connector is recorded **enabled** and
is ready to use; `/connectors enable <id>` connects its servers now in a running
session (FR-011). The durable enable state is the **existing**
`ragent_agent::mcp::enable_state` ledger (`mcp_state.json`), so a connector
disabled once stays disabled everywhere and the connector crate adds no second
enable file (FR-018). `remove` is refused while the connector is enabled
(FR-030), and a duplicate connector id is refused (FR-027).

### 19C.3 Catalogue, providers, and the default endpoint

A catalogue index is fetched over `https` under a byte cap, a per-fetch
wall-clock timeout, and a TTL cache (FR-024, FR-031). Each catalogue is
normalised through a `ConnectorProvider` shaped like
`ragent_plugins::store_provider::StoreProvider` (tolerant per-entry skip with a
`skipped` counter), so the browser domain stays catalogue-agnostic. A compiled
default Claude connector-catalogue endpoint constant and a pure, offline
provenance resolver (FR-034..FR-038, NFR-001..003) mirror the plugin store's
`DEFAULT_CLAUDE_STORE_URL` / `effective_endpoint_with_source` discipline; the
literal is declared exactly once, enforced by
`scripts/check-connector-endpoint-literal.sh` (FR-038, NFR-001).
`/connectors stores [--check]` reports each endpoint with its provenance
(`default` or `config`) and, with `--check`, probes it (FR-036).

### 19C.4 Bridge to the MCP client

The bridge resolves every enabled connector's server definitions into
`(bridged_server_id, McpServerConfig)` pairs (FR-003). Bridged server ids are
`<connector-id>.<server>`, the same collision-avoidance shape the plugin MCP
bridge uses (spec `plugins` FR-030); a collision is refused (FR-033). The
session feeds those configs to the existing `McpClient`, so no new transport is
written: every connector resolves to stdio, SSE, or HTTP exactly as a configured
MCP server does, and its tools surface under the normal
`mcp_<sanitized-server>_<tool>` names (FR-020). A bridged server that fails to
connect or returns a tool error is contained and named (FR-016); a disabled
connector starts nothing and is invisible to the model (FR-018).

### 19C.5 Authentication

A connector's secrets (tokens, client secrets, refresh tokens) live in the
existing **encrypted credential store**; the connector manifest stores only the
credential *name*, never the value (FR-005). `/connectors auth <id>` reports a
connector's auth state and lets a token be supplied (FR-014); a connector that
needs authentication and has no valid credential reports `needs auth` and its
servers are refused until it is satisfied (FR-022, FR-023, FR-032), with no
retry loop on an auth failure. The `connectors.credentials` config block maps a
connector id to non-secret credential **names** for environments where the
credential must be resolved from an environment variable.

### 19C.6 The `/connectors` command family

Thirteen subcommands, dispatched through
`crates/ragent-tui/src/app/connector.rs`, registered in `SLASH_COMMANDS`, and
listed in the command catalog and the `/` autocomplete menu (seeded from the
crate's `CONNECTOR_SUBCOMMANDS` token list so the menu cannot drift from the
usage block) (FR-004):

| Subcommand | Purpose |
|------------|---------|
| `/connectors list [--verbose] [--category <name>]` | List installed connectors with state, auth state, category, and bridged server/tool counts (FR-009) |
| `/connectors claude [query] [--category <name>] [--refresh]` | Open the interactive catalogue browser (FR-010) |
| `/connectors add <id\|source> [--force]` | Install a connector (recorded enabled; FR-011) |
| `/connectors remove <id>` | Uninstall (refused while enabled; FR-030) |
| `/connectors enable <id>` | Enable and connect the connector's servers now (FR-012) |
| `/connectors disable <id>` | Disable and disconnect (FR-013) |
| `/connectors connect <id>` | Connect an enabled connector's servers without a restart (FR-019) |
| `/connectors disconnect <id>` | Disconnect, leaving the connector enabled (FR-019) |
| `/connectors auth <id>` | Manage the credential and report auth state (FR-014) |
| `/connectors test <id>` | Isolated connect-and-invoke harness (FR-015, FR-016) |
| `/connectors stores [--check]` | Report each catalogue endpoint's provenance (FR-036) |
| `/connectors marketplace show` / `marketplace claude list\|add\|remove` | Manage the additional catalogues the browser fetches (FR-042, FR-043) |
| `/connectors help` | Usage block (FR-017) |

A bare `/connectors`, `/connectors help`, and an unrecognised subcommand all
render the same usage block. Every report carries a `From: /connectors <sub>`
attribution line and is ASCII-only (FR-006). The category filter (FR-039,
FR-040, FR-041) restricts the displayed rows; `ALL` (any case) or a blank value
clears it, and an unknown category is refused with an `[err]` row and changes no
state. The interactive `claude` browser is the single catalogue surface (FR-010):
its `--category <name>` launch argument and its in-panel `c` cycle key select the
filter over the categories the fetched catalogue declares, so there is no
standalone `search` subcommand and the browser and the textual `list` report
share one filter model. The single-reference subcommands (`remove`, `enable`,
`disable`, `connect`, `disconnect`, `auth`, `test`) accept a connector by
canonical id, slug, or display name; the reference is resolved to the installed
connector's canonical id before the lifecycle call.

The TUI drives a single published lifecycle session. `src/main.rs` resolves the
enabled connectors through the bridge into the same background connect loop that
starts the configured `ragent.json` and plugin-contributed servers, then
publishes a `ConnectorSession` and its status snapshot on the session processor
(type-erased, so `ragent-agent` does not depend on `ragent-connectors`).
`/connectors enable|disable|connect|disconnect` drives that session and
reconciles the MCP tool registry afterwards, and `/connectors list` takes its
per-server tool counts from the live MCP client, so a bridged server that
connected after startup reports a real count (T-008).

### 19C.7 Catalogue browser (`/connectors claude`)

A modal overlay in `crates/ragent-tui/src/layout.rs`, wearing the same design
language as the plugin-store panel (spec `connectors` FR-047): a calm slate accent
frame capped at 140x41; a structured title line (catalogue name badge, accent
counts, and the active `search`/`category`/`marketplace` filters; a
`loading details...` hint is appended while the manifest-description enrichment is
still running); a left column split into a `Categories` navigator above a
`Marketplace` navigator, each under a reversed-video title band with every row
labelled `Name (N)`; a pinned dark-on-light column-heading row above zebra-banded
result rows carrying a `> ` cursor gutter, a one-column bold-green `*` lane on an
installed connector, the id and category columns, the marketplace in the accent
colour (or `-`), and a description running to the panel's right edge; a centred
footer of key hints; and a bottom ` search: ` box with a clickable `[x] clear`
button. The category navigator lists `ALL`, an `Installed` pseudo-category matched
against the store scan, then the catalogue's declared categories. Three panes are
focusable (`Results` initial, then the two navigators, cycled by `Tab`/`BackTab`
with only the focused pane's cursor filled); `Up`/`Down` move the focused pane and
apply a navigator row, `ENTER` over a navigator applies and returns focus to the
results, `c` clears the focused navigator's filter to `ALL`, and `Backspace`/`Esc`
edit the query (`Esc` on an empty query dismisses the panel). The catalogue fetch
and the `ENTER` install both run **off the event loop** on a blocking worker and
deposit their result for the UI thread to drain, so the fetch never blocks the
loop or an agent turn. The launch parser
removes both `--refresh` and `--category <name>` from the query tokens (so a
pre-filled query may contain spaces) and validates the category against the
categories the fetched catalogue actually declares, falling back to an
unfiltered launch when it is unknown; a refused category is reported in the
footer while every row stays visible (FR-041).

### 19C.7a Catalogue marketplaces and description enrichment

The panel fetches the base catalogue endpoint and every marketplace added
through `/connectors marketplace claude add`, merging their entries into the one
list (`store_ops::fetch_all_catalogues`,
`CatalogueKind::effective_endpoints_with_source`); a duplicate id keeps the first
occurrence (the base endpoint's) and a refused added URL is skipped and surfaced
as a warning in the message window rather than blinding the panel. Each merged
entry carries the display name of the marketplace it came from in its
`marketplace` field (`marketplace_name_for`: the added marketplace's `name`, or
the catalogue label for the base endpoint), rendered in the browse row.

After the catalogue lands, each entry whose `source` is a `github.com` `git+`
source has its **blank** description resolved from its own repository manifest off
the event loop (`crates/ragent-connectors/src/catalogue_enrich.rs`, ported from
`ragent_plugins::store_enrich`): the raw URL
`https://raw.githubusercontent.com/<owner>/<repo>/<ref>/<subpath>/connector.json`
is fetched (with `.claude-plugin/connector.json` tried after it) through a
bounded thread pool, best-effort and never fatal, cached per URL, and behind the
injectable `ManifestSource` seam (the production `NetworkManifestSource`).
`/connectors marketplace show` and `/connectors marketplace claude list|add|remove`
manage the `connectors.stores.marketplaces` array in the user-global config
(atomic splice, duplicate name refused without `--force`) (FR-042..FR-044).

### 19C.8 Configuration

The `connectors` block in `ragent.json` (§5.2) controls the master switch
(`connectors.enabled`, default `true`), the store directory override, the
catalogue endpoints and shared fetch budgets (`timeout_ms`, `max_index_bytes`,
`cache_ttl_secs`), and the non-secret credential-name mapping. The section
merges overlay-wins (project over user-global) (FR-007, FR-021, FR-024). With
`connectors.enabled: false` the subsystem performs no discovery, no catalogue
fetch, and no connection, and every subcommand other than `help` reports that it
is disabled (FR-021).

### 19C.9 CLI parity

The same operations are available from a shell via the `ragent connectors <sub>`
CLI parity surface (`src/connectors.rs`), mirroring `src/plugins.rs`: the
session-free subcommands, the management subcommands (over an ephemeral
lifecycle session), and the `test` harness. The report is re-spelled for the CLI
surface (`From: /connectors X` -> `ragent connectors X`).

---

## 19D. Execution Backends (`local` / `docker` / `podman` / `remote`)

A pluggable execution backend selects **where** a session's tool invocations run
(spec `openhands` FR-001, FR-019, FR-020, FR-026, FR-031, FR-035). The backend
dispatch leaf is `crates/ragent-agent/src/backend/`; the agent loop, LLM
providers, tool registry, permission system, and event bus are unchanged by the
choice - only the tool-execution leaf routes through the `ExecutionBackend`
trait.

### 19D.1 Kinds and defaults

`ExecutionBackendKind` covers `local`, `docker`, `podman`, and `remote`. With no
`execution_backend` key configured (or an unknown value) the effective kind
resolves to `local`, the host adapter that runs the tool exactly as before
(FR-019). `local` is a thin pass-through, so selecting it is behaviour-preserving.

### 19D.2 Container backend (T-002)

`ContainerBackend` provisions one long-lived sandbox per session and dispatches
each tool call into it with `<runtime> exec`:

```text
<podman|docker> run -d --name ragent-sandbox-<hash> \
        -v ragent-sandbox-vol-<hash>:/projects -w /projects \
        <image> sleep infinity
```

- **Runtime detection (FR-026).** `crates/ragent-agent/src/backend/detection.rs`
  probes `PATH` for `podman`/`docker` and never spawns a process. `podman` is the
  default container runtime when none is specified; a container backend is only
  offered when a runtime is present.
- **Workspace isolation (FR-020, FR-035).** The workspace is a **container-named
  volume, never a host bind mount**, so a sandboxed tool cannot read or write the
  host working directory. The descriptor's `workspace` field names the
  container-side mount point when it is an absolute path (default `/projects`).
- **Dispatch.** `Bash` is validated by the host 7-layer validator before it runs
  (FR-002, FR-037); a file/search subset (`read`, `Write`, `create`,
  `append_to_file`, `rm`, `make_directory`, `list`, `Glob`, `Grep`) is translated
  to confined sandbox `exec` scripts; every other tool is refused with no host fallback
  (FR-031).
- **Fail without fallback (FR-031).** A provisioning failure (missing runtime,
  missing or unpullable image) fails the turn with a `Provision` error and never
  runs the tool on the host.

The runtime is selected from `Config::effective_backend_config()`, which resolves
both the bare-label spelling and an inline descriptor (image and workspace). The
processor caches one `ContainerBackend` per `(kind, id, image, workspace, working
dir)` so a session's container is provisioned once and reused (FR-009).

### 19D.3 Remote backend (T-003)

`RemoteBackend` drives a **second ragent server** over its REST+SSE API instead of
running tools locally (FR-021). The transport, stream decoder, and remote-session
cache live in `crates/ragent-agent/src/backend/remote.rs`; the session-level bridge
that mirrors the remote stream onto the local bus lives in
`crates/ragent-agent/src/session/remote_dispatch.rs`.

- **Turn flow.** The processor opens (or reuses) a remote session at
  `POST <url>/sessions` with the local working directory, posts the prompt to
  `POST <url>/sessions/{id}/messages`, and consumes the server's
  `text/event-stream`. Each frame is decoded into a `RemoteUpdate` and re-published
  on the **local** event bus under the local session id, so `text_delta`,
  `tool_call_start`/`tool_call_end`, `tool_result`, `agent_notice`,
  `permission_requested`, and `question_requested` render exactly like a local turn
  (FR-021). The assistant placeholder is persisted and updated with the mirrored
  text, so a remote turn is resumable like any other.
- **Auth and descriptor (FR-004, FR-030).** A `remote` descriptor carries `url`
  (base URL) and `api_key`; the key is sent as a bearer token on every request and
  is redacted from diagnostics. `Config::effective_backend_config()` resolves both
  the bare-label spelling and an inline/registry descriptor.
- **Reuse (FR-009).** One `RemoteBackend` is cached per `(id, url, api_key)`, and
  the remote session id is cached per `(backend id, local session id)`, so a later
  turn continues the same remote conversation over one HTTP connection pool.
- **Fail without fallback (FR-031, FR-034).** A remote backend with no `url`, or a
  server that cannot be reached before the turn, fails the turn with a `Provision`
  error. A stream that drops **part-way through** a turn is reported as a `Protocol`
  error, surfaced as an `AgentError`, and never re-runs the turn's tools locally. The
  relay is bounded by a turn timeout and polls the cancellation flag, so an
  unresponsive server cannot hang the session. `RemoteBackend::execute_tool` refuses
  every local tool dispatch, so a leaked dispatch cannot execute on the host either.

### 19D.4 Sandbox secret injection (T-006; FR-010, FR-023)

A `docker`/`podman`/`remote` descriptor may list `credentials` - non-secret
*names* stored in the encrypted credential store (`crates/ragent-storage`, the
same store `/auth` writes to). The value is resolved **at spawn time** and never
carried in `ragent.json`:

- **Container (FR-010).** Each named value is resolved when the sandbox is
  provisioned and injected as a container environment variable
  (`<runtime> run ... -e NAME=VALUE ...`), never written into the image or the
  workspace. The value is registered with the shared redaction registry, so a
  later rendering of tool output masks it (FR-035). A name the store does not
  hold fails the provision with a `Provision` error - the sandbox is never
  started without a required credential.
- **Remote (FR-010).** A remote descriptor that names a credential (and has no
  literal `api_key`) resolves its bearer key from the same store at turn start; a
  missing credential fails the turn rather than relaying it unauthenticated. A
  literal `api_key` still works and takes precedence, so existing configs are
  unchanged.
- **Workspace persistence (FR-023).** The sandbox workspace is a named volume
  derived from the *project* identity `(runtime, id, image, workspace, host
  working dir)`, so it survives replacement of the container: recreating the
  container always re-mounts the same volume. The *container* name additionally
  folds in the descriptor's credential names, so changing the credential set
  provisions a fresh container environment while preserving the workspace.

The credential store enters the backend through the `SecretResolver` trait
(`crates/ragent-agent/src/backend/secrets.rs`): the production
`StorageSecretResolver` reads the encrypted SQLite store, and `MapSecretResolver`
is the in-memory test seam. `resolve_backend_with_secrets` wires the store into
the container and remote adapters; a backend built without a store fails a
spawn that names a credential rather than starting without it.

### 19D.5 Durable backend registry (T-004; FR-004, FR-030)

`crates/ragent-agent/src/backend/registry.rs` exposes every configured execution
backend as a durable registry entry. The **config file is the durable record**;
the registry is its resolved read model, rebuilt from [`Config`] on demand, so no
separate store can drift from the configured backends. Each
`BackendRegistryEntry` carries:

| Field      | Meaning                                                                 |
| ---------- | ----------------------------------------------------------------------- |
| `id`       | A stable id from the descriptor's `id` (or the kind label when empty).  |
| `name`     | The display name (`name`, defaulting to `id`).                          |
| `kind`     | `local`, `docker`, `podman`, or `remote` (`ExecutionBackendKind`).      |
| `connection` | A secret-free `ConnectionDescriptor` (see below).                     |
| `health`   | A `HealthState` (`ok`, `unavailable`, `unknown`) plus a `detail`.       |

- **`ConnectionDescriptor`** is secret-free (FR-035): `Local`, `Container
  { runtime, image, workspace, container, volume, credential_names }`, or
  `Remote { url, has_api_key, credential_names }`. It records key *presence* and
  credential *names* only; `summary()` is safe to render in any log or panel.
- **Construction.** `BackendRegistry::from_config(config, host_working_dir)` always
  registers the built-in `local` entry first (FR-019), then each entry in
  `Config::backends`, then the active backend (an inline `execution_backend`
  descriptor or the registered entry it names), and finally a synthesized entry for
  a bare kind label with no matching descriptor, so the active backend is always
  visible. Duplicate ids are collapsed.
- **Health (FR-004).** `local` is always `ok`. A `docker`/`podman` entry is `ok`
  when its runtime binary resolves on `PATH` (FR-026) and `unavailable` with a
  detail naming the missing binary otherwise. A `remote` entry is `ok` when a base
  URL and a key are configured (FR-030), `unavailable` when either is absent, and
  refined by `refresh_health` / `probe_remote`, which probe the server's **public**
  `/health` endpoint (no bearer key materialised or sent). `refresh_health`
  re-probes every entry.
- **Remote registration (FR-030).** `BackendRegistry::register_remote` adds a
  `remote` backend by URL and key, replacing any entry with the same id in place.
- **Active resolution.** `BackendRegistry::active_id(config)` returns the id of the
  backend the config resolves to, preferring the active descriptor's id, then the
  first entry whose kind matches the effective kind (FR-019).

The registry is consumed by the TUI backend surface (T-005, FR-008) to show the
active backend and the health of every registered backend.

The TUI surface is the `/backend` family (`crates/ragent-tui/src/app/backend_panel.rs`,
`App::handle_backend_command`): with no argument (or `show`/`list`) it opens an
interactive switcher panel that renders the active backend and a health row for
every registered backend, and the status bar's second line opens with
`Backend:<name>` in cyan naming the active backend. Moving the
cursor and pressing `Enter` switches the active backend (persisting
`execution_backend`), after which its health is re-reported. `/backend <kind|id>`
switches directly; `/backend help` renders the usage table. A container backend is
offered only when its runtime resolves on `PATH` (FR-026), and the actual tool
dispatch still resolves the backend from the live config, so the panel is a view
over the registry rather than a second source of truth.

### 19D.8 LLM security analyzer (T-015; FR-006, FR-016, FR-017)

The security analyzer is an opt-in permission mode (`security_analyzer.enabled`,
or the TUI `/security on` / `security-analyzer` alias, which persists the flag).
Instead of deciding a proposed tool action from the static rules alone, ragent
asks a model for an `allow` / `ask` / `deny` verdict with a rationale, published
as `Event::SecurityVerdict` *before* the action is permitted or refused (FR-017)
so the user sees the reasoning.

- **Tightening-only (FR-006).** `crates/ragent-agent/src/security/mod.rs` builds a
  no-tools analyzer request and parses the verdict (`parse_verdict`); the decision
  is applied at the permission gate (`crates/ragent-agent/src/session/permissions.rs`):
  an analyzer `deny` is a hard denial reported to the model, an analyzer `allow` can
  satisfy a bare `Ask` (no explicit policy rule) but never overrides an explicit
  policy `Deny` or a forced destructive-action checkpoint, and an analyzer `ask` -
  or any failure (missing model, provider error, unparseable output, timeout) -
  degrades to the normal interactive prompt via `SecurityVerdict::fail_safe`, so a
  broken analyzer can never silently broaden access.
- **Model (FR-006).** `security_analyzer.model` routes the analyzer to a
  fast/cheap model; `security_analyzer.timeout_secs` (default 20) bounds one call.
- **Hook exit codes (FR-016).** `PreToolUse` hook semantics are independent and
  unchanged: exit code 2 blocks and returns the hook's stderr as the tool result,
  exit code 1 warns and continues.

```jsonc
{
  "security_analyzer": {
    "enabled": true,
    "model": { "provider_id": "ollama", "model_id": "qwen2.5:1.5b" },
    "timeout_secs": 20
  }
}
```

### 19D.6 Configuration

The `execution_backend` key accepts a bare label or an inline descriptor, and the
`backends` array declares the registry entries the switcher (T-005) and the
container backend read. See §5.2 / `docs/howtos/config.md` §7.36.

### 19D.7 Hardening and secret-safety invariants (T-018; FR-002, FR-024, FR-032, FR-035, FR-037, FR-038)

The new execution/interop surfaces add no security layer and weaken none; the
invariants below hold across every one of them and are pinned by hermetic tests
that fail on any host, with or without a container runtime.

- **The security decision is upstream of the backend (FR-002, FR-037).** The
  permission check, the file-path containment guard, the always-allowed
  codeindex hardwiring, and the 7-layer bash validator all run in the session
  tool-dispatch path (`crates/ragent-agent/src/session/`) *before* the leaf is
  handed to an `ExecutionBackend`. The trait carries no permission or approval
  token (`BackendToolCall { tool, input, ctx }`), so a backend can only run an
  already-approved call - selection changes *where* a tool runs, never *whether*
  it is allowed (assumption A1).
- **Sandbox bash is gated by the canonical host validator (FR-002, FR-037).**
  `ContainerBackend::execute_tool` calls
  `ragent_tools_core::bash::validate_shell_command` on every `Bash` call before
  it provisions or touches a container, so a banned tool, a denied command name,
  a denied pattern, a directory escape, or an obfuscated command is rejected with
  the tool never executing on the host. The refusal precedes provisioning, so it
  is identical with or without a runtime present. Covered by
  `crates/ragent-agent/tests/test_hardening_invariants.rs`.
- **A sandbox cannot reach the host working directory (FR-035).** The workspace
  is a container-managed named volume mounted at `/projects`, never a host bind
  mount; the host working directory is only an input to the sandbox *name hash*,
  never a tool working directory or mount source
  (`test_hardening_invariants.rs::sandbox_workspace_is_the_container_mount_not_the_host_working_directory`).
  A sandbox file tool whose path escapes the workspace is rejected before any
  container is touched
  (`test_container_backend.rs::container_file_tools_reject_paths_outside_the_workspace`).
- **No secret reaches a rendered surface (FR-035).** A resolved credential value
  is registered with the shared redaction registry (`ragent-types::sanitize`) at
  resolution time; `RemoteBackend`'s hand-written `Debug` renders the bearer key
  as `[REDACTED]`; the secret-free backend `ConnectionDescriptor` records key
  *presence* and credential *names* only; the OpenAPI document declares the
  `bearerAuth` scheme with no value; `GET /config` redacts every credential-keyed
  field by shape. Covered by `test_hardening_invariants.rs`,
  `test_backend_registry.rs`, `test_sandbox_secrets.rs`, `test_openapi_document.rs`,
  and `ragent-server`'s `test_config_redaction.rs`.
- **The OpenAI-compatible surface inherits the native auth and never bypasses
  permissions (FR-024, FR-038).** `openai_routes()` is merged into the
  auth-protected router, so `/v1/models` and `/v1/chat/completions` require the
  same bearer token as the native API; the surface never sets `auto_approve`, so
  a tool that needs confirmation raises the ordinary prompt
  (`test_openai_surface.rs`).
- **Refuse to serve unauthenticated (FR-032).** When `openai.enabled` is true and
  no token resolves from `openai.token` or the ambient `RAGENT_TOKEN`, `ragent
  serve` refuses to start the surface with a non-zero exit and an actionable
  message rather than serving it unauthenticated (`src/main.rs`, the `Serve` arm).
  Covered by `tests/test_serve_openai_gate.rs`.

## 19E. Agent Client Protocol (`ragent-agent/src/acp/`)

ragent speaks ACP (Agent Client Protocol) in both directions (spec `openhands`
FR-015, FR-022, FR-028, FR-036).

### 19E.1 ACP client (T-007; FR-015, FR-022, FR-036)

`crates/ragent-agent/src/acp/mod.rs` drives an external coding agent (Claude
Code, Codex, Gemini CLI, or a custom ACP server) as a **subprocess** that speaks
JSON-RPC 2.0 over its stdin/stdout. `AcpClient::spawn` starts the command (the
child is `kill_on_drop`), `initialize` and `session/new` perform the handshake,
and `session/prompt` relays one turn, rendering each streamed `session/update`
through a callback. A turn dispatched to an ACP agent is relayed to it instead of
a local ragent LLM provider (FR-022); the bridge from `AcpUpdate`s onto the local
event bus lives in `crates/ragent-agent/src/session/acp_dispatch.rs`.

Failure handling is first-class (FR-036): a non-zero exit, a malformed stdout
frame, a JSON-RPC error reply, or a silent subprocess that outlives its turn
budget all fail the turn - the relay never blocks indefinitely. Configuration is
the `acp.agents` registry plus an optional `acp.default_agent`.

### 19E.2 ACP server on stdio (T-008; FR-022, FR-028)

`crates/ragent-agent/src/acp/server.rs` is the mirror image: it **serves** an
ACP-capable editor (Zed, VS Code, JetBrains) over JSON-RPC on ragent's own
stdin/stdout. Each editor session maps onto a local ragent session, and the
session's agent-loop events are streamed back as ACP `session/update`
notifications (FR-022):

| Request            | Behaviour                                                                                    |
| ------------------ | -------------------------------------------------------------------------------------------- |
| `initialize`       | Negotiates the ACP protocol version (`1`).                                                     |
| `session/new`      | Opens a local ragent session rooted at the editor's `cwd` (or the process working dir).        |
| `session/prompt`   | Runs one ragent turn, streaming `agent_message_chunk` / `agent_thought_chunk` / `tool_call` / `tool_call_update` updates, then replies with the mapped `stopReason`. |
| `session/cancel`   | Raises the turn's cancellation flag.                                                           |

**Feature-gated and disabled by default (FR-028).** The endpoint compiles only
behind the `acp-server` Cargo feature and runs only when `acp.server_enabled` is
`true` in the configuration. It is reachable only through the `ragent acp-server`
subcommand; when either gate is closed the command prints a diagnostic and exits
with a usage error (code 2) rather than serving. A malformed request frame is
skipped (never tears the connection down) and an unknown request method gets a
JSON-RPC "method not found". `acp.server_agent` selects the driven agent,
resolving `server_agent` -> `default_agent` -> the top-level `defaultAgent`.

```bash
cargo build --features acp-server
ragent acp-server
```

---

## 19F. Automation Service (`ragent-agent/src/automation/`)

The automation service turns a named, schedulable or webhook-triggered agent run
into a durable, backend-confined unit with run history (spec `openhands` T-016;
FR-013, FR-014, FR-018, FR-033). It builds on the existing agent loop and the
cron schedule grammar rather than replacing either (assumption A6).

### 19F.1 Definitions and triggers

An automation is declared under the `automation` block in `ragent.json`
(`crates/ragent-config/src/automation.rs`, `AutomationConfig`). The service is
inert unless the block sets `enabled: true` (`Config::automation_enabled`), so a
project that never opted in starts no scheduler and accepts no webhook. Each
`AutomationDefinition` carries an id, an agent, a prompt template (with
`{{payload}}` substituted at run time), a trigger, a backend, and optional
dispatch targets:

| Trigger | Fires from |
| ------- | ---------- |
| `{ "kind": "webhook" }` | An inbound `POST /auto/<id>` (FR-013) |
| `{ "kind": "schedule", "schedule": "every 2m" }` | The scheduler's due-time tick (FR-014) |

### 19F.2 Webhook ingress, scheduler, and run history

- **Webhook ingress (FR-013).** `crates/ragent-server/src/routes/automation.rs`
  registers `POST /auto/{id}` as a **public** route (like `/health`), because an
  external system such as a GitHub webhook has no bearer token. The request body
  is compacted to JSON and supplied to the run as prompt context.
- **Scheduler (FR-014).** `AutomationService::spawn_scheduler` ticks every
  `scheduler_tick_secs`, enqueues a run when a schedule automation's due time
  passes (skipping one whose previous run is still in flight), and advances its
  next-due time with `CronSchedule::advance_next_due`. The TUI (`ragent-tui`)
  and `ragent serve` each start one scheduler when the config enables the
  service.
- **Run history (FR-018).** `ragent-storage`'s `automation_runs` table records
  one row per run (`Storage::insert_automation_run` / `finish_automation_run` /
  `list_automation_runs` / `prune_automation_runs`). A terminal run captures the
  automation id, trigger kind, start/end times, outcome, selected backend, and
  an output reference; history is pruned to `run_history_cap`.

### 19F.3 Backend confinement (FR-033)

A run executes only in its configured execution backend. `AutomationService`
installs the selected descriptor as a per-session override
(`register_session_backend`) before the run's first turn; the session loop
applies it (`apply_session_backend_override` in
`crates/ragent-agent/src/session/loop_steps.rs` and
`remote_dispatch.rs`), so a container/remote run is routed there and a relay
failure never silently re-runs it on the host (FR-031). No part of the run
executes outside the selected backend.

### 19F.4 Third-party dispatch (FR-013)

`crates/ragent-agent/src/automation/dispatch.rs` builds a provider-shaped payload
(`payload_for`) for each `DispatchTarget` and POSTs it on run completion. The
supported kinds are `slack`, `github`, `linear`, `notion`, and `webhook` (a
generic JSON POST). A target names its credential's environment variable
(`token_env`); the value is read at dispatch time and never stored or logged
(FR-035). A target with no `url` is inert.

### 19F.5 Surfaces

| Surface | Command / path |
| ------- | -------------- |
| TUI | `/automation [list]`, `/automation runs <id>`, `/automation run <id>`, `/automation help` |
| CLI | `ragent automation list\|runs <id>\|run <id>\|help` |
| HTTP | `POST /auto/{id}` (public), `GET /automation`, `GET /automation/runs/{id}`, `POST /automation/{id}/run` (bearer) |
| OpenAPI | The three admin routes and the webhook route are in the route table (`crates/ragent-server/src/routes/openapi.rs`) |

See `docs/howtos/slashcommands/automation.md` for the full manual.

---

# Part VII: Operations & Reference

---

## 20. Auto-Update Mechanism

An automatic update check is on the roadmap. The TUI already reserves the `/update`
and `/update install` slash commands, but they currently return a "not implemented"
message. When completed, this subsystem will:

- Query a GitHub releases endpoint for the latest version tag.
- Compare against the compile-time version baked into the binary.
- Offer a one-command download-and-replace flow for supported platforms.

Until then, users should update via `cargo install` or by downloading the latest
release binary manually.

---

## 21. Harness Enhancements

This section captures cross-cutting harness-layer behaviours that the
implementation exposes for operators and CI/CD pipelines.

### 21.1 Skill Catalog (`SkillCatalog`)

The agent does not load full skill bodies into the system prompt at session
start. Instead, `SkillRegistry::load()` first discovers skill metadata and
produces a compact `SkillCatalogEntry` for each skill via
`SkillRegistry::catalog()`. The catalog contains only the skill name,
description, trigger phrase, scope, and invocation flags; it is cheap to build
and keeps the startup context small. Full skill bodies (prompts, context
files, and tool lists) are loaded lazily when a skill is invoked.

### 21.2 Per-Run Cost Summary (`RunCostSummary`)

At the end of each `process_user_message` turn, the agent accumulates token
usage records, applies the built-in (or configured) per-model price table, and
publishes `Event::RunCostSummary` on the event bus. Consumers such as the TUI
and the HTTP SSE stream use this event to surface per-run spend without having
to accumulate per-request `TokenUsage` events themselves. The summary includes
input tokens, output tokens, estimated USD cost, model identifier, and
wall-clock duration.

### 21.3 Dry-Run Readiness Report

The `ragent --dry-run` / `ragent config check` flow builds a
`ReadinessReport` by loading and merging configuration, resolving provider/model
auth state, discovering skills via the skill catalog, enumerating visible tools,
and performing lightweight MCP connectivity checks — all without invoking the
LLM or executing any tool. The report is rendered as human-readable text by
default and as JSON when `--json` is supplied. See `QUICKSTART.md` for usage
examples.

### 21.4 Test Layout: One Test Binary per Crate

`cargo`'s default autotest discovery turns each top-level `tests/*.rs` file into
its own binary. At this workspace's scale that was ~948 integration binaries and
~948 link steps per full `cargo test`. Every crate (and the root `ragent`
package) therefore sets `autotests = false` and declares exactly one integration
target:

```toml
[package]
autotests = false

[[test]]
name = "all"
path = "tests/all.rs"
```

```rust
// crates/<crate>/tests/all.rs
#[path = "test_agent_memory_deep.rs"] mod test_agent_memory_deep;
#[path = "test_automation_dispatch.rs"] mod test_automation_dispatch;
// ... one `#[path]` line per former top-level test file
```

`#[path]` inclusion is what makes this a pure packaging change: it anchors
module resolution at the child file, so plain `mod support;`, nested
`#[path = "../src/<module>.rs"]` source re-imports, `include_str!("support/..")`
and inner attributes (`#![forbid(...)]`, `#![cfg(feature = ...)]`) all keep
working. Test names keep a `test_<file>::<fn>` shape, so one file is run with
the filter form `cargo test -p <crate> --test all test_foo::`. The workspace
compiles **one integration test target per crate plus one for the root package**
(`ls crates/*/tests/all.rs tests/all.rs`) instead of one per test file, and
[`docs/BUILDSTATS.md`](docs/BUILDSTATS.md) records the resulting test cost and the re-measured line
coverage.

Three consequences are part of the contract:

- **Merging binaries merges process state.** Each `tests/all.rs` defines a
  binary-wide, poison-tolerant `global_env_mutex()` / `global_env_lock()`, and
  every test that mutates or observes a process-global (cwd,
  `XDG_CONFIG_HOME`, `HOME`, `PATH`, a `static` registry) contends on it.
  `#[serial]` remains acceptable for globals that need no value control.
- **`crate::` in a `#[path]`-included source module resolves at the binary
  root**, not at the crate under test. Where a child includes `src/foo.rs` that
  refers to `crate::bar`, `tests/all.rs` re-exports `bar` (the shims at the top
  of `crates/ragent-agent/tests/all.rs` are the pattern) rather than duplicating
  the module.
- **Tests must not write into the repository.** Three write paths were made
  injectable so a test run leaves the working tree byte-identical: the research
  vault root is explicit `AppState::research_root` state (production passes
  `<working dir>/research`, tests pass `target/temp/ragent-server-research/`),
  `session::history::set_subagent_log_root` redirects diverted `wait_agents`
  batches, and `ragent_tui::app::set_log_root_override` /
  `use_scratch_log_root_for_tests` redirect the TUI log-window spools. All three
  are `#[doc(hidden)]` `OnceLock` setters — production never calls them, so the
  shipped defaults (`<working dir>/log`, `<working dir>/research`) are unchanged.

The review, the measurement method and the per-crate rollout record live in
`docs/reports/testconsolidation-review.md`.

---

# Appendices

---

## Appendix B: Documentation

All documentation markdown files are located in `docs/` except for these root files:

| File | Purpose |
|------|---------|
| `README.md` | Project overview |
| `QUICKSTART.md` | Quick start guide |
| `SPEC.md` | This specification |
| `AGENTS.md` | Agent guidelines |
| `CHANGELOG.md` | Change log |
| `STATS.md` | Project statistics |

---

## Appendix C: Project Contact & Repository

- **Repository:** https://github.com/thawkins/ragent
- **License:** MIT
- **Author:** Tim Hawkins

---

## Research Configuration

The `/research create` synthesis prompt (in `crates/ragent-research/src/analysis.rs`)
honours several optional `ragent.json` keys under a top-level `research` object.
All are opt-in and default to the legacy behaviour, so existing
configurations continue to work unchanged.

The synthesis prompt builder also has few-shot exemplar and persona knobs
(`SynthesisPromptConfig.few_shot_examples`, `SynthesisPromptConfig.persona`),
but these are not `ragent.json` keys: `ResearchConfig` has no `few_shot` or
`analysis_persona` field, so such keys are ignored.

### `research.models.*` (string, optional)

Per-phase model overrides. **Parsed but not yet applied:** the run builder
(`build_session_config` in `ragent-research` `run_request.rs`) copies only the
per-run request fields (`--research-model`, `--compression-model`,
`--final-report-model`), so these config keys currently have no effect.

- `research_model` — model used by research agents / sub-topic workers.
- `compression_model` — model used to compress or summarise intermediate findings.
- `final_report_model` — model used to write the final report.

There is no `summarization_model` config key; the per-page summarization model
is set per run with `--summarization-model` on the TUI `/research create`
command (FR-002).

### `research.supervisor.max_concurrent_research_units` (integer, default 5)

Maximum number of researcher agents that may run concurrently in `supervisor`
and `competitive` modes (FR-012). **Parsed but not yet applied:** runs use the
per-run `--max-concurrent-research-units` value or the built-in default.

### `research.evaluate.enabled` (boolean, default `false`)

Intended to append a deterministic quality scorecard to every run's
`RESEARCH.md` (FR-015). **Parsed but not yet applied:** the run builder reads
only the per-run `evaluate` field, so pass `--evaluate` on the TUI
`/research create` command (the `ragent research create` CLI has no
`--evaluate` flag).

### Example stanza

```jsonc
{
  "research": {
    "models": {
      "compression_model": "ollama:phi4",
      "research_model": "openrouter/anthropic/claude-sonnet-4-20250514",
      "final_report_model": "openrouter/anthropic/claude-sonnet-4-20250514"
    },
    "supervisor": {
      "max_concurrent_research_units": 3
    },
    "evaluate": {
      "enabled": true
    }
  }
}
```

### Implementation status

The prompt-builder knobs (`SynthesisPromptConfig.few_shot_examples`,
`SynthesisPromptConfig.persona`) and the `LlmAnalysisEngine::with_persona`
builder are implemented in `crates/ragent-research/src/analysis.rs`. Model
overrides, supervisor concurrency, and evaluation are propagated from
`ResearchRunRequest` through `build_session_config` in
`crates/ragent-research/src/run_request.rs` to the engine phases. The
`ragent.json` `research.*` keys are read via `Config::merge` in
`crates/ragent-config/src/config.rs` and propagated through
`build_session_config` so CLI/TUI defaults match the config file. Callers can
still override per run with the corresponding flags.
