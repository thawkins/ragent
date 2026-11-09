//! Tests for profiler-bucket tagging on exported metrics.
//!
//! Covers `ragent.profile.duration` (every agent-loop profiler sample, tagged
//! with `profile.operation` / `profile.group` / `tool.name`) and the
//! `profile.bucket` attribute on the existing LLM and tool duration metrics.

#![cfg(feature = "telemetry")]

use std::collections::HashMap;
use std::time::Duration;

use opentelemetry_sdk::metrics::InMemoryMetricExporter;
use opentelemetry_sdk::metrics::SdkMeterProvider;
use opentelemetry_sdk::metrics::data::{AggregatedMetrics, MetricData, ResourceMetrics};
use ragent_telemetry::recorder::{LlmRecorder, ToolRecorder};
use ragent_telemetry::{InstrumentRegistry, OtelConfig, TelemetrySubsystem};

/// Build an enabled subsystem over an in-memory exporter.
fn subsystem_with_toggles(
    toggles: HashMap<String, bool>,
) -> (
    TelemetrySubsystem,
    InMemoryMetricExporter,
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
    let config = OtelConfig {
        metrics: toggles,
        ..OtelConfig::default()
    };
    (
        TelemetrySubsystem::from_provider(config, provider),
        exporter,
        rt,
    )
}

/// Flush the subsystem's provider and return the exported batches.
fn flush(
    sub: &TelemetrySubsystem,
    exporter: &InMemoryMetricExporter,
    rt: &tokio::runtime::Runtime,
) -> Vec<ResourceMetrics> {
    let provider = sub.provider().expect("live provider");
    rt.block_on(async { provider.force_flush().expect("flush") });
    exporter.get_finished_metrics().unwrap_or_default()
}

/// Attribute sets (as `key=value` strings) of every data point of the f64
/// histogram `name`.
fn histogram_points(metrics: &[ResourceMetrics], name: &str) -> Vec<Vec<String>> {
    let mut points = Vec::new();
    for rm in metrics {
        for sm in rm.scope_metrics() {
            for metric in sm.metrics().filter(|m| m.name() == name) {
                if let AggregatedMetrics::F64(MetricData::Histogram(hist)) = metric.data() {
                    for point in hist.data_points() {
                        let mut attrs: Vec<String> = point
                            .attributes()
                            .map(|kv| format!("{}={}", kv.key, kv.value))
                            .collect();
                        attrs.sort();
                        points.push(attrs);
                    }
                }
            }
        }
    }
    points
}

fn attrs(pairs: &[&str]) -> Vec<String> {
    let mut v: Vec<String> = pairs.iter().map(|s| (*s).to_string()).collect();
    v.sort();
    v
}

#[test]
fn test_profile_attrs_static_label() {
    let got: Vec<String> = InstrumentRegistry::profile_attrs("loop.llm.stream")
        .iter()
        .map(|kv| format!("{}={}", kv.key, kv.value))
        .collect();
    assert_eq!(
        got,
        vec![
            "profile.operation=loop.llm.stream",
            "profile.group=loop.llm"
        ]
    );
}

#[test]
fn test_profile_attrs_tool_label_adds_tool_name() {
    let got: Vec<String> = InstrumentRegistry::profile_attrs("tool.total:bash")
        .iter()
        .map(|kv| format!("{}={}", kv.key, kv.value))
        .collect();
    assert_eq!(
        got,
        vec![
            "profile.operation=tool.total:bash",
            "profile.group=tool",
            "tool.name=bash"
        ]
    );
}

#[test]
fn test_profile_attrs_undotted_label_is_its_own_group() {
    let got: Vec<String> = InstrumentRegistry::profile_attrs("startup")
        .iter()
        .map(|kv| format!("{}={}", kv.key, kv.value))
        .collect();
    assert_eq!(
        got,
        vec!["profile.operation=startup", "profile.group=startup"]
    );
}

#[test]
fn test_profile_attrs_rejects_unsafe_tool_suffix() {
    let got: Vec<String> = InstrumentRegistry::profile_attrs("tool.total:rm -rf /\nsecret")
        .iter()
        .map(|kv| format!("{}={}", kv.key, kv.value))
        .collect();
    assert!(got.contains(&"tool.name=unknown".to_string()), "{got:?}");
    assert!(got.iter().all(|a| !a.contains("rm -rf")), "{got:?}");
}

#[test]
fn test_profile_recorder_exports_tagged_samples() {
    let (sub, exporter, rt) = subsystem_with_toggles(HashMap::new());
    let recorder = sub
        .profile_recorder()
        .expect("enabled subsystem has recorder");
    recorder.record("loop.llm.stream", 12.5);
    recorder.record("tool.total:read", 3.0);

    let points = histogram_points(&flush(&sub, &exporter, &rt), "ragent.profile.duration");
    assert!(
        points.contains(&attrs(&[
            "profile.operation=loop.llm.stream",
            "profile.group=loop.llm"
        ])),
        "{points:?}"
    );
    assert!(
        points.contains(&attrs(&[
            "profile.operation=tool.total:read",
            "profile.group=tool",
            "tool.name=read"
        ])),
        "{points:?}"
    );
}

#[test]
fn test_profile_recorder_none_when_disabled() {
    assert!(TelemetrySubsystem::disabled().profile_recorder().is_none());
}

#[test]
fn test_profile_recorder_none_when_metric_toggled_off() {
    let toggles = HashMap::from([("ragent.profile.duration".to_string(), false)]);
    let (sub, _exporter, _rt) = subsystem_with_toggles(toggles);
    assert!(sub.profile_recorder().is_none());
}

#[test]
fn test_llm_and_tool_duration_carry_profile_bucket() {
    let (sub, exporter, rt) = subsystem_with_toggles(HashMap::new());
    let llm = LlmRecorder::from_subsystem(&sub);
    llm.record_duration("gpt-x", "openai", 100.0);
    llm.record_ttft("gpt-x", 10.0);
    ToolRecorder::from_subsystem(&sub).record_duration("bash", 5.0);

    let metrics = flush(&sub, &exporter, &rt);
    assert_eq!(
        histogram_points(&metrics, "ragent.llm.duration"),
        vec![attrs(&[
            "model=gpt-x",
            "provider=openai",
            "profile.bucket=loop.llm.total"
        ])]
    );
    assert_eq!(
        histogram_points(&metrics, "ragent.llm.time_to_first_token"),
        vec![attrs(&[
            "model=gpt-x",
            "profile.bucket=loop.llm.first_event_wait"
        ])]
    );
    assert_eq!(
        histogram_points(&metrics, "ragent.tool.duration"),
        vec![attrs(&["tool.name=bash", "profile.bucket=tool.total"])]
    );
}
