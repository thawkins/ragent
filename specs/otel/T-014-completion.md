# T-014 Completion: Instrument session processor: active/total sessions, loop duration, iterations

**Spec ID:** otel
**Task:** T-014
**Requirements:** FR-011, FR-010
**Status:** completed
**Dependencies:** T-006
**Effort:** M
**Priority:** High

## Summary

Instrumented the session processor so that session lifecycle metrics and
agent-loop performance metrics are recorded as OTEL instruments when
telemetry is enabled.

## Requirements

### FR-011

> The system shall record `ragent.sessions.active` as an `UpDownCounter`
> incremented on session start and decremented on session end.

### FR-010

> The system shall record `ragent.agent_loop.duration` and
> `ragent.agent_loop.iterations` from the `AgentLoopProfiler` data for each
> completed session.

## Changes

### `crates/ragent-telemetry/src/recorder.rs`

Created a new `SessionRecorder` type alongside the existing `LlmRecorder`
and `ToolRecorder`:

- **Feature-on variant** (`#[cfg(feature = "telemetry")]`,
  `#[derive(Clone)]`): Holds `Option<InstrumentRegistry>` — `Some` when the
  subsystem is enabled, `None` when disabled.

  - `from_subsystem(&TelemetrySubsystem) -> Self` — builds a live recorder
    from the subsystem's instrument registry.
  - `disabled() -> Self` — holds `None`; all methods are no-ops.
  - `is_enabled() -> bool`
  - `record_session_start()` — increments the `ragent.sessions.active`
    up/down counter by 1 and the `ragent.sessions.total` counter by 1
    (FR-011).
  - `record_session_end()` — decrements the `ragent.sessions.active`
    up/down counter by 1 (FR-011).
  - `record_agent_loop(duration_ms: f64, iterations: u64)` — records to the
    `ragent.agent_loop.duration` histogram and the
    `ragent.agent_loop.iterations` histogram (FR-010).

- **Feature-off variant** (`#[cfg(not(feature = "telemetry"))]`,
  `#[derive(Clone, Copy)]`): Zero-sized no-op. All methods discard their
  arguments. This ensures zero overhead when the `telemetry` Cargo feature
  is off.

- **`impl Default for SessionRecorder`** — returns `disabled()`.

- **Module doc comment** updated to include the metrics table mapping
  `record_session_start` → FR-011, `record_session_end` → FR-011, and
  `record_agent_loop` → FR-010.

- **Unit tests added** (feature-on test module):
  - `test_disabled_session_recorder_is_noop` — confirms no-op behavior and
    no panic.
  - `test_session_recorder_record_session_start_increments_counters` —
    calls `record_session_start` twice, flushes the in-memory exporter,
    asserts `ragent.sessions.active` sum is 2 and `ragent.sessions.total`
    sum is 2.
  - `test_session_recorder_record_session_end_decrements_active` — calls
    `record_session_start` twice then `record_session_end` once, asserts
    `ragent.sessions.active` sum is 1.
  - `test_session_recorder_record_agent_loop_records_histograms` — records
    one `record_agent_loop(12345.6, 5)` call, asserts both
    `ragent.agent_loop.duration` and `ragent.agent_loop.iterations` appear
    in exported metrics.

- **No-op test added** (feature-off test module):
  - `test_noop_session_recorder` — calls all no-op methods without panic.

### `crates/ragent-agent/src/session/processor.rs`

- **New struct field**: `pub session_recorder:
  ragent_telemetry::recorder::SessionRecorder` added to `SessionProcessor`,
  with a docblock describing it as a zero-overhead no-op when telemetry is
  disabled.

- **Instrumentation in `process_user_message`**:

  - At the top of the method, after the session ID is set up and before
    storing the user message:
    ```rust
    // FR-011: increment `ragent.sessions.active` and
    // `ragent.sessions.total` on session start.
    self.session_recorder.record_session_start();
    ```

  - At the end of the agent loop, after `total_elapsed_ms` is computed and
    before the `MessageEnd` event is published:
    ```rust
    // FR-010: record agent-loop duration and iteration count.
    self.session_recorder.record_agent_loop(
        total_start.elapsed().as_secs_f64() * 1000.0,
        self.event_bus.current_step(session_id) as u64,
    );
    ```

  - Before returning `Ok(saved_msg)`, after the `OnSessionEnd` hooks fire:
    ```rust
    // FR-011: decrement `ragent.sessions.active` on session end.
    self.session_recorder.record_session_end();
    ```

### `src/main.rs`

- Added `session_recorder:
  ragent_telemetry::recorder::SessionRecorder::disabled()` to the
  `SessionProcessor` struct literal in `main()`. (Wired to a live subsystem
  in a future task when the telemetry subsystem is initialized at startup;
  for now it defaults to disabled.)

### `crates/ragent-tui/src/app/tests.rs`

- Added `session_recorder:
  ragent_telemetry::recorder::SessionRecorder::disabled()` to the test
  `SessionProcessor` struct literal.
- Also fixed a pre-existing corruption in the `read_timestamps` block that
  was exposed by the struct literal change.

### `crates/ragent-tui/Cargo.toml`

- Added `ragent-telemetry = { path = "../ragent-telemetry" }` dependency
  (needed for the `SessionRecorder::disabled()` call in tests).
- Added `async-trait = { workspace = true }` dependency (was missing,
  causing a pre-existing test compilation failure that surfaced when the
  test crate was recompiled).

## Verification

### Build checks

```
cargo check --workspace                          # pass (default features, no telemetry)
cargo check --workspace --features telemetry     # pass
cargo check -p ragent-telemetry --features telemetry # pass
```

### Test results

```
cargo test -p ragent-telemetry --features telemetry --lib recorder
test result: ok. 19 passed; 0 failed; 0 ignored; 0 measured; 29 filtered out

cargo test -p ragent-telemetry --lib
test result: ok. 17 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

cargo test -p ragent-agent --lib
test result: ok. 316 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

cargo test -p ragent-tui --lib
test result: ok. 63 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
```

### New tests for T-014

| Test | Module | Verifies |
|------|--------|----------|
| `test_disabled_session_recorder_is_noop` | recorder (telemetry) | No-op path doesn't panic |
| `test_session_recorder_record_session_start_increments_counters` | recorder (telemetry) | `sessions.active` and `sessions.total` increment correctly |
| `test_session_recorder_record_session_end_decrements_active` | recorder (telemetry) | `sessions.active` decrements on end |
| `test_session_recorder_record_agent_loop_records_histograms` | recorder (telemetry) | Both agent_loop histograms receive data |
| `test_noop_session_recorder` | recorder (no-telemetry) | Feature-off no-op works |

## Spec / Plan updates

- `specs/otel/PLAN.md` T-014 row: status changed from `pending` to `completed`.
- Spec task `T-014` status updated to `completed` via `spec_task_update`.