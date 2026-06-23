# COMMSPLAN.md — Agent Communication Remediation Plan

**Team:** swarm-20260621-133647  
**Author:** swarm-s6 (task s6)  
**Date:** 2026-06-21  
**Status:** Planning — no code changes yet  

This document synthesizes the findings from the swarm communication review
(tasks s1–s5) into an actionable, prioritized remediation plan. It describes
what to change, why each change matters, and which reviewer finding(s) it
addresses. Implementation of the remediation code itself is **out of scope
for this task** and is left to subsequent engineering work.

---

## 1. Executive Summary

The ragent multi-agent communication stack is built on three overlapping
subsystems:

1. **Teams / Swarm** (`ragent-team` crate, mirrored partially in
   `crates/ragent-agent/src/team/`) — file-backed mailboxes, shared
   `tasks.json`, and the `EventBus`.
2. **Sub-agent tasks** (`ragent-agent/src/task/`) — in-memory `TaskManager`
   plus `EventBus` events.
3. **Orchestrator** (`ragent-agent/src/orchestrator/`) — capability-based
   in-process / HTTP routing.

The primary production path is the **Teams / Swarm** subsystem. The audit
found it functionally works for simple, low-concurrency cases, but it has
serious correctness and observability gaps that become dangerous under
concurrent use, leader restarts, or long-running swarms.

### Most severe issues

| Priority | Issue | Consequence if left unfixed |
|----------|-------|----------------------------|
| **P0** | `ragent-team` mailbox and `TaskStore` writes release the `flock` before the atomic write completes | Silent message loss / task-state corruption under concurrent writes |
| **P0** | `TeamStore::save()` / `load()` for `config.json` have no file locking | Concurrent config modifications overwrite each other, losing member status, plan approvals, and task assignments |
| **P0** | `team_shutdown_teammate` only writes a mailbox message; it does **not** set the teammate cancel flag | Busy teammates ignore shutdown requests and run indefinitely |
| **P0** | `team_wait` subscribes to the `EventBus` **after** reading the team store and ignores `TeammateFailed` | Lead hangs for up to 300 s waiting for teammates that are already idle or failed |
| **P1** | The team module is duplicated between `ragent-team` and `ragent-agent`, and the copies have diverged | Fixes applied to one copy (e.g. the race-free `write_locked` helper) are missing from the other; any change must be made twice |
| **P1** | Mailbox messages are drained and marked `read` before the recipient can act on them; there is no acknowledgement or redelivery | Messages can be permanently lost if event publishing fails, the model ignores them, or a concurrent `drain_unread` runs first |
| **P1** | `team_idle` does not publish `Event::TeammateIdle`, and the mailbox poll loop does not inject messages back into the teammate's agent loop | `team_wait` misses explicit idle declarations; teammates cannot be woken up by lead messages after going idle |
| **P1** | `team_read_messages` serializes `MessageType` with `Debug` (PascalCase) while the on-disk format is `snake_case`, and omits the `to` and `read` fields | LLM round-trips mismatch; agents lack recipient context for P2P messages |
| **P2** | Best-effort `EventBus` drops events when the buffer is full or there are no subscribers; several team event types are defined but never published | TUI/SSE miss state changes; coordination primitives (`team_wait`, plan approval) are unreliable |
| **P2** | No schema versioning, input validation, `deny_unknown_fields`, correlation IDs, or `updated_at` timestamps | Silent data corruption on manual edits, no migration path, ambiguous request/reply pairing, hard-to-debug races |

### Guiding remediation principles

1. **Fix data-loss races first.** Every persistent write must be atomic under
   the appropriate lock.
2. **Eliminate duplication.** The `ragent-agent` team module should be removed
   or made a thin re-export of `ragent-team` so fixes are applied once.
3. **Make coordination events reliable.** `team_wait`, plan approval, and
   shutdown must be robust to missed `EventBus` messages and must observe disk
   state on timeout.
4. **Clarify ownership.** Tools that update both `config.json` and a mailbox
   should use a single manager-level helper so config/mailbox state does not
   diverge.
