//! Tests-only coverage for FR-018, FR-032 and FR-043: when a finished sub-agent
//! task has no durable copy of its report, `list_agents` and `wait_agents` say
//! so explicitly instead of omitting the field.
//!
//! `persist_task_output` returns `None` when the report cannot be written - an
//! unwritable or read-only `log/subagents/`, a full disk, a failed rename. The
//! task still completes with its result in memory, so before this change the
//! caller saw a row with no `output_file` and had no way to tell "nothing was
//! ever written, the only copy is in memory" from "this task has no report".
//! A model reading the first shape as the second goes looking for a file that
//! does not exist and loses the findings when the text is truncated.
//!
//! The tests seed a finished, report-less `TaskEntry` directly, because driving
//! a real sub-agent to a persistence failure needs a live provider and a
//! broken filesystem; the condition under test is the reporting, and the entry
//! is exactly what the registry holds when the write fails.

use std::path::PathBuf;
use std::sync::Arc;

use ragent_agent::event::EventBus;
use ragent_agent::task::{AgentManager, TaskStatus};
use ragent_agent::tool::list_agents::ListAgentsTool;
use ragent_agent::tool::wait_agents::{IN_MEMORY_REPORT_NOTE, WaitAgentsTool};
use ragent_agent::tool::{Tool, ToolContext};
use serde_json::json;

#[path = "task_support/mod.rs"]
mod task_support;
use task_support::{entry_full, manager};

/// A `ToolContext` bound to `session_id` with `manager` wired in.
fn ctx_for(session_id: &str, manager: Arc<AgentManager>) -> ToolContext {
    ToolContext {
        session_id: session_id.to_string(),
        working_dir: PathBuf::from("target/temp"),
        event_bus: Arc::new(EventBus::new(16)),
        storage: None,
        agent_manager: Some(manager),
        active_model: None,
        provider_registry: None,
        team_context: None,
        team_manager: None,
        code_index: None,
        bg_service: None,
        spec_manager: None,
        active_spec_id: None,
        config: None,
        allowed_roots: Vec::new(),
        read_timestamps: Arc::new(std::sync::RwLock::new(std::collections::HashMap::new())),
        cached_team_dir: Arc::new(std::sync::Mutex::new(None)),
        permission_checker: None,
        tool_registry: ToolContext::default_tool_registry(),
        canonical_cache: Arc::new(ragent_tools_core::CanonicalPathCache::new()),
    }
}

/// Seed a finished background task whose report never reached disk.
fn seed_unpersisted(
    parent: &str,
    task_id: &str,
    status: TaskStatus,
) -> ragent_agent::task::TaskEntry {
    let mut entry = entry_full(task_id, status, parent, true, false);
    entry.result = Some(Arc::from("the only copy of these findings"));
    entry.completed_at = Some(entry.created_at);
    entry.output_file = None;
    entry
}

// ---------------------------------------------------------------------------
// list_agents (FR-018 list rows, FR-032 detail view)
// ---------------------------------------------------------------------------

/// FR-018: the list row states the in-memory-only condition.
#[tokio::test]
async fn list_mode_states_the_report_is_in_memory_only() {
    let mgr = manager(4);
    let entry = seed_unpersisted("parent", "t-nofile", TaskStatus::Completed);
    mgr.seed_completed_for_test(entry).await;

    let ctx = ctx_for("parent", Arc::clone(&mgr));
    let out = ListAgentsTool
        .execute(json!({}), &ctx)
        .await
        .expect("list_agents must succeed");

    assert!(
        out.content.contains("[memory]"),
        "the row must carry an explicit in-memory marker, not silently omit the path: {}",
        out.content
    );
    assert!(
        out.content.contains(IN_MEMORY_REPORT_NOTE),
        "the row must use the shared wording: {}",
        out.content
    );
    assert!(
        out.content.contains("in-memory only"),
        "the condition must be named in words a model can act on: {}",
        out.content
    );
    assert!(
        !out.content.contains("Full report:"),
        "no durable path may be claimed when none exists (FR-043): {}",
        out.content
    );
}

