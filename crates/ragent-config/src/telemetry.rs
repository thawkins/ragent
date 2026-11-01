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

/// Minimum-severity rank selected by `telemetry.otel.logs.level: "all"`.
///
/// One below `TRACE`'s rank of `0` (see
/// [`ragent_telemetry::logs::severity_rank`]), so a record at any `tracing`
/// level clears the `min_rank` comparison and every event the layer sees is
/// exported. It is `0` in practice because the layer can only observe the five
/// `tracing` levels; the constant exists so the intent reads explicitly rather
/// than as a magic literal.
pub const ALL_SEVERITY_RANK: u8 = 0;

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
///   "metrics": {},
///   "logs": {
///     "enabled": false,
///     "level": "info"
///   }
/// }
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
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
    /// This is independent of the OTLP export path - both can run
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

    /// OTLP logs-signal export (spec `otel`, FR-036).
    ///
    /// Disabled by default. When enabled, ragent's `tracing` events are also
    /// shipped to the same collector as OTel log records, in addition to
    /// whatever local sink (TUI log panel or stderr) the run mode uses.
    #[serde(default)]
    pub logs: OtelLogsConfig,
}

/// Configuration for the `telemetry.otel.logs` block (spec `otel`, FR-036).
///
/// A log signal is only exported while [`OtelConfig::enabled`] is `true` and
/// this block is also enabled: the `logs` block selects the signal, it does
/// not switch telemetry on by itself (FR-040).
///
/// ```jsonc
/// {
///   "telemetry": {
///     "otel": {
///       "enabled": true,
///       "logs": { "enabled": true, "level": "info" }
///     }
///   }
/// }
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OtelLogsConfig {
    /// Whether the logs signal is exported (default `false`).
    #[serde(default)]
    pub enabled: bool,

    /// Minimum severity exported to the collector (default `"info"`).
    ///
    /// One of `all`, `trace`, `debug`, `info`, `warn`, `error`. `all` exports
    /// every record the layer sees, including events below `TRACE`; events
    /// below any other level still reach the local sink, so the remote export
    /// is filtered and a `--log-level debug` run does not turn into an
    /// unbounded log stream unless `all` is chosen deliberately.
    #[serde(default = "default_log_level")]
    pub level: String,
}

impl OtelLogsConfig {
    /// Returns `true` when the logs signal is enabled.
    #[must_use]
    pub const fn is_enabled(&self) -> bool {
        self.enabled
    }

    /// Parse [`Self::level`] into a min-severity rank.
    ///
    /// The returned rank is `0` for `trace`, `1` for `debug`, `2` for `info`,
    /// `3` for `warn`, and `4` for `error`, matching the ordering a
    /// `tracing::Level` maps to (see
    /// `ragent_telemetry::logs::severity_rank`). `all` returns
    /// [`ALL_SEVERITY_RANK`], one below `trace`, so nothing the layer sees is
    /// filtered out.
    ///
    /// An unrecognised value is reported as a validation problem by
    /// [`OtelConfig::validate`]; this helper falls back to `info` (`2`) so the
    /// export filter never fails to build.
    #[must_use]
    pub fn min_severity_rank(&self) -> u8 {
        match self.level.trim().to_ascii_lowercase().as_str() {
            "all" => ALL_SEVERITY_RANK,
            "trace" => 0,
            "debug" => 1,
            "warn" | "warning" => 3,
            "error" => 4,
            _ => 2,
        }
    }
}

impl Default for OtelLogsConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            level: default_log_level(),
        }
    }
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
        if !ragent_types::strutil::is_http_url(&self.endpoint) {
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
        if !matches!(
            self.logs.level.trim().to_ascii_lowercase().as_str(),
            "all" | "trace" | "debug" | "info" | "warn" | "warning" | "error"
        ) {
            problems.push(format!(
                "telemetry.otel.logs.level must be one of all, trace, debug, info, warn, error (got \"{}\")",
                self.logs.level
            ));
        }
        problems
    }

    /// Returns `true` when telemetry export is enabled.
    #[must_use]
    pub const fn is_enabled(&self) -> bool {
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
            logs: OtelLogsConfig::default(),
        }
    }
}
/// Top-level telemetry configuration wrapping the `telemetry.otel` block.
///
/// Embedded into [`crate::config::Config`] as the `telemetry` field.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct TelemetryConfig {
    /// The `telemetry.otel` block.
    #[serde(default)]
    pub otel: OtelConfig,
}

impl TelemetryConfig {
    /// Returns `true` when OTEL export is enabled.
    #[must_use]
    pub const fn is_enabled(&self) -> bool {
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
    /// * `legacy_open_telemetry` - the value of
    ///   `ExperimentalFlags.open_telemetry`.
    ///
    /// # Returns
    ///
    /// `true` when the legacy flag caused telemetry to be enabled (caller
    /// should log a deprecation warning); `false` otherwise.
    pub const fn apply_legacy_flag(&mut self, legacy_open_telemetry: bool) -> bool {
        if self.otel.enabled {
            // Already explicitly enabled - legacy flag is irrelevant.
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
    #[must_use]
    pub fn merge(base: &Self, overlay: &Self) -> Self {
        if overlay.otel.enabled {
            return overlay.clone();
        }

        // Overlay did not enable telemetry - keep base's otel config but union
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

// -- Serde default functions ---------------------------------------------

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

fn default_log_level() -> String {
    "info".to_string()
}

const fn default_cardinality_limit() -> usize {
    1000
}

#[cfg(test)]
#[path = "../tests/inline/telemetry_tests.rs"]
mod tests;
