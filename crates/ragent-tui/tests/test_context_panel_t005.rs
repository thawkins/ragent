//! Tests for the Context side panel (spec `contextpanel`).
//!
//! - **T-005**: Compute toolset metadata/wrapper token size.

mod support;

#[test]
fn test_tool_metadata_token_count_is_positive_with_default_registry() {
    // FR-007: the provider wire envelope adds per-tool overhead beyond the
    // raw definitions, so the metadata estimate must be positive for the
    // default registry.
    let app = support::make_app();
    let count = app.tool_metadata_token_count();
    assert!(
        count > 0,
        "tool metadata token count should be positive; got {count}"
    );
}

#[test]
fn test_tool_metadata_token_count_decreases_with_hidden_tools() {
    // FR-007: wrapper overhead is per-tool, so hiding tools must shrink it.
    let app = support::make_app();
    let full = app.tool_metadata_token_count();
    app.session_processor.tool_registry.set_hidden(&[
        "read".to_string(),
        "write".to_string(),
        "bash".to_string(),
    ]);
    let reduced = app.tool_metadata_token_count();
    assert!(
        reduced < full,
        "hiding tools should reduce metadata count: full={full}, reduced={reduced}"
    );
}

#[test]
fn test_tool_metadata_is_smaller_than_catalog() {
    // FR-006/FR-007 consistency: the envelope overhead is the difference
    // between the wire bytes and the raw definitions, so it must stay below
    // the catalog size for a non-trivial tool set.
    let app = support::make_app();
    let catalog = app.tool_catalog_token_count();
    let metadata = app.tool_metadata_token_count();
    assert!(
        metadata < catalog,
        "metadata ({metadata}) should be smaller than catalog ({catalog})"
    );
}
