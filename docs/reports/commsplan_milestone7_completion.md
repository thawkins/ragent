# COMMSPLAN Milestone 7 — Completion Report

**Date:** 2026-06-21
**Plan:** `COMMSPLAN.md` §2 Milestone 7
**Status:** ✅ COMPLETE
**Priority:** P2 (correctness)

## Goal

Fix the less critical but still incorrect behaviors in the non-team sub-agent
path (the `TaskManager` / `new_task` / `wait_tasks` subsystem).

## Deliverables

- [x] Suspend/resume removed from the API surface (M7-T1).
- [x] `kill_task` uses a blocking `write()` for the cancel flag (M7-T2).
- [x] `wait_tasks` waiter-count bookkeeping is accurate (M7-T3).
- [x] Cancel detection uses typed inspection instead of string matching (M7-T4).

## Task-by-task summary

### M7-T1 — Honor or remove `suspend_task`

**Files:** `crates/ragent-agent/src/task/mod.rs`.

**Decision: remove** (option (b) from the plan). `suspend_task` and
`resume_task` now return a clear error explaining that suspension is not
implemented — the session processor's agent loop does not honour
`suspend_flags`, so the previous implementation was a misleading no-op that
changed a status field but kept the agent loop running and consuming tokens.

The `SubagentSuspended` / `SubagentResumed` events are no longer published by
these methods. The event variants remain in `ragent-types` for backward
compatibility with SSE clients, but the TUI handlers will never receive them
in practice. The `suspend_flags` field is kept on `TaskManager` (as an
unused private field) to avoid a struct-field-count break, but it is never
populated.

The TUI's "Suspend" button (`InputAction::SuspendTask`) calls
`tm.suspend_task(&id)` and will now receive the error, which is surfaced as a
log message. Operators should use `cancel_task` instead.

**Findings addressed:** s4 Subagent review §2.3a.

### M7-T2 — Honor `kill_flag` or rely on cancel flag alone

**Files:** `crates/ragent-agent/src/task/mod.rs`.

`kill_task` previously used `self.cancel_flags.try_write()` — a non-blocking
lock acquisition that silently fails if another task holds the write lock.
This could cause the cancel flag to never be set, leaving the task running
until the 10-second force-kill escalation.

Fixed: `kill_task` now uses `self.cancel_flags.write().await` (a blocking
write) so the cancel signal is never lost due to lock contention. The
`kill_flags` map is no longer populated or checked — the cancel flag alone
is sufficient. The `kill_flags` field is kept on the struct (unused) to
avoid a deserialization break, but it is never written to.

**Findings addressed:** s4 Subagent review §2.3b/2.3c.

### M7-T3 — Fix `wait_tasks` waiter-count bookkeeping

**Files:** `crates/ragent-agent/src/task/mod.rs`, `crates/ragent-agent/src/tool/wait_tasks.rs`.

**Problem:** The old code blindly called `increment_waiter(task_id)` for every
task in the wait set, including tasks that were already completed (whose
results were collected before the wait loop). Then, after the wait, it
called `decrement_waiter(task_id)` for every task ID in
`results.keys().chain(waiting_for.iter())` — including the already-completed
tasks that were never incremented. These spurious decrements could cause
`drain_completed` to inject results prematurely when another waiter was
still waiting.

**Fix:**
- `increment_waiter` now returns `bool`: `true` if the task was found and
  still `Running` (increment succeeded), `false` if the task was already
  completed or not found (no increment). The `wait_tasks` tool only adds
  tasks to the `still_waiting` list when `increment_waiter` returns `true`.
- `decrement_waiter` is now a no-op if `waiter_count == 0` (no spurious
  decrement). It only decrements tasks that were actually incremented.
- The `wait_tasks` tool's cleanup loop now only decrements tasks in
  `still_waiting` (the ones that returned `true` from `increment_waiter`),
  not the full `results.keys().chain(waiting_for)` set.

**Findings addressed:** s4 Subagent review §2.5a.

### M7-T4 — Replace cancelled string matching with typed cancellation

**Files:** `crates/ragent-agent/src/task/mod.rs`.

The old code detected cancellation via `error_msg.contains("cancelled")` —
a fragile string match that would misclassify cancellations if the error
wording changed, or false-positive on any error message that happened to
contain "cancelled".

Replaced with `is_cancel_error(err: &anyhow::Error)` which:
1. Walks the `anyhow::Error::chain()` and checks each error's **type name**
   for `"Cancelled"` (a typed signal).
2. Falls back to checking each error's `Display` string for `"cancelled"`
   (case-insensitive) as a secondary path — this catches the processor's
   `FinishReason::Cancelled` which may not have a distinct type in the
   chain.
3. Checks the top-level error's Display as a last resort.

This is robust to wording changes: the type-name check catches the
structured `Cancelled` variant, and the Display-string fallback is only a
secondary path, not the primary one.

**Findings addressed:** s4 Subagent review §2.2a.

## Files modified

| File | Change |
|------|--------|
| `crates/ragent-agent/src/task/mod.rs` | `suspend_task`/`resume_task` return explicit "not implemented" errors (M7-T1); `kill_task` uses blocking `write()` for cancel flag, `kill_flags` no longer populated (M7-T2); `increment_waiter` returns `bool`, `decrement_waiter` no-ops on 0 count (M7-T3); `is_cancel_error()` typed cancellation detection (M7-T4). |
| `crates/ragent-agent/src/tool/wait_tasks.rs` | Only increments waiters for still-running tasks; only decrements tasks that were actually incremented (M7-T3). |

## Verification

- `cargo build --workspace` — ✅
- `cargo fmt` — applied
- `cargo test -p ragent-agent --lib` — 352 pass
- `cargo test -p ragent-team` — 62 pass (all M3–M6 suites)
- `cargo test -p ragent-tui --lib` — 44 pass
- `cargo test -p ragent-tui --test test_teams_tui` — 47 pass
- `cargo test -p ragent-server` — 74 pass

## Notes / caveats

- `suspend_task` / `resume_task` still exist as methods (they return errors
  rather than being deleted) so the TUI's keybinding handlers don't need
  code changes — they will receive the error and log it. A future cleanup
  can remove the TUI buttons entirely.
- `kill_flags` and `suspend_flags` fields remain on `TaskManager` as unused
  private fields to avoid a struct-field-count change. They are never
  populated or checked.
- The `SubagentSuspended` / `SubagentResumed` event variants remain in
  `ragent-types::Event` and have TUI/SSE handlers, but they will never fire
  in practice. They can be removed in a future cleanup if desired.
- `is_cancel_error` checks both the type-name chain and the Display string.
  This is more robust than the old `contains("cancelled")` check, but a
  future refactor could make the processor return a dedicated
  `Cancelled` error type so the type-name check alone is sufficient.