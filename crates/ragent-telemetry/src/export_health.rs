//! OTLP export failure accounting and the "Agent Notice" auto-disable policy.
//!
//! OpenTelemetry's SDK deliberately does **not** implement retry logic and the
//! periodic reader reports an export failure only through the internal
//! `otel_error!` log. That leaves a ragent operator with a single tracing line
//! and no idea that every metric batch since startup has been dropped, for
//! example because the collector answers `503` or is unreachable.
//!
//! This module sits between the built OTLP exporter and the periodic reader:
//!
//! - [`ObservedMetricExporter`] wraps the real exporter (see
//!   [`WatchedMetricExporter`]) and every `export` result - success, non-2xx
//!   HTTP response, transport error or export timeout - is classified and
//!   recorded into the process-wide [`export_health`] monitor.
//! - [`ExportHealth`] keeps the failure streak, a de-duplicated list of
//!   notices the UI has not shown yet ([`take_notices`]), and the latched
//!   auto-disable flag.
//!
//! The policy the operator asked for is: every failure produces one notice,
//! and the fifth **consecutive** failure turns telemetry off. A successful
//! export resets the streak.
//!
//! # Why a process-wide monitor
//!
//! The exporter is built inside [`TelemetrySubsystem::new`]/
//! [`reconfigure`](crate::TelemetrySubsystem::reconfigure) and only its
//! `PushMetricExporter` handle reaches the reader; the TUI holds an
//! `Arc<TelemetrySubsystem>`. Both sides therefore share the monitor rather
//! than a hand-threaded handle, which keeps the construction signatures and
//! the ~200 test constructors untouched (the same reasoning as
//! [`crate::counters`]).
//!
//! [`TelemetrySubsystem::new`]: crate::TelemetrySubsystem::new

use std::sync::Mutex;
use std::sync::OnceLock;
use std::sync::atomic::{AtomicU32, AtomicU64, Ordering};

/// Number of consecutive export failures that turn telemetry off (FR: the
/// operator asked for "after 5 consecutive errors, turn off the telemetry").
pub const AUTO_DISABLE_THRESHOLD: u32 = 5;

/// How many notices may queue up for the UI before the oldest is dropped.
///
/// Bounded so an endpoint that fails for hours cannot grow the queue without
/// limit when nothing is draining it (a headless `ragent serve`, for example).
const MAX_QUEUED_NOTICES: usize = 16;

/// Why an OTLP metric export failed, classified from the SDK error.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExportFailureKind {
    /// The collector answered with a non-2xx status (`503`, `401`, ...).
    HttpStatus,
    /// The export exceeded `telemetry.otel.export_timeout_seconds`.
    Timeout,
    /// The request never completed: refused connection, DNS failure, TLS
    /// error, or any other transport-level problem.
    Transport,
    /// The export was abandoned before it finished, or the reader itself was
    /// already shutting down - the SDK reports both as
    /// `InternalFailure("oneshot canceled")` when a `force_flush`/`shutdown`
    /// arrives after the reader stopped accepting work. Kept distinct so a
    /// shutdown log does not misreport a cancelled flush as "collector
    /// unreachable".
    Cancelled,
}

impl ExportFailureKind {
    /// Classify an [`OTelSdkError`](opentelemetry_sdk::error::OTelSdkError)
    /// returned by the exporter.
    ///
    /// The OTLP/HTTP client stringifies its own failures into
    /// `OTelSdkError::InternalFailure`, so the two shapes that matter are
    /// matched by text: the status form emitted by `opentelemetry-otlp`
    /// ("HTTP export failed with status code: 503") and the network form
    /// ("HTTP export failed: network error"). The periodic reader reports its
    /// own deadline as `OTelSdkError::Timeout`, and a flush that loses a race
    /// with reader shutdown as `InternalFailure(".. oneshot canceled")`.
    #[must_use]
    pub fn classify(error: &str) -> Self {
        let lower = error.to_ascii_lowercase();
        if lower.contains("http export failed with status code")
            || lower.contains("status code:")
            || lower.contains("status_code")
        {
            Self::HttpStatus
        } else if lower.contains("oneshot canceled")
            || lower.contains("oneshot cancelled")
            || lower.contains("reader is shut down")
            || lower.contains("already shut down")
        {
            Self::Cancelled
        } else if lower.contains("timeout") || lower.contains("timed out") {
            Self::Timeout
        } else {
            Self::Transport
        }
    }

    /// Short human-readable label used in the Agent Notice text.
    #[must_use]
    pub const fn describe(self) -> &'static str {
        match self {
            Self::HttpStatus => "collector returned an error status",
            Self::Timeout => "export timed out",
            Self::Transport => "collector unreachable",
            Self::Cancelled => "export cancelled while the reader was shutting down",
        }
    }
}

/// A failure worth showing the operator, drained by the TUI.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExportNotice {
    /// Consecutive-failure number this notice belongs to (1-based).
    pub consecutive_failures: u32,
    /// Why the export failed.
    pub kind: ExportFailureKind,
    /// The endpoint that was being exported to (already masked of userinfo by
    /// the subsystem's `mask_endpoint_userinfo` before it reaches here).
    pub endpoint: String,
    /// The SDK's own error text, trimmed for terminal display.
    pub detail: String,
    /// Whether this failure tripped the auto-disable.
    pub auto_disabled: bool,
}

