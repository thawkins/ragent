# HuggingFace Provider Implementation Plan

## Overview

Add a HuggingFace Inference API provider to ragent, enabling access to
HuggingFace-hosted models (both the free Inference API and paid Inference
Endpoints) through the standard `Provider` / `LlmClient` trait system.

**Provider ID:** `huggingface`
**Display Name:** `Hugging Face`
**API Base:** `https://api-inference.huggingface.co`
**Auth:** `Authorization: Bearer <HF_TOKEN>`
**Env Variable:** `HF_TOKEN` (standard HuggingFace token)

---

## 1. Files to Create / Modify

### New Files

| File | Purpose |
|------|---------|
| `crates/ragent-core/src/provider/huggingface.rs` | Provider + LlmClient implementation |

### Modified Files

| File | Change |
|------|--------|
| `crates/ragent-core/src/provider/mod.rs` | Add `pub mod huggingface;` and register in `create_default_registry()` |
| `SPEC.md` | Add HuggingFace to §3.1 LLM Providers table |

---

## 2. Provider Implementation (`huggingface.rs`)

### 2.1 Structs

```rust
pub struct HuggingFaceProvider;

pub(crate) struct HuggingFaceClient {
    api_key: String,
    base_url: String,
    http: reqwest::Client,
}
```

### 2.2 Provider Trait Implementation

```rust
#[async_trait::async_trait]
impl Provider for HuggingFaceProvider {
    fn id(&self) -> &str { "huggingface" }
    fn name(&self) -> &str { "Hugging Face" }

    fn default_models(&self) -> Vec<ModelInfo> {
        // Return popular HF Inference API models
        vec![
            // See §3 for model list
        ]
    }

    async fn create_client(
        &self,
        api_key: &str,
        base_url: Option<&str>,
        options: &HashMap<String, Value>,
    ) -> Result<Box<dyn LlmClient>> {
        let client = HuggingFaceClient::new(
            api_key,
            base_url.unwrap_or("https://api-inference.huggingface.co"),
        );
        Ok(Box::new(client))
    }
}
```

### 2.3 LlmClient Trait Implementation

The HuggingFace Inference API supports an OpenAI-compatible chat
completions endpoint at `/v1/chat/completions`. This simplifies
implementation as we can follow the OpenAI streaming SSE pattern.

```rust
#[async_trait::async_trait]
impl LlmClient for HuggingFaceClient {
    async fn chat(
        &self,
        request: ChatRequest,
    ) -> Result<Pin<Box<dyn Stream<Item = StreamEvent> + Send>>> {
        // 1. Build request body (OpenAI-compatible format)
        // 2. POST to {base_url}/v1/chat/completions
        // 3. Parse SSE stream (same format as OpenAI)
        // 4. Yield StreamEvent variants
    }
}
```

### 2.4 API Endpoint Selection

HuggingFace has two endpoint styles:

| Style | URL Pattern | Use Case |
|-------|------------|----------|
| **Inference API** | `https://api-inference.huggingface.co/v1/chat/completions` | Free/Pro tier, shared infrastructure |
| **Inference Endpoints** | `https://<endpoint-id>.endpoints.huggingface.cloud/v1/chat/completions` | Dedicated deployments |

Both support the OpenAI-compatible `/v1/chat/completions` endpoint with
streaming. The `base_url` configuration parameter selects between them.

### 2.5 Request Format

```json
{
  "model": "meta-llama/Llama-3.1-70B-Instruct",
  "messages": [
    {"role": "system", "content": "..."},
    {"role": "user", "content": "..."}
  ],
  "stream": true,
  "max_tokens": 4096,
  "temperature": 0.7,
  "tools": [...]
}
```

### 2.6 Authentication

```
Authorization: Bearer hf_xxxxxxxxxxxxx
```

**Environment variable resolution order:**
1. `HF_TOKEN` (standard HuggingFace token)
2. `HUGGING_FACE_HUB_TOKEN` (legacy name)
3. `RAGENT_API_KEY_HUGGINGFACE` (ragent convention)

The Token should be aquired from the provider setup dialog, and stored in the secure storage. 

