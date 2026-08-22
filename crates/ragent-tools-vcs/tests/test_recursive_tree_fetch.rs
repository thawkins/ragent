//! Unit tests for recursive tree fetch with mocked directory structures
//! (FR-027, FR-028).
//!
//! These tests exercise `GitHubClient::fetch_tree_recursive` and
//! `GitLabClient::fetch_repository_tree_recursive` against local mock HTTP
//! servers (via `wiremock`). They verify:
//!
//! - FR-027: directories are recursively expanded up to the requested depth.
//! - FR-028: directory entries get a trailing `/`; paths use `/` separators.

use ragent_tools_vcs::gitlab::GitLabClient;
use wiremock::matchers::{method, path, query_param};
use wiremock::{Mock, MockServer, ResponseTemplate};

const TOKEN: &str = "test-token";

// ===========================================================================
// GitHub: fetch_tree_recursive
// ===========================================================================

fn github_client(server: &MockServer) -> ragent_tools_vcs::github::GitHubClient {
    ragent_tools_vcs::github::GitHubClient::with_base_url(server.uri(), TOKEN.to_string())
}

#[tokio::test]
async fn test_github_recursive_depth_1_dirs_get_trailing_slash() {
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/repos/owner/repo/contents"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!([
            {"name": "README.md", "type": "file"},
            {"name": "src", "type": "dir"},
            {"name": "Cargo.toml", "type": "file"}
        ])))
        .mount(&server)
        .await;

    let client = github_client(&server);
    let tree = client
        .fetch_tree_recursive("owner", "repo", 1)
        .await
        .unwrap();

    // FR-028: directories get trailing slash at depth 1.
    assert!(tree.contains(&"README.md".to_string()));
    assert!(tree.contains(&"src/".to_string()));
    assert!(tree.contains(&"Cargo.toml".to_string()));
}

#[tokio::test]
async fn test_github_recursive_depth_2_expands_directories() {
    let server = MockServer::start().await;

    // Root level.
    Mock::given(method("GET"))
        .and(path("/repos/owner/repo/contents"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!([
            {"name": "src", "type": "dir"},
            {"name": "README.md", "type": "file"}
        ])))
        .mount(&server)
        .await;

    // src/ contents.
    Mock::given(method("GET"))
        .and(path("/repos/owner/repo/contents/src"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!([
            {"name": "main.rs", "type": "file"},
            {"name": "models", "type": "dir"}
        ])))
        .mount(&server)
        .await;

    let client = github_client(&server);
    let tree = client
        .fetch_tree_recursive("owner", "repo", 2)
        .await
        .unwrap();

    // FR-027: src/ is expanded.
    // FR-028: paths use separators, dirs get trailing slash.
    assert!(tree.contains(&"src/".to_string()));
    assert!(tree.contains(&"src/main.rs".to_string()));
    assert!(tree.contains(&"src/models/".to_string()));
    assert!(tree.contains(&"README.md".to_string()));
    // models/ is NOT expanded (depth 2 stops at src level).
    assert!(!tree.contains(&"src/models/user.rs".to_string()));
}

#[tokio::test]
async fn test_github_recursive_depth_3_deep_nesting() {
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/repos/owner/repo/contents"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!([
            {"name": "src", "type": "dir"}
        ])))
        .mount(&server)
        .await;

    Mock::given(method("GET"))
        .and(path("/repos/owner/repo/contents/src"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!([
            {"name": "models", "type": "dir"}
        ])))
        .mount(&server)
        .await;

    Mock::given(method("GET"))
        .and(path("/repos/owner/repo/contents/src/models"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!([
            {"name": "user.rs", "type": "file"},
            {"name": "order.rs", "type": "file"}
        ])))
        .mount(&server)
        .await;

    let client = github_client(&server);
    let tree = client
        .fetch_tree_recursive("owner", "repo", 3)
        .await
        .unwrap();

    assert!(tree.contains(&"src/".to_string()));
    assert!(tree.contains(&"src/models/".to_string()));
    assert!(tree.contains(&"src/models/user.rs".to_string()));
    assert!(tree.contains(&"src/models/order.rs".to_string()));
}

