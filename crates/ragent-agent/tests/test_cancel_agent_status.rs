//! `cancel_agent` reports the status a task is *actually* in
//! (spec `agentssetup` FR-007, FR-013, FR-040).
//!
//! A cancel request is not a cancellation: the registry keeps running the task
//! until its loop reaches a safe point. These tests pin the three outcomes the
//! tool can produce:
//!
//! - an accepted request reports `cancel_requested: true` and the entry's
//!   current status, never the word `cancelled` (FR-013);
//! - a request the registry rejects reports the task's real status instead of a
//!   bare error string (FR-040);
//! - every status the tool emits is `TaskStatus::as_str()`, so no tool
//!   re-derives its own vocabulary (FR-007).

use std::path::PathBuf;
use std::sync::Arc;

use ragent_agent::event::EventBus;
use ragent_agent::task::{AgentManager, TaskStatus};
use ragent_agent::tool::cancel_agent::CancelAgentTool;
use ragent_agent::tool::{Tool, ToolContext};
use serde_json::{Value, json};

#[path = "task_support/mod.rs"]
mod task_support;
use task_support::manager;

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

/// Read one metadata key, asserting the metadata is an object.
fn meta<'a>(out: &'a ragent_agent::tool::ToolOutput, key: &str) -> &'a Value {
    let metadata = out.metadata.as_ref().expect("metadata present");
    metadata
        .get(key)
        .unwrap_or_else(|| panic!("metadata must carry `{key}`: {metadata}"))
}

/// A task whose cancel flag still exists: `spawn_background` inserts the flag
/// and the entry synchronously, so the request lands on an accepted call even
/// when the spawned loop is already failing for want of a provider.
///
/// The cancellation this test exercises is the *request*, so the task's
/// eventual terminal status is irrelevant - and the assertion reads the status
/// back through the same `get_task` the tool uses, so the two can never drift.
#[tokio::test]
async fn cancel_reports_the_current_status_not_a_cancellation() {
    let mgr = manager(4);
    let dir = tempfile::tempdir().expect("tempdir");
    let entry = mgr
        .spawn_background("parent", "explore", "do it", None, dir.path(), None)
        .await
        .expect("spawn accepted");

    let out = CancelAgentTool
        .execute(json!({ "task_id": entry.id }), &ctx_for(&mgr))
        .await
        .expect("an accepted request is a result, not an error");

    // FR-013: the request was accepted ...
    assert_eq!(meta(&out, "cancel_requested"), &json!(true), "{out:?}");
    // ... and it must not claim a terminal state the registry has not reached.
    let observed = mgr.get_task(&entry.id).await;
    let expected = observed.as_ref().map_or("running", |e| e.status.as_str());
    assert_eq!(
        meta(&out, "status"),
        &json!(expected),
        "the reported status must be the entry's real status: {out:?}"
    );
    assert_ne!(
        meta(&out, "status"),
        &json!("cancelled"),
        "a cancel request must never report `cancelled` before the status arrives: {out:?}"
    );
    // The content names the same status, so a model reading prose and a caller
    // reading metadata see one vocabulary (FR-007).
    assert!(
        out.content.contains(expected),
        "the content must name the current status `{expected}`: {}",
        out.content
    );
    assert!(
        out.content.contains("wait_agents"),
        "the content must name how to obtain the terminal status: {}",
        out.content
    );
}

/// A request the registry rejects because the flag is gone reports the status
/// the task is actually in, not a bare "could not cancel" (FR-040).
///
/// The spawned loop removes its cancel flag when it finishes, so a cancel that
/// arrives after the task settled is exactly the shape FR-040 describes: the
/// flag lookup fails, and the tool must name the terminal status it finds
/// rather than leaving the caller with an error string.
#[tokio::test]
async fn cancel_of_a_finished_task_reports_its_real_status() {
    let mgr = manager(4);
    let dir = tempfile::tempdir().expect("tempdir");
    // No provider is configured in the stub processor, so the spawned loop
    // settles as `failed` on its own and drops its cancel flag with it.
    let entry = mgr
        .spawn_background("parent", "explore", "do it", None, dir.path(), None)
        .await
        .expect("spawn accepted");

    let mut settled = None;
    for _ in 0..250 {
        tokio::time::sleep(std::time::Duration::from_millis(20)).await;
        match mgr.get_task(&entry.id).await {
            Some(current) if current.status != TaskStatus::Running => {
                settled = Some(current.status);
                break;
            }
            None => break,
            Some(_) => {}
        }
    }
    let settled = settled.expect("the task settles without a provider");

    let out = CancelAgentTool
        .execute(json!({ "task_id": entry.id }), &ctx_for(&mgr))
        .await
        .expect("a failed delivery is a result carrying a reason, not an escalation");

    assert_eq!(
        meta(&out, "reason"),
        &json!("cancel_failed"),
        "the refusal keeps its machine-readable code: {out:?}"
    );
    assert_eq!(
        meta(&out, "cancel_requested"),
        &json!(false),
        "a rejected request must not claim a cancellation: {out:?}"
    );
    // FR-040: the reported status is the one the registry holds, never the
    // `cancel_failed` reason code standing in for a status.
    assert_eq!(
        meta(&out, "status"),
        &json!(settled.as_str()),
        "the refusal must name the real status, not `cancel_failed`: {out:?}"
    );
    assert_ne!(
        meta(&out, "status"),
        &json!("unknown"),
        "a status the tool CAN read back must be reported: {out:?}"
    );
    assert!(
        out.content
            .contains(&format!("already {}", settled.as_str())),
        "the content must say the task already finished as `{}`: {}",
        settled.as_str(),
        out.content
    );
}

/// The `unknown` refinement is reserved for a status the tool could not read
/// back, and it is the only value the tool emits that is not a `TaskStatus`
/// string (FR-007).
#[test]
fn every_status_cancel_agent_emits_is_the_canonical_vocabulary() {
    let canonical: Vec<&str> = [
        TaskStatus::Running,
        TaskStatus::Completed,
        TaskStatus::Failed,
        TaskStatus::Suspended,
        TaskStatus::Terminating,
        TaskStatus::Cancelled,
        TaskStatus::TimedOut,
    ]
    .iter()
    .map(TaskStatus::as_str)
    .collect();

    // The source of truth is `TaskStatus::as_str`; the tool must not carry a
    // second copy of the vocabulary.
    let source = include_str!("../src/tool/cancel_agent.rs");
    for status in &canonical {
        // No hard-coded status literal in the tool body: every reported status
        // comes from `as_str()`.
        assert!(
            !source.contains(&format!("\"{status}\"")),
            "`{status}` must come from `TaskStatus::as_str()`, not a literal"
        );
    }
    assert!(
        source.contains("STATUS_UNKNOWN: &str = \"unknown\""),
        "the one documented refinement must stay declared in one place"
    );
}

/// The cancellation claim is gone from every success path, so the vocabulary
/// cannot regress.
#[test]
fn the_tool_no_longer_claims_a_cancellation() {
    let source = include_str!("../src/tool/cancel_agent.rs");
    assert!(
        !source.contains("has been cancelled"),
        "an accepted request is not a cancellation (FR-013)"
    );
    assert!(
        source.contains("cancel_requested"),
        "the accepted request must report `cancel_requested`"
    );
}
