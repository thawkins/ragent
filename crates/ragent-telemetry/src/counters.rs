//! In-memory snapshot of telemetry counter, gauge, and histogram values.
//!
//! OpenTelemetry instruments do not expose their current aggregate values
//! through the public API; values are accumulated inside the SDK and exported
//! asynchronously. This module maintains a cheap, lock-free mirror of every
//! counter, up/down counter, gauge, and last-recorded histogram value so that
//! the TUI and other diagnostic surfaces can show live numbers without needing
//! access to the active [`TelemetrySubsystem`] or an exported metric batch.
//!
//! The snapshot is updated by the recorder methods in [`crate::recorder`] and
//! by direct calls to the typed helpers below. Reads use `Relaxed` ordering and
//! are intended for human-readable diagnostics only.

use std::sync::atomic::{AtomicI64, AtomicU64, Ordering};

/// A lock-free floating-point value stored as raw bits.
#[derive(Debug, Default)]
pub struct AtomicF64 {
    inner: AtomicU64,
}

impl AtomicF64 {
    /// Store a new `f64` value.
    pub fn store(&self, value: f64) {
        self.inner.store(value.to_bits(), Ordering::Relaxed);
    }

    /// Load the current `f64` value.
    #[must_use]
    pub fn load(&self) -> f64 {
        f64::from_bits(self.inner.load(Ordering::Relaxed))
    }

    /// Add `value` atomically using a compare-and-swap loop.
    pub fn fetch_add(&self, value: f64) {
        loop {
            let current = self.load();
            let new = current + value;
            let current_bits = current.to_bits();
            if self
                .inner
                .compare_exchange_weak(
                    current_bits,
                    new.to_bits(),
                    Ordering::Relaxed,
                    Ordering::Relaxed,
                )
                .is_ok()
            {
                break;
            }
        }
    }
}

/// Live snapshot of all counter-like telemetry values.
///
/// Each field is updated by the corresponding recorder method. Histograms store
/// the last observed value so the counters display can still surface a number.
#[derive(Debug, Default)]
pub struct CounterSnapshot {
    // ── Usage metrics ────────────────────────────────────────────────────
    /// `ragent.llm.requests`
    pub llm_requests: AtomicU64,
    /// `ragent.sessions.active`
    pub sessions_active: AtomicI64,
    /// `ragent.sessions.total`
    pub sessions_total: AtomicU64,
    /// `ragent.messages.user`
    pub messages_user: AtomicU64,
    /// `ragent.tool.invocations`
    pub tool_invocations: AtomicU64,
    /// `ragent.agents.active`
    pub agents_active: AtomicI64,
    /// `ragent.agents.completed`
    pub agents_completed: AtomicU64,
    /// `ragent.subagent.spawns`
    pub subagent_spawns: AtomicU64,
    /// `ragent.team.members`
    pub team_members: AtomicI64,

    // ── Cost metrics ─────────────────────────────────────────────────────
    /// `ragent.tokens.input`
    pub tokens_input: AtomicU64,
    /// `ragent.tokens.output`
    pub tokens_output: AtomicU64,
    /// `ragent.tokens.cache_read`
    pub tokens_cache_read: AtomicU64,
    /// `ragent.tokens.cache_write`
    pub tokens_cache_write: AtomicU64,
    /// `ragent.cost.estimated`
    pub cost_estimated: AtomicF64,

    // ── Effectiveness metrics ────────────────────────────────────────────
    /// `ragent.errors.total`
    pub errors_total: AtomicU64,
    /// `ragent.timeouts.total`
    pub timeouts_total: AtomicU64,
    /// `ragent.permission.denied`
    pub permission_denied: AtomicU64,
    /// `ragent.permission.approved`
    pub permission_approved: AtomicU64,
    /// `ragent.context.compressions`
    pub context_compressions: AtomicU64,
    /// `ragent.task.completions`
    pub task_completions: AtomicU64,
    /// `ragent.retries.llm`
    pub retries_llm: AtomicU64,
    /// `ragent.snapshot.restores`
    pub snapshot_restores: AtomicU64,

    // ── Gauges ─────────────────────────────────────────────────────────────
    /// `ragent.rate_limit.requests_pct`
    pub rate_limit_requests_pct: AtomicF64,
    /// `ragent.rate_limit.tokens_pct`
    pub rate_limit_tokens_pct: AtomicF64,

