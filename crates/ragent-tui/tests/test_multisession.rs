//! Multi-session core (spec MULTISESSION, milestone M3).
//!
//! Covers opening, switching and closing sessions, routing bus events to the
//! session that owns them, and the attention state a background session
//! reports. Every session lives in the in-memory storage `make_app` builds,
//! so nothing is written into the repository.

use ragent_agent::event::{Event, FinishReason};
use ragent_tui::App;
use ragent_tui::app::Attention;

#[path = "support/mod.rs"]
mod support;

/// An app with two real sessions: `(app, a, b)` where `b` is focused and
/// `a` is parked in the background.
async fn app_with_two_sessions() -> (App, String, String) {
    let mut app = support::make_app();
    // The slash gate creates session A; `/session new` then creates B.
    app.execute_slash_command("/session new").await;
    let b = app.session.session_id.clone().expect("focused session B");
    let a = app
        .background
        .iter()
        .next()
        .and_then(|v| v.session_id.clone())
        .expect("background session A");
    (app, a, b)
}

/// The background view with id `id`.
fn background<'a>(app: &'a App, id: &str) -> &'a ragent_tui::app::SessionView {
    app.background
        .iter()
        .find(|v| v.session_id.as_deref() == Some(id))
        .expect("session is in the background")
}

/// Concatenated text of every message in a view.
fn transcript(view: &ragent_tui::app::SessionView) -> String {
    view.messages
        .iter()
        .map(|m| m.text_content())
        .collect::<Vec<_>>()
        .join("\n")
}

#[tokio::test]
async fn test_multisession_new_session_parks_the_previous_one() {
    let (app, a, b) = app_with_two_sessions().await;
    assert_ne!(a, b, "the new session must have its own id");
    assert_eq!(app.background.len(), 1);
}

#[tokio::test]
async fn test_multisession_text_for_background_session_lands_in_its_transcript() {
    let (mut app, a, _b) = app_with_two_sessions().await;
    let focused_before = transcript(&app.session);

    app.handle_event(Event::TextDelta {
        session_id: a.clone(),
        text: "hello from A".to_string(),
    })
    .await;

    assert!(
        transcript(background(&app, &a)).contains("hello from A"),
        "A's text must land in A's transcript"
    );
    assert_eq!(
        transcript(&app.session),
        focused_before,
        "the focused session must not see A's text"
    );
}

#[tokio::test]
async fn test_multisession_background_permission_queues_without_dialog() {
    let (mut app, a, _b) = app_with_two_sessions().await;

    app.handle_event(Event::PermissionRequested {
        session_id: a.clone(),
        request_id: "p1".to_string(),
        permission: "file:write".to_string(),
        description: "write a file".to_string(),
        options: vec!["once".to_string()],
    })
    .await;

    assert_eq!(background(&app, &a).permission_queue.len(), 1);
    assert_eq!(background(&app, &a).attention(), Attention::Waiting);
    assert!(
        app.session.permission_queue.is_empty(),
        "no permission dialog may open over the focused session"
    );
    assert_eq!(app.background.waiting_count(), 1);
}

#[tokio::test]
async fn test_multisession_background_event_keeps_focused_status() {
    let (mut app, a, _b) = app_with_two_sessions().await;
    app.status = "focused status".to_string();

    app.handle_event(Event::TextDelta {
        session_id: a,
        text: "background chatter".to_string(),
    })
    .await;

    assert_eq!(app.status, "focused status");
}

#[tokio::test]
async fn test_multisession_finished_background_run_is_unread_until_focused() {
    let mut app = support::make_app();
    app.execute_slash_command("/session list").await;
    let a = app.session.session_id.clone().expect("session A");
    app.session.is_processing = true;
    app.execute_slash_command("/session new").await;

    app.handle_event(Event::MessageEnd {
        session_id: a.clone(),
        message_id: "m1".to_string(),
        reason: FinishReason::Stop,
    })
    .await;

    assert!(!background(&app, &a).is_processing, "A's run must end");
    assert_eq!(background(&app, &a).attention(), Attention::Unread);

    app.switch_to_session(&a).expect("switch to A");
    assert_eq!(app.session.session_id.as_deref(), Some(a.as_str()));
    assert_eq!(app.session.attention(), Attention::Idle);
}

#[tokio::test]
async fn test_multisession_switch_by_id_suffix_swaps_focus() {
    let (mut app, a, b) = app_with_two_sessions().await;
    let suffix = &a[a.len() - 8..];

    app.execute_slash_command(&format!("/session switch {suffix}"))
        .await;

    assert_eq!(app.session.session_id.as_deref(), Some(a.as_str()));
    assert_eq!(app.background.position(&b), Some(0));
    assert_eq!(app.background.len(), 1);
}

