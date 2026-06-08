---
status: implemented
audit:
  - { time: 1780920414, from: "none", to: "draft", actor: "system" }
  - { time: 1780928000, from: "draft", to: "implemented", actor: "agent" }
---
# LiteRTInternalLLM — Specification

## Overview

Replace the current Candle-based embedded internal-LLM runtime with Google's
LiteRT-LM edge runtime, using the `litertlm` and `litert` Rust crates
(`litert-rs` workspace). LiteRT-LM provides cross-platform, hardware-accelerated
on-device LLM inference supporting CPU, GPU, and NPU backends — a significant
upgrade over the current pure-CPU Candle/GGUF implementation.

The change introduces a new `LitertLmBackend` implementing the existing
`EmbeddedBackend` trait, a new `litertlm` feature flag, updated configuration,
model format migration from `.gguf` to `.litertlm`, and installation
instructions surfaced via `/internal-llm help`.

## Terminology

| Term | Meaning |
|---|---|
| **LiteRT** | Google's on-device ML runtime, formerly TensorFlow Lite |
| **LiteRT-LM** | The LLM inference component of LiteRT (text generation) |
| **litert-rs** | The Rust workspace (`litert` + `litertlm` + `litert-sys` + `litert-lm-sys` crates) providing safe Rust bindings |
| **Candle** | The current pure-Rust GGUF inference backend (HuggingFace `candle`) |
| **EmbeddedBackend** | ragent's trait for pluggable inference backends |
| **.litertlm** | LiteRT-LM model file format (converted from GGUF/SafeTensors via the `litertlm` CLI) |

## Requirements

### FR-001: LiteRT-LM Backend Implementation

The system **shall** provide a `LitertLmBackend` struct that implements the
`EmbeddedBackend` trait defined in `crates/ragent-llm/src/embedded/mod.rs`,
using the `litertlm` crate for model loading and inference.

### FR-002: Feature Flag Gating

The system **shall** gate the LiteRT-LM backend behind a Cargo feature flag
`litertlm` in the `ragent-llm` crate. When the feature is disabled, the
existing `CandleBackend` **shall** remain the default and only embedded-LLM
backend.

### FR-003: Backend Selection via Configuration

When the `internal_llm.backend` configuration field is set to `"litertlm"`,
the system **shall** use `LitertLmBackend` for inference. When set to
`"candle"` (or left at the default), the system **shall** use
`CandleBackend`. If the selected backend's feature flag is not compiled in, the
system **shall** log a warning and fall back to the available backend.

### FR-004: Model Artifact Format

When `internal_llm.backend` is `"litertlm"`, the system **shall** expect model
artifacts in `.litertlm` format instead of `.gguf`. The
`EmbeddedModelManifest.artifacts` list **shall** reference a `.litertlm` file
and an accompanying `tokenizer.json`.

### FR-005: Hardware Acceleration

When the `litertlm` feature is enabled and the `internal_llm.backend` is
`"litertlm"`, the system **shall** support the following accelerator backends
via `litertlm::Backend`:

| Config value | `litertlm::Backend` variant | Description |
|---|---|---|
| `"cpu"` | `Backend::Cpu` | CPU-only inference (default) |
| `"gpu"` | `Backend::Gpu` | GPU acceleration (Metal/CUDA/Vulkan) |
| `"npu"` | `Backend::Npu` | NPU acceleration where available |

The `internal_llm` config **shall** gain a new field
`accelerator: String` (default `"cpu"`) to select the backend.

Configuration example (`ragent.json`):

```json
{
  "internal_llm": {
    "enabled": true,
    "backend": "litertlm",
    "model_id": "gemma-3-1b-it-litertlm",
    "accelerator": "cpu",
    "context_window": 2048,
    "max_output_tokens": 256
  }
}
```

### FR-006: Graceful Degradation on Missing Runtime

If the LiteRT-LM native runtime libraries cannot be loaded at startup (missing
`.so`/`.dylib`/`.dll`, incompatible architecture), the system **shall** log a
warning and mark the embedded runtime as `Failed` rather than panicking. The
`InternalLlmService` **shall** record a fallback event so that compaction and
other tasks fall back to the provider LLM.

### FR-007: Installation Instructions in `/internal-llm help`

The `/internal-llm help` command **shall** include a section describing how to
install and configure the LiteRT-LM runtime. This section **shall** cover:

1. Minimum Rust version required
2. No extra build flags required — `litertlm` is included by default
3. System library dependencies (none required — `litertlm` downloads prebuilt
   binaries via build scripts)
4. How to convert a GGUF model to `.litertlm` format using the LiteRT-LM CLI
5. Where to place model files (`~/.local/share/ragent/embedded/<model_id>/`)
6. The `accelerator` configuration field (`"cpu"`, `"gpu"`, or `"npu"`)

