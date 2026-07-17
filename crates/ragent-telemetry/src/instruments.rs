//! OTEL instrument registry — constructs and holds all metrics from the catalog.
//!
//! The [`InstrumentRegistry`] owns a [`Meter`] and all OTEL instruments
//! (counters, histograms, gauges, up/down counters) defined in the otel
//! spec's Metric Catalog (FR-003). When telemetry is enabled, it creates real
//! instruments from the live [`SdkMeterProvider`]; when disabled, it creates
//! no-op instruments from the global [`NoopMeterProvider`] (FR-022, NFR-002).
//!
//! All instruments are named per the OTEL semantic conventions: lowercase,
//! dot-separated, with a `{unit}` suffix where applicable.

#[cfg(feature = "telemetry")]
use opentelemetry::KeyValue;
#[cfg(feature = "telemetry")]
use opentelemetry::metrics::MeterProvider;

#[cfg(feature = "telemetry")]
use crate::cardinality::CardinalityCache;

/// Instrument names from the otel spec Metric Catalog.
///
/// Centralised as constants so the registry and tests reference a single
/// source of truth (FR-003).
#[allow(dead_code)] // Some metrics are used only when instrumented by later tasks
pub mod names {
    // ── Usage metrics ───────────────────────────────────────────────────
    /// Metric name for total LLM requests (`ragent.llm.requests`).
    pub const LLM_REQUESTS: &str = "ragent.llm.requests";
    /// Metric name for currently active sessions (`ragent.sessions.active`).
    pub const SESSIONS_ACTIVE: &str = "ragent.sessions.active";
    /// Metric name for total sessions created (`ragent.sessions.total`).
    pub const SESSIONS_TOTAL: &str = "ragent.sessions.total";
    /// Metric name for user messages submitted (`ragent.messages.user`).
    pub const MESSAGES_USER: &str = "ragent.messages.user";
    /// Metric name for tool invocations (`ragent.tool.invocations`).
    pub const TOOL_INVOCATIONS: &str = "ragent.tool.invocations";
    /// Metric name for currently active sub-agents (`ragent.agents.active`).
    pub const AGENTS_ACTIVE: &str = "ragent.agents.active";
    /// Metric name for completed sub-agents (`ragent.agents.completed`).
    pub const AGENTS_COMPLETED: &str = "ragent.agents.completed";
    /// Metric name for sub-agent spawn events (`ragent.subagent.spawns`).
    pub const SUBAGENT_SPAWNS: &str = "ragent.subagent.spawns";
    /// Metric name for current team members (`ragent.team.members`).
    pub const TEAM_MEMBERS: &str = "ragent.team.members";

    // ── Performance metrics ─────────────────────────────────────────────
    /// Metric name for LLM call duration (`ragent.llm.duration`).
    pub const LLM_DURATION: &str = "ragent.llm.duration";
    /// Metric name for LLM time-to-first-token (`ragent.llm.time_to_first_token`).
    pub const LLM_TTFT: &str = "ragent.llm.time_to_first_token";
    /// Metric name for tool execution duration (`ragent.tool.duration`).
    pub const TOOL_DURATION: &str = "ragent.tool.duration";
    /// Metric name for agent loop duration (`ragent.agent_loop.duration`).
    pub const AGENT_LOOP_DURATION: &str = "ragent.agent_loop.duration";
    /// Metric name for agent loop iterations (`ragent.agent_loop.iterations`).
    pub const AGENT_LOOP_ITERATIONS: &str = "ragent.agent_loop.iterations";
    /// Metric name for session duration (`ragent.session.duration`).
    pub const SESSION_DURATION: &str = "ragent.session.duration";
    /// Metric name for tool permission wait time (`ragent.tool.permission_wait`).
    pub const TOOL_PERMISSION_WAIT: &str = "ragent.tool.permission_wait";

