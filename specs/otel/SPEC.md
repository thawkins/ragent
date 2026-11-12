---
status: implemented
audit:
  - { time: 1784284250, from: "none", to: "draft", actor: "system" }
  - { time: 1794443740, from: "draft", to: "in_review", actor: "cli" }
  - { time: 1794443740, from: "in_review", to: "approved", actor: "cli" }
  - { time: 1794443740, from: "approved", to: "in_progress", actor: "cli" }
  - { time: 1794443740, from: "in_progress", to: "implemented", actor: "cli" }
---
# Specification: OpenTelemetry Metrics and Logs Export Interface

## Spec ID

otel

## Summary

This specification defines an OpenTelemetry (OTEL) metrics export interface for
ragent, enabling the agent harness to stream structured metrics to a designated
OTLP-compatible endpoint (collector or direct backend). It introduces a
configurable `telemetry.otel` block in `ragent.json`, a new `ragent-telemetry`
crate that owns the meter provider and instrument registry, and a curated set of
common metrics that track the **usage**, **performance**, **cost**, and
**effectiveness** of the coding harness across sessions, agent loops, LLM calls,
and tool invocations.

The design leverages the existing `AgentLoopProfiler`, `Coordinator::Metrics`,
`StreamEvent::Usage`, `Cost` model, and the `EventBus` as instrumentation sources,
wiring them into OTEL instruments rather than replacing them.

## Scope

### In Scope

- Add a `telemetry.otel` configuration schema for endpoint URL, protocol
  (HTTP/gRPC), export interval, resource attributes, and per-metric enable/disable
  toggles.
- Introduce a `ragent-telemetry` crate that owns the `SdkMeterProvider`,
  instrument registry, and OTLP exporter lifecycle.
- Define and register a curated catalog of OTEL metrics across four dimensions:
  usage, performance, cost, and effectiveness.
- Instrument the session processor, LLM provider layer, tool execution path,
  coordinator, and permission system to record metrics.
- Provide a `/otel` TUI slash command to toggle export and show live status.
- Wire the existing `ExperimentalFlags.open_telemetry` flag into the new
  telemetry subsystem as the legacy on/off switch (deprecated alias).
- Export metrics via the standard OTLP/HTTP and OTLP/gRPC protocols.
- Export `tracing` events over the OTLP **logs** signal, opt-in via
  `telemetry.otel.logs`, through a `tracing_subscriber::Layer` that composes
  with the existing local sinks.

- Support graceful shutdown that flushes pending metric exports on process
  termination.

### Out of Scope

- Distributed tracing (spans) — this spec covers metrics only; traces may be a
  future spec.
- Log export via OTLP logs signal.
- Custom user-defined metrics or plugin-instrumented metrics.
- Real-time streaming of individual metric data points to the TUI (the TUI
  continues to use `AgentLoopProfiler` snapshots; OTEL is a background exporter).
- Backend-specific dashboards or alerting rules.
- OpenTelemetry Collector configuration guidance.

## Context

### Existing Instrumentation Sources

- **`AgentLoopProfiler`** (`crates/ragent-agent/src/session/profiler.rs`):
  Tracks per-operation counters and timing aggregates (`count`, `total_ms`,
  `avg_ms`, `max_ms`, `last_ms`) for the session processor. It exposes
  `record_duration(label, duration)` and `ProfileSnapshot`. This is the primary
  source for performance histograms.

- **`Coordinator::Metrics`**
  (`crates/ragent-agent/src/orchestrator/coordinator.rs`):
  Atomic counters for `active_jobs`, `completed_jobs`, `timeouts`, and `errors`.
  Exposed via `metrics_snapshot() -> MetricsSnapshot`. These feed the
  usage/effectiveness gauges and counters.

- **`StreamEvent::Usage`** (`crates/ragent-types/src/llm.rs`):
  Carries `input_tokens` and `output_tokens` per LLM response. This is the
  source for token counters and derived cost.

- **`StreamEvent::RateLimit`** (`crates/ragent-types/src/llm.rs`):
  Carries `requests_used_pct` and `tokens_used_pct`. Source for quota gauges.