5. **Improve observability before adding features.** Add message-type context,
   missing fields, validation, and heartbeats so operators can see when
   communication fails.

---

## 2. Milestones and Remediation Tasks

### Milestone 1 — Eliminate data-loss races in persistent stores

**Goal:** Ensure every on-disk mutation in the team subsystem is atomic and
race-free under concurrent readers and writers.

**Deliverables:**
- Race-free mailbox writes in `ragent-team`.
- Race-free `TaskStore` writes in `ragent-team`.
- File-locked `TeamStore::save()` / `load()` for `config.json`.
- A single, well-tested write helper used by all mutating paths.

**Priority:** P0 (data loss)

#### Tasks

| ID | Task | What to change | Why it matters | Findings addressed |
|----|------|----------------|----------------|--------------------|
| M1-T1 | Hold the lock across the full mailbox write | Change `Mailbox::push`, `drain_unread`, and `mark_read` in `crates/ragent-team/src/team/mailbox.rs` so that `file.unlock()` is called **after** the data is written to disk (either via `write_locked` in place or via `write_atomic` while still holding the lock). Keep atomic rename for crash safety. | The current code releases `flock` before `write_atomic`, creating a TOCTOU window where a second writer can interleave and be overwritten. This silently drops messages. | s2 Issue 1, s4 Issue 1, s3 §5.1/5.2, s1 §4.1 |
| M1-T2 | Hold the lock across the full TaskStore write | Apply the same fix to all mutating `TaskStore` methods in `crates/ragent-team/src/team/task.rs` (`add_task`, `claim_next`, `claim_specific`, `complete`, `update_task`, `pre_assign_task`). Use `write_locked` or keep the lock for the duration of `write_atomic`. | Task claims and completions can be overwritten by concurrent writers, producing inconsistent `assigned_to`, `status`, and `depends_on` state. | s4 Issue 1, s2 §4.20 |
| M1-T3 | Add file locking to `TeamStore` | Add `fs2` shared locks to `TeamStore::load()` and exclusive locks to `TeamStore::save()` in `crates/ragent-team/src/team/store.rs`. Re-use the same lock file handle for the read-modify-write in higher-level helpers. | `config.json` is the source of truth for member status, plan status, and `lead_session_id`. Without locking, concurrent saves from tools and the manager clobber each other. | s2 Issue 5, s4 §4.5, s3 §5.7 |
| M1-T4 | Make temp-file names unpredictable | Replace `path.with_extension("tmp")` in mailbox / store atomic writes with a unique name containing a UUID or process ID (e.g. `.{uuid}.tmp`). | Two concurrent writers that both reach `write_atomic` without a lock (or after lock release) would otherwise write to the same temp path and corrupt each other. | s2 Issue 23, s3 §5.6 |
| M1-T5 | Add concurrent-write regression tests | Add tests that spawn multiple tasks/threads, have them push mailbox messages or claim/complete tasks concurrently, and assert no messages or state updates are lost. | Prevents regressions of the exact race conditions that were found. | s2, s4 |

---

### Milestone 2 — Unify the duplicated team implementation

**Goal:** Remove the `ragent-agent` duplicate of the team module so that all
team fixes are applied in exactly one place.

**Deliverables:**
- `ragent-agent` consumes `ragent-team` as a library dependency.
- The two `TeamManager` / mailbox / task implementations are merged into a
  single implementation in `ragent-team`.
- A decision record documenting why the split existed and how it was resolved.

**Priority:** P1 (maintainability, prevents re-introduction of M1 bugs)

#### Tasks