#[tokio::test]
async fn test_multisession_close_focuses_most_recent_session() {
    let (mut app, a, _b) = app_with_two_sessions().await;

    app.execute_slash_command("/session close").await;

    assert_eq!(app.session.session_id.as_deref(), Some(a.as_str()));
    assert!(app.background.is_empty());
}

#[tokio::test]
async fn test_multisession_close_refuses_while_running() {
    let (mut app, _a, b) = app_with_two_sessions().await;
    app.session.is_processing = true;

    assert!(app.close_focused_session().is_err());
    assert_eq!(app.session.session_id.as_deref(), Some(b.as_str()));
}

#[tokio::test]
async fn test_multisession_loading_stored_session_keeps_other_step_state() {
    let (mut app, a, b) = app_with_two_sessions().await;
    // Close B (it stays in storage) so focusing it again loads it from there.
    app.execute_slash_command("/session close").await;
    app.last_step_per_session.insert(a.clone(), 7);

    app.switch_to_session(&b).expect("load B from storage");

    assert_eq!(app.session.session_id.as_deref(), Some(b.as_str()));
    assert_eq!(
        app.last_step_per_session.get(&a),
        Some(&7),
        "loading B must not clear A's step bookkeeping"
    );
}

#[tokio::test]
async fn test_multisession_session_in_other_directory_moves_the_working_directory() {
    let (mut app, _a, b) = app_with_two_sessions().await;
    let home = app.cwd_path.clone();
    let elsewhere = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("src")
        .canonicalize()
        .expect("crate src");
    let other = app
        .session_processor
        .session_manager
        .create_session(elsewhere.clone())
        .expect("create session in another directory");

    app.switch_to_session(&other.id)
        .expect("a session in another directory opens");
    assert_eq!(
        app.cwd_path, elsewhere,
        "the TUI works in that session's directory"
    );
    assert_eq!(app.session.directory, elsewhere);

    app.switch_to_session(&b).expect("back to B");
    assert_eq!(app.cwd_path, home, "switching back restores B's directory");
}

#[tokio::test]
#[allow(clippy::await_holding_lock)] // current-thread runtime; the lock pins the cwd
async fn test_multisession_new_session_in_a_permitted_path() {
    // The permitted roots start from the process cwd, which other tests in
    // this binary swap under the same lock.
    let _cwd = crate::global_env_lock();
    let (mut app, _a, _b) = app_with_two_sessions().await;
    let sub = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("src")
        .canonicalize()
        .expect("crate src");

    app.execute_slash_command(&format!("/session new {}", sub.display()))
        .await;

    let id = app.session.session_id.clone().expect("new session");
    assert_eq!(app.cwd_path, sub);
    let row = app.storage.get_session(&id).expect("get").expect("row");
    assert_eq!(std::path::PathBuf::from(row.directory), sub);
    assert_eq!(app.background.len(), 2);
}

#[tokio::test]
async fn test_multisession_new_session_outside_permitted_roots_is_refused() {
    let (mut app, _a, b) = app_with_two_sessions().await;

    app.execute_slash_command("/session new /").await;
    assert!(app.status.contains("outside"), "status was: {}", app.status);

    app.execute_slash_command("/session new ./no-such-directory-here")
        .await;
    assert!(
        app.status.starts_with("session:"),
        "status was: {}",
        app.status
    );
    assert_eq!(app.session.session_id.as_deref(), Some(b.as_str()));
}

/// Drops every open view, as a restart would, keeping the same storage.
fn simulate_restart(app: &mut App) {
    let current = &app.session;
    app.session = ragent_tui::app::SessionView::new(
        current.agent_name.clone(),
        current.agent_info.clone(),
        current.current_agent_index,
    );
    app.background = ragent_tui::app::SessionRegistry::default();
}

#[tokio::test]
async fn test_multisession_restart_restores_open_sessions_in_order() {
    let (mut app, a, b) = app_with_two_sessions().await;
    simulate_restart(&mut app);

    assert_eq!(app.restore_open_sessions(), 2);

    assert_eq!(app.session.session_id.as_deref(), Some(b.as_str()));
    assert_eq!(app.background.position(&a), Some(0));
}

#[tokio::test]
async fn test_multisession_restart_skips_archived_sessions() {
    let (mut app, a, b) = app_with_two_sessions().await;
    app.storage.archive_session(&a).expect("archive A");
    simulate_restart(&mut app);

    assert_eq!(app.restore_open_sessions(), 1);
    assert_eq!(app.session.session_id.as_deref(), Some(b.as_str()));
    assert!(app.background.is_empty());
}

