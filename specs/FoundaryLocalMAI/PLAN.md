# FoundaryLocalMAI — Implementation Plan

## Architecture

The Foundry Local provider follows the same patterns as existing local/embedded providers (e.g. `embedded` candle backend and `ollama`):

1. **New module**: `crates/ragent-llm/src/providers/foundry_local.rs`
2. **Provider struct**: `FoundryLocalProvider` implementing the `Provider` trait
3. **Client struct**: `FoundryLocalClient` implementing `LlmClient` via the SDK's OpenAI-compatible `ChatClient`
4. **Service manager**: Thin wrapper around `FoundryLocalManager` singleton for lifecycle (init, start, stop, URL cache)
5. **Model catalog adapter**: Converts SDK `Catalog` / `Model` entries into ragent `ModelInfo` structs
6. **Feature gating**: All code lives behind `#[cfg(feature = "foundry-local")]` to avoid linking the SDK when disabled
7. **Registry wiring**: Conditional registration in `create_default_registry()`

### Dependency Decision

Rather than reimplementing the Foundry Local HTTP protocol (which is OpenAI-compatible but has service-management quirks), we consume the official `foundry-local` Rust SDK. This is a small, focused dependency (~2 MB) that handles:

- Binary discovery and lifecycle
- Model catalog enumeration
- OpenAI-compatible `ChatClient` with streaming
- Device-type selection (CPU/GPU/NPU)

This matches the spirit of the existing `ollama` provider, which also relies on an external runtime.

### File Layout

```
crates/ragent-llm/src/providers/
├── foundry_local.rs          # FoundryLocalProvider + FoundryLocalClient + service wrapper
└── mod.rs                    # +1 line: #[cfg(feature = "foundry-local")] pub mod foundry_local;

crates/ragent-llm/src/
└── lib.rs                    # +1 line: re-export behind feature gate

crates/ragent-llm/tests/
└── test_foundry_local_provider.rs  # Unit tests for catalog mapping, config, error paths

crates/ragent-llm/Cargo.toml
└── [dependencies]             # + optional dep: foundry-local = "0.2"
```

## Tasks

| ID | Title | Requirement | Effort | Priority | Status | Dependencies |
|----|-------|-------------|--------|----------|--------|--------------|
| T-001 | Add `foundry-local` feature and optional dependency | FR-001, FR-002, FR-003 | S | Critical | completed | — |
| T-002 | Implement `FoundryLocalService` wrapper (SDK lifecycle) | FR-004, FR-005, FR-006, FR-007 | M | Critical | completed | T-001 |
| T-003 | Implement `FoundryLocalProvider` struct (Provider trait) | FR-008, FR-009, FR-010 | M | Critical | completed | T-002 |
| T-004 | Implement `FoundryLocalClient` (LlmClient trait via SDK ChatClient) | FR-013, FR-014, FR-015, FR-016, FR-017, FR-018 | L | Critical | completed | T-002 |
| T-005 | Implement model discovery and catalog mapping | FR-011, FR-012 | M | High | completed | T-003 |
| T-006 | Register provider in default registry behind feature gate | FR-009 | S | Critical | completed | T-003 |
| T-007 | Implement configuration parsing (auto_start, device, models_path) | FR-019, FR-020, FR-021, FR-022 | M | High | completed | T-003 |
| T-008 | Implement error handling for missing runtime / service failures | FR-023, FR-024 | M | High | completed | T-002, T-004 |
| T-009 | Add TUI integration (provider list, status indicator) | FR-026, FR-027 | S | Medium | completed | T-006 |
| T-010 | Write unit tests for service wrapper and catalog mapping | NFR-003, FR-010, FR-011 | M | Medium | completed | T-002, T-005 |
| T-011 | Write unit tests for request/response translation | FR-014, FR-015, FR-016, FR-017, FR-018 | M | Medium | completed | T-004 |
| T-012 | Write integration test stub (conditional on SDK presence) | NFR-003 | S | Low | completed | T-004 |
| T-013 | Update PROVIDERS.md and SPEC.md documentation | — | S | Low | completed | T-006 |
| T-014 | Add `/internal-llm foundry` slash command support | — | S | Low | completed | T-003 |
## Task Details

### T-001 — Add `foundry-local` Feature and Optional Dependency (S, Critical)

Modify `crates/ragent-llm/Cargo.toml`:

