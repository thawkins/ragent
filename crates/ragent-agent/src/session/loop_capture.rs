//! Pre-loop workspace capture and post-loop change summary (spec
//! `agentloop`, FR-018 / FR-019).
//!
//! Before a goal-driven loop performs its first write action the processor
//! captures a file snapshot of the workspace
//! ([`capture_pre_loop_snapshot`], backed by the existing
//! `ragent-storage` snapshot machinery) so the loop's changes can later be
//! summarised and rolled back. When the workspace is inside a git
//! repository, [`git_state`] additionally records the current branch and
//! HEAD; when it is not, the caller warns the user that rollback will be
//! snapshot-only.
//!
//! On termination [`compute_change_summary`] compares the live workspace
//! against the capture and produces the modified/created/deleted counts plus
//! an aggregate diffstat that the processor publishes as
//! [`Event::LoopChangeSummary`].
//!
//! Dependencies: `ragent-storage` (snapshot capture/restore), `walkdir`
//! (workspace walk), `similar` (diffstat), `tokio` (async git probe).
//! **Rollback flow** — after termination `Event::LoopChangeSummary` renders
//! the diffstat and the TUI offers a one-key rollback (`Enter` restores the
//! pre-loop snapshot, `Esc` declines and keeps changes, FR-020). See
//! [`rollback_to_capture`].

use anyhow::{Context, Result};
use ragent_storage::snapshot::{Snapshot, restore_snapshot, take_snapshot};
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use walkdir::WalkDir;

/// Maximum number of workspace files captured or re-walked for the summary.
const MAX_WORKSPACE_FILES: usize = 4096;
/// Files larger than this are skipped by the capture (they cannot be
/// restored meaningfully and would inflate the snapshot).
const MAX_CAPTURED_FILE_BYTES: u64 = 2 * 1024 * 1024;
/// Git probe timeout: a wedged git hook must not stall loop start.
const GIT_PROBE_TIMEOUT_SECS: u64 = 10;

/// Directories never captured or diffed (build outputs, VCS internals,
/// session artefacts, dependency caches).
const SKIP_DIRS: &[&str] = &[
    ".git",
    "target",
    "node_modules",
    "log",
    "logs",
    ".ragent",
    ".cargo",
    "vendor",
    "dist",
    "research",
];

/// The recorded git state of a workspace captured at loop start (FR-018).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GitState {
    /// Current branch name (e.g. `main`).
    pub branch: String,
    /// Current HEAD commit hash.
    pub head: String,
}

impl GitState {
    /// Render the state for event payloads and logs.
    #[must_use]
    pub fn describe(&self) -> String {
        format!("branch {} @ {}", self.branch, self.head)
    }
}

/// The pre-loop capture of one goal-driven loop run.
///
/// Held in [`crate::session::processor::SessionProcessor::
/// active_loop_captures`]: armed lazily by
/// [`SessionProcessor::ensure_pre_loop_capture`] right before the loop's
/// first write action, consulted on termination for the change summary, and
/// consumed by the rollback flow (T-013).
#[derive(Debug, Clone)]
pub struct LoopCapture {
    /// The captured pre-loop file snapshot (`None` when the workspace walk
    /// produced no files, in which case the change summary is empty).
    pub snapshot: Option<Snapshot>,
    /// Git branch + HEAD when the workspace is inside a git repository.
    pub git: Option<GitState>,
}

/// Collect the workspace files under `dir` for capture or diffing.
///
/// The walk is bounded (`MAX_WORKSPACE_FILES` files, 5 levels deep) and
/// skips [`SKIP_DIRS`] so build outputs, dependency caches, and session
/// logs never enter a snapshot.
#[must_use]
pub fn workspace_files(dir: &Path) -> Vec<PathBuf> {
    let mut files = Vec::new();
    let walker = WalkDir::new(dir)
        .max_depth(5)
        .follow_links(false)
        .into_iter()
        .filter_entry(|entry| {
            entry
                .file_name()
                .to_str()
                .is_none_or(|name| !SKIP_DIRS.contains(&name))
        });
    for entry in walker {
        let Ok(entry) = entry else { continue };
        if !entry.file_type().is_file() {
            continue;
        }
        if files.len() >= MAX_WORKSPACE_FILES {
            break;
        }
        files.push(entry.into_path());
    }
    files
}

