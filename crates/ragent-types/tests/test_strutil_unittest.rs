//! strutil unit tests relocated out of the inline `#[cfg(test)]` block in
//! `src/strutil.rs` (ANTIPAT M2.5/F1). All four helpers are public API.

use ragent_types::strutil::{floor_char_boundary, truncate_bytes_no_ellipsis};

#[test]
fn floor_char_boundary_snaps_to_character_start() {
    // The two-byte character is at offsets 3 and 4: index 3 is the char
    // start, index 4 is inside it and steps back to 3. Index 5 is the end.
    let two_byte = "caf\u{e9}";
    assert_eq!(floor_char_boundary(two_byte, 3), 3);
    assert_eq!(floor_char_boundary(two_byte, 4), 3);
    assert_eq!(floor_char_boundary(two_byte, 5), 5);
    // The three-byte character starts at offset 1: any cut inside steps to 1.
    let three_byte = "a\u{2014}b";
    assert_eq!(floor_char_boundary(three_byte, 1), 1);
    assert_eq!(floor_char_boundary(three_byte, 2), 1);
    assert_eq!(floor_char_boundary(three_byte, 3), 1);
    assert_eq!(floor_char_boundary(three_byte, 4), 4);
    // Out-of-range clamps to the length.
    assert_eq!(floor_char_boundary("abc", 99), 3);
    assert_eq!(floor_char_boundary("", 5), 0);
}

#[test]
fn truncate_bytes_no_ellipsis_keeps_short_strings() {
    assert_eq!(truncate_bytes_no_ellipsis("hello", 10), "hello");
}

#[test]
fn truncate_bytes_no_ellipsis_truncates_at_char_boundary() {
    // The two-byte character is at offsets 3 and 4; byte index 3 lands
    // between 'f' and it.
    let two_byte = "caf\u{e9}";
    assert_eq!(truncate_bytes_no_ellipsis(two_byte, 3), "caf");
    // The three-byte character starts at offset 1; "a" + it is 4 bytes.
    let three_byte = "a\u{2014}b";
    assert_eq!(truncate_bytes_no_ellipsis(three_byte, 4), "a\u{2014}");
    // Cutting inside it should step back to the start of the character.
    assert_eq!(truncate_bytes_no_ellipsis(three_byte, 3), "a");
}

#[test]
fn truncate_bytes_no_ellipsis_empty_when_zero() {
    assert_eq!(truncate_bytes_no_ellipsis("hello", 0), "");
}
