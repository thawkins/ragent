# Swarm Task s4 — Communication Reliability Audit

**Scope:** Audit error handling, retries, timeouts, acknowledgments, ordering guarantees, idempotency, and failure recovery in ragent's inter-agent (team/swarm) communication paths.
**Status:** Review complete — gaps documented, no fixes implemented.

## 1. Communication Architecture Summary

Inter-agent communication in ragent is built on three mechanisms:

1. **Mailbox files** (`crates/ragent-team/src/team/mailbox.rs`) — one JSON file per agent under `teams/{name}/mailbox/{agent_id}.json`. Senders append; recipients drain unread.
2. **In-process event bus** (`crates/ragent-types/src/event/mod.rs`) — Tokio broadcast channel used for UI updates and `team_wait` wakeups.
3. **On-disk task/config stores** (`crates/ragent-team/src/team/task.rs`, `store.rs`) — shared state for task claims, member status, and settings.

Teammates are spawned by `TeamManager` (`crates/ragent-team/src/team/manager.rs`) which starts an agent loop and a mailbox polling loop per teammate. The lead uses `team_wait` to block on `Event::TeammateIdle`.

---

## 2. What Works Today

| Mechanism | Implementation | Reliability level |
|---|---|---|
| Mailbox write atomicity | `write_atomic` uses temp file + `fs::rename` | ✅ Atomic at filesystem level |
| Concurrent mailbox access | `fs2` exclusive/shared locks | ✅ Protected |
| Push-based polling wakeup | `Notify` registry keyed by `(team_dir, agent_id)` | ✅ Near-instant delivery when in-process |
| Transient LLM failures | Exponential backoff + jitter in `teammate_retry_backoff` | ✅ 3 retries, capped at 30 s |
| Task claim races | `flock` exclusive lock around read-modify-write | ✅ Single-machine safe |
| Failure persistence | `MemberStatus::Failed` + `last_spawn_error` written to `config.json` | ✅ Visible after crash |
| Graceful shutdown | `cancel` flag + `ShutdownRequest` mailbox message + `ShutdownAck` | ✅ Clean termination path exists |
| Quality-gate hooks | `HookOutcome::Allow`/`Feedback` for lifecycle events | ✅ Reversible actions |

---

## 3. Reliability & Resilience Gaps

### 3.1 No end-to-end message acknowledgments

- `MailboxMessage` has a `message_id` (UUID v4) but no field indicating it was successfully delivered or processed.
- `MessageType` includes `ShutdownAck` only; there is no generic `Ack`/`Processed` type.
- `team_message` and `team_broadcast` return success as soon as the file append completes; they do not verify the recipient's poll loop woke or that the message was read.
- **Gap:** A sender cannot distinguish "message persisted to disk" from "recipient consumed it". If a recipient's poll loop is dead, the message sits unread indefinitely while the sender believes communication succeeded.

### 3.2 Message delivery has no retry or dead-letter path

- `Mailbox::push` fails only on I/O error; callers propagate the error but do not retry.
- If a `Mailbox::push` succeeds but the recipient crashes before reading, the message remains in the mailbox but no retry/alarm is raised.
- There is no dead-letter queue or undelivered-message counter.
- **Gap:** Silent message loss is possible when recipients are unreachable but the filesystem write succeeds.

### 3.3 Event bus drops events silently

- `EventBus::publish` uses a fixed-size broadcast buffer (default 1024). When full, it emits a warning and drops the event (`SendError` path).
- It also warns when there are zero subscribers, then drops the event.
- `team_wait` relies entirely on `Event::TeammateIdle`. If the event is dropped because the lead's receiver lagged, `team_wait` times out even though the teammate may have gone idle.
- `team_spawn` waits for `PermissionReplied` with `timeout(Duration::from_mins(5))`; a dropped event causes a false timeout.
- **Gap:** Best-effort broadcast is insufficient for coordination primitives that must be reliable (`team_wait`, plan approval, shutdown ack).

### 3.4 No idempotency on task claims or completions

- `TaskStore::claim_next` and `complete` are atomic under `flock`, but they are not idempotent across separate calls.
- `complete(task_id, agent_id)` auto-claims the task if it was pending/unclaimed. Calling it twice with the same agent succeeds both times.
- `team_task_claim` for a specific task returns success if already claimed by the same agent, but `claim_next` returns `(task, true)` for an already-in-progress task.
- There is no idempotency key or client-side deduplication.
- **Gap:** Network-level retries or LLM-driven duplicate calls can cause double-completion or surprising state changes.

