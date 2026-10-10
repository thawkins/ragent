# /skill
> Load or inspect skill packs (/skill [name] | /skill list | /skill help)

## Overview

`/skill` lists the registered skill packs and loads a named pack for
invocation. It is the explicit form of the bare `/<name>` skill trigger: both
resolve the pack through the same skill registry, so an AgentSkills pack
discovered under `.agents/skills/` (project) or `~/.agents/skills/` (user) can
be listed and loaded by its frontmatter `name` (FR-005).

With no argument (or `list`) it prints the same listing as `/skills`. With a
name it loads that pack and starts the invocation exactly as `/<name> [args]`
would: the pack body is substituted with the arguments and injected into the
session as a user turn for the model to act on.

## Syntax

```
/skill                       # list every registered pack
/skill list                  # alias of the above
/skill <name> [args...]      # load and invoke the named pack
/skill help                  # usage help
```

## Options / Subcommands

| Form | Description |
| --- | --- |
| `/skill` | List every registered pack with scope, access, and description |
| `/skill list` | Alias of `/skill` |
| `/skill <name> [args...]` | Load the named pack and invoke it (same as `/<name> [args]`) |
| `/skill help` | Print usage help |

## Examples

List the registry, including an AgentSkills pack:

```
/skill
```

Load and invoke the `tiny` AgentSkills pack:

```
/skill tiny
```

Load a pack with an argument:

```
/skill deploy staging
```

## Output

The list form renders a fenced block per pack with the pack's `/name`, its
`scope` (for example `openskills-project` for a `.agents/skills/` pack, or
`project` for `.ragent/skills/`), its `access` level, and its description, then
a footer line reporting the registered count.

The load form echoes `/name` as a user message and sets the status to
`invoking skill /<name>...`. If the name is not registered, it reports
`No skill named \`<name>\` is registered` and points at `/skill`. A
non-user-invocable pack reports the refusal; with no provider or model
configured, it points at `/provider` / `/model`.

## Discovery

Packs are discovered from, lowest to highest priority:

- `~/.ragent/skills/<name>/SKILL.md` (personal)
- `.ragent/skills/<name>/SKILL.md` (project)
- the AgentSkills convention `~/.agents/skills/<name>/SKILL.md` (user) and
  `.agents/skills/<name>/SKILL.md` (project)

Each pack is a directory holding a `SKILL.md` whose YAML frontmatter declares
at least `name` and `description`. On a name clash the existing higher-scope
pack wins, so an AgentSkills pack never shadows a same-named `.ragent/skills/`
pack. Reload with `/reload skills` after adding or editing packs.

## Related

- `/skills` - list-only form of the same registry
- `/reload skills` - rescan skill directories after edits
- `skill_manage` - list/read/load/reload skills from chat
