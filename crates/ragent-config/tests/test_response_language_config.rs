//! Tests for the `response_language` config field.
//!
//! The field names the language the model must answer in. It is a value (not an
//! additive list), so a project-level setting overrides the global one, and it
//! must round-trip through JSON. An omitted key defaults to English, so a model
//! that would otherwise drift back to its training language is constrained
//! without any setup; an explicitly blank value is the opt-out.

use ragent_config::Config;

#[test]
fn test_response_language_defaults_to_english() {
    assert_eq!(
        Config::default().response_language.as_deref(),
        Some("English"),
        "a config that never names a language must still pin English"
    );
}

#[test]
fn test_response_language_defaults_to_english_when_key_is_omitted() {
    // The serde default must agree with `Config::default()`, or a parsed config
    // and a constructed one would behave differently.
    let config: Config = serde_json::from_str("{}").expect("parse");
    assert_eq!(config.response_language.as_deref(), Some("English"));
}

#[test]
fn test_response_language_blank_is_an_opt_out() {
    // Blank is how a user who wants the model's own default back turns the
    // directive off without deleting the key.
    let config: Config = serde_json::from_str(r#"{"response_language": ""}"#).expect("parse");
    assert_eq!(config.response_language.as_deref(), Some(""));
}

#[test]
fn test_response_language_parses_from_json() {
    let config: Config =
        serde_json::from_str(r#"{"response_language": "English"}"#).expect("parse");
    assert_eq!(config.response_language.as_deref(), Some("English"));
}

#[test]
fn test_response_language_is_written_when_set() {
    // The default is now a real value, so it is serialised: a save writes the
    // language out, which is what lets a user see and change it in their config.
    let json = serde_json::to_string(&Config::default()).expect("serialize");
    assert!(
        json.contains(r#""response_language":"English""#),
        "the default language must be visible in a saved config: {json}"
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
