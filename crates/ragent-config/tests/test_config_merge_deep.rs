//! Deep-merge coverage for `Config::merge`: provider/model deep merge, the
//! ACP registry merge-by-id, the OpenAI enable/token merge, backend registry
//! merge-by-id, wholesale section overlays, and config-path provenance.

use ragent_config::Config;

fn parse(json: &str) -> Config {
    serde_json::from_str(json).expect("parse config")
}

#[test]
fn test_merge_provider_deep_merges_models_and_sections() {
    let base = parse(
        r#"{
            "provider": { "openrouter": {
                "thinking": { "enabled": true, "level": "low" },
                "models": { "m1": { "name": "Base M1" } }
            } }
        }"#,
    );
    let overlay = parse(
        r#"{
            "provider": { "openrouter": {
                "models": { "m1": { "thinking": { "enabled": true, "level": "high" } },
                            "m2": { "name": "Overlay M2" } }
            } }
        }"#,
    );

    let merged = Config::merge(base, overlay);
    let provider = merged.provider.get("openrouter").expect("provider");
    // The provider-level thinking default survives (overlay omitted it).
    assert!(provider.thinking.is_some());
    // m1 is deep-merged: the base name survives, the overlay thinking applies.
    let m1 = provider.models.get("m1").expect("m1");
    assert_eq!(m1.name.as_deref(), Some("Base M1"));
    assert!(m1.thinking.is_some());
    // m2 is added purely from the overlay.
    assert!(provider.models.contains_key("m2"));
}

#[test]
fn test_merge_backends_by_id_replace_and_append() {
    let base = parse(
        r#"{ "backends": [
            { "id": "box", "kind": "podman", "image": "old:1" },
            { "id": "keep", "kind": "docker" }
        ] }"#,
    );
    let overlay = parse(
        r#"{ "backends": [
            { "id": "box", "kind": "podman", "image": "new:2" },
            { "id": "added", "kind": "remote", "url": "http://host" }
        ] }"#,
    );

    let merged = Config::merge(base, overlay);
    let boxed = merged
        .backends
        .iter()
        .find(|b| b.id == "box")
        .expect("box entry");
    assert_eq!(boxed.image.as_deref(), Some("new:2"));
    assert!(merged.backends.iter().any(|b| b.id == "keep"));
    assert!(merged.backends.iter().any(|b| b.id == "added"));
    assert_eq!(
        merged.backends.iter().filter(|b| b.id == "box").count(),
        1,
        "a replaced entry must not be duplicated"
    );
}

#[test]
fn test_merge_acp_registry_extends_base_by_id() {
    let base = parse(
        r#"{ "acp": {
            "default_agent": "claude",
            "agents": { "claude": { "id": "claude", "command": "claude" } }
        } }"#,
    );
    let overlay = parse(
        r#"{ "acp": {
            "server_enabled": true,
            "server_agent": "codex",
            "agents": { "codex": { "id": "codex", "command": "codex" } }
        } }"#,
    );

    let merged = Config::merge(base, overlay);
    let acp = merged.acp.as_ref().expect("acp section");
    assert!(acp.agents.contains_key("claude"));
    assert!(acp.agents.contains_key("codex"));
    // The overlay's server_agent wins; server_enabled is enabled by the overlay.
    assert_eq!(acp.server_agent.as_deref(), Some("codex"));
    assert!(acp.server_enabled);
    assert_eq!(acp.default_agent.as_deref(), Some("claude"));
}

#[test]
fn test_merge_openai_enable_and_token() {
    let base = parse(r#"{ "openai": { "enabled": false, "token": "base-token" } }"#);
    // Overlay enables the surface and sets a new token.
    let overlay = parse(r#"{ "openai": { "enabled": true, "token": "new-token" } }"#);
    let merged = Config::merge(base, overlay);
    assert!(merged.openai_surface_enabled());
    assert_eq!(merged.openai_surface_token().as_deref(), Some("new-token"));

    // An overlay that only enables keeps the base token.
    let base = parse(r#"{ "openai": { "enabled": false, "token": "base-token" } }"#);
    let overlay = parse(r#"{ "openai": { "enabled": true } }"#);
    let merged = Config::merge(base, overlay);
    assert!(merged.openai_surface_enabled());
    assert_eq!(merged.openai_surface_token().as_deref(), Some("base-token"));

    // An overlay with only a token (surface not yet enabled) attaches the token
    // to the existing section.
    let base = parse(r#"{ "openai": { "enabled": true, "token": "base-token" } }"#);
    let overlay = parse(r#"{ "openai": { "token": "replaced" } }"#);
    let merged = Config::merge(base, overlay);
    assert_eq!(merged.openai_surface_token().as_deref(), Some("replaced"));
}

#[test]
fn test_merge_wholesale_sections_and_config_paths() {
    let mut base = Config::default();
    base.config_paths = vec![std::path::PathBuf::from("a.json")];
    let mut overlay = Config::default();
    overlay.plugins = Some(Default::default());
    overlay.connectors = Some(Default::default());
    overlay.automation = Some(Default::default());
    overlay.input_queue_capacity = Some(7);
    overlay.config_paths = vec![
        std::path::PathBuf::from("a.json"),
        std::path::PathBuf::from("b.json"),
    ];

    let merged = Config::merge(base, overlay);
    assert!(merged.plugins.is_some());
    assert!(merged.connectors.is_some());
    assert!(merged.automation.is_some());
    assert_eq!(merged.effective_input_queue_capacity(), 7);
    // a.json is not duplicated; b.json is appended.
    assert_eq!(
        merged
            .config_paths
            .iter()
            .filter(|p| p.as_os_str() == "a.json")
            .count(),
        1
    );
    assert!(
        merged
            .config_paths
            .iter()
            .any(|p| p.as_os_str() == "b.json")
    );
}

#[test]
fn test_merge_execution_backend_overlay_wins() {
    let base = parse(r#"{ "execution_backend": "local" }"#);
    let overlay = parse(r#"{ "execution_backend": "podman" }"#);
    let merged = Config::merge(base, overlay);
    assert_eq!(merged.effective_execution_backend().as_str(), "podman");
}

#[test]
fn test_merge_i18n_overlay_wins_wholesale() {
    let base = parse(r#"{ "i18n": { "enabled": true, "locale": "fr" } }"#);
    let overlay = parse(r#"{ "i18n": { "enabled": false, "locale": "de" } }"#);
    let merged = Config::merge(base, overlay);
    let i18n = merged.i18n.as_ref().expect("i18n section");
    assert!(!i18n.enabled);
    assert_eq!(i18n.locale, "de");
}

#[test]
fn test_merge_agent_and_command_maps_overlay_wins() {
    let base = parse(r#"{ "agent": { "a": { "prompt": "base" } } }"#);
    let overlay = parse(
        r#"{ "agent": { "a": { "prompt": "overlay" } },
             "command": { "hi": { "command": "echo hi", "description": "greet" } } }"#,
    );
    let merged = Config::merge(base, overlay);
    assert!(merged.agent.contains_key("a"));
    assert!(merged.command.contains_key("hi"));
}
