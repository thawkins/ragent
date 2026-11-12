//! Session project grouping, pinning and archiving (spec MULTISESSION, M4).
//!
//! Every fixture directory lives under the git-ignored `target/temp/` tree,
//! in a directory unique to the test, so parallel test processes never share
//! one and nothing is written into the repository.

use std::path::{Path, PathBuf};

use ragent_storage::project_root;
use ragent_storage::storage::Storage;

/// A fresh, empty fixture directory unique to `name`.
fn fixture_dir(name: &str) -> PathBuf {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../target/temp/session_grouping")
        .join(name);
    let _ = std::fs::remove_dir_all(&dir); // INTENTIONAL: may not exist yet
    std::fs::create_dir_all(&dir).expect("create fixture dir");
    dir.canonicalize().expect("canonical fixture dir")
}

/// Two git work trees (`repo_a` a normal clone with a `.git` directory and a
/// nested `sub` directory, `repo_b` a linked worktree with a `.git` file) and
/// one plain directory outside any work tree.
struct Layout {
    repo_a: PathBuf,
    repo_a_sub: PathBuf,
    repo_b: PathBuf,
    plain: PathBuf,
}

fn layout(name: &str) -> Layout {
    let root = fixture_dir(name);
    let repo_a = root.join("repo_a");
    let repo_a_sub = repo_a.join("sub");
    let repo_b = root.join("repo_b");
    let plain = root.join("plain");
    std::fs::create_dir_all(repo_a.join(".git")).expect("repo_a/.git");
    std::fs::create_dir_all(&repo_a_sub).expect("repo_a/sub");
    std::fs::create_dir_all(&repo_b).expect("repo_b");
    std::fs::write(repo_b.join(".git"), "gitdir: /elsewhere\n").expect("repo_b/.git file");
    std::fs::create_dir_all(&plain).expect("plain");
    Layout {
        repo_a,
        repo_a_sub,
        repo_b,
        plain,
    }
}

fn text(path: &Path) -> String {
    path.display().to_string()
}

#[test]
fn test_session_grouping_project_root_finds_the_work_tree() {
    let l = layout("project_root");
    assert_eq!(project_root(&text(&l.repo_a)), text(&l.repo_a));
    assert_eq!(project_root(&text(&l.repo_a_sub)), text(&l.repo_a));
    assert_eq!(
        project_root(&text(&l.repo_b)),
        text(&l.repo_b),
        "a `.git` file (linked worktree) marks a work tree too"
    );
    // Fixtures live under `target/temp/` inside the ragent checkout, so a
    // directory with no `.git` of its own belongs to that enclosing repo.
    let workspace = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .expect("workspace root");
    assert_eq!(project_root(&text(&l.plain)), text(&workspace));
    // Outside any work tree a directory is its own project.
    assert_eq!(project_root("/"), "/");
    let missing = text(&l.plain.join("deleted"));
    assert_eq!(
        project_root(&missing),
        missing,
        "a missing directory is kept as is"
    );
}

#[test]
fn test_session_grouping_create_session_records_project() {
    let l = layout("create_records");
    let storage = Storage::open_in_memory().expect("storage");
    storage
        .create_session("sub", &text(&l.repo_a_sub))
        .expect("session");
    let row = storage.get_session("sub").expect("get").expect("row");
    assert_eq!(row.project_id, text(&l.repo_a));
}

#[test]
fn test_session_grouping_groups_by_project_newest_first() {
    let l = layout("groups");
    let missing = text(&l.plain.join("deleted"));
    let storage = Storage::open_in_memory().expect("storage");
    // Created oldest first; each create bumps `updated_at`.
    storage
        .create_session("a-root", &text(&l.repo_a))
        .expect("a-root");
    std::thread::sleep(std::time::Duration::from_millis(5));
    storage.create_session("b", &text(&l.repo_b)).expect("b");
    std::thread::sleep(std::time::Duration::from_millis(5));
    storage.create_session("gone", &missing).expect("gone");
    std::thread::sleep(std::time::Duration::from_millis(5));
    storage
        .create_session("a-sub", &text(&l.repo_a_sub))
        .expect("a-sub");

    let groups = storage.list_sessions_grouped().expect("grouped");
    let shape: Vec<(String, Vec<String>)> = groups
        .iter()
        .map(|g| {
            (
                g.project_id.clone(),
                g.sessions.iter().map(|s| s.id.clone()).collect(),
            )
        })
        .collect();
    assert_eq!(
        shape,
        vec![
            (
                text(&l.repo_a),
                vec!["a-sub".to_string(), "a-root".to_string()]
            ),
            (missing.clone(), vec!["gone".to_string()]),
            (text(&l.repo_b), vec!["b".to_string()]),
        ],
        "groups ordered by their newest session; sessions newest first"
    );
}

