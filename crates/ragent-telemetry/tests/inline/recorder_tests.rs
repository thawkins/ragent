//! Inline tests for `recorder.rs` - compiled inside the module tree
//! via the `#[path]` delegation left in the source file
//! (ANTIPAT M2 inline-test migration; no behaviour change).

use super::*;
use opentelemetry::KeyValue;
use opentelemetry_sdk::metrics::InMemoryMetricExporter;
use opentelemetry_sdk::metrics::SdkMeterProvider;
use std::time::Duration;

fn build_provider() -> (
    SdkMeterProvider,
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
    (provider, exporter, rt)
}

#[test]
fn test_disabled_recorder_is_noop() {
    let rec = LlmRecorder::disabled();
    assert!(!rec.is_enabled());
    rec.record_request("gpt-4", "openai");
    rec.record_usage("gpt-4", "openai", 100, 50);
    rec.record_cost("gpt-4", "openai", 0.001);
    rec.record_duration("gpt-4", "openai", 500.0);
    rec.record_ttft("gpt-4", 200.0);
    rec.record_rate_limit("openai", Some(50.0), None);
}

#[test]
fn test_record_rate_limit_updates_gauges() {
    let (provider, exporter, rt) = build_provider();
    let registry = InstrumentRegistry::from_provider(&provider);
    let rec = LlmRecorder {
        registry: Some(registry),
    };

    rec.record_rate_limit("openai", Some(75.0), Some(40.0));
    // None values should not panic and should not record.
    rec.record_rate_limit("anthropic", None, None);

    rt.block_on(async {
        provider.force_flush().expect("flush");
    });

    let metrics = exporter.get_finished_metrics().unwrap_or_default();

    let requests_pct: Option<f64> = metrics
        .iter()
        .flat_map(|rm| rm.scope_metrics.iter())
        .flat_map(|sm| sm.metrics.iter())
        .filter(|m| m.name == "ragent.rate_limit.requests_pct")
        .filter_map(|m| {
            m.data
                .as_any()
                .downcast_ref::<opentelemetry_sdk::metrics::data::Gauge<f64>>()
        })
        .flat_map(|g| g.data_points.iter())
        .find(|dp| {
            dp.attributes
                .iter()
                .any(|kv| kv.key.as_str() == "provider" && kv.value.as_str() == "openai")
        })
        .map(|dp| dp.value);

    let tokens_pct: Option<f64> = metrics
        .iter()
        .flat_map(|rm| rm.scope_metrics.iter())
        .flat_map(|sm| sm.metrics.iter())
        .filter(|m| m.name == "ragent.rate_limit.tokens_pct")
        .filter_map(|m| {
            m.data
                .as_any()
                .downcast_ref::<opentelemetry_sdk::metrics::data::Gauge<f64>>()
        })
        .flat_map(|g| g.data_points.iter())
        .find(|dp| {
            dp.attributes
                .iter()
                .any(|kv| kv.key.as_str() == "provider" && kv.value.as_str() == "openai")
        })
        .map(|dp| dp.value);

    assert!(
        requests_pct.is_some(),
        "ragent.rate_limit.requests_pct should have a data point for openai"
    );
    assert!((requests_pct.unwrap() - 75.0).abs() < 1e-6);
    assert!(
        tokens_pct.is_some(),
        "ragent.rate_limit.tokens_pct should have a data point for openai"
    );
    assert!((tokens_pct.unwrap() - 40.0).abs() < 1e-6);
}

#[test]
fn test_compute_cost_usd_formula() {
    // 1M input tokens at $3.00/M -> $3.00
    let cost = ragent_config::Cost {
        input: 3.0,
        output: 15.0,
    };
    let total = compute_cost_usd(1_000_000, 0, &cost);
    assert!(
        (total - 3.0).abs() < 1e-9,
        "1M input @ $3/M = $3, got {total}"
    );

    // 1M output tokens at $15.00/M -> $15.00
    let total = compute_cost_usd(0, 1_000_000, &cost);
    assert!(
        (total - 15.0).abs() < 1e-9,
        "1M output @ $15/M = $15, got {total}"
    );

    // Mixed: 500K input + 200K output -> 1.5 + 3.0 = 4.5
    let total = compute_cost_usd(500_000, 200_000, &cost);
    assert!(
        (total - 4.5).abs() < 1e-9,
        "500K in + 200K out = $4.5, got {total}"
    );

    // Zero tokens -> zero cost
    let total = compute_cost_usd(0, 0, &cost);
    assert!(total.abs() < 1e-9, "zero tokens = $0, got {total}");
}

