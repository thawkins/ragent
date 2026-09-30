//! HTML tag stripping helper.
//!
//! Provides [`strip_tags`] - a leaf helper that removes HTML tags from a
//! string while preserving the text content.  Previously duplicated in
//! `ragent-tools-extended/src/webfetch.rs` and
//! `ragent-research/src/web_date.rs` (see `DUPPLAN.md` Milestone F).

/// Strip HTML tags from a string, replacing tag boundaries with spaces.
///
/// Iterates over the input character-by-character, toggling an `in_tag` flag
/// on `<` and `>` characters.  Characters outside tags are preserved; a
/// space is pushed when entering a tag (on `<`) so that words separated by
/// tags do not merge (e.g. `"foo<b>bar</b>"` becomes `"foo bar"` rather than
/// `"foobar"`).
///
/// # Arguments
///
/// * `html` - The HTML string to strip tags from.
///
/// # Returns
///
/// A plain-text string with all tags removed.
#[must_use]
pub fn strip_tags(html: &str) -> String {
    let mut result = String::with_capacity(html.len());
    let mut in_tag = false;
    for ch in html.chars() {
        match ch {
            '<' => {
                in_tag = true;
                result.push(' ');
            }
            '>' => in_tag = false,
            _ if !in_tag => result.push(ch),
            _ => {}
        }
    }
    result
}

/// Decode the common HTML entities that appear in extracted text.
///
/// Handles `&amp;`, `&lt;`, `&gt;`, `&quot;`, `&#39;`, and `&nbsp;`.
#[must_use]
pub fn decode_entities(text: &str) -> String {
    text.replace("&amp;", "&")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&#39;", "'")
        .replace("&nbsp;", " ")
}

/// Strip HTML tags, collapse runs of whitespace to a single space, and decode
/// the common entities - the plain-text extraction used by the browser module
/// (see `ANTIPAT.md` M3.8).
#[must_use]
pub fn html_to_plain_text(html: &str) -> String {
    let stripped = strip_tags(html);
    let mut collapsed = String::with_capacity(stripped.len());
    let mut prev_ws = false;
    for ch in stripped.chars() {
        if ch.is_whitespace() {
            if !prev_ws {
                collapsed.push(' ');
                prev_ws = true;
            }
        } else {
            collapsed.push(ch);
            prev_ws = false;
        }
    }
    decode_entities(&collapsed)
}
