//! Additional tests-only coverage for the session archive export/import
//! (`ragent_agent::session::archive`).
//!
//! Covers the optional-sidecar branches the sibling `test_session_archive.rs`
//! leaves out: trigger-rule export/import, cron-job export/import, loop-state
//! copy round-trips, checksum-mismatch rejection, and the SEC-ragent-agent-002
//! archive-entry escape/bomb guards. Crafted archives are built by hand with a
//! minimal raw tar writer so no `tar`/`flate2` dev-dependency is required, and
//! the global-state directory is redirected to a temp dir so the loop-state
//! paths never touch the real `~/.config/ragent`.

#![allow(unsafe_code)]

use std::ffi::OsString;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use ragent_agent::session::archive::{
    ArchiveConfig, ImportConfig, export_session_archive, import_session_archive,
};
use ragent_agent::storage::Storage;
use ragent_agent::trigger::{
    DynamicTriggerEngine, NoopActionDispatcher, SimpleConditionEvaluator, TriggerRuntime,
};
use ragent_config::TriggerConfig;
use ragent_types::cron::{CronEvent, CronSchedule};
use ragent_types::message::Message;
use serde_json::json;

// ---------------------------------------------------------------------------
// Global-state isolation: redirect XDG_CONFIG_HOME so loop-state I/O lands in
// a temp dir. Process-global mutation is serialised with `serial_test::serial`
// and the guard restores the previous value on drop.
// ---------------------------------------------------------------------------

struct EnvGuard {
    key: &'static str,
    prev: Option<OsString>,
}

impl EnvGuard {
    fn set(key: &'static str, value: impl AsRef<std::ffi::OsStr>) -> Self {
        let prev = std::env::var_os(key);
        // SAFETY: process-global mutation, serialised by `serial_test::serial`.
        unsafe { std::env::set_var(key, value) };
        Self { key, prev }
    }
}

impl Drop for EnvGuard {
    fn drop(&mut self) {
        // SAFETY: restoring the value saved in `set`.
        unsafe {
            match &self.prev {
                Some(v) => std::env::set_var(self.key, v),
                None => std::env::remove_var(self.key),
            }
        }
    }
}

fn seeded_storage() -> (Arc<Storage>, String) {
    let storage = Arc::new(Storage::open_in_memory().expect("storage"));
    let session_id = "sess-archive-more".to_string();
    storage
        .create_session(&session_id, "/tmp/project")
        .expect("create session");
    storage
        .create_message(&Message::user_text(&session_id, "hello"))
        .expect("user message");
    storage
        .create_message(&Message::assistant_text(&session_id, "hi"))
        .expect("assistant message");
    (storage, session_id)
}

fn engine() -> DynamicTriggerEngine {
    DynamicTriggerEngine::new(
        TriggerRuntime::default(),
        TriggerConfig::default(),
        Arc::new(SimpleConditionEvaluator::new()),
        Arc::new(NoopActionDispatcher::new()),
    )
}

// ---------------------------------------------------------------------------
// Minimal raw tar + gzip writer (so crafted/escaping archives can be built).
// ---------------------------------------------------------------------------

fn raw_entry(name: &str, data: &[u8]) -> Vec<u8> {
    let mut header = [0u8; 512];
    let nb = name.as_bytes();
    header[..nb.len()].copy_from_slice(nb);
    header[100..108].copy_from_slice(b"0000644\0");
    header[108..116].copy_from_slice(b"0000000\0");
    header[116..124].copy_from_slice(b"0000000\0");
    let size = format!("{:011o}\0", data.len());
    header[124..136].copy_from_slice(size.as_bytes());
    let mtime = format!("{:011o}\0", 0u64);
    header[136..148].copy_from_slice(mtime.as_bytes());
    header[148..156].copy_from_slice(b"        ");
    header[156] = b'0';
    let sum: u32 = header.iter().map(|&b| u32::from(b)).sum();
    let checksum = format!("{sum:06o}\0 ");
    header[148..156].copy_from_slice(checksum.as_bytes());

    let mut out = header.to_vec();
    out.extend_from_slice(data);
    let pad = (512 - (data.len() % 512)) % 512;
    out.extend(std::iter::repeat_n(0u8, pad));
    out
}

