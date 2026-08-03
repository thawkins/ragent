# ALPLAN H2 Completion Report

Task: Reduce `loop.llm.create_stream` local CPU overhead by caching serialised
tool definitions, serialising request bodies directly to bytes, and reusing the
HTTP client across turns.

## Changes Made

### 1. Serialised tool-definition byte cache

`crates/ragent-llm/src/providers/tool_cache.rs` provides a process-global cache
keyed by `(ToolFormat, tool_fingerprint)`. It pre-serialises the tool list for:

- `ToolFormat::OpenAi` — used by OpenAI, Generic OpenAI, Azure Foundry, Azure
  Resource (OpenAI path), Ollama, Ollama Cloud, Copilot.
- `ToolFormat::Anthropic` — used by Anthropic and Bedrock Anthropic.
- `ToolFormat::Gemini` — used by Gemini.
- `ToolFormat::Bedrock` — used by Bedrock Converse `toolConfig`.
- `ToolFormat::HuggingFace` — HuggingFace with `t_` prefix.

`SessionProcessor::invalidate_tool_cache` also calls
`ragent_llm::provider::tool_cache::invalidate_tool_cache()` so the cache is
cleared whenever the tool registry changes.

### 2. Direct byte serialisation of request bodies

All affected providers now build the request body with `serde_json::to_vec`
and send it via `RequestBuilder::body(body_bytes)` instead of `.json(&body)`:

- `openai.rs`
- `anthropic.rs`
- `ollama.rs`
- `ollama_cloud.rs`
- `gemini.rs`
- `huggingface.rs`
- `copilot.rs`
- `bedrock.rs` (both Anthropic and Converse paths)
- `azure_resource.rs` (Anthropic wrapper)
- `azure_foundry.rs`

### 3. HTTP client reuse

- `http_client.rs` already caches the streaming and non-streaming `reqwest::Client`
  in process-global `OnceLock`s.
- `SessionProcessor` caches the warm `Arc<dyn LlmClient>` per `provider/model`
  in `llm_client_cache` across turns.
- `RouterClient` caches downstream clients per `(provider, model)`.
- Fixed `AzureFoundryClient::chat`, which was creating a fresh `reqwest::Client`
  inside the hot path. It now reuses the inner `OpenAiClient` HTTP client via a
  new `pub(crate) fn http_client()` accessor, and sends cached `body_bytes`
  through the retry wrapper.

## Verification

- `cargo check --workspace` — passes.
- `cargo fmt --check` — passes.
- `cargo test -p ragent-llm --lib` — 247 passed.
- `cargo test -p ragent-agent --lib` — 218 passed.

## Expected Effect

`loop.llm.create_stream` no longer re-serialises ~111 tool schemas on every loop
step, and connection-pool warm-up is amortised across turns. The remaining
`create_stream` time should now be dominated by provider round-trip / model
inference latency, matching the raw floor described in ALPLAN H2.
