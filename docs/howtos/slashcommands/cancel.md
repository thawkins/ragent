# /cancel

> Cancel a background task (/cancel <task_id_prefix> | /cancel help)

## Overview

`/cancel` cancels a running background task by ID prefix. It matches against
two registries: the active benchmark task (cancelling it raises the bench
cancel flag and asks the benchmark loop to stop cooperatively) and the active
sub-agent task list (the matched entry is removed from the list immediately).

A prefix match is used, so a short unique ID prefix is enough  -  the command
finds the first task whose ID starts with the given prefix. `/cancel help`
prints the subcommand table; an empty argument is a usage error.

## Syntax

```
/cancel <task_id_prefix>
/cancel help
```

## Options / Subcommands

| Form | Description |
| --- | --- |
| `/cancel <task_id_prefix>` | Cancel a running background task or benchmark by ID prefix |
| `/cancel help` | Show the `/cancel` help table |

Notes:

- Benchmark tasks: when the prefix matches `active_bench_task_id`, the bench
  cancellation flag is set and the status bar shows a wait marker; the
  benchmark stops between steps, not mid-step.
- Sub-agent tasks: the first `active_tasks` entry whose ID starts with the
  prefix is removed from the list.
- Prefix matching is `starts_with` on the raw ID; no case folding.

## Examples

```
/cancel a1b2c3
```

```
/cancel bench
```

Cancel a benchmark (status bar):

```
bench: cancellation requested
```

Cancel a sub-agent task (status bar shows first 8 ID chars):

```
Cancelled task a1b2c3d4 (coder)
```

```
/cancel help
```

Missing argument (status bar + warn log):

```
[warn] Please provide a task ID prefix: /cancel <id>
```

No match:

```
No task found with ID starting with 'zz'
```

## Output

- On benchmark cancel: status bar `bench: cancellation requested`, Info log
  `Benchmark cancellation requested for <prefix>`.
- On task cancel: status bar `Cancelled task <8-char id> (<agent>)`, Info log
  `Task cancelled: <8-char id>... (<agent>)`, and the entry is removed from
  the active task list.
- On no match: status bar `No task found with ID starting with '<prefix>'`
  plus a Warn log entry.
- `/cancel help` appends an assistant bubble `From: /cancel help` with the
  subcommand table; status shows `cancel: help`.

## Related

- `/bench`  -  run benchmarks that produce the cancellable bench task
- Background sub-agents are spawned with `new_agent` and listed in the task UI
- `/help`  -  list all available slash commands