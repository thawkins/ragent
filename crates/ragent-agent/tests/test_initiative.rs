//! Tests for the `initiative` tool (JCODEPLAN M8 T-070).

use ragent_agent::event::EventBus;
use ragent_agent::storage::{InitiativeMilestone, Storage};
use ragent_agent::tool::{Tool, ToolContext, initiative::InitiativeTool};
use serde_json::json;
use std::path::PathBuf;
use std::sync::Arc;

const WD: &str = "/test/project-m8";

fn ctx_with_storage(storage: Arc<Storage>, session_id: &str) -> ToolContext {
    ToolContext {
        session_id: session_id.to_string(),
        working_dir: PathBuf::from(WD),
        event_bus: Arc::new(EventBus::new(16)),
        storage: Some(storage),
        agent_manager: None,
        active_model: None,
        team_context: None,
        team_manager: None,
        code_index: None,
        bg_service: None,
        spec_manager: None,
        active_spec_id: None,
        config: None,
        cached_team_dir: Arc::new(std::sync::Mutex::new(None)),
        read_timestamps: Arc::new(std::sync::RwLock::new(std::collections::HashMap::new())),
    }
}

// ── Tool identity ───────────────────────────────────────────────────

#[test]
fn test_initiative_identity() {
    let tool = InitiativeTool;
    assert_eq!(tool.name(), "initiative");
    assert!(tool.description().contains("durable"));
    assert_eq!(tool.permission_category(), "storage:write");
}

#[test]
fn test_initiative_schema_actions() {
    let schema = InitiativeTool.parameters_schema();
    let actions: Vec<&str> = schema["properties"]["action"]["enum"]
        .as_array()
        .expect("action enum")
        .iter()
        .filter_map(serde_json::Value::as_str)
        .collect();
    for expected in ["create", "read", "update", "checkpoint", "list", "close"] {
        assert!(actions.contains(&expected), "missing action {expected}");
    }
    assert_eq!(
        schema["required"],
        serde_json::json!(["action"]),
        "action must be required"
    );
}

// ── Create + read ───────────────────────────────────────────────────

#[tokio::test]
async fn test_initiative_create_then_read() {
    let storage = Arc::new(Storage::open_in_memory().expect("storage"));
    let ctx = ctx_with_storage(Arc::clone(&storage), "sess-1");
    let tool = InitiativeTool;

    let out = tool
        .execute(
            json!({
                "action": "create",
                "id": "api-v2",
                "title": "Ship API v2",
                "description": "Migrate all endpoints to v2",
                "milestones": ["design", "implement", "migrate clients", "deprecate v1"],
            }),
            &ctx,
        )
        .await
        .expect("create");

    assert!(out.content.contains("`api-v2`"), "content: {}", out.content);
    assert!(out.content.contains("Ship API v2"));
    let meta = out.metadata.expect("metadata");
    assert_eq!(meta["action"], "create");
    assert_eq!(meta["id"], "api-v2");
    assert_eq!(meta["milestone_count"], 4);

    // Read back via the tool.
    let out = tool
        .execute(json!({"action": "read", "id": "api-v2"}), &ctx)
        .await
        .expect("read");
    assert!(out.content.contains("`api-v2`"));
    assert!(out.content.contains("**Status:** active"));
    assert!(out.content.contains("ms-1"));
    assert!(out.content.contains("design"));
    assert!(out.content.contains("[ ]"), "no milestones should be done");
}

#[tokio::test]
async fn test_initiative_create_auto_id() {
    let storage = Arc::new(Storage::open_in_memory().expect("storage"));
    let ctx = ctx_with_storage(storage, "sess-1");
    let tool = InitiativeTool;

    let out = tool
        .execute(json!({"action": "create", "title": "Auto id goal"}), &ctx)
        .await
        .expect("create");

    let meta = out.metadata.expect("metadata");
    let id = meta["id"].as_str().expect("id");
    assert!(
        id.starts_with("initiative-"),
        "auto id should have initiative- prefix: {id}"
    );
    assert_eq!(id.len(), "initiative-".len() + 8);
}

