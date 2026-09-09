//! Remote-init flow for the `/new` scaffolder (FR-008 remote half, T-009).
//!
//! When a hosting flag is supplied the scaffolder must create the hosting
//! repository, register it as `origin`, and push the initial commit. This
//! module implements both hosting paths (T-009 GitHub, T-010 GitLab):
//!
//! - **GitHub** — resolve the token (`GITHUB_TOKEN` env, then the
//!   `~/.ragent/github_token` file written by `/github login`); create the
//!   repository via `POST /user/repos`, reusing an existing same-name
//!   repository (422 → `GET /repos/{login}/{name}`, FR-015 idempotent retry).
//! - **GitLab** — resolve the token (`GITLAB_TOKEN` env, then the
//!   `~/.ragent/gitlab_token` file) and instance URL (`GITLAB_URL` env, then
//!   `~/.ragent/gitlab_config.json`, else `https://gitlab.com`); create the
//!   project via `POST /projects`, reusing an existing same-name project
//!   (400 "has already been taken" → `GET /projects/{user}%2F{name}`),
//!   authenticating with the `PRIVATE-TOKEN` header.
//!
//! Both paths then register `origin` (`git remote add`, or `git remote
//! set-url` when a remote already exists — FR-015 retry tolerance) and push
//! the initial commit (`git push -u origin <branch>`).
//!
//! Failure containment (FR-010): every failure is returned as a
//! [`RemoteFailure`] naming the failed step with an actionable message; the
//! local scaffold is never rolled back and no partially-initialised remote
//! state is reported as success. Push failures keep the configured `origin`
//! so the user can retry `git push -u origin <branch>` directly.
//!
//! The API calls use a blocking HTTP client because the `/new` command runs
//! in the synchronous TUI foreground flow (FR-014). The injectable variants
//! [`init_github_remote_with_token`] / [`init_gitlab_remote_with_token`] let
//! tests point the client at a local mock server.

use std::fmt;
use std::path::Path;
use std::process::Command;

use reqwest::blocking::Client;
use serde_json::Value;

/// Default GitHub API base URL.
const GITHUB_API_BASE: &str = "https://api.github.com";

/// Default GitLab instance base URL (the API path `/api/v4` is appended
/// per-request, mirroring the `ragent-tools-vcs` GitLab client).
const GITLAB_API_BASE: &str = "https://gitlab.com";

/// Remote-init sub-steps that can fail (FR-010 containment naming).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RemoteStep {
    /// No usable GitHub credentials, or the credentials were rejected.
    Auth,
    /// Hosting repository creation (with the FR-015 reuse fallback).
    RepoCreate,
    /// No usable GitLab credentials, or the credentials were rejected.
    GitLabAuth,
    /// GitLab project creation (with the FR-015 reuse fallback).
    GitLabRepoCreate,
    /// `origin` remote registration (`git remote add` / `set-url`).
    RemoteAdd,
    /// Pushing the initial commit.
    Push,
}

impl RemoteStep {
    /// Human-readable step name for error and summary messages.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Auth => "github auth",
            Self::RepoCreate => "github repo create",
            Self::GitLabAuth => "gitlab auth",
            Self::GitLabRepoCreate => "gitlab repo create",
            Self::RemoteAdd => "git remote add",
            Self::Push => "git push",
        }
    }
}

impl fmt::Display for RemoteStep {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// A failed remote-init step with actionable diagnostics attached (FR-010).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RemoteFailure {
    /// Step that failed.
    pub step: RemoteStep,
    /// Failure diagnostics and remediation guidance.
    pub message: String,
}

impl fmt::Display for RemoteFailure {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} failed: {}", self.step, self.message.trim())
    }
}

/// Result of a successful remote-init pass.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RemoteReport {
    /// True when the hosting repository was created by this pass; false
    /// when an existing same-name repository was reused (FR-015).
    pub repo_created: bool,
    /// True when `origin` already existed and was re-pointed via
    /// `git remote set-url` (FR-015); false when it was newly added.
    pub remote_existed: bool,
    /// Hosting URL the `origin` remote points at and the push targeted.
    pub url: String,
}

/// Resolve the GitHub token: `GITHUB_TOKEN` first, then the
/// `~/.ragent/github_token` file written by `/github login`. Mirrors the
/// `ragent-tools-vcs` token precedence without a cross-crate dependency.
pub fn load_github_token() -> Option<String> {
    if let Ok(token) = std::env::var("GITHUB_TOKEN")
        && !token.trim().is_empty()
    {
        return Some(token.trim().to_owned());
    }
    let path = dirs::home_dir()?.join(".ragent").join("github_token");
    let raw = std::fs::read_to_string(path).ok()?;
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_owned())
    }
}

