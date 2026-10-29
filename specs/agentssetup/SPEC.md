---
status: draft
audit:
  - { time: 1793228106, from: "none", to: "draft", actor: "system" }
---
# Specification: Sub-Agent Tool Family and Agent-Control Surface - Consistency, Gaps and Usability

## Executive Summary

This specification covers the **sub-agent tool family** (`new_agent`, `cancel_agent`,
`list_agents`, `wait_agents`), the **Agents panel** that projects the same task registry
into the TUI, and the **sibling asynchronous-work surfaces** that share the same mental
model: the `bg` tool over `BackgroundTaskService`, the `/spawn` and `/cancel` slash
commands, and the sub-agent REST routes on the server.

The family is *functionally complete* - a sub-agent can be spawned, observed,
awaited, suspended, resumed and killed - but it was grown by accretion and the
seams show. Three different execution contracts are exposed under one tool name, at
least one parameter is silently ignored in a plausible call shape, one refusal is
delivered as a *successful* tool call, one cancellation path is a UI-local lie, and
the presentation layer (`note_subagent_started`) hard-codes a field it does not
have. This document is a **requirements** specification: it states what the family
must do, not what it currently does, and the accompanying analysis sections name the
exact file and line where the current behaviour diverges.

### Why this exists

`new_agent` is the single most consequential tool in an autonomous run: it decides
how much work happens in parallel and how much of the result reaches the parent's
context window. Four tools, one panel and three sibling surfaces all describe
"work happening elsewhere", and they disagree with each other on vocabulary
(`background` / `detached` / `bg` / shell task), on limit semantics, on when a row
disappears, and on what a cancellation means. A model reading four tool
descriptions and a prose prompt cannot form one consistent model of the subsystem;
neither can a human reading the Agents panel. Every inconsistency below has been
observed in the source, not inferred.

## Scope

**In scope**

- `crates/ragent-agent/src/tool/new_agent.rs`
- `crates/ragent-agent/src/tool/cancel_agent.rs`
- `crates/ragent-agent/src/tool/list_agents.rs`
- `crates/ragent-agent/src/tool/wait_agents.rs`
- `crates/ragent-agent/src/tool/bg.rs` and `crates/ragent-agent/src/background/mod.rs`
- `crates/ragent-agent/src/task/mod.rs` (`AgentManager`, `TaskEntry`, `TaskStatus`)
- `crates/ragent-agent/src/agent/mod.rs` (the sub-agent guidance block in the system prompt)
- `crates/ragent-tui/src/app/subagent_events.rs`, `crates/ragent-tui/src/app/spawn.rs`,
  the `/cancel` arm of `crates/ragent-tui/src/app/slash.rs`, and the Agents panel
  renderers
- `crates/ragent-server/src/routes/mod.rs` sub-agent routes

**Out of scope**

- The team subsystem (`team_*` tools, `TeamManager`). Teammates are a separate
  runtime with their own specification (`specs/teamssetup`); this document only
  constrains the *boundary* where `new_agent` refuses inside a team and where a
  team task must not be confused with a sub-agent task.
- Provider-specific model behaviour, cost accounting, and the LLM client.
- A durable cross-restart task log. The current registry is in memory and reaped;
  making sub-agent history survive a restart is a product decision recorded as an
  analysis-only finding (AN-002) rather than a requirement here.

## Terminology

| Term | Meaning |
|------|---------|
| **Sub-agent task** | One run of an agent, created by `new_agent`, tracked in `AgentManager::tasks` as a `TaskEntry`, identified by a task id such as `explore-a1b2c3d4`. |
| **Foreground task** | A `background: false` sub-agent task. `new_agent` blocks until it finishes and returns the response inline. |
| **Background task** | A `background: true`, `detached: false` sub-agent task. It is listed by `list_agents`, awaitable by `wait_agents`, and its result is injected into the parent session on completion. |
| **Detached task** | A `background: true`, `detached: true` sub-agent task. It is excluded from `list_agents` and `wait_agents` and its result is never injected. |
| **Shell task** | A `bg`-tool task managed by `BackgroundTaskService` and persisted in storage. Not a sub-agent task; a different type with a different id space (UUID) and a different lifecycle. |
| **Limit** | `max_background` (`experimental.max_background_agents`, default 8), the cap on concurrently *running background* sub-agent tasks. |
| **Task registry** | `AgentManager::tasks`, the authoritative in-memory record of sub-agent tasks. |

