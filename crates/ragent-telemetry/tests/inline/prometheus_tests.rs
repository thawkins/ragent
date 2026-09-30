//! Inline tests for `prometheus.rs` - compiled inside the module tree
//! via the `#[path]` delegation left in the source file
//! (ANTIPAT M2 inline-test migration; no behaviour change).

use super::*;
use opentelemetry::metrics::MeterProvider;
use opentelemetry_sdk::Resource;

#[test]
fn test_escape_label_value() {
    assert_eq!(escape_label_value("simple"), "simple");
    assert_eq!(escape_label_value("has\"quote"), "has\\\"quote");
    assert_eq!(escape_label_value("back\\slash"), "back\\\\slash");
    assert_eq!(escape_label_value("multi\nline"), "multi\\nline");
}

#[test]
fn test_render_after_shutdown_returns_empty() {
    // A ManualReader with no registered provider -> collect fails -> empty.
    let reader = ManualReader::builder().build();
    let text = render_prometheus_text(&reader);
    assert_eq!(text, "", "unregistered reader should produce empty output");
}

#[test]
fn test_format_resource_metrics_with_resource() {
    let rm = ResourceMetrics {
        resource: Resource::builder_empty()
            .with_attribute(opentelemetry::KeyValue::new("service.name", "test-ragent"))
            .build(),
        scope_metrics: vec![],
    };
    let text = format_resource_metrics(&rm);
    assert!(
        text.contains("target_info"),
        "should contain target_info, got: {text}"
    );
    assert!(
        text.contains("service.name"),
        "should contain service.name label"
    );
}

#[test]
fn test_format_resource_metrics_empty() {
    // Use an explicitly-empty Resource (not Resource::default(), which
    // includes SDK defaults like telemetry.sdk.* and unknown_service).
    let rm = ResourceMetrics {
        resource: Resource::builder_empty().build(),
        scope_metrics: vec![],
    };
    let text = format_resource_metrics(&rm);
    // Empty resource -> no target_info line.
    assert!(!text.contains("target_info"));
}

#[test]
fn test_shared_manual_reader_delegates() {
    use opentelemetry_sdk::metrics::SdkMeterProvider;

    // SharedManualReader wraps an Arc<ManualReader> and delegates
    // MetricReader trait methods. We verify it can be registered on a
    // provider (which takes ownership) while we hold a handle, and
    // that calling `collect` on the handle returns a non-empty
    // snapshot (proving the delegation works end-to-end).
    let shared = SharedManualReader::new();
    let handle = shared.handle();

    let provider = SdkMeterProvider::builder()
        .with_resource(
            Resource::builder_empty()
                .with_attribute(opentelemetry::KeyValue::new("service.name", "ragent"))
                .build(),
        )
        .with_reader(shared) // ownership moves to the provider
        .build();

    // Record a metric.
    let meter = provider.meter("ragent");
    let counter = meter.u64_counter("ragent.llm.requests").build();
    counter.add(7, &[]);

    // Collect via the handle (the Arc<ManualReader> we kept).
    let mut rm = ResourceMetrics {
        resource: Resource::builder_empty().build(),
        scope_metrics: vec![],
    };
    assert!(
        handle.collect(&mut rm).is_ok(),
        "collect via the handle should succeed (delegation works)"
    );
    // The resource must be present (proving the reader is wired).
    assert!(
        rm.resource
            .get(&opentelemetry::Key::from("service.name"))
            .is_some(),
        "resource attributes must be collected via the handle"
    );
    // The counter must appear in the scope_metrics.
    let has_counter = rm
        .scope_metrics
        .iter()
        .flat_map(|sm| sm.metrics.iter())
        .any(|m| m.name == "ragent.llm.requests");
    assert!(
        has_counter,
        "the recorded counter must appear in the collected metrics"
    );

    // The renderer should also produce the metric name.
    let text = render_prometheus_text(&handle);
    assert!(
        text.contains("ragent.llm.requests"),
        "renderer should contain metric name, got: {text}"
    );
}

#[test]
fn test_build_labels() {
    let attrs = vec![
        opentelemetry::KeyValue::new("model", "claude"),
        opentelemetry::KeyValue::new("provider", "anthropic"),
    ];
    let labels = build_labels(&attrs);
    // Labels are sorted by key: model, provider.
    assert_eq!(labels, "{model=\"claude\",provider=\"anthropic\"}");
}

#[test]
fn test_build_labels_empty() {
    let attrs: Vec<opentelemetry::KeyValue> = vec![];
    let labels = build_labels(&attrs);
    assert_eq!(labels, "");
}

#[test]
fn test_append_le_label_empty() {
    let result = append_le_label("", "le=\"100\"");
    assert_eq!(result, "{le=\"100\"}");
}

#[test]
fn test_append_le_label_with_base() {
    let result = append_le_label("{model=\"claude\"}", "le=\"100\"");
    assert_eq!(result, "{model=\"claude\", le=\"100\"}");
}
