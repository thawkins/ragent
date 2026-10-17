//! External tests exercising the recorder constructors that are not covered by
//! the inline suite: `from_subsystem` (enabled + disabled), the `Default`
//! impls, and the per-metric toggle short-circuit branches (FR-027).
//!
//! The recorders write to a process-global `SNAPSHOT` (see
//! `ragent_telemetry::counters`), shared with every other test in the binary
//! and with tests in sibling binaries running concurrently. Assertions are
//! therefore written as *monotonic* checks - `>=` against a value this test
//! itself caused - rather than exact equality, and the metric-counting tests
//! take a process-wide lock so they do not interleave with each other.

#![cfg(feature = "telemetry")]

use std::sync::{Mutex, MutexGuard, OnceLock};

/// Serialises the tests that read back process-global counter values.
///
/// Recovers from a poisoned lock so one panicking test does not cascade.
fn counters_lock() -> MutexGuard<'static, ()> {
    static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    LOCK.get_or_init(|| Mutex::new(()))
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

use std::collections::HashMap;
use std::time::Duration;

use opentelemetry_sdk::metrics::SdkMeterProvider;
use ragent_telemetry::recorder::{
    CompressionRecorder, CoordinatorRecorder, LlmRecorder, PermissionRecorder, SessionRecorder,
    SnapshotRecorder, ToolRecorder,
};
use ragent_telemetry::{InstrumentRegistry, OtelConfig, TelemetrySubsystem};

/// An enabled subsystem backed by an in-memory provider (no network traffic).
fn enabled_subsystem() -> (TelemetrySubsystem, tokio::runtime::Runtime) {
    let rt = tokio::runtime::Runtime::new().expect("tokio runtime");
    let exporter = opentelemetry_sdk::metrics::InMemoryMetricExporter::default();
    let provider = {
        let reader = opentelemetry_sdk::metrics::PeriodicReader::builder(exporter)
            .with_interval(Duration::from_hours(1))
            .build();
        SdkMeterProvider::builder().with_reader(reader).build()
    };
    let sub = TelemetrySubsystem::from_provider(OtelConfig::default(), provider);
    (sub, rt)
}

/// Every recorder's `from_subsystem` yields a live recorder for an enabled
/// subsystem and a disabled (no-op) recorder for a disabled subsystem; the
/// `Default` impl matches the disabled constructor.
#[test]
fn test_recorder_from_subsystem_and_defaults() {
    let (enabled, _rt) = enabled_subsystem();
    let disabled = TelemetrySubsystem::disabled();

    // Enabled subsystem -> live recorders.
    assert!(LlmRecorder::from_subsystem(&enabled).is_enabled());
    assert!(ToolRecorder::from_subsystem(&enabled).is_enabled());
    assert!(SessionRecorder::from_subsystem(&enabled).is_enabled());
    assert!(CoordinatorRecorder::from_subsystem(&enabled).is_enabled());
    assert!(PermissionRecorder::from_subsystem(&enabled).is_enabled());
    assert!(CompressionRecorder::from_subsystem(&enabled).is_enabled());
    assert!(SnapshotRecorder::from_subsystem(&enabled).is_enabled());

    // Disabled subsystem -> no-op recorders.
    assert!(!LlmRecorder::from_subsystem(&disabled).is_enabled());
    assert!(!ToolRecorder::from_subsystem(&disabled).is_enabled());
    assert!(!SessionRecorder::from_subsystem(&disabled).is_enabled());
    assert!(!CoordinatorRecorder::from_subsystem(&disabled).is_enabled());
    assert!(!PermissionRecorder::from_subsystem(&disabled).is_enabled());
    assert!(!CompressionRecorder::from_subsystem(&disabled).is_enabled());
    assert!(!SnapshotRecorder::from_subsystem(&disabled).is_enabled());

    // Default == disabled constructor.
    assert!(!LlmRecorder::default().is_enabled());
    assert!(!ToolRecorder::default().is_enabled());
    assert!(!SessionRecorder::default().is_enabled());
    assert!(!CoordinatorRecorder::default().is_enabled());
    assert!(!PermissionRecorder::default().is_enabled());
    assert!(!CompressionRecorder::default().is_enabled());
    assert!(!SnapshotRecorder::default().is_enabled());
}

