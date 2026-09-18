# Tools — Skills

Loadable skill packs (bundled or custom YAML / `SKILL.md`) that inject tools,
prompts, and file context into agent sessions.

| Tool | Description |
|------|-------------|
| `skill_manage` | List, read, load, or reload skill packs. |

---

## skill_manage

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
skill_manage action="list"
skill_manage action="load" name="code-audit" arguments="crates/ragent-server"
```
