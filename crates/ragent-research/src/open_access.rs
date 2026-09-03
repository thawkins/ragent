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
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::fmt;
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
            .user_agent(self.user_agent())
            .build()
            .map_err(OpenAccessError::Http)
    }
}

#[async_trait]
impl OpenAccessClient for ReqwestOpenAccessClient {
    async fn fetch_text(&self, url: &str) -> Result<String> {
        let client = self.client()?;
        let text = client
            .get(url)
            .send()
            .await?
            .error_for_status()?
            .text()
            .await?;
        Ok(text)
    }

    async fn fetch_json(&self, url: &str) -> Result<Value> {
        let client = self.client()?;
        let json = client
            .get(url)
            .send()
            .await?
            .error_for_status()?
            .json()
            .await?;
        Ok(json)
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
mod tests {
    use super::*;

    struct FakeClient {
        responses: std::collections::HashMap<String, String>,
    }

    #[async_trait]
    impl OpenAccessClient for FakeClient {
        async fn fetch_text(&self, url: &str) -> Result<String> {
            self.responses
                .get(url)
                .cloned()
                .ok_or_else(|| OpenAccessError::Unexpected(format!("no fake response for {url}")))
        }

        async fn fetch_json(&self, url: &str) -> Result<Value> {
            let text = self.fetch_text(url).await?;
            serde_json::from_str(&text).map_err(|e| OpenAccessError::Json(format!("bad json: {e}")))
        }
    }

    #[test]
    fn extract_doi_from_doi_org_url() {
        assert_eq!(
            extract_doi("https://doi.org/10.1234/example"),
            Some("10.1234/example".to_string())
        );
    }

    #[test]
    fn extract_doi_from_landing_page() {
        assert_eq!(
            extract_doi("https://publisher.example/journal/article/10.1234/example"),
            Some("10.1234/example".to_string())
        );
    }

    #[test]
    fn extract_doi_returns_none_for_random_url() {
        assert!(extract_doi("https://example.com/page").is_none());
    }

    #[tokio::test]
    async fn query_unpaywall_requires_email() {
        let client = FakeClient {
            responses: std::collections::HashMap::new(),
        };
        let result = query_unpaywall("10.1234/example", None, &client)
            .await
            .unwrap();
        assert!(result.is_none());
    }

    #[tokio::test]
    async fn query_unpaywall_parses_best_oa_location() {
        let encoded_doi = form_urlencoded::byte_serialize(b"10.1234/example").collect::<String>();
        let encoded_email = form_urlencoded::byte_serialize(b"oa@example.com").collect::<String>();
        let url = format!("https://api.unpaywall.org/v2/{encoded_doi}?email={encoded_email}");
        let json = serde_json::json!({
            "is_oa": true,
            "oa_status": "gold",
            "best_oa_location": {
                "url_for_pdf": "https://oa.example.com/paper.pdf",
                "url": "https://oa.example.com/paper",
                "license": "cc-by"
            }
        })
        .to_string();
        let client = FakeClient {
            responses: std::iter::once((url.to_string(), json)).collect(),
        };
        let result = query_unpaywall("10.1234/example", Some("oa@example.com"), &client)
            .await
            .unwrap();
        assert!(result.is_some());
        let recovered = result.unwrap();
        assert_eq!(recovered.url, "https://oa.example.com/paper.pdf");
        assert_eq!(recovered.source.to_string(), "unpaywall");
        assert_eq!(recovered.license.as_deref(), Some("cc-by"));
    }

    #[tokio::test]
    async fn query_unpaywall_returns_none_when_not_oa() {
        let encoded_doi = form_urlencoded::byte_serialize(b"10.1234/example").collect::<String>();
        let encoded_email = form_urlencoded::byte_serialize(b"oa@example.com").collect::<String>();
        let url = format!("https://api.unpaywall.org/v2/{encoded_doi}?email={encoded_email}");
        let json = serde_json::json!({ "is_oa": false }).to_string();
        let client = FakeClient {
            responses: std::iter::once((url.to_string(), json)).collect(),
        };
        let result = query_unpaywall("10.1234/example", Some("oa@example.com"), &client)
            .await
            .unwrap();
        assert!(result.is_none());
    }

    #[tokio::test]
    async fn query_europepmc_prefers_open_access_pdf() {
        let encoded_doi = form_urlencoded::byte_serialize(b"10.1234/example").collect::<String>();
        let url = format!(
            "https://www.ebi.ac.uk/europepmc/webservices/rest/search?query={encoded_doi}&format=json&resultType=core"
        );
        let json = serde_json::json!({
            "resultList": {
                "result": [{
                    "isOpenAccess": "Y",
                    "fullTextUrlList": {
                        "fullTextUrl": [
                            { "documentStyle": "html", "availability": "Free", "url": "https://html.example.com" },
                            { "documentStyle": "pdf", "availability": "Open Access", "url": "https://pdf.example.com" }
                        ]
                    }
                }]
            }
        })
        .to_string();
        let client = FakeClient {
            responses: std::iter::once((url.to_string(), json)).collect(),
        };
        let result = query_europepmc("10.1234/example", &client).await.unwrap();
        assert_eq!(
            result.map(|r| r.url),
            Some("https://pdf.example.com".to_string())
        );
    }

    #[tokio::test]
    async fn recover_open_access_prefers_unpaywall_then_europepmc() {
        let unpaywall_url = format!(
            "https://api.unpaywall.org/v2/{encoded_doi}?email={encoded_email}",
            encoded_doi = form_urlencoded::byte_serialize(b"10.1234/example").collect::<String>(),
            encoded_email = form_urlencoded::byte_serialize(b"oa@example.com").collect::<String>()
        );
        let europepmc_url = format!(
            "https://www.ebi.ac.uk/europepmc/webservices/rest/search?query={encoded_doi}&format=json&resultType=core",
            encoded_doi = form_urlencoded::byte_serialize(b"10.1234/example").collect::<String>()
        );
        let client = FakeClient {
            responses: [
                (unpaywall_url.to_string(), serde_json::json!({ "is_oa": false }).to_string()),
                (
                    europepmc_url.to_string(),
                    serde_json::json!({
                        "resultList": {
                            "result": [{
                                "isOpenAccess": "Y",
                                "fullTextUrlList": {
                                    "fullTextUrl": [
                                        { "documentStyle": "html", "availability": "Open Access", "url": "https://html.example.com" }
                                    ]
                                }
                            }]
                        }
                    })
                    .to_string(),
                ),
            ]
            .into_iter()
            .collect(),
        };
        let result = recover_open_access(
            "https://doi.org/10.1234/example",
            Some("oa@example.com"),
            &client,
        )
        .await
        .unwrap();
        assert_eq!(result.map(|r| r.source), Some(RecoverySource::EuropePmc));
    }
}