/// API flavour controlling the authentication header style (T-009/T-010).
#[derive(Debug, Clone, Copy)]
enum ApiFlavor {
    /// GitHub: `Authorization: Bearer` plus the GitHub JSON accept header.
    GitHub,
    /// GitLab: `PRIVATE-TOKEN` header (mirrors the `ragent-tools-vcs`
    /// GitLab client).
    GitLab,
}

/// One hosting-provider API JSON request; returns `(status, parsed body)`
/// or an error string when the request cannot be completed at all.
fn api_json(
    client: &Client,
    method: reqwest::Method,
    base_url: &str,
    path: &str,
    token: &str,
    body: Option<&Value>,
    flavor: ApiFlavor,
) -> Result<(u16, Value), String> {
    let url = format!("{}{path}", base_url.trim_end_matches('/'));
    let mut request = client
        .request(method, &url)
        .header("User-Agent", "ragent-newproj");
    request = match flavor {
        ApiFlavor::GitHub => request
            .header("Authorization", format!("Bearer {token}"))
            .header("Accept", "application/vnd.github+json"),
        ApiFlavor::GitLab => request.header("PRIVATE-TOKEN", token),
    };
    if let Some(payload) = body {
        request = request.json(payload);
    }
    let response = request.send().map_err(|err| err.to_string())?;
    let status = response.status().as_u16();
    let value = response.json::<Value>().unwrap_or(Value::Null);
    Ok((status, value))
}

/// Extract GitHub's human-readable `message` field from an error body.
fn api_message(value: &Value) -> String {
    value
        .get("message")
        .and_then(Value::as_str)
        .unwrap_or("(no message)")
        .to_owned()
}

/// Extract the `html_url` field used as the `origin` push URL.
fn html_url(value: &Value) -> Result<String, RemoteFailure> {
    value
        .get("html_url")
        .and_then(Value::as_str)
        .map(str::to_owned)
        .filter(|url| !url.is_empty())
        .ok_or_else(|| RemoteFailure {
            step: RemoteStep::RepoCreate,
            message: "GitHub response missing html_url".to_owned(),
        })
}

/// Run `git` in `root`, mapping spawn/exit failures onto `step`.
fn run_remote_git(root: &Path, step: RemoteStep, args: &[&str]) -> Result<String, RemoteFailure> {
    let output = Command::new("git")
        .args(args)
        .current_dir(root)
        .output()
        .map_err(|err| RemoteFailure {
            step,
            message: err.to_string(),
        })?;
    if output.status.success() {
        Ok(String::from_utf8_lossy(&output.stdout).into_owned())
    } else {
        Err(RemoteFailure {
            step,
            message: format!(
                "{}{}",
                String::from_utf8_lossy(&output.stderr),
                String::from_utf8_lossy(&output.stdout)
            ),
        })
    }
}

/// Register `origin` (or re-point it via set-url when it already exists —
/// FR-015 retry tolerance) and push the initial commit. Shared tail of the
/// GitHub and GitLab hosting flows, which otherwise duplicate this block
/// verbatim.
///
/// # Errors
///
/// [`RemoteFailure`] naming the failing step (`RemoteAdd` / `Push`). A push
/// failure keeps the configured `origin` and carries the manual-retry
/// remediation line.
fn register_origin_and_push(root: &Path, url: &str) -> Result<RemoteReport, RemoteFailure> {
    let remote_existed = Command::new("git")
        .args(["remote", "get-url", "origin"])
        .current_dir(root)
        .output()
        .map(|out| out.status.success())
        .unwrap_or(false);
    let register_args: &[&str] = if remote_existed {
        &["remote", "set-url", "origin", url]
    } else {
        &["remote", "add", "origin", url]
    };
    run_remote_git(root, RemoteStep::RemoteAdd, register_args)?;

    // Push the initial commit on the current branch.
    let branch = run_remote_git(
        root,
        RemoteStep::Push,
        &["rev-parse", "--abbrev-ref", "HEAD"],
    )?
    .trim()
    .to_owned();
    if let Err(mut failure) =
        run_remote_git(root, RemoteStep::Push, &["push", "-u", "origin", &branch])
    {
        failure.message = format!(
            "{}\nremediation: the remote is configured and the hosting target \
             exists; retry manually with `git push -u origin {branch}` - the local \
             scaffold is intact",
            failure.message.trim()
        );
        return Err(failure);
    }

    Ok(RemoteReport {
        repo_created: false,
        remote_existed,
        url: url.to_owned(),
    })
}

