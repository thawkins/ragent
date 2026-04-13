//! Integration test: scan a temp directory, store results, modify a file,
//! re-scan, and verify stale detection correctly identifies changes.
#![allow(missing_docs)]

use ragent_code::scanner::scan_directory;
use ragent_code::store::IndexStore;
use ragent_code::types::{ScanConfig, ScannedFile};
use std::fs;
use tempfile::TempDir;

/// Helper: create a temp project with Rust and Python files.
fn create_sample_project() -> TempDir {
    let dir = TempDir::new().expect("create temp dir");
    let root = dir.path();

    // Source files
    fs::create_dir_all(root.join("src")).unwrap();
    fs::write(
        root.join("src/main.rs"),
        "fn main() {\n    println!(\"hello\");\n}\n",
    )
    .unwrap();
    fs::write(
        root.join("src/lib.rs"),
        "pub fn greet() -> &'static str {\n    \"hello\"\n}\n",
    )
    .unwrap();
    fs::write(
        root.join("app.py"),
        "def main():\n    print('hello')\n\nif __name__ == '__main__':\n    main()\n",
    )
    .unwrap();

    // Binary file (contains NUL bytes — should be skipped)
    fs::write(root.join("image.bin"), b"\x89PNG\r\n\x00\x1a\n").unwrap();

    // Empty file (should be skipped)
    fs::write(root.join("empty.rs"), "").unwrap();

    // Gitignored dir content
    fs::create_dir_all(root.join("target/debug")).unwrap();
    fs::write(root.join("target/debug/output"), "should be skipped").unwrap();

    // .gitignore (additional pattern) — needs .git dir for ignore crate to activate
    fs::create_dir_all(root.join(".git")).unwrap();
    fs::write(root.join(".gitignore"), "*.log\n").unwrap();
    fs::write(root.join("debug.log"), "log content").unwrap();

    dir
}

#[test]
fn test_scan_discovers_source_files() {
    let dir = create_sample_project();
    let config = ScanConfig::default();
    let files = scan_directory(dir.path(), &config).unwrap();

    let paths: Vec<String> = files
        .iter()
        .map(|f| f.path.to_string_lossy().to_string())
        .collect();

    // Should find: src/main.rs, src/lib.rs, app.py, .gitignore
    // Should NOT find: image.bin (binary), empty.rs (empty), target/** (excluded dir), debug.log (gitignored)
    assert!(
        paths.iter().any(|p| p.contains("src/main.rs")),
        "should find src/main.rs, got: {paths:?}"
    );
    assert!(
        paths.iter().any(|p| p.contains("src/lib.rs")),
        "should find src/lib.rs, got: {paths:?}"
    );
    assert!(
        paths.iter().any(|p| p.contains("app.py")),
        "should find app.py, got: {paths:?}"
    );
    assert!(
        !paths.iter().any(|p| p.contains("image.bin")),
        "should skip binary file, got: {paths:?}"
    );
    assert!(
        !paths.iter().any(|p| p.contains("empty.rs")),
        "should skip empty file, got: {paths:?}"
    );
    assert!(
        !paths.iter().any(|p| p.contains("target")),
        "should skip target/ dir, got: {paths:?}"
    );
    assert!(
        !paths.iter().any(|p| p.contains("debug.log")),
        "should skip gitignored files, got: {paths:?}"
    );
}

#[test]
fn test_scan_language_detection() {
    let dir = create_sample_project();
    let config = ScanConfig::default();
    let files = scan_directory(dir.path(), &config).unwrap();

    let rs_files: Vec<&ScannedFile> = files
        .iter()
        .filter(|f| f.language.as_deref() == Some("rust"))
        .collect();
    let py_files: Vec<&ScannedFile> = files
        .iter()
        .filter(|f| f.language.as_deref() == Some("python"))
        .collect();

    assert_eq!(rs_files.len(), 2, "should find 2 Rust files");
    assert_eq!(py_files.len(), 1, "should find 1 Python file");
}

#[test]
fn test_scan_hashing() {
    let dir = create_sample_project();
    let config = ScanConfig::default();
    let files = scan_directory(dir.path(), &config).unwrap();

    // All scanned files should have non-empty hashes.
    for file in &files {
        assert!(
            !file.hash.is_empty(),
            "hash should not be empty for {:?}",
            file.path
        );
        assert!(
            file.hash.len() == 64,
            "blake3 hash should be 64 hex chars, got {}",
            file.hash.len()
        );
    }

    // Deterministic: scanning again should produce the same hashes.
    let files2 = scan_directory(dir.path(), &config).unwrap();
    for (a, b) in files.iter().zip(files2.iter()) {
        if a.path == b.path {
            assert_eq!(
                a.hash, b.hash,
                "hash should be deterministic for {:?}",
                a.path
            );
        }
    }
}

#[test]
fn test_scan_to_store_round_trip() {
    let dir = create_sample_project();
    let config = ScanConfig::default();

    // Step 1: Scan
    let files = scan_directory(dir.path(), &config).unwrap();
    assert!(files.len() >= 3, "should scan at least 3 source files");

    // Step 2: Store
    let store = IndexStore::open_in_memory().unwrap();
    let diff = store.get_stale_files(&files).unwrap();

    // First time: everything is "to_add"
    assert_eq!(diff.to_add.len(), files.len());
    assert!(diff.to_update.is_empty());
    assert!(diff.to_remove.is_empty());

    // Apply
    store.apply_diff(&diff).unwrap();
    assert_eq!(store.file_count().unwrap(), files.len() as u64);

    // Step 3: Verify stored data
    let stored = store.list_files().unwrap();
    assert_eq!(stored.len(), files.len());
    for entry in &stored {
        assert!(!entry.content_hash.is_empty());
        assert!(entry.byte_size > 0);
    }

    // Step 4: Re-scan — no changes
    let files2 = scan_directory(dir.path(), &config).unwrap();
    let diff2 = store.get_stale_files(&files2).unwrap();
    assert!(
        diff2.is_empty(),
        "no changes expected, got: add={}, update={}, remove={}",
        diff2.to_add.len(),
        diff2.to_update.len(),
        diff2.to_remove.len()
    );
}

