//! Unit tests for `GitHubClient::parse_repo_url` (FR-003, FR-016, NFR-002).
//!
//! Covers all three accepted input forms (full HTTPS URL, SSH URL, shorthand),
//! normalisation (`.git` suffix, trailing slash, query string, fragment,
//! whitespace), and invalid-input rejection (empty, single word, three-segment,
//! bare hostname, missing repo segment).

use ragent_tools_vcs::github::GitHubClient;

// ---------------------------------------------------------------------------
// Full HTTPS URL form: https://github.com/owner/repo
// ---------------------------------------------------------------------------

#[test]
fn test_parse_https_url_basic() {
    let result = GitHubClient::parse_repo_url("https://github.com/octocat/Hello-World");
    assert_eq!(
        result,
        Some(("octocat".to_string(), "Hello-World".to_string()))
    );
}

#[test]
fn test_parse_http_url_basic() {
    let result = GitHubClient::parse_repo_url("http://github.com/octocat/Hello-World");
    assert_eq!(
        result,
        Some(("octocat".to_string(), "Hello-World".to_string()))
    );
}

#[test]
fn test_parse_https_url_with_trailing_slash() {
    let result = GitHubClient::parse_repo_url("https://github.com/octocat/Hello-World/");
    assert_eq!(
        result,
        Some(("octocat".to_string(), "Hello-World".to_string()))
    );
}

#[test]
fn test_parse_https_url_with_git_suffix() {
    let result = GitHubClient::parse_repo_url("https://github.com/octocat/Hello-World.git");
    assert_eq!(
        result,
        Some(("octocat".to_string(), "Hello-World".to_string()))
    );
}

#[test]
fn test_parse_https_url_with_git_suffix_and_trailing_slash() {
    let result = GitHubClient::parse_repo_url("https://github.com/octocat/Hello-World.git/");
    assert_eq!(
        result,
        Some(("octocat".to_string(), "Hello-World".to_string()))
    );
}

#[test]
fn test_parse_https_url_with_query_string() {
    let result = GitHubClient::parse_repo_url("https://github.com/octocat/Hello-World?tab=readme");
    assert_eq!(
        result,
        Some(("octocat".to_string(), "Hello-World".to_string()))
    );
}

#[test]
fn test_parse_https_url_with_fragment() {
    let result = GitHubClient::parse_repo_url("https://github.com/octocat/Hello-World#section");
    assert_eq!(
        result,
        Some(("octocat".to_string(), "Hello-World".to_string()))
    );
}

#[test]
fn test_parse_https_url_with_query_and_fragment() {
    let result =
        GitHubClient::parse_repo_url("https://github.com/octocat/Hello-World?tab=readme#top");
    assert_eq!(
        result,
        Some(("octocat".to_string(), "Hello-World".to_string()))
    );
}

#[test]
fn test_parse_https_url_with_git_suffix_and_query() {
    let result =
        GitHubClient::parse_repo_url("https://github.com/octocat/Hello-World.git?ref=main");
    assert_eq!(
        result,
        Some(("octocat".to_string(), "Hello-World".to_string()))
    );
}

#[test]
fn test_parse_https_url_with_leading_trailing_whitespace() {
    let result = GitHubClient::parse_repo_url("  https://github.com/octocat/Hello-World  ");
    assert_eq!(
        result,
        Some(("octocat".to_string(), "Hello-World".to_string()))
    );
}

// ---------------------------------------------------------------------------
// SSH URL form: git@github.com:owner/repo.git
// ---------------------------------------------------------------------------

#[test]
fn test_parse_ssh_url_basic() {
    let result = GitHubClient::parse_repo_url("git@github.com:octocat/Hello-World");
    assert_eq!(
        result,
        Some(("octocat".to_string(), "Hello-World".to_string()))
    );
}

#[test]
fn test_parse_ssh_url_with_git_suffix() {
    let result = GitHubClient::parse_repo_url("git@github.com:octocat/Hello-World.git");
    assert_eq!(
        result,
        Some(("octocat".to_string(), "Hello-World".to_string()))
    );
}

#[test]
fn test_parse_ssh_url_with_trailing_slash() {
    let result = GitHubClient::parse_repo_url("git@github.com:octocat/Hello-World/");
    assert_eq!(
        result,
        Some(("octocat".to_string(), "Hello-World".to_string()))
    );
}

#[test]
fn test_parse_ssh_url_with_git_suffix_and_trailing_slash() {
    let result = GitHubClient::parse_repo_url("git@github.com:octocat/Hello-World.git/");
    assert_eq!(
        result,
        Some(("octocat".to_string(), "Hello-World".to_string()))
    );
}

#[test]
fn test_parse_ssh_url_with_query_string() {
    let result = GitHubClient::parse_repo_url("git@github.com:octocat/Hello-World?tab=readme");
    assert_eq!(
        result,
        Some(("octocat".to_string(), "Hello-World".to_string()))
    );
}

// ---------------------------------------------------------------------------
// Shorthand form: owner/repo
// ---------------------------------------------------------------------------

#[test]
fn test_parse_shorthand_basic() {
    let result = GitHubClient::parse_repo_url("octocat/Hello-World");
    assert_eq!(
        result,
        Some(("octocat".to_string(), "Hello-World".to_string()))
    );
}

#[test]
fn test_parse_shorthand_with_trailing_slash() {
    let result = GitHubClient::parse_repo_url("octocat/Hello-World/");
    assert_eq!(
        result,
        Some(("octocat".to_string(), "Hello-World".to_string()))
    );
}