### 3.5 No heartbeat / liveness detection for subagents

- `TeammateHandle` tracks a `_child_session_id`, `cancel` flag, and poll loop, but the TeamManager does not periodically verify the child is alive.
- The only way a failed teammate is detected is when its agent loop returns an error and triggers `Event::TeammateFailed`.
- If a child process hangs silently (e.g. blocked on I/O), there is no timeout to mark it failed.
- **Gap:** A subagent can be "unreachable" (hung/crashed) without the lead knowing until `team_wait` times out or the user notices.

### 3.6 Leader crash leaves team in inconsistent state

- `TeamManager`, `TaskStore`, and `Mailbox` state is persisted to disk, but there is no recovery protocol when the lead restarts.
- `lead_session_id` is stored in `config.json`; a new lead session will have a different ID and may not take ownership of an existing Active team.
- Tasks left `InProgress` after a leader crash remain assigned to the old agent and will not be reclaimed unless manually reset.
- **Gap:** No leadership handoff, orphan-task cleanup, or automatic reconciliation after a lead session crash/restart.

### 3.7 Mailbox file is a single point of corruption

- `mailbox/{agent_id}.json` stores the entire message history as one JSON array.
- A truncated or corrupt file causes `serde_json::from_str` to fail; the polling loop logs a warning and `continue`s, but the recipient never receives new messages until the file is repaired or deleted.
- There is no compaction, rotation, or corruption recovery.
- **Gap:** One bad disk write can permanently disable a teammate's inbox.

### 3.8 `team_read_messages` drains and marks read in one operation

- `drain_unread` reads the entire mailbox, returns unread messages, and marks them all `read` in the same locked operation.
- If the tool later fails to present the messages to the model (or the model ignores them), they are already marked read and cannot be replayed.
- **Gap:** At-least-once consumption is not guaranteed; message loss after read is possible.

### 3.9 Ordering guarantees are implicit and fragile

- Messages are ordered by append order to a single file and by `sent_at` timestamps.
- The `Notify` registry provides wakeups but does not guarantee every wakeup processes every message (wakeup is lossy: `notify_one` may fire before the loop is waiting).
- Broadcast messages are written to each recipient's mailbox sequentially; if one push fails, the rest still proceed.
- **Gap:** No explicit sequence numbers or ordering contracts; cross-mailbox ordering is not defined.

### 3.10 Cancel flag synchronization

- `cancel.load(Ordering::Relaxed)` / `store(true, Ordering::Relaxed)` is used to terminate agent loops and poll loops.
- While likely sufficient in practice, `Relaxed` ordering provides no happens-before guarantee with the Tokio task scheduler; a woken task may not observe the store immediately.
- **Gap:** Potential delayed shutdown or missed cancellation on some CPU/memory models (minor, but worth noting).

### 3.11 Swarm reconcile has bounded attempts but no escalation

- `reconcile_spawning_members` tries 10 times with 100 ms sleeps and then gives up.
- Persistent spawn failures (e.g. model provider down) are logged but not retried later or surfaced to the user as a blocked swarm.
- **Gap:** Long-duration outages can leave swarm members stuck in `Spawning` with no automatic recovery.

### 3.12 `team_spawn` may queue indefinitely when `TeamManager` is absent

- If `ctx.team_manager` is `None`, the tool returns `status: "pending_manager"` with no follow-up mechanism.
- In the TUI, `/team create` seeds a blueprint in a background thread, but there is no guaranteed delivery that the `TeamManager` was initialized before `team_spawn` runs.
- **Gap:** Tool-time race can produce a "spawned" UI message that never actually starts the teammate.

### 3.13 No duplicate message suppression

- `MailboxMessage::new` always generates a fresh UUID. Identical content sent twice creates two records.
- There is no deduplication on `message_id` for retransmissions.
- **Gap:** Retries would produce duplicates; clients must tolerate or ignore them.

### 3.14 Plan approval lacks timeout and recovery

- `approve_plan` writes `PlanStatus::Approved`/`Rejected` to disk, but the waiting teammate polls by re-reading `config.json` in `is_plan_pending`.
- If the lead's approval event is lost or the config write fails, the teammate remains blocked.
- No timeout or fallback path exists for a stuck `PlanPending` teammate.
- **Gap:** Plan approval is a two-phase commit with no recovery coordinator.