#[tokio::test]
async fn test_initiative_create_duplicate_rejected() {
    let storage = Arc::new(Storage::open_in_memory().expect("storage"));
    let ctx = ctx_with_storage(storage, "sess-1");
    let tool = InitiativeTool;

    tool.execute(
        json!({"action": "create", "id": "goal-1", "title": "One"}),
        &ctx,
    )
    .await
    .expect("first create");

    let err = tool
        .execute(
            json!({"action": "create", "id": "goal-1", "title": "Two"}),
            &ctx,
        )
        .await
        .expect_err("duplicate create should fail");
    assert!(err.to_string().contains("already exists"), "err: {err}");
}

#[tokio::test]
async fn test_initiative_create_invalid_slug_rejected() {
    let storage = Arc::new(Storage::open_in_memory().expect("storage"));
    let ctx = ctx_with_storage(storage, "sess-1");
    let tool = InitiativeTool;

    let err = tool
        .execute(
            json!({"action": "create", "id": "bad id with spaces", "title": "x"}),
            &ctx,
        )
        .await
        .expect_err("spaces in id should fail");
    assert!(err.to_string().contains("alphanumerics"), "err: {err}");
}

#[tokio::test]
async fn test_initiative_create_requires_title() {
    let storage = Arc::new(Storage::open_in_memory().expect("storage"));
    let ctx = ctx_with_storage(storage, "sess-1");
    let err = InitiativeTool
        .execute(json!({"action": "create"}), &ctx)
        .await
        .expect_err("create without title should fail");
    assert!(err.to_string().contains("title"), "err: {err}");
}

// ── Checkpoint (T-070 acceptance) ───────────────────────────────────

#[tokio::test]
async fn test_initiative_checkpoint_updates_progress() {
    let storage = Arc::new(Storage::open_in_memory().expect("storage"));
    let ctx = ctx_with_storage(Arc::clone(&storage), "sess-1");
    let tool = InitiativeTool;

    tool.execute(
        json!({
            "action": "create",
            "id": "api-v2",
            "title": "Ship API v2",
            "milestones": ["design", "implement", "ship"],
        }),
        &ctx,
    )
    .await
    .expect("create");

    // Acceptance criterion from JCODEPLAN M8:
    // initiative action="checkpoint" id="api-v2" updates progress.
    let out = tool
        .execute(
            json!({
                "action": "checkpoint",
                "id": "api-v2",
                "milestone": "ms-1",
                "progress": 33,
                "note": "Design doc merged",
            }),
            &ctx,
        )
        .await
        .expect("checkpoint");

    assert!(
        out.content
            .contains("Checkpoint recorded on `api-v2` (33%)")
    );
    assert!(out.content.contains("Milestone 'ms-1' marked complete"));
    assert!(out.content.contains("[x] `ms-1`"), "ms-1 should be checked");

    // Verify persisted state.
    let row = storage
        .get_initiative("api-v2", WD)
        .expect("get")
        .expect("exists");
    assert_eq!(row.progress, 33);
    let ms = row.milestones();
    assert!(ms[0].done, "ms-1 should be done");
    assert!(!ms[1].done, "ms-2 should still be pending");
    assert!(ms[0].completed_at.is_some(), "completed_at should be set");
    // Note should be appended to description.
    assert!(
        row.description.contains("Design doc merged"),
        "note appended: {}",
        row.description
    );
}

