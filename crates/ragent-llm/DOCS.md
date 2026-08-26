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
- **azure_resource** — `AzureResourceEntry`, `AzureResourceProvider`, `parse_azure_resources`.
- **generic_openai** — `GenericOpenAiProvider`.
- **xai** — `XaiProvider`, `xai_default_models`, `resolve_xai_model_id`.
- **http_client** — `create_http_client`, `create_streaming_http_client`.
- **mock_llm_client** — `MockScenario` (enum), `MockLlmClient`.
- **tool_cache** — `ToolFormat` (enum), `CachedTools`, `cached_tools`, `invalidate_tool_cache`.
- **router** — `RouterProvider`; methods: `new`, `with_defaults`, `config`, `reload_config`, `set_enabled`, `set_registry`, `set_storage`, `set_event_bus`.
- **router_client** — `RouterClient`, `extract_attachments`.
- **router_classifier** — `AttachmentInfo`, `ClassificationResult`, `PromptClassifier`, `dimension_name`.
- **router_config** — `TierEntry`, `TierConfig`, `Tier` (enum), `WeightConfig`, `BoundaryConfig`, `RouterConfig`.
- **router_modifiers** — `ModifierResult`, `detect_modifier`.