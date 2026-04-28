use ragent_types::{ThinkingConfig, ThinkingDisplay, ThinkingLevel};

#[test]
fn test_thinking_level_serde_roundtrip() {
    let level: ThinkingLevel =
        serde_json::from_str(r#""medium""#).expect("level should deserialize");
    assert_eq!(level, ThinkingLevel::Medium);

    let serialized = serde_json::to_string(&ThinkingLevel::High).expect("level should serialize");
    assert_eq!(serialized, r#""high""#);
}

#[test]
fn test_thinking_level_enabled_state_matches_off_only() {
    assert!(ThinkingLevel::Auto.is_enabled());
    assert!(ThinkingLevel::Low.is_enabled());
    assert!(ThinkingLevel::Medium.is_enabled());
    assert!(ThinkingLevel::High.is_enabled());
    assert!(!ThinkingLevel::Off.is_enabled());
}

#[test]
fn test_thinking_level_default_is_auto() {
    assert_eq!(ThinkingLevel::default(), ThinkingLevel::Auto);
}

#[test]
fn test_thinking_config_default_is_auto_enabled() {
    let config = ThinkingConfig::default();

    assert!(config.enabled);
    assert_eq!(config.level, ThinkingLevel::Auto);
    assert!(config.budget_tokens.is_none());
    assert!(config.display.is_none());
    assert!(config.is_effective_enabled());
}

#[test]
fn test_thinking_config_constructors_keep_level_consistent() {
    let high = ThinkingConfig::new(ThinkingLevel::High);
    assert!(high.enabled);
    assert_eq!(high.level, ThinkingLevel::High);
    assert!(high.is_effective_enabled());

    let off = ThinkingConfig::off();
    assert!(!off.enabled);
    assert_eq!(off.level, ThinkingLevel::Off);
    assert!(!off.is_effective_enabled());
}

#[test]
fn test_thinking_config_serializes_optional_fields() {
    let config = ThinkingConfig {
        enabled: true,
        level: ThinkingLevel::Low,
        budget_tokens: Some(2048),
        display: Some(ThinkingDisplay::Summarized),
    };

    let json = serde_json::to_value(config).expect("config should serialize");
    assert_eq!(json["enabled"], true);
    assert_eq!(json["level"], "low");
    assert_eq!(json["budget_tokens"], 2048);
    assert_eq!(json["display"], "summarized");
}

#[test]
fn test_thinking_display_serde_roundtrip() {
    let display: ThinkingDisplay =
        serde_json::from_str(r#""omitted""#).expect("display should deserialize");
    assert_eq!(display, ThinkingDisplay::Omitted);

    let serialized =
        serde_json::to_string(&ThinkingDisplay::Full).expect("display should serialize");
    assert_eq!(serialized, r#""full""#);
}
