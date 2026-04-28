use ragent_config::Config;
use ragent_types::{ThinkingConfig, ThinkingDisplay, ThinkingLevel};

#[test]
fn test_capabilities_default_to_no_thinking_levels() {
    let config = Config::default();
    let capabilities = ragent_config::Capabilities::default();

    assert!(capabilities.thinking_levels.is_empty());
    assert!(config.provider.is_empty());
}

#[test]
fn test_model_config_parses_thinking_block_and_levels() {
    let config: Config = serde_json::from_str(
        r#"{
            "provider": {
                "anthropic": {
                    "models": {
                        "claude-sonnet-4-20250514": {
                            "thinking": {
                                "enabled": true,
                                "level": "high",
                                "budget_tokens": 8192,
                                "display": "full"
                            },
                            "capabilities": {
                                "reasoning": true,
                                "thinking_levels": ["auto", "off", "low", "medium", "high"]
                            }
                        }
                    }
                }
            }
        }"#,
    )
    .expect("config should parse");

    let provider = config
        .provider
        .get("anthropic")
        .expect("anthropic provider should exist");
    let model = provider
        .models
        .get("claude-sonnet-4-20250514")
        .expect("model config should exist");

    assert_eq!(
        model
            .thinking
            .as_ref()
            .expect("thinking config should exist"),
        &ThinkingConfig {
            enabled: true,
            level: ThinkingLevel::High,
            budget_tokens: Some(8192),
            display: Some(ThinkingDisplay::Full),
        }
    );

    let capabilities = model
        .capabilities
        .as_ref()
        .expect("capabilities should exist");
    assert_eq!(
        capabilities.thinking_levels,
        vec![
            ThinkingLevel::Auto,
            ThinkingLevel::Off,
            ThinkingLevel::Low,
            ThinkingLevel::Medium,
            ThinkingLevel::High,
        ]
    );
}

#[test]
fn test_model_config_thinking_is_optional() {
    let config: Config = serde_json::from_str(
        r#"{
            "provider": {
                "openai": {
                    "models": {
                        "gpt-4o": {
                            "name": "GPT-4o"
                        }
                    }
                }
            }
        }"#,
    )
    .expect("config should parse without thinking");

    let model = &config.provider["openai"].models["gpt-4o"];
    assert!(model.thinking.is_none());
    assert!(model.capabilities.is_none());
}

#[test]
fn test_provider_config_parses_thinking_block() {
    let config: Config = serde_json::from_str(
        r#"{
            "provider": {
                "anthropic": {
                    "thinking": {
                        "enabled": true,
                        "level": "medium",
                        "budget_tokens": 16000
                    }
                }
            }
        }"#,
    )
    .expect("provider thinking config should parse");

    let provider = config
        .provider
        .get("anthropic")
        .expect("anthropic provider should exist");
    assert_eq!(
        provider.thinking,
        Some(ThinkingConfig {
            enabled: true,
            level: ThinkingLevel::Medium,
            budget_tokens: Some(16000),
            display: None,
        })
    );
}

#[test]
fn test_thinking_config_for_model_prefers_model_over_provider() {
    let config: Config = serde_json::from_str(
        r#"{
            "provider": {
                "anthropic": {
                    "thinking": {
                        "enabled": true,
                        "level": "low"
                    },
                    "models": {
                        "claude-sonnet-4-20250514": {
                            "thinking": {
                                "enabled": true,
                                "level": "high"
                            }
                        }
                    }
                }
            }
        }"#,
    )
    .expect("config should parse");

    assert_eq!(
        config.thinking_config_for_model("anthropic", "claude-sonnet-4-20250514"),
        Some(ThinkingConfig::new(ThinkingLevel::High))
    );
    assert_eq!(
        config.thinking_config_for_model("anthropic", "claude-3-5-haiku-latest"),
        Some(ThinkingConfig::new(ThinkingLevel::Low))
    );
}

#[test]
fn test_merge_preserves_lower_precedence_provider_model_thinking() {
    let base: Config = serde_json::from_str(
        r#"{
            "provider": {
                "anthropic": {
                    "env": ["ANTHROPIC_API_KEY"],
                    "thinking": {
                        "enabled": true,
                        "level": "low"
                    },
                    "models": {
                        "claude-sonnet-4-20250514": {
                            "thinking": {
                                "enabled": true,
                                "level": "high"
                            }
                        }
                    }
                }
            }
        }"#,
    )
    .expect("base config should parse");
    let overlay: Config = serde_json::from_str(
        r#"{
            "provider": {
                "anthropic": {
                    "api": {
                        "base_url": "https://example.invalid"
                    }
                }
            }
        }"#,
    )
    .expect("overlay config should parse");

    let merged = Config::merge(base, overlay);
    let provider = merged
        .provider
        .get("anthropic")
        .expect("anthropic provider should exist");

    assert_eq!(provider.env, vec!["ANTHROPIC_API_KEY"]);
    assert_eq!(
        provider.thinking,
        Some(ThinkingConfig::new(ThinkingLevel::Low))
    );
    assert_eq!(
        provider
            .models
            .get("claude-sonnet-4-20250514")
            .and_then(|model| model.thinking.clone()),
        Some(ThinkingConfig::new(ThinkingLevel::High))
    );
    assert_eq!(
        provider
            .api
            .as_ref()
            .and_then(|api| api.base_url.as_deref()),
        Some("https://example.invalid")
    );
}
