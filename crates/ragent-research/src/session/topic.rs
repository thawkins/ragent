//! Topic derivation — extract a concise research topic from a fetched page's
//! title and/or body text.
//!
//! These helpers were previously inline free functions in `session.rs`.
//! They are pure (no I/O, no async) and benefit from being isolated for
//! unit testing.

/// Minimum number of words a page title or body fragment must have to be
/// considered a usable research topic.
const MIN_TOPIC_WORDS: usize = 3;

/// Maximum number of characters a derived topic may span.
pub const MAX_DERIVED_TOPIC_CHARS: usize = 240;

/// Maximum number of characters a body-derived description may span.
const MAX_BODY_DESCRIPTION_CHARS: usize = 140;

/// Derive a concise but informative research topic from a `--from-url` page.
///
/// The `webfetch` tool already extracts a page title via `readability-rs`, so
/// the title is tried first — site names and common chrome prefixes are
/// stripped and glued-together words are split. When the title is available,
/// the first substantive sentence of the cleaned body is appended as a short
/// description (separated by an em dash) so the topic captures both *what* the
/// page is about and *why* it matters. If the title is missing or unusable,
/// the first substantive sentence of the body is used alone. Returns `None`
/// only when neither source yields usable text, so the caller can abort cleanly
/// instead of using a URL-only topic.
pub fn derive_topic_from_url_body(
    src_body: &str,
    src_title: &str,
    _src_url: &str,
) -> Option<String> {
    let title_topic = clean_site_title(src_title);
    let body_description = derive_topic_description(src_body, title_topic.as_deref());
    match (title_topic, body_description) {
        (Some(title), Some(desc)) => {
            let combined = format!("{title} — {desc}");
            Some(truncate_at_char_boundary(
                &combined,
                MAX_DERIVED_TOPIC_CHARS,
            ))
        }
        (Some(title), None) => Some(title),
        (None, Some(desc)) => Some(desc),
        (None, None) => {
            let body_topic = derive_topic_from_body(src_body);
            if body_topic.is_empty() {
                None
            } else {
                Some(body_topic)
            }
        }
    }
}

/// Pick the first substantive sentence from a cleaned page body to use as the
/// research topic. This is intentionally lightweight because the `webfetch` tool
/// already runs `readability-rs` to strip nav/cookie/footer chrome. If the
/// extractor could not isolate the article text and the tool fell back to
/// html2text, this helper skips link-only lines, headings of tables of contents,
/// update banners, and other common page noise.
pub fn derive_topic_from_body(cleaned_body: &str) -> String {
    let trimmed = cleaned_body.trim();
    if trimmed.is_empty() {
        return String::new();
    }

    for raw in trimmed.split(['.', '?', '!', '\n']) {
        let fragment = raw.trim();
        if fragment.is_empty() {
            continue;
        }
        let cleaned = clean_topic_fragment(fragment);
        if cleaned.is_empty() {
            continue;
        }
        if is_topic_noise(&cleaned, fragment) {
            continue;
        }
        let word_count = cleaned.split_whitespace().count();
        // Headings ("# Introducing deep research") are valuable even when short.
        let is_heading = fragment.starts_with('#');
        if word_count >= 4 || (is_heading && word_count >= 3) {
            return truncate_at_char_boundary(&cleaned, MAX_DERIVED_TOPIC_CHARS);
        }
    }

    String::new()
}

/// Extract a short descriptive sentence from the cleaned page body to use as a
/// subtitle for a title-derived topic. The sentence must be substantive (at
/// least `MIN_BODY_DESCRIPTION_WORDS`), must not duplicate the page title, and
/// is truncated to [`MAX_BODY_DESCRIPTION_CHARS`].
pub fn derive_topic_description(cleaned_body: &str, title: Option<&str>) -> Option<String> {
    const MIN_BODY_DESCRIPTION_WORDS: usize = 6;

    let trimmed = cleaned_body.trim();
    if trimmed.is_empty() {
        return None;
    }

    let title_normalized = title.map(|t| collapse_whitespace(t).to_lowercase());

    for raw in trimmed.split(['.', '?', '!', '\n']) {
        let fragment = raw.trim();
        if fragment.is_empty() {
            continue;
        }
        let cleaned = clean_topic_fragment(fragment);
        if cleaned.is_empty() {
            continue;
        }
        if is_topic_noise(&cleaned, fragment) {
            continue;
        }
        let word_count = cleaned.split_whitespace().count();
        if word_count < MIN_BODY_DESCRIPTION_WORDS {
            continue;
        }
        // Skip a sentence that is the same as (or contains/is contained by) the title.
        if let Some(ref t) = title_normalized {
            let cleaned_normalized = cleaned.to_lowercase();
            if t.starts_with(&cleaned_normalized)
                || cleaned_normalized.starts_with(t)
                || fuzzy_contains(t, &cleaned_normalized)
                || fuzzy_contains(&cleaned_normalized, t)
            {
                continue;
            }
        }
        return Some(truncate_at_char_boundary(
            &cleaned,
            MAX_BODY_DESCRIPTION_CHARS,
        ));
    }

    None
}

