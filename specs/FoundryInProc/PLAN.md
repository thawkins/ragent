# FoundryInProc — Implementation Plan

## Architecture

The conversion to an in-process Foundry Local provider will be carried out in three layers:

1. **In-process client** — a new `FoundryLocalInProcClient` implementing `LlmClient` by calling the SDK's native core chat API directly.  It will reuse the existing `FoundryLocalService` / `FoundryLocalManager` singleton for model catalog access, download orchestration, and in-process model loading.
2. **Provider routing** — extend `FoundryLocalProvider` so that `create_client()` selects either the existing web-service `FoundryLocalClient` or the new in-process client based on the `provider.foundry_local.in_process` configuration flag.
3. **Service lifecycle simplification** — when the in-process path is selected, avoid starting the local web service.  The `auto_start` flag becomes relevant only for the web-service path.

### File Layout

```
crates/ragent-llm/src/providers/
├── foundry_local_client.rs          # existing web-service client (unchanged API)
├── foundry_local_inproc_client.rs   # NEW: in-process LlmClient implementation
├── foundry_local_provider.rs        # EXTENDED: selects web-service vs in-process
├── foundry_local_service.rs         # existing SDK manager wrapper (unchanged)
└── mod.rs                           # re-export new module

crates/ragent-llm/tests/
└── test_foundry_local_inproc.rs     # NEW: unit + conditional integration tests

ragent.json / .ragent/ragent.json    # add in_process and device settings
```

### Key Design Decisions

- **Reuse the SDK manager singleton**: `FoundryLocalManager` is already `&'static`; the new in-process client will hold the same reference as the web-service client, avoiding duplicate initialisation.
- **Keep both paths**: rather than replacing the web-service client, both clients coexist so that `in_process: false` retains exact existing behaviour and provides an escape hatch.
- **Model loading strategy**: the SDK's in-process `model.load()` loads the model into the native core.  For the in-process path this is the correct place to load, unlike the web-service path where `load()` was loading in-process but inference ran in the web service.
- **Device mapping**: map the existing `device` config string to the SDK `DeviceType` enum; this is currently missing from the web-service implementation and will be added for both paths.
- **Configuration default**: `in_process` defaults to `false` to preserve current behaviour.

## Tasks

| ID | Title | Requirement | Effort | Priority | Status | Dependencies |
|----|-------|-------------|--------|----------|--------|--------------|
| T-001 | Survey SDK in-process chat and model loading APIs | FR-004, FR-010, FR-013, FR-014 | S | Critical | completed | — |
| T-002 | Add `in_process` and `device` configuration parsing to provider config | FR-022, FR-023, FR-024 | S | Critical | completed | — |
| T-003 | Implement `FoundryLocalInProcClient` skeleton and `LlmClient` trait wiring | FR-001, FR-013 | M | Critical | completed | T-001 |
| T-004 | Implement model alias resolution and in-process loading in the new client | FR-008, FR-009, FR-010, FR-011, FR-012 | M | Critical | completed | T-003 |
| T-005 | Implement chat request mapping to SDK in-process chat API | FR-014 | M | High | completed | T-003 |
| T-006 | Implement streaming response translation to `StreamEvent`s | FR-015, FR-016, FR-017, FR-018 | L | Critical | completed | T-005 |
| T-007 | Extend `FoundryLocalProvider::create_client` to select web-service vs in-process | FR-003, FR-005, FR-006, FR-007 | M | Critical | completed | T-002, T-004, T-006 |
| T-008 | Add `DeviceType` mapping helper and wire it into both paths | FR-023, FR-024 | S | Medium | completed | T-002 |
| T-009 | Implement error handling for missing native core and catalog errors | FR-019, FR-020, FR-021 | M | High | completed | T-004, T-006 |
| T-010 | Add runtime flag / environment variable to force web-service path | FR-030 | S | Low | completed | T-007 |
| T-011 | Update TUI status indicator for in-process backend | FR-025, FR-026, FR-027 | S | Medium | completed | T-007 |
| T-012 | Write unit tests for request mapping and alias resolution | NFR-003, FR-012, FR-014 | M | Medium | completed | T-003, T-005 |
| T-013 | Write unit tests for stream event translation | NFR-003, FR-015, FR-016, FR-017, FR-018 | M | Medium | completed | T-006 |
| T-014 | Write conditional integration test for in-process chat | NFR-003 | S | Low | completed | T-007 |
| T-015 | Update `PROVIDERS.md` with in-process configuration and migration notes | — | S | Low | completed | T-007 |
| T-016 | Update `SPEC.md` provider list and internal-LLM defaults | FR-027 | S | Low | completed | T-011 |
## Task Details

