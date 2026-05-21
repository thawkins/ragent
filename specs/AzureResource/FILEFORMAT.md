# `azureresources.json` File Format Specification

## Overview

`azureresources.json` is a user-supplied JSON catalog that registers one or more Azure-hosted LLM endpoints (Azure OpenAI Service, Azure AI Foundry, custom endpoints, etc.) with ragent **without** rebuilding the application or modifying source code.

When the `azure_resource` provider is active, ragent reads this file at startup, validates every entry, and exposes each resource as a [`ModelInfo`](../../crates/ragent-llm/src/providers/mod.rs) object in the provider registry. The provider then routes chat-completion requests to the appropriate endpoint using the stored `endpoint` and authentication credentials.

## File Locations (Search Order)

Ragent looks for `azureresources.json` in the following order. The **first file found wins**; no merging is performed.

1. `~/.config/ragent/azureresources.json` — user-global (all projects)
2. `.ragent/azureresources.json` — project-local (current working directory)

If neither file exists, the provider advertises an empty catalog and skips loading.

## Top-Level Structure

```json
{
  "version": "1",
  "resources": [
    { /* AzureResourceEntry */ },
    { /* AzureResourceEntry */ }
  ]
}
```

| Field      | Type   | Required | Description |
|------------|--------|----------|-------------|
| `version`  | string | **Yes**  | Schema version. Must be exactly `"1"`. Any other value causes a fatal parse error. |
| `resources`| array  | **Yes**  | Ordered list of [`AzureResourceEntry`](#azure-resource-entry) objects. |

## Azure Resource Entry

Each object in the `resources` array represents a single deployable Azure LLM endpoint.

### Fields

| Field           | Type     | Required | Default | Description |
|-----------------|----------|----------|---------|-------------|
| `id`            | string   | **Yes**  | —       | Unique identifier for this resource. Becomes the `model_id` used in provider resolution and UI pickers. |
| `name`          | string   | **Yes**  | —       | Human-readable display name (shown in `/model` picker, `--models` list, etc.). |
| `endpoint`      | string   | **Yes**  | —       | Base URL of the Azure resource, **without** the `/openai/v1/chat/completions` suffix. Example: `https://my-resource.openai.azure.com` |
| `api_key`       | string   | No       | `null`  | **Discouraged.** Inline API key. Use `api_key_env` instead to avoid secrets in version control. |
| `api_key_env`   | string   | No       | `null`  | Name of an environment variable that holds the API key (e.g. `"MY_AOAI_KEY"`). |
| `context_window`| integer  | No       | `128000`| Maximum tokens the model accepts in a single request. |
| `capabilities`  | string[] | No       | *(see below)* | Explicit capability tags. When present, **only** the listed capabilities are enabled. When absent, safe defaults are applied. |
| `thinking`      | object   | No       | `null`  | Optional [`ThinkingConfig`](#thinking-config) for reasoning models (o-series, etc.). |

### Authentication Rules

Every entry **must** specify at least one of `api_key` or `api_key_env`. If neither is provided, the entry is skipped with a `tracing::warn!` log.

- **`api_key_env` is preferred** — keeps secrets out of the JSON file.
- If both are present, `api_key_env` is the canonical source at runtime; `api_key` may be ignored depending on the client implementation.

### Capability Semantics

The `capabilities` array is an **opt-in whitelist** of feature strings:

| String      | Maps to `Capabilities` field | Meaning |
|-------------|------------------------------|---------|
| `"reasoning"` | `reasoning: true`            | Supports chain-of-thought / reasoning tokens. |
| `"streaming"` | `streaming: true`              | Supports Server-Sent Events (SSE) streaming. |
| `"vision"`    | `vision: true`                 | Accepts image inputs (base64 PNG/JPG). |
| `"tool_use"`  | `tool_use: true`               | Supports function/tool calling. |

**When `capabilities` is present**, only the explicitly listed flags are set to `true`; everything else is `false`.  
**When `capabilities` is absent**, the following safe defaults are used:

```rust
Capabilities {
    reasoning: false,
    streaming: true,
    vision:    false,
    tool_use:  true,
    thinking_levels: [],
}
```

> **Note:** `thinking_levels` is never populated from the JSON file; it is reserved for provider-specific discovery APIs.

### Thinking Config

The `thinking` object follows the same schema as [`ragent_types::ThinkingConfig`](../../crates/ragent-types/src/thinking.rs):

| Field           | Type     | Required | Default | Description |
|-----------------|----------|----------|---------|-------------|
| `enabled`       | boolean  | No       | `true`  | Master switch. When `false`, `level` is ignored. |
| `level`         | string   | No       | `"auto"`| One of: `"auto"`, `"off"`, `"low"`, `"medium"`, `"high"`. |
| `budget_tokens` | integer  | No       | `null`  | Maximum tokens the model may spend on reasoning (Anthropic-style). |
| `display`       | string   | No       | `null`  | One of: `"full"`, `"summarized"`, `"omitted"`. Controls how reasoning content is surfaced in responses. |

## Validation & Error Handling

During parsing, each entry is validated individually:

1. **Mandatory fields** — `id`, `name`, and `endpoint` must be non-empty after trimming. Empty values cause the entry to be skipped with a warning.
2. **Authentication** — At least one of `api_key` or `api_key_env` must be present. Missing both skips the entry.
3. **Duplicate IDs** — If two entries share the same `id`, the **first one wins**; subsequent duplicates are skipped with a warning.
4. **Fatal errors** — Malformed JSON or an unsupported `version` value causes the entire file parse to fail (returns an empty catalog for the provider).

## Complete Example

```json
{
  "version": "1",
  "resources": [
    {
      "id": "kimi-k2.6",
      "name": "kimi-k2.6",
      "endpoint": "https://a1a-52048-dev-ais-shr1-eus2-1.openai.azure.com",
      "api_key_env": "AZURE_AI_FOUNDRY_API_KEY",
      "context_window": 128000,
      "capabilities": ["reasoning", "streaming", "vision", "tool_use"],
      "thinking": {
        "enabled": true,
        "level": "medium",
        "budget_tokens": 8192
      }
    },
    {
      "id": "my-gpt-4o",
      "name": "My Azure GPT-4o",
      "endpoint": "https://my-resource.openai.azure.com",
      "api_key_env": "MY_AOAI_KEY",
      "context_window": 128000,
      "capabilities": ["streaming", "vision", "tool_use"]
    },
    {
      "id": "minimal-endpoint",
      "name": "Minimal Endpoint",
      "endpoint": "https://minimal.example.com",
      "api_key": "sk-12345"
    }
  ]
}
```

## Mapping to Internal `ModelInfo`

After validation, each [`AzureResourceEntry`](../../crates/ragent-llm/src/providers/azure_resource.rs) is converted into a [`ModelInfo`](../../crates/ragent-llm/src/providers/mod.rs) with the following fixed values:

| `ModelInfo` field       | Value |
|-------------------------|-------|
| `id`                    | `entry.id` |
| `provider_id`           | `"azure_resource"` |
| `name`                  | `entry.name` |
| `cost.input`            | `0.0` (not tracked for file-based resources) |
| `cost.output`           | `0.0` |
| `capabilities`          | Derived from `capabilities` array or safe defaults (see above) |
| `context_window`        | `entry.context_window.unwrap_or(128_000)` |
| `max_output`            | `None` |
| `request_multiplier`    | `None` |
| `thinking_config`       | `entry.thinking` (if present) |

## References

- Source code: [`crates/ragent-llm/src/providers/azure_resource.rs`](../../crates/ragent-llm/src/providers/azure_resource.rs)
- Provider trait & `ModelInfo`: [`crates/ragent-llm/src/providers/mod.rs`](../../crates/ragent-llm/src/providers/mod.rs)
- `ThinkingConfig`: [`crates/ragent-types/src/thinking.rs`](../../crates/ragent-types/src/thinking.rs)
- `Capabilities` & `Cost`: [`crates/ragent-config/src/config.rs`](../../crates/ragent-config/src/config.rs)
