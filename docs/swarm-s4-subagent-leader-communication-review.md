# Subagent-to-Leader Communication Review

**Task ID:** s4
**Reviewer:** swarm-s4
**Date:** 2026-06-21
**Scope:** Deep-dive into the code paths that handle subagents and team members reporting results, status updates, errors, and completion signals back to the leader/orchestrator.

---

## 1. Architecture Overview

There are **three independent subsystems** for subagent-to-leader communication in this codebase, each with different mechanisms:

| Subsystem | Crate / Module | Delivery Mechanism | Result Storage |
|-----------|----------------|--------------------|----------------|
| **Sub-agent tasks** (F13/F14) | `ragent-agent/src/task/mod.rs` | `Event::SubagentComplete` on `EventBus` + in-memory `TaskEntry` | `TaskManager.tasks: RwLock<HashMap>` |
| **Teams** (mailbox-based) | `ragent-agent/src/team/manager.rs`, `team/mailbox.rs`, `team/task.rs` | `EventBus` events (`TeammateIdle`, `TeammateFailed`, `TeammateMessage`, etc.) + JSON mailbox files on disk | `TeamStore` (config.json) + `TaskStore` (tasks.json) |
| **Orchestrator** (multi-agent) | `ragent-agent/src/orchestrator/coordinator.rs`, `router.rs`, `leader.rs` | `JobEvent` broadcast channel + `oneshot` reply channels | `DashMap<String, JobEntry>` |

---

## 2. Sub-agent Task System (`TaskManager`)

### File: `crates/ragent-agent/src/task/mod.rs`

#### 2.1 Synchronous spawn (`spawn_sync`, lines 197–322)

**Flow:**
1. Create isolated session, register `TaskEntry` with `Running` status.
2. Publish `Event::SubagentStart`.
3. Call `run_subagent()` which invokes `processor.process_message()`.
4. On success: update entry to `Completed`, publish `Event::SubagentComplete { success: true }`, return `TaskResult`.
5. On error: update entry to `Failed`, publish `Event::SubagentComplete { success: false }`, return `Err`.

**Assessment:** Results are reliably delivered. The synchronous path blocks the caller, so the result is returned directly via `TaskResult`. The `Event::SubagentComplete` event is always published (both success and failure paths), ensuring any `wait_tasks` listener is notified.

**No issues found.**

#### 2.2 Background spawn (`spawn_background`, lines 330–531)

**Flow:**
1. Check concurrency limit (`max_background`, default 8).
2. Create session, register `TaskEntry`.
3. Publish `Event::SubagentStart`.
4. `tokio::spawn` a background task that:
   - Resolves agent (on error: marks `Failed`, publishes `SubagentComplete { success: false }`, returns).
   - Runs `processor.process_message()`.
   - On success: marks `Completed`, publishes `SubagentComplete { success: true }`.
   - On error: checks if cancelled (error message contains "cancelled"); marks `Cancelled` and publishes `SubagentCancelled`, or marks `Failed` and publishes `SubagentComplete { success: false }`.

**Assessment:** Results are reliably delivered via both the in-memory `TaskEntry` and the `Event::SubagentComplete` event. The cancelled-vs-failed distinction is handled correctly.

**⚠️ Issue 2.2a — Cancelled detection relies on string matching (line 496)**
```rust
let cancelled = error_msg.contains("cancelled");
```
File: `task/mod.rs:496`

The system determines whether a task was cancelled by checking if the error message contains the substring "cancelled". This is fragile — if the underlying error message changes wording (e.g., "canceled" American spelling, or "Cancellation requested"), the task will be incorrectly marked as `Failed` instead of `Cancelled`, and a `SubagentComplete` event will be published instead of `SubagentCancelled`. This is a **correctness risk** but not a critical bug since both paths still mark the task as terminal and publish an event.

**⚠️ Issue 2.2b — Agent resolution uses `Config::default()` (lines 427, 765)**
```rust
let config = crate::Config::default();
let mut agent_info = match crate::agent::resolve_agent_with_customs(&agent, &config, &working_dir_buf) { ... }
```
File: `task/mod.rs:427` (background) and `task/mod.rs:765` (sync `run_subagent`)

