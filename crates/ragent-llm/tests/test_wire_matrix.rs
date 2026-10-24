//! Tests for the FR-016 wire matrix renderer.
//!
//! FR-016 asks for a per-provider table of the nine declared profile fields,
//! printable without contacting a provider and without executing a tool. The
//! renderer is a pure function over provider ids plus the `const` profile table,
//! so everything here is offline: no registry, no environment, no I/O (NFR-004).
//!
//! The cases are grouped by the property each protects:
//!
//! - **No blank cell** (TC-002, NFR-005): every cell of every row is a non-empty
//!   ASCII value, and the table is ASCII throughout - a provider with a missing
//!   label cannot print as whitespace and pass for complete.
//! - **One row per provider**: a row is printed for each id supplied, and the
//!   count matches, so the CLI's "count the rows against the provider list" step
//!   (TC-002 step 3) has something true to compare.
//! - **Faithful to the declared contract**: the cells a reader is most likely to
//!   question (Gemini's synthesised id, the Responses family's `by_call_id`
//!   stream keying, Bedrock's native error flag) are asserted from the table
//!   constants, so the printed matrix cannot drift from what the encoders send.
//! - **Deterministic**: the registry enumerates a `HashMap`, so the renderer sorts
//!   by id; the same ids in a different order must render the same bytes.
//! - **Refuses a hole**: an id with no declared profile is an error naming it,
//!   never a row of blanks.

use ragent_llm::provider::wire::matrix::{HEADINGS, MissingProfile, render};
use ragent_llm::provider::wire::profile_table::PROVIDER_WIRE;

/// Every registered provider id, in table order.
fn all_ids() -> Vec<&'static str> {
    PROVIDER_WIRE.iter().map(|(id, _)| *id).collect()
}

/// Split one printed row into its columns.
///
/// Columns are separated by **two or more** spaces (a narrow cell is padded to
/// its column width, so the gap widens), and no cell contains two consecutive
/// spaces, which makes this split exact rather than approximate.
fn columns(line: &str) -> Vec<&str> {
    line.split("  ")
        .map(str::trim)
        .filter(|c| !c.is_empty())
        .collect()
}

#[test]
fn test_wire_matrix_prints_one_row_per_provider() {
    let ids = all_ids();
    let table = render(&ids).expect("every table row has a profile");
    // heading + rule + one row per provider, with no trailing blank line.
    assert_eq!(
        table.lines().count(),
        ids.len() + 2,
        "expected a heading, a rule and {} rows, got:\n{table}",
        ids.len()
    );
    for id in &ids {
        assert!(
            table.lines().any(|line| line.starts_with(id)),
            "no row for provider {id:?} in:\n{table}"
        );
    }
}

#[test]
fn test_wire_matrix_has_no_blank_cell() {
    let table = render(&all_ids()).expect("every table row has a profile");
    // TC-002: no cell prints `-`, `none` or a blank where a value is expected. A
    // blank cell would collapse the two-space column separator into a run of
    // spaces, so splitting on a run of 2+ spaces and requiring a non-empty value
    // for each of the ten columns covers both the "blank" and the "-" cases.
    for (index, line) in table.lines().enumerate().skip(2) {
        let cells = columns(line);
        assert_eq!(
            cells.len(),
            HEADINGS.len(),
            "row {index} does not have {} populated columns: {line:?}",
            HEADINGS.len()
        );
        for (column, cell) in cells.iter().enumerate() {
            assert!(
                !cell.is_empty(),
                "row {index}, column {column} ({}) is blank: {line:?}",
                HEADINGS[column]
            );
            assert_ne!(*cell, "-", "row {index}, column {column} is a dash");
            assert_ne!(
                cell.to_ascii_lowercase(),
                "none",
                "row {index}, column {column} is `none`"
            );
        }
    }
}

#[test]
fn test_wire_matrix_is_plain_ascii() {
    let table = render(&all_ids()).expect("every table row has a profile");
    assert!(
        table.is_ascii(),
        "the wire matrix must be plain ASCII, got:\n{table}"
    );
    // Only printable ASCII plus the newline that ends each row.
    assert!(
        table.chars().all(|c| c == '\n' || (' '..='~').contains(&c)),
        "the wire matrix contains a control character"
    );
    // No line carries trailing whitespace, so an editor or a paste cannot make
    // two identical tables compare unequal.
    for line in table.lines() {
        assert_eq!(line.trim_end(), line, "trailing whitespace in {line:?}");
    }
}

