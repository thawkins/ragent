//! Tests for the FR-006 app-type layout mapping (T-003, spec `newproj`).
//!
//! Verifies the composed layout plan per (language, app type): manifest
//! first + hello-world source, library layouts without a binary entrypoint,
//! public-function exports, and `{name}` substitution across paths and
//! contents.

use ragent_tools_extended::project_scaffold::{
    AppLayout, AppType, Language, plan_app_layout, recipe_for,
};

/// All registry languages (FR-017).
const ALL_LANGUAGES: [(Language, &str); 4] = [
    (Language::Rust, "rust"),
    (Language::Python, "python"),
    (Language::Go, "go"),
    (Language::TypeScript, "typescript"),
];

/// All app types (FR-006).
const ALL_APP_TYPES: [AppType; 4] = [
    AppType::Library,
    AppType::Cmdline,
    AppType::Tui,
    AppType::Gui,
];

/// Project slug used in every render.
const NAME: &str = "myproj";

/// Binary-entrypoint basenames that a library layout must never contain.
const ENTRY_BASENAMES: [&str; 4] = ["main.rs", "main.py", "main.go", "main.ts"];

/// Binary-entrypoint source path per language (cmdline/tui/gui share it).
const ENTRYPOINTS: [(Language, &str); 4] = [
    (Language::Rust, "src/main.rs"),
    (Language::Python, "main.py"),
    (Language::Go, "main.go"),
    (Language::TypeScript, "src/main.ts"),
];

/// Library source path per language, rendered for [`NAME`].
const LIB_SOURCES: [(Language, &str); 4] = [
    (Language::Rust, "src/lib.rs"),
    (Language::Python, "src/myproj/__init__.py"),
    (Language::Go, "myproj.go"),
    (Language::TypeScript, "src/index.ts"),
];

/// Public-function marker per language (FR-006 library export requirement).
const PUBLIC_FNS: [(Language, &str); 4] = [
    (Language::Rust, "pub fn hello"),
    (Language::Python, "def hello"),
    (Language::Go, "func Hello"),
    (Language::TypeScript, "export function hello"),
];

/// Plan a layout for a language + app type.
fn plan(language: Language, app_type: AppType) -> AppLayout {
    let recipe = recipe_for(language).expect("recipe present");
    plan_app_layout(recipe, app_type, NAME)
}

/// Basename of a forward-slash path.
fn basename(path: &str) -> &str {
    path.rsplit('/').next().unwrap_or(path)
}

// ------------------------------------------------- manifest + source shape ---

#[test]
fn test_layout_pairs_plan_manifest_first_plus_source() {
    for (language, name) in ALL_LANGUAGES {
        let recipe = recipe_for(language).expect("recipe present");
        for app_type in ALL_APP_TYPES {
            let layout = plan(language, app_type);
            assert_eq!(layout.files.len(), 2, "{name}/{app_type}: shape");
            assert_eq!(layout.files[0].path, recipe.manifest_path);
            assert_eq!(layout.files[0].content, recipe.render_manifest(NAME));
            assert_eq!(layout.app_type, app_type);
        }
    }
}

#[test]
fn test_layout_no_duplicate_paths_or_absolute_components() {
    for (language, name) in ALL_LANGUAGES {
        for app_type in ALL_APP_TYPES {
            let layout = plan(language, app_type);
            let mut seen = std::collections::HashSet::new();
            for file in &layout.files {
                assert!(seen.insert(file.path.as_str()), "{name}: duplicate path");
                assert!(!file.path.starts_with('/'), "{name}: absolute path");
                assert!(!file.path.contains(".."), "{name}: parent path");
                assert!(!file.path.contains('\\'), "{name}: windows path");
            }
        }
    }
}

// ------------------------------------------------------ FR-006: library -----

#[test]
fn test_layout_library_has_no_binary_entrypoint() {
    for (language, name) in ALL_LANGUAGES {
        let layout = plan(language, AppType::Library);
        assert!(layout.is_library(), "{name}: is_library");
        for file in &layout.files {
            assert!(
                !ENTRY_BASENAMES.contains(&basename(&file.path)),
                "{name}: library layout contains entrypoint {}",
                file.path
            );
        }
    }
}

#[test]
fn test_layout_library_uses_expected_source_paths() {
    for (language, expected) in LIB_SOURCES {
        let layout = plan(language, AppType::Library);
        assert_eq!(layout.files[1].path, expected);
    }
}

#[test]
fn test_layout_library_exports_public_function() {
    for (language, marker) in PUBLIC_FNS {
        let layout = plan(language, AppType::Library);
        let source = &layout.files[1];
        assert!(
            source.content.contains(marker),
            "{}: missing public function marker '{marker}'",
            layout.files[1].path
        );
    }
}

// ------------------------------------------- FR-006: cmdline / tui / gui ----

#[test]
fn test_layout_binary_types_use_language_entrypoint() {
    for (language, entrypoint) in ENTRYPOINTS {
        for app_type in [AppType::Cmdline, AppType::Tui, AppType::Gui] {
            let layout = plan(language, app_type);
            assert!(!layout.is_library());
            assert_eq!(layout.files[1].path, entrypoint, "{app_type}");
        }
    }
}

#[test]
fn test_layout_cmdline_prints_hello_world() {
    for (language, name) in ALL_LANGUAGES {
        let layout = plan(language, AppType::Cmdline);
        let source = &layout.files[1];
        assert!(source.content.contains("Hello, world!"), "{name}");
        assert!(!source.content.contains("starter"), "{name}");
    }
}

#[test]
fn test_layout_tui_and_gui_generate_starters() {
    for (language, name) in ALL_LANGUAGES {
        let tui = plan(language, AppType::Tui);
        assert!(
            tui.files[1].content.contains("(tui starter)"),
            "{name}: tui starter"
        );
        let gui = plan(language, AppType::Gui);
        assert!(
            gui.files[1].content.contains("(gui starter)"),
            "{name}: gui starter"
        );
    }
}

// -------------------------------------------------- `{name}` substitution ----

#[test]
fn test_layout_substitutes_name_in_paths_and_content() {
    // Python library: package dir carries the project slug.
    let python = plan(Language::Python, AppType::Library);
    assert_eq!(python.files[1].path, "src/myproj/__init__.py");
    assert!(
        python.files[1].content.contains("myproj library package"),
        "python lib docstring must carry the slug"
    );
    // Go library: file name carries the slug.
    let go = plan(Language::Go, AppType::Library);
    assert_eq!(go.files[1].path, "myproj.go");
    // Manifests carry the slug.
    for (language, name) in ALL_LANGUAGES {
        let layout = plan(language, AppType::Cmdline);
        assert!(
            layout.files[0].content.contains(NAME),
            "{name}: manifest missing slug"
        );
    }
}

#[test]
fn test_layout_name_with_special_chars_is_inert() {
    // The slug is plain text substitution; regex-special chars must survive.
    let recipe = recipe_for(Language::Rust).expect("recipe present");
    let layout = plan_app_layout(recipe, AppType::Cmdline, "my-proj_2");
    assert_eq!(layout.files[1].path, "src/main.rs");
    assert!(layout.files[0].content.contains("\"my-proj_2\""));
}
