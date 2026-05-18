# Implementation Plan: Microsoft AI Foundry Provider

## SPEC ID: AIFoundaryProv

## Overview

This plan implements the Microsoft AI Foundry provider for ragent, enabling connection to models hosted on Azure AI Foundry through an OpenAI-compatible API endpoint.

## Tasks

| ID | Title | Requirement | Effort | Priority | Dependencies |
|----|-------|-------------|--------|----------|--------------|
| T-001 | Scaffold `AzureFoundryProvider` struct and module | FR-001, FR-002 | M | Critical | — |
| T-002 | Implement provider configuration parsing | FR-011 | S | High | T-001 |
| T-003 | Implement API key authentication (api-key header + Bearer) | FR-003 | S | High | T-001 |
| T-004 | Implement model discovery via `/models` endpoint | FR-004, FR-005 | M | Critical | T-001, T-003 |
| T-005 | Implement chat completions with SSE streaming | FR-006 | M | Critical | T-001, T-003 |
| T-006 | Implement tool calling support | FR-007 | M | High | T-005 |
| T-007 | Implement vision input support | FR-008 | S | Medium | T-005 |
| T-008 | Implement reasoning/thinking levels for o-series | FR-009 | S | Medium | T-005 |
| T-009 | Implement capability detection with safe defaults | FR-010 | S | High | T-004 |
| T-010 | Implement health probe with timeout | FR-013 | S | High | T-003 |
| T-011 | Implement error handling (401, 429, 5xx) | FR-014, FR-015 | S | High | T-005 |
| T-012 | Register provider in `ProviderRegistry` | FR-001, FR-016 | S | Critical | T-001, T-004, T-005 |
| T-013 | Update provider setup dialog for Azure AI Foundry | FR-002, FR-016 | M | High | T-012 |
| T-014 | Update TUI slash commands and model picker | FR-016, FR-017 | S | Medium | T-012, T-013 |
| T-015 | Update `ragent.json` schema and config validation | FR-011 | S | High | T-002 |
| T-016 | Auto-detect `AZURE_AI_FOUNDRY_API_KEY` env var | FR-012 | S | Medium | T-013 |
| T-017 | Write unit tests for request/response parsing | NFR-003 | M | High | T-001–T-011 |
| T-018 | Write integration test for provider setup flow | NFR-003 | L | Medium | T-012–T-016 |
| T-019 | Update CHANGELOG.md and RELEASE.md | — | S | Low | T-012–T-016 |
| T-020 | Update README.md with Azure AI Foundry provider docs | — | S | Low | T-012–T-016 |

## Task Details

### T-001: Scaffold `AzureFoundryProvider` struct and module

Create `crates/ragent-llm/src/providers/azure_foundry.rs` with:
- `AzureFoundryProvider` struct holding `base_url`, `api_key`, `client`
- Implement `Provider` trait (or equivalent ragent-llm provider interface)
- Add module declaration in `crates/ragent-llm/src/providers/mod.rs`

### T-002: Implement provider configuration parsing

Support `ragent.json` configuration:
```json
{
  "provider": {
    "azure_foundry": {
      "api_key_env": "AZURE_AI_FOUNDRY_API_KEY",
      "base_url": "https://my-project.eastus2.services.ai.azure.com",
      "thinking": { "enabled": true, "level": "low" },
      "models": {
        "gpt-4o": { "thinking": { "enabled": true, "level": "high" } }
      }
    }
  }
}
```

### T-003: Implement API key authentication

Azure AI Foundry accepts authentication in two ways:
- `api-key: <key>` header (Azure OpenAI Service style)
- `Authorization: Bearer <key>` header (OpenAI-compatible)

Implement both and prefer `api-key` for Azure endpoints.

### T-004: Implement model discovery

Query the Azure AI Foundry model listing endpoint:
```
GET {base_url}/models
```
or
```
GET {base_url}/openai/models
```

Parse the response into ragent's internal `ModelInfo` structures with capabilities.

### T-005: Implement chat completions with SSE streaming

Use the OpenAI-compatible chat completions endpoint:
```
POST {base_url}/v1/chat/completions
```

Support streaming via SSE (`stream: true`).

### T-006: Implement tool calling support

Map ragent tool definitions to OpenAI function calling format. Parse tool call deltas from streaming responses.

### T-007: Implement vision input support

Support base64-encoded image attachments in the `content` array (OpenAI vision format).

### T-008: Implement reasoning/thinking levels

For o-series models (o1, o3-mini), support `reasoning_effort` parameter mapping to `low`/`medium`/`high`.

### T-009: Implement capability detection with safe defaults

When model discovery does not return capability flags, default to:
- `streaming: true`
- `tool_use: false`
- `vision: false`

### T-010: Implement health probe

Send lightweight `GET /models` or `GET /health` with 5-second timeout. Return connectivity status.

### T-011: Implement error handling

Map Azure HTTP errors to ragent error types:
- `401 Unauthorized` → Invalid credentials
- `429 Too Many Requests` → Rate limited
- `5xx` → Service unavailable

### T-012: Register provider in `ProviderRegistry`

Add `azure_foundry` to the provider registry in `crates/ragent-llm/src/llm.rs` or equivalent registry module.

### T-013: Update provider setup dialog

Add "Azure AI Foundry" as an option in the TUI provider setup wizard with fields for:
- Endpoint URL
- API Key
- (Optional) Default model

### T-014: Update TUI slash commands and model picker

Ensure `/model`, `/models`, and provider switching commands include `azure_foundry`.

### T-015: Update `ragent.json` schema

Add `azure_foundry` to the provider enum in `ragent-config` and validate configuration fields.

### T-016: Auto-detect environment variable

Check for `AZURE_AI_FOUNDRY_API_KEY` at startup and pre-fill in the provider setup dialog if present.

### T-017: Write unit tests

Tests for:
- Request serialization (chat completions, tool calls, vision)
- Response parsing (streaming deltas, model listing)
- Error handling (401, 429)
- Configuration parsing

### T-018: Write integration test

End-to-end test for the provider setup flow using a mock Azure AI Foundry server.

### T-019: Update CHANGELOG.md and RELEASE.md

Document the new provider in release notes.

### T-020: Update README.md

Add Azure AI Foundry to the provider list and configuration examples.

## Risk Assessment

| Risk | Likelihood | Impact | Mitigation |
|------|-----------|--------|------------|
| Azure API rate limits during testing | Medium | Low | Use cached responses; mock server for tests |
| Model ID naming inconsistency | High | Low | Normalize model IDs; use discovery data |
| Streaming format differences from OpenAI | Low | High | Extensive testing with live endpoint |
| o-series model reasoning parameter differences | Medium | Medium | Test with each model family |

## Success Criteria

1. User can successfully configure and chat with Azure AI Foundry models.
2. Model discovery, streaming, tool use, and vision work correctly.
3. All tests pass (unit + integration).
4. Documentation is complete and accurate.

## Timeline Estimate

- **Total Effort:** 15 story points
- **Estimated Duration:** 2–3 days
- **Parallelizable Tasks:** T-013–T-016 (UI work) can proceed in parallel with T-005–T-011 (provider logic).
