# Custom Agents

ragent supports custom agent definitions using the
[Open Agentic Schema Framework (OASF)](https://oasf.agntcy.org/) standard.
Custom agents let you tailor the agent's system prompt, permissions, model, and
behaviour without changing any code.

---

## Table of Contents

- [Quick Start](#quick-start)
- [Discovery Paths](#discovery-paths)
- [Schema Reference](#schema-reference)
- [Template Variables](#template-variables)
- [Permission Rules](#permission-rules)
- [Validation Rules](#validation-rules)
- [Examples](#examples)
- [Slash Commands](#slash-commands)

---

## Quick Start

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

Subdirectories are searched recursively. Only `.json` files are loaded.

---

## Schema Reference

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
| `model` | string | active model | Lock to a specific model: `"provider:model"` format (e.g. `"anthropic:claude-opus-4-5"`). |
| `hidden` | bool | `false` | When `true`, the agent is available for direct switch (`/agent <name>`) but not shown in the picker or `/agents` list. |
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

| Condition | Error |
|-----------|-------|
| `name` is empty or contains spaces | `agent name must be non-empty and contain no spaces` |
| `description` is empty | `description must not be empty` |
| No `ragent/agent/v1` module | `missing required module type 'ragent/agent/v1'` |
| `system_prompt` is empty | `system_prompt must not be empty` |
| `system_prompt` exceeds 32,768 chars | `system_prompt too long (N chars; max 32768)` |
| `mode` is unrecognised | `unknown mode '<value>'; expected primary, subagent, or all` |
| `temperature` outside `[0.0, 2.0]` | `temperature N out of range [0.0, 2.0]` |
| `top_p` outside `[0.0, 1.0]` | `top_p N out of range [0.0, 1.0]` |
| `model` not in `provider:model` format | `model '<value>' must be in 'provider:model' format` |
| `max_steps` is 0 | `max_steps must be greater than 0` |
| Permission `action` unrecognised | `unknown action '<value>'; expected allow, deny, or ask` |
| `name` collides with a built-in | Warning (not skip): loaded as `custom:<name>` |

---

## Examples

### Minimal Agent

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
      "max_steps": 50
    }
  }]
}
```

### Security Reviewer

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

## Slash Commands

| Command | Description |
|---------|-------------|
| `/agents` | List all agents (built-in and custom) with scope and diagnostics |
| `/agent` | Open interactive picker — custom agents show a yellow `[custom]` badge |
| `/agent <name>` | Switch directly to a named agent |

Custom agents loaded from the project directory show `[project]` scope in
`/agents`; user-global agents show `[global]`.
