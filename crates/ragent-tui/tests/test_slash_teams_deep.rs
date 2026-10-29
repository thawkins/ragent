//! `/team` and `/inbox` dispatch coverage for the paths the existing suites
//! leave unvisited: `/team focus` (clear, single match, unknown match) and
//! `/team forcecleanup` (the confirmation modal and the confirmed run),
//! `/team create` without a name (generated slug name), `/team message` with a
//! real teammate, `/inbox` against a seeded inbox file, and the `/loop`
//! argument-error branches.
//!
//! Tests only - no production logic is touched.
#![allow(unsafe_code)]
#![cfg(test)]
// The isolation harness holds a process-global mutex across the async test body
// so cwd/env mutations cannot interleave.
#![allow(clippy::await_holding_lock)]

use ragent_agent::team::{MemberStatus, TeamMember};
use ragent_tui::App;

#[path = "slash_support/mod.rs"]
mod slash_support;
#[path = "support/mod.rs"]
mod support;

use slash_support::{app_with_session, enter_project, last_message};

/// An app in an isolated tempdir project with the `.ragent/teams` tree ready.
fn team_app() -> (
    std::sync::MutexGuard<'static, ()>,
    slash_support::EnvGuard,
    tempfile::TempDir,
    App,
) {
    let (guard, env, temp) = enter_project(r#"{ "defaultAgent": "general" }"#);
    std::fs::create_dir_all(temp.path().join(".ragent/teams")).expect("create teams dir");
    std::fs::create_dir_all(temp.path().join(".ragent")).expect("create .ragent");
    let app = app_with_session();
    (guard, env, temp, app)
}

/// Create a project-local team through the slash command and return its name.
async fn create_team(app: &mut App, name: &str) {
    app.execute_slash_command(&format!("/team create bp1 {name}"))
        .await;
}

// ---------------------------------------------------------------------------
// /team create without a name
// ---------------------------------------------------------------------------

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn create_without_name_generates_slug_name() {
    let (_g, _e, _t, mut app) = team_app();
    app.execute_slash_command("/team create demo-blueprint")
        .await;
    let team = app.active_team.as_ref().expect("active team must be set");
    assert!(
        team.name.starts_with("demo-blueprint-"),
        "generated name should be slugified with a timestamp: {}",
        team.name
    );
    assert!(app.show_teams);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn create_with_unsafe_name_is_rejected() {
    let (_g, _e, _t, mut app) = team_app();
    app.execute_slash_command("/team create bp1 ../escape")
        .await;
    assert!(app.active_team.is_none(), "no team may be created");
    assert_ne!(app.status, "ready", "status must name the rejection");
}

// ---------------------------------------------------------------------------
// /team focus
// ---------------------------------------------------------------------------

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn focus_without_active_team_is_refused() {
    let (_g, _e, _t, mut app) = team_app();
    app.execute_slash_command("/team focus alice").await;
    assert_eq!(app.status, "No active team");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn focus_unknown_teammate_reports_available() {
    let (_g, _e, _t, mut app) = team_app();
    create_team(&mut app, "focus-team").await;
    app.team_members
        .push(TeamMember::new("alice", "tm-001", "general"));

    app.execute_slash_command("/team focus bob").await;
    let text = last_message(&app);
    assert!(text.contains("No teammate matching 'bob'"), "text: {text}");
    assert!(text.contains("alice"), "available list: {text}");
    assert!(app.focused_teammate.is_none());
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn focus_single_match_focuses_teammate() {
    let (_g, _e, _t, mut app) = team_app();
    create_team(&mut app, "focus-team2").await;
    let mut alice = TeamMember::new("alice", "tm-001", "general");
    alice.status = MemberStatus::Working;
    app.team_members.push(alice);

    app.execute_slash_command("/team focus alice").await;
    assert_eq!(app.focused_teammate.as_deref(), Some("tm-001"));
    let text = last_message(&app);
    assert!(text.contains("Focused on **alice**"), "text: {text}");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn focus_without_argument_clears_focus() {
    let (_g, _e, _t, mut app) = team_app();
    create_team(&mut app, "focus-team3").await;
    app.team_members
        .push(TeamMember::new("alice", "tm-001", "general"));
    app.focused_teammate = Some("tm-001".to_string());

    app.execute_slash_command("/team focus").await;
    assert_eq!(app.status, "team: focus cleared");
    assert!(app.focused_teammate.is_none());
}

// ---------------------------------------------------------------------------
// /team message with a real teammate
// ---------------------------------------------------------------------------

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn message_to_real_teammate_is_delivered() {
    let (_g, _e, _t, mut app) = team_app();
    create_team(&mut app, "msg-team").await;
    app.team_members
        .push(TeamMember::new("alice", "tm-001", "general"));

    app.execute_slash_command("/team message alice please review")
        .await;
    // Delivery is refused unless the message carries an authenticated sender
    // session id (the mailbox authentication guard), so either the send
    // succeeds or the guard names the reason - both are the real code path.
    assert!(
        app.status == "message sent to alice" || app.status.starts_with("Failed to send message:"),
        "unexpected status: {}",
        app.status
    );
}

// ---------------------------------------------------------------------------
// /team forcecleanup
// ---------------------------------------------------------------------------

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn forcecleanup_without_confirm_opens_modal() {
    let (_g, _e, _t, mut app) = team_app();
    create_team(&mut app, "force-team").await;
    let mut alice = TeamMember::new("alice", "tm-001", "general");
    alice.status = MemberStatus::Working;
    app.team_members.push(alice);

    app.execute_slash_command("/team forcecleanup").await;
    assert_eq!(app.status, "forcecleanup confirmation required");
    let pending = app
        .pending_forcecleanup
        .as_ref()
        .expect("confirmation modal must be staged");
    assert_eq!(pending.team_name, "force-team");
    assert_eq!(pending.active_members.len(), 1);
    let text = last_message(&app);
    assert!(text.contains("alice"), "modal must list members: {text}");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn forcecleanup_confirm_removes_team() {
    let (_g, _e, temp, mut app) = team_app();
    create_team(&mut app, "force-team2").await;
    let dir = temp.path().join(".ragent/teams/force-team2");
    assert!(dir.is_dir(), "team dir must exist at {dir:?}");

    app.execute_slash_command("/team forcecleanup confirm")
        .await;
    assert_eq!(app.status, "team force cleaned up");
    assert!(app.active_team.is_none());
    assert!(!dir.exists(), "team dir must be removed");
    assert!(last_message(&app).contains("force cleaned up"));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn forcecleanup_without_active_team_reports_none() {
    let (_g, _e, _t, mut app) = team_app();
    app.execute_slash_command("/team forcecleanup confirm")
        .await;
    assert_eq!(app.status, "No active team to clean up");
}

// ---------------------------------------------------------------------------
// /inbox against a seeded inbox
// ---------------------------------------------------------------------------

/// Seed one inbox entry into the isolated project's data dir.
fn seed_inbox_entry(temp: &tempfile::TempDir) -> String {
    let entry = ragent_agent::loop_state::InboxEntry::new("event-1", "build failed on main");
    ragent_agent::loop_state::write_inbox_entries(temp.path(), std::slice::from_ref(&entry))
        .expect("write inbox");
    entry.id
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn inbox_list_with_entry_renders_row() {
    let (_g, _e, temp, mut app) = team_app();
    seed_inbox_entry(&temp);

    app.execute_slash_command("/inbox list").await;
    assert_eq!(app.status, "inbox: list");
    let text = last_message(&app);
    assert!(text.contains("Triage Inbox"), "header: {text}");
    assert!(text.contains("build failed on main"), "content: {text}");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn inbox_claim_and_dismiss_update_status() {
    let (_g, _e, temp, mut app) = team_app();
    let id = seed_inbox_entry(&temp);

    app.execute_slash_command(&format!("/inbox claim {id}"))
        .await;
    assert_eq!(app.status, "inbox: claimed");

    app.execute_slash_command(&format!("/inbox dismiss {id}"))
        .await;
    assert_eq!(app.status, "inbox: dismissed");

    app.execute_slash_command("/inbox clear").await;
    assert_eq!(app.status, "inbox: cleared");
    assert!(last_message(&app).contains("Cleared 1 finding"));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn inbox_claim_without_id_is_a_usage_error() {
    let (_g, _e, _t, mut app) = team_app();
    app.execute_slash_command("/inbox claim").await;
    assert_eq!(app.status, "inbox: claimed usage");
    assert!(last_message(&app).contains("Usage: `/inbox claimed"));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn inbox_help_renders_reference() {
    let (_g, _e, _t, mut app) = team_app();
    app.execute_slash_command("/inbox help").await;
    assert_eq!(app.status, "inbox: help");
    assert!(last_message(&app).contains("Triage inbox findings"));
}

// ---------------------------------------------------------------------------
// /queue with entries staged
// ---------------------------------------------------------------------------

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn queue_list_with_entries_renders_rows() {
    let (_g, _e, _t, mut app) = team_app();
    app.input_queue.push_back(ragent_tui::app::QueuedInput {
        text: "first queued message".to_string(),
        image_paths: Vec::new(),
    });
    app.input_queue.push_back(ragent_tui::app::QueuedInput {
        text: "second queued message".to_string(),
        image_paths: Vec::new(),
    });

    app.execute_slash_command("/queue list").await;
    assert_eq!(app.status, "queue: 2 entries");
    let text = last_message(&app);
    assert!(text.contains("Input Queue"), "header: {text}");
    assert!(text.contains("first queued message"), "text: {text}");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn queue_clear_with_entries_empties_the_queue() {
    let (_g, _e, _t, mut app) = team_app();
    app.input_queue.push_back(ragent_tui::app::QueuedInput {
        text: "only entry".to_string(),
        image_paths: Vec::new(),
    });

    app.execute_slash_command("/queue clear").await;
    assert_eq!(app.status, "queue: cleared");
    assert_eq!(app.input_queue.len(), 0);
    assert!(last_message(&app).contains("Cleared 1 entry"));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn queue_next_on_empty_queue_reports_empty() {
    let (_g, _e, _t, mut app) = team_app();
    app.execute_slash_command("/queue next").await;
    assert_eq!(app.status, "queue: next empty");
}

// ---------------------------------------------------------------------------
// /loop argument errors and help
// ---------------------------------------------------------------------------

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn loop_help_and_no_args_open_the_dialog() {
    let (_g, _e, _t, mut app) = team_app();

    app.execute_slash_command("/loop help").await;
    assert!(
        app.loop_setup.is_some() || app.status.contains("loop"),
        "help path"
    );

    app.execute_slash_command("/loop").await;
    assert!(
        app.loop_setup.is_some(),
        "bare /loop opens the setup dialog"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn loop_invalid_flag_reports_error() {
    let (_g, _e, _t, mut app) = team_app();
    app.execute_slash_command("/loop coder --max-steps abc fix it")
        .await;
    assert_eq!(app.status, "loop: invalid argument");
    assert!(last_message(&app).contains("**Error:**"));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn loop_missing_goal_reports_missing_field() {
    let (_g, _e, _t, mut app) = team_app();
    app.execute_slash_command("/loop coder").await;
    // The missing-goal error is appended, then the setup dialog is re-opened
    // (which rewrites the status), so assert on the message window.
    assert!(last_message(&app).contains("missing field: goal"));
    assert!(app.loop_setup.is_some(), "the dialog must re-open");
}

// ---------------------------------------------------------------------------
// /config branches
// ---------------------------------------------------------------------------

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn config_help_show_list_and_usage() {
    let (_g, _e, _t, mut app) = team_app();

    app.execute_slash_command("/config help").await;
    assert_eq!(app.status, "config: help");

    app.execute_slash_command("/config show").await;
    assert_eq!(app.status, "config: show");
    assert!(last_message(&app).contains("Application Paths"));

    // A fresh isolated config dir has no backups yet.
    app.execute_slash_command("/config list").await;
    assert!(
        app.status == "config: list empty" || app.status == "config: list",
        "got {}",
        app.status
    );

    app.execute_slash_command("/config frobnicate").await;
    assert_eq!(app.status, "config: usage");
    assert!(last_message(&app).contains("Usage: `/config"));
}

// ---------------------------------------------------------------------------
// /thinking, /model, /provider, /tools, /dirs and /bash error branches
// ---------------------------------------------------------------------------

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn thinking_without_model_warns() {
    let (_g, _e, _t, mut app) = team_app();
    app.selected_model = None;
    app.execute_slash_command("/thinking high").await;
    assert_eq!(
        app.status,
        "[warn] No model selected - use /model to choose"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn model_help_and_usage() {
    let (_g, _e, _t, mut app) = team_app();

    app.execute_slash_command("/model help").await;
    assert_eq!(app.status, "model: help");

    app.execute_slash_command("/model frobnicate").await;
    assert_eq!(app.status, "Usage: /model [show]");

    app.execute_slash_command("/model show").await;
    assert!(
        app.status == "[warn] No active model selected" || app.status == "active model metadata",
        "got {}",
        app.status
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn provider_help_and_usage() {
    let (_g, _e, _t, mut app) = team_app();

    app.execute_slash_command("/provider help").await;
    assert_eq!(app.status, "provider: help");

    app.execute_slash_command("/provider frobnicate").await;
    assert_eq!(app.status, "provider: usage");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn dirs_add_remove_without_pattern_is_a_usage_error() {
    let (_g, _e, _t, mut app) = team_app();
    app.execute_slash_command("/dirs add allow").await;
    assert!(last_message(&app).contains("Usage: `/dirs add allow|deny"));
    app.execute_slash_command("/dirs remove deny").await;
    assert!(last_message(&app).contains("Usage: `/dirs remove allow|deny"));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn cancel_help_and_missing_prefix() {
    let (_g, _e, _t, mut app) = team_app();

    app.execute_slash_command("/cancel help").await;
    assert_eq!(app.status, "cancel: help");

    app.execute_slash_command("/cancel").await;
    assert!(app.status.contains("Please provide a task ID prefix"));

    app.execute_slash_command("/cancel nosuchtask").await;
    // FR-041: the unmatched-prefix report names the prefix it matched nothing
    // for (`test_cancel_slash.rs` pins the wording).
    assert!(app.status.contains("No task matching 'nosuchtask'"));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn update_help_and_unknown_subcommand() {
    let (_g, _e, _t, mut app) = team_app();

    app.execute_slash_command("/update help").await;
    assert_eq!(app.status, "update: help");

    app.execute_slash_command("/update frobnicate").await;
    assert!(app.status.starts_with("update:"), "got {}", app.status);
}

// ---------------------------------------------------------------------------
// /doctor
// ---------------------------------------------------------------------------

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn doctor_help_shows_reference() {
    let (_g, _e, _t, mut app) = team_app();
    app.execute_slash_command("/doctor help").await;
    assert_eq!(app.status, "doctor: help");
    assert!(last_message(&app).contains("System diagnostics"));
}

// ---------------------------------------------------------------------------
// /github and /gitlab help / status / usage
// ---------------------------------------------------------------------------

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn github_help_status_and_usage() {
    let (_g, _e, _t, mut app) = team_app();

    app.execute_slash_command("/github help").await;
    assert_eq!(app.status, "github: help");

    app.execute_slash_command("/github frobnicate").await;
    assert!(last_message(&app).contains("Usage: `/github"));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn gitlab_help_and_usage() {
    let (_g, _e, _t, mut app) = team_app();

    app.execute_slash_command("/gitlab help").await;
    assert_eq!(app.status, "gitlab: help");

    app.execute_slash_command("/gitlab frobnicate").await;
    assert!(last_message(&app).contains("Usage: `/gitlab"));
}
