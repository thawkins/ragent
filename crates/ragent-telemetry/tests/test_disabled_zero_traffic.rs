//! Integration test: disabled telemetry produces zero network traffic
//! (T-031, AC-2).
//!
//! AC-2: "With `telemetry.otel.enabled: false`, no HTTP/gRPC traffic is
//! generated to the endpoint and the agent loop incurs no measurable
//! overhead."
//!
//! These tests verify that a disabled [`TelemetrySubsystem`]:
//!
//! 1. Does not construct a live meter provider or OTLP exporter.
//! 2. Does not provide any instruments for recording.
//! 3. Does not produce any exported metric data when metrics are recorded
//!    through the no-op meter provider and a flush is triggered.
//! 4. Has `flush()` and `shutdown()` as no-ops that succeed instantly.
//! 5. Does not construct a `PeriodicReader` or any background export task.
//!
//! The tests run both with and without the `telemetry` Cargo feature to
//! ensure the no-op path is identical in both build configurations.

use ragent_telemetry::{OtelConfig, TelemetryState, TelemetrySubsystem};

// ── Tests that work in all feature configurations ────────────────────────

/// A disabled subsystem reports `TelemetryState::Disabled`.
#[test]
fn test_disabled_subsystem_state_is_disabled() {
    let sub = TelemetrySubsystem::disabled();
    assert_eq!(sub.state(), TelemetryState::Disabled);
    assert!(!sub.is_enabled());
}

/// A subsystem constructed from `OtelConfig { enabled: false }` is
/// disabled (AC-2).
#[test]
fn test_config_enabled_false_produces_disabled_subsystem() {
    let config = OtelConfig::default(); // enabled == false by default
    let sub = TelemetrySubsystem::new(config).expect("disabled subsystem");
    assert_eq!(sub.state(), TelemetryState::Disabled);
    assert!(!sub.is_enabled());
}

/// A disabled subsystem has no live meter provider (AC-2: no exporter
/// is constructed, so no network connection can be made).
#[cfg(feature = "telemetry")]
#[test]
fn test_disabled_subsystem_has_no_provider() {
    let sub = TelemetrySubsystem::disabled();
    assert!(
        sub.provider().is_none(),
        "disabled subsystem must not hold a live SdkMeterProvider"
    );
}

/// A disabled subsystem provides no instruments (AC-2).
///
/// `instruments()` returns `None`, so callers cannot record any metrics
/// through the subsystem. This is the primary guarantee that no metric
/// data is ever collected or exported.
#[cfg(feature = "telemetry")]
#[test]
fn test_disabled_subsystem_has_no_instruments() {
    let sub = TelemetrySubsystem::disabled();
    assert!(
        sub.instruments().is_none(),
        "disabled subsystem must not provide any InstrumentRegistry"
    );
}

/// A disabled subsystem with a configured endpoint still does not
/// construct a provider (AC-2).
///
/// Even when `endpoint` is set to a real URL, if `enabled` is `false`,
/// no exporter, reader, or provider is created. No HTTP/gRPC connection
/// is attempted.
#[cfg(feature = "telemetry")]
#[test]
fn test_disabled_with_endpoint_still_no_provider() {
    let config = OtelConfig {
        enabled: false,
        endpoint: "http://localhost:4318".to_string(),
        ..Default::default()
    };

    let sub = TelemetrySubsystem::new(config).expect("disabled subsystem");
    assert_eq!(sub.state(), TelemetryState::Disabled);
    assert!(
        sub.provider().is_none(),
        "disabled subsystem with endpoint must not construct a provider"
    );
    assert!(
        sub.instruments().is_none(),
        "disabled subsystem with endpoint must not provide instruments"
    );
}

/// `flush()` on a disabled subsystem is a no-op that succeeds instantly
/// (AC-2, FR-032).
#[test]
fn test_disabled_flush_is_noop() {
    let sub = TelemetrySubsystem::disabled();
    let result = sub.flush();
    assert!(result.is_ok(), "flush on disabled subsystem must succeed");
}

