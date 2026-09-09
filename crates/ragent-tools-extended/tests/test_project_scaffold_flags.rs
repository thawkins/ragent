//! Tests for the `/new` scaffold flag layer (T-001, spec `newproj`).
//!
//! Covers FR-002 (empty-directory guard decision), FR-003 (required flag
//! validation), and FR-009 (mutual exclusion of hosting flags), plus the
//! pure-logic contracts the later tasks build on.

use ragent_tools_extended::project_scaffold::{
    AppType, FilePathEntry, HostingTarget, Language, ScaffoldError, app_type_value_list, is_help,
    language_value_list, parse_flags, validate_target_directory,
};

// ---------------------------------------------------------------- FR-002 ---
// Empty-directory guard decision (pure logic over a gathered entry list).

#[test]
fn test_flags_empty_directory_passes_guard() {
    let entries = vec![];
    assert!(validate_target_directory(&entries).is_ok());
}

#[test]
fn test_flags_allowlisted_artifacts_pass_guard() {
    let entries = vec![
        FilePathEntry {
            name: ".ragent".to_owned(),
            is_dir: true,
        },
        FilePathEntry {
            name: "log".to_owned(),
            is_dir: true,
        },
        FilePathEntry {
            name: "target".to_owned(),
            is_dir: true,
        },
    ];
    assert!(validate_target_directory(&entries).is_ok());
}

#[test]
fn test_flags_stray_file_fails_guard_with_entry_names() {
    let entries = vec![
        FilePathEntry {
            name: ".ragent".to_owned(),
            is_dir: true,
        },
        FilePathEntry {
            name: "keepme.txt".to_owned(),
            is_dir: false,
        },
    ];
    match validate_target_directory(&entries) {
        Err(ScaffoldError::DirectoryNotEmpty(offending)) => {
            assert_eq!(offending, vec!["keepme.txt".to_owned()]);
        }
        other => panic!("expected DirectoryNotEmpty, got {other:?}"),
    }
}

#[test]
fn test_flags_stray_directory_fails_guard_with_slash_suffix() {
    let entries = vec![FilePathEntry {
        name: "src".to_owned(),
        is_dir: true,
    }];
    match validate_target_directory(&entries) {
        Err(ScaffoldError::DirectoryNotEmpty(offending)) => {
            assert_eq!(offending, vec!["src/".to_owned()]);
        }
        other => panic!("expected DirectoryNotEmpty, got {other:?}"),
    }
}

#[test]
fn test_flags_guard_reports_all_offending_entries() {
    let entries = vec![
        FilePathEntry {
            name: "a.txt".to_owned(),
            is_dir: false,
        },
        FilePathEntry {
            name: "log".to_owned(),
            is_dir: true,
        },
        FilePathEntry {
            name: "sub".to_owned(),
            is_dir: true,
        },
        FilePathEntry {
            name: "b.bin".to_owned(),
            is_dir: false,
        },
    ];
    match validate_target_directory(&entries) {
        Err(ScaffoldError::DirectoryNotEmpty(offending)) => {
            assert_eq!(
                offending,
                vec!["a.txt".to_owned(), "sub/".to_owned(), "b.bin".to_owned()]
            );
        }
        other => panic!("expected DirectoryNotEmpty, got {other:?}"),
    }
}

// ---------------------------------------------------------------- FR-003 ---
// Required flag validation: missing/unknown values abort before any work.

#[test]
fn test_flags_minimal_valid_request_parses() {
    let request = parse_flags(&["--language", "rust", "--type", "cmdline"]).expect("valid request");
    assert_eq!(request.language(), Language::Rust);
    assert_eq!(request.app_type(), AppType::Cmdline);
    assert_eq!(request.stack(), None);
    assert_eq!(request.hosting(), None);
    assert!(!request.is_help());
}

#[test]
fn test_flags_missing_language_rejected() {
    match parse_flags(&["--type", "tui"]) {
        Err(ScaffoldError::MissingLanguage) => {}
        other => panic!("expected MissingLanguage, got {other:?}"),
    }
}