## Analysis Findings

Each finding names the file evidence and is referenced by the requirements that
close it.

### A - `new_agent` exposes three execution contracts under one name

`new_agent` (`tool/new_agent.rs:162-230`) selects between `spawn_detached`,
`spawn_background` and `spawn_sync` from two independent booleans. Consequences:

- **A1 - The `detached` flag is silently ignored when `background` is absent.**
  `background` defaults to `false` (`new_agent.rs:132-135`) and `detached` is only
  consulted inside the `if background` branch (`new_agent.rs:163`). A call with
  `{"detached": true}` and no `background` runs a *foreground*, blocking task - the
  opposite of what the caller asked for - and reports success.
- **A2 - `additionalProperties: false` is declared but not enforced.**
  `new_agent.rs:78`, `cancel_agent.rs:38`, `list_agents.rs:51`, `wait_agents.rs:101`
  and `bg.rs:100` all declare it. No tool in the family rejects an unknown key; a
  misspelled `backgound: true` degrades to foreground silently.
- **A3 - No timeout parameter.** The only bound is the global
  `background_timeout_secs` applied as a *stall* timeout (`task/mod.rs:713,730-755`)
  and `wait_agents`' own `timeout_secs` (default 300, `wait_agents.rs:121-124`). A
  caller who knows a task should not run 20 minutes has no way to say so per call.
- **A4 - Inconsistent permission category.** `new_agent` is `agent:spawn`
  (`new_agent.rs:83`), `cancel_agent` is also `agent:spawn` (it is a *control*
  action, `cancel_agent.rs:42`), `wait_agents` is `agent:control`
  (`wait_agents.rs:108`) and `list_agents` is `"none"` (`list_agents.rs:54`). Four
  tools over one registry, three categories, and the read-only one is categorised as
  "no category" rather than a read category.
- **A5 - The `model` parameter accepts two separator conventions** and documents
  both (`"provider/model"` in the rustdoc at `new_agent.rs:27-28`,
  `"provider:model"` in the schema at `new_agent.rs:74`) without saying which one is
  authoritative.
- **A6 - `spawn_sync` has no length bound.** A foreground task is not subject to
  `max_background` and blocks the tool loop; a runaway foreground agent has no
  timeout at all on the `spawn_sync` path (`task/mod.rs:419-543` has no
  `tokio::time::timeout`).

### B - Cancellation vocabulary is not reconciled with task state

- **B1 - `cancel_agent` reports `"cancelled"` for a task that is still `Running`.**
  `AgentManager::cancel_agent` (`task/mod.rs:866-876`) only *sets a flag*; the tool
  then returns `status: "cancelled"` (`cancel_agent.rs:88-94`). The entry stays
  `Running` until the agent loop observes the flag at a step boundary. The tool
  claims a state transition that has not happened.
- **B2 - A killed row disappears by reconciliation, not by event.** `kill_task`
  publishes `SubagentKilled` (`task/mod.rs:1015-1020`) but the TUI's
  `SubagentKilled` arm does not exist in `subagent_events.rs` (the module handles
  `Started` / `Completed` / `Cancelled` / `Suspended` / `Resumed` only); the row is
  removed by the 1.5 s registry reconciliation instead
  (`state.rs:5323-5340`, "cancel `Terminating`" in the merge).
- **B3 - `wait_agents` renders cancellation and failure identically.** A cancelled
  task is collected with `success = false` (`wait_agents.rs:186`, and the event arm
  at `:247`), and the final metadata hard-codes
  `"status": if *success { "completed" } else { "failed" }`
  (`wait_agents.rs:406`). A waiter cannot distinguish "the provider returned an
  error" from "somebody cancelled this", though `TaskStatus` has the distinction.
- **B4 - A suspended task is invisible to `wait_agents`.** `waiting_for` is seeded
  from `Running | Completed | Failed | Cancelled` (`wait_agents.rs:150-167`), so a
  suspended task is neither awaited nor reported; with a timeout the call reports
  "N task(s) still running" while the task is in fact parked.

