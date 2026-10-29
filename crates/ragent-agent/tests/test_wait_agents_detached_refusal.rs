//! Tests-only coverage for FR-044: asking `wait_agents` to await a DETACHED
//! (fire-and-forget) task is refused with an error that names the contract,
//! rather than reporting the id as unknown.
//!
//! A detached task - `new_agent` with `background: true, detached: true` - is
//! deliberately excluded from `list_agents` and from this tool's wait set: it
//! belongs to a fire-and-forget consumer (`/spawn`, future cron paths), its
//! completion is never injected into the session, and nothing will ever notify
//! a waiter. A wait on such an id therefore cannot settle. Before this change
//! the id fell through the settled-set scan and the caller was told there were
//! "no running background tasks to wait for", which is both wrong (the task
//! exists) and unusable (it names no way forward).
//!
//! The requirement is a *contract-naming* refusal, so the tests assert the
//! substance rather than the prose: the error says the task is detached, names
//! the task, and points at the durable report file - the one surface through
//! which the output can still be recovered.
//!
//! The tests seed the registry directly, because a detached task's report is
//! written by a live sub-agent run; the condition under test is the refusal,
//! and the seeded entry is exactly what the registry holds after a `/spawn`.

use std::path::PathBuf;
use std::sync::Arc;

use ragent_agent::event::EventBus;
use ragent_agent::task::{AgentManager, TaskEntry, TaskStatus};
use ragent_agent::tool::wait_agents::{REASON_TASK_DETACHED, WaitAgentsTool};
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

/// A detached background task as the registry holds it after `/spawn`: it is
/// fire-and-forget, still running (so it has no result yet), and its report
/// path is unknown until it finishes.
fn detached_entry(parent: &str, task_id: &str) -> TaskEntry {
    entry_full(task_id, TaskStatus::Running, parent, true, true)
}

/// FR-044: the refusal names the task as detached and points at the report
/// file instead of calling the id unknown.
#[tokio::test]
async fn awaiting_a_detached_task_is_refused_by_contract() {
    let mgr = manager(4);
    mgr.seed_completed_for_test(detached_entry("parent", "det-0001"))
        .await;

    let ctx = ctx_for("parent", Arc::clone(&mgr));
    let err = WaitAgentsTool
        .execute(json!({"task_ids": ["det-0001"]}), &ctx)
        .await
        .expect_err("FR-044: awaiting a detached task must be refused");

    let message = err.to_string();
    let lowered = message.to_ascii_lowercase();
    assert!(
        lowered.contains("detached"),
        "FR-044: the refusal must name the task as detached: {message}"
    );
    // The contract, not just the adjective: the spawn shape that created it.
    assert!(
        message.contains("background: true, detached: true"),
        "FR-044: the refusal must name the contract that produced the task: {message}"
    );
    assert!(
        message.contains("det-0001"),
        "FR-044: the refusal must name the task, so the caller can act on it: {message}"
    );
    // The way forward: the file that can still be read.
    assert!(
        message.contains("log/subagents/"),
        "FR-044: the refusal must point at the durable report file: {message}"
    );
    assert!(
        message.contains("read"),
        "FR-044: the refusal must say how to recover the output: {message}"
    );
    // It must NOT look like an unknown id, which is what this replaced.
    assert!(
        !lowered.contains("unknown"),
        "FR-044: a real detached id must not be reported as unknown: {message}"
    );
    assert!(
        !lowered.contains("not found"),
        "FR-044: a real detached id must not be reported as not found: {message}"
    );
}

/// The refusal carries the machine-readable reason, so a caller can branch on
/// the contract without parsing English (FR-010 vocabulary, FR-044 substance).
#[tokio::test]
async fn the_detached_refusal_carries_a_reason_code() {
    let mgr = manager(4);
    mgr.seed_completed_for_test(detached_entry("parent", "det-0002"))
        .await;

    let ctx = ctx_for("parent", Arc::clone(&mgr));
    let err = WaitAgentsTool
        .execute(json!({"task_ids": ["det-0002"]}), &ctx)
        .await
        .expect_err("FR-044: awaiting a detached task must be refused");

    assert!(
        err.to_string().contains(REASON_TASK_DETACHED),
        "the refusal must name `{REASON_TASK_DETACHED}`: {err}"
    );
}

