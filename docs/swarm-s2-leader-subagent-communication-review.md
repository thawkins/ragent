# Leader-to-Subagent Communication Review

**Reviewer:** swarm-s2  
**Date:** 2026-06-21  
**Task:** Deep-dive into leader/orchestrator → subagent/team-member communication  
**Scope:** Message delivery, task dispatch, race conditions, error handling, dropped messages  

---

## Architecture Overview

The ragent codebase has **three distinct communication pathways** from a leader/orchestrator to subagents:

### 1. Team-Based Communication (`ragent-team` crate)
- **Mailbox system**: File-backed per-agent JSON mailboxes at `mailbox/{agent-id}.json`
- **Message flow**: `Mailbox::push()` (sender) → file → `Mailbox::drain_unread()` (recipient)
- **Notification**: In-process `tokio::sync::Notify` registry + 500ms fallback poll loop
- **Tools**: `team_message`, `team_broadcast`, `team_read_messages`, `team_submit_plan`, `team_approve_plan`, `team_shutdown_teammate`, `team_shutdown_ack`, `team_idle`
- **Task dispatch**: `tasks.json` with `fs2` file locks; `team_task_create`, `team_assign_task`, `team_task_claim`, `team_task_complete`
- **Spawn**: `team_spawn` → `TeamManager::spawn_teammate_internal` → child session + background agent loop + mailbox poll loop

### 2. Sub-Agent Communication (`ragent-agent` crate, `task/mod.rs`)
- **TaskManager**: `spawn_sync` (blocking) and `spawn_background` (non-blocking)
- **No mailbox**: One-shot task prompt via `process_message()`
- **Notification**: `EventBus` pub/sub (`SubagentStart`, `SubagentComplete`, `SubagentCancelled`)
- **Tools**: `new_task`, `wait_tasks`, `cancel_task`, `list_tasks`, `task_complete`

### 3. Orchestrator (`ragent-agent` crate, `orchestrator/`)
- **Coordinator** + **AgentRegistry** + **Router** (in-process or HTTP)
- **InProcessRouter**: `mpsc` channels (capacity 100) with `oneshot` replies, 5s timeout
- **HttpRouter**: HTTP POST with configurable timeout
- **RouterComposite**: Tries routers in sequence, first success wins

---

## Findings

### CRITICAL — Issue 1: Mailbox `push()` TOCTOU race condition (lock released before write)

**File:** `crates/ragent-team/src/team/mailbox.rs`, lines 188–215  
**Severity:** Critical  

The `push()` method acquires an exclusive lock, reads all messages, appends the new one, then **unlocks the file before calling `write_atomic()`**:

```rust
pub fn push(&self, message: MailboxMessage) -> Result<()> {
    let mut file = OpenOptions::new()...open(&self.path)?;
    file.lock_exclusive()?;
    // ... read messages ...
    messages.push(message);
    file.unlock()?;           // ← LOCK RELEASED HERE
    Self::write_atomic(&self.path, &messages)?;  // ← WRITE HAPPENS UNLOCKED
    ...
}
```

Between `file.unlock()` and `write_atomic()`, another writer can:
1. Open the file, acquire exclusive lock, read the **old** content (without the new message)
2. Append their own message
3. Unlock and write

The first writer's `write_atomic` then overwrites the second writer's message — **a dropped message**.

The same pattern exists in `drain_unread()` (lines 218–252) and `mark_read()` (lines 255–292): lock is released before `write_atomic()`.

**Impact:** Under concurrent writes (e.g., lead sends a message while another teammate sends a P2P message to the same recipient), messages can be silently dropped.

**Fix:** Write while holding the lock, or use the file handle's `set_len()` + `write_all()` instead of a separate temp-file rename.

---

### HIGH — Issue 2: `team_wait` misses `TeammateFailed` events

**File:** `crates/ragent-team/src/tools/team_wait.rs`, lines 168–188  
**Severity:** High  

The `team_wait` event loop only matches on `Event::TeammateIdle`:

```rust
match tokio::time::timeout_at(deadline, rx.recv()).await {
    Ok(Ok(Event::TeammateIdle { ... })) if ... => {
        waiting_for.remove(&agent_id);
    }
    Ok(Ok(_)) => continue,  // ← TeammateFailed is ignored!
    ...
}
```