#[test]
fn test_incremental_change_detection() {
    let dir = create_sample_project();
    let config = ScanConfig::default();

    // Initial scan and store
    let files = scan_directory(dir.path(), &config).unwrap();
    let store = IndexStore::open_in_memory().unwrap();
    let diff = store.get_stale_files(&files).unwrap();
    store.apply_diff(&diff).unwrap();

    // Modify one file
    fs::write(
        dir.path().join("src/main.rs"),
        "fn main() {\n    println!(\"modified\");\n}\n",
    )
    .unwrap();

    // Add a new file
    fs::write(dir.path().join("src/new_module.rs"), "pub fn new_fn() {}\n").unwrap();

    // Delete a file
    fs::remove_file(dir.path().join("app.py")).unwrap();

    // Re-scan
    let files2 = scan_directory(dir.path(), &config).unwrap();
    let diff2 = store.get_stale_files(&files2).unwrap();

    // src/main.rs should be in to_update (hash changed)
    assert!(
        diff2
            .to_update
            .iter()
            .any(|f| f.path.to_string_lossy().contains("main.rs")),
        "modified file should be in to_update: {:?}",
        diff2
            .to_update
            .iter()
            .map(|f| f.path.to_string_lossy().to_string())
            .collect::<Vec<_>>()
    );

    // src/new_module.rs should be in to_add
    assert!(
        diff2
            .to_add
            .iter()
            .any(|f| f.path.to_string_lossy().contains("new_module.rs")),
        "new file should be in to_add: {:?}",
        diff2
            .to_add
            .iter()
            .map(|f| f.path.to_string_lossy().to_string())
            .collect::<Vec<_>>()
    );

    // app.py should be in to_remove
    assert!(
        diff2.to_remove.iter().any(|p| p.contains("app.py")),
        "deleted file should be in to_remove: {:?}",
        diff2.to_remove
    );

    // Apply and verify counts
    store.apply_diff(&diff2).unwrap();

    // Original was ~4 files (main.rs, lib.rs, app.py, .gitignore)
    // After: main.rs (updated), lib.rs (same), new_module.rs (added), .gitignore (same), app.py (removed)
    let _final_count = store.file_count().unwrap();
    let final_files = store.list_files().unwrap();
    let final_paths: Vec<&str> = final_files.iter().map(|f| f.path.as_str()).collect();

    assert!(
        !final_paths.iter().any(|p| p.contains("app.py")),
        "app.py should be removed from index"
    );
    assert!(
        final_paths.iter().any(|p| p.contains("new_module.rs")),
        "new_module.rs should be in index"
    );
}

#[test]
fn test_max_file_size_filter() {
    let dir = TempDir::new().unwrap();

    // Create a file that exceeds the size limit
    let big_content = "x".repeat(2_000_000); // 2 MB
    fs::write(dir.path().join("big.rs"), &big_content).unwrap();
    fs::write(dir.path().join("small.rs"), "fn small() {}").unwrap();

    let config = ScanConfig {
        max_file_size: 1_048_576, // 1 MB
        ..Default::default()
    };
    let files = scan_directory(dir.path(), &config).unwrap();

    let paths: Vec<String> = files
        .iter()
        .map(|f| f.path.to_string_lossy().to_string())
        .collect();
    assert!(
        !paths.iter().any(|p| p.contains("big.rs")),
        "oversized file should be skipped"
    );
    assert!(
        paths.iter().any(|p| p.contains("small.rs")),
        "small file should be included"
    );
}

#[test]
fn test_store_on_disk() {
    let dir = TempDir::new().unwrap();
    let db_path = dir.path().join("index.db");

    // Create, insert, close.
    {
        let store = IndexStore::open(&db_path).unwrap();
        let entry = ragent_code::types::FileEntry {
            path: "src/main.rs".to_string(),
            content_hash: "testhash".to_string(),
            byte_size: 42,
            language: Some("rust".to_string()),
            last_indexed: chrono::Utc::now(),
            mtime_ns: 1_000_000,
            line_count: 5,
        };
        store.upsert_file(&entry).unwrap();
    }

    // Re-open and verify persistence.
    {
        let store = IndexStore::open(&db_path).unwrap();
        let got = store.get_file("src/main.rs").unwrap();
        assert!(got.is_some());
        assert_eq!(got.unwrap().content_hash, "testhash");
    }
}

#[test]
fn test_extra_exclude_dirs() {
    let dir = TempDir::new().unwrap();
    fs::create_dir_all(dir.path().join("vendor/lib")).unwrap();
    fs::write(dir.path().join("vendor/lib/dep.rs"), "fn dep() {}").unwrap();
    fs::write(dir.path().join("main.rs"), "fn main() {}").unwrap();

    let config = ScanConfig {
        extra_exclude_dirs: vec!["vendor".to_string()],
        ..Default::default()
    };
    let files = scan_directory(dir.path(), &config).unwrap();
    let paths: Vec<String> = files
        .iter()
        .map(|f| f.path.to_string_lossy().to_string())
        .collect();

    assert!(
        !paths.iter().any(|p| p.contains("vendor")),
        "extra excluded dir should be skipped"
    );
    assert!(
        paths.iter().any(|p| p.contains("main.rs")),
        "non-excluded files should be included"
    );
}
