//! Tests for the `/spec govcreate` argument parser (spec `govdoc` T-002, FR-002).
//!
//! Covers positional extraction, quoted paths, spec-ID validation with the
//! existing rules, `/new` flag delegation, `--force` handling, and the
//! usage-error surface.

use ragent_specs::SpecCommand;
use ragent_tools_extended::project_scaffold::{ScaffoldError, parse_flags};

/// Build the `/new` request the shared parser would produce for the given
/// flag tokens, for equality checks against the govcreate parse result.
fn expected_scaffold(flags: &[&str]) -> ragent_tools_extended::project_scaffold::ScaffoldRequest {
    parse_flags(flags).expect("test fixture flags must be valid")
}

#[test]
fn parse_govcreate_three_positionals_with_flags() {
    let cmd = SpecCommand::parse(
        "govcreate payments-arch https://example.com/arch ./tc --language rust --type cmdline",
    );
    let SpecCommand::GovCreate {
        spec_id,
        content_ref,
        target_folder,
        scaffold,
        force,
    } = cmd
    else {
        panic!("expected GovCreate, got {cmd:?}");
    };
    assert_eq!(spec_id, "payments-arch");
    assert_eq!(content_ref, "https://example.com/arch");
    assert_eq!(target_folder, "./tc");
    assert_eq!(
        scaffold,
        expected_scaffold(&["--language", "rust", "--type", "cmdline"])
    );
    assert!(!force);
}

#[test]
fn parse_govcreate_local_folder_reference() {
    let cmd = SpecCommand::parse(
        "govcreate datahub-arch ./fixtures/docs ./tc-lib --language python --type library",
    );
    let SpecCommand::GovCreate {
        spec_id,
        content_ref,
        target_folder,
        scaffold,
        force,
    } = cmd
    else {
        panic!("expected GovCreate, got {cmd:?}");
    };
    assert_eq!(spec_id, "datahub-arch");
    assert_eq!(content_ref, "./fixtures/docs");
    assert_eq!(target_folder, "./tc-lib");
    assert_eq!(
        scaffold,
        expected_scaffold(&["--language", "python", "--type", "library"])
    );
    assert!(!force);
}

#[test]
fn parse_govcreate_with_stack_and_hosting() {
    let cmd = SpecCommand::parse(
        "govcreate arch https://example.com/ ./tc --language rust --type cmdline --stack axum --github",
    );
    let SpecCommand::GovCreate {
        scaffold, force, ..
    } = cmd
    else {
        panic!("expected GovCreate, got {cmd:?}");
    };
    assert_eq!(
        scaffold,
        expected_scaffold(&[
            "--language",
            "rust",
            "--type",
            "cmdline",
            "--stack",
            "axum",
            "--github"
        ])
    );
    assert!(!force);
}

#[test]
fn parse_govcreate_force_defaults_false() {
    let cmd = SpecCommand::parse("govcreate arch ./docs ./tc --language go --type library");
    let SpecCommand::GovCreate { force, .. } = cmd else {
        panic!("expected GovCreate, got {cmd:?}");
    };
    assert!(!force);
}

#[test]
fn parse_govcreate_force_is_stripped_and_set() {
    let cmd = SpecCommand::parse("govcreate arch ./docs ./tc --language go --type library --force");
    let SpecCommand::GovCreate {
        scaffold, force, ..
    } = cmd
    else {
        panic!("expected GovCreate, got {cmd:?}");
    };
    assert!(force, "trailing --force must set the flag");
    // --force is a govcreate flag, not a /new flag: it must not reach the
    // scaffold parser (which would reject it as unknown).
    assert_eq!(
        scaffold,
        expected_scaffold(&["--language", "go", "--type", "library"])
    );
}

#[test]
fn parse_govcreate_force_in_any_position() {
    let cmd = SpecCommand::parse("govcreate arch ./docs ./tc --force --language go --type library");
    let SpecCommand::GovCreate {
        scaffold, force, ..
    } = cmd
    else {
        panic!("expected GovCreate, got {cmd:?}");
    };
    assert!(force);
    assert_eq!(
        scaffold,
        expected_scaffold(&["--language", "go", "--type", "library"])
    );
}

#[test]
fn parse_govcreate_quoted_content_ref_with_spaces() {
    let cmd = SpecCommand::parse(
        "govcreate arch \"https://example.com/a b\" \"./target folder\" --language rust --type cmdline",
    );
    let SpecCommand::GovCreate {
        content_ref,
        target_folder,
        ..
    } = cmd
    else {
        panic!("expected GovCreate, got {cmd:?}");
    };
    assert_eq!(content_ref, "https://example.com/a b");
    assert_eq!(target_folder, "./target folder");
}

#[test]
fn parse_govcreate_missing_third_positional_is_usage_error() {
    let cmd = SpecCommand::parse("govcreate arch ./docs --language rust --type cmdline");
    assert!(
        matches!(&cmd, SpecCommand::Unknown(s) if s == "govcreate"),
        "two positionals must be a usage error, got {cmd:?}"
    );
    assert!(cmd.is_usage_error());
}

