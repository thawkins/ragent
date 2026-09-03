#![allow(clippy::assert_is_empty)]
//! Tests for skill model resolution helpers.

use ragent_agent::agent::{AgentInfo, ModelRef, resolve_agent};
use ragent_agent::skill::invoke::{
    SkillInvocation, parse_model_ref, resolve_forked_skill_agent, resolve_inline_skill_agent,
};
use std::sync::Arc;

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
    let base_agent: Arc<AgentInfo> =
        resolve_agent("general", &Default::default()).expect("resolve general agent");

    let resolved = resolve_inline_skill_agent(&base_agent, Some("copilot/gpt-5.4"), None, &[]);
    let model = resolved
        .model
        .as_ref()
        .expect("resolved inline agent should have a model");

    assert_eq!(model.provider_id, "copilot");
    assert_eq!(model.model_id, "gpt-5.4");
}

#[test]
fn test_resolve_inline_skill_agent_preserves_pinned_model() {
    let base_agent = {
        let mut a = AgentInfo::new("custom", "Pinned model agent");
        a.model = Some(ModelRef {
            provider_id: "openai".to_string(),
            model_id: "gpt-4.1".to_string(),
        });
        a.model_pinned = true;
        Arc::new(a)
    };

    let resolved = resolve_inline_skill_agent(&base_agent, Some("copilot/gpt-5.4"), None, &[]);
    let model = resolved
        .model
        .as_ref()
        .expect("pinned agent should retain its model");

    assert_eq!(model.provider_id, "openai");
    assert_eq!(model.model_id, "gpt-4.1");
}

#[test]
fn test_resolve_inline_skill_agent_prefers_explicit_skill_model() {
    let base_agent: Arc<AgentInfo> =
        resolve_agent("general", &Default::default()).expect("resolve general agent");

    let resolved = resolve_inline_skill_agent(
        &base_agent,
        Some("copilot/gpt-5.4"),
        Some("openai:gpt-4o"),
        &[],
    );
    let model = resolved
        .model
        .as_ref()
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
        .as_ref()
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
        .as_ref()
        .expect("explicit skill model should override inherited model");

    assert_eq!(model.provider_id, "openai");
    assert_eq!(model.model_id, "gpt-4o");
}

// On-demand skill body loading (T-009).

use ragent_agent::skill::SkillRegistry;
use std::path::Path;

#[tokio::test]
async fn test_invoke_skill_loads_body_on_demand() {
    let tmp = std::env::temp_dir().join("ragent_test_invoke_on_demand");
    let _ = std::fs::remove_dir_all(&tmp);

    let skill_dir = tmp.join(".ragent").join("skills").join("deploy");
    std::fs::create_dir_all(&skill_dir).unwrap();
    std::fs::write(
        skill_dir.join("SKILL.md"),
        "---\ndescription: Deploy app\n---\nDeploy $ARGUMENTS to production\n",
    )
    .unwrap();

    let registry = SkillRegistry::load(&tmp, &[]);
    let skill = registry
        .get("deploy")
        .expect("deploy skill should be registered")
        .clone();

    // The registry should have discovered the skill metadata but deferred the body.
    assert!(skill.body.is_empty());
    assert_eq!(skill.description.as_deref(), Some("Deploy app"));

    let result =
        ragent_agent::skill::invoke::invoke_skill(&skill, "staging", "sess-1", Path::new("/tmp"))
            .await
            .expect("invoke_skill should load the body on demand");

    assert_eq!(result.skill_name, "deploy");
    assert_eq!(result.content.trim(), "Deploy staging to production");

    let _ = std::fs::remove_dir_all(&tmp);
}

#[tokio::test]
async fn test_invoke_skill_caches_body_from_disk() {
    let tmp = std::env::temp_dir().join("ragent_test_invoke_cache");
    let _ = std::fs::remove_dir_all(&tmp);

    let skill_dir = tmp.join(".ragent").join("skills").join("deploy");
    std::fs::create_dir_all(&skill_dir).unwrap();
    let skill_md = skill_dir.join("SKILL.md");
    std::fs::write(
        &skill_md,
        "---\ndescription: Deploy app\n---\nDeploy $ARGUMENTS to production\n",
    )
    .unwrap();

    let registry = SkillRegistry::load(&tmp, &[]);
    let skill = registry
        .get("deploy")
        .expect("deploy skill should be registered")
        .clone();

    let first =
        ragent_agent::skill::invoke::invoke_skill(&skill, "staging", "sess-1", Path::new("/tmp"))
            .await
            .expect("first invocation should load body");
    assert_eq!(first.content.trim(), "Deploy staging to production");

    // Tamper with the on-disk body. A cached invocation must return the original content.
    std::fs::write(
        &skill_md,
        "---\ndescription: Deploy app\n---\nTAMPERED $ARGUMENTS content\n",
    )
    .unwrap();

    let second =
        ragent_agent::skill::invoke::invoke_skill(&skill, "staging", "sess-1", Path::new("/tmp"))
            .await
            .expect("second invocation should use cached body");
    assert_eq!(second.content.trim(), "Deploy staging to production");

    let _ = std::fs::remove_dir_all(&tmp);
}
