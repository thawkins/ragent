//! Session-owned git worktrees (spec `projects`, T-006).

#[path = "support/project_fixture.rs"]
mod project_fixture;

use std::path::Path;

use project_fixture::{Managers, fixture_dir, git, init_repo, managers};
use ragent_agent::project::worktree::{
    create_session_worktree, path_slug, prune_worktrees, session_worktree_path,
};
use ragent_agent::project::{Project, ProjectRemote, RemoteKind};

/// A repository with one commit, registered as a project.
fn project_in(m: &Managers, base: &Path) -> Project {
    let repo = base.join("repo");
    init_repo(&repo);
    m.projects.resolve_for_dir(&repo).expect("project")
}

fn head_branch(dir: &Path) -> String {
    git(dir, &["rev-parse", "--abbrev-ref", "HEAD"])
}

#[test]
fn test_session_worktree_path_slug() {
    assert_eq!(path_slug("feature/add login"), "feature-add-login");
    assert_eq!(path_slug("a//b"), "a-b");
    assert_eq!(path_slug("../.."), "worktree");
    assert_eq!(path_slug("v1.2_rc-3"), "v1.2_rc-3");
}

#[test]
fn test_session_worktree_created_at_new_branch_and_grouped_under_project() {
    let base = fixture_dir("worktree_new_branch");
    let m = managers();
    let project = project_in(&m, &base);
    let root = base.join("trees");
    let lead = m
        .sessions
        .create_session(project.worktree.clone())
        .expect("session in the project worktree");

    let session = m
        .projects
        .create_session(&m.sessions, &project.id, Some("feature/login"), &root)
        .expect("session with a worktree");

    let worktree = session
        .worktree
        .clone()
        .expect("the session owns a worktree");
    assert_eq!(session.directory, worktree);
    assert_eq!(
        worktree,
        session_worktree_path(&root, &project, "feature/login")
            .canonicalize()
            .expect("canonical")
    );
    assert!(
        worktree.join("README.md").is_file(),
        "files are checked out"
    );
    assert_eq!(head_branch(&worktree), "feature/login");
    assert_eq!(
        head_branch(&project.worktree),
        "main",
        "the project is untouched"
    );
    assert_eq!(session.branch.as_deref(), Some("feature/login"));
    assert_eq!(session.project_id, project.id);

    // The worktree is a `.git`-file checkout of its own, but its sessions
    // group under the owning project, including ones created in it later.
    let later = m.sessions.create_session(worktree).expect("later session");
    assert_eq!(later.project_id, project.id);
    let groups = m.storage.list_sessions_grouped().expect("grouped");
    assert_eq!(groups.len(), 1, "one project, not one per worktree");
    let mut ids: Vec<&str> = groups[0].sessions.iter().map(|s| s.id.as_str()).collect();
    ids.sort_unstable();
    let mut expected = vec![lead.id.as_str(), session.id.as_str(), later.id.as_str()];
    expected.sort_unstable();
    assert_eq!(ids, expected);
}

#[test]
fn test_session_worktree_without_a_branch_uses_the_project_worktree() {
    let base = fixture_dir("worktree_no_branch");
    let m = managers();
    let project = project_in(&m, &base);

    let session = m
        .projects
        .create_session(&m.sessions, &project.id, None, &base.join("trees"))
        .expect("session");

    assert_eq!(session.directory, project.worktree);
    assert_eq!(session.branch, None);
    assert_eq!(session.worktree, None);
    assert!(!base.join("trees").exists(), "no worktree root was created");
}

#[test]
fn test_session_worktree_uses_an_existing_local_branch() {
    let base = fixture_dir("worktree_existing");
    let m = managers();
    let project = project_in(&m, &base);
    git(&project.worktree, &["branch", "existing"]);
    std::fs::write(project.worktree.join("later.txt"), "x").expect("write");
    git(&project.worktree, &["add", "later.txt"]);
    git(&project.worktree, &["commit", "--quiet", "-m", "later"]);

    let path = create_session_worktree(&project, "existing", &base.join("trees")).expect("add");

    assert_eq!(head_branch(&path), "existing");
    assert!(
        !path.join("later.txt").exists(),
        "the existing branch was checked out, not recreated from HEAD"
    );
}

