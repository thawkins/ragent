# /profile

> Toggle the agent-loop profiler panel (/profile on|off|help)

## Overview

`/profile` toggles the agent-loop profiler side panel, which reads
`AgentLoopProfiler::snapshot()` and displays the recorded action-loop timing
samples. The profiler must be enabled before `/actionloop` has data to
report.

## Syntax

```
/profile            Print the help table (same as /profile help)
/profile on         Enable the profiler panel
/profile off        Disable the profiler panel
/profile help       Print the help table
```

## Options / Subcommands

| Form | Description |
| ---- | ----------- |
| `on` | Enable the profiler panel via `set_profile_panel_enabled(true)`. |
| `off` | Disable the profiler panel. |
| `help` | Print the help table, which notes that it reads `AgentLoopProfiler::snapshot()` and that the alias `/perf` behaves identically. |

`--help` and `-h` are accepted as aliases of `help`; the bare form also
prints help. Any other argument prints the usage text.

## Examples

```
/profile on
```
Opens the profiler side panel and starts collecting action-loop timing data.

```
/profile off
```
Closes the profiler side panel.

```
/profile help
```
Prints the help table including the alias note.

```
/profile on
/actionloop
```
Enable profiling, then read the timing report.

## Output

`on`/`off` confirm the panel state change. `help` prints the table that cites
the profiler snapshot source and mentions the `/perf` alias. Unknown
arguments print the usage line.

## Related

- `/perf` -- full alias of `/profile` with identical semantics.
- `/actionloop` -- prints the timing report the profiler records.