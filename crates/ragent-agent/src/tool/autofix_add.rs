//! `autofix_add` - open a pull request (GitHub) or merge request (GitLab)
//! for the current branch, bind it to the session and turn Auto-fix on
//! (spec SESSIONMENU).
//!
//! The request merges the working tree's current branch into the branch it
//! was derived from on the `origin` remote. Git does not record that, so
//! the base is the remote branch the current commit diverged from most
//! recently (fewest commits on `HEAD` that the branch lacks), preferring the
//! remote's default branch on a tie and falling back to it; a `base`
//! parameter overrides the guess. The branch is pushed first when the
//! remote lacks it or it has unpushed commits (a plain push, never forced).
//! When the branch already has an open request, that one is bound instead
//! of opening a duplicate.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use anyhow::{Context, Result, bail};
use serde_json::{Value, json};

use super::{Tool, ToolContext, ToolOutput};
use crate::project::requests::{
    PullRequestCounts, RequestInfo, RequestSource, open_requests_for_branch,
};
use crate::project::{ProjectRemote, RemoteKind, detect_remote_with_hosts};

/// Tool that opens (or finds) the current branch's PR/MR and turns Auto-fix
/// on for it.
pub struct AutofixAddTool;

/// Most commit subjects listed in a generated request description.
const MAX_BODY_COMMITS: usize = 50;

/// Picks the base branch from `candidates`: `(branch, ahead)` pairs where
/// `ahead` is how many commits `HEAD` has that the branch lacks. The branch
/// with the fewest wins (the one `HEAD` diverged from most recently); a tie
/// prefers `default`, then the first name in order. Branches `HEAD` is
/// already part of (`ahead == 0`) are skipped. Falls back to `default`.
///
/// # Arguments
///
/// * `candidates` - remote branches (short names, no remote prefix) and
///   their ahead counts.
/// * `default` - the remote's default branch, if known.
#[must_use]
pub fn pick_base(candidates: &[(String, u64)], default: Option<&str>) -> Option<String> {
    candidates
        .iter()
        .filter(|(_, ahead)| *ahead > 0)
        .min_by(|(a, an), (b, bn)| {
            an.cmp(bn)
                .then_with(|| (Some(b.as_str()) == default).cmp(&(Some(a.as_str()) == default)))
                .then_with(|| a.cmp(b))
        })
        .map(|(name, _)| name.clone())
        .or_else(|| default.map(str::to_string))
}

/// Runs git in `dir`; `Some(trimmed stdout)` on success, `None` when git
/// exits non-zero.
async fn git(dir: &Path, args: &[&str]) -> Result<Option<String>> {
    let output = ragent_tools_vcs::git::run_git_output_async(
        args.iter().map(|a| (*a).to_string()).collect(),
        dir.to_path_buf(),
    )
    .await?;
    if !output.success {
        return Ok(None);
    }
    Ok(Some(
        String::from_utf8_lossy(&output.stdout).trim().to_string(),
    ))
}

/// Runs git in `dir` and fails with its (redacted) stderr when it exits
/// non-zero.
async fn git_checked(dir: &Path, args: &[&str]) -> Result<String> {
    let output = ragent_tools_vcs::git::run_git_output_async(
        args.iter().map(|a| (*a).to_string()).collect(),
        dir.to_path_buf(),
    )
    .await?;
    if !output.success {
        let stderr = String::from_utf8_lossy(&output.stderr);
        bail!(
            "git {} failed: {}",
            args.first().copied().unwrap_or_default(),
            ragent_types::sanitize::redact_secrets(stderr.trim())
        );
    }
    Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
}

/// Commits on `HEAD` that `reference` lacks.
async fn ahead_of(dir: &Path, reference: &str) -> Result<Option<u64>> {
    Ok(
        git(dir, &["rev-list", "--count", &format!("{reference}..HEAD")])
            .await?
            .and_then(|n| n.parse().ok()),
    )
}

/// The base branch for `branch`: see [`pick_base`].
async fn guess_base(dir: &Path, remote: &ProjectRemote, branch: &str) -> Result<Option<String>> {
    let prefix = format!("{}/", remote.remote_name);
    let refs = git(
        dir,
        &[
            "for-each-ref",
            "--format=%(refname:short)",
            &format!("refs/remotes/{}", remote.remote_name),
        ],
    )
    .await?
    .unwrap_or_default();
    let mut candidates = Vec::new();
    for full in refs.lines().map(str::trim) {
        let Some(name) = full.strip_prefix(&prefix) else {
            continue; // `origin` itself: the symbolic `origin/HEAD`
        };
        if name == branch || name == "HEAD" {
            continue;
        }
        if let Some(ahead) = ahead_of(dir, full).await? {
            candidates.push((name.to_string(), ahead));
        }
    }
    Ok(pick_base(&candidates, remote.default_branch.as_deref()))
}

