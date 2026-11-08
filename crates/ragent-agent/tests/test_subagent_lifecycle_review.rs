//! Regression tests for sub-agent lifecycle bugs found in the agents review.
//!
//! - `drain_completed` must not drain (inject) or reap a `suspended` or
//!   `terminating` background task: both are still live, and reaping one made
//!   a later `resume_task` fail with "not found" and lost the real result.
//! - `kill_task` must refuse a `timed_out` task like any other finished task,
//!   instead of re-marking it `terminating` and force-failing it 10 s later.
//! - `wait_agents` with `task_ids` omitted must wait for RUNNING background
//!   tasks, not only collect the ones that already settled.

use std::path::PathBuf;
use std::sync::Arc;

use ragent_agent::event::EventBus;
use ragent_agent::task::{AgentManager, TaskStatus};
use ragent_agent::tool::wait_agents::WaitAgentsTool;
use ragent_agent::tool::{Tool, ToolContext};
use serde_json::json;

#[path = "task_support/mod.rs"]
mod task_support;
use task_support::{entry_full, manager};

/// A `ToolContext` wired to `mgr`, owned by session `parent`.
fn ctx_for(mgr: &Arc<AgentManager>) -> ToolContext {
    ToolContext {
        session_id: "parent".to_string(),
        working_dir: PathBuf::from("target/temp"),
        event_bus: Arc::new(EventBus::new(16)),
        storage: None,
        agent_manager: Some(Arc::clone(mgr)),
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
        tool_registry: ToolContext::default_tool_registry(),
        cached_team_dir: Arc::new(std::sync::Mutex::new(None)),
        permission_checker: None,
        read_timestamps: Arc::new(std::sync::RwLock::new(std::collections::HashMap::new())),
        canonical_cache: Arc::new(ragent_tools_core::CanonicalPathCache::new()),
    }
}

#[test]
fn test_task_status_is_terminal_classification() {
    for status in [
        TaskStatus::Completed,
        TaskStatus::Failed,
        TaskStatus::Cancelled,
        TaskStatus::TimedOut,
    ] {
        assert!(status.is_terminal(), "{status} must be terminal");
    }
    for status in [
        TaskStatus::Running,
        TaskStatus::Suspended,
        TaskStatus::Terminating,
    ] {
        assert!(!status.is_terminal(), "{status} must be live");
    }
}

#[tokio::test]
async fn test_drain_completed_keeps_suspended_task_resumable() {
    let mgr = manager(4);
    mgr.insert_task_for_test(entry_full(
        "explore-susp",
        TaskStatus::Suspended,
        "parent",
        true,
        false,
    ));
    mgr.set_pending_background_for_test();

    let drained = mgr.drain_completed("parent").await;
    assert!(
        drained.is_empty(),
        "a suspended task is not finished and must not be injected: {drained:?}"
    );
    let entry = mgr
        .get_task("explore-susp")
        .await
        .expect("a suspended task must not be reaped from the registry");
    assert_eq!(entry.status, TaskStatus::Suspended);
    assert!(
        !entry.reported,
        "a suspended task must not be marked reported"
    );

    mgr.resume_task("explore-susp")
        .await
        .expect("a suspended task must stay resumable after a drain");
    assert_eq!(
        mgr.get_task("explore-susp").await.map(|e| e.status),
        Some(TaskStatus::Running)
    );
}

#[tokio::test]
async fn test_drain_completed_keeps_terminating_task() {
    let mgr = manager(4);
    mgr.insert_task_for_test(entry_full(
        "explore-term",
        TaskStatus::Terminating,
        "parent",
        true,
        false,
    ));
    mgr.set_pending_background_for_test();

    assert!(mgr.drain_completed("parent").await.is_empty());
    assert!(
        mgr.get_task("explore-term").await.is_some(),
        "a terminating task has not settled and must not be reaped"
    );
}

#[tokio::test]
async fn test_kill_task_refuses_timed_out_task() {
    let mgr = manager(4);
    mgr.insert_task_for_test(entry_full(
        "explore-to",
        TaskStatus::TimedOut,
        "parent",
        true,
        false,
    ));

    let err = mgr
        .kill_task("explore-to")
        .await
        .expect_err("killing a timed-out task must be refused");
    assert!(err.to_string().contains("already finished"), "{err}");
    assert_eq!(
        mgr.get_task("explore-to").await.map(|e| e.status),
        Some(TaskStatus::TimedOut),
        "a refused kill must not re-status the task"
    );
}

