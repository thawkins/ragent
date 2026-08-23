//! Tests for agent/config thinking fallback precedence.

use ragent_agent::Config;
use ragent_agent::agent::{AgentInfo, ModelRef, apply_fallback_thinking, resolve_agent};
use ragent_agent::provider::create_default_registry;
use ragent_types::{ThinkingConfig, ThinkingLevel};
use std::sync::Arc;

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

#[test]
fn test_resolve_agent_returns_shared_arc_for_builtin_without_overrides() {
    let config = Config::default();
    let a1 = resolve_agent("general", &config).expect("resolve general");
    let a2 = resolve_agent("general", &config).expect("resolve general again");
    // Both resolutions should point to the same shared built-in definition when
    // there are no config overrides (FR-005 / FR-013).
    assert!(
        Arc::ptr_eq(&a1, &a2),
        "resolve_agent should reuse the same Arc for built-ins"
    );
}

#[test]
fn test_resolve_agent_returns_distinct_arc_with_override() {
    let config: Config = serde_json::from_str(
        r#"{
            "agent": {
                "general": {
                    "temperature": 0.5
                }
            }
        }"#,
    )
    .expect("config parses");
    let a1 = resolve_agent("general", &config).expect("resolve general with override");
    let a2 = resolve_agent("general", &config).expect("resolve general with override again");
    // Config overrides force a new allocation on every call because the shared
    // built-in Arc cannot be mutated. The important thing is that the override
    // is actually applied, not that pointers are equal.
    assert!(
        !Arc::ptr_eq(&a1, &a2),
        "overridden agents must be independent allocations"
    );
    assert_eq!(a1.temperature, Some(0.5));
    assert_eq!(a2.temperature, Some(0.5));
}
