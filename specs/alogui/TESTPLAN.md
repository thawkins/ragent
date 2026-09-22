---
status: draft
---

# Manual Test Plan — /alog Activity Log UI

Spec: `specs/alogui/SPEC.md`

## Prerequisites

1. A working build of `ragent` with the `/alog` command implemented:
   `cargo build` succeeds.
2. At least one ragent session has run previously so that the activity-log
   database exists under the platform data directory
   (`~/.local/share/ragent` on Linux, or the equivalent `dirs::data_dir()`
   location on macOS/Windows). If the database does not yet exist, the
   `/alog` commands must still run without crashing and must report that the
   log is empty / unreachable.
3. The terminal supports the ragent TUI (alternate screen, mouse, etc.).
4. No other ragent process is holding an exclusive lock on the activity-log
   database (WAL mode allows concurrent readers).
5. The current working directory is writable so that `/alog export` can
   create the `log/exports/` directory and write the export file into it.

## Test Cases

### TC-001 — `/alog` with no subcommand shows help

**Preconditions:**
- ragent TUI is open.
- The activity-log database may or may not exist.

**Steps:**
1. Type `/alog` in the input line (no trailing subcommand).
2. Press `Enter`.

**Expected results:**
- An assistant message appears prefixed `From: /alog help`.
- The message contains a table listing the subcommands: `help`, `config`,
  `list`, `status`, `delete`, `export`.
- The status bar (bottom of the TUI) reads `alog: help`.
- No error text appears.

---

### TC-002 — `/alog help` shows the help table

**Preconditions:**
- ragent TUI is open.

**Steps:**
1. Type `/alog help`.
2. Press `Enter`.

**Expected results:**
- Identical output to TC-001 (the explicit `help` subcommand).
- The help table includes a `delete` row describing the syntax
  `/alog delete <run-id> --yes` and noting that `--yes` is required.
- The help table includes an `export` row describing the syntax
  `/alog export <run-id> --yes`, noting that `--yes` is required, and that
  the output file is written to `log/exports/export-<run-id>.jsonl`.
- Status bar reads `alog: help`.

---

### TC-003 — `/alog config` shows database location and setup

**Preconditions:**
- ragent TUI is open.
- The activity-log database file path is known (the platform data directory
  joined with the activity-log filename).

**Steps:**
1. Type `/alog config`.
2. Press `Enter`.

**Expected results:**
- An assistant message appears prefixed `From: /alog config`.
- The message shows:
  - the resolved filesystem path of the activity-log database,
  - whether that file exists on disk,
  - the SQLite journal mode (e.g. `WAL`),
  - the `ACTIVITY_EVENT_SCHEMA_VERSION` value, and
  - a short note on the data-directory location convention.
- Status bar reads `alog: config`.

---

### TC-004 — `/alog list` enumerates runs with counts

**Preconditions:**
- ragent TUI is open.
- At least one prior ragent session has produced activity-log events so that
  `list_runs()` returns one or more runs. If none exist, see TC-005.

**Steps:**
1. Type `/alog list`.
2. Press `Enter`.

**Expected results:**
- An assistant message appears prefixed `From: /alog list`.
- For each run the message lists:
  - the `RunId` (truncated short prefix is acceptable),
  - the derived `RunStatus` (`Active`, `Completed`, `Interrupted`, etc.),
  - the total event count, and
  - a per-`EventKind` breakdown, e.g. a line or column block showing counts
    for model messages, tool calls, tool results, permission decisions,
    checkpoints, terminations, branch-origin, mutation-rejected, lifecycle.
- The runs appear in a readable table or list layout.
- Status bar reads `alog: list`.
- No error text appears if runs exist.

---

### TC-005 — `/alog list` on an empty log

**Preconditions:**
- ragent TUI is open.
- The activity-log database does not exist or contains zero runs (e.g. a
  fresh install or after deleting the database file).

**Steps:**
1. Type `/alog list`.
2. Press `Enter`.

**Expected results:**
- An assistant message appears stating the activity log is empty (no runs
  found).
