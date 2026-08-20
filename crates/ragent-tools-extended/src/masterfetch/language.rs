//! Human-language detection for masterfetch pages.
//!
//! This module wraps the [`lingua`] crate to derive the natural language of a
//! fetched page's extracted text. The detected language name (e.g. `"English"`,
//! `"French"`) is attached to [`super::PageMetadata::detected_language`] by
//! `mf_fetch` so downstream consumers — notably the research system's
//! `RESEARCH.md` References Index — can report the language of each web source.
//!
//! The [`LanguageDetector`] is thread-safe and shared across all calls via a
//! [`OnceLock`], so the 75-language model is loaded into memory only once per
//! process. Detection is a best-effort, never-panicking operation: empty or
//! non-linguistic content returns `None`.

use std::sync::OnceLock;

use lingua::{LanguageDetector, LanguageDetectorBuilder};

/// Shared detector built from all 75 supported languages.
///
/// `lingua` detectors are thread-safe and share language-model memory across
/// instances, so a single process-wide detector is both safe and cheap.
static DETECTOR: OnceLock<LanguageDetector> = OnceLock::new();

/// Shared low-accuracy detector used as a last-resort best-guess.
///
/// This detector loads a smaller subset of `lingua`'s n-gram models so it is
/// more willing to return a language for short or otherwise marginal text.
/// It is intentionally separate from the normal detector so that `mf_fetch`
/// and other callers can keep the stricter default while the research layer
/// can fall back to a best-guess for the stored `Source::Web.language`.
static AGGRESSIVE_DETECTOR: OnceLock<LanguageDetector> = OnceLock::new();

/// Return the shared [`LanguageDetector`], building it on first use.
fn detector() -> &'static LanguageDetector {
    DETECTOR.get_or_init(|| LanguageDetectorBuilder::from_all_languages().build())
}

/// Return the shared low-accuracy [`LanguageDetector`], building it on first use.
fn aggressive_detector() -> &'static LanguageDetector {
    AGGRESSIVE_DETECTOR.get_or_init(|| {
        LanguageDetectorBuilder::from_all_languages()
            .with_low_accuracy_mode()
            .build()
    })
}

/// Detect the human language of `text` and return its full name.
///
/// Returns `None` when:
/// - `text` is empty or whitespace-only.
/// - `lingua` cannot confidently identify a language (very short or
///   non-linguistic content).
///
/// The returned string is the human-readable language name (e.g. `"English"`,
/// `"German"`, `"Japanese"`), suitable for display in the `RESEARCH.md`
/// References Index.
///
/// # Example
///
/// ```
/// use ragent_tools_extended::masterfetch::language::detect_language;
///
/// let lang = detect_language("The quick brown fox jumps over the lazy dog.");
/// assert_eq!(lang.as_deref(), Some("English"));
/// ```
#[must_use]
pub fn detect_language(text: &str) -> Option<String> {
    if text.trim().is_empty() {
        return None;
    }
    detector()
        .detect_language_of(text)
        .map(|lang| lang.to_string())
}

/// Detect the human language of `text`, returning a best guess when the normal
/// detector is not confident.
///
/// This function first tries [`detect_language`]. If that returns `None`, it
/// falls back to `lingua`'s low-accuracy detector so that short snippets (such
/// as search-engine abstracts or very short pages) still get a language label.
/// The trade-off is a higher chance of misclassification for marginal text.
///
/// Returns `None` only when `text` is empty or whitespace-only, or when no
/// language can be guessed even in low-accuracy mode.
///
/// The returned string uses the same human-readable language names as
/// [`detect_language`] (e.g. `"English"`, `"French"`).
///
/// # Example
///
/// ```
/// use ragent_tools_extended::masterfetch::language::detect_language_best_effort;
///
/// let lang = detect_language_best_effort("The quick brown fox.");
/// assert_eq!(lang.as_deref(), Some("English"));
/// ```
#[must_use]
pub fn detect_language_best_effort(text: &str) -> Option<String> {
    if text.trim().is_empty() {
        return None;
    }
    detect_language(text).or_else(|| {
        aggressive_detector()
            .detect_language_of(text)
            .map(|lang| lang.to_string())
    })
}
