# /inputdiag

> "Dump current input state diagnostics" (SlashCommandDef, crates/ragent-tui/src/app/state.rs).

## Overview

`/inputdiag` prints a snapshot of the terminal UI's input-related state for
troubleshooting input, selection, menu, and pane-layout issues. It takes no
arguments and performs no state changes; it only reads and reports. The
snapshot covers the current screen, the input buffer and cursor, all overlay
menus, text selection, pane geometry, and the `@` file browse cache.

The handler is the `inputdiag` arm in `crates/ragent-tui/src/app/slash.rs`
(approximately lines 3160-3217).

## Syntax

```
/inputdiag
```

No arguments. Additional text after the command is ignored.

## Options / Subcommands

None. `/inputdiag` takes no forms.

## Examples

### Dump diagnostics

```
/inputdiag
```

Output (representative, condensed):

```
From: /inputdiag
screen:           App
input chars:      42
cursor index:     42
slash menu:       inactive
file menu:        inactive
history picker:   inactive
text selection:   none
context menu:     none
message area:     Rect { x: 0, y: 0, w: 100, h: 30 }
log area:         Rect { x: 0, y: 30, w: 100, h: 10 }
input area:       Rect { x: 0, y: 40, w: 100, h: 3 }
@ browse cache:   cwd=/home/user/Projects/ragent entries=0 refreshed=never
  menu state:     query="" dir="" selected=0 offset=0 results=[]
```

### After selecting text

With an active selection the `text selection` line reports the pane and the
anchor-to-endpoint span instead of `none`; when a `@` browse menu is open the
cache block reports the live query, current directory, selected index, scroll
offset, and result count.

## Output

Fields reported by `/inputdiag`:

- current screen variant (which TUI screen is active),
- input buffer character count and cursor index,
- slash-menu, file-menu, and history-picker active flags,
- text-selection state (`pane, anchor->endpoint` or `none`),
- context-menu state,
- message, log, and input area `Rect` geometry,
- `@` browse cache: current working directory, entry count,
  refreshed-at timestamp, and menu state (query, directory, selected
  index, scroll offset, result list).

The status line shows `inputdiag`.

## Related

- `/context` - clear the prompt context cache
- `/debug` - troubleshoot the session from debug logs
- `/startup` - show startup timing report