### T-001 — Survey SDK In-Process Chat and Model Loading APIs (S, Critical)

- Inspect the `foundry-local-sdk` crate documentation and source for the in-process chat API (likely `ChatClient` or a `chat` method on `Model`, plus `Model::load` for in-process loading).
- Confirm how streaming works for in-process inference versus the web-service path.
- Identify the exact types for device selection, finish reasons, usage metadata, and tool-use deltas.
- Document findings in a short note in `docs/reports/foundry_inproc_sdk_survey.md`.

### T-002 — Add `in_process` and `device` Configuration Parsing (S, Critical)

- Extend the provider-specific options struct used by `FoundryLocalProvider::create_client` to include:
  - `in_process: Option<bool>` (default `false`)
  - `device: Option<String>` (default `"auto"`)
- Validate `device` values (`auto`, `cpu`, `gpu`, `npu`); reject unknown values with a clear error.
- Pass the resolved device string into `FoundryLocalService` / both clients.

### T-003 — Implement `FoundryLocalInProcClient` Skeleton and `LlmClient` Trait Wiring (M, Critical)

- Create `crates/ragent-llm/src/providers/foundry_local_inproc_client.rs`.
- Define `pub struct FoundryLocalInProcClient` holding:
  - `manager: &'static foundry_local_sdk::FoundryLocalManager`
  - `event_bus: Option<Arc<EventBus>>`
  - `device: DeviceType`
  - `loaded_model: Mutex<Option<LoadedModelHandle>>` (type to be refined after SDK survey)
- Implement `LlmClient::chat` returning an error stub initially so that `cargo check` passes.
- Re-export the new module from `crates/ragent-llm/src/providers/mod.rs` and `src/lib.rs`.

### T-004 — Implement Model Alias Resolution and In-Process Loading (M, Critical)

- Port the alias/variant resolution logic from `ensure_model_loaded` in `foundry_local_client.rs` into a shared helper or duplicate it in the in-process client, depending on API differences.
- Ensure the model is downloaded if not cached, reusing the existing download-progress event publishing.
- Load the model via the SDK's in-process `model.load()` API and cache the loaded handle per `FoundryLocalInProcClient` instance.
- When a different model is requested, unload the previous model (if the SDK requires it) and load the new one.

### T-005 — Implement Chat Request Mapping to SDK In-Process Chat API (M, High)

- Map ragent `ChatRequest` to the SDK in-process chat builder or direct call:
  - `model` → resolved SDK variant id
  - `messages` → SDK message list (system first, then user/assistant/tool)
  - `temperature` → SDK temperature setter
  - `max_tokens` → SDK max tokens setter
  - `tools` ��� SDK tools JSON schema
- Keep the mapping code in a single private function so it can be unit-tested independently.

### T-006 — Implement Streaming Response Translation to `StreamEvent`s (L, Critical)

- Consume the SDK's in-process stream and translate each chunk:
  - Text delta → `StreamEvent::TextDelta`
  - Tool call start/delta/end → matching `StreamEvent` variants
  - Usage block → `StreamEvent::Usage`
  - Finish reason → `StreamEvent::Finish`
- Return the stream as `Pin<Box<dyn Stream<Item = StreamEvent> + Send>>`.
- Ensure the stream is cancellation-safe and does not block the async runtime.

### T-007 — Extend `FoundryLocalProvider::create_client` to Select Web-Service vs In-Process (M, Critical)

- Read `provider.foundry_local.in_process` from the provider options in `create_client`.
- If `in_process` is `true`:
  - Do not start the web service.
  - Obtain the `FoundryLocalManager` singleton.
  - Construct `FoundryLocalInProcClient` and return it as the boxed `LlmClient`.
- If `in_process` is `false` or unset:
  - Preserve existing behaviour: ensure the web service is running, then return the existing `FoundryLocalClient`.
- Ensure the provider's `id()`, `name()`, and `default_models()` are unaffected.

### T-008 — Add `DeviceType` Mapping Helper and Wire It Into Both Paths (S, Medium)

