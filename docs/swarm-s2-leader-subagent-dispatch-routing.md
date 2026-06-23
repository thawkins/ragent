# Leader → Subagent Dispatch and Result Routing — Audit

**Reviewer:** swarm-s2 (`tm-002`)
**Task ID:** s2
**Date:** 2026-06-21
**Scope:** Trace how the leader dispatches work to subagents (and teammates) and how results flow back. Identify correlation/routing defects, race conditions, dropped replies, fan-out/fan-in bugs, and lifecycle-signal propagation.
**Crates covered:** `ragent-agent`, `ragent-team`, `ragent-tui`, `ragent-server`, `ragent-types`
**Method:** Static analysis only — no code modifications, no test runs.

---

## 0. Executive Summary

The leader → subagent dispatch and result return paths in ragent are implemented in **two parallel stacks** that share the same conceptual design but have diverged in code:

1. **Team stack** (`ragent-team` crate + the `team/*` modules re-exported by `ragent-agent`).
   - Lead → teammate: `team_spawn` → `TeamManager::spawn_teammate_internal` creates an isolated child session and starts a background agent loop. Task assignment is via `tasks.json` (file-locked), and ad-hoc communication is via per-agent mailbox files (`mailbox/{agent-id}.json`).
   - Teammate → lead: the manager's per-teammate mailbox poll loop drains the teammate's mailbox, translates each `MailboxMessage` into an `Event::TeammateMessage` / `TeammateP2PMessage` / `TeammateIdle` and broadcasts on the lead's `EventBus`. Lifecycle ends are signalled by `Event::TeammateFailed` / `TeammateSpawned` / `TeammateSuspended` / `TeammateResumed` published from `TeamManager`.

2. **Task/sub-agent stack** (`ragent-agent` crate, `task/mod.rs`).
   - Lead → sub-agent: `new_task` (sync or background) → `TaskManager::spawn_sync` / `spawn_background` creates a child session and runs `processor.process_message()` against it. A `TaskEntry` is registered, indexed by `task_id`.
   - Sub-agent → lead: results are written to the `TaskEntry` map and `Event::SubagentComplete` / `SubagentCancelled` / `SubagentKilled` are published on the lead's `EventBus`. `wait_tasks` subscribes to the bus to await results.

3. **Orchestrator stack** (`ragent-agent` crate, `orchestrator/*`).
   - Lead → agent: `Coordinator::start_job_sync` / `start_job_first_success` / `start_job_async` dispatches via a pluggable `Router` trait (`InProcessRouter` uses mpsc + oneshot, `HttpRouter` uses HTTP POST). Selected by capability tags, not by ID.
   - Agent → coordinator: in-process returns the response on the oneshot; HTTP returns the JSON `result` field.

The two team stacks are nearly-duplicate code. There is no canonical schema, the duplicate implementations have already drifted, and critical event types (`TeamTaskClaimed` / `TeamTaskCompleted` / `IdleNotify`) are consumed but never published. Correlation IDs are weak: the agent loop only knows the teammate by `agent_id` ("tm-001") and the lead session ID; there is no per-task correlation token propagated across the mailbox, the in-process event bus, and the on-disk `tasks.json` updates.

The most serious problems are:

- **Dead events** (consumed but never published) — `Event::TeamTaskClaimed` / `TeamTaskCompleted` / `MessageType::IdleNotify` are defined and routed, but no tool or background task ever publishes them. TUI/TUI tests, `team_wait`, and `EventBus` subscribers never see them.
- **Lost "shutdown" signal** — the `team_shutdown_teammate` tool only pushes a mailbox `ShutdownRequest` and updates config status; it does **not** flip the agent loop's cancel flag, so a busy teammate continues running indefinitely.
- **Two-phase `TeamStore::save` race window** — `spawn_teammate_internal` writes config twice (once before child session is created, once after). Combined with no file locking on `config.json`, concurrent writers can clobber each other.
- **`current_task_id` is write-only on the on-disk side** — the `TeamMember` field is cleared on idle/shutdown but never set when a teammate claims a task, so any on-disk observer sees stale `None` even while a task is in flight.
- **`team_wait` ignores `TeammateFailed`** — a failed teammate blocks the lead until the 300s timeout instead of being unblocked by the failure event.
- **`Mailbox::push` in the `ragent-team` crate releases the flock before writing** — leads to silently-dropped messages under concurrent writes (the `ragent-agent` duplicate correctly holds the lock for the write).

---

## 1. Leader Dispatch Path (sequence)

### 1.1. Team path — `team_spawn` → `TeamManager::spawn_teammate_internal`

Source files:
- `crates/ragent-agent/src/tool/team_spawn.rs`
- `crates/ragent-agent/src/team/manager.rs` (and its `ragent-team` mirror at `crates/ragent-team/src/team/manager.rs`)
- `crates/ragent-team/src/team/store.rs` (for `TeamStore::save` / `load`)

Sequence:

1. **Lead calls `team_spawn`** (`crates/ragent-agent/src/tool/team_spawn.rs:75`).
   - Validates multi-item prompt heuristic (lines 100–201) via `PermissionRequested`/`PermissionReplied` events.
   - Parses `model` override (lines 202–211).
   - Parses `memory` scope (lines 213–218).
   - If `ctx.team_manager` is `None`, returns a `pending_manager` status — the member is **not** actually spawned, but the system prompt still asks the LLM to "call `team_wait` (not `wait_tasks`) after all spawns" (line 314). This is a known waiting-state mismatch: the lead is instructed to wait for teammates that were never created.

2. **`TeamManager::spawn_teammate`** is called (interface at `crates/ragent-agent/src/team/manager.rs:929`; implementation at `crates/ragent-team/src/team/manager.rs:106–167`). The `ragent-agent` version delegates to a `&self` interface; the `ragent-team` version calls `spawn_teammate_internal` directly.

3. **`spawn_teammate_internal`** (`crates/ragent-team/src/team/manager.rs:108–365`, and the near-duplicate at `crates/ragent-agent/src/team/manager.rs:455–706`):
   - `acquire spawn_lock` (serialises spawn ops).
   - **Load config #1**, allocate a `tm-NNN` `agent_id`, append a `TeamMember` with `status = Spawning` and `model_override`, save.
   - **Create child session** via `processor.session_manager.create_session(working_dir)` — produces a fresh `child_session.id`.
   - **Load config #2**, update the same member with `session_id = Some(child_sid)` and `status = Working`, save again.
   - Resolve the agent profile, build the team prompt addition, inject a `Memory` block if `memory_scope != None`.
   - Register a `Notify` in the global notifier map (so `Mailbox::push` can wake the poll loop).
   - Insert a `TeammateHandle { cancel, poll_cancel, notify }` keyed by `agent_id` into `self.handles`.
   - **`tokio::spawn` agent loop** running `processor.process_message(child_sid, prompt, agent, cancel)` with up to 3 retries on transient failure.
   - **`tokio::spawn` mailbox poll loop** (`start_poll_loop`) — drains `mailbox/{agent_id}.json` on push-notify or 500ms tick.
   - Publish `Event::TeammateSpawned` with `agent_id`.
   - Return `agent_id` to the calling tool.

4. **Reconcile path** (`crates/ragent-agent/src/team/manager.rs:359–454`):
   - Run in a background `tokio::spawn`.
   - 10 retry attempts to catch members written after `TeamManager` was created.
   - Filters: `status == Spawning`, `session_id.is_none()`, not already in `handles`.
   - Uses `spawn_prompt.clone().unwrap_or_default()` (line 460). If the on-disk member does not have a `spawn_prompt` persisted, the teammate receives an **empty prompt** and will run `process_message("")` against the LLM.
   - Calls `spawn_teammate_internal` per member.

5. **Reconcile has its own first-write race**: The `team_create` blueprint seeding (`crates/ragent-agent/src/tool/team_create.rs:155–447`) may persist `Spawning` members to `config.json` before `TeamManager` is wired in; the reconcile loop then picks them up. The 10× retry is an explicit workaround for this ordering race.

### 1.2. Task path — `new_task` → `TaskManager::spawn_*`

Source files:
- `crates/ragent-agent/src/tool/new_task.rs`
- `crates/ragent-agent/src/task/mod.rs`

Sequence:

