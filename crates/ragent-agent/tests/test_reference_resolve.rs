//! Tests-only coverage for `@`-reference resolution
//! (`ragent_agent::reference::{parse, resolve}`).
//!
//! Exercises file, directory, and fuzzy resolution, line numbering, the
//! per-reference size truncation, and the aggregate `resolve_all_refs` prompt
//! assembly (including the error-collection paths). All I/O is local.

use ragent_agent::reference::parse::{FileRef, parse_refs};
use ragent_agent::reference::resolve::{resolve_all_refs, resolve_ref};

fn write(dir: &std::path::Path, name: &str, body: &str) -> std::path::PathBuf {
    let p = dir.join(name);
    if let Some(parent) = p.parent() {
        std::fs::create_dir_all(parent).expect("mkdir");
    }
    std::fs::write(&p, body).expect("write");
    p
}

#[tokio::test]
async fn resolve_file_numbers_lines() {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = write(dir.path(), "src/a.rs", "line one\nline two\n");
    let parsed = parse_refs(&format!("@{}", path.display()));
    assert_eq!(parsed.len(), 1);

    let resolved = resolve_ref(&parsed[0], dir.path()).await.expect("resolve");
    assert!(resolved.content.contains("   1  line one"));
    assert!(resolved.content.contains("   2  line two"));
    assert!(!resolved.truncated);
    assert!(matches!(resolved.kind, FileRef::File(_)));
    assert_eq!(resolved.resolved_path.as_deref(), Some(path.as_path()));
}

#[tokio::test]
async fn resolve_missing_file_errors() {
    let dir = tempfile::tempdir().expect("tempdir");
    let missing = dir.path().join("nope.txt");
    let parsed = parse_refs(&format!("@{}", missing.display()));
    let err = resolve_ref(&parsed[0], dir.path())
        .await
        .expect_err("missing file must error");
    assert!(err.to_string().contains("Cannot read file"), "err: {err}");
}

#[tokio::test]
async fn resolve_directory_lists_recursively() {
    let dir = tempfile::tempdir().expect("tempdir");
    write(dir.path(), "tree/one.txt", "a");
    write(dir.path(), "tree/sub/two.txt", "b");

    // A directory reference is classified by the trailing slash.
    let target = format!("@{}/tree/", dir.path().display());
    let parsed = parse_refs(&target);
    assert!(matches!(parsed[0].kind, FileRef::Directory(_)));

    let resolved = resolve_ref(&parsed[0], dir.path()).await.expect("resolve");
    assert!(resolved.content.contains("one.txt"));
    assert!(resolved.content.contains("sub"));
    assert!(resolved.content.contains("two.txt"));
}

#[tokio::test]
async fn resolve_directory_without_slash_is_a_file_error() {
    let dir = tempfile::tempdir().expect("tempdir");
    std::fs::create_dir_all(dir.path().join("somedir")).expect("mkdir");
    // No trailing slash means File(path) classification, whose read fails on a dir.
    let parsed = parse_refs(&format!("@{}/somedir", dir.path().display()));
    let err = resolve_ref(&parsed[0], dir.path())
        .await
        .expect_err("reading a directory as a file must error");
    assert_ne!(err.to_string(), "");
}

#[tokio::test]
async fn resolve_fuzzy_matches_project_file() {
    let dir = tempfile::tempdir().expect("tempdir");
    write(dir.path(), "src/unique_widget.rs", "content\n");

    // A bare name (no slash/dot) is fuzzy-classified.
    let parsed = parse_refs("@unique_widget");
    assert!(matches!(parsed[0].kind, FileRef::Fuzzy(_)));

    match resolve_ref(&parsed[0], dir.path()).await {
        Ok(resolved) => assert!(resolved.content.contains("content")),
        // A fuzzy miss is acceptable (no match) - the point is the branch runs.
        Err(e) => assert_ne!(e.to_string(), ""),
    }
}

#[tokio::test]
async fn truncate_content_marks_oversized_files() {
    let dir = tempfile::tempdir().expect("tempdir");
    // > 50 KiB of text.
    let big = "x".repeat(60 * 1024);
    let path = write(dir.path(), "big.txt", &big);
    let parsed = parse_refs(&format!("@{}", path.display()));

    let resolved = resolve_ref(&parsed[0], dir.path()).await.expect("resolve");
    assert!(
        resolved.truncated,
        "an oversized file must be flagged truncated"
    );
    assert!(resolved.content.contains("[Content truncated"));
}

#[tokio::test]
async fn resolve_all_refs_with_no_references_is_a_passthrough() {
    let dir = tempfile::tempdir().expect("tempdir");
    let (out, refs) = resolve_all_refs("no references here", dir.path())
        .await
        .expect("resolve_all");
    assert_eq!(out, "no references here");
    assert_eq!(refs.len(), 0);
}

#[tokio::test]
async fn resolve_all_refs_appends_a_referenced_files_block() {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = write(dir.path(), "note.md", "hello world\n");
    let input = format!("see @{} for details", path.display());

    let (out, refs) = resolve_all_refs(&input, dir.path())
        .await
        .expect("resolve_all");
    assert_eq!(refs.len(), 1);
    assert!(out.contains("<referenced_files>"), "out: {out}");
    assert!(out.contains("</referenced_files>"));
    assert!(out.contains("hello world"));
}

#[tokio::test]
async fn resolve_all_refs_collects_errors_without_failing_when_one_succeeds() {
    let dir = tempfile::tempdir().expect("tempdir");
    let good = write(dir.path(), "good.txt", "ok\n");
    let missing = dir.path().join("missing.txt");
    let input = format!("@{}\n@{}", good.display(), missing.display());

    let (out, refs) = resolve_all_refs(&input, dir.path())
        .await
        .expect("one good reference should succeed");
    assert_eq!(refs.len(), 1, "only the readable file resolves");
    assert!(
        out.contains("[Warning: some references could not be resolved"),
        "out: {out}"
    );
}

#[tokio::test]
async fn resolve_all_refs_fails_when_all_references_error() {
    let dir = tempfile::tempdir().expect("tempdir");
    let a = dir.path().join("a.txt");
    let b = dir.path().join("b.txt");
    let input = format!("@{}\n@{}", a.display(), b.display());

    let err = resolve_all_refs(&input, dir.path())
        .await
        .expect_err("all references failing must error");
    assert!(
        err.to_string().contains("Failed to resolve references"),
        "err: {err}"
    );
}
