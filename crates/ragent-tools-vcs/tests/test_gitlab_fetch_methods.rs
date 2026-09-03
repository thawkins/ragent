//! Unit tests for GitLab fetch methods with mock HTTP responses (FR-008,
//! FR-009, FR-010, FR-011).
//!
//! These tests exercise the async `fetch_project_metadata`,
//! `fetch_repository_tree`, and `fetch_readme` methods against a local mock
//! GitLab API server (via `wiremock`). They cover:
//!
//! - FR-008/FR-009: `fetch_project_metadata` maps the project JSON response
//!   and the `/languages` endpoint onto `RepoMetadata`.
//! - FR-010: `fetch_repository_tree` extracts root-level file/directory names.
//! - FR-011: `fetch_readme` returns `Ok(None)` on 404 and `Ok(Some(text))` on
//!   success via the `readme_url` field.

use ragent_tools_vcs::gitlab::GitLabClient;
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

const TOKEN: &str = "test-token";

fn client(server: &MockServer) -> GitLabClient {
    GitLabClient::with_credentials(server.uri(), TOKEN.to_string())
}

// ---------------------------------------------------------------------------
// FR-008 / FR-009: fetch_project_metadata
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_fetch_project_metadata_all_fields() {
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/api/v4/projects/group%2Fproject"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "description": "A test project",
            "topics": ["rust", "cli"],
            "star_count": 42,
            "default_branch": "main",
            "path_with_namespace": "group/project",
        })))
        .mount(&server)
        .await;

    Mock::given(method("GET"))
        .and(path("/api/v4/projects/group%2Fproject/languages"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "Rust": 80.0,
            "Shell": 20.0,
        })))
        .mount(&server)
        .await;

    let client = client(&server);
    let md = client
        .fetch_project_metadata("group/project")
        .await
        .unwrap();

    assert_eq!(md.description, "A test project");
    assert_eq!(md.language, "Rust");
    assert_eq!(md.topics, vec!["rust".to_string(), "cli".to_string()]);
    assert_eq!(md.stargazers_count, 42);
    assert_eq!(md.default_branch, "main");
}

#[tokio::test]
async fn test_fetch_project_metadata_null_description() {
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/api/v4/projects/group%2Fproject"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "description": null,
            "star_count": 0,
            "default_branch": "master",
        })))
        .mount(&server)
        .await;

    Mock::given(method("GET"))
        .and(path("/api/v4/projects/group%2Fproject/languages"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({})))
        .mount(&server)
        .await;

    let client = client(&server);
    let md = client
        .fetch_project_metadata("group/project")
        .await
        .unwrap();

    assert_eq!(md.description, "");
    assert_eq!(md.language, "");
    assert_eq!(md.topics, Vec::<String>::new());
    assert_eq!(md.stargazers_count, 0);
}

#[tokio::test]
async fn test_fetch_project_metadata_languages_endpoint_failure_defaults_empty() {
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/api/v4/projects/group%2Fproject"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "description": "desc",
            "star_count": 5,
            "default_branch": "main",
        })))
        .mount(&server)
        .await;

    // The /languages endpoint returns 500 — should NOT propagate; language
    // defaults to empty string (FR-009).
    Mock::given(method("GET"))
        .and(path("/api/v4/projects/group%2Fproject/languages"))
        .respond_with(ResponseTemplate::new(500).set_body_string("internal error"))
        .mount(&server)
        .await;

    let client = client(&server);
    let md = client
        .fetch_project_metadata("group/project")
        .await
        .unwrap();

    assert_eq!(md.description, "desc");
    assert_eq!(md.language, "");
    assert_eq!(md.stargazers_count, 5);
}

#[tokio::test]
async fn test_fetch_project_metadata_nested_namespace_url_encoded() {
    // FR-022: nested namespaces are URL-encoded as a single :id.
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/api/v4/projects/group%2Fsubgroup%2Fproject"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "description": "nested",
            "star_count": 1,
            "default_branch": "main",
            "topics": ["nested"],
        })))
        .mount(&server)
        .await;

    Mock::given(method("GET"))
        .and(path(
            "/api/v4/projects/group%2Fsubgroup%2Fproject/languages",
        ))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "Python": 100.0,
        })))
        .mount(&server)
        .await;

    let client = client(&server);
    let md = client
        .fetch_project_metadata("group/subgroup/project")
        .await
        .unwrap();

    assert_eq!(md.description, "nested");
    assert_eq!(md.language, "Python");
    assert_eq!(md.topics, vec!["nested".to_string()]);
}

#[tokio::test]
async fn test_fetch_project_metadata_project_404_propagates_error() {
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/api/v4/projects/missing%2Fproject"))
        .respond_with(ResponseTemplate::new(404).set_body_string("Not Found"))
        .mount(&server)
        .await;

    let client = client(&server);
    let result = client.fetch_project_metadata("missing/project").await;

    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("404"));
}

#[tokio::test]
async fn test_fetch_project_metadata_401_propagates_auth_error() {
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/api/v4/projects/group%2Fproject"))
        .respond_with(ResponseTemplate::new(401).set_body_string("Unauthorized"))
        .mount(&server)
        .await;

    let client = client(&server);
    let result = client.fetch_project_metadata("group/project").await;

    assert!(result.is_err());
    let msg = result.unwrap_err().to_string();
    assert!(
        msg.contains("authentication failed"),
        "expected auth error, got: {msg}"
    );
}

