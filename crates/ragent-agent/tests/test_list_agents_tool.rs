//! Tests-only coverage for the `list_agents` tool
//! (`ragent_agent::tool::list_agents::ListAgentsTool`).
//!
//! The tool was registered but never invoked by an existing test. These drive
//! its `execute` paths directly against an `AgentManager` seeded through the
//! `#[doc(hidden)]` test helpers: the missing-manager error, list mode
//! (empty / filtered / populated), detail mode (hit + miss), the per-status
//! emoji markers, the running vs completed duration rendering, and the
//! detailed view for result / error / output-file / report-status fields.

use std::path::PathBuf;
use std::sync::Arc;

use ragent_agent::task::{ReportStatus, TaskStatus};
use ragent_agent::tool::list_agents::ListAgentsTool;
use ragent_agent::tool::{Tool, ToolContext};
use serde_json::json;

#[path = "task_support/mod.rs"]
mod task_support;
use task_support::{entry_full, manager};

/// A `ToolContext` with no `AgentManager` wired in.
fn base_ctx() -> ToolContext {
    use ragent_agent::event::EventBus;
    ToolContext {
        session_id: "parent".to_string(),
        working_dir: PathBuf::from("target/temp"),
        event_bus: Arc::new(EventBus::new(16)),
        storage: None,
        agent_manager: None,
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
        cached_team_dir: Arc::new(std::sync::Mutex::new(None)),
        permission_checker: None,
        tool_registry: ToolContext::default_tool_registry(),
        read_timestamps: Arc::new(std::sync::RwLock::new(std::collections::HashMap::new())),
        canonical_cache: Arc::new(ragent_tools_core::CanonicalPathCache::new()),
    }
}

#[tokio::test]
async fn execute_errors_without_agent_manager() {
    let tool = ListAgentsTool;
    let err = tool
        .execute(json!({}), &base_ctx())
        .await
        .expect_err("no manager must error");
    assert!(err.to_string().contains("AgentManager"), "err: {err}");
}

#[tokio::test]
async fn list_mode_empty_with_and_without_status_filter() {
    let mgr = manager(4);
    let mut ctx = base_ctx();
    ctx.agent_manager = Some(mgr);

    let tool = ListAgentsTool;

    let none = tool.execute(json!({}), &ctx).await.expect("empty list");
    assert!(none.content.contains("No sub-agent tasks found"));

    let filtered = tool
        .execute(json!({"status": "running"}), &ctx)
        .await
        .expect("filtered empty list");
    assert!(
        filtered
            .content
            .contains("No tasks with status 'running' found.")
    );
}

#[tokio::test]
async fn list_mode_renders_rows_and_report_markers() {
    let mgr = manager(4);

    let mut truncated = entry_full("truncated-1", TaskStatus::Completed, "parent", true, false);
    truncated.result = Some(Arc::from("a summary"));
    truncated.completed_at = Some(truncated.created_at);
    truncated.report_status = ReportStatus::Truncated;
    truncated.output_file = Some(PathBuf::from("log/subagents/truncated-1.md"));
    mgr.seed_completed_for_test(truncated).await;

    let mut continued = entry_full("continued-1", TaskStatus::Running, "parent", false, false);
    continued.report_status = ReportStatus::Continued;
    mgr.seed_completed_for_test(continued).await;

    let mut ctx = base_ctx();
    ctx.agent_manager = Some(mgr);

    let out = ListAgentsTool.execute(json!({}), &ctx).await.expect("list");

    // Header + per-row markers.
    assert!(
        out.content
            .contains("Sub-agent tasks (2 total, 1 running):")
    );
    assert!(out.content.contains("(TRUNCATED)"));
    assert!(out.content.contains("(continued)"));
    assert!(
        out.content
            .contains("Full report: `log/subagents/truncated-1.md`")
    );
    // A running row renders its "(running)" duration suffix.
    assert!(out.content.contains("(running)"));
}