    // ── Histogram last-recorded values ────────────────────────────────────
    /// `ragent.llm.duration`
    pub llm_duration_last: AtomicF64,
    /// `ragent.llm.time_to_first_token`
    pub llm_ttft_last: AtomicF64,
    /// `ragent.tool.duration`
    pub tool_duration_last: AtomicF64,
    /// `ragent.agent_loop.duration`
    pub agent_loop_duration_last: AtomicF64,
    /// `ragent.agent_loop.iterations`
    pub agent_loop_iterations_last: AtomicU64,
    /// `ragent.session.duration`
    pub session_duration_last: AtomicF64,
    /// `ragent.tool.permission_wait`
    pub tool_permission_wait_last: AtomicF64,
    /// `ragent.cost.session`
    pub cost_session_last: AtomicF64,
    /// `ragent.context.compression_ratio`
    pub context_compression_ratio_last: AtomicF64,
    /// `ragent.tool.calls_per_session`
    pub tool_calls_per_session_last: AtomicU64,
}

impl CounterSnapshot {
    /// Build an empty, `const`-compatible snapshot for the global static.
    const fn new_const() -> Self {
        Self {
            llm_requests: AtomicU64::new(0),
            sessions_active: AtomicI64::new(0),
            sessions_total: AtomicU64::new(0),
            messages_user: AtomicU64::new(0),
            tool_invocations: AtomicU64::new(0),
            agents_active: AtomicI64::new(0),
            agents_completed: AtomicU64::new(0),
            subagent_spawns: AtomicU64::new(0),
            team_members: AtomicI64::new(0),
            tokens_input: AtomicU64::new(0),
            tokens_output: AtomicU64::new(0),
            tokens_cache_read: AtomicU64::new(0),
            tokens_cache_write: AtomicU64::new(0),
            cost_estimated: AtomicF64::new_const(),
            errors_total: AtomicU64::new(0),
            timeouts_total: AtomicU64::new(0),
            permission_denied: AtomicU64::new(0),
            permission_approved: AtomicU64::new(0),
            context_compressions: AtomicU64::new(0),
            task_completions: AtomicU64::new(0),
            retries_llm: AtomicU64::new(0),
            snapshot_restores: AtomicU64::new(0),
            rate_limit_requests_pct: AtomicF64::new_const(),
            rate_limit_tokens_pct: AtomicF64::new_const(),
            llm_duration_last: AtomicF64::new_const(),
            llm_ttft_last: AtomicF64::new_const(),
            tool_duration_last: AtomicF64::new_const(),
            agent_loop_duration_last: AtomicF64::new_const(),
            agent_loop_iterations_last: AtomicU64::new(0),
            session_duration_last: AtomicF64::new_const(),
            tool_permission_wait_last: AtomicF64::new_const(),
            cost_session_last: AtomicF64::new_const(),
            context_compression_ratio_last: AtomicF64::new_const(),
            tool_calls_per_session_last: AtomicU64::new(0),
        }
    }
}

impl AtomicF64 {
    const fn new_const() -> Self {
        Self {
            inner: AtomicU64::new(0),
        }
    }
}

/// Global in-memory telemetry snapshot.
///
/// Updates are lock-free and safe to call from any thread. The snapshot is
/// independent of the OTEL export pipeline: it is useful for diagnostics even
/// when the `telemetry` Cargo feature is disabled.
static SNAPSHOT: CounterSnapshot = CounterSnapshot::new_const();

