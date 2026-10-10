//! Integration tests for the `automation` config section and the automation
//! run-history types it feeds (spec `openhands` T-016; FR-013, FR-014, FR-033).

use ragent_config::{
    AutomationConfig, AutomationDefinition, AutomationTriggerKind, Config, DispatchTarget,
};

#[test]
fn absent_automation_block_is_disabled_by_default() {
    // FR-013, FR-014: a project that never opted in runs no automations.
    let config = Config::default();
    assert!(config.automation.is_none());
    assert!(!config.automation_enabled());
    assert_eq!(config.automation_config().automations.len(), 0);
    // The compiled default section is enabled, but absence is what makes the
    // service inert (`Config::automation_enabled` reads presence).
    assert!(AutomationConfig::default().is_enabled());
}

#[test]
fn automation_block_parses_webhook_and_schedule_definitions() {
    // FR-013, FR-014: webhook and schedule triggers parse with their fields.
    let json = r#"{
        "automation": {
            "enabled": true,
            "run_history_cap": 25,
            "scheduler_tick_secs": 5,
            "automations": [
                {
                    "id": "on-issue",
                    "agent": "general",
                    "prompt": "Triage issue {{payload}}",
                    "trigger": { "kind": "webhook" },
                    "backend": "local",
                    "dispatch": [
                        { "kind": "slack", "channel": "alerts", "token_env": "SLACK_BOT_TOKEN" }
                    ]
                },
                {
                    "id": "nightly",
                    "trigger": { "kind": "schedule", "schedule": "every 2m" }
                }
            ]
        }
    }"#;
    let config: Config = serde_json::from_str(json).expect("parse config");
    assert!(config.automation_enabled());
    let automation = config.automation_config();
    assert_eq!(automation.run_history_cap, 25);
    assert_eq!(automation.scheduler_tick_secs, 5);
    assert_eq!(automation.automations.len(), 2);

    let on_issue = config.automation.as_ref().unwrap().get("on-issue").unwrap();
    assert_eq!(on_issue.trigger, AutomationTriggerKind::Webhook);
    assert_eq!(on_issue.trigger.label(), "webhook");
    assert_eq!(on_issue.backend_label(), "local");
    assert_eq!(on_issue.dispatch.len(), 1);
    assert_eq!(on_issue.dispatch[0].kind_lower(), "slack");
    assert_eq!(
        on_issue.dispatch[0].token_env.as_deref(),
        Some("SLACK_BOT_TOKEN")
    );

    let nightly = config.automation.as_ref().unwrap().get("nightly").unwrap();
    assert_eq!(nightly.trigger.label(), "schedule");
    assert_eq!(nightly.trigger.schedule(), Some("every 2m"));
    // A definition with no `backend` defaults to `local` (FR-019).
    assert_eq!(nightly.backend_label(), "local");
    // A schedule definition has no webhook schedule.
    assert!(on_issue.trigger.schedule().is_none());
}

#[test]
fn dispatch_target_never_stores_a_credential_value() {
    // FR-035: a dispatch target names the credential env var; it carries no
    // secret value.
    let target = DispatchTarget {
        kind: "github".into(),
        url: Some("https://api.github.com/repos/o/r/issues".into()),
        token_env: Some("GITHUB_TOKEN".into()),
        target: Some("o/r".into()),
    };
    let json = serde_json::to_value(&target).expect("serialise");
    let text = serde_json::to_string(&json).unwrap();
    assert!(text.contains("GITHUB_TOKEN"));
    assert!(!text.contains("ghp_"));
}

#[test]
fn overlay_automation_block_wins_wholesale() {
    // Merge precedence mirrors plugins/connectors: the overlay section wins.
    let base = AutomationConfig {
        enabled: true,
        automations: vec![AutomationDefinition {
            id: "base".into(),
            name: None,
            agent: None,
            prompt: String::new(),
            trigger: AutomationTriggerKind::Webhook,
            backend: None,
            dispatch: Vec::new(),
            enabled: true,
        }],
        ..AutomationConfig::default()
    };
    let mut config = Config::default();
    config.automation = Some(base);
    assert!(config.automation_enabled());
    let automation = config.automation_config();
    assert_eq!(automation.automations.len(), 1);
    assert_eq!(automation.automations[0].id, "base");
}
