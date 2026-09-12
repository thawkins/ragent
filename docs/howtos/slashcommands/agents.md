# /agents

> List all agents - built-in and custom

## Overview

`/agents` prints a complete inventory of the agents available in the session,
split into two sections. The **Built-in Agents** section lists every cycleable
built-in agent with its description, marking the currently active agent with a
bullet. The **Custom Agents** section lists user-defined agents loaded from
`.ragent/agents/` (project scope) or `~/.ragent/agents/` (global scope), each
annotated with its scope (`project` or `global`) and format (`oasf` for JSON
definitions, `profile` for Markdown definitions). When no custom agents are
loaded, a placeholder line explains where to place them.

If any custom-agent definitions failed to load cleanly, a **Diagnostics**
section lists the warnings at the end.

## Syntax

```
/agents
```

## Options / Subcommands

| Form | Description |
| --- | --- |
| `/agents` | List built-in and custom agents with descriptions, scope/format badges, and load diagnostics |

No subcommands; arguments are ignored.

## Examples

```
/agents
```

Typical output shape (built-ins):

```
From: /agents

Built-in Agents

- `general` - General-purpose assistant
- `coder` - Full-stack coding agent ...
- `orchestrator` - Coordinates sub-agents ...
```

Custom agents with badges (scope/format, active marker):

```
Custom Agents

- `reviewer` - Reviews diffs for quality issues [project/profile]
- `researcher` - Deep web research agent [global/oasf] *
```

No custom agents loaded:

```
Custom Agents

*(none - place .json or .md files in .ragent/agents/ or ~/.ragent/agents/)*
```

Diagnostics section (only when load warnings exist):

```
Diagnostics

- [warn] agent.json: missing 'description' field
```

## Output

- One assistant bubble `From: /agents` containing the Built-in Agents,
  Custom Agents, and (when present) Diagnostics sections.
- The active agent is marked with a trailing bullet (`*`) on its list entry.
- Status bar shows `agents`.

## Related

- `/agent`  -  open the picker or switch directly to a named agent
- `/help`  -  list all available slash commands
- Custom agents are documented in `docs/custom-agents.md`