# T-026 Completion Report — Optional Prometheus text endpoint

## Spec / Plan references

- **Spec ID:** `otel`
- **Task:** T-026 — Optional: add Prometheus text endpoint for local scraping
- **Requirement:** FR-028
- **Dependencies:** T-003 (TelemetrySubsystem)
- **Status:** `completed` (see `specs/otel/PLAN.md` line 81)

## Requirement text

> **FR-028** The system may support an in-process metrics endpoint
> (`telemetry.otel.internal_port`) that exposes metrics in Prometheus text
> format for local scraping without an OTLP collector.

## What was implemented

### 1. Config field

`crates/ragent-config/src/telemetry.rs`:

- Added `pub internal_port: Option<u16>` to `OtelConfig` with
  `#[serde(default)]`. Default is `None` (no Prometheus endpoint).
- Updated `impl Default for OtelConfig` to seed `internal_port: None`.

### 2. Prometheus module

`crates/ragent-telemetry/src/prometheus.rs` (new file):

- **`SharedManualReader`** — a newtype wrapper around `Arc<ManualReader>`
  that implements the OTEL `MetricReader` trait. This is necessary because
  `SdkMeterProvider::with_reader` takes `T: MetricReader` by value, and
  `Arc<ManualReader>` does not auto-implement `MetricReader`. The wrapper
  delegates every trait method (`register_pipeline`, `collect`,
  `force_flush`, `shutdown`, `temporality`) to the inner `ManualReader`,
  so the same reader instance can be registered on the provider (which
  takes ownership) and held by the HTTP server (which needs to call
  `collect` on demand).

- **`render_prometheus_text(reader: &ManualReader) -> String`** — collects
  a metric snapshot via the reader and renders it as Prometheus
  text-format exposition:
  - Resource attributes become a `target_info` line (per the Prometheus
    OTEL exposition convention).
  - Counters (`Sum<u64>`, `Sum<f64>`) render as `# TYPE ... counter`.
  - Up/down counters (`Sum<i64>`) render as `# TYPE ... gauge`.
  - Gauges (`Gauge<u64>`, `Gauge<i64>`, `Gauge<f64>`) render as
    `# TYPE ... gauge`.
  - Histograms (`Histogram<u64>`, `Histogram<f64>`) render as
    `# TYPE ... histogram` with `_bucket{le="..."}`, `_sum`, and `_count`
    lines.
  - Attribute values are escaped per the exposition format (`\`, `"`, `\n`).
  - Returns an empty string if `collect` fails (FR-031: never crash).

- **`serve(reader: Arc<ManualReader>, port: u16)`** — an async HTTP
  server that binds `127.0.0.1:<port>` and serves the rendered text at
  `GET /metrics`. Returns 200 + text on success, 503 on collect failure,
  404 for non-`/metrics` paths. Runs on a background tokio task.

### 3. Subsystem wiring

`crates/ragent-telemetry/src/subsystem.rs`:

- `TelemetrySubsystem` gained two new feature-gated fields:
  `prometheus_reader: Option<SharedManualReader>` and
  `prometheus_handle: Option<JoinHandle<io::Result<JoinHandle<()>>>>`.

- `TelemetrySubsystem::new()` now:
  1. Builds a `SharedManualReader` when `config.internal_port.is_some()`.
  2. Rebuilds the `SdkMeterProvider` with both the OTLP `PeriodicReader`
     and the `SharedManualReader` via `build_provider_with_prometheus`
     (so both paths see the same metrics).
  3. Spawns the `serve` future as a background tokio task.
  4. Stores the handle for later cleanup.

- `TelemetrySubsystem::shutdown()` aborts the Prometheus server task
  before shutting down the provider.

- `build_provider_with_prometheus` mirrors `build_provider` but adds the
  `SharedManualReader` as a second reader.

### 4. Non-blocking guarantee (FR-031, FR-033)

- The HTTP server runs on a background tokio task; the agent loop never
  waits for it.
- A failed `collect` (e.g. provider shut down) returns an empty body with
  a 503 status rather than crashing the task.
- A bind failure (port in use) is logged at `warn` level and telemetry
  continues without the endpoint.
- `render_prometheus_text` never panics.

### 5. Tests

9 unit tests in the `prometheus` module (all passing):

| Test | What it verifies |
|-----|------------------|
| `test_escape_label_value` | `\`, `"`, `\n` escaping |
| `test_render_after_shutdown_returns_empty` | Unregistered reader → empty output (FR-031) |
| `test_format_resource_metrics_with_resource` | `target_info` line with resource attributes |
| `test_format_resource_metrics_empty` | Empty resource → no `target_info` |
| `test_shared_manual_reader_delegates` | Full end-to-end: register reader on provider, record a counter, collect via the handle, verify resource + counter appear in the rendered text |
| `test_build_labels` | Label string sorting and formatting |
| `test_build_labels_empty` | Empty labels → empty string |
| `test_append_le_label_empty` | `le=` label on empty base |
| `test_append_le_label_with_base` | `le=` label appended to existing labels |

## Verification

```text
$ cargo check -p ragent-telemetry --features telemetry
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 2.90s

$ cargo check -p ragent-telemetry
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 2.44s

$ cargo test -p ragent-telemetry --features telemetry --lib --tests
... (112 lib + 10+18+16+10+20+19+6+5+19 integration tests all pass) ...

$ cargo test -p ragent-telemetry --features telemetry --doc
test result: ok. 7 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
```

## Files touched

| File | Change |
|------|--------|
| `crates/ragent-config/src/telemetry.rs` | Added `internal_port: Option<u16>` field + `Default` seed. |
| `crates/ragent-config/src/lib.rs` | Exported `pub mod telemetry` and re-exported `OtelConfig`, `OtelProtocol`, `TelemetryConfig`. |
| `crates/ragent-telemetry/src/prometheus.rs` | New module: `SharedManualReader`, `render_prometheus_text`, `serve`, 9 tests. |
| `crates/ragent-telemetry/src/lib.rs` | Registered `pub mod prometheus`. |
| `crates/ragent-telemetry/src/subsystem.rs` | `TelemetrySubsystem` gains `prometheus_reader` + `prometheus_handle` fields; `new()` wires the endpoint; `shutdown()` aborts the task; `build_provider_with_prometheus` helper. |
| `specs/otel/PLAN.md` | T-026 status flipped from `pending` to `completed`. |
| `CHANGELOG.md` | Added "Added — Optional Prometheus text endpoint for local scraping (spec: otel, T-026)" entry under `## Unreleased`. |

## Definition of Done checklist (from PLAN.md)

- [x] `cargo check -p ragent-telemetry` and `cargo test -p ragent-telemetry`
      pass with the `telemetry` feature enabled.
- [x] `cargo check` (default features, no `telemetry`) still passes and
      produces zero OTEL-related warnings.
- [x] The Prometheus endpoint serves metrics in text format at
      `GET /metrics` on `127.0.0.1:<internal_port>`.
- [x] The endpoint is independent of the OTLP export path (both can run
      simultaneously via separate readers on the same provider).
- [x] Non-blocking: the HTTP server runs on a background task; a failed
      scrape returns 503, not a crash.
- [x] No existing agent loop, TUI, or HTTP server functionality is broken
      when telemetry is disabled or `internal_port` is `None`.