impl ExportNotice {
    /// Render the multi-line "Agent Notice" body shown in the TUI message
    /// window.
    ///
    /// The text is deliberately specific: the failure count, the reason, the
    /// endpoint, and - once the threshold is reached - the fact that telemetry
    /// has been switched off and how to re-enable it.
    #[must_use]
    pub fn render(&self) -> String {
        let mut out = format!(
            "telemetry export failed ({} of {} consecutive): {}",
            self.consecutive_failures,
            AUTO_DISABLE_THRESHOLD,
            self.kind.describe()
        );
        if !self.endpoint.is_empty() {
            out.push_str(&format!(" - OTLP endpoint {}", self.endpoint));
        }
        if !self.detail.is_empty() {
            out.push_str(&format!("\nCause: {}", self.detail));
        }
        if self.auto_disabled {
            out.push_str(&format!(
                "\nTelemetry disabled after {} consecutive export failures, so ragent \
                 stops retrying this endpoint. Fix the endpoint (for example start the \
                 collector, or update `telemetry.otel.endpoint` in ragent.json) and \
                 re-enable with `/telemetry on`.",
                AUTO_DISABLE_THRESHOLD
            ));
        } else {
            out.push_str(&format!(
                "\nTelemetry turns itself off after {} consecutive failures.",
                AUTO_DISABLE_THRESHOLD
            ));
        }
        out
    }
}

/// Process-wide OTLP export health monitor.
///
/// All fields are atomic so the exporter's background task never blocks the UI
/// thread, and the notice queue is a short `Mutex<Vec<_>>` because it is
/// written once per failed export and drained once per UI tick.
#[derive(Debug)]
pub struct ExportHealth {
    /// Consecutive failed exports (reset to zero by a successful export).
    consecutive_failures: AtomicU32,
    /// Total failed exports since process start (never reset).
    total_failures: AtomicU64,
    /// Whether the auto-disable threshold has been reached.
    ///
    /// Latched: it stays set until [`clear`](Self::clear) so the TUI performs
    /// the `/telemetry off` side effect exactly once per threshold crossing
    /// rather than on every poll.
    auto_disabled: AtomicU32,
    /// Notices waiting to be drained by a UI.
    notices: Mutex<Vec<ExportNotice>>,
}

impl ExportHealth {
    /// Create an empty monitor.
    #[must_use]
    pub fn new() -> Self {
        Self {
            consecutive_failures: AtomicU32::new(0),
            total_failures: AtomicU64::new(0),
            auto_disabled: AtomicU32::new(0),
            notices: Mutex::new(Vec::new()),
        }
    }

    /// Record a successful export, clearing the consecutive-failure streak.
    ///
    /// The latched [`is_auto_disabled`](Self::is_auto_disabled) flag is **not**
    /// cleared here: the subsystem has been switched off by the time a later
    /// export could succeed, so the flag must survive until the UI acts on it.
    pub fn record_success(&self) {
        self.consecutive_failures.store(0, Ordering::Relaxed);
    }

    /// Record a failed export and queue the notice describing it.
    ///
    /// Returns the queued notice (also available to the UI through
    /// [`take_notices`](Self::take_notices)).
    pub fn record_failure(
        &self,
        kind: ExportFailureKind,
        endpoint: &str,
        detail: &str,
    ) -> ExportNotice {
        self.total_failures.fetch_add(1, Ordering::Relaxed);
        let consecutive = self.consecutive_failures.fetch_add(1, Ordering::Relaxed) + 1;
        let tripped = consecutive >= AUTO_DISABLE_THRESHOLD;
        let auto_disabled = tripped && !self.is_auto_disabled();
        if auto_disabled {
            self.auto_disabled.store(1, Ordering::Relaxed);
        }
        let notice = ExportNotice {
            consecutive_failures: consecutive.min(AUTO_DISABLE_THRESHOLD),
            kind,
            endpoint: endpoint.to_string(),
            detail: detail.trim().to_string(),
            auto_disabled,
        };
        self.push_notice(notice.clone());
        notice
    }

    /// Queue a notice, dropping the oldest once the queue is full.
    fn push_notice(&self, notice: ExportNotice) {
        let mut guard = self
            .notices
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if guard.len() >= MAX_QUEUED_NOTICES {
            guard.remove(0);
        }
        guard.push(notice);
    }

    /// Current consecutive-failure count.
    #[must_use]
    pub fn consecutive_failures(&self) -> u32 {
        self.consecutive_failures.load(Ordering::Relaxed)
    }

    /// Total failed exports since process start.
    #[must_use]
    pub fn total_failures(&self) -> u64 {
        self.total_failures.load(Ordering::Relaxed)
    }

    /// Whether the auto-disable threshold has been reached and not yet
    /// acknowledged with [`clear`](Self::clear).
    #[must_use]
    pub fn is_auto_disabled(&self) -> bool {
        self.auto_disabled.load(Ordering::Relaxed) != 0
    }

