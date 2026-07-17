# T-023 Completion Report — Per-metric enable/disable toggles from config

## Spec / Plan references

- **Spec ID:** `otel`
- **Task:** T-023 — Add per-metric enable/disable toggles from config
- **Requirement:** FR-027
- **Dependencies:** T-006 (instrument registry), T-002 (config schema)
- **Status:** `completed` (see `specs/otel/PLAN.md` line 78)

## Requirement text

> **FR-027** The system may support per-metric enable/disable toggles via a
> `telemetry.otel.metrics` map in `ragent.json`, allowing users to disable
> specific metrics to reduce cardinality or volume.

## What was implemented

### 1. Config field (already present from T-002)

`OtelConfig.metrics: HashMap<String, bool>` already exists in
`crates/ragent-config/src/telemetry.rs`, keyed by canonical instrument name.
A metric absent from the map is enabled by default; a metric present and
set to `false` is disabled. The map serialises/deserialises from the
`telemetry.otel.metrics` JSON block.

### 2. InstrumentRegistry carries the toggles

`crates/ragent-telemetry/src/instruments.rs`:

- Added `metric_toggles: Arc<HashMap<String, bool>>` field to
  `InstrumentRegistry`, shared across clones via `Arc` (same pattern as
  the cardinality cache).
- Added `with_metric_toggles(map)` builder method, mirroring
  `with_cardinality_limit`.
- Added `is_metric_enabled(name) -> bool` guard:
  - absent key → `true` (enabled by default, fail-open)
  - present key → the stored `bool`
- `from_meter` initialises the toggles to an empty `Arc<HashMap>` by
  default; `TelemetrySubsystem::instruments()` overrides it with the
  user's config.

### 3. Subsystem wires the config into the registry

`crates/ragent-telemetry/src/subsystem.rs`:

- `TelemetrySubsystem::instruments()` now calls
  `.with_metric_toggles(self.config.metrics.clone())` after
  `.with_cardinality_limit(...)`, so the `telemetry.otel.metrics` map
  from `ragent.json` reaches every recorder.

### 4. Recorders short-circuit disabled metrics

`crates/ragent-telemetry/src/recorder.rs` — every recorder method now
checks `reg.is_metric_enabled(names::METRIC)` before calling the
underlying OTEL instrument:

| Recorder | Method | Metrics guarded |
|----------|--------|-----------------|
| `LlmRecorder` | `record_request` | `ragent.llm.requests` |
| `LlmRecorder` | `record_usage` | `ragent.tokens.input`, `ragent.tokens.output` (independent) |
| `LlmRecorder` | `record_cost` | `ragent.cost.estimated` |
| `LlmRecorder` | `record_duration` | `ragent.llm.duration` |
| `LlmRecorder` | `record_ttft` | `ragent.llm.time_to_first_token` |
| `LlmRecorder` | `record_retry` | `ragent.retries.llm` |
| `LlmRecorder` | `record_rate_limit` | `ragent.rate_limit.requests_pct`, `ragent.rate_limit.tokens_pct` (independent) |
| `ToolRecorder` | `record_invocation` | `ragent.tool.invocations` |
| `ToolRecorder` | `record_duration` | `ragent.tool.duration` |
| `SessionRecorder` | `record_session_start` | `ragent.sessions.total`, `ragent.sessions.active` (independent) |
| `SessionRecorder` | `record_session_end` | `ragent.sessions.active` |
| `SessionRecorder` | `record_agent_loop` | `ragent.agent_loop.duration`, `ragent.agent_loop.iterations` (independent) |
| `CoordinatorRecorder` | `record_agent_spawn` | `ragent.subagent.spawns`, `ragent.agents.active` (independent) |
| `CoordinatorRecorder` | `record_agent_complete` | `ragent.agents.active`, `ragent.agents.completed` (independent) |
| `CoordinatorRecorder` | `record_error` | `ragent.errors.total` |
| `CoordinatorRecorder` | `record_timeout` | `ragent.timeouts.total` |
| `PermissionRecorder` | `record_approved` | `ragent.permission.approved` |
| `PermissionRecorder` | `record_denied` | `ragent.permission.denied` |
| `CompressionRecorder` | `record_compression` | `ragent.context.compressions`, `ragent.context.compression_ratio` (independent) |