#[test]
fn test_parse_shorthand_with_git_suffix() {
    let result = GitHubClient::parse_repo_url("octocat/Hello-World.git");
    assert_eq!(
        result,
        Some(("octocat".to_string(), "Hello-World".to_string()))
    );
}

#[test]
fn test_parse_shorthand_with_git_suffix_and_trailing_slash() {
    let result = GitHubClient::parse_repo_url("octocat/Hello-World.git/");
    assert_eq!(
        result,
        Some(("octocat".to_string(), "Hello-World".to_string()))
    );
}

#[test]
fn test_parse_shorthand_with_whitespace() {
    let result = GitHubClient::parse_repo_url("  octocat/Hello-World  ");
    assert_eq!(
        result,
        Some(("octocat".to_string(), "Hello-World".to_string()))
    );
}

#[test]
fn test_parse_shorthand_with_query_string() {
    let result = GitHubClient::parse_repo_url("octocat/Hello-World?ref=main");
    assert_eq!(
        result,
        Some(("octocat".to_string(), "Hello-World".to_string()))
    );
}

#[test]
fn test_parse_shorthand_with_fragment() {
    let result = GitHubClient::parse_repo_url("octocat/Hello-World#section");
    assert_eq!(
        result,
        Some(("octocat".to_string(), "Hello-World".to_string()))
    );
}

// ---------------------------------------------------------------------------
// Generic github.com URL (e.g. with www or other subdomains)
// ---------------------------------------------------------------------------

#[test]
fn test_parse_www_github_com_url() {
    let result = GitHubClient::parse_repo_url("https://www.github.com/octocat/Hello-World");
    assert_eq!(
        result,
        Some(("octocat".to_string(), "Hello-World".to_string()))
    );
}

// ---------------------------------------------------------------------------
// Invalid inputs (FR-016) — all must return None
// ---------------------------------------------------------------------------

#[test]
fn test_parse_empty_string_returns_none() {
    assert!(GitHubClient::parse_repo_url("").is_none());
}

#[test]
fn test_parse_whitespace_only_returns_none() {
    assert!(GitHubClient::parse_repo_url("   ").is_none());
    assert!(GitHubClient::parse_repo_url("\t\n").is_none());
}

#[test]
fn test_parse_single_word_returns_none() {
    assert!(GitHubClient::parse_repo_url("justoneword").is_none());
}

#[test]
fn test_parse_single_word_with_at_prefix_returns_none() {
    // Not a valid SSH URL — no github.com host.
    assert!(GitHubClient::parse_repo_url("@justoneword").is_none());
}

#[test]
fn test_parse_three_segment_shorthand_returns_none() {
    assert!(GitHubClient::parse_repo_url("owner/repo/extra").is_none());
}

#[test]
fn test_parse_three_segment_url_returns_none() {
    assert!(GitHubClient::parse_repo_url("https://github.com/owner/repo/extra").is_none());
}

#[test]
fn test_parse_three_segment_ssh_returns_none() {
    assert!(GitHubClient::parse_repo_url("git@github.com:owner/repo/extra.git").is_none());
}

#[test]
fn test_parse_bare_hostname_returns_none() {
    assert!(GitHubClient::parse_repo_url("https://github.com").is_none());
    assert!(GitHubClient::parse_repo_url("https://github.com/").is_none());
}

#[test]
fn test_parse_bare_hostname_with_query_returns_none() {
    assert!(GitHubClient::parse_repo_url("https://github.com?tab=repositories").is_none());
}

#[test]
fn test_parse_owner_only_url_returns_none() {
    // https://github.com/owner — only one path segment.
    assert!(GitHubClient::parse_repo_url("https://github.com/octocat").is_none());
    assert!(GitHubClient::parse_repo_url("https://github.com/octocat/").is_none());
}

#[test]
fn test_parse_shorthand_owner_only_returns_none() {
    assert!(GitHubClient::parse_repo_url("octocat").is_none());
    assert!(GitHubClient::parse_repo_url("octocat/").is_none());
    assert!(GitHubClient::parse_repo_url("/Hello-World").is_none());
}

#[test]
fn test_parse_non_github_url_returns_none() {
    // A URL on a different host with only two segments would parse as
    // shorthand if it has no github.com marker — but with github.com absent
    // and no slash, it's a single word.
    assert!(GitHubClient::parse_repo_url("https://gitlab.com/octocat/Hello-World").is_none());
}

// ---------------------------------------------------------------------------
// Edge cases
// ---------------------------------------------------------------------------

#[test]
fn test_parse_preserves_case() {
    let result = GitHubClient::parse_repo_url("https://github.com/Org/My-Repo");
    assert_eq!(result, Some(("Org".to_string(), "My-Repo".to_string())));
}

#[test]
fn test_parse_dashes_and_dots_in_names() {
    let result = GitHubClient::parse_repo_url("my-org/my.repo.name");
    assert_eq!(
        result,
        Some(("my-org".to_string(), "my.repo.name".to_string()))
    );
}

#[test]
fn test_parse_numeric_names() {
    let result = GitHubClient::parse_repo_url("123/456");
    assert_eq!(result, Some(("123".to_string(), "456".to_string())));
}

#[test]
fn test_parse_multiple_trailing_slashes() {
    let result = GitHubClient::parse_repo_url("https://github.com/octocat/Hello-World///");
    assert_eq!(
        result,
        Some(("octocat".to_string(), "Hello-World".to_string()))
    );
}

#[test]
fn test_parse_multiple_query_params_stripped() {
    let result = GitHubClient::parse_repo_url("https://github.com/octocat/Hello-World?a=1&b=2&c=3");
    assert_eq!(
        result,
        Some(("octocat".to_string(), "Hello-World".to_string()))
    );
}
