//! Integration tests for the OTLP logs signal (spec `otel` FR-036..FR-041).
//!
//! These tests cover the three halves of the feature:
//!
//! 1. the pure conversion from a `tracing` event to the record fields
//!    ([`convert_event`], [`truncate_body`], [`severity_rank`]), which is also
//!    where the sensitive-data guard is asserted;
//! 2. the [`TracingLogsLayer`] driving a real `SdkLoggerProvider` whose
//!    exporter is an `InMemoryLogExporter`, so a record that reaches the
//!    collector is observed end to end rather than inferred;
//! 3. the `/v1/logs` endpoint resolution, which must share the metrics
//!    resolver's rules so one configured base URL addresses both signals.
//!
//! No test needs a live collector, a network socket, or a global subscriber:
//! every case builds its own provider and its own layer.

use std::sync::Arc;

use opentelemetry::logs::{AnyValue, LoggerProvider};
use opentelemetry_sdk::logs::{InMemoryLogExporter, SdkLogRecord, SdkLoggerProvider};
use ragent_config::{OtelConfig, OtelLogsConfig};
use ragent_telemetry::logs::{
    ActiveExport, AllowAllEvents, ExportCell, ExportEventFilter, LogEvent, MAX_BODY_CHARS,
    SuppressNoiseEvents, TracingLogsLayer, convert_event, severity_rank, severity_text,
    truncate_body,
};
use tracing::Level;
use tracing::{Event, Subscriber};
use tracing_subscriber::Layer;
use tracing_subscriber::layer::Context;
use tracing_subscriber::layer::SubscriberExt as _;

// -- Helpers ---------------------------------------------------------------

/// Build a log pipeline whose exporter keeps the exported records in memory.
///
/// The provider is created without an explicit `Resource` so the exporter's
/// own resource is attached; the tests assert on records, not attribution.
fn in_memory_provider() -> (SdkLoggerProvider, InMemoryLogExporter) {
    let exporter = InMemoryLogExporter::default();
    let provider = SdkLoggerProvider::builder()
        .with_batch_exporter(exporter.clone())
        .build();
    (provider, exporter)
}

/// Fill an export cell so the layer forwards to `provider`.
fn exporting_cell(
    provider: &SdkLoggerProvider,
    min_level: Level,
    filter: Arc<dyn ExportEventFilter>,
) -> ExportCell {
    let logger = provider.logger(ragent_telemetry::logs::LOGGER_NAME);
    Arc::new(parking_lot::RwLock::new(Some(ActiveExport::for_test(
        logger,
        severity_rank(&min_level),
        filter,
    ))))
}

/// Run `body` with a layer installed for a scoped (thread-local) subscriber.
///
/// A scoped subscriber is used instead of the global one so tests can run in
/// parallel: `with_default` installs it for the calling thread only.
fn with_layer<F: FnOnce()>(cell: ExportCell, body: F) {
    let layer = TracingLogsLayer::new(cell);
    let subscriber = tracing_subscriber::registry().with(layer);
    tracing::subscriber::with_default(subscriber, body);
}

/// Flush the provider and return the records that reached the exporter.
fn drained(exporter: &InMemoryLogExporter, provider: &SdkLoggerProvider) -> Vec<SdkLogRecord> {
    provider.force_flush().expect("flush should succeed");
    exporter
        .get_emitted_logs()
        .expect("in-memory export should be readable")
        .into_iter()
        .map(|log| log.record)
        .collect()
}

/// The body of a record as a string, or `None` for a non-string body.
fn body_of(record: &SdkLogRecord) -> Option<String> {
    match record.body() {
        Some(AnyValue::String(value)) => Some(value.to_string()),
        _ => None,
    }
}

/// The string value of a record attribute.
fn attribute_of(record: &SdkLogRecord, key: &str) -> Option<String> {
    record
        .attributes_iter()
        .find(|(name, _)| name.as_str() == key)
        .and_then(|(_, value)| match value {
            AnyValue::String(text) => Some(text.to_string()),
            _ => None,
        })
}

// -- Conversion ------------------------------------------------------------

/// FR-036: an event converts to a record carrying its severity, body, target
/// and source file.
#[test]
fn test_convert_event_captures_severity_body_and_target() {
    let seen = capture_one(Level::WARN, "provider call failed");

    assert_eq!(seen.severity_text, "WARN");
    assert_eq!(seen.severity_rank, severity_rank(&Level::WARN));
    assert_eq!(seen.body, "provider call failed");
    assert_eq!(seen.target.as_deref(), Some(TARGET));
    assert!(
        seen.file.as_deref().is_some_and(|f| std::path::Path::new(f)
            .extension()
            .is_some_and(|ext| ext == "rs")),
        "the source file must be recorded, got {:?}",
        seen.file
    );
}

