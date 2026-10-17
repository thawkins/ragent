//! Coverage for the smaller config sections: the LLM security analyzer,
//! the trigger system, the automation service, OpenAI-surface defaults, and
//! the plugin/connector store accessors.

use ragent_config::automation::{
    AutomationConfig, AutomationDefinition, AutomationTriggerKind, DispatchTarget,
};
use ragent_config::openai::OpenAiConfig;
use ragent_config::trigger::{McpNotificationMode, TriggerConfig};
use ragent_config::{Config, SecurityAnalyzerConfig};

#[test]
fn test_security_analyzer_default_is_disabled() {
    let cfg = SecurityAnalyzerConfig::default();
    assert!(!cfg.enabled);
    assert!(cfg.model.is_none());
    assert_eq!(cfg.timeout_secs, 20);
    assert!(cfg.is_default());
}

#[test]
fn test_security_analyzer_parses_and_is_not_default() {
    let cfg: SecurityAnalyzerConfig = serde_json::from_str(
        r#"{ "enabled": true, "model": { "provider_id": "ollama", "model_id": "qwen2.5:7b" }, "timeout_secs": 5 }"#,
    )
    .expect("parse analyzer config");
    assert!(cfg.enabled);
    assert_eq!(cfg.timeout_secs, 5);
    let model = cfg.model.as_ref().expect("model");
    assert_eq!(model.provider_id, "ollama");
    assert_eq!(model.model_id, "qwen2.5:7b");
    assert!(!cfg.is_default());

    // An empty object parses to the defaults via `#[serde(default)]`.
    let empty: SecurityAnalyzerConfig = serde_json::from_str("{}").expect("parse empty");
    assert_eq!(empty, SecurityAnalyzerConfig::default());
}

#[test]
fn test_trigger_config_defaults_and_helpers() {
    let cfg = TriggerConfig::default();
    assert!(cfg.is_enabled());
    assert!(cfg.is_empty());
    assert_eq!(cfg.poll_interval().as_secs(), 30);
    assert_eq!(cfg.max_rules, 32);

    let cfg: TriggerConfig =
        serde_json::from_str(r#"{ "enabled": false, "poll_interval_secs": 5, "max_rules": 1 }"#)
            .expect("parse trigger");
    assert!(!cfg.is_enabled());
    assert!(!cfg.is_empty());
    assert_eq!(cfg.poll_interval().as_secs(), 5);
    assert_eq!(cfg.max_rules, 1);
}

#[test]
fn test_mcp_notification_mode_helpers_and_serde() {
    assert!(McpNotificationMode::None.is_none());
    assert!(McpNotificationMode::InjectSummary.is_inject_summary());
    assert!(McpNotificationMode::InjectAndRun.is_inject_and_run());
    assert!(!McpNotificationMode::None.is_inject_summary());

    assert_eq!(
        serde_json::to_string(&McpNotificationMode::InjectAndRun).expect("serialise"),
        "\"inject_and_run\""
    );
    let back: McpNotificationMode =
        serde_json::from_str("\"inject_summary\"").expect("deserialise");
    assert_eq!(back, McpNotificationMode::InjectSummary);
    assert_eq!(McpNotificationMode::default(), McpNotificationMode::None);
}

#[test]
fn test_automation_config_helpers() {
    let cfg = AutomationConfig::default();
    assert!(cfg.is_enabled());
    assert_eq!(cfg.run_history_cap, 500);
    assert_eq!(cfg.scheduler_tick_secs, 30);
    assert!(cfg.enabled_definitions().next().is_none());

    let cfg: AutomationConfig = serde_json::from_str(
        r#"{ "automations": [
            { "id": "a", "prompt": "p", "trigger": { "kind": "webhook" } }
        ] }"#,
    )
    .expect("parse automations");
    // `get` is case-insensitive.
    assert!(cfg.get("A").is_some());
    assert!(cfg.get("missing").is_none());
    assert_eq!(cfg.enabled_definitions().count(), 1);
    let def = cfg.get("a").expect("automation");
    assert_eq!(def.display_name(), "a");
    assert_eq!(def.backend_label(), "local");
}

#[test]
fn test_automation_definition_name_and_dispatch() {
    let def: AutomationDefinition = serde_json::from_str(
        r#"{ "id": "nightly", "name": "Nightly", "backend": "podman",
             "dispatch": [ { "kind": " Slack " } ],
             "trigger": { "kind": "schedule", "schedule": "every 2m" } }"#,
    )
    .expect("parse definition");
    assert_eq!(def.display_name(), "Nightly");
    assert_eq!(def.backend_label(), "podman");
    assert_eq!(def.trigger.label(), "schedule");
    assert_eq!(def.trigger.schedule(), Some("every 2m"));
    let target = DispatchTarget {
        kind: def.dispatch[0].kind.clone(),
        url: None,
        token_env: None,
        target: None,
    };
    assert_eq!(target.kind_lower(), "slack");

    let webhook = AutomationTriggerKind::Webhook;
    assert_eq!(webhook.label(), "webhook");
    assert!(webhook.schedule().is_none());
}

#[test]
fn test_openai_config_is_default() {
    assert!(OpenAiConfig::default().is_default());
    let enabled = OpenAiConfig {
        enabled: true,
        token: None,
    };
    assert!(!enabled.is_default());
    let tokened = OpenAiConfig {
        enabled: false,
        token: Some("sk-x".to_string()),
    };
    assert!(!tokened.is_default());
}

#[test]
fn test_plugins_and_connectors_store_accessors() {
    let mut plugins = ragent_config::PluginsConfig::default();
    assert!(plugins.is_enabled());
    // Absent stores block -> compiled default.
    assert!(plugins.stores.is_none());
    let _ = plugins.stores_or_default();
    // `stores_mut` creates the block.
    let _ = plugins.stores_mut();
    assert!(plugins.stores.is_some());

    let mut connectors = ragent_config::ConnectorsConfig::default();
    assert!(connectors.is_enabled());
    assert!(connectors.catalogue_url("anything").is_none());
    let _ = connectors.stores_or_default();
    let _ = connectors.stores_mut();
    assert!(connectors.stores.is_some());
}

#[test]
fn test_config_top_level_openai_and_security_analyzer_round_trip() {
    let config: Config = serde_json::from_str(
        r#"{
            "openai": { "enabled": true, "token": "sk-t" },
            "security_analyzer": { "enabled": true, "timeout_secs": 9 }
        }"#,
    )
    .expect("parse config");
    assert!(config.openai_surface_enabled());
    assert_eq!(config.openai_surface_token().as_deref(), Some("sk-t"));
    let analyzer = config.security_analyzer.expect("analyzer section");
    assert_eq!(analyzer.timeout_secs, 9);
}

#[test]
fn test_config_automation_accessors() {
    let config: Config =
        serde_json::from_str(r#"{ "automation": { "enabled": true, "automations": [] } }"#)
            .expect("parse config");
    assert!(config.automation_enabled());
    assert!(config.automation_config().is_enabled());

    // Absent block -> disabled and the compiled default config.
    let config = Config::default();
    assert!(!config.automation_enabled());
    assert!(config.automation_config().is_enabled());
}
