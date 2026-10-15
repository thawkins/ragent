//! Open-access (OA) full-text recovery for paywalled scholarly sources (FR-010).
//!
//! When the web gatherer captures a scholarly source whose extracted body is
//! shorter than the configured threshold, the recovery layer queries
//! [Unpaywall](https://unpaywall.org/) and [Europe PMC](https://europepmc.org/)
//! for a legal OA copy. If a copy is found, the gatherer fetches the recovered
//! URL and uses that full text as the source body while keeping the original
//! paywalled URL as the canonical citation address.
//!
//! The recovery module is intentionally decoupled from HTTP implementation
//! details via [`OpenAccessClient`] so tests can supply a fake client and so
//! the gatherer can reuse any configured timeout/user-agent behaviour.

use async_trait::async_trait;
use futures::StreamExt;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::fmt;
use std::net::IpAddr;
use std::time::Duration;
use url::form_urlencoded;

/// Default minimum full-text length (in characters) that a scholarly source
/// must have before OA recovery is attempted (FR-010).
///
/// Pages whose body is shorter than this are assumed to be abstracts,
/// paywalls, or otherwise incomplete captures.
pub const DEFAULT_OA_MIN_FULL_TEXT_CHARS: usize = 1000;

/// User-Agent sent to OA services. The contact email is appended when
/// configured so the requests satisfy Unpaywall's terms and participate in
/// Europe PMC's polite pool.
const OA_USER_AGENT: &str = "ragent/1.0 (https://github.com/thawkins/ragent)";

/// Maximum number of response-body bytes buffered from an OA service before the
/// request is rejected (SEC-065).
///
/// An OA lookup is driven by a DOI scraped from an untrusted page, so a
/// malicious or compromised endpoint could return a multi-gigabyte body that
/// `.text()`/`.json()` would buffer and parse in one shot. 8 MiB is comfortably
/// above any legitimate Unpaywall / Europe PMC JSON payload while bounding the
/// allocation.
const OA_MAX_BODY_BYTES: usize = 8 * 1024 * 1024;

/// Maximum number of redirects the OA HTTP client follows (SEC-066).
///
/// Bounding the chain explicitly prevents a hostile OA endpoint from looping
/// the client through an unbounded redirect sequence.
const OA_MAX_REDIRECTS: usize = 5;

/// Errors emitted by the open-access recovery layer.
#[derive(Debug, thiserror::Error)]
pub enum OpenAccessError {
    /// The HTTP request failed.
    #[error("OA HTTP error: {0}")]
    Http(#[from] reqwest::Error),
    /// The response was not valid JSON.
    #[error("OA JSON error: {0}")]
    Json(String),
    /// The response structure was unexpected.
    #[error("OA unexpected response: {0}")]
    Unexpected(String),
}

/// Result alias for OA operations.
pub type Result<T> = std::result::Result<T, OpenAccessError>;

/// Identifies the service that provided a recovered OA copy.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RecoverySource {
    /// Recovered via the Unpaywall API.
    Unpaywall,
    /// Recovered via the Europe PMC API.
    EuropePmc,
}

impl fmt::Display for RecoverySource {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Unpaywall => write!(f, "unpaywall"),
            Self::EuropePmc => write!(f, "europepmc"),
        }
    }
}

/// A recovered open-access full-text location.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RecoveredOpenAccess {
    /// URL of the OA full-text copy (PDF or HTML).
    pub url: String,
    /// Service that located the copy.
    pub source: RecoverySource,
    /// SPDX or free-text license of the OA copy, when reported.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub license: Option<String>,
    /// Version of the OA copy (e.g. `publishedVersion`, `acceptedVersion`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub version: Option<String>,
}

/// Async HTTP abstraction used by the OA recovery queries.
///
/// The default implementation builds a `reqwest::Client` per call and applies
/// a short timeout. Tests provide an in-memory fake.
#[async_trait]
pub trait OpenAccessClient: Send + Sync {
    /// Fetch `url` and return the raw response body as text.
    async fn fetch_text(&self, url: &str) -> Result<String>;

    /// Fetch `url` and return the parsed JSON response.
    async fn fetch_json(&self, url: &str) -> Result<Value>;
}

/// Production HTTP client for OA lookups.
#[derive(Debug, Clone, Default)]
pub struct ReqwestOpenAccessClient {
    /// Optional contact email appended to the user-agent.
    pub contact_email: Option<String>,
    /// Per-request timeout.
    pub timeout: Duration,
}