#[test]
fn test_compute_cost_usd_default_cost() {
    // Default Cost is all-zero, so any token count costs nothing.
    let cost = ragent_config::Cost::default();
    let total = compute_cost_usd(1_000_000, 1_000_000, &cost);
    assert!(total.abs() < 1e-9, "default zero cost = $0, got {total}");
}

#[test]
fn test_record_cost_increments_counter() {
    let (provider, exporter, rt) = build_provider();
    let registry = InstrumentRegistry::from_provider(&provider);
    let rec = LlmRecorder {
        registry: Some(registry),
    };

    rec.record_cost("gpt-4", "openai", 1.5);
    rec.record_cost("gpt-4", "openai", 2.5);

    rt.block_on(async {
        provider.force_flush().expect("flush");
    });

    let metrics = exporter.get_finished_metrics().unwrap_or_default();
    let cost_sum: f64 = metrics
        .iter()
        .flat_map(|rm| rm.scope_metrics.iter())
        .flat_map(|sm| sm.metrics.iter())
        .filter(|m| m.name == "ragent.cost.estimated")
        .filter_map(|m| {
            m.data
                .as_any()
                .downcast_ref::<opentelemetry_sdk::metrics::data::Sum<f64>>()
        })
        .flat_map(|sum| sum.data_points.iter())
        .map(|dp| dp.value)
        .sum();

    assert!(
        (cost_sum - 4.0).abs() < 1e-9,
        "two record_cost calls (1.5 + 2.5) should sum to 4.0, got {cost_sum}"
    );
}

#[test]
fn test_record_request_increments_counter() {
    let (provider, exporter, rt) = build_provider();
    let registry = InstrumentRegistry::from_provider(&provider);
    let rec = LlmRecorder {
        registry: Some(registry),
    };

    rec.record_request("gpt-4", "openai");
    rec.record_request("gpt-4", "openai");
    rec.record_request("claude-3", "anthropic");

    rt.block_on(async {
        provider.force_flush().expect("flush");
    });

    let metrics = exporter.get_finished_metrics().unwrap_or_default();
    assert!(!metrics.is_empty());

    let llm_requests = metrics.iter().flat_map(|rm| {
        rm.scope_metrics
            .iter()
            .flat_map(|sm| sm.metrics.iter())
            .filter(|m| m.name == "ragent.llm.requests")
    });
    let mut total: u64 = 0;
    for m in llm_requests {
        if let Some(sum) = m
            .data
            .as_any()
            .downcast_ref::<opentelemetry_sdk::metrics::data::Sum<u64>>()
        {
            total += sum.data_points.iter().map(|dp| dp.value).sum::<u64>();
        }
    }
    assert_eq!(total, 3, "should have recorded 3 LLM requests");
}

#[test]
fn test_record_usage_increments_token_counters() {
    let (provider, exporter, rt) = build_provider();
    let registry = InstrumentRegistry::from_provider(&provider);
    let rec = LlmRecorder {
        registry: Some(registry),
    };

    rec.record_usage("gpt-4", "openai", 500, 200);

    rt.block_on(async {
        provider.force_flush().expect("flush");
    });

    let metrics = exporter.get_finished_metrics().unwrap_or_default();
    let has_input = metrics.iter().any(|rm| {
        rm.scope_metrics
            .iter()
            .flat_map(|sm| sm.metrics.iter())
            .any(|m| m.name == "ragent.tokens.input")
    });
    let has_output = metrics.iter().any(|rm| {
        rm.scope_metrics
            .iter()
            .flat_map(|sm| sm.metrics.iter())
            .any(|m| m.name == "ragent.tokens.output")
    });
    assert!(has_input, "should have ragent.tokens.input");
    assert!(has_output, "should have ragent.tokens.output");
}

#[test]
fn test_record_duration_records_histogram() {
    let (provider, exporter, rt) = build_provider();
    let registry = InstrumentRegistry::from_provider(&provider);
    let rec = LlmRecorder {
        registry: Some(registry),
    };

    rec.record_duration("gpt-4", "openai", 1234.5);

    rt.block_on(async {
        provider.force_flush().expect("flush");
    });

    let metrics = exporter.get_finished_metrics().unwrap_or_default();
    let has_duration = metrics.iter().any(|rm| {
        rm.scope_metrics
            .iter()
            .flat_map(|sm| sm.metrics.iter())
            .any(|m| m.name == "ragent.llm.duration")
    });
    assert!(has_duration, "should have ragent.llm.duration");
}

