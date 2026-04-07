# Custom Agents

ragent supports custom agent definitions in two formats:

1. **Agent Profiles** (`.md`) — Markdown files with JSON frontmatter. The
   markdown body IS the system prompt. Simple and self-documenting.
2. **OASF Records** (`.json`) — Structured JSON files following the
   [Open Agentic Schema Framework](https://oasf.agntcy.org/) standard.

Both formats let you tailor the agent's system prompt, permissions, model, and
behaviour without changing any code.

---

## Table of Contents

- [Quick Start — Profiles (.md)](#quick-start--profiles-md)
- [Quick Start — OASF (.json)](#quick-start--oasf-json)
- [Discovery Paths](#discovery-paths)
- [Profile Format (.md)](#profile-format-md)
- [OASF Schema Reference (.json)](#oasf-schema-reference-json)
- [Template Variables](#template-variables)
- [Permission Rules](#permission-rules)
- [Validation Rules](#validation-rules)
- [Persistent Memory](#persistent-memory)
- [Examples](#examples)
- [Using Profiles in Team Blueprints](#using-profiles-in-team-blueprints)
- [Slash Commands](#slash-commands)

---

## Quick Start — Profiles (.md)

The easiest way to create a custom agent. Write a markdown file — the body
becomes the system prompt.

1. Create the agents directory:

   ```bash
   mkdir -p .ragent/agents
   ```

2. Create a profile:

   ```bash
   cat > .ragent/agents/my-agent.md << 'EOF'
   ---
   {
     "name": "my-agent",
     "description": "A helpful assistant for my project"
   }
   ---

   You are a helpful AI assistant working on this project.

   Focus on clear, concise answers. When editing code, follow the existing
   style and conventions.
   EOF
   ```

3. Start ragent — your agent loads automatically. Use `/agents` to verify.

---

## Quick Start — OASF (.json)

For full OASF compatibility or when you need the structured envelope format.

1. Create the agents directory:

   ```bash
   # User-global (available in every project)
   mkdir -p ~/.ragent/agents

   # Project-local (this project only, takes priority)
   mkdir -p .ragent/agents
   ```

2. Copy an example and customise it:

   ```bash
   cp examples/agents/minimal-agent.json ~/.ragent/agents/my-agent.json
   $EDITOR ~/.ragent/agents/my-agent.json
   ```

3. Start ragent — your agent loads automatically. Use `/agents` to verify it
   appeared, or `/agent` to pick it from the interactive list.

---

## Discovery Paths

ragent searches two directories at startup:

| Priority | Directory | Scope |
|----------|-----------|-------|
| Lower | `~/.ragent/agents/` | All projects (user-global) |
| Higher | `[PROJECT]/.ragent/agents/` | This project only |

The **project directory** is the nearest ancestor of the current working
directory that contains a `.ragent/agents/` subdirectory. Project-local
definitions override user-global definitions when both have the same `name`.

Subdirectories are searched recursively. Both `.md` (profile) and `.json`
(OASF) files are loaded.

---

## Profile Format (.md)

A profile is a markdown file with a JSON frontmatter block between `---`
delimiters. Everything after the closing `---` becomes the `system_prompt`.

### Structure

````markdown
---
{
  "name": "agent-name",
  "description": "One-line summary"
}
---

Your system prompt goes here. This is the markdown body.

It can contain **rich formatting**, code blocks, lists — anything the
model can interpret as instructions.
````

### Frontmatter Fields

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `name` | string | — | **Required.** Unique agent identifier (kebab-case, no spaces). |
| `description` | string | — | **Required.** One-line summary shown in `/agents` and the picker. |
| `mode` | string | `"all"` | Availability: `"primary"`, `"subagent"`, or `"all"`. |
| `model` | string | *inherited* | Lock to a specific model: `"provider:model"` format (e.g. `"anthropic:claude-sonnet-4-20250514"`). When omitted the agent inherits the globally-selected model from `/provider`. |
| `max_steps` | integer | `100` | Maximum tool-call steps before the agent stops. |
| `temperature` | float | provider default | Sampling temperature in `[0.0, 2.0]`. |
| `top_p` | float | provider default | Nucleus sampling probability in `[0.0, 1.0]`. |
| `hidden` | bool | `false` | When `true`, hidden from picker but available via `/agent <name>`. |
| `memory` | string | `"none"` | Persistent memory scope: `"none"`, `"user"`, or `"project"`. See [Persistent Memory](#persistent-memory). |
| `permissions` | object[] | default ruleset | [Permission rules](#permission-rules). |
| `skills` | string[] | `[]` | Skill names the agent can invoke. |
| `options` | object | `{}` | Provider-specific options passed through verbatim. |

The markdown body supports [template variables](#template-variables) just like
the OASF `system_prompt` field.

---

## OASF Schema Reference (.json)

Each custom agent is a JSON file containing one OASF record. The top-level
fields follow the OASF envelope; ragent-specific configuration lives inside
the `ragent/agent/v1` module payload.

### Top-Level Fields

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `name` | string | ✅ | Unique agent identifier. No spaces. Used as the agent selector key. |
| `description` | string | ✅ | One-line summary shown in `/agents` and the picker. |
| `version` | string | | Semantic version of this agent definition. |
| `schema_version` | string | | OASF schema version (e.g. `"0.7.0"`). |
| `authors` | string[] | | List of author names or email addresses. |
| `created_at` | string | | ISO 8601 creation timestamp. |
| `skills` | object[] | | OASF skill taxonomy annotations (informational only). |
| `domains` | object[] | | OASF domain taxonomy annotations (informational only). |
| `locators` | object[] | | Source/artifact locators (informational only). |
| `modules` | object[] | ✅ | Must contain at least one entry with `"type": "ragent/agent/v1"`. |

### `ragent/agent/v1` Payload Fields

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `system_prompt` | string | — | **Required.** The agent's system prompt. Supports [template variables](#template-variables). Max 32,768 chars. |
| `mode` | string | `"all"` | Availability: `"primary"` (user-selectable), `"subagent"` (delegation only), `"all"` (both). |
| `max_steps` | integer | `100` | Maximum tool-call steps before the agent stops. Must be ≥ 1. |
| `temperature` | float | provider default | Sampling temperature in `[0.0, 2.0]`. |
| `top_p` | float | provider default | Nucleus sampling probability in `[0.0, 1.0]`. |
| `model` | string | *inherited* | Lock to a specific model: `"provider:model"` format (e.g. `"anthropic:claude-opus-4-5"`). When omitted the agent inherits the globally-selected model. |
| `hidden` | bool | `false` | When `true`, the agent is available for direct switch (`/agent <name>`) but not shown in the picker or `/agents` list. |
| `memory` | string | `"none"` | Persistent memory scope: `"none"`, `"user"`, or `"project"`. See [Persistent Memory](#persistent-memory). |
| `permissions` | object[] | default ruleset | [Permission rules](#permission-rules). Omit to inherit the default ruleset. |
| `options` | object | `{}` | Provider-specific options passed through verbatim (e.g. `{"max_tokens": 4096}`). |
| `skills` | string[] | `[]` | Skill names the agent can invoke (e.g. `"simplify"`). |

---

## Template Variables

The following placeholders in `system_prompt` are substituted at session start:

| Variable | Replaced With |
|----------|---------------|
| `{{WORKING_DIR}}` | Absolute path of the current working directory |
| `{{FILE_TREE}}` | Two-level directory listing of the working directory |
| `{{AGENTS_MD}}` | Contents of `AGENTS.md` in the project root (if it exists) |
| `{{DATE}}` | Current date in `YYYY-MM-DD` format (UTC) |

Sections whose content was already embedded via a template variable are not
auto-appended by the agent system, so there is no duplication.

### Example

```json
"system_prompt": "You are a documentation writer.\nProject: {{WORKING_DIR}}\nDate: {{DATE}}\n\n{{AGENTS_MD}}"
```

---

## Permission Rules

The `permissions` array controls what file and shell operations the agent may
perform without asking for confirmation. Rules are evaluated in order; the first
match wins.

### Rule Object

```json
{ "permission": "<category>", "pattern": "<glob>", "action": "<action>" }
```

| Field | Values | Description |
|-------|--------|-------------|
| `permission` | `read`, `edit`, `bash`, `question` | Operation category |
| `pattern` | glob string | Files or commands the rule matches (e.g. `"**"`, `"src/**/*.rs"`) |
| `action` | `allow`, `deny`, `ask` | What to do when matched |

### Example — Read-Only Agent

```json
"permissions": [
  { "permission": "read", "pattern": "**", "action": "allow" },
  { "permission": "edit", "pattern": "**", "action": "deny"  },
  { "permission": "bash", "pattern": "**", "action": "deny"  }
]
```

### Example — Docs-Only Writer

```json
"permissions": [
  { "permission": "read",  "pattern": "**",      "action": "allow" },
  { "permission": "edit",  "pattern": "docs/**", "action": "allow" },
  { "permission": "edit",  "pattern": "**/*.md", "action": "allow" },
  { "permission": "edit",  "pattern": "**",      "action": "ask"   },
  { "permission": "bash",  "pattern": "**",      "action": "deny"  }
]
```

---

## Validation Rules

If a file fails validation it is skipped with a non-fatal diagnostic (shown at
startup in the log panel and in `/agents → Diagnostics`).

### Common Rules (both formats)

| Condition | Error |
|-----------|-------|
| `name` is empty or contains spaces | `agent name must be non-empty and contain no spaces` |
| `description` is empty | `description must not be empty` |
| `system_prompt` is empty | `system_prompt must not be empty` |
| `system_prompt` exceeds 32,768 chars | `system_prompt too long (N chars; max 32768)` |
| `mode` is unrecognised | `unknown mode '<value>'; expected primary, subagent, or all` |
| `temperature` outside `[0.0, 2.0]` | `temperature N out of range [0.0, 2.0]` |
| `top_p` outside `[0.0, 1.0]` | `top_p N out of range [0.0, 1.0]` |
| `model` not in `provider:model` format | `model '<value>' must be in 'provider:model' format` |
| `memory` not one of `none`, `user`, `project` | `unknown memory scope '<value>'; expected none, user, or project` |
| `max_steps` is 0 | `max_steps must be greater than 0` |
| Permission `action` unrecognised | `unknown action '<value>'; expected allow, deny, or ask` |
| `name` collides with a built-in | Warning (not skip): loaded as `custom:<name>` |

### Profile-Specific Rules (.md)

| Condition | Error |
|-----------|-------|
| Missing `---` frontmatter delimiters | `missing JSON frontmatter (expected --- delimiters)` |
| Invalid JSON in frontmatter | `frontmatter JSON parse error: ...` |
| Empty markdown body after `---` | `markdown body (system_prompt) must not be empty` |

### OASF-Specific Rules (.json)

| Condition | Error |
|-----------|-------|
| No `ragent/agent/v1` module | `missing required module type 'ragent/agent/v1'` |

---

## Persistent Memory

Agents can maintain a persistent memory directory that survives across sessions.
This lets teammates build institutional knowledge — learned patterns, project
conventions, frequently needed context — that is automatically injected into
their system prompt at spawn time.

### Memory Scopes

| Scope | Directory | Use Case |
|-------|-----------|----------|
| `"none"` | *(disabled)* | Default. No memory directory. |
| `"user"` | `~/.ragent/agent-memory/<agent-name>/` | User-global memory shared across all projects. |
| `"project"` | `.ragent/agent-memory/<agent-name>/` | Project-local memory specific to the repository. |

### How It Works

1. **At spawn** — If memory is enabled, ragent reads `MEMORY.md` from the
   agent's memory directory and injects the first 200 lines (or 25 KB) into the
   system prompt.
2. **During execution** — The agent can read/write files in its memory directory
   using the `team_memory_read` and `team_memory_write` tools.
3. **Across sessions** — The memory directory persists on disk, so information
   written in one session is available in the next.

### Memory Tools

| Tool | Description |
|------|-------------|
| `team_memory_read` | Read a file from the agent's memory directory. Defaults to `MEMORY.md`. |
| `team_memory_write` | Write or append to a file in the memory directory. Defaults to append mode on `MEMORY.md`. |

### Example — Agent with Project Memory

```markdown
---
{
  "name": "architect",
  "description": "System architect with project memory",
  "memory": "project"
}
---

You are a system architect. Review code structure, suggest improvements,
and document architectural decisions.

Use your memory to record:
- Key architectural decisions and rationale
- Component relationships and dependencies
- Known technical debt items
```

### Memory Scope Inheritance

When a teammate is spawned in a team:

1. Blueprint `memory` field (from `spawn-prompts.json`) takes priority
2. Falls back to the agent profile's `memory` field
3. Falls back to `"none"` (disabled)

---

## Examples

### Minimal Profile (.md)

```markdown
---
{
  "name": "my-agent",
  "description": "A minimal custom agent"
}
---

You are a helpful AI agent.
Working directory: {{WORKING_DIR}}

{{AGENTS_MD}}
```

### Security Reviewer Profile (.md)

```markdown
---
{
  "name": "security-reviewer",
  "description": "OWASP-focused security code reviewer",
  "mode": "subagent",
  "max_steps": 30,
  "temperature": 0.2,
  "permissions": [
    { "permission": "read", "pattern": "**", "action": "allow" },
    { "permission": "edit", "pattern": "**", "action": "deny"  },
    { "permission": "bash", "pattern": "**", "action": "deny"  }
  ]
}
---

You are a security-focused code reviewer specialising in the OWASP Top 10.

For every review:
1. Identify injection flaws (SQL, command, LDAP, XPath)
2. Check authentication and session management weaknesses
3. Look for sensitive data exposure (keys, tokens, PII in logs)
4. Verify access controls and authorisation logic
5. Check for security misconfigurations

Report only high-signal issues with file paths and concrete fixes.
```

### Minimal OASF Agent (.json)

```json
{
  "name": "my-agent",
  "description": "A minimal custom agent example",
  "version": "1.0.0",
  "schema_version": "0.7.0",
  "modules": [{
    "type": "ragent/agent/v1",
    "payload": {
      "system_prompt": "You are a helpful AI agent.\nWorking directory: {{WORKING_DIR}}\n\n{{AGENTS_MD}}",
      "mode": "primary",
              "max_steps": 500    }
  }]
}
```

### Security Reviewer OASF (.json)

Read-only OWASP-focused reviewer (see `examples/agents/security-reviewer.json`):

```json
{
  "name": "security-reviewer",
  "description": "OWASP-focused security code reviewer",
  "version": "1.0.0",
  "schema_version": "0.7.0",
  "modules": [{
    "type": "ragent/agent/v1",
    "payload": {
      "system_prompt": "You are a security-focused code reviewer...\n{{WORKING_DIR}}\n{{AGENTS_MD}}",
      "mode": "primary",
      "max_steps": 30,
      "temperature": 0.2,
      "permissions": [
        { "permission": "read", "pattern": "**", "action": "allow" },
        { "permission": "edit", "pattern": "**", "action": "deny"  },
        { "permission": "bash", "pattern": "**", "action": "deny"  }
      ]
    }
  }]
}
```

See `examples/agents/` for complete, ready-to-use agent files.

---

## Using Profiles in Team Blueprints

Team blueprints can reference agent profiles by name in `spawn-prompts.json`
using either the `agent_type` or `profile` key:

```json
[
  {
    "tool_name": "team_spawn",
    "teammate_name": "reviewer",
    "profile": "security-reviewer",
    "prompt": "Review the authentication module for vulnerabilities."
  }
]
```

When ragent spawns the teammate, it resolves `"security-reviewer"` via the
same discovery pipeline — loading the `.md` or `.json` agent definition from
`.ragent/agents/`. The profile's system prompt, permissions, model, and other
settings are applied to the spawned teammate.

> **Tip:** `"profile"` is an alias for `"agent_type"`. Both work identically
> in `spawn-prompts.json`. Use `"profile"` when referencing a declarative
> agent profile for clarity.

---

## Slash Commands

| Command | Description |
|---------|-------------|
| `/agents` | List all agents (built-in and custom) with scope, format, and diagnostics |
| `/agent` | Open interactive picker — custom agents show a yellow `[custom]` badge |
| `/agent <name>` | Switch directly to a named agent |

Custom agents loaded from the project directory show `[project/profile]` or
`[project/oasf]` scope in `/agents`; user-global agents show `[global/profile]`
or `[global/oasf]`.
