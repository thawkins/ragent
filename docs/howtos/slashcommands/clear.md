# /clear

> Clear message history for the current session

## Overview

`/clear` wipes the in-memory message history for the current session: the
message list, the rendered-line cache, the tool step-numbering maps, and the
scroll position are all reset, and the shared prompt-context cache is cleared
so the next turn is rebuilt from scratch. The session itself is not deleted  - 
stored history on disk is untouched; only what the TUI displays is reset.

The status bar confirms with `messages cleared` and an Info log entry records
the action.

## Syntax

```
/clear
```

## Options / Subcommands

| Form | Description |
| --- | --- |
| `/clear` | Clear the message window history for the current session |

No arguments or subcommands; any extra text is ignored.

## Examples

```
/clear
```

After running, the message window is empty and the status bar shows:

```
messages cleared
```

## Output

- The message window is emptied immediately (messages, line cache, scroll
  offset, and tool-step counters reset).
- The prompt-context cache is invalidated.
- Status bar shows `messages cleared`; an Info log entry
  `Message history cleared` is written.
- The session record and its persisted history are not affected.

## Notes

- `/clear` operates only on the TUI display layer. Sessions stored in SQLite
  keep their full transcript; resuming the session restores what was stored.
- Tool step numbering (the `1.`, `2.` step numbers shown next to tool calls)
  restarts from scratch after a clear, since the per-session step counters are
  reset along with the messages.
- The prompt-context cache shared with the agent layer is invalidated, so the
  next prompt is assembled without reference to the cleared messages.

## Related

- `/compact`  -  summarise long history into a compact form instead of clearing
- `/history`  -  browse previous inputs and re-insert one
- `/session`  -  manage stored sessions (list, resume, export)