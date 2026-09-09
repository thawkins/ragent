//! Tests for the `/new help` detailed help renderer (spec `newproj` T-016,
//! FR-018, NFR-001).
//!
//! Covers the FR-018 content contract (purpose, per-argument docs with
//! optionality / accepted values / omitted-default behaviour, two worked
//! examples) and the NFR-001 registry-derived value lists (languages,
//! app types, known stacks from `STACK_RECIPES`).

use ragent_tools_extended::project_scaffold::{
    Language, STACK_RECIPES, app_type_value_list, language_value_list, render_detailed_help,
};

/// The same slash-surface arguments the TUI `new_usage_message` passes.
fn slash_help() -> String {
    render_detailed_help(
        "## /new - Scaffold a new project\n\
         \n\
         \x20 /new --language <lang> --type <type> [--stack <name>] [--github | --gitlab]\n\
         \x20 /new help",
        "\x20 /new --language rust --type cmdline",
        "\x20 /new --language python --type library --gitlab",
    )
}

#[test]
fn test_help_purpose_section_present() {
    let text = slash_help();
    assert!(text.contains("Purpose:"), "FR-018 purpose: {text}");
    assert!(text.contains("Scaffold a brand-new project"));
    // Purpose names the concrete deliverables of the command.
    assert!(text.contains("ragent workspace"));
    assert!(text.contains("initial commit"));
}

#[test]
fn test_help_documents_every_argument_with_optionality() {
    let text = slash_help();
    for argument in ["--language", "--type", "--stack", "--github", "--gitlab"] {
        assert!(
            text.contains(&format!("\x20 {argument}")),
            "argument {argument} documented: {text}"
        );
    }
    for required in [
        "Required. The computer language to scaffold.",
        "Required. The kind of application to generate.",
        "Optional. Layers a known framework starter",
        "Optional flag. Creates a private GitHub repository",
        "Same as --github but on GitLab",
    ] {
        assert!(text.contains(required), "optionality text: {required}");
    }
}

#[test]
fn test_help_shows_omitted_default_behaviour() {
    let text = slash_help();
    assert!(
        text.contains("Omitted: validation error; nothing is created."),
        "required-flag omitted default: {text}"
    );
    assert!(
        text.contains("Omitted: no remote is created and nothing is pushed"),
        "hosting omitted default: {text}"
    );
    assert!(
        text.contains("Omitted: no stack layer is applied."),
        "stack omitted default: {text}"
    );
}

#[test]
fn test_help_documents_mutual_exclusion() {
    let text = slash_help();
    assert!(
        text.contains("mutually exclusive"),
        "FR-009 in help: {text}"
    );
}

#[test]
fn test_help_includes_two_worked_examples() {
    let text = slash_help();
    assert!(text.contains("Worked examples:"), "FR-018 examples: {text}");
    // Minimal (no hosting) and hosting-flag examples, both present.
    assert!(text.contains("/new --language rust --type cmdline"));
    assert!(text.contains("/new --language python --type library --gitlab"));
    // Each example carries an explanatory note (the FR-011 default for
    // minimal runs and the hosting outcome for the hosted one). Wrapping
    // follows the renderer's hard wrap width (6-space note indent).
    assert!(text.contains("No\n      remote is created and nothing is pushed."));
    assert!(text.contains("private hosting repository"));
}

#[test]
fn test_help_value_lists_derived_from_registry() {
    let text = slash_help();
    // NFR-001: exactly the registry lists, not hardcoded prose.
    assert!(text.contains(&language_value_list()));
    assert!(text.contains(&app_type_value_list()));
    assert!(text.contains("dialect aliases"), "alias documented: {text}");
    // Scanner-id dialect aliases are spelled out on the page.
    for alias in [
        "tsx",
        "jsx",
        "c_header",
        "cpp_header",
        "bash",
        "yml",
        "sv",
        "vhd",
        "tf",
        "scad",
        "kts",
    ] {
        assert!(text.contains(alias), "alias {alias} documented");
    }
}

#[test]
fn test_help_stack_examples_derive_from_stack_recipes() {
    let text = slash_help();
    for recipe in STACK_RECIPES {
        assert!(
            text.contains(recipe.stack),
            "known stack '{}' listed in help: {text}",
            recipe.stack
        );
    }
    // Grouped under the language they apply to.
    assert!(
        text.contains("\n        rust: axum, warp, raylib, gtk4, ratatui"),
        "rust stack group: {text}"
    );
}

#[test]
fn test_help_registry_extension_renders_new_values() {
    // Simulate the NFR-001 drift guarantee: the renderer output must contain
    // every registry entry for languages it derives from.
    let text = slash_help();
    for language in Language::all() {
        assert!(
            text.contains(language.as_str()),
            "language {}",
            language.as_str()
        );
    }
}

#[test]
fn test_help_usage_lines_are_caller_supplied() {
    let cli = render_detailed_help(
        "\x20 ragent new --language <lang> --type <type>",
        "\x20 ragent new --language rust --type cmdline",
        "\x20 ragent new --language python --type library --gitlab",
    );
    assert!(
        cli.contains("ragent new --language <lang>"),
        "cli usage: {cli}"
    );
    assert!(
        !cli.contains("/new --language <lang>"),
        "no slash usage leaked"
    );
}
