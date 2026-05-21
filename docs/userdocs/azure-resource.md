# Azure Resource Provider

The **Azure Resource (File)** provider lets you register Azure-hosted LLM
endpoints (Azure OpenAI, Azure AI Foundry, custom endpoints) in a JSON file
instead of rebuilding the application.

## Where to place the file

`azureresources.json` is searched in this order:

1. `~/.config/ragent/azureresources.json`
2. `.ragent/azureresources.json` (current working directory)

## Full JSON schema

```json
{
  "version": "1",
  "resources": [
    {
      "id": "my-gpt-4o",
      "name": "My Azure GPT-4o",
      "endpoint": "https://my-resource.openai.azure.com",
      "api_key_env": "MY_AOAI_KEY",
      "context_window": 128000,
      "capabilities": ["streaming", "vision", "tool_use"],
      "thinking": { "enabled": false }
    }
  ]
}
```

### Field reference

| Field | Required | Description |
|---|---|---|
| `version` | ✅ | Must be `"1"`. |
| `id` | ✅ | Unique identifier used as the model ID. |
| `name` | ✅ | Human-readable display name. |
| `endpoint` | ✅ | Base URL for the Azure resource. |
| `api_key` | optional | Inline API key (discouraged — prefer `api_key_env`). |
| `api_key_env` | optional | Name of the environment variable holding the API key. |
| `context_window` | optional | Maximum context-window size in tokens (default: 128000). |
| `capabilities` | optional | Capability tags: `"streaming"`, `"vision"`, `"tool_use"`, `"reasoning"`. |
| `thinking` | optional | Thinking / reasoning configuration. |

### API key resolution

At least one of `api_key` or `api_key_env` must be present for an entry to be
loaded. When the entry is selected in the TUI, the key is resolved in this order:

1. If `api_key` is present, it is used directly.
2. Otherwise, the environment variable named by `api_key_env` is read.
3. If neither is available, a clear error is shown.

### Capabilities

When `capabilities` is explicitly listed, **only** the listed capabilities are
enabled. When omitted, sensible defaults are applied (streaming and tool_use
enabled, reasoning and vision disabled).

### Thinking overrides

The `thinking` block follows the same format as model-specific thinking config in
`ragent.json`:

```json
"thinking": {
  "enabled": true,
  "level": "medium",
  "budget_tokens": 8192
}
```

## Copy-pasteable example

```json
{
  "version": "1",
  "resources": [
    {
      "id": "my-gpt-4o",
      "name": "My Azure GPT-4o",
      "endpoint": "https://my-resource.openai.azure.com",
      "api_key_env": "MY_AOAI_KEY",
      "context_window": 128000,
      "capabilities": ["streaming", "vision", "tool_use"]
    },
    {
      "id": "my-o1",
      "name": "My Azure o1",
      "endpoint": "https://my-o1.openai.azure.com",
      "api_key_env": "MY_O1_KEY",
      "context_window": 200000,
      "capabilities": ["streaming", "reasoning", "tool_use"],
      "thinking": { "enabled": true, "level": "high" }
    }
  ]
}
```

## How it works in the TUI

1. Run `/setup` and choose **Azure Resource (File)**.
2. The TUI reads `azureresources.json` and shows a list of resources.
3. Use ↑/↓ to navigate and **Enter** to select.
4. The selection is persisted across restarts in the database.
5. Chat requests are forwarded to the underlying `azure_foundry` provider using
   the selected endpoint and model ID.

## Troubleshooting

| Symptom | Likely cause |
|---|---|
| "No Azure resources found" | `azureresources.json` is missing or malformed. |
| "Environment variable X is not set" | `api_key_env` references an unset env var. |
| Model has no capabilities | `capabilities` array is empty or omitted. |
