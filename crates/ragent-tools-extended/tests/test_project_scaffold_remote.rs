//! Tests for T-009: GitHub remote create + push integration with failure
//! containment (FR-008 remote half, FR-010, FR-015).
//!
//! The GitHub API surface is exercised against a local `wiremock` mock
//! server via the injectable `init_github_remote_with_token` variant; the
//! git side (origin registration, push) runs real `git` inside tempdirs.
//! Push targets a local bare repository (file:// URL), so no network access
//! happens in any test.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use ragent_tools_extended::project_scaffold::{RemoteStep, init_github_remote_with_token};
use wiremock::MockServer;

const TOKEN: &str = "test-token";

/// Fresh temp root for one test.
fn temp_root(name: &str) -> tempfile::TempDir {
    tempfile::Builder::new()
        .prefix(name)
        .tempdir()
        .expect("tempdir")
}

/// Run a git command in `root`, returning combined stdout+stderr.
fn git(root: &Path, args: &[&str]) -> String {
    let output = Command::new("git")
        .args(args)
        .current_dir(root)
        .output()
        .expect("git spawn");
    let stdout = String::from_utf8_lossy(&output.stdout).into_owned();
    let stderr = String::from_utf8_lossy(&output.stderr).into_owned();
    format!("{stdout}{stderr}")
}

/// A local git repository with one commit, simulating the post-emission
/// state the remote-init flow runs against.
fn seeded_repo(name: &str) -> tempfile::TempDir {
    let root = temp_root(name);
    fs::write(root.path().join("main.rs"), b"fn main() {}\n").expect("write");
    git(root.path(), &["init"]);
    git(root.path(), &["config", "user.name", "test-runner"]);
    git(
        root.path(),
        &["config", "user.email", "test@example.invalid"],
    );
    git(root.path(), &["add", "-A"]);
    git(root.path(), &["commit", "-m", "Initial scaffold"]);
    root
}

/// A local bare repository acting as the "hosting" remote (file:// URL).
fn bare_host(name: &str) -> (tempfile::TempDir, String) {
    let dir = temp_root(name);
    git(dir.path(), &["init", "--bare", "host.git"]);
    let path: PathBuf = dir.path().join("host.git");
    (dir, format!("file://{}", path.display()))
}

/// Mock `GET /user` returning a fixed login.
async fn mock_user(server: &MockServer) {
    wiremock::Mock::given(wiremock::matchers::method("GET"))
        .and(wiremock::matchers::path("/user"))
        .respond_with(
            wiremock::ResponseTemplate::new(200)
                .set_body_json(serde_json::json!({"login": "octocat"})),
        )
        .mount(server)
        .await;
}

/// Mock `POST /user/repos` answering `status` with `body`.
async fn mock_create(server: &MockServer, status: u16, body: serde_json::Value) {
    wiremock::Mock::given(wiremock::matchers::method("POST"))
        .and(wiremock::matchers::path("/user/repos"))
        .respond_with(wiremock::ResponseTemplate::new(status).set_body_json(body))
        .mount(server)
        .await;
}

/// The origin URL registered in the seeded repo (empty when absent).
fn origin_url(root: &Path) -> String {
    let out = git(root, &["remote", "get-url", "origin"]);
    let trimmed = out.trim();
    if trimmed.starts_with("error:") || trimmed.is_empty() {
        String::new()
    } else {
        trimmed.to_owned()
    }
}

/// Commits reachable on the bare host's default branch (proves the push).
fn host_commits(host: &Path) -> usize {
    let out = git(host, &["rev-list", "--count", "HEAD"]);
    out.trim().parse().unwrap_or(0)
}

// ------------------------------------------------- success + FR-015 reuse ---

#[tokio::test]
async fn test_github_remote_create_and_push_end_to_end() {
    let server = MockServer::start().await;
    mock_user(&server).await;
    let (host, host_url) = bare_host("remote-create-host");
    mock_create(
        &server,
        201,
        serde_json::json!({"html_url": host_url.clone()}),
    )
    .await;

    let repo = seeded_repo("remote-create-repo");
    let root = repo.path().to_path_buf();
    let report = tokio::task::spawn_blocking(move || {
        init_github_remote_with_token(
            server.uri().as_str(),
            Some(TOKEN),
            &root,
            "hello-world",
            true,
        )
    })
    .await
    .expect("spawn_blocking")
    .expect("remote init");

    // Report reflects a fresh create, newly added origin.
    assert!(report.repo_created);
    assert!(!report.remote_existed);
    assert_eq!(report.url, host_url);

    // Origin points at the hosting repository returned by the API.
    assert_eq!(origin_url(repo.path()), host_url);

    // The initial commit landed on the hosting repository.
    assert_eq!(host_commits(host.path().join("host.git").as_path()), 1);
}

