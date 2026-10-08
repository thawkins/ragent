# /tools
> Toggle tool visibility: /tools [github|gitlab|teams|agents|plan|codeindex|masterfetch] [on|off] | /tools help

## Overview

`/tools` has two behaviours. The bare form (and `/tools show`) prints the tool
visibility table - the current on/off state of every visibility switch. With a
switch name it reports that switch's state, and with a switch plus a state
word it toggles the visibility of that tool family for the session and saves
the choice to the active config source.

The implementation accepts seven switches: `github`, `gitlab`, `teams`,
`agents`, `plan`, `codeindex` and `masterfetch`. Toggling a switch rewrites
`tool_visibility` in the config source and invalidates the session
processor's config cache so the change takes effect immediately.

## Syntax

```
/tools                        # show the visibility table (same as: show)
/tools show                   # same as bare form
/tools <switch>               # read one switch's state
/tools <switch> on|off        # set one switch (enable/disable also accepted)
/tools help                   # usage help (aliases: --help, -h, usage)
```

## Options / Subcommands

| Form | Description |
| --- | --- |
| `/tools` | Render the tool visibility table |
| `/tools show` | Same as the bare form |
| `/tools <switch>` | Report whether `<switch>` is currently on or off |
| `/tools <switch> on` | Enable the switch (`enable` also accepted) |
| `/tools <switch> off` | Disable the switch (`disable` also accepted) |
| `/tools help` | Print usage help |

Switches (all seven accepted): `github`, `gitlab`, `teams`,
`agents`, `plan`, `codeindex`, `masterfetch`.

## Examples

Show the visibility table:

```
/tools
```

Read a single switch:

```
/tools teams
```

Turn the GitHub tool family off for this project:

```
/tools github off
```

Re-enable codeindex tools:

```
/tools codeindex on
```

Disable the masterfetch tool family:

```
/tools masterfetch off
```

Print the usage help:

```
/tools help
```

## Output

Visibility table (bare form): one row per switch with its on/off state.

Single switch read:

```
`teams` is currently **on**.
```

Successful toggle (the config is saved and the config cache invalidated):

```
[ok] `<switch>` visibility is now **on**.
```

Invalid state word:

```
[warn] Usage: /tools <switch> on|off
```

Config save failure (the in-session change still applied):

```
[warn] `<switch>` visibility changed ... but saving config failed: <error>
```

and the status bar notes `tools: <switch> ... (unsaved)`.

## Related

- `/config show` - inspect the resolved configuration including tool visibility
- `/reload config` - re-read `ragent.json` after manual edits
- `tool_visibility` section in `ragent.json` - persistent equivalent of these switches