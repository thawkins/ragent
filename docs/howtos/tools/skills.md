# Tools — Skills

Loadable skill packs (bundled or custom YAML / `SKILL.md`) that inject tools,
prompts, and file context into agent sessions.

| Tool | Description |
|------|-------------|
| `Skill` | List, read, load, or reload skill packs. |

---

## Tool restrictions (`allowed-tools`)

A `SKILL.md` frontmatter block may limit the tools the run can call:

```yaml
---
name: rename-check
description: Check the rename landed
allowed-tools:
  - Bash
  - read
  - Grep
  - Glob
---
```

Resolution is an **exact name comparison** against the registry. There is no
translation layer, so:

- the thirteen Claude Code spellings the rename adopted resolve as written -
  `AskUserQuestion`, `Bash`, `Edit`, `EnterPlanMode`, `ExitPlanMode`, `Glob`,
  `Grep`, `Skill`, `TaskCreate`, `TaskGet`, `TaskList`, `TaskUpdate`, `Write`;
- the tools the rename did **not** touch keep their ragent spelling - `read`,
  `list`, `think`, `codeindex_search`, and so on;
- a retired spelling (`bash`, `grep`, `task_create`, ...) matches **nothing**, and
  neither does `Read`, which was rated too close to `read` to adopt;
- entries are case-sensitive: `Bash` is not `bash`, and `Read` is not `read`;
- an empty list means "no restriction", not "no tools".

An entry that matches no registered tool would silently remove a capability, so
it is reported once per `(entry, skill)` pair - naming the entry and the skill -
at the moment the skill's list becomes the restriction (`Skill`,
`/skill <name>` / `/<name>`, and the forked-skill resolver). Fix the manifest;
the run continues with the entries that did resolve.

YAML reads a bare comma list as one scalar: `allowed-tools: Bash, ReadFile` is a
single entry named `Bash, ReadFile`. Use the block form above.

---

## Skill

Manage the skill registry at runtime.

**Arguments**

| Argument | Type | Required | Description | Typical value |
|----------|------|----------|-------------|---------------|
| `action` | enum | yes | `list`, `read`, `load`, `reload` | `"list"` |
| `name` | string | yes for `read`/`load` | Skill name | `"code-audit"` |
| `arguments` | string | no | Substituted into the skill body (`$ARGUMENTS` and friends) for `read`/`load` | `"crates/ragent-server"` |
| `scope` | enum | no | `list` filter: `bundled`, `enterprise`, `openskills-global`, `personal`, `openskills-project`, `project` | `"project"` |
| `include_bodies` | boolean | no | `list` includes full prompt bodies (default false: metadata only) | `false` |

**Example:**
```text
Skill action="list"
Skill action="load" name="code-audit" arguments="crates/ragent-server"
```
