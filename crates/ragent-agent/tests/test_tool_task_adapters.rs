//! Tests-only coverage for the extracted-tool adapter surface in
//! `crates/ragent-agent/src/tool/mod.rs` (the `LegacyMultiEditAlias`
//! legacy-parameter normalisation and the `Extracted*ToolAdapter` `execute`
//! paths) and the `AgentManager` task surfaces in
//! `crates/ragent-agent/src/task/mod.rs` that the existing registry tests do
//! not touch (`kill_task`/`suspend_task`/`resume_task`, `spawn_sync`, the
//! no-model background failure branch, and the `ReportStatus`/`TaskStatus`
//! `Display` impls).
//!
//! No production code is modified and all file I/O stays inside temp dirs.

use std::sync::Arc;

use ragent_agent::event::EventBus;
use ragent_agent::task::{ReportStatus, TaskStatus};
use ragent_agent::tool::{ToolContext, create_default_registry};

#[path = "task_support/mod.rs"]
mod task_support;
use task_support::{entry_full, manager};

// -- ToolContext helper -----------------------------------------------------

fn ctx_in(dir: &std::path::Path) -> ToolContext {
    ToolContext {
        session_id: "sess-1".to_string(),
        working_dir: dir.to_path_buf(),
        event_bus: Arc::new(EventBus::new(32)),
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

// -- LegacyMultiEditAlias ---------------------------------------------------

#[test]
fn multiedit_alias_identity_and_schema_are_forwarded() {
    let registry = create_default_registry();
    let alias = registry
        .get("multiedit")
        .expect("multiedit alias registered");
    assert_eq!(alias.name(), "multiedit");
    assert_eq!(alias.permission_category(), "file:write");
    // The schema is delegated verbatim to the wrapped core `multi_edit` tool.
    let schema = alias.parameters_schema();
    assert!(
        schema["properties"]["edits"].is_object(),
        "schema: {schema}"
    );
    assert!(
        alias.description().to_lowercase().contains("deprecated"),
        "description: {}",
        alias.description()
    );
    // The canonical tool is registered alongside the legacy alias.
    assert!(registry.get("multi_edit").is_some());
}

#[tokio::test]
async fn multiedit_alias_normalises_legacy_params_and_warns() {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = dir.path().join("a.txt");
    std::fs::write(&path, "hello world\n").expect("write");

    let registry = create_default_registry();
    let alias = registry.get("multiedit").expect("multiedit alias");
    let ctx = ctx_in(dir.path());

    // Legacy `path`/`old_str`/`new_str` spellings (no canonical keys).
    let out = alias
        .execute(
            serde_json::json!({
                "edits": [
                    { "path": "a.txt", "old_str": "hello", "new_str": "goodbye" }
                ]
            }),
            &ctx,
        )
        .await
        .expect("legacy multiedit executes");

    assert_eq!(
        std::fs::read_to_string(&path).expect("read"),
        "goodbye world\n"
    );
    let meta = out.metadata.expect("metadata present");
    assert!(
        meta["deprecation_warning"]
            .as_str()
            .is_some_and(|w| w.contains("deprecated")),
        "deprecation warning emitted: {meta}"
    );
}

#[tokio::test]
async fn multiedit_alias_canonical_key_wins_over_legacy() {
    let dir = tempfile::tempdir().expect("tempdir");
    let canonical = dir.path().join("canonical.txt");
    let legacy = dir.path().join("legacy.txt");
    std::fs::write(&canonical, "one\n").expect("write canonical");
    std::fs::write(&legacy, "one\n").expect("write legacy");

    let registry = create_default_registry();
    let alias = registry.get("multiedit").expect("multiedit alias");
    let ctx = ctx_in(dir.path());

    // Both spellings present: the canonical `file_path` must be the one edited,
    // so `normalise_legacy_params` leaves the canonical slot untouched.
    alias
        .execute(
            serde_json::json!({
                "edits": [{
                    "file_path": "canonical.txt",
                    "path": "legacy.txt",
                    "old_string": "one",
                    "new_string": "two"
                }]
            }),
            &ctx,
        )
        .await
        .expect("canonical-wins multiedit executes");

    assert_eq!(
        std::fs::read_to_string(&canonical).expect("read canonical"),
        "two\n"
    );
    assert_eq!(
        std::fs::read_to_string(&legacy).expect("read legacy"),
        "one\n",
        "the legacy path must not be edited"
    );
}

#[tokio::test]
async fn multiedit_alias_passthrough_when_edits_key_is_absent() {
    let dir = tempfile::tempdir().expect("tempdir");
    let registry = create_default_registry();
    let alias = registry.get("multiedit").expect("multiedit alias");
    let ctx = ctx_in(dir.path());

    // No `edits` array: `normalise_legacy_params` returns the input unchanged
    // and the core tool reports the missing parameter.
    let err = alias
        .execute(serde_json::json!({ "dry_run": true }), &ctx)
        .await
        .expect_err("missing edits array must error");
    assert!(
        err.to_string().contains("edits"),
        "error should name the edits array: {err}"
    );
}

// -- ExtractedCoreToolAdapter via the default registry ----------------------

#[tokio::test]
async fn core_read_adapter_executes_through_default_registry() {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = dir.path().join("readme.txt");
    std::fs::write(&path, "alpha\nbeta\n").expect("write");

    let registry = create_default_registry();
    let read = registry.get("read").expect("read tool registered");
    assert_eq!(read.name(), "read");
    assert_eq!(read.permission_category(), "file:read");
    let ctx = ctx_in(dir.path());

    let out = read
        .execute(serde_json::json!({ "path": "readme.txt" }), &ctx)
        .await
        .expect("read executes");
    assert!(out.content.contains("alpha"), "content: {}", out.content);
    assert!(out.content.contains("beta"), "content: {}", out.content);
}

#[tokio::test]
async fn core_write_adapter_then_read_roundtrips() {
    let dir = tempfile::tempdir().expect("tempdir");
    let registry = create_default_registry();
    let ctx = ctx_in(dir.path());

    registry
        .get("Write")
        .expect("write tool")
        .execute(
            serde_json::json!({ "path": "out.txt", "content": "round trip\n" }),
            &ctx,
        )
        .await
        .expect("write executes");

    let out = registry
        .get("read")
        .expect("read tool")
        .execute(serde_json::json!({ "path": "out.txt" }), &ctx)
        .await
        .expect("read executes");
    assert!(
        out.content.contains("round trip"),
        "content: {}",
        out.content
    );
}

// -- ExtractedExtendedToolAdapter via the default registry -------------------

#[tokio::test]
async fn extended_mf_version_adapter_executes_without_network() {
    let dir = tempfile::tempdir().expect("tempdir");
    let registry = create_default_registry();
    let tool = registry.get("mf_version").expect("mf_version registered");
    assert_eq!(tool.name(), "mf_version");
    let ctx = ctx_in(dir.path());

    let out = tool
        .execute(serde_json::json!({}), &ctx)
        .await
        .expect("mf_version executes offline");
    assert!(!out.content.is_empty(), "mf_version returns a report");
}

#[tokio::test]
async fn extended_task_list_adapter_uses_the_storage_backend() {
    let dir = tempfile::tempdir().expect("tempdir");
    let storage = Arc::new(ragent_agent::storage::Storage::open_in_memory().expect("storage"));
    storage
        .create_session("sess-1", "target/temp")
        .expect("session");

    let registry = create_default_registry();
    let tool = registry.get("TaskList").expect("task_list registered");
    assert_eq!(tool.name(), "TaskList");
    assert_eq!(tool.permission_category(), "task");

    let mut ctx = ctx_in(dir.path());
    ctx.storage = Some(storage);
    ctx.session_id = "sess-1".to_string();

    // An empty task list exercises the CoreStorageAdapter::list_tasks bridge
    // end to end (tool -> adapter -> storage).
    let out = tool
        .execute(serde_json::json!({}), &ctx)
        .await
        .expect("task_list executes");
    assert!(
        out.content.to_lowercase().contains("no tasks")
            || out.content.to_lowercase().contains("task"),
        "content: {}",
        out.content
    );
}

#[tokio::test]
async fn extended_task_tool_without_storage_errors_cleanly() {
    let dir = tempfile::tempdir().expect("tempdir");
    let registry = create_default_registry();
    let ctx = ctx_in(dir.path());
    // No storage wired into the context.
    let err = registry
        .get("TaskList")
        .expect("TaskList")
        .execute(serde_json::json!({}), &ctx)
        .await
        .expect_err("task_list needs a storage backend");
    assert!(
        err.to_string().contains("Storage"),
        "error names the missing storage: {err}"
    );
}

// -- ExtractedVcsToolAdapter via the default registry ------------------------

#[test]
fn vcs_adapter_is_registered_and_advertises_its_identity() {
    let registry = create_default_registry();
    let tool = registry.get("git_status").expect("git_status registered");
    assert_eq!(tool.name(), "git_status");
    assert_ne!(tool.description(), "");
    assert!(tool.parameters_schema().is_object());
}

#[tokio::test]
async fn vcs_adapter_executes_git_status_in_a_local_repo() {
    let dir = tempfile::tempdir().expect("tempdir");
    // Initialise a throwaway repo so `git_status` has something harmless to read.
    let status = std::process::Command::new("git")
        .arg("init")
        .arg("-q")
        .current_dir(dir.path())
        .status();
    let Ok(status) = status else {
        eprintln!("git unavailable; skipping");
        return;
    };
    if !status.success() {
        eprintln!("git init failed; skipping");
        return;
    }

    let registry = create_default_registry();
    let tool = registry.get("git_status").expect("git_status");
    let ctx = ctx_in(dir.path());
    let out = tool
        .execute(serde_json::json!({}), &ctx)
        .await
        .expect("git_status executes in a repo");
    assert!(!out.content.is_empty(), "git_status returns output");
}

// -- AgentManager: kill / suspend / resume -----------------------------------

#[tokio::test]
async fn kill_task_running_moves_it_to_terminating() {
    let mgr = manager(4);
    mgr.seed_completed_for_test(entry_full("run", TaskStatus::Running, "p", false, false))
        .await;

    mgr.kill_task("run").await.expect("kill running task");
    let entry = mgr.get_task("run").await.expect("entry present");
    assert_eq!(entry.status, TaskStatus::Terminating);
}

#[tokio::test]
async fn kill_task_unknown_and_finished_are_rejected() {
    let mgr = manager(4);
    let err = mgr.kill_task("ghost").await.expect_err("unknown task");
    assert!(err.to_string().contains("not found"), "err: {err}");

    mgr.seed_completed_for_test(entry_full("done", TaskStatus::Completed, "p", false, false))
        .await;
    let err = mgr.kill_task("done").await.expect_err("finished task");
    assert!(err.to_string().contains("already finished"), "err: {err}");
}

#[tokio::test]
async fn suspend_and_resume_unknown_task_rejected() {
    let mgr = manager(4);
    let suspend = mgr
        .suspend_task("any")
        .await
        .expect_err("unknown task suspend");
    assert!(
        suspend.to_string().contains("not found"),
        "suspend err: {suspend}"
    );
    let resume = mgr
        .resume_task("any")
        .await
        .expect_err("unknown task resume");
    assert!(
        resume.to_string().contains("not found"),
        "resume err: {resume}"
    );
}

// -- AgentManager: spawn without a usable provider ---------------------------

#[tokio::test]
async fn spawn_sync_without_a_provider_fails_fast() {
    let mgr = manager(4);
    let dir = tempfile::tempdir().expect("tempdir");
    // The in-memory processor has an empty provider registry, so the sub-agent
    // turn cannot build a client and the synchronous spawn returns an error.
    let err = mgr
        .spawn_sync("p", "explore", "do it", None, dir.path())
        .await
        .expect_err("no provider is configured");
    assert!(!err.to_string().is_empty(), "non-empty error");
}

#[tokio::test]
async fn spawn_background_without_a_provider_records_a_failure() {
    let mgr = manager(4);
    let dir = tempfile::tempdir().expect("tempdir");
    let entry = mgr
        .spawn_background("p", "explore", "do it", None, dir.path())
        .await
        .expect("spawn accepted");
    assert!(entry.background);
    assert!(!entry.detached);

    // Poll until the background task errors out (no provider/model configured).
    let mut status = entry.status.clone();
    for _ in 0..250 {
        tokio::time::sleep(std::time::Duration::from_millis(20)).await;
        if let Some(current) = mgr.get_task(&entry.id).await {
            status = current.status.clone();
            if status != TaskStatus::Running {
                break;
            }
        } else {
            break;
        }
    }
    assert_eq!(
        status,
        TaskStatus::Failed,
        "an unusable provider must mark the task failed"
    );
}

// -- TaskStatus / ReportStatus Display ---------------------------------------

#[test]
fn task_status_display_matches_serde_rename() {
    for (status, label) in [
        (TaskStatus::Running, "running"),
        (TaskStatus::Completed, "completed"),
        (TaskStatus::Failed, "failed"),
        (TaskStatus::Suspended, "suspended"),
        (TaskStatus::Terminating, "terminating"),
        (TaskStatus::Cancelled, "cancelled"),
    ] {
        assert_eq!(status.as_str(), label);
        assert_eq!(status.to_string(), label);
        let json = serde_json::to_string(&status).expect("serialize");
        assert_eq!(json, format!("\"{label}\""));
    }
}

#[test]
fn report_status_display_and_classification() {
    assert_eq!(ReportStatus::default(), ReportStatus::Complete);
    assert_eq!(ReportStatus::Complete.as_str(), "complete");
    assert_eq!(ReportStatus::Complete.to_string(), "complete");
    assert_eq!(ReportStatus::Continued.to_string(), "continued");
    assert_eq!(ReportStatus::Truncated.to_string(), "truncated");

    for label in ["length", "truncation", "content_filter", "truncated"] {
        assert_eq!(
            ReportStatus::from_finish_reason_label(label),
            ReportStatus::Truncated,
            "label {label}"
        );
    }
    assert_eq!(
        ReportStatus::from_finish_reason_label("continued"),
        ReportStatus::Continued
    );
    for label in ["stop", "", "tool_use"] {
        assert_eq!(
            ReportStatus::from_finish_reason_label(label),
            ReportStatus::Complete,
            "label {label}"
        );
    }
}
