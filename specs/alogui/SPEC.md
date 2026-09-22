---
status: draft
audit:
  - { time: 1787733990, from: "none", to: "draft", actor: "system" }
---
# /alog — Activity Log UI

## Overview

The activity log subsystem (implemented in `ragent-storage::activity_log` and
`ragent_types::activity`) maintains an append-only, SQLite-backed event log of
agent execution facts: model messages, tool calls, tool results, permission
decisions, checkpoints, terminations, and lifecycle events. Each event belongs
to a run identified by a `RunId` and carries a per-run monotonic sequence
number, an immutable `EventId`, a schema version, and a typed `EventKind`
payload.

The subsystem already exposes a rich storage API — `ActivityLog::list_runs`,
`count`, `run_status`, `read_run`, `export_jsonl`, `replay_run`, etc. — but
there is no in-TUI surface for inspecting it. This spec defines a new slash
command, `/alog`, that gives the user read-only access to activity-log
information: help, configuration/location, per-run listing with object counts,
and overall system status.

## Scope

The `/alog` command is primarily **read-only and informational**. It does
not modify the activity-log database schema or storage path. It surfaces
information that the storage layer already computes, with one exception:
the `delete` subcommand, which removes a run's events from the log when
explicitly confirmed with `--yes`.

In scope:

- `/alog help` — usage reference.
- `/alog config` — show where the activity-log database lives and how it is
  configured.
- `/alog list` — enumerate every run in the log with per-run event counts
  broken down by `EventKind`.
- `/alog status` — a one-shot health summary of the activity-log system.
- `/alog delete <run-id> --yes` — delete all events for a given run. The
  `--yes` flag is mandatory; without it the command warns and aborts.

Out of scope:

- Mutating, expiring, or archiving runs via the inspection subcommands
  (`help`, `config`, `list`, `status`), which remain read-only (FR-010).
- Exporting JSONL from the TUI (the storage API exists; a future spec may add
  `/alog export <run>`).
- Replay/rollback/resume of runs from the TUI.
- Any new HTTP endpoint (this is a TUI-only feature).

## Background

Key types and APIs the implementation will use:

- `ragent_storage::activity_log::ActivityLog`
  - `open(path: &Path) -> Result<Self>` — opens or creates the log database.
  - `list_runs() -> Result<Vec<RunId>>` — all runs with at least one event.
  - `count(run_id) -> Result<u64>` — total events for a run.
  - `run_status(run_id) -> Result<RunStatus>` — derived lifecycle state.
  - `read_run(run_id) -> Result<Vec<ActivityEvent>>` — ordered events.
- `ragent_types::activity`
  - `RunId` — opaque identifier (string-backed).
  - `ActivityEvent { id, run_id, seq, schema_version, timestamp, kind }`.
  - `EventKind` — tagged union: `ModelMessage`, `ToolCall`, `ToolResult`,
    `PermissionDecision`, `Checkpoint`, `Termination`, `BranchOrigin`,
    `MutationRejected`, `Lifecycle`.
  - `RunStatus` — `Active`, `Interrupted`, `Completed`, `Unrecoverable`,
    `Rebuilding`, `RolledBack`.
  - `Projection` — derived state from `replay_run`.

The activity-log database is stored alongside the main ragent database under
the platform data directory (`dirs::data_dir().join("ragent")`). The exact
file name is an implementation detail of `ActivityLog::open`; `/alog config`
will surface the resolved path.

## Requirements

### FR-001 — Slash command registration (Ubiquitous)

The TUI **shall** register a slash command named `alog` in the
`SLASH_COMMANDS` table so that it appears in the `/` autocomplete menu with a
short description.

### FR-002 — Command dispatch (Ubiquitous)

The TUI **shall** dispatch `/alog` and its subcommands (`help`, `config`,
`list`, `status`) from the central slash-command match in
`execute_slash_command_inner`, consistent with how `/editlog` and `/telemetry`
are dispatched.

### FR-003 — Help subcommand (State-driven)