Both `spawn_background` and `run_subagent` use `Config::default()` to resolve agents, which means custom agents defined in project `.ragent/agents/` may not be found if they require non-default config paths. However, `resolve_agent_with_customs` does scan the working directory for custom agent files, so this is mitigated. **Low risk.**

#### 2.3 Cancel/suspend/kill operations

**`cancel_task` (line 534):** Sets `cancel_flag` to `true`. The agent loop checks this flag at lines 1425, 1684, and 2914 of `processor.rs`. **Works correctly.**

**`suspend_task` (line 546):** Sets status to `Suspended` and sets a suspend flag. However:

**⚠️ Issue 2.3a — Suspend flag is never checked by the processor**
File: `task/mod.rs:555-559` (sets `suspend_flags`)
File: `session/processor.rs` — no reference to `suspend_flags`

The `suspend_flags` HashMap is populated by `suspend_task()` but is never read by `SessionProcessor::process_user_message()` or anywhere else in the agent loop. The suspend operation only changes the `TaskEntry.status` to `Suspended` and publishes an event — it does **not** actually pause the sub-agent's execution. The agent loop continues running. The `resume_task` method (line 573) sets the status back to `Running` and removes the flag, but since the flag was never checked, resume is a no-op in terms of actual execution control.

**Impact: Medium.** `suspend_task` gives the illusion of pausing a sub-agent but does not actually pause it. The task continues consuming tokens and making LLM API calls. Only `cancel_task` and `kill_task` actually stop execution.

**`kill_task` (line 599):** Sets status to `Terminating`, sets both kill and cancel flags, publishes `SubagentKilled { force: false }`. After 10 seconds, a background task force-kills: marks `Failed` with "Force-killed after timeout", publishes `SubagentKilled { force: true }`.

**⚠️ Issue 2.3b — `kill_task` uses `try_write` for cancel_flags (line 621)**
```rust
if let Ok(flags) = self.cancel_flags.try_write() {
    if let Some(cf) = flags.get(task_id) {
        cf.store(true, Ordering::Relaxed);
    }
}
```
File: `task/mod.rs:621-625`

`try_write()` is non-blocking and will silently fail if the lock is held. If another operation holds the `cancel_flags` write lock at the moment `kill_task` runs, the cancel flag will **not** be set, and the task won't receive the cancel signal. Only the kill flag gets set. Since the processor only checks `cancel_flag` (not `kill_flag`), the task may not actually terminate until the 10-second force-kill escalation fires.

**Impact: Low-Medium.** The 10-second force-kill provides a safety net, but there's a window where a kill request may be ineffectual due to lock contention.

**⚠️ Issue 2.3c — `kill_flag` is never checked by the processor**
File: `task/mod.rs:616-620` (sets `kill_flags`)
File: `session/processor.rs` — no reference to `kill_flags`

Similar to the suspend flag issue. The `kill_flags` HashMap is populated but never read by the processor. The kill mechanism relies entirely on the cancel flag (set via `try_write` — see Issue 2.3b). The kill flag is only used as a bookkeeping entry that gets cleaned up after 10 seconds.

#### 2.4 `drain_completed` — Background result injection (line 711)

The session processor calls `drain_completed()` between iterations (processor.rs:2647) to inject completed background task results into the parent conversation. Tasks with `waiter_count > 0` are skipped to avoid double-reporting (a waiter via `wait_tasks` is already handling it).

**Assessment:** This is well-designed. The `reported` flag prevents re-injection, and the `waiter_count` mechanism correctly coordinates with `wait_tasks`.

#### 2.5 `wait_tasks` tool — `crates/ragent-agent/src/tool/wait_tasks.rs`

**Flow:**
1. Subscribe to event bus BEFORE reading current state (eliminates race — good).
2. Collect already-completed tasks from current state.
3. Increment `waiter_count` for tasks still running (prevents `drain_completed` double-injection).
4. Wait on `Event::SubagentComplete` events with timeout.
5. Decrement `waiter_count` for all waited tasks (cleanup).