/// Copy of the current snapshot, suitable for display or serialization.
#[derive(Debug, Clone, Copy, PartialEq)]
#[non_exhaustive]
pub struct CounterValues {
    /// `ragent.llm.requests`
    pub llm_requests: u64,
    /// `ragent.sessions.active`
    pub sessions_active: i64,
    /// `ragent.sessions.total`
    pub sessions_total: u64,
    /// `ragent.messages.user`
    pub messages_user: u64,
    /// `ragent.tool.invocations`
    pub tool_invocations: u64,
    /// `ragent.agents.active`
    pub agents_active: i64,
    /// `ragent.agents.completed`
    pub agents_completed: u64,
    /// `ragent.subagent.spawns`
    pub subagent_spawns: u64,
    /// `ragent.team.members`
    pub team_members: i64,
    /// `ragent.tokens.input`
    pub tokens_input: u64,
    /// `ragent.tokens.output`
    pub tokens_output: u64,
    /// `ragent.tokens.cache_read`
    pub tokens_cache_read: u64,
    /// `ragent.tokens.cache_write`
    pub tokens_cache_write: u64,
    /// `ragent.cost.estimated`
    pub cost_estimated: f64,
    /// `ragent.errors.total`
    pub errors_total: u64,
    /// `ragent.timeouts.total`
    pub timeouts_total: u64,
    /// `ragent.permission.denied`
    pub permission_denied: u64,
    /// `ragent.permission.approved`
    pub permission_approved: u64,
    /// `ragent.context.compressions`
    pub context_compressions: u64,
    /// `ragent.task.completions`
    pub task_completions: u64,
    /// `ragent.retries.llm`
    pub retries_llm: u64,
    /// `ragent.snapshot.restores`
    pub snapshot_restores: u64,
    /// `ragent.rate_limit.requests_pct`
    pub rate_limit_requests_pct: f64,
    /// `ragent.rate_limit.tokens_pct`
    pub rate_limit_tokens_pct: f64,
    /// `ragent.llm.duration`
    pub llm_duration_last: f64,
    /// `ragent.llm.time_to_first_token`
    pub llm_ttft_last: f64,
    /// `ragent.tool.duration`
    pub tool_duration_last: f64,
    /// `ragent.agent_loop.duration`
    pub agent_loop_duration_last: f64,
    /// `ragent.agent_loop.iterations`
    pub agent_loop_iterations_last: u64,
    /// `ragent.session.duration`
    pub session_duration_last: f64,
    /// `ragent.tool.permission_wait`
    pub tool_permission_wait_last: f64,
    /// `ragent.cost.session`
    pub cost_session_last: f64,
    /// `ragent.context.compression_ratio`
    pub context_compression_ratio_last: f64,
    /// `ragent.tool.calls_per_session`
    pub tool_calls_per_session_last: u64,
}

/// Return the current in-memory counter/gauge/histogram snapshot.
#[must_use]
pub fn current_values() -> CounterValues {
    CounterValues {
        llm_requests: SNAPSHOT.llm_requests.load(Ordering::Relaxed),
        sessions_active: SNAPSHOT.sessions_active.load(Ordering::Relaxed),
        sessions_total: SNAPSHOT.sessions_total.load(Ordering::Relaxed),
        messages_user: SNAPSHOT.messages_user.load(Ordering::Relaxed),
        tool_invocations: SNAPSHOT.tool_invocations.load(Ordering::Relaxed),
        agents_active: SNAPSHOT.agents_active.load(Ordering::Relaxed),
        agents_completed: SNAPSHOT.agents_completed.load(Ordering::Relaxed),
        subagent_spawns: SNAPSHOT.subagent_spawns.load(Ordering::Relaxed),
        team_members: SNAPSHOT.team_members.load(Ordering::Relaxed),
        tokens_input: SNAPSHOT.tokens_input.load(Ordering::Relaxed),
        tokens_output: SNAPSHOT.tokens_output.load(Ordering::Relaxed),
        tokens_cache_read: SNAPSHOT.tokens_cache_read.load(Ordering::Relaxed),
        tokens_cache_write: SNAPSHOT.tokens_cache_write.load(Ordering::Relaxed),
        cost_estimated: SNAPSHOT.cost_estimated.load(),
        errors_total: SNAPSHOT.errors_total.load(Ordering::Relaxed),
        timeouts_total: SNAPSHOT.timeouts_total.load(Ordering::Relaxed),
        permission_denied: SNAPSHOT.permission_denied.load(Ordering::Relaxed),
        permission_approved: SNAPSHOT.permission_approved.load(Ordering::Relaxed),
        context_compressions: SNAPSHOT.context_compressions.load(Ordering::Relaxed),
        task_completions: SNAPSHOT.task_completions.load(Ordering::Relaxed),
        retries_llm: SNAPSHOT.retries_llm.load(Ordering::Relaxed),
        snapshot_restores: SNAPSHOT.snapshot_restores.load(Ordering::Relaxed),
        rate_limit_requests_pct: SNAPSHOT.rate_limit_requests_pct.load(),
        rate_limit_tokens_pct: SNAPSHOT.rate_limit_tokens_pct.load(),
        llm_duration_last: SNAPSHOT.llm_duration_last.load(),
        llm_ttft_last: SNAPSHOT.llm_ttft_last.load(),
        tool_duration_last: SNAPSHOT.tool_duration_last.load(),
        agent_loop_duration_last: SNAPSHOT.agent_loop_duration_last.load(),
        agent_loop_iterations_last: SNAPSHOT.agent_loop_iterations_last.load(Ordering::Relaxed),
        session_duration_last: SNAPSHOT.session_duration_last.load(),
        tool_permission_wait_last: SNAPSHOT.tool_permission_wait_last.load(),
        cost_session_last: SNAPSHOT.cost_session_last.load(),
        context_compression_ratio_last: SNAPSHOT.context_compression_ratio_last.load(),
        tool_calls_per_session_last: SNAPSHOT.tool_calls_per_session_last.load(Ordering::Relaxed),
    }
}

