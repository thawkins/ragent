//! Tests for the `/spec govcreate` scaffold-reuse stage (spec `govdoc` T-005,
//! FR-010, FR-011).
//!
//! The stage must create the target folder, run the shared empty-directory
//! guard before any other action, then produce exactly the project the shared
//! `project_scaffold` engine produces for the same request - with no second
//! scaffold path.

use std::fs;

use ragent_tools_extended::archdoc::{GovCreateScaffoldError, run_govcreate_scaffold};
use ragent_tools_extended::project_scaffold::{
    Language, ScaffoldSummary, parse_flags, plan_and_emit, recipe_for,
};

/// Fresh temp parent plus a target folder path beneath it.
fn temp_parent(name: &str) -> tempfile::TempDir {
    tempfile::Builder::new()
        .prefix(name)
        .tempdir()
        .expect("tempdir")
}

/// Parse a `/new` flag list into a validated scaffold request.
fn request(flags: &[&str]) -> ragent_tools_extended::project_scaffold::ScaffoldRequest {
    parse_flags(flags).expect("valid scaffold flags")
}

// ----------------------------------------------------- target-folder creation ---

#[test]
fn test_govcreate_creates_missing_target_folder() {
    let parent = temp_parent("govcreate-missing-target");
    let target = parent.path().join("brand-new-project");
    assert!(!target.exists(), "precondition: target does not exist");

    let req = request(&["--language", "rust", "--type", "cmdline"]);
    let outcome = run_govcreate_scaffold(&req, &target).expect("scaffold succeeds");

    assert!(target.is_dir(), "target folder was created");
    assert_eq!(outcome.slug, "brand-new-project");
    assert!(target.join("Cargo.toml").is_file(), "rust layout emitted");
    assert!(target.join("specs").is_dir(), "workspace specs dir emitted");
}

#[test]
fn test_govcreate_creates_nested_missing_target_folder() {
    let parent = temp_parent("govcreate-nested-target");
    let target = parent.path().join("a").join("b").join("proj");

    let req = request(&["--language", "python", "--type", "library"]);
    run_govcreate_scaffold(&req, &target).expect("scaffold succeeds");

    assert!(target.is_dir());
    assert_eq!(
        target.file_name().map(|n| n.to_string_lossy().into_owned()),
        Some("proj".to_owned()),
        "slug is the deepest component"
    );
}

// ----------------------------------------------------------- FR-011 guard ---

#[test]
fn test_govcreate_guard_refuses_non_empty_target() {
    let parent = temp_parent("govcreate-busy-target");
    let target = parent.path().join("busy");
    fs::create_dir(&target).expect("mkdir");
    fs::write(target.join("keepme.txt"), b"occupied").expect("write");

    let req = request(&["--language", "rust", "--type", "cmdline"]);
    let err = run_govcreate_scaffold(&req, &target).expect_err("busy target refuses");

    match err {
        GovCreateScaffoldError::Guard(guard) => {
            let message = guard.to_string();
            assert!(
                message.contains("keepme.txt"),
                "names the blocking entry: {message}"
            );
        }
        other => panic!("expected Guard, got {other:?}"),
    }
    assert_eq!(
        fs::read_dir(&target).expect("read target").count(),
        1,
        "no scaffold files written past the guard"
    );
}

#[test]
fn test_govcreate_guard_allows_artifact_allowlist_only_target() {
    let parent = temp_parent("govcreate-allowlist-target");
    let target = parent.path().join("allowlisted");
    fs::create_dir_all(target.join(".ragent")).expect("mkdir .ragent");
    fs::create_dir_all(target.join("log")).expect("mkdir log");
    fs::create_dir_all(target.join("target")).expect("mkdir target");

    let req = request(&["--language", "rust", "--type", "cmdline"]);
    run_govcreate_scaffold(&req, &target).expect("allowlist-only target scaffolds");
    assert!(target.join("Cargo.toml").is_file());
}

// ------------------------------------------- FR-010 single scaffold engine ---

#[test]
fn test_govcreate_reuses_the_shared_engine_exactly() {
    // FR-010: the govcreate stage must produce the same file set the shared
    // engine produces for the same request - proving there is no second path.
    let parent = temp_parent("govcreate-reuse");
    let via_govcreate = parent.path().join("via-govcreate");
    let via_engine = parent.path().join("via-engine");

    let req = request(&["--language", "rust", "--type", "cmdline", "--stack", "axum"]);
    let outcome = run_govcreate_scaffold(&req, &via_govcreate).expect("govcreate scaffold");
    let created_via_govcreate: Vec<String> = outcome.summary.emit.created.clone();

    let recipe = recipe_for(Language::Rust).expect("rust recipe");
    let (engine_summary, engine_note) = plan_and_emit(
        &via_engine,
        &req,
        "via-engine",
        recipe,
        "2026-01-01T00:00:00Z",
    )
    .expect("engine scaffold");
    let created_via_engine: Vec<String> = engine_summary.emit.created.clone();

    assert_eq!(
        created_via_govcreate, created_via_engine,
        "govcreate emits exactly what the shared engine emits"
    );
    assert_eq!(outcome.stack_note, engine_note, "same stack note");
}

#[test]
fn test_govcreate_attaches_git_outcome_to_summary() {
    let parent = temp_parent("govcreate-git");
    let target = parent.path().join("with-git");

    let req = request(&["--language", "rust", "--type", "cmdline"]);
    let outcome = run_govcreate_scaffold(&req, &target).expect("scaffold succeeds");

    // The local git half is always attempted; a temp dir is a fresh repo.
    assert!(
        outcome.summary.git.is_some(),
        "git outcome recorded on the summary"
    );
    assert!(target.join(".git").is_dir(), "git repository initialised");
}

#[test]
fn test_govcreate_no_hosting_flag_has_no_remote() {
    let parent = temp_parent("govcreate-no-remote");
    let target = parent.path().join("no-remote");

    let req = request(&["--language", "rust", "--type", "cmdline"]);
    let outcome = run_govcreate_scaffold(&req, &target).expect("scaffold succeeds");

    assert_eq!(
        outcome.summary.remote,
        ragent_tools_extended::project_scaffold::RemoteStatus::None
    );
}

#[test]
fn test_govcreate_unknown_stack_warns_and_continues() {
    let parent = temp_parent("govcreate-unknown-stack");
    let target = parent.path().join("unknown-stack");

    let req = request(&[
        "--language",
        "rust",
        "--type",
        "cmdline",
        "--stack",
        "no-such-stack",
    ]);
    let outcome = run_govcreate_scaffold(&req, &target).expect("unknown stack continues");

    assert!(
        outcome.stack_note.contains("[warn]"),
        "unknown stack warns: {}",
        outcome.stack_note
    );
    assert!(
        target.join("Cargo.toml").is_file(),
        "base layout still emitted"
    );
}

#[test]
fn test_govcreate_summary_reports_the_target() {
    let parent = temp_parent("govcreate-summary-target");
    let target = parent.path().join("reported");

    let req = request(&["--language", "go", "--type", "cmdline"]);
    let outcome = run_govcreate_scaffold(&req, &target).expect("scaffold succeeds");

    let summary: &ScaffoldSummary = &outcome.summary;
    assert_eq!(summary.target, target);
    assert!(
        summary.render().contains(&target.display().to_string()),
        "rendered summary names the target"
    );
}