#[tokio::test]
async fn test_github_recursive_empty_directory() {
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/repos/owner/repo/contents"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!([])))
        .mount(&server)
        .await;

    let client = github_client(&server);
    let tree = client
        .fetch_tree_recursive("owner", "repo", 3)
        .await
        .unwrap();
    assert!(tree.is_empty());
}

#[tokio::test]
async fn test_github_recursive_only_files_no_dirs() {
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/repos/owner/repo/contents"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!([
            {"name": "file1.rs", "type": "file"},
            {"name": "file2.rs", "type": "file"}
        ])))
        .mount(&server)
        .await;

    let client = github_client(&server);
    let tree = client
        .fetch_tree_recursive("owner", "repo", 5)
        .await
        .unwrap();
    assert_eq!(tree, vec!["file1.rs".to_string(), "file2.rs".to_string()]);
}

#[tokio::test]
async fn test_github_recursive_dir_fetch_failure_tolerated() {
    let server = MockServer::start().await;

    // Root succeeds.
    Mock::given(method("GET"))
        .and(path("/repos/owner/repo/contents"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!([
            {"name": "src", "type": "dir"},
            {"name": "README.md", "type": "file"}
        ])))
        .mount(&server)
        .await;

    // src/ fails with 500 — should be tolerated.
    Mock::given(method("GET"))
        .and(path("/repos/owner/repo/contents/src"))
        .respond_with(ResponseTemplate::new(500).set_body_string("error"))
        .mount(&server)
        .await;

    let client = github_client(&server);
    let tree = client
        .fetch_tree_recursive("owner", "repo", 3)
        .await
        .unwrap();

    // src/ is listed but its children are omitted.
    assert!(tree.contains(&"src/".to_string()));
    assert!(tree.contains(&"README.md".to_string()));
    // No strict children of src/ were fetched (the src/ fetch failed).
    assert!(tree.iter().all(|e| !e.starts_with("src/") || e == "src/"));
}

#[tokio::test]
async fn test_github_recursive_mixed_tree_structure() {
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/repos/owner/repo/contents"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!([
            {"name": ".gitignore", "type": "file"},
            {"name": "src", "type": "dir"},
            {"name": "tests", "type": "dir"},
            {"name": "Cargo.toml", "type": "file"},
            {"name": "README.md", "type": "file"}
        ])))
        .mount(&server)
        .await;

    Mock::given(method("GET"))
        .and(path("/repos/owner/repo/contents/src"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!([
            {"name": "main.rs", "type": "file"},
            {"name": "lib.rs", "type": "file"}
        ])))
        .mount(&server)
        .await;

    Mock::given(method("GET"))
        .and(path("/repos/owner/repo/contents/tests"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!([
            {"name": "integration.rs", "type": "file"}
        ])))
        .mount(&server)
        .await;

    let client = github_client(&server);
    let tree = client
        .fetch_tree_recursive("owner", "repo", 2)
        .await
        .unwrap();

    assert!(tree.contains(&".gitignore".to_string()));
    assert!(tree.contains(&"src/".to_string()));
    assert!(tree.contains(&"src/main.rs".to_string()));
    assert!(tree.contains(&"src/lib.rs".to_string()));
    assert!(tree.contains(&"tests/".to_string()));
    assert!(tree.contains(&"tests/integration.rs".to_string()));
    assert!(tree.contains(&"Cargo.toml".to_string()));
    assert!(tree.contains(&"README.md".to_string()));
}

// ===========================================================================
// GitLab: fetch_repository_tree_recursive
// ===========================================================================

fn gitlab_client(server: &MockServer) -> GitLabClient {
    GitLabClient::with_credentials(server.uri(), TOKEN.to_string())
}