The current `/internal-llm help` output format:

```
/internal-llm help

Usage:
/internal-llm show — Display current status and configuration
/internal-llm on|off — Enable or disable the internal LLM
/internal-llm chat — Open an interactive chat panel
/internal-llm sessiontitle on|off — Toggle session title generation
/internal-llm promptcontext on|off — Toggle prompt/context compaction
/internal-llm memoryextraction on|off — Toggle memory extraction

Backends:
- `litertlm` — Google LiteRT-LM on-device inference (CPU/GPU/NPU). Enabled by default.
- `candle` — Pure-Rust GGUF inference (Candle). Requires `--features embedded-llm`.

LiteRT-LM Setup:
No extra build flags required — `litertlm` is included by default.
1. Convert a GGUF/SafeTensors model to `.litertlm` format:
   `litertlm convert --input model.gguf --output model.litertlm`
2. Place the `.litertlm` file and `tokenizer.json` in
   `~/.local/share/ragent/embedded/<model_id>/`
3. Set `internal_llm.backend` to `"litertlm"` and `internal_llm.model_id`
   to the model directory name (e.g. `gemma-3-1b-it-litertlm`).

Accelerator:
Set `internal_llm.accelerator` to `"cpu"` (default), `"gpu"`, or `"npu"`.
LiteRT-LM manages its own thread pool; the `threads` setting is ignored when
using the `litertlm` backend.
```

### FR-008: Streaming Token Callbacks

The `LitertLmBackend` **shall** support token-by-token generation via the
`litertlm` crate's streaming API (`send_message_stream`). The backend
**shall** collect streamed tokens into a complete string before returning, so
that the existing `InternalLlmExecutor::execute` interface remains
synchronous.

### FR-009: Context Window and Token Limits

When `internal_llm.backend` is `"litertlm"`, the
`internal_llm.context_window` and `internal_llm.max_output_tokens` configuration
fields **shall** be passed through to `litertlm::EngineSettings` as
`max_num_tokens`. The engine **shall** truncate prompts that exceed the
configured context window.

### FR-010: Cancellation Support

The `LitertLmBackend` **shall** respect the `InferenceControls.cancel_flag`
passed during inference. When the flag is set, the backend **shall** abort the
ongoing `litertlm` generation and return `EmbeddedInferenceError::Cancelled`.

### FR-011: Timeout Enforcement

The `LitertLmBackend` **shall** respect the
`InferenceControls.deadline` passed during inference. If generation exceeds the
deadline, the backend **shall** return
`EmbeddedInferenceError::DeadlineExceeded`.

### FR-012: Runtime Status Reporting

The `LitertLmBackend` **shall** report its status through the existing
`EmbeddedRuntimeStatus` and `EmbeddedRuntimeSettings` types. The
`execution_device` field **shall** reflect the configured accelerator
(`"cpu"`, `"gpu"`, or `"npu"`). The `quantized_runtime` field **shall** read
`"litertlm via litert-lm C API"`.

### FR-013: Thread Configuration

When `internal_llm.backend` is `"litertlm"`, the `internal_llm.threads`
configuration field **shall** be ignored because LiteRT-LM manages its own
thread pool. The `threading` field in `EmbeddedRuntimeSettings` **shall**
report `"LiteRT-LM manages its own thread pool internally"`.

### FR-014: Existing Candle Backend Preservation

The `CandleBackend` and all existing `embedded-llm` feature-gated code
**shall not** be removed. The `embedded-llm` feature flag **shall** continue
to compile and function exactly as before. The two backends **shall** coexist
behind their respective feature flags.

### FR-015: Default Model for LiteRT-LM

When `internal_llm.backend` is `"litertlm"` and no model is explicitly
configured, the system **shall** default to `gemma-3-1b-it-litertlm` as the
model identifier. This matches a small, widely-available model suitable for
internal helper tasks.

### FR-016: Build Configuration

The `ragent-llm` crate's `Cargo.toml` **shall** declare an optional dependency
on the `litertlm` crate. The `ragent-agent` crate's `Cargo.toml` **shall**
expose a `litertlm` feature that enables the dependency and the
`LitertLmBackend` module.

### FR-017: No New Mandatory System Dependencies

When the `litertlm` feature is not enabled, the build **shall** succeed without
any LiteRT-LM system libraries, C toolchains, or Bazel installations. The
`litertlm` crate downloads prebuilt native binaries during its build script,
requiring no system-level installation by the user beyond a standard Rust
toolchain.

### FR-018: Unwanted: Candle Backend Removal