| ID | Task | What to change | Why it matters | Findings addressed |
|----|------|----------------|----------------|--------------------|
| M2-T1 | Add `ragent-team` as a dependency of `ragent-agent` | Update `crates/ragent-agent/Cargo.toml` to depend on `ragent-team`. Remove the mirrored source files under `crates/ragent-agent/src/team/` (except any agent-loop integration glue that legitimately belongs in `ragent-agent`). | The two crates currently maintain independent copies of `mailbox.rs`, `task.rs`, `manager.rs`, `swarm.rs`, and `config.rs`. The copies have diverged; the `ragent-agent` versions contain fixes (e.g. `write_locked`) that the `ragent-team` versions lack. | s4 Issue 2, s2 §4.28, s1 ��3.2 |
| M2-T2 | Reconcile `TeamManager` integration points | Move the `ragent-agent`-specific `TeamManager` glue (session creation, agent-loop spawn, prompt injection) into a thin wrapper in `crates/ragent-agent/src/team_integration.rs` that calls the unified `ragent-team::TeamManager`. | The manager needs access to `SessionProcessor` to spawn child sessions, but the core team logic (mailbox polling, store updates, event publishing) should live in one crate. | s1 §3.2, s3 §7.1/7.2 |
| M2-T3 | Decide the fate of `store.rs` shared via `#[path]` | `store.rs` is currently the only file shared via a `#[path]` include. Either keep it in `ragent-team` and re-export it, or move it to a new `ragent-team-types` crate if circular dependencies prevent direct use. Document the decision. | Ensures the final structure is maintainable and does not re-introduce the duplication through a different mechanism. | s4 Issue 2 |
| M2-T4 | Run the existing team/swarm test suite against the unified crate | Move and adapt tests from `crates/ragent-agent/tests/` and `crates/ragent-team/tests/` so they exercise the single implementation. Add a CI check that fails if duplicated team code reappears. | Confirms the merge does not break existing behavior and prevents future drift. | s1–s5 |

---

### Milestone 3 — Make `team_wait`, shutdown, and idle signalling reliable

**Goal:** Ensure the lead can accurately detect when teammates finish, fail, or
shut down, and can terminate them promptly.

**Deliverables:**
- `team_wait` observes `TeammateFailed`, subscribes before reading state, and
  falls back to disk on timeout.
- `team_idle` publishes `Event::TeammateIdle`.
- `team_shutdown_teammate` actually cancels the agent and poll loops.
- A unified shutdown path used by both the tool and `TeamManager`.

**Priority:** P0 (liveness / hangs)

#### Tasks

| ID | Task | What to change | Why it matters | Findings addressed |
|----|------|----------------|----------------|--------------------|
| M3-T1 | Subscribe to `EventBus` before reading team state in `team_wait` | Reorder `team_wait.rs` so the event subscription happens **before** the initial `TeamStore::load`/status scan. Capture any events that arrive between the scan and the wait loop and reconcile them. | A teammate that goes idle between the store read and the subscription will otherwise be missed, causing a false 300 s timeout. | s2 Issue 3, s3 §5.4 |
| M3-T2 | Handle `TeammateFailed` in `team_wait` | Add an `Ok(Ok(Event::TeammateFailed { agent_id, .. }))` branch that removes the failed agent from `waiting_for` and records the error. | A failed teammate will never become idle; without this branch the lead waits the full timeout. | s2 Issue 2, s4 §4.2/3.15 |
| M3-T3 | Re-check disk state on `team_wait` timeout | Before returning a timeout, call `TeamStore::load()` again and treat any members whose on-disk status is `Idle`, `Failed`, or `Stopped` as finished. | Dropped `EventBus` events (buffer full / no subscribers) currently cause false timeouts even though the teammate is legitimately idle on disk. | s3 §5.3/5.5, s4 §3.15 |
| M3-T4 | Publish `TeammateIdle` from `team_idle` | After `team_idle` successfully marks the member `Idle` in `TeamStore`, call `ctx.event_bus.publish(Event::TeammateIdle { ... })`. | `team_wait` and the TUI rely on this event; the tool currently only updates disk state. | s2 Issue 12, s4 §3.2a |
| M3-T5 | Set cancel flags from `team_shutdown_teammate` | Have the tool resolve the `TeamManager` and call a single `shutdown_teammate` helper that sets `cancel`, sets `poll_cancel`, deregisters the notifier, pushes `ShutdownRequest`, and updates status. | The tool currently only writes a mailbox message; a busy teammate never reads it and keeps running. | s2 Issue 4, s4 §4.11 |
| M3-T6 | Unify graceful vs. immediate shutdown semantics | Define one shutdown flow with an optional `graceful: bool` parameter. Graceful: set `ShuttingDown`, push `ShutdownRequest`, wait for `ShutdownAck`. Immediate: set cancel flags, mark `Stopped`. Make both the tool and `TeamManager` use the same helper. | Two divergent implementations currently create inconsistent state (`ShuttingDown` forever, `Stopped` too early, duplicate `ShutdownRequest` pushes). | s2 Issue 6, s3 §6.4 |
| M3-T7 | Add lifecycle tests | Write tests for: (a) teammate fails while lead is in `team_wait`, (b) teammate goes idle before `team_wait` starts, (c) `EventBus` event is dropped but disk state is correct, (d) shutdown actually terminates a busy agent loop. | Provides confidence that the liveness fixes work and do not regress. | s2–s4 |

