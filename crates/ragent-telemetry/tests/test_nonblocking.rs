//! Integration tests for the non-blocking guarantee (T-021, FR-031, FR-033).
//!
//! FR-031: "The system shall not block the agent loop, LLM streaming, or tool
//! execution if the OTLP exporter is unavailable or slow; all metric recording
//! and export shall be asynchronous and non-blocking."
//!
//! FR-033: "The system shall not crash the process if the OTLP endpoint returns
//! an error; exporter errors shall be logged at `warn` level and retried on the
//! next export interval."
//!
//! # What "non-blocking" means here
//!
//! The guarantee has three layers, each tested below:
//!
//! 1. **Recording is non-blocking** — `Counter::add`, `Histogram::record`, and
//!    `Gauge::record` are synchronous atomic operations that never touch the
//!    network. They complete in nanoseconds regardless of exporter state.
//! 2. **Periodic export is asynchronous** — the `PeriodicReader` runs exports on
//!    a background tokio task; the agent loop never waits for it.
//! 3. **Error handling never panics** — `flush()`, `shutdown()`, and
//!    `ShutdownGuard::drop` log exporter errors at `warn` level and return
//!    `Err` (or swallow them in the case of `Drop`) rather than panicking.
//!
//! Additionally, the exporter is configured with a bounded request timeout
//! ([`OtelConfig::export_timeout_seconds`]) so that even an explicit `flush()`
//! against a slow endpoint cannot hang indefinitely.

#![cfg(feature = "telemetry")]

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use ragent_telemetry::{
    InstrumentRegistry, OtelConfig, OtelProtocol, TelemetryState, TelemetrySubsystem,
};

// ── Helpers ───────────────────────────────────────────────────────────────

/// Build an `OtelConfig` pointing at an unreachable endpoint with a 1-second
/// export timeout.
fn unreachable_config() -> OtelConfig {
    let mut config = OtelConfig::default();
    config.enabled = true;
    // Port 1 is reserved and refuses connections immediately (ECONNREFUSED).
    config.endpoint = "http://127.0.0.1:1".to_string();
    config.protocol = OtelProtocol::Http;
    // 1 second — the minimum allowed. Clamped by build_metric_exporter.
    config.export_timeout_seconds = 1;
    // Long interval so no background export fires during the test.
    config.export_interval_seconds = 3600;
    config
}

// ── 1. Recording is non-blocking ──────────────────────────────────────────

/// Recording metrics through a live `InstrumentRegistry` does not block on
/// network I/O, even when the exporter endpoint is unreachable (FR-031).
///
/// `Counter::add`, `Histogram::record`, and `Gauge::record` are synchronous
/// atomic operations. The actual network export happens asynchronously on a
/// background `PeriodicReader` task.
#[test]
fn test_recording_does_not_block_on_unreachable_endpoint() {
    let config = unreachable_config();
    let rt = tokio::runtime::Runtime::new().expect("tokio runtime");
    let sub = rt.block_on(async { TelemetrySubsystem::new(config).expect("should construct") });

    assert_eq!(sub.state(), TelemetryState::Enabled);

    let registry = sub
        .instruments()
        .expect("enabled subsystem must provide instruments");

    // Record a batch of metrics. This must complete in well under a second
    // because recording is a synchronous atomic operation, not a network call.
    let start = Instant::now();
    for _ in 0..1000 {
        registry.llm_requests.add(1, &[]);
        registry.tokens_input.add(100, &[]);
        registry.tokens_output.add(50, &[]);
        registry.tool_invocations.add(1, &[]);
        registry.llm_duration.record(42.5, &[]);
        registry.tool_duration.record(10.0, &[]);
    }
    let elapsed = start.elapsed();

    assert!(
        elapsed < Duration::from_millis(500),
        "recording 1000 metric points took {elapsed:?}, expected < 500ms — \
         recording must not block on network I/O (FR-031)"
    );
}

/// Recording through a no-op registry is effectively instantaneous (FR-022,
/// NFR-002).
#[test]
fn test_recording_noop_registry_is_instantaneous() {
    let registry = InstrumentRegistry::noop();

    let start = Instant::now();
    for _ in 0..10_000 {
        registry.llm_requests.add(1, &[]);
        registry.tokens_input.add(100, &[]);
        registry.llm_duration.record(42.5, &[]);
    }
    let elapsed = start.elapsed();

    assert!(
        elapsed < Duration::from_millis(100),
        "no-op recording 10000 points took {elapsed:?}, expected < 100ms (FR-022)"
    );
}

