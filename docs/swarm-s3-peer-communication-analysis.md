# Swarm S3 — Message Routing and Delivery Code Review

**Task ID:** s3  
**Scope:** Trace actual code paths that send, route, receive and dispatch messages between the leader and subagents/teammates. Identify transport mechanisms, missing handlers, dead code, circular dependencies, race conditions, and dropped/misdirected-message scenarios.  
**Constraint:** Do not fix code.

---

## 1. Executive Summary

Team/sub-agent messaging is built on **two parallel transports**:

1. **Persistent mailbox files** — JSON append/drain files at `<team_dir>/mailbox/{agent-id}.json`, used for the actual message payload.
2. **In-process Tokio broadcast events** — `EventBus` (`tokio::sync::broadcast`) used to wake receivers and update the TUI in real time.

Messages are **not** sent via function calls, sockets, or durable queues. The leader and teammates are separate `SessionProcessor` sessions; all durable communication goes through the filesystem, and all in-process notification goes through the `EventBus`. This design is simple but introduces several correctness and observability gaps, documented below.

---

## 2. Transport Mechanism

### 2.1 Mailbox files (persistent, authoritative)

- `Mailbox::open(team_dir, agent_id)` — `crates/ragent-team/src/team/mailbox.rs:144`
- Path: `team_dir/mailbox/{agent_id}.json`
- `Mailbox::push` — `mailbox.rs:188`
  - Opens file, acquires **exclusive `flock`**, reads existing JSON array, appends message, unlocks, then writes atomically via a `.tmp` + `rename`.
  - After write it calls `signal_notifier(team_dir, agent_id)` to wake an in-process `tokio::sync::Notify` if one is registered.
- `Mailbox::drain_unread` — `mailbox.rs:218`
  - Acquires exclusive lock, reads all messages, filters `read == false`, marks **all** messages as `read`, writes file back.
  - Returns unread list to caller.
- `Mailbox::mark_read` — `mailbox.rs:255`
  - Marks a single `message_id` read; used nowhere in the team runtime (only exposed as a tool and API).

**Transport semantics:** append-only, at-least-once delivery to a poll loop, but **no acknowledgement or redelivery**. Once `drain_unread` runs, messages are permanently marked `read` regardless of whether downstream processing succeeds.

### 2.2 In-process `EventBus` (notification only)

- Defined in `crates/ragent-types/src/event/mod.rs:644`
- `EventBus::publish` broadcasts to all subscribers; warns when no subscribers or when buffer overflows.
- Team-relevant events: `TeammateSpawned`, `TeammateMessage`, `TeammateP2PMessage`, `TeammateIdle`, `TeammateFailed`, `TeammateSuspended`, `TeammateResumed`, `TeamTaskClaimed`, `TeamTaskCompleted`, `TeamCleanedUp`.

The `EventBus` does **not** carry the full message payload; it only carries a 200-char `preview`. The actual content is in the mailbox file.

### 2.3 Notifier registry (wakeup optimization)

- `MailboxNotifierRegistry` — `mailbox.rs:90`
- Static `OnceLock<RwLock<HashMap<(PathBuf, String), Arc<Notify>>>>`.
- `register_notifier` / `deregister_notifier` / `signal_notifier`.
- Each teammate poll loop registers its own `Notify` handle; `Mailbox::push` wakes it immediately instead of waiting for the 500 ms poll interval.

**Limitation:** the registry is **process-local**. If the leader and a teammate run in separate OS processes, the `Notify` is not registered and delivery falls back to periodic polling.

---

## 3. Send Paths

### 3.1 Direct message (`team_message` tool)

`crates/ragent-team/src/tools/team_message.rs:47`

- Resolves recipient via `resolve_agent_id`.
- If `to` is `"lead"` or starts with `"tm-"`, it is accepted verbatim without verifying existence (`team_message.rs:97`).
- Calls `Mailbox::open` for recipient and `push` with `MessageType::Message`.
- `from` is taken from `ToolContext.team_context.agent_id`, otherwise `"lead"`.

