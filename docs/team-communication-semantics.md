# Team Communication Semantics

**Status:** Reference document
**Source:** `COMMSPLAN.md` Milestone 8, Task M8-T6
**Audience:** Maintainers of `ragent-team` and `ragent-agent`, and agents
operating inside a team.

This document is the single source of truth for what the ragent team
subsystem guarantees when agents communicate. It describes the delivery
semantics of each primitive, the on-disk locking strategy, and the recovery
behavior when things go wrong. It is deliberately conservative: where a
guarantee is *not* listed, callers must assume it is absent.

---

## 1. Subsystems at a glance

| Primitive | Backing store | Transport | Guarantee |
|---|---|---|---|
| Mailbox messages | `mailbox/{agent-id}.json` per team | File append + `MailboxNotifierRegistry` wake | At-least-once append, **no** end-to-end ack |
| Shared task list | `tasks.json` per team | File read-modify-write under `flock` | Atomic per mutation, last-writer-wins on concurrent claim races (see §4) |
| Team config | `config.json` per team | File read-modify-write under `flock` | Atomic per save, member status / `lead_session_id` are the source of truth |
| Coordination events | In-process `EventBus` broadcast channel | Tokio broadcast | Best-effort — dropped on buffer overflow or no subscribers |
| Team context resolution | `SessionProcessor.team_context_cache` | In-memory `HashMap` with 5 s TTL | Stale by at most 5 s; invalidated on every `team_*` tool call |

The primary production path is the **Teams / Swarm** subsystem
(`ragent-team`). Sub-agent tasks (`ragent-agent/src/task/`) and the
orchestrator (`ragent-agent/src/orchestrator/`) are separate paths with
their own, weaker guarantees; they are out of scope for this document.

---

## 2. Mailbox delivery guarantees

### 2.1 What is guaranteed

- **At-least-once append.** `Mailbox::push` acquires an exclusive `flock` on
  the recipient's mailbox file, appends the message, and only releases the
  lock after the atomic rename (`write_atomic`) completes. A sender that
  retries after a crash may therefore produce duplicate messages; recipients
  must be idempotent on `message_id`.
- **Durability on ack.** When `push` returns `Ok`, the message is on disk and
  will survive a process crash. It is *not* guaranteed to have been seen by
  the recipient.
- **FIFO per recipient.** Messages are drained in `created_at` order within a
  single mailbox file.

### 2.2 What is *not* guaranteed

