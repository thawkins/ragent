//! Tests for skill model resolution helpers.

use ragent_agent::agent::{AgentInfo, ModelRef, resolve_agent};
use ragent_agent::skill::invoke::{
    SkillInvocation, parse_model_ref, resolve_forked_skill_agent, resolve_inline_skill_agent,
};

#[test]
fn test_parse_model_ref_accepts_slash_and_colon_formats() {
    let slash = parse_model_ref("copilot/gpt-5.4").expect("slash model ref should parse");
    assert_eq!(slash.provider_id, "copilot");
    assert_eq!(slash.model_id, "gpt-5.4");

    let colon = parse_model_ref("openai:gpt-4o").expect("colon model ref should parse");
    assert_eq!(colon.provider_id, "openai");
    assert_eq!(colon.model_id, "gpt-4o");
}

#[test]
fn test_resolve_inline_skill_agent_inherits_active_model_for_unpinned_agent() {
    let base_agent = resolve_agent("general", &Default::default()).expect("resolve general agent");

    let resolved = resolve_inline_skill_agent(&base_agent, Some("copilot/gpt-5.4"), None);
    let model = resolved
        .model
        .expect("resolved inline agent should have a model");

    assert_eq!(model.provider_id, "copilot");
    assert_eq!(model.model_id, "gpt-5.4");
}

#[test]
fn test_resolve_inline_skill_agent_preserves_pinned_model() {
    let mut base_agent = AgentInfo::new("custom", "Pinned model agent");
    base_agent.model = Some(ModelRef {
        provider_id: "openai".to_string(),
        model_id: "gpt-4.1".to_string(),
    });
    base_agent.model_pinned = true;

    let resolved = resolve_inline_skill_agent(&base_agent, Some("copilot/gpt-5.4"), None);
    let model = resolved
        .model
        .expect("pinned agent should retain its model");

    assert_eq!(model.provider_id, "openai");
    assert_eq!(model.model_id, "gpt-4.1");
}

#[test]
fn test_resolve_inline_skill_agent_prefers_explicit_skill_model() {
    let base_agent = resolve_agent("general", &Default::default()).expect("resolve general agent");

    let resolved =
        resolve_inline_skill_agent(&base_agent, Some("copilot/gpt-5.4"), Some("openai:gpt-4o"));
    let model = resolved
        .model
        .expect("explicit skill model should set the inline agent model");

    assert_eq!(model.provider_id, "openai");
    assert_eq!(model.model_id, "gpt-4o");
}

#[test]
fn test_resolve_forked_skill_agent_inherits_active_model() {
    let invocation = SkillInvocation {
        skill_name: "release".to_string(),
        content: "Cut a release".to_string(),
        forked: true,
        fork_agent: None,
        model_override: None,
        allowed_tools: vec![],
    };

    let resolved = resolve_forked_skill_agent(
        &invocation,
        Some(&ModelRef {
            provider_id: "copilot".to_string(),
            model_id: "gpt-5.4".to_string(),
        }),
    )
    .expect("forked skill agent should resolve");
    let model = resolved
        .model
        .expect("forked agent should inherit the active model");

    assert_eq!(model.provider_id, "copilot");
    assert_eq!(model.model_id, "gpt-5.4");
}

#[test]
fn test_resolve_forked_skill_agent_prefers_explicit_skill_model() {
    let invocation = SkillInvocation {
        skill_name: "release".to_string(),
        content: "Cut a release".to_string(),
        forked: true,
        fork_agent: None,
        model_override: Some("openai:gpt-4o".to_string()),
        allowed_tools: vec![],
    };

    let resolved = resolve_forked_skill_agent(
        &invocation,
        Some(&ModelRef {
            provider_id: "copilot".to_string(),
            model_id: "gpt-5.4".to_string(),
        }),
    )
    .expect("forked skill agent should resolve");
    let model = resolved
        .model
        .expect("explicit skill model should override inherited model");

    assert_eq!(model.provider_id, "openai");
    assert_eq!(model.model_id, "gpt-4o");
}