#[test]
fn test_record_ttft_records_histogram() {
    let (provider, exporter, rt) = build_provider();
    let registry = InstrumentRegistry::from_provider(&provider);
    let rec = LlmRecorder {
        registry: Some(registry),
    };

    rec.record_ttft("gpt-4", 150.0);

    rt.block_on(async {
        provider.force_flush().expect("flush");
    });

    let metrics = exporter.get_finished_metrics().unwrap_or_default();
    let has_ttft = metrics.iter().any(|rm| {
        rm.scope_metrics
            .iter()
            .flat_map(|sm| sm.metrics.iter())
            .any(|m| m.name == "ragent.llm.time_to_first_token")
    });
    assert!(has_ttft, "should have ragent.llm.time_to_first_token");
}

#[test]
fn test_attributes_include_model_and_provider() {
    let (provider, exporter, rt) = build_provider();
    let registry = InstrumentRegistry::from_provider(&provider);
    let rec = LlmRecorder {
        registry: Some(registry),
    };

    rec.record_request("gpt-4", "openai");

    rt.block_on(async {
        provider.force_flush().expect("flush");
    });

    let metrics = exporter.get_finished_metrics().unwrap_or_default();
    let has_attrs = metrics.iter().any(|rm| {
        rm.scope_metrics
            .iter()
            .flat_map(|sm| sm.metrics.iter())
            .filter(|m| m.name == "ragent.llm.requests")
            .flat_map(|m| {
                if let Some(sum) = m
                    .data
                    .as_any()
                    .downcast_ref::<opentelemetry_sdk::metrics::data::Sum<u64>>()
                {
                    sum.data_points.to_vec()
                } else {
                    Vec::new()
                }
            })
            .any(|dp| {
                dp.attributes
                    .iter()
                    .any(|kv| kv.key.as_str() == "model" && kv.value.as_str() == "gpt-4")
                    && dp
                        .attributes
                        .iter()
                        .any(|kv| kv.key.as_str() == "provider" && kv.value.as_str() == "openai")
            })
    });
    assert!(
        has_attrs,
        "metrics should have model and provider attributes"
    );
    // Suppress unused import warning
    let _ = KeyValue::new("test", "value");
}

#[test]
fn test_disabled_tool_recorder_is_noop() {
    let rec = ToolRecorder::disabled();
    assert!(!rec.is_enabled());
    rec.record_invocation("read");
    rec.record_duration("read", 42.0);
}

#[test]
fn test_tool_recorder_record_invocation_increments_counter() {
    let (provider, exporter, rt) = build_provider();
    let registry = InstrumentRegistry::from_provider(&provider);
    let rec = ToolRecorder {
        registry: Some(registry),
    };

    rec.record_invocation("read");
    rec.record_invocation("read");
    rec.record_invocation("write");

    rt.block_on(async {
        provider.force_flush().expect("flush");
    });

    let metrics = exporter.get_finished_metrics().unwrap_or_default();
    let total: u64 = metrics
        .iter()
        .flat_map(|rm| rm.scope_metrics.iter())
        .flat_map(|sm| sm.metrics.iter())
        .filter(|m| m.name == "ragent.tool.invocations")
        .filter_map(|m| {
            m.data
                .as_any()
                .downcast_ref::<opentelemetry_sdk::metrics::data::Sum<u64>>()
        })
        .flat_map(|sum| sum.data_points.iter())
        .map(|dp| dp.value)
        .sum();
    assert_eq!(total, 3, "should have recorded 3 tool invocations");
}

#[test]
fn test_tool_recorder_record_duration_records_histogram() {
    let (provider, exporter, rt) = build_provider();
    let registry = InstrumentRegistry::from_provider(&provider);
    let rec = ToolRecorder {
        registry: Some(registry),
    };

    rec.record_duration("read", 1234.5);

    rt.block_on(async {
        provider.force_flush().expect("flush");
    });

    let metrics = exporter.get_finished_metrics().unwrap_or_default();
    let has_duration = metrics.iter().any(|rm| {
        rm.scope_metrics
            .iter()
            .flat_map(|sm| sm.metrics.iter())
            .any(|m| m.name == "ragent.tool.duration")
    });
    assert!(has_duration, "should have ragent.tool.duration");
}

