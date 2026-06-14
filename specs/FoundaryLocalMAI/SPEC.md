---
status: draft
audit:
  - { time: 1780997561, from: "none", to: "draft", actor: "system" }
---
# FoundaryLocalMAI — Microsoft Foundry Local MAI Provider

## Overview

This specification defines a new LLM provider for ragent that connects to **Microsoft Foundry Local** (formerly Windows AI Studio), enabling locally-hosted **MAI (Microsoft AI)** models to run on the same device as ragent. The provider uses the official `foundry-local` Rust SDK to manage the local service lifecycle, model catalog, and inference.

Microsoft Foundry Local is an end-to-end local AI solution that hosts models (including MAI models such as Phi-4, Phi-3.5, and other Microsoft-optimized models) via a local OpenAI-compatible HTTP endpoint. The `foundry-local` Rust SDK provides programmatic control over the service, model management, and chat completion APIs.

The provider implements the existing `Provider` trait and `LlmClient` interface, making Foundry Local models selectable alongside cloud providers. The provider is gated behind a Cargo feature (`foundry-local`) to avoid forcing the SDK dependency on all builds.

## Requirements

### SDK Dependency & Feature Gating

**FR-001** (Ubiquitous) The system shall compile the Foundry Local provider code only when the `foundry-local` Cargo feature is enabled on the `ragent-llm` crate.

**FR-002** (Ubiquitous) The system shall declare an optional dependency on the `foundry-local` crate (version `^0.2`) in `ragent-llm/Cargo.toml`, gated by the `foundry-local` feature.

**FR-003** (State-driven) While the `foundry-local` feature is disabled, the system shall omit the `FoundryLocalProvider` from the default provider registry and shall not link the `foundry-local` crate.

### Service Lifecycle & Auto-Installation

**FR-004** (Event-driven) When the `FoundryLocalProvider` is instantiated for the first time, the system shall invoke `FoundryLocalManager::create()` with a default `FoundryLocalConfig` to initialise the SDK singleton.

**FR-005** (Event-driven) When the local web service is not running at the moment a chat request is initiated, the system shall automatically call `start_web_service()` and wait for `urls()` to return at least one valid endpoint URL before proceeding.

**FR-006** (Ubiquitous) The system shall cache the resolved local endpoint URL after the first successful service start and reuse it for subsequent requests within the same process lifetime, avoiding redundant service starts.

**FR-007** (Optional) Where the Foundry Local CLI or runtime binaries are not present on the host system, the system shall surface a clear, actionable error message directing the user to install Foundry Local via the official Microsoft installer or `winget` / `brew`.

### Provider Trait Implementation

**FR-008** (Ubiquitous) The system shall implement the `Provider` trait for `FoundryLocalProvider` with `id()` returning `"foundry_local"` and `name()` returning `"Microsoft Foundry Local"`.

**FR-009** (Ubiquitous) The system shall register `FoundryLocalProvider` in `create_default_registry()` conditional on the `foundry-local` feature being enabled at compile time.

**FR-010** (Ubiquitous) The system shall provide a default model catalog including at minimum: `phi-4`, `phi-3.5-mini`, and `phi-3.5-moe`, mapped to their Foundry Local model identifiers.

### Model Discovery

**FR-011** (Event-driven) When `list_models()` or the TUI model picker requests the available model list, the system shall query `FoundryLocalManager::catalog()` and return all models whose `runtime` field indicates local CPU/GPU compatibility.

**FR-012** (State-driven) While the Foundry Local catalog contains no locally-downloaded models, the system shall still expose the default model catalog (FR-010) and surface a TUI notification guiding the user to run `foundry-local model pull <model-id>`.

### Client & API Compatibility

**FR-013** (Ubiquitous) The system shall construct an `LlmClient` for the Foundry Local provider by creating a `ChatClient` via the SDK's OpenAI-compatible API and wrapping it in a thin adapter that translates between ragent's `ChatRequest`/`StreamEvent` types and the SDK's types.