impl ReqwestOpenAccessClient {
    /// Build a client with the given contact email and the default 30-second
    /// timeout.
    #[must_use]
    pub fn new(contact_email: Option<String>) -> Self {
        Self {
            contact_email,
            timeout: Duration::from_secs(30),
        }
    }

    fn user_agent(&self) -> String {
        match &self.contact_email {
            Some(email) => format!("{OA_USER_AGENT} (mailto:{email})"),
            None => OA_USER_AGENT.to_string(),
        }
    }

    fn client(&self) -> Result<reqwest::Client> {
        reqwest::Client::builder()
            .timeout(self.timeout)
            // Fail fast on DNS/TCP stalls: without a connect timeout a
            // non-responsive host can burn the full request timeout before
            // the transport reports an error.
            .connect_timeout(Duration::from_secs(10))
            // SEC-066: bound the redirect chain explicitly rather than relying
            // on reqwest's default so a hostile/compromised OA endpoint cannot
            // bounce the client through an unbounded redirect loop.
            .redirect(reqwest::redirect::Policy::limited(OA_MAX_REDIRECTS))
            .user_agent(self.user_agent())
            .build()
            .map_err(OpenAccessError::Http)
    }
}

/// Read an OA response body into memory, rejecting anything over
/// [`OA_MAX_BODY_BYTES`] (SEC-065).
///
/// The streamed read stops as soon as the cap is exceeded and returns a
/// [`OpenAccessError::Unexpected`] rather than buffering an unbounded body, so
/// a malicious endpoint cannot exhaust memory.
async fn read_body_capped(resp: reqwest::Response) -> Result<Vec<u8>> {
    // Reject up-front when the declared length already exceeds the cap - this
    // avoids even opening the body stream for an obviously oversized response.
    if let Some(len) = resp.content_length()
        && len > OA_MAX_BODY_BYTES as u64
    {
        return Err(OpenAccessError::Unexpected(format!(
            "OA response declared {len} bytes, exceeding the {OA_MAX_BODY_BYTES}-byte cap"
        )));
    }

    let mut body: Vec<u8> = Vec::new();
    let mut stream = resp.bytes_stream();
    while let Some(chunk) = stream.next().await {
        let chunk = chunk?;
        if body.len().saturating_add(chunk.len()) > OA_MAX_BODY_BYTES {
            return Err(OpenAccessError::Unexpected(format!(
                "OA response exceeded the {OA_MAX_BODY_BYTES}-byte cap"
            )));
        }
        body.extend_from_slice(&chunk);
    }
    Ok(body)
}

#[async_trait]
impl OpenAccessClient for ReqwestOpenAccessClient {
    async fn fetch_text(&self, url: &str) -> Result<String> {
        let client = self.client()?;
        let resp = client.get(url).send().await?.error_for_status()?;
        let body = read_body_capped(resp).await?;
        String::from_utf8(body)
            .map_err(|e| OpenAccessError::Unexpected(format!("OA body was not valid UTF-8: {e}")))
    }

    async fn fetch_json(&self, url: &str) -> Result<Value> {
        let client = self.client()?;
        let resp = client.get(url).send().await?.error_for_status()?;
        let body = read_body_capped(resp).await?;
        serde_json::from_slice(&body).map_err(|e| OpenAccessError::Json(e.to_string()))
    }
}

/// Extract a bare DOI from a URL or DOI string.
///
/// Recognises:
/// - `https://doi.org/10.x/y`
/// - `https://dx.doi.org/10.x/y`
/// - `doi:10.x/y`
/// - `10.x/y` at the start or after a slash
#[must_use]
pub fn extract_doi(url: &str) -> Option<String> {
    let trimmed = url.trim();
    let after_prefix = trimmed
        .strip_prefix("https://doi.org/")
        .or_else(|| trimmed.strip_prefix("http://doi.org/"))
        .or_else(|| trimmed.strip_prefix("https://dx.doi.org/"))
        .or_else(|| trimmed.strip_prefix("http://dx.doi.org/"))
        .or_else(|| trimmed.strip_prefix("doi:"))
        .or_else(|| trimmed.strip_prefix("DOI:"));

    let candidate = match after_prefix {
        Some(c) => c,
        None => {
            // Accept "10." appearing after the last path segment.
            trimmed.rfind("/10.").map(|i| &trimmed[i + 1..])?
        }
    };

    let doi = candidate.trim();
    if doi.starts_with("10.") && doi.contains('/') {
        // Remove query/fragment suffixes.
        Some(
            doi.split_once('?')
                .map(|(left, _)| left)
                .unwrap_or(doi)
                .split_once('#')
                .map(|(left, _)| left)
                .unwrap_or(doi)
                .to_string(),
        )
    } else {
        None
    }
}

