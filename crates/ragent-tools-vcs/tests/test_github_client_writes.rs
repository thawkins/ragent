//! Regression tests for FUNC-062: GitHub write verbs (`post`/`put`/`patch`) must
//! honour the configured `base_url` instead of hardcoding `api.github.com`, so a
//! GitHub Enterprise host or a local mock server is routed to correctly.
//!
//! They exercise the client against a local `wiremock` server via
//! [`GitHubClient::with_base_url`].

use ragent_tools_vcs::github::GitHubClient;
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

fn client(server: &MockServer) -> GitHubClient {
    GitHubClient::with_base_url(server.uri(), "test-token".to_string())
}

#[tokio::test]
async fn test_post_routes_to_base_url() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/repos/octocat/Hello-World/issues"))
        .respond_with(ResponseTemplate::new(201).set_body_json(serde_json::json!({"ok": true})))
        .mount(&server)
        .await;

    let client = client(&server);
    let result = client
        .post(
            "/repos/octocat/Hello-World/issues",
            &serde_json::json!({"title": "hi"}),
        )
        .await;

    assert!(
        result.is_ok(),
        "POST should reach the configured base_url, got: {result:?}"
    );
}

#[tokio::test]
async fn test_put_routes_to_base_url() {
    let server = MockServer::start().await;
    Mock::given(method("PUT"))
        .and(path("/repos/octocat/Hello-World/pulls/1/merge"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({"merged": true})))
        .mount(&server)
        .await;

    let client = client(&server);
    let result = client
        .put(
            "/repos/octocat/Hello-World/pulls/1/merge",
            &serde_json::json!({"merge_method": "merge"}),
        )
        .await;

    assert!(
        result.is_ok(),
        "PUT should reach the configured base_url, got: {result:?}"
    );
}

#[tokio::test]
async fn test_patch_routes_to_base_url() {
    let server = MockServer::start().await;
    Mock::given(method("PATCH"))
        .and(path("/repos/octocat/Hello-World/issues/1"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(serde_json::json!({"state": "closed"})),
        )
        .mount(&server)
        .await;

    let client = client(&server);
    let result = client
        .patch(
            "/repos/octocat/Hello-World/issues/1",
            &serde_json::json!({"state": "closed"}),
        )
        .await;

    assert!(
        result.is_ok(),
        "PATCH should reach the configured base_url, got: {result:?}"
    );
}