#[test]
fn test_tool_recorder_attributes_include_tool_name() {
    let (provider, exporter, rt) = build_provider();
    let registry = InstrumentRegistry::from_provider(&provider);
    let rec = ToolRecorder {
        registry: Some(registry),
    };

    rec.record_invocation("read");

    rt.block_on(async {
        provider.force_flush().expect("flush");
    });

    let metrics = exporter.get_finished_metrics().unwrap_or_default();
    let has_attrs = metrics.iter().any(|rm| {
        rm.scope_metrics
            .iter()
            .flat_map(|sm| sm.metrics.iter())
            .filter(|m| m.name == "ragent.tool.invocations")
            .flat_map(|m| {
                if let Some(sum) = m
                    .data
                    .as_any()
                    .downcast_ref::<opentelemetry_sdk::metrics::data::Sum<u64>>()
                {
                    sum.data_points.to_vec()
                } else {
                    Vec::new()
                }
            })
            .any(|dp| {
                dp.attributes
                    .iter()
                    .any(|kv| kv.key.as_str() == "tool.name" && kv.value.as_str() == "read")
            })
    });
    assert!(has_attrs, "tool metrics should have tool.name attribute");
}

#[test]
fn test_tool_recorder_is_clone() {
    let (provider, _exporter, _rt) = build_provider();
    let registry = InstrumentRegistry::from_provider(&provider);
    let rec = ToolRecorder {
        registry: Some(registry),
    };
    let rec2 = rec.clone();
    assert!(rec.is_enabled());
    assert!(rec2.is_enabled());
    rec2.record_invocation("read");
}

#[test]
fn test_disabled_session_recorder_is_noop() {
    let rec = SessionRecorder::disabled();
    assert!(!rec.is_enabled());
    rec.record_session_start();
    rec.record_agent_loop(500.0, 10);
    rec.record_session_end();
}

#[test]
fn test_session_recorder_record_session_start_increments_counters() {
    let (provider, exporter, rt) = build_provider();
    let registry = InstrumentRegistry::from_provider(&provider);
    let rec = SessionRecorder {
        registry: Some(registry),
    };

    rec.record_session_start();
    rec.record_session_start();

    rt.block_on(async {
        provider.force_flush().expect("flush");
    });

    let metrics = exporter.get_finished_metrics().unwrap_or_default();

    let active: i64 = metrics
        .iter()
        .flat_map(|rm| rm.scope_metrics.iter())
        .flat_map(|sm| sm.metrics.iter())
        .filter(|m| m.name == "ragent.sessions.active")
        .filter_map(|m| {
            m.data
                .as_any()
                .downcast_ref::<opentelemetry_sdk::metrics::data::Sum<i64>>()
        })
        .flat_map(|sum| sum.data_points.iter())
        .map(|dp| dp.value)
        .sum();
    assert_eq!(active, 2, "sessions.active should be 2 after two starts");

    let total: u64 = metrics
        .iter()
        .flat_map(|rm| rm.scope_metrics.iter())
        .flat_map(|sm| sm.metrics.iter())
        .filter(|m| m.name == "ragent.sessions.total")
        .filter_map(|m| {
            m.data
                .as_any()
                .downcast_ref::<opentelemetry_sdk::metrics::data::Sum<u64>>()
        })
        .flat_map(|sum| sum.data_points.iter())
        .map(|dp| dp.value)
        .sum();
    assert_eq!(total, 2, "sessions.total should be 2 after two starts");
}

#[test]
fn test_session_recorder_record_session_end_decrements_active() {
    let (provider, exporter, rt) = build_provider();
    let registry = InstrumentRegistry::from_provider(&provider);
    let rec = SessionRecorder {
        registry: Some(registry),
    };

    rec.record_session_start();
    rec.record_session_start();
    rec.record_session_end();

    rt.block_on(async {
        provider.force_flush().expect("flush");
    });

    let metrics = exporter.get_finished_metrics().unwrap_or_default();
    let active: i64 = metrics
        .iter()
        .flat_map(|rm| rm.scope_metrics.iter())
        .flat_map(|sm| sm.metrics.iter())
        .filter(|m| m.name == "ragent.sessions.active")
        .filter_map(|m| {
            m.data
                .as_any()
                .downcast_ref::<opentelemetry_sdk::metrics::data::Sum<i64>>()
        })
        .flat_map(|sum| sum.data_points.iter())
        .map(|dp| dp.value)
        .sum();
    assert_eq!(
        active, 1,
        "sessions.active should be 1 after 2 starts and 1 end"
    );
}

