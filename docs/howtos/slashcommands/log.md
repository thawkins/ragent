# /log

> Log panel: /log [clear subagents|panics|research|editlog|help]

## Overview

`/log` controls the log side panel and the on-disk log files. Invoked with no
arguments it toggles the log panel: other side panels (profile, tasks, memory,
telemetry) are dismissed and the buffered log-window history is spooled into
the panel. With the `clear` subcommand it empties one of the five log
directories that the agent writes to under `log/`.

## Syntax

```
/log                Toggle the log side panel
/log clear <name>   Clear one log directory
/log help           Print the usage table
```

## Options / Subcommands

| Form | Description |
| ---- | ----------- |
| (bare) | Toggle the log side panel on/off. |
| `clear subagents` | Empty `log/subagents/` (background agent reports). |
| `clear panics` | Empty `log/panics/`. |
| `clear research` | Empty `logs/research/` (note the plural top-level directory). |
| `clear editlog` | Empty `log/editlog/`. |
| `clear logwindow` | Empty `log/logwindow/`. |
| `help` | Print the usage table. |

Unknown subcommands print the usage text.

## Examples

```
/log
```
Toggles the log panel. If it was closed it opens and the panel history is
spooled; if it was open it closes.

```
/log clear subagents
```
Empties `log/subagents/` so old background-agent reports no longer consume
disk.

```
/log clear research
```
Empties `logs/research/`. This is the only clear target that maps to the
plural `logs/` root rather than `log/`.

```
/log clear editlog
```
Empties `log/editlog/`. The files are kept (emptied), so subsequent edit-log
stats in `/editlog show` restart from zero.

```
/log clear logwindow
```
Empties `log/logwindow/`, removing accumulated TUI log-window session logs.

```
/log help
```
Prints the subcommand table shown above.

## Output

Bare invocation shows or hides the log side panel; when opening, any other
side panel (profile, tasks, memory, telemetry) is dismissed and the buffered
log-window history is spooled into the panel. `clear` reports which directory
was emptied. `help` prints the table of valid clear targets.

## Related

- `/editlog` -- edit-operation statistics (the `editlog` clear target).
- `/profile`, `/memory`, `/telemetry` -- side panels dismissed when
  the log panel opens.
- `log/` directory layout: `log/subagents/`, `log/panics/`, `log/editlog/`,
  `log/logwindow/`, and `logs/research/`.