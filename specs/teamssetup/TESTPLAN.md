---
status: draft
audit:
  - { time: 1793241600, from: "none", to: "draft", actor: "system" }
---
# Manual Test Plan: Team Runtime Hardening

## Purpose

This is a **manual** test plan. A human operator follows each case by hand in a
running `ragent` TUI session, reading the `Teams` popup, the message window, and
the on-disk team directory to decide whether the case passes. No case requires
writing test code.

Each case names the requirement it proves (from `SPEC.md`) and the task that
implements it (from `PLAN.md`).

## Prerequisites

Complete these once before running any case.

### P-1. Build the binary

```
cd /home/thawkins/Projects/ragent
cargo build
```

Confirm `target/debug/ragent` exists.

### P-2. Isolated sandbox directory

Every case writes into `target/temp/teamtest/` so the repository tree stays
clean.

```
mkdir -p target/temp/teamtest
cd target/temp/teamtest
```

Run `ragent` from inside `target/temp/teamtest` for every case below.

### P-3. Provider configuration

The team runtime spawns real agent loops, so at least one provider must work.

1. `export ANTHROPIC_API_KEY="<your key>"` (or `OPENAI_API_KEY`).
2. Launch `ragent`.
3. Press `Ctrl+P` to open the provider selector, choose the provider you
   configured, press `Enter`.
4. Press `Ctrl+M`, choose a small, fast model (e.g. a Haiku-class model), press
   `Enter`.

Rationale: several cases wait on a teammate's real turn, so a slow model makes
the case take minutes instead of seconds.

### P-4. A blueprint that always yields two teammates

Create `target/temp/teamtest/.ragent/blueprints/teams/manual/blueprint.json`:

```json
{
  "name": "manual",
  "description": "Manual test team - two teammates",
  "settings": {
    "max_teammates": 8,
    "require_plan_approval": false,
    "auto_claim_tasks": true
  },
  "teammates": [
    { "name": "alpha", "agent_type": "general", "prompt": "You are alpha. Run the command sleep 400 as your first action, then report done." },
    { "name": "beta",  "agent_type": "general", "prompt": "You are beta. Immediately call team_idle and do nothing else." }
  ]
}
```

The `sleep 400` in alpha's prompt is what makes several watchdog and lifecycle
cases deterministic: it holds a live member for well past the 5-minute timeout.

### P-5. A hook script that always fails

Create `target/temp/teamtest/hook-fail.sh`:

```
#!/bin/sh
echo "static analysis reported 3 issues"
exit 2
```

Run `chmod +x target/temp/teamtest/hook-fail.sh`.

### P-6. The hook-event spelling for fixture edits

`HookEvent` (`crates/ragent-agent/src/team/config.rs:212`) has **no**
`rename_all` attribute, so its wire form is the Rust variant name, not
snake_case:

| Variant | JSON spelling |
| --- | --- |
| `TeammateIdle` | `TeammateIdle` |
| `TaskCreated` | `TaskCreated` |
| `TaskCompleted` | `TaskCompleted` |

A fixture that writes `"event": "task_completed"` will fail to deserialise and
the whole `config.json` load fails, which looks like an unrelated defect. Every
`jq` hook edit in this plan therefore writes `"TaskCompleted"`.

### P-7. A hook command that fails validation

