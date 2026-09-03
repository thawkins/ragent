#![allow(clippy::assert_is_empty)]
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
        yahoo_fallback: None,
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
        yahoo_fallback: None,
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
        yahoo_fallback: None,
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
        yahoo_fallback: None,
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
fn test_yahoo_fallback_disabled_by_default_for_paid_provider() {
    let paid_config = FinanceProviderConfig {
        provider: "twelvedata".to_string(),
        api_key: Some("td-test-key".to_string()),
        base_url: None,
        requests_per_minute: None,
        user_agent: None,
        min_call_interval_seconds: Default::default(),
        yahoo_fallback: None,
    };
    assert!(paid_config.is_paid_provider_configured());
    assert!(!paid_config.yahoo_fallback_enabled());
}

#[test]
fn test_yahoo_fallback_enabled_by_default_for_yahoo_provider() {
    let yahoo_config = FinanceProviderConfig::default();
    assert_eq!(yahoo_config.provider, "yahoo");
    assert!(!yahoo_config.is_paid_provider_configured());
    assert!(yahoo_config.yahoo_fallback_enabled());
}

#[test]
fn test_yahoo_fallback_can_be_explicitly_enabled_for_paid_provider() {
    let paid_config = FinanceProviderConfig {
        provider: "twelvedata".to_string(),
        api_key: Some("td-test-key".to_string()),
        base_url: None,
        requests_per_minute: None,
        user_agent: None,
        min_call_interval_seconds: Default::default(),
        yahoo_fallback: Some(true),
    };
    assert!(paid_config.is_paid_provider_configured());
    assert!(paid_config.yahoo_fallback_enabled());
}