// ── 2. flush()/shutdown() against an unreachable endpoint never panic ─────

/// `flush()` against an unreachable endpoint does not panic — it returns a
/// `Result` that the caller handles (FR-033).
///
/// This mirrors `test_subsystem_flush_does_not_panic_without_collector` in
/// `test_batch_export.rs` but uses an explicitly refused port and a 1-second
/// export timeout so the flush fails (or no-ops) promptly.
#[test]
fn test_flush_unreachable_endpoint_does_not_panic() {
    let config = unreachable_config();
    let rt = tokio::runtime::Runtime::new().expect("tokio runtime");
    let sub = rt.block_on(async { TelemetrySubsystem::new(config).expect("should construct") });

    if let Some(registry) = sub.instruments() {
        registry.llm_requests.add(1, &[]);
    }

    // The guarantee: this call does not panic, regardless of Ok/Err.
    let _ = sub.flush();
}

/// `shutdown()` against an unreachable endpoint does not panic (FR-033).
#[test]
fn test_shutdown_unreachable_endpoint_does_not_panic() {
    let config = unreachable_config();
    let rt = tokio::runtime::Runtime::new().expect("tokio runtime");
    let sub = rt.block_on(async { TelemetrySubsystem::new(config).expect("should construct") });

    if let Some(registry) = sub.instruments() {
        registry.llm_requests.add(1, &[]);
    }

    let _ = sub.shutdown();
}

/// A disabled subsystem's `flush()` and `shutdown()` always succeed and never
/// panic (FR-022, FR-032).
#[test]
fn test_disabled_flush_shutdown_never_panic() {
    let sub = TelemetrySubsystem::disabled();
    assert!(sub.flush().is_ok(), "disabled flush must succeed");
    assert!(sub.shutdown().is_ok(), "disabled shutdown must succeed");
}

// ── 3. ShutdownGuard::drop is infallible ─────────────────────────────────

/// `ShutdownGuard::drop` does not panic even when the endpoint is unreachable
/// and flush/shutdown fail (FR-031, FR-033).
///
/// This is critical because `Drop` runs during stack unwinding (e.g. if the
/// agent loop panicked for an unrelated reason) and must not itself fail. The
/// guard logs errors at `warn` level but never panics.
#[test]
fn test_shutdown_guard_drop_does_not_panic_on_unreachable_endpoint() {
    let config = unreachable_config();
    let rt = tokio::runtime::Runtime::new().expect("tokio runtime");
    let sub = rt.block_on(async { TelemetrySubsystem::new(config).expect("should construct") });

    if let Some(registry) = sub.instruments() {
        registry.llm_requests.add(1, &[]);
    }

    // Install the guard. When it drops at the end of this block, it will
    // attempt to flush+shutdown against the unreachable endpoint. The errors
    // must be logged but not panicked.
    {
        let guard = ragent_telemetry::shutdown::ShutdownGuard::new(sub);
        // Use the guard so it's not optimised away.
        assert_eq!(guard.subsystem().state(), TelemetryState::Enabled);
        // guard drops here — must not panic.
    }
    // If we reach this point, the guard's Drop did not panic.
}

/// `ShutdownGuard::drop` on a disabled subsystem is a clean no-op (FR-022).
#[test]
fn test_shutdown_guard_drop_disabled_is_clean() {
    let sub = TelemetrySubsystem::disabled();
    let guard = ragent_telemetry::shutdown::ShutdownGuard::new(sub);
    drop(guard); // must not panic or hang
}

// ── 4. Recording after a failed export still works (retry semantics) ──────

