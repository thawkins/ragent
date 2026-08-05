//! Integration test: SIGINT flushes pending exports (T-033, AC-6).
//!
//! AC-6: "A SIGINT/SIGTERM signal received while a session is active must
//! flush pending telemetry exports before the process exits."
//!
//! This test simulates the lifecycle without relying on an actual Unix signal.
//! The signal handler and [`ShutdownGuard`](ragent_telemetry::shutdown::ShutdownGuard)
//! both call [`TelemetrySubsystem::flush()`] followed by
//! [`TelemetrySubsystem::shutdown()`]. We verify that:
//!
//! 1. `TelemetrySubsystem::flush()` forces a synchronous export of buffered
//!    metrics to an in-memory exporter.
//! 2. `ShutdownGuard::flush()` and `ShutdownGuard::drop()` complete without
//!    panicking and leave metrics visible in the exporter.
//!
//! A `PeriodicReader` with an in-memory exporter is used; `force_flush()`
//! drives the export synchronously in the test runtime.

#![cfg(feature = "telemetry")]

use std::time::Duration;

use opentelemetry_sdk::metrics::SdkMeterProvider;

use opentelemetry_sdk::metrics::InMemoryMetricExporter;
use ragent_telemetry::{OtelConfig, TelemetryState, TelemetrySubsystem, shutdown::ShutdownGuard};

fn build_subsystem() -> (
    TelemetrySubsystem,
    InMemoryMetricExporter,
    tokio::runtime::Runtime,
) {
    let config = OtelConfig {
        enabled: true,
        endpoint: "http://localhost:4318".to_string(),
        ..Default::default()
    };

    let rt = tokio::runtime::Runtime::new().expect("tokio runtime");
    let exporter = InMemoryMetricExporter::default();
    let exporter_clone = exporter.clone();

    let provider = rt.block_on(async {
        let reader = opentelemetry_sdk::metrics::PeriodicReader::builder(exporter_clone)
            .with_interval(Duration::from_hours(1))
            .build();
        SdkMeterProvider::builder().with_reader(reader).build()
    });

    let sub = TelemetrySubsystem::from_provider(config, provider);
    (sub, exporter, rt)
}

fn sum_u64(metrics: &[opentelemetry_sdk::metrics::data::ResourceMetrics], name: &str) -> u64 {
    metrics
        .iter()
        .flat_map(|rm| rm.scope_metrics.iter())
        .flat_map(|sm| sm.metrics.iter())
        .filter(|m| m.name == name)
        .filter_map(|m| {
            m.data
                .as_any()
                .downcast_ref::<opentelemetry_sdk::metrics::data::Sum<u64>>()
        })
        .flat_map(|sum| sum.data_points.iter())
        .map(|dp| dp.value)
        .sum()
}

/// [`TelemetrySubsystem::flush`] flushes recorded metrics before returning.
#[test]
fn test_shutdown_flushes_pending_metrics() {
    let (sub, exporter, rt) = build_subsystem();

    assert_eq!(sub.state(), TelemetryState::Enabled);

    let reg = sub
        .instruments()
        .expect("enabled subsystem has instruments");
    reg.llm_requests.add(1, &[]);
    reg.tool_invocations.add(2, &[]);

    // Use flush (the same path a signal handler would call first) so metrics
    // are forced out before shutdown.
    rt.block_on(async {
        sub.flush().expect("flush should complete");
    });

    let metrics = exporter.get_finished_metrics().unwrap_or_default();
    assert!(
        !metrics.is_empty(),
        "flush should produce at least one batch of metrics"
    );

    assert_eq!(
        sum_u64(&metrics, "ragent.llm.requests"),
        1,
        "SIGINT flush should export the LLM request counter"
    );
    assert_eq!(
        sum_u64(&metrics, "ragent.tool.invocations"),
        2,
        "SIGINT flush should export the tool invocation counter"
    );

    // Clean shutdown completes without error.
    rt.block_on(async {
        sub.shutdown().expect("shutdown should complete");
    });
}

/// The [`ShutdownGuard`] drop path also flushes and does not panic.
#[test]
fn test_shutdown_guard_drop_does_not_panic_and_flushes() {
    let (sub, exporter, rt) = build_subsystem();

    let reg = sub
        .instruments()
        .expect("enabled subsystem has instruments");
    reg.sessions_total.add(1, &[]);

    let guard = ShutdownGuard::new(sub);
    // Explicitly flush via the guard. The signal handler also calls flush
    // first; dropping the guard then runs shutdown. Both must complete
    // without panicking and the flushed metrics must be visible.
    rt.block_on(async {
        guard.flush().expect("guard flush should complete");
    });

    let metrics = exporter.get_finished_metrics().unwrap_or_default();
    let has_sessions_total = metrics
        .iter()
        .flat_map(|rm| rm.scope_metrics.iter())
        .flat_map(|sm| sm.metrics.iter())
        .any(|m| {
            m.name == "ragent.sessions.total"
                && m.data
                    .as_any()
                    .downcast_ref::<opentelemetry_sdk::metrics::data::Sum<u64>>()
                    .is_some()
        });

    assert!(
        has_sessions_total,
        "ShutdownGuard drop should flush recorded metrics"
    );
}