1. **Lead calls `new_task`** (`crates/ragent-agent/src/tool/new_task.rs:89`).
   - If `team_context.is_some()`, refuses and emits guidance to use team tools (lines 90–119). This is an **enforced routing boundary** between the two stacks: a session with team context cannot use `new_task`.
   - Parses `agent`, `task`, `background`, `model`.

2. **Sync path** (`TaskManager::spawn_sync`, `crates/ragent-agent/src/task/mod.rs:197–322`):
   - Generate `task_id = "{sanitize(agent_name)}-{8 hex}"` (e.g. `explore-a1b2c3d4`).
   - Create isolated child session via `processor.session_manager.create_session(working_dir)`.
   - Register `TaskEntry { id, parent_session_id, child_session_id, agent_name, task_prompt, background:false, status:Running, ... }` in the in-memory map.
   - Allocate an `Arc<AtomicBool>` cancel flag in `cancel_flags`.
   - Publish `Event::SubagentStart { session_id: parent_session_id, task_id, child_session_id, agent, task, background:false }`.
   - `run_subagent(child_sid, agent_name, task_prompt, model, cancel, working_dir)` calls `processor.process_message(child_sid, task_prompt, agent, cancel)` and returns the final `Message.text_content()`.
   - On success: mark `Completed`, populate `result`, publish `Event::SubagentComplete { success:true, summary, duration_ms }`, return `TaskResult { entry, response }`.
   - On error: mark `Failed` (or `Cancelled` if `error_msg.contains("cancelled")`), publish `Event::SubagentComplete { success:false, summary: "Error: ...", duration_ms }` OR `Event::SubagentCancelled`.

3. **Background path** (`TaskManager::spawn_background`, `crates/ragent-agent/src/task/mod.rs:330–531`):
   - Enforce `max_background` (default 8).
   - Same `task_id` and `TaskEntry` creation.
   - Publish `SubagentStart { background:true }`.
   - `tokio::spawn` a closure that:
     - Resolves the agent profile (`resolve_agent_with_customs`).
     - Applies model override.
     - Runs `processor.process_message(csid, prompt, agent, cancel)`.
     - On success: `TaskStatus::Completed`, `Event::SubagentComplete { success:true, ... }`.
     - On error: detects "cancelled" in the error string; if so `TaskStatus::Cancelled` + `Event::SubagentCancelled`; otherwise `Failed` + `SubagentComplete { success:false }`.

   **Important fan-out detail:** The error/cancelled detection in background (`mod.rs:496`) is done by **substring match on the error message**: `if error_msg.contains("cancelled")`. Any other error message that happens to contain the word "cancelled" will be misclassified. This is a brittle routing decision.

4. **Result injection into the parent** (`crates/ragent-agent/src/session/processor.rs:2649–2702`):
   - At the end of each agent loop iteration, the processor calls `task_manager.drain_completed(parent_session_id)`.
   - For each completed task, it appends a synthetic `ContentPart::Text` to the next user message, of the form `"[Background Task {status}: {agent_name} — {short_id}]\n\n{body}"`.
   - The TUI also subscribes to `Event::SubagentComplete` and surfaces results in the chat log.

5. **Cancelling** (`TaskManager::cancel_task` / `kill_task`, `crates/ragent-agent/src/task/mod.rs:534–661`):
   - `cancel_task`: sets the cancel flag (cooperative). Emits a `tracing::info` but **does not** publish any event.
   - `kill_task`: sets cancel + kill flags, publishes `Event::SubagentKilled { force:false }`, then spawns a 10-second escalation task that force-sets the cancel flag again.
   - The cancel_tool (`crates/ragent-agent/src/tool/cancel_task.rs:84`) wraps this and verifies ownership (`entry.parent_session_id == ctx.session_id`).

### 1.3. Orchestrator path — `Coordinator::start_job_*`

Source files:
- `crates/ragent-agent/src/orchestrator/coordinator.rs`
- `crates/ragent-agent/src/orchestrator/router.rs`
- `crates/ragent-agent/src/orchestrator/transport.rs`

Sequence:

1. **Call into `Coordinator`** with a `JobDescriptor { id, required_capabilities, payload }`.
2. `AgentRegistry::match_agents(required_capabilities)` returns all `AgentEntry`s whose `capabilities` contain *each* required tag (substring match — `registry.rs:150–162`). Order is insertion order.
3. **Sync** (`start_job_sync`, `coordinator.rs:213–299`):
   - Spawn one `tokio::spawn` per matched agent; each calls `router.send(agent_id, msg)` and stores the result in a `Vec<JoinHandle<Result<(String, String)>>>`.
   - Collects responses in order. Tracks `responses` and applies the conflict resolver (default: concatenate with `--- agent: {id} ---` separators).
4. **First-success** (`start_job_first_success`, `coordinator.rs:309–366`): serial — tries agents in order, returns the first response that does not start with the literal `"error:"` (case-insensitive, `coordinator.rs:335`).
5. **Async** (`start_job_async`, `coordinator.rs:371–480`):
   - Creates a `broadcast::channel::<JobEvent>(16)` for the job.
   - Spawns the work loop in a background task; assigns subtasks to each matched agent; collects responses into `parts`; updates the `JobEntry { status:"completed", result }`; emits `JobEvent::JobCompleted { success: true }` **unconditionally** (see defect D-13 below).

Correlation: the only handle returned to the caller is `job_id` (a `String`). The `JobEntry` is stored in `self.jobs: DashMap<String, JobEntry>`. **No agent_id is associated with the job** at the `Coordinator` level — when subtasks complete, the result is just appended to `parts`; the caller cannot tell which agent produced which part without re-parsing the result text.

Router variants:
- `InProcessRouter::send` (`router.rs:35–60`): looks up `AgentEntry`, takes its `mailbox: mpsc::Sender<OrchestrationRequest>`, sends an `OrchestrationRequest { job_id, payload, reply: oneshot::Sender<String> }`, awaits the reply with a `timeout(5s)`. Per-agent mailbox is `mpsc::channel(100)`. **No retry.** On timeout, returns `Err("request to agent timed out")` (and this string contains the substring "timed out", which is what `coordinator.rs:257` keys off of to increment the `timeouts` metric).
- `HttpRouter::send` (`transport.rs:117–170`): POSTs `{ job_id, payload }` to the agent's `endpoint_url`; expects `{ result: "…" }`. Times out at `request_timeout`.
- `RouterComposite::send` (`transport.rs:186–196`): tries routers in order; returns the first `Ok(_)`; the **first `Err` short-circuits** and is propagated (no fall-through on error).

---

## 2. Result Return Path

### 2.1. Team mailbox → lead event bus

Source: `crates/ragent-agent/src/team/manager.rs:716–767` (and the duplicate in `ragent-team`).

For each teammate:

1. A `tokio::spawn` runs `start_poll_loop(agent_id, cancel, notify)`.
2. Loop body:
   - Check `cancel`. If true, break.
   - `tokio::select!` on `notify.notified()` and `tokio::time::sleep(500ms)`.
   - `Mailbox::open(team_dir, agent_id).drain_unread()` — atomically returns all unread and marks them as read (uses `fs2` flock + `serde_json` round-trip).
   - For each drained message, call `publish_message_event(event_bus, lead_session_id, team_name, agent_id, msg)`.
3. `publish_message_event` (`crates/ragent-agent/src/team/manager.rs:955–991`):
   - Preview = first 200 chars of `msg.content`.
   - Match `msg.message_type`:
     - `IdleNotify` → `Event::TeammateIdle { session_id: lead, team_name, agent_id: msg.from }`.
     - `from != "lead" && to != "lead"` (P2P) → `Event::TeammateP2PMessage { ..., from: msg.from, to: msg.to, preview }`.
     - else → `Event::TeammateMessage { ..., from: msg.from, to: msg.to, preview }`.
4. Subscribers (TUI, `team_wait`, `team_status` HTTP endpoint via SSE) receive the event.

### 2.2. Idle / shutdown / failure signalling

- **Idle** — published from two places, neither reliable:
  - `TeamManager::spawn_teammate_internal`'s background agent loop (`crates/ragent-agent/src/team/manager.rs:628–632`) when `process_message` returns `Ok` (i.e. the agent finished its initial prompt).
  - `publish_message_event` when a `MessageType::IdleNotify` arrives in the mailbox — but **no tool ever pushes `IdleNotify`** (see defect D-1).