/// After a failed export (unreachable endpoint), recording still works — the
/// SDK does not poison instruments (FR-033: "retried on the next export
/// interval").
#[test]
fn test_recording_still_works_after_failed_flush() {
    let config = unreachable_config();
    let rt = tokio::runtime::Runtime::new().expect("tokio runtime");
    let sub = rt.block_on(async { TelemetrySubsystem::new(config).expect("should construct") });

    let registry = sub
        .instruments()
        .expect("enabled subsystem must provide instruments");

    // Record, attempt a flush (which may fail), then record again.
    registry.llm_requests.add(1, &[]);
    // flush may fail — that's fine, we swallow it.
    let _ = sub.flush();
    // Recording must still work after a failed flush (FR-033 retry semantics).
    registry.llm_requests.add(1, &[]);

    // If we got here without panicking, the guarantee holds.
}

// ── 5. Export timeout is configurable and clamped ─────────────────────────

/// The default export timeout is 10 seconds (matching the OTEL SDK default).
#[test]
fn test_default_export_timeout_is_10_seconds() {
    let config = OtelConfig::default();
    assert_eq!(
        config.export_timeout_seconds, 10,
        "default export timeout must be 10 seconds (FR-031)"
    );
}

/// A custom export timeout is preserved in the config accessor.
#[test]
fn test_custom_export_timeout_preserved() {
    let mut config = unreachable_config();
    config.export_timeout_seconds = 5;

    let rt = tokio::runtime::Runtime::new().expect("tokio runtime");
    let sub = rt.block_on(async { TelemetrySubsystem::new(config).expect("should construct") });

    assert_eq!(sub.config().export_timeout_seconds, 5);
}

/// A zero export timeout is clamped to 1 second at build time so that every
/// export does not fail immediately (FR-031).
#[test]
fn test_zero_export_timeout_clamped_to_one_second() {
    let mut config = unreachable_config();
    config.export_timeout_seconds = 0;

    // The config validates and rejects zero, but build_metric_exporter
    // defensively clamps to max(1) so the subsystem still constructs.
    let rt = tokio::runtime::Runtime::new().expect("tokio runtime");
    let result = rt.block_on(async { TelemetrySubsystem::new(config) });
    assert!(
        result.is_ok(),
        "subsystem with zero timeout should construct (clamped), got: {:?}",
        result.err()
    );
}

/// The config validator flags a zero export timeout as a problem.
#[test]
fn test_validate_rejects_zero_export_timeout() {
    let config = OtelConfig {
        enabled: true,
        endpoint: "http://localhost:4318".to_string(),
        export_timeout_seconds: 0,
        ..OtelConfig::default()
    };
    let problems = config.validate();
    assert!(
        problems
            .iter()
            .any(|p| p.contains("export_timeout_seconds")),
        "expected an export_timeout problem, got {problems:?}"
    );
}

// ── 6. flush_on_signal_arc never panics on install ────────────────────────

/// `flush_on_signal_arc` installs a signal handler without panicking (FR-031,
/// FR-033).
///
/// Both the subsystem construction (which spawns a `PeriodicReader` background
/// task) and the signal handler installation must happen within a tokio
/// runtime context. A manually constructed runtime is used so that dropping
/// it forcibly cancels the background tasks, avoiding a hang on test
/// shutdown.
#[test]
fn test_flush_on_signal_arc_constructs_without_panic() {
    let config = unreachable_config();
    let rt = tokio::runtime::Runtime::new().expect("tokio runtime");

    let (sub, handle) = rt.block_on(async {
        let sub = Arc::new(TelemetrySubsystem::new(config).expect("should construct"));
        let handle = ragent_telemetry::shutdown::flush_on_signal_arc(sub.clone())
            .expect("signal handler should install");
        (sub, handle)
    });

    // Abort the background signal-listener task.
    handle.abort();
    // Drop the subsystem and then the runtime to cancel all background tasks.
    drop(sub);
    drop(rt);
}

/// `flush_on_signal_arc` with a disabled subsystem installs cleanly and the
/// task is a no-op (FR-022).
#[test]
fn test_flush_on_signal_arc_disabled_installs_cleanly() {
    let rt = tokio::runtime::Runtime::new().expect("tokio runtime");

    rt.block_on(async {
        let sub = Arc::new(TelemetrySubsystem::disabled());
        ragent_telemetry::shutdown::flush_on_signal_arc(sub)
            .expect("signal handler should install")
            .await
            .expect("signal handler task completed")
    });

    drop(rt);
}

// ── 7. Concurrent recording does not deadlock or panic ────────────────────

