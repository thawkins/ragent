# /skills
> List all registered skills and their descriptions (/skills help)

## Overview

`/skills` renders a fixed-width table of every skill registered in the skill
registry: bundled skills plus any custom YAML skill packs discovered from the
user and project skill directories. Each row shows the command name, its
scope, its access level, and its description.

Use it to verify that a custom skill pack was picked up after editing, or to
check which skills the model is allowed to invoke on its own. Reload with
`/reload skills` after adding or editing skill files; the registry is not
watched live.

## Syntax

```
/skills          # list all registered skills
/skills help     # usage help
```

## Options / Subcommands

| Form | Description |
| --- | --- |
| `/skills` | List all registered skills in a fixed-width table |
| `/skills help` | Print usage help |

## Examples

List the registry:

```
/skills
```

Check the usage line:

```
/skills help
```

After editing a skill pack, reload and re-list:

```
/reload skills
/skills
```

Find the access level of a specific skill by scanning the table output:

```
/skills
# look for the row whose Command column matches the skill name
```

## Output

A fenced fixed-width table with the columns:

```
Command | Scope | Access | Description
```

- `Access` is derived from the skill flags: `user_invocable` and
  `disable_model_invocation` combine into one of `both`, `user-only`,
  `agent-only`, or `disabled`.
- If no skills are registered, the output lists the discovery paths instead:
  personal skills live in `~/.ragent/skills/` and project skills in
  `.ragent/skills/`.
- A footer line reports the registered count, e.g. `N skill(s) registered`.

## Related

- `/reload skills` - rescan skill directories after edits
- `skill_manage` - list/read/load/reload skills from chat
- `~/.ragent/skills/` - personal (user-global) skill packs
- `.ragent/skills/` - project-local skill packs (higher priority)