#[test]
fn test_session_grouping_pin_does_not_reorder() {
    let storage = Storage::open_in_memory().expect("storage");
    storage.create_session("s", "/nonexistent/pin").expect("s");
    let before = storage
        .get_session("s")
        .expect("get")
        .expect("row")
        .updated_at;

    storage.set_session_pinned("s", true).expect("pin");
    assert!(storage.pinned_session_ids().expect("ids").contains("s"));
    let after = storage
        .get_session("s")
        .expect("get")
        .expect("row")
        .updated_at;
    assert_eq!(before, after, "pinning must not bump updated_at");

    storage.set_session_pinned("s", false).expect("unpin");
    assert!(storage.pinned_session_ids().expect("ids").is_empty());
}

#[test]
fn test_session_grouping_archive_round_trip() {
    let storage = Storage::open_in_memory().expect("storage");
    storage.create_session("s", "/nonexistent/arch").expect("s");
    storage.set_session_pinned("s", true).expect("pin");

    storage.archive_session("s").expect("archive");
    assert!(storage.list_sessions_grouped().expect("grouped").is_empty());
    assert!(
        storage.pinned_session_ids().expect("ids").is_empty(),
        "an archived session is not listed as pinned"
    );
    let archived = storage.list_archived_sessions().expect("archived");
    assert_eq!(archived.len(), 1);
    assert!(archived[0].archived_at.is_some());

    storage.unarchive_session("s").expect("unarchive");
    assert!(
        storage
            .list_archived_sessions()
            .expect("archived")
            .is_empty()
    );
    assert_eq!(storage.list_sessions_grouped().expect("grouped").len(), 1);
}

/// A database written before schema v4 (no `pinned` column, no project
/// index, empty `project_id`) migrates on open: the column and index appear
/// and every session's project is backfilled from its directory.
#[test]
fn test_session_grouping_migrates_a_v3_database() {
    let l = layout("migrate_v3");
    let path = fixture_dir("migrate_v3_db").join("ragent.db");
    {
        let storage = Storage::open(&path).expect("open");
        storage
            .create_session("old", &text(&l.repo_a_sub))
            .expect("session");
        let guard = storage.conn_lock_for_test().expect("lock");
        guard
            .execute_batch(
                "DROP INDEX IF EXISTS idx_sessions_project_updated;
                 ALTER TABLE sessions DROP COLUMN pinned;
                 UPDATE sessions SET project_id = '';
                 UPDATE settings SET value = '3' WHERE key = 'schema_version';",
            )
            .expect("rewind to a v3 schema");
    }

    let storage = Storage::open(&path).expect("migrate");

    let row = storage.get_session("old").expect("get").expect("row");
    assert_eq!(row.project_id, text(&l.repo_a), "project backfilled");
    storage
        .set_session_pinned("old", true)
        .expect("the pinned column exists after migration");
    let guard = storage.conn_lock_for_test().expect("lock");
    let index_count: i64 = guard
        .query_row(
            "SELECT COUNT(*) FROM sqlite_master \
             WHERE type = 'index' AND name = 'idx_sessions_project_updated'",
            [],
            |r| r.get(0),
        )
        .expect("count index");
    assert_eq!(index_count, 1, "project index created");
    let version: String = guard
        .query_row(
            "SELECT value FROM settings WHERE key = 'schema_version'",
            [],
            |r| r.get(0),
        )
        .expect("version");
    assert_eq!(version, ragent_storage::SCHEMA_VERSION.to_string());
}

