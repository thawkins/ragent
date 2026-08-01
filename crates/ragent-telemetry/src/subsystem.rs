//! The telemetry subsystem — owner of the meter provider and exporter lifecycle.
//!
//! [`TelemetrySubsystem`] is the single handle that the rest of ragent
//! interacts with. It encapsulates whether telemetry is enabled or disabled
//! (FR-021, FR-022) and, when enabled, holds the live
//! [`SdkMeterProvider`](https://docs.rs/opentelemetry_sdk/0.27/opentelemetry_sdk/metrics/struct.SdkMeterProvider.html)
//! and OTLP exporter.
//!
//! # No-op fallback (NFR-002)
//!
//! When telemetry is disabled — either via configuration or because the
//! `telemetry` Cargo feature is off — the subsystem uses the OpenTelemetry
//! `NoopMeterProvider`. All instrument calls (`.add()`, `.record()`) are
//! cheap no-ops with zero network traffic (FR-022, FR-032).

use crate::Result;
#[cfg(not(feature = "telemetry"))]
use crate::TelemetryError;
use ragent_config::OtelConfig;

/// Whether the subsystem is actively exporting or running as a no-op.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TelemetryState {
    /// Telemetry is disabled — a no-op meter provider is in use (FR-022, NFR-002).
    Disabled,
    /// Telemetry is enabled and the live meter provider is active (FR-021).
    Enabled,
}

/// Owner of the OpenTelemetry meter provider and OTLP exporter lifecycle.
///
/// Created from an [`OtelConfig`]. When `config.enabled` is `false` (or when
/// the `telemetry` Cargo feature is off) the subsystem runs in
/// [`TelemetryState::Disabled`] mode, where all metric recording calls are
/// zero-overhead no-ops (NFR-002).
///
/// When enabled and the `telemetry` feature is active, the subsystem
/// constructs a real `SdkMeterProvider` with a periodic reader backed by an
/// OTLP/HTTP exporter (T-003, T-004).
///
/// # Runtime reconfiguration
///
/// The live provider state is held behind a `parking_lot::Mutex` so the
/// subsystem can be reconfigured at runtime through a shared
/// `Arc<TelemetrySubsystem>` via [`reconfigure`](Self::reconfigure). This is
/// what powers the `/telemetry on|off` slash commands: toggling off shuts
/// down the live provider (stopping the periodic OTLP reader and therefore
/// the "Failed to export metrics" log noise) without restarting the process.
pub struct TelemetrySubsystem {
    /// Interior-mutable runtime state so [`reconfigure`](Self::reconfigure)
    /// can swap providers through `&self` while the subsystem is shared via
    /// `Arc<TelemetrySubsystem>`.
    runtime: parking_lot::Mutex<RuntimeState>,
}

/// Mutable runtime state owned by [`TelemetrySubsystem`], held under a mutex.
struct RuntimeState {
    /// Whether the subsystem is actively exporting or running as a no-op.
    state: TelemetryState,
    /// The resolved [`OtelConfig`] that built (or last reconfigured) this
    /// subsystem. Kept in sync with the live provider so [`instruments`]
    /// can apply the cardinality limit and per-metric toggles.
    config: OtelConfig,
    /// The live meter provider, held to keep the SDK alive for the lifetime
    /// of the subsystem. `None` when disabled or when the `telemetry`
    /// feature is not compiled in. Wrapped in `Arc` so [`provider`] can
    /// return a cheap clone and so [`InstrumentRegistry`] handles can keep
    /// the (possibly shut-down) provider alive independently.
    #[cfg(feature = "telemetry")]
    provider: Option<std::sync::Arc<opentelemetry_sdk::metrics::SdkMeterProvider>>,
    /// The [`SharedManualReader`] used by the optional Prometheus endpoint
    /// (FR-028). `None` when `telemetry.otel.internal_port` is `None` or
    /// telemetry is disabled. Held so the subsystem can extract the
    /// reader handle for the HTTP server task.
    #[cfg(feature = "telemetry")]
    #[allow(dead_code)]
    prometheus_reader: Option<crate::prometheus::SharedManualReader>,
    /// The join handle for the Prometheus HTTP server task (FR-028).
    /// `None` when no `internal_port` is configured. The handle is for the
    /// outer task that resolves to the inner server handle (or a bind
    /// error); we store it so it can be aborted on shutdown/reconfigure.
    #[cfg(feature = "telemetry")]
    prometheus_handle:
        Option<tokio::task::JoinHandle<std::io::Result<tokio::task::JoinHandle<()>>>>,
}

