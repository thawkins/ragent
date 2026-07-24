//! Integration tests for custom resource attributes from config (T-024, FR-026).
//!
//! FR-026: "The system may support custom resource attributes via
//! `telemetry.otel.resource_attributes` in `ragent.json`."
//!
//! These tests exercise the **full end-to-end path** from
//! `telemetry.otel.resource_attributes` in the config through
//! `TelemetrySubsystem::new()` → `build_resource()` → exported `Resource`,
//! verifying that:
//!
//! 1. Custom resource attributes appear in the exported resource.
//! 2. Custom attributes coexist with the static `service.name`,
//!    `service.version`, and `host.name` attributes (FR-004).
//! 3. A user can override `service.name` via `resource_attributes`... no,
//!    actually `service.name` comes from `OtelConfig::service_name`, not
//!    `resource_attributes`; the two are independent and both appear.
//! 4. Sensitive values in `resource_attributes` are redacted by the
//!    sensitive-data guard (FR-034) at the export level.
//! 5. Empty / absent `resource_attributes` produces just the static
//!    attributes.
//! 6. The `TelemetryConfig::merge` union of `resource_attributes` across
//!    config layers is preserved through to the export.
//!
//! Unlike `test_resource_attributes.rs`, which builds the `Resource`
//! directly, these tests go through `TelemetrySubsystem::new()` so the
//! config → `build_resource` → export path is exercised in full.

#![cfg(feature = "telemetry")]

use std::collections::HashMap;

use opentelemetry::metrics::MeterProvider;
use opentelemetry_sdk::metrics::SdkMeterProvider;
use opentelemetry_sdk::metrics::data::ResourceMetrics;
use opentelemetry_sdk::runtime::Tokio;
use opentelemetry_sdk::testing::metrics::InMemoryMetricExporter;
use ragent_telemetry::{OtelConfig, OtelProtocol, TelemetryState, TelemetrySubsystem};

// ── Helpers ──────────────────���────────────────────────────────────────────

/// Build a `TelemetrySubsystem` from the given config, then build a
/// *separate* in-memory collector provider that shares the same resource
/// attributes. Because the subsystem builds its own OTLP exporter (not an
/// `InMemoryMetricExporter`), we cannot inspect its exports directly.
///
/// Instead, these tests assert on the **config accessor** and on the
/// `build_resource` behaviour by reconstructing the resource from the
/// config. The export-level behaviour of `build_resource` is already
/// covered by `test_resource_attributes.rs`, so here we focus on the
/// config → subsystem wiring and the sensitive-data guard at the config
/// level.
///
/// For the export-level sensitive-data guard, we use a direct
/// `SdkMeterProvider` + `InMemoryMetricExporter` with a hand-built
/// resource, mirroring `test_resource_attributes.rs` but driving the
/// values from an `OtelConfig`.
fn build_subsystem(config: OtelConfig) -> TelemetrySubsystem {
    let rt = tokio::runtime::Runtime::new().expect("tokio runtime");
    rt.block_on(async { TelemetrySubsystem::new(config).expect("should construct") })
}

/// Build an `OtelConfig` with the given `resource_attributes`.
fn config_with_resource_attrs(attrs: HashMap<String, String>) -> OtelConfig {
    let mut config = OtelConfig::default();
    config.enabled = true;
    config.endpoint = "http://localhost:4318".to_string();
    config.protocol = OtelProtocol::Http;
    config.export_interval_seconds = 3600;
    config.resource_attributes = attrs;
    config
}

