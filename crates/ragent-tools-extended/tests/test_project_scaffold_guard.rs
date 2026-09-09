//! Tests for the FR-002 empty-directory guard I/O shell (T-006, spec
//! `newproj`).
//!
//! The pure emptiness decision is covered in `test_project_scaffold_flags.rs`;
//! these tests exercise the real-filesystem layer: entry gathering, hidden
//! entries, unreadable targets, and the end-to-end guard flow that refuses
//! before any filesystem mutation.

use std::fs;
use std::path::Path;

use ragent_tools_extended::project_scaffold::{
    ScaffoldError, enforce_empty_directory_guard, read_target_entries,
};

/// Create an empty temp directory for a scaffold target.
fn temp_target(name: &str) -> tempfile::TempDir {
    tempfile::Builder::new()
        .prefix(name)
        .tempdir()
        .expect("tempdir")
}

/// Assert the error is `DirectoryNotEmpty` carrying exactly `expected`.
fn assert_directory_not_empty(err: ScaffoldError, expected: &[&str]) {
    match err {
        ScaffoldError::DirectoryNotEmpty(offending) => {
            assert_eq!(offending, expected.to_vec());
        }
        other => panic!("expected DirectoryNotEmpty, got {other:?}"),
    }
}

// ---------------------------------------------------------- entry gathering ---

#[test]
fn test_guard_empty_directory_yields_no_entries() {
    let target = temp_target("guard-empty");
    let entries = read_target_entries(target.path()).expect("readable");
    assert!(entries.is_empty());
}

#[test]
fn test_guard_gathers_files_and_dirs() {
    let target = temp_target("guard-mixed");
    fs::create_dir(target.path().join("src")).expect("mkdir");
    fs::write(target.path().join("keepme.txt"), b"x").expect("write");
    let entries = read_target_entries(target.path()).expect("readable");
    assert_eq!(entries.len(), 2);
    assert!(entries.iter().any(|e| e.name == "src" && e.is_dir));
    assert!(entries.iter().any(|e| e.name == "keepme.txt" && !e.is_dir));
}

#[test]
fn test_guard_hidden_entries_are_included() {
    // Dotfiles are real occupancy signals: a stray `.env` must fail the guard.
    let target = temp_target("guard-hidden");
    fs::write(target.path().join(".env"), b"SECRET=1").expect("write");
    fs::write(target.path().join(".gitkeep"), b"").expect("write");
    assert_directory_not_empty(
        enforce_empty_directory_guard(target.path()).expect_err("must refuse"),
        &[".env", ".gitkeep"],
    );
}

// ------------------------------------------------------- unreadable targets ---

#[test]
fn test_guard_missing_target_is_unreadable() {
    let base = temp_target("guard-missing-base");
    let missing = base.path().join("nope");
    match read_target_entries(&missing) {
        Err(ScaffoldError::TargetUnreadable(detail)) => {
            assert!(detail.contains("not a directory"));
        }
        other => panic!("expected TargetUnreadable, got {other:?}"),
    }
}

#[test]
fn test_guard_file_target_is_unreadable() {
    let base = temp_target("guard-file-base");
    let file_target = base.path().join("plain.txt");
    fs::write(&file_target, b"x").expect("write");
    match enforce_empty_directory_guard(&file_target) {
        Err(ScaffoldError::TargetUnreadable(detail)) => {
            assert!(detail.contains("not a directory"));
        }
        other => panic!("expected TargetUnreadable, got {other:?}"),
    }
}

// ------------------------------------------------ end-to-end guard (FR-002) ---

#[test]
fn test_guard_empty_target_passes() {
    let target = temp_target("guard-pass");
    enforce_empty_directory_guard(target.path()).expect("empty target scaffolds");
}

#[test]
fn test_guard_ragent_allowlist_passes() {
    // Only ragent-owned artifacts (`.ragent`, `log`, `target`) are ignored.
    let target = temp_target("guard-allowlist");
    for dir in [".ragent", "log", "target"] {
        fs::create_dir(target.path().join(dir)).expect("mkdir");
    }
    enforce_empty_directory_guard(target.path()).expect("allowlist-only target scaffolds");
}

#[test]
fn test_guard_allowlist_plus_stray_file_refuses() {
    let target = temp_target("guard-stray");
    fs::create_dir(target.path().join(".ragent")).expect("mkdir");
    fs::create_dir(target.path().join("log")).expect("mkdir");
    fs::write(target.path().join("Cargo.toml"), b"[package]").expect("write");
    assert_directory_not_empty(
        enforce_empty_directory_guard(target.path()).expect_err("must refuse"),
        &["Cargo.toml"],
    );
}

#[test]
fn test_guard_refusal_reports_all_offenders_without_mutation() {
    let target = temp_target("guard-no-mutation");
    fs::create_dir(target.path().join("src")).expect("mkdir");
    fs::write(target.path().join("README.md"), b"x").expect("write");
    assert_directory_not_empty(
        enforce_empty_directory_guard(target.path()).expect_err("must refuse"),
        // Directories are suffixed with `/` in the report.
        &["README.md", "src/"],
    );
    // FR-002: refusal leaves the directory exactly as found.
    let mut names: Vec<String> = fs::read_dir(target.path())
        .expect("readable")
        .map(|e| e.expect("entry").file_name().to_string_lossy().into_owned())
        .collect();
    names.sort();
    assert_eq!(names, vec!["README.md".to_owned(), "src".to_owned()]);
}

#[test]
fn test_guard_error_message_lists_offenders() {
    let target = temp_target("guard-message");
    fs::write(target.path().join("stray.bin"), b"x").expect("write");
    let message = enforce_empty_directory_guard(target.path())
        .expect_err("must refuse")
        .to_string();
    assert!(message.contains("directory is not empty"));
    assert!(message.contains("stray.bin"));
}

#[test]
fn test_guard_symlink_to_dir_reported_as_directory() {
    let base = temp_target("guard-symlink-base");
    let target = base.path().join("target-dir");
    fs::create_dir(&target).expect("mkdir");
    #[cfg(unix)]
    {
        let linked = base.path().join("linked-dir");
        std::os::unix::fs::symlink(base.path(), &linked).expect("symlink");
        let entries = read_target_entries(Path::new(&linked)).expect("readable");
        assert!(
            entries.iter().any(|e| e.name == "target-dir" && e.is_dir),
            "symlinked directory must be classified as a directory"
        );
    }
    #[cfg(not(unix))]
    {
        let _ = &target; // keep `target` used on non-unix platforms
    }
}