### 2.7 Streaming Response Parsing

The HF Inference API returns OpenAI-compatible SSE events:

```
data: {"choices":[{"delta":{"content":"Hello"},"index":0}]}
data: {"choices":[{"delta":{"tool_calls":[...]},"index":0}]}
data: {"choices":[{"finish_reason":"stop","index":0}]}
data: [DONE]
```

Map to `StreamEvent`:

| SSE Field | StreamEvent |
|-----------|-------------|
| `delta.content` | `TextDelta { text }` |
| `delta.tool_calls[].function.name` | `ToolCallStart { id, name }` |
| `delta.tool_calls[].function.arguments` | `ToolCallDelta { id, args_json }` |
| `finish_reason: "stop"` | `Finish { reason: Stop }` |
| `finish_reason: "tool_calls"` | `Finish { reason: ToolUse }` |
| `finish_reason: "length"` | `Finish { reason: Length }` |
| `usage` object | `Usage { input_tokens, output_tokens }` |

### 2.8 Error Handling

| HTTP Status | Meaning | Action |
|-------------|---------|--------|
| 401 | Invalid token | Return auth error |
| 403 | Model gated / access denied | Return descriptive error |
| 429 | Rate limited | Log warning, return error (retry handled by session) |
| 503 | Model loading | Return "model loading" error with estimated wait |

HuggingFace returns a specific error when a model is loading:

```json
{"error": "Model meta-llama/... is currently loading", "estimated_time": 60.0}
```

This should be surfaced to the user with the estimated wait time.

---

## 3. Default Models

Models available on the free Inference API tier:

| Model ID | Display Name | Context | Max Output | Cost (per M tokens) | Capabilities |
|----------|-------------|---------|------------|---------------------|--------------|
| `meta-llama/Llama-3.1-8B-Instruct` | Llama 3.1 8B | 128,000 | 4,096 | Free | streaming, tool_use |
| `meta-llama/Llama-3.1-70B-Instruct` | Llama 3.1 70B | 128,000 | 4,096 | Free | streaming, tool_use |
| `mistralai/Mistral-7B-Instruct-v0.3` | Mistral 7B | 32,000 | 4,096 | Free | streaming |
| `mistralai/Mixtral-8x7B-Instruct-v0.1` | Mixtral 8x7B | 32,000 | 4,096 | Free | streaming, tool_use |
| `Qwen/Qwen2.5-72B-Instruct` | Qwen 2.5 72B | 128,000 | 4,096 | Free | streaming, tool_use |
| `microsoft/Phi-3-mini-4k-instruct` | Phi-3 Mini | 4,096 | 2,048 | Free | streaming |

ALL Models should be aquired dynamicaly, the maximum number of models displayed should be 50. 

**Note:** HuggingFace model availability changes frequently. The provider
should support any model ID — these are just the defaults shown in
`ragent models`.

---

## 4. Configuration

### 4.1 ragent.json

```jsonc
{
  "provider": {
    "huggingface": {
      "env": ["HF_TOKEN"],
      "api": {
        "base_url": "https://api-inference.huggingface.co"
      },
      "models": {
        "meta-llama/Llama-3.1-70B-Instruct": {
          "name": "Llama 3.1 70B",
          "capabilities": {
            "reasoning": false,
            "streaming": true,
            "vision": false,
            "tool_use": true
          }
        }
      },
      "options": {
        "wait_for_model": true
      }
    }
  }
}
```

### 4.2 Provider-Specific Options

| Option | Type | Default | Description |
|--------|------|---------|-------------|
| `wait_for_model` | bool | `true` | Wait for model to load instead of erroring on 503 |
| `use_cache` | bool | `true` | Enable HF server-side response caching |

### 4.3 Using an Inference Endpoint

```jsonc
{
  "provider": {
    "huggingface": {
      "env": ["HF_TOKEN"],
      "api": {
        "base_url": "https://my-endpoint.endpoints.huggingface.cloud"
      }
    }
  }
}
```

---

## 5. Registration

In `crates/ragent-core/src/provider/mod.rs`:

```rust
// Add module declaration
pub mod huggingface;

// In create_default_registry():
pub fn create_default_registry() -> ProviderRegistry {
    let mut registry = ProviderRegistry::new();
    // ... existing providers ...
    registry.register(Box::new(huggingface::HuggingFaceProvider));
    registry
}
```

---

## 6. API Key Resolution

In `crates/ragent-core/src/session/processor.rs`, the `resolve_api_key`
method resolves API keys by checking:

1. Provider config `env` list (checks each env var name)
2. Standard `RAGENT_API_KEY_<PROVIDER>` pattern

For HuggingFace, the env list should be:
```rust
env: vec!["HF_TOKEN".to_string(), "HUGGING_FACE_HUB_TOKEN".to_string()]
```

This means the key is found from:
- `HF_TOKEN` environment variable (checked first)
- `HUGGING_FACE_HUB_TOKEN` environment variable (legacy)
- `RAGENT_API_KEY_HUGGINGFACE` (ragent convention, auto-checked)

---

## 7. Implementation Steps

### Phase 1: Core Provider

1. Create `crates/ragent-core/src/provider/huggingface.rs`
2. Implement `HuggingFaceProvider` struct with `Provider` trait
3. Implement `HuggingFaceClient` struct with `LlmClient` trait
4. Add OpenAI-compatible SSE stream parsing
5. Handle HF-specific errors (model loading, gated models)
6. Register in `create_default_registry()`

### Phase 2: Model Discovery (Optional)

7. Add `fetch_models()` method to query HF API for available models
8. Endpoint: `GET https://huggingface.co/api/models?pipeline_tag=text-generation&inference=warm`
9. Filter for models with active inference endpoints

### Phase 3: Testing

10. Add unit tests for request body building
11. Add unit tests for SSE response parsing
12. Add integration test with mock server
13. Test with real HF token (manual)

### Phase 4: Documentation

14. Update SPEC.md §3.1 with HuggingFace provider entry
15. Update QUICKSTART.md with HuggingFace setup instructions
16. Add example configuration to README.md

---

## 8. Implementation Notes

### Reuse Opportunity

Since HuggingFace uses an OpenAI-compatible API, significant code can be
shared with or modelled after `openai.rs`:

- Request body building (same JSON structure)
- SSE stream parsing (same `data: {...}` format)
- Tool call handling (same delta format)

Consider extracting a shared `openai_compat` module if not already present,
or directly reference the OpenAI implementation patterns.

### Tool Use Support

HuggingFace's OpenAI-compatible endpoint supports tool use for models that
have been trained for it (Llama 3.1+, Mixtral, Qwen 2.5). The tool
definition format is identical to OpenAI's JSON schema format, so no
conversion is needed.

### Rate Limits

The free Inference API has rate limits that vary by model popularity and
user tier (free vs Pro). Rate limit headers are returned:

```
X-RateLimit-Limit: 100
X-RateLimit-Remaining: 95
X-RateLimit-Reset: 1234567890
```

These should be parsed and emitted as `StreamEvent::RateLimit` events.

### Vision Support

Some HuggingFace models support vision (e.g., Llama 3.2 Vision). The
`ImageUrl` content part should be passed through for models with
`vision: true` in their capabilities.

---

## 9. Testing Checklist

- [ ] Provider `id()` returns `"huggingface"`
- [ ] Provider `name()` returns `"Hugging Face"`
- [ ] `default_models()` returns at least 4 models
- [ ] `create_client()` creates client with correct base URL
- [ ] Client builds correct request body for chat completions
- [ ] Client handles streaming SSE responses correctly
- [ ] Tool call start/delta/end events are parsed correctly
- [ ] Usage tokens are extracted from response
- [ ] 401 errors produce clear authentication error message
- [ ] 503 "model loading" errors include estimated wait time
- [ ] Rate limit headers are parsed into RateLimit events
- [ ] Custom base_url works for Inference Endpoints
- [ ] HF_TOKEN environment variable is resolved correctly
- [ ] Model with tool_use capability correctly sends tools in request
- [ ] Model without tool_use capability omits tools from request
