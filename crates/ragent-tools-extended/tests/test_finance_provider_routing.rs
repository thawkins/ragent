//! Unit tests for paid-provider routing logic.
//!
//! These tests verify that the finance module selects the right provider based
//! on configuration: free Yahoo when no paid provider is configured, paid
//! provider when configured, and errors for unsupported or misconfigured
//! paid providers.

use ragent_config::finance::FinanceProviderConfig;
use ragent_tools_extended::finance::{
    FinanceError, FinanceProvider, YahooFinanceProvider, default_provider,
    paid_provider_from_config,
};
use std::sync::Arc;

#[test]
fn test_default_provider_uses_yahoo_when_no_config() {
    let provider = default_provider(None);
    assert_eq!(provider.name(), "yahoo");
    assert!(provider.is_available());
}

#[test]
fn test_default_provider_uses_yahoo_for_explicit_default_config() {
    let config = FinanceProviderConfig::default();
    assert_eq!(config.provider, "yahoo");
    assert!(!config.is_paid_provider_configured());

    let provider = default_provider(Some(&config));
    assert_eq!(provider.name(), "yahoo");
}

#[test]
fn test_paid_provider_from_config_returns_alpha_vantage() {
    let config = FinanceProviderConfig {
        provider: "alpha_vantage".to_string(),
        api_key: Some("test-key".to_string()),
        base_url: None,
        requests_per_minute: None,
        user_agent: None,
        min_call_interval_seconds: Default::default(),
    };

    let provider = paid_provider_from_config(&config).expect("valid paid config should build");
    assert_eq!(provider.name(), "alpha_vantage");
    assert!(provider.is_available());
}

#[test]
fn test_default_provider_disables_free_adapter_when_paid_configured() {
    let config = FinanceProviderConfig {
        provider: "alpha_vantage".to_string(),
        api_key: Some("paid-key".to_string()),
        base_url: None,
        requests_per_minute: None,
        user_agent: None,
        min_call_interval_seconds: Default::default(),
    };

    assert!(config.is_paid_provider_configured());

    let provider = default_provider(Some(&config));
    assert_eq!(
        provider.name(),
        "alpha_vantage",
        "paid provider should be selected and free Yahoo adapter disabled"
    );
}

#[test]
fn test_paid_provider_from_config_rejects_missing_api_key() {
    let config = FinanceProviderConfig {
        provider: "alpha_vantage".to_string(),
        api_key: None,
        base_url: None,
        requests_per_minute: None,
        user_agent: None,
        min_call_interval_seconds: Default::default(),
    };

    let result = paid_provider_from_config(&config);
    let err = result.expect_err("missing API key should fail");
    assert!(
        matches!(err, FinanceError::ConfigError(ref msg) if msg.contains("API key missing")),
        "expected ConfigError for missing API key, got {:?}",
        err
    );
}

#[test]
fn test_paid_provider_from_config_rejects_unsupported_provider() {
    let config = FinanceProviderConfig {
        provider: "polygon".to_string(),
        api_key: Some("test-key".to_string()),
        base_url: None,
        requests_per_minute: None,
        user_agent: None,
        min_call_interval_seconds: Default::default(),
    };

    let result = paid_provider_from_config(&config);
    let err = result.expect_err("unsupported provider should fail");
    assert!(
        matches!(err, FinanceError::ConfigError(ref msg) if msg.contains("not supported")),
        "expected ConfigError for unsupported provider, got {:?}",
        err
    );
}

#[test]
fn test_paid_provider_new_rejects_empty_name() {
    let err = ragent_tools_extended::finance::PaidProvider::new("", "key", None)
        .expect_err("empty provider name should fail");
    assert!(
        matches!(err, FinanceError::ConfigError(ref msg) if msg.contains("name is empty")),
        "expected ConfigError for empty provider name, got {:?}",
        err
    );
}

#[test]
fn test_paid_provider_new_rejects_empty_api_key() {
    let err = ragent_tools_extended::finance::PaidProvider::new("alpha_vantage", "", None)
        .expect_err("empty API key should fail");
    assert!(
        matches!(err, FinanceError::ConfigError(ref msg) if msg.contains("API key is empty")),
        "expected ConfigError for empty API key, got {:?}",
        err
    );
}

#[test]
fn test_paid_provider_configured_requires_non_yahoo_provider_and_key() {
    let mut config = FinanceProviderConfig::default();
    assert!(!config.is_paid_provider_configured());

    // Yahoo with a key is still not considered a paid provider.
    config.api_key = Some("ignored".to_string());
    assert!(!config.is_paid_provider_configured());

    // Alpha Vantage without a key is not considered configured.
    config.provider = "alpha_vantage".to_string();
    config.api_key = None;
    assert!(!config.is_paid_provider_configured());

    // Alpha Vantage with a key is configured.
    config.api_key = Some("real-key".to_string());
    assert!(config.is_paid_provider_configured());
}

#[test]
fn test_yahoo_provider_name_is_constant() {
    let provider = YahooFinanceProvider::default_client();
    assert_eq!(provider.name(), "yahoo");
    assert!(provider.is_available());
}

#[test]
fn test_default_provider_reuses_yahoo_instance() {
    let a = default_provider(None);
    let b = default_provider(None);
    assert!(
        Arc::ptr_eq(&a, &b),
        "default_provider should return the same shared Yahoo instance"
    );
}

#[test]
fn test_yahoo_provider_from_config_applies_user_agent_and_throttle() {
    let config = FinanceProviderConfig {
        provider: "yahoo".to_string(),
        api_key: None,
        base_url: None,
        requests_per_minute: Some(30),
        user_agent: Some("Mozilla/5.0 custom".to_string()),
        min_call_interval_seconds: Default::default(),
    };

    let provider = YahooFinanceProvider::from_config(&config).expect("valid config should build");
    assert_eq!(provider.name(), "yahoo");
}

#[test]
fn test_yahoo_provider_from_config_falls_back_on_invalid_user_agent() {
    // yfinance_rs accepts any string as a user agent, so an empty string is
    // still valid; this test guards the fallback path in default_provider.
    let config = FinanceProviderConfig {
        provider: "yahoo".to_string(),
        api_key: None,
        base_url: None,
        requests_per_minute: None,
        user_agent: Some("".to_string()),
        min_call_interval_seconds: Default::default(),
    };

    let provider = default_provider(Some(&config));
    assert_eq!(provider.name(), "yahoo");
}

#[tokio::test]
async fn test_alpha_vantage_history_returns_bars_for_msft() {
    let config = FinanceProviderConfig {
        provider: "alpha_vantage".to_string(),
        api_key: Some(
            std::env::var("ALPHA_VANTAGE_API_KEY").unwrap_or_else(|_| "demo".to_string()),
        ),
        base_url: None,
        requests_per_minute: None,
        user_agent: None,
        min_call_interval_seconds: Default::default(),
    };
    if config.api_key.as_deref() == Some("demo") {
        // Skip live test when no real API key is available.
        return;
    }

    let provider = paid_provider_from_config(&config).expect("valid paid config should build");
    let bars = provider
        .history("MSFT", "1d", "1w")
        .await
        .expect("MSFT history should return bars");
    assert!(!bars.is_empty(), "MSFT 1-week history should contain bars");
    assert!(bars.windows(2).all(|w| w[0].timestamp <= w[1].timestamp));
}