#[tokio::test]
async fn test_wait_agents_without_task_ids_waits_for_running_task() {
    let mgr = manager(4);
    mgr.insert_task_for_test(entry_full(
        "explore-run",
        TaskStatus::Running,
        "parent",
        true,
        false,
    ));

    // Finish the task shortly after the wait starts; the wait must still be
    // open to collect it.
    let finisher = {
        let mgr = Arc::clone(&mgr);
        tokio::spawn(async move {
            tokio::time::sleep(std::time::Duration::from_millis(200)).await;
            let mut done = entry_full("explore-run", TaskStatus::Completed, "parent", true, false);
            done.result = Some(Arc::from("FINDINGS-42"));
            mgr.seed_completed_for_test(done).await;
        })
    };

    let out = WaitAgentsTool
        .execute(json!({ "timeout_secs": 2 }), &ctx_for(&mgr))
        .await
        .expect("wait_agents succeeds");
    finisher.await.expect("finisher task");

    assert!(
        !out.content.contains("No running background tasks"),
        "omitting task_ids must wait for the running task: {}",
        out.content
    );
    assert!(
        out.content.contains("FINDINGS-42"),
        "the running task's result must be collected: {}",
        out.content
    );
}

#[tokio::test]
async fn test_wait_agents_without_task_ids_skips_suspended_task() {
    let mgr = manager(4);
    mgr.insert_task_for_test(entry_full(
        "explore-park",
        TaskStatus::Suspended,
        "parent",
        true,
        false,
    ));

    let started = std::time::Instant::now();
    let out = WaitAgentsTool
        .execute(json!({ "timeout_secs": 5 }), &ctx_for(&mgr))
        .await
        .expect("wait_agents succeeds");

    assert!(
        started.elapsed() < std::time::Duration::from_secs(4),
        "a parked task must not hold an omit-task_ids wait open to the deadline"
    );
    assert!(
        out.content.contains("No running background tasks"),
        "{}",
        out.content
    );
}

#[tokio::test]
async fn test_cancel_all_reaches_suspended_task() {
    let mgr = manager(4);
    let flag = mgr.insert_task_for_test(entry_full(
        "explore-parked",
        TaskStatus::Suspended,
        "parent",
        true,
        false,
    ));

    mgr.cancel_all("parent").await;

    assert!(
        flag.load(std::sync::atomic::Ordering::Relaxed),
        "a parked task must be cancelled on session cleanup, not left parked forever"
    );
}

#[tokio::test]
async fn test_finished_foreground_task_is_reaped_by_next_drain() {
    let mgr = manager(4);
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../target/temp/fg-reap");
    std::fs::create_dir_all(&dir).expect("scratch dir");

    // No provider is configured, so the run fails - exercising the real
    // `spawn_sync` terminal bookkeeping without a live model.
    mgr.spawn_sync("parent", "general", "noop", None, &dir, None)
        .await
        .expect_err("a run with no provider fails");

    let finished = mgr.list_agents("parent").await;
    assert_eq!(
        finished.len(),
        1,
        "the finished run is listed until drained"
    );
    let task_id = finished[0].id.clone();
    assert!(finished[0].status.is_terminal());
    assert!(finished[0].reported, "the caller got the outcome inline");

    let drained = mgr.drain_completed("parent").await;
    assert!(
        drained.is_empty(),
        "a foreground run is never injected as a background result"
    );
    assert!(
        mgr.get_task(&task_id).await.is_none(),
        "a finished foreground run must not stay in the registry forever"
    );
}

#[tokio::test]
async fn test_pending_flag_stays_armed_while_background_task_runs() {
    let mgr = manager(4);
    mgr.insert_task_for_test(entry_full(
        "explore-live",
        TaskStatus::Running,
        "parent",
        true,
        false,
    ));
    mgr.set_pending_background_for_test();

    assert!(mgr.drain_completed("parent").await.is_empty());
    assert!(
        mgr.has_pending_background(),
        "a drain while a background task is live must not disarm the flag, or \
         its result is never drained"
    );
}

#[tokio::test]
async fn test_pending_flag_kept_for_other_parents_pending_result() {
    let mgr = manager(4);
    mgr.insert_task_for_test(entry_full(
        "explore-other",
        TaskStatus::Completed,
        "other-parent",
        true,
        false,
    ));
    mgr.set_pending_background_for_test();

    assert!(mgr.drain_completed("parent").await.is_empty());
    assert!(
        mgr.has_pending_background(),
        "draining one parent must not disarm the flag under another's result"
    );
    assert_eq!(mgr.drain_completed("other-parent").await.len(), 1);
    assert!(!mgr.has_pending_background());
}