**⚠️ Issue 2.5a — Waiter count decrement includes already-completed tasks (line 175-179)**
```rust
let all_waited_ids: Vec<String> =
    results.keys().chain(waiting_for.iter()).cloned().collect();
for task_id in &all_waited_ids {
    task_manager.decrement_waiter(task_id).await;
}
```
File: `wait_tasks.rs:175-179`

The decrement loop includes tasks that were already completed before the wait started (those in `results` from the pre-check at lines 117-129). These tasks never had `increment_waiter` called on them (the increment at line 133-135 only runs for `waiting_for`, which excludes already-completed tasks). The `decrement_waiter` uses `saturating_sub`, so it won't underflow, but it will incorrectly decrement the waiter count for tasks that may have other legitimate waiters.

**Impact: Low.** `saturating_sub` prevents underflow, but if another `wait_tasks` call is concurrently waiting on the same task, this spurious decrement could cause that task's `drain_completed` to inject results prematurely (waiter_count drops to 0 when it shouldn't).

**⚠️ Issue 2.5b — Timeout returns partial results without error (line 165-168)**
When the timeout expires, the tool returns `Ok(ToolOutput)` with `timed_out: true` and the partial results collected so far. Tasks that didn't complete are listed as "still running". This is a design choice (not a bug) — the caller gets whatever results are available rather than an error. **Acceptable.**

---

## 3. Team System (`TeamManager` + Mailbox + TaskStore)

### File: `crates/ragent-agent/src/team/manager.rs`

#### 3.1 Teammate spawn and initial completion (lines 455–706)

**Flow:**
1. Allocate agent ID, update config.json with `Spawning` status.
2. Create child session, update config with `Working` status.
3. Resolve agent, build system prompt with team context.
4. Register mailbox notifier.
5. `tokio::spawn` the agent loop with retry logic (MAX_RETRIES=3):
   - On success: mark `Idle`, publish `Event::TeammateIdle`, return.
   - On error: check if transient (retry) or permanent (mark `Failed`, publish `Event::TeammateFailed`).
6. Start mailbox poll loop.
7. Publish `Event::TeammateSpawned`.

**Assessment:** The spawn flow is well-structured. The retry logic with linear backoff (500ms * attempt) handles transient API errors. Permanent errors (detected by `is_permanent_llm_api_error`) break the retry loop immediately.

**⚠️ Issue 3.1a — Only the initial prompt is run; subsequent turns rely on mailbox polling**
File: `manager.rs:605-633`

The spawned `tokio::spawn` block calls `processor.process_message()` once with the initial prompt. If the teammate completes that prompt successfully, it's marked `Idle`. For the teammate to do more work, the lead must send messages to its mailbox, which the poll loop picks up and... actually, the poll loop only **publishes events** — it does NOT inject messages into the teammate's session.

**⚠️ Issue 3.1b — Mailbox poll loop does NOT wake the teammate's agent loop**
File: `manager.rs:716-767` (`start_poll_loop`)

The poll loop drains unread messages and publishes events (`TeammateMessage`, `TeammateIdle`, `TeammateP2PMessage`), but it does **not** call `processor.process_message()` to inject the message content into the teammate's session. This means:

- When the lead sends a message to a teammate's mailbox, the teammate's agent loop (which has already returned from its initial `process_message` call) is **not restarted**.
- The message content is only visible to the TUI/event subscribers, not to the teammate's LLM context.
- The teammate must call `team_read_messages` as a tool within its own agent loop to read mailbox messages — but if the agent loop has already finished (status = `Idle`), there's no running loop to call the tool.

**Impact: High.** After a teammate finishes its initial prompt and goes idle, there is no mechanism to wake it up for additional work via mailbox messages. The lead would need to spawn a new teammate or the teammate's agent loop would need to be long-running (not returning after the first prompt). This appears to be a fundamental architectural gap in the team communication design. The system prompt tells teammates to "call `team_read_messages` at the start of each turn," but there is no next turn if the initial `process_message` returns.

