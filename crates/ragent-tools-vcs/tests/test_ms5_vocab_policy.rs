//! ANTIPAT.md M5.1 - cross-provider vocabulary and HTTP policy tests.
//!
//! Covers the `state` normalisation helpers (I-2) so both GitHub's `open` and
//! GitLab's `opened` spellings are accepted on every state-bearing tool, and
//! the GitHub 403/429 classification (I-5) so a genuine permission denial is no
//! longer mis-reported as a rate limit.

use ragent_tools_vcs::github::GitHubClient;
use ragent_tools_vcs::vocab::{
    normalize_gitlab_issue_state, normalize_gitlab_state, normalize_issue_state,
};
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

// ---------------------------------------------------------------------------
// I-2 - state normalisation accepts both spellings
// ---------------------------------------------------------------------------

#[test]
fn test_github_state_accepts_both_open_spellings() {
    assert_eq!(normalize_issue_state("open"), "open");
    assert_eq!(normalize_issue_state("opened"), "open");
    assert_eq!(normalize_issue_state("closed"), "closed");
    assert_eq!(normalize_issue_state("all"), "all");
}

#[test]
fn test_github_state_maps_merged_to_open() {
    // GitHub has no `merged` issue/PR state; it must not reach the API verbatim.
    assert_eq!(normalize_issue_state("merged"), "open");
}

#[test]
fn test_gitlab_state_accepts_both_open_spellings() {
    assert_eq!(normalize_gitlab_state("opened"), "opened");
    assert_eq!(normalize_gitlab_state("open"), "opened");
    assert_eq!(normalize_gitlab_state("closed"), "closed");
    assert_eq!(normalize_gitlab_state("merged"), "merged");
    assert_eq!(normalize_gitlab_state("all"), "all");
}

#[test]
fn test_gitlab_issue_state_maps_merged_to_opened() {
    // GitLab's issues endpoint rejects `merged`; it maps to `opened`.
    assert_eq!(normalize_gitlab_issue_state("merged"), "opened");
    assert_eq!(normalize_gitlab_issue_state("open"), "opened");
    assert_eq!(normalize_gitlab_issue_state("closed"), "closed");
    assert_eq!(normalize_gitlab_issue_state("all"), "all");
}

// ---------------------------------------------------------------------------
// I-5 - 403 classification: rate limit only when the quota is exhausted
// ---------------------------------------------------------------------------

fn client(server: &MockServer) -> GitHubClient {
    GitHubClient::with_base_url(server.uri(), "test-token".to_string())
}

#[tokio::test]
async fn test_403_without_remaining_header_is_permission_denied() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/user"))
        .respond_with(ResponseTemplate::new(403).set_body_string("Forbidden"))
        .mount(&server)
        .await;

    let err = client(&server)
        .get("/user")
        .await
        .expect_err("403 must be an error");
    let msg = err.to_string();
    assert!(
        msg.contains("permission denied"),
        "403 without an exhausted rate limit must be reported as permission denied, got: {msg}"
    );
    assert!(
        !msg.contains("rate limit"),
        "403 without x-ratelimit-remaining: 0 must not claim a rate limit, got: {msg}"
    );
}

#[tokio::test]
async fn test_403_with_zero_remaining_is_rate_limited() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/user"))
        .respond_with(
            ResponseTemplate::new(403)
                .insert_header("x-ratelimit-remaining", "0")
                .insert_header("x-ratelimit-reset", "1787366400"),
        )
        .mount(&server)
        .await;

    let err = client(&server)
        .get("/user")
        .await
        .expect_err("403 must be an error");
    let msg = err.to_string();
    assert!(
        msg.contains("rate limit"),
        "403 with x-ratelimit-remaining: 0 is a rate limit, got: {msg}"
    );
}

#[tokio::test]
async fn test_429_is_always_rate_limited() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/user"))
        .respond_with(ResponseTemplate::new(429))
        .mount(&server)
        .await;

    let err = client(&server)
        .get("/user")
        .await
        .expect_err("429 must be an error");
    assert!(
        err.to_string().contains("rate limit"),
        "429 must be a rate limit, got: {err}"
    );
}
