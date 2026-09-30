//! VCS-agnostic provider parsing for the `/spec reverse` command (FR-012, FR-013,
//! FR-022).
//!
//! The [`VcsProvider`] enum carries the resolved provider and parsed project
//! path. [`parse_reverse_repo`] accepts any supported repository identifier
//! format - provider-prefixed, bare shorthand, or full URL/SSH URL - and
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
/// - `github:owner/repo` - routes to GitHub (FR-001)
/// - `github:https://github.com/owner/repo` - routes to GitHub
/// - `gitlab:namespace/project` - routes to GitLab, host from config (FR-002)
/// - `gitlab:group/subgroup/project` - nested namespaces (FR-022)
/// - `gitlab:host/namespace/project` - self-hosted GitLab (FR-003)
///
/// **Bare shorthand:**
/// - `owner/repo` - routes to GitHub (FR-005, backward compat)
///
/// **GitHub URLs:**
/// - `https://github.com/owner/repo` - routes to GitHub (FR-006)
/// - `git@github.com:owner/repo.git` - routes to GitHub (FR-006)
///
/// **GitLab URLs:**
/// - `https://gitlab.com/namespace/project` - routes to GitLab (FR-004)
/// - `https://gitlab.example.com/group/project` - self-hosted GitLab (FR-004)
/// - `git@gitlab.com:namespace/project.git` - SSH URL (FR-004)
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
/// // Bare owner/repo -> GitHub.
/// let provider = parse_reverse_repo("octocat/Hello-World").unwrap();
/// assert_eq!(
///     provider,
///     VcsProvider::GitHub {
///         owner: "octocat".to_string(),
///         repo: "Hello-World".to_string()
///     }
/// );
///
/// // gitlab: prefix -> GitLab with no explicit host.
/// let provider = parse_reverse_repo("gitlab:group/project").unwrap();
/// assert_eq!(
///     provider,
///     VcsProvider::GitLab {
///         host: None,
///         project_path: "group/project".to_string()
///     }
/// );
///
/// // Invalid input -> error message.
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

    // GitHub HTTPS/SSH URLs. FUNC-067: classify by the parsed *host*, not by a
    // substring of the whole input - a GitLab URL whose path merely contains
    // "github.com" (e.g. `https://gitlab.com/x/github.com/y`) must stay GitLab.
    if extract_host(input).is_some_and(is_github_host) {
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

    // --- Bare owner/repo -> GitHub (FR-005, backward compat) ---

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

    // No format matched - reject (FR-013).
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
/// - `namespace/project` - no explicit host (resolved from config at fetch
///   time).
/// - `group/subgroup/project` - nested namespace (FR-022).
/// - `host/namespace/project` - self-hosted GitLab (FR-003). The first segment
///   is treated as a host if it contains a `.` (domain) or `:` (port).
/// - A full GitLab HTTPS/SSH URL - delegate to URL parsing.
fn parse_gitlab_prefixed(rest: &str) -> Result<VcsProvider, String> {
    let rest = rest.trim().trim_end_matches(".git");
    // If the inner portion is a full URL, delegate to URL parsing.
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
        // SEC-ragent-tools-vcs-004 (SECTASKS T-022): the host is caller-supplied
        // and a GitLab PAT is attached to every request to it, so an
        // attacker-chosen `gitlab:evil.example/ns/proj` would exfiltrate the
        // token. Accept it only when it matches the configured instance origin.
        let raw = segments[0];
        let configured = std::env::var("GITLAB_URL").ok().filter(|u| !u.is_empty());
        if !is_trusted_gitlab_host(raw, configured.as_deref()) {
            return Err(format!(
                "refusing GitLab host '{raw}': it is not the configured instance. \
                 Set GITLAB_URL (or `gitlab.instance_url`) to that host, or use \
                 a full https:// URL to confirm the target instance."
            ));
        }
        let host = format!("https://{}", raw);
        let project_path = segments[1..].join("/");
        return Ok(VcsProvider::GitLab {
            host: Some(host),
            project_path,
        });
    }

    // No host segment - all segments form the project path (FR-002, FR-022).
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

    // host_part is like "git@gitlab.com" - strip the "user@" prefix.
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

/// Known GitLab-hosted SaaS endpoints that may be named explicitly.
///
/// SEC-ragent-tools-vcs-004 (SECTASKS T-022): only an explicit short host that
/// is on the fixed allowlist is accepted without a configured instance. Every
/// other `gitlab:<host>/...` short form must match `GITLAB_URL` (the
/// `gitlab.instance_url` config value), so an LLM-supplied host cannot have
/// the user's PAT attached to it.
const ALLOWED_GITLAB_SAAS_HOSTS: &[&str] = &["gitlab.com"];

/// Whether a `gitlab:<host>/...` short-form host may carry the GitLab token.
///
/// `configured` is the `GITLAB_URL` value, if any. A host is trusted when it
/// is a known SaaS host, or when its host:port matches the configured instance
/// (case-insensitive, default ports normalised).
fn is_trusted_gitlab_host(raw_host: &str, configured: Option<&str>) -> bool {
    let candidate = raw_host.trim().trim_end_matches('/');
    let candidate_lower = candidate.to_ascii_lowercase();
    if ALLOWED_GITLAB_SAAS_HOSTS.contains(&candidate_lower.as_str()) {
        return true;
    }
    let Some(configured) = configured else {
        return false;
    };
    let configured_origin = configured
        .trim()
        .trim_end_matches('/')
        .trim_start_matches("https://")
        .trim_start_matches("http://");
    // The candidate may carry a port that the configured origin also carries
    // (`gitlab.example.com:8443`), so compare whole origins first.
    if configured_origin.eq_ignore_ascii_case(candidate) {
        return true;
    }
    // A candidate that is only the host (no port) still matches a configured
    // origin whose port is the default for its scheme: the token goes to the
    // same instance either way.
    let configured_host = configured_origin
        .split(['/', ':'])
        .next()
        .unwrap_or(configured_origin);
    let candidate_host = candidate.split(['/', ':']).next().unwrap_or(candidate);
    configured_host.eq_ignore_ascii_case(candidate_host)
}

/// Extract the host from an HTTPS/HTTP URL or an SSH `git@host:path` input
/// (FUNC-067). Returns `None` for bare `owner/repo` shorthand, which carries no
/// host and is routed by its `/` separator later.
fn extract_host(input: &str) -> Option<&str> {
    if let Some(rest) = input
        .strip_prefix("https://")
        .or_else(|| input.strip_prefix("http://"))
    {
        return Some(rest.split('/').next().unwrap_or(rest));
    }
    if let Some(rest) = input.strip_prefix("git@") {
        return Some(rest.split(':').next().unwrap_or(rest));
    }
    None
}

/// True when `host` is exactly a GitHub host (FUNC-067). Any userinfo and port
/// are stripped before comparison; a substring match is deliberately avoided so
/// `github.com.evil.example` does not misroute to GitHub.
fn is_github_host(host: &str) -> bool {
    let host = host.rsplit('@').next().unwrap_or(host);
    let host = host.split(':').next().unwrap_or(host);
    host.eq_ignore_ascii_case("github.com") || host.eq_ignore_ascii_case("www.github.com")
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
#[path = "../tests/inline/vcs_provider_tests.rs"]
mod tests;
