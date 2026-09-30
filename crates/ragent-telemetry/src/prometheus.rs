//! Optional Prometheus text endpoint for local scraping (T-026, FR-028).
//!
//! FR-028: "The system may support an in-process metrics endpoint
//! (`telemetry.otel.internal_port`) that exposes metrics in Prometheus text
//! format for local scraping without an OTLP collector."
//!
//! This module provides:
//!
//! - [`SharedManualReader`] - a newtype wrapper around `Arc<ManualReader>`
//!   that implements the OTEL `MetricReader` trait, so the same reader
//!   instance can be registered on a `SdkMeterProvider` (which takes
//!   ownership) and held by the Prometheus HTTP server (which needs to call
//!   `collect` on demand).
//! - [`render_prometheus_text`] - a pure function that collects a metric
//!   snapshot and renders it as Prometheus text-format exposition.
//! - [`serve`] - an async HTTP server that binds `127.0.0.1:<port>` and
//!   serves the rendered text at `GET /metrics`.
//!
//! # Architecture
//!
//! The Prometheus endpoint is **independent** of the OTLP export path. It
//! uses a [`SharedManualReader`] registered alongside the `PeriodicReader`
//! on the same `SdkMeterProvider`, so both paths see the same metrics.
//! Recording is unaffected - the OTLP exporter batches on a timer, while
//! the Prometheus endpoint collects on-demand when a scraper hits
//! `/metrics`.
//!
//! # Non-blocking guarantee (FR-031, FR-033)
//!
//! The HTTP server runs on a background tokio task. The renderer never
//! panics: a `collect` error (e.g. provider shut down) produces an empty
//! body and a 503 status rather than crashing the task.
//!
//! # Sensitive-data guard (FR-034)
//!
//! Attribute values are already sanitised at the `attr_*` helpers, so the
//! rendered text never contains API keys or file content. The renderer
//! additionally escapes any `"` or `\` in attribute values per the
//! Prometheus exposition format.

#![cfg(feature = "telemetry")]

use std::sync::Arc;

use opentelemetry::Value;
use opentelemetry_sdk::error::OTelSdkResult;
use opentelemetry_sdk::metrics::Pipeline;
use opentelemetry_sdk::metrics::data::{
    Gauge, Histogram, HistogramDataPoint, ResourceMetrics, Sum,
};
use opentelemetry_sdk::metrics::reader::MetricReader;
use opentelemetry_sdk::metrics::{InstrumentKind, ManualReader, MetricResult, Temporality};

/// Size of the HTTP request read buffer used by the scrape server (LOW-2).
///
/// The scrape server only needs the request line to distinguish `/metrics`
/// from any other path, so a 1 KiB buffer is sufficient for the request head.
const HTTP_READ_BUF: usize = 1024;

// ── SharedManualReader ───────────────────────────────────────────────────

/// A newtype wrapper around `Arc<ManualReader>` that implements
/// [`MetricReader`], so the same reader instance can be registered on a
/// [`SdkMeterProvider`] (which takes ownership via `with_reader`) and held
/// by the Prometheus HTTP server (which needs to call `collect` on demand).
///
/// This is necessary because the OTEL SDK's `with_reader` takes `T:
/// MetricReader` by value, and `Arc<ManualReader>` does not auto-implement
/// `MetricReader`. The wrapper delegates every trait method to the inner
/// `ManualReader`.
#[derive(Debug, Clone)]
pub struct SharedManualReader(Arc<ManualReader>);

impl SharedManualReader {
    /// Create a new shared reader wrapping a fresh [`ManualReader`].
    #[must_use]
    pub fn new() -> Self {
        Self(Arc::new(ManualReader::builder().build()))
    }

    /// Create a new shared reader wrapping the given [`ManualReader`].
    #[must_use]
    pub fn from_reader(reader: ManualReader) -> Self {
        Self(Arc::new(reader))
    }

    /// Returns an `Arc` clone of the inner [`ManualReader`] so the HTTP
    /// server can call `collect` on it.
    #[must_use]
    pub fn handle(&self) -> Arc<ManualReader> {
        Arc::clone(&self.0)
    }
}

impl Default for SharedManualReader {
    fn default() -> Self {
        Self::new()
    }
}

