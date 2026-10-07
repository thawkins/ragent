//! High-level recorders for the telemetry subsystem.
//!
//! This module provides convenience wrappers around the
//! [`InstrumentRegistry`] that the session processor, LLM provider layer,
//! tool execution path, coordinator, permission system, compression pipeline,
//! and snapshot subsystem use to record metrics. It encapsulates the
//! attribute construction and instrument calls so call sites stay clean.
//!
//! # Feature gating
//!
//! When the `telemetry` feature is enabled, each recorder holds an
//! `Option<InstrumentRegistry>` - `Some` when the subsystem is enabled,
//! `None` when disabled. When the feature is off, all recorders are
//! zero-sized no-ops.
//!
//! # Metrics recorded
//!
//! | Recorder | Method | Metric | Requirement |
//! |----------|--------|--------|-------------|
//! | [`LlmRecorder`] | [`record_request`](LlmRecorder::record_request) | `ragent.llm.requests` | FR-007 |
//! | [`LlmRecorder`] | [`record_usage`](LlmRecorder::record_usage) | `ragent.tokens.input`, `ragent.tokens.output` | FR-013 |
//! | [`LlmRecorder`] | [`record_cost`](LlmRecorder::record_cost) | `ragent.cost.estimated` | FR-008 |
//! | [`LlmRecorder`] | [`record_duration`](LlmRecorder::record_duration) | `ragent.llm.duration` | FR-007 |
//! | [`LlmRecorder`] | [`record_ttft`](LlmRecorder::record_ttft) | `ragent.llm.time_to_first_token` | FR-007 |
//! | [`SnapshotRecorder`] | [`record_restore`](SnapshotRecorder::record_restore) | `ragent.snapshot.restores` | FR-029 |

//! # In-memory counter mirrors (ANTIPAT M4.5/MEDIUM-8)
//!
//! The `counters::SNAPSHOT` mirrors that back the `/telemetry counters`
//! diagnostics surface are updated **unconditionally** in every recorder
//! method - whether the OTEL provider is enabled or disabled, and whether the
//! individual metric is toggled on or off. The mirrors are an independent,
//! in-process view of the same events, so gating them behind the OTEL registry
//! (or behind a per-metric toggle) made the counters depend on which recorder
//! last ran. Instrument *export*, by contrast, stays gated on the FR-027
//! toggles.

#[cfg(feature = "telemetry")]
use crate::instruments::InstrumentRegistry;
#[cfg(feature = "telemetry")]
use crate::instruments::names;

/// Number of tokens in one million, used to convert per-million-token model
/// pricing into a per-call USD cost (LOW-2).
const TOKENS_PER_MILLION: f64 = 1_000_000.0;

/// Compute the estimated USD cost for an LLM response (FR-008).
///
/// Multiplies the recorded token counts by the model's [`Cost`] metadata
/// (USD per million tokens) and returns the sum:
///
/// ```text
/// cost = (input_tokens x cost.input + output_tokens x cost.output) / 1_000_000
/// ```
///
/// This is a pure function with no side effects; the session processor calls
/// it and then passes the result to [`LlmRecorder::record_cost`].
///
/// # Arguments
///
/// * `input_tokens` - Number of input/prompt tokens consumed.
/// * `output_tokens` - Number of output/completion tokens produced.
/// * `cost` - The model's per-million-token cost metadata.
#[must_use]
pub fn compute_cost_usd(input_tokens: u64, output_tokens: u64, cost: &ragent_config::Cost) -> f64 {
    let input_cost = (input_tokens as f64) * cost.input;
    let output_cost = (output_tokens as f64) * cost.output;
    (input_cost + output_cost) / TOKENS_PER_MILLION
}

/// Recorder for LLM provider metrics (T-010, FR-007, FR-013, FR-008).
///
/// Wraps an [`InstrumentRegistry`] and provides high-level methods that
/// construct the correct attributes (`model`, `provider`) and call the
/// appropriate OTEL instruments.
///
/// When telemetry is disabled, the recorder holds `None` and all methods
/// are zero-overhead no-ops (FR-022, NFR-002).
#[cfg(feature = "telemetry")]
pub struct LlmRecorder {
    registry: Option<InstrumentRegistry>,
}

/// No-op recorder used when the `telemetry` Cargo feature is off.
#[cfg(not(feature = "telemetry"))]
pub struct LlmRecorder;

#[cfg(feature = "telemetry")]
impl LlmRecorder {
    /// Create a recorder from a live [`InstrumentRegistry`].
    ///
    /// Primarily intended for tests and advanced use cases where the caller
    /// already has an instrument registry in hand. Prefer
    /// [`from_subsystem`](Self::from_subsystem) in production code.
    #[must_use]
    pub const fn new(registry: InstrumentRegistry) -> Self {
        Self {
            registry: Some(registry),
        }
    }

    /// Create a recorder from a [`TelemetrySubsystem`](crate::TelemetrySubsystem).
    ///
    /// If the subsystem is enabled and the `telemetry` feature is active,
    /// the recorder holds a live `InstrumentRegistry`. Otherwise, it holds
    /// `None` and all recording methods are no-ops.
    #[must_use]
    pub fn from_subsystem(sub: &crate::TelemetrySubsystem) -> Self {
        Self {
            registry: sub.instruments(),
        }
    }

    /// Create a disabled recorder (all methods are no-ops).
    #[must_use]
    pub const fn disabled() -> Self {
        Self { registry: None }
    }

    /// Returns `true` if this recorder has live instruments.
    #[must_use]
    pub const fn is_enabled(&self) -> bool {
        self.registry.is_some()
    }

