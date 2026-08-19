//! Regression tests for the finance configuration block.

use ragent_config::Config;

#[test]
fn finance_config_defaults_to_yahoo() {
    let config = Config::default();
    assert_eq!(config.finance.provider, "yahoo");
    assert!(config.finance.api_key.is_none());
    assert!(!config.finance.is_paid_provider_configured());
}

#[test]
fn finance_config_parses_alpha_vantage() {
    let config: Config = serde_json::from_str(
        r#"{
            "finance": {
                "provider": "alpha_vantage",
                "api_key": "demo-key",
                "requests_per_minute": 25
            }
        }"#,
    )
    .expect("config should parse");

    assert_eq!(config.finance.provider, "alpha_vantage");
    assert_eq!(config.finance.api_key.as_deref(), Some("demo-key"));
    assert_eq!(config.finance.requests_per_minute, Some(25));
    assert!(config.finance.is_paid_provider_configured());
}

#[test]
fn finance_config_is_propagated_by_merge() {
    let base = Config::default();
    let mut overlay = Config::default();
    overlay.finance.provider = "alpha_vantage".to_string();
    overlay.finance.api_key = Some("demo-key".to_string());

    let merged = Config::merge(base, overlay);
    assert_eq!(merged.finance.provider, "alpha_vantage");
    assert_eq!(merged.finance.api_key.as_deref(), Some("demo-key"));
}

#[test]
fn finance_config_overlay_yahoo_with_user_agent_is_propagated() {
    let mut base = Config::default();
    base.finance.provider = "alpha_vantage".to_string();
    base.finance.api_key = Some("global-key".to_string());

    let mut overlay = Config::default();
    overlay.finance.user_agent = Some("custom-ua".to_string());

    let merged = Config::merge(base, overlay);
    // Explicit Yahoo-keep with user_agent should override the global paid config.
    assert_eq!(merged.finance.provider, "yahoo");
    assert!(merged.finance.api_key.is_none());
    assert_eq!(merged.finance.user_agent.as_deref(), Some("custom-ua"));
}
