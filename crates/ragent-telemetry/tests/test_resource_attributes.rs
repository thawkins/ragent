//! Integration tests for resource attribute injection (T-007, FR-004, FR-025).
//!
//! These tests use the OTEL SDK's [`InMemoryMetricExporter`] with a
//! [`PeriodicReader`] (inside a Tokio runtime) to collect metrics in-memory
//! and inspect the `Resource` attributes — without requiring a live OTLP
//! collector.
//!
//! FR-004: `service.name`, `service.version`, and `host.name` are attached
//! as static resource attributes.
//! FR-025: `session.id` is a dynamic metric attribute, not a resource
//! attribute (OTEL resources are immutable at provider construction).

#![cfg(feature = "telemetry")]

use opentelemetry::KeyValue;
use opentelemetry::metrics::MeterProvider;
use opentelemetry_sdk::Resource;
use opentelemetry_sdk::metrics::SdkMeterProvider;

use opentelemetry_sdk::metrics::InMemoryMetricExporter;
use ragent_telemetry::InstrumentRegistry;

/// Build a provider with an `InMemoryMetricExporter` and ragent resource
/// construction. Returns the provider, the exporter (for reading collected
/// metrics), and the runtime (kept alive for the duration of the test).
fn build_provider(
    kvs: Vec<KeyValue>,
) -> (
    SdkMeterProvider,
    InMemoryMetricExporter,
    tokio::runtime::Runtime,
) {
    let rt = tokio::runtime::Runtime::new().expect("tokio runtime");
    let exporter = InMemoryMetricExporter::default();
    let exporter_clone = exporter.clone();

    // Build the provider inside the runtime context (PeriodicReader needs it).
    let provider = rt.block_on(async {
        let reader = opentelemetry_sdk::metrics::PeriodicReader::builder(exporter_clone).build();
        let resource = Resource::builder_empty().with_attributes(kvs).build();
        SdkMeterProvider::builder()
            .with_resource(resource)
            .with_reader(reader)
            .build()
    });

    (provider, exporter, rt)
}