/// A project whose `origin` is a local bare repository holding `main`, a
/// newer `trunk` (the default branch) and `shared`.
fn project_with_origin(m: &Managers, base: &Path) -> Project {
    let seed = base.join("seed");
    init_repo(&seed);
    git(&seed, &["branch", "shared"]);
    git(&seed, &["checkout", "--quiet", "-b", "trunk"]);
    std::fs::write(seed.join("trunk.txt"), "x").expect("write");
    git(&seed, &["add", "trunk.txt"]);
    git(&seed, &["commit", "--quiet", "-m", "trunk work"]);
    let bare = base.join("origin.git");
    git(base, &["clone", "--quiet", "--bare", "seed", "origin.git"]);
    let repo = base.join("repo");
    git(
        base,
        &["clone", "--quiet", "--branch", "main", "origin.git", "repo"],
    );
    let mut project = m.projects.resolve_for_dir(&repo).expect("project");
    assert_eq!(
        project.remote, None,
        "a local path is not a described remote"
    );
    // Describe the remote by hand so the test needs no network.
    project.remote = Some(ProjectRemote {
        kind: RemoteKind::GitHub,
        host: "github.com".to_string(),
        path: "acme/app".to_string(),
        url: bare.display().to_string(),
        remote_name: "origin".to_string(),
        default_branch: Some("trunk".to_string()),
    });
    project
}

#[test]
fn test_session_worktree_tracks_a_branch_that_exists_on_the_remote() {
    let base = fixture_dir("worktree_remote_branch");
    let m = managers();
    let project = project_with_origin(&m, &base);

    let path = create_session_worktree(&project, "shared", &base.join("trees")).expect("add");

    assert_eq!(head_branch(&path), "shared");
    assert_eq!(
        git(&path, &["rev-parse", "--abbrev-ref", "shared@{upstream}"]),
        "origin/shared"
    );
}

#[test]
fn test_session_worktree_new_branch_starts_from_the_remote_default_branch() {
    let base = fixture_dir("worktree_default_base");
    let m = managers();
    let project = project_with_origin(&m, &base);

    let path = create_session_worktree(&project, "fresh", &base.join("trees")).expect("add");

    assert_eq!(head_branch(&path), "fresh");
    assert!(
        path.join("trunk.txt").is_file(),
        "started from origin/trunk, not from the checked-out main"
    );
}

#[test]
fn test_session_worktree_rejects_bad_requests() {
    let base = fixture_dir("worktree_rejects");
    let m = managers();
    let project = project_in(&m, &base);
    let root = base.join("trees");

    for bad in ["-b", "--force", "has space", "bad..name", ""] {
        assert!(
            create_session_worktree(&project, bad, &root).is_err(),
            "{bad:?} must be rejected"
        );
    }
    assert!(!root.exists(), "nothing was created for a rejected branch");

    create_session_worktree(&project, "one", &root).expect("first");
    let again = create_session_worktree(&project, "one", &root).expect_err("second");
    assert!(again.to_string().contains("already exists"), "{again}");

    let plain = m
        .projects
        .resolve_for_dir(Path::new("/nonexistent/worktree_rejects/plain"))
        .expect("plain project");
    assert!(
        create_session_worktree(&plain, "x", &root).is_err(),
        "a project that is not a git checkout cannot have session worktrees"
    );
}

#[test]
fn test_session_worktree_prune_removes_clean_and_keeps_dirty() {
    let base = fixture_dir("worktree_prune");
    let m = managers();
    let project = project_in(&m, &base);
    let root = base.join("trees");
    let make = |branch: &str| {
        m.projects
            .create_session(&m.sessions, &project.id, Some(branch), &root)
            .expect("session")
    };
    let clean = make("clean");
    let dirty = make("dirty");
    let live = make("live");
    let dirty_path = dirty.worktree.clone().expect("worktree");
    std::fs::write(dirty_path.join("uncommitted.txt"), "work").expect("write");

    m.sessions
        .archive_session(&clean.id)
        .expect("archive clean");
    m.sessions
        .archive_session(&dirty.id)
        .expect("archive dirty");

    let report = prune_worktrees(&m.storage, &project).expect("prune");

    let clean_path = clean.worktree.clone().expect("worktree");
    assert_eq!(report.removed, vec![clean_path.clone()]);
    assert!(!clean_path.exists(), "the clean worktree is gone");
    assert_eq!(report.skipped.len(), 1);
    assert_eq!(report.skipped[0].path, dirty_path);
    assert!(report.skipped[0].reason.contains("uncommitted"));
    assert!(dirty_path.join("uncommitted.txt").is_file(), "work is kept");
    assert!(
        live.worktree.clone().expect("worktree").exists(),
        "a live session's worktree is never pruned"
    );

    let stored = m
        .storage
        .session_checkout(&clean.id)
        .expect("get")
        .expect("row");
    assert_eq!(stored.worktree, None, "the removed worktree is forgotten");
    assert_eq!(
        stored.branch.as_deref(),
        Some("clean"),
        "the branch is kept"
    );
    assert!(
        git(&project.worktree, &["branch", "--list", "clean"]).contains("clean"),
        "pruning a worktree does not delete its branch"
    );

    // A second run has nothing new to remove and still reports the dirty one.
    let second = prune_worktrees(&m.storage, &project).expect("prune again");
    assert_eq!(second.removed, Vec::<std::path::PathBuf>::new());
    assert_eq!(second.skipped.len(), 1);
}

