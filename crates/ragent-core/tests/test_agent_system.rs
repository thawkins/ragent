use ragent_core::agent::*;
use ragent_core::config::Config;

// ── Built-in agents ──────────────────────────────────────────────

#[test]
fn test_builtin_agents_all_present() {
    let agents = create_builtin_agents();
    let names: Vec<&str> = agents.iter().map(|a| a.name.as_str()).collect();

    assert!(names.contains(&"ask"), "Missing 'ask' agent");
    assert!(names.contains(&"general"), "Missing 'general' agent");
    assert!(names.contains(&"build"), "Missing 'build' agent");
    assert!(names.contains(&"plan"), "Missing 'plan' agent");
    assert!(names.contains(&"explore"), "Missing 'explore' agent");
    assert!(names.contains(&"title"), "Missing 'title' agent");
    assert!(names.contains(&"summary"), "Missing 'summary' agent");
    assert!(names.contains(&"compaction"), "Missing 'compaction' agent");
}

#[test]
fn test_builtin_agents_have_valid_fields() {
    let agents = create_builtin_agents();
    for agent in &agents {
        assert!(!agent.name.is_empty(), "Agent name should not be empty");
        assert!(
            !agent.description.is_empty(),
            "Agent '{}' should have a description",
            agent.name
        );
        assert!(
            agent.model.is_some(),
            "Agent '{}' should have a model binding",
            agent.name
        );
    }
}

#[test]
fn test_builtin_agent_modes() {
    let agents = create_builtin_agents();

    let general = agents.iter().find(|a| a.name == "general").unwrap();
    assert_eq!(general.mode, AgentMode::Primary);

    let build = agents.iter().find(|a| a.name == "build").unwrap();
    assert_eq!(build.mode, AgentMode::Subagent);

    let ask = agents.iter().find(|a| a.name == "ask").unwrap();
    assert_eq!(ask.mode, AgentMode::Primary);
}

#[test]
fn test_builtin_hidden_agents() {
    let agents = create_builtin_agents();

    let title = agents.iter().find(|a| a.name == "title").unwrap();
    assert!(title.hidden);

    let summary = agents.iter().find(|a| a.name == "summary").unwrap();
    assert!(summary.hidden);

    let general = agents.iter().find(|a| a.name == "general").unwrap();
    assert!(!general.hidden);
}

// ── Agent resolution ─────────────────────────────────────────────

#[test]
fn test_resolve_builtin_agent() {
    let config = Config::default();
    let agent = resolve_agent("general", &config).unwrap();

    assert_eq!(agent.name, "general");
    assert_eq!(agent.description, "General-purpose coding agent");
    assert!(agent.model.is_some());
    assert!(agent.prompt.is_some());
    assert_eq!(agent.max_steps, Some(50));
}

#[test]
fn test_resolve_unknown_agent_fallback() {
    let config = Config::default();
    let agent = resolve_agent("my_custom_agent", &config).unwrap();

    assert_eq!(agent.name, "my_custom_agent");
    assert!(agent.description.contains("Custom agent"));
    assert!(agent.model.is_none());
}

#[test]
fn test_resolve_agent_with_config_overrides() {
    let config: Config = serde_json::from_str(
        r#"{
            "agent": {
                "general": {
                    "model": "openai:gpt-4o",
                    "temperature": 0.5,
                    "prompt": "Custom prompt for general",
                    "max_steps": 100
                }
            }
        }"#,
    )
    .unwrap();

    let agent = resolve_agent("general", &config).unwrap();

    assert_eq!(agent.temperature, Some(0.5));
    assert_eq!(agent.prompt.as_deref(), Some("Custom prompt for general"));
    assert_eq!(agent.max_steps, Some(100));

    let model = agent.model.unwrap();
    assert_eq!(model.provider_id, "openai");
    assert_eq!(model.model_id, "gpt-4o");
}

