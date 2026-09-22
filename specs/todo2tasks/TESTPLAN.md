---
status: draft
spec_id: todo2tasks
title: "Todo-to-Tasks Migration — Manual Test Plan"
created: 2026-08-16
---

# Manual Test Plan — Todo-to-Tasks Migration

**Spec:** `specs/todo2tasks/SPEC.md`
**Plan:** `specs/todo2tasks/PLAN.md`

This document defines **manual** test cases for a human tester to execute
against a running ragent instance (TUI or CLI). Each case lists
preconditions, step-by-step instructions, test data, and expected
results. Test cases are keyed to the spec requirements they cover.

## Prerequisites

- A working ragent build (`cargo build`) with the Todo-to-Tasks
  migration applied.
- A configured LLM provider (any provider will do; the test prompts are
  simple tool-call requests).
- A clean or known-state SQLite database. If testing migration of
  legacy data, pre-populate the `todos` table with rows that lack the
  new columns before starting.
- The TUI is the primary test surface (`ragent` with no `--no-tui` flag)
  unless a case specifies CLI mode.

## Test Cases

### TC-001 — Create a task with subject and description

**Covers:** FR-001, FR-011, FR-012

**Preconditions:**
- A fresh session is open in the TUI.

**Steps:**
1. Type the following prompt:
   ```
   Create a task with subject "Set up CI pipeline" and description "Configure GitHub Actions for lint, test, and build stages."
   ```
2. Observe the LLM calls `task_create`.
3. Observe the tool result returned to the LLM.

**Test data:**
- Subject: `Set up CI pipeline`
- Description: `Configure GitHub Actions for lint, test, and build stages.`

**Expected results:**
- The tool result contains a task object with:
  - `id` — a non-empty string matching the existing `generate_todo_id` scheme.
  - `subject` — `Set up CI pipeline`
  - `description` — `Configure GitHub Actions for lint, test, and build stages.`
  - `status` — `pending`
  - `active_form` — empty or null
  - `owner` — empty or null
  - `metadata` — `{}`
  - `blocked_by` — `[]`
  - `created_at` and `updated_at` — present, non-null.
- The task is scoped to the current session (verify by opening a
  different session and confirming the task does not appear there).

---

### TC-002 — Update task status through the lifecycle

**Covers:** FR-011, FR-013

**Preconditions:**
- TC-001 has been completed; the task ID is known.

**Steps:**
1. Prompt the LLM:
   ```
   Update the task with ID <id> to status in_progress.
   ```
2. Observe the `task_update` tool call and its result.
3. Prompt the LLM:
   ```
   Update the task with ID <id> to status completed.
   ```
4. Observe the second `task_update` tool call and its result.

**Test data:**
- Task ID: the ID from TC-001.
- Status sequence: `in_progress` → `completed`.

**Expected results:**
- After step 2, the result shows `status` = `in_progress` and
  `updated_at` is later than `created_at`.
- After step 4, the result shows `status` = `completed`.
- No permission prompt appears for either `task_update` call (the tool
  is hardwired auto-approved per FR-011).

---

### TC-003 — Create tasks with dependency edges

**Covers:** FR-004, FR-009, FR-012, FR-013

**Preconditions:**
- A fresh session is open.

**Steps:**
1. Prompt the LLM:
   ```
   Create a task with subject "Write database migration" and description "Add the tasks table schema."
   ```
2. Note the returned task ID (call it `task-A`).
3. Prompt the LLM:
   ```
   Create a task with subject "Write migration tests" and description "Test the new schema." with blocked_by set to ["<task-A>"].
   ```
4. Note the returned task ID (call it `task-B`).
5. Prompt the LLM:
   ```
   Get task <task-B>.
   ```
6. Observe the `task_get` result.

**Test data:**
- `task-A` subject: `Write database migration`
- `task-B` subject: `Write migration tests`
- `task-B` blocked_by: `["<task-A>"]`

**Expected results:**
- `task-B` is created successfully with `blocked_by` = `["<task-A>"]`.
- `task_get` on `task-B` returns:
  - `blocked_by` containing `task-A`'s ID.
  - `blocks` — derived, empty for `task-B` (nothing is blocked by it).
- `task_get` on `task-A` (performed separately) would show `blocks`
  containing `task-B`'s ID (the inverse edge).

---

### TC-004 — Auto-unblock when a blocking task completes

**Covers:** FR-003, FR-005, FR-013, FR-015

