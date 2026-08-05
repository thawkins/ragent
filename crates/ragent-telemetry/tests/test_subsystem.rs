//! External tests for `tests` from `crates/ragent-telemetry/src/subsystem.rs`
//!
//! Relocated from the inline `#[cfg(test)]` module.

use ragent_telemetry::{OtelConfig, TelemetryState, TelemetrySubsystem};

#[cfg(not(feature = "telemetry"))]
use crate::TelemetryError;

#[test]
fn test_disabled_subsystem_is_noop() {
    let sub = TelemetrySubsystem::disabled();
    assert_eq!(sub.state(), TelemetryState::Disabled);
    assert!(!sub.is_enabled());
    assert!(sub.shutdown().is_ok());
}

#[test]
fn test_new_disabled_from_config() {
    let config = OtelConfig::default();
    let sub = TelemetrySubsystem::new(config).expect("disabled subsystem");
    assert_eq!(sub.state(), TelemetryState::Disabled);
}

#[test]
fn test_new_enabled_without_feature_returns_error() {
    let config = OtelConfig {
        enabled: true,
        ..Default::default()
    };

    #[cfg(not(feature = "telemetry"))]
    {
        let result = TelemetrySubsystem::new(config);
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(
            matches!(err, TelemetryError::FeatureNotEnabled),
            "expected FeatureNotEnabled, got {err:?}"
        );
    }

    #[cfg(feature = "telemetry")]
    {
        // The PeriodicReader needs a Tokio runtime context, so run inside
        // a tokio runtime.
        let rt = tokio::runtime::Runtime::new().expect("tokio runtime");
        let sub = rt.block_on(async {
            TelemetrySubsystem::new(config).expect("enabled subsystem should construct")
        });
        assert_eq!(sub.state(), TelemetryState::Enabled);
    }
}

#[test]
fn test_default_is_disabled() {
    let sub = TelemetrySubsystem::default();
    assert!(!sub.is_enabled());
}

#[test]
fn test_config_accessor() {
    let config = OtelConfig {
        service_name: "test-agent".to_string(),
        ..Default::default()
    };
    let sub = TelemetrySubsystem::new(config).expect("disabled subsystem");
    assert_eq!(sub.config().service_name, "test-agent");
}

#[test]
fn test_debug_format_includes_state() {
    let sub = TelemetrySubsystem::disabled();
    let debug = format!("{sub:?}");
    assert!(
        debug.contains("Disabled"),
        "debug should include state: {debug}"
    );
}

#[cfg(feature = "telemetry")]
#[test]
fn test_enabled_subsystem_has_provider() {
    let config = OtelConfig {
        enabled: true,
        endpoint: "http://localhost:4318".to_string(),
        ..Default::default()
    };

    // The PeriodicReader needs a Tokio runtime context.
    let rt = tokio::runtime::Runtime::new().expect("tokio runtime");
    let sub = rt.block_on(async { TelemetrySubsystem::new(config).expect("enabled subsystem") });
    assert_eq!(sub.state(), TelemetryState::Enabled);
    assert!(
        sub.provider().is_some(),
        "enabled subsystem should have a provider"
    );
}

#[cfg(feature = "telemetry")]
#[test]
fn test_enabled_subsystem_shutdown_succeeds() {
    let config = OtelConfig {
        enabled: true,
        endpoint: "http://localhost:4318".to_string(),
        ..Default::default()
    };

    let rt = tokio::runtime::Runtime::new().expect("tokio runtime");
    let sub = rt.block_on(async { TelemetrySubsystem::new(config).expect("enabled subsystem") });
    // shutdown should succeed even though there's no real collector
    assert!(sub.shutdown().is_ok());
}

#[cfg(feature = "telemetry")]
#[test]
fn test_invalid_endpoint_returns_error() {
    let config = OtelConfig {
        enabled: true,
        endpoint: "ftp://bad".to_string(),
        ..Default::default()
    };

    let result = TelemetrySubsystem::new(config);
    assert!(result.is_err(), "invalid endpoint should error");
}

#[cfg(feature = "telemetry")]
#[test]
fn test_empty_endpoint_returns_error() {
    let config = OtelConfig {
        enabled: true,
        endpoint: String::new(),
        ..Default::default()
    };

    let result = TelemetrySubsystem::new(config);
    assert!(result.is_err(), "empty endpoint should error");
}

#[cfg(feature = "telemetry")]
#[test]
fn test_http_protocol_builds_successfully() {
    // FR-023: OTLP/HTTP exporter wiring must construct without error
    // when a valid HTTP endpoint is provided.
    let config = OtelConfig {
        enabled: true,
        endpoint: "http://localhost:4318".to_string(),
        protocol: ragent_config::OtelProtocol::Http,
        ..Default::default()
    };

    let rt = tokio::runtime::Runtime::new().expect("tokio runtime");
    let sub = rt.block_on(async {
        TelemetrySubsystem::new(config).expect("HTTP subsystem should construct")
    });
    assert_eq!(sub.state(), TelemetryState::Enabled);
    assert!(
        sub.provider().is_some(),
        "HTTP subsystem should have a live provider"
    );
}

#[cfg(feature = "telemetry")]
#[test]
fn test_https_endpoint_builds_successfully() {
    // FR-023: HTTPS endpoints are also valid for OTLP/HTTP.
    let config = OtelConfig {
        enabled: true,
        endpoint: "https://collector.example.com:4318".to_string(),
        protocol: ragent_config::OtelProtocol::Http,
        ..Default::default()
    };

    let rt = tokio::runtime::Runtime::new().expect("tokio runtime");
    let sub = rt.block_on(async {
        TelemetrySubsystem::new(config).expect("HTTPS subsystem should construct")
    });
    assert_eq!(sub.state(), TelemetryState::Enabled);
}

#[cfg(feature = "telemetry")]
#[test]
fn test_grpc_protocol_builds_successfully() {
    // FR-024: OTLP/gRPC exporter wiring must construct without error
    // when a valid gRPC endpoint is provided. Tonic uses connect_lazy(),
    // so no actual connection is made at construction time.
    let config = OtelConfig {
        enabled: true,
        endpoint: "http://localhost:4317".to_string(),
        protocol: ragent_config::OtelProtocol::Grpc,
        ..Default::default()
    };

    let rt = tokio::runtime::Runtime::new().expect("tokio runtime");
    let sub = rt.block_on(async {
        TelemetrySubsystem::new(config).expect("gRPC subsystem should construct")
    });
    assert_eq!(sub.state(), TelemetryState::Enabled);
    assert!(
        sub.provider().is_some(),
        "gRPC subsystem should have a live provider"
    );
}

#[cfg(feature = "telemetry")]
#[test]
fn test_http_exporter_with_custom_endpoint() {
    // Verify the exporter uses the configured endpoint, not a default.
    let config = OtelConfig {
        enabled: true,
        endpoint: "http://my-collector:9999".to_string(),
        protocol: ragent_config::OtelProtocol::Http,
        ..Default::default()
    };

    let rt = tokio::runtime::Runtime::new().expect("tokio runtime");
    let sub =
        rt.block_on(async { TelemetrySubsystem::new(config).expect("subsystem should construct") });
    // The provider should exist; we can't inspect the internal endpoint
    // without a mock collector, but construction success proves the
    // endpoint was accepted.
    assert!(sub.provider().is_some());
    assert_eq!(sub.config().endpoint, "http://my-collector:9999");
}
