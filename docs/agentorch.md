# ragent — Agent Loop & Orchestrator Internals

This document is a component-by-component map of ragent's **agent loop** (the
`SessionProcessor` that drives a single user message through LLM streaming and
tool execution) and the **orchestrator** (the multi-agent coordination layer).
Every entry annotates the file and line range where the component lives, so you
can jump straight to the code.

All paths are relative to `crates/ragent-agent/src/` unless stated otherwise.

---

## 1. Architecture at a glance

```
User message
   │
   ▼
SessionProcessor::process_user_message        (session/processor.rs:459)
   ├─ 1. persist user message                  (processor.rs:474)
   ├─ 2. prepare_client                        (session/loop_steps.rs:125)
   ├─ 3. build_turn_system_prompt              (loop_steps.rs:364)
   ├─ 4. build_turn_chat_messages              (loop_steps.rs:602)
   ├─ 5. run_inline_init_acknowledgement       (loop_steps.rs:689)
   │
   ▼  main loop  (processor.rs:675)
   ├─ compaction trigger check                 (processor.rs:756)
   ├─ call_llm_step   ──► StreamBuffer ──► events      (loop_steps.rs:782)
   ├─ no-tool decision / nudge                 (loop_steps.rs:1265)
   ├─ tool dispatch (parallel tokio tasks)     (processor.rs:947)
   ├─ background task / shell injection        (processor.rs:1544 / 1587)
   └─ interim save                             (processor.rs:1622)
   │
   ▼
finalize / run-cost summary / MessageEnd       (processor.rs:1682, 1381)
```

The **orchestrator** crate section (`orchestrator/*`) is a parallel, higher-level
abstraction used for multi-agent job coordination — it is *not* the loop that
`SessionProcessor` runs. See §8.

---

## 2. The SessionProcessor struct

`SessionProcessor` holds every shared dependency the loop needs. It lives in
`session/processor.rs:58` and is the single struct that drives an agent turn.

Key fields:

| Field | Location | Purpose |
| ----- | -------- | ------- |
| `session_manager` | `processor.rs:60` | session CRUD + persistence |
| `provider_registry` | `processor.rs:62` | LLM provider lookup |
| `tool_registry` | `processor.rs:64` | registered tools |
| `permission_checker` | `processor.rs:66` | allow/deny/ask policy gate |
| `event_bus` | `processor.rs:68` | tokio broadcast of all lifecycle events |
| `agent_manager` | `processor.rs:71` | sub-agent spawning (`OnceLock`) |
| `team_manager` | `processor.rs:74` | team/teammate coordination (`OnceLock`) |
| `mcp_client` | `processor.rs:86` | dynamic MCP tools (`OnceLock`) |
| `code_index` | `processor.rs:89` | codebase search (`OnceLock`) |
| `bg_service` | `processor.rs:92` | background shell tasks (`OnceLock`) |
| `active_spec` | `processor.rs:95` | active spec id for prompt injection |
| `spec_manager` | `processor.rs:98` | spec CRUD (`OnceLock`) |
| `cached_tool_definitions` | `processor.rs:101` | per-step tool defs cache |
| `cached_tool_names` | `processor.rs:109` | `ToolsSent` event names cache (PERF-003) |
| `cached_tool_definition_bytes` | `processor.rs:116` | serialised size cache (PERF-014) |
| `stream_config` | `processor.rs:118` | timeouts / retries / backoff |
| `auto_approve` | `processor.rs:122` | `--yes` YOLO-style flag |
| `system_prompt_cache` | `processor.rs:131` | per-turn prompt component cache |
| `read_timestamps` | `processor.rs:136` | stale-file edit detection |
| `cached_config` | `processor.rs:145` | mtime-keyed config cache (P-2) |
| `telemetry` | `processor.rs:148` | metrics subsystem |
| `skill_body_cache` | `processor.rs:155` | per-session skill body cache |

Tool-definition caching is managed by:

- `set_mcp_client` — `processor.rs:181` (registers MCP tools, invalidates cache)
- `invalidate_tool_cache` — `processor.rs:229`
- `get_cached_tool_definitions` — `processor.rs:251` (also primes names + bytes)
- `get_cached_tool_definition_bytes` — `processor.rs:284`
- `get_cached_tool_names` — `processor.rs:293`

Config resolution:

- `system_prompt_cache` / `invalidate_system_prompt_cache` — `processor.rs:307` / `325`
- `invalidate_config_cache` — `processor.rs:333`
- `load_config_cached` — `processor.rs:344` (mtime-keyed `CachedConfig`, struct at `processor.rs:167`)

---

## 3. Storage bridging

Blocking SQLite work is never done on the async runtime. `storage_op` spawns a
blocking thread:

- `storage_op` — `session/processor.rs:393` (`tokio::task::spawn_blocking`)

Every storage call in the loop routes through this helper (e.g. `get_messages`,
`create_message`, `update_message`).

---

## 4. The agent loop — step by step

`process_user_message` (`processor.rs:459`) is the orchestrator entry point. It
delegates to the extracted steps in `loop_steps.rs`.

### 4.1 Persist user message (`processor.rs:474`)

The incoming `Message` is written via `storage_op(s.create_message)`, then
`Event::MessageStart` is published.

### 4.2 Run-cost accumulator listener (`processor.rs:490`)

A background task subscribes to `Event::TokenUsage` and accumulates input/output
tokens for the turn; `publish_run_cost_summary` (`processor.rs:530`) computes
`compute_run_cost` (`cost/mod.rs:167`) and publishes `Event::RunCostSummary` at
the end. Cost persistence uses `RunCostSummaryRow` (FR-018).

### 4.3 prepare_client (`loop_steps.rs:125`)

Resolves everything needed to talk to the model:

1. Load config once (cached) — `loop_steps.rs:148`
2. Resolve model ref — `loop_steps.rs:153`
3. Resolve provider — `loop_steps.rs:164`
4. Resolve API key — `loop_steps.rs:176`
5. Provider-specific base URL (copilot / generic_openai / azure_foundry /
   azure_resource) — `loop_steps.rs:189`
6. Create the `LlmClient` with provider options — `loop_steps.rs:254`
7. Resolve working dir — `loop_steps.rs:291`
8. Resolve team context (5s TTL cache) — `loop_steps.rs:300`
9. Parse hook configs + fire `OnSessionStart` on first message — `loop_steps.rs:331`

Returns a `TurnClient` (`loop_steps.rs:58`) holding the model ref, client, config,
hook configs, working dir, and team context.

### 4.4 build_turn_system_prompt (`loop_steps.rs:364`)

Assembles the per-turn system prompt from:

- skill registry (`loop_steps.rs:374`)
- git status / README / AGENTS.md / file tree (`loop_steps.rs:378`)
- memory section via `spawn_blocking` (`loop_steps.rs:382`)
- durable initiatives section (`loop_steps.rs:395`)
- base prompt via `build_system_prompt_with_storage_and_memory` (`loop_steps.rs:405`)
- tool reference (compact for lead, detailed for subagent) via
  `SystemPromptCache` (`loop_steps.rs:421`)
- question-tool usage guidance (`loop_steps.rs:442`)
- codeindex guidance (active/disabled, cached) (`loop_steps.rs:458`)
- tool-calling directive `TOOL_CALLING_GUIDANCE` (`prompt_builders.rs:16`)
- team-lead task-distribution rules or teammate workflow (`loop_steps.rs:475`)
- active spec section (cached by `(spec_id, modified_at)`) (`loop_steps.rs:537`)

Returns an `Arc<str>` (PERF-006). The builders live in `session/prompt_builders.rs`.

### 4.5 build_turn_chat_messages (`loop_steps.rs:602`)

1. Load persisted history via `storage_op(get_messages)` — `loop_steps.rs:610`
2. Resolve the model's context window (falls back to 128k for virtual router) —
   `loop_steps.rs:627`
3. Convert history → `ChatMessage`s via `history_to_chat_messages`
   (`session/history.rs:104`), cached by `SessionState` version
   (`session/cache.rs:504`) — `loop_steps.rs:646`

Returns `(chat_messages, compressed, last_reported_input_tokens, context_window)`.

### 4.6 run_inline_init_acknowledgement (`loop_steps.rs:689`)

Display-only AGENTS.md acknowledgement exchange: streams a one-shot "guidelines
loaded" turn to the TUI. It is **not** persisted to history. Skipped for
subagents and when an assistant turn already exists.

### 4.7 The main loop (`processor.rs:675`)

The loop body per iteration:

1. **Increment step** — `processor.rs:677` (`event_bus.set_step`); stop when
   `step > max_steps` (`processor.rs:688`) or the cancel flag is set
   (`processor.rs:697`).
