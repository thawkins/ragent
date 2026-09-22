# T-013 Completion: Instrument tool execution path: invocations and duration

**Spec ID:** otel
**Task:** T-013
**Requirements:** FR-009, FR-015
**Status:** completed
**Dependencies:** T-006
**Effort:** M
**Priority:** Critical

## Summary

Instrumented the tool execution path so that every tool invocation records the
`ragent.tool.invocations` counter (FR-009) and the `ragent.tool.duration`
histogram (FR-015), both tagged with the `tool.name` attribute.

## Requirements

### FR-009

> The system shall record `ragent.tool.invocations` and `ragent.tool.duration`
> for every tool execution, tagged with `tool.name`.

### FR-015

> When a tool invocation begins, the system shall start a timer and record
> `ragent.tool.invocations`; when it completes, the system shall record
> `ragent.tool.duration`.

## Changes

### `crates/ragent-telemetry/src/instruments.rs`

- Added `#[derive(Clone)]` to the feature-gated `InstrumentRegistry` struct.
  OTEL `Meter` and instrument handles (`Counter`, `Histogram`, `Gauge`, etc.)
  are all `Arc`-backed, so cloning is cheap. This allows `ToolRecorder` to be
  cloned into `tokio::spawn` closures for per-tool-call instrumentation.

### `crates/ragent-telemetry/src/recorder.rs`

Created a new `ToolRecorder` type alongside the existing `LlmRecorder`:

- **Feature-on variant** (`#[cfg(feature = "telemetry")]`, `#[derive(Clone)]`):
  Holds `Option<InstrumentRegistry>` — `Some` when the subsystem is enabled,
  `None` when disabled.

  - `from_subsystem(&TelemetrySubsystem) -> Self` — builds a live recorder from
    the subsystem's instrument registry.
  - `disabled() -> Self` — holds `None`; all methods are no-ops.
  - `is_enabled() -> bool`
  - `record_invocation(tool_name: &str)` — increments the
    `ragent.tool.invocations` counter tagged with the `tool.name` attribute
    (FR-009).
  - `record_duration(tool_name: &str, duration_ms: f64)` — records to the
    `ragent.tool.duration` histogram tagged with `tool.name` (FR-015).

- **Feature-off variant** (`#[cfg(not(feature = "telemetry"))]`,
  `#[derive(Clone, Copy)]`): Zero-sized no-op. All methods discard their
  arguments. This ensures zero overhead when the `telemetry` Cargo feature is
  off.

- **`impl Default for ToolRecorder`** — returns `disabled()`.

- **Module doc comment** updated to include the metrics table mapping
  `record_invocation` → FR-009 and `record_duration` → FR-015.

- **Unit tests added** (feature-on test module):
  - `test_disabled_tool_recorder_is_noop` — confirms no-op behavior and no
    panic.
  - `test_tool_recorder_record_invocation_increments_counter` — records 3
    invocations across 2 tools, flushes the in-memory exporter, asserts the
    `ragent.tool.invocations` counter sum is 3.
  - `test_tool_recorder_record_duration_records_histogram` — records one
    duration and asserts `ragent.tool.duration` appears in exported metrics.
  - `test_tool_recorder_attributes_include_tool_name` — asserts the exported
    data point has a `tool.name` attribute with the correct value.
  - `test_tool_recorder_is_clone` — confirms the recorder can be cloned and
    the clone is enabled.

- **No-op test added** (feature-off test module):
  - `test_noop_tool_recorder` — calls all no-op methods without panic.

### `crates/ragent-agent/src/session/processor.rs`

- **New struct field**: `pub tool_recorder: ragent_telemetry::recorder::ToolRecorder`
  added to `SessionProcessor`, with a docblock describing it as a zero-overhead
  no-op when telemetry is disabled, and noting it is cheap to clone for use in
  `tokio::spawn` closures.

- **Instrumentation in the tool execution spawn closure** (the `fut` created
  per tool call in the tool dispatch phase):

  - The recorder is cloned into the closure:
    `let tool_recorder = self.tool_recorder.clone();`

  - At the start of the async block, immediately after the
    `Event::ToolCallStart` publish:
    ```rust
    // FR-009: record tool invocation counter tagged with tool.name.
    tool_recorder.record_invocation(&tc_clone.name);
    ```

  - After `duration_ms` is computed (`start.elapsed().as_millis() as u64`):
    ```rust
    // FR-015: record tool execution duration tagged with tool.name.
    tool_recorder.record_duration(&tc_clone.name, duration_ms as f64);
    ```

### `src/main.rs`

- Added `tool_recorder: ragent_telemetry::recorder::ToolRecorder::disabled()`
  to the `SessionProcessor` struct literal in `main()`. (Wired to a live
  subsystem in a future task when the telemetry subsystem is initialized at
  startup; for now it defaults to disabled.)

## Verification

### Build checks

```
cargo check --workspace                          # pass (default features, no telemetry)
cargo check -p ragent-agent --features telemetry # pass
cargo check -p ragent-telemetry --features telemetry # pass
```

### Test results

```
cargo test -p ragent-telemetry --features telemetry --lib recorder
test result: ok. 15 passed; 0 failed; 0 ignored; 0 measured; 29 filtered out

cargo test -p ragent-telemetry --lib
test result: ok. 16 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

cargo test -p ragent-agent --lib
test result: ok. 316 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
```

### New tests for T-013

| Test | Module | Verifies |
|------|--------|----------|
| `test_disabled_tool_recorder_is_noop` | recorder (telemetry) | No-op path doesn't panic |
| `test_tool_recorder_record_invocation_increments_counter` | recorder (telemetry) | Counter increments correctly |
| `test_tool_recorder_record_duration_records_histogram` | recorder (telemetry) | Histogram receives data |
| `test_tool_recorder_attributes_include_tool_name` | recorder (telemetry) | `tool.name` attribute is present |
| `test_tool_recorder_is_clone` | recorder (telemetry) | Recorder is cloneable |
| `test_noop_tool_recorder` | recorder (no-telemetry) | Feature-off no-op works |

## Spec / Plan updates

- `specs/otel/PLAN.md` T-013 row: status changed from `pending` to `completed`.
- Spec task `T-013` status updated to `completed` via `spec_task_update`.