/// Create a private GitHub repository, register it as `origin`, and push
/// the initial commit — resolving the token from the environment/credential
/// file (FR-008, T-009).
///
/// # Errors
///
/// [`RemoteFailure`] naming the failed step (FR-010 containment): the local
/// scaffold is never rolled back, and a push failure keeps the configured
/// `origin` so the user can retry the push directly.
pub fn init_github_remote(
    root: &Path,
    repo_name: &str,
    private: bool,
) -> Result<RemoteReport, RemoteFailure> {
    init_github_remote_with_token(
        GITHUB_API_BASE,
        load_github_token().as_deref(),
        root,
        repo_name,
        private,
    )
}

/// Injectable variant of [`init_github_remote`]: the caller supplies the API
/// base URL and token directly. Tests point `base_url` at a local mock
/// server; production uses the GitHub API default.
///
/// # Errors
///
/// [`RemoteFailure`] naming the failed step (FR-010 containment).
pub fn init_github_remote_with_token(
    base_url: &str,
    token: Option<&str>,
    root: &Path,
    repo_name: &str,
    private: bool,
) -> Result<RemoteReport, RemoteFailure> {
    let token = token
        .map(str::trim)
        .filter(|t| !t.is_empty())
        .ok_or_else(|| RemoteFailure {
            step: RemoteStep::Auth,
            message: "no GitHub token found; run `/github login` or set GITHUB_TOKEN, then \
                      retry the hosting step - the local scaffold is intact"
                .to_owned(),
        })?;

    let client = Client::builder()
        .timeout(std::time::Duration::from_secs(30))
        .build()
        .map_err(|err| RemoteFailure {
            step: RemoteStep::RepoCreate,
            message: err.to_string(),
        })?;

    // 1. Identify the authenticated account (login needed for the FR-015
    //    existing-repository reuse path).
    let (status, value) = api_json(
        &client,
        reqwest::Method::GET,
        base_url,
        "/user",
        token,
        None,
        ApiFlavor::GitHub,
    )
    .map_err(|message| RemoteFailure {
        step: RemoteStep::Auth,
        message,
    })?;
    if status == 401 {
        return Err(RemoteFailure {
            step: RemoteStep::Auth,
            message: "GitHub rejected the token (401); run `/github login` to re-authenticate \
                      - the local scaffold is intact"
                .to_owned(),
        });
    }
    let login = value
        .get("login")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_owned();
    if login.is_empty() {
        return Err(RemoteFailure {
            step: RemoteStep::Auth,
            message: "could not read the GitHub login from GET /user; the token may lack the \
                      required scopes - the local scaffold is intact"
                .to_owned(),
        });
    }

    // 2. Create the hosting repository, or reuse an existing same-name
    //    repository (GitHub answers 422 "already exists" on retry).
    let payload = serde_json::json!({ "name": repo_name, "private": private });
    let (status, value) = api_json(
        &client,
        reqwest::Method::POST,
        base_url,
        "/user/repos",
        token,
        Some(&payload),
        ApiFlavor::GitHub,
    )
    .map_err(|message| RemoteFailure {
        step: RemoteStep::RepoCreate,
        message,
    })?;
    let (repo_created, url) = match status {
        201 => (true, html_url(&value)?),
        422 => {
            // FR-015 idempotent retry: the repository already exists on the
            // account; resolve its URL and continue with the push.
            let path = format!("/repos/{login}/{repo_name}");
            let (status, value) = api_json(
                &client,
                reqwest::Method::GET,
                base_url,
                &path,
                token,
                None,
                ApiFlavor::GitHub,
            )
            .map_err(|message| RemoteFailure {
                step: RemoteStep::RepoCreate,
                message: format!(
                    "repository '{repo_name}' already exists but could not be read: {message}"
                ),
            })?;
            if status != 200 {
                return Err(RemoteFailure {
                    step: RemoteStep::RepoCreate,
                    message: format!(
                        "repository '{repo_name}' already exists but GET /repos returned \
                         {status}: {}",
                        api_message(&value)
                    ),
                });
            }
            (false, html_url(&value)?)
        }
        401 => {
            return Err(RemoteFailure {
                step: RemoteStep::Auth,
                message: "GitHub rejected the token (401); run `/github login` to \
                          re-authenticate - the local scaffold is intact"
                    .to_owned(),
            });
        }
        other => {
            return Err(RemoteFailure {
                step: RemoteStep::RepoCreate,
                message: format!("GitHub API error {other}: {}", api_message(&value)),
            });
        }
    };

    // 3. Register `origin` and 4. push the initial commit (shared tail).
    let report = register_origin_and_push(root, &url)?;
    Ok(RemoteReport {
        repo_created,
        remote_existed: report.remote_existed,
        url: report.url,
    })
}
// --------------------------------------------------------------------------
// T-010: GitLab remote-init flow (FR-008, FR-010, FR-015)
// --------------------------------------------------------------------------

