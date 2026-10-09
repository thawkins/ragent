//! Inline tests for `loop_capture.rs` - compiled inside the module tree
//! via the `#[path]` delegation left in the source file
//! (ANTIPAT M2 inline-test migration; no behaviour change).

use super::*;

#[test]
fn test_workspace_files_skips_build_and_session_dirs() {
    let dir = tempfile::tempdir().expect("tempdir");
    let root = dir.path();
    std::fs::write(root.join("top.txt"), "root").expect("root file");
    std::fs::create_dir_all(root.join("src/sub")).expect("src dir");
    std::fs::write(root.join("src/sub/deep.rs"), "code").expect("src file");
    std::fs::create_dir_all(root.join("target/debug")).expect("target dir");
    std::fs::write(root.join("target/debug/binary"), "bin").expect("target file");
    std::fs::create_dir_all(root.join(".git")).expect("git dir");
    std::fs::write(root.join(".git/HEAD"), "ref").expect("git file");

    let files = workspace_files(root);
    let names: Vec<String> = files.iter().map(|p| relative_path(root, p)).collect();
    assert!(names.contains(&"top.txt".to_string()));
    assert!(names.contains(&"src/sub/deep.rs".to_string()));
    assert!(
        !names.iter().any(|n| n.starts_with("target/")),
        "build outputs are skipped: {names:?}"
    );
    assert!(
        !names.iter().any(|n| n.starts_with(".git/")),
        "git internals are skipped: {names:?}"
    );
}

#[test]
fn test_compute_change_summary_counts_match_induced_changes() {
    let dir = tempfile::tempdir().expect("tempdir");
    let root = dir.path();
    std::fs::write(root.join("a.txt"), "original\n").expect("a");
    std::fs::create_dir_all(root.join("sub")).expect("sub");
    std::fs::write(root.join("sub/b.txt"), "kept\n").expect("b");

    let snapshot =
        take_snapshot("s1", "m1", &[root.join("a.txt"), root.join("sub/b.txt")]).expect("snapshot");
    let capture = LoopCapture {
        snapshot: Some(snapshot),
        git: None,
    };

    // Induce: modify a.txt, create c.txt, delete sub/b.txt.
    std::fs::write(root.join("a.txt"), "changed\nlines\n").expect("a2");
    std::fs::write(root.join("c.txt"), "new file\n").expect("c");
    std::fs::remove_file(root.join("sub/b.txt")).expect("rm b");

    let summary = compute_change_summary(&capture, root);
    assert_eq!(summary.modified, 1, "one modified file");
    assert_eq!(summary.created, 1, "one created file");
    assert_eq!(summary.deleted, 1, "one deleted file");
    assert_eq!(
        summary.files,
        vec![
            "a.txt".to_string(),
            "c.txt".to_string(),
            "sub/b.txt".to_string()
        ],
        "affected files are sorted relative paths"
    );
    // a.txt: +2 -1 lines; c.txt: +1 line; sub/b.txt deleted: -1 line.
    assert_eq!(summary.added_lines, 3);
    assert_eq!(summary.deleted_lines, 2);
    assert!(summary.diffstat().contains("+3 -2"));
}

#[test]
fn test_rollback_restores_captured_contents() {
    let dir = tempfile::tempdir().expect("tempdir");
    let root = dir.path();
    std::fs::write(root.join("a.txt"), "original\n").expect("a");
    let snapshot = take_snapshot("s1", "m1", &[root.join("a.txt")]).expect("snap");
    let capture = LoopCapture {
        snapshot: Some(snapshot),
        git: None,
    };
    std::fs::write(root.join("a.txt"), "mutated\n").expect("a2");
    std::fs::write(root.join("created.txt"), "extra\n").expect("created");

    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("runtime");
    runtime
        .block_on(async { rollback_to_capture(capture).await })
        .expect("rollback");

    let restored = std::fs::read_to_string(root.join("a.txt")).expect("read a");
    assert_eq!(restored, "original\n", "the modified file is restored");
    assert!(
        root.join("created.txt").exists(),
        "created files are outside the snapshot's file set and are kept"
    );
    // Rollback is terminal: the capture was consumed by the call above.
    let _ = capture;
}

#[tokio::test]
async fn test_git_state_is_none_outside_a_repository() {
    let dir = tempfile::tempdir().expect("tempdir");
    assert!(
        git_state(dir.path()).await.is_none(),
        "a plain tempdir is not a git repository"
    );
}

#[test]
fn test_capture_without_snapshot_yields_empty_summary() {
    let dir = tempfile::tempdir().expect("tempdir");
    std::fs::write(dir.path().join("x.txt"), "content\n").expect("x");
    let capture = LoopCapture {
        snapshot: None,
        git: None,
    };
    let summary = compute_change_summary(&capture, dir.path());
    assert_eq!(summary, ChangeSummary::default());
}

#[test]
fn test_git_state_describe() {
    let state = GitState {
        branch: "main".to_string(),
        head: "abc123".to_string(),
    };
    assert_eq!(state.describe(), "branch main @ abc123");
}
