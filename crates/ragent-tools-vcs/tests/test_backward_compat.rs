//! Backward-compatibility tests for `parse_reverse_repo` (FR-005, FR-006).
//!
//! These tests verify that bare `owner/repo` identifiers and bare GitHub URLs
//! (no provider prefix) still route to the GitHub API, preserving backward
//! compatibility with existing `/reverse` usage before the GitLab support was
//! added.

use ragent_tools_vcs::vcs_provider::{VcsProvider, parse_reverse_repo};

// ---------------------------------------------------------------------------
// FR-005: bare owner/repo → GitHub
// ---------------------------------------------------------------------------

#[test]
fn test_backward_compat_bare_owner_repo() {
    let provider = parse_reverse_repo("octocat/Hello-World").unwrap();
    assert_eq!(
        provider,
        VcsProvider::GitHub {
            owner: "octocat".to_string(),
            repo: "Hello-World".to_string()
        }
    );
}

#[test]
fn test_backward_compat_bare_owner_repo_with_hyphens() {
    let provider = parse_reverse_repo("my-org/my-repo-name").unwrap();
    assert_eq!(
        provider,
        VcsProvider::GitHub {
            owner: "my-org".to_string(),
            repo: "my-repo-name".to_string()
        }
    );
}

#[test]
fn test_backward_compat_bare_owner_repo_with_numbers() {
    let provider = parse_reverse_repo("user123/repo456").unwrap();
    assert_eq!(
        provider,
        VcsProvider::GitHub {
            owner: "user123".to_string(),
            repo: "repo456".to_string()
        }
    );
}

#[test]
fn test_backward_compat_bare_owner_repo_with_dots() {
    let provider = parse_reverse_repo("user.name/repo.io").unwrap();
    assert_eq!(
        provider,
        VcsProvider::GitHub {
            owner: "user.name".to_string(),
            repo: "repo.io".to_string()
        }
    );
}

#[test]
fn test_backward_compat_bare_owner_repo_trailing_slash() {
    let provider = parse_reverse_repo("octocat/Hello-World/").unwrap();
    assert_eq!(
        provider,
        VcsProvider::GitHub {
            owner: "octocat".to_string(),
            repo: "Hello-World".to_string()
        }
    );
}

#[test]
fn test_backward_compat_bare_owner_repo_with_git_suffix() {
    let provider = parse_reverse_repo("octocat/Hello-World.git").unwrap();
    assert_eq!(
        provider,
        VcsProvider::GitHub {
            owner: "octocat".to_string(),
            repo: "Hello-World".to_string()
        }
    );
}

#[test]
fn test_backward_compat_bare_three_segments_rejected() {
    // GitHub repos are always exactly owner/repo (2 segments).
    assert!(parse_reverse_repo("owner/repo/extra").is_err());
}

// ---------------------------------------------------------------------------
// FR-006: bare GitHub HTTPS URL → GitHub
// ---------------------------------------------------------------------------

#[test]
fn test_backward_compat_github_https_url() {
    let provider = parse_reverse_repo("https://github.com/octocat/Hello-World").unwrap();
    assert_eq!(
        provider,
        VcsProvider::GitHub {
            owner: "octocat".to_string(),
            repo: "Hello-World".to_string()
        }
    );
}

#[test]
fn test_backward_compat_github_https_url_with_trailing_slash() {
    let provider = parse_reverse_repo("https://github.com/octocat/Hello-World/").unwrap();
    assert_eq!(
        provider,
        VcsProvider::GitHub {
            owner: "octocat".to_string(),
            repo: "Hello-World".to_string()
        }
    );
}

#[test]
fn test_backward_compat_github_https_url_with_git_suffix() {
    let provider = parse_reverse_repo("https://github.com/octocat/Hello-World.git").unwrap();
    assert_eq!(
        provider,
        VcsProvider::GitHub {
            owner: "octocat".to_string(),
            repo: "Hello-World".to_string()
        }
    );
}

#[test]
fn test_backward_compat_github_https_url_with_query_string() {
    let provider = parse_reverse_repo("https://github.com/octocat/Hello-World?tab=readme").unwrap();
    assert_eq!(
        provider,
        VcsProvider::GitHub {
            owner: "octocat".to_string(),
            repo: "Hello-World".to_string()
        }
    );
}

#[test]
fn test_backward_compat_github_http_url() {
    let provider = parse_reverse_repo("http://github.com/octocat/Hello-World").unwrap();
    assert_eq!(
        provider,
        VcsProvider::GitHub {
            owner: "octocat".to_string(),
            repo: "Hello-World".to_string()
        }
    );
}

// ---------------------------------------------------------------------------
// FR-006: bare GitHub SSH URL → GitHub
// ---------------------------------------------------------------------------

#[test]
fn test_backward_compat_github_ssh_url() {
    let provider = parse_reverse_repo("git@github.com:octocat/Hello-World.git").unwrap();
    assert_eq!(
        provider,
        VcsProvider::GitHub {
            owner: "octocat".to_string(),
            repo: "Hello-World".to_string()
        }
    );
}

#[test]
fn test_backward_compat_github_ssh_url_without_git_suffix() {
    let provider = parse_reverse_repo("git@github.com:octocat/Hello-World").unwrap();
    assert_eq!(
        provider,
        VcsProvider::GitHub {
            owner: "octocat".to_string(),
            repo: "Hello-World".to_string()
        }
    );
}

// ---------------------------------------------------------------------------
// FR-005/FR-006: routing correctness — bare identifiers never route to GitLab
// ---------------------------------------------------------------------------

#[test]
fn test_backward_compat_bare_repo_routes_to_github_not_gitlab() {
    let provider = parse_reverse_repo("octocat/Hello-World").unwrap();
    assert!(
        matches!(provider, VcsProvider::GitHub { .. }),
        "bare owner/repo must route to GitHub, not GitLab"
    );
}

#[test]
fn test_backward_compat_github_url_routes_to_github_not_gitlab() {
    let provider = parse_reverse_repo("https://github.com/octocat/Hello-World").unwrap();
    assert!(
        matches!(provider, VcsProvider::GitHub { .. }),
        "github.com URL must route to GitHub, not GitLab"
    );
}

#[test]
fn test_backward_compat_github_ssh_routes_to_github_not_gitlab() {
    let provider = parse_reverse_repo("git@github.com:octocat/Hello-World.git").unwrap();
    assert!(
        matches!(provider, VcsProvider::GitHub { .. }),
        "git@github.com SSH URL must route to GitHub, not GitLab"
    );
}
