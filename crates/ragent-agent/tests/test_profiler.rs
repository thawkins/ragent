//! Tests for the agent-loop profiler.

use std::sync::Arc;

use ragent_agent::session::profiler::AgentLoopProfiler;

#[test]
fn test_profiler_records_scoped_operations() {
    let profiler = Arc::new(AgentLoopProfiler::new());
    profiler.set_enabled(true);

    {
        let _scope = profiler.scope("unit.test");
        std::thread::sleep(std::time::Duration::from_millis(2));
    }

    let snapshot = profiler.snapshot();
    assert!(snapshot.enabled);
    assert_eq!(snapshot.total_samples, 1);
    assert_eq!(snapshot.operations.len(), 1);
    assert_eq!(snapshot.operations[0].name, "unit.test");
    assert_eq!(snapshot.operations[0].count, 1);
    assert!(snapshot.operations[0].avg_ms >= 1.0);
    assert!(snapshot.operations[0].self_total_ms >= 1.0);
}

#[test]
fn test_profiler_enable_resets_previous_samples() {
    let profiler = Arc::new(AgentLoopProfiler::new());
    profiler.set_enabled(true);
    profiler.record_duration("unit.test", std::time::Duration::from_millis(5));
    assert_eq!(profiler.snapshot().total_samples, 1);

    profiler.set_enabled(true);

    let snapshot = profiler.snapshot();
    assert!(snapshot.enabled);
    assert_eq!(snapshot.total_samples, 0);
    assert!(snapshot.operations.is_empty());
}

#[test]
fn test_profiler_computes_self_time_for_loop_buckets() {
    let profiler = Arc::new(AgentLoopProfiler::new());
    profiler.set_enabled(true);

    profiler.record_duration("loop.llm.stream", std::time::Duration::from_millis(10));
    profiler.record_duration(
        "loop.llm.first_event_wait",
        std::time::Duration::from_millis(6),
    );
    profiler.record_duration(
        "loop.llm.wait_next_event",
        std::time::Duration::from_millis(2),
    );
    profiler.record_duration(
        "loop.llm.handle.text_delta",
        std::time::Duration::from_millis(1),
    );

    let snapshot = profiler.snapshot();
    let stream = snapshot
        .operations
        .iter()
        .find(|op| op.name == "loop.llm.stream")
        .expect("loop.llm.stream snapshot");

    assert_eq!(stream.total_ms, 10.0);
    assert_eq!(stream.self_total_ms, 1.0);
}

#[test]
fn test_profiler_computes_self_time_for_tool_totals() {
    let profiler = Arc::new(AgentLoopProfiler::new());
    profiler.set_enabled(true);

    profiler.record_duration("tool.total:grep", std::time::Duration::from_millis(20));
    profiler.record_duration("tool.pre_hooks:grep", std::time::Duration::from_millis(1));
    profiler.record_duration("tool.permission:grep", std::time::Duration::from_millis(2));
    profiler.record_duration("tool.execute:grep", std::time::Duration::from_millis(15));
    profiler.record_duration("tool.post_hooks:grep", std::time::Duration::from_millis(1));

    let snapshot = profiler.snapshot();
    let tool_total = snapshot
        .operations
        .iter()
        .find(|op| op.name == "tool.total:grep")
        .expect("tool.total:grep snapshot");

    assert_eq!(tool_total.total_ms, 20.0);
    assert_eq!(tool_total.self_total_ms, 1.0);
}