**Note:** This may be by design if teammates are expected to be single-prompt agents that complete one task and go idle. The `team_task_claim` → `team_task_complete` → `team_idle` workflow supports this model. But the mailbox system's `team_message` tool implies bidirectional communication should work, and it currently doesn't wake a sleeping teammate.

#### 3.2 Teammate idle notification (`team_idle` tool, `tool/team_idle.rs`)

**Flow:**
1. Guard: block idle if agent has InProgress tasks (prevents going idle mid-task — good).
2. Run `TeammateIdle` hook (quality gate).
3. If hook returns `Feedback`: revert to `Working` status, return feedback to teammate.
4. Otherwise: mark `Idle`, clear `current_task_id`, return success.

**⚠️ Issue 3.2a — `team_idle` does NOT publish `Event::TeammateIdle`**
File: `tool/team_idle.rs:122-130`

The `team_idle` tool marks the member as `Idle` in `TeamStore` but does **not** publish `Event::TeammateIdle` on the event bus. The event is only published by:
- `manager.rs:628` — when the initial `process_message` completes successfully.
- `manager.rs:966` — when the mailbox poll loop receives an `IdleNotify` message.

Since the `team_idle` tool is the intended way for a teammate to report idle state after completing tasks, the `team_wait` tool (which subscribes to `Event::TeammateIdle`) will **not** be notified when a teammate calls `team_idle` after its initial prompt.

**Impact: High.** `team_wait` will hang until timeout if it's waiting for a teammate that has already called `team_idle` but whose initial `process_message` already published `TeammateIdle` earlier. The race depends on timing:
- If `team_wait` is called BEFORE the teammate's initial prompt finishes, it catches the `TeammateIdle` from `manager.rs:628` and works correctly.
- If `team_wait` is called AFTER the teammate's initial prompt finishes AND the teammate then does more work via tasks and calls `team_idle`, the event is not published and `team_wait` times out.

**This is a significant bug.** The `team_idle` tool should publish `Event::TeammateIdle` after marking the member idle.

#### 3.3 Teammate failure reporting (`manager.rs:667-691`)

When all retries are exhausted:
1. Persist `Failed` status and `last_spawn_error` to `TeamStore`.
2. Publish `Event::TeammateFailed` with the error message.

**Assessment:** Error propagation is correct. The TUI handles `TeammateFailed` at `app.rs:12755` and updates the member display.

#### 3.4 Mailbox message delivery (`team/mailbox.rs`)

**`Mailbox::push` (line 182):** Acquires exclusive file lock, reads existing messages, appends, writes back, signals notifier. **Correct.**

**`Mailbox::drain_unread` (line 212):** Acquires exclusive lock, filters unread, marks all as read, writes back. **Correct.**

**⚠️ Issue 3.4a — `write_locked` truncates and rewrites without atomic rename**
File: `mailbox.rs:169-176`
```rust
fn write_locked(file: &mut File, messages: &[MailboxMessage]) -> Result<()> {
    let json = serde_json::to_string_pretty(messages)?;
    file.set_len(0)?;
    file.seek(SeekFrom::Start(0))?;
    file.write_all(json.as_bytes())?;
    file.flush()?;
    Ok(())
}
```

The write operation truncates the file (`set_len(0)`) and then writes new content. If the process crashes between truncation and the write completing, the mailbox file will be empty or partially written, losing all messages. A safer approach would be to write to a temporary file and atomically rename.

**Impact: Low.** Process crashes during this narrow window are unlikely, and the file lock prevents concurrent writers. But data loss is possible on crash.

#### 3.5 Team task completion (`tool/team_task_complete.rs`)

**Flow:**
1. Call `TaskStore::complete(task_id, agent_id)`.
2. On error: return descriptive error (not a hard failure — good for agent UX).
3. Run `TaskCompleted` hook. If rejected: revert task to `InProgress`, return feedback.
4. On success: return completion confirmation.