/// Read a file for capture, skipping oversized or unreadable entries.
fn read_for_capture(path: &Path) -> Option<Vec<u8>> {
    let metadata = std::fs::metadata(path).ok()?;
    if !metadata.is_file() || metadata.len() > MAX_CAPTURED_FILE_BYTES {
        return None;
    }
    std::fs::read(path).ok()
}

/// Probe the workspace's git state (FR-018): the current branch and HEAD.
///
/// Returns `None` when the workspace is not inside a git repository, git is
/// not installed, or the probe exceeds [`GIT_PROBE_TIMEOUT_SECS`]. The
/// caller warns the user in that case that rollback will be snapshot-only.
#[must_use]
pub async fn git_state(dir: &Path) -> Option<GitState> {
    async fn run_git(dir: &Path, args: &[&str]) -> Option<String> {
        let output = tokio::process::Command::new("git")
            .args(["-C"])
            .arg(dir)
            .args(args)
            .output()
            .await
            .ok()?;
        if !output.status.success() {
            return None;
        }
        let text = String::from_utf8_lossy(&output.stdout).trim().to_string();
        if text.is_empty() { None } else { Some(text) }
    }
    let probe = async {
        let branch = run_git(dir, &["rev-parse", "--abbrev-ref", "HEAD"]).await?;
        let head = run_git(dir, &["rev-parse", "HEAD"]).await?;
        Some(GitState { branch, head })
    };
    tokio::time::timeout(
        std::time::Duration::from_secs(GIT_PROBE_TIMEOUT_SECS),
        probe,
    )
    .await
    .ok()
    .flatten()
}

/// Capture the pre-loop workspace snapshot (FR-018).
///
/// Snapshots every workspace file into a `ragent-storage`
/// [`Snapshot`] via the session's storage connection. Returns `None` when
/// the workspace contains no captureable files (the summary then reports no
/// changes).
///
/// # Errors
///
/// Returns an error when the storage write fails.
pub async fn capture_pre_loop_snapshot(
    session_id: &str,
    message_id: &str,
    dir: &Path,
) -> Result<Option<Snapshot>> {
    let files: Vec<PathBuf> = workspace_files(dir)
        .into_iter()
        .filter(|path| read_for_capture(path).is_some())
        .collect();
    if files.is_empty() {
        return Ok(None);
    }
    let session_id = session_id.to_string();
    let message_id = message_id.to_string();
    let snapshot = tokio::task::spawn_blocking(move || -> Result<Snapshot> {
        let existing: Vec<PathBuf> = files
            .iter()
            .filter(|path| path.is_file())
            .cloned()
            .collect();
        let mut snapshot = take_snapshot(&session_id, &message_id, &existing)?;
        // Replace each stored buffer with the guard-checked read so oversized
        // or unreadable entries never enter the snapshot.
        snapshot
            .files
            .retain(|path, _| read_for_capture(path).is_some());
        for (path, content) in snapshot.files.iter_mut() {
            if let Some(checked) = read_for_capture(path) {
                *content = checked;
            }
        }
        Ok(snapshot)
    })
    .await
    .context("pre-loop snapshot task panicked")??;
    if snapshot.files.is_empty() {
        return Ok(None);
    }
    Ok(Some(snapshot))
}

/// Whether `path` is inside (or equal to) `dir`.
fn path_is_under(dir: &Path, path: &Path) -> bool {
    path.starts_with(dir)
}

/// The post-loop change summary for a terminated loop run (FR-019).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ChangeSummary {
    /// Workspace files whose contents changed since the capture.
    pub modified: u64,
    /// Workspace files created since the capture.
    pub created: u64,
    /// Workspace files deleted since the capture.
    pub deleted: u64,
    /// Aggregate added/removed line counts across the changed text files.
    pub added_lines: u64,
    /// Aggregate removed-line count across the changed text files.
    pub deleted_lines: u64,
    /// Affected workspace paths (sorted, relative to the workspace root).
    pub files: Vec<String>,
}

impl ChangeSummary {
    /// Render the aggregate diffstat line for
    /// [`Event::LoopChangeSummary`].
    #[must_use]
    pub fn diffstat(&self) -> String {
        format!(
            "+{} -{} lines across {} file{}",
            self.added_lines,
            self.deleted_lines,
            self.files.len(),
            if self.files.len() == 1 { "" } else { "s" }
        )
    }
}

