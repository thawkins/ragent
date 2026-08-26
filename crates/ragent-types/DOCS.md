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
- **ContentPart** (enum) — Typed content block in a ChatMessage.
- **LlmFinishReason** (type alias) — Re-export of `event::FinishReason`.
- **StreamEvent** (enum) — Events emitted by an LLM streaming response.
- **ToolDefinition** (struct) — Schema describing a tool the LLM may invoke.
- **ImageData** (struct) — An image attachment reference.
- **Message** (struct) / **MessagePart** (enum) / **Role** (enum) — Conversation message types.
- **ToolCallState** (struct) / **ToolCallStatus** (enum) — Tool call lifecycle.
- **PermissionDecision** (enum) — User's response to a permission request.
- **StartupTimings** (struct) — Collected timings for every instrumented startup stage.
- **ThinkingConfig** (struct) / **ThinkingDisplay** (enum) / **ThinkingLevel** (enum) — Thinking configuration.
- **truncate_bytes** / **truncate_bytes_no_ellipsis** / **truncate_chars** (fns) — String truncation.
- **TriggerActionKind** (enum) / **TriggerEnvelope** (struct) / **TriggerFired** (struct) / **TriggerRule** (struct) / **TriggerRuleId** (newtype) / **TriggerRuleStatus** (enum) / **TriggerSourceKind** (enum) — Trigger types.