//! Isolation coverage for the settings key-value store, the discovered-model
//! cache, and the file-backed `Storage::open` lifecycle (`open`,
//! `checkpoint_wal`, `journal_mode`, the read-only connection path, session
//! title/archive updates).

use ragent_storage::storage::Storage;

fn temp_db(name: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join("ragent-storage-settings-test");
    std::fs::create_dir_all(&dir).expect("temp dir");
    let path = dir.join(name);
    let _ = std::fs::remove_file(&path);
    let _ = std::fs::remove_file(path.with_extension("db-wal"));
    let _ = std::fs::remove_file(path.with_extension("db-shm"));
    path
}

#[test]
fn settings_set_get_delete() {
    let storage = Storage::open_in_memory().expect("storage");
    assert!(storage.get_setting("theme").expect("get").is_none());

    storage.set_setting("theme", "dark").expect("set");
    assert_eq!(
        storage.get_setting("theme").expect("get"),
        Some("dark".to_string())
    );

    // Overwrite replaces.
    storage.set_setting("theme", "light").expect("overwrite");
    assert_eq!(
        storage.get_setting("theme").expect("get"),
        Some("light".to_string())
    );

    storage.delete_setting("theme").expect("delete");
    assert!(storage.get_setting("theme").expect("get").is_none());
    // Deleting an absent key is a no-op.
    storage.delete_setting("theme").expect("delete again");
}

#[test]
fn discovered_models_cache_round_trip() {
    let storage = Storage::open_in_memory().expect("storage");
    assert!(
        storage
            .get_discovered_models("openai")
            .expect("get")
            .is_none()
    );

    let json = r#"[{"id":"gpt-4o"},{"id":"gpt-4o-mini"}]"#;
    storage.set_discovered_models("openai", json).expect("set");
    assert_eq!(
        storage.get_discovered_models("openai").expect("get"),
        Some(json.to_string())
    );

    // Overwrite replaces.
    storage
        .set_discovered_models("openai", "[]")
        .expect("overwrite");
    assert_eq!(
        storage.get_discovered_models("openai").expect("get"),
        Some("[]".to_string())
    );

    storage.delete_discovered_models("openai").expect("delete");
    assert!(
        storage
            .get_discovered_models("openai")
            .expect("get")
            .is_none()
    );
}

#[test]
fn session_title_and_archive_updates() {
    let storage = Storage::open_in_memory().expect("storage");
    storage.create_session("s1", "/tmp/proj").expect("create");

    storage.update_session("s1", "Renamed").expect("title");
    let session = storage.get_session("s1").expect("get").expect("row");
    assert_eq!(session.title, "Renamed");

    storage.archive_session("s1").expect("archive");
    let session = storage.get_session("s1").expect("get").expect("row");
    assert!(session.archived_at.is_some(), "archived_at stamped");
    assert!(
        storage.list_sessions().expect("list").is_empty(),
        "archived sessions excluded from list_sessions"
    );
}

#[test]
fn file_backed_open_sets_wal_and_checkpoints() {
    let path = temp_db("file-backed.db");
    let storage = Storage::open(&path).expect("open");

    // WAL is enabled so a background writer never serialises readers.
    assert_eq!(
        storage.journal_mode().expect("journal mode").to_lowercase(),
        "wal"
    );

    // The read-only connection observes writes made through the writer.
    storage.create_session("s1", "/tmp/proj").expect("create");
    assert!(storage.get_session("s1").expect("read").is_some());
    assert_eq!(storage.list_sessions().expect("list").len(), 1);

    // A checkpoint truncates the WAL without losing data.
    storage.checkpoint_wal().expect("checkpoint");
    assert!(
        storage
            .get_session("s1")
            .expect("read after ckpt")
            .is_some()
    );

    // On unix the database file and its sidecars are 0600.
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let meta = std::fs::metadata(&path).expect("metadata");
        assert_eq!(
            meta.permissions().mode() & 0o777,
            0o600,
            "database file restricted to owner-only"
        );
    }

    drop(storage);
    let _ = std::fs::remove_file(&path);
    let _ = std::fs::remove_file(path.with_extension("db-wal"));
    let _ = std::fs::remove_file(path.with_extension("db-shm"));
}

#[test]
fn file_backed_open_is_idempotent_across_reopen() {
    let path = temp_db("reopen.db");
    {
        let storage = Storage::open(&path).expect("first open");
        storage.create_session("s1", "/tmp/proj").expect("create");
        storage.set_setting("k", "v").expect("set");
    }
    // The schema-version fast path skips the migration batch on the warm open;
    // the data must still be there.
    let storage = Storage::open(&path).expect("second open");
    assert!(storage.get_session("s1").expect("read").is_some());
    assert_eq!(
        storage.get_setting("k").expect("get"),
        Some("v".to_string())
    );
    drop(storage);
    let _ = std::fs::remove_file(&path);
    let _ = std::fs::remove_file(path.with_extension("db-wal"));
    let _ = std::fs::remove_file(path.with_extension("db-shm"));
}