When the user enters `/alog help` (or `/alog` with no subcommand), the TUI
**shall** display a help table listing every `/alog` subcommand and its
description, and set the status bar to `alog: help`.

### FR-004 — Config subcommand (Ubiquitous)

The TUI **shall** provide `/alog config`, which displays:

- the resolved path of the activity-log database file,
- whether the database file currently exists on disk,
- the SQLite journal mode (e.g. WAL),
- the schema version constant `ACTIVITY_EVENT_SCHEMA_VERSION`, and
- the location convention (platform data directory).

### FR-005 — List subcommand (Event-driven)

When the user enters `/alog list`, the TUI **shall** open the activity log at
the same data-directory path used by the main process and enumerate every run
returned by `ActivityLog::list_runs()`. For each run the display **shall**
include:

- the `RunId` (full or truncated to a stable short prefix),
- the derived `RunStatus` from `run_status()`,
- the total event count from `count()`, and
- a per-`EventKind` breakdown of event counts (model messages, tool calls,
  tool results, permission decisions, checkpoints, terminations,
  branch-origin, mutation-rejected, lifecycle).

If there are no runs, the TUI **shall** display a message stating the log is
empty.

### FR-006 — Status subcommand (State-driven)

When the user enters `/alog status`, the TUI **shall** display an overall
health summary of the activity-log system, including:

- whether the database file is openable,
- total number of runs,
- total number of events across all runs,
- count of runs by `RunStatus` (active, completed, interrupted,
  unrecoverable, rebuilding, rolled-back),
- the current `ACTIVITY_EVENT_SCHEMA_VERSION`, and
- a short one-line indication of whether the subsystem is healthy.

### FR-007 — Output rendering (Ubiquitous)

All `/alog` output **shall** be rendered as assistant-prefixed markdown text
via `append_assistant_text` and **shall** set the status bar to a short
`alog: <subcommand>` string, mirroring the conventions used by `/editlog` and
`/telemetry`.

### FR-008 — Error handling (Unwanted)

If the activity-log database cannot be opened, or any storage query fails,
the TUI **shall** display a user-visible warning message containing the error
detail and **shall not** crash or leave the TUI in an inconsistent state.
Storage operations **shall** be performed on a blocking thread to avoid
blocking the async event loop.

### FR-009 — Autocomplete suggestions (Optional)

The slash-command autocomplete **may** suggest `help`, `config`, `list`, and
`status` as subcommand completions for `/alog`, consistent with how `/editlog`
and `/mouse` provide subcommand suggestions.

### FR-010 — Read-only guarantee (Unwanted)

The `/alog` command **shall not** perform any write, mutation, expiry, or
archive operation against the activity log. Any code path that would mutate
the log **shall** be rejected during implementation review.

### FR-011 — Delete subcommand (Event-driven)

When the user enters `/alog delete <run-id> --yes`, the TUI **shall** delete
all activity-log events belonging to the specified `RunId` from the
activity-log database. The `--yes` flag is mandatory; if it is omitted the
TUI **shall not** perform the deletion and **shall** instead display a
warning instructing the user to add `--yes` to confirm the destructive
operation.

### FR-012 — Delete argument validation (State-driven)

When the user enters `/alog delete` without a `<run-id>` argument, the TUI
**shall** display a usage error message showing the expected syntax
(`/alog delete <run-id> --yes`) and **shall not** attempt any storage
operation.

### FR-013 — Delete unknown run (Unwanted)

If the supplied `<run-id>` does not correspond to any run in the activity
log (i.e. it is not returned by `ActivityLog::list_runs()`), the TUI
**shall** display a warning stating that no run with that identifier was
found and **shall not** perform any deletion.

### FR-014 — Delete confirmation output (Ubiquitous)

After a successful deletion, the TUI **shall** display an assistant-prefixed
message stating the `RunId` that was deleted and the number of events removed,
and **shall** set the status bar to `alog: deleted`.

### FR-015 — Delete storage operation (Ubiquitous)

The deletion **shall** be performed via a storage call that removes all
events for the given `RunId` (e.g. `ActivityLog::expire_run` or an equivalent
delete-all query). The operation **shall** be executed on a blocking thread
to avoid blocking the async event loop, consistent with FR-008.

