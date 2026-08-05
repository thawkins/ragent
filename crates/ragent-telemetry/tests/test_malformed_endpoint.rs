//! Integration test: malformed endpoint does not crash (T-032, AC-7).
//!
//! AC-7: "A malformed or unreachable OTLP endpoint must not crash the agent;
//! exporter errors are logged and retried on the next export interval."
//!
//! This test constructs a [`TelemetrySubsystem`] with a deliberately invalid
//! endpoint and verifies that:
//!
//! 1. Subsystem construction either succeeds (gated to a no-op/safe provider)
//!    or returns an `Err` — it never panics.
//! 2. Recording metrics through the resulting provider is still non-blocking
//!    and does not panic.
//! 3. Flush and shutdown complete without panicking, even if they return an
//!    error.
//!
//! The malformed cases cover syntactically invalid URLs (not just "bad
//! protocol") and an endpoint that cannot be resolved at export time.

#![cfg(feature = "telemetry")]

use ragent_telemetry::{OtelConfig, OtelProtocol, TelemetrySubsystem};

/// A URL with an invalid transport is rejected gracefully without panicking.
#[test]
fn test_malformed_endpoint_invalid_protocol_does_not_panic() {
    let config = OtelConfig {
        enabled: true,
        endpoint: "not-a-url".to_string(),
        protocol: OtelProtocol::Http,
        ..Default::default()
    };

    let result = TelemetrySubsystem::new(config);
    assert!(
        result.is_err(),
        "a malformed endpoint should be rejected, not crash"
    );
}

/// A syntactically invalid URL with a valid-looking scheme is rejected
/// gracefully without panicking.
#[test]
fn test_malformed_endpoint_bad_url_syntax_does_not_panic() {
    let config = OtelConfig {
        enabled: true,
        endpoint: "http:///no-host:4318".to_string(),
        protocol: OtelProtocol::Http,
        ..Default::default()
    };

    let result = TelemetrySubsystem::new(config);
    assert!(
        result.is_err(),
        "a syntactically invalid endpoint should be rejected, not crash"
    );
}

/// A validly constructed but unreachable endpoint does not panic when
/// exporting; errors are surfaced through `flush()`/`shutdown()` or retried
/// on the next interval (FR-033).
#[test]
fn test_unreachable_endpoint_does_not_panic_on_flush_or_shutdown() {
    let config = OtelConfig {
        enabled: true,
        endpoint: "http://[::1]:1".to_string(),
        protocol: OtelProtocol::Http,
        export_timeout_seconds: 1,
        export_interval_seconds: 3600,
        ..Default::default()
    };

    let rt = tokio::runtime::Runtime::new().expect("tokio runtime");
    let sub = rt
        .block_on(async { TelemetrySubsystem::new(config) })
        .expect("validly-formed endpoint should construct");

    // Recording remains safe even though export will fail.
    if let Some(registry) = sub.instruments() {
        registry.llm_requests.add(1, &[]);
    }

    // Flush and shutdown may error, but they must not panic.
    let _ = rt.block_on(async { sub.flush() });
    let _ = rt.block_on(async { sub.shutdown() });
}
