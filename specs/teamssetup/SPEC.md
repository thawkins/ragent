---
status: draft
audit:
  - { time: 1793204755, from: "none", to: "draft", actor: "system" }
---
# Specification: Team Runtime Hardening - Consistency, Functional Gaps, and Usability

## Executive Summary

This specification records a deep analysis of the ragent **team subsystem**
(`crates/ragent-agent/src/team/`, the 21 `team_*` tools in
`crates/ragent-agent/src/tool/team_*.rs`, and the TUI `/team` slash surface in
`crates/ragent-tui/src/app/slash.rs`) and converts every finding into a testable
requirement.

The team subsystem lets one *lead* session spawn named *teammates*, share a
file-locked task list, exchange mailbox messages, gate work on plan approval, and
run quality-gate hooks. It works, but the analysis found five classes of problem:

1. **Ownership and status inconsistency.** The same logical operation enforces a
   different rule depending on which of several code paths handles it.
2. **Reachable dead states.** `MemberStatus::Blocked` can be entered but never
   left; a `Suspended` teammate is failed by the watchdog; `Cancelled` tasks stay
   claimable on two of four claim paths.
3. **Configuration that does nothing.** Settings declared in `TeamSettings` are
   never consulted, or are consulted in one place and ignored in another, so the
   operator's declared intent is silently not applied.
4. **Awkward or unsafe UI operations.** `/team cleanup`, `/team delete`, and
   `team_cleanup` disagree about what "still active" means; the lead cannot
   distinguish a blocked teammate from a busy one, and cannot clear the block.
5. **Anti-patterns in the design.** A single boolean `auto_claim_tasks` with two
   contradictory readings, three independent status writers, and a manager that
   silently adopts a team owned by a different lead session.

A sixth class emerged while cross-checking the findings against the code rather
than against the analysis notes: **a destructive verb that is not in the help
table**. `/team forcecleanup` removes the team directory after force-stopping
members, is referenced by another subcommand's refusal text, and appears in no
help table at all (F-26); `/team delete` performs the same removal with no
confirmation when the team is not the active one (F-28); and the `Teams` popup's
suspend and shutdown verbs both open a Yes/No dialog with `No` pre-selected, so
the intuitive `Space` `Enter` silently does nothing (F-27).

The requirements below are written in EARS notation, grouped by finding. Every
requirement names the observable behaviour that distinguishes a compliant
implementation, so each maps to at least one manual test case in `TESTPLAN.md`
and at least one task in `PLAN.md`.

**One finding was wrong and is left in place rather than deleted.** F-16's
second half claimed `MAX_MESSAGE_BYTES` truncates silently; `mailbox.rs:402`
actually refuses an oversized message with an error naming the limit and the
actual size. FR-021 therefore records existing behaviour as a regression guard,
and T-031 is a confirmation task rather than a fix. A spec that quietly drops
its own false findings cannot be audited later.

## Scope

### In scope

- `crates/ragent-agent/src/team/{manager,config,task,mailbox,store,swarm}.rs`
- The `team_*` tool family in `crates/ragent-agent/src/tool/`
- The TUI `/team` and `/team cleanup` / `/team delete` / `/team tasks` surfaces
- The `Teams` popup and teammate strip surfaces already defined by spec `agentui`
  (this spec only adds the state-transition contracts those surfaces display)
- `TeamSettings` fields that are declared but not honoured

### Out of scope

- The `agentui` panel layout, key bindings, and rendering rules (spec `agentui`
  owns those; this spec owns only the state they display)
- The `new_agent` sub-agent runtime (not a team member)
- Blueprint file format and discovery
- Cross-machine team coordination (teams are single-host, single-lead today)

### Definitions

| Term | Meaning |
| --- | --- |
| **Lead** | The primary session that created the team. Agent ID `lead`. |
| **Teammate** | A spawned member session. Agent ID `tm-NNN`. |
| **Terminal status** | A `MemberStatus` from which no forward transition exists except an explicit respawn: `Stopped`, `Failed`. |
| **Parked status** | A status meaning "deliberately paused, not making progress by design": `Suspended`, `PlanPending`, `Blocked`. |
| **Active member** | A member that is neither `Stopped` nor `Failed`. |
| **Live member** | A member with a live agent loop (a `TeammateHandle`): not `Stopped`, not `Failed`, not `Suspended`. |
| **Claim path** | One of the four code paths that can move a task to `InProgress`: `TaskStore::claim_next`, `TaskStore::claim_specific`, `TeamManager::claim_next_task`, `TeamManager::complete_task` (auto-claim branch). |
| **Completion path** | One of the two code paths that can move a task to `Completed`: `TaskStore::complete`, `TeamManager::complete_task`. |

