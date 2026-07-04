# PERFPLAN.md — Agent Action Loop Performance Plan

**Status:** ✅ COMPLETE — all 6 milestones (A–F) implemented and verified
**Owner:** ragent-agent
**Completion report:** `docs/reports/perfplan-completion.md`
**Baseline:** `docs/reports/agent_loop_perf_baseline.md`
**Scope:** `crates/ragent-agent/src/session/processor.rs`, `loop_steps.rs`, and supporting modules under `crates/ragent-agent/src/session/`.

## Purpose

A concrete, file-and-line-referenced plan to reduce per-turn latency and per-step
allocations in `SessionProcessor::process_user_message`. This is the execution
companion to `specs/AgentPerf/SPEC.md` (status: implemented) and supersedes the
remaining open items in `specs/AgentPerf/PLAN.md` by collecting the still-live
findings into one prioritised backlog with exact change sites.

The agent loop is the single latency-sensitive hot path in ragent. A user turn
can spend seconds to tens of seconds inside it, and a substantial fraction of
that is spent on work the loop does not need to redo: re-cloning chat messages,
re-reading the config from disk, re-locking async guards, re-serialising JSON,
and re-publishing events nobody consumes. This plan enumerates those wastes and
orders them so each milestone is independently shippable and independently
testable.

## Method

Each finding below was verified by reading the current source:

- `crates/ragent-agent/src/session/processor.rs` — the `process_user_message`
  orchestrator (lines 333–1164) and the inline tool-dispatch block (lines
  710–1082) that REMPLAN T6.5 left inline.
- `crates/ragent-agent/src/session/loop_steps.rs` — the seven extracted steps
  (`prepare_client`, `build_turn_system_prompt`, `build_turn_chat_messages`,
  `run_inline_init_acknowledgement`, `call_llm_step`, `handle_no_tool_decision`,
  `finalize_assistant_message`) plus the `TurnClient` / `LoopState` /
  `LlmStepResult` structs.
- `crates/ragent-agent/src/session/profiler.rs` — the `AgentLoopProfiler` and
  the `scope` / `scope_with` / `scope_owned` paths.
- `crates/ragent-agent/src/session/cache.rs` — `SystemPromptCache` and the
  per-session `SessionStateCache`.
- `crates/ragent-agent/src/session/history.rs` — `history_to_chat_messages`,
  `estimate_request_bytes`, `estimate_tool_definition_bytes`,
  `chat_request_payload_bytes`, `emergency_compress_chat_messages`.
- `crates/ragent-types/src/event/mod.rs` — the `Event::ToolsSent` variant and
  its consumers.

I also cross-checked `specs/AgentPerf/SPEC.md` to avoid re-proposing work that
spec already marks done (FR-006 history cache, FR-008/FR-009 system-prompt cache,
FR-010 `storage_op`, FR-016 `STALL_PATTERN_SET`). Findings that the spec already
addresses are excluded; what remains below is net-new waste observed in the
current source.

## Findings

Each finding is keyed `P-N`, names the exact file and line range, states the
observed waste, and estimates the fix size (S ≤ 1h, M ≤ 1d, L > 1d). Findings
are grouped by theme; the execution plan at the end reorders them by risk and
independence.

### A. Dead / redundant work

**P-1 — `get_messages` runs synchronous SQLite on the async runtime.**
`processor.rs:386` calls `self.session_manager.get_messages(session_id)?`
directly inside `process_user_message`, not via `storage_op`. Every other
storage call in the loop goes through `storage_op`
(`processor.rs:350, 426, 479, 826, 1136, 1148`) which dispatches to
`tokio::task::spawn_blocking`. This one site blocks the executor on SQLite.
Fix: route through `storage_op`. **Size: S.**

**P-2 — `Config::load()` re-reads config files from disk every turn.**
`loop_steps.rs:prepare_client` resolves the session config by loading
`ragent.json`/`ragent.jsonc` from disk on every `process_user_message` call.
The config is immutable for the lifetime of a session; re-reading it per turn
is pure I/O. Fix: cache the resolved `Config` on `SessionManager` (or on the
processor) keyed by (working_dir, mtime) and only reload when the file mtime
changes. **Size: M.**