/// Return true when `needle` appears as a contiguous sequence of words inside
/// `haystack`, after normalising whitespace. Used to avoid appending a body
/// sentence that merely repeats the page title.
pub fn fuzzy_contains(haystack: &str, needle: &str) -> bool {
    let hay_words: Vec<&str> = haystack.split_whitespace().collect();
    let needle_words: Vec<&str> = needle.split_whitespace().collect();
    if needle_words.is_empty() || hay_words.len() < needle_words.len() {
        return false;
    }
    hay_words
        .windows(needle_words.len())
        .any(|window| window == needle_words.as_slice())
}

/// Strip markdown heading/list markers, split glued-together words, and remove
/// leading site-chrome tokens from a candidate topic fragment.
pub fn clean_topic_fragment(s: &str) -> String {
    let mut out = s.trim().to_string();
    while out.starts_with('#') {
        out = out.trim_start_matches('#').trim_start().to_string();
    }
    for prefix in ["* ", "- ", "+ "] {
        if let Some(rest) = out.strip_prefix(prefix) {
            out = rest.to_string();
        }
    }
    // Drop trailing markdown reference-link indices like "[12]".
    if let Some(idx) = out.rfind('[') {
        let tail = &out[idx..];
        if tail.ends_with(']') && tail.chars().filter(char::is_ascii_digit).count() > 0 {
            out.truncate(idx);
            out = out.trim_end().to_string();
        }
    }
    out = split_glued_words(&out);
    out = remove_topic_nav_prefixes(&out);
    collapse_whitespace(&out)
}

/// Characters used to separate article titles from site branding in HTML
/// `<title>` tags and `OpenGraph` metadata.
const TITLE_SEPARATORS: &[char] = &['|', '-', '—', '–', '/', '>', '»', '·'];

/// Clean a page title so it can be used directly as the research topic.
///
/// Splits the title on common separator characters, evaluates each segment
/// independently, and returns the longest usable segment. This handles both
/// leading site-brand tokens ("`InfoQ` Homepage Articles ...") and trailing
/// site names ("... | `InfoQ`"). Each segment has nav words, glued tokens, and
/// short/generic noise removed. Returns `None` when no segment is meaningful.
pub fn clean_site_title(title: &str) -> Option<String> {
    let mut best: Option<String> = None;
    let mut best_words = 0;

    for raw_segment in title.split(TITLE_SEPARATORS) {
        let segment = raw_segment.trim();
        if segment.is_empty() {
            continue;
        }
        if let Some(cleaned) = clean_site_title_segment(segment) {
            let words = cleaned.split_whitespace().count();
            if words > best_words {
                best_words = words;
                best = Some(cleaned);
            }
        }
    }

    best
}

/// Clean a single title segment: strip leading site nav words, drop a leading
/// site-brand token when it is followed by a nav word, split glued tokens such
/// as "`HomepageArticlesLarge`", and discard empty or short results.
fn clean_site_title_segment(title: &str) -> Option<String> {
    let mut out = title.trim().to_string();
    if out.is_empty() {
        return None;
    }
    out = collapse_whitespace(&out);
    out = split_glued_words(&out);
    out = remove_topic_nav_prefixes(&out);

    // Drop a leading site-brand token when it is immediately followed by a
    // nav word ("InfoQ Homepage Articles ..." -> "Articles ..." -> etc.).
    let words: Vec<String> = out.split_whitespace().map(str::to_string).collect();
    let mut i = 0;
    while i < words.len() {
        let lower = words[i].to_lowercase();
        if TOPIC_NAV_PREFIXES.contains(&lower.as_str()) {
            i += 1;
            continue;
        }
        if i + 1 < words.len() && words[i].starts_with(|c: char| c.is_uppercase()) {
            let next_lower = words[i + 1].to_lowercase();
            if TOPIC_NAV_PREFIXES.contains(&next_lower.as_str()) {
                i += 1;
                continue;
            }
        }
        break;
    }
    let kept: Vec<&str> = words[i..].iter().map(std::string::String::as_str).collect();
    out = kept.join(" ");

    out = collapse_whitespace(&out);
    if out.is_empty() || is_topic_noise(&out, &out) {
        return None;
    }
    if out.split_whitespace().count() < MIN_TOPIC_WORDS {
        return None;
    }
    Some(truncate_at_char_boundary(&out, MAX_DERIVED_TOPIC_CHARS))
}

/// Tokens that, when they appear at the start of a title or fragment, indicate
/// site navigation chrome rather than article content.
const TOPIC_NAV_PREFIXES: &[&str] = &[
    "home",
    "homepage",
    "articles",
    "about",
    "contact",
    "login",
    "sign in",
    "sign up",
    "menu",
    "search",
    "subscribe",
    "share",
    "sitemap",
    "rss",
    "feed",
    "privacy",
    "terms",
];

