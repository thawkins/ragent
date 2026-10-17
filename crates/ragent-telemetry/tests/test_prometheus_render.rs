//! External tests for the Prometheus text renderer and scrape server in
//! `prometheus.rs`. These drive real OTEL instruments through an in-memory
//! `ManualReader` and assert on the rendered exposition text, then exercise the
//! HTTP scrape path end to end.

use opentelemetry::metrics::MeterProvider;
use opentelemetry_sdk::metrics::{ManualReader, SdkMeterProvider};
use ragent_telemetry::prometheus::{SharedManualReader, render_prometheus_text, serve};

/// Build a provider + shared reader handle and return both so tests can record
/// metrics and collect/render them without a runtime timer.
fn provider_with_reader() -> (
    SdkMeterProvider,
    std::sync::Arc<ManualReader>,
    tokio::runtime::Runtime,
) {
    let rt = tokio::runtime::Runtime::new().expect("tokio runtime");
    let shared = SharedManualReader::new();
    let handle = shared.handle();
    let provider = SdkMeterProvider::builder().with_reader(shared).build();
    (provider, handle, rt)
}

/// A `u64_counter` renders a `# TYPE ... counter` line and a sample line with a
/// `+Inf`-free scalar body.
#[test]
fn test_render_u64_counter_and_labels() {
    let (provider, handle, rt) = provider_with_reader();
    let meter = provider.meter("ragent.prom");
    let counter = meter.u64_counter("ragent.llm.requests").build();
    counter.add(3, &[opentelemetry::KeyValue::new("provider", "openai")]);

    rt.block_on(async {
        // ManualReader::collect is synchronous; no flush needed.
    });

    let text = render_prometheus_text(&handle);
    assert!(
        text.contains("# TYPE ragent.llm.requests counter"),
        "expected counter TYPE line, got: {text}"
    );
    assert!(
        text.contains("provider=\"openai\""),
        "expected label, got: {text}"
    );
    assert!(
        text.contains("ragent.llm.requests{provider=\"openai\"} 3"),
        "expected sample value 3, got: {text}"
    );
    let _ = provider;
}

/// A `f64_counter` renders via the `Sum<f64>` path.
#[test]
fn test_render_f64_counter() {
    let (provider, handle, _rt) = provider_with_reader();
    let meter = provider.meter("ragent.prom");
    let counter = meter.f64_counter("ragent.cost.estimated").build();
    counter.add(1.5, &[]);

    let text = render_prometheus_text(&handle);
    assert!(
        text.contains("ragent.cost.estimated 1.5"),
        "expected f64 sum, got: {text}"
    );
    let _ = provider;
}

/// An `i64_up_down_counter` renders via the `Sum<i64>` path (gauge word).
#[test]
fn test_render_i64_up_down_counter() {
    let (provider, handle, _rt) = provider_with_reader();
    let meter = provider.meter("ragent.prom");
    let gauge = meter.i64_up_down_counter("ragent.sessions.active").build();
    gauge.add(7, &[]);

    let text = render_prometheus_text(&handle);
    assert!(
        text.contains("ragent.sessions.active 7"),
        "expected i64 sum, got: {text}"
    );
    let _ = provider;
}

/// `i64_gauge` and `f64_gauge` render through the `Gauge` paths.
#[test]
fn test_render_gauges() {
    let (provider, handle, _rt) = provider_with_reader();
    let meter = provider.meter("ragent.prom");
    let g_i = meter.i64_gauge("ragent.team.members").build();
    g_i.record(-2, &[]);
    let g_f = meter.f64_gauge("ragent.rate_limit.requests_pct").build();
    g_f.record(42.5, &[]);

    let text = render_prometheus_text(&handle);
    assert!(
        text.contains("ragent.team.members -2"),
        "expected i64 gauge, got: {text}"
    );
    assert!(
        text.contains("ragent.rate_limit.requests_pct 42.5"),
        "expected f64 gauge, got: {text}"
    );
    let _ = provider;
}

/// A `u64_histogram` renders `_bucket`, `_sum`, and `_count` lines, including a
/// bounded `le=` bucket and the `+Inf` bucket.
#[test]
fn test_render_u64_histogram() {
    let (provider, handle, _rt) = provider_with_reader();
    let meter = provider.meter("ragent.prom");
    let hist = meter
        .u64_histogram("ragent.tool.duration.count")
        .with_boundaries(vec![10.0, 100.0])
        .build();
    hist.record(5, &[opentelemetry::KeyValue::new("tool", "bash")]);
    hist.record(50, &[opentelemetry::KeyValue::new("tool", "bash")]);
    hist.record(500, &[opentelemetry::KeyValue::new("tool", "bash")]);

    let text = render_prometheus_text(&handle);
    assert!(
        text.contains("ragent.tool.duration.count_bucket"),
        "expected _bucket lines, got: {text}"
    );
    assert!(
        text.contains("le=\"10\""),
        "expected a finite le bucket, got: {text}"
    );
    assert!(
        text.contains("le=\"+Inf\""),
        "expected the +Inf bucket, got: {text}"
    );
    assert!(
        text.contains("ragent.tool.duration.count_count"),
        "expected the _count line, got: {text}"
    );
    assert!(
        text.contains("ragent.tool.duration.count_sum"),
        "expected the _sum line, got: {text}"
    );
    let _ = provider;
}

