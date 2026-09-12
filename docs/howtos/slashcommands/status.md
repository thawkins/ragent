# /status

> Show status message history: /status [clear]

## Overview

`/status` is registered in the slash-command registry (`SLASH_COMMANDS` in
`crates/ragent-tui/src/app/state.rs`) with the description above and an
autocomplete suggestion for `clear`. A `StatusHistory` backing type exists in
`crates/ragent-tui/src/theme.rs` (line ~525).

**However, no `/status` dispatch arm was found in the TUI command dispatcher**
(`crates/ragent-tui/src/app/slash.rs`) in the sections verified. The `status`
arms that do exist in that file are subcommands of other commands (`/alog
status`, `/editlog status`, `/github status`, `/gitlab status`, `/router
status`, `/triggers status`) -- not a top-level `/status` handler.

## Syntax

```
/status             (registered; no dispatch arm verified)
/status clear       (autocomplete suggestion; no dispatch arm verified)
```

## Options / Subcommands

| Form | Description |
| ---- | ----------- |
| `clear` | The only autocomplete suggestion registered (slash.rs ~216-218). No dispatch arm was found, so behaviour is not verified. |

## Examples

```
/status
```
Registered in autocomplete, but no dispatch arm was located in the verified
dispatcher sections. Typing `/status` completes the token; whether a handler
executes is not verified here.

```
/status clear
```
Offered as a suggestion; no dispatch arm was found.

## Output

Not verified. No handler was found, so the message-window behaviour of this
command cannot be stated honestly. Do not rely on documented output.

## Related

- `/triggers status`, `/alog status`, `/editlog status` -- real `status`
  subcommands of other commands.
- `StatusHistory` (`crates/ragent-tui/src/theme.rs`) -- the backing type.
- Recommendation: the dispatch arm is either missing or dead; add one or
  remove the command from `SLASH_COMMANDS`.