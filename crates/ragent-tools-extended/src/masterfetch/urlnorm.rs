//! URL normalisation and deduplication for the masterfetch toolset.
//!
//! Implements **FR-027** and **NFR-003**.
//!
//! This module provides [`normalise_url`], a pure function that transforms a
//! URL into a canonical form suitable for use as a cache key and for crawl /
//! search-result deduplication. The normalisation steps are:
//!
//! 1. **Lowercase the host** — e.g. `Example.COM` → `example.com`.
//! 2. **Strip default ports** — `:80` for `http`, `:443` for `https`.
//! 3. **Remove trailing slashes on non-root paths** — `/path/` → `/path`,
//!    but `/` is preserved.
//! 4. **Strip tracking query parameters** — any parameter whose key begins
//!    with `utm` (covering `utm_source`, `utm_medium`, `utm_campaign`, …)
//!    as well as `fbclid`, `gclid`, `ref`, `_ga`, `mc_cid`, and `mc_eid`.
//!
//! The [`url`] crate already lowercases the scheme and host and strips default
//! ports during parsing, so those steps are handled implicitly. The trailing
//! slash and tracking-parameter stripping are performed explicitly here.
//!
//! The function is idempotent: normalising an already-normalised URL yields
//! the same string. This is essential for cache-key stability.
//!
//! # Example
//!
//! ```
//! use ragent_tools_extended::masterfetch::urlnorm::normalise_url;
//!
//! let norm = normalise_url("https://Example.com:443/path/?utm_source=x&keep=1").unwrap();
//! assert_eq!(norm, "https://example.com/path?keep=1");
//! ```

use thiserror::Error;
use url::Url;

/// Tracking query parameter keys that are stripped during normalisation
/// (FR-027).
///
/// Any parameter whose key exactly matches one of these names, or begins with
/// the `utm` prefix, is removed.
const TRACKING_PARAM_KEYS: &[&str] = &["fbclid", "gclid", "ref", "_ga", "mc_cid", "mc_eid"];

/// Error returned when a URL cannot be normalised.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum UrlNormError {
    /// The URL failed to parse.
    #[error("URL parse error: {0}")]
    Parse(String),
}

/// Normalise a URL into its canonical form for deduplication and cache keys.
///
/// Performs the following transformations (FR-027):
///
/// - Lowercases the scheme and host (handled by the `url` crate on parse).
/// - Strips default ports (`:80` for `http`, `:443` for `https` — handled by
///   the `url` crate on parse).
/// - Removes trailing slashes on non-root paths (e.g. `/path/` → `/path`).
/// - Strips tracking query parameters (`utm_*`, `fbclid`, `gclid`, `ref`,
///   `_ga`, `mc_cid`, `mc_eid`).
///
/// The function is **idempotent**: applying it a second time produces the
/// same string.
///
/// # Errors
///
/// Returns [`UrlNormError::Parse`] if the input is not a valid absolute URL.
///
/// # Examples
///
/// ```
/// use ragent_tools_extended::masterfetch::urlnorm::normalise_url;
///
/// assert_eq!(
///     normalise_url("https://Example.com:443/path/").unwrap(),
///     "https://example.com/path"
/// );
/// assert_eq!(
///     normalise_url("https://example.com?utm_source=foo&keep=1").unwrap(),
///     "https://example.com/?keep=1"
/// );
/// // Idempotent: normalising twice yields the same result.
/// let once = normalise_url("http://Example.com:80/a/").unwrap();
/// let twice = normalise_url(&once).unwrap();
/// assert_eq!(once, twice);
/// ```
pub fn normalise_url(raw: &str) -> Result<String, UrlNormError> {
    let mut url = Url::parse(raw).map_err(|e| UrlNormError::Parse(e.to_string()))?;

    // Step 1 — strip trailing slashes on non-root paths.
    strip_trailing_slash(&mut url);

    // Step 2 — strip tracking query parameters.
    strip_tracking_params(&mut url);

    Ok(url.to_string())
}

/// Normalise a list of URLs and return the deduplicated set, preserving the
/// order of first occurrence.
///
/// Each URL is normalised via [`normalise_url`]; URLs that fail to parse are
/// silently skipped. The resulting list contains no duplicate normalised
/// URLs.
///
/// # Examples
///
/// ```
/// use ragent_tools_extended::masterfetch::urlnorm::dedup_urls;
///
/// let urls = vec![
///     "https://example.com/page/",
///     "https://example.com/page",   // same as #1 after normalisation
///     "https://other.com",
/// ];
/// let deduped = dedup_urls(&urls);
/// assert_eq!(deduped, vec!["https://example.com/page", "https://other.com/"]);
/// ```
pub fn dedup_urls(urls: &[&str]) -> Vec<String> {
    let mut seen = std::collections::HashSet::new();
    let mut result = Vec::with_capacity(urls.len());
    for raw in urls {
        if let Ok(norm) = normalise_url(raw)
            && seen.insert(norm.clone())
        {
            result.push(norm);
        }
    }
    result
}

// ---------------------------------------------------------------------------
// Internal helpers
// ---------------------------------------------------------------------------

/// Remove trailing slashes from the URL path, preserving the root path `/`.
///
/// `/path/` → `/path`
/// `/` → `/`  (unchanged)
/// `` (empty) → `/`  (unchanged, the url crate already normalises this to `/`)
fn strip_trailing_slash(url: &mut Url) {
    let path = url.path().to_owned();
    // Only strip if the path is longer than "/" and ends with '/'.
    if path.len() > 1 && path.ends_with('/') {
        let trimmed = path.trim_end_matches('/');
        // If the path becomes empty after trimming (shouldn't happen given the
        // len > 1 guard, but be defensive), restore to "/".
        let new_path = if trimmed.is_empty() { "/" } else { trimmed };
        url.set_path(new_path);
    }
}

/// Remove tracking query parameters from the URL.
///
/// Strips any parameter whose key begins with `utm` or matches one of the
/// entries in [`TRACKING_PARAM_KEYS`]. If all parameters are removed, the
/// query string is cleared entirely (no trailing `?`).
fn strip_tracking_params(url: &mut Url) {
    // Collect the surviving key-value pairs.
    let surviving: Vec<(String, String)> = url
        .query_pairs()
        .filter(|(key, _)| !is_tracking_param(key))
        .map(|(k, v)| (k.into_owned(), v.into_owned()))
        .collect();

    // If nothing was filtered, no mutation is needed.
    if surviving.len() == url.query_pairs().count() {
        return;
    }

    // Rebuild the query string from surviving pairs.
    if surviving.is_empty() {
        url.set_query(None);
    } else {
        let mut serializer = url.query_pairs_mut();
        serializer.clear();
        for (key, value) in &surviving {
            serializer.append_pair(key, value);
        }
        // Drop the serializer to release the borrow on `url`.
        drop(serializer);
    }
}

/// Returns `true` if the given query parameter key is a tracking parameter
/// that should be stripped during normalisation.
///
/// A key is considered tracking if it begins with `utm` (case-insensitive,
/// covering `utm_source`, `utm_medium`, `utm_campaign`, etc.) or exactly
/// matches one of the keys in [`TRACKING_PARAM_KEYS`] (case-insensitive).
fn is_tracking_param(key: &str) -> bool {
    let lower = key.to_ascii_lowercase();
    lower.starts_with("utm") || TRACKING_PARAM_KEYS.iter().any(|&t| lower == t)
}