**Observation:** the `resolve_agent_id` name lookup (`team_message.rs:104`) only checks `member.name == name_or_id`; it does not handle duplicate teammate names. It also accepts any `"tm-..."` string as a valid agent ID, so a typo like `tm-999` will not fail here — the message is simply written to a mailbox file that nobody owns.

### 3.2 Broadcast (`team_broadcast` tool)

`crates/ragent-team/src/tools/team_broadcast.rs:43`

- Loads `TeamStore`, iterates members whose `status != Stopped`, opens each mailbox, and pushes a `MessageType::Broadcast`.
- Snapshot of status is taken at call time; if a member transitions to `Stopped` during the loop, it still receives the message.

### 3.3 Plan submission / approval / rejection

- Submit: `team_submit_plan.rs:47` → writes `PlanRequest` to lead mailbox, sets member `plan_status = Pending` and `status = PlanPending`.
- Approve/Reject: `team_approve_plan.rs:55` → updates config, then writes `PlanApproved`/`PlanRejected` to teammate mailbox.

### 3.4 Shutdown

- `team_shutdown_teammate.rs:49` → sets `status = ShuttingDown`, pushes `ShutdownRequest` to teammate mailbox.
- `TeamManager::shutdown_teammate` (`manager.rs:964`) also pushes `ShutdownRequest` and deregisters the notifier.
- `team_shutdown_ack.rs:41` → sets `status = Stopped`, pushes `ShutdownAck` to lead mailbox.

### 3.5 TUI direct message to focused teammate

`crates/ragent-tui/src/app.rs:13577`

- `send_teammate_message` bypasses the tool layer and writes directly to `Mailbox::open(&store.dir, &member.agent_id)`.

---

## 4. Receive / Dispatch Paths

### 4.1 Per-teammate poll loop

`TeamManager::start_poll_loop` — `manager.rs:832`

```text
tokio::spawn loop:
  - if cancel → break
  - select! { notify.notified() | sleep(500ms) }
  - if cancel → break
  - open mailbox
  - drain_unread()
  - for msg in unread { publish_message_event(...) }
```

`publish_message_event` — `manager.rs:1070`

Maps `MailboxMessage.message_type` to `Event`:

- `MessageType::IdleNotify` → `Event::TeammateIdle`
- Any message where neither `from` nor `to` is `"lead"` → `Event::TeammateP2PMessage`
- Everything else → `Event::TeammateMessage`

**Critical semantic loss:** `PlanRequest`, `PlanApproved`, `PlanRejected`, `Broadcast`, `ShutdownRequest`, and `ShutdownAck` all collapse into the generic `TeammateMessage`/`TeammateP2PMessage` events. The event payload only contains a 200-char `preview`; it does not preserve the original `MessageType`. Downstream handlers cannot distinguish a shutdown request from a casual message without re-parsing the mailbox content.

### 4.2 TUI event handling

`crates/ragent-tui/src/app.rs` `handle_event` matches:

- `TeammateSpawned` — `app.rs:12646`
- `TeammateMessage` — `app.rs:12694`
- `TeammateP2PMessage` — `app.rs:12717`
- `TeammateIdle` — `app.rs:12738`
- `TeammateFailed` — `app.rs:12755`
- `TeamTaskClaimed` — `app.rs:12783`
- `TeamTaskCompleted` — `app.rs:12802`
- `TeamCleanedUp` — `app.rs:12820`

**Missing handlers:**

- `Event::TeammateSuspended` is defined in `ragent-types/src/event/mod.rs:503` and published by `TeamManager::suspend_teammate` (`manager.rs:908`), but the TUI never matches it. The UI will not reflect a suspended teammate.
- `Event::TeammateResumed` is defined (`ragent-types/src/event/mod.rs:512`) and published by `TeamManager::resume_teammate` (`manager.rs:950`), but the TUI never matches it.

**Dead/zombie event definitions:**