- **`Cost`** (`crates/ragent-config/src/config.rs`):
  Per-token cost in USD per million tokens for input and output. Used to derive
  estimated cost counters from token counts.

- **`EventBus`** (`ragent_types::event::EventBus`):
  Tokio broadcast bus used by the session processor, tool layer, TUI, and HTTP
  server. A telemetry subscriber can listen to events for instrumentation without
  coupling to internal structs.

- **`ExperimentalFlags.open_telemetry`**
  (`crates/ragent-config/src/config.rs`):
  An existing boolean flag (default `false`). This spec formalises the full
  telemetry subsystem; the flag becomes a deprecated alias for
  `telemetry.otel.enabled`.

### Existing Crate Layout

The project is a 15-crate Cargo workspace. The new `ragent-telemetry` crate will
sit alongside `ragent-config`, `ragent-types`, and `ragent-agent` and depend on
`ragent-types` (for `EventBus`, `StreamEvent`) and `ragent-config` (for the
`TelemetryConfig` schema).

### Agent Loop Flow

1. User submits a prompt (TUI or HTTP).
2. `SessionProcessor` starts/resumes the agent loop.
3. Each iteration: build `ChatRequest` → stream LLM response → parse tool calls
   → execute tools → append results → repeat until `FinishReason::Stop`.
4. `StreamEvent::Usage` is emitted per LLM call with token counts.
5. Tool execution emits events via `EventBus`.
6. `Coordinator` tracks active/completed jobs for sub-agents and teams.

Each of these stages is an instrumentation point.

## Assumptions

1. The user has access to an OTLP-compatible collector or backend (e.g.
   Jaeger, Prometheus via OpenTelemetry Collector, Grafana Cloud, Honeycomb,
   Datadog) reachable at a configured endpoint URL.
2. Network access to the endpoint may be unavailable at runtime; the exporter
   must degrade gracefully (buffer-and-drop or log-and-continue) and must never
   block the agent loop.
3. The existing `AgentLoopProfiler` and `Coordinator::Metrics` remain the
   primary in-process sources; OTEL instruments read from them rather than
   replacing them.
4. Metric export is asynchronous and batched; the export interval is configurable
   (default 30 s).
5. The `opentelemetry` and `opentelemetry-otlp` Rust crates are acceptable
   dependencies to add to the workspace.
6. All metric names follow the OTEL semantic conventions for instrument names
   (lowercase, dot-separated, unit-suffixed where applicable).

## Metric Catalog

Metrics are grouped into four dimensions. Each metric lists its instrument
type (Counter, UpDownCounter, Gauge, Histogram), unit, source, and the
requirements that reference it.

### Usage Metrics

| Metric Name | Type | Unit | Source |
|---|---|---|---|
| `ragent.llm.requests` | Counter | `{request}` | LLM provider call sites |
| `ragent.sessions.active` | UpDownCounter | `{session}` | SessionProcessor start/stop |
| `ragent.sessions.total` | Counter | `{session}` | SessionProcessor creation |
| `ragent.messages.user` | Counter | `{message}` | User input events |
| `ragent.tool.invocations` | Counter (by `tool.name`) | `{invocation}` | Tool execution path |
| `ragent.agents.active` | UpDownCounter | `{agent}` | Coordinator active_jobs |
| `ragent.agents.completed` | Counter | `{agent}` | Coordinator completed_jobs |
| `ragent.subagent.spawns` | Counter | `{subagent}` | Task subsystem |
| `ragent.team.members` | Gauge | `{member}` | Team manager |

### Performance Metrics

