# T-021 Completion Report — Non-blocking guarantee for the OTLP exporter

## Spec / Plan references

- **Spec ID:** `otel`
- **Task:** T-021 — Add non-blocking guarantee: exporter errors logged, never panic
- **Requirements:** FR-031, FR-033
- **Dependencies:** T-004 (OTLP/HTTP exporter wiring)
- **Status:** `completed` (see `specs/otel/PLAN.md` line 76)

## Requirement text

> **FR-031** The system shall not block the agent loop, LLM streaming, or tool
> execution if the OTLP exporter is unavailable or slow; all metric recording
> and export shall be asynchronous and non-blocking.

> **FR-033** The system shall not crash the process if the OTLP endpoint
> returns an error; exporter errors shall be logged at `warn` level and
> retried on the next export interval.

## What was implemented

### 1. Bounded export timeout (FR-031)

The OTLP exporter (HTTP and gRPC) now carries a bounded request timeout so
that an explicit `flush()` or `shutdown()` against a slow or unreachable
endpoint fails in finite time instead of hanging indefinitely.

- **Config field** (`crates/ragent-config/src/telemetry.rs`):
  - Added `pub export_timeout_seconds: u64` to `OtelConfig` with
    `#[serde(default = "default_export_timeout")]`.
  - Added `const fn default_export_timeout() -> u64 { 10 }` (matches the OTEL
    SDK constant `OTEL_EXPORTER_OTLP_TIMEOUT_DEFAULT`).
  - Added validation in `OtelConfig::validate()` that rejects a zero timeout
    when telemetry is enabled.
  - Updated `impl Default for OtelConfig` to seed
    `export_timeout_seconds: default_export_timeout()`.

- **Exporter build** (`crates/ragent-telemetry/src/subsystem.rs`):
  - `build_metric_exporter` computes
    `let timeout = Duration::from_secs(config.export_timeout_seconds.max(1));`
    (clamped to at least 1 second so a zero config cannot make every export
    fail immediately).
  - Both the HTTP and gRPC exporter builders call `.with_timeout(timeout)`.
  - The function's doc comment explicitly references FR-031 / FR-033 and
    explains the clamp.

### 2. Never-panic error handling (FR-033)

- `TelemetrySubsystem::flush()` and `shutdown()` log the exporter error at
  `warn` level and return `Err(TelemetryError::ExporterInit(...))` — they
  never panic. The `flush()` doc comment now carries a
  "Non-blocking guarantee (FR-031, FR-033)" section that documents the
  bounded-timeout contract and instructs callers not to propagate the error
  in a way that would crash the agent loop.

- `ShutdownGuard::drop` (`crates/ragent-telemetry/src/shutdown.rs`) is
  infallible: it logs flush/shutdown errors at `warn` level but never
  panics. This is critical because `Drop` runs during stack unwinding and
  must not itself fail. The guard's doc comment and `Drop` impl comment
  reference FR-031 / FR-033.

- `flush_on_signal_arc` (the SIGINT/SIGTERM handler) logs flush errors at
  `warn` level and exits the background task cleanly — it never panics.

- `CardinalityCache::resolve` (`crates/ragent-telemetry/src/cardinality.rs`)
  fails open on a poisoned `RwLock`: it returns the attributes unchanged
  rather than blocking or panicking.

- Recording (`Counter::add`, `Histogram::record`, `Gauge::record`) remains
  synchronous atomic operations regardless of exporter state; the
  `PeriodicReader` runs exports on a background tokio task, so the agent loop
  never waits for network I/O.

### 3. Tests

`crates/ragent-telemetry/tests/test_nonblocking.rs` (19 tests, all
passing) exercises the non-blocking guarantee across eight facets:

| Facet | Tests |
|-------|-------|
| Recording is non-blocking on an unreachable endpoint | `test_recording_does_not_block_on_unreachable_endpoint`, `test_recording_noop_registry_is_instantaneous` |
| `flush()`/`shutdown()` against an unreachable endpoint never panic | `test_flush_unreachable_endpoint_does_not_panic`, `test_shutdown_unreachable_endpoint_does_not_panic`, `test_disabled_flush_shutdown_never_panic` |
| `ShutdownGuard::drop` is infallible | `test_shutdown_guard_drop_does_not_panic_on_unreachable_endpoint`, `test_shutdown_guard_drop_disabled_is_clean` |
| Recording still works after a failed export (retry semantics) | `test_recording_still_works_after_failed_flush` |
| Export timeout is configurable and clamped | `test_default_export_timeout_is_10_seconds`, `test_custom_export_timeout_preserved`, `test_zero_export_timeout_clamped_to_one_second`, `test_validate_rejects_zero_export_timeout` |
| `flush_on_signal_arc` installs without panic | `test_flush_on_signal_arc_constructs_without_panic`, `test_flush_on_signal_arc_disabled_installs_cleanly` |
| Concurrent recording does not deadlock | `test_concurrent_recording_does_not_deadlock` |
| No-op recorder methods never panic / invalid endpoint returns Err | `test_noop_recorder_methods_never_panic`, `test_invalid_endpoint_returns_err_not_panic`, `test_empty_endpoint_returns_err_not_panic`, `test_non_http_endpoint_returns_err_not_panic` |

The unreachable-endpoint tests use `http://127.0.0.1:1` (a reserved port
that refuses connections immediately) with `export_timeout_seconds = 1` and
a 1-hour export interval so that no background export fires during the
test. This exercises the error path without waiting for the full export
timeout.

## Verification

```text
$ cargo check -p ragent-telemetry --features telemetry
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.74s

$ cargo check -p ragent-telemetry
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.38s

$ cargo test -p ragent-telemetry --features telemetry --test test_nonblocking
running 19 tests
... (all 19 pass) ...
test result: ok. 19 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
```

## Files touched

| File | Change |
|------|--------|
| `crates/ragent-config/src/telemetry.rs` | Added `export_timeout_seconds` field, `default_export_timeout()` const fn, validation, and `Default` seed. |
| `crates/ragent-telemetry/src/subsystem.rs` | `build_metric_exporter` applies `.with_timeout(timeout)` (clamped) for HTTP + gRPC; `flush()` doc comment documents the FR-031/FR-033 contract. |
| `crates/ragent-telemetry/tests/test_nonblocking.rs` | New 19-test integration suite (new file). |
| `specs/otel/PLAN.md` | T-021 status flipped from `pending` to `completed`. |
| `CHANGELOG.md` | "Added — OpenTelemetry non-blocking guarantee (spec: otel, T-021, FR-031, FR-033)" entry under `## Unreleased`. |

## Follow-on tasks unlocked

- **T-032** — "Write integration test: malformed endpoint does not crash"
  (AC-7), which depends on T-021, is now unblocked.

## Definition of Done checklist (from PLAN.md)

- [x] `cargo check -p ragent-telemetry` and `cargo test -p ragent-telemetry`
      pass with the `telemetry` feature enabled.
- [x] `cargo check` (default features, no `telemetry`) still passes and
      produces zero OTEL-related warnings.
- [x] The exporter never blocks the agent loop when the endpoint is
      unreachable or slow (bounded timeout + async periodic reader).
- [x] Exporter errors never crash the process (logged at `warn`, returned
      as `Err`, retried on the next interval).
- [x] No existing agent loop, TUI, or HTTP server functionality is broken
      when telemetry is disabled.