impl RuntimeState {
    /// Construct a disabled runtime state with the given config.
    #[cfg(feature = "telemetry")]
    const fn disabled(config: OtelConfig) -> Self {
        Self {
            state: TelemetryState::Disabled,
            config,
            provider: None,
            prometheus_reader: None,
            prometheus_handle: None,
        }
    }

    /// Construct a disabled runtime state with the given config
    /// (feature-off variant — no provider fields exist).
    #[cfg(not(feature = "telemetry"))]
    fn disabled(config: OtelConfig) -> Self {
        Self {
            state: TelemetryState::Disabled,
            config,
        }
    }
}

impl TelemetrySubsystem {
    /// Return a disabled copy of this subsystem with the same config.
    /// Used when the subsystem is shared via `Arc` but a by-value copy is
    /// needed for a shutdown guard.
    #[must_use]
    pub fn clone_disabled(&self) -> Self {
        let config = self.runtime.lock().config.clone();
        Self::disabled_with_config(config)
    }

    /// Construct a subsystem from the given [`OtelConfig`].
    ///
    /// When `config.enabled` is `false`, returns a [`Disabled`](TelemetryState::Disabled)
    /// subsystem immediately. When `true` and the `telemetry` feature is
    /// active, initialises the real provider (T-003). When `true` but the
    /// feature is off, returns an error.
    ///
    /// # Errors
    ///
    /// Returns [`TelemetryError::FeatureNotEnabled`] if the config requests
    /// enabled telemetry but the `telemetry` Cargo feature is not compiled in.
    /// Returns [`TelemetryError::InvalidEndpoint`] if the endpoint URL is
    /// empty or malformed. Returns [`TelemetryError::ExporterInit`] if the
    /// OTLP exporter or meter provider fails to initialise.
    ///
    /// # Examples
    ///
    /// ```
    /// use ragent_telemetry::{OtelConfig, TelemetrySubsystem};
    ///
    /// let disabled = OtelConfig::default(); // enabled == false
    /// let sub = TelemetrySubsystem::new(disabled).expect("disabled subsystem");
    /// assert_eq!(sub.state(), ragent_telemetry::TelemetryState::Disabled);
    /// ```
    pub fn new(config: OtelConfig) -> Result<Self> {
        if config.enabled {
            #[cfg(feature = "telemetry")]
            {
                let provider = build_provider(&config)?;

                // Optional Prometheus text endpoint (FR-028).
                // When `internal_port` is `Some(port)`, build a SharedManualReader
                // registered alongside the OTLP PeriodicReader, and spawn
                // an HTTP server that renders the snapshot on `/metrics`.
                let prometheus_reader: Option<crate::prometheus::SharedManualReader> =
                    if config.internal_port.is_some() {
                        Some(crate::prometheus::SharedManualReader::new())
                    } else {
                        None
                    };

                // Rebuild the provider with the Prometheus reader attached
                // if we have one. `build_provider` already built one with
                // just the PeriodicReader; we rebuild with both readers.
                let provider = if let Some(reader) = &prometheus_reader {
                    build_provider_with_prometheus(&config, reader.clone())?
                } else {
                    provider
                };

                // Spawn the HTTP server if a port is configured. `serve` is
                // async, so spawn it as a background task on the caller's
                // tokio runtime and store the outer JoinHandle for later
                // abort on shutdown/reconfigure.
                let prometheus_handle = if let Some(port) = config.internal_port {
                    prometheus_reader
                        .as_ref()
                        .map(|reader| tokio::spawn(crate::prometheus::serve(reader.handle(), port)))
                } else {
                    None
                };

                Ok(Self {
                    runtime: parking_lot::Mutex::new(RuntimeState {
                        state: TelemetryState::Enabled,
                        config,
                        provider: Some(std::sync::Arc::new(provider)),
                        prometheus_reader,
                        prometheus_handle,
                    }),
                })
            }
            #[cfg(not(feature = "telemetry"))]
            {
                // The user explicitly enabled telemetry but the feature is not
                // compiled in. Rather than silently degrading, surface the error
                // so the build configuration mismatch is visible.
                tracing::warn!(
                    "telemetry.otel.enabled is true but the 'telemetry' Cargo \
                       feature is not enabled; falling back to no-op. Rebuild with \
                       --features ragent-telemetry/telemetry to enable export."
                );
                Err(TelemetryError::FeatureNotEnabled)
            }
        } else {
            Ok(Self::disabled_with_config(config))
        }
    }