**P-3 — `build_turn_chat_messages` computes `context_window` twice.**
`loop_steps.rs:579–588` resolves `context_window` from the provider registry,
and `processor.rs:431–440` resolves the identical `context_window` again
immediately after. Fix: return `context_window` from
`build_turn_chat_messages` and reuse it. **Size: S.**

**P-4 — `ToolsSent` event published every step, consumed only on step 1.**
`processor.rs:491–500` publishes `Event::ToolsSent { tools: tool_names }` on
every loop step, cloning ~111 `String`s (or cloning the cached `Arc<[String]>`
into a `Vec<String>`) each time. The TUI only renders the tool list on the first
step of a turn. Fix: publish only when `step == 1`, or switch the event payload
to `Arc<[String]>` so the publish is a refcount bump. **Size: S.**

**P-5 — `handle_no_tool_decision` is called, its result discarded, then the
same nudge logic runs inline.**
`loop_steps.rs:1152` defines `handle_no_tool_decision`, which computes the three
nudge booleans and pushes nudge messages into `LoopState`. In `processor.rs`
the orchestrator constructs a `LoopState`, calls the helper, then immediately
recomputes the same three booleans inline (lines 654–705) and pushes the same
nudge messages. The helper call is dead work on every no-tool step. Fix: delete
the inline recomputation and trust the `LoopState` mutation the helper already
did. **Size: S.**

### B. Per-step allocation

**P-6 — `loop_state.chat_messages.clone()` per LLM attempt.**
`loop_steps.rs:803–804` does
`Arc::new(loop_state.chat_messages.clone())` for every retry attempt, cloning
the entire history `Vec<ChatMessage>` including all tool-result `ContentPart`s.
On a long conversation this is the single largest per-step allocation. Fix:
make `LoopState.chat_messages: Arc<Vec<ChatMessage>>` and mutate via
`Arc::make_mut`, so an unchanged history is shared by refcount. **Size: M.**

**P-7 — `estimate_tool_definition_bytes` exists but is never called; the
per-step byte estimate re-serialises ~111 tool schemas.**
`history.rs` exports `estimate_tool_definition_bytes` (see the re-export at
`processor.rs:50`) but the loop never uses it. Wherever the request size is
estimated per step, the code falls back to re-serialising the full
`tool_definitions` slice. Fix: call `estimate_tool_definition_bytes` once when
`cached_tool_definitions` is populated and store the result alongside it.
**Size: S.**

**P-8 — `ToolContext` rebuilt per tool call with ~20 clones.**
`processor.rs:796–815` constructs a fresh `ToolContext` inside the `for tc in
&llm_result.tool_calls` loop. Every field is cloned per tool call, including
`turn.working_dir` (a `PathBuf`), `turn.team_context` (an `Arc`), the storage
handle, and `self.read_timestamps` (an `Arc<RwLock<…>>`). Most of these are
identical across all tool calls in a step. Fix: build one `ToolContext` per
step and `.clone()` only the per-call fields (`session_id` is already shared,
`active_spec_id` can be read once per step). **Size: M.**

**P-9 — `active_spec.read().await.clone()` per tool call.**
`processor.rs:811` reads the `tokio::sync::RwLock<Option<String>>` and clones
the `Option<String>` for every tool call in the step. The active spec does not
change mid-step. Fix: read it once per step (or once per turn) and reuse the
cloned value. **Size: S.**

**P-10 — Auto spec-task-update takes the async `active_spec` lock and scans
the spec on every step that has a file-write tool call.**
`processor.rs:1053–1078` runs after every tool phase. It acquires
`self.active_spec.read().await`, then `spec_mgr.read_spec(&id).await`, then
walks every task looking for `InProgress` to flip to `Completed`. This runs on
every step that calls `write`/`edit`/`multiedit`/`patch`/`create`/`append_to_file`,
even when no spec is active. Fix: short-circuit when `active_spec` is `None`
(cheap), and only scan the spec when a tool call actually returned success.
**Size: S.**

