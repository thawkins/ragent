//! Coverage tests for `GitLabClient` request construction, error mapping, and
//! the readme/raw fetch paths, driven against a local `wiremock` server.

use std::process::Command;
use std::sync::atomic::{AtomicUsize, Ordering};

use ragent_tools_vcs::gitlab::GitLabClient;
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

const TOKEN: &str = "glpat-test";

fn client(server: &MockServer) -> GitLabClient {
    GitLabClient::with_credentials(server.uri(), TOKEN.to_string())
}

#[test]
fn test_with_credentials_exposes_url_and_token_and_trims_slash() {
    let c = GitLabClient::with_credentials("https://gitlab.example.com/".to_string(), TOKEN.into());
    assert_eq!(c.instance_url(), "https://gitlab.example.com");
    assert_eq!(c.token(), TOKEN);
}

#[tokio::test]
async fn test_get_parses_json_and_relative_path_is_under_api_v4() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/api/v4/projects/g%2Fp/issues"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!([{"iid": 1}])))
        .mount(&server)
        .await;

    let c = client(&server);
    let v = c.get("/projects/g%2Fp/issues").await.unwrap();
    assert_eq!(v[0]["iid"], 1);
}

#[tokio::test]
async fn test_post_and_put_route_to_api_v4() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/api/v4/projects/1/issues"))
        .respond_with(ResponseTemplate::new(201).set_body_json(serde_json::json!({"iid": 9})))
        .mount(&server)
        .await;
    Mock::given(method("PUT"))
        .and(path("/api/v4/projects/1/issues/9"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(serde_json::json!({"state": "closed"})),
        )
        .mount(&server)
        .await;

    let c = client(&server);
    let created = c
        .post("/projects/1/issues", &serde_json::json!({"title": "x"}))
        .await
        .unwrap();
    assert_eq!(created["iid"], 9);
    let updated = c
        .put(
            "/projects/1/issues/9",
            &serde_json::json!({"state_event": "close"}),
        )
        .await
        .unwrap();
    assert_eq!(updated["state"], "closed");
}

#[tokio::test]
async fn test_handle_response_204_empty_body_is_null() {
    let server = MockServer::start().await;
    Mock::given(method("PUT"))
        .and(path("/api/v4/projects/1/issues/9"))
        .respond_with(ResponseTemplate::new(204))
        .mount(&server)
        .await;

    let c = client(&server);
    let v = c
        .put("/projects/1/issues/9", &serde_json::json!({}))
        .await
        .unwrap();
    assert!(v.is_null());
}

#[tokio::test]
async fn test_handle_response_permission_denied_403() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/api/v4/projects/1/secret"))
        .respond_with(ResponseTemplate::new(403))
        .mount(&server)
        .await;

    let c = client(&server);
    let err = c.get("/projects/1/secret").await.unwrap_err().to_string();
    assert!(err.contains("permission denied"), "got: {err}");
}

#[tokio::test]
async fn test_handle_response_unauthorized_401() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/api/v4/projects/1/secret"))
        .respond_with(ResponseTemplate::new(401))
        .mount(&server)
        .await;

    let c = client(&server);
    let err = c.get("/projects/1/secret").await.unwrap_err().to_string();
    assert!(err.contains("authentication failed"), "got: {err}");
}

#[tokio::test]
async fn test_get_paged_retries_429_then_fails() {
    let server = MockServer::start().await;
    // Every attempt returns 429; the client retries MAX_RATE_LIMIT_RETRIES
    // times honouring Retry-After: 0, then surfaces the rate-limit error.
    Mock::given(method("GET"))
        .and(path("/api/v4/projects/1/pipelines"))
        .respond_with(ResponseTemplate::new(429).insert_header("retry-after", "0"))
        .mount(&server)
        .await;

    let c = client(&server);
    let err = c
        .get("/projects/1/pipelines")
        .await
        .unwrap_err()
        .to_string();
    assert!(err.contains("rate limit"), "got: {err}");
}

