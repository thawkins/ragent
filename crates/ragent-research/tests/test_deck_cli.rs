//! Tests for the `deck` command parser (spec `diffdeck` T-009: FR-001, FR-014,
//! FR-015, FR-016, FR-017, FR-020).
//!
//! Every case drives `ResearchCliCommand::parse` over a command string, so the
//! grammar the TUI slash command and the CLI share is exercised as one surface.
//! Nothing is written to disk and no research item is needed.

use ragent_research::cli::{ResearchCliCommand, deck_element_names};
use ragent_research::deck::DeckElement;

/// Parse `deck <args>` and return the `Deck` variant, failing on any other.
///
/// `args` is the raw command tail (after the `deck` verb); it may contain
/// double-quoted values, which [`ResearchCliCommand::parse`] honours.
fn parse_deck(args: &str) -> ResearchCliCommand {
    let cmd = ResearchCliCommand::parse(&format!("deck {args}"));
    assert!(
        matches!(cmd, ResearchCliCommand::Deck { .. }),
        "expected a Deck command, got: {cmd:?}"
    );
    cmd
}

/// The refusal message a `deck <args>` invocation produced.
fn refusal(args: &str) -> String {
    match ResearchCliCommand::parse(&format!("deck {args}")) {
        ResearchCliCommand::Invalid(message) => message,
        other => panic!("expected a refusal, got: {other:?}"),
    }
}

/// `deck` with a `--elements` value whose entries contain spaces.
///
/// The element names such as `Executive Summary` carry a space, so a test that
/// supplies one quotes the whole list. The parser strips the quotes and then
/// splits on commas, which is exactly the form a user types.
fn parse_deck_elements_list(list: &str) -> Vec<DeckElement> {
    match parse_deck(&format!("ai-survey --elements \"{list}\"")) {
        ResearchCliCommand::Deck { elements, .. } => elements,
        other => panic!("unreachable: {other:?}"),
    }
}

#[test]
fn test_deck_parse_bare_name_takes_every_default() {
    match parse_deck("ai-survey") {
        ResearchCliCommand::Deck {
            name,
            from,
            to,
            elements,
            include_old,
            out,
            force,
        } => {
            assert_eq!(name, "ai-survey");
            assert_eq!(from, None);
            assert_eq!(to, None);
            assert!(elements.is_empty(), "no --elements filter by default");
            assert!(!include_old);
            assert_eq!(out, None);
            assert!(!force);
        }
        other => panic!("unreachable: {other:?}"),
    }
}

#[test]
fn test_deck_parse_from_and_to_stamps() {
    match parse_deck("ai-survey --from 2025-01-05 --to 2025-02-14") {
        ResearchCliCommand::Deck { from, to, .. } => {
            assert_eq!(from.as_deref(), Some("2025-01-05"));
            assert_eq!(to.as_deref(), Some("2025-02-14"));
        }
        other => panic!("unreachable: {other:?}"),
    }
}

#[test]
fn test_deck_parse_from_alone_leaves_to_unset() {
    match parse_deck("ai-survey --from 2025-01-05") {
        ResearchCliCommand::Deck { from, to, .. } => {
            assert_eq!(from.as_deref(), Some("2025-01-05"));
            assert_eq!(to, None);
        }
        other => panic!("unreachable: {other:?}"),
    }
}

#[test]
fn test_deck_parse_to_alone_leaves_from_unset() {
    match parse_deck("ai-survey --to 2025-02-14") {
        ResearchCliCommand::Deck { from, to, .. } => {
            assert_eq!(from, None);
            assert_eq!(to.as_deref(), Some("2025-02-14"));
        }
        other => panic!("unreachable: {other:?}"),
    }
}

#[test]
fn test_deck_parse_impossible_date_is_refused() {
    // 2025-04-31 is well-formed but names no real day, so it is refused at
    // parse time rather than surfacing as an "unknown report date" later.
    let message = refusal("ai-survey --from 2025-04-31");
    assert!(message.contains("--from 2025-04-31"), "got: {message}");
    assert!(message.contains("YYYY-MM-DD"), "got: {message}");
}

#[test]
fn test_deck_parse_non_date_stamp_is_refused() {
    for value in ["yesterday", "2025-1-5", "05/01/2025", "2025-13-01"] {
        let message = refusal(&format!("ai-survey --to {value}"));
        assert!(message.contains("--to"), "value {value:?} -> {message}");
        assert!(
            message.contains("YYYY-MM-DD"),
            "value {value:?} -> {message}"
        );
    }
}

#[test]
fn test_deck_parse_value_flags_without_a_value_are_refused() {
    for flag in ["--from", "--to", "--elements", "--out"] {
        let message = refusal(&format!("ai-survey {flag}"));
        assert!(
            message.contains(&format!("{flag} requires a value")),
            "got: {message}"
        );
    }
}

#[test]
fn test_deck_parse_element_names_resolve_by_label() {
    match parse_deck("ai-survey --elements Findings,References") {
        ResearchCliCommand::Deck { elements, .. } => {
            assert_eq!(
                elements,
                vec![DeckElement::Findings, DeckElement::References]
            );
        }
        other => panic!("unreachable: {other:?}"),
    }
}

#[test]
fn test_deck_parse_element_names_are_case_insensitive() {
    match parse_deck("ai-survey --elements findings") {
        ResearchCliCommand::Deck { elements, .. } => {
            assert_eq!(elements, vec![DeckElement::Findings]);
        }
        other => panic!("unreachable: {other:?}"),
    }
}