| Metric Name | Type | Unit | Source |
|---|---|---|---|
| `ragent.llm.duration` | Histogram (by `model`, `provider`, `profile.bucket=loop.llm.total`) | `ms` | LLM call timing |
| `ragent.llm.time_to_first_token` | Histogram (by `model`, `profile.bucket=loop.llm.first_event_wait`) | `ms` | Stream first delta |
| `ragent.tool.duration` | Histogram (by `tool.name`, `profile.bucket=tool.total`) | `ms` | Tool execution timing |
| `ragent.agent_loop.duration` | Histogram | `ms` | AgentLoopProfiler label `agent_loop` |
| `ragent.agent_loop.iterations` | Histogram | `{iteration}` | Session loop iteration count |
| `ragent.session.duration` | Histogram | `ms` | Session start to close |
| `ragent.tool.permission_wait` | Histogram | `ms` | Permission request to resolution |
| `ragent.profile.duration` | Histogram (by `profile.operation`, `profile.group`, `tool.name` for `tool.*:<tool>` labels) | `ms` | Every AgentLoopProfiler scope |

`ragent.profile.duration` carries one series per profiler bucket: each
AgentLoopProfiler scope (`loop.llm.stream`, `loop.tool_phase.total`,
`tool.total:bash`, ...) is recorded with `profile.operation` set to the label
and `profile.group` set to the label minus its last dot segment and any
`:<tool>` suffix (`loop.llm`, `tool`). It is recorded whenever OTel export is
enabled, independent of the `/profile` panel, and can be switched off with
`"ragent.profile.duration": false` in `telemetry.otel.metrics`. The
`profile.bucket` attribute on `ragent.llm.duration`,
`ragent.llm.time_to_first_token` and `ragent.tool.duration` names the profiler
bucket each of those metrics measures, so the two can be joined in a dashboard.

### Cost Metrics

| Metric Name | Type | Unit | Source |
|---|---|---|---|
| `ragent.tokens.input` | Counter (by `model`, `provider`) | `{token}` | `StreamEvent::Usage` |
| `ragent.tokens.output` | Counter (by `model`, `provider`) | `{token}` | `StreamEvent::Usage` |
| `ragent.tokens.cache_read` | Counter (by `model`) | `{token}` | Usage cache_read field |
| `ragent.tokens.cache_write` | Counter (by `model`) | `{token}` | Usage cache_write field |
| `ragent.cost.estimated` | Counter (by `model`, `provider`) | `USD` | Derived from tokens × Cost |
| `ragent.cost.session` | Histogram | `USD` | Sum of costs per session |
| `ragent.rate_limit.requests_pct` | Gauge (by `provider`) | `%` | `StreamEvent::RateLimit` |
| `ragent.rate_limit.tokens_pct` | Gauge (by `provider`) | `%` | `StreamEvent::RateLimit` |

### Effectiveness Metrics

| Metric Name | Type | Unit | Source |
|---|---|---|---|
| `ragent.errors.total` | Counter (by `component`) | `{error}` | Error events / Coordinator |
| `ragent.timeouts.total` | Counter | `{timeout}` | Coordinator timeouts |
| `ragent.permission.denied` | Counter (by `tool.name`) | `{denial}` | Permission system |
| `ragent.permission.approved` | Counter (by `tool.name`) | `{approval}` | Permission system |
| `ragent.context.compressions` | Counter | `{compression}` | Compression pipeline |
| `ragent.context.compression_ratio` | Histogram | `%` | Before/after token count |
| `ragent.tool.calls_per_session` | Histogram | `{call}` | Per-session tool call count |
| `ragent.task.completions` | Counter | `{task}` | `task_complete` / team tasks |
| `ragent.retries.llm` | Counter (by `model`) | `{retry}` | Provider retry logic |
| `ragent.snapshot.restores` | Counter | `{restore}` | Snapshot undo system |

## Requirements

### Ubiquitous

- **FR-001** The system shall provide a `ragent-telemetry` crate that owns the
  OpenTelemetry `SdkMeterProvider`, instrument registry, and OTLP exporter
  lifecycle.
- **FR-002** The system shall read telemetry configuration from a
  `telemetry.otel` block in `ragent.json` (or `ragent.jsonc`), falling back to
  disabled-by-default when the block is absent.
- **FR-003** The system shall register every metric listed in the Metric
  Catalog as an OTEL instrument with the specified name, type, unit, and
  attribute dimensions.