**P-11 — `drain_completed` polled every step.**
`processor.rs:1087–1108` calls `tm.drain_completed(session_id).await` on every
loop step even when no background task has been spawned. The call acquires a
lock and scans the task map. Fix: track an `AtomicBool` "has pending background
tasks" flag set by `new_task` and cleared by `drain_completed`, and skip the
call when the flag is false. **Size: S.**

**P-12 — Interim-save hash calls `Value::to_string()` per tool-call part.**
`processor.rs:1113–1132` hashes `assistant_parts` to detect changes. For
`MessagePart::ToolCall` it calls `state.input.to_string()` and
`out.to_string()` — full JSON serialisation of every tool-call input/output on
every step. Fix: hash the `Value` directly via `serde_json`'s raw bytes, or
cache the hash on the `ToolCallState` when it is built. **Size: M.**

### C. Event / tool-call throughput

**P-13 — `event_bus.set_step` called twice per step setup.**
`processor.rs:447–449` calls `set_step` then `current_step` then `set_step` again
in the same block. One of the two `set_step` calls is redundant. Fix: single
`set_step(session_id, step + 1)` and read the result. **Size: S.**

**P-14 — `stall_pattern_set()` re-fetched every no-tool step.**
`processor.rs:665` and `loop_steps.rs:handle_no_tool_decision` both call
`stall_pattern_set().is_match(...)`. `stall_pattern_set` returns a
`&'static RegexSet` via `OnceLock`, so the fetch is cheap, but the match itself
runs on the full text buffer even when the buffer is empty. Fix: guard with
`!llm_result.text_buffer.is_empty()` (already done in the inline version at
line 663; ensure the helper matches). **Size: S.**

**P-15 — `Event::ToolCallStart` / `ToolCallEnd` / `ToolResult` published
sequentially per tool call, even in parallel mode.**
`processor.rs:833–1030` publishes three events per tool call inside the
spawned task. In `parallel_tool_calls` mode these publications race and the
TUI must sort them by `call_id`. Fix: batch the per-tool events into a single
`Event::ToolCallBatch` consumed atomically by the TUI. **Size: L (new event
variant + TUI work).**

**P-16 — `tool_result_content_for_llm` returns `Arc<str>` but is called with
`&str` then immediately pushed into a `ContentPart::ToolResult` that owns a
`String`.**
`processor.rs:756–763` builds `ContentPart::ToolResult { content: ... }` where
`content` is a `String` derived from the `Arc<str>`. The `Arc<str>` could be
stored directly if `ContentPart::ToolResult` accepted `Arc<str>`. Fix: widen
`ContentPart::ToolResult.content` to `Cow<'static, str>` or `Arc<str>`.
**Size: M (touches `ragent-types`).**

**P-17 — `assistant_content_parts` and `tool_result_parts` allocated per
step.**
`processor.rs:713` and `720` allocate two fresh `Vec<ContentPart>` per step
even when the step has no tool calls (in which case both stay empty). Fix:
reuse a single pair of `Vec`s across steps via `clear()` + `truncate(0)`.
**Size: S.**

**P-18 — `llm_result.text_buffer.clone()` for `assistant_content_parts`.**
`processor.rs:714–718` clones the text buffer into a `ContentPart::Text`. The
buffer is then discarded. Fix: move the `String` into the `ContentPart` and
reconstruct an empty buffer for the next step, or pass `Arc<str>`. **Size: S.**

### D. Storage / I/O hygiene

**P-19 — `Event::ToolResult` content is truncated with a char-boundary scan
that re-iterates `char_indices` up to 200.**
`processor.rs:1017–1020` builds a preview by walking `char_indices` and taking
while `i <= 200`. For large tool outputs this is O(n) per event. Fix: use
`trailing_at_char_boundary` (already in `history.rs:195`) or cap the scan at
the first 400 bytes. **Size: S.**

**P-20 — `storage_op` closure captures a cloned `Message` per call.**
`processor.rs:349, 424, 479, 1136, 1148` each `let msg = user_msg.clone()` /
`assistant_msg.clone()` to move into the `storage_op` closure. The clone is
necessary because the closure is `Send + 'static`, but for the final save
(line 1147) the message could be moved directly since it is not used after.
Fix: `std::mem::take` the final `assistant_msg` into the closure. **Size: S.**