---

### Milestone 4 — Harden message delivery semantics

**Goal:** Reduce the chance that messages are lost after being written to the
mailbox, and give senders visibility into delivery.

**Deliverables:**
- `drain_unread` does not permanently mark messages read until downstream
  processing succeeds, or a dead-letter path exists.
- `team_assign_task` notifies the assigned teammate.
- `team_broadcast` reports per-recipient success/failure.
- `team_message` validates the recipient is active.
- Messages carry enough context for the LLM to understand them.

**Priority:** P1 (correctness, UX)

#### Tasks

| ID | Task | What to change | Why it matters | Findings addressed |
|----|------|----------------|----------------|--------------------|
| M4-T1 | Separate "read" from "processed" in mailbox consumption | Change `drain_unread` to either (a) return messages without marking them read and provide an explicit `acknowledge(message_id)` path, or (b) mark read only after the recipient tool / poll loop successfully forwards the message to the model. Keep the existing behavior as a fallback for backward compatibility. | Currently messages are removed from the mailbox as soon as `drain_unread` runs, even if the event consumer lags or the model ignores them. | s3 §5.1, s4 §3.8 |
| M4-T2 | Notify assigned teammate on `team_assign_task` | After updating `tasks.json`, push a `MailboxMessage { type: Message, to: agent_id, content: task assignment notice }` to the assigned teammate, or publish a new `TeamTaskAssigned` event. | Without notification, assigned tasks sit idle until the teammate polls `team_task_list` or `team_task_claim`. | s2 Issue 8, s4 Issue 9 |
| M4-T3 | Return per-recipient results from `team_broadcast` | Replace the early-return `?` in `team_broadcast.rs` with a loop that collects `Result` per agent and returns a JSON summary of succeeded/failed recipients. | Partial broadcast delivery currently returns a single opaque error with no indication of which teammates received the message. | s2 Issue 7 |
| M4-T4 | Validate recipient state in `team_message` | Before pushing, load `TeamStore` and check that the recipient exists and is not `Stopped` / `Failed`. Return a warning (but still deliver?) or an error if the recipient is dead. | Messages to dead teammates sit unread forever while the sender receives a success response. | s2 Issue 9 |
| M4-T5 | Fix `team_read_messages` output schema | Use `serde_json::to_value(&m.message_type)` (snake_case) instead of `format!("{:?}", m.message_type)` (PascalCase). Add `"to"` and `"read"` fields to the JSON metadata and to the human-readable text output. | LLM sees PascalCase from the tool but snake_case on disk; round-tripping or comparing values fails. P2P messages also lack recipient context. | s4 Issues 4–5 |
| M4-T6 | Add delivery-status outbox (optional, post-MVP) | Consider adding an `outbox.json` per sender or per team that records pending messages and a `delivered_at` / `read_at` timestamp once the recipient drains them. | Provides the foundation for retries, dead-letter handling, and observability without changing the core mailbox semantics. | s4 §3.1/3.2 |

---

### Milestone 5 — Fix schema, validation, and observability gaps

**Goal:** Make the on-disk and event-layer representations consistent,
versioned, and self-describing.

