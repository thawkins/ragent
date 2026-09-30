//! ANTIPAT M5.7 C-3: regression tests for the multi-statement writers that were
//! wrapped in `conn.transaction()` so a mid-write failure cannot desync the FTS
//! index from the base table.
//!
//! These tests pin the *observable* post-condition of each writer: after the
//! call, the base table and its FTS companion agree. They also cover the C-4
//! change that routes the pure SELECT in `search_memories` through the
//! read-only connection while the `access_count` bump still writes through the
//! writer connection.

use ragent_storage::storage::Storage;
use ragent_types::message::Message;

/// `create_memory` inserts the memory row, its tag rows, and the FTS row in one
/// transaction; all three must be visible afterwards.
#[test]
fn create_memory_commits_memory_tags_and_fts() {
    let storage = Storage::open_in_memory().expect("storage");
    let tags = vec!["rust".to_string(), "storage".to_string()];
    let id = storage
        .create_memory(
            "sqlite transactions keep FTS in sync",
            "insight",
            "test",
            0.9,
            "/proj",
            "sess-1",
            &tags,
        )
        .expect("create memory");

    let row = storage.get_memory(id).expect("get").expect("row");
    assert_eq!(row.content, "sqlite transactions keep FTS in sync");

    let mut stored_tags = storage.get_memory_tags(id).expect("tags");
    stored_tags.sort();
    assert_eq!(stored_tags, tags, "tag rows committed with the memory");

    let hits = storage
        .search_memories("transactions", None, None, 10, 0.0)
        .expect("search");
    assert_eq!(hits.len(), 1, "FTS row committed with the memory");
    assert_eq!(hits[0].id, id);
}

/// `delete_memory` removes the FTS row and the base row together; neither may
/// survive the call.
#[test]
fn delete_memory_removes_base_and_fts_rows() {
    let storage = Storage::open_in_memory().expect("storage");
    let id = storage
        .create_memory("deletable memory", "fact", "test", 0.8, "/proj", "s1", &[])
        .expect("create");

    // The FTS row is present before the delete.
    assert_eq!(
        storage
            .search_memories("deletable", None, None, 10, 0.0)
            .expect("search")
            .len(),
        1
    );

    let deleted = storage.delete_memory(id).expect("delete");
    assert!(deleted, "row was reported deleted");
    assert!(storage.get_memory(id).expect("get").is_none());
    assert!(
        storage
            .search_memories("deletable", None, None, 10, 0.0)
            .expect("search")
            .is_empty(),
        "FTS row removed with the base row"
    );
}

/// `delete_memories_by_filter` deletes a whole set of ids (base + FTS) in one
/// transaction.
#[test]
fn delete_memories_by_filter_removes_all_matching() {
    let storage = Storage::open_in_memory().expect("storage");
    for (content, conf) in [("keep alpha", 0.9), ("drop beta", 0.1), ("drop gamma", 0.2)] {
        storage
            .create_memory(content, "fact", "test", conf, "/proj", "s1", &[])
            .expect("create");
    }

    let deleted = storage
        .delete_memories_by_filter(None, Some(0.3), None, None)
        .expect("delete by filter");
    assert_eq!(deleted, 2, "two low-confidence memories removed");
    assert_eq!(storage.count_memories().expect("count"), 1);

    let remaining = storage
        .search_memories("alpha", None, None, 10, 0.0)
        .expect("search");
    assert_eq!(remaining.len(), 1, "the kept memory's FTS row survives");
    assert!(
        storage
            .search_memories("beta", None, None, 10, 0.0)
            .expect("search")
            .is_empty(),
        "removed memories' FTS rows are gone"
    );
}

/// `delete_messages` removes a session's messages row and FTS row together.
#[test]
fn delete_messages_removes_base_and_fts_rows() {
    let storage = Storage::open_in_memory().expect("storage");
    storage
        .create_session("sess-del", "/tmp/x")
        .expect("session");
    storage
        .create_message(&Message::user_text("sess-del", "database migration plan"))
        .expect("message");

    // Search finds it before deletion.
    assert!(
        !storage
            .search_conversation("sess-del", "migration", 10)
            .expect("search")
            .is_empty()
    );

    let deleted = storage.delete_messages("sess-del").expect("delete");
    assert_eq!(deleted, 1);
    assert!(storage.get_messages("sess-del").expect("get").is_empty());
    assert!(
        storage
            .search_conversation("sess-del", "migration", 10)
            .expect("search")
            .is_empty(),
        "FTS row removed with the message row"
    );
}

/// C-4: `search_memories` reads through the read-only connection and still
/// bumps `access_count` for the rows it returned (via the writer connection).
#[test]
fn search_memories_still_bumps_access_count() {
    let storage = Storage::open_in_memory().expect("storage");
    let id = storage
        .create_memory("access counter probe", "fact", "test", 0.7, "/p", "s", &[])
        .expect("create");

    let hits = storage
        .search_memories("counter", None, None, 10, 0.0)
        .expect("search");
    assert_eq!(hits.len(), 1);

    let row = storage.get_memory(id).expect("get").expect("row");
    assert_eq!(row.access_count, 1, "search bumped the access counter");
    assert!(row.last_accessed.is_some(), "last_accessed stamped");
}
