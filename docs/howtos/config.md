# How-To: ragent Configuration (`ragent.json`)

ragent is configured through a single JSON file called `ragent.json`. This
document is the complete reference for every section of that file: what it
controls, the schema, the defaults, and worked examples.

If you only want to get running quickly, read the **Quick Start** section in
the project root `README.md` and the **Tutorial** in
[`docs/howtos/tutorial.md`](tutorial.md). This document is the deep reference.

---

## Table of Contents

1. [Overview](#1-overview)
2. [File Discovery and Loading Order](#2-file-discovery-and-loading-order)
3. [Merge Semantics](#3-merge-semantics)
4. [CLI and Environment Overrides](#4-cli-and-environment-overrides)
5. [Config Error Reporting](#5-config-error-reporting)
6. [Backup and Restore](#6-backup-and-restore)
7. [Section Reference](#7-section-reference)
   - 7.1  [`username`](#71-username)
   - 7.2  [`defaultAgent`](#72-defaultagent)
   - 7.3  [`provider`](#73-provider)
   - 7.4  [`agent`](#74-agent)
   - 7.5  [`permission`](#75-permission)
   - 7.6  [`command`](#76-command)
   - 7.7  [`mcp`](#77-mcp)
   - 7.8  [`instructions`](#78-instructions)
   - 7.9  [`skill_dirs`](#79-skill_dirs)
   - 7.10 [`experimental`](#710-experimental)
   - 7.11 [`hooks`](#711-hooks)
   - 7.12 [`bash`](#72-bash)
   - 7.13 [`dirs`](#713-dirs)
   - 7.14 [`tool_visibility`](#714-tool_visibility)
   - 7.15 [`code_index`](#715-code_index)
   - 7.16 [`stream`](#716-stream)
   - 7.17 [`memory`](#717-memory)
   - 7.18 [`compaction`](#718-compaction)
   - 7.19 [`gitlab`](#719-gitlab)
   - 7.20 [`hidden_tools`](#720-hidden_tools)
   - 7.21 [`yolo`](#721-yolo)
   - 7.22 [`edit_log`](#722-edit_log)
   - 7.23 [`prices`](#723-prices)
   - 7.24 [`browser`](#724-browser)
   - 7.25 [`channels`](#725-channels)
   - 7.26 [`gmail`](#726-gmail)
   - 7.27 [`telemetry`](#727-telemetry)
   - 7.28 [`agent_perf`](#728-agent_perf)
   - 7.29 [`finance`](#729-finance)
   - 7.30 [`tavily_api_key` / `langsearch_api_key` / `perplexity_api_key` / `exa_api_key` / `openalex_email`](#730-search-api-keys)
   - 7.31 [`sdd`](#731-sdd)
   - 7.32 [`trigger`](#732-trigger)
   - 7.33 [`piegap`](#733-piegap)
   - 7.34 [`research`](#734-research)
8. [Full Example File](#8-full-example-file)
9. [Common Recipes](#9-common-recipes)
10. [Related Documents](#10-related-documents)

---

## 1. Overview

All ragent runtime behaviour is driven by a single layered JSON configuration:

- **Provider connections** (API keys, base URLs, thinking settings, model
  catalogues)
- **Agent defaults and overrides** (system prompts, max steps, skills)
- **Permissions** (allow / deny / ask rules, bash and directory allow/deny lists)
- **Tool visibility** (which tool families are advertised to the LLM)
- **Memory, compaction, and retrieval** settings
- **Telemetry / OpenTelemetry export**
- **External integrations** (GitLab, Telegram/Discord channels, Gmail, finance
  data providers)
- **Feature flags** (experimental, SDD, pie-gap, triggers, research)

The config is loaded once at startup by `ragent-config::Config::load`, merged
across layers, and then held in memory for the lifetime of the process.
Several subsystems (YOLO mode, edit logging, bash/directory lists) also read
runtime atomic flags that are synced from the config at startup.

### Design principles

| Principle | Meaning |
| --------- | ------- |
| Layered merge | Global config provides defaults; project config overrides; env vars override both. |
| Explicit-over-default | Many fields track whether the user *explicitly* set them so a serialise round-trip does not lose a toggle. |
| Sensible defaults | Every section has `Default` implementations so an empty `{}` is valid. |
| No secrets in config by force | API keys come from env vars or `env:VAR_NAME` indirection; the config file can reference them but does not have to store them. |
| Atomic persistence | Writes use temp-file-then-rename so a crash never leaves a partial config. |

---

## 2. File Discovery and Loading Order

`Config::load` resolves configuration in a strict precedence order. Each layer
is merged on top of the previous one, with later layers overriding earlier
ones for explicitly-set fields.

```
compiled defaults
  -> global file:   <config_dir>/ragent/ragent.json
  -> project file:  ./.ragent/ragent.json
  -> env file:      $RAGENT_CONFIG  (path to a JSON file)
  -> env inline:    $RAGENT_CONFIG_CONTENT  (raw JSON string)
```

### Platform config directory

The global config directory is resolved by the `dirs` crate:

| Platform | Path |
| -------- | ---- |
| Linux    | `$XDG_CONFIG_HOME/ragent/ragent.json` (default `~/.config/ragent/ragent.json`) |
| macOS    | `~/Library/Application Support/ragent/ragent.json` |
| Windows  | `%APPDATA%\ragent\ragent.json` |

The helper `Config::global_config_dir()` returns `<config_dir>/ragent` and
`Config::global_config_path()` returns the full `ragent.json` path.

### Project config

The project config lives at `./.ragent/ragent.json` relative to the current
working directory. This is the file created automatically on first run when
neither a global nor project config exists.

### JSONC note

The crate-level documentation mentions `ragent.jsonc` as a supported filename,
and JSONC-style comments appear in documentation examples (`//` line comments).
However, the parser uses `serde_json::from_str`, which is strict JSON. Inline
`//` comments are **not** supported in the actual config file. Use a `.json`
extension and plain JSON. The `jsonc` examples in code doc-comments are for
illustration only.

### Auto-creation

If no config file is found at either the global or project path, `Config::load`
creates a `.ragent/` directory and writes a default `ragent.json` there so the
user has a starting point.

---

## 3. Merge Semantics

`Config::merge(base, overlay)` combines two configs. Different sections use
different merge strategies:

| Section | Strategy |
| ------- | -------- |
| `username`, API keys, `gitlab`, `channels`, `gmail` | Overlay overrides base when `Some`. |
| `defaultAgent` | Overlay overrides when explicitly set or different from default. |
| `provider` | Deep merge per provider key: provider-level `api`, `env`, `thinking`, and `options` are replaced when the overlay specifies them, while `models` entries are merged per-model-id so a partial overlay (e.g. `provider.openrouter.models."anthropic/claude-sonnet-4".thinking`) does not discard lower-layer fields such as `name` or `capabilities`. Model-level `thinking` overrides provider-level `thinking`. |
| `agent`, `command`, `mcp` | Per-key overlay replaces base entry. |
| `permission`, `instructions`, `skill_dirs`, `hooks` | Append (union). |
| `bash.allowlist`, `bash.denylist` | Union (deduplicated). |
| `prices` | Per-model last-wins (overlay entry replaces base entry for the same model id). |
| `code_index`, `tool_visibility` | Overlay wins **only** for explicitly-set fields. The `specified` flags are propagated so the value survives a reload. |
| `compaction` | Overlay replaces base entirely. |
| `yolo`, `edit_log` | OR semantics: if the overlay sets `true`, it wins. |
| `sdd`, `piegap` | OR semantics: a flag enabled in either layer stays enabled. |
| `research.open_access_recovery` | OR semantics. `contact_email` and `oa_min_full_text_chars`: overlay overrides when present. |
| `telemetry` | If overlay enables telemetry, the whole overlay `otel` block replaces the base. Otherwise maps (`resource_attributes`, `metrics`) are unioned. |
| `experimental` | `open_telemetry` and `parallel_tool_calls`: OR semantics. Other fields: overlay defaults. |

The `config_paths` field records every file that contributed to the final
merged config, in load order.

---

## 4. CLI and Environment Overrides

### `--config <PATH>` CLI flag

```
ragent --config /path/to/my-config.json run "hello"
```

When `--config` is supplied, the file at that path is loaded directly and
bypasses the normal discovery order. Parse errors for this path include the
file path, line, column, the problematic source line, and a caret marker.

### Environment variables

| Variable | Purpose |
| -------- | ------- |
| `RAGENT_CONFIG` | Path to a JSON config file, merged on top of project config. |
| `RAGENT_CONFIG_CONTENT` | Raw JSON string, merged last (highest precedence). Parse errors show the inline content. |
| Provider API keys | See [section 7.3](#73-provider). Env vars always take precedence over config-file values. |
| `TAVILY_API_KEY` | Overrides `tavily_api_key`. |
| `OPENALEX_EMAIL` | Overrides `openalex_email`. |
| `EXA_API_KEY` | Overrides `exa_api_key`. |
| `GITLAB_TOKEN`, `GITLAB_URL`, `GITLAB_USERNAME` | Override `gitlab` fields (highest priority). |

### Other CLI flags that affect config

| Flag | Effect |
| ---- | ------ |
| `--model <provider/model>` | Overrides the resolved model. |
| `--agent <name>` | Overrides `defaultAgent`. |
| `--yes` / `--no-prompt` | Auto-approves all permissions (like YOLO but does not persist). |

---

## 5. Config Error Reporting

When a config file fails to parse, ragent produces an actionable diagnostic:

```
Failed to parse config file '.ragent/ragent.json':
Error at line 12, column 8:
──────────────────────────────────────────────────────────────────��─────────────
Problematic line:
    "defaultAgent: "coder"
       ^
Parse error: expected `:` at line 12 column 8
```

This includes:

- The file path (or `RAGENT_CONFIG_CONTENT` for the inline env var).
- The line and column from the serde error.
- A separator line.
- The exact source line that caused the failure.
- A caret (`^`) positioned under the error column.
- The underlying serde message.

The same format is used for `--config`, `RAGENT_CONFIG`, and
`RAGENT_CONFIG_CONTENT` errors.

---

## 6. Backup and Restore

`Config::backup_global_config()` snapshots the current global `ragent.json`
into a timestamped file inside `<config_dir>/ragent/saves/`. The backup name
uses `YYYY-MM-DD.HH-MM-SS` (hyphens in the time portion for Windows NTFS
compatibility). If multiple saves happen in the same second, a counter is
appended. The write is atomic (temp-file-then-rename).

`Config::restore_global_config()` restores a backup over the global
`ragent.json`. It validates that:

- The backup file exists and is a regular file.
- The resolved destination is exactly `<config_dir>/ragent.json`.
- The backup content is valid JSON.

The restore is also atomic.

These are used by the `/config backup` and `/config restore` slash commands.

---

## 7. Section Reference

### 7.1 `username`

| Field | Type | Default | Description |
| ----- | ---- | ------- | ----------- |
| `username` | `Option<String>` | `None` | Display name of the user, used in prompts and logs. |

```json
{ "username": "alice" }
```

---

### 7.2 `defaultAgent`

| Field | Type | Default | Description |
| ----- | ---- | ------- | ----------- |
| `defaultAgent` (alias `default_agent`) | `String` | `"general"` | Name of the agent used when none is specified via `--agent`. |

```json
{ "defaultAgent": "coder" }
```

The loader tracks whether the user explicitly set this field
(`specified_default_agent`) so merge operations do not accidentally overwrite
an explicit project-level choice with a global default.

---

### 7.3 `provider`

The `provider` section is the most important part of the config. It is a map
keyed by provider id, where each entry configures one LLM provider.

```json
{
  "provider": {
    "anthropic": {
      "env": ["ANTHROPIC_API_KEY"],
      "thinking": { "enabled": true, "level": "low" },
      "models": {
        "claude-sonnet-4-20250514": {
          "thinking": { "enabled": true, "level": "high", "budget_tokens": 16000 }
        }
      }
    },
    "generic_openai": {
      "env": ["GENERIC_OPENAI_API_KEY"],
      "api": { "base_url": "http://127.0.0.1:8080" }
    }
  }
}
```

#### `ProviderConfig` schema

| Field | Type | Default | Description |
| ----- | ---- | ------- | ----------- |
| `env` | `Vec<String>` | `[]` | Environment variable names required by this provider. ragent checks these are set before model discovery. |
| `api` | `Option<ApiConfig>` | `None` | Optional API endpoint and header overrides. |
| `thinking` | `Option<ThinkingConfig>` | `None` | Default thinking/reasoning config for all models under this provider. Overridden by per-model `thinking`. |
| `models` | `HashMap<String, ModelConfig>` | `{}` | Model definitions keyed by model id. |
| `options` | `HashMap<String, Value>` | `{}` | Arbitrary provider-specific options. |

#### `ApiConfig` schema

| Field | Type | Default | Description |
| ----- | ---- | ------- | ----------- |
| `base_url` | `Option<String>` | `None` | Base URL for API requests, overriding the provider default. |
| `headers` | `HashMap<String, String>` | `{}` | Extra HTTP headers sent with every request. |

#### `ModelConfig` schema

| Field | Type | Default | Description |
| ----- | ---- | ------- | ----------- |
| `name` | `Option<String>` | `None` | Human-readable display name. |
| `cost` | `Option<Cost>` | `None` | Per-token pricing (USD per million tokens). |
| `capabilities` | `Option<Capabilities>` | `None` | Feature flags for this model. |
| `thinking` | `Option<ThinkingConfig>` | `None` | Per-model thinking config; overrides provider-level default. |

#### `Cost` schema

| Field | Type | Description |
| ----- | ---- | ----------- |
| `input` | `f64` | Cost per million input tokens (USD). |
| `output` | `f64` | Cost per million output tokens (USD). |

#### `Capabilities` schema

| Field | Type | Default | Description |
| ----- | ---- | ------- | ----------- |
| `reasoning` | `bool` | `false` | Supports chain-of-thought reasoning. |
| `streaming` | `bool` | `true` | Supports streaming responses. |
| `vision` | `bool` | `false` | Can process image inputs. |
| `tool_use` | `bool` | `true` | Supports tool/function calling. |
| `thinking_levels` | `Vec<ThinkingLevel>` | `[]` | Which thinking/reasoning levels this model supports. |

#### `ThinkingConfig` schema

Defined in `ragent-types`, this is shared across providers.

| Field | Type | Default | Description |
| ----- | ---- | ------- | ----------- |
| `enabled` | `bool` | `true` | Whether thinking is enabled. When `false`, `level` is ignored. |
| `level` | `ThinkingLevel` | `auto` | Thinking depth: `auto`, `off`, `low`, `medium`, `high`. |
| `budget_tokens` | `Option<u32>` | `None` | Max tokens for thinking (Anthropic `budget_tokens`). `None` = provider default. |
| `display` | `Option<ThinkingDisplay>` | `None` | How thinking content is surfaced: `full`, `summarized`, `omitted`. |

`ThinkingLevel` maps to provider-native parameters:

| Provider | Native parameter |
| -------- | ----------------- |
| Anthropic | `thinking.type` + `effort` / `budget_tokens` |
| OpenAI / Copilot | `reasoning_effort` (`low`, `medium`, `high`, `none`) |
| Gemini | `thinkingConfig.thinkingLevel` (`minimal`, `low`, `medium`, `high`, `auto`) |
| Ollama | `think` boolean |

#### Supported provider ids

Each provider is implemented in `crates/ragent-llm/src/providers/`. The
provider id (from `fn id() -> &'static str`) is the key in the `provider` map.

| Provider id | Env var(s) | Notes |
| ----------- | ---------- | ----- |
| `anthropic` | `ANTHROPIC_API_KEY` | Claude models. Supports thinking/budget_tokens. |
| `openai` | `OPENAI_API_KEY` | GPT models. Supports reasoning_effort. |
| `openai_responses` | `OPENAI_API_KEY` | OpenAI Responses API variant. |
| `gemini` | `GOOGLE_API_KEY` or `GEMINI_API_KEY` | Google Gemini. Supports thinkingConfig. |
| `ollama` | `OLLAMA_API_KEY` (optional for local) | Local Ollama. `think` boolean for thinking. |
| `ollama_cloud` | `OLLAMA_API_KEY` | Ollama Cloud variant. |
| `huggingface` | `HF_TOKEN` or `HUGGING_FACE_HUB_TOKEN` | Hugging Face Inference. |
| `copilot` | `GITHUB_COPILOT_TOKEN` or `GITHUB_TOKEN` (via `gh auth`) | GitHub Copilot. |
| `generic_openai` | `GENERIC_OPENAI_API_KEY` or `OPENAI_API_KEY`; `GENERIC_OPENAI_API_BASE` for base URL | Any OpenAI-compatible endpoint. |
| `azure_foundry` | `AZURE_AI_FOUNDRY_API_KEY`; `AZURE_AI_FOUNDRY_BASE` for base URL | Azure AI Foundry. |
| `azure_resource` | (from `azureresources.json`) | Azure Resource (File) provider. Reads model definitions from a separate `azureresources.json` file. |
| `bedrock` | `AWS_ACCESS_KEY_ID`, `AWS_SECRET_ACCESS_KEY`, `AWS_REGION` / `AWS_BEDROCK_REGION`, optional `AWS_SESSION_TOKEN` | Amazon Bedrock. Uses AWS SigV4. |
| `xai` | `XAI_API_KEY`; `XAI_API_BASE` for base URL | xAI (Grok). |
| `router` | (aggregates other providers) | Model Router virtual provider. Classifies prompts into tiers and routes to the best model. |

#### Model Router configuration

The router is configured under `provider.router` and has its own
`RouterConfig`:

```json
{
  "provider": {
    "router": {
      "enabled": true,
      "tiers": {
        "SIMPLE":   { "models": [{ "provider": "ollama", "model": "llama3.2" }] },
        "MEDIUM":   { "models": [{ "provider": "anthropic", "model": "claude-sonnet-4-20250514" }] },
        "COMPLEX":  { "models": [{ "provider": "anthropic", "model": "claude-sonnet-4-20250514" }] },
        "REASONING": { "models": [{ "provider": "openai", "model": "o3" }] }
      },
      "boundaries": { "simple_medium": 0.25, "medium_complex": 0.50, "complex_reasoning": 0.75 },
      "context_messages": 3,
      "default_timeout_ms": 30000
    }
  }
}
```

| Field | Type | Default | Description |
| ----- | ---- | ------- | ----------- |
| `enabled` | `bool` | `false` | Master switch. When `false`, requests pass through to the MEDIUM tier default without classification. |
| `tiers` | `HashMap<String, TierConfig>` | `{}` | Per-tier model fallback chains. Keys: `SIMPLE`, `MEDIUM`, `COMPLEX`, `REASONING`. |
| `weights` | `WeightConfig` | (15 dimensions, summing to 1.0) | Classifier dimension weights. |
| `boundaries` | `BoundaryConfig` | `0.25 / 0.50 / 0.75` | Score boundaries between tiers. Must be ascending and in [0.0, 1.0]. |
| `context_messages` | `usize` | `3` | Number of recent conversation messages included in classification. |
| `default_timeout_ms` | `u64` | `30000` | Default request timeout. |

`TierConfig` has `models: Vec<TierEntry>` (ordered fallback list) and optional
`timeout_ms`. `TierEntry` has `provider` and `model` string fields.

The `WeightConfig` has 15 dimension weights (token_count, vocabulary_complexity,
syntax_complexity, domain_specificity, ambiguity, context_dependency,
reasoning_depth, creativity_level, emotional_complexity, multimodality,
instruction_complexity, knowledge_recency, code_complexity,
mathematical_complexity, image_attachment). Weights are normalised to sum to
1.0 at load time.

For more on providers and models, see the Tutorial
([`docs/howtos/tutorial.md`](tutorial.md)).

---

### 7.4 `agent`

Per-agent configuration overrides, keyed by agent name. Applied on top of
built-in agent defaults.

```json
{
  "agent": {
    "coder": {
      "model": "anthropic/claude-sonnet-4-20250514",
      "temperature": 0.2,
      "max_steps": 200,
      "skills": ["rust-review"],
      "permission": [
        { "permission": "file:write", "pattern": "src/**", "action": "allow" }
      ]
    }
  }
}
```

#### `AgentConfig` schema

| Field | Type | Default | Description |
| ----- | ---- | ------- | ----------- |
| `model` | `Option<String>` | `None` | Model in `"provider:model"` format. |
| `variant` | `Option<String>` | `None` | Agent variant selector. |
| `prompt` | `Option<String>` | `None` | System prompt override. |
| `temperature` | `Option<f32>` | `None` | Sampling temperature override. |
| `top_p` | `Option<f32>` | `None` | Top-p (nucleus) sampling override. |
| `mode` | `Option<String>` | `None` | Agent mode: `"primary"`, `"subagent"`, or `"all"`. |
| `hidden` | `bool` | `false` | Hide from user-facing listings. |
| `permission` | `Vec<PermissionRule>` | `[]` | Permission rules specific to this agent. |
| `max_steps` | `Option<u32>` | `None` | Maximum agentic loop iterations. |
| `skills` | `Vec<String>` | `[]` | Skill names to preload into the prompt. |
| `options` | `HashMap<String, Value>` | `{}` | Arbitrary agent-specific options. |

For custom agents defined as JSON/Markdown files (OASF format), see
[`docs/howtos/custom-agents.md`](custom-agents.md).

---

### 7.5 `permission`

Global permission rules applied to all agents. Rules are evaluated
last-match-wins using glob patterns.

```json
{
  "permission": [
    { "permission": "file:write", "pattern": "src/**", "action": "allow" },
    { "permission": "file:write", "pattern": "secrets/**", "action": "deny" },
    { "permission": "bash", "pattern": "git push --force", "action": "deny" },
    { "permission": "*", "pattern": "/etc/**", "action": "deny" }
  ]
}
```

#### `PermissionRule` schema

| Field | Type | Description |
| ----- | ---- | ----------- |
| `permission` | `String` (parsed to `Permission`) | The permission type. Supports flat names (`read`, `bash`) and namespaced categories (`file:read`, `file:write`). |
| `pattern` | `Option<String>` | Glob pattern matched against the resource path. `None` matches all. |
| `action` | `PermissionAction` | `allow`, `deny`, or `ask`. |

#### Standard permission types

| Name | Permission variant | Description |
| ---- | ------------------ | ----------- |
| `read` / `file:read` | `Read` | Read access to files/resources. |
| `edit` / `write` / `file:write` | `Edit` | Write/edit access. |
| `bash` / `execute` | `Bash` | Shell command execution. |
| `web` / `fetch` | `Web` | Network/web access. |
| `question` | `Question` | Interactive question to user. |
| `plan_enter` / `plan` | `PlanEnter` | Enter planning phase. |
| `plan_exit` | `PlanExit` | Exit planning phase. |
| `task` | `Task` | Create/modify task items. |
| `external_directory` | `ExternalDirectory` | Access dirs outside project root. |
| `doom_loop` | `DoomLoop` | Detect/break infinite loops. |
| `*` | `Custom("*")` | Wildcard: applies to all permissions. |
| any other string | `Custom(String)` | User-defined permission type. |

#### Evaluation order

1. **Permanent grants** (runtime "always allow" recorded via
   `PermissionChecker::record_always`) are checked first.
2. **Permission-specific rules** are evaluated last-match-wins.
3. **Wildcard rules** (`permission: "*"`) are evaluated last.
4. If no rule matches, the result is `ask`.

For the full permission system, YOLO mode, and autopilot, see
[`docs/howtos/permissions.md`](permissions.md).

---

### 7.6 `command`

User-defined slash-command shortcuts. Each entry maps a slash command name to
a shell command.

```json
{
  "command": {
    "build": { "command": "cargo build --release", "description": "Build release binary" },
    "test": { "command": "cargo test -- --nocapture", "description": "Run all tests with output" }
  }
}
```

#### `CommandDef` schema

| Field | Type | Description |
| ----- | ---- | ----------- |
| `command` | `String` | Shell command to execute. |
| `description` | `String` | Human-readable description shown in help output. |

---

### 7.7 `mcp`

MCP (Model Context Protocol) server definitions, keyed by server id.

```json
{
  "mcp": {
    "filesystem": {
      "type": "stdio",
      "command": "npx",
      "args": ["-y", "@modelcontextprotocol/server-filesystem", "/home/user/docs"],
      "env": {}
    },
    "remote-api": {
      "type": "sse",
      "url": "https://mcp.example.com/sse",
      "headers": { "Authorization": "Bearer token123" },
      "disabled": false,
      "notification": "inject_summary"
    }
  }
}
```

#### `McpServerConfig` schema

| Field | Type | Default | Description |
| ----- | ---- | ------- | ----------- |
| `type` | `McpTransport` | `stdio` | Transport: `stdio`, `sse`, or `http`. |
| `command` | `Option<String>` | `None` | Executable path/name (stdio). |
| `args` | `Vec<String>` | `[]` | Command-line arguments (stdio). |
| `env` | `HashMap<String, String>` | `{}` | Environment variables (stdio). |
| `url` | `Option<String>` | `None` | URL endpoint (SSE/HTTP). |
| `headers` | `HashMap<String, String>` | `{}` | HTTP headers (SSE/HTTP). |
| `disabled` | `bool` | `false` | If `true`, server is configured but not started. |
| `notification` | `McpNotificationMode` | `none` | Push notification handling: `none`, `inject_summary`, `inject_and_run`. |

---

### 7.8 `instructions`

Additional instruction strings appended to agent system prompts.

```json
{
  "instructions": [
    "Always use rustfmt before writing Rust files.",
    "Prefer iterator adapters over manual loops."
  ]
}
```

Type: `Vec<String>`. Default: `[]`. These are appended to every agent's system
prompt, merged (union) across config layers.

---

### 7.9 `skill_dirs`

Additional directories to scan for skill definitions, beyond the built-in
locations.

```json
{
  "skill_dirs": ["~/my-skills", "/shared/team-skills"]
}
```

Type: `Vec<String>`. Default: `[]`. Merged (union) across config layers.

---

### 7.10 `experimental`

Feature flags for experimental functionality.

```json
{
  "experimental": {
    "open_telemetry": false,
    "parallel_tool_calls": false,
    "max_background_agents": 8,
    "background_agent_timeout": 3600
  }
}
```

#### `ExperimentalFlags` schema

| Field | Type | Default | Description |
| ----- | ---- | ------- | ----------- |
| `open_telemetry` | `bool` | `false` | Enable OpenTelemetry trace export (legacy; prefer `telemetry.otel`). |
| `parallel_tool_calls` | `bool` | `false` | Allow multiple tool calls from a single model turn to execute in parallel. |
| `max_background_agents` | `usize` | `8` | Maximum concurrent background sub-agent tasks. |
| `background_agent_timeout` | `u64` | `3600` | Timeout in seconds for background sub-agent tasks. |

`open_telemetry` is mapped as a deprecated alias: if `telemetry.otel` is not
explicitly enabled but `experimental.open_telemetry` is `true`, telemetry is
enabled with default settings and a deprecation warning is emitted.

---

### 7.11 `hooks`

Lifecycle hooks. Currently a placeholder (`Vec<serde_json::Value>`). See
[`docs/howtos/hooks.md`](hooks.md) for the hooks system documentation.

Type: `Vec<Value>`. Default: `[]`. Merged (append) across config layers.

---

### 7.12 `bash`

User-defined bash command allowlist and denylist additions.

```json
{
  "bash": {
    "allowlist": ["curl", "wget"],
    "denylist": ["git push --force", "rm -rf /"]
  }
}
```

#### `BashConfig` schema

| Field | Type | Default | Description |
| ----- | ---- | ------- | ----------- |
| `allowlist` | `Vec<String>` | `[]` | Command prefixes exempted from the banned-command check (e.g. `"curl"`). |
| `denylist` | `Vec<String>` | `[]` | Substring patterns that always reject a command (e.g. `"git push --force"`). |

Both lists are unioned (deduplicated) across global and project configs. The
`/bash add` and `/bash remove` slash commands mutate these lists and persist
changes to the targeted config file.

**Allowlist** entries match the first word of the command. **Denylist**
entries are substring-matched anywhere in the command string.

This interacts with the 7-layer bash security model. See
[`docs/howtos/permissions.md`](permissions.md) for the full model.

---

### 7.13 `dirs`

User-defined directory/file path allowlist and denylist additions.

```json
{
  "dirs": {
    "allowlist": ["src/**", "tests/**", "*.rs"],
    "denylist": ["secrets/**", "/etc/**", ".env"]
  }
}
```

#### `DirsConfig` schema

| Field | Type | Default | Description |
| ----- | ---- | ------- | ----------- |
| `allowlist` | `Vec<String>` | `[]` | Glob patterns for paths automatically allowed (no prompt). |
| `denylist` | `Vec<String>` | `[]` | Glob patterns unconditionally rejected. |

Both lists are unioned across config layers. **Denylist takes precedence over
allowlist.** The `/dirs add` and `/dirs remove` slash commands mutate these
lists.

Built-in denylist patterns (always active, not configurable) include system
directories: `/bin/**`, `/sbin/**`, `/boot/**`, `/dev/**`, `/proc/**`,
`/sys/**`, `/etc/**`, `/usr/bin/**`, `/usr/sbin/**`, `/usr/lib/**`, `/lib/**`,
`/lib64/**`, and their macOS/Windows equivalents.

---

### 7.14 `tool_visibility`

Controls which tool families are advertised to the LLM. When a switch is
`false`, all tools in that family are hidden from tool definitions and
system-prompt listings. Hidden tools remain registered and executable.

```json
{
  "tool_visibility": {
    "office": true,
    "github": true,
    "gitlab": false,
    "teams": true,
    "agents": true,
    "plan": true,
    "codeindex": true,
    "masterfetch": true,
    "browser": true,
    "finance": true
  }
}
```

#### Switches and defaults

| Switch | Default | Tools governed |
| ------ | ------- | -------------- |
| `office` | `false` | `office_read`, `office_write`, `office_info`, `libre_read`, `libre_write`, `libre_info`, `pdf_read`, `pdf_write` |
| `github` | `false` | `github_list_issues`, `github_get_issue`, `github_create_issue`, `github_comment_issue`, `github_close_issue`, `github_list_prs`, `github_get_pr`, `github_create_pr`, `github_merge_pr`, `github_review_pr` |
| `gitlab` | `false` | 19 GitLab tools (issues, MRs, pipelines, jobs) |
| `teams` | `false` | 20 team coordination tools |
| `agents` | `false` | `cancel_agent`, `list_agents`, `new_agent`, `agent_complete`, `wait_agents` |
| `plan` | `false` | `plan_enter`, `plan_exit` |
| `codeindex` | `true` | `codeindex_search`, `codeindex_status`, `codeindex_symbols`, `codeindex_references`, `codeindex_dependencies`, `codeindex_reindex` |
| `masterfetch` | `true` | `mf_fetch`, `mf_crawl`, `mf_search`, `mf_screenshot`, `mf_cache_clear`, `mf_version` |
| `browser` | `true` | `browser` |
| `finance` | `true` | `stock_quote`, `stock_history`, `stock_fundamentals`, `currency_rate`, `currency_history`, `stock_search`, `stock_options` |

The `codeindex`, `masterfetch`, `browser`, and `finance` switches are
serialised **only** when explicitly set (tracked by `specified` flags). This
lets the default config omit the key so code-level default changes propagate,
while ensuring an explicit user toggle persists on save.

The `/tools` slash command toggles these at runtime. See
[`docs/howtos/tool-visibility.md`](tool-visibility.md) and
[`docs/howtos/tools.md`](tools.md).

---

### 7.15 `code_index`

Persistent configuration for the codebase indexing subsystem.

```json
{
  "code_index": {
    "enabled": true,
    "max_file_size": 1048576,
    "extra_exclude_dirs": ["vendor", "node_modules"],
    "extra_exclude_patterns": ["*.generated.rs"]
  }
}
```

#### `CodeIndexConfig` schema

| Field | Type | Default | Description |
| ----- | ---- | ------- | ----------- |
| `enabled` | `bool` | `true` | Whether code indexing is enabled. |
| `max_file_size` | `u64` | `1048576` (1 MB) | Maximum file size in bytes to index. |
| `extra_exclude_dirs` | `Vec<String>` | `[]` | Additional directory names to exclude from scanning. |
| `extra_exclude_patterns` | `Vec<String>` | `[]` | Additional glob patterns to exclude. |

The `enabled` field is serialised only when explicitly set (tracked by
`specified.enabled`), so `/codeindex on`/`/codeindex off` toggles persist on
save.

For the full code index guide, see [`docs/howtos/codeindex.md`](codeindex.md).

---

### 7.16 `stream`

LLM streaming behaviour: timeouts and retries.

```json
{
  "stream": {
    "initial_response_timeout_secs": 300,
    "timeout_secs": 120,
    "max_retries": 4,
    "retry_backoff_secs": 2
  }
}
```

#### `StreamConfig` schema

| Field | Type | Default | Validation | Description |
| ----- | ---- | ------- | ---------- | ----------- |
| `initial_response_timeout_secs` | `u64` | `300` | >= 5, >= `timeout_secs` | Seconds the HTTP client waits for the **first byte** of a streaming response. |
| `timeout_secs` | `u64` | `120` | >= 5 | Seconds of silence between stream deltas before a stream is considered stalled. |
| `max_retries` | `u32` | `4` | <= 32 | Maximum retry attempts after a stall or connection failure. |
| `retry_backoff_secs` | `u64` | `2` | — | Backoff multiplier per retry. Attempt N waits `N * retry_backoff_secs` seconds. |

`validate()` returns a list of problems; the session processor refuses to
start if any are present.

---

### 7.17 `memory`

Memory system configuration: blocks, structured store, semantic search,
retrieval, auto-extraction, decay, and cross-project sharing.

```json
{
  "memory": {
    "enabled": true,
    "tier": "core",
    "structured": { "enabled": true },
    "semantic": {
      "enabled": false,
      "model": "all-MiniLM-L6-v2",
      "dimensions": 384
    },
    "retrieval": {
      "max_memories_per_prompt": 5,
      "recency_weight": 0.3,
      "relevance_weight": 0.7
    },
    "auto_extract": {
      "enabled": false,
      "require_confirmation": true
    },
    "decay": {
      "factor": 0.95,
      "min_confidence": 0.1
    },
    "cross_project": {
      "enabled": false,
      "search_global": true,
      "project_override": true
    }
  }
}
```

#### `MemoryConfig` schema

| Field | Type | Default | Description |
| ----- | ---- | ------- | ----------- |
| `enabled` | `bool` | `true` | Master switch for the memory system. |
| `tier` | `String` | `"core"` | Memory tier: `"core"` (file blocks), `"structured"` (SQLite store), `"semantic"` (with embeddings). |
| `structured` | `StructuredMemoryConfig` | `{ enabled: true }` | Structured store configuration. |
| `retrieval` | `RetrievalConfig` | (see below) | Retrieval configuration for prompt injection. |
| `semantic` | `SemanticConfig` | (see below) | Semantic search (embedding) configuration. |
| `auto_extract` | `AutoExtractConfig` | (see below) | Automatic memory extraction. |
| `decay` | `DecayConfig` | (see below) | Confidence decay. |
| `cross_project` | `CrossProjectConfig` | (see below) | Cross-project memory sharing. |

#### Sub-schemas

**`StructuredMemoryConfig`**: `enabled: bool` (default `true`).

**`RetrievalConfig`**:

| Field | Type | Default | Description |
| ----- | ---- | ------- | ----------- |
| `max_memories_per_prompt` | `usize` | `5` | Max structured memories injected into the system prompt. |
| `recency_weight` | `f64` | `0.3` | Weight for recency when ranking (0.0-1.0). |
| `relevance_weight` | `f64` | `0.7` | Weight for relevance when ranking (0.0-1.0). |

**`SemanticConfig`**:

| Field | Type | Default | Description |
| ----- | ---- | ------- | ----------- |
| `enabled` | `bool` | `false` | Whether semantic search via embeddings is enabled. Requires the `embeddings` Cargo feature. |
| `model` | `String` | `"all-MiniLM-L6-v2"` | ONNX sentence-transformer model name. |
| `dimensions` | `usize` | `384` | Embedding vector dimensions (must match model output). |

**`AutoExtractConfig`**:

| Field | Type | Default | Description |
| ----- | ---- | ------- | ----------- |
| `enabled` | `bool` | `false` | Whether automatic memory extraction is enabled. |
| `require_confirmation` | `bool` | `true` | Whether extracted candidates require explicit confirmation before storage. |

**`DecayConfig`**:

| Field | Type | Default | Description |
| ----- | ---- | ------- | ----------- |
| `factor` | `f64` | `0.95` | Multiplicative decay factor per day since last access. 1.0 disables decay. |
| `min_confidence` | `f64` | `0.1` | Minimum confidence floor. Memories never decay below this. |

**`CrossProjectConfig`**:

| Field | Type | Default | Description |
| ----- | ---- | ------- | ----------- |
| `enabled` | `bool` | `false` | Whether cross-project memory sharing is enabled. |
| `search_global` | `bool` | `true` | Whether search includes global memories. |
| `project_override` | `bool` | `true` | Whether project-specific blocks override global blocks with the same label. |

---

### 7.18 `compaction`

Context-window compaction configuration (OpenCode-derived summarisation).

```json
{
  "compaction": {
    "auto": true,
    "threshold": 0.7,
    "buffer": 0.10,
    "keep": { "tokens": 0.20 }
  }
}
```

#### `CompactionConfig` schema

| Field | Type | Default | Description |
| ----- | ---- | ------- | ----------- |
| `auto` | `bool` | `true` | Whether automatic pre-send compaction is enabled. When `false`, only provider context-overflow errors trigger emergency compaction. |
| `threshold` | `Option<f64>` | `Some(0.7)` | Fraction of context window at which to trigger compaction (0.0-1.0). `None` falls back to buffer-based trigger. Clamped to minimum 0.7 so routine prompts under 70% never trigger compaction. |
| `buffer` | `f64` | `0.10` | Token buffer as a fraction of context window. When `threshold` is `None`, triggers when tokens exceed `context_window - max(output_tokens, context_window * buffer)`. |
| `keep` | `KeepConfig` | `{ tokens: Some(0.20) }` | Recent turns kept verbatim after compaction. |

**`KeepConfig`**: `tokens: Option<f64>` (default `Some(0.20)`) — max fraction
of context window preserved as the verbatim tail.

Fixed constants:
- `summary_output_tokens()`: 4096 tokens requested for a compaction summary.
- `tool_output_max_chars()`: 2000 chars — long tool outputs are truncated
  before being serialised into the compaction prompt.

---

### 7.19 `gitlab`

GitLab integration configuration. Values set here override those stored in
the ragent database (set via `/gitlab setup`). Environment variables take the
highest priority.

```json
{
  "gitlab": {
    "instance_url": "https://gitlab.example.com",
    "token": "glpat-xxxxxxxxxxxx",
    "username": "myuser"
  }
}
```

#### `GitLabIntegrationConfig` schema

| Field | Type | Default | Description |
| ----- | ---- | ------- | ----------- |
| `instance_url` | `Option<String>` | `None` | GitLab instance base URL. |
| `token` | `Option<String>` | `None` | Personal Access Token. |
| `username` | `Option<String>` | `None` | GitLab username / identity. |

Env var overrides: `GITLAB_TOKEN`, `GITLAB_URL`, `GITLAB_USERNAME`.

---

### 7.20 `hidden_tools`

Tool names to hide from the LLM. Hidden tools remain registered and
executable; they are simply not advertised to the model.

```json
{
  "hidden_tools": ["github_list_issues", "github_get_issue", "gitlab_list_mrs"]
}
```

Type: `Vec<String>`. Default: `[]`. Unioned (deduplicated) across config
layers.

The `effective_hidden_tools()` method also computes tools hidden by
`tool_visibility` switches set to `false`.

---

### 7.21 `yolo`

YOLO mode bypasses all command validation and tool restrictions.

```json
{ "yolo": false }
```

Type: `bool`. Default: `false`.

When enabled, the following safety checks are skipped:
- Bash denied patterns (destructive commands like `rm -rf /` are allowed)
- Dynamic context allowlist (any executable can run in skill bodies)
- MCP config validation (shell metacharacters and unvalidated paths permitted)

**Warning**: This is inherently dangerous. Use only when you trust the agent
and its inputs completely, or for local development/debugging.

The `Alt+Y` keybinding and `/yolo` slash command toggle this at runtime and
persist the change. See [`docs/howtos/permissions.md`](permissions.md).

---

### 7.22 `edit_log`

Edit-operation logging for `edit` and `multi_edit` tools.

```json
{ "edit_log": false }
```

Type: `bool`. Default: `false`.

When enabled, all edit operations are logged. The `Alt+E` keybinding and
`/editlog` slash command toggle this at runtime and persist the change.

---

### 7.23 `prices`

User-defined price overrides for cost estimation. Each entry overrides the
built-in price table for a specific model. Prices are in USD per 1,000,000
tokens.

```json
{
  "prices": [
    { "model": "gpt-4o", "input_per_1m": 2.50, "output_per_1m": 10.00 },
    { "model": "claude-sonnet-4-20250514", "input_per_1m": 3.00, "output_per_1m": 15.00 }
  ]
}
```

#### `PriceEntry` schema

| Field | Type | Description |
| ----- | ---- | ----------- |
| `model` | `String` | Model identifier as returned by the provider. |
| `input_per_1m` | `f64` | Price per 1M input/prompt tokens (USD). |
| `output_per_1m` | `f64` | Price per 1M output/completion tokens (USD). |

When merged, overlay entries replace base entries with the same model id
(last-wins per model).

---

### 7.24 `browser`

Browser automation configuration (Chrome DevTools Protocol endpoint).

```json
{
  "browser": {
    "cdp_endpoint": "http://127.0.0.1:9222",
    "default_headless": true
  }
}
```

#### `BrowserConfig` schema

| Field | Type | Default | Description |
| ----- | ---- | ------- | ----------- |
| `cdp_endpoint` | `Option<String>` | `None` | CDP HTTP endpoint URL. When `None`/empty, defaults to `http://127.0.0.1:9222`. |
| `default_headless` | `bool` | `true` | Default headless mode for the `setup` action. |

---

### 7.25 `channels`

External messaging channel configuration for the `send_channel_message` tool.

```json
{
  "channels": {
    "enabled": true,
    "telegram": { "bot_token": "env:TELEGRAM_BOT_TOKEN", "chat_id": "-100123" },
    "discord": { "webhook_url": "https://discord.com/api/webhooks/..." }
  }
}
```

#### `ChannelsConfig` schema

| Field | Type | Default | Description |
| ----- | ---- | ------- | ----------- |
| `enabled` | `bool` | `false` | Master switch for the channel messaging tool. |
| `telegram` | `Option<TelegramChannelConfig>` | `None` | Telegram bot channel. |
| `discord` | `Option<DiscordChannelConfig>` | `None` | Discord webhook channel. |

**`TelegramChannelConfig`**: `bot_token: Option<String>`, `chat_id:
Option<String>`, `base_url: Option<String>` (defaults to
`https://api.telegram.org`).

**`DiscordChannelConfig`**: `webhook_url: Option<String>`.

All token/secret fields support the `env:VAR_NAME` indirection — the value is
read from the named environment variable at use time, so secrets do not need
to live in the config file.

---

### 7.26 `gmail`

Gmail tool configuration (OAuth2 client credentials).

```json
{
  "gmail": {
    "client_id": "...apps.googleusercontent.com",
    "client_secret": "env:GMAIL_CLIENT_SECRET"
  }
}
```

#### `GmailConfig` schema

| Field | Type | Default | Description |
| ----- | ---- | ------- | ----------- |
| `client_id` | `Option<String>` | `None` | OAuth2 client ID. Supports `env:VAR_NAME`. Falls back to `GMAIL_CLIENT_ID`. |
| `client_secret` | `Option<String>` | `None` | OAuth2 client secret. Supports `env:VAR_NAME`. Falls back to `GMAIL_CLIENT_SECRET`. |
| `base_url` | `Option<String>` | `None` | Optional HTTP endpoint override (defaults to `https://gmail.googleapis.com`). |

The OAuth2 access/refresh tokens are managed by the `gmail` tool itself
(`auth`/`status`/`logout` actions) and stored encrypted in `ragent-storage` —
never in this file.

---

### 7.27 `telemetry`

OpenTelemetry metrics export configuration.

```json
{
  "telemetry": {
    "otel": {
      "enabled": false,
      "endpoint": "http://localhost:4318",
      "protocol": "http",
      "export_interval_seconds": 30,
      "export_timeout_seconds": 10,
      "service_name": "ragent",
      "resource_attributes": { "deployment.environment": "production" },
      "metrics": { "ragent.tool.call_count": false },
      "internal_port": null,
      "cardinality_limit": 1000
    }
  }
}
```

#### `TelemetryConfig` schema

Wraps `OtelConfig` as `telemetry.otel`.

#### `OtelConfig` schema

| Field | Type | Default | Description |
| ----- | ---- | ------- | ----------- |
| `enabled` | `bool` | `false` | Master on/off switch. |
| `endpoint` | `String` | `"http://localhost:4318"` | OTLP endpoint base URL. Must be `http://` or `https://`. |
| `protocol` | `OtelProtocol` | `http` | Transport: `http` or `grpc`. |
| `export_interval_seconds` | `u64` | `30` | Batch export interval. Must be > 0 when enabled. |
| `export_timeout_seconds` | `u64` | `10` | Per-export request timeout. Must be > 0 when enabled. |
| `service_name` | `String` | `"ragent"` | `service.name` resource attribute. Must not be empty. |
| `resource_attributes` | `HashMap<String, String>` | `{}` | Custom resource attributes appended to every metric. |
| `metrics` | `HashMap<String, bool>` | `{}` | Per-metric enable/disable toggles. Absent = enabled by default. |
| `internal_port` | `Option<u16>` | `None` | Optional in-process Prometheus text endpoint port. |
| `cardinality_limit` | `usize` | `1000` | Max distinct attribute combinations per metric before overflow into `unknown` bucket. |

The legacy `experimental.open_telemetry` flag is mapped as a deprecated alias
via `apply_legacy_flag()`.

---

### 7.28 `agent_perf`

Agent-loop performance configuration.

```json
{
  "agent_perf": {
    "enabled": true,
    "profiling": false,
    "step_budget_secs": 300,
    "stall_timeout_secs": 60,
    "max_concurrent_tools": 4,
    "parallel_independent_tools": true
  }
}
```

#### `AgentPerfConfig` schema

| Field | Type | Default | Validation | Description |
| ----- | ---- | ------- | ---------- | ----------- |
| `enabled` | `bool` | `true` | — | Master switch. When `false`, all performance optimisations short-circuit. |
| `profiling` | `bool` | `false` | — | Emit detailed per-scope timing logs at `info` level. |
| `step_budget_secs` | `u64` | `300` | >= 5 | Maximum wall-clock seconds per agent step. |
| `stall_timeout_secs` | `u64` | `60` | >= 5 | Maximum seconds without a stream delta before stall recovery fires. |
| `max_concurrent_tools` | `u32` | `min(available_parallelism, 4)` | >= 1 | Maximum parallel tool calls per turn. |
| `parallel_independent_tools` | `bool` | `true` | — | Execute independent tool calls in parallel. |

`validate()` returns a list of problems; the agent loop refuses to start if
any are present.

---

### 7.29 `finance`

Paid finance-provider configuration for the stock/currency toolset.

```json
{
  "finance": {
    "provider": "alpha_vantage",
    "api_key": "env:ALPHA_VANTAGE_API_KEY",
    "requests_per_minute": 5,
    "min_call_interval_seconds": 5,
    "yahoo_fallback": true
  }
}
```

#### `FinanceProviderConfig` schema

| Field | Type | Default | Description |
| ----- | ---- | ------- | ----------- |
| `provider` | `String` | `"yahoo"` | Selected provider: `"yahoo"` (free, default) or `"alpha_vantage"`. |
| `api_key` | `Option<String>` | `None` | API key when a paid provider is selected. |
| `base_url` | `Option<String>` | `None` | Optional base URL override for the paid provider. |
| `requests_per_minute` | `Option<u32>` | `None` | Optional request rate limit. |
| `user_agent` | `Option<String>` | `None` | Optional custom User-Agent header for the free Yahoo provider. |
| `min_call_interval_seconds` | `u64` | `5` | Minimum seconds between any two finance API calls. |
| `yahoo_fallback` | `Option<bool>` | `None` | Whether to fall back to Yahoo when the paid provider fails. `None` = enabled for Yahoo, disabled for paid providers. |

---

### 7.30 Search API keys

Several API keys for the `mf_search` / `websearch` tools can be stored in the
config file. Environment variables always take precedence.

| Field | Type | Default | Env var override | Description |
| ----- | ---- | ------- | ---------------- | ----------- |
| `tavily_api_key` | `Option<String>` | `None` | `TAVILY_API_KEY` | Tavily search API key for the `websearch` tool. |
| `langsearch_api_key` | `Option<String>` | `None` | — | LangSearch API key for `mf_search`. Masked in diagnostics. |
| `perplexity_api_key` | `Option<String>` | `None` | — | Perplexity Sonar API key for `mf_search`. Masked in diagnostics. |
| `exa_api_key` | `Option<String>` | `None` | `EXA_API_KEY` | Exa Search API key for `mf_search`. Masked in diagnostics. |
| `openalex_email` | `Option<String>` | `None` | `OPENALEX_EMAIL` | OpenAlex polite-pool email for `mf_search`. Masked in diagnostics. |

When present, `mf_search` queries the corresponding backend as an additional
search engine alongside the keyless backends (DuckDuckGo, Brave, OpenAlex,
Wikipedia).

---

### 7.31 `sdd`

Spec-Driven Development (SDD) capability toggles. All flags default to `false`
(opt-in).

```json
{
  "sdd": {
    "clarification_markers": true,
    "quality_checklists": true,
    "constitution": true,
    "phase_minus_one_gates": true,
    "branch_per_spec": true,
    "research_artifacts": true,
    "data_model": true,
    "contracts": true,
    "quickstart": true,
    "test_first_ordering": true,
    "consistency_checks": true
  }
}
```

#### `SddConfig` schema

| Field | Type | Default | Description |
| ----- | ---- | ------- | ----------- |
| `clarification_markers` | `bool` | `false` | Enable `[NEEDS CLARIFICATION]` marker detection (FR-002). |
| `quality_checklists` | `bool` | `false` | Embed quality checklists in spec/plan templates (FR-006). |
| `constitution` | `bool` | `false` | Generate and parse `CONSTITUTION.md` (FR-007). |
| `phase_minus_one_gates` | `bool` | `false` | Enable Phase -1 pre-implementation gate validation (FR-008). |
| `branch_per_spec` | `bool` | `false` | Create a git branch per spec (FR-009). |
| `research_artifacts` | `bool` | `false` | Link research artifacts into SPEC.md frontmatter (FR-010). |
| `data_model` | `bool` | `false` | Generate `data-model.md` during `/spec plan` (FR-011). |
| `contracts` | `bool` | `false` | Generate `contracts/` directory during `/spec plan` (FR-012). |
| `quickstart` | `bool` | `false` | Generate `quickstart.md` validation scenarios (FR-013). |
| `test_first_ordering` | `bool` | `false` | Enforce test-first file creation ordering in plans (FR-014). |
| `consistency_checks` | `bool` | `false` | Run ambiguity, contradiction, and gap consistency checks (FR-015). |

Merge uses OR semantics: a flag enabled in either base or overlay stays
enabled.

For the spec system, see [`docs/howtos/spec.md`](spec.md).

---

### 7.32 `trigger`

Dynamic trigger rule system configuration.

```json
{
  "trigger": {
    "enabled": true,
    "poll_interval_secs": 30,
    "max_rules": 32
  }
}
```

#### `TriggerConfig` schema

| Field | Type | Default | Description |
| ----- | ---- | ------- | ----------- |
| `enabled` | `bool` | `true` | Master feature gate. When `false`, all trigger functionality no-ops. |
| `poll_interval_secs` | `u64` | `30` | Interval at which dynamic trigger rules poll their conditions. |
| `max_rules` | `usize` | `32` | Maximum number of dynamic trigger rules per session. |

`is_empty()` returns `true` when all fields are at default values, so the
section is omitted from serialised output when nothing is configured.

---

### 7.33 `piegap`

Pie feature gap toggles. Each flag gates a standalone pie-derived feature. All
default to `false` (opt-in).

```json
{
  "piegap": {
    "triggers": true,
    "mcp_notifications": true,
    "inbox": true,
    "hooks": true,
    "archive": true,
    "bug_report": true,
    "templates": true,
    "goal": true,
    "web_ui": true,
    "undo": true,
    "session_naming": true
  }
}
```

#### `PieGapConfig` schema

| Field | Type | Default | Description |
| ----- | ---- | ------- | ----------- |
| `triggers` | `bool` | `false` | Enable dynamic trigger rules (G-01). |
| `mcp_notifications` | `bool` | `false` | Enable MCP notification push events (G-02). |
| `inbox` | `bool` | `false` | Enable stateful loops + triage inbox (G-03). |
| `hooks` | `bool` | `false` | Enable lifecycle hooks (G-04). |
| `archive` | `bool` | `false` | Enable portable session archive export/import (G-05). |
| `bug_report` | `bool` | `false` | Enable bug report generation (G-06). |
| `templates` | `bool` | `false` | Enable reusable prompt templates (G-07). |
| `goal` | `bool` | `false` | Enable goal-based autonomous stop hook (G-10). |
| `web_ui` | `bool` | `false` | Enable browser-based web UI (G-12). |
| `undo` | `bool` | `false` | Enable `/undo` slash command (G-13). |
| `session_naming` | `bool` | `false` | Enable session naming (G-14). |

Merge uses OR semantics. `is_empty()` returns `true` when no flags are
enabled.

---

### 7.34 `research`

Research subsystem configuration.

```json
{
  "research": {
    "open_access_recovery": true,
    "contact_email": "user@example.com",
    "oa_min_full_text_chars": 1000
  }
}
```

#### `ResearchConfig` schema

| Field | Type | Default | Description |
| ----- | ---- | ------- | ----------- |
| `open_access_recovery` | `bool` | `false` | Enable open-access recovery via Unpaywall and Europe PMC for short scholarly sources (FR-011). |
| `contact_email` | `Option<String>` | `None` | Contact email required by Unpaywall's terms of service (FR-012). |
| `oa_min_full_text_chars` | `usize` | `1000` | Minimum full-text length (chars) that triggers OA recovery. |

`open_access_recovery` uses OR semantics on merge. `contact_email` and
`oa_min_full_text_chars` override base when present. `is_empty()` returns
`true` when at default values.

For the research system, see [`docs/howtos/research.md`](research.md).

---

## 8. Full Example File

The following is a comprehensive example showing every section. You do not
need all of these — every section has defaults, so an empty `{}` is valid.

```json
{
  "username": "alice",
  "defaultAgent": "coder",

  "provider": {
    "anthropic": {
      "env": ["ANTHROPIC_API_KEY"],
      "thinking": { "enabled": true, "level": "low" },
      "models": {
        "claude-sonnet-4-20250514": {
          "thinking": {
            "enabled": true,
            "level": "high",
            "budget_tokens": 16000
          }
        }
      }
    },
    "openai": {
      "env": ["OPENAI_API_KEY"]
    },
    "generic_openai": {
      "env": ["GENERIC_OPENAI_API_KEY"],
      "api": { "base_url": "http://127.0.0.1:8080" }
    },
    "router": {
      "enabled": true,
      "tiers": {
        "SIMPLE": {
          "models": [{ "provider": "ollama", "model": "llama3.2" }]
        },
        "MEDIUM": {
          "models": [
            { "provider": "anthropic", "model": "claude-sonnet-4-20250514" }
          ]
        },
        "COMPLEX": {
          "models": [
            { "provider": "anthropic", "model": "claude-sonnet-4-20250514" }
          ]
        },
        "REASONING": {
          "models": [{ "provider": "openai", "model": "o3" }]
        }
      }
    }
  },

  "agent": {
    "coder": {
      "model": "anthropic/claude-sonnet-4-20250514",
      "temperature": 0.2,
      "max_steps": 200,
      "skills": ["rust-review"]
    }
  },

  "permission": [
    { "permission": "file:write", "pattern": "src/**", "action": "allow" },
    { "permission": "file:write", "pattern": "secrets/**", "action": "deny" },
    { "permission": "bash", "pattern": "git push --force", "action": "deny" }
  ],

  "command": {
    "build": {
      "command": "cargo build --release",
      "description": "Build release binary"
    }
  },

  "mcp": {
    "filesystem": {
      "type": "stdio",
      "command": "npx",
      "args": ["-y", "@modelcontextprotocol/server-filesystem", "/home/user/docs"]
    }
  },

  "instructions": [
    "Always use rustfmt before writing Rust files."
  ],

  "skill_dirs": ["~/my-skills"],

  "experimental": {
    "parallel_tool_calls": false,
    "max_background_agents": 8,
    "background_agent_timeout": 3600
  },

  "bash": {
    "allowlist": ["curl", "wget"],
    "denylist": ["git push --force"]
  },

  "dirs": {
    "allowlist": ["src/**", "tests/**"],
    "denylist": ["secrets/**", ".env"]
  },

  "tool_visibility": {
    "office": true,
    "github": true,
    "gitlab": false,
    "teams": true,
    "agents": true,
    "plan": true
  },

  "code_index": {
    "enabled": true,
    "max_file_size": 1048576,
    "extra_exclude_dirs": ["vendor"]
  },

  "stream": {
    "initial_response_timeout_secs": 300,
    "timeout_secs": 120,
    "max_retries": 4,
    "retry_backoff_secs": 2
  },

  "memory": {
    "enabled": true,
    "tier": "core",
    "structured": { "enabled": true },
    "semantic": {
      "enabled": false,
      "model": "all-MiniLM-L6-v2",
      "dimensions": 384
    },
    "retrieval": {
      "max_memories_per_prompt": 5,
      "recency_weight": 0.3,
      "relevance_weight": 0.7
    },
    "auto_extract": {
      "enabled": false,
      "require_confirmation": true
    },
    "decay": {
      "factor": 0.95,
      "min_confidence": 0.1
    },
    "cross_project": {
      "enabled": false,
      "search_global": true,
      "project_override": true
    }
  },

  "compaction": {
    "auto": true,
    "threshold": 0.7,
    "buffer": 0.10,
    "keep": { "tokens": 0.20 }
  },

  "gitlab": {
    "instance_url": "https://gitlab.example.com",
    "token": "env:GITLAB_TOKEN",
    "username": "myuser"
  },

  "hidden_tools": [],

  "yolo": false,
  "edit_log": false,

  "prices": [
    {
      "model": "gpt-4o",
      "input_per_1m": 2.50,
      "output_per_1m": 10.00
    }
  ],

  "browser": {
    "cdp_endpoint": "http://127.0.0.1:9222",
    "default_headless": true
  },

  "channels": {
    "enabled": true,
    "telegram": {
      "bot_token": "env:TELEGRAM_BOT_TOKEN",
      "chat_id": "-100123"
    }
  },

  "gmail": {
    "client_id": "env:GMAIL_CLIENT_ID",
    "client_secret": "env:GMAIL_CLIENT_SECRET"
  },

  "telemetry": {
    "otel": {
      "enabled": false,
      "endpoint": "http://localhost:4318",
      "protocol": "http",
      "export_interval_seconds": 30,
      "export_timeout_seconds": 10,
      "service_name": "ragent",
      "cardinality_limit": 1000
    }
  },

  "agent_perf": {
    "enabled": true,
    "step_budget_secs": 300,
    "stall_timeout_secs": 60,
    "max_concurrent_tools": 4,
    "parallel_independent_tools": true
  },

  "finance": {
    "provider": "yahoo",
    "min_call_interval_seconds": 5
  },

  "tavily_api_key": null,
  "langsearch_api_key": null,
  "perplexity_api_key": null,
  "exa_api_key": null,
  "openalex_email": null,

  "sdd": {
    "clarification_markers": false,
    "quality_checklists": false
  },

  "trigger": {
    "enabled": true,
    "poll_interval_secs": 30,
    "max_rules": 32
  },

  "piegap": {
    "triggers": false,
    "hooks": false,
    "undo": false
  },

  "research": {
    "open_access_recovery": false,
    "contact_email": null,
    "oa_min_full_text_chars": 1000
  }
}
```

---

## 9. Common Recipes

### Minimal config (just an API key reference)

```json
{
  "provider": {
    "anthropic": { "env": ["ANTHROPIC_API_KEY"] }
  }
}
```

The API key itself is read from the environment variable; it does not need to
be in the config file.

### Local-first (Ollama only)

```json
{
  "defaultAgent": "coder",
  "provider": {
    "ollama": {}
  }
}
```

ragent resolves to the first available local/self-hosted provider when no
model is explicitly configured.

### Custom OpenAI-compatible endpoint

```json
{
  "provider": {
    "generic_openai": {
      "env": ["GENERIC_OPENAI_API_KEY"],
      "api": { "base_url": "http://127.0.0.1:8080" }
    }
  }
}
```

The base URL can also be set via `GENERIC_OPENAI_API_BASE` env var.

### Allow writes to src/, deny secrets

```json
{
  "permission": [
    { "permission": "file:write", "pattern": "src/**", "action": "allow" },
    { "permission": "file:write", "pattern": "secrets/**", "action": "deny" }
  ]
}
```

### Enable GitHub and teams tools

```json
{
  "tool_visibility": {
    "github": true,
    "teams": true
  }
}
```

### Disable code indexing

```json
{
  "code_index": { "enabled": false }
}
```

### Semantic memory with embeddings

```json
{
  "memory": {
    "semantic": {
      "enabled": true,
      "model": "all-MiniLM-L6-v2",
      "dimensions": 384
    }
  }
}
```

Requires the `embeddings` Cargo feature. When disabled, memory search falls
back to FTS5-only mode.

### OpenTelemetry export to a collector

```json
{
  "telemetry": {
    "otel": {
      "enabled": true,
      "endpoint": "http://otel-collector:4318",
      "protocol": "http",
      "export_interval_seconds": 15,
      "service_name": "ragent-prod",
      "resource_attributes": {
        "deployment.environment": "production",
        "host.name": "build-server-01"
      }
    }
  }
}
```

### Telegram notifications

```json
{
  "channels": {
    "enabled": true,
    "telegram": {
      "bot_token": "env:TELEGRAM_BOT_TOKEN",
      "chat_id": "env:TELEGRAM_CHAT_ID"
    }
  }
}
```

### Model Router with fallback chains

```json
{
  "provider": {
    "router": {
      "enabled": true,
      "tiers": {
        "SIMPLE": {
          "models": [
            { "provider": "ollama", "model": "llama3.2" },
            { "provider": "openai", "model": "gpt-4o-mini" }
          ]
        },
        "MEDIUM": {
          "models": [
            { "provider": "anthropic", "model": "claude-sonnet-4-20250514" },
            { "provider": "openai", "model": "gpt-4o" }
          ]
        },
        "COMPLEX": {
          "models": [
            { "provider": "anthropic", "model": "claude-sonnet-4-20250514" }
          ]
        },
        "REASONING": {
          "models": [
            { "provider": "openai", "model": "o3" },
            { "provider": "anthropic", "model": "claude-sonnet-4-20250514" }
          ]
        }
      }
    }
  }
}
```

### Override pricing for a custom model

```json
{
  "prices": [
    {
      "model": "my-custom-model",
      "input_per_1m": 1.00,
      "output_per_1m": 5.00
    }
  ]
}
```

### Inline config via environment variable

```bash
RAGENT_CONFIG_CONTENT='{"defaultAgent":"ask","yolo":true}' ragent run "hello"
```

This is merged last, with the highest precedence.

---

## 10. Related Documents

| Document | Topic |
| -------- | ----- |
| [`docs/howtos/tutorial.md`](tutorial.md) | End-to-end tutorial from setup to release |
| [`docs/howtos/permissions.md`](permissions.md) | Permissions, autopilot, and YOLO mode |
| [`docs/howtos/tools.md`](tools.md) | Complete tool reference |
| [`docs/howtos/tool-visibility.md`](tool-visibility.md) | Tool family visibility toggles |
| [`docs/howtos/custom-agents.md`](custom-agents.md) | Custom agent definitions (OASF / Markdown) |
| [`docs/howtos/teams.md`](teams.md) | Teams and swarm coordination |
| [`docs/howtos/hooks.md`](hooks.md) | Lifecycle hooks |
| [`docs/howtos/codeindex.md`](codeindex.md) | Code index (tree-sitter, search, graph) |
| [`docs/howtos/spec.md`](spec.md) | Spec lifecycle management |
| [`docs/howtos/research.md`](research.md) | Research system |
| [`docs/howtos/finance.md`](finance.md) | Stock and currency tools |
| [`docs/howtos/communications.md`](communications.md) | Gmail and messaging channels |
| [`docs/howtos/reverse.md`](reverse.md) | GitHub repo reverse-engineering |
| [`README.md`](../../README.md) | Project overview and quick start |
| [`SPEC.md`](../../SPEC.md) | Full configuration schema reference |
| [`QUICKSTART.md`](../../QUICKSTART.md) | Quick start guide |