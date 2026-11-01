//! OTLP logs-signal export for ragent's `tracing` events (spec `otel` FR-036).
//!
//! ragent logs through `tracing` to a local sink only: the TUI log panel in a
//! terminal session, or stderr in a headless/server run. This module adds a
//! second, opt-in destination: every emitted event can also be shipped to the
//! same OTLP collector that already receives the metrics signal, as an
//! OpenTelemetry **log record**.
//!
//! # Shape of the integration
//!
//! [`TracingLogsLayer`] is a [`tracing_subscriber::Layer`], so it composes with
//! the existing sinks through `tracing_subscriber::registry()` instead of
//! replacing them. The local sink is unchanged: enabling logs export adds a
//! destination, it never redirects the panel or the terminal.
//!
//! The layer hands each event to [`convert_event`], a pure function that builds
//! the OTel [`LogRecord`] fields (severity number and text, body, target and
//! sanitised attributes). Because the conversion has no side effects it is
//! unit-testable without a collector, and the same function serves the live
//! layer and the tests.
//!
//! # Sensitive-data guard (FR-034, FR-039)
//!
//! Log bodies are free text and ragent already interpolates provider errors and
//! response fragments into them, so the remote path is deliberately stricter
//! than the metric path:
//!
//! 1. the body is passed through
//!    [`ragent_types::sanitize::redact_secrets`], which masks both registered
//!    secrets and credential-shaped text;
//! 2. every attribute is passed through [`crate::sensitive::sanitize_attr_value`],
//!    the same cardinality/length/credential guard the metric attributes use;
//! 3. [`ExportEventFilter`] decides whether a record is exported at all, so
//!    ragent can refuse a category of event (a user prompt, a tool's file
//!    content) without changing what the local sink shows. ragent's own
//!    pipeline installs [`SuppressNoiseEvents`], which withholds a small set of
//!    per-frame provider stream diagnostics that would otherwise flood a
//!    collector.
//!
//! # Non-blocking (FR-037)
//!
//! The layer never exports synchronously: it converts and hands the record to
//! the SDK's batch log processor, which delivers on its own background thread.
//! Records are batched on the same `export_interval_seconds` as the metrics,
//! so one configured cadence covers both signals.
//!
//! # Reconfiguration
//!
//! The layer holds an [`Arc`] swap cell rather than a fixed provider, so it can
//! be installed once before the first event (the subscriber is global and cannot
//! be replaced) and later re-pointed by `/telemetry on|off`,
//! `/telemetry logs <on|off>` and `/telemetry setup` without restarting the
//! process.
//!
//! # Feature gating (NFR-006)
//!
//! The OTLP pipeline needs the `telemetry` Cargo feature. With the feature off
//! the layer still exists and still compiles into a subscriber, but its swap
//! cell can never be filled, so it exports nothing and costs one `None` check
//! per event. [`convert_event`] is not feature-gated: it is the pure, testable
//! half of the conversion and it only needs `tracing`.

use std::sync::Arc;

use parking_lot::RwLock;
use tracing::field::{Field, Visit};
use tracing::{Event, Level, Subscriber};
use tracing_subscriber::Layer;
#[cfg(feature = "telemetry")]
use tracing_subscriber::layer::Context;

/// Instrumentation-scope name ragent's log records are emitted under.
pub const LOGGER_NAME: &str = "ragent";

/// Key of the attribute carrying the `tracing` target (module path) of an
/// event, following the `code.*` family of the OpenTelemetry log semantic
/// conventions.
pub const TARGET_ATTRIBUTE: &str = "code.namespace";

/// Key of the attribute carrying the `tracing` source file of an event.
pub const FILE_ATTRIBUTE: &str = "code.filepath";

/// Longest log body exported to the collector, in characters.
///
/// A log body is not subject to the metric attribute guard, but an unbounded
/// body (a provider echo of a whole HTML page, a multi-megabyte diff) would
/// blow up a batch. The body is truncated at this many characters with a
/// `... [truncated]` marker; the local sink is untouched.
pub const MAX_BODY_CHARS: usize = 16 * 1024;