/// Every `tracing` level maps onto a monotonic rank, and the text matches the
/// rank so a collector sees a consistent severity.
#[test]
fn test_severity_ranks_are_monotonic() {
    let levels = [
        Level::TRACE,
        Level::DEBUG,
        Level::INFO,
        Level::WARN,
        Level::ERROR,
    ];
    let ranks: Vec<u8> = levels.iter().map(severity_rank).collect();
    assert_eq!(ranks, vec![0, 1, 2, 3, 4], "ranks must be ordered");
    for level in levels {
        assert_eq!(
            severity_text(&level),
            level.as_str(),
            "severity text must match the tracing level name"
        );
    }
}

/// FR-039: a credential embedded in the body never reaches the record.
#[test]
fn test_convert_event_redacts_credentials_in_body() {
    let seen = capture_one(
        Level::ERROR,
        "request failed with Authorization: Bearer sk-proj-abcdefghijklmnop1234",
    );

    assert!(
        !seen.body.contains("sk-proj-abcdefghijklmnop1234"),
        "an API key must not survive into an exported body: {}",
        seen.body
    );
    assert!(seen.body.contains("[REDACTED]"), "got {}", seen.body);
}

/// FR-034: an attribute whose value looks like a secret is replaced.
#[test]
fn test_convert_event_sanitises_attributes() {
    let seen = capture_one_with_fields(Level::INFO, "attaching", ("key", "sk-live-1234567890"));

    let value = seen
        .attributes
        .iter()
        .find(|(name, _)| name == "key")
        .map(|(_, value)| value.clone())
        .expect("the key attribute should be present");
    assert_eq!(
        value,
        ragent_types::sanitize::REDACTION_PLACEHOLDER,
        "a credential-shaped attribute must be redacted"
    );
}

/// A normal attribute is preserved verbatim so a collector can filter on it.
#[test]
fn test_convert_event_keeps_ordinary_attributes() {
    let seen = capture_one_with_fields(Level::DEBUG, "cache hit", ("key", "gpt-4o"));

    assert_eq!(
        seen.attributes,
        vec![("key".to_string(), "gpt-4o".to_string())]
    );
}

/// FR-039: an oversized body is truncated rather than batched whole.
#[test]
fn test_truncate_body_caps_length_on_a_char_boundary() {
    // Multi-byte characters at the cut point must not panic on slicing.
    let text = "é".repeat(MAX_BODY_CHARS + 10);
    let truncated = truncate_body(&text);

    assert!(truncated.ends_with("... [truncated]"));
    assert_eq!(
        truncated.chars().filter(|c| *c == 'é').count(),
        MAX_BODY_CHARS
    );
}

/// A body inside the cap is returned unchanged.
#[test]
fn test_truncate_body_keeps_short_text() {
    assert_eq!(truncate_body("short message"), "short message");
}

// -- Layer ----------------------------------------------------------------

/// FR-036/FR-037: an enabled layer hands the event to the provider, which
/// delivers it to the exporter on flush.
#[test]
fn test_layer_exports_event_to_the_collector() {
    let (provider, exporter) = in_memory_provider();
    let cell = exporting_cell(&provider, Level::TRACE, Arc::new(AllowAllEvents));

    with_layer(cell, || {
        tracing::info!(target: concat!(module_path!(), "::live"), model = "gpt-4o", "hello collector");
    });

    let records = drained(&exporter, &provider);
    let record = records.first().expect("the event must reach the exporter");

    assert_eq!(record.severity_text(), Some("INFO"));
    assert_eq!(
        record.severity_number(),
        Some(opentelemetry::logs::Severity::Info)
    );
    assert_eq!(body_of(record).as_deref(), Some("hello collector"));
    assert_eq!(record.target(), Some(&"ragent".into()));
    assert_eq!(
        attribute_of(record, ragent_telemetry::logs::TARGET_ATTRIBUTE).as_deref(),
        Some(concat!(module_path!(), "::live")),
        "the tracing target must travel as code.namespace"
    );
    assert_eq!(attribute_of(record, "model").as_deref(), Some("gpt-4o"));
}

/// FR-040: an empty cell exports nothing, which is the default state of every
/// run that has not enabled the signal.
#[test]
fn test_layer_with_empty_cell_exports_nothing() {
    let (provider, exporter) = in_memory_provider();
    let cell: ExportCell = Arc::new(parking_lot::RwLock::new(None));

    with_layer(cell, || {
        tracing::error!("this must not leave the process");
    });

    let records = drained(&exporter, &provider);
    assert!(
        records.is_empty(),
        "a layer with no pipeline must not export: {records:?}"
    );
}

