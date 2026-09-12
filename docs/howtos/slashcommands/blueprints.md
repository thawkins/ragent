# /blueprints

> List installed team blueprints: /blueprints help|list

## Overview

Team blueprints are installed team templates: named recipes that define the
shape of a team so it can be started with a single `/team create <blueprint>`
call. The `/blueprints` command is the read-only companion to team creation --
it shows which blueprints are installed, so the name passed to `/team create`
is always one that exists.

The command has exactly two forms: `help` and `list`. It makes no changes to
any team or configuration.

## Syntax

```
/blueprints help
/blueprints list
```

## Options / Subcommands

| Form | Description |
|------|-------------|
| `help` | Prints the `/blueprints` usage summary. |
| `list` | Lists the installed team blueprints. |

## Examples

```
/blueprints help
```

```
/blueprints list
```

```
/blueprints list
/team create code-review
```

Lists the installed blueprints, then creates a team from one of the listed
names.

```
/team blueprint
```

`/team blueprint [name]` shows or selects the blueprint for the active team;
`/blueprints list` is the catalogue-level view across all installed
blueprints.

## Output

- `/blueprints help` prints the usage line for the command.
- `/blueprints list` prints the installed team blueprints. Every name in the
  list is a valid `<blueprint>` value for `/team create <blueprint> [name]`.

## Choosing a blueprint

- Pick a blueprint from `/blueprints list` that matches the work: a review
  blueprint for audit-style passes, a build-style blueprint for
  implementation work.
- `/team create <blueprint>` starts the team; adding a name
  (`/team create <blueprint> <name>`) controls the team name, otherwise it is
  auto-generated as `<blueprint>-<YYYYMMDD-HH-MM-SS>` (UTC).
- `/team blueprint [name]` shows or switches the blueprint of the active team
  after creation.

## Related

- `/team create <blueprint> [name]` -- create a team from a listed blueprint
- `/team blueprint [name]` -- show or select the active team's blueprint
- `/team` -- full team command surface
- docs/userdocs/TEAMS.md -- teams user guide
- docs/howtos/teams.md -- teams how-to manual