- Status bar reads `alog: list`.
- The TUI does not crash or show a Rust panic.

---

### TC-006 — `/alog status` shows system health

**Preconditions:**
- ragent TUI is open.
- At least one run exists in the activity log (ideally a mix: some
  `Completed`, some `Interrupted` if reproducible).

**Steps:**
1. Type `/alog status`.
2. Press `Enter`.

**Expected results:**
- An assistant message appears prefixed `From: /alog status`.
- The message shows:
  - whether the database is openable,
  - total number of runs,
  - total number of events across all runs,
  - counts of runs grouped by `RunStatus`,
  - the current `ACTIVITY_EVENT_SCHEMA_VERSION`, and
  - a one-line health indicator (e.g. "healthy" / "unreachable").
- Status bar reads `alog: status`.

---

### TC-007 — `/alog status` when the database cannot be opened

**Preconditions:**
- ragent TUI is open.
- The activity-log database file is missing, or its directory is not
  readable (simulate by temporarily renaming the file or pointing at a
  non-existent path).

**Steps:**
1. Type `/alog status`.
2. Press `Enter`.

**Expected results:**
- An assistant warning message appears describing the open failure and
  including the error detail.
- The TUI does not crash; the user can continue running other slash commands
  afterward.
- Status bar reads `alog: status` (or `alog: error`).

---

### TC-008 — `/alog` appears in the slash autocomplete menu

**Preconditions:**
- ragent TUI is open.

**Steps:**
1. Type `/` to open the slash-command autocomplete menu.
2. Type `a` then `l` to filter for `alog`.

**Expected results:**
- The `alog` entry appears in the filtered list with a short description
  referencing the activity log.
- Selecting it and pressing `Enter` inserts `/alog ` into the input line.

---

### TC-009 — `/alog <sub>` autocomplete suggestions

**Preconditions:**
- ragent TUI is open.
- The `alog` subcommand autocomplete is implemented (FR-009, FR-017, FR-025).

**Steps:**
1. Type `/alog ` (note the trailing space).
2. Observe whether the autocomplete suggests `help`, `config`, `list`,
   `status`, `delete`, `export`.

**Expected results:**
- The subcommands `help`, `config`, `list`, `status`, `delete`, and `export`
  are suggested if the implementation provides subcommand completions for
  `/alog`.
- If FR-009 / FR-017 / FR-025 are not implemented, this test is informational
  only (those features are optional).

---

### TC-010 — Inspection subcommands do not mutate the activity log

**Preconditions:**
- ragent TUI is open.
- Note the current run count and total event count (run `/alog status` once
  and record the numbers).

**Steps:**
1. Run `/alog help`.
2. Run `/alog config`.
3. Run `/alog list`.
4. Run `/alog status`.
5. Run `/alog status` again.

**Expected results:**
- The total run count and total event count reported by the final
  `/alog status` are unchanged from the values recorded in the
  preconditions.
- No new runs or events were created by any inspection subcommand.

---

### TC-011 — `/alog delete` without `--yes` flag aborts

**Preconditions:**
- ragent TUI is open.
- At least one run exists in the activity log (note its `RunId` from
  `/alog list`).

**Steps:**
1. Type `/alog delete <run-id>` (substitute a real run-id from the list,
   do NOT append `--yes`).
2. Press `Enter`.

**Expected results:**
- An assistant warning message appears stating that `--yes` is required to
  confirm the destructive deletion.
- The warning message states clearly that deletion is irreversible (deleted
  runs cannot be recovered).
- No storage mutation occurs — the run still appears in a subsequent
  `/alog list` with the same event count.
- Status bar reads `alog: delete` or `alog: aborted`.

---

### TC-012 — `/alog delete` without a run-id argument shows usage error

**Preconditions:**
- ragent TUI is open.

**Steps:**
1. Type `/alog delete --yes` (note: no `<run-id>` before the flag).
2. Press `Enter`.

