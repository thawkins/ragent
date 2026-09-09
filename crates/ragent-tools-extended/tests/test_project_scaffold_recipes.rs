//! Tests for the per-language scaffold recipes (T-002, spec `newproj`).
//!
//! Covers FR-005 (hello-world artifact set buildable with the default
//! toolchain) and FR-017 (language registry covering rust/python/go/
//! typescript), plus the `{name}` substitution contracts used by the
//! renderer milestone (T-003+).

use ragent_codeindex::scanner::SUPPORTED_LANGUAGES;
use ragent_tools_extended::project_scaffold::{AppType, Language, SourceFile, recipe_for};

/// The original four first-release languages, in registry order.
const FIRST_RELEASE: [(Language, &str); 4] = [
    (Language::Rust, "rust"),
    (Language::Python, "python"),
    (Language::Go, "go"),
    (Language::TypeScript, "typescript"),
];

/// Full application languages with all four app-type sources.
const APPLICATION_LANGUAGES: [(Language, &str); 26] = [
    (Language::Rust, "rust"),
    (Language::Python, "python"),
    (Language::Go, "go"),
    (Language::TypeScript, "typescript"),
    (Language::JavaScript, "javascript"),
    (Language::C, "c"),
    (Language::Cpp, "cpp"),
    (Language::Java, "java"),
    (Language::Kotlin, "kotlin"),
    (Language::Ruby, "ruby"),
    (Language::Swift, "swift"),
    (Language::CSharp, "csharp"),
    (Language::Lua, "lua"),
    (Language::Zig, "zig"),
    (Language::Nim, "nim"),
    (Language::Elixir, "elixir"),
    (Language::Erlang, "erlang"),
    (Language::Haskell, "haskell"),
    (Language::Ocaml, "ocaml"),
    (Language::R, "r"),
    (Language::Dart, "dart"),
    (Language::Php, "php"),
    (Language::Perl, "perl"),
    (Language::Shell, "shell"),
    (Language::Zsh, "zsh"),
    (Language::Fish, "fish"),
];

/// Data/DSL/format stubs: manifest plus cmdline/library samples only.
const STUB_LANGUAGES: [(Language, &str); 20] = [
    (Language::Toml, "toml"),
    (Language::Yaml, "yaml"),
    (Language::Json, "json"),
    (Language::Xml, "xml"),
    (Language::Html, "html"),
    (Language::Css, "css"),
    (Language::Scss, "scss"),
    (Language::Sql, "sql"),
    (Language::Markdown, "markdown"),
    (Language::Protobuf, "protobuf"),
    (Language::Verilog, "verilog"),
    (Language::Vhdl, "vhdl"),
    (Language::Terraform, "terraform"),
    (Language::OpenScad, "openscad"),
    (Language::Cmake, "cmake"),
    (Language::Gradle, "gradle"),
    (Language::GradleKts, "gradle_kts"),
    (Language::Maven, "maven"),
    (Language::Nix, "nix"),
    (Language::Hcl, "hcl"),
];

/// Keep the historic alias pointing at the original first-release set so
/// the per-language spot checks below read naturally.
const ALL_LANGUAGES: [(Language, &str); 4] = FIRST_RELEASE;

// ---------------------------------------------------------------- FR-017 ---
// Registry completeness and lookup behaviour.

#[test]
fn test_recipes_registry_covers_every_language() {
    for language in Language::all() {
        let recipe =
            recipe_for(*language).unwrap_or_else(|| panic!("{language} missing from registry"));
        assert_eq!(recipe.language, *language);
    }
}

#[test]
fn test_recipes_registry_has_no_duplicate_languages() {
    let mut seen = std::collections::HashSet::new();
    for recipe in ragent_tools_extended::project_scaffold::REGISTRY {
        assert!(
            seen.insert(recipe.language),
            "duplicate: {}",
            recipe.language
        );
    }
    assert_eq!(seen.len(), Language::all().len());
}

#[test]
fn test_recipes_registry_matches_codeindex_language_list() {
    // Every codeindex scanner id must resolve to a scaffold language:
    // either a canonical value or a dialect alias. This is the FR-017
    // parity guarantee between `/new` and the codeindex scanner.
    let canonical: std::collections::HashSet<&str> =
        Language::all().iter().map(|l| l.as_str()).collect();
    let aliases: std::collections::HashSet<&str> = [
        "ts",
        "tsx",
        "js",
        "jsx",
        "c++",
        "c_header",
        "cpp_header",
        "sh",
        "bash",
        "yml",
        "sv",
        "vhd",
        "vhdl",
        "tf",
        "scad",
        "kts",
    ]
    .into_iter()
    .collect();
    for id in SUPPORTED_LANGUAGES {
        assert!(
            canonical.contains(id) || aliases.contains(id),
            "codeindex id {id:?} has no /new scaffold language mapping"
        );
    }
    assert_eq!(Language::all().len(), 46);
}

