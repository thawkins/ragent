//! Tests for the ragent workspace initialiser (T-005, spec `newproj`).
//!
//! Covers FR-004: the fixed ragent workspace layer (`.ragent/`, `specs/`,
//! `log/`, `.gitignore`, `AGENTS.md`) planned for every scaffold, with the
//! `.gitignore` derived from the per-language recipe (registry single source,
//! FR-017).

use ragent_tools_extended::project_scaffold::{
    Language, WorkspaceArtifact, recipe_for, workspace_artifacts, workspace_dirs,
};

/// All four registry languages, in registry order (FR-017 first release).
const ALL_LANGUAGES: [(Language, &str); 4] = [
    (Language::Rust, "rust"),
    (Language::Python, "python"),
    (Language::Go, "go"),
    (Language::TypeScript, "typescript"),
];

/// The five files every scaffold must plan (FR-004).
const REQUIRED_FILES: [&str; 5] = [
    ".gitignore",
    "AGENTS.md",
    ".ragent/config.json",
    ".ragent/agents/README.md",
    "specs/README.md",
];

/// Fetch a planned artifact by path, failing the test when absent.
fn artifact<'a>(artifacts: &'a [WorkspaceArtifact], path: &str) -> &'a WorkspaceArtifact {
    artifacts
        .iter()
        .find(|a| a.path == path)
        .unwrap_or_else(|| panic!("missing artifact {path}"))
}

// ---------------------------------------------------------------- FR-004 ---
// Required artifact set and directory layout.

#[test]
fn test_workspace_artifacts_contain_required_file_set() {
    for (language, name) in ALL_LANGUAGES {
        let recipe = recipe_for(language).expect("recipe present");
        let planned = workspace_artifacts(recipe, "myproj");
        for required in REQUIRED_FILES {
            assert!(
                planned.iter().any(|a| a.path == required),
                "{name}: workspace missing {required}"
            );
        }
    }
}

#[test]
fn test_workspace_artifact_count_is_exact() {
    // FR-004 plans exactly the five fixed workspace files per scaffold.
    for (language, name) in ALL_LANGUAGES {
        let recipe = recipe_for(language).expect("recipe present");
        let planned = workspace_artifacts(recipe, "myproj");
        assert_eq!(planned.len(), REQUIRED_FILES.len(), "{name}: extra files");
    }
}

#[test]
fn test_workspace_dirs_cover_required_directories() {
    let dirs = workspace_dirs();
    for required in [".ragent/agents", "specs", "log"] {
        assert!(dirs.contains(&required), "missing workspace dir {required}");
    }
}

#[test]
fn test_workspace_paths_are_relative_and_normalised() {
    for (language, _) in ALL_LANGUAGES {
        let recipe = recipe_for(language).expect("recipe present");
        for planned in workspace_artifacts(recipe, "myproj") {
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
        for dir in workspace_dirs() {
            assert!(!dir.starts_with('/'), "absolute dir {dir}");
            assert!(!dir.contains(".."), "parent dir {dir}");
        }
    }
}

#[test]
fn test_workspace_no_duplicate_paths() {
    for (language, _) in ALL_LANGUAGES {
        let recipe = recipe_for(language).expect("recipe present");
        let mut seen = std::collections::HashSet::new();
        for planned in workspace_artifacts(recipe, "myproj") {
            assert!(seen.insert(planned.path), "duplicate path {}", planned.path);
        }
    }
}

// -------------------------------------------------------------- gitignore ---

#[test]
fn test_workspace_gitignore_matches_recipe_render() {
    // Single source of truth: the language recipe renders the gitignore.
    for (language, name) in ALL_LANGUAGES {
        let recipe = recipe_for(language).expect("recipe present");
        let planned = workspace_artifacts(recipe, "myproj");
        let gitignore = artifact(&planned, ".gitignore");
        assert_eq!(gitignore.content, recipe.render_gitignore(), "{name}");
    }
}

#[test]
fn test_workspace_gitignore_covers_ragent_artifacts() {
    // FR-004: `.ragent/`, `log/`, `target/` are gitignored for every language.
    for (language, name) in ALL_LANGUAGES {
        let recipe = recipe_for(language).expect("recipe present");
        let planned = workspace_artifacts(recipe, "myproj");
        let gitignore = artifact(&planned, ".gitignore");
        for required in [".ragent/", "log/", "target/"] {
            assert!(
                gitignore.content.contains(required),
                "{name}: gitignore missing {required}"
            );
        }
    }
}

// -------------------------------------------------------------- AGENTS.md ---

#[test]
fn test_workspace_agents_md_names_the_project() {
    let recipe = recipe_for(Language::Rust).expect("recipe present");
    let planned = workspace_artifacts(recipe, "myproj");
    let agents_md = artifact(&planned, "AGENTS.md");
    assert!(agents_md.content.contains("myproj"));
    assert!(agents_md.content.contains("Language: rust"));
}

#[test]
fn test_workspace_agents_md_lists_recipe_commands() {
    let expected = [
        (Language::Rust, "cargo run", "cargo test"),
        (
            Language::Python,
            "python3 main.py",
            "python3 -m unittest discover",
        ),
        (Language::Go, "go run .", "go test ./..."),
        (Language::TypeScript, "npx tsx src/main.ts", "npm test"),
    ];
    for (language, run, test) in expected {
        let recipe = recipe_for(language).expect("recipe present");
        let planned = workspace_artifacts(recipe, "myproj");
        let agents_md = artifact(&planned, "AGENTS.md");
        assert!(
            agents_md.content.contains(&format!("Run: `{run}`")),
            "{}: run command missing",
            recipe.language
        );
        assert!(
            agents_md.content.contains(&format!("Test: `{test}`")),
            "{}: test command missing",
            recipe.language
        );
    }
}

// ------------------------------------------------- .ragent/ placeholders ----

#[test]
fn test_workspace_config_placeholder_is_valid_json() {
    let recipe = recipe_for(Language::Rust).expect("recipe present");
    let planned = workspace_artifacts(recipe, "myproj");
    let config = artifact(&planned, ".ragent/config.json");
    let parsed: serde_json::Value = serde_json::from_str(&config.content).expect("valid JSON");
    assert_eq!(parsed["project"], "myproj");
}

#[test]
fn test_workspace_agents_dir_placeholder_mentions_agents() {
    let recipe = recipe_for(Language::Python).expect("recipe present");
    let planned = workspace_artifacts(recipe, "myproj");
    let readme = artifact(&planned, ".ragent/agents/README.md");
    assert!(readme.content.contains("agent"));
    assert!(!readme.content.is_empty());
}

#[test]
fn test_workspace_specs_dir_placeholder_mentions_specs() {
    let recipe = recipe_for(Language::Go).expect("recipe present");
    let planned = workspace_artifacts(recipe, "myproj");
    let readme = artifact(&planned, "specs/README.md");
    assert!(readme.content.contains("/spec"));
    assert!(!readme.content.is_empty());
}
