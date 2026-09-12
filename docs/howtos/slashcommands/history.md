# /history

> Browse and re-use previous inputs; /history [filter] restricts to matching entries (arrow keys to select, Enter to insert); /history help

## Overview

`/history` opens an interactive picker over the inputs you have typed this
session, newest first. Use the up/down arrow keys to move the selection and
Enter to insert the chosen entry back into the input box for editing and
resubmission. An optional argument filters the list to entries containing that
substring (case-insensitive).

If there is no input history yet, or the filter matches nothing, the status bar
reports it and no picker opens. `/history help` prints the subcommand table.

## Syntax

```
/history              # open the history picker (newest first)
/history <filter>     # restrict the picker to matching entries
/history help         # show the subcommand table
```

## Options / Subcommands

| Form | Description |
| --- | --- |
| `/history` | Open the history picker (newest first; up/down to select, Enter to insert) |
| `/history <filter>` | Restrict the picker to entries containing `<filter>` |
| `/history help` | Show the `/history` help table |

Notes:

- Filtering is a case-insensitive substring match on the entry text; the
  argument itself is lowercased before matching.
- Opening the picker clears the current input line and resets the cursor, so
  Enter cleanly replaces whatever was being typed.
- Entries are shown in reverse chronological order (newest first) and the
  picker starts with the newest entry selected.

## Examples

```
/history
```

```
/history cargo
```

Only inputs containing `cargo` are listed (e.g. past prompts that mentioned
cargo commands).

```
/cancel help
```

(illustrative `/help`-style subcommand table printed by `/history help`)

```
## /history - Input history

| Subcommand | Description |
|---|---|
| `/history` | Open the history picker (newest first; up/down to select, Enter to insert) |
| `/history <filter>` | Restrict the picker to entries containing `<filter>` |
| `/history help` | Show this help |
```

Empty history (status bar, no picker):

```
No input history yet
```

Filter with no matches:

```
No history entries contain "zzz"
```

## Output

- Picker opened: the input line is cleared and the history picker overlay is
  shown with the newest matching entry selected; no assistant bubble is
  emitted for the open action.
- `/history help`: assistant bubble `From: /history help` with the subcommand
  table; status shows `history: help`.
- Empty history: status bar `No input history yet`.
- No filter matches: status bar `No history entries contain "<filter>"`.

## Related

- `/clear`  -  wipe the displayed message history (input history is separate)
- `/session`  -  resume stored sessions
- The picker supports text selection and clipboard copy of entries