#[test]
fn test_session_grouping_children_record_parent_and_kind() {
    use ragent_storage::SessionKind;
    let storage = Storage::open_in_memory().expect("storage");
    storage
        .create_session("lead", "/nonexistent/kids")
        .expect("lead");
    storage
        .create_session_with(
            "sub",
            "/nonexistent/kids",
            Some("lead"),
            SessionKind::Subagent,
        )
        .expect("sub");
    storage
        .create_session_with(
            "mate",
            "/nonexistent/kids",
            Some("lead"),
            SessionKind::Teammate,
        )
        .expect("mate");
    storage
        .create_session_with("run", "/nonexistent/kids", None, SessionKind::Automation)
        .expect("automation run");

    let children: Vec<String> = storage
        .list_child_sessions("lead")
        .expect("children")
        .into_iter()
        .map(|row| row.id)
        .collect();
    assert_eq!(children, vec!["sub".to_string(), "mate".to_string()]);
    let sub = storage.get_session("sub").expect("get").expect("row");
    assert_eq!(sub.parent_id.as_deref(), Some("lead"));
    assert_eq!(
        storage.session_kind("mate").expect("kind"),
        Some(SessionKind::Teammate)
    );
    assert_eq!(
        storage.session_kind("lead").expect("kind"),
        Some(SessionKind::Interactive)
    );

    let listed: Vec<String> = storage
        .list_sessions_grouped()
        .expect("grouped")
        .into_iter()
        .flat_map(|g| g.sessions.into_iter().map(|s| s.id))
        .collect();
    assert_eq!(
        listed,
        vec!["lead".to_string()],
        "children and automation runs are not top-level sessions"
    );
}

#[test]
fn test_session_grouping_child_of_missing_parent_keeps_the_link() {
    use ragent_storage::SessionKind;
    let storage = Storage::open_in_memory().expect("storage");
    storage
        .create_session_with(
            "orphan",
            "/nonexistent/o",
            Some("gone"),
            SessionKind::Teammate,
        )
        .expect("a missing parent does not stop the child");
    let row = storage.get_session("orphan").expect("get").expect("row");
    assert_eq!(row.parent_id.as_deref(), Some("gone"));
}

#[test]
fn test_session_grouping_archive_takes_the_whole_tree() {
    use ragent_storage::SessionKind;
    let storage = Storage::open_in_memory().expect("storage");
    storage
        .create_session("root", "/nonexistent/tree")
        .expect("root");
    storage
        .create_session_with(
            "child",
            "/nonexistent/tree",
            Some("root"),
            SessionKind::Subagent,
        )
        .expect("child");
    storage
        .create_session_with(
            "grandchild",
            "/nonexistent/tree",
            Some("child"),
            SessionKind::Subagent,
        )
        .expect("grandchild");
    storage
        .create_session("other", "/nonexistent/tree")
        .expect("other");

    storage.archive_session("root").expect("archive");
    let archived: Vec<String> = storage
        .list_archived_sessions()
        .expect("archived")
        .into_iter()
        .map(|r| r.id)
        .collect();
    for id in ["root", "child", "grandchild"] {
        assert!(
            archived.contains(&id.to_string()),
            "{id} archived with root"
        );
    }
    assert!(!archived.contains(&"other".to_string()));

    storage.unarchive_session("root").expect("unarchive");
    assert!(
        storage
            .list_archived_sessions()
            .expect("archived")
            .is_empty()
    );
    assert_eq!(storage.list_child_sessions("child").expect("kids").len(), 1);
}

/// A v4 database (no `kind` column) migrates: the column appears with every
/// existing session read as interactive.
#[test]
fn test_session_grouping_migrates_a_v4_database() {
    let path = fixture_dir("migrate_v4_db").join("ragent.db");
    {
        let storage = Storage::open(&path).expect("open");
        storage
            .create_session("old", "/nonexistent/v4")
            .expect("session");
        let guard = storage.conn_lock_for_test().expect("lock");
        guard
            .execute_batch(
                "DROP INDEX IF EXISTS idx_sessions_parent;
                 ALTER TABLE sessions DROP COLUMN kind;
                 UPDATE settings SET value = '4' WHERE key = 'schema_version';",
            )
            .expect("rewind to a v4 schema");
    }

    let storage = Storage::open(&path).expect("migrate");

    assert_eq!(
        storage.session_kind("old").expect("kind"),
        Some(ragent_storage::SessionKind::Interactive)
    );
    assert_eq!(storage.list_sessions_grouped().expect("grouped").len(), 1);
}