/// Validate a recovered OA full-text URL before it is returned to the caller
/// and later fetched (SEC-066).
///
/// OA URLs come straight from remote Unpaywall / Europe PMC JSON, so they must
/// not be trusted. This requires an `http`/`https` scheme and rejects hosts that
/// are `localhost` or that are IP literals in a loopback, private, link-local,
/// unique-local, unspecified, or otherwise non-global range - the SSRF targets a
/// hostile OA endpoint would point the gatherer at.
///
/// # Errors
///
/// Returns a human-readable reason string when the URL is missing, unparseable,
/// uses a non-HTTP scheme, or targets a disallowed host.
pub fn validate_oa_url(url: &str) -> std::result::Result<(), String> {
    let parsed = url::Url::parse(url).map_err(|e| format!("unparseable OA URL: {e}"))?;

    match parsed.scheme() {
        "http" | "https" => {}
        other => return Err(format!("OA URL uses disallowed scheme '{other}'")),
    }

    match parsed.host() {
        Some(url::Host::Domain(domain)) => {
            let lower = domain.to_ascii_lowercase();
            if lower == "localhost" || lower.ends_with(".localhost") {
                return Err("OA URL targets localhost".to_string());
            }
            Ok(())
        }
        Some(url::Host::Ipv4(addr)) => {
            if is_disallowed_ip(IpAddr::V4(addr)) {
                Err(format!("OA URL targets a non-global IPv4 address {addr}"))
            } else {
                Ok(())
            }
        }
        Some(url::Host::Ipv6(addr)) => {
            if is_disallowed_ip(IpAddr::V6(addr)) {
                Err(format!("OA URL targets a non-global IPv6 address {addr}"))
            } else {
                Ok(())
            }
        }
        None => Err("OA URL has no host".to_string()),
    }
}

/// Returns `true` when `ip` is a loopback, private, link-local, unique-local,
/// unspecified, or otherwise non-global address that an OA-recovered URL must
/// never target.
fn is_disallowed_ip(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(v4) => {
            v4.is_loopback()
                || v4.is_private()
                || v4.is_link_local()
                || v4.is_unspecified()
                || v4.is_broadcast()
                || v4.is_documentation()
                // 100.64.0.0/10 carrier-grade NAT.
                || (v4.octets()[0] == 100 && (64..=127).contains(&v4.octets()[1]))
                // 192.0.0.0/24 IETF protocol assignments.
                || (v4.octets()[0] == 192 && v4.octets()[1] == 0 && v4.octets()[2] == 0)
                // 198.18.0.0/15 benchmarking.
                || (v4.octets()[0] == 198 && (v4.octets()[1] == 18 || v4.octets()[1] == 19))
                // 240.0.0.0/4 reserved.
                || v4.octets()[0] >= 240
        }
        IpAddr::V6(v6) => {
            v6.is_loopback()
                || v6.is_unspecified()
                // Unique-local fc00::/7.
                || (v6.segments()[0] & 0xfe00) == 0xfc00
                // Link-local fe80::/10.
                || (v6.segments()[0] & 0xffc0) == 0xfe80
                // IPv4-mapped ::ffff:0:0/96 - unwrap and re-check the v4 rules
                // so ::ffff:127.0.0.1 and friends are rejected too.
                || v6
                    .to_ipv4_mapped()
                    .is_some_and(|v4| is_disallowed_ip(IpAddr::V4(v4)))
        }
    }
}

/// Query Unpaywall for an OA copy of `doi`.
///
/// Unpaywall requires a contact email in the query string; if `email` is
/// `None` the query is skipped to respect the terms of service.
pub async fn query_unpaywall(
    doi: &str,
    email: Option<&str>,
    client: &dyn OpenAccessClient,
) -> Result<Option<RecoveredOpenAccess>> {
    let Some(email) = email else {
        return Ok(None);
    };
    let encoded_doi = form_urlencoded::byte_serialize(doi.as_bytes()).collect::<String>();
    let encoded_email = form_urlencoded::byte_serialize(email.as_bytes()).collect::<String>();
    let url = format!("https://api.unpaywall.org/v2/{encoded_doi}?email={encoded_email}");
    let value = client.fetch_json(&url).await?;

    if !value
        .get("is_oa")
        .and_then(|v| v.as_bool())
        .unwrap_or(false)
    {
        return Ok(None);
    }

    let best = value.get("best_oa_location").cloned();
    if let Some(location) = best {
        let pdf_url = location
            .get("url_for_pdf")
            .and_then(|v| v.as_str())
            .map(str::to_string);
        let landing_url = location
            .get("url")
            .and_then(|v| v.as_str())
            .map(str::to_string);
        let resolved_url = pdf_url.or(landing_url);
        if let Some(url) = resolved_url {
            // SEC-066: never return an OA URL that would not be safe to fetch.
            if let Err(reason) = validate_oa_url(&url) {
                tracing::warn!(
                    url = %url,
                    reason = %reason,
                    "open_access: rejecting recovered Unpaywall URL"
                );
                return Ok(None);
            }
            let license = location
                .get("license")
                .and_then(|v| v.as_str())
                .map(str::to_string);
            let version = value
                .get("oa_status")
                .and_then(|v| v.as_str())
                .map(str::to_string);
            return Ok(Some(RecoveredOpenAccess {
                url,
                source: RecoverySource::Unpaywall,
                license,
                version,
            }));
        }
    }

    Ok(None)
}

