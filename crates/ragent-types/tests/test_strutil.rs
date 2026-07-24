//! Integration tests for `ragent-types` string truncation utilities.
//!
//! Relocated from the inline `#[cfg(test)]` module in `src/strutil.rs`
//! (T-010 of the testconsolidate spec). All tested functions are public.

use ragent_types::strutil::{truncate_bytes, truncate_chars};

#[test]
fn test_truncate_chars_noop() {
    assert_eq!(truncate_chars("hello", 10), "hello");
}

#[test]
fn test_truncate_chars_ascii() {
    assert_eq!(truncate_chars("hello world", 5), "hello…");
}

#[test]
fn test_truncate_chars_multibyte() {
    assert_eq!(truncate_chars("café résumé", 5), "café …");
}

#[test]
fn test_truncate_chars_en_dash() {
    assert_eq!(truncate_chars("A – B C", 4), "A – …");
}

#[test]
fn test_truncate_bytes_noop() {
    assert_eq!(truncate_bytes("hello", 10), "hello");
}

#[test]
fn test_truncate_bytes_ascii() {
    assert_eq!(truncate_bytes("hello world", 3), "hel…");
}

#[test]
fn test_truncate_bytes_boundary_adjustment() {
    assert_eq!(truncate_bytes("café", 3), "caf…");
}

#[test]
fn test_truncate_bytes_en_dash() {
    let result = truncate_bytes("A – B", 2);
    assert!(result.ends_with('…'));
    assert!(result.starts_with('A'));
}

#[test]
fn test_truncate_bytes_em_dash_at_400_boundary() {
    // Regression test for a panic where a tool-result preview sliced at a
    // fixed 400-byte index that fell inside a 3-byte em dash (bytes 398..401).
    // `truncate_bytes` must step back to the previous character boundary.
    let prefix = "a".repeat(398);
    let input = format!("{prefix}\u{2014}more text after");
    let result = truncate_bytes(&input, 400);
    assert_eq!(result, format!("{prefix}…"));
}