If a teammate fails (publishes `Event::TeammateFailed`) while the lead is waiting, the lead ignores the event and waits until the timeout expires. The lead is blocked for up to 300 seconds even though the teammate will never become idle.

**Impact:** Lead wastes time waiting for failed teammates; user perceives a hang.

**Fix:** Also match `Event::TeammateFailed` and remove the agent from `waiting_for`.

---

### HIGH — Issue 3: `team_wait` race between store read and event bus subscribe

**File:** `crates/ragent-team/src/tools/team_wait.rs`, lines 78–158  
**Severity:** High  

The code:
1. Loads the team store from disk to determine which teammates are working (lines 79–142)
2. Subscribes to the event bus (line 158)
3. Enters the wait loop (line 168)

The comment at line 156 says "Subscribe BEFORE the wait loop to avoid the race" — but the subscription happens **after** the store read. If a teammate transitions from `Working` to `Idle` between steps 1 and 2, the `TeammateIdle` event is published before the subscription, so the lead never receives it. The teammate remains in `waiting_for` until timeout.

**Impact:** Lead waits unnecessarily for teammates that are already idle.

**Fix:** Subscribe to the event bus **before** reading the team store, then reconcile the store state with any events received during the gap.

---

### HIGH — Issue 4: `team_shutdown_teammate` tool doesn't cancel the agent loop

**File:** `crates/ragent-team/src/tools/team_shutdown_teammate.rs`, lines 49–100  
**Severity:** High  

The tool only:
1. Marks member as `ShuttingDown` in config
2. Sends a `ShutdownRequest` mailbox message

It does **not** call `TeamManager::shutdown_teammate()` which would:
- Set the `cancel` flag (terminates the agent loop)
- Set the `poll_cancel` flag (stops mailbox polling)
- Deregister the notifier
- Mark member as `Stopped`

The teammate only learns about shutdown if it happens to call `team_read_messages`. If the teammate is mid-task and not checking messages, it will continue running indefinitely. The `ShuttingDown` status is cosmetic — nothing enforces it.

**Impact:** "Shutdown" requests can be ignored by busy teammates; the lead has no way to force termination through the tool interface.

**Fix:** The tool should also call `TeamManager::shutdown_teammate()` or at minimum set the cancel flag via the team manager handle.

---

### HIGH — Issue 5: `TeamStore::save()` has no file locking — concurrent config corruption

**File:** `crates/ragent-team/src/team/store.rs`, lines 174–183  
**Severity:** High  

`TeamStore::save()` writes to a temp file and renames, but `TeamStore::load()` (lines 154–164) reads **without any file lock**. Multiple processes can:
1. Load the same config simultaneously
2. Each modify different fields (e.g., lead assigns task → updates member A; teammate updates its own status)
3. Each saves — the **last save wins**, overwriting the other's changes

Unlike `tasks.json` and mailbox files (which use `fs2` locks), `config.json` has no locking at all.

**Impact:** Concurrent config modifications silently overwrite each other. Member status updates, task assignments, and plan approvals can be lost.

**Fix:** Add `fs2` file locking to `TeamStore::load()` (shared lock) and `TeamStore::save()` (exclusive lock), or use a single read-modify-write under exclusive lock.

---

### HIGH — Issue 6: Divergent shutdown paths — tool vs manager

**File:** `crates/ragent-team/src/tools/team_shutdown_teammate.rs` vs `crates/ragent-team/src/team/manager.rs:908–939`  
**Severity:** High  

Two completely different shutdown implementations:

| Aspect | Tool (`team_shutdown_teammate.rs`) | Manager (`shutdown_teammate`) |
|--------|------------------------------------|-------------------------------|
| Member status | `ShuttingDown` | `Stopped` |
| Cancel flag | Not set | Set (terminates agent loop) |
| Poll cancel | Not set | Set (stops mailbox polling) |
| Notifier | Not deregistered | Deregistered |
| Mailbox message | `ShutdownRequest` | `ShutdownRequest` |
| Graceful | Yes (waits for ack) | No (immediate) |

The tool path is "graceful" (member stays in `ShuttingDown` until teammate calls `team_shutdown_ack`), but nothing enforces the transition. The manager path is "immediate" (marks `Stopped` right away). These divergent paths create inconsistent state depending on which code path is invoked.