    // ── Cost metrics ────────────────────────────────────────────────────
    /// Metric name for input tokens (`ragent.tokens.input`).
    pub const TOKENS_INPUT: &str = "ragent.tokens.input";
    /// Metric name for output tokens (`ragent.tokens.output`).
    pub const TOKENS_OUTPUT: &str = "ragent.tokens.output";
    /// Metric name for cache-read tokens (`ragent.tokens.cache_read`).
    pub const TOKENS_CACHE_READ: &str = "ragent.tokens.cache_read";
    /// Metric name for cache-write tokens (`ragent.tokens.cache_write`).
    pub const TOKENS_CACHE_WRITE: &str = "ragent.tokens.cache_write";
    /// Metric name for estimated cost (`ragent.cost.estimated`).
    pub const COST_ESTIMATED: &str = "ragent.cost.estimated";
    /// Metric name for per-session cost (`ragent.cost.session`).
    pub const COST_SESSION: &str = "ragent.cost.session";
    /// Metric name for request rate-limit percentage (`ragent.rate_limit.requests_pct`).
    pub const RATE_LIMIT_REQUESTS_PCT: &str = "ragent.rate_limit.requests_pct";
    /// Metric name for token rate-limit percentage (`ragent.rate_limit.tokens_pct`).
    pub const RATE_LIMIT_TOKENS_PCT: &str = "ragent.rate_limit.tokens_pct";

    // ── Effectiveness metrics ─────────────────────────────────────────
    /// Metric name for total errors (`ragent.errors.total`).
    pub const ERRORS_TOTAL: &str = "ragent.errors.total";
    /// Metric name for total timeouts (`ragent.timeouts.total`).
    pub const TIMEOUTS_TOTAL: &str = "ragent.timeouts.total";
    /// Metric name for permission denials (`ragent.permission.denied`).
    pub const PERMISSION_DENIED: &str = "ragent.permission.denied";
    /// Metric name for permission approvals (`ragent.permission.approved`).
    pub const PERMISSION_APPROVED: &str = "ragent.permission.approved";
    /// Metric name for context compression invocations (`ragent.context.compressions`).
    pub const CONTEXT_COMPRESSIONS: &str = "ragent.context.compressions";
    /// Metric name for context compression ratio (`ragent.context.compression_ratio`).
    pub const CONTEXT_COMPRESSION_RATIO: &str = "ragent.context.compression_ratio";
    /// Metric name for tool calls per session (`ragent.tool.calls_per_session`).
    pub const TOOL_CALLS_PER_SESSION: &str = "ragent.tool.calls_per_session";
    /// Metric name for task completions (`ragent.task.completions`).
    pub const TASK_COMPLETIONS: &str = "ragent.task.completions";
    /// Metric name for LLM retries (`ragent.retries.llm`).
    pub const RETRIES_LLM: &str = "ragent.retries.llm";
    /// Metric name for snapshot restores (`ragent.snapshot.restores`).
    pub const SNAPSHOT_RESTORES: &str = "ragent.snapshot.restores";
}

/// A registry of all OTEL instruments defined in the otel spec Metric Catalog.
///
/// When the `telemetry` feature is enabled, instruments are backed by a real
/// [`Meter`] from the live [`SdkMeterProvider`]. When the feature is disabled
/// (or the subsystem is in [`Disabled`](crate::TelemetryState::Disabled) mode),
/// instruments are no-op handles that discard all recorded data (FR-022,
/// NFR-002).
///
/// All instrument handles are cheap to clone; the underlying SDK uses `Arc`
/// internally, so passing clones around has no measurable cost.
#[cfg(feature = "telemetry")]
#[derive(Clone)]
pub struct InstrumentRegistry {
    /// The OpenTelemetry [`Meter`] used to create the instruments below.
    ///
    /// This field is kept primarily to anchor the meter lifetime to the
    /// registry; it is not directly exposed because callers interact with
    /// the typed instrument handles instead.
    #[allow(dead_code)]
    meter: opentelemetry::metrics::Meter,

    /// Cardinality cache shared across all clones (FR-035).
    /// Tracks distinct attribute combinations per metric and collapses
    /// overflow into an `unknown` bucket.
    cardinality: std::sync::Arc<CardinalityCache>,

    /// Per-metric enable/disable toggles keyed by instrument name (FR-027).
    ///
    /// A metric absent from this map is enabled by default. A metric
    /// present and set to `false` is disabled — recording calls to that
    /// instrument are short-circuited by
    /// [`is_metric_enabled`](Self::is_metric_enabled) before they reach the
    /// underlying OTEL instrument, so the metric produces zero exported
    /// data points.
    metric_toggles: std::sync::Arc<std::collections::HashMap<String, bool>>,

