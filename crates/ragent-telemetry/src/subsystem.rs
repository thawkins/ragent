//! The telemetry subsystem - owner of the meter provider and exporter lifecycle.
//!
//! [`TelemetrySubsystem`] is the single handle that the rest of ragent
//! interacts with. It encapsulates whether telemetry is enabled or disabled
//! (FR-021, FR-022) and, when enabled, holds the live
//! [`SdkMeterProvider`](https://docs.rs/opentelemetry_sdk/0.27/opentelemetry_sdk/metrics/struct.SdkMeterProvider.html)
//! and OTLP exporter.
//!
//! # No-op fallback (NFR-002)
//!
//! When telemetry is disabled - either via configuration or because the
//! `telemetry` Cargo feature is off - the subsystem uses the OpenTelemetry
//! `NoopMeterProvider`. All instrument calls (`.add()`, `.record()`) are
//! cheap no-ops with zero network traffic (FR-022, FR-032).

use crate::Result;
#[cfg(not(feature = "telemetry"))]
use crate::TelemetryError;
use ragent_config::OtelConfig;

/// Whether the subsystem is actively exporting or running as a no-op.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TelemetryState {
    /// Telemetry is disabled - a no-op meter provider is in use (FR-022, NFR-002).
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
    /// Threads still shutting down pipelines detached by
    /// [`reconfigure`](Self::reconfigure). [`shutdown`](Self::shutdown) joins
    /// them so a process exiting right after `/telemetry off` still delivers
    /// the retired pipeline's final export. Kept outside `runtime` so joining
    /// never holds the state lock.
    #[cfg_attr(not(feature = "telemetry"), allow(dead_code))]
    retiring: parking_lot::Mutex<Vec<std::thread::JoinHandle<()>>>,
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
    /// SEC-ragent-telemetry-003 (SECTASKS T-032): the per-metric cardinality
    /// cap lives here, created once when the provider is built, and is cloned
    /// into every [`InstrumentRegistry`]. Previously each `instruments()` call
    /// allocated a fresh cache, so it only ever observed one recording and the
    /// cap never accumulated process-wide.
    #[cfg(feature = "telemetry")]
    cardinality: std::sync::Arc<crate::cardinality::CardinalityCache>,
    /// The join handle for the Prometheus HTTP server task (FR-028).
    /// `None` when no `internal_port` is configured. The handle is for the
    /// outer task that resolves to the inner server handle (or a bind
    /// error); we store it so it can be aborted on shutdown/reconfigure.
    #[cfg(feature = "telemetry")]
    prometheus_handle:
        Option<tokio::task::JoinHandle<std::io::Result<tokio::task::JoinHandle<()>>>>,
    /// The OTLP logs-signal pipeline (spec `otel` FR-036). `None` when
    /// `telemetry.otel.logs.enabled` is `false` (the default). Flushed and
    /// shut down alongside the meter provider so the last records of a run
    /// are not lost with the batch buffer.
    #[cfg(feature = "telemetry")]
    logger: Option<crate::logs::LoggerProviderHandle>,
    /// The swap cell of the installed [`TracingLogsLayer`] (spec `otel`
    /// FR-036). `None` until [`TelemetrySubsystem::logs_layer`] hands the
    /// process's layer to the subsystem; while it is empty the logs signal is
    /// configured but has nowhere to send records, which is the state of a
    /// process whose tracing subscriber was installed without the layer (for
    /// example inside a libtest harness).
    ///
    /// [`TracingLogsLayer`]: crate::logs::TracingLogsLayer
    #[cfg(feature = "telemetry")]
    logs_cell: Option<crate::logs::ExportCell>,
}

impl RuntimeState {
    /// Construct a disabled runtime state with the given config.
    #[cfg(feature = "telemetry")]
    fn disabled(config: OtelConfig) -> Self {
        let cardinality = std::sync::Arc::new(crate::cardinality::CardinalityCache::new(
            config.cardinality_limit,
        ));
        Self {
            state: TelemetryState::Disabled,
            config,
            provider: None,
            cardinality,
            prometheus_handle: None,
            logger: None,
            logs_cell: None,
        }
    }

    /// Construct a disabled runtime state with the given config
    /// (feature-off variant - no provider fields exist).
    #[cfg(not(feature = "telemetry"))]
    fn disabled(config: OtelConfig) -> Self {
        Self {
            state: TelemetryState::Disabled,
            config,
        }
    }
}

/// A detached export pipeline waiting to be shut down by
/// [`TelemetrySubsystem::reconfigure`].
#[cfg(feature = "telemetry")]
struct RetiringPipeline {
    logger: Option<crate::logs::LoggerProviderHandle>,
    provider: Option<std::sync::Arc<opentelemetry_sdk::metrics::SdkMeterProvider>>,
}