**Deliverables:**
- Consistent serialization for statuses and message types.
- Schema versions on all root persisted types.
- Input validation and `deny_unknown_fields` on key structs.
- Correlation IDs for request/reply message pairs.
- `updated_at` timestamps and clearer event payloads.

**Priority:** P1 (debuggability, future-proofing)

#### Tasks

| ID | Task | What to change | Why it matters | Findings addressed |
|----|------|----------------|----------------|--------------------|
| M5-T1 | Unify `TaskStatus` string representation | Choose one representation (snake_case recommended) for serde, Debug, tool output, and SSE. Add unit tests that assert all output paths produce the same value. | `TaskStatus` currently appears as `"pending"`, `"Pending"`, and `"in_progress"` depending on the output path, confusing consumers. | s4 Issue 3 |
| M5-T2 | Add `schema_version` to root persisted types | Add `#[serde(default)] schema_version: u32` to `TeamConfig`, `TaskList`, and the per-mailbox envelope / metadata. Bump the version on breaking changes and add a migration helper. | Without versioning, any future field rename/addition will break deserializing existing team directories. | s4 Issue 6 |
| M5-T3 | Add `#[serde(deny_unknown_fields)]` and validation | Apply `deny_unknown_fields` to `MailboxMessage`, `Task`, and `TeamConfig`. Add a `validate()` method that checks `from`/`to` are valid agent IDs, `message_id` is a UUID, `task.id` matches the expected format, and `assigned_to` references a team member. | Typos and manual edits are silently ignored today, producing messages with no sender or tasks assigned to non-existent agents. | s4 Issues 7, 11 |
| M5-T4 | Add `correlation_id` to `MailboxMessage` | Add `pub correlation_id: Option<String>`. Set it on `PlanRequest`, `ShutdownRequest`, and any other request message; copy it into the corresponding `PlanApproved`/`PlanRejected`/`ShutdownAck` reply. | Currently a teammate cannot tell which plan was approved or rejected, and shutdown request/ack pairs are not correlated. | s4 Issue 8 |
| M5-T5 | Add `updated_at` to `TeamConfig` and `TaskList` | Add `#[serde(default)] updated_at: Option<DateTime<Utc>>` and update it in every `save()`. | Debugging concurrent config races is currently impossible because the schema does not record when it was last modified. | s4 Issue 12 |
| M5-T6 | Distinguish message types in events | Extend `Event::TeammateMessage` / `TeammateP2PMessage` to include the original `MessageType` (or add dedicated variants for `PlanApproved`, `PlanRejected`, `ShutdownRequest`, `ShutdownAck`, `Broadcast`). Update SSE payload structs accordingly. | Event consumers currently cannot distinguish a plan approval from a broadcast without parsing the 200-char preview. | s2 Issue 20, s3 §6.2 |
| M5-T7 | Remove or publish dead event types | Either wire `team_task_claim` and `team_task_complete` to publish `Event::TeamTaskClaimed` / `TeamTaskCompleted`, or delete the variants and their TUI/SSE handlers. Do the same for `TeamCleanedUp` and `MessageType::IdleNotify`. | Dead code misleads maintainers and clutters the event schema. | s3 §4.2, s4 Issues 13/15, s2 §4.9/4.10 |

---

### Milestone 6 — Improve runtime resilience and leader recovery

**Goal:** Make the system tolerate hung/crashed teammates and leader session
restarts without leaving tasks stuck in limbo.

**Deliverables:**
- Heartbeat / watchdog for teammates.
- Leader crash recovery / orphan cleanup.
- Idempotent task claim/completion.
- `EventBus` overflow handling.

**Priority:** P2 (resilience)

#### Tasks