#[tokio::test]
async fn test_github_remote_reuses_existing_repository_422() {
    let server = MockServer::start().await;
    mock_user(&server).await;
    let (host, host_url) = bare_host("remote-reuse-host");

    // FR-015: creation answers 422 "already exists" on a retry; the flow
    // resolves the existing repository via GET /repos/{login}/{name}.
    mock_create(
        &server,
        422,
        serde_json::json!({
            "message": "name already exists on this account",
            "errors": [{"message": "name already exists on this account"}]
        }),
    )
    .await;
    wiremock::Mock::given(wiremock::matchers::method("GET"))
        .and(wiremock::matchers::path("/repos/octocat/hello-world"))
        .respond_with(
            wiremock::ResponseTemplate::new(200)
                .set_body_json(serde_json::json!({"html_url": host_url.clone()})),
        )
        .mount(&server)
        .await;

    let repo = seeded_repo("remote-reuse-repo");
    let root = repo.path().to_path_buf();
    let report = tokio::task::spawn_blocking(move || {
        init_github_remote_with_token(
            server.uri().as_str(),
            Some(TOKEN),
            &root,
            "hello-world",
            true,
        )
    })
    .await
    .expect("spawn_blocking")
    .expect("remote init via reuse");

    assert!(!report.repo_created, "existing repository is reused");
    assert_eq!(report.url, host_url);
    assert_eq!(origin_url(repo.path()), host_url);
    assert_eq!(host_commits(host.path().join("host.git").as_path()), 1);
}

#[tokio::test]
async fn test_github_remote_add_repoints_existing_origin() {
    let server = MockServer::start().await;
    mock_user(&server).await;
    let (host, host_url) = bare_host("remote-repoint-host");
    mock_create(
        &server,
        201,
        serde_json::json!({"html_url": host_url.clone()}),
    )
    .await;

    // FR-015: a previous pass (or manual setup) left an origin behind; the
    // retry re-points it instead of failing on "remote already exists".
    let repo = seeded_repo("remote-repoint-repo");
    let root = repo.path().to_path_buf();
    let (old_host, old_url) = bare_host("remote-repoint-old");
    git(repo.path(), &["remote", "add", "origin", &old_url]);

    let report = tokio::task::spawn_blocking(move || {
        init_github_remote_with_token(
            server.uri().as_str(),
            Some(TOKEN),
            &root,
            "hello-world",
            true,
        )
    })
    .await
    .expect("spawn_blocking")
    .expect("remote init with existing origin");

    assert!(report.remote_existed);
    assert_eq!(origin_url(repo.path()), host_url);
    assert_eq!(host_commits(host.path().join("host.git").as_path()), 1);
    drop(old_host);
}

// ---------------------------------------------------- FR-010 containment ---

#[test]
fn test_github_remote_missing_token_is_contained() {
    let repo = seeded_repo("remote-noauth-repo");
    let failure = init_github_remote_with_token("http://127.0.0.1:1", None, repo.path(), "x", true)
        .expect_err("missing token must fail");

    assert_eq!(failure.step, RemoteStep::Auth);
    assert!(
        failure.message.contains("/github login"),
        "actionable remediation expected, got: {failure}"
    );
    assert!(failure.message.contains("local scaffold is intact"));
    // No partial remote state: origin was never registered.
    assert_eq!(origin_url(repo.path()), "");
}

#[tokio::test]
async fn test_github_remote_auth_rejection_is_contained() {
    let server = MockServer::start().await;
    wiremock::Mock::given(wiremock::matchers::method("GET"))
        .and(wiremock::matchers::path("/user"))
        .respond_with(
            wiremock::ResponseTemplate::new(401)
                .set_body_json(serde_json::json!({"message": "Bad credentials"})),
        )
        .mount(&server)
        .await;

    let repo = seeded_repo("remote-401-repo");
    let root = repo.path().to_path_buf();
    let failure = tokio::task::spawn_blocking(move || {
        init_github_remote_with_token(
            server.uri().as_str(),
            Some(TOKEN),
            &root,
            "hello-world",
            true,
        )
    })
    .await
    .expect("spawn_blocking")
    .expect_err("401 must fail");

    assert_eq!(failure.step, RemoteStep::Auth);
    assert!(failure.message.contains("/github login"));
    assert_eq!(origin_url(repo.path()), "");
}

