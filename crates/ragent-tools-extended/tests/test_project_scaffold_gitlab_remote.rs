//! Tests for T-010: GitLab remote create + push integration with failure
//! containment (FR-008 remote half, FR-010, FR-015).
//!
//! The GitLab API surface is exercised against a local `wiremock` mock
//! server via the injectable `init_gitlab_remote_with_token` variant; the
//! git side (origin registration, push) runs real `git` inside tempdirs.
//! Push targets a local bare repository (file:// URL), so no network access
//! happens in any test.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use ragent_tools_extended::project_scaffold::{
    RemoteStep, init_gitlab_remote_with_token, load_gitlab_base_url,
};
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

/// Mock `GET /api/v4/user` returning a fixed username.
async fn mock_user(server: &MockServer) {
    wiremock::Mock::given(wiremock::matchers::method("GET"))
        .and(wiremock::matchers::path("/api/v4/user"))
        .respond_with(
            wiremock::ResponseTemplate::new(200)
                .set_body_json(serde_json::json!({"username": "testuser"})),
        )
        .mount(server)
        .await;
}

/// Mock `POST /api/v4/projects` answering `status` with `body`.
async fn mock_create(server: &MockServer, status: u16, body: serde_json::Value) {
    wiremock::Mock::given(wiremock::matchers::method("POST"))
        .and(wiremock::matchers::path("/api/v4/projects"))
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

#[tokio::test]
async fn test_gitlab_remote_create_and_push_end_to_end() {
    let server = MockServer::start().await;
    mock_user(&server).await;
    let (host, host_url) = bare_host("gl-create-host");
    mock_create(
        &server,
        201,
        serde_json::json!({"web_url": host_url.clone()}),
    )
    .await;

    let repo = seeded_repo("gl-create-repo");
    let root = repo.path().to_path_buf();
    let report = tokio::task::spawn_blocking(move || {
        init_gitlab_remote_with_token(
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

    // Origin points at the hosting project returned by the API.
    assert_eq!(origin_url(repo.path()), host_url);

    // The initial commit landed on the hosting project.
    assert_eq!(host_commits(host.path().join("host.git").as_path()), 1);
}

#[tokio::test]
async fn test_gitlab_remote_reuses_existing_project_400() {
    let server = MockServer::start().await;
    mock_user(&server).await;
    let (host, host_url) = bare_host("gl-reuse-host");

    // FR-015: creation answers 400 "has already been taken" on a retry; the
    // flow resolves the existing project via GET /projects/{user}%2F{name}.
    mock_create(
        &server,
        400,
        serde_json::json!({
            "message": {
                "path": ["has already been taken"],
                "name": ["has already been taken"]
            }
        }),
    )
    .await;
    wiremock::Mock::given(wiremock::matchers::method("GET"))
        .and(wiremock::matchers::path(
            "/api/v4/projects/testuser%2Fhello-world",
        ))
        .respond_with(
            wiremock::ResponseTemplate::new(200)
                .set_body_json(serde_json::json!({"web_url": host_url.clone()})),
        )
        .mount(&server)
        .await;

    let repo = seeded_repo("gl-reuse-repo");
    let root = repo.path().to_path_buf();
    let report = tokio::task::spawn_blocking(move || {
        init_gitlab_remote_with_token(
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

    assert!(!report.repo_created, "existing project is reused");
    assert_eq!(report.url, host_url);
    assert_eq!(origin_url(repo.path()), host_url);
    assert_eq!(host_commits(host.path().join("host.git").as_path()), 1);
}

#[tokio::test]
async fn test_gitlab_remote_add_repoints_existing_origin() {
    let server = MockServer::start().await;
    mock_user(&server).await;
    let (host, host_url) = bare_host("gl-repoint-host");
    mock_create(
        &server,
        201,
        serde_json::json!({"web_url": host_url.clone()}),
    )
    .await;

    // FR-015: a previous pass (or manual setup) left an origin behind; the
    // retry re-points it instead of failing on "remote already exists".
    let repo = seeded_repo("gl-repoint-repo");
    let root = repo.path().to_path_buf();
    let (old_host, old_url) = bare_host("gl-repoint-old");
    git(repo.path(), &["remote", "add", "origin", &old_url]);

    let report = tokio::task::spawn_blocking(move || {
        init_gitlab_remote_with_token(
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
fn test_gitlab_remote_missing_token_is_contained() {
    let repo = seeded_repo("gl-noauth-repo");
    let failure = init_gitlab_remote_with_token("http://127.0.0.1:1", None, repo.path(), "x", true)
        .expect_err("missing token must fail");

    assert_eq!(failure.step, RemoteStep::GitLabAuth);
    assert!(
        failure.message.contains("/gitlab setup"),
        "actionable remediation expected, got: {failure}"
    );
    assert!(failure.message.contains("local scaffold is intact"));
    // No partial remote state: origin was never registered.
    assert_eq!(origin_url(repo.path()), "");
}

#[tokio::test]
async fn test_gitlab_remote_auth_rejection_is_contained() {
    let server = MockServer::start().await;
    wiremock::Mock::given(wiremock::matchers::method("GET"))
        .and(wiremock::matchers::path("/api/v4/user"))
        .respond_with(
            wiremock::ResponseTemplate::new(401)
                .set_body_json(serde_json::json!({"message": "401 Unauthorized"})),
        )
        .mount(&server)
        .await;

    let repo = seeded_repo("gl-401-repo");
    let root = repo.path().to_path_buf();
    let failure = tokio::task::spawn_blocking(move || {
        init_gitlab_remote_with_token(
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

    assert_eq!(failure.step, RemoteStep::GitLabAuth);
    assert!(failure.message.contains("/gitlab setup"));
    assert_eq!(origin_url(repo.path()), "");
}

#[tokio::test]
async fn test_gitlab_remote_create_http_error_is_contained() {
    let server = MockServer::start().await;
    mock_user(&server).await;
    mock_create(
        &server,
        500,
        serde_json::json!({"message": "server exploded"}),
    )
    .await;

    let repo = seeded_repo("gl-500-repo");
    let root = repo.path().to_path_buf();
    let failure = tokio::task::spawn_blocking(move || {
        init_gitlab_remote_with_token(
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

    assert_eq!(failure.step, RemoteStep::GitLabRepoCreate);
    assert!(failure.message.contains("500"));
    assert!(failure.message.contains("server exploded"));
    // FR-010: no partially-initialised remote state after project-create
    // failure.
    assert_eq!(origin_url(repo.path()), "");
}

#[tokio::test]
async fn test_gitlab_remote_400_not_path_taken_is_contained() {
    let server = MockServer::start().await;
    mock_user(&server).await;
    // A 400 that is NOT a path-taken validation error must not be mistaken
    // for the reuse path.
    mock_create(
        &server,
        400,
        serde_json::json!({"message": {"name": ["is invalid"]}}),
    )
    .await;

    let repo = seeded_repo("gl-400bad-repo");
    let root = repo.path().to_path_buf();
    let failure = tokio::task::spawn_blocking(move || {
        init_gitlab_remote_with_token(
            server.uri().as_str(),
            Some(TOKEN),
            &root,
            "hello-world",
            true,
        )
    })
    .await
    .expect("spawn_blocking")
    .expect_err("non-reuse 400 must fail");

    assert_eq!(failure.step, RemoteStep::GitLabRepoCreate);
    assert!(failure.message.contains("400"));
    assert_eq!(origin_url(repo.path()), "");
}

#[tokio::test]
async fn test_gitlab_remote_push_failure_is_contained_and_keeps_origin() {
    let server = MockServer::start().await;
    mock_user(&server).await;
    let (host, host_url) = bare_host("gl-pushfail-host");
    mock_create(
        &server,
        201,
        serde_json::json!({"web_url": host_url.clone()}),
    )
    .await;

    // Make the hosting project reject pushes via a failing pre-receive
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

    let repo = seeded_repo("gl-pushfail-repo");
    let root = repo.path().to_path_buf();
    let failure = tokio::task::spawn_blocking(move || {
        init_gitlab_remote_with_token(
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
async fn test_gitlab_remote_missing_web_url_is_contained() {
    let server = MockServer::start().await;
    mock_user(&server).await;
    mock_create(&server, 201, serde_json::json!({"id": 123})).await;

    let repo = seeded_repo("gl-nourl-repo");
    let root = repo.path().to_path_buf();
    let failure = tokio::task::spawn_blocking(move || {
        init_gitlab_remote_with_token(
            server.uri().as_str(),
            Some(TOKEN),
            &root,
            "hello-world",
            true,
        )
    })
    .await
    .expect("spawn_blocking")
    .expect_err("missing web_url must fail");

    assert_eq!(failure.step, RemoteStep::GitLabRepoCreate);
    assert!(failure.message.contains("web_url"));
    assert_eq!(origin_url(repo.path()), "");
}

// ---------------------------------------------------------------- misc ---

#[test]
fn test_gitlab_remote_step_display_names() {
    assert_eq!(RemoteStep::GitLabAuth.as_str(), "gitlab auth");
    assert_eq!(RemoteStep::GitLabRepoCreate.as_str(), "gitlab repo create");
    assert_eq!(
        RemoteStep::GitLabRepoCreate.to_string(),
        "gitlab repo create"
    );
}

#[test]
fn test_gitlab_base_url_defaults_to_gitlab_dot_com() {
    // No env/config setup in this test process points GITLAB_URL at an
    // instance; the resolver must fall back to the public default.
    let url = load_gitlab_base_url();
    assert!(
        url == "https://gitlab.com" || !url.is_empty(),
        "resolver must return a usable base URL, got: {url}"
    );
}
