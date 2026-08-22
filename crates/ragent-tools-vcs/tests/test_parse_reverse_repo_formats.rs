//! Comprehensive unit tests for `parse_reverse_repo` covering all supported
//! input formats (FR-012, FR-013).
//!
//! This file consolidates format-coverage tests into a single location so
//! that any regression in the parser is immediately visible. Individual
//! edge-case tests also live inline in `vcs_provider.rs`, but this file
//! provides a structured matrix of every format combination.

use ragent_tools_vcs::vcs_provider::{VcsProvider, parse_reverse_repo};

// ===========================================================================
// FR-012: Every supported format parses into the correct VcsProvider
// ===========================================================================

// --- GitHub: bare owner/repo ----------------------------------------------

#[test]
fn fr012_github_bare_owner_repo() {
    let p = parse_reverse_repo("octocat/Hello-World").unwrap();
    assert_eq!(
        p,
        VcsProvider::GitHub {
            owner: "octocat".into(),
            repo: "Hello-World".into()
        }
    );
}

#[test]
fn fr012_github_bare_with_underscores() {
    let p = parse_reverse_repo("my_org/my_repo").unwrap();
    assert_eq!(
        p,
        VcsProvider::GitHub {
            owner: "my_org".into(),
            repo: "my_repo".into()
        }
    );
}

#[test]
fn fr012_github_bare_trailing_slash() {
    let p = parse_reverse_repo("octocat/Hello-World/").unwrap();
    assert_eq!(
        p,
        VcsProvider::GitHub {
            owner: "octocat".into(),
            repo: "Hello-World".into()
        }
    );
}

#[test]
fn fr012_github_bare_with_git_suffix() {
    let p = parse_reverse_repo("octocat/Hello-World.git").unwrap();
    assert_eq!(
        p,
        VcsProvider::GitHub {
            owner: "octocat".into(),
            repo: "Hello-World".into()
        }
    );
}

// --- GitHub: prefix --------------------------------------------------------

#[test]
fn fr012_github_prefix_bare() {
    let p = parse_reverse_repo("github:octocat/Hello-World").unwrap();
    assert_eq!(
        p,
        VcsProvider::GitHub {
            owner: "octocat".into(),
            repo: "Hello-World".into()
        }
    );
}

#[test]
fn fr012_github_prefix_https_url() {
    let p = parse_reverse_repo("github:https://github.com/octocat/Hello-World").unwrap();
    assert_eq!(
        p,
        VcsProvider::GitHub {
            owner: "octocat".into(),
            repo: "Hello-World".into()
        }
    );
}

#[test]
fn fr012_github_prefix_ssh_url() {
    let p = parse_reverse_repo("github:git@github.com:octocat/Hello-World.git").unwrap();
    assert_eq!(
        p,
        VcsProvider::GitHub {
            owner: "octocat".into(),
            repo: "Hello-World".into()
        }
    );
}

// --- GitHub: bare URLs -----------------------------------------------------

#[test]
fn fr012_github_https_url() {
    let p = parse_reverse_repo("https://github.com/octocat/Hello-World").unwrap();
    assert_eq!(
        p,
        VcsProvider::GitHub {
            owner: "octocat".into(),
            repo: "Hello-World".into()
        }
    );
}

#[test]
fn fr012_github_http_url() {
    let p = parse_reverse_repo("http://github.com/octocat/Hello-World").unwrap();
    assert_eq!(
        p,
        VcsProvider::GitHub {
            owner: "octocat".into(),
            repo: "Hello-World".into()
        }
    );
}

#[test]
fn fr012_github_ssh_url() {
    let p = parse_reverse_repo("git@github.com:octocat/Hello-World.git").unwrap();
    assert_eq!(
        p,
        VcsProvider::GitHub {
            owner: "octocat".into(),
            repo: "Hello-World".into()
        }
    );
}

#[test]
fn fr012_github_ssh_url_no_git_suffix() {
    let p = parse_reverse_repo("git@github.com:octocat/Hello-World").unwrap();
    assert_eq!(
        p,
        VcsProvider::GitHub {
            owner: "octocat".into(),
            repo: "Hello-World".into()
        }
    );
}

#[test]
fn fr012_github_url_with_query_string() {
    let p = parse_reverse_repo("https://github.com/octocat/Hello-World?tab=readme").unwrap();
    assert_eq!(
        p,
        VcsProvider::GitHub {
            owner: "octocat".into(),
            repo: "Hello-World".into()
        }
    );
}

#[test]
fn fr012_github_url_with_fragment() {
    let p = parse_reverse_repo("https://github.com/octocat/Hello-World#readme").unwrap();
    assert_eq!(
        p,
        VcsProvider::GitHub {
            owner: "octocat".into(),
            repo: "Hello-World".into()
        }
    );
}

// --- GitLab: prefix (configured instance) ---------------------------------

#[test]
fn fr012_gitlab_prefix_simple() {
    let p = parse_reverse_repo("gitlab:group/project").unwrap();
    assert_eq!(
        p,
        VcsProvider::GitLab {
            host: None,
            project_path: "group/project".into()
        }
    );
}

