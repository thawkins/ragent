//! Tests for the FR-007 stack overlay system (T-004, spec `newproj`).
//!
//! Covers the known-stack registry lookup (language-scoped, case-insensitive),
//! the None / Applied / Unknown overlay resolution, and the in-place layout
//! mutation: dependency append to the manifest, import + starter snippet
//! layering on the source, unchanged file count and paths.

use ragent_tools_extended::project_scaffold::{
    AppLayout, AppType, Language, STACK_RECIPES, StackOverlay, apply_stack_overlay,
    plan_app_layout, recipe_for, resolve_stack_overlay,
};

const NAME: &str = "myproj";

/// Plan the base layout for a language + app type.
fn base(language: Language, app_type: AppType) -> AppLayout {
    let recipe = recipe_for(language).expect("recipe present");
    plan_app_layout(recipe, app_type, NAME)
}

// ------------------------------------------------- registry resolution ---

#[test]
fn test_stack_known_value_resolves_applied_for_own_language() {
    for stack in ["axum", "warp", "raylib", "gtk4", "ratatui"] {
        let overlay = resolve_stack_overlay(Some(stack), Language::Rust);
        match overlay {
            StackOverlay::Applied(recipe) => assert_eq!(recipe.stack, stack),
            other => panic!("'{stack}' must resolve Applied for rust, got {other:?}"),
        }
    }
}

#[test]
fn test_stack_matching_is_case_insensitive() {
    let overlay = resolve_stack_overlay(Some("AXUM"), Language::Rust);
    assert!(matches!(overlay, StackOverlay::Applied(_)));
    let overlay = resolve_stack_overlay(Some("Warp"), Language::Rust);
    assert!(matches!(overlay, StackOverlay::Applied(_)));
}

#[test]
fn test_stack_is_scoped_to_language() {
    // axum is Rust-only: other languages must resolve Unknown.
    for language in [Language::Python, Language::Go, Language::TypeScript] {
        let overlay = resolve_stack_overlay(Some("axum"), language);
        assert!(
            matches!(overlay, StackOverlay::Unknown(_)),
            "axum must not apply to {language}"
        );
    }
}

#[test]
fn test_stack_absent_resolves_none() {
    assert!(matches!(
        resolve_stack_overlay(None, Language::Rust),
        StackOverlay::None
    ));
}

#[test]
fn test_stack_unknown_value_resolves_unknown_with_raw_name() {
    let overlay = resolve_stack_overlay(Some("doesnotexist"), Language::Rust);
    match overlay {
        StackOverlay::Unknown(raw) => assert_eq!(raw, "doesnotexist"),
        other => panic!("unknown stack must resolve Unknown, got {other:?}"),
    }
}

#[test]
fn test_stack_registry_has_unique_names_per_language() {
    let mut seen: std::collections::HashSet<(String, &str)> = std::collections::HashSet::new();
    for recipe in STACK_RECIPES {
        assert!(
            seen.insert((format!("{:?}", recipe.language), recipe.stack)),
            "duplicate stack '{}' for {:?}",
            recipe.stack,
            recipe.language
        );
    }
}

#[test]
fn test_stack_registry_recipes_target_rust() {
    // First release ships only Rust framework stacks.
    assert!(STACK_RECIPES.iter().all(|r| r.language == Language::Rust));
}

// ------------------------------------------------- overlay application ---

#[test]
fn test_stack_overlay_appends_dependency_to_manifest() {
    let recipe = recipe_for(Language::Rust).expect("recipe present");
    let overlay = resolve_stack_overlay(Some("axum"), Language::Rust);
    let StackOverlay::Applied(stack) = overlay else {
        panic!("axum must resolve Applied");
    };
    let mut layout = base(Language::Rust, AppType::Cmdline);
    apply_stack_overlay(&mut layout, recipe, stack);

    let manifest = &layout.files[0];
    assert_eq!(manifest.path, "Cargo.toml");
    let expected_tail = format!(
        "{}[dependencies]\naxum = \"0.7\"\ntokio = {{ version = \"1\", \
         features = [\"macros\", \"rt-multi-thread\"] }}\n",
        recipe.render_manifest(NAME)
    );
    assert_eq!(manifest.content, expected_tail);
}

#[test]
fn test_stack_overlay_layers_import_and_starter_on_source() {
    let recipe = recipe_for(Language::Rust).expect("recipe present");
    let StackOverlay::Applied(stack) = resolve_stack_overlay(Some("axum"), Language::Rust) else {
        panic!("axum must resolve Applied");
    };
    let mut layout = base(Language::Rust, AppType::Cmdline);
    apply_stack_overlay(&mut layout, recipe, stack);

    let source = &layout.files[1];
    assert_eq!(source.path, "src/main.rs");
    // Import block first, then the base hello-world body, then the starter.
    assert!(
        source
            .content
            .starts_with("use axum::routing::get;\nuse axum::Router;\n")
    );
    assert!(source.content.contains("fn main()"));
    assert!(source.content.contains("Hello, world!"));
    assert!(source.content.contains("axum::serve"));
    // Snippet order: import block before the base body, starter after it.
    let import_end = source.content.find("fn main()").expect("base main present");
    assert!(
        source
            .content
            .find("hello_route")
            .expect("starter route present")
            > import_end,
        "starter snippet must come after the base body"
    );
}