## Findings Register

Each requirement below traces to one finding. The register lists the finding,
the concrete evidence, and the requirement that remediates it.

| ID | Finding | Evidence | Severity | Requirements |
| --- | --- | --- | --- | --- |
| F-01 | `Cancelled` tasks stay claimable | `Task::is_claimable` (`task.rs:144`) tests only `status == Pending`; that is correct, but `manager.rs:1750` and `task.rs:438` also treat a re-read list whose tasks were cancelled in another process as pending when the cache is stale, and `TaskStatus::Cancelled` has **no producer at all** - nothing in the workspace ever writes it, so a lead has no way to cancel a task | High | FR-005, FR-006 |
| F-02 | `TeamManager::complete_task` reassigns and completes anyone's task | `manager.rs:1781-1791`: when `assigned_to != agent_id` and the task is `Pending` or unassigned, it silently re-assigns **and** completes. `TaskStore::complete` (`task.rs:586`) applies the same auto-claim. The M6-T3 ownership check only fires when the task is already `Completed` | Critical | FR-001, FR-002 |
| F-03 | Two divergent completion implementations | `TaskStore::complete` and `TeamManager::complete_task` are near-duplicates with different error text and different idempotency wording; a fix applied to one does not reach the other | High | FR-001, FR-025 |
| F-04 | Two divergent claim implementations | `TaskStore::claim_next` / `claim_specific` versus `TeamManager::claim_next_task` / `apply_to_task_list` duplicate the claim logic including the "already has an in-progress task" guard | High | FR-025 |
| F-05 | No task cancellation surface | `Cancelled` exists in `TaskStatus`, `as_str`, and the serde rename table, but no tool, no `/team` subcommand, and no manager method writes it. A lead cannot abandon a task without editing `tasks.json` | Medium | FR-005, FR-006 |
| F-06 | `MemberStatus::Blocked` is a terminal sink | Only `crates/ragent-tui/src/app/swarm.rs:159` writes `Blocked`; nothing in `ragent-agent` reads it to unblock, and `resume_teammate` bails unless the member is `Suspended`. A `Blocked` member therefore blocks `team_cleanup` (FR-022) forever with no in-product escape | Critical | FR-009, FR-010, FR-011 |
| F-07 | Watchdog fails parked teammates | `manager.rs:1553-1560` treats `Suspended` as "still active" and therefore eligible for failure; a teammate the lead deliberately paused is marked `Failed` after the 5-minute timeout with `last_spawn_error` "watchdog: no progress" | Critical | FR-012 |
| F-08 | Watchdog has no `Blocked` branch | `manager.rs:1553-1560` matches `Working`, `Spawning`, `PlanPending`, `Suspended` - not `Blocked`, not `ShuttingDown`. The two unlisted states take an implicit path determined by an unrelated `unwrap_or(false)` | Medium | FR-012 |
| F-09 | Watchdog timeout is not configurable and not discoverable | The 5-minute timeout and 30-second tick are hardcoded; a teammate legitimately running one long build is failed with no operator recourse | Medium | FR-013 |
| F-10 | `require_plan_approval` only changes prompt text | `config.rs:324` declares it; `manager.rs:175` is the only reader and it emits a sentence into the teammate system prompt. Nothing gates write/bash tools on it, so a teammate that never submits a plan is never stopped | Critical | FR-014 |
| F-11 | `auto_claim_tasks` is read by nothing | `config.rs:326` declares it, `config.rs:337` defaults it `true`; a workspace-wide grep finds no consumer. Its doc comment ("teammates automatically claim the next available task when idle") and its default `true` imply a behaviour that does not exist | High | FR-015 |
| F-12 | `max_teammates` skipped on respawn | `manager.rs:895` guards the limit check with `if existing.is_none()`. Re-spawning an existing name bypasses the cap entirely, so an LLM loop that re-spawns one name repeatedly is unbounded | High | FR-016 |
| F-13 | Hooks fire from tools, not from the manager | `team_task_create` / `team_task_complete` / `team_task_claim` call `run_team_hook`; `TeamManager::apply_to_task_list` and every `TaskStore` mutator do not. Any path that bypasses the tool (lead-side claim, auto-claim, adoption) fires no hook | High | FR-017, FR-018 |
| F-14 | Hook refusal is not observable | `run_hook` returns `HookOutcome::Allow` on spawn failure, on timeout, and on command-validation refusal (`manager.rs:308-356`). A hook that never runs is indistinguishable from a hook that approves | Medium | FR-019 |
| F-15 | `HookOutcome::Feedback` has no manager-side consumer | The three tools interpret exit code 2; nothing else does. A hook defined on a team with no tool traffic silently does nothing | Low | FR-018 |
| F-16 | Mailbox messages have no delivery contract | `drain_unread` (`mailbox.rs:514`) marks-then-returns; a crash between the two loses the message, and a message addressed to a teammate that has already terminated is never read by anyone and is never reported. The **size** half of this finding was wrong on inspection: `mailbox.rs:402` already refuses a message over `MAX_MESSAGE_BYTES` (256 KiB) with an error naming the limit and the actual size, so FR-021 records existing behaviour as a regression guard rather than a fix | Medium | FR-020, FR-021 |
| F-17 | `team_wait` cannot distinguish "idle" from "stuck" | `team_wait` blocks until every teammate is idle with a timeout; on timeout it returns partial results with no statement of which members are parked versus genuinely unresponsive | Medium | FR-031 |
| F-18 | Task status is a flat enum with no legal-transition model | Every write path sets `status` directly. Nothing prevents `Completed -> InProgress`, `Cancelled -> Completed`, or `Completed -> Cancelled`. Legal transitions exist only in the heads of the four claim paths | High | FR-003, FR-004, FR-025 |
| F-19 | No single status writer | Member status is written at nine sites across `manager.rs`, three tools, `swarm.rs`, and the TUI event handler (`app/event_handler.rs:1816`). Each writes a status the others do not reconcile, which is the mechanical cause of F-06, F-07, and F-08 | Critical | FR-008, FR-025 |
| F-20 | `TeamManager::new` silently adopts a foreign team | `manager.rs` `TeamManager::new` loads the team by name and adopts it regardless of `lead_session_id`, mutating the foreign team's on-disk state (via `adopt_orphaned_tasks` at `manager.rs:1627`) with no warning, no event, and no operator consent | High | FR-023, FR-024 |
| F-21 | Orphan adoption is asymmetric | `adopt_orphaned_tasks` resets `InProgress` tasks owned by the *old lead* to `Pending` but leaves `InProgress` tasks owned by a departed *teammate* stranded forever | Medium | FR-024 |
| F-22 | `/team cleanup` and `team_cleanup` disagree | TUI (`slash.rs:6433`) refuses only when a member is `Working`; the tool (`team_cleanup.rs:69`) refuses when any member is `!= Stopped`. The TUI therefore deletes a team directory out from under `Blocked`, `Idle`, `PlanPending`, `Suspended`, and `ShuttingDown` members | Critical | FR-022 |
| F-23 | `/team delete` has a third rule | `slash.rs:6173` filters on `MemberStatus::Working` only, and additionally allows deleting *any* named team, including the active one, without the confirmation the tool requires | High | FR-022 |
| F-24 | Status vocabulary drifts between surfaces | `as_str()` and `display_label()` differ (`plan_pending`/`planning`, `shutting_down`/`stopping`) by design for width, but a third spelling exists in prompt text and tool error strings, so a user grepping a status word finds three answers | Low | FR-033 |
| F-25 | Suspend/resume is not symmetric with `Blocked` | `suspend_teammate` / `resume_teammate` handle exactly one status. There is no equivalent operator verb for any other parked state, so F-06 has no fix shape to copy | High | FR-010, FR-011 |
| F-26 | `forcecleanup` is an undocumented destructive subcommand | `slash.rs:6515` dispatches `forcecleanup` / `force-cleanup`, `slash.rs:6463` tells the operator to run it, and it removes the team directory after force-stopping members - but it appears in no `/team help` table and in no `COMMANDS` entry, and it bypasses the `force` argument the `team_cleanup` tool requires. A destructive verb reachable only by being told about it is a discoverability defect and a guard bypass in one | High | FR-040, FR-022 |
| F-27 | The popup's row verbs are modal but undocumented | `Space` and `Del` on the `Teams` popup both open a two-option Yes/No dialog with `No` pre-selected (`windows.rs:629`, `windows.rs:650`; `input_handler.rs:1215`). `Enter` on an un-toggled dialog **cancels**, so the intuitive `Space` `Enter` sequence silently does nothing. No footer or help text states that a toggle is required | Medium | FR-028, FR-032, FR-040 |
| F-28 | `/team delete` never asks | `slash.rs:6154` deletes a named team with no confirmation step at all when the team is not the session's active team, and with only a `Working`-count gate when it is. A destructive filesystem operation with no confirm is the same class of defect as `forcecleanup` above | High | FR-023, FR-030 |