/// Query Europe PMC for an OA copy of `doi`.
///
/// Europe PMC does not require an email, but the supplied contact email is
/// included in the user-agent when a production client is used.
pub async fn query_europepmc(
    doi: &str,
    client: &dyn OpenAccessClient,
) -> Result<Option<RecoveredOpenAccess>> {
    let encoded_doi = form_urlencoded::byte_serialize(doi.as_bytes()).collect::<String>();
    let url = format!(
        "https://www.ebi.ac.uk/europepmc/webservices/rest/search?query={encoded_doi}&format=json&resultType=core"
    );
    let value = client.fetch_json(&url).await?;

    let result_list = value
        .get("resultList")
        .and_then(|v| v.get("result"))
        .and_then(|v| v.as_array())
        .cloned()
        .unwrap_or_default();

    for result in result_list {
        let is_open_access = result
            .get("isOpenAccess")
            .and_then(|v| v.as_str())
            .map(|s| s == "Y")
            .unwrap_or(false);
        if !is_open_access {
            continue;
        }

        let full_text_urls = result
            .get("fullTextUrlList")
            .and_then(|v| v.get("fullTextUrl"))
            .and_then(|v| v.as_array())
            .cloned()
            .unwrap_or_default();

        // Prefer PDF, then HTML; prefer "Open Access" availability.
        let mut best: Option<(String, u32, String)> = None;
        for entry in full_text_urls {
            let doc_style = entry
                .get("documentStyle")
                .and_then(|v| v.as_str())
                .unwrap_or("");
            let availability = entry
                .get("availability")
                .and_then(|v| v.as_str())
                .unwrap_or("");
            let url = entry.get("url").and_then(|v| v.as_str());
            if let Some(url) = url {
                // SEC-066: a hostile OA endpoint could point this at an
                // internal service; skip any URL that is not a global http(s)
                // address rather than scoring it.
                if let Err(reason) = validate_oa_url(url) {
                    tracing::warn!(
                        url = %url,
                        reason = %reason,
                        "open_access: ignoring Europe PMC full-text URL"
                    );
                    continue;
                }
                let score = match (doc_style, availability) {
                    ("pdf", "Open Access") => 3,
                    ("pdf", _) => 2,
                    ("html", "Open Access") => 2,
                    ("html", _) => 1,
                    _ => 0,
                };
                if best
                    .as_ref()
                    .map(|(_, best_score, _)| *best_score)
                    .unwrap_or(0)
                    < score
                {
                    best = Some((doc_style.to_string(), score, url.to_string()));
                }
            }
        }

        if let Some((_, _, url)) = best {
            let license = result
                .get("license")
                .and_then(|v| v.as_str())
                .map(str::to_string);
            return Ok(Some(RecoveredOpenAccess {
                url,
                source: RecoverySource::EuropePmc,
                license,
                version: Some("publishedVersion".to_string()),
            }));
        }
    }

    Ok(None)
}

/// Attempt to recover an OA full-text copy for `url`.
///
/// First extracts a DOI from `url`, then queries Unpaywall (if a contact
/// email is configured) and falls back to Europe PMC. Returns `Ok(None)` when
/// no OA copy is found.
pub async fn recover_open_access(
    url: &str,
    email: Option<&str>,
    client: &dyn OpenAccessClient,
) -> Result<Option<RecoveredOpenAccess>> {
    let Some(doi) = extract_doi(url) else {
        return Ok(None);
    };

    if let Some(recovered) = query_unpaywall(&doi, email, client).await? {
        return Ok(Some(recovered));
    }

    if let Some(recovered) = query_europepmc(&doi, client).await? {
        return Ok(Some(recovered));
    }

    Ok(None)
}

#[cfg(test)]
#[path = "../tests/inline/open_access_tests.rs"]
mod tests;
