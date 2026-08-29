//! Tests for the `skill_manage` tool (JCODEPLAN M8 T-071).

use ragent_agent::event::EventBus;
use ragent_agent::tool::{Tool, ToolContext, skill_manage::SkillManageTool};
use serde_json::json;
use std::sync::Arc;

fn ctx_in(dir: &std::path::Path) -> ToolContext {
    ToolContext {
        session_id: "sess-1".to_string(),
        working_dir: dir.to_path_buf(),
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
        cached_team_dir: Arc::new(std::sync::Mutex::new(None)),
        read_timestamps: Arc::new(std::sync::RwLock::new(std::collections::HashMap::new())),
        canonical_cache: std::sync::Arc::new(ragent_tools_core::CanonicalPathCache::new()),
    }
}

/// Write a minimal project-scoped skill into `<dir>/.ragent/skills/<name>/SKILL.md`.
fn write_project_skill(dir: &std::path::Path, name: &str, body: &str) {
    let skill_dir = dir.join(".ragent").join("skills").join(name);
    std::fs::create_dir_all(&skill_dir).expect("mkdir skill");
    std::fs::write(
        skill_dir.join("SKILL.md"),
        format!("---\nname: {name}\ndescription: test skill {name}\n---\n\n{body}\n"),
    )
    .expect("write SKILL.md");
}

// ── Identity ────────────────────────────────────────────────────────

#[test]
fn test_skill_manage_identity() {
    let tool = SkillManageTool;
    assert_eq!(tool.name(), "skill_manage");
    assert!(tool.description().contains("skill"));
    assert_eq!(tool.permission_category(), "skill:manage");
}

#[test]
fn test_skill_manage_schema_actions() {
    let schema = SkillManageTool.parameters_schema();
    let actions: Vec<&str> = schema["properties"]["action"]["enum"]
        .as_array()
        .expect("action enum")
        .iter()
        .filter_map(serde_json::Value::as_str)
        .collect();
    for expected in ["list", "read", "load", "reload"] {
        assert!(actions.contains(&expected), "missing action {expected}");
    }
}

// ── List ────────────────────────────────────────────────────────────

#[tokio::test]
async fn test_skill_manage_list_includes_bundled_and_project() {
    let tmp = tempfile::tempdir().expect("tmp");
    write_project_skill(tmp.path(), "m8-test-skill", "Do the {placeholder} thing");

    let ctx = ctx_in(tmp.path());
    let out = SkillManageTool
        .execute(json!({"action": "list"}), &ctx)
        .await
        .expect("list");

    // Bundled skills are always present (e.g. debug, loop, batch).
    assert!(out.content.contains("skill(s)"), "content: {}", out.content);
    // Our project skill should appear.
    assert!(
        out.content.contains("m8-test-skill"),
        "project skill listed: {}",
        out.content
    );
    // Scope column rendered.
    assert!(
        out.content.contains("Scope"),
        "scope column: {}",
        out.content
    );
    assert_eq!(out.metadata.expect("meta")["action"], "list");
}

#[tokio::test]
async fn test_skill_manage_list_scope_filter() {
    let tmp = tempfile::tempdir().expect("tmp");
    write_project_skill(tmp.path(), "m8-proj-only", "Body");

    let ctx = ctx_in(tmp.path());
    let out = SkillManageTool
        .execute(json!({"action": "list", "scope": "project"}), &ctx)
        .await
        .expect("list project scope");
    assert!(
        out.content.contains("m8-proj-only"),
        "content: {}",
        out.content
    );
    // Bundled skills should be filtered out.
    assert!(
        !out.content.contains("debug"),
        "bundled skills excluded from project-only scope: {}",
        out.content
    );
}

// ── Read (prompt fetch with arg substitution) ───────────────────────

#[tokio::test]
async fn test_skill_manage_read_returns_prompt_body() {
    let tmp = tempfile::tempdir().expect("tmp");
    write_project_skill(
        tmp.path(),
        "m8-echo",
        "You received: $ARGUMENTS. Do it carefully.",
    );

    let ctx = ctx_in(tmp.path());
    let out = SkillManageTool
        .execute(
            json!({"action": "read", "name": "m8-echo", "arguments": "hello world"}),
            &ctx,
        )
        .await
        .expect("read");

    assert!(
        out.content
            .contains("You received: hello world. Do it carefully."),
        "substituted prompt body: {}",
        out.content
    );
    let meta = out.metadata.expect("meta");
    assert_eq!(meta["action"], "read");
    assert_eq!(meta["name"], "m8-echo");
    assert_eq!(meta["scope"], "project");
}

#[tokio::test]
async fn test_skill_manage_read_unknown_skill_lists_available() {
    let tmp = tempfile::tempdir().expect("tmp");
    let ctx = ctx_in(tmp.path());

    let err = SkillManageTool
        .execute(json!({"action": "read", "name": "no-such-skill"}), &ctx)
        .await
        .expect_err("unknown skill should fail");
    let msg = err.to_string();
    assert!(msg.contains("no-such-skill"), "err: {msg}");
    assert!(
        msg.contains("Available skills"),
        "err lists available: {msg}"
    );
}