    /// Drain the queued notices (oldest first). Returns an empty vector when
    /// there is nothing to show.
    #[must_use]
    pub fn take_notices(&self) -> Vec<ExportNotice> {
        let mut guard = self
            .notices
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        std::mem::take(&mut *guard)
    }

    /// Number of notices waiting to be drained.
    #[must_use]
    pub fn pending_notice_count(&self) -> usize {
        self.notices
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .len()
    }

    /// Reset the monitor: zero the counters, drop queued notices and release
    /// the latched auto-disable flag.
    ///
    /// Called when telemetry is (re)enabled through `/telemetry on` so a fresh
    /// provider starts with a clean streak, and by the TUI after it has acted
    /// on an auto-disable.
    pub fn clear(&self) {
        self.consecutive_failures.store(0, Ordering::Relaxed);
        self.auto_disabled.store(0, Ordering::Relaxed);
        self.notices
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .clear();
    }
}

impl Default for ExportHealth {
    fn default() -> Self {
        Self::new()
    }
}

/// The process-wide export health monitor.
static EXPORT_HEALTH: OnceLock<ExportHealth> = OnceLock::new();

/// Return the process-wide [`ExportHealth`] monitor, creating it on first use.
#[must_use]
pub fn export_health() -> &'static ExportHealth {
    EXPORT_HEALTH.get_or_init(ExportHealth::new)
}

/// Wrap a live OTLP metric exporter so every export result is recorded in
/// [`export_health`].
///
/// This is the type the periodic reader is built from in
/// [`crate::subsystem`].
#[cfg(feature = "telemetry")]
pub type WatchedMetricExporter = ObservedMetricExporter<opentelemetry_otlp::MetricExporter>;

/// Decorator over a [`PushMetricExporter`](opentelemetry_sdk::metrics::exporter::PushMetricExporter)
/// that records each export outcome in the shared [`ExportHealth`] monitor.
///
/// All calls (including the synchronous `force_flush`/`shutdown` pair) are
/// forwarded unchanged to the wrapped exporter, so behaviour is identical to
/// using the exporter directly - only the observation is added.
#[cfg(feature = "telemetry")]
#[derive(Debug, Clone)]
pub struct ObservedMetricExporter<E> {
    inner: E,
    endpoint: std::sync::Arc<str>,
}

#[cfg(feature = "telemetry")]
impl<E> ObservedMetricExporter<E> {
    /// Wrap `inner`, attributing its failures to `endpoint` in the notices.
    ///
    /// The caller is responsible for masking any userinfo out of `endpoint`
    /// first (the subsystem passes the output of
    /// [`mask_endpoint_userinfo`](crate::TelemetrySubsystem::mask_endpoint_userinfo)).
    pub fn new(inner: E, endpoint: impl Into<std::sync::Arc<str>>) -> Self {
        Self {
            inner,
            endpoint: endpoint.into(),
        }
    }

    /// Borrow the wrapped exporter.
    #[must_use]
    pub const fn inner(&self) -> &E {
        &self.inner
    }
}

#[cfg(feature = "telemetry")]
impl<E> opentelemetry_sdk::metrics::exporter::PushMetricExporter for ObservedMetricExporter<E>
where
    E: opentelemetry_sdk::metrics::exporter::PushMetricExporter,
{
    /// Forward the export to the wrapped exporter and record its outcome.
    ///
    /// The result is returned unchanged so the periodic reader logs it exactly
    /// as it would without the wrapper; the recorded notice is what the TUI
    /// turns into an "Agent Notice" bubble.
    async fn export(
        &self,
        metrics: &opentelemetry_sdk::metrics::data::ResourceMetrics,
    ) -> opentelemetry_sdk::error::OTelSdkResult {
        let result = self.inner.export(metrics).await;
        match &result {
            Ok(()) => export_health().record_success(),
            Err(error) => {
                let detail = error.to_string();
                let notice = export_health().record_failure(
                    ExportFailureKind::classify(&detail),
                    &self.endpoint,
                    &detail,
                );
                if notice.auto_disabled {
                    tracing::warn!(
                        endpoint = %self.endpoint,
                        failures = notice.consecutive_failures,
                        "telemetry: disabling metric export after {} consecutive failures",
                        AUTO_DISABLE_THRESHOLD
                    );
                } else {
                    tracing::warn!(
                        endpoint = %self.endpoint,
                        failures = notice.consecutive_failures,
                        error = %detail,
                        "telemetry: OTLP metric export failed"
                    );
                }
            }
        }
        result
    }

    fn force_flush(&self) -> opentelemetry_sdk::error::OTelSdkResult {
        self.inner.force_flush()
    }

    fn shutdown_with_timeout(
        &self,
        timeout: std::time::Duration,
    ) -> opentelemetry_sdk::error::OTelSdkResult {
        self.inner.shutdown_with_timeout(timeout)
    }

    fn temporality(&self) -> opentelemetry_sdk::metrics::Temporality {
        self.inner.temporality()
    }
}