**P-21 — `emergency_compress_chat_messages` called from two sites in
`call_llm_step` with identical arguments.**
`loop_steps.rs:840` and `1045` both invoke `_emergency_compress` with the same
parameters when a stream error indicates token overflow. The second call is
only reachable if the first did not fire. Consolidate into one call site.
**Size: S.**

**P-22 — `history_to_chat_messages` is `async` but performs no `.await`.**
`loop_steps.rs:639` calls `history_to_chat_messages(&history).await`. The
function (re-exported at `processor.rs:48`) is async but does no I/O; the
`.await` is a no-op state machine hop. Fix: make the function synchronous (or
provide a sync variant) so the per-turn history build drops the `Future`
allocation. **Size: S.**

### E. Compression path

**P-23 — `crate::compression::pipeline::should_compress` re-scans the full
history every turn.**
`loop_steps.rs:592–597` calls `should_compress(&history, context_window,
auto_threshold)` which walks every `Message` to estimate tokens. The estimate
is unchanged between turns unless new messages were added. Fix: cache the
last-estimated token count on `SessionStateCache` and only re-run when the
history version changes. **Size: M.**

**P-24 — `build_turn_system_prompt` re-reads the active spec from disk every
turn even when the spec has not changed.**
`loop_steps.rs:514–553` acquires `self.active_spec.read().await`, then
`spec_mgr.read_spec(&id).await`, then formats the entire spec section into the
system prompt. The spec content is stable across turns. Fix: cache the
rendered spec section on `SystemPromptCache` keyed by (spec_id, spec_version).
**Size: M.**

### F. Profiler overhead

**P-25 — `profiler.scope(label)` allocates a `String` per scope even when
profiling is disabled.**
`profiler.rs:105–107` shows `scope` calls `scope_owned(label.to_string())`,
which allocates unconditionally before the `is_enabled()` check at line 124.
The check happens inside `scope_owned`, so the `to_string()` allocation is
already paid. Fix: move the `is_enabled()` check into `scope` before the
`to_string()`, returning `ProfileScope::disabled()` early. **Size: S.**

**P-26 — `scope_with` allocates the label via the closure even when profiling
is disabled.**
`profiler.rs:111–119` calls `label_fn()` only after the `is_enabled()` check,
so this path is already cheap — but `processor.rs:832`
(`profiler_clone.scope_with(|| format!("tool.total:{}", tc_clone.name))`)
still constructs the `format!` closure object per tool call. The closure is
cheap, but verify the `is_enabled()` short-circuit is actually hit in the
default (profiling-off) path. **Size: S (verification).**

## Execution Plan

Six milestones, ordered so each is independently shippable. Every milestone
ends with `cargo check --workspace` + `cargo build --workspace --tests` green
and the relevant crate test suite passing. No milestone changes the observable
event order or the on-disk session format.

### Milestone A — Low-risk mechanical wins

Scope: P-5, P-13, P-14, P-20, P-25, P-26.

| Task | Finding | Files | Size |
|------|---------|-------|------|
| A-1  | P-5     | `processor.rs` (delete inline nudge recomputation, trust `LoopState`) | S |
| A-2  | P-13    | `processor.rs:447–449` (single `set_step`) | S |
| A-3  | P-14    | `loop_steps.rs:handle_no_tool_decision` (guard empty buffer) | S |
| A-4  | P-20    | `processor.rs:1147` (`mem::take` final assistant msg) | S |
| A-5  | P-25    | `profiler.rs:105–107` (early `is_enabled()` in `scope`) | S |
| A-6  | P-26    | verify `scope_with` short-circuit; add test | S |

Exit criteria: `processor.rs` inline nudge block deleted; profiler `scope` no
longer allocates when disabled; all existing tests pass.

### Milestone B — Per-step allocation reduction

Scope: P-6, P-7, P-8, P-9, P-17, P-18.