| ID | Task | What to change | Why it matters | Findings addressed |
|----|------|----------------|----------------|--------------------|
| M6-T1 | Implement teammate heartbeat / watchdog | Add a background task in `TeamManager` that periodically checks each `Working`/`Spawning` member. If no progress (idle/failure/task completion) has been observed within a configurable timeout, mark the member `Failed` and publish `TeammateFailed`. | A hung or silently crashed teammate is currently only detected when `team_wait` times out. | s4 §3.5, s4 Failure-Mode Matrix |
| M6-T2 | Add leader recovery / orphan cleanup | On `TeamManager` initialization, if the current lead `session_id` differs from `config.json`'s `lead_session_id`, adopt the team and reassign tasks that were `InProgress` for the old lead to `Pending`. Document the handoff semantics. | A leader crash leaves tasks stuck `InProgress` assigned to a dead session; a new lead currently has no recovery path. | s4 §3.6, s4 Failure-Mode Matrix |
| M6-T3 | Make task claim and completion idempotent | Add an idempotency key to `Task` (e.g. `claimed_by: Option<String>`, `completed_by: Option<String>`) and reject a second completion by a different agent. Return success without mutation when the same agent repeats a claim/completion it already owns. | Duplicate LLM-driven calls can currently auto-claim and complete tasks out from under the intended owner. | s4 Issues 10, s4 §3.4 |
| M6-T4 | Handle `EventBus` overflow for coordination events | For events that drive coordination primitives (`TeammateIdle`, `TeammateFailed`, `PermissionReplied`), either (a) use a dedicated unbounded mpsc channel, or (b) make `team_wait` and plan approval fall back to polling disk state when an event is suspected to have been dropped. | The broadcast channel silently drops events when subscribers lag; coordination primitives cannot afford that. | s3 §5.5, s4 §3.3 |
| M6-T5 | Mailbox corruption recovery | In the poll loop / `drain_unread`, if `serde_json::from_str` fails, move the corrupt file aside (e.g. `mailbox/tm-001.json.corrupt.{timestamp}`), start a fresh empty mailbox, and surface the incident to the UI/log. | One bad disk write currently disables a teammate's inbox permanently. | s4 §3.7 |

---

### Milestone 7 — Sub-agent task robustness

**Goal:** Fix the less critical but still incorrect behaviors in the non-team
sub-agent path.

**Deliverables:**
- Suspend and kill flags are actually honored by the processor, or the API is
  simplified to remove the misleading operations.
- `wait_tasks` waiter accounting is accurate.
- Cancel detection uses a typed signal instead of string matching.

**Priority:** P2 (correctness)

#### Tasks

| ID | Task | What to change | Why it matters | Findings addressed |
|----|------|----------------|----------------|--------------------|
| M7-T1 | Honor or remove `suspend_task` | Either (a) wire `suspend_flags` into `SessionProcessor` so the agent loop actually pauses, or (b) remove `suspend_task`/`resume_task` and the `SubagentSuspended`/`SubagentResumed` events if they are not intended to be implemented. Document the choice. | `suspend_task` currently only changes a status field; the agent loop keeps running and consuming tokens. | s4 Subagent review §2.3a |
| M7-T2 | Honor `kill_flag` or rely on cancel flag alone | Ensure `kill_task` uses a blocking write (`write()` instead of `try_write()`) for `cancel_flags` so the cancel signal cannot be lost due to lock contention. If `kill_flag` has no purpose, remove it. | A kill request can currently fail to set the cancel flag due to `try_write` contention; the task then runs until the 10 s force-kill. | s4 Subagent review §2.3b/2.3c |
| M7-T3 | Fix `wait_tasks` waiter-count bookkeeping | Only decrement `waiter_count` for task IDs that were actually incremented during the wait. Do not include already-completed tasks in the decrement loop. | Spurious decrements can cause `drain_completed` to inject results prematurely when another waiter is still waiting. | s4 Subagent review §2.5a |
| M7-T4 | Replace cancelled string matching with typed cancellation | Propagate a structured cancellation error (`Cancelled` variant) from the agent loop to `TaskManager` instead of checking `error_msg.contains("cancelled")`. | The current string check will misclassify cancellations if the error wording changes. | s4 Subagent review §2.2a |

---

### Milestone 8 — Performance, cleanup, and documentation

**Goal:** Remove low-impact debt and document cross-cutting behavior.