#[tokio::test]
async fn test_gitlab_recursive_depth_1_dirs_get_trailing_slash() {
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/api/v4/projects/group%2Fproject/repository/tree"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!([
            {"name": "README.md", "type": "blob"},
            {"name": "src", "type": "tree"},
            {"name": "Cargo.toml", "type": "blob"}
        ])))
        .mount(&server)
        .await;

    let client = gitlab_client(&server);
    let tree = client
        .fetch_repository_tree_recursive("group/project", 1)
        .await
        .unwrap();

    assert!(tree.contains(&"README.md".to_string()));
    assert!(tree.contains(&"src/".to_string()));
    assert!(tree.contains(&"Cargo.toml".to_string()));
}

#[tokio::test]
async fn test_gitlab_recursive_depth_2_expands_directories() {
    let server = MockServer::start().await;

    // src/ contents — uses ?path= query param. Mounted BEFORE the root mock so
    // wiremock's first-match-wins ordering does not let the root mock (which
    // matches any query string) swallow the src/ request.
    Mock::given(method("GET"))
        .and(path("/api/v4/projects/group%2Fproject/repository/tree"))
        .and(query_param("path", "src"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!([
            {"name": "main.rs", "type": "blob"},
            {"name": "models", "type": "tree"}
        ])))
        .mount(&server)
        .await;

    // Root level — mounted last so it only matches the no-query root request.
    Mock::given(method("GET"))
        .and(path("/api/v4/projects/group%2Fproject/repository/tree"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!([
            {"name": "src", "type": "tree"},
            {"name": "README.md", "type": "blob"}
        ])))
        .mount(&server)
        .await;

    let client = gitlab_client(&server);
    let tree = client
        .fetch_repository_tree_recursive("group/project", 2)
        .await
        .unwrap();

    assert!(tree.contains(&"src/".to_string()));
    assert!(tree.contains(&"src/main.rs".to_string()));
    assert!(tree.contains(&"src/models/".to_string()));
    assert!(tree.contains(&"README.md".to_string()));
    // models/ NOT expanded at depth 2.
    assert!(!tree.contains(&"src/models/user.rs".to_string()));
}

#[tokio::test]
async fn test_gitlab_recursive_depth_3_deep_nesting() {
    let server = MockServer::start().await;

    // Deepest level first — query_param matching is exact, so the specific
    // mocks do not cross-match each other; the root mock (no query_param) is
    // mounted last so it only catches the no-query root request.
    Mock::given(method("GET"))
        .and(path("/api/v4/projects/group%2Fproject/repository/tree"))
        .and(query_param("path", "src/models"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!([
            {"name": "user.rs", "type": "blob"},
            {"name": "order.rs", "type": "blob"}
        ])))
        .mount(&server)
        .await;

    Mock::given(method("GET"))
        .and(path("/api/v4/projects/group%2Fproject/repository/tree"))
        .and(query_param("path", "src"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!([
            {"name": "models", "type": "tree"}
        ])))
        .mount(&server)
        .await;

    Mock::given(method("GET"))
        .and(path("/api/v4/projects/group%2Fproject/repository/tree"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!([
            {"name": "src", "type": "tree"}
        ])))
        .mount(&server)
        .await;

    let client = gitlab_client(&server);
    let tree = client
        .fetch_repository_tree_recursive("group/project", 3)
        .await
        .unwrap();

    assert!(tree.contains(&"src/".to_string()));
    assert!(tree.contains(&"src/models/".to_string()));
    assert!(tree.contains(&"src/models/user.rs".to_string()));
    assert!(tree.contains(&"src/models/order.rs".to_string()));
}

#[tokio::test]
async fn test_gitlab_recursive_empty_directory() {
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/api/v4/projects/group%2Fproject/repository/tree"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!([])))
        .mount(&server)
        .await;

    let client = gitlab_client(&server);
    let tree = client
        .fetch_repository_tree_recursive("group/project", 3)
        .await
        .unwrap();
    assert!(tree.is_empty());
}

#[tokio::test]
async fn test_gitlab_recursive_only_blobs_no_trees() {
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/api/v4/projects/group%2Fproject/repository/tree"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!([
            {"name": "file1.rs", "type": "blob"},
            {"name": "file2.rs", "type": "blob"}
        ])))
        .mount(&server)
        .await;

    let client = gitlab_client(&server);
    let tree = client
        .fetch_repository_tree_recursive("group/project", 5)
        .await
        .unwrap();
    assert_eq!(tree, vec!["file1.rs".to_string(), "file2.rs".to_string()]);
}

