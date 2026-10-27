# ragent-types

Shared cross-crate types for ragent: IDs, events, messages, LLM primitives,
permissions, activity-log schema, cron, thinking, triggers, sanitisation,
and startup instrumentation. This is the lowest-level shared crate and
depends on no other `ragent-*` workspace crates.

## Workspace Dependencies

None. `ragent-types` is a leaf crate consumed by every other ragent crate.

## External Dependencies

- tokio
- serde
- serde_json
- anyhow
- thiserror
- uuid
- tracing
- chrono
- regex
- rusqlite
- once_cell

Dev-dependencies: tokio (full), serial_test.

## Public API (crate root)

### Modules

- **activity** — Activity-log event schema: immutable, self-describing execution facts (model messages, tool calls, results, permissions, checkpoints, terminations).
- **cron** — Cron scheduling types and parsers for the agent cron system.
- **embedding** — Shared embedding serialisation helpers (`Vec<f32>` <-> byte blob for SQLite BLOB storage).
- **error** — Structured error enum (`RagentError`) for all core operations.
- **event** — Event streaming infrastructure: `EventBus` (Tokio broadcast) and `Event` lifecycle enum.
- **html** — HTML tag-stripping helper (`strip_tags`).
- **id** — Typed newtype wrappers for identifiers (`SessionId`, `MessageId`, `ProviderId`, `ToolCallId`, `RunId`, `EventId`).
- **llm** — Provider-agnostic LLM conversation primitives (`ChatRequest`, `ChatMessage`, `StreamEvent`, `ToolDefinition`, etc.).
- **message** — Conversation message types (`Message`, `MessagePart`, `Role`, `ToolCallState`, `ToolCallStatus`, `ImageData`).
- **panic_guard** — Thread-local contained-panic marker cooperating with the global panic hook.
- **permission** — Permission decision types (`PermissionDecision`) referenced by the event system.
- **resource** — Process resource limits: bounded concurrency semaphores for child-process spawning and tool execution.
- **sanitize** — Secret redaction utilities (pattern-based and exact-match registry).
- **startup** — Startup timing instrumentation (`StartupTimings`).
- **strutil** — UTF-8-safe string truncation utilities.
- **thinking** — Provider-agnostic thinking/reasoning configuration types.
- **tool_args** — The one decoder for accumulated tool-call argument text (spec `uniformtools` FR-007): a JSON object, a JSON string that decodes to one, or empty/whitespace as `{}`, with `ToolArgDecodeError` naming the failure for the corrective error of FR-008. `ToolArgDecodeError` is also re-exported at the crate root, because it is the argument type of the refusal constructor (`ragent_agent::session::tool_input_error`).
- **tool_bridge** — The FR-032 rule for dynamically sourced tools: `check_dynamic_bridge(source, prefix, surfaced)` refuses a surfaced name that equals a `before` spelling (returning the pair, so the sentence carries the table's own values) or that is not under the source's `prefix`, and `report_bridge_refusals_once` logs each refusal once per source and name behind a process-wide ledger. Also owns the two manglings every call site now shares (`mcp_tool_name`/`mcp_tool_prefix` folding `-` `.` `/`; `plugin_tool_name`/`plugin_tool_prefix` leaving the plugin id unfolded, because it keys the store entry and the `plugin:<id>` permission category) and the one-wording source labels. It lives here because `ragent-plugins` and `ragent-agent` both ask it and cannot depend on each other. Registration-time only - nothing on a dispatch path calls it (NFR-006).
- **tool_rename** — The FR-023 tool-rename table: `TOOL_RENAMES`, one checked-in `[Rename; 13]` of `(before, after)` pairs ascending by `after`, plus the `Rename` lookups (`after_of`, `before_of`, `is_retired`, `is_rename_spelling`). It sits in this leaf crate so `ragent-config` can refuse a retired `tools.aliases` spelling without depending on `ragent-agent`; `ragent_agent::tool::rename` re-exports it. It is not a translation layer - nothing consults it on a dispatch path (NFR-006).
- **trigger** — Trigger envelope types for the trigger system.

### Re-exported items

- **ACTIVITY_EVENT_SCHEMA_VERSION** (const) — Schema version stamped on every activity event (currently 1).
- **ActivityEvent** (struct) — Immutable record of a single execution fact.
- **BoundaryTarget** (enum) — Sandbox boundary crossed by a tool (FileSystem, Shell, Network, Mcp, Other).
- **ConsistencyError** (enum) — Inconsistency in a run's event log during resume validation.
- **EventKind** (enum) — Typed payload of an activity-log event.
- **Principal** (enum) — Who made a permission decision (Operator, Policy).
- **ProjectedCheckpoint** / **ProjectedMessage** / **ProjectedPermission** / **ProjectedToolCall** / **ProjectedToolResult** (structs) — Replay projection entries.
- **Projection** (struct) — Derived state rebuilt by replaying a run's event log.
- **ResumeResult** (struct) — Result of a resume operation.
- **RollbackResult** (struct) — Result of a rollback operation.
- **RunStatus** (enum) — Lifecycle state of a run.
- **TerminationReason** (enum) — Why a run terminated.
- **validate_event_log_consistency** (fn) — Validates an event log for gaps and orphaned results.
- **CronEvent** (struct) — A scheduled agent run.
- **CronForm** (enum) — The three schedule forms (OneShot, RepeatFrom, RepeatNow).
- **CronSchedule** (struct) — A schedule capturing one of the three forms.
- **ParsedSchedule** (struct) / **ScheduleParseError** (enum) — Schedule parsing.
- **parse_duration** (fn) / **DurationParseError** (enum) — Duration parsing.
- **parse_schedule** (fn) — Parse a schedule expression.
- **RagentError** (enum) — Structured error type for shared ragent operations.
- **Event** (enum) — Discrete occurrence in a session lifecycle (~90 variants).
- **EventBus** (struct) — Broadcasts `Event` values via a Tokio broadcast channel.
- **EventId** / **MessageId** / **RunId** / **SessionId** (newtypes) — Typed identifiers.
- **ChatContent** (enum) / **ChatMessage** (struct) / **ChatRequest** (struct) — LLM conversation primitives.
- **ContentPart** (enum) — Typed content block in a ChatMessage. `ToolUse::input` is `serde_json::Value` so a replayed call round-trips byte for byte (FR-006/FR-018; spec `uniformtools` T-025).
- **LlmFinishReason** (type alias) — Re-export of `event::FinishReason`.
- **StreamEvent** (enum) — Events emitted by an LLM streaming response.
- **ToolDefinition** (struct) — Schema describing a tool the LLM may invoke. Its `parameters` is `serde_json::Value` on purpose (spec `uniformtools` T-025): the field is forwarded, not interpreted, so a typed subset could only drop keywords or add a schema crate (NFR-002).
- **ImageData** (struct) — An image attachment reference.
- **Message** (struct) / **MessagePart** (enum) / **Role** (enum) — Conversation message types.
- **ToolCallState** (struct) / **ToolCallStatus** (enum) — Tool call lifecycle. `input` is `serde_json::Value` (decoded by `tool_args::decode_tool_arguments`) and `output` is `Option<Value>`; both stay untyped because the shape is owned by the callee and the callee set is open at run time (spec `uniformtools` T-025).
- **PermissionDecision** (enum) — User's response to a permission request.
- **StartupTimings** (struct) — Collected timings for every instrumented startup stage.
- **ThinkingConfig** (struct) / **ThinkingDisplay** (enum) / **ThinkingLevel** (enum) — Thinking configuration.
- **truncate_bytes** / **truncate_bytes_no_ellipsis** / **truncate_chars** (fns) — String truncation.
- **TriggerActionKind** (enum) / **TriggerEnvelope** (struct) / **TriggerFired** (struct) / **TriggerRule** (struct) / **TriggerRuleId** (newtype) / **TriggerRuleStatus** (enum) / **TriggerSourceKind** (enum) — Trigger types.

## Design decisions

- **Tool-call JSON stays `serde_json::Value` (T-025, FR-001).** Four fields hold
  tool-call JSON untyped: `ToolCallState::input` and `::output`
  (`message`), `ContentPart::ToolUse::input` (`llm`) and
  `ToolDefinition::parameters` (`llm`). This is a recorded decision, not an open
  TODO: the arguments shape is owned by the callee and the callee set is open at
  run time (148 built-in tools plus whatever an MCP server or plugin registers
  after the binary was compiled), the wire record must round-trip the payload
  unchanged (FR-006, FR-018), and the parameters field is forwarded JSON Schema -
  155 `additionalProperties` keyword literals in tree, and arbitrary third-party
  documents beyond them - so a typed subset could only drop keywords or add a
  schema crate, which NFR-002 forbids. Full rationale:
  `specs/uniformtools/T-025-TYPED-ARGS-DECISION.md`. Guarded by
  `tests/test_typed_args_decision.rs`.