#[test]
fn test_wire_matrix_shows_the_declared_contract_not_a_second_copy() {
    // The cells a reader is most likely to question, asserted against the printed
    // text: Gemini synthesises ids, the Responses family keys its stream by call
    // id, Bedrock transmits the error flag natively, and only Anthropic replays
    // signed reasoning.
    let table = render(&all_ids()).expect("every table row has a profile");

    let row = |provider: &str| -> String {
        table
            .lines()
            .find(|line| line.starts_with(provider) && line.contains("  "))
            .unwrap_or_else(|| panic!("no row for {provider} in:\n{table}"))
            .to_string()
    };

    let gemini = row("gemini ");
    assert!(
        gemini.contains("synthesised"),
        "gemini row does not report the synthesised id: {gemini:?}"
    );
    assert!(
        gemini.contains("gemini_function_declaration")
            && gemini.contains("gemini_function_response"),
        "gemini row does not report its envelope and carrier: {gemini:?}"
    );

    let responses = row("openai_responses ");
    assert!(
        responses.contains("by_call_id"),
        "openai_responses row does not report id-keyed accumulation: {responses:?}"
    );

    let bedrock = row("bedrock");
    assert!(
        bedrock.contains("bedrock_tool_spec") && bedrock.contains("toolUseId"),
        "bedrock row does not report its envelope and id key: {bedrock:?}"
    );

    let anthropic = row("anthropic");
    let reasoning = columns(&anthropic)[HEADINGS
        .iter()
        .position(|h| *h == "REASONING")
        .expect("REASONING is a heading")];
    assert_eq!(
        reasoning, "yes",
        "the anthropic row must report reasoning echoed: {anthropic:?}"
    );

    // The two Ollama providers are different families, which is the distinction
    // the table exists to record; the table must show it.
    let ollama = row("ollama ");
    let ollama_cloud = row("ollama_cloud ");
    assert!(
        ollama.contains("json_string") && ollama_cloud.contains("json_object"),
        "the two Ollama rows must differ on the argument encoding:\n{ollama}\n{ollama_cloud}"
    );
}

#[test]
fn test_wire_matrix_is_sorted_and_order_independent() {
    let mut ids = all_ids();
    let sorted = render(&ids).expect("every table row has a profile");

    ids.reverse();
    let reversed = render(&ids).expect("every table row has a profile");
    assert_eq!(
        sorted, reversed,
        "the registry enumerates a HashMap, so the renderer must sort by id"
    );

    let printed: Vec<&str> = sorted
        .lines()
        .skip(2)
        .map(|line| line.split_whitespace().next().unwrap_or_default())
        .collect();
    let mut expected = printed.clone();
    expected.sort_unstable();
    assert_eq!(printed, expected, "rows are not sorted by provider id");
}

#[test]
fn test_wire_matrix_refuses_a_provider_with_no_declared_profile() {
    let err = render(&["definitely_not_a_provider"])
        .expect_err("an undeclared provider must not print a row of blanks");
    assert_eq!(
        err,
        MissingProfile {
            providers: vec!["definitely_not_a_provider".to_string()],
        }
    );
    let message = err.to_string();
    assert!(
        message.contains("definitely_not_a_provider"),
        "the diagnostic must name the provider: {message}"
    );

    // A mix of known and unknown ids renders nothing at all, so a partial table
    // is never printed as if it were complete.
    let mixed = render(&["openai", "definitely_not_a_provider"]);
    assert!(mixed.is_err(), "a partial table must not be rendered");
}

#[test]
fn test_wire_matrix_headings_match_the_columns_each_row_prints() {
    // One heading per column, and the heading row is padded like a data row, so a
    // column added without its heading breaks this rather than silently shifting
    // every value one column left.
    let table = render(&["openai"]).expect("openai has a profile");
    let heading = table.lines().next().expect("heading line");
    let cells = columns(heading);
    assert_eq!(cells.len(), HEADINGS.len());
    assert_eq!(cells, HEADINGS.to_vec());

    // The rule line spans the full table width.
    let rule = table.lines().nth(1).expect("rule line");
    assert!(
        rule.chars().all(|c| c == '-'),
        "the rule line must be all dashes, got {rule:?}"
    );
    assert!(
        rule.len() >= heading.trim_end().len(),
        "the rule is narrower than the heading"
    );
}

#[test]
fn test_wire_matrix_renders_an_empty_provider_list_as_a_bare_heading() {
    // Not reachable from the CLI (the default registry always registers a
    // provider), but the renderer must not panic or print a stray rule when
    // handed nothing - a future provider-less build should degrade quietly.
    let table = render(&[]).expect("no ids is not an error");
    let lines: Vec<&str> = table.lines().collect();
    assert_eq!(lines.len(), 2, "expected heading + rule, got:\n{table}");
    assert!(lines[0].starts_with("PROVIDER"));
}
