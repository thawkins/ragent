//! Regression tests for FUNC-063: percent-encoding of query values and GitLab
//! project paths must encode UTF-8 bytes (not the scalar `char` value), so
//! non-ASCII and reserved characters are handled correctly.

use ragent_tools_vcs::percent::{encode_component, encode_project_path};

#[test]
fn unreserved_chars_pass_through() {
    assert_eq!(encode_component("abcXYZ019-_.~"), "abcXYZ019-_.~");
}

#[test]
fn space_and_reserved_chars_encoded() {
    assert_eq!(encode_component("a b"), "a%20b");
    assert_eq!(encode_component("a&b=c"), "a%26b%3Dc");
    assert_eq!(encode_component("a#b"), "a%23b");
    assert_eq!(encode_component("a,b"), "a%2Cb");
}

#[test]
fn non_ascii_encodes_as_utf8_bytes() {
    // `é` is U+00E9; correct UTF-8 percent-encoding is the two bytes %C3%A9,
    // not the scalar-value form %E9.
    assert_eq!(encode_component("é"), "%C3%A9");
    assert_eq!(encode_component("café"), "caf%C3%A9");
    // `€` (U+20AC) is three UTF-8 bytes.
    assert_eq!(encode_component("€"), "%E2%82%AC");
}

#[test]
fn project_path_encodes_slashes() {
    assert_eq!(encode_project_path("group/project"), "group%2Fproject");
    assert_eq!(
        encode_project_path("group/subgroup/project"),
        "group%2Fsubgroup%2Fproject"
    );
}

#[test]
fn project_path_encodes_non_ascii_namespace() {
    // A namespace containing a non-ASCII segment must encode those bytes.
    assert_eq!(encode_project_path("groué/project"), "grou%C3%A9%2Fproject");
}
