use ragent_config::Config;

#[test]
fn test_code_index_defaults_are_enabled() {
    let config = Config::default();

    assert!(
        config.code_index.enabled,
        "code_index.enabled should default to true"
    );
    assert_eq!(config.code_index.max_file_size, 1_048_576);
    assert!(config.code_index.extra_exclude_dirs.is_empty());
    assert!(config.code_index.extra_exclude_patterns.is_empty());
}

#[test]
fn test_code_index_deserializes_partial_config() {
    let config: Config = serde_json::from_str(
        r#"{
            "code_index": {
                "max_file_size": 2048,
                "extra_exclude_dirs": ["vendor", "node_modules"]
            }
        }"#,
    )
    .unwrap();

    // Missing "enabled" should fall back to default (true)
    assert!(
        config.code_index.enabled,
        "code_index.enabled should default to true when absent from JSON"
    );
    assert_eq!(config.code_index.max_file_size, 2048);
    assert_eq!(
        config.code_index.extra_exclude_dirs,
        vec!["vendor".to_string(), "node_modules".to_string()]
    );
}

#[test]
fn test_code_index_explicit_disable() {
    let config: Config = serde_json::from_str(
        r#"{
            "code_index": {
                "enabled": false
            }
        }"#,
    )
    .unwrap();

    assert!(!config.code_index.enabled);
}

#[test]
fn test_code_index_merge_preserves_enabled_when_overlay_silent() {
    let mut base = Config::default();
    // Simulate user explicitly enabling code index
    base.code_index.enabled = true;

    let overlay: Config = serde_json::from_str(
        r#"{
            "code_index": {
                "max_file_size": 512
            }
        }"#,
    )
    .unwrap();

    let merged = Config::merge(base, overlay);

    assert!(
        merged.code_index.enabled,
        "overlay without 'enabled' should not reset code_index.enabled"
    );
    assert_eq!(merged.code_index.max_file_size, 512);
}

#[test]
fn test_config_serialise_preserves_code_index_enabled() {
    let config = Config::default();
    let json = serde_json::to_string_pretty(&config).unwrap();

    // The serialised JSON should contain "enabled": true for code_index
    assert!(
        json.contains(r#""code_index": {"#),
        "serialised config should contain code_index section"
    );

    // Round-trip: deserialise should preserve defaults
    let roundtrip: Config = serde_json::from_str(&json).unwrap();
    assert!(
        roundtrip.code_index.enabled,
        "round-tripped config should have code_index.enabled = true"
    );
    assert!(
        !roundtrip.internal_llm.enabled,
        "round-tripped config should have internal_llm.enabled = false"
    );
}

#[test]
fn test_config_defaults_are_safe() {
    let config = Config::default();

    // Code index should be ON by default
    assert!(
        config.code_index.enabled,
        "code_index.enabled should default to true — agents need code search"
    );

    // Internal LLM should be OFF by default
    assert!(
        !config.internal_llm.enabled,
        "internal_llm.enabled should default to false — requires explicit opt-in"
    );

    // Tool visibility: codeindex should be ON by default
    assert!(
        config.tool_visibility.codeindex,
        "tool_visibility.codeindex should default to true"
    );
}

#[test]
fn test_config_roundtrip_preserves_defaults() {
    let original = Config::default();
    let json = serde_json::to_string(&original).unwrap();
    let restored: Config = serde_json::from_str(&json).unwrap();

    assert_eq!(restored.code_index.enabled, original.code_index.enabled);
    assert_eq!(restored.internal_llm.enabled, original.internal_llm.enabled);
    assert_eq!(restored.tool_visibility.codeindex, original.tool_visibility.codeindex);
}

#[test]
fn test_serialised_default_omits_code_index_enabled() {
    // When serialising the default config, `code_index.enabled` (true) should
    // be omitted so that code-level default changes take effect without manual
    // config edits.
    let config = Config::default();
    let json = serde_json::to_string_pretty(&config).unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();

    let code_index = &parsed["code_index"];
    // The "enabled" key should be absent from serialised output when it equals true
    assert!(
        code_index.get("enabled").is_none(),
        "code_index.enabled should be omitted from serialised default (it is true), but found: {:?}",
        code_index.get("enabled")
    );
}

#[test]
fn test_serialised_default_omits_internal_llm_enabled() {
    // When serialising the default config, `internal_llm.enabled` (false) should
    // be omitted so that code-level default changes take effect without manual
    // config edits.
    let config = Config::default();
    let json = serde_json::to_string_pretty(&config).unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();

    let internal_llm = &parsed["internal_llm"];
    // The "enabled" key should be absent from serialised output when it equals false
    assert!(
        internal_llm.get("enabled").is_none(),
        "internal_llm.enabled should be omitted from serialised default (it is false), but found: {:?}",
        internal_llm.get("enabled")
    );
}

#[test]
fn test_serialised_tool_visibility_omits_default_codeindex() {
    // When serialising the default config, `tool_visibility.codeindex` (true) should
    // be omitted so that code-level default changes take effect.
    let config = Config::default();
    let json = serde_json::to_string_pretty(&config).unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();

    let tool_visibility = &parsed["tool_visibility"];
    assert!(
        tool_visibility.get("codeindex").is_none(),
        "tool_visibility.codeindex should be omitted from serialised default (it is true), but found: {:?}",
        tool_visibility.get("codeindex")
    );
}