## Requirements

### Ownership and completion (F-02, F-03, F-18)

**FR-001** (Ubiquitous)
The team runtime **shall** reject any attempt to mark a task `Completed` by an
agent that is neither the task's `assigned_to` agent nor `lead`.

**FR-002** (Unwanted behaviour)
**If** `TaskStore::complete` or `TeamManager::complete_task` receives a request
from an agent that does not own the task **and** the task's `assigned_to` is
`Some(other_agent)` with `status == InProgress`, **then** the runtime **shall**
return an error naming the current owner, and **shall not** mutate
`assigned_to`, `claimed_at`, `status`, `completed_at`, or `completed_by`.

**FR-003** (Ubiquitous)
The runtime **shall** define exactly one function that decides whether a task
transition is legal, and every write path **shall** call it before mutating a
task's `status`.

The single function is `TaskStatus::can_transition_to(&self, to: Self) -> bool`
in `crates/ragent-agent/src/team/task.rs`, with `TaskStatus::is_terminal`
(`Completed`, `Cancelled`) alongside it. Its table is design decision D3:
`Pending -> {InProgress, Cancelled}`, `InProgress -> {Completed, Pending,
Cancelled}`, and no outgoing edge from either terminal status. Self-transitions
are illegal: re-asserting a status is not a state change, so an idempotent
"already there" path tests for equality before validating. The function takes no
actor by design; the ownership restrictions D3 attaches to individual pairs
(FR-001, FR-005) are actor checks the caller applies before calling it.

