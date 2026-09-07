//! Tests for `hard_break_lines`, the helper that converts single newlines
//! into markdown hard breaks before agent_complete summaries are routed
//! through the markdown-to-HTML-to-text rendering pipeline.

use ragent_tui::app::hard_break_lines;

#[test]
fn test_hard_break_lines_adds_two_trailing_spaces() {
    let input = "Implemented feature X.\nWrote 3 tests.\nUpdated docs.";
    assert_eq!(
        hard_break_lines(input),
        "Implemented feature X.  \nWrote 3 tests.  \nUpdated docs.  "
    );
}

#[test]
fn test_hard_break_lines_keeps_blank_lines_as_paragraph_breaks() {
    let input = "First paragraph.\n\nSecond paragraph.";
    assert_eq!(
        hard_break_lines(input),
        "First paragraph.  \n\nSecond paragraph.  "
    );
}

#[test]
fn test_hard_break_lines_preserves_existing_hard_breaks() {
    let input = "already broken  \nnext line";
    assert_eq!(hard_break_lines(input), "already broken  \nnext line  ");
}

#[test]
fn test_hard_break_lines_trims_trailing_whitespace_before_break() {
    let input = "trailing spaces   \nnext";
    assert_eq!(hard_break_lines(input), "trailing spaces  \nnext  ");
}

#[test]
fn test_hard_break_lines_empty_and_single_line() {
    assert_eq!(hard_break_lines(""), "");
    assert_eq!(hard_break_lines("only line"), "only line  ");
}