    // ── Usage: counters ──��──────────────────────────────────────────────
    /// Total LLM API requests made by the harness (`ragent.llm.requests`).
    pub llm_requests: opentelemetry::metrics::Counter<u64>,
    /// Total number of sessions created (`ragent.sessions.total`).
    pub sessions_total: opentelemetry::metrics::Counter<u64>,
    /// Total user messages submitted (`ragent.messages.user`).
    pub messages_user: opentelemetry::metrics::Counter<u64>,
    /// Total tool invocations, tagged by `tool.name` (`ragent.tool.invocations`).
    pub tool_invocations: opentelemetry::metrics::Counter<u64>,
    /// Total sub-agent completions (`ragent.agents.completed`).
    pub agents_completed: opentelemetry::metrics::Counter<u64>,
    /// Total sub-agent spawn events (`ragent.subagent.spawns`).
    pub subagent_spawns: opentelemetry::metrics::Counter<u64>,

    // ── Usage: up/down counters ────────────────────────────────────────
    /// Currently active sessions (`ragent.sessions.active`).
    pub sessions_active: opentelemetry::metrics::UpDownCounter<i64>,
    /// Currently active sub-agents (`ragent.agents.active`).
    pub agents_active: opentelemetry::metrics::UpDownCounter<i64>,

    // ── Usage: gauges ───────────────────────────────────────────────────
    /// Current number of team members (`ragent.team.members`).
    pub team_members: opentelemetry::metrics::Gauge<i64>,

    // ── Performance: histograms ───────────────────────────────────────
    /// LLM call wall-clock duration in ms, tagged by model and provider (`ragent.llm.duration`).
    pub llm_duration: opentelemetry::metrics::Histogram<f64>,
    /// Time to first token from LLM stream in ms, tagged by model (`ragent.llm.time_to_first_token`).
    pub llm_ttft: opentelemetry::metrics::Histogram<f64>,
    /// Tool execution wall-clock duration in ms, tagged by tool.name (`ragent.tool.duration`).
    pub tool_duration: opentelemetry::metrics::Histogram<f64>,
    /// Agent loop iteration duration in ms (`ragent.agent_loop.duration`).
    pub agent_loop_duration: opentelemetry::metrics::Histogram<f64>,
    /// Number of iterations in a completed agent loop (`ragent.agent_loop.iterations`).
    pub agent_loop_iterations: opentelemetry::metrics::Histogram<u64>,
    /// Session start-to-close wall-clock duration in ms (`ragent.session.duration`).
    pub session_duration: opentelemetry::metrics::Histogram<f64>,
    /// Time spent waiting for user permission on a tool call in ms (`ragent.tool.permission_wait`).
    pub tool_permission_wait: opentelemetry::metrics::Histogram<f64>,

    // ── Cost: counters ────────────────────────────────────────────────
    /// Input/prompt tokens consumed, tagged by model (`ragent.tokens.input`).
    pub tokens_input: opentelemetry::metrics::Counter<u64>,
    /// Output/completion tokens produced, tagged by model (`ragent.tokens.output`).
    pub tokens_output: opentelemetry::metrics::Counter<u64>,
    /// Cache-read tokens, tagged by model (`ragent.tokens.cache_read`).
    pub tokens_cache_read: opentelemetry::metrics::Counter<u64>,
    /// Cache-write tokens, tagged by model (`ragent.tokens.cache_write`).
    pub tokens_cache_write: opentelemetry::metrics::Counter<u64>,
    /// Estimated cost in USD, tagged by model and provider (`ragent.cost.estimated`).
    pub cost_estimated: opentelemetry::metrics::Counter<f64>,

    // ── Cost: histograms ───────────────────────────────────────────────
    /// Total estimated cost per session in USD (`ragent.cost.session`).
    pub cost_session: opentelemetry::metrics::Histogram<f64>,

    // ── Cost: gauges ──────────────────────────────────────────────────
    /// Percentage of request quota consumed, tagged by provider (`ragent.rate_limit.requests_pct`).
    pub rate_limit_requests_pct: opentelemetry::metrics::Gauge<f64>,
    /// Percentage of token quota consumed, tagged by provider (`ragent.rate_limit.tokens_pct`).
    pub rate_limit_tokens_pct: opentelemetry::metrics::Gauge<f64>,

