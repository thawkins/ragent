//! Tests for agent/config thinking fallback precedence.

use ragent_agent::agent::{AgentInfo, ModelRef, apply_fallback_thinking, resolve_agent};
use ragent_agent::config::Config;
use ragent_agent::provider::create_default_registry;
use ragent_types::{ThinkingConfig, ThinkingLevel};

#[test]
fn test_apply_fallback_thinking_uses_config_model_thinking_when_agent_has_no_default() {
    let config: Config = serde_json::from_str(
        r#"{
            "provider": {
                "copilot": {
                    "models": {
                        "claude-sonnet-4.5": {
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

    let registry = create_default_registry();
    let mut agent = AgentInfo::new("general", "General");
    agent.model = Some(ModelRef {
        provider_id: "copilot".to_string(),
        model_id: "claude-sonnet-4.5".to_string(),
    });

    apply_fallback_thinking(&mut agent, &config, &registry);
    assert_eq!(
        agent.thinking,
        Some(ThinkingConfig::new(ThinkingLevel::High))
    );
}

#[test]
fn test_apply_fallback_thinking_defaults_to_off_without_config_override() {
    let config = Config::default();
    let registry = create_default_registry();
    let mut agent = AgentInfo::new("general", "General");
    agent.model = Some(ModelRef {
        provider_id: "anthropic".to_string(),
        model_id: "claude-sonnet-4-20250514".to_string(),
    });

    apply_fallback_thinking(&mut agent, &config, &registry);
    assert_eq!(agent.thinking, Some(ThinkingConfig::off()));
}

#[test]
fn test_resolve_agent_preserves_agent_default_over_config_thinking() {
    let config: Config = serde_json::from_str(
        r#"{
            "provider": {
                "copilot": {
                    "models": {
                        "claude-sonnet-4.5": {
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

    let agent = resolve_agent("ask", &config).expect("ask agent should resolve");
    assert_eq!(agent.thinking, Some(ThinkingConfig::off()));
}
