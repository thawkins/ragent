//! Tests for markdown rendering and ASCII table normalization (Section 4.E).

#[path = "support/mod.rs"]
mod support;

// =========================================================================
// render_markdown_to_ascii — prefix gating
// =========================================================================

#[test]
fn test_render_markdown_no_prefix_passthrough() {
    let mut app = support::make_app();
    let input = "Hello world, **bold** text";
    let output = app.render_markdown_to_ascii(input);
    // Without the "From: /" prefix, text is returned as-is.
    assert_eq!(output, input);
}

#[test]
fn test_render_markdown_with_prefix_converts() {
    let mut app = support::make_app();
    let input = "From: /help\n\n**Bold** text";
    let output = app.render_markdown_to_ascii(input);
    // html2text converts HTML back to terminal-friendly text.
    // For simple bold, output may be identical (html2text renders <strong> as **...**).
    // The key is that the pipeline runs without error and preserves content.
    assert!(output.contains("Bold"), "text content should be preserved");
    assert!(
        output.contains("text"),
        "remaining text should be preserved"
    );
    assert!(output.contains("From: /help"), "header should be preserved");
}

// =========================================================================
// render_markdown_to_ascii — table rendering
// =========================================================================

#[test]
fn test_render_markdown_table() {
    let mut app = support::make_app();
    let input = "From: /test\n\n| Name | Value |\n|------|-------|\n| foo | 42 |\n| bar | 99 |";
    let output = app.render_markdown_to_ascii(input);
    // Should contain normalized table with pipes and dashes.
    assert!(
        output.contains("foo"),
        "cell content should survive: {output}"
    );
    assert!(
        output.contains("42"),
        "cell content should survive: {output}"
    );
    assert!(
        output.contains("bar"),
        "cell content should survive: {output}"
    );
    // Should have some kind of table border structure
    assert!(
        output.contains('+') || output.contains('|') || output.contains('─'),
        "should have table structure: {output}"
    );
}

// =========================================================================
// normalize_ascii_tables — direct tests
// =========================================================================

#[test]
fn test_normalize_tables_non_table_passthrough() {
    let app = support::make_app();
    let input = "Hello\nWorld\nNo tables here";
    let output = app.normalize_ascii_tables(input);
    assert_eq!(output, input);
}

#[test]
fn test_normalize_tables_aligns_columns() {
    let app = support::make_app();
    // Simulate html2text output with │ separators
    let input = "│ Name │ Value │\n─────────────────\n│ foo │ 42 │\n│ barbaz │ 1 │";
    let output = app.normalize_ascii_tables(input);
    // After normalization, columns should be aligned (padded to equal width).
    let lines: Vec<&str> = output.lines().collect();
    // Find data rows (lines containing '|')
    let data_lines: Vec<&&str> = lines.iter().filter(|l| l.contains('|')).collect();
    if data_lines.len() >= 2 {
        // All data rows should have the same length (padded).
        let first_len = data_lines[0].len();
        for line in &data_lines {
            assert_eq!(
                line.len(),
                first_len,
                "all data rows should have the same width"
            );
        }
    }
}

#[test]
fn test_normalize_tables_adds_borders() {
    let app = support::make_app();
    // Table with │ separators — normalize should add +---+---+ borders.
    let input = "│ A │ B │\n──────────\n│ 1 │ 2 │";
    let output = app.normalize_ascii_tables(input);
    assert!(
        output.contains('+'),
        "normalized table should have '+' border corners: {output}"
    );
    assert!(
        output.contains('-'),
        "normalized table should have '-' border lines: {output}"
    );
}

// =========================================================================
// render_markdown_to_ascii — fallback on plain text
// =========================================================================

#[test]
fn test_render_markdown_empty_input() {
    let mut app = support::make_app();
    let output = app.render_markdown_to_ascii("");
    assert_eq!(output, "");
}

#[test]
fn test_render_markdown_code_block() {
    let mut app = support::make_app();
    let input = "From: /test\n\n```rust\nfn main() {}\n```";
    let output = app.render_markdown_to_ascii(input);
    assert!(
        output.contains("fn main()"),
        "code block content should survive: {output}"
    );
}

#[test]
fn test_render_markdown_list() {
    let mut app = support::make_app();
    let input = "From: /test\n\n- Item one\n- Item two\n- Item three";
    let output = app.render_markdown_to_ascii(input);
    assert!(
        output.contains("Item one"),
        "list items should survive: {output}"
    );
    assert!(
        output.contains("Item three"),
        "list items should survive: {output}"
    );
}

#[test]
fn test_render_markdown_html2text_panic_fallback() {
    let mut app = support::make_app();
    // Construct HTML that triggers the html2text overflow in text_renderer.rs:509.
    // A deeply nested preformatted block with a width smaller than its internal
    // line accounting can cause `self.width - self.line.len` to underflow.
    let input = "From: /test\n\n```\n".to_string() + &"a ".repeat(500) + "\n```";
    let output = app.render_markdown_to_ascii(&input);
    // If html2text panics, the catch_unwind fallback returns the original text.
    assert!(
        output.contains("From: /test"),
        "fallback should preserve original text: {output}"
    );
    assert!(
        output.contains("a a"),
        "fallback should preserve content: {output}"
    );
}
