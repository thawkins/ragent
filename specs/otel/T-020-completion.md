# T-020 Completion: Add cardinality cap (1000 per metric, overflow to `unknown` bucket)

**Spec ID:** otel
**Task:** T-020
**Requirement:** FR-035
**Status:** completed
**Dependencies:** T-006
**Effort:** M
**Priority:** High

## Summary

Implemented a per-metric cardinality cap that limits the number of distinct
attribute-value combinations tracked per metric. When the limit (default 1000)
is exceeded, excess combinations are collapsed into a single `unknown` bucket
by replacing all attribute values with `"unknown"` (FR-035).

The cap is enforced centrally in a new `CardinalityCache` type, wired into
`InstrumentRegistry` via an `Arc`-shared field, and called by all recorder
methods that use tagged attributes before passing them to the underlying OTEL
instruments.

## Requirement

### FR-035

> The system shall not exceed a default cardinality limit of 1000 distinct
> attribute combinations per metric; if the limit is exceeded, the system
> shall aggregate excess attributes into an `unknown` bucket.

## Changes

### `crates/ragent-config/src/telemetry.rs`

- **New field `cardinality_limit: usize`** on `OtelConfig` — the per-metric
  cardinality limit. Defaults to 1000 via `default_cardinality_limit()`.
  Serialised/deserialised as `cardinality_limit` in the `telemetry.otel`
  JSON block.

### `crates/ragent-config/src/config.rs`

- **New field `pub telemetry: TelemetryConfig`** on `Config` — the
  `TelemetryConfig` wrapper was defined but never embedded into the
  top-level `Config` struct. This was a pre-existing gap (the test file
  `test_telemetry_config.rs` was already failing to compile). Now
  `Config` has a `telemetry` field with `#[serde(default)]`, making
  `config.telemetry.otel` accessible to consumers.

- **`Config::merge`** — added `base.telemetry = TelemetryConfig::merge(...)`
  so overlay-enabled telemetry config takes precedence and resource/metric
  maps are unioned (FR-002, T-019).

### `crates/ragent-telemetry/src/cardinality.rs` (NEW)

- **`CardinalityCache`** — thread-safe (`RwLock<HashMap<String, HashSet>>>`)
  cache that tracks distinct attribute-value signatures per metric name.
  - `new(max: usize)` — create with a per-metric limit.
  - `resolve(metric_name, attrs) -> Vec<KeyValue>` — the core method:
    returns attrs unchanged if the combination is already seen or the
    limit hasn't been reached; replaces all values with `"unknown"` if
    the limit is exceeded.
  - `distinct_count(metric_name) -> usize` — test helper.
  - `limit() -> usize` — returns the configured limit.
  - `Default` impl uses `DEFAULT_CARDINALITY_LIMIT` (1000).
  - Feature-off no-op stub (`CardinalityCache` zero-sized struct).
  - 10 feature-on unit tests + 1 no-op test.

### `crates/ragent-telemetry/src/instruments.rs`

- **New field `cardinality: Arc<CardinalityCache>`** on `InstrumentRegistry`.
  Shared across all clones so cardinality is tracked globally per metric.
- **New method `with_cardinality_limit(limit) -> Self`** — builder that
  replaces the default cache with one using the given limit.
- **New method `resolve_attrs(metric_name, attrs) -> Vec<KeyValue>`** —
  delegates to the `CardinalityCache::resolve`. Called by all recorder
  methods before recording.

### `crates/ragent-telemetry/src/subsystem.rs`

- **`TelemetrySubsystem::instruments()`** — now calls
  `.with_cardinality_limit(self.config.cardinality_limit)` on the
  registry so the configured limit is propagated from config.

### `crates/ragent-telemetry/src/recorder.rs`

