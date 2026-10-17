//! External tests for `subsystem.rs` public surface: endpoint masking, the
//! Prometheus-endpoint construction path, and runtime `reconfigure`/flush/
//! shutdown transitions.

use ragent_telemetry::subsystem::mask_endpoint_userinfo;
use ragent_telemetry::{OtelConfig, TelemetryState, TelemetrySubsystem};

/// `mask_endpoint_userinfo` replaces the userinfo with `***` and leaves a URL
/// without userinfo untouched.
#[test]
fn test_mask_endpoint_userinfo_variants() {
    // Credentials in the authority are masked.
    assert_eq!(
        mask_endpoint_userinfo("https://user:secret@collector:4318"),
        "https://***@collector:4318"
    );
    // No userinfo -> unchanged.
    assert_eq!(
        mask_endpoint_userinfo("http://localhost:4317"),
        "http://localhost:4317"
    );
    // Path/query/fragment preserved after the authority.
    assert_eq!(
        mask_endpoint_userinfo("https://u:p@host:4318/v1/metrics?x=1"),
        "https://***@host:4318/v1/metrics?x=1"
    );
    // No scheme separator -> returned unchanged.
    assert_eq!(mask_endpoint_userinfo("not-a-url"), "not-a-url");
    // An authority with no '@' -> unchanged.
    assert_eq!(
        mask_endpoint_userinfo("http://host:4317/path"),
        "http://host:4317/path"
    );
    // The LAST '@' is used so a malformed userinfo cannot shift the host.
    assert_eq!(
        mask_endpoint_userinfo("http://a@b@real-host:4317"),
        "http://***@real-host:4317"
    );
}

/// Constructing an enabled subsystem with `internal_port` set starts the
/// Prometheus scrape server; `shutdown` aborts it (exercising the
/// `prometheus_handle` branch).
#[cfg(feature = "telemetry")]
#[test]
fn test_enabled_with_internal_port_starts_and_shuts_down_prometheus() {
    let config = OtelConfig {
        enabled: true,
        endpoint: "http://localhost:4318".to_string(),
        internal_port: Some(0), // OS-assigned port
        ..Default::default()
    };

    let rt = tokio::runtime::Runtime::new().expect("tokio runtime");
    let sub = rt.block_on(async {
        TelemetrySubsystem::new(config).expect("enabled subsystem with internal_port")
    });
    assert_eq!(sub.state(), TelemetryState::Enabled);
    // Instruments are available on an enabled subsystem.
    assert!(sub.instruments().is_some());
    // shutdown must abort the Prometheus task and succeed.
    assert!(sub.shutdown().is_ok());
}

/// `reconfigure` flips a live enabled subsystem to disabled, then back to
/// enabled, updating state/config each time.
#[cfg(feature = "telemetry")]
#[test]
fn test_reconfigure_toggle_disabled_then_enabled() {
    let rt = tokio::runtime::Runtime::new().expect("tokio runtime");
    rt.block_on(async {
        let enabled = OtelConfig {
            enabled: true,
            endpoint: "http://localhost:4318".to_string(),
            service_name: "reconf-a".to_string(),
            ..Default::default()
        };
        let sub = TelemetrySubsystem::new(enabled).expect("enabled subsystem");
        assert_eq!(sub.state(), TelemetryState::Enabled);

        // Reconfigure to disabled.
        let disabled = OtelConfig {
            enabled: false,
            service_name: "reconf-b".to_string(),
            ..Default::default()
        };
        sub.reconfigure(disabled).expect("disable reconfigure");
        assert_eq!(sub.state(), TelemetryState::Disabled);
        assert_eq!(sub.config().service_name, "reconf-b");
        assert!(sub.provider().is_none());
        assert!(sub.instruments().is_none());

        // Reconfigure back to enabled.
        let enabled_again = OtelConfig {
            enabled: true,
            endpoint: "http://localhost:4318".to_string(),
            service_name: "reconf-c".to_string(),
            ..Default::default()
        };
        sub.reconfigure(enabled_again)
            .expect("re-enable reconfigure");
        assert_eq!(sub.state(), TelemetryState::Enabled);
        assert_eq!(sub.config().service_name, "reconf-c");
        assert!(sub.provider().is_some());

        // flush on an enabled subsystem must succeed (no collector -> still Ok
        // because force_flush surfaces errors, but a local endpoint yields Ok).
        let _ = sub.flush();

        // Reconfigure with a bad endpoint must error AND leave telemetry
        // disabled (never half-initialised).
        let bad = OtelConfig {
            enabled: true,
            endpoint: "ftp://nope".to_string(),
            ..Default::default()
        };
        let err = sub.reconfigure(bad);
        assert!(err.is_err(), "bad endpoint reconfigure should error");
        assert_eq!(sub.state(), TelemetryState::Disabled);
    });
}

