# /cancel

> Request cancellation of a sub-agent task or a benchmark (/cancel <task_id_prefix> | /cancel help)

## Overview

`/cancel` asks a running task to stop, resolved by ID prefix. It targets two
places: the active benchmark task (cancelling it raises the bench cancel flag
and asks the benchmark loop to stop cooperatively) and the sub-agent tasks the
Agents window is tracking.

For a sub-agent task the command **delegates to the task registry**: it resolves
the prefix, then calls `AgentManager::cancel_agent` - the same method behind the
`cancel_agent` tool - and reports the *request*. It does not remove the row. The
task's entry stays `running` until its agent loop observes the cooperative
cancel flag at the next step boundary, and the Agents window retires the row when
the `SubagentCancelled` event (or the registry reconciliation) reports the
transition. That is deliberate: the window mirrors the registry, so a row removed
before the registry confirms would reappear on the next sweep and would make a
real cancellation indistinguishable from a cosmetic one.

A prefix is matched with `starts_with` on the raw ID, no case folding. Because a
prefix can match several tasks, the command refuses an ambiguous one and lists
the matches. `/cancel help` prints the subcommand table; an empty argument is a
usage error.

## Syntax

```
/cancel <task_id_prefix>
/cancel help
```

## Options / Subcommands

| Form | Description |
| --- | --- |
| `/cancel <task_id_prefix>` | Request cancellation of a sub-agent task or benchmark by ID prefix |
| `/cancel help` | Show the `/cancel` help table |

Notes:

- Benchmark tasks take precedence: when the prefix matches
  `active_bench_task_id`, the bench cancellation flag is set and the status bar
  shows a wait marker; the benchmark stops between steps, not mid-step.
- Sub-agent tasks: the prefix is matched against every task the Agents window is
  tracking. Exactly one match is forwarded to the registry; more than one is
  refused; none lists the live ids.
- "Live" means `running`, `suspended` or `terminating`. A `completed`, `failed`,
  `cancelled` or `timed_out` task is never offered as a cancel target.
- The task keeps running until its next step boundary, so the status bar reports
  a request, not an outcome.

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

Cancel a sub-agent task (status bar names the request):

```
Cancellation requested for a1b2c3d4 (coder)
```

```
/cancel help
```

Missing argument (status bar + warn log):

```
[warn] Please provide a task ID prefix: /cancel <id>
```

Ambiguous prefix (refused, every match listed):

```
[err] Ambiguous task prefix `explore-` - 2 tasks match.

explore- (explore, running), explore- (plan, running)
```

No match (the live ids are listed, not just the prefix):

```
[err] No live task matches `zzzz`.

Live tasks: a1b2c3d4 (coder), e5f6a7b8 (explore)
```

Already finished:

```
[warn] Task a1b2c3d4 (coder) is already `completed` - there is nothing to cancel.
```

## Output

- On benchmark cancel: status bar `bench: cancellation requested`, Info log
  `Benchmark cancellation requested for <prefix>`.
- On task cancel: status bar `Cancellation requested for <8-char id> (<agent>)`,
  Info log `[stop] Cancel requested for task <8-char id> (<agent>)` attributed to
  the child session, and an assistant bubble `From: /cancel <8-char id>` stating
  that the task keeps running until its next step boundary. The row is **not**
  removed; the Agents window updates when the registry reports the transition.
- On an ambiguous prefix: status bar `Ambiguous task prefix '<prefix>'`, a Warn
  log naming every match, and an assistant bubble listing them as
  `<short id> (<agent>, <status>)` asking for a longer prefix. Nothing is
  cancelled.
- On no match: status bar `No task matching '<prefix>'`, a Warn log, and an
  assistant bubble listing the live task ids (or saying there are none).
- On a finished task: status bar `Task <short id> is already <status>`, a Warn
  log, and an assistant bubble saying there is nothing to cancel.
- `/cancel help` appends an assistant bubble `From: /cancel help` with the
  subcommand table; status shows `cancel: help`.

## Related

- `/bench`  -  run benchmarks that produce the cancellable bench task
- Background sub-agents are spawned with `new_agent` and listed in the Agents
  window (`Alt+A`); `cancel_agent` is the tool-side equivalent of this command
- `/help`  -  list all available slash commands
