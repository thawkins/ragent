//! Paid-provider configuration for the finance toolset.
//!
//! This module is part of `ragent-config` so that tools can read the selected
//! provider and its API key from the standard config layer.

use serde::{Deserialize, Serialize};

/// Finance provider selection and credentials.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct FinanceProviderConfig {
    /// Selected provider: "yahoo" (default) or "alpha_vantage".
    #[serde(default = "default_provider")]
    pub provider: String,
    /// API key when a paid provider is selected.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub api_key: Option<String>,
    /// Optional base URL override for the paid provider.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub base_url: Option<String>,
    /// Optional request rate limit (requests per minute).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub requests_per_minute: Option<u32>,
    /// Optional custom User-Agent header for the free Yahoo provider.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub user_agent: Option<String>,
    /// Minimum seconds between any two finance provider API calls.
    ///
    /// Shared across Yahoo and Alpha Vantage so that rapid tool calls do not
    /// trigger provider-side rate limits. Defaults to 5 seconds.
    #[serde(default = "default_min_call_interval_seconds")]
    pub min_call_interval_seconds: u64,
}

fn default_provider() -> String {
    "yahoo".to_string()
}

fn default_min_call_interval_seconds() -> u64 {
    5
}

impl Default for FinanceProviderConfig {
    fn default() -> Self {
        Self {
            provider: default_provider(),
            api_key: None,
            base_url: None,
            requests_per_minute: None,
            user_agent: None,
            min_call_interval_seconds: default_min_call_interval_seconds(),
        }
    }
}

impl FinanceProviderConfig {
    /// Returns true when the configuration explicitly selects a non-Yahoo
    /// provider and provides an API key.
    #[must_use]
    pub fn is_paid_provider_configured(&self) -> bool {
        !self.provider.eq_ignore_ascii_case("yahoo") && self.api_key.is_some()
    }

    /// Returns true when any field differs from the default Yahoo-only config,
    /// i.e. the user explicitly supplied a finance block.
    #[must_use]
    pub fn is_explicitly_configured(&self) -> bool {
        self.provider != default_provider()
            || self.api_key.is_some()
            || self.base_url.is_some()
            || self.requests_per_minute.is_some()
            || self.user_agent.is_some()
            || self.min_call_interval_seconds != default_min_call_interval_seconds()
    }
}