/// Compute the change summary by comparing the live workspace against the
/// pre-loop capture (FR-019).
///
/// `added_lines` / `deleted_lines` come from unified text diffs of the
/// changed files (binary files count as one changed file without line
/// counts). The summary is always computable: a missing capture yields the
/// empty summary.
#[must_use]
pub fn compute_change_summary(capture: &LoopCapture, dir: &Path) -> ChangeSummary {
    let Some(snapshot) = &capture.snapshot else {
        return ChangeSummary::default();
    };
    let mut summary = ChangeSummary::default();
    let current: HashSet<PathBuf> = workspace_files(dir).into_iter().collect();

    // Modified and deleted: every file recorded in the capture.
    for (path, stored) in &snapshot.files {
        if !current.contains(path) && !path.is_file() {
            summary.deleted += 1;
            summary.files.push(relative_path(dir, path));
            // A deleted text file's removed lines count toward the diffstat.
            if let Ok(stored_str) = std::str::from_utf8(stored) {
                summary.deleted_lines += u64::try_from(stored_str.lines().count()).unwrap_or(0);
            }
            continue;
        }
        match read_for_capture(path) {
            Some(live) if live != *stored => {
                summary.modified += 1;
                summary.files.push(relative_path(dir, path));
                let (added, removed) = count_diff_lines(stored, &live);
                summary.added_lines += added;
                summary.deleted_lines += removed;
            }
            _ => {}
        }
    }

    // Created: live files the capture never saw.
    for path in current {
        if !snapshot.files.contains_key(&path) {
            summary.created += 1;
            summary.files.push(relative_path(dir, &path));
            if let Some(content) = read_for_capture(&path) {
                // A new text file's line count is its added-line
                // contribution (diffing against an empty base would emit a
                // spurious extra change).
                if let Ok(content_str) = std::str::from_utf8(&content) {
                    summary.added_lines += u64::try_from(content_str.lines().count()).unwrap_or(0);
                }
            }
        }
    }

    summary.files.sort();
    summary
}

/// Render `path` relative to `dir` when possible, else its lossy display.
fn relative_path(dir: &Path, path: &Path) -> String {
    match path.strip_prefix(dir) {
        Ok(relative) => relative.display().to_string(),
        Err(_) => path.display().to_string(),
    }
}

/// Count added/removed lines between two file snapshots.
fn count_diff_lines(old: &[u8], new: &[u8]) -> (u64, u64) {
    let (Ok(old_str), Ok(new_str)) = (std::str::from_utf8(old), std::str::from_utf8(new)) else {
        // Binary change: count the file as one added line in the aggregate.
        return (1, 0);
    };
    let diff = similar::TextDiff::from_lines(old_str, new_str);
    let mut added = 0u64;
    let mut removed = 0u64;
    for change in diff.iter_all_changes() {
        match change.tag() {
            similar::ChangeTag::Insert => added += 1,
            similar::ChangeTag::Delete => removed += 1,
            similar::ChangeTag::Equal => {}
        }
    }
    (added, removed)
}

/// Restore the workspace from a pre-loop capture (FR-020, T-013).
///
/// Writes every captured file back to disk; created files stay unless the
/// caller also removes them (the snapshot-based restore covers modified and
/// deleted files).
///
/// # Errors
///
/// Returns an error when a captured file cannot be written back.
pub async fn rollback_to_capture(capture: &LoopCapture) -> Result<()> {
    if let Some(snapshot) = &capture.snapshot {
        let snapshot = snapshot.clone();
        tokio::task::spawn_blocking(move || {
            restore_snapshot(&snapshot).context("restoring the pre-loop workspace snapshot failed")
        })
        .await
        .context("pre-loop restore task panicked")??;
    }
    Ok(())
}

/// Whether `capture` recorded a usable snapshot (i.e. rollback is possible).
#[must_use]
pub fn capture_is_rollbackable(capture: &LoopCapture) -> bool {
    capture
        .snapshot
        .as_ref()
        .is_some_and(|s| !s.files.is_empty())
}