- Add feature `foundry-local = ["dep:foundry-local"]`
- Add `[dependencies.foundry-local]` section with `version = "0.2"`, `optional = true`
- Add feature re-export in `src/lib.rs` (or re-export in `src/providers/mod.rs`)
- Ensure `cargo check` and `cargo check --features foundry-local` both pass

### T-002 — Implement `FoundryLocalService` Wrapper (M, Critical)

Create `FoundryLocalService` struct in `foundry_local.rs`:

- `new()` — calls `FoundryLocalManager::create(FoundryLocalConfig::default())`, wraps the `&'static` singleton in an `Arc`
- `ensure_running().await` — checks `urls()`. If empty and `auto_start` is true, calls `start_web_service().await`, polls `urls()` with a 30-second timeout, and caches the first URL in an `Arc<Mutex<Option<String>>>`
- `cached_url()` — returns the cached URL if present
- `device_type(config: &FoundryLocalConfig) -> DeviceType` — maps config string `"cpu"`/`"gpu"`/`"npu"`/`"auto"` to SDK `DeviceType`
- `is_installed() -> bool` — probes whether the Foundry Local CLI/runtime is discoverable by the SDK (or checks `which foundry-local` as fallback)
- Error paths return descriptive `anyhow::Error`s (FR-007, FR-023)

### T-003 — Implement `FoundryLocalProvider` Struct (M, Critical)

Implement `Provider` for `FoundryLocalProvider`:

- `id()` → `"foundry_local"`, `name()` → `"Microsoft Foundry Local"` (FR-008)
- `default_models()` → static `Vec<ModelInfo>` for `phi-4`, `phi-3.5-mini`, `phi-3.5-moe` with reasonable cost/capability defaults (FR-010)
- `create_client()` — resolves config from `options`, constructs `FoundryLocalService`, calls `ensure_running().await`, selects the target model from the catalog, builds a `ChatClient`, and boxes it as `FoundryLocalClient`
- `fetch_usage()` → `None` (FR-025)

### T-004 — Implement `FoundryLocalClient` (L, Critical)

Wrap the SDK `ChatClient` to implement `LlmClient`:

- `chat(request: ChatRequest)` → stream of `StreamEvent`
- Map `ChatRequest`:
  - `model` → SDK `ChatClient::new(model_id)`
  - `messages` → SDK message list (system as first message, then user/assistant/tool alternation)
  - `temperature` → `ChatClient::temperature()`
  - `max_tokens` → `ChatClient::max_tokens()`
  - `tools` → `ChatClient::tools()` (JSON schema conversion)
- Stream conversion:
  - SDK `TextDelta` → `StreamEvent::TextDelta`
  - SDK `ToolCallStart` → `StreamEvent::ToolCallStart`
  - SDK `ToolCallDelta` → `StreamEvent::ToolCallDelta`
  - SDK `ToolCallEnd` → `StreamEvent::ToolCallEnd`
  - SDK `Usage` → `StreamEvent::Usage`
  - SDK `Finish` → `StreamEvent::Finish`
- Error handling: catch SDK errors and wrap in `anyhow::Error` with provider context

### T-005 — Implement Model Discovery and Catalog Mapping (M, High)

- `discover_models(service: &FoundryLocalService) -> Vec<ModelInfo>` queries `manager.catalog()` and iterates over available models
- Filter by `runtime` compatibility (CPU/GPU/NPU matching config device preference)
- Map each SDK `Model` to ragent `ModelInfo` with:
  - `id` → `model.id()`
  - `provider_id` → `"foundry_local"`
  - `name` → `model.display_name()` or fallback to `model.id()`
  - `capabilities` → infer from model metadata (default: streaming=true, tool_use=false unless known)
  - `context_window` → from model metadata or default 128k for Phi-4, 32k for Phi-3.5
- If catalog is empty, return the static default catalog (FR-012) and log a warning

### T-006 — Registry Registration (S, Critical)

In `providers/mod.rs`:

- Add `#[cfg(feature = "foundry-local")] pub mod foundry_local;`
- In `create_default_registry()`, add `#[cfg(feature = "foundry-local")] registry.register(Box::new(foundry_local::FoundryLocalProvider));`

### T-007 — Configuration Parsing (M, High)

Parse `provider.foundry_local` from `ragent.json` into a `FoundryLocalConfig` struct:

- `auto_start: bool` (default `true`)
- `device: String` (default `"auto"`)
- `models_path: Option<String>`
- Pass `models_path` to the SDK `FoundryLocalConfig` builder if present
- Validate `device` values; reject unknown values with a clear error

