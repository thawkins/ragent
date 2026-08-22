//! VCS-agnostic provider parsing for the `/reverse` command (FR-012, FR-013,
//! FR-022).
//!
//! The [`VcsProvider`] enum carries the resolved provider and parsed project
//! path. [`parse_reverse_repo`] accepts any supported repository identifier
//! format — provider-prefixed, bare shorthand, or full URL/SSH URL — and
//! returns a [`VcsProvider`] value or a human-readable error.

use crate::github::GitHubClient;

/// A resolved VCS provider with its parsed project identifier (FR-012).
///
/// Produced by [`parse_reverse_repo`] from any supported input format. The
/// dispatch layer uses the variant to route fetch calls to the correct API
/// client.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum VcsProvider {
    /// A GitHub repository identified by `owner/repo`.
    GitHub {
        /// Repository owner (user or organisation).
        owner: String,
        /// Repository name.
        repo: String,
    },
    /// A GitLab repository identified by its full `namespace/project` path
    /// (which may include nested subgroups) and an optional explicit host.
    GitLab {
        /// Resolved GitLab instance base URL (e.g. `https://gitlab.com` or
        /// `https://gitlab.example.com`). When `None`, the host is resolved
        /// from configuration at fetch time, defaulting to
        /// `https://gitlab.com` (FR-002).
        host: Option<String>,
        /// Full project path including any nested subgroups
        /// (e.g. `group/project` or `group/subgroup/project`) (FR-022).
        project_path: String,
    },
}

/// Parse a repository identifier into a [`VcsProvider`] value (FR-012).
///
/// Accepts the following formats:
///
/// **Provider-prefixed:**
/// - `github:owner/repo` — routes to GitHub (FR-001)
/// - `github:https://github.com/owner/repo` — routes to GitHub
/// - `gitlab:namespace/project` — routes to GitLab, host from config (FR-002)
/// - `gitlab:group/subgroup/project` — nested namespaces (FR-022)
/// - `gitlab:host/namespace/project` — self-hosted GitLab (FR-003)
///
/// **Bare shorthand:**
/// - `owner/repo` — routes to GitHub (FR-005, backward compat)
///
/// **GitHub URLs:**
/// - `https://github.com/owner/repo` — routes to GitHub (FR-006)
/// - `git@github.com:owner/repo.git` — routes to GitHub (FR-006)
///
/// **GitLab URLs:**
/// - `https://gitlab.com/namespace/project` — routes to GitLab (FR-004)
/// - `https://gitlab.example.com/group/project` — self-hosted GitLab (FR-004)
/// - `git@gitlab.com:namespace/project.git` — SSH URL (FR-004)
///
/// # Errors
///
/// Returns `Err(message)` when the identifier does not match any supported
/// format (FR-013). The `message` is a human-readable string listing all
/// accepted formats, suitable for display in the TUI.
///
/// # Examples
///
/// ```
/// use ragent_tools_vcs::vcs_provider::{VcsProvider, parse_reverse_repo};
///
/// // Bare owner/repo → GitHub.
/// let provider = parse_reverse_repo("octocat/Hello-World").unwrap();
/// assert_eq!(
///     provider,
///     VcsProvider::GitHub {
///         owner: "octocat".to_string(),
///         repo: "Hello-World".to_string()
///     }
/// );
///
/// // gitlab: prefix → GitLab with no explicit host.
/// let provider = parse_reverse_repo("gitlab:group/project").unwrap();
/// assert_eq!(
///     provider,
///     VcsProvider::GitLab {
///         host: None,
///         project_path: "group/project".to_string()
///     }
/// );
///
/// // Invalid input → error message.
/// assert!(parse_reverse_repo("justoneword").is_err());
/// ```
pub fn parse_reverse_repo(input: &str) -> Result<VcsProvider, String> {
    let input = input.trim();
    if input.is_empty() {
        return Err(usage_error());
    }

    // Strip query strings and fragments from the input.
    let input = input.split(['?', '#']).next().unwrap_or(input);

    // --- Provider-prefixed identifiers (FR-001, FR-002, FR-003) ---

    if let Some(rest) = input.strip_prefix("github:") {
        return parse_github_prefixed(rest);
    }

    if let Some(rest) = input.strip_prefix("gitlab:") {
        return parse_gitlab_prefixed(rest);
    }

    // --- Full URL / SSH URL formats (FR-004, FR-006) ---

    // GitHub HTTPS/SSH URLs.
    if input.contains("github.com") {
        return GitHubClient::parse_repo_url(input)
            .map(|(owner, repo)| VcsProvider::GitHub { owner, repo })
            .ok_or_else(usage_error);
    } // GitLab HTTPS URLs: https://host/namespace/project or http://host/...
    if input.starts_with("https://") || input.starts_with("http://") {
        return parse_gitlab_https_url(input);
    }

    // GitLab SSH URLs: git@host:namespace/project.git
    if input.starts_with("git@") && input.contains(':') {
        return parse_gitlab_ssh_url(input);
    }

    // --- Bare owner/repo → GitHub (FR-005, backward compat) ---

    // Reject the SSH-without-user format `host:group/project` (no `git@`
    // prefix). A colon here doesn't match any GitHub owner/repo format, so it
    // must be rejected rather than silently misrouted to GitHub (FR-013).
    if input.contains(':') && !input.contains('@') {
        return Err(usage_error());
    }

    if input.contains('/') {
        return GitHubClient::parse_repo_url(input)
            .map(|(owner, repo)| VcsProvider::GitHub { owner, repo })
            .ok_or_else(usage_error);
    }

    // No format matched — reject (FR-013).
    Err(usage_error())
}