#[cfg(feature = "telemetry")]
impl RetiringPipeline {
    /// Detach the live pipeline from `state`: abort the Prometheus server (so
    /// its port is free for a rebuilt one), stop the logs layer pointing at a
    /// provider about to be shut down, and take the providers.
    fn take(state: &mut RuntimeState) -> Self {
        if let Some(handle) = state.prometheus_handle.take() {
            handle.abort();
        }
        let logger = state.logger.take();
        if logger.is_some()
            && let Some(cell) = &state.logs_cell
        {
            // A record emitted in between must not reach a dead processor.
            *cell.write() = None;
        }
        Self {
            logger,
            provider: state.provider.take(),
        }
    }

    /// Shut the detached providers down on a background thread, delivering
    /// the records and metrics recorded up to the detach. Logs go first so a
    /// collector timeline keeps the order events happened in. Errors are
    /// logged only: the pipeline is being discarded regardless.
    fn retire(self) -> Option<std::thread::JoinHandle<()>> {
        if self.logger.is_none() && self.provider.is_none() {
            return None;
        }
        let shutdown = move || {
            if let Some(logger) = self.logger
                && let Err(e) = logger.shutdown()
                && !matches!(e, opentelemetry_sdk::error::OTelSdkError::AlreadyShutdown)
            {
                tracing::warn!(error = %e, "OTEL log provider shutdown during reconfigure");
            }
            if let Some(provider) = self.provider
                && let Err(e) = provider.shutdown()
            {
                tracing::warn!(error = %e, "OTEL meter provider shutdown during reconfigure");
            }
        };
        // `spawn` only fails when the OS refuses a thread. The closure (and
        // the providers it owns) is then dropped without the final export,
        // which the warning records.
        let builder = std::thread::Builder::new().name("otel-retire".to_string());
        match builder.spawn(shutdown) {
            Ok(handle) => Some(handle),
            Err(e) => {
                tracing::warn!(error = %e, "could not spawn OTEL retire thread; final export skipped");
                None
            }
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
                // Shared construction path (also used by `reconfigure`): builds
                // the provider, the (optional) Prometheus reader, and the
                // spawned scrape-server handle (see `ANTIPAT.md` M3.13). The
                // reader is consumed by `build_provider`; only the handle is
                // retained on `RuntimeState`.
                let (provider, _prometheus_reader, prometheus_handle) =
                    build_enabled_provider(&config)?;
                // A logs pipeline that cannot be built must not take the
                // metrics signal down with it (FR-041): `build_logs_pipeline`
                // logs the failure and returns `None`.
                let logger = build_logs_pipeline(&config);

                let cardinality = std::sync::Arc::new(crate::cardinality::CardinalityCache::new(
                    config.cardinality_limit,
                ));
                Ok(Self {
                    retiring: parking_lot::Mutex::default(),
                    runtime: parking_lot::Mutex::new(RuntimeState {
                        state: TelemetryState::Enabled,
                        config,
                        provider: Some(std::sync::Arc::new(provider)),
                        cardinality,
                        prometheus_handle,
                        logger,
                        logs_cell: None,
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
        let cardinality = std::sync::Arc::new(crate::cardinality::CardinalityCache::new(
            config.cardinality_limit,
        ));
        Self {
            retiring: parking_lot::Mutex::default(),
            runtime: parking_lot::Mutex::new(RuntimeState {
                state: TelemetryState::Enabled,
                config,
                provider: Some(std::sync::Arc::new(provider)),
                cardinality,
                prometheus_handle: None,
                // A test that injects its own meter provider owns the log
                // pipeline too; nothing is exported unless it opts in.
                logger: None,
                logs_cell: None,
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

    fn disabled_with_config(config: OtelConfig) -> Self {
        Self {
            retiring: parking_lot::Mutex::default(),
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

    /// Attach the process's [`TracingLogsLayer`] cell to this subsystem.
    ///
    /// The tracing subscriber is global and is installed once, before the
    /// telemetry subsystem exists, so the layer cannot hold the subsystem. It
    /// holds a swap cell instead, and this call hands that cell to the
    /// subsystem: from then on [`reconfigure`](Self::reconfigure) fills it in
    /// when `telemetry.otel.logs.enabled` is set and clears it when the signal
    /// is off, which is what lets `/telemetry on|off` change log export in a
    /// running process without reinstalling the subscriber.
    ///
    /// Calling this with the same cell more than once is harmless; a later call
    /// with a different cell detaches the earlier one (it is cleared first, so
    /// it stops exporting).
    ///
    /// [`TracingLogsLayer`]: crate::logs::TracingLogsLayer
    #[cfg(feature = "telemetry")]
    pub fn attach_logs_layer(&self, cell: crate::logs::ExportCell) {
        let mut guard = self.runtime.lock();
        if let Some(previous) = guard.logs_cell.take() {
            // Two layers can only exist if a caller installed a second
            // subscriber; clearing the previous cell stops it duplicating every
            // record into the same pipeline.
            *previous.write() = None;
        }
        guard.logs_cell = Some(cell);
        install_logs_cell(&guard);
    }

    /// Returns `true` while log records are being exported to the collector
    /// (spec `otel` FR-036).
    ///
    /// This is `false` when telemetry is off, when the `logs` block is disabled,
    /// and when no [`TracingLogsLayer`](crate::logs::TracingLogsLayer) has been
    /// attached even though the block is enabled.
    #[must_use]
    pub fn logs_exporting(&self) -> bool {
        #[cfg(feature = "telemetry")]
        {
            let guard = self.runtime.lock();
            guard
                .logs_cell
                .as_ref()
                .is_some_and(|cell| cell.read().is_some())
        }
        #[cfg(not(feature = "telemetry"))]
        {
            false
        }
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
    /// disabled, returns `None` - callers should use a no-op registry
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
                .with_shared_cardinality(guard.cardinality.clone())
                .with_metric_toggles(guard.config.metrics.clone()),
        )
    }

    /// Reconfigure the subsystem at runtime from a new [`OtelConfig`].
    ///
    /// This is the runtime-toggle entry point used by the `/telemetry on|off`
    /// slash commands. Under the runtime mutex it:
    ///
    /// 1. Detaches the currently live provider (if any) and aborts any
    ///    Prometheus HTTP server task so its TCP listener is released.
    /// 2. If the new config is enabled (and the `telemetry` feature is
    ///    active), builds a fresh provider, Prometheus endpoint and
    ///    cardinality cache.
    /// 3. Updates the stored state and config so subsequent calls to
    ///    [`is_enabled`](Self::is_enabled), [`config`](Self::config), and
    ///    [`instruments`](Self::instruments) reflect the new state.
    ///
    /// The detached provider is then shut down on a background thread, after
    /// the lock is released: its final export can take up to the export
    /// timeout, and must stall neither the caller (the TUI event loop) nor
    /// readers of the subsystem state. Shutting it down stops the periodic
    /// OTLP reader and therefore the "Failed to export metrics" log noise
    /// that would otherwise continue after `/telemetry off`.
    ///
    /// When the `telemetry` Cargo feature is off, only the config/state are
    /// updated (there is no provider to shut down or build).
    ///
    /// # Errors
    ///
    /// Returns [`TelemetryError::InvalidEndpoint`] or
    /// [`TelemetryError::ExporterInit`] if building a new enabled provider
    /// fails. On such a failure the subsystem is left in the **disabled**
    /// state (the old provider has already been detached) so the agent
    /// loop never continues with a half-initialised provider.
    pub fn reconfigure(&self, config: OtelConfig) -> Result<()> {
        let mut guard = self.runtime.lock();

        // A reconfigure means a brand-new export pipeline (or none at all), so
        // the previous streak of export failures no longer describes anything.
        // Clearing also releases a latched auto-disable, which is what lets
        // `/telemetry on` retry an endpoint that previously failed.
        // The new generation also stops the retired pipeline's late results
        // from being counted against the replacement.
        let health = crate::export_health::export_health();
        health.clear();
        health.begin_generation();

        // 1. Detach the existing pipeline. It is shut down below, after the
        // lock is released.
        #[cfg(feature = "telemetry")]
        let retiring = RetiringPipeline::take(&mut guard);

        let result = Self::apply_config(&mut guard, config);
        drop(guard);

        // Shutting the old pipeline down performs its final export, which can
        // take up to the export timeout against a dead collector. Doing that
        // under the runtime lock stalled every `is_enabled()` caller (the TUI
        // status bar reads it each frame), and doing it on the caller's thread
        // froze the TUI on `/telemetry off`; it now runs on its own thread.
        #[cfg(feature = "telemetry")]
        if let Some(handle) = retiring.retire() {
            let mut threads = self.retiring.lock();
            threads.retain(|t| !t.is_finished());
            threads.push(handle);
        }
        result
    }

    /// Steps 2-3 of [`reconfigure`](Self::reconfigure): build the pipeline
    /// `config` asks for (if any) and point the logs layer at it.
    fn apply_config(state: &mut RuntimeState, config: OtelConfig) -> Result<()> {
        // 2. Build the new provider if enabled.
        if config.enabled {
            #[cfg(feature = "telemetry")]
            {
                match build_enabled_provider(&config) {
                    Ok((provider, _prometheus_reader, prometheus_handle)) => {
                        // The logs pipeline is rebuilt too, so a re-pointed
                        // endpoint/level takes effect without a restart. A
                        // logs failure leaves metrics running (FR-041).
                        let logger = build_logs_pipeline(&config);
                        state.state = TelemetryState::Enabled;
                        state.config = config;
                        state.provider = Some(std::sync::Arc::new(provider));
                        // A fresh provider starts with no series, so the
                        // cardinality cap starts over too - and picks up a
                        // changed `cardinality_limit`, which the old cache
                        // kept until the next restart.
                        state.cardinality =
                            std::sync::Arc::new(crate::cardinality::CardinalityCache::new(
                                state.config.cardinality_limit,
                            ));
                        state.prometheus_handle = prometheus_handle;
                        state.logger = logger;
                    }
                    Err(e) => {
                        // Building the new provider failed - leave the
                        // subsystem disabled so the agent loop never runs
                        // with a half-initialised provider.
                        tracing::warn!(error = %e, "reconfigure: provider build failed; leaving telemetry disabled");
                        state.state = TelemetryState::Disabled;
                        state.config = config;
                        state.provider = None;
                        state.logger = None;
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
                state.state = TelemetryState::Disabled;
                state.config = config;
                return Err(TelemetryError::FeatureNotEnabled);
            }
        } else {
            state.state = TelemetryState::Disabled;
            state.config = config;
            #[cfg(feature = "telemetry")]
            {
                state.provider = None;
                state.logger = None;
            }
        }

        // 3. Point the installed logs layer at whatever pipeline now exists
        // (or at nothing, when the signal is off).
        #[cfg(feature = "telemetry")]
        install_logs_cell(state);
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
    /// returned here - the caller **must not** propagate it in a way that
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
            // Clone the provider handle so the blocking flush never runs while
            // `self.runtime` is locked - `reconfigure` needs that lock. The log
            // pipeline is cloned for the same reason and drained first: its
            // records are the ones emitted while the metrics were last being
            // recorded, so flushing it first keeps a collector timeline in the
            // order the events happened.
            let (logger, provider) = {
                let guard = self.runtime.lock();
                (
                    guard.logger.clone(),
                    guard.provider.as_ref().map(std::sync::Arc::clone),
                )
            };
            // A failed log flush must not skip the metric flush: the two
            // signals are independent, and returning early here dropped the
            // last metric batch whenever the logs endpoint was unhealthy.
            let mut first_error = None;
            if let Some(logger) = logger
                && let Err(e) = logger.force_flush()
                && !matches!(e, opentelemetry_sdk::error::OTelSdkError::AlreadyShutdown)
            {
                let kind = crate::export_health::ExportFailureKind::classify(&e.to_string());
                tracing::warn!(
                    reason = kind.describe(),
                    "OTEL log provider force_flush error: {e}"
                );
                first_error = Some(crate::TelemetryError::ExporterInit(format!(
                    "log flush failed: {e}"
                )));
            }
            if let Some(provider) = provider
                && let Err(e) = provider.force_flush()
            {
                // Route the SDK's opaque text through the same classifier the
                // Agent Notice uses, so a shutdown log says "export timed out"
                // rather than the raw `oneshot canceled` internal failure.
                let kind = crate::export_health::ExportFailureKind::classify(&e.to_string());
                tracing::warn!(
                    reason = kind.describe(),
                    "OTEL meter provider force_flush error: {e}"
                );
                first_error.get_or_insert(crate::TelemetryError::ExporterInit(format!(
                    "flush failed: {e}"
                )));
            }
            if let Some(e) = first_error {
                return Err(e);
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
            // Clone the handles and release the lock before the blocking
            // shutdown, as `flush` does, so state readers are not stalled for
            // the length of the final export.
            let (logger, provider) = {
                let guard = self.runtime.lock();
                if let Some(handle) = &guard.prometheus_handle {
                    handle.abort();
                }
                (
                    guard.logger.clone(),
                    guard.provider.as_ref().map(std::sync::Arc::clone),
                )
            };
            // Shut both signals down even when the first fails; returning
            // early left the meter provider running and its final batch
            // unexported whenever the log provider reported an error.
            let mut first_error = None;
            if let Some(logger) = &logger
                && let Err(e) = logger.shutdown()
                && !matches!(e, opentelemetry_sdk::error::OTelSdkError::AlreadyShutdown)
            {
                tracing::warn!("OTEL log provider shutdown error: {e}");
                first_error = Some(crate::TelemetryError::ExporterInit(format!(
                    "log shutdown failed: {e}"
                )));
            }
            if let Some(provider) = &provider
                && let Err(e) = provider.shutdown()
            {
                tracing::warn!("OTEL meter provider shutdown error: {e}");
                first_error.get_or_insert(crate::TelemetryError::ExporterInit(format!(
                    "shutdown failed: {e}"
                )));
            }
            // Wait for pipelines retired by `reconfigure` to finish their final
            // export. The handles are taken first so the join holds no lock.
            let retiring = std::mem::take(&mut *self.retiring.lock());
            for handle in retiring {
                if handle.join().is_err() {
                    tracing::warn!("OTEL retire thread panicked");
                }
            }
            if let Some(e) = first_error {
                return Err(e);
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
            // SEC-055: the endpoint may embed credentials in its userinfo
            // (`https://user:secret@collector:4318`); never echo them into
            // Debug output.
            .field("endpoint", &mask_endpoint_userinfo(&guard.config.endpoint))
            .field("protocol", &guard.config.protocol)
            .finish()
    }
}

/// Mask the userinfo portion of an endpoint URL for safe logging (SEC-055).
///
/// An OTLP endpoint may embed credentials in its authority
/// (`https://user:secret@collector:4318`). This replaces everything before the
/// `@` with `***` so the value can be placed into `Debug`, error, or tracing
/// output without leaking the credentials. A URL without userinfo (or an
/// unparseable value with no `://`) is returned unchanged.
///
/// # Arguments
///
/// * `endpoint` - The raw endpoint string to be logged.
///
/// # Examples
///
/// ```
/// use ragent_telemetry::subsystem::mask_endpoint_userinfo;
///
/// assert_eq!(
///     mask_endpoint_userinfo("https://user:secret@collector:4318"),
///     "https://***@collector:4318"
/// );
/// assert_eq!(
///     mask_endpoint_userinfo("http://localhost:4317"),
///     "http://localhost:4317"
/// );
/// ```
#[must_use]
pub fn mask_endpoint_userinfo(endpoint: &str) -> String {
    // Locate the authority start (after "scheme://"); the authority runs until
    // the first '/', '?', or '#'.
    let Some(scheme_end) = endpoint.find("://") else {
        return endpoint.to_string();
    };
    let auth_start = scheme_end + 3;
    let rest = &endpoint[auth_start..];
    let auth_end = rest.find(['/', '?', '#']).unwrap_or(rest.len());
    let authority = &rest[..auth_end];
    // Use the LAST '@' so a malformed userinfo containing '@' cannot mislead
    // the host boundary.
    let Some(at) = authority.rfind('@') else {
        return endpoint.to_string();
    };
    let mut masked = String::with_capacity(endpoint.len());
    masked.push_str(&endpoint[..auth_start]);
    masked.push_str("***@");
    // Keep the host[:port] and everything after the authority verbatim.
    masked.push_str(&authority[at + 1..]);
    masked.push_str(&rest[auth_end..]);
    masked
}

// -- Provider construction (feature-gated) ------------------------------

#[cfg(feature = "telemetry")]
use opentelemetry::KeyValue;
#[cfg(feature = "telemetry")]
use opentelemetry_sdk::Resource;
#[cfg(feature = "telemetry")]
use opentelemetry_sdk::metrics::SdkMeterProvider;

/// Validate the configured OTLP endpoint (FR-021).
///
/// The endpoint must be non-empty and start with `http://` or `https://`.
/// Shared by both provider-construction paths so the contract is defined once
/// (see `ANTIPAT.md` M3.13).
#[cfg(feature = "telemetry")]
fn validate_endpoint(config: &OtelConfig) -> Result<()> {
    if config.endpoint.is_empty() {
        return Err(crate::TelemetryError::InvalidEndpoint(
            "endpoint is empty".to_string(),
        ));
    }
    if !ragent_types::strutil::is_http_url(&config.endpoint) {
        // SEC-055: never echo the raw endpoint - it may embed userinfo.
        return Err(crate::TelemetryError::InvalidEndpoint(format!(
            "endpoint must be HTTP or HTTPS URL: {}",
            mask_endpoint_userinfo(&config.endpoint)
        )));
    }
    Ok(())
}

/// Build the live `SdkMeterProvider` from the config (FR-021).
///
/// Wires an OTLP/HTTP exporter into a periodic reader with the configured
/// export interval, attaches resource attributes, and builds the provider.
/// When `prometheus_reader` is `Some`, the same provider also serves the
/// Prometheus scrape path (FR-028).
///
/// # Why the default reader (no Tokio runtime required)
///
/// The SDK's default
/// [`PeriodicReader`](opentelemetry_sdk::metrics::PeriodicReader) drives
/// exports from a plain OS thread via `futures_executor::block_on`, i.e.
/// outside any Tokio context. That is only safe if the exporter's `export`
/// call never touches a reactor, so this crate builds the OTLP exporter with
/// `opentelemetry-otlp`'s **blocking** reqwest client
/// (`reqwest-blocking-client`) rather than the async one. The async client
/// calls `tokio::time::sleep` inside its export future, and on the very first
/// (immediate) export that panicked with "there is no reactor running" on the
/// reader's OS thread - and because a panic on any thread runs the
/// process-wide hook that restores the terminal, telemetry-enabled startup
/// tore down the TUI alternate screen.
///
/// The async-runtime reader is deliberately **not** used: it binds the export
/// task to whichever runtime was current when the reader was built, so
/// [`TelemetrySubsystem::flush`] (a synchronous `force_flush`) deadlocks on a
/// current-thread runtime and hangs forever if that runtime has since
/// stopped. Keeping the exporter blocking removes the coupling entirely, so
/// the provider can be built and flushed from any thread.
#[cfg(feature = "telemetry")]
fn build_provider(
    config: &OtelConfig,
    prometheus_reader: Option<crate::prometheus::SharedManualReader>,
) -> Result<SdkMeterProvider> {
    use std::time::Duration;

    validate_endpoint(config)?;

    // Build the OTLP metric exporter (HTTP/protobuf by default, gRPC if configured).
    let exporter = build_metric_exporter(config)?;

    // Wrap the exporter so every export result (success, non-2xx status,
    // transport error, or export timeout) is recorded in the shared
    // `ExportHealth` monitor. The TUI drains that monitor and renders an
    // "Agent Notice" per failure, switching telemetry off after
    // `AUTO_DISABLE_THRESHOLD` consecutive failures. The wrapper forwards the
    // result unchanged, so the SDK still logs it exactly as before.
    let exporter = crate::export_health::ObservedMetricExporter::new(
        exporter,
        crate::subsystem::mask_endpoint_userinfo(&config.endpoint),
    );

    // Build a periodic reader with the configured export interval (FR-006).
    //
    // The default `PeriodicReader` is correct here because the exporter is
    // blocking (see the doc comment above): its export loop runs on its own
    // OS thread and needs no reactor, which is what lets `flush` and
    // `shutdown` be called from any thread.
    let interval = Duration::from_secs(config.export_interval_seconds.max(1));
    let reader = opentelemetry_sdk::metrics::PeriodicReader::builder(exporter)
        .with_interval(interval)
        .build();

    // Build the resource with service.name, service.version, and custom attributes (FR-004).
    let resource = build_resource(config);

    let mut builder = SdkMeterProvider::builder()
        .with_resource(resource)
        .with_reader(reader);
    if let Some(prometheus_reader) = prometheus_reader {
        builder = builder.with_reader(prometheus_reader);
    }

    Ok(builder.build())
}

/// Build the live provider plus optional Prometheus reader/handle from a
/// config that is known to be enabled.
///
/// This is the shared construction path used by both [`TelemetrySubsystem::new`]
/// (indirectly, inlined) and [`TelemetrySubsystem::reconfigure`]. It returns
/// the three pieces a caller needs to populate [`RuntimeState`].
///
/// Requires a Tokio runtime context only when `config.internal_port` is set,
/// because the Prometheus `serve` future is spawned via [`tokio::spawn`]; that
/// case is checked explicitly and returns an error instead of panicking. The
/// OTLP exporter and its periodic reader are fully blocking and need no
/// runtime.
#[cfg(feature = "telemetry")]
fn build_enabled_provider(
    config: &OtelConfig,
) -> Result<(
    SdkMeterProvider,
    Option<crate::prometheus::SharedManualReader>,
    Option<tokio::task::JoinHandle<std::io::Result<tokio::task::JoinHandle<()>>>>,
)> {
    let prometheus_reader: Option<crate::prometheus::SharedManualReader> =
        if config.internal_port.is_some() {
            Some(crate::prometheus::SharedManualReader::new())
        } else {
            None
        };

    let provider = build_provider(config, prometheus_reader.clone())?;

    let prometheus_handle = if let Some(port) = config.internal_port {
        // The scrape server is spawned as a tokio task, so this is the one
        // requirement for a runtime context. Fail loudly with a typed error
        // rather than letting `tokio::spawn` panic with "there is no reactor
        // running" after the provider (and its reader thread) already exist.
        if tokio::runtime::Handle::try_current().is_err() {
            return Err(crate::TelemetryError::ExporterInit(
                "telemetry.internal_port requires a Tokio runtime context to serve the                  Prometheus scrape endpoint"
                    .to_string(),
            ));
        }
        prometheus_reader
            .as_ref()
            .map(|reader| tokio::spawn(crate::prometheus::serve(reader.handle(), port)))
    } else {
        None
    };

    Ok((provider, prometheus_reader, prometheus_handle))
}

/// Resolve the OTLP/HTTP metrics URL from a configured endpoint.
///
/// The configured `telemetry.otel.endpoint` is a base URL
/// (`http://localhost:4318`). `opentelemetry-otlp` treats a programmatic
/// `with_endpoint` value as the complete signal URL and posts to it verbatim,
/// so the bare base URL reached the collector as `POST /` and a stock collector
/// answered `404`. The SDK appends `/v1/metrics` only to its own default (or to
/// the spec's `OTEL_EXPORTER_OTLP_ENDPOINT` base-URL variable), so ragent
/// appends it here.
///
/// An endpoint that already names a signal path is left alone, which keeps
/// both forms working:
///
/// | configured endpoint | posted to |
/// | --- | --- |
/// | `http://localhost:4318` | `http://localhost:4318/v1/metrics` |
/// | `http://localhost:4318/` | `http://localhost:4318/v1/metrics` |
/// | `https://gw.example.com/otel` | `https://gw.example.com/otel/v1/metrics` |
/// | `https://gw.example.com/p/v1/metrics` | unchanged |
///
/// Query strings and fragments are preserved after the appended path.
#[cfg(feature = "telemetry")]
pub fn otlp_http_metrics_endpoint(endpoint: &str) -> String {
    otlp_http_signal_endpoint(endpoint, "metrics")
}

/// Resolve the OTLP/HTTP URL for a signal from a configured base URL.
///
/// `signal` is the OTLP signal path segment: `"metrics"`, `"logs"` or
/// `"traces"`. See [`otlp_http_metrics_endpoint`] for the reasoning; the logs
/// signal (spec `otel` FR-036) reuses the same resolver so one configured
/// `telemetry.otel.endpoint` addresses both signals.
///
/// An endpoint that already names a `v1/<signal>` path is returned unchanged,
/// whichever signal it names: a deployment that points `endpoint` at
/// `.../v1/metrics` has made an explicit choice, and silently rewriting it to
/// `.../v1/logs` would send metrics to the logs receiver.
#[cfg(feature = "telemetry")]
pub fn otlp_http_signal_endpoint(endpoint: &str, signal: &str) -> String {
    /// Any `v1/<signal>` path segment means the operator named the signal URL.
    const SIGNAL_SEGMENT: &str = "v1/";

    let endpoint = endpoint.trim();
    let (base, suffix) = match endpoint.find(['?', '#']) {
        Some(index) => endpoint.split_at(index),
        None => (endpoint, ""),
    };

    if base.contains(SIGNAL_SEGMENT) {
        return endpoint.to_string();
    }

    format!("{}/v1/{signal}{suffix}", base.trim_end_matches('/'))
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
            //
            // The SDK treats a programmatic `with_endpoint` value as the
            // COMPLETE signal URL and posts straight to it; only the default
            // (or the spec's base-URL env var) gets the `/v1/metrics` suffix
            // appended by `build_endpoint_uri`. Passing the bare base URL
            // therefore POSTs to `http://localhost:4318/`, which a stock
            // collector answers with 404. Normalise to the signal URL here so
            // a configured base URL works as the docs describe it.
            let builder = opentelemetry_otlp::MetricExporter::builder()
                .with_http()
                .with_temporality(Temporality::default())
                .with_endpoint(otlp_http_metrics_endpoint(&config.endpoint))
                .with_timeout(timeout);

            builder.build().map_err(|e| {
                crate::TelemetryError::ExporterInit(format!("OTLP HTTP exporter: {e}"))
            })
        }
        OtelProtocol::Grpc => {
            // SEC-ragent-telemetry-001 (SECTASKS T-019): defence in depth.
            // TLS is now compiled in (`tls-roots`), but a non-loopback
            // `https://` endpoint whose scheme would be downgraded is refused
            // outright so a cleartext export can never happen silently.
            ensure_grpc_endpoint_is_encrypted(&config.endpoint)?;

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

/// Build the logs pipeline when `telemetry.otel.logs.enabled` is set.
///
/// Returns `None` when the signal is off, and logs (rather than propagates) a
/// build failure: a broken logs exporter must never stop the metrics signal or
/// the process (FR-041), so the caller keeps running with metrics only.
#[cfg(feature = "telemetry")]
fn build_logs_pipeline(config: &OtelConfig) -> Option<crate::logs::LoggerProviderHandle> {
    if !config.logs.enabled {
        return None;
    }
    match crate::logs::build_logger_provider(config) {
        Ok(provider) => Some(provider),
        Err(e) => {
            let kind = crate::export_health::ExportFailureKind::classify(&e.to_string());
            tracing::warn!(
                failure = kind.describe(),
                "OTLP log export disabled: the log exporter could not be built: {e}"
            );
            None
        }
    }
}

/// Point the attached logs layer at the current pipeline (or at nothing).
///
/// # Lock order
///
/// The runtime lock is held by the caller and the cell lock is taken here, so
/// the order is always runtime -> cell. [`TracingLogsLayer::on_event`] takes
/// only the cell lock and never calls back into the subsystem, so no path takes
/// them the other way round.
///
/// [`TracingLogsLayer::on_event`]: crate::logs::TracingLogsLayer
#[cfg(feature = "telemetry")]
fn install_logs_cell(guard: &RuntimeState) {
    let Some(cell) = &guard.logs_cell else {
        return;
    };
    // The `logs` block selects the signal: a reconfigure that leaves it
    // disabled must detach the layer even though a provider exists, or log
    // records would keep leaving the machine after the operator turned the
    // signal off.
    let (Some(logger), true) = (&guard.logger, guard.config.logs.enabled) else {
        *cell.write() = None;
        return;
    };
    let active = match crate::logs::build_export(&guard.config, logger) {
        Ok(active) => Some(active),
        Err(e) => {
            tracing::warn!(error = %e, "attaching the OTLP log layer failed");
            None
        }
    };
    *cell.write() = active;
}

/// Construct the OTLP log exporter, mirroring [`build_metric_exporter`].
///
/// The logs signal travels to the same collector as metrics, on the same
/// protocol, with the same timeout and the same TLS rules:
///
/// - HTTP posts to the `/v1/logs` sibling of the metrics path, so a configured
///   base URL works for both signals without a second setting;
/// - gRPC reuses [`ensure_grpc_endpoint_is_encrypted`], so a cleartext
///   non-loopback endpoint is refused here exactly as it is for metrics.
#[cfg(feature = "telemetry")]
pub(crate) fn build_log_exporter(config: &OtelConfig) -> Result<opentelemetry_otlp::LogExporter> {
    use opentelemetry_otlp::WithExportConfig;
    use ragent_config::OtelProtocol;
    use std::time::Duration;

    let timeout = Duration::from_secs(config.export_timeout_seconds.max(1));

    match config.protocol {
        OtelProtocol::Http => opentelemetry_otlp::LogExporter::builder()
            .with_http()
            .with_endpoint(otlp_http_signal_endpoint(&config.endpoint, "logs"))
            .with_timeout(timeout)
            .build()
            .map_err(|e| {
                crate::TelemetryError::ExporterInit(format!("OTLP HTTP log exporter: {e}"))
            }),
        OtelProtocol::Grpc => {
            ensure_grpc_endpoint_is_encrypted(&config.endpoint)?;
            opentelemetry_otlp::LogExporter::builder()
                .with_tonic()
                .with_endpoint(config.endpoint.clone())
                .with_timeout(timeout)
                .build()
                .map_err(|e| {
                    crate::TelemetryError::ExporterInit(format!("OTLP gRPC log exporter: {e}"))
                })
        }
    }
}

/// Refuse an OTLP gRPC endpoint that would be contacted in cleartext.
///
/// SEC-ragent-telemetry-001 (SECTASKS T-019): only `https://` (TLS) and
/// loopback `http://` (a local collector) are accepted. Everything else -
/// notably a plain `http://` non-loopback host - is rejected so the operator
/// cannot accidentally ship metadata over an unencrypted link.
///
/// Used by both signal builders (`build_metric_exporter`, `build_log_exporter`),
/// so a logs-only build of the crate still references it.
#[cfg(feature = "telemetry")]
fn ensure_grpc_endpoint_is_encrypted(
    endpoint: &str,
) -> std::result::Result<(), crate::TelemetryError> {
    // SEC-053: parse the URL once and decide on the *parsed* host. A naive
    // string split would accept `http://localhost:4317@evil.example` as
    // loopback (reading only the text before the first `:`), when the real
    // authority is `localhost:4317@evil.example` -> host `evil.example`.
    let safe = mask_endpoint_userinfo(endpoint);
    let uri = url::Url::parse(endpoint.trim()).map_err(|e| {
        crate::TelemetryError::ExporterInit(format!(
            "unsupported OTLP/gRPC endpoint '{safe}': not a valid URL ({e})"
        ))
    })?;

    // Reject any userinfo outright: an embedded credential means the operator
    // intended to authenticate, which is only meaningful over TLS, and it also
    // muddies the loopback decision (`http://user@127.0.0.1:4317`).
    if !uri.username().is_empty() || uri.password().is_some() {
        return Err(crate::TelemetryError::ExporterInit(format!(
            "refusing OTLP/gRPC endpoint '{safe}': userinfo in the endpoint URL \
             is not supported; use an https:// endpoint (TLS is compiled in)"
        )));
    }

    match uri.scheme() {
        "https" => Ok(()),
        "http" => {
            let is_loopback = uri
                .host_str()
                .is_some_and(|host| matches!(host, "localhost" | "127.0.0.1" | "[::1]" | "::1"));
            if is_loopback {
                return Ok(());
            }
            Err(crate::TelemetryError::ExporterInit(format!(
                "refusing to export OTLP/gRPC over cleartext to '{safe}'; use \
                 an https:// endpoint (TLS is compiled in)"
            )))
        }
        _ => Err(crate::TelemetryError::ExporterInit(format!(
            "unsupported OTLP/gRPC endpoint '{safe}': expected an http(s) URL"
        ))),
    }
}

/// Build the OTEL [`Resource`] with standard and custom attributes (FR-004).
///
/// The following static resource attributes are attached to all exported
/// metrics:
///
/// - `service.name` - from `config.service_name` (default `"ragent"`)
/// - `service.version` - from `CARGO_PKG_VERSION` at compile time
/// - `host.name` - best-effort system hostname lookup (FR-004)
///
/// Custom resource attributes from `config.resource_attributes` are merged
/// in (FR-026). The dynamic `session.id` is **not** a resource attribute -
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
pub(crate) fn build_resource(config: &OtelConfig) -> Resource {
    use crate::sensitive::sanitize_attr_value;

    let mut kvs = vec![
        KeyValue::new("service.name", sanitize_attr_value(&config.service_name)),
        // SEC-ragent-telemetry-005 (SECTASKS T-066): every other resource
        // attribute is routed through the guard; route the version too so the
        // contract holds uniformly.
        KeyValue::new(
            "service.version",
            sanitize_attr_value(env!("CARGO_PKG_VERSION")),
        ),
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
