# /context

> "Context cache management: /context refresh | /context help" (SlashCommandDef, crates/ragent-tui/src/app/state.rs).

## Overview

Prompt context (the project file tree, git status, and README content) is
assembled once and cached between messages so that every turn does not have to
re-walk the repository. `/context refresh` drops that cache, forcing the next
user message to recompute the file tree, git status, and README from the
current working directory. Use it after creating, moving, or deleting files
outside of the agent's own tool calls, or when switching branches in a
separate terminal.

The handler is the `context` arm in `crates/ragent-tui/src/app/slash.rs`
(approximately lines 2215-2235).

## Syntax

```
/context refresh
/context help
```

Bare `/context` is equivalent to `/context help`.

## Options / Subcommands

| Form | Description |
|---|---|
| `/context refresh` | Clear the prompt context cache; the next message recomputes file tree, git status, and README |
| `/context help` | Show the subcommand table |

## Examples

### Force a context refresh

```
/context refresh
```

Output:

```
From: /context refresh
[sync] Context cache cleared - next message will recompute file tree, git status, and README.
```

### Show help

```
/context help
```

Output (subcommand table):

```
From: /context help
| Subcommand | Description |
|---|---|
| refresh | Clear the prompt context cache |
| help | Show this help |
```

### Unknown subcommand

```
/context rebuild
```

Output (usage line):

```
From: /context
Unknown subcommand. Usage: /context refresh | /context help
```

## Output

- `refresh` calls `ragent_agent::agent::clear_prompt_context_cache()` and
  reports `[sync] Context cache cleared - next message will recompute file
  tree, git status, and README.` The status line shows the subcommand name.
- `help` (and bare `/context`) prints the subcommand table.
- Any other argument prints a usage line. Nothing is persisted and no files
  are touched; the effect is a single cache invalidation.

The cache is also invalidated automatically after agent file edits; manual
`refresh` is only needed when the working tree changes outside the agent.

## Related

- `/undo` - remove the last turn pair from the conversation
- `/reload` - reload configuration, agents, MCP, and skills
- `/inputdiag` - dump current input and pane state for diagnostics