/// Parse a `github:`-prefixed identifier, delegating to
/// [`GitHubClient::parse_repo_url`] for the inner portion (FR-001, FR-005,
/// FR-006).
fn parse_github_prefixed(rest: &str) -> Result<VcsProvider, String> {
    GitHubClient::parse_repo_url(rest)
        .map(|(owner, repo)| VcsProvider::GitHub { owner, repo })
        .ok_or_else(usage_error)
}

/// Parse a `gitlab:`-prefixed identifier (FR-002, FR-003, FR-022).
///
/// The portion after `gitlab:` may be:
/// - `namespace/project` — no explicit host (resolved from config at fetch
///   time).
/// - `group/subgroup/project` — nested namespace (FR-022).
/// - `host/namespace/project` — self-hosted GitLab (FR-003). The first segment
///   is treated as a host if it contains a `.` (domain) or `:` (port).
/// - A full GitLab HTTPS/SSH URL — delegate to URL parsing.
fn parse_gitlab_prefixed(rest: &str) -> Result<VcsProvider, String> {
    let rest = rest.trim().trim_end_matches(".git"); // If the inner portion is a full URL, delegate to URL parsing.
    if rest.starts_with("https://") || rest.starts_with("http://") {
        return parse_gitlab_https_url(rest);
    }
    if rest.starts_with("git@") {
        return parse_gitlab_ssh_url(rest);
    }

    let segments: Vec<&str> = rest.split('/').filter(|s| !s.is_empty()).collect();
    if segments.len() < 2 {
        return Err(usage_error());
    }

    // If the first segment looks like a host (contains '.' or ':'), treat it
    // as a self-hosted GitLab instance (FR-003).
    if looks_like_host(segments[0]) {
        if segments.len() < 3 {
            // host + only one more segment is not enough for namespace/project.
            return Err(usage_error());
        }
        let host = format!("https://{}", segments[0]);
        let project_path = segments[1..].join("/");
        return Ok(VcsProvider::GitLab {
            host: Some(host),
            project_path,
        });
    }

    // No host segment — all segments form the project path (FR-002, FR-022).
    let project_path = segments.join("/");
    Ok(VcsProvider::GitLab {
        host: None,
        project_path,
    })
}

/// Parse a GitLab HTTPS URL (FR-004).
///
/// `url` is the full URL including the scheme, e.g.
/// `https://gitlab.com/group/project`, `http://gitlab.com/group/project`, or
/// `https://gitlab.example.com/group/subgroup/project`. The scheme is
/// preserved in the resolved host so that non-HTTPS instances are honoured.
fn parse_gitlab_https_url(url: &str) -> Result<VcsProvider, String> {
    let (scheme, rest) = url
        .strip_prefix("https://")
        .map(|r| ("https", r))
        .or_else(|| url.strip_prefix("http://").map(|r| ("http", r)))
        .ok_or_else(usage_error)?;

    let rest = rest.trim_end_matches(".git").trim_end_matches('/');
    // Find the first '/' to split host from path.
    let slash_idx = match rest.find('/') {
        Some(idx) => idx,
        None => return Err(usage_error()),
    };
    let host = &rest[..slash_idx];
    let path = &rest[slash_idx + 1..];

    if host.is_empty() || path.is_empty() {
        return Err(usage_error());
    }

    let project_path = path.trim_end_matches(".git");
    if !project_path.contains('/') {
        return Err(usage_error());
    }

    Ok(VcsProvider::GitLab {
        host: Some(format!("{scheme}://{host}")),
        project_path: project_path.to_string(),
    })
}