**⚠️ Issue 3.5a — `Event::TeamTaskCompleted` and `Event::TeamTaskClaimed` are never published**
File: `tool/team_task_complete.rs` — no `event_bus.publish()` call.
File: `tool/team_task_claim.rs` — no `event_bus.publish()` call.

The event types `TeamTaskCompleted` and `TeamTaskClaimed` are defined in `ragent-types/src/event/mod.rs:521,532` and are handled by the TUI (`app.rs:12783,12802`) and SSE server (`sse.rs:314,315`), but **no code in the entire codebase ever publishes these events**. The TUI handlers exist but will never fire.

**Impact: Medium.** The TUI will never show real-time "task claimed" or "task completed" notifications for team tasks. The TUI can only discover task state changes by polling `team_status` or reloading from disk. This is a missed notification, not a data loss bug — the task state is correctly persisted in `tasks.json`.

**Root cause:** The `team_task_claim` and `team_task_complete` tools don't have access to the `EventBus` through `ToolContext` in a way that's wired up for team tools. They would need `ctx.event_bus.publish(...)` calls added.

#### 3.6 `current_task_id` is never set to `Some(...)` 

File: `team/config.rs:146` — field definition
File: `team/config.rs:182` — initialized to `None`
File: `team/manager.rs:624` — set to `None` on idle
File: `tool/team_idle.rs:127` — set to `None` on idle
File: `tool/team_shutdown_ack.rs:60` — set to `None` on shutdown

**⚠️ Issue 3.6a — `current_task_id` is only ever set to `None`**

A grep for `current_task_id = Some` returns zero matches. The field is initialized to `None`, set to `None` on idle/shutdown, but is **never set to a task ID** when a teammate claims a task. The `team_task_claim` tool updates `tasks.json` (setting `assigned_to` and `status=InProgress`) but does not update `config.json`'s `member.current_task_id`.

**Impact: Low.** The `team_status` tool displays `current_task_id` (line 96-99 of `team_status.rs`), but it will always be `None`. The TUI also reads this field. Functionally, task assignment is tracked in `tasks.json` via `assigned_to`, so the information exists — it's just not mirrored to the member record.

---

## 4. Orchestrator System (`Coordinator` + `Router`)

### File: `crates/ragent-agent/src/orchestrator/coordinator.rs`

#### 4.1 Synchronous job (`start_job_sync`, line 213)

**Flow:**
1. Match agents by capabilities.
2. Spawn a `tokio::spawn` per agent to send via router.
3. Collect results with `h.await?` (JoinError propagated).
4. If no responses: bail with "no successful responses from agents".
5. Apply conflict resolution policy or concatenate.

**⚠️ Issue 4.1a — JoinHandle errors are propagated, skipping remaining agents (line 251)**
```rust
for h in handles {
    match h.await? {
```
File: `coordinator.rs:251`

The `?` operator on `h.await` propagates `JoinError` (e.g., agent task panicked) as an `anyhow::Error`, which causes the entire `start_job_sync` to return `Err`. This means if one agent's tokio task panics, remaining agents' results are discarded even if they succeeded.

**Impact: Medium.** A single agent panic causes the entire job to fail, losing results from other successful agents. The `start_job_async` path (line 371) handles this better — it continues to the next agent on error.

#### 4.2 First-success job (`start_job_first_success`, line 309)

Tries agents in order, returns first non-error response. Skips timeout/error responses.

**⚠️ Issue 4.2a — "success" determined by string prefix check (line 335)**
```rust
if !resp.trim_start().to_lowercase().starts_with("error:") {
```
File: `coordinator.rs:335`

Success is determined by checking if the response does NOT start with "error:". This is a pragmatic heuristic but unreliable — a legitimate response could start with "error:" (e.g., an agent discussing error handling), or a failed response might not start with "error:" (e.g., a panic message).

**Impact: Low.** This is documented as MVP semantics (line 304-308). Real deployments should use proper Result types.

#### 4.3 Asynchronous job (`start_job_async`, line 371)

