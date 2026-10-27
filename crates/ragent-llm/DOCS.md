# ragent-llm

Provider-agnostic LLM client abstraction, provider registry, and all concrete
LLM provider implementations (Anthropic, OpenAI, Gemini, Bedrock, Copilot,
Ollama, HuggingFace, Azure, xAI, and a model router).

## Workspace Dependencies

- ragent-types
- ragent-config
- ragent-storage

## External Dependencies

- async-trait, tokio, serde, serde_json, anyhow, thiserror, tracing
- reqwest, futures, async-stream, tokio-util
- chrono, base64, regex, dirs, sha2, hex

Dev-dependencies: tempfile.

## Public API (crate root)

### Modules

- **llm** — Provider-agnostic LLM client abstraction; re-exports `ChatRequest`, `ChatMessage`, `StreamEvent`, `ChatContent`, `ContentPart`, `ToolDefinition`, `LlmFinishReason` from `ragent_types::llm` and defines the `LlmClient` streaming trait.
- **providers** — Provider trait, provider registry, and all concrete LLM provider implementations.
- **provider** (module alias) — Compatibility re-export of `providers`.
- **shared_request** — `SharedChatRequest` type for cheaply-cloneable shared chat request bodies.

### Crate-root items

- **ModelInfo** (struct) — Metadata describing an LLM model (id, provider, name, cost, capabilities, context window, max output, request multiplier, thinking config).
- **ProviderInfo** (struct) — Summary of a provider and its models.
- **UsageInfo** (struct) — Plan-based usage info (plan label, quota percent).
- **Provider** (trait) — Trait for LLM provider backends; methods: `id`, `name`, `default_models`, `set_event_bus`, `discover_models`, `create_client`, `as_any_static`, `fetch_usage`.
- **ProviderRegistry** (struct) — Registry of providers; methods: `new`, `register`, `set_event_bus`, `set_event_bus_all`, `get`, `get_as_any`, `list`, `resolve_model`, `resolve_model_async`.
- **create_default_registry** (fn) — Creates a `ProviderRegistry` pre-populated with all built-in providers.
- **SharedChatRequest** (struct) — Arc-backed shared handle to chat messages and tool definitions.
- Provider structs: **AnthropicProvider**, **OpenAiProvider**, **ResponsesApiProvider**, **GeminiProvider**, **OllamaProvider**, **OllamaCloudProvider**, **HuggingFaceProvider**, **CopilotProvider**, **BedrockProvider**, **AzureFoundryProvider**, **AzureResourceProvider**, **AzureResourceEntry**, **GenericOpenAiProvider**, **XaiProvider**, **RouterProvider**.

### Module: llm

- **LlmClient** (trait) — Streaming LLM client trait; `chat()` converts a `ChatRequest` into a `StreamEvent` stream.
- Re-exported types: `ChatRequest`, `ChatMessage`, `ChatContent`, `ContentPart`, `StreamEvent`, `ToolDefinition`, `LlmFinishReason`.

### Module: shared_request

- **SharedChatRequest** (struct) — Arc-backed shared handle; methods: `new`, `from_arc`, `len`, `is_empty`.

### Module: providers

- **ModelInfo** / **ProviderInfo** / **UsageInfo** (structs) — Metadata types.
- **Provider** (trait) — Provider trait.
- **ProviderRegistry** (struct) — Registry.
- **create_default_registry** (fn) — Factory.

### Provider submodules