    /// Record an LLM request: increment `ragent.llm.requests` (FR-007).
    ///
    /// Tagged with `model` and `provider` attributes.
    ///
    /// Short-circuits when the metric is disabled via
    /// `telemetry.otel.metrics` (FR-027).
    pub fn record_request(&self, model: &str, provider: &str) {
        if let Some(reg) = &self.registry {
            if !reg.is_metric_enabled(names::LLM_REQUESTS) {
                return;
            }
            let attrs = [
                InstrumentRegistry::attr_model(model),
                InstrumentRegistry::attr_provider(provider),
            ];
            let resolved = reg.resolve_attrs(names::LLM_REQUESTS, &attrs);
            reg.llm_requests.add(1, &resolved);
            crate::counters::increment_llm_requests(1);
        }
    }

    /// Record token usage from a `StreamEvent::Usage` event (FR-013).
    ///
    /// Increments `ragent.tokens.input` and `ragent.tokens.output`,
    /// tagged with `model` and `provider` attributes.
    ///
    /// Short-circuits when both token metrics are disabled via
    /// `telemetry.otel.metrics` (FR-027). If only one is disabled, the
    /// other is still recorded.
    pub fn record_usage(&self, model: &str, provider: &str, input_tokens: u64, output_tokens: u64) {
        if let Some(reg) = &self.registry {
            let attrs = [
                InstrumentRegistry::attr_model(model),
                InstrumentRegistry::attr_provider(provider),
            ];
            if reg.is_metric_enabled(names::TOKENS_INPUT) {
                let resolved_in = reg.resolve_attrs(names::TOKENS_INPUT, &attrs);
                reg.tokens_input.add(input_tokens, &resolved_in);
                crate::counters::increment_tokens_input(input_tokens);
            }
            if reg.is_metric_enabled(names::TOKENS_OUTPUT) {
                let resolved_out = reg.resolve_attrs(names::TOKENS_OUTPUT, &attrs);
                reg.tokens_output.add(output_tokens, &resolved_out);
                crate::counters::increment_tokens_output(output_tokens);
            }
        }
    }

    /// Record the estimated USD cost for an LLM response (FR-008).
    ///
    /// `cost_usd` should be the pre-computed estimated cost, typically obtained
    /// from [`compute_cost_usd`] using the model's [`Cost`](ragent_config::Cost)
    /// metadata and the token counts from a `StreamEvent::Usage` event. The
    /// value is added to the `ragent.cost.estimated` counter, tagged with
    /// `model` and `provider` attributes.
    ///
    /// Recording is a no-op when telemetry is disabled.
    ///
    /// Short-circuits when `ragent.cost.estimated` is disabled via
    /// `telemetry.otel.metrics` (FR-027).
    pub fn record_cost(&self, model: &str, provider: &str, cost_usd: f64) {
        if let Some(reg) = &self.registry {
            if !reg.is_metric_enabled(names::COST_ESTIMATED) {
                return;
            }
            let attrs = [
                InstrumentRegistry::attr_model(model),
                InstrumentRegistry::attr_provider(provider),
            ];
            let resolved = reg.resolve_attrs(names::COST_ESTIMATED, &attrs);
            reg.cost_estimated.add(cost_usd, &resolved);
            crate::counters::increment_cost_estimated(cost_usd);
        }
    }

    /// Record LLM call duration in milliseconds (FR-007).
    ///
    /// Records to `ragent.llm.duration` histogram, tagged with `model`
    /// and `provider` attributes.
    ///
    /// Short-circuits when the metric is disabled via
    /// `telemetry.otel.metrics` (FR-027).
    pub fn record_duration(&self, model: &str, provider: &str, duration_ms: f64) {
        if let Some(reg) = &self.registry {
            if !reg.is_metric_enabled(names::LLM_DURATION) {
                return;
            }
            let attrs = [
                InstrumentRegistry::attr_model(model),
                InstrumentRegistry::attr_provider(provider),
            ];
            let resolved = reg.resolve_attrs(names::LLM_DURATION, &attrs);
            reg.llm_duration.record(duration_ms, &resolved);
            crate::counters::set_llm_duration_last(duration_ms);
        }
    }

    /// Record time-to-first-token in milliseconds (FR-007).
    ///
    /// Records to `ragent.llm.time_to_first_token` histogram, tagged with
    /// `model` attribute.
    ///
    /// Short-circuits when the metric is disabled via
    /// `telemetry.otel.metrics` (FR-027).
    pub fn record_ttft(&self, model: &str, ttft_ms: f64) {
        if let Some(reg) = &self.registry {
            if !reg.is_metric_enabled(names::LLM_TTFT) {
                return;
            }
            let attrs = [InstrumentRegistry::attr_model(model)];
            let resolved = reg.resolve_attrs(names::LLM_TTFT, &attrs);
            reg.llm_ttft.record(ttft_ms, &resolved);
            crate::counters::set_llm_ttft_last(ttft_ms);
        }
    }

    /// Record an LLM retry (increments `ragent.retries.llm`).
    ///
    /// Short-circuits when the metric is disabled via
    /// `telemetry.otel.metrics` (FR-027).
    pub fn record_retry(&self, model: &str, provider: &str) {
        if let Some(reg) = &self.registry {
            if !reg.is_metric_enabled(names::RETRIES_LLM) {
                return;
            }
            let attrs = [
                InstrumentRegistry::attr_model(model),
                InstrumentRegistry::attr_provider(provider),
            ];
            let resolved = reg.resolve_attrs(names::RETRIES_LLM, &attrs);
            reg.retries_llm.add(1, &resolved);
            crate::counters::increment_retries_llm(1);
        }
    }