**Flow:**
1. Create `JobEntry` with broadcast channel.
2. `tokio::spawn` background task.
3. Publish `JobStarted`, match agents.
4. If no agents: publish `JobFailed`, mark "failed", return.
5. For each agent: send via router, publish `SubtaskAssigned`/`SubtaskCompleted`.
6. Concatenate results, mark "completed", publish `JobCompleted`.

**⚠️ Issue 4.3a — `JobCompleted` always reports `success: true` even if all agents failed (line 467)**
```rust
let _ = tx.send(JobEvent::JobCompleted {
    job_id: job_id_for_spawn.clone(),
    success: true,
});
```
File: `coordinator.rs:467`

After the agent loop, the job is marked as completed with `success: true` regardless of whether any agents actually succeeded. If all agents failed (every `router.send` returned `Err`), `parts` will be empty, `result` will be an empty string, but the job is still marked as "completed" with `success: true`.

**Impact: Medium.** Callers monitoring `JobEvent::JobCompleted { success: true }` will incorrectly believe the job succeeded. The `get_job_result` method (line 496) will return `("completed", Some(""))` — an empty result with "completed" status. There's no way for the caller to distinguish between "all agents succeeded with empty responses" and "all agents failed."

#### 4.4 Router timeout (`router.rs`, line 51)

```rust
let res = timeout(self.request_timeout, reply_rx).await;
```

Default timeout: 5 seconds (`InProcessRouter::new`, line 28).

**Assessment:** Timeout handling is correct. Three cases handled:
- `Ok(Ok(resp))` — success.
- `Ok(Err(_))` — agent dropped reply channel (agent crashed).
- `Err(_)` — timeout expired.

All produce appropriate error messages. The coordinator's error handling distinguishes timeouts from other errors for metrics (coordinator.rs:257-265, 448-456).

#### 4.5 `CoordinatorCluster` leader-based routing (`leader.rs`)

**���️ Issue 4.5a — `leader_coordinator` fallback is non-deterministic (line 201)**
```rust
nodes.values().next().cloned()
```
File: `leader.rs:201`

When no leader is elected, the cluster falls back to "any registered coordinator" via `HashMap::values().next()`. HashMap iteration order is non-deterministic (randomized per-process in Rust). This means different runs may route jobs to different coordinators.

**Impact: Low.** This is only relevant when no leader has been elected (before any `elect()` call). Once a leader is elected, routing is deterministic.

---

## 5. Summary of Issues

### Critical / High Impact

| ID | Issue | File:Line | Description |
|----|-------|-----------|-------------|
| 3.1b | Mailbox poll loop doesn't wake teammate agent loop | `manager.rs:716-767` | After a teammate's initial prompt completes, mailbox messages are not injected into its session. The teammate can't receive new work via messages. |
| 3.2a | `team_idle` tool doesn't publish `TeammateIdle` event | `team_idle.rs:122-130` | `team_wait` won't be notified when a teammate calls `team_idle` after doing task work. Causes `team_wait` to time out. |

### Medium Impact

| ID | Issue | File:Line | Description |
|----|-------|-----------|-------------|
| 2.3a | Suspend flag never checked by processor | `task/mod.rs:555`, `processor.rs` | `suspend_task` doesn't actually pause execution; the agent loop continues running. |
| 3.5a | `TeamTaskCompleted`/`TeamTaskClaimed` events never published | `team_task_complete.rs`, `team_task_claim.rs` | TUI/SSE handlers exist but never fire. No real-time task notifications. |
| 4.1a | JoinHandle error propagates in `start_job_sync` | `coordinator.rs:251` | One agent panic causes entire job to fail, losing other agents' results. |
| 4.3a | `JobCompleted` always `success: true` | `coordinator.rs:467` | Async job reports success even when all agents failed. |

### Low Impact