    /// Construct an enabled subsystem from an existing [`SdkMeterProvider`] and
    /// config without building a new exporter.
    ///
    /// Primarily intended for tests that need to inspect exports with an
    /// `InMemoryMetricExporter`. The caller owns the provider; the subsystem
    /// will flush and shut it down on [`shutdown`](Self::shutdown) or when a
    /// [`ShutdownGuard`](crate::shutdown::ShutdownGuard) drops.
    #[cfg(feature = "telemetry")]
    #[must_use]
    pub fn from_provider(config: OtelConfig, provider: SdkMeterProvider) -> Self {
        Self {
            runtime: parking_lot::Mutex::new(RuntimeState {
                state: TelemetryState::Enabled,
                config,
                provider: Some(std::sync::Arc::new(provider)),
                prometheus_reader: None,
                prometheus_handle: None,
            }),
        }
    }
    /// Construct a no-op subsystem that discards all metrics.
    ///
    /// This is the default when no `telemetry.otel` block is present in
    /// `ragent.json` (FR-002).
    #[must_use]
    pub fn disabled() -> Self {
        Self::disabled_with_config(OtelConfig::default())
    }

    const fn disabled_with_config(config: OtelConfig) -> Self {
        Self {
            runtime: parking_lot::Mutex::new(RuntimeState::disabled(config)),
        }
    }

    /// Returns the current telemetry state.
    #[must_use]
    pub fn state(&self) -> TelemetryState {
        self.runtime.lock().state
    }

    /// Returns `true` when the subsystem is actively exporting metrics.
    #[must_use]
    pub fn is_enabled(&self) -> bool {
        self.runtime.lock().state == TelemetryState::Enabled
    }

    /// Returns a clone of the resolved [`OtelConfig`].
    ///
    /// Returns an owned copy (rather than a reference) because the config
    /// lives behind the interior-mutability mutex; cloning is cheap and
    /// avoids exposing a locked guard to callers.
    #[must_use]
    pub fn config(&self) -> OtelConfig {
        self.runtime.lock().config.clone()
    }

    /// Returns a cheap clone of the live meter provider, if telemetry is
    /// enabled and the `telemetry` feature is compiled in.
    ///
    /// Returns `None` when disabled or when the feature is off. The
    /// returned [`Arc`] keeps the provider alive independently of the
    /// subsystem, so callers can build [`InstrumentRegistry`] handles from
    /// it even if the subsystem is later reconfigured.
    #[must_use]
    #[cfg(feature = "telemetry")]
    pub fn provider(&self) -> Option<std::sync::Arc<SdkMeterProvider>> {
        self.runtime.lock().provider.clone()
    }

    /// Build and return an [`InstrumentRegistry`] backed by this subsystem's
    /// meter provider (FR-003).
    ///
    /// When telemetry is enabled and the `telemetry` feature is active,
    /// returns `Some(InstrumentRegistry)` with live instruments. When
    /// disabled, returns `None` — callers should use a no-op registry
    /// instead (FR-022, NFR-002).
    ///
    /// The registry is configured with the cardinality limit (FR-035) and
    /// per-metric enable/disable toggles (FR-027) from the `OtelConfig`
    /// that built this subsystem.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// # use ragent_telemetry::{OtelConfig, TelemetrySubsystem};
    /// let config = OtelConfig::default(); // disabled
    /// let sub = TelemetrySubsystem::new(config).expect("disabled subsystem");
    /// assert!(sub.instruments().is_none(), "disabled subsystem has no live instruments");
    /// ```
    #[must_use]
    #[cfg(feature = "telemetry")]
    pub fn instruments(&self) -> Option<crate::instruments::InstrumentRegistry> {
        let guard = self.runtime.lock();
        let provider = guard.provider.as_ref()?;
        Some(
            crate::instruments::InstrumentRegistry::from_provider(provider)
                .with_cardinality_limit(guard.config.cardinality_limit)
                .with_metric_toggles(guard.config.metrics.clone()),
        )
    }