    /// Record rate-limit / quota percentages from a `StreamEvent::RateLimit`
    /// event (FR-014).
    ///
    /// Updates the `ragent.rate_limit.requests_pct` and
    /// `ragent.rate_limit.tokens_pct` gauges, tagged with the `provider`
    /// attribute. Each `Option<f32>` value is converted to `f64`; `None`
    /// values skip the corresponding gauge (it is left untouched).
    ///
    /// Recording is a no-op when telemetry is disabled.
    ///
    /// Short-circuits a gauge when its metric is disabled via
    /// `telemetry.otel.metrics` (FR-027).
    pub fn record_rate_limit(
        &self,
        provider: &str,
        requests_used_pct: Option<f32>,
        tokens_used_pct: Option<f32>,
    ) {
        if let Some(reg) = &self.registry {
            let attr = [InstrumentRegistry::attr_provider(provider)];
            if let Some(pct) = requests_used_pct
                && reg.is_metric_enabled(names::RATE_LIMIT_REQUESTS_PCT)
            {
                let resolved = reg.resolve_attrs(names::RATE_LIMIT_REQUESTS_PCT, &attr);
                reg.rate_limit_requests_pct
                    .record(f64::from(pct), &resolved);
                crate::counters::set_rate_limit_requests_pct(f64::from(pct));
            }
            if let Some(pct) = tokens_used_pct
                && reg.is_metric_enabled(names::RATE_LIMIT_TOKENS_PCT)
            {
                let resolved = reg.resolve_attrs(names::RATE_LIMIT_TOKENS_PCT, &attr);
                reg.rate_limit_tokens_pct.record(f64::from(pct), &resolved);
                crate::counters::set_rate_limit_tokens_pct(f64::from(pct));
            }
        }
    }
}

#[cfg(not(feature = "telemetry"))]
impl LlmRecorder {
    /// Create a recorder (no-op when the feature is off).
    #[must_use]
    pub fn from_subsystem(_sub: &crate::TelemetrySubsystem) -> Self {
        Self
    }

    /// Create a disabled recorder.
    #[must_use]
    pub fn disabled() -> Self {
        Self
    }

    /// Returns `false` when the feature is off.
    #[must_use]
    pub fn is_enabled(&self) -> bool {
        false
    }

    /// No-op: record an LLM request (still updates the in-memory snapshot).
    pub fn record_request(&self, _model: &str, _provider: &str) {
        crate::counters::increment_llm_requests(1);
    }

    /// No-op: record token usage (still updates the in-memory snapshot).
    pub fn record_usage(
        &self,
        _model: &str,
        _provider: &str,
        input_tokens: u64,
        output_tokens: u64,
    ) {
        crate::counters::increment_tokens_input(input_tokens);
        crate::counters::increment_tokens_output(output_tokens);
    }

    /// No-op: record LLM call duration (still updates the in-memory snapshot).
    pub fn record_duration(&self, _model: &str, _provider: &str, duration_ms: f64) {
        crate::counters::set_llm_duration_last(duration_ms);
    }

    /// No-op: record time-to-first-token (still updates the in-memory snapshot).
    pub fn record_ttft(&self, _model: &str, ttft_ms: f64) {
        crate::counters::set_llm_ttft_last(ttft_ms);
    }

    /// No-op: record an LLM retry (still updates the in-memory snapshot).
    pub fn record_retry(&self, _model: &str, _provider: &str) {
        crate::counters::increment_retries_llm(1);
    }

    /// No-op: record the estimated cost (still updates the in-memory snapshot).
    pub fn record_cost(&self, _model: &str, _provider: &str, cost_usd: f64) {
        crate::counters::increment_cost_estimated(cost_usd);
    }

    /// No-op: record rate-limit percentages (still updates the in-memory snapshot).
    pub fn record_rate_limit(
        &self,
        _provider: &str,
        requests_used_pct: Option<f32>,
        tokens_used_pct: Option<f32>,
    ) {
        if let Some(pct) = requests_used_pct {
            crate::counters::set_rate_limit_requests_pct(f64::from(pct));
        }
        if let Some(pct) = tokens_used_pct {
            crate::counters::set_rate_limit_tokens_pct(f64::from(pct));
        }
    }
}

impl Default for LlmRecorder {
    fn default() -> Self {
        Self::disabled()
    }
}

// -- Tool recorder --------------------------------------------------------

/// Recorder for tool execution metrics (T-013, FR-009, FR-015).
///
/// Wraps an [`InstrumentRegistry`] and provides high-level methods that
/// construct the correct `tool.name` attribute and call the appropriate
/// OTEL instruments.
///
/// When telemetry is disabled, the recorder holds `None` and all methods
/// are zero-overhead no-ops (FR-022, NFR-002). When the `telemetry` Cargo
/// feature is off, [`ToolRecorder`] is a zero-sized no-op.
///
/// # Metrics recorded (FR-009, FR-015)
///
/// | Method | Metric | Requirement |
/// |--------|--------|-------------|
/// | [`record_invocation`](ToolRecorder::record_invocation) | `ragent.tool.invocations` | FR-009 |
/// | [`record_duration`](ToolRecorder::record_duration) | `ragent.tool.duration` | FR-015 |
#[cfg(feature = "telemetry")]
#[derive(Clone)]
pub struct ToolRecorder {
    registry: Option<InstrumentRegistry>,
}

/// No-op tool recorder used when the `telemetry` feature is off.
#[cfg(not(feature = "telemetry"))]
#[derive(Clone, Copy)]
pub struct ToolRecorder;

#[cfg(feature = "telemetry")]
impl ToolRecorder {
    /// Create a recorder from a live [`InstrumentRegistry`].
    #[must_use]
    pub const fn new(registry: InstrumentRegistry) -> Self {
        Self {
            registry: Some(registry),
        }
    }