#[tokio::test]
async fn test_multisession_restore_without_saved_list_reopens_nothing() {
    let mut app = support::make_app();
    assert_eq!(app.restore_open_sessions(), 0);
}

#[tokio::test]
async fn test_multisession_rename_and_pin_focused_session() {
    let (mut app, _a, b) = app_with_two_sessions().await;

    app.execute_slash_command("/session rename Fix the parser")
        .await;
    app.execute_slash_command("/session pin").await;

    let row = app.storage.get_session(&b).expect("get").expect("row");
    assert_eq!(row.title, "Fix the parser");
    assert!(app.storage.pinned_session_ids().expect("ids").contains(&b));

    app.execute_slash_command("/session pin").await;
    assert!(!app.storage.pinned_session_ids().expect("ids").contains(&b));
}

#[tokio::test]
async fn test_multisession_archive_then_unarchive_by_suffix() {
    let (mut app, a, b) = app_with_two_sessions().await;

    app.execute_slash_command("/session archive").await;

    assert_eq!(app.session.session_id.as_deref(), Some(a.as_str()));
    assert!(
        app.storage
            .list_sessions()
            .expect("list")
            .iter()
            .all(|row| row.id != b),
        "an archived session is no longer listed"
    );

    let suffix = &b[b.len() - 8..];
    app.execute_slash_command(&format!("/session unarchive {suffix}"))
        .await;
    assert!(
        app.storage
            .list_sessions()
            .expect("list")
            .iter()
            .any(|row| row.id == b),
        "unarchive lists the session again"
    );
}

#[tokio::test]
async fn test_multisession_list_all_groups_by_project() {
    let (mut app, _a, _b) = app_with_two_sessions().await;
    let project = ragent_storage::project_root(&app.cwd_path.display().to_string());
    let name = ragent_storage::default_project_name(&project);
    let project = format!("{name} - `{project}`");

    app.execute_slash_command("/session list --all").await;

    let last = app
        .session
        .messages
        .last()
        .map(|m| m.text_content())
        .unwrap_or_default();
    assert!(
        last.contains(&format!("### {project}")),
        "expected a heading for {project}, got: {last}"
    );
}

#[test]
fn test_multisession_title_from_prompt() {
    use ragent_tui::app::title_from_prompt;
    assert_eq!(
        title_from_prompt("  Fix   the parser\nmore detail"),
        Some("Fix the parser".to_string())
    );
    assert_eq!(title_from_prompt("\n  \n"), None);
    let long = "word ".repeat(30);
    let title = title_from_prompt(&long).expect("title");
    assert!(title.ends_with("..."), "{title}");
    assert!(title.chars().count() <= 60, "{title}");
    assert!(!title.contains("wo..."), "cut at a word boundary: {title}");
    let one_word = "x".repeat(80);
    assert_eq!(
        title_from_prompt(&one_word).map(|t| t.chars().count()),
        Some(60)
    );
}

/// A user message for `session_id` with `text`.
fn user_message(session_id: &str, text: &str) -> ragent_agent::message::Message {
    ragent_agent::message::Message::new(
        session_id,
        ragent_agent::message::Role::User,
        vec![ragent_agent::message::MessagePart::Text {
            text: text.to_string(),
        }],
    )
}

fn reply_end(session_id: &str) -> Event {
    Event::MessageEnd {
        session_id: session_id.to_string(),
        message_id: "m1".to_string(),
        reason: FinishReason::Stop,
    }
}

#[tokio::test]
async fn test_multisession_first_reply_titles_focused_and_background_sessions() {
    let (mut app, a, b) = app_with_two_sessions().await;
    app.session
        .messages
        .push(user_message(&b, "Refactor the config loader"));
    app.background
        .view_mut_for_test(0)
        .messages
        .push(user_message(&a, "Add a sidebar"));

    app.handle_event(reply_end(&b)).await;
    app.handle_event(reply_end(&a)).await;

    let title = |id: &str| {
        app.storage
            .get_session(id)
            .expect("get")
            .expect("row")
            .title
    };
    assert_eq!(title(&b), "Refactor the config loader");
    assert_eq!(title(&a), "Add a sidebar");
}

#[tokio::test]
async fn test_multisession_auto_title_keeps_a_user_title() {
    let (mut app, _a, b) = app_with_two_sessions().await;
    app.execute_slash_command("/session rename Mine").await;
    app.session
        .messages
        .push(user_message(&b, "Something else"));

    app.handle_event(reply_end(&b)).await;

    let row = app.storage.get_session(&b).expect("get").expect("row");
    assert_eq!(row.title, "Mine");
}

