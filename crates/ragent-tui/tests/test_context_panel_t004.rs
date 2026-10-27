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
