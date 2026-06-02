---
status: draft
audit:
  - { time: 1780421668, from: "none", to: "draft", actor: "system" }
---
# XaiProvider — xAI Grok Model Provider

## Overview

This specification defines the requirements for integrating the xAI Grok model
family into ragent as a first-class LLM provider. The xAI API exposes an
OpenAI-compatible Chat Completions endpoint, so the implementation can reuse the
existing `OpenAiClient` for request building and SSE stream parsing, following
the same pattern as `AzureFoundryProvider` and `GenericOpenAiProvider`.

**Provider ID:** `xai`
**Default base URL:** `https://api.x.ai`
**Authentication:** `Authorization: Bearer <XAI_API_KEY>`

---

## Requirements

### FR-001: Provider Registration

The system **shall** register an `xai` provider in the
`create_default_registry()` function so that it appears alongside all other
built-in providers.

*EARS: Ubiquitous — "The system shall…"*

### FR-002: Provider Identity

The `xai` provider **shall** return `"xai"` from `Provider::id()` and
`"xAI"` from `Provider::name()`.

*EARS: Ubiquitous — "The system shall…"*

### FR-003: Default Model Catalog

The `xai` provider **shall** expose the following default models when
`default_models()` is called:

| Model ID                | Name              | Vision | Tool Use | Context  | Max Output |
|------------------------|--------------------|--------|----------|----------|-----------|
| `grok-3`               | Grok 3            | No     | Yes      | 131,072  | 16,384    |
| `grok-3-mini`          | Grok 3 Mini       | No     | Yes      | 131,072  | 16,384    |
| `grok-3-mini-fast`     | Grok 3 Mini Fast  | No     | Yes      | 131,072  | 16,384    |
| `grok-2`               | Grok 2            | No     | Yes      | 131,072  | 16,384    |
| `grok-2-mini`          | Grok 2 Mini       | No     | Yes      | 131,072  | 16,384    |
| `grok-2-vision-1212`   | Grok 2 Vision     | Yes    | Yes      | 131,072  | 16,384    |

*EARS: Ubiquitous — "The system shall…"*

### FR-004: API Key Authentication

When `create_client()` is invoked, the system **shall** use the provided API
key as a Bearer token in the `Authorization` header, consistent with the xAI
API convention.

*EARS: Ubiquitous — "The system shall…"*

### FR-005: Base URL Resolution

When `create_client()` is invoked, the system **shall** resolve the base URL
in the following priority order:

1. `base_url` parameter (passed from configuration)
2. `XAI_API_BASE` environment variable
3. `https://api.x.ai` (default)

*EARS: State-driven — "When create_client() is invoked, the system shall
resolve the base URL based on the following priority order…"*

### FR-006: OpenAI-Compatible Client Reuse

When `create_client()` constructs the client, the system **shall** reuse the
existing `OpenAiClient` by delegating request building and SSE stream parsing
to it, using the chat endpoint path `/v1/chat/completions`.

*EARS: Event-driven — "When create_client() constructs the client, the system
shall…"*

### FR-007: Streaming Support

The `xai` provider **shall** support streaming responses via SSE, using the
same `parse_sse_stream()` logic inherited from `OpenAiClient`.

*EARS: Ubiquitous — "The system shall…"*

### FR-008: Tool Use Support

The `xai` provider **shall** support OpenAI-compatible tool/function calling
by delegating tool-call request building and response parsing to the
`OpenAiClient`.

*EARS: Ubiquitous — "The system shall…"*

### FR-009: Vision Support

The `xai` provider **shall** set `vision: true` only for models whose IDs
contain `vision`, and `vision: false` for all other models in the default
catalog.

*EARS: State-driven — "The system shall set vision: true only for models
whose IDs contain 'vision', and vision: false for all other models…"*

### FR-010: Vendor Suffix Stripping

When a model ID with the `@xai` vendor suffix is provided (e.g.
`grok-3@xai`), the system **shall** strip the suffix and resolve the model
against the `xai` provider's catalog.

*EARS: Optional — "When a model ID with the @xai vendor suffix is provided,
the system shall…"*

### FR-011: Model Alias Support

The `xai` provider **shall** support short aliases for model names so that
users can reference models by simplified names (e.g., `grok2` → `grok-2`,
`grok3` → `grok-3`, `grok2vision` → `grok-2-vision-1212`).

*EARS: Optional — "The xai provider shall support short aliases…"*

### FR-012: Error Handling

When the xAI API returns a non-2xx status code, the system **shall** return an
error message that includes the HTTP status code and the response body, using
the same error format as the `OpenAiClient` chat method.

*EARS: Unwanted — "When the xAI API returns a non-2xx status code, the system
shall not silently ignore the error; it shall return an error message…"*

### FR-013: No New Crate Dependencies

The implementation **shall not** introduce any new crate dependencies to
`ragent-llm/Cargo.toml`. All functionality must be achieved using existing
dependencies (`reqwest`, `serde_json`, `anyhow`, etc.).

*EARS: Unwanted — "The implementation shall not introduce any new crate
dependencies…"*

### FR-014: Rate Limit Header Parsing

When the xAI API returns rate-limit headers
(`x-ratelimit-limit-requests`, `x-ratelimit-remaining-requests`, etc.), the
system **shall** parse them using `parse_openai_rate_limit_headers()` and
emit a `StreamEvent::RateLimit` event.

*EARS: Optional — "When the xAI API returns rate-limit headers, the system
shall…"*

### FR-015: Connection Logging

When `create_client()` successfully constructs a client, the system **shall**
log an informational message including the resolved chat endpoint URL and
models endpoint URL, matching the logging pattern of other providers.

*EARS: Event-driven — "When create_client() successfully constructs a client,
the system shall…"*

---

## Non-Functional Requirements

### NFR-001: Zero New Dependencies

The implementation must not add any new entries to `ragent-llm/Cargo.toml`
`[dependencies]`. The xAI API is OpenAI-compatible, so the existing `reqwest`,
`serde_json`, and `OpenAiClient` infrastructure is sufficient.

### NFR-002: Test Coverage

The implementation must include:

- Unit tests for `default_models()` verifying model count, IDs, capabilities,
  and costs.
- Unit tests for base URL resolution priority order.
- Unit tests for vendor suffix stripping (`@xai`).
- Integration tests for `Provider::create_client()` verifying the returned
  client connects to the correct endpoint.

### NFR-003: Code Style

The implementation must follow project conventions:

- 4-space indentation, 100-character line width.
- `//!` module-level documentation.
- `///` doc comments on all public items.
- `snake_case` functions, `PascalCase` types.
- `anyhow::Result` for error handling.
- `tracing` crate for logging (no `println!`).