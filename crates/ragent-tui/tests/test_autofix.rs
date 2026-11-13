//! `/autofix` pull/merge request binding, the browser opener and Auto-fix
//! trigger routing (spec SESSIONMENU M4-M5, T4.3-T4.4, T5.2-T5.3).
//!
//! No network: binding rows are written straight to storage, the one
//! command that would look a request up is run against a project without a
//! remote, which is refused before any lookup, and triggers are fed to the
//! app as events (the watcher itself is not started).

use std::path::{Path, PathBuf};

use ragent_agent::event::Event;
use ragent_agent::project::{ProjectRemote, RemoteKind};
use ragent_storage::SessionRequest;
use ragent_tui::App;
use ragent_tui::app::autofix::parse_request_ref;
use ragent_tui::open_in::{LaunchEnv, url_spec};

#[path = "support/mod.rs"]
mod support;

fn remote(kind: RemoteKind, repo_path: &str) -> ProjectRemote {
    ProjectRemote {
        kind,
        host: "example.invalid".to_string(),
        path: repo_path.to_string(),
        url: String::new(),
        remote_name: "origin".to_string(),
        default_branch: None,
    }
}

/// An app with one open session; returns its id.
async fn app_with_session() -> (App, String) {
    let mut app = support::make_app();
    app.execute_slash_command("/session list").await;
    let id = app.session.session_id.clone().expect("session");
    (app, id)
}

fn binding(session_id: &str) -> SessionRequest {
    SessionRequest {
        session_id: session_id.to_string(),
        forge: "gitlab".to_string(),
        host: "gitlab.example.com".to_string(),
        repo: "group/project".to_string(),
        number: 5,
        title: "Fix the thing".to_string(),
        web_url: "https://gitlab.example.com/group/project/-/merge_requests/5".to_string(),
        head_branch: "fix".to_string(),
        state: "open".to_string(),
    }
}

fn last_message(app: &App) -> String {
    app.session
        .messages
        .last()
        .map(|m| m.text_content())
        .unwrap_or_default()
}

/// A fresh git checkout without a remote, unique to `name`. It must be a
/// checkout of its own: a plain directory under `target/temp` belongs to
/// this repository's work tree and would resolve to its real remote.
fn checkout_without_remote(name: &str) -> PathBuf {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../target/temp/autofix")
        .join(name);
    let _ = std::fs::remove_dir_all(&dir); // INTENTIONAL: may not exist yet
    std::fs::create_dir_all(&dir).expect("dir");
    let status = std::process::Command::new("git")
        .args(["init", "--quiet"])
        .current_dir(&dir)
        .env("GIT_CONFIG_GLOBAL", "/dev/null")
        .env("GIT_CONFIG_SYSTEM", "/dev/null")
        .status()
        .expect("git init");
    assert!(status.success(), "git init");
    dir.canonicalize().expect("canonical dir")
}

#[test]
fn test_autofix_parse_request_ref_numbers_and_urls() {
    let github = remote(RemoteKind::GitHub, "acme/app");
    let gitlab = remote(RemoteKind::GitLab, "group/sub/project");
    assert_eq!(parse_request_ref("12", &github), Ok(12));
    assert_eq!(parse_request_ref("#12", &github), Ok(12));
    assert_eq!(parse_request_ref("!7", &gitlab), Ok(7));
    assert_eq!(
        parse_request_ref("https://github.com/acme/app/pull/34/files", &github),
        Ok(34)
    );
    assert_eq!(
        parse_request_ref(
            "https://gitlab.example.com/group/sub/project/-/merge_requests/8#note_1",
            &gitlab
        ),
        Ok(8)
    );
    assert!(parse_request_ref("0", &github).is_err());
    assert!(parse_request_ref("abc", &github).is_err());
    let other_repo = parse_request_ref("https://github.com/other/app/pull/1", &github);
    assert!(
        other_repo.is_err_and(|e| e.contains("acme/app")),
        "a URL of another repository is refused"
    );
    assert!(
        parse_request_ref("https://github.com/acme/app/issues/3", &github).is_err(),
        "an issue URL is not a pull request"
    );
}

#[test]
fn test_autofix_url_spec_opens_only_web_urls() {
    let env = |os: &str| LaunchEnv {
        os: os.to_string(),
        vars: std::collections::HashMap::new(),
        path: Vec::new(),
    };
    let url = "https://github.com/acme/app/pull/1";
    for (os, program) in [
        ("linux", "xdg-open"),
        ("macos", "open"),
        ("windows", "explorer"),
    ] {
        let spec = url_spec(url, &env(os)).expect(os);
        assert_eq!(spec.program, program, "{os}");
        assert_eq!(spec.args, [url], "{os}");
    }
    assert!(url_spec("file:///etc/passwd", &env("linux")).is_none());
    assert!(url_spec("--help", &env("linux")).is_none());
}

#[tokio::test]
async fn test_autofix_status_shows_binding_and_unbind_clears_it() {
    let (mut app, id) = app_with_session().await;
    app.storage
        .set_session_request(&binding(&id))
        .expect("bind");

    app.execute_slash_command("/autofix status").await;
    let status = last_message(&app);
    assert!(status.contains("MR !5"), "{status}");
    assert!(status.contains("Fix the thing"), "{status}");
    assert!(status.contains("merge_requests/5"), "{status}");

    app.execute_slash_command("/autofix unbind").await;
    assert!(last_message(&app).contains("Unbound from MR !5"));
    assert!(app.storage.session_request(&id).expect("read").is_none());

    app.execute_slash_command("/autofix unbind").await;
    assert!(last_message(&app).contains("not bound"));
}

