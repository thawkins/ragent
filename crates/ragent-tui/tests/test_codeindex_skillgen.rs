//! Tests for `/codeindex skillgen` subcommand and the `skillgen` module.
//!
//! Verifies the graphify skill is written with the correct file structure
//! (SKILL.md + 8 reference files), and that existing files are overwritten
//! on re-run. Tests the inner `generate_graphify_skill_in` function directly
//! to avoid manipulating HOME (which would require `unsafe`).

#[path = "support/mod.rs"]
mod support;

use std::fs;

use ragent_tui::App;

/// Build an [`App`] backed by an in-memory database (mirrors test_slash_commands.rs).
/// A test [`App`] built on the shared crate-wide fixture.
fn make_app() -> App {
    support::make_app()
}

/// Expected reference file names.
const EXPECTED_REFS: &[&str] = &[
    "add-watch.md",
    "exports.md",
    "extraction-spec.md",
    "github-and-merge.md",
    "hooks.md",
    "query.md",
    "transcribe.md",
    "update.md",
];

#[test]
fn test_skillgen_writes_skill_files() {
    let temp = tempfile::tempdir().expect("tempdir");
    let skills_dir = temp.path().join(".ragent").join("skills");

    let result =
        ragent_tui::app::skillgen::generate_graphify_skill_in(&skills_dir).expect("generate skill");

    assert_eq!(result.files_written, 9, "1 SKILL.md + 8 references");
    assert_eq!(
        result.dest,
        skills_dir.join("graphify"),
        "dest should be <skills_dir>/graphify"
    );
    assert!(
        !result.already_existed,
        "skill dir should not have existed before"
    );

    // SKILL.md must exist and have the graphify frontmatter.
    let skill_md = result.dest.join("SKILL.md");
    assert!(skill_md.is_file(), "SKILL.md should exist");
    let content = fs::read_to_string(&skill_md).expect("read SKILL.md");
    assert!(
        content.starts_with("---\nname: graphify"),
        "SKILL.md should start with graphify frontmatter, got: {}",
        &content[..50]
    );

    // All 8 reference files must exist and be non-empty.
    let refs_dir = result.dest.join("references");
    assert!(refs_dir.is_dir(), "references/ dir should exist");
    for name in EXPECTED_REFS {
        let path = refs_dir.join(name);
        assert!(path.is_file(), "reference file {name} should exist");
        let content =
            fs::read_to_string(&path).unwrap_or_else(|e| panic!("read reference {name}: {e}"));
        assert!(!content.is_empty(), "reference {name} should not be empty");
    }
}

#[test]
fn test_skillgen_overwrites_existing() {
    let temp = tempfile::tempdir().expect("tempdir");
    let skills_dir = temp.path().join(".ragent").join("skills");

    // Run once.
    let result1 =
        ragent_tui::app::skillgen::generate_graphify_skill_in(&skills_dir).expect("first run");
    assert!(!result1.already_existed);

    let skill_md = result1.dest.join("SKILL.md");
    let first_content = fs::read_to_string(&skill_md).expect("read first SKILL.md");

    // Run again - should report already_existed.
    let result2 =
        ragent_tui::app::skillgen::generate_graphify_skill_in(&skills_dir).expect("second run");
    assert!(
        result2.already_existed,
        "second run should report already_existed"
    );

    let second_content = fs::read_to_string(&skill_md).expect("read second SKILL.md");

    // Content should be identical (idempotent overwrite).
    assert_eq!(
        first_content, second_content,
        "second run should overwrite with same content"
    );
    assert_eq!(
        result2.files_written, 9,
        "second run should also write 9 files"
    );
}

#[tokio::test]
async fn test_codeindex_skillgen_help_lists_subcommand() {
    let mut app = make_app();
    app.session_id = Some("test-session".to_string());

    app.execute_slash_command("/codeindex help").await;

    let text = app.messages.last().expect("help message").text_content();
    assert!(
        text.contains("skillgen"),
        "help should mention skillgen, got: {text}"
    );
}

#[tokio::test]
async fn test_codeindex_skillgen_unknown_fallback_lists_subcommand() {
    let mut app = make_app();
    app.session_id = Some("test-session".to_string());

    // An unknown subcommand should show the usage string which now lists skillgen.
    app.execute_slash_command("/codeindex unknownxyz").await;

    let text = app.messages.last().expect("usage message").text_content();
    assert!(
        text.contains("skillgen"),
        "usage fallback should mention skillgen, got: {text}"
    );
}
