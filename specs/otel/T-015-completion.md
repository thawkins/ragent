# T-015 Completion: Instrument coordinator: active/completed agents, errors, timeouts

**Spec ID:** otel
**Task:** T-015
**Requirements:** FR-012, FR-018
**Status:** completed
**Dependencies:** T-006
**Effort:** M
**Priority:** High

## Summary

Instrumented the coordinator (`Coordinator` struct) so that sub-agent
lifecycle metrics and error/timeout metrics are recorded as OTEL instruments
when telemetry is enabled. All three job-start methods (`start_job_sync`,
`start_job_first_success`, `start_job_async`) are instrumented.

## Requirements

### FR-012

> The system shall record `ragent.errors.total` and `ragent.timeouts.total`
> from the `Coordinator::Metrics` counters, tagged with `component` where
> identifiable.

### FR-018

> When a sub-agent is spawned, the system shall record
> `ragent.subagent.spawns` and increment `ragent.agents.active`; when it
> completes, the system shall decrement `ragent.agents.active` and increment
> `ragent.agents.completed`.

## Changes

### `crates/ragent-telemetry/src/recorder.rs`

Created a new `CoordinatorRecorder` type alongside the existing recorders:

- **Feature-on variant** (`#[cfg(feature = "telemetry")]`,
  `#[derive(Clone)]`): Holds `Option<InstrumentRegistry>`.

  - `from_subsystem(&TelemetrySubsystem) -> Self`
  - `disabled() -> Self`
  - `is_enabled() -> bool`
  - `record_agent_spawn()` — increments `ragent.subagent.spawns` counter and
    `ragent.agents.active` up/down counter by 1 (FR-018).
  - `record_agent_complete()` — decrements `ragent.agents.active` by 1 and
    increments `ragent.agents.completed` by 1 (FR-018).
  - `record_error(component: &str)` — increments `ragent.errors.total`
    tagged with the `component` attribute (FR-012).
  - `record_timeout()` — increments `ragent.timeouts.total` (FR-012).

- **Feature-off variant** (`#[cfg(not(feature = "telemetry"))]`,
  `#[derive(Clone, Copy)]`): Zero-sized no-op.

- **`impl Default for CoordinatorRecorder`** — returns `disabled()`.

- **Unit tests added** (feature-on test module):
  - `test_disabled_coordinator_recorder_is_noop`
  - `test_coordinator_recorder_record_agent_spawn` — verifies
    `ragent.subagent.spawns` = 2 and `ragent.agents.active` = 2
  - `test_coordinator_recorder_record_agent_complete` — verifies
    `ragent.agents.active` = 1 and `ragent.agents.completed` = 1 after
    2 spawns + 1 complete
  - `test_coordinator_recorder_record_error` — verifies
    `ragent.errors.total` = 2 and the `component=coordinator` attribute
  - `test_coordinator_recorder_record_timeout` — verifies
    `ragent.timeouts.total` = 2

- **No-op test added** (feature-off test module):
  - `test_noop_coordinator_recorder`

### `crates/ragent-agent/src/orchestrator/coordinator.rs`

- **New struct field**: `pub coordinator_recorder:
  ragent_telemetry::recorder::CoordinatorRecorder` added to `Coordinator`,
  with a docblock. Defaulting to `disabled()` in all three constructors
  (`new`, `with_router`, `with_request_timeout`).

- **Instrumentation in `start_job_sync`**:
  - `record_agent_spawn()` after `active_jobs.fetch_add(1)`
  - `record_error("coordinator")` when no agents match
  - `record_timeout()` on timeout errors
  - `record_agent_complete()` after `completed_jobs.fetch_add(1)`
  - `record_error("coordinator")` when no responses or policy resolve fails

- **Instrumentation in `start_job_first_success`**:
  - `record_agent_spawn()` after `active_jobs.fetch_add(1)`
  - `record_error("coordinator")` when no agents match
  - `record_agent_complete()` on successful first response
  - `record_timeout()` on timeout errors
  - `record_error("coordinator")` on non-timeout errors
  - `record_agent_complete()` when no agent succeeded

- **Instrumentation in `start_job_async`**:
  - Recorder is cloned into the `tokio::spawn` closure
  - `record_agent_spawn()` before the spawn (on the `self` reference)
  - Inside the spawn: `record_error("coordinator")` and
    `record_agent_complete()` on no-match failure
  - Inside the spawn: `record_timeout()` or `record_error("coordinator")`
    on per-agent send errors
  - Inside the spawn: `record_agent_complete()` on job completion

## Verification

### Build checks

```
cargo check --workspace                          # pass (default features)
cargo check -p ragent-agent --features telemetry # pass
cargo check -p ragent-telemetry --features telemetry # pass
```

### Test results

```
cargo test -p ragent-telemetry --features telemetry --lib recorder
test result: ok. 24 passed; 0 failed; 0 ignored; 0 measured; 29 filtered out

cargo test -p ragent-telemetry --lib
test result: ok. 18 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

cargo test -p ragent-agent --lib
test result: ok. 316 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

cargo test -p ragent-tui --lib
test result: ok. 63 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
```

### New tests for T-015

| Test | Module | Verifies |
|------|--------|----------|
| `test_disabled_coordinator_recorder_is_noop` | recorder (telemetry) | No-op path doesn't panic |
| `test_coordinator_recorder_record_agent_spawn` | recorder (telemetry) | `subagent.spawns` and `agents.active` increment |
| `test_coordinator_recorder_record_agent_complete` | recorder (telemetry) | `agents.active` decrements, `agents.completed` increments |
| `test_coordinator_recorder_record_error` | recorder (telemetry) | `errors.total` increments with `component` attribute |
| `test_coordinator_recorder_record_timeout` | recorder (telemetry) | `timeouts.total` increments |
| `test_noop_coordinator_recorder` | recorder (no-telemetry) | Feature-off no-op works |

## Spec / Plan updates

- `specs/otel/PLAN.md` T-015 row: status changed from `pending` to `completed`.
- Spec task `T-015` status updated to `completed` via `spec_task_update`.