### C - The limit and the refusal surface are inconsistent

- **C1 - A team refusal is delivered as a successful tool call.**
  `new_agent` returns `Ok(ToolOutput { content: guidance, metadata: {blocked: true} })`
  inside an active team (`new_agent.rs:110-119`). Every other precondition in the
  family is an `Err` (`new_agent.rs:125,130,155`). The model sees a normal result
  whose content is prose.
- **C2 - Suspended and terminating tasks are not counted against the limit.**
  `spawn_background_mode` counts `status == Running && background`
  (`task/mod.rs:603-616`). A suspended background task therefore frees a limit slot
  while it continues to consume its wall-clock budget and will resume later - the
  count is a policy decision, not a description of live work.
- **C3 - The limit error is a `bail!` with no machine-readable reason**, so a caller
  can only pattern-match the English text (`task/mod.rs:608-614`).
- **C4 - `list_agents` cannot report the limit.** It reports
  `task_count` / `running_count` (`list_agents.rs:170-176`), so a caller has no way
  to learn headroom before it attempts a spawn.
- **C5 - `new_agent`'s success content has no `output_file`** because the file is
  written asynchronously for background tasks and the tool returns before it
  exists; the field appears only later, in `list_agents` / `wait_agents`.

### D - Anti-patterns in the implementation

- **D1 - The tool description prose has already drifted.** `new_agent`'s
  `description()` (`new_agent.rs:37-50`) omits `detached` entirely and omits the
  `provider:model` spelling of `model`; both are in the schema. The description is
  the primary artefact the model reads.
- **D2 - `&task.id[..8]` is a byte slice on a possibly non-ASCII id.**
  `list_agents.rs:134-138` slices by bytes while the shared helper
  `ragent_types::strutil::short_prefix` (`strutil.rs:194`) exists precisely to cut on
  a character boundary, and `wait_agents.rs:330` already uses it. Two ids of the same
  registry are shortened two different ways, and one of them can panic.
- **D3 - `/spawn` hand-rolls a launch slot and a helper runtime.**
  `app/spawn.rs:104-181` guards a `Mutex<Option<Result<String,String>>>` so only one
  launch is in flight, and on a thread without a reactor builds a
  `current_thread` runtime to run `spawn_detached`. The command therefore has a
  state machine (`[wait] launching` / `poll_spawn_result`) that no other launch
  path has, and it silently drops the outcome if the mutex is poisoned
  (`spawn.rs:152-155`).
- **D4 - `Drop` claims prompts it cannot stop.** A drop-guard pattern that reports
  "stopping" on scope exit cannot guarantee the underlying tokio task has observed
  the cancel flag before the process moves on; the claim is optimistic by
  construction.
- **D5 - Three ways to say "the result is on disk".** `output_file` is populated only
  on the success arm of `spawn_background_mode` (`task/mod.rs:790-800`) and only
  when `persist_task_output` succeeded; the `None` path logs a warning
  (`task/mod.rs:803-810`) but no tool reports "no durable copy exists", so a failed
  persist is invisible to the model that later tries to `read` the path.

### E - The Agents panel and the slash commands

- **E1 - `note_subagent_started` hard-codes `detached: false`.**
  `app/subagent_events.rs:92` builds the painted `TaskEntry` with `detached: false`
  unconditionally, so a detached `/spawn` row and an ordinary background row are
  visually and structurally identical even though the underlying entries differ in
  whether they are listed, awaitable or injected.
- **E2 - The Agents panel is live-only.** `note_subagent_completed` removes the row
  (`subagent_events.rs:132-138`), so a finished task's result can be read only from
  the log panel or the `log/subagents/<task-id>.md` file - the panel offers no
  completed history.
- **E3 - `/cancel` is a UI-local lie.** The `/cancel` arm
  (`app/slash.rs:4459-4474`) finds `active_tasks.find(|t| t.id.starts_with(args))`,
  removes the row, and prints "Cancelled task ...". It never calls
  `AgentManager::cancel_agent`, so the sub-agent keeps running, its completion
  event still arrives, and the row can be re-added by reconciliation.