/// Remove leading nav/site tokens from a topic candidate, one pass at a time,
/// so strings like "`InfoQ` Homepage Articles Large Concept Models..." collapse
/// to "Large Concept Models...".
fn remove_topic_nav_prefixes(s: &str) -> String {
    let mut out = s.trim().to_string();
    loop {
        let lower = out.to_lowercase();
        let mut changed = false;
        for prefix in TOPIC_NAV_PREFIXES {
            if lower.starts_with(prefix) {
                let rest = &out[prefix.len()..]
                    .trim_start_matches(|c: char| !c.is_alphanumeric())
                    .trim_start();
                if rest.len() < out.len() {
                    out = rest.to_string();
                    changed = true;
                    break;
                }
            }
        }
        if !changed {
            break;
        }
    }
    out
}

/// Common page-chrome phrases that should never become a research topic.
const TOPIC_NOISE_KEYWORDS: &[&str] = &[
    "skip to main content",
    "skip to content",
    "skip navigation",
    "cookie",
    "subscribe",
    "newsletter",
    "sign in",
    "sign up",
    "log in",
    "login",
    "loading",
    "share",
    "all rights reserved",
    "footer",
    "update:",
    "table of contents",
    "try chatgpt",
    "jump to content",
    "copyright",
    "©",
    "posted",
    "updated",
    "min read",
    "minutes to read",
    "listen to this article",
    "your browser does not support",
    "audio element",
    "key takeaways",
    "like key takeaways",
];

/// Return true when a fragment is clearly page chrome rather than article prose.
fn is_topic_noise(cleaned: &str, original: &str) -> bool {
    let lower = cleaned.to_lowercase();
    // Whole-phrase chrome strings anywhere in the fragment.
    for kw in TOPIC_NOISE_KEYWORDS {
        if lower.contains(kw) {
            return true;
        }
    }
    // Leading nav words (Home, Articles, Login, ...) are almost always chrome.
    if let Some(first) = lower.split_whitespace().next()
        && TOPIC_NAV_PREFIXES.contains(&first)
    {
        return true;
    }
    // Markdown reference-link lines like "[Skip to main content][1]" or
    // "* [Foundation(opens in a new window)][7]" are nav links, not topics.
    if original.contains("][") || original.contains("](") {
        let stripped = remove_markdown_links(original);
        let remaining_words = stripped.split_whitespace().count();
        if remaining_words < 3 {
            return true;
        }
    }
    false
}

/// Split glued-together words such as "`HomepageArticlesLarge`" or "`AIReasoning`"
/// into separate tokens so topic derivation and nav-prefix removal work on the
/// individual words. Uses character-boundary heuristics instead of regex
/// look-around, which the `regex` crate does not support.
///
/// Acronyms that end with a lowercase plural suffix (e.g. "LCMs", "APIs") are
/// kept intact rather than split into "LC Ms".
pub fn split_glued_words(s: &str) -> String {
    let chars: Vec<char> = s.chars().collect();
    let mut out = String::with_capacity(s.len() + chars.len());
    for (i, c) in chars.iter().enumerate() {
        if i > 0 {
            let prev = chars[i - 1];
            let next = chars.get(i + 1).copied();
            let after_next = chars.get(i + 2).copied();
            if should_split_topic_words(prev, *c, next, after_next) {
                out.push(' ');
            }
        }
        out.push(*c);
    }
    out
}

/// Decide whether to insert a word boundary between `prev` and `curr`.
fn should_split_topic_words(
    prev: char,
    curr: char,
    next: Option<char>,
    after_next: Option<char>,
) -> bool {
    // "eA..." or "5A..." when the uppercase letter starts a new word.
    if (prev.is_lowercase() || prev.is_ascii_digit())
        && curr.is_uppercase()
        && next.is_some_and(char::is_lowercase)
    {
        return true;
    }
    // "AIReasoning" -> split before the R because it starts a capitalised word.
    // Do not split pluralised acronyms such as "LCMs" or "APIs": the split is
    // only inserted when at least two lowercase letters follow the uppercase
    // letter, i.e. a real word rather than a trailing "s" or a space/punctuation.
    if prev.is_uppercase()
        && curr.is_uppercase()
        && next.is_some_and(char::is_lowercase)
        && after_next.is_some_and(char::is_lowercase)
    {
        return true;
    }
    false
}

/// Collapse runs of whitespace into a single space and trim the result.
pub fn collapse_whitespace(s: &str) -> String {
    s.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// Remove markdown reference and inline links, leaving only the surrounding text.
fn remove_markdown_links(s: &str) -> String {
    static RE: std::sync::LazyLock<regex::Regex> = std::sync::LazyLock::new(|| {
        regex::Regex::new(r"(?s)\[[^\]]+\](?:\[[^\]]*\]|\([^)]+\))").unwrap()
    });
    RE.replace_all(s, "").into_owned()
}

/// Truncate `s` to at most `max_chars` characters on a UTF-8 char boundary.
pub fn truncate_at_char_boundary(s: &str, max_chars: usize) -> String {
    if s.chars().count() <= max_chars {
        return s.to_string();
    }
    let end = s
        .char_indices()
        .take(max_chars)
        .last()
        .map_or(s.len(), |(i, _)| i);
    s[..end].to_string()
}
