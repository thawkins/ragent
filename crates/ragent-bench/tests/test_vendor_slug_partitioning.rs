//! Tests for vendor-slug model id partitioning (openrouterprov FR-017).
//!
//! Verifies that a `provider/model` string with multiple path segments is split
//! after the first segment only, so the provider id stays `openrouter` and the
//! model id keeps its full vendor slug.

use ragent_bench::model::resolve_selected_model;

#[test]
fn test_resolve_selected_model_preserves_openrouter_vendor_slug() {
    let resolved = resolve_selected_model("openrouter/anthropic/claude-sonnet-4")
        .expect("vendor-slug model should parse");
    assert_eq!(resolved.provider_id, "openrouter");
    assert_eq!(resolved.model_id, "anthropic/claude-sonnet-4");
}

#[test]
fn test_resolve_selected_model_single_segment_model() {
    let resolved = resolve_selected_model("anthropic/claude-sonnet-4-20250514")
        .expect("single-segment model should parse");
    assert_eq!(resolved.provider_id, "anthropic");
    assert_eq!(resolved.model_id, "claude-sonnet-4-20250514");
}

#[test]
fn test_resolve_selected_model_requires_separator() {
    let err =
        resolve_selected_model("claude-sonnet-4-20250514").expect_err("bare model id must fail");
    assert!(err.to_string().contains("provider/model"));
}
