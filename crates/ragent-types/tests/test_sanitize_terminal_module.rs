//! Terminal-sanitizer unit tests relocated out of the inline `#[cfg(test)]`
//! block in `src/sanitize_terminal.rs` (ANTIPAT M2.5/F1). `sanitize_terminal`
//! is public API.

use std::borrow::Cow;

use ragent_types::sanitize_terminal::sanitize_terminal;
#[test]
fn strips_osc_window_title() {
    assert_eq!(sanitize_terminal("\u{1b}]0;evil\u{7}hello"), "hello");
}
#[test]
fn strips_osc52_clipboard_write() {
    let payload = "\u{1b}]52;c;aGVsbG8=\u{7}after";
    assert_eq!(sanitize_terminal(payload), "after");
}
#[test]
fn strips_csi_sequences() {
    assert_eq!(sanitize_terminal("\u{1b}[2Jclear"), "clear");
    assert_eq!(sanitize_terminal("red\u{1b}[31mtext"), "redtext");
}
#[test]
fn strips_dcs_and_apc_sequences() {
    assert_eq!(sanitize_terminal("\u{1b}Pdata\u{1b}\\tail"), "tail");
    assert_eq!(sanitize_terminal("\u{1b}X\u{1b}\\ok"), "ok");
}
#[test]
fn keeps_newline_and_tab() {
    assert_eq!(sanitize_terminal("a\nb\tc"), "a\nb\tc");
}
#[test]
fn drops_bare_control_characters() {
    assert_eq!(sanitize_terminal("a\u{7}b\u{0}c"), "abc");
    assert_eq!(sanitize_terminal("cr\rhere"), "crhere");
}
#[test]
fn drops_bidi_overrides() {
    assert_eq!(sanitize_terminal("ab\u{202e}cd"), "abcd");
    assert_eq!(sanitize_terminal("ab\u{2066}cd\u{2069}"), "abcd");
}
#[test]
fn split_escape_across_chunks_is_neutralised_per_message() {
    // The renderer calls this per message, not per streamed chunk, so an
    // ESC split across two TextDelta chunks reassembles into one string
    // before the call and is consumed here.
    let joined = format!("{}{}", "prefix\u{1b}", "[2Jdone");
    assert_eq!(sanitize_terminal(&joined), "prefixdone");
}
#[test]
fn plain_text_borrows_without_allocation() {
    assert!(matches!(
        sanitize_terminal("nothing to do here"),
        Cow::Borrowed(_)
    ));
}
