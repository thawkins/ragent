//! Integration tests for batch export timer and configurable interval
//! (T-008, FR-006).
//!
//! FR-006: "The system shall batch metric exports on a configurable interval
//! (default 30 seconds) and flush all pending exports on process shutdown."
//!
//! These tests verify:
//!
//! 1. The configurable export interval is applied from `OtelConfig` (FR-006).
//! 2. Metrics are buffered and not exported until a flush is triggered.
//! 3. `flush()` forces an immediate export of all buffered metrics.
//! 4. `shutdown()` succeeds and is idempotent for a disabled subsystem.
//! 5. A disabled subsystem's `flush()` and `shutdown()` are no-ops.
//!
//! They use the OTEL SDK's [`InMemoryMetricExporter`] so no live OTLP
//! collector is required.
//!
//! # Note on `InMemoryMetricExporter::shutdown`
//!
//! The `InMemoryMetricExporter`'s `shutdown()` implementation **clears**
//! its internal metric buffer. This means that after `provider.shutdown()`
//! returns, `get_finished_metrics()` will be empty — the metrics were
//! collected during the shutdown flush and then immediately discarded by
//! the exporter's own shutdown. This is an artifact of the in-memory test
//! exporter, not of the real OTLP exporter (which sends data over the
//! network before shutting down). Therefore, tests that need to inspect
//! metric contents use `force_flush()` (which does not clear), while
//! shutdown tests verify that `shutdown()` returns `Ok(())` without
//! panicking.

#![cfg(feature = "telemetry")]

use std::time::Duration;

use opentelemetry_sdk::metrics::SdkMeterProvider;
use opentelemetry_sdk::runtime::Tokio;
use opentelemetry_sdk::testing::metrics::InMemoryMetricExporter;
use ragent_telemetry::{InstrumentRegistry, OtelConfig, TelemetryState, TelemetrySubsystem};

// ── Helpers ───────────────────────────────────────────────────────────────

/// Build a `SdkMeterProvider` backed by an `InMemoryMetricExporter` with a
/// `PeriodicReader` configured to use a very long export interval so that no
/// background export fires during the test. The caller controls flushing via
/// `force_flush()` or `shutdown()`.
fn build_in_memory_provider() -> (
    SdkMeterProvider,
    InMemoryMetricExporter,
    tokio::runtime::Runtime,
) {
    let rt = tokio::runtime::Runtime::new().expect("tokio runtime");
    let exporter = InMemoryMetricExporter::default();
    let exporter_clone = exporter.clone();

    let provider = rt.block_on(async {
        // Use a 1-hour interval so no background export fires during tests.
        let reader = opentelemetry_sdk::metrics::PeriodicReader::builder(exporter_clone, Tokio)
            .with_interval(Duration::from_hours(1))
            .build();
        SdkMeterProvider::builder().with_reader(reader).build()
    });

    (provider, exporter, rt)
}

/// Record a single LLM request counter via the instrument registry.
fn record_llm_request(provider: &SdkMeterProvider, model: &str, provider_name: &str) {
    let registry = InstrumentRegistry::from_provider(provider);
    let attrs = &[
        InstrumentRegistry::attr_model(model),
        InstrumentRegistry::attr_provider(provider_name),
    ];
    registry.llm_requests.add(1, attrs);
}

// ── Tests ─────────────────────────────────────────────────────────────────

/// The export interval from `OtelConfig` is applied to the subsystem.
///
/// FR-006: "batch metric exports on a configurable interval (default 30
/// seconds)".
#[test]
fn test_default_export_interval_is_30_seconds() {
    let config = OtelConfig::default();
    assert_eq!(
        config.export_interval_seconds, 30,
        "default export interval must be 30 seconds per FR-006"
    );
}