    /// Create a recorder from a [`TelemetrySubsystem`](crate::TelemetrySubsystem).
    ///
    /// If the subsystem is enabled and the `telemetry` feature is active,
    /// the recorder holds a live `InstrumentRegistry`. Otherwise, it holds
    /// `None` and all recording methods are no-ops.
    #[must_use]
    pub fn from_subsystem(sub: &crate::TelemetrySubsystem) -> Self {
        Self {
            registry: sub.instruments(),
        }
    }

    /// Create a disabled recorder (all methods are no-ops).
    #[must_use]
    pub const fn disabled() -> Self {
        Self { registry: None }
    }

    /// Returns `true` if this recorder has live instruments.
    #[must_use]
    pub const fn is_enabled(&self) -> bool {
        self.registry.is_some()
    }

    /// Record a tool invocation: increment `ragent.tool.invocations` (FR-009).
    ///
    /// Tagged with the `tool.name` attribute.
    pub fn record_invocation(&self, tool_name: &str) {
        if let Some(reg) = &self.registry {
            // FR-027 (ANTIPAT HIGH-4): honour `telemetry.otel.metrics`.
            if !reg.is_metric_enabled(names::TOOL_INVOCATIONS) {
                return;
            }
            let attrs = [InstrumentRegistry::attr_tool(tool_name)];
            let resolved = reg.resolve_attrs(names::TOOL_INVOCATIONS, &attrs);
            reg.tool_invocations.add(1, &resolved);
            crate::counters::increment_tool_invocations(1);
        }
    }

    /// Record tool execution duration in milliseconds (FR-015).
    ///
    /// Records to `ragent.tool.duration` histogram, tagged with the
    /// `tool.name` attribute.
    pub fn record_duration(&self, tool_name: &str, duration_ms: f64) {
        if let Some(reg) = &self.registry {
            if !reg.is_metric_enabled(names::TOOL_DURATION) {
                return;
            }
            let attrs = [InstrumentRegistry::attr_tool(tool_name)];
            let resolved = reg.resolve_attrs(names::TOOL_DURATION, &attrs);
            reg.tool_duration.record(duration_ms, &resolved);
            crate::counters::set_tool_duration_last(duration_ms);
        }
    }
}

#[cfg(not(feature = "telemetry"))]
impl ToolRecorder {
    /// Create a recorder (no-op when the feature is off).
    #[must_use]
    pub fn from_subsystem(_sub: &crate::TelemetrySubsystem) -> Self {
        Self
    }

    /// Create a disabled recorder.
    #[must_use]
    pub fn disabled() -> Self {
        Self
    }

    /// Returns `false` when the feature is off.
    #[must_use]
    pub fn is_enabled(&self) -> bool {
        false
    }

    /// No-op: record a tool invocation.
    pub fn record_invocation(&self, _tool_name: &str) {}

    /// No-op: record tool execution duration.
    pub fn record_duration(&self, _tool_name: &str, _duration_ms: f64) {}
}

impl Default for ToolRecorder {
    fn default() -> Self {
        Self::disabled()
    }
}

// -- Session recorder -----------------------------------------------------

/// Recorder for session processor metrics (T-014, FR-011, FR-010).
///
/// Wraps an [`InstrumentRegistry`] and provides high-level methods for
/// session lifecycle and agent-loop metrics.
///
/// When telemetry is disabled, the recorder holds `None` and all methods
/// are zero-overhead no-ops (FR-022, NFR-002). When the `telemetry` Cargo
/// feature is off, [`SessionRecorder`] is a zero-sized no-op.
///
/// # Metrics recorded (FR-011, FR-010)
///
/// | Method | Metric | Requirement |
/// |--------|--------|-------------|
/// | [`record_session_start`](SessionRecorder::record_session_start) | `ragent.sessions.active` (up), `ragent.sessions.total` | FR-011 |
/// | [`record_session_end`](SessionRecorder::record_session_end) | `ragent.sessions.active` (down) | FR-011 |
/// | [`record_agent_loop`](SessionRecorder::record_agent_loop) | `ragent.agent_loop.duration`, `ragent.agent_loop.iterations` | FR-010 |
/// | [`record_tool_calls_per_session`](SessionRecorder::record_tool_calls_per_session) | `ragent.tool.calls_per_session` | FR-025 |
#[cfg(feature = "telemetry")]
#[derive(Clone)]
pub struct SessionRecorder {
    registry: Option<InstrumentRegistry>,
}

/// No-op session recorder used when the `telemetry` feature is off.
#[cfg(not(feature = "telemetry"))]
#[derive(Clone, Copy)]
pub struct SessionRecorder;

#[cfg(feature = "telemetry")]
impl SessionRecorder {
    /// Create a recorder from a live [`InstrumentRegistry`].
    #[must_use]
    pub const fn new(registry: InstrumentRegistry) -> Self {
        Self {
            registry: Some(registry),
        }
    }

    /// Create a recorder from a [`TelemetrySubsystem`](crate::TelemetrySubsystem).
    #[must_use]
    pub fn from_subsystem(sub: &crate::TelemetrySubsystem) -> Self {
        Self {
            registry: sub.instruments(),
        }
    }

    /// Create a disabled recorder (all methods are no-ops).
    #[must_use]
    pub const fn disabled() -> Self {
        Self { registry: None }
    }

    /// Returns `true` if this recorder has live instruments.
    #[must_use]
    pub const fn is_enabled(&self) -> bool {
        self.registry.is_some()
    }