**Deliverables:**
- Cached team context resolution.
- Aggregated blueprint spawn errors.
- Correct `current_task_id` tracking.
- Stale comments and dead code removed.
- Developer-facing documentation for team communication semantics.

**Priority:** P3 (polish, maintainability)

#### Tasks

| ID | Task | What to change | Why it matters | Findings addressed |
|----|------|----------------|----------------|--------------------|
| M8-T1 | Cache `resolve_team_context_for_session` | Add a `DashMap<SessionId, (TeamContext, Instant)>` cache in `SessionProcessor` with a short TTL (e.g. 5 s), and invalidate on team create/join/leave. | The processor currently scans every team directory on every message, which is O(teams) and disk-bound. | s2 Issue 16, s3 §8.1 |
| M8-T2 | Aggregate blueprint spawn errors in `team_create` | Collect per-prompt spawn results in `team_create.rs` and include a `failed_spawns` list in the tool output. | Failed blueprint teammates are currently logged but not reported to the lead. | s2 Issue 17 |
| M8-T3 | Set `current_task_id` when claiming a task | In `team_task_claim`, after successfully claiming a task, update the claiming member's `current_task_id` in `TeamStore` and clear it on idle/shutdown. | The field is currently always `None`, so `team_status` and the TUI cannot show what a teammate is working on. | s4 Subagent review §3.6, s2 §4.8 |
| M8-T4 | Fix stale `TaskStore::add_task` comment | Update the doc comment to reflect that the method **does** acquire an exclusive lock. | Misleading documentation can cause future maintainers to make unsafe assumptions. | s2 Issue 22 |
| M8-T5 | Validate `resolve_agent_id` against actual members | Change `team_message.rs` to check `TeamStore.members` for the exact `agent_id` or `name` rather than accepting any `"tm-..."` string. | A typo like `tm-999` currently succeeds but writes to a mailbox nobody owns. | s3 §3.1 |
| M8-T6 | Document communication semantics | Add a new `docs/team-communication-semantics.md` describing delivery guarantees (at-least-once mailbox append, best-effort event bus, no end-to-end ack by default), locking strategy, and recovery behavior. | Future agents and maintainers need a single source of truth for what the team subsystem guarantees. | s1–s5 |

---

## 3. Cross-Cutting Dependencies

The milestones above must be executed in roughly the following order:

1. **Milestone 1** (data-loss races) is the absolute first priority and
   should not depend on later work.
2. **Milestone 2** (unify duplication) should ideally start in parallel with
   M1 but will need to land immediately after so that M3–M8 fixes are only
   applied once.
3. **Milestone 3** (liveness) can proceed in parallel with M2 once the team
   module is unified, because the fixes touch the same files as M2.
4. **Milestones 4–8** can be worked largely in parallel after M2, but some
   tasks depend on M3 (e.g. event-type fixes in M5-T6 should use the same
   events that M3-T4 publishes).

High-risk coupling:
- M2 and M3 both touch `TeamManager`. Coordinate to avoid merge conflicts.
- M4-T1 (read-vs-processed) may interact with M3-T6 (shutdown flow) because
  `ShutdownRequest` must remain deliverable until acknowledged.
- M5-T6 (event message types) must align with M3-T4 and M4-T2 so that the TUI
  and SSE display the new events correctly.

---

## 4. Definition of Done

The plan is considered complete when:

- `COMMSPLAN.md` exists at the repository root and contains all sections above.
- Every task maps to at least one finding from s1–s5.
- Every P0 and P1 task has a concrete "what to change", "why it matters",
  and "findings addressed" entry.
- The milestones are ordered by impact (data loss → liveness → correctness →
  resilience → polish).
- A brief dependency / ordering guide is included.

The actual implementation of the remediation code is intentionally excluded
from this definition of done and is tracked by separate engineering tasks.

---

## 5. Out of Scope

The following items are noted for context but are **not part of this plan**:

- Re-architecting the team subsystem to use a real message queue (e.g. SQLite,
  NATS, Redis). The plan keeps the file-backed design and hardens it.
