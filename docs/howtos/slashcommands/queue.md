# /queue

> Inspect the message input queue (SlashCommandDef, crates/ragent-tui/src/app/state.rs).

## Overview

`/queue` inspects and controls the **message input queue** (spec `inputqueue`).
While the primary agent is executing, the message-window input field stays
editable: each `Enter` appends the submission to a bounded FIFO queue instead
of rejecting it with `busy - wait for the current turn to finish`. The oldest
queued entry runs automatically at each turn boundary, and a two-digit counter
appears before the prompt while entries are pending.

Since the FR-017 amendment, **slash commands are queued the same way**: typing
`/status` (or any other slash command) while a turn is executing appends it to
the queue instead of being refused, and the queued command runs at the next turn
boundary — or immediately via `/queue next`. Because a synchronous command leaves
the turn boundary free, a run of consecutive queued commands executes
back-to-back rather than stalling behind the first. Bang commands (`!…`) and
teammate-targeted messages keep their existing busy refusal.

`/queue` is the prompt-side view of the same queue the `Alt+Q` queue-control
menu manipulates. It is an optional command: it never mutates the running turn,
and `clear`/`next` are safe to run at any time.

## Syntax

```
/queue
/queue list
/queue clear
/queue next
/queue help
```

A bare `/queue` behaves as `/queue list`.

## Options / Subcommands

| Form | Description |
|------|-------------|
| `list` (default) | List the queued entries in submission order (oldest first) with the queue depth |
| `clear` | Empty the queue immediately; the counter disappears |
| `next` | Dispatch the oldest queued entry now (deferred to the turn boundary while a turn is executing) |
| `help` | Show the sub-command table and the `Alt+Q` pointer |

## Examples

### List a non-empty queue

```
/queue list
```

```
From: /queue list

## Input Queue

1. refactor the parser
2. add tests for the new branch

**2 entries queued (oldest first).**
```

Status bar: `queue: 2 entries`.

### Empty queue

```
/queue list
```

```
From: /queue list

ℹ️  The input queue is empty.

Messages typed while the agent is executing are staged here and run in order.
```

Status bar: `queue: list empty`.

### Clear the queue

```
/queue clear
```

```
From: /queue clear
[clr] Cleared 2 entries from the input queue.
```

Status bar: `queue: cleared`.

### Run the oldest entry

```
/queue next
```

When no turn is executing this dispatches the oldest entry immediately
(`queue: next dispatched`); when a turn is still executing the action is
deferred to the turn boundary and reports `queue: next deferred` — use
`Alt+Q` → `Next` to stop the current turn and run it now.

## Output

`list` renders a numbered `## Input Queue` block ending with the count. `clear`
and `next` print a one-line `From: /queue …` confirmation. Unknown sub-commands
print a `[warn]` line and status `queue: unknown`. Statuses land in the status
bar (`queue: list empty`, `queue: cleared`, `queue: next deferred`, and so on).

## Related

- `Alt+Q`  -  the interactive queue-control menu (`Next` / `Stop` / `Clear` /
  `Show`); `Up`/`Down` move the highlight, `Enter` activates the row, and the
  `Show` row opens a scrollable queue-entry panel where `Enter` moves an entry
  toward the front and `Del` removes it
- `/history`  -  recall previously entered messages (a queued entry is added to
  history the moment you press `Enter`)
- `TUI-QUICKSTART.md` §4  -  the input queue and the ALT-Q menu