- **No end-to-end acknowledgement by default.** `drain_unread` marks
  messages `read = true` as it returns them to the caller. If the caller
  (the recipient's poll loop or `team_read_messages` tool) fails to forward
  the message to the model, or the model ignores it, the message is still
  marked read and will not be redelivered. A dead-letter / outbox path is
  described in `COMMSPLAN.md` M4-T6 but is **not** implemented.
- **No ordering across recipients.** A broadcast pushes to each mailbox in
  sequence; recipients may drain at different times.
- **No back-pressure.** A mailbox grows without bound if the recipient never
  drains it.

### 2.3 Wake mechanism

`MailboxNotifierRegistry` (in `ragent-team::team::mailbox`) holds a
`Notify` per agent id. `push` calls `notify_one` after a successful append,
so a blocked poll loop wakes immediately instead of waiting for its next
tick. This is a *performance* optimization, not a reliability mechanism —
the message is already durable on disk before `notify_one` is called.

---

## 3. EventBus guarantees

The `EventBus` is a Tokio `broadcast` channel. This has direct consequences
for coordination primitives:

- **Best-effort delivery.** If a subscriber's buffer is full when an event is
  published, the event is dropped for that subscriber. If there are no
  subscribers, the event is dropped entirely.
- **No persistence.** Events exist only in memory; they do not survive a
  process restart.
- **Coordination primitives must not rely on events alone.** `team_wait`,
  plan approval, and shutdown all fall back to re-reading disk state on
  timeout (see `COMMSPLAN.md` M3-T3 / M6-T4). A missed event degrades to a
  delayed response, not a hang.

Team event variants (`TeammateIdle`, `TeammateFailed`, `TeammateMessage`,
`TeamTaskClaimed`, `TeamTaskCompleted`, etc.) are published from the
relevant tools. A variant that is defined but never published is dead code;
`COMMSPLAN.md` M5-T7 tracks their removal or wiring.

---

## 4. Locking strategy

All persistent mutations in `ragent-team` go through a single race-free
pattern:

1. Acquire an exclusive `fs2` `flock` on a companion `.lock` file
   (`acquire_lock(true)`).
2. Read the current file contents.
3. Modify the in-memory representation.
4. Write atomically via `write_locked` (write to a unique temp file, then
   `fsync` + `rename`) **while still holding the lock**.
5. Release the lock.

This applies to:

- `Mailbox::push`, `drain_unread`, `mark_read` (`mailbox.rs`)
- `TaskStore::add_task`, `claim_next`, `claim_specific`, `complete`,
  `update_task`, `pre_assign_task` (`task.rs`)
- `TeamStore::save` / `load` for `config.json` (`store.rs`)

Temp file names include a UUID so two concurrent writers that somehow both
reach the write step cannot corrupt each other's temp file
(`COMMSPLAN.md` M1-T4).

### 4.1 Task claim races

`TaskStore::claim_next` is atomic per call, but the "claim or skip" decision
is last-writer-wins: if two teammates race to claim the same task, the first
to hold the lock wins and the second sees the task as `InProgress` and
moves on. There is no optimistic-concurrency retry; the loser simply claims
the next available task.

### 4.2 Idempotency

Task claim and completion are **not** idempotent by default
(`COMMSPLAN.md` M6-T3 tracks adding an idempotency key). A duplicate
`team_task_complete` call for an already-completed task returns an error
rather than silently succeeding.

---

## 5. Team context resolution (M8-T1)

`SessionProcessor` resolves the team identity for the current session on
every user message. The uncached path (`resolve_team_context_for_session`)
scans every team directory under the working dir — O(teams) and disk-bound.

The M8-T1 cache (`team_context_cache: Arc<RwLock<HashMap<SessionId,
(TeamContext, Instant)>>>`) wraps this with a 5-second TTL:

- **Hit within TTL:** return cached `TeamContext`, no disk I/O.
- **Miss or stale:** fall back to the full scan, cache the result (or evict
  the entry if the session is no longer in any team).
- **Invalidation:** the processor clears the entire cache after every
  `team_*` tool execution, since team tools can change membership for any
  session (create/join/leave/spawn/shutdown).

A stale cache entry is always safe: at worst a session briefly sees an old
team affiliation, and the next mutation forces a refresh.

---

## 6. Shutdown and idle signaling

### 6.1 `team_idle`

Marks the calling member `Idle` in `config.json` and publishes
`Event::TeammateIdle`. `team_wait` relies on either the event or a
disk-state re-check on timeout. `current_task_id` is cleared on idle.

### 6.2 `team_shutdown_teammate`

The unified shutdown path sets `ShuttingDown`, pushes a `ShutdownRequest`
mailbox message, sets the `cancel` and `poll_cancel` flags, deregisters the
notifier, and (for immediate shutdown) marks the member `Stopped`. The
teammate's poll loop observes the cancel flags and terminates. A busy
agent loop that does not poll will be cancelled on its next tool boundary.

### 6.3 `team_wait`

Subscribes to the `EventBus` **before** reading team state, handles
`TeammateIdle` and `TeammateFailed`, and on timeout re-reads `config.json`
to catch members whose on-disk status is already `Idle` / `Failed` /
`Stopped` but whose event was dropped.

---

## 7. Recovery behavior

| Failure | Detection | Recovery |
|---|---|---|
| Teammate hangs (no progress) | `team_wait` timeout (300 s default) | Lead re-reads disk; `Failed`/`Stopped`/`Idle` members are treated as finished. Heartbeat/watchdog tracked by M6-T1 (not yet implemented). |
| Lead crashes mid-team | New lead session id ≠ `config.json.lead_session_id` | M6-T2 (not yet implemented): adopt the team, reassign `InProgress` tasks of the old lead to `Pending`. |
| Mailbox file corrupt | `drain_unread` / `load` `serde` error | M6-T5 (not yet implemented): move file aside, start fresh mailbox, surface to UI. Today: the mailbox is unusable until manually repaired. |
| `EventBus` event dropped | `team_wait` spurious timeout | Disk-state re-check on timeout recovers gracefully. |
| Process restart | All in-memory state lost | `config.json`, `tasks.json`, and mailboxes are on disk and re-read on next `TeamStore::load`. `EventBus` state is lost; coordination primitives must re-derive from disk. |

---

## 8. Schema and validation

- **`TaskStatus`** serializes as `snake_case` (`pending`, `in_progress`,
  `completed`, `blocked`) across serde, Debug, tool output, and SSE.
- **`MessageType`** serializes as `snake_case` (`message`, `broadcast`,
  `plan_request`, `plan_approved`, `plan_rejected`, `shutdown_request`,
  `shutdown_ack`, `idle_notify`).
- **`schema_version`** is present on `TeamConfig`, `TaskList`, and mailbox
  metadata (`#[serde(default)]`); bump on breaking changes (M5-T2).
- **`deny_unknown_fields`** and `validate()` are applied to `MailboxMessage`,
  `Task`, and `TeamConfig` to reject typos and dangling references (M5-T3).
- **`correlation_id`** on `MailboxMessage` pairs request/reply messages
  (plan approval, shutdown ack) (M5-T4).
- **`updated_at`** on `TeamConfig` and `TaskList` records the last mutation
  time for debugging concurrent races (M5-T5).

---

## 9. Agent-facing summary

When an agent operates inside a team, it should assume:

1. **Mailbox messages you drain are gone once drained.** Act on them
   immediately or persist them yourself. Do not assume redelivery.
2. **Events are hints, not guarantees.** If you are waiting on a teammate,
   always be prepared to fall back to `team_status` / `team_task_list` to
   re-read disk state.
3. **Task claims are first-come-first-served.** A failed `team_task_claim`
   means another teammate got there first; claim the next task.
4. **`current_task_id` reflects what you are working on.** It is set on
   `team_task_claim` and cleared on `team_task_complete`, `team_idle`, and
   shutdown. Keep it accurate so the lead and TUI can see your work.
5. **Shutdown is cooperative.** When you observe a `ShutdownRequest` or your
   cancel flag is set, finish your current tool boundary and acknowledge
   with `team_shutdown_ack`.

---

*Cross-reference: `COMMSPLAN.md` §2 Milestones 1–8 for the remediation
tasks that established these guarantees. This document is updated as
each milestone lands.*