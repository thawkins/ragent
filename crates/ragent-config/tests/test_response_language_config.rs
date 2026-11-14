//! Tests for the `response_language` config field.
//!
//! The field names the language the model must answer in. It is a value (not an
//! additive list), so a project-level setting overrides the global one, and it
//! must round-trip through JSON without appearing in a serialised config that
//! never set it.

use ragent_config::Config;

#[test]
fn test_response_language_defaults_to_none() {
    assert!(
        Config::default().response_language.is_none(),
        "an unconfigured language must stay unset"
    );
}

#[test]
fn test_response_language_parses_from_json() {
    let config: Config =
        serde_json::from_str(r#"{"response_language": "English"}"#).expect("parse");
    assert_eq!(config.response_language.as_deref(), Some("English"));
}

#[test]
fn test_response_language_is_omitted_when_unset() {
    // An unset field must not be written back, so a save does not litter every
    // user's config with a null nobody asked for.
    let json = serde_json::to_string(&Config::default()).expect("serialize");
    assert!(
        !json.contains("response_language"),
        "an unset language must not be serialized: {json}"
    );
}

#[test]
fn test_response_language_round_trips() {
    let config: Config = serde_json::from_str(r#"{"response_language": "French"}"#).expect("parse");
    let json = serde_json::to_string(&config).expect("serialize");
    let back: Config = serde_json::from_str(&json).expect("parse again");
    assert_eq!(back.response_language.as_deref(), Some("French"));
}

#[test]
fn test_project_response_language_overrides_global() {
    let base: Config =
        serde_json::from_str(r#"{"response_language": "English"}"#).expect("parse base");
    let overlay: Config =
        serde_json::from_str(r#"{"response_language": "French"}"#).expect("parse overlay");
    let merged = Config::merge_project(base, overlay);
    assert_eq!(
        merged.response_language.as_deref(),
        Some("French"),
        "a project value wins outright (it is a value, not an additive list)"
    );
}

#[test]
fn test_project_without_response_language_keeps_global() {
    let base: Config =
        serde_json::from_str(r#"{"response_language": "English"}"#).expect("parse base");
    let overlay: Config = serde_json::from_str("{}").expect("parse overlay");
    let merged = Config::merge_project(base, overlay);
    assert_eq!(
        merged.response_language.as_deref(),
        Some("English"),
        "an overlay that says nothing must not clear the global value"
    );
}
