---
status: draft
audit:
  - { time: 1787991901, from: "none", to: "draft", actor: "system" }
---
# Specification: OpenRouter Model Provider

**Spec ID:** `openrouterprov`
**Status:** draft
**Priority:** High
**Aligned version:** v1.0.64

## Overview

This specification defines a first-class **OpenRouter** LLM provider for ragent,
patterned after the existing `ollama_cloud` provider. OpenRouter
(<https://openrouter.ai>) is an OpenAI-compatible model aggregator that fronts
hundreds of models (Anthropic, OpenAI, Google, Meta, DeepSeek, xAI, Mistral,
Qwen and community/loft models) behind a single API key and a unified
`/api/v1/chat/completions` endpoint with transparent failover and per-model
usage billing.

The provider delivers three user-visible capabilities on par with
`ollama_cloud`:

1. **Model discovery** — enumerate every model OpenRouter can serve, enriched
   with context window, pricing, vision/reasoning capability flags, and
   thinking/reasoning levels.
2. **Secure token storage** — the OpenRouter API key is stored in ragent's
   encrypted credential store via `ragent auth openrouter <key>` (with an
   `OPENROUTER_API_KEY` environment-variable fallback), never written to
   plain configuration files or logs.
3. **Thinking / reasoning levels** — reasoning models accept the full
   `Auto/Off/Low/Medium/High` effort range, mapped to OpenRouter's
   `reasoning` payload object; non-reasoning models present no thinking UI.

## Background

### Why OpenRouter

OpenRouter collapses multi-provider access into one contract: a single Bearer
token, an OpenAI-style chat-completions schema, and a server-side router that
selects healthy upstreams. For ragent users this means one key unlocks models
that would otherwise require separate Anthropic, OpenAI, Gemini, and Bedrock
credentials — attractive for cost routing, model comparison, and
redundancy/failover workflows.

### Pattern source: the Ollama Cloud provider

OpenRouter is intentionally modeled on
`crates/ragent-llm/src/providers/ollama_cloud.rs`, which already demonstrates
the shapes this spec reuses:

| Concern | Ollama Cloud precedent | OpenRouter equivalent |
| --- | --- | --- |
| Provider struct | `OllamaCloudProvider { base_url }` with `new()` / `with_url()` / `Default` | `OpenRouterProvider { base_url }`, same trio |
| Provider id | `ollama_cloud` | `openrouter` |
| Client bail | `bail!("Ollama Cloud requires an API key.")` when key empty | `bail!("OpenRouter requires an API key.")` |
| Discovery | `GET /api/tags` + parallel `POST /api/show` per model | single `GET /api/v1/models`, no fan-out |
| Streaming | `LlmClient::chat` over `create_streaming_http_client()`, `take_sse_line` framing, `["DONE"]` handling, per-chunk idle timeout, first-byte timeout | identical framing (`data: {...}` SSE + `[DONE]`) |
| Thinking flag | `think_flag_from_request` → `{"think": bool}` | `reasoning_effort_from_request` → `{"reasoning": {"effort": ...}}` |
| Registration | `providers/mod.rs` module decl + registry register + `lib.rs` re-export + TUI id/name table + env alias tables | same five touch points with `openrouter` values |

### Related Research

- Vendor integration notes: <https://openrouter.ai/docs/api-reference/list-available-models>
- Prior art in this codebase: `ollama_cloud.rs` (discovery + streaming),
  `openai.rs` (`reasoning_effort` payload), `thinking.rs` (shared level
  mapping), `storage.rs` (`set_provider_auth` / `get_provider_auth`).

## Goals

1. Agents can select `openrouter/<model>` and chat, stream, and use tools
   through OpenRouter exactly as they do with `ollama_cloud`.
2. Every OpenRouter model is discoverable with accurate metadata (context
   window, prices, capabilities, thinking levels) without per-model fan-out.
3. The API key is protected at rest and redacted in every output path.
4. Users control reasoning effort per request and per model, consistent with
   the existing thinking-level UX (`/thinking` selector, per-model config).

## Non-Goals

- No prompt caching (`cache_control`) or provider-quirk normalization beyond
  payload shaping.
- No credit/balance-account queries or auto-topup features.
- No automatic model routing / cost-optimization heuristics (OpenRouter's
  server-side `models` auto-router may still be used by naming the model
  explicitly).
- No new HTTP security machinery — the existing 7-layer tool security, the
  shared `http_client` helpers (`create_http_client`, `create_streaming_http_client`,
  `execute_with_retry`, `take_sse_line`), and the storage-level encryption are
  reused unchanged.

## Users and Stakeholders

- **Interactive TUI users** — pick `openrouter` from the provider cycle,
  discover models with `/models`, chat with streaming and tool calls.
- **CLI / one-shot users** — `ragent run --model openrouter/anthropic/claude-4.5-sonnet "..."`
  and `ragent models openrouter`.
- **HTTP-server clients** — REST/SSE sessions driven through `ragent serve`.
- **Operators** — configure defaults, base URL, and per-model thinking
  overrides in `ragent.json`.

## Requirements

Requirements use EARS notation. All five templates are covered: ubiquitous
(FR-001..005, 012..016, 018, 020, 024), event-driven (FR-006, 007, 021,
022, 023), state-driven (FR-008, 009, 017, 019, 026), optional (FR-010,
011), and unwanted (FR-025).

### FR-001 — Provider identity and registration *[ubiquitous]*

The system shall expose an `openrouter` LLM provider with the identifier
`openrouter` and display name `OpenRouter` registered in the provider
registry, re-exported from `ragent-llm`, listed in the TUI provider cycle,
and present in the environment-variable alias tables so tool/Bang-command
error hints and router classification can resolve `OPENROUTER_API_KEY`.

**Acceptance:** `ragent models` and the TUI provider switch both list
`openrouter`; `provider_registry.get("openrouter")` succeeds.

### FR-002 — base URL default and override *[ubiquitous]*
The system shall default the OpenRouter API base to `https://openrouter.ai`
and shall honor `provider.openrouter.api.base_url` from `ragent.json`,
trailing-slash-trimmed, for both discovery and chat.

**Acceptance:** with `base_url = "http://127.0.0.1:8999/"` all requests hit
`http://127.0.0.1:8999/api/v1/...`.

### FR-003 — secure default base URL *[ubiquitous]*
The system shall use HTTPS for the default base URL and shall refuse to use
an `http:` base URL for chat or discovery unless an explicit override is
present in `ragent.json`.

**Acceptance:** `http://openrouter.ai` as a `base_url` value is honored in
config (operator override) but is never the default.

### FR-004 — API key sourcing precedence *[ubiquitous]*
The system shall resolve the OpenRouter API key in this order: (a) the
per-call key argument passed by the session layer, (b) the encrypted
credential stored under provider id `openrouter`, (c) the `OPENROUTER_API_KEY`
environment variable.

**Acceptance:** a stored key wins over environment; an empty argument yields
the stored value; both empty means no key.

### FR-005 — redaction of the API key *[ubiquitous]*
The system shall never include the OpenRouter API key in log records, error
messages, snapshots, session archives, or tool output; diagnostics refer to
the key only as a masked fingerprint (e.g. `sk-or-…abcd`).

**Acceptance:** `grep`-ing log output for the literal key finds nothing.

### FR-006 — token storage via `ragent auth` *[event-driven]*
When the user runs `ragent auth openrouter <key>`, the system shall store the
key in the encrypted credential store under provider id `openrouter` using
`Storage::set_provider_auth` and shall emit an info-level confirmation
without echoing the key.

**Acceptance:** `ragent models openrouter` subsequently connects without
`OPENROUTER_API_KEY` set.

### FR-007 — model discovery request *[event-driven]*
When the user triggers model discovery (TUI `/models`, CLI
`ragent models openrouter`, or startup-time enrichment), the system shall
issue a single `GET {base}/api/v1/models` request (Bearer header attached
only when a key is available) with a 10-second timeout, parse the
`{"data": [...]}` response, and map every entry to `ModelInfo`.

**Acceptance:** discovery of the live catalog succeeds with zero per-model
follow-up requests.

### FR-008 — discovery without a stored key *[state-driven]*
When `OPENROUTER_API_KEY` is unset/empty and no credential is stored, the
system shall still perform model discovery (the models endpoint is public)
and shall annotate the resulting actions with a "key required for chat"
note in CLI output and TUI diagnostics.

**Acceptance:** a fresh install with no key can list OpenRouter models.

### FR-009 — missing key at chat time *[state-driven]*
When a chat request reaches `OpenRouterProvider::create_client` with an
empty API key, the system shall reject the request with an explicit error
("OpenRouter requires an API key.") rather than attempt an unauthorized
call.

**Acceptance:** error message names the provider and remediation
(`ragent auth openrouter <key>` / `OPENROUTER_API_KEY`).

### FR-010 — model metadata enrichment *[optional]*
Where OpenRouter supplies model metadata, the system may propagate it into
`ModelInfo` fields as follows:
- `context_length` → `context_window` (usize; omit when absent)
- `pricing.prompt` / `pricing.completion` (US dollars per token, numeric or
  scientific-notation strings) → `cost.input` / `cost.output`
- `architecture.input_modalities` containing `"image"` → `vision: true`
- `supported_parameters` containing `"reasoning"` (or `allowed`/`required`
  reasoning variants) → `reasoning: true` and the thinking-level set
- `top_provider.context_length` as a fallback context source
- omission of any unknown field must not fail parsing

**Acceptance:** representative entries (e.g. `anthropic/claude-sonnet-4`,
`deepseek/deepseek-chat`) parse with plausible values; unknown schemas do not
break discovery.

### FR-011 — model id skipping *[optional]*
The system may skip malformed model entries (missing `id`, or a `data`
payload that does not deserialize) rather than aborting the whole discovery,
logging a `warn!` with the offending identifier.

**Acceptance:** one corrupt entry in a fixture still yields the remaining
models.

### FR-012 — chat endpoint and payload *[ubiquitous]*
The system shall send chat requests as `POST {base}/api/v1/chat/completions`
with JSON bodies matching the OpenAI-compatible schema: `model`, `messages`
(system first), `stream: true`, optional `tools` (cached OpenAI-format
serialization), optional `temperature`/`top_p`/`max_tokens` on `Some`,
`reasoning` object per FR-018, and `HTTP-Referer` / `X-Title` identifying
headers on the wire.

**Acceptance:** a captured request body is byte-comparable to an
OpenAI-style request for the same conversation, plus the `reasoning` object
when enabled.

### FR-013 — streaming response parsing *[ubiquitous]*
The system shall stream chat responses by consuming `text/event-stream`
frames (`data: {json}`, terminated by `data: [DONE]`), reusing
`take_sse_line` framing and `create_streaming_http_client()`; it shall map
`choices[0].delta.{reasoning, reasoning_content, content}`, inline
`tool_calls` deltas (incremental `arguments` string fragments), final-chunk
`usage` when present, and `choices[0].finish_reason` to the shared
`StreamEvent` vocabulary.

**Acceptance:** visual parity with `ollama` streaming in the TUI (text
deltas, tool call steps, usage line, finish reason).

### FR-014 — timeouts and stream-stall detection *[ubiquitous]*
The system shall apply a first-byte timeout of
`request.stream_timeout_secs.unwrap_or(600)` seconds and a per-chunk idle
timeout of `STREAM_CHUNK_IDLE_TIMEOUT_SECS` (120 s), emitting
`StreamEvent::Error` ("OpenRouter: stream stalled —") on idle expiry and
terminating the stream without panicking.

**Acceptance:** a stalled TCP connection ends in an `Error` event within
the idle window; the agent loop recovers.

### FR-015 — non-2xx error handling *[ubiquitous]*
When the OpenRouter API responds with a non-2xx status, the system shall
raise `anyhow::bail!("OpenRouter API error ({status}): {error_body}")`
truncated to a bounded length (matching `ollama_cloud`'s 300-char cap) and
shall not surface the API key from the response or request context.

**Acceptance:** an invalid-key (401) chat session displays a readable
message and returns to the prompt without crashing.

### FR-016 — per-line parse resilience *[ubiquitous]*
The system shall treat individual malformed SSE lines as recoverable
(`warn!` + skip), never aborting the stream for a single bad frame.

**Acceptance:** a fixture stream containing one invalid JSON line still
yields the surrounding content and correct finish reason.

### FR-017 — model id partitioning *[state-driven]*
When a `provider/model` name is resolved, the system shall strip only the
first path segment for provider detection, so OpenRouter vendor-slug ids
such as `openrouter/anthropic/claude-sonnet-4` (provider `openrouter`, model
`anthropic/claude-sonnet-4`) survive; a bare chat call that somehow receives
a vendor-prefixed id only (e.g. `anthropic/claude-sonnet-4` with no
`openrouter/` prefix) is out of scope and re-uses whatever generic fallback
exists for ambiguous ids.

**Acceptance:** pick-model round-trips (`/model openrouter/<vendor-slug>`,
CLI `--model openrouter/<vendor-slug>`) reach OpenRouter with the full
vendor-slug id intact.

### FR-018 — thinking / reasoning level mapping *[ubiquitous]*
The system shall map `ThinkingConfig` levels to OpenRouter's
`reasoning` payload as follows and shall omit the `reasoning` object when
no thinking configuration is present in the request:

| ThinkingLevel | `reasoning` payload |
| --- | --- |
| `Auto` | `{"effort": "medium", "enabled": true}` — server decides depth |
| `Off` (enabled=false or `Off`) | `{"effort": "none", "enabled": false}` |
| `Low` | `{"effort": "low", "enabled": true}` |
| `Medium` | `{"effort": "medium", "enabled": true}` |
| `High` | `{"effort": "high", "enabled": true}` |

`budget_tokens`, when present, maps to `reasoning.max_tokens`.

**Acceptance:** unit-table comparison against `reasoning_effort_from_request`
for every level; no `reasoning` object on plain-chat requests.

### FR-019 — per-model thinking levels *[state-driven]*
When a discovered model reports reasoning support, the system shall offer
the full `Auto/Off/Low/Medium/High` level set in `ModelInfo.thinking_levels`
and set `thinking_config` to `ThinkingConfig::new(ThinkingLevel::Auto)`
by default; models without reasoning support shall present an empty level
set and no default thinking config.

**Acceptance:** `deepseek/deepseek-r1`-style entries expose levels;
GPT-3.5-style entries do not.

### FR-020 — reasoning token accounting *[ubiquitous]*
When a stream chunk carries `reasoning` or `reasoning_content` deltas, the
system shall route them to the reasoning UI (`ReasoningStart` /
`ReasoningDelta` / `ReasoningEnd`) with no mixing into `TextDelta`; when a
final usage object reports nonzero `usage.prompt_tokens` /
`usage.completion_tokens`, the system shall emit
`StreamEvent::Usage` with those counts and count **all** completion tokens
(reasoning + visible) toward the completion figure.

**Acceptance:** TUI shows thinking bubbles and a usage line matching
OpenRouter-reported token counts.

### FR-021 — config-driven model overrides *[event-driven]*
When `ragent.json` declares `provider.openrouter.models.<id>`, the system
shall merge those user-declared `ModelDefinition` entries into the
discovered catalog (and honor `thinking` overrides at the model level
before provider-level `thinking` defaults), letting operators pin context
sizes, prices, or reasoning defaults for models where discovery data is
inaccurate.

**Acceptance:** a hand-declared model appears in `/models` with the
configured values; a model-level `thinking.level = "high"` wins over a
provider default of `low`.

### FR-022 — discovery failure surfacing *[event-driven]*
When `GET /api/v1/models` fails (connect error, timeout, non-2xx), the
system shall log a `warn!` with context and surface a human-readable message
("Could not connect to OpenRouter: {e}") rather than panic, leaving the
previous model cache (if any) untouched.

**Acceptance:** with the network down, `ragent models openrouter` prints the
connect failure and exits 0 (CLI listing path) without corrupting state.

### FR-023 — TUI provider in the cycle *[event-driven]*
When the TUI cycles providers (repeated keypress on the provider widget),
the system shall include `openrouter` in the cycle, preselect it via
`--provider openrouter` (where that flag exists), and honor
`/model openrouter/<id>` autocomplete against discovered models.

**Acceptance:** cycling reaches `OpenRouter`; autocomplete offers discovered
vendor-slug ids.

### FR-024 — documentation and slash-command support *[ubiquitous]*
The system shall document the OpenRouter provider in `SPEC.md` (provider
table row, feature notes), `README.md` (feature bullet + `OPENROUTER_API_KEY`
example), and `QUICKSTART.md` (auth and model-selection snippets), and
audit trail entries shall be recorded at spec milestones.

**Acceptance:** docs mention `openrouter` next to `ollama_cloud` in every
provider enumeration.

### FR-025 — key handling and billing safety *[unwanted]*
The system shall NOT:
- persist the API key outside the encrypted credential store;
- echo the key in `tracing` records, panic messages, or `Debug` impls;
- send the key to any host other than the configured base URL host;
- automatically retry a failed chat POST (billing safety; only the
  discovery GET is ever retry-eligible);
- expose OpenRouter models in tool lists, bash security allowlists, or the
  finance/codeindex tool families (`openrouter` is never a tool provider).

**Acceptance:** log/panic/`Debug` sites show no key interpolation; the chat
POST path has no retry wrapper.

### FR-026 — models listing output *[state-driven]*
When `ragent models openrouter` runs, the system shall print one line per
model in the shared `provider/model` display format (`  openrouter/<id padded>
<display name>` sorted by id), the total count, and — when no key is
configured — the remediation hint
`Run 'ragent auth openrouter <key>' or set OPENROUTER_API_KEY.`

**Acceptance:** output format matches the `ollama_cloud` listing block.

## Constraints

1. **Crate layout** — implementation lives in
   `crates/ragent-llm/src/providers/openrouter.rs` (provider + client +
   discovery), with integration tests in
   `crates/ragent-llm/tests/test_openrouter_*.rs`.
2. **No unsafe** — pure safe Rust; HTTP via the existing `reqwest` helper
   factories.
3. **Workspace style** — edition 2024, `cargo fmt`, clippy `-D warnings`,
   zero-warning builds, `tracing` logging only (no `println!` in library
   code), doc comments on every public item.
4. **Sequential coupling** — OpenRouter support must not change behavior of
   existing providers; all changes are additive.
5. **Reused infrastructure** — `Provider` trait, `LlmClient::chat`,
   `StreamEvent`, `ModelInfo`, `ThinkingConfig`, `thinking.rs` helpers,
   `http_client.rs` factories, `tool_cache.rs` serialization,
   `Storage::set/get_provider_auth`.

## Dependencies

- Upstream schema stability of `openrouter.ai/api/v1` (discovery + chat).
- Workspace crates: `ragent-llm`, `ragent-config`, `ragent-storage`,
  `ragent-agent` (session wiring / env alias table), `ragent-tui`
  (provider widget + `/model` autocomplete).

## Risks

| Risk | Mitigation |
| --- | --- |
| Model-catalog schema drift breaks discovery | tolerant parsing (FR-010, FR-011), fixture-based tests, local echo server tests |
| Vendor-slug model ids confuse provider routing | first-segment-only partitioning (FR-017) |
| Reasoning payload incompatibility with some upstream models | `reasoning` object only sent when configured (FR-018 v default-omission rule) |
| Key leakage in logs | single redaction helper used by all diagnostics (FR-005) |
| Billing surprise from retried POSTs | no chat retry (FR-025) |

## Success Criteria

1. All 26 requirements are implemented and verified by the plan tasks below.
2. Existing test suite remains green (no regressions in `ragent-llm`,
   `ragent-tui`, `ragent-config`, or `ragent-agent`).
3. A user with only an OpenRouter key can install, run
   `ragent auth openrouter <key>`, select `openrouter/<model>` in the TUI,
   and complete a streaming chat with tool use end to end.