# /team

> Team management (/team help|status|show [name]|create/open/delete <name>|close|message <id> <text>|tasks|clear|cleanup)

## Overview

`/team` drives multi-agent team coordination from the TUI: create a team from
a blueprint, check team and task status, send mailbox messages to teammates,
inspect the shared task list, and tear the team down when finished. Teammates
themselves are spawned and coordinated with the `team_*` tools; the `/team`
commands manage the surrounding lifecycle.

`/teams` is an alias of `/team`: both triggers dispatch to the same handler
and accept the same forms (for example `/teams show <name>`). There is no
separate `/teams` command surface.

## Syntax

```
/team
/team help
/team status
/team show [name]
/team create <blueprint> [name]
/team close
/team delete <name>
/team blueprint [name]
/team message <teammate-name> <text>
/team tasks
/team clear
/team cleanup
/team focus [name]
```

## Options / Subcommands

| Form | Description |
|------|-------------|
| `/team` (bare) | Shows the current team status (equivalent to `/team status`). |
| `help` | Prints the help table. |
| `status` | Team and task status summary for the active team. |
| `show [name]` | Shows team details; an optional team name may be given. |
| `create <blueprint> [name]` | Creates a team from a blueprint. Without `[name]` the team is auto-named `<blueprint>-<YYYYMMDD-HH-MM-SS>` using the current UTC time. |
| `close` | Closes the active team. |
| `delete <name>` | Deletes the named team's state. |
| `blueprint [name]` | Shows or selects the team blueprint. |
| `message <teammate-name> <text>` | Sends a mailbox message to the named teammate. |
| `tasks` | Lists the shared team task list. |
| `clear` | Clears team state. |
| `cleanup` | Tears the team down after teammates have stopped. |
| `focus [name]` | Focuses the named team; with no argument, clears the current focus. |

## Examples

```
/team
```

```
/team create code-review
```

```
/team create code-review review-2026-09
```

```
/team message reviewer Please start with the highest-priority task
```

```
/team tasks
```

```
/team cleanup
```

## Output

- Bare `/team` and `/team status` print the team and task status summary.
- `/team create` reports the created team; with no name supplied the generated
  name is `<blueprint>-<YYYYMMDD-HH-MM-SS>` (UTC timestamp).
- `/team message` delivers the text to the named teammate's mailbox.
- `/team tasks` lists the shared task list; claim and complete individual
  tasks with the `team_task_claim` / `team_task_complete` tools.
- `/team focus` switches which team subsequent team commands address, or
  clears the focus when called without an argument.

## Workflow

A typical team session:

```
/team create <blueprint> [name]     # start the team
/team tasks                         # inspect the shared task list
/team message <teammate> <text>     # coordinate with a teammate
/team cleanup                       # tear down when finished
```

Teammates are added with the `team_spawn` tool and report back through the
same task and messaging tools the lead uses.

## Related

- `/blueprints` -- list installed team blueprints
- `/swarm` -- auto-decompose a goal into parallel subtasks
- `team_*` tools -- spawn, task, and messaging tools used inside a team
- docs/userdocs/TEAMS.md -- teams user guide
- docs/howtos/teams.md -- teams how-to manual