    /// Record session start: increment `ragent.sessions.active` (up/down
    /// counter) and `ragent.sessions.total` (counter) (FR-011).
    pub fn record_session_start(&self) {
        if let Some(reg) = &self.registry {
            // FR-027 (ANTIPAT HIGH-4): per-metric toggles are honoured.
            if reg.is_metric_enabled(names::SESSIONS_ACTIVE) {
                reg.sessions_active.add(1, &[]);
                crate::counters::add_sessions_active(1);
            }
            if reg.is_metric_enabled(names::SESSIONS_TOTAL) {
                reg.sessions_total.add(1, &[]);
                crate::counters::increment_sessions_total(1);
            }
        }
    }

    /// Record session end: decrement `ragent.sessions.active` (FR-011).
    pub fn record_session_end(&self) {
        if let Some(reg) = &self.registry {
            if !reg.is_metric_enabled(names::SESSIONS_ACTIVE) {
                return;
            }
            reg.sessions_active.add(-1, &[]);
            crate::counters::add_sessions_active(-1);
        }
    }

    /// Record agent-loop metrics for a completed session (FR-010, FR-025 of
    /// spec `agentloop`).
    ///
    /// `duration_ms` is the total wall-clock duration of the agent loop and
    /// `iterations` is the number of loop iterations completed.
    ///
    /// The last-value snapshot gauges are updated unconditionally (the
    /// in-memory mirror is independent of the OTEL export pipeline), so loop
    /// depth and duration stay observable in the diagnostics surfaces even
    /// when the OTEL provider is disabled.
    pub fn record_agent_loop(&self, duration_ms: f64, iterations: u64) {
        if let Some(reg) = &self.registry {
            if reg.is_metric_enabled(names::AGENT_LOOP_DURATION) {
                reg.agent_loop_duration.record(duration_ms, &[]);
            }
            if reg.is_metric_enabled(names::AGENT_LOOP_ITERATIONS) {
                reg.agent_loop_iterations.record(iterations, &[]);
            }
        }
        crate::counters::set_agent_loop_duration_last(duration_ms);
        crate::counters::set_agent_loop_iterations_last(iterations);
    }

    /// Record the tool-call total for a completed agent-loop run (FR-025 of
    /// spec `agentloop`): records to the `ragent.tool.calls_per_session`
    /// histogram and updates the last-value snapshot gauge.
    ///
    /// Like [`record_agent_loop`](Self::record_agent_loop), the snapshot
    /// gauge is updated even when the OTEL provider is disabled.
    pub fn record_tool_calls_per_session(&self, count: u64) {
        if let Some(reg) = &self.registry {
            if reg.is_metric_enabled(names::TOOL_CALLS_PER_SESSION) {
                reg.tool_calls_per_session.record(count, &[]);
            }
        }
        crate::counters::set_tool_calls_per_session_last(count);
    }
}

#[cfg(not(feature = "telemetry"))]
impl SessionRecorder {
    /// Create a recorder (no-op when the feature is off).
    #[must_use]
    pub fn from_subsystem(_sub: &crate::TelemetrySubsystem) -> Self {
        Self
    }

    /// Create a disabled recorder.
    #[must_use]
    pub fn disabled() -> Self {
        Self
    }

    /// Returns `false` when the feature is off.
    #[must_use]
    pub fn is_enabled(&self) -> bool {
        false
    }

    /// No-op: record session start.
    pub fn record_session_start(&self) {}

    /// No-op: record session end.
    pub fn record_session_end(&self) {}

    /// Record agent-loop metrics; the last-value snapshot gauges are still
    /// updated (the mirror is independent of the OTEL pipeline).
    pub fn record_agent_loop(&self, duration_ms: f64, iterations: u64) {
        crate::counters::set_agent_loop_duration_last(duration_ms);
        crate::counters::set_agent_loop_iterations_last(iterations);
    }

    /// Record the per-run tool-call total; the last-value snapshot gauge is
    /// still updated (the mirror is independent of the OTEL pipeline).
    pub fn record_tool_calls_per_session(&self, count: u64) {
        crate::counters::set_tool_calls_per_session_last(count);
    }
}

impl Default for SessionRecorder {
    fn default() -> Self {
        Self::disabled()
    }
}

// -- Coordinator recorder --------------------------------------------------

/// Recorder for coordinator metrics (T-015, FR-012, FR-018).
///
/// Wraps an [`InstrumentRegistry`] and provides high-level methods for
/// coordinator / sub-agent lifecycle and error metrics.
///
/// When telemetry is disabled, the recorder holds `None` and all methods
/// are zero-overhead no-ops (FR-022, NFR-002). When the `telemetry` Cargo
/// feature is off, [`CoordinatorRecorder`] is a zero-sized no-op.
///
/// # Metrics recorded (FR-012, FR-018)
///
/// | Method | Metric | Requirement |
/// |--------|--------|-------------|
/// | [`record_agent_spawn`](CoordinatorRecorder::record_agent_spawn) | `ragent.subagent.spawns`, `ragent.agents.active` (up) | FR-018 |
/// | [`record_agent_complete`](CoordinatorRecorder::record_agent_complete) | `ragent.agents.active` (down), `ragent.agents.completed` | FR-018 |
/// | [`record_error`](CoordinatorRecorder::record_error) | `ragent.errors.total` | FR-012 |
/// | [`record_timeout`](CoordinatorRecorder::record_timeout) | `ragent.timeouts.total` | FR-012 |
#[cfg(feature = "telemetry")]
#[derive(Clone)]
pub struct CoordinatorRecorder {
    registry: Option<InstrumentRegistry>,
}

/// No-op coordinator recorder used when the `telemetry` feature is off.
#[cfg(not(feature = "telemetry"))]
#[derive(Clone, Copy)]
pub struct CoordinatorRecorder;