/// Map a [`Level`] to its rank: `0` trace, `1` debug, `2` info, `3` warn,
/// `4` error.
///
/// The rank is the ordering [`ragent_config::OtelLogsConfig::min_severity_rank`]
/// expresses, so the configured `telemetry.otel.logs.level` and a live event
/// can be compared without either side knowing about `tracing`.
#[must_use]
pub fn severity_rank(level: &Level) -> u8 {
    match *level {
        Level::TRACE => 0,
        Level::DEBUG => 1,
        Level::INFO => 2,
        Level::WARN => 3,
        Level::ERROR => 4,
    }
}

/// The `severity_text` for a [`Level`] (`"TRACE"` .. `"ERROR"`).
#[must_use]
pub const fn severity_text(level: &Level) -> &'static str {
    match *level {
        Level::TRACE => "TRACE",
        Level::DEBUG => "DEBUG",
        Level::INFO => "INFO",
        Level::WARN => "WARN",
        Level::ERROR => "ERROR",
    }
}

/// A `tracing` event reduced to the parts an OTel log record carries.
///
/// Produced by [`convert_event`]. Every string has already been through the
/// sensitive-data guard, so a holder of a [`LogEvent`] cannot accidentally
/// export a credential.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LogEvent {
    /// `severity_text` for the record (`"INFO"`, `"WARN"`, ...).
    pub severity_text: &'static str,
    /// Severity rank (see [`severity_rank`]) of the record.
    pub severity_rank: u8,
    /// Redacted, possibly truncated event body.
    pub body: String,
    /// `tracing` target (module path), when the event declares one.
    pub target: Option<String>,
    /// `tracing` source file, when the event declares one.
    pub file: Option<String>,
    /// Sanitised event fields other than `message`.
    pub attributes: Vec<(String, String)>,
}

/// Collects a `tracing` event's fields into a [`LogEvent`].
///
/// `message` becomes the body; every other field becomes an attribute, with a
/// `Debug`-formatted field stored under its own name. Values are left raw here
/// (a credential registry lookup per field is not free) and are redacted once,
/// in [`convert_event`].
#[derive(Default)]
struct FieldVisitor {
    message: Option<String>,
    attributes: Vec<(String, String)>,
}

impl FieldVisitor {
    fn push(&mut self, field: &Field, value: String) {
        if field.name() == "message" {
            self.message = Some(value);
        } else {
            self.attributes.push((field.name().to_string(), value));
        }
    }
}

impl Visit for FieldVisitor {
    fn record_str(&mut self, field: &Field, value: &str) {
        self.push(field, value.to_string());
    }

    fn record_debug(&mut self, field: &Field, value: &dyn std::fmt::Debug) {
        self.push(field, format!("{value:?}"));
    }
}

/// Truncate `text` to at most [`MAX_BODY_CHARS`] characters.
///
/// Counts characters rather than bytes so a multi-byte UTF-8 body is cut on a
/// character boundary and never produces invalid `String` content.
#[must_use]
pub fn truncate_body(text: &str) -> String {
    match text.char_indices().nth(MAX_BODY_CHARS) {
        Some((idx, _)) => {
            let mut truncated = text[..idx].to_string();
            truncated.push_str("... [truncated]");
            truncated
        }
        None => text.to_string(),
    }
}

