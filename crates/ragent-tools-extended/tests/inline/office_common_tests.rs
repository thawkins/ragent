//! Inline tests for `office_common.rs` - compiled inside the module tree
//! via the `#[path]` delegation left in the source file
//! (ANTIPAT M2 inline-test migration; no behaviour change).

use super::*;

#[test]
fn truncate_output_does_not_panic_on_multibyte_boundary() {
    // Build a string longer than MAX_OUTPUT_BYTES where the byte limit
    // falls inside a multi-byte character ("e" is 2 bytes).
    let chunk = "e".repeat(200);
    let text = chunk.repeat(MAX_OUTPUT_BYTES / chunk.len() + 1);
    let result = truncate_output(text);
    assert!(
        result.len() <= MAX_OUTPUT_BYTES + 128,
        "truncated result unexpectedly large"
    );
    assert!(
        result.contains("Output truncated"),
        "truncated result should include the truncation notice"
    );
}

#[test]
fn truncate_output_keeps_short_text_unchanged() {
    let text = "Short text with emojis [done]".to_string();
    assert_eq!(truncate_output(text.clone()), text);
}