The system **shall not** remove or disable the existing `CandleBackend` or the
`embedded-llm` feature flag. Both backends must remain available so that users
who prefer or require Candle can continue using it without migration.

### FR-019: Unwanted: Mandatory Model Download

The system **shall not** automatically download `.litertlm` model files at build
time or startup without explicit user consent. Model downloads **shall** follow
the existing `download_policy` configuration (always / on-demand / never),
consistent with the current GGUF artifact download behaviour.

### FR-020: Event-Driven: Auto-Download on First Use

When `internal_llm.download_policy` is `"on_demand"` and the `.litertlm`
model file is absent from the cache, the system **shall** attempt to download
it from the manifest's `source_url` on the first inference request. If the
download fails, the system **shall** record a fallback event and skip internal
LLM processing for that request.

### FR-021: State-Driven: Feature Availability Check

When the user invokes `/internal-llm show`, the system **shall** display
whether the `litertlm` feature is compiled in, which backend is active, and
the accelerator configuration. When `litertlm` is the default feature
(per FR-025), `/internal-llm show` **shall** indicate that `litertlm` is
compiled in by default.

When the feature is not compiled in but `internal_llm.backend` is set to
`"litertlm"`, the system **shall** show a warning indicating that the feature
must be rebuilt with `--features litertlm`.

The `/internal-llm show` output **shall** include the following fields:

- `enabled` — whether the internal LLM subsystem is on or off
- `backend` — the configured backend (`litertlm` or `candle`)
- `model` — the configured model identifier
- `session title` — whether session title generation is enabled
- `prompt/context compaction` — whether prompt/context compaction is enabled
- `memory extraction prefilter` — whether memory extraction is enabled
- `chat mode` — whether the chat panel is active
- When runtime is available: execution device, quantized runtime, threading,
  GPU offload, backend name, cache root, model dir
- Feature availability: `litertlm` compiled in by default (or warning if not)

### FR-022: Optional: GPU/NPU Acceleration

When `internal_llm.accelerator` is set to `"gpu"` or `"npu"`, the system
**shall** attempt to use the corresponding `litertlm::Backend` variant. If the
requested accelerator is not available on the host platform, the system
**shall** fall back to CPU inference and log a warning.

### FR-023: Optional: Model Conversion Guidance

When the user runs `/internal-llm help`, the system **shall** display a
section titled "Converting models for LiteRT-LM" that explains how to use the
`litertlm` CLI tool to convert a GGUF or SafeTensors model to the `.litertlm`
format, including the command:

```
litertlm convert --input model.gguf --output model.litertlm
```

### FR-024: Test Coverage

The `LitertLmBackend` **shall** have unit tests covering:

1. Backend selection logic (candle vs litertlm based on config)
2. Feature-flag gating (litertlm code is not compiled when the feature is off)
3. Configuration parsing for the new `accelerator` field
4. Graceful fallback when the runtime fails to load
5. Cancellation and timeout error mapping

Integration tests that require the `litertlm` feature **shall** be gated behind
the feature flag.

### FR-025: Default Feature Enablement

The `litertlm` feature **shall** be included in the default feature set of the
workspace `Cargo.toml` so that `cargo build` (without explicit `--features`)
produces a binary with LiteRT-LM support enabled. Users **shall not** need to
pass `--features litertlm` to obtain LiteRT-LM functionality.

### FR-026: Candle as Optional Feature

When the `litertlm` feature is the default, the `embedded-llm` (Candle) feature
**shall** become an opt-in feature. The workspace default features **shall**
include `litertlm` and **shall not** include `embedded-llm`. Users who require
the Candle backend **shall** build with `--features embedded-llm`.

### FR-027: Ubiquitous: Backend Availability at Runtime

When the `litertlm` feature is enabled (which it is by default per FR-025),
the `EmbeddedRuntime::availability()` method **shall** return
`RuntimeAvailability::Available` for the LiteRT-LM backend, regardless of
whether the `embedded-llm` feature is also compiled in.

### FR-028: State-Driven: Default Backend Selection

When `internal_llm.backend` is not explicitly set in the configuration file and
the `litertlm` feature is compiled in (default per FR-025), the
`default_internal_llm_backend()` function **shall** return `"litertlm"`. When
only the `embedded-llm` feature is compiled in (user explicitly built with
`--features embedded-llm --no-default-features`), the default **shall** remain
`"candle"`.

### FR-029: Unwanted: Silent Feature Regression

The build **shall not** produce a binary where neither `litertlm` nor
`embedded-llm` is enabled unless the user explicitly passed
`--no-default-features` without selecting a replacement. If both features are
disabled and `internal_llm.enabled` is `true`, the application **shall** log a
warning at startup indicating that no embedded backend is available.