#[test]
fn test_session_worktree_prune_keeps_a_worktree_a_live_session_works_in() {
    let base = fixture_dir("worktree_prune_live_guest");
    let m = managers();
    let project = project_in(&m, &base);
    let root = base.join("trees");
    let owner = m
        .projects
        .create_session(&m.sessions, &project.id, Some("shared"), &root)
        .expect("owner");
    let worktree = owner.worktree.clone().expect("worktree");
    std::fs::create_dir_all(worktree.join("src")).expect("subdir");
    // A plain session started from inside the worktree: it works there but
    // does not own it. One in the root, one in a subdirectory.
    let guest = m.sessions.create_session(worktree.clone()).expect("guest");
    let deep_guest = m
        .sessions
        .create_session(worktree.join("src"))
        .expect("guest in a subdirectory");
    assert_eq!(guest.worktree, None);
    m.sessions
        .archive_session(&owner.id)
        .expect("archive owner");

    let report = prune_worktrees(&m.storage, &project).expect("prune");
    assert_eq!(report, Default::default(), "nothing may be touched");
    assert!(
        worktree.is_dir(),
        "the checkout under the live sessions is kept"
    );

    // With only the subdirectory session left it is still in use.
    m.sessions
        .archive_session(&guest.id)
        .expect("archive guest");
    let report = prune_worktrees(&m.storage, &project).expect("prune");
    assert_eq!(report.removed, Vec::<std::path::PathBuf>::new());
    assert!(worktree.is_dir());

    // Once no live session works in it, it goes.
    m.sessions
        .archive_session(&deep_guest.id)
        .expect("archive deep guest");
    // `src` is untracked and empty; git does not count an empty directory.
    let report = prune_worktrees(&m.storage, &project).expect("prune");
    assert_eq!(report.removed, vec![worktree.clone()]);
    assert!(!worktree.exists());
}

#[test]
fn test_session_worktree_prune_moves_sessions_to_a_directory_that_exists() {
    let base = fixture_dir("worktree_prune_relocate");
    let m = managers();
    let project = project_in(&m, &base);
    let root = base.join("trees");
    let owner = m
        .projects
        .create_session(&m.sessions, &project.id, Some("done"), &root)
        .expect("owner");
    let worktree = owner.worktree.clone().expect("worktree");
    let guest = m.sessions.create_session(worktree.clone()).expect("guest");
    m.sessions
        .archive_session(&owner.id)
        .expect("archive owner");
    m.sessions
        .archive_session(&guest.id)
        .expect("archive guest");

    let report = prune_worktrees(&m.storage, &project).expect("prune");
    assert_eq!(report.removed, vec![worktree.clone()]);

    // Bringing either session back must not leave it in a deleted checkout.
    for id in [&owner.id, &guest.id] {
        m.storage.unarchive_session(id).expect("unarchive");
        let session = m.sessions.get_session(id).expect("get").expect("session");
        assert_eq!(session.directory, project.worktree, "{id}");
        assert!(session.directory.is_dir());
        assert_eq!(session.worktree, None);
        assert_eq!(session.project_id, project.id);
    }
    let owner = m
        .sessions
        .get_session(&owner.id)
        .expect("get")
        .expect("owner");
    assert_eq!(
        owner.branch.as_deref(),
        Some("done"),
        "the branch is remembered"
    );
}