**Impact:** Teammates may be in `ShuttingDown` forever if they don't call `team_shutdown_ack`. The `team_wait` tool includes `ShuttingDown` in its wait filter (line 117), so the lead will wait for teammates that are stuck in this state.

---

### MEDIUM — Issue 7: `team_broadcast` — partial failure not tracked

**File:** `crates/ragent-team/src/tools/team_broadcast.rs`, lines 71–81  
**Severity:** Medium  

If `Mailbox::open()` or `mailbox.push()` fails for one teammate, the entire `execute()` returns an `Err`. Messages already delivered to earlier teammates are not reported. The lead gets an error but doesn't know which teammates received the broadcast and which didn't.

```rust
for agent_id in &active {
    let mailbox = Mailbox::open(&team_dir, agent_id)?;  // ← fails here on 3rd agent
    mailbox.push(...)?;
    sent += 1;
}
```

**Impact:** Partial broadcast delivery with no visibility into which recipients got the message.

**Fix:** Collect per-recipient results and return a summary with succeeded/failed lists.

---

### MEDIUM — Issue 8: `team_assign_task` — no notification to the assigned teammate

**File:** `crates/ragent-team/src/tools/team_assign_task.rs`, lines 48–99  
**Severity:** Medium  

When the lead assigns a task to a teammate, the task is updated in `tasks.json` but **no mailbox message or event is sent** to the teammate. The teammate has no way to know it's been assigned a task unless it actively polls `team_task_claim` or `team_task_list`.

**Impact:** Assigned tasks sit idle if the teammate doesn't poll. The lead may believe work is in progress when the teammate is unaware of the assignment.

**Fix:** Send a mailbox message (e.g., `MessageType::Message` with task details) to the assigned teammate, or publish a `TeamTaskAssigned` event.

---

### MEDIUM — Issue 9: `team_message` — no validation that recipient is active

**File:** `crates/ragent-team/src/tools/team_message.rs`, lines 47–91  
**Severity:** Medium  

The `team_message` tool resolves the recipient and pushes to their mailbox, but doesn't check if the recipient is in a valid state (`Working`, `Idle`, `PlanPending`, `Spawning`). Messages can be sent to `Stopped` or `Failed` teammates and will sit unread forever. `team_broadcast` at least filters for non-`Stopped` members.

**Impact:** Wasted messages to dead teammates; sender gets a success response even though the message will never be read.

**Fix:** Check member status and return a warning if the recipient is `Stopped` or `Failed`.

---

### MEDIUM — Issue 10: Mailbox poll loop drains messages but doesn't inject into agent conversation

**File:** `crates/ragent-team/src/team/manager.rs`, lines 776–826  
**Severity:** Medium  

The poll loop drains unread messages and publishes events to the **event bus** (`TeammateMessage`, `TeammateP2PMessage`). These events go to the lead's session and the TUI — they do **not** inject messages into the teammate's conversation history. The teammate must independently call `team_read_messages` to actually read its mail.

If the teammate is busy processing a tool call or LLM response and doesn't call `team_read_messages`, messages pile up in the mailbox. The poll loop doesn't interrupt the agent loop to deliver messages.

**Impact:** Messages from the lead can sit unread in the teammate's mailbox for an extended period. The lead sees events but the teammate doesn't act on them.

**Note:** This is by design (teammates are instructed to call `team_read_messages` at the start of each turn), but it relies on LLM compliance.

---

### MEDIUM — Issue 11: `team_approve_plan` rejection leaves inconsistent state

**File:** `crates/ragent-team/src/tools/team_approve_plan.rs`, lines 86–98  
**Severity:** Medium  

On rejection:
- `plan_status = PlanStatus::Rejected`
- `status` is NOT changed (stays `PlanPending` — the comment says "Keep PlanPending so the UI shows they need to resubmit")
- A `PlanRejected` mailbox message is sent to the teammate

But `TeamManager::is_plan_pending()` (manager.rs line 974–982) checks `plan_status == PlanStatus::Pending`. After rejection, `plan_status` is `Rejected`, so `is_plan_pending()` returns **false**. This means the plan-pending write/bash tool blocking (if implemented in the processor) no longer blocks, even though the member status is still `PlanPending`.

The state is inconsistent: `status = PlanPending` but `plan_status = Rejected` and `is_plan_pending() = false`.

