//! Inline tests for `actions.rs` - compiled inside the module tree
//! via the `#[path]` delegation left in the source file
//! (ANTIPAT M2 inline-test migration; no behaviour change).

use super::*;

#[test]
fn test_html_to_text_strips_tags() {
    let html = "<html><body><h1>Title</h1><p>Hello &amp; world</p></body></html>";
    let text = html_to_text(html);
    assert!(text.contains("Title"));
    assert!(text.contains("Hello & world"));
    assert!(!text.contains('<'));
}

#[test]
fn test_html_to_text_collapses_whitespace() {
    let html = "<div>  multiple   spaces  </div>";
    let text = html_to_text(html);
    assert!(!text.contains("  "));
}

#[test]
fn test_key_code_for_enter() {
    assert_eq!(key_code_for("Enter"), "Enter");
    assert_eq!(key_code_for("Tab"), "Tab");
    assert_eq!(key_code_for(" "), "Space");
}

#[test]
fn test_virtual_key_code_for_enter() {
    assert_eq!(virtual_key_code_for("Enter"), 0x0D);
    assert_eq!(virtual_key_code_for("Tab"), 0x09);
    assert_eq!(virtual_key_code_for("Escape"), 0x1B);
}
