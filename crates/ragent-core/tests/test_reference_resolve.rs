//! External tests for `ragent_core::reference::resolve`.

use ragent_core::reference::parse::parse_refs;
use ragent_core::reference::resolve::resolve_all_refs;
use std::fs;
use std::path::PathBuf;

/// Create a temporary directory with a unique name for testing.
fn make_temp_dir(suffix: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("ragent_test_ref_{suffix}_{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).expect("create temp dir");
    dir
}

/// Clean up a test directory.
fn cleanup(dir: &PathBuf) {
    let _ = fs::remove_dir_all(dir);
}

#[tokio::test]
async fn test_resolve_existing_file() {
    let dir = make_temp_dir("existing");
    let file_path = dir.join("test.txt");
    fs::write(&file_path, "hello world").expect("write");

    let input = format!("see @{}", file_path.display());
    let (resolved, refs) = resolve_all_refs(&input, &dir).await.expect("resolve");

    assert!(!refs.is_empty(), "should have resolved refs");
    assert!(
        resolved.contains("<referenced_files>"),
        "should contain XML block"
    );
    assert!(resolved.contains("hello world"), "should contain file content");
    cleanup(&dir);
}

#[tokio::test]
async fn test_resolve_missing_file() {
    let dir = make_temp_dir("missing");
    let input = "see @nonexistent_file_xyz.rs";
    // Missing file should return an error from resolve_all_refs
    let result = resolve_all_refs(input, &dir).await;
    assert!(result.is_err(), "missing file should produce an error");
    cleanup(&dir);
}

#[tokio::test]
async fn test_resolve_directory() {
    let dir = make_temp_dir("dir");
    fs::create_dir(dir.join("subdir")).expect("mkdir");
    fs::write(dir.join("subdir/file.txt"), "content").expect("write");

    let input = format!("list @{}/", dir.join("subdir").display());
    let (resolved, refs) = resolve_all_refs(&input, &dir).await.expect("resolve");

    assert!(!refs.is_empty());
    assert!(resolved.contains("<referenced_files>"));
    cleanup(&dir);
}

#[tokio::test]
async fn test_resolve_no_refs() {
    let dir = make_temp_dir("norefs");
    let input = "no refs here";
    let (resolved, refs) = resolve_all_refs(input, &dir).await.expect("resolve");

    assert_eq!(resolved, input);
    assert!(refs.is_empty());
    cleanup(&dir);
}

#[tokio::test]
async fn test_resolve_multiple_refs() {
    let dir = make_temp_dir("multi");
    fs::write(dir.join("a.txt"), "alpha").expect("write");
    fs::write(dir.join("b.txt"), "beta").expect("write");

    let input = format!(
        "compare @{} with @{}",
        dir.join("a.txt").display(),
        dir.join("b.txt").display()
    );
    let (resolved, refs) = resolve_all_refs(&input, &dir).await.expect("resolve");

    assert_eq!(refs.len(), 2);
    assert!(resolved.contains("alpha"));
    assert!(resolved.contains("beta"));
    cleanup(&dir);
}

#[tokio::test]
async fn test_resolve_large_file_truncated() {
    let dir = make_temp_dir("large");
    let file_path = dir.join("big.txt");
    let content = "x".repeat(60_000);
    fs::write(&file_path, &content).expect("write");

    let input = format!("see @{}", file_path.display());
    let (resolved, refs) = resolve_all_refs(&input, &dir).await.expect("resolve");

    assert!(!refs.is_empty());
    if let Some(r) = refs.first() {
        assert!(r.truncated, "large file should be truncated");
    }
    assert!(
        resolved.contains("[truncated]") || resolved.len() < content.len(),
        "resolved content should be truncated"
    );
    cleanup(&dir);
}

#[test]
fn test_parse_refs_for_resolution() {
    let refs = parse_refs("@src/main.rs and @Cargo.toml");
    assert_eq!(refs.len(), 2);
}