- **FR-004** The system shall attach the following resource attributes to all
  exported metrics: `service.name` (default `"ragent"`), `service.version`
  (from `CARGO_PKG_VERSION`), `host.name` (system hostname), and
  `session.id` (the active session ID when known).
- **FR-005** The system shall export metrics using the OTLP/HTTP protocol by
  default and support OTLP/gRPC as a configurable alternative.
- **FR-006** The system shall batch metric exports on a configurable interval
  (default 30 seconds) and flush all pending exports on process shutdown.
- **FR-007** The system shall record `ragent.llm.requests`, `ragent.tokens.input`,
  and `ragent.tokens.output` for every LLM provider call, tagged with `model`
  and `provider` attributes.
- **FR-008** The system shall compute `ragent.cost.estimated` by multiplying
  recorded token counts by the model's `Cost` metadata (USD per million tokens)
  and record it as a counter per `model` and `provider`.
- **FR-009** The system shall record `ragent.tool.invocations` and
  `ragent.tool.duration` for every tool execution, tagged with `tool.name`.
- **FR-010** The system shall record `ragent.agent_loop.duration` and
  `ragent.agent_loop.iterations` from the `AgentLoopProfiler` data for each
  completed session.
- **FR-011** The system shall record `ragent.sessions.active` as an
  `UpDownCounter` incremented on session start and decremented on session end.
- **FR-012** The system shall record `ragent.errors.total` and
  `ragent.timeouts.total` from the `Coordinator::Metrics` counters, tagged with
  `component` where identifiable.

### Event-Driven

- **FR-013** When a `StreamEvent::Usage` is received from an LLM provider, the
  system shall record `ragent.tokens.input` and `ragent.tokens.output` with the
  `model` and `provider` attributes.
- **FR-014** When a `StreamEvent::RateLimit` is received, the system shall
  update the `ragent.rate_limit.requests_pct` and `ragent.rate_limit.tokens_pct`
  gauges for the corresponding `provider`.
- **FR-015** When a tool invocation begins, the system shall start a timer and
  record `ragent.tool.invocations`; when it completes, the system shall record
  `ragent.tool.duration`.
- **FR-016** When a permission request is resolved (approved or denied), the
  system shall record `ragent.permission.approved` or
  `ragent.permission.denied` tagged with `tool.name`.
- **FR-017** When the context compression pipeline runs, the system shall
  record `ragent.context.compressions` and observe the before/after ratio in
  `ragent.context.compression_ratio`.
- **FR-018** When a sub-agent is spawned, the system shall record
  `ragent.subagent.spawns` and increment `ragent.agents.active`; when it
  completes, the system shall decrement `ragent.agents.active` and increment
  `ragent.agents.completed`.
- **FR-019** When the process receives a shutdown signal (SIGINT/SIGTERM), the
  system shall flush all pending metric exports to the configured endpoint
  before terminating.
- **FR-020** When a tool execution results in an error, the system shall record
  `ragent.errors.total` tagged with `component = "tool"` and the `tool.name`.

### State-Driven

- **FR-021** While telemetry is enabled, the system shall maintain a live
  `SdkMeterProvider` with all registered instruments available for recording.
- **FR-022** While telemetry is disabled, the system shall use a no-op meter
  provider so instrumentation calls have zero overhead.
- **FR-023** While the OTLP exporter is configured for HTTP, the system shall
  use `OTLP/HTTP` with `POST` to the configured endpoint URL.
- **FR-024** While the OTLP exporter is configured for gRPC, the system shall
  use `OTLP/gRPC` to the configured endpoint URL.
- **FR-025** While a session is active, the `session.id` resource attribute
  shall be set on all metrics recorded within that session's context.

### Optional

- **FR-026** The system may support custom resource attributes via
  `telemetry.otel.resource_attributes` in `ragent.json`.
- **FR-027** The system may support per-metric enable/disable toggles via a
  `telemetry.otel.metrics` map in `ragent.json`, allowing users to disable
  specific metrics to reduce cardinality or volume.