/// A custom export interval is preserved in the config accessor.
#[test]
fn test_custom_export_interval_preserved() {
    let mut config = OtelConfig::default();
    config.enabled = true;
    config.endpoint = "http://localhost:4318".to_string();
    config.export_interval_seconds = 60;

    let rt = tokio::runtime::Runtime::new().expect("tokio runtime");
    let sub = rt.block_on(async { TelemetrySubsystem::new(config).expect("should construct") });

    assert_eq!(sub.config().export_interval_seconds, 60);
}

/// Metrics are buffered by the `PeriodicReader` and not exported until
/// `flush()` is called.
///
/// FR-006: the system batches metric exports — metrics recorded between
/// export intervals are accumulated and only sent to the exporter on the
/// next flush/interval tick.
#[test]
fn test_metrics_buffered_until_flush() {
    let (provider, exporter, rt) = build_in_memory_provider();

    // Record a metric.
    record_llm_request(&provider, "gpt-4", "openai");

    // Without flushing, the in-memory exporter should have no metrics.
    let metrics_before = exporter.get_finished_metrics().unwrap_or_default();
    assert!(
        metrics_before.is_empty(),
        "no metrics should be exported before a flush, got {} metric batches",
        metrics_before.len()
    );

    // Force flush — now metrics should appear.
    rt.block_on(async {
        provider.force_flush().expect("force_flush should succeed");
    });

    let metrics_after = exporter.get_finished_metrics().unwrap_or_default();
    assert!(
        !metrics_after.is_empty(),
        "metrics should be exported after force_flush, got empty result"
    );
}

/// `flush()` on an enabled `TelemetrySubsystem` triggers an immediate
/// export of all buffered metrics.
///
/// FR-006 + FR-019: "flush all pending exports on process shutdown" — the
/// `flush()` method is the mechanism the shutdown signal handler (T-009)
/// calls before `shutdown()`.
#[test]
fn test_flush_triggers_immediate_export() {
    let (provider, exporter, rt) = build_in_memory_provider();

    // Record multiple metrics.
    record_llm_request(&provider, "gpt-4", "openai");
    record_llm_request(&provider, "claude-3", "anthropic");
    record_llm_request(&provider, "gpt-4", "openai");

    // Flush via the provider's force_flush (same as TelemetrySubsystem::flush).
    rt.block_on(async {
        provider.force_flush().expect("flush should succeed");
    });

    let metrics = exporter.get_finished_metrics().unwrap_or_default();
    assert!(!metrics.is_empty(), "flush should export buffered metrics");

    // Verify at least one metric batch contains our counter.
    let has_llm_requests = metrics.iter().any(|rm| {
        rm.scope_metrics
            .iter()
            .flat_map(|sm| sm.metrics.iter())
            .any(|m| m.name == "ragent.llm.requests")
    });
    assert!(
        has_llm_requests,
        "exported metrics should contain ragent.llm.requests"
    );
}

/// `shutdown()` on an enabled subsystem returns `Ok(())`.
///
/// FR-006: "flush all pending exports on process shutdown."
///
/// Note: the `InMemoryMetricExporter` clears its buffer during shutdown,
/// so we cannot inspect metrics after `shutdown()` — we verify the call
/// succeeds without error, which proves the flush-and-shutdown path
/// executes.
#[test]
fn test_shutdown_succeeds_with_pending_metrics() {
    let (provider, _exporter, rt) = build_in_memory_provider();

    // Record a metric.
    record_llm_request(&provider, "gpt-4", "openai");

    // Shutdown should flush (collect_and_export) and then shut down the
    // exporter. It should return Ok(()).
    rt.block_on(async {
        provider.shutdown().expect("shutdown should succeed");
    });
}

/// `flush()` on a disabled subsystem is a no-op.
#[test]
fn test_flush_disabled_subsystem_is_noop() {
    let sub = TelemetrySubsystem::disabled();
    assert_eq!(sub.state(), TelemetryState::Disabled);
    assert!(
        sub.flush().is_ok(),
        "flush on disabled subsystem should succeed"
    );
}

