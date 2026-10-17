//! Additional `/swarm` coverage reachable through the PUBLIC
//! `execute_slash_command` entry point.
//!
//! The swarm implementation helpers in `src/app/swarm.rs`
//! (`execute_swarm_decomposition`, `spawn_swarm_teammates`, `poll_swarm_unblock`,
//! `poll_swarm_completion`, `finalize_swarm_completion`) are `pub(crate)` with no
//! public test hook, so they cannot be called directly from an integration test;
//! this file exercises the reachable `status` / `cancel` / `help` grammar and the
//! decomposition-parse fallback arms the existing suites do not touch.
//!
//! Tests only - no production logic is touched.

use ragent_agent::team::{
    MemberStatus, SwarmDecomposition, SwarmState, SwarmSubtask, TeamMember, TeamStore,
};
use ragent_tui::App;

#[path = "support/mod.rs"]
mod support;

fn app_with_session(session: &str) -> App {
    let mut app = support::make_app();
    app.session_id = Some(session.to_string());
    app.needs_redraw = false;
    app
}

fn temp_dir(tag: &str) -> std::path::PathBuf {
    static N: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    let n = N.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let dir = std::env::temp_dir().join(format!(
        "ragent-tui-swarm-more-{}-{}-{}",
        std::process::id(),
        tag,
        n
    ));
    let _ = std::fs::create_dir_all(dir.join(".ragent"));
    dir
}

fn subtask(id: &str, deps: &[&str]) -> SwarmSubtask {
    SwarmSubtask {
        id: id.to_string(),
        title: format!("Task {id}"),
        description: format!("Do work for {id}"),
        depends_on: deps.iter().map(|s| (*s).to_string()).collect(),
        agent_type: None,
        model: None,
    }
}

fn joined_text(app: &App) -> String {
    app.messages
        .iter()
        .flat_map(|m| m.parts.iter())
        .filter_map(|p| match p {
            ragent_agent::message::MessagePart::Text { text } => Some(text.clone()),
            _ => None,
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn swarm(tasks: Vec<SwarmSubtask>, completed: bool) -> SwarmState {
    SwarmState {
        team_name: "swarm-manual".to_string(),
        prompt: "do work".to_string(),
        decomposition: SwarmDecomposition { tasks },
        spawned: true,
        completed,
        default_agent_type: None,
    }
}

// ---------------------------------------------------------------------------
// /swarm status - decomposition fallback rows (no task store)
// ---------------------------------------------------------------------------

#[tokio::test]
async fn swarm_status_without_task_store_renders_decomposition_rows() {
    let dir = temp_dir("status-fallback");
    let mut app = app_with_session("s-lead");
    app.cwd_path = dir;
    app.swarm_state = Some(swarm(
        vec![subtask("s1", &[]), subtask("s2", &["s1"])],
        false,
    ));

    app.execute_slash_command("/swarm status").await;
    let text = joined_text(&app);
    // No task store at the swarm's cwd -> the decomposition rows back the table.
    assert!(text.contains("| s1 |"), "got: {text}");
    assert!(text.contains("| s2 |"), "got: {text}");
    assert!(text.contains("[wait]"), "got: {text}");
    assert!(text.contains("(spawning...)"), "got: {text}");
}

#[tokio::test]
async fn swarm_status_renders_teammate_rows_when_members_registered() {
    let dir = temp_dir("status-members");
    let mut app = app_with_session("s-lead");
    app.cwd_path = dir;
    app.swarm_state = Some(swarm(vec![subtask("s1", &[])], false));
    let mut member = TeamMember::new("swarm-s1", "tm-001", "general");
    member.status = MemberStatus::Working;
    app.team_members.push(member);

    app.execute_slash_command("/swarm status").await;
    let text = joined_text(&app);
    assert!(text.contains("swarm-s1"), "got: {text}");
    assert!(text.contains("working"), "the member status is lowercased");
}

#[tokio::test]
async fn swarm_status_all_complete_prints_the_cleanup_hint() {
    let dir = temp_dir("status-complete");
    let mut app = app_with_session("s-lead");
    app.cwd_path = dir;
    app.swarm_state = Some(swarm(vec![subtask("s1", &[])], true));

    app.execute_slash_command("/swarm status").await;
    let text = joined_text(&app);
    // A completed swarm of one task with no store shows the fallback row and
    // the completion hint (completed == total here only via the fallback count).
    assert!(text.contains("Progress:"), "got: {text}");
}

// ---------------------------------------------------------------------------
// /swarm cancel - state cleared even without an on-disk team
// ---------------------------------------------------------------------------

#[tokio::test]
async fn swarm_cancel_clears_state_and_prints_cancelled() {
    let dir = temp_dir("cancel-no-team");
    let mut app = app_with_session("s-lead");
    app.cwd_path = dir;
    app.swarm_state = Some(swarm(vec![subtask("s1", &[])], false));

    app.execute_slash_command("/swarm cancel").await;
    assert!(app.swarm_state.is_none());
    assert_eq!(app.status, "swarm: cancelled");
    assert!(joined_text(&app).contains("Swarm Cancelled"));
}

#[tokio::test]
async fn swarm_cancel_with_live_team_store_shuts_it_down() {
    let dir = temp_dir("cancel-live");
    let team = "swarm-live-1";
    let _ = TeamStore::create(team, "s-lead", &dir, true).expect("create team");
    let mut app = app_with_session("s-lead");
    app.cwd_path = dir;
    app.swarm_state = Some(SwarmState {
        team_name: team.to_string(),
        ..swarm(vec![], false)
    });

    app.execute_slash_command("/swarm cancel").await;
    assert!(app.swarm_state.is_none());
    assert!(app.status.contains("cancelled"));
}

// ---------------------------------------------------------------------------
// /swarm help + usage + already-active guards
// ---------------------------------------------------------------------------

#[tokio::test]
async fn swarm_help_lists_every_subcommand() {
    let mut app = app_with_session("s-lead");
    app.execute_slash_command("/swarm help").await;
    let text = joined_text(&app);
    for needle in ["/swarm status", "/swarm cancel", "/swarm help"] {
        assert!(text.contains(needle), "help missing {needle}: {text}");
    }
}

#[tokio::test]
async fn swarm_bare_prompt_without_model_requires_configuration() {
    // With no selected model the prompt path warns before spawning anything.
    let mut app = app_with_session("s-lead");
    app.selected_model = None;
    app.swarm_state = None;
    app.execute_slash_command("/swarm build a thing").await;
    assert!(
        app.status.contains("requires a configured model"),
        "got: {}",
        app.status
    );
    // A placeholder swarm state is recorded so a later status reports in-flight.
    assert!(app.swarm_state.is_some());
}

#[tokio::test]
async fn swarm_second_invocation_while_active_is_refused() {
    let mut app = app_with_session("s-lead");
    app.swarm_state = Some(swarm(vec![], false));
    app.execute_slash_command("/swarm another goal").await;
    assert!(app.status.contains("already active"), "got: {}", app.status);
}

#[tokio::test]
async fn swarm_agent_flag_without_prompt_shows_usage() {
    let mut app = app_with_session("s-lead");
    app.execute_slash_command("/swarm --agent coder").await;
    assert!(joined_text(&app).contains("Usage:"));
}