// ── Typed update helpers used by recorders ────────────────────────────────

/// Increment `ragent.llm.requests`.
pub fn increment_llm_requests(delta: u64) {
    SNAPSHOT.llm_requests.fetch_add(delta, Ordering::Relaxed);
}

/// Increment `ragent.sessions.total`.
pub fn increment_sessions_total(delta: u64) {
    SNAPSHOT.sessions_total.fetch_add(delta, Ordering::Relaxed);
}

/// Increment `ragent.messages.user`.
pub fn increment_messages_user(delta: u64) {
    SNAPSHOT.messages_user.fetch_add(delta, Ordering::Relaxed);
}

/// Increment `ragent.tool.invocations`.
pub fn increment_tool_invocations(delta: u64) {
    SNAPSHOT
        .tool_invocations
        .fetch_add(delta, Ordering::Relaxed);
}

/// Increment `ragent.agents.completed`.
pub fn increment_agents_completed(delta: u64) {
    SNAPSHOT
        .agents_completed
        .fetch_add(delta, Ordering::Relaxed);
}

/// Increment `ragent.subagent.spawns`.
pub fn increment_subagent_spawns(delta: u64) {
    SNAPSHOT.subagent_spawns.fetch_add(delta, Ordering::Relaxed);
}

/// Add to `ragent.sessions.active`.
pub fn add_sessions_active(delta: i64) {
    SNAPSHOT.sessions_active.fetch_add(delta, Ordering::Relaxed);
}

/// Add to `ragent.agents.active`.
pub fn add_agents_active(delta: i64) {
    SNAPSHOT.agents_active.fetch_add(delta, Ordering::Relaxed);
}

/// Set `ragent.team.members`.
pub fn set_team_members(value: i64) {
    SNAPSHOT.team_members.store(value, Ordering::Relaxed);
}

/// Increment `ragent.tokens.input`.
pub fn increment_tokens_input(delta: u64) {
    SNAPSHOT.tokens_input.fetch_add(delta, Ordering::Relaxed);
}

/// Increment `ragent.tokens.output`.
pub fn increment_tokens_output(delta: u64) {
    SNAPSHOT.tokens_output.fetch_add(delta, Ordering::Relaxed);
}

/// Increment `ragent.tokens.cache_read`.
pub fn increment_tokens_cache_read(delta: u64) {
    SNAPSHOT
        .tokens_cache_read
        .fetch_add(delta, Ordering::Relaxed);
}

/// Increment `ragent.tokens.cache_write`.
pub fn increment_tokens_cache_write(delta: u64) {
    SNAPSHOT
        .tokens_cache_write
        .fetch_add(delta, Ordering::Relaxed);
}

/// Increment `ragent.cost.estimated`.
pub fn increment_cost_estimated(delta: f64) {
    SNAPSHOT.cost_estimated.fetch_add(delta);
}

/// Increment `ragent.errors.total`.
pub fn increment_errors_total(delta: u64) {
    SNAPSHOT.errors_total.fetch_add(delta, Ordering::Relaxed);
}

/// Increment `ragent.timeouts.total`.
pub fn increment_timeouts_total(delta: u64) {
    SNAPSHOT.timeouts_total.fetch_add(delta, Ordering::Relaxed);
}

/// Increment `ragent.permission.denied`.
pub fn increment_permission_denied(delta: u64) {
    SNAPSHOT
        .permission_denied
        .fetch_add(delta, Ordering::Relaxed);
}

/// Increment `ragent.permission.approved`.
pub fn increment_permission_approved(delta: u64) {
    SNAPSHOT
        .permission_approved
        .fetch_add(delta, Ordering::Relaxed);
}

/// Increment `ragent.context.compressions`.
pub fn increment_context_compressions(delta: u64) {
    SNAPSHOT
        .context_compressions
        .fetch_add(delta, Ordering::Relaxed);
}

/// Increment `ragent.task.completions`.
pub fn increment_task_completions(delta: u64) {
    SNAPSHOT
        .task_completions
        .fetch_add(delta, Ordering::Relaxed);
}

/// Increment `ragent.retries.llm`.
pub fn increment_retries_llm(delta: u64) {
    SNAPSHOT.retries_llm.fetch_add(delta, Ordering::Relaxed);
}