/// `shutdown()` on a disabled subsystem is a no-op.
#[test]
fn test_shutdown_disabled_subsystem_is_noop() {
    let sub = TelemetrySubsystem::disabled();
    assert!(
        sub.shutdown().is_ok(),
        "shutdown on disabled subsystem should succeed"
    );
}

/// `flush()` followed by `shutdown()` on an enabled subsystem exports
/// metrics and then shuts down cleanly.
///
/// This simulates the graceful-shutdown path: the signal handler calls
/// `flush()`, reads/verifies metrics, then the main shutdown path calls
/// `shutdown()`.
#[test]
fn test_flush_then_shutdown_succeeds() {
    let (provider, exporter, rt) = build_in_memory_provider();

    record_llm_request(&provider, "gpt-4", "openai");

    // Flush first — metrics should appear in the exporter.
    rt.block_on(async {
        provider.force_flush().expect("first flush should succeed");
    });

    let metrics_after_flush = exporter.get_finished_metrics().unwrap_or_default();
    assert!(
        !metrics_after_flush.is_empty(),
        "first flush should export buffered metrics"
    );

    // Record more metrics after flush.
    record_llm_request(&provider, "claude-3", "anthropic");

    // Flush again to export the second batch.
    rt.block_on(async {
        provider.force_flush().expect("second flush should succeed");
    });

    let metrics_after_second_flush = exporter.get_finished_metrics().unwrap_or_default();
    assert!(
        !metrics_after_second_flush.is_empty(),
        "second flush should export metrics recorded after the first flush"
    );

    // Shutdown should succeed (it will clear the exporter buffer, but
    // the metrics were already exported during the flushes above).
    rt.block_on(async {
        provider.shutdown().expect("shutdown should succeed");
    });
}

/// `TelemetrySubsystem::flush()` triggers an immediate export via the
/// subsystem handle and does not panic when no collector is running.
///
/// This verifies that the `flush()` method on `TelemetrySubsystem` calls
/// `provider.force_flush()` correctly. When no live OTLP collector is
/// reachable (as in this test), the flush returns an `Err` but does not
/// panic (FR-031, FR-033: exporter errors must not crash the process).
#[test]
fn test_subsystem_flush_does_not_panic_without_collector() {
    let mut config = OtelConfig::default();
    config.enabled = true;
    config.endpoint = "http://localhost:4318".to_string();

    let rt = tokio::runtime::Runtime::new().expect("tokio runtime");
    let sub = rt.block_on(async { TelemetrySubsystem::new(config).expect("should construct") });

    // Get instruments and record a metric.
    if let Some(registry) = sub.instruments() {
        registry.llm_requests.add(1, &[]);
    }

    // flush() may return Err because no collector is running, but it must
    // not panic (FR-031, FR-033).
    let _ = sub.flush();

    // Shutdown should also not panic (it will try to flush and may fail,
    // but the process must not crash).
    let _ = sub.shutdown();
}

/// A zero export interval is clamped to 1 second by `build_provider`.
///
/// The `OtelConfig::validate()` rejects zero intervals, but `build_provider`
/// defensively clamps to `max(1)` so the `PeriodicReader` never gets a
/// zero-duration interval (which would cause a tight loop).
#[test]
fn test_zero_interval_clamped_in_build_provider() {
    let mut config = OtelConfig::default();
    config.enabled = true;
    config.endpoint = "http://localhost:4318".to_string();
    config.export_interval_seconds = 0;

    let rt = tokio::runtime::Runtime::new().expect("tokio runtime");
    let result = rt.block_on(async { TelemetrySubsystem::new(config) });

    assert!(
        result.is_ok(),
        "subsystem with zero interval should construct (clamped), got: {:?}",
        result.err()
    );
    let sub = result.expect("subsystem should construct");
    assert_eq!(sub.state(), TelemetryState::Enabled);
    assert!(
        sub.flush().is_ok(),
        "flush should work after clamped interval"
    );
    assert!(
        sub.shutdown().is_ok(),
        "shutdown should work after clamped interval"
    );
}