    // ── Effectiveness: counters ─────────────────────────────────────────
    /// Total errors, tagged by component (`ragent.errors.total`).
    pub errors_total: opentelemetry::metrics::Counter<u64>,
    /// Total timeout events (`ragent.timeouts.total`).
    pub timeouts_total: opentelemetry::metrics::Counter<u64>,
    /// Permission denials, tagged by tool.name (`ragent.permission.denied`).
    pub permission_denied: opentelemetry::metrics::Counter<u64>,
    /// Permission approvals, tagged by tool.name (`ragent.permission.approved`).
    pub permission_approved: opentelemetry::metrics::Counter<u64>,
    /// Context compression pipeline invocations (`ragent.context.compressions`).
    pub context_compressions: opentelemetry::metrics::Counter<u64>,
    /// Completed sub-agent and team tasks (`ragent.task.completions`).
    pub task_completions: opentelemetry::metrics::Counter<u64>,
    /// LLM provider retry attempts, tagged by model (`ragent.retries.llm`).
    pub retries_llm: opentelemetry::metrics::Counter<u64>,
    /// Snapshot undo system restores (`ragent.snapshot.restores`).
    pub snapshot_restores: opentelemetry::metrics::Counter<u64>,

    // ── Effectiveness: histograms ─────────────────────────────────────
    /// Context compression before/after token ratio (`ragent.context.compression_ratio`).
    pub context_compression_ratio: opentelemetry::metrics::Histogram<f64>,
    /// Tool call count per session (`ragent.tool.calls_per_session`).
    pub tool_calls_per_session: opentelemetry::metrics::Histogram<u64>,
}

#[cfg(feature = "telemetry")]
impl InstrumentRegistry {
    /// Build the instrument registry from a live [`SdkMeterProvider`].
    ///
    /// Creates a [`Meter`] scoped to `"ragent"` and constructs every
    /// instrument listed in the Metric Catalog with its correct name, unit,
    /// and description (FR-003, FR-004).
    #[must_use]
    pub fn from_provider(provider: &opentelemetry_sdk::metrics::SdkMeterProvider) -> Self {
        let meter = provider.meter("ragent");
        Self::from_meter(meter)
    }

    /// Build the instrument registry from a no-op [`Meter`].
    ///
    /// Used when telemetry is disabled so all instrument calls are cheap
    /// no-ops with zero overhead (FR-022, NFR-002).
    #[must_use]
    pub fn noop() -> Self {
        let provider = opentelemetry::global::meter_provider();
        let meter = provider.meter("ragent");
        Self::from_meter(meter)
    }

    /// Sets the cardinality limit for this registry (FR-035).
    ///
    /// Returns a new registry with a fresh [`CardinalityCache`] using the
    /// given limit. This should be called after `from_provider` or
    /// `from_meter` but before any recording occurs. The default limit is
    /// [`DEFAULT_CARDINALITY_LIMIT`](crate::cardinality::DEFAULT_CARDINALITY_LIMIT)
    /// (1000).
    #[must_use]
    pub fn with_cardinality_limit(mut self, limit: usize) -> Self {
        self.cardinality = std::sync::Arc::new(CardinalityCache::new(limit));
        self
    }

    /// Sets the per-metric enable/disable toggles for this registry (FR-027).
    ///
    /// The map is keyed by instrument name (e.g. `"ragent.llm.requests"`).
    /// A metric absent from the map is enabled by default; a metric present
    /// and set to `false` is disabled. Disabled metrics short-circuit in
    /// [`is_metric_enabled`](Self::is_metric_enabled) before any recording
    /// reaches the underlying OTEL instrument, so the metric produces zero
    /// exported data points and zero cardinality growth.
    ///
    /// This should be called after `from_provider` but before any recording
    /// occurs. The default is an empty map (all metrics enabled).
    #[must_use]
    pub fn with_metric_toggles(mut self, toggles: std::collections::HashMap<String, bool>) -> Self {
        self.metric_toggles = std::sync::Arc::new(toggles);
        self
    }

    /// Returns `true` when the named metric is enabled (FR-027).
    ///
    /// A metric is enabled when it is **absent** from the toggles map or
    /// present and set to `true`. A metric present and set to `false` is
    /// disabled. Recorders call this before touching the underlying OTEL
    /// instrument so disabled metrics produce zero exported data.
    ///
    /// This is a cheap `Arc`-cloned `HashMap` lookup; it never blocks and
    /// never panics.
    #[must_use]
    pub fn is_metric_enabled(&self, metric_name: &str) -> bool {
        match self.metric_toggles.get(metric_name) {
            Some(enabled) => *enabled,
            None => true,
        }
    }