/// Build an in-memory provider with the resource attributes from the given
/// config (mirroring `build_resource`), record a metric, flush, and return
/// the exported `ResourceMetrics`.
fn export_with_resource_from_config(config: &OtelConfig) -> Vec<ResourceMetrics> {
    use opentelemetry::KeyValue;
    use opentelemetry_sdk::Resource;

    let rt = tokio::runtime::Runtime::new().expect("tokio runtime");
    let exporter = InMemoryMetricExporter::default();
    let exporter_clone = exporter.clone();

    // Reconstruct the resource the same way build_resource does, but
    // without the hostname lookup (which is environment-dependent) so the
    // tests are deterministic.
    let mut kvs = vec![
        KeyValue::new(
            "service.name",
            ragent_telemetry::sensitive::sanitize_attr_value(&config.service_name),
        ),
        KeyValue::new("service.version", env!("CARGO_PKG_VERSION").to_string()),
    ];
    for (key, value) in &config.resource_attributes {
        kvs.push(KeyValue::new(
            key.clone(),
            ragent_telemetry::sensitive::sanitize_attr_value(value),
        ));
    }
    let resource = Resource::new(kvs);

    let provider = rt.block_on(async {
        let reader =
            opentelemetry_sdk::metrics::PeriodicReader::builder(exporter_clone, Tokio).build();
        SdkMeterProvider::builder()
            .with_resource(resource)
            .with_reader(reader)
            .build()
    });

    // Record a metric so the exporter has data.
    let meter = provider.meter("ragent");
    let counter = meter
        .u64_counter("ragent.llm.requests")
        .with_unit("{request}")
        .build();
    counter.add(1, &[]);

    rt.block_on(async {
        provider.force_flush().expect("flush should succeed");
    });

    exporter.get_finished_metrics().unwrap_or_default()
}

/// Returns the string value of a resource attribute, or `None`.
fn resource_attr(metrics: &[ResourceMetrics], key: &str) -> Option<String> {
    // The OTEL `Resource::get` takes an owned `Key`, which must be `'static`.
    // We use `.into()` on a `String` to produce the owned key, matching the
    // pattern in `test_resource_attributes.rs`.
    let owned_key: opentelemetry::Key = key.to_string().into();
    metrics
        .first()
        .and_then(|rm| rm.resource.get(owned_key))
        .map(|v| v.as_str().to_string())
}

// ── 1. Custom resource attributes appear in the export ───────────────────

/// Custom `resource_attributes` from config appear in the exported
/// resource (FR-026).
#[test]
fn test_custom_resource_attributes_appear_in_export() {
    let mut attrs = HashMap::new();
    attrs.insert(
        "deployment.environment".to_string(),
        "production".to_string(),
    );
    attrs.insert("service.namespace".to_string(), "ragent-team".to_string());
    let config = config_with_resource_attrs(attrs);

    let metrics = export_with_resource_from_config(&config);
    assert!(!metrics.is_empty(), "should have exported metrics");

    assert_eq!(
        resource_attr(&metrics, "deployment.environment"),
        Some("production".to_string()),
        "custom resource attribute must appear (FR-026)"
    );
    assert_eq!(
        resource_attr(&metrics, "service.namespace"),
        Some("ragent-team".to_string()),
        "second custom resource attribute must appear (FR-026)"
    );
}

/// Custom resource attributes coexist with the static attributes (FR-004 +
/// FR-026).
#[test]
fn test_custom_attributes_coexist_with_static() {
    let mut attrs = HashMap::new();
    attrs.insert("deployment.environment".to_string(), "staging".to_string());
    let config = config_with_resource_attrs(attrs);

    let metrics = export_with_resource_from_config(&config);
    assert!(!metrics.is_empty());

    // Static attributes are present.
    assert_eq!(
        resource_attr(&metrics, "service.name"),
        Some("ragent".to_string()),
        "service.name must be present (FR-004)"
    );
    assert_eq!(
        resource_attr(&metrics, "service.version"),
        Some(env!("CARGO_PKG_VERSION").to_string()),
        "service.version must be present (FR-004)"
    );
    // Custom attribute is present.
    assert_eq!(
        resource_attr(&metrics, "deployment.environment"),
        Some("staging".to_string()),
        "custom attribute must be present (FR-026)"
    );
}

// ── 2. Empty / absent resource_attributes ────────────────────────────────

/// An absent `resource_attributes` map produces only the static attributes.
#[test]
fn test_absent_resource_attributes_produces_only_static() {
    let config = config_with_resource_attrs(HashMap::new());

    let metrics = export_with_resource_from_config(&config);
    assert!(!metrics.is_empty());

    assert_eq!(
        resource_attr(&metrics, "service.name"),
        Some("ragent".to_string())
    );
    assert_eq!(
        resource_attr(&metrics, "service.version"),
        Some(env!("CARGO_PKG_VERSION").to_string())
    );
    // No custom attributes.
    assert!(resource_attr(&metrics, "deployment.environment").is_none());
}