**FR-014** (Ubiquitous) The system shall map ragent `ChatRequest` fields (`model`, `messages`, `temperature`, `max_tokens`, `tools`) to the SDK `ChatClient` builder methods before sending the request.

**FR-015** (Ubiquitous) The system shall stream response chunks from the SDK's `ChatCompletionStream` into ragent `StreamEvent::TextDelta` events, preserving the incremental delivery contract.

**FR-016** (Ubiquitous) The system shall convert SDK tool-use blocks into ragent `StreamEvent::ToolCallStart`, `ToolCallDelta`, and `ToolCallEnd` events, matching the existing Anthropic/OpenAI provider behaviour.

**FR-017** (Ubiquitous) The system shall emit `StreamEvent::Usage` with `input_tokens` and `output_tokens` when the SDK response includes usage metadata.

**FR-018** (Ubiquitous) The system shall emit `StreamEvent::Finish { reason }` when the SDK stream ends, mapping the SDK finish reason to ragent's `FinishReason` enum.

### Configuration

**FR-019** (Ubiquitous) The system shall accept the following configuration via `ragent.json` under `provider.foundry_local`:

| Field | Type | Default | Description |
|---|---|---|---|
| `auto_start` | boolean | `true` | Automatically start the local web service on first use |
| `device` | string | `"auto"` | Preferred inference device: `auto`, `cpu`, `gpu`, or `npu` |
| `models_path` | string | — | Override path for the local model cache directory |

**FR-020** (Event-driven) When `provider.foundry_local.device` is set to `"cpu"`, the system shall pass `DeviceType::Cpu` to the SDK `ModelSettings` builder when creating model clients.

**FR-021** (Event-driven) When `provider.foundry_local.device` is set to `"gpu"`, the system shall pass `DeviceType::Gpu` to the SDK `ModelSettings` builder.

**FR-022** (Event-driven) When `provider.foundry_local.auto_start` is set to `false`, the system shall not invoke `start_web_service()` automatically; instead, it shall return an error if the service is not already running.

### Error Handling

**FR-023** (Unwanted) If the Foundry Local web service fails to start (e.g. port conflict, missing runtime, or permission error), the system shall not silently fall back to a cloud provider; it shall return a descriptive `anyhow::Error` with the SDK's underlying error message.

**FR-024** (Event-driven) When a requested model is not present in the local catalog and `auto_start` is enabled, the system shall return an error listing the model ID and suggesting the `foundry-local model pull <id>` command.

**FR-025** (Ubiquitous) The system shall implement `Provider::fetch_usage()` for `FoundryLocalProvider` by returning `None`, because local inference does not expose plan or quota information.

### TUI Integration

**FR-026** (Ubiquitous) The system shall display the Foundry Local provider in the TUI provider list with a `[local]` badge, distinguishing it from cloud providers.

**FR-027** (Event-driven) When the user selects a Foundry Local model in the TUI, the system shall show a status indicator (`Local — ready` or `Local — starting`) reflecting the web service state.

### Non-Functional Requirements

**NFR-001** The provider shall not block the async runtime for more than 500 ms during service startup; any synchronous SDK calls shall be wrapped in `tokio::task::spawn_blocking`.

**NFR-002** The provider shall support streaming responses with a first-token latency of less than 5 seconds on a typical consumer CPU for the default `phi-3.5-mini` model.

**NFR-003** The provider shall be covered by unit tests for SDK initialisation, model catalog mapping, and error paths; integration tests are optional due to the external runtime dependency.

## Configuration Example

```jsonc
{
  "provider": {
    "foundry_local": {
      "auto_start": true,
      "device": "auto",
      "models_path": "~/.foundry-local/models"
    }
  }
}
```

## Out of Scope

- Automatic model download / pulling (user must run `foundry-local model pull` externally).
- Fine-tuning or training workflows via the SDK.
- Embedding, audio, or image-generation APIs (chat completions only for the initial release).
- Cross-platform NPU detection logic beyond what the SDK already handles.