**FR-004** (Event-driven)
**When** a task transition violates the legal-transition model, the runtime
**shall** return an error naming the current status, the requested status, and
the transition table row that forbids it, and **shall** leave `tasks.json`
byte-identical.

The naming error is produced by the shared `transition_task` helper (T-007),
which is the only caller of `TaskStatus::can_transition_to`; it validates before
any mutation so a refused transition never reaches `TaskStore::write_locked`.

**FR-005** (Optional)
**Where** a team is active, the lead **shall** be able to cancel a `Pending` or
`InProgress` task through both a `team_task_cancel` tool and a `/team tasks`
UI action, causing `status` to become `Cancelled` and the owning teammate (if
any) to be notified through the mailbox.

**FR-006** (State-driven)
**While** a task's status is `Cancelled`, every one of the four claim paths
**shall** refuse to claim it and **shall** report "cancelled" rather than
"pending" in its refusal message.

**FR-007** (Ubiquitous)
The runtime **shall** derive "task is claimable" from a single predicate used by
all four claim paths, so a change to the claim rule takes effect everywhere at
once.

### Member status machine (F-06, F-08, F-19)

**FR-008** (Ubiquitous)
Every write to `TeamMember::status` **shall** go through one function that
validates the transition, writes the status, and publishes the corresponding
event, so no code path can set a status without both.

**FR-009** (Ubiquitous)
The runtime **shall** expose three `MemberStatus` predicates - `is_terminal`,
`is_parked`, and `is_live` - and every guard that asks "is this teammate still
active?" **shall** use one of them rather than spelling a status list again.
Every status **shall** satisfy exactly one predicate:

| Predicate | Statuses | Meaning |
| --- | --- | --- |
| `is_terminal` | `Stopped`, `Failed` | No session; only a respawn revives it |
| `is_parked` | `Idle`, `PlanPending`, `Blocked`, `Suspended` | Alive, waiting on something external; cannot be wedged |
| `is_live` | `Spawning`, `Working`, `ShuttingDown` | Has an outstanding action it must finish on its own |

`Idle` is parked, not live: a teammate with no task cannot be wedged, and the
existing watchdog already excludes it from the failable set
(`manager.rs:1555`). `ShuttingDown` is live, not terminal: its session still
exists and must acknowledge shutdown before the member reaches `Stopped`.

**FR-010** (State-driven)
**While** a teammate's status is `Blocked`, the runtime **shall** accept the
same operator verbs that `Suspended` accepts: `resume` (returning the member to
`Idle` and clearing the block reason) and `shutdown`.