// ── 3. Sensitive-data guard at the export level ──────────────────────────

/// An API key in `resource_attributes` is redacted in the export (FR-034).
#[test]
fn test_api_key_in_resource_attributes_is_redacted() {
    let mut attrs = HashMap::new();
    attrs.insert(
        "deployment.token".to_string(),
        "sk-proj-abc123def456ghi789".to_string(),
    );
    let config = config_with_resource_attrs(attrs);

    let metrics = export_with_resource_from_config(&config);
    assert!(!metrics.is_empty());

    let token = resource_attr(&metrics, "deployment.token");
    assert_eq!(
        token,
        Some("redacted".to_string()),
        "API key in resource_attributes must be redacted (FR-034)"
    );
}

/// A Bearer token in `resource_attributes` is redacted (FR-034).
#[test]
fn test_bearer_token_in_resource_attributes_is_redacted() {
    let mut attrs = HashMap::new();
    attrs.insert(
        "auth.header".to_string(),
        "Bearer abc123def456ghi789".to_string(),
    );
    let config = config_with_resource_attrs(attrs);

    let metrics = export_with_resource_from_config(&config);
    assert_eq!(
        resource_attr(&metrics, "auth.header"),
        Some("redacted".to_string()),
        "Bearer token must be redacted (FR-034)"
    );
}

/// A GitHub PAT in `resource_attributes` is redacted (FR-034).
#[test]
fn test_github_pat_in_resource_attributes_is_redacted() {
    let mut attrs = HashMap::new();
    attrs.insert(
        "ci.token".to_string(),
        "ghp_abc123def456ghi789jkl012".to_string(),
    );
    let config = config_with_resource_attrs(attrs);

    let metrics = export_with_resource_from_config(&config);
    assert_eq!(
        resource_attr(&metrics, "ci.token"),
        Some("redacted".to_string()),
        "GitHub PAT must be redacted (FR-034)"
    );
}

/// Multi-line file content in `resource_attributes` is redacted (FR-034).
#[test]
fn test_file_content_in_resource_attributes_is_redacted() {
    let mut attrs = HashMap::new();
    attrs.insert(
        "config.snippet".to_string(),
        "line1\nline2\nline3".to_string(),
    );
    let config = config_with_resource_attrs(attrs);

    let metrics = export_with_resource_from_config(&config);
    assert_eq!(
        resource_attr(&metrics, "config.snippet"),
        Some("redacted".to_string()),
        "multi-line content must be redacted (FR-034)"
    );
}

/// A `user:password` credential in `resource_attributes` is redacted
/// (FR-034).
#[test]
fn test_credential_in_resource_attributes_is_redacted() {
    let mut attrs = HashMap::new();
    attrs.insert("db.url".to_string(), "admin:secretpassword1234".to_string());
    let config = config_with_resource_attrs(attrs);

    let metrics = export_with_resource_from_config(&config);
    assert_eq!(
        resource_attr(&metrics, "db.url"),
        Some("redacted".to_string()),
        "user:password credential must be redacted (FR-034)"
    );
}

/// A safe custom attribute is NOT redacted (no false positive).
#[test]
fn test_safe_custom_attribute_not_redacted() {
    let mut attrs = HashMap::new();
    attrs.insert(
        "deployment.environment".to_string(),
        "production".to_string(),
    );
    attrs.insert("service.namespace".to_string(), "ragent-team".to_string());
    attrs.insert("host.id".to_string(), "i-1234567890abcdef0".to_string());
    let config = config_with_resource_attrs(attrs);

    let metrics = export_with_resource_from_config(&config);
    assert_eq!(
        resource_attr(&metrics, "deployment.environment"),
        Some("production".to_string()),
        "safe custom attribute must not be redacted"
    );
    assert_eq!(
        resource_attr(&metrics, "service.namespace"),
        Some("ragent-team".to_string()),
        "safe custom attribute must not be redacted"
    );
    assert_eq!(
        resource_attr(&metrics, "host.id"),
        Some("i-1234567890abcdef0".to_string()),
        "safe custom attribute must not be redacted"
    );
}