- **E4 - Prefix matching has no ambiguity check.** `starts_with(args)`
  (`app/slash.rs:4459`) picks the first match in `Vec` order; with
  `explore-a1b2c3d4` and `explore-a1b2c3d5` live, `/cancel explore-a` cancels
  whichever happens to be earlier, with no confirmation and no "ambiguous prefix"
  message.
- **E5 - ~~The Agents window has no keyboard open chord.~~** *(Closed by T-019 and
  T-028.)* `toggle_agents_window` was called only from the mouse router
  (`input_handler.rs:667`); every other panel had an `Alt` chord (`Alt+C`, `Alt+M`,
  `Alt+T`, `Alt+Q` - the Tasks panel has since moved to `Alt+I`). Once the window was
  open it was keyboard-first, but opening it required a mouse, so a keyboard-only
  terminal user could not reach the panel at all. `Alt+A` now opens and closes it,
  `Alt+T` does the same for the Teams window - which was equally mouse-only - and
  both chords close an already-open window regardless of whether its subject still
  exists (FR-046).
- **E6 - The `bg` and sub-agent rows share a panel but not a vocabulary.**
  `bg_tasks` is rendered in the same window (`layout_active_agents.rs:583-600`)
  under a separate section, but the `bg` tool calls them "background tasks" and the
  sub-agent family calls *its* tasks "background tasks" too, with different limits
  (none for `bg`), different cancellation and different id shapes.
- **E7 - `bg` `list` and `cleanup` accept a `session_id` override** (`bg.rs:167,
  279`) that lets one session list or delete another session's shell tasks, while
  `cancel_agent` explicitly refuses a task from another session
  (`cancel_agent.rs:69-75`). The two families disagree on cross-session safety.

### F - Observability gaps

- **F1 - The limit and the timeout are invisible at runtime.** Nothing in the Agents
  panel, the status bar or `list_agents` reports the current limit, the current
  occupancy or a task's remaining wall-clock budget.
- **F2 - There is no waiter visibility.** `waiter_count` exists
  (`task/mod.rs:314-320`) but no tool or panel exposes it, so two agents waiting on
  the same task are indistinguishable from none.
- **F3 - A `Suspended` row has no age.** The panel paints the status and an elapsed
  time, but not how long it has been parked, which is the number that decides
  whether to resume or kill it.

### G - Documentation drift

- **G1** `create_default_registry`'s rustdoc (`tool/mod.rs:1744-1747`) lists
  `list_agents` but not `wait_agents`.
- **G2** `README.md`'s "Agent (4)" group names the four tools but does not describe
  the three execution contracts or the limit.
- **G3** The system-prompt guidance block (`agent/mod.rs:2880-2925`) teaches
  `background: true` and the limit but never mentions `detached`, so the one
  parameter that changes the result-delivery contract is undocumented where the
  model is most likely to read.

### H - Missing test coverage

- **H1** No test asserts that `detached: true` without `background` is rejected (or
  that it degrades) - see A1.
- **H2** No test asserts that an unknown parameter is rejected - see A2.
- **H3** No test asserts the reported status of a cancelled-but-still-running task -
  see B1.
- **H4** No test asserts that `wait_agents` distinguishes cancelled from failed - see
  B3.
- **H5** `list_agents` (`tests/test_list_agents_tool.rs`) and `bg`
  (`tests/test_bg_tool.rs`) have coverage; `/cancel`'s delegation to
  `AgentManager::cancel_agent` does not exist to be covered.

### Analysis-only findings (recorded, not specified)

- **AN-001 - Durable cross-restart sub-agent task history.** The registry is in
  memory and reaped by `drain_completed` (`task/mod.rs:1131-1168`); a restart loses
  every task except the `log/subagents/*.md` files. Whether sub-agent history
  should be a first-class durable artefact is a product decision.
- **AN-002 - Pre-emptive termination.** `kill_task` is cooperative with a 10-second
  escalation (`task/mod.rs:1021-1043`). Terminating without waiting for a loop safe
  point would require the processor to expose an abort handle, which is an
  architecture decision beyond this scope.

## Requirements

### Ubiquitous requirements

**FR-001 - One tool, three named execution contracts.**
The `new_agent` tool shall implement exactly three execution contracts and shall name
them consistently in its schema, its description and its system-prompt guidance:
*foreground* (`background: false`), *background* (`background: true, detached: false`)
and *detached* (`background: true, detached: true`).

