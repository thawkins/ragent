# T-028 Completion Report — Instrument registry unit tests with in-memory exporter

## Spec / Plan references

- **Spec ID:** `otel`
- **Task:** T-028 — Write unit tests with in-memory exporter for instrument registry
- **Requirement:** NFR-005
- **Dependencies:** T-006 (Build instrument registry for all metrics in the catalog)
- **Status:** `completed` (see `specs/otel/PLAN.md` line 83)

## Requirement text

> **NFR-005** The telemetry subsystem must be covered by automated unit tests.

## What was implemented

### 1. In-memory exporter test helper

`crates/ragent-telemetry/src/instruments.rs`:

- Added `build_registry_with_exporter()` helper that returns:
  - an [`InstrumentRegistry`] built from a fresh [`SdkMeterProvider`],
  - the [`InMemoryMetricExporter`] wired to that provider,
  - the provider itself (so tests can call `force_flush()`),
  - and a tokio runtime.
- The helper uses a `PeriodicReader` with a 1-hour interval so tests control
  flushing explicitly.

### 2. New in-memory exporter tests

| Test | What it verifies |
|------|------------------|
| `test_in_memory_counter_export` | Records to `llm_requests` with `model` + `provider` attributes, flushes, and asserts the exported `Sum<u64>` sums to `1` |
| `test_in_memory_histogram_export` | Records to `llm_duration` and asserts a `Histogram<f64>` with the metric name appears after flush |
| `test_in_memory_gauge_export` | Records `5` to `team_members` and asserts the exported `Gauge<i64>` sums to `5` |
| `test_in_memory_up_down_counter_export` | Performs `+1, +1, -1` on `sessions_active` and asserts the exported `Sum<i64>` net is `1` |
| `test_in_memory_metric_toggles_disable_export` | Builds a registry with `telemetry.otel.metrics["ragent.llm.requests"] = false` and asserts `is_metric_enabled()` returns the correct enabled/disabled state |

These tests complement the existing smoke tests (`test_counter_can_add`,
`test_histogram_can_record`, etc.) by actually validating exported metric
snapshots rather than only checking that instruments can be called without
panicking.

### 3. Existing tests preserved

The original `InstrumentRegistry` tests remain in place:

- `test_registry_constructs_all_instruments`
- `test_counter_can_add`
- `test_histogram_can_record`
- `test_gauge_can_record`
- `test_up_down_counter_can_add`
- `test_attr_helpers`
- `test_noop_registry_default` (no-telemetry feature)

## Verification

```text
$ cargo test -p ragent-telemetry --features telemetry --lib instruments::tests
running 11 tests
test instruments::tests::test_attr_helpers ... ok
test instruments::tests::test_in_memory_gauge_export ... ok
test instruments::tests::test_in_memory_histogram_export ... ok
test instruments::tests::test_in_memory_metric_toggles_disable_export ... ok
test instruments::tests::test_histogram_can_record ... ok
test instruments::tests::test_gauge_can_record ... ok
test instruments::tests::test_in_memory_counter_export ... ok
test instruments::tests::test_in_memory_up_down_counter_export ... ok
test instruments::tests::test_counter_can_add ... ok
test instruments::tests::test_up_down_counter_can_add ... ok
test instruments::tests::test_registry_constructs_all_instruments ... ok

test result: ok. 11 passed; 0 failed; 0 ignored; 0 measured; 109 filtered out

$ cargo test -p ragent-telemetry --features telemetry --lib --tests
test result: ok. 120 passed ...
test result: ok. 10 passed ...
test result: ok. 18 passed ...
test result: ok. 16 passed ...
test result: ok. 10 passed ...
test result: ok. 20 passed ...
test result: ok. 19 passed ...
test result: ok. 6 passed ...
test result: ok. 5 passed ...
test result: ok. 19 passed ...

$ cargo test -p ragent-telemetry --features telemetry --doc
test result: ok. 7 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

$ cargo check -p ragent-telemetry
warning: `ragent-telemetry` (lib) generated 34 warnings
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.60s
```

## Files touched

| File | Change |
|------|--------|
| `crates/ragent-telemetry/src/instruments.rs` | Added `build_registry_with_exporter()` helper, 5 in-memory exporter tests, and preserved existing smoke tests. |
| `specs/otel/PLAN.md` | T-028 status flipped from `pending` to `completed`. |
| `CHANGELOG.md` | Added "Added — Instrument registry in-memory exporter unit tests (spec: otel, T-028)" entry under `## Unreleased`. |
| `docs/reports/otel-t028-completion.md` | This report. |

## Definition of Done checklist (from PLAN.md)

- [x] `cargo check -p ragent-telemetry` and `cargo test -p ragent-telemetry` pass with the `telemetry` feature enabled.
- [x] `cargo check` (default features, no `telemetry`) still passes and produces zero OTEL-related errors.
- [x] Unit tests for the instrument registry use the in-memory exporter and assert exported metric data.
- [x] Tests cover counters, histograms, gauges, and up/down counters.
- [x] Per-metric toggle state is verified via `is_metric_enabled()`.
