//! Tests-only coverage for the `spec_task_update` tool's session-tracker mirror
//! (`ragent_agent::tool::spec_task_update::SpecTaskUpdateTool`).
//!
//! The existing `test_spec_tools.rs` covers the not-configured branch and a
//! pending -> completed transition with no storage. The gap is the mirror path
//! in `mirror_session_task`: with a real in-memory `Storage` wired into the
//! context, the first call creates a tracker task (`created`) and the second
//! updates it (`updated`); with no storage the tool still succeeds but notes the
//! tracker is unavailable (`None`). Missing-parameter, unknown-status,
//! missing-manager, invalid-id, and unknown-spec errors are covered too.

use std::sync::Arc;

use ragent_agent::storage::Storage;
use ragent_agent::tool::{Tool, ToolContext, spec_task_update::SpecTaskUpdateTool};
use ragent_specs::{SpecIo, SpecManager};
use serde_json::json;

fn base_ctx() -> ToolContext {
    use ragent_agent::event::EventBus;
    use std::path::PathBuf;
    ToolContext {
        session_id: "session-1".to_string(),
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

/// Create a one-task spec (`T-001`) under a fresh temp specs root and return the
/// matching `SpecManager`.
async fn spec_manager_with_task() -> (SpecManager, tempfile::TempDir) {
    let tmp = tempfile::tempdir().expect("tempdir");
    let specs_root = tmp.path();

    let id = ragent_specs::spec::SpecId::new("testspec").expect("valid id");
    let spec_md = "---\nstatus: draft\n---\n\n# Test Spec\n";
    let plan_md = "# Plan\n\n## Tasks\n\n| ID | Title | Requirement | Effort | Priority | Dependencies |\n|----|-------|-------------|--------|----------|--------------|\n| T-001 | Do X | — | S | High | — |\n";
    SpecIo::create_spec_dir(specs_root, &id, spec_md, plan_md)
        .await
        .expect("create spec dir");

    (SpecManager::new(specs_root), tmp)
}

/// Look up the tracker task row tagged with `spec_id`/`spec_task_id`.
fn find_mirror(
    storage: &Storage,
    session_id: &str,
    spec_id: &str,
    spec_task_id: &str,
) -> Option<ragent_agent::storage::TaskRow> {
    storage
        .list_tasks(session_id, None)
        .expect("list tasks")
        .into_iter()
        .find(|row| {
            serde_json::from_str::<serde_json::Value>(&row.metadata).is_ok_and(|meta| {
                meta["spec_id"].as_str() == Some(spec_id)
                    && meta["spec_task_id"].as_str() == Some(spec_task_id)
            })
        })
}

#[tokio::test]
async fn mirror_creates_then_updates_the_session_task() {
    let (mgr, _tmp) = spec_manager_with_task().await;
    let storage = Arc::new(Storage::open_in_memory().expect("in-memory storage"));
    // The `todos` table has a foreign key on `sessions`, so the tracker session
    // must exist before a mirror task can be created.
    storage
        .create_session("session-1", "target/temp")
        .expect("create session");

    let mut ctx = base_ctx();
    ctx.spec_manager = Some(Arc::new(mgr));
    ctx.storage = Some(storage.clone());

    let tool = SpecTaskUpdateTool;

    // First call: no tracker task exists, so the mirror creates one.
    let created = tool
        .execute(
            json!({"spec_id": "testspec", "task_id": "T-001", "status": "in_progress"}),
            &ctx,
        )
        .await
        .expect("first update");
    assert!(
        created.content.contains("synced (created)"),
        "content: {}",
        created.content
    );

    let row = find_mirror(&storage, "session-1", "testspec", "T-001").expect("mirror created");
    assert_eq!(row.status, "in_progress");
    assert_eq!(row.title, "Do X");

    // Second call: the tracker task now exists, so the mirror updates it.
    let updated = tool
        .execute(
            json!({"spec_id": "testspec", "task_id": "T-001", "status": "completed"}),
            &ctx,
        )
        .await
        .expect("second update");
    assert!(
        updated.content.contains("synced (updated)"),
        "content: {}",
        updated.content
    );

    let row = find_mirror(&storage, "session-1", "testspec", "T-001").expect("mirror present");
    assert_eq!(row.status, "completed");
}

#[tokio::test]
async fn mirror_blocked_status_maps_to_pending_in_tracker() {
    let (mgr, _tmp) = spec_manager_with_task().await;
    let storage = Arc::new(Storage::open_in_memory().expect("in-memory storage"));
    storage
        .create_session("session-1", "target/temp")
        .expect("create session");

    let mut ctx = base_ctx();
    ctx.spec_manager = Some(Arc::new(mgr));
    ctx.storage = Some(storage.clone());

    let out = SpecTaskUpdateTool
        .execute(
            json!({"spec_id": "testspec", "task_id": "T-001", "status": "blocked"}),
            &ctx,
        )
        .await
        .expect("blocked update");
    assert!(out.content.contains("synced (created)"), "{}", out.content);

    // The tracker has no `blocked` status, so a blocked spec task seeds pending.
    let row = find_mirror(&storage, "session-1", "testspec", "T-001").expect("mirror present");
    assert_eq!(row.status, "pending");
}

#[tokio::test]
async fn mirror_without_storage_warns_but_succeeds() {
    let (mgr, _tmp) = spec_manager_with_task().await;

    let mut ctx = base_ctx();
    ctx.spec_manager = Some(Arc::new(mgr));
    // storage stays None: the spec write succeeds, the mirror is unavailable.
    ctx.storage = None;

    let out = SpecTaskUpdateTool
        .execute(
            json!({"spec_id": "testspec", "task_id": "T-001", "status": "completed"}),
            &ctx,
        )
        .await
        .expect("update without storage");
    assert!(
        out.content.contains("tracker unavailable or mirror failed"),
        "content: {}",
        out.content
    );
}

#[tokio::test]
async fn missing_parameters_error() {
    let tool = SpecTaskUpdateTool;

    let no_spec = tool
        .execute(
            json!({"task_id": "T-001", "status": "completed"}),
            &base_ctx(),
        )
        .await
        .expect_err("missing spec_id");
    assert!(no_spec.to_string().contains("spec_id"), "err: {no_spec}");

    let no_task = tool
        .execute(
            json!({"spec_id": "testspec", "status": "completed"}),
            &base_ctx(),
        )
        .await
        .expect_err("missing task_id");
    assert!(no_task.to_string().contains("task_id"), "err: {no_task}");

    let no_status = tool
        .execute(
            json!({"spec_id": "testspec", "task_id": "T-001"}),
            &base_ctx(),
        )
        .await
        .expect_err("missing status");
    assert!(no_status.to_string().contains("status"), "err: {no_status}");
}

#[tokio::test]
async fn unknown_status_error() {
    let err = SpecTaskUpdateTool
        .execute(
            json!({"spec_id": "testspec", "task_id": "T-001", "status": "nonsense"}),
            &base_ctx(),
        )
        .await
        .expect_err("unknown status");
    assert!(
        err.to_string().contains("Unknown task status"),
        "err: {err}"
    );
}

#[tokio::test]
async fn missing_spec_manager_error() {
    let err = SpecTaskUpdateTool
        .execute(
            json!({"spec_id": "testspec", "task_id": "T-001", "status": "completed"}),
            &base_ctx(),
        )
        .await
        .expect_err("no spec manager");
    assert!(
        err.to_string().contains("Spec manager is not configured"),
        "err: {err}"
    );
}

#[tokio::test]
async fn invalid_spec_id_error() {
    let (mgr, _tmp) = spec_manager_with_task().await;
    let mut ctx = base_ctx();
    ctx.spec_manager = Some(Arc::new(mgr));

    let err = SpecTaskUpdateTool
        .execute(
            json!({"spec_id": "bad/id", "task_id": "T-001", "status": "completed"}),
            &ctx,
        )
        .await
        .expect_err("invalid spec id");
    assert!(err.to_string().contains("Invalid spec ID"), "err: {err}");
}

#[tokio::test]
async fn unknown_spec_error() {
    let (mgr, _tmp) = spec_manager_with_task().await;
    let mut ctx = base_ctx();
    ctx.spec_manager = Some(Arc::new(mgr));

    let err = SpecTaskUpdateTool
        .execute(
            json!({"spec_id": "missing", "task_id": "T-001", "status": "completed"}),
            &ctx,
        )
        .await
        .expect_err("unknown spec");
    assert!(
        err.to_string().contains("Failed to read spec"),
        "err: {err}"
    );
}
