//! Tests for the agent-loop profiler.

#![allow(clippy::float_cmp)] // integer millisecond values are represented exactly

use std::sync::atomic::AtomicUsize;
use std::sync::{Arc, Mutex};

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

// ---- PERFPLAN Milestone A (P-25, P-26) -------------------------------------
//
// These tests verify that the profiler's `scope` and `scope_with` paths
// short-circuit *before* allocating the label when profiling is disabled
// (the default). P-25: `scope` must not call `to_string()` on the static
// label when disabled. P-26: `scope_with` must not invoke the label closure
// when disabled.

#[test]
fn test_scope_disabled_profiler_records_nothing() {
    // P-25: `scope` with profiling disabled must produce no recorded samples.
    // A disabled profiler should never touch the stats map.
    let profiler = Arc::new(AgentLoopProfiler::new());
    assert!(!profiler.is_enabled());

    {
        let _scope = profiler.scope("unit.disabled");
        std::thread::sleep(std::time::Duration::from_millis(2));
    }

    let snapshot = profiler.snapshot();
    assert!(!snapshot.enabled);
    assert_eq!(snapshot.total_samples, 0);
    assert!(snapshot.operations.is_empty());
}

#[test]
fn test_scope_with_skips_label_fn_when_disabled() {
    // P-26: `scope_with` must not invoke the label closure when profiling
    // is disabled. We track closure invocations with a shared counter; the
    // test fails if the counter increments while the profiler is off.
    let profiler = Arc::new(AgentLoopProfiler::new());
    assert!(!profiler.is_enabled());

    let call_count = Arc::new(AtomicUsize::new(0));
    let call_count_clone = Arc::clone(&call_count);
    {
        let _scope = profiler.scope_with(move || {
            call_count_clone.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            "dynamic.label".to_string()
        });
        std::thread::sleep(std::time::Duration::from_millis(1));
    }

    // Closure must not have been invoked.
    assert_eq!(
        call_count.load(std::sync::atomic::Ordering::SeqCst),
        0,
        "scope_with invoked the label closure while profiling was disabled"
    );

    // And no sample should have been recorded.
    let snapshot = profiler.snapshot();
    assert_eq!(snapshot.total_samples, 0);
    assert!(snapshot.operations.is_empty());
}

#[test]
fn test_scope_with_invokes_label_fn_when_enabled() {
    // P-26 companion: when profiling IS enabled, the label closure must run
    // exactly once and the scope must be recorded.
    let profiler = Arc::new(AgentLoopProfiler::new());
    profiler.set_enabled(true);

    let call_count = Arc::new(AtomicUsize::new(0));
    let call_count_clone = Arc::clone(&call_count);
    {
        let _scope = profiler.scope_with(move || {
            call_count_clone.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            "dynamic.enabled".to_string()
        });
        std::thread::sleep(std::time::Duration::from_millis(2));
    }

    assert_eq!(
        call_count.load(std::sync::atomic::Ordering::SeqCst),
        1,
        "scope_with did not invoke the label closure while profiling was enabled"
    );

    let snapshot = profiler.snapshot();
    assert!(snapshot.enabled);
    let op = snapshot
        .operations
        .iter()
        .find(|o| o.name == "dynamic.enabled")
        .expect("dynamic.enabled scope was recorded");
    assert_eq!(op.count, 1);
}

#[test]
fn test_scope_disabled_then_enabled_records() {
    // P-25: confirm `scope` records normally once profiling is enabled,
    // so the early-return only fires on the disabled path.
    let profiler = Arc::new(AgentLoopProfiler::new());
    profiler.set_enabled(true);

    {
        let _scope = profiler.scope("unit.enabled");
        std::thread::sleep(std::time::Duration::from_millis(2));
    }

    let snapshot = profiler.snapshot();
    assert!(snapshot.enabled);
    assert_eq!(snapshot.total_samples, 1);
    assert_eq!(snapshot.operations[0].name, "unit.enabled");
}

// A small Mutex-based helper is kept here in case future tests need to
// observe closure invocation order across threads; currently unused to
// avoid dead-code warnings, so reference it once.
#[allow(dead_code)]
const fn _unused_mutex_anchor() -> Mutex<usize> {
    Mutex::new(0)
}