2. **Publish `ToolsSent`** on step 1 only (PERF-004) — `processor.rs:734`.
3. **Pre-send compaction check** — `processor.rs:756` (see §5).
4. **call_llm_step** — `processor.rs:853` (see §4.8).
5. **Collect response parts** (reasoning + text) — `processor.rs:876`.
6. **No-tool decision** — `processor.rs:914` (see §4.10).
7. **Tool dispatch** — `processor.rs:947` (see §4.11).
8. **Background task / shell injection** — `processor.rs:1544` / `1587`.
9. **Interim save** — `processor.rs:1622` (hash-compare to avoid redundant
   writes).

### 4.8 call_llm_step (`loop_steps.rs:782`)

The largest single step. With `max_retries` + linear backoff:

1. Retry loop with backoff sleep — `loop_steps.rs:807`
2. Build `ChatRequest` (sharing history by `Arc` refcount, PERF-006) —
   `loop_steps.rs:837`
3. Publish `Event::RequestStarted` — `loop_steps.rs:852`
4. Call `client.chat` — `loop_steps.rs:857`; on failure, classify the error:
   - token overflow → **emergency compaction** (`emergency_compact`) then retry
     (`loop_steps.rs:877`)
   - transient → backoff retry (`loop_steps.rs:868`)
   - permanent → fire `OnError` hook, bail (`loop_steps.rs:938`)
5. Stream events through a stall-guarded select loop — `loop_steps.rs:966`:
   - `TextDelta` / `ReasoningDelta` → buffered via `StreamBuffer`
     (`session/stream_buffer.rs:50`), flushed to the bus
   - `ToolCallStart` / `ToolCallDelta` / `ToolCallEnd` → accumulated into
     `PendingToolCall`s
   - `Usage` → recorded + `Event::TokenUsage`
   - `Error` → emergency overflow / retry / fatal handling
   - `RateLimit` → `Event::QuotaUpdate`
6. Return `LlmStepResult` (`loop_steps.rs:105`) with text/reasoning/tool-call
   buffers and token counts.

### 4.9 Stream buffering & stall detection (`session/stream_buffer.rs`)

- `StreamBuffer` (`stream_buffer.rs:50`) coalesces small deltas; flush on
  `256` chars or `50` ms (`stream_buffer.rs:14-16`).
- `stall_pattern_set()` (`stream_buffer.rs:24`) — a `OnceLock<RegexSet>` of
  Ollama "planning" phrases used to nudge the model.

### 4.10 handle_no_tool_decision (`loop_steps.rs:1265`)

When the LLM returned no tool calls:

1. Detect stall (dots-only output), planning (regex match on Ollama), and
   incomplete-file-task (`detect_incomplete_file_task`,
   `session/history.rs:533`) — `loop_steps.rs:1275`.
2. If any nudge applies, push an assistant + user nudge pair into
   `chat_messages` and return `true` (continue) — `loop_steps.rs:1303`.
3. Post-compaction continuation nudge (once per turn) — `loop_steps.rs:1351`.
4. Otherwise return `false` (break).

### 4.11 Tool dispatch (`processor.rs:947`)

- Build a single `ToolContext` per step and clone it per call (PERF-008/009) —
  `processor.rs:969`.
- Each tool call is `tokio::spawn`ed — `processor.rs:1129`:
  1. Publish `ToolCallStart` — `processor.rs:1132`
  2. Run **pre-tool-use hooks** (`hooks/mod.rs:225`) — `processor.rs:1137`
  3. Acquire a tool permit (`ragent_types::resource`) — `processor.rs:1204`
  4. Resolve + execute the tool — `processor.rs:1210`
  5. Permission gate via `check_permission_with_prompt`
     (`session/permissions.rs:189`); bash sub-commands split and checked
     individually — `processor.rs:1216`
  6. Record telemetry — `processor.rs:1308`
  7. Run **post-tool-use hooks** (`hooks/mod.rs:398`) — `processor.rs:1324`
  8. Publish `ToolCallEnd` + `ToolResult` — `processor.rs:1404` / `1424`
  9. Feed `ExtractionEngine` for memory candidates — `processor.rs:1433`
- **Parallel vs serial**: when `experimental.parallel_tool_calls` is set the
  futures are `join_all`ed — `processor.rs:1457` / `1463`.
- Publish a single `ToolCallBatch` (PERF-015) — `processor.rs:1476`.
- Agent switch / task complete break the loop — `processor.rs:1483`.
- Auto spec-task update on file-write tools — `processor.rs:1489`.
- Append assistant + tool-result `ChatMessage`s to history — `processor.rs:1534`.

