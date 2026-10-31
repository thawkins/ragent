//! Integration tests for OTLP export-failure accounting (the "Agent Notice"
//! and auto-disable policy).
//!
//! These exercise [`ExportHealth`] directly - it is a plain atomic counter
//! plus a short queue, so no live collector or SDK provider is required - and
//! the [`ObservedMetricExporter`] decorator over a fake `PushMetricExporter`
//! so the classification of a non-2xx response, a timeout, and a transport
//! error is verified end to end.

use ragent_telemetry::export_health::{
    AUTO_DISABLE_THRESHOLD, ExportFailureKind, ExportHealth, export_health,
};

/// A fresh monitor starts clean: no streak, no notices, no auto-disable.
#[test]
fn test_export_health_starts_clean() {
    let health = ExportHealth::new();
    assert_eq!(health.consecutive_failures(), 0);
    assert_eq!(health.total_failures(), 0);
    assert!(!health.is_auto_disabled());
    assert_eq!(health.pending_notice_count(), 0);
    assert_eq!(health.take_notices().len(), 0);
}

/// Each failure queues exactly one notice and reports the running streak.
#[test]
fn test_export_health_queues_one_notice_per_failure() {
    let health = ExportHealth::new();
    let first = health.record_failure(ExportFailureKind::HttpStatus, "http://c:4318", "503");
    assert_eq!(first.consecutive_failures, 1);
    assert!(!first.auto_disabled);
    let second = health.record_failure(ExportFailureKind::Timeout, "http://c:4318", "timed out");
    assert_eq!(second.consecutive_failures, 2);
    assert_eq!(health.pending_notice_count(), 2);
    assert_eq!(health.total_failures(), 2);
    assert_eq!(health.take_notices().len(), 2);
    // Draining empties the queue.
    assert_eq!(health.pending_notice_count(), 0);
}

/// The fifth consecutive failure latches the auto-disable, exactly once.
#[test]
fn test_export_health_auto_disables_on_fifth_consecutive_failure() {
    let health = ExportHealth::new();
    for n in 1..AUTO_DISABLE_THRESHOLD {
        let notice =
            health.record_failure(ExportFailureKind::Transport, "http://c:4318", "refused");
        assert_eq!(notice.consecutive_failures, n);
        assert!(!notice.auto_disabled, "failure {n} must not auto-disable");
        assert!(!health.is_auto_disabled());
    }
    let fifth = health.record_failure(ExportFailureKind::Transport, "http://c:4318", "refused");
    assert_eq!(fifth.consecutive_failures, AUTO_DISABLE_THRESHOLD);
    assert!(fifth.auto_disabled, "the fifth failure must auto-disable");
    assert!(health.is_auto_disabled());

    // A sixth failure keeps the streak counting but does not re-trip the
    // latched flag: the TUI must run the disable side effect only once.
    let sixth = health.record_failure(ExportFailureKind::Transport, "http://c:4318", "refused");
    assert!(
        !sixth.auto_disabled,
        "the latch trips once, not on every failure"
    );
}

/// A successful export resets the streak, so failures must be consecutive.
#[test]
fn test_export_health_success_resets_streak() {
    let health = ExportHealth::new();
    for _ in 0..(AUTO_DISABLE_THRESHOLD - 1) {
        health.record_failure(ExportFailureKind::HttpStatus, "http://c:4318", "503");
    }
    health.record_success();
    assert_eq!(health.consecutive_failures(), 0);
    assert!(!health.is_auto_disabled());

    let after = health.record_failure(ExportFailureKind::HttpStatus, "http://c:4318", "503");
    assert_eq!(after.consecutive_failures, 1);
    assert!(!after.auto_disabled);
    // The total is a lifetime counter and is never reset by a success.
    assert_eq!(health.total_failures(), AUTO_DISABLE_THRESHOLD as u64);
}

