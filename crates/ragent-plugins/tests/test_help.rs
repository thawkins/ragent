//! Tests for the `/plugins help` usage text and command-surface metadata
//! (spec `plugins` T-014; FR-006, FR-014).

use ragent_plugins::{PLUGIN_SUBCOMMANDS, render_help, subcommand_of};

/// The usage block documents all seven subcommands, their arguments, and the
/// accepted source forms (FR-014).
#[test]
fn usage_block_documents_every_subcommand_and_source_form() {
    let help = render_help("help");

    // Attribution prefix matches the `/spec` family convention.
    assert!(help.starts_with("From: /plugins help"), "{help}");

    // Every subcommand and its argument form is present.
    for needle in [
        "`/plugins list [--verbose]`",
        "`/plugins add <source> [--force]`",
        "`/plugins remove <pluginid>`",
        "`/plugins enable <pluginid>`",
        "`/plugins disable <pluginid>`",
        "`/plugins test <pluginid>`",
        "`/plugins help`",
    ] {
        assert!(help.contains(needle), "missing {needle} in:\n{help}");
    }

    // The accepted source forms (FR-010) are documented.
    for needle in ["local directory", ".zip", ".tar.gz", "https://"] {
        assert!(
            help.contains(needle),
            "missing source form {needle}:\n{help}"
        );
    }

    // `--force` overwrite semantics are documented.
    assert!(help.contains("--force"), "{help}");
}

/// The usage block is ASCII only (project convention).
#[test]
fn usage_block_is_ascii_only() {
    assert!(render_help("").is_ascii());
    assert!(render_help("list").is_ascii());
}

/// A bare `/plugins` attributes to `From: /plugins` with no subcommand token.
#[test]
fn bare_entry_has_no_subcommand_attribution() {
    let bare = render_help("");
    assert!(bare.starts_with("From: /plugins\n"), "{bare}");
    assert!(!bare.starts_with("From: /plugins "), "{bare}");
}

/// The subcommand list is exactly the seven documented tokens (FR-006).
#[test]
fn subcommand_list_matches_the_documented_family() {
    assert_eq!(
        PLUGIN_SUBCOMMANDS,
        ["list", "add", "remove", "enable", "disable", "test", "help"]
    );
    // Every listed token appears in the usage block.
    let help = render_help("");
    for sub in PLUGIN_SUBCOMMANDS {
        assert!(
            help.contains(&format!("`/plugins {sub}")),
            "`{sub}` missing from the usage block:\n{help}"
        );
    }
}

/// `subcommand_of` extracts the first whitespace-delimited token.
#[test]
fn subcommand_of_extracts_the_first_token() {
    assert_eq!(subcommand_of(""), "");
    assert_eq!(subcommand_of("list"), "list");
    assert_eq!(subcommand_of("add ./p --force"), "add");
    assert_eq!(subcommand_of("  list  --verbose "), "list");
}

/// An unknown subcommand still renders the body (FR-014) but is attributed to
/// that subcommand so the user can see what was rejected.
#[test]
fn unknown_subcommand_renders_body_with_attribution() {
    let help = render_help("bogus");
    assert!(help.starts_with("From: /plugins bogus"), "{help}");
    assert!(help.contains("command reference"), "{help}");
}