#[tokio::test]
async fn test_initiative_checkpoint_unknown_milestone() {
    let storage = Arc::new(Storage::open_in_memory().expect("storage"));
    let ctx = ctx_with_storage(storage, "sess-1");
    let tool = InitiativeTool;

    tool.execute(
        json!({"action": "create", "id": "g1", "title": "Goal", "milestones": ["a", "b"]}),
        &ctx,
    )
    .await
    .expect("create");

    let err = tool
        .execute(
            json!({"action": "checkpoint", "id": "g1", "milestone": "ms-99"}),
            &ctx,
        )
        .await
        .expect_err("unknown milestone should fail");
    assert!(err.to_string().contains("ms-99"), "err: {err}");
    assert!(err.to_string().contains("ms-1"), "err lists valid: {err}");
}

#[tokio::test]
async fn test_initiative_checkpoint_double_complete_is_idempotent() {
    let storage = Arc::new(Storage::open_in_memory().expect("storage"));
    let ctx = ctx_with_storage(storage, "sess-1");
    let tool = InitiativeTool;

    tool.execute(
        json!({"action": "create", "id": "g1", "title": "Goal", "milestones": ["a"]}),
        &ctx,
    )
    .await
    .expect("create");
    tool.execute(
        json!({"action": "checkpoint", "id": "g1", "milestone": "ms-1"}),
        &ctx,
    )
    .await
    .expect("first checkpoint");

    let out = tool
        .execute(
            json!({"action": "checkpoint", "id": "g1", "milestone": "ms-1"}),
            &ctx,
        )
        .await
        .expect("second checkpoint should not error");
    assert!(
        out.content.contains("already complete"),
        "content: {}",
        out.content
    );
}

#[tokio::test]
async fn test_initiative_checkpoint_on_closed_rejected() {
    let storage = Arc::new(Storage::open_in_memory().expect("storage"));
    let ctx = ctx_with_storage(storage, "sess-1");
    let tool = InitiativeTool;

    tool.execute(
        json!({"action": "create", "id": "g1", "title": "Goal"}),
        &ctx,
    )
    .await
    .expect("create");
    tool.execute(
        json!({"action": "close", "id": "g1", "status": "completed"}),
        &ctx,
    )
    .await
    .expect("close");

    let err = tool
        .execute(
            json!({"action": "checkpoint", "id": "g1", "progress": 50}),
            &ctx,
        )
        .await
        .expect_err("checkpoint on completed should fail");
    assert!(err.to_string().contains("completed"), "err: {err}");
}

// ── Update / close / delete behaviour via storage ───────────────────

#[tokio::test]
async fn test_initiative_update_title_and_status() {
    let storage = Arc::new(Storage::open_in_memory().expect("storage"));
    let ctx = ctx_with_storage(Arc::clone(&storage), "sess-1");
    let tool = InitiativeTool;

    tool.execute(
        json!({"action": "create", "id": "g1", "title": "Old title"}),
        &ctx,
    )
    .await
    .expect("create");

    let out = tool
        .execute(
            json!({"action": "update", "id": "g1", "title": "New title", "status": "paused"}),
            &ctx,
        )
        .await
        .expect("update");
    assert!(out.content.contains("New title"));
    assert!(out.content.contains("**Status:** paused"));

    let row = storage
        .get_initiative("g1", WD)
        .expect("get")
        .expect("exists");
    assert_eq!(row.title, "New title");
    assert_eq!(row.status, "paused");
    assert!(row.closed_at.is_none(), "paused should not set closed_at");
}

#[tokio::test]
async fn test_initiative_update_requires_a_field() {
    let storage = Arc::new(Storage::open_in_memory().expect("storage"));
    let ctx = ctx_with_storage(storage, "sess-1");
    let tool = InitiativeTool;

    tool.execute(json!({"action": "create", "id": "g1", "title": "T"}), &ctx)
        .await
        .expect("create");

    let err = tool
        .execute(json!({"action": "update", "id": "g1"}), &ctx)
        .await
        .expect_err("update with no fields should fail");
    assert!(err.to_string().contains("at least one of"), "err: {err}");
}

