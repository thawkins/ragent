//! Shared test-support helpers for `ragent-team` integration tests.
//!
//! Provides [`setup_workspace`] - a temp-directory + path helper used across
//! team test files.  Previously copy-pasted into 5 test files (see
//! `DUPPLAN.md` Milestone I, `cargo dupes` group 37).

use std::path::PathBuf;

use tempfile::TempDir;

/// Create a temp directory and return it along with a `.ragent/teams` path
/// inside it.
///
/// # Returns
///
/// A tuple of `(TempDir, PathBuf)` where the `TempDir` owns the temporary
/// directory (and cleans it up when dropped) and the `PathBuf` points to the
/// `.ragent/teams` subdirectory within it.
///
/// `#[allow(dead_code)]`: this helper is `#[path]`-included by every test file
/// that needs a team workspace, so each inclusion is its own module and the
/// files that only want [`setup_team_with_task_store`] would otherwise report
/// it unused.
#[allow(dead_code)]
#[must_use]
pub fn setup_workspace() -> (TempDir, PathBuf) {
    let dir = TempDir::new().unwrap();
    let teams_dir = dir.path().join(".ragent").join("teams");
    std::fs::create_dir_all(&teams_dir).unwrap();
    (dir, teams_dir)
}

/// Create a temp directory holding a *real* team directory, and return it with
/// an open [`TaskStore`] on that team.
///
/// `TaskStore::add_task` / `claim_*` / `complete` fire the team's lifecycle
/// hooks from the mutation (spec `teamssetup` FR-017), so a hook lookup now
/// needs a readable `config.json` beside `tasks.json`. A bare
/// `.ragent/teams` directory (what [`setup_workspace`] returns) has none, and a
/// store opened there would fail every mutating call. Tests that exercise the
/// task store itself use this helper; tests about hook behaviour install their
/// own hooks on top of it.
///
/// The team is named `task-store-team`; `team_dir` is `tmp/.ragent/teams/<name>`.
///
/// `#[allow(dead_code)]`: this helper is `#[path]`-included by every test file
/// that needs a task store, so each inclusion is its own module and the files
/// that only want [`setup_workspace`] would otherwise report it unused.
#[allow(dead_code)]
#[must_use]
pub fn setup_team_with_task_store() -> (TempDir, std::path::PathBuf, ragent_agent::team::TaskStore)
{
    // `TeamStore::create` writes into an existing `.ragent/` directory, so the
    // workspace shape `setup_workspace` produces is a prerequisite, not an
    // alternative.
    let (dir, _teams_dir) = setup_workspace();
    let store =
        ragent_agent::team::TeamStore::create("task-store-team", "lead-sess", dir.path(), true)
            .expect("create team");
    let team_dir = store.dir.clone();
    let tasks = ragent_agent::team::TaskStore::open(&team_dir).expect("open task store");
    (dir, team_dir, tasks)
}