#[cfg(feature = "telemetry")]
impl CoordinatorRecorder {
    /// Create a recorder from a live [`InstrumentRegistry`].
    #[must_use]
    pub const fn new(registry: InstrumentRegistry) -> Self {
        Self {
            registry: Some(registry),
        }
    }

    /// Create a recorder from a [`TelemetrySubsystem`](crate::TelemetrySubsystem).
    #[must_use]
    pub fn from_subsystem(sub: &crate::TelemetrySubsystem) -> Self {
        Self {
            registry: sub.instruments(),
        }
    }

    /// Create a disabled recorder (all methods are no-ops).
    #[must_use]
    pub const fn disabled() -> Self {
        Self { registry: None }
    }

    /// Returns `true` if this recorder has live instruments.
    #[must_use]
    pub const fn is_enabled(&self) -> bool {
        self.registry.is_some()
    }

    /// Record a sub-agent spawn: increment `ragent.subagent.spawns` and
    /// `ragent.agents.active` (FR-018).
    pub fn record_agent_spawn(&self) {
        if let Some(reg) = &self.registry {
            // FR-027 (ANTIPAT HIGH-4): per-metric toggles are honoured.
            if reg.is_metric_enabled(names::SUBAGENT_SPAWNS) {
                reg.subagent_spawns.add(1, &[]);
                crate::counters::increment_subagent_spawns(1);
            }
            if reg.is_metric_enabled(names::AGENTS_ACTIVE) {
                reg.agents_active.add(1, &[]);
                crate::counters::add_agents_active(1);
            }
        }
    }

    /// Record a sub-agent completion: decrement `ragent.agents.active`
    /// and increment `ragent.agents.completed` (FR-018).
    pub fn record_agent_complete(&self) {
        if let Some(reg) = &self.registry {
            if reg.is_metric_enabled(names::AGENTS_ACTIVE) {
                reg.agents_active.add(-1, &[]);
                crate::counters::add_agents_active(-1);
            }
            if reg.is_metric_enabled(names::AGENTS_COMPLETED) {
                reg.agents_completed.add(1, &[]);
                crate::counters::increment_agents_completed(1);
            }
        }
    }

    /// Record a coordinator error: increment `ragent.errors.total` tagged
    /// with `component` (FR-012).
    pub fn record_error(&self, component: &str) {
        if let Some(reg) = &self.registry {
            if !reg.is_metric_enabled(names::ERRORS_TOTAL) {
                return;
            }
            let attrs = [InstrumentRegistry::attr_component(component)];
            let resolved = reg.resolve_attrs(names::ERRORS_TOTAL, &attrs);
            reg.errors_total.add(1, &resolved);
            crate::counters::increment_errors_total(1);
        }
    }

    /// Record a coordinator timeout: increment `ragent.timeouts.total`
    /// (FR-012).
    pub fn record_timeout(&self) {
        if let Some(reg) = &self.registry {
            if !reg.is_metric_enabled(names::TIMEOUTS_TOTAL) {
                return;
            }
            reg.timeouts_total.add(1, &[]);
            crate::counters::increment_timeouts_total(1);
        }
    }
}

#[cfg(not(feature = "telemetry"))]
impl CoordinatorRecorder {
    /// Create a recorder (no-op when the feature is off).
    #[must_use]
    pub fn from_subsystem(_sub: &crate::TelemetrySubsystem) -> Self {
        Self
    }

    /// Create a disabled recorder.
    #[must_use]
    pub fn disabled() -> Self {
        Self
    }

    /// Returns `false` when the feature is off.
    #[must_use]
    pub fn is_enabled(&self) -> bool {
        false
    }

    /// No-op: record a sub-agent spawn.
    pub fn record_agent_spawn(&self) {}

    /// No-op: record a sub-agent completion.
    pub fn record_agent_complete(&self) {}

    /// No-op: record a coordinator error.
    pub fn record_error(&self, _component: &str) {}

    /// No-op: record a coordinator timeout.
    pub fn record_timeout(&self) {}
}

impl Default for CoordinatorRecorder {
    fn default() -> Self {
        Self::disabled()
    }
}

// -- Permission recorder --------------------------------------------------

/// Recorder for permission system metrics (T-016, FR-016).
///
/// Wraps an [`InstrumentRegistry`] and provides high-level methods that
/// construct the correct `tool.name` attribute and call the appropriate
/// OTEL instruments when a permission request is resolved.
///
/// When telemetry is disabled, the recorder holds `None` and all methods
/// are zero-overhead no-ops (FR-022, NFR-002). When the `telemetry` Cargo
/// feature is off, [`PermissionRecorder`] is a zero-sized no-op.
///
/// # Metrics recorded (FR-016)
///
/// | Method | Metric | Requirement |
/// |--------|--------|-------------|
/// | [`record_approved`](PermissionRecorder::record_approved) | `ragent.permission.approved` | FR-016 |
/// | [`record_denied`](PermissionRecorder::record_denied) | `ragent.permission.denied` | FR-016 |
#[cfg(feature = "telemetry")]
#[derive(Clone)]
pub struct PermissionRecorder {
    registry: Option<InstrumentRegistry>,
}

/// No-op permission recorder used when the `telemetry` feature is off.
#[cfg(not(feature = "telemetry"))]
#[derive(Clone, Copy)]
pub struct PermissionRecorder;

#[cfg(feature = "telemetry")]
impl PermissionRecorder {
    /// Create a recorder from a live [`InstrumentRegistry`].
    #[must_use]
    pub const fn new(registry: InstrumentRegistry) -> Self {
        Self {
            registry: Some(registry),
        }
    }