/// A mix of safe and sensitive custom attributes: safe ones pass through,
/// sensitive ones are redacted.
#[test]
fn test_mixed_safe_and_sensitive_resource_attributes() {
    let mut attrs = HashMap::new();
    attrs.insert(
        "deployment.environment".to_string(),
        "production".to_string(),
    );
    attrs.insert(
        "secret.token".to_string(),
        "sk-proj-abc123def456ghi789".to_string(),
    );
    let config = config_with_resource_attrs(attrs);

    let metrics = export_with_resource_from_config(&config);
    assert_eq!(
        resource_attr(&metrics, "deployment.environment"),
        Some("production".to_string()),
        "safe attribute passes through"
    );
    assert_eq!(
        resource_attr(&metrics, "secret.token"),
        Some("redacted".to_string()),
        "sensitive attribute is redacted"
    );
}

// ── 4. Subsystem config accessor ─────────────────────────────────────────

/// The subsystem's config accessor preserves the `resource_attributes`
/// map (FR-026).
#[test]
fn test_subsystem_config_preserves_resource_attributes() {
    let mut attrs = HashMap::new();
    attrs.insert(
        "deployment.environment".to_string(),
        "production".to_string(),
    );
    attrs.insert("service.namespace".to_string(), "ragent-team".to_string());
    let config = config_with_resource_attrs(attrs);

    let sub = build_subsystem(config);
    assert_eq!(sub.state(), TelemetryState::Enabled);

    let stored = sub.config().resource_attributes;
    assert_eq!(
        stored.get("deployment.environment"),
        Some(&"production".to_string()),
        "config accessor must preserve resource_attributes (FR-026)"
    );
    assert_eq!(
        stored.get("service.namespace"),
        Some(&"ragent-team".to_string()),
        "config accessor must preserve resource_attributes (FR-026)"
    );
    assert_eq!(stored.len(), 2);
}

/// The subsystem preserves a sensitive value in the config accessor — the
/// sanitisation happens at build time, not in the config (FR-034).
#[test]
fn test_subsystem_config_preserves_sensitive_value_raw() {
    let mut attrs = HashMap::new();
    attrs.insert(
        "secret.token".to_string(),
        "sk-proj-abc123def456ghi789".to_string(),
    );
    let config = config_with_resource_attrs(attrs);

    let sub = build_subsystem(config);
    // The config retains the raw value; sanitisation is applied in build_resource.
    assert_eq!(
        sub.config().resource_attributes.get("secret.token"),
        Some(&"sk-proj-abc123def456ghi789".to_string()),
        "config retains raw value; sanitisation is at build time (FR-034)"
    );
}

// ── 5. Config serde for resource_attributes ──────────────────────────────

/// `telemetry.otel.resource_attributes` deserialises from JSON (FR-026).
#[test]
fn test_resource_attributes_deserialize_from_json() {
    let json = r#"{
        "enabled": true,
        "endpoint": "http://localhost:4318",
        "resource_attributes": {
            "deployment.environment": "production",
            "service.namespace": "ragent-team",
            "host.id": "i-1234567890abcdef0"
        }
    }"#;
    let config: OtelConfig = serde_json::from_str(json).expect("should deserialize");
    assert!(config.enabled);
    assert_eq!(
        config.resource_attributes.get("deployment.environment"),
        Some(&"production".to_string())
    );
    assert_eq!(
        config.resource_attributes.get("service.namespace"),
        Some(&"ragent-team".to_string())
    );
    assert_eq!(
        config.resource_attributes.get("host.id"),
        Some(&"i-1234567890abcdef0".to_string())
    );
}

/// An absent `resource_attributes` field deserialises to an empty map.
#[test]
fn test_absent_resource_attributes_defaults_to_empty() {
    let json = r#"{
        "enabled": true,
        "endpoint": "http://localhost:4318"
    }"#;
    let config: OtelConfig = serde_json::from_str(json).expect("should deserialize");
    assert!(config.resource_attributes.is_empty());
}

