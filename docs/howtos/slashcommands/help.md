# /help

> Show available slash commands

## Overview

`/help` prints the full list of slash commands in a fenced code block, one
command per line in a fixed-width column, drawn from the `SLASH_COMMANDS`
registry (the same definitions that drive autocomplete). After the built-in
commands, it appends a `Skills:` section listing every user-invocable skill
loaded from the configured skill directories, with its argument hint appended
to the trigger and its description (or `(no description)`).

It takes no arguments and performs no filtering  -  it is always the complete
inventory.

## Syntax

```
/help
```

## Options / Subcommands

| Form | Description |
| --- | --- |
| `/help` | List all available slash commands plus user-invocable skills |

No arguments or subcommands.

## Examples

```
/help
```

Built-in commands section (fixed-width columns inside a code block):

```
Available commands:

  /about             Show application info, version, and authors
  /agent             Switch the active agent: /agent [<name>] | /agent help
  /agents            List all agents - built-in and custom
  ...
```

Skills section (appended when any user-invocable skills are loaded):

```
Skills:
  /docupdate         Refresh docs and CHANGELOG
  /release <args>    Run the release pipeline
```

```
/help
```

No skills loaded: the output ends after the built-in command block.

## Output

- One assistant bubble `From: /help` containing a fenced code block of all
  registered commands (`/{trigger}` padded, then description), optionally
  followed by a `Skills:` list of user-invocable skills from the skill
  registry (name + argument hint on the left, description on the right).
- The working directory and skill directories come from the loaded ragent
  config; skills are discovered fresh at each invocation.
- Status bar shows `help`.

## Related

- `/agents`  -  list agents (commands are listed by `/help`, agents by
  `/agents`)
- Skills are custom YAML packs in the configured skill directories
- `/help` per-command variants such as `/agent help`, `/cancel help`,
  `/history help` print their own subcommand tables