/// The marker names the status that finished without a durable copy, so a
/// cancelled or timed-out task reads distinctly from a completed one.
#[tokio::test]
async fn list_mode_names_the_status_that_lost_its_report() {
    let mgr = manager(4);
    for (task_id, status) in [
        ("t-cancelled", TaskStatus::Cancelled),
        ("t-timedout", TaskStatus::TimedOut),
    ] {
        let entry = seed_unpersisted("parent", task_id, status);
        mgr.seed_completed_for_test(entry).await;
    }

    let ctx = ctx_for("parent", Arc::clone(&mgr));
    let out = ListAgentsTool
        .execute(json!({}), &ctx)
        .await
        .expect("list_agents must succeed");

    assert!(
        out.content.contains("cancelled - report is"),
        "a cancelled task's absence line must name `cancelled`: {}",
        out.content
    );
    assert!(
        out.content.contains("timed_out - report is"),
        "a timed-out task's absence line must name `timed_out`: {}",
        out.content
    );
}

/// FR-032: the detail view states the same condition and prints no path.
#[tokio::test]
async fn detail_mode_states_the_report_is_in_memory_only() {
    let mgr = manager(4);
    let entry = seed_unpersisted("parent", "t-nofile", TaskStatus::Completed);
    mgr.seed_completed_for_test(entry).await;

    let ctx = ctx_for("parent", Arc::clone(&mgr));
    let out = ListAgentsTool
        .execute(json!({"task_id": "t-nofile"}), &ctx)
        .await
        .expect("detail must succeed");

    assert!(
        out.content.contains("Output File: none -"),
        "the detail view must state the absence rather than say nothing (FR-032): {}",
        out.content
    );
    assert!(
        out.content.contains(IN_MEMORY_REPORT_NOTE),
        "the detail view must use the shared wording: {}",
        out.content
    );
    assert!(
        !out.content
            .contains("Output File (full untruncated report)"),
        "no `Output File` heading may appear without a path: {}",
        out.content
    );
}

/// A task that DID persist its report keeps the path and gets no absence line,
/// so the note cannot become noise on every healthy task.
#[tokio::test]
async fn a_persisted_report_gets_no_absence_note() {
    let mgr = manager(4);
    let mut entry = seed_unpersisted("parent", "t-file", TaskStatus::Completed);
    entry.output_file = Some(PathBuf::from("log/subagents/t-file.md"));
    mgr.seed_completed_for_test(entry).await;

    let ctx = ctx_for("parent", Arc::clone(&mgr));

    let listed = ListAgentsTool
        .execute(json!({}), &ctx)
        .await
        .expect("list must succeed");
    assert!(listed.content.contains("Full report:"));
    assert!(
        !listed.content.contains(IN_MEMORY_REPORT_NOTE),
        "a persisted report must not also claim to be in-memory only: {}",
        listed.content
    );

    let detail = ListAgentsTool
        .execute(json!({"task_id": "t-file"}), &ctx)
        .await
        .expect("detail must succeed");
    assert!(
        detail
            .content
            .contains("Output File (full untruncated report):\nlog/subagents/t-file.md")
    );
    assert!(!detail.content.contains(IN_MEMORY_REPORT_NOTE));
}

/// A live task reports nothing: the write has not been attempted yet, so
/// "no durable copy" would describe a failure that has not happened.
#[tokio::test]
async fn a_live_task_reports_no_absence_note() {
    let mgr = manager(4);
    let live = entry_full("t-live", TaskStatus::Running, "parent", true, false);
    mgr.seed_completed_for_test(live).await;
    let parked = entry_full("t-parked", TaskStatus::Suspended, "parent", true, false);
    mgr.seed_completed_for_test(parked).await;

    let ctx = ctx_for("parent", Arc::clone(&mgr));
    let out = ListAgentsTool
        .execute(json!({}), &ctx)
        .await
        .expect("list must succeed");

    assert!(
        !out.content.contains(IN_MEMORY_REPORT_NOTE),
        "an in-flight task must not be told its report failed to persist: {}",
        out.content
    );
}

// ---------------------------------------------------------------------------
// wait_agents (FR-018, FR-043)
// ---------------------------------------------------------------------------

