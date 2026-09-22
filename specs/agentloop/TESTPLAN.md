---
status: draft
spec_id: agentloop
---
# Agentic Loop Mechanism (`/loop`) — Manual Test Plan

This manual test plan validates the `agentloop` specification
(`specs/agentloop/SPEC.md`): the `/loop` slash command (agent selection +
goal setting), the plan-act-observe iteration with feedback, the four stop
conditions (goal achieved, unrecoverable error, budget exhausted, human
intervention), verification gating, scope/constraint restrictions,
destructive-action checkpoints, and snapshot/rollback. Each test case is
executed by a human tester against a working ragent build.

## Prerequisites

- A debug build of ragent: `cargo build`.
- A configured LLM provider (any) with a working API key. Ollama (local) is
  acceptable and avoids provider spend.
- A scratch working directory (e.g. `target/temp/agentloop-tests/`) so the
  real project is never modified. Create it fresh for each destructive test:

  ```bash
  mkdir -p target/temp/agentloop-tests
  cd target/temp/agentloop-tests
  git init
  echo "fn main() {}" > src.rs
  printf 'assert 1 + 1 == 2\n' > check.py   # verification helper
  ```

- A local `ragent.json` in that directory for loop defaults (edit per test
  case as instructed), then restart ragent so the config is loaded:

  ```json
  {
    "agent": {
      "max_steps": 25,
      "error_retry_allowance": 3
    },
    "loop": {
      "cost_limit": 200000,
      "checkpoints": true
    }
  }
  ```

- The ragent TUI launched in that directory: `ragent` (or
  `ragent --agent coder`).
- A second terminal in the same directory for file-system verification
  (`ls`, `git status`, `git diff --stat`).

## Test Cases

### TC-001 — `/loop` registered and opens the setup dialog

**Requirement:** FR-001, FR-002

**Preconditions:**
- ragent is running in the TUI in the scratch directory.

**Steps:**
1. Press `/` and type `lo` — observe the autocomplete list.
2. Select `loop` (arrow keys + Enter) and press Enter.
3. Observe the dialog that opens.

**Test data:** none.

**Expected results:**
- `/loop` appears in autocomplete and in `/help`.
- The setup dialog opens with: agent selector, goal field, verification
  command field, scope field, constraints field, tool-set field, step-limit
  field (default 25), cost-limit field (from config), checkpoints toggle
  (default on).

---

### TC-002 — Dialog navigation and cancellation

**Requirement:** FR-002, FR-004

**Preconditions:**
- The setup dialog is open (from TC-001 step 3).

**Steps:**
1. Press Down/Up to move through the agent selector; confirm an agent with
   Enter (e.g. `coder`).
2. Tab to the goal field and type: `Make check.py pass by fixing src.rs.`
3. Tab to the step-limit field and change the value to `10`.
4. Toggle the checkpoints field off, then on again.
5. Press `Esc`.
6. Type `/loop` and press Enter again.

**Test data:** goal `Make check.py pass by fixing src.rs.`; step limit 10.

**Expected results:**
- All fields are navigable and editable with the stated keys.
- After `Esc` the dialog closes and **no** loop starts (no steps appear).
- Re-opening `/loop` restores the previously entered goal (`Make check.py
  pass by fixing src.rs.`) and step limit (10) — values were preserved.

---

### TC-003 — One-shot form starts with documented defaults

**Requirement:** FR-003

**Preconditions:**
- ragent is running in the scratch directory with default config.

**Steps:**
1. Type `/loop coder List the files in this directory and stop.` and press
   Enter.
2. Observe the step log while the run progresses.

**Test data:** `/loop coder List the files in this directory and stop.`

**Expected results:**
- The loop starts immediately (no dialog) with agent `coder`.
- Defaults are active: step limit 25, config cost limit, checkpoints on
  (a destructive action would prompt), no verification command.
- The loop completes normally (no-tool-call completion).

---

### TC-004 — Empty goal rejected

**Requirement:** FR-005

**Preconditions:**
- ragent is running in the TUI.

**Steps:**
1. Open the dialog with `/loop`, leave the goal field empty, confirm the
   dialog.
2. Note the result.
3. Type `/loop coder    ` (agent but blank goal) and press Enter.

**Test data:** empty goal in both forms.

**Expected results:**
- The dialog refuses to start and shows an error naming the missing field
  (`goal`), returning to the dialog for correction.
- The one-shot form prints usage help instead of starting a loop.
- No iteration, LLM request, or tool call occurs in either case.

---

### TC-002b — Structured goal is composed into the prompt

**Requirement:** FR-006

