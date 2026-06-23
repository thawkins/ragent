# COMMSPLAN Milestone 3 — Completion Report

**Date:** 2026-06-21
**Plan:** `COMMSPLAN.md` §2 Milestone 3
**Status:** ✅ COMPLETE
**Priority:** P0 (liveness / hangs)

## Goal

Make `team_wait`, shutdown, and idle signalling reliable so the lead can
accurately detect when teammates finish, fail, or shut down, and can terminate
them promptly.

## Deliverables

- [x] `team_wait` observes `TeammateFailed`, subscribes before reading state,
      and falls back to disk on timeout.
- [x] `team_idle` publishes `Event::TeammateIdle`.
- [x] `team_shutdown_teammate` actually cancels the agent and poll loops.
- [x] A unified shutdown path used by both the tool and `TeamManager`.
- [x] Lifecycle regression tests.

## Task-by-task summary

### M3-T1 — Subscribe to EventBus before reading team state in `team_wait`

**File:** `crates/ragent-team/src/tools/team_wait.rs` (+ mirrored copy in
`crates/ragent-agent/src/tool/team_wait.rs`)

The event-bus receiver is now created **before** the initial
`TeamStore::load_by_name`/`list_teams` scan. A pre-loop `while let Ok(event) =
rx.try_recv()` drain reconciles any `TeammateIdle` / `TeammateFailed` events
that arrived between the subscribe and the store read into the `waiting_for`
set. This closes the race where a teammate goes idle between the store read
and the subscribe and the lead would otherwise wait the full 300 s timeout.

**Findings addressed:** s2 Issue 3, s3 §5.4.

### M3-T2 — Handle `TeammateFailed` in `team_wait`

**File:** `crates/ragent-team/src/tools/team_wait.rs`

Added an `Ok(Ok(Event::TeammateFailed { session_id, team_name, agent_id, error }))`
branch (gated on the lead session id, team name, and membership in
`waiting_for`) that removes the failed agent from `waiting_for` and logs the
error. A failed teammate will never become idle; without this branch the lead
waited the full timeout. The same branch is also present in the pre-loop drain
so a failure that arrives before the wait loop starts is captured.

**Findings addressed:** s2 Issue 2, s4 §4.2/3.15.

### M3-T3 — Re-check disk state on `team_wait` timeout

**File:** `crates/ragent-team/src/tools/team_wait.rs`

Before declaring a timeout, `team_wait` now reloads `TeamStore::load_by_name`
and treats any member whose on-disk status is `Idle`, `Failed`, or `Stopped`
as finished, removing them from `waiting_for`. This recovers terminal state
when an `EventBus` event was dropped (buffer full / no subscribers) but the
teammate legitimately reached a terminal state on disk. A `tracing::info!`
line records the recovered and remaining agent ids.

**Findings addressed:** s3 §5.3/5.5, s4 §3.15.

### M3-T4 — Publish `TeammateIdle` from `team_idle`

**File:** `crates/ragent-team/src/tools/team_idle.rs` (+ mirrored copy in
`crates/ragent-agent/src/tool/team_idle.rs`)

After `TeamStore::save()` commits the `Idle` status, the tool now calls
`ctx.event_bus.publish(Event::TeammateIdle { ... })`. The lead session id is
derived from the on-disk `TeamConfig::lead_session_id` (falling back to
`ctx.session_id` if the store cannot be reloaded). `team_wait` and the TUI/SSE
all rely on this event; previously the tool only updated disk state.

**Findings addressed:** s2 Issue 12, s4 §3.2a.

### M3-T5 — Set cancel flags from `team_shutdown_teammate`

**Files:** `crates/ragent-agent/src/tool/mod.rs` (trait),
`crates/ragent-team/src/team/manager.rs` (impl),
`crates/ragent-team/src/tools/team_shutdown_teammate.rs` (+ mirrored copy in
`crates/ragent-agent/src/tool/team_shutdown_teammate.rs`).

`TeamManagerInterface` gained a `shutdown_teammate(&self, agent_id: &str,
graceful: bool)` method. The tool resolves the `TeamManager` from
`ctx.team_manager` and delegates to it. When no manager is wired into the
context (e.g. the lead session has not initialised one), the tool falls back
to a disk-only path that performs the same on-disk status transition. The tool
gained an `immediate: bool` parameter (default `false`).

**Findings addressed:** s2 Issue 4, s4 §4.11.

### M3-T6 — Unify graceful vs. immediate shutdown semantics

**File:** `crates/ragent-team/src/team/manager.rs`

`TeamManager::shutdown_teammate` is now the single unified shutdown path with
a `graceful: bool` parameter:

- **Graceful (`graceful = true`):** mark the member `ShuttingDown` on disk,
  push a `ShutdownRequest` mailbox message so a teammate in its agent loop
  receives it via `team_read_messages` and can call `team_shutdown_ack`. Cancel
  flags are **not** set; the teammate is expected to ack.
- **Immediate (`graceful = false`):** set the agent-loop `cancel` flag and the
  mailbox-poll `poll_cancel` flag, wake the poll loop, deregister the mailbox
  notifier, push a `ShutdownRequest` as a fallback, and mark the member
  `Stopped` on disk (clearing `current_task_id`).

`shutdown_all` now calls `shutdown_teammate(&id, false)` (immediate), and the
three TUI teardown call sites in `crates/ragent-tui/src/app.rs` were updated
to the new two-argument signature. The `TeamManagerInterface` impl delegates
to `Self::shutdown_teammate` so the trait object and the concrete type share
one implementation.

