//! Session-owned git worktrees (spec `projects`, T-006).
//!
//! A session may work in its own checkout of the project's repository at a
//! chosen branch, created with `git worktree add` under a root directory
//! outside the project. The worktree is never removed implicitly: archiving
//! a session leaves it on disk, and [`prune_worktrees`] removes only clean
//! worktrees of archived sessions, never with `--force`.
//!
//! Every git call goes through the timeout-bounded runner in
//! `ragent-tools-vcs`.

use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};
use ragent_storage::Storage;
use ragent_tools_vcs::git::{reject_option_like, run_git_output};

use super::Project;

/// Runs git in `dir`, returning trimmed stdout on success and the trimmed
/// stderr as the error otherwise.
fn git(args: &[&str], dir: &Path) -> Result<String> {
    let output = run_git_output(args, dir)?;
    if output.success {
        Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
    } else {
        bail!(
            "git {} failed: {}",
            args.first().copied().unwrap_or_default(),
            String::from_utf8_lossy(&output.stderr).trim()
        )
    }
}

/// Whether a git query succeeds in `dir`.
fn git_ok(args: &[&str], dir: &Path) -> bool {
    run_git_output(args, dir).is_ok_and(|o| o.success)
}

/// Turns arbitrary text into one safe path component: ASCII letters, digits,
/// `.`, `_` and `-` are kept, every other run of characters becomes one `-`.
///
/// # Examples
///
/// ```
/// use ragent_agent::project::worktree::path_slug;
///
/// assert_eq!(path_slug("feature/add login"), "feature-add-login");
/// assert_eq!(path_slug("../.."), "worktree");
/// ```
#[must_use]
pub fn path_slug(text: &str) -> String {
    let mut slug = String::with_capacity(text.len());
    for c in text.chars() {
        if c.is_ascii_alphanumeric() || c == '_' || c == '-' || c == '.' {
            slug.push(c);
        } else if !slug.ends_with('-') {
            slug.push('-');
        }
    }
    let slug = slug.trim_matches(|c| c == '-' || c == '.').to_string();
    if slug.is_empty() {
        "worktree".to_string()
    } else {
        slug
    }
}

/// The directory a session worktree for `branch` of `project` lives in,
/// under `root`. The project's id keeps two projects with the same name
/// apart.
#[must_use]
pub fn session_worktree_path(root: &Path, project: &Project, branch: &str) -> PathBuf {
    let short_id: String = project.id.chars().take(8).collect();
    root.join(format!("{}-{short_id}", path_slug(&project.name)))
        .join(path_slug(branch))
}

/// The default root for session worktrees: `worktrees/` in the ragent data
/// directory, or `None` when the platform has no data directory.
#[must_use]
pub fn default_worktree_root() -> Option<PathBuf> {
    dirs::data_dir().map(|d| d.join("ragent").join("worktrees"))
}

/// Creates a git worktree of the project's repository at `branch`.
///
/// The branch is taken, in order, from: an existing local branch; the
/// project remote's branch of that name (fetched first, tracking it); or a
/// new branch started from the remote's default branch, or from `HEAD` when
/// the project has no remote.
///
/// # Arguments
///
/// * `project` - the owning project; its worktree must be a git checkout.
/// * `branch` - the branch to check out or create.
/// * `root` - directory session worktrees are created under.
///
/// # Returns
///
/// The canonical path of the new worktree.
///
/// # Errors
///
/// Returns an error when `branch` is not a valid branch name, the project is
/// not a git checkout, the destination already exists, or git refuses (for
/// example because the branch is already checked out in another worktree).
pub fn create_session_worktree(project: &Project, branch: &str, root: &Path) -> Result<PathBuf> {
    let repo = project.worktree.as_path();
    reject_option_like(branch, "branch")?;
    if !git_ok(&["check-ref-format", "--branch", branch], repo) {
        bail!("'{branch}' is not a valid branch name");
    }
    if !git_ok(&["rev-parse", "--git-dir"], repo) {
        bail!(
            "project '{}' is not a git checkout; a session worktree needs one",
            project.name
        );
    }
    let dest = session_worktree_path(root, project, branch);
    if dest.exists() {
        bail!(
            "a worktree for branch '{branch}' already exists at {}",
            dest.display()
        );
    }
    if let Some(parent) = dest.parent() {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("cannot create {}", parent.display()))?;
    }
    let dest_text = dest.display().to_string();

    let remote = project.remote.as_ref();
    if let Some(remote) = remote {
        // Best effort: the branch may not exist on the remote yet, and the
        // machine may be offline. Either way the local refs decide below.
        if !git_ok(&["fetch", &remote.remote_name, branch], repo) {
            tracing::debug!(branch, "branch not fetched from the remote");
        }
    }
    let local_ref = format!("refs/heads/{branch}");
    if git_ok(&["show-ref", "--verify", "--quiet", &local_ref], repo) {
        git(&["worktree", "add", &dest_text, branch], repo)?;
    } else {
        let remote_branch = remote.map(|r| format!("{}/{branch}", r.remote_name));
        let tracked = remote_branch.filter(|rb| {
            git_ok(
                &[
                    "show-ref",
                    "--verify",
                    "--quiet",
                    &format!("refs/remotes/{rb}"),
                ],
                repo,
            )
        });
        match tracked {
            Some(remote_branch) => {
                git(
                    &[
                        "worktree",
                        "add",
                        "--track",
                        "-b",
                        branch,
                        &dest_text,
                        &remote_branch,
                    ],
                    repo,
                )?;
            }
            None => {
                let base = remote
                    .and_then(|r| {
                        r.default_branch
                            .as_ref()
                            .map(|b| format!("{}/{b}", r.remote_name))
                    })
                    .filter(|b| {
                        git_ok(
                            &[
                                "show-ref",
                                "--verify",
                                "--quiet",
                                &format!("refs/remotes/{b}"),
                            ],
                            repo,
                        )
                    })
                    .unwrap_or_else(|| "HEAD".to_string());
                git(
                    &[
                        "worktree",
                        "add",
                        "--no-track",
                        "-b",
                        branch,
                        &dest_text,
                        &base,
                    ],
                    repo,
                )?;
            }
        }
    }
    Ok(dest.canonicalize().unwrap_or(dest))
}

