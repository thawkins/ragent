//! Tests for config-driven thinking defaults in the TUI model picker and status label.

use std::sync::{Mutex, OnceLock};

use ragent_agent::{agent, provider};
use ragent_tui::App;
use ragent_tui::app::{ConfiguredProvider, ProviderSource};
use ragent_types::{ThinkingConfig, ThinkingLevel};

#[path = "support/mod.rs"]
mod support;

struct CwdGuard(std::path::PathBuf);

impl Drop for CwdGuard {
    fn drop(&mut self) {
        let _ = std::env::set_current_dir(&self.0);
    }
}

fn cwd_test_lock() -> &'static Mutex<()> {
    static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    LOCK.get_or_init(|| Mutex::new(()))
}

fn enter_temp_config_dir() -> tempfile::TempDir {
    let temp = tempfile::tempdir().expect("tempdir");
    std::env::set_current_dir(temp.path()).expect("set cwd");
    std::fs::create_dir_all(temp.path().join(".ragent")).expect("create .ragent");
    temp
}

/// Seed the Anthropic discovery cache with the two models these tests assert on.
///
/// `AnthropicProvider::default_models` returns an empty catalog (models are
/// discovered at runtime from the `/v1/models` endpoint), so without seeding
/// `models_for_provider("anthropic")` / `active_model_entry()` resolve no
/// entries and the config-driven thinking overlay never applies.
fn seed_anthropic_models(app: &App) {
    let models = vec![
        provider::ModelInfo {
            id: "claude-sonnet-4-20250514".to_string(),
            provider_id: "anthropic".to_string(),
            name: "Claude Sonnet 4".to_string(),
            cost: ragent_config::Cost {
                input: 3.0,
                output: 15.0,
            },
            capabilities: ragent_config::Capabilities {
                reasoning: true,
                streaming: true,
                vision: true,
                tool_use: true,
                thinking_levels: vec![ThinkingLevel::Low, ThinkingLevel::High],
            },
            context_window: 200_000,
            max_output: Some(16_384),
            request_multiplier: None,
            thinking_config: None,
        },
        provider::ModelInfo {
            id: "claude-3-5-haiku-latest".to_string(),
            provider_id: "anthropic".to_string(),
            name: "Claude 3.5 Haiku".to_string(),
            cost: ragent_config::Cost {
                input: 0.25,
                output: 1.25,
            },
            capabilities: ragent_config::Capabilities {
                reasoning: false,
                streaming: true,
                vision: false,
                tool_use: true,
                thinking_levels: vec![ThinkingLevel::Low, ThinkingLevel::High],
            },
            context_window: 200_000,
            max_output: Some(8_192),
            request_multiplier: None,
            thinking_config: None,
        },
    ];
    let json = serde_json::to_string(&models).expect("serialize anthropic models");
    app.storage
        .set_discovered_models("anthropic", &json)
        .expect("seed anthropic discovered models");
}

#[test]
fn test_models_for_provider_applies_model_and_provider_thinking_defaults() {
    let _lock = cwd_test_lock()
        .lock()
        .unwrap_or_else(|err| err.into_inner());
    let original_cwd = std::env::current_dir().expect("current dir");
    let _guard = CwdGuard(original_cwd);
    let temp = enter_temp_config_dir();
    std::fs::write(
        temp.path().join(".ragent/ragent.json"),
        r#"{
            "provider": {
                "anthropic": {
                    "thinking": {
                        "enabled": true,
                        "level": "low"
                    },
                    "models": {
                        "claude-sonnet-4-20250514": {
                            "thinking": {
                                "enabled": true,
                                "level": "high"
                            }
                        }
                    }
                }
            }
        }"#,
    )
    .expect("write config");

    let app = support::make_app();
    seed_anthropic_models(&app);
    let models = app.models_for_provider("anthropic");

    let sonnet = models
        .iter()
        .find(|entry| entry.id == "claude-sonnet-4-20250514")
        .expect("claude-sonnet-4-20250514 should exist");
    assert_eq!(
        sonnet.thinking_config,
        Some(ThinkingConfig::new(ThinkingLevel::High))
    );

    let haiku = models
        .iter()
        .find(|entry| entry.id == "claude-3-5-haiku-latest")
        .expect("claude-3-5-haiku-latest should exist");
    assert_eq!(
        haiku.thinking_config,
        Some(ThinkingConfig::new(ThinkingLevel::Low))
    );
}

#[test]
fn test_provider_label_prefers_agent_default_over_config_thinking() {
    let _lock = cwd_test_lock()
        .lock()
        .unwrap_or_else(|err| err.into_inner());
    let original_cwd = std::env::current_dir().expect("current dir");
    let _guard = CwdGuard(original_cwd);
    let temp = enter_temp_config_dir();
    std::fs::write(
        temp.path().join(".ragent/ragent.json"),
        r#"{
            "provider": {
                "anthropic": {
                    "thinking": {
                        "enabled": true,
                        "level": "high"
                    }
                }
            }
        }"#,
    )
    .expect("write config");

    let mut app = support::make_app();
    seed_anthropic_models(&app);
    app.agent_info = agent::resolve_agent("ask", &Default::default()).expect("resolve ask agent");
    // detect_provider() relies on ambient env vars / auto-discovery (e.g. gh
    // CLI tokens) which are not present in CI. Set the configured provider
    // explicitly so the test is deterministic regardless of environment.
    app.configured_provider = Some(ConfiguredProvider {
        id: "anthropic".to_string(),
        name: "Anthropic".to_string(),
        source: ProviderSource::Database,
    });
    app.selected_model = Some("anthropic/claude-sonnet-4-20250514".to_string());

    let label = app
        .provider_model_label()
        .expect("provider label should exist");
    assert!(label.contains("claude-sonnet-4-20250514"));
    assert!(label.ends_with("[thinking: off]"));
}
