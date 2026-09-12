# /task

> Toggle the TASKS side panel, or list/help tasks: /task [list|help]

## Overview

`/task` has two roles. With no arguments it toggles the TASKS side panel on or
off; showing the panel hides the log, profile, memory, and telemetry panels so
only one side panel is visible at a time. With arguments it operates on session
tasks: `list` renders the task table, `add`/`create`/`update`/`get` route to
the `task_create`, `task_update`, and `task_get` tools.

## Syntax

```
/task
/task list [status]
/task add <subject>
/task create <subject>
/task update <id> <status>
/task get <id>
/task help
```

## Options / Subcommands

| Form | Description |
|------|-------------|
| (bare) | Toggle the Tasks side panel |
| `list [status]` | Render the task table, optionally filtered by status |
| `add <subject>` | Create a task via `task_create` |
| `create <subject>` | Alias for `add` |
| `update <id> <status>` | Change a task's status via `task_update` |
| `get <id>` | Inspect one task via `task_get` |
| `help` | Show this subcommand table |

Valid statuses for `update` and the `list` filter are the session tracker's
`pending`, `in_progress`, and `completed`.

## Examples

```
From: /task
tasks panel visible
(The panel opens; a second bare /task hides it.)
```

```
From: /task list
| ID | Status | Subject |
| T-1 | [wait] pending | Write migration script |
| T-2 | [sync] in_progress | Update SPEC.md |
```

```
From: /task list completed
Only completed tasks are shown.
```

```
From: /task add Update the changelog
A new session task is created via task_create and appears in /task list.
```

```
From: /task update T-1 completed
task_update marks T-1 completed; dependent tasks are re-evaluated.
```

```
From: /task get T-1
task_get returns the full record: description, owner, blocked_by.
```

## Output

The bare form prints `tasks panel visible` or `tasks panel hidden`. `list`
renders the task table with status icons `[wait]` pending, `[sync]` in
progress, `[ok]` completed. The add/update/get forms show the tool's result
in the message window.

## Related

- `task_create`, `task_update`, `task_get`, `task_list` tools
- Spec tracking: `spec_task_update` mirrors into the same session tracker