- **FR-028** The system may support an in-process metrics endpoint
  (`telemetry.otel.internal_port`) that exposes metrics in Prometheus text
  format for local scraping without an OTLP collector.
- **FR-029** The system may support exporting a `ragent.snapshot.restores`
  counter when the snapshot undo system is invoked.
- **FR-030** The system may support a `/otel status` TUI sub-command that
  displays the configured endpoint, export count, last export time, and any
  exporter errors.

### Unwanted

- **FR-031** The system shall not block the agent loop, LLM streaming, or tool
  execution if the OTLP exporter is unavailable or slow; all metric recording
  and export shall be asynchronous and non-blocking.
- **FR-032** The system shall not export metrics when `telemetry.otel.enabled`
  is `false`; the no-op meter provider must produce zero network traffic.
- **FR-033** The system shall not crash the process if the OTLP endpoint returns
  an error; exporter errors shall be logged at `warn` level and retried on the
  next export interval.
- **FR-034** The system shall not record sensitive data (API keys, file
  contents, user prompts) as metric attributes or resource attributes.
- **FR-035** The system shall not exceed a default cardinality limit of 1000
  distinct attribute combinations per metric; if the limit is exceeded, the
  system shall aggregate excess attributes into an `unknown` bucket.

### Logs Signal

- **FR-036** The system shall export its `tracing` events to the OTLP logs
  signal when `telemetry.otel.enabled` and `telemetry.otel.logs.enabled` are both
  `true`, posting to `/v1/logs` (HTTP) or the collector's gRPC log service at the
  same endpoint and on the same protocol as the metrics.
- **FR-036a** The system shall re-emit every TUI log-window entry as a `tracing`
  event at **the entry's own severity**, so the OTLP logs signal receives the
  activity the panel shows - tool-call start, tool arguments, tool result, tool
  failure, warnings and information lines - whether or not the panel is open
  (FR-036e).
  A tool call is exported at `INFO`, a warning at `WARN` and an error at `ERROR`,
  so both default floors (`--log-level info` and
  `telemetry.otel.logs.level: info`) admit them and tool-call activity reaches a
  collector with no extra configuration. Each such record shall carry the
  attributes `log_window = true` and `log_window_level` naming the entry's own
  severity (`tool`, `info`, `warn`, `error`), so a collector can separate a tool
  failure from a routine tool call even where both map to the same `INFO`
  severity.
- **FR-036e** The re-emit required by FR-036a shall **not** be conditional on the
  log panel being visible. A session normally runs with the panel closed, so
  gating the export on `show_log` made the collector's tool-call coverage depend
  on whether the operator happened to open `/log`: `Bash` emits a native
  `Executing bash command` record on every call and so arrived regardless, while
  `Edit`, `Write`, `Grep`, `read` and the remaining tools emit no `tracing` event
  at all and arrived only with the panel open. Observed from a collector that
  reads as "only Bash tool calls are exported", for tools that the panel-gated
  code treated identically. The panel-visibility gate shall therefore apply only
  to the panel's own rendering and to the spool file; the export is unconditional
  (subject to the FR-036d duplicate guard and the FR-036c burst bound).
- **FR-036d** The system shall keep each line to exactly one exported record at
  every `logs.level` floor by putting the duplicate guard on the **caller**
  rather than on a universally quiet severity. Several panel lines exist twice -
  once as a native `tracing` event and once as the panel's copy of it (the drain
  of `TuiTracingLayer` records, the mirrored-child stderr drain) - so those call
  sites use the deferred push, which records the line in the panel and the spool
  and never re-emits it. Every other entry is re-emitted exactly once. An earlier
  revision instead emitted every re-emit at a fixed `DEBUG`, which kept the
  streams disjoint but placed the transport below both default floors, so a tool
  call reached the collector only when the operator also raised `--log-level`;
  in practice the logs signal shipped no tool-call activity at all. A fixed echo
  severity also tied the duplicate guard to one `logs.level` floor, whereas
  suppressing the echo holds at every floor.