/// What [`add_request`] did.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AddOutcome {
    /// A new request was opened (after pushing when `pushed`).
    Opened {
        /// The request.
        info: RequestInfo,
        /// Its target branch.
        base: String,
        /// Whether the branch was pushed first.
        pushed: bool,
    },
    /// The branch already had one open request; it was bound.
    Existing(RequestInfo),
}

/// Opens or finds the current branch's request on `remote`, binds it to
/// the top-level session of `ctx.session_id` and turns Auto-fix on.
///
/// # Arguments
///
/// * `ctx` - the tool context: working directory, session and storage.
/// * `input` - the tool input (`base`, `title`, `body`, `draft`).
/// * `remote` - the working directory's `origin` remote.
/// * `source` - the API client for `remote`.
///
/// # Errors
///
/// Returns an error on a detached `HEAD`, when the branch has several open
/// requests, when there is nothing to merge into the base, or when a git
/// or API call fails.
pub async fn add_request(
    ctx: &ToolContext,
    input: &Value,
    remote: &ProjectRemote,
    source: &RequestSource,
) -> Result<AddOutcome> {
    let storage = ctx
        .storage
        .as_ref()
        .context("Storage is not available in this session.")?;
    let dir = ctx.working_dir.as_path();
    let noun = remote.kind.noun();
    let branch = git_checked(dir, &["rev-parse", "--abbrev-ref", "HEAD"]).await?;
    if branch.is_empty() || branch == "HEAD" {
        bail!("HEAD is detached; check out a branch first");
    }
    ragent_tools_vcs::git::reject_option_like(&branch, "branch")?;

    let open = open_requests_for_branch(source, remote, &branch)
        .await
        .map_err(|e| anyhow::anyhow!("could not look up open {}s: {e}", noun.long))?;
    let outcome = match open.as_slice() {
        [one] => AddOutcome::Existing(one.clone()),
        [] => open_new(dir, input, remote, source, &branch).await?,
        several => {
            let list: Vec<String> = several
                .iter()
                .map(|r| format!("{}{}", noun.sigil, r.number))
                .collect();
            bail!(
                "`{branch}` already has several open {}s ({}); bind one with /autofix bind",
                noun.long,
                list.join(", ")
            );
        }
    };
    let info = match &outcome {
        AddOutcome::Opened { info, .. } | AddOutcome::Existing(info) => info,
    };
    let root = storage
        .bind_session_request(&info.to_session_request(&ctx.session_id, remote), false)
        .context("could not store the binding")?;
    storage
        .set_session_autofix(&root, true)
        .context("could not turn Auto-fix on")?;
    Ok(outcome)
}

/// Pushes when needed and opens the request.
async fn open_new(
    dir: &Path,
    input: &Value,
    remote: &ProjectRemote,
    source: &RequestSource,
    branch: &str,
) -> Result<AddOutcome> {
    let remote_name = remote.remote_name.as_str();
    let base = match input["base"]
        .as_str()
        .map(str::trim)
        .filter(|b| !b.is_empty())
    {
        Some(base) => base.to_string(),
        None => guess_base(dir, remote, branch)
            .await?
            .context("could not tell which branch this one came from; pass 'base'")?,
    };
    ragent_tools_vcs::git::reject_option_like(&base, "base")?;
    if base == branch {
        bail!("`{branch}` cannot be merged into itself; pass a different 'base'");
    }
    let base_ref = format!("{remote_name}/{base}");
    if let Some(0) = ahead_of(dir, &base_ref).await? {
        bail!("`{branch}` has no commits that `{base_ref}` lacks; nothing to merge");
    }

    // Push when the remote lacks the branch or is behind it.
    let tracking = format!("{remote_name}/{branch}");
    let pushed = match ahead_of(dir, &tracking).await? {
        Some(0) => false,
        _ => {
            git_checked(dir, &["push", "--set-upstream", remote_name, branch]).await?;
            true
        }
    };

    let title = match input["title"]
        .as_str()
        .map(str::trim)
        .filter(|t| !t.is_empty())
    {
        Some(title) => title.to_string(),
        None => git_checked(dir, &["log", "-1", "--format=%s"]).await?,
    };
    let body = match input["body"].as_str() {
        Some(body) => body.to_string(),
        None => {
            let log = git(dir, &["log", "--format=- %s", &format!("{base_ref}..HEAD")])
                .await?
                .unwrap_or_default();
            let lines: Vec<&str> = log.lines().take(MAX_BODY_COMMITS).collect();
            format!("Commits:\n\n{}", lines.join("\n"))
        }
    };
    let draft = input["draft"].as_bool().unwrap_or(false);
    let info = match source {
        RequestSource::GitHub(client) => {
            let created = client
                .post(
                    &format!("/repos/{}/pulls", remote.path),
                    &json!({
                        "title": title, "head": branch, "base": base,
                        "body": body, "draft": draft
                    }),
                )
                .await?;
            RequestInfo::from_github(&created)
        }
        RequestSource::GitLab(client) => {
            let title = if draft {
                format!("Draft: {title}")
            } else {
                title
            };
            let created = client
                .post(
                    &format!(
                        "/projects/{}/merge_requests",
                        ragent_tools_vcs::percent::encode_project_path(&remote.path)
                    ),
                    &json!({
                        "source_branch": branch, "target_branch": base,
                        "title": title, "description": body
                    }),
                )
                .await?;
            RequestInfo::from_gitlab(&created)
        }
    }
    .with_context(|| {
        format!(
            "unexpected response creating the {}",
            remote.kind.noun().long
        )
    })?;
    Ok(AddOutcome::Opened { info, base, pushed })
}