    /// Reconfigure the subsystem at runtime from a new [`OtelConfig`].
    ///
    /// This is the runtime-toggle entry point used by the `/telemetry on|off`
    /// slash commands. It atomically (under the runtime mutex):
    ///
    /// 1. Shuts down the currently live provider (if any), which stops the
    ///    periodic OTLP reader and therefore the "Failed to export metrics"
    ///    log noise that would otherwise continue after `/telemetry off`.
    /// 2. Aborts any Prometheus HTTP server task so its TCP listener is
    ///    released.
    /// 3. If the new config is enabled (and the `telemetry` feature is
    ///    active), builds a fresh provider and Prometheus endpoint.
    /// 4. Updates the stored state and config so subsequent calls to
    ///    [`is_enabled`](Self::is_enabled), [`config`](Self::config), and
    ///    [`instruments`](Self::instruments) reflect the new state.
    ///
    /// When the `telemetry` Cargo feature is off, only the config/state are
    /// updated (there is no provider to shut down or build).
    ///
    /// # Errors
    ///
    /// Returns [`TelemetryError::InvalidEndpoint`] or
    /// [`TelemetryError::ExporterInit`] if building a new enabled provider
    /// fails. On such a failure the subsystem is left in the **disabled**
    /// state (the old provider has already been shut down) so the agent
    /// loop never continues with a half-initialised provider.
    pub fn reconfigure(&self, config: OtelConfig) -> Result<()> {
        let mut guard = self.runtime.lock();

        // 1. Shut down the existing live provider (stops the periodic reader).
        #[cfg(feature = "telemetry")]
        {
            if let Some(handle) = &guard.prometheus_handle {
                handle.abort();
            }
            guard.prometheus_handle = None;
            guard.prometheus_reader = None;
            if let Some(provider) = guard.provider.take() {
                // `SdkMeterProvider::shutdown` takes `&self`; call it through
                // the Arc. Errors are logged but non-fatal — the provider is
                // being discarded regardless.
                if let Err(e) = provider.shutdown() {
                    tracing::warn!(error = %e, "OTEL meter provider shutdown during reconfigure");
                }
            }
        }

        // 2. Build the new provider if enabled.
        if config.enabled {
            #[cfg(feature = "telemetry")]
            {
                match build_enabled_provider(&config) {
                    Ok((provider, prometheus_reader, prometheus_handle)) => {
                        guard.state = TelemetryState::Enabled;
                        guard.config = config;
                        guard.provider = Some(std::sync::Arc::new(provider));
                        guard.prometheus_reader = prometheus_reader;
                        guard.prometheus_handle = prometheus_handle;
                    }
                    Err(e) => {
                        // Building the new provider failed — leave the
                        // subsystem disabled so the agent loop never runs
                        // with a half-initialised provider.
                        tracing::warn!(error = %e, "reconfigure: provider build failed; leaving telemetry disabled");
                        guard.state = TelemetryState::Disabled;
                        guard.config = config;
                        guard.provider = None;
                        return Err(e);
                    }
                }
            }
            #[cfg(not(feature = "telemetry"))]
            {
                tracing::warn!(
                    "reconfigure: telemetry.otel.enabled is true but the 'telemetry' \
                       Cargo feature is not enabled; remaining in no-op mode."
                );
                guard.state = TelemetryState::Disabled;
                guard.config = config;
                return Err(TelemetryError::FeatureNotEnabled);
            }
        } else {
            guard.state = TelemetryState::Disabled;
            guard.config = config;
            #[cfg(feature = "telemetry")]
            {
                guard.provider = None;
            }
        }
        Ok(())
    }