**FR-002 - Every declared parameter is honoured or refused.**
The sub-agent tools (`new_agent`, `cancel_agent`, `list_agents`, `wait_agents`) shall
either honour every parameter declared in their `parameters_schema` or reject the call
with an error that names the parameter and the reason it was refused; no declared
parameter shall be silently ignored in any reachable call shape.

**FR-003 - `detached` without `background` is refused, not degraded.**
`new_agent` shall reject `detached: true` when `background` is absent or `false`,
because a detached-foreground contract does not exist; the error shall name both
parameters and the two legal combinations.

**FR-004 - Consistent parameter validation across the family.**
Every sub-agent tool that declares `additionalProperties: false` shall reject a call
carrying an unknown property, and the error shall name the offending key.

**FR-005 - One permission taxonomy for one registry.**
The four sub-agent tools shall be classified as: `agent:spawn` for `new_agent`,
`agent:control` for `cancel_agent` and `wait_agents`, and a read-only category for
`list_agents`; no sub-agent tool shall be classified `none`.

**FR-006 - `model` overrides accept one documented spelling set.**
Every place that documents the `new_agent` `model` parameter (schema description,
tool description, system prompt, `README.md`) shall document the same accepted
spellings (`provider/model` and `provider:model`), and the schema shall say which is
preferred.

**FR-007 - Status vocabulary is shared, not re-derived.**
Every tool that reports a task status shall emit the canonical
`TaskStatus::as_str()` value (`running`, `suspended`, `terminating`, `completed`,
`failed`, `cancelled`) or a documented refinement of it; no tool shall collapse two
distinct statuses into one label.

**FR-008 - Short identifiers are produced by one helper.**
Every renderer or formatter that abbreviates a task id or session id shall use
`ragent_types::strutil::short_prefix`, and shall never slice a string by byte offset.

**FR-009 - Refusals are errors, not successful results.**
A precondition failure in a sub-agent tool (team context active, limit reached,
manager unavailable, task owned by another session) shall be returned as a tool
error whose message states the condition and the supported alternative; no
precondition failure shall be returned as a successful tool output whose content is
prose.

**FR-010 - Refusal and limit failures carry a machine-readable reason.**
Every sub-agent tool error that a caller could branch on (limit reached, wrong
session, ambiguous prefix, refused contract) shall carry a stable machine-readable
reason code in the tool result metadata alongside the human message.

**FR-011 - The limit is reported, not just enforced.**
`list_agents` shall report the configured background limit, the number of live
background tasks occupying it, and the resulting headroom in both its content and its
metadata.

**FR-012 - Waiter visibility.**
`list_agents` detail mode and the Agents panel shall report, per live task, the
number of active waiters (`waiter_count`) when it is greater than zero.

**FR-013 - A cancelled task reports the state it is actually in.**
`cancel_agent` shall report the task's *current* status after the cancel request,
shall distinguish "cancel requested" from "cancelled", and shall not claim a terminal
state the registry has not reached.

**FR-014 - Cancellation and failure are distinguishable.**
`wait_agents` output and metadata shall report a cancelled task as `cancelled` and a
failed task as `failed`, with the same distinction `TaskStatus` records.

**FR-015 - A suspended task is not reported as running.**
`wait_agents` shall report a suspended task as `suspended` in its partial-result
listing and shall not describe a parked task as "still running".

**FR-016 - The tool description is generated from the contract.**
The `new_agent`, `cancel_agent`, `list_agents` and `wait_agents` descriptions shall
name every parameter they accept and every execution contract they support, and a
test shall assert that each description mentions each schema property name.

**FR-017 - Per-call bound on foreground and background sub-agents.**
`new_agent` shall accept an optional per-call time bound and shall terminate the task
with a `TimedOut`-style failure when the bound is exceeded; the bound shall not
override the configured stall timeout, it shall only tighten it.

**FR-018 - Durable-copy absence is visible.**
When a completed task has no `output_file`, `list_agents` and `wait_agents` shall say
so explicitly rather than omitting the field, so a caller does not attempt to `read`
a path that does not exist.

