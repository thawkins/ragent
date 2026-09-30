//! Inline tests for `error.rs` - compiled inside the module tree
//! via the `#[path]` delegation left in the source file
//! (ANTIPAT M2 inline-test migration; no behaviour change).

use super::*;

#[test]
fn symbol_not_found_formats_message() {
    let err = FinanceError::SymbolNotFound {
        symbol: "INVALID".to_string(),
    };
    assert!(err.is_symbol_not_found());
    assert!(!err.is_rate_limit());
    assert_eq!(err.to_string(), "symbol not found: INVALID");
}

#[test]
fn rate_limit_formats_retry_after() {
    let err = FinanceError::RateLimit {
        provider: "yahoo".to_string(),
        retry_after: Some(30),
    };
    assert!(err.is_rate_limit());
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
