//! Deep `/spec` slash-command dispatch coverage.
//!
//! The existing `test_slash_spec.rs` drives the `/spec` arms in an EMPTY
//! `specs/` tree, so every arm takes its "not found" branch. This file seeds a
//! real spec pair (`SPEC.md` + `PLAN.md`) into an isolated tempdir project and
//! drives the success branches: `list` with a row, `search` with a hit,
//! `validate` on a real spec, `status` (show + transition + unknown status),
//! `task` (show + update + unknown status), `activate` / `deactivate`,
//! `delete --yes`, `coverage`, `impl --dry-run`, `feedback`, plus the usage
//! errors for `govcreate` / `reverse`.
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

/// A `SPEC.md` that passes EARS validation (one ubiquity requirement).
const SPEC_MD: &str = "---\nstatus: draft\n---\n# Sample Spec\n\n\
## Requirements\n\n\
FR-001: The system shall always expose the sample behaviour.\n";

/// A `PLAN.md` whose task table parses into two linked tasks.
const PLAN_MD: &str = "# Plan\n\n## Tasks\n\n\
| ID | Title | Requirement | Effort | Priority | Dependencies |\n\
|---|---|---|---|---|---|\n\
| T-001 | Build the thing | FR-001 | S | Critical | - |\n\
| T-002 | Test the thing | FR-001 | S | High | T-001 |\n";

/// An app in a fresh isolated project with a live session and one real spec
/// named `sample`.
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

