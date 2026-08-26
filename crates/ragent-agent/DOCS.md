# ragent-agent

The central orchestration and runtime layer for ragent. Owns sessions, the
agent loop, tool registry, MCP client, memory system, teams/swarms, background
agents, compaction, hooks, permissions, and more. Wires together the config,
storage, LLM, tools, and types crates behind the agent runtime.

## Workspace Dependencies

- ragent-codeindex
- ragent-config
- ragent-llm
- ragent-research
- ragent-storage
- ragent-tools-core
- ragent-tools-extended
- ragent-tools-vcs
- ragent-types
- ragent-specs
- ragent-telemetry

## External Dependencies

- tokio, serde, serde_json, anyhow, thiserror, tracing
- rmcp (MCP client SDK)
- dashmap, lru, rayon, parking_lot
- chrono, uuid, regex, once_cell
- futures, async-trait, async-stream

Optional: `embeddings` feature for local ONNX semantic search.
Benchmarks: criterion (`agent_loop`).

## Public API (crate root)

`src/lib.rs` declares 30+ public modules and re-exports config, LLM, VCS, and
types for ergonomic access (`ragent_agent::Config`, `ragent_agent::Provider`,
etc.).

### Modules

- **agent** — Agent definitions, built-in roster, system-prompt building, OASF custom agent loading.
- **background** — Background shell task service (`bg` tool), wake/notify hooks.
- **bang_command** — `!`-prefixed shell command helpers shared by CLI + TUI.
- **compaction** — Context-window summarisation (estimator, prompt, runner, serializer, convert).
- **cost** — Per-run cost estimation from token usage + price table.
- **dry_run** — `ragent --dry-run` readiness report (no LLM/tool invocation).
- **error** — `RagentError` thiserror enum.
- **event** — Re-export of `ragent_types::event::{Event, EventBus, FinishReason}`.
- **file_ops** — Concurrent file reader, staged edit batching.
- **goal** — Goal-based autonomous stop hook (LLM evaluator).
- **hooks** — Lifecycle hooks (on_session_start, pre/post_tool_use, etc.).
- **id** — Newtype IDs: `SessionId`, `MessageId`, `ProviderId`, `ToolCallId`.
- **loop_state** — Stateful cron mode: `<loop-state>`/`<inbox>` tag parsing + persistence.
- **mcp** — MCP client (stdio + HTTP), server discovery, tool bridging.
- **memory** — Structured memory, extraction engine, knowledge graph, embedding search, visualisation.
- **message** — Re-export of `ragent_types::message`.
- **orchestrator** — Multi-agent orchestration: registry, router, coordinator, leader election, conflict resolution.
- **perf** — Agent-loop profiling flag (env/config/runtime override).
- **permission** — Re-export of `ragent_config::permission` + `ragent_types::permission`.
- **reference** — `@`-syntax file/dir/URL reference parsing + fuzzy matching.
- **research_adapter** — Wires agent tool registry into research system gatherers.
- **sanitize** — Secret redaction (regex + exact-match registry).
- **session** — Session lifecycle, agent loop, history truncation, profiling, streaming.
- **skill** — Skill discovery, loading, argument substitution, invocation.
- **snapshot** — File-level snapshot + incremental delta (undo support).
- **storage** — Re-export of `ragent_storage`.
- **task** — Sub-agent `AgentManager` (sync/background spawn, cancel, suspend, resume, kill).
- **team** — Team coordination runtime + 20 team tools.
- **telemetry** — Re-export of `ragent_telemetry`.
- **template** — Prompt template discovery/loading/substitution.
- **tool** — `Tool` trait, `ToolRegistry`, adapter layers, all built-in tool registrations.
- **trigger** — Trigger deduplication, cycle suppression, dynamic rules, MCP notification adapter.
- **updater** — GitHub release check + self-update.
- **yolo** — Global YOLO mode flag (bypass safety checks).

### Re-exported items at crate root

- **Config** (re-export from `ragent_config`) — Top-level ragent configuration.
- **Provider** (trait, re-export from `ragent_llm`) — LLM provider trait.
- **ProviderRegistry** (re-export from `ragent_llm`) — Provider registry.
- **Storage** (re-export from `ragent_storage`) — SQLite storage facade.
- **Event** / **EventBus** (re-export from `ragent_types`) — Event system.
- **create_default_registry** (fn) — Builds the full ~169-tool registry.

### Key types

- **AgentManager** (struct, `task` module) — Sub-agent manager; methods: `spawn_sync`, `spawn_background`, `cancel_agent`, `suspend_task`, `resume_task`, `kill_task`, `drain_completed`, `list_tasks`, `wait_for_agents`.
- **SessionManager** (struct, `session` module) — Session lifecycle manager.
- **SessionProcessor** (trait, `session` module) — The agent loop processor; `process_message` drives LLM call -> tool execution -> repeat.
- **Tool** (trait, `tool` module) — `name()`, `description()`, `parameters_schema()`, `permission_category()`, `execute()`.
- **ToolContext** (struct, `tool` module) — Execution context (session_id, working_dir, config, event_bus, storage, agent_manager, team interface).
- **ToolOutput** (struct, `tool` module) — `{ content, metadata }` returned by tools.
- **ToolRegistry** (struct, `tool` module) — Thread-safe tool registry.
- **TeamManager** (struct, `team` module) — Team coordination runtime.
- **McpClient** (struct, `mcp` module) — MCP stdio/HTTP client.
- **MemoryStore** (struct, `memory` module) — Structured SQLite memory store.
- **SnapshotManager** (struct, `snapshot` module) — File-level snapshot + undo.
- **CompactionRunner** (struct, `compaction` module) — Context-window summarisation.
- **HookEngine** (struct, `hooks` module) — Lifecycle hook execution.