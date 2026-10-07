//! Inline tests for `instruments.rs` - compiled inside the module tree
//! via the `#[path]` delegation left in the source file
//! (ANTIPAT M2 inline-test migration; no behaviour change).

use super::*;
use crate::OtelConfig;
use opentelemetry_sdk::metrics::InMemoryMetricExporter;
use opentelemetry_sdk::metrics::SdkMeterProvider;
use std::time::Duration;

fn build_registry() -> InstrumentRegistry {
    let config = OtelConfig {
        enabled: true,
        endpoint: "http://localhost:4318".to_string(),
        ..Default::default()
    };

    let rt = tokio::runtime::Runtime::new().expect("tokio runtime");
    let sub =
        rt.block_on(async { crate::TelemetrySubsystem::new(config).expect("enabled subsystem") });
    InstrumentRegistry::from_provider(&sub.provider().unwrap())
}

/// Build an [`InstrumentRegistry`] backed by an [`SdkMeterProvider`] that
/// uses an [`InMemoryMetricExporter`] (NFR-005). Returns the registry, the
/// exporter, the provider, and the tokio runtime so callers can flush and
/// inspect the exported metric data.
fn build_registry_with_exporter() -> (
    InstrumentRegistry,
    InMemoryMetricExporter,
    SdkMeterProvider,
    tokio::runtime::Runtime,
) {
    let rt = tokio::runtime::Runtime::new().expect("tokio runtime");
    let exporter = InMemoryMetricExporter::default();
    let exporter_clone = exporter.clone();
    let provider = rt.block_on(async {
        let reader = opentelemetry_sdk::metrics::PeriodicReader::builder(exporter_clone)
            .with_interval(Duration::from_hours(1))
            .build();
        SdkMeterProvider::builder().with_reader(reader).build()
    });
    let registry = InstrumentRegistry::from_provider(&provider);
    (registry, exporter, provider, rt)
}

#[test]
fn test_registry_constructs_all_instruments() {
    // FR-003: the registry must register every metric in the catalog.
    let registry = build_registry();
    // Just accessing the fields proves they were constructed.
    let _ = &registry.llm_requests;
    let _ = &registry.sessions_active;
    let _ = &registry.team_members;
    let _ = &registry.llm_duration;
    let _ = &registry.tokens_input;
    let _ = &registry.cost_estimated;
    let _ = &registry.errors_total;
    let _ = &registry.context_compression_ratio;
}

#[test]
fn test_counter_can_add() {
    let (registry, exporter, provider, rt) = build_registry_with_exporter();
    registry.llm_requests.add(1, &[]);
    rt.block_on(async { provider.force_flush().expect("flush") });
    let metrics = exporter.get_finished_metrics().unwrap_or_default();
    assert!(
        !metrics.is_empty(),
        "counter add should produce a metric batch"
    );
}

#[test]
fn test_histogram_can_record() {
    let (registry, exporter, provider, rt) = build_registry_with_exporter();
    registry.llm_duration.record(42.0, &[]);
    rt.block_on(async { provider.force_flush().expect("flush") });
    let metrics = exporter.get_finished_metrics().unwrap_or_default();
    let has_histogram = metrics.iter().any(|rm| {
        rm.scope_metrics()
            .flat_map(|sm| sm.metrics())
            .any(|m| m.name() == "ragent.llm.duration")
    });
    assert!(
        has_histogram,
        "histogram record should produce ragent.llm.duration"
    );
}

#[test]
fn test_gauge_can_record() {
    let (registry, exporter, provider, rt) = build_registry_with_exporter();
    registry.team_members.record(3, &[]);
    rt.block_on(async { provider.force_flush().expect("flush") });
    let metrics = exporter.get_finished_metrics().unwrap_or_default();
    let has_gauge = metrics.iter().any(|rm| {
        rm.scope_metrics()
            .flat_map(|sm| sm.metrics())
            .any(|m| m.name() == "ragent.team.members")
    });
    assert!(has_gauge, "gauge record should produce ragent.team.members");
}

#[test]
fn test_up_down_counter_can_add() {
    let (registry, exporter, provider, rt) = build_registry_with_exporter();
    registry.sessions_active.add(1, &[]);
    registry.sessions_active.add(-1, &[]);
    rt.block_on(async { provider.force_flush().expect("flush") });
    let metrics = exporter.get_finished_metrics().unwrap_or_default();
    let has_up_down = metrics.iter().any(|rm| {
        rm.scope_metrics()
            .flat_map(|sm| sm.metrics())
            .any(|m| m.name() == "ragent.sessions.active")
    });
    assert!(
        has_up_down,
        "up-down counter add should produce ragent.sessions.active"
    );
}

#[test]
fn test_attrs_are_sanitised() {
    // FR-034: sensitive tool names should be redacted.
    let tool_attr = InstrumentRegistry::attr_tool("sk-secret-key");
    assert_eq!(tool_attr.value.to_string(), "redacted");
}

#[test]
fn test_metric_toggles_disable_metric() {
    // FR-027: a metric set to false should be disabled.
    let mut toggles = std::collections::HashMap::new();
    toggles.insert("ragent.llm.requests".to_string(), false);
    let registry = build_registry().with_metric_toggles(toggles);
    assert!(!registry.is_metric_enabled("ragent.llm.requests"));
    assert!(registry.is_metric_enabled("ragent.tool.invocations"));
}