- **Failure** — published from `spawn_teammate_internal`'s outer error path (`crates/ragent-agent/src/team/manager.rs:686–691`) after `MAX_RETRIES = 3` retries or on `is_permanent_llm_api_error`. The error is also persisted to `member.last_spawn_error`.
- **Shutdown request** (lead → teammate) — `team_shutdown_teammate` (`crates/ragent-agent/src/tool/team_shutdown_teammate.rs:49–100`): only writes `member.status = ShuttingDown` and pushes a `MessageType::ShutdownRequest` to the teammate's mailbox. It does **not** set the cancel flag, so the agent loop keeps running until the teammate happens to call `team_read_messages` (defect D-4).
- **Shutdown ack** (teammate → lead) — `team_shutdown_ack` (`crates/ragent-agent/src/tool/team_shutdown_ack.rs:41–85`): writes `member.status = Stopped`, then pushes a `MessageType::ShutdownAck` to the lead's mailbox at `mailbox/lead.json`. The poll loop will pick this up on its next tick and translate it to `Event::TeammateMessage { from, to:"lead" }` — there is **no dedicated `Event::TeammateShutdownAck`**.
- **Suspend / resume** — `TeamManager::suspend_teammate` (`manager.rs:777–799`) sets `poll_cancel=true` and writes `member.status = Suspended`, then publishes `Event::TeammateSuspended`. `resume_teammate` (lines 805–841) does the reverse and publishes `Event::TeammateResumed`. The agent loop itself is **not paused** — only the mailbox poll loop is, so the teammate can still send mail but won't see new lead messages.

### 2.3. Sub-agent (new_task) result path

Source: `crates/ragent-agent/src/task/mod.rs:756–787` (`run_subagent`) and `mod.rs:472–528` (background completion path).

- `run_subagent` calls `processor.process_message(child_sid, task_prompt, agent, cancel)` and returns `Ok(response_msg.text_content())`. There is no in-band correlation between the response and the `task_id` inside the response itself — the caller (either `spawn_sync` directly, or the closure in `spawn_background`) uses the closure's captured `tid` to look up the `TaskEntry` and write the result.
- `wait_tasks` (`crates/ragent-agent/src/tool/wait_tasks.rs:70–247`) subscribes to `Event::SubagentComplete` on the event bus; matches on `session_id == ctx.session_id` (parent session) **and** `task_id in waiting_for`; on match, stores the result. **Unrelated `SubagentComplete` events are passed over with `continue`**.
- `increment_waiter` / `decrement_waiter` (`task/mod.rs:730–755`) are used to prevent `drain_completed` from re-reporting a task that `wait_tasks` is already handling.
- The processor's own `drain_completed` (`session/processor.rs:2649–2702`) handles completion by injecting a synthetic `ContentPart::Text` into the next user message of the lead's chat.

### 2.4. Orchestrator result path

- **In-process** — `InProcessRouter::send` awaits the `oneshot::Receiver<String>`. The agent loop in `AgentRegistry::register` (`registry.rs:91–99`) calls the responder and `let _ = req.reply.send(resp)`; any send error is silently dropped (`best-effort: ignore send error`).
- **HTTP** — `HttpRouter::send` (`transport.rs:117–170`) issues `reqwest::Client::new().post(...)`; checks `resp.status().is_success()`; parses `RemoteAgentResponse { result: String }`. On HTTP error, returns `Err("HTTP {status}: {body}")`. The coordinator matches on `err_str.contains("timed out") || err_str.contains("timeout")` (`coordinator.rs:257, 348, 448`) — this is the only signal that the request failed because of a timeout; all other errors are bucketed as "error".

---

## 3. Correlation / Routing Primitives

There is **no consistent correlation-ID scheme** end-to-end.

| Path | ID used | Propagated? | Comments |
|------|---------|-------------|----------|
| Team mailbox | `MailboxMessage.message_id` (UUID v4) | yes (per-message) | File-stored, not correlated to any team task. |
| Team task | `task_id` (e.g. `task-001`) | yes (in `tasks.json`) | Generated by `TeamStore::next_task_id`. The mailbox message that "should" announce the claim doesn't exist (see D-1). |
| Team member | `agent_id` (e.g. `tm-001`) + `session_id` | yes (in `config.json` + `TeammateHandle`) | The agent loop's `child_session_id` is captured into `TeammateHandle._child_session_id` but **never used** to route replies — the only consumer is the `teammate_retry_backoff` test fixture pattern and the unused `_child_session_id` field is named with an underscore prefix. |
| Lead session | `lead_session_id` (String) | yes (in `config.json`, in events) | The lead's session ID is propagated to every team event; `team_wait` filters on it. |
| Sub-agent | `task_id` (e.g. `explore-a1b2c3d4`) | yes (in `TaskEntry` + event payload) | The `task_id` IS the correlation ID between `SubagentStart`, `SubagentComplete`, `cancel_task`, `wait_tasks`, and `drain_completed`. This is the cleanest path. |
| Orchestrator | `job_id` (UUID-like) | partial | `Coordinator::start_job_async` puts `job_id` in the `JobEntry`, but the per-agent responses are written as `"--- agent: {id} ---\n{resp}"` strings — no per-agent correlation in the result structure. |

Implications:

- The team path has **no way to know which `MailboxMessage` arrived in response to which `team_spawn` call or which `team_assign_task` call**. The mailbox is unordered, undirected, and any "context" is implicit in `msg.content` text.
- The `InProcessRouter` oneshot channel is a one-shot correlation (one router.send → one reply) and is sound for that purpose. But because `Coordinator::start_job_sync` spawns one task per matched agent, **all of them race for the same `JobEntry` and a single `JobCompleted` is emitted at the end**; the per-subtask `JobEvent::SubtaskCompleted` is published but never read by the TUI / HTTP layer (searched: no subscribers of `JobEvent`).
- `team_wait` (`crates/ragent-agent/src/tool/team_wait.rs:172–188`) only matches `Event::TeammateIdle`; it deliberately ignores `TeammateMessage`, `TeammateP2PMessage`, and `TeammateFailed` — a teammate sending a final report does not unblock `team_wait`; only an explicit idle does.

---

## 4. Race Conditions, Dropped Replies, Duplicate Handling, Fan-out/Fan-in Bugs

### 4.1. `team_wait` window between store read and event subscription

`team_wait` (`crates/ragent-agent/src/tool/team_wait.rs:79–158`):
1. Loads the team store (lines 79–90).
2. Builds the `waiting_for` set from member statuses (lines 95–125).
3. Pre-removes already-idle/done members (lines 127–142).
4. **Then** subscribes to the event bus (line 158).
5. Enters the wait loop.

If a teammate transitions from `Working` to `Idle` between steps 2 and 4, the `Event::TeammateIdle` is published before the subscription, so the lead never receives it. The teammate remains in `waiting_for` until the 300s timeout. (The comment at line 156 says "Subscribe BEFORE the wait loop to avoid the race" — but the subscription happens after the store read, so the race is still present.)

### 4.2. `team_wait` only matches `TeammateIdle`

`team_wait.rs:172–188`:
```rust
match tokio::time::timeout_at(deadline, rx.recv()).await {
    Ok(Ok(Event::TeammateIdle { ... })) if ... => { waiting_for.remove(&agent_id); }
    Ok(Ok(_)) => continue,  // ← TeammateFailed, TeammateMessage, etc. ignored
    ...
}
```

A teammate that publishes `Event::TeammateFailed` (defect D-2 in the file, but a real event) is ignored, and the lead blocks for up to `timeout_secs` (default 300s) waiting for an idle that will never arrive. There is no per-iteration check of `member.status` in the on-disk store.

### 4.3. `team_wait` includes `ShuttingDown` / `Suspended` / `Spawning` in wait filter

`team_wait.rs:111–119`:
```rust
matches!(
    m.status,
    MemberStatus::Working
        | MemberStatus::Spawning
        | MemberStatus::PlanPending
        | MemberStatus::ShuttingDown
        | MemberStatus::Suspended
)
```