#[test]
fn test_session_recorder_record_agent_loop_records_histograms() {
    let (provider, exporter, rt) = build_provider();
    let registry = InstrumentRegistry::from_provider(&provider);
    let rec = SessionRecorder {
        registry: Some(registry),
    };

    rec.record_agent_loop(12345.6, 5);

    rt.block_on(async {
        provider.force_flush().expect("flush");
    });

    let metrics = exporter.get_finished_metrics().unwrap_or_default();
    let has_duration = metrics.iter().any(|rm| {
        rm.scope_metrics
            .iter()
            .flat_map(|sm| sm.metrics.iter())
            .any(|m| m.name == "ragent.agent_loop.duration")
    });
    let has_iterations = metrics.iter().any(|rm| {
        rm.scope_metrics
            .iter()
            .flat_map(|sm| sm.metrics.iter())
            .any(|m| m.name == "ragent.agent_loop.iterations")
    });
    assert!(has_duration, "should have ragent.agent_loop.duration");
    assert!(has_iterations, "should have ragent.agent_loop.iterations");
}

#[test]
fn test_disabled_coordinator_recorder_is_noop() {
    let rec = CoordinatorRecorder::disabled();
    assert!(!rec.is_enabled());
    rec.record_agent_spawn();
    rec.record_agent_complete();
    rec.record_error("coordinator");
    rec.record_timeout();
}

#[test]
fn test_coordinator_recorder_record_agent_spawn() {
    let (provider, exporter, rt) = build_provider();
    let registry = InstrumentRegistry::from_provider(&provider);
    let rec = CoordinatorRecorder {
        registry: Some(registry),
    };

    rec.record_agent_spawn();
    rec.record_agent_spawn();

    rt.block_on(async {
        provider.force_flush().expect("flush");
    });

    let metrics = exporter.get_finished_metrics().unwrap_or_default();

    let spawns: u64 = metrics
        .iter()
        .flat_map(|rm| rm.scope_metrics.iter())
        .flat_map(|sm| sm.metrics.iter())
        .filter(|m| m.name == "ragent.subagent.spawns")
        .filter_map(|m| {
            m.data
                .as_any()
                .downcast_ref::<opentelemetry_sdk::metrics::data::Sum<u64>>()
        })
        .flat_map(|sum| sum.data_points.iter())
        .map(|dp| dp.value)
        .sum();
    assert_eq!(spawns, 2, "subagent.spawns should be 2");

    let active: i64 = metrics
        .iter()
        .flat_map(|rm| rm.scope_metrics.iter())
        .flat_map(|sm| sm.metrics.iter())
        .filter(|m| m.name == "ragent.agents.active")
        .filter_map(|m| {
            m.data
                .as_any()
                .downcast_ref::<opentelemetry_sdk::metrics::data::Sum<i64>>()
        })
        .flat_map(|sum| sum.data_points.iter())
        .map(|dp| dp.value)
        .sum();
    assert_eq!(active, 2, "agents.active should be 2 after two spawns");
}

#[test]
fn test_coordinator_recorder_record_agent_complete() {
    let (provider, exporter, rt) = build_provider();
    let registry = InstrumentRegistry::from_provider(&provider);
    let rec = CoordinatorRecorder {
        registry: Some(registry),
    };

    rec.record_agent_spawn();
    rec.record_agent_spawn();
    rec.record_agent_complete();

    rt.block_on(async {
        provider.force_flush().expect("flush");
    });

    let metrics = exporter.get_finished_metrics().unwrap_or_default();

    let active: i64 = metrics
        .iter()
        .flat_map(|rm| rm.scope_metrics.iter())
        .flat_map(|sm| sm.metrics.iter())
        .filter(|m| m.name == "ragent.agents.active")
        .filter_map(|m| {
            m.data
                .as_any()
                .downcast_ref::<opentelemetry_sdk::metrics::data::Sum<i64>>()
        })
        .flat_map(|sum| sum.data_points.iter())
        .map(|dp| dp.value)
        .sum();
    assert_eq!(
        active, 1,
        "agents.active should be 1 after 2 spawns and 1 complete"
    );

    let completed: u64 = metrics
        .iter()
        .flat_map(|rm| rm.scope_metrics.iter())
        .flat_map(|sm| sm.metrics.iter())
        .filter(|m| m.name == "ragent.agents.completed")
        .filter_map(|m| {
            m.data
                .as_any()
                .downcast_ref::<opentelemetry_sdk::metrics::data::Sum<u64>>()
        })
        .flat_map(|sum| sum.data_points.iter())
        .map(|dp| dp.value)
        .sum();
    assert_eq!(completed, 1, "agents.completed should be 1");
}