/// Increment `ragent.snapshot.restores`.
pub fn increment_snapshot_restores(delta: u64) {
    SNAPSHOT
        .snapshot_restores
        .fetch_add(delta, Ordering::Relaxed);
}

/// Set `ragent.rate_limit.requests_pct`.
pub fn set_rate_limit_requests_pct(value: f64) {
    SNAPSHOT.rate_limit_requests_pct.store(value);
}

/// Set `ragent.rate_limit.tokens_pct`.
pub fn set_rate_limit_tokens_pct(value: f64) {
    SNAPSHOT.rate_limit_tokens_pct.store(value);
}

/// Record the last observed `ragent.llm.duration`.
pub fn set_llm_duration_last(value: f64) {
    SNAPSHOT.llm_duration_last.store(value);
}

/// Record the last observed `ragent.llm.time_to_first_token`.
pub fn set_llm_ttft_last(value: f64) {
    SNAPSHOT.llm_ttft_last.store(value);
}

/// Record the last observed `ragent.tool.duration`.
pub fn set_tool_duration_last(value: f64) {
    SNAPSHOT.tool_duration_last.store(value);
}

/// Record the last observed `ragent.agent_loop.duration`.
pub fn set_agent_loop_duration_last(value: f64) {
    SNAPSHOT.agent_loop_duration_last.store(value);
}

/// Record the last observed `ragent.agent_loop.iterations`.
pub fn set_agent_loop_iterations_last(value: u64) {
    SNAPSHOT
        .agent_loop_iterations_last
        .store(value, Ordering::Relaxed);
}

/// Record the last observed `ragent.session.duration`.
pub fn set_session_duration_last(value: f64) {
    SNAPSHOT.session_duration_last.store(value);
}

/// Record the last observed `ragent.tool.permission_wait`.
pub fn set_tool_permission_wait_last(value: f64) {
    SNAPSHOT.tool_permission_wait_last.store(value);
}

/// Record the last observed `ragent.cost.session`.
pub fn set_cost_session_last(value: f64) {
    SNAPSHOT.cost_session_last.store(value);
}

/// Record the last observed `ragent.context.compression_ratio`.
pub fn set_context_compression_ratio_last(value: f64) {
    SNAPSHOT.context_compression_ratio_last.store(value);
}

/// Record the last observed `ragent.tool.calls_per_session`.
pub fn set_tool_calls_per_session_last(value: u64) {
    SNAPSHOT
        .tool_calls_per_session_last
        .store(value, Ordering::Relaxed);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_atomic_f64_store_load() {
        let a = AtomicF64::default();
        a.store(std::f64::consts::PI);
        assert!((a.load() - std::f64::consts::PI).abs() < 1e-9);
    }

    #[test]
    fn test_atomic_f64_fetch_add() {
        let a = AtomicF64::default();
        a.fetch_add(1.5);
        a.fetch_add(2.5);
        assert!((a.load() - 4.0).abs() < 1e-9);
    }

    #[test]
    fn test_counter_helpers_update_snapshot() {
        increment_llm_requests(2);
        add_sessions_active(1);
        add_sessions_active(-1);
        set_rate_limit_requests_pct(42.0);
        set_llm_duration_last(123.4);

        let values = current_values();
        assert_eq!(values.llm_requests, 2);
        assert_eq!(values.sessions_active, 0);
        assert!((values.rate_limit_requests_pct - 42.0).abs() < 1e-9);
        assert!((values.llm_duration_last - 123.4).abs() < 1e-9);
    }
}

/// Structured representation of the counter/gauge groups used by
/// `/telemetry counters` and the live telemetry side panel. Keeps the chat
/// markdown output and the panel rendering in sync without duplicating metric
/// definitions.
#[derive(Debug, Clone)]
pub struct TelemetryCountersContent {
    /// Usage metrics group: `(metric_name, instrument_type, description, current_value)`.
    pub usage: Vec<(String, String, String, String)>,
    /// Performance metrics group.
    pub performance: Vec<(String, String, String, String)>,
    /// Cost metrics group.
    pub cost: Vec<(String, String, String, String)>,
    /// Effectiveness metrics group.
    pub effectiveness: Vec<(String, String, String, String)>,
    /// Pre-rendered markdown suitable for the chat transcript.
    pub markdown: String,
}

impl TelemetryCountersContent {
    /// Return the pre-rendered markdown for the `/telemetry counters` chat output.
    #[must_use]
    pub fn markdown(&self) -> String {
        self.markdown.clone()
    }
}