#[tokio::test]
#[allow(clippy::await_holding_lock)] // current-thread runtime; the lock pins the env
async fn test_multisession_background_attention_fires_notification_hook() {
    // The injected config is only honoured while no `RAGENT_CONFIG*`
    // override is set, and other tests in this binary set those under the
    // same lock.
    let _env = crate::global_env_lock();
    let out = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../target/temp/multisession_notify");
    let _ = std::fs::remove_dir_all(&out); // INTENTIONAL: may not exist yet
    std::fs::create_dir_all(&out).expect("out dir");
    let log = out.join("hook.log");

    let (mut app, a, _b) = app_with_two_sessions().await;
    let mut config = ragent_config::Config::default();
    config.hooks = vec![serde_json::json!({
        "trigger": "on_notification",
        "command": format!(
            "printf '%s|%s|%s\\n' \"$RAGENT_NOTIFICATION_KIND\" \"$RAGENT_SESSION_ID\" \
             \"$RAGENT_NOTIFICATION_MESSAGE\" >> '{}'",
            log.display()
        ),
    })];
    *app.session_processor.cached_config.lock() =
        Some(ragent_agent::session::processor::CachedConfig {
            config: std::sync::Arc::new(config),
            file_mtimes: Vec::new(),
            env_overrides_present: false,
        });

    app.handle_event(Event::PermissionRequested {
        session_id: a.clone(),
        request_id: "p1".to_string(),
        permission: "file:write".to_string(),
        description: "write".to_string(),
        options: vec!["once".to_string()],
    })
    .await;

    let mut written = String::new();
    for _ in 0..100 {
        written = std::fs::read_to_string(&log).unwrap_or_default();
        if !written.is_empty() {
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
    }
    let short = &a[a.len() - 8..];
    assert_eq!(
        written,
        format!("waiting|{a}|session {short} needs you\n"),
        "hook output"
    );
}

#[tokio::test]
async fn test_multisession_scaffold_output_lands_in_the_session_that_started_it() {
    let (mut app, a, _b) = app_with_two_sessions().await;
    // `/new` was started in A, which is now in the background.
    app.newproj_origin_session = Some(a.clone());
    app.newproj_progress_slug = Some("Scaffolding `demo`...".to_string());
    app.newproj_progress
        .lock()
        .expect("progress")
        .push("[1/2] wrote Cargo.toml".to_string());
    *app.newproj_result.lock().expect("result") =
        Some(Ok(("Scaffolded demo".to_string(), String::new())));
    let focused_before = transcript(&app.session);

    app.poll_newproj_result();

    let in_a = transcript(background(&app, &a));
    assert!(
        in_a.contains("wrote Cargo.toml"),
        "progress panel in A: {in_a}"
    );
    assert!(in_a.contains("Scaffolded demo"), "summary in A: {in_a}");
    assert_eq!(transcript(&app.session), focused_before, "nothing in B");
    assert!(
        app.newproj_progress_text.is_none(),
        "panel text dropped once final"
    );

    // A later poll in B must not recreate the panel there.
    app.poll_newproj_result();
    assert_eq!(transcript(&app.session), focused_before);
}

#[tokio::test]
async fn test_multisession_bench_output_lands_in_the_session_that_started_it() {
    let (mut app, a, _b) = app_with_two_sessions().await;
    app.bench_origin_session = Some(a.clone());
    *app.bench_result.lock().expect("result") = Some(Err("runner crashed".to_string()));
    let focused_before = transcript(&app.session);

    app.poll_pending_bench();

    assert!(transcript(background(&app, &a)).contains("runner crashed"));
    assert_eq!(transcript(&app.session), focused_before);
    assert!(app.bench_origin_session.is_none());
}

#[tokio::test]
async fn test_multisession_new_session_prints_startup_block_but_children_do_not() {
    let (app, a, b) = app_with_two_sessions().await;

    let created = transcript(&app.session);
    assert!(
        created.contains(&format!("Version {}", env!("CARGO_PKG_VERSION"))),
        "the new session shows the banner: {created}"
    );
    assert!(created.contains("[ok] Session created:"));
    assert!(
        !transcript(background(&app, &a)).contains("[ok] Session created:"),
        "the earlier session's transcript is untouched"
    );

    // A sub-agent session is created by the agent, not the TUI: no banner.
    let child = app
        .session_processor
        .session_manager
        .create_child_session(
            app.cwd_path.clone(),
            &b,
            ragent_storage::SessionKind::Subagent,
        )
        .expect("child session");
    assert!(
        app.storage
            .get_messages(&child.id)
            .expect("messages")
            .is_empty(),
        "sub-agent sessions get no startup block"
    );
}
