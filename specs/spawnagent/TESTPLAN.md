---
status: draft
spec_id: spawnagent
---

# `/spawn` (Detached Background Sub-Agent) — Manual Test Plan

This manual test plan validates the `spawnagent` specification
(`specs/spawnagent/SPEC.md`): a TUI slash command that launches a named
agent as a **detached** background sub-agent — visible in the Agents panel
and log, but never listed by `list_agents`, never awaited by `wait_agents`,
and never injected back into the parent session's message stream.

## Prerequisites

- A debug build of ragent: `cargo build`.
- A configured LLM provider (any) with a working API key. Ollama (local) is
  acceptable and avoids provider spend.
- A scratch working directory (e.g. `target/temp/spawnagent-tests/`)
  containing nothing but a small README so the spawned agent has something
  to act on:

  ```bash
  mkdir -p target/temp/spawnagent-tests
  cd target/temp/spawnagent-tests
  echo "# Scratch — /spawn test bed" > README.md
  ```

- Launch ragent from that directory and pick a cheap/fast model.

## Test Cases

### TC-SPAWN-001 — Help via `/spawn help`

**Setup:** none.
**Action:** type `/spawn help` and Enter.
**Expect:**
- The chat shows the help block with the usage table
  (`/spawn <agent> <prompt...>` and `/spawn help`).
- Status bar shows `spawn: help`.
- No background task starts.

### TC-SPAWN-002 — Bare `/spawn` shows the same help

**Action:** `/spawn` alone.
**Expect:** identical help content and `spawn: help` status (treated as a
help alias, not a usage error).

### TC-SPAWN-003 — Missing prompt is a usage error

**Action:** `/spawn general` (agent but no prompt).
**Expect:**
- Chat shows `Usage: /spawn <agent> <prompt...> — the prompt must not be empty.`
- Status bar shows `spawn: usage`.
- No task launched; the `/spawn` slot is immediately free (a follow-up
  `/spawn help` works).

### TC-SPAWN-004 — Unknown agent name rejected with roster

**Action:** `/spawn no-such-agent please work`.
**Expect:**
- Status bar: `spawn: unknown agent 'no-such-agent'`.
- Chat: `[warn] Unknown agent` plus a list of resolvable agents.
- No `SubagentStart` event fires; Agents panel stays unchanged.

### TC-SPAWN-005 — Happy path: `/spawn general <prompt>`

**Action:** `/spawn general Summarise the README.md in one sentence and write it to SCRATCH-SUMMARY.md`.
**Expect:**
1. Immediately: status `[wait] spawn: launching general …`.
2. Within a second: chat shows the "Detached sub-agent launched" message
   with the task id; Agents panel lists the running entry with agent
   `general`.
3. The background task creates/updates `SCRATCH-SUMMARY.md`.
4. On completion the Agents panel entry clears and a `[ok] Task completed`
   log line appears in the log panel.
5. **Detached contract:** the chat does NOT receive the
   `[Background Task completed: general — ...]` user-turn injection that a
   normal `new_agent(background: true)` call produces.
6. `log/subagents/<task-id>.md` exists on disk and contains the full
   report.

### TC-SPAWN-006 — `list_agents` does not see a detached task

**Setup:** a detached task from TC-005 is running or recently completed.
**Action:** prompt the primary agent: "Call `list_agents` right now and show
the table."
**Expect:** the reported table contains neither the detached task id nor an
entry whose agent is `general` spawned at TC-005 (it is filtered out).

### TC-SPAWN-007 — `wait_agents` cannot wait on a detached task

**Setup:** TC-005 completed.
**Action:** prompt the primary agent: "Call `wait_agents` with no task_ids."
**Expect:** the tool returns immediately with
`No running background tasks to wait for.` and metadata `count: 0`.
**Action (explicit):** prompt: "Call `wait_agents` with task_ids
`["<task-id>"]` using the id from the spawn output above."
**Expect:** the tool reports the id is not a background task for this
session (the entry is filtered out, not resolved).

### TC-SPAWN-008 — Cancellation via `/cancel <prefix>`

**Action:** launch `/spawn general Write a three-paragraph essay about
semaphores` then, while it is still running, `/cancel <first 6 chars of the
task id>`.
**Expect:** the Agents panel row disappears, the log shows `[bg] task
cancelled`, and no result text is injected into the chat.

### TC-SPAWN-009 — Queued while the agent is busy

**Setup:** start a long-running request in the main chat (e.g. ask the agent
to summarise several files).
**Action:** while the turn is in progress, type `/spawn general poke` and
Enter.
**Expect:** the command is queued (input-queue counter increments); after
the current turn ends, the queued `/spawn` is executed and behaves exactly
like TC-005. No command loss, no duplicate launch.

### TC-SPAWN-010 — `new_agent` tool `detached: true` surface

**Action:** prompt the primary agent: "Spawn an explore agent with
`new_agent`, background: true, detached: true, to look up where `SLASH_COMMANDS`
is defined. Do NOT wait for it."
**Expect:**
- The tool returns `Background task spawned successfully. Task ID: ...`
  and its metadata includes `detached: true`.
- The chat does NOT receive a `[Background Task completed ...]` injection
  when the explore agent finishes.
- The log still shows the `SubagentStart` / `SubagentComplete` events.

## Regression Checks

- `cargo test -p ragent-agent` — all pre-existing `new_agent` /
  `wait_agents` / `list_agents` tests still pass (the non-detached path is
  unchanged).
- `cargo test -p ragent-tui --test test_slash_commands` — all 190 tests
  pass, including the 5 new `/spawn` cases.
- The cron scheduler (`/cron list`, `ragent_cron` runs) still executes its
  background sub-agents successfully (cron does not use `spawn_detached`).

## Cleanup

```bash
cd /home/thawkins/Projects/ragent
rm -rf target/temp/spawnagent-tests
```
