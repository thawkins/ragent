//! Tests for the FR-019 project documentation scaffold (T-017, spec
//! `newproj`).
//!
//! Verifies the planned doc file set (`README.md`, `QUICKSTART.md`,
//! `STATS.md`, `docs/README.md`), that all build/run instructions derive
//! from the language recipe (NFR-002), that STATS carries the generation
//! timestamp, language, app type, stack, generated-file count, and producing
//! ragent version, and that the docs emit through the FR-016 no-silent-
//! overwrite emitter (creating the `docs/` folder and preserving existing
//! files).

use std::fs;

use ragent_tools_extended::project_scaffold::{
    AppType, DocFile, Language, doc_artifacts, emit_file, emit_files, plan_app_layout, recipe_for,
    workspace_artifacts,
};

/// All four registry languages, in registry order (FR-017 first release).
const ALL_LANGUAGES: [(Language, &str); 4] = [
    (Language::Rust, "rust"),
    (Language::Python, "python"),
    (Language::Go, "go"),
    (Language::TypeScript, "typescript"),
];

/// The four documentation files every scaffold must plan (FR-019).
const REQUIRED_DOCS: [&str; 4] = ["README.md", "QUICKSTART.md", "STATS.md", "docs/README.md"];

const STAMP: &str = "2026-09-10T12:00:00+00:00";
const RAGENT_VERSION: &str = "1.0.89";

/// Fetch a planned doc by path, failing the test when absent.
fn doc<'a>(docs: &'a [DocFile], path: &str) -> &'a DocFile {
    docs.iter()
        .find(|d| d.path == path)
        .unwrap_or_else(|| panic!("missing doc {path}"))
}

/// Plan docs for a language/app-type combo with fixed test inputs.
fn plan(language: Language, app_type: AppType, project: &str, stack: Option<&str>) -> Vec<DocFile> {
    let recipe = recipe_for(language).expect("recipe present");
    doc_artifacts(recipe, app_type, project, stack, 7, STAMP, RAGENT_VERSION)
}

// --------------------------------------------------------- file set FR-019 ---

#[test]
fn test_docs_contain_required_file_set() {
    for (language, name) in ALL_LANGUAGES {
        let planned = plan(language, AppType::Cmdline, "myproj", None);
        for required in REQUIRED_DOCS {
            assert!(
                planned.iter().any(|d| d.path == required),
                "{name}: docs missing {required}"
            );
        }
    }
}

#[test]
fn test_docs_count_is_exact() {
    // FR-019 plans exactly the four doc files per scaffold.
    for (language, name) in ALL_LANGUAGES {
        let planned = plan(language, AppType::Library, "myproj", None);
        assert_eq!(planned.len(), REQUIRED_DOCS.len(), "{name}: extra docs");
    }
}

#[test]
fn test_docs_paths_are_relative_and_normalised() {
    for (language, _) in ALL_LANGUAGES {
        for planned in plan(language, AppType::Tui, "myproj", None) {
            assert!(
                !planned.path.starts_with('/'),
                "absolute path {}",
                planned.path
            );
            assert!(!planned.path.contains(".."), "parent path {}", planned.path);
            assert!(
                !planned.path.contains('\\'),
                "windows path {}",
                planned.path
            );
        }
    }
}

// ------------------------------------------------------- NFR-002 consistency --

#[test]
fn test_readme_instructions_match_recipe() {
    // NFR-002: run/test commands and entrypoint come from the recipe.
    for (language, name) in ALL_LANGUAGES {
        let recipe = recipe_for(language).expect("recipe present");
        let planned = plan(language, AppType::Cmdline, "myproj", None);
        let readme = doc(&planned, "README.md");
        assert!(
            readme.content.contains(recipe.run_command),
            "{name}: README missing run command {}",
            recipe.run_command
        );
        assert!(
            readme.content.contains(recipe.test_command),
            "{name}: README missing test command {}",
            recipe.test_command
        );
        let entrypoint = recipe
            .source_for(AppType::Cmdline)
            .expect("cmdline source")
            .path;
        assert!(
            readme.content.contains(entrypoint),
            "{name}: README missing entrypoint {entrypoint}"
        );
    }
}

#[test]
fn test_readme_entrypoint_follows_app_type() {
    // The library entrypoint differs from the cmdline one per recipe.
    let recipe = recipe_for(Language::Rust).expect("recipe present");
    let cmdline_docs = plan(Language::Rust, AppType::Cmdline, "p", None);
    let library_docs = plan(Language::Rust, AppType::Library, "p", None);
    let cmdline = doc(&cmdline_docs, "README.md");
    let library = doc(&library_docs, "README.md");
    let cmdline_src = recipe.source_for(AppType::Cmdline).expect("cmdline").path;
    let library_src = recipe.source_for(AppType::Library).expect("library").path;
    assert!(cmdline.content.contains(cmdline_src));
    assert!(library.content.contains(library_src));
    assert_ne!(cmdline_src, library_src);
}

#[test]
fn test_readme_titles_the_project() {
    let readme_docs = plan(Language::Go, AppType::Cmdline, "widget-kit", None);
    let readme = doc(&readme_docs, "README.md");
    assert!(readme.content.starts_with("# widget-kit\n"));
}

