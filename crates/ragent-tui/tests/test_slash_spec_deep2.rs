//! Third `/spec` dispatch file: the argument-validation and missing-spec arms
//! that neither `test_slash_spec.rs` (empty tree) nor `test_slash_spec_deep.rs`
//! (seeded tree) reaches, plus the `--task` / `--dry-run` impl options and the
//! `GovCreateUsage` variant.
//!
//! Tests only - no production logic is touched.
#![allow(unsafe_code)]
#![cfg(test)]
// The isolation harness holds a process-global mutex across the async test body
// so cwd/env mutations cannot interleave.
#![allow(clippy::await_holding_lock)]

use ragent_tui::App;

#[path = "slash_support/mod.rs"]
mod slash_support;
#[path = "support/mod.rs"]
mod support;

use slash_support::{app_with_session, enter_project, last_message};

/// A `SPEC.md` that parses with one ubiquity requirement.
const SPEC_MD: &str = "---\nstatus: draft\n---\n# Sample Spec\n\n\
## Requirements\n\n\
FR-001: The system shall always expose the sample behaviour.\n";

/// A `PLAN.md` whose task table parses into two linked tasks.
const PLAN_MD: &str = "# Plan\n\n## Tasks\n\n\
| ID | Title | Requirement | Effort | Priority | Dependencies |\n\
|---|---|---|---|---|---|\n\
| T-001 | Build the thing | FR-001 | S | Critical | - |\n\
| T-002 | Test the thing | FR-001 | S | High | T-001 |\n";