**Preconditions:**
- ragent is running with debug logging (`--log-level debug`).

**Steps:**
1. Start a loop with: goal `Fix src.rs so check.py passes.`, verification
   command `python3 check.py`, scope `src.rs`, constraints
   `check.py (read-only)`.
2. Inspect the system/user prompt content in the debug log.

**Test data:** as above.

**Expected results:**
- The composed prompt contains the success state, the verification command,
  the scope boundary (`src.rs`), the read-only constraint on `check.py`, and
  the active step/cost limits — all present before the first iteration.

---

### TC-005 — Stop condition 1: goal achieved completes the loop

**Requirement:** FR-001, FR-002, FR-010, FR-023

**Preconditions:**
- Scratch directory contains `note.txt` with content `alpha`.

**Steps:**
1. Start the loop: goal `Read note.txt and report its content, then stop.`
   (via dialog or one-shot).
2. Observe the step log until the run ends.

**Test data:** goal `Read note.txt and report its content, then stop.`;
`note.txt` = `alpha`.

**Expected results:**
- One step-numbered tool call (`read note.txt`) then an assistant message
  containing `alpha`.
- No iteration after the final message; the run ends with a
  `completed` indication and the iteration count (1) shown.
- All observations appear in the log; none are dropped.

---

### TC-006 — Feedback self-correction between iterations

**Requirement:** FR-006, FR-012

**Preconditions:**
- Scratch directory contains `fallback.txt` = `plan-b` and no
  `missing-target.txt`.

**Steps:**
1. Start the loop: goal `Read missing-target.txt. If that fails, read
   fallback.txt instead and report its content.`
2. Observe each iteration in the step log.

**Test data:** as above.

**Expected results:**
- The first iteration's read fails and the error is visible as an
  observation.
- The next iteration reads `fallback.txt` — the model corrected course from
  the fed-back error.
- The loop completes with `plan-b` reported.

---

### TC-007 — Stop condition 3a: step limit stops the loop before the next request

**Requirement:** FR-013, FR-017

**Preconditions:**
- The scratch `ragent.json` sets `agent.max_steps: 2`; ragent restarted.

**Steps:**
1. Start the loop: goal `Create a.txt with 1, then b.txt with 2, then c.txt
   with 3, then d.txt with 4, then list all files you created.`
2. Watch the step counter.
3. After the stop, list the directory contents in the second terminal.

**Test data:** `max_steps: 2`; the multi-file goal above.

**Expected results:**
- The loop stops after the 2nd iteration with status `budget_exhausted` and
  the iteration count (2) shown.
- No third LLM request is issued.
- At most two files exist.

---

### TC-008 — Stop condition 3b: cost limit stops the loop

**Requirement:** FR-014, FR-017

**Preconditions:**
- The scratch `ragent.json` sets `loop.cost_limit: 1500`; ragent restarted.

**Steps:**
1. Start the loop: goal `Write a detailed 1000-word essay about the history
   of operating systems, then summarise it in 100 words.`
2. Observe the run until it stops.

**Test data:** `loop.cost_limit: 1500`; the essay goal.

**Expected results:**
- The loop terminates with `budget_exhausted` once the token tally reaches
  the limit.
- No additional LLM request follows the breach.
- The termination indication names the budget/cause and partial output is
  preserved.

---

### TC-009 — Stop condition 2: unrecoverable error stops the loop, no retry

**Requirement:** FR-011, FR-017

**Preconditions:**
- ragent is launched with an invalid provider key
  (e.g. `ANTHROPIC_API_KEY=sk-invalid`) or a dead endpoint
  (`http://127.0.0.1:9`); budgets generous.

**Steps:**
1. Restart ragent with the broken auth in the scratch directory.
2. Start the loop: goal `List the files in this directory.`
3. Observe the outcome.

**Test data:** invalid key; goal `List the files in this directory.`

**Expected results:**
- The loop terminates with status `error`; the failure reason (auth/
  transport) is surfaced.
- Exactly one failed attempt — no silent retry, and no tool executes after
  the failure.

---

### TC-010 — Recoverable errors loop with feedback and stop at the retry allowance

**Requirement:** FR-012, FR-023

**Preconditions:**
- The scratch `ragent.json` sets `agent.error_retry_allowance: 2`; ragent
  restarted. `bash` permitted (no prompts).

**Steps:**
1. Start the loop: goal `Run the bash command 'exit 7' three times in a
   row, using the same command each time, and report each result.`
2. Observe each iteration.

**Test data:** `error_retry_allowance: 2`; the repeated-failure goal.