/// A branch session survives a restart: after the database file is closed
/// and reopened, the session is still listed under its project with its
/// branch and worktree.
#[test]
fn test_session_worktree_branch_session_survives_reopening_the_database() {
    let base = fixture_dir("worktree_survives_reopen");
    let db = base.join("ragent.db");
    let root = base.join("trees");
    let repo = base.join("repo");
    init_repo(&repo);
    let (session_id, project_id) = {
        let storage = std::sync::Arc::new(ragent_storage::Storage::open(&db).expect("storage"));
        let events = std::sync::Arc::new(ragent_agent::event::EventBus::new(64));
        let sessions = ragent_agent::session::SessionManager::new(storage.clone(), events.clone());
        let projects = ragent_agent::project::ProjectManager::new(storage, events);
        let project = projects.resolve_for_dir(&repo).expect("project");
        let session = projects
            .create_session(&sessions, &project.id, Some("feature/restart"), &root)
            .expect("branch session");
        (session.id, project.id)
    };

    let storage = ragent_storage::Storage::open(&db).expect("reopen storage");
    let groups = storage.list_sessions_grouped().expect("grouped");
    let group = groups
        .iter()
        .find(|g| g.project_id == project_id)
        .expect("the project is listed after reopening");
    assert!(
        group.sessions.iter().any(|s| s.id == session_id),
        "the branch session is listed after reopening"
    );
    let checkout = storage
        .session_checkout(&session_id)
        .expect("checkout query")
        .expect("checkout row");
    assert_eq!(checkout.branch.as_deref(), Some("feature/restart"));
    assert!(checkout.worktree.is_some(), "the worktree is kept");
}

/// Creates a branch session for `branch` while `trigger` (SQL installed on a
/// second connection) makes recording it fail, then checks that the worktree
/// is gone, no session row was kept, and the error keeps the branch and the
/// cause.
fn assert_worktree_rolled_back(name: &str, branch: &str, trigger: &str) {
    let base = fixture_dir(name);
    let db = base.join("ragent.db");
    let root = base.join("trees");
    let repo = base.join("repo");
    init_repo(&repo);
    let storage = std::sync::Arc::new(ragent_storage::Storage::open(&db).expect("storage"));
    let events = std::sync::Arc::new(ragent_agent::event::EventBus::new(64));
    let sessions = ragent_agent::session::SessionManager::new(storage.clone(), events.clone());
    let projects = ragent_agent::project::ProjectManager::new(storage.clone(), events);
    let project = projects.resolve_for_dir(&repo).expect("project");
    let conn = rusqlite::Connection::open(&db).expect("second connection");
    conn.execute_batch(trigger).expect("install trigger");

    let err = projects
        .create_session(&sessions, &project.id, Some(branch), &root)
        .expect_err("recording the session is refused");

    let message = format!("{err:#}");
    assert!(message.contains(branch), "names the branch: {message}");
    assert!(
        message.contains("refused by test"),
        "keeps the cause: {message}"
    );
    let dest = session_worktree_path(&root, &project, branch);
    assert!(!dest.exists(), "the worktree directory is removed");
    let listed = git(&repo, &["worktree", "list", "--porcelain"]);
    assert!(
        !listed.contains(&path_slug(branch)),
        "git no longer lists the worktree: {listed}"
    );
    let rows: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM sessions WHERE branch = ?1",
            [branch],
            |r| r.get(0),
        )
        .expect("count sessions");
    assert_eq!(rows, 0, "no session row points at the removed worktree");
}

/// A refused session insert removes the worktree created for it.
#[test]
fn test_session_worktree_removed_when_the_session_cannot_be_recorded() {
    assert_worktree_rolled_back(
        "worktree_rollback_on_insert_failure",
        "feature/rollback",
        "CREATE TRIGGER refuse_sessions BEFORE INSERT ON sessions \
         BEGIN SELECT RAISE(ABORT, 'refused by test'); END;",
    );
}

/// A refused project update after the session insert rolls the insert back
/// too, so removing the worktree never strands a recorded session.
#[test]
fn test_session_worktree_removed_when_the_project_update_fails() {
    assert_worktree_rolled_back(
        "worktree_rollback_on_project_update_failure",
        "feature/rollback-update",
        "CREATE TRIGGER refuse_projects BEFORE UPDATE ON projects \
         BEGIN SELECT RAISE(ABORT, 'refused by test'); END;",
    );
}
