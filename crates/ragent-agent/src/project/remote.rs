//! Describing the remote a project worktree was checked out from (spec
//! `projects`, T-004).
//!
//! GitHub and GitLab are recognised. The classification is by the remote
//! URL's host only: `github.com` or a configured GitHub Enterprise Server
//! host (`vcs.github.hosts`) is GitHub; `gitlab.com`, a host whose first
//! label is `gitlab`, or the configured self-hosted instance is GitLab.
//! Anything else yields no remote description, and the project stays a
//! local project. Depends on `ragent-tools-vcs` for the timeout-bounded git
//! runner.

use std::path::Path;

use ragent_config::GitHubHost;
use ragent_storage::ProjectRemoteRow;
use serde::{Deserialize, Serialize};

/// The words a hosting system uses for a change request: GitHub has pull
/// requests, GitLab has merge requests (spec SESSIONMENU A4).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RequestNoun {
    /// Abbreviation: `PR` or `MR`.
    pub short: &'static str,
    /// Full name in lower case: `pull request` or `merge request`.
    pub long: &'static str,
    /// Prefix of a request number: `#` on GitHub, `!` on GitLab.
    pub sigil: char,
}

/// The hosting system of a project remote.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum RemoteKind {
    /// github.com.
    GitHub,
    /// gitlab.com or a self-hosted GitLab instance.
    GitLab,
}

impl RemoteKind {
    /// The stored and displayed spelling.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::GitHub => "github",
            Self::GitLab => "gitlab",
        }
    }

    /// What this hosting system calls a change request.
    #[must_use]
    pub const fn noun(self) -> RequestNoun {
        match self {
            Self::GitHub => RequestNoun {
                short: "PR",
                long: "pull request",
                sigil: '#',
            },
            Self::GitLab => RequestNoun {
                short: "MR",
                long: "merge request",
                sigil: '!',
            },
        }
    }

    /// Parses a stored spelling.
    #[must_use]
    pub fn parse(stored: &str) -> Option<Self> {
        match stored {
            "github" => Some(Self::GitHub),
            "gitlab" => Some(Self::GitLab),
            _ => None,
        }
    }
}

impl std::fmt::Display for RemoteKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

/// The remote a project worktree was checked out from.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProjectRemote {
    /// Hosting system.
    pub kind: RemoteKind,
    /// Host name, with a port when the URL carries one.
    pub host: String,
    /// Repository path on the host: `owner/repo` or `group/sub/project`.
    pub path: String,
    /// Remote URL with any embedded credentials removed.
    pub url: String,
    /// Git remote name, normally `origin`.
    pub remote_name: String,
    /// The remote's default branch, when known.
    pub default_branch: Option<String>,
}

impl ProjectRemote {
    /// Builds the remote from its stored row; `None` when the stored kind is
    /// not one this version knows.
    #[must_use]
    pub fn from_row(row: &ProjectRemoteRow) -> Option<Self> {
        Some(Self {
            kind: RemoteKind::parse(&row.kind)?,
            host: row.host.clone(),
            path: row.path.clone(),
            url: row.url.clone(),
            remote_name: row.remote_name.clone(),
            default_branch: row.default_branch.clone(),
        })
    }

    /// The row to store for this remote.
    #[must_use]
    pub fn to_row(&self) -> ProjectRemoteRow {
        ProjectRemoteRow {
            kind: self.kind.as_str().to_string(),
            host: self.host.clone(),
            path: self.path.clone(),
            url: self.url.clone(),
            remote_name: self.remote_name.clone(),
            default_branch: self.default_branch.clone(),
        }
    }
}

/// The pieces of a git remote URL.
struct RemoteUrl {
    /// Scheme prefix including `://`, empty for the scp-like form.
    scheme: String,
    /// User part without credentials (`git` in `git@host:path`), if any.
    user: Option<String>,
    /// Host, with a port when present.
    host: String,
    /// Path without a leading `/` or a trailing `.git`.
    path: String,
    /// Whether the URL was the scp-like `user@host:path` form.
    scp_like: bool,
}