#[test]
fn test_flags_missing_type_rejected() {
    match parse_flags(&["--language", "python"]) {
        Err(ScaffoldError::MissingAppType) => {}
        other => panic!("expected MissingAppType, got {other:?}"),
    }
}

#[test]
fn test_flags_unknown_language_rejected() {
    match parse_flags(&["--language", "cobol", "--type", "cmdline"]) {
        Err(ScaffoldError::UnknownLanguage(value)) => assert_eq!(value, "cobol"),
        other => panic!("expected UnknownLanguage, got {other:?}"),
    }
}

#[test]
fn test_flags_unknown_type_rejected() {
    match parse_flags(&["--language", "rust", "--type", "webapp"]) {
        Err(ScaffoldError::UnknownAppType(value)) => assert_eq!(value, "webapp"),
        other => panic!("expected UnknownAppType, got {other:?}"),
    }
}

#[test]
fn test_flags_unknown_flag_rejected() {
    match parse_flags(&["--language", "rust", "--type", "cmdline", "--wat"]) {
        Err(ScaffoldError::UnknownFlag(flag)) => assert_eq!(flag, "--wat"),
        other => panic!("expected UnknownFlag, got {other:?}"),
    }
}

#[test]
fn test_flags_flag_without_value_rejected() {
    match parse_flags(&["--language", "rust", "--type"]) {
        Err(ScaffoldError::UnknownFlag(message)) => {
            assert!(message.contains("--type"), "message: {message}");
        }
        other => panic!("expected UnknownFlag, got {other:?}"),
    }
}

#[test]
fn test_flags_all_registry_languages_parse() {
    for (value, expected) in [
        ("rust", Language::Rust),
        ("python", Language::Python),
        ("go", Language::Go),
        ("typescript", Language::TypeScript),
        ("ts", Language::TypeScript),
    ] {
        let request =
            parse_flags(&["--language", value, "--type", "library"]).expect("valid language");
        assert_eq!(request.language(), expected, "language value: {value}");
    }
}

#[test]
fn test_flags_every_language_enum_value_parses() {
    // Every enum variant's canonical value must round-trip through
    // `parse_flags` -- the help surface and renderer both iterate
    // `Language::all()`, so an unparseable variant would break them.
    for language in Language::all() {
        let request = parse_flags(&["--language", language.as_str(), "--type", "library"])
            .unwrap_or_else(|err| panic!("{language}: {err:?}"));
        assert_eq!(request.language(), *language);
    }
}

#[test]
fn test_flags_dialect_aliases_parse_to_parent_language() {
    // Scanner-id dialect variants and common spellings map to their
    // parent scaffold language.
    for (value, expected) in [
        ("ts", Language::TypeScript),
        ("tsx", Language::TypeScript),
        ("js", Language::JavaScript),
        ("jsx", Language::JavaScript),
        ("c++", Language::Cpp),
        ("c_header", Language::C),
        ("cpp_header", Language::Cpp),
        ("sh", Language::Shell),
        ("bash", Language::Shell),
        ("yml", Language::Yaml),
        ("sv", Language::Verilog),
        ("vhd", Language::Vhdl),
        ("vhdl", Language::Vhdl),
        ("tf", Language::Terraform),
        ("scad", Language::OpenScad),
        ("kts", Language::GradleKts),
        ("gradle_kts", Language::GradleKts),
    ] {
        let request =
            parse_flags(&["--language", value, "--type", "library"]).expect("valid alias");
        assert_eq!(request.language(), expected, "alias value: {value}");
    }
}

#[test]
fn test_flags_language_values_are_case_insensitive() {
    let request = parse_flags(&["--language", "RUST", "--type", "library"]).expect("valid");
    assert_eq!(request.language(), Language::Rust);
    let request = parse_flags(&["--language", "Gradle_KTS", "--type", "library"]).expect("valid");
    assert_eq!(request.language(), Language::GradleKts);
}

#[test]
fn test_flags_all_app_types_parse() {
    for (value, expected) in [
        ("library", AppType::Library),
        ("cmdline", AppType::Cmdline),
        ("tui", AppType::Tui),
        ("gui", AppType::Gui),
    ] {
        let request = parse_flags(&["--language", "go", "--type", value]).expect("valid app type");
        assert_eq!(request.app_type(), expected, "app-type value: {value}");
    }
}

