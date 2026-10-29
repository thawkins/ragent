---
spec_id: teamssetup
status: draft
audit:
  - { time: 1793241600, from: "none", to: "draft", actor: "system" }
---
# Implementation Plan: Team Runtime Hardening

## Overview

This plan turns the 45 requirements in `SPEC.md` into 43 tasks across nine
dependency-ordered milestones. The ordering is deliberately bottom-up: the
status machine (M1) and the unified task transitions (M2) are prerequisites for
almost everything else, because most findings are symptoms of "the same rule is
written three times".

The three design decisions below define the target shape. Every task either
builds one of them or consumes it.

### Design decision D1 - One member-status writer

`TeamManager::set_member_status(agent_id, new_status, cause) -> Result<()>` is
the only function that writes `TeamMember::status`. It:

1. loads the store under the existing `flock`,
2. validates the transition with `MemberStatus::can_transition_to`,
3. writes the status plus any status-specific side effect (clear
   `last_spawn_error` on recovery, set `shutdown_request_id` on
   `ShuttingDown`),
4. saves,
5. publishes exactly one `Event::TeammateStatusChanged` with
   `{ agent_id, from, to, cause }`.

Every current write site - `manager.rs` (spawn, suspend, resume, shutdown,
watchdog, plan approval), the three task tools, `swarm.rs`, and the TUI event
handler - is replaced by a call to it. This alone fixes FR-008, FR-035, and the
mechanical cause of FR-012.

### Design decision D2 - Three status predicates

On `MemberStatus`:

- `is_terminal()` - `Stopped | Failed`. No forward transition except respawn.
- `is_parked()` - `Idle | Suspended | PlanPending | Blocked`. Alive but not
  attached to work it must complete on its own; not eligible for a watchdog
  verdict.
- `is_live()` - `Spawning | Working | ShuttingDown`. Has an outstanding action
  the session must finish by itself (finish starting, finish the task,
  acknowledge shutdown).

`Idle` is **parked**, not live. A teammate with no task cannot be wedged - it
waits for an assignment the lead will supply - so the watchdog must not fail it.
The current watchdog already agrees: `manager.rs:1555` fails only
`Working | Spawning | PlanPending | Suspended`, and `Idle` is absent from that
list. Classifying `Idle` as live would silently add it to the failable set and
reverse an existing correct decision.

Every guard in the codebase that currently spells a status list becomes one of
these three, and every status satisfies exactly one of them (asserted by
`test_team_member_status::test_member_status_predicates_are_mutually_exclusive`).
FR-009, FR-012, and FR-022 all reduce to picking the right predicate.

### Design decision D3 - One task-transition contract

`TaskStatus::can_transition_to` plus a single `transition_task` helper that
validates, applies, persists, and fires hooks. `TaskStore` and `TeamManager`
both delegate. The legal table:

| From | To | Allowed |
| --- | --- | --- |
| `Pending` | `InProgress` | yes |
| `Pending` | `Cancelled` | yes |
| `InProgress` | `Completed` | yes |
| `InProgress` | `Pending` (release) | yes, owner or lead only |
| `InProgress` | `Cancelled` | yes, lead only |
| `Completed` | anything | no |
| `Cancelled` | anything | no |

`Completed` and `Cancelled` are terminal. Completion requires ownership
(FR-001). This fixes FR-002, FR-003, FR-004, FR-005, FR-006, FR-018, FR-025.

### Milestone map

| Milestone | Theme | Tasks | Exit criterion |
| --- | --- | --- | --- |
| M1 | Status machine foundation | T-001..T-005 | D1 and D2 exist, all write sites routed, no behaviour change yet |
| M2 | Task transition contract | T-006..T-013 | D3 exists, ownership and cancellation enforced, one claim and one complete path |
| M3 | Watchdog and parked states | T-014..T-017 | Parked members survive; `Blocked` has an exit |
| M4 | Configuration honouring | T-018..T-022 | Every `TeamSettings` field has a consumer |
| M5 | Hooks on the runtime path | T-023..T-026 | Hooks fire from the manager; failures are visible |
| M6 | Lifecycle guards | T-027..T-029 | Cleanup and delete share one rule; adoption is explicit |
| M7 | Mailbox and robustness | T-030..T-033 | Undelivered messages reported, malformed state diagnosed |
| M8 | UI and discoverability | T-034..T-037 | Popup and slash surfaces expose the new verbs |
| M9 | Documentation and rollout | T-038..T-043 | Text matches behaviour, CHANGELOG entries land |

## Tasks

| ID | Title | Requirement | Effort | Priority | Status | Dependencies |
|----|-------|-------------|--------|----------|--------|--------------|
| T-001 | Add `is_terminal`, `is_parked`, `is_live` to `MemberStatus` and `can_transition_to` with the legal transition table | FR-009, FR-003 | S | Critical | completed | — |
| T-002 | Add `MemberStatus::transition_cause` type and `Event::TeammateStatusChanged` with `{agent_id, from, to, cause}` | FR-035 | S | High | completed | T-001 |
| T-003 | Implement `TeamManager::set_member_status` as the single status writer with validation, side effects, save, and one event | FR-008, FR-035 | M | Critical | completed | T-001, T-002 |
| T-004 | Route all nine `TeamMember::status` write sites in `manager.rs` (spawn, suspend, resume, shutdown, watchdog, approve_plan) through `set_member_status` | FR-008, FR-026 | M | Critical | completed | T-003 |
| T-005 | Route the remaining write sites in `team_*.rs` tools, `team/swarm.rs`, and `app/event_handler.rs:1816` through `set_member_status`; delete the direct writes | FR-008, FR-026 | M | Critical | completed | T-004 |
| T-006 | Implement `TaskStatus::can_transition_to` with the D3 legal table | FR-003, FR-004 | S | Critical | completed | T-001 |
| T-007 | Implement shared `transition_task(list, id, to, actor) -> Result<Task>` validating via FR-003/FR-004 and returning the naming error of FR-004 | FR-003, FR-004 | M | Critical | completed | T-006 |
| T-008 | Rewrite `TaskStore::complete` and `TeamManager::complete_task` as delegations to one implementation enforcing FR-001 ownership | FR-001, FR-002, FR-025 | M | Critical | completed | T-007 |
| T-009 | Rewrite `TaskStore::claim_next`/`claim_specific` and `TeamManager::claim_next_task` as delegations to one claim implementation built on `is_claimable` | FR-007, FR-025, FR-026 | M | High | completed | T-007 |
| T-010 | Make every claim path use the single claimability predicate and refuse `Cancelled` with a "cancelled" message | FR-006, FR-007 | S | High | completed | T-009 |
| T-011 | Add `TaskStatus::Cancelled` producer: `TaskStore::cancel`, `TeamManager::cancel_task`, and mailbox notification to the owning teammate | FR-005, FR-006 | M | High | completed | T-007 |
| T-012 | Add the `team_task_cancel` tool, its registry entry, permission category, and tool description naming the lead-only rule | FR-005, FR-042 | S | Medium | completed | T-011 |
| T-013 | Emit `Event::TaskCompletionRejected` when FR-001 refuses a completion the previous release accepted | FR-044 | S | Medium | completed | T-008 |
| T-014 | Make the watchdog eligibility check `is_live()` so `Suspended`, `PlanPending`, and `Blocked` members are never failed | FR-012, FR-009 | S | Critical | completed | T-001 |
| T-015 | Add the explicit `Blocked`/`ShuttingDown` branches to the watchdog so the `unwrap_or(false)` implicit path is removed | FR-012 | S | High | completed | T-014 |
| T-016 | Extend `resume_teammate` to accept `Blocked` as well as `Suspended`, clearing the block reason via `set_member_status` | FR-010 | M | Critical | completed | T-003, T-014 |
| T-017 | Auto-unblock: when a task completes, transition every `Blocked` member whose last unmet dependency is now satisfied to `Idle` and publish `TeammateResumed` | FR-011 | M | High | completed | T-016, T-008 |
| T-018 | Gate write-class and bash-class tool calls on `require_plan_approval` via the existing `is_plan_pending` gate, with a refusal naming `team_submit_plan` | FR-014 | M | Critical | completed | T-003 |
| T-019 | Implement `auto_claim_tasks`: on the `Idle` transition, claim the next task when `true` and hold the member idle when `false` | FR-015 | M | High | completed | T-009, T-003 |
| T-020 | Make the teammate system prompt match `auto_claim_tasks` so the prompt states the enforced behaviour | FR-027 | S | Medium | completed | T-019 |
| T-021 | Move the `max_teammates` check out of the `existing.is_none()` guard so a respawn is counted | FR-016 | S | High | completed | — |
| T-022 | Add `TeamSettings.watchdog_timeout_secs` with a 300-second default and wire it into the watchdog interval calculation | FR-013 | S | Medium | completed | T-014 |
| T-023 | Fire `TaskCreated`/`TaskClaimed`/`TaskCompleted` from `transition_task` rather than from the three tools, and drop the tool-side calls | FR-017 | M | High | completed | T-007 |
| T-024 | Fire `TeammateSpawned`/`TeammateStopped` from `set_member_status` | FR-017 | S | Medium | completed | T-003 |
| T-025 | Deliver `HookOutcome::Feedback` from the runtime path to the completing agent as a tool result | FR-018 | M | Medium | completed | T-023 |
| T-026 | Add the append-only `hook-failures.jsonl` ledger with 1 MiB rotation, written on validation refusal, spawn failure, and timeout | FR-019, NFR-004 | M | Medium | completed | T-023 |
| T-027 | Replace the three divergent cleanup guards with `is_live() | ShuttingDown` in `/team cleanup`, `/team delete`, and `team_cleanup`, sharing one refusal text | M | Medium | completed | Pending |
| T-028 | Add a `force` confirmation step to `/team delete` so deleting a team with live members requires an explicit second action | FR-023, FR-030 | S | High | completed | T-027 |
| T-029 | Make `TeamManager::new` refuse a foreign `lead_session_id` unless `adopt: true`, and on adoption reset every orphaned `InProgress` task regardless of prior owner plus publish `TeamAdopted` | FR-023, FR-024 | M | High | completed | T-007 |
| T-030 | Report undelivered messages in `/team show` and `team_status` instead of dropping them when a recipient terminates | FR-020 | M | Medium | completed | T-003 |
| T-031 | Confirm and pin the oversized-message refusal already implemented at `mailbox.rs:402`: add a regression test asserting the error names the limit and the actual size, and document the limit in the `team_message` tool description | FR-021 | S | Low | completed | — |
| T-032 | Replace the `TaskList::default()` parse fallback with an error naming the file, byte offset, and offending field | FR-037 | S | High | completed | T-007 |
| T-033 | Validate `TaskList` after every write and refuse to persist an invalid list; detect dangling `depends_on` and dependency cycles at write time | FR-038, FR-039 | M | High | completed | T-032 |
| T-034 | Extend `team_wait` to report each awaited member as idle, parked, failed, or still-running at timeout | FR-031 | M | Medium | completed | T-001 |
| T-035 | Add the `Teams` popup hint row and the `Del` resume path for `Blocked` members, with a confirmation naming the member and action | FR-028, FR-029 | M | High | completed | T-016 |
| T-036 | Show the pending plan and the approve/reject action on a `PlanPending` popup row | FR-034 | S | Medium | completed | T-003 |
| T-037 | Audit every team refusal string for entity, current state, and next action; route every status word through `display_label()` | FR-032, FR-033 | M | Medium | completed | T-005 |
| T-038 | Add a `/team tasks` cancel/reassign action that refuses on a live owner without confirmation | FR-005, FR-030 | M | Medium | completed | T-012, T-027 |
| T-039 | Extend `/team help` with per-subcommand active-team requirement, disk mutation, and refusing guard; print effective `TeamSettings` values in `/team show`; add per-member status history to `team_status` | FR-040, FR-041, FR-036 | M | Medium | completed | T-027, T-005 |
| T-040 | Update the `team_task_complete` and `team_cleanup` tool descriptions, and author the two CHANGELOG behaviour-change entries with an operator migration note | FR-042, FR-043 | S | Medium | completed | T-008, T-027 |
| T-041 | Add `forcecleanup` to the `/team help` table and the `COMMANDS` catalogue, and make it share the FR-022 guard plus an explicit `force: true` equivalent so it cannot bypass the tool's rule | FR-040, FR-022 | S | High | completed | T-027, T-039 |
| T-042 | Make every popup row-action dialog state which option performs and which cancels the action, and add the toggle hint to the popup footer | FR-028, FR-032 | S | Medium | completed | T-035 |
| T-043 | Require an explicit confirmation for `/team delete` on every named team, not only the active one | FR-045, FR-030 | M | Critical | completed | T-027 |
## Task Detail

