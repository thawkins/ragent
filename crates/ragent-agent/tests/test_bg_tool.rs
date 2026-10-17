//! Tests-only coverage for the `bg` tool surface (`ragent_agent::tool::bg`).
//!
//! The service is backed by in-memory storage; a real spawned shell command
//! produces output to list/status/tail/output. The tool's own argument
//! validation and unknown-action branches are exercised directly.

use std::sync::Arc;

use ragent_agent::background::BackgroundTaskService;
use ragent_agent::event::EventBus;
use ragent_agent::storage::Storage;
use ragent_agent::tool::bg::BgTool;
use ragent_agent::tool::{Tool, ToolContext};
use serde_json::json;

/// Build a `ToolContext` with a wired background service over in-memory storage.
async fn ctx_with_bg(session: &str) -> (ToolContext, tempfile::TempDir) {
    let storage = Storage::open_in_memory().expect("storage");
    let _ = storage.create_session(session, ".");
    let storage = Arc::new(storage);
    let bus = Arc::new(EventBus::new(64));
    let service = Arc::new(BackgroundTaskService::new(storage, bus.clone()));

    let tmp = tempfile::tempdir().expect("tempdir");
    let ctx = ToolContext {
        session_id: session.to_string(),
        working_dir: tmp.path().to_path_buf(),
        event_bus: bus,
        storage: None,
        agent_manager: None,
        active_model: None,
        provider_registry: None,
        team_context: None,
        team_manager: None,
        code_index: None,
        bg_service: Some(service),
        spec_manager: None,
        active_spec_id: None,
        config: None,
        cached_team_dir: Arc::new(std::sync::Mutex::new(None)),
        permission_checker: None,
        tool_registry: ToolContext::default_tool_registry(),
        read_timestamps: Arc::new(std::sync::RwLock::new(std::collections::HashMap::new())),
        canonical_cache: Arc::new(ragent_tools_core::CanonicalPathCache::new()),
        allowed_roots: Vec::new(),
    };
    (ctx, tmp)
}

/// A `ToolContext` with **no** background service wired in.
fn ctx_without_bg() -> ToolContext {
    let tmp = tempfile::tempdir().expect("tempdir");
    ToolContext {
        session_id: "no-bg".to_string(),
        working_dir: tmp.path().to_path_buf(),
        event_bus: Arc::new(EventBus::new(8)),
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
        cached_team_dir: Arc::new(std::sync::Mutex::new(None)),
        permission_checker: None,
        tool_registry: ToolContext::default_tool_registry(),
        read_timestamps: Arc::new(std::sync::RwLock::new(std::collections::HashMap::new())),
        canonical_cache: Arc::new(ragent_tools_core::CanonicalPathCache::new()),
        allowed_roots: Vec::new(),
    }
}

#[tokio::test]
async fn bg_tool_metadata_and_schema() {
    let tool = BgTool::new();
    assert_eq!(tool.name(), "bg");
    assert_eq!(tool.permission_category(), "bash:execute");
    let schema = tool.parameters_schema();
    let actions = schema["properties"]["action"]["enum"]
        .as_array()
        .expect("action enum");
    assert!(actions.iter().any(|a| a == "spawn"));
    assert!(actions.iter().any(|a| a == "cleanup"));
}

#[tokio::test]
async fn bg_tool_without_a_service_errors() {
    let ctx = ctx_without_bg();
    let err = BgTool
        .execute(json!({ "action": "list" }), &ctx)
        .await
        .expect_err("no service must error");
    assert!(
        err.to_string()
            .contains("Background task service is not available"),
        "err: {err}"
    );
}

#[tokio::test]
async fn bg_tool_unknown_action_errors() {
    let (ctx, _tmp) = ctx_with_bg("bg-unknown").await;
    let err = BgTool
        .execute(json!({ "action": "frobnicate" }), &ctx)
        .await
        .expect_err("unknown action must error");
    assert!(err.to_string().contains("Unknown bg action"), "err: {err}");
}

