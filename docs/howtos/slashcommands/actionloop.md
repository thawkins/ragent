# /actionloop

> Agent action-loop timing: /actionloop [help|clip]

## Overview

`/actionloop` prints timing information for the agent action loop -- the
per-action durations collected by the agent-loop profiler. The bare form
prints the report in the message window; `clip` copies the same report to the
system clipboard.

## Syntax

```
/actionloop            Print the action-loop timing report
/actionloop clip       Copy the timing report to the clipboard
/actionloop help       Print the help table
```

## Options / Subcommands

| Form | Description |
| ---- | ----------- |
| (bare) | Print the timing report, or the no-samples message when profiling has not recorded anything yet. |
| `clip` | Copy the `actionloop_report()` text to the clipboard. With no recorded samples it prints the no-samples message instead of copying. |
| `help` | Print the help table. |

`--help` and `-h` are accepted as aliases of `help`.

## Examples

```
/actionloop
```
Prints the current timing report. If no samples were recorded it prints the
reminder: enable `/profile on` and run the agent.

```
/actionloop clip
```
Copies the report so it can be pasted into a bug report or review.

```
/actionloop help
```
Prints the table above.

```
/profile on
/actionloop
```
Typical sequence: enable the profiler panel first so the action loop records
samples, then print the report.

## Output

Bare form prints the timing report when samples exist; otherwise it prints
"Enable `/profile on` and run the agent". `clip` copies the report and
confirms; with no samples it prints the same enable-profiling reminder.

## Related

- `/profile` -- enable/disable the profiler panel that feeds this report.
- `/perf` -- alias of `/profile`.