    /// Force-flush all pending metric exports immediately (FR-006).
    ///
    /// This triggers an immediate export of all buffered metrics to the
    /// configured endpoint, bypassing the periodic export interval. It is
    /// used by:
    ///
    /// - The graceful-shutdown signal handler (T-009 / FR-019) before
    ///   calling [`shutdown`](Self::shutdown).
    /// - Callers that want to ensure metrics are delivered at a specific
    ///   point (e.g. before a test assertion or a benchmark checkpoint).
    ///
    /// When telemetry is disabled, this is a no-op.
    ///
    /// # Non-blocking guarantee (FR-031, FR-033)
    ///
    /// The exporter has a bounded request timeout (see
    /// [`OtelConfig::export_timeout_seconds`]). If the endpoint is
    /// unreachable or slow, the export fails with an error rather than
    /// blocking indefinitely. That error is logged at `warn` level and
    /// returned here — the caller **must not** propagate it in a way that
    /// would crash the agent loop. The [`ShutdownGuard`] and signal handler
    /// both swallow such errors for this reason.
    ///
    /// # Errors
    ///
    /// Returns [`TelemetryError::ExporterInit`] if the flush fails. The
    /// caller should log the error but **must not** propagate it in a way
    /// that would crash the agent loop (FR-031, FR-033).
    pub fn flush(&self) -> Result<()> {
        #[cfg(feature = "telemetry")]
        {
            let guard = self.runtime.lock();
            if let Some(provider) = &guard.provider
                && let Err(e) = provider.force_flush()
            {
                tracing::warn!("OTEL meter provider force_flush error: {e}");
                return Err(crate::TelemetryError::ExporterInit(format!(
                    "flush failed: {e}"
                )));
            }
        }
        Ok(())
    }

    /// Gracefully shut down the subsystem, flushing pending exports.
    ///
    /// When telemetry is enabled, this calls `SdkMeterProvider::shutdown()`,
    /// which flushes all pending metric exports to the configured endpoint
    /// before terminating (FR-019). When disabled, it is a no-op.
    ///
    /// After `shutdown()` the provider can no longer record metrics; callers
    /// should treat the subsystem as consumed. For a non-destructive flush
    /// (e.g. periodic checkpointing), use [`flush`](Self::flush) instead.
    ///
    /// # Prometheus endpoint (FR-028)
    ///
    /// If a Prometheus HTTP server is running on `internal_port`, its
    /// background task is aborted here so the TCP listener is released
    /// promptly. Aborting is safe because the server is stateless.
    ///
    /// # Errors
    ///
    /// Returns [`TelemetryError::ExporterInit`] if the flush fails.
    pub fn shutdown(&self) -> Result<()> {
        #[cfg(feature = "telemetry")]
        {
            let guard = self.runtime.lock();
            if let Some(handle) = &guard.prometheus_handle {
                handle.abort();
            }
            if let Some(provider) = &guard.provider
                && let Err(e) = provider.shutdown()
            {
                tracing::warn!("OTEL meter provider shutdown error: {e}");
                return Err(crate::TelemetryError::ExporterInit(format!(
                    "shutdown failed: {e}"
                )));
            }
        }
        Ok(())
    }
}

impl Default for TelemetrySubsystem {
    fn default() -> Self {
        Self::disabled()
    }
}

impl std::fmt::Debug for TelemetrySubsystem {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let guard = self.runtime.lock();
        f.debug_struct("TelemetrySubsystem")
            .field("state", &guard.state)
            .field("endpoint", &guard.config.endpoint)
            .field("protocol", &guard.config.protocol)
            .finish()
    }
}

// ── Provider construction (feature-gated) ──────────────────────────────

#[cfg(feature = "telemetry")]
use opentelemetry::KeyValue;
#[cfg(feature = "telemetry")]
use opentelemetry_sdk::Resource;
#[cfg(feature = "telemetry")]
use opentelemetry_sdk::metrics::SdkMeterProvider;