/// `clear` releases the latch and drops queued notices (the `/telemetry on`
/// path), so a retried endpoint starts with a clean slate.
#[test]
fn test_export_health_clear_releases_latch() {
    let health = ExportHealth::new();
    for _ in 0..AUTO_DISABLE_THRESHOLD {
        health.record_failure(ExportFailureKind::Transport, "http://c:4318", "refused");
    }
    assert!(health.is_auto_disabled());
    health.clear();
    assert!(!health.is_auto_disabled());
    assert_eq!(health.consecutive_failures(), 0);
    assert_eq!(health.pending_notice_count(), 0);
    // The lifetime total deliberately survives `clear`.
    assert_eq!(health.total_failures(), AUTO_DISABLE_THRESHOLD as u64);
}

/// The notice text names the failure count, the reason, the endpoint, and -
/// once tripped - the fact that telemetry was disabled.
#[test]
fn test_export_notice_render_names_count_reason_and_endpoint() {
    let health = ExportHealth::new();
    let notice = health.record_failure(
        ExportFailureKind::HttpStatus,
        "http://collector:4318/v1/metrics",
        "HTTP export failed with status code: 503",
    );
    let text = notice.render();
    assert!(text.contains("1 of 5 consecutive"), "got: {text}");
    assert!(
        text.contains("collector returned an error status"),
        "got: {text}"
    );
    assert!(
        text.contains("http://collector:4318/v1/metrics"),
        "got: {text}"
    );
    assert!(text.contains("status code: 503"), "got: {text}");
    assert!(!text.contains("Telemetry disabled"), "got: {text}");
}

/// The auto-disable notice explains what happened and how to recover.
#[test]
fn test_export_notice_render_explains_auto_disable() {
    let health = ExportHealth::new();
    let mut notice =
        health.record_failure(ExportFailureKind::Transport, "http://c:4318", "refused");
    for _ in 1..AUTO_DISABLE_THRESHOLD {
        notice = health.record_failure(ExportFailureKind::Transport, "http://c:4318", "refused");
    }
    let text = notice.render();
    assert!(text.contains("5 of 5 consecutive"), "got: {text}");
    assert!(
        text.contains("Telemetry disabled after 5 consecutive"),
        "got: {text}"
    );
    assert!(text.contains("/telemetry on"), "got: {text}");
}

/// An unreachable endpoint with no userinfo is left verbatim in the notice.
#[test]
fn test_export_notice_endpoint_is_verbatim_when_unmasked() {
    let health = ExportHealth::new();
    let notice = health.record_failure(ExportFailureKind::Timeout, "http://c:4318", "");
    assert_eq!(notice.endpoint, "http://c:4318");
    // An empty detail must not produce a dangling "Cause:" line.
    assert!(
        !notice.render().contains("Cause:"),
        "got: {}",
        notice.render()
    );
}

/// The failure kinds the SDK can produce are classified correctly.
#[test]
fn test_export_failure_kind_classification() {
    assert_eq!(
        ExportFailureKind::classify("Operation failed: HTTP export failed with status code: 401"),
        ExportFailureKind::HttpStatus
    );
    assert_eq!(
        ExportFailureKind::classify("Operation failed: HTTP export failed: network error"),
        ExportFailureKind::Transport
    );
    assert_eq!(
        ExportFailureKind::classify("Operation timed out: 2s"),
        ExportFailureKind::Timeout
    );
    assert_eq!(
        ExportFailureKind::describe(ExportFailureKind::Timeout),
        "export timed out"
    );
}

/// The process-wide monitor is a single shared instance.
#[test]
fn test_export_health_is_process_wide() {
    assert!(std::ptr::eq(export_health(), export_health()));
}

// -- ObservedMetricExporter -------------------------------------------------

/// A fake exporter that always fails with a caller-chosen message.
#[cfg(feature = "telemetry")]
mod observed {
    use super::*;
    use opentelemetry_sdk::error::{OTelSdkError, OTelSdkResult};
    use opentelemetry_sdk::metrics::Temporality;
    use opentelemetry_sdk::metrics::data::ResourceMetrics;
    use opentelemetry_sdk::metrics::exporter::PushMetricExporter;

