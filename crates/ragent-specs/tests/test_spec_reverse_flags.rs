//! Integration tests for the `/spec reverse` argument parser (FR-002,
//! FR-011, FR-013).
//!
//! The parser lives in `ragent_specs::SpecCommand::parse` (fn `parse_reverse`);
//! it validates the repository positional, the `--create`/`--depth` flags, and
//! the `/new`-style scaffold flags (`--language` / `--type` / `--stack`) plus
//! the scaffold-only `--folder` target and `--github` / `--gitlab` hosting
//! flags, delegating every scaffold-flag validation to the shared `/new` parser
//! (`ragent_tools_extended::project_scaffold::parse_flags`, FR-002/FR-003).
//!
//! These tests pin that contract: the accepted scaffold value space, the
//! all-or-nothing `--language`/`--type` pairing, the `--tech` removal, the
//! scaffold-only `--folder`/hosting gates, and the usage-error forms for
//! missing/invalid input.

use ragent_specs::SpecCommand;

/// Parse a `/spec reverse …` argument tail (the string after `/spec`).
fn parse_reverse(args: &str) -> SpecCommand {
    SpecCommand::parse(&format!("reverse {args}"))
}

#[test]
fn test_reverse_minimal_parses_without_optional_flags() {
    let cmd = parse_reverse("owner/repo");
    let SpecCommand::Reverse {
        repo,
        create,
        depth,
        scaffold,
        folder,
    } = cmd
    else {
        panic!("a bare repo must parse as Reverse");
    };
    assert_eq!(repo, "owner/repo");
    assert!(create.is_none());
    assert!(depth.is_none());
    assert!(scaffold.is_none());
    assert!(folder.is_none());
}

/// A flag-validation failure is reported as a usage error carrying the cause.
///
/// The cause is either the dedicated `ReverseUsage` variant (a `/reverse`-level
/// refusal) or the `Unknown("reverse-usage: ...")` form used when the failure is
/// raised by the shared `/new` scaffold parser, because a `/spec reverse`
/// invocation whose first token is a flag never reaches `parse_reverse` and must
/// still be reported as a usage error rather than dropped.
fn usage_reason(args: &str) -> String {
    let cmd = parse_reverse(args);
    assert!(cmd.is_usage_error(), "expected usage error, got {cmd:?}");
    match cmd {
        SpecCommand::ReverseUsage(reason) => reason,
        SpecCommand::Unknown(sub) => sub
            .strip_prefix("reverse-usage: ")
            .map(str::to_string)
            .unwrap_or_else(|| panic!("expected a reverse usage error, got Unknown({sub:?})")),
        other => panic!("expected a usage error, got {other:?}"),
    }
}

#[test]
fn test_reverse_language_and_type_parse_into_scaffold() {
    let cmd = parse_reverse("owner/repo --language rust --type cmdline");
    let SpecCommand::Reverse { scaffold, .. } = cmd else {
        panic!("a valid scaffold invocation must parse as Reverse");
    };
    let scaffold = scaffold.expect("scaffold request");
    assert_eq!(scaffold.language().as_str(), "rust");
    assert_eq!(scaffold.app_type().as_str(), "cmdline");
    assert_eq!(scaffold.stack(), None);
}

#[test]
fn test_reverse_language_type_and_stack_parse_together() {
    let cmd = parse_reverse("owner/repo --language rust --type tui --stack ratatui");
    let SpecCommand::Reverse { scaffold, .. } = cmd else {
        panic!("a valid scaffold invocation must parse as Reverse");
    };
    let scaffold = scaffold.expect("scaffold request");
    assert_eq!(scaffold.language().as_str(), "rust");
    assert_eq!(scaffold.app_type().as_str(), "tui");
    assert_eq!(scaffold.stack(), Some("ratatui"));
}

#[test]
fn test_reverse_webapp_type_is_accepted() {
    // The help text advertises `--type webapp`; it must be accepted.
    let cmd = parse_reverse("owner/repo --language typescript --type webapp");
    let SpecCommand::Reverse { scaffold, .. } = cmd else {
        panic!("webapp must be an accepted --type value");
    };
    assert_eq!(
        scaffold.expect("scaffold request").app_type().as_str(),
        "webapp"
    );
}

