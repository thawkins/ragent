---
status: implemented
---

# Azure Resource Provider API Type Switch – Specification

**Spec ID:** `AzureAPISwitch`

**Created:** 2026-04-21
**Author:** ragent-agent
**Version:** 1.0.0-draft

---

## Executive Summary

This specification extends the existing **Azure Resource Provider** so that users
can declare Anthropic-model endpoints in `azureresources.json`. When an entry's
`api_type` field is set to `"anthropic"`, the provider routes chat-completion
requests to `{endpoint}/anthropic/v1/messages` using the Anthropic Messages API
format, while preserving Azure's `api-key` header convention. All other values
(or a missing field) continue to use the default OpenAI-compatible branch
(`{endpoint}/openai/v1/chat/completions`).

## Scope & Objectives

### Scope

- Adding an `api_type` optional field to the `azureresources.json` entry schema.
- Validating the `api_type` value during JSON parsing ( `"openai"` | `"anthropic"` ).
- Routing `AzureResourceProvider::create_client` to either an OpenAI-compatible
  client (`AzureFoundryClient`) or an Anthropic-compatible client based on the
  entry's `api_type`.
- Ensuring the Anthropic branch uses Azure-style `api-key` authentication rather
  than the standard Anthropic `x-api-key`.
- Preserving backward compatibility for existing `azureresources.json` files
  that do not contain the new field.

### Out of Scope

- Modifying the `azure_foundry` provider itself (it remains independent).
- Adding new API types beyond `"openai"` and `"anthropic"`.
- Changing the authentication scheme for the OpenAI-compatible branch.

### Objectives

1. Allow teams that deploy Anthropic models on Azure to use them seamlessly via
   the Azure Resource Provider.
2. Keep the `azureresources.json` format self-describing: a single file can mix
   OpenAI and Anthropic endpoints.
3. Maintain zero-breaking-change guarantees for existing configurations.

---

## Requirements

### FR-001 — Schema Extension for `api_type` (Ubiquitous)

`The <Azure Resource provider> shall <accept an optional "api_type" field on every
entry in azureresources.json whose value is either "openai" or "anthropic">.`

**Acceptance criteria:**
- A top-level `"api_type"` string field may appear in each `resources[]` object.
- `"openai"` and `"anthropic"` are the only recognised values.
- The field is optional; when absent it defaults to `"openai"`.
- Any other value causes the entry to be skipped with a logged warning and does
  not abort parsing of subsequent entries.

### FR-002 — Provider-Level Branching (Event-Driven)

`When <AzureResourceProvider::create_client is invoked for a model whose
api_type is "anthropic">, the <provider> shall <instantiate an Anthropic-compatible
client whose base URL is the entry's endpoint with path "anthropic/v1/messages">.`

`When <AzureResourceProvider::create_client is invoked for a model whose
api_type is "openai" or missing>, the <provider> shall <instantiate the existing
AzureFoundryClient whose base URL is the entry's endpoint with path
"openai/v1/chat/completions">.`

**Acceptance criteria:**
- The branch selection is deterministic and based solely on the entry's
  `api_type`.
- The Anthropic branch constructs a client that serialises requests in Anthropic
  Messages API format.
- The OpenAI branch continues to behave exactly as before.
- Both branches receive the same `api_key` string resolved from the entry's
  `api_key` or `api_key_env` fields.

### FR-003 — Azure-Style Authentication on Anthropic Branch (Ubiquitous)

`The <Anthropic branch of the Azure Resource provider> shall <send the API key in
an "api-key" HTTP header rather than the standard Anthropic "x-api-key" or
"Authorization" header>.`

**Acceptance criteria:**
- Every outgoing HTTP request from the Anthropic branch carries the header
  `api-key: {resolved_key}`.
- Neither `x-api-key` nor `Authorization: Bearer` is present.
- If the API key is empty or missing, client creation fails with a clear error.

### FR-004 — Anthropic SSE Stream Compatibility (State-Driven)

`While <the Anthropic branch is active>, the <provider> shall <parse the SSE
response stream using the same event-type logic as the native AnthropicClient>.`

**Acceptance criteria:**
- The stream parser recognises Anthropic SSE events: `message_start`,
  `content_block_delta`, `message_delta`, etc.
