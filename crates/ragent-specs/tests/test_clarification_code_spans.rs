//! Tests that `[NEEDS CLARIFICATION: ...]` text inside Markdown code is not
//! counted as an open clarification marker (#63).
//!
//! A spec that documents the marker syntax (e.g. `reqeng`) quotes it in an
//! inline code span or a fenced block; those quotes are examples, not open
//! questions, and must not block the `approved` transition.

use ragent_specs::validate::detect_clarification_markers;

#[test]
fn test_clarification_code_spans_inline_code_is_ignored() {
    let content = "The system shall report `[NEEDS CLARIFICATION: <question>]` markers.";
    assert_eq!(detect_clarification_markers(content), Vec::new());
}

#[test]
fn test_clarification_code_spans_double_backtick_span_is_ignored() {
    let content = "Use ``a ` tick [NEEDS CLARIFICATION: x]`` here.";
    assert_eq!(detect_clarification_markers(content), Vec::new());
}

#[test]
fn test_clarification_code_spans_fenced_block_is_ignored() {
    let content = "Intro\n```markdown\n[NEEDS CLARIFICATION: example]\n```\n~~~\n\
                   [NEEDS CLARIFICATION: tilde example]\n~~~\nOutro";
    assert_eq!(detect_clarification_markers(content), Vec::new());
}

#[test]
fn test_clarification_code_spans_marker_outside_code_still_counts() {
    let content = "Syntax: `[NEEDS CLARIFICATION: x]`. Open: [NEEDS CLARIFICATION: which DB?]\n\
                   ```\n[NEEDS CLARIFICATION: in fence]\n```\n\
                   [NEEDS CLARIFICATION: after fence]";
    let markers = detect_clarification_markers(content);
    let found: Vec<(usize, &str)> = markers
        .iter()
        .map(|m| (m.line, m.question.as_str()))
        .collect();
    assert_eq!(found, vec![(1, "which DB?"), (5, "after fence")]);
}

#[test]
fn test_clarification_code_spans_unclosed_backtick_does_not_hide_marker() {
    let content = "A stray ` backtick then [NEEDS CLARIFICATION: real question]";
    let markers = detect_clarification_markers(content);
    assert_eq!(markers.len(), 1);
    assert_eq!(markers[0].question, "real question");
}

#[test]
fn test_clarification_code_spans_reqeng_spec_has_no_open_markers() {
    let path = concat!(env!("CARGO_MANIFEST_DIR"), "/../../specs/reqeng/SPEC.md");
    let content = std::fs::read_to_string(path).expect("read reqeng SPEC.md");
    let markers = detect_clarification_markers(&content);
    assert!(markers.is_empty(), "unexpected markers: {markers:?}");
}

#[test]
fn test_clarification_code_spans_inline_triple_backticks_do_not_open_fence() {
    // A triple-backtick inline span at the start of a line is not a fence
    // (a backtick info string may not contain backticks), so the marker on
    // the next line still counts.
    let content = "```code``` example\n[NEEDS CLARIFICATION: still open]";
    let markers = detect_clarification_markers(content);
    assert_eq!(markers.len(), 1);
    assert_eq!(markers[0].line, 2);
}

#[test]
fn test_clarification_code_spans_closing_fence_needs_bare_line() {
    // "``` not a close" is not a closing fence, so the block stays open and
    // its example stays hidden; the marker after the real close counts.
    let content = "```\n``` not a close\n[NEEDS CLARIFICATION: example]\n```\n\
                   [NEEDS CLARIFICATION: real]";
    let markers = detect_clarification_markers(content);
    let found: Vec<(usize, &str)> = markers
        .iter()
        .map(|m| (m.line, m.question.as_str()))
        .collect();
    assert_eq!(found, vec![(5, "real")]);
}

#[test]
fn test_clarification_code_spans_escaped_backticks_do_not_hide_marker() {
    let content = r"Text \`[NEEDS CLARIFICATION: open]\` text";
    let markers = detect_clarification_markers(content);
    assert_eq!(markers.len(), 1);
    assert_eq!(markers[0].question, "open");
}

#[test]
fn test_clarification_code_spans_multiline_span_is_ignored() {
    let content = "Report `markers like\n[NEEDS CLARIFICATION: example]` in specs.\n\n\
                   [NEEDS CLARIFICATION: real]";
    let markers = detect_clarification_markers(content);
    let found: Vec<(usize, &str)> = markers
        .iter()
        .map(|m| (m.line, m.question.as_str()))
        .collect();
    assert_eq!(found, vec![(4, "real")]);
}

#[test]
fn test_clarification_code_spans_span_does_not_cross_blank_line() {
    let content = "A `stray\n\n[NEEDS CLARIFICATION: real]` tick";
    let markers = detect_clarification_markers(content);
    assert_eq!(markers.len(), 1);
    assert_eq!(markers[0].line, 3);
}