fn gzip(raw: &[u8]) -> Vec<u8> {
    let mut buf = Vec::new();
    {
        let mut encoder = flate2::write::GzEncoder::new(&mut buf, flate2::Compression::default());
        encoder.write_all(raw).expect("gzip write");
        encoder.finish().expect("gzip finish");
    }
    buf
}

/// Write a `.tar.gz` archive built from `(name, bytes)` pairs (paths taken
/// verbatim, so an escaping `../` name survives into the archive).
fn write_crafted_archive(path: &Path, entries: &[(&str, &[u8])]) {
    let mut raw = Vec::new();
    for (name, data) in entries {
        raw.extend(raw_entry(name, data));
    }
    raw.extend(std::iter::repeat_n(0u8, 1024));
    std::fs::write(path, gzip(&raw)).expect("write archive");
}

fn manifest_json(session_directory: &str, files: serde_json::Value) -> Vec<u8> {
    serde_json::to_vec(&json!({
        "manifest_version": 1,
        "session_id": "crafted",
        "session_title": "Crafted",
        "session_directory": session_directory,
        "created_at": "2024-01-01T00:00:00Z",
        "message_count": 0,
        "trigger_count": 0,
        "cron_job_count": 0,
        "loop_state_count": 0,
        "files": files,
        "sensitivity_warning": "warning",
    }))
    .expect("manifest json")
}

// ---------------------------------------------------------------------------
// Round-trip with every optional sidecar enabled.
// ---------------------------------------------------------------------------

#[tokio::test]
#[serial_test::serial]
async fn export_and_import_round_trip_all_sidecars() {
    let config_root = tempfile::tempdir().expect("config root");
    let _env = EnvGuard::set("XDG_CONFIG_HOME", config_root.path());

    // Seed a stateful cron job and a matching loop-state file under the
    // redirected global state dir so the export copies it into the archive.
    let loop_state_dir = config_root.path().join("ragent").join("loop-state");
    std::fs::create_dir_all(&loop_state_dir).expect("loop-state dir");

    let storage = Arc::new(Storage::open_in_memory().expect("storage"));
    let session_id = "sess-sidecars".to_string();
    storage
        .create_session(&session_id, "/tmp/project")
        .expect("create session");
    storage
        .create_message(&Message::user_text(&session_id, "go"))
        .expect("user message");

    let cron = CronEvent::new(
        "cron-1".to_string(),
        "general".to_string(),
        "do the thing".to_string(),
        CronSchedule::repeat_now(300),
        "every 5m".to_string(),
        chrono::Utc::now(),
    );
    let mut cron = cron;
    cron.stateful = true;
    storage.insert_cron_event(&cron).expect("insert cron event");
    std::fs::write(loop_state_dir.join("cron-1.txt"), "loop state body").expect("loop state file");

    let source_engine = engine();
    source_engine
        .create_rule_from_parts("a file exists", "run the tests", false, false)
        .expect("create rule");

    let dir = tempfile::tempdir().expect("archive dir");
    let archive_path = dir.path().join("sidecars.tar.gz");
    let export_config = ArchiveConfig {
        include_triggers: true,
        include_cron: true,
        include_loop_state: true,
        include_cost: false,
    };
    export_session_archive(
        &storage,
        Some(&source_engine),
        &session_id,
        &archive_path,
        &export_config,
    )
    .expect("export with sidecars");
    assert!(archive_path.exists());

    // Import everything back into a fresh store.
    let target = Arc::new(Storage::open_in_memory().expect("target storage"));
    let target_engine = engine();
    let import_config = ImportConfig {
        activate_triggers: true,
        activate_cron: true,
        restore_loop_state: true,
        verify_checksums: true,
        import_triggers: true,
    };
    let result =
        import_session_archive(&target, Some(&target_engine), &archive_path, &import_config)
            .await
            .expect("import with sidecars");

    assert_eq!(result.messages_imported, 1);
    assert_eq!(result.triggers_imported, 1, "the trigger rule is imported");
    assert_eq!(result.cron_jobs_imported, 1, "the cron job is imported");
    assert_eq!(
        result.loop_state_files_restored, 1,
        "the loop state is restored"
    );
    assert!(result.checksums_verified);
    assert_eq!(target.list_cron_events().expect("list cron").len(), 1);
    assert_eq!(target_engine.list_rules().len(), 1);
    assert_eq!(
        std::fs::read_to_string(loop_state_dir.join("cron-1.txt")).expect("restored"),
        "loop state body"
    );
}

