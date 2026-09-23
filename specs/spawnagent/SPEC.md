---
status: draft
audit:
  - { time: 1790153000, from: "none", to: "draft", actor: "system" }
---
# Specification: `/spawn` — Detached Fire-and-Forget Background Sub-Agent

## Executive Summary

This specification defines the **`/spawn`** TUI slash command and the
underlying **`new_agent` tool `detached` parameter**: a way for a user to
launch a named agent as a background sub-agent directly from the message
input (`/spawn general Some prompt`) that **nothing ever waits on**. A
detached sub-agent:

- runs concurrently as a background task, publishing the usual
  `SubagentStart` / `SubagentComplete` events so it is visible in the Agents
  panel and log panel;
- is **excluded** from the delegation surface: `list_agents` does not show
  it, `wait_agents` (with or without `task_ids`) does not and cannot wait
  for or collect its result, and the processor's `drain_completed` reaps it
  without injecting anything into the parent session's message stream;
- is unrelated to the team runtime, so team waits (`team_wait`) never see it
  either;
- can be cancelled through the existing `/cancel <task-id-prefix>` flow.

The capability exists for workflows where the caller wants only the
sub-agent's **side effects** (e.g. "generate a report into
`docs/reports/…`" or "refresh the code index") and never wants the reply
body.

## Background & Definitions

Pre-existing behaviour (before this spec):

- `new_agent` exposes `agent`, `task`, `background`, `model`. With
  `background: true` the task is registered on `AgentManager` under the
  current session, appears in `list_agents`, is waited on by `wait_agents`,
  and its completion is injected back into the parent conversation by
  `drain_completed`. Team waits (`team_wait`) operate on the separate
  `TeamManager` and are unaffected.
- There is no user-facing slash command that launches a sub-agent directly;
  the user must talk the primary agent into calling `new_agent` on their
  behalf.

### Definitions

- **Detached task** — a background sub-agent whose `TaskEntry.detached` flag
  is true. It runs on the same `AgentManager` but is invisible to the
  delegation wait/list/announce surface.
- **Delegation pipeline** — `new_agent` → `spawn_background` →
  `list_agents` / `wait_agents` / `drain_completed` → result injection into
  the caller's message stream.
- **Fire-and-forget** — the detached subset: started, tracked for display,
  never awaited, result discarded after completion marker.

## Scope & Objectives

### In scope

- `TaskEntry.detached: bool` (default false; `#[serde(default)]` so legacy
  stored entries decode).
- `AgentManager::spawn_detached(...)` — same signature as
  `spawn_background`, sets `detached = true`. Both are thin wrappers over a
  shared `spawn_background_mode` implementation.
- Exclusion of detached tasks from `AgentManager::list_agents` and
  `running_background_count`.
- `AgentManager::drain_completed`: a detached task's completion is reaped
  but never injected into the parent session's message stream.
- `AgentManager::get_task`, `cancel_agent`, `kill_task`,
  `tasks_snapshot`, `cancel_all`, `Drop`, and the P-11
  `has_pending_background` bookkeeping are intentionally **untouched** —
  detached tasks remain cancel-able, observable via the Agents panel /
  event log, and still reap on completion.
- `new_agent` tool gains an optional `detached: bool` parameter, permitted
  only as `background: true, detached: true` (the parameter description
  documents it is meaningless in foreground mode); when set the tool calls
  `spawn_detached` and annotates the response metadata with
  `detached: true`.
- TUI **`/spawn <agent> <prompt...>`** slash command plus
  **`/spawn help`**, wired into the slash-command ladder and the
  `SLASH_COMMANDS` autocomplete table; resolution of `<agent>` against
  built-ins plus loaded custom agents (identical to the `/prompt <name>`
  rule); user-visible warnings for unknown agents, usage errors, and the
  "another /spawn is still launching" race guard.
- The async launch lands in a shared slot (`App::spawn_result`), surfaced by
  a `poll_spawn_result` pass on the event loop so the wrapper's
  `[wait]` status convention is honoured.
- Tests: `cargo test -p ragent-tui --test test_slash_commands spawn` covering
  help, usage errors, unknown-agent rejection, and the launch path;
  `cargo test -p ragent-agent` covering `spawn_background` (unmodified),
  exclusion of detached tasks from `list`/`wait`, and
  `new_agent.detached` forwarding.

### Out of scope

- A CLI (`ragent run` / `ragent serve`) equivalent for `/spawn` — the
  capability is TUI-only for now; the underlying `new_agent.detached`
  parameter is available to any caller that drives the tool surface.
- Team-member spawning changes (the team runtime is untouched).
- Waiting primitives for detached tasks. That is the point: there are none.
  A follow-up spec could add an opt-in read-only inspection endpoint.

## Functional Requirements

- **FR-001** — `TaskEntry` carries a `detached: bool` flag which defaults to
  `false` so previously serialised entries decode unchanged.
- **FR-002** — `AgentManager::spawn_detached(...)` runs a background sub-
  agent identical to `spawn_background` but records the entry with
  `detached = true`.
- **FR-003** — `AgentManager::list_agents` and `running_background_count`
  omit detached tasks.
- **FR-004** — `AgentManager::drain_completed` marks detached completions as
  reported and drops them (no injection into `chat_messages`).
- **FR-004a** — On successful completion (detached or not) the task layer
  persists the FULL untruncated output to `log/subagents/<task-id>.md` under
  the parent session's working directory and records the path in
  `TaskEntry::output_file`. Before this fix `output_file` was always `None`
  and no per-task report file existed — the `wait_agents`/`list_agents`
  "recover from the on-disk report" path was a lie. Temp-file + rename so a
  crash mid-write never leaves a partial report; I/O failure leaves
  `output_file: None` and the in-memory `result` as the only copy.
