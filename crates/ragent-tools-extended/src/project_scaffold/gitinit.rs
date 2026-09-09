//! Local git initialisation and initial commit (FR-008 local half).
//!
//! The scaffolder initialises a git repository in the scaffold root and
//! creates the initial commit covering every emitted file (language layout,
//! FR-004 workspace, later FR-019 docs). Remote creation and push are
//! separate milestones (T-009/T-010); this module only produces the local,
//! remote-free state that FR-008's "neither flag supplied" branch leaves and
//! that the FR-015 retry path tolerates.
//!
//! Failure containment (FR-010 philosophy): a git failure never rolls back
//! the emitted scaffold; the error carries the failing step and git's own
//! diagnostics so the FR-011 summary can print actionable remediation.
//!
//! Commit identity: sandboxes and fresh machines often have no git identity.
//! Existing repository/global config always wins; when unset, a repo-local
//! scaffold identity (`ragent-scaffold <ragent@localhost>`) is configured so
//! the initial commit cannot fail for want of `user.name`/`user.email`.

use std::fmt;
use std::path::Path;
use std::process::Command;

/// Git sub-steps that can fail during local initialisation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GitStep {
    /// `git init` (or the repo-detection pre-flight).
    Init,
    /// `git add -A`.
    Stage,
    /// `git commit`.
    Commit,
}

impl GitStep {
    /// Human-readable step name for error and summary messages.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Init => "git init",
            Self::Stage => "git add",
            Self::Commit => "git commit",
        }
    }
}

impl fmt::Display for GitStep {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// A failed local-git step with git's own diagnostics attached.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GitFailure {
    /// Step that failed.
    pub step: GitStep,
    /// Combined stderr+stdout from the git invocation.
    pub message: String,
}

impl fmt::Display for GitFailure {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} failed: {}", self.step, self.message.trim())
    }
}

/// Result of a successful local initialisation pass.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GitInitReport {
    /// True when `git init` created a new repository; false when the root
    /// already contained one (FR-015 retry tolerance).
    pub repo_created: bool,
    /// True when this pass created a commit; false when the tree was clean
    /// (nothing to commit).
    pub committed: bool,
}

/// Run `git` in `root`, returning stdout on success.
///
/// # Errors
///
/// [`GitFailure`] with `step` and git's combined stderr+stdout when the
/// spawn fails or git exits non-zero.
fn run_git(root: &Path, step: GitStep, args: &[&str]) -> Result<String, GitFailure> {
    let output = Command::new("git")
        .args(args)
        .current_dir(root)
        .output()
        .map_err(|err| GitFailure {
            step,
            message: err.to_string(),
        })?;
    if output.status.success() {
        Ok(String::from_utf8_lossy(&output.stdout).into_owned())
    } else {
        Err(GitFailure {
            step,
            message: format!(
                "{}{}",
                String::from_utf8_lossy(&output.stderr),
                String::from_utf8_lossy(&output.stdout)
            ),
        })
    }
}

/// Read a git config value resolved for `root`; `None` when unset or git is
/// unavailable. Infallible by design: identity probing must never abort.
fn git_config_value(root: &Path, key: &str) -> Option<String> {
    Command::new("git")
        .args(["config", key])
        .current_dir(root)
        .output()
        .ok()
        .filter(|output| output.status.success())
        .map(|output| String::from_utf8_lossy(&output.stdout).trim().to_owned())
        .filter(|value| !value.is_empty())
}

/// Ensure a commit identity resolves inside `root`.
///
/// Existing repository/global config wins; only genuinely missing keys get a
/// repo-local scaffold fallback. Config writes are best-effort: if they fail
/// the subsequent commit fails with git's own actionable diagnostics.
fn ensure_commit_identity(root: &Path) {
    if git_config_value(root, "user.name").is_none() {
        let _ = Command::new("git")
            .args(["config", "user.name", "ragent-scaffold"])
            .current_dir(root)
            .output();
    }
    if git_config_value(root, "user.email").is_none() {
        let _ = Command::new("git")
            .args(["config", "user.email", "ragent@localhost"])
            .current_dir(root)
            .output();
    }
}

/// True when git refused to commit because there was nothing staged — an
/// expected clean-tree outcome, not a failure.
fn is_nothing_to_commit(message: &str) -> bool {
    [
        "nothing to commit",
        "nothing added to commit",
        "no changes added to commit",
    ]
    .iter()
    .any(|phrase| message.contains(phrase))
}

/// Initialise a git repository in `root` (when absent) and create the
/// initial commit from the current tree.
///
/// - Existing `.git` is reused, never re-initialised (FR-015 retry path).
/// - `git add -A` stages the emitted scaffold (respecting the generated
///   `.gitignore`, so `log/`/`target/`-style paths stay out).
/// - A clean tree yields `committed: false` instead of an error.
///
/// # Errors
///
/// [`GitFailure`] naming the failing step ([`GitStep::Init`],
/// [`GitStep::Stage`], or [`GitStep::Commit`]) with git's diagnostics; the
/// emitted scaffold is left intact.
pub fn init_and_commit(root: &Path, message: &str) -> Result<GitInitReport, GitFailure> {
    let repo_created = if root.join(".git").exists() {
        false
    } else {
        run_git(root, GitStep::Init, &["init"])?;
        true
    };
    ensure_commit_identity(root);
    run_git(root, GitStep::Stage, &["add", "-A"])?;
    let committed = match run_git(root, GitStep::Commit, &["commit", "-m", message]) {
        Ok(_) => true,
        Err(failure) if is_nothing_to_commit(&failure.message) => false,
        Err(failure) => return Err(failure),
    };
    Ok(GitInitReport {
        repo_created,
        committed,
    })
}
