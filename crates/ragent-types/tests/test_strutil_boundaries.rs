//! Coverage for the remaining `strutil` helpers that only a doctest touched:
//! `truncate_bytes_no_ellipsis` is covered elsewhere, but the "keep whole
//! string within budget" and boundary-step branches of `truncate_chars` and
//! `truncate_bytes` are exercised here for the exact-cut and over-cut cases.

use ragent_types::strutil::{
    floor_char_boundary, truncate_bytes, truncate_bytes_no_ellipsis, truncate_chars,
};

#[test]
fn truncate_chars_returns_the_input_at_or_under_the_budget() {
    // Exactly at the budget: no ellipsis, no allocation of a suffix.
    assert_eq!(truncate_chars("abc", 3), "abc");
    // One over: the 3-char ellipsis eats into the head.
    assert_eq!(truncate_chars("abcd", 3), "...");
    assert_eq!(truncate_chars("abcde", 4), "a...");
    assert_eq!(truncate_chars("", 0), "");
}

#[test]
fn truncate_bytes_keeps_short_strings_and_marks_long_ones() {
    assert_eq!(truncate_bytes("hello", 5), "hello");
    assert_eq!(truncate_bytes("hello", 6), "hello");
    assert_eq!(truncate_bytes("hello world", 3), "hel...");
}

#[test]
fn truncate_bytes_no_ellipsis_steps_back_off_a_split_codepoint() {
    assert_eq!(truncate_bytes_no_ellipsis("hello", 5), "hello");
    // A 4-byte emoji starting at byte 0: indices 1..3 land inside it.
    let emoji = "\u{1F600}x";
    assert_eq!(truncate_bytes_no_ellipsis(emoji, 2), "");
    assert_eq!(truncate_bytes_no_ellipsis(emoji, 4), "\u{1F600}");
}

#[test]
fn floor_char_boundary_clamps_and_reaches_zero() {
    assert_eq!(floor_char_boundary("abcdef", 6), 6);
    assert_eq!(floor_char_boundary("abcdef", 0), 0);
    assert_eq!(floor_char_boundary("", 10), 0);
}
