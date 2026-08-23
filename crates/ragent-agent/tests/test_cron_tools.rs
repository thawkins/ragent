//! Tests for the cron scheduler tools (`cron_add`, `cron_remove`, `cron_list`,
//! `cron_enable`, `cron_disable`).
//!
//! These tools mirror the `/cron` slash commands and provide the LLM with
//! direct access to the cron scheduler via `ragent_storage::Storage`.

use ragent_agent::event::EventBus;
use ragent_agent::storage::Storage;
use ragent_agent::tool::{Tool, ToolContext, cron::*};
use serde_json::json;
use std::path::PathBuf;
use std::sync::Arc;

const WD: &str = "/test/project-cron";

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
        canonical_cache: std::sync::Arc::new(ragent_tools_core::CanonicalPathCache::new()),
    }
}

// ── Tool identity ────────────────────────────────────────────────────────

#[test]
fn test_cron_add_identity() {
    let tool = CronAddTool;
    assert_eq!(tool.name(), "cron_add");
    assert!(tool.description().contains("schedule"));
    assert_eq!(tool.permission_category(), "cron:write");
}

#[test]
fn test_cron_remove_identity() {
    let tool = CronRemoveTool;
    assert_eq!(tool.name(), "cron_remove");
    assert_eq!(tool.permission_category(), "cron:write");
}

#[test]
fn test_cron_list_identity() {
    let tool = CronListTool;
    assert_eq!(tool.name(), "cron_list");
    assert_eq!(tool.permission_category(), "cron:read");
}

#[test]
fn test_cron_enable_identity() {
    let tool = CronEnableTool;
    assert_eq!(tool.name(), "cron_enable");
    assert_eq!(tool.permission_category(), "cron:write");
}

#[test]
fn test_cron_disable_identity() {
    let tool = CronDisableTool;
    assert_eq!(tool.name(), "cron_disable");
    assert_eq!(tool.permission_category(), "cron:write");
}

#[test]
fn test_cron_add_schema_requires_all_fields() {
    let schema = CronAddTool.parameters_schema();
    let required = schema["required"].as_array().expect("required array");
    for field in ["id", "agent", "schedule", "prompt"] {
        assert!(
            required.iter().any(|v| v.as_str() == Some(field)),
            "field '{field}' must be required"
        );
    }
}

// ── cron_add ─────────────────────────────────────────────────────────────

#[tokio::test]
async fn test_cron_add_creates_event() {
    let storage = Arc::new(Storage::open_in_memory().expect("storage"));
    let ctx = ctx_with_storage(Arc::clone(&storage), "sess-1");

    let out = CronAddTool
        .execute(
            json!({
                "id": "nightly",
                "agent": "general",
                "schedule": "every 30m",
                "prompt": "Run tests"
            }),
            &ctx,
        )
        .await
        .expect("add should succeed");

    assert!(out.content.contains("✅"));
    assert!(out.content.contains("nightly"));
    assert!(out.content.contains("every 30m"));

    // Verify it was stored.
    let row = storage
        .get_cron_event("nightly")
        .expect("query")
        .expect("event should exist");
    assert_eq!(row.agent_type, "general");
    assert_eq!(row.prompt, "Run tests");
    assert!(row.enabled, "new events should be enabled by default");
}

#[tokio::test]
async fn test_cron_add_duplicate_fails() {
    let storage = Arc::new(Storage::open_in_memory().expect("storage"));
    let ctx = ctx_with_storage(Arc::clone(&storage), "sess-1");

    CronAddTool
        .execute(
            json!({"id": "dup", "agent": "general", "schedule": "every 1h", "prompt": "A"}),
            &ctx,
        )
        .await
        .expect("first add");

    let result = CronAddTool
        .execute(
            json!({"id": "dup", "agent": "coder", "schedule": "every 2h", "prompt": "B"}),
            &ctx,
        )
        .await;
    assert!(result.is_err(), "duplicate id should fail");
}

#[tokio::test]
async fn test_cron_add_invalid_schedule() {
    let storage = Arc::new(Storage::open_in_memory().expect("storage"));
    let ctx = ctx_with_storage(Arc::clone(&storage), "sess-1");

    let result = CronAddTool
        .execute(
            json!({"id": "bad", "agent": "general", "schedule": "nonsense", "prompt": "x"}),
            &ctx,
        )
        .await;
    assert!(result.is_err(), "invalid schedule should fail");
}

#[tokio::test]
async fn test_cron_add_missing_param() {
    let storage = Arc::new(Storage::open_in_memory().expect("storage"));
    let ctx = ctx_with_storage(Arc::clone(&storage), "sess-1");

    let result = CronAddTool
        .execute(
            json!({"id": "x", "agent": "general", "schedule": "every 1h"}),
            &ctx,
        )
        .await;
    assert!(result.is_err(), "missing prompt should fail");
}

// ── cron_list ─────────────────────────────────────────────────────────────