impl MetricReader for SharedManualReader {
    fn register_pipeline(&self, pipeline: std::sync::Weak<Pipeline>) {
        self.0.register_pipeline(pipeline);
    }

    fn collect(&self, rm: &mut ResourceMetrics) -> MetricResult<()> {
        self.0.collect(rm)
    }

    fn force_flush(&self) -> OTelSdkResult {
        self.0.force_flush()
    }

    fn shutdown(&self) -> OTelSdkResult {
        self.0.shutdown()
    }

    fn temporality(&self, kind: InstrumentKind) -> Temporality {
        self.0.temporality(kind)
    }
}

// ── Renderer ──────────────────────────────────────────────────────────────

/// Render a metric snapshot from the given reader as Prometheus text
/// format (FR-028).
///
/// # Arguments
///
/// * `reader` - A [`ManualReader`] registered on the provider whose
///   metrics should be rendered.
///
/// # Non-blocking guarantee (FR-031, FR-033)
///
/// Returns an empty string if the reader cannot collect (e.g. the
/// provider has been shut down). This keeps the HTTP endpoint
/// non-blocking: a failed scrape returns an empty body rather than
/// crashing the server task.
#[must_use]
pub fn render_prometheus_text(reader: &ManualReader) -> String {
    let mut rm = ResourceMetrics {
        resource: opentelemetry_sdk::Resource::builder_empty().build(),
        scope_metrics: Vec::new(),
    };
    if reader.collect(&mut rm).is_err() {
        return String::new();
    }
    format_resource_metrics(&rm)
}

/// Format a [`ResourceMetrics`] snapshot as Prometheus text exposition.
fn format_resource_metrics(rm: &ResourceMetrics) -> String {
    let mut out = String::new();

    // Resource attributes become a synthetic `target_info` line per the
    // Prometheus OTEL exposition convention.
    if !rm.resource.is_empty() {
        out.push_str("# HELP target_info Target metadata\n");
        out.push_str("# TYPE target_info gauge\n");
        out.push_str("target_info");
        let mut kvs: Vec<(opentelemetry::Key, opentelemetry::Value)> = rm
            .resource
            .iter()
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect();
        kvs.sort_by(|a, b| a.0.as_str().cmp(b.0.as_str()));
        for (k, v) in &kvs {
            out.push_str(&format!(
                " {}=\"{}\"",
                k.as_str(),
                escape_label_value(&v.to_string())
            ));
        }
        out.push_str(" 1\n");
    }

    for scope in &rm.scope_metrics {
        for metric in &scope.metrics {
            render_metric(&mut out, metric.name.as_ref(), metric.data.as_any());
        }
    }

    out
}

/// Render a single metric (all its data points) into `out`.
fn render_metric(out: &mut String, name: &str, data: &dyn std::any::Any) {
    if let Some(sum) = data.downcast_ref::<Sum<u64>>() {
        render_sum_u64(out, name, sum);
    } else if let Some(sum) = data.downcast_ref::<Sum<i64>>() {
        render_sum_i64(out, name, sum);
    } else if let Some(sum) = data.downcast_ref::<Sum<f64>>() {
        render_sum_f64(out, name, sum);
    } else if let Some(gauge) = data.downcast_ref::<Gauge<u64>>() {
        render_gauge_u64(out, name, gauge);
    } else if let Some(gauge) = data.downcast_ref::<Gauge<i64>>() {
        render_gauge_i64(out, name, gauge);
    } else if let Some(gauge) = data.downcast_ref::<Gauge<f64>>() {
        render_gauge_f64(out, name, gauge);
    } else if let Some(hist) = data.downcast_ref::<Histogram<u64>>() {
        render_histogram_u64(out, name, hist);
    } else if let Some(hist) = data.downcast_ref::<Histogram<f64>>() {
        render_histogram_f64(out, name, hist);
    }
    // Unknown aggregation types are silently skipped (FR-033: never crash).
}

// ── Sum renderers ────────────────────────────────────────────────────────

fn render_sum_u64(out: &mut String, name: &str, sum: &Sum<u64>) {
    out.push_str(&format!("# HELP {name} ragent metric\n"));
    out.push_str(&format!("# TYPE {name} counter\n"));
    for dp in &sum.data_points {
        let labels = build_labels(&dp.attributes);
        out.push_str(&format!("{name}{labels} {}\n", dp.value));
    }
}

