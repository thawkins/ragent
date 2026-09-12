# /goal

> Set or clear the goal-driven loop goal (/goal set|clear|show|test)

## Overview

`/goal` manages the goal used by the goal-driven agentic loop. In this build
`/goal set` and `/goal clear` only print confirmations - the goal is not yet
persisted to session storage - and `/goal show` and `/goal test` always report
that no goal is set. See the limitations note below before relying on this
command.

## Syntax

```
/goal                  Show the usage block
/goal set <description>  Set the goal description
/goal clear            Clear the goal
/goal show             Show the current goal
/goal test             Test the goal condition
/goal help             Show the usage block
```

## Options / Subcommands

| Form | Description |
|------|-------------|
| `/goal` | Alias of the help form; prints the usage block, including the example goal and the note that the goal is evaluated after each turn |
| `/goal set <description>` | Builds a goal condition from the description and prints a confirmation |
| `/goal clear` | Prints a confirmation that the goal was cleared |
| `/goal show` | Prints the current goal state |
| `/goal test` | Evaluates the goal condition |
| `/goal help` | Alias of the bare form |

Unknown subcommands print `Usage: /goal set|clear|show|test|help`.

## Examples

```
/goal set Stop when all tests pass and the build succeeds
```
The example goal from the usage block; prints a confirmation.

```
/goal set Remove all compiler warnings in crates/ragent-config
```
Sets a custom goal description and prints the confirmation.

```
/goal clear
```
Prints the confirmation that the goal was cleared.

```
/goal show
```
Prints `No goal is currently set.`

```
/goal test
```
Prints `No goal is currently set to test.`

## Output

- Bare form and `help`: the usage block with the example
  `/goal set Stop when all tests pass and the build succeeds`.
- `set` / `clear`: confirmation messages only.
- `show`: always prints `No goal is currently set.` in this build.
- `test`: always prints `No goal is currently set to test.` in this build.

## Current limitations

- `/goal set` and `/goal clear` are confirmations only. The goal is **not**
  persisted to session storage in this build, so it does not survive a restart
  and is not consumed by the loop yet.
- Consequently `/goal show` and `/goal test` always report that no goal is set,
  regardless of whether a `/goal set` was issued earlier in the session.

## Related

- `/loop` - the goal-driven agentic loop with verification gates
- Goal conditions are defined by `GoalCondition` in the `ragent-agent` crate