#[test]
fn test_reverse_stack_alone_is_a_usage_error() {
    // A lone --stack has no --language/--type to pair with; the shared /new
    // parser rejects it and its error text is reported verbatim.
    let reason = usage_reason("owner/repo --stack axum");
    assert!(
        !reason.is_empty(),
        "the shared /new parser must supply the cause"
    );
}

#[test]
fn test_reverse_language_without_type_is_a_usage_error() {
    let reason = usage_reason("owner/repo --language rust");
    assert!(
        !reason.is_empty(),
        "the shared /new parser must supply the cause"
    );
}

#[test]
fn test_reverse_missing_repo_with_scaffold_flags_is_a_usage_error() {
    // Regression: `/spec reverse --language rust` (no <repo>) puts the first
    // flag token in the subcommand slot, so the dispatcher never runs
    // `parse_reverse`. It must still be recognised as a usage error rather
    // than treated as an unknown subcommand.
    let cmd = SpecCommand::parse("reverse --language rust");
    assert!(cmd.is_usage_error(), "expected usage error, got {cmd:?}");
}

#[test]
fn test_reverse_tech_flag_is_rejected() {
    let cmd = parse_reverse("owner/repo --tech rust");
    let SpecCommand::ReverseUsage(reason) = cmd else {
        panic!("--tech must no longer be accepted");
    };
    assert!(
        reason.contains("unknown option"),
        "unexpected reason: {reason}"
    );
}

#[test]
fn test_reverse_unknown_language_is_a_usage_error() {
    let reason = usage_reason("owner/repo --language bogus --type cmdline");
    assert!(
        reason.contains("bogus"),
        "the unknown value must be named: {reason}"
    );
}

#[test]
fn test_reverse_all_flags_parse_together() {
    let cmd = parse_reverse(
        "owner/repo --language rust --type cmdline --create my-spec --depth 3 --folder ./out",
    );
    let SpecCommand::Reverse {
        repo,
        create,
        depth,
        scaffold,
        folder,
    } = cmd
    else {
        panic!("all flags together must parse");
    };
    assert_eq!(repo, "owner/repo");
    assert_eq!(create.as_deref(), Some("my-spec"));
    assert_eq!(depth.as_deref(), Some("3"));
    assert_eq!(folder.as_deref(), Some("./out"));
    assert_eq!(
        scaffold.expect("scaffold request").language().as_str(),
        "rust"
    );
}

#[test]
fn test_reverse_folder_with_scaffold_parses_into_folder() {
    let cmd = parse_reverse("owner/repo --language rust --type cmdline --folder ./my-app");
    let SpecCommand::Reverse {
        folder, scaffold, ..
    } = cmd
    else {
        panic!("scaffold + --folder must parse");
    };
    assert_eq!(folder.as_deref(), Some("./my-app"));
    assert!(scaffold.is_some());
}

#[test]
fn test_reverse_github_with_scaffold_sets_hosting() {
    let cmd = parse_reverse("owner/repo --language rust --type cmdline --github");
    let SpecCommand::Reverse { scaffold, .. } = cmd else {
        panic!("scaffold + --github must parse");
    };
    assert_eq!(
        scaffold.expect("scaffold request").hosting(),
        Some(ragent_tools_extended::project_scaffold::HostingTarget::GitHub)
    );
}

#[test]
fn test_reverse_gitlab_with_scaffold_sets_hosting() {
    let cmd = parse_reverse("owner/repo --language rust --type cmdline --gitlab");
    let SpecCommand::Reverse { scaffold, .. } = cmd else {
        panic!("scaffold + --gitlab must parse");
    };
    assert_eq!(
        scaffold.expect("scaffold request").hosting(),
        Some(ragent_tools_extended::project_scaffold::HostingTarget::GitLab)
    );
}

#[test]
fn test_reverse_both_hosting_flags_is_a_usage_error() {
    // The mutual-exclusion rule lives in the shared /new parser, so the error
    // text must match /new's.
    let reason = usage_reason("owner/repo --language rust --type cmdline --github --gitlab");
    assert!(
        !reason.is_empty(),
        "the shared /new parser must supply the cause"
    );
}