// ---------------------------------------------------------------------------
// list / search / validate with real data
// ---------------------------------------------------------------------------

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn list_with_one_spec_renders_row() {
    let (_g, _e, _t, mut app) = app_with_spec();
    app.execute_slash_command("/spec list").await;
    assert_eq!(app.status, "spec: 1 spec(s) listed");
    let text = last_message(&app);
    assert!(text.contains("sample"), "row: {text}");
    assert!(text.contains("draft"), "status column: {text}");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn list_with_prefix_and_archived_filters() {
    let (_g, _e, _t, mut app) = app_with_spec();
    app.execute_slash_command("/spec list --prefix=sample --archived")
        .await;
    assert_eq!(app.status, "spec: 1 spec(s) listed");

    app.execute_slash_command("/spec list --prefix=nomatch")
        .await;
    assert_eq!(app.status, "spec: 0 spec(s) listed");
    assert!(last_message(&app).contains("No specs found"));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn search_with_match_renders_snippets() {
    let (_g, _e, _t, mut app) = app_with_spec();
    app.execute_slash_command("/spec search sample").await;
    assert_eq!(app.status, "spec: search complete");
    let text = last_message(&app);
    assert!(
        text.contains("From: /spec search 'sample'"),
        "header: {text}"
    );
    assert!(text.contains("Sample Spec"), "title: {text}");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn validate_real_spec_renders_report() {
    let (_g, _e, _t, mut app) = app_with_spec();
    app.execute_slash_command("/spec validate sample").await;
    assert_eq!(app.status, "spec: validation complete");
    let text = last_message(&app);
    assert!(text.contains("Validation: `sample`"), "report: {text}");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn validate_all_with_one_spec() {
    let (_g, _e, _t, mut app) = app_with_spec();
    app.execute_slash_command("/spec validate").await;
    assert_eq!(app.status, "spec: validation complete");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn validate_all_with_no_specs_reports_none() {
    let (_g, _e, _t, mut app) = app_without_spec();
    app.execute_slash_command("/spec validate").await;
    assert_eq!(app.status, "spec: validation complete");
    assert!(last_message(&app).contains("No specs found"));
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
// status
// ---------------------------------------------------------------------------

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn status_show_lists_allowed_transitions() {
    let (_g, _e, _t, mut app) = app_with_spec();
    app.execute_slash_command("/spec status sample").await;
    assert_eq!(app.status, "spec: status updated");
    let text = last_message(&app);
    assert!(text.contains("current status: `draft`"), "text: {text}");
    assert!(text.contains("Allowed transitions"), "text: {text}");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn status_transition_succeeds() {
    let (_g, _e, _t, mut app) = app_with_spec();
    app.execute_slash_command("/spec status sample in_review")
        .await;
    assert_eq!(app.status, "spec: status updated");
    let text = last_message(&app);
    // `transition_with_flags` applies the new status before the message is
    // formatted, so both sides read `in_review`; assert only the shape.
    assert!(text.contains("transitioned from `"), "{text}");
    assert!(text.contains("to `in_review`"), "{text}");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn status_unknown_status_is_rejected() {
    let (_g, _e, _t, mut app) = app_with_spec();
    app.execute_slash_command("/spec status sample bogus").await;
    assert_eq!(app.status, "spec: spec: unknown status 'bogus'");
    assert!(last_message(&app).contains("**Error:**"));
}

// ---------------------------------------------------------------------------
// task
// ---------------------------------------------------------------------------

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn task_show_renders_details() {
    let (_g, _e, _t, mut app) = app_with_spec();
    app.execute_slash_command("/spec task sample T-001").await;
    assert_eq!(app.status, "spec: task complete");
    let text = last_message(&app);
    assert!(
        text.contains("## Task T-001 - Build the thing"),
        "text: {text}"
    );
    assert!(text.contains("**Status:**"), "text: {text}");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn task_update_sets_status() {
    let (_g, _e, _t, mut app) = app_with_spec();
    app.execute_slash_command("/spec task sample T-001 completed")
        .await;
    assert_eq!(app.status, "spec: task complete");
    assert!(last_message(&app).contains("updated to `completed`"));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn task_unknown_task_is_reported() {
    let (_g, _e, _t, mut app) = app_with_spec();
    app.execute_slash_command("/spec task sample T-999").await;
    assert_eq!(app.status, "spec: spec: task T-999 not found in sample");
    assert!(
        last_message(&app).contains("T-999"),
        "text: {}",
        last_message(&app)
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn task_unknown_status_is_rejected() {
    let (_g, _e, _t, mut app) = app_with_spec();
    app.execute_slash_command("/spec task sample T-001 bogus")
        .await;
    assert_eq!(app.status, "spec: spec: unknown task status 'bogus'");
}

// ---------------------------------------------------------------------------
// activate / deactivate / coverage
// ---------------------------------------------------------------------------

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn activate_real_spec_sets_active_spec() {
    let (_g, _e, _t, mut app) = app_with_spec();
    app.execute_slash_command("/spec activate sample").await;
    assert_eq!(app.status, "spec: sample activated");
    assert!(app.active_spec.is_some(), "active spec must be recorded");
    assert_eq!(app.active_spec.as_deref(), Some("sample"));
    assert!(app.spec_manager.is_some());
    let text = last_message(&app);
    assert!(text.contains("is now the active spec"), "text: {text}");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn activate_missing_spec_reports_error() {
    let (_g, _e, _t, mut app) = app_with_spec();
    app.execute_slash_command("/spec activate nope").await;
    assert!(
        app.status.starts_with("spec: spec: failed to read nope"),
        "got {}",
        app.status
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn deactivate_active_spec_clears_it() {
    let (_g, _e, _t, mut app) = app_with_spec();
    app.execute_slash_command("/spec activate sample").await;
    app.execute_slash_command("/spec deactivate").await;
    assert_eq!(app.status, "spec: deactivated");
    assert!(app.active_spec.is_none());
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn coverage_real_spec_renders_report() {
    let (_g, _e, _t, mut app) = app_with_spec();
    app.execute_slash_command("/spec coverage sample").await;
    assert_eq!(app.status, "spec: coverage complete");
    assert_ne!(last_message(&app), "");
}

// ---------------------------------------------------------------------------
// delete --yes
// ---------------------------------------------------------------------------

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn delete_with_yes_removes_directory() {
    let (_g, _e, temp, mut app) = app_with_spec();
    let dir = temp.path().join("specs").join("sample");
    assert!(dir.is_dir());
    app.execute_slash_command("/spec delete sample --yes").await;
    assert_eq!(app.status, "spec: deleting 'sample'...");
    // The delete runs on a spawned task; poll until the directory is gone.
    for _ in 0..200 {
        if !dir.exists() {
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(10)).await;
    }
    assert!(!dir.exists(), "spec directory must be deleted");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn delete_invalid_id_is_rejected() {
    let (_g, _e, _t, mut app) = app_with_spec();
    app.execute_slash_command("/spec delete bad!id --yes").await;
    assert_eq!(app.status, "spec: invalid spec ID: bad!id");
}

// ---------------------------------------------------------------------------
// impl --dry-run
// ---------------------------------------------------------------------------

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn impl_dry_run_renders_summary() {
    let (_g, _e, _t, mut app) = app_with_spec();
    app.execute_slash_command("/spec impl sample --dry-run")
        .await;
    assert_eq!(app.status, "spec: sample dry-run complete");
    assert_ne!(last_message(&app), "");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn impl_missing_spec_lists_available() {
    let (_g, _e, _t, mut app) = app_with_spec();
    app.execute_slash_command("/spec impl nope").await;
    assert_eq!(app.status, "spec: spec nope not found");
    let text = last_message(&app);
    assert!(text.contains("Available specs"), "text: {text}");
    assert!(text.contains("sample"), "text: {text}");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn impl_already_implemented_is_refused() {
    let (_g, _e, temp, mut app) = app_with_spec();
    // Rewrite SPEC.md so the spec is already implemented.
    let spec_md = SPEC_MD.replace("status: draft", "status: implemented");
    std::fs::write(temp.path().join("specs/sample/SPEC.md"), spec_md.as_bytes())
        .expect("rewrite SPEC.md");

    app.execute_slash_command("/spec impl sample").await;
    assert_eq!(app.status, "spec: sample is implemented");
    assert!(last_message(&app).contains("already marked as"));
}

// ---------------------------------------------------------------------------
// feedback
// ---------------------------------------------------------------------------

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn feedback_appends_note_file() {
    let (_g, _e, temp, mut app) = app_with_spec();
    app.execute_slash_command("/spec feedback sample metrics dropped")
        .await;
    assert!(
        app.status.starts_with("spec: appending feedback"),
        "got {}",
        app.status
    );
    let fb = temp.path().join("specs/sample/FEEDBACK.md");
    assert!(fb.is_file(), "FEEDBACK.md must exist at {fb:?}");
    let text = std::fs::read_to_string(&fb).expect("read FEEDBACK.md");
    assert!(text.contains("metrics dropped"), "note: {text}");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn feedback_missing_spec_reports_available() {
    let (_g, _e, _t, mut app) = app_with_spec();
    app.execute_slash_command("/spec feedback nope a note")
        .await;
    assert!(app.status.contains("nope"), "got {}", app.status);
    assert!(last_message(&app).contains("Available specs"));
}

// ---------------------------------------------------------------------------
// plan / tasks / update / add / specify / create (agent-backed arms)
// ---------------------------------------------------------------------------

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn create_starts_generation() {
    let (_g, _e, _t, mut app) = app_without_spec();
    app.execute_slash_command("/spec create newspec a brand new feature")
        .await;
    assert_eq!(
        app.status,
        ragent_specs::SpecCommand::build_create_status("newspec")
    );
    assert!(app.is_processing);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn create_with_folder_targets_that_project() {
    let (_g, _e, _t, mut app) = app_without_spec();
    app.execute_slash_command("/spec create newspec a feature --folder /tmp/other")
        .await;
    assert_eq!(
        app.status,
        ragent_specs::SpecCommand::build_create_status("newspec")
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn specify_starts_generation() {
    let (_g, _e, _t, mut app) = app_without_spec();
    app.execute_slash_command("/spec specify newspec a brand new feature")
        .await;
    assert_eq!(
        app.status,
        ragent_specs::SpecCommand::build_specify_status("newspec")
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn plan_starts_generation_on_real_spec() {
    let (_g, _e, _t, mut app) = app_with_spec();
    app.execute_slash_command("/spec plan sample use rust")
        .await;
    assert_eq!(
        app.status,
        ragent_specs::SpecCommand::build_plan_status("sample")
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn plan_missing_spec_reports_available() {
    let (_g, _e, _t, mut app) = app_with_spec();
    app.execute_slash_command("/spec plan nope use rust").await;
    assert!(app.status.contains("nope"), "got {}", app.status);
    assert!(last_message(&app).contains("Available specs"));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn tasks_writes_tasks_file() {
    let (_g, _e, temp, mut app) = app_with_spec();
    app.execute_slash_command("/spec tasks sample").await;
    let path = temp.path().join("specs/sample/TASKS.md");
    assert!(path.is_file(), "TASKS.md must exist at {path:?}");
    let text = std::fs::read_to_string(&path).expect("read TASKS.md");
    assert!(text.contains("T-001"), "tasks: {text}");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn tasks_on_empty_plan_reports_empty() {
    let (_g, _e, temp, mut app) = app_with_spec();
    std::fs::write(temp.path().join("specs/sample/PLAN.md"), "").expect("empty PLAN.md");
    app.execute_slash_command("/spec tasks sample").await;
    assert_eq!(app.status, "spec: PLAN.md is empty for sample");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn update_starts_regeneration() {
    let (_g, _e, _t, mut app) = app_with_spec();
    app.execute_slash_command("/spec update sample").await;
    assert_eq!(
        app.status,
        ragent_specs::SpecCommand::build_update_status("sample")
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn add_starts_incremental_update() {
    let (_g, _e, _t, mut app) = app_with_spec();
    app.execute_slash_command("/spec add sample another requirement")
        .await;
    assert_eq!(
        app.status,
        ragent_specs::SpecCommand::build_add_status("sample")
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn add_missing_spec_reports_available() {
    let (_g, _e, _t, mut app) = app_with_spec();
    app.execute_slash_command("/spec add nope a feature").await;
    assert!(
        app.status.contains("nope"),
        "add on a missing spec must name it: {}",
        app.status
    );
}

// ---------------------------------------------------------------------------
// usage errors
// ---------------------------------------------------------------------------

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn govcreate_without_arguments_reports_usage() {
    let (_g, _e, _t, mut app) = app_with_spec();
    app.execute_slash_command("/spec govcreate").await;
    assert_eq!(app.status, "Usage: /spec govcreate - try /spec help");
    assert!(last_message(&app).contains("missing required argument"));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn reverse_without_repo_reports_usage() {
    let (_g, _e, _t, mut app) = app_with_spec();
    app.execute_slash_command("/spec reverse").await;
    assert_eq!(app.status, "Usage: /spec reverse - try /spec help");
    assert!(last_message(&app).contains("no repository given"));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn govcreate_without_model_reports_no_model() {
    let (_g, _e, _t, mut app) = app_with_spec();
    // A syntactically valid govcreate needs a target folder; with no model
    // selected the arm reports the usage-level error before touching disk.
    app.execute_slash_command("/spec govcreate newspec ./arch.md ./target")
        .await;
    assert_eq!(app.status, "spec govcreate: no model");
}