#[test]
fn test_recipes_lookup_by_all_enum_variants() {
    // Every `Language` variant must resolve — the help surface and renderer
    // both iterate the enum, so a missing recipe would break them.
    for language in Language::all() {
        assert!(recipe_for(*language).is_some(), "no recipe for {language}");
    }
}

// ---------------------------------------------------------------- FR-005 ---
// Hello-world artifact set per language + app type.

#[test]
fn test_recipes_every_language_has_all_app_types() {
    for (language, name) in APPLICATION_LANGUAGES {
        let recipe = recipe_for(language).expect("recipe present");
        for app_type in AppType::all() {
            let source = recipe
                .source_for(*app_type)
                .unwrap_or_else(|| panic!("{name} recipe missing source for {app_type:?}"));
            assert!(!source.path.is_empty(), "{name}/{app_type:?}: empty path");
            assert!(
                source.content.contains("Hello, world!"),
                "{name}/{app_type:?}: source is not a hello-world"
            );
        }
    }
}

#[test]
fn test_recipes_stubs_have_cmdline_and_library_samples() {
    // Data/DSL/format stubs plan a manifest plus cmdline/library sample
    // documents; tui/gui degrade to manifest-only (layout layer).
    for (language, name) in STUB_LANGUAGES {
        let recipe = recipe_for(language).expect("recipe present");
        for app_type in [AppType::Cmdline, AppType::Library] {
            let source = recipe
                .source_for(app_type)
                .unwrap_or_else(|| panic!("{name} recipe missing source for {app_type:?}"));
            assert!(!source.path.is_empty(), "{name}/{app_type:?}: empty path");
        }
        assert!(
            recipe.source_for(AppType::Tui).is_none(),
            "{name}: stub recipes must not define tui"
        );
        assert!(
            recipe.source_for(AppType::Gui).is_none(),
            "{name}: stub recipes must not define gui"
        );
    }
}

#[test]
fn test_recipes_stub_cmdline_samples_say_hello() {
    // Stub sample documents carry a Hello, world! greeting so the
    // QUICKSTART "expected first output" contract stays coherent.
    for (language, name) in STUB_LANGUAGES {
        let recipe = recipe_for(language).expect("recipe present");
        let cmdline = recipe
            .source_for(AppType::Cmdline)
            .unwrap_or_else(|| panic!("{name}: missing cmdline sample"));
        assert!(
            cmdline.content.contains("Hello, world!"),
            "{name}: cmdline sample is not a hello-world"
        );
    }
}

#[test]
fn test_recipes_registry_partitions_into_app_and_stub_sets() {
    // The application and stub tables together cover every variant
    // exactly once -- adding a language to the enum without extending
    // one of the test tables fails here.
    assert_eq!(
        APPLICATION_LANGUAGES.len() + STUB_LANGUAGES.len(),
        Language::all().len(),
        "test tables must partition the language enum"
    );
    let mut seen = std::collections::HashSet::new();
    for (language, _) in APPLICATION_LANGUAGES.iter().chain(STUB_LANGUAGES.iter()) {
        assert!(
            seen.insert(*language),
            "duplicate test-table entry: {language}"
        );
    }
    for language in Language::all() {
        assert!(
            seen.contains(language),
            "missing from test tables: {language}"
        );
    }
}

#[test]
fn test_recipes_rust_cmdline_uses_main_rs() {
    let recipe = recipe_for(Language::Rust).expect("rust recipe");
    let source = recipe.source_for(AppType::Cmdline).expect("cmdline source");
    assert_eq!(source.path, "src/main.rs");
    assert!(source.content.contains("fn main()"));
    assert!(source.content.contains("println!"));
}

#[test]
fn test_recipes_rust_library_has_no_binary_entrypoint() {
    let recipe = recipe_for(Language::Rust).expect("rust recipe");
    let source = recipe.source_for(AppType::Library).expect("library source");
    assert_eq!(source.path, "src/lib.rs");
    assert!(
        !source.content.contains("fn main()"),
        "library must not define main"
    );
    assert!(source.content.contains("pub fn hello"));
}

#[test]
fn test_recipes_python_cmdline_uses_main_py() {
    let recipe = recipe_for(Language::Python).expect("python recipe");
    let source = recipe.source_for(AppType::Cmdline).expect("cmdline source");
    assert_eq!(source.path, "main.py");
    assert!(source.content.contains("print(\"Hello, world!\")"));
    assert!(source.content.contains("if __name__ == \"__main__\":"));
}

#[test]
fn test_recipes_go_cmdline_prints_via_fmt() {
    let recipe = recipe_for(Language::Go).expect("go recipe");
    let source = recipe.source_for(AppType::Cmdline).expect("cmdline source");
    assert_eq!(source.path, "main.go");
    assert!(source.content.contains("package main"));
    assert!(source.content.contains("fmt.Println"));
}