**Preconditions:**
- TC-003 has been completed; `task-A` and `task-B` are known.
- `task-A` is in `pending` status; `task-B` is in `pending` status and
  blocked by `task-A`.

**Steps:**
1. Prompt the LLM:
   ```
   List all tasks.
   ```
2. Observe the `task_list` result — confirm `task-B` is annotated as
   blocked.
3. Prompt the LLM:
   ```
   Update task <task-A> to status completed.
   ```
4. Prompt the LLM:
   ```
   List all tasks.
   ```
5. Observe the second `task_list` result.

**Test data:**
- Task IDs from TC-003.

**Expected results:**
- In step 2, `task-B` shows `status` = `pending` and is annotated as
  blocked (e.g., `[blocked by #<task-A>]` or equivalent in the
  structured output).
- In step 5, `task-B` still shows `status` = `pending` but is now
  flagged as "available" (unblocked) — no blocked annotation.
- `task-A` shows `status` = `completed`.

---

### TC-005 — Reject a dependency cycle

**Covers:** FR-004, FR-013

**Preconditions:**
- TC-003 has been completed; `task-A` and `task-B` are known.
- `task-B` is already blocked by `task-A`.

**Steps:**
1. Prompt the LLM:
   ```
   Update task <task-A> to add blocked_by ["<task-B>"].
   ```
2. Observe the `task_update` tool result.

**Test data:**
- `task-A` ID, `task-B` ID.

**Expected results:**
- The `task_update` call fails with an error describing the cycle
  (e.g., "Dependency cycle detected: <task-A> → <task-B> → <task-A>").
- The edge is NOT persisted — `task_A`'s `blocked_by` remains empty.
- The error message names the specific task IDs involved in the cycle.

---

### TC-006 — Reject a reference to a non-existent task ID

**Covers:** FR-009, FR-012, FR-013

**Preconditions:**
- A fresh session is open.

**Steps:**
1. Prompt the LLM:
   ```
   Create a task with subject "Orphan dependency test" and description "Tests rejection of bad refs." with blocked_by set to ["nonexistent-id-999"].
   ```
2. Observe the `task_create` tool result.

**Test data:**
- blocked_by: `["nonexistent-id-999"]`

**Expected results:**
- The `task_create` call fails with an error stating that
  `nonexistent-id-999` does not exist in the current session.
- No task is created.

---

### TC-007 — Reject `status=blocked` on task_update

**Covers:** FR-005, FR-013

**Preconditions:**
- A task exists in the current session (from TC-001 or a new one).

**Steps:**
1. Prompt the LLM:
   ```
   Update task <id> to status blocked.
   ```
2. Observe the `task_update` tool result.

**Test data:**
- Any valid task ID.
- Status: `blocked`

**Expected results:**
- The `task_update` call fails with an error listing the valid statuses:
  `pending`, `in_progress`, `completed`.
- The task's status is unchanged.

---

### TC-008 �� Backward-compatible `todo_read` alias

**Covers:** FR-016

**Preconditions:**
- At least one task exists in the current session.

**Steps:**
1. Prompt the LLM:
   ```
   Call todo_read to list all todos.
   ```
2. Observe the `todo_read` tool result.

**Test data:**
- (none — reads existing session tasks)

**Expected results:**
- `todo_read` returns a human-readable list of tasks (same format as
  the old Todo output).
- The output includes a deprecation notice directing callers to use
  `task_list` instead.
- The task data returned matches what `task_list` would return for the
  same session.

---

### TC-009 — Backward-compatible `todo_write` alias

**Covers:** FR-016

**Preconditions:**
- A fresh session is open.

**Steps:**
1. Prompt the LLM:
   ```
   Call todo_write with action add, title "Legacy add test", and description "Added via the old tool."
   ```
2. Observe the `todo_write` tool result.
3. Prompt the LLM:
   ```
   Call todo_write with action complete for the task that was just created.
   ```
4. Observe the second `todo_write` tool result.

**Test data:**
- action: `add`
- title: `Legacy add test`
- description: `Added via the old tool.`
- action: `complete`

**Expected results:**
- Step 2: A task is created (delegating to `task_create` internally).
  The output includes a deprecation notice directing callers to use
  `task_create`.
- Step 4: The task's status is set to `completed` (delegating to
  `task_update` internally). The output includes a deprecation notice
  directing callers to use `task_update`.
- Both outputs confirm the operation succeeded.

---

### TC-010 — Owner and metadata fields

**Covers:** FR-006, FR-008, FR-012, FR-014

**Preconditions:**
- A fresh session is open.