### M1 - Status machine foundation (T-001..T-005)

The foundation milestone changes no observable behaviour: it moves every direct
write onto one function so M3, M6, and M8 can each change a rule in one place.

- **T-001** adds to `crates/ragent-agent/src/team/config.rs`:
  three predicates and `can_transition_to`. `can_transition_to` encodes
  `Spawning -> Working | Failed`, `Working -> Idle | PlanPending | Blocked |
  Suspended | ShuttingDown | Failed`, `Idle -> Working | Blocked | Suspended |
  ShuttingDown | Failed`, `PlanPending -> Working | Idle | Failed`,
  `Blocked -> Idle | Suspended | ShuttingDown | Failed`, `Suspended -> Idle |
  ShuttingDown | Failed`, `ShuttingDown -> Stopped | Failed`, and no exit from
  `Stopped` or `Failed` except a respawn (`Failed -> Spawning`).
- **T-005** touches `crates/ragent-tui/src/app/event_handler.rs:1816` and
  `crates/ragent-tui/src/app/swarm.rs:159`. The TUI must reach the manager
  rather than writing `config.json` itself; where the TUI has no manager handle,
  the write is replaced by a manager method called over the existing interface.

Verify: `/team show` output before and after M1 is identical for a scripted
spawn/suspend/resume/shutdown sequence.

**T-002 as built.** FR-035 is served by one new type and one new event:

- `TransitionCause` (`config.rs`, re-exported from `ragent_agent::team`) - the
  closed vocabulary of reasons a writer moves a member: `Spawned`, `SpawnFailed`,
  `TaskClaimed`, `TaskCompleted`, `ReportedIdle`, `PlanSubmitted`,
  `PlanApproved`, `PlanRejected`, `DependencyBlocked`, `DependencySatisfied`,
  `Suspended`, `Resumed`, `ShutdownRequested`, `ShutdownAcked`,
  `WatchdogTimeout`, `TaskCancelled`, `TeamCleanedUp`. One variant per *reason a
  writer moves a member*, not one per edge - several causes share an edge, so the
  cause is always passed in by the caller and never inferred from the status
  pair. `as_str()` is the canonical `snake_case` token, and the serde
  representation matches it (asserted).
- `MemberStatus::transition_cause(from, to) -> TransitionCause` - a deliberately
  lossy best-match used where the cause is not known (a legacy write, a
  deserialised history row) and by tests asserting the vocabulary totals over the
  transition table. The two obvious failure edges are distinguished:
  `Spawning -> Failed` is `SpawnFailed`, every other `-> Failed` is
  `WatchdogTimeout`.
- `Event::TeammateStatusChanged { session_id, team_name, agent_id, from, to,
  cause }` (`ragent-types`) - the complete record. The status and cause fields
  are the canonical tokens as `String`, not the typed enums: `MemberStatus` and
  `TransitionCause` live in `ragent-agent`, which depends on `ragent-types`, so
  carrying them would invert the dependency. The SSE surface maps it to
  `teammate_status_changed`.

Covered by `crates/ragent-agent/tests/test_team_status_changed_event.rs` (8
tests: token uniqueness and `snake_case` shape, serde round trip, every legal
edge derives a cause, the intuitive cause for the twelve common edges, the wire
variant name, the full payload round trip, and `session_id()` routing) and
`crates/ragent-server/tests/test_sse_event_variants.rs::test_teammate_status_changed`.
The coarse events (`TeammateIdle`, `TeammateFailed`, `TeammateSuspended`,
`TeammateResumed`) are left in place: existing consumers still read them, and
T-003 adds the single new event alongside them.

**T-003 as built.** FR-008 and FR-035 are served by one writer on the manager,
plus one narrow companion for the fields that ride alongside a status change:

- `TeamManager::set_member_status(agent_id, to, cause) ->
  Result<Option<StatusTransition>>` (`team/manager.rs`) is the only permitted
  writer of `TeamMember::status`. It validates against
  `MemberStatus::can_transition_to`, applies the side effects the destination
  implies, persists the roster once with `TeamStore::save`, and publishes
  exactly one `Event::TeammateStatusChanged`. The refusal names the member, the
  team, the current status, the requested status, and the cause:
  `teammate 'tm-001' in team 't' is spawning and cannot become idle
  (cause: reported_idle); see the member status transition table`.
- An **unknown `agent_id` is an error**, not a silent no-op: a caller that
  addresses a member which is not in the roster is a bug, and swallowing it
  hides the bug.
- A write whose `to` equals the status the member already holds is **not a
  transition**: `Ok(None)`, no side effect, no `config.json` write, no event.
  This is what keeps a re-asserting caller (`team_idle` on an already-idle
  member, a respawn recording the status a member already holds) from making
  the event stream claim that something moved, and it is why FR-035's "exactly
  one event" holds rather than "one per call".
- **Side effects derived from the destination**, not enumerated per call site:
  `Idle` / `Stopped` / `Failed` clear `current_task_id` (the member holds no
  work); leaving `PlanPending` clears `plan_request_id` (the correlated request
  has been answered and must not be copied by a later reply); reaching
  `Stopped` clears `shutdown_request_id` (same, for the shutdown request). The
  write also drops the PERF-027 `is_plan_pending` cache entry, because a status
  move can change that answer and the cache must not outlive it by a TTL.
- `StatusTransition { from, to, cause }` (re-exported from `ragent_agent::team`)
  is the record the event carries, returned so a caller can log or assert on
  what moved without subscribing to the bus.
- `TeamManager::update_member(agent_id, |&mut TeamMember|)` is the escape hatch
  T-004 and T-005 need: the fields that accompany a status change (`session_id`
  at spawn, `last_spawn_error` at failure, `shutdown_request_id` at the request)
  have to land in the *same* `config.json` write as the status, but no caller
  may set the status itself. A caller updates the accompanying fields here and
  then calls `set_member_status`. Splitting the two writes is deliberate - the
  alternative, letting the caller pass the status through `update_member`, would
  be the second status writer FR-008 forbids.
- The coarse events are **not** emitted by the writer. `TeammateIdle`,
  `TeammateFailed`, `TeammateSuspended`, `TeammateResumed`, `TeammateSpawned`,
  and `TeammateStopped` stay the business of the call sites that own them until
  T-024 moves the spawn/stop pair here; emitting them from the writer before
  then would double-publish.

Covered by `crates/ragent-agent/tests/test_set_member_status.rs` (14 tests):
illegal transition refused naming member and both statuses; unknown agent ID is
an error; terminal status has no exit but a respawn; legal transition persists;
leaving `Working` / `PlanPending` / reaching `Stopped` clear their respective
ids; exactly one event with the full record; a refused transition publishes
nothing; re-asserting is not a transition and publishes nothing; every legal
edge in the transition table is accepted by the writer (the writer must not add
a rule the table does not have); `update_member` refuses an unknown ID and
persists its change without moving the status. Regression-proved: neutralising
the `can_transition_to` check makes `refused_transition_publishes_nothing` fail
(`Working -> Spawning is not legal: Some(StatusTransition { from: Working, to:
Spawning, cause: Spawned })`), and guarding the `publish` behind `if false`
makes `legal_transition_publishes_exactly_one_event_with_the_full_record` fail
with `left: 0, right: 1`; both restored.

**T-004 as built.** FR-008 and FR-026 are served by routing every
`TeamMember::status` write in `manager.rs` through `set_member_status`. After
T-004 that function holds the **only** `member.status = to` assignment in the
file; every other path calls it:

| Path | Move | Cause |
| --- | --- | --- |
| `spawn_teammate_internal` (insert) | already `Spawning`, no-op | - |
| `spawn_teammate_internal` (respawn) | `Failed`/`Stopped` -> `Spawning` | `Spawned` |
| `spawn_teammate_internal` (session up) | `Spawning` -> `Working` | `Spawned` |
| teammate agent loop (prompt done) | `Working` -> `Idle` | `ReportedIdle` |
| teammate agent loop (retries spent) | `Working`/`Spawning` -> `Failed` | `SpawnFailed` |
| `suspend_teammate` | -> `Suspended` | `Suspended` |
| `resume_teammate` | `Suspended` -> `Idle` | `Resumed` |
| `shutdown_teammate` (graceful) | -> `ShuttingDown` | `ShutdownRequested` |
| `shutdown_teammate` (immediate) | -> `ShuttingDown` -> `Stopped` | `ShutdownRequested`, `ShutdownAcked` |
| `start_watchdog` | live -> `Failed` | `WatchdogTimeout` |
| `approve_plan` | `PlanPending` -> `Working` | `PlanApproved` |

The spawn's insert is the one row that is not a move: `TeamMember::new` already
starts a fresh record in `Spawning`, and the writer call after the spawn lock is
released is what makes that status pass through the writer (`Ok(None)`, no
write, no event) while a respawn's move is validated and published.

Three consequences are deliberate behaviour changes, not accidents:

- **The immediate shutdown path makes two moves.** `Working -> Stopped` is not
  in the table (`ShuttingDown` is the only status with an edge to `Stopped`), so
  an immediate shutdown requests and then acknowledges, publishing
  `working -> shutting_down` and `shutting_down -> stopped`. The member is still
  `Stopped` when the call returns, which is what `shutdown_all` and the
  `team_shutdown_teammate` tool rely on.
- **A respawn validates its edge.** A respawn of a member that came to rest in a
  parked or terminal status now moves through `Failed`/`Stopped -> Spawning`.
  Re-spawning a member that is still live (`Working`, `Idle`, …) has no such
  edge and the spawn fails with the writer's refusal naming both statuses,
  instead of silently leaving the roster claiming a live session the spawn does
  not have. A fresh member is inserted in `Spawning`, so its writer call is
  `Ok(None)` and publishes nothing.
- **`approve_plan` reports a refusal.** Approving a member that cannot reach
  `Working` (a terminal one) returns the writer's error rather than recording an
  `Approved` verdict it did not apply. Approving one that already holds
  `Working` is a self transition: no event, verdict recorded. A rejection moves
  no status at all.

Two supporting changes make the routing possible:

- `spawn_teammate_internal` takes `self: &Arc<Self>` and the agent loop it
  starts holds a strong manager handle (cloned from the `self_handle` slot that
  `TeamManager::into_arc` / `new_in_arc` installs at construction). The loop
  cannot report a status through a manager that has been dropped, and the handle
  is obtained once, up front, rather than upgraded per event.
- `TeamManager::new_in_arc(..)` / `TeamManager::into_arc()` are the constructors
  that install that handle; the TUI's `App::init_team_manager` uses `into_arc`
  so it can still set `active_model` before wrapping. `TeamManager::new` remains
  for read-only and non-spawning use, and `self_handle()` returns a named error
  naming the constructor rather than spawning a teammate whose status could
  never be recorded.

Covered by `crates/ragent-agent/tests/test_member_status_write_sites.rs`
(9 tests): suspend, resume, both shutdown paths, and plan approval each record
their move through the writer (persisted status **and** exactly one
`TeammateStatusChanged` with the full record); a second graceful shutdown is not
a transition and publishes nothing; a rejection moves no status and records the
verdict; approving into `Working` from `Working` publishes nothing and still
records the verdict; approving a terminal member is refused and leaves no
verdict. Regression-proved by mutation: restoring the pre-T-004 direct write in
`suspend_teammate` makes `suspend_records_the_move_through_the_writer` fail
(`left: [], right: [("tm-001", "working", "suspended", "suspended")]`), and
restoring it in `approve_plan` fails `plan_approval_records_the_move_through_the_writer`
and `approving_a_plan_for_a_terminal_member_is_refused`; both restored.

The `manager_support::team_manager` fixture member starts `Working`, not
`Spawning`: nothing is spawning a freshly created team, and `Spawning` has no
edge to the parked or shutdown statuses, so a `Spawning` fixture would have made
the T-003 and T-004 suites assert fixture state rather than behaviour.

### M2 - Task transition contract (T-006..T-013)

The highest-value milestone: it closes F-01, F-02, F-03, F-04, F-05, and F-18.

- **T-006 as built** - `TaskStatus::can_transition_to` lives in
  `crates/ragent-agent/src/team/task.rs` beside `as_str`, with a supporting
  `TaskStatus::is_terminal` (`Completed`/`Cancelled`). The table is D3 verbatim:
  `Pending -> {InProgress, Cancelled}`, `InProgress -> {Completed, Pending,
  Cancelled}`, and no outgoing edge from either terminal status. Self-transitions
  are illegal by design (re-asserting a status is not a state change), so the
  idempotent-claim and idempotent-complete sites must test "already there" before
  validating. The function deliberately takes **no actor**: the ownership
  restrictions D3 attaches to specific pairs (completion is owner-or-lead,
  release is owner-or-lead, cancelling an `InProgress` task is lead-only) are
  actor checks the caller applies on top of the table, since the table answers
  only "is this state pair legal". Coverage is
  `crates/ragent-agent/tests/test_task_status_transitions.rs` - a transcribed
  16-row table with a completeness check, plus self-transition, terminal-sink,
  exit-exists, `is_terminal`, and the three named F-18 regression pairs.
