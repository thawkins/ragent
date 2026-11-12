//! Session unread mark, delete and fork (spec SESSIONMENU, milestone M2).
//!
//! The migration fixture lives under the git-ignored `target/temp/` tree in a
//! directory unique to this file, so nothing is written into the repository.

use std::path::{Path, PathBuf};

use ragent_storage::storage::{SessionKind, SessionSearchParams, Storage};
use ragent_types::message::Message;

/// A fresh, empty fixture directory unique to `name`.
fn fixture_dir(name: &str) -> PathBuf {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../target/temp/session_delete_fork")
        .join(name);
    let _ = std::fs::remove_dir_all(&dir); // INTENTIONAL: may not exist yet
    std::fs::create_dir_all(&dir).expect("create fixture dir");
    dir
}

/// Session ids whose messages match `query` in the search index.
fn search_hits(storage: &Storage, query: &str) -> Vec<String> {
    let params = SessionSearchParams {
        query: query.to_string(),
        limit: 50,
        include_tools: true,
        include_system: true,
        ..Default::default()
    };
    storage
        .search_session_messages(&params)
        .expect("search")
        .into_iter()
        .map(|r| r.session_id)
        .collect()
}

#[test]
fn test_session_delete_fork_unread_round_trip_skips_archived() {
    let storage = Storage::open_in_memory().expect("storage");
    storage.create_session("a", "/nonexistent/p").expect("a");
    storage.create_session("b", "/nonexistent/p").expect("b");
    let before = storage
        .get_session("a")
        .expect("get")
        .expect("a")
        .updated_at;

    storage.set_session_unread("a", true).expect("mark a");
    storage.set_session_unread("b", true).expect("mark b");
    storage.archive_session("b").expect("archive b");

    let unread = storage.unread_session_ids().expect("unread");
    assert!(unread.contains("a"));
    assert!(!unread.contains("b"), "archived sessions are not listed");
    let after = storage
        .get_session("a")
        .expect("get")
        .expect("a")
        .updated_at;
    assert_eq!(before, after, "marking does not reorder the list");

    storage.set_session_unread("a", false).expect("clear a");
    assert!(storage.unread_session_ids().expect("unread").is_empty());
}

#[test]
fn test_session_delete_fork_delete_removes_tree_and_index_only() {
    let storage = Storage::open_in_memory().expect("storage");
    storage
        .create_session("lead", "/nonexistent/p")
        .expect("lead");
    storage
        .create_session_with("kid", "/nonexistent/p", Some("lead"), SessionKind::Subagent)
        .expect("kid");
    storage
        .create_session_with(
            "grandkid",
            "/nonexistent/p",
            Some("kid"),
            SessionKind::Subagent,
        )
        .expect("grandkid");
    storage
        .create_session("other", "/nonexistent/p")
        .expect("other");
    for sid in ["lead", "kid", "grandkid", "other"] {
        storage
            .create_message(&Message::user_text(sid, "zebracorn"))
            .expect("message");
    }

    let deleted = storage.delete_session("lead").expect("delete");

    assert_eq!(deleted, ["lead", "kid", "grandkid"]);
    for sid in ["lead", "kid", "grandkid"] {
        assert!(
            storage.get_session(sid).expect("get").is_none(),
            "{sid} row"
        );
        assert!(
            storage.get_messages(sid).expect("msgs").is_empty(),
            "{sid} msgs"
        );
    }
    assert!(storage.get_session("other").expect("get").is_some());
    assert_eq!(storage.get_messages("other").expect("msgs").len(), 1);
    assert_eq!(
        search_hits(&storage, "zebracorn"),
        ["other"],
        "index pruned"
    );
}

#[test]
fn test_session_delete_fork_delete_missing_is_empty() {
    let storage = Storage::open_in_memory().expect("storage");
    assert_eq!(
        storage.delete_session("nope").expect("delete"),
        Vec::<String>::new()
    );
}

#[test]
fn test_session_delete_fork_fork_copies_messages_with_new_ids() {
    let storage = Storage::open_in_memory().expect("storage");
    storage
        .create_session("src", "/nonexistent/p")
        .expect("src");
    let first = Message::user_text("src", "first quokka");
    let second = Message::assistant_text("src", "second reply");
    storage.create_message(&first).expect("first");
    std::thread::sleep(std::time::Duration::from_millis(2));
    storage.create_message(&second).expect("second");

    storage
        .fork_session("src", "copy", "src (fork)")
        .expect("fork");

    let copy = storage.get_session("copy").expect("get").expect("copy");
    assert_eq!(copy.title, "src (fork)");
    assert_eq!(copy.directory, "/nonexistent/p");
    assert!(copy.parent_id.is_none(), "a fork is a top-level session");
    assert_eq!(
        storage.session_kind("copy").expect("kind"),
        Some(SessionKind::Interactive)
    );
    let messages = storage.get_messages("copy").expect("messages");
    let texts: Vec<String> = messages.iter().map(Message::text_content).collect();
    assert_eq!(texts, ["first quokka", "second reply"]);
    assert!(
        messages
            .iter()
            .all(|m| m.id != first.id && m.id != second.id)
    );
    assert_eq!(messages[0].created_at, first.created_at, "timestamps kept");
    assert_eq!(
        storage.get_messages("src").expect("src").len(),
        2,
        "source kept"
    );
    let mut hits = search_hits(&storage, "quokka");
    hits.sort();
    assert_eq!(hits, ["copy", "src"], "the fork is searchable");
    let listed = storage.list_sessions_grouped().expect("grouped");
    assert!(listed[0].sessions.iter().any(|s| s.id == "copy"));

    // The two are independent: deleting the source keeps the fork.
    storage.delete_session("src").expect("delete src");
    assert_eq!(storage.get_messages("copy").expect("copy").len(), 2);
}

#[test]
fn test_session_delete_fork_fork_of_missing_creates_nothing() {
    let storage = Storage::open_in_memory().expect("storage");
    assert!(storage.fork_session("nope", "copy", "x").is_err());
    assert!(storage.get_session("copy").expect("get").is_none());
}

/// A schema 5 database (no `unread` column) migrates on open and keeps its
/// sessions and messages.
#[test]
fn test_session_delete_fork_migrates_a_v5_database() {
    let path = fixture_dir("migrate_v5").join("ragent.db");
    {
        let storage = Storage::open(&path).expect("open");
        storage
            .create_session("old", "/nonexistent/p")
            .expect("session");
        storage
            .create_message(&Message::user_text("old", "kept"))
            .expect("message");
        let guard = storage.conn_lock_for_test().expect("lock");
        guard
            .execute_batch(
                "ALTER TABLE sessions DROP COLUMN unread;
                 UPDATE settings SET value = '5' WHERE key = 'schema_version';",
            )
            .expect("rewind to a v5 schema");
    }

    let storage = Storage::open(&path).expect("migrate");

    assert!(storage.get_session("old").expect("get").is_some());
    assert_eq!(storage.get_messages("old").expect("msgs").len(), 1);
    assert!(storage.unread_session_ids().expect("unread").is_empty());
    storage
        .set_session_unread("old", true)
        .expect("the unread column exists after migration");
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