### 3.15 `team_wait` can miss events and return partial results

- `team_wait` subscribes after reading the store, but between those two operations a teammate can go idle and publish `TeammateIdle` before the subscription is active.
- Even though it checks for already-idle members, the event-driven remainder is vulnerable to dropped broadcast events.
- **Gap:** False timeouts and inaccurate "still working" lists.

---

## 4. Failure-Mode Matrix

| Scenario | Current behavior | Desired behavior | Gap severity |
|---|---|---|---|
| Subagent process crashes | `Event::TeammateFailed` after retries; status persisted | Immediate detection + automatic respawn or task reassignment | High |
| Subagent hung/infinite loop | Detected only by `team_wait` timeout | Heartbeat + watchdog timeout | High |
| Message push succeeds but poll loop dead | Message sits unread | Delivery confirmation + retry/escalation | High |
| Lead session crashes | Team left Active; tasks stuck InProgress | Recovery protocol / leadership handoff | High |
| Event bus full / receiver lagged | Event dropped; `team_wait` may time out | Reliable coordination channel or at least event replay | High |
| Mailbox JSON corrupted | Poll loop logs warning and skips forever | Corruption recovery / rotation | Medium |
| Duplicate task completion | Second call succeeds (auto-claims if pending) | Idempotent completion | Medium |
| Network retry of `team_message` | Duplicate message in mailbox | Deduplication / idempotency key | Medium |
| Plan approval lost | Teammate blocks indefinitely | Approval timeout + retry or cancellation | Medium |
| Cancel flag propagation delay | Possible delayed shutdown | Acquire-release ordering | Low |

---

## 5. Recommendations (Not Implemented)

1. **Add an acknowledgment protocol** for at-least mailbox-level delivery: recipient writes an `Ack` message or updates a per-message `read_at` timestamp that the sender can observe.
2. **Persist an outbox + retry scheduler** for messages, or rely on a small task queue with exponential retry for failed pushes.
3. **Replace best-effort broadcast for coordination events** with a durable coordination log (e.g. a `coordination.json` file) that `team_wait` and plan approval can poll/consume.
4. **Implement a heartbeat / watchdog** in `TeamManager` that marks members `Failed` if no progress (idle/failure/task completion) is observed within a configurable timeout.
5. **Add idempotency keys** to `MailboxMessage` and `Task` operations so retries are safe.
6. **Introduce a leader recovery / orphan-cleanup routine** run at startup that reassigns tasks from a previous lead session when the current lead takes over.
7. **Mailbox rotation / corruption recovery**: on parse failure, move the corrupt file aside and start a fresh mailbox, surfacing the incident to the UI.
8. **Use `Ordering::SeqCst` or at least `Release`/`Acquire`** for cancel flags to guarantee cross-task visibility.

---

## 6. Files Reviewed

- `crates/ragent-team/src/team/manager.rs`
- `crates/ragent-team/src/team/mailbox.rs`
- `crates/ragent-team/src/team/store.rs`
- `crates/ragent-team/src/team/config.rs`
- `crates/ragent-team/src/team/task.rs`
- `crates/ragent-team/src/team/swarm.rs`
- `crates/ragent-team/src/tools/team_message.rs`
- `crates/ragent-team/src/tools/team_broadcast.rs`
- `crates/ragent-team/src/tools/team_read_messages.rs`
- `crates/ragent-team/src/tools/team_wait.rs`
- `crates/ragent-team/src/tools/team_spawn.rs`
- `crates/ragent-team/src/tools/team_task_claim.rs`
- `crates/ragent-team/src/tools/team_task_complete.rs`
- `crates/ragent-team/src/tools/team_task_create.rs`
- `crates/ragent-team/src/tools/team_idle.rs`
- `crates/ragent-team/src/tools/team_shutdown_ack.rs`
- `crates/ragent-team/src/tools/team_assign_task.rs`
- `crates/ragent-team/src/tools/team_cleanup.rs`
- `crates/ragent-team/src/tools/team_status.rs`
- `crates/ragent-types/src/event/mod.rs`
- `crates/ragent-agent/src/session/processor.rs`
- `crates/ragent-tui/src/app.rs` (relevant team event handling)
- `crates/ragent-team/tests/test_teammate_retry_backoff.rs`
- `crates/ragent-team/tests/test_swarm_agent_assignment.rs`