#[test]
fn test_recipes_typescript_cmdline_uses_main_ts() {
    let recipe = recipe_for(Language::TypeScript).expect("typescript recipe");
    let source = recipe.source_for(AppType::Cmdline).expect("cmdline source");
    assert_eq!(source.path, "src/main.ts");
    assert!(source.content.contains("console.log"));
}

// ------------------------------------------------------ manifests + render --

#[test]
fn test_recipes_manifest_paths_are_canonical() {
    let expected = [
        (Language::Rust, "Cargo.toml"),
        (Language::Python, "pyproject.toml"),
        (Language::Go, "go.mod"),
        (Language::TypeScript, "package.json"),
    ];
    for (language, path) in expected {
        let recipe = recipe_for(language).expect("recipe present");
        assert_eq!(recipe.manifest_path, path);
    }
}

/// The `{name}` placeholder, spelled indirectly so clippy's
/// `suspicious_format_impls`-style heuristic does not flag `.contains()` args
/// as format-macro arguments.
const NAME_PLACEHOLDER: &str = concat!("{", "name", "}");

#[test]
fn test_recipes_render_manifest_substitutes_project_name() {
    for (language, name) in ALL_LANGUAGES {
        let recipe = recipe_for(language).expect("recipe present");
        let manifest = recipe.render_manifest("myproj");
        assert!(
            !manifest.contains(NAME_PLACEHOLDER),
            "{name}: placeholder left"
        );
        assert!(manifest.contains("myproj"), "{name}: project name missing");
    }
}

#[test]
fn test_recipes_render_source_path_substitutes_project_name() {
    let recipe = recipe_for(Language::Python).expect("python recipe");
    let library = recipe.source_for(AppType::Library).expect("library source");
    assert_eq!(
        recipe.render_source_path(library, "myproj"),
        "src/myproj/__init__.py"
    );
    let go = recipe_for(Language::Go).expect("go recipe");
    let go_lib = go.source_for(AppType::Library).expect("library source");
    assert_eq!(go.render_source_path(go_lib, "myproj"), "myproj.go");
}

#[test]
fn test_recipes_render_source_content_substitutes_project_name() {
    let python = recipe_for(Language::Python).expect("python recipe");
    let library = python.source_for(AppType::Library).expect("library source");
    let rendered = python.render_source_content(library, "myproj");
    assert!(!rendered.contains("{name}"), "placeholder left: {rendered}");
    assert!(rendered.contains("myproj library package"));
}

#[test]
fn test_recipes_paths_without_placeholder_unchanged() {
    let recipe = recipe_for(Language::Rust).expect("rust recipe");
    let source = recipe.source_for(AppType::Cmdline).expect("cmdline source");
    assert_eq!(recipe.render_source_path(source, "whatever"), "src/main.rs");
    let content = recipe.render_source_content(source, "whatever");
    assert_eq!(content, source.content);
}

// ----------------------------------------------------- run/test/gitignore ---

#[test]
fn test_recipes_run_and_test_commands_present() {
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
        assert_eq!(recipe.run_command, run);
        assert_eq!(recipe.test_command, test);
    }
}

#[test]
fn test_recipes_gitignore_covers_language_and_ragent_artifacts() {
    for language in Language::all() {
        let recipe = recipe_for(*language).expect("recipe present");
        let gitignore = recipe.render_gitignore();
        for required in [".ragent/", "log/", "target/"] {
            assert!(
                gitignore.contains(required),
                "{language}: gitignore missing {required}"
            );
        }
        assert!(
            !gitignore.contains(NAME_PLACEHOLDER),
            "{language}: placeholder leaked into gitignore"
        );
    }
}

#[test]
fn test_recipes_gitignore_language_specific_entries() {
    let rust = recipe_for(Language::Rust)
        .expect("rust recipe")
        .render_gitignore();
    assert!(rust.contains("/target"));
    let python = recipe_for(Language::Python)
        .expect("python recipe")
        .render_gitignore();
    assert!(python.contains("__pycache__/"));
    let go = recipe_for(Language::Go)
        .expect("go recipe")
        .render_gitignore();
    assert!(go.contains("vendor/"));
    let ts = recipe_for(Language::TypeScript)
        .expect("typescript recipe")
        .render_gitignore();
    assert!(ts.contains("node_modules/"));
}

// ------------------------------------------------------- public surface -----

#[test]
fn test_recipes_source_file_fields_are_public() {
    // Guards the public surface: SourceFile fields are directly readable,
    // which later milestones rely on for snapshot comparisons.
    let file = SourceFile {
        path: "a.txt",
        content: "hi\n",
    };
    assert_eq!(file.path, "a.txt");
    assert_eq!(file.content, "hi\n");
}