/// Resolve the GitLab Personal Access Token: `GITLAB_TOKEN` env first, then
/// the `~/.ragent/gitlab_token` file (a legacy path the `/gitlab setup`
/// flow migrates into the database). Mirrors the `ragent-tools-vcs` token
/// precedence without a cross-crate dependency; the TUI cannot be consulted
/// here because this module must stay testable without storage.
pub fn load_gitlab_token() -> Option<String> {
    if let Ok(token) = std::env::var("GITLAB_TOKEN")
        && !token.trim().is_empty()
    {
        return Some(token.trim().to_owned());
    }
    let path = dirs::home_dir()?.join(".ragent").join("gitlab_token");
    let raw = std::fs::read_to_string(path).ok()?;
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_owned())
    }
}

/// Resolve the GitLab instance base URL: `GITLAB_URL` env first, then the
/// `instance_url` recorded in `~/.ragent/gitlab_config.json`, else the
/// public `https://gitlab.com` default.
#[must_use]
pub fn load_gitlab_base_url() -> String {
    if let Ok(url) = std::env::var("GITLAB_URL")
        && !url.trim().is_empty()
    {
        return url.trim().to_owned();
    }
    if let Some(path) = dirs::home_dir().map(|h| h.join(".ragent").join("gitlab_config.json"))
        && let Ok(raw) = std::fs::read_to_string(path)
        && let Ok(value) = serde_json::from_str::<Value>(&raw)
        && let Some(url) = value.get("instance_url").and_then(Value::as_str)
        && !url.trim().is_empty()
    {
        return url.trim().to_owned();
    }
    GITLAB_API_BASE.to_owned()
}

/// Extract the `web_url` field GitLab uses as the project's browser URL
/// (the `origin` push URL).
fn web_url(value: &Value) -> Result<String, RemoteFailure> {
    value
        .get("web_url")
        .and_then(Value::as_str)
        .map(str::to_owned)
        .filter(|url| !url.is_empty())
        .ok_or_else(|| RemoteFailure {
            step: RemoteStep::GitLabRepoCreate,
            message: "GitLab response missing web_url".to_owned(),
        })
}

/// True when the GitLab error body indicates the project path is already
/// taken (the GitLab reuse trigger for the FR-015 idempotent retry).
/// GitLab reports validation errors either as a flat string message or as
/// a per-field object (`{"path": ["has already been taken"], ...}`).
fn path_taken(value: &Value) -> bool {
    match value.get("message") {
        Some(Value::String(message)) => message.contains("has already been taken"),
        Some(Value::Object(fields)) => fields.values().any(|reasons| {
            reasons.as_array().is_some_and(|reasons| {
                reasons
                    .iter()
                    .filter_map(Value::as_str)
                    .any(|reason| reason.contains("has already been taken"))
            })
        }),
        _ => false,
    }
}

/// Create a private GitLab project, register it as `origin`, and push the
/// initial commit — resolving the token and instance URL from the
/// environment/credential files (FR-008, T-010).
///
/// # Errors
///
/// [`RemoteFailure`] naming the failed step (FR-010 containment): the local
/// scaffold is never rolled back, and a push failure keeps the configured
/// `origin` so the user can retry the push directly.
pub fn init_gitlab_remote(
    root: &Path,
    repo_name: &str,
    private: bool,
) -> Result<RemoteReport, RemoteFailure> {
    init_gitlab_remote_with_token(
        &load_gitlab_base_url(),
        load_gitlab_token().as_deref(),
        root,
        repo_name,
        private,
    )
}