/// An `f64_histogram` renders through the `Histogram<f64>` path.
#[test]
fn test_render_f64_histogram() {
    let (provider, handle, _rt) = provider_with_reader();
    let meter = provider.meter("ragent.prom");
    let hist = meter.f64_histogram("ragent.llm.duration").build();
    hist.record(12.5, &[]);

    let text = render_prometheus_text(&handle);
    assert!(
        text.contains("ragent.llm.duration_count"),
        "expected f64 histogram _count line, got: {text}"
    );
    assert!(
        text.contains("ragent.llm.duration_sum"),
        "expected f64 histogram _sum line, got: {text}"
    );
    let _ = provider;
}

/// Recording an exponential histogram (via an explicit view is complex; instead
/// assert that an instrument with no matching aggregation still renders the
/// metrics we do support). This is a smoke test ensuring mixed instrument types
/// render without panicking.
#[test]
fn test_render_mixed_instruments_no_panic() {
    let (provider, handle, _rt) = provider_with_reader();
    let meter = provider.meter("ragent.prom");
    meter.u64_counter("a.counter").build().add(1, &[]);
    meter.f64_gauge("b.gauge").build().record(2.5, &[]);
    meter.u64_histogram("c.hist").build().record(3, &[]);
    meter.i64_up_down_counter("d.updown").build().add(4, &[]);

    let text = render_prometheus_text(&handle);
    assert!(text.contains("a.counter"));
    assert!(text.contains("b.gauge"));
    assert!(text.contains("c.hist_bucket"));
    assert!(text.contains("d.updown"));
    let _ = provider;
}

/// `serve` binds a loopback port and answers `GET /metrics` with 200 and a body
/// that carries the recorded metric; a non-metrics path yields 404.
#[test]
fn test_serve_scrape_metrics_and_404() {
    let rt = tokio::runtime::Runtime::new().expect("tokio runtime");
    rt.block_on(async {
        let shared = SharedManualReader::new();
        let handle = shared.handle();
        let provider = SdkMeterProvider::builder().with_reader(shared).build();
        let meter = provider.meter("ragent.prom");
        meter.u64_counter("ragent.scrape.probe").build().add(9, &[]);

        // Bind an ephemeral port by asking the OS for one, then serve.
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
            .await
            .expect("bind ephemeral");
        let port = listener.local_addr().expect("local addr").port();
        drop(listener);

        let server = serve(handle, port).await.expect("serve binds");

        // Give the accept loop a moment to start.
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;

        use tokio::io::{AsyncReadExt, AsyncWriteExt};

        // A /metrics request should return 200 with the rendered body.
        let mut sock = tokio::net::TcpStream::connect(("127.0.0.1", port))
            .await
            .expect("connect");
        sock.write_all(b"GET /metrics HTTP/1.1\r\nHost: x\r\n\r\n")
            .await
            .expect("write request");
        let mut resp = Vec::new();
        sock.read_to_end(&mut resp).await.expect("read resp");
        let resp = String::from_utf8_lossy(&resp).to_string();
        assert!(
            resp.starts_with("HTTP/1.1 200 OK"),
            "expected 200 for /metrics, got: {resp}"
        );
        assert!(
            resp.contains("ragent.scrape.probe"),
            "response body should carry the metric, got: {resp}"
        );

        // A non-metrics path should return 404.
        let mut sock2 = tokio::net::TcpStream::connect(("127.0.0.1", port))
            .await
            .expect("connect 2");
        sock2
            .write_all(b"GET /other HTTP/1.1\r\nHost: x\r\n\r\n")
            .await
            .expect("write request 2");
        let mut resp2 = Vec::new();
        sock2.read_to_end(&mut resp2).await.expect("read resp 2");
        let resp2 = String::from_utf8_lossy(&resp2).to_string();
        assert!(
            resp2.starts_with("HTTP/1.1 404 Not Found"),
            "expected 404 for /other, got: {resp2}"
        );

        server.abort();
        let _ = provider;
    });
}