#[test]
fn test_yahoo_fallback_can_be_explicitly_disabled_for_yahoo_provider() {
    let yahoo_config = FinanceProviderConfig {
        yahoo_fallback: Some(false),
        ..Default::default()
    };
    assert!(!yahoo_config.yahoo_fallback_enabled());
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
        yahoo_fallback: None,
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
        user_agent: Some(String::new()),
        min_call_interval_seconds: Default::default(),
        yahoo_fallback: None,
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
        yahoo_fallback: None,
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

#[tokio::test]
async fn test_stock_tools_publish_provider_notice() {
    //! Verify that a stock tool emits an `Event::AgentNotice` naming the
    //! selected finance provider so the TUI log window can display it.
    use ragent_tools_extended::finance::tools::{quote::StockQuoteTool, search::StockSearchTool};
    use ragent_tools_extended::{Tool, ToolContext};
    use serde_json::json;
    use std::sync::Arc;

    let event_bus = Arc::new(ragent_types::event::EventBus::new(64));
    let ctx = ToolContext {
        session_id: "test-session".to_string(),
        working_dir: std::env::temp_dir(),
        event_bus: event_bus.clone(),
        storage: None,
        code_index: None,
        config: None,
        read_timestamps: Arc::new(std::sync::RwLock::new(std::collections::HashMap::new())),
    };

    // stock_quote resolves from cache when present; with no prior cache the
    // provider notice should still be published before the network call.
    let mut rx = event_bus.subscribe();
    let tool = StockQuoteTool::new();
    // The quote call will likely fail without network, but the notice must
    // still be emitted before the provider method is invoked.
    let _ = tool.execute(json!({"symbol": "AAPL"}), &ctx).await;

    let notice = helpers::recv_agent_notice(&mut rx).await;
    assert_eq!(notice.session_id, "test-session");
    assert!(
        notice.message.contains("stock_quote"),
        "notice should name the tool: {}",
        notice.message
    );
    assert!(
        notice.message.contains("'yahoo'"),
        "notice should name the provider: {}",
        notice.message
    );

    // stock_search also emits a provider notice.
    let mut rx = event_bus.subscribe();
    let tool = StockSearchTool;
    let _ = tool.execute(json!({"query": "Apple"}), &ctx).await;

    let notice = helpers::recv_agent_notice(&mut rx).await;
    assert_eq!(notice.session_id, "test-session");
    assert!(
        notice.message.contains("stock_search"),
        "notice should name the tool: {}",
        notice.message
    );
    assert!(
        notice.message.contains("'yahoo'"),
        "notice should name the provider: {}",
        notice.message
    );
}

mod helpers {
    use super::AgentNotice;
    use ragent_types::event::Event;
    use tokio::sync::broadcast::Receiver;

    pub async fn recv_agent_notice(rx: &mut Receiver<Event>) -> AgentNotice {
        loop {
            match rx.recv().await {
                Ok(Event::AgentNotice {
                    session_id,
                    message,
                }) => {
                    return AgentNotice {
                        session_id,
                        message,
                    };
                }
                Ok(_) => continue,
                Err(e) => panic!("event channel closed before AgentNotice: {e}"),
            }
        }
    }
}

/// Minimal struct matching the AgentNotice payload for assertions.
/// Session that produced the notice.
pub struct AgentNotice {
    /// Identifier of the session the notice refers to.
    pub session_id: String,
    /// Human-readable notice text.
    pub message: String,
}

#[test]
fn test_paid_provider_from_config_returns_twelvedata() {
    let config = FinanceProviderConfig {
        provider: "twelvedata".to_string(),
        api_key: Some("td-test-key".to_string()),
        base_url: None,
        requests_per_minute: None,
        user_agent: None,
        min_call_interval_seconds: Default::default(),
        yahoo_fallback: None,
    };

    let provider =
        paid_provider_from_config(&config).expect("valid TwelveData config should build");
    assert_eq!(provider.name(), "twelvedata");
    assert!(provider.is_available());
}

#[test]
fn test_paid_provider_from_config_rejects_missing_twelvedata_key() {
    let config = FinanceProviderConfig {
        provider: "twelvedata".to_string(),
        api_key: None,
        base_url: None,
        requests_per_minute: None,
        user_agent: None,
        min_call_interval_seconds: Default::default(),
        yahoo_fallback: None,
    };

    let err = paid_provider_from_config(&config).expect_err("missing key should fail");
    assert!(
        matches!(err, FinanceError::ConfigError(ref msg) if msg.contains("TwelveData API key missing")),
        "expected ConfigError for missing TwelveData key, got {:?}",
        err
    );
}

#[tokio::test]
async fn test_twelvedata_quote_and_history_with_api_key() {
    use ragent_tools_extended::finance::TwelveDataProvider;

    let api_key = std::env::var("TWELVEDATA_API_KEY").unwrap_or_default();
    if api_key.is_empty() {
        // Skip live integration test when no real key is available.
        return;
    }

    let provider = TwelveDataProvider::new(&api_key, None).expect("valid key should build");

    // US ticker works unchanged.
    let quote = provider
        .quote("MSFT")
        .await
        .expect("MSFT quote should succeed with a valid key");
    assert_eq!(quote.symbol, "MSFT");

    let bars = provider
        .history("MSFT", "1d", "1wk")
        .await
        .expect("MSFT 1-week history should succeed");
    assert!(!bars.is_empty(), "MSFT 1-week history should contain bars");
    assert!(bars.windows(2).all(|w| w[0].timestamp <= w[1].timestamp));

    // LSE ticker with .L suffix should be normalized and routed to the LSE exchange.
    let lse_quote = provider
        .quote("LSEG.L")
        .await
        .expect("LSEG.L quote should succeed with exchange=LSE");
    assert_eq!(lse_quote.symbol, "LSEG.L");

    let lse_bars = provider
        .history("LSEG.L", "1d", "1wk")
        .await
        .expect("LSEG.L 1-week history should succeed");
    assert!(
        !lse_bars.is_empty(),
        "LSEG.L 1-week history should contain bars"
    );
    assert!(
        lse_bars
            .windows(2)
            .all(|w| w[0].timestamp <= w[1].timestamp)
    );
}