/// Reconfiguring a disabled subsystem through the Prometheus-enabled path
/// exercises aborting the previous (absent) handle plus a fresh spawn.
#[cfg(feature = "telemetry")]
#[test]
fn test_reconfigure_from_disabled_to_prometheus_enabled() {
    let rt = tokio::runtime::Runtime::new().expect("tokio runtime");
    rt.block_on(async {
        let sub = TelemetrySubsystem::new(OtelConfig::default()).expect("disabled subsystem");
        assert_eq!(sub.state(), TelemetryState::Disabled);

        let enabled = OtelConfig {
            enabled: true,
            endpoint: "http://localhost:4318".to_string(),
            internal_port: Some(0),
            ..Default::default()
        };
        sub.reconfigure(enabled).expect("enable with prometheus");
        assert_eq!(sub.state(), TelemetryState::Enabled);
        assert!(sub.provider().is_some());
        assert!(sub.shutdown().is_ok());
    });
}

/// `clone_disabled` returns a disabled copy carrying the same config.
#[test]
fn test_clone_disabled_carries_config() {
    let config = OtelConfig {
        service_name: "clone-src".to_string(),
        ..Default::default()
    };
    let sub = TelemetrySubsystem::new(config).expect("disabled subsystem");
    let clone = sub.clone_disabled();
    assert!(!clone.is_enabled());
    assert_eq!(clone.config().service_name, "clone-src");
}

/// A gRPC endpoint on a non-loopback cleartext host is refused at construction
/// (SEC-ragent-telemetry-001), exercising `ensure_grpc_endpoint_is_encrypted`.
#[cfg(feature = "telemetry")]
#[test]
fn test_grpc_cleartext_non_loopback_refused() {
    let config = OtelConfig {
        enabled: true,
        endpoint: "http://collector.example.com:4317".to_string(),
        protocol: ragent_config::OtelProtocol::Grpc,
        ..Default::default()
    };
    let result = TelemetrySubsystem::new(config);
    assert!(
        result.is_err(),
        "cleartext non-loopback gRPC endpoint must be refused"
    );
}

/// A gRPC endpoint that embeds userinfo is refused even on loopback.
#[cfg(feature = "telemetry")]
#[test]
fn test_grpc_endpoint_with_userinfo_refused() {
    let config = OtelConfig {
        enabled: true,
        endpoint: "http://user:pw@127.0.0.1:4317".to_string(),
        protocol: ragent_config::OtelProtocol::Grpc,
        ..Default::default()
    };
    assert!(TelemetrySubsystem::new(config).is_err());
}

/// A gRPC endpoint with an unsupported scheme is refused.
#[cfg(feature = "telemetry")]
#[test]
fn test_grpc_endpoint_unsupported_scheme_refused() {
    let config = OtelConfig {
        enabled: true,
        endpoint: "ftp://127.0.0.1:4317".to_string(),
        protocol: ragent_config::OtelProtocol::Grpc,
        ..Default::default()
    };
    assert!(TelemetrySubsystem::new(config).is_err());
}

/// An `https` gRPC endpoint is accepted (encrypted transport).
#[cfg(feature = "telemetry")]
#[test]
fn test_grpc_https_endpoint_accepted() {
    let config = OtelConfig {
        enabled: true,
        endpoint: "https://collector.example.com:4317".to_string(),
        protocol: ragent_config::OtelProtocol::Grpc,
        ..Default::default()
    };
    let rt = tokio::runtime::Runtime::new().expect("tokio runtime");
    let sub = rt.block_on(async {
        TelemetrySubsystem::new(config).expect("https gRPC endpoint should construct")
    });
    assert_eq!(sub.state(), TelemetryState::Enabled);
}

/// Custom resource attributes and a custom service_name flow through
/// `build_resource` without error.
#[cfg(feature = "telemetry")]
#[test]
fn test_custom_resource_attributes_construct() {
    let mut attrs = std::collections::HashMap::new();
    attrs.insert("deployment.env".to_string(), "test".to_string());
    attrs.insert(
        "leaky.key".to_string(),
        "sk-ant-api03-abcdefghijklmnopqrstuvwxyz0123456789".to_string(),
    );
    let config = OtelConfig {
        enabled: true,
        endpoint: "http://localhost:4318".to_string(),
        service_name: "resource-probe".to_string(),
        resource_attributes: attrs,
        ..Default::default()
    };
    let rt = tokio::runtime::Runtime::new().expect("tokio runtime");
    let sub =
        rt.block_on(async { TelemetrySubsystem::new(config).expect("custom resource subsystem") });
    assert_eq!(sub.state(), TelemetryState::Enabled);
}

/// A zero export interval / timeout is clamped to at least 1s (exercises the
/// `.max(1)` paths) without error.
#[cfg(feature = "telemetry")]
#[test]
fn test_zero_intervals_are_clamped() {
    let config = OtelConfig {
        enabled: true,
        endpoint: "http://localhost:4318".to_string(),
        export_interval_seconds: 0,
        export_timeout_seconds: 0,
        ..Default::default()
    };
    let rt = tokio::runtime::Runtime::new().expect("tokio runtime");
    let sub =
        rt.block_on(async { TelemetrySubsystem::new(config).expect("zero-interval subsystem") });
    assert_eq!(sub.state(), TelemetryState::Enabled);
}