#[tokio::test]
async fn test_initiative_close_completed_sets_100_and_closed_at() {
    let storage = Arc::new(Storage::open_in_memory().expect("storage"));
    let ctx = ctx_with_storage(Arc::clone(&storage), "sess-1");
    let tool = InitiativeTool;

    tool.execute(
        json!({"action": "create", "id": "g1", "title": "T", "progress": 0}),
        &ctx,
    )
    .await
    .expect("create");

    let out = tool
        .execute(
            json!({"action": "close", "id": "g1", "status": "completed"}),
            &ctx,
        )
        .await
        .expect("close");
    assert!(out.content.contains("status **completed**"));

    let row = storage
        .get_initiative("g1", WD)
        .expect("get")
        .expect("exists");
    assert_eq!(row.status, "completed");
    assert_eq!(row.progress, 100, "close completed implies 100%");
    assert!(row.closed_at.is_some());
}

#[tokio::test]
async fn test_initiative_close_abandoned_keeps_progress() {
    let storage = Arc::new(Storage::open_in_memory().expect("storage"));
    let ctx = ctx_with_storage(Arc::clone(&storage), "sess-1");
    let tool = InitiativeTool;

    tool.execute(json!({"action": "create", "id": "g1", "title": "T"}), &ctx)
        .await
        .expect("create");
    tool.execute(
        json!({"action": "checkpoint", "id": "g1", "progress": 40}),
        &ctx,
    )
    .await
    .expect("checkpoint");

    tool.execute(
        json!({"action": "close", "id": "g1", "status": "abandoned"}),
        &ctx,
    )
    .await
    .expect("close");

    let row = storage
        .get_initiative("g1", WD)
        .expect("get")
        .expect("exists");
    assert_eq!(row.status, "abandoned");
    assert_eq!(row.progress, 40, "abandoned should not force 100%");
    assert!(row.closed_at.is_some());
}

#[tokio::test]
async fn test_initiative_close_invalid_status_rejected() {
    let storage = Arc::new(Storage::open_in_memory().expect("storage"));
    let ctx = ctx_with_storage(storage, "sess-1");
    let tool = InitiativeTool;

    tool.execute(json!({"action": "create", "id": "g1", "title": "T"}), &ctx)
        .await
        .expect("create");
    let err = tool
        .execute(
            json!({"action": "close", "id": "g1", "status": "active"}),
            &ctx,
        )
        .await
        .expect_err("close with active status should fail");
    assert!(err.to_string().contains("completed"), "err: {err}");
}

// ── List ────────────────────────────────────────────────────────────

#[tokio::test]
async fn test_initiative_list_active_by_default() {
    let storage = Arc::new(Storage::open_in_memory().expect("storage"));
    let ctx = ctx_with_storage(storage, "sess-1");
    let tool = InitiativeTool;

    tool.execute(
        json!({"action": "create", "id": "g-active", "title": "Active"}),
        &ctx,
    )
    .await
    .expect("create active");
    tool.execute(
        json!({"action": "create", "id": "g-done", "title": "Done"}),
        &ctx,
    )
    .await
    .expect("create done");
    tool.execute(
        json!({"action": "close", "id": "g-done", "status": "completed"}),
        &ctx,
    )
    .await
    .expect("close done");

    let out = tool
        .execute(json!({"action": "list"}), &ctx)
        .await
        .expect("list default");
    assert!(out.content.contains("g-active"));
    assert!(
        !out.content.contains("g-done"),
        "completed should be hidden by default: {}",
        out.content
    );
    assert_eq!(out.metadata.expect("meta")["count"], 1);

    let out = tool
        .execute(json!({"action": "list", "status": "all"}), &ctx)
        .await
        .expect("list all");
    assert!(out.content.contains("g-active"), "all: {}", out.content);
    assert!(out.content.contains("g-done"), "all: {}", out.content);
    // `truncate_chars` shortens ids to 22 chars; both are short enough here.
    assert_eq!(out.metadata.expect("meta")["count"], 2);
}