/// A detached task in the same list as an awaitable one refuses the whole
/// call: the wait never runs, so it cannot hang on the task it cannot see.
#[tokio::test]
async fn a_detached_id_refuses_the_call_even_beside_an_awaitable_one() {
    let mgr = manager(4);
    let mut settled = entry_full("done-001", TaskStatus::Completed, "parent", true, false);
    settled.result = Some(Arc::from("finished findings"));
    settled.completed_at = Some(settled.created_at);
    mgr.seed_completed_for_test(settled).await;
    mgr.seed_completed_for_test(detached_entry("parent", "det-0003"))
        .await;

    let ctx = ctx_for("parent", Arc::clone(&mgr));
    let err = WaitAgentsTool
        .execute(json!({"task_ids": ["done-001", "det-0003"]}), &ctx)
        .await
        .expect_err("FR-044: a detached id in the list must refuse the call");

    assert!(
        err.to_string().contains("det-0003"),
        "the refusal must name the detached task: {err}"
    );
    // The awaitable task must not have been consumed by a call that failed:
    // its entry is untouched and still awaitable afterwards.
    let out = WaitAgentsTool
        .execute(json!({"task_ids": ["done-001"]}), &ctx)
        .await
        .expect("waiting on the awaitable task must still succeed");
    assert!(
        out.content.contains("finished findings"),
        "the refused call must not have disturbed the other task: {}",
        out.content
    );
}

/// A refused call counts no waiters: the detached task is left exactly as it
/// was, so the refusal has no side effects on the registry (FR-012).
#[tokio::test]
async fn a_refused_await_leaves_the_waiter_count_untouched() {
    let mgr = manager(4);
    mgr.seed_completed_for_test(detached_entry("parent", "det-0004"))
        .await;

    let ctx = ctx_for("parent", Arc::clone(&mgr));
    let _ = WaitAgentsTool
        .execute(json!({"task_ids": ["det-0004"]}), &ctx)
        .await
        .expect_err("FR-044: awaiting a detached task must be refused");

    let entry = mgr
        .get_task("det-0004")
        .await
        .expect("the detached task must still be registered");
    assert_eq!(
        entry.waiter_count, 0,
        "FR-012: a refused wait must not register a waiter"
    );
}

/// The refusal is scoped to detached tasks: the same id shape, non-detached,
/// is still awaited normally. Without this, a refusal implemented too broadly
/// would pass the positive test above but break every real await.
#[tokio::test]
async fn a_background_task_is_still_awaitable() {
    let mgr = manager(4);
    let mut entry = entry_full("bg-0001", TaskStatus::Completed, "parent", true, false);
    entry.result = Some(Arc::from("background findings"));
    entry.completed_at = Some(entry.created_at);
    mgr.seed_completed_for_test(entry).await;

    let ctx = ctx_for("parent", Arc::clone(&mgr));
    let out = WaitAgentsTool
        .execute(json!({"task_ids": ["bg-0001"]}), &ctx)
        .await
        .expect("a background task must remain awaitable");

    assert!(
        out.content.contains("background findings"),
        "the background task's result must be reported: {}",
        out.content
    );
    assert_eq!(
        out.metadata
            .as_ref()
            .and_then(|m| m.get("completed_count"))
            .and_then(serde_json::Value::as_u64),
        Some(1),
        "the background task must be collected as settled: {:?}",
        out.metadata
    );
}

/// A detached task belonging to another session is still refused by contract:
/// it is detached, so the reason it cannot be awaited does not depend on which
/// session asks. The refusal must not silently degrade to a wait that returns
/// nothing.
#[tokio::test]
async fn a_detached_task_from_another_session_is_refused_too() {
    let mgr = manager(4);
    mgr.seed_completed_for_test(detached_entry("other-session", "det-0005"))
        .await;

    let ctx = ctx_for("parent", Arc::clone(&mgr));
    let err = WaitAgentsTool
        .execute(json!({"task_ids": ["det-0005"]}), &ctx)
        .await
        .expect_err("FR-044: a detached task must be refused whatever session asks");

    assert!(
        err.to_string().contains("det-0005"),
        "the refusal must name the detached task: {err}"
    );
}

/// The tool description states the refusal, so the model knows the contract
/// before it calls the tool rather than after (FR-016).
#[test]
fn the_description_states_the_detached_refusal() {
    let description = WaitAgentsTool.description();
    assert!(
        description.contains("DETACHED"),
        "the description must name the detached contract: {description}"
    );
    assert!(
        description.contains("refused"),
        "the description must say the await is refused: {description}"
    );
    assert!(
        description.contains("log/subagents/"),
        "the description must point at the durable report file: {description}"
    );
}