- Changing the orchestrator into the primary multi-agent path. The plan only
  fixes the most severe orchestrator bug (`start_job_async` always reporting
  success) and the router retry issue.
- Adding new user-facing features such as team chat UI, video/SSE streaming
  improvements, or new slash commands.
- Performance optimizations beyond the O(teams) lookup cache (M8-T1).

---

## 6. Finding-to-Task Index

| Finding (source) | Primary tasks |
|------------------|---------------|
| s2 Issue 1 — mailbox TOCTOU race | M1-T1, M1-T4, M1-T5 |
| s2 Issue 2 — `team_wait` ignores `TeammateFailed` | M3-T2 |
| s2 Issue 3 — `team_wait` store-read / subscribe race | M3-T1, M3-T3 |
| s2 Issue 4 — `team_shutdown_teammate` doesn't cancel loop | M3-T5 |
| s2 Issue 5 — `TeamStore` has no locking | M1-T3 |
| s2 Issue 6 — divergent shutdown paths | M3-T6 |
| s2 Issue 7 — `team_broadcast` partial failure | M4-T3 |
| s2 Issue 8 — `team_assign_task` no notification | M4-T2 |
| s2 Issue 9 — `team_message` no recipient validation | M4-T4 |
| s2 Issue 10 — poll loop doesn't inject messages into conversation | M4-T1, M3-T4 |
| s2 Issue 11 — plan rejection inconsistent state | M3-T6, M5-T4 |
| s2 Issue 12 — `team_idle` doesn't publish idle event | M3-T4 |
| s2 Issue 13 — `Coordinator::start_job_async` always success | M7 (track under sub-agent/orchestrator) |
| s2 Issue 14 — reconcile uses empty prompts | M2, M8-T2 |
| s2 Issue 15 — two-phase config save race | M1-T3, M2 |
| s2 Issue 16 — `resolve_team_context` scans all teams | M8-T1 |
| s2 Issue 17 — blueprint spawn errors not aggregated | M8-T2 |
| s2 Issue 18 — `TaskStore::complete` auto-claims | M6-T3 |
| s2 Issue 19 — `InProcessRouter` no retry | M7 (orchestrator) |
| s2 Issue 20 — semantic message types collapsed | M5-T6 |
| s2 Issue 21 — pre-assignment failure non-blocking | M4-T2 |
| s2 Issue 22 — stale `add_task` comment | M8-T4 |
| s2 Issue 23 — predictable temp file path | M1-T4 |
| s4 Issue 1 — TOCTOU in `ragent-team` | M1-T1, M1-T2 |
| s4 Issue 2 — duplicate team code | M2 |
| s4 Issues 3–5 — serialization / missing fields | M5-T1, M4-T5 |
| s4 Issues 6–8 — versioning / validation / correlation | M5-T2, M5-T3, M5-T4 |
| s4 Issues 9–12 — task state / timestamps | M6-T3, M5-T5 |
| s4 Issues 13–15 — dead code / ID mismatch / SSE payloads | M5-T7, M8-T5 |
| s3 §5.1/5.2 — `drain_unread` marks read too early | M4-T1 |
| s3 §5.3 — event bus drops events | M6-T4 |
| s3 §5.4 — `team_wait` timeout race | M3-T1, M3-T3 |
| s3 §5.5 — broadcast overflow | M6-T4 |
| s3 §6.2 — message types collapsed | M5-T6 |
| s3 §7.1/7.2 — SessionProcessor ↔ TeamManager cycle | M2 |
| s3 §8.1 — O(teams) resolution | M8-T1 |
| s3 §8.2 — agent ID allocation race | M1-T3 (config locking) |
| s4 Reliability §3.1–3.15 | M1, M3, M4, M6 |
| s4 Subagent review §2.2a/2.3a/2.3b/2.3c/2.5a | M7 |
| s4 Subagent review §3.1b/3.2a/3.4a/3.5a/3.6a | M3, M4, M8-T3 |

---

*End of COMMSPLAN.md.*