#[test]
fn test_deck_parse_element_names_accept_report_headings() {
    // The report heading spellings resolve to the same five elements, so a user
    // copying a heading out of a report is not refused.
    assert_eq!(
        parse_deck_elements_list("Concepts,In-Project Cross-References"),
        vec![DeckElement::Concepts, DeckElement::CrossReferences]
    );
}

#[test]
fn test_deck_parse_element_list_is_ordered_by_precedence() {
    // Supplied in reverse precedence order: the parser returns FR-003 order so
    // the slide order can never depend on the flag's spelling.
    assert_eq!(
        parse_deck_elements_list("References,Executive Summary,Findings"),
        vec![
            DeckElement::ExecutiveSummary,
            DeckElement::Findings,
            DeckElement::References
        ]
    );
}

#[test]
fn test_deck_parse_element_list_deduplicates() {
    match parse_deck("ai-survey --elements Findings,findings,Findings") {
        ResearchCliCommand::Deck { elements, .. } => {
            assert_eq!(elements, vec![DeckElement::Findings]);
        }
        other => panic!("unreachable: {other:?}"),
    }
}

#[test]
fn test_deck_parse_unknown_element_names_the_value_and_accepted_set() {
    let message = refusal("ai-survey --elements Findings,Conclusions");
    assert!(message.contains("'Conclusions'"), "got: {message}");
    assert!(message.contains("accepted values"), "got: {message}");
    for label in DeckElement::ALL.iter().map(|element| element.label()) {
        assert!(message.contains(label), "missing {label:?} in: {message}");
    }
}

#[test]
fn test_deck_parse_empty_element_list_is_refused() {
    for value in ["", ",", "Findings,,References"] {
        let message = refusal(&format!("ai-survey --elements '{value}'"));
        assert!(
            message.contains("--elements"),
            "value {value:?} -> {message}"
        );
    }
}

#[test]
fn test_deck_parse_switches() {
    match parse_deck("ai-survey --include-old --force") {
        ResearchCliCommand::Deck {
            include_old, force, ..
        } => {
            assert!(include_old);
            assert!(force);
        }
        other => panic!("unreachable: {other:?}"),
    }
}

#[test]
fn test_deck_parse_out_path() {
    match parse_deck("ai-survey --out docs/decks/ai.md") {
        ResearchCliCommand::Deck { out, name, .. } => {
            assert_eq!(out.as_deref(), Some("docs/decks/ai.md"));
            assert_eq!(name, "ai-survey");
        }
        other => panic!("unreachable: {other:?}"),
    }
}

#[test]
fn test_deck_parse_name_is_not_taken_from_a_flag_value() {
    // The name precedes the flags here; the reverse order must find it too, and
    // never mistake a date or path for the item name.
    match parse_deck("--from 2025-01-05 --out decks/out.md ai-survey") {
        ResearchCliCommand::Deck {
            name, from, out, ..
        } => {
            assert_eq!(name, "ai-survey");
            assert_eq!(from.as_deref(), Some("2025-01-05"));
            assert_eq!(out.as_deref(), Some("decks/out.md"));
        }
        other => panic!("unreachable: {other:?}"),
    }
}

#[test]
fn test_deck_parse_full_flag_set() {
    match parse_deck(
        "ai-survey --from 2025-01-05 --to 2025-02-14 --elements Findings --include-old \
         --out decks/ai.md --force",
    ) {
        ResearchCliCommand::Deck {
            name,
            from,
            to,
            elements,
            include_old,
            out,
            force,
        } => {
            assert_eq!(name, "ai-survey");
            assert_eq!(from.as_deref(), Some("2025-01-05"));
            assert_eq!(to.as_deref(), Some("2025-02-14"));
            assert_eq!(elements, vec![DeckElement::Findings]);
            assert!(include_old);
            assert_eq!(out.as_deref(), Some("decks/ai.md"));
            assert!(force);
        }
        other => panic!("unreachable: {other:?}"),
    }
}

#[test]
fn test_deck_parse_tolerates_unknown_flags() {
    // An unrecognised switch is ignored rather than swallowing the next token as
    // its value, so the item name still resolves.
    match parse_deck("--verbose ai-survey") {
        ResearchCliCommand::Deck { name, .. } => assert_eq!(name, "ai-survey"),
        other => panic!("unreachable: {other:?}"),
    }
}

#[test]
fn test_deck_element_names_lists_every_element_in_precedence_order() {
    let names = deck_element_names();
    let listed: Vec<&str> = names.split(", ").collect();
    let expected: Vec<&str> = DeckElement::ALL.iter().map(|e| e.label()).collect();
    assert_eq!(listed, expected);
}

#[test]
fn test_deck_element_names_round_trip_through_the_parser() {
    // Every advertised name is accepted by `--elements`, so the help text and
    // the parser cannot drift (PLAN risk: element-name drift).
    let names = deck_element_names();
    assert_eq!(parse_deck_elements_list(&names), DeckElement::ALL.to_vec());
}

#[test]
fn test_deck_verb_is_listed_in_help() {
    let help = ResearchCliCommand::build_help_message();
    assert!(help.contains("deck <name>"), "help missing deck verb");
    for flag in [
        "--from",
        "--to",
        "--elements",
        "--include-old",
        "--out",
        "--force",
    ] {
        assert!(help.contains(flag), "help missing {flag}");
    }
}
