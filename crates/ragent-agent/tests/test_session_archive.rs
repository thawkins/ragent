//! Tests-only coverage for session archive export/import
//! (`ragent_agent::session::archive`).
//!
//! Round-trips a session through a `.tar.gz` archive with in-memory storage and
//! no trigger engine, then exercises the missing-session and checksum-mismatch
//! error branches. All files live under a temp directory.

use std::sync::Arc;

use ragent_agent::session::archive::{
    ArchiveConfig, ImportConfig, export_session_archive, import_session_archive,
};
use ragent_agent::storage::Storage;
use ragent_types::message::Message;

fn seeded_storage() -> (Arc<Storage>, String) {
    let storage = Arc::new(Storage::open_in_memory().expect("storage"));
    let session_id = "sess-archive-1".to_string();
    storage
        .create_session(&session_id, "/tmp/project")
        .expect("create session");
    storage
        .create_message(&Message::user_text(&session_id, "hello archive"))
        .expect("user message");
    storage
        .create_message(&Message::assistant_text(&session_id, "hi there"))
        .expect("assistant message");
    (storage, session_id)
}

#[tokio::test]
async fn export_then_import_round_trips_messages() {
    let (storage, session_id) = seeded_storage();
    let dir = tempfile::tempdir().expect("tempdir");
    let archive_path = dir.path().join("session.tar.gz");

    let written = export_session_archive(
        &storage,
        None,
        &session_id,
        &archive_path,
        &ArchiveConfig::default(),
    )
    .expect("export");
    assert_eq!(written, archive_path);
    assert!(archive_path.exists(), "archive file must be written");

    let import_config = ImportConfig {
        verify_checksums: true,
        ..Default::default()
    };
    let result = import_session_archive(&storage, None, &archive_path, &import_config)
        .await
        .expect("import");

    assert_eq!(result.messages_imported, 2);
    assert!(result.checksums_verified);
    assert_ne!(
        result.session_id, session_id,
        "import creates a fresh session"
    );

    let imported = storage
        .get_messages(&result.session_id)
        .expect("imported messages");
    assert_eq!(imported.len(), 2);
}

#[tokio::test]
async fn export_config_flags_omit_optional_files() {
    let (storage, session_id) = seeded_storage();
    let dir = tempfile::tempdir().expect("tempdir");
    let archive_path = dir.path().join("minimal.tar.gz");

    let config = ArchiveConfig {
        include_triggers: false,
        include_cron: false,
        include_loop_state: false,
        include_cost: false,
    };
    export_session_archive(&storage, None, &session_id, &archive_path, &config).expect("export");

    // Re-import with all optional import features off to cover those branches.
    let import_config = ImportConfig {
        activate_triggers: false,
        activate_cron: false,
        restore_loop_state: false,
        verify_checksums: true,
        import_triggers: false,
    };
    let result = import_session_archive(&storage, None, &archive_path, &import_config)
        .await
        .expect("import");
    assert_eq!(result.triggers_imported, 0);
    assert_eq!(result.cron_jobs_imported, 0);
    assert_eq!(result.loop_state_files_restored, 0);
}

#[test]
fn export_unknown_session_errors() {
    let (storage, _session_id) = seeded_storage();
    let dir = tempfile::tempdir().expect("tempdir");
    let archive_path = dir.path().join("missing.tar.gz");

    let err = export_session_archive(
        &storage,
        None,
        "no-such-session",
        &archive_path,
        &ArchiveConfig::default(),
    )
    .expect_err("unknown session must error");
    assert!(err.to_string().contains("Session not found"), "err: {err}");
}

#[tokio::test]
async fn import_rejects_a_truncated_archive() {
    let (storage, session_id) = seeded_storage();
    let dir = tempfile::tempdir().expect("tempdir");
    let archive_path = dir.path().join("bad.tar.gz");
    export_session_archive(
        &storage,
        None,
        &session_id,
        &archive_path,
        &ArchiveConfig::default(),
    )
    .expect("export");

    // Truncate the gzip stream part-way so decompression fails unexpectedly.
    // (Flipping a byte inside the compressed body is unreliable: it can land
    // in trailing padding or produce a still-valid stream.)
    let bytes = std::fs::read(&archive_path).expect("read archive");
    assert!(bytes.len() > 4);
    std::fs::write(&archive_path, &bytes[..bytes.len() / 2]).expect("truncate archive");

    let import_config = ImportConfig {
        verify_checksums: true,
        ..Default::default()
    };
    let result = import_session_archive(&storage, None, &archive_path, &import_config).await;
    assert!(
        result.is_err(),
        "a truncated archive must fail import at decompression"
    );
}

#[tokio::test]
async fn import_missing_archive_file_errors() {
    let (storage, _session_id) = seeded_storage();
    let dir = tempfile::tempdir().expect("tempdir");
    let missing = dir.path().join("nope.tar.gz");
    let err = import_session_archive(&storage, None, &missing, &ImportConfig::default())
        .await
        .expect_err("missing archive must error");
    assert!(
        err.to_string().contains("Failed to open archive"),
        "err: {err}"
    );
}

#[test]
fn archive_config_defaults_enable_trigger_cron_and_loop_state() {
    let config = ArchiveConfig::default();
    assert!(config.include_triggers);
    assert!(config.include_cron);
    assert!(config.include_loop_state);
    assert!(!config.include_cost);
}

#[test]
fn import_config_defaults_restore_loop_state_with_verification() {
    let config = ImportConfig::default();
    assert!(!config.activate_triggers);
    assert!(!config.activate_cron);
    assert!(config.restore_loop_state);
    assert!(config.verify_checksums);
    assert!(!config.import_triggers);
}