#[tokio::test]
async fn test_gitlab_recursive_dir_fetch_failure_tolerated() {
    let server = MockServer::start().await;

    // src/ fails with 500 — mounted first so it matches the ?path=src request.
    Mock::given(method("GET"))
        .and(path("/api/v4/projects/group%2Fproject/repository/tree"))
        .and(query_param("path", "src"))
        .respond_with(ResponseTemplate::new(500).set_body_string("error"))
        .mount(&server)
        .await;

    // Root succeeds — mounted last so it only matches the no-query root request.
    Mock::given(method("GET"))
        .and(path("/api/v4/projects/group%2Fproject/repository/tree"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!([
            {"name": "src", "type": "tree"},
            {"name": "README.md", "type": "blob"}
        ])))
        .mount(&server)
        .await;

    let client = gitlab_client(&server);
    let tree = client
        .fetch_repository_tree_recursive("group/project", 3)
        .await
        .unwrap();

    assert!(tree.contains(&"src/".to_string()));
    assert!(tree.contains(&"README.md".to_string()));
    // src/ is listed but no strict children were fetched (the src/ fetch failed).
    assert!(tree.iter().all(|e| !e.starts_with("src/") || e == "src/"));
}

#[tokio::test]
async fn test_gitlab_recursive_mixed_tree_structure() {
    let server = MockServer::start().await;

    // Specific (query_param) mocks first — exact query matching means they
    // only catch their own path; the root mock goes last.
    Mock::given(method("GET"))
        .and(path("/api/v4/projects/group%2Fproject/repository/tree"))
        .and(query_param("path", "src"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!([
            {"name": "main.rs", "type": "blob"},
            {"name": "lib.rs", "type": "blob"}
        ])))
        .mount(&server)
        .await;

    Mock::given(method("GET"))
        .and(path("/api/v4/projects/group%2Fproject/repository/tree"))
        .and(query_param("path", "tests"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!([
            {"name": "integration.rs", "type": "blob"}
        ])))
        .mount(&server)
        .await;

    Mock::given(method("GET"))
        .and(path("/api/v4/projects/group%2Fproject/repository/tree"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!([
            {"name": ".gitignore", "type": "blob"},
            {"name": "src", "type": "tree"},
            {"name": "tests", "type": "tree"},
            {"name": "Cargo.toml", "type": "blob"}
        ])))
        .mount(&server)
        .await;

    let client = gitlab_client(&server);
    let tree = client
        .fetch_repository_tree_recursive("group/project", 2)
        .await
        .unwrap();

    assert!(tree.contains(&".gitignore".to_string()));
    assert!(tree.contains(&"src/".to_string()));
    assert!(tree.contains(&"src/main.rs".to_string()));
    assert!(tree.contains(&"src/lib.rs".to_string()));
    assert!(tree.contains(&"tests/".to_string()));
    assert!(tree.contains(&"tests/integration.rs".to_string()));
    assert!(tree.contains(&"Cargo.toml".to_string()));
}

#[tokio::test]
async fn test_gitlab_recursive_nested_namespace_url_encoded() {
    // FR-022: nested namespace URL-encoded in recursive calls.
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path(
            "/api/v4/projects/group%2Fsub%2Fproject/repository/tree",
        ))
        .and(query_param("path", "src"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!([
            {"name": "main.rs", "type": "blob"}
        ])))
        .mount(&server)
        .await;

    Mock::given(method("GET"))
        .and(path(
            "/api/v4/projects/group%2Fsub%2Fproject/repository/tree",
        ))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!([
            {"name": "src", "type": "tree"}
        ])))
        .mount(&server)
        .await;

    let client = gitlab_client(&server);
    let tree = client
        .fetch_repository_tree_recursive("group/sub/project", 2)
        .await
        .unwrap();

    assert!(tree.contains(&"src/".to_string()));
    assert!(tree.contains(&"src/main.rs".to_string()));
}
