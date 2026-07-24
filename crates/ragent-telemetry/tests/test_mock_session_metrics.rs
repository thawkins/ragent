//! Integration test: mock session records expected metrics (T-030, AC-1).
//!
//! This test simulates a single agent session by exercising the high-level
//! recorders in the same order the real session processor would, then
//! force-flushing a `SdkMeterProvider` backed by an `InMemoryMetricExporter`
//! and asserting that the expected usage, performance, cost, and
//! effectiveness metrics are present.
//!
//! AC-1: "The system shall record usage, performance, cost, and effectiveness
//! metrics during an agent session."

#[cfg(feature = "telemetry")]
mod mock_session {
    use std::time::Duration;

    use opentelemetry_sdk::metrics::SdkMeterProvider;
    use opentelemetry_sdk::runtime::Tokio;
    use opentelemetry_sdk::testing::metrics::InMemoryMetricExporter;

    use ragent_config::Cost;
    use ragent_telemetry::InstrumentRegistry;
    use ragent_telemetry::recorder::{
        CompressionRecorder, CoordinatorRecorder, LlmRecorder, PermissionRecorder, SessionRecorder,
        SnapshotRecorder, ToolRecorder, compute_cost_usd,
    };

    fn build_registry() -> (
        InstrumentRegistry,
        InMemoryMetricExporter,
        SdkMeterProvider,
        tokio::runtime::Runtime,
    ) {
        let rt = tokio::runtime::Runtime::new().expect("tokio runtime");
        let exporter = InMemoryMetricExporter::default();
        let exporter_clone = exporter.clone();
        let provider = rt.block_on(async {
            let reader = opentelemetry_sdk::metrics::PeriodicReader::builder(exporter_clone, Tokio)
                .with_interval(Duration::from_hours(1))
                .build();
            SdkMeterProvider::builder().with_reader(reader).build()
        });
        let registry = InstrumentRegistry::from_provider(&provider);
        (registry, exporter, provider, rt)
    }

    fn sum_u64(metrics: &[opentelemetry_sdk::metrics::data::ResourceMetrics], name: &str) -> u64 {
        metrics
            .iter()
            .flat_map(|rm| rm.scope_metrics.iter())
            .flat_map(|sm| sm.metrics.iter())
            .filter(|m| m.name == name)
            .filter_map(|m| {
                m.data
                    .as_any()
                    .downcast_ref::<opentelemetry_sdk::metrics::data::Sum<u64>>()
            })
            .flat_map(|sum| sum.data_points.iter())
            .map(|dp| dp.value)
            .sum()
    }

    fn sum_i64(metrics: &[opentelemetry_sdk::metrics::data::ResourceMetrics], name: &str) -> i64 {
        metrics
            .iter()
            .flat_map(|rm| rm.scope_metrics.iter())
            .flat_map(|sm| sm.metrics.iter())
            .filter(|m| m.name == name)
            .filter_map(|m| {
                m.data
                    .as_any()
                    .downcast_ref::<opentelemetry_sdk::metrics::data::Sum<i64>>()
            })
            .flat_map(|sum| sum.data_points.iter())
            .map(|dp| dp.value)
            .sum()
    }

    fn has_histogram_f64(
        metrics: &[opentelemetry_sdk::metrics::data::ResourceMetrics],
        name: &str,
    ) -> bool {
        metrics.iter().any(|rm| {
            rm.scope_metrics
                .iter()
                .flat_map(|sm| sm.metrics.iter())
                .filter(|m| m.name == name)
                .any(|m| {
                    m.data
                        .as_any()
                        .downcast_ref::<opentelemetry_sdk::metrics::data::Histogram<f64>>()
                        .is_some()
                })
        })
    }

    fn has_histogram_u64(
        metrics: &[opentelemetry_sdk::metrics::data::ResourceMetrics],
        name: &str,
    ) -> bool {
        metrics.iter().any(|rm| {
            rm.scope_metrics
                .iter()
                .flat_map(|sm| sm.metrics.iter())
                .filter(|m| m.name == name)
                .any(|m| {
                    m.data
                        .as_any()
                        .downcast_ref::<opentelemetry_sdk::metrics::data::Histogram<u64>>()
                        .is_some()
                })
        })
    }