/// With every metric toggle set to `false`, each recorder method takes its
/// short-circuit `return`/skip branch and records nothing.
#[test]
fn test_all_metrics_disabled_short_circuits() {
    // Reads the process-global counter snapshot; serialise against the sibling
    // metric-counting test so the two do not interleave.
    let _guard = counters_lock();
    let mut metrics = HashMap::new();
    for name in [
        "ragent.llm.requests",
        "ragent.tokens.input",
        "ragent.tokens.output",
        "ragent.cost.estimated",
        "ragent.llm.duration",
        "ragent.llm.time_to_first_token",
        "ragent.retries.llm",
        "ragent.rate_limit.requests_pct",
        "ragent.rate_limit.tokens_pct",
        "ragent.tool.invocations",
        "ragent.tool.duration",
        "ragent.sessions.active",
        "ragent.sessions.total",
        "ragent.agent_loop.duration",
        "ragent.agent_loop.iterations",
        "ragent.tool.calls_per_session",
        "ragent.subagent.spawns",
        "ragent.agents.active",
        "ragent.agents.completed",
        "ragent.errors.total",
        "ragent.timeouts.total",
        "ragent.permission.approved",
        "ragent.permission.denied",
        "ragent.context.compressions",
        "ragent.context.compression_ratio",
        "ragent.snapshot.restores",
    ] {
        metrics.insert(name.to_string(), false);
    }

    let rt = tokio::runtime::Runtime::new().expect("tokio runtime");
    let exporter = opentelemetry_sdk::metrics::InMemoryMetricExporter::default();
    let provider = {
        let reader = opentelemetry_sdk::metrics::PeriodicReader::builder(exporter)
            .with_interval(Duration::from_hours(1))
            .build();
        SdkMeterProvider::builder().with_reader(reader).build()
    };
    let config = OtelConfig {
        metrics,
        ..Default::default()
    };
    let sub = TelemetrySubsystem::from_provider(config, provider);

    let llm = LlmRecorder::from_subsystem(&sub);
    llm.record_request("gpt-4", "openai");
    llm.record_usage("gpt-4", "openai", 10, 20);
    llm.record_cost("gpt-4", "openai", 0.01);
    llm.record_duration("gpt-4", "openai", 5.0);
    llm.record_ttft("gpt-4", 1.0);
    llm.record_retry("gpt-4", "openai");
    llm.record_rate_limit("openai", Some(1.0), Some(2.0));

    let tool = ToolRecorder::from_subsystem(&sub);
    tool.record_invocation("read");
    tool.record_duration("read", 3.0);

    let session = SessionRecorder::from_subsystem(&sub);
    session.record_session_start();
    session.record_session_end();
    session.record_agent_loop(100.0, 3);
    session.record_tool_calls_per_session(4);

    let coord = CoordinatorRecorder::from_subsystem(&sub);
    coord.record_agent_spawn();
    coord.record_agent_complete();
    coord.record_error("llm");
    coord.record_timeout();

    let perm = PermissionRecorder::from_subsystem(&sub);
    perm.record_approved("read");
    perm.record_denied("bash");

    let comp = CompressionRecorder::from_subsystem(&sub);
    comp.record_compression(1000, 500, 0.5);

    let snap = SnapshotRecorder::from_subsystem(&sub);
    snap.record_restore();

    // Snapshot gauges still update for the last-value mirrors (they are
    // independent of the OTEL toggles), so a couple of values are non-zero.
    let values = ragent_telemetry::counters::current_values();
    assert!(values.agent_loop_iterations_last >= 3);

    let _ = rt;
}

/// With toggles left at default (enabled), a live `InstrumentRegistry`-backed
/// recorder routes through the enabled branch; this exercises the `new`
/// constructors for each recorder.
#[test]
fn test_recorder_new_constructors_with_enabled_registry() {
    // Writes the process-global counter snapshot (`record_agent_loop` sets the
    // last-value gauge to 2), so it must not interleave with the sibling
    // metric-counting test's `>= 3` read.
    let _guard = counters_lock();
    let rt = tokio::runtime::Runtime::new().expect("tokio runtime");
    let exporter = opentelemetry_sdk::metrics::InMemoryMetricExporter::default();
    let provider = {
        let reader = opentelemetry_sdk::metrics::PeriodicReader::builder(exporter)
            .with_interval(Duration::from_hours(1))
            .build();
        SdkMeterProvider::builder().with_reader(reader).build()
    };

    let registry = InstrumentRegistry::from_provider(&provider);
    let llm = LlmRecorder::new(registry.clone());
    llm.record_request("m", "p");
    llm.record_retry("m", "p");

    let tool = ToolRecorder::new(registry.clone());
    tool.record_invocation("read");

    let session = SessionRecorder::new(registry.clone());
    session.record_session_start();
    session.record_session_end();
    session.record_agent_loop(50.0, 2);
    session.record_tool_calls_per_session(7);

    let coord = CoordinatorRecorder::new(registry.clone());
    coord.record_agent_spawn();
    coord.record_agent_complete();
    coord.record_error("tool");
    coord.record_timeout();

    let perm = PermissionRecorder::new(registry.clone());
    perm.record_approved("read");
    perm.record_denied("write");

    let comp = CompressionRecorder::new(registry.clone());
    comp.record_compression(2000, 1000, 0.5);

    let snap = SnapshotRecorder::new(registry);
    snap.record_restore();

    assert!(llm.is_enabled());
    assert!(tool.is_enabled());
    assert!(session.is_enabled());
    assert!(coord.is_enabled());
    assert!(perm.is_enabled());
    assert!(comp.is_enabled());
    assert!(snap.is_enabled());
    let _ = rt;
}