**Impact:** Teammate may start implementing before the plan is re-approved, depending on how `is_plan_pending` is used.

**Fix:** Either keep `plan_status = Pending` on rejection, or update `is_plan_pending()` to also check `status == PlanStatus::Rejected` or `status == MemberStatus::PlanPending`.

---

### MEDIUM — Issue 12: `team_idle` — no event published when teammate calls it mid-conversation

**File:** `crates/ragent-team/src/tools/team_idle.rs`, lines 122–145  
**Severity:** Medium  

When a teammate calls `team_idle`:
1. Member is marked `Idle` in config
2. A success message is returned

But no `Event::TeammateIdle` is published. The `TeammateIdle` event is only published by the `TeamManager` background task when the agent loop finishes (manager.rs lines 680–693). If the teammate calls `team_idle` mid-conversation (e.g., between tool calls), the lead won't receive an idle notification until the teammate's `process_message` call returns.

If the lead is using `team_wait`, it won't see the idle state until the agent loop exits, even though the teammate has explicitly declared itself idle.

**Impact:** Lead may wait longer than necessary for teammates that have declared idle but are still in their agent loop.

**Fix:** Publish `Event::TeammateIdle` from the `team_idle` tool.

---

### MEDIUM — Issue 13: `Coordinator::start_job_async` always reports success

**File:** `crates/ragent-agent/src/orchestrator/coordinator.rs`, lines 462–477  
**Severity:** Medium  

After collecting all agent responses, the async job **always** publishes `JobCompleted { success: true }`:

```rust
let result = parts.join("\n");
// ... update job entry ...
let _ = tx.send(JobEvent::JobCompleted {
    job_id: job_id_for_spawn.clone(),
    success: true,  // ← always true, even if all agents failed
});
```

If all agents failed (empty `parts`), the job is still marked as completed successfully with an empty result string. Subscribers see `JobCompleted { success: true }` with no indication of failure.

**Impact:** Callers relying on `JobEvent::JobCompleted.success` will incorrectly believe the job succeeded.

**Fix:** Set `success: !parts.is_empty()` or track whether any agent succeeded.

---

### MEDIUM — Issue 14: `TeamManager::reconcile_spawning_members` uses empty prompts

**File:** `crates/ragent-team/src/team/manager.rs`, lines 396–479  
**Severity:** Medium  

The reconcile loop uses `spawn_prompt.clone().unwrap_or_default()` (line 429). If the spawn prompt wasn't persisted in the member record (the `spawn_prompt` field), the teammate gets an empty string as its prompt. The comment at line 395 acknowledges this: "Prompts are not persisted by blueprints."

A teammate spawned with an empty prompt will either:
- Immediately idle (no work to do)
- Produce unexpected output (LLM interprets empty prompt freely)

**Impact:** Reconciled teammates from blueprints that don't persist prompts start with no instructions.

**Fix:** Blueprint seeding should always persist `spawn_prompt` in the member record (the code at team_create.rs:409–411 does this for `pending_manager` status, but not for successfully spawned members).

---

### MEDIUM — Issue 15: `TeamManager::spawn_teammate_internal` — two-phase config save race window

**File:** `crates/ragent-team/src/team/manager.rs`, lines 521–568  
**Severity:** Medium  

The spawn process:
1. Load store, add member with `Spawning` status, save (lines 521–536)
2. Create child session (lines 540–544)
3. Load store again, update member with `session_id` + `Working` status, save (lines 548–568)

Between phases 1 and 3, another process could load the config and see the member in `Spawning` state with no `session_id`. If that process makes decisions based on this (e.g., `team_status` showing a spawning member), the data is stale.

Combined with Issue 5 (no config file locking), a concurrent `TeamStore::save()` from another process between phases 1 and 3 could **overwrite** the phase-1 save entirely, losing the member record.

**Impact:** Member records can be lost or left in inconsistent state during concurrent spawns.

**Fix:** Use a single read-modify-write cycle under an exclusive lock, or add file locking to `TeamStore`.

---

### LOW — Issue 16: `resolve_team_context_for_session` scans all teams on every message

**File:** `crates/ragent-agent/src/session/processor.rs`, lines 3079–3111  
**Severity:** Low (performance)  