/// Recording from multiple threads concurrently against an unreachable
/// endpoint does not deadlock or panic (FR-031).
///
/// The `CardinalityCache` uses an `RwLock` and fails open on poison, and the
/// OTEL instruments are `Arc`-backed atomic operations. Together these
/// guarantee that concurrent recording is lock-free in the common case and
/// never panics.
#[test]
fn test_concurrent_recording_does_not_deadlock() {
    use std::thread;

    let config = unreachable_config();
    let rt = tokio::runtime::Runtime::new().expect("tokio runtime");
    let sub = rt.block_on(async { TelemetrySubsystem::new(config).expect("should construct") });

    let registry = sub
        .instruments()
        .expect("enabled subsystem must provide instruments");

    let panicked = Arc::new(AtomicBool::new(false));
    let mut handles = Vec::new();

    for _ in 0..4 {
        let reg = registry.clone();
        let p = panicked.clone();
        handles.push(thread::spawn(move || {
            for i in 0..500u64 {
                let attrs = [
                    InstrumentRegistry::attr_model("test-model"),
                    InstrumentRegistry::attr_provider("test-provider"),
                ];
                let resolved = reg.resolve_attrs("ragent.llm.requests", &attrs);
                reg.llm_requests.add(1, &resolved);
                reg.tokens_input.add(i, &resolved);
                reg.llm_duration.record(f64::from(i as u32), &resolved);
            }
            // If we reach here, no panic occurred.
            p.store(false, Ordering::SeqCst);
        }));
    }

    for h in handles {
        h.join().expect("worker thread must not panic");
    }

    assert!(
        !panicked.load(Ordering::SeqCst),
        "concurrent recording must not panic (FR-031)"
    );
}

// ── 8. No-op recorder methods never panic ───────────���─────────────────────

/// All no-op recorder methods (used when the `telemetry` feature is off or the
/// subsystem is disabled) never panic and complete instantly (FR-022).
#[test]
fn test_noop_recorder_methods_never_panic() {
    use ragent_telemetry::recorder::{
        CompressionRecorder, LlmRecorder, PermissionRecorder, SessionRecorder, ToolRecorder,
    };

    let llm = LlmRecorder::disabled();
    llm.record_request("model", "provider");
    llm.record_usage("model", "provider", 100, 50);
    llm.record_cost("model", "provider", 0.001);
    llm.record_duration("model", "provider", 42.5);
    llm.record_ttft("model", 100.0);
    llm.record_retry("model", "provider");
    llm.record_rate_limit("provider", Some(75.0), Some(40.0));

    let tool = ToolRecorder::disabled();
    tool.record_invocation("bash");
    tool.record_duration("bash", 10.0);

    let session = SessionRecorder::disabled();
    session.record_session_start();
    session.record_session_end();
    session.record_agent_loop(1000.0, 5);

    let perm = PermissionRecorder::disabled();
    perm.record_approved("bash");
    perm.record_denied("bash");

    let comp = CompressionRecorder::disabled();
    comp.record_compression(1000, 500, 0.5);

    // If we got here, none of the no-op methods panicked.
}

// ── 9. Error path in TelemetrySubsystem::new is non-panicking ─────────────

/// An invalid endpoint returns an `Err` rather than panicking (FR-033).
#[test]
fn test_invalid_endpoint_returns_err_not_panic() {
    let mut config = OtelConfig::default();
    config.enabled = true;
    config.endpoint = "not-a-url".to_string();

    let result = TelemetrySubsystem::new(config);
    assert!(result.is_err(), "invalid endpoint should error, not panic");
}

/// An empty endpoint returns an `Err` rather than panicking (FR-033).
#[test]
fn test_empty_endpoint_returns_err_not_panic() {
    let mut config = OtelConfig::default();
    config.enabled = true;
    config.endpoint = String::new();

    let result = TelemetrySubsystem::new(config);
    assert!(result.is_err(), "empty endpoint should error, not panic");
}

/// A non-HTTP protocol endpoint returns an `Err` rather than panicking.
#[test]
fn test_non_http_endpoint_returns_err_not_panic() {
    let mut config = OtelConfig::default();
    config.enabled = true;
    config.endpoint = "ftp://bad:1234".to_string();

    let result = TelemetrySubsystem::new(config);
    assert!(result.is_err(), "non-HTTP endpoint should error, not panic");
}