- **All 12 tagged-attribute recorder methods updated** to call
  `reg.resolve_attrs(metric_name, &attrs)` before passing to the
  underlying OTEL instrument. Each metric name uses the constant from
  `instruments::names` (e.g. `names::LLM_REQUESTS`,
  `names::TOKENS_INPUT`, `names::TOOL_INVOCATIONS`, etc.).

  Methods updated:
  - `LlmRecorder::record_request` → `LLM_REQUESTS`
  - `LlmRecorder::record_usage` → `TOKENS_INPUT`, `TOKENS_OUTPUT`
  - `LlmRecorder::record_cost` → `COST_ESTIMATED`
  - `LlmRecorder::record_duration` → `LLM_DURATION`
  - `LlmRecorder::record_ttft` → `LLM_TTFT`
  - `LlmRecorder::record_retry` → `RETRIES_LLM`
  - `LlmRecorder::record_rate_limit` → `RATE_LIMIT_REQUESTS_PCT`, `RATE_LIMIT_TOKENS_PCT`
  - `ToolRecorder::record_invocation` → `TOOL_INVOCATIONS`
  - `ToolRecorder::record_duration` → `TOOL_DURATION`
  - `CoordinatorRecorder::record_error` → `ERRORS_TOTAL`
  - `PermissionRecorder::record_approved` → `PERMISSION_APPROVED`
  - `PermissionRecorder::record_denied` → `PERMISSION_DENIED`

  Methods that record with no attributes (empty `&[]` slice) are not
  affected — they have no cardinality to cap:
  - `SessionRecorder::record_session_start/end`, `record_agent_loop`
  - `CoordinatorRecorder::record_agent_spawn/complete`, `record_timeout`
  - `CompressionRecorder::record_compression`

### `crates/ragent-telemetry/src/lib.rs`

- Added `pub mod cardinality;` module declaration.

## Verification

### Build checks

```
cargo check -p ragent-config                           # pass
cargo check -p ragent-telemetry                        # pass (no feature)
cargo check -p ragent-telemetry --features telemetry   # pass
cargo check -p ragent-agent --features telemetry       # pass
cargo check                                            # pass (full workspace)
```

### Test results

```
cargo test -p ragent-telemetry --features telemetry --lib -- cardinality
test result: ok. 10 passed; 0 failed; 0 ignored; 0 measured; 62 filtered out

cargo test -p ragent-telemetry --features telemetry --lib -- --skip test_flush_on_signal
test result: ok. 70 passed; 0 failed; 0 ignored; 0 measured; 2 filtered out

cargo test -p ragent-telemetry --lib  (no feature)
test result: ok. 21 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

cargo test -p ragent-config  (all tests)
test result: ok. 16+6+9+2+3+6+4+3+6+7+14+6+6+5+6 passed across all test binaries

cargo test -p ragent-agent --lib
test result: ok. 316 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
```

The `test_flush_on_signal*` tests are skipped because they spawn real signal
handlers that block indefinitely — pre-existing, not related to T-020.

### New cardinality unit tests

| Test | Verifies |
|------|----------|
| `test_empty_attrs_returns_empty` | No-attr path is fast-path no-op |
| `test_new_combination_registered` | First-seen combo returns unchanged |
| `test_seen_combination_returned_unchanged` | Already-seen combo returns unchanged |
| `test_different_combinations_tracked_separately` | Distinct combos counted independently |
| `test_overflow_collapses_to_unknown` | Excess combos collapse to `"unknown"` |
| `test_existing_combination_after_overflow_still_returned_unchanged` | Pre-overflow combos survive |
| `test_different_metrics_tracked_independently` | Per-metric isolation |
| `test_multi_attribute_overflow_replaces_all_values` | All attr values replaced on overflow |
| `test_default_limit_is_1000` | Default limit matches FR-035 |
| `test_limit_zero_collapses_immediately` | Edge case: limit=0 |
| `test_noop_cardinality_cache` (no-feature) | No-op stub doesn't panic |

## Pre-existing issue fixed

The `Config` struct was missing a `telemetry: TelemetryConfig` field — the
module and type existed but were never wired into `Config`. The test file
`test_telemetry_config.rs` (untracked, pre-existing) was failing to compile
because of this. T-020 adds the field and the merge logic, fixing 26
compilation errors and 1 test failure in the config test suite.

## Spec / Plan updates

- `specs/otel/PLAN.md` T-020 row: status changed from `pending` to `completed`.
- Spec task `T-020` status updated to `completed` via `spec_task_update`.