**FR-011** (Unwanted behaviour)
**If** a teammate remains `Blocked` when its last unsatisfied dependency
completes, **then** the runtime **shall** transition it to `Idle` automatically
and publish `TeammateResumed`, without requiring the lead to act.

### Watchdog (F-07, F-08, F-09)

**FR-012** (State-driven)
**While** a teammate's status is `Suspended`, `PlanPending`, or `Blocked`, the
watchdog **shall not** mark the teammate `Failed`, regardless of elapsed time.
Only a member that `is_live()` is eligible for a failure verdict.

The eligibility check is literally `MemberStatus::is_live()` in
`TeamManager::start_watchdog` (`crates/ragent-agent/src/team/manager.rs`), so the
failable set is exactly `Spawning`, `Working`, `ShuttingDown`. A parked member
(`Idle`, `PlanPending`, `Blocked`, `Suspended`) and a terminal member (`Stopped`,
`Failed`) are skipped and have their progress marker refreshed so they are not
re-examined each tick. When the team config cannot be read the member is treated
as not live: the watchdog never fails a member it cannot confirm is live.

**FR-013** (Optional)
**Where** `TeamSettings.watchdog_timeout_secs` is set, the watchdog **shall**
use that value as its no-progress timeout, and `/team help` **shall** name the
setting so the operator can discover it.

### Configuration that must do work (F-10, F-11, F-12)

**FR-014** (Optional)
**Where** `TeamSettings.require_plan_approval` is `true`, a teammate whose
`plan_status` is not `Approved` **shall** have write-class and bash-class tool
calls refused, and the refusal **shall** name `team_submit_plan` as the way to
clear the gate.

**FR-015** (Optional)
**Where** `TeamSettings.auto_claim_tasks` is `true`, a teammate that goes `Idle`
with no current task **shall** be given the next claimable task automatically;
**where** it is `false`, the teammate **shall** stay `Idle` until the lead
assigns or the teammate claims explicitly.

**FR-016** (Unwanted behaviour)
**If** a spawn would take the number of active members above
`TeamSettings.max_teammates`, **then** the spawn **shall** be refused, whether
the name is new or already on the roster.

The count is the roster the spawn would produce, so the cap decides by *slots*,
not by *calls*: a name that is not on the roster, or is on it as a terminal
(`Stopped`/`Failed`) member, occupies no active slot and its spawn adds one; a
respawn of a member that is still active (`!is_terminal()`) reuses the slot it
already holds and is therefore permitted at the cap. `max_teammates == 0` means
unlimited.

### Hooks (F-13, F-14, F-15)

**FR-017** (Ubiquitous)
The `TaskCreated`, `TaskCompleted`, `TaskClaimed`, `TeammateSpawned`, and
`TeammateStopped` hooks **shall** fire from the team runtime's own mutation
path, so a lead-side claim, an auto-claim, and an adoption each fire the same
hooks as the equivalent tool call.

**FR-018** (Event-driven)
**When** a `TaskCompleted` hook returns `HookOutcome::Feedback`, the runtime
**shall** deliver that feedback to the completing agent as a tool result
regardless of which path completed the task.

**FR-019** (Unwanted behaviour)
**If** a configured hook cannot be executed because its command failed
validation, failed to spawn, or exceeded the 30-second timeout, **then** the
runtime **shall** record the hook's name, the reason, and the timestamp in the
team's `hook-failures.jsonl` and **shall** surface the count in `/team show` and
`team_status`, so an inert hook is not mistaken for a passing one.

### Mailbox (F-16)

**FR-020** (Unwanted behaviour)
**If** a teammate terminates between a message being enqueued and its first
`team_read_messages` call, **then** the runtime **shall** leave the message
unread and **shall** report it in `/team show` as an undelivered message for the
lead, rather than dropping it.

**FR-021** (Ubiquitous)
A message whose content exceeds `MAX_MESSAGE_BYTES` **shall** be rejected at
send time with an error naming the limit and the actual size, and **shall not**
be silently truncated.

### Lifecycle guards (F-22, F-23)

**FR-022** (Ubiquitous)
`/team cleanup`, `/team delete`, and the `team_cleanup` tool **shall** all use
the same guard - "no member is `is_live()` or `ShuttingDown`" - and **shall**
report the same refusal text listing every blocking member by name and status.

