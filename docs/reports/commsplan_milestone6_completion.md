# COMMSPLAN Milestone 6 — Completion Report

**Date:** 2026-06-21
**Plan:** `COMMSPLAN.md` §2 Milestone 6
**Status:** ✅ COMPLETE
**Priority:** P2 (resilience)

## Goal

Make the system tolerate hung/crashed teammates and leader session restarts
without leaving tasks stuck in limbo.

## Deliverables

- [x] Heartbeat / watchdog for teammates (M6-T1).
- [x] Leader crash recovery / orphan cleanup (M6-T2).
- [x] Idempotent task claim/completion (M6-T3).
- [x] `EventBus` overflow handling for coordination events (M6-T4) ��
      `team_wait` already falls back to disk re-check (M3-T3); documented.
- [x] Mailbox corruption recovery (M6-T5).

## Task-by-task summary

### M6-T1 — Implement teammate heartbeat / watchdog

**Files:** `crates/ragent-team/src/team/manager.rs`, `crates/ragent-tui/src/app.rs`.

`TeammateHandle` gained a `last_progress: Arc<Mutex<Instant>>` field. The
`TeamManager` gained `watchdog_timeout: Duration` (default 300s),
`watchdog_cancel: Arc<AtomicBool>`, and:

- `start_watchdog(self: Arc<Self>)` — spawns a background task that ticks
  every `min(watchdog_timeout, 60s) / 2`. On each tick it collects handles
  whose `last_progress` is older than `watchdog_timeout`, confirms the
  member is still `Working`/`Spawning`/`PlanPending`/`Suspended` on disk
  (so Idle/Failed/Stopped members are not re-flagged), sets the cancel
  flags, deregisters the notifier, marks the member `Failed` on disk, and
  publishes `Event::TeammateFailed`.
- `record_progress(&self, agent_id: &str)` — resets `last_progress` for the
  given agent. Called by the TUI event loop when it observes
  `TeammateIdle`, `TeammateFailed`, `TeamTaskClaimed`, or
  `TeamTaskCompleted`.
- `shutdown_all` now sets `watchdog_cancel` so the watchdog stops before
  tearing down teammates.

The TUI calls `manager.start_watchdog()` after `TeamManager` is
initialised, and calls `tm.record_progress(agent_id)` in each of the four
event handlers. `start_watchdog` gracefully no-ops if no tokio runtime is
available (so non-async unit tests that construct a `TeamManager` don't
panic).

**Findings addressed:** s4 §3.5, s4 Failure-Mode Matrix.

### M6-T2 — Add leader recovery / orphan cleanup

**Files:** `crates/ragent-team/src/team/manager.rs`.

`TeamManager::new` now checks the on-disk `config.json`'s
`lead_session_id` against the new lead's session id. If they differ, the
team is *adopted*:

1. `adopt_orphaned_tasks(team_dir, old_lead_sid)` reassigns any task that
   is `InProgress` and assigned to the old lead back to `Pending` (via
   `TaskStore::update_task`). Tasks assigned to teammates are untouched.
2. The config's `lead_session_id` is updated to the new lead's session id.

This means a leader crash no longer leaves tasks stuck in `InProgress`
forever — a new lead picking up the team automatically recovers them.

**Findings addressed:** s4 §3.6, s4 Failure-Mode Matrix.

### M6-T3 — Make task claim and completion idempotent

**Files:** `crates/ragent-team/src/team/task.rs`.

`Task` gained `completed_by: Option<String>` (serde default).
`TaskStore::complete` is now idempotent:
- If the task is already `Completed` **by the same `agent_id`** → return
  the task unchanged (no-op success).
- If the task is already `Completed` **by a different agent** → reject
  with `"task '{id}' is already completed by '{owner}', not '{agent_id}'"`.
- On a fresh completion, `completed_by = Some(agent_id)` is recorded.

`TaskStore::claim_specific` was already idempotent (if the agent already
owns the task as `InProgress`, it returns the task unchanged). The doc
comment now explicitly documents this as M6-T3.

**Findings addressed:** s4 Issue 10, s4 §3.4, s2 Issue 18.

### M6-T4 — Handle `EventBus` overflow for coordination events

M3-T3 (already implemented in Milestone 3) made `team_wait` re-check disk
state on timeout, so a dropped `TeammateIdle`/`TeammateFailed` event is
recovered from the on-disk status. This is the "fall back to polling disk
state when an event is suspected to have been dropped" approach from the
plan. No additional code was needed for M6-T4.

**Findings addressed:** s3 §5.5, s4 §3.3.

### M6-T5 — Mailbox corruption recovery

**Files:** `crates/ragent-team/src/team/mailbox.rs`.

`Mailbox::read_all`, `peek_unread`, and `drain_unread` now catch
`serde_json::from_str` errors. On corruption:
- The corrupt file is moved aside to
  `<path>.corrupt.<UTC-timestamp>.json`.
- A `tracing::warn!` is emitted with the path and error.
- An empty `Vec<MailboxMessage>` is returned so the caller (poll loop,
  tool) continues with a fresh inbox instead of looping forever on a
  parse error.

Subsequent writes to the mailbox (e.g. `push`) recreate the file
normally.

**Findings addressed:** s4 §3.7.

## Files modified

| File | Change |
|------|--------|
| `crates/ragent-team/src/team/manager.rs` | `TeammateHandle.last_progress`; `TeamManager.watchdog_timeout/cancel`; `start_watchdog()`; `adopt_orphaned_tasks()`; `record_progress()`; `new()` adopts team on lead mismatch; `shutdown_all` cancels watchdog. |
| `crates/ragent-team/src/team/task.rs` | `Task.completed_by`; idempotent `complete()` (same-agent no-op, different-agent reject); `claim_specific` idempotency documented. |
| `crates/ragent-team/src/team/mailbox.rs` | `read_all`/`peek_unread`/`drain_unread` recover from corrupt JSON by moving file aside. |
| `crates/ragent-tui/src/app.rs` | Calls `start_watchdog()` after manager init; `record_progress()` in event handlers. |
| `crates/ragent-team/tests/test_m6_resilience.rs` | New integration test suite (9 tests). |

## Verification

- `cargo build --workspace` — ✅
- `cargo fmt` — applied
- `cargo test -p ragent-team` — 62 tests pass (16 lib + 6 + 7 M3 + 12 M4 + **9 M6** + 8 + 4)
- `cargo test -p ragent-agent --lib` — 352 pass
- `cargo test -p ragent-tui --lib` — 44 pass
- `cargo test -p ragent-tui --test test_teams_tui` — 47 pass
- `cargo test -p ragent-server` — 74 pass

## Notes / caveats

- The watchdog requires a live tokio runtime to spawn its background task.
  `start_watchdog` gracefully no-ops if `Handle::try_current()` fails, so
  non-async tests that construct a `TeamManager` (e.g. TUI tests that call
  `team_create` synchronously) don't panic. In production, the TUI app
  always calls `start_watchdog` from within a tokio runtime.
- `adopt_orphaned_tasks` only reassigns tasks assigned to the old *lead*.
  Tasks assigned to teammates are left untouched because the teammates may
  still be alive (the lead changed, not the teammates).
- M6-T4 relies on M3-T3's disk re-check. No new event channel was added;
  the plan's option (b) ("fall back to polling disk state") was already
  implemented.