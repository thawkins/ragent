//! Title cleaning — strip nav chrome, markdown links, and noise from captured
//! web-source titles before they are stored on `Source::Web`.
//!
//! These helpers were previously inline free functions in `web_gatherer.rs`.

/// Maximum length enforced for a stored web-source title. Longer titles are
/// truncated at a word boundary with an ellipsis so the References Index and
/// the per-finding `**Sources:**` bullets stay readable. Captured titles come
/// from the page's readability-extracted `<title>`/heading or the search-hit
/// title and frequently contain nav chrome ("Skip to main content") or consent
/// banners ("We use essential cookies to make our site work..."); see
/// [`clean_web_source_title`].
pub(crate) const MAX_WEB_SOURCE_TITLE_CHARS: usize = 120;

/// Leading phrases that mark a captured title as page chrome rather than
/// article content. When the cleaned title starts with one of these it is
/// stripped; when the *entire* cleaned title is one of these (after markdown
/// link syntax is removed) the title is discarded in favour of the fallback.
const TITLE_NOISE_PHRASES: &[&str] = &[
    "skip to main content",
    "skip to content",
    "skip navigation",
    "skip to nav",
    "jump to content",
    "we use essential cookies",
    "we use cookies",
    "this site uses cookies",
    "agree & join",
    "agree and join",
    "sign in",
    "sign up",
    "log in",
    "join/login",
    "join sign in",
];

/// Clean a page title captured from a fetch or search hit before it is stored
/// on a [`Source::Web`], so the title shown in the References Index and the
/// per-finding `**Sources:**` bullets is short and meaningful rather than nav
/// chrome or a consent banner. This is a pure code transform — no LLM.
///
/// Steps:
/// 1. Strip markdown reference-link (`[text][n]`) and inline-link
///    (`[text](url)`) syntax, keeping the link text.
/// 2. Drop a leading nav/cookie/consent phrase from [`TITLE_NOISE_PHRASES`].
/// 3. Collapse internal whitespace and trim.
/// 4. Truncate to [`MAX_WEB_SOURCE_TITLE_CHARS`] at a word boundary with an
///    ellipsis.
/// 5. When the cleaned primary is empty (or was pure noise), repeat on
///    `fallback` (typically the search-hit title or the URL). When both are
///    empty/noise, return the raw fallback so the title is never blank.
#[must_use]
#[allow(dead_code)]
pub(crate) fn clean_web_source_title(primary: &str, fallback: &str) -> String {
    let cleaned = clean_title_text(primary);
    if !cleaned.is_empty() {
        return cleaned;
    }
    let cleaned_fallback = clean_title_text(fallback);
    if !cleaned_fallback.is_empty() {
        return cleaned_fallback;
    }
    // Both reduced to nothing — surface a non-empty raw value so the
    // References Index never shows a blank title cell.
    fallback.trim().to_string()
}

/// Strip markdown link syntax, leading nav/consent noise, collapse whitespace,
/// and truncate to [`MAX_WEB_SOURCE_TITLE_CHARS`] at a word boundary.
pub(crate) fn clean_title_text(s: &str) -> String {
    let stripped = strip_markdown_link_text(s);
    let stripped = strip_leading_noise(&stripped);
    let collapsed = collapse_title_ws(&stripped);
    truncate_title_words(&collapsed, MAX_WEB_SOURCE_TITLE_CHARS)
}

/// Replace markdown reference links (`[text][n]`, `[text][]`) and inline links
/// (`[text](url)`) with just the link `text`, leaving non-link content intact.
fn strip_markdown_link_text(s: &str) -> String {
    static RE: std::sync::LazyLock<regex::Regex> = std::sync::LazyLock::new(|| {
        // Match `[text]` immediately followed by either `[...]` or `(...)`.
        regex::Regex::new(r"\[([^\]]*)\](?:\[[^\]]*\]|\([^)]*\))").expect("title link regex")
    });
    RE.replace_all(s, "$1").into_owned()
}

/// Remove a single leading nav/cookie/consent phrase (case-insensitive) from
/// `s`, including any trailing separator punctuation. Returns `s` unchanged
/// when no noise phrase matches the start.
fn strip_leading_noise(s: &str) -> String {
    let trimmed = s.trim_start();
    let lower = trimmed.to_lowercase();
    for phrase in TITLE_NOISE_PHRASES {
        if lower.starts_with(phrase) {
            // Map the matched prefix length back to the original slice so we
            // keep the original casing of the remainder.
            let kept = &trimmed[phrase.len()..];
            let after = kept.trim_start_matches([' ', ',', ':', '|', '-', '—', '·']);
            return after.trim().to_string();
        }
    }
    trimmed.trim().to_string()
}

/// Collapse runs of whitespace into single spaces and trim the ends.
fn collapse_title_ws(s: &str) -> String {
    s.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// Truncate `s` to at most `max_chars` Unicode scalar values, cutting at the
/// last whitespace boundary at or before the limit so words are not split. An
/// ellipsis is appended when truncation occurs.
pub(crate) fn truncate_title_words(s: &str, max_chars: usize) -> String {
    if s.chars().count() <= max_chars {
        return s.to_string();
    }
    // Reserve two chars for the " …" suffix when possible.
    let budget = max_chars.saturating_sub(2);
    let mut end_byte = 0usize;
    let mut last_space_byte = 0usize;
    for (i, (byte_idx, ch)) in s.char_indices().enumerate() {
        if i >= budget {
            break;
        }
        end_byte = byte_idx + ch.len_utf8();
        if ch.is_whitespace() {
            last_space_byte = byte_idx;
        }
    }
    // Prefer to cut at the last whitespace so we don't split a word.
    let cut_byte = if last_space_byte > 0 {
        last_space_byte
    } else {
        end_byte
    };
    // Walk back to a UTF-8 char boundary (last_space_byte is already on a
    // boundary; end_byte is a char-end boundary by construction).
    let mut out = s[..cut_byte].trim_end().to_string();
    if !out.is_empty() {
        out.push('…');
    }
    out
}