The guard is `blocks_teardown` / `blocking_members` / `cleanup_refusal` in
`crates/ragent-agent/src/team/mod.rs` (T-027). `/team delete` additionally
**shall** treat its `force` override as a request to confirm rather than a
permission to delete (T-028): with live members present, the first
`force`-bearing invocation **shall** print the `delete_confirmation` text, arm
the confirmation for that exact team name, and remove nothing; only a second
identical invocation **shall** perform the removal, and its success message
**shall** state how many live members were forced. A named team that is not the
session's active team **shall** be checked against its persisted roster, not the
in-memory one, so the "delete any named team with no check" path of F-28 is
closed. Since T-043 that confirmation covers a roster with nothing live as well
(FR-045); `cleanup_refusal` remains the gate that decides whether a delete may
proceed at all, and `delete_confirmation` decides only whether this command is
the confirmed second action.

**FR-023** (Unwanted behaviour)
**If** `TeamManager::new` loads a team whose `lead_session_id` differs from the
current session, **then** the manager **shall not** mutate that team's state;
it **shall** return an error naming the owning lead session, and only an
explicit `adopt: true` argument **shall** allow adoption.

**FR-024** (Event-driven)
**When** a manager adopts a team owned by a different lead session, the runtime
**shall** reset every `InProgress` task to `Pending` regardless of whether the
previous owner was the old lead or a departed teammate, and **shall** publish
one `TeamAdopted` event listing each task it reset.

### Design consistency (F-03, F-04, F-25)

**FR-025** (Ubiquitous)
The runtime **shall** expose exactly one claim implementation and exactly one
completion implementation, with `TaskStore` and `TeamManager` both delegating to
them, so no behavioural difference can exist between the cached and uncached
paths.

**FR-026** (Ubiquitous)
The runtime **shall not** contain two functions that differ only in which of the
lead / teammate / cached / uncached shape they assume; a divergence in error
text between two paths to the same operation **shall** be treated as a defect.

**FR-027** (Optional)
**Where** `TeamSettings.auto_claim_tasks` is `false`, the teammate system prompt
**shall** state that the teammate must not call `team_task_claim` without an
explicit lead instruction, so the prompt and the enforced behaviour agree.

### Usability of the UI surfaces (F-05, F-06, F-17, F-22, F-23, F-24)

**FR-028** (State-driven)
**While** a team has one or more `Blocked` or `Suspended` members, the `Teams`
popup **shall** show a hint row naming the resume action, so the operator can
see that the state has a way out.

**While** any row verb is modal, the popup footer **shall** state that the
dialog's selection must be toggled before `Enter` accepts, so `Space` `Enter`
cannot silently cancel an action the operator believed they had confirmed.

**FR-029** (Event-driven)
**When** the lead presses `Del` on a `Blocked` member in the `Teams` popup, the
runtime **shall** offer the same confirm-then-resume path it offers for
`Suspended`, and the confirmation **shall** name the member and the action.

**FR-030** (Unwanted behaviour)
**If** a `/team tasks` row is selected for cancellation or reassignment, the
runtime **shall** refuse the action while the task's owner is `is_live()` unless
the operator confirms, so a live teammate's work is never silently reassigned.

**FR-031** (Ubiquitous)
`team_wait` and `/team wait` **shall** report, for every member they were
waiting on, whether it ended the wait as `Idle`, as parked, as failed, or still
running when the timeout fired.

The classification is `WaitOutcome::of` in `tool/team_wait.rs`: `Idle`;
parked for `PlanPending`, `Blocked`, `Suspended`; `Failed`; and still running for
`Spawning`, `Working`, `ShuttingDown`, `Stopped`. Each category is reported with
the member's own status word from `MemberStatus::display_label()`, so "parked"
never hides which kind of park it was, and `Stopped` counts as still running
because an unacknowledged shutdown is not a settled member.

Two consequences follow and both **shall** hold: a member that is already parked
**shall not** be awaited, so a pause never consumes the wait budget, and a member
parked during the wait (`TeammateSuspended`) **shall** settle it. The outcome
**shall** be derived from the member's final on-disk status rather than from the
event that ended the wait, so a stale row cannot be reported as a state the store
does not hold.

**FR-032** (Ubiquitous)
Every refusal produced by a team tool **shall** name the offending entity, its
current state, and one concrete next action, so the operator does not have to
consult `/team help` to recover.

A confirmation dialog **shall** state whether its pre-selected option performs
or cancels the action, so "pressed `Enter` and nothing happened" is never the
observed behaviour.

