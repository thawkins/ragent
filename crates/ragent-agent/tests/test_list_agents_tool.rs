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
async fn status_filter_selects_a_suspended_task() {
    // FR-030 / FR-007: a caller that needs to free a slot must be able to find
    // the suspended task holding it. The filter used to declare only
    // running/completed/failed/cancelled while the registry also emits
    // `suspended`, `terminating` and `timed_out`, so the row could not be
    // selected. Both newly declared values are exercised here.
    let mgr = manager(4);
    mgr.seed_completed_for_test(entry_full(
        "parked",
        TaskStatus::Suspended,
        "parent",
        true,
        false,
    ))
    .await;
    mgr.seed_completed_for_test(entry_full(
        "bounded",
        TaskStatus::TimedOut,
        "parent",
        true,
        false,
    ))
    .await;
    mgr.seed_completed_for_test(entry_full(
        "live",
        TaskStatus::Running,
        "parent",
        true,
        false,
    ))
    .await;

    let mut ctx = base_ctx();
    ctx.agent_manager = Some(mgr);

    let suspended = ListAgentsTool
        .execute(json!({"status": "suspended"}), &ctx)
        .await
        .expect("a suspended filter must be accepted");
    assert!(suspended.content.contains("parked"));
    assert!(!suspended.content.contains("bounded"));
    let meta = suspended.metadata.expect("metadata");
    assert_eq!(meta["task_count"], 1);

    let timed_out = ListAgentsTool
        .execute(json!({"status": "timed_out"}), &ctx)
        .await
        .expect("a timed_out filter must be accepted");
    assert!(timed_out.content.contains("bounded"));
    assert_eq!(timed_out.metadata.expect("metadata")["task_count"], 1);

    // Every value the schema declares is accepted by the validator, so the
    // schema and the refusal list cannot drift apart.
    let declared = ListAgentsTool.parameters_schema()["properties"]["status"]["enum"]
        .as_array()
        .expect("enum")
        .iter()
        .filter_map(|v| v.as_str())
        .map(str::to_string)
        .collect::<Vec<_>>();
    for value in &declared {
        ListAgentsTool
            .execute(json!({ "status": value }), &ctx)
            .await
            .unwrap_or_else(|e| panic!("declared status `{value}` was refused: {e}"));
    }
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
    assert!(
        out.content.starts_with("Task 'absent' not found."),
        "content: {}",
        out.content
    );
    // FR-010: the miss is a branchable outcome, so it names its reason.
    assert_eq!(
        out.metadata.as_ref().and_then(|m| m.get("reason")),
        Some(&json!("task_not_found"))
    );
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

/// FR-008: a short id is produced by `ragent_types::strutil::short_prefix`,
/// never by slicing the string at a byte offset.
///
/// The id below is `"a"` followed by nine `U+00E9` characters, so byte 8 lands
/// *inside* the fourth codepoint (each is 2 bytes). The previous
/// `&task.id[..8]` would therefore panic the tool on a non-ASCII id; the shared
/// helper takes whole characters instead, yielding `"a"` plus three `e-acute`
/// (4 characters, well under the 8-byte cut).
#[tokio::test]
async fn list_mode_abbreviates_a_multibyte_id_without_panicking() {
    let mgr = manager(4);
    let id = format!("a{}", "\u{e9}".repeat(9));
    mgr.seed_completed_for_test(entry_full(&id, TaskStatus::Running, "parent", true, false))
        .await;

    let mut ctx = base_ctx();
    ctx.agent_manager = Some(mgr);

    let out = ListAgentsTool
        .execute(json!({}), &ctx)
        .await
        .expect("a multi-byte task id must not panic the renderer");

    let expected_short = format!("a{}", "\u{e9}".repeat(3));
    assert!(
        out.content.contains(&expected_short),
        "expected the char-aligned short id {expected_short:?} in:\n{}",
        out.content
    );
    // The full id must not leak into the abbreviated ID column.
    assert!(
        !out.content.contains(&format!("| {id} |")),
        "the row must use the abbreviated id, not the full id:\n{}",
        out.content
    );
}

/// Every refusal a caller could branch on carries a stable machine-readable
/// reason code (spec `agentssetup` FR-010), extracted through the one shared
/// helper so the assertion does not depend on the English wording.
#[tokio::test]
async fn refusals_carry_reason_codes() {
    use ragent_agent::tool::param_schema::reason_in;

    // A status filter outside the schema's declared set is refused, not treated
    // as "matches nothing".
    let err = ListAgentsTool
        .execute(json!({ "status": "sleeping" }), &base_ctx())
        .await
        .expect_err("an unsupported status filter must be refused");
    assert_eq!(reason_in(&err.to_string()), Some("invalid_status_filter"));
    assert!(
        err.to_string().contains("'sleeping'"),
        "the refusal must name the offending value: {err}"
    );

    // The missing-manager case names its own reason.
    let err = ListAgentsTool
        .execute(json!({}), &base_ctx())
        .await
        .expect_err("no manager must error");
    assert_eq!(reason_in(&err.to_string()), Some("manager_unavailable"));

    // A detail lookup that misses is a result, so the reason rides in metadata.
    let manager = manager(8);
    let mut ctx = base_ctx();
    ctx.agent_manager = Some(manager);
    let out = ListAgentsTool
        .execute(json!({ "task_id": "explore-nope" }), &ctx)
        .await
        .expect("a missed lookup is a result, not an error");
    assert_eq!(
        out.metadata.as_ref().and_then(|m| m.get("reason")),
        Some(&json!("task_not_found"))
    );
}

/// Reads `limit` / `occupancy` / `headroom` out of a `list_agents` result's
/// metadata as `(limit, occupancy, headroom)`, asserting all three are present.
fn limit_triple(metadata: &serde_json::Value) -> (u64, u64, u64) {
    let field = |name: &str| {
        metadata
            .get(name)
            .and_then(serde_json::Value::as_u64)
            .unwrap_or_else(|| panic!("metadata must carry `{name}`: {metadata}"))
    };
    (field("limit"), field("occupancy"), field("headroom"))
}

/// FR-011: the limit, its occupancy and the resulting headroom are reported in
/// both the content and the metadata, and the headroom tracks a live task.
#[tokio::test]
async fn limit_occupancy_and_headroom_are_reported() {
    let mgr = manager(2);
    let mut ctx = base_ctx();
    ctx.agent_manager = Some(mgr.clone());

    let idle = ListAgentsTool.execute(json!({}), &ctx).await.expect("idle");
    let idle_meta = idle.metadata.as_ref().expect("metadata");
    assert_eq!(limit_triple(idle_meta), (2, 0, 2));
    assert!(
        idle.content.contains("Background limit: 0/2"),
        "the content must carry the limit line:\n{}",
        idle.content
    );

    mgr.seed_completed_for_test(entry_full(
        "busy",
        TaskStatus::Running,
        "parent",
        true,
        false,
    ))
    .await;

    let one = ListAgentsTool.execute(json!({}), &ctx).await.expect("one");
    let one_meta = one.metadata.as_ref().expect("metadata");
    assert_eq!(limit_triple(one_meta), (2, 1, 1));
    assert_eq!(
        one_meta.get("occupying_task_ids"),
        Some(&json!(["busy"])),
        "the occupying task ids ride in the metadata"
    );
    assert!(one.content.contains("Background limit: 1/2"));
    assert!(one.content.contains("headroom 1"));
}

/// FR-035: a status filter narrows the listing, never the limit report - the
/// three numbers still describe the whole session even when the filtered list
/// is empty, and the empty listing keeps its `no_tasks` reason.
#[tokio::test]
async fn limit_report_composes_with_a_status_filter() {
    let mgr = manager(2);
    mgr.seed_completed_for_test(entry_full(
        "busy",
        TaskStatus::Running,
        "parent",
        true,
        false,
    ))
    .await;

    let mut ctx = base_ctx();
    ctx.agent_manager = Some(mgr);

    let filtered = ListAgentsTool
        .execute(json!({ "status": "completed" }), &ctx)
        .await
        .expect("filtered listing");

    assert!(
        filtered
            .content
            .contains("No tasks with status 'completed' found."),
        "the filtered listing is empty:\n{}",
        filtered.content
    );
    let meta = filtered.metadata.as_ref().expect("metadata");
    // Whole-session numbers, not the empty filtered subset.
    assert_eq!(limit_triple(meta), (2, 1, 1));
    assert_eq!(meta.get("reason"), Some(&json!("no_tasks")));
    assert_eq!(meta.get("count"), Some(&json!(0)));
    // The content carries the numbers too, so a model reading only prose sees
    // the real headroom.
    assert!(filtered.content.contains("Background limit: 1/2"));
}

/// FR-011: detail mode reports the same three numbers on both its hit and its
/// miss path, so a caller never has to infer the limit state from the branch.
#[tokio::test]
async fn detail_mode_reports_the_limit_on_hit_and_miss() {
    let mgr = manager(2);
    mgr.seed_completed_for_test(entry_full(
        "bg-live",
        TaskStatus::Running,
        "parent",
        true,
        false,
    ))
    .await;

    let mut ctx = base_ctx();
    ctx.agent_manager = Some(mgr);

    let hit = ListAgentsTool
        .execute(json!({ "task_id": "bg-live" }), &ctx)
        .await
        .expect("detail hit");
    assert_eq!(
        limit_triple(hit.metadata.as_ref().expect("metadata")),
        (2, 1, 1)
    );
    assert!(hit.content.contains("Background limit: 1/2"));

    let miss = ListAgentsTool
        .execute(json!({ "task_id": "nope" }), &ctx)
        .await
        .expect("detail miss");
    assert_eq!(
        limit_triple(miss.metadata.as_ref().expect("metadata")),
        (2, 1, 1)
    );
    assert_eq!(
        miss.metadata.as_ref().and_then(|m| m.get("reason")),
        Some(&json!("task_not_found"))
    );
}

// -- FR-012: waiter visibility (T-014) ---------------------------------------

/// FR-012: an awaited live task reports its waiter count in the detail view
/// and in the metadata.
///
/// The count comes from the registry's own tally, so the test drives the real
/// [`ragent_agent::task::AgentManager::increment_waiter`] path a running
/// `wait_agents` call uses rather than writing the field behind the
/// implementation's back.
#[tokio::test]
async fn detail_mode_reports_the_waiter_count_of_an_awaited_live_task() {
    let mgr = manager(4);
    mgr.seed_completed_for_test(entry_full(
        "t-awaited",
        TaskStatus::Running,
        "parent",
        true,
        false,
    ))
    .await;
    assert!(
        mgr.increment_waiter("t-awaited").await,
        "a running task must accept a waiter increment"
    );
    assert!(mgr.increment_waiter("t-awaited").await);
    // A suspension is not a finish: the callers stay queued on the task.
    mgr.suspend_task("t-awaited").await.expect("suspend");

    let mut ctx = base_ctx();
    ctx.agent_manager = Some(mgr);

    let out = ListAgentsTool
        .execute(json!({ "task_id": "t-awaited" }), &ctx)
        .await
        .expect("detail");

    assert!(
        out.content.contains("Active Waiters: 2"),
        "the detail view must name both waiters:\n{}",
        out.content
    );
    assert_eq!(
        out.metadata.as_ref().and_then(|m| m.get("waiter_count")),
        Some(&json!(2)),
        "the metadata must carry the same count the content shows"
    );
}

/// FR-012: the waiter count appears only when it is greater than zero, and the
/// detail view stays clean for a task nobody awaits.
#[tokio::test]
async fn detail_mode_omits_the_waiter_count_when_nobody_is_waiting() {
    let mgr = manager(4);
    mgr.seed_completed_for_test(entry_full(
        "t-unawaited",
        TaskStatus::Running,
        "parent",
        true,
        false,
    ))
    .await;

    let mut ctx = base_ctx();
    ctx.agent_manager = Some(mgr);

    let out = ListAgentsTool
        .execute(json!({ "task_id": "t-unawaited" }), &ctx)
        .await
        .expect("detail");

    assert!(
        !out.content.contains("Active Waiters"),
        "a task nobody awaits must not render a `0` waiter line:\n{}",
        out.content
    );
    assert!(
        out.metadata
            .as_ref()
            .is_none_or(|m| m.get("waiter_count").is_none()),
        "the metadata must not claim a waiter count nobody holds"
    );
}

/// FR-012: after the wait returns the count is back to `0`, and the detail
/// view stops reporting it.
///
/// This is the second half of the spec's scenario: an in-flight wait is
/// visible, a finished one leaves no trace on the task.
#[tokio::test]
async fn detail_mode_drops_the_waiter_count_after_the_wait_returns() {
    let mgr = manager(4);
    mgr.seed_completed_for_test(entry_full(
        "t-returned",
        TaskStatus::Running,
        "parent",
        true,
        false,
    ))
    .await;
    assert!(mgr.increment_waiter("t-returned").await);

    let mut ctx = base_ctx();
    ctx.agent_manager = Some(mgr.clone());

    let during = ListAgentsTool
        .execute(json!({ "task_id": "t-returned" }), &ctx)
        .await
        .expect("detail while awaited");
    assert!(during.content.contains("Active Waiters: 1"));

    mgr.decrement_waiter("t-returned").await;

    let after = ListAgentsTool
        .execute(json!({ "task_id": "t-returned" }), &ctx)
        .await
        .expect("detail after the wait");
    assert!(
        !after.content.contains("Active Waiters"),
        "the count must be gone once the wait returned:\n{}",
        after.content
    );
}

/// FR-012: a listing row names the waiters of a live task, and only those.
#[tokio::test]
async fn list_mode_marks_only_the_awaited_live_task() {
    let mgr = manager(8);
    for id in ["t-held", "t-idle"] {
        mgr.seed_completed_for_test(entry_full(id, TaskStatus::Running, "parent", true, false))
            .await;
    }
    assert!(mgr.increment_waiter("t-held").await);

    let mut ctx = base_ctx();
    ctx.agent_manager = Some(mgr);

    let out = ListAgentsTool
        .execute(json!({ "status": "running" }), &ctx)
        .await
        .expect("list");

    // Exactly one waiter line, and it follows the awaited task's row.
    assert_eq!(
        out.content.matches("active waiter(s)").count(),
        1,
        "only the awaited row may carry a waiter line:\n{}",
        out.content
    );
    let held_at = out.content.find("t-held").expect("held row");
    let waiter_at = out.content.find("1 active waiter(s)").expect("waiter line");
    assert!(
        waiter_at > held_at,
        "the waiter line must follow its own row:\n{}",
        out.content
    );
}

/// FR-012: a finished task's leftover tally is not painted as a live queue.
#[tokio::test]
async fn a_finished_task_never_reports_waiters() {
    let mgr = manager(4);
    let mut entry = entry_full("t-done", TaskStatus::Completed, "parent", true, false);
    entry.completed_at = Some(entry.created_at);
    // A completed task cannot accept a waiter increment, so seed the residue
    // directly: this is the state a racing increment leaves behind.
    entry.waiter_count = 1;
    mgr.seed_completed_for_test(entry).await;

    let mut ctx = base_ctx();
    ctx.agent_manager = Some(mgr);

    let detail = ListAgentsTool
        .execute(json!({ "task_id": "t-done" }), &ctx)
        .await
        .expect("detail");
    assert!(
        !detail.content.contains("Active Waiters"),
        "a finished task must not report waiters:\n{}",
        detail.content
    );
}