- **FR-004b** — `SubagentComplete.finish_reason` reflects the real loop
  outcome: the session loop records the terminal `MessageEnd` reason on
  `SessionProcessor::last_message_end_reason` and the task registry maps it
  onto the event (`"truncation"` for a provider-side silent end-of-stream,
  `"length"`, `"cancelled"`, `"stop"`). The TUI Agents panel maps
  `"truncation"`/`"length"` to the TRUNCATED marker. Before this fix every
  successful completion was hard-coded `"stop"`, so a cut-off `/spawn` run
  appeared healthy in the log.
- **FR-004c** — The Subagent mode system prompt includes a *Deliverable
  Enforcement* section: when the task prompt asks for a file to be written,
  the sub-agent MUST call `write`/`create` for that file BEFORE
  `agent_complete`, verify it exists afterwards, and either include the
  findings inline in the `summary` or (on write failure) say so explicitly
  instead of claiming success.
- **FR-005** — `wait_agents` cannot observe or collect a detached task, via
  both the explicit-`task_ids` form (the ID is not resolved against the
  registry for a detached entry) and the omit-`task_ids` form (detached
  tasks are not part of the default wait set). No error is raised for a
  stale detached ID; it is simply invisible.
- **FR-006** — `new_agent` accepts an optional `detached: bool` parameter
  (only meaningful with `background: true`); when set the tool delegates to
  `AgentManager::spawn_detached` and annotates response metadata with
  `detached: true`.
- **FR-007** — The TUI exposes `/spawn <agent> <prompt...>` which launches
  the named agent as a detached background task under the current session,
  reusing the current session's model selection unless the user overrides it
  (via a future flag — v1 does not add flags).
- **FR-008** — `/spawn <agent> <prompt...>` validates `<agent>` against the
  built-in roster and the loaded custom agents, rejecting unknown names with
  a warning that lists resolvable alternatives.
- **FR-009** — `/spawn` honours the slash-command queue: mid-turn invocations
  are enqueued by the existing
  `InputAction::SlashCommand` → `is_input_blocked` path unchanged.
- **FR-010** — The launch is registered in `App::spawn_result` (slot,
  `Mutex<Option<Result<String, String>>>`); the event-loop
  `poll_spawn_result` pass emits the outcome into the chat/status. While a
  launch is pending a second `/spawn` is refused with a `spawn: another
  /spawn is still launching — wait for it` warning.
- **FR-011** — Cancellation: `/cancel <id-prefix>` (existing) matches a
  detached task and cancels it through `AgentManager::cancel_agent`; the
  Agents panel reflects the stop.

## Non-Functional Requirements

- **NFR-001** — No new external dependencies (workspace crates only).
- **NFR-002** — The `spawn_background` code path is shared
  (`spawn_background_mode`); `spawn_detached` adds zero new error cases and
  inherits the concurrency cap, panic handling, and event publication.
- **NFR-003** — Detached tasks are still reaped: the completion event writes
  the `reported` marker and the entry is removed from the task map during
  `drain_completed`, so the DashMap does not leak them.
- **NFR-004** — The `/spawn` slash-command path follows the existing
  `[wait]` status deferral pattern so the wrapper's `Finished` log timing is
  unchanged.

## Acceptance Criteria

1. Building the workspace succeeds: `cargo check --workspace --all-targets`.
2. `cargo test -p ragent-agent` passes, including a regression that a
   `spawn_background` (non-detached) task is listed, awaited and injected
   exactly as before.
3. `cargo test -p ragent-agent` passes a test where a `spawn_detached` task:
   (a) does not appear in `list_agents`, (b) cannot be collected by
   `wait_agents` with or without `task_ids`, (c) does not inject completion
   text into the parent session, and (d) writes its full output to
   `log/subagents/<task-id>.md` so the file is the durable recovery path.
4. `cargo test -p ragent-tui --test test_slash_commands spawn` covers
   `/spawn help` (both bare and `help`), `/spawn <agent>` without a prompt
   (usage error), an unknown agent (warning + no pending slot), and the
   `/spawn general ...` happy path (slot registered, status transitions).
5. `cargo fmt --all -- --check` is clean; `cargo clippy -p ragent-agent
   -p ragent-tui --all-targets` introduces no NEW warnings beyond the
   pre-existing `pub(crate) function inside private module` sites.
6. Manual: from the TUI with a real provider, `/spawn general Summarise the
   README for me` runs to completion, appears under the Agents panel while
   running, never surfaces in `list_agents` / `wait_agents`, and is cleaned
   up at the next idle loop iteration.

## Dependencies

- None beyond existing workspace crates.

## Glossary

- **Detached** — the `TaskEntry.detached == true` state.
- **Fire-and-forget** — the user-facing behavior of detached tasks: started,
  observed in panels, never awaited, result dropped.

## References

- `crates/ragent-agent/src/task/mod.rs` — `TaskEntry`, `AgentManager`,
  `spawn_background`, `drain_completed`.
- `crates/ragent-agent/src/tool/new_agent.rs` — tool surface and parameter
  schema.
- `crates/ragent-tui/src/app/slash.rs` — slash-command ladder; `swarm` +
  `bench` patterns for async launches.
- `crates/ragent-tui/src/app/spawn.rs` — `/spawn` handler and
  `poll_spawn_result`.
- `crates/ragent-tui/src/app/prompt.rs` — `resolve_agent` roster resolution
  reused for agent-name validation.