    /// Create a recorder from a [`TelemetrySubsystem`](crate::TelemetrySubsystem).
    ///
    /// If the subsystem is enabled and the `telemetry` feature is active,
    /// the recorder holds a live `InstrumentRegistry`. Otherwise, it holds
    /// `None` and all recording methods are no-ops.
    #[must_use]
    pub fn from_subsystem(sub: &crate::TelemetrySubsystem) -> Self {
        Self {
            registry: sub.instruments(),
        }
    }

    /// Create a disabled recorder (all methods are no-ops).
    #[must_use]
    pub const fn disabled() -> Self {
        Self { registry: None }
    }

    /// Returns `true` if this recorder has live instruments.
    #[must_use]
    pub const fn is_enabled(&self) -> bool {
        self.registry.is_some()
    }

    /// Record a permission approval: increment `ragent.permission.approved`
    /// tagged with `tool.name` (FR-016).
    ///
    /// Short-circuits when the metric is disabled via
    /// `telemetry.otel.metrics` (FR-027).
    pub fn record_approved(&self, tool_name: &str) {
        if let Some(reg) = &self.registry {
            if !reg.is_metric_enabled(names::PERMISSION_APPROVED) {
                return;
            }
            let attrs = [InstrumentRegistry::attr_tool(tool_name)];
            let resolved = reg.resolve_attrs(names::PERMISSION_APPROVED, &attrs);
            reg.permission_approved.add(1, &resolved);
            crate::counters::increment_permission_approved(1);
        }
    }

    /// Record a permission denial: increment `ragent.permission.denied`
    /// tagged with `tool.name` (FR-016).
    ///
    /// Short-circuits when the metric is disabled via
    /// `telemetry.otel.metrics` (FR-027).
    pub fn record_denied(&self, tool_name: &str) {
        if let Some(reg) = &self.registry {
            if !reg.is_metric_enabled(names::PERMISSION_DENIED) {
                return;
            }
            let attrs = [InstrumentRegistry::attr_tool(tool_name)];
            let resolved = reg.resolve_attrs(names::PERMISSION_DENIED, &attrs);
            reg.permission_denied.add(1, &resolved);
            crate::counters::increment_permission_denied(1);
        }
    }
}

#[cfg(not(feature = "telemetry"))]
impl PermissionRecorder {
    /// Create a recorder (no-op when the feature is off).
    #[must_use]
    pub fn from_subsystem(_sub: &crate::TelemetrySubsystem) -> Self {
        Self
    }

    /// Create a disabled recorder.
    #[must_use]
    pub fn disabled() -> Self {
        Self
    }

    /// Returns `false` when the feature is off.
    #[must_use]
    pub fn is_enabled(&self) -> bool {
        false
    }

    /// No-op: record a permission approval.
    pub fn record_approved(&self, _tool_name: &str) {}

    /// No-op: record a permission denial.
    pub fn record_denied(&self, _tool_name: &str) {}
}

impl Default for PermissionRecorder {
    fn default() -> Self {
        Self::disabled()
    }
}

// -- Compression recorder --------------------------------------------------

/// Recorder for context compression pipeline metrics (T-017, FR-017).
///
/// Wraps an [`InstrumentRegistry`] and provides high-level methods that
/// call the appropriate OTEL instruments when the compression pipeline runs.
///
/// When telemetry is disabled, the recorder holds `None` and all methods
/// are zero-overhead no-ops (FR-022, NFR-002). When the `telemetry` Cargo
/// feature is off, [`CompressionRecorder`] is a zero-sized no-op.
///
/// # Metrics recorded (FR-017)
///
/// | Method | Metric | Requirement |
/// |--------|--------|-------------|
/// | [`record_compression`](CompressionRecorder::record_compression) | `ragent.context.compressions`, `ragent.context.compression_ratio` | FR-017 |
#[cfg(feature = "telemetry")]
#[derive(Clone)]
pub struct CompressionRecorder {
    registry: Option<InstrumentRegistry>,
}

/// No-op compression recorder used when the `telemetry` feature is off.
#[cfg(not(feature = "telemetry"))]
#[derive(Clone, Copy)]
pub struct CompressionRecorder;

#[cfg(feature = "telemetry")]
impl CompressionRecorder {
    /// Create a recorder from a live [`InstrumentRegistry`].
    #[must_use]
    pub const fn new(registry: InstrumentRegistry) -> Self {
        Self {
            registry: Some(registry),
        }
    }

    /// Create a recorder from a [`TelemetrySubsystem`](crate::TelemetrySubsystem).
    ///
    /// If the subsystem is enabled and the `telemetry` feature is active,
    /// the recorder holds a live `InstrumentRegistry`. Otherwise, it holds
    /// `None` and all recording methods are no-ops.
    #[must_use]
    pub fn from_subsystem(sub: &crate::TelemetrySubsystem) -> Self {
        Self {
            registry: sub.instruments(),
        }
    }

    /// Create a disabled recorder (all methods are no-ops).
    #[must_use]
    pub const fn disabled() -> Self {
        Self { registry: None }
    }

    /// Returns `true` if this recorder has live instruments.
    #[must_use]
    pub const fn is_enabled(&self) -> bool {
        self.registry.is_some()
    }

