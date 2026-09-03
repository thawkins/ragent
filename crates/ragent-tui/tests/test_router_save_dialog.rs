//! Tests for the Model Router save confirmation dialog.

use std::collections::HashMap;

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ragent_llm::providers::router_config::{RouterConfig, Tier};
use ragent_tui::app::{ConfiguredProvider, ProviderSetupStep};

#[path = "support/mod.rs"]
mod support;

#[test]
fn test_router_save_confirm_persists_and_activates() {
    let mut app = support::make_app();
    app.storage
        .set_provider_auth("anthropic", "sk-test")
        .expect("store anthropic key");
    let providers = vec![ConfiguredProvider {
        id: "anthropic".to_string(),
        name: "Anthropic (Claude)".to_string(),
        source: ragent_tui::app::ProviderSource::Database,
    }];
    app.provider_setup = Some(ProviderSetupStep::SetupRouter {
        providers,
        selected_provider_ids: vec!["anthropic".to_string()],
        selected_provider_index: 0,
        draft_config: {
            let mut cfg = RouterConfig {
                enabled: false,
                tiers: HashMap::new(),
                ..RouterConfig::default()
            };
            cfg.tiers.insert(
                "SIMPLE".to_string(),
                ragent_llm::providers::router_config::TierConfig {
                    models: vec![ragent_llm::providers::router_config::TierEntry {
                        provider: "anthropic".to_string(),
                        model: "claude-sonnet-4-20250514".to_string(),
                    }],
                    timeout_ms: None,
                },
            );
            cfg
        },
        active_bucket: Tier::Simple,
        active_bucket_index: 0,
        left_pane_focused: true,
        error: None,
    });
    let config_path = std::env::temp_dir().join(format!(
        "ragent-router-save-confirm-{}.json",
        std::process::id()
    ));
    app.config_paths = vec![config_path.clone()];

    // Press Ctrl+S to trigger the save confirmation modal.
    app.handle_key_event(KeyEvent::new(KeyCode::Char('s'), KeyModifiers::CONTROL));

    assert!(
        app.pending_router_save.is_some(),
        "save confirmation should be pending"
    );
    assert!(
        matches!(
            app.provider_setup,
            Some(ProviderSetupStep::SetupRouter { .. })
        ),
        "setup dialog should stay open behind the modal"
    );

    // Confirm with Enter.
    app.handle_key_event(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));

    assert!(
        app.pending_router_save.is_none(),
        "pending save should be cleared"
    );
    assert!(app.provider_setup.is_none(), "setup dialog should close");
    assert!(app.router_enabled, "router should be enabled");
    assert_eq!(
        app.selected_model.as_deref(),
        Some("router/router"),
        "router should be active model"
    );

    let raw = std::fs::read_to_string(&config_path).expect("read saved config");
    let json: serde_json::Value = serde_json::from_str(&raw).expect("parse saved config");
    let router = json
        .get("provider")
        .and_then(|p| p.get("router"))
        .expect("provider.router present");
    assert_eq!(
        router.get("enabled").and_then(serde_json::Value::as_bool),
        Some(true)
    );
    let simple = router
        .get("tiers")
        .and_then(|t| t.get("SIMPLE"))
        .and_then(|t| t.get("models"))
        .and_then(|m| m.as_array())
        .expect("SIMPLE tier models");
    assert!(
        !simple.is_empty(),
        "SIMPLE tier must contain at least one model"
    );

    let _ = std::fs::remove_file(&config_path);
}

#[test]
fn test_router_save_cancel_does_not_persist() {
    let mut app = support::make_app();
    app.storage
        .set_provider_auth("anthropic", "sk-test")
        .expect("store anthropic key");
    let providers = vec![ConfiguredProvider {
        id: "anthropic".to_string(),
        name: "Anthropic (Claude)".to_string(),
        source: ragent_tui::app::ProviderSource::Database,
    }];
    app.provider_setup = Some(ProviderSetupStep::SetupRouter {
        providers,
        selected_provider_ids: vec!["anthropic".to_string()],
        selected_provider_index: 0,
        draft_config: {
            let mut cfg = RouterConfig {
                enabled: false,
                tiers: HashMap::new(),
                ..RouterConfig::default()
            };
            cfg.tiers.insert(
                "SIMPLE".to_string(),
                ragent_llm::providers::router_config::TierConfig {
                    models: vec![ragent_llm::providers::router_config::TierEntry {
                        provider: "anthropic".to_string(),
                        model: "claude-sonnet-4-20250514".to_string(),
                    }],
                    timeout_ms: None,
                },
            );
            cfg
        },
        active_bucket: Tier::Simple,
        active_bucket_index: 0,
        left_pane_focused: true,
        error: None,
    });
    let config_path = std::env::temp_dir().join(format!(
        "ragent-router-save-cancel-{}.json",
        std::process::id()
    ));
    app.config_paths = vec![config_path.clone()];
    std::fs::write(&config_path, "{}").expect("seed empty config so read_to_string succeeds");

    // Press Ctrl+S to trigger the save confirmation modal.
    app.handle_key_event(KeyEvent::new(KeyCode::Char('s'), KeyModifiers::CONTROL));

    // Cancel with Esc.
    app.handle_key_event(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE));

    assert!(
        app.pending_router_save.is_none(),
        "pending save should be cancelled"
    );
    assert!(
        matches!(
            app.provider_setup,
            Some(ProviderSetupStep::SetupRouter { .. })
        ),
        "setup dialog should remain open"
    );
    assert!(!app.router_enabled, "router should not be enabled");

    let raw = std::fs::read_to_string(&config_path).unwrap_or_default();
    assert!(
        !raw.contains("router"),
        "config should not contain a router block"
    );

    let _ = std::fs::remove_file(&config_path);
}