fn render_sum_i64(out: &mut String, name: &str, sum: &Sum<i64>) {
    out.push_str(&format!("# HELP {name} ragent metric\n"));
    out.push_str(&format!("# TYPE {name} gauge\n"));
    for dp in &sum.data_points {
        let labels = build_labels(&dp.attributes);
        out.push_str(&format!("{name}{labels} {}\n", dp.value));
    }
}

fn render_sum_f64(out: &mut String, name: &str, sum: &Sum<f64>) {
    out.push_str(&format!("# HELP {name} ragent metric\n"));
    out.push_str(&format!("# TYPE {name} counter\n"));
    for dp in &sum.data_points {
        let labels = build_labels(&dp.attributes);
        out.push_str(&format!("{name}{labels} {}\n", dp.value));
    }
}

// ── Gauge renderers ──────────────────────────────────────────────────────

fn render_gauge_u64(out: &mut String, name: &str, gauge: &Gauge<u64>) {
    out.push_str(&format!("# HELP {name} ragent metric\n"));
    out.push_str(&format!("# TYPE {name} gauge\n"));
    for dp in &gauge.data_points {
        let labels = build_labels(&dp.attributes);
        out.push_str(&format!("{name}{labels} {}\n", dp.value));
    }
}

fn render_gauge_i64(out: &mut String, name: &str, gauge: &Gauge<i64>) {
    out.push_str(&format!("# HELP {name} ragent metric\n"));
    out.push_str(&format!("# TYPE {name} gauge\n"));
    for dp in &gauge.data_points {
        let labels = build_labels(&dp.attributes);
        out.push_str(&format!("{name}{labels} {}\n", dp.value));
    }
}

fn render_gauge_f64(out: &mut String, name: &str, gauge: &Gauge<f64>) {
    out.push_str(&format!("# HELP {name} ragent metric\n"));
    out.push_str(&format!("# TYPE {name} gauge\n"));
    for dp in &gauge.data_points {
        let labels = build_labels(&dp.attributes);
        out.push_str(&format!("{name}{labels} {}\n", dp.value));
    }
}

// ── Histogram renderers ──────────────────────────────────────────────────

fn render_histogram_u64(out: &mut String, name: &str, hist: &Histogram<u64>) {
    out.push_str(&format!("# HELP {name} ragent histogram\n"));
    out.push_str(&format!("# TYPE {name} histogram\n"));
    for dp in &hist.data_points {
        render_histogram_point(out, name, dp);
    }
}

fn render_histogram_f64(out: &mut String, name: &str, hist: &Histogram<f64>) {
    out.push_str(&format!("# HELP {name} ragent histogram\n"));
    out.push_str(&format!("# TYPE {name} histogram\n"));
    for dp in &hist.data_points {
        render_histogram_point(out, name, dp);
    }
}

fn render_histogram_point<T>(out: &mut String, name: &str, dp: &HistogramDataPoint<T>)
where
    T: Copy + std::fmt::Display,
{
    let base_labels = build_labels(&dp.attributes);
    let total_count: u64 = dp.bucket_counts.iter().sum();
    let sum = &dp.sum;

    // Bucket counts with le="..." labels.
    for (i, bound) in dp.bounds.iter().enumerate() {
        let count: u64 = dp.bucket_counts[..=i].iter().sum();
        // Append le="bound" to the base labels.
        let le_label = format!("le=\"{bound}\"");
        let labels = append_le_label(&base_labels, &le_label);
        out.push_str(&format!("{name}_bucket{labels} {count}\n"));
    }
    let le_inf = "le=\"+Inf\"";
    let labels = append_le_label(&base_labels, le_inf);
    out.push_str(&format!("{name}_bucket{labels} {total_count}\n"));

    out.push_str(&format!("{name}_sum{base_labels} {sum}\n"));
    out.push_str(&format!("{name}_count{base_labels} {total_count}\n"));
}

