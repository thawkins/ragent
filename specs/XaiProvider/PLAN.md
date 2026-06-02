# XaiProvider — Implementation Plan

## Overview

This plan implements the xAI Grok model provider as a new module in
`crates/ragent-llm/src/providers/xai.rs`. Because the xAI API is
OpenAI-compatible, the implementation reuses the existing `OpenAiClient` for
request building and SSE stream parsing — the same strategy used by
`AzureFoundryProvider` and `GenericOpenAiProvider`.

---

## Tasks

| ID | Title | Requirement | Effort | Priority | Status | Dependencies |
|----|-------|-------------|--------|----------|--------|--------------|
| T-001 | Create `xai.rs` provider module | FR-001, FR-002 | S | Critical | completed | — |
| T-002 | Implement `default_models()` catalog | FR-003, FR-009 | M | Critical | pending | T-001 |
| T-003 | Implement `create_client()` with base URL resolution | FR-004, FR-005, FR-006 | S | Critical | pending | T-001 |
| T-004 | Register `XaiProvider` in `create_default_registry()` | FR-001 | S | Critical | pending | T-001, T-003 |
| T-005 | Add `pub mod xai` to `providers/mod.rs` | FR-001 | S | Critical | pending | T-001 |
| T-006 | Add xAI thinking-level helper to `thinking.rs` | FR-003 | S | High | pending | T-002 |
| T-007 | Implement model alias resolution | FR-011 | S | Medium | pending | T-002 |
| T-008 | Implement vendor suffix stripping for `@xai` | FR-010 | S | Medium | pending | T-004 |
| T-009 | Add connection logging in `create_client()` | FR-015 | S | High | pending | T-003 |
| T-010 | Verify rate-limit header parsing inheritance | FR-014 | S | Low | pending | T-003 |
| T-011 | Write unit tests for `default_models()` | NFR-002, FR-003, FR-009 | M | High | pending | T-002 |
| T-012 | Write unit tests for base URL resolution | NFR-002, FR-005 | M | High | pending | T-003 |
| T-013 | Write unit tests for vendor suffix stripping | NFR-002, FR-010 | S | Medium | pending | T-008 |
| T-014 | Write integration test for `create_client()` | NFR-002 | M | High | pending | T-004 |
| T-015 | Verify zero new dependencies | NFR-001, FR-013 | S | Critical | pending | T-001–T-010 |
| T-016 | Run `cargo clippy` and `cargo fmt` | NFR-003 | S | Medium | pending | T-001–T-015 |
| T-017 | Update `ragent-llm/Cargo.toml` description | FR-001 | S | Low | pending | T-004 |
## Task Details

### T-001: Create `xai.rs` provider module

Create `crates/ragent-llm/src/providers/xai.rs` with:

- Module-level `//!` doc comment describing the xAI Grok provider
- `XaiProvider` struct (unit struct, same as `OpenAiProvider`)
- `Provider` trait implementation with `id()` → `"xai"` and `name()` → `"xAI"`
- Placeholder `default_models()` and `create_client()` stubs

**Files:** `crates/ragent-llm/src/providers/xai.rs` (new)

### T-002: Implement `default_models()` catalog

Populate `default_models()` with 6 Grok models:

| Model ID              | Cost (in/out per M tokens) | Vision | Reasoning | Context  |
|-----------------------|---------------------------|--------|-----------|----------|
| `grok-3`             | $3.00 / $15.00            | No     | No        | 131,072  |
| `grok-3-mini`        | $0.15 / $0.60             | No     | No        | 131,072  |
| `grok-3-mini-fast`   | $0.15 / $0.60             | No     | No        | 131,072  |
| `grok-2`             | $2.00 / $10.00            | No     | No        | 131,072  |
| `grok-2-mini`        | $0.30 / $0.80             | No     | No        | 131,072  |
| `grok-2-vision-1212` | $2.00 / $10.00            | Yes    | No        | 131,072  |

**Files:** `crates/ragent-llm/src/providers/xai.rs`

### T-003: Implement `create_client()` with base URL resolution

Implement `Provider::create_client()`:

1. Resolve base URL: `base_url` parameter → `XAI_API_BASE` env var → `https://api.x.ai`
2. Construct `OpenAiClient::new(api_key, &resolved_base)`
3. Wrap in `XaiClient` struct (or directly return `Box<OpenAiClient>` if no
   custom headers needed — xAI uses standard Bearer auth)
4. Return `Ok(Box::new(client))`

Since xAI uses the standard `Authorization: Bearer` header (same as OpenAI),
no custom client wrapper is needed — `OpenAiClient` works directly.

**Files:** `crates/ragent-llm/src/providers/xai.rs`

### T-004: Register `XaiProvider` in `create_default_registry()`

Add `registry.register(Box::new(xai::XaiProvider));` to the
`create_default_registry()` function in `mod.rs`.

**Files:** `crates/ragent-llm/src/providers/mod.rs`

### T-005: Add `pub mod xai` to `providers/mod.rs`

Add `pub mod xai;` to the module declarations at the top of
`crates/ragent-llm/src/providers/mod.rs`.

**Files:** `crates/ragent-llm/src/providers/mod.rs`

### T-006: Add xAI thinking-level helper to `thinking.rs`

