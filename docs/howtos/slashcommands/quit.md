# /quit

> "Quit the application" (SlashCommandDef, crates/ragent-tui/src/app/state.rs).

## Overview

`/quit` terminates the terminal UI by setting the application's running flag
to false, which unwinds the main event loop and returns to the shell. `/exit`
is an alias handled by the same code path, so both commands behave identically
and either name can be used.

`/quit` and `/exit` are the only slash commands that bypass the
`ensure_session()` gate in the dispatcher: they run even when no session is
active, because exiting must always be possible. They take no arguments, have
no help form, and produce no message-window output.

The handler is the single `quit`/`exit` arm in
`crates/ragent-tui/src/app/slash.rs` (lines 3724-3726).

## Syntax

```
/quit
/exit
```

Both forms are exact aliases: the dispatcher matches `"quit" | "exit"` to one
arm that sets `is_running = false`.

## Options / Subcommands

None. `/quit` (and `/exit`) take no forms, arguments, or help variant.

## Examples

### Leave the TUI

```
/quit
```

The TUI closes and the shell prompt returns. No `From: /quit` block is
printed; the command acts by ending the event loop.

### Alias form

```
/exit
```

Identical behaviour to `/quit`.

### Quit without an active session

```
/quit
```

Because the session gate is bypassed, this works even in a freshly launched
TUI with no conversation started. Contrast with most other commands, which
report `[warn] No active session` when `ensure_session()` fails.

## Output

- No message-window output. The single effect is `is_running = false` on the
  app state, which the main loop observes and uses to shut down.
- The status bar does not receive a new entry; the terminal is restored
  directly.

Note: the Ctrl+C-then-Ctrl+D quit path is keyboard-driven and is not part of
this command's handler; it is documented in TUI-QUICKSTART instead.

## Related

- `/session resume` - return to a stored session in a future run
- `/name` - label the session before quitting so it is easy to find again
- TUI-QUICKSTART.md - keyboard quit bindings and terminal handling