    use ragent_telemetry::export_health::ObservedMetricExporter;

    /// Build an empty `ResourceMetrics` for the fake exporter to receive.
    fn empty_metrics() -> ResourceMetrics {
        ResourceMetrics::default()
    }

    /// Fake exporter returning `failure` (an error message) or success.
    #[derive(Debug)]
    struct FakeExporter(Option<String>);

    impl PushMetricExporter for FakeExporter {
        fn export(
            &self,
            _metrics: &ResourceMetrics,
        ) -> impl std::future::Future<Output = OTelSdkResult> + Send {
            let failure = self.0.clone();
            async move {
                match failure {
                    Some(message) => Err(OTelSdkError::InternalFailure(message)),
                    None => Ok(()),
                }
            }
        }

        fn force_flush(&self) -> OTelSdkResult {
            Ok(())
        }

        fn shutdown_with_timeout(&self, _timeout: std::time::Duration) -> OTelSdkResult {
            Ok(())
        }

        fn temporality(&self) -> Temporality {
            Temporality::Cumulative
        }
    }

    /// Convenience: wrap a fake exporter behind the observed decorator.
    fn observed(failure: Option<&str>) -> ObservedMetricExporter<FakeExporter> {
        let inner = FakeExporter(failure.map(str::to_string));
        ObservedMetricExporter::new(inner, "http://c:4318")
    }

    /// Run one export to completion on a current-thread runtime.
    fn run_export(exporter: &ObservedMetricExporter<FakeExporter>) -> OTelSdkResult {
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("runtime");
        rt.block_on(async { exporter.export(&empty_metrics()).await })
    }

    /// A failing export still returns the SDK error unchanged (the periodic
    /// reader's own logging is unaffected) and queues one notice.
    #[test]
    fn test_observed_exporter_forwards_error_and_records_notice() {
        let health = ragent_telemetry::export_health::export_health();
        health.clear();
        let exporter = observed(Some("HTTP export failed with status code: 503"));

        let result = run_export(&exporter);
        let error = result.expect_err("the SDK error must be forwarded unchanged");
        assert!(
            error.to_string().contains("status code: 503"),
            "got: {error}"
        );

        let notices = health.take_notices();
        assert_eq!(notices.len(), 1, "one failed export must queue one notice");
        assert_eq!(notices[0].kind, ExportFailureKind::HttpStatus);
        assert_eq!(notices[0].endpoint, "http://c:4318");
        assert_eq!(health.consecutive_failures(), 1);
        health.clear();
    }

    /// A successful export records success and queues nothing.
    #[test]
    fn test_observed_exporter_success_queues_no_notice() {
        let health = ragent_telemetry::export_health::export_health();
        health.clear();
        let exporter = observed(None);

        assert!(run_export(&exporter).is_ok());
        assert_eq!(health.pending_notice_count(), 0);
        assert_eq!(health.consecutive_failures(), 0);
        health.clear();
    }

    /// Five consecutive failing exports through the decorator latch the
    /// auto-disable, and the fifth notice carries the disable wording.
    #[test]
    fn test_observed_exporter_trips_auto_disable_on_fifth_failure() {
        let health = ragent_telemetry::export_health::export_health();
        health.clear();
        let exporter = observed(Some("Operation failed: HTTP export failed: network error"));

        for _ in 0..AUTO_DISABLE_THRESHOLD {
            assert!(run_export(&exporter).is_err());
        }
        assert!(health.is_auto_disabled());
        let notices = health.take_notices();
        assert_eq!(notices.len(), AUTO_DISABLE_THRESHOLD as usize);
        assert!(
            notices.last().is_some_and(|n| n.auto_disabled),
            "the final notice must announce the disable"
        );
        assert_eq!(notices[0].kind, ExportFailureKind::Transport);
        health.clear();
    }
}