- Add `fn device_type_from_str(device: &str) -> Result<DeviceType>` in a shared module or `foundry_local_service.rs`.
- Map `"auto"`, `"cpu"`, `"gpu"`, and `"npu"` to the corresponding SDK enum variants.
- Pass the resulting `DeviceType` into the in-process client and log it for the web-service client (for consistency).

### T-009 — Implement Error Handling for Missing Native Core and Catalog Errors (M, High)

- Detect the "native core library missing/incompatible" error from `FoundryLocalManager::create` and surface an actionable message with install instructions (FR-021).
- Detect model-not-in-catalog errors and include `foundry model pull <id>` suggestion (FR-020).
- Ensure in-process path failures do not silently fall through to the web-service path (FR-019).

### T-010 — Add Runtime Flag / Environment Variable to Force Web-Service Path (S, Low)

- Support `RAGENT_FOUNDRY_LOCAL_FORCE_WEB` environment variable.
- When set to `1` or `true`, force `in_process` to `false` regardless of config.
- Document the variable in `PROVIDERS.md` as a debugging / compatibility escape hatch.

### T-011 — Update TUI Status Indicator for In-Process Backend (S, Medium)

- In the TUI provider list and internal-LLM panel, show:
  - `Local — in-process` when the in-process backend is selected
  - `Local — web` when the web-service backend is selected
- Update `/internal-llm foundry` to default to in-process unless `in_process` is explicitly `false`.

### T-012 — Write Unit Tests for Request Mapping and Alias Resolution (M, Medium)

- `test_inproc_alias_to_variant` — validates that `phi-4` resolves to an SDK variant id.
- `test_inproc_device_type_mapping` — validates string-to-`DeviceType` conversion.
- `test_inproc_request_to_sdk_builder` — validates temperature, max_tokens, model id mapping.
- Use SDK mocks or `cfg(test)` stubs where the SDK does not permit headless access.

### T-013 — Write Unit Tests for Stream Event Translation (M, Medium)

- `test_text_delta_to_stream_event` — checks `TextDelta` → `StreamEvent::TextDelta`
- `test_tool_call_stream_events` — verifies tool-use block splitting into start/delta/end events
- `test_usage_emitted` — confirms `StreamEvent::Usage` is produced when SDK provides usage
- `test_finish_reason_mapping` — maps SDK finish reasons to ragent `FinishReason`

### T-014 — Write Conditional Integration Test for In-Process Chat (S, Low)

- Add `crates/ragent-llm/tests/test_foundry_local_inproc.rs`.
- Use `is_foundry_local_available()` to skip the test when the SDK cannot initialise.
- Verify: provider construction → in-process client creation → simple chat → stream events → finish.

### T-015 — Update `PROVIDERS.md` with In-Process Configuration and Migration Notes (S, Low)

- Add a new "In-process mode" subsection under the Foundry Local provider entry.
- Include the `in_process`, `device`, and `models_path` config snippet.
- Explain that `auto_start` applies only to the web-service path.
- Mention the `RAGENT_FOUNDRY_LOCAL_FORCE_WEB` escape hatch.

### T-016 — Update `SPEC.md` Provider List and Internal-LLM Defaults (S, Low)

- Add the in-process Foundry Local backend to the provider list in `SPEC.md`.
- Update internal-LLM documentation to note that `/internal-llm foundry` defaults to in-process when available.

## Estimated Effort

| Phase | Tasks | Total Effort |
|---|---|---|
| Survey & config | T-001, T-002, T-008 | S + S + S = ~1.5 days |
| In-process client | T-003, T-004, T-005, T-006, T-009 | M + M + M + L + M = ~6 days |
| Provider integration | T-007, T-010, T-011 | M + S + S = ~2.5 days |
| Testing | T-012, T-013, T-014 | M + M + S = ~2.5 days |
| Documentation | T-015, T-016 | S + S = ~1 day |
| **Total** | | **~13.5 days** |

## Risks

- The SDK's in-process chat API may differ significantly from the web-service OpenAI-compatible API, requiring a larger translation layer than estimated.
- Native core library loading may fail on some platforms (e.g. headless Linux, WSL, older Windows builds), limiting the in-process path and increasing the importance of the web-service fallback.
- Model handle lifecycle (when to unload, concurrent model usage) is not yet fully understood and may require additional design work.