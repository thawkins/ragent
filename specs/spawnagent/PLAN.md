---
spec_id: spawnagent
status: draft
---

# Implementation Plan: `/spawn` — Detached Fire-and-Forget Background Sub-Agent

## Overview

Implementation in three layers, shipped in one change-set:

1. **`ragent-agent` task registry** — add `detached: bool` to `TaskEntry`,
   add `AgentManager::spawn_detached`, exclude detached entries from
   `list_agents` / `wait_agents` / `drain_completed` injection.
2. **`ragent-agent` tool surface** — add optional `detached: bool` to
   `new_agent`; when `background + detached` are both true the tool routes to
   `spawn_detached`.
3. **`ragent-tui`** — `/spawn <agent> <prompt...>` slash command, roster
   validation via `app::prompt::resolve_agent`, async launch deposited into
   `App::spawn_result` and surfaced by `poll_spawn_result` on the event loop.

## Task List

| ID    | Status      | Description |
|-------|-------------|-------------|
| T-001 | completed   | `TaskEntry.detached: bool` (serde default) plus updates to all `TaskEntry` struct-literal construction sites (ragent-agent src+tests, bench, event_handler, TUI tests/benches). |
| T-002 | completed   | `AgentManager::spawn_detached` + shared `spawn_background_mode` implementation; `spawn_background` becomes a thin wrapper. |
| T-003 | completed   | Exclude detached entries from `AgentManager::list_agents` and `running_background_count`. |
| T-004 | completed   | `drain_completed` marks detached completions reported and DROPS them (no `chat_messages` injection) while still reaping the entry. |
| T-005 | completed   | `new_agent` tool: new optional `detached: bool` parameter; `background + detached` routes to `spawn_detached`; metadata annotated. |
| T-006 | completed   | TUI `/spawn` command: parse args, help, unknown-agent warning, queue-guard via `is_input_blocked`, launch via `spawn_detached` on tokio or a helper thread. |
| T-007 | completed   | Result plumbing: `App::spawn_result` slot, `poll_spawn_result` in the event loop, `poll_spawn_result_for_tests` test hook. |
| T-008 | completed   | `SLASH_COMMANDS` entry in `state.rs` for `/spawn`. |
| T-009 | completed   | Tests: 5 cases in `crates/ragent-tui/tests/test_slash_commands.rs` (help, bare, no-prompt, unknown-agent, launch); ragent-agent suite still green with the shared `spawn_background_mode`. |
| T-010 | in_progress | Spec files + howto doc + README bullet. |
| T-011 | pending     | `cargo fmt`, `cargo clippy` (no new warnings), full `cargo test -p ragent-tui` + `-p ragent-agent`. |
| T-012 | completed   | FR-004a/FR-004b gap fix: `log/subagents/<task-id>.md` written on every success (with `TaskEntry::output_file` set at last), real `finish_reason` plumbed via `SessionProcessor::last_message_end_reason`, TUI truncation markers accept the real labels. |
| T-013 | completed   | FR-004c: Subagent system prompt gains the "Deliverable Enforcement (FILE-WRITE TASKS)" block. |
| T-014 | completed   | Regression tests: `crates/ragent-agent/tests/test_spawn_detached.rs` (persist writes the report / failure-safe, `last_message_end_reason` round trip, detached hidden from list/wait, reaped-but-snapshotted). |

## Verification

- `cargo check --workspace --all-targets` clean.
- `cargo test -p ragent-agent` — all tests pass.
- `cargo test -p ragent-tui --test test_slash_commands` — all 190 tests pass
  (5 new).
- Manual acceptance: `/spawn general <prompt>` from the TUI with a real
  provider produces a visible running task, completes silently, and is
  absent from `list_agents` output.

## Notes

- Detached tasks still honour the Agents panel event flow
  (`SubagentStart`/`SubagentComplete` publish; `active_tasks` reconcile
  pass), so the user sees them; only the *delegation* APIs ignore them.
- The `new_agent.detached` parameter is the model-facing surface for the
  same feature, so a future `general` agent can itself decide to spawn a
  fire-and-forget sub-agent.
- Cron (`crates/ragent-tui/src/app/cron.rs`) currently uses
  `spawn_background` against a synthetic parent session; it is unaffected.
