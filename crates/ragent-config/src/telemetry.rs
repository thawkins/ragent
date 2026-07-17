//! Telemetry configuration for the OpenTelemetry metrics export subsystem.
//!
//! This module defines the `telemetry.otel` configuration schema that lives
//! inside [`crate::config::Config`]. The types here are consumed by the
//! `ragent-telemetry` crate, which owns the meter provider and OTLP exporter
//! lifecycle.
//!
//! All fields use `#[serde(default)]` so a partial or empty `telemetry.otel`
//! block deserialises to disabled-by-default (FR-002 of the otel spec).

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

/// Transport protocol for OTLP metric export.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum OtelProtocol {
    /// OTLP over HTTP (default).
    #[default]
    Http,
    /// OTLP over gRPC.
    Grpc,
}

/// Configuration for the `telemetry.otel` block.
///
/// Serialises/deserialises as:
///
/// ```jsonc
/// {
///   "enabled": false,
///   "endpoint": "http://localhost:4318",
///   "protocol": "http",
///   "export_interval_seconds": 30,
///   "service_name": "ragent",
///   "resource_attributes": {},
///   "metrics": {}
/// }
/// ```
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct OtelConfig {
    /// Master on/off switch (default `false`).
    #[serde(default)]
    pub enabled: bool,

    /// OTLP endpoint base URL (default `"http://localhost:4318"`).
    #[serde(default = "default_endpoint")]
    pub endpoint: String,

    /// Export transport protocol (default `Http`).
    #[serde(default)]
    pub protocol: OtelProtocol,

    /// Batch export interval in seconds (default `30`).
    #[serde(default = "default_export_interval")]
    pub export_interval_seconds: u64,

    /// Per-export HTTP/gRPC request timeout in seconds (default `10`).
    ///
    /// This caps how long a single flush/export can block before the exporter
    /// gives up and surfaces an error (FR-031: the agent loop must never block
    /// on a slow/unreachable endpoint). The value is clamped to at least `1`
    /// second at build time so a zero value cannot produce a zero-duration
    /// timeout (which would make every export fail).
    #[serde(default = "default_export_timeout")]
    pub export_timeout_seconds: u64,

    /// `service.name` resource attribute (default `"ragent"`).
    #[serde(default = "default_service_name")]
    pub service_name: String,

    /// Custom resource attributes appended to every exported metric (FR-026).
    #[serde(default)]
    pub resource_attributes: HashMap<String, String>,

    /// Per-metric enable/disable toggles keyed by metric name (FR-027).
    ///
    /// A metric absent from this map is enabled by default. Setting a metric
    /// to `false` disables it to reduce cardinality or volume.
    #[serde(default)]
    pub metrics: HashMap<String, bool>,

    /// Optional in-process Prometheus text endpoint port (FR-028).
    ///
    /// When `Some(port)`, the telemetry subsystem serves a `/metrics`
    /// endpoint in Prometheus text format on `127.0.0.1:<port>` for local
    /// scraping without an OTLP collector. When `None` (the default), no
    /// Prometheus endpoint is started.
    ///
    /// This is independent of the OTLP export path — both can run
    /// simultaneously.
    #[serde(default)]
    pub internal_port: Option<u16>,

    /// Maximum number of distinct attribute combinations per metric before
    /// overflow into an `unknown` bucket (FR-035).
    ///
    /// When the number of unique attribute-value combinations for a single
    /// metric exceeds this limit, excess combinations are collapsed into a
    /// single `unknown` bucket so that metric cardinality stays bounded.
    /// Defaults to `1000`.
    #[serde(default = "default_cardinality_limit")]
    pub cardinality_limit: usize,
}

impl OtelConfig {
    /// Validate the configuration and return a list of any problems.
    ///
    /// Returns an empty vector when the configuration is valid or disabled.
    /// Validation only runs when `enabled` is `true`, since a disabled config
    /// does not use the endpoint or interval.
    #[must_use]
    pub fn validate(&self) -> Vec<String> {
        let mut problems = Vec::new();
        if !self.enabled {
            return problems;
        }
        if self.endpoint.is_empty() {
            problems.push(
                "telemetry.otel.endpoint must not be empty when telemetry is enabled".to_string(),
            );
        }
        if !self.endpoint.starts_with("http://") && !self.endpoint.starts_with("https://") {
            problems.push(format!(
                "telemetry.otel.endpoint must be a valid HTTP or HTTPS URL (got \"{}\")",
                self.endpoint
            ));
        }
        if self.export_interval_seconds == 0 {
            problems.push(
                "telemetry.otel.export_interval_seconds must be > 0 when telemetry is enabled"
                    .to_string(),
            );
        }
        if self.export_timeout_seconds == 0 {
            problems.push(
                "telemetry.otel.export_timeout_seconds must be > 0 when telemetry is enabled"
                    .to_string(),
            );
        }
        if self.service_name.is_empty() {
            problems.push("telemetry.otel.service_name must not be empty".to_string());
        }
        problems
    }

