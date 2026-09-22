# Implementation Plan: OpenRouter Model Provider

**Spec ID:** `openrouterprov`
**Spec status:** draft

## Overview

This plan implements the `openrouterprov` specification: a first-class
OpenRouter LLM provider patterned after the existing `ollama_cloud` provider,
covering model discovery (`GET /api/v1/models`), secure token storage (the
existing encrypted credential store plus `OPENROUTER_API_KEY` fallback), and
thinking/reasoning levels (the shared `reasoning_effort` machinery mapped to
OpenRouter's `reasoning` payload object).

All work is additive. No existing provider behavior changes. The
implementation lives in a single new module,
`crates/ragent-llm/src/providers/openrouter.rs`, plus the five registration
touch points already used for `ollama_cloud`.

## Architecture

```
crates/ragent-llm/src/providers/
├── mod.rs                      # +pub mod openrouter; +registry.register(openrouter)
├── openrouter.rs               # NEW: provider, client, discovery (~700 lines)
└── mod.rs (re-exports)         # lib.rs: +OpenRouterProvider

crates/ragent-llm/tests/
└── test_openrouter_provider.rs # integration tests (fixtures + live optional)

Touch points outside ragent-llm:
├── crates/ragent-tui/src/app/state.rs        # ("openrouter", "OpenRouter") row
├── crates/ragent-llm/src/providers/router_client.rs  # env alias entry
├── crates/ragent-agent/src/session/processor.rs      # env alias entry
└── src/main.rs                                # `ragent models openrouter` branch + auth note
```

### Module layout inside `openrouter.rs` (mirrors `ollama_cloud.rs`)

| Section | Contents |
| --- | --- |
| Consts | `DEFAULT_OPENROUTER_BASE_URL = "https://openrouter.ai"`, timeout consts |
| `OpenRouterProvider` | `pub struct { base_url: String }` + `new()` / `with_url()` / `Default` |
| `Provider` impl | `id() = "openrouter"`, `name() = "OpenRouter"`, empty `default_models()`, `discover_models()`, `create_client()` (bails on empty key) |
| Discovery wire types | `ModelsResponse { data: Vec<ModelEntry> }`, `ModelEntry { id, name, context_length, pricing, architecture, supported_parameters, top_provider, ... }` — all tolerant (`#[serde(default)]`) |
| Metadata mapping | `parse_price` (USD-per-token, strings allowed) → `Cost`; `input_modalities` → vision; `supported_parameters` ⊇ `reasoning` → thinking levels per FR-019 |
| `OpenRouterClient` | implements `LlmClient::chat` → POST `/api/v1/chat/completions`, SSE via `take_sse_line`, `reasoning` object from FR-018, per-chunk idle timeout, first-byte timeout |
| Redaction helper | `masked_key(&str) -> String` used by every diagnostic |

## Requirements Traceability

Every FR is covered: discovery (FR-007..011, 022) by T-003/T-004; secure
tokens (FR-004..006, 009) by T-002; thinking levels (FR-018..020) by T-006;
payload/streaming (FR-012..016) by T-005; model-id partitioning (FR-017)
by T-007; registration and UX surfacing (FR-001, 023, 026) by T-008/T-009;
config overrides (FR-002, 003, 021) by T-002/T-010; docs (FR-024) by T-011;
and unwanted behaviors (FR-025) verified in T-012.

## Tasks

| ID | Title | Requirement | Effort | Priority | Status | Dependencies |
|----|-------|-------------|--------|----------|--------|--------------|
| T-001 | Skeleton module: `openrouter.rs` with `OpenRouterProvider { base_url }`, `new()`/`with_url()`/`Default`, `Provider` impl (id `openrouter`, name `OpenRouter`, empty `default_models()`), plus `pub mod` decl in `providers/mod.rs`, registry `register()` call, and `lib.rs` re-export | FR-001, FR-002 | S | Critical | completed | — |
| T-002 | Key sourcing precedence and `create_client` gate: per-call arg → stored credential id `openrouter` → `OPENROUTER_API_KEY`; `bail!("OpenRouter requires an API key.")` on empty; base URL override handling incl. trailing-slash trim (FR-002/003); masked-key redaction helper; no chat retry | FR-004, FR-005, FR-009, FR-002, FR-003, FR-025 | S | Critical | completed | T-001 |
| T-003 | Discovery client: `GET {base}/api/v1/models` (10 s timeout, Bearer only when key present), tolerant serde wire types with `#[serde(default)]`, single-call (no fan-out), `warn!` + human-readable error surfacing on failure, previous cache preserved | FR-007, FR-008, FR-022 | M | Critical | completed | T-001 |
| T-004 | Metadata mapping: `context_length`→`context_window` (with `top_provider` fallback), `pricing.prompt/completion`→`Cost` (numeric + scientific-notation strings), `architecture.input_modalities`→`vision`, `supported_parameters` reasoning detection; malformed-entry skipping with `warn!` and fixture unit tests | FR-010, FR-011 | M | High | completed | T-003 |
| T-005 | Chat client: `OpenRouterClient` implementing `LlmClient::chat` — `POST /api/v1/chat/completions`, system-first messages, `stream: true`, cached OpenAI-format `tools`, optional `temperature`/`top_p`/`max_tokens`, SSE parsing via `take_sse_line` with `[DONE]` handling, `choices[0].delta` mapping (incl. `reasoning`/`reasoning_content`), incremental `tool_calls` argument fragments, final-chunk `usage`, `finish_reason` mapping, first-byte (600 s default) + 120 s per-chunk idle timeouts, non-2xx bail with bounded error body, per-line parse resilience | FR-012, FR-013, FR-014, FR-015, FR-016, FR-020 | L | Critical | completed | T-002 |
| T-006 | Thinking/reasoning levels: `reasoning_effort_from_request` integration producing the `reasoning` object per the FR-018 table (Auto/Off/Low/Medium/High, `budget_tokens`→`max_tokens`), `reasoning_levels`/`thinking_config` population from `supported_parameters` (FR-019), reasoning-delta→UI event routing and token accounting (FR-020); unit-table tests across all levels | FR-018, FR-019, FR-020 | M | High | completed | T-005 |
| T-007 | Vendor-slug model id partitioning: first-segment-only provider detection so `openrouter/anthropic/claude-sonnet-4` resolves provider `openrouter` + model `anthropic/claude-sonnet-4`; verify pick-model round-trips in TUI and CLI | FR-017 | S | High | completed | T-001 |
| T-008 | Integration tests (offline): fixture-driven tests in `crates/ragent-llm/tests/test_openrouter_provider.rs` for wire-type parsing, metadata mapping, reasoning-payload table, SSE-event mapping incl. tool-call fragments and usage, timeout/stall behaviour via a local TCP echo server; optional `#[ignore]` live test behind `OPENROUTER_API_KEY` | FR-007, FR-010, FR-013, FR-016, FR-018, FR-020 | M | High | completed | T-004, T-005, T-006 |
| T-009 | UX surfacing: TUI provider-cycle row ("openrouter", "OpenRouter"), env-alias entries in `router_client.rs` and `session/processor.rs`, `ragent models openrouter` CLI branch (listing format per FR-026 incl. remediation hint), `/model openrouter/...` autocomplete verification | FR-001, FR-023, FR-026 | S | High | completed | T-001 |
| T-010 | Config override merging in `ragent-config`: deep-merge docs/tests for `provider.openrouter.{api.base_url, models.<id>, thinking}` overlay precedence (model over provider) wired into `thinking_config_for` | FR-002, FR-003, FR-021 | S | Medium | completed | T-001 |
| T-011 | Documentation: `SPEC.md` provider-table row + feature notes, `README.md` feature bullet + `OPENROUTER_API_KEY` quick-start snippet, `QUICKSTART.md` auth/model-selection section; CHANGELOG entry per Keep a Changelog | FR-024 | S | Medium | completed | T-009 |
| T-012 | Manual QA pass vs `TESTPLAN.md` (TC-001..TC-012) plus unwanted-behaviour review (key redaction audit, no chat POST retry, no tool-family exposure); full workspace test run and format/clippy gate before closure | All | M | High | completed | T-008, T-009, T-010, T-011 |
## Verification & Gates

For every task touching `.rs` files:

1. `cargo fmt` immediately after each edit (mandatory, before anything else).
2. `cargo check --workspace` clean, zero warnings.
3. `cargo clippy --workspace -- -D warnings` clean.
4. `cargo test --workspace` green before marking the task complete.
5. Dead-code lint pass (`-D unreachable_pub -D dead_code -D unused_imports`)
   before final closure.

## Risks & Mitigations (execution view)

- **Wire schema drift** — keep all OpenRouter structs `#[serde(default)]`,
  and add one fixture file per discovered field so drift shows up as a
  failing fixture test, not a runtime panic.
- **Streaming leaks** — every loop exit path must drain open tool calls and
  emit `ToolCallEnd` (mirror `ollama_cloud`'s done-frame draining).
- **Key leaks** — the masked-key helper is the only permitted formatting of
  the key; code review greps `tracing` macros for key interpolation.
- **Billing safety** — no `execute_with_retry` around the chat POST; the
  discovery GET may use it.

## Out of Scope (unchanged from spec)

- Credit/balance queries, auto-topup, prompt caching, server-side
  cost-routing heuristics, and any changes to the 7-layer bash security or
  permission system.