/// Splits a git remote URL. Supports `https://`, `http://`, `ssh://`,
/// `git://` and the scp-like `user@host:path` form.
fn split_remote_url(url: &str) -> Option<RemoteUrl> {
    let url = url.trim();
    let (scheme, rest, scp_like) = match url.find("://") {
        Some(i) => (url[..i + 3].to_ascii_lowercase(), &url[i + 3..], false),
        None => (String::new(), url, true),
    };
    let (authority, path) = if scp_like {
        // `user@host:path`; a bare local path has no `:` before its first `/`.
        let colon = rest.find(':')?;
        if rest[..colon].contains('/') {
            return None;
        }
        (&rest[..colon], &rest[colon + 1..])
    } else {
        let slash = rest.find('/')?;
        (&rest[..slash], &rest[slash + 1..])
    };
    let (userinfo, host) = match authority.rsplit_once('@') {
        Some((userinfo, host)) => (Some(userinfo), host),
        None => (None, authority),
    };
    // Keep the user name (needed for ssh), drop any password or token.
    let user = userinfo
        .map(|u| u.split(':').next().unwrap_or(u))
        .filter(|u| !u.is_empty())
        .map(str::to_string);
    let path = path
        .trim_matches('/')
        .trim_end_matches(".git")
        .trim_end_matches('/');
    if host.is_empty() || path.is_empty() {
        return None;
    }
    Some(RemoteUrl {
        scheme,
        user,
        host: host.to_ascii_lowercase(),
        path: path.to_string(),
        scp_like,
    })
}

/// The host (with port) and path prefix of a configured GitLab instance.
///
/// `https://gitlab.example.com/gitlab/` yields
/// `("gitlab.example.com", "gitlab")`; a bare `code.example.org` yields
/// `("code.example.org", "")`.
#[must_use]
pub fn split_instance_url(instance: &str) -> (String, String) {
    let lower = instance.trim().to_ascii_lowercase();
    let rest = lower.split_once("://").map_or(lower.as_str(), |(_, r)| r);
    let rest = rest.rsplit_once('@').map_or(rest, |(_, host)| host);
    let (host, prefix) = rest.split_once('/').unwrap_or((rest, ""));
    (host.to_string(), prefix.trim_matches('/').to_string())
}

/// Whether `host` (with an optional port) is the configured instance host.
/// A configured host without a port matches the remote host on any port,
/// because an ssh remote and the web UI rarely share one.
pub(crate) fn is_instance_host(host: &str, instance_host: &str) -> bool {
    let name = host.split(':').next().unwrap_or(host);
    !instance_host.is_empty()
        && (instance_host == host || instance_host.split(':').next() == Some(name))
}

/// The configured GitHub Enterprise Server entry for `host`, if any.
///
/// Entries are matched by host name (a scheme or path in the configured
/// value is ignored); an entry without a port matches any port.
#[must_use]
pub fn github_host_entry<'a>(host: &str, github_hosts: &'a [GitHubHost]) -> Option<&'a GitHubHost> {
    github_hosts
        .iter()
        .find(|entry| is_instance_host(host, &split_instance_url(&entry.host).0))
}

/// Classifies a host. `gitlab_host` is the configured self-hosted GitLab
/// host or base URL, if any; `github_hosts` are the configured GitHub
/// Enterprise Server hosts.
fn classify_host(
    host: &str,
    gitlab_host: Option<&str>,
    github_hosts: &[GitHubHost],
) -> Option<RemoteKind> {
    let name = host.split(':').next().unwrap_or(host);
    let name = name.strip_prefix("www.").unwrap_or(name);
    if name == "github.com" || github_host_entry(host, github_hosts).is_some() {
        return Some(RemoteKind::GitHub);
    }
    let configured =
        gitlab_host.is_some_and(|instance| is_instance_host(host, &split_instance_url(instance).0));
    if name == "gitlab.com" || name.starts_with("gitlab.") || configured {
        return Some(RemoteKind::GitLab);
    }
    None
}

/// Describes a git remote URL.
///
/// # Arguments
///
/// * `url` - the remote URL as git reports it.
/// * `remote_name` - the git remote the URL belongs to.
/// * `gitlab_host` - the configured self-hosted GitLab host or base URL, if
///   any; a remote on that host is GitLab whatever its name.
///
/// # Returns
///
/// The remote description, with credentials stripped from the stored URL, or
/// `None` when the URL cannot be parsed, the host is neither GitHub nor
/// GitLab, or a GitHub path is not exactly `owner/repo`.
///
/// # Examples
///
/// ```
/// use ragent_agent::project::{RemoteKind, parse_remote_url};
///
/// let remote = parse_remote_url("git@github.com:acme/app.git", "origin", None).unwrap();
/// assert_eq!(remote.kind, RemoteKind::GitHub);
/// assert_eq!(remote.path, "acme/app");
/// assert!(parse_remote_url("https://example.com/acme/app.git", "origin", None).is_none());
/// ```
#[must_use]
pub fn parse_remote_url(
    url: &str,
    remote_name: &str,
    gitlab_host: Option<&str>,
) -> Option<ProjectRemote> {
    parse_remote_url_with_hosts(url, remote_name, gitlab_host, &[])
}

