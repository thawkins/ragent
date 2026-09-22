# T-018 Completion: Instrument sub-agent/task subsystem: spawns and completions

**Spec ID:** otel
**Task:** T-018
**Requirement:** FR-018
**Status:** completed
**Dependencies:** T-015
**Effort:** S
**Priority:** Medium

## Summary

Instrumented the `TaskManager` so that sub-agent lifecycle metrics are recorded
as OTEL instruments when telemetry is enabled. Both `spawn_sync` (blocking) and
`spawn_background` (async) paths are instrumented.

The `CoordinatorRecorder` type already existed from T-015 and already provided
`record_agent_spawn()` and `record_agent_complete()` methods that increment the
correct OTEL instruments (`ragent.subagent.spawns`, `ragent.agents.active` up/down,
`ragent.agents.completed`). T-018 wires this recorder into `TaskManager`.

## Requirement

### FR-018

> When a sub-agent is spawned, the system shall record
> `ragent.subagent.spawns` and increment `ragent.agents.active`; when it
> completes, the system shall decrement `ragent.agents.active` and increment
> `ragent.agents.completed`.

## Changes

### `crates/ragent-agent/src/task/mod.rs`

- **New struct field**: `pub coordinator_recorder:
  ragent_telemetry::recorder::CoordinatorRecorder` added to `TaskManager`,
  with a docblock. Defaulted to `disabled()` in the `new()` constructor.

- **New builder method**: `with_coordinator_recorder(recorder)` — sets the
  recorder from a `TelemetrySubsystem`-created `CoordinatorRecorder`. Called
  by the application bootstrap (`src/main.rs`) when telemetry is enabled.

- **`spawn_sync` instrumentation**:
  - `record_agent_spawn()` after the `Event::SubagentStart` publish.
  - `record_agent_complete()` after `run_subagent` returns (before the
    `match result` block), so both success and failure paths record the
    completion.

- **`spawn_background` instrumentation**:
  - `record_agent_spawn()` after the `Event::SubagentStart` publish (on the
    `self` reference, before the `tokio::spawn`).
  - The recorder is cloned into the `tokio::spawn` closure
    (`coordinator_recorder`).
  - `record_agent_complete()` called at all three completion exits inside the
    spawned task:
    1. Agent-resolve failure (early return)
    2. `process_message` success (Ok branch)
    3. `process_message` failure/cancel (Err branch)

## Verification

### Build checks

```
cargo check -p ragent-agent                          # pass (default features)
cargo check -p ragent-agent --features telemetry      # pass
cargo check -p ragent-telemetry --features telemetry  # pass
```

### Test results

```
cargo test -p ragent-agent --lib
test result: ok. 316 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

cargo test -p ragent-telemetry --features telemetry --lib -- --skip test_flush_on_signal
test result: ok. 60 passed; 0 failed; 0 ignored; 0 measured; 2 filtered out
```

The `test_flush_on_signal*` tests are skipped because they spawn real signal
handlers that block indefinitely — this is a pre-existing characteristic, not
related to T-018.

### Existing tests for T-015 (already cover the recorder)

| Test | Module | Verifies |
|------|--------|----------|
| `test_disabled_coordinator_recorder_is_noop` | recorder (telemetry) | No-op path doesn't panic |
| `test_coordinator_recorder_record_agent_spawn` | recorder (telemetry) | `subagent.spawns` and `agents.active` increment |
| `test_coordinator_recorder_record_agent_complete` | recorder (telemetry) | `agents.active` decrements, `agents.completed` increments |
| `test_noop_coordinator_recorder` | recorder (no-telemetry) | Feature-off no-op works |

## Spec / Plan updates

- `specs/otel/PLAN.md` T-018 row: status changed from `pending` to `completed`.
- Spec task `T-018` status updated to `completed` via `spec_task_update`.