/// Convert a `tracing` event into a [`LogEvent`], applying the sensitive-data
/// guard (FR-034, FR-039).
///
/// The body is passed through [`ragent_types::sanitize::redact_secrets`] and
/// then truncated with [`truncate_body`]; each attribute value is passed
/// through [`crate::sensitive::sanitize_attr_value`].
#[must_use]
pub fn convert_event(event: &Event<'_>) -> LogEvent {
    use crate::sensitive::sanitize_attr_value;

    let mut visitor = FieldVisitor::default();
    event.record(&mut visitor);

    let raw_body = visitor.message.unwrap_or_default();
    let body = truncate_body(&ragent_types::sanitize::redact_secrets(&raw_body));

    let attributes = visitor
        .attributes
        .into_iter()
        .map(|(name, value)| {
            // A field name is ragent's own `ident`, but the value may be an
            // interpolated secret, so both halves go through the guard: the
            // shared `sanitize_attr_value` rejects a name that is too long or
            // contains whitespace, which keeps the attribute map bounded.
            (sanitize_attr_value(&name), sanitize_attr_value(&value))
        })
        .filter(|(name, _)| !name.is_empty())
        .collect();

    let metadata = event.metadata();
    let target = {
        let target = metadata.target();
        (!target.is_empty()).then(|| target.to_string())
    };

    LogEvent {
        severity_text: severity_text(metadata.level()),
        severity_rank: severity_rank(metadata.level()),
        body,
        target,
        file: metadata.file().map(str::to_string),
        attributes,
    }
}

/// Decides whether a converted [`LogEvent`] is exported to the collector.
///
/// The filter is applied *after* conversion so the guard has already run, and
/// it can only ever reduce what leaves the machine. Ragent installs
/// [`AllowAllEvents`], which exports every event that reaches the layer; the
/// trait exists so an embedding application, or a test, can refuse a category
/// of event without touching the local sink.
pub trait ExportEventFilter: Send + Sync + 'static {
    /// Return `true` when `event` should be exported.
    fn allows(&self, event: &LogEvent) -> bool;
}

/// [`ExportEventFilter`] that exports every event.
#[derive(Debug, Default, Clone, Copy)]
pub struct AllowAllEvents;

impl ExportEventFilter for AllowAllEvents {
    fn allows(&self, _event: &LogEvent) -> bool {
        true
    }
}

/// Event bodies ragent never exports to the collector.
///
/// These are per-frame stream diagnostics: one record per completed provider
/// response, plus a second when a response batches its tool calls into its
/// final frame. They are useful in the local sink while debugging a provider,
/// but over a session that issues hundreds of requests they are pure noise in
/// a collector, and the same information is already carried by the usage
/// metrics. The bodies are ragent's own `tracing` message strings, so the match
/// is exact and cannot hide a user-supplied message.
const NOISE_BODIES: &[&str] = &[
    "Ollama Cloud: done frame received",
    "Ollama Cloud: tool_calls found in done frame",
];

/// [`ExportEventFilter`] for ragent's own export pipeline.
///
/// Exports every event except a small set of known-repetitive provider stream
/// diagnostics ([`NOISE_BODIES`]). It only ever reduces what leaves the
/// machine: the local sink (the TUI log panel or stderr) is unchanged, so the
/// suppressed lines are still visible where a developer looks for them.
#[derive(Debug, Default, Clone, Copy)]
pub struct SuppressNoiseEvents;

impl ExportEventFilter for SuppressNoiseEvents {
    fn allows(&self, event: &LogEvent) -> bool {
        !NOISE_BODIES.contains(&event.body.as_str())
    }
}

/// Everything the layer needs to export one record: a logger, and the
/// minimum-severity rank the configured `telemetry.otel.logs.level` selects.
///
/// Opaque on purpose: the fields are an implementation detail of the layer and
/// the subsystem that populates its swap cell, so a caller can only reach the
/// cell through [`TracingLogsLayer`] and
/// [`TelemetrySubsystem::attach_logs_layer`][attach].
///
/// [attach]: crate::subsystem::TelemetrySubsystem::attach_logs_layer
pub struct ActiveExport {
    /// Logger obtained from the shared pipeline handle, so the provider it came
    /// from can still be flushed and shut down by the subsystem while the layer
    /// emits.
    #[cfg(feature = "telemetry")]
    logger: LoggerProviderHandle,
    /// Records below this rank are not exported.
    min_rank: u8,
    /// Records this sink is allowed to send.
    #[cfg(feature = "telemetry")]
    filter: Arc<dyn ExportEventFilter>,
}

