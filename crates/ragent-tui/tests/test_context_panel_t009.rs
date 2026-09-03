//! Tests for the Context side panel (spec `contextpanel`).
//!
//! - **T-009**: Aggregate totals and remaining headroom (FR-012).

mod support;

use ragent_tui::app::ContextPartitionSnapshot;

fn snapshot(
    system: u64,
    catalog: u64,
    metadata: u64,
    history: u64,
    window: Option<usize>,
) -> ContextPartitionSnapshot {
    ContextPartitionSnapshot {
        system_prompt_tokens: system,
        tool_catalog_tokens: catalog,
        tool_metadata_tokens: metadata,
        history_tokens: history,
        history_message_count: 0,
        skills_tokens: 0,
        memory_tokens: 0,
        agents_md_tokens: 0,
        context_window_tokens: window,
        last_input_tokens: 0,
    }
}

#[test]
fn test_total_is_sum_of_top_level_partitions() {
    // FR-012: the total is the arithmetic sum of the four top-level
    // partitions; sub-partitions of the system prompt are excluded to avoid
    // double counting.
    let snap = snapshot(1_000, 5_000, 600, 2_400, Some(128_000));
    assert_eq!(snap.total_tokens(), 1_000 + 5_000 + 600 + 2_400);
}

#[test]
fn test_remaining_headroom_is_window_minus_total() {
    // FR-012: headroom = capacity - total.
    let snap = snapshot(10_000, 20_000, 5_000, 15_000, Some(128_000));
    assert_eq!(snap.total_tokens(), 50_000);
    assert_eq!(snap.remaining_tokens(), Some(78_000));
}

#[test]
fn test_remaining_headroom_saturates_at_zero_when_over_capacity() {
    // FR-012: an estimate exceeding the advertised window must clamp at
    // zero rather than underflow.
    let snap = snapshot(200_000, 0, 0, 0, Some(128_000));
    assert_eq!(snap.remaining_tokens(), Some(0));
}

#[test]
fn test_headroom_and_percent_unknown_without_window() {
    // FR-011: with no advertised capacity, headroom and percentages must be
    // "unknown", while absolute counts stay available.
    let snap = snapshot(1_000, 2_000, 0, 3_000, None);
    assert_eq!(snap.remaining_tokens(), None);
    assert_eq!(snap.percent_of_window(1_000), None);
    assert_eq!(snap.total_percent(), None);
    assert_eq!(snap.total_tokens(), 6_000);
}

#[test]
fn test_percent_of_window_arithmetic() {
    // FR-010: percentage math across partitions.
    let snap = snapshot(0, 0, 0, 32_000, Some(128_000));
    let pct = snap.percent_of_window(32_000).expect("window known");
    assert!((pct - 25.0).abs() < 1e-9, "25% expected, got {pct}");
    let total_pct = snapshot(8_000, 16_000, 2_000, 6_000, Some(128_000)).total_percent();
    assert!(
        total_pct.is_some_and(|p| (p - 25.0).abs() < 1e-9),
        "32k of 128k should be 25%"
    );
}

#[test]
fn test_app_snapshot_matches_component_methods() {
    // FR-012: the App-level snapshot must agree with the individual
    // partition methods it aggregates.
    let app = support::make_app();
    let snap = app.context_partition_snapshot();
    assert_eq!(snap.system_prompt_tokens, app.system_prompt_token_count());
    assert_eq!(snap.tool_catalog_tokens, app.tool_catalog_token_count());
    assert_eq!(snap.tool_metadata_tokens, app.tool_metadata_token_count());
    assert_eq!(snap.history_tokens, app.conversation_history_token_count());
    assert_eq!(snap.history_message_count, app.conversation_message_count());
    assert_eq!(snap.skills_tokens, app.skills_token_count());
    assert_eq!(snap.memory_tokens, app.memory_injection_token_count());
    assert_eq!(snap.agents_md_tokens, app.agents_md_token_count());
    assert_eq!(
        snap.context_window_tokens,
        app.active_context_window_tokens()
    );
    assert!(
        snap.total_tokens() > 0,
        "default registry must produce a positive aggregate"
    );
}
