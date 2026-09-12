# /name

> "Set a human-readable display name for the session: /name <display-name> | /name help" (SlashCommandDef, crates/ragent-tui/src/app/state.rs).

## Overview

`/name` assigns a display name to the active session (FR-015). The name is
persisted in session metadata through the storage layer, so it appears in
session lists and survives restarts and resume operations. Calling `/name`
with no argument clears the existing name. Use it to label long-running work
sessions so they are easy to find later in `/session list`.

The handler is the `name` arm in `crates/ragent-tui/src/app/slash.rs`
(lines 5855-5913).

## Syntax

```
/name <display-name>
/name
/name help
```

## Options / Subcommands

| Form | Description |
|---|---|
| `/name <display-name>` | Set a human-readable name for the active session (persists in session metadata) |
| `/name` | Clear the session name |
| `/name help` | Show the help table |

## Examples

### Name the session

```
/name compaction-benchmarks
```

Status line:

```
Session name set to 'compaction-benchmarks'
```

Log entry: `Session name set to 'compaction-benchmarks'`.

### Clear the session name

```
/name
```

Status line:

```
Session name cleared
```

Log entry: `Session name cleared`.

### Show help

```
/name help
```

Output:

```
From: /name help

## /name - Session display name

| Subcommand | Description |
|---|---|
| `/name <display-name>` | Set a human-readable name for the active session (persists in session metadata) |
| `/name` | Clear the session name |
| `/name help` | Show this help |
```

### Storage failure

If the underlying storage update fails, the status line shows
`[warn] Failed to set session name: <error>` (or
`[warn] Failed to clear session name: <error>`) and the log records the error.

## Output

- Success (`name set`): status `Session name set to '<name>'` plus an Info log
  entry.
- Success (`name cleared`): status `Session name cleared` plus an Info log
  entry.
- No active session: status `[warn] No active session to name`.
- Storage error: `[warn] Failed to set/clear session name: <error>` plus an
  Error log entry with the same text.

The name is written through `storage.update_session(&session_id, name)`; an
empty name argument is the clear path and passes an empty string.

## Related

- `/session list` - see named sessions in the list
- `/session resume` - resume a named session by id
- `/quit` - leave the session (the name persists for next time)