#[tokio::test]
async fn test_autofix_bind_without_remote_and_unknown_subcommand() {
    let (mut app, _id) = app_with_session().await;
    app.session.directory = checkout_without_remote("no_remote");

    app.execute_slash_command("/autofix bind 3").await;
    assert!(
        app.status.contains("no GitHub or GitLab remote"),
        "{}",
        app.status
    );

    app.execute_slash_command("/autofix frobnicate").await;
    assert!(app.status.contains("unknown subcommand"), "{}", app.status);

    app.execute_slash_command("/autofix help").await;
    assert!(last_message(&app).contains("/autofix bind"));
}

/// An app with two sessions, `a` in the background and `b` focused.
async fn app_with_two_sessions() -> (App, String, String) {
    let (mut app, a) = app_with_session().await;
    std::thread::sleep(std::time::Duration::from_millis(5));
    app.execute_slash_command("/session new").await;
    let b = app.session.session_id.clone().expect("session B");
    (app, a, b)
}

fn github_binding(session_id: &str) -> SessionRequest {
    SessionRequest {
        session_id: session_id.to_string(),
        forge: "github".to_string(),
        host: "github.com".to_string(),
        repo: "acme/app".to_string(),
        number: 12,
        title: "Add menu".to_string(),
        web_url: "https://github.com/acme/app/pull/12".to_string(),
        head_branch: "menu".to_string(),
        state: "open".to_string(),
    }
}

fn trigger(session_id: &str, kind: &str, summary: &str, prompt: &str) -> Event {
    Event::AutofixTrigger {
        session_id: session_id.to_string(),
        kind: kind.to_string(),
        summary: summary.to_string(),
        prompt: prompt.to_string(),
    }
}

#[tokio::test]
async fn test_autofix_on_off_and_status() {
    let (mut app, id) = app_with_session().await;
    app.execute_slash_command("/autofix on").await;
    assert!(app.status.contains("not bound"), "{}", app.status);

    app.storage
        .set_session_request(&github_binding(&id))
        .expect("bind");
    app.execute_slash_command("/autofix on").await;
    assert!(last_message(&app).contains("Auto-fix on for PR #12"));
    assert!(
        app.storage
            .session_autofix(&id)
            .expect("read")
            .expect("bound")
            .enabled
    );

    app.execute_slash_command("/autofix status").await;
    let status = last_message(&app);
    assert!(status.contains("Auto-fix: on"), "{status}");

    app.execute_slash_command("/autofix off").await;
    assert!(last_message(&app).contains("Auto-fix off for PR #12"));
    assert!(
        !app.storage
            .session_autofix(&id)
            .expect("read")
            .expect("bound")
            .enabled
    );
}

#[tokio::test]
async fn test_autofix_trigger_queues_for_busy_background_session() {
    let (mut app, a, b) = app_with_two_sessions().await;
    let index = app.background.position(&a).expect("A in background");
    app.background.view_mut_for_test(index).is_processing = true;
    app.status = "typing".to_string();

    let prompt = "Auto-fix: CI failed on PR #12. Fix it, commit and push.";
    app.handle_event(trigger(
        &a,
        "ci",
        "PR #12 auto-fix: CI failed (attempt 1/3)",
        prompt,
    ))
    .await;

    assert_eq!(
        app.session.session_id.as_deref(),
        Some(b.as_str()),
        "B stays focused"
    );
    assert_eq!(app.status, "typing", "the focused status line is untouched");
    assert!(app.session.permission_queue.is_empty(), "no dialog over B");
    let view = app
        .background
        .iter()
        .find(|v| v.session_id.as_deref() == Some(a.as_str()))
        .expect("A still open");
    assert_eq!(
        view.input_queue.front().map(|q| q.text.as_str()),
        Some(prompt),
        "the busy session gets the prompt queued"
    );
    assert!(view.unread, "A is marked for attention");
    assert!(
        app.storage
            .unread_session_ids()
            .expect("unread")
            .contains(&a)
    );
    assert_eq!(
        app.autofix_status.get(&a).map(String::as_str),
        Some("PR #12 auto-fix: CI failed (attempt 1/3)")
    );
}

#[tokio::test]
async fn test_autofix_trigger_opens_closed_session_and_reports_merge() {
    let (mut app, a, b) = app_with_two_sessions().await;
    app.switch_to_session(&a).expect("focus A");
    app.execute_slash_command("/session close").await;
    app.switch_to_session(&b).expect("focus B");
    assert!(app.background.position(&a).is_none(), "A is closed");

    app.handle_event(trigger(&a, "closed", "PR #12 merged; auto-fix stopped", ""))
        .await;

    assert_eq!(app.session.session_id.as_deref(), Some(b.as_str()));
    let view = app
        .background
        .iter()
        .find(|v| v.session_id.as_deref() == Some(a.as_str()))
        .expect("A opened in the background");
    let notice = view
        .messages
        .last()
        .map(|m| m.text_content())
        .unwrap_or_default();
    assert!(notice.contains("Auto-fix: PR #12 merged"), "{notice}");
    assert!(view.input_queue.is_empty(), "a notice sends nothing");
}

#[tokio::test]
async fn test_autofix_trigger_for_focused_busy_session_sets_status() {
    let (mut app, id) = app_with_session().await;
    app.session.is_processing = true;
    app.handle_event(trigger(
        &id,
        "review",
        "PR #12 auto-fix: new review comments (attempt 1/3)",
        "Address the comments.",
    ))
    .await;
    assert_eq!(app.session.input_queue.len(), 1);
    assert_eq!(
        app.status,
        "PR #12 auto-fix: new review comments (attempt 1/3)"
    );
}
