//! Behavioural backward-compatibility verification for the `/codeindex` command
//! surface (spec graphCI, T-028; audit T-811).
//!
//! Earlier revisions of this suite scraped the text of `src/app/slash.rs` and
//! `src/app/state.rs` and asserted on substring matches.  That made the tests
//! fail on pure reformatting (renamed match arms, moved help text) while giving
//! no real protection.  These tests instead exercise the app's own command
//! data — [`SLASH_COMMANDS`] — which is the single source of truth the TUI uses
//! to advertise and dispatch `/codeindex`.

use ragent_tui::app::SLASH_COMMANDS;

/// The registered `/codeindex` command definition.
fn codeindex_def() -> Option<&'static ragent_tui::app::SlashCommandDef> {
    SLASH_COMMANDS.iter().find(|d| d.trigger == "codeindex")
}

#[test]
fn codeindex_command_is_registered() {
    assert!(
        codeindex_def().is_some(),
        "the `/codeindex` command must remain registered in SLASH_COMMANDS"
    );
}

#[test]
fn codeindex_description_lists_existing_subcommands() {
    let def = codeindex_def().expect("codeindex command registered");
    for sub in ["on", "off", "show", "lang", "reindex", "rebuild", "help"] {
        assert!(
            def.description.contains(sub),
            "codeindex description must still advertise the existing `{sub}` sub-command; got: {}",
            def.description
        );
    }
}

#[test]
fn codeindex_description_lists_graph_subcommands() {
    let def = codeindex_def().expect("codeindex command registered");
    for sub in ["graph", "explain", "path", "communities", "godnodes"] {
        assert!(
            def.description.contains(sub),
            "codeindex description must advertise the additive graph sub-command `{sub}`; got: {}",
            def.description
        );
    }
}

#[test]
fn graph_features_are_not_top_level_commands() {
    // FR-022: the graph features are reachable only through `/codeindex
    // <subcommand>`; there must be no top-level `/graph`, `/explain`, `/path`,
    // `/communities`, or `/godnodes` command.
    for cmd in ["graph", "explain", "path", "communities", "godnodes"] {
        assert!(
            SLASH_COMMANDS.iter().all(|d| d.trigger != cmd),
            "unexpected top-level slash command `/{}` - graph features must stay under `/codeindex`",
            cmd
        );
    }
}