#[test]
fn test_quickstart_instructions_match_recipe() {
    // NFR-002: QUICKSTART's run command is the recipe's run command.
    for (language, name) in ALL_LANGUAGES {
        let recipe = recipe_for(language).expect("recipe present");
        let planned = plan(language, AppType::Cmdline, "myproj", None);
        let quickstart = doc(&planned, "QUICKSTART.md");
        assert!(
            quickstart.content.contains(recipe.run_command),
            "{name}: QUICKSTART missing run command {}",
            recipe.run_command
        );
        assert!(
            quickstart.content.contains("Hello, world!"),
            "{name}: QUICKSTART missing expected first output"
        );
        assert!(
            quickstart.content.to_lowercase().contains("prerequisite"),
            "{name}: QUICKSTART missing prerequisites section"
        );
    }
}

#[test]
fn test_quickstart_run_block_matches_readme_run_command() {
    // The two docs must agree on the run command (single recipe source).
    let planned = plan(Language::TypeScript, AppType::Library, "p", None);
    let readme = doc(&planned, "README.md");
    let quickstart = doc(&planned, "QUICKSTART.md");
    let run = recipe_for(Language::TypeScript)
        .expect("recipe")
        .run_command;
    assert!(readme.content.contains(run));
    assert!(quickstart.content.contains(run));
}

// ------------------------------------------------------------ STATS content ---

#[test]
fn test_stats_carries_generation_metadata() {
    let planned = plan(Language::Rust, AppType::Cmdline, "myproj", None);
    let stats = doc(&planned, "STATS.md");
    assert!(stats.content.contains(STAMP), "missing UTC timestamp");
    assert!(stats.content.contains("rust"));
    assert!(stats.content.contains("cmdline"));
    assert!(
        stats.content.contains("none"),
        "stack must read none when omitted"
    );
    assert!(stats.content.contains("Generated files: 7"));
    assert!(stats.content.contains(RAGENT_VERSION));
}

#[test]
fn test_stats_records_stack_when_supplied() {
    let planned = plan(Language::Python, AppType::Cmdline, "myproj", Some("flask"));
    let stats = doc(&planned, "STATS.md");
    assert!(stats.content.contains("flask"));
    assert!(!stats.content.contains("- Stack: none"));
}

// -------------------------------------------------------- emission via T-007 ---

#[test]
fn test_docs_emit_creates_files_and_docs_folder() {
    let temp = tempfile::Builder::new()
        .prefix("newproj-docs-emit-")
        .tempdir()
        .expect("tempdir");
    let root = temp.path();
    let planned = plan(Language::Rust, AppType::Cmdline, "myproj", None);
    let report = emit_files(root, &planned).expect("emit docs");
    assert_eq!(report.created.len(), REQUIRED_DOCS.len());
    for required in REQUIRED_DOCS {
        assert!(root.join(required).is_file(), "{required} not written");
    }
    // FR-019: the docs/ folder exists after emission.
    assert!(root.join("docs").is_dir());
    let readme = fs::read_to_string(root.join("README.md")).expect("readme");
    assert!(readme.contains("cargo run"));
}

#[test]
fn test_docs_emit_never_overwrites_existing_files() {
    // FR-016 applies to the doc layer too: pre-existing README.md is left
    // untouched and reported as skipped.
    let temp = tempfile::Builder::new()
        .prefix("newproj-docs-skip-")
        .tempdir()
        .expect("tempdir");
    let root = temp.path();
    fs::write(root.join("README.md"), b"original").expect("seed README");
    let planned = plan(Language::Go, AppType::Library, "myproj", None);
    let report = emit_files(root, &planned).expect("emit docs");
    assert_eq!(report.skipped_existing, vec!["README.md".to_owned()]);
    let readme = fs::read_to_string(root.join("README.md")).expect("readme");
    assert_eq!(readme, "original");
}

#[test]
fn test_docs_are_emitted_after_layout_and_workspace() {
    // Ordering guarantee for the summary: docs extend the combined report of
    // the code + workspace layers, so a full scaffold plans 7 + 4 files.
    let recipe = recipe_for(Language::Rust).expect("recipe present");
    let layout = plan_app_layout(recipe, AppType::Cmdline, "myproj");
    let workspace = workspace_artifacts(recipe, "myproj");
    let docs = doc_artifacts(
        recipe,
        AppType::Cmdline,
        "myproj",
        None,
        layout.files.len() + workspace.len(),
        STAMP,
        RAGENT_VERSION,
    );
    assert_eq!(layout.files.len() + workspace.len() + docs.len(), 11);
}

#[test]
fn test_single_doc_file_emit_reports_status() {
    // emit_file classifies a single planned doc the same as any artifact.
    let temp = tempfile::Builder::new()
        .prefix("newproj-docs-single-")
        .tempdir()
        .expect("tempdir");
    let root = temp.path();
    let planned = plan(Language::Python, AppType::Cmdline, "myproj", None);
    let quickstart = doc(&planned, "QUICKSTART.md");
    let first = emit_file(root, quickstart.path, &quickstart.content).expect("first emit");
    assert_eq!(
        first,
        ragent_tools_extended::project_scaffold::FileStatus::Created
    );
    let second = emit_file(root, quickstart.path, &quickstart.content).expect("second emit");
    assert_eq!(
        second,
        ragent_tools_extended::project_scaffold::FileStatus::SkippedExisting
    );
}