#[tokio::test]
async fn test_initiative_list_empty_hint() {
    let storage = Arc::new(Storage::open_in_memory().expect("storage"));
    let ctx = ctx_with_storage(storage, "sess-1");
    let out = InitiativeTool
        .execute(json!({"action": "list"}), &ctx)
        .await
        .expect("list");
    assert!(out.content.contains("No active initiatives"));
    assert!(
        out.content.contains("action=\"create\""),
        "should hint how to create: {}",
        out.content
    );
}

#[tokio::test]
async fn test_initiative_list_invalid_filter_rejected() {
    let storage = Arc::new(Storage::open_in_memory().expect("storage"));
    let ctx = ctx_with_storage(storage, "sess-1");
    let err = InitiativeTool
        .execute(json!({"action": "list", "status": "bogus"}), &ctx)
        .await
        .expect_err("bogus filter");
    assert!(
        err.to_string().contains("Invalid status filter"),
        "err: {err}"
    );
}

// ── Cross-session durability ────────────────────────────────────────

#[tokio::test]
async fn test_initiative_visible_from_another_session() {
    let storage = Arc::new(Storage::open_in_memory().expect("storage"));
    let tool = InitiativeTool;

    let ctx_a = ctx_with_storage(Arc::clone(&storage), "sess-A");
    tool.execute(
        json!({"action": "create", "id": "shared-goal", "title": "Shared"}),
        &ctx_a,
    )
    .await
    .expect("create in session A");

    // A different session in the same working directory sees the initiative.
    let ctx_b = ctx_with_storage(Arc::clone(&storage), "sess-B");
    let out = tool
        .execute(json!({"action": "read", "id": "shared-goal"}), &ctx_b)
        .await
        .expect("read from session B");
    assert!(out.content.contains("`shared-goal`"));
}

#[tokio::test]
async fn test_initiative_isolated_per_project() {
    let storage = Arc::new(Storage::open_in_memory().expect("storage"));
    let tool = InitiativeTool;

    let ctx_a = ctx_with_storage(Arc::clone(&storage), "sess-A");
    tool.execute(
        json!({"action": "create", "id": "proj-goal", "title": "Mine"}),
        &ctx_a,
    )
    .await
    .expect("create");

    // Same storage, different working dir → not visible.
    let mut ctx_other = ctx_with_storage(Arc::clone(&storage), "sess-B");
    ctx_other.working_dir = PathBuf::from("/test/other-project");
    let err = tool
        .execute(json!({"action": "read", "id": "proj-goal"}), &ctx_other)
        .await
        .expect_err("other project should not see initiative");
    assert!(err.to_string().contains("not found"), "err: {err}");
}

// ── Errors ──────────────────────────���───────────────────────────────

#[tokio::test]
async fn test_initiative_missing_storage_graceful() {
    let ctx = ToolContext {
        session_id: "s".to_string(),
        working_dir: PathBuf::from(WD),
        event_bus: Arc::new(EventBus::new(16)),
        storage: None,
        agent_manager: None,
        active_model: None,
        team_context: None,
        team_manager: None,
        code_index: None,
        bg_service: None,
        spec_manager: None,
        active_spec_id: None,
        config: None,
        cached_team_dir: Arc::new(std::sync::Mutex::new(None)),
        read_timestamps: Arc::new(std::sync::RwLock::new(std::collections::HashMap::new())),
    };
    let err = InitiativeTool
        .execute(json!({"action": "list"}), &ctx)
        .await
        .expect_err("no storage should fail");
    assert!(err.to_string().contains("storage"), "err: {err}");
}