**Steps:**
1. Prompt the LLM:
   ```
   Create a task with subject "Auth module" and description "Implement JWT auth." Set owner to "coder-agent" and metadata to {"feature": "auth", "phase": 1, "priority": "high"}.
   ```
2. Note the returned task ID.
3. Prompt the LLM:
   ```
   Get task <id>.
   ```
4. Observe the `task_get` result.

**Test data:**
- subject: `Auth module`
- description: `Implement JWT auth.`
- owner: `coder-agent`
- metadata: `{"feature": "auth", "phase": 1, "priority": "high"}`

**Expected results:**
- The task is created with `owner` = `coder-agent` and `metadata`
  containing all three key-value pairs.
- `task_get` returns the `owner` and `metadata` fields verbatim.
- The metadata keys `feature`, `phase`, `priority` are all present and
  accepted silently (open schema per FR-008).

---

### TC-011 — Active form display in TUI panel

**Covers:** FR-007, FR-018

**Preconditions:**
- A fresh session is open in the TUI.

**Steps:**
1. Prompt the LLM:
   ```
   Create a task with subject "Implement JWT auth" and description "Add JWT middleware." Set active_form to "Implementing JWT auth".
   ```
2. Note the returned task ID.
3. Prompt the LLM:
   ```
   Update task <id> to status in_progress.
   ```
4. Press Alt+T to open the tasks panel.
5. Observe the panel content.

**Test data:**
- subject: `Implement JWT auth`
- active_form: `Implementing JWT auth`

**Expected results:**
- The panel header reads "TASKS" (not "TODO").
- The task appears as a cyan-coloured line (status = `in_progress`).
- The subject "Implement JWT auth" is displayed on the main line.
- The active_form "Implementing JWT auth" is displayed as an indented
  sub-line beneath the subject.
- No `[blocked by …]` annotation appears (the task has no dependencies).

---

### TC-012 — Legacy database migration

**Covers:** FR-002, NFR-002

**Preconditions:**
- A ragent database exists with rows in the `todos` table that predate
  the migration (i.e., the rows do not have the new columns:
  `active_form`, `owner`, `metadata`, `blocked_by`).
- To set this up: start ragent on the pre-migration build, create a few
  todos, then shut down. Apply the migration and restart.

**Steps:**
1. Start ragent with the migrated build, using the same database.
2. Open a session that had pre-existing todos.
3. Prompt the LLM:
   ```
   List all tasks.
   ```
4. Observe the `task_list` result.

**Test data:**
- (uses pre-existing database rows)

**Expected results:**
- ragent starts without errors — the schema migration runs
  additively (ALTER TABLE ADD COLUMN).
- The pre-existing todos appear in the `task_list` output.
- Each legacy row has safe defaults:
  - `active_form` — empty
  - `owner` — null/None
  - `metadata` — `{}`
  - `blocked_by` — `[]`
- No data loss: the `subject` field contains the old `title` value, and
  `description`, `status` (mapped: `done` → `completed`), and
  timestamps are preserved.

---

### TC-013 — TUI panel mutual exclusion and scroll

**Covers:** FR-018

**Preconditions:**
- A session is open with multiple tasks (at least 5) in the TUI.
- The tasks panel is not currently visible.

**Steps:**
1. Press Alt+T to open the tasks panel.
2. Confirm the panel is visible and the status bar says "tasks panel
   visible".
3. Press the key to open the log panel (e.g., Alt+L or the configured
   toggle).
4. Confirm the tasks panel is no longer visible and the log panel is
   visible instead.
5. Press Alt+T again to re-open the tasks panel.
6. Use scroll keys (Page Up / Page Down or arrow keys) to scroll through
   the task list.

**Test data:**
- (uses existing session tasks)

**Expected results:**
- Only one panel (tasks / log / profile / memory / telemetry) is
  visible at a time.
- The status bar updates to reflect which panel is visible.
- Scrolling works: the scroll offset changes, and the scrollbar (if
  rendered) reflects the position.
- Text selection (if applicable) continues to work within the tasks
  panel.

---

### TC-014 — `/task` and `/tasks` slash command aliases

**Covers:** FR-019

**Preconditions:**
- A session is open in the TUI.
- The tasks panel is not currently visible.

**Steps:**
1. Type `/task` and press Enter.
2. Observe whether the tasks panel appears.
3. Type `/task` and press Enter again.
4. Observe whether the tasks panel hides.
5. Type `/tasks` and press Enter.
6. Observe whether the tasks panel appears.
7. Type `/todo` and press Enter.
8. Observe whether the tasks panel hides (toggle).