#[test]
fn test_coordinator_recorder_record_error() {
    let (provider, exporter, rt) = build_provider();
    let registry = InstrumentRegistry::from_provider(&provider);
    let rec = CoordinatorRecorder {
        registry: Some(registry),
    };

    rec.record_error("coordinator");
    rec.record_error("tool");

    rt.block_on(async {
        provider.force_flush().expect("flush");
    });

    let metrics = exporter.get_finished_metrics().unwrap_or_default();
    let total: u64 = metrics
        .iter()
        .flat_map(|rm| rm.scope_metrics.iter())
        .flat_map(|sm| sm.metrics.iter())
        .filter(|m| m.name == "ragent.errors.total")
        .filter_map(|m| {
            m.data
                .as_any()
                .downcast_ref::<opentelemetry_sdk::metrics::data::Sum<u64>>()
        })
        .flat_map(|sum| sum.data_points.iter())
        .map(|dp| dp.value)
        .sum();
    assert_eq!(total, 2, "errors.total should be 2");

    let has_component = metrics.iter().any(|rm| {
        rm.scope_metrics
            .iter()
            .flat_map(|sm| sm.metrics.iter())
            .filter(|m| m.name == "ragent.errors.total")
            .flat_map(|m| {
                if let Some(sum) = m
                    .data
                    .as_any()
                    .downcast_ref::<opentelemetry_sdk::metrics::data::Sum<u64>>()
                {
                    sum.data_points.to_vec()
                } else {
                    Vec::new()
                }
            })
            .any(|dp| {
                dp.attributes
                    .iter()
                    .any(|kv| kv.key.as_str() == "component" && kv.value.as_str() == "coordinator")
            })
    });
    assert!(
        has_component,
        "errors.total should have component=coordinator attribute"
    );
}

#[test]
fn test_coordinator_recorder_record_timeout() {
    let (provider, exporter, rt) = build_provider();
    let registry = InstrumentRegistry::from_provider(&provider);
    let rec = CoordinatorRecorder {
        registry: Some(registry),
    };

    rec.record_timeout();
    rec.record_timeout();

    rt.block_on(async {
        provider.force_flush().expect("flush");
    });

    let metrics = exporter.get_finished_metrics().unwrap_or_default();
    let total: u64 = metrics
        .iter()
        .flat_map(|rm| rm.scope_metrics.iter())
        .flat_map(|sm| sm.metrics.iter())
        .filter(|m| m.name == "ragent.timeouts.total")
        .filter_map(|m| {
            m.data
                .as_any()
                .downcast_ref::<opentelemetry_sdk::metrics::data::Sum<u64>>()
        })
        .flat_map(|sum| sum.data_points.iter())
        .map(|dp| dp.value)
        .sum();
    assert_eq!(total, 2, "timeouts.total should be 2");
}

// ── PermissionRecorder tests (T-016, FR-016) ──────────────────────────

#[test]
fn test_disabled_permission_recorder_is_noop() {
    let rec = PermissionRecorder::disabled();
    assert!(!rec.is_enabled());
    rec.record_approved("bash");
    rec.record_denied("edit");
}