/// [`parse_remote_url`] that also recognises the configured GitHub
/// Enterprise Server hosts (`vcs.github.hosts`) as GitHub.
///
/// # Examples
///
/// ```
/// use ragent_agent::project::{RemoteKind, parse_remote_url_with_hosts};
/// use ragent_config::GitHubHost;
///
/// let hosts = [GitHubHost {
///     host: "github.example.com".into(),
///     api_url: "https://github.example.com/api/v3".into(),
/// }];
/// let url = "git@github.example.com:acme/app.git";
/// let remote = parse_remote_url_with_hosts(url, "origin", None, &hosts).unwrap();
/// assert_eq!(remote.kind, RemoteKind::GitHub);
/// assert_eq!(remote.host, "github.example.com");
/// ```
#[must_use]
pub fn parse_remote_url_with_hosts(
    url: &str,
    remote_name: &str,
    gitlab_host: Option<&str>,
    github_hosts: &[GitHubHost],
) -> Option<ProjectRemote> {
    let mut parts = split_remote_url(url)?;
    let kind = classify_host(&parts.host, gitlab_host, github_hosts)?;
    // An instance served under a path (`https://host/gitlab`) puts that
    // prefix in front of every http(s) clone path; it is not part of the
    // project path. ssh remotes do not carry it.
    let mut url_prefix = String::new();
    if let (RemoteKind::GitLab, false, Some(instance)) = (kind, parts.scp_like, gitlab_host) {
        let (instance_host, prefix) = split_instance_url(instance);
        if !prefix.is_empty()
            && parts.scheme.starts_with("http")
            && is_instance_host(&parts.host, &instance_host)
        {
            if let Some(rest) = parts
                .path
                .strip_prefix(prefix.as_str())
                .and_then(|r| r.strip_prefix('/'))
            {
                parts.path = rest.to_string();
                url_prefix = format!("{prefix}/");
            }
        }
    }
    let segments = parts.path.split('/').filter(|s| !s.is_empty()).count();
    let valid = match kind {
        RemoteKind::GitHub => segments == 2,
        RemoteKind::GitLab => segments >= 2,
    };
    if !valid {
        return None;
    }
    let user = parts.user.as_deref();
    let clean = if parts.scp_like {
        match user {
            Some(user) => format!("{user}@{}:{}.git", parts.host, parts.path),
            None => format!("{}:{}.git", parts.host, parts.path),
        }
    } else {
        // Over http(s) a user part is a credential; over ssh it is the login.
        match user.filter(|_| parts.scheme == "ssh://") {
            Some(user) => format!(
                "{}{user}@{}/{url_prefix}{}.git",
                parts.scheme, parts.host, parts.path
            ),
            None => format!(
                "{}{}/{url_prefix}{}.git",
                parts.scheme, parts.host, parts.path
            ),
        }
    };
    Some(ProjectRemote {
        kind,
        host: parts.host,
        path: parts.path,
        url: clean,
        remote_name: remote_name.to_string(),
        default_branch: None,
    })
}

/// Runs a git query in `dir` and returns its trimmed stdout, or `None` when
/// git fails or prints nothing.
fn git_line(args: &[&str], dir: &Path) -> Option<String> {
    let output = ragent_tools_vcs::git::run_git_output(args, dir).ok()?;
    if !output.success {
        return None;
    }
    let text = String::from_utf8(output.stdout).ok()?;
    let text = text.trim();
    (!text.is_empty()).then(|| text.to_string())
}

/// Detects the remote of the checkout at `worktree` from its `origin`
/// remote.
///
/// # Arguments
///
/// * `worktree` - the project's local checkout.
/// * `gitlab_host` - the configured self-hosted GitLab host, if any.
///
/// # Returns
///
/// `None` when `worktree` is not a git checkout, has no `origin` remote, or
/// the remote is on a host that is neither GitHub nor GitLab.
#[must_use]
pub fn detect_remote(worktree: &Path, gitlab_host: Option<&str>) -> Option<ProjectRemote> {
    detect_remote_with_hosts(worktree, gitlab_host, &[])
}

/// [`detect_remote`] that also recognises the configured GitHub Enterprise
/// Server hosts as GitHub.
#[must_use]
pub fn detect_remote_with_hosts(
    worktree: &Path,
    gitlab_host: Option<&str>,
    github_hosts: &[GitHubHost],
) -> Option<ProjectRemote> {
    const REMOTE: &str = "origin";
    let url = git_line(&["remote", "get-url", REMOTE], worktree)?;
    let mut remote = parse_remote_url_with_hosts(&url, REMOTE, gitlab_host, github_hosts)?;
    // `refs/remotes/origin/HEAD` -> `origin/main` -> `main`.
    remote.default_branch = git_line(
        &["symbolic-ref", "--short", "refs/remotes/origin/HEAD"],
        worktree,
    )
    .and_then(|head| head.strip_prefix("origin/").map(str::to_string));
    Some(remote)
}