    /// Returns `true` when telemetry export is enabled.
    #[must_use]
    pub fn is_enabled(&self) -> bool {
        self.enabled
    }
}

impl Default for OtelConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            endpoint: default_endpoint(),
            protocol: OtelProtocol::Http,
            export_interval_seconds: default_export_interval(),
            export_timeout_seconds: default_export_timeout(),
            service_name: default_service_name(),
            resource_attributes: HashMap::new(),
            metrics: HashMap::new(),
            internal_port: None,
            cardinality_limit: default_cardinality_limit(),
        }
    }
}
/// Top-level telemetry configuration wrapping the `telemetry.otel` block.
///
/// Embedded into [`crate::config::Config`] as the `telemetry` field.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct TelemetryConfig {
    /// The `telemetry.otel` block.
    #[serde(default)]
    pub otel: OtelConfig,
}

impl TelemetryConfig {
    /// Returns `true` when OTEL export is enabled.
    #[must_use]
    pub fn is_enabled(&self) -> bool {
        self.otel.enabled
    }

    /// Apply the legacy `experimental.open_telemetry` flag as a fallback
    /// (T-019 / FR-002).
    ///
    /// If `telemetry.otel` is already explicitly enabled, this is a no-op.
    /// If `telemetry.otel` is disabled **but** the legacy
    /// `experimental.open_telemetry` flag is `true`, enable telemetry with
    /// default settings and return `true` so the caller can emit a
    /// deprecation warning.
    ///
    /// # Arguments
    ///
    /// * `legacy_open_telemetry` — the value of
    ///   `ExperimentalFlags.open_telemetry`.
    ///
    /// # Returns
    ///
    /// `true` when the legacy flag caused telemetry to be enabled (caller
    /// should log a deprecation warning); `false` otherwise.
    pub fn apply_legacy_flag(&mut self, legacy_open_telemetry: bool) -> bool {
        if self.otel.enabled {
            // Already explicitly enabled — legacy flag is irrelevant.
            return false;
        }
        if legacy_open_telemetry {
            self.otel.enabled = true;
            // Use default settings for endpoint, protocol, interval, etc.
            true
        } else {
            false
        }
    }

    /// Merge an overlay telemetry config into a base, with overlay taking
    /// precedence for explicitly-set fields.
    ///
    /// Because all `OtelConfig` fields have serde defaults, we cannot
    /// distinguish "explicitly set to default" from "absent". The merge
    /// strategy is:
    ///
    /// - If the overlay's `enabled` is `true`, take the entire overlay `otel`
    ///   block (the user explicitly turned telemetry on and configured it).
    /// - Otherwise, preserve the base's `enabled` but still merge
    ///   `resource_attributes` and `metrics` maps (union) so global config can
    ///   supply defaults that a project config extends.
    pub fn merge(base: &Self, overlay: &Self) -> Self {
        if overlay.otel.enabled {
            return overlay.clone();
        }

        // Overlay did not enable telemetry — keep base's otel config but union
        // the maps so a global config can pre-populate attributes/metric
        // toggles that a project config extends.
        let mut merged = base.clone();
        for (k, v) in &overlay.otel.resource_attributes {
            merged.otel.resource_attributes.insert(k.clone(), v.clone());
        }
        for (k, v) in &overlay.otel.metrics {
            merged.otel.metrics.insert(k.clone(), *v);
        }
        merged
    }
}

// ── Serde default functions ─────────────────────────────────────────────

const fn default_export_interval() -> u64 {
    30
}

const fn default_export_timeout() -> u64 {
    10
}

fn default_endpoint() -> String {
    "http://localhost:4318".to_string()
}

fn default_service_name() -> String {
    "ragent".to_string()
}

const fn default_cardinality_limit() -> usize {
    1000
}

#[cfg(test)]
mod tests {
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

    // ── OtelConfig::validate tests ───────────────────────────────────────

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

    // ── TelemetryConfig::apply_legacy_flag tests ────────────────────────

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
}
