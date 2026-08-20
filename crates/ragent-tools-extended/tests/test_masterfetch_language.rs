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
#[test]
fn best_effort_falls_back_for_short_text() {
    // The normal detector may or may not return None for very short snippets
    // depending on the text. The important property is that the best-effort
    // detector returns a guess rather than None when it can, and matches the
    // normal detector when the normal detector is confident.
    let text = "The quick brown fox.";
    let normal = detect_language(text);
    let best_effort = detect_language_best_effort(text);
    assert!(
        best_effort.is_some(),
        "best-effort detector should produce a language guess for short text"
    );
    if let Some(n) = normal {
        assert_eq!(
            best_effort.as_deref(),
            Some(n.as_str()),
            "best-effort should match normal detection when normal is confident"
        );
    }
}

#[test]
fn best_effort_still_returns_none_for_non_linguistic_text() {
    assert!(detect_language_best_effort("").is_none());
    assert!(detect_language_best_effort("   \n\t  ").is_none());
}

#[test]
fn best_effort_matches_normal_detector_when_confident() {
    let text = "Le renard brun rapide saute par-dessus le chien paresseux. Portez ce vieux whisky au juge blond qui fume.";
    assert_eq!(
        detect_language_best_effort(text),
        detect_language(text),
        "best-effort should return the same confident result as normal detection"
    );
}