/// Append a `le="..."` label to an existing label string.
fn append_le_label(base: &str, le: &str) -> String {
    if base.is_empty() {
        format!("{{{le}}}")
    } else {
        // base is like "{k1=\"v1\",k2=\"v2\"}" - insert before the closing }.
        format!("{{{}, {le}}}", &base[1..base.len() - 1])
    }
}

/// Build the Prometheus label string `{k1="v1",k2="v2"}` from a slice of
/// `KeyValue` pairs, sorted by key.
fn build_labels(attrs: &[opentelemetry::KeyValue]) -> String {
    if attrs.is_empty() {
        return String::new();
    }
    let mut kvs: Vec<(&opentelemetry::Key, &Value)> =
        attrs.iter().map(|kv| (&kv.key, &kv.value)).collect();
    kvs.sort_by(|a, b| a.0.as_str().cmp(b.0.as_str()));
    let mut s = String::from("{");
    for (i, (k, v)) in kvs.iter().enumerate() {
        if i > 0 {
            s.push(',');
        }
        s.push_str(&format!(
            "{}=\"{}\"",
            k.as_str(),
            escape_label_value(&v.to_string())
        ));
    }
    s.push('}');
    s
}

/// Escape a label value per the Prometheus exposition format: backslash
/// and double-quote are escaped, newline becomes `\n`.
fn escape_label_value(s: &str) -> String {
    s.replace('\\', "\\\\")
        .replace('"', "\\\"")
        .replace('\n', "\\n")
}

// ── HTTP server ───────────────────────────────────────────────────────────

/// Spawn a Prometheus text endpoint on `127.0.0.1:<port>` (FR-028).
///
/// The server listens for `GET /metrics` and responds with the current
/// metric snapshot rendered as Prometheus text. It runs on a background
/// tokio task; the returned [`tokio::task::JoinHandle`] can be awaited
/// (to know when the server stops) or dropped (fire-and-forget).
///
/// # Arguments
///
/// * `reader` - A [`ManualReader`] (wrapped in `Arc`) registered on the
///   live [`SdkMeterProvider`]. The reader must outlive the server.
/// * `port` - The TCP port to bind on `127.0.0.1`.
///
/// # Errors
///
/// Returns [`std::io::Error`] if the `TcpListener` cannot bind (e.g. port
/// in use). The server task itself never panics - a failed scrape returns
/// an empty body with a 503 status (FR-031, FR-033).
pub async fn serve(
    reader: Arc<ManualReader>,
    port: u16,
) -> std::io::Result<tokio::task::JoinHandle<()>> {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use tokio::net::TcpListener;

    let addr = format!("127.0.0.1:{port}");
    let listener = TcpListener::bind(&addr).await?;

    let handle = tokio::spawn(async move {
        loop {
            let (mut sock, _peer) = match listener.accept().await {
                Ok(p) => p,
                Err(e) => {
                    tracing::warn!(error = %e, "prometheus: accept failed");
                    continue;
                }
            };

            let mut buf = [0u8; HTTP_READ_BUF];
            let n = match sock.read(&mut buf).await {
                Ok(n) => n,
                Err(_) => continue,
            };
            let req = String::from_utf8_lossy(&buf[..n]);
            let is_metrics = req
                .lines()
                .next()
                .is_some_and(|line| line.starts_with("GET /metrics") || line.starts_with("GET / "));

            let body = if is_metrics {
                render_prometheus_text(&reader)
            } else {
                String::new()
            };
            let status = if is_metrics && !body.is_empty() {
                "200 OK"
            } else if is_metrics {
                "503 Service Unavailable"
            } else {
                "404 Not Found"
            };

            let response = format!(
                "HTTP/1.1 {status}\r\nContent-Type: text/plain; version=0.0.4\r\nContent-Length: {}\r\n\r\n{body}",
                body.len()
            );
            if let Err(e) = sock.write_all(response.as_bytes()).await {
                tracing::debug!(error = %e, "prometheus scrape response write failed");
            }
            if let Err(e) = sock.flush().await {
                tracing::debug!(error = %e, "prometheus scrape response flush failed");
            }
        }
    });

    Ok(handle)
}

// ── Tests ─────────────────────────────────────────────────────────────────

#[cfg(test)]
#[path = "../tests/inline/prometheus_tests.rs"]
mod tests;