**FR-019 - `bg` shell tasks keep their own vocabulary and their own safety.**
The `bg` tool shall continue to manage shell tasks, shall describe them as "shell
tasks" (never "background tasks") so they cannot be confused with sub-agent
background tasks, and shall refuse a `session_id` override that names a session other
than the calling session.

**FR-020 - The Agents panel paints the real execution contract.**
The Agents panel shall paint, per sub-agent row, the row's execution contract
(foreground / background / detached) and shall derive it from the task registry, not
from a hard-coded default.

**FR-021 - Completed sub-agent tasks remain inspectable.**
The Agents panel shall retain a bounded history of recently completed sub-agent tasks
in the window, with their final status, duration and `output_file`, separate from the
live section.

**FR-022 - Every panel action is reachable from the keyboard.**
The Agents panel shall be openable, and every row action shall be invocable, from the
keyboard alone; no panel function shall require a mouse.

**FR-023 - `/cancel` delegates to the task registry.**
The `/cancel` command shall request cancellation through
`AgentManager::cancel_agent` and shall report the registry's accepted outcome; it
shall not remove a row before the registry confirms the request.

**FR-024 - Ambiguous prefixes are refused.**
`/cancel <prefix>` shall refuse a prefix that matches more than one live task and
shall list the matching ids so the user can disambiguate.

**FR-025 - The system prompt teaches the whole contract.**
The sub-agent guidance block in the system prompt shall describe all three execution
contracts, the limit and its occupancy, the await contract, and the durable report
path, using the same vocabulary as the tool descriptions.

### Event-driven requirements

**FR-026 - When a task is killed, the panel updates from the event.**
When `AgentManager` publishes `SubagentKilled` for a task, the Agents panel shall
remove or re-status that task's row from the event, within the next paint, without
waiting for the periodic registry reconciliation.

**FR-027 - When a cancel request is accepted, the row shows a pending state.**
When `cancel_agent` or `/cancel` has requested cancellation of a task that the
registry still reports as `running`, the Agents panel shall paint the row as
cancellation-requested until the terminal status arrives.

**FR-028 - When a task completes, its history row is recorded before the live row is
removed.** When `SubagentComplete` or `SubagentCancelled` is received, the panel
shall record the finished row in the completed history before removing the live row,
so an already-painted task is never lost from both sections.

**FR-029 - When a spawn is refused for the limit, the report names the headroom.**
When `new_agent` refuses a spawn because the background limit is reached, the error
shall name the limit, the current occupancy and the tasks that occupy it.

### State-driven requirements

**FR-030 - While a task is suspended, the limit continues to count it.**
While a background sub-agent task is in `suspended` status, the background limit
shall continue to count it as occupancy, because a suspended task still holds its
wall-clock budget and can resume at any time.

**FR-031 - While a task is terminating, the panel shows a terminating row.**
While a sub-agent task is in `terminating` status, the Agents panel shall paint it
with a distinct terminating state and exclude it from the live task count on the
Agents button.

**FR-032 - While no durable copy exists, the row says so.**
While a completed task has no `output_file`, its history row and its
`list_agents` detail view shall state that the report is in-memory only.

**FR-033 - While a foreground task runs, its exclusivity is visible.**
While a foreground sub-agent task is running, `list_agents` and the Agents panel
shall mark it as blocking the caller, so its presence explains why no other tool call
is progressing.

### Optional-feature requirements

**FR-034 - Optional per-call time bound.**
Where `new_agent` is given a per-call time bound, it shall apply that bound to the
spawned task and report the bound in the task's detail view; where no bound is given,
the configured stall timeout shall apply unchanged.

**FR-035 - Optional status filter composes with the limit report.**
Where `list_agents` is given a `status` filter, the limit and headroom figures in its
result shall describe the whole session, not the filtered subset.

**FR-036 - Optional completed history in the panel.**
Where the completed-history section is enabled, the Agents panel shall show at most a
configured number of most-recently completed tasks, newest first, and shall evict the
oldest when the cap is exceeded.

**FR-037 - Optional `bg` session scoping.**
Where the `bg` tool is called with a `session_id` naming the calling session, its
`list` and `cleanup` actions shall behave exactly as when the parameter is omitted.

