# /status

> Show status message history: /status [clear]

## Overview

`/status` shows the history of status-line messages recorded in this TUI
session, newest first, or clears it. The history keeps the most recent 100
messages (`StatusHistory` in `crates/ragent-tui/src/theme.rs`); messages are
recorded when a command sets a working/in-progress status.

## Syntax

```
/status
/status clear
```

## Options / Subcommands

| Form | Description |
| ---- | ----------- |
| `/status` | Lists the recorded status messages, newest first, each with its age in seconds. |
| `/status clear` | Empties the history. |

Any other argument prints a usage warning.

## Examples

```
/status
```
Shows the status history.

```
/status clear
```
Clears it.

## Output

`/status` prints a `## Status history (newest first)` list in the message
window, one `- <N>s ago: <message>` line per entry, or `No status messages
recorded yet.` when the history is empty. `/status clear` prints `Status
history cleared.`

## Related

- `/triggers status`, `/alog status`, `/editlog status` -- `status`
  subcommands of other commands.