#[test]
fn test_permission_recorder_record_approved_increments_counter() {
    let (provider, exporter, rt) = build_provider();
    let registry = InstrumentRegistry::from_provider(&provider);
    let rec = PermissionRecorder {
        registry: Some(registry),
    };

    rec.record_approved("bash");
    rec.record_approved("bash");
    rec.record_approved("edit");

    rt.block_on(async {
        provider.force_flush().expect("flush");
    });

    let metrics = exporter.get_finished_metrics().unwrap_or_default();
    let total: u64 = metrics
        .iter()
        .flat_map(|rm| rm.scope_metrics.iter())
        .flat_map(|sm| sm.metrics.iter())
        .filter(|m| m.name == "ragent.permission.approved")
        .filter_map(|m| {
            m.data
                .as_any()
                .downcast_ref::<opentelemetry_sdk::metrics::data::Sum<u64>>()
        })
        .flat_map(|sum| sum.data_points.iter())
        .map(|dp| dp.value)
        .sum();
    assert_eq!(total, 3, "permission.approved should be 3");
}

#[test]
fn test_permission_recorder_record_denied_increments_counter() {
    let (provider, exporter, rt) = build_provider();
    let registry = InstrumentRegistry::from_provider(&provider);
    let rec = PermissionRecorder {
        registry: Some(registry),
    };

    rec.record_denied("bash");
    rec.record_denied("edit");
    rec.record_denied("edit");

    rt.block_on(async {
        provider.force_flush().expect("flush");
    });

    let metrics = exporter.get_finished_metrics().unwrap_or_default();
    let total: u64 = metrics
        .iter()
        .flat_map(|rm| rm.scope_metrics.iter())
        .flat_map(|sm| sm.metrics.iter())
        .filter(|m| m.name == "ragent.permission.denied")
        .filter_map(|m| {
            m.data
                .as_any()
                .downcast_ref::<opentelemetry_sdk::metrics::data::Sum<u64>>()
        })
        .flat_map(|sum| sum.data_points.iter())
        .map(|dp| dp.value)
        .sum();
    assert_eq!(total, 3, "permission.denied should be 3");
}

#[test]
fn test_permission_recorder_attributes_include_tool_name() {
    let (provider, exporter, rt) = build_provider();
    let registry = InstrumentRegistry::from_provider(&provider);
    let rec = PermissionRecorder {
        registry: Some(registry),
    };

    rec.record_approved("bash");
    rec.record_denied("edit");

    rt.block_on(async {
        provider.force_flush().expect("flush");
    });

    let metrics = exporter.get_finished_metrics().unwrap_or_default();

    // Check approved has tool.name=bash
    let has_bash = metrics.iter().any(|rm| {
        rm.scope_metrics
            .iter()
            .flat_map(|sm| sm.metrics.iter())
            .filter(|m| m.name == "ragent.permission.approved")
            .flat_map(|m| {
                if let Some(sum) = m
                    .data
                    .as_any()
                    .downcast_ref::<opentelemetry_sdk::metrics::data::Sum<u64>>()
                {
                    sum.data_points.to_vec()
                } else {
                    Vec::new()
                }
            })
            .any(|dp| {
                dp.attributes
                    .iter()
                    .any(|kv| kv.key.as_str() == "tool.name" && kv.value.as_str() == "bash")
            })
    });
    assert!(
        has_bash,
        "permission.approved should have tool.name=bash attribute"
    );

    // Check denied has tool.name=edit
    let has_edit = metrics.iter().any(|rm| {
        rm.scope_metrics
            .iter()
            .flat_map(|sm| sm.metrics.iter())
            .filter(|m| m.name == "ragent.permission.denied")
            .flat_map(|m| {
                if let Some(sum) = m
                    .data
                    .as_any()
                    .downcast_ref::<opentelemetry_sdk::metrics::data::Sum<u64>>()
                {
                    sum.data_points.to_vec()
                } else {
                    Vec::new()
                }
            })
            .any(|dp| {
                dp.attributes
                    .iter()
                    .any(|kv| kv.key.as_str() == "tool.name" && kv.value.as_str() == "edit")
            })
    });
    assert!(
        has_edit,
        "permission.denied should have tool.name=edit attribute"
    );
}

#[test]
fn test_permission_recorder_is_clone() {
    let rec = PermissionRecorder::disabled();
    let _clone = rec;
}

// ── CompressionRecorder tests (T-017, FR-017) ──────────────────────────

#[test]
fn test_disabled_compression_recorder_is_noop() {
    let rec = CompressionRecorder::disabled();
    assert!(!rec.is_enabled());
    rec.record_compression(1000, 500, 2.0);
}