**Expected results:**
- Each failing invocation appears as an observation (error text visible).
- After consecutive failures exceed the allowance (2), the loop terminates
  with status `error` naming repeated recoverable failures — no runaway.

---

### TC-011 — Verification gate: pass completes, fail continues, fail-without-stops exhausts

**Requirement:** FR-007, FR-010, FR-013

**Preconditions:**
- Scratch directory: `src.rs` with `fn main() { unknown_fn(); }` (does not
  compile), `check.py` = `import subprocess,sys; sys.exit(0 if
  subprocess.run(['rustc','--edition','2021','--crate-type','bin','src.rs'],
  capture_output=True).returncode==0 else 1)`.
- Loop configured with verification command `python3 check.py`.

**Steps:**
1. Start the loop: goal `Fix src.rs so that the verification command
   passes.`
2. Observe iterations: the model should edit `src.rs`, and the loop should
   re-run verification only when the model first claims completion.
3. After the loop ends, run `python3 check.py` in the second terminal.

**Test data:** as above.

**Expected results:**
- The loop does NOT accept the model's first completion claim until the
  verification command has run.
- If verification failed with steps remaining, the failure output appears as
  an observation and the loop continues.
- On eventual pass the loop terminates `completed` with the verification
  outcome shown; `python3 check.py` exits 0 afterwards.

---

### TC-012 — Read-only constraints prevent test-file tampering (anti-cheat)

**Requirement:** FR-021, FR-009

**Preconditions:**
- Scratch directory: a deliberately broken `src.rs` and a passing
  `check.py` (verification helper).
- Loop constraints set `check.py` as read-only; scope set to `src.rs`,
  `check.py`.

**Steps:**
1. Start the loop: goal `Make check.py pass.`
2. Observe whether any write to `check.py` is attempted.
3. After termination, inspect `check.py` in the second terminal
   (`git diff -- check.py`).

**Test data:** broken `src.rs`; `check.py` read-only constraint.

**Expected results:**
- If the model attempts to modify `check.py`, the write is denied and the
  denial observation (naming the constraint) is visible in the log.
- `check.py` is unchanged at loop end (`git diff` shows nothing for it).
- The loop either fixes `src.rs` (verification passes) or terminates without
  tampering with the protected file.

---

### TC-013 — Scope boundaries and out-of-scope tool denial

**Requirement:** FR-009, FR-022

**Preconditions:**
- Loop scope set to `note.txt` only; tool set set to `read`, `bash`.
- Scratch directory contains `note.txt` and an `outside/` subdirectory with
  `secret.txt`.

**Steps:**
1. Start the loop: goal `Read note.txt, then read outside/secret.txt, then
   write summary.txt with both contents.`
2. Observe each tool attempt in the log.

**Test data:** scope `note.txt`; tool set `read`, `bash`.

**Expected results:**
- Reading `note.txt` succeeds.
- Reading `outside/secret.txt` is denied with a scope-violation observation.
- Any `write` attempt is denied with an out-of-scope-tool observation (write
  is not in the loop tool set).
- All denials reach the model as observations; the loop does not crash and
  eventually terminates normally.

---

### TC-014 — Destructive-action checkpoints (stop condition 4: approval)

**Requirement:** FR-015, FR-024

**Preconditions:**
- Scratch `ragent.json` sets `file:write` action `allow` for the directory;
  `loop.checkpoints: true`; ragent restarted.

**Steps:**
1. Start the loop: goal `Delete scratch_old.txt if it exists, then create
   result.txt containing hello.`
   (`touch scratch_old.txt` beforehand in the second terminal.)
2. When the permission prompt appears for the deletion, note the countdown,
   then approve.
3. Run the loop a second time with the same goal; this time **deny** the
   deletion prompt.
4. Run a third time; let the prompt time out without pressing anything.

**Test data:** goal above; `scratch_old.txt` present; checkpoints on.

**Expected results:**
- The destructive call is prompted even though an allow rule exists
  (checkpoint escalation).
- Approve path: deletion executes, `result.txt` is created, loop completes.
- Deny path: no deletion; the denial is recorded as an observation and the
  model receives it; `scratch_old.txt` survives.
- Timeout path: after the 120-second countdown the prompt shows EXPIRED and
  the outcome matches the deny path (safe default).

---

### TC-015 — Human interrupt mid-loop (stop condition 4: interrupt)

**Requirement:** FR-016

**Preconditions:**
- ragent running with `bash` permitted (no prompts).

**Steps:**
1. Start the loop: goal `Run 'sleep 30' then report the current date.`
2. While the run is executing (during the sleep), press `Esc`.
3. After the stop, inspect the session list (in ragent: `/sessions`, or CLI
   `ragent session list` in another terminal).