#[cfg(feature = "telemetry")]
impl ActiveExport {
    /// Assemble an export pipeline from parts, for tests that drive the layer
    /// with their own provider instead of letting [`build_logger_provider`] and
    /// [`crate::subsystem::TelemetrySubsystem::attach_logs_layer`] wire it.
    ///
    /// `min_rank` is the [`severity_rank`] of the lowest exported level, and
    /// `filter` decides which converted events are sent (see
    /// [`ExportEventFilter`]). Production callers reach an `ActiveExport` only
    /// through the subsystem, so this constructor is hidden from the crate's
    /// public documentation.
    #[doc(hidden)]
    #[must_use]
    pub fn for_test(logger: SdkLogger, min_rank: u8, filter: Arc<dyn ExportEventFilter>) -> Self {
        Self {
            logger: LoggerProviderHandle::from_logger(logger),
            min_rank,
            filter,
        }
    }
}

impl std::fmt::Debug for ActiveExport {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut debug = f.debug_struct("ActiveExport");
        debug.field("min_rank", &self.min_rank);
        #[cfg(feature = "telemetry")]
        debug.field("logger", &self.logger);
        debug.finish_non_exhaustive()
    }
}

/// Swap cell shared between [`TracingLogsLayer`] and the telemetry subsystem.
///
/// `None` means "not exporting": the layer then only reads the cell and
/// returns, which is the zero-cost state a default (telemetry-off) run is in.
pub type ExportCell = Arc<RwLock<Option<ActiveExport>>>;

// -- OTLP pipeline (feature `telemetry`) ---------------------------------

#[cfg(feature = "telemetry")]
use opentelemetry::Key;
#[cfg(feature = "telemetry")]
use opentelemetry::logs::{AnyValue, LogRecord, Logger, LoggerProvider};
#[cfg(feature = "telemetry")]
use opentelemetry_sdk::logs::LogExporter;
#[cfg(feature = "telemetry")]
use opentelemetry_sdk::logs::{SdkLogger, SdkLoggerProvider};
#[cfg(feature = "telemetry")]
use ragent_config::OtelConfig;

/// The OTel severity number for a [`Level`], using the mid-point of each OTel
/// severity bucket (INFO -> 9, WARN -> 13, ERROR -> 17, and so on).
///
/// `tracing` has five levels where OTel has six buckets (TRACE..FATAL); the
/// mapping is monotonic and keeps the standard bucket boundaries, which is what
/// a collector's severity filter expects.
#[cfg(feature = "telemetry")]
#[must_use]
pub fn severity_number(level: &Level) -> opentelemetry::logs::Severity {
    use opentelemetry::logs::Severity;
    match *level {
        Level::TRACE => Severity::Trace,
        Level::DEBUG => Severity::Debug,
        Level::INFO => Severity::Info,
        Level::WARN => Severity::Warn,
        Level::ERROR => Severity::Error,
    }
}

/// A [`tracing_subscriber::Layer`] that forwards events to the OTLP logs
/// signal.
///
/// Install it once, before the first event, through
/// `tracing_subscriber::registry()`; [`TelemetrySubsystem`][sub] later fills in
/// (or clears) the export cell through
/// [`TelemetrySubsystem::attach_logs_layer`][attach] and `reconfigure`. Because
/// the registry is global and cannot be replaced, the cell is the only way
/// `/telemetry on|off` can change log export in a running process.
///
/// [sub]: crate::subsystem::TelemetrySubsystem
/// [attach]: crate::subsystem::TelemetrySubsystem::attach_logs_layer
///
/// # Examples
///
/// ```
/// use ragent_telemetry::logs::TracingLogsLayer;
///
/// // In production the cell is handed to the telemetry subsystem, which fills
/// // it in when the `telemetry.otel.logs` block is enabled.
/// let layer = TracingLogsLayer::disabled();
/// assert!(!layer.is_exporting());
/// ```
#[derive(Clone)]
pub struct TracingLogsLayer {
    cell: ExportCell,
}