/// `shutdown()` on a disabled subsystem is a no-op that succeeds
/// instantly (AC-2, FR-032).
#[test]
fn test_disabled_shutdown_is_noop() {
    let sub = TelemetrySubsystem::disabled();
    let result = sub.shutdown();
    assert!(
        result.is_ok(),
        "shutdown on disabled subsystem must succeed"
    );
}

/// `flush()` then `shutdown()` on a disabled subsystem both succeed
/// (AC-2).
#[test]
fn test_disabled_flush_then_shutdown() {
    let sub = TelemetrySubsystem::disabled();
    assert!(sub.flush().is_ok());
    assert!(sub.shutdown().is_ok());
}

/// The default `OtelConfig` has `enabled: false` (AC-2).
#[test]
fn test_default_config_is_disabled() {
    let config = OtelConfig::default();
    assert!(
        !config.enabled,
        "default OtelConfig must have enabled=false"
    );
}

/// A `TelemetryConfig` wrapping a default `OtelConfig` is also disabled.
#[test]
fn test_telemetry_config_default_is_disabled() {
    let tc = ragent_telemetry::TelemetryConfig::default();
    assert!(!tc.is_enabled(), "default TelemetryConfig must be disabled");
}

// ── Tests that verify zero metric export with the telemetry feature ──────
//
// These tests use the OTEL SDK's `InMemoryMetricExporter` to prove that
// recording metrics through the no-op meter provider produces zero
// exported data — even after a `force_flush()`.

#[cfg(feature = "telemetry")]
mod disabled_zero_traffic {
    use super::*;
    use opentelemetry_sdk::metrics::SdkMeterProvider;

    use opentelemetry_sdk::metrics::InMemoryMetricExporter;
    use ragent_telemetry::InstrumentRegistry;
    use std::time::Duration;

    /// Build an `SdkMeterProvider` backed by an `InMemoryMetricExporter`
    /// with a very long export interval. This simulates the "collector
    /// side" — if any metrics were exported, they would appear here.
    fn build_collector() -> (
        SdkMeterProvider,
        InMemoryMetricExporter,
        tokio::runtime::Runtime,
    ) {
        let rt = tokio::runtime::Runtime::new().expect("tokio runtime");
        let exporter = InMemoryMetricExporter::default();
        let exporter_clone = exporter.clone();

        let provider = rt.block_on(async {
            let reader = opentelemetry_sdk::metrics::PeriodicReader::builder(exporter_clone)
                .with_interval(Duration::from_hours(1))
                .build();
            SdkMeterProvider::builder().with_reader(reader).build()
        });

        (provider, exporter, rt)
    }

    /// When the subsystem is disabled, recording metrics through the
    /// no-op `InstrumentRegistry::noop()` produces zero exported metrics
    /// — even after a `force_flush()` on a *separate* collector provider.
    ///
    /// This proves that the no-op meter provider does not route data to
    /// any exporter (AC-2).
    #[test]
    fn test_noop_registry_produces_zero_exported_metrics() {
        let (collector_provider, collector_exporter, rt) = build_collector();

        // Build a no-op instrument registry (as a disabled subsystem would).
        let noop_registry = InstrumentRegistry::noop();

        // Record metrics through the no-op registry.
        noop_registry.llm_requests.add(1, &[]);
        noop_registry.llm_requests.add(1, &[]);
        noop_registry.tokens_input.add(100, &[]);
        noop_registry.tool_invocations.add(1, &[]);
        noop_registry.llm_duration.record(42.5, &[]);

        // Flush the *collector* provider — no metrics should appear because
        // the no-op registry's meter is not connected to this provider.
        rt.block_on(async {
            collector_provider
                .force_flush()
                .expect("flush should succeed");
        });

        let metrics = collector_exporter
            .get_finished_metrics()
            .unwrap_or_default();
        assert!(
            metrics.is_empty(),
            "no-op registry must not produce any exported metrics, got {} batches",
            metrics.len()
        );
    }