#[test]
fn test_resolve_agent_config_permissions_override() {
    use ragent_core::permission::{PermissionAction, PermissionRule};

    let config: Config = serde_json::from_str(
        r#"{
            "agent": {
                "general": {
                    "permission": [
                        {"permission": "file:read", "pattern": "**", "action": "allow"},
                        {"permission": "file:write", "pattern": "**", "action": "deny"}
                    ]
                }
            }
        }"#,
    )
    .unwrap();

    let agent = resolve_agent("general", &config).unwrap();
    assert_eq!(agent.permission.len(), 2);
    assert_eq!(agent.permission[1].action, PermissionAction::Deny);
}

// ── AgentInfo construction ───────────────────────────────────────

#[test]
fn test_agent_info_new() {
    let agent = AgentInfo::new("test", "A test agent");

    assert_eq!(agent.name, "test");
    assert_eq!(agent.description, "A test agent");
    assert_eq!(agent.mode, AgentMode::Primary);
    assert!(!agent.hidden);
    assert!(agent.temperature.is_none());
    assert!(agent.model.is_none());
    assert!(agent.prompt.is_none());
    assert!(agent.permission.is_empty());
    assert!(agent.max_steps.is_none());
    assert!(agent.options.is_empty());
}

#[test]
fn test_agent_info_default() {
    let agent = AgentInfo::default();
    assert_eq!(agent.name, "");
    assert_eq!(agent.description, "");
}

// ── AgentMode Display ────────────────────────────────────────────

#[test]
fn test_agent_mode_display() {
    assert_eq!(AgentMode::Primary.to_string(), "primary");
    assert_eq!(AgentMode::Subagent.to_string(), "subagent");
    assert_eq!(AgentMode::All.to_string(), "all");
}

// ── System prompt building ───────────────────────────────────────

#[test]
fn test_build_system_prompt_with_tools() {
    let agent = AgentInfo {
        name: "general".into(),
        prompt: Some("You are a helpful agent.".into()),
        max_steps: Some(50),
        ..AgentInfo::default()
    };

    let prompt = build_system_prompt(
        &agent,
        std::path::Path::new("/my/project"),
        "src/\n  main.rs\n  lib.rs\n",
    );

    assert!(prompt.contains("You are a helpful agent."));
    assert!(prompt.contains("/my/project"));
    assert!(prompt.contains("main.rs"));
    assert!(prompt.contains("Guidelines"));
}

#[test]
fn test_build_system_prompt_single_step_skips_context() {
    let agent = AgentInfo {
        name: "ask".into(),
        prompt: Some("Answer questions.".into()),
        max_steps: Some(1),
        ..AgentInfo::default()
    };

    let prompt = build_system_prompt(
        &agent,
        std::path::Path::new("/project"),
        "big file tree here",
    );

    assert!(prompt.contains("Answer questions."));
    assert!(!prompt.contains("/project"), "Single-step agent should skip working dir");
    assert!(!prompt.contains("big file tree"), "Single-step agent should skip file tree");
}

#[test]
fn test_build_system_prompt_empty_file_tree() {
    let agent = AgentInfo {
        name: "general".into(),
        prompt: Some("Hello.".into()),
        max_steps: Some(50),
        ..AgentInfo::default()
    };

    let prompt = build_system_prompt(&agent, std::path::Path::new("/proj"), "");

    assert!(prompt.contains("Hello."));
    assert!(prompt.contains("/proj"));
    assert!(!prompt.contains("Project Structure"), "Empty tree should be skipped");
}

#[test]
fn test_build_system_prompt_no_prompt() {
    let agent = AgentInfo {
        name: "custom".into(),
        prompt: None,
        max_steps: Some(10),
        ..AgentInfo::default()
    };

    let prompt = build_system_prompt(&agent, std::path::Path::new("/proj"), "files");

    assert!(prompt.contains("/proj"));
    assert!(prompt.contains("files"));
}

// ── ModelRef ─────────────────────────────────────────────────────

#[test]
fn test_model_ref_serde() {
    let model_ref = ModelRef {
        provider_id: "anthropic".to_string(),
        model_id: "claude-sonnet-4-20250514".to_string(),
    };

    let json = serde_json::to_string(&model_ref).unwrap();
    let deserialized: ModelRef = serde_json::from_str(&json).unwrap();
    assert_eq!(deserialized.provider_id, "anthropic");
    assert_eq!(deserialized.model_id, "claude-sonnet-4-20250514");
}
