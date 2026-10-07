//! Inline tests for `research_name.rs` - compiled inside the module tree
//! via the `#[path]` delegation left in the source file
//! (ANTIPAT M2 inline-test migration; no behaviour change).

use super::*;

#[test]
fn accepts_simple_lowercase_name() {
    assert!(ResearchName::new("rust").is_some());
}

#[test]
fn accepts_letters_digits_and_hyphens() {
    assert!(ResearchName::new("rust-2024").is_some());
    assert!(ResearchName::new("a-b-c").is_some());
    assert!(ResearchName::new("abc123").is_some());
}

#[test]
fn rejects_empty() {
    assert_eq!(ResearchName::try_new(""), Err(ResearchNameError::Empty));
}

#[test]
fn rejects_too_short() {
    let err = ResearchName::try_new("ab").unwrap_err();
    assert_eq!(err, ResearchNameError::TooShort { length: 2 });
}

#[test]
fn accepts_min_length() {
    assert!(ResearchName::new("abc").is_some());
}

#[test]
fn rejects_too_long() {
    let too_long = "a".repeat(MAX_LEN + 1);
    let err = ResearchName::try_new(too_long).unwrap_err();
    assert_eq!(
        err,
        ResearchNameError::TooLong {
            length: MAX_LEN + 1
        }
    );
}

#[test]
fn accepts_max_length() {
    let name = "a".repeat(MAX_LEN);
    assert!(ResearchName::new(name).is_some());
}

#[test]
fn rejects_uppercase() {
    assert_eq!(
        ResearchName::try_new("Rust"),
        Err(ResearchNameError::InvalidStart { ch: 'R' }),
    );
    assert_eq!(
        ResearchName::try_new("rustAsync"),
        Err(ResearchNameError::InvalidCharacter { ch: 'A' }),
    );
}

#[test]
fn rejects_starting_with_digit() {
    assert_eq!(
        ResearchName::try_new("1abc"),
        Err(ResearchNameError::InvalidStart { ch: '1' }),
    );
}

#[test]
fn rejects_starting_with_hyphen() {
    assert_eq!(
        ResearchName::try_new("-abc"),
        Err(ResearchNameError::InvalidStart { ch: '-' }),
    );
}

// -- FR-017: path-traversal rejection --------------------------------

#[test]
fn rejects_path_traversal() {
    assert!(ResearchName::new("../etc").is_none());
    assert!(ResearchName::new("foo/bar").is_none());
    assert!(ResearchName::new("..").is_none());
    assert!(ResearchName::new(".hidden").is_none());
}

#[test]
fn rejects_parent_traversal_with_path_traversal_error() {
    // `../etc` should produce a dedicated PathTraversal error, not
    // a generic InvalidCharacter one, so callers can show a security
    // message.
    let err = ResearchName::try_new("../etc").unwrap_err();
    assert!(
        matches!(err, ResearchNameError::PathTraversal { .. }),
        "expected PathTraversal error, got {err:?}"
    );
}

#[test]
fn rejects_forward_slash_with_path_traversal_error() {
    let err = ResearchName::try_new("foo/bar").unwrap_err();
    assert!(
        matches!(err, ResearchNameError::PathTraversal { .. }),
        "expected PathTraversal error for '/', got {err:?}"
    );
}

#[test]
fn rejects_backslash_with_path_traversal_error() {
    let err = ResearchName::try_new("foo\\bar").unwrap_err();
    assert!(
        matches!(err, ResearchNameError::PathTraversal { .. }),
        "expected PathTraversal error for backslash, got {err:?}"
    );
}

#[test]
fn rejects_leading_dot_with_path_traversal_error() {
    let err = ResearchName::try_new(".hidden").unwrap_err();
    assert!(
        matches!(err, ResearchNameError::PathTraversal { .. }),
        "expected PathTraversal error for leading dot, got {err:?}"
    );
}

#[test]
fn rejects_nested_traversal_with_path_traversal_error() {
    let err = ResearchName::try_new("a/../b").unwrap_err();
    assert!(matches!(err, ResearchNameError::PathTraversal { .. }));
}

#[test]
fn rejects_absolute_path_with_path_traversal_error() {
    let err = ResearchName::try_new("/etc/passwd").unwrap_err();
    assert!(matches!(err, ResearchNameError::PathTraversal { .. }));
}

#[test]
fn rejects_windows_absolute_path_with_path_traversal_error() {
    let err = ResearchName::try_new("C:\\Windows").unwrap_err();
    assert!(matches!(err, ResearchNameError::PathTraversal { .. }));
}

#[test]
fn is_path_traversal_helper_classifies_correctly() {
    assert!(is_path_traversal(".."));
    assert!(is_path_traversal("../etc"));
    assert!(is_path_traversal("foo/bar"));
    assert!(is_path_traversal("foo\\bar"));
    assert!(is_path_traversal(".hidden"));
    assert!(is_path_traversal("a/../b"));
    assert!(!is_path_traversal("rust-async"));
    assert!(!is_path_traversal("abc"));
    assert!(!is_path_traversal("a-b-c"));
}

#[test]
fn rejects_whitespace_and_unicode() {
    assert!(ResearchName::new("foo bar").is_none());
    assert!(ResearchName::new("foo\nbar").is_none());
    assert!(ResearchName::new("caf\u{e9}").is_none());
}

#[test]
fn dir_name_matches_as_str() {
    let name = ResearchName::new("rust-async").unwrap();
    assert_eq!(name.dir_name(), "rust-async");
    assert_eq!(name.as_str(), "rust-async");
    assert_eq!(name.to_string(), "rust-async");
}

#[test]
fn try_from_string_succeeds() {
    let name: ResearchName = "foo-bar".to_string().try_into().unwrap();
    assert_eq!(name.as_str(), "foo-bar");
}

#[test]
fn try_from_string_fails() {
    let result: Result<ResearchName, _> = "AB".to_string().try_into();
    assert!(result.is_err());
}

#[test]
fn equality_and_hash() {
    let a = ResearchName::new("rust-lang").unwrap();
    let b = ResearchName::new("rust-lang").unwrap();
    let c = ResearchName::new("go-lang").unwrap();
    assert_eq!(a, b);
    assert_ne!(a, c);
    use std::collections::HashSet;
    let mut set = HashSet::new();
    set.insert(a);
    assert!(set.contains(&b));
    assert!(!set.contains(&c));
}

#[test]
fn path_traversal_error_displays_readably() {
    let err = ResearchNameError::PathTraversal {
        sequence: "../etc".to_string(),
    };
    let msg = err.to_string();
    assert!(msg.contains("path-traversal"), "msg: {msg}");
    assert!(msg.contains("FR-017"), "msg: {msg}");
}

#[test]
fn non_traversal_invalid_character_still_surfaces_correctly() {
    // A `?` is an invalid character but NOT a traversal sequence, so
    // it should still produce InvalidCharacter (not PathTraversal).
    let err = ResearchName::try_new("foo?bar").unwrap_err();
    assert_eq!(
        err,
        ResearchNameError::InvalidCharacter { ch: '?' },
        "expected InvalidCharacter, got {err:?}"
    );
}
