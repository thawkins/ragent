# /undo

> "Remove the last user/assistant turn pair from the conversation (/undo help)" (SlashCommandDef, crates/ragent-tui/src/app/state.rs).

## Overview

`/undo` rolls the conversation back one turn: it finds the most recent user
message, truncates that message and everything after it (typically the single
assistant response that followed), and resets the scroll position to the new
end of the conversation. The status bar reports how many messages were
removed. It only edits the in-memory conversation; it does not rewrite stored
session history.

The handler is the `undo` arm in `crates/ragent-tui/src/app/slash.rs`
(lines 5796-5851).

## Syntax

```
/undo
/undo help
```

Any other argument is ignored; the command proceeds as bare `/undo`.

## Options / Subcommands

| Form | Description |
|---|---|
| `/undo` | Remove the last user message and everything after it (typically one assistant response) from the conversation |
| `/undo help` | Show the help table |

## Examples

### Remove the last turn

```
/undo
```

Status line:

```
Undid last turn (removed 2 message(s))
```

Log entry: `Undo: removed 2 message(s) from end of conversation`. The
transcript now ends at the previous user message.

### Show help

```
/undo help
```

Output:

```
From: /undo help

## /undo - Remove the last turn

| Subcommand | Description |
|---|---|
| `/undo` | Remove the last user message and everything after it (typically one assistant response) from the conversation |
| `/undo help` | Show this help |
```

### No active session

```
/undo
```

Status line:

```
[warn] No active session to undo
```

### Empty conversation

```
/undo
```

Status line:

```
[warn] No messages to undo
```

### No user message in the transcript

When the conversation contains no `User` role message at all, the status line
shows `[warn] No user message found to undo` and the log records
`Undo: no user message found in conversation`.

## Output

- On success the last user message and all following messages (assistant
  replies, tool transcripts, compaction markers) are removed in one
  `truncate`; the count of removed messages is reported in the status line.
- The scroll offset is reset to 0 so the end of the trimmed conversation is
  visible.
- Guards in order: no session, empty message list, no user message found -
  each emits a `[warn]` status and leaves the conversation untouched.

## Related

- `/clear` - remove the whole conversation
- `/compact` - summarise rather than delete history
- `/session resume` - restore a previously stored session