### 4.12 Background task / shell injection

- Sub-agent completions drained from `AgentManager` and injected as a user
  `ContentPart::Text` — `processor.rs:1544`.
- Background shell task completions drained from `bg_service` — `processor.rs:1587`.

### 4.13 Interim save (`processor.rs:1622`)

Hashes the assistant parts (`FxHasher`) and only writes when the hash changes
(PERF-012), avoiding redundant SQLite updates every step.

### 4.14 Finalize

- Final save (moving the message into the closure, PERF-020) — `processor.rs:1682`
- Publish `MessageEnd` (`FinishReason::Stop`) — `processor.rs:1716`
- Fire `OnSessionEnd` hook — `processor.rs:1721`
- The extracted helper `finalize_assistant_message` does the same in
  `loop_steps.rs:1381`.

---

## 5. Compaction

The compaction pipeline replaces the old Headroom compression module. Module
root: `compaction/mod.rs`.

| Component | Location | Purpose |
| --------- | -------- | ------- |
| Estimator + trigger | `compaction/estimator.rs` | local token estimate + decide whether to compact |
| Prompt builder | `compaction/prompt.rs` | Markdown summarisation template |
| Serialiser | `compaction/serializer.rs` | flatten history → transcript |
| Runner | `compaction/runner.rs` | select tail, call LLM, build replacement |
| Converters | `compaction/convert.rs` | `ChatMessage` ↔ `Message` |

### 5.1 Estimator (`compaction/estimator.rs`)

- `estimate_text_tokens` — `estimator.rs:70` (chars ÷ 4, OpenCode-compatible)
- `estimate_message_tokens` — `estimator.rs:84`
- `estimate_tool_tokens` — `estimator.rs:114`
- `estimate_request_tokens` — `estimator.rs:134`
- `estimate_chat_request_tokens` — `estimator.rs:152`
- `effective_request_tokens` — `estimator.rs:190` (prefer provider-reported input)
- `compaction_threshold` — `estimator.rs:207` (percentage or buffer model)
- `evaluate_trigger` — `estimator.rs:236` → returns `TriggerDecision` (`estimator.rs:170`)
- constants: `CHARS_PER_TOKEN` (`estimator.rs:53`), `MESSAGE_OVERHEAD_TOKENS`
  (`estimator.rs:60`), `IMAGE_TOKEN_ESTIMATE` (`estimator.rs:63`)

### 5.2 Runner (`compaction/runner.rs`)

- `select` — `runner.rs:117` (choose verbatim recent tail within `keep_tokens`,
  always keep last message) → `SelectedSplit` (`runner.rs:67`)
- `summarize_via_client` — `runner.rs:198` (heartbeat + overall timeout)
- `build_summary_request` — `runner.rs:259`
- `build_compaction_message` — `runner.rs:292`
- `compact` �� `runner.rs:337` → `CompactionOutcome` (`runner.rs:85`):
  1. select tail (`runner.rs:354`)
  2. nothing-to-summarise guard (`runner.rs:359`)
  3. build prompt + overflow guard (`runner.rs:371`)
  4. publish `CompressionStarted` + call LLM (`runner.rs:401`)
  5. build `[compaction_msg, ...recent]` (`runner.rs:441`)
  6. publish `CompressionFinished` with token stats (`runner.rs:468`)
- `emergency_compact` — `runner.rs:518` (replaces `chat_messages` in place for
  the overflow path)
- `cap_head_transcript` — `runner.rs:564`
- `MAX_COMPACTION_PROMPT_CHARS` (`runner.rs:56`), `SUMMARY_HEARTBEAT_SECS`
  (`runner.rs:59`)

### 5.3 Serialiser (`compaction/serializer.rs`)

- `serialize_messages` — `serializer.rs:40`
- `serialize_message` — `serializer.rs:54`
- `truncate` — `serializer.rs:163`
- `TOOL_OUTPUT_MAX_CHARS` (`serializer.rs:27`)

### 5.4 Converters (`compaction/convert.rs`)

- `chat_messages_to_messages` — `convert.rs:21`
- `messages_to_chat_messages` — `convert.rs:115`

### 5.5 Where compaction is invoked from the loop

- **Pre-send** (auto, per-iteration): `processor.rs:756` → `estimate_request_tokens`
  → `evaluate_trigger` → `compact(...)`; persist the compaction message
  (`processor.rs:811`).