- **anthropic** — `AnthropicProvider`, `AnthropicClient`.
- **openai** — `OpenAiProvider`, `OpenAiClient`, `openai_default_models`.
- **openai_responses** — `ResponsesApiProvider`, `ResponsesApiClient`, `ResponsesApiUsage`, `responses_api_default_models`.
- **gemini** — `GeminiProvider`, `gemini_default_models`.
- **ollama** — `OllamaProvider`.
- **ollama_cloud** — `OllamaCloudProvider`.
- **huggingface** — `HuggingFaceProvider`, `huggingface_default_models`.
- **copilot** — `CopilotProvider`, `CopilotAuth`, `find_copilot_token`, `is_pat_token`, `resolve_copilot_github_token`, `DeviceFlowStart`, `start_copilot_device_flow`, `cached_copilot_plan`.
- **bedrock** — `BedrockProvider`, `bedrock_default_models`, `BedrockAnthropicClient`, `BedrockConverseClient`.
- **bedrock_credentials** — `AwsCredentials`, `resolve_aws_credentials`, `resolve_region`.
- **bedrock_sigv4** — `BEDROCK_SERVICE`, `sign_request`.
- **azure_foundry** — `AzureFoundryProvider`, `AzureFoundryClient`.
- **azure_resource** — `AzureResourceEntry`, `AzureResourceProvider`, `parse_azure_resources`; its `AzureAnthropicClient` drives the Anthropic Messages shape through the shared `wire::decode::ToolCallAccumulator` (`content_block_start` -> `open`, `input_json_delta` -> `delta`, `content_block_stop` -> `end`), so FR-004/FR-005/FR-019/FR-020 apply to it exactly as they do to `anthropic.rs` (T-011).
- **generic_openai** — `GenericOpenAiProvider`.
- **xai** — `XaiProvider`, `xai_default_models`, `resolve_xai_model_id`.
- **http_client** — `create_http_client`, `create_streaming_http_client`.
- **mock_llm_client** — `MockScenario` (enum), `MockLlmClient`.
- **tool_cache** — `ToolFormat` (enum: the cache key, and the only thing that maps a format to a profile, a name prefix and a body key - `format_profile`, `name_prefix`, `tool_envelope`, `body_key`, `ALL`), `cached_tools`, `invalidate_tool_cache`, and `tool_arguments_json`; re-exports `CachedTools` from `wire::encode` so the historic path still names it. The cache key is `(ToolFormat, fingerprint)` over name/description/parameters, and the bytes always come from `encode_tools`, so a hit is byte-identical to a miss (FR-003, T-004). `body_key` derives the body key a provider assigns the encoded list to from the declared envelope - `toolConfig` for `BedrockToolSpec` (the whole `{"tools":[...]}` object), `tools` for the other five (the inner array) - behind the `TOOLS_KEY` and `BEDROCK_TOOLS_KEY` constants, so no provider module opens the key as a literal and the key cannot drift from the payload (FR-003, T-027). The same type states the FR-014 marking rule: `message_profile` (the profile this format encodes *messages* with), `transmits_is_error_natively` (that profile's `supports_is_error`, never a second opinion), `result_content` (`ERROR: `-prefixed exactly where the flag has no native carrier) and `message_name_prefix` (always `None` - a definition-time rewrite such as HuggingFace's `t_` does not reach a message shape whose results are keyed by call id). A dialect that assembles its own `messages` array names its format on `MessageContext::error_marking` and the encoder marks its results by the same rule (FR-014, T-017).
- **wire** — the declared tool-calling contract (FR-002): `ToolWireProfile` (nine required fields, no `Default`, no optional field), `ToolEnvelope` (5), `ResultCarrier` (4), `IdField` (`Key` / `Synthesised`), `ArgsEncoding` (2), `StreamKeying` (2). Submodules: `profile_table` (one `const` profile per family, the `FAMILIES` list naming each of the six exactly once, the `PROVIDER_WIRE` id-to-family table and `profile_for`), `encode` (`encode_tools` - the one tool-definition encoder, folding the five former `build_*` builders into one `match` on `tool_envelope`, with HuggingFace's `t_` prefix a `name_prefix` argument rather than a sixth envelope - plus `CachedTools`, `message` encoder), `decode` (`ToolCallAccumulator` - `StreamKey::{Index,CallId}` plus `from_frame`, and `new`/`keying`/`open_count`/`deferred_count`/`set_id`/`open`/`delta`/`end`/`end_all`/`defer`/`flush_deferred`; the one keyed tracker every provider stream delegates its open tool calls to, FR-004/T-008; it also owns every synthetic call id, minting `<provider>_<counter>_<name>` from the provider id it was constructed with and logging one debug line - FR-019/T-010 - and it records the id a frame supplied apart from the key a call is addressed by, so a key is never mistaken for an id), `matrix` (the `--wire-matrix` renderer) and `trace` (the opt-in payload trace, FR-017/T-023: `set_enabled`/`is_enabled` process-wide switch defaulting off, `next_path` returning `None` while off so no directory is ever created, `record_body` writing `<session>-<step:04>.json` atomically, `set_root`/`root` for the test hook, and two redaction passes over the shared sanitizer - credential-named keys wholesale, then `redact_secrets` matching inside every string - plus non-ASCII escaped to `\uXXXX`).
- **router** — `RouterProvider`; methods: `new`, `with_defaults`, `config`, `reload_config`, `set_enabled`, `set_registry`, `set_storage`, `set_event_bus` (its `provider::wire` row is nominal: `RouterClient` picks a downstream provider per request and forwards the canonical `ChatRequest` to that provider's client, which encodes with *its* profile, so a consumer needing the effective contract for a router session resolves the downstream provider rather than reading the row).
- **Unified tool path (spec `uniformtools`)** — end to end the encoder/decoder chain is one declared contract: a client holds a `ToolWireProfile` and dispatches its outbound definitions on `tool_envelope` through `encode_tools` (bytes cached per `(ToolFormat, fingerprint)`, so a hit equals a miss), its persisted tool results on `result_carrier` / `id_field` through `encode_message` (`ERROR: `-prefixed via `error_marked` when `supports_is_error` is `false`), and its streamed deltas on `stream_keying` through `ToolCallAccumulator`. No provider module builds tool JSON of its own (FR-003), and that is a scan over every file under `providers/` rather than a claim: see `tests/test_wire_encode.rs` (T-027). A construct a family cannot carry is refused rather than dropped: `encode_message_fallible` / `encode_messages_fallible` return `WireLoss { construct, provider }` and the caller issues no request - the Responses input list refuses an image part and a message role it has no item for (`role:<role>`), which is what stops a history from being silently shortened (FR-018, T-007). A signed reasoning block is replayed by the one family whose profile declares `echo_reasoning`: `encode_message`'s Anthropic arm writes `{"type":"thinking","thinking":<text>}` ahead of the turn's `tool_use` blocks, adding `signature` only when the record carries one (an unsigned block gains no key and no `null`), and every other family omits the part - the omission is the profile's declared term, which is why no converter and no provider module consults `echo_reasoning` (FR-011, T-018). Arguments are never decoded here: `ragent_types::tool_args::decode_tool_arguments` is the one helper and it is the harness's call, not the provider's (FR-007). `ragent --wire-matrix` prints the whole contract per registered id as one aligned ASCII table (ten columns, sorted rows, no blank or `-` cell, NFR-005) without contacting a provider, and reports a registered id with no row by name with a non-zero exit rather than printing a hole in the table. Every family is replayed end to end without a provider: `tests/test_wire_replay.rs` serves a recorded response body per family from a loopback listener, drives the family's own client, then feeds the decoded call back through that family's encoder (NFR-004).
- **router_client** — `RouterClient`, `extract_attachments`.
- **router_classifier** — `AttachmentInfo`, `ClassificationResult`, `PromptClassifier`, `dimension_name`.
- **router_config** — `TierEntry`, `TierConfig`, `Tier` (enum), `WeightConfig`, `BoundaryConfig`, `RouterConfig`.
- **router_modifiers** — `ModifierResult`, `detect_modifier`.