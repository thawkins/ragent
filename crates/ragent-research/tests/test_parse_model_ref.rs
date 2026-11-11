//! Tests for `parse_model_ref`, the shared `provider:model` /
//! `provider/model` parser for research model overrides (#55).

use ragent_research::parse_model_ref;

#[test]
fn test_parse_model_ref_accepts_documented_colon_form() {
    assert_eq!(parse_model_ref("ollama:phi4"), Some(("ollama", "phi4")));
}

#[test]
fn test_parse_model_ref_accepts_slash_form() {
    assert_eq!(
        parse_model_ref("openai/gpt-4.1-nano"),
        Some(("openai", "gpt-4.1-nano"))
    );
}

#[test]
fn test_parse_model_ref_slash_keeps_model_tag() {
    assert_eq!(
        parse_model_ref("ollama/phi4:latest"),
        Some(("ollama", "phi4:latest"))
    );
}

#[test]
fn test_parse_model_ref_colon_keeps_model_tag() {
    assert_eq!(
        parse_model_ref("ollama:phi4:latest"),
        Some(("ollama", "phi4:latest"))
    );
}

#[test]
fn test_parse_model_ref_rejects_missing_parts() {
    assert_eq!(parse_model_ref("phi4"), None);
    assert_eq!(parse_model_ref(":phi4"), None);
    assert_eq!(parse_model_ref("ollama/"), None);
}