#[tokio::test]
async fn test_github_remote_create_http_error_is_contained() {
    let server = MockServer::start().await;
    mock_user(&server).await;
    mock_create(
        &server,
        500,
        serde_json::json!({"message": "server exploded"}),
    )
    .await;

    let repo = seeded_repo("remote-500-repo");
    let root = repo.path().to_path_buf();
    let failure = tokio::task::spawn_blocking(move || {
        init_github_remote_with_token(
            server.uri().as_str(),
            Some(TOKEN),
            &root,
            "hello-world",
            true,
        )
    })
    .await
    .expect("spawn_blocking")
    .expect_err("500 must fail");

    assert_eq!(failure.step, RemoteStep::RepoCreate);
    assert!(failure.message.contains("500"));
    assert!(failure.message.contains("server exploded"));
    // FR-010: no partially-initialised remote state after repo-create
    // failure.
    assert_eq!(origin_url(repo.path()), "");
}

#[tokio::test]
async fn test_github_remote_push_failure_is_contained_and_keeps_origin() {
    let server = MockServer::start().await;
    mock_user(&server).await;
    let (host, host_url) = bare_host("remote-pushfail-host");
    mock_create(
        &server,
        201,
        serde_json::json!({"html_url": host_url.clone()}),
    )
    .await;

    // Make the hosting repository reject pushes via a failing pre-receive
    // hook (server-side rejection, the most realistic push failure).
    let hook_dir = host.path().join("host.git/hooks");
    fs::create_dir_all(&hook_dir).expect("hooks dir");
    fs::write(hook_dir.join("pre-receive"), b"#!/bin/sh\nexit 1\n").expect("hook");
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(
            hook_dir.join("pre-receive"),
            fs::Permissions::from_mode(0o755),
        )
        .expect("chmod");
    }

    let repo = seeded_repo("remote-pushfail-repo");
    let root = repo.path().to_path_buf();
    let failure = tokio::task::spawn_blocking(move || {
        init_github_remote_with_token(
            server.uri().as_str(),
            Some(TOKEN),
            &root,
            "hello-world",
            true,
        )
    })
    .await
    .expect("spawn_blocking")
    .expect_err("rejected push must fail");

    assert_eq!(failure.step, RemoteStep::Push);
    // FR-010: actionable remediation, remote stays configured for a manual
    // retry.
    assert!(failure.message.contains("remediation:"));
    assert!(failure.message.contains("git push -u origin "));
    assert!(failure.message.contains("local scaffold is intact"));
    assert_eq!(origin_url(repo.path()), host_url);
}

#[tokio::test]
async fn test_github_remote_missing_html_url_is_contained() {
    let server = MockServer::start().await;
    mock_user(&server).await;
    mock_create(&server, 201, serde_json::json!({"id": 123})).await;

    let repo = seeded_repo("remote-nourl-repo");
    let root = repo.path().to_path_buf();
    let failure = tokio::task::spawn_blocking(move || {
        init_github_remote_with_token(
            server.uri().as_str(),
            Some(TOKEN),
            &root,
            "hello-world",
            true,
        )
    })
    .await
    .expect("spawn_blocking")
    .expect_err("missing html_url must fail");

    assert_eq!(failure.step, RemoteStep::RepoCreate);
    assert!(failure.message.contains("html_url"));
    assert_eq!(origin_url(repo.path()), "");
}

// ---------------------------------------------------------------- misc ---

#[test]
fn test_remote_step_display_names() {
    assert_eq!(RemoteStep::Auth.as_str(), "github auth");
    assert_eq!(RemoteStep::RepoCreate.as_str(), "github repo create");
    assert_eq!(RemoteStep::RemoteAdd.as_str(), "git remote add");
    assert_eq!(RemoteStep::Push.as_str(), "git push");
    assert_eq!(RemoteStep::Push.to_string(), "git push");
}
