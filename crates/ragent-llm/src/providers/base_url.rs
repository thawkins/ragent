//! Base-URL validation for providers whose endpoint can be supplied through the
//! ambient environment (`GENERIC_OPENAI_API_BASE`, `AZURE_AI_FOUNDRY_BASE`, the
//! `OLLAMA_HOST` family, ...).
//!
//! An endpoint read straight from the environment is an exfiltration vector: a
//! poisoned environment could redirect every request (and the credentials this
//! process attaches to it) at a host the operator did not intend. This module
//! rejects anything that is not an `http`/`https` URL with a non-empty host, so
//! a `file://`, `ftp://`, or scheme-relative value can never be dialled.
//!
//! It is intentionally dependency-free (no `url` crate): the check is a scheme
//! allowlist plus a structural host presence test, not a full parser.

use anyhow::{Result, bail};

/// Validate an environment-supplied base URL.
///
/// Accepts only `http://` or `https://` URLs carrying a non-empty authority
/// (host[:port]) with no embedded whitespace. Returns the URL with any trailing
/// `/` stripped, matching how the providers normalise endpoints.
///
/// The `provider` argument names the provider in the error message so a
/// misconfigured environment variable is attributable.
///
/// # Errors
///
/// Returns an error when `raw` is empty, uses a scheme other than `http`/
/// `https`, has no host, or contains whitespace or a bare `//` authority.
pub(crate) fn validate_base_url(provider: &str, raw: &str) -> Result<String> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        bail!("{provider}: base URL from environment is empty");
    }
    if trimmed.chars().any(char::is_whitespace) {
        bail!("{provider}: base URL contains whitespace");
    }

    let (scheme, rest) = trimmed.split_once("://").ok_or_else(|| {
        anyhow::anyhow!("{provider}: base URL is missing a scheme (expected http:// or https://)")
    })?;

    if !scheme.eq_ignore_ascii_case("http") && !scheme.eq_ignore_ascii_case("https") {
        bail!("{provider}: unsupported base URL scheme `{scheme}` (only http/https are allowed)");
    }

    // The authority runs to the first `/`, `?`, or `#`.
    let authority = rest
        .split(['/', '?', '#'])
        .next()
        .unwrap_or_default()
        .trim();
    if authority.is_empty() {
        bail!("{provider}: base URL has no host");
    }

    Ok(trimmed.trim_end_matches('/').to_string())
}

/// Resolve a provider base URL from an optional config/env candidate, validating
/// it and falling back to `default` when it is absent or invalid.
///
/// SEC-ragent-llm-005 (SECTASKS T-028): a provider `api.base_url` sourced from
/// `ragent.json` is attacker-influenceable exactly like an environment variable,
/// so it must pass the same scheme/host check before it is dialled. When the
/// candidate is empty or fails validation the provider's known-good `default`
/// is used instead, so a bad config degrades to the stock endpoint rather than
/// redirecting an authenticated request.
///
/// The returned URL has any trailing `/` stripped, matching how the providers
/// normalise endpoints.
#[must_use]
pub(crate) fn resolve_validated_base_url(
    provider: &str,
    candidate: Option<&str>,
    default: &str,
) -> String {
    match candidate {
        Some(raw) if !raw.trim().is_empty() => match validate_base_url(provider, raw) {
            Ok(valid) => valid,
            Err(e) => {
                tracing::warn!(
                    provider = %provider,
                    error = %e,
                    "ignoring invalid configured base URL; using default"
                );
                default.to_string()
            }
        },
        _ => default.to_string(),
    }
}

#[cfg(test)]
#[path = "../../tests/inline/base_url_tests.rs"]
mod base_url_tests;