- `Event::TeamTaskClaimed`, `Event::TeamTaskCompleted`, and `Event::TeamCleanedUp` are defined, handled by the TUI, and forwarded to SSE, but **never published by any production code path**. Search across `crates/ragent-team/src`, `crates/ragent-agent/src/tool`, and `crates/ragent-tui/src` found no `event_bus.publish(Event::TeamTaskClaimed { ... })` or similar. These events exist only in tests (`ragent-tui/tests/test_teams_tui.rs`, `ragent-server/tests/test_event_to_sse.rs`).

---

## 5. Race Conditions and Message-Loss Scenarios

### 5.1 Messages can be lost if `drain_unread` succeeds but event publishing fails

`drain_unread` marks every unread message as `read` and rewrites the file **before** `publish_message_event` returns. If the event bus subscriber lags, the TUI restarts, or the process crashes between `drain_unread` and event consumption, the messages have been removed from the mailbox and will never be delivered again. There is no dead-letter or replay mechanism.

### 5.2 `team_read_messages` races with the poll loop

`team_read_messages` (`crates/ragent-team/src/tools/team_read_messages.rs:41`) also calls `drain_unread`. If a teammate tool and the manager poll loop both drain the same mailbox concurrently, the exclusive lock serializes them, but whichever runs second will receive an empty set because the first already marked all messages `read`. The second caller may therefore miss messages that were intended for it. There is no mechanism to ensure only one consumer processes a given message.

### 5.3 Multiple subscribers on the same `EventBus` can duplicate work

The broadcast channel delivers events to all active subscribers. The TUI subscribes once; the `team_wait` tool subscribes per invocation; any other component can subscribe. If two subscribers both react to `TeammateIdle`, duplicate action is possible (e.g., both trying to update config or unblocking swarm members).

### 5.4 `team_wait` timeout race

`team_wait.rs:156` subscribes to `EventBus` **after** reading the store. It then waits for `TeammateIdle` events whose `session_id` matches the lead. The event filter is:

```rust
Ok(Ok(Event::TeammateIdle { session_id, team_name: ev_team, agent_id }))
    if session_id == ctx.session_id && ev_team == resolved_team_name && waiting_for.contains(&agent_id)
```

Because the store is read before subscribing, an idle event that fires between the store read and the `subscribe()` call will be missed. The tool has no fallback to re-check disk state on timeout, so it may incorrectly report a timeout while the teammate is already idle on disk.

### 5.5 Broadcast-channel overflow

`EventBus::publish` warns and drops events when the broadcast buffer (default 1024) is full. Slow TUI/SSE subscribers can cause team events to be lost entirely.

### 5.6 `start_poll_loop` wakeup race

The poll loop does:

```rust
tokio::select! {
    () = notify.notified() => {}
    () = tokio::time::sleep(interval) => {}
}
```

If `notify.notify_one()` is called before the loop enters `select!`, the notification is stored by `Notify` and consumed correctly. However, if the poll loop is cancelled immediately after `drain_unread`, the `notify.notified()` branch may consume a notification that arrived while shutting down, causing a final no-op iteration. This is harmless but wastes a wake.

### 5.7 Status vs. mailbox inconsistency

Many tools mutate `TeamStore.config` (e.g., `team_submit_plan.rs` sets `status = PlanPending`) and then push a mailbox message. Between the two writes, a crash or concurrent reader can observe a member in `PlanPending` with no corresponding `PlanRequest` in the mailbox, or vice versa. There is no atomic transaction spanning config and mailbox.

---

## 6. Dead Code / Misdirected Delivery

### 6.1 `Mailbox::mark_read` is effectively dead

`mark_read` (`mailbox.rs:255`) is exposed as a low-level API but no team tool or manager code uses it. The only consumer is `drain_unread`, which marks all unread messages in bulk. A per-message read API exists but is unused.

### 6.2 `MessageType` variants are not all handled in `publish_message_event`

Variants: `Message`, `Broadcast`, `PlanRequest`, `PlanApproved`, `PlanRejected`, `IdleNotify`, `ShutdownRequest`, `ShutdownAck`.