- **FR-036c** The system shall bound the log-window re-emit at 60 records per
  second across the whole process, as a token bucket, because one panel line is
  one exported record: a wall of mirrored child-process stderr - an MCP server
  launched through `npx` emits hundreds of banner, deprecation and readiness
  lines at startup - would otherwise put thousands of records into one batch.
  When a burst ends, the first entry that fits again shall be preceded by a
  `warn` record naming the number of entries withheld, so the collector sees an
  explicit gap rather than a silent loss. Interactive diagnosis is unchanged:
  the panel and the spool file still show every line.
- **FR-036b** The system shall accept `--log-level all` as a spelling of the
  highest verbosity `tracing` defines (`trace`), so a run can record every
  severity for both the local sink and the OTLP logs export.
- **FR-037** The system shall deliver log records through a batched log
  processor on a background thread; the thread that emits a `tracing` event
  shall never block on the network or on the collector.
- **FR-038** The system shall batch log exports on the same
  `telemetry.otel.export_interval_seconds` as the metrics, and shall flush the
  log pipeline on `flush`, `shutdown` and reconfigure so a process that exits
  inside one interval does not drop its last records.
- **FR-039** The system shall pass every exported log body through the shared
  credential redactor and every exported log attribute through the existing
  metric-attribute sensitive-data guard, and shall truncate a body longer than
  16 KiB rather than exporting it whole.
- **FR-040** While `telemetry.otel.logs.enabled` is `false`, or telemetry itself
  is disabled, the system shall export no log records at all, and the local sink
  (the TUI log panel or stderr) shall continue to receive every event.
- **FR-041** If the OTLP log exporter cannot be built, the system shall log the
  failure, keep the metric signal running, and continue with local logging
  only; a broken logs pipeline shall never disable telemetry or stop the
  process.
- **FR-042** When the log pipeline is reconfigured (`/telemetry on`, `/telemetry
  off`, `/telemetry logs <level>`, `/telemetry setup`), the system shall apply
  the change to the installed layer without reinstalling the global tracing
  subscriber, and shall not lose the records emitted before the change.

## Non-Functional Requirements

- **NFR-001** The `ragent-telemetry` crate shall depend only on
  `ragent-types`, `ragent-config`, `opentelemetry`, `opentelemetry-otlp`, and
  `tokio`; no additional heavyweight dependencies.
- **NFR-002** Metric recording calls shall complete in under 1 µs per call
  when telemetry is disabled (no-op provider).
- **NFR-003** Metric recording calls shall complete in under 10 µs per call
  when telemetry is enabled, excluding the asynchronous export path.
- **NFR-004** The exporter shall not consume more than 16 MB of memory for
  metric buffers under default configuration.
- **NFR-005** All configuration and instrumentation code shall be unit-testable
  without a live OTLP endpoint (using a mock/in-memory exporter).
- **NFR-006** The telemetry subsystem shall be feature-gated behind a
  `telemetry` Cargo feature so users who do not need OTEL can build without the
  dependency.
- **NFR-007** All public functions in `ragent-telemetry` shall have docblock
  documentation per the project's documentation standards.
- **NFR-008** With the OTLP logs signal disabled, the tracing layer shall add no
  more than one `None` check per emitted event, and shall allocate nothing.

## Configuration Schema

The `telemetry.otel` block in `ragent.json`:

```jsonc
{
  "telemetry": {
    "otel": {
      "enabled": false,
      "endpoint": "http://localhost:4318",
      "protocol": "http",
      "export_interval_seconds": 30,
      "service_name": "ragent",
      "resource_attributes": {
        "deployment.environment": "development"
      },
      "metrics": {
        "ragent.tokens.input": true,
        "ragent.cost.estimated": true,
        "ragent.tool.invocations": false
      },
      "logs": {
        "enabled": false,
        "level": "info"
      }
    }
  }
}
```

