//! Tests for test_config_integration.rs

use ragent_core::config::*;

// ── Config merging sequences ─────────────────────────────────────

#[test]
fn test_config_merge_multiple_overlays() {
    let base: Config = serde_json::from_str("{}").unwrap();

    let overlay1: Config =
        serde_json::from_str(r#"{"username": "alice", "instructions": ["be helpful"]}"#).unwrap();

    let overlay2: Config =
        serde_json::from_str(r#"{"username": "bob", "instructions": ["be concise"]}"#).unwrap();

    let merged = Config::merge(Config::merge(base, overlay1), overlay2);

    assert_eq!(merged.username, Some("bob".to_string()));
    assert_eq!(merged.instructions, vec!["be helpful", "be concise"]);
}

// ── Provider configs merge ───────────────────────────────────────

#[test]
fn test_config_merge_provider_configs() {
    let base: Config =
        serde_json::from_str(r#"{"provider": {"anthropic": {"env": ["ANTHROPIC_API_KEY"]}}}"#)
            .unwrap();

    let overlay: Config =
        serde_json::from_str(r#"{"provider": {"openai": {"env": ["OPENAI_API_KEY"]}}}"#).unwrap();

    let merged = Config::merge(base, overlay);
    assert!(merged.provider.contains_key("anthropic"));
    assert!(merged.provider.contains_key("openai"));
}

#[test]
fn test_config_merge_provider_overlay_wins() {
    let base: Config =
        serde_json::from_str(r#"{"provider": {"anthropic": {"env": ["OLD_KEY"]}}}"#).unwrap();

    let overlay: Config =
        serde_json::from_str(r#"{"provider": {"anthropic": {"env": ["NEW_KEY"]}}}"#).unwrap();

    let merged = Config::merge(base, overlay);
    assert_eq!(merged.provider["anthropic"].env, vec!["NEW_KEY"]);
}

// ── Permission rules append ──────────────────────────────────────

#[test]
fn test_config_merge_permissions_append() {
    let base: Config = serde_json::from_str(
        r#"{"permission": [{"permission": "read", "pattern": "**", "action": "allow"}]}"#,
    )
    .unwrap();

    let overlay: Config = serde_json::from_str(
        r#"{"permission": [{"permission": "edit", "pattern": "/tmp/**", "action": "deny"}]}"#,
    )
    .unwrap();

    let merged = Config::merge(base, overlay);
    assert_eq!(merged.permission.len(), 2);
}

// ── Instructions append ──────────────────────────────────────────

#[test]
fn test_config_merge_instructions_append() {
    let base: Config = serde_json::from_str(r#"{"instructions": ["first"]}"#).unwrap();

    let overlay: Config = serde_json::from_str(r#"{"instructions": ["second", "third"]}"#).unwrap();

    let merged = Config::merge(base, overlay);
    assert_eq!(merged.instructions, vec!["first", "second", "third"]);
}

// ── Experimental flags ───────────────────────────────────────────

#[test]
fn test_config_merge_experimental_flags() {
    let base: Config = serde_json::from_str(r"{}").unwrap();
    assert!(!base.experimental.open_telemetry);

    let overlay: Config =
        serde_json::from_str(r#"{"experimental": {"open_telemetry": true}}"#).unwrap();

    let merged = Config::merge(base, overlay);
    assert!(merged.experimental.open_telemetry);
}

#[test]
fn test_config_experimental_flag_sticky() {
    let base: Config =
        serde_json::from_str(r#"{"experimental": {"open_telemetry": true}}"#).unwrap();

    let overlay: Config = serde_json::from_str(r"{}").unwrap();
    let merged = Config::merge(base, overlay);
    // Once set to true in base, stays true even if overlay doesn't set it
    assert!(merged.experimental.open_telemetry);
}

// ── MCP config merge ─────────────────────────────────────────────

#[test]
fn test_config_merge_mcp_servers() {
    let base: Config =
        serde_json::from_str(r#"{"mcp": {"github": {"type": "stdio", "command": "gh-mcp"}}}"#)
            .unwrap();

    let overlay: Config = serde_json::from_str(
        r#"{"mcp": {"jira": {"type": "sse", "url": "http://localhost:3000"}}}"#,
    )
    .unwrap();

    let merged = Config::merge(base, overlay);
    assert!(merged.mcp.contains_key("github"));
    assert!(merged.mcp.contains_key("jira"));
}

// ── Command definitions merge ────────────────────────────────────

#[test]
fn test_config_merge_commands() {
    let base: Config = serde_json::from_str(
        r#"{"command": {"test": {"command": "cargo test", "description": "Run tests"}}}"#,
    )
    .unwrap();

    let overlay: Config = serde_json::from_str(
        r#"{"command": {"lint": {"command": "cargo clippy", "description": "Lint"}}}"#,
    )
    .unwrap();

    let merged = Config::merge(base, overlay);
    assert!(merged.command.contains_key("test"));
    assert!(merged.command.contains_key("lint"));
}

// ── Full JSON deserialization ────────────────────────────────────

#[test]
fn test_config_full_deserialization() {
    let json = r#"{
        "username": "developer",
        "default_agent": "build",
        "provider": {
            "anthropic": {
                "env": ["ANTHROPIC_API_KEY"],
                "api": {
                    "base_url": "https://api.anthropic.com",
                    "headers": {"x-custom": "value"}
                },
                "models": {
                    "claude-3": {
                        "name": "Claude 3",
                        "cost": {"input": 3.0, "output": 15.0},
                        "capabilities": {
                            "reasoning": true,
                            "streaming": true,
                            "vision": true,
                            "tool_use": true
                        }
                    }
                }
            }
        },
        "permission": [
            {"permission": "read", "pattern": "**", "action": "allow"}
        ],
        "agent": {
            "general": {
                "model": "openai:gpt-4",
                "temperature": 0.7,
                "max_steps": 100
            }
        },
        "command": {
            "test": {"command": "cargo test", "description": "Run tests"}
        },
        "mcp": {
            "server1": {"type": "stdio", "command": "mcp-server"}
        },
        "instructions": ["Follow best practices"],
        "experimental": {"open_telemetry": true}
    }"#;

    let config: Config = serde_json::from_str(json).unwrap();

    assert_eq!(config.username, Some("developer".to_string()));
    assert_eq!(config.default_agent, "build");
    assert_eq!(config.provider.len(), 1);
    assert!(config.provider.contains_key("anthropic"));
    assert_eq!(config.permission.len(), 1);
    assert_eq!(config.agent.len(), 1);
    assert_eq!(config.command.len(), 1);
    assert_eq!(config.mcp.len(), 1);
    assert_eq!(config.instructions, vec!["Follow best practices"]);
    assert!(config.experimental.open_telemetry);

    // Check nested provider config
    let anthropic = &config.provider["anthropic"];
    assert_eq!(anthropic.env, vec!["ANTHROPIC_API_KEY"]);
    assert!(anthropic.api.is_some());
    let api = anthropic.api.as_ref().unwrap();
    assert_eq!(api.base_url.as_deref(), Some("https://api.anthropic.com"));
    assert_eq!(
        api.headers.get("x-custom").map(|s| s.as_str()),
        Some("value")
    );

    // Check model config
    let claude3 = &anthropic.models["claude-3"];
    assert_eq!(claude3.name.as_deref(), Some("Claude 3"));
    let cost = claude3.cost.as_ref().unwrap();
    assert!((cost.input - 3.0).abs() < f64::EPSILON);
    assert!((cost.output - 15.0).abs() < f64::EPSILON);
    let caps = claude3.capabilities.as_ref().unwrap();
    assert!(caps.reasoning);
    assert!(caps.vision);
}

// ── Capabilities defaults ────────────────────────────────────────

#[test]
fn test_capabilities_defaults() {
    let caps = Capabilities::default();
    assert!(!caps.reasoning);
    assert!(caps.streaming);
    assert!(!caps.vision);
    assert!(caps.tool_use);
}

// ── McpTransport serde ───────────────────────────────────────────

#[test]
fn test_mcp_transport_serde() {
    for transport in &[McpTransport::Stdio, McpTransport::Sse, McpTransport::Http] {
        let json = serde_json::to_string(transport).unwrap();
        let deserialized: McpTransport = serde_json::from_str(&json).unwrap();
        assert_eq!(&deserialized, transport);
    }
}

// ── Default agent name ──────────────────────────────────────────

#[test]
fn test_config_default_agent_name() {
    let config: Config = serde_json::from_str("{}").unwrap();
    assert_eq!(config.default_agent, "general");
}

// ── Agent config merge ───────────────────────────────────────────

#[test]
fn test_config_merge_agent_configs() {
    let base: Config =
        serde_json::from_str(r#"{"agent": {"general": {"temperature": 0.5}}}"#).unwrap();

    let overlay: Config = serde_json::from_str(
        r#"{"agent": {"general": {"temperature": 0.8}, "build": {"max_steps": 20}}}"#,
    )
    .unwrap();

    let merged = Config::merge(base, overlay);
    assert_eq!(merged.agent["general"].temperature, Some(0.8));
    assert!(merged.agent.contains_key("build"));
}

// ── Skill dirs append ────────────────────────────────────────────

#[test]
fn test_config_merge_skill_dirs_append() {
    let base: Config = serde_json::from_str(r#"{"skill_dirs": ["/home/user/skills"]}"#).unwrap();

    let overlay: Config =
        serde_json::from_str(r#"{"skill_dirs": ["/opt/team/skills", "/shared/skills"]}"#).unwrap();

    let merged = Config::merge(base, overlay);
    assert_eq!(
        merged.skill_dirs,
        vec!["/home/user/skills", "/opt/team/skills", "/shared/skills"]
    );
}

#[test]
fn test_config_skill_dirs_default_empty() {
    let config: Config = serde_json::from_str(r"{}").unwrap();
    assert!(config.skill_dirs.is_empty());
}

#[test]
fn test_config_skill_dirs_deserialize() {
    let config: Config =
        serde_json::from_str(r#"{"skill_dirs": ["/custom/skills", "relative/skills"]}"#).unwrap();
    assert_eq!(config.skill_dirs.len(), 2);
    assert_eq!(config.skill_dirs[0], "/custom/skills");
    assert_eq!(config.skill_dirs[1], "relative/skills");
}