    #[test]
    fn test_mock_session_records_expected_metrics() {
        let (registry, exporter, provider, rt) = build_registry();

        // Simulate session start.
        let session = SessionRecorder::new(registry.clone());
        session.record_session_start();

        // Simulate one LLM call.
        let llm = LlmRecorder::new(registry.clone());
        llm.record_request("gpt-4", "openai");
        llm.record_usage("gpt-4", "openai", 100, 50);
        let cost = Cost {
            input: 3.0,
            output: 6.0,
        };
        let cost_usd = compute_cost_usd(100, 50, &cost);
        llm.record_cost("gpt-4", "openai", cost_usd);
        llm.record_duration("gpt-4", "openai", 1234.0);
        llm.record_ttft("gpt-4", 150.0);

        // Simulate one tool execution.
        let tool = ToolRecorder::new(registry.clone());
        tool.record_invocation("read");
        tool.record_duration("read", 42.0);

        // Simulate coordinator / sub-agent lifecycle.
        let coordinator = CoordinatorRecorder::new(registry.clone());
        coordinator.record_agent_spawn();
        coordinator.record_agent_complete();
        coordinator.record_error("tool");
        coordinator.record_timeout();

        // Simulate permission resolution.
        let permission = PermissionRecorder::new(registry.clone());
        permission.record_approved("bash");
        permission.record_denied("edit");

        // Simulate compression pipeline run.
        let compression = CompressionRecorder::new(registry.clone());
        compression.record_compression(1000, 500, 2.0);

        // Simulate snapshot restore.
        let snapshot = SnapshotRecorder::new(registry);
        snapshot.record_restore();
        // Simulate agent loop completion.
        session.record_agent_loop(5000.0, 3);

        // End the session.
        session.record_session_end();

        // Flush and inspect exported metrics.
        rt.block_on(async {
            provider.force_flush().expect("flush provider");
        });
        let metrics = exporter.get_finished_metrics().unwrap_or_default();

        assert_eq!(
            sum_u64(&metrics, "ragent.sessions.total"),
            1,
            "sessions.total"
        );
        assert_eq!(
            sum_i64(&metrics, "ragent.sessions.active"),
            0,
            "sessions.active net after start+end"
        );
        assert_eq!(sum_u64(&metrics, "ragent.llm.requests"), 1, "llm.requests");
        assert_eq!(
            sum_u64(&metrics, "ragent.tokens.input"),
            100,
            "tokens.input"
        );
        assert_eq!(
            sum_u64(&metrics, "ragent.tokens.output"),
            50,
            "tokens.output"
        );
        let exported_cost = sum_f64(&metrics, "ragent.cost.estimated");
        assert!(
            (exported_cost - cost_usd).abs() < 1e-9,
            "cost.estimated should be {cost_usd}, got {exported_cost}"
        );
        assert_eq!(
            sum_u64(&metrics, "ragent.tool.invocations"),
            1,
            "tool.invocations"
        );
        assert_eq!(
            sum_u64(&metrics, "ragent.subagent.spawns"),
            1,
            "subagent.spawns"
        );
        assert_eq!(
            sum_u64(&metrics, "ragent.agents.completed"),
            1,
            "agents.completed"
        );
        assert_eq!(
            sum_i64(&metrics, "ragent.agents.active"),
            0,
            "agents.active net after spawn+complete"
        );
        assert_eq!(
            sum_u64(&metrics, "ragent.permission.approved"),
            1,
            "permission.approved"
        );
        assert_eq!(
            sum_u64(&metrics, "ragent.permission.denied"),
            1,
            "permission.denied"
        );
        assert_eq!(
            sum_u64(&metrics, "ragent.context.compressions"),
            1,
            "context.compressions"
        );
        assert_eq!(
            sum_u64(&metrics, "ragent.snapshot.restores"),
            1,
            "snapshot.restores"
        );
        assert_eq!(sum_u64(&metrics, "ragent.errors.total"), 1, "errors.total");
        assert_eq!(
            sum_u64(&metrics, "ragent.timeouts.total"),
            1,
            "timeouts.total"
        );

        assert!(
            has_histogram_f64(&metrics, "ragent.llm.duration"),
            "llm.duration histogram missing"
        );
        assert!(
            has_histogram_f64(&metrics, "ragent.llm.time_to_first_token"),
            "llm.time_to_first_token histogram missing"
        );
        assert!(
            has_histogram_f64(&metrics, "ragent.tool.duration"),
            "tool.duration histogram missing"
        );
        assert!(
            has_histogram_f64(&metrics, "ragent.agent_loop.duration"),
            "agent_loop.duration histogram missing"
        );
        assert!(
            has_histogram_u64(&metrics, "ragent.agent_loop.iterations"),
            "agent_loop.iterations histogram missing"
        );
        assert!(
            has_histogram_f64(&metrics, "ragent.context.compression_ratio"),
            "context.compression_ratio histogram missing"
        );
    }

    fn sum_f64(metrics: &[opentelemetry_sdk::metrics::data::ResourceMetrics], name: &str) -> f64 {
        metrics
            .iter()
            .flat_map(|rm| rm.scope_metrics.iter())
            .flat_map(|sm| sm.metrics.iter())
            .filter(|m| m.name == name)
            .filter_map(|m| {
                m.data
                    .as_any()
                    .downcast_ref::<opentelemetry_sdk::metrics::data::Sum<f64>>()
            })
            .flat_map(|sum| sum.data_points.iter())
            .map(|dp| dp.value)
            .sum()
    }
}

#[cfg(not(feature = "telemetry"))]
mod mock_session {
    #[test]
    fn test_mock_session_noop_when_feature_off() {
        // When telemetry feature is disabled, there is nothing to record and
        // the test crate has no real instruments. This test exists so the file
        // still compiles and passes in the default feature set.
        assert!(true);
    }
}
