# ragent-telemetry

OpenTelemetry instrumentation and OTLP export for ragent. Provides the
telemetry subsystem, instrument registry, recorders, cardinality caps,
sensitive-data redaction, Prometheus exposition, and graceful shutdown.

## Workspace Dependencies

- ragent-types
- ragent-config

## External Dependencies

- serde, serde_json, anyhow, thiserror, tracing, tokio, parking_lot
- opentelemetry, opentelemetry_sdk, opentelemetry-otlp (optional, behind `telemetry` feature)

## Public API (crate root)

### Modules

- **cardinality** — Cardinality cap for metric attributes; collapses excess combinations into an `unknown` bucket.
- **counters** — Lock-free in-memory snapshot of all counter, gauge, and histogram values for live diagnostic display.
- **instruments** — OTEL instrument registry that constructs and holds all metrics from the Metric Catalog.
- **prometheus** — Optional Prometheus text-format exposition endpoint for local scraping (feature-gated).
- **recorder** — High-level convenience recorders wrapping `InstrumentRegistry`.
- **sensitive** — Sensitive-data guard that redacts API keys, file content, and prompts from metric attributes.
- **shutdown** — RAII `ShutdownGuard` and signal-based flush helper for graceful telemetry shutdown.
- **subsystem** — `TelemetrySubsystem`: the single handle owning the meter provider and OTLP exporter lifecycle.

### Re-exported items

- **OtelConfig** (struct) / **OtelProtocol** (enum) / **TelemetryConfig** (struct) — Re-exported from `ragent_config`.
- **LlmRecorder** (struct) — Recorder for LLM provider metrics.
- **SessionRecorder** (struct) — Recorder for session lifecycle metrics.
- **ToolRecorder** (struct) — Recorder for tool execution metrics.
- **TelemetryState** (enum) — `Enabled` or `Disabled`.
- **TelemetrySubsystem** (struct) — Owner of the OTEL meter provider and OTLP exporter lifecycle.
- **InstrumentRegistry** (type alias) — `InstrumentRegistry` when feature on, `NoopInstrumentRegistry` when off.

### Crate-root items

- **Result\<T\>** (type alias) — `std::result::Result<T, TelemetryError>`.
- **TelemetryError** (enum) — `FeatureNotEnabled`, `InvalidEndpoint(String)`, `ExporterInit(String)`.

## Module: cardinality

- **UNKNOWN_BUCKET** (const) — `"unknown"` sentinel.
- **DEFAULT_CARDINALITY_LIMIT** (const) — Default per-metric limit (1000).
- **CardinalityCache** (struct) — Thread-safe cache; methods: `new`, `resolve`, `distinct_count`. No-op stub when feature off.

## Module: counters

- **AtomicF64** (struct) — Lock-free floating-point value.
- **CounterSnapshot** (struct) — Live atomic mirror of all counter/gauge/histogram values.
- **CounterValues** (struct) — Plain snapshot for read-out.
- **current_values** (fn) — Return current in-memory snapshot.
- **increment_\* / add_\* / set_\*** (fns, ~30) — Typed update helpers per metric.
- **TelemetryCountersContent** (struct) — Markdown-renderable counters; method: `markdown`.

## Module: instruments

- **names** (module) — Canonical metric name constants from the OTEL Metric Catalog.
- **InstrumentRegistry** (struct, feature-gated) — Registry of all OTEL instruments; methods: `from_provider`, `noop`, `with_cardinality_limit`, `with_metric_toggles`, `is_metric_enabled`, `resolve_attrs`, `attr_tool`/`attr_model`/`attr_provider`/`attr_component`/`attr_session`.
- **NoopInstrumentRegistry** (struct) — Zero-sized no-op registry when feature off.

## Module: prometheus (feature-gated)

- **SharedManualReader** (struct) — Newtype around `Arc<ManualReader>` implementing `MetricReader`.
- **render_prometheus_text(reader)** (fn) — Collect and render as Prometheus text format.
- **serve(...)** (async fn) — HTTP server serving `GET /metrics` on `127.0.0.1:<port>`.

## Module: recorder

- **compute_cost_usd(input_tokens, output_tokens, cost)** (fn) — Pure cost computation.
- **LlmRecorder** (struct) — Methods: `new`, `from_subsystem`, `disabled`, `is_enabled`, `record_request`, `record_usage`, `record_cost`, `record_duration`, `record_ttft`, `record_retry`, `record_rate_limit`.
- **ToolRecorder** (struct) — Methods: `from_subsystem`, `disabled`, `is_enabled`, `record_invocation`, `record_duration`.
- **SessionRecorder** (struct) — Methods: `from_subsystem`, `record_session_start`, `record_session_end`, `record_agent_loop`.
- **CoordinatorRecorder** (struct) — Methods: `record_agent_spawn`, `record_agent_complete`, `record_error`, `record_timeout`.
- **PermissionRecorder** (struct) — Methods: `record_approved`, `record_denied`.
- **CompressionRecorder** (struct) — Method: `record_compression`.
- **SnapshotRecorder** (struct) — Method: `record_restore`.

## Module: sensitive

- **REDACTED** (const) — `"redacted"` sentinel.
- **MAX_ATTR_VALUE_LEN** (const) — Maximum legitimate attribute value length (256).
- **looks_sensitive(value)** (fn) — Returns true if value matches a sensitive pattern.
- **sanitize_attr_value(value)** (fn) — Returns `REDACTED` if sensitive, otherwise original.

## Module: shutdown

- **ShutdownGuard** (struct) — RAII guard; methods: `new`, `subsystem`, `subsystem_mut`, `flush`, `into_inner`. Implements `Drop` (flush + shutdown).
- **flush_on_signal_arc(subsystem)** (async fn) — Background task listening for SIGINT/SIGTERM and flushing.