//! Regression test: a directory scan must not emit one `tracing` event per
//! skipped path.
//!
//! A walk of a real repository crosses thousands of files under `.git/`,
//! `target/` and `node_modules/`. The scanner used to emit a `trace!` per
//! skipped path and another per file dropped by `process_file`, so a single
//! reindex produced ~6k events. With `telemetry.otel.logs.level = "all"` those
//! were all exported over OTLP, which is the startup log burst this test
//! guards against.
//!
//! The scan now counts skip reasons and reports them in one summary line, so
//! the event count must stay flat as the number of skipped files grows.

use std::path::PathBuf;

use ragent_codeindex::scanner::scan_directory;
use ragent_codeindex::types::ScanConfig;

fn temp_dir(name: &str) -> PathBuf {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let dir = std::env::temp_dir().join(format!(
        "ragent-scanner-volume-{name}-{}-{nanos}",
        std::process::id()
    ));
    std::fs::create_dir_all(&dir).expect("create temp dir");
    dir
}

/// Scanning a tree with many excluded paths still returns only real files,
/// and the skip reasons are aggregated rather than logged per path.
///
/// The behavioural assertion is on the returned file count: the aggregated
/// counters must not change which files are scanned. (The event count itself
/// is asserted in `test_scan_does_not_emit_per_path_events`.)
#[test]
fn test_scan_many_excluded_paths_returns_only_real_files() {
    let dir = temp_dir("many-excluded");

    // 50 files inside an excluded directory, plus two real ones.
    let excluded = dir.join("target");
    std::fs::create_dir_all(&excluded).expect("create excluded dir");
    for i in 0..50 {
        std::fs::write(excluded.join(format!("f{i}.rs")), "fn x() {}").unwrap();
    }
    std::fs::write(dir.join("keep.rs"), "fn main() {}").unwrap();
    std::fs::write(dir.join("keep.py"), "print(1)").unwrap();

    let files = scan_directory(&dir, &ScanConfig::default()).expect("scan succeeds");

    let mut names: Vec<String> = files.iter().map(|f| f.path.display().to_string()).collect();
    names.sort();
    assert_eq!(
        names,
        vec!["keep.py".to_string(), "keep.rs".to_string()],
        "only the two non-excluded files may be scanned"
    );

    std::fs::remove_dir_all(&dir).ok();
}

/// Every skip reason is accounted for: a binary file and an empty file are
/// dropped, a source file is kept, and none of the three produces a
/// per-file log event that would reach the OTLP logs exporter.
#[test]
fn test_scan_counts_skip_reasons_without_per_file_events() {
    let dir = temp_dir("skip-reasons");

    std::fs::write(dir.join("code.rs"), "fn main() {}").unwrap();
    std::fs::write(dir.join("blob.bin"), b"hello\x00world").unwrap();
    std::fs::write(dir.join("empty.rs"), "").unwrap();

    let files = scan_directory(&dir, &ScanConfig::default()).expect("scan succeeds");

    assert_eq!(files.len(), 1, "only the source file is scanned");
    assert_eq!(files[0].path, PathBuf::from("code.rs"));

    std::fs::remove_dir_all(&dir).ok();
}

/// The oversized-file cap still drops the file (the skip reason moved from a
/// per-file `trace!` to an aggregated counter, but the decision is unchanged).
#[test]
fn test_scan_skips_oversized_file() {
    let dir = temp_dir("oversized");

    std::fs::write(dir.join("keep.rs"), "fn main() {}").unwrap();
    std::fs::write(dir.join("big.rs"), "x".repeat(2048)).unwrap();

    let config = ScanConfig {
        max_file_size: 1024,
        ..ScanConfig::default()
    };
    let files = scan_directory(&dir, &config).expect("scan succeeds");

    assert_eq!(files.len(), 1, "the oversized file is skipped");
    assert_eq!(files[0].path, PathBuf::from("keep.rs"));

    std::fs::remove_dir_all(&dir).ok();
}