For every message processed, the session processor calls `resolve_team_context_for_session`, which iterates all teams on disk (`TeamStore::list_teams`), loads each config, and checks if the session is the lead or a member. There's no caching.

For projects with many teams, this is an O(n) file system operation per message.

**Impact:** Performance degradation with many teams; not a correctness issue.

**Fix:** Cache the team context per session ID with a TTL, or index sessions to team membership.

---

### LOW — Issue 17: `team_create` blueprint spawn — no error aggregation

**File:** `crates/ragent-team/src/tools/team_create.rs`, lines 249–447  
**Severity:** Low  

When processing `spawn-prompts.json`, each spawn tool invocation is independent. If one spawn fails (line 439–441), the error is logged but execution continues. The final output only shows the member list — failed spawns are not reported. The lead doesn't know which teammates failed to spawn from the blueprint.

**Impact:** Lead may not realize some teammates failed to spawn, leading to missing work.

**Fix:** Collect spawn failures and include them in the tool output.

---

### LOW — Issue 18: `TaskStore::complete` auto-claims unclaimed tasks

**File:** `crates/ragent-team/src/team/task.rs`, lines 379–392  
**Severity:** Low  

If a task is `Pending` or `assigned_to.is_none()`, the `complete()` method auto-claims it for the calling agent. This means any teammate can "complete" any unclaimed task without explicitly claiming it first.

```rust
if task.assigned_to.as_deref() != Some(agent_id) {
    if task.status == TaskStatus::Pending || task.assigned_to.is_none() {
        task.assigned_to = Some(agent_id.to_owned());
        // ... auto-claim ...
    } else {
        return Err(...);
    }
}
```

**Impact:** A teammate could mark a task complete that it never explicitly claimed or worked on. While the file lock prevents true races, the semantics are surprising.

**Fix:** Require explicit claim before completion, or document this as intentional behavior.

---

### LOW — Issue 19: `InProcessRouter` — no retry on channel send failure

**File:** `crates/ragent-agent/src/orchestrator/router.rs`, lines 48–50  
**Severity:** Low  

If `tx.send(req).await` fails (receiver dropped or channel full), the error is immediately returned with no retry:

```rust
tx.send(req)
    .await
    .map_err(|_| anyhow::anyhow!("failed to send to agent mailbox"))?;
```

The channel has a capacity of 100. Under high load, the channel could be full. There's no backpressure handling or retry.

**Impact:** Orchestrator jobs can fail under load if agent mailboxes are saturated.

**Fix:** Add a bounded retry with backoff, or increase channel capacity.

---

### LOW — Issue 20: `publish_message_event` — semantic message types collapsed into `TeammateMessage`

**File:** `crates/ragent-team/src/team/manager.rs`, lines 1015–1050  
**Severity:** Low  

The `publish_message_event` function maps all non-P2P, non-idle messages to `Event::TeammateMessage`:

```rust
_ if msg.from != "lead" && msg.to != "lead" => TeammateP2PMessage { ... },
_ => TeammateMessage { ... },  // PlanApproved, PlanRejected, ShutdownRequest, ShutdownAck, Broadcast
```

`PlanApproved`, `PlanRejected`, `ShutdownRequest`, `ShutdownAck`, and `Broadcast` all produce identical `TeammateMessage` events. The event doesn't distinguish between a plan approval and a shutdown request. Subscribers that want to react differently to these must inspect the `preview` field (first 200 chars of content).

**Impact:** Event consumers can't distinguish message types without parsing content; makes event-driven logic fragile.

**Fix:** Add message type to the event variants, or create dedicated event variants for plan/shutdown/broadcast.

---

### LOW — Issue 21: `team_spawn` — task pre-assignment failure is non-blocking

**File:** `crates/ragent-team/src/tools/team_spawn.rs`, lines 257–287  
**Severity:** Low  

If `task_store.pre_assign_task()` fails (task not found, already assigned, not pending), the error is logged as a warning and appended to the output as a warning message, but the spawn **still succeeds**. The teammate starts without a task.

```rust
Err(e) => {
    tracing::warn!(...);
    task_assignment_msg = format!("\n⚠️ Failed to pre-assign task '{task_id}': {e}");
}
```

**Impact:** Teammate is spawned but has no task. It must figure out to call `team_task_claim` on its own, or the lead must manually assign.

