//! Integration tests for OTLP exporter wiring (T-004, T-005, FR-005, FR-023, FR-024).
//!
//! These tests verify that the telemetry subsystem constructs a working
//! `SdkMeterProvider` backed by OTLP exporters (HTTP and gRPC) when the
//! `telemetry` feature is enabled and a valid endpoint is configured.
//!
//! They do **not** require a live OTLP collector — the exporter and
//! `PeriodicReader` are constructed but no export is triggered (the export
//! interval is long enough that no background export fires during the test).
//! Tonic uses `connect_lazy()`, so gRPC construction does not attempt a
//! connection either.

#![cfg(feature = "telemetry")]

use ragent_telemetry::{OtelConfig, OtelProtocol, TelemetryState, TelemetrySubsystem};

/// A valid HTTP endpoint constructs an enabled subsystem with a live provider.
#[test]
fn test_http_exporter_constructs_with_valid_endpoint() {
    let config = OtelConfig {
        enabled: true,
        endpoint: "http://localhost:4318".to_string(),
        protocol: OtelProtocol::Http,
        ..Default::default()
    };

    let rt = tokio::runtime::Runtime::new().expect("tokio runtime");
    let sub = rt.block_on(async { TelemetrySubsystem::new(config).expect("should construct") });

    assert_eq!(sub.state(), TelemetryState::Enabled);
    assert!(
        sub.provider().is_some(),
        "enabled subsystem must hold a live meter provider"
    );
}

/// An HTTPS endpoint is also accepted for OTLP/HTTP (FR-023).
#[test]
fn test_http_exporter_accepts_https() {
    let config = OtelConfig {
        enabled: true,
        endpoint: "https://collector.example.com:4318".to_string(),
        protocol: OtelProtocol::Http,
        ..Default::default()
    };

    let rt = tokio::runtime::Runtime::new().expect("tokio runtime");
    let sub = rt.block_on(async { TelemetrySubsystem::new(config).expect("should construct") });

    assert_eq!(sub.state(), TelemetryState::Enabled);
}

/// A non-HTTP/HTTPS endpoint is rejected before the exporter is built.
#[test]
fn test_http_exporter_rejects_non_http_endpoint() {
    let config = OtelConfig {
        enabled: true,
        endpoint: "ftp://bad:1234".to_string(),
        protocol: OtelProtocol::Http,
        ..Default::default()
    };

    let result = TelemetrySubsystem::new(config);
    assert!(result.is_err(), "non-HTTP endpoint should be rejected");
}

/// An empty endpoint is rejected (FR-031 / FR-033 robustness).
#[test]
fn test_http_exporter_rejects_empty_endpoint() {
    let config = OtelConfig {
        enabled: true,
        endpoint: String::new(),
        protocol: OtelProtocol::Http,
        ..Default::default()
    };

    let result = TelemetrySubsystem::new(config);
    assert!(result.is_err(), "empty endpoint should be rejected");
}

/// A valid gRPC endpoint constructs an enabled subsystem with a live provider
/// (FR-024). Tonic uses `connect_lazy()`, so no connection is attempted at
/// construction time.
#[test]
fn test_grpc_exporter_constructs_with_valid_endpoint() {
    let config = OtelConfig {
        enabled: true,
        endpoint: "http://localhost:4317".to_string(),
        protocol: OtelProtocol::Grpc,
        ..Default::default()
    };

    let rt = tokio::runtime::Runtime::new().expect("tokio runtime");
    let sub = rt.block_on(async { TelemetrySubsystem::new(config).expect("should construct") });

    assert_eq!(sub.state(), TelemetryState::Enabled);
    assert!(
        sub.provider().is_some(),
        "gRPC subsystem must hold a live meter provider"
    );
}

/// The subsystem can be cleanly shut down after gRPC exporter construction.
#[test]
fn test_grpc_exporter_shutdown_is_clean() {
    let config = OtelConfig {
        enabled: true,
        endpoint: "http://localhost:4317".to_string(),
        protocol: OtelProtocol::Grpc,
        ..Default::default()
    };

    let rt = tokio::runtime::Runtime::new().expect("tokio runtime");
    let sub = rt.block_on(async { TelemetrySubsystem::new(config).expect("should construct") });

    assert!(sub.shutdown().is_ok(), "shutdown should be clean");
}

/// A custom gRPC endpoint is preserved in the config accessor.
#[test]
fn test_grpc_exporter_custom_endpoint_preserved() {
    let config = OtelConfig {
        enabled: true,
        endpoint: "http://my-grpc-collector:9999".to_string(),
        protocol: OtelProtocol::Grpc,
        ..Default::default()
    };

    let rt = tokio::runtime::Runtime::new().expect("tokio runtime");
    let sub = rt.block_on(async { TelemetrySubsystem::new(config).expect("should construct") });

    assert_eq!(sub.config().endpoint, "http://my-grpc-collector:9999");
    assert_eq!(sub.config().protocol, OtelProtocol::Grpc);
}

/// An HTTPS endpoint is also accepted for OTLP/gRPC (TLS via tonic).
#[test]
fn test_grpc_exporter_accepts_https() {
    let config = OtelConfig {
        enabled: true,
        endpoint: "https://grpc-collector.example.com:4317".to_string(),
        protocol: OtelProtocol::Grpc,
        ..Default::default()
    };

    let rt = tokio::runtime::Runtime::new().expect("tokio runtime");
    let sub = rt.block_on(async { TelemetrySubsystem::new(config).expect("should construct") });

    assert_eq!(sub.state(), TelemetryState::Enabled);
}

/// The subsystem can be cleanly shut down after HTTP exporter construction.
#[test]
fn test_http_exporter_shutdown_is_clean() {
    let config = OtelConfig {
        enabled: true,
        endpoint: "http://localhost:4318".to_string(),
        protocol: OtelProtocol::Http,
        ..Default::default()
    };

    let rt = tokio::runtime::Runtime::new().expect("tokio runtime");
    let sub = rt.block_on(async { TelemetrySubsystem::new(config).expect("should construct") });

    // Shutdown must succeed even with no live collector.
    assert!(sub.shutdown().is_ok(), "shutdown should be clean");
}

/// A custom (non-default) HTTP endpoint is accepted and reflected in the
/// config accessor.
#[test]
fn test_http_exporter_custom_endpoint_preserved() {
    let config = OtelConfig {
        enabled: true,
        endpoint: "http://my-collector:9999".to_string(),
        protocol: OtelProtocol::Http,
        ..Default::default()
    };

    let rt = tokio::runtime::Runtime::new().expect("tokio runtime");
    let sub = rt.block_on(async { TelemetrySubsystem::new(config).expect("should construct") });

    assert_eq!(sub.config().endpoint, "http://my-collector:9999");
    assert_eq!(sub.config().protocol, OtelProtocol::Http);
}