#[test]
fn test_reverse_folder_without_scaffold_is_a_usage_error() {
    let reason = usage_reason("owner/repo --folder ./out");
    assert!(
        reason.contains("--folder requires --language and --type"),
        "unexpected reason: {reason}"
    );
}

#[test]
fn test_reverse_duplicate_folder_is_a_usage_error() {
    let cmd = parse_reverse("owner/repo --language rust --type cmdline --folder a --folder b");
    let SpecCommand::ReverseUsage(reason) = cmd else {
        panic!("duplicate --folder must be a usage error");
    };
    assert!(
        reason.contains("--folder given twice"),
        "unexpected reason: {reason}"
    );
}

#[test]
fn test_reverse_missing_repo_is_a_usage_error() {
    let cmd = SpecCommand::parse("reverse");
    assert!(matches!(&cmd, SpecCommand::Unknown(name) if name == "reverse"));
    assert!(cmd.is_usage_error());
}

#[test]
fn test_reverse_github_flag_without_scaffold_is_a_usage_error() {
    // --github/--gitlab only take effect with scaffold flags.
    let reason = usage_reason("owner/repo --github");
    assert!(
        reason.contains("--github requires --language and --type"),
        "unexpected reason: {reason}"
    );
}

#[test]
fn test_reverse_duplicate_create_is_a_usage_error() {
    let cmd = parse_reverse("owner/repo --create one --create two");
    let SpecCommand::ReverseUsage(reason) = cmd else {
        panic!("duplicate --create must be a usage error");
    };
    assert!(
        reason.contains("--create given twice"),
        "unexpected reason: {reason}"
    );
}

#[test]
fn test_reverse_quoted_flag_value_is_grouped() {
    // Shell-like tokenizer: quoted values containing spaces stay one token.
    let cmd = parse_reverse("owner/repo --create \"my spec\"");
    let SpecCommand::Reverse { create, .. } = cmd else {
        panic!("a quoted --create value must parse");
    };
    assert_eq!(create.as_deref(), Some("my spec"));
}

#[test]
fn test_reverse_https_url_with_scaffold_flags_parses() {
    // Regression for the reported silent no-op: a full GitHub URL plus
    // `--create`/`--depth`/`--language`/`--stack` (a `/new` flag that is NOT a
    // `--type`) must surface the specific cause, never vanish.
    let reason = usage_reason(
        "https://github.com/thawkins/gcodekit5 --create gcodekit5 --depth 3 \
         --language rust --stack gtk4 --folder /home/thawkins/Projects/gcodekittest",
    );
    assert!(
        !reason.is_empty(),
        "the missing --type must be reported, not silently ignored"
    );
}

#[test]
fn test_reverse_https_url_with_type_parses_cleanly() {
    // The same invocation with `--type gui` is valid and must parse without an
    // error.
    let cmd = parse_reverse(
        "https://github.com/thawkins/gcodekit5 --create gcodekit5 --depth 3 \
         --language rust --type gui --stack gtk4",
    );
    let SpecCommand::Reverse { repo, scaffold, .. } = cmd else {
        panic!("a valid URL invocation must parse as Reverse");
    };
    assert_eq!(repo, "https://github.com/thawkins/gcodekit5");
    let scaffold = scaffold.expect("scaffold request");
    assert_eq!(scaffold.language().as_str(), "rust");
    assert_eq!(scaffold.app_type().as_str(), "gui");
    assert_eq!(scaffold.stack(), Some("gtk4"));
}

#[test]
fn test_reverse_trailing_bare_word_after_repo_is_rejected() {
    // A bare word after the repo is not a flag value; the invocation must be
    // reported, never silently dropped or treated as the default --language.
    for invocation in ["owner/repo --language rust extra", "owner/repo extra"] {
        assert!(
            parse_reverse(invocation).is_usage_error(),
            "expected a usage error for {invocation:?}"
        );
    }
}

#[test]
fn test_reverse_quoted_repo_consumes_first_token() {
    let cmd = parse_reverse("\"owner/repo\" --language rust --type cmdline");
    let SpecCommand::Reverse { repo, scaffold, .. } = cmd else {
        panic!("a quoted repo must still parse");
    };
    assert_eq!(repo, "owner/repo");
    assert_eq!(
        scaffold.expect("scaffold request").language().as_str(),
        "rust"
    );
}