Add `xai_thinking_levels_for_model()` to `crates/ragent-llm/src/providers/thinking.rs`.
Initially returns an empty `Vec` since no current Grok models support configurable
thinking. This placeholder allows future Grok reasoning models to be added
without touching the provider module.

**Files:** `crates/ragent-llm/src/providers/thinking.rs`

### T-007: Implement model alias resolution

Add a `resolve_alias()` function that maps short names to full model IDs:

| Alias          | Resolves To           |
|----------------|-----------------------|
| `grok2`        | `grok-2`              |
| `grok2mini`    | `grok-2-mini`         |
| `grok2vision`  | `grok-2-vision-1212` |
| `grok3`        | `grok-3`              |
| `grok3mini`    | `grok-3-mini`         |

This is used within `default_models()` lookups and the provider's model
resolution path.

**Files:** `crates/ragent-llm/src/providers/xai.rs`

### T-008: Implement vendor suffix stripping for `@xai`

The existing `resolve_model()` in `ProviderRegistry` already handles `@`
vendor suffixes generically. Verify this works for `grok-3@xai` and add
a unit test confirming the behaviour. No code changes expected — just test
coverage.

**Files:** `crates/ragent-llm/tests/test_xai_provider.rs` (new)

### T-009: Add connection logging in `create_client()`

Add `tracing::info!` call in `create_client()` after successful client
construction, matching the pattern used by other providers:

```rust
tracing::info!(
    chat_endpoint = %format!("{}/v1/chat/completions", resolved_base),
    models_endpoint = %format!("{}/v1/models", resolved_base),
    "xAI provider connected"
);
```

**Files:** `crates/ragent-llm/src/providers/xai.rs`

### T-010: Verify rate-limit header parsing inheritance

Since `OpenAiClient::parse_sse_stream()` already calls
`parse_openai_rate_limit_headers()`, the xAI provider inherits rate-limit
parsing for free. Verify by reading the code path and add a comment in the
module doc noting this inherited capability.

**Files:** `crates/ragent-llm/src/providers/xai.rs` (doc only)

### T-011: Write unit tests for `default_models()`

Test file: `crates/ragent-llm/tests/test_xai_provider.rs`

Tests:
- `test_xai_default_models_count` — exactly 6 models
- `test_xai_model_ids` — all expected model IDs present
- `test_xai_vision_capability` — only `grok-2-vision-1212` has vision
- `test_xai_tool_use_capability` — all models have tool_use
- `test_xai_streaming_capability` — all models have streaming
- `test_xai_costs_positive` — all costs are ≥ 0
- `test_xai_provider_id` — all models have `provider_id == "xai"`

### T-012: Write unit tests for base URL resolution

Test file: `crates/ragent-llm/tests/test_xai_provider.rs`

Tests:
- `test_xai_base_url_default` — resolves to `https://api.x.ai` when no override
- `test_xai_base_url_from_env` — `XAI_API_BASE` env var takes priority over default
- `test_xai_base_url_from_parameter` — `base_url` parameter takes highest priority

### T-013: Write unit tests for vendor suffix stripping

Test file: `crates/ragent-llm/tests/test_xai_provider.rs`

Tests:
- `test_xai_vendor_suffix_stripped` — `grok-3@xai` resolves to `grok-3`

### T-014: Write integration test for `create_client()`

Test file: `crates/ragent-llm/tests/test_xai_provider.rs`

Tests:
- `test_xai_provider_registered` — `create_default_registry().get("xai")` returns a provider
- `test_xai_provider_identity` — registered provider returns correct `id()` and `name()`
- `test_xai_create_client_returns_client` — `create_client()` with a dummy key returns
  a valid `LlmClient` (no actual API call)

### T-015: Verify zero new dependencies

After implementation, diff `crates/ragent-llm/Cargo.toml` to confirm no new
entries in `[dependencies]`. Run `cargo build -p ragent-llm` to verify
compilation with existing dependencies only.

### T-016: Run `cargo clippy` and `cargo fmt`

Run `cargo clippy -p ragent-llm` and `cargo fmt -p ragent-llm --check` to
ensure the new code adheres to project style guidelines.

### T-017: Update `ragent-llm/Cargo.toml` description

Update the `description` field to include "xAI" in the provider list:
`"LLM provider implementations for ragent (Anthropic, OpenAI, Gemini, Ollama,
HuggingFace, Copilot, Generic OpenAI, Azure AI Foundry, Azure Resource, Amazon
Bedrock, xAI)"`

---

## Execution Order

```
T-001 ──► T-002 ──► T-011
  │         │
  │         └──► T-006
  │
  ├──► T-003 ──► T-009 ──► T-012
  │                │
  │                └──► T-010
  │
  ├──► T-005
  │
  └──► T-004 ──► T-008 ──► T-013
                  │
                  └──► T-007
                      
T-001..T-010 ──► T-014 (integration tests)
T-001..T-014 ──► T-015 (dependency check)
T-001..T-015 ──► T-016 (lint/format)
T-015 ──► T-017 (description update)
```

## Estimated Total Effort

| Effort | Count |
|--------|-------|
| S      | 11    |
| M      | 6     |
| **Total** | **17 task-units** |

Estimated implementation time: **1–2 hours** for an experienced Rust developer
familiar with the ragent provider architecture.