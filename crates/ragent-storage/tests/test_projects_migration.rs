//! Schema 5 -> 6 migration: project records are created from the work-tree
//! roots that `sessions.project_id` used to hold (spec `projects`, T-001).
//!
//! Fixtures live under the git-ignored `target/temp/` tree, one directory per
//! test, so nothing is written into the repository.

use std::path::{Path, PathBuf};

use ragent_storage::storage::Storage;

fn fixture_dir(name: &str) -> PathBuf {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../target/temp/projects_migration")
        .join(name);
    let _ = std::fs::remove_dir_all(&dir); // INTENTIONAL: may not exist yet
    std::fs::create_dir_all(&dir).expect("create fixture dir");
    dir.canonicalize().expect("canonical fixture dir")
}

/// Rewinds a current database to the schema-5 shape: no `projects` table, no
/// checkout columns, and `project_id` holding the work-tree root path.
fn rewind_to_v5(storage: &Storage) {
    let guard = storage.conn_lock_for_test().expect("lock");
    guard
        .execute_batch(
            "UPDATE sessions SET project_id =
                 (SELECT worktree FROM projects WHERE projects.id = sessions.project_id);
             DROP TABLE projects;
             ALTER TABLE sessions DROP COLUMN branch;
             ALTER TABLE sessions DROP COLUMN worktree;
             ALTER TABLE sessions DROP COLUMN pr_count;
             UPDATE settings SET value = '5' WHERE key = 'schema_version';",
        )
        .expect("rewind to a v5 schema");
}

#[test]
fn test_projects_migration_creates_one_project_per_root() {
    let path = fixture_dir("one_per_root").join("ragent.db");
    {
        let storage = Storage::open(&path).expect("open");
        storage
            .create_session("a1", "/nonexistent/alpha")
            .expect("a1");
        storage
            .create_session("a2", "/nonexistent/alpha")
            .expect("a2");
        storage
            .create_session("b1", "/nonexistent/beta")
            .expect("b1");
        storage
            .create_session("gone", "/nonexistent/gamma")
            .expect("gone");
        storage.archive_session("gone").expect("archive");
        rewind_to_v5(&storage);
        let guard = storage.conn_lock_for_test().expect("lock");
        let root: String = guard
            .query_row("SELECT project_id FROM sessions WHERE id = 'a1'", [], |r| {
                r.get(0)
            })
            .expect("project_id");
        assert_eq!(root, "/nonexistent/alpha", "the fixture is a v5 database");
    }

    let storage = Storage::open(&path).expect("migrate");

    let project_of = |id: &str| {
        let row = storage.get_session(id).expect("get").expect("row");
        storage
            .get_project(&row.project_id)
            .expect("get project")
            .unwrap_or_else(|| panic!("session {id} has no project record"))
    };
    let alpha = project_of("a1");
    assert_eq!(alpha.worktree, "/nonexistent/alpha");
    assert_eq!(alpha.name, "alpha");
    assert!(alpha.remote.is_none());
    assert!(
        !alpha.remote_checked,
        "remote detection is left for first use"
    );
    assert_eq!(project_of("a2").id, alpha.id, "one project per root");
    assert_ne!(project_of("b1").id, alpha.id);

    let guard = storage.conn_lock_for_test().expect("lock");
    let archived_project: String = guard
        .query_row(
            "SELECT p.worktree FROM sessions s JOIN projects p ON p.id = s.project_id \
             WHERE s.id = 'gone'",
            [],
            |r| r.get(0),
        )
        .expect("archived session is attached too");
    assert_eq!(archived_project, "/nonexistent/gamma");
    let projects: i64 = guard
        .query_row("SELECT COUNT(*) FROM projects", [], |r| r.get(0))
        .expect("count");
    assert_eq!(projects, 3);
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
fn test_projects_migration_rekeys_collapsed_sidebar_sections() {
    let path = fixture_dir("rekey").join("ragent.db");
    {
        let storage = Storage::open(&path).expect("open");
        storage
            .create_session("a1", "/nonexistent/alpha")
            .expect("a1");
        storage
            .set_setting(
                "tui.sessions.sidebar",
                r#"{"visible":true,"width":40,"collapsed":["\u0000pinned","/nonexistent/alpha"]}"#,
            )
            .expect("setting");
        rewind_to_v5(&storage);
    }

    let storage = Storage::open(&path).expect("migrate");

    let alpha = storage
        .get_project_by_worktree("/nonexistent/alpha")
        .expect("get")
        .expect("project");
    let saved = storage
        .get_setting("tui.sessions.sidebar")
        .expect("get setting")
        .expect("setting kept");
    let layout: serde_json::Value = serde_json::from_str(&saved).expect("json");
    assert_eq!(layout["width"], 40, "other fields are untouched");
    assert_eq!(
        layout["collapsed"],
        serde_json::json!(["\u{0}pinned", alpha.id]),
        "the project section is keyed by id; the pinned key is kept"
    );
}

#[test]
fn test_projects_migration_second_open_changes_nothing() {
    let path = fixture_dir("idempotent").join("ragent.db");
    {
        let storage = Storage::open(&path).expect("open");
        storage
            .create_session("a1", "/nonexistent/alpha")
            .expect("a1");
        rewind_to_v5(&storage);
    }
    let first = {
        let storage = Storage::open(&path).expect("migrate");
        storage
            .get_session("a1")
            .expect("get")
            .expect("row")
            .project_id
    };
    // Force the slow path again: the migration must not mint a second
    // project or move the session.
    {
        let storage = Storage::open(&path).expect("reopen");
        let guard = storage.conn_lock_for_test().expect("lock");
        guard
            .execute_batch("UPDATE settings SET value = '5' WHERE key = 'schema_version';")
            .expect("force slow path");
    }
    let storage = Storage::open(&path).expect("migrate again");
    assert_eq!(
        storage
            .get_session("a1")
            .expect("get")
            .expect("row")
            .project_id,
        first
    );
    assert_eq!(storage.list_projects().expect("list").len(), 1);
}