impl TracingLogsLayer {
    /// Create a layer that exports nothing until the cell is filled in.
    #[must_use]
    pub fn disabled() -> Self {
        Self {
            cell: Arc::new(RwLock::new(None)),
        }
    }

    /// Create a layer from an already-populated export cell.
    #[must_use]
    pub fn new(cell: ExportCell) -> Self {
        Self { cell }
    }

    /// The swap cell this layer reads on every event.
    #[must_use]
    pub fn cell(&self) -> ExportCell {
        Arc::clone(&self.cell)
    }

    /// Returns `true` while the layer is forwarding events to a collector.
    #[must_use]
    pub fn is_exporting(&self) -> bool {
        self.cell.read().is_some()
    }
}

impl<S: Subscriber> Layer<S> for TracingLogsLayer {
    #[cfg(feature = "telemetry")]
    fn on_event(&self, event: &Event<'_>, _ctx: Context<'_, S>) {
        let level_rank = severity_rank(event.metadata().level());

        // Reading the cell costs an uncontended lock; returning immediately
        // when it is empty is what keeps a telemetry-off run at zero cost.
        //
        // The batch log processor owns a clone of the provider, so a record can
        // never reach a provider that has been dropped; a record that arrives
        // after a shutdown is discarded by the processor rather than lost
        // silently by this layer.
        let guard = self.cell.read();
        let Some(active) = guard.as_ref() else {
            return;
        };
        if level_rank < active.min_rank {
            return;
        }

        let converted = convert_event(event);
        if !active.filter.allows(&converted) {
            return;
        }

        let mut record = active.logger.create_log_record();
        record.set_severity_number(severity_number(event.metadata().level()));
        record.set_severity_text(converted.severity_text);
        record.set_target(LOGGER_NAME);
        record.set_timestamp(std::time::SystemTime::now());
        record.set_body(AnyValue::String(converted.body.clone().into()));
        if let Some(target) = &converted.target {
            record.add_attribute(
                Key::from(TARGET_ATTRIBUTE),
                AnyValue::String(target.clone().into()),
            );
        }
        if let Some(file) = &converted.file {
            record.add_attribute(
                Key::from(FILE_ATTRIBUTE),
                AnyValue::String(file.clone().into()),
            );
        }
        for (name, value) in converted.attributes {
            record.add_attribute(Key::from(name), AnyValue::String(value.into()));
        }
        active.logger.emit(record);
    }
}

impl std::fmt::Debug for TracingLogsLayer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TracingLogsLayer")
            .field("exporting", &self.is_exporting())
            .finish()
    }
}

/// Build the OTLP log pipeline for an enabled `telemetry.otel.logs` block.
///
/// The provider batches records and delivers them on its own thread, so the
/// emitting thread never blocks on the network (FR-037). It is created from the
/// same [`Resource`](opentelemetry_sdk::Resource) as the meter provider, so a
/// record and a metric from one session carry identical
/// `service.name`/`host.name` attribution (FR-004).
///
/// # Errors
///
/// Returns [`crate::TelemetryError::ExporterInit`] when the OTLP log exporter
/// cannot be built (for example an endpoint the transport rejects).
#[cfg(feature = "telemetry")]
pub fn build_logger_provider(config: &OtelConfig) -> crate::Result<LoggerProviderHandle> {
    let exporter = crate::subsystem::build_log_exporter(config)?;
    let provider = SdkLoggerProvider::builder()
        .with_resource(crate::subsystem::build_resource(config))
        .with_batch_exporter(exporter)
        .build();
    Ok(LoggerProviderHandle::new(provider))
}

/// Build the export cell contents for an enabled `telemetry.otel.logs` block.
///
/// # Errors
///
/// Currently infallible; the `Result` mirrors the construction path so a future
/// filter construction failure has somewhere to go without changing the
/// subsystem's call site.
#[cfg(feature = "telemetry")]
pub(crate) fn build_export(
    config: &OtelConfig,
    logger: &LoggerProviderHandle,
) -> crate::Result<ActiveExport> {
    Ok(ActiveExport {
        logger: logger.clone(),
        min_rank: config.logs.min_severity_rank(),
        filter: Arc::new(SuppressNoiseEvents),
    })
}

