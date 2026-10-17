//! Coverage for the HTML-to-text helpers in [`ragent_types::html`].
//!
//! Pins the exact output of `strip_tags`, `decode_entities`, and the
//! combined `html_to_plain_text` (tag stripping, whitespace collapsing, and
//! entity decoding) that the browser/research extraction path relies on.

use ragent_types::html::{decode_entities, html_to_plain_text, strip_tags};

#[test]
fn strip_tags_inserts_a_space_at_each_tag_boundary() {
    assert_eq!(strip_tags("foo<b>bar</b>"), "foo bar ");
    assert_eq!(strip_tags("<p>Hi</p>"), " Hi ");
}

#[test]
fn strip_tags_preserves_text_when_no_tags_present() {
    assert_eq!(strip_tags("plain text"), "plain text");
    assert_eq!(strip_tags(""), "");
}

#[test]
fn strip_tags_ignores_content_inside_the_tag() {
    assert_eq!(strip_tags("<a href=\"x\">link</a>"), " link ");
    // A lone `<` with no closing `>` swallows the remainder.
    assert_eq!(strip_tags("a<b c"), "a ");
}

#[test]
fn decode_entities_replaces_each_known_entity() {
    assert_eq!(
        decode_entities("a &amp; b &lt; c &gt; d &quot;e&quot; &#39;f&#39;&nbsp;g"),
        "a & b < c > d \"e\" 'f' g"
    );
}

#[test]
fn decode_entities_leaves_unknown_entities_untouched() {
    assert_eq!(decode_entities("&copy; &hellip;"), "&copy; &hellip;");
    assert_eq!(decode_entities("plain"), "plain");
}

#[test]
fn html_to_plain_text_collapses_whitespace_and_decodes() {
    let out = html_to_plain_text("<p>Hello</p>\n\n   <span>world</span> &amp; friends");
    assert_eq!(out, " Hello world & friends");
}

#[test]
fn html_to_plain_text_collapses_newlines_and_tabs() {
    let out = html_to_plain_text("a\t\t\n  b");
    assert_eq!(out, "a b");
}

#[test]
fn html_to_plain_text_empty_input() {
    assert_eq!(html_to_plain_text(""), "");
}