**Test data:** goal above; `Esc` during execution.

**Expected results:**
- The loop aborts at a safe point — no further LLM request or tool call after
  the interrupt.
- The termination status is `interrupted` with the iteration count at the
  point of interruption.
- The session remains persisted and listed/resumable.

---

### TC-016 — Pre-loop snapshot, change summary, and rollback (test with rollback)

**Requirement:** FR-018, FR-019, FR-020

**Preconditions:**
- Scratch directory is a git repository on a clean tree
  (`git status` shows nothing to commit).
- ragent running in the scratch directory.

**Steps:**
1. Note the starting state: `git status` and `git log --oneline -1` in the
   second terminal.
2. Start the loop: goal `Modify src.rs to contain 'fn main() {
   println!("loop"); }' and create loopout.txt with 'done'.`
3. When the loop terminates, observe the change summary (banner/summary
   line).
4. Decline the rollback offer.
5. Start an identical loop again; this time accept the rollback offer at the
   end.

**Test data:** goal as above; loop modifies `src.rs` and creates
`loopout.txt`.

**Expected results:**
- Before any write, a snapshot of the workspace was captured (and the git
  branch/HEAD recorded — visible in debug log); outside git a warning and
  confirmation would have been required.
- At termination a change summary is rendered: 1 modified (`src.rs`), 1
  created (`loopout.txt`), 0 deleted — matching `git status --short`.
- Declining rollback keeps the changes (files still changed afterwards).
- Accepting rollback restores the pre-loop snapshot: `git status` shows a
  clean tree again and `loopout.txt` is gone.

---

### TC-017 — No iteration after any stop condition (unwanted-behaviour guard)

**Requirement:** FR-017, FR-023

**Preconditions:**
- ragent running with the standard scratch config (`max_steps: 25`).

**Steps:**
1. Run the TC-005 loop to natural completion; count the LLM requests in the
   step log.
2. Run TC-009 (broken provider). After the `error` termination, type
   `continue` and press Enter.
3. Observe the step numbering of the new activity.

**Test data:** as referenced.

**Expected results:**
- After completion, nothing further runs for that loop.
- After the error, sending `continue` starts a NEW loop (step 1), never a
  resumption of the terminated loop.
- No loop ever shows activity after its stop condition was met.

---

### TC-018 — Telemetry and termination publication (visibility)

**Requirement:** FR-025, FR-019

**Preconditions:**
- ragent launched with debug logging.

**Steps:**
1. Launch ragent with debug logging enabled.
2. Run a completing loop (TC-005) and note the end banner.
3. Run a step-limit loop (TC-007) and note the banner.
4. Run an interrupt loop (TC-015) and note the banner.

**Test data:** standard scratch config; referenced goals.

**Expected results:**
- Each loop's end renders the correct termination status: `completed`,
  `budget_exhausted`, or `interrupted`, with the iteration count.
- The debug log shows exactly one agent-loop metric record per loop
  (iterations + duration) matching the visible step count.
- The change summary line (when file changes occurred) lists the affected
  file counts.

---

### TC-019 — Checkpoints enforced in auto-approve mode (unwanted guard)

**Requirement:** FR-024

**Preconditions:**
- ragent launched with autopilot enabled (`/autopilot on` or `--yes`), with
  `loop.checkpoints: true` in the scratch config.

**Steps:**
1. Start the loop: goal `Delete scratch_old.txt if it exists.` (file
   present).
2. Observe the permission flow for the deletion.

**Test data:** autopilot on; checkpoints on; destructive goal.

**Expected results:**
- The deletion still triggers the checkpoint prompt (or is safely denied
  under the checkpoint timeout rule) — auto-approval does NOT silently
  bypass the destructive-action checkpoint.
- Hard-deny rules (if configured) are never overridden by auto-approval.

## Cleanup

1. Restore or remove the scratch `ragent.json`
   (`target/temp/agentloop-tests/ragent.json`) so no budget/checkpoint
   overrides leak into other sessions.
2. Delete the scratch directory contents created during testing
   (`a.txt`, `b.txt`, `result.txt`, `loopout.txt`, `scratch_old.txt`,
   generated `src.rs` variants, etc.) or remove the whole scratch directory.
3. Restore the original provider API key environment and restart any
   terminal that had an invalid key exported.
4. Close ragent; verify the real user config (`~/.config/ragent/config.json`
   or project `.ragent/ragent.json`) was never modified.
5. Confirm no stray git branches were left in the scratch repository
   (`git branch` shows only the initial branch).

---

*End of Manual Test Plan*