/// Parse a GitLab SSH URL (FR-004).
///
/// Format: `git@host:namespace/project.git`
fn parse_gitlab_ssh_url(input: &str) -> Result<VcsProvider, String> {
    let input = input.trim().trim_end_matches(".git");
    // Extract the portion after the first ':'.
    let colon_idx = match input.find(':') {
        Some(idx) => idx,
        None => return Err(usage_error()),
    };
    let host_part = &input[..colon_idx];
    let path_part = &input[colon_idx + 1..];

    // host_part is like "git@gitlab.com" — strip the "user@" prefix.
    let host = match host_part.split('@').next_back() {
        Some(h) if !h.is_empty() => h,
        _ => return Err(usage_error()),
    };

    if path_part.is_empty() || !path_part.contains('/') {
        return Err(usage_error());
    }

    Ok(VcsProvider::GitLab {
        host: Some(format!("https://{host}")),
        project_path: path_part.to_string(),
    })
}

/// Heuristic: does a segment look like a host (contains a domain dot or port
/// colon)?
fn looks_like_host(segment: &str) -> bool {
    segment.contains('.') || segment.contains(':')
}

/// Build the human-readable error message listing all accepted formats
/// (FR-013).
fn usage_error() -> String {
    "Invalid repository identifier.\n\
     \n\
     Accepted formats:\n\
     - `owner/repo` (defaults to GitHub)\n\
     - `github:owner/repo` or `github:<github-url>`\n\
     - `gitlab:namespace/project` (uses configured GitLab instance)\n\
     - `gitlab:group/subgroup/project` (nested namespaces)\n\
     - `gitlab:host/namespace/project` (self-hosted GitLab)\n\
     - `https://github.com/owner/repo`\n\
     - `git@github.com:owner/repo.git`\n\
     - `https://gitlab.com/namespace/project`\n\
     - `https://<gitlab-host>/namespace/project` (self-hosted)\n\
     - `git@<gitlab-host>:namespace/project.git` (self-hosted SSH)\n\
     \n\
     GitLab repositories require `/gitlab setup` (or GITLAB_TOKEN + GITLAB_URL \
     env vars) before use."
        .to_string()
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
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

    #[test]
    fn test_parse_gitlab_prefix_self_hosted() {
        let provider = parse_reverse_repo("gitlab:gitlab.example.com/group/project").unwrap();
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
        let provider =
            parse_reverse_repo("gitlab:gitlab.example.com/group/subgroup/project").unwrap();
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
        let provider = parse_reverse_repo("gitlab:gitlab.example.com:8443/group/project").unwrap();
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
        let provider = parse_reverse_repo("gitlab:10.0.0.1/group/project").unwrap();
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
        let provider = parse_reverse_repo("gitlab:localhost:8080/group/project").unwrap();
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
        let provider = parse_reverse_repo("gitlab:gitlab.corp.com/team/repo").unwrap();
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
        let provider = parse_reverse_repo("gitlab:gitlab.corp.com/a/b/c/project").unwrap();
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
        let provider = parse_reverse_repo("gitlab:gitlab.example.com/group/project.git").unwrap();
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
        assert!(parse_reverse_repo("gitlab:gitlab.example.com/project").is_err());
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
        // Missing the git@ prefix — should not be treated as a GitLab SSH URL.
        // This would fall through to bare owner/repo → GitHub, but "host:group/project"
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
        let provider =
            parse_reverse_repo("github:https://www.github.com/octocat/Hello-World").unwrap();
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
        // gitlab:justoneword — no '/' → invalid.
        assert!(parse_reverse_repo("gitlab:justoneword").is_err());
    }

    #[test]
    fn test_parse_gitlab_prefix_host_only_rejected() {
        // gitlab:gitlab.example.com — host but no namespace/project.
        assert!(parse_reverse_repo("gitlab:gitlab.example.com").is_err());
    }

    #[test]
    fn test_parse_github_prefix_invalid_rejected() {
        // github:justoneword — no '/' → invalid.
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
        let provider =
            parse_reverse_repo("https://github.com/octocat/Hello-World?tab=readme").unwrap();
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
}
