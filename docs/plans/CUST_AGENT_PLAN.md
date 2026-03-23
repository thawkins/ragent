# Custom Agent System — Design & Implementation Plan

## Overview

This document specifies the design and implementation plan for a custom agent system in ragent. Custom agents are defined using a ragent-adapted profile of the **Open Agentic Schema Framework (OASF)** standard ([schema.oasf.outshift.com](https://schema.oasf.outshift.com)), stored as `.json` files in well-known directories, and surfaced through the TUI `/agents` slash command.

---

## 1. Goals

| # | Goal |
|---|------|
| G1 | Users can define custom agents in plain JSON using a standard, documented schema |
| G2 | Agent files are discovered automatically from project and home directories |
| G3 | Custom agents integrate seamlessly with the existing `/agent` picker and session processor |
| G4 | The `/agents` slash command lists built-in and custom agents in clearly separated sections |
| G5 | Invalid or malformed agent files produce clear, non-fatal diagnostics logged to the TUI |
| G6 | Custom agents fully support system prompts, permissions, skills, model binding, and sub-agent delegation |

---

## 2. OASF Agent Record Format

Custom agents are stored as UTF-8 JSON files (`.json` extension). The schema is a **ragent profile** of OASF — it uses OASF's core envelope (name, description, version, schema_version, authors, skills, domains, locators, modules) and extends it with ragent-specific fields inside a `modules` entry of type `ragent/agent/v1`.

### 2.1 Minimal Example

```json
{
  "name": "my-reviewer",
  "description": "Code reviewer that focuses on security and performance issues",
  "version": "1.0.0",
  "schema_version": "0.7.0",
  "authors": ["Jane Doe <jane@example.com>"],
  "created_at": "2025-01-01T00:00:00Z",
  "skills": [
    { "name": "software_engineering/code_review", "id": 2001 }
  ],
  "domains": [
    { "name": "technology/software_development", "id": 3001 }
  ],
  "locators": [],
  "modules": [
    {
      "type": "ragent/agent/v1",
      "payload": {
        "system_prompt": "You are an expert code reviewer. Focus on security vulnerabilities, performance bottlenecks, and correctness. Be concise and actionable.",
        "mode": "primary",
        "max_steps": 50,
        "temperature": 0.3,
        "model": "anthropic:claude-sonnet-4-20250514",
        "skills": ["review"],
        "permissions": [
          { "permission": "read",  "pattern": "**", "action": "allow" },
          { "permission": "bash",  "pattern": "**", "action": "deny"  },
          { "permission": "edit",  "pattern": "**", "action": "ask"   }
        ],
        "options": {}
      }
    }
  ]
}
```

### 2.2 Full Field Reference

#### OASF Core Fields

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `name` | `string` | ✅ | Unique identifier (kebab-case, e.g. `my-reviewer`). Must be unique across built-ins + custom. |
| `description` | `string` | ✅ | One-line description shown in the TUI agent picker |
| `version` | `string` | ✅ | Semantic version of the agent definition (e.g. `1.0.0`) |
| `schema_version` | `string` | ✅ | OASF schema version this record targets (e.g. `0.7.0`) |
| `authors` | `string[]` | ❌ | List of authors in `Name <email>` format |
| `created_at` | `string` | ❌ | RFC 3339 timestamp of creation |
| `skills` | `Skill[]` | ❌ | OASF skill annotations for discovery (not ragent skill invocation) |
| `domains` | `Domain[]` | ❌ | OASF domain annotations |
| `locators` | `Locator[]` | ❌ | Source code / registry references |
| `modules` | `Module[]` | ✅ | Must contain exactly one module with `type: "ragent/agent/v1"` |

#### ragent/agent/v1 Module Payload

| Field | Type | Required | Default | Description |
|-------|------|----------|---------|-------------|
| `system_prompt` | `string` | ✅ | — | The agent's system prompt. Supports `{{WORKING_DIR}}`, `{{FILE_TREE}}`, `{{AGENTS_MD}}` template variables |
| `mode` | `"primary" \| "subagent" \| "all"` | ❌ | `"all"` | Whether agent is user-selectable, spawnable as sub-agent, or both |
| `max_steps` | `u32` | ❌ | `100` | Maximum agentic loop iterations |
| `temperature` | `f32` | ❌ | `null` | Override model sampling temperature (0.0–2.0) |
| `top_p` | `f32` | ❌ | `null` | Override nucleus sampling |
| `model` | `string` | ❌ | `null` | Model binding in `provider:model` format (e.g. `anthropic:claude-haiku-4-5`) |
| `skills` | `string[]` | ❌ | `[]` | List of ragent skill names to auto-load for this agent |
| `permissions` | `Permission[]` | ❌ | default permissions | Tool permission ruleset |
| `hidden` | `bool` | ❌ | `false` | If true, omit from user-visible agent pickers |
| `options` | `object` | ❌ | `{}` | Provider-specific options (passed as-is to the LLM API) |

#### Permission Object

```json
{ "permission": "bash", "pattern": "**", "action": "allow" }
```

| Field | Values |
|-------|--------|
| `permission` | `"read"`, `"edit"`, `"bash"`, `"web"`, `"question"`, `"todo"`, `"plan_enter"`, `"plan_exit"` |
| `pattern` | Glob string (e.g. `"src/**"`, `"**"`) |
| `action` | `"allow"`, `"deny"`, `"ask"` |

---

## 3. Agent Discovery

### 3.1 Search Paths (in priority order, highest last)

| Priority | Location | Scope |
|----------|----------|-------|
| 1 (lowest) | `~/.ragent/agents/*.json` | User-global custom agents |
| 2 (highest) | `[PROJECT]/.ragent/agents/*.json` | Project-local custom agents (override global) |

Project directory is the current working directory at launch, walked upward until a `.ragent/` directory or filesystem root is found.

Both directories are scanned at startup. If the same `name` appears in both, the project-local definition wins.

### 3.2 Built-in vs Custom Precedence

- Built-in agents cannot be deleted by custom definitions
- If a custom agent shares a name with a built-in, a diagnostic warning is emitted and the custom agent is loaded under the name with a `custom:` prefix (e.g. `custom:general`) to avoid collision
- Users can extend a built-in's behaviour by using a distinct name and using the built-in's system prompt as a template

### 3.3 File Naming

Files must have the `.json` extension. The file name is informational only — the agent `name` field inside the JSON is the actual identifier. Multiple agents may be defined in subdirectories; ragent walks the search path recursively.

---

## 4. Code Architecture

### 4.1 New Module: `ragent-core/src/agent/custom.rs`

```rust
/// Loaded, validated custom agent definition.
pub struct CustomAgentDef {
    /// The parsed OASF agent record.
    pub record: OasfAgentRecord,
    /// Path from which the record was loaded.
    pub source_path: PathBuf,
    /// Resolved AgentInfo ready for use by the session processor.
    pub agent_info: AgentInfo,
}

/// Load all custom agents from the standard discovery paths.
/// Returns (agents, errors) — errors are non-fatal diagnostic strings.
pub fn load_custom_agents(working_dir: &Path) -> (Vec<CustomAgentDef>, Vec<String>)

/// Validate a single OASF record and convert to AgentInfo.
pub fn record_to_agent_info(record: &OasfAgentRecord) -> Result<AgentInfo, String>

/// Find the project's .ragent/agents directory (walks up from working_dir).
pub fn find_project_agents_dir(working_dir: &Path) -> Option<PathBuf>

/// Return ~/.ragent/agents/ (user-global).
pub fn global_agents_dir() -> Option<PathBuf>
```

### 4.2 OASF Data Structures: `ragent-core/src/agent/oasf.rs`

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OasfAgentRecord {
    pub name: String,
    pub description: String,
    pub version: String,
    pub schema_version: String,
    pub authors: Option<Vec<String>>,
    pub created_at: Option<String>,
    pub skills: Option<Vec<OasfSkill>>,
    pub domains: Option<Vec<OasfDomain>>,
    pub locators: Option<Vec<OasfLocator>>,
    pub modules: Vec<OasfModule>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OasfModule {
    #[serde(rename = "type")]
    pub module_type: String,
    pub payload: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RagentAgentPayload {
    pub system_prompt: String,
    pub mode: Option<String>,
    pub max_steps: Option<u32>,
    pub temperature: Option<f32>,
    pub top_p: Option<f32>,
    pub model: Option<String>,
    pub skills: Option<Vec<String>>,
    pub permissions: Option<Vec<RagentPermissionRule>>,
    pub hidden: Option<bool>,
    pub options: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RagentPermissionRule {
    pub permission: String,
    pub pattern: String,
    pub action: String,
}
```

### 4.3 Changes to Existing Code

#### `ragent-core/src/agent/mod.rs`
- Add `pub fn load_all_agents(working_dir: &Path) -> (Vec<AgentInfo>, Vec<String>)` — returns built-ins + loaded customs and any load errors
- Expose `RAGENT_MODULE_TYPE: &str = "ragent/agent/v1"`

#### `ragent-tui/src/app.rs`
- In `App::new()`: call `load_all_agents(working_dir)`, store result in `self.cycleable_agents` (existing field)
- Store custom agent metadata separately: `self.custom_agent_defs: Vec<CustomAgentDef>` for display in `/agents`
- Log any load errors into the log panel at startup as `LogLevel::Warn`

#### `ragent-tui/src/app/state.rs`
- Add field: `pub custom_agent_defs: Vec<CustomAgentDef>`
- Initialise to empty `Vec`

### 4.4 `/agents` Slash Command

Add to `SLASH_COMMANDS` in `state.rs`:
```rust
SlashCommandDef {
    trigger: "agents",
    description: "List all built-in and custom agents with descriptions",
},
```

Handle in `app.rs` `"agents"` match arm:
- Render a detailed info panel (similar to `/help`) into the messages area showing:
  - **Section 1 — Built-in Agents**: table of name, mode, description, max_steps, model
  - **Section 2 — Custom Agents**: table of name, source file, version, mode, description, skills, model
  - Any load errors/warnings in a **Section 3 — Diagnostics**

#### `/agent` Picker Enhancement
- Custom agents appear alongside built-ins in the existing `/agent` picker dialog
- Custom agents show a `[custom]` badge next to their name

---

## 5. System Prompt Template Variables

When a custom agent's `system_prompt` contains these variables they are substituted at invocation time (matching the existing `build_system_prompt` flow):

| Variable | Substituted With |
|----------|-----------------|
| `{{WORKING_DIR}}` | Absolute path of the current working directory |
| `{{FILE_TREE}}` | Project file tree (same as built-in agents receive) |
| `{{AGENTS_MD}}` | Contents of `AGENTS.md` if present, else empty string |
| `{{DATE}}` | Current date in `YYYY-MM-DD` format |

---

## 6. Validation Rules

The following conditions produce a warning and skip the agent (non-fatal):

| Rule | Error Message |
|------|--------------|
| `name` is empty or contains spaces | `"agent name must be non-empty and contain no spaces"` |
| No `ragent/agent/v1` module found | `"missing required module type 'ragent/agent/v1'"` |
| `system_prompt` is empty | `"system_prompt must not be empty"` |
| `mode` value unrecognised | `"unknown mode '<value>'; expected primary, subagent, or all"` |
| `temperature` outside `[0.0, 2.0]` | `"temperature out of range [0, 2]"` |
| `model` not in `provider:model` format | `"model must be in 'provider:model' format"` |
| `permission` action unrecognised | `"unknown action '<value>'; expected allow, deny, or ask"` |
| `name` collides with a built-in | `"name collides with built-in agent; loaded as 'custom:<name>'"` (warning, not skip) |

---

## 7. Example Agent Files

### 7.1 Security Reviewer (`~/.ragent/agents/security-reviewer.json`)

```json
{
  "name": "security-reviewer",
  "description": "OWASP-focused security code reviewer",
  "version": "1.0.0",
  "schema_version": "0.7.0",
  "authors": ["Security Team"],
  "created_at": "2025-01-01T00:00:00Z",
  "skills": [
    { "name": "software_engineering/code_review", "id": 2001 },
    { "name": "cybersecurity/vulnerability_assessment", "id": 5001 }
  ],
  "domains": [{ "name": "technology/cybersecurity", "id": 4001 }],
  "locators": [],
  "modules": [{
    "type": "ragent/agent/v1",
    "payload": {
      "system_prompt": "You are a security-focused code reviewer. Working directory: {{WORKING_DIR}}\n\nFocus on OWASP Top 10 vulnerabilities, injection flaws, authentication issues, and data exposure risks. Provide CWE references where applicable.\n\n{{AGENTS_MD}}",
      "mode": "primary",
      "max_steps": 30,
      "temperature": 0.2,
      "permissions": [
        { "permission": "read", "pattern": "**", "action": "allow" },
        { "permission": "bash", "pattern": "**", "action": "deny" },
        { "permission": "edit", "pattern": "**", "action": "deny" }
      ]
    }
  }]
}
```

### 7.2 Documentation Writer (`.ragent/agents/doc-writer.json`)

```json
{
  "name": "doc-writer",
  "description": "Technical documentation specialist for this project",
  "version": "1.0.0",
  "schema_version": "0.7.0",
  "authors": ["Project Team"],
  "created_at": "2025-01-01T00:00:00Z",
  "skills": [
    { "name": "natural_language_processing/natural_language_generation/text_completion", "id": 1001 }
  ],
  "domains": [{ "name": "technology/software_development", "id": 3001 }],
  "locators": [
    { "type": "source_code", "urls": ["https://github.com/example/project"] }
  ],
  "modules": [{
    "type": "ragent/agent/v1",
    "payload": {
      "system_prompt": "You are a technical writer for this project.\nProject root: {{WORKING_DIR}}\n\nWrite clear, concise Markdown documentation. Follow existing style in the project docs/ folder. Always include examples.\n\n{{AGENTS_MD}}",
      "mode": "all",
      "max_steps": 50,
      "temperature": 0.7,
      "skills": ["docs-check"],
      "permissions": [
        { "permission": "read",  "pattern": "**",      "action": "allow" },
        { "permission": "edit",  "pattern": "docs/**", "action": "allow" },
        { "permission": "edit",  "pattern": "**",      "action": "ask"   },
        { "permission": "bash",  "pattern": "**",      "action": "deny"  }
      ]
    }
  }]
}
```

---

## 8. TUI `/agents` Display Format

```
Built-in Agents
───────────────────────────────────────────────────────
  ask          primary    Quick answers, no tools
  general      primary    General-purpose coding agent    [500 steps]
  build        subagent   Build/test runner               [500 steps]
  plan         subagent   Planning and task breakdown     [20 steps]
  explore      subagent   Codebase exploration            [15 steps]

Custom Agents  (2 loaded from ~/.ragent/agents, 1 from .ragent/agents)
───────────────────────────────────────────────────────
  security-reviewer  primary    OWASP-focused security code reviewer    ~/.ragent/agents/security-reviewer.json
  doc-writer         all        Technical documentation specialist       .ragent/agents/doc-writer.json

Diagnostics
───────────────────────────────────────────────────────
  ⚠ ~/.ragent/agents/broken.json: missing required module type 'ragent/agent/v1'
```

---

## 9. Implementation Tasks

### Milestone 1 — Core Data Structures and Loading

| Task | File(s) | Description |
|------|---------|-------------|
| M1-T1 | `ragent-core/src/agent/oasf.rs` (new) | Define `OasfAgentRecord`, `OasfModule`, `RagentAgentPayload`, `RagentPermissionRule` serde structs |
| M1-T2 | `ragent-core/src/agent/custom.rs` (new) | Implement `load_custom_agents()`, `find_project_agents_dir()`, `global_agents_dir()`, `record_to_agent_info()` |
| M1-T3 | `ragent-core/src/agent/mod.rs` | Add `load_all_agents()` combining built-ins + custom; expose `CustomAgentDef` type |
| M1-T4 | `ragent-core/src/agent/mod.rs` | Update `build_system_prompt()` to substitute `{{WORKING_DIR}}`, `{{FILE_TREE}}`, `{{AGENTS_MD}}`, `{{DATE}}` variables |
| M1-T5 | `Cargo.toml` | No new dependencies needed (serde_json already available) |

### Milestone 2 — TUI Integration

| Task | File(s) | Description |
|------|---------|-------------|
| M2-T1 | `ragent-tui/src/app/state.rs` | Add `custom_agent_defs: Vec<CustomAgentDef>` field to `App` struct |
| M2-T2 | `ragent-tui/src/app.rs` | In `App::new()`: call `load_all_agents()`, populate `cycleable_agents` with both built-in and custom (non-hidden); store full `custom_agent_defs`; log load errors as Warn |
| M2-T3 | `ragent-tui/src/app/state.rs` | Add `"agents"` entry to `SLASH_COMMANDS` |
| M2-T4 | `ragent-tui/src/app.rs` | Implement `"agents"` slash command handler — builds formatted info text and appends as assistant message |
| M2-T5 | `ragent-tui/src/layout.rs` | In `/agent` picker dialog: render `[custom]` badge (yellow) after custom agent names |

### Milestone 3 — Validation and Diagnostics

| Task | File(s) | Description |
|------|---------|-------------|
| M3-T1 | `ragent-core/src/agent/custom.rs` | Implement full field validation per Section 6 rules |
| M3-T2 | `ragent-core/src/agent/custom.rs` | Handle name collision with built-ins (rename to `custom:<name>` + warn) |
| M3-T3 | `ragent-tui/src/app.rs` | Display load errors in `/agents` Diagnostics section and in the TUI log panel at startup |

### Milestone 4 — Documentation and Examples

| Task | File(s) | Description |
|------|---------|-------------|
| M4-T1 | `docs/custom-agents.md` (new) | Full user-facing documentation: schema reference, examples, template variables, discovery paths |
| M4-T2 | `examples/agents/` (new dir) | Ship `security-reviewer.json` and `doc-writer.json` as example custom agent files |
| M4-T3 | `README.md` | Add "Custom Agents" section linking to docs |
| M4-T4 | `QUICKSTART.md` | Add quick-start example: creating a minimal custom agent |

### Milestone 5 — Testing

| Task | File(s) | Description |
|------|---------|-------------|
| M5-T1 | `crates/ragent-core/tests/test_custom_agents.rs` (new) | Unit tests: valid record parses to AgentInfo, invalid records produce correct errors, discovery path resolution, name collision handling |
| M5-T2 | `crates/ragent-core/tests/test_custom_agents.rs` | Test template variable substitution in system_prompt |
| M5-T3 | `crates/ragent-core/tests/test_custom_agents.rs` | Test permission ruleset parsing and round-trip |

---

## 10. Milestones Summary

| Milestone | Deliverable | Dependencies |
|-----------|-------------|--------------|
| **M1** | Core OASF parsing and custom agent loading in `ragent-core` | None |
| **M2** | TUI wiring: `/agents` command, picker badge, startup loading | M1 |
| **M3** | Validation, collision handling, diagnostic display | M1, M2 |
| **M4** | User documentation and example files | M1 |
| **M5** | Full test coverage | M1, M2, M3 |

---

## 11. Out of Scope (Future Work)

- Hot-reload of agent files without restart (inotify/kqueue watch)
- OASF Directory service integration for remote agent discovery
- Agent marketplace / sharing
- OASF schema version compatibility matrix and migration tooling
- Custom agent sandboxing / capability isolation beyond the existing permission system
- GUI agent editor

---

## 12. Schema Version Compatibility

This implementation targets **OASF schema_version `0.7.0`** (current at time of writing). The `schema_version` field in the record is stored and displayed but not used for validation gating in this release — all records that contain a valid `ragent/agent/v1` module are accepted regardless of declared `schema_version`. This policy will be revisited as OASF reaches 1.0.

---

*Document version: 1.0.0 | Status: Design / Pre-implementation*