**FR-033** (Optional)
**Where** a status word is rendered, all surfaces **shall** take it from
`MemberStatus::display_label()`, and no surface **shall** introduce a spelling
that function does not produce.

**FR-034** (State-driven)
**While** a teammate is `PlanPending`, the `Teams` popup **shall** show the plan
waiting for approval and the approve/reject action, rather than the member's
elapsed time only.

### Observability (F-19, F-24)

**FR-035** (Ubiquitous)
Every member-status transition **shall** emit exactly one event carrying the
agent ID, the previous status, the new status, and the cause, so the event
stream alone can reconstruct the status history.

The event is `Event::TeammateStatusChanged { agent_id, from, to, cause }`
(published once per write by `TeamManager::set_member_status`), and the cause is
drawn from the closed `TransitionCause` vocabulary - one variant per reason a
writer moves a member (`spawned`, `spawn_failed`, `task_claimed`,
`task_completed`, `reported_idle`, `plan_submitted`, `plan_approved`,
`plan_rejected`, `dependency_blocked`, `dependency_satisfied`, `suspended`,
`resumed`, `shutdown_requested`, `shutdown_acked`, `watchdog_timeout`,
`task_cancelled`, `team_cleaned_up`) - so a reader can answer "why did this
teammate stop?" from the event alone rather than by inferring it from the
ordering of the coarser `TeammateIdle` / `TeammateFailed` / `TeammateResumed`
events.

**FR-036** (Ubiquitous)
`team_status` **shall** report a per-member status history of the last five
transitions with timestamps, so a "why did this teammate stop?" question is
answerable from the tool surface.

### Robustness (F-01, F-20)

**FR-037** (Unwanted behaviour)
**If** `tasks.json` fails to parse, **then** the runtime **shall** return an
error naming the file, the byte offset, and the offending field, and **shall
not** silently fall back to `TaskList::default()`, which today discards every
task on a malformed file.

**FR-038** (Ubiquitous)
The runtime **shall** validate `tasks.json` against `TaskList::validate()` after
every write and **shall** refuse to persist a list that fails validation.

**FR-039** (Unwanted behaviour)
**If** a task `depends_on` a task ID that does not exist in the list, **then**
the runtime **shall** report the cycle or the dangling dependency at write time
rather than leaving the task unclaimable forever.

### Documentation and discoverability (F-09, F-11, F-22)

**FR-040** (Ubiquitous)
`/team help` **shall** document, for every subcommand, whether it requires the
active team, whether it mutates disk, and which guard can refuse it, and that
list **shall** include `forcecleanup`, which today is dispatched
(`slash.rs:6515`) and referenced by another subcommand's refusal text
(`slash.rs:6463`) but appears in no help table.

**FR-041** (Optional)
**Where** a `TeamSettings` field exists, `/team show` **shall** print its
effective value alongside the field name, so an unhonoured setting is visible.

**FR-042** (Ubiquitous)
The `team_task_complete` and `team_cleanup` tool descriptions **shall** state
the ownership rule and the liveness guard the implementation enforces, so the
model's expectation matches the runtime.

### Backward compatibility

**FR-043** (Ubiquitous)
The runtime **shall** load every `config.json` and `tasks.json` written by the
previous release without migration, and **shall** keep the existing snake_case
on-disk spellings for every `TaskStatus`, `MemberStatus`, and `TeamStatus`.

**FR-044** (Event-driven)
**When** the ownership rule in FR-001 rejects a completion that the previous
release accepted, the runtime **shall** emit a `TaskCompletionRejected` event so
the behaviour change is visible in the log rather than appearing as a silent
regression.

**FR-045** (State-driven)
**While** `/team delete` names any team, whether or not it is the session's
active team, the runtime **shall** require an explicit confirmation before
removing the team directory, and **shall not** delete on a single unconfirmed
`Enter`.

The confirmation is `delete_confirmation` in
`crates/ragent-agent/src/team/mod.rs`, which **shall** return `Some` for every
roster that is not excluded - including a roster with nothing live and an empty
one - so a live member is no longer the condition for it. The text **shall**
name the team, **shall** name every blocking member with its status when there
is one, and **shall** state that nothing has been deleted.