`validate_hook_command` (`crates/ragent-agent/src/team/manager.rs:414`) refuses
an empty command, a command over 4096 bytes, any control character, and any of
the substrings `; | & \` $( > < * ? ~ \`. A command containing `>` is therefore
rejected before it is spawned. The literal string `echo > out.txt` is used in
TC-012 for that reason; no file is needed, the string is written into
`config.json`.

### P-8. Invoking a team tool from the TUI

Several cases need one specific tool call rather than a natural-language
request, because the point of the case is that a *particular* code path is
exercised. `ragent` has no `/tool` slash command, so a tool is invoked by
asking the lead's own agent loop, which keeps the call deterministic if the
instruction is literal. Type, for example:

```
Call the tool team_status with {"team_name": "thook"} and report the raw output.
```

Where a case writes `call team_status(team_name: "thook")`, that is shorthand
for the instruction above; expand it to the `Call the tool <name> with {json}`
form when typing. The message window shows the tool call as a numbered step,
and its result is the `Test step` evidence the case asks you to read.

Note that the lead's tool calls are permission-gated like any other agent
action: approve them at the prompt if interactive approval is enabled, or run
the session in a mode that already allows the `team:*` categories.

### P-9. Locating the team directory

Every case inspects or edits on-disk state. The team directory is:

```
target/temp/teamtest/.ragent/teams/<team-name>/
```

containing `config.json`, `tasks.json`, `mailboxes/`, and (after M5)
`hook-failures.jsonl`.

Open a second terminal at `target/temp/teamtest` for `cat` / `jq` inspection.
Do **not** edit `config.json` while a case is running unless the case says to.

### P-10. Reading and driving the popup

Several cases read and drive the `Teams` popup. To open it: press `Alt+T` (or
click the `Teams` button beside the message input). `Up`/`Down` move the block
cursor, `PgUp`/`PgDn` page it, `Enter` focuses the row's teammate and opens
their output overlay, `Esc` closes the popup. The popup lists the lead and every
teammate with a status word, elapsed time, and step count.

**`Space` and `Del` both open a two-option Yes/No dialog; neither acts
immediately.** This matches the shipped behaviour and several cases below
depend on it:

| Key | Dialog it opens | `No` is pre-selected |
| --- | --- | --- |
| `Space` (member not `Suspended`) | suspend this teammate | yes |
| `Space` (member `Suspended`) | none - resumes immediately | n/a |
| `Del` or `K` | shut this teammate down | yes |

So the literal sequence to **suspend** a row is:

```
Space      (opens the suspend dialog, No selected)
Left       (or Right / Tab - toggles the two options)
Enter      (accepts Yes)
```

and pressing `Enter` straight after `Space` cancels, it does not suspend. The
dialog's `Esc` also cancels. Every case below therefore spells `Space`, `Left`,
`Enter` when it means "suspend", and `Del`, `Left`, `Enter` when it means
"shut down". `Space` on an already-`Suspended` member is the one case with no
dialog, which is why resume is a single keypress.

## Test Cases

### TC-001 - Teammate cannot complete another teammate's task

**Requirement:** FR-001, FR-002, FR-025, FR-042, FR-044
**Task:** T-008, T-009, T-013
**Preconditions:** P-1 to P-4 complete. No team exists yet.

**Steps:**

1. Launch `ragent` from `target/temp/teamtest`.
2. Type `/team create manual tcompletion` and press `Enter`.
3. Wait until the message window shows both `alpha` and `beta` spawned. In the
   `Teams` popup (`Alt+T`) confirm two rows with a `working` status.
4. Press `Alt+T`, `Esc` to close the popup, then in the input type:
   `create a task titled "Shared work" with description "owned by beta"`
   and press `Enter`. This is a natural-language instruction to the lead; the
   lead will call `team_task_create` with `title: "Shared work"`.
5. Confirm the message window shows a `team_task_create` step. Note the task id
   from the step's JSON output (expected `task-001`).
6. Assign it to beta: type `assign task-001 to beta using team_assign_task` and
   press `Enter`.
7. In the second terminal run
   `jq '.tasks[] | select(.id=="task-001") | {status,assigned_to}' \
   .ragent/teams/tcompletion/tasks.json`.
   Expected: `{"status": "in_progress", "assigned_to": "tm-002"}`.
8. Instruct alpha to complete beta's task: type
   `as alpha, call team_task_complete(team_name: "tcompletion", task_id: "task-001")`
   and press `Enter`.

**Test data:** team name `tcompletion`; task title `Shared work`; task id
`task-001`; owner `beta` = `tm-002`.

**Expected results:**

- The step output contains an error naming the current owner, in the shape
  "task 'task-001' is assigned to tm-002, not tm-001".
- Re-running step 7 shows `{"status": "in_progress", "assigned_to": "tm-002"}`
  unchanged - no reassignment, no completion.
- `rg 'TaskCompletionRejected' log/` in `target/temp/teamtest` finds one entry.
- `tasks.json` shows `completed_by: null` for `task-001`.

**Note:** if the lead itself completes the task, the case still passes the
ownership check but fails step 2 above only for an alpha-sourced completion.
Ensure the completion is requested *as alpha*, which the prompt in step 8 does.

---

### TC-002 - Lead can complete a task on a teammate's behalf

**Requirement:** FR-001 (lead exemption), FR-002
**Task:** T-008

**Preconditions:** TC-001 left `task-001` `in_progress` and assigned to `tm-002`.
Team `tcompletion` is still active and focused.

**Steps:**

1. In the input type
   `call team_task_complete(team_name: "tcompletion", task_id: "task-001")`
   and press `Enter`. The lead issues this call, so the caller is `lead`.
2. In the second terminal run
   `jq '.tasks[] | select(.id=="task-001")' .ragent/teams/tcompletion/tasks.json`.

**Test data:** team `tcompletion`, task `task-001`.

**Expected results:**

- The step succeeds with a `completed` result.
- `jq` shows `"status": "completed"`, `"completed_by": "lead"`, a non-null
  `completed_at`, and the `assigned_to` field **unchanged** at `tm-002`.

---

### TC-003 - Cancelled task is never claimable on any path

**Requirement:** FR-005, FR-006, FR-007, FR-025
**Task:** T-009, T-010, T-011, T-012

**Preconditions:** P-1 to P-4 complete. Team `tcancel` created from blueprint
`manual`, both teammates spawned.

**Steps:**

1. Type `/team create manual tcancel` and press `Enter`.
2. Type `create three tasks: "first", "second", "third"` and press `Enter`.
   Confirm three tasks appear with ids `task-001`, `task-002`, `task-003`.
3. Cancel the middle one via the tool: type
   `call team_task_cancel(team_name: "tcancel", task_id: "task-002", reason: "not needed")`
   and press `Enter`.
4. Verify on disk:
   `jq '.tasks[] | {id,status}' .ragent/teams/tcancel/tasks.json`.
5. Now try to claim it explicitly: type
   `call team_task_claim(team_name: "tcancel", task_id: "task-002")` and press
   `Enter`.
6. Try to claim it through the tool that claims the *next* task: type
   `call team_task_claim(team_name: "tcancel")` (no `task_id`) and press `Enter`.
7. Try to complete it: type
   `call team_task_complete(team_name: "tcancel", task_id: "task-002")` and
   press `Enter`.

**Test data:** team `tcancel`; tasks `task-001` "first", `task-002` "second",
`task-003` "third".

**Expected results:**

- Step 4 output: `task-002` has `"status": "cancelled"`; `task-001` and
  `task-003` remain `pending`.
- Step 5: refusal containing the word `cancelled` (not `pending`), i.e.
  "task 'task-002' cannot be claimed (status: cancelled)".
- Step 6: succeeds, but claims `task-001` - never `task-002`.
- Step 7: refusal; `task-002` stays `cancelled` with `completed_by: null`.

---

### TC-004 - Illegal task transitions are refused and leave the file untouched

**Requirement:** FR-003, FR-004, FR-025
**Task:** T-006, T-007

**Preconditions:** Team `tcancel` from TC-003 is active.

**Steps:**

1. In the second terminal capture the current file:
   `cp .ragent/teams/tcancel/tasks.json target-004-before.json`
   (run from `target/temp/teamtest`, writing the copy to `target/temp/`).
2. Attempt `Completed -> InProgress` by releasing the completed task from
   TC-002: type
   `call team_task_claim(team_name: "tcompletion", task_id: "task-001")` and
   press `Enter`.
3. Attempt `Cancelled -> Completed`: type
   `call team_task_complete(team_name: "tcancel", task_id: "task-002")` and
   press `Enter`.
4. Attempt `Cancelled -> InProgress` again via a specific claim:
   `call team_task_claim(team_name: "tcancel", task_id: "task-002", agent_id: "tm-001")`.

**Test data:** team `tcompletion` task `task-001` (completed); team `tcancel`
task `task-002` (cancelled).

**Expected results:**

- Every step refuses, and the refusal names all three of: the task id, its
  current status, and the requested status. Expected shape: "task 'task-002'
  cannot move from cancelled to completed".
- `diff target/temp/tcancel-before.json .ragent/teams/tcancel/tasks.json`
  (adjusted for the path you copied to) reports no differences.

---

### TC-005 - Suspended teammate survives the watchdog

**Requirement:** FR-012
**Task:** T-004, T-014

**Preconditions:** Team `tsuspend` created from blueprint `manual`. `alpha` is
running the `sleep 400` prompt, so it is `working`.

**Steps:**

1. Type `/team create manual tsuspend` and press `Enter`. Wait for both
   teammates to reach `working`/`idle`.
2. Open `Alt+T`; move the cursor with `Down` to the `alpha` row (the row whose
   status reads `working`).
3. Press `Space` to open the suspend dialog, `Left` to select `Yes`, then
   `Enter` to accept. Confirm the row's status word becomes `suspended`.
4. Record the time and leave the TUI untouched for **6 minutes**. Do not send
   any prompt; the watchdog needs the wall clock to pass.
5. After 6 minutes, reopen `Alt+T` and read alpha's row.
6. In the second terminal run
   `jq '.members[] | {name,status,last_spawn_error}' \
   .ragent/teams/tsuspend/config.json`.

**Test data:** team `tsuspend`; teammate `alpha`; wait duration 6 minutes
(the watchdog timeout is 5 minutes).

**Expected results:**

- Step 5: the row still reads `suspended`. It does **not** read `failed`.
- Step 6: `{"name":"alpha","status":"suspended","last_spawn_error":null}`.
- No `TeammateFailed` line for alpha appears in the message window during the
  wait.

---

### TC-006 - Blocked teammate can be resumed and unblocks cleanup

**Requirement:** FR-009, FR-010, FR-011, FR-022, FR-029
**Task:** T-016, T-017, T-027

**Preconditions:** Team `tblock` created from blueprint `manual`. You will
inject the `Blocked` status by hand, because the only in-product producer today
is `/swarm`.

**Steps:**

1. Type `/team create manual tblock` and press `Enter`.
2. Stop both teammates so nothing races the edit: `Alt+T`, `Down` to `beta`,
   `Space` `Left` `Enter` (suspend), `Down` to `alpha`, `Space` `Left` `Enter`
   (suspend). Confirm both rows read `suspended`.
3. In the second terminal force `beta` to `blocked`:
   `jq '(.members[] | select(.name=="beta") | .status) = "blocked"' \
   .ragent/teams/tblock/config.json > c.json && mv c.json \
   .ragent/teams/tblock/config.json`
4. Back in the TUI, run `/team cleanup` and press `Enter`.
5. Reopen `Alt+T`. Select the `beta` row and press `Del`. This arms the
   shutdown dialog; it is **not** the resume path, so record what the dialog
   offers and press `Esc` to dismiss it. This is the pre-fix behaviour.
6. Now exercise the fix: with `beta` still selected, press `Space`. Read the
   dialog's text, press `Left` to select `Yes`, then `Enter` to accept.
7. Read beta's row status.
8. Run `/team cleanup` again and press `Enter`.

**Test data:** team `tblock`; member `beta` forced to `blocked`.

**Expected results:**

- Step 4: refusal naming `beta` and its status `blocked`, and naming the next
  action (resume it, or shut it down). No files are deleted.
- Step 5: a shutdown dialog naming `beta` but **not** a resume action - the
  pre-fix popup has no way out of `blocked`, which is the defect FR-029 fixes.
  `Esc` dismisses it and `beta` stays `blocked`.
- Step 6: after the fix, `Space` on a `blocked` row offers the resume action,
  and the dialog names both the member (`beta`) and the action (resume).
  Pressing `Esc` instead of `Left` `Enter` cancels without change.
- Step 7: beta's status word reads `idle`.
- Step 8: cleanup succeeds and the message window reports the team cleaned up.

---

### TC-007 - Cleanup refuses on every live status, and the three surfaces agree

**Requirement:** FR-009, FR-022, FR-026, FR-032
**Task:** T-004, T-005, T-027

**Preconditions:** Team `tguard` active with `alpha` `working` (running
`sleep 400`) and `beta` `idle`.

**Steps:**

1. Type `/team create manual tguard` and press `Enter`. Wait for alpha to be
   `working`.
2. Run `/team cleanup` and press `Enter`. Read the refusal.
3. Run the tool directly: type
   `call team_cleanup(team_name: "tguard")` and press `Enter`. Read the refusal.
4. Run `/team delete tguard` and press `Enter`. Confirm whether a second
   confirmation is requested.
5. Now make alpha `suspended`: `Alt+T`, `Down` to `alpha`, then
   `Space` `Left` `Enter`. Confirm `suspended`.
6. Run `/team cleanup` again and press `Enter`.

**Test data:** team `tguard`; members `alpha` (working then suspended), `beta`
(idle).

**Expected results:**

- Steps 2 and 3 produce the **same** refusal text, listing `alpha` by name and
  status. The text must not differ between the slash command and the tool.
- Step 4: with `alpha` still working, `/team delete` requires a confirmation
  step; pressing `Esc` leaves the team directory on disk. Verify with
  `ls .ragent/teams/tguard` in the second terminal.
- Step 6: cleanup **succeeds**. `Suspended` is parked, not `is_live()`, so the
  single shared guard of FR-022 must let it through - which is exactly what the
  pre-fix TUI guard already did and the pre-fix tool guard did **not**, so this
  step is the one that proves the three surfaces now agree. Confirm the message
  window reports the team cleaned up and `ls .ragent/teams/tguard` reports
  "No such file or directory".

---

### TC-008 - `max_teammates` counts slots, not spawn calls

**Requirement:** FR-016
**Task:** T-021

**Preconditions:** None; create a custom blueprint.

**Steps:**

1. Create `target/temp/teamtest/.ragent/blueprints/teams/tiny/blueprint.json`:

```json
{
  "name": "tiny",
  "description": "Cap test",
  "settings": { "max_teammates": 1, "require_plan_approval": false, "auto_claim_tasks": true },
  "teammates": []
}
```

2. Type `/team create tiny tcap` and press `Enter`.
3. Spawn one teammate: type
   `call team_spawn(team_name: "tcap", teammate_name: "solo", agent_type: "general", prompt: "call team_idle immediately")`
   and press `Enter`.
4. Spawn a **different** name: type
   `call team_spawn(team_name: "tcap", teammate_name: "extra", agent_type: "general", prompt: "call team_idle immediately")`.
5. Respawn the **same**, still-active name: type
   `call team_spawn(team_name: "tcap", teammate_name: "solo", agent_type: "general", prompt: "call team_idle immediately")`.
6. Stop `solo`: type
   `call team_shutdown_teammate(team_name: "tcap", teammate: "solo")`, have `solo`
   type `call team_shutdown_ack`, then confirm with
   `jq '.members[] | select(.name=="solo") | .status' .ragent/teams/tcap/config.json`
   printing `"stopped"`.
7. Revive the now-terminal name: type
   `call team_spawn(team_name: "tcap", teammate_name: "solo", agent_type: "general", prompt: "call team_idle immediately")`.

**Test data:** team `tcap`; `max_teammates: 1`; names `solo`, `extra`.

**Expected results:**

- Step 3: succeeds; `solo` is on the roster as `tm-001`.
- Step 4: refused, naming the active count (1) and the configured maximum (1).
- Step 5: **succeeds**. `solo` is still active, so it reuses the slot it already
  holds and adds nothing to the active count. It reuses `tm-001`; no second
  member is appended.
- Step 7: **refused** with the same "already has 1 active teammates; the
  configured maximum is 1" message, and `solo` stays `stopped`. This is the
  case's point: a terminal member occupies no active slot, so reviving it adds
  one and must respect the cap. The pre-fix build allows step 7 through because
  the limit check is skipped for every existing name.
- `jq '.members | length' .ragent/teams/tcap/config.json` prints `1` throughout:
  every spawn reuses the one record rather than appending.

---

### TC-009 - `require_plan_approval` gates write and bash tools

**Requirement:** FR-014
**Task:** T-018

**Preconditions:** A blueprint with plan approval on.

**Steps:**

1. Create `target/temp/teamtest/.ragent/blueprints/teams/gated/blueprint.json`:

```json
{
  "name": "gated",
  "description": "Plan gate test",
  "settings": { "max_teammates": 4, "require_plan_approval": true, "auto_claim_tasks": true },
  "teammates": [
    { "name": "writer", "agent_type": "general", "prompt": "Create the file report.md in the working directory with the text 'hello'. Do not submit a plan." }
  ]
}
```

2. Type `/team create gated tgate` and press `Enter`.
3. Wait for the teammate `writer` to attempt its work. Watch the message window
   and the `Teams` popup.
4. In the second terminal run
   `ls target/temp/teamtest/report.md` and `cat .ragent/teams/tgate/config.json | jq '.members[0] | {name,status,plan_status}'`.
5. Now approve a plan so the gate opens. The approve step requires the member to
   be `PlanPending` with a recorded `plan_request_id`, which the writer's own
   `team_submit_plan` call sets. Ask the writer to submit one: type
   `ask writer to call team_submit_plan with a one-line plan` and press `Enter`.
   Confirm `jq '.members[0] | {status,plan_status}' .ragent/teams/tgate/config.json`
   prints `{"status": "plan_pending", "plan_status": "pending"}`. Then approve:
   `call team_approve_plan(team_name: "tgate", teammate: "writer", approved: true)`
   and press `Enter`. Confirm `plan_status` is `approved` and the status is
   `working`.
6. Ask the writer to retry: type `ask writer to create report.md now` and press
   `Enter`.

**Test data:** team `tgate`; teammate `writer`; target file `report.md` with
content `hello`.

**Expected results:**

- Step 3: the writer's write attempt is refused, and the refusal text names
  `team_submit_plan` as the way to clear the gate.
- Step 4: `report.md` does **not** exist; `plan_status` is `none` and the
  member's status is `plan_pending` or `idle`, not `working` on the write.
- Step 6: `report.md` now exists and contains `hello`.

---

### TC-010 - `auto_claim_tasks: false` holds the teammate idle

**Requirement:** FR-015, FR-027
**Task:** T-019, T-020

**Preconditions:** A blueprint with auto-claim off.

**Steps:**

1. Create `target/temp/teamtest/.ragent/blueprints/teams/manual-claim/blueprint.json`
   identical to P-4 but with `"auto_claim_tasks": false`.
2. Rename the blueprint directory to `manualclaim` and its `"name"` field to
   `manualclaim`.
3. Type `/team create manualclaim tnoauto` and press `Enter`.
4. Create two tasks: type `create two tasks "one" and "two"` and press `Enter`.
5. Instruct the beta teammate to look for work: type
   `ask beta to check for work` and press `Enter`.
6. Open `Alt+T` and read beta's row.
7. Repeat the whole case with a copy of the blueprint that has
   `"auto_claim_tasks": true`, named `manualauto`, team `tauto`, and compare.

**Test data:** team `tnoauto` (auto off), team `tauto` (auto on); tasks `one`,
`two`.

**Expected results:**

- Step 6, auto-off: beta's row reads `idle` and `jq '.tasks[] | {id,status}'`
  shows both tasks still `pending`. Nothing was claimed.
- Step 7, auto-on: after the teammate goes idle, the runtime claims the next
  task for it, so one task reads `in_progress` with `assigned_to` set to the
  teammate's id.
- In both runs, the teammate's system prompt (viewable with `/prompt` while
  focused on that teammate, or in the spawn step's JSON) states the
  corresponding rule.

---

### TC-011 - Hooks fire from the runtime path, not only from tools

**Requirement:** FR-017, FR-018
**Task:** T-023, T-024, T-025

**Preconditions:** P-5 complete. Team `thook` active.

**Steps:**

1. Type `/team create manual thook` and press `Enter`.
2. Register a `TaskCompleted` hook by editing the team config in the second
   terminal:
   `jq '.settings.hooks = [{"event":"TaskCompleted","command":"/home/thawkins/Projects/ragent/target/temp/teamtest/hook-fail.sh"}]' \
   .ragent/teams/thook/config.json > h.json && mv h.json \
   .ragent/teams/thook/config.json`
3. Back in the TUI create a task: type `create task "hook probe"` and press
   `Enter`.
4. **Complete it through the runtime, not the tool.** Suspend both teammates
   so nothing else claims, then as the lead run
   `call team_task_complete(team_name: "thook", task_id: "task-001")`.
5. Read the step's output.

**Test data:** team `thook`; hook command
`/home/thawkins/Projects/ragent/target/temp/teamtest/hook-fail.sh`; task
`task-001` "hook probe".

**Expected results:**

- Step 5: the step output contains the hook's stdout, `static analysis reported
  3 issues`, as feedback. On the pre-fix build this text is absent when the
  completion comes from the lead-side runtime path, which is the point of the
  case.
- Step 5: the step's JSON metadata carries `hook_rejected: true` and the same
  text in `feedback`, and the prose says the completion stands.
- The message window shows no error; the completion still succeeded and
  `tasks.json` shows `status: completed`. The task is **not** reverted to
  `in_progress`: `Completed` is terminal, so the agent is told to raise the
  finding with the lead rather than to retry a transition that cannot happen.
- Step 4 repeated as the teammate rather than the lead (`team_task_claim` then
  `team_task_complete` from that teammate's session) shows the same feedback
  text: FR-018 asks for the verdict on every path, not only the lead's.

---

### TC-012 - A hook that cannot run is recorded, not silently approved

**Requirement:** FR-019
**Task:** T-026

**Preconditions:** Team `thook` from TC-011 is active.

**Steps:**

1. Replace the hook with one that fails validation (contains `>`):
   `jq '.settings.hooks = [{"event":"TaskCompleted","command":"echo > out.txt"}]' \
   .ragent/teams/thook/config.json > h.json && mv h.json \
   .ragent/teams/thook/config.json`
2. Create and complete a task so the hook fires: type
   `create task "ledger probe"` then
   `call team_task_complete(team_name: "thook", task_id: "task-002")`.
3. Inspect the ledger:
   `cat .ragent/teams/thook/hook-failures.jsonl`.
4. Run `/team show thook` and read the hook section.
5. Run `call team_status(team_name: "thook")` and read the hook section.

**Test data:** hook command `echo > out.txt`; event `TaskCompleted`.

**Expected results:**

- Step 3: one JSON line containing the hook command, the reason
  (`hook command contains a shell metacharacter`) and a timestamp. The line is
  a standalone JSON object with `timestamp`, `event`, `hook`, `reason` and
  `detail` keys; `reason` is the token `invalid` (the other two tokens are
  `spawn_failed` and `timeout`). The completion itself still succeeded and the
  agent saw no error: a hook that cannot run does not block work, it is
  reported.
- Step 4 and step 5: both report a non-zero hook-failure count under a
  `Hook failures (1)` heading naming the command and the reason, so the operator
  can tell an inert hook from a passing one. `team_status` also carries
  `hook_failure_count: 1` and the record in its result metadata.
- A hook that runs and approves writes nothing and creates no
  `hook-failures.jsonl` at all: the section is absent, not present-and-zero.
- Once `hook-failures.jsonl` reaches 1 MiB the next failure renames it whole to
  `hook-failures.jsonl.1` (replacing any previous generation) and starts a fresh
  live file, so the ledger is bounded to roughly 2 MiB on disk and no existing
  record is ever rewritten.

**Automated coverage:** `crates/ragent-agent/tests/test_hook_failure_ledger.rs`
(17 tests) drives all three failure kinds plus the rotation, the append-only
guarantee, a truncated final line, and the `team_status` surface.

---

### TC-013 - A foreign-lead team is not silently adopted

**Requirement:** FR-023, FR-024
**Task:** T-029

**Preconditions:** Two terminals available, both at `target/temp/teamtest`.

**Steps:**

1. Terminal 1: launch `ragent`, type `/team create manual towner` and press
   `Enter`. Leave it running.
2. Terminal 1: note the lead session id:
   `jq -r '.lead_session_id' .ragent/teams/towner/config.json`.
3. Terminal 1: create a task and assign it to the lead so it is `in_progress`
   under the lead's id.
4. Terminal 2: launch a **second** `ragent` in the same directory and run
   `/team show towner`.
5. Terminal 2: make it active and act on it - the manager for `towner` is
   constructed by any of `/team show towner`, `/team tasks`, or the first team
   tool call in that session. Use `/team show towner` first, then
   `call team_status(team_name: "towner")`.
6. Terminal 2: confirm whether the foreign team was left alone by comparing
   `md5sum .ragent/teams/towner/config.json` against the value you recorded in
   step 2's terminal.
7. Terminal 2: retry with explicit consent. No `adopt` argument exists yet, so
   on the pre-fix build there is nothing to type; after T-029 the surface this
   case grades is `adopt: true` on the manager-opening path described in
   `PLAN.md` T-029. Until T-029 lands, record the absence of any such surface
   as the case result rather than inventing a command.

**Test data:** team `towner`; two lead sessions.

**Expected results:**

- Step 5/6: an error naming the owning lead session id from step 2, and
  `config.json` byte-identical to the pre-step-5 `md5sum`. On the pre-fix build
  this fails both ways: no error is raised **and** the file changes, because
  `TeamManager::new` (`manager.rs:662`) adopts unconditionally and rewrites
  `lead_session_id`.
- Step 7: after T-029, an explicit adoption proceeds and
  `adopt_orphaned_tasks` has reset the lead-owned `in_progress` task to
  `pending`, with one `TeamAdopted` event listing that task id.

---

### TC-014 - Orphaned teammate tasks are adopted too

**Requirement:** FR-024
**Task:** T-029

**Preconditions:** A team whose task is `in_progress` under a teammate that is
now `stopped`.

**Steps:**

1. Type `/team create manual torphan` and press `Enter`.
2. Create a task and assign it to `alpha`: `create task "orphan probe"` then
   `assign task-001 to alpha`.
3. Kill alpha's session: `Alt+T`, `Down` to alpha, `Del` `Left` `Enter` to
   shut it
   down immediately. Confirm its row reads `stopped`.
4. Verify the task is still owned by the dead teammate:
   `jq '.tasks[] | select(.id=="task-001") | {status,assigned_to}'`.
5. Force the adoption path. Adoption fires whenever a manager is constructed
   by a **different** lead session id than the one in `config.json`
   (`manager.rs:662`). `/team close` alone does not change the lead session id,
   so instead: run `/team close`, restart `ragent` from the same directory (a
   restart creates a new session id), then `/team show torphan` to construct the
   manager.
6. Verify again with the same `jq` command.

**Test data:** team `torphan`; task `task-001` "orphan probe"; owner `alpha`.

**Expected results:**

- Step 4: `{"status": "in_progress", "assigned_to": "tm-001"}`.
- Step 6: `{"status": "pending", "assigned_to": null}` - the departed
  teammate's task was reset, matching the old-lead behaviour. On the pre-fix
  build this stays `in_progress` forever, because `adopt_orphaned_tasks`
  (`manager.rs:1627`) only resets tasks whose `assigned_to` matches the *old
  lead* session id, never a teammate's agent id.

---

### TC-015 - Malformed `tasks.json` is diagnosed, not discarded

**Requirement:** FR-037
**Task:** T-032

**Preconditions:** Team `tparse` active with one task.

**Steps:**

1. Type `/team create manual tparse` and press `Enter`.
2. Create a task: `create task "parse probe"`.
3. Copy the good file: `cp .ragent/teams/tparse/tasks.json good.json`.
4. Break it in the second terminal by removing a closing brace:
   `sed -i 's/]}/]/' .ragent/teams/tparse/tasks.json`
5. In the TUI run `/team tasks` and press `Enter`.
6. Run `call team_task_list(team_name: "tparse")`.
7. Restore: `cp good.json .ragent/teams/tparse/tasks.json`.
8. Run `/team tasks` again.

**Test data:** team `tparse`; task `task-001`.

**Expected results:**

- Steps 5 and 6: an error naming `tasks.json`, a byte offset, and the field
  where parsing failed. The tool must **not** silently return an empty list.
- Nothing writes the empty list back over the file: after step 6, the file on
  disk is byte-identical to the broken version (compare with `md5sum` against a
  saved copy).
- Step 8: the single task is listed again.

---

### TC-016 - A dangling dependency is reported at write time

**Requirement:** FR-038, FR-039
**Task:** T-033

**Preconditions:** Team `tdep` active.

**Steps:**

1. Type `/team create manual tdep` and press `Enter`.
2. Create a task that depends on a task that does not exist: type
   `call team_task_create(team_name: "tdep", title: "blocked forever", depends_on: ["task-999"])`
   and press `Enter`.
3. Read the step output.
4. Confirm nothing was written: `cat .ragent/teams/tdep/tasks.json`. Save a
   copy as `before.json` (with `md5sum before.json`) for the later comparison.
5. Create a legal pair: `create task "first"` (`task-001`) and
   `create task "second"` with `depends_on: ["task-001"]` (`task-002`). Both
   are accepted, which shows the rule is about validity, not about forbidding
   dependencies.
6. Close a cycle behind the runtime's back, so the next write has to validate a
   list it did not build itself. In the second terminal:
   `jq '.tasks[0].depends_on = ["task-002"]' .ragent/teams/tdep/tasks.json > broken.json`
   then `cp broken.json .ragent/teams/tdep/tasks.json`. `task-001` now depends
   on `task-002`, which depends on `task-001`.
7. Perform any write so the loaded list is validated: `create task "third"`.

**Test data:** team `tdep`; dependency id `task-999`; sibling ids `task-001`,
`task-002`.

**Expected results:**

- Step 3: an error naming the missing id (`task-999`), the dependent task, the
  ids that do exist, and stating that the task would never become claimable.
  The tool reports `refusing to write invalid <path>: ...`, so it is clear the
  refusal came from the write boundary rather than from the tool.
- Step 4: `tasks.json` is byte-identical to what step 2 found (`md5sum` matches
  `before.json`), and `task-999`'s dependency was **not** persisted.
- Step 5: both tasks are created; `task-002` shows `[deps: task-001]`.
- Step 7: the write is refused with an error naming the cycle
  (`task-001 -> task-002 -> task-001`) and stating that no task in it can ever
  be claimed. `task-003` is not created, and `tasks.json` is left exactly as the
  operator edited it - the runtime refuses to persist a list it cannot validate
  rather than repairing it silently.

---

### TC-017 - Oversized mailbox message is rejected at send time

**Requirement:** FR-021
**Task:** T-031

**Preconditions:** Team `tmail` active with a spawned `beta` (`tm-002`).

**Steps:**

1. Type `/team create manual tmail` and press `Enter`.
2. Send a normal message to confirm the path works:
   `/team message beta hello there` and press `Enter`.
3. Send an oversized message by using the tool with a long body. Build the body
   first in the second terminal:
   `python3 -c "print('x'*300000)" > big.txt` then paste its contents as the
   `content` argument of
   `call team_message(team_name: "tmail", to: "tm-002", content: "<the 300000 x characters>")`.
4. Read the step output.
5. Verify the mailbox file:
   `ls -l .ragent/teams/tmail/mailboxes/`.

**Test data:** 300,000 `x` characters (the limit is 262,144 bytes).

**Expected results:**

- Step 2: succeeds; the message appears in `beta`'s mailbox file.
- Step 4: an error naming the limit (`262144`) and the actual size (about
  `300000`). This half already works (`mailbox.rs:402`); the case is a
  regression guard for it, not a fix verification.
- Step 5: no oversized entry exists; the mailbox file size is unchanged from
  after step 2.

---

### TC-018 - Undelivered message is reported, not lost

**Requirement:** FR-020
**Task:** T-030

**Preconditions:** Team `tundel` active.

**Steps:**

1. Type `/team create manual tundel` and press `Enter`.
2. Shut `beta` down immediately: `Alt+T`, `Down` to beta, `Del`, `Left`,
   `Enter`.
   Confirm its row reads `stopped`.
3. Send it a message anyway:
   `call team_message(team_name: "tundel", to: "tm-002", content: "are you there")`.
4. Read the step output.
5. Run `/team show tundel` and read the undelivered section.
6. Run `call team_status(team_name: "tundel")` and read the undelivered section.

**Test data:** team `tundel`; recipient `tm-002` (stopped).

**Expected results:**

- Step 4: succeeds (or warns), and the message stays `read: false` on disk -
  verify with `jq '.messages[] | {from,to,read}' .ragent/teams/tundel/mailboxes/*.json`.
- Steps 5 and 6: both name the undelivered message and its recipient, so the
  lead knows the message will never be read instead of assuming it was
  delivered.

---

### TC-019 - `team_wait` distinguishes idle from stuck

**Requirement:** FR-031
**Task:** T-034

**Preconditions:** Team `twait` with `alpha` working (`sleep 400`) and `beta`
idle. Suspend `beta` so it is parked.

**Steps:**

1. Type `/team create manual twait` and press `Enter`.
2. `Alt+T`, `Down` to `beta`, then `Space` `Left` `Enter` to suspend.
   Confirm `suspended`.
3. Run `call team_wait(team_name: "twait", timeout_secs: 30)` and press `Enter`.
4. Read the result summary.

**Test data:** team `twait`; timeout 30 seconds; alpha working, beta suspended.

**Expected results:**

- Step 4: the summary lists each member with its ending state. `beta` is
  reported as **parked** (suspended), `alpha` as **still running** at timeout.
  Neither is reported as merely "timed out" without a per-member reason.

---

### TC-020 - `Teams` popup offers a way out of a parked row

**Requirement:** FR-028, FR-029, FR-033, FR-034
**Task:** T-035, T-036, T-042

**Preconditions:** TC-009 has been run, so the `gated` blueprint
(`require_plan_approval: true`) exists at
`.ragent/blueprints/teams/gated/blueprint.json`. Team `tpopup` will hold one
`blocked` member (injected as in TC-006 step 3) and one `plan_pending` member.

**Steps:**

1. Type `/team create gated tpopup` and press `Enter` (the `gated` blueprint
   from TC-009 has `require_plan_approval: true`, so the teammate enters
   `plan_pending`).
2. Wait for the writer's row to read `planning`.
3. Open `Alt+T` and read the footer hint row.
4. Select the `planning` row and read what the row shows - a plan waiting for
   approval and the approve/reject action, not only elapsed time.
5. Inject a `blocked` member as in TC-006 step 3, reopen `Alt+T`, and read the
   footer hint row again.
6. Press `Space` on the `blocked` row and read the dialog. Then press `Enter`
   **without** toggling, and read the result. Then repeat with `Space`, `Left`,
   `Enter`.
7. Read the footer for the toggle hint.

**Test data:** team `tpopup`; statuses `planning` and `blocked`.

**Expected results:**

- Step 3/4: the `planning` row shows the plan and the approve/reject action.
- Step 5: the footer hint row names the resume action while a `blocked` member
  exists, so the state is visibly escapable.
- Step 6: the un-toggled `Enter` cancels and the member stays `blocked`; the
  `Left` `Enter` sequence resumes it to `idle`. The dialog states which option
  does which (F-27).
- Step 7: the footer states that the dialog's selection must be toggled before
  `Enter` accepts, so the silent no-op is documented where the operator is
  looking.
- The status words read `planning` and `blocked` - the `display_label()`
  spellings - not `plan_pending` or any third variant.

---

### TC-021 - Every team refusal names entity, state, and next action

**Requirement:** FR-026, FR-032, FR-033, FR-042
**Task:** T-004, T-005, T-037

**Preconditions:** Team `trefuse` active.

**Steps:**

1. Type `/team create manual trefuse` and press `Enter`.
2. Trigger five different refusals and copy each message text into a scratch
   file:
   a. `/team cleanup` while a teammate is working.
   b. `call team_task_complete(team_name: "trefuse", task_id: "nope")`.
   c. `call team_task_claim(team_name: "trefuse", task_id: "task-001")` twice,
      from two different teammates.
   d. `/team message nonexistent-name hello`.
   e. `call team_spawn` past the `max_teammates` cap (use the `tiny` blueprint
      from TC-008).
3. For each message, mark whether it contains (i) the offending entity, (ii)
   its current state, (iii) a concrete next action.

**Test data:** team `trefuse`; the five refusal triggers above.

**Expected results:** all five messages contain all three elements. Any message
that only says "failed" or "cannot" without naming a next action is a failure of
this case.

---

### TC-022 - `/team help` and `/team show` describe what the runtime does

**Requirement:** FR-040, FR-041
**Task:** T-039, T-041

**Preconditions:** Team `tshow` active.

**Steps:**

1. Type `/team create manual tshow` and press `Enter`.
2. Run `/team help` and read the table. Check every subcommand the TUI
   actually dispatches is present: `help`, `status`, `show`, `create`, `close`,
   `delete`, `blueprint`, `message`, `tasks`, `clear`, `cleanup`,
   `forcecleanup`, `focus`.
3. Run `/team show tshow` and read the settings block.
4. Compare against the file:
   `jq '.settings' .ragent/teams/tshow/config.json`.
5. Confirm `forcecleanup` is discoverable: it must appear in the step 2 table
   with its guard, not only in `/team cleanup`'s refusal text.

**Test data:** team `tshow`; default settings.

**Expected results:**

- Step 2: every subcommand row states whether it needs the active team, whether
  it writes to disk, and which guard can refuse it. `/team cleanup`,
  `/team forcecleanup`, and `/team delete` rows all name the live-member guard,
  and `forcecleanup` is present at all (F-26).
- Step 3: each `TeamSettings` field is printed with its **effective** value,
  including `watchdog_timeout_secs`, and the values match step 4 exactly.
- Step 5: `forcecleanup` is reachable from the documentation, not only from a
  refusal message that happens to mention it.

---

### TC-023 - Per-member status history answers "why did it stop?"

**Requirement:** FR-008, FR-035, FR-036
**Task:** T-002, T-003, T-004, T-005, T-039

**Preconditions:** Team `thist` active.

**Steps:**

1. Type `/team create manual thist` and press `Enter`.
2. Drive `beta` through several states: `Alt+T`, `Down` to beta,
   `Space` `Left` `Enter` (suspend), `Space` (resume - no dialog from
   `Suspended`), then `Del` `Left` `Enter` (shut down).
3. Run `call team_status(team_name: "thist")` and read beta's row.
4. Cross-check the event log: `rg "TeammateStatusChanged" log/` from
   `target/temp/teamtest`.

**Test data:** team `thist`; teammate `beta` = `tm-002`.

**Expected results:**

- Step 3: beta's entry lists at least five transitions with timestamps:
  `spawning`, `working`, `suspended`, `idle`, `stopped`, plus the cause of each.
- Step 4: exactly one `TeammateStatusChanged` line per transition - no
  duplicates, no missing transitions.

---

### TC-024 - Existing team directories load unchanged

**Requirement:** FR-043
**Task:** T-001, T-040

**Preconditions:** A team directory written by the **previous release**.

**Steps:**

1. Before upgrading, with the old binary, create a team and a task:
   `/team create manual tcompat`, `create task "compat probe"`, and complete
   half of it (leave one `pending`, one `completed`).
2. Note the checksums:
   `md5sum .ragent/teams/tcompat/config.json .ragent/teams/tcompat/tasks.json`.
3. Stop the old binary. Build and launch the new binary from the same directory.
4. Run `/team show tcompat` and `/team tasks`.
5. Read both files' contents with `jq`.

**Test data:** team `tcompat`; statuses `pending`, `completed`.

**Expected results:**

- Step 4: both surfaces list the team and its tasks with no migration prompt
  and no error.
- Step 5: every status string is still snake_case (`pending`, `completed`,
  `in_progress`, `cancelled`). No field is renamed.
- No `.bak` or migration artefact is created in the team directory.

---

### TC-025 - Suspended and blocked members are not reported as failures

**Requirement:** FR-012, FR-032
**Task:** T-014, T-015

**Preconditions:** Team `twd` with alpha working and beta suspended.

**Steps:**

1. Type `/team create manual twd` and press `Enter`.
2. `Alt+T`, `Down` to beta, `Space` `Left` `Enter` (suspend).
3. Leave the TUI idle for **6 minutes** to cross the watchdog timeout.
4. Run `call team_status(team_name: "twd")`.
5. In the second terminal, check for failure records:
   `rg -c "watchdog: no progress" .ragent/teams/twd/config.json log/ 2>/dev/null`.

**Test data:** team `twd`; wait 6 minutes.

**Expected results:**

- Step 4: alpha is reported as it is (working or idle); beta is `suspended`.
  Neither carries a watchdog error string.
- Step 5: no `watchdog: no progress` record exists for beta. A record for alpha
  is acceptable only if alpha itself was silent and live.

---

### TC-026 - Configuration settings round-trip through `/team show`

**Requirement:** FR-041, FR-013
**Task:** T-022, T-039

**Preconditions:** Team `tsettings` active.

**Steps:**

1. Type `/team create manual tsettings` and press `Enter`.
2. Set a custom watchdog timeout by editing config in the second terminal:
   `jq '.settings.watchdog_timeout_secs = 900' \
   .ragent/teams/tsettings/config.json > s.json && mv s.json \
   .ragent/teams/tsettings/config.json`
3. Run `/team show tsettings` and read the settings block.
4. Confirm the setting has an effect: with alpha working, wait 6 minutes (under
   the new 900-second timeout) and confirm alpha is **not** failed.

**Test data:** team `tsettings`; `watchdog_timeout_secs: 900`; wait 6 minutes.

**Expected results:**

- Step 3: the block reports `watchdog_timeout_secs 900`.
- Step 4: alpha is still `working`, not `failed`, because 6 minutes is under the
  configured 15-minute timeout.

---

### TC-027 - Deleting a team never removes live work without consent

**Requirement:** FR-022, FR-023, FR-030, FR-045
**Task:** T-027, T-028, T-038, T-043

**Preconditions:** Team `tdelete` active with a live `alpha`.

**Steps:**

1. Type `/team create manual tdelete` and press `Enter`.
2. Run `/team delete tdelete` and press `Enter`. Read the confirmation.
3. Press `Esc`.
4. In the second terminal confirm the directory still exists:
   `ls .ragent/teams/tdelete`.
5. Run `/team delete tdelete` again and press `Enter`, then confirm.
6. Confirm the directory is gone: `ls .ragent/teams/tdelete`.
7. Repeat steps 1-6 against a team with **no** live members and confirm the
   confirmation step is still required (a destructive action is never one
   keystroke away).
8. Now test the case the current build gets wrong: create a second team,
   `/team create manual tdelete2`, then `/team close` so it is **not** the
   session's active team, then `/team delete tdelete2` and press `Enter`
   **once**.
9. Confirm the directory still exists: `ls .ragent/teams/tdelete2`.

**Test data:** team `tdelete`.

**Expected results:**

- Step 2: a confirmation naming the team and its live member `alpha`, and
  stating that its agent loop will be cancelled.
- Step 3/4: nothing deleted.
- Step 5/6: the directory is removed and `/team status` reports no active team.
- Step 7: the same confirmation appears; `/team delete` is never silent.
- Steps 8/9: a confirmation appears even though `tdelete2` is not the active
  team, the single un-toggled `Enter` cancels, and the directory survives. On
  the pre-fix build step 9 finds no directory, which is the F-28 defect.

## Cleanup

Run these after the final case to leave the machine as it was found.

1. Stop any running `ragent` instance: press `Ctrl+C` in each terminal, or
   `Esc` then close the window. Confirm no `ragent` process remains:
   `pgrep -fl ragent`.
2. Delete the sandbox tree:
   `rm -rf /home/thawkins/Projects/ragent/target/temp/teamtest`.
3. Confirm the repository tree is unchanged:
   `cd /home/thawkins/Projects/ragent && git status --porcelain`.
   Expected: only the three `specs/teamssetup/` files appear (plus any
   pre-existing untracked files). If a case leaked a team directory, log,
   `INDEX.md`, or a generated document into the repository, that is a defect in
   the case setup and must be recorded rather than papered over with
   `git restore`.
4. Remove any background `sleep` left by the blueprint prompts:
   `pkill -f "sleep 400"`.
5. Unset the provider key if the shell is not dedicated to this run:
   `unset ANTHROPIC_API_KEY` (or whichever variable P-3 used).

## Case to Requirement Traceability

| Requirement | Test cases |
| --- | --- |
| FR-001 | TC-001, TC-002 |
| FR-002 | TC-001, TC-002 |
| FR-003 | TC-004 |
| FR-004 | TC-004 |
| FR-005 | TC-003 |
| FR-006 | TC-003 |
| FR-007 | TC-003 |
| FR-008 | TC-023 |
| FR-009 | TC-006, TC-007 |
| FR-010 | TC-006 |
| FR-011 | TC-006 |
| FR-012 | TC-005, TC-025 |
| FR-013 | TC-026 |
| FR-014 | TC-009 |
| FR-015 | TC-010 |
| FR-016 | TC-008 |
| FR-017 | TC-011 |
| FR-018 | TC-011 |
| FR-019 | TC-012 |
| FR-020 | TC-018 |
| FR-021 | TC-017 |
| FR-022 | TC-006, TC-007, TC-027 |
| FR-023 | TC-013, TC-027 |
| FR-024 | TC-013, TC-014 |
| FR-025 | TC-001, TC-003, TC-004 |
| FR-026 | TC-007, TC-021 |
| FR-027 | TC-010 |
| FR-028 | TC-020 |
| FR-029 | TC-006, TC-020 |
| FR-030 | TC-027 |
| FR-031 | TC-019 |
| FR-032 | TC-007, TC-021, TC-025 |
| FR-033 | TC-020, TC-021 |
| FR-034 | TC-020 |
| FR-035 | TC-023 |
| FR-036 | TC-023 |
| FR-037 | TC-015 |
| FR-038 | TC-016 |
| FR-039 | TC-016 |
| FR-040 | TC-022 |
| FR-041 | TC-022, TC-026 |
| FR-042 | TC-001, TC-021 |
| FR-043 | TC-024 |
| FR-044 | TC-001 |
| FR-045 | TC-027 |
