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

/// Return the shared [`LanguageDetector`], building it on first use.
fn detector() -> &'static LanguageDetector {
    DETECTOR.get_or_init(|| LanguageDetectorBuilder::from_all_languages().build())
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