**Fix:** Consider making pre-assignment failure a hard error (abort the spawn), or at minimum make the warning more prominent in the tool output.

---

### LOW — Issue 22: Stale comment in `TaskStore::add_task`

**File:** `crates/ragent-team/src/team/task.rs`, line 403  
**Severity:** Low (documentation)  

The doc comment says "not file-locked because the lead is the only writer of new tasks", but the implementation **does** acquire an exclusive lock at line 413. The comment is incorrect and misleading.

**Fix:** Update the comment to reflect the actual implementation.

---

### LOW — Issue 23: Mailbox `write_atomic` uses predictable temp file path

**File:** `crates/ragent-team/src/team/mailbox.rs`, lines 176–182  
**Severity:** Low  

`write_atomic` creates a temp file at `path.with_extension("tmp")` — i.e., `mailbox/tm-001.json.tmp`. This path is predictable. If two processes attempt `write_atomic` simultaneously (possible if the file lock wasn't held — see Issue 1), they would write to the same temp file, corrupting each other's data.

**Fix:** Use a unique temp file name (e.g., include a UUID or PID).

---

## Summary Table

| # | Severity | Issue | File |
|---|----------|-------|------|
| 1 | Critical | Mailbox push TOCTOU race (lock released before write) | mailbox.rs:188–215 |
| 2 | High | team_wait ignores TeammateFailed events | team_wait.rs:168–188 |
| 3 | High | team_wait race between store read and subscribe | team_wait.rs:78–158 |
| 4 | High | team_shutdown_teammate doesn't cancel agent loop | team_shutdown_teammate.rs:49–100 |
| 5 | High | TeamStore save/load has no file locking | store.rs:154–183 |
| 6 | High | Divergent shutdown paths (tool vs manager) | team_shutdown_teammate.rs vs manager.rs:908–939 |
| 7 | Medium | team_broadcast partial failure not tracked | team_broadcast.rs:71–81 |
| 8 | Medium | team_assign_task sends no notification | team_assign_task.rs:48–99 |
| 9 | Medium | team_message doesn't validate recipient state | team_message.rs:47–91 |
| 10 | Medium | Poll loop doesn't inject messages into conversation | manager.rs:776–826 |
| 11 | Medium | Plan rejection leaves inconsistent state | team_approve_plan.rs:86–98 |
| 12 | Medium | team_idle doesn't publish TeammateIdle event | team_idle.rs:122–145 |
| 13 | Medium | Coordinator::start_job_async always reports success | coordinator.rs:462–477 |
| 14 | Medium | Reconcile uses empty prompts for blueprints | manager.rs:396–479 |
| 15 | Medium | Two-phase config save race window | manager.rs:521–568 |
| 16 | Low | resolve_team_context scans all teams per message | processor.rs:3079–3111 |
| 17 | Low | Blueprint spawn errors not aggregated | team_create.rs:249–447 |
| 18 | Low | TaskStore::complete auto-claims unclaimed tasks | task.rs:379–392 |
| 19 | Low | InProcessRouter no retry on send failure | router.rs:48–50 |
| 20 | Low | Semantic message types collapsed in events | manager.rs:1015–1050 |
| 21 | Low | Task pre-assignment failure is non-blocking | team_spawn.rs:257–287 |
| 22 | Low | Stale comment in TaskStore::add_task | task.rs:403 |
| 23 | Low | Predictable temp file path in write_atomic | mailbox.rs:176–182 |

---

## Key Recommendations

1. **Fix the mailbox TOCTOU race (Issue 1)** — This is the most critical issue. Messages can be silently dropped under concurrent access. Write while holding the lock.

2. **Add file locking to TeamStore (Issue 5)** — Config corruption from concurrent saves affects all team operations. This is foundational.

3. **Fix team_wait to handle failures (Issue 2)** and **subscribe before reading store (Issue 3)** — These two fixes together make `team_wait` reliable.

4. **Unify shutdown paths (Issue 4 + Issue 6)** — The tool should delegate to `TeamManager::shutdown_teammate()` or at minimum set cancel flags.

5. **Add task assignment notifications (Issue 8)** — Without this, assigned tasks can sit idle indefinitely.

6. **Publish TeammateIdle from team_idle tool (Issue 12)** — This makes `team_wait` responsive to explicit idle declarations.