#[tokio::test]
async fn bg_tool_missing_action_errors() {
    let (ctx, _tmp) = ctx_with_bg("bg-missing-action").await;
    let err = BgTool
        .execute(json!({}), &ctx)
        .await
        .expect_err("missing action must error");
    assert!(
        err.to_string().contains("Missing required 'action'"),
        "err: {err}"
    );
}

#[tokio::test]
async fn bg_tool_spawn_list_status_tail_output_lifecycle() {
    let (ctx, _tmp) = ctx_with_bg("bg-lifecycle").await;

    // Spawn a short command.
    let spawned = BgTool
        .execute(
            json!({ "action": "spawn", "command": "echo bg-tool-hello" }),
            &ctx,
        )
        .await
        .expect("spawn");
    assert!(spawned.content.contains("Spawned background task"));
    let task_id = spawned.metadata.as_ref().expect("metadata")["task_id"]
        .as_str()
        .expect("task_id")
        .to_string();

    // Wait for it to finish.
    let waited = BgTool
        .execute(
            json!({ "action": "wait", "task_id": task_id, "timeout": 15 }),
            &ctx,
        )
        .await
        .expect("wait");
    assert!(waited.content.contains("status="));

    // List returns at least this task.
    let listed = BgTool
        .execute(json!({ "action": "list" }), &ctx)
        .await
        .expect("list");
    let tasks = listed.metadata.as_ref().expect("metadata")["tasks"]
        .as_array()
        .expect("tasks array");
    assert!(!tasks.is_empty(), "the spawned task must be listed");

    // Status reports the id.
    let status = BgTool
        .execute(json!({ "action": "status", "task_id": task_id }), &ctx)
        .await
        .expect("status");
    assert!(status.content.contains(&task_id));

    // Output contains the echoed text.
    let output = BgTool
        .execute(json!({ "action": "output", "task_id": task_id }), &ctx)
        .await
        .expect("output");
    assert!(
        output.content.contains("bg-tool-hello"),
        "output: {}",
        output.content
    );

    // Tail defaults to 20 lines.
    let tail = BgTool
        .execute(json!({ "action": "tail", "task_id": task_id }), &ctx)
        .await
        .expect("tail");
    assert!(tail.content.contains("bg-tool-hello") || tail.content.contains("STDOUT"));
}

#[tokio::test]
async fn bg_tool_spawn_missing_command_errors() {
    let (ctx, _tmp) = ctx_with_bg("bg-no-command").await;
    let err = BgTool
        .execute(json!({ "action": "spawn" }), &ctx)
        .await
        .expect_err("spawn without a command must error");
    assert!(
        err.to_string().contains("Missing required 'command'"),
        "err: {err}"
    );
}

#[tokio::test]
async fn bg_tool_status_missing_task_id_errors() {
    let (ctx, _tmp) = ctx_with_bg("bg-no-id").await;
    let err = BgTool
        .execute(json!({ "action": "status" }), &ctx)
        .await
        .expect_err("status without task_id must error");
    assert!(
        err.to_string().contains("Missing required 'task_id'"),
        "err: {err}"
    );
}

#[tokio::test]
async fn bg_tool_cleanup_runs() {
    let (ctx, _tmp) = ctx_with_bg("bg-cleanup").await;
    // Spawn then cancel so a terminal task exists, then clean it up.
    let spawned = BgTool
        .execute(
            json!({ "action": "spawn", "command": "echo cleanup-me" }),
            &ctx,
        )
        .await
        .expect("spawn");
    let task_id = spawned.metadata.as_ref().expect("metadata")["task_id"]
        .as_str()
        .expect("task_id")
        .to_string();
    let _ = BgTool
        .execute(
            json!({ "action": "wait", "task_id": task_id, "timeout": 15 }),
            &ctx,
        )
        .await
        .expect("wait");

    let cleaned = BgTool
        .execute(
            json!({ "action": "cleanup", "older_than_minutes": 0 }),
            &ctx,
        )
        .await
        .expect("cleanup");
    assert!(
        cleaned.content.contains("Cleaned up"),
        "cleanup output: {}",
        cleaned.content
    );
}
