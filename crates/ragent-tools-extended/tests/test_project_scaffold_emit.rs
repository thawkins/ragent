//! Tests for the FR-016 no-silent-overwrite emitter (T-007, spec `newproj`).
//!
//! Verifies that existing scaffold files (`.gitignore`, `AGENTS.md`, and any
//! other planned path) are reported and left byte-for-byte untouched, that
//! missing files and their parent directories are created, and that the
//! emission report classifies and merges correctly.

use std::fs;

use ragent_tools_extended::project_scaffold::{
    AppType, EmitReport, FileStatus, Language, PlannedFile, ScaffoldError, emit_file, emit_files,
    ensure_workspace_dirs, plan_app_layout, recipe_for, workspace_artifacts,
};

/// Create an empty temp directory for an emission target.
fn temp_root(name: &str) -> tempfile::TempDir {
    tempfile::Builder::new()
        .prefix(name)
        .tempdir()
        .expect("tempdir")
}

/// Read a file under the temp root as UTF-8.
fn read(root: &tempfile::TempDir, path: &str) -> String {
    fs::read_to_string(root.path().join(path)).expect("readable")
}

// ------------------------------------------------------- single-file emit ---

#[test]
fn test_emit_file_creates_file_with_exact_content() {
    let root = temp_root("emit-create");
    let status = emit_file(root.path(), "Cargo.toml", "[package]\n").expect("emit");
    assert_eq!(status, FileStatus::Created);
    assert_eq!(read(&root, "Cargo.toml"), "[package]\n");
}

#[test]
fn test_emit_file_skips_existing_and_leaves_untouched() {
    // FR-016 core: existing content wins, the scaffold stub is discarded.
    let root = temp_root("emit-skip");
    fs::write(root.path().join("AGENTS.md"), b"user rules\n").expect("write");
    let status = emit_file(root.path(), "AGENTS.md", "scaffold stub\n").expect("emit");
    assert_eq!(status, FileStatus::SkippedExisting);
    assert_eq!(read(&root, "AGENTS.md"), "user rules\n");
}

#[test]
fn test_emit_file_creates_nested_parent_directories() {
    let root = temp_root("emit-nested");
    let status = emit_file(root.path(), ".ragent/agents/README.md", "hi\n").expect("emit");
    assert_eq!(status, FileStatus::Created);
    assert!(root.path().join(".ragent/agents/README.md").is_file());
}

// ------------------------------------------------------------- reporting ---

#[test]
fn test_emit_files_classifies_created_and_skipped() {
    let root = temp_root("emit-classify");
    fs::write(root.path().join("kept.txt"), b"old\n").expect("write");
    let files = vec![
        PlannedFile {
            path: "kept.txt".to_owned(),
            content: "new\n".to_owned(),
        },
        PlannedFile {
            path: "fresh.txt".to_owned(),
            content: "fresh\n".to_owned(),
        },
    ];
    let report = emit_files(root.path(), &files).expect("emit");
    assert_eq!(report.created, vec!["fresh.txt".to_owned()]);
    assert_eq!(report.skipped_existing, vec!["kept.txt".to_owned()]);
    assert_eq!(read(&root, "kept.txt"), "old\n");
}

#[test]
fn test_emit_report_merge_combines_lists() {
    let mut first = EmitReport::default();
    first.record(FileStatus::Created, "x");
    first.record(FileStatus::SkippedExisting, "y");
    let mut second = EmitReport::default();
    second.record(FileStatus::Created, "z");
    second.record(FileStatus::SkippedExisting, "w");
    first.merge(second);
    assert_eq!(first.created, vec!["x".to_owned(), "z".to_owned()]);
    assert_eq!(first.skipped_existing, vec!["y".to_owned(), "w".to_owned()]);
}

// --------------------------------------------------- spec-named FR-016 ------

#[test]
fn test_emit_preserves_preexisting_gitignore_and_agents_md() {
    let root = temp_root("emit-named");
    fs::write(root.path().join(".gitignore"), b"*.tmp\n").expect("write");
    fs::write(root.path().join("AGENTS.md"), b"mine\n").expect("write");
    let recipe = recipe_for(Language::Rust).expect("recipe");
    let artifacts = workspace_artifacts(recipe, "myproj");
    let report = emit_files(root.path(), &artifacts).expect("emit");
    assert!(report.skipped_existing.contains(&".gitignore".to_owned()));
    assert!(report.skipped_existing.contains(&"AGENTS.md".to_owned()));
    assert!(report.created.contains(&".ragent/config.json".to_owned()));
    assert_eq!(read(&root, ".gitignore"), "*.tmp\n");
    assert_eq!(read(&root, "AGENTS.md"), "mine\n");
}

#[test]
fn test_emit_workspace_artifacts_twice_all_skipped() {
    let root = temp_root("emit-twice");
    let recipe = recipe_for(Language::Go).expect("recipe");
    let artifacts = workspace_artifacts(recipe, "myproj");
    let first = emit_files(root.path(), &artifacts).expect("first emit");
    assert_eq!(first.created.len(), 5);
    assert!(first.skipped_existing.is_empty());
    let second = emit_files(root.path(), &artifacts).expect("second emit");
    assert!(second.created.is_empty());
    assert_eq!(second.skipped_existing.len(), 5);
    // Spot-check: content unchanged by the second pass.
    assert!(read(&root, "specs/README.md").contains("/spec"));
}

#[test]
fn test_emit_layout_and_workspace_merge_report() {
    let root = temp_root("emit-mixed");
    ensure_workspace_dirs(root.path()).expect("dirs");
    let recipe = recipe_for(Language::Rust).expect("recipe");
    let layout = plan_app_layout(recipe, AppType::Cmdline, "myproj");
    let layout_report = emit_files(root.path(), &layout.files).expect("layout emit");
    let workspace_report =
        emit_files(root.path(), &workspace_artifacts(recipe, "myproj")).expect("workspace emit");
    let mut report = layout_report;
    report.merge(workspace_report);
    assert_eq!(report.created.len(), 7);
    assert!(report.skipped_existing.is_empty());
    assert!(root.path().join("src/main.rs").is_file());
    assert!(root.path().join(".ragent/config.json").is_file());
}

// --------------------------------------------------- workspace directories --

#[test]
fn test_ensure_workspace_dirs_creates_and_is_idempotent() {
    let root = temp_root("emit-dirs");
    ensure_workspace_dirs(root.path()).expect("ensure");
    for dir in [".ragent/agents", "specs", "log"] {
        assert!(root.path().join(dir).is_dir(), "{dir}");
    }
    ensure_workspace_dirs(root.path()).expect("second ensure");
}

// --------------------------------------------------------------- failures ---

#[test]
fn test_emit_error_propagates_path_context() {
    let root = temp_root("emit-error");
    // `src` exists as a FILE, so creating it as a parent directory fails.
    fs::write(root.path().join("src"), b"not a dir\n").expect("write");
    let files = vec![PlannedFile {
        path: "src/main.rs".to_owned(),
        content: "fn main() {}\n".to_owned(),
    }];
    let err = emit_files(root.path(), &files).expect_err("must fail");
    match &err {
        ScaffoldError::EmissionFailed { path, .. } => assert_eq!(path, "src/main.rs"),
        other => panic!("expected EmissionFailed, got {other:?}"),
    }
    assert!(err.to_string().contains("src/main.rs"));
}