#[test]
fn fr012_gitlab_prefix_nested_2levels() {
    let p = parse_reverse_repo("gitlab:group/subgroup/project").unwrap();
    assert_eq!(
        p,
        VcsProvider::GitLab {
            host: None,
            project_path: "group/subgroup/project".into()
        }
    );
}

#[test]
fn fr012_gitlab_prefix_nested_4levels() {
    let p = parse_reverse_repo("gitlab:a/b/c/d/project").unwrap();
    assert_eq!(
        p,
        VcsProvider::GitLab {
            host: None,
            project_path: "a/b/c/d/project".into()
        }
    );
}

#[test]
fn fr012_gitlab_prefix_with_git_suffix() {
    let p = parse_reverse_repo("gitlab:group/project.git").unwrap();
    assert_eq!(
        p,
        VcsProvider::GitLab {
            host: None,
            project_path: "group/project".into()
        }
    );
}

#[test]
fn fr012_gitlab_prefix_filters_empty_segments() {
    let p = parse_reverse_repo("gitlab:group//project").unwrap();
    assert_eq!(
        p,
        VcsProvider::GitLab {
            host: None,
            project_path: "group/project".into()
        }
    );
}

// --- GitLab: prefix (self-hosted) ------------------------------------------

#[test]
fn fr012_gitlab_prefix_self_hosted() {
    let p = parse_reverse_repo("gitlab:gitlab.example.com/group/project").unwrap();
    assert_eq!(
        p,
        VcsProvider::GitLab {
            host: Some("https://gitlab.example.com".into()),
            project_path: "group/project".into()
        }
    );
}

#[test]
fn fr012_gitlab_prefix_self_hosted_with_port() {
    let p = parse_reverse_repo("gitlab:gitlab.example.com:8443/group/project").unwrap();
    assert_eq!(
        p,
        VcsProvider::GitLab {
            host: Some("https://gitlab.example.com:8443".into()),
            project_path: "group/project".into()
        }
    );
}

#[test]
fn fr012_gitlab_prefix_self_hosted_ip() {
    let p = parse_reverse_repo("gitlab:10.0.0.1/group/project").unwrap();
    assert_eq!(
        p,
        VcsProvider::GitLab {
            host: Some("https://10.0.0.1".into()),
            project_path: "group/project".into()
        }
    );
}

#[test]
fn fr012_gitlab_prefix_self_hosted_nested() {
    let p = parse_reverse_repo("gitlab:gitlab.corp.com/a/b/c/project").unwrap();
    assert_eq!(
        p,
        VcsProvider::GitLab {
            host: Some("https://gitlab.corp.com".into()),
            project_path: "a/b/c/project".into()
        }
    );
}

// --- GitLab: bare HTTPS URL ------------------------------------------------

#[test]
fn fr012_gitlab_https_url() {
    let p = parse_reverse_repo("https://gitlab.com/group/project").unwrap();
    assert_eq!(
        p,
        VcsProvider::GitLab {
            host: Some("https://gitlab.com".into()),
            project_path: "group/project".into()
        }
    );
}

#[test]
fn fr012_gitlab_https_self_hosted_url() {
    let p = parse_reverse_repo("https://gitlab.example.com/group/project").unwrap();
    assert_eq!(
        p,
        VcsProvider::GitLab {
            host: Some("https://gitlab.example.com".into()),
            project_path: "group/project".into()
        }
    );
}

#[test]
fn fr012_gitlab_https_url_nested() {
    let p = parse_reverse_repo("https://gitlab.com/group/subgroup/project").unwrap();
    assert_eq!(
        p,
        VcsProvider::GitLab {
            host: Some("https://gitlab.com".into()),
            project_path: "group/subgroup/project".into()
        }
    );
}

#[test]
fn fr012_gitlab_https_url_with_git_suffix() {
    let p = parse_reverse_repo("https://gitlab.com/group/project.git").unwrap();
    assert_eq!(
        p,
        VcsProvider::GitLab {
            host: Some("https://gitlab.com".into()),
            project_path: "group/project".into()
        }
    );
}

#[test]
fn fr012_gitlab_https_url_with_port() {
    let p = parse_reverse_repo("https://gitlab.example.com:8443/group/project").unwrap();
    assert_eq!(
        p,
        VcsProvider::GitLab {
            host: Some("https://gitlab.example.com:8443".into()),
            project_path: "group/project".into()
        }
    );
}

// --- GitLab: bare SSH URL --------------------------------------------------

#[test]
fn fr012_gitlab_ssh_url() {
    let p = parse_reverse_repo("git@gitlab.com:group/project.git").unwrap();
    assert_eq!(
        p,
        VcsProvider::GitLab {
            host: Some("https://gitlab.com".into()),
            project_path: "group/project".into()
        }
    );
}

#[test]
fn fr012_gitlab_ssh_self_hosted() {
    let p = parse_reverse_repo("git@gitlab.corp.com:group/project.git").unwrap();
    assert_eq!(
        p,
        VcsProvider::GitLab {
            host: Some("https://gitlab.corp.com".into()),
            project_path: "group/project".into()
        }
    );
}

