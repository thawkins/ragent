//! Deep dispatch coverage for the remaining `/slash` match arms in
//! `crates/ragent-tui/src/app/slash.rs`.
//!
//! The sibling suites (`test_slash_commands`, `test_slash_info`,
//! `test_slash_config_write`, `test_slash_teams_deep`, `test_slash_heavy_deep`,
//! `test_slash_codeindex_deep`, `test_slash_misc_deep2`) cover the happy path
//! and the usage guards. This file drives the arms they leave unvisited: the
//! `/team` sub-commands against a team that actually exists on disk, the
//! `/config` env-source rows, the `/init config` failure branches, the
//! `/model` Azure-resource and router entry paths, the `--global` and
//! not-found branches of `/dirs` and `/bash`, and the diagnostic /
//! reload / index-status / web-API arms whose bodies only run under specific
//! environment shapes.
//!
//! Tests only - no production logic is touched, and every write goes to the
//! harness tempdir (or the isolated `XDG_CONFIG_HOME` inside it).
#![allow(unsafe_code)]
#![cfg(test)]
// The isolation harness holds a process-global mutex across the async test body
// so cwd/env mutations cannot interleave.
#![allow(clippy::await_holding_lock)]

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use chrono::Utc;
use ragent_agent::task::{ReportStatus, TaskEntry};
use ragent_agent::team::{
    MAX_MESSAGE_BYTES, MemberStatus, Task as TeamTask, TaskStatus as TeamTaskStatus, TeamMember,
    TeamStore,
};
use ragent_tui::App;
use ragent_tui::app::{ConfiguredProvider, ProviderSource};

#[path = "slash_support/mod.rs"]
mod slash_support;
#[path = "support/mod.rs"]
mod support;

use slash_support::{EnvGuard, all_messages, app_with_session, enter_project, last_message};

type Harness = (
    std::sync::MutexGuard<'static, ()>,
    EnvGuard,
    tempfile::TempDir,
    App,
);

