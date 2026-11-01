//! Integration tests for the OTLP/HTTP metrics request path.
//!
//! `telemetry.otel.endpoint` is documented as a base URL
//! (`http://localhost:4318`), but `opentelemetry-otlp` treats a programmatic
//! `with_endpoint` value as the COMPLETE signal URL and posts to it verbatim.
//! Passing the bare base URL therefore reached a stock collector as `POST /`
//! and drew a `404`, which the export-health monitor reported as
//! "collector returned an error status" until telemetry switched itself off.
//!
//! These tests pin both halves of the fix: the pure URL resolver
//! [`ragent_telemetry::subsystem::otlp_http_metrics_endpoint`], and a real
//! export through a live `SdkMeterProvider` against a mock collector that
//! records the request line it receives.

#![cfg(feature = "telemetry")]

use std::io::{Read, Write};
use std::net::TcpListener;
use std::sync::mpsc;
use std::time::Duration;

use ragent_telemetry::subsystem::otlp_http_metrics_endpoint;

/// A bare base URL gains the `/v1/metrics` signal path.
#[test]
fn test_endpoint_base_url_gains_signal_path() {
    assert_eq!(
        otlp_http_metrics_endpoint("http://localhost:4318"),
        "http://localhost:4318/v1/metrics"
    );
}

/// A trailing slash on the base URL does not produce a doubled slash.
#[test]
fn test_endpoint_trailing_slash_is_normalised() {
    assert_eq!(
        otlp_http_metrics_endpoint("http://localhost:4318/"),
        "http://localhost:4318/v1/metrics"
    );
    assert_eq!(
        otlp_http_metrics_endpoint("http://collector:4318///"),
        "http://collector:4318/v1/metrics"
    );
}

/// A deployment that mounts the collector under a path keeps that path.
#[test]
fn test_endpoint_base_path_is_preserved() {
    assert_eq!(
        otlp_http_metrics_endpoint("https://gw.example.com/otel"),
        "https://gw.example.com/otel/v1/metrics"
    );
    assert_eq!(
        otlp_http_metrics_endpoint("https://gw.example.com/otel/"),
        "https://gw.example.com/otel/v1/metrics"
    );
}

/// An endpoint that already names a signal URL is left exactly as configured,
/// so the documented base-URL form and the explicit signal-URL form both work.
#[test]
fn test_endpoint_signal_url_is_unchanged() {
    for endpoint in [
        "https://gw.example.com/v1/metrics",
        "https://gw.example.com/collector/v1/metrics",
        "https://gw.example.com/v1/traces",
    ] {
        assert_eq!(
            otlp_http_metrics_endpoint(endpoint),
            endpoint,
            "{endpoint} already names a signal path and must not be rewritten"
        );
    }
}

/// A query string or fragment survives the appended path.
#[test]
fn test_endpoint_query_and_fragment_are_preserved() {
    assert_eq!(
        otlp_http_metrics_endpoint("https://gw.example.com/otel?tenant=acme"),
        "https://gw.example.com/otel/v1/metrics?tenant=acme"
    );
    assert_eq!(
        otlp_http_metrics_endpoint("https://gw.example.com?tenant=acme"),
        "https://gw.example.com/v1/metrics?tenant=acme"
    );
}

/// Surrounding whitespace in a hand-edited config does not corrupt the URL.
#[test]
fn test_endpoint_surrounding_whitespace_is_trimmed() {
    assert_eq!(
        otlp_http_metrics_endpoint("  http://localhost:4318  "),
        "http://localhost:4318/v1/metrics"
    );
}

/// The regression itself, end to end: a subsystem built from the documented
/// base URL exports to `/v1/metrics`, not to `/`.
///
/// A stock collector answers `POST /` with 404 and `POST /v1/metrics` with 200,
/// so the mock is faithful in the one respect that matters: it answers 404 for
/// any path other than `/v1/metrics`.
#[test]
fn test_export_targets_v1_metrics_path() {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind mock collector");
    let port = listener.local_addr().expect("local addr").port();
    let endpoint = format!("http://127.0.0.1:{port}");

    let (tx, rx) = mpsc::channel();
    let collector = std::thread::spawn(move || {
        if let Ok((mut stream, _)) = listener.accept() {
            let mut buffer = [0_u8; 8192];
            let read = stream.read(&mut buffer).unwrap_or(0);
            let request = String::from_utf8_lossy(&buffer[..read]).to_string();
            let target = request
                .lines()
                .next()
                .unwrap_or_default()
                .split_whitespace()
                .nth(1)
                .unwrap_or("/")
                .to_string();
            let status = if target == "/v1/metrics" {
                "200 OK"
            } else {
                "404 Not Found"
            };
            // Drain whatever body remains so the client is not reset mid-write.
            let _ = stream.set_read_timeout(Some(Duration::from_millis(200)));
            let _ = stream.write_all(
                format!("HTTP/1.1 {status}\r\nContent-Length: 0\r\nConnection: close\r\n\r\n")
                    .as_bytes(),
            );
            let _ = stream.flush();
            let _ = tx.send(target);
        }
    });

    let config = ragent_config::OtelConfig {
        enabled: true,
        endpoint,
        export_interval_seconds: 1,
        export_timeout_seconds: 5,
        ..Default::default()
    };

    let subsystem = ragent_telemetry::TelemetrySubsystem::new(config)
        .expect("subsystem builds from the documented base URL");

    // A flush with no recorded metric exports nothing at all, so record one.
    let registry = subsystem
        .instruments()
        .expect("enabled subsystem has instruments");
    registry.llm_requests.add(1, &[]);

    // Force an immediate export so the test does not wait a full interval.
    let _ = subsystem.flush();

    let target = rx
        .recv_timeout(Duration::from_secs(15))
        .expect("collector should have received the export");
    collector.join().expect("collector thread");

    assert_eq!(
        target, "/v1/metrics",
        "the export must target the OTLP/HTTP metrics path, not the bare root"
    );
}