/// Build the live `SdkMeterProvider` from the config (FR-021).
///
/// Wires an OTLP/HTTP exporter into a periodic reader with the configured
/// export interval, attaches resource attributes, and builds the provider.
#[cfg(feature = "telemetry")]
fn build_provider(config: &OtelConfig) -> Result<SdkMeterProvider> {
    use std::time::Duration;

    // Validate endpoint.
    if config.endpoint.is_empty() {
        return Err(crate::TelemetryError::InvalidEndpoint(
            "endpoint is empty".to_string(),
        ));
    }
    if !config.endpoint.starts_with("http://") && !config.endpoint.starts_with("https://") {
        return Err(crate::TelemetryError::InvalidEndpoint(format!(
            "endpoint must be HTTP or HTTPS URL: {}",
            config.endpoint
        )));
    }

    // Build the OTLP metric exporter (HTTP/protobuf by default, gRPC if configured).
    let exporter = build_metric_exporter(config)?;

    // Build a periodic reader with the configured export interval (FR-006).
    let interval = Duration::from_secs(config.export_interval_seconds.max(1));
    let reader = opentelemetry_sdk::metrics::PeriodicReader::builder(exporter)
        .with_interval(interval)
        .build();

    // Build the resource with service.name, service.version, and custom attributes (FR-004).
    let resource = build_resource(config);

    let provider = SdkMeterProvider::builder()
        .with_resource(resource)
        .with_reader(reader)
        .build();

    Ok(provider)
}

/// Build a live `SdkMeterProvider` with both the OTLP `PeriodicReader` and
/// a Prometheus `ManualReader` attached (FR-028).
///
/// This mirrors [`build_provider`] but adds the given `prometheus_reader`
/// so the same `SdkMeterProvider` serves both export paths. Recording is
/// unaffected — the OTLP exporter batches on a timer, while the
/// Prometheus endpoint collects on-demand when a scraper hits `/metrics`.
#[cfg(feature = "telemetry")]
fn build_provider_with_prometheus(
    config: &OtelConfig,
    prometheus_reader: crate::prometheus::SharedManualReader,
) -> Result<SdkMeterProvider> {
    use std::time::Duration;

    // Validate endpoint.
    if config.endpoint.is_empty() {
        return Err(crate::TelemetryError::InvalidEndpoint(
            "endpoint is empty".to_string(),
        ));
    }
    if !config.endpoint.starts_with("http://") && !config.endpoint.starts_with("https://") {
        return Err(crate::TelemetryError::InvalidEndpoint(format!(
            "endpoint must be HTTP or HTTPS URL: {}",
            config.endpoint
        )));
    }

    let exporter = build_metric_exporter(config)?;
    let interval = Duration::from_secs(config.export_interval_seconds.max(1));
    let periodic = opentelemetry_sdk::metrics::PeriodicReader::builder(exporter)
        .with_interval(interval)
        .build();

    let resource = build_resource(config);

    let provider = SdkMeterProvider::builder()
        .with_resource(resource)
        .with_reader(periodic)
        .with_reader(prometheus_reader)
        .build();

    Ok(provider)
}

/// Build the live provider plus optional Prometheus reader/handle from a
/// config that is known to be enabled.
///
/// This is the shared construction path used by both [`TelemetrySubsystem::new`]
/// (indirectly, inlined) and [`TelemetrySubsystem::reconfigure`]. It returns
/// the three pieces a caller needs to populate [`RuntimeState`].
///
/// Must be called from within a tokio runtime context because the Prometheus
/// `serve` future is spawned via [`tokio::spawn`].
#[cfg(feature = "telemetry")]
fn build_enabled_provider(
    config: &OtelConfig,
) -> Result<(
    SdkMeterProvider,
    Option<crate::prometheus::SharedManualReader>,
    Option<tokio::task::JoinHandle<std::io::Result<tokio::task::JoinHandle<()>>>>,
)> {
    let provider = build_provider(config)?;

    let prometheus_reader: Option<crate::prometheus::SharedManualReader> =
        if config.internal_port.is_some() {
            Some(crate::prometheus::SharedManualReader::new())
        } else {
            None
        };

    let provider = if let Some(reader) = &prometheus_reader {
        build_provider_with_prometheus(config, reader.clone())?
    } else {
        provider
    };

    let prometheus_handle = if let Some(port) = config.internal_port {
        prometheus_reader
            .as_ref()
            .map(|reader| tokio::spawn(crate::prometheus::serve(reader.handle(), port)))
    } else {
        None
    };

    Ok((provider, prometheus_reader, prometheus_handle))
}