- Tool-use blocks (`tool_use`) are forwarded as `ToolCall` stream events.
- Text deltas are forwarded as `Text` stream events.
- Stop reasons (`end_turn`, `max_tokens`, etc.) map to `FinishReason::Stop` or
  `FinishReason::Length` as appropriate.

### FR-005 — Metadata Preservation (Ubiquitous)

`The <Azure Resource provider> shall <carry the entry's api_type through to
create_client so that the correct client variant is selected for every model>.`

**Acceptance criteria:**
- `entry_to_model_info` preserves the `api_type` in a way that `create_client` can
  access it.
- The `ModelInfo` struct may be extended with an `api_type: Option<String>`
  field, or an equivalent side-channel mechanism is used.
- The provider advertises the correct `provider_id` (`"azure_resource"`) for
  both branches.

### FR-006 — Validation of Invalid `api_type` (Unwanted)

`If <an entry contains an unsupported api_type value>, the <Azure Resource provider>
shall <skip the entry and log a warning>.`

**Acceptance criteria:**
- Values such as `"gemini"`, `"ollama"`, or arbitrary strings are rejected.
- The warning includes the entry's `id` and the invalid `api_type`.
- Parsing continues for remaining entries.

### FR-007 — Backward Compatibility (Ubiquitous)

`The <Azure Resource provider> shall <continue to load and operate with existing
azureresources.json files that do not contain the api_type field>.`

**Acceptance criteria:**
- All existing tests for the Azure Resource Provider pass without modification.
- A file with zero `api_type` fields behaves identically to the pre-change
  behaviour.
- No fatal errors are raised for missing fields.

---

## Non-Functional Requirements

### NFR-001 — Performance
- JSON parsing overhead for the new field must not exceed 0.1 ms per entry.

### NFR-002 — Reliability
- An invalid `api_type` in one entry must not prevent other valid entries from
  loading.

### NFR-003 — Security
- The `api_key` value must not be logged in plain text during client creation.

### NFR-004 — Maintainability
- The Anthropic branch must reuse existing `AnthropicClient` internals (body
  builder, SSE parser) rather than duplicating protocol logic.

---

## Interfaces & Dependencies

| Interface | Crate | Purpose |
|---|---|---|
| `AzureResourceEntry` | `ragent-llm` | Extended with `api_type` field |
| `ModelInfo` | `ragent-llm` | Extended with `api_type` (optional) |
| `AzureFoundryClient` | `ragent-llm` | Reused for the OpenAI-compatible branch |
| `AnthropicClient` | `ragent-llm` | Delegated to for the Anthropic branch |
| `Provider::create_client` | `ragent-llm` | Branches on `api_type` |

---

## Glossary

| Term | Definition |
|---|---|
| **api_type** | A per-entry discriminator in `azureresources.json` that selects the wire protocol (OpenAI or Anthropic). |
| **AzureFoundryClient** | Existing OpenAI-compatible HTTP client that sends `api-key` headers. |
| **AnthropicClient** | Native Anthropic provider client that normally sends `x-api-key`. |
| **AzureAnthropicClient** | New thin wrapper around `AnthropicClient` that overrides the auth header to `api-key` and targets `/anthropic/v1/messages`. |

---

## Example Configuration

```json
{
  "version": "1",
  "resources": [
    {
      "id": "my-gpt-4o",
      "name": "My Azure GPT-4o",
      "endpoint": "https://my-resource.openai.azure.com",
      "api_key_env": "MY_AOAI_KEY",
      "api_type": "openai"
    },
    {
      "id": "my-claude-sonnet",
      "name": "My Azure Claude Sonnet",
      "endpoint": "https://my-anthropic-resource.eastus2.services.ai.azure.com",
      "api_key_env": "MY_ANTHROPIC_KEY",
      "api_type": "anthropic"
    }
  ]
}
```

In this example:
- `my-gpt-4o` is routed to `{endpoint}/openai/v1/chat/completions` via the
  `AzureFoundryClient`.
- `my-claude-sonnet` is routed to `{endpoint}/anthropic/v1/messages` via the
  new `AzureAnthropicClient` wrapper.