/// FR-018 / FR-043: `wait_agents` reports the absence in content AND metadata.
#[tokio::test]
async fn wait_agents_reports_the_absence_in_content_and_metadata() {
    let mgr = manager(4);
    let entry = seed_unpersisted("parent", "t-nofile", TaskStatus::Completed);
    mgr.seed_completed_for_test(entry).await;

    let ctx = ctx_for("parent", Arc::clone(&mgr));
    let out = WaitAgentsTool
        .execute(json!({}), &ctx)
        .await
        .expect("wait_agents must succeed");

    assert!(
        out.content.contains("[memory] No durable copy:"),
        "the content must name the absence: {}",
        out.content
    );
    assert!(
        out.content.contains(IN_MEMORY_REPORT_NOTE),
        "the content must use the shared wording: {}",
        out.content
    );

    let meta = out.metadata.expect("metadata must be present");
    let results = meta
        .get("results")
        .and_then(serde_json::Value::as_array)
        .expect("results array must exist");
    assert_eq!(results.len(), 1);
    let entry = &results[0];

    // FR-043: the flag is the machine-readable half of the same fact.
    assert_eq!(
        entry
            .get("report_persisted")
            .and_then(serde_json::Value::as_bool),
        Some(false),
        "a report that could not be persisted must say so: {entry}"
    );
    assert_eq!(
        entry
            .get("report_location")
            .and_then(serde_json::Value::as_str),
        Some("in_memory_only"),
        "the location must be named, not left to inference: {entry}"
    );
    // The field is still present and null, so a caller that only checks for
    // presence does not mistake the absence for a shape change.
    assert!(
        entry
            .get("output_file")
            .is_some_and(serde_json::Value::is_null),
        "output_file must be present and null, not omitted: {entry}"
    );

    // The per-task roll-up carries the same fact (FR-018).
    let tasks = meta
        .get("tasks")
        .and_then(serde_json::Value::as_array)
        .expect("tasks array must exist");
    assert_eq!(
        tasks[0]
            .get("report_persisted")
            .and_then(serde_json::Value::as_bool),
        Some(false),
        "the tasks roll-up must not imply the report was persisted: {}",
        tasks[0]
    );
}

/// The persisted case keeps `report_persisted: true` and the real path.
#[tokio::test]
async fn wait_agents_marks_a_persisted_report_as_durable() {
    let mgr = manager(4);
    let mut entry = seed_unpersisted("parent", "t-file", TaskStatus::Completed);
    let path = PathBuf::from("log/subagents/t-file.md");
    entry.output_file = Some(path.clone());
    mgr.seed_completed_for_test(entry).await;

    let ctx = ctx_for("parent", Arc::clone(&mgr));
    let out = WaitAgentsTool
        .execute(json!({}), &ctx)
        .await
        .expect("wait_agents must succeed");

    assert!(
        !out.content.contains("[memory] No durable copy:"),
        "a persisted report must not claim to be in-memory only: {}",
        out.content
    );

    let meta = out.metadata.expect("metadata must be present");
    let entry = &meta
        .get("results")
        .and_then(serde_json::Value::as_array)
        .expect("results must exist")[0];
    assert_eq!(
        entry
            .get("report_persisted")
            .and_then(serde_json::Value::as_bool),
        Some(true)
    );
    assert_eq!(
        entry
            .get("report_location")
            .and_then(serde_json::Value::as_str),
        Some("durable")
    );
    assert_eq!(
        entry.get("output_file").and_then(serde_json::Value::as_str),
        Some(path.display().to_string().as_str())
    );
}

/// A wait that returns with a task still in flight says nothing about its
/// report, because there is not yet one to persist or fail.
#[tokio::test]
async fn wait_agents_says_nothing_about_an_unsettled_task_report() {
    let mgr = manager(4);
    mgr.seed_completed_for_test(entry_full(
        "t-parked",
        TaskStatus::Suspended,
        "parent",
        true,
        false,
    ))
    .await;

    let ctx = ctx_for("parent", Arc::clone(&mgr));
    let out = WaitAgentsTool
        .execute(json!({"task_ids": ["t-parked"], "timeout_secs": 1}), &ctx)
        .await
        .expect("wait_agents must succeed");

    assert!(
        !out.content.contains(IN_MEMORY_REPORT_NOTE),
        "an unsettled task has no report to report on: {}",
        out.content
    );
    let meta = out.metadata.expect("metadata must be present");
    let unsettled = meta
        .get("unsettled")
        .and_then(serde_json::Value::as_array)
        .expect("unsettled must exist");
    assert!(
        unsettled[0].get("report_persisted").is_none(),
        "an unsettled entry must not carry a persistence verdict: {}",
        unsettled[0]
    );
}
