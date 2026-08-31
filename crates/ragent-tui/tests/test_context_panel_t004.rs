//! Tests for the Context side panel (spec `contextpanel`).
//!
//! - **T-004**: Compute toolset catalog token size.

mod support;

#[test]
fn test_tool_catalog_token_count_is_positive_with_default_registry() {
    // FR-006: the panel must be able to compute a positive token proxy for
    // the visible tool catalog. The default registry exposes many tools, so
    // the byte/char estimate is expected to be substantially larger than zero.
    let app = support::make_app();
    let count = app.tool_catalog_token_count();
    assert!(
        count > 1000,
        "tool catalog token count should be positive; got {count}"
    );
}

#[test]
fn test_tool_catalog_token_count_changes_with_hidden_tools() {
    // FR-006: hiding tools reduces the visible catalog size, so the count
    // must decrease after tools are hidden.
    let app = support::make_app();
    let full = app.tool_catalog_token_count();
    app.session_processor.tool_registry.set_hidden(&[
        "read".to_string(),
        "write".to_string(),
        "bash".to_string(),
    ]);
    let reduced = app.tool_catalog_token_count();
    assert!(
        reduced < full,
        "hiding tools should reduce catalog count: full={full}, reduced={reduced}"
    );
}