Only `IdleNotify` receives special treatment; all others are emitted as generic `TeammateMessage`/`TeammateP2PMessage`. Consequences:

- A teammate cannot easily filter for plan approvals vs. broadcast chatter without re-reading its mailbox.
- The leader cannot tell from the event alone whether an inbound message is a `ShutdownAck` or a normal reply.
- The `Broadcast` type is indistinguishable from a direct `Message` at the event layer.

### 6.3 `TeamManager::approve_plan` does not send any event

`approve_plan` (`manager.rs:1014`) updates the on-disk `TeamStore` but does not push a mailbox message or publish an event. The actual approval notification is handled by the `team_approve_plan` tool, which writes to the teammate mailbox. This split is confusing: the manager method is a config-only helper, while the tool handles messaging. If any other code calls `approve_plan` directly, the teammate is never notified.

### 6.4 `TeamManager::shutdown_teammate` sends `ShutdownRequest` after setting cancel flags

`manager.rs:964` first sets `cancel = true` and `poll_cancel = true`, **then** deregisters the notifier, **then** pushes the `ShutdownRequest`. Because the poll loop may have already exited, the `ShutdownRequest` may sit unread in the mailbox. The teammate's `team_shutdown_ack` tool will still mark it `Stopped` if called, but if the teammate never calls `team_read_messages`, the message is orphaned.

---

## 7. Circular Dependencies

### 7.1 `SessionProcessor` ↔ `TeamManager` cycle

`crates/ragent-agent/src/session/processor.rs:613`:

```rust
pub team_manager: std::sync::OnceLock<Arc<crate::team::TeamManager>>,
```

Comment claims the `OnceLock` "breaks the circular dependency with `TeamManager`". However, `TeamManager` itself holds an `Arc<SessionProcessor>` (`ragent-team/src/team/manager.rs:408`). Setting the `OnceLock` creates a reference cycle:

```text
SessionProcessor → OnceLock → Arc<TeamManager> → Arc<SessionProcessor>
```

Because `TeamManager` stores the processor as `Arc<SessionProcessor>` and the processor stores the manager as `Arc<TeamManager>`, both are kept alive by strong counts. In practice this cycle is broken only when the TUI or caller explicitly drops its references, so this is a runtime retention cycle, not a build-time cycle.

### 7.2 `TeamManager` owns child sessions created by the processor it references

`spawn_teammate_internal` (`manager.rs:544`) creates a child session via `self.processor.session_manager.create_session(...)` and then spawns a `tokio::task` that calls `self.processor.process_message(&child_sid, ...)`. The manager therefore both creates and drives child sessions through the same processor it depends on.

---

## 8. Identity Resolution

### 8.1 `resolve_team_context_for_session` is O(teams) and disk-based

`crates/ragent-agent/src/session/processor.rs:3165`

For every incoming message, the processor:
1. Lists all teams on disk.
2. Loads each `TeamStore`.
3. Checks if `session_id` matches the lead or any member's `session_id`.

This happens inside `process_message`, on every turn. It is not cached. If a team config file is temporarily locked or corrupt, the lookup silently continues (`let Ok(store) = ... else { continue }`).

### 8.2 Agent ID allocation race

`TeamStore::next_agent_id` (`store.rs:276`) computes the next `tm-NNN` by scanning existing members. If two teammates are created concurrently via separate tool calls (e.g., blueprint seeding in `team_create` and a direct `team_spawn`), both may compute the same next ID. The `spawn_lock` in `TeamManager::spawn_teammate_internal` serializes manager-based spawns, but the `team_create` seeding path does not hold that lock.

---

## 9. Tool / Manager Split Ambiguities