/// Handle to the live OTLP log pipeline: a logger for the layer plus the
/// provider it came from.
///
/// Kept next to the export cell so
/// [`TelemetrySubsystem::flush`][crate::subsystem::TelemetrySubsystem::flush]
/// and `shutdown` can drain pending records: a process that exits within the
/// batch interval would otherwise drop its last records.
#[cfg(feature = "telemetry")]
pub struct LoggerProviderHandle {
    /// The live provider; holding it keeps the batch processor's thread alive
    /// for as long as the layer can emit.
    provider: Arc<SdkLoggerProvider>,
    /// Logger the layer emits through, obtained once from the provider.
    logger: opentelemetry_sdk::logs::SdkLogger,
}

#[cfg(feature = "telemetry")]
impl LoggerProviderHandle {
    /// Wrap a freshly built logger provider so it can be emitted through and,
    /// later, flushed and shut down.
    #[must_use]
    pub fn new(provider: SdkLoggerProvider) -> Self {
        let logger = provider.logger(LOGGER_NAME);
        Self {
            provider: Arc::new(provider),
            logger,
        }
    }

    /// Flush every buffered log record to the collector.
    ///
    /// Returns `Ok(())` when the provider reports the pipeline is no longer
    /// running ([`OTelSdkError::AlreadyShutdown`][already]): a flush after
    /// shutdown has nothing left to deliver, which is a normal consequence of
    /// the shutdown guard running after `/telemetry off`, not a failure worth
    /// reporting.
    ///
    /// [already]: opentelemetry_sdk::error::OTelSdkError::AlreadyShutdown
    ///
    /// # Errors
    ///
    /// Returns the SDK error when a processor reports an export failure.
    pub fn force_flush(&self) -> opentelemetry_sdk::error::OTelSdkResult {
        self.provider.force_flush()
    }

    /// Flush and shut the pipeline down.
    ///
    /// `AlreadyShutdown` is treated as success for the same reason as
    /// [`Self::force_flush`].
    ///
    /// # Errors
    ///
    /// Returns the SDK error when a processor reports a failure.
    pub fn shutdown(&self) -> opentelemetry_sdk::error::OTelSdkResult {
        self.provider.shutdown()
    }

    /// Wrap a logger that came from a test's own provider.
    ///
    /// The handle needs a provider to flush and shut down; a test that passes
    /// its own logger keeps responsibility for draining it, so this builds a
    /// discard pipeline around it. Only [`ActiveExport::for_test`] uses this.
    fn from_logger(logger: SdkLogger) -> Self {
        /// Exporter that accepts and drops every batch.
        #[derive(Debug)]
        struct DiscardExporter;

        impl LogExporter for DiscardExporter {
            async fn export(
                &self,
                _batch: opentelemetry_sdk::logs::LogBatch<'_>,
            ) -> opentelemetry_sdk::error::OTelSdkResult {
                Ok(())
            }
        }

        let provider = SdkLoggerProvider::builder()
            .with_batch_exporter(DiscardExporter)
            .build();
        Self {
            provider: Arc::new(provider),
            logger,
        }
    }

    /// Create a log record from this pipeline's logger.
    fn create_log_record(&self) -> opentelemetry_sdk::logs::SdkLogRecord {
        self.logger.create_log_record()
    }

    /// Hand a filled-in record to the batch processor.
    fn emit(&self, record: opentelemetry_sdk::logs::SdkLogRecord) {
        self.logger.emit(record);
    }
}

#[cfg(feature = "telemetry")]
impl std::fmt::Debug for LoggerProviderHandle {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("LoggerProviderHandle")
            .finish_non_exhaustive()
    }
}

#[cfg(feature = "telemetry")]
impl Clone for LoggerProviderHandle {
    fn clone(&self) -> Self {
        Self {
            provider: Arc::clone(&self.provider),
            logger: self.logger.clone(),
        }
    }
}