| Field | Type | Default | Description |
|---|---|---|---|
| `enabled` | bool | `false` | Master on/off switch |
| `endpoint` | string | `"http://localhost:4318"` | OTLP endpoint base URL |
| `protocol` | `"http"` \| `"grpc"` | `"http"` | Export transport protocol |
| `export_interval_seconds` | u64 | `30` | Batch export interval |
| `service_name` | string | `"ragent"` | `service.name` resource attribute |
| `resource_attributes` | map | `{}` | Custom resource attributes |
| `metrics` | map<string, bool> | `{}` | Per-metric enable/disable |
| `logs.enabled` | bool | `false` | Export `tracing` events over the OTLP logs signal |
| `logs.level` | `"all"`\|`"trace"`\|`"debug"`\|`"info"`\|`"warn"`\|`"error"` | `"info"` | Minimum severity exported to the collector (the local sink is unaffected) |

The existing `experimental.open_telemetry` flag is treated as a deprecated
alias: if `telemetry.otel` is absent and `experimental.open_telemetry` is
`true`, telemetry is enabled with default settings and a log warning is
emitted.

## Acceptance Criteria

1. With `telemetry.otel.enabled: true` and a valid endpoint, running a
   one-shot `ragent run "hello"` produces at least one export containing
   `ragent.llm.requests`, `ragent.tokens.input`, and `ragent.tokens.output`
   metrics with correct `model` and `provider` attributes.
2. With `telemetry.otel.enabled: false`, no HTTP/gRPC traffic is generated to
   the endpoint and the agent loop incurs no measurable overhead.
3. Tool invocations are reflected in `ragent.tool.invocations` and
   `ragent.tool.duration` with the `tool.name` attribute.
4. Sub-agent spawns and completions are reflected in `ragent.agents.active`
   (up/down) and `ragent.subagent.spawns`.
5. A session with multiple LLM calls produces a `ragent.cost.estimated` counter
   whose value approximates `tokens × cost_per_million / 1_000_000`.
6. Sending SIGINT during an active session flushes pending exports before the
   process exits.
7. A malformed or unreachable endpoint does not crash the process; errors are
   logged at `warn` level.
8. The `ragent-telemetry` crate compiles and all unit tests pass without a live
   OTLP endpoint (using an in-memory exporter).
9. The `/otel` TUI command toggles export on/off and displays current status.
10. No API keys, file contents, or user prompts appear in any metric attribute
    or resource attribute.
11. With `telemetry.otel.enabled: true` and `telemetry.otel.logs.enabled: true`,
    `ragent run "hello"` posts at least one record to `/v1/logs` whose body is a
    message ragent actually logged, and those same messages still appear in the
    log panel (TUI) or on stderr.
12. With `telemetry.otel.logs.enabled: false` (the default), no request is made
    to `/v1/logs` and the tracing layer adds no allocation to the emitting path.
13. A log line containing a credential (an API key, an `Authorization: Bearer`
    header) reaches the collector redacted, and a body over 16 KiB is truncated
    with a `... [truncated]` marker.
14. Setting `telemetry.otel.logs.level: "warn"` stops `info` and `debug` records
    being exported while the local sink still shows them.
15. An unreachable collector or a malformed log endpoint leaves the process
    running with local logging intact; the failure is logged at `warn` level and
    the metric signal keeps exporting.

## Related Work

- Existing profiler: `crates/ragent-agent/src/session/profiler.rs`
- Existing coordinator metrics: `crates/ragent-agent/src/orchestrator/coordinator.rs`
- Existing LLM stream events: `crates/ragent-types/src/llm.rs`
- Existing config schema: `crates/ragent-config/src/config.rs`
- Existing event bus: `crates/ragent-types/src/event/mod.rs`
- OpenTelemetry Rust SDK: <https://crates.io/crates/opentelemetry>
- OTLP exporter crate: <https://crates.io/crates/opentelemetry-otlp>
- OTel logs data model: <https://opentelemetry.io/docs/specs/otel/logs/data-model/>
- Existing local sinks: `src/main.rs` (tracing init), `crates/ragent-tui/src/tracing_layer.rs`
- OTEL semantic conventions: <https://opentelemetry.io/docs/specs/semconv/>