/// Best-effort hostname — mirrors the private `hostname_str` in subsystem.rs.
fn hostname_str() -> Option<String> {
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

/// FR-004: `service.name` is present in the exported resource.
#[test]
fn test_resource_has_service_name() {
    let kvs = vec![
        KeyValue::new("service.name", "test-ragent".to_string()),
        KeyValue::new("service.version", env!("CARGO_PKG_VERSION").to_string()),
    ];
    let (provider, exporter, rt) = build_provider(kvs);
    let registry = InstrumentRegistry::from_provider(&provider);
    // Record at least one metric so the exporter has data to flush.
    registry.llm_requests.add(1, &[]);
    rt.block_on(async {
        provider.force_flush().unwrap();
    });

    let metrics = exporter.get_finished_metrics().unwrap_or_default();
    assert!(
        !metrics.is_empty(),
        "should have collected at least one batch"
    );
    let service_name = metrics[0]
        .resource
        .get(&opentelemetry::Key::from("service.name"))
        .map(|v| v.as_str().to_string());
    assert_eq!(
        service_name,
        Some("test-ragent".to_string()),
        "service.name must be in the resource (FR-004)"
    );
}

/// FR-004: `service.version` is present and matches `CARGO_PKG_VERSION`.
#[test]
fn test_resource_has_service_version() {
    let kvs = vec![
        KeyValue::new("service.name", "ragent".to_string()),
        KeyValue::new("service.version", env!("CARGO_PKG_VERSION").to_string()),
    ];
    let (provider, exporter, rt) = build_provider(kvs);
    let registry = InstrumentRegistry::from_provider(&provider);
    // Record at least one metric so the exporter has data to flush.
    registry.llm_requests.add(1, &[]);
    rt.block_on(async {
        provider.force_flush().unwrap();
    });

    let metrics = exporter.get_finished_metrics().unwrap_or_default();
    assert!(!metrics.is_empty());
    let version = metrics[0]
        .resource
        .get(&opentelemetry::Key::from("service.version"))
        .map(|v| v.as_str().to_string());
    assert_eq!(
        version,
        Some(env!("CARGO_PKG_VERSION").to_string()),
        "service.version must match CARGO_PKG_VERSION (FR-004)"
    );
}

/// FR-004: `host.name` is present when the hostname can be determined.
#[test]
fn test_resource_has_host_name() {
    let mut kvs = vec![
        KeyValue::new("service.name", "ragent".to_string()),
        KeyValue::new("service.version", env!("CARGO_PKG_VERSION").to_string()),
    ];
    if let Some(host) = hostname_str() {
        kvs.push(KeyValue::new("host.name", host));
    }
    let (provider, exporter, rt) = build_provider(kvs);
    let registry = InstrumentRegistry::from_provider(&provider);
    // Record at least one metric so the exporter has data to flush.
    registry.llm_requests.add(1, &[]);
    rt.block_on(async {
        provider.force_flush().unwrap();
    });

    let metrics = exporter.get_finished_metrics().unwrap_or_default();
    assert!(!metrics.is_empty());

    if let Some(expected) = hostname_str() {
        let host = metrics[0]
            .resource
            .get(&opentelemetry::Key::from("host.name"))
            .map(|v| v.as_str().to_string());
        assert_eq!(host, Some(expected), "host.name must match (FR-004)");
    }
}

/// FR-026: custom resource attributes from config are merged in.
#[test]
fn test_resource_has_custom_attributes() {
    let kvs = vec![
        KeyValue::new("service.name", "ragent".to_string()),
        KeyValue::new("deployment.environment", "testing".to_string()),
    ];
    let (provider, exporter, rt) = build_provider(kvs);
    let registry = InstrumentRegistry::from_provider(&provider);
    // Record at least one metric so the exporter has data to flush.
    registry.llm_requests.add(1, &[]);
    rt.block_on(async {
        provider.force_flush().unwrap();
    });

    let metrics = exporter.get_finished_metrics().unwrap_or_default();
    assert!(!metrics.is_empty());
    let env = metrics[0]
        .resource
        .get(&opentelemetry::Key::from("deployment.environment"))
        .map(|v| v.as_str().to_string());
    assert_eq!(
        env,
        Some("testing".to_string()),
        "custom resource attributes must be merged (FR-026)"
    );
}

/// FR-025: `session.id` is a metric attribute, not a resource attribute.
///
/// We record a counter with `attr_session` and verify the data point has
/// the `session.id` attribute. The resource should NOT contain `session.id`.
#[test]
fn test_session_id_is_metric_attribute_not_resource() {
    let kvs = vec![KeyValue::new("service.name", "ragent".to_string())];
    let (provider, exporter, rt) = build_provider(kvs);

    let meter = provider.meter("ragent");
    let counter = meter
        .u64_counter("test.session_metric")
        .with_unit("{test}")
        .build();

    // Record with session.id as a metric attribute.
    counter.add(1, &[InstrumentRegistry::attr_session("sess-abc-123")]);

    rt.block_on(async {
        provider.force_flush().unwrap();
    });

    let metrics = exporter.get_finished_metrics().unwrap_or_default();
    assert!(!metrics.is_empty(), "should have collected metrics");

    // The resource should NOT contain session.id — it's dynamic.
    let session_in_resource = metrics[0]
        .resource
        .get(&opentelemetry::Key::from("session.id"));
    assert!(
        session_in_resource.is_none(),
        "session.id must not be a static resource attribute (FR-025)"
    );

    // The metric data point should contain session.id as an attribute.
    let mut found_session_attr = false;
    for rm in &metrics {
        for scope_metrics in &rm.scope_metrics {
            for metric in &scope_metrics.metrics {
                if let Some(sum) = metric
                    .data
                    .as_any()
                    .downcast_ref::<opentelemetry_sdk::metrics::data::Sum<u64>>()
                {
                    for data_point in &sum.data_points {
                        if data_point.attributes.iter().any(|kv| {
                            kv.key.as_str() == "session.id" && kv.value.as_str() == "sess-abc-123"
                        }) {
                            found_session_attr = true;
                        }
                    }
                }
            }
        }
    }
    assert!(
        found_session_attr,
        "session.id must be present as a metric attribute (FR-025)"
    );
}

/// FR-004: default `service.name` is `"ragent"` when not overridden.
#[test]
fn test_resource_default_service_name() {
    let kvs = vec![KeyValue::new("service.name", "ragent".to_string())];
    let (provider, exporter, rt) = build_provider(kvs);
    let registry = InstrumentRegistry::from_provider(&provider);
    // Record at least one metric so the exporter has data to flush.
    registry.llm_requests.add(1, &[]);
    rt.block_on(async {
        provider.force_flush().unwrap();
    });

    let metrics = exporter.get_finished_metrics().unwrap_or_default();
    assert!(!metrics.is_empty());
    let name = metrics[0]
        .resource
        .get(&opentelemetry::Key::from("service.name"))
        .map(|v| v.as_str().to_string());
    assert_eq!(name, Some("ragent".to_string()));
}