### T-008 — Error Handling for Missing Runtime / Service Failures (M, High)

- `is_installed()` check before `create()`; if false, return error with install instructions (FR-007)
- `start_web_service()` timeout: if `urls()` stays empty after 30 s, return error with diagnostic suggestions (port conflict, firewall, missing runtime)
- Model-not-found error: include model ID and `foundry-local model pull <id>` suggestion (FR-024)
- No silent fallback to cloud providers (FR-023)

### T-009 — TUI Integration (S, Medium)

- In the TUI provider list (`app.rs` or `app/state.rs`), append `[local]` badge to `FoundryLocalProvider` name
- Status bar / model picker: show `Local — ready` (URL cached) or `Local — starting` (service start in progress)
- Reuse existing TUI infrastructure for provider badges (similar to `[custom]` for custom agents)

### T-010 — Unit Tests for Service Wrapper and Catalog Mapping (M, Medium)

- `test_foundry_local_service_caches_url` — verifies URL caching after first start
- `test_foundry_local_device_type_mapping` — validates `"cpu"` → `DeviceType::Cpu`, etc.
- `test_foundry_local_catalog_empty_fallback` — confirms static defaults are returned when catalog is empty
- `test_foundry_local_model_info_mapping` — checks correct mapping of SDK `Model` to `ModelInfo`
- Use `tempfile` and mocked SDK responses where the SDK permits; otherwise use compile-time `cfg(test)` stubs

### T-011 — Unit Tests for Request/Response Translation (M, Medium)

- `test_chat_request_to_sdk_builder` — validates temperature, max_tokens, model ID mapping
- `test_text_delta_stream_conversion` — checks `TextDelta` → `StreamEvent::TextDelta`
- `test_tool_call_stream_conversion` — verifies tool-use block splitting into start/delta/end events
- `test_usage_emitted` — confirms `StreamEvent::Usage` is produced when SDK provides usage
- `test_finish_reason_mapping` — maps SDK finish reasons to ragent `FinishReason`

### T-012 — Integration Test Stub (S, Low)

- `tests/test_foundry_local_provider.rs`
- Skipped by default (`#[ignore = "requires foundry-local runtime"]`)
- Can be run manually with `cargo test --features foundry-local -- --ignored` when the runtime is installed
- Verifies end-to-end: provider construction → service start → simple chat → stream events → finish

### T-013 — Documentation Update (S, Low)

- Add Foundry Local section to `PROVIDERS.md` with:
  - Installation link (`winget install Microsoft.FoundryLocal` or `brew install foundry-local`)
  - Feature flag (`--features foundry-local`)
  - Config snippet
  - Model list
- Update `SPEC.md` provider list

### T-014 — `/internal-llm foundry` Slash Command (S, Low)

- Add `"foundry"` variant to the internal-LLM backend selector in `ragent-tui/src/app.rs`
- When selected, set the internal LLM provider to `foundry_local` with `phi-3.5-mini` as the default model
- Display status in the internal-LLM panel (`/internal-llm show`)

## Estimated Effort

| Phase | Tasks | Total Effort |
|---|---|---|
| Foundation | T-001, T-002, T-006 | S + M + S = ~2 days |
| Core Provider | T-003, T-004, T-005 | M + L + M = ~4 days |
| Polish | T-007, T-008, T-009 | M + M + S = ~2 days |
| Testing | T-010, T-011, T-012 | M + M + S = ~2 days |
| Docs & Extras | T-013, T-014 | S + S = ~1 day |
| **Total** | | **~11 days** |

## Risks

| Risk | Likelihood | Impact | Mitigation |
|---|---|---|---|
| `foundry-local` crate API changes before v1 | Medium | High | Pin to `=0.2.x` in `Cargo.toml`; monitor SDK release notes |
| SDK binary not discoverable on non-Windows platforms | Medium | Medium | Provide `is_installed()` fallback (`which foundry-local`); document platform support |
| Service startup latency > 5 s on slow CPUs | High | Low | Acceptable for local inference; cache URL; show TUI spinner |
| Feature-flag compilation breaks CI (SDK not present) | Medium | Medium | Only enable feature in a dedicated CI job; default CI stays `default` features |
| NPU/GPU runtime unavailable on host | Medium | Low | Default to `"auto"`; SDK handles fallback to CPU |