//! Inline tests for `telemetry.rs` - compiled inside the module tree
//! via the `#[path]` delegation left in the source file
//! (ANTIPAT M2 inline-test migration; no behaviour change).

use super::*;

#[test]
fn test_otel_config_defaults_to_disabled() {
    let config = OtelConfig::default();
    assert!(
        !config.enabled,
        "OTEL should be disabled by default (FR-002)"
    );
    assert_eq!(config.endpoint, "http://localhost:4318");
    assert_eq!(config.protocol, OtelProtocol::Http);
    assert_eq!(config.export_interval_seconds, 30);
    assert_eq!(config.service_name, "ragent");
}

#[test]
fn test_otel_config_deserializes_partial_block() {
    let json = r#"{ "enabled": true }"#;
    let config: OtelConfig = serde_json::from_str(json).expect("should deserialize");
    assert!(config.enabled);
    // Absent fields fall back to defaults.
    assert_eq!(config.endpoint, "http://localhost:4318");
    assert_eq!(config.export_interval_seconds, 30);
}

#[test]
fn test_otel_config_deserializes_full_block() {
    let json = r#"{
        "enabled": true,
        "endpoint": "https://otel.example.com:4317",
        "protocol": "grpc",
        "export_interval_seconds": 15,
        "service_name": "my-ragent",
        "resource_attributes": { "deployment.environment": "production" },
        "metrics": { "ragent.tool.invocations": false }
    }"#;
    let config: OtelConfig = serde_json::from_str(json).expect("should deserialize");
    assert!(config.enabled);
    assert_eq!(config.endpoint, "https://otel.example.com:4317");
    assert_eq!(config.protocol, OtelProtocol::Grpc);
    assert_eq!(config.export_interval_seconds, 15);
    assert_eq!(config.service_name, "my-ragent");
    assert_eq!(
        config.resource_attributes.get("deployment.environment"),
        Some(&"production".to_string())
    );
    assert_eq!(config.metrics.get("ragent.tool.invocations"), Some(&false));
}

#[test]
fn test_otel_config_empty_json_uses_defaults() {
    let config: OtelConfig = serde_json::from_str("{}").expect("should deserialize");
    assert!(!config.enabled);
    assert_eq!(config.protocol, OtelProtocol::Http);
}

#[test]
fn test_telemetry_config_is_enabled() {
    let mut config = TelemetryConfig::default();
    assert!(!config.is_enabled());
    config.otel.enabled = true;
    assert!(config.is_enabled());
}

#[test]
fn test_telemetry_config_merge_overlay_enabled_takes_overlay() {
    let base = TelemetryConfig::default();
    let mut overlay = TelemetryConfig::default();
    overlay.otel.enabled = true;
    overlay.otel.endpoint = "https://collector:4318".to_string();

    let merged = TelemetryConfig::merge(&base, &overlay);
    assert!(merged.is_enabled());
    assert_eq!(merged.otel.endpoint, "https://collector:4318");
}

#[test]
fn test_telemetry_config_merge_overlay_disabled_preserves_base() {
    let mut base = TelemetryConfig::default();
    base.otel.enabled = true;
    base.otel.endpoint = "https://base:4318".to_string();

    let overlay = TelemetryConfig::default();

    let merged = TelemetryConfig::merge(&base, &overlay);
    assert!(merged.is_enabled(), "base enabled state preserved");
    assert_eq!(merged.otel.endpoint, "https://base:4318");
}

#[test]
fn test_telemetry_config_merge_unions_resource_attributes() {
    let mut base = TelemetryConfig::default();
    base.otel
        .resource_attributes
        .insert("service.name".to_string(), "ragent".to_string());

    let mut overlay = TelemetryConfig::default();
    overlay
        .otel
        .resource_attributes
        .insert("deployment.environment".to_string(), "staging".to_string());

    let merged = TelemetryConfig::merge(&base, &overlay);
    assert_eq!(merged.otel.resource_attributes.len(), 2);
    assert_eq!(
        merged
            .otel
            .resource_attributes
            .get("deployment.environment"),
        Some(&"staging".to_string())
    );
}

// -- OtelConfig::validate tests ---------------------------------------

#[test]
fn test_validate_disabled_config_has_no_problems() {
    let config = OtelConfig::default();
    assert!(
        config.validate().is_empty(),
        "disabled config should not validate"
    );
}

#[test]
fn test_validate_enabled_with_empty_endpoint_has_problems() {
    let config = OtelConfig {
        enabled: true,
        endpoint: String::new(),
        ..OtelConfig::default()
    };
    let problems = config.validate();
    assert!(
        problems.iter().any(|p| p.contains("endpoint")),
        "expected an endpoint problem, got {problems:?}"
    );
}

#[test]
fn test_validate_enabled_valid_config_no_problems() {
    let config = OtelConfig {
        enabled: true,
        endpoint: "http://localhost:4318".to_string(),
        ..OtelConfig::default()
    };
    assert!(
        config.validate().is_empty(),
        "valid config should have no problems, got {:?}",
        config.validate()
    );
}

#[test]
fn test_validate_rejects_zero_export_interval() {
    let config = OtelConfig {
        enabled: true,
        endpoint: "http://localhost:4318".to_string(),
        export_interval_seconds: 0,
        ..OtelConfig::default()
    };
    let problems = config.validate();
    assert!(
        problems
            .iter()
            .any(|p| p.contains("export_interval_seconds")),
        "expected an export_interval problem, got {problems:?}"
    );
}

#[test]
fn test_validate_rejects_non_http_endpoint() {
    let config = OtelConfig {
        enabled: true,
        endpoint: "ftp://bad:1234".to_string(),
        ..OtelConfig::default()
    };
    let problems = config.validate();
    assert!(
        problems
            .iter()
            .any(|p| p.contains("HTTP") || p.contains("HTTPS")),
        "expected an endpoint protocol problem, got {problems:?}"
    );
}

// -- TelemetryConfig::apply_legacy_flag tests ------------------------

#[test]
fn test_apply_legacy_flag_enables_when_otel_disabled() {
    let mut tc = TelemetryConfig::default();
    assert!(!tc.is_enabled());

    let activated = tc.apply_legacy_flag(true);
    assert!(activated, "legacy flag should activate telemetry");
    assert!(tc.is_enabled());
    // Default settings are used.
    assert_eq!(tc.otel.endpoint, "http://localhost:4318");
}

#[test]
fn test_apply_legacy_flag_noop_when_otel_already_enabled() {
    let mut tc = TelemetryConfig::default();
    tc.otel.enabled = true;
    tc.otel.endpoint = "https://custom:4318".to_string();

    let activated = tc.apply_legacy_flag(true);
    assert!(
        !activated,
        "should not report legacy activation when already enabled"
    );
    // Custom settings are preserved.
    assert_eq!(tc.otel.endpoint, "https://custom:4318");
}

#[test]
fn test_apply_legacy_flag_false_does_nothing() {
    let mut tc = TelemetryConfig::default();
    let activated = tc.apply_legacy_flag(false);
    assert!(!activated);
    assert!(!tc.is_enabled());
}
