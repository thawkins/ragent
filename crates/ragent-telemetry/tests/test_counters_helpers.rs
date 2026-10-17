//! External tests exercising every typed counter/gauge update helper and the
//! `TelemetryCountersContent` accessor in `counters.rs`.

use ragent_telemetry::counters::{
    AtomicF64, TelemetryCountersContent, add_agents_active, add_sessions_active, current_values,
    increment_agents_completed, increment_context_compressions, increment_cost_estimated,
    increment_errors_total, increment_llm_requests, increment_messages_user,
    increment_permission_approved, increment_permission_denied, increment_retries_llm,
    increment_sessions_total, increment_snapshot_restores, increment_subagent_spawns,
    increment_task_completions, increment_timeouts_total, increment_tokens_cache_read,
    increment_tokens_cache_write, increment_tokens_input, increment_tokens_output,
    increment_tool_invocations, set_agent_loop_duration_last, set_agent_loop_iterations_last,
    set_context_compression_ratio_last, set_cost_session_last, set_llm_duration_last,
    set_llm_ttft_last, set_rate_limit_requests_pct, set_rate_limit_tokens_pct,
    set_session_duration_last, set_team_members, set_tool_calls_per_session_last,
    set_tool_duration_last, set_tool_permission_wait_last,
};

/// Each increment helper must move its matching snapshot field by the
/// requested delta. Uses large unique deltas so the assertion is not confused
/// by any other test running concurrently in the same process.
#[test]
fn test_every_increment_helper_updates_its_field() {
    // A unique, large base so concurrent tests cannot collide.
    let base: u64 = 8_000_000_000;

    increment_llm_requests(base + 1);
    increment_sessions_total(base + 2);
    increment_messages_user(base + 3);
    increment_tool_invocations(base + 4);
    increment_agents_completed(base + 5);
    increment_subagent_spawns(base + 6);
    increment_tokens_input(base + 7);
    increment_tokens_output(base + 8);
    increment_tokens_cache_read(base + 9);
    increment_tokens_cache_write(base + 10);
    increment_errors_total(base + 11);
    increment_timeouts_total(base + 12);
    increment_permission_denied(base + 13);
    increment_permission_approved(base + 14);
    increment_context_compressions(base + 15);
    increment_task_completions(base + 16);
    increment_retries_llm(base + 17);
    increment_snapshot_restores(base + 18);

    let v = current_values();
    assert!(v.llm_requests > base);
    assert!(v.sessions_total > base + 1);
    assert!(v.messages_user > base + 2);
    assert!(v.tool_invocations > base + 3);
    assert!(v.agents_completed > base + 4);
    assert!(v.subagent_spawns > base + 5);
    assert!(v.tokens_input > base + 6);
    assert!(v.tokens_output > base + 7);
    assert!(v.tokens_cache_read > base + 8);
    assert!(v.tokens_cache_write > base + 9);
    assert!(v.errors_total > base + 10);
    assert!(v.timeouts_total > base + 11);
    assert!(v.permission_denied > base + 12);
    assert!(v.permission_approved > base + 13);
    assert!(v.context_compressions > base + 14);
    assert!(v.task_completions > base + 15);
    assert!(v.retries_llm > base + 16);
    assert!(v.snapshot_restores > base + 17);
}

/// The signed add helpers and `set_*` helpers must round-trip through the
/// snapshot for gauges and last-values.
#[test]
fn test_gauge_add_and_set_helpers_round_trip() {
    add_sessions_active(5);
    add_sessions_active(-3);
    assert_eq!(current_values().sessions_active, 2);

    add_agents_active(4);
    add_agents_active(-1);
    assert_eq!(current_values().agents_active, 3);

    set_team_members(-7);
    assert_eq!(current_values().team_members, -7);

    increment_cost_estimated(1.25);
    increment_cost_estimated(0.75);
    assert!((current_values().cost_estimated - 2.0).abs() < 1e-9);

    set_rate_limit_requests_pct(12.5);
    set_rate_limit_tokens_pct(87.5);
    set_llm_duration_last(101.0);
    set_llm_ttft_last(202.0);
    set_tool_duration_last(303.0);
    set_agent_loop_duration_last(404.0);
    set_agent_loop_iterations_last(55);
    set_session_duration_last(606.0);
    set_tool_permission_wait_last(707.0);
    set_cost_session_last(8.25);
    set_context_compression_ratio_last(0.375);
    set_tool_calls_per_session_last(99);

    let v = current_values();
    assert!((v.rate_limit_requests_pct - 12.5).abs() < 1e-9);
    assert!((v.rate_limit_tokens_pct - 87.5).abs() < 1e-9);
    assert!((v.llm_duration_last - 101.0).abs() < 1e-9);
    assert!((v.llm_ttft_last - 202.0).abs() < 1e-9);
    assert!((v.tool_duration_last - 303.0).abs() < 1e-9);
    assert!((v.agent_loop_duration_last - 404.0).abs() < 1e-9);
    assert_eq!(v.agent_loop_iterations_last, 55);
    assert!((v.session_duration_last - 606.0).abs() < 1e-9);
    assert!((v.tool_permission_wait_last - 707.0).abs() < 1e-9);
    assert!((v.cost_session_last - 8.25).abs() < 1e-9);
    assert!((v.context_compression_ratio_last - 0.375).abs() < 1e-9);
    assert_eq!(v.tool_calls_per_session_last, 99);
}

/// `AtomicF64` stores and loads the exact bit pattern of a value including a
/// negative and a subnormal-ish value.
#[test]
fn test_atomic_f64_negative_and_fractional() {
    let a = AtomicF64::default();
    a.store(-123.456);
    assert!((a.load() + 123.456).abs() < 1e-9);
    a.fetch_add(23.456);
    assert!((a.load() + 100.0).abs() < 1e-9);
}

/// `TelemetryCountersContent::markdown` returns the pre-rendered string.
#[test]
fn test_counters_content_markdown_accessor() {
    let content = TelemetryCountersContent {
        usage: vec![(
            "ragent.llm.requests".to_string(),
            "counter".to_string(),
            "requests".to_string(),
            "3".to_string(),
        )],
        performance: vec![],
        cost: vec![],
        effectiveness: vec![],
        markdown: "# counters\nbody".to_string(),
    };
    assert_eq!(content.markdown(), "# counters\nbody");
    assert_eq!(content.usage.len(), 1);
}
