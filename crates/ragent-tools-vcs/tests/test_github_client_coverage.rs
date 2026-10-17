//! Coverage tests for `GitHubClient` request construction, error mapping, and
//! binary/readme fetch paths, driven against a local `wiremock` server.
//!
//! These exercise the client verbs and guards that the agent-facing tools call
//! indirectly: `get_bytes`, `get_bytes_limited`, `current_user`,
//! `fetch_repo_metadata`, `fetch_root_tree`, `fetch_readme`, `detect_repo`,
//! `with_token`, `client_id`, and the shared status classifier.

use std::process::Command;
use std::sync::atomic::{AtomicUsize, Ordering};

use ragent_tools_vcs::github::GitHubClient;
use wiremock::matchers::{header, method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

fn client(server: &MockServer) -> GitHubClient {
    GitHubClient::with_base_url(server.uri(), "test-token".to_string())
}

#[test]
fn test_with_token_builds_and_client_id_defaults_non_empty() {
    // `with_token` builds a default-base client; the OAuth client id resolver
    // reads `RAGENT_GITHUB_CLIENT_ID` with a compiled fallback. Asserting the
    // fallback is non-empty covers that branch without any network access.
    let c = GitHubClient::with_token("ghp_example".to_string());
    assert!(
        !GitHubClient::client_id().is_empty(),
        "client id must never be empty"
    );
    drop(c);
}

#[tokio::test]
async fn test_get_relative_path_appends_to_base_url() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/user"))
        .and(header("authorization", "Bearer test-token"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({"login": "me"})))
        .mount(&server)
        .await;

    let c = client(&server);
    let user = c.current_user().await.unwrap();
    assert_eq!(user["login"], "me");
}

#[tokio::test]
async fn test_get_absolute_url_used_verbatim() {
    let server = MockServer::start().await;
    let abs = format!("{}/custom/path", server.uri());
    Mock::given(method("GET"))
        .and(path("/custom/path"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({"ok": true})))
        .mount(&server)
        .await;

    let c = client(&server);
    let v = c.get(&abs).await.unwrap();
    assert_eq!(v["ok"], true);
}

#[tokio::test]
async fn test_check_status_rate_limited_429() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/rate"))
        .respond_with(
            ResponseTemplate::new(429)
                .insert_header("x-ratelimit-remaining", "0")
                .insert_header("x-ratelimit-reset", "1700000000"),
        )
        .mount(&server)
        .await;

    let c = client(&server);
    let err = c.get("/rate").await.unwrap_err().to_string();
    assert!(err.contains("rate limit"), "got: {err}");
    assert!(err.contains("1700000000"), "reset time missing: {err}");
}

#[tokio::test]
async fn test_check_status_permission_denied_403_without_quota() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/forbidden"))
        .respond_with(ResponseTemplate::new(403))
        .mount(&server)
        .await;

    let c = client(&server);
    let err = c.get("/forbidden").await.unwrap_err().to_string();
    assert!(err.contains("permission denied"), "got: {err}");
}

#[tokio::test]
async fn test_check_status_unauthorized_401() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/unauth"))
        .respond_with(ResponseTemplate::new(401))
        .mount(&server)
        .await;

    let c = client(&server);
    let err = c.get("/unauth").await.unwrap_err().to_string();
    assert!(err.contains("authentication failed"), "got: {err}");
}

#[tokio::test]
async fn test_handle_response_generic_non_success_body() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/boom"))
        .respond_with(ResponseTemplate::new(500).set_body_string("kaboom"))
        .mount(&server)
        .await;

    let c = client(&server);
    let err = c.get("/boom").await.unwrap_err().to_string();
    assert!(err.contains("500"), "got: {err}");
    assert!(err.contains("kaboom"), "got: {err}");
}

// --- get_bytes -------------------------------------------------------------

#[tokio::test]
async fn test_get_bytes_success() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/raw/file"))
        .respond_with(ResponseTemplate::new(200).set_body_bytes(b"hello-bytes".to_vec()))
        .mount(&server)
        .await;

    let c = client(&server);
    let bytes = c.get_bytes("/raw/file").await.unwrap();
    assert_eq!(bytes, b"hello-bytes");
}

#[tokio::test]
async fn test_get_bytes_refuses_cross_origin() {
    let server = MockServer::start().await;
    let c = client(&server);
    let err = c
        .get_bytes("https://evil.example.com/steal")
        .await
        .unwrap_err()
        .to_string();
    assert!(err.contains("cross-origin"), "got: {err}");
}

#[tokio::test]
async fn test_get_bytes_non_success_status() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/raw/missing"))
        .respond_with(ResponseTemplate::new(404).set_body_string("nope"))
        .mount(&server)
        .await;

    let c = client(&server);
    let err = c.get_bytes("/raw/missing").await.unwrap_err().to_string();
    assert!(err.contains("404"), "got: {err}");
}

// --- get_bytes_limited -----------------------------------------------------

#[tokio::test]
async fn test_get_bytes_limited_success() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/limited/ok"))
        .respond_with(ResponseTemplate::new(200).set_body_bytes(b"12345".to_vec()))
        .mount(&server)
        .await;

    let c = client(&server);
    let bytes = c.get_bytes_limited("/limited/ok", 100).await.unwrap();
    assert_eq!(bytes, b"12345");
}

