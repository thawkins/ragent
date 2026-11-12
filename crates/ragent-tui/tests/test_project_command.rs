//! `/project` and `/session new --project/--branch` (spec `projects`, T-008).
//!
//! Git fixtures live under the git-ignored `target/temp/` tree, one directory
//! per test; sessions live in the in-memory storage `make_app` builds.

use std::path::{Path, PathBuf};
use std::process::Command;

use ragent_agent::project::Project;
use ragent_tui::App;

#[path = "support/mod.rs"]
mod support;

fn fixture_dir(name: &str) -> PathBuf {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../target/temp/project_command")
        .join(name);
    let _ = std::fs::remove_dir_all(&dir); // INTENTIONAL: may not exist yet
    std::fs::create_dir_all(&dir).expect("fixture dir");
    dir.canonicalize().expect("canonical")
}

fn git(dir: &Path, args: &[&str]) -> String {
    let output = Command::new("git")
        .args(args)
        .current_dir(dir)
        .env("GIT_CONFIG_GLOBAL", "/dev/null")
        .env("GIT_CONFIG_SYSTEM", "/dev/null")
        .env("GIT_AUTHOR_NAME", "Test")
        .env("GIT_AUTHOR_EMAIL", "test@example.invalid")
        .env("GIT_COMMITTER_NAME", "Test")
        .env("GIT_COMMITTER_EMAIL", "test@example.invalid")
        .output()
        .expect("run git");
    assert!(
        output.status.success(),
        "git {args:?} failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8_lossy(&output.stdout).trim().to_string()
}

/// An app, a git repository registered as project `name`, and a worktree
/// root, all under one fixture directory.
fn app_with_project(name: &str) -> (App, Project, PathBuf) {
    let base = fixture_dir(name);
    let repo = base.join(name);
    std::fs::create_dir_all(&repo).expect("repo dir");
    git(&repo, &["init", "--quiet", "--initial-branch=main"]);
    std::fs::write(repo.join("README.md"), "fixture\n").expect("write");
    git(&repo, &["add", "README.md"]);
    git(&repo, &["commit", "--quiet", "-m", "initial"]);
    let mut app = support::make_app();
    app.worktree_root = Some(base.join("trees"));
    let project = app
        .project_manager_for_test()
        .resolve_for_dir(&repo)
        .expect("project");
    (app, project, base)
}

fn last_message(app: &App) -> String {
    app.session
        .messages
        .last()
        .map(|m| m.text_content())
        .unwrap_or_default()
}

#[tokio::test]
async fn test_project_command_show_and_list() {
    let (mut app, project, _base) = app_with_project("show_list");

    app.execute_slash_command("/project").await;
    let focused = last_message(&app);
    let launch = ragent_storage::project_root(&app.cwd_path.display().to_string());
    assert!(focused.contains("## Project"), "{focused}");
    assert!(
        focused.contains(&launch),
        "shows the focused project: {focused}"
    );

    app.execute_slash_command("/project show show_list").await;
    let named = last_message(&app);
    assert!(named.contains("## Project show_list"), "{named}");
    assert!(named.contains(&project.worktree.display().to_string()));
    assert!(named.contains("none (local project)"), "{named}");

    app.execute_slash_command("/project list").await;
    let list = last_message(&app);
    assert!(list.contains("show_list"), "{list}");
    assert!(
        list.contains("Worktree") && list.contains("Base model"),
        "{list}"
    );

    app.execute_slash_command("/project show no-such-project")
        .await;
    assert!(
        app.status.contains("no project named"),
        "status: {}",
        app.status
    );
    app.execute_slash_command("/project frobnicate").await;
    assert!(
        app.status.contains("unknown subcommand"),
        "status: {}",
        app.status
    );
}

#[tokio::test]
async fn test_project_command_help_lists_every_subcommand() {
    let mut app = support::make_app();
    app.execute_slash_command("/project help").await;
    let help = last_message(&app);
    for sub in [
        "list", "add", "rename", "model", "refresh", "prune", "remove",
    ] {
        assert!(
            help.contains(&format!("/project {sub}")),
            "missing {sub}: {help}"
        );
    }
}

#[tokio::test]
async fn test_project_command_rename_and_model_edit_the_focused_project() {
    let mut app = support::make_app();
    app.execute_slash_command("/project rename Storefront")
        .await;
    let project = app.focused_project_for_test().expect("project");
    assert_eq!(project.name, "Storefront");

    app.execute_slash_command("/project model anthropic/claude-test")
        .await;
    assert_eq!(
        app.focused_project_for_test()
            .expect("project")
            .base_model_label()
            .as_deref(),
        Some("anthropic/claude-test")
    );

    app.session.selected_model = Some("openai/gpt-test".to_string());
    app.execute_slash_command("/project model current").await;
    assert_eq!(
        app.focused_project_for_test()
            .expect("project")
            .base_model_label()
            .as_deref(),
        Some("openai/gpt-test")
    );

    app.execute_slash_command("/project model not-a-model")
        .await;
    assert!(
        app.status.contains("<provider>/<model>"),
        "status: {}",
        app.status
    );
    assert!(
        app.focused_project_for_test()
            .expect("project")
            .base_model
            .is_some(),
        "a rejected value leaves the base model alone"
    );

    app.execute_slash_command("/project model clear").await;
    assert_eq!(
        app.focused_project_for_test().expect("project").base_model,
        None
    );
    app.execute_slash_command("/project rename").await;
    assert!(app.status.contains("usage"), "status: {}", app.status);
}

#[tokio::test]
async fn test_project_command_add_registers_permitted_directories_only() {
    let mut app = support::make_app();
    let src = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");

    app.execute_slash_command(&format!("/project add {}", src.display()))
        .await;
    assert!(
        last_message(&app).contains("registered"),
        "{}",
        last_message(&app)
    );

    app.execute_slash_command("/project add /").await;
    assert!(app.status.contains("outside"), "status: {}", app.status);

    app.execute_slash_command("/project add https://example.com/acme/app.git")
        .await;
    assert!(
        app.status.contains("not a GitHub or GitLab"),
        "status: {}",
        app.status
    );
    app.execute_slash_command("/project add").await;
    assert!(app.status.contains("usage"), "status: {}", app.status);
}

#[tokio::test]
async fn test_project_command_remove_needs_no_live_sessions() {
    let (mut app, project, _base) = app_with_project("remove_me");
    let manager = app.project_manager_for_test();

    // The focused (launch) project has a live session.
    let launch = app.focused_project_for_test().expect("launch project");
    app.execute_slash_command(&format!("/project remove {}", launch.id))
        .await;
    assert!(
        app.status.contains("live session"),
        "status: {}",
        app.status
    );
    assert!(manager.find(&launch.id).expect("find").is_some());

    app.execute_slash_command("/project remove remove_me").await;
    assert!(last_message(&app).contains("removed from the list"));
    assert!(manager.find(&project.id).expect("find").is_none());
    assert!(
        project.worktree.join("README.md").is_file(),
        "files are untouched"
    );
}

#[tokio::test]
async fn test_project_command_session_new_in_a_named_project() {
    let (mut app, project, base) = app_with_project("named");
    app.execute_slash_command("/session list").await;
    let first = app.session.session_id.clone().expect("first session");

    app.execute_slash_command("/session new --project named")
        .await;

    let id = app.session.session_id.clone().expect("new session");
    assert_ne!(id, first);
    assert_eq!(app.background.len(), 1, "the previous session is parked");
    let row = app.storage.get_session(&id).expect("get").expect("row");
    assert_eq!(row.project_id, project.id);
    assert_eq!(PathBuf::from(row.directory), project.worktree);
    assert_eq!(app.cwd_path, project.worktree);
    assert!(!base.join("trees").exists(), "no worktree without --branch");
}

#[tokio::test]
async fn test_project_command_session_new_with_a_branch_gets_its_own_worktree() {
    let (mut app, project, base) = app_with_project("branched");

    app.execute_slash_command("/session new --project branched --branch feature/login")
        .await;

    let id = app.session.session_id.clone().expect("new session");
    let checkout = app
        .storage
        .session_checkout(&id)
        .expect("get")
        .expect("checkout");
    assert_eq!(checkout.branch.as_deref(), Some("feature/login"));
    let worktree = PathBuf::from(checkout.worktree.expect("the session owns a worktree"));
    assert!(
        worktree.starts_with(base.join("trees")),
        "{}",
        worktree.display()
    );
    assert_eq!(
        git(&worktree, &["rev-parse", "--abbrev-ref", "HEAD"]),
        "feature/login"
    );
    assert_eq!(
        app.cwd_path, worktree,
        "the TUI works in the session's worktree"
    );
    assert_eq!(app.session.directory, worktree);
    assert_eq!(
        app.storage
            .get_session(&id)
            .expect("get")
            .expect("row")
            .project_id,
        project.id,
        "the session groups under the owning project"
    );
    assert_eq!(
        app.focused_project_for_test().expect("project").id,
        project.id,
        "the worktree resolves to the owning project, not a new one"
    );

    // A second session on the same branch is refused and nothing changes.
    app.execute_slash_command("/session new --project branched --branch feature/login")
        .await;
    assert!(
        app.status.contains("already exists"),
        "status: {}",
        app.status
    );
    assert_eq!(app.session.session_id.as_deref(), Some(id.as_str()));
}

#[tokio::test]
async fn test_project_command_session_new_rejects_bad_options() {
    let (mut app, _project, _base) = app_with_project("bad_options");
    app.execute_slash_command("/session list").await;
    let before = app.session.session_id.clone();

    for (command, expected) in [
        ("/session new --branch", "needs a value"),
        ("/session new --project", "needs a value"),
        ("/session new --project --branch x", "needs a value"),
        ("/session new --bogus", "unknown option"),
        ("/session new --project nope", "no project named"),
        ("/session new src --project bad_options", "not both"),
        (
            "/session new --project bad_options --branch bad..name",
            "not a valid branch",
        ),
    ] {
        app.execute_slash_command(command).await;
        assert!(
            app.status.contains(expected),
            "{command}: status was {}",
            app.status
        );
        assert_eq!(app.session.session_id, before, "{command} must not switch");
    }
}

#[tokio::test]
async fn test_project_command_prune_removes_archived_clean_worktrees() {
    let (mut app, _project, _base) = app_with_project("pruned");
    app.execute_slash_command("/session new --project pruned --branch done")
        .await;
    let id = app.session.session_id.clone().expect("session");
    let worktree = app.session.directory.clone();
    assert!(worktree.is_dir());

    // Archiving closes the session; focus a session of the same project so
    // `/project prune` acts on it.
    app.execute_slash_command("/session archive").await;
    assert!(worktree.is_dir(), "archiving leaves the worktree on disk");
    app.execute_slash_command("/session new --project pruned")
        .await;

    app.execute_slash_command("/project prune").await;

    let report = last_message(&app);
    assert!(report.contains("Removed 1 worktree"), "{report}");
    assert!(!worktree.exists(), "the clean worktree was removed");
    assert_eq!(
        app.storage
            .session_checkout(&id)
            .expect("get")
            .expect("row")
            .worktree,
        None
    );
}
