//! Integration tests for the additive `--type crate` scaffold (FR-020).
//!
//! `--type crate` is the one app type that adds to an existing project rather
//! than creating one, so these tests pin the three properties that make that
//! safe:
//!
//! 1. the crate layout is exactly `crates/<name>/Cargo.toml` plus
//!    `crates/<name>/src/lib.rs` - nothing is planned for the project root;
//! 2. a `members` glob that already covers `crates/*` leaves the root manifest
//!    byte-identical, and an explicit list gains only the new entry;
//! 3. an existing crate file is reported and left untouched (no silent
//!    overwrite), and a non-workspace target is refused before anything is
//!    written.
//!
//! Every test writes into its own `target/temp/<unique-name>` directory so a
//! run leaves the checked-in tree untouched.

use std::path::{Path, PathBuf};

use ragent_tools_extended::project_scaffold::{
    CrateWorkspaceError, MemberStatus, ScaffoldRequest, parse_flags, plan_crate_layout,
    plan_crate_workspace,
};

/// A scratch directory unique to this test, under the git-ignored `target/`
/// tree so a run never writes into the repository.
fn scratch(tag: &str) -> PathBuf {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("target")
        .join("temp")
        .join(format!("crate-workspace-{tag}"));
    let _ = std::fs::remove_dir_all(&root); // INTENTIONAL: per-test scratch reset
    std::fs::create_dir_all(&root).expect("create scratch root");
    root
}

/// Parse a `/new`-style flag list into a validated scaffold request.
fn request(flags: &[&str]) -> ScaffoldRequest {
    parse_flags(flags).expect("valid scaffold flags")
}

#[test]
fn test_crate_type_is_an_accepted_flag_value() {
    let parsed = request(&["--language", "rust", "--type", "crate"]);
    assert_eq!(parsed.app_type().as_str(), "crate");
}

#[test]
fn test_crate_layout_plans_a_manifest_and_a_library_only() {
    let layout = plan_crate_layout(
        ragent_tools_extended::project_scaffold::Language::Rust,
        "demo",
    )
    .expect("rust has a crate recipe");

    assert_eq!(layout.crate_dir, "crates/demo");
    let paths: Vec<&str> = layout.files.iter().map(|f| f.path.as_str()).collect();
    assert_eq!(
        paths,
        vec!["crates/demo/Cargo.toml", "crates/demo/src/lib.rs"],
        "the crate layout must not plan anything outside crates/<name>/"
    );

    let manifest = &layout.files[0].content;
    assert!(manifest.contains("name = \"demo\""));
    assert!(
        manifest.contains("edition.workspace = true"),
        "a crate must inherit the workspace edition, got:\n{manifest}"
    );
    assert!(
        manifest.contains("[lints]\nworkspace = true"),
        "a crate must inherit the workspace lints, got:\n{manifest}"
    );

    let lib = &layout.files[1].content;
    assert!(lib.contains("#![deny(unsafe_code)]"));
    assert!(lib.contains("Hello from demo!"));
}

#[test]
fn test_crate_layout_refuses_a_language_without_a_crate_recipe() {
    let err = plan_crate_layout(
        ragent_tools_extended::project_scaffold::Language::Python,
        "demo",
    )
    .expect_err("python has no crate recipe");

    assert!(
        matches!(&err, CrateWorkspaceError::UnsupportedLanguage(lang) if lang == "python"),
        "expected UnsupportedLanguage(python), got {err:?}"
    );
    assert!(err.to_string().contains("only supported for rust"));
}

#[test]
fn test_crate_scaffold_leaves_a_crates_glob_manifest_untouched() {
    let root = scratch("glob");
    let manifest_before = "[workspace]\nmembers = [\"crates/*\"]\nresolver = \"2\"\n";
    std::fs::write(root.join("Cargo.toml"), manifest_before).expect("write root manifest");

    let outcome = plan_crate_workspace(
        &root,
        &request(&["--language", "rust", "--type", "crate"]),
        "demo",
    )
    .expect("crate scaffold succeeds in a workspace");

    assert_eq!(
        outcome.member,
        MemberStatus::CoveredByGlob("crates/*".to_owned()),
        "a crates/* glob already covers the new crate"
    );
    assert_eq!(
        std::fs::read_to_string(root.join("Cargo.toml")).expect("read manifest"),
        manifest_before,
        "a covered crate must leave the root manifest byte-identical"
    );
    assert!(root.join("crates/demo/Cargo.toml").is_file());
    assert!(root.join("crates/demo/src/lib.rs").is_file());
    assert!(
        !root.join("AGENTS.md").exists() && !root.join(".gitignore").exists(),
        "an additive crate must not create root-level scaffold files"
    );
}