A teammate stuck in `ShuttingDown` (because the `team_shutdown_teammate` tool doesn't actually cancel the agent loop — see D-4) is in the wait set. The lead will wait the full 300s default for a teammate that may never become idle.

### 4.4. `Mailbox::push` (ragent-team) TOCTOU race

`crates/ragent-team/src/team/mailbox.rs:188–215`:
```rust
file.lock_exclusive()?;
let mut messages: Vec<MailboxMessage> = ...;
messages.push(message);
file.unlock()?;            // ← LOCK RELEASED HERE
Self::write_atomic(&self.path, &messages)?;   // ← WRITE HAPPENS UNLOCKED
signal_notifier(...);
```

Between `unlock` and `write_atomic`, another writer (or drainer) can open the file, see the old content, append its own message, and write it back — then the first writer's `write_atomic` overwrites it. The same pattern exists in `drain_unread` (lines 218–252) and `mark_read` (lines 255–292).

The duplicate in `crates/ragent-agent/src/team/mailbox.rs:182–209` correctly holds the lock through the write via `write_locked` (uses `file.set_len(0); file.seek(Start(0)); file.write_all(...)`). The two implementations have already diverged, and only one of them is safe under concurrent access.

### 4.5. `TeamStore::save` has no file locking

`crates/ragent-team/src/team/store.rs:174–183`:
```rust
pub fn save(&self) -> Result<()> {
    let config_path = self.dir.join("config.json");
    let tmp_path = self.dir.join("config.json.tmp");
    let json = serde_json::to_string_pretty(&self.config)?;
    fs::write(&tmp_path, json)...;
    fs::rename(&tmp_path, &config_path)...;
    Ok(())
}
```

No `fs2` flock on the file. `TaskStore` and `Mailbox` both use `flock` (`fs2`); `TeamStore` does not. Two writers (e.g. the lead calling `team_assign_task` while a teammate's poll loop sets its own `status` to `Working` via the reconcile path) will each load → modify → save; the last `save` wins. Member status updates, task assignments, plan approvals, and `last_spawn_error` can all be silently lost.

### 4.6. Two-phase save race window in `spawn_teammate_internal`

`crates/ragent-team/src/team/manager.rs:521–568` (and the ragent-agent mirror at `manager.rs:467–516`):
1. Load config #1, append member with `Spawning`, save.
2. Create child session (no save).
3. Load config #2, update member with `session_id` + `Working`, save.

Between phases 1 and 3, another process can load the config, see a `Spawning` member with no `session_id`, and either:
- Issue a `team_message` to it (resolved by `name` via `member_by_name`) — works because the member is in the config.
- Call `team_status` and see a phantom member.
- **Worse, with D-5 in play**: save its own changes over the phase-1 save, deleting the `Spawning` member entirely before the agent loop gets a chance to write `session_id`.

### 4.7. `reconcile_spawning_members` uses empty prompt fallback

`crates/ragent-agent/src/team/manager.rs:460` (and the ragent-team mirror at `manager.rs:392`):
```rust
.map(|m| (m.name.clone(), m.agent_type.clone(), m.spawn_prompt.clone().unwrap_or_default(), m.model_override.clone()))
```

The blueprint seeding in `team_create` (lines 249–447) does **not** persist `spawn_prompt` to the `TeamMember` record (only the reconcile path itself writes `spawn_prompt`, and the blueprint path does not). A reconciled teammate whose prompt wasn't saved runs `process_message("")` against the LLM — a teammate with no instructions.

### 4.8. `current_task_id` is never set in the on-disk member

`TeamMember.current_task_id: Option<String>` is defined at `crates/ragent-team/src/team/config.rs:149` (and `crates/ragent-agent/src/team/config.rs:146`).

Places that **clear** it:
- `team_idle.rs:127` → `member.current_task_id = None;`
- `team_shutdown_ack.rs:60` → `member.current_task_id = None;`
- `TeamManager::spawn_teammate_internal`'s success path (`manager.rs:624`) → `m.current_task_id = None;`

Places that **set** it to `Some(...)`:
- **None in the ragent-agent and ragent-team crates.** It is set only by the TUI's local in-memory state from `Event::TeamTaskClaimed` (`crates/ragent-tui/src/app.rs:12795`). But `Event::TeamTaskClaimed` is **never published** by any team tool (searched the entire repo with `grep -rn "Event::TeamTaskClaimed\|Event::TeamTaskCompleted" crates/` — the only matches are subscribers in TUI, SSE translation in ragent-server, and the EventBus enum itself).

Net effect: any observer reading the on-disk `config.json` (`team_status` tool, `team_status` HTTP endpoint, `team_list_members`, etc.) sees `current_task_id = None` for all teammates, even while a task is in flight. The field is functionally write-only on disk.

### 4.9. `Event::TeamTaskClaimed` and `Event::TeamTaskCompleted` are dead-code events

`crates/ragent-types/src/event/mod.rs:520–541` defines both events. They are translated to SSE in `crates/ragent-server/src/sse.rs:688–704` and handled in the TUI (`crates/ragent-tui/src/app.rs:12783, 12802`). **No code publishes them.** `team_task_claim` (`crates/ragent-agent/src/tool/team_task_claim.rs:47–208`) updates `tasks.json` but does not emit any event. `team_task_complete` (`crates/ragent-agent/src/tool/team_task_complete.rs:56–183`) does not emit any event either.

The TUI's `current_task_id` tracker at `app.rs:12795` therefore never updates from on-disk state, and the HTTP `/sse` stream never includes `team_task_claimed` / `team_task_completed` events.

### 4.10. `MessageType::IdleNotify` is defined but never sent

`crates/ragent-team/src/team/mailbox.rs:41` and `crates/ragent-agent/src/team/mailbox.rs:41` define `MessageType::IdleNotify`. `publish_message_event` handles it by emitting `Event::TeammateIdle` (manager.rs:1081 in ragent-team, manager.rs:965 in ragent-agent). But `team_idle` (`crates/ragent-agent/src/tool/team_idle.rs`) does **not** push an `IdleNotify` to the lead's mailbox — it just writes `member.status = Idle` to `config.json`. The lead only learns about idle through the agent-loop-finished path in `spawn_teammate_internal`, which fires once and only after the initial prompt completes.

### 4.11. `team_shutdown_teammate` does not cancel the agent loop

`crates/ragent-agent/src/tool/team_shutdown_teammate.rs:49–100`:
- Sets `member.status = ShuttingDown`.
- Pushes a `MessageType::ShutdownRequest` to the teammate's mailbox.
- Returns success.

It does **not**:
- Call `TeamManager::shutdown_teammate()` (the manager would set the cancel flag and deregister the notifier).
- Set the `cancel` flag on the `TeammateHandle`.
- Set the `poll_cancel` flag.
- Deregister the notifier.

The teammate only learns about shutdown if it happens to call `team_read_messages` between tool calls. If the teammate is mid-`process_message` (which can take many minutes for large agent loops), the `ShutdownRequest` sits unread in the mailbox, and the agent loop continues. Combined with D-3, the lead waiting via `team_wait` will block the full timeout.

### 4.12. Two divergent shutdown paths

`team_shutdown_teammate` (tool): graceful, sets `ShuttingDown`, no flag.
`TeamManager::shutdown_teammate` (manager, `crates/ragent-team/src/team/manager.rs:908–939` and the ragent-agent mirror at `manager.rs:848–879`): immediate, sets cancel + poll_cancel, deregisters notifier, pushes a `ShutdownRequest` mailbox message, sets `member.status = Stopped`.

Calling the tool produces different runtime behaviour from calling the manager. The TUI's `/shutdown` slash command and the `team_shutdown_teammate` tool use the tool path; nothing exposes the manager path externally.

### 4.13. `Coordinator::start_job_async` always reports `success: true`

`crates/ragent-agent/src/orchestrator/coordinator.rs:462–477`:
```rust
let result = parts.join("\n");
if let Some(mut j) = jobs.get_mut(&job_id_for_spawn) {
    j.status = "completed".to_string();
    j.result = Some(result.clone());
}
let _ = tx.send(JobEvent::JobCompleted {
    job_id: job_id_for_spawn.clone(),
    success: true,   // ← unconditional
});
```

If all matched agents failed, `parts` is empty, but `JobCompleted { success: true }` is still emitted. There is no `JobFailed` event published in this path. Subscribers that key off `success` will silently see "success" for a job that produced no output.

### 4.14. `team_broadcast` partial failure loses success state

`crates/ragent-agent/src/tool/team_broadcast.rs:71–81`:
```rust
for agent_id in &active {
    let mailbox = Mailbox::open(&team_dir, agent_id)?;
    mailbox.push(...)?;
    sent += 1;
}
```

`?` propagates the first error. Messages already sent (up to the failing iteration) are not reported. The lead sees only the first failure; downstream tools can't tell which recipients got the broadcast.

### 4.15. `InProcessRouter::send` has no retry and silently drops late replies

`crates/ragent-agent/src/orchestrator/router.rs:35–60`:
- Single `tx.send(req).await` — if the mailbox is full (capacity 100, `registry.rs:87`), returns `Err("failed to send to agent mailbox")`.
- Single `timeout(5s, reply_rx)` — on timeout, returns `Err("request to agent timed out")`.
- The agent-side loop in `registry.rs:91–99` does `let _ = req.reply.send(resp)` — a oneshot send error is silently dropped. A reply that arrives after the router has timed out is lost without trace.

### 4.16. `RouterComposite` first-error short-circuits

`crates/ragent-agent/src/orchestrator/transport.rs:186–196`:
```rust
for router in &self.routers {
    if let Ok(resp) = router.send(agent_id, msg.clone()).await {
        return Ok(resp);
    }
}
Err(anyhow!("all routers failed"))
```

The first router that returns `Err` is treated as "try the next" only if it returns `Ok` — but `?` operators elsewhere propagate errors. The intent appears to be "first success wins", and that works. But because `InProcessRouter::send` is deterministic and the `HttpRouter` is the fallback, the first `Err` from the in-process router (e.g. "agent not found") is silently swallowed and the next router is tried. This is the desired behaviour but is undocumented; if someone wires routers in the wrong order they will see mysterious successes for un-routed IDs.

### 4.17. `team_assign_task` does not notify the assigned teammate

`crates/ragent-agent/src/tool/team_assign_task.rs:48–99` updates `tasks.json` and persists via `task_store.update_task`. It does **not**:
- Push a mailbox message to the assigned teammate.
- Publish an event.
- Wake the teammate's poll loop.

The teammate will only learn of the assignment the next time it calls `team_task_claim` or `team_read_messages`. If the teammate is mid-task and the new assignment is intended to interrupt, the interrupt never arrives.

### 4.18. `team_idle` hook rejection leaves state inconsistent

`crates/ragent-agent/src/tool/team_idle.rs:96–120`:
- Runs `TeammateIdle` hook with the `summary` on stdin.
- If hook returns `Feedback(feedback)`, sets `member.status = Working` again.
- Returns `HookOutcome::Feedback` content as a tool result.

But:
- `member.current_task_id` was never cleared (because the idle path didn't actually transition).
- If the hook was intended to keep the teammate working, the teammate's LLM loop has already decided to be idle — the only "kept working" enforcement is a single config write.
- No event is published, so the lead's `team_wait` (if waiting) does not unblock.

### 4.19. `team_approve_plan` rejection leaves `plan_status = Rejected` but `status = PlanPending`

`crates/ragent-agent/src/tool/team_approve_plan.rs:85–98`:
- On rejection, sets `member.plan_status = Rejected`, keeps `member.status = PlanPending`.
- Sends `MessageType::PlanRejected` to the teammate.

`TeamManager::is_plan_pending()` (mirror at `crates/ragent-agent/src/team/manager.rs:914–928`) checks `plan_status == PlanStatus::Pending`, not `status == MemberStatus::PlanPending`. After rejection, `is_plan_pending()` returns **false** — any plan-gated tool checks in the agent loop would not block, even though the teammate is still in `PlanPending` per the config.

### 4.20. Fan-out/fan-in at the inbox level: `drain_unread` and `mark_read` race

`crates/ragent-agent/src/team/mailbox.rs:212–245` (`drain_unread`):
- Acquires exclusive lock.
- Reads all messages.
- Marks all unread as read.
- Writes back via `write_locked` (which uses `set_len(0)` + `write_all`).
- Releases lock.

This is correct **in this implementation**, but the ragent-team mirror at `mailbox.rs:218–252` is not (see D-4). If a process running ragent-agent code and a process running ragent-team code (e.g. the TUI launches a child ragent-team binary, or a future HTTP server imports ragent-team) operate on the same mailbox file, the ragent-team writer can overwrite the ragent-agent writer's drain.

### 4.21. `team_message` does not validate recipient state

`crates/ragent-agent/src/tool/team_message.rs:47–91`:
- Resolves `to` via `member_by_name` (or accepts `"lead"` / `"tm-XXX"`).
- Pushes the message to the resolved mailbox.
- Returns success.

No check that the recipient's `status` is one of `{Working, Idle, PlanPending, Spawning}`. Messages to `Stopped` or `Failed` teammates are silently delivered to a mailbox that will never be drained.

### 4.22. `process_message` in the teammate loop captures cancel by value

`crates/ragent-agent/src/team/manager.rs:605–611` (and the ragent-team mirror at `manager.rs:605–611`):
```rust
match proc.process_message(
    &child_sid_clone,
    &prompt_owned,
    &agent_clone,
    cancel_clone.clone(),     // ← cloned on every retry
).await
```

`cancel` is cloned on every retry attempt. The `cancel` field of `TeammateHandle` is held in `handles: Arc<RwLock<HashMap<String, TeammateHandle>>>`. When `shutdown_teammate` sets `handle.cancel.store(true)`, the background loop has its **own clone** of the `Arc<AtomicBool>`. Both copies refer to the same atomic, so setting it on one is observed by the other. This part is correct.

But: the `agent_clone` is captured by value (`&agent_clone`) — that's fine. The `child_sid_clone` is `String`, captured by value. These are per-attempt. The `process_message` itself does not know which `attempt` it is on; retries are external.

`process_message` runs the full agent loop. The cancel flag is checked by the inner agent loop (`session/processor.rs:1184–1185`:
> per `process_user_message` call. The flag is reset on every new turn because each turn starts a fresh `process_user_message`.)

So a cancel that arrives mid-agent-loop will not abort the current LLM call — only the next iteration of the outer loop. This is documented and intentional, but means a cancel during a long LLM inference can take 30–60s to take effect. Not a routing bug; mentioned for completeness.

### 4.23. `InProcessRouter` mailbox capacity is unbounded for replies

`registry.rs:87`: `mpsc::channel::<OrchestrationRequest>(100)`. Capacity 100. If a registered agent's responder is slow, the inbox fills, and `router.send(agent_id, msg)` will get `SendError` → returns `Err("failed to send to agent mailbox")`. No backpressure to the lead; the lead sees a generic error and has no way to retry only the failed agent in a `start_job_sync` (the `?` returns the first error from `h.await?` — but actually `h.await?` propagates the error and the loop continues for other agents, so the result list will simply be missing responses for the failed agent. The lead sees a partial result with no per-agent error attribution).

### 4.24. `SubagentStart` / `SubagentComplete` carry `parent_session_id` not `lead_session_id`

For teams, all team events carry `session_id: lead_session_id`. For sub-agents, all sub-agent events carry `session_id: parent_session_id`. These are the same lead session in normal use. But if a teammate (which is itself a session) calls `new_task` (which is forbidden by the `team_context` check), the events would carry the teammate's `session_id`, not the lead's. This is consistent with the design but worth flagging for the cross-stack correlation analysis.

### 4.25. `task_complete` tool shadow name

`crates/ragent-agent/src/tool/task_complete.rs` exists alongside `crates/ragent-agent/src/tool/team_task_complete.rs`. The TUI's completion message at `team_task_complete.rs:23–27` explicitly warns about the name collision:
> ⚠️ DO NOT confuse with `task_complete` (a different tool used OUTSIDE teams …)

The tool registry may register both. There is no central guard. The two tools are routed by the LLM; mistakes are possible.

### 4.26. `team_wait` cancels teammates on session_id mismatch only

`team_wait.rs:177`:
```rust
session_id == ctx.session_id
```

This filters out idle events from a previous lead session. But if the lead itself was migrated (e.g. session was reparented in `resolve_team_context_for_session`), the new session ID won't match and the lead will wait the full timeout. There is no log line for the rejected idle event.

### 4.27. `team_spawn` with `pending_manager` returns metadata but the LLM has already been told to `team_wait`

`team_spawn.rs:226–242`:
```rust
if ctx.team_manager.is_none() {
    return Ok(ToolOutput {
        content: format!("Teammate '{}' queued for team '{}' ...", ...),
        metadata: Some(json!({... "status": "pending_manager" })),
    });
}
```

The tool returns success. The LLM has been instructed (line 314) to call `team_wait` after spawning. `team_wait` will then look for members in `Spawning`/`Working`/etc. — but the pending manager member is `Spawning` in `config.json` (because `team_create` blueprint seeding persists it), so `team_wait` will wait for a teammate that will never become idle (no `TeamManager` exists to drive the lifecycle). The lead blocks the full 300s timeout.

### 4.28. Two implementations of `Mailbox` and `MailboxMessage` have drifted

The two crates ship near-identical `Mailbox` and `MailboxMessage` types:
- `crates/ragent-agent/src/team/mailbox.rs` (uses `write_locked` — file-locked, safe).
- `crates/ragent-team/src/team/mailbox.rs` (uses `write_atomic` — file unlocked, racy).

They have already diverged in 3 places: write strategy, drain implementation, and the agent-side `_agent_id` parameter in `publish_message_event` is `_`-prefixed (unused) in the ragent-agent mirror. They are not aliases. The `ragent-team` crate imports its own mailbox module (line 33); the `ragent-agent` crate uses its own (manager.rs:35).

The `ragent-team` crate declares a dependency on `ragent-agent` (Cargo.toml:24) but re-implements rather than re-exports the mailbox. A future change to either will not affect the other.

---

## 5. Defects Found

> Format: `D-{N} | severity | title | file:line`.

### Dispatch / routing

| ID | Severity | Title | Location |
|----|----------|-------|----------|
| D-1 | Critical | `Event::TeamTaskClaimed` and `Event::TeamTaskCompleted` defined and consumed but never published | `crates/ragent-types/src/event/mod.rs:520–541` (definitions); publishers expected at `crates/ragent-agent/src/tool/team_task_claim.rs:142–205` and `crates/ragent-agent/src/tool/team_task_complete.rs:104–132` (none). |
| D-2 | High | `MessageType::IdleNotify` is defined and translated to `Event::TeammateIdle` but never sent by `team_idle` | `crates/ragent-agent/src/tool/team_idle.rs:122–145` (no push); handler at `crates/ragent-agent/src/team/manager.rs:965–970`. |
| D-3 | High | `team_shutdown_teammate` does not cancel the agent loop; relies on cooperative mailbox check | `crates/ragent-agent/src/tool/team_shutdown_teammate.rs:49–100`; compare with `crates/ragent-team/src/team/manager.rs:908–939` (which does cancel). |
| D-4 | High | `current_task_id` is cleared on idle/shutdown but never set on claim | `crates/ragent-team/src/team/config.rs:149` (field); TUI sets it from dead event at `crates/ragent-tui/src/app.rs:12795`; the canonical claim site at `crates/ragent-agent/src/tool/team_task_claim.rs:142–205` does not. |
| D-5 | High | `Coordinator::start_job_async` always reports `success: true` even when all agents fail | `crates/ragent-agent/src/orchestrator/coordinator.rs:462–477`. |
| D-6 | High | `InProcessRouter` mailbox send / reply are not retried; late replies are silently dropped | `crates/ragent-agent/src/orchestrator/router.rs:35–60`; agent side `registry.rs:91–99`. |
| D-7 | High | Two divergent implementations of `Mailbox` have already drifted (ragent-team uses unlocked write; ragent-agent uses locked write) | `crates/ragent-team/src/team/mailbox.rs:188–215` vs `crates/ragent-agent/src/team/mailbox.rs:182–209`. |
| D-8 | High | Two divergent shutdown paths (tool vs manager) produce different runtime semantics | `crates/ragent-agent/src/tool/team_shutdown_teammate.rs:49–100` vs `crates/ragent-team/src/team/manager.rs:908–939`. |
| D-9 | High | `team_wait` ignores `Event::TeammateFailed`; lead blocks full timeout for failed teammates | `crates/ragent-agent/src/tool/team_wait.rs:172–188`. |
| D-10 | High | `team_wait` subscribes AFTER reading the team store, leaving a window where a teammate becoming idle is missed | `crates/ragent-agent/src/tool/team_wait.rs:79–158`. |
| D-11 | High | `team_wait` includes `ShuttingDown` / `Suspended` / `Spawning` in the wait filter; stuck teammates block the lead | `crates/ragent-agent/src/tool/team_wait.rs:111–119`. |
| D-12 | High | `Mailbox::push` (ragent-team) releases `flock` before `write_atomic` — concurrent writers can lose messages | `crates/ragent-team/src/team/mailbox.rs:188–215` (push), 218–252 (drain), 255–292 (mark_read). |
| D-13 | High | `TeamStore::save` has no file lock — concurrent config writes lose data | `crates/ragent-team/src/team/store.rs:174–183` (save) vs `TaskStore` and `Mailbox` which use `fs2`. |
| D-14 | High | Two-phase save in `spawn_teammate_internal` leaves a window where the member exists with no `session_id` | `crates/ragent-team/src/team/manager.rs:521–568`; ragent-agent mirror at `crates/ragent-agent/src/team/manager.rs:467–516`. |
| D-15 | High | `reconcile_spawning_members` uses empty prompt fallback if `spawn_prompt` is missing | `crates/ragent-agent/src/team/manager.rs:460`. |
| D-16 | Medium | `team_assign_task` does not notify the assigned teammate (no mailbox push, no event) | `crates/ragent-agent/src/tool/team_assign_task.rs:48–99`. |
| D-17 | Medium | `team_message` does not validate recipient is active; messages to `Stopped`/`Failed` teammates are silently dropped on receipt | `crates/ragent-agent/src/tool/team_message.rs:47–91`. |
| D-18 | Medium | `team_broadcast` short-circuits on first error and does not report partial delivery | `crates/ragent-agent/src/tool/team_broadcast.rs:71–81`. |
| D-19 | Medium | `team_idle` does not publish `Event::TeammateIdle`; lead learns idle only via the agent-loop-finished path | `crates/ragent-agent/src/tool/team_idle.rs:122–145`; see also D-2. |
| D-20 | Medium | `team_approve_plan` rejection sets `plan_status = Rejected` while leaving `status = PlanPending`; `is_plan_pending()` returns false and gating breaks | `crates/ragent-agent/src/tool/team_approve_plan.rs:85–98`; check at `crates/ragent-agent/src/team/manager.rs:914–928`. |
| D-21 | Medium | `team_idle` hook rejection sets `status = Working` but does not clear `current_task_id` (already None — see D-4) and does not re-publish any event | `crates/ragent-agent/src/tool/team_idle.rs:98–120`. |
| D-22 | Medium | `team_spawn` with `pending_manager` returns success; the LLM has already been told to `team_wait`, which will block for a teammate that will never be created | `crates/ragent-agent/src/tool/team_spawn.rs:221–242`; system-prompt injection at `crates/ragent-team/src/team/manager.rs:80–84`. |
| D-23 | Medium | `cancel_task` does not publish any event; the lead's `wait_tasks` does not get a "cancelled" notification (it relies on the error string containing "cancelled") | `crates/ragent-agent/src/task/mod.rs:534–543` (cancel_task) vs `mod.rs:511–525` (classification by `error_msg.contains("cancelled")`). |
| D-24 | Medium | `InProcessRouter` `mpsc` capacity of 100 silently drops send failures (no backpressure) | `crates/ragent-agent/src/orchestrator/registry.rs:87`; router at `router.rs:35–60`. |
| D-25 | Medium | `RouterComposite` first-error short-circuit is undocumented; mis-ordered routers can produce surprising successes | `crates/ragent-agent/src/orchestrator/transport.rs:186–196`. |
| D-26 | Medium | `Coordinator::start_job_async` does not track which agent produced which response in its `JobEntry.result`; per-subtask `JobEvent::SubtaskCompleted` is published but never subscribed to | `crates/ragent-agent/src/orchestrator/coordinator.rs:420–460`. |
| D-27 | Low | `resolve_team_context_for_session` iterates all teams and reads each `config.json` on every lead message — no caching | `crates/ragent-agent/src/session/processor.rs:3087–3119`; call site at `processor.rs:933`. |
| D-28 | Low | `Mailbox::write_atomic` (ragent-team) uses predictable temp path `mailbox/{id}.json.tmp`; concurrent writers without the flock (D-12) would clobber | `crates/ragent-team/src/team/mailbox.rs:176–182`. |
| D-29 | Low | `team_spawn` task pre-assignment failure is non-blocking (just logs a warning) | `crates/ragent-agent/src/tool/team_spawn.rs:265–287`. |
| D-30 | Low | `TaskStore::complete` auto-claims an unclaimed task if status is `Pending` and `assigned_to` is `None` — surprising side effect | `crates/ragent-agent/src/team/task.rs:373–384`. |
| D-31 | Low | `TaskStore::add_task` comment claims the lead is the only writer, but the implementation acquires an exclusive lock | `crates/ragent-agent/src/team/task.rs:395–406` (comment vs impl). |
| D-32 | Low | `mailbox/{lead}.json` and `mailbox/{tm-N}.json` share a single namespace; `to: "lead"` and `to: "tm-001"` are the only routing key | `crates/ragent-agent/src/team/mailbox.rs:144–154` (path derivation); no global "outbox" concept. |
| D-33 | Low | `team_message` and `team_broadcast` do not include a `request_id` / correlation token in `MailboxMessage`; replies cannot be matched to a request | `crates/ragent-agent/src/team/mailbox.rs:51–68` (no request_id field). |
| D-34 | Low | `MailboxMessage.sent_at` is written by the sender but not validated by the receiver; clock skew between senders is not handled | `crates/ragent-agent/src/team/mailbox.rs:65`. |
| D-35 | Low | `Event::TeammateMessage` collapses all message types (except `IdleNotify` and P2P) into a single preview event with no semantic type — receivers cannot distinguish `PlanRequest` from `PlanApproved` from `ShutdownAck` from a free-form `Message` | `crates/ragent-agent/src/team/manager.rs:981–990`. |
| D-36 | Low | `team_spawn` spawns a `tokio::spawn` per teammate but does not track the join handle; on manager drop, in-flight agent loops are detached | `crates/ragent-agent/src/team/manager.rs:588–692`. |
| D-37 | Low | `drain_completed` and `wait_tasks` both consume `SubagentComplete` events; the `waiter_count` field prevents double-handling but is racy if incremented/decremented across the bus | `crates/ragent-agent/src/task/mod.rs:730–755`; consumer at `processor.rs:2649–2702`. |
| D-38 | Low | `team_spawn` returns `agent_id` in tool metadata, but if the LLM passes the teammate NAME to `team_message` (rather than the agent_id), resolution depends on `member_by_name` matching — duplicate names cause silent misrouting | `crates/ragent-agent/src/tool/team_message.rs:96–107` (resolve_agent_id). |
| D-39 | Low | `Coordinator::start_job_sync` collects per-agent handles in a `Vec<JoinHandle<...>>` and awaits them in order — a slow agent at the end of the vec blocks the entire job even if earlier agents are done | `crates/ragent-agent/src/orchestrator/coordinator.rs:248–268`. |
| D-40 | Low | `InProcessRouter::send` is `&self` async; if the agent's responder panics, the `oneshot::Sender` is dropped silently and the router sees `Err("agent dropped reply channel")` with no panic trace | `crates/ragent-agent/src/orchestrator/router.rs:54`. |

---

## 6. Timeout, Cancellation, and "Subagent Died" Propagation

| Signal | Producer | Consumer | Propagation path | Gap |
|--------|----------|----------|------------------|-----|
| **Sub-agent finished (success)** | `TaskManager::spawn_sync` / `spawn_background` | `wait_tasks`, `drain_completed` in `processor`, TUI, SSE | `Event::SubagentComplete { success:true, summary, duration_ms }` → `EventBus::broadcast` → `wait_tasks` filter (session_id + task_id) and `drain_completed` (in-memory TaskEntry lookup). | None. |
| **Sub-agent finished (failure)** | Same | Same | `Event::SubagentComplete { success:false, summary: "Error: ...", duration_ms }`. | None. |
| **Sub-agent cancelled** | Background path inside `spawn_background` | Same | `Event::SubagentCancelled { session_id, task_id }` — published **only if** `error_msg.contains("cancelled")` (`task/mod.rs:496, 511–515`). | `cancel_task` does NOT publish an event; the cancel signal relies on the agent loop's cooperative check producing an error string containing "cancelled" — fragile. |
| **Sub-agent killed** | `kill_task` | TUI, SSE | `Event::SubagentKilled { session_id, task_id, child_session_id, force:false }`. | A 10-second escalation task re-arms the cancel flag (`task/mod.rs:633–661`) but does not publish a second event. |
| **Teammate finished initial prompt** | `TeamManager::spawn_teammate_internal` success path | `team_wait`, TUI | `Event::TeammateIdle { session_id, team_name, agent_id }`. | Only fires for the **initial** prompt, not for subsequent `process_message` invocations. Teammates that re-run their loop on a new prompt won't fire `TeammateIdle` until they finish that prompt and **return** from `process_message`. |
| **Teammate idle (explicit)** | `team_idle` tool | `team_wait`, TUI | Currently **none** — the tool writes `member.status = Idle` to disk only. `Event::TeammateIdle` is published by `publish_message_event` if a `MessageType::IdleNotify` arrives, but `team_idle` never sends one. | **D-2, D-19** — the canonical idle signal is broken. |
| **Teammate failed** | `TeamManager::spawn_teammate_internal` after retries | TUI, SSE | `Event::TeammateFailed { session_id, team_name, agent_id, error }`. | `team_wait` does not match on it; lead blocks full timeout (**D-9**). |
| **Teammate spawn (success)** | `TeamManager::spawn_teammate_internal` | TUI, SSE | `Event::TeammateSpawned { session_id, team_name, teammate_name, agent_id }`. | None. |
| **Teammate suspended** | `TeamManager::suspend_teammate` | TUI, SSE | `Event::TeammateSuspended { session_id, team_name, agent_id }`. | The agent loop is not paused; only the mailbox poll loop is. The teammate can still send mail (poll loop drains its own outgoing? No — the poll loop is inbound-only). So a suspended teammate can drain its mailbox once and then its incoming is paused; its own outgoing depends on the agent loop's tool calls. |
| **Teammate resumed** | `TeamManager::resume_teammate` | TUI, SSE | `Event::TeammateResumed { session_id, team_name, agent_id }`. | Same as above. |
| **Teammate shutdown requested** | `team_shutdown_teammate` tool | Teammate (only if it checks mail) | `MessageType::ShutdownRequest` pushed to teammate's mailbox + `member.status = ShuttingDown` written. **No event published.** | `team_wait` includes `ShuttingDown` in wait set (**D-11**); cancel flag not set (**D-3**); lead has no way to know the request was merely queued. |
| **Teammate shutdown acknowledged** | `team_shutdown_ack` tool | Lead (via mailbox poll) | `MessageType::ShutdownAck` pushed to lead's mailbox; poll loop translates to `Event::TeammateMessage { from, to:"lead", preview }`. **No dedicated `Event::TeammateShutdownAck`.** | Lead cannot programmatically wait for shutdown acknowledgements; must poll `Event::TeammateMessage` and parse the type from the preview text. |
| **Teammate shutdown (manager path)** | `TeamManager::shutdown_teammate` | Teammate (if checks mail) | Sets cancel + poll_cancel, deregisters notifier, pushes `ShutdownRequest`, sets `member.status = Stopped`. **No event published.** | Called only by the manager; the TUI never invokes it directly. |
| **Teammate plan request** | `team_submit_plan` | Lead (via mailbox poll) | `MessageType::PlanRequest` pushed to lead mailbox; poll loop → `Event::TeammateMessage` (preview only). | Plan approval workflow is not wired into the LLM's loop — the lead is told to call `team_approve_plan` in the system prompt, but there is no event-driven prompt for it. |
| **Teammate plan approved/rejected** | `team_approve_plan` | Teammate (via mailbox poll) | `MessageType::PlanApproved` or `PlanRejected` pushed to teammate mailbox. | No event published; teammate must `team_read_messages` to learn of approval. |
| **Task claimed** | `team_task_claim` | TUI, SSE | `Event::TeamTaskClaimed` is defined, translated to SSE, and handled by TUI — **but never published by the tool**. | **D-1.** |
| **Task completed** | `team_task_complete` | TUI, SSE | `Event::TeamTaskCompleted` is defined, translated, handled — **but never published**. | **D-1.** |
| **Task assigned (lead → teammate)** | `team_assign_task` | (None) | `tasks.json` is updated. **No mailbox message, no event, no poll-loop wake.** | **D-16.** |
| **Subagent died (panic in agent loop)** | `process_message` | TUI, SSE | A panic in the agent loop would surface as an `Err` from `process_message`; the manager's background loop catches it and increments the retry counter. On final failure, `Event::TeammateFailed` is published. | If the panic occurs outside the agent loop (e.g. in the poll loop or in `publish_message_event`), the panic is on a detached `tokio::spawn` task with no `JoinHandle` tracked — the lead is not notified. The TUI is also not notified. |

---

## 7. High-Level Fix Suggestions

> These are suggestions only — no concrete code changes. Detailed implementation belongs to a follow-up plan (s5).

1. **Consolidate the team mailbox into a single canonical type.** Pick either `ragent-team/src/team/mailbox.rs` or `ragent-agent/src/team/mailbox.rs` as the source of truth, and have the other crate re-export it. Fix the `write_atomic`-without-lock bug in the chosen source.

2. **Publish the dead events.** Make `team_task_claim` publish `Event::TeamTaskClaimed { session_id, team_name, agent_id, task_id }` after a successful claim. Make `team_task_complete` publish `Event::TeamTaskCompleted`. Make `team_idle` push a `MessageType::IdleNotify` (or just publish `Event::TeammateIdle` directly with the agent_id, bypassing the mailbox indirection). Make `team_assign_task` push a `MessageType::Message` to the assigned teammate (or publish `Event::TaskAssigned`).

3. **Unify the two shutdown paths.** Either make `team_shutdown_teammate` delegate to `TeamManager::shutdown_teammate()` (forceful), or make the manager's path graceful. The two should produce the same end state: `member.status = Stopped`, cancel flag set, notifier deregistered. Then add a `Event::TeammateShutdownRequest` and `Event::TeammateShutdownAck` so the lead can wait for confirmation.

4. **Subscribe to the event bus before any state read in `team_wait`.** Restructure `team_wait` to: (a) subscribe first, (b) read the team store, (c) reconcile by replaying any relevant events received during the gap, (d) match on `TeammateIdle` AND `TeammateFailed` (remove from `waiting_for` on failure too), (e) shorten the wait set to `Working` only (exclude `ShuttingDown` and `Suspended`).

5. **Add a `request_id` (or `task_token`) to `MailboxMessage`.** Generated by the sender (e.g. UUID v4), included in every `team_message` and `team_broadcast` payload, and echoed in any reply. The receiver side can match replies to requests deterministically. `team_message` should also accept an optional `reply_to` parameter; the receiving teammate can then call `team_message` back with the `reply_to` token, and the leader's mailbox can route the reply.

6. **Add `fs2` file locking to `TeamStore::save` and `load`.** Match the pattern used by `TaskStore` and `Mailbox`. This closes the two-phase save race and the concurrent config corruption class of bugs.

7. **Set `current_task_id` when a task is claimed or completed.** Update `team_task_claim` (claim_next and claim_specific) and `team_task_complete` to write `member.current_task_id = Some(task.id)` and `member.current_task_id = None` respectively, in the same atomic section as the `tasks.json` update. Do the same for `team_assign_task` and `pre_assign_task`.

8. **Fix `Coordinator::start_job_async`'s unconditional `success: true`.** Track per-agent success counts; set `JobCompleted { success: !parts.is_empty() && errors.is_empty() }` and publish a `JobFailed` event when all agents failed.

9. **Backpressure on `InProcessRouter`.** Either bump mailbox capacity, implement retry with backoff on `SendError`, or expose a "mailbox full" error to the lead so the lead can decide to spawn fewer parallel jobs.

10. **Add a per-agent reply correlation in `start_job_sync` and `start_job_async`.** The `JobEntry.result` should be `HashMap<agent_id, Result<String, String>>` (or similar), not a concatenated string. Subscribers can then attribute failures.

11. **Make `team_spawn`'s `pending_manager` path explicit.** Either: (a) return an error so the LLM does not call `team_wait`, or (b) provide a `team_wait_for_pending` tool that polls for the manager to appear. Document the state machine in the system prompt.

12. **Add a panic catch in the background agent loop and poll loop.** Use `tokio::spawn` with a top-level `match` that publishes `Event::TeammateFailed` on panic. Track the `JoinHandle` in `TeammateHandle` so the manager can `await` it on shutdown.

13. **Document the cross-stack event taxonomy in one place.** Add a `docs/team-events.md` that lists every `Event::Teammate*` variant, its producer, its consumer(s), and its trigger condition. Currently this information is scattered across `ragent-types/src/event/mod.rs`, `ragent-agent/src/team/manager.rs`, the TUI, and the SSE translator.

14. **Cache `resolve_team_context_for_session`.** Use a per-session `OnceLock<Arc<TeamContext>>` or an LRU keyed by `session_id` to avoid re-scanning all teams on every message. With TTL or invalidation on team config changes.

15. **Reconcile should always have a prompt.** When `reconcile_spawning_members` picks up a member without a `spawn_prompt`, refuse to spawn (return error) rather than spawning with an empty prompt. This forces blueprint seeding to persist prompts.

16. **Subscribe race fix in `team_wait`:** subscribe to the event bus **before** the team store read, and reconcile the waiting set against any `TeammateIdle` / `TeammateFailed` events received in the gap.

17. **Plan state consistency:** on rejection, set `member.plan_status = Pending` (re-submission) **and** `member.status = PlanPending`. Or, on rejection, set `member.status = Working` and clear the plan. The current "keep `status = PlanPending`" comment is misleading; the state machine is unclear.

18. **Re-think the `InProcessRouter` oneshot contract.** A panic in the responder silently drops the reply. Wrap the responder in a `tokio::spawn` with a top-level `catch_unwind`, send `Err("responder panicked")` on panic, and let the router return it. This makes "subagent died" observable to the coordinator.

19. **Fan-out in `Coordinator::start_job_sync` should use `FuturesUnordered` (or `tokio::join_all`), not sequential `h.await?`.** Currently if 5 agents are dispatched and #1 takes 60s while #2 finishes in 1s, the lead waits 60s before receiving #2's response. `FuturesUnordered` would surface completions in arrival order.

20. **Single source of truth for the duplicate mailbox / team / task / config / swarm modules.** Either the agent crate or the team crate should own them; the other should re-export. This is the prerequisite for fixes 1, 4, 6, 7, 15 to be sustainable.

---

## 8. Cross-Cutting Observations (for context, not direct fixes)

- The `ragent-team` crate is conceptually a re-packaging of `ragent-agent`'s team code, but it has already drifted (mailbox write strategy, `publish_message_event` `_agent_id` parameter, several `let _ = ` discards). It depends on `ragent-agent` (`Cargo.toml:24`) but does not reuse the team types.
- The system prompt injected into every teammate (`crates/ragent-team/src/team/manager.rs:52–104`) is comprehensive but does not mention that `MessageType::IdleNotify` is not sent by `team_idle`, or that `Event::TeamTaskClaimed` is not published. The teammate LLM is being told to "call `team_read_messages` at the start of each turn" — and this is the only path by which the teammate learns of any lead action, because no other notifications fire.
- The TUI at `crates/ragent-tui/src/app.rs:12783–12820` handles `Event::TeamTaskClaimed` / `TeamTaskCompleted` to update `current_task_id` — but because these events are never published, the TUI's `current_task_id` is only ever updated by the local in-memory mutation in `app.rs:12795` and is otherwise stale. The on-disk `TeamMember.current_task_id` (read by `team_status` tool) is **always None**.
- The `team_wait` tool includes a comment at line 156: "Subscribe BEFORE the wait loop to avoid the race where a teammate becomes idle between the store read and the subscribe." This comment is **aspirational** — the subscription happens at line 158, after the store read at lines 79–90. Either fix the code or fix the comment.

---

## 9. Out of Scope (deferred to other reviewers)

- Peer-to-peer message ordering, deadlock, and impersonation — covered by swarm-s3.
- Transport reliability (retries, dead-letter, backpressure on the HTTP router) — covered by swarm-s4.
- Message schema/serialization versioning — covered by swarm-s1.
- Remediation milestones — owned by swarm-s5; this report intentionally does not propose a roadmap.