    /// Resolve a set of attributes for the given metric, applying the
    /// cardinality cap (FR-035).
    ///
    /// If the attribute combination is already tracked for this metric,
    /// the original attributes are returned unchanged. If the combination
    /// is new and the per-metric limit has not been reached, the
    /// combination is registered and the original attributes are returned.
    /// If the limit has been reached, all attribute values are replaced
    /// with `"unknown"`.
    ///
    /// This is called by the recorder methods before passing attributes to
    /// the underlying OTEL instruments.
    #[must_use]
    pub fn resolve_attrs(&self, metric_name: &str, attrs: &[KeyValue]) -> Vec<KeyValue> {
        self.cardinality.resolve(metric_name, attrs)
    }

    /// Construct all instruments from the given [`Meter`].
    fn from_meter(meter: opentelemetry::metrics::Meter) -> Self {
        use names::*;

        // The metric toggles map defaults to empty (all metrics enabled).
        // TelemetrySubsystem::instruments() calls with_metric_toggles() to
        // wire the user's telemetry.otel.metrics config (FR-027).
        let metric_toggles = std::sync::Arc::new(std::collections::HashMap::new());

        // ── Usage: counters ────────────────────────────────────────────
        let llm_requests = meter
            .u64_counter(LLM_REQUESTS)
            .with_unit("{request}")
            .with_description("Total LLM API requests made by the harness")
            .build();
        let sessions_total = meter
            .u64_counter(SESSIONS_TOTAL)
            .with_unit("{session}")
            .with_description("Total number of sessions created")
            .build();
        let messages_user = meter
            .u64_counter(MESSAGES_USER)
            .with_unit("{message}")
            .with_description("Total user messages submitted")
            .build();
        let tool_invocations = meter
            .u64_counter(TOOL_INVOCATIONS)
            .with_unit("{invocation}")
            .with_description("Total tool invocations, tagged by tool.name")
            .build();
        let agents_completed = meter
            .u64_counter(AGENTS_COMPLETED)
            .with_unit("{agent}")
            .with_description("Total sub-agent completions")
            .build();
        let subagent_spawns = meter
            .u64_counter(SUBAGENT_SPAWNS)
            .with_unit("{subagent}")
            .with_description("Total sub-agent spawn events")
            .build();

        // ── Usage: up/down counters ────────────────────────────────────
        let sessions_active = meter
            .i64_up_down_counter(SESSIONS_ACTIVE)
            .with_unit("{session}")
            .with_description("Currently active sessions")
            .build();
        let agents_active = meter
            .i64_up_down_counter(AGENTS_ACTIVE)
            .with_unit("{agent}")
            .with_description("Currently active sub-agents")
            .build();

        // ── Usage: gauges ──────────────────────────��────────────────
        let team_members = meter
            .i64_gauge(TEAM_MEMBERS)
            .with_unit("{member}")
            .with_description("Current number of team members")
            .build();

        // ── Performance: histograms ─────────────────────────────────────
        let llm_duration = meter
            .f64_histogram(LLM_DURATION)
            .with_unit("ms")
            .with_description("LLM call wall-clock duration, tagged by model and provider")
            .build();
        let llm_ttft = meter
            .f64_histogram(LLM_TTFT)
            .with_unit("ms")
            .with_description("Time to first token from LLM stream, tagged by model")
            .build();
        let tool_duration = meter
            .f64_histogram(TOOL_DURATION)
            .with_unit("ms")
            .with_description("Tool execution wall-clock duration, tagged by tool.name")
            .build();
        let agent_loop_duration = meter
            .f64_histogram(AGENT_LOOP_DURATION)
            .with_unit("ms")
            .with_description("Agent loop iteration duration")
            .build();
        let agent_loop_iterations = meter
            .u64_histogram(AGENT_LOOP_ITERATIONS)
            .with_unit("{iteration}")
            .with_description("Number of iterations in a completed agent loop")
            .build();
        let session_duration = meter
            .f64_histogram(SESSION_DURATION)
            .with_unit("ms")
            .with_description("Session start-to-close wall-clock duration")
            .build();
        let tool_permission_wait = meter
            .f64_histogram(TOOL_PERMISSION_WAIT)
            .with_unit("ms")
            .with_description("Time spent waiting for user permission on a tool call")
            .build();

        // ── Cost: counters ───────────────────────────────────────────
        let tokens_input = meter
            .u64_counter(TOKENS_INPUT)
            .with_unit("{token}")
            .with_description("Input tokens, tagged by model")
            .build();
        let tokens_output = meter
            .u64_counter(TOKENS_OUTPUT)
            .with_unit("{token}")
            .with_description("Output tokens, tagged by model")
            .build();
        let tokens_cache_read = meter
            .u64_counter(TOKENS_CACHE_READ)
            .with_unit("{token}")
            .with_description("Cache-read tokens, tagged by model")
            .build();
        let tokens_cache_write = meter
            .u64_counter(TOKENS_CACHE_WRITE)
            .with_unit("{token}")
            .with_description("Cache-write tokens, tagged by model")
            .build();
        let cost_estimated = meter
            .f64_counter(COST_ESTIMATED)
            .with_unit("USD")
            .with_description("Estimated cost in USD, tagged by model and provider")
            .build();

        // ── Cost: histograms ───────────────────────────────────────────
        let cost_session = meter
            .f64_histogram(COST_SESSION)
            .with_unit("USD")
            .with_description("Total estimated cost per session")
            .build();

        // ── Cost: gauges ──────────────────────────────────────────────
        let rate_limit_requests_pct = meter
            .f64_gauge(RATE_LIMIT_REQUESTS_PCT)
            .with_unit("%")
            .with_description("Percentage of request quota consumed, tagged by provider")
            .build();
        let rate_limit_tokens_pct = meter
            .f64_gauge(RATE_LIMIT_TOKENS_PCT)
            .with_unit("%")
            .with_description("Percentage of token quota consumed, tagged by provider")
            .build();

        // ── Effectiveness: counters ───────────────────────────────────
        let errors_total = meter
            .u64_counter(ERRORS_TOTAL)
            .with_unit("{error}")
            .with_description("Total errors, tagged by component")
            .build();
        let timeouts_total = meter
            .u64_counter(TIMEOUTS_TOTAL)
            .with_unit("{timeout}")
            .with_description("Total timeout events")
            .build();
        let permission_denied = meter
            .u64_counter(PERMISSION_DENIED)
            .with_unit("{denial}")
            .with_description("Permission denials, tagged by tool.name")
            .build();
        let permission_approved = meter
            .u64_counter(PERMISSION_APPROVED)
            .with_unit("{approval}")
            .with_description("Permission approvals, tagged by tool.name")
            .build();
        let context_compressions = meter
            .u64_counter(CONTEXT_COMPRESSIONS)
            .with_unit("{compression}")
            .with_description("Context compression pipeline invocations")
            .build();
        let task_completions = meter
            .u64_counter(TASK_COMPLETIONS)
            .with_unit("{task}")
            .with_description("Completed sub-agent and team tasks")
            .build();
        let retries_llm = meter
            .u64_counter(RETRIES_LLM)
            .with_unit("{retry}")
            .with_description("LLM provider retry attempts, tagged by model")
            .build();
        let snapshot_restores = meter
            .u64_counter(SNAPSHOT_RESTORES)
            .with_unit("{restore}")
            .with_description("Snapshot undo system restores")
            .build();

        // ── Effectiveness: histograms ───────────────────────────────────
        let context_compression_ratio = meter
            .f64_histogram(CONTEXT_COMPRESSION_RATIO)
            .with_unit("%")
            .with_description("Context compression before/after token ratio")
            .build();
        let tool_calls_per_session = meter
            .u64_histogram(TOOL_CALLS_PER_SESSION)
            .with_unit("{call}")
            .with_description("Tool call count per session")
            .build();

        Self {
            meter,
            cardinality: std::sync::Arc::new(CardinalityCache::default()),
            metric_toggles,
            llm_requests,
            sessions_total,
            messages_user,
            tool_invocations,
            agents_completed,
            subagent_spawns,
            sessions_active,
            agents_active,
            team_members,
            llm_duration,
            llm_ttft,
            tool_duration,
            agent_loop_duration,
            agent_loop_iterations,
            session_duration,
            tool_permission_wait,
            tokens_input,
            tokens_output,
            tokens_cache_read,
            tokens_cache_write,
            cost_estimated,
            cost_session,
            rate_limit_requests_pct,
            rate_limit_tokens_pct,
            errors_total,
            timeouts_total,
            permission_denied,
            permission_approved,
            context_compressions,
            task_completions,
            retries_llm,
            snapshot_restores,
            context_compression_ratio,
            tool_calls_per_session,
        }
    }
    /// Returns the [`KeyValue`] attribute for a tool name, suitable for
    /// tagging instrument calls.
    ///
    /// The value is passed through [`crate::sensitive::sanitize_attr_value`]
    /// so that a caller that accidentally passes a sensitive string (e.g. an
    /// API key or a chunk of file content) as a tool name has it replaced
    /// with `"redacted"` rather than exported as a metric attribute (FR-034).
    #[must_use]
    pub fn attr_tool(name: &str) -> KeyValue {
        KeyValue::new("tool.name", crate::sensitive::sanitize_attr_value(name))
    }