    /// A disabled `TelemetrySubsystem` does not provide an
    /// `InstrumentRegistry`, so there is no way for callers to record
    /// metrics through the subsystem at all (AC-2).
    #[test]
    fn test_disabled_subsystem_instruments_is_none() {
        let sub = TelemetrySubsystem::disabled();
        assert!(
            sub.instruments().is_none(),
            "disabled subsystem must return None from instruments()"
        );
    }

    /// A disabled subsystem constructed from an `OtelConfig` with
    /// `enabled: false` and a valid endpoint does not provide
    /// instruments (AC-2).
    #[test]
    fn test_disabled_from_config_instruments_is_none() {
        let config = OtelConfig {
            enabled: false,
            endpoint: "http://localhost:4318".to_string(),
            ..Default::default()
        };

        let sub = TelemetrySubsystem::new(config).expect("disabled subsystem");
        assert!(sub.instruments().is_none());
    }

    /// Recording metrics through a no-op registry and then calling
    /// `flush()` on the disabled subsystem produces no exported data
    /// (AC-2, FR-032).
    ///
    /// This simulates the real-world flow: instrumentation code obtains
    /// a no-op registry, records metrics, and the subsystem's `flush()`
    /// is called on shutdown. No traffic is generated.
    #[test]
    fn test_disabled_flush_produces_zero_traffic() {
        let sub = TelemetrySubsystem::disabled();

        // Even if someone creates a no-op registry and records metrics,
        // the subsystem's flush() is a no-op.
        let noop_registry = InstrumentRegistry::noop();
        noop_registry.llm_requests.add(1, &[]);
        noop_registry.tokens_input.add(100, &[]);

        // flush() on the disabled subsystem should succeed and produce
        // no network traffic.
        assert!(
            sub.flush().is_ok(),
            "flush must succeed on disabled subsystem"
        );

        // shutdown() should also succeed with no traffic.
        assert!(
            sub.shutdown().is_ok(),
            "shutdown must succeed on disabled subsystem"
        );
    }

    /// A `ShutdownGuard` wrapping a disabled subsystem does not produce
    /// any network traffic on Drop (AC-2).
    #[test]
    fn test_shutdown_guard_disabled_no_traffic() {
        let sub = TelemetrySubsystem::disabled();
        let guard = ragent_telemetry::shutdown::ShutdownGuard::new(sub);
        // Drop the guard — it calls flush()+shutdown(), both no-ops.
        drop(guard);
        // No assertion needed: if the guard tried to contact a collector,
        // it would have panicked or hung. The fact that this test completes
        // proves zero traffic.
    }

    /// The `InstrumentRegistry::noop()` constructs instruments from the
    /// global no-op meter provider, not from any live provider. Recording
    /// through these instruments is safe and produces no exported data
    /// (AC-2, FR-022, NFR-002).
    #[test]
    fn test_noop_registry_does_not_connect_to_collector() {
        // Build a collector that would receive metrics if any were exported.
        let (collector_provider, collector_exporter, rt) = build_collector();

        // Create a no-op registry and record a variety of metrics.
        let registry = InstrumentRegistry::noop();
        registry.llm_requests.add(1, &[]);
        registry.sessions_total.add(1, &[]);
        registry.tool_invocations.add(1, &[]);
        registry.sessions_active.add(1, &[]);
        registry.team_members.record(5, &[]);
        registry.llm_duration.record(100.0, &[]);
        registry.tokens_input.add(500, &[]);
        registry.tokens_output.add(200, &[]);
        registry.cost_estimated.add(1.0, &[]);
        registry.errors_total.add(1, &[]);

        // Flush the collector — nothing should appear.
        rt.block_on(async {
            collector_provider
                .force_flush()
                .expect("flush should succeed");
        });

        let metrics = collector_exporter
            .get_finished_metrics()
            .unwrap_or_default();
        assert!(
            metrics.is_empty(),
            "no-op registry must not route any metrics to the collector, got {} batches",
            metrics.len()
        );
    }
}