#[test]
fn parse_govcreate_no_args_is_usage_error() {
    let cmd = SpecCommand::parse("govcreate");
    assert!(
        matches!(&cmd, SpecCommand::Unknown(s) if s == "govcreate"),
        "no positionals must be a usage error, got {cmd:?}"
    );
    assert!(cmd.is_usage_error());
}

#[test]
fn parse_govcreate_invalid_spec_id_rejects_path_traversal() {
    let cmd = SpecCommand::parse("govcreate ../escape ./docs ./tc --language rust --type cmdline");
    let SpecCommand::GovCreateUsage(reason) = cmd else {
        panic!("expected GovCreateUsage, got {cmd:?}");
    };
    assert!(
        reason.contains("invalid spec id"),
        "reason should name the invalid spec id: {reason}"
    );
}

#[test]
fn parse_govcreate_invalid_spec_id_with_slash() {
    let cmd = SpecCommand::parse("govcreate a/b ./docs ./tc --language rust --type cmdline");
    assert!(
        matches!(cmd, SpecCommand::GovCreateUsage(_)),
        "a slash in the spec ID must be rejected, got {cmd:?}"
    );
}

#[test]
fn parse_govcreate_invalid_spec_id_is_usage_error() {
    let cmd = SpecCommand::parse("govcreate ../escape ./docs ./tc --language rust --type cmdline");
    assert!(
        cmd.is_usage_error(),
        "an invalid spec ID must report the usage surface"
    );
}

#[test]
fn parse_govcreate_no_flags_yields_help_request() {
    // Mirrors `/new`: an invocation with no scaffold flags parses to a
    // help request, which the runner treats as the usage surface.
    let cmd = SpecCommand::parse("govcreate arch ./docs ./tc");
    let SpecCommand::GovCreate { ref scaffold, .. } = cmd else {
        panic!("expected GovCreate help request, got {cmd:?}");
    };
    assert!(scaffold.is_help(), "no flags must parse to a help request");
    assert!(!cmd.is_usage_error());
}

#[test]
fn parse_govcreate_force_with_no_flags_is_help_request() {
    // `--force` alone is stripped; with no `/new` flags the scaffold request is
    // a help request and `force` is still recorded.
    let cmd = SpecCommand::parse("govcreate arch ./docs ./tc --force");
    let SpecCommand::GovCreate {
        scaffold, force, ..
    } = cmd
    else {
        panic!("expected GovCreate help request, got {cmd:?}");
    };
    assert!(force);
    assert!(scaffold.is_help());
}

#[test]
fn parse_govcreate_unknown_language_is_usage() {
    let cmd = SpecCommand::parse("govcreate arch ./docs ./tc --language klingon --type cmdline");
    let SpecCommand::GovCreateUsage(reason) = cmd else {
        panic!("expected GovCreateUsage, got {cmd:?}");
    };
    assert!(
        reason.contains("unknown --language value 'klingon'"),
        "{reason}"
    );
}

#[test]
fn parse_govcreate_unknown_flag_is_usage() {
    let cmd =
        SpecCommand::parse("govcreate arch ./docs ./tc --language rust --type cmdline --bogus");
    let SpecCommand::GovCreateUsage(reason) = cmd else {
        panic!("expected GovCreateUsage, got {cmd:?}");
    };
    assert_eq!(
        reason,
        ScaffoldError::UnknownFlag("--bogus".to_string()).to_string()
    );
}

#[test]
fn parse_govcreate_hosting_conflict_is_usage() {
    let cmd = SpecCommand::parse(
        "govcreate arch ./docs ./tc --language rust --type cmdline --github --gitlab",
    );
    let SpecCommand::GovCreateUsage(reason) = cmd else {
        panic!("expected GovCreateUsage, got {cmd:?}");
    };
    assert_eq!(reason, ScaffoldError::HostingConflict.to_string());
}

#[test]
fn parse_govcreate_flag_error_message_matches_new_surface() {
    // FR-002: error text must not drift from `/new` — the shared parser is the
    // single source of truth, so the govcreate failure equals the /new failure.
    let direct = parse_flags(&["--language", "rust"])
        .unwrap_err()
        .to_string();
    let cmd = SpecCommand::parse("govcreate arch ./docs ./tc --language rust");
    let SpecCommand::GovCreateUsage(reason) = cmd else {
        panic!("expected GovCreateUsage, got {cmd:?}");
    };
    assert_eq!(reason, direct);
}

#[test]
fn parse_govcreate_is_not_a_usage_error_for_a_valid_invocation() {
    let cmd = SpecCommand::parse("govcreate arch ./docs ./tc --language rust --type cmdline");
    assert!(!cmd.is_usage_error());
}

#[test]
fn parse_govcreate_unterminated_quote_runs_to_end() {
    // An unterminated quote is tolerated: the remaining text forms one token.
    let cmd = SpecCommand::parse("govcreate arch ./docs \"./target");
    let SpecCommand::GovCreate { target_folder, .. } = cmd else {
        panic!("expected GovCreate, got {cmd:?}");
    };
    assert_eq!(target_folder, "./target");
}