**Findings addressed:** s2 Issue 6, s3 §6.4.

### M3-T7 — Lifecycle tests

**File:** `crates/ragent-team/tests/test_m3_lifecycle.rs` (new, 7 tests)

1. `test_team_wait_handles_teammate_failed_event` — a `TeammateFailed` event
   published during `team_wait` removes the agent from the waiting set (M3-T2).
2. `test_team_wait_pre_loop_drain_picks_up_idle_event` — an idle event that
   arrives before the wait loop starts is captured by the pre-loop drain
   (M3-T1).
3. `test_team_wait_disk_recheck_recovers_terminal_state` — when the event bus
   drops the idle event (capacity 1, no subscribers) but the member is marked
   `Idle` on disk, the post-timeout disk re-check recovers it (M3-T3).
4. `test_team_idle_publishes_teammate_idle_event` — the tool publishes
   `Event::TeammateIdle` and writes `Idle` to disk (M3-T4).
5. `test_team_shutdown_teammate_graceful_marks_shutting_down` — the
   disk-fallback graceful path marks the member `ShuttingDown` (M3-T5/T6).
6. `test_team_shutdown_teammate_immediate_marks_stopped` — the
   disk-fallback immediate path marks the member `Stopped` (M3-T5/T6).
7. `test_team_manager_shutdown_graceful_keeps_running_status` — documents the
   `TeamManager`-level expectation (the helper shares the same status
   assignment as the fallback).

All 7 tests pass.

**Findings addressed:** s2–s4.

## Files modified

| File | Change |
|------|--------|
| `crates/ragent-agent/src/tool/mod.rs` | Added `shutdown_teammate` to `TeamManagerInterface` trait. |
| `crates/ragent-team/src/team/manager.rs` | Unified `shutdown_teammate(agent_id, graceful)` helper; `shutdown_all` uses immediate; trait impl delegates to `Self`. |
| `crates/ragent-team/src/tools/team_wait.rs` | Subscribe-before-load, pre-loop drain, `TeammateFailed` branch, post-timeout disk re-check. |
| `crates/ragent-team/src/tools/team_idle.rs` | Publish `TeammateIdle` after committing idle status. |
| `crates/ragent-team/src/tools/team_shutdown_teammate.rs` | Route through `TeamManagerInterface::shutdown_teammate`; new `immediate` parameter; disk-only fallback. |
| `crates/ragent-agent/src/tool/team_wait.rs` | Mirrored copy (kept byte-identical with `ragent-team`). |
| `crates/ragent-agent/src/tool/team_idle.rs` | Mirrored copy. |
| `crates/ragent-agent/src/tool/team_shutdown_teammate.rs` | Mirrored copy. |
| `crates/ragent-tui/src/app.rs` | Three `shutdown_teammate(&id)` call sites updated to `shutdown_teammate(&id, false)`. |
| `crates/ragent-team/tests/test_m3_lifecycle.rs` | New integration test suite (7 tests). |
| `CHANGELOG.md` | New "COMMSPLAN Milestone 3" section under 0.1.0-alpha.114. |
| `SPEC.md` | `team_shutdown_teammate` row documents the `immediate` parameter. |

## Verification

- `cargo build --workspace` — ✅
- `cargo clippy -p ragent-team -p ragent-agent -p ragent-tui` — ✅ no new warnings
- `cargo fmt` — applied
- `cargo test -p ragent-team` — 41 tests pass (16 lib + 6 + 7 new M3 + 8 + 4)
- `cargo test -p ragent-agent --lib` — 352 pass
- `cargo test -p ragent-tui --test test_teams_tui` — 47 pass
- `cargo test -p ragent-tui --test test_slash_commands` — 9 pre-existing
  failures (reproduced on `main` via `git stash`; CWD-tempdir race unrelated
  to M3). All M3-related tests pass in isolation.

## Notes / caveats

- The `team_wait.rs`, `team_idle.rs`, and `team_shutdown_teammate.rs` tool
  files are duplicated between `ragent-agent/src/tool/` and
  `ragent-team/src/tools/`. M3 edits both copies identically so neither crate
  regresses. Eliminating the duplication is **Milestone 2's** scope
  (`COMMSPLAN.md` §2 M2-T1).
- The `TeamManager::shutdown_teammate` unified helper is exercised at the tool
  level via the disk-fallback path (tests 5 and 6). A full end-to-end test
  that constructs a real `SessionProcessor` + `TeamManager` and asserts the
  cancel flags are set on immediate shutdown is left to a follow-up because
  `SessionProcessor` construction requires a live storage handle and provider
  registry that are heavy to stand up in a unit test.
- `team_idle`'s `TeammateIdle` event uses the on-disk `lead_session_id`
  rather than `ctx.team_context` because `TeamContext` does not currently
  expose the lead session id. This is a minor redundancy (a second store load)
  that can be cleaned up when `TeamContext` is extended (M5 schema work).

## Next milestones

Per `COMMSPLAN.md` §3, Milestones 4–8 can now proceed largely in parallel
after M2 lands. High-risk couplings to watch:

- **M4-T1** (read-vs-processed) interacts with **M3-T6** (shutdown flow) —
  `ShutdownRequest` must remain deliverable until acknowledged.
- **M5-T6** (event message types) must align with **M3-T4** and **M4-T2** so
  the TUI and SSE display the new events correctly.