| ID | Issue | File:Line | Description |
|----|-------|-----------|-------------|
| 2.2a | Cancelled detection by string matching | `task/mod.rs:496` | Fragile "cancelled" substring check; may misclassify cancelled tasks as failed. |
| 2.3b | `kill_task` uses `try_write` for cancel flag | `task/mod.rs:621` | Lock contention may prevent cancel flag from being set; 10s force-kill is fallback. |
| 2.3c | Kill flag never checked by processor | `task/mod.rs:616` | Kill relies on cancel flag; kill flag is bookkeeping only. |
| 2.5a | Waiter count spurious decrement | `wait_tasks.rs:175-179` | Already-completed tasks get decrement without prior increment; `saturating_sub` prevents underflow. |
| 3.4a | Non-atomic mailbox writes | `mailbox.rs:169-176` | Process crash during write can lose messages. |
| 3.6a | `current_task_id` never set to `Some` | `team/config.rs:146` | Field always `None`; task assignment tracked only in `tasks.json`. |
| 4.2a | First-success by string prefix | `coordinator.rs:335` | "error:" prefix check is unreliable for determining agent failure. |
| 4.5a | Non-deterministic coordinator fallback | `leader.rs:201` | `HashMap::values().next()` is random when no leader elected. |

---

## 6. What Works Correctly

- **Synchronous sub-agent spawn** (`spawn_sync`): Results reliably returned via `TaskResult` and `SubagentComplete` event.
- **Background sub-agent completion**: `SubagentComplete` event always published (success, failure, and cancellation paths).
- **`drain_completed` + `waiter_count` coordination**: Prevents double-reporting of background task results between `drain_completed` and `wait_tasks`.
- **Mailbox file locking**: `fs2::FileExt` exclusive locks prevent concurrent write corruption.
- **Mailbox notifier registry**: Push-based wakeup via `tokio::sync::Notify` eliminates polling latency for in-process messages.
- **Team task store**: File-locked `tasks.json` with proper `claim_next`/`claim_specific`/`complete` operations.
- **Teammate failure reporting**: `TeammateFailed` event published with error message; TUI displays it.
- **Router timeout handling**: Three-way match (success/channel-dropped/timeout) with appropriate error messages.
- **Conflict resolution policies**: `Concat`, `FirstSuccess`, `LastResponse`, `Consensus`, `HumanReview` all implemented and tested.
- **`team_wait` subscribe-before-read**: Eliminates race between state check and event subscription.
- **`team_idle` guard**: Prevents teammates from going idle with InProgress tasks.
- **`team_task_complete` hook**: Quality-gate hooks can reject completion and revert task to InProgress.

---

## 7. Recommendations

1. **Fix `team_idle` to publish `Event::TeammateIdle`** (Issue 3.2a) — Add `ctx.event_bus.publish(Event::TeammateIdle { ... })` after marking the member idle in `tool/team_idle.rs`.

2. **Publish `TeamTaskClaimed` and `TeamTaskCompleted` events** (Issue 3.5a) — Add `ctx.event_bus.publish(...)` calls in `team_task_claim.rs` and `team_task_complete.rs`.

3. **Implement mailbox message injection into teammate sessions** (Issue 3.1b) — When the poll loop receives a non-idle message, call `processor.process_message()` with the message content to wake the teammate's agent loop. This requires keeping a reference to the processor and child session ID in the poll loop.

4. **Fix `JobCompleted` success flag** (Issue 4.3a) — Set `success: !parts.is_empty()` or track whether any agent succeeded.

5. **Fix `start_job_sync` JoinHandle error handling** (Issue 4.1a) — Use `h.await.ok().flatten()` instead of `h.await?` to skip panicked agents without failing the entire job.

6. **Set `current_task_id` on task claim** (Issue 3.6a) — In `team_task_claim.rs`, update the member's `current_task_id` in `TeamStore` after a successful claim.

7. **Make `suspend_task` actually pause execution** (Issue 2.3a) — Have the processor check the suspend flag and yield/sleep while suspended, or document that suspend is advisory-only.

8. **Use atomic file writes for mailboxes** (Issue 3.4a) — Write to `mailbox/{agent_id}.json.tmp` then rename to `mailbox/{agent_id}.json`.