#[test]
fn test_stack_overlay_warp_and_raylib_snippets() {
    let recipe = recipe_for(Language::Rust).expect("recipe present");

    let StackOverlay::Applied(warp) = resolve_stack_overlay(Some("warp"), Language::Rust) else {
        panic!("warp must resolve Applied");
    };
    let mut layout = base(Language::Rust, AppType::Cmdline);
    apply_stack_overlay(&mut layout, recipe, warp);
    assert!(layout.files[0].content.contains("warp = \"0.3\""));
    assert!(layout.files[1].content.contains("use warp::Filter;\n"));
    assert!(layout.files[1].content.contains("warp::serve"));
    assert!(layout.files[1].content.contains("Hello, world!"));

    let StackOverlay::Applied(raylib) = resolve_stack_overlay(Some("raylib"), Language::Rust)
    else {
        panic!("raylib must resolve Applied");
    };
    let mut layout = base(Language::Rust, AppType::Gui);
    apply_stack_overlay(&mut layout, recipe, raylib);
    assert!(layout.files[0].content.contains("raylib = \"5\""));
    assert!(
        layout.files[1]
            .content
            .contains("use raylib::prelude::*;\n")
    );
    assert!(layout.files[1].content.contains("draw_text"));
}

#[test]
fn test_stack_overlay_gtk4_snippet() {
    let recipe = recipe_for(Language::Rust).expect("recipe present");
    let StackOverlay::Applied(gtk4) = resolve_stack_overlay(Some("gtk4"), Language::Rust) else {
        panic!("gtk4 must resolve Applied");
    };
    let mut layout = base(Language::Rust, AppType::Gui);
    apply_stack_overlay(&mut layout, recipe, gtk4);
    assert!(layout.files[0].content.contains("gtk4 = \"0.9\""));
    assert!(layout.files[1].content.contains("use gtk4::prelude::*;\n"));
    assert!(layout.files[1].content.contains("Application::builder()"));
    assert!(layout.files[1].content.contains("app.run()"));
}

#[test]
fn test_stack_overlay_ratatui_snippet() {
    let recipe = recipe_for(Language::Rust).expect("recipe present");
    let StackOverlay::Applied(ratatui) = resolve_stack_overlay(Some("ratatui"), Language::Rust)
    else {
        panic!("ratatui must resolve Applied");
    };
    let mut layout = base(Language::Rust, AppType::Tui);
    apply_stack_overlay(&mut layout, recipe, ratatui);
    assert!(layout.files[0].content.contains("ratatui = \"0.29\""));
    assert!(layout.files[0].content.contains("crossterm = \"0.28\""));
    assert!(
        layout.files[1]
            .content
            .contains("use ratatui::{Terminal, backend::CrosstermBackend, widgets::Paragraph};\n")
    );
    assert!(layout.files[1].content.contains("Terminal::new("));
    assert!(layout.files[1].content.contains("Hello, world!"));
    assert!(layout.files[1].content.contains("event::read()"));
}

#[test]
fn test_stack_overlay_preserves_file_count_and_paths() {
    let recipe = recipe_for(Language::Rust).expect("recipe present");
    let StackOverlay::Applied(stack) = resolve_stack_overlay(Some("axum"), Language::Rust) else {
        panic!("axum must resolve Applied");
    };
    let plain = base(Language::Rust, AppType::Tui);
    let mut overlaid = base(Language::Rust, AppType::Tui);
    apply_stack_overlay(&mut overlaid, recipe, stack);

    assert_eq!(plain.files.len(), overlaid.files.len());
    for (before, after) in plain.files.iter().zip(overlaid.files.iter()) {
        assert_eq!(before.path, after.path, "overlay must not rename files");
    }
}

#[test]
fn test_stack_overlay_applies_to_library_layout() {
    let recipe = recipe_for(Language::Rust).expect("recipe present");
    let StackOverlay::Applied(stack) = resolve_stack_overlay(Some("axum"), Language::Rust) else {
        panic!("axum must resolve Applied");
    };
    let mut layout = base(Language::Rust, AppType::Library);
    apply_stack_overlay(&mut layout, recipe, stack);

    assert!(layout.files[0].content.contains("axum = \"0.7\""));
    assert_eq!(layout.files[1].path, "src/lib.rs");
    assert!(layout.files[1].content.contains("pub fn hello"));
    assert!(
        layout.files[1]
            .content
            .contains("use axum::routing::get;\n")
    );
}

#[test]
fn test_stack_unknown_leaves_base_layout_untouched() {
    // The FR-007 unknown-stack path: warn and continue with the base layout.
    let overlay = resolve_stack_overlay(Some("doesnotexist"), Language::Rust);
    assert!(matches!(overlay, StackOverlay::Unknown(_)));
    let plain = base(Language::Rust, AppType::Cmdline);
    // No apply_stack_overlay call — the base plan is used verbatim.
    assert!(!plain.files[0].content.contains("axum"));
    assert!(plain.files[1].content.contains("Hello, world!"));
}