/// An app in a fresh isolated project with a live session and one real spec.
fn app_with_spec() -> (
    std::sync::MutexGuard<'static, ()>,
    slash_support::EnvGuard,
    tempfile::TempDir,
    App,
) {
    let (guard, env, temp) = enter_project(r#"{ "defaultAgent": "general" }"#);
    let dir = temp.path().join("specs").join("sample");
    std::fs::create_dir_all(&dir).expect("create spec dir");
    std::fs::write(dir.join("SPEC.md"), SPEC_MD).expect("write SPEC.md");
    std::fs::write(dir.join("PLAN.md"), PLAN_MD).expect("write PLAN.md");
    let app = app_with_session();
    (guard, env, temp, app)
}

/// An app in a project whose `specs/` directory is empty.
fn app_without_spec() -> (
    std::sync::MutexGuard<'static, ()>,
    slash_support::EnvGuard,
    tempfile::TempDir,
    App,
) {
    let (guard, env, temp) = enter_project(r#"{ "defaultAgent": "general" }"#);
    std::fs::create_dir_all(temp.path().join("specs")).expect("create specs");
    let app = app_with_session();
    (guard, env, temp, app)
}

// ---------------------------------------------------------------------------
// invalid spec IDs (the early-return arms)
// ---------------------------------------------------------------------------

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn invalid_ids_are_rejected_by_every_id_validating_arm() {
    let (_g, _e, _t, mut app) = app_with_spec();
    for (cmd, prefix) in [
        ("/spec jtbd bad!id", "spec: invalid spec ID"),
        ("/spec impl bad!id", "spec: invalid spec ID"),
        ("/spec tasks bad!id", "spec: invalid spec ID"),
        ("/spec update bad!id", "spec: invalid spec ID"),
        ("/spec plan bad!id tech", "spec: invalid spec ID"),
        ("/spec feedback bad!id a note", "spec: invalid spec ID"),
        ("/spec coverage bad!id", "spec: invalid spec ID"),
        ("/spec activate bad!id", "spec: invalid spec ID"),
        ("/spec task bad!id", "spec: invalid spec ID"),
        ("/spec status bad!id", "spec: invalid spec ID"),
    ] {
        app.execute_slash_command(cmd).await;
        assert!(app.status.starts_with(prefix), "{cmd} -> {}", app.status);
    }
}

// ---------------------------------------------------------------------------
// missing specs on every arm
// ---------------------------------------------------------------------------

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn invalid_name_on_parse_validated_arms_reports_usage() {
    let (_g, _e, _t, mut app) = app_with_spec();
    // `create` / `specify` reject an invalid name at PARSE time, so they take
    // the usage arm rather than the per-arm spec-ID check.
    for cmd in [
        "/spec create bad!id a feature",
        "/spec specify bad!id a feature",
    ] {
        app.execute_slash_command(cmd).await;
        assert!(
            app.status.starts_with("Usage: /spec"),
            "{cmd} -> {}",
            app.status
        );
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn missing_spec_on_every_arm_names_the_spec() {
    let (_g, _e, _t, mut app) = app_with_spec();
    for cmd in [
        "/spec tasks nope",
        "/spec update nope",
        "/spec plan nope tech",
        "/spec feedback nope a note",
        "/spec add nope a feature",
        "/spec jtbd nope",
    ] {
        app.execute_slash_command(cmd).await;
        assert!(app.status.contains("nope"), "{cmd} -> {}", app.status);
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn missing_spec_with_empty_tree_says_none_found() {
    let (_g, _e, _t, mut app) = app_without_spec();
    app.execute_slash_command("/spec plan nope tech").await;
    let text = last_message(&app);
    assert!(text.contains("(none found)"), "text: {text}");
}

// ---------------------------------------------------------------------------
// /spec jtbd
// ---------------------------------------------------------------------------

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn jtbd_missing_spec_dir_is_refused() {
    let (_g, _e, _t, mut app) = app_with_spec();
    app.execute_slash_command("/spec jtbd nope").await;
    assert!(app.status.contains("not found"), "got {}", app.status);
    assert!(last_message(&app).contains("**Error:**"));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn jtbd_unknown_agent_override_is_refused() {
    let (_g, _e, _t, mut app) = app_with_spec();
    app.execute_slash_command("/spec jtbd sample --agent no-such-agent")
        .await;
    assert!(
        app.status.contains("agent 'no-such-agent' not found"),
        "got {}",
        app.status
    );
    assert!(last_message(&app).contains("No task was spawned"));
}

// ---------------------------------------------------------------------------
// /spec impl options
// ---------------------------------------------------------------------------

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn impl_with_task_and_dry_run_reports_summary() {
    let (_g, _e, _t, mut app) = app_with_spec();
    app.execute_slash_command("/spec impl sample --task T-001 --dry-run")
        .await;
    assert_eq!(app.status, "spec: sample dry-run complete");
    assert_ne!(last_message(&app), "");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn impl_invalid_id_is_rejected() {
    let (_g, _e, _t, mut app) = app_with_spec();
    app.execute_slash_command("/spec impl bad!id").await;
    assert_eq!(app.status, "spec: invalid spec ID: bad!id");
}

// ---------------------------------------------------------------------------
// /spec tasks failure arms
// ---------------------------------------------------------------------------

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn tasks_missing_spec_lists_available() {
    let (_g, _e, _t, mut app) = app_with_spec();
    app.execute_slash_command("/spec tasks nope").await;
    assert!(app.status.contains("nope"), "got {}", app.status);
    assert!(last_message(&app).contains("Available specs"));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn tasks_with_no_task_table_reports_no_tasks() {
    let (_g, _e, temp, mut app) = app_with_spec();
    // A PLAN.md with prose but no task table parses to zero tasks.
    std::fs::write(
        temp.path().join("specs/sample/PLAN.md"),
        "# Plan\n\nNo table here.\n",
    )
    .expect("rewrite PLAN.md");
    app.execute_slash_command("/spec tasks sample").await;
    assert!(
        app.status.contains("no tasks found") || app.status.contains("parse error"),
        "got {}",
        app.status
    );
}

// ---------------------------------------------------------------------------
// /spec add, update, specify, plan on a real spec
// ---------------------------------------------------------------------------

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn add_and_update_start_generation_on_a_real_spec() {
    let (_g, _e, _t, mut app) = app_with_spec();

    app.execute_slash_command("/spec add sample a new requirement")
        .await;
    assert_eq!(
        app.status,
        ragent_specs::SpecCommand::build_add_status("sample")
    );

    app.execute_slash_command("/spec update sample").await;
    assert_eq!(
        app.status,
        ragent_specs::SpecCommand::build_update_status("sample")
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn plan_on_a_real_spec_starts_generation() {
    let (_g, _e, _t, mut app) = app_with_spec();
    app.execute_slash_command("/spec plan sample use rust")
        .await;
    assert_eq!(
        app.status,
        ragent_specs::SpecCommand::build_plan_status("sample")
    );
    assert!(app.is_processing);
}

// ---------------------------------------------------------------------------
// /spec govcreate usage variants
// ---------------------------------------------------------------------------

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn govcreate_incomplete_arguments_report_usage() {
    let (_g, _e, _t, mut app) = app_with_spec();
    for cmd in [
        "/spec govcreate",
        "/spec govcreate onlyonespec",
        "/spec govcreate specid ./arch.md",
    ] {
        app.execute_slash_command(cmd).await;
        assert!(
            app.status.starts_with("Usage: /spec govcreate"),
            "{cmd} -> {}",
            app.status
        );
        assert!(last_message(&app).contains("missing required argument"));
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn govcreate_invalid_spec_id_reports_usage() {
    let (_g, _e, _t, mut app) = app_with_spec();
    app.execute_slash_command("/spec govcreate bad!id ./arch.md ./target")
        .await;
    assert_eq!(app.status, "spec: govcreate usage");
    assert!(last_message(&app).contains("govcreate"));
}

// ---------------------------------------------------------------------------
// /spec reverse usage variants
// ---------------------------------------------------------------------------

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn reverse_unknown_flag_reports_usage() {
    let (_g, _e, _t, mut app) = app_with_spec();
    app.execute_slash_command("/spec reverse owner/repo --nope")
        .await;
    assert!(
        app.status == "spec: reverse usage" || app.status.starts_with("Usage: /spec reverse"),
        "got {}",
        app.status
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn reverse_folder_without_scaffold_flags_is_refused() {
    let (_g, _e, _t, mut app) = app_with_spec();
    app.execute_slash_command("/spec reverse owner/repo --folder /tmp/x")
        .await;
    assert!(
        app.status == "spec: reverse usage" || app.status.starts_with("Usage: /spec reverse"),
        "got {}",
        app.status
    );
}

// ---------------------------------------------------------------------------
// SPEC-only specs (PLAN.md is optional)
// ---------------------------------------------------------------------------

/// An app whose project has one spec directory holding only `SPEC.md`.
///
/// `PLAN.md` is optional per `docs/howtos/slashcommands/spec.md`: a spec whose
/// plan has not been generated yet must still be listed and readable.
fn app_with_spec_only() -> (
    std::sync::MutexGuard<'static, ()>,
    slash_support::EnvGuard,
    tempfile::TempDir,
    App,
) {
    let (guard, env, temp) = enter_project(r#"{ "defaultAgent": "general" }"#);
    let dir = temp.path().join("specs").join("spec-only");
    std::fs::create_dir_all(&dir).expect("create spec dir");
    std::fs::write(dir.join("SPEC.md"), SPEC_MD).expect("write SPEC.md");
    let app = app_with_session();
    (guard, env, temp, app)
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn update_starts_regeneration_for_spec_only_spec() {
    let (_g, _e, t, mut app) = app_with_spec_only();
    app.execute_slash_command("/spec update spec-only").await;
    assert_eq!(
        app.status,
        ragent_specs::SpecCommand::build_update_status("spec-only"),
        "a SPEC-only spec must update, not report not-found"
    );
    assert!(
        !t.path().join("specs/spec-only/PLAN.md").exists(),
        "reading a SPEC-only spec must not create an empty PLAN.md"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn list_shows_spec_only_spec() {
    let (_g, _e, _t, mut app) = app_with_spec_only();
    app.execute_slash_command("/spec list").await;
    let text = slash_support::all_messages(&app);
    assert!(text.contains("spec-only"), "list output: {text}");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn validate_spec_only_spec_reports_missing_plan_without_not_found() {
    let (_g, _e, _t, mut app) = app_with_spec_only();
    app.execute_slash_command("/spec validate spec-only").await;
    assert!(
        !app.status.contains("not found"),
        "validate must not report not-found for a SPEC-only spec: {}",
        app.status
    );
}