    /// Returns the [`KeyValue`] attribute for a model name.
    ///
    /// Sanitised via [`crate::sensitive::sanitize_attr_value`] (FR-034).
    #[must_use]
    pub fn attr_model(name: &str) -> KeyValue {
        KeyValue::new("model", crate::sensitive::sanitize_attr_value(name))
    }

    /// Returns the [`KeyValue`] attribute for a provider name.
    ///
    /// Sanitised via [`crate::sensitive::sanitize_attr_value`] (FR-034).
    #[must_use]
    pub fn attr_provider(name: &str) -> KeyValue {
        KeyValue::new("provider", crate::sensitive::sanitize_attr_value(name))
    }

    /// Returns the [`KeyValue`] attribute for a component name (used by
    /// `ragent.errors.total`).
    ///
    /// Sanitised via [`crate::sensitive::sanitize_attr_value`] (FR-034).
    #[must_use]
    pub fn attr_component(name: &str) -> KeyValue {
        KeyValue::new("component", crate::sensitive::sanitize_attr_value(name))
    }

    /// Returns the [`KeyValue`] attribute for a session ID (FR-025).
    ///
    /// While `service.name`, `service.version`, and `host.name` are static
    /// resource attributes set at provider construction, `session.id` is
    /// dynamic — it changes per session — so it is attached as a metric
    /// attribute rather than a resource attribute. Callers should include
    /// this in the `attributes` slice when recording metrics within a
    /// specific session context.
    ///
    /// Sanitised via [`crate::sensitive::sanitize_attr_value`] (FR-034).
    #[must_use]
    pub fn attr_session(id: &str) -> KeyValue {
        KeyValue::new("session.id", crate::sensitive::sanitize_attr_value(id))
    }
}

