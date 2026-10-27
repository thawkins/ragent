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

- **acp** — Agent Client Protocol (spec `openhands`): the ACP **client** (`AcpClient`, JSON-RPC on a subprocess's stdio; `relay_turn`) that drives external agents, and the feature-gated ACP **server** (`server::serve_stdio`, behind the `acp-server` feature) that serves ACP-capable editors over ragent's own stdio (FR-015, FR-022, FR-028, FR-036).
- **agent** — Agent definitions, built-in roster, system-prompt building, OASF custom agent loading.
- **backend** — Execution backends (spec `openhands`): the `ExecutionBackend` trait, the `local` host adapter, the `docker`/`podman` `ContainerBackend` (sandbox provisioning, workspace volume mount, container tool dispatch), container-runtime detection with `podman` as the default, and the `remote` `RemoteBackend` (drive a second ragent server over REST+SSE and mirror its event stream locally).
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
- **session** — Session lifecycle, agent loop, history truncation, profiling, streaming. Data-modules rather than drivers: `pending` (the FR-013 `PendingVerdicts` tracker: per session, the call ids currently at a permission prompt, marked by the dispatch pipeline's permission stage before it prompts and settled when the verdict resolves, read by the tool phase's append path through `result_is_appendable` so no tool result is appended for a call whose verdict has not resolved), `tool_input_error` (the FR-008/FR-009 `ToolInputError` value that owns a pre-execution refusal's sentence, record status and therefore its `is_error` flag - both loop gates build it, so a decode failure and a schema violation cannot be worded or recorded differently) and `dispatch` (the FR-012 ordered pipeline: `ToolDispatchContext` owns the turn's borrows so it can be moved into the spawned task, `ToolDispatchPipeline::run` is the one place the stages exist and `denial`/`refusal`/`synthesised` are its three terminal-outcome constructors, `resolve_call`/`resolve_tool_call` run stages 1-3 once per call, and `suggest_tool_name` re-exports `tool_suggestion`'s one FR-021 ranking. Both execution arms call it: the step is a list of slots in the provider's order - a future for a call that runs, a settled outcome for a refused one - drained in one ordered pass after every call is prepared, so the two modes cannot record a call out of order).
- **skill** — Skill discovery, loading, argument substitution, invocation.
- **snapshot** — File-level snapshot + incremental delta (undo support).
- **storage** — Re-export of `ragent_storage`.
- **task** — Sub-agent `AgentManager` (sync/background spawn, cancel, suspend, resume, kill).
- **team** — Team coordination runtime + 20 team tools.
- **telemetry** — Re-export of `ragent_telemetry`.
- **template** — Prompt template discovery/loading/substitution.
- **tool** — `Tool` trait, `ToolRegistry`, adapter layers, all built-in tool registrations, plus three data-modules rather than tools: `alias_table` (the FR-015 `ProviderAlias` table, `AliasTable`, the one inbound resolver `resolve_inbound_tool_name` applied in `process_message` before the unknown-tool check, and the one outbound projection `AliasTable::outbound_definitions` that `tools.expose_alias_names` turns on), `rename` (the FR-023 `(before, after)` table, re-exported from `ragent_types::tool_rename`), and `allowed_tools` (the FR-029 exact-name resolution of a skill's `allowed-tools` entries plus the once-per-`(declarer, entry)` report for an entry that names no registered tool; nothing there consults `rename`, deliberately). `mcp_tool` also carries the one MCP bridge, `bridge_server_tools`: it splits a server's advertised tools into wrappers and FR-032 refusals (`ragent_types::tool_bridge::check_dynamic_bridge`), reports the refusals once, and is shared by `SessionProcessor::set_mcp_client` and the TUI's `register_mcp_tools`, so a refused tool is structurally absent from `wrappers` instead of being filtered by each caller. An alias table entry is a spelling resolved on the way in; it is never a registry entry, so it is not counted or dispatched as a tool, and it reaches an outbound tool list only through `outbound_definitions` when `tools.expose_alias_names` asks for it (FR-030, FR-015).
- **tool** (continued) — the crate's `tests/test_tool_rename_guard.rs` is the guard case for NFR-008 and NFR-009 (spec `uniformtools` T-048): it runs `scripts/check_tool_rename.py` - the ONE implementation the CI guard `scripts/check-tool-rename.sh` also invokes - as a subprocess, reads its `--json` report and asserts the same verdict, the same hits and the same ASCII wording the script prints, seeds each predicate's fixture into a private copy of the tree under `target/temp/` (`--root DIR`) rather than into the checkout, and pins the guard's cost (a full run inside a 10 s budget) and its hermeticity (no socket or workbook import, every reported path inside the root, a copy with no `tools.xlsx` judging identically). `UpdateFileTool::execute` forwards to the canonical `write::WriteTool` by design and says so in its own doc comment, which is the reviewed reason the predicate (g) exemption table records that line.
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
- **SessionProcessor** (trait, `session` module) — The agent loop processor; `process_message` drives LLM call -> tool execution -> repeat. Its per-step tool phase reads the user's `experimental.parallel_tool_calls` and then gates it on the active provider's wire profile (`crate::provider::wire::profile_table::profile_for(..).is_none_or(|p| !p.supports_parallel)`), demoting the step to the sequential arm and logging one `tracing::info!` naming the provider and the reason - FR-010 (spec `uniformtools` T-015); an id with no declared profile is demoted too, and the gate never enables concurrency. That same phase also writes the step's signed reasoning block into the assistant message ahead of its text, so it precedes the `tool_use` blocks of the turn that answers the call and reaches the request built from the in-turn message list rather than only the next turn's rebuilt history - FR-011 (spec `uniformtools` T-018). The reasoning text and its streamed `ReasoningSignature` are kept apart through `LlmStepResult`, saved as one `MessagePart::Reasoning`, and the signature is cleared with the text it signed on a retry; whether the block is replayed or omitted is the wire profile's `echo_reasoning` decision, made only in the encoder.
  The same tool phase refuses to append a tool result for a call whose permission verdict has not resolved: the dispatch pipeline's permission stage marks the call in `session::pending::PendingVerdicts` immediately before it prompts and settles it when the verdict resolves (a timeout is a denial, and a surviving `Ask` is refused as an error), and the append path asks the same tracker through `result_is_appendable`. On an unsettled verdict the call's `MessagePart::ToolCall` is still written - status `Pending`, no output, no error, so the provider's `tool_use` is not orphaned - but no `ContentPart::ToolResult` is appended and one `tracing::warn!` names the call - FR-013 (spec `uniformtools` T-016). The tracker is keyed by session and call, because call ids are not globally unique, and one `settle` resolves a call that raised several prompts (the bash sub-command loop) because the verdict is one verdict.
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