#[tokio::test]
#[serial_test::serial]
async fn export_triggers_without_an_engine_yields_no_rules() {
    // `include_triggers` with `None` for the engine maps to an empty list.
    let (storage, session_id) = seeded_storage();
    let dir = tempfile::tempdir().expect("archive dir");
    let archive_path = dir.path().join("no-engine.tar.gz");
    export_session_archive(
        &storage,
        None,
        &session_id,
        &archive_path,
        &ArchiveConfig::default(),
    )
    .expect("export");

    let target = Arc::new(Storage::open_in_memory().expect("target storage"));
    let import_config = ImportConfig {
        import_triggers: true,
        verify_checksums: true,
        ..Default::default()
    };
    let result = import_session_archive(&target, None, &archive_path, &import_config)
        .await
        .expect("import");
    assert_eq!(result.triggers_imported, 0);
}

// ---------------------------------------------------------------------------
// Crafted archives: checksum, escape and manifest branches.
// ---------------------------------------------------------------------------

#[tokio::test]
async fn import_rejects_a_checksum_mismatch() {
    let dir = tempfile::tempdir().expect("dir");
    let archive_path = dir.path().join("bad-hash.tar.gz");
    let transcript = b"[]";
    write_crafted_archive(
        &archive_path,
        &[
            (
                "manifest.json",
                &manifest_json(
                    "/tmp",
                    json!({ "transcript.json": "0000000000000000000000000000000000000000000000000000000000000000" }),
                ),
            ),
            ("transcript.json", &transcript[..]),
        ],
    );

    let storage = Arc::new(Storage::open_in_memory().expect("storage"));
    let err = import_session_archive(&storage, None, &archive_path, &ImportConfig::default())
        .await
        .expect_err("a checksum mismatch must be rejected");
    assert!(
        err.to_string().contains("Checksum verification failed for"),
        "err: {err}"
    );
}

#[tokio::test]
async fn import_rejects_a_missing_manifest_file() {
    let dir = tempfile::tempdir().expect("dir");
    let archive_path = dir.path().join("missing-entry.tar.gz");
    write_crafted_archive(
        &archive_path,
        &[
            (
                "manifest.json",
                &manifest_json(
                    "/tmp",
                    json!({ "absent.json": "0000000000000000000000000000000000000000000000000000000000000000" }),
                ),
            ),
            ("transcript.json", b"[]"),
        ],
    );

    let storage = Arc::new(Storage::open_in_memory().expect("storage"));
    let err = import_session_archive(&storage, None, &archive_path, &ImportConfig::default())
        .await
        .expect_err("a missing manifest file must be rejected");
    assert!(err.to_string().contains("missing file"), "err: {err}");
}

#[tokio::test]
async fn import_rejects_an_archive_entry_that_escapes_the_extraction_dir() {
    let dir = tempfile::tempdir().expect("dir");
    let archive_path = dir.path().join("escape.tar.gz");
    // An entry path with a parent component must be refused by the guarded
    // unpack before it can write outside the temp dir (SEC-ragent-agent-002).
    write_crafted_archive(
        &archive_path,
        &[
            ("../escape.txt", b"owned"),
            ("manifest.json", &manifest_json("/tmp", json!({}))),
        ],
    );

    let storage = Arc::new(Storage::open_in_memory().expect("storage"));
    let err = import_session_archive(&storage, None, &archive_path, &ImportConfig::default())
        .await
        .expect_err("an escaping entry must be rejected");
    assert!(
        err.to_string().contains("escapes the extraction directory"),
        "err: {err}"
    );
}

