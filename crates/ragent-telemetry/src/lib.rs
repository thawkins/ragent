//! OpenTelemetry metrics export subsystem for ragent.
//!
//! This crate owns the OpenTelemetry [`SdkMeterProvider`], instrument registry,
//! and OTLP exporter lifecycle (FR-001). It reads configuration from the
//! `telemetry.otel` block in `ragent.json` (via [`ragent_config::TelemetryConfig`])
//! and exposes a single [`TelemetrySubsystem`] handle that the session
//! processor, LLM provider layer, tool execution path, coordinator, and
//! permission system use to record metrics.
//!
//! # Feature gating (NFR-006)
//!
//! The real OpenTelemetry implementation is gated behind the `telemetry` Cargo
//! feature. When the feature is **disabled** (the default), the crate compiles
//! with zero-overhead no-op stubs and pulls no `opentelemetry` dependencies.
//! When the feature is **enabled**, [`TelemetrySubsystem::enabled`]
//! constructs a real [`SdkMeterProvider`] with OTLP exporters.
//!
//! # Dependency policy (NFR-001)
//!
//! The crate depends only on `ragent-types`, `ragent-config`, `opentelemetry`,
//! `opentelemetry-otlp`, and `tokio` — no additional heavyweight dependencies.
//!
//! [`SdkMeterProvider`]: https://docs.rs/opentelemetry/0.27/opentelemetry/sdk/metrics/struct.SdkMeterProvider.html

pub mod cardinality;
pub mod counters;
pub mod instruments;
pub mod prometheus;
pub mod recorder;
pub mod sensitive;
pub mod shutdown;
pub mod subsystem;

// Re-export the config types from ragent-config so consumers can use a
// single import path. The canonical definitions live in `ragent_config::telemetry`.
pub use ragent_config::{OtelConfig, OtelProtocol, TelemetryConfig};
pub use subsystem::{TelemetryState, TelemetrySubsystem};

#[cfg(feature = "telemetry")]
pub use instruments::InstrumentRegistry;
#[cfg(not(feature = "telemetry"))]
pub use instruments::NoopInstrumentRegistry as InstrumentRegistry;

/// Crate-level result alias.
pub type Result<T> = std::result::Result<T, TelemetryError>;

/// Errors returned by the telemetry subsystem.
///
/// Kept intentionally narrow for the scaffold; richer variants will be added
/// as exporter wiring (T-004, T-005) and configuration parsing (T-002) land.
#[derive(Debug, thiserror::Error)]
pub enum TelemetryError {
    /// The `telemetry` Cargo feature is not enabled, so the real OpenTelemetry
    /// provider cannot be constructed.
    #[error("the 'telemetry' Cargo feature is required for live OTEL export")]
    FeatureNotEnabled,
    /// The configured endpoint URL is missing or invalid.
    #[error("invalid OTEL endpoint: {0}")]
    InvalidEndpoint(String),
    /// The OTLP exporter failed to initialise.
    #[error("OTLP exporter initialisation failed: {0}")]
    ExporterInit(String),
}