/// Injectable variant of [`init_gitlab_remote`]: the caller supplies the
/// instance base URL and token directly. Tests point `base_url` at a local
/// mock server; production uses the GitLab instance resolution.
///
/// # Errors
///
/// [`RemoteFailure`] naming the failed step (FR-010 containment).
pub fn init_gitlab_remote_with_token(
    base_url: &str,
    token: Option<&str>,
    root: &Path,
    repo_name: &str,
    private: bool,
) -> Result<RemoteReport, RemoteFailure> {
    let token = token
        .map(str::trim)
        .filter(|t| !t.is_empty())
        .ok_or_else(|| RemoteFailure {
            step: RemoteStep::GitLabAuth,
            message: "no GitLab token found; run `/gitlab setup` or set GITLAB_TOKEN, then \
                      retry the hosting step - the local scaffold is intact"
                .to_owned(),
        })?;

    let client = Client::builder()
        .timeout(std::time::Duration::from_secs(30))
        .build()
        .map_err(|err| RemoteFailure {
            step: RemoteStep::GitLabRepoCreate,
            message: err.to_string(),
        })?;

    // 1. Identify the authenticated account (username needed for the
    //    FR-015 existing-project reuse path). `root` requires `sudo`.
    let (status, value) = api_json(
        &client,
        reqwest::Method::GET,
        base_url,
        "/api/v4/user",
        token,
        None,
        ApiFlavor::GitLab,
    )
    .map_err(|message| RemoteFailure {
        step: RemoteStep::GitLabAuth,
        message,
    })?;
    if status == 401 || status == 403 {
        return Err(RemoteFailure {
            step: RemoteStep::GitLabAuth,
            message: format!(
                "GitLab rejected the token ({status}); run `/gitlab setup` to re-authenticate - \
                 the local scaffold is intact"
            ),
        });
    }
    if status != 200 {
        return Err(RemoteFailure {
            step: RemoteStep::GitLabAuth,
            message: format!("GitLab API error {status}: {}", api_message(&value)),
        });
    }
    let username = value
        .get("username")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_owned();
    if username.is_empty() {
        return Err(RemoteFailure {
            step: RemoteStep::GitLabAuth,
            message: "could not read the GitLab username from GET /user; the token may lack \
                      the required scopes - the local scaffold is intact"
                .to_owned(),
        });
    }

    // 2. Create the hosting project, or reuse an existing same-name project
    //    (GitLab answers 400 "has already been taken" on a retry).
    let payload = serde_json::json!({ "name": repo_name, "visibility": if private { "private" } else { "public" } });
    let (status, value) = api_json(
        &client,
        reqwest::Method::POST,
        base_url,
        "/api/v4/projects",
        token,
        Some(&payload),
        ApiFlavor::GitLab,
    )
    .map_err(|message| RemoteFailure {
        step: RemoteStep::GitLabRepoCreate,
        message,
    })?;
    let (repo_created, url) = match status {
        201 => (true, web_url(&value)?),
        400 if path_taken(&value) => {
            // FR-015 idempotent retry: the project already exists on the
            // account; resolve its URL and continue with the push.
            let path = format!("/api/v4/projects/{}%2F{repo_name}", username);
            let (status, value) = api_json(
                &client,
                reqwest::Method::GET,
                base_url,
                &path,
                token,
                None,
                ApiFlavor::GitLab,
            )
            .map_err(|message| RemoteFailure {
                step: RemoteStep::GitLabRepoCreate,
                message: format!(
                    "project '{repo_name}' already exists but could not be read: {message}"
                ),
            })?;
            if status != 200 {
                return Err(RemoteFailure {
                    step: RemoteStep::GitLabRepoCreate,
                    message: format!(
                        "project '{repo_name}' already exists but GET /projects returned \
                         {status}: {}",
                        api_message(&value)
                    ),
                });
            }
            (false, web_url(&value)?)
        }
        401 | 403 => {
            return Err(RemoteFailure {
                step: RemoteStep::GitLabAuth,
                message: format!(
                    "GitLab rejected the token ({status}); run `/gitlab setup` to \
                     re-authenticate - the local scaffold is intact"
                ),
            });
        }
        other => {
            return Err(RemoteFailure {
                step: RemoteStep::GitLabRepoCreate,
                message: format!("GitLab API error {other}: {}", api_message(&value)),
            });
        }
    };

    // 3. Register `origin` and 4. push the initial commit (shared tail).
    let report = register_origin_and_push(root, &url)?;
    Ok(RemoteReport {
        repo_created,
        remote_existed: report.remote_existed,
        url: report.url,
    })
}