The confirming action **shall** be the same command carrying a `force` spelling
(`--force`, `force`, `force: true`, `force=true`), and it **shall** only apply to
the team the confirmation was armed for: a `delete` naming a different team
**shall** disarm the arm and confirm the new team instead. The runtime **shall**
provide an escape (`cancel` / `--cancel`) that disarms the confirmation, removes
nothing, and reports the cancellation.

## Non-Functional Requirements

| ID | Requirement |
| --- | --- |
| NFR-001 | The status-transition function of FR-008 shall be lock-free on its hot path; it may load-then-store `config.json` under the existing `flock`. |
| NFR-002 | Adding the three `MemberStatus` predicates (FR-009) shall not change the on-disk schema. |
| NFR-003 | The unified claim/completion path (FR-025) shall keep the `TaskStore` exclusive `flock` for the whole read-modify-write cycle. |
| NFR-004 | The hook-failure ledger (FR-019) shall be append-only and bounded to 1 MiB with rotation. |
| NFR-005 | No requirement in this spec may introduce an `unsafe` block. |
| NFR-006 | No requirement in this spec may add an `unwrap()` or `expect()` on a user-facing path. |

## Requirement Coverage Summary

| Group | Requirements | Count |
| --- | --- | --- |
| Ownership and completion | FR-001..FR-007 | 7 |
| Member status machine | FR-008..FR-011 | 4 |
| Watchdog | FR-012..FR-013 | 2 |
| Configuration that must do work | FR-014..FR-016 | 3 |
| Hooks | FR-017..FR-019 | 3 |
| Mailbox | FR-020..FR-021 | 2 |
| Lifecycle guards | FR-022..FR-024 | 3 |
| Design consistency | FR-025..FR-027 | 3 |
| Usability of the UI surfaces | FR-028..FR-034 | 7 |
| Observability | FR-035..FR-036 | 2 |
| Robustness | FR-037..FR-039 | 3 |
| Documentation and discoverability | FR-040..FR-042 | 3 |
| Backward compatibility | FR-043..FR-044 | 2 |
| Lifecycle confirmation | FR-045..FR-045 | 1 |
| **Total** | | **45** |

## EARS Template Coverage

| Template | Requirements |
| --- | --- |
| Ubiquitous | FR-001, FR-003, FR-007, FR-008, FR-009, FR-017, FR-021, FR-022, FR-025, FR-026, FR-031, FR-032, FR-035, FR-036, FR-038, FR-040, FR-042, FR-043 |
| Event-driven | FR-004, FR-018, FR-024, FR-029, FR-044 |
| State-driven | FR-006, FR-010, FR-012, FR-028, FR-034, FR-045 |
| Optional | FR-005, FR-013, FR-014, FR-015, FR-027, FR-033, FR-041 |
| Unwanted behaviour | FR-002, FR-011, FR-016, FR-019, FR-020, FR-023, FR-030, FR-037, FR-039 |

## Behaviour Changes Requiring a CHANGELOG Entry

1. **FR-001 / FR-002 / FR-044 - completion ownership is enforced.** A teammate
   that completes a task it does not own now receives an error instead of having
   the task reassigned to it. Any operator workflow that relied on "anyone can
   close any task" must switch to the lead completing on the owner's behalf.
2. **FR-023 - team adoption is explicit.** A second session that opens a team
   owned by a different lead session now gets an error instead of silently
   adopting and mutating that team. Operators who run two sessions against one
   team directory must pass `adopt: true`.

Both changes are tightening-only; neither removes a capability, and both are
visible in the event stream (FR-044, FR-024) rather than failing silently.

## References

- `crates/ragent-agent/src/team/manager.rs` - `TeamManager`, watchdog,
  `complete_task`, `claim_next_task`, adoption, hooks
- `crates/ragent-agent/src/team/task.rs` - `TaskStatus`, `Task`, `TaskList`,
  `TaskStore` claim/complete
- `crates/ragent-agent/src/team/config.rs` - `TeamSettings`, `MemberStatus`,
  `TeamStatus`, `PlanStatus`
- `crates/ragent-agent/src/team/mailbox.rs` - `MessageType`, `Mailbox`,
  `drain_unread`
- `crates/ragent-agent/src/tool/team_*.rs` - the 21 team tools
- `crates/ragent-tui/src/app/slash.rs` - `/team` surface, `/team cleanup`
  (line 6426), `/team delete` (line 6154)
- `crates/ragent-tui/src/app/swarm.rs:159` - the only `MemberStatus::Blocked`
  producer
- Spec `agentui` - the `Teams` popup and teammate strip layout this spec feeds