// ── No-op stub when `telemetry` feature is off ─────────���────────────────

/// A no-op instrument registry that discards all recorded metrics.
///
/// This is used when the `telemetry` Cargo feature is not enabled. It
/// provides the same public fields as the feature-gated
/// [`InstrumentRegistry`](crate::InstrumentRegistry) but all methods are
/// no-ops. In practice, when the feature is off, callers should not hold
/// an `InstrumentRegistry` at all — the [`TelemetrySubsystem`] returns
/// `None` from `instruments()`.
#[cfg(not(feature = "telemetry"))]
pub struct NoopInstrumentRegistry;

#[cfg(not(feature = "telemetry"))]
impl NoopInstrumentRegistry {
    /// Create a new no-op instrument registry.
    #[must_use]
    pub fn new() -> Self {
        Self
    }
}

#[cfg(not(feature = "telemetry"))]
impl Default for NoopInstrumentRegistry {
    fn default() -> Self {
        Self::new()
    }
}

// ── Tests ──────────────────────────────────────────────────────────────

#[cfg(all(test, feature = "telemetry"))]
mod tests {
    use super::*;
    use crate::OtelConfig;
    use opentelemetry_sdk::metrics::SdkMeterProvider;
    use opentelemetry_sdk::runtime::Tokio;
    use opentelemetry_sdk::testing::metrics::InMemoryMetricExporter;
    use std::time::Duration;

    fn build_registry() -> InstrumentRegistry {
        let mut config = OtelConfig::default();
        config.enabled = true;
        config.endpoint = "http://localhost:4318".to_string();

        let rt = tokio::runtime::Runtime::new().expect("tokio runtime");
        let sub = rt
            .block_on(async { crate::TelemetrySubsystem::new(config).expect("enabled subsystem") });
        InstrumentRegistry::from_provider(sub.provider().unwrap())
    }