#[test]
fn test_flags_value_lists_cover_registry() {
    assert_eq!(
        language_value_list(),
        "rust, python, go, typescript, javascript, c, cpp, java, kotlin, ruby, \
         swift, csharp, lua, zig, nim, elixir, erlang, haskell, ocaml, r, dart, \
         php, perl, shell, zsh, fish, toml, yaml, json, xml, html, css, scss, \
         sql, markdown, protobuf, verilog, vhdl, terraform, openscad, cmake, \
         gradle, gradle_kts, maven, nix, hcl"
    );
    assert_eq!(app_type_value_list(), "library, cmdline, tui, gui");
}

// ---------------------------------------------------------------- FR-009 ---
// Mutual exclusion of hosting flags.

#[test]
fn test_flags_hosting_conflict_rejected() {
    match parse_flags(&[
        "--language",
        "rust",
        "--type",
        "cmdline",
        "--github",
        "--gitlab",
    ]) {
        Err(ScaffoldError::HostingConflict) => {}
        other => panic!("expected HostingConflict, got {other:?}"),
    }
}

#[test]
fn test_flags_hosting_conflict_detected_before_missing_required_check() {
    // The conflict is reported even when required flags are also absent:
    // validation short-circuits on the first failure in argument order, and
    // the conflict check runs before the FR-003 required-flag check.
    match parse_flags(&["--github", "--gitlab"]) {
        Err(ScaffoldError::HostingConflict) => {}
        other => panic!("expected HostingConflict, got {other:?}"),
    }
}

#[test]
fn test_flags_github_only_parses() {
    let request =
        parse_flags(&["--language", "rust", "--type", "cmdline", "--github"]).expect("valid");
    assert_eq!(request.hosting(), Some(HostingTarget::GitHub));
}

#[test]
fn test_flags_gitlab_only_parses() {
    let request =
        parse_flags(&["--language", "rust", "--type", "cmdline", "--gitlab"]).expect("valid");
    assert_eq!(request.hosting(), Some(HostingTarget::GitLab));
}

#[test]
fn test_flags_no_hosting_flag_means_none() {
    let request = parse_flags(&["--language", "rust", "--type", "cmdline"]).expect("valid");
    assert_eq!(request.hosting(), None);
}

// ------------------------------------------------------- help + stack ------- /

#[test]
fn test_flags_bare_invocation_is_help() {
    assert!(is_help(&[]));
    let request = parse_flags(&[]).expect("bare invocation is valid help");
    assert!(request.is_help());
}

#[test]
fn test_flags_help_word_is_help() {
    assert!(is_help(&["help"]));
    let request = parse_flags(&["help"]).expect("help invocation is valid");
    assert!(request.is_help());
}

#[test]
fn test_flags_help_word_with_flags_rejected() {
    match parse_flags(&["help", "--language", "rust"]) {
        Err(ScaffoldError::UnexpectedArgument(message)) => {
            assert!(message.contains("help"), "message: {message}");
        }
        other => panic!("expected UnexpectedArgument, got {other:?}"),
    }
}

#[test]
fn test_flags_stack_parses_for_known_and_unknown_values() {
    let known =
        parse_flags(&["--language", "rust", "--type", "cmdline", "--stack", "axum"]).expect("ok");
    assert_eq!(known.stack(), Some("axum"));
    let unknown = parse_flags(&[
        "--language",
        "rust",
        "--type",
        "cmdline",
        "--stack",
        "made_up_framework",
    ])
    .expect("ok");
    assert_eq!(unknown.stack(), Some("made_up_framework"));
}

#[test]
fn test_flags_error_display_mentions_registry_values() {
    let error = ScaffoldError::UnknownLanguage("cobol".to_owned());
    let rendered = error.to_string();
    assert!(rendered.contains("rust"), "rendered: {rendered}");
    assert!(rendered.contains("typescript"), "rendered: {rendered}");
    let conflict = ScaffoldError::HostingConflict.to_string();
    assert!(
        conflict.contains("mutually exclusive"),
        "rendered: {conflict}"
    );
}
