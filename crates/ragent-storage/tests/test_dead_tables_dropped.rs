//! T-14: the dead `snapshots` and `mcp_servers` tables are gone.
//!
//! Neither had a producer or a consumer anywhere in the tree: MCP server
//! configuration is read from config files and discovery, and snapshots are the
//! file-based ones in `ragent-storage::snapshot`. Keeping the DDL meant two
//! tables and their bookkeeping in every migration batch for no reader.

use ragent_storage::storage::Storage;
use ragent_types::message::{Message, MessagePart, Role};
use std::path::Path;

fn table_exists(storage: &Storage, name: &str) -> bool {
    let guard = storage.conn_lock_for_test().expect("lock");
    let count: i64 = guard
        .query_row(
            "SELECT COUNT(*) FROM sqlite_master WHERE type = 'table' AND name = ?1",
            [name],
            |r| r.get(0),
        )
        .expect("count table");
    count > 0
}

/// A fresh database must not contain either table.
#[test]
fn fresh_database_has_neither_dead_table() {
    let storage = Storage::open_in_memory().expect("storage");
    assert!(
        !table_exists(&storage, "snapshots"),
        "snapshots must be gone"
    );
    assert!(
        !table_exists(&storage, "mcp_servers"),
        "mcp_servers must be gone"
    );
    // The tables that DO carry data are untouched.
    assert!(table_exists(&storage, "sessions"));
    assert!(table_exists(&storage, "messages"));
    assert!(table_exists(&storage, "memories"));
}

/// Migrating a database that already has the old tables must drop them and
/// leave every populated table intact.
#[test]
fn migration_from_v2_drops_the_dead_tables_only() {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("target/temp/t14_drop");
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("mkdir");
    let path = dir.join("ragent.db");

    {
        let storage = Storage::open(&path).expect("open");
        storage.create_session("sess", "/w").expect("session");
        storage
            .create_message(&Message::new(
                "sess",
                Role::User,
                vec![MessagePart::Text {
                    text: "hello".to_string(),
                }],
            ))
            .expect("message");
        storage
            .create_memory("kept memory", "fact", "test", 0.8, "/w", "sess", &[])
            .expect("memory");

        // Recreate the old tables and a schema_version that predates T-14, the
        // way a database written by the previous release looks.
        let guard = storage.conn_lock_for_test().expect("lock");
        guard
            .execute_batch(
                "CREATE TABLE IF NOT EXISTS mcp_servers (
                     id TEXT PRIMARY KEY, config TEXT NOT NULL,
                     status TEXT NOT NULL DEFAULT 'disabled',
                     updated_at TEXT NOT NULL);
                 CREATE TABLE IF NOT EXISTS snapshots (
                     id TEXT PRIMARY KEY, session_id TEXT NOT NULL,
                     message_id TEXT NOT NULL, data TEXT NOT NULL,
                     created_at TEXT NOT NULL,
                     FOREIGN KEY (session_id) REFERENCES sessions(id));
                 INSERT INTO mcp_servers (id, config, updated_at)
                     VALUES ('legacy', '{}', '2024-01-01T00:00:00Z');
                 UPDATE settings SET value = '2' WHERE key = 'schema_version';",
            )
            .expect("recreate legacy schema");
    }

    let storage = Storage::open(&path).expect("migrate");
    assert!(!table_exists(&storage, "snapshots"));
    assert!(!table_exists(&storage, "mcp_servers"));

    assert_eq!(storage.list_sessions().expect("sessions").len(), 1);
    assert_eq!(storage.get_messages("sess").expect("messages").len(), 1);
    assert_eq!(storage.count_memories().expect("memories"), 1);
    let hits = storage
        .search_memories("kept", None, None, 10, 0.0)
        .expect("search");
    assert_eq!(hits.len(), 1, "the memory FTS index survived the migration");

    let _ = std::fs::remove_dir_all(&dir);
}
