# /swarm

> Auto-decompose a goal into parallel subtasks (/swarm <prompt> | /swarm status | /swarm help)

## Overview

`/swarm` takes a goal prompt and asks the configured model to decompose it
into parallel subtasks that are then executed by teammates. It is the quickest
way to fan a large piece of work out across a team without hand-writing the
task list: the model proposes the decomposition, and the swarm runtime drives
the subtasks from there.

An optional `--agent <type>` prefix sets the default agent type used for the
decomposed subtasks.

## Syntax

```
/swarm <prompt>
/swarm --agent <type> <prompt>
/swarm status
/swarm cancel
/swarm help
```

## Options / Subcommands

| Form | Description |
|------|-------------|
| `/swarm <prompt>` | Decomposes the prompt into subtasks and starts the swarm. |
| `--agent <type>` | Optional prefix flag; sets the default agent type for the subtasks. |
| `status` | Shows current swarm progress. |
| `cancel` | Cancels the active swarm. |
| `help` | Prints usage. |

## Guards

- Starting a second swarm while one is active is refused:
  `[warn] A swarm is already active - use /swarm cancel first`
- Starting a swarm without a configured model is refused:
  `[warn] /swarm requires a configured model - use /model`

## Examples

```
/swarm Refactor the config parser and update every caller in the tests
```

```
/swarm --agent coder Split the migration into per-crate tasks and run them
```

```
/swarm status
```

```
/swarm cancel
```

```
/swarm help
```

## Output

- On start, the status line shows `[wait] swarm: decomposing goal...` while
  the decomposition request is in flight.
- Decomposition is an asynchronous LLM call: the command returns immediately
  and the decomposed subtasks are reported as the model produces them.
- `/swarm status` reports the running swarm's progress.
- `/swarm cancel` stops the active swarm, after which a new `/swarm` can be
  started.

## Notes

- A model must be selected before a swarm can start (`/model`).
- The decomposed subtasks run as teammates; use `/team tasks` to watch the
  shared task list fill up, and `/swarm status` for swarm-level progress.
- Only one swarm may be active at a time.

## Related

- `/team` -- team lifecycle commands for the teammates a swarm creates
- `/blueprints` -- list installed team blueprints
- `/model` -- required before a swarm can start
- docs/howtos/teams.md -- teams and swarm how-to manual