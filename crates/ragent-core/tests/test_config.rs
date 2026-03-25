//! Tests for test_config.rs

use ragent_core::config::Config;

#[test]
fn test_config_default_values() {
    // Serde deserialization of an empty object applies field defaults
    let config: Config = serde_json::from_str("{}").unwrap();
    assert_eq!(config.default_agent, "general");
    assert!(config.username.is_none());
    assert!(config.provider.is_empty());
    assert!(config.permission.is_empty());
    assert!(config.agent.is_empty());
    assert!(config.command.is_empty());
    assert!(config.mcp.is_empty());
    assert!(config.instructions.is_empty());
    assert!(!config.experimental.open_telemetry);
}

#[test]
fn test_config_merge_preserves_base() {
    // Build base via serde so default_agent gets the serde default "general"
    let mut base: Config = serde_json::from_str("{}").unwrap();
    base.username = Some("alice".to_string());
    base.instructions = vec!["be concise".to_string()];

    // Overlay that does NOT set username
    let overlay: Config = serde_json::from_str(r#"{"instructions": ["use tools"]}"#).unwrap();

    let merged = Config::merge(base, overlay);
    // Base username preserved because overlay didn't set it
    assert_eq!(merged.username, Some("alice".to_string()));
    // Default agent unchanged
    assert_eq!(merged.default_agent, "general");
    // Instructions appended
    assert_eq!(merged.instructions, vec!["be concise", "use tools"]);
}