### Unwanted-behaviour requirements

**FR-038 - If a parameter is unknown, the call fails.**
If a sub-agent tool call carries a property the tool's schema does not declare, then
the tool shall fail the call with an error naming the unknown key, rather than
ignoring it.

**FR-039 - If a task belongs to another session, the call is refused.**
If `cancel_agent` or `wait_agents` is asked to act on a task whose
`parent_session_id` differs from the calling session, then the tool shall refuse the
call and name both sessions.

**FR-040 - If a cancel request cannot be delivered, the tool does not report success.**
If `AgentManager::cancel_agent` returns an error because the cancel flag no longer
exists, then the tool shall report a failure that names the task's actual status,
rather than a cancellation.

**FR-041 - If a prefix matches no task, the message names the live ids.**
If `/cancel <prefix>` matches no live task, then the command shall report the failure
and list the live task ids, rather than only echoing the prefix.

**FR-042 - If a panel is opened with nothing to show, the open is refused and named.**
If the Agents window is opened while no sub-agent and no shell task is tracked, or
the Teams window is opened while no team is active, then the open shall be a no-op
that leaves the window closed and reports why on the status line, rather than
opening a modal with no subject to paint.

**FR-043 - If a report cannot be persisted, no tool claims it was.**
If `persist_task_output` fails for a completed task, then `list_agents` and
`wait_agents` shall not present an `output_file` path, and shall report the
in-memory-only condition (FR-018, FR-032).

**FR-044 - If a caller asks to await a detached task, the refusal names the contract.**
If `wait_agents` is given the id of a detached task, then it shall refuse with an
error that names the task as detached and points to the report file, rather than
reporting the id as unknown.

**FR-045 - If `new_agent` is called inside a team, the refusal is an error.**
If `new_agent` is called from a lead or teammate session that has an active team
context, then it shall return a tool error naming the team and the supported team
tools, rather than a successful output containing guidance prose.

**FR-046 - Every panel toggles from the keyboard, open or shut.**
The Agents window and the Teams window shall each be openable **and** closable from
a dedicated `Alt` chord (`Alt+A` and `Alt+T` respectively) with no mouse
interaction, and that chord shall close the window whenever it is open regardless
of whether its subject still exists, so a window that empties while it is being
viewed can never trap a keyboard-only operator. Opening a window with no subject
remains a refusal that names the reason (FR-042); a refusal is the only outcome the
chord withholds.

## Non-Functional Requirements

**NFR-001 - No new dependency.** Every requirement shall be met with the crates
already in the workspace.

**NFR-002 - No behaviour change for a call shape that already works for the right
reason.** Making FR-003 an error changes the behaviour of exactly one call shape
(`detached: true` without `background`); every other currently-working call shall
behave identically.

**NFR-003 - Cost of the limit report is bounded.** The FR-011 limit report shall be
computed from the in-memory registry without a new storage read.

**NFR-004 - Panel history is bounded.** The FR-021 completed history shall be capped
by a configuration value with a default that keeps the panel's memory cost constant
in the number of tasks ever spawned.

**NFR-005 - Every new error message is actionable.** An error added by this
specification shall name the condition, the offending value and at least one
supported next action.

## Traceability Summary

| Theme | Requirements |
|-------|--------------|
| A - execution contracts | FR-001, FR-002, FR-003, FR-004, FR-005, FR-006, FR-017, FR-034, FR-038 |
| B - cancellation vocabulary | FR-007, FR-013, FR-014, FR-015, FR-026, FR-027, FR-040, FR-044 |
| C - limit and refusals | FR-009, FR-010, FR-011, FR-029, FR-030, FR-045 |
| D - anti-patterns | FR-008, FR-016, FR-018, FR-043 |
| E - panel and slash UI | FR-019, FR-020, FR-021, FR-022, FR-023, FR-024, FR-031, FR-033, FR-036, FR-037, FR-041, FR-042, FR-046 |
| F - observability | FR-011, FR-012, FR-029, FR-031, FR-033 |
| G - documentation | FR-006, FR-016, FR-025 |
| H - test coverage | FR-002, FR-003, FR-004, FR-013, FR-014, FR-023, FR-024 (each closed by a named test in PLAN.md) |