/// An app in an isolated tempdir project with an empty scratch config.
fn isolated_app() -> Harness {
    let (guard, env, temp) = enter_project(r#"{ "defaultAgent": "general" }"#);
    let app = app_with_session();
    (guard, env, temp, app)
}

/// An app in an isolated tempdir project whose `.ragent/teams` tree exists.
fn team_app() -> Harness {
    let (guard, env, temp) = enter_project(r#"{ "defaultAgent": "general" }"#);
    std::fs::create_dir_all(temp.path().join(".ragent/teams")).expect("create teams dir");
    let app = app_with_session();
    (guard, env, temp, app)
}

/// Absolute path of the isolated global config directory (`<xdg>/ragent`).
fn global_config_dir() -> std::path::PathBuf {
    let xdg = std::env::var("XDG_CONFIG_HOME").expect("harness sets XDG_CONFIG_HOME");
    std::path::PathBuf::from(xdg).join("ragent")
}

/// Write the isolated global `ragent.json`.
fn write_global_config(json: &str) {
    let dir = global_config_dir();
    std::fs::create_dir_all(&dir).expect("create global config dir");
    std::fs::write(dir.join("ragent.json"), json).expect("write global config");
    ragent_config::Config::invalidate_load_cache();
}

/// Create a project-local team on disk and make it the active team.
async fn active_team_on_disk(app: &mut App, name: &str) {
    app.execute_slash_command(&format!("/team create bp1 {name}"))
        .await;
    assert_eq!(app.status, format!("team: {name}"));
}

fn task_entry(id: &str, agent: &str) -> TaskEntry {
    TaskEntry {
        id: id.to_string(),
        parent_session_id: "s1".to_string(),
        child_session_id: format!("child-{id}"),
        agent_name: agent.to_string(),
        task_prompt: "do the thing".to_string(),
        background: true,
        detached: false,
        status: ragent_agent::task::TaskStatus::Running,
        result: None,
        error: None,
        created_at: Utc::now(),
        completed_at: None,
        reported: false,
        waiter_count: 0,
        output_file: None,
        report_status: ReportStatus::default(),
        time_bound_secs: None,
    }
}

// ---------------------------------------------------------------------------
// /team - create, status, show
// ---------------------------------------------------------------------------

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn team_create_without_blueprint_reports_usage() {
    let (_g, _e, _t, mut app) = team_app();
    app.execute_slash_command("/team create").await;
    assert_eq!(app.status, "Usage: /team create <blueprint> [name]");
    assert!(app.active_team.is_none());
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn team_status_lists_populated_roster_and_empty_roster() {
    let (_g, _e, _t, mut app) = team_app();
    active_team_on_disk(&mut app, "status-team").await;

    // Empty roster branch.
    app.execute_slash_command("/team status").await;
    assert_eq!(app.status, "team: status");
    assert!(last_message(&app).contains("no teammates yet"));

    // Populated roster branch, including a model override.
    let mut member = TeamMember::new("alice", "tm-001", "general");
    member.status = MemberStatus::Working;
    member.current_task_id = Some("task-001".to_string());
    member.model_override = Some(ragent_agent::agent::ModelRef {
        provider_id: "ollama".to_string(),
        model_id: "qwen2.5".to_string(),
    });
    app.team_members.push(member);

    app.execute_slash_command("/team status").await;
    let text = last_message(&app);
    assert!(text.contains("alice"), "text: {text}");
    assert!(text.contains("ollama/qwen2.5"), "text: {text}");
    assert!(text.contains("task-001"), "text: {text}");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn team_show_all_covers_global_scope_and_broken_team() {
    let (_g, _e, temp, mut app) = team_app();

    // A user-global team lands under the isolated XDG config home and is
    // reported with `scope:global`.
    TeamStore::create("global-team", "s1", temp.path(), false).expect("create global team");

    // A directory whose config.json cannot be parsed exercises the
    // "failed to load" row.
    let broken = temp.path().join(".ragent/teams/broken-team");
    std::fs::create_dir_all(&broken).expect("create broken team dir");
    std::fs::write(broken.join("config.json"), "{ not json").expect("write bad config");

    app.execute_slash_command("/team show").await;
    assert_eq!(app.status, "team: show all");
    let text = last_message(&app);
    assert!(text.contains("global-team"), "text: {text}");
    assert!(text.contains("scope:global"), "text: {text}");
    assert!(text.contains("failed to load"), "text: {text}");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn team_show_unknown_name_reports_load_failure() {
    let (_g, _e, _t, mut app) = team_app();
    app.execute_slash_command("/team show ghost-team").await;
    assert!(app.status.starts_with("Failed to load team:"));
    assert!(app.status.contains("ghost-team"));
}

// ---------------------------------------------------------------------------
// /team - delete, blueprint
// ---------------------------------------------------------------------------

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn team_delete_unknown_name_reports_load_failure() {
    // T-043: the explicit `--force` is the confirmation, so the confirmed
    // delete reaches the store and reports that the team does not exist.
    let (_g, _e, _t, mut app) = team_app();
    app.execute_slash_command("/team delete ghost-team --force")
        .await;
    assert!(
        app.status.starts_with("Failed to load team:"),
        "status: {}",
        app.status
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn team_delete_reports_remove_failure() {
    let (_g, _e, temp, mut app) = team_app();
    active_team_on_disk(&mut app, "undeletable").await;
    let teams_dir = temp.path().join(".ragent/teams");

    // Removing a child directory needs write permission on its parent; drop it
    // so `remove_dir_all` fails inside the handler, then restore it so the
    // tempdir can be cleaned up.
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt as _;
        let original = std::fs::metadata(&teams_dir)
            .expect("stat teams dir")
            .permissions();
        std::fs::set_permissions(&teams_dir, std::fs::Permissions::from_mode(0o500))
            .expect("chmod teams dir");
        app.execute_slash_command("/team delete undeletable --force")
            .await;
        std::fs::set_permissions(&teams_dir, original).expect("restore teams dir mode");
        assert!(
            app.status.starts_with("Failed to delete team:"),
            "status: {}",
            app.status
        );
    }
    #[cfg(not(unix))]
    {
        let _ = teams_dir;
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn team_blueprint_lists_and_reports_missing() {
    let (_g, _e, temp, mut app) = team_app();

    // An installed blueprint so the list has a row.
    let bp_dir = temp.path().join(".ragent/blueprints/teams/demo-blueprint");
    std::fs::create_dir_all(&bp_dir).expect("create blueprint dir");
    std::fs::write(bp_dir.join("README.md"), "A demo blueprint.").expect("write blueprint readme");
    std::fs::write(
        bp_dir.join("spawn-prompts.json"),
        r#"[{"teammate_name": "alice", "agent_type": "general", "prompt": "go"}]"#,
    )
    .expect("write spawn prompts");

    app.execute_slash_command("/team blueprint").await;
    assert_eq!(app.status, "team: blueprints");
    let list = last_message(&app);
    assert!(list.contains("demo-blueprint"), "text: {list}");

    app.execute_slash_command("/team blueprint no-such-blueprint")
        .await;
    assert_eq!(app.status, "Blueprint 'no-such-blueprint' not found");
}

// ---------------------------------------------------------------------------
// /team - message, tasks, clear, cleanup
// ---------------------------------------------------------------------------

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn team_message_reports_send_failure_and_missing_team() {
    let (_g, _e, _t, mut app) = team_app();
    active_team_on_disk(&mut app, "msg-team").await;
    app.team_members
        .push(TeamMember::new("alice", "tm-001", "general"));

    // A message over the mailbox size limit is the real error path of the
    // handler (SEC-ragent-team-006); the lead-origin sender is authenticated
    // (SEC-ragent-team-003), so only the size boundary rejects here.
    let oversized = "x".repeat(MAX_MESSAGE_BYTES + 1);
    app.execute_slash_command(&format!("/team message alice {oversized}"))
        .await;
    assert!(
        app.status.starts_with("Failed to send message:"),
        "status: {}",
        app.status
    );

    // An active team whose directory is gone fails at load.
    std::fs::remove_dir_all(
        std::env::current_dir()
            .expect("cwd")
            .join(".ragent/teams/msg-team"),
    )
    .expect("remove team dir");
    app.execute_slash_command("/team message alice ping").await;
    assert!(
        app.status.starts_with("Failed to load team:"),
        "status: {}",
        app.status
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn team_tasks_renders_completed_and_cancelled_rows() {
    let (_g, _e, temp, mut app) = team_app();
    active_team_on_disk(&mut app, "task-team").await;

    let store = TeamStore::load_by_name("task-team", temp.path()).expect("load team");
    let task_store = store.task_store().expect("task store");
    let mut done = TeamTask::new("task-001", "Ship it");
    done.status = TeamTaskStatus::Completed;
    done.assigned_to = Some("lead".to_string());
    task_store.add_task(done).await.expect("add completed task");
    let mut dropped = TeamTask::new("task-002", "Dropped work");
    dropped.status = TeamTaskStatus::Cancelled;
    task_store
        .add_task(dropped)
        .await
        .expect("add cancelled task");

    app.execute_slash_command("/team tasks").await;
    assert_eq!(app.status, "2 task(s)");
    let text = last_message(&app);
    assert!(text.contains("completed"), "text: {text}");
    assert!(text.contains("cancelled"), "text: {text}");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn team_tasks_without_task_file_reports_no_tasks() {
    let (_g, _e, _t, mut app) = team_app();
    active_team_on_disk(&mut app, "empty-task-team").await;

    app.execute_slash_command("/team tasks").await;
    assert_eq!(app.status, "0 task(s)");
    assert!(last_message(&app).contains("(no tasks)"));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn team_tasks_with_missing_team_directory_fails_to_load() {
    let (_g, _e, _t, mut app) = team_app();
    active_team_on_disk(&mut app, "gone-task-team").await;
    std::fs::remove_dir_all(
        std::env::current_dir()
            .expect("cwd")
            .join(".ragent/teams/gone-task-team"),
    )
    .expect("remove team dir");

    app.execute_slash_command("/team tasks").await;
    assert!(
        app.status.starts_with("Failed to load team:"),
        "status: {}",
        app.status
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn team_clear_removes_task_file_and_reports_missing_team() {
    let (_g, _e, temp, mut app) = team_app();
    active_team_on_disk(&mut app, "clear-team").await;

    let store = TeamStore::load_by_name("clear-team", temp.path()).expect("load team");
    store
        .task_store()
        .expect("task store")
        .add_task(TeamTask::new("task-001", "Clear me"))
        .await
        .expect("add task");

    app.execute_slash_command("/team clear").await;
    assert_eq!(app.status, "team tasks cleared");
    assert!(last_message(&app).contains("Cleared 1 task(s)"));

    std::fs::remove_dir_all(
        std::env::current_dir()
            .expect("cwd")
            .join(".ragent/teams/clear-team"),
    )
    .expect("remove team dir");
    app.execute_slash_command("/team clear").await;
    assert!(
        app.status.starts_with("Failed to load team:"),
        "status: {}",
        app.status
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn team_cleanup_removes_on_disk_team() {
    let (_g, _e, temp, mut app) = team_app();
    active_team_on_disk(&mut app, "cleanup-team").await;
    let dir = temp.path().join(".ragent/teams/cleanup-team");
    assert!(dir.is_dir());

    app.execute_slash_command("/team cleanup").await;
    assert_eq!(app.status, "team cleaned up");
    assert!(app.active_team.is_none());
    assert!(!dir.exists(), "team dir must be removed");
    assert!(last_message(&app).contains("cleaned up"));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn team_forcecleanup_deactivates_persisted_members() {
    let (_g, _e, temp, mut app) = team_app();
    active_team_on_disk(&mut app, "force-members").await;

    // Persist members in the team config so the confirmed run has teammates to
    // deactivate even though the TUI roster is empty.
    let mut store = TeamStore::load_by_name("force-members", temp.path()).expect("load team");
    let mut alice = TeamMember::new("alice", "tm-001", "general");
    alice.status = MemberStatus::Working;
    store.add_member(alice).expect("add member");
    let dir = store.dir.clone();

    app.execute_slash_command("/team forcecleanup confirm")
        .await;
    assert_eq!(app.status, "team force cleaned up");
    assert!(app.active_team.is_none());
    assert!(!dir.exists(), "team dir must be removed");
    let text = last_message(&app);
    assert!(text.contains("alice"), "text: {text}");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn team_forcecleanup_with_missing_directory_reports_failure() {
    let (_g, _e, _t, mut app) = team_app();
    active_team_on_disk(&mut app, "vanish-team").await;
    std::fs::remove_dir_all(
        std::env::current_dir()
            .expect("cwd")
            .join(".ragent/teams/vanish-team"),
    )
    .expect("remove team dir");

    app.execute_slash_command("/team forcecleanup confirm")
        .await;
    assert!(
        app.status.starts_with("Failed to force cleanup team:"),
        "status: {}",
        app.status
    );
}

// ---------------------------------------------------------------------------
// /config - env sources, save failure
// ---------------------------------------------------------------------------

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn config_show_reports_env_file_and_inline_content_sources() {
    let (_g, _e, temp, mut app) = isolated_app();

    let env_path = temp.path().join("env-ragent.json");
    std::fs::write(
        &env_path,
        r#"{ "defaultAgent": "coder", "memory": { "enabled": false } }"#,
    )
    .expect("write env config");
    unsafe {
        std::env::set_var("RAGENT_CONFIG", &env_path);
        std::env::set_var(
            "RAGENT_CONFIG_CONTENT",
            r#"{ "defaultAgent": "explorer", "compaction": { "auto": false } }"#,
        );
    }

    app.execute_slash_command("/config show").await;
    assert_eq!(app.status, "config: show");
    let text = last_message(&app);
    assert!(text.contains("Env (RAGENT_CONFIG)"), "text: {text}");
    // The source column is rendered in a narrow table cell and therefore wraps,
    // so match the source label and the file name separately.
    assert!(text.contains("env"), "source column: {text}");
    assert!(text.contains("env-ragent.json"), "source column: {text}");
    assert!(text.contains("RAGENT_C"), "source column: {text}");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn config_show_reports_missing_env_file_marker() {
    let (_g, _e, temp, mut app) = isolated_app();
    let missing = temp.path().join("does-not-exist.json");
    unsafe {
        std::env::set_var("RAGENT_CONFIG", &missing);
    }

    app.execute_slash_command("/config show").await;
    let text = last_message(&app);
    assert!(text.contains("[x]"), "missing marker: {text}");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn config_save_reports_backup_failure_without_global_config() {
    let (_g, _e, _t, mut app) = isolated_app();
    // The isolated global config home has no ragent.json yet.
    app.execute_slash_command("/config save").await;
    assert_eq!(app.status, "config: save error");
    assert!(last_message(&app).contains("Failed to back up global config"));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn config_save_then_list_opens_picker() {
    let (_g, _e, _t, mut app) = isolated_app();
    write_global_config(r#"{ "defaultAgent": "general" }"#);

    app.execute_slash_command("/config save").await;
    assert_eq!(app.status, "config: saved");

    app.execute_slash_command("/config list").await;
    assert_eq!(app.status, "config: list");
    let picker = app.config_save_picker.as_ref().expect("picker populated");
    assert_eq!(picker.entries.len(), 1);
    assert_eq!(picker.selected, 0);
    assert!(last_message(&app).contains("1 saved configuration(s)"));
}

// ---------------------------------------------------------------------------
// /init config - failure branches
// ---------------------------------------------------------------------------

#[cfg(unix)]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn init_config_reports_create_dir_error() {
    use std::os::unix::fs::PermissionsExt as _;

    let (_g, _e, temp, mut app) = isolated_app();

    // Point the global config home at a path whose parent is a regular FILE, so
    // `create_dir_all` cannot succeed.
    let blocker = temp.path().join("blocker-file");
    std::fs::write(&blocker, "not a directory").expect("write blocker");
    unsafe {
        std::env::set_var("XDG_CONFIG_HOME", blocker.join("nested"));
    }

    app.execute_slash_command("/init config").await;
    assert_eq!(app.status, "init config: error");
    assert!(
        last_message(&app).contains("Failed to create config directory"),
        "text: {}",
        last_message(&app)
    );

    // Leave the tempdir permissions clean for cleanup.
    let _ = std::fs::set_permissions(&blocker, std::fs::Permissions::from_mode(0o600));
}

#[cfg(unix)]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn init_config_reports_write_error() {
    use std::os::unix::fs::PermissionsExt as _;

    let (_g, _e, temp, mut app) = isolated_app();

    // A read-only config directory: `create_dir_all` succeeds (it exists) but
    // the file write is refused.
    let dir = temp.path().join("xdg").join("ragent");
    std::fs::create_dir_all(&dir).expect("create config dir");
    unsafe {
        std::env::set_var("XDG_CONFIG_HOME", temp.path().join("xdg"));
    }
    std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o500))
        .expect("chmod config dir");

    app.execute_slash_command("/init config").await;
    std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o700))
        .expect("restore config dir mode");

    assert_eq!(app.status, "init config: error");
    assert!(
        last_message(&app).contains("Failed to write config file"),
        "text: {}",
        last_message(&app)
    );
}

#[cfg(not(unix))]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn init_config_reports_create_dir_error() {
    // Permission-based divergence is Unix-only; other platforms skip.
}

// ---------------------------------------------------------------------------
// /model - bare invocation per configured provider
// ---------------------------------------------------------------------------

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn model_bare_opens_azure_resource_picker_with_last_selection() {
    let (_g, _e, temp, mut app) = isolated_app();

    let azure_dir = temp.path().join(".ragent");
    std::fs::create_dir_all(&azure_dir).expect("create .ragent");
    std::fs::write(
        azure_dir.join("azureresources.json"),
        r#"{"version": "1", "resources": [
            {"id": "deployment-a", "name": "A", "endpoint": "https://a.openai.azure.com", "api_key": "k"},
            {"id": "deployment-b", "name": "B", "endpoint": "https://b.openai.azure.com", "api_key": "k"}
        ]}"#,
    )
    .expect("write azure resources");
    app.storage
        .set_setting("azure_resource_last_selection", r#"{"id": "deployment-b"}"#)
        .expect("store last selection");
    app.configured_provider = Some(ConfiguredProvider {
        id: "azure_resource".to_string(),
        name: "Azure Resource (File)".to_string(),
        source: ProviderSource::Database,
    });

    app.execute_slash_command("/model").await;
    match app.provider_setup.as_ref() {
        Some(ragent_tui::app::ProviderSetupStep::SelectAzureResource {
            entries, selected, ..
        }) => {
            assert_eq!(entries.len(), 2);
            assert_eq!(*selected, 1, "last selection must be restored");
        }
        other => panic!("expected SelectAzureResource, got {other:?}"),
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn model_bare_azure_resource_without_entries_falls_back_to_provider_picker() {
    let (_g, _e, temp, mut app) = isolated_app();
    std::fs::create_dir_all(temp.path().join(".ragent")).expect("create .ragent");
    app.configured_provider = Some(ConfiguredProvider {
        id: "azure_resource".to_string(),
        name: "Azure Resource (File)".to_string(),
        source: ProviderSource::Database,
    });

    app.execute_slash_command("/model").await;
    assert!(matches!(
        app.provider_setup,
        Some(ragent_tui::app::ProviderSetupStep::SelectProvider { .. })
    ));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn model_bare_router_without_providers_falls_back_to_provider_picker() {
    let (_g, _e, _t, mut app) = isolated_app();
    // The router arm enumerates providers from the environment too, so every
    // candidate is explicitly marked reset to make "no providers" deterministic
    // regardless of what a parallel test may have exported.
    for (id, _) in ragent_tui::app::PROVIDER_LIST {
        if *id == "router" {
            continue;
        }
        app.storage
            .set_setting(&format!("provider_{id}_disabled"), "1")
            .expect("mark provider disabled");
    }
    app.configured_provider = Some(ConfiguredProvider {
        id: "router".to_string(),
        name: "Model Router".to_string(),
        source: ProviderSource::Database,
    });

    app.execute_slash_command("/model").await;
    assert!(matches!(
        app.provider_setup,
        Some(ragent_tui::app::ProviderSetupStep::SelectProvider { .. })
    ));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn model_bare_router_with_providers_seeds_router_setup() {
    let (_g, _e, _t, mut app) = isolated_app();
    app.storage
        .set_provider_auth("anthropic", "sk-test")
        .expect("store provider auth");
    app.configured_provider = Some(ConfiguredProvider {
        id: "router".to_string(),
        name: "Model Router".to_string(),
        source: ProviderSource::Database,
    });

    app.execute_slash_command("/model").await;
    assert!(
        app.provider_setup.is_some(),
        "router setup step must be seeded"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn model_bare_other_provider_starts_discovery() {
    let (_g, _e, _t, mut app) = isolated_app();
    app.configured_provider = Some(ConfiguredProvider {
        id: "anthropic".to_string(),
        name: "Anthropic (Claude)".to_string(),
        source: ProviderSource::EnvVar,
    });

    app.execute_slash_command("/model").await;
    match app.provider_setup.as_ref() {
        Some(ragent_tui::app::ProviderSetupStep::LoadingModels {
            provider_id,
            provider_name,
        }) => {
            assert_eq!(provider_id, "anthropic");
            assert_eq!(provider_name, "Anthropic (Claude)");
        }
        other => panic!("expected LoadingModels, got {other:?}"),
    }
}

// ---------------------------------------------------------------------------
// /thinking
// ---------------------------------------------------------------------------

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn thinking_rejects_level_when_model_has_no_thinking_support() {
    let (_g, _e, _t, mut app) = isolated_app();
    // An Azure Foundry model resolves to no picker entry, so the model reports
    // an empty set of thinking levels.
    app.selected_model = Some("azure_foundry/unknown-model".to_string());

    app.execute_slash_command("/thinking high").await;
    assert_eq!(
        app.status,
        "[warn] Active model does not support configurable thinking"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn thinking_off_is_persisted_without_support_check() {
    let (_g, _e, _t, mut app) = isolated_app();
    app.selected_model = Some("azure_foundry/unknown-model".to_string());

    app.execute_slash_command("/thinking off").await;
    assert_eq!(app.status, "thinking: off");
    assert_eq!(
        app.storage
            .get_setting("thinking_level")
            .expect("read setting")
            .as_deref(),
        Some("off")
    );
}

// ---------------------------------------------------------------------------
// /cancel and /bench
// ---------------------------------------------------------------------------

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn cancel_targets_benchmark_flag_and_active_task() {
    let (_g, _e, _t, mut app) = isolated_app();

    let flag = Arc::new(AtomicBool::new(false));
    app.active_bench_task_id = Some("bench-42".to_string());
    app.active_bench_cancel = Some(Arc::clone(&flag));

    app.execute_slash_command("/cancel bench").await;
    assert_eq!(app.status, "[wait] bench: cancellation requested");
    assert!(flag.load(Ordering::Relaxed), "cancel flag must be set");

    app.active_bench_task_id = None;
    app.active_bench_cancel = None;
    app.active_tasks.push(task_entry("task-abc", "explore"));

    app.execute_slash_command("/cancel task").await;
    // FR-023: the row is not removed optimistically - `/cancel` delegates to
    // `AgentManager::cancel_agent` and the Agents window retires the row when
    // the registry reports the transition (`test_cancel_slash` owns the full
    // delegation contract).
    assert!(
        app.status.starts_with("Cancellation requested for"),
        "status: {}",
        app.status
    );
    assert_eq!(app.active_tasks.len(), 1);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn bench_show_and_cancel_without_active_run() {
    let (_g, _e, _t, mut app) = isolated_app();

    app.execute_slash_command("/bench show").await;
    assert_eq!(app.status, "bench show");

    app.execute_slash_command("/bench cancel").await;
    assert_eq!(app.status, "No active benchmark run");
    assert!(last_message(&app).contains("No active benchmark run"));
}

// ---------------------------------------------------------------------------
// /dirs - populated lists, --global scope, not-found and error branches
// ---------------------------------------------------------------------------

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn dirs_global_add_and_remove_round_trip() {
    let (_g, _e, _t, mut app) = isolated_app();

    app.execute_slash_command("/dirs add allow zz-arms-deep/**")
        .await;
    assert!(
        last_message(&app).contains("Added `zz-arms-deep/**` to the **allowlist**"),
        "text: {}",
        last_message(&app)
    );

    app.execute_slash_command("/dirs add deny zz-arms-denied/** --global")
        .await;
    let text = last_message(&app);
    assert!(text.contains("Added `zz-arms-denied/**`"), "text: {text}");
    assert!(
        text.contains("global: `~/.config/ragent/ragent.json`"),
        "text: {text}"
    );

    app.execute_slash_command("/dirs add roots zz-arms-root --global")
        .await;
    assert!(last_message(&app).contains("allowed_roots"));

    // Populated show output.
    app.execute_slash_command("/dirs show").await;
    let shown = last_message(&app);
    assert!(shown.contains("zz-arms-deep/**"), "text: {shown}");
    assert!(shown.contains("zz-arms-denied/**"), "text: {shown}");
    assert!(shown.contains("zz-arms-root"), "text: {shown}");

    app.execute_slash_command("/dirs remove allow zz-arms-deep/**")
        .await;
    assert!(last_message(&app).contains("Removed `zz-arms-deep/**`"));
    app.execute_slash_command("/dirs remove deny zz-arms-denied/** --global")
        .await;
    assert!(last_message(&app).contains("Removed `zz-arms-denied/**`"));
    app.execute_slash_command("/dirs remove roots zz-arms-root --global")
        .await;
    assert!(last_message(&app).contains("Removed `zz-arms-root`"));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn dirs_remove_not_found_warns() {
    let (_g, _e, _t, mut app) = isolated_app();

    app.execute_slash_command("/dirs remove deny never-added-deny")
        .await;
    assert!(
        last_message(&app).contains("was not in the project denylist"),
        "text: {}",
        last_message(&app)
    );

    app.execute_slash_command("/dirs remove roots never-added-root")
        .await;
    assert!(
        last_message(&app).contains("was not in the project allowed_roots"),
        "text: {}",
        last_message(&app)
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn dirs_persist_errors_are_reported() {
    let (_g, _e, temp, mut app) = isolated_app();
    let blocker = temp.path().join("blocker");
    std::fs::write(&blocker, "file").expect("write blocker");
    unsafe {
        std::env::set_var("XDG_CONFIG_HOME", blocker.join("nested"));
    }

    for args in [
        "/dirs add allow err-allow --global",
        "/dirs add deny err-deny --global",
        "/dirs add roots err-root --global",
        "/dirs remove allow err-allow --global",
        "/dirs remove deny err-deny --global",
        "/dirs remove roots err-root --global",
    ] {
        app.execute_slash_command(args).await;
        assert!(
            last_message(&app).contains("[err] Error:"),
            "{args} -> {}",
            last_message(&app)
        );
    }
}

// ---------------------------------------------------------------------------
// /bash - populated lists, --global scope, not-found and error branches
// ---------------------------------------------------------------------------

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn bash_global_add_remove_and_populated_show() {
    let (_g, _e, _t, mut app) = isolated_app();

    app.execute_slash_command("/bash add allow zzcurl").await;
    app.execute_slash_command("/bash add deny zzforbidden")
        .await;
    assert!(last_message(&app).contains("Added `zzforbidden`"));

    app.execute_slash_command("/bash show").await;
    let shown = last_message(&app);
    assert!(shown.contains("zzcurl"), "text: {shown}");
    assert!(shown.contains("zzforbidden"), "text: {shown}");

    app.execute_slash_command("/bash add allow zzglobalcurl --global")
        .await;
    let text = last_message(&app);
    assert!(
        text.contains("global: `~/.config/ragent/ragent.json`"),
        "text: {text}"
    );

    app.execute_slash_command("/bash remove allow zzcurl").await;
    assert!(last_message(&app).contains("Removed `zzcurl`"));
    app.execute_slash_command("/bash remove deny zzforbidden")
        .await;
    assert!(last_message(&app).contains("Removed `zzforbidden`"));
    app.execute_slash_command("/bash remove allow zzglobalcurl --global")
        .await;
    assert!(last_message(&app).contains("Removed `zzglobalcurl`"));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn bash_remove_not_found_warns() {
    let (_g, _e, _t, mut app) = isolated_app();

    app.execute_slash_command("/bash remove allow zznever-added")
        .await;
    assert!(
        last_message(&app).contains("was not in the project allowlist"),
        "text: {}",
        last_message(&app)
    );

    app.execute_slash_command("/bash remove deny zznever-added-deny")
        .await;
    assert!(
        last_message(&app).contains("was not in the project denylist"),
        "text: {}",
        last_message(&app)
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn bash_persist_errors_are_reported() {
    let (_g, _e, temp, mut app) = isolated_app();
    let blocker = temp.path().join("bash-blocker");
    std::fs::write(&blocker, "file").expect("write blocker");
    unsafe {
        std::env::set_var("XDG_CONFIG_HOME", blocker.join("nested"));
    }

    for args in [
        "/bash add allow errtool --global",
        "/bash add deny errpattern --global",
        "/bash remove allow errtool --global",
        "/bash remove deny errpattern --global",
    ] {
        app.execute_slash_command(args).await;
        assert!(
            last_message(&app).contains("[err] Error:"),
            "{args} -> {}",
            last_message(&app)
        );
    }
}

// ---------------------------------------------------------------------------
// /agents - global OASF agents and diagnostics
// ---------------------------------------------------------------------------

/// Write a valid OASF agent record plus a malformed one into the isolated
/// user-global agents directory.
fn seed_global_custom_agents() {
    let dir = global_config_dir().join("agents");
    std::fs::create_dir_all(&dir).expect("create global agents dir");
    std::fs::write(
        dir.join("reviewer.json"),
        r#"{
            "name": "global-reviewer",
            "description": "Reviews code from the user-global scope",
            "version": "1.0.0",
            "schema_version": "0.7.0",
            "modules": [{
                "type": "ragent/agent/v1",
                "payload": { "system_prompt": "You review code." }
            }]
        }"#,
    )
    .expect("write global agent");
    std::fs::write(dir.join("broken.json"), "{ not an oasf record").expect("write broken agent");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn agents_lists_global_oasf_agent_and_diagnostics() {
    let (_g, _e, _t, mut app) = isolated_app();
    seed_global_custom_agents();

    app.execute_slash_command("/reload agents").await;
    app.execute_slash_command("/agents").await;
    assert_eq!(app.status, "agents");
    let text = last_message(&app);
    assert!(text.contains("global-reviewer"), "text: {text}");
    // The scope label is `project`/`global`; the format label is `oasf`.
    assert!(text.contains("/oasf]"), "text: {text}");
    assert!(text.contains("Diagnostics"), "text: {text}");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn reload_agents_reports_builtin_name_collision() {
    let (_g, _e, _t, mut app) = isolated_app();
    let dir = std::env::current_dir().expect("cwd").join(".ragent/agents");
    std::fs::create_dir_all(&dir).expect("create project agents dir");
    std::fs::write(
        dir.join("general.md"),
        "---\n{\"name\": \"general\", \"description\": \"Shadowed general\"}\n---\n\nYou are general.\n",
    )
    .expect("write colliding profile");

    app.execute_slash_command("/reload agents").await;
    // The collision diagnostic is emitted to the log, not the chat report.
    assert!(
        app.log_entries
            .iter()
            .any(|e| e.message.contains("custom:general")),
        "logs: {:?}",
        app.log_entries
            .iter()
            .map(|e| e.message.clone())
            .collect::<Vec<_>>()
    );
}

// ---------------------------------------------------------------------------
// /reload - config parse failure
// ---------------------------------------------------------------------------

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn reload_config_reports_parse_failure_for_config_and_mcp() {
    let (_g, _e, _t, mut app) = isolated_app();
    write_global_config("{ this is not json");
    ragent_config::Config::invalidate_load_cache();

    app.execute_slash_command("/reload all").await;
    assert_eq!(app.status, "reload");
    let text = last_message(&app);
    assert!(text.contains("Config reload failed"), "text: {text}");
    assert!(text.contains("MCP reload failed"), "text: {text}");
}

// ---------------------------------------------------------------------------
// /yolo - persist failure
// ---------------------------------------------------------------------------

#[cfg(unix)]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn yolo_persist_failure_is_reported() {
    let (_g, _e, _t, mut app) = isolated_app();
    // Make `<xdg>/ragent/ragent.json` a DIRECTORY so the atomic rename into it
    // cannot succeed.
    let dir = global_config_dir();
    std::fs::create_dir_all(dir.join("ragent.json")).expect("create directory at config path");
    ragent_config::Config::invalidate_load_cache();

    app.execute_slash_command("/yolo").await;
    assert!(
        app.status
            .starts_with("[warn] failed to persist YOLO mode:"),
        "status: {}",
        app.status
    );
    assert!(last_message(&app).contains("Failed to persist YOLO mode"));
}

// ---------------------------------------------------------------------------
// /name - set and clear
// ---------------------------------------------------------------------------

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn name_sets_then_clears_the_session_title() {
    let (_g, _e, _t, mut app) = isolated_app();

    app.execute_slash_command("/name scratch-session").await;
    assert_eq!(app.status, "Session name set to 'scratch-session'");

    app.execute_slash_command("/name").await;
    assert_eq!(app.status, "Session name cleared");
}

// ---------------------------------------------------------------------------
// /memory - panel toggle off
// ---------------------------------------------------------------------------

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn memory_show_toggles_panel_off() {
    let (_g, _e, _t, mut app) = isolated_app();

    app.execute_slash_command("/memory show").await;
    assert_eq!(app.status, "memory panel visible");
    assert!(app.show_memory);

    app.execute_slash_command("/memory show").await;
    assert_eq!(app.status, "memory panel hidden");
    assert!(!app.show_memory);
}

// ---------------------------------------------------------------------------
// /mcp - status variants and connect/disconnect reporting
// ---------------------------------------------------------------------------

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn mcp_list_renders_needs_auth_and_failed_statuses() {
    let (_g, _e, _t, mut app) = isolated_app();
    use ragent_agent::mcp::{McpServer, McpStatus};
    app.mcp_servers = vec![
        McpServer {
            id: "auth-needed".to_string(),
            config: ragent_agent::McpServerConfig::default(),
            status: McpStatus::NeedsAuth,
            tools: Vec::new(),
        },
        McpServer {
            id: "broken-server".to_string(),
            config: ragent_agent::McpServerConfig::default(),
            status: McpStatus::Failed {
                error: "spawn refused".to_string(),
            },
            tools: Vec::new(),
        },
    ];

    app.execute_slash_command("/mcp").await;
    assert_eq!(app.status, "mcp");
    let text = last_message(&app);
    assert!(text.contains("needs auth"), "text: {text}");
    assert!(text.contains("failed: spawn refused"), "text: {text}");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn mcp_connect_unknown_server_reports_error() {
    let (_g, _e, _t, mut app) = isolated_app();

    app.execute_slash_command("/mcp connect no-such-server")
        .await;
    assert!(
        last_message(&app).contains("is not a known MCP server"),
        "text: {}",
        last_message(&app)
    );

    app.execute_slash_command("/mcp disconnect no-such-server")
        .await;
    assert!(
        last_message(&app).contains("disabled") || last_message(&app).contains("[ok]"),
        "text: {}",
        last_message(&app)
    );
}

// ---------------------------------------------------------------------------
// /autopilot - argument errors and status
// ---------------------------------------------------------------------------

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn autopilot_invalid_argument_and_status_off() {
    let (_g, _e, _t, mut app) = isolated_app();

    app.execute_slash_command("/autopilot on --max-time soon")
        .await;
    assert_eq!(app.status, "autopilot: invalid argument");
    assert!(last_message(&app).contains("--max-time soon"));

    app.execute_slash_command("/autopilot on --max-tokens 100 --max-time 60")
        .await;
    assert_eq!(app.status, " autopilot");
    let text = last_message(&app);
    assert!(text.contains("Token budget: 100"), "text: {text}");
    assert!(text.contains("Time limit: 60s"), "text: {text}");

    app.execute_slash_command("/autopilot off").await;
    app.execute_slash_command("/autopilot status").await;
    assert!(last_message(&app).contains("Autopilot: **OFF**"));
}

// ---------------------------------------------------------------------------
// /swarm - decomposition dispatch
// ---------------------------------------------------------------------------

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn swarm_decomposition_runs_with_an_unusable_provider() {
    let (_g, _e, _t, mut app) = isolated_app();
    // An unregistered provider makes the background completer fail fast, so
    // the spawned decomposition task still records a result.
    app.selected_model = Some("no-such-provider/no-such-model".to_string());

    app.execute_slash_command("/swarm audit the repository")
        .await;
    assert_eq!(app.status, "[wait] swarm: decomposing goal...");
    assert!(last_message(&app).contains("Swarm Decomposition"));

    // Wait for the background task to deposit its (error) outcome.
    for _ in 0..200 {
        if app.swarm_result.lock().is_ok_and(|g| g.is_some()) {
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(10)).await;
    }
    let result = app
        .swarm_result
        .lock()
        .expect("swarm result mutex")
        .clone()
        .expect("decomposition outcome recorded");
    assert!(result.is_err(), "unregistered provider must fail");
}

// ---------------------------------------------------------------------------
// /webapi - already running, disable, and the running status line
// ---------------------------------------------------------------------------

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn webapi_already_running_disable_and_status_with_token() {
    let (_g, _e, _t, mut app) = isolated_app();
    // Port 0 keeps parallel tests from colliding on the default port.
    app.webapi_addr = "127.0.0.1:0".to_string();

    app.execute_slash_command("/webapi enable").await;
    assert!(app.webapi_server.is_some(), "server handle must be set");

    app.execute_slash_command("/webapi enable").await;
    assert!(
        last_message(&app).contains("already running"),
        "text: {}",
        last_message(&app)
    );

    app.execute_slash_command("/webapi status").await;
    let text = last_message(&app);
    assert!(text.contains("**Running**"), "text: {text}");
    assert!(text.contains("Bearer Token"), "text: {text}");

    app.execute_slash_command("/webapi disable").await;
    assert_eq!(last_message(&app), "[stop] **Web API disabled.**");
    assert!(app.webapi_server.is_none());
    assert!(app.webapi_token.is_none());
}

// ---------------------------------------------------------------------------
// /websearch - test and search dispatch
// ---------------------------------------------------------------------------

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn websearch_test_and_search_hand_off_to_the_background_renderer() {
    let (_g, _e, _t, mut app) = isolated_app();

    app.execute_slash_command("/websearch test").await;
    assert_eq!(app.status, "websearch: testing engines...");
    assert!(last_message(&app).contains("Starting Websearch test"));

    app.execute_slash_command("/websearch search rust ownership")
        .await;
    assert_eq!(app.status, "websearch: searching...");
    assert!(last_message(&app).contains("Starting web search for \"rust ownership\""));
}

// ---------------------------------------------------------------------------
// /gitlab - configured status variants
// ---------------------------------------------------------------------------

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn gitlab_status_reports_configured_and_partial_states() {
    let (_g, _e, _t, mut app) = isolated_app();

    let config = ragent_agent::gitlab::auth::GitLabConfig {
        instance_url: "https://gitlab.example.com".to_string(),
        username: "alice".to_string(),
    };
    ragent_agent::gitlab::auth::save_config(app.storage.as_ref(), &config)
        .expect("save gitlab config");

    app.execute_slash_command("/gitlab status").await;
    let partial = last_message(&app);
    assert!(partial.contains("partially configured"), "text: {partial}");
    assert!(partial.contains("gitlab.example.com"), "text: {partial}");

    ragent_agent::gitlab::auth::save_token(app.storage.as_ref(), "glpat-test")
        .expect("save gitlab token");
    app.execute_slash_command("/gitlab status").await;
    let full = last_message(&app);
    assert!(full.contains("GitLab configured"), "text: {full}");
    assert!(full.contains("Token**: [ok] configured"), "text: {full}");

    app.execute_slash_command("/gitlab setup").await;
    assert!(matches!(
        app.provider_setup,
        Some(ragent_tui::app::ProviderSetupStep::GitLabSetup { .. })
    ));
}

// ---------------------------------------------------------------------------
// /update - install and check dispatch
// ---------------------------------------------------------------------------

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn update_install_and_check_start_background_work() {
    let (_g, _e, _t, mut app) = isolated_app();

    app.execute_slash_command("/update install").await;
    assert!(
        last_message(&app).contains("Downloading latest release"),
        "text: {}",
        last_message(&app)
    );

    app.execute_slash_command("/update").await;
    assert!(
        last_message(&app).contains("Checking for updates"),
        "text: {}",
        last_message(&app)
    );
}

// ---------------------------------------------------------------------------
// /init - project analysis dispatch
// ---------------------------------------------------------------------------

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn init_analysis_dispatches_a_background_run() {
    let (_g, _e, _t, mut app) = isolated_app();

    app.execute_slash_command("/init").await;
    assert_eq!(app.status, "init: analysing project...");
    assert!(app.is_processing);
    let text = all_messages(&app);
    assert!(text.contains("Analysing project"), "text: {text}");
    assert!(text.contains("PROJECT_ANALYSIS.md"), "text: {text}");
}

// ---------------------------------------------------------------------------
// /doctor - provider, config, guidelines and index status lines
// ---------------------------------------------------------------------------

/// Run `/doctor` and return the published diagnostic report.
async fn run_doctor(app: &mut App) -> String {
    let mut rx = app.event_bus.subscribe();
    app.execute_slash_command("/doctor").await;
    for _ in 0..600 {
        if let Ok(ragent_agent::event::Event::AgentNotice { message, .. }) = rx.try_recv()
            && message.contains("Diagnostic Report")
        {
            return message;
        }
        tokio::time::sleep(std::time::Duration::from_millis(25)).await;
    }
    panic!("doctor did not publish a diagnostic report");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn doctor_reports_provider_model_and_guidelines() {
    let (_g, _e, temp, mut app) = isolated_app();
    std::fs::write(temp.path().join("AGENTS.md"), "# Project rules\n").expect("write AGENTS.md");
    // The memory section must live in the PROJECT config: a project overlay
    // re-applies the serde defaults for every section it contains, so a global
    // file alone would be overwritten by the project overlay's defaults.
    std::fs::write(
        temp.path().join(".ragent/ragent.json"),
        r#"{
            "defaultAgent": "general",
            "memory": { "enabled": false, "structured": { "enabled": false } }
        }"#,
    )
    .expect("write project config");
    ragent_config::Config::invalidate_load_cache();
    write_global_config(r#"{ "mcp": { "demo": { "type": "stdio", "command": "echo" } } }"#);
    // `GITHUB_TOKEN` feeds GitHub auth AND the copilot provider-credential
    // probe, so it must be restored before the test ends.
    let previous_github_token = std::env::var("GITHUB_TOKEN").ok();
    unsafe {
        std::env::set_var("GITHUB_TOKEN", "ghp_armsdeeptoken");
    }
    let health = Arc::clone(&app.provider_health);
    health.store(2, Ordering::Relaxed);
    app.configured_provider = Some(ConfiguredProvider {
        id: "anthropic".to_string(),
        name: "Anthropic (Claude)".to_string(),
        source: ProviderSource::EnvVar,
    });
    app.selected_model = Some("anthropic/claude-sonnet-4".to_string());
    app.code_index = None;
    app.code_index_enabled = false;

    let report = run_doctor(&mut app).await;

    assert!(
        report.contains("provider: Anthropic (Claude) (anthropic via env)"),
        "report: {report}"
    );
    assert!(
        report.contains("model=anthropic/claude-sonnet-4"),
        "report: {report}"
    );
    assert!(report.contains("unreachable"), "report: {report}");
    assert!(report.contains("GitHub token"), "report: {report}");
    assert!(
        report.contains("project guidelines loaded: AGENTS.md"),
        "report: {report}"
    );
    assert!(
        report.contains("MCP servers configured"),
        "report: {report}"
    );
    assert!(report.contains("code index: disabled"), "report: {report}");
    assert!(
        report.contains("memory system (blocks="),
        "report: {report}"
    );

    unsafe {
        match previous_github_token {
            Some(value) => std::env::set_var("GITHUB_TOKEN", value),
            None => std::env::remove_var("GITHUB_TOKEN"),
        }
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn doctor_reports_missing_ripgrep_and_config_error() {
    let (_g, _e, temp, mut app) = isolated_app();

    // A `rg` stand-in that always fails, prepended to PATH so no other tool
    // lookup is disturbed.
    let bin = temp.path().join("bin");
    std::fs::create_dir_all(&bin).expect("create bin dir");
    let fake_rg = bin.join("rg");
    std::fs::write(&fake_rg, "#!/bin/sh\nexit 1\n").expect("write fake rg");
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt as _;
        std::fs::set_permissions(&fake_rg, std::fs::Permissions::from_mode(0o755))
            .expect("chmod fake rg");
    }
    let original_path = std::env::var("PATH").unwrap_or_default();
    unsafe {
        std::env::set_var("PATH", format!("{}:{original_path}", bin.display()));
    }

    // A corrupt global config exercises the load-error line.
    write_global_config("{ not json");

    let report = run_doctor(&mut app).await;
    unsafe {
        std::env::set_var("PATH", &original_path);
    }

    assert!(report.contains("ripgrep not found"), "report: {report}");
    assert!(report.contains("parse error"), "report: {report}");
}

// ---------------------------------------------------------------------------
// /codeindex - language list, graph usage, watcher, rebuild, skillgen, misses
// ---------------------------------------------------------------------------

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn codeindex_language_list_and_graph_usage() {
    let (_g, _e, _t, mut app) = isolated_app();

    app.execute_slash_command("/codeindex lang").await;
    let text = last_message(&app);
    assert!(text.contains("Supported Languages"), "text: {text}");
    assert!(text.contains("**Total:"), "text: {text}");
    assert!(
        app.status.starts_with("codeindex: ") && app.status.ends_with("languages supported"),
        "status: {}",
        app.status
    );

    app.execute_slash_command("/codeindex graph bogus").await;
    assert!(
        last_message(&app).contains("Usage: `/codeindex graph"),
        "text: {}",
        last_message(&app)
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn codeindex_on_twice_reports_already_active_with_watcher() {
    let (_g, _e, _t, mut app) = isolated_app();
    app.code_index = None;

    app.execute_slash_command("/codeindex on").await;
    assert_eq!(app.status, "codeindex: on");
    assert!(
        app.code_index_watch_session.is_some(),
        "file watcher must start"
    );

    app.execute_slash_command("/codeindex on").await;
    assert_eq!(app.status, "codeindex: on");
    assert!(
        last_message(&app).contains("already active and enabled"),
        "text: {}",
        last_message(&app)
    );
    if let Some(mut session) = app.code_index_watch_session.take() {
        session.stop();
    }
}

/// A small Rust file so the index has symbols and the graph has edges.
fn write_rust_source(dir: &std::path::Path) {
    std::fs::create_dir_all(dir.join(".ragent")).expect("create .ragent");
    std::fs::write(
        dir.join("helper.rs"),
        "pub fn leaf() -> i32 {\n    7\n}\n\npub fn middle() -> i32 {\n    leaf() + 1\n}\n\n\
         pub fn root() -> i32 {\n    middle() + 1\n}\n",
    )
    .expect("write helper.rs");
}

/// Attach an in-memory index over `dir` with the semantic graph already built.
fn attach_built_graph_index(
    app: &mut App,
    dir: &std::path::Path,
) -> Arc<ragent_codeindex::CodeIndex> {
    let config = ragent_codeindex::types::CodeIndexConfig {
        enabled: true,
        project_root: dir.to_path_buf(),
        index_dir: dir.join(".ragent/codeindex"),
        scan_config: ragent_codeindex::types::ScanConfig::default(),
    };
    let idx = Arc::new(
        ragent_codeindex::CodeIndex::open_in_memory(&config).expect("in-memory code index"),
    );
    idx.index_file(&dir.join("helper.rs"))
        .expect("index source");
    idx.build_graph().expect("build semantic graph");
    app.code_index = Some(Arc::clone(&idx));
    app.code_index_enabled = true;
    idx
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn codeindex_rebuild_with_populated_index_reports_documents() {
    let (_g, _e, temp, mut app) = isolated_app();
    write_rust_source(temp.path());
    attach_built_graph_index(&mut app, temp.path());

    app.execute_slash_command("/codeindex rebuild").await;
    assert!(
        app.status.starts_with("codeindex: FTS rebuilt ("),
        "status: {}",
        app.status
    );
    assert!(last_message(&app).contains("FTS rebuild complete"));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn codeindex_skillgen_installs_then_updates() {
    let (_g, _e, _t, mut app) = isolated_app();

    app.execute_slash_command("/codeindex skillgen").await;
    assert!(
        app.status.starts_with("codeindex: skillgen ("),
        "status: {}",
        app.status
    );
    assert!(last_message(&app).contains("Installed graphify skill"));

    app.execute_slash_command("/codeindex skillgen").await;
    assert!(
        last_message(&app).contains("Updated graphify skill"),
        "text: {}",
        last_message(&app)
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn codeindex_explain_and_path_report_missing_symbols() {
    let (_g, _e, temp, mut app) = isolated_app();
    write_rust_source(temp.path());
    let idx = attach_built_graph_index(&mut app, temp.path());
    assert!(
        idx.graph_edge_count().unwrap_or(0) > 0,
        "the built graph must have edges"
    );

    app.execute_slash_command("/codeindex explain nosuchsymbol")
        .await;
    assert_eq!(app.status, "codeindex: explain (not found)");
    assert!(last_message(&app).contains("not found in the index"));

    app.execute_slash_command("/codeindex path root nosuchsymbol")
        .await;
    assert_eq!(app.status, "codeindex: path (no path)");
    assert!(last_message(&app).contains("No path found between"));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn codeindex_graph_lang_without_language_shows_usage() {
    let (_g, _e, _t, mut app) = isolated_app();
    app.execute_slash_command("/codeindex graph lang").await;
    assert_eq!(app.status, "codeindex: graph lang (usage)");
}

// ---------------------------------------------------------------------------
// /github - device flow without a tokio runtime
// ---------------------------------------------------------------------------

#[test]
fn github_login_without_runtime_reports_missing_runtime() {
    let (_g, _e, _t, mut app) = isolated_app();
    // `futures::executor::block_on` runs outside any tokio runtime, so
    // `Handle::try_current()` fails and the guard arm reports it. The test is
    // deliberately a plain `#[test]`: a `#[tokio::test]` would install a
    // runtime, and `block_in_place` (the login arm's nested-runtime bridge)
    // refuses to run on a current-thread one.
    futures::executor::block_on(app.execute_slash_command("/github login"));
    assert!(
        last_message(&app).contains("Async runtime not available"),
        "text: {}",
        last_message(&app)
    );
}

// ---------------------------------------------------------------------------
// /router - reload variants and the status label
// ---------------------------------------------------------------------------

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn router_reload_uses_the_global_config_when_project_config_is_absent() {
    let (_g, _e, _t, mut app) = isolated_app();
    std::fs::remove_file(
        std::env::current_dir()
            .expect("cwd")
            .join(".ragent/ragent.json"),
    )
    .expect("remove project config");
    write_global_config(
        r#"{ "provider": { "router": { "enabled": true, "tiers": {}, "boundaries": {} } } }"#,
    );

    app.execute_slash_command("/router reload").await;
    assert_eq!(app.status, "router: reloaded");
    let text = last_message(&app);
    assert!(text.contains("Router config reloaded from"), "text: {text}");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn router_reload_reports_provider_router_parse_error() {
    let (_g, _e, _t, mut app) = isolated_app();
    let project = std::env::current_dir()
        .expect("cwd")
        .join(".ragent/ragent.json");
    std::fs::write(
        &project,
        r#"{ "defaultAgent": "general", "provider": { "router": { "boundaries": "broken" } } }"#,
    )
    .expect("write project config");

    app.execute_slash_command("/router reload").await;
    assert_eq!(app.status, "router: reload parse error");
    assert!(last_message(&app).contains("Failed to parse provider.router"));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn router_reload_reports_whole_file_parse_error() {
    let (_g, _e, _t, mut app) = isolated_app();
    let project = std::env::current_dir()
        .expect("cwd")
        .join(".ragent/ragent.json");
    std::fs::write(&project, "{ not json").expect("write broken project config");

    app.execute_slash_command("/router reload").await;
    assert_eq!(app.status, "router: reload parse error");
    assert!(last_message(&app).contains("Failed to parse ragent.json"));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn router_status_reports_disabled_without_a_tier() {
    let (_g, _e, _t, mut app) = isolated_app();
    app.router_enabled = false;
    app.router_current_tier = None;

    app.execute_slash_command("/router status").await;
    assert_eq!(app.status, "router: status");
    let text = last_message(&app);
    assert!(text.contains("Router status: **disabled**"), "text: {text}");
    assert!(text.contains("Current tier: none"), "text: {text}");
}

// ---------------------------------------------------------------------------
// /webapi - bind failure inside the spawned server task
// ---------------------------------------------------------------------------

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn webapi_enable_with_an_invalid_address_logs_a_server_error() {
    let (_g, _e, _t, mut app) = isolated_app();
    app.webapi_addr = "not-a-valid-socket-address".to_string();

    app.execute_slash_command("/webapi enable").await;
    assert!(app.webapi_server.is_some());
    // Give the spawned server task time to fail its bind.
    tokio::time::sleep(std::time::Duration::from_millis(250)).await;

    app.execute_slash_command("/webapi disable").await;
    assert_eq!(last_message(&app), "[stop] **Web API disabled.**");
}

// ---------------------------------------------------------------------------
// /update - background check resolves
// ---------------------------------------------------------------------------

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn update_check_publishes_an_outcome() {
    let (_g, _e, _t, mut app) = isolated_app();
    let mut rx = app.event_bus.subscribe();

    app.execute_slash_command("/update").await;
    let mut outcome = None;
    for _ in 0..600 {
        if let Ok(ragent_agent::event::Event::AgentError { error, .. }) = rx.try_recv() {
            outcome = Some(error);
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(25)).await;
    }
    let outcome = outcome.expect("the update check must publish an outcome");
    let lowered = outcome.to_lowercase();
    assert!(
        lowered.contains("up to date") || lowered.contains("update available"),
        "outcome: {outcome}"
    );
}

// ---------------------------------------------------------------------------
// /codeindex - save failures and refused background work
// ---------------------------------------------------------------------------

#[cfg(unix)]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn codeindex_on_and_off_report_unsaved_when_the_config_is_read_only() {
    use std::os::unix::fs::PermissionsExt as _;

    let (_g, _e, temp, mut app) = isolated_app();
    app.code_index = None;
    let ragent_dir = temp.path().join(".ragent");
    let original = std::fs::metadata(&ragent_dir)
        .expect("stat .ragent")
        .permissions();

    std::fs::set_permissions(&ragent_dir, std::fs::Permissions::from_mode(0o500))
        .expect("chmod .ragent");
    app.execute_slash_command("/codeindex on").await;
    std::fs::set_permissions(&ragent_dir, original.clone()).expect("restore .ragent mode");

    assert_eq!(app.status, "codeindex: on (unsaved)");
    assert!(last_message(&app).contains("could not open index"));

    // Disable with an index attached so the `was_active` message is the one
    // that renders, then force the save to fail the same way.
    app.code_index_enabled = true;
    std::fs::set_permissions(&ragent_dir, std::fs::Permissions::from_mode(0o500))
        .expect("chmod .ragent");
    app.execute_slash_command("/codeindex off").await;
    std::fs::set_permissions(&ragent_dir, original).expect("restore .ragent mode");

    assert_eq!(app.status, "codeindex: off (unsaved)");
    assert!(last_message(&app).contains("saving config failed"));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn codeindex_graph_lang_reports_a_refused_spawn() {
    let (_g, _e, temp, mut app) = isolated_app();
    write_rust_source(temp.path());
    attach_built_graph_index(&mut app, temp.path());
    // A build is already running, so the spawn helper refuses.
    app.code_index_graph_spawned = true;

    app.execute_slash_command("/codeindex graph lang rust")
        .await;
    assert_eq!(app.status, "codeindex: graph lang failed");
    assert!(last_message(&app).contains("Graph lang build failed"));
}

#[cfg(unix)]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn codeindex_skillgen_reports_an_unwritable_skills_dir() {
    use std::os::unix::fs::PermissionsExt as _;

    let (_g, _e, _t, mut app) = isolated_app();
    let dir = global_config_dir();
    std::fs::create_dir_all(&dir).expect("create global config dir");
    let original = std::fs::metadata(&dir)
        .expect("stat global config dir")
        .permissions();
    std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o500))
        .expect("chmod global config dir");

    app.execute_slash_command("/codeindex skillgen").await;
    std::fs::set_permissions(&dir, original).expect("restore global config dir mode");

    assert_eq!(app.status, "codeindex: skillgen failed");
    assert!(last_message(&app).contains("Failed to generate graphify skill"));
}

// ---------------------------------------------------------------------------
// /mcp discover
// ---------------------------------------------------------------------------

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn mcp_discover_opens_the_discovery_state() {
    let (_g, _e, _t, mut app) = isolated_app();

    app.execute_slash_command("/mcp discover").await;
    let state = app.mcp_discover.as_ref().expect("discover state populated");
    assert_eq!(state.enabled.len(), state.servers.len());
    assert_eq!(state.selected, 0);
}

// ---------------------------------------------------------------------------
// /router reload - unreadable config
// ---------------------------------------------------------------------------

#[cfg(unix)]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn router_reload_reports_a_read_error() {
    use std::os::unix::fs::PermissionsExt as _;

    let (_g, _e, _t, mut app) = isolated_app();
    let project = std::env::current_dir()
        .expect("cwd")
        .join(".ragent/ragent.json");
    let original = std::fs::metadata(&project)
        .expect("stat config")
        .permissions();
    std::fs::set_permissions(&project, std::fs::Permissions::from_mode(0o000))
        .expect("chmod config");

    app.execute_slash_command("/router reload").await;
    std::fs::set_permissions(&project, original).expect("restore config mode");

    assert_eq!(app.status, "router: reload read error");
    assert!(last_message(&app).contains("Failed to read"));
}

// ---------------------------------------------------------------------------
// /doctor - database-sourced provider with a reachable health flag
// ---------------------------------------------------------------------------

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn doctor_reports_database_provider_and_healthy_state() {
    let (_g, _e, _t, mut app) = isolated_app();
    let health = Arc::clone(&app.provider_health);
    health.store(1, Ordering::Relaxed);
    app.configured_provider = Some(ConfiguredProvider {
        id: "ollama".to_string(),
        name: "Ollama (Local)".to_string(),
        source: ProviderSource::Database,
    });

    let report = run_doctor(&mut app).await;
    assert!(
        report.contains("provider: Ollama (Local) (ollama via database)"),
        "report: {report}"
    );
    assert!(report.contains("reachable"), "report: {report}");
    assert!(report.contains("no explicit model"), "report: {report}");
    assert!(report.contains("no AGENTS.md"), "report: {report}");
}