| Task | Finding | Files | Size |
|------|---------|-------|------|
| B-1  | P-6     | `loop_steps.rs:LoopState` (`Arc<Vec<ChatMessage>>` + `make_mut`); `call_llm_step` | M |
| B-2  | P-7     | `processor.rs:get_cached_tool_definitions` (store byte estimate) | S |
| B-3  | P-8     | `processor.rs:796–815` (one `ToolContext` per step) | M |
| B-4  | P-9     | `processor.rs:811` (read `active_spec` once per step) | S |
| B-5  | P-17    | `processor.rs:713, 720` (reuse `Vec`s across steps) | S |
| B-6  | P-18    | `processor.rs:714–718` (move `text_buffer` into `ContentPart`) | S |

Exit criteria: per-step `String`/`Vec` allocations on the no-tool path reduced
to zero new heap allocations for an unchanged history; `cargo test -p
ragent-agent` green.

### Milestone C — Storage & I/O hygiene

Scope: P-1, P-2, P-3, P-10, P-11, P-12.

| Task | Finding | Files | Size |
|------|---------|-------|------|
| C-1  | P-1     | `processor.rs:386` (route `get_messages` through `storage_op`) | S |
| C-2  | P-2     | `loop_steps.rs:prepare_client` + `SessionManager` (config mtime cache) | M |
| C-3  | P-3     | `loop_steps.rs:build_turn_chat_messages` + `processor.rs:431` (return `context_window`) | S |
| C-4  | P-10    | `processor.rs:1053–1078` (short-circuit when no active spec / failed tool) | S |
| C-5  | P-11    | `processor.rs:1087` + `task::TaskManager` (`has_pending` flag) | S |
| C-6  | P-12    | `processor.rs:1113–1132` (hash `Value` directly, cache on `ToolCallState`) | M |

Exit criteria: no storage call on the async runtime except via `storage_op`;
config not re-read when mtime unchanged; interim-save hash does not
`to_string()` any `Value`.

### Milestone D — Event & tool-call throughput

Scope: P-4, P-15, P-16, P-19.

| Task | Finding | Files | Size |
|------|---------|-------|------|
| D-1  | P-4     | `processor.rs:491–500` (publish `ToolsSent` once, or `Arc<[String]>` payload) | S |
| D-2  | P-15    | `ragent-types/event/mod.rs` + `processor.rs` (`ToolCallBatch` event) | L |
| D-3  | P-16    | `ragent-types` `ContentPart::ToolResult` (`Arc<str>` content) | M |
| D-4  | P-19    | `processor.rs:1017–1020` (bounded preview scan) | S |

Exit criteria: `ToolsSent` no longer clones 111 strings per step; parallel
tool-call events are delivered in one batch; tool-result content is not
re-serialised into a `String` when an `Arc<str>` is available.

### Milestone E — Compression-path allocation

Scope: P-21, P-22, P-23, P-24.

| Task | Finding | Files | Size |
|------|---------|-------|------|
| E-1  | P-21    | `loop_steps.rs:840, 1045` (one `emergency_compress` call site) | S |
| E-2  | P-22    | `history.rs:history_to_chat_messages` (sync variant) | S |
| E-3  | P-23    | `loop_steps.rs:592–597` + `SessionStateCache` (cache token estimate) | M |
| E-4  | P-24    | `loop_steps.rs:514–553` + `cache.rs` (cache spec section) | M |

Exit criteria: compression estimate not re-run when history version unchanged;
spec section not re-rendered when spec version unchanged;
`history_to_chat_messages` has a sync path.

### Milestone F — Measurement & gating

Scope: hermetic benchmark suite + baseline report + `/perf` TUI panel.

| Task | Description | Files | Size |
|------|-------------|-------|------|
| F-1  | Implement `MockLlmClient` returning canned `StreamEvent` sequences | `crates/ragent-bench/src/mock.rs` | M |
| F-2  | Criterion bench `agent_loop` covering: TTFT, step latency, tool-call throughput | `crates/ragent-bench/benches/agent_loop.rs` | M |
| F-3  | Baseline report on reference machine | `docs/reports/agent_loop_perf_baseline.md` | S |
| F-4  | `/perf` TUI panel reading `AgentLoopProfiler::snapshot()` | `crates/ragent-tui/src/app/` | M |
| F-5  | CI guard: fail when bench median regresses > 10% vs baseline | `scripts/check-bench-regression.sh` | S |

