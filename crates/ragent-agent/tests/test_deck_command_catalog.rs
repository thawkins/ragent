//! Guard: the `/research deck` surface agrees between the slash-command
//! catalogue and the parser (spec `diffdeck`: FR-001, FR-014, FR-015, FR-016,
//! FR-017).
//!
//! The catalogue in `ragent_agent::tool::command_catalog` drives completion and
//! `commands_info`; the parser in `ragent_research::cli` decides what a typed
//! line actually means. They are separate tables, so a flag added to one can
//! silently drift from the other: completion then offers a flag the parser
//! refuses, or the parser accepts a flag nothing advertises. These cases pin
//! both directions for the deck verb and its flags.
//!
//! The `/research help` text is the third surface, and it is asserted here too:
//! it must name the verb and every flag the catalogue advertises.

use std::collections::BTreeSet;

use ragent_agent::tool::command_catalog::COMMAND_CATALOG;
use ragent_research::cli::{ResearchCliCommand, deck_element_names};

/// The catalogue entry for `/research`.
fn research_entry() -> &'static ragent_agent::tool::command_catalog::CommandCatalogEntry {
    match COMMAND_CATALOG
        .iter()
        .find(|entry| entry.trigger == "research")
    {
        Some(entry) => entry,
        None => panic!("the catalogue must carry a `research` entry"),
    }
}

/// Parse a `/research` argument line, returning the refusal text on failure.
///
/// The parser reports a bad deck argument as the `Invalid` variant carrying the
/// user-facing message, which is what a surface prints.
fn parse_failure(args: &str) -> String {
    match ResearchCliCommand::parse(args) {
        ResearchCliCommand::Invalid(message) => message,
        other => panic!("expected `{args}` to be refused, got {other:?}"),
    }
}

#[test]
fn test_catalogue_advertises_the_deck_subcommand() {
    let entry = research_entry();
    assert!(
        entry.subcommands.contains(&"deck"),
        "the catalogue must offer the deck verb, got {:?}",
        entry.subcommands
    );
    // Completion derives the verb from the same list the parser accepts, so a
    // typed `deck` must resolve rather than falling through to the create path.
    match ResearchCliCommand::parse("deck myitem") {
        ResearchCliCommand::Deck { name, .. } => assert_eq!(name, "myitem"),
        other => panic!("`/research deck myitem` must parse as the deck verb, got {other:?}"),
    }
}

#[test]
fn test_catalogue_advertises_exactly_the_deck_flags_the_parser_accepts() {
    // Every flag the deck parser consumes, as the spec names it (FR-014..FR-017).
    let accepted: BTreeSet<&str> = [
        "--from",
        "--to",
        "--elements",
        "--include-old",
        "--out",
        "--force",
    ]
    .into_iter()
    .collect();
    let entry = research_entry();
    let advertised: BTreeSet<&str> = entry
        .flags
        .iter()
        .copied()
        .filter(|flag| accepted.contains(flag))
        .collect();
    assert_eq!(
        advertised, accepted,
        "the catalogue must advertise the deck flags, got {:?}",
        entry.flags
    );
}

#[test]
fn test_every_advertised_deck_flag_is_accepted_by_the_parser() {
    // A flag is only worth offering if typing it works. Each one is supplied with
    // its own valid value (switches carry none) and must parse as the deck verb.
    let lines = [
        "deck item --from 2026-01-01",
        "deck item --to 2026-01-02",
        "deck item --elements Findings",
        "deck item --include-old",
        "deck item --out target/temp/deck.md",
        "deck item --force",
    ];
    for line in lines {
        match ResearchCliCommand::parse(line) {
            ResearchCliCommand::Deck { .. } => {}
            other => panic!("advertised form `{line}` must parse as deck, got {other:?}"),
        }
    }
}

#[test]
fn test_research_help_names_the_deck_verb_and_flags() {
    let help = ResearchCliCommand::build_help_message();
    assert!(
        help.contains("deck <name>"),
        "the help text must name the deck verb: {help}"
    );
    for flag in [
        "--from",
        "--to",
        "--elements",
        "--include-old",
        "--out",
        "--force",
    ] {
        assert!(
            help.contains(flag),
            "the help text must document the deck flag {flag}"
        );
    }
}

#[test]
fn test_research_help_names_the_element_values_the_parser_accepts() {
    // `--elements` is the one deck flag whose values are a closed set, so help,
    // catalogue and parser must agree on it. `deck_element_names` is the single
    // source the refusal message uses.
    let help = ResearchCliCommand::build_help_message();
    for element in deck_element_names().split(", ") {
        assert!(
            help.contains(element),
            "the help text must name the accepted element {element}"
        );
        // The help must advertise a value the parser then accepts.
        let line = format!("deck item --elements \"{element}\"");
        match ResearchCliCommand::parse(&line) {
            ResearchCliCommand::Deck { elements, .. } => assert_eq!(
                elements,
                vec![
                    ragent_research::deck::DeckElement::from_name(element)
                        .expect("an advertised element must resolve")
                ],
                "`{element}` must parse to its own element"
            ),
            other => panic!("advertised element `{element}` must parse, got {other:?}"),
        }
    }
}

#[test]
fn test_deck_flag_refusals_name_the_offered_element_set() {
    // FR-014: an unknown element is refused naming the offending value and the
    // accepted values, so a user who mistypes sees the same set completion offers.
    let failure = parse_failure("deck item --elements Nonsense");
    assert!(
        failure.contains("Nonsense"),
        "the refusal must name the offending value: {failure}"
    );
    for element in deck_element_names().split(", ") {
        assert!(
            failure.contains(element),
            "the refusal must name the accepted value {element}: {failure}"
        );
    }
}