#[tokio::test]
async fn list_mode_status_filter_selects_matching_tasks() {
    let mgr = manager(4);
    mgr.seed_completed_for_test(entry_full(
        "run",
        TaskStatus::Running,
        "parent",
        true,
        false,
    ))
    .await;
    mgr.seed_completed_for_test(entry_full(
        "done",
        TaskStatus::Completed,
        "parent",
        true,
        false,
    ))
    .await;

    let mut ctx = base_ctx();
    ctx.agent_manager = Some(mgr);

    let completed = ListAgentsTool
        .execute(json!({"status": "completed"}), &ctx)
        .await
        .expect("filtered");
    assert!(completed.content.contains("done"));
    assert!(!completed.content.contains("| run |"));
    // The metadata reflects the filtered count.
    let meta = completed.metadata.expect("metadata");
    assert_eq!(meta["task_count"], 1);
    assert_eq!(meta["running_count"], 0);
}

#[tokio::test]
async fn detail_mode_hit_includes_result_error_output_and_report_status() {
    let mgr = manager(4);

    let mut entry = entry_full(
        "t-detail",
        TaskStatus::Failed,
        "parent-12345678",
        false,
        false,
    );
    entry.agent_name = "explore".to_string();
    entry.task_prompt = "do the thing".to_string();
    entry.result = Some(Arc::from("partial result"));
    entry.error = Some(Arc::from("boom"));
    entry.report_status = ReportStatus::Truncated;
    entry.output_file = Some(PathBuf::from("log/subagents/t-detail.md"));
    mgr.seed_completed_for_test(entry).await;

    let mut ctx = base_ctx();
    ctx.agent_manager = Some(mgr);

    let out = ListAgentsTool
        .execute(json!({"task_id": "t-detail"}), &ctx)
        .await
        .expect("detail");

    assert!(out.content.contains("Task: t-detail"));
    assert!(out.content.contains("Status: [x] failed"));
    assert!(out.content.contains("Task Prompt:\ndo the thing"));
    assert!(out.content.contains("Result:\npartial result"));
    assert!(out.content.contains("Report Status: truncated"));
    assert!(
        out.content
            .contains("Output File (full untruncated report):\nlog/subagents/t-detail.md")
    );
    assert!(out.content.contains("Error:\nboom"));
}

#[tokio::test]
async fn detail_mode_miss_reports_not_found() {
    let mgr = manager(4);
    let mut ctx = base_ctx();
    ctx.agent_manager = Some(mgr);

    let out = ListAgentsTool
        .execute(json!({"task_id": "absent"}), &ctx)
        .await
        .expect("detail miss");
    assert_eq!(out.content, "Task 'absent' not found.");
    assert!(out.metadata.is_none());
}

#[tokio::test]
async fn detail_mode_complete_report_status_is_silent() {
    let mgr = manager(4);
    let mut entry = entry_full("t-complete", TaskStatus::Completed, "p", true, false);
    entry.completed_at = Some(entry.created_at);
    entry.report_status = ReportStatus::Complete;
    mgr.seed_completed_for_test(entry).await;

    let mut ctx = base_ctx();
    ctx.agent_manager = Some(mgr);

    let out = ListAgentsTool
        .execute(json!({"task_id": "t-complete"}), &ctx)
        .await
        .expect("detail");
    // A completed run renders a fixed duration with no "(still running)" suffix.
    assert!(out.content.contains("Duration: 0s"));
    assert!(!out.content.contains("(still running)"));
    // The detail view stays clean for a normally-completed report.
    assert!(!out.content.contains("Report Status:"));
}

#[tokio::test]
async fn every_status_emoji_is_rendered_in_the_detail_view() {
    let mgr = manager(4);
    let cases = [
        ("s-running", TaskStatus::Running, "[..]"),
        ("s-completed", TaskStatus::Completed, "[ok]"),
        ("s-failed", TaskStatus::Failed, "[x]"),
        ("s-cancelled", TaskStatus::Cancelled, "[blocked]"),
        ("s-suspended", TaskStatus::Suspended, "[||]"),
        ("s-terminating", TaskStatus::Terminating, "[skull]"),
    ];
    for (id, status, _) in &cases {
        mgr.seed_completed_for_test(entry_full(id, status.clone(), "p", true, false))
            .await;
    }

    let mut ctx = base_ctx();
    ctx.agent_manager = Some(mgr);

    for (id, status, emoji) in &cases {
        let out = ListAgentsTool
            .execute(json!({"task_id": id}), &ctx)
            .await
            .expect("detail");
        let expected = format!("Status: {emoji} {}", status.as_str());
        assert!(
            out.content.contains(&expected),
            "task {id} should render `{expected}`; got:\n{}",
            out.content
        );
    }
}