/// The `origin` remote of `dir`, recognising the configured GitHub
/// Enterprise hosts and GitLab instance.
async fn detect(dir: PathBuf, ctx: &ToolContext) -> Result<ProjectRemote> {
    let storage = ctx
        .storage
        .as_ref()
        .context("Storage is not available in this session.")?;
    let gitlab_host =
        ragent_tools_vcs::gitlab::auth::load_config(storage.as_ref()).map(|c| c.instance_url);
    let hosts = ctx
        .config
        .as_ref()
        .map(|c| c.vcs.github.hosts.clone())
        .unwrap_or_default();
    tokio::task::spawn_blocking(move || {
        detect_remote_with_hosts(&dir, gitlab_host.as_deref(), &hosts)
    })
    .await?
    .context("the working directory has no GitHub or GitLab `origin` remote")
}

#[async_trait::async_trait]
impl Tool for AutofixAddTool {
    fn name(&self) -> &'static str {
        "autofix_add"
    }

    fn description(&self) -> &'static str {
        "Open a pull request (GitHub) or merge request (GitLab) on the `origin` remote that \
         merges the current branch into the branch it was derived from, bind it to this \
         session and turn Auto-fix on, so the session is woken when CI fails, the branch \
         conflicts or a reviewer comments. The base branch is the remote branch the current \
         commit diverged from most recently (else the default branch); pass 'base' to choose \
         it. The branch is pushed first when needed (never forced). If the branch already \
         has an open PR/MR, that one is bound instead. Optional: 'base', 'title' (default: \
         last commit subject), 'body' (default: commit list), 'draft' (boolean)."
    }

    fn parameters_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "base": {
                    "type": "string",
                    "description": "Target branch (default: the branch the current one was derived from)"
                },
                "title": {
                    "type": "string",
                    "description": "PR/MR title (default: the last commit's subject)"
                },
                "body": {
                    "type": "string",
                    "description": "PR/MR description in markdown (default: the list of commits)"
                },
                "draft": {
                    "type": "boolean",
                    "description": "Open as a draft (default false)"
                }
            },
            "additionalProperties": false
        })
    }

    fn permission_category(&self) -> &'static str {
        "git:write"
    }

    async fn execute(&self, input: Value, ctx: &ToolContext) -> Result<ToolOutput> {
        let storage = ctx
            .storage
            .as_ref()
            .context("Storage is not available in this session.")?;
        let remote = detect(ctx.working_dir.clone(), ctx).await?;
        let hosts = ctx
            .config
            .as_ref()
            .map(|c| c.vcs.github.hosts.clone())
            .unwrap_or_default();
        let source = PullRequestCounts::new(Arc::clone(storage), Arc::clone(&ctx.event_bus))
            .with_github_hosts(hosts)
            .source_for(&remote)
            .with_context(|| format!("no {} credentials for {}", remote.kind, remote.host))?;
        let outcome = add_request(ctx, &input, &remote, &source).await?;
        Ok(report(remote.kind, &outcome))
    }
}

/// The tool output for `outcome`.
fn report(kind: RemoteKind, outcome: &AddOutcome) -> ToolOutput {
    let noun = kind.noun();
    let (info, verb, base, pushed) = match outcome {
        AddOutcome::Opened { info, base, pushed } => (info, "Opened", Some(base), *pushed),
        AddOutcome::Existing(info) => (info, "Found open", None, false),
    };
    let label = format!("{} {}{}", noun.short, noun.sigil, info.number);
    let into = base.map(|b| format!(" into `{b}`")).unwrap_or_default();
    let push_note = if pushed {
        " Pushed the branch first."
    } else {
        ""
    };
    ToolOutput {
        content: format!(
            "{verb} {label} \"{}\": `{}`{into}. {}\n\nBound to this session with Auto-fix \
             on: the session is woken when CI fails, the branch conflicts or a reviewer \
             comments.{push_note}",
            info.title, info.source_branch, info.web_url
        ),
        metadata: Some(json!({
            "action": if base.is_some() { "opened" } else { "existing" },
            "forge": kind.as_str(),
            "number": info.number,
            "title": info.title,
            "url": info.web_url,
            "branch": info.source_branch,
            "base": base,
            "pushed": pushed,
            "autofix": "on",
        })),
    }
}
