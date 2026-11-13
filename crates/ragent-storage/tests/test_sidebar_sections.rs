//! User-defined sidebar sections (spec SESSIONMENU, milestone M3).
//!
//! The migration fixture lives under the git-ignored `target/temp/` tree in a
//! directory unique to this file, so nothing is written into the repository.

use std::path::{Path, PathBuf};

use ragent_storage::storage::Storage;

/// A fresh, empty fixture directory unique to `name`.
fn fixture_dir(name: &str) -> PathBuf {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../target/temp/sidebar_sections")
        .join(name);
    let _ = std::fs::remove_dir_all(&dir); // INTENTIONAL: may not exist yet
    std::fs::create_dir_all(&dir).expect("create fixture dir");
    dir
}

fn names(storage: &Storage) -> Vec<String> {
    storage
        .list_sections()
        .expect("list")
        .into_iter()
        .map(|s| s.name)
        .collect()
}

#[test]
fn test_sidebar_sections_create_lists_by_name_and_rejects_duplicates() {
    let storage = Storage::open_in_memory().expect("storage");
    storage.create_section("reviews").expect("reviews");
    storage.create_section("  Archive-ish ").expect("trimmed");

    assert_eq!(names(&storage), ["Archive-ish", "reviews"]);
    assert!(
        storage.create_section("REVIEWS").is_err(),
        "case-insensitive"
    );
    assert!(storage.create_section("   ").is_err(), "empty name");
}

#[test]
fn test_sidebar_sections_rename_checks_conflicts() {
    let storage = Storage::open_in_memory().expect("storage");
    let a = storage.create_section("A").expect("a");
    storage.create_section("B").expect("b");

    assert!(storage.rename_section(&a.id, "b").is_err(), "name taken");
    storage
        .rename_section(&a.id, "a")
        .expect("same name, new case");
    storage.rename_section(&a.id, "C").expect("rename");
    assert_eq!(names(&storage), ["B", "C"]);
    assert!(
        storage.rename_section("nope", "D").is_err(),
        "missing section"
    );
}

#[test]
fn test_sidebar_sections_move_and_delete_returns_sessions() {
    let storage = Storage::open_in_memory().expect("storage");
    storage.create_session("s1", "/nonexistent/p").expect("s1");
    storage.create_session("s2", "/nonexistent/p").expect("s2");
    let section = storage.create_section("Work").expect("section");
    let before = storage
        .get_session("s1")
        .expect("get")
        .expect("s1")
        .updated_at;

    storage
        .set_session_section("s1", Some(&section.id))
        .expect("move s1");
    storage
        .set_session_section("s2", Some(&section.id))
        .expect("move s2");
    storage.archive_session("s2").expect("archive s2");

    let placed = storage.session_sections().expect("placed");
    assert_eq!(placed.len(), 1, "archived sessions are left out");
    assert_eq!(placed["s1"], section.id);
    let after = storage
        .get_session("s1")
        .expect("get")
        .expect("s1")
        .updated_at;
    assert_eq!(before, after, "moving does not reorder the list");
    assert!(
        storage.set_session_section("s1", Some("nope")).is_err(),
        "unknown section"
    );

    storage.delete_section(&section.id).expect("delete");
    assert_eq!(storage.list_sections().expect("list"), Vec::new());
    assert!(storage.session_sections().expect("placed").is_empty());
    storage.unarchive_session("s2").expect("unarchive");
    assert!(
        storage.session_sections().expect("placed").is_empty(),
        "an archived member is released too"
    );
}

#[test]
fn test_sidebar_sections_move_back_to_project() {
    let storage = Storage::open_in_memory().expect("storage");
    storage.create_session("s", "/nonexistent/p").expect("s");
    let section = storage.create_section("Work").expect("section");
    storage
        .set_session_section("s", Some(&section.id))
        .expect("move");

    storage.set_session_section("s", None).expect("back");

    assert!(storage.session_sections().expect("placed").is_empty());
}

/// A schema 6 database (no sections) migrates on open and keeps its data.
#[test]
fn test_sidebar_sections_migrates_a_v6_database() {
    let path = fixture_dir("migrate_v6").join("ragent.db");
    {
        let storage = Storage::open(&path).expect("open");
        storage
            .create_session("old", "/nonexistent/p")
            .expect("session");
        storage.set_session_unread("old", true).expect("unread");
        let guard = storage.conn_lock_for_test().expect("lock");
        guard
            .execute_batch(
                "ALTER TABLE sessions DROP COLUMN section_id;
                 DROP TABLE sidebar_sections;
                 UPDATE settings SET value = '6' WHERE key = 'schema_version';",
            )
            .expect("rewind to a v6 schema");
    }

    let storage = Storage::open(&path).expect("migrate");

    assert!(
        storage
            .unread_session_ids()
            .expect("unread")
            .contains("old")
    );
    let section = storage.create_section("Work").expect("table exists");
    storage
        .set_session_section("old", Some(&section.id))
        .expect("column exists");
    let guard = storage.conn_lock_for_test().expect("lock");
    let version: String = guard
        .query_row(
            "SELECT value FROM settings WHERE key = 'schema_version'",
            [],
            |r| r.get(0),
        )
        .expect("version");
    assert_eq!(version, ragent_storage::SCHEMA_VERSION.to_string());
}
