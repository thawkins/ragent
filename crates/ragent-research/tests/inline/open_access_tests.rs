//! Inline tests for `open_access.rs` - compiled inside the module tree
//! via the `#[path]` delegation left in the source file
//! (ANTIPAT M2 inline-test migration; no behaviour change).

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