| Operation | Tool writes mailbox? | Tool updates config? | Manager updates config? | Manager sends event? |
|-----------|------------------------|----------------------|--------------------------|----------------------|
| `team_submit_plan` | Yes (`PlanRequest`) | Yes (`PlanPending`) | No | No |
| `team_approve_plan` | Yes (`PlanApproved`/`PlanRejected`) | Yes | No (`approve_plan` helper exists but unused) | No |
| `team_shutdown_teammate` | Yes (`ShutdownRequest`) | Yes (`ShuttingDown`) | Yes (duplicate push) | No |
| `team_shutdown_ack` | Yes (`ShutdownAck`) | Yes (`Stopped`) | No | No |
| `team_idle` | No | Yes (`Idle`) | Yes (`TeammateIdle` event) | Yes |
| `TeamManager::shutdown_teammate` | Yes (`ShutdownRequest`) | Yes (`Stopped`) | Yes | No direct event |

The responsibilities are split between tools and `TeamManager` in a way that can lead to duplicate or missing notifications depending on which entry point is used.

---

## 10. Concrete Code Locations

| Concern | File | Line(s) |
|---------|------|---------|
| Mailbox persistence | `crates/ragent-team/src/team/mailbox.rs` | 144–292 |
| Notifier registry | `crates/ragent-team/src/team/mailbox.rs` | 90–128 |
| Poll loop / dispatch | `crates/ragent-team/src/team/manager.rs` | 832–883, 1070–1107 |
| Spawn / child session | `crates/ragent-team/src/team/manager.rs` | 544–822 |
| Shutdown / suspend / resume | `crates/ragent-team/src/team/manager.rs` | 887–995 |
| Direct message tool | `crates/ragent-team/src/tools/team_message.rs` | 47–92 |
| Broadcast tool | `crates/ragent-team/src/tools/team_broadcast.rs` | 43–91 |
| Plan submit/approve | `crates/ragent-team/src/tools/team_submit_plan.rs`, `team_approve_plan.rs` | 47–96, 55–127 |
| Shutdown tools | `crates/ragent-team/src/tools/team_shutdown_teammate.rs`, `team_shutdown_ack.rs` | 49–100, 41–85 |
| Task claim/complete | `crates/ragent-team/src/tools/team_task_claim.rs`, `team_task_complete.rs` | 47–166, 56–182 |
| `team_wait` event subscription | `crates/ragent-team/src/tools/team_wait.rs` | 64–222 |
| TUI event handlers | `crates/ragent-tui/src/app.rs` | 12646–12840 |
| TUI direct message | `crates/ragent-tui/src/app.rs` | 13577–13622 |
| TUI team manager initialization | `crates/ragent-tui/src/app.rs` | 3237–3307 |
| Processor team context resolution | `crates/ragent-agent/src/session/processor.rs` | 1003–1006, 3165–3197 |
| Processor tool context | `crates/ragent-agent/src/session/processor.rs` | 2226–2243 |
| Hardwired auto-approve for `team_*` | `crates/ragent-agent/src/session/processor.rs` | 457–466 |
| Event definitions | `crates/ragent-types/src/event/mod.rs` | 459–565, 644–913 |

---

## 11. Recommendations (for follow-up tasks)

1. **Add missing TUI handlers** for `TeammateSuspended` and `TeammateResumed`.
2. **Either publish or remove** `TeamTaskClaimed`, `TeamTaskCompleted`, and `TeamCleanedUp`; currently they are dead event types.
3. **Preserve `MessageType` in events** so handlers can distinguish plan approvals, shutdowns, and broadcasts without re-parsing mailboxes.
4. **Decouple `drain_unread` from event publishing**: consider a two-phase read (mark as `read` only after successful downstream processing) or a dedicated acknowledgement path.
5. **Cache team membership** in `SessionProcessor` instead of scanning all team directories on every turn.
6. **Resolve the `SessionProcessor` ↔ `TeamManager` strong-reference cycle**; one side should use a weak reference or explicit teardown signal.
7. **Validate agent IDs** in `resolve_agent_id` against actual team members rather than trusting the `"tm-"` prefix.
8. **Make `team_wait` re-check disk state** after a timeout to avoid false-positive timeouts.
9. **Unify mailbox + config updates** behind a single atomic-ish path or at least a documented ownership rule to avoid split-brain state.

---

*Report generated by swarm-s3 for task s3.*