/// Removes the session worktree at `path` from the project's repository,
/// discarding any changes in it. Used to undo [`create_session_worktree`]
/// when the session that was to own the worktree could not be recorded.
///
/// # Errors
///
/// Returns an error when git refuses to remove the worktree.
pub fn remove_session_worktree(project: &Project, path: &Path) -> Result<()> {
    let path_text = path.display().to_string();
    git(
        &["worktree", "remove", "--force", &path_text],
        &project.worktree,
    )?;
    Ok(())
}

/// Why a worktree was left in place by [`prune_worktrees`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SkippedWorktree {
    /// The worktree that was kept.
    pub path: PathBuf,
    /// Why it was kept.
    pub reason: String,
}

/// What [`prune_worktrees`] did.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct PruneReport {
    /// Worktrees that were removed.
    pub removed: Vec<PathBuf>,
    /// Worktrees that were kept, with the reason.
    pub skipped: Vec<SkippedWorktree>,
}

/// Removes the worktrees of a project's archived sessions when they are
/// clean and no live session uses them.
///
/// A worktree with uncommitted or untracked changes is kept and reported. A
/// worktree whose directory is already gone is forgotten. Removal uses
/// `git worktree remove` without `--force`. The sessions that worked in a
/// removed worktree are moved to the project worktree and keep their branch,
/// so an unarchived session still has a directory that exists.
///
/// # Errors
///
/// Returns an error if storage fails; a git failure on one worktree is
/// reported in [`PruneReport::skipped`] and does not stop the others.
pub fn prune_worktrees(storage: &Storage, project: &Project) -> Result<PruneReport> {
    let mut report = PruneReport::default();
    let fallback = project.worktree.display().to_string();
    // Several archived sessions can share one worktree; handle it once.
    let mut seen = std::collections::HashSet::new();
    for (_session_id, worktree) in storage.prunable_session_worktrees(&project.id)? {
        if !seen.insert(worktree.clone()) {
            continue;
        }
        let path = PathBuf::from(&worktree);
        if path.exists() {
            let dirty = match git(&["status", "--porcelain"], &path) {
                Ok(status) => !status.is_empty(),
                Err(e) => {
                    report.skipped.push(SkippedWorktree {
                        path,
                        reason: e.to_string(),
                    });
                    continue;
                }
            };
            if dirty {
                report.skipped.push(SkippedWorktree {
                    path,
                    reason: "has uncommitted changes".to_string(),
                });
                continue;
            }
            if let Err(e) = git(&["worktree", "remove", &worktree], &project.worktree) {
                report.skipped.push(SkippedWorktree {
                    path,
                    reason: e.to_string(),
                });
                continue;
            }
            report.removed.push(path);
        } else if project.worktree.exists() {
            // The directory was deleted by hand; drop git's stale record.
            if let Err(e) = git(&["worktree", "prune"], &project.worktree) {
                tracing::debug!(error = %e, "git worktree prune failed");
            }
        }
        storage.forget_removed_worktree(&worktree, &fallback)?;
    }
    Ok(report)
}
