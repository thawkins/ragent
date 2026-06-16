---
status: implemented
audit:
  - { time: 1781496227, from: "none", to: "draft", actor: "system" }
---
# FoundryInProc — In-Process Foundry Local Provider

## Overview

This specification defines a plan to convert the **Microsoft Foundry Local** provider in `ragent-llm` from its current **web-service-backed** architecture to an **in-process** architecture.  Today the provider starts a local Foundry Local web service and routes chat requests to its OpenAI-compatible HTTP endpoint.  The goal is to use the `foundry-local-sdk` directly in the same process as ragent, removing the HTTP server hop and relying on the SDK's native core library for model lifecycle, inference, and streaming.

The in-process path is expected to reduce first-token latency, remove port and process-management failure modes, and simplify deployment.  The conversion must remain feature-compatible with the current provider: the same model aliases, streaming events, tool-use translation, and TUI status indicators must continue to work.

## Requirements

### Dependency & Feature Gating

**FR-001** (Ubiquitous) The system shall compile the Foundry Local provider always, the `foundry-local-sdk` dependeny should always be included.

**FR-002** the system should ALWAYS link the `foundry-local-sdk` and it MUST not be feature gated, the `foundry-local` Provider MUST always be present.

### Service Architecture Conversion

**FR-003** (Ubiquitous) The system shall provide an in-process inference path for the Foundry Local provider that does not require a local web service to be running.

**FR-004** (Event-driven) When an in-process chat request is initiated, the system shall use the SDK's native core library to load and run the model inside the ragent process.

**FR-005** (Ubiquitous) The system shall continue to support the existing web-service-backed path until the in-process path is fully verified.

**FR-006** (Event-driven) When the `foundry_local` provider configuration key `in_process` is set to `true`, the system shall select the in-process inference path.

**FR-007** (Optional) Where the user does not explicitly set `in_process`, the system shall default to the current web-service path so that existing behaviour is preserved.

### Model Lifecycle

**FR-008** (Ubiquitous) The system shall ensure the requested model is downloaded to the local cache before attempting in-process inference.

**FR-009** (Event-driven) When the requested model is not cached, the system shall invoke `Model::download()` through the SDK and publish download progress events to the event bus.

**FR-010** (Event-driven) When the model is cached but not loaded in-process, the system shall load it via the SDK's in-process model loading API before the first chat token is produced.

**FR-011** (State-driven) While a model is loaded in-process, the system shall reuse the loaded instance across chat requests in the same process lifetime.

**FR-012** (Ubiquitous) The system shall resolve model aliases (e.g. `phi-4`) to concrete SDK variant ids using the same selection logic as the web-service path.

### Inference & Streaming

**FR-013** (Ubiquitous) The system shall implement `LlmClient` for the in-process Foundry Local backend and return a stream of `StreamEvent` values.

**FR-014** (Ubiquitous) The system shall map ragent `ChatRequest` fields (`model`, `messages`, `temperature`, `max_tokens`, `tools`) to the SDK's in-process chat API.

**FR-015** (Ubiquitous) The system shall stream incremental response chunks into `StreamEvent::TextDelta` events.

**FR-016** (Ubiquitous) The system shall translate SDK tool-use blocks into `StreamEvent::ToolCallStart`, `StreamEvent::ToolCallDelta`, and `StreamEvent::ToolCallEnd` events.

**FR-017** (Ubiquitous) The system shall emit `StreamEvent::Usage` with `input_tokens` and `output_tokens` when usage metadata is available.

**FR-018** (Ubiquitous) The system shall emit `StreamEvent::Finish { reason }` when the in-process stream ends.

### Error Handling

**FR-019** (Unwanted) The system shall not silently fall back to the web-service path if the in-process path fails; it shall return a descriptive `anyhow::Error`.

**FR-020** (Event-driven) When the SDK reports that a requested model is not in the local catalog, the system shall return an error listing the model id and suggesting the `foundry model pull <id>` command.

**FR-021** (Ubiquitous) The system shall surface a clear, actionable error when the native core library is missing, incompatible, or cannot be loaded by the SDK.

### Configuration

**FR-022** (Ubiquitous) The system shall accept the following configuration via `ragent.json` under `provider.foundry_local`:

| Field | Type | Default | Description |
|---|---|---|---|
| `in_process` | boolean | `false` | Use in-process inference instead of the web service |
| `device` | string | `"auto"` | Preferred inference device: `auto`, `cpu`, `gpu`, or `npu` |
| `models_path` | string | — | Override path for the local model cache directory |
| `auto_start` | boolean | `true` | (web-service path only) Start the local web service automatically |

**FR-023** (Event-driven) When `provider.foundry_local.device` is set to `"cpu"`, the system shall pass `DeviceType::Cpu` to the SDK when creating the in-process model session.

**FR-024** (Event-driven) When `provider.foundry_local.device` is set to `"gpu"`, the system shall pass `DeviceType::Gpu` to the SDK when creating the in-process model session.

### TUI & CLI Integration

**FR-025** (Ubiquitous) The system shall display the Foundry Local provider in the TUI provider list and model picker with a `[local]` badge.

**FR-026** (Event-driven) When the user selects a Foundry Local model in the TUI, the system shall show a status indicator reflecting whether the web service or the in-process backend is active.

**FR-027** (Event-driven) When `/internal-llm foundry` is invoked, the system shall default to the in-process backend if `in_process` is not explicitly set to `false`.

### Backward Compatibility & Migration

**FR-028** (Ubiquitous) The system shall keep the existing `FoundryLocalClient` (web-service path) unchanged except for renaming if necessary to disambiguate the new in-process client.

**FR-029** (Ubiquitous) The system shall keep the existing `FoundryLocalProvider` API and `Provider` trait implementation stable so that existing callers do not require changes.

**FR-030** (Ubiquitous) The system shall expose a runtime flag or environment variable that allows operators to force the web-service path for debugging or compatibility.

### Non-Functional Requirements

**NFR-001** The in-process provider shall support streaming responses with a first-token latency no worse than the current web-service path for the same model on the same hardware.

**NFR-002** The in-process provider shall not block the async runtime for more than 500 ms during model loading; any synchronous SDK calls shall be wrapped in `tokio::task::spawn_blocking`.

**NFR-003** The in-process provider shall be covered by unit tests for request translation, model alias resolution, and SDK error mapping; integration tests are optional due to the external runtime dependency.

## Configuration Example

```jsonc
{
  "provider": {
    "foundry_local": {
      "in_process": true,
      "device": "auto",
      "models_path": "~/.foundry-local/models"
    }
  }
}
```

## Out of Scope

- Automatic model download / pulling (user must run `foundry model pull` externally).
- Fine-tuning or training workflows via the SDK.
- Embedding, audio, or image-generation APIs (chat completions only).
- Removing the web-service path in this release; it remains available behind `in_process: false`.