#[tokio::test]
async fn test_cron_list_empty() {
    let storage = Arc::new(Storage::open_in_memory().expect("storage"));
    let ctx = ctx_with_storage(Arc::clone(&storage), "sess-1");

    let out = CronListTool.execute(json!({}), &ctx).await.expect("list");
    assert!(out.content.contains("No scheduled events"));
    assert_eq!(out.metadata.unwrap()["count"].as_u64(), Some(0));
}

#[tokio::test]
async fn test_cron_list_shows_events() {
    let storage = Arc::new(Storage::open_in_memory().expect("storage"));
    let ctx = ctx_with_storage(Arc::clone(&storage), "sess-1");

    CronAddTool
        .execute(
            json!({"id": "a", "agent": "general", "schedule": "every 1h", "prompt": "A"}),
            &ctx,
        )
        .await
        .expect("add a");
    CronAddTool
        .execute(
            json!({"id": "b", "agent": "coder", "schedule": "every 2h", "prompt": "B"}),
            &ctx,
        )
        .await
        .expect("add b");

    let out = CronListTool.execute(json!({}), &ctx).await.expect("list");
    assert!(out.content.contains("`a`"));
    assert!(out.content.contains("`b`"));
    assert!(out.content.contains("general"));
    assert!(out.content.contains("coder"));
    assert_eq!(out.metadata.unwrap()["count"].as_u64(), Some(2));
}

// ── cron_remove ──────────────────���────────────────────────────────────────

#[tokio::test]
async fn test_cron_remove_existing() {
    let storage = Arc::new(Storage::open_in_memory().expect("storage"));
    let ctx = ctx_with_storage(Arc::clone(&storage), "sess-1");

    CronAddTool
        .execute(
            json!({"id": "rm", "agent": "general", "schedule": "every 1h", "prompt": "x"}),
            &ctx,
        )
        .await
        .expect("add");

    let out = CronRemoveTool
        .execute(json!({"id": "rm"}), &ctx)
        .await
        .expect("remove");
    assert!(out.content.contains("removed"));

    // Verify it's gone.
    assert!(storage.get_cron_event("rm").expect("query").is_none());
}

#[tokio::test]
async fn test_cron_remove_not_found() {
    let storage = Arc::new(Storage::open_in_memory().expect("storage"));
    let ctx = ctx_with_storage(Arc::clone(&storage), "sess-1");

    let out = CronRemoveTool
        .execute(json!({"id": "nope"}), &ctx)
        .await
        .expect("remove");
    assert!(out.content.contains("not found"));
}

// ── cron_enable / cron_disable ──────────────────────────────────���────────

#[tokio::test]
async fn test_cron_disable_then_enable() {
    let storage = Arc::new(Storage::open_in_memory().expect("storage"));
    let ctx = ctx_with_storage(Arc::clone(&storage), "sess-1");

    CronAddTool
        .execute(
            json!({"id": "toggle", "agent": "general", "schedule": "every 1h", "prompt": "x"}),
            &ctx,
        )
        .await
        .expect("add");

    // Should start enabled.
    let row = storage.get_cron_event("toggle").expect("q").expect("event");
    assert!(row.enabled);

    // Disable.
    let out = CronDisableTool
        .execute(json!({"id": "toggle"}), &ctx)
        .await
        .expect("disable");
    assert!(out.content.contains("disabled"));
    let row = storage.get_cron_event("toggle").expect("q").expect("event");
    assert!(!row.enabled);

    // Re-enable.
    let out = CronEnableTool
        .execute(json!({"id": "toggle"}), &ctx)
        .await
        .expect("enable");
    assert!(out.content.contains("enabled"));
    let row = storage.get_cron_event("toggle").expect("q").expect("event");
    assert!(row.enabled);
}

#[tokio::test]
async fn test_cron_enable_not_found() {
    let storage = Arc::new(Storage::open_in_memory().expect("storage"));
    let ctx = ctx_with_storage(Arc::clone(&storage), "sess-1");

    let out = CronEnableTool
        .execute(json!({"id": "ghost"}), &ctx)
        .await
        .expect("enable");
    assert!(out.content.contains("not found"));
}

#[tokio::test]
async fn test_cron_disable_not_found() {
    let storage = Arc::new(Storage::open_in_memory().expect("storage"));
    let ctx = ctx_with_storage(Arc::clone(&storage), "sess-1");

    let out = CronDisableTool
        .execute(json!({"id": "ghost"}), &ctx)
        .await
        .expect("disable");
    assert!(out.content.contains("not found"));
}

// ── registry registration ────────────────────────────────────────────────

#[test]
fn test_cron_tools_registered() {
    let registry = ragent_agent::tool::create_default_registry();
    for name in [
        "cron_add",
        "cron_remove",
        "cron_list",
        "cron_enable",
        "cron_disable",
    ] {
        assert!(
            registry.get(name).is_some(),
            "tool '{name}' should be registered"
        );
    }
}