- **Emergency overflow** (request failure): `loop_steps.rs:877`
- **Emergency overflow** (stream error): `loop_steps.rs:1114`
- Both call `emergency_compact` (`runner.rs:518`) with `reason = "overflow"`.

---

## 6. Event bus

The event bus is a tokio `broadcast` channel over the `Event` enum (71 variants)
with a per-session step counter.

| Item | Location |
| ---- | -------- |
| Re-export | `event/mod.rs:8` |
| `EventBus` struct | `crates/ragent-types/src/event/mod.rs:850` |
| `EventBus::new` | `event/mod.rs:1034` |
| `set_step` / `current_step` | `event/mod.rs:1046` / `1062` |
| `subscribe` | `event/mod.rs:1082` |
| `publish` | `event/mod.rs:1108` |

Key events the loop emits: `MessageStart`, `ToolsSent`, `RequestStarted`,
`TextDelta`, `ReasoningDelta`, `TokenUsage`, `ModelResponse`, `ToolCallStart`,
`ToolCallArgs`, `ToolCallEnd`, `ToolResult`, `ToolCallBatch`, `AgentNotice`,
`AgentError`, `RunCostSummary`, `CompressionStarted`, `CompressionFinished`,
`MessageEnd`.

---

## 7. Supporting modules used by the loop

### 7.1 History helpers (`session/history.rs`)

- `PendingToolCall` — `history.rs:37`
- `resolve_team_context_for_session` — `history.rs:48`
- `history_to_chat_messages` — `history.rs:104`
- `tool_result_content_for_llm` — `history.rs:220` (12k char cap, head/tail)
- `estimate_request_bytes` — `history.rs:276`
- `estimate_tool_definition_bytes` — `history.rs:331`
- `chat_request_payload_bytes` — `history.rs:339`
- `is_token_overflow_error_message` — `history.rs:346`
- `is_permanent_llm_api_error` — `history.rs:376`
- `stream_has_meaningful_partial_output` — `history.rs:437`
- `should_retry_stream_error` — `history.rs:448`
- `detect_incomplete_file_task` — `history.rs:533`

### 7.2 Permissions (`session/permissions.rs` + `permission/mod.rs`)

- `permission/mod.rs:17` re-exports the canonical types from
  `ragent_config::permission` and `ragent_types::permission`.
- `check_permission_with_prompt` — `session/permissions.rs:189`
- bash splitting / resource extraction helpers imported in `processor.rs:39-41`.

### 7.3 Profiler (`session/profiler.rs`)

- `AgentLoopProfiler` — `profiler.rs:58`
- `scope` / `scope_with` / `scope_owned` — `profiler.rs:111` / `125` / `137`
- `record_duration` — `profiler.rs:156`
- `snapshot` — `profiler.rs:173`
- `agent_loop_profiler()` — `profiler.rs:355`

Scopes are sprinkled through the loop (e.g. `loop.step.total`,
`loop.tool_phase.total`, `loop.llm.stream`) and power the `/profiler` panel.

### 7.4 Prompt builders (`session/prompt_builders.rs`)

- `TOOL_CALLING_GUIDANCE` — `prompt_builders.rs:16`
- `build_codeindex_guidance_section_active` — `prompt_builders.rs:26`
- `build_codeindex_guidance_section_disabled` — `prompt_builders.rs:62`
- `build_tool_reference_section` — `prompt_builders.rs:80`
- `build_detailed_tool_reference_section` — sub-agent variant (used at
  `loop_steps.rs:426`)

### 7.5 Session-state / system-prompt cache (`session/cache.rs`)

- `SystemPromptCache` — `cache.rs:103` (agent prompt `:172`, tool reference
  `:217`, codeindex guidance `:252`, team guidance `:297`, spec section
  `:390`/`:405`)
- `SessionState` — `cache.rs:427` (chat-message cache by history version
  `:504`, serialised cache `:528`)
- `invalidate_all_caches` / `current_cache_version` — `cache.rs:24` / `30`

### 7.6 Hooks (`hooks/mod.rs`)

- `HookTrigger` — `hooks/mod.rs:48`
- `parse_hook_configs` — `hooks/mod.rs:122`
- `run_pre_tool_use_hooks` — `hooks/mod.rs:225`
- `run_post_tool_use_hooks` — `hooks/mod.rs:398`
- `fire_hooks` — `hooks/mod.rs:562`