/// Construct the OTLP metric exporter based on the configured protocol
/// (FR-005, FR-023, FR-024).
///
/// When `protocol` is [`OtelProtocol::Http`], builds an OTLP/HTTP exporter
/// with protobuf encoding (FR-023). When [`OtelProtocol::Grpc`], builds an
/// OTLP/gRPC exporter via tonic (FR-024).
///
/// A bounded export timeout is applied (FR-031, FR-033) so that a slow or
/// unreachable endpoint cannot block the agent loop indefinitely. The
/// timeout is clamped to at least 1 second to avoid a zero-duration
/// timeout. On expiry the exporter surfaces an error that is logged at
/// `warn` level by the caller; it never panics.
#[cfg(feature = "telemetry")]
fn build_metric_exporter(config: &OtelConfig) -> Result<opentelemetry_otlp::MetricExporter> {
    use opentelemetry_otlp::WithExportConfig;
    use opentelemetry_sdk::metrics::Temporality;
    use ragent_config::OtelProtocol;
    use std::time::Duration;

    // Clamp the export timeout to at least 1s (FR-031): a zero-duration
    // timeout would make every export fail immediately, so we floor it.
    let timeout = Duration::from_secs(config.export_timeout_seconds.max(1));

    match config.protocol {
        OtelProtocol::Http => {
            // OTLP/HTTP with protobuf encoding (FR-023).
            let builder = opentelemetry_otlp::MetricExporter::builder()
                .with_http()
                .with_temporality(Temporality::default())
                .with_endpoint(config.endpoint.clone())
                .with_timeout(timeout);

            builder.build().map_err(|e| {
                crate::TelemetryError::ExporterInit(format!("OTLP HTTP exporter: {e}"))
            })
        }
        OtelProtocol::Grpc => {
            // OTLP/gRPC via tonic (FR-024).
            let builder = opentelemetry_otlp::MetricExporter::builder()
                .with_tonic()
                .with_temporality(Temporality::default())
                .with_endpoint(config.endpoint.clone())
                .with_timeout(timeout);

            builder.build().map_err(|e| {
                crate::TelemetryError::ExporterInit(format!("OTLP gRPC exporter: {e}"))
            })
        }
    }
}

/// Build the OTEL [`Resource`] with standard and custom attributes (FR-004).
///
/// The following static resource attributes are attached to all exported
/// metrics:
///
/// - `service.name` — from `config.service_name` (default `"ragent"`)
/// - `service.version` — from `CARGO_PKG_VERSION` at compile time
/// - `host.name` — best-effort system hostname lookup (FR-004)
///
/// Custom resource attributes from `config.resource_attributes` are merged
/// in (FR-026). The dynamic `session.id` is **not** a resource attribute —
/// it changes per session and is attached as a metric attribute via
/// [`InstrumentRegistry::attr_session`] (FR-025).
///
/// # Sensitive-data guard (FR-034)
///
/// Every resource attribute value is passed through
/// [`crate::sensitive::sanitize_attr_value`]. A user-defined
/// `resource_attributes` entry that accidentally contains an API key,
/// a credential, or file content is replaced with `"redacted"` rather
/// than exported as a resource attribute. The static `service.name`,
/// `service.version`, and `host.name` values are also sanitised for
/// defence-in-depth (e.g. a hostname that happens to contain a colon
/// between two dense parts would be redacted).
#[cfg(feature = "telemetry")]
fn build_resource(config: &OtelConfig) -> Resource {
    use crate::sensitive::sanitize_attr_value;

    let mut kvs = vec![
        KeyValue::new("service.name", sanitize_attr_value(&config.service_name)),
        KeyValue::new("service.version", env!("CARGO_PKG_VERSION").to_string()),
    ];

    // Add host.name from the system hostname when available (FR-004).
    if let Some(host) = hostname_str() {
        kvs.push(KeyValue::new("host.name", sanitize_attr_value(&host)));
    }

    // Merge in user-defined custom resource attributes (FR-026).
    // Each value is sanitised (FR-034) so a user who accidentally puts
    // an API key or secret into resource_attributes gets "redacted"
    // rather than a leaked credential.
    for (key, value) in &config.resource_attributes {
        kvs.push(KeyValue::new(key.clone(), sanitize_attr_value(value)));
    }

    Resource::builder_empty().with_attributes(kvs).build()
}