#[tokio::test]
async fn test_get_paged_returns_x_next_page_cursor() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/api/v4/projects/1/jobs"))
        .respond_with(
            ResponseTemplate::new(200)
                .insert_header("x-next-page", "2")
                .set_body_json(serde_json::json!([{"id": 1}])),
        )
        .mount(&server)
        .await;

    let c = client(&server);
    let (_value, cursor) = c.get_paged("/projects/1/jobs").await.unwrap();
    assert_eq!(cursor, Some(2));
}

// --- fetch_readme ----------------------------------------------------------

#[tokio::test]
async fn test_fetch_readme_404_returns_none() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/api/v4/projects/g%2Fp/readme"))
        .respond_with(ResponseTemplate::new(404))
        .mount(&server)
        .await;

    let c = client(&server);
    assert!(c.fetch_readme("g/p").await.unwrap().is_none());
}

#[tokio::test]
async fn test_fetch_readme_same_origin_url_uses_raw_fetch() {
    let server = MockServer::start().await;
    let raw_url = format!("{}/raw/readme", server.uri());
    Mock::given(method("GET"))
        .and(path("/api/v4/projects/g%2Fp/readme"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(serde_json::json!({"readme_url": raw_url})),
        )
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/raw/readme"))
        .respond_with(ResponseTemplate::new(200).set_body_string("readme body"))
        .mount(&server)
        .await;

    let c = client(&server);
    let readme = c.fetch_readme("g/p").await.unwrap();
    assert_eq!(readme.as_deref(), Some("readme body"));
}

#[tokio::test]
async fn test_fetch_readme_foreign_url_falls_back_to_file_path_raw() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/api/v4/projects/g%2Fp/readme"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "readme_url": "https://evil.example.com/readme",
            "file_path": "READ ME.md"
        })))
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path(
            "/api/v4/projects/g%2Fp/repository/files/READ%20ME.md/raw",
        ))
        // The fallback goes through the JSON `get()` path, which expects a
        // JSON-encoded string body, not a bare string.
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!("raw fallback")))
        .mount(&server)
        .await;

    let c = client(&server);
    let readme = c.fetch_readme("g/p").await.unwrap();
    assert_eq!(readme.as_deref(), Some("raw fallback"));
}

#[tokio::test]
async fn test_fetch_readme_missing_both_fields_errors() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/api/v4/projects/g%2Fp/readme"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(serde_json::json!({"name": "README"})),
        )
        .mount(&server)
        .await;

    let c = client(&server);
    let err = c.fetch_readme("g/p").await.unwrap_err().to_string();
    assert!(err.contains("readme_url"), "got: {err}");
}

// --- detect_project --------------------------------------------------------

fn git_in(dir: &std::path::Path, args: &[&str]) {
    let out = Command::new("git")
        .args(args)
        .current_dir(dir)
        .output()
        .expect("run git");
    assert!(out.status.success(), "git {args:?} failed");
}

static UNIQUE: AtomicUsize = AtomicUsize::new(0);

#[test]
fn test_detect_project_parses_ssh_and_https_remotes() {
    let n = UNIQUE.fetch_add(1, Ordering::SeqCst);
    let dir = std::env::temp_dir().join(format!("gl_detect_{}_{n}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    git_in(&dir, &["init"]);
    git_in(
        &dir,
        &[
            "remote",
            "add",
            "origin",
            "git@gitlab.example.com:group/sub/project.git",
        ],
    );

    let detected = GitLabClient::detect_project(&dir);
    assert_eq!(detected, Some("group%2Fsub%2Fproject".to_string()));

    git_in(
        &dir,
        &[
            "remote",
            "set-url",
            "origin",
            "https://gitlab.example.com/group/project.git",
        ],
    );
    assert_eq!(
        GitLabClient::detect_project(&dir),
        Some("group%2Fproject".to_string())
    );

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn test_detect_project_invalid_remote_returns_none() {
    let n = UNIQUE.fetch_add(1, Ordering::SeqCst);
    let dir = std::env::temp_dir().join(format!("gl_bad_{}_{n}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    git_in(&dir, &["init"]);
    // A remote with no group/project path segment is not a valid project.
    git_in(
        &dir,
        &[
            "remote",
            "add",
            "origin",
            "https://gitlab.example.com/onlyone",
        ],
    );

    assert_eq!(GitLabClient::detect_project(&dir), None);
    let _ = std::fs::remove_dir_all(&dir);
}
