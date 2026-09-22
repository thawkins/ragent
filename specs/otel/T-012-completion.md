# T-012 Completion: Instrument RateLimit events into request/token percentage gauges

**Spec ID:** otel
**Task:** T-012
**Requirement:** FR-014
**Status:** completed
**Dependencies:** T-010

## Summary

Instrumented the `StreamEvent::RateLimit` stream event so that the request
and token quota percentages are recorded as OTEL gauges when telemetry is
enabled.

## FR-014

> When a `StreamEvent::RateLimit` is received, the system shall update the
> `ragent.rate_limit.requests_pct` and `ragent.rate_limit.tokens_pct` gauges
> for the corresponding `provider`.

## Changes

### `crates/ragent-telemetry/src/recorder.rs`

- **Module doc comment** — added `record_rate_limit` to the metrics table
  mapping `ragent.rate_limit.requests_pct` / `ragent.rate_limit.tokens_pct`
  to FR-014.

- **`LlmRecorder::record_rate_limit`** (feature-on impl, line ~187) —
  takes `provider: &str`, `requests_used_pct: Option<f32>`,
  `tokens_used_pct: Option<f32>`. When telemetry is enabled, each `Some`
  value is converted to `f64` and recorded to the corresponding gauge tagged
  with the `provider` attribute. `None` values skip the gauge (left
  untouched). No-op when the registry is `None` (telemetry disabled).

- **`LlmRecorder::record_rate_limit`** (feature-off no-op impl, line ~251)
  — zero-overhead stub that discards all arguments.

- **Unit test `test_record_rate_limit_updates_gauges`** (line ~308) —
  records `Some(75.0)` / `Some(40.0)` for provider `"openai"` and
  `None` / `None` for `"anthropic"`, flushes the in-memory exporter, then
  asserts the openai-tagged `ragent.rate_limit.requests_pct` data point is
  `75.0` and `ragent.rate_limit.tokens_pct` is `40.0`.

- **Existing tests updated** — `test_disabled_recorder_is_noop` and the
  feature-off `test_noop_recorder` now also call `record_rate_limit` to
  confirm the no-op path doesn't panic.

### `crates/ragent-agent/src/session/loop_steps.rs`

- **`StreamEvent::RateLimit` handler** (line ~1165) — now calls
  `self.llm_recorder.record_rate_limit(&turn.model_ref.provider_id, ...)`
  before publishing the existing `Event::QuotaUpdate`. The event-bus
  publish is unchanged.

## Verification

```
cargo check -p ragent-telemetry --features telemetry   # pass
cargo check -p ragent-agent --features telemetry        # pass
cargo check -p ragent-agent                             # pass (no telemetry)
cargo test -p ragent-telemetry --features telemetry --lib recorder
test result: ok. 10 passed; 0 failed; 0 ignored; 0 measured
```

All 10 recorder unit tests pass, including the new
`test_record_rate_limit_updates_gauges`.