### FR-016 — Delete help entry (Ubiquitous)

The `/alog help` table **shall** include a row for the `delete` subcommand
describing its syntax (`/alog delete <run-id> --yes`) and noting that the
`--yes` flag is required.

### FR-017 — Delete autocomplete suggestion (Optional)

The slash-command autocomplete **may** suggest `delete` as a subcommand
completion for `/alog`, alongside `help`, `config`, `list`, and `status`.

### FR-018 — Export subcommand (Event-driven)

When the user enters `/alog export <run-id> --yes`, the TUI **shall** export
all activity-log events belonging to the specified `RunId` as JSON Lines to
the file `log/exports/export-<run-id>.jsonl` relative to the current working
directory. The `--yes` flag is mandatory; if it is omitted the TUI **shall
not** perform the export and **shall** instead display a warning instructing
the user to add `--yes` to confirm the operation.

### FR-019 — Export argument validation (State-driven)

When the user enters `/alog export` without a `<run-id>` argument, the TUI
**shall** display a usage error message showing the expected syntax
(`/alog export <run-id> --yes`) and **shall not** attempt any storage
operation.

### FR-020 — Export unknown run (Unwanted)

If the supplied `<run-id>` does not correspond to any run in the activity
log (i.e. it is not returned by `ActivityLog::list_runs()`), the TUI
**shall** display a warning stating that no run with that identifier was
found and **shall not** perform any export.

### FR-021 — Export file location and format (Ubiquitous)

The export **shall** be written to the path `log/exports/export-<run-id>.jsonl`
relative to the current working directory. The `log/exports/` directory
**shall** be created if it does not already exist. The file format **shall**
be JSON Lines — one `ActivityEvent` serialised as a standalone JSON object
per line, in ascending sequence-number order — using the existing
`ActivityLog::export_jsonl` API.

### FR-022 — Export confirmation output (Ubiquitous)

After a successful export, the TUI **shall** display an assistant-prefixed
message stating the `RunId` that was exported, the number of events written,
and the filesystem path of the exported file, and **shall** set the status bar
to `alog: exported`.

### FR-023 — Export storage operation (Ubiquitous)

The export **shall** be performed via `ActivityLog::export_jsonl` (or the
equivalent `export_jsonl_to` writer API) and **shall** be executed on a
blocking thread to avoid blocking the async event loop, consistent with
FR-008.

### FR-024 — Export help entry (Ubiquitous)

The `/alog help` table **shall** include a row for the `export` subcommand
describing its syntax (`/alog export <run-id> --yes`) and noting that the
`--yes` flag is required and that the output file is written to
`log/exports/export-<run-id>.jsonl`.

### FR-025 — Export autocomplete suggestion (Optional)

The slash-command autocomplete **may** suggest `export` as a subcommand
completion for `/alog`, alongside `help`, `config`, `list`, `status`, and
`delete`.

## Non-Functional Requirements

### NFR-001 — Delete safety guard (Unwanted)

The `/alog delete` implementation **shall not** allow deletion without the
explicit `--yes` flag under any code path, including programmatic or
autopilot-driven invocations. A missing `--yes` flag **shall always** abort
the operation before any storage access.

### NFR-002 — Delete irreversibility warning (Unwanted)

The deletion performed by `/alog delete` is irreversible. The help text and
the missing-`--yes` warning message **shall** state clearly that deleted
runs cannot be recovered from the activity log.

### NFR-003 — Export safety guard (Unwanted)

The `/alog export` implementation **shall not** perform the export without
the explicit `--yes` flag under any code path, including programmatic or
autopilot-driven invocations. A missing `--yes` flag **shall always** abort
the operation before any storage access or file write.

### NFR-004 — Export non-destructive guarantee (Unwanted)

The `/alog export` operation **shall not** mutate, delete, or expire any
events in the activity log. The export is a read-only copy of the run's
events written to a new file; the original run data **shall** remain intact
in the database.