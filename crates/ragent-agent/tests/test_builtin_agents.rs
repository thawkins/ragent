//! External tests for `tests` from `crates/ragent-agent/src/agent/mod.rs`
//!
//! Relocated from the inline `#[cfg(test)]` module.

use ragent_agent::agent::{
    AgentInfo, AgentMode, build_system_prompt_with_storage, create_builtin_agents,
};
use ragent_config::permission::{Permission, PermissionAction};
use ragent_types::ThinkingConfig;
use std::path::Path;

#[test]
fn test_ask_agent_defaults_thinking_off() {
    let ask = create_builtin_agents()
        .into_iter()
        .find(|agent| agent.name == "ask")
        .expect("ask agent");

    assert_eq!(ask.thinking, Some(ThinkingConfig::off()));
    assert!(ask.options.is_empty());
}

#[test]
fn test_task_tool_family_guidance_is_injected_into_system_prompt() {
    // The system prompt must always end with the task tool family
    // guidance so the model understands the difference between
    // `agent_complete` (autonomous loop signal) and
    // `team_task_complete` (team workflow).  This guards against
    // regressions where someone removes the append at the end of
    // `build_system_prompt_with_storage`.
    let mut agent = AgentInfo::new("general", "general agent");
    agent.prompt = Some("You are a helpful assistant.".to_string());
    agent.max_steps = Some(10);

    let prompt = build_system_prompt_with_storage(
        &agent,
        Path::new("/tmp"),
        "",
        None,
        None,
        None,
        None,
        None,
        None,
    );

    assert!(
        prompt.contains("## Task Tool Family"),
        "system prompt must include the '## Task Tool Family' section, got tail: {}",
        &prompt[prompt.len().saturating_sub(2000)..]
    );
    assert!(
        prompt.contains("agent_complete") && prompt.contains("team_task_complete"),
        "system prompt must reference both agent_complete and team_task_complete"
    );
    // The guidance must call out that `agent_complete` only takes
    // `summary` and `team_task_complete` takes `team_name` + `task_id`.
    assert!(
        prompt.contains("summary") && prompt.contains("team_name") && prompt.contains("task_id"),
        "system prompt guidance must list the required parameters of each tool"
    );
}

#[test]
fn test_domain_agents_exist() {
    let agents = create_builtin_agents();
    let names: Vec<&str> = agents.iter().map(|a| a.name.as_str()).collect();

    let expected = [
        "ask",
        "general",
        "build",
        "plan",
        "explore",
        "title",
        "summary",
        "rust-coder",
        "python-coder",
        "typescript-coder",
        "fastapi-agent",
        "security-auditor",
        "test-writer",
        "documenter",
        "devops-agent",
        "database-agent",
        "frontend-agent",
    ];

    for name in &expected {
        assert!(
            names.contains(name),
            "built-in agent '{}' should exist",
            name
        );
    }

    assert_eq!(
        agents.len(),
        expected.len(),
        "built-in agent count mismatch"
    );
}

#[test]
fn test_domain_agents_are_primary() {
    let agents = create_builtin_agents();
    let primary_names = [
        "ask",
        "general",
        "rust-coder",
        "python-coder",
        "typescript-coder",
        "fastapi-agent",
        "security-auditor",
        "test-writer",
        "documenter",
        "devops-agent",
        "database-agent",
        "frontend-agent",
    ];

    for name in &primary_names {
        let agent = agents.iter().find(|a| a.name == *name).expect(name);
        assert_eq!(
            agent.mode,
            AgentMode::Primary,
            "agent '{}' should be Primary",
            name
        );
    }
}

#[test]
fn test_security_auditor_is_read_only() {
    let agents = create_builtin_agents();
    let auditor = agents
        .iter()
        .find(|a| a.name == "security-auditor")
        .expect("security-auditor agent");

    // Should have read-only permissions (no edit, no bash)
    assert!(
        auditor.permission.iter().any(|r| {
            matches!(r.action, PermissionAction::Deny)
                && matches!(r.permission, Permission::Edit | Permission::Bash)
        }),
        "security-auditor should deny edit and bash permissions"
    );
}
