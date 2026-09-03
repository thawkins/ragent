#![allow(clippy::assert_is_empty)]
//! Integration tests for the persisted run-cost summary table (FR-018).
//!
//! FR-018 requires that run-cost summaries are stored separately from the
//! session transcript so the default session export never exposes per-run
//! dollar costs. These tests verify the storage round-trip and the
//! default-vs-opt-in export separation.

use ragent_storage::storage::{RunCostSummaryRow, Storage};

fn sample_row(session_id: &str, id: &str, cost: f64) -> RunCostSummaryRow {
    RunCostSummaryRow {
        id: id.to_string(),
        session_id: session_id.to_string(),
        model_id: "claude-3-5-sonnet".to_string(),
        input_tokens: 1_200,
        output_tokens: 340,
        total_cost_usd: cost,
        duration_ms: 4_500,
        created_at: chrono::Utc::now().to_rfc3339(),
    }
}

#[test]
fn test_create_and_list_run_cost_summary() {
    let storage = Storage::open_in_memory().unwrap();
    storage.create_session("sess-1", "/tmp/project").unwrap();

    storage
        .create_run_cost_summary(&sample_row("sess-1", "rc-1", 0.01))
        .unwrap();
    storage
        .create_run_cost_summary(&sample_row("sess-1", "rc-2", 0.02))
        .unwrap();

    let summaries = storage.list_run_cost_summaries("sess-1").unwrap();
    assert_eq!(summaries.len(), 2);
    // Ordered by created_at ASC; both created ~now, but id ordering is not
    // guaranteed by SQL — assert by collecting ids.
    let ids: Vec<&str> = summaries.iter().map(|s| s.id.as_str()).collect();
    assert!(ids.contains(&"rc-1"));
    assert!(ids.contains(&"rc-2"));
    assert_eq!(summaries[0].model_id, "claude-3-5-sonnet");
    assert_eq!(summaries[0].input_tokens, 1_200);
    assert_eq!(summaries[0].output_tokens, 340);
    assert!(
        (summaries[0].total_cost_usd - 0.01).abs() < f64::EPSILON
            || (summaries[0].total_cost_usd - 0.02).abs() < f64::EPSILON
    );
}

#[test]
fn test_list_run_cost_summaries_is_session_scoped() {
    let storage = Storage::open_in_memory().unwrap();
    storage.create_session("sess-a", "/tmp/a").unwrap();
    storage.create_session("sess-b", "/tmp/b").unwrap();

    storage
        .create_run_cost_summary(&sample_row("sess-a", "rc-a1", 0.5))
        .unwrap();
    storage
        .create_run_cost_summary(&sample_row("sess-b", "rc-b1", 0.7))
        .unwrap();

    assert_eq!(storage.list_run_cost_summaries("sess-a").unwrap().len(), 1);
    assert_eq!(storage.list_run_cost_summaries("sess-b").unwrap().len(), 1);
    // Unknown session → empty, no error.
    assert!(
        storage
            .list_run_cost_summaries("sess-unknown")
            .unwrap()
            .is_empty()
    );
}

#[test]
fn test_run_cost_summary_serializes_for_include_cost_export() {
    let row = sample_row("sess-1", "rc-1", 0.0123);
    let json = serde_json::to_string(&row).expect("row should serialize");
    assert!(json.contains("\"total_cost_usd\":0.0123"), "json: {json}");
    assert!(
        json.contains("\"model_id\":\"claude-3-5-sonnet\""),
        "json: {json}"
    );
    assert!(json.contains("\"duration_ms\":4500"), "json: {json}");
}

#[test]
fn test_default_export_excludes_cost_data() {
    // FR-018: the default export path serializes only messages; cost
    // summaries live in a separate table and are only attached when the
    // caller explicitly opts in. Simulate both export shapes here.
    let storage = Storage::open_in_memory().unwrap();
    storage.create_session("sess-1", "/tmp/project").unwrap();

    // Persist a cost summary.
    storage
        .create_run_cost_summary(&sample_row("sess-1", "rc-1", 0.99))
        .unwrap();

    // Default export: messages only (empty for this session).
    let messages = storage.get_messages("sess-1").unwrap();
    let default_json = serde_json::to_string_pretty(&messages).expect("serialize messages");
    assert!(
        !default_json.contains("total_cost_usd"),
        "default export must not contain cost data: {default_json}"
    );
    assert!(
        !default_json.contains("cost_summaries"),
        "default export must not contain cost_summaries key: {default_json}"
    );

    // Opt-in export: messages + cost_summaries wrapper object.
    let cost_summaries = storage.list_run_cost_summaries("sess-1").unwrap();
    let export = serde_json::json!({
        "messages": messages,
        "cost_summaries": cost_summaries,
    });
    let include_cost_json = serde_json::to_string_pretty(&export).expect("serialize export");
    assert!(
        include_cost_json.contains("\"cost_summaries\""),
        "include_cost export should contain cost_summaries: {include_cost_json}"
    );
    assert!(
        include_cost_json.contains("\"total_cost_usd\": 0.99"),
        "include_cost export should contain the cost value: {include_cost_json}"
    );
}