// ── Load (JCODEPLAN M8 acceptance) ──────────────────────────────────

#[tokio::test]
async fn test_skill_manage_load_injects_skill_prompt() {
    let tmp = tempfile::tempdir().expect("tmp");
    write_project_skill(
        tmp.path(),
        "rust-error-handling",
        "Always use Result<T, E> with thiserror for custom errors.",
    );

    let ctx = ctx_in(tmp.path());
    // Acceptance criterion: skill_manage action="load" name="rust-error-handling"
    // injects the skill.
    let out = SkillManageTool
        .execute(
            json!({"action": "load", "name": "rust-error-handling"}),
            &ctx,
        )
        .await
        .expect("load");

    assert!(
        out.content
            .contains("Skill `rust-error-handling` loaded (project scope)"),
        "load header: {}",
        out.content
    );
    assert!(
        out.content.contains("thiserror"),
        "prompt content injected: {}",
        out.content
    );
    let meta = out.metadata.expect("meta");
    assert_eq!(meta["action"], "load");
    assert_eq!(meta["name"], "rust-error-handling");
}

// ── Reload ──────────────────────────────────────────────────────────

#[tokio::test]
async fn test_skill_manage_reload_reports_added_and_baseline() {
    let tmp = tempfile::tempdir().expect("tmp");
    write_project_skill(tmp.path(), "m8-reload-a", "A body");
    write_project_skill(tmp.path(), "m8-reload-b", "B body");

    let ctx = ctx_in(tmp.path());
    let out = SkillManageTool
        .execute(json!({"action": "reload"}), &ctx)
        .await
        .expect("reload");

    let meta = out.metadata.expect("meta");
    assert_eq!(meta["action"], "reload");
    // Bundled baseline > 0 (debug, loop, batch, etc).
    assert!(
        meta["bundled"].as_u64().expect("bundled") > 0,
        "bundled baseline > 0"
    );
    // Total includes our two project skills + bundled.
    let total = meta["total"].as_u64().expect("total");
    let bundled = meta["bundled"].as_u64().expect("bundled");
    assert!(
        total >= bundled + 2,
        "total {total} >= bundled {bundled} + 2"
    );
    // Both project skills visible in the table.
    assert!(
        out.content.contains("m8-reload-a"),
        "table: {}",
        out.content
    );
    assert!(
        out.content.contains("m8-reload-b"),
        "table: {}",
        out.content
    );
}

#[tokio::test]
async fn test_skill_manage_reload_picks_up_new_skill_added_after_first_scan() {
    let tmp = tempfile::tempdir().expect("tmp");
    write_project_skill(tmp.path(), "m8-original", "Original body");

    let ctx = ctx_in(tmp.path());
    // Prime the registry once (loads "m8-original").
    let _ = SkillManageTool
        .execute(json!({"action": "list"}), &ctx)
        .await
        .expect("first list");

    // Add a NEW skill after the first registry build.
    write_project_skill(tmp.path(), "m8-late-added", "Late body");

    let out = SkillManageTool
        .execute(json!({"action": "read", "name": "m8-late-added"}), &ctx)
        .await
        .expect("read late-added skill — proves discovery is re-run per call");
    assert!(
        out.content.contains("Late body"),
        "content: {}",
        out.content
    );
}

#[tokio::test]
async fn test_skill_manage_reload_reflects_edited_body() {
    let tmp = tempfile::tempdir().expect("tmp");
    write_project_skill(tmp.path(), "m8-editable", "Version one body");

    let ctx = ctx_in(tmp.path());
    let out1 = SkillManageTool
        .execute(json!({"action": "read", "name": "m8-editable"}), &ctx)
        .await
        .expect("first read");
    assert!(out1.content.contains("Version one body"));

    // Edit the SKILL.md on disk.
    write_project_skill(tmp.path(), "m8-editable", "Version two body — updated");

    // reload clears caches and re-discovers.
    let _ = SkillManageTool
        .execute(json!({"action": "reload"}), &ctx)
        .await
        .expect("reload");
    let out2 = SkillManageTool
        .execute(json!({"action": "read", "name": "m8-editable"}), &ctx)
        .await
        .expect("second read");
    assert!(
        out2.content.contains("Version two body — updated"),
        "edited body reflected after reload: {}",
        out2.content
    );
}

// ── Errors ──────────────────────────────────────────────────────────

#[tokio::test]
async fn test_skill_manage_unknown_action_rejected() {
    let tmp = tempfile::tempdir().expect("tmp");
    let ctx = ctx_in(tmp.path());
    let err = SkillManageTool
        .execute(json!({"action": "explode"}), &ctx)
        .await
        .expect_err("unknown action");
    assert!(
        err.to_string().contains("Unknown skill_manage action"),
        "err: {err}"
    );
}

#[tokio::test]
async fn test_skill_manage_read_requires_name() {
    let tmp = tempfile::tempdir().expect("tmp");
    let ctx = ctx_in(tmp.path());
    let err = SkillManageTool
        .execute(json!({"action": "read"}), &ctx)
        .await
        .expect_err("read without name");
    assert!(err.to_string().contains("name"), "err: {err}");
}