#[tokio::test]
async fn test_initiative_unknown_action_rejected() {
    let storage = Arc::new(Storage::open_in_memory().expect("storage"));
    let ctx = ctx_with_storage(storage, "sess-1");
    let err = InitiativeTool
        .execute(json!({"action": "frobnicate"}), &ctx)
        .await
        .expect_err("unknown action");
    assert!(
        err.to_string().contains("Unknown initiative action"),
        "err: {err}"
    );
}

// ── Direct storage coverage ─────────────────────────────────────────

#[test]
fn test_storage_initiative_round_trip() {
    let storage = Storage::open_in_memory().expect("storage");
    let ms = vec![
        InitiativeMilestone {
            id: "ms-1".to_string(),
            title: "Phase 1".to_string(),
            done: false,
            completed_at: None,
        },
        InitiativeMilestone {
            id: "ms-2".to_string(),
            title: "Phase 2".to_string(),
            done: false,
            completed_at: None,
        },
    ];
    storage
        .create_initiative("g1", "Title", "Desc", &ms, WD, "sess-1")
        .expect("create");
    let row = storage
        .get_initiative("g1", WD)
        .expect("get")
        .expect("exists");
    assert_eq!(row.status, "active");
    assert_eq!(row.progress, 0);
    assert_eq!(row.milestones().len(), 2);

    // Update milestones + progress.
    let mut updated = row.milestones();
    updated[0].done = true;
    updated[0].completed_at = Some("2025-01-01T00:00:00Z".to_string());
    let changed = storage
        .update_initiative("g1", WD, None, None, Some(&updated), Some(50), None, None)
        .expect("update");
    assert!(changed);

    let row = storage
        .get_initiative("g1", WD)
        .expect("get")
        .expect("exists");
    assert_eq!(row.progress, 50);
    assert!(row.milestones()[0].done);

    // Delete.
    assert!(storage.delete_initiative("g1", WD).expect("delete"));
    assert!(storage.get_initiative("g1", WD).expect("get").is_none());
    assert!(!storage.delete_initiative("g1", WD).expect("delete again"));
}

// ── System-prompt section ───────────────────────────────────────────

#[test]
fn test_build_initiatives_prompt_section_empty() {
    let storage = Storage::open_in_memory().expect("storage");
    let section = ragent_agent::tool::initiative::build_initiatives_prompt_section(
        &storage,
        std::path::Path::new(WD),
    );
    assert!(section.is_empty(), "no initiatives → empty section");
}

#[test]
fn test_build_initiatives_prompt_section_lists_active() {
    let storage = Storage::open_in_memory().expect("storage");
    storage
        .create_initiative(
            "api-v2",
            "Ship API v2",
            "desc",
            &[
                InitiativeMilestone {
                    id: "ms-1".to_string(),
                    title: "design".to_string(),
                    done: true,
                    completed_at: Some("2025-01-01".to_string()),
                },
                InitiativeMilestone {
                    id: "ms-2".to_string(),
                    title: "implement".to_string(),
                    done: false,
                    completed_at: None,
                },
            ],
            WD,
            "sess-1",
        )
        .expect("create");
    storage
        .update_initiative("api-v2", WD, None, None, None, Some(60), None, None)
        .expect("update");
    // A completed initiative should NOT appear.
    storage
        .create_initiative("old-goal", "Old", "", &[], WD, "sess-1")
        .expect("create old");
    storage
        .update_initiative(
            "old-goal",
            WD,
            None,
            None,
            None,
            None,
            Some("completed"),
            None,
        )
        .expect("close old");

    let section = ragent_agent::tool::initiative::build_initiatives_prompt_section(
        &storage,
        std::path::Path::new(WD),
    );
    assert!(
        section.contains("## Active Initiatives"),
        "section: {section}"
    );
    assert!(section.contains("`api-v2`"));
    assert!(section.contains("Ship API v2"));
    assert!(section.contains("60%"));
    assert!(section.contains("implement"), "remaining milestone preview");
    assert!(
        !section.contains("old-goal"),
        "completed initiative should be hidden: {section}"
    );
}
