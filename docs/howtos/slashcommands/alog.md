# /alog

> Activity log: /alog help|on|off|config|list|status|delete <run-id> --yes|export <run-id> --yes

## Overview

`/alog` controls the activity log, which records each agent run as a JSONL
activity file. Use it to enable or disable recording, inspect past runs, and
-- with the mandatory `--yes` guard -- delete or export a specific run.

## Syntax

```
/alog                   Print the help table (same as /alog help)
/alog on                Enable activity logging
/alog off               Disable activity logging
/alog config            Show the activity-log configuration
/alog list              List recorded runs
/alog status            Show the activity-log runtime status
/alog delete <run-id> --yes   Delete one run (guard required)
/alog export <run-id> --yes   Export one run to a JSONL file (guard required)
/alog help              Print the help table
```

## Options / Subcommands

| Form | Description |
| ---- | ----------- |
| `on` | Enable activity logging via `persist_activity_log`. |
| `off` | Disable activity logging. |
| `config` | Show the current activity-log configuration. |
| `list` | List recorded runs. |
| `status` | Show the activity-log runtime status. |
| `delete <run-id> --yes` | Delete the run. `--yes` is mandatory: run-id and the guard are validated (FR-011/012/013/014/015) and the deletion runs on a blocking thread via `expire_run`. Confirmation reports the number of removed events. |
| `export <run-id> --yes` | Export the run to `log/exports/export-<run-id>.jsonl` (FR-018/019/020/021/022/023). `--yes` is mandatory here too. |
| `help` | Print the help table, including the current ON/OFF state. |

`--help` and `-h` are accepted as aliases of `help`. Unknown subcommands
print the usage text.

## Examples

```
/alog on
```
Starts recording run activity.

```
/alog list
```
Lists the recorded runs so a `run-id` can be copied for delete/export.

```
/alog status
```
Shows the runtime status of activity logging.

```
/alog delete 9c3f1b2a --yes
```
Deletes run `9c3f1b2a` after the `--yes` guard, confirming the number of
events removed.

```
/alog export 9c3f1b2a --yes
```
Writes `log/exports/export-9c3f1b2a.jsonl` containing that run's events.

```
/alog help
```
Prints the help table including the current ON/OFF state.

## Output

`on`/`off` confirm the state change (persist failures surface as warnings).
`config`, `list`, and `status` print their respective views. `delete` prints a
confirmation with the event count; without `--yes` the command is refused.
`export` prints the written path `log/exports/export-<run-id>.jsonl`; without
`--yes` it is refused. `help` prints the table with the live ON/OFF state.

## Related

- `/log clear research` clears `logs/research/`, a sibling log root.
- `/editlog` -- the separate edit-operation log.
- `log/exports/` -- destination for `/alog export`.