**Expected results:**
- An assistant message appears showing a usage error with the expected
  syntax: `/alog delete <run-id> --yes`.
- No storage operation is attempted.
- Status bar reads `alog: delete` or `alog: usage`.

---

### TC-013 — `/alog delete <unknown-run-id> --yes` warns and does not delete

**Preconditions:**
- ragent TUI is open.
- A run-id that does not exist in the activity log (e.g. `nonexistent-run`).

**Steps:**
1. Type `/alog delete nonexistent-run --yes`.
2. Press `Enter`.

**Expected results:**
- An assistant warning message appears stating that no run with that
  identifier was found.
- No deletion is performed.
- Status bar reads `alog: delete` or `alog: not found`.

---

### TC-014 — `/alog delete <run-id> --yes` deletes the run and confirms

**Preconditions:**
- ragent TUI is open.
- At least one run exists in the activity log. Record its `RunId` and event
  count via `/alog list` before deletion.
- Total run count and total event count recorded via `/alog status`.

**Steps:**
1. Type `/alog delete <run-id> --yes` (substitute the recorded run-id).
2. Press `Enter`.
3. Run `/alog list` again.
4. Run `/alog status` again.

**Expected results:**
- After step 2, an assistant message appears prefixed `From: /alog delete`
  stating the `RunId` that was deleted and the number of events removed.
- The event-removed count matches the count recorded in the preconditions.
- Status bar reads `alog: deleted`.
- After step 3, the deleted `RunId` no longer appears in the `/alog list`
  output.
- After step 4, the total run count has decreased by exactly one, and the
  total event count has decreased by the number of events that belonged to
  the deleted run.

---

### TC-015 — `/alog delete` leaves other runs intact

**Preconditions:**
- ragent TUI is open.
- At least two runs exist in the activity log. Record both `RunId`s, both
  event counts, and the total run/event counts via `/alog status`.

**Steps:**
1. Type `/alog delete <run-id-A> --yes` (delete only the first run).
2. Press `Enter`.
3. Run `/alog list`.
4. Run `/alog status`.

**Expected results:**
- Run A no longer appears in `/alog list` output (step 3).
- Run B still appears in `/alog list` with its event count unchanged from
  the preconditions.
- The total event count in `/alog status` (step 4) decreased by exactly the
  event count of run A; run B's events are untouched.

---

### TC-016 — `/alog delete` when the database cannot be opened

**Preconditions:**
- ragent TUI is open.
- The activity-log database file is missing or unreadable (simulate by
  temporarily renaming the file).

**Steps:**
1. Type `/alog delete <any-run-id> --yes`.
2. Press `Enter`.

**Expected results:**
- An assistant warning message appears describing the open failure with the
  error detail.
- The TUI does not crash; the user can continue running other slash commands.
- Status bar reads `alog: delete` or `alog: error`.

---

### TC-017 — `/alog export` without `--yes` flag aborts

**Preconditions:**
- ragent TUI is open.
- At least one run exists in the activity log (note its `RunId` from
  `/alog list`).

**Steps:**
1. Type `/alog export <run-id>` (substitute a real run-id from the list,
   do NOT append `--yes`).
2. Press `Enter`.

**Expected results:**
- An assistant warning message appears stating that `--yes` is required to
  confirm the export operation.
- No file is written to `log/exports/`.
- No storage mutation occurs — the run still appears in a subsequent
  `/alog list` with the same event count.
- Status bar reads `alog: export` or `alog: aborted`.

---

### TC-018 — `/alog export` without a run-id argument shows usage error

**Preconditions:**
- ragent TUI is open.

**Steps:**
1. Type `/alog export --yes` (note: no `<run-id>` before the flag).
2. Press `Enter`.

**Expected results:**
- An assistant message appears showing a usage error with the expected
  syntax: `/alog export <run-id> --yes`.
- No storage operation is attempted and no file is written.
- Status bar reads `alog: export` or `alog: usage`.

---

### TC-019 — `/alog export <unknown-run-id> --yes` warns and does not export

