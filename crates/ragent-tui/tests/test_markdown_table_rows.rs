//! Test: a whitespace-only cell row must not emit an all-blank data line, and
//! a table that ends on a separator must gain exactly one closing border.

#[path = "support/mod.rs"]
mod support;

#[test]
fn test_normalize_tables_drops_whitespace_only_row() {
    let app = support::make_app();
    // html2text emits a whitespace-only text row when one body row of a table
    // is dropped (it shows up in the middle of the html2text output as a row
    // made of empty cells).
    let input = "───┬────┬──\n\
                 │  │    │  │\n\
                 ───┼────┼──\n\
                 │ID│Name│Val│\n\
                 ───┼────┼──\n\
                 │x │y   │1  │\n\
                 ───┴────┴──";
    let out = app.normalize_ascii_tables(input);
    assert!(
        !out.lines().any(|line| {
            let t = line.trim();
            !t.is_empty() && t.chars().all(|c| matches!(c, '|' | ' '))
        }),
        "an all-empty data row must not be rendered: {out}"
    );
    assert!(out.contains("Name"), "header should survive: {out}");
    assert!(out.contains("| x "), "data row should survive: {out}");
}

#[test]
fn test_normalize_tables_duplicate_bottom_border_removed() {
    let app = support::make_app();
    // A table whose html2text output ends on the `+` bottom border used to get
    // one extra closing border line below it (the "empty row at the bottom").
    let input = "───┬────┬──\n\
                 │ID│Name│Val│\n\
                 ───┼────┼──\n\
                 │x │y   │1  │\n\
                 ───┴────┴──";
    let out = app.normalize_ascii_tables(input);
    let closing_borders = out
        .lines()
        .filter(|line| {
            let t = line.trim();
            t.len() > 1 && t.chars().all(|c| matches!(c, '+' | '-'))
        })
        .count();
    assert_eq!(
        closing_borders, 3,
        "top + header-bottom + bottom borders only: {out}"
    );
    assert!(out.contains("| x "), "data row should survive: {out}");
}

#[test]
fn test_normalize_tables_header_only_table_gets_bottom_border() {
    let app = support::make_app();
    // A table with a header row and no data rows still needs its closing
    // border after the fix.
    let input = "───┬────┬──\n\
                 │ID│Name│Val│\n\
                 ───┼────┼──";
    let out = app.normalize_ascii_tables(input);
    let closing_borders = out
        .lines()
        .filter(|line| {
            let t = line.trim();
            t.len() > 1 && t.chars().all(|c| matches!(c, '+' | '-'))
        })
        .count();
    assert_eq!(
        closing_borders, 2,
        "header table: top border and header-bottom border: {out}"
    );
    assert!(out.contains("Name"), "header should survive: {out}");
}