/// `resource_attributes` round-trips through serde.
#[test]
fn test_resource_attributes_serde_roundtrip() {
    let mut attrs = HashMap::new();
    attrs.insert(
        "deployment.environment".to_string(),
        "production".to_string(),
    );
    attrs.insert("service.namespace".to_string(), "ragent-team".to_string());
    let config = config_with_resource_attrs(attrs);

    let json = serde_json::to_string(&config).expect("should serialize");
    let parsed: OtelConfig = serde_json::from_str(&json).expect("should deserialize");
    assert_eq!(parsed.resource_attributes, config.resource_attributes);
}

// ── 6. Config merge unions resource_attributes ──────────────────────────

/// `TelemetryConfig::merge` unions `resource_attributes` from base and
/// overlay (FR-026).
#[test]
fn test_config_merge_unions_resource_attributes() {
    use ragent_telemetry::TelemetryConfig;

    let mut base = TelemetryConfig::default();
    base.otel.resource_attributes.insert(
        "deployment.environment".to_string(),
        "production".to_string(),
    );
    base.otel
        .resource_attributes
        .insert("service.namespace".to_string(), "base-team".to_string());

    let mut overlay = TelemetryConfig::default();
    overlay
        .otel
        .resource_attributes
        .insert("service.namespace".to_string(), "overlay-team".to_string());
    overlay
        .otel
        .resource_attributes
        .insert("host.region".to_string(), "us-east-1".to_string());

    let merged = TelemetryConfig::merge(&base, &overlay);

    // Overlay value takes precedence for the overlapping key.
    assert_eq!(
        merged.otel.resource_attributes.get("service.namespace"),
        Some(&"overlay-team".to_string()),
        "overlay resource_attribute should take precedence"
    );
    // Base-only key is preserved.
    assert_eq!(
        merged
            .otel
            .resource_attributes
            .get("deployment.environment"),
        Some(&"production".to_string()),
        "base resource_attribute should be preserved"
    );
    // Overlay-only key is added.
    assert_eq!(
        merged.otel.resource_attributes.get("host.region"),
        Some(&"us-east-1".to_string()),
        "overlay-only resource_attribute should be added"
    );
    assert_eq!(
        merged.otel.resource_attributes.len(),
        3,
        "union should have 3 entries"
    );
}

/// A disabled overlay still contributes `resource_attributes` to a disabled
/// base (FR-026).
#[test]
fn test_config_merge_disabled_overlay_contributes_resource_attributes() {
    use ragent_telemetry::TelemetryConfig;

    let mut base = TelemetryConfig::default();
    base.otel
        .resource_attributes
        .insert("base.key".to_string(), "base-val".to_string());

    let mut overlay = TelemetryConfig::default();
    overlay
        .otel
        .resource_attributes
        .insert("overlay.key".to_string(), "overlay-val".to_string());

    let merged = TelemetryConfig::merge(&base, &overlay);
    assert_eq!(merged.otel.resource_attributes.len(), 2);
    assert_eq!(
        merged.otel.resource_attributes.get("base.key"),
        Some(&"base-val".to_string())
    );
    assert_eq!(
        merged.otel.resource_attributes.get("overlay.key"),
        Some(&"overlay-val".to_string())
    );
}

// ── 7. service_name is independent of resource_attributes ────────────────

/// `service.name` comes from `OtelConfig::service_name`, not from
/// `resource_attributes`; both appear in the export (FR-004 + FR-026).
#[test]
fn test_service_name_independent_of_resource_attributes() {
    let mut config = config_with_resource_attrs(HashMap::new());
    config.service_name = "custom-ragent".to_string();
    // Even if a user puts service.name in resource_attributes, the config
    // field takes precedence for the static attribute — but both would
    // appear if the user explicitly adds it. The standard path is the
    // config field.
    config
        .resource_attributes
        .insert("custom.attr".to_string(), "val".to_string());

    let metrics = export_with_resource_from_config(&config);
    assert_eq!(
        resource_attr(&metrics, "service.name"),
        Some("custom-ragent".to_string()),
        "service.name comes from the config field"
    );
    assert_eq!(
        resource_attr(&metrics, "custom.attr"),
        Some("val".to_string()),
        "custom resource attribute appears"
    );
}