- **T-007 as built** - `transition_task(list: &mut TaskList, task_id: &str,
  to: TaskStatus, actor: &str) -> Result<Option<TaskTransition>>` lives in
  `crates/ragent-agent/src/team/task.rs` next to the status table, re-exported
  from `ragent_agent::team` with its `TaskTransition { task_id, from, to,
  actor }` record. It mutates the caller's already-loaded list in memory and
  never touches disk, so FR-004's "`tasks.json` byte-identical" holds
  structurally: a refused transition returns before the caller can reach
  `TaskStore::write_locked`. Two deliberate departures from the task's literal
  signature: it returns `Option<TaskTransition>` rather than the mutated `Task`
  (the transition is what T-023's event publishing and T-011's mailbox
  notification need, and returning a clone of the task would be dead weight for
  callers that already hold it), and the idempotent re-assert (`from == to`)
  returns `Ok(None)` instead of an error - FR-003 states re-asserting a status
  is not a state change, so the claim and complete paths get "already there"
  for free and need no special case. An unknown `task_id` is an error naming the
  tasks that do exist. The FR-004 refusal reads `task 'X' cannot move from
  <from> to <to>; allowed from <from>: <row>` where `<row>` is the D3 row for
  the current status, so the operator sees the task, both statuses, and why the
  move is forbidden (`completed -> nothing: completed is terminal`). Coverage is
  `crates/ragent-agent/tests/test_transition_task.rs`: the 16-row D3 table
  driven through the helper, legal-move record contents, actor capture, the
  naming refusal for a table-forbidden pair and for both terminal rows,
  re-assert-is-`None` for all four statuses, unknown-id, and a source ratchet
  test that pins the five remaining direct `.status = TaskStatus::` writes in
  `task.rs` (T-008/T-009/T-011's to route) so a *new* one cannot appear
  unnoticed. Neutralising the `can_transition_to` guard makes four of the eight
  fail; restored.
- **T-008** is the behaviour change of FR-001. The current auto-claim branch
  (`manager.rs:1781`, `task.rs:586`) is deleted, not guarded: an unowned
  `Pending` task is **not** silently claimed by whoever completes it. The
  replacement message is "task 'X' is not assigned to you; the lead can assign
  it with `team_assign_task`, or you can claim it with `team_task_claim`".
- **T-009** keeps the "agent already has an in-progress task" guard as part of
  the single claim implementation, so `claim_next` and `claim_specific` cannot
  disagree about it.

  **As built (T-010):** FR-007 needed no new predicate - T-009 already made
  `claim_task_in_list` the only claim implementation and `Task::is_claimable`
  its only rule, so all four paths (`claim_next`, `claim_specific`,
  `pre_assign_task`, `claim_next_task`) inherit it. T-010 changed the
  *explanation* FR-006 requires. `claim_status_refusal` no longer answers a
  cancelled task with "only pending tasks can be claimed": it now reports
  `task 'X' cannot be claimed (status: cancelled); a cancelled task is terminal
  and cannot be claimed again`, while `InProgress` and `Completed` keep the
  generic sentence. "Only pending can be claimed" was a dead end for a cancelled
  task - `Cancelled` has no outgoing row in the D3 table, so retrying is never
  the answer - and the status itself is still named in both cases, which is the
  fact FR-006 asks a caller to see. The fifth surface, the `team_task_claim`
  tool's "nothing available" summary, now lists `cancelled` among the reasons
  and appends `<n> cancelled task(s) are terminal and cannot be claimed.` (with
  a `cancelled_tasks` metadata count) instead of reporting a cancelled-only list
  as "in progress, completed, or blocked by dependencies", so a cancelled task
  is no longer describable as waiting work on any surface. The
  `team_task_claim` description also states the rule the runtime enforces
  (pending only, the held status named on refusal, cancelled is terminal), per
  FR-042's "state the enforced rule" principle. Coverage:
  `tests/test_team_claim_single.rs` drives a table over all four `ClaimPath`
  variants against one cancelled task in one team directory - named claims
  refuse with `status: cancelled` + `terminal` and never call it pending,
  "next" claims find `None`, the task is neither re-assigned nor status-changed,
  and `tasks.json` is byte-identical across all four refusals (FR-004) - plus a
  per-status check that every non-pending status names itself and a check that
  the cancelled refusal differs from the completed one. The fixture gained
  `manager_support::team_manager_for(team_dir)` so a test can point the cached
  and uncached halves at one `tasks.json` instead of two fixtures that merely
  agree. One pre-existing assertion had to change: it required the cancelled
  refusal to mention "pending", which is exactly the wording FR-006 replaces; it
  now asserts "terminal" and the *absence* of "pending".
- **T-012 as built** - `crates/ragent-agent/src/tool/team_task_cancel.rs` is the
  lead's cancellation verb, and F-05 is closed: `TaskStatus::Cancelled` had no
  producer at all before T-011, and no *surface* before T-012, so a lead could
  only abandon work by editing `tasks.json` by hand.

  The tool is a thin surface, not an implementation. It resolves the team
  directory from `working_dir`, then delegates: `TeamManager::cancel_task` when
  the session has a manager, `TaskStore::cancel` when it does not. Both are
  façades over the single `cancel_task_in_list` (T-011), so the legal-transition
  check, the lead-only actor rule, the mailbox notice to an `InProgress` owner
  and the refusal wording are the same on every path (FR-025, FR-026). The
  manager path is preferred because it is the one that can send the FR-005
  notice from the lead session id it holds; the store fallback cancels but has
  no mailbox identity to sign one with, which it says in the reply.

  `TeamManagerInterface` gained `cancel_task`, because the tool holds an
  `Arc<dyn ...>` and the alternative - cancelling through a `TaskStore` of the
  tool's own - is exactly the second implementation FR-026 forbids. It follows
  the `set_member_status_in` precedent: the values cross the trait as plain
  data (`(from_status token, owner, notified)`) rather than the team config
  types, which that module must not depend on. The default arm bails, so a test
  double that does not model cancellation is refused rather than silently
  falling back to a different implementation.

  The permission category is `team:manage`, matching `team_assign_task` and
  `team_cleanup`: cancelling is a lead-only management verb, not a read
  (`team:read`) and not a claim (`team:tasks`).

  FR-042's description states what the runtime enforces: the lead-only rule and
  that a teammate's attempt is refused; that only `pending` and `in_progress`
  are cancellable and that `completed`/`cancelled` are terminal with the refusal
  naming the status the task actually holds; that an `in_progress` owner is
  notified through its mailbox; that a cancelled task can never be claimed or
  completed again; and that the task is *kept*, not deleted - a model that
  expected a delete would go looking for a missing task.

  Registering the tool moves the FR-030 surface from 149 to 150 names and the
  `team` family from 19 to 20 (the family count stays 21). Three pinned ratchets
  were recomputed, each of which documents that it is the place to record a
  legitimate addition: `test_fr030_surface_agreement` (`FR030_TOTAL`, the
  `agent`/`team` canary families, and the after-rename digest, which becomes
  `282b7298caba...` for 150 names), `test_tool_rename_atomic_landing`
  (`FR030_TOTAL` and the canary families), and the TUI's
  `test_fr030_surface_agreement` (`FR030_TOTAL`). The new values were measured
  from the registry rather than guessed, by a throwaway probe test that was
  deleted afterwards. `README.md` records 150 tools across 21 categories and
  names `team_task_cancel` in the Teams group (now 20).

  Coverage is `crates/ragent-agent/tests/test_team_task_cancel_tool.rs`
  (7 tests): a `pending` task is cancelled, kept in the list, and reports
  `pending -> cancelled` with nobody notified; an `in_progress` task is
  cancelled, retains its owner and delivers the `TaskCancelled` mailbox notice
  carrying the lead's reason; a **teammate** calling the tool is refused
  (`only the lead`) and mutates nothing; a cancelled task is then refused by the
  specific-claim path with `status: cancelled` and no `status: pending`, skipped
  by the next-task scan, and refused by completion; a terminal task cannot be
  cancelled and the refusal names the status it holds; an unknown id is refused;
  and the description/schema/permission assertions of FR-042 (every rule above,
  `team_name`+`task_id` required with `reason` optional, `additionalProperties:
  false`, and category `team:manage`).
- **T-011** adds the missing `Cancelled` producer. Cancellation of an
  `InProgress` task notifies the owner through the existing `MessageType::Message`
  channel with a new `MessageType::TaskCancelled` variant.

  **As built:** `cancel_task_in_list(list, task_id, actor) -> Result<TaskCancellation>`
  in `task.rs` is the single implementation; `TaskStore::cancel` (plus a
  `cancel_blocking` async wrapper) and `TeamManager::cancel_task` both delegate to
  it, exactly as the claim and completion paths share theirs (FR-025, FR-026), so
  the two entry points emit the same refusal text - pinned by
  `test_team_cancel::manager_and_store_share_one_refusal_text`. The status write
  goes through `transition_task` (FR-003), so the structural ratchet in
  `test_transition_task` stays at zero direct writers; the pre-check on
  `TaskStatus::is_terminal` exists only to produce the cancellation-specific
  `status: completed` / `status: cancelled` message FR-006 requires, not to bypass
  the table. `TaskCancellation` carries `{task, owner, from, transition, notified}`
  and `notify_target()` returns an owner only when the task was `InProgress` - a
  `Pending` task has nobody working on it, so no message is written. The
  notification is sent *after* the write (a mailbox failure cannot roll back a
  committed cancellation; it is returned as an error naming the task and the
  owner), and carries the lead session id the mailbox write boundary validates
  (SEC-ragent-team-003). `TeamManager::cancel_task` additionally enforces the
  FR-005 actor rule - only `lead` may cancel - before the task list is loaded.
  Cancellation leaves `assigned_to` and `claimed_at` intact so the lead can still
  see who was working on the abandoned task. Coverage:
  `tests/test_team_cancel.rs` (12 tests).

Verify: a table-driven check over all seven `(from, to)` pairs of the D3 table.

### M3 - Watchdog and parked states (T-014..T-017)

Closes F-06, F-07, F-08, F-09. T-016 and T-017 are the pair that makes
`Blocked` recoverable: T-016 gives the operator a verb, T-017 removes the need
for it in the common case.

Verify: suspend a teammate, wait past the timeout, confirm the status is still
`suspended` and no `TeammateFailed` event fired.

- **T-014 as built** - the watchdog's on-disk eligibility check in
  `TeamManager::start_watchdog` (`manager.rs`) is now
  `member.status.is_live()` instead of a hand-spelled
  `Working | Spawning | PlanPending | Suspended` list, so the failable set is
  exactly `Spawning`, `Working`, `ShuttingDown` (FR-012, FR-009). That single
  change is both halves of the reported defect: `Suspended` leaves the eligible
  set (F-07) and `Blocked` and `ShuttingDown` stop taking the implicit
  `unwrap_or(false)` path (F-08). A team whose config cannot be read still
  yields `false` - the watchdog refuses to fail a member it cannot confirm is
  live. Coverage is
  `test_team_manager_extra::watchdog_never_fails_a_parked_member`: four spawned
  members are parked on disk as `Suspended`, `Blocked` and `PlanPending` with
  their handles and `last_progress` untouched (so all are stale), the watchdog
  is started, and the assertion holds only once a stale `Working` control member
  has been failed - proving the watchdog ran - while all three parked members
  keep their status, carry no "no progress" error, and publish no
  `TeammateFailed`. Restoring the old status list fails that test with
  `left: Failed, right: Suspended`.

- **T-015 as built** - the watchdog's eligibility decision in
  `TeamManager::start_watchdog` (`crates/ragent-agent/src/team/manager.rs`) is now
  a single `match` over every `MemberStatus` variant, so no status reaches the
  failure verdict by fall-through and the `is_some_and(|s| s.is_live())` +
  `unwrap_or(false)` shape is gone (FR-012, F-08). The three arms are the D2
  classes: `Spawning | Working | ShuttingDown` fall through to the failure
  verdict; `Idle | PlanPending | Blocked | Suspended` (parked) and
  `Stopped | Failed` (terminal) refresh the handle's progress marker and
  `continue`; `None` - a member absent from a config that *was* read, or a tick
  whose config could not be read - logs at `debug!` and takes no verdict, so the
  watchdog still never fails a member it cannot confirm is live. Two supporting
  changes make the explicit branches work without putting the watchdog on the
  disk: a new `config_cache` (mtime-keyed, like the existing `task_cache`) means
  one `config.json` read serves a whole tick instead of one read per stale
  member, and `cached_config` is invalidated at the top of `set_member_status`
  and `update_member` so a write landing inside the filesystem's `mtime`
  granularity cannot leave the watchdog reading the pre-write status. A tick
  whose config read fails now returns before touching any handle, which changes
  the old behaviour of refreshing progress on an unreadable team. The failure
  verdict also goes through the single status writer
  (`set_member_status(_, Failed, WatchdogTimeout)`) instead of assigning
  `m.status` directly, so it publishes exactly one `TeammateStatusChanged`
  alongside the coarse `TeammateFailed` and drops the caches the move
  invalidates; `last_spawn_error` still lands in its own `update_member` write.
  Coverage is `crates/ragent-agent/tests/test_watchdog_status_branches.rs`: all
  nine statuses stale in one manager with both live controls (`Working`,
  `ShuttingDown`) required to be failed before the parked and terminal members
  are asserted unchanged and event-free; a `Blocked` member surviving dozens of
  ticks beside a failed live member; an unreadable team directory producing no
  verdict; and one `TeammateStatusChanged working -> failed` plus one
  `TeammateFailed` for a watchdog failure. Restoring the pre-T-015 shape (a
  hand-spelled `Working | Spawning | PlanPending | Suspended` list with an
  implicit `unwrap_or(false)` default) fails three of the four; restored.

- **T-016 as built** - `TeamManager::resume_teammate`
  (`crates/ragent-agent/src/team/manager.rs`) is now the operator's single
  resume verb for both parked statuses that need one (FR-010, F-06, F-25).
  The pre-fix guard was `is_suspended`, a boolean test on exactly one status,
  so a `Blocked` member had no in-product escape: nothing in `ragent-agent`
  unblocked it and `team_cleanup` refuses a live member (F-22), which wedged
  the team for good. The guard is now
  `matches!(current, Suspended | Blocked)` over the status read from the store,
  and the rest of the function is unchanged: mailbox polling restarts and the
  move to `Idle` goes through the single status writer
  (`set_member_status(_, Idle, Resumed)`), so the exit is validated against the
  transition table (`Suspended -> Idle` and `Blocked -> Idle` are both legal
  edges), persisted, and published as one
  `TeammateStatusChanged { from, to: "idle", cause: "resumed" }` plus the
  coarse `TeammateResumed`. **"Clearing the block reason" is the exit itself.**
  `TeamMember` has no block-reason field - `Blocked` is a bare status - so the
  move to `Idle` is the whole clearing, and the writer's existing side effect on
  the way into `Idle` also drops `current_task_id`, releasing the task the
  member was parked on. One further behaviour change: an unknown `agent_id` is
  now an error naming the team and the id instead of the misleading
  `"is not suspended"`, and a non-parked member is refused with a message that
  names its actual status and the two statuses resume accepts, so the TUI log
  line a failed `Space` produces is actionable. `resume_teammate` deliberately
  does **not** re-block a member whose dependencies are still unmet - FR-010 is
  the operator override, and re-parking belongs to the dependency rule
  (FR-011, T-017). The TUI's `Space` toggle
  (`crates/ragent-tui/src/app/windows.rs::teams_cursor_toggle_stop_resume`) was
  widened the same way, so a `Blocked` row offers resume; the `Del` shutdown
  path and its confirmation are T-035's. Coverage is
  `crates/ragent-agent/tests/test_resume_teammate_blocked.rs` (7 tests): a
  blocked member resumed to `Idle` with one `blocked -> idle`/`resumed`
  transition, exactly one `TeammateResumed`, and its held task released; the
  suspended path unchanged; both parked statuses reaching the same destination;
  a `Working` member refused, unmoved and event-free; a `Stopped` member
  refused; an unknown id refused by name; and resume not re-blocking.
  `test_team_manager_runtime::resume_non_suspended_member_errors` was updated
  for the new refusal wording. Mutation proof: restoring the pre-fix
  `is_suspended` guard fails
  `resume_of_a_blocked_member_returns_it_to_idle_through_the_writer` and
  `blocked_and_suspended_resume_to_the_same_idle_destination`; restored
  byte-identical.

### M4 - Configuration honouring (T-018..T-022)

Closes F-10, F-11, F-12. Each task makes one declared setting do what it says.

- **T-018** makes `require_plan_approval` gate the call rather than decorate the
  prompt: it adds a second, wider predicate (`is_plan_gate_active`) beside the
  existing `is_plan_pending` gate that already blocks write tools while
  `PlanPending`, so a teammate that never submitted a plan is stopped too, and
  the refusal text names `team_submit_plan`.
- **T-019** is the only task that decides what `auto_claim_tasks: false` means.
  The chosen reading: the teammate stays `Idle` and the lead assigns work. The
  competing reading - "the runtime auto-claims instead of the teammate" - is
  rejected because the runtime already does not auto-claim today, so that
  reading would make the default a no-op.

#### T-018 as built

FR-014 is the requirement that `require_plan_approval` **does** something. Before
this task the setting was read at exactly one place - the teammate system prompt
(`manager.rs:188`) - so a teammate that never called `team_submit_plan` was never
stopped: F-10.

The task as planned said "reuse the `is_plan_pending` gate". That alone cannot
satisfy the requirement, and the reason is worth recording. The existing gate
(stage 6 of `session/dispatch.rs`, T-307) asks `is_plan_pending`, which is true
only for `PlanStatus::Pending` - a plan *submitted and awaiting the verdict*. A
teammate that has submitted nothing has `PlanStatus::None`, and the FR's own
words are "a teammate whose `plan_status` is not `Approved`", which covers `None`
and `Rejected` as well. Widening `is_plan_pending` in place would have changed a
pre-existing predicate's meaning for a pre-existing caller, so the gate got a
second question instead:

| | asks | true when |
| --- | --- | --- |
| `is_plan_pending` (unchanged) | has a plan awaiting the verdict? | `plan_status == Pending` |
| `is_plan_gate_active` (new) | may this teammate write? | `settings.require_plan_approval && plan_status != Approved` |

Stage 6 ORs the two. With the setting off the new predicate is inert, so the
only restriction is the old `PlanPending` one - which is why the two are ORed
rather than replaced: the narrow behaviour survives a setting disabled mid-run.

Four decisions:

- **Both predicates share one PERF-027 cache entry.** `cached_plan_query` takes
  the `plan_status` predicate as a parameter and stores the answer under the same
  per-agent key. A second entry keyed on the same agent would let the two queries
  disagree for up to the 2-second TTL, and `approve_plan` /
  `team_submit_plan` invalidate exactly one key. The cost is recorded in the
  test: reading `is_plan_pending` after `is_plan_gate_active` on the same manager
  returns the cached *gate* answer, so a test that needs both meanings reads them
  from a fresh manager.
- **Absent data fails open, not closed.** A member id not on the roster, or an
  unreadable team directory, yields `false`. A gate that fires on data it could
  not read would refuse the wrong agent, and the setting is opt-in: an
  unknowable `require_plan_approval` must not stop every teammate.
- **The bash class is the tool, not the command.** `LOOP_BASH_TOOLS` is a new
  public class constant (`Bash`, `run_code`) beside `LOOP_WRITE_TOOLS`, so the
  gate refuses a shell call whatever the command string says. Classifying a
  command as a write is `bash_sub_command_is_write`'s best-effort heuristic, and a
  gate built on a heuristic is a gate with holes.
- **The refusal names the class, the state and the remedy.** `plan_gate_denial`
  now says "write-class or bash-class tool", "your plan has not been approved
  yet", and "call `team_submit_plan`" - entity, current state, next action
  (FR-032's shape). The spawn prompt gained one sentence saying that write- and
  bash-class calls *are refused* until approval, because a prompt that only says
  "submit a plan" invites the model to retry a refused tool forever.

Coverage: `crates/ragent-agent/tests/test_plan_approval_gate.rs` (16 tests) - the
setting axis (`off` is inert; `None`/`Pending`/`Rejected` gate; `Approved` clears,
through a warm cache; unknown member and unreadable team are inert), the tool axis
(every `LOOP_WRITE_TOOLS` member and every `LOOP_BASH_TOOLS` member refused;
read-only and `team_*` tools pass; the lead never gated; a closed gate refuses
nothing), the refusal text, and two source assertions that stage 6 asks both
predicates and that the prompt states the consequence.

Mutation proof: changing
`is_plan_gate_active`'s predicate back to `plan_status == PlanStatus::Pending`
fails `gate_fires_for_a_teammate_that_never_submitted_a_plan` and
`gate_fires_after_a_rejected_plan` (2 of 16); restored byte-identical.

#### T-022 as built

`TeamSettings.watchdog_timeout_secs` (u64, `Default` =
`DEFAULT_WATCHDOG_TIMEOUT_SECS` = 300) is the operator's remedy for F-09: a
teammate legitimately running one long build can be given a wider window instead
of being failed with no recourse. Three decisions are worth recording.

- **`0` means the default, not a zero window.** The field carries a `serde`
  default of 300, and `effective_watchdog_timeout_secs()` substitutes
  `DEFAULT_WATCHDOG_TIMEOUT_SECS` for a configured `0`. Without that, a config
  with the field zeroed would arm a zero-length window and fail every live
  teammate on its first tick - the opposite of what an operator zeroing a
  timeout would intend. The same accessor is what `TeamManager::new` reads, so a
  `config.json` written before this field existed keeps the documented window.
- **The tick interval is derived, then capped.** `watchdog_check_interval(timeout)`
  is `min(timeout, 1 min) / 2`: half the window keeps detection within one tick
  of the deadline, and the one-minute cap stops a wide operator window (an hour,
  say) from also widening the latency - such a team is still checked every
  thirty seconds. Both the window and the interval come from the single
  `watchdog_timeout` field, so there is one place to look.
- **`pub` rather than `pub(crate)`.** `TeamManager::watchdog_timeout` stays a
  public field so a caller (and the tests, which cannot wait minutes) can tighten
  it after construction; a change there changes the tick interval too, because
  the interval is derived from it at `start_watchdog`.

FR-013's second clause is honoured by the `/team help` table gaining a "Team
settings" section that names every `settings` field with its default and effect,
so `watchdog_timeout_secs` is discoverable where an operator looks. `tests/test_team_watchdog_timeout.rs` covers the declared default, the save/load
round trip and the `0` fallback, the missing-field `serde` default, the
constructor seeding the field from the team's settings, and the interval
derivation table; restoring a hardcoded `Duration::from_mins(5)`, a constant
30-second tick, or a bare `self.watchdog_timeout_secs` fails one test each.

The verdict semantics those numbers are measured against (which member is
eligible for a failure, what gets published) remain covered by
`tests/test_watchdog_status_branches.rs` from T-015.

### M5 - Hooks on the runtime path (T-023..T-026)

Closes F-13, F-14, F-15. After M5 the hook surface has one trigger point (the
transition functions) and one failure surface (the ledger).

**T-023 as built.** FR-017 is served by moving the trigger from the three
`team_task_*` tools into the mutation that the tools and every other caller
share, so the tools no longer call `run_team_hook` at all:

| Mutation | Hook | Entry points that reach it |
| --- | --- | --- |
| a task added to the list | `TaskCreated` | `team_task_create`, the `team_create` seed path, any test fixture |
| `Pending -> InProgress` | `TaskClaimed` | `claim_next`, `claim_specific`, `pre_assign_task`, `TeamManager::claim_next_task`, `team_assign_task` |
| `InProgress -> Completed` | `TaskCompleted` | `TaskStore::complete`, `TeamManager::complete_task`, a swarm auto-complete |

- `fire_task_hook(team_dir, transition: Option<&TaskTransition>, task, actor)`
  in `crates/ragent-agent/src/team/task.rs` is the one trigger point. It
  **derives** the event from the transition rather than taking it as a
  parameter, so a caller cannot forget or mis-name it: `to == InProgress` is
  `TaskClaimed`, `to == Completed` is `TaskCompleted`, and a `None` transition
  (a creation, which is a push rather than a status move) is `TaskCreated`.
  `Cancelled` and a release to `Pending` fire nothing - this schema has no hook
  event for either. `fire_task_created_hook` is the creation-typed shorthand.
- The hook runs **after** the write, never inside it. A hook is an external
  process (up to the 30 s timeout), and holding the team's `tasks.json` `flock`
  across it would serialise every task mutation in the team behind one script.
  The consequence is stated in each entry point's docs: a hook-lookup failure is
  reported *after* a committed mutation, so the caller learns the gate did not
  pass rather than that the work was refused.
- `HookEvent` gains `TaskClaimed`. FR-017 names it explicitly, and no configured
  hook could match the claim event before.
- Three signatures changed to carry the verdict out of the runtime:
  `TaskStore::add_task` and `TaskStore::claim_next`/`claim_specific` are now
  `async` (they run the hook), `TaskStore::complete` and
  `TeamManager::complete_task` return `(Task, HookOutcome)`, and
  `TeamManager::claim_next_task` is `async`. The tools consume the outcome
  instead of producing it: `team_task_complete` still reverts a rejected
  completion to `InProgress` and reports the feedback, but reads the verdict
  from `TaskStore::complete` rather than running the hook itself.
- The coarse `TeamTaskClaimed` / `TeamTaskCompleted` events moved with the hook.
  `TeamManager::publish_task_event` publishes them from `claim_next_task` and
  `complete_task`, where the lead session id is already in memory (PERF-017,
  PERF-018), so a lead-side completion reports the same event the tool's does.
  The per-agent `current_task_id` bookkeeping stays in the tools: it is roster
  state about the caller, not part of the task-list mutation.
- `team_assign_task` no longer writes `Task::status` itself. It calls the new
  `TaskStore::assign`, which moves the status through `transition_task` (FR-003)
  and then fires the claim hook - the F-13 headline case, a task assigned by the
  lead rather than claimed by the teammate. A task that is not `Pending` is
  returned unchanged and fires nothing, which is the previous behaviour made
  explicit.
- The four `*_blocking` wrappers around `add_task` / `claim_next` /
  `claim_specific` / `complete` are gone: those methods are `async` on the store
  themselves now, so the wrapper would be a second async hop around an async
  call. `read_blocking`, `update_task_blocking` and `cancel_blocking` remain,
  and `pre_assign_task_blocking` joins them.

Coverage is `crates/ragent-agent/tests/test_task_hook_runtime.rs` (12 tests).
Each test installs a hook that appends its own name to a marker log and then
drives one entry point, asserting on the log rather than on a tool's output -
which is what makes it a test of the runtime: a tool that still fired the hook
itself would append the name twice. Covered: `add_task` fires `TaskCreated` from
the store; a duplicate add is refused, fires nothing and leaves `tasks.json`
byte-identical; `claim_specific` fires `TaskClaimed`; the lead-side
`team_assign_task` path (`TaskStore::assign`) fires it too; an idempotent
re-claim and a refused claim fire nothing; `complete` fires `TaskCompleted` and
hands back `HookOutcome::Allow`; a refused completion fires nothing; the
completion **tool** fires the hook exactly once (from the store, not twice); the
`TaskTransition -> HookEvent` mapping including the no-event `Cancelled` case;
and a `TeamManager` completion fires it as well.

Two supporting changes fell out of the "the hook needs a team" requirement: the
task-store fixtures in `test_team_task_store`, `test_team_claim_single`,
`test_team_cancel` and `test_concurrent_store_writes` now open a **real** team
directory (`team_support::setup_team_with_task_store`) rather than a bare
`.ragent/teams` path, because a hook lookup needs a readable `config.json`; and
the TUI's swarm paths (`execute_swarm_decomposition`, `poll_pending_swarm`,
`poll_swarm_completion`) became `async` because they seed and complete tasks
through the now-async store. `cargo nextest run -p ragent-agent`: 2362 tests,
2362 pass. Note that the `ragent-tui` integration binary does not compile for
reasons unrelated to this task: `test_teams_tui.rs` asserts on
`App::team_delete_confirmed`, which T-028/T-043 have not yet added to
`app/slash.rs`; the `ragent-tui` **lib** target's 142 tests pass.

**T-025 as built.** FR-018 needed one new route and one behaviour change, not a
new completion implementation:

- `TeamManagerInterface::complete_task_in(team_dir, task_id, agent_id) ->
  Result<(task_id, title, completed_at, feedback: Option<String>)>`
  (`crates/ragent-agent/src/tool/mod.rs`) is the tool-facing arm. Its default
  implementation bails ("the runtime has no completion path"), following
  `set_member_status_in` / `cancel_task`; `TeamManager` overrides it by
  delegating to the inherent `complete_task` - the one path that already runs
  the FR-001 ownership rule, the D3 transition, the `TeamTaskCompleted` event
  and the `TaskCompleted` hook. The manager refuses a `team_dir` that is not its
  own rather than applying its roster, bus and cache to a foreign list, which is
  the observable proof that the tool cannot complete behind the runtime's back
  (FR-026).
- `complete_task_through_manager(ctx, team_dir, task_id, agent_id)`
  (`tool/mod.rs`) is the one call site: it uses the attached manager when the
  session has one and `TaskStore::complete` otherwise - the same in-list
  implementation, so the verdict reaches the agent on both paths.
  `team_task_complete` now calls it instead of `TaskStore::complete` directly,
  and reports `hook_rejected` + `feedback` in its result.
- **Behaviour change: a veto no longer reverts the task.** The pre-T-025 tool
  wrote `status = InProgress` behind a vetoed completion. That is a second
  writer of the task status (FR-008, FR-026), it left `TeamTaskCompleted`
  published for a task that read `in_progress`, and it asked the agent to "try
  again" across a terminal status: `Completed -> InProgress` is not in the D3
  table, so the retry could never succeed. The verdict is now *delivered*, not
  acted on - the tool result says the completion stands and points at the lead
  for a follow-up task or a cancel-and-reissue. The pre-commit gate is still the
  `TaskClaimed` hook, which runs before the transition is written.
- The description gains a HOOK VERDICT paragraph so an agent reads the veto as a
  report rather than a retryable failure (FR-042's wording task, T-040, had left
  the verdict undocumented).

Coverage is `crates/ragent-agent/tests/test_hook_feedback_delivery.rs` (9
tests): the runtime path delivers the hook's stdout into the tool result and
into `metadata.feedback`; the vetoed task stays `Completed`; an allowing hook
produces no veto; the store fallback delivers the same verdict; a lead-side
completion gets it too; a foreign manager is refused and leaves that team's
`tasks.json` at `InProgress`; the description states the contract; the trait's
default arm refuses; and the completing member's `current_task_id` is still
cleared. `cargo nextest run -p ragent-agent`: 2404 tests, 2402 pass (the two
pre-existing failures
`test_m3_lifecycle::test_team_idle_publishes_teammate_idle_event` and
`test_task_tools_cache_more::adopt_orphaned_tasks_leaves_tasks_owned_by_others_untouched`
are unchanged by this task).

**T-026 as built.** FR-019/NFR-004 close F-14 with one new module and one
signature change; no verdict changed.

- `crates/ragent-agent/src/team/hook_ledger.rs` owns the ledger:
  `HookFailure { timestamp, event, hook, reason, detail }` is one line of
  `hook-failures.jsonl`, `HookFailureReason` is `invalid` / `spawn_failed` /
  `timeout`, and `try_record_hook_failure` is the append. `record_hook_failure`
  wraps it and swallows an I/O error with a `warn!`: the ledger is observability,
  not a gate, so it must never invent a verdict. `read_hook_failures`,
  `hook_failure_count` and `render_hook_failures` are the read surface.
- **NFR-004's bound is a whole-file rotation, not a truncation.** The size is
  checked *before* the append: at or over `MAX_LEDGER_BYTES` (1 MiB) the live
  file is renamed to `hook-failures.jsonl.1` (replacing any previous generation)
  and the new record starts a fresh file. Nothing already written is ever
  rewritten outside rotation, and a reader sees a bounded live file plus at most
  one previous generation. A missing file reads as an empty ledger, and a
  truncated final line (a crash mid-append) is skipped rather than hiding the
  records before it.
- `run_hook` gained two parameters, `team_dir: &Path` and `event: HookEvent`,
  because a failure record is meaningless without the team it belongs to and the
  trigger it was registered for. Its three "the hook never ran" arms now record
  before returning `HookOutcome::Allow`. `run_team_hook` already knew both and
  passes them through, and `fire_task_hook` already passed the event, so no task
  path changed.
- The surfaces are `team_status` (a `Hook failures (N)` block plus
  `hook_failure_count` / `hook_failures` metadata) and `/team show <name>` and
  `/team status` in the TUI. Both call `render_hook_failures`, which returns an
  empty string when nothing failed, so a healthy team adds no noise.

Coverage is `crates/ragent-agent/tests/test_hook_failure_ledger.rs` (17 tests):
the refused command, the spawn failure and the timeout each record exactly one
line and keep `HookOutcome::Allow`; a hook that runs records nothing and creates
no file; a task completion and `run_team_hook` reach the ledger on the runtime
path; each line parses as a standalone JSON object; the 1 MiB rotation keeps one
whole previous generation and the second rotation replaces it; an append leaves
the existing bytes untouched; a truncated final line does not hide the rest; the
render names the count, each failure, both filenames and the team; and
`team_status` reports the count in both its prose and its metadata. Suppressing
the record in `run_hook` fails 5 of them, so the ledger is regression-proof
rather than merely present. `cargo nextest run -p ragent-agent`: 2421 tests,
2419 pass - the two pre-existing failures
`test_m3_lifecycle::test_team_idle_publishes_teammate_idle_event` and
`test_task_tools_cache_more::adopt_orphaned_tasks_leaves_tasks_owned_by_others_untouched`
are unchanged by this task. The `ragent-tui` integration binary still fails to
compile on the pre-existing `App::team_delete_confirmed` gap (T-028/T-043); the
`ragent-tui` lib target builds clean.

### M6 - Lifecycle guards (T-027..T-029)

Closes F-22, F-23, F-20, F-21. T-027 collapses three guards into one predicate
expression; the shared refusal text is a single function so the three surfaces
cannot drift again.

**T-027 as built.** The rule lives in `crates/ragent-agent/src/team/mod.rs`
beside `plan_gate_denial`, because the TUI and the tool both depend on
`ragent-agent` and neither can own a rule the other must share:

- `blocks_teardown(status)` - the one predicate: `status.is_live()` or
  `ShuttingDown` (the sets coincide today; both terms are spelled out so the
  status that *sounds* terminal is visibly covered).
- `blocking_members(members)` - the blocking members as `name (status)` strings,
  using the canonical `MemberStatus::as_str` token.
- `cleanup_refusal(team_name, members)` - the refusal text, or `None`.

`/team cleanup`, `/team delete`, and `team_cleanup` all call
`cleanup_refusal`, and the two TUI surfaces derive their status-line count from
`blocks_teardown` so the number and the text cannot disagree. `force: true`
remains a tool-only override. Covered by
`crates/ragent-agent/tests/test_team_cleanup_guard.rs` (14 tests); restoring
either the tool or a TUI surface to its pre-fix rule fails the behavioural
tests, so the guard is regression-proof rather than merely present.

**T-028 as built.** T-027 gave `/team delete` the shared teardown refusal, but a
single well-formed command still removed a team directory for good, which is
F-28. The destructive direction is now a second action, and the rule lives
beside the other team policies in `crates/ragent-agent/src/team/mod.rs` so the
text is shared rather than reworded per surface:

- `delete_confirmation(team_name, members, exclude)` - the confirmation text, or
  `None`. It fires exactly for the rosters `cleanup_refusal` refuses, so the two
  guards partition the roster and the `force` override can never be silently
  honoured. An `exclude` list is checked first and wins: a name the operator
  withholds cannot be deleted at all, so it is never confirmed either.
- `/team delete <name> [force]` parses the override (`--force`, `force`,
  `force: true`, `force=true` all spell it), then: without `force` the FR-022
  refusal fires; with `force` and an unarmed confirmation the confirmation text
  is printed, `team_delete_confirmed` is armed with the team name, and nothing
  is deleted; with `force` and that name already armed the delete proceeds and
  the success message states how many live members were forced. Any other
  `delete` argument clears the arming, so the override never carries over to a
  different team.
- A named team that is **not** the session's active team is now checked against
  its **persisted** roster rather than the empty in-memory one, which is what
  made F-28's "no confirmation at all" branch possible.

Covered by 3 tests in `crates/ragent-agent/tests/test_team_cleanup_guard.rs`
(the helper: quiet roster, every refused status, exclusion) and 7 in
`crates/ragent-tui/tests/test_teams_tui.rs` (the surface: persisted-roster
refusal, the two-step force, parked rosters needing no ceremony, and the active
path). Neutralising the `!confirmed` gate makes both two-step tests fail at the
first `--force`, so the confirmation is regression-proof rather than merely
present.

**T-043 as built.** T-028 confirmed the *override*, so a team with nothing live
was still removed by one command. FR-045 makes the confirmation unconditional:
`delete_confirmation` now returns `Some` for **every** roster that is not
excluded, so the two guards no longer partition it - `cleanup_refusal` still
decides whether the delete may proceed at all, and `delete_confirmation` decides
whether this command is the confirmed second action.

- The empty-roster branch names what is actually lost - the directory, its
  tasks, mailbox and roster - rather than a teammate count, because there is no
  live member to name.
- `/team delete <name> [args]` keeps the FR-022 refusal for a live roster
  without the override, then requires `!confirmed` for *every* remaining case,
  active team or not. `team_delete_confirmed` still holds the exact team name,
  so arming one team never authorises another; a `delete` for a different name
  disarms the first and confirms the second.
- `cancel` / `--cancel` is the escape: it disarms the arming, deletes nothing,
  and reports the cancellation, so a confirmation taken out by mistake cannot
  strand the operator with a team they cannot delete. The arming is also cleared
  before any filesystem work, so the confirmation cannot survive a name that is
  not the armed one.
- The confirmation's next-action line is short (`Confirm with: /team delete
  <name> --force`) because the TUI renders slash output through `html2text` at a
  fixed 120-column width, which breaks a longer line at a space and would hand
  the operator a command split in two.
- `/team help` states the two-step shape and the `cancel` escape in the
  `/team delete` row.

Coverage: 3 helper tests in `crates/ragent-agent/tests/test_team_cleanup_guard.rs`
(a quiet roster and an empty roster are confirmed, every status is confirmed
with its blocking member named, exclusion wins even for a quiet roster) and the
surface tests in `crates/ragent-tui/tests/test_teams_tui.rs` (a parked named team
needs the second action, `cancel` disarms the confirmation and the next
`--force` starts a fresh cycle, an arming does not carry to another team, plus
the two pre-existing delete tests updated to the two-step shape and the
`test_slash_arms_deep` delete arms updated to confirm first). Neutralising the
`!confirmed` gate makes `test_team_delete_of_a_parked_roster_still_needs_confirmation`
and `test_team_delete_force_requires_a_second_action` fail with `team deleted`
on the first command, so the requirement is regression-proof rather than merely
present.

### M7 - Mailbox and robustness (T-030..T-033)

Closes F-16, F-37, F-38, F-39. T-032 is a safety fix: today a malformed
`tasks.json` silently becomes an empty list, and the next write persists the
empty list.

**T-030 as built.** FR-020 needed no change to the mailbox: a message for a
teammate that terminates before reading it is already left on disk with
`read: false` (nothing drains a mailbox whose owner is gone), so the whole task
is *reporting* it. Three pieces:

- `team::undelivered_messages(&TeamStore) -> Vec<UndeliveredMessage>` asks the
  two questions that make a message undelivered - has the recipient terminated
  (`MemberStatus::is_terminal()`, i.e. `Stopped` or `Failed`), and is the message
  still unread (`Mailbox::peek_unread`, which does not mark it read). A member
  that is alive or parked is skipped entirely: its mailbox will still be drained
  when it wakes, so reporting its backlog would be noise on every status call.
  An unreadable mailbox is skipped rather than failing the report - the mailbox
  layer already diagnoses corruption by moving the file aside.
- `team::render_undelivered(team_name, &[..], heading_level)` is the one
  renderer both surfaces call, so they cannot disagree about what was stranded.
  It names the count, and each row's recipient, recipient status, sender, type,
  timestamp, id and body; it states that the message was **kept, not deleted**,
  and names the two actions that exist (read it as that recipient, or start the
  recipient again). An empty list renders an empty string, so no surface grows an
  empty section. `heading_level` lets `/team show` nest the block at h2 and
  `team_status` print it at h1.
- `team_status` appends the block to its text report and mirrors it in metadata
  as `undelivered_count` + `undelivered[]` (`agent_id`, `recipient`,
  `recipient_status`, `message_id`, `from`, `type`, `sent_at`, `content`), so an
  agent can assert on the shape without parsing prose. Its `description()`
  previously told the model the tool "does not read mailbox messages"; it now
  states that it reports stranded ones and still points at `team_read_messages`
  for the caller's own inbox. `/team show <name>` and `/team status` both render
  the same block in the TUI.

Coverage is `crates/ragent-agent/tests/test_undelivered_messages.rs` (12 tests):
the message survives unread on disk; a stopped and a failed recipient both
report; an already-read message is not reported; none of the seven live/parked
statuses report; the rendered block names recipient, status, sender, type and
content and says the message was kept; an empty report renders nothing; the
heading depth follows the caller; several stranded messages for several
recipients all appear; `team_status` renders the block and the metadata; a
healthy team has no section and a zero count; and reporting is idempotent and
does not mark the message read. Two mutations fail the suite: forcing the
`is_terminal()` filter to `false` fails
`a_live_or_parked_recipient_is_never_reported`, and emptying the collected list
inside `team_status` fails `team_status_reports_undelivered_messages` plus
`reporting_is_idempotent_and_does_not_mark_the_message_read`. The source was
restored byte-identically after each.

**T-031 as built.** F-16's size half was a false finding: `Mailbox::push`
already refuses a body over `MAX_MESSAGE_BYTES` at the write boundary
(`mailbox.rs:402`) with an error naming the message id, the actual size, and the
limit, so no production change was needed. The task therefore pins what is
there:

- `crates/ragent-agent/tests/test_team_message_size_limit.rs` (3 tests) - the
  boundary (`>`, not `>=`, so exactly `MAX_MESSAGE_BYTES` is accepted and
  stored complete), that the refusal text contains the actual size, the limit,
  and the offending `message_id`, and that a refused message writes nothing (the
  failure is a refusal, not a silent truncation leaving a short message behind).
  Weakening the error string to `"mailbox message over the byte limit"` fails
  the naming test, so the guard is regression-proof.
- `team_message`'s `description()` and `content` parameter now state the
  262144-byte (256 KiB) cap and that an oversized body is refused rather than
  truncated, so a sender learns the limit from the tool surface rather than
  from a failure.

**T-033 as built.** FR-038 and FR-039 close F-38 and F-39 with one predicate and
one call site.

- `TaskList::validate()` was per-task only: it looped `Task::validate()` and
  stopped there, so a list could be internally inconsistent while every row was
  individually well-formed. It now checks the list *as a list* - no duplicate
  task ids (a duplicate makes `get`/`get_mut` ambiguous, so a claim could move
  one row while a completion moved the other), then
  `validate_dependencies()`: every `depends_on` entry must name a task in this
  list, and the dependency graph must be acyclic. Dangling edges are reported
  first with the missing id, the dependent, and the ids that do exist; cycles
  are found by a three-colour iterative DFS over "depends on" edges (O(V + E),
  stack-safe on a long chain) and the error prints the closing chain, e.g.
  `task-001 -> task-002 -> task-001`. A new `TaskList::task_ids()` makes the
  dangling message actionable.
- The single call site is `TaskStore::write_locked` - the one function every
  persisted list passes through, whether from a `TaskStore` mutator or from the
  `TeamManager` cache's `write_through_fallible`. Validating there rather than in
  each mutator is what makes FR-038 hold: a future path cannot forget it. The
  check runs before the temp file is created, so a refusal leaves `tasks.json`
  byte-identical; the message is folded into one string
  (`refusing to write invalid <path>: <reason>`) rather than wrapped as an
  `anyhow` context, because `Display` on a context chain prints only the outer
  frame and the frame an operator needs names the task and the edge.
- `team_task_create`'s description stated that a dangling dependency "will block
  the task from being claimed" - i.e. it would persist and hang. It now spells
  out the enforced rule: every id must already name a task in the same list, the
  graph must stay acyclic, and a violation is refused at write time so the task
  is **not** created, with the recovery (check `team_task_list`, create the
  dependency first, drop the edge that closes a loop).

Coverage is `crates/ragent-agent/tests/test_task_list_write_validation.rs` (15
tests): the predicate layer (a legal chain and a diamond both validate; an empty
list validates; a dangling edge names the dependent, the missing id, the
"forever" consequence and the existing ids; a 2-cycle and a 3-cycle both report
the chain; a cycle reachable only through a legal tail is found; a
self-dependency and a duplicate id are refused; `task_ids()` order), and the
write boundary (a dangling add and a cycle-closing `update_task` are both refused
with `tasks.json` byte-identical and the bad row absent; `write_through`
validates too; a legal chain persists across three writes; and a validated edge
is one that *can* be satisfied - completing the dependency makes the dependent
claimable). Two mutations fail the suite: suppressing the `write_locked` check
fails the three write-boundary tests, and suppressing
`validate_dependencies()` fails seven. The source was restored
byte-identically after each.

Five pre-existing tests built lists that the new rule correctly refuses, and were
fixtures rather than behaviour assertions, so they were corrected to satisfy
FR-039 instead of weakening it: `test_team_task_store`
(`claim_next_respects_dependencies` now creates the prerequisite first;
`claim_specific_rejects_non_pending_and_unsatisfied` and
`pre_assign_guards_and_success` depend on a task that exists *in progress* rather
than on a missing id), `test_team_manager_extra::claim_specific_rejects_unsatisfied_dependencies`
and `test_auto_claim_tasks::auto_claim_skips_tasks_that_are_not_claimable` (same
in-progress prerequisite). Two fixtures also assigned tasks to an id the per-task
rule rejects - `test_m6_resilience` used the *session* id `old-lead` as an owner
(now `lead`, which is what a lead-owned task is called; the session id is still
what adoption matches) and `test_concurrent_store_writes`'s claimer used
`worker-N` (now `tm-NNN`) - both of which FR-038 now surfaces as the
pre-existing rule violation they were.

### M8 - UI and discoverability (T-034..T-038)

Closes F-05, F-06, F-17, F-22, F-23, F-24 on the UI side. Every task here
consumes something built in M1-M7; none of them can start earlier.

- **T-034 as built** - `team_wait` classifies every member it was asked about
  into one of four outcomes via a new private `WaitOutcome` enum
  (`tool/team_wait.rs`): `Idle`, `Parked` (`PlanPending`, `Blocked`,
  `Suspended`), `Failed`, `StillRunning` (`Spawning`, `Working`,
  `ShuttingDown`, `Stopped`). Two changes make F-17 go away. First, the waitable
  set is now "not settled" instead of a hand-spelled list that included
  `PlanPending` and `Suspended`, so a deliberately parked teammate no longer
  costs the lead the full timeout (measured: 30.001 s before, 0.013 s after).
  Second, the wait subscribes to `TeammateSuspended` as well as
  `TeammateIdle`/`TeammateFailed`, and the post-loop disk reconcile treats any
  settled status as settled - so a member parked during the wait also ends it
  promptly instead of running out the clock. `TeammateResumed` is deliberately
  not handled: a resume lands the member in `Idle`, which the reconcile already
  reads as settled. The reported outcome is derived from the member's final
  status, never invented from the event that ended the wait, so an event and a
  stale disk row cannot disagree in the output. The result is rendered as a
  `## Wait outcome` block naming each member, its status via `display_label()`,
  and its outcome; the same data is in metadata under `outcomes`, keyed by agent
  ID with `name`/`status`/`outcome`. `Stopped` stays in `StillRunning` because an
  unacknowledged shutdown is not settled. Coverage is
  `test_team_wait_outcomes` (7 tests: all four categories in one call, the text
  report, all three parked variants keeping their own status word, a parked-only
  team returning immediately, a mid-wait suspension, a timeout naming its
  straggler, and a mid-wait failure). Restoring the old waitable list fails
  `team_wait_does_not_block_on_a_parked_member` at 30 s.

### M9 - Documentation and rollout (T-039..T-040)

T-040 owns the two behaviour-change entries described in `SPEC.md`.

- **T-040 as built** - the two tool descriptions now state the rule the runtime
  enforces (FR-042), and the CHANGELOG carries the required behaviour-change
  entries with their operator migrations.

  `team_task_complete` (`crates/ragent-agent/src/tool/team_task_complete.rs`)
  previously said only that the task "must be currently assigned to the caller",
  which reads as *become* the assignee - the exact silent re-assignment FR-001
  removed. The description now spells out an `OWNERSHIP RULE (enforced)` block:
  only the assigned agent or the lead may complete; a foreign or `pending` task
  is refused and is *not* re-assigned; the refusal names the owner plus the two
  verbs that do work (`team_assign_task`, `team_task_claim`); re-completion is a
  no-op success for the owner or lead and refused otherwise; and every refusal
  reports `rejection_reason` (`not_owner`, `not_claimed`, `already_completed`)
  with the task's `owner` in the result metadata.

  `team_cleanup` (`crates/ragent-agent/src/tool/team_cleanup.rs`) already listed
  the blocking statuses. It now also states that the refusal names every blocking
  member with its status, that `force: true` is the only way past the guard, that
  a `blocked` member is *parked, not live*, so it never blocks a teardown, and
  that the guard is shared with `/team cleanup` and `/team delete` (FR-022), so
  the three surfaces refuse the same rosters with the same words.

  Coverage is two new tests in
  `crates/ragent-agent/tests/test_task_tool_family_prompts.rs`:
  `test_team_task_complete_description_states_the_ownership_rule` asserts the
  description contains `ownership rule`, `assigned agent`, `lead`,
  `not re-assigned`, both verbs, `pending`, `no-op success`, all three refusal
  tokens and `rejection_reason`;
  `test_team_cleanup_description_states_the_liveness_guard` asserts the exact
  blocking set, every parked and terminal status, `force`, and both slash
  surfaces, plus that `force` stays out of the schema's `required` array (a
  required override would offer no guard at all). Restoring either pre-T-040
  description fails its test on the first missing phrase.

  `CHANGELOG.md` gains an `[Unreleased]` section (release 1.0.153) whose two
  entries are the ones `SPEC.md` requires - **"Team task completion now requires
  ownership"** (FR-001, FR-002, FR-044) and **"Team adoption now requires
  explicit consent"** (FR-023, FR-024) - each with an *Operator migration*
  paragraph naming the verb that replaces the broken workflow (lead completes on
  the owner's behalf, or the caller claims first; `AdoptTeam::Adopt` /
  `adopt: true` for the second session). The completion entry names all three
  observation surfaces the refusal now reaches: the
  `TeamTaskCompletionRejected` event (`team_task_completion_rejected` on SSE),
  the tool metadata, and the `warn` line under `ragent_agent::task` with
  `event = "TaskCompletionRejected"` that makes `rg 'TaskCompletionRejected'
  log/` find it. The adoption entry names `TeamManager::new` (refuses) versus
  `TeamManager::try_new` + `AdoptTeam::Adopt` (proceeds) and the
  `TeamAdopted { previous_lead_session_id, reset_task_ids }` event. Alongside
  them the entry records the tightening that carries no migration (single
  status writer, task transition table, parked members leaving the watchdog,
  `Blocked` exits, `team_wait` parked outcomes, `/team delete` confirmation),
  and a `### Compatibility` section states FR-043: every `config.json` and
  `tasks.json` from the previous release loads without migration because all
  three status enums still serialise through
  `#[serde(rename_all = "lowercase")]` and no new required field was added.

  `cargo fmt --check` and `cargo clippy -p ragent-agent --all-targets` are both
  clean. Two `ragent-agent` tests fail for reasons unrelated to this task and
  were already failing: `test_m3_lifecycle::test_team_idle_publishes_teammate_idle_event`
  (the `team_idle` harness has no manager attached, from in-flight work on
  `team_idle.rs`) and
  `test_task_tools_cache_more::adopt_orphaned_tasks_leaves_tasks_owned_by_others_untouched`
  (it asserts the pre-T-029 rule that a teammate-owned `InProgress` task survives
  adoption, which FR-024/T-029 deliberately replaced). 2371 of 2373 tests pass.

