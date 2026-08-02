//! External tests for `tests` from `crates/ragent-tools-extended/src/masterfetch/language.rs`
//!
//! Relocated from the inline `#[cfg(test)]` module.

use ragent_tools_extended::masterfetch::language::*;

#[test]
fn detects_english() {
    let lang = detect_language(
        "The quick brown fox jumps over the lazy dog. \
         Pack my box with five dozen liquor jugs.",
    );
    assert_eq!(lang.as_deref(), Some("English"));
}

#[test]
fn detects_french() {
    let lang = detect_language(
        "Le renard brun rapide saute par-dessus le chien paresseux. \
         Portez ce vieux whisky au juge blond qui fume.",
    );
    assert_eq!(lang.as_deref(), Some("French"));
}

#[test]
fn empty_text_returns_none() {
    assert!(detect_language("").is_none());
    assert!(detect_language("   \n\t  ").is_none());
}

#[test]
fn very_short_text_returns_none_or_some() {
    // Single characters are not reliably detectable; the helper must not
    // panic regardless of the outcome.
    let _ = detect_language("a");
}