#[tokio::test]
async fn import_falls_back_to_cwd_when_manifest_directory_escapes() {
    // A manifest `session_directory` outside the working dir is ignored: the
    // import proceeds in the cwd and records the archived value as metadata.
    let dir = tempfile::tempdir().expect("dir");
    let archive_path = dir.path().join("escape-dir.tar.gz");
    write_crafted_archive(
        &archive_path,
        &[
            (
                "manifest.json",
                &manifest_json("/etc/should-not-be-used", json!({})),
            ),
            ("transcript.json", b"[]"),
        ],
    );

    let storage = Arc::new(Storage::open_in_memory().expect("storage"));
    let result = import_session_archive(&storage, None, &archive_path, &ImportConfig::default())
        .await
        .expect("import falls back to the cwd");
    assert_eq!(result.messages_imported, 0);
    let session = storage
        .get_session(&result.session_id)
        .expect("get session")
        .expect("session present");
    let cwd = std::env::current_dir().expect("cwd");
    assert_eq!(PathBuf::from(&session.directory), cwd);
}

#[tokio::test]
async fn import_without_checksum_verification_skips_hash_check() {
    let dir = tempfile::tempdir().expect("dir");
    let archive_path = dir.path().join("skip-hash.tar.gz");
    write_crafted_archive(
        &archive_path,
        &[
            (
                "manifest.json",
                &manifest_json("/tmp", json!({ "transcript.json": "deadbeef" })),
            ),
            ("transcript.json", b"[]"),
        ],
    );

    let storage = Arc::new(Storage::open_in_memory().expect("storage"));
    let config = ImportConfig {
        verify_checksums: false,
        ..Default::default()
    };
    let result = import_session_archive(&storage, None, &archive_path, &config)
        .await
        .expect("checksum check disabled");
    assert!(!result.checksums_verified);
}

#[tokio::test]
async fn import_reports_a_missing_manifest() {
    let dir = tempfile::tempdir().expect("dir");
    let archive_path = dir.path().join("no-manifest.tar.gz");
    write_crafted_archive(&archive_path, &[("transcript.json", b"[]")]);

    let storage = Arc::new(Storage::open_in_memory().expect("storage"));
    let err = import_session_archive(&storage, None, &archive_path, &ImportConfig::default())
        .await
        .expect_err("a missing manifest must be rejected");
    assert!(err.to_string().contains("manifest.json"), "err: {err}");
}

#[tokio::test]
async fn import_reports_a_malformed_manifest() {
    let dir = tempfile::tempdir().expect("dir");
    let archive_path = dir.path().join("bad-manifest.tar.gz");
    write_crafted_archive(
        &archive_path,
        &[
            ("manifest.json", b"{ not json }"),
            ("transcript.json", b"[]"),
        ],
    );

    let storage = Arc::new(Storage::open_in_memory().expect("storage"));
    let err = import_session_archive(&storage, None, &archive_path, &ImportConfig::default())
        .await
        .expect_err("a malformed manifest must be rejected");
    assert!(err.to_string().contains("parse manifest"), "err: {err}");
}

#[tokio::test]
async fn import_reports_a_malformed_transcript() {
    let dir = tempfile::tempdir().expect("dir");
    let archive_path = dir.path().join("bad-transcript.tar.gz");
    write_crafted_archive(
        &archive_path,
        &[
            ("manifest.json", &manifest_json("/tmp", json!({}))),
            ("transcript.json", b"{ not a list }"),
        ],
    );

    let storage = Arc::new(Storage::open_in_memory().expect("storage"));
    let err = import_session_archive(&storage, None, &archive_path, &ImportConfig::default())
        .await
        .expect_err("a malformed transcript must be rejected");
    assert!(err.to_string().contains("parse transcript"), "err: {err}");
}

#[tokio::test]
async fn import_reports_a_missing_transcript() {
    let dir = tempfile::tempdir().expect("dir");
    let archive_path = dir.path().join("no-transcript.tar.gz");
    write_crafted_archive(
        &archive_path,
        &[("manifest.json", &manifest_json("/tmp", json!({})))],
    );

    let storage = Arc::new(Storage::open_in_memory().expect("storage"));
    let err = import_session_archive(&storage, None, &archive_path, &ImportConfig::default())
        .await
        .expect_err("a missing transcript must be rejected");
    assert!(err.to_string().contains("transcript.json"), "err: {err}");
}
