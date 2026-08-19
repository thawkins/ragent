//! Typed errors for the finance toolset.
//!
//! Errors are normalized across providers so that tools can present a consistent
//! message and avoid retrying user errors such as an unknown ticker symbol.

use thiserror::Error;

/// Error type returned by all finance provider operations and tools.
#[derive(Error, Debug, Clone, PartialEq)]
pub enum FinanceError {
    /// The requested symbol could not be resolved by the provider.
    #[error("symbol not found: {symbol}")]
    SymbolNotFound { symbol: String },

    /// The provider rate-limited the request.
    #[error("rate limit hit for provider {provider}{}; try again later or configure a paid provider such as alpha_vantage", retry_after.map(|s| format!(" (retry after {}s)", s)).unwrap_or_default())]
    RateLimit {
        provider: String,
        retry_after: Option<u64>,
    },

    /// A provider-side failure that is not a rate limit or parse error.
    #[error("provider {provider} failure: {message}")]
    ProviderFailure { provider: String, message: String },

    /// The provider returned data that could not be parsed into the normalized model.
    #[error("failed to parse response from {provider}: {detail}")]
    ParseFailure { provider: String, detail: String },

    /// The configuration for the requested provider is missing or invalid.
    #[error("finance configuration error: {0}")]
    ConfigError(String),
}

impl FinanceError {
    /// Returns `true` if this error represents an unknown symbol.
    #[must_use]
    pub fn is_symbol_not_found(&self) -> bool {
        matches!(self, FinanceError::SymbolNotFound { .. })
    }

    /// Returns `true` if this error represents a provider rate limit.
    #[must_use]
    pub fn is_rate_limit(&self) -> bool {
        matches!(self, FinanceError::RateLimit { .. })
    }
}

/// Convenience type alias for finance operations.
pub type FinanceResult<T> = std::result::Result<T, FinanceError>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn symbol_not_found_formats_message() {
        let err = FinanceError::SymbolNotFound {
            symbol: "INVALID".to_string(),
        };
        assert_eq!(err.is_symbol_not_found(), true);
        assert_eq!(err.is_rate_limit(), false);
        assert_eq!(err.to_string(), "symbol not found: INVALID");
    }

    #[test]
    fn rate_limit_formats_retry_after() {
        let err = FinanceError::RateLimit {
            provider: "yahoo".to_string(),
            retry_after: Some(30),
        };
        assert_eq!(err.is_rate_limit(), true);
        assert!(
            err.to_string()
                .contains("rate limit hit for provider yahoo")
        );
        assert!(err.to_string().contains("retry after 30s"));
    }

    #[test]
    fn provider_failure_formats_provider_and_message() {
        let err = FinanceError::ProviderFailure {
            provider: "alpha_vantage".to_string(),
            message: "network timeout".to_string(),
        };
        assert!(
            err.to_string()
                .contains("provider alpha_vantage failure: network timeout")
        );
    }

    #[test]
    fn parse_failure_formats_provider_and_detail() {
        let err = FinanceError::ParseFailure {
            provider: "yahoo".to_string(),
            detail: "missing field 'regularMarketPrice'".to_string(),
        };
        assert!(
            err.to_string()
                .contains("failed to parse response from yahoo")
        );
        assert!(
            err.to_string()
                .contains("missing field 'regularMarketPrice'")
        );
    }
}
