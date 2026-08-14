//! Git branch creation for the branch-per-spec workflow (FR-009).
//!
//! When `sdd.branch_per_spec` is enabled in `ragent.json`, the `/spec specify`
//! command calls [`create_spec_branch`] to isolate spec work on a dedicated
//! git branch named `spec/<specname>`.
//!
//! The function is deliberately tolerant: if the working directory is not a
//! git repository or the branch already exists, spec creation proceeds
//! without branching.

use std::path::Path;
use std::process::Command;

/// Outcome of a branch-creation attempt.
///
/// Returned by [`create_spec_branch`] so callers can surface an appropriate
/// user-facing message without inspecting stderr.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BranchResult {
    /// Branch was created and checked out successfully.
    Created {
        /// The full branch name (e.g. `spec/my-feature`).
        branch_name: String,
    },
    /// The working directory is not inside a git repository.
    NotARepo,
    /// A branch with this name already exists.
    AlreadyExists {
        /// The full branch name.
        branch_name: String,
    },
    /// A git command failed for an unexpected reason.
    Failed {
        /// Human-readable error detail.
        msg: String,
    },
}

/// Build the conventional branch name for a spec: `spec/<specname>`.
///
/// # Example
/// ```
/// # use ragent_specs::git::spec_branch_name;
/// assert_eq!(spec_branch_name("my-feature"), "spec/my-feature");
/// ```
#[must_use]
pub fn spec_branch_name(specname: &str) -> String {
    format!("spec/{specname}")
}

/// Attempt to create and check out a git branch named `spec/<specname>` in
/// the given working directory.
///
/// This is a best-effort operation:
/// - If the directory is not inside a git work tree, returns
///   [`BranchResult::NotARepo`] and spec creation proceeds without branching.
/// - If the branch already exists, returns [`BranchResult::AlreadyExists`].
/// - If `git checkout -b` fails for any other reason, returns
///   [`BranchResult::Failed`] with the stderr output.
///
/// # Arguments
/// * `specname` — The spec identifier (used to derive the branch name).
/// * `working_dir` — The directory in which to run git commands (typically
///   the project root).
///
/// # Returns
/// A [`BranchResult`] indicating the outcome.
pub fn create_spec_branch(specname: &str, working_dir: &Path) -> BranchResult {
    let branch_name = spec_branch_name(specname);

    // Check whether we are inside a git work tree.
    let is_repo = Command::new("git")
        .args(["rev-parse", "--is-inside-work-tree"])
        .current_dir(working_dir)
        .output();

    let is_repo = matches!(is_repo, Ok(o) if o.status.success());

    if !is_repo {
        return BranchResult::NotARepo;
    }

    // Check whether the branch already exists.
    let branch_exists = Command::new("git")
        .args(["rev-parse", "--verify", &branch_name])
        .current_dir(working_dir)
        .output();

    let branch_exists = matches!(branch_exists, Ok(o) if o.status.success());

    if branch_exists {
        return BranchResult::AlreadyExists { branch_name };
    }

    // Create and check out the new branch.
    let output = Command::new("git")
        .args(["checkout", "-b", &branch_name])
        .current_dir(working_dir)
        .output();

    match output {
        Ok(o) if o.status.success() => BranchResult::Created { branch_name },
        Ok(o) => {
            let stderr = String::from_utf8_lossy(&o.stderr).trim().to_string();
            BranchResult::Failed {
                msg: if stderr.is_empty() {
                    format!("git checkout -b {branch_name} failed")
                } else {
                    stderr
                },
            }
        }
        Err(e) => BranchResult::Failed {
            msg: format!("failed to execute git: {e}"),
        },
    }
}
