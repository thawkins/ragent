//! Tests for the Context side panel (spec `contextpanel`).
//!
//! - **T-015**: Unit tests for token partition arithmetic (FR-009, FR-012).
//!
//! These tests pin the arithmetic of [`ragent_tui::app::ContextPartitionSnapshot`]
//! directly: which partitions feed the FR-012 aggregate, that the FR-009
//! system-prompt sub-partitions are never double counted, and how the
//! headroom and percentage helpers behave at boundary values.

mod support;

use ragent_tui::app::ContextPartitionSnapshot;

fn snapshot(
    system: u64,
    catalog: u64,
    metadata: u64,
    history: u64,
    skills: u64,
    memory: u64,
    agents_md: u64,
    window: Option<usize>,
) -> ContextPartitionSnapshot {
    ContextPartitionSnapshot {
        system_prompt_tokens: system,
        tool_catalog_tokens: catalog,
        tool_metadata_tokens: metadata,
        history_tokens: history,
        history_message_count: 0,
        skills_tokens: skills,
        memory_tokens: memory,
        agents_md_tokens: agents_md,
        context_window_tokens: window,
        last_input_tokens: 0,
    }
}

#[test]
fn test_sub_partitions_are_excluded_from_total() {
    // FR-009/FR-012: skills, memory and AGENTS.md are slices of the system
    // prompt, so they must not be added to the aggregate a second time.
    let snap = snapshot(1_000, 5_000, 600, 2_400, 50, 70, 30, Some(128_000));
    assert_eq!(
        snap.total_tokens(),
        1_000 + 5_000 + 600 + 2_400,
        "sub-partitions inside the system prompt must not be double counted"
    );
}

#[test]
fn test_total_is_invariant_to_sub_partition_values() {
    // FR-012: changing the sub-partition breakdown alone must leave the
    // aggregate untouched, because they are views into the system prompt.
    let base = snapshot(8_000, 20_000, 3_000, 10_000, 0, 0, 0, Some(200_000));
    let with_subs = snapshot(8_000, 20_000, 3_000, 10_000, 400, 900, 250, Some(200_000));
    assert_eq!(base.total_tokens(), with_subs.total_tokens());
    assert_eq!(base.remaining_tokens(), with_subs.remaining_tokens());
    assert_eq!(base.total_percent(), with_subs.total_percent());
}

#[test]
fn test_headroom_shrinks_by_exact_history_delta() {
    // FR-012: growing the conversation history by N tokens reduces the
    // remaining headroom by exactly N.
    let before = snapshot(5_000, 10_000, 1_000, 2_000, 0, 0, 0, Some(64_000));
    let after = snapshot(5_000, 10_000, 1_000, 5_000, 0, 0, 0, Some(64_000));
    assert_eq!(after.history_tokens - before.history_tokens, 3_000);
    assert_eq!(
        before
            .remaining_tokens()
            .map(|r| r - after.remaining_tokens().expect("window known")),
        Some(3_000)
    );
}

#[test]
fn test_zero_partitions_yield_full_headroom() {
    // FR-012 boundary: an empty context consumes nothing and leaves the full
    // advertised window as headroom; a zero-size partition is 0 percent.
    let snap = snapshot(0, 0, 0, 0, 0, 0, 0, Some(128_000));
    assert_eq!(snap.total_tokens(), 0);
    assert_eq!(snap.remaining_tokens(), Some(128_000));
    assert_eq!(snap.percent_of_window(0), Some(0.0));
    assert_eq!(snap.total_percent(), Some(0.0));
}

#[test]
fn test_zero_capacity_window_is_reported_unknown() {
    // FR-011 boundary: a zero capacity is not a valid window; percentages
    // must fall back to "unknown" instead of dividing by zero.
    let snap = snapshot(1_000, 2_000, 0, 3_000, 0, 0, 0, Some(0));
    assert_eq!(snap.remaining_tokens(), Some(6_000 - 6_000));
    assert_eq!(snap.percent_of_window(1_000), None);
    assert_eq!(snap.total_percent(), None);
}

#[test]
fn test_headroom_saturates_for_each_partition_class() {
    // FR-012: saturation must hold regardless of which partition overflows
    // the window - each row saturates at zero, never underflows.
    let over_system = snapshot(500_000, 0, 0, 0, 0, 0, 0, Some(128_000));
    let over_catalog = snapshot(0, 500_000, 0, 0, 0, 0, 0, Some(128_000));
    let over_history = snapshot(0, 0, 0, 500_000, 0, 0, 0, Some(128_000));
    for snap in [over_system, over_catalog, over_history] {
        assert_eq!(snap.remaining_tokens(), Some(0), "must saturate at zero");
        assert_eq!(
            snap.total_percent(),
            Some(100.0),
            "percentage must cap at 100%"
        );
    }
}

#[test]
fn test_percent_of_window_caps_at_100() {
    // FR-010: the panel must never display a usage percentage above 100%,
    // even when the estimator exceeds the advertised context window.
    let snap = snapshot(200_000, 0, 0, 0, 0, 0, 0, Some(128_000));
    assert_eq!(snap.percent_of_window(200_000), Some(100.0));
    let snap2 = snapshot(0, 0, 0, 300_000, 0, 0, 0, Some(128_000));
    assert_eq!(snap2.total_percent(), Some(100.0));
}

#[test]
fn test_history_growth_leaves_system_prompt_partition_stable() {
    // FR-009/FR-012 at the App level: appending messages grows only the
    // history partition and the aggregate; the system prompt (and its
    // skills/memory/AGENTS.md sub-breakdown) is unchanged by chat turns.
    let mut app = support::make_app();
    let before = app.context_partition_snapshot();

    app.messages.push(ragent_agent::message::Message::new(
        "session-1",
        ragent_agent::message::Role::User,
        vec![ragent_agent::message::MessagePart::Text {
            text: "turn content for partition arithmetic".into(),
        }],
    ));

    let after = app.context_partition_snapshot();
    assert_eq!(
        before.system_prompt_tokens, after.system_prompt_tokens,
        "chat turns must not alter the system prompt partition"
    );
    assert_eq!(
        before.skills_tokens, after.skills_tokens,
        "skills sub-partition must stay stable across chat turns"
    );
    assert!(
        after.history_tokens > before.history_tokens,
        "history partition must grow with new messages"
    );
    assert_eq!(
        after.history_message_count,
        before.history_message_count + 1
    );
    assert_eq!(
        after.total_tokens() - before.total_tokens(),
        after.history_tokens - before.history_tokens,
        "aggregate must grow by exactly the history delta"
    );
}
