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
fn single_character_returns_some_or_none_but_never_a_panic() {
    // `lingua` returns whichever language its n-gram model scores highest, so a
    // single character can yield a confident-but-meaningless guess ("a" and "x"
    // both score Somali) rather than `None`. Pinning the exact language would
    // encode a training-data accident, so this pins the CONTRACT instead: the
    // call terminates with a well-formed answer and never panics, and any
    // language it does return is a non-empty name from the supported set.
    for text in ["a", "x", "ab", "a b"] {
        match detect_language(text) {
            Some(name) => assert!(
                !name.trim().is_empty(),
                "detect_language({text:?}) returned a blank language name"
            ),
            None => {}
        }
    }
}

#[test]
fn single_character_result_is_reproducible() {
    // A one-character input is the marginal case where a change in the model,
    // the language set, or the accuracy mode would show up first. Whatever it
    // decides, it must decide it consistently - a detector that answered
    // differently on a second call would make the References Index unstable.
    for text in ["a", "x", "ab"] {
        assert_eq!(
            detect_language(text),
            detect_language(text),
            "detect_language({text:?}) is not reproducible"
        );
    }
}

#[test]
fn digit_only_text_returns_none() {
    // Digits carry no linguistic signal in any supported language, so `lingua`
    // declines to answer. This is the genuine `None` short-input case - the
    // counterpart to the single-character one above, which does produce a guess.
    assert_eq!(detect_language("1"), None);
    assert_eq!(detect_language("12345"), None);
}

#[test]
fn punctuation_only_text_returns_none() {
    assert_eq!(detect_language("!!"), None);
    assert_eq!(detect_language("... --- ..."), None);
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