### 7.7 Tool registry (`tool/mod.rs`)

- `Tool::execute` trait — `tool/mod.rs:324`
- `ToolRegistry::get` — `tool/mod.rs:1229`
- `ToolRegistry::definitions` — `tool/mod.rs:1265`

---

## 8. The orchestrator crate

`orchestrator/mod.rs` provides a separate, higher-level multi-agent coordination
layer (capability registry, in-process router, coordinator, leader election,
policy). It is not the loop that `SessionProcessor` runs, but it is the
orchestration substrate for parallel/team work.

### 8.1 Module map (`orchestrator/mod.rs:13-20`)

| Submodule | File | Role |
| --------- | ---- | ---- |
| coordinator | `orchestrator/coordinator.rs` | job dispatch + aggregation |
| leader | `orchestrator/leader.rs` | leader election + `CoordinatorCluster` |
| policy | `orchestrator/policy.rs` | conflict resolution + human fallback |
| registry | `orchestrator/registry.rs` | capability-based agent registry |
| router | `orchestrator/router.rs` | routing trait + in-process router |
| transport | `orchestrator/transport.rs` | pluggable transport adapters |

### 8.2 Types

- `OrchestrationMessage` — `coordinator.rs:13`
- `JobDescriptor` — `coordinator.rs:22` (id + required capabilities + payload)
- `JobEvent` — `coordinator.rs:33` (started / assigned / completed / failed)
- `Coordinator` — `coordinator.rs:120`
  - `start_job_sync` — `coordinator.rs:213`
  - `start_job_async` — `coordinator.rs:371`
  - `subscribe_job_events` — `coordinator.rs:484`
- `AgentRegistry` — `registry.rs:57` (`RwLock<HashMap<AgentId, AgentEntry>>`)
  - `AgentEntry` — `registry.rs:25` (id + capabilities + mailbox + heartbeat)
  - `Responder` — `registry.rs:11` (callback for in-process agents)
- `LeaderElector` — `leader.rs:47`
  - `nominate` / `withdraw` / `current_leader` / `is_leader` / `subscribe` —
    `leader.rs:68-104`
  - `CoordinatorCluster` — `leader.rs:150`
- `ConflictPolicy` — `policy.rs:31`
  - `ConflictResolver` — `policy.rs:92`; `resolve` — `policy.rs:115`
  - `LoggingFallback` — `policy.rs:71`

### 8.3 How it fits together

1. Agents register with the `AgentRegistry` by id + capabilities
   (`registry.rs:70`), optionally providing a `Responder` callback.
2. A `Coordinator` takes a `JobDescriptor`, matches agents by required
   capabilities, and runs the job synchronously (`start_job_sync`) or
   asynchronously (`start_job_async`) with event subscription
   (`subscribe_job_events`).
3. `LeaderElector` / `CoordinatorCluster` add distributed leader selection.
4. `ConflictResolver` applies a `ConflictPolicy` (with optional human fallback)
   when aggregating multiple agent responses.

Note: the production team/sub-agent flow (`/swarm`, `team_spawn`, `new_agent`)
lives in `team/` and `task/` and routes through `SessionProcessor`'s
`agent_manager` / `team_manager`, not through this orchestrator crate.

---

## 9. Quick reference: key line numbers

| Concern | File:line |
| ------- | --------- |
| Loop entry point | `session/processor.rs:459` |
| Loop body | `session/processor.rs:675` |
| Compaction trigger (pre-send) | `session/processor.rs:756` |
| Tool dispatch | `session/processor.rs:947` |
| Finalize | `session/processor.rs:1682` |
| prepare_client | `session/loop_steps.rs:125` |
| build_turn_system_prompt | `session/loop_steps.rs:364` |
| build_turn_chat_messages | `session/loop_steps.rs:602` |
| call_llm_step | `session/loop_steps.rs:782` |
| handle_no_tool_decision | `session/loop_steps.rs:1265` |
| Storage bridge | `session/processor.rs:393` |
| Event bus | `crates/ragent-types/src/event/mod.rs:850` |
| Compaction runner | `compaction/runner.rs:337` |
| Emergency compaction | `compaction/runner.rs:518` |
| Token estimator | `compaction/estimator.rs:70` |
| Trigger decision | `compaction/estimator.rs:236` |
| Coordinator | `orchestrator/coordinator.rs:120` |
| Agent registry | `orchestrator/registry.rs:57` |
