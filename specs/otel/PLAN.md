# Implementation Plan: OpenTelemetry Metrics Export Interface

## Spec ID

otel

## Objective

Add an OpenTelemetry metrics export subsystem to ragent so the agent harness can
stream usage, performance, cost, and effectiveness metrics to a designated OTLP
endpoint. The work introduces a new `ragent-telemetry` crate, a
`telemetry.otel` configuration schema, a curated metric catalog, and
instrumentation hooks wired into the session processor, LLM provider layer, tool
execution path, coordinator, and permission system.

## Approach

1. Create a `ragent-telemetry` crate with a `TelemetrySubsystem` that owns the
   `SdkMeterProvider`, instrument registry, and OTLP exporter. Gate it behind a
   `telemetry` Cargo feature.
2. Add a `TelemetryConfig` / `OtelConfig` schema to `ragent-config` under
   `telemetry.otel`. Map the legacy `ExperimentalFlags.open_telemetry` flag as a
   deprecated alias.
3. Build an instrument registry that constructs all metrics from the catalog
   (counters, histograms, gauges, up/down counters) with the correct names, units,
   and attribute keys. Use an in-memory exporter for unit tests.
4. Instrument in order of dependency: LLM provider call sites (token/cost/duration)
   → tool execution path (invocations/duration) → session processor (sessions/
   loop/iterations) → coordinator (active/completed/errors/timeouts) → permission
   system (approved/denied) → compression pipeline → sub-agent/task subsystem.
5. Add a TUI `/otel` slash command for toggling and status display.
6. Wire shutdown flush via a `tokio::signal` handler or existing graceful shutdown
   path.
7. Add unit tests (in-memory exporter) and one integration test that runs a mock
   session and asserts expected metrics are recorded.
8. Update CHANGELOG.md, SPEC.md status, and QUICKSTART.md.

## Assumptions and Risks

- The `opentelemetry` and `opentelemetry-otlp` crates add non-trivial build time;
  the `telemetry` feature gate keeps them off the default build path.
- High-cardinality attributes (e.g. `model` × `provider` × `tool.name`) could
  blow up metric buffers; FR-035 caps cardinality at 1000 combinations per metric.
- The exporter must never block the agent loop; all recording is synchronous but
  cheap, and export is async-batched.
- The `EventBus` is the lowest-coupling instrumentation point, but some metrics
  (e.g. tool duration) require timing that the event payload doesn't carry; those
  will be instrumented at the call site rather than via the bus.
- The legacy `ExperimentalFlags.open_telemetry` flag must remain functional for
  backward compatibility until a future deprecation cycle.

## Tasks

