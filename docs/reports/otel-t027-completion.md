# T-027 Completion Report — Snapshot restore counter instrumentation

## Spec / Plan references

- **Spec ID:** `otel`
- **Task:** T-027 — Optional: instrument snapshot restore counter
- **Requirement:** FR-029
- **Dependencies:** T-006 (Instrument registry)
- **Status:** `completed` (see `specs/otel/PLAN.md` line 82)

## Requirement text

> **FR-029** The system may expose a counter metric for snapshot/undo restores.

## What was implemented

### 1. Metric catalog entry (already present)

`crates/ragent-telemetry/src/instruments.rs` already defined:

- `pub const SNAPSHOT_RESTORES: &str = "ragent.snapshot.restores";`
- `pub snapshot_restores: opentelemetry::metrics::Counter<u64>` in `InstrumentRegistry`
- Counter registration in `InstrumentRegistry::from_meter` with unit `{restore}` and description "Snapshot undo system restores".

### 2. Snapshot recorder

`crates/ragent-telemetry/src/recorder.rs`:

- Added `SnapshotRecorder` with the same feature-gated pattern as the other recorders:
  - Real implementation under `#[cfg(feature = "telemetry")]` holding `Option<InstrumentRegistry>`.
  - Zero-sized no-op implementation under `#[cfg(not(feature = "telemetry"))]`.
  - `from_subsystem`, `disabled`, `is_enabled`, and `record_restore` methods.
- `record_restore()` increments `ragent.snapshot.restores` by `1`.
  - No attributes are attached, so file paths and snapshot contents can never leak into exported metrics (FR-034).
  - Short-circuits when the metric is disabled via `telemetry.otel.metrics` (FR-027).
- Updated the module-level docblock to list `SnapshotRecorder` and FR-029 in the metrics table.

### 3. Tests

Five new tests added to `crates/ragent-telemetry/src/recorder.rs`:

| Test | Feature | What it verifies |
|------|---------|------------------|
| `test_disabled_snapshot_recorder_is_noop` | telemetry | `disabled()` recorder does nothing and reports `is_enabled() == false` |
| `test_snapshot_recorder_record_restore_increments_counter` | telemetry | Three `record_restore()` calls produce a sum of `3` for `ragent.snapshot.restores` |
| `test_snapshot_recorder_respects_metric_toggle` | telemetry | Setting `telemetry.otel.metrics["ragent.snapshot.restores"] = false` suppresses the counter |
| `test_noop_snapshot_recorder` | no telemetry | Feature-off `SnapshotRecorder::disabled()` is a no-op |

## Verification

```text
$ cargo check -p ragent-telemetry --features telemetry
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.74s

$ cargo check -p ragent-telemetry
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.37s

$ cargo test -p ragent-telemetry --features telemetry --lib --tests
running 115 tests ... ok
running 10 tests ... ok
running 18 tests ... ok
running 16 tests ... ok
running 10 tests ... ok
running 20 tests ... ok
running 19 tests ... ok
running 6 tests ... ok
running 5 tests ... ok
running 19 tests ... ok
```

## Files touched

| File | Change |
|------|--------|
| `crates/ragent-telemetry/src/recorder.rs` | Added `SnapshotRecorder` (real + no-op), updated module docblock, added 5 tests. |
| `specs/otel/PLAN.md` | T-027 status flipped from `pending` to `completed`. |
| `CHANGELOG.md` | Added entry under `## Unreleased`. |
| `docs/reports/otel-t027-completion.md` | This report. |

## Definition of Done checklist (from PLAN.md)

- [x] `cargo check -p ragent-telemetry` and `cargo test -p ragent-telemetry` pass with the `telemetry` feature enabled.
- [x] `cargo check` (default features, no `telemetry`) still passes and produces zero OTEL-related errors.
- [x] The `ragent.snapshot.restores` counter is registered and recordable via `SnapshotRecorder::record_restore()`.
- [x] The counter has no attributes, satisfying the sensitive-data guard (FR-034).
- [x] Per-metric toggle short-circuits the counter when disabled (FR-027).

---
*Final verification written after task completion.*