/// Best-effort system hostname lookup.
///
/// Returns `None` if the hostname cannot be determined (e.g. on minimal
/// containers). Never panics.
#[cfg(feature = "telemetry")]
fn hostname_str() -> Option<String> {
    // Use std::env::args or /proc/hostname; the simplest portable approach is
    // the `hostname` command via std::process::Command. However, to avoid
    // spawning a process at startup, we fall back to the `HOSTNAME` env var
    // (commonly set by shells) and then to reading /etc/hostname.
    if let Ok(h) = std::env::var("HOSTNAME")
        && !h.is_empty()
    {
        return Some(h);
    }
    if let Ok(bytes) = std::fs::read("/etc/hostname") {
        let h = String::from_utf8_lossy(&bytes).trim().to_string();
        if !h.is_empty() {
            return Some(h);
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
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
        let mut config = OtelConfig::default();
        config.enabled = true;

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
        let mut config = OtelConfig::default();
        config.service_name = "test-agent".to_string();
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
        let mut config = OtelConfig::default();
        config.enabled = true;
        config.endpoint = "http://localhost:4318".to_string();

        // The PeriodicReader needs a Tokio runtime context.
        let rt = tokio::runtime::Runtime::new().expect("tokio runtime");
        let sub =
            rt.block_on(async { TelemetrySubsystem::new(config).expect("enabled subsystem") });
        assert_eq!(sub.state(), TelemetryState::Enabled);
        assert!(
            sub.provider().is_some(),
            "enabled subsystem should have a provider"
        );
    }

    #[cfg(feature = "telemetry")]
    #[test]
    fn test_enabled_subsystem_shutdown_succeeds() {
        let mut config = OtelConfig::default();
        config.enabled = true;
        config.endpoint = "http://localhost:4318".to_string();

        let rt = tokio::runtime::Runtime::new().expect("tokio runtime");
        let sub =
            rt.block_on(async { TelemetrySubsystem::new(config).expect("enabled subsystem") });
        // shutdown should succeed even though there's no real collector
        assert!(sub.shutdown().is_ok());
    }

    #[cfg(feature = "telemetry")]
    #[test]
    fn test_invalid_endpoint_returns_error() {
        let mut config = OtelConfig::default();
        config.enabled = true;
        config.endpoint = "ftp://bad".to_string();

        let result = TelemetrySubsystem::new(config);
        assert!(result.is_err(), "invalid endpoint should error");
    }

    #[cfg(feature = "telemetry")]
    #[test]
    fn test_empty_endpoint_returns_error() {
        let mut config = OtelConfig::default();
        config.enabled = true;
        config.endpoint = String::new();

        let result = TelemetrySubsystem::new(config);
        assert!(result.is_err(), "empty endpoint should error");
    }

    #[cfg(feature = "telemetry")]
    #[test]
    fn test_http_protocol_builds_successfully() {
        // FR-023: OTLP/HTTP exporter wiring must construct without error
        // when a valid HTTP endpoint is provided.
        let mut config = OtelConfig::default();
        config.enabled = true;
        config.endpoint = "http://localhost:4318".to_string();
        config.protocol = ragent_config::OtelProtocol::Http;

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
        let mut config = OtelConfig::default();
        config.enabled = true;
        config.endpoint = "https://collector.example.com:4318".to_string();
        config.protocol = ragent_config::OtelProtocol::Http;

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
        let mut config = OtelConfig::default();
        config.enabled = true;
        config.endpoint = "http://localhost:4317".to_string();
        config.protocol = ragent_config::OtelProtocol::Grpc;

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
        let mut config = OtelConfig::default();
        config.enabled = true;
        config.endpoint = "http://my-collector:9999".to_string();
        config.protocol = ragent_config::OtelProtocol::Http;

        let rt = tokio::runtime::Runtime::new().expect("tokio runtime");
        let sub = rt.block_on(async {
            TelemetrySubsystem::new(config).expect("subsystem should construct")
        });
        // The provider should exist; we can't inspect the internal endpoint
        // without a mock collector, but construction success proves the
        // endpoint was accepted.
        assert!(sub.provider().is_some());
        assert_eq!(sub.config().endpoint, "http://my-collector:9999");
    }
}