**Test data:**
- (no arguments — panel toggle mode)

**Expected results:**
- `/task` toggles the tasks panel on/off.
- `/tasks` toggles the tasks panel on/off (same behaviour as `/task`).
- `/todo` continues to toggle the panel (backward-compatible alias).
- All three commands are accepted without error.

---

### TC-015 — Status filter on `task_list`

**Covers:** FR-015

**Preconditions:**
- A session is open with tasks in multiple statuses:
  - At least one `pending` task.
  - At least one `in_progress` task.
  - At least one `completed` task.

**Steps:**
1. Prompt the LLM:
   ```
   List all tasks with status filter "pending".
   ```
2. Observe the `task_list` result.
3. Prompt the LLM:
   ```
   List all tasks with status filter "in_progress".
   ```
4. Observe the result.
5. Prompt the LLM:
   ```
   List all tasks with status filter "completed".
   ```
6. Observe the result.
7. Prompt the LLM:
   ```
   List all tasks with status filter "all".
   ```
8. Observe the result.

**Test data:**
- Filter values: `pending`, `in_progress`, `completed`, `all`.

**Expected results:**
- Step 2: Only `pending` tasks are returned.
- Step 4: Only `in_progress` tasks are returned.
- Step 6: Only `completed` tasks are returned.
- Step 8: All tasks are returned (default behaviour when filter is
  `all` or omitted).
- Each entry in every result includes `id`, `subject`, `status`,
  `owner` (if any), and `blocked_by` (array).
- Results are ordered by `created_at` (ascending).

---

### TC-016 — Derived `blocks` field in `task_get`

**Covers:** FR-005, FR-014

**Preconditions:**
- A session is open with at least two tasks where `task-A` is listed in
  `task-B`'s `blocked_by`.

**Steps:**
1. Prompt the LLM:
   ```
   Get task <task-A>.
   ```
2. Observe the `task_get` result for `task-A`.
3. Prompt the LLM:
   ```
   Get task <task-B>.
   ```
4. Observe the `task_get` result for `task-B`.

**Test data:**
- `task-A` and `task-B` IDs from a prior dependency setup (TC-003 or
  fresh).

**Expected results:**
- `task-A`'s result includes:
  - `blocked_by` — `[]` (task-A is not blocked by anything).
  - `blocks` — an array containing `task-B`'s ID (derived: task-B
    lists task-A in its `blocked_by`).
- `task-B`'s result includes:
  - `blocked_by` — an array containing `task-A`'s ID.
  - `blocks` — `[]` (nothing lists task-B in their `blocked_by`).
- The `blocks` field is derived at read time, not stored.

---

### TC-017 — `active_form` fallback to subject

**Covers:** FR-007, FR-018

**Preconditions:**
- A fresh session is open in the TUI.

**Steps:**
1. Prompt the LLM:
   ```
   Create a task with subject "Write integration tests" and description "Add end-to-end test coverage." Do not set active_form.
   ```
2. Note the returned task ID.
3. Prompt the LLM:
   ```
   Update task <id> to status in_progress.
   ```
4. Press Alt+T to open the tasks panel.
5. Observe the panel content for this task.

**Test data:**
- subject: `Write integration tests`
- active_form: (not provided)

**Expected results:**
- The task appears as a cyan-coloured line (status = `in_progress`).
- The subject "Write integration tests" is displayed on the main line.
- No separate active_form sub-line appears (since `active_form` was not
  provided, the panel falls back to showing only the subject per
  FR-007).

---

### TC-018 — TUI panel refresh after task mutation

**Covers:** FR-017

**Preconditions:**
- A session is open in the TUI with the tasks panel visible (Alt+T).
- At least one task exists.

**Steps:**
1. Note the current content of the tasks panel.
2. Prompt the LLM:
   ```
   Create a task with subject "Refresh test task" and description "Verifies panel auto-refresh."
   ```
3. Observe the tasks panel without pressing any keys.

**Test data:**
- subject: `Refresh test task`
- description: `Verifies panel auto-refresh.`

**Expected results:**
- The tasks panel updates automatically after the `task_create` tool
  completes — the new task appears without requiring a manual Alt+T
  toggle.
- No manual panel toggle or refresh key press is needed.

---

## Cleanup

After all test cases are complete:

1. Delete the test database if one was created specifically for this
   test run.
2. If the legacy migration test (TC-012) used a real database, back it
   up first or run on a copy to avoid data loss.
3. Close all ragent sessions.