# /perf

> Alias for /profile - toggle the agent-loop perf panel (/perf on|off|help)

## Overview

`/perf` is an alias of `/profile`. It is implemented as a full duplicate
dispatch arm with identical `on` / `off` / `help` semantics, and its own help
text states that it is the alias form. Both commands toggle the same
agent-loop profiler panel.

## Syntax

```
/perf               Print the help table (same as /perf help)
/perf on            Enable the profiler panel
/perf off           Disable the profiler panel
/perf help          Print the help table
```

## Options / Subcommands

| Form | Description |
| ---- | ----------- |
| `on` | Enable the profiler panel (identical to `/profile on`). |
| `off` | Disable the profiler panel (identical to `/profile off`). |
| `help` | Print the help table, which notes "(alias of /profile)". |

`--help` and `-h` are accepted as aliases of `help`; the bare form also
prints help. Any other argument prints the usage text.

## Examples

```
/perf on
```
Enables the profiler panel - exactly what `/profile on` does.

```
/perf off
```
Disables the profiler panel.

```
/perf help
```
Prints the help table noting the alias relationship.

```
/perf on
/actionloop
```
The alias works anywhere `/profile` does in the enable-then-report sequence.

## Output

Identical to `/profile`: `on`/`off` confirm the panel state change and `help`
prints the table noting that `/perf` is an alias of `/profile`.

## Related

- `/profile` -- the canonical command; `/perf` behaves identically.
- `/actionloop` -- prints the timing report the profiler records.