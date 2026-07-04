//! Integration tests for `ragent-team` teammate model override logic.
//!
//! Relocated from the inline `#[cfg(test)]` module in `src/team/manager.rs`
//! (T-006 of the testconsolidate spec). The helper `apply_teammate_model_override`
//! was widened to `pub(crate)` (FR-007) and re-exported from `ragent_team::`
//! so this external test file can reach it.

use ragent_agent::agent::{AgentInfo, ModelRef};
use ragent_team::apply_teammate_model_override;

#[test]
fn test_teammate_model_takes_priority_over_lead() {
    let mut agent = AgentInfo::new("explore", "test");
    agent.model = Some(ModelRef {
        provider_id: "anthropic".to_string(),
        model_id: "claude-3-5-haiku-latest".to_string(),
    });

    let teammate = ModelRef {
        provider_id: "openai".to_string(),
        model_id: "gpt-4o".to_string(),
    };
    let lead = ModelRef {
        provider_id: "copilot".to_string(),
        model_id: "claude-sonnet-4.6".to_string(),
    };
    apply_teammate_model_override(&mut agent, Some(&teammate), Some(&lead));

    let model = agent.model.expect("model set");
    assert_eq!(model.provider_id, "openai");
    assert_eq!(model.model_id, "gpt-4o");
}

#[test]
fn test_lead_model_used_when_no_teammate_model() {
    let mut agent = AgentInfo::new("explore", "test");
    agent.model = Some(ModelRef {
        provider_id: "anthropic".to_string(),
        model_id: "claude-3-5-haiku-latest".to_string(),
    });

    let lead = ModelRef {
        provider_id: "copilot".to_string(),
        model_id: "claude-sonnet-4.6".to_string(),
    };
    apply_teammate_model_override(&mut agent, None, Some(&lead));

    let model = agent.model.expect("model set");
    assert_eq!(model.provider_id, "copilot");
    assert_eq!(model.model_id, "claude-sonnet-4.6");
}

#[test]
fn test_agent_default_preserved_when_no_overrides() {
    let mut agent = AgentInfo::new("explore", "test");
    agent.model = Some(ModelRef {
        provider_id: "anthropic".to_string(),
        model_id: "claude-3-5-haiku-latest".to_string(),
    });

    apply_teammate_model_override(&mut agent, None, None);

    let model = agent.model.expect("model set");
    assert_eq!(model.provider_id, "anthropic");
    assert_eq!(model.model_id, "claude-3-5-haiku-latest");
}
