//! Typed errors for the finance toolset.
//!
//! Errors are normalized across providers so that tools can present a consistent
//! message and avoid retrying user errors such as an unknown ticker symbol.

use thiserror::Error;

/// Error type returned by all finance provider operations and tools.
#[derive(Error, Debug, Clone, PartialEq, Eq)]
pub enum FinanceError {
    /// The requested symbol could not be resolved by the provider.
    #[error("symbol not found: {symbol}")]
    SymbolNotFound {
        /// The ticker symbol that could not be resolved.
        symbol: String,
    },

    /// The provider rate-limited the request.
    #[error("rate limit hit for provider {provider}{}; try again later or configure a paid provider such as alpha_vantage or twelvedata", retry_after.map(|s| format!(" (retry after {}s)", s)).unwrap_or_default())]
    RateLimit {
        /// The provider that returned the rate limit.
        provider: String,
        /// Seconds to wait before retrying, when the provider supplies one.
        retry_after: Option<u64>,
    },

    /// A provider-side failure that is not a rate limit or parse error.
    #[error("provider {provider} failure: {message}")]
    ProviderFailure {
        /// The provider that failed.
        provider: String,
        /// The failure message.
        message: String,
    },

    /// The provider returned data that could not be parsed into the normalized model.
    #[error("failed to parse response from {provider}: {detail}")]
    ParseFailure {
        /// The provider whose response could not be parsed.
        provider: String,
        /// Detail about what failed to parse.
        detail: String,
    },

    /// The configuration for the requested provider is missing or invalid.
    #[error("finance configuration error: {0}")]
    ConfigError(String),
}

impl FinanceError {
    /// Returns `true` if this error represents an unknown symbol.
    #[must_use]
    pub fn is_symbol_not_found(&self) -> bool {
        matches!(self, Self::SymbolNotFound { .. })
    }

    /// Returns `true` if this error represents a provider rate limit.
    #[must_use]
    pub fn is_rate_limit(&self) -> bool {
        matches!(self, Self::RateLimit { .. })
    }
}

/// Convenience type alias for finance operations.
pub type FinanceResult<T> = std::result::Result<T, FinanceError>;

#[cfg(test)]
#[path = "../tests/inline/error_tests.rs"]
mod tests;