**Preconditions:**
- ragent TUI is open.
- A run-id that does not exist in the activity log (e.g. `nonexistent-run`).

**Steps:**
1. Type `/alog export nonexistent-run --yes`.
2. Press `Enter`.

**Expected results:**
- An assistant warning message appears stating that no run with that
  identifier was found.
- No export is performed and no file is written.
- Status bar reads `alog: export` or `alog: not found`.

---

### TC-020 — `/alog export <run-id> --yes` writes a JSONL file and confirms

**Preconditions:**
- ragent TUI is open.
- At least one run exists in the activity log. Record its `RunId` and event
  count via `/alog list` before export.
- The `log/exports/` directory does not already exist (or is empty) in the
  current working directory.

**Steps:**
1. Type `/alog export <run-id> --yes` (substitute the recorded run-id).
2. Press `Enter`.
3. Open the file `log/exports/export-<run-id>.jsonl` in a text editor.

**Expected results:**
- After step 2, an assistant message appears prefixed `From: /alog export`
  stating the `RunId` that was exported, the number of events written, and
  the filesystem path of the exported file.
- The event count in the confirmation message matches the count recorded in
  the preconditions.
- Status bar reads `alog: exported`.
- The file `log/exports/export-<run-id>.jsonl` exists on disk.
- The file contains one JSON object per line, in ascending sequence-number
  order, and the number of lines equals the event count.
- Each line is valid JSON containing the `ActivityEvent` fields (`id`,
  `run_id`, `seq`, `schema_version`, `timestamp`, `kind`).

---

### TC-021 — `/alog export` does not mutate the activity log

**Preconditions:**
- ragent TUI is open.
- At least one run exists in the activity log. Record its `RunId`, event
  count, and the total run/event counts via `/alog status`.

**Steps:**
1. Type `/alog export <run-id> --yes` (substitute the recorded run-id).
2. Press `Enter`.
3. Run `/alog list`.
4. Run `/alog status`.

**Expected results:**
- After step 3, the exported `RunId` still appears in the `/alog list`
  output with its event count unchanged from the preconditions.
- After step 4, the total run count and total event count are unchanged
  from the preconditions. The export wrote a read-only copy; the original
  run data remains intact in the database.

---

### TC-022 — `/alog export` creates `log/exports/` if it does not exist

**Preconditions:**
- ragent TUI is open.
- At least one run exists in the activity log.
- The `log/exports/` directory does NOT exist in the current working
  directory (delete it if it does).

**Steps:**
1. Type `/alog export <run-id> --yes` (substitute a real run-id).
2. Press `Enter`.
3. Verify the filesystem for the presence of `log/exports/`.

**Expected results:**
- The `log/exports/` directory is created automatically.
- The export file `log/exports/export-<run-id>.jsonl` is written inside it.
- No error about a missing directory appears.

---

### TC-023 — `/alog export` when the database cannot be opened

**Preconditions:**
- ragent TUI is open.
- The activity-log database file is missing or unreadable (simulate by
  temporarily renaming the file).

**Steps:**
1. Type `/alog export <any-run-id> --yes`.
2. Press `Enter`.

**Expected results:**
- An assistant warning message appears describing the open failure with the
  error detail.
- The TUI does not crash; the user can continue running other slash commands.
- No file is written to `log/exports/`.
- Status bar reads `alog: export` or `alog: error`.

---

## Cleanup

1. If you renamed or moved the activity-log database for any test
   (TC-007, TC-016, TC-023), restore it to its original path.
2. If you deleted a run during TC-014 or TC-015 and need it back, re-run a
   ragent session to generate fresh activity-log data — deleted runs cannot
   be recovered (NFR-002).
3. Exported JSONL files written to `log/exports/` during TC-020–TC-022 may
   be deleted if they are no longer needed. The exports are copies; deleting
   the export files does not affect the activity-log database.
4. Quit the TUI with `Ctrl+C` twice (or `/quit`) and confirm the terminal
   returns to a normal shell prompt with no raw-mode artifacts.