#[test]
fn test_crate_scaffold_registers_a_member_in_an_explicit_list() {
    let root = scratch("explicit");
    std::fs::write(
        root.join("Cargo.toml"),
        "[workspace]\nmembers = [\n    \"crates/alpha\",\n]\nresolver = \"2\"\n",
    )
    .expect("write root manifest");

    let outcome = plan_crate_workspace(
        &root,
        &request(&["--language", "rust", "--type", "crate"]),
        "beta",
    )
    .expect("crate scaffold succeeds");

    assert_eq!(
        outcome.member,
        MemberStatus::Registered("crates/beta".to_owned())
    );
    let updated = std::fs::read_to_string(root.join("Cargo.toml")).expect("read manifest");
    assert!(
        updated.contains("\"crates/alpha\""),
        "the existing member must survive, got:\n{updated}"
    );
    assert!(
        updated.contains("\"crates/beta\""),
        "the new member must be registered, got:\n{updated}"
    );
    assert!(
        updated.contains("resolver = \"2\""),
        "unrelated manifest keys must survive, got:\n{updated}"
    );
}

#[test]
fn test_crate_scaffold_does_not_overwrite_an_existing_crate_file() {
    let root = scratch("no-overwrite");
    std::fs::write(
        root.join("Cargo.toml"),
        "[workspace]\nmembers = [\"crates/*\"]\n",
    )
    .expect("write root manifest");
    std::fs::create_dir_all(root.join("crates/demo/src")).expect("create crate dir");
    std::fs::write(root.join("crates/demo/src/lib.rs"), "// hand-written\n")
        .expect("write existing lib");

    let outcome = plan_crate_workspace(
        &root,
        &request(&["--language", "rust", "--type", "crate"]),
        "demo",
    )
    .expect("crate scaffold succeeds");

    assert_eq!(
        std::fs::read_to_string(root.join("crates/demo/src/lib.rs")).expect("read lib"),
        "// hand-written\n",
        "an existing crate file must be left byte-identical"
    );
    assert!(
        outcome
            .report
            .skipped_existing
            .iter()
            .any(|path| path == "crates/demo/src/lib.rs"),
        "the skipped file must be reported, got {:?}",
        outcome.report
    );
    assert!(
        outcome
            .report
            .created
            .iter()
            .any(|path| path == "crates/demo/Cargo.toml"),
        "the missing manifest must still be created, got {:?}",
        outcome.report
    );
}

#[test]
fn test_crate_scaffold_refuses_a_project_that_is_not_a_workspace() {
    let root = scratch("no-workspace");

    let err = plan_crate_workspace(
        &root,
        &request(&["--language", "rust", "--type", "crate"]),
        "demo",
    )
    .expect_err("no Cargo.toml means no workspace");

    assert!(
        matches!(err, CrateWorkspaceError::NotAWorkspace(_)),
        "expected NotAWorkspace, got {err:?}"
    );
    assert!(
        !root.join("crates").exists(),
        "a refused crate must not leave a partial tree behind"
    );
}

#[test]
fn test_crate_scaffold_refuses_a_package_only_manifest() {
    let root = scratch("package-only");
    std::fs::write(root.join("Cargo.toml"), "[package]\nname = \"solo\"\n")
        .expect("write root manifest");

    let err = plan_crate_workspace(
        &root,
        &request(&["--language", "rust", "--type", "crate"]),
        "demo",
    )
    .expect_err("a package-only manifest is not a workspace root");

    assert!(
        matches!(err, CrateWorkspaceError::NotAWorkspaceRoot(_)),
        "expected NotAWorkspaceRoot, got {err:?}"
    );
    assert!(
        !root.join("crates").exists(),
        "a refused crate must not leave a partial tree behind"
    );
}