| ID | Title | Requirement | Effort | Priority | Status | Dependencies |
|----|-------|-------------|--------|----------|--------|--------------|
| T-001 | Create `ragent-telemetry` crate scaffold and add to workspace | FR-001, NFR-001, NFR-006 | S | Critical | completed | — |
| T-002 | Add `telemetry.otel` config schema to `ragent-config` | FR-002 | M | Critical | completed | — |
| T-003 | Implement `TelemetrySubsystem` with `SdkMeterProvider` and no-op fallback | FR-021, FR-022, NFR-002 | L | Critical | completed | T-001, T-002 |
| T-004 | Implement OTLP/HTTP exporter wiring | FR-005, FR-023 | M | Critical | completed | T-003 |
| T-005 | Implement OTLP/gRPC exporter wiring | FR-005, FR-024 | M | High | completed | T-003 |
| T-006 | Build instrument registry for all metrics in the catalog | FR-003, FR-004 | L | Critical | completed | T-003 |
| T-007 | Implement resource attribute injection (service.name, version, host, session.id) | FR-004, FR-025 | M | High | completed | T-006 |
| T-008 | Implement batch export timer and configurable interval | FR-006 | M | High | completed | T-004 |
| T-009 | Implement graceful shutdown flush (SIGINT/SIGTERM) | FR-019 | M | High | completed | T-008 |
| T-010 | Instrument LLM provider layer: requests, tokens, duration, TTFT | FR-007, FR-013, FR-010 | L | Critical | completed | T-006 |
| T-011 | Compute and record estimated cost from token counts and `Cost` metadata | FR-008 | M | High | completed | T-010 |
| T-012 | Instrument RateLimit events into request/token percentage gauges | FR-014 | S | Medium | completed | T-010 |
| T-013 | Instrument tool execution path: invocations and duration | FR-009, FR-015 | M | Critical | completed | T-006 |
| T-014 | Instrument session processor: active/total sessions, loop duration, iterations | FR-011, FR-010 | M | High | completed | T-006 |
| T-015 | Instrument coordinator: active/completed agents, errors, timeouts | FR-012, FR-018 | M | High | completed | T-006 |
| T-016 | Instrument permission system: approved and denied counters | FR-016 | S | Medium | completed | T-006 |
| T-017 | Instrument context compression pipeline | FR-017 | S | Low | completed | T-006 |
| T-018 | Instrument sub-agent/task subsystem: spawns and completions | FR-018 | S | Medium | completed | T-015 |
| T-019 | Map legacy `ExperimentalFlags.open_telemetry` to new config (deprecated alias) | FR-002 | S | Medium | completed | T-002 |
| T-020 | Add cardinality cap (1000 per metric, overflow to `unknown` bucket) | FR-035 | M | High | completed | T-006 |
| T-021 | Add non-blocking guarantee: exporter errors logged, never panic | FR-031, FR-033 | M | Critical | completed | T-004 |
| T-022 | Add sensitive-data guard: no keys/contents/prompts in attributes | FR-034 | S | High | completed | T-006 |
| T-023 | Add per-metric enable/disable toggles from config | FR-027 | M | Low | completed | T-006, T-002 |
| T-024 | Add custom resource attributes from config | FR-026 | S | Low | completed | T-007, T-002 |
| T-025 | Add `/otel` TUI slash command (toggle + status) | FR-030 | M | Medium | completed | T-003 |
| T-026 | Optional: add Prometheus text endpoint for local scraping | FR-028 | L | Low | completed | T-003 |
| T-027 | Optional: instrument snapshot restore counter | FR-029 | S | Low | completed | T-006 |
| T-028 | Write unit tests with in-memory exporter for instrument registry | NFR-005 | M | High | completed | T-006 |
| T-029 | Write unit tests for config parsing and legacy alias | NFR-005 | S | Medium | completed | T-002, T-019 |
| T-030 | Write integration test: mock session records expected metrics | NFR-005, AC-1 | L | High | completed | T-010, T-013, T-014 |
| T-031 | Write integration test: disabled telemetry produces zero network traffic | AC-2 | M | High | completed | T-003 |
| T-032 | Write integration test: malformed endpoint does not crash | AC-7 | M | Medium | completed | T-021 |
| T-033 | Write integration test: SIGINT flushes pending exports | AC-6 | M | Medium | completed | T-009 |
| T-034 | Add docblock documentation to all public functions | NFR-007 | M | Medium | completed | T-003 |
| T-035 | Update CHANGELOG.md, SPEC.md status, and QUICKSTART.md | — | S | Low | completed | T-030 |
## Milestones

### Milestone 1 — Crate and config foundation (T-001, T-002, T-019, T-029)

The `ragent-telemetry` crate exists in the workspace, the `telemetry.otel`
config schema is parsed, and the legacy flag is mapped. Unit tests for config
parsing pass.

### Milestone 2 — Meter provider and exporters (T-003, T-004, T-005, T-008, T-009)

The `TelemetrySubsystem` can create a real or no-op meter provider, export via
HTTP or gRPC, batch on a configurable interval, and flush on shutdown.

### Milestone 3 — Instrument registry and attributes (T-006, T-007, T-020, T-022, T-024)

All catalog metrics are registered with correct names, units, and attributes.
Resource attributes are injected. Cardinality is capped and sensitive data is
guarded.

### Milestone 4 — Core instrumentation (T-010, T-011, T-012, T-013, T-014, T-015)

LLM calls, tool executions, session lifecycle, and coordinator metrics are
instrumented. This is the largest milestone and unblocks acceptance criteria
1–5.

### Milestone 5 — Extended instrumentation (T-016, T-017, T-018, T-027)

Permission system, compression, sub-agents, and snapshot restores are
instrumented.

### Milestone 6 — TUI, options, and tests (T-023, T-025, T-026, T-028, T-030, T-031, T-032, T-033)

Per-metric toggles, `/otel` command, optional Prometheus endpoint, and the full
integration test suite are complete.

### Milestone 7 — Docs and release (T-034, T-035)

Documentation is complete, CHANGELOG and QUICKSTART are updated, and the spec
status moves from `draft` to `implemented`.

## Definition of Done

- `cargo check -p ragent-telemetry` and `cargo test -p ragent-telemetry` pass
  with the `telemetry` feature enabled.
- `cargo check` (default features, no `telemetry`) still passes and produces
  zero OTEL-related warnings.
- All acceptance criteria 1–10 are demonstrated by automated or manual test
  evidence.
- The spec status is moved from `draft` to `implemented` with audit metadata.
- No existing agent loop, TUI, or HTTP server functionality is broken when
  telemetry is disabled.
- No API keys, file contents, or user prompts appear in any exported metric.