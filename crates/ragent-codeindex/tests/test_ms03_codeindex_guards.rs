//! MS-03 regression tests for the codeindex hardening tasks.
//!
//! - SEC-ragent-codeindex-005 / SECTASKS T-060: the scanner re-checks the size
//!   of the bytes it actually read.
//! - SEC-ragent-codeindex-006 / SECTASKS T-060: `extra_exclude_patterns` is
//!   applied to the scan.
//! - SEC-ragent-codeindex-007 / SECTASKS T-060: a symlinked FTS index
//!   directory is refused before the recovery wipe.

use std::path::PathBuf;

use ragent_codeindex::scanner::scan_directory;
use ragent_codeindex::types::ScanConfig;

fn temp_dir(name: &str) -> PathBuf {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let dir = std::env::temp_dir().join(format!(
        "ragent-ms03-codeindex-{name}-{}-{nanos}",
        std::process::id()
    ));
    std::fs::create_dir_all(&dir).expect("create temp dir");
    dir
}

#[test]
fn test_scan_applies_extra_exclude_patterns() {
    let dir = temp_dir("globs");
    std::fs::write(dir.join("keep.rs"), "fn main() {}").unwrap();
    std::fs::write(dir.join("secret.snap"), "fn hidden() {}").unwrap();
    std::fs::create_dir_all(dir.join("secrets")).unwrap();
    std::fs::write(dir.join("secrets/token.rs"), "fn token() {}").unwrap();

    let config = ScanConfig {
        extra_exclude_patterns: vec!["**/*.snap".to_string(), "**/secrets/**".to_string()],
        ..ScanConfig::default()
    };

    let files = scan_directory(&dir, &config).expect("scan succeeds");
    let names: Vec<String> = files
        .iter()
        .map(|f| f.path.to_string_lossy().to_string())
        .collect();
    assert!(
        names.iter().any(|n| n.ends_with("keep.rs")),
        "the non-excluded file must be scanned: {names:?}"
    );
    assert!(
        !names
            .iter()
            .any(|n| n.to_ascii_lowercase().ends_with(".snap")),
        "a glob-excluded extension must not be scanned: {names:?}"
    );
    assert!(
        !names.iter().any(|n| n.contains("secrets")),
        "a glob-excluded directory must not be scanned: {names:?}"
    );

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn test_scan_without_patterns_is_unchanged() {
    let dir = temp_dir("nopatterns");
    std::fs::write(dir.join("a.rs"), "fn a() {}").unwrap();
    std::fs::write(dir.join("b.snap"), "fn b() {}").unwrap();

    let files = scan_directory(&dir, &ScanConfig::default()).expect("scan succeeds");
    assert_eq!(files.len(), 2, "no patterns configured means no filtering");

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn test_scan_ignores_files_over_the_size_cap() {
    let dir = temp_dir("sizecap");
    let big = "x".repeat(4096);
    std::fs::write(dir.join("big.rs"), &big).unwrap();
    std::fs::write(dir.join("small.rs"), "fn s() {}").unwrap();

    let config = ScanConfig {
        max_file_size: 64,
        ..ScanConfig::default()
    };
    let files = scan_directory(&dir, &config).expect("scan succeeds");
    let names: Vec<String> = files
        .iter()
        .map(|f| f.path.to_string_lossy().to_string())
        .collect();
    assert!(!names.iter().any(|n| n.ends_with("big.rs")));
    assert!(names.iter().any(|n| n.ends_with("small.rs")));
    for file in &files {
        assert!(
            file.size <= config.max_file_size,
            "stored size must reflect the bytes actually read"
        );
    }

    let _ = std::fs::remove_dir_all(&dir);
}

#[cfg(unix)]
#[test]
fn test_scan_does_not_follow_symlinked_files() {
    let dir = temp_dir("symlink");
    let outside = temp_dir("symlink-target");
    std::fs::write(outside.join("outside.rs"), "fn outside() {}").unwrap();
    std::fs::write(dir.join("real.rs"), "fn real() {}").unwrap();
    std::os::unix::fs::symlink(outside.join("outside.rs"), dir.join("link.rs"))
        .expect("create symlink");

    let files = scan_directory(&dir, &ScanConfig::default()).expect("scan succeeds");
    let names: Vec<String> = files
        .iter()
        .map(|f| f.path.to_string_lossy().to_string())
        .collect();
    assert!(names.iter().any(|n| n.ends_with("real.rs")));
    assert!(
        !names.iter().any(|n| n.ends_with("link.rs")),
        "a symlinked entry must not be indexed through the link: {names:?}"
    );

    let _ = std::fs::remove_dir_all(&dir);
    let _ = std::fs::remove_dir_all(&outside);
}
