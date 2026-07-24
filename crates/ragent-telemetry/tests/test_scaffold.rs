//! Scaffold smoke tests for the `ragent-telemetry` crate.
//!
//! These tests verify that the crate compiles, exports its public API, and
//! that the no-op subsystem works correctly without the `telemetry` feature
//! (NFR-002, NFR-005). They do not require a live OTLP endpoint.

use ragent_telemetry::{
    OtelConfig, OtelProtocol, TelemetryConfig, TelemetryState, TelemetrySubsystem,
};

#[test]
fn test_crate_exports_are_accessible() {
    // Ensure all re-exported types are usable from the crate root.
    let _config: OtelConfig = OtelConfig::default();
    let _proto: OtelProtocol = OtelProtocol::Http;
    let _tc: TelemetryConfig = TelemetryConfig::default();
    let _state: TelemetryState = TelemetryState::Disabled;
    let _sub: TelemetrySubsystem = TelemetrySubsystem::disabled();
}

#[test]
fn test_disabled_subsystem_has_zero_overhead() {
    // NFR-002: a disabled subsystem should be a cheap no-op.
    let sub = TelemetrySubsystem::disabled();
    assert!(!sub.is_enabled());
    assert_eq!(sub.state(), TelemetryState::Disabled);
    // shutdown on a disabled subsystem must succeed instantly.
    assert!(sub.shutdown().is_ok());
}

#[test]
fn test_enabled_without_feature_errors_gracefully() {
    let mut config = OtelConfig::default();
    config.enabled = true;

    #[cfg(not(feature = "telemetry"))]
    {
        let result = TelemetrySubsystem::new(config);
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            TelemetryError::FeatureNotEnabled
        ));
    }

    #[cfg(feature = "telemetry")]
    {
        // The PeriodicReader needs a Tokio runtime context.
        let rt = tokio::runtime::Runtime::new().expect("tokio runtime");
        let sub = rt.block_on(async {
            TelemetrySubsystem::new(config).expect("enabled subsystem should construct")
        });
        assert_eq!(sub.state(), TelemetryState::Enabled);
    }
}

#[test]
fn test_otel_protocol_serde_roundtrip() {
    let http = serde_json::to_string(&OtelProtocol::Http).expect("serialize");
    assert_eq!(http, "\"http\"");

    let grpc = serde_json::to_string(&OtelProtocol::Grpc).expect("serialize");
    assert_eq!(grpc, "\"grpc\"");

    let back: OtelProtocol = serde_json::from_str(&grpc).expect("deserialize");
    assert_eq!(back, OtelProtocol::Grpc);
}

#[test]
fn test_telemetry_config_default_disabled() {
    let tc = TelemetryConfig::default();
    assert!(!tc.is_enabled(), "default telemetry must be disabled");
}
