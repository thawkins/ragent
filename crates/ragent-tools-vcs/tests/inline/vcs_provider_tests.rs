//! Inline tests for `vcs_provider.rs` - compiled inside the module tree
//! via the `#[path]` delegation left in the source file
//! (ANTIPAT M2 inline-test migration; no behaviour change).

use super::*;

// --- GitHub formats -----------------------------------------------------

#[test]
fn test_parse_bare_owner_repo_defaults_to_github() {
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
fn test_parse_github_prefix_bare() {
    let provider = parse_reverse_repo("github:octocat/Hello-World").unwrap();
    assert_eq!(
        provider,
        VcsProvider::GitHub {
            owner: "octocat".to_string(),
            repo: "Hello-World".to_string()
        }
    );
}

#[test]
fn test_parse_github_prefix_url() {
    let provider = parse_reverse_repo("github:https://github.com/octocat/Hello-World").unwrap();
    assert_eq!(
        provider,
        VcsProvider::GitHub {
            owner: "octocat".to_string(),
            repo: "Hello-World".to_string()
        }
    );
}

#[test]
fn test_parse_github_https_url() {
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
fn test_parse_github_ssh_url() {
    let provider = parse_reverse_repo("git@github.com:octocat/Hello-World.git").unwrap();
    assert_eq!(
        provider,
        VcsProvider::GitHub {
            owner: "octocat".to_string(),
            repo: "Hello-World".to_string()
        }
    );
}

// --- GitLab prefixed formats (FR-002, FR-003, FR-022) ------------------

#[test]
fn test_parse_gitlab_prefix_simple() {
    let provider = parse_reverse_repo("gitlab:group/project").unwrap();
    assert_eq!(
        provider,
        VcsProvider::GitLab {
            host: None,
            project_path: "group/project".to_string()
        }
    );
}

#[test]
fn test_parse_gitlab_prefix_host_none_means_configured_instance() {
    // FR-002: `gitlab:namespace/project` with no explicit host returns
    // host=None, meaning the dispatch layer resolves the configured
    // GitLab instance URL (defaulting to https://gitlab.com).
    let provider = parse_reverse_repo("gitlab:my-org/my-repo").unwrap();
    assert_eq!(
        provider,
        VcsProvider::GitLab {
            host: None,
            project_path: "my-org/my-repo".to_string()
        }
    );
}

#[test]
fn test_parse_gitlab_prefix_strips_git_suffix() {
    // The .git suffix should be stripped from the project path.
    let provider = parse_reverse_repo("gitlab:group/project.git").unwrap();
    assert_eq!(
        provider,
        VcsProvider::GitLab {
            host: None,
            project_path: "group/project".to_string()
        }
    );
}

#[test]
fn test_parse_gitlab_prefix_nested_strips_git_suffix() {
    let provider = parse_reverse_repo("gitlab:group/subgroup/project.git").unwrap();
    assert_eq!(
        provider,
        VcsProvider::GitLab {
            host: None,
            project_path: "group/subgroup/project".to_string()
        }
    );
}

#[test]
fn test_parse_gitlab_prefix_filters_empty_segments() {
    // Double slashes should produce empty segments that are filtered out.
    let provider = parse_reverse_repo("gitlab:group//project").unwrap();
    assert_eq!(
        provider,
        VcsProvider::GitLab {
            host: None,
            project_path: "group/project".to_string()
        }
    );
}

#[test]
fn test_parse_gitlab_prefix_preserves_full_nested_path() {
    // FR-022: the full group/subgroup/project path is preserved, not
    // just the last two segments.
    let provider = parse_reverse_repo("gitlab:a/b/c/project").unwrap();
    assert_eq!(
        provider,
        VcsProvider::GitLab {
            host: None,
            project_path: "a/b/c/project".to_string()
        }
    );
}

#[test]
fn test_parse_gitlab_prefix_trims_whitespace() {
    let provider = parse_reverse_repo("  gitlab:group/project  ").unwrap();
    assert_eq!(
        provider,
        VcsProvider::GitLab {
            host: None,
            project_path: "group/project".to_string()
        }
    );
}

#[test]
fn test_parse_gitlab_prefix_nested_namespace() {
    let provider = parse_reverse_repo("gitlab:group/subgroup/project").unwrap();
    assert_eq!(
        provider,
        VcsProvider::GitLab {
            host: None,
            project_path: "group/subgroup/project".to_string()
        }
    );
}

#[test]
fn test_parse_gitlab_prefix_deep_nested_namespace() {
    let provider = parse_reverse_repo("gitlab:a/b/c/d/project").unwrap();
    assert_eq!(
        provider,
        VcsProvider::GitLab {
            host: None,
            project_path: "a/b/c/d/project".to_string()
        }
    );
}

/// SEC-ragent-tools-vcs-004 (SECTASKS T-022): the self-hosted short form is
/// only accepted for a host that matches the configured instance (or the
/// fixed SaaS allowlist), so these tests set `GITLAB_URL` for the duration
/// of the parse. The environment is process-global, so the helper is used
/// under `serial`-style discipline: each test sets and clears it around one
/// parse.
/// Serialises the `GITLAB_URL` mutation across this binary's tests.
///
/// These tests run on multiple threads and all mutate the same process
/// environment variable, so without a lock one test's configured instance
/// can be observed by another test's parse (turning a trusted host into a
/// rejected one). The integration test binary in
/// `tests/test_parse_reverse_repo_formats.rs` holds its own lock only for
/// its own process, so this lock cannot be shared - the two binaries are
/// separate processes and do not contend.
static GITLAB_URL_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

fn parse_with_configured_gitlab(configured: &str, spec: &str) -> Result<VcsProvider, String> {
    let _lock = GITLAB_URL_LOCK
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let _guard = EnvVarGuard::set("GITLAB_URL", configured);
    parse_reverse_repo(spec)
}

/// Restores (or removes) an environment variable when dropped.
///
/// Needed because `std::env::set_var` is `unsafe` under edition 2024 and
/// this crate forbids `unsafe`; the guard encapsulates the single
/// test-only mutation and restores the previous value on every exit path.
struct EnvVarGuard {
    key: &'static str,
    previous: Option<String>,
}

impl EnvVarGuard {
    fn set(key: &'static str, value: &str) -> Self {
        let previous = std::env::var(key).ok();
        // `set_var` is unsafe in edition 2024; this helper is test-only and
        // the tests using it are single-threaded for this variable.
        // SAFETY: the caller holds the process-wide environment mutation
        // for the duration of the guard and restores it on drop.
        #[allow(unsafe_code)]
        unsafe {
            std::env::set_var(key, value);
        }
        Self { key, previous }
    }
}

impl Drop for EnvVarGuard {
    fn drop(&mut self) {
        // SAFETY: see `EnvVarGuard::set`.
        #[allow(unsafe_code)]
        unsafe {
            match self.previous.take() {
                Some(value) => std::env::set_var(self.key, value),
                None => std::env::remove_var(self.key),
            }
        }
    }
}

#[test]
fn test_parse_gitlab_prefix_self_hosted() {
    let provider = parse_with_configured_gitlab(
        "https://gitlab.example.com",
        "gitlab:gitlab.example.com/group/project",
    )
    .unwrap();
    assert_eq!(
        provider,
        VcsProvider::GitLab {
            host: Some("https://gitlab.example.com".to_string()),
            project_path: "group/project".to_string()
        }
    );
}

#[test]
fn test_parse_gitlab_prefix_self_hosted_nested() {
    let provider = parse_with_configured_gitlab(
        "https://gitlab.example.com",
        "gitlab:gitlab.example.com/group/subgroup/project",
    )
    .unwrap();
    assert_eq!(
        provider,
        VcsProvider::GitLab {
            host: Some("https://gitlab.example.com".to_string()),
            project_path: "group/subgroup/project".to_string()
        }
    );
}

#[test]
fn test_parse_gitlab_prefix_self_hosted_with_port() {
    // FR-003: self-hosted GitLab with a port number in the host.
    let provider = parse_with_configured_gitlab(
        "https://gitlab.example.com:8443",
        "gitlab:gitlab.example.com:8443/group/project",
    )
    .unwrap();
    assert_eq!(
        provider,
        VcsProvider::GitLab {
            host: Some("https://gitlab.example.com:8443".to_string()),
            project_path: "group/project".to_string()
        }
    );
}

#[test]
fn test_parse_gitlab_prefix_self_hosted_ip_address() {
    // FR-003: self-hosted GitLab with a bare IP address as the host.
    let provider =
        parse_with_configured_gitlab("https://10.0.0.1", "gitlab:10.0.0.1/group/project").unwrap();
    assert_eq!(
        provider,
        VcsProvider::GitLab {
            host: Some("https://10.0.0.1".to_string()),
            project_path: "group/project".to_string()
        }
    );
}

#[test]
fn test_parse_gitlab_prefix_self_hosted_localhost_with_port() {
    // FR-003: localhost with a port is detected as a host (the ':'
    // triggers looks_like_host).
    let provider = parse_with_configured_gitlab(
        "https://localhost:8080",
        "gitlab:localhost:8080/group/project",
    )
    .unwrap();
    assert_eq!(
        provider,
        VcsProvider::GitLab {
            host: Some("https://localhost:8080".to_string()),
            project_path: "group/project".to_string()
        }
    );
}

#[test]
fn test_parse_gitlab_prefix_self_hosted_host_gets_https_prefix() {
    // FR-003: the host is always prefixed with https://.
    let provider = parse_with_configured_gitlab(
        "https://gitlab.corp.com",
        "gitlab:gitlab.corp.com/team/repo",
    )
    .unwrap();
    let host = match provider {
        VcsProvider::GitLab { host, .. } => host.unwrap(),
        _ => panic!("expected GitLab provider"),
    };
    assert!(host.starts_with("https://"));
    assert!(!host.contains("http://"));
}

#[test]
fn test_parse_gitlab_prefix_self_hosted_with_nested_namespace() {
    // FR-003 + FR-022: self-hosted GitLab with a deeply nested namespace.
    let provider = parse_with_configured_gitlab(
        "https://gitlab.corp.com",
        "gitlab:gitlab.corp.com/a/b/c/project",
    )
    .unwrap();
    assert_eq!(
        provider,
        VcsProvider::GitLab {
            host: Some("https://gitlab.corp.com".to_string()),
            project_path: "a/b/c/project".to_string()
        }
    );
}

#[test]
fn test_parse_gitlab_prefix_self_hosted_strips_git_suffix() {
    let provider = parse_with_configured_gitlab(
        "https://gitlab.example.com",
        "gitlab:gitlab.example.com/group/project.git",
    )
    .unwrap();
    assert_eq!(
        provider,
        VcsProvider::GitLab {
            host: Some("https://gitlab.example.com".to_string()),
            project_path: "group/project".to_string()
        }
    );
}

#[test]
fn test_parse_gitlab_prefix_self_hosted_host_only_one_segment_rejected() {
    // FR-003: host + only one path segment is not enough for
    // namespace/project.
    assert!(
        parse_with_configured_gitlab(
            "https://gitlab.example.com",
            "gitlab:gitlab.example.com/project"
        )
        .is_err()
    );
}

// --- GitLab URL formats (FR-004) ----------------------------------------

#[test]
fn test_parse_gitlab_https_url() {
    let provider = parse_reverse_repo("https://gitlab.com/group/project").unwrap();
    assert_eq!(
        provider,
        VcsProvider::GitLab {
            host: Some("https://gitlab.com".to_string()),
            project_path: "group/project".to_string()
        }
    );
}

#[test]
fn test_parse_gitlab_https_url_nested() {
    let provider = parse_reverse_repo("https://gitlab.com/group/subgroup/project").unwrap();
    assert_eq!(
        provider,
        VcsProvider::GitLab {
            host: Some("https://gitlab.com".to_string()),
            project_path: "group/subgroup/project".to_string()
        }
    );
}

#[test]
fn test_parse_gitlab_https_self_hosted_url() {
    let provider = parse_reverse_repo("https://gitlab.example.com/group/project").unwrap();
    assert_eq!(
        provider,
        VcsProvider::GitLab {
            host: Some("https://gitlab.example.com".to_string()),
            project_path: "group/project".to_string()
        }
    );
}
#[test]
fn test_parse_gitlab_https_url_with_git_suffix() {
    let provider = parse_reverse_repo("https://gitlab.com/group/project.git").unwrap();
    assert_eq!(
        provider,
        VcsProvider::GitLab {
            host: Some("https://gitlab.com".to_string()),
            project_path: "group/project".to_string()
        }
    );
}

#[test]
fn test_parse_gitlab_https_url_http_scheme() {
    // FR-004: http:// (non-HTTPS) GitLab URL.
    let provider = parse_reverse_repo("http://gitlab.com/group/project").unwrap();
    assert_eq!(
        provider,
        VcsProvider::GitLab {
            host: Some("http://gitlab.com".to_string()),
            project_path: "group/project".to_string()
        }
    );
}

#[test]
fn test_parse_gitlab_https_url_with_port() {
    // FR-004: GitLab URL with a port in the host.
    let provider = parse_reverse_repo("https://gitlab.example.com:8443/group/project").unwrap();
    assert_eq!(
        provider,
        VcsProvider::GitLab {
            host: Some("https://gitlab.example.com:8443".to_string()),
            project_path: "group/project".to_string()
        }
    );
}

#[test]
fn test_parse_gitlab_https_url_with_trailing_slash() {
    let provider = parse_reverse_repo("https://gitlab.com/group/project/").unwrap();
    assert_eq!(
        provider,
        VcsProvider::GitLab {
            host: Some("https://gitlab.com".to_string()),
            project_path: "group/project".to_string()
        }
    );
}

#[test]
fn test_parse_gitlab_https_self_hosted_with_port_and_nested() {
    // FR-004: self-hosted GitLab URL with port and nested namespace.
    let provider =
        parse_reverse_repo("https://gitlab.corp.com:8080/group/subgroup/project").unwrap();
    assert_eq!(
        provider,
        VcsProvider::GitLab {
            host: Some("https://gitlab.corp.com:8080".to_string()),
            project_path: "group/subgroup/project".to_string()
        }
    );
}

#[test]
fn test_parse_gitlab_https_url_single_path_segment_rejected() {
    // A URL with only one path segment after the host is not enough for
    // namespace/project.
    assert!(parse_reverse_repo("https://gitlab.com/justproject").is_err());
}

#[test]
fn test_parse_gitlab_https_url_no_path_rejected() {
    // A URL with no path after the host.
    assert!(parse_reverse_repo("https://gitlab.com").is_err());
    assert!(parse_reverse_repo("https://gitlab.com/").is_err());
}

#[test]
fn test_parse_gitlab_ssh_url() {
    let provider = parse_reverse_repo("git@gitlab.com:group/project.git").unwrap();
    assert_eq!(
        provider,
        VcsProvider::GitLab {
            host: Some("https://gitlab.com".to_string()),
            project_path: "group/project".to_string()
        }
    );
}

#[test]
fn test_parse_gitlab_ssh_self_hosted_url() {
    let provider = parse_reverse_repo("git@gitlab.example.com:group/project.git").unwrap();
    assert_eq!(
        provider,
        VcsProvider::GitLab {
            host: Some("https://gitlab.example.com".to_string()),
            project_path: "group/project".to_string()
        }
    );
}
#[test]
fn test_parse_gitlab_ssh_nested() {
    let provider = parse_reverse_repo("git@gitlab.com:group/subgroup/project.git").unwrap();
    assert_eq!(
        provider,
        VcsProvider::GitLab {
            host: Some("https://gitlab.com".to_string()),
            project_path: "group/subgroup/project".to_string()
        }
    );
}

#[test]
fn test_parse_gitlab_ssh_without_git_suffix() {
    // FR-004: SSH URL without a trailing .git.
    let provider = parse_reverse_repo("git@gitlab.com:group/project").unwrap();
    assert_eq!(
        provider,
        VcsProvider::GitLab {
            host: Some("https://gitlab.com".to_string()),
            project_path: "group/project".to_string()
        }
    );
}

#[test]
fn test_parse_gitlab_ssh_self_hosted_with_port() {
    // FR-004: SSH URL with a self-hosted GitLab instance.
    let provider = parse_reverse_repo("git@gitlab.corp.com:group/project.git").unwrap();
    assert_eq!(
        provider,
        VcsProvider::GitLab {
            host: Some("https://gitlab.corp.com".to_string()),
            project_path: "group/project".to_string()
        }
    );
}

#[test]
fn test_parse_gitlab_ssh_self_hosted_nested() {
    // FR-004 + FR-022: SSH URL with nested namespace on self-hosted GitLab.
    let provider = parse_reverse_repo("git@gitlab.corp.com:group/sub/project.git").unwrap();
    assert_eq!(
        provider,
        VcsProvider::GitLab {
            host: Some("https://gitlab.corp.com".to_string()),
            project_path: "group/sub/project".to_string()
        }
    );
}

#[test]
fn test_parse_gitlab_ssh_no_path_after_colon_rejected() {
    // SSH URL with no path after the host:colon.
    assert!(parse_reverse_repo("git@gitlab.com:").is_err());
    assert!(parse_reverse_repo("git@gitlab.com:justoneword").is_err());
}

#[test]
fn test_parse_gitlab_ssh_no_at_sign_rejected() {
    // Missing the git@ prefix - should not be treated as a GitLab SSH URL.
    // This would fall through to bare owner/repo -> GitHub, but "host:group/project"
    // has a colon which doesn't match any GitHub format.
    assert!(parse_reverse_repo("gitlab.com:group/project").is_err());
}

// --- Error / rejection cases (FR-013) -----------------------------------

#[test]
fn test_parse_github_prefix_ssh_url() {
    let provider = parse_reverse_repo("github:git@github.com:octocat/Hello-World.git").unwrap();
    assert_eq!(
        provider,
        VcsProvider::GitHub {
            owner: "octocat".to_string(),
            repo: "Hello-World".to_string()
        }
    );
}

#[test]
fn test_parse_github_prefix_http_url() {
    let provider = parse_reverse_repo("github:http://github.com/octocat/Hello-World").unwrap();
    assert_eq!(
        provider,
        VcsProvider::GitHub {
            owner: "octocat".to_string(),
            repo: "Hello-World".to_string()
        }
    );
}

#[test]
fn test_parse_github_prefix_with_trailing_slash() {
    let provider = parse_reverse_repo("github:octocat/Hello-World/").unwrap();
    assert_eq!(
        provider,
        VcsProvider::GitHub {
            owner: "octocat".to_string(),
            repo: "Hello-World".to_string()
        }
    );
}

#[test]
fn test_parse_github_prefix_with_git_suffix() {
    let provider = parse_reverse_repo("github:octocat/Hello-World.git").unwrap();
    assert_eq!(
        provider,
        VcsProvider::GitHub {
            owner: "octocat".to_string(),
            repo: "Hello-World".to_string()
        }
    );
}

#[test]
fn test_parse_github_prefix_three_segments_rejected() {
    // GitHub repos are always exactly owner/repo (2 segments).
    assert!(parse_reverse_repo("github:owner/repo/extra").is_err());
}

#[test]
fn test_parse_bare_owner_repo_with_trailing_dot_git() {
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
fn test_parse_github_prefix_url_with_subdomain() {
    let provider = parse_reverse_repo("github:https://www.github.com/octocat/Hello-World").unwrap();
    assert_eq!(
        provider,
        VcsProvider::GitHub {
            owner: "octocat".to_string(),
            repo: "Hello-World".to_string()
        }
    );
}

#[test]
fn test_parse_empty_input_rejected() {
    assert!(parse_reverse_repo("").is_err());
    assert!(parse_reverse_repo("   ").is_err());
}

#[test]
fn test_parse_single_word_rejected() {
    let result = parse_reverse_repo("justoneword");
    assert!(result.is_err());
    assert!(result.unwrap_err().contains("Accepted formats"));
}

#[test]
fn test_parse_error_message_lists_all_formats() {
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
}

#[test]
fn test_parse_gitlab_prefix_single_segment_rejected() {
    // gitlab:justoneword - no '/' -> invalid.
    assert!(parse_reverse_repo("gitlab:justoneword").is_err());
}

#[test]
fn test_parse_gitlab_prefix_host_only_rejected() {
    // gitlab:gitlab.example.com - host but no namespace/project.
    assert!(parse_reverse_repo("gitlab:gitlab.example.com").is_err());
}

#[test]
fn test_parse_github_prefix_invalid_rejected() {
    // github:justoneword - no '/' -> invalid.
    assert!(parse_reverse_repo("github:justoneword").is_err());
}

#[test]
fn test_parse_input_trimmed() {
    let provider = parse_reverse_repo("  octocat/Hello-World  ").unwrap();
    assert_eq!(
        provider,
        VcsProvider::GitHub {
            owner: "octocat".to_string(),
            repo: "Hello-World".to_string()
        }
    );
}

#[test]
fn test_parse_query_string_stripped() {
    let provider = parse_reverse_repo("https://github.com/octocat/Hello-World?tab=readme").unwrap();
    assert_eq!(
        provider,
        VcsProvider::GitHub {
            owner: "octocat".to_string(),
            repo: "Hello-World".to_string()
        }
    );
}

// --- looks_like_host helper ---------------------------------------------

#[test]
fn test_looks_like_host_with_dot() {
    assert!(looks_like_host("gitlab.example.com"));
}

#[test]
fn test_looks_like_host_without_dot() {
    assert!(!looks_like_host("group"));
    assert!(!looks_like_host("my-org"));
    assert!(!looks_like_host("localhost"));
}