    /// Record a compression pipeline run: increment the
    /// `ragent.context.compressions` counter and observe the before/after
    /// ratio in the `ragent.context.compression_ratio` histogram (FR-017).
    ///
    /// Call this every time the compression pipeline runs, regardless of
    /// whether it actually reduced the token count. When no reduction
    /// occurs the ratio will be approximately `1.0`.
    pub fn record_compression(&self, original_tokens: usize, compressed_tokens: usize, ratio: f64) {
        if let Some(reg) = &self.registry {
            // FR-027 (ANTIPAT HIGH-4): per-metric toggles are honoured.
            if reg.is_metric_enabled(names::CONTEXT_COMPRESSIONS) {
                reg.context_compressions.add(1, &[]);
                crate::counters::increment_context_compressions(1);
            }
            if reg.is_metric_enabled(names::CONTEXT_COMPRESSION_RATIO) {
                reg.context_compression_ratio.record(ratio, &[]);
                crate::counters::set_context_compression_ratio_last(ratio);
            }
            // Keep original/compressed token counts in a debug trace for
            // troubleshooting; they are not exported as metric attributes
            // (FR-034 sensitive-data guard will enforce this in T-022).
            tracing::debug!(original_tokens, compressed_tokens, "compression applied");
        }
    }
}

#[cfg(not(feature = "telemetry"))]
impl CompressionRecorder {
    /// Create a recorder (no-op when the feature is off).
    #[must_use]
    pub fn from_subsystem(_sub: &crate::TelemetrySubsystem) -> Self {
        Self
    }

    /// Create a disabled recorder.
    #[must_use]
    pub fn disabled() -> Self {
        Self
    }

    /// Returns `false` when the feature is off.
    #[must_use]
    pub fn is_enabled(&self) -> bool {
        false
    }

    /// No-op: record a compression pipeline run.
    pub fn record_compression(
        &self,
        _original_tokens: usize,
        _compressed_tokens: usize,
        _ratio: f64,
    ) {
    }
}

impl Default for CompressionRecorder {
    fn default() -> Self {
        Self::disabled()
    }
}

// -- Snapshot recorder -----------------------------------------------------

/// Recorder for snapshot undo/restore metrics (T-027, FR-029).
///
/// Wraps an [`InstrumentRegistry`] and provides a high-level method that
/// increments the `ragent.snapshot.restores` counter whenever the snapshot
/// subsystem restores a captured file snapshot.
///
/// When telemetry is disabled, the recorder holds `None` and all methods
/// are zero-overhead no-ops (FR-022, NFR-002). When the `telemetry` Cargo
/// feature is off, [`SnapshotRecorder`] is a zero-sized no-op.
///
/// # Metrics recorded (FR-029)
///
/// | Method | Metric | Requirement |
/// |--------|--------|-------------|
/// | [`record_restore`](SnapshotRecorder::record_restore) | `ragent.snapshot.restores` | FR-029 |
#[cfg(feature = "telemetry")]
#[derive(Clone)]
pub struct SnapshotRecorder {
    registry: Option<InstrumentRegistry>,
}

/// No-op snapshot recorder used when the `telemetry` feature is off.
#[cfg(not(feature = "telemetry"))]
#[derive(Clone, Copy)]
pub struct SnapshotRecorder;

#[cfg(feature = "telemetry")]
impl SnapshotRecorder {
    /// Create a recorder from a live [`InstrumentRegistry`].
    #[must_use]
    pub const fn new(registry: InstrumentRegistry) -> Self {
        Self {
            registry: Some(registry),
        }
    }

    /// Create a recorder from a [`TelemetrySubsystem`](crate::TelemetrySubsystem).
    ///
    /// If the subsystem is enabled and the `telemetry` feature is active,
    /// the recorder holds a live `InstrumentRegistry`. Otherwise, it holds
    /// `None` and all recording methods are no-ops.
    #[must_use]
    pub fn from_subsystem(sub: &crate::TelemetrySubsystem) -> Self {
        Self {
            registry: sub.instruments(),
        }
    }

    /// Create a disabled recorder (all methods are no-ops).
    #[must_use]
    pub const fn disabled() -> Self {
        Self { registry: None }
    }

    /// Returns `true` if this recorder has live instruments.
    #[must_use]
    pub const fn is_enabled(&self) -> bool {
        self.registry.is_some()
    }

    /// Record a snapshot restore: increment `ragent.snapshot.restores`
    /// (FR-029).
    ///
    /// Call this every time `ragent_storage::snapshot::restore_snapshot`
    /// (or the equivalent storage-layer restore) successfully writes a captured
    /// snapshot back to disk. The counter has no attributes, so it cannot leak
    /// file paths or contents (FR-034).
    ///
    /// Short-circuits when the metric is disabled via
    /// `telemetry.otel.metrics` (FR-027).
    pub fn record_restore(&self) {
        if let Some(reg) = &self.registry {
            if !reg.is_metric_enabled(names::SNAPSHOT_RESTORES) {
                return;
            }
            reg.snapshot_restores.add(1, &[]);
            crate::counters::increment_snapshot_restores(1);
        }
    }
}

#[cfg(not(feature = "telemetry"))]
impl SnapshotRecorder {
    /// Create a recorder (no-op when the feature is off).
    #[must_use]
    pub fn from_subsystem(_sub: &crate::TelemetrySubsystem) -> Self {
        Self
    }

    /// Create a disabled recorder.
    #[must_use]
    pub fn disabled() -> Self {
        Self
    }

    /// Returns `false` when the feature is off.
    #[must_use]
    pub fn is_enabled(&self) -> bool {
        false
    }

    /// No-op: record a snapshot restore.
    pub fn record_restore(&self) {}
}

impl Default for SnapshotRecorder {
    fn default() -> Self {
        Self::disabled()
    }
}

// -- Tests -----------------------------------------------------------------

#[cfg(all(test, feature = "telemetry"))]
#[path = "../tests/inline/recorder_tests.rs"]
mod tests;

#[cfg(all(test, not(feature = "telemetry")))]
#[path = "../tests/inline/recorder_tests.rs"]
mod tests;