#[test]
fn fr012_gitlab_ssh_nested() {
    let p = parse_reverse_repo("git@gitlab.com:group/sub/project.git").unwrap();
    assert_eq!(
        p,
        VcsProvider::GitLab {
            host: Some("https://gitlab.com".into()),
            project_path: "group/sub/project".into()
        }
    );
}

#[test]
fn fr012_gitlab_ssh_without_git_suffix() {
    let p = parse_reverse_repo("git@gitlab.com:group/project").unwrap();
    assert_eq!(
        p,
        VcsProvider::GitLab {
            host: Some("https://gitlab.com".into()),
            project_path: "group/project".into()
        }
    );
}

// --- Whitespace trimming ---------------------------------------------------

#[test]
fn fr012_whitespace_trimmed() {
    let p = parse_reverse_repo("  octocat/Hello-World  ").unwrap();
    assert_eq!(
        p,
        VcsProvider::GitHub {
            owner: "octocat".into(),
            repo: "Hello-World".into()
        }
    );
}

#[test]
fn fr012_gitlab_prefix_whitespace_trimmed() {
    let p = parse_reverse_repo("  gitlab:group/project  ").unwrap();
    assert_eq!(
        p,
        VcsProvider::GitLab {
            host: None,
            project_path: "group/project".into()
        }
    );
}

// ===========================================================================
// FR-013: Invalid identifiers are rejected with a human-readable error
// ===========================================================================

#[test]
fn fr013_empty_rejected() {
    assert!(parse_reverse_repo("").is_err());
}

#[test]
fn fr013_whitespace_only_rejected() {
    assert!(parse_reverse_repo("   ").is_err());
}

#[test]
fn fr013_single_word_rejected() {
    assert!(parse_reverse_repo("justoneword").is_err());
}

#[test]
fn fr013_github_prefix_single_word_rejected() {
    assert!(parse_reverse_repo("github:justoneword").is_err());
}

#[test]
fn fr013_gitlab_prefix_single_segment_rejected() {
    assert!(parse_reverse_repo("gitlab:justoneword").is_err());
}

#[test]
fn fr013_gitlab_prefix_host_only_rejected() {
    assert!(parse_reverse_repo("gitlab:gitlab.example.com").is_err());
}

#[test]
fn fr013_gitlab_prefix_host_one_segment_rejected() {
    assert!(parse_reverse_repo("gitlab:gitlab.example.com/project").is_err());
}

#[test]
fn fr013_github_three_segments_rejected() {
    assert!(parse_reverse_repo("owner/repo/extra").is_err());
}

#[test]
fn fr013_gitlab_https_no_path_rejected() {
    assert!(parse_reverse_repo("https://gitlab.com").is_err());
    assert!(parse_reverse_repo("https://gitlab.com/").is_err());
}

#[test]
fn fr013_gitlab_https_single_segment_rejected() {
    assert!(parse_reverse_repo("https://gitlab.com/justproject").is_err());
}

#[test]
fn fr013_gitlab_ssh_no_path_rejected() {
    assert!(parse_reverse_repo("git@gitlab.com:").is_err());
    assert!(parse_reverse_repo("git@gitlab.com:justoneword").is_err());
}

#[test]
fn fr013_error_message_lists_all_formats() {
    let err = parse_reverse_repo("bad").unwrap_err();
    assert!(err.contains("owner/repo"));
    assert!(err.contains("github:"));
    assert!(err.contains("gitlab:"));
    assert!(err.contains("https://github.com"));
    assert!(err.contains("git@github.com"));
    assert!(err.contains("https://gitlab.com"));
    assert!(err.contains("git@<gitlab-host>"));
    assert!(err.contains("self-hosted"));
    assert!(err.contains("nested"));
    assert!(err.contains("/gitlab setup"));
}

// ===========================================================================
// Routing correctness — each format routes to the right provider
// ===========================================================================

#[test]
fn routing_bare_repo_to_github() {
    assert!(matches!(
        parse_reverse_repo("octocat/Hello-World").unwrap(),
        VcsProvider::GitHub { .. }
    ));
}

#[test]
fn routing_github_url_to_github() {
    assert!(matches!(
        parse_reverse_repo("https://github.com/octocat/Hello-World").unwrap(),
        VcsProvider::GitHub { .. }
    ));
}

#[test]
fn routing_gitlab_prefix_to_gitlab() {
    assert!(matches!(
        parse_reverse_repo("gitlab:group/project").unwrap(),
        VcsProvider::GitLab { .. }
    ));
}

#[test]
fn routing_gitlab_url_to_gitlab() {
    assert!(matches!(
        parse_reverse_repo("https://gitlab.com/group/project").unwrap(),
        VcsProvider::GitLab { .. }
    ));
}

#[test]
fn routing_gitlab_ssh_to_gitlab() {
    assert!(matches!(
        parse_reverse_repo("git@gitlab.com:group/project.git").unwrap(),
        VcsProvider::GitLab { .. }
    ));
}

#[test]
fn routing_github_prefix_to_github() {
    assert!(matches!(
        parse_reverse_repo("github:octocat/Hello-World").unwrap(),
        VcsProvider::GitHub { .. }
    ));
}