/// Whether `path` sits inside `dir` (re-exported helper for processors).
#[must_use]
pub fn path_within(dir: &Path, path: &Path) -> bool {
    path_is_under(dir, path)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_workspace_files_skips_build_and_session_dirs() {
        let dir = tempfile::tempdir().expect("tempdir");
        let root = dir.path();
        std::fs::write(root.join("top.txt"), "root").expect("root file");
        std::fs::create_dir_all(root.join("src/sub")).expect("src dir");
        std::fs::write(root.join("src/sub/deep.rs"), "code").expect("src file");
        std::fs::create_dir_all(root.join("target/debug")).expect("target dir");
        std::fs::write(root.join("target/debug/binary"), "bin").expect("target file");
        std::fs::create_dir_all(root.join(".git")).expect("git dir");
        std::fs::write(root.join(".git/HEAD"), "ref").expect("git file");

        let files = workspace_files(root);
        let names: Vec<String> = files.iter().map(|p| relative_path(root, p)).collect();
        assert!(names.contains(&"top.txt".to_string()));
        assert!(names.contains(&"src/sub/deep.rs".to_string()));
        assert!(
            !names.iter().any(|n| n.starts_with("target/")),
            "build outputs are skipped: {names:?}"
        );
        assert!(
            !names.iter().any(|n| n.starts_with(".git/")),
            "git internals are skipped: {names:?}"
        );
    }

    #[test]
    fn test_compute_change_summary_counts_match_induced_changes() {
        let dir = tempfile::tempdir().expect("tempdir");
        let root = dir.path();
        std::fs::write(root.join("a.txt"), "original\n").expect("a");
        std::fs::create_dir_all(root.join("sub")).expect("sub");
        std::fs::write(root.join("sub/b.txt"), "kept\n").expect("b");

        let snapshot = take_snapshot("s1", "m1", &[root.join("a.txt"), root.join("sub/b.txt")])
            .expect("snapshot");
        let capture = LoopCapture {
            snapshot: Some(snapshot),
            git: None,
        };

        // Induce: modify a.txt, create c.txt, delete sub/b.txt.
        std::fs::write(root.join("a.txt"), "changed\nlines\n").expect("a2");
        std::fs::write(root.join("c.txt"), "new file\n").expect("c");
        std::fs::remove_file(root.join("sub/b.txt")).expect("rm b");

        let summary = compute_change_summary(&capture, root);
        assert_eq!(summary.modified, 1, "one modified file");
        assert_eq!(summary.created, 1, "one created file");
        assert_eq!(summary.deleted, 1, "one deleted file");
        assert_eq!(
            summary.files,
            vec![
                "a.txt".to_string(),
                "c.txt".to_string(),
                "sub/b.txt".to_string()
            ],
            "affected files are sorted relative paths"
        );
        // a.txt: +2 -1 lines; c.txt: +1 line; sub/b.txt deleted: -1 line.
        assert_eq!(summary.added_lines, 3);
        assert_eq!(summary.deleted_lines, 2);
        assert!(summary.diffstat().contains("+3 -2"));
    }

    #[test]
    fn test_rollback_restores_captured_contents() {
        let dir = tempfile::tempdir().expect("tempdir");
        let root = dir.path();
        std::fs::write(root.join("a.txt"), "original\n").expect("a");
        let snapshot = take_snapshot("s1", "m1", &[root.join("a.txt")]).expect("snap");
        let capture = LoopCapture {
            snapshot: Some(snapshot),
            git: None,
        };
        std::fs::write(root.join("a.txt"), "mutated\n").expect("a2");
        std::fs::write(root.join("created.txt"), "extra\n").expect("created");

        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("runtime");
        runtime
            .block_on(async { rollback_to_capture(&capture).await })
            .expect("rollback");

        let restored = std::fs::read_to_string(root.join("a.txt")).expect("read a");
        assert_eq!(restored, "original\n", "the modified file is restored");
        assert!(
            root.join("created.txt").exists(),
            "created files are outside the snapshot's file set and are kept"
        );
        assert!(capture_is_rollbackable(&capture));
    }

    #[tokio::test]
    async fn test_git_state_is_none_outside_a_repository() {
        let dir = tempfile::tempdir().expect("tempdir");
        assert!(
            git_state(dir.path()).await.is_none(),
            "a plain tempdir is not a git repository"
        );
    }

    #[test]
    fn test_capture_without_snapshot_yields_empty_summary() {
        let dir = tempfile::tempdir().expect("tempdir");
        std::fs::write(dir.path().join("x.txt"), "content\n").expect("x");
        let capture = LoopCapture {
            snapshot: None,
            git: None,
        };
        let summary = compute_change_summary(&capture, dir.path());
        assert_eq!(summary, ChangeSummary::default());
    }

    #[test]
    fn test_git_state_describe() {
        let state = GitState {
            branch: "main".to_string(),
            head: "abc123".to_string(),
        };
        assert_eq!(state.describe(), "branch main @ abc123");
    }
}