// ---------------------------------------------------------------------------
// FR-008 / FR-010: fetch_repository_tree
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_fetch_repository_tree_normal() {
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/api/v4/projects/group%2Fproject/repository/tree"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!([
            {"id": "abc", "name": "README.md", "type": "blob"},
            {"id": "def", "name": "src", "type": "tree"},
            {"id": "ghi", "name": "Cargo.toml", "type": "blob"},
        ])))
        .mount(&server)
        .await;

    let client = client(&server);
    let tree = client.fetch_repository_tree("group/project").await.unwrap();

    assert_eq!(
        tree,
        vec![
            "README.md".to_string(),
            "src".to_string(),
            "Cargo.toml".to_string(),
        ]
    );
}

#[tokio::test]
async fn test_fetch_repository_tree_empty() {
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/api/v4/projects/group%2Fproject/repository/tree"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!([])))
        .mount(&server)
        .await;

    let client = client(&server);
    let tree = client.fetch_repository_tree("group/project").await.unwrap();

    assert_eq!(tree, Vec::<String>::new());
}

#[tokio::test]
async fn test_fetch_repository_tree_404_propagates_error() {
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/api/v4/projects/missing%2Fproject/repository/tree"))
        .respond_with(ResponseTemplate::new(404).set_body_string("Not Found"))
        .mount(&server)
        .await;

    let client = client(&server);
    let result = client.fetch_repository_tree("missing/project").await;

    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("404"));
}

#[tokio::test]
async fn test_fetch_repository_tree_nested_namespace() {
    // FR-022: nested namespace URL-encoded.
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path(
            "/api/v4/projects/group%2Fsub%2Fproject/repository/tree",
        ))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!([
            {"name": "main.rs", "type": "blob"},
            {"name": "lib", "type": "tree"},
        ])))
        .mount(&server)
        .await;

    let client = client(&server);
    let tree = client
        .fetch_repository_tree("group/sub/project")
        .await
        .unwrap();

    assert_eq!(tree, vec!["main.rs".to_string(), "lib".to_string()],);
}

// ---------------------------------------------------------------------------
// FR-008 / FR-011: fetch_readme
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_fetch_readme_present_via_readme_url() {
    let server = MockServer::start().await;

    // The readme metadata endpoint returns a readme_url.
    Mock::given(method("GET"))
        .and(path("/api/v4/projects/group%2Fproject/readme"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "readme_url": format!("{}/raw/group/project/main/README.md", server.uri()),
            "file_path": "README.md",
        })))
        .mount(&server)
        .await;

    // The raw README content served at the readme_url.
    let _raw_url = format!("{}/raw/group/project/main/README.md", server.uri());
    Mock::given(method("GET"))
        .and(path("/raw/group/project/main/README.md"))
        .respond_with(ResponseTemplate::new(200).set_body_string("# My Project\n\nHello world!"))
        .mount(&server)
        .await;

    let client = client(&server);
    let readme = client.fetch_readme("group/project").await.unwrap();

    assert_eq!(readme, Some("# My Project\n\nHello world!".to_string()));
}

#[tokio::test]
async fn test_fetch_readme_404_returns_none() {
    // FR-011: a 404 on the readme endpoint returns Ok(None).
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/api/v4/projects/group%2Fproject/readme"))
        .respond_with(ResponseTemplate::new(404).set_body_string("Not Found"))
        .mount(&server)
        .await;

    let client = client(&server);
    let readme = client.fetch_readme("group/project").await.unwrap();

    assert!(readme.is_none());
}

#[tokio::test]
async fn test_fetch_readme_401_propagates_auth_error() {
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/api/v4/projects/group%2Fproject/readme"))
        .respond_with(ResponseTemplate::new(401).set_body_string("Unauthorized"))
        .mount(&server)
        .await;

    let client = client(&server);
    let result = client.fetch_readme("group/project").await;

    assert!(result.is_err());
    let msg = result.unwrap_err().to_string();
    assert!(
        msg.contains("authentication failed"),
        "expected auth error, got: {msg}"
    );
}

#[tokio::test]
async fn test_fetch_readme_nested_namespace() {
    // FR-022: nested namespace URL-encoded in the readme endpoint path.
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/api/v4/projects/group%2Fsub%2Fproject/readme"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "readme_url": format!("{}/raw/README.md", server.uri()),
        })))
        .mount(&server)
        .await;

    Mock::given(method("GET"))
        .and(path("/raw/README.md"))
        .respond_with(ResponseTemplate::new(200).set_body_string("# Nested"))
        .mount(&server)
        .await;

    let client = client(&server);
    let readme = client.fetch_readme("group/sub/project").await.unwrap();

    assert_eq!(readme, Some("# Nested".to_string()));
}

#[tokio::test]
async fn test_fetch_readme_500_propagates_error() {
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/api/v4/projects/group%2Fproject/readme"))
        .respond_with(ResponseTemplate::new(500).set_body_string("Internal Server Error"))
        .mount(&server)
        .await;

    let client = client(&server);
    let result = client.fetch_readme("group/project").await;

    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("500"));
}