#[tokio::test]
async fn test_get_bytes_limited_exceeds_cap() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/limited/big"))
        .respond_with(ResponseTemplate::new(200).set_body_bytes(vec![b'x'; 64]))
        .mount(&server)
        .await;

    let c = client(&server);
    let err = c
        .get_bytes_limited("/limited/big", 8)
        .await
        .unwrap_err()
        .to_string();
    assert!(err.contains("exceeded"), "got: {err}");
}

#[tokio::test]
async fn test_get_bytes_limited_refuses_cross_origin() {
    let server = MockServer::start().await;
    let c = client(&server);
    let err = c
        .get_bytes_limited("http://10.0.0.1/internal", 1024)
        .await
        .unwrap_err()
        .to_string();
    assert!(err.contains("cross-origin"), "got: {err}");
}

// --- typed repo fetches ----------------------------------------------------

#[tokio::test]
async fn test_fetch_repo_metadata_success() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/repos/octocat/Hello-World"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "description": "demo",
            "language": "Rust",
            "topics": ["a", "b"],
            "stargazers_count": 7,
            "default_branch": "main"
        })))
        .mount(&server)
        .await;

    let c = client(&server);
    let md = c
        .fetch_repo_metadata("octocat", "Hello-World")
        .await
        .unwrap();
    assert_eq!(md.description, "demo");
    assert_eq!(md.language, "Rust");
    assert_eq!(md.topics, vec!["a".to_string(), "b".to_string()]);
    assert_eq!(md.stargazers_count, 7);
    assert_eq!(md.default_branch, "main");
}

#[tokio::test]
async fn test_fetch_repo_metadata_rejects_path_traversal_segment() {
    let server = MockServer::start().await;
    let c = client(&server);
    let err = c
        .fetch_repo_metadata("bad owner", "repo")
        .await
        .unwrap_err()
        .to_string();
    assert!(err.contains("Invalid GitHub owner"), "got: {err}");

    let err2 = c
        .fetch_repo_metadata("owner", "repo/extra")
        .await
        .unwrap_err()
        .to_string();
    assert!(err2.contains("Invalid GitHub repo"), "got: {err2}");
}

#[tokio::test]
async fn test_fetch_root_tree_success_and_guards() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/repos/o/r/contents"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!([
            {"name": "README.md", "type": "file"},
            {"name": "src", "type": "dir"}
        ])))
        .mount(&server)
        .await;

    let c = client(&server);
    let tree = c.fetch_root_tree("o", "r").await.unwrap();
    assert_eq!(tree, vec!["README.md".to_string(), "src".to_string()]);

    let err = c.fetch_root_tree("", "r").await.unwrap_err().to_string();
    assert!(err.contains("Invalid GitHub owner"), "got: {err}");
}

#[tokio::test]
async fn test_fetch_readme_404_returns_none() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/repos/o/r/readme"))
        .respond_with(ResponseTemplate::new(404))
        .mount(&server)
        .await;

    let c = client(&server);
    assert!(c.fetch_readme("o", "r").await.unwrap().is_none());
}

#[tokio::test]
async fn test_fetch_readme_success_via_download_url() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/repos/o/r/readme"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "download_url": format!("{}/raw/readme", server.uri())
        })))
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/raw/readme"))
        .respond_with(ResponseTemplate::new(200).set_body_string("# Hello"))
        .mount(&server)
        .await;

    let c = client(&server);
    let readme = c.fetch_readme("o", "r").await.unwrap();
    assert_eq!(readme.as_deref(), Some("# Hello"));
}

#[tokio::test]
async fn test_fetch_readme_missing_download_url_errors() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/repos/o/r/readme"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(serde_json::json!({"name": "README"})),
        )
        .mount(&server)
        .await;

    let c = client(&server);
    let err = c.fetch_readme("o", "r").await.unwrap_err().to_string();
    assert!(err.contains("download_url"), "got: {err}");
}

// --- detect_repo -----------------------------------------------------------

fn git_in(dir: &std::path::Path, args: &[&str]) {
    let status = Command::new("git")
        .args(args)
        .current_dir(dir)
        .output()
        .expect("run git");
    assert!(status.status.success(), "git {args:?} failed");
}

static UNIQUE: AtomicUsize = AtomicUsize::new(0);

#[test]
fn test_detect_repo_parses_ssh_and_https_remotes() {
    let n = UNIQUE.fetch_add(1, Ordering::SeqCst);
    let dir = std::env::temp_dir().join(format!("vcs_detect_{}_{n}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    git_in(&dir, &["init"]);
    git_in(
        &dir,
        &[
            "remote",
            "add",
            "origin",
            "git@github.com:octocat/Hello-World.git",
        ],
    );

    let detected = GitHubClient::detect_repo(&dir);
    assert_eq!(
        detected,
        Some(("octocat".to_string(), "Hello-World".to_string()))
    );

    git_in(
        &dir,
        &[
            "remote",
            "set-url",
            "origin",
            "https://github.com/foo/bar.git",
        ],
    );
    let detected = GitHubClient::detect_repo(&dir);
    assert_eq!(detected, Some(("foo".to_string(), "bar".to_string())));

    // A non-GitHub remote yields None.
    git_in(
        &dir,
        &["remote", "set-url", "origin", "https://example.com/x/y.git"],
    );
    assert_eq!(GitHubClient::detect_repo(&dir), None);

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn test_detect_repo_no_repo_returns_none() {
    let n = UNIQUE.fetch_add(1, Ordering::SeqCst);
    let dir = std::env::temp_dir().join(format!("vcs_norepo_{}_{n}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    assert_eq!(GitHubClient::detect_repo(&dir), None);
    let _ = std::fs::remove_dir_all(&dir);
}