Sibling metrics recorded by the same method (e.g. `ragent.tokens.input`
vs `ragent.tokens.output`, `ragent.sessions.total` vs
`ragent.sessions.active`) are checked independently so disabling one
does not suppress the other.

### 5. Fail-open design

The toggle is keyed by the canonical instrument name. A typo in the
key (e.g. `ragent.llm.reqeusts`) silently leaves the real metric enabled
rather than silently suppressing data. This is the safer failure mode:
an unexpected metric is a minor cardinality surprise, while a silently
missing metric could mask a real operational issue.

## Tests

`crates/ragent-telemetry/tests/test_metric_toggles.rs` (20 tests, all
passing):

| Category | Tests |
|----------|-------|
| `is_metric_enabled` helper | `test_is_metric_enabled_absent_is_true`, `test_is_metric_enabled_present_uses_stored_value`, `test_empty_toggles_all_enabled` |
| Sibling independence | `test_disabling_one_token_metric_leaves_sibling_enabled`, `test_disabling_sessions_active_leaves_total_enabled`, `test_disabling_permission_approved_leaves_denied_enabled` |
| Fail-open on typo | `test_typo_in_toggle_key_leaves_metric_enabled` |
| Shared state across clones | `test_toggles_shared_across_clones` |
| Builder override | `test_with_metric_toggles_replaces_default` |
| Subsystem config wiring | `test_subsystem_instruments_wires_toggles_from_config`, `test_subsystem_no_toggles_all_enabled` |
| Per-recorder short-circuit | `test_llm_recorder_short_circuits_disabled_metric`, `test_tool_recorder_short_circuits_disabled_metric`, `test_session_recorder_short_circuits_disabled_metric`, `test_coordinator_recorder_short_circuits_disabled_metric`, `test_permission_recorder_short_circuits_disabled_metric`, `test_compression_recorder_short_circuits_disabled_metric` |
| Config serde | `test_metrics_map_deserializes_from_json`, `test_absent_metrics_field_defaults_to_empty` |
| No-op recorder defence | `test_disabled_recorders_ignore_toggles` |

## Verification

```text
$ cargo check -p ragent-telemetry --features telemetry
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.81s

$ cargo check -p ragent-telemetry
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.64s

$ cargo check --workspace
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 18.67s

$ cargo check --workspace --features ragent-telemetry/telemetry
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 12.12s

$ cargo test -p ragent-telemetry --features telemetry --test test_metric_toggles
running 20 tests
... (all 20 pass) ...
test result: ok. 20 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

$ cargo test -p ragent-telemetry --features telemetry --lib --tests
... (103 lib + 10+16+10+20+19+6+5+19 integration tests all pass) ...

$ cargo test -p ragent-telemetry --features telemetry --doc
test result: ok. 7 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
```

## Files touched

| File | Change |
|------|--------|
| `crates/ragent-telemetry/src/instruments.rs` | Added `metric_toggles` field, `with_metric_toggles` builder, `is_metric_enabled` guard; initialised in `from_meter`. |
| `crates/ragent-telemetry/src/subsystem.rs` | `instruments()` calls `.with_metric_toggles(self.config.metrics.clone())`; updated doc comment. |
| `crates/ragent-telemetry/src/recorder.rs` | Every recorder method now short-circuits on `!is_metric_enabled(name)`; doc comments reference FR-027. |
| `crates/ragent-telemetry/tests/test_metric_toggles.rs` | New 20-test integration suite (new file). |
| `specs/otel/PLAN.md` | T-023 status flipped from `pending` to `completed`. |
| `CHANGELOG.md` | "Added — Per-metric enable/disable toggles from config" entry under `## Unreleased`. |

## Definition of Done checklist (from PLAN.md)

- [x] `cargo check -p ragent-telemetry` and `cargo test -p ragent-telemetry`
      pass with the `telemetry` feature enabled.
- [x] `cargo check` (default features, no `telemetry`) still passes and
      produces zero OTEL-related warnings.
- [x] Per-metric toggles from `telemetry.otel.metrics` suppress disabled
      metrics (zero exported data points).
- [x] Sibling metrics recorded by the same method are independent.
- [x] The toggle is fail-open (typo leaves the real metric enabled).
- [x] No existing agent loop, TUI, or HTTP server functionality is broken
      when telemetry is disabled.