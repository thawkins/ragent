//! Tests for `GitHubClient::validate_repo_input` — the FR-016 invalid-input
//! rejection path. Covers the three named invalid cases (empty, single-word,
//! three-segment) plus valid inputs and confirms the error messages are
//! human-readable usage strings.

use ragent_tools_vcs::github::GitHubClient;

// --- Valid inputs (sanity checks that validation passes through to parse) ---

#[test]
fn test_validate_shorthand_ok() {
    let (owner, repo) = GitHubClient::validate_repo_input("octocat/Hello-World")
        .expect("valid shorthand should parse");
    assert_eq!(owner, "octocat");
    assert_eq!(repo, "Hello-World");
}

#[test]
fn test_validate_https_url_ok() {
    let (owner, repo) = GitHubClient::validate_repo_input("https://github.com/octocat/Hello-World")
        .expect("valid https URL should parse");
    assert_eq!(owner, "octocat");
    assert_eq!(repo, "Hello-World");
}

#[test]
fn test_validate_ssh_url_with_git_suffix_ok() {
    let (owner, repo) = GitHubClient::validate_repo_input("git@github.com:octocat/Hello-World.git")
        .expect("valid ssh URL should parse");
    assert_eq!(owner, "octocat");
    assert_eq!(repo, "Hello-World");
}

// --- FR-016 invalid-input rejection: empty ---

#[test]
fn test_validate_empty_string_rejected() {
    let err = GitHubClient::validate_repo_input("").expect_err("empty string must be rejected");
    assert!(
        err.contains("Invalid repository identifier"),
        "error should mention invalid identifier, got: {err}"
    );
    assert!(
        err.contains("Usage"),
        "error should include a usage hint, got: {err}"
    );
}

#[test]
fn test_validate_whitespace_only_rejected() {
    let err = GitHubClient::validate_repo_input("   ")
        .expect_err("whitespace-only input must be rejected");
    assert!(err.contains("Invalid repository identifier"));
}

// --- FR-016 invalid-input rejection: single word ---

#[test]
fn test_validate_single_word_rejected() {
    let err =
        GitHubClient::validate_repo_input("justoneword").expect_err("single word must be rejected");
    assert!(err.contains("Invalid repository identifier"));
    assert!(
        err.contains("justoneword"),
        "error should echo the bad input, got: {err}"
    );
}

#[test]
fn test_validate_single_word_url_without_path_rejected() {
    // A bare hostname with no owner/repo path.
    let err = GitHubClient::validate_repo_input("https://github.com")
        .expect_err("URL with no path segments must be rejected");
    assert!(err.contains("Invalid repository identifier"));
}

// --- FR-016 invalid-input rejection: three-segment identifier ---

#[test]
fn test_validate_three_segment_shorthand_rejected() {
    let err = GitHubClient::validate_repo_input("owner/repo/extra")
        .expect_err("three-segment shorthand must be rejected");
    assert!(err.contains("Invalid repository identifier"));
    assert!(
        err.contains("owner/repo/extra"),
        "error should echo the bad input, got: {err}"
    );
}

#[test]
fn test_validate_three_segment_url_rejected() {
    let err = GitHubClient::validate_repo_input("https://github.com/owner/repo/extra")
        .expect_err("three-segment URL must be rejected");
    assert!(err.contains("Invalid repository identifier"));
}

#[test]
fn test_validate_three_segment_ssh_rejected() {
    let err = GitHubClient::validate_repo_input("git@github.com:owner/repo/extra.git")
        .expect_err("three-segment ssh URL must be rejected");
    assert!(err.contains("Invalid repository identifier"));
}

// --- Edge cases ---

#[test]
fn test_validate_trailing_slash_still_ok() {
    // A trailing slash on a valid two-segment identifier is stripped, not rejected.
    let (owner, repo) = GitHubClient::validate_repo_input("octocat/Hello-World/")
        .expect("trailing slash on valid input should be stripped, not rejected");
    assert_eq!(owner, "octocat");
    assert_eq!(repo, "Hello-World");
}

#[test]
fn test_validate_query_string_stripped_ok() {
    let (owner, repo) =
        GitHubClient::validate_repo_input("https://github.com/octocat/Hello-World?tab=readme")
            .expect("query string on valid input should be stripped");
    assert_eq!(owner, "octocat");
    assert_eq!(repo, "Hello-World");
}

#[test]
fn test_validate_error_message_contains_two_segment_hint() {
    let err = GitHubClient::validate_repo_input("only-one")
        .expect_err("single segment should be rejected");
    assert!(
        err.contains("two non-empty path segments"),
        "error should explain the two-segment requirement, got: {err}"
    );
}