/// The configured `telemetry.otel.logs.level` filters the remote export while
/// the local sink keeps everything: a `warn` level must not ship `info`.
#[test]
fn test_layer_filters_below_configured_level() {
    let (provider, exporter) = in_memory_provider();
    let cell = exporting_cell(&provider, Level::WARN, Arc::new(AllowAllEvents));

    with_layer(cell, || {
        tracing::info!("below the configured level");
        tracing::error!("at or above the configured level");
    });

    let records = drained(&exporter, &provider);
    assert_eq!(records.len(), 1, "only the error should be exported");
    assert_eq!(
        body_of(&records[0]).as_deref(),
        Some("at or above the configured level")
    );
}

/// A filter can refuse a record after conversion, which is how an embedding
/// application suppresses a category of event without touching the local sink.
#[test]
fn test_layer_honours_export_filter() {
    /// Filter that refuses every record.
    #[derive(Debug)]
    struct RefuseAll;

    impl ExportEventFilter for RefuseAll {
        fn allows(&self, _event: &LogEvent) -> bool {
            false
        }
    }

    let (provider, exporter) = in_memory_provider();
    let cell = exporting_cell(&provider, Level::TRACE, Arc::new(RefuseAll));

    with_layer(cell, || {
        tracing::warn!("refused by the filter");
    });

    assert!(
        drained(&exporter, &provider).is_empty(),
        "the filter must stop the record before it is emitted"
    );
}

/// ragent's production filter suppresses the per-response provider stream
/// diagnostics while leaving every other record exported.
#[test]
fn test_suppress_noise_filter_drops_stream_diagnostics() {
    let (provider, exporter) = in_memory_provider();
    let cell = exporting_cell(&provider, Level::TRACE, Arc::new(SuppressNoiseEvents));

    with_layer(cell, || {
        tracing::info!(model = "m", "Ollama Cloud: done frame received");
        tracing::info!(
            model = "m",
            count = 2,
            "Ollama Cloud: tool_calls found in done frame"
        );
        tracing::info!("Ollama Cloud provider connected");
        tracing::warn!("Ollama Cloud: stream stalled - no data received for 30s");
    });

    let records = drained(&exporter, &provider);
    let bodies: Vec<String> = records.iter().filter_map(body_of).collect();

    assert_eq!(
        bodies,
        vec![
            "Ollama Cloud provider connected".to_string(),
            "Ollama Cloud: stream stalled - no data received for 30s".to_string(),
        ],
        "only the per-frame stream diagnostics are withheld"
    );
}

// -- Configuration --------------------------------------------------------

/// FR-040: the logs block is off by default.
#[test]
fn test_logs_default_is_disabled() {
    let config = OtelConfig::default();
    assert!(!config.logs.enabled);
    assert_eq!(config.logs.level, "info");
    assert_eq!(config.logs.min_severity_rank(), 2);
}

/// The configured level string is accepted in any case and maps onto the rank
/// ordering, with an unknown value falling back to `info`.
#[test]
fn test_logs_level_rank_mapping() {
    let rank = |level: &str| {
        OtelLogsConfig {
            enabled: true,
            level: level.to_string(),
        }
        .min_severity_rank()
    };

    assert_eq!(rank("all"), 0, "`all` admits every level the layer sees");
    assert_eq!(rank("ALL"), 0);
    assert_eq!(rank("trace"), 0);
    assert_eq!(rank("DEBUG"), 1);
    assert_eq!(rank("info"), 2);
    assert_eq!(rank("warn"), 3);
    assert_eq!(rank("error"), 4);
    assert_eq!(rank("nonsense"), 2, "an unknown level falls back to info");
}

/// `all` is the most permissive setting: no severity the layer can observe is
/// filtered out, and it validates on an enabled config.
#[test]
fn test_logs_level_all_exports_every_tracing_level() {
    let config = OtelConfig {
        enabled: true,
        logs: OtelLogsConfig {
            enabled: true,
            level: "all".to_string(),
        },
        ..OtelConfig::default()
    };

    assert!(config.validate().is_empty(), "`all` must validate");

    let min_rank = config.logs.min_severity_rank();
    for level in [
        tracing::Level::TRACE,
        tracing::Level::DEBUG,
        tracing::Level::INFO,
        tracing::Level::WARN,
        tracing::Level::ERROR,
    ] {
        assert!(
            ragent_telemetry::logs::severity_rank(&level) >= min_rank,
            "`all` must not filter {level} out"
        );
    }
}