Exit criteria: `cargo bench -p ragent-bench --bench agent_loop` runs hermetically;
baseline report committed; `/perf` panel renders per-scope timings; CI guard
wired into `pre-flight.sh`.

## Success Criteria

1. **Per-step latency:** ≥ 15% reduction in median step latency on the
   `agent_loop` benchmark vs the Milestone-F baseline, measured on a
   reference machine. — **Measurement harness in place (Milestone F); baseline
   captured in `docs/reports/agent_loop_perf_baseline.md`.**
2. **Per-step allocations:** zero new heap allocations on the no-tool path when
   the history is unchanged (Milestone B exit criterion). — **Met:
   `LoopState.chat_messages` is `Arc<Vec<ChatMessage>>` with `Arc::make_mut`;
   tool-definition byte sum, `ToolContext`, content-part buffers, and
   `text_buffer` are reused across steps.**
3. **Blocking I/O:** no `Storage` method called on the async runtime except via
   `storage_op` (Milestone C exit criterion). — **Met: both `get_messages`
   sites routed through `storage_op`; config loaded via mtime-keyed
   `CachedConfig` cache.**
4. **Profiler overhead:** `profiler.scope(...)` allocates zero `String`s when
   profiling is disabled (Milestone A exit criterion). — **Met: `scope`
   checks `is_enabled()` before `to_string()`; covered by 4 tests in
   `crates/ragent-agent/tests/test_profiler.rs`.**
5. **Tests:** `cargo check --workspace`, `cargo build --workspace --tests`, and
   the relevant crate test suites pass after every milestone. — **Met after
   each milestone; final pass: `cargo check --workspace` ✅,
   `cargo build --workspace --tests` ✅, ragent-agent (24/24), ragent-bench,
   ragent-tui, ragent-server all green.**
6. **No regressions:** event order unchanged; on-disk session format
   unchanged; public `SessionProcessor` API unchanged. — **Met:
   `ToolsSent` still published on step 1; `ToolCallBatch` added alongside
   per-call events as a fallback; session format and public API unchanged.**

## Risks

- **P-6 (`Arc<Vec<ChatMessage>>`)** touches the `LoopState` struct shared
  across `loop_steps.rs` and `processor.rs`; must update both atomically.
- **P-16 (`ContentPart::ToolResult`)** is in `ragent-types` and is consumed by
  every provider; widening the type is a workspace-wide change. Gate behind a
  milestone of its own if D-3 proves larger than expected.
- **P-15 (`ToolCallBatch`)** adds a new event variant; the TUI and HTTP server
  must be updated in lockstep. Keep the per-call events as a fallback until the
  TUI is proven on the batch variant.
- **P-2 (config mtime cache)** must invalidate when the user edits
  `ragent.json` mid-session; document the invalidation hook in
  `SessionManager`.

## Out of Scope

- Model routing, planning modes, new tools, new providers.
- Replacing the provider crates or the `StreamBuffer` streaming logic.
- Changing the public `SessionProcessor` API or the on-disk session format.
- Any item already marked implemented in `specs/AgentPerf/SPEC.md`
  (FR-006, FR-008, FR-009, FR-010, FR-016 and their tests).

## References

- `specs/AgentPerf/SPEC.md` — the implemented performance spec.
- `specs/AgentPerf/PLAN.md` — the original execution plan.
- `crates/ragent-agent/src/session/processor.rs` — the loop orchestrator.
- `crates/ragent-agent/src/session/loop_steps.rs` — the extracted step helpers.
- `crates/ragent-agent/src/session/profiler.rs` — `AgentLoopProfiler`.
- `crates/ragent-agent/src/session/cache.rs` — `SystemPromptCache`.
- `crates/ragent-agent/src/session/history.rs` — history↔chat conversion and
  byte-estimate helpers.
- `crates/ragent-types/src/event/mod.rs` — the `Event` enum.
- `docs/reports/agent_loop_perf_baseline.md` — to be produced in Milestone F.