## Risks

| Risk | Likelihood | Impact | Mitigation |
| --- | --- | --- | --- |
| Routing every status write through one function serialises on `config.json` `flock` and slows a many-teammate spawn | Medium | Medium | `set_member_status` writes inside the existing spawn critical section where one already exists; measure spawn of 8 teammates in M1 (NFR-001) |
| Enforcing FR-001 breaks a live operator workflow | Medium | High | `TaskCompletionRejected` event (T-013) makes the break visible; CHANGELOG entry states the migration |
| Refusing foreign-lead adoption (FR-023) breaks a two-session workflow | Medium | High | `adopt: true` retains the capability; CHANGELOG entry names it |
| Deleting the auto-claim branch in `complete` changes completion semantics for the lead | Low | Medium | The lead is exempt from the ownership rule, so lead-driven completion is unaffected |
| T-019's reading of `auto_claim_tasks` is the wrong one | Medium | Medium | The reading is stated in this plan; confirm with the operator before T-019 starts, since it is a behaviour change |
| Consolidating claim paths (T-009) regresses a subtle guard | Medium | High | The guard list is enumerated in T-009 detail; the existing `test_*team*` suite covers the guards |

## Rollout

1. M1 and M2 land together behind the existing test suite; M1 alone changes no
   behaviour, M2 changes two (FR-001, FR-044).
2. M3 through M7 land as ordinary changes.
3. M8 lands last so the UI never offers a verb the runtime cannot honour.
4. Two CHANGELOG entries are required, recorded in T-040:
   - "Team task completion now requires ownership" (FR-001, FR-002, FR-044)
   - "Team adoption now requires explicit consent" (FR-023, FR-024)

## Open Questions

| ID | Question | Owner | Blocks |
| --- | --- | --- | --- |
| Q-1 | Should `auto_claim_tasks: false` mean "teammate waits for the lead" (the reading chosen above) or "the runtime claims on the teammate's behalf"? | Operator | T-019 |
| Q-2 | Should the lead be exempt from FR-001, i.e. may the lead always complete any task? The plan assumes yes. | Operator | T-008 |
| Q-3 | Is a 300-second default `watchdog_timeout_secs` acceptable, or should the default stay at 5 minutes with no field at all? | Operator | Resolved by T-022: field adopted, 300 s default, matching the hardcoded 5-minute window it replaces |