# /tools
> List every registered tool: /tools (help) | /tools list|show | /tools help

## Overview

`/tools` reports the live tool registry. The bare form prints the help page;
`/tools list` (alias `/tools show`) prints one row per registered tool with its
source and description.

The command has no switches and changes no state. Every tool the registry holds
is advertised to the model, so the listing is exactly the set the model is given.
A tool absent from the list means the subsystem that registers it is not active -
for example the ten codeindex tools register only while the code index is on.

## One table, four readers (FR-033)

`command_catalog::TOOLS_SUBS` is the single source of the forms `/tools`
accepts. Everything that offers one reads it:

- the autocomplete menu (`get_command_suggestions`) - so the menu offers exactly
  these words;
- the parameter hint (`get_parameter_hint`), rendered as `[<word>|<word>|...]`;
- this command's handler, which derives both its accepted set and the forms its
  help page and every usage warning print;
- `commands_info`, whose `tools` entry publishes the same `subcommands` array.

If you add a `/tools` form, add it to `TOOLS_SUBS` and nowhere else.

## Syntax

```
/tools                        # help
/tools list                   # list every registered tool
/tools show                   # same as list
/tools help                   # usage help (aliases: --help, -h, usage)
```

## Options / Subcommands

| Form | Description |
| --- | --- |
| `/tools` | Show the help page |
| `/tools list` | List every registered tool with its source and description |
| `/tools show` | Same as `/tools list` |
| `/tools help` | Print usage help |

## Examples

Show the help page:

```
/tools
```

List the registered tools:

```
/tools list
```

## Output

Help page:

```
/tools

## /tools - Tool list

| Subcommand | Description |
|---|---|
| `/tools` | Show this help |
| `/tools list` | List every registered tool with its source and description |
| `/tools show` | Same as `/tools list` |
| `/tools help` | Show this help |
```

Usage warning for an unrecognised form:

```
From: /tools
[warn] Usage: `/tools list`, `/tools show`, `/tools help`.
```

## Related

- `/config show` - inspect the resolved configuration
- `/reload config` - re-read `ragent.json` after manual edits
- [`docs/howtos/tools/INDEX.md`](../tools/INDEX.md) - the tool catalogue by
  category
