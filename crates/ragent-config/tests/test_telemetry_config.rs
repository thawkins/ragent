//! Integration tests for the `telemetry.otel` configuration schema (T-002).
//!
//! These tests verify that the `telemetry` block parses correctly inside a
//! full [`Config`], that defaults are disabled-by-default (FR-002), and that
//! the legacy `experimental.open_telemetry` flag maps via
//! [`TelemetryConfig::apply_legacy_flag`] (T-019 / FR-002).

use ragent_config::{Config, OtelConfig, OtelProtocol, TelemetryConfig};

// ── Config-level deserialization ──────────────────────────────────────────

#[test]
fn test_config_has_telemetry_field_defaulting_to_disabled() {
    let config = Config::default();
    assert!(
        !config.telemetry.is_enabled(),
        "telemetry must be disabled by default in a fresh Config (FR-002)"
    );
}

#[test]
fn test_config_deserializes_telemetry_otel_block() {
    let json = r#"{
        "telemetry": {
            "otel": {
                "enabled": true,
                "endpoint": "https://collector.example.com:4318",
                "protocol": "grpc",
                "export_interval_seconds": 15,
                "service_name": "prod-ragent"
            }
        }
    }"#;

    let config: Config = serde_json::from_str(json).expect("should deserialize");

    assert!(config.telemetry.is_enabled());
    assert_eq!(
        config.telemetry.otel.endpoint,
        "https://collector.example.com:4318"
    );
    assert_eq!(config.telemetry.otel.protocol, OtelProtocol::Grpc);
    assert_eq!(config.telemetry.otel.export_interval_seconds, 15);
    assert_eq!(config.telemetry.otel.service_name, "prod-ragent");
}

#[test]
fn test_config_deserializes_telemetry_with_partial_otel_block() {
    let json = r#"{ "telemetry": { "otel": { "enabled": true } } }"#;
    let config: Config = serde_json::from_str(json).expect("should deserialize");

    assert!(config.telemetry.is_enabled());
    // Absent fields fall back to defaults.
    assert_eq!(config.telemetry.otel.endpoint, "http://localhost:4318");
    assert_eq!(config.telemetry.otel.protocol, OtelProtocol::Http);
    assert_eq!(config.telemetry.otel.export_interval_seconds, 30);
}

#[test]
fn test_config_deserializes_telemetry_with_resource_attributes() {
    let json = r#"{
        "telemetry": {
            "otel": {
                "enabled": true,
                "endpoint": "http://localhost:4318",
                "resource_attributes": {
                    "deployment.environment": "production",
                    "host.id": "web-01"
                }
            }
        }
    }"#;

    let config: Config = serde_json::from_str(json).expect("should deserialize");
    assert!(config.telemetry.is_enabled());
    assert_eq!(
        config
            .telemetry
            .otel
            .resource_attributes
            .get("deployment.environment"),
        Some(&"production".to_string())
    );
    assert_eq!(
        config.telemetry.otel.resource_attributes.get("host.id"),
        Some(&"web-01".to_string())
    );
}

#[test]
fn test_config_deserializes_telemetry_with_metric_toggles() {
    let json = r#"{
        "telemetry": {
            "otel": {
                "enabled": true,
                "endpoint": "http://localhost:4318",
                "metrics": {
                    "ragent.tool.invocations": false,
                    "ragent.tokens.input": true
                }
            }
        }
    }"#;

    let config: Config = serde_json::from_str(json).expect("should deserialize");
    assert!(config.telemetry.is_enabled());
    assert_eq!(
        config.telemetry.otel.metrics.get("ragent.tool.invocations"),
        Some(&false)
    );
    assert_eq!(
        config.telemetry.otel.metrics.get("ragent.tokens.input"),
        Some(&true)
    );
}

#[test]
fn test_config_without_telemetry_block_uses_defaults() {
    let json = r#"{ "default_agent": "coder" }"#;
    let config: Config = serde_json::from_str(json).expect("should deserialize");
    assert!(!config.telemetry.is_enabled());
    assert_eq!(config.telemetry.otel.endpoint, "http://localhost:4318");
}

// ── Config merge ─────────────────────────────────────────────────────────

#[test]
fn test_config_merge_preserves_telemetry_enabled() {
    let mut base = Config::default();
    base.telemetry.otel.enabled = true;
    base.telemetry.otel.endpoint = "https://base:4318".to_string();

    let overlay = Config::default();

    let merged = Config::merge(base, overlay);
    assert!(
        merged.telemetry.is_enabled(),
        "base enabled state preserved through merge"
    );
    assert_eq!(merged.telemetry.otel.endpoint, "https://base:4318");
}

#[test]
fn test_config_merge_overlay_enables_telemetry() {
    let base = Config::default();

    let mut overlay = Config::default();
    overlay.telemetry.otel.enabled = true;
    overlay.telemetry.otel.endpoint = "https://overlay:4318".to_string();

    let merged = Config::merge(base, overlay);
    assert!(merged.telemetry.is_enabled());
    assert_eq!(merged.telemetry.otel.endpoint, "https://overlay:4318");
}

// ── Legacy flag (T-019) ──────────────────────────────────────────────────

#[test]
fn test_legacy_flag_enables_telemetry_when_otel_disabled() {
    let mut tc = TelemetryConfig::default();
    assert!(!tc.is_enabled());

    let activated = tc.apply_legacy_flag(true);
    assert!(activated, "legacy flag should activate telemetry");
    assert!(tc.is_enabled());
    // Default settings are used.
    assert_eq!(tc.otel.endpoint, "http://localhost:4318");
}

#[test]
fn test_legacy_flag_does_not_override_explicit_otel_config() {
    let mut tc = TelemetryConfig::default();
    tc.otel.enabled = true;
    tc.otel.endpoint = "https://custom:4318".to_string();

    let activated = tc.apply_legacy_flag(true);
    assert!(
        !activated,
        "should not report legacy activation when already enabled"
    );
    assert_eq!(tc.otel.endpoint, "https://custom:4318");
}

#[test]
fn test_legacy_flag_false_does_not_enable() {
    let mut tc = TelemetryConfig::default();
    let activated = tc.apply_legacy_flag(false);
    assert!(!activated);
    assert!(!tc.is_enabled());
}

// ── OtelConfig standalone ─────────────────────────────────────────────────

#[test]
fn test_otel_config_validate_accepts_valid_enabled() {
    let config = OtelConfig {
        enabled: true,
        endpoint: "http://localhost:4318".to_string(),
        ..OtelConfig::default()
    };
    assert!(
        config.validate().is_empty(),
        "valid config should have no problems"
    );
}

#[test]
fn test_otel_config_validate_rejects_bad_endpoint() {
    let config = OtelConfig {
        enabled: true,
        endpoint: "ftp://bad".to_string(),
        ..OtelConfig::default()
    };
    let problems = config.validate();
    assert!(
        problems
            .iter()
            .any(|p| p.contains("HTTP") || p.contains("HTTPS")),
        "expected an endpoint protocol problem"
    );
}

#[test]
fn test_otel_config_serde_roundtrip() {
    let config = OtelConfig {
        enabled: true,
        endpoint: "https://otel:4317".to_string(),
        protocol: OtelProtocol::Grpc,
        export_interval_seconds: 60,
        service_name: "test".to_string(),
        ..OtelConfig::default()
    };

    let json = serde_json::to_string(&config).expect("serialize");
    let back: OtelConfig = serde_json::from_str(&json).expect("deserialize");
    assert_eq!(config, back);
}