#[test]
fn test_compression_recorder_record_increments_counter() {
    let (provider, exporter, rt) = build_provider();
    let registry = InstrumentRegistry::from_provider(&provider);
    let rec = CompressionRecorder {
        registry: Some(registry),
    };

    rec.record_compression(1000, 500, 2.0);
    rec.record_compression(2000, 800, 2.5);
    rec.record_compression(1500, 1500, 1.0);

    rt.block_on(async {
        provider.force_flush().expect("flush");
    });

    let metrics = exporter.get_finished_metrics().unwrap_or_default();
    let total: u64 = metrics
        .iter()
        .flat_map(|rm| rm.scope_metrics.iter())
        .flat_map(|sm| sm.metrics.iter())
        .filter(|m| m.name == "ragent.context.compressions")
        .filter_map(|m| {
            m.data
                .as_any()
                .downcast_ref::<opentelemetry_sdk::metrics::data::Sum<u64>>()
        })
        .flat_map(|sum| sum.data_points.iter())
        .map(|dp| dp.value)
        .sum();
    assert_eq!(total, 3, "context.compressions should be 3");
}

#[test]
fn test_compression_recorder_records_ratio_histogram() {
    let (provider, exporter, rt) = build_provider();
    let registry = InstrumentRegistry::from_provider(&provider);
    let rec = CompressionRecorder {
        registry: Some(registry),
    };

    rec.record_compression(1000, 500, 2.0);

    rt.block_on(async {
        provider.force_flush().expect("flush");
    });

    let metrics = exporter.get_finished_metrics().unwrap_or_default();
    let has_ratio = metrics.iter().any(|rm| {
        rm.scope_metrics
            .iter()
            .flat_map(|sm| sm.metrics.iter())
            .any(|m| m.name == "ragent.context.compression_ratio")
    });
    assert!(
        has_ratio,
        "ragent.context.compression_ratio should be in exported metrics"
    );
}

#[test]
fn test_compression_recorder_is_clone() {
    let rec = CompressionRecorder::disabled();
    let _clone = rec;
}

// ── SnapshotRecorder tests (T-027, FR-029) ─────────────────────────────

#[test]
fn test_disabled_snapshot_recorder_is_noop() {
    let rec = SnapshotRecorder::disabled();
    assert!(!rec.is_enabled());
    rec.record_restore();
}

#[test]
fn test_snapshot_recorder_record_restore_increments_counter() {
    let (provider, exporter, rt) = build_provider();
    let registry = InstrumentRegistry::from_provider(&provider);
    let rec = SnapshotRecorder {
        registry: Some(registry),
    };

    rec.record_restore();
    rec.record_restore();
    rec.record_restore();

    rt.block_on(async {
        provider.force_flush().expect("flush");
    });

    let metrics = exporter.get_finished_metrics().unwrap_or_default();
    let total: u64 = metrics
        .iter()
        .flat_map(|rm| rm.scope_metrics.iter())
        .flat_map(|sm| sm.metrics.iter())
        .filter(|m| m.name == "ragent.snapshot.restores")
        .filter_map(|m| {
            m.data
                .as_any()
                .downcast_ref::<opentelemetry_sdk::metrics::data::Sum<u64>>()
        })
        .flat_map(|sum| sum.data_points.iter())
        .map(|dp| dp.value)
        .sum();
    assert_eq!(total, 3, "snapshot.restores should be 3");
}

#[test]
fn test_snapshot_recorder_respects_metric_toggle() {
    let (provider, exporter, rt) = build_provider();
    let mut toggles = std::collections::HashMap::<String, bool>::new();
    toggles.insert(names::SNAPSHOT_RESTORES.to_string(), false);
    let registry = InstrumentRegistry::from_provider(&provider).with_metric_toggles(toggles);
    let rec = SnapshotRecorder {
        registry: Some(registry),
    };

    rec.record_restore();

    rt.block_on(async {
        provider.force_flush().expect("flush");
    });

    let metrics = exporter.get_finished_metrics().unwrap_or_default();
    let total: u64 = metrics
        .iter()
        .flat_map(|rm| rm.scope_metrics.iter())
        .flat_map(|sm| sm.metrics.iter())
        .filter(|m| m.name == "ragent.snapshot.restores")
        .filter_map(|m| {
            m.data
                .as_any()
                .downcast_ref::<opentelemetry_sdk::metrics::data::Sum<u64>>()
        })
        .flat_map(|sum| sum.data_points.iter())
        .map(|dp| dp.value)
        .sum();
    assert_eq!(total, 0, "snapshot.restores should be disabled by toggle");
}
