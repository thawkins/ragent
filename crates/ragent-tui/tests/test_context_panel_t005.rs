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
fn test_tool_metadata_measures_the_selected_providers_own_envelope() {
    // FR-007 / T-005: `tool_metadata_token_count` measures the wire bytes of the
    // *selected provider's own* envelope, minus the raw definition bytes. The
    // Responses envelope is 519 bytes smaller than the chat one over the default
    // registry (each item drops the nested `function` wrapper), so a Responses
    // session must report strictly *less* overhead than a chat session sent the
    // same definitions. A session measured with the wrong envelope, or one that
    // re-derives the array instead of reading the cache, does not.
    let mut app = support::make_app();
    app.agent_info.model = None;
    app.selected_model = Some("openai/gpt-5.6".to_string());
    let chat = app.tool_metadata_token_count();

    app.selected_model = Some("openai_responses/gpt-5.6".to_string());
    let responses = app.tool_metadata_token_count();

    assert!(chat > 0, "the chat envelope has per-tool overhead");
    assert!(
        responses < chat,
        "a Responses session ({responses}) must measure a smaller per-tool envelope \
         than a chat session ({chat}) over the same definitions"
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