    /// Build an [`InstrumentRegistry`] backed by an [`SdkMeterProvider`] that
    /// uses an [`InMemoryMetricExporter`] (NFR-005). Returns the registry, the
    /// exporter, the provider, and the tokio runtime so callers can flush and
    /// inspect the exported metric data.
    fn build_registry_with_exporter() -> (
        InstrumentRegistry,
        InMemoryMetricExporter,
        SdkMeterProvider,
        tokio::runtime::Runtime,
    ) {
        let rt = tokio::runtime::Runtime::new().expect("tokio runtime");
        let exporter = InMemoryMetricExporter::default();
        let exporter_clone = exporter.clone();
        let provider = rt.block_on(async {
            let reader = opentelemetry_sdk::metrics::PeriodicReader::builder(exporter_clone, Tokio)
                .with_interval(Duration::from_secs(3600))
                .build();
            SdkMeterProvider::builder().with_reader(reader).build()
        });
        let registry = InstrumentRegistry::from_provider(&provider);
        (registry, exporter, provider, rt)
    }

    #[test]
    fn test_registry_constructs_all_instruments() {
        // FR-003: the registry must register every metric in the catalog.
        let registry = build_registry();
        // Just accessing the fields proves they were constructed.
        let _ = &registry.llm_requests;
        let _ = &registry.sessions_active;
        let _ = &registry.team_members;
        let _ = &registry.llm_duration;
        let _ = &registry.tokens_input;
        let _ = &registry.cost_estimated;
        let _ = &registry.errors_total;
        let _ = &registry.context_compression_ratio;
    }

    #[test]
    fn test_counter_can_add() {
        let (registry, exporter, provider, rt) = build_registry_with_exporter();
        registry.llm_requests.add(1, &[]);
        rt.block_on(async { provider.force_flush().expect("flush") });
        let metrics = exporter.get_finished_metrics().unwrap_or_default();
        assert!(
            !metrics.is_empty(),
            "counter add should produce a metric batch"
        );
    }

    #[test]
    fn test_histogram_can_record() {
        let (registry, exporter, provider, rt) = build_registry_with_exporter();
        registry.llm_duration.record(42.0, &[]);
        rt.block_on(async { provider.force_flush().expect("flush") });
        let metrics = exporter.get_finished_metrics().unwrap_or_default();
        let has_histogram = metrics.iter().any(|rm| {
            rm.scope_metrics
                .iter()
                .flat_map(|sm| sm.metrics.iter())
                .any(|m| m.name == "ragent.llm.duration")
        });
        assert!(
            has_histogram,
            "histogram record should produce ragent.llm.duration"
        );
    }

    #[test]
    fn test_gauge_can_record() {
        let (registry, exporter, provider, rt) = build_registry_with_exporter();
        registry.team_members.record(3, &[]);
        rt.block_on(async { provider.force_flush().expect("flush") });
        let metrics = exporter.get_finished_metrics().unwrap_or_default();
        let has_gauge = metrics.iter().any(|rm| {
            rm.scope_metrics
                .iter()
                .flat_map(|sm| sm.metrics.iter())
                .any(|m| m.name == "ragent.team.members")
        });
        assert!(has_gauge, "gauge record should produce ragent.team.members");
    }

    #[test]
    fn test_up_down_counter_can_add() {
        let (registry, exporter, provider, rt) = build_registry_with_exporter();
        registry.sessions_active.add(1, &[]);
        registry.sessions_active.add(-1, &[]);
        rt.block_on(async { provider.force_flush().expect("flush") });
        let metrics = exporter.get_finished_metrics().unwrap_or_default();
        let has_up_down = metrics.iter().any(|rm| {
            rm.scope_metrics
                .iter()
                .flat_map(|sm| sm.metrics.iter())
                .any(|m| m.name == "ragent.sessions.active")
        });
        assert!(
            has_up_down,
            "up-down counter add should produce ragent.sessions.active"
        );
    }

    #[test]
    fn test_attrs_are_sanitised() {
        // FR-034: sensitive tool names should be redacted.
        let tool_attr = InstrumentRegistry::attr_tool("sk-secret-key");
        assert_eq!(tool_attr.value.to_string(), "redacted");
    }

    #[test]
    fn test_metric_toggles_disable_metric() {
        // FR-027: a metric set to false should be disabled.
        let mut toggles = std::collections::HashMap::new();
        toggles.insert("ragent.llm.requests".to_string(), false);
        let registry = build_registry().with_metric_toggles(toggles);
        assert!(!registry.is_metric_enabled("ragent.llm.requests"));
        assert!(registry.is_metric_enabled("ragent.tool.invocations"));
    }
}