/// A malformed level is a validation problem on an enabled config.
#[test]
fn test_validate_rejects_unknown_logs_level() {
    let config = OtelConfig {
        enabled: true,
        logs: OtelLogsConfig {
            enabled: true,
            level: "loud".to_string(),
        },
        ..OtelConfig::default()
    };

    let problems = config.validate();
    assert!(
        problems.iter().any(|p| p.contains("logs.level")),
        "expected a logs.level problem, got {problems:?}"
    );
}

/// The logs block round-trips through JSON, so `/telemetry setup` persistence
/// cannot lose it.
#[test]
fn test_logs_config_serde_roundtrip() {
    let json = r#"{"enabled":true,"logs":{"enabled":true,"level":"warn"}}"#;
    let parsed: OtelConfig = serde_json::from_str(json).expect("parse");
    assert!(parsed.logs.enabled);
    assert_eq!(parsed.logs.level, "warn");
}

// -- Endpoint resolution ---------------------------------------------------

/// The logs signal posts to `/v1/logs` at the same base URL as metrics.
#[test]
fn test_logs_endpoint_gains_signal_path() {
    assert_eq!(
        ragent_telemetry::subsystem::otlp_http_signal_endpoint("http://localhost:4318", "logs"),
        "http://localhost:4318/v1/logs"
    );
    assert_eq!(
        ragent_telemetry::subsystem::otlp_http_signal_endpoint(
            "https://gw.example.com/otel/",
            "logs"
        ),
        "https://gw.example.com/otel/v1/logs"
    );
}

/// An endpoint that already names a signal path is left alone, so a
/// deployment that pins `.../v1/metrics` is not silently redirected.
#[test]
fn test_signal_endpoint_leaves_explicit_signal_url_unchanged() {
    for endpoint in [
        "https://gw.example.com/v1/metrics",
        "https://gw.example.com/v1/logs",
    ] {
        assert_eq!(
            ragent_telemetry::subsystem::otlp_http_signal_endpoint(endpoint, "logs"),
            endpoint
        );
    }
}

// -- Fixtures -------------------------------------------------------------

/// `tracing` target the capture fixtures emit under.
const TARGET: &str = "ragent_logs_test::capture";

/// Emit one event and return the [`LogEvent`] its `message` converts to.
///
/// A `tracing::Event` borrows its dispatcher and cannot outlive the emit call,
/// so [`convert_event`] is invoked from inside a layer while the event is being
/// recorded - exactly as [`TracingLogsLayer`] does - and the converted value is
/// handed back out through a channel.
fn capture_one(level: Level, message: &'static str) -> LogEvent {
    capture(level, message, None)
}

/// Emit one event carrying a single extra field and return its [`LogEvent`].
fn capture_one_with_fields(
    level: Level,
    message: &'static str,
    field: (&'static str, &'static str),
) -> LogEvent {
    capture(level, message, Some(field))
}

/// Layer that converts every event it sees and forwards the result.
struct CaptureLayer {
    tx: std::sync::mpsc::Sender<LogEvent>,
}

impl<S: Subscriber> Layer<S> for CaptureLayer {
    fn on_event(&self, event: &Event<'_>, _ctx: Context<'_, S>) {
        let _ = self.tx.send(convert_event(event)); // INTENTIONAL: a closed receiver means the test finished
    }
}

/// Shared emit-and-capture driver.
fn capture(
    level: Level,
    message: &'static str,
    field: Option<(&'static str, &'static str)>,
) -> LogEvent {
    let (tx, rx) = std::sync::mpsc::channel();
    let subscriber = tracing_subscriber::registry().with(CaptureLayer { tx });

    tracing::subscriber::with_default(subscriber, || match (level, field) {
        (Level::TRACE, None) => tracing::trace!(target: TARGET, "{message}"),
        (Level::DEBUG, None) => tracing::debug!(target: TARGET, "{message}"),
        (Level::INFO, None) => tracing::info!(target: TARGET, "{message}"),
        (Level::WARN, None) => tracing::warn!(target: TARGET, "{message}"),
        (Level::ERROR, None) => tracing::error!(target: TARGET, "{message}"),
        (Level::TRACE, Some(("key", value))) => {
            tracing::trace!(target: TARGET, key = value, "{message}")
        }
        (Level::DEBUG, Some(("key", value))) => {
            tracing::debug!(target: TARGET, key = value, "{message}")
        }
        (Level::INFO, Some(("key", value))) => {
            tracing::info!(target: TARGET, key = value, "{message}")
        }
        (Level::WARN, Some(("key", value))) => {
            tracing::warn!(target: TARGET, key = value, "{message}")
        }
        (Level::ERROR, Some(("key", value))) => {
            tracing::error!(target: TARGET, key = value, "{message}")
        }
        (_, Some((name, _))) => panic!("unsupported capture field {name}"),
    });

    rx.try_recv()
        .expect("the capture layer must have converted the event")
}
