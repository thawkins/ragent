//! Inline tests for `archive.rs` - compiled inside the module tree
//! via the `#[path]` delegation left in the source file
//! (ANTIPAT M2 inline-test migration; no behaviour change).

use super::*;

#[test]
fn test_archive_manifest_serialization() {
    let mut files = HashMap::new();
    files.insert("transcript.json".to_string(), "abc123".to_string());

    let manifest = ArchiveManifest {
        manifest_version: MANIFEST_VERSION,
        session_id: "test-session".to_string(),
        session_title: "Test Session".to_string(),
        session_directory: "/tmp/test".to_string(),
        created_at: Utc::now().to_rfc3339(),
        message_count: 10,
        trigger_count: 2,
        cron_job_count: 1,
        loop_state_count: 0,
        files,
        sensitivity_warning: "Warning".to_string(),
    };

    let json = serde_json::to_string(&manifest).unwrap();
    let _back: ArchiveManifest = serde_json::from_str(&json).unwrap();
}

#[test]
fn test_sha256_file_empty() {
    let temp_dir = tempfile::tempdir().unwrap();
    let file_path = temp_dir.path().join("empty.txt");
    fs::write(&file_path, b"").unwrap();

    let hash = sha256_file(&file_path).unwrap();
    // SHA-256 of empty string
    assert_eq!(
        hash,
        "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
    );
}

#[test]
fn test_sha256_file_content() {
    let temp_dir = tempfile::tempdir().unwrap();
    let file_path = temp_dir.path().join("test.txt");
    fs::write(&file_path, b"hello world").unwrap();

    let hash = sha256_file(&file_path).unwrap();
    // SHA-256 of "hello world"
    assert_eq!(
        hash,
        "b94d27b9934d3e08a52e52d7da7dabfac484efe37a5380ee9088f7ace2efcde9"
    );
}

#[test]
fn test_archive_config_defaults() {
    let config = ArchiveConfig::default();
    assert!(config.include_triggers);
    assert!(config.include_cron);
    assert!(config.include_loop_state);
    assert!(!config.include_cost);
}

#[test]
fn test_import_config_defaults() {
    let config = ImportConfig::default();
    assert!(!config.activate_triggers);
    assert!(!config.activate_cron);
    assert!(config.restore_loop_state);
    assert!(config.verify_checksums);
    assert!(!config.import_triggers);
}
