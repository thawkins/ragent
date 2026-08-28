//! Web content fetching tool.
//!
//! Provides [`WebFetchTool`], which fetches the content of a URL via HTTP GET,
//! optionally converting HTML to plain text. Supports configurable timeout and
//! maximum content length.

use anyhow::{Context, Result, bail};
use serde_json::{Value, json};

use super::{Tool, ToolContext, ToolOutput};
use crate::masterfetch::security::validate_url;

/// Fetches web content from a URL, with optional HTML-to-text conversion.
pub struct WebFetchTool;

const DEFAULT_MAX_LENGTH: usize = 50_000;
const DEFAULT_TIMEOUT_SECS: u64 = 30;
const MAX_REDIRECTS: usize = 5;
// reason: kept as documentation of the max-redirect limit; redirects are
// handled implicitly by reqwest, so this constant is not read at runtime.
#[allow(dead_code)]
const _UNUSED_REDIRECTS: usize = MAX_REDIRECTS;
const USER_AGENT: &str = "ragent/0.1 (https://github.com/thawkins/ragent)";

/// Extract the article text from HTML using the `readability-rs` crate, which
/// removes navigation, ads, footers and other page chrome before converting the
/// main content to plain text.
fn extract_article_text(html: &str, url: &str) -> Option<(String, String)> {
    let parsed_url = url::Url::parse(url).ok()?;
    let mut input = std::io::Cursor::new(html.as_bytes());
    let readable = readability::extract(
        &mut input,
        &parsed_url,
        readability::ExtractOptions::default(),
    )
    .ok()?;
    let text = readable.text.trim().to_string();
    let title = readable.title.trim().to_string();
    if text.is_empty() {
        return None;
    }
    Some((text, title))
}

const MIN_READABILITY_TEXT_LEN: usize = 500;

#[async_trait::async_trait]
impl Tool for WebFetchTool {
    fn name(&self) -> &'static str {
        "webfetch"
    }

    /// # Errors
    ///
    /// Returns an error if the description string cannot be converted or returned.
    fn description(&self) -> &'static str {
        "Fetch the content of a URL via HTTP GET. REQUIRED parameter: 'url' (string). \
             Optional: 'format' (string enum raw/text, default text), 'max_length' \
             (integer, default 50000), and 'timeout' (integer seconds, default 30). \
             HTML is automatically converted to plain text unless format is 'raw'. \
             Common gotcha: only HTTP/HTTPS URLs are supported; invalid or missing \
             schemes return an error."
    }

    fn parameters_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "url": {
                    "type": "string",
                    "description": "The URL to fetch (required)"
                },
                "format": {
                    "type": "string",
                    "description": "Output format: 'raw' (unchanged), 'text' (HTML→plain text). Default: 'text'",
                    "enum": ["raw", "text"]
                },
                "max_length": {
                    "type": "integer",
                    "description": "Maximum characters to return (default: 50000)"
                },
                "timeout": {
                    "type": "integer",
                    "description": "Request timeout in seconds (default: 30)"
                }
            },
            "required": ["url"],
            "additionalProperties": false
        })
    }
    /// # Errors
    ///
    /// Returns an error if the category string cannot be converted or returned.
    fn permission_category(&self) -> &'static str {
        "web"
    }

    /// # Errors
    ///
    /// Returns an error if the `url` parameter is missing or uses an unsupported
    /// scheme, if the HTTP client build fails, if the request fails, if the response
    /// status is not successful, or if content processing fails.
    async fn execute(&self, input: Value, _ctx: &ToolContext) -> Result<ToolOutput> {
        let url = input["url"]
            .as_str()
            .context("Missing required 'url' parameter")?;

        // C-001: Enforce SSRF / URL safety guard before any outbound request.
        validate_url(url).with_context(|| format!("URL failed security validation: {url}"))?;

        // Validate URL scheme
        if !url.starts_with("http://") && !url.starts_with("https://") {
            bail!(
                "Only http:// and https:// URLs are supported. The provided URL '{url}' uses an unsupported scheme."
            );
        }

        let format = input["format"].as_str().unwrap_or("text");
        let max_length = input["max_length"]
            .as_u64()
            .map_or(DEFAULT_MAX_LENGTH, |v| v as usize);
        let timeout_secs = input["timeout"].as_u64().unwrap_or(DEFAULT_TIMEOUT_SECS);

        // M-014: reuse the shared reqwest client singleton (reusing the
        // connection pool + TLS session cache) instead of building a fresh
        // client per call. The per-request timeout is applied via `RequestBuilder`.
        let client = crate::masterfetch::http::shared_client()
            .map_err(|e| anyhow::anyhow!("Failed to build HTTP client: {e}"))?;

        let response = client
            .get(url)
            .timeout(std::time::Duration::from_secs(timeout_secs))
            .header(reqwest::header::USER_AGENT, USER_AGENT)
            .send()
            .await
            .with_context(|| format!("Failed to fetch URL: {url}"))?;

        let status = response.status().as_u16();
        let content_type = response
            .headers()
            .get("content-type")
            .and_then(|v| v.to_str().ok())
            .unwrap_or("unknown")
            .to_string();
        let content_length = response.content_length();

        if !response.status().is_success() {
            bail!(
                "HTTP {} fetching {}: {}",
                status,
                url,
                response.status().canonical_reason().unwrap_or("Unknown")
            );
        }

        // M-015: check the declared content length up-front and cap the body
        // at `max_length` + a small headroom so a huge file is not fully
        // downloaded and thrown away. Stream the body and stop once the cap
        // is reached.
        const CAP_HEADROOM: usize = 256 * 1024;
        let download_cap = max_length.saturating_add(CAP_HEADROOM);
        if let Some(len) = content_length
            && len as usize > download_cap
        {
            tracing::debug!(
                url,
                declared = len,
                cap = download_cap,
                "webfetch: declared content length exceeds cap; reading capped stream"
            );
        }
        let mut body_bytes: Vec<u8> = Vec::new();
        {
            use futures::StreamExt;
            let mut stream = response.bytes_stream();
            while let Some(chunk) = stream.next().await {
                let chunk =
                    chunk.with_context(|| format!("Failed to read response body from: {url}"))?;
                let remaining = download_cap.saturating_sub(body_bytes.len());
                if remaining == 0 {
                    break;
                }
                let take = chunk.len().min(remaining);
                body_bytes.extend_from_slice(&chunk[..take]);
                if body_bytes.len() >= download_cap {
                    break;
                }
            }
        }
        let body = String::from_utf8_lossy(&body_bytes).into_owned();
        // Ensure the final string still respects `max_length` (the read cap is
        // a safety bound; `max_length` is the user-visible truncation).
        let body = if body.len() > max_length {
            let mut s: String = body.chars().take(max_length).collect();
            s.push('…');
            s
        } else {
            body
        };

        let is_html =
            content_type.contains("text/html") || content_type.contains("application/xhtml");

        // The streaming read above already capped the body at
        // `download_cap` (M-015). Reuse the existing post-processing that
        // handles HTML→text conversion and final `max_length` truncation.
        let (processed, extracted_title) = if is_html && format != "raw" {
            match extract_article_text(&body, url) {
                Some((text, title)) if text.len() >= MIN_READABILITY_TEXT_LEN => {
                    (text, Some(title))
                }
                Some((_, title)) => {
                    tracing::debug!(
                        url,
                        "readability extraction produced very short text; falling back to html2text"
                    );
                    (html_to_text(&body), Some(title))
                }
                None => {
                    tracing::debug!(
                        url,
                        "readability extraction failed; falling back to html2text"
                    );
                    (html_to_text(&body), None)
                }
            }
        } else {
            (body, None)
        };

        // Truncate at a char boundary
        let truncated = if processed.len() > max_length {
            let end = processed
                .char_indices()
                .map(|(i, _)| i)
                .take_while(|&i| i <= max_length)
                .last()
                .unwrap_or(0);
            let mut s = processed[..end].to_string();
            s.push_str("\n\n[Content truncated]");
            s
        } else {
            processed
        };

        let line_count = truncated.lines().count();
        let byte_count = truncated.len();

        let mut metadata = json!({
            "url": url,
            "http_status": status,
            "content_type": content_type,
            "content_length": content_length,
            "line_count": line_count,
            "byte_count": byte_count,
        });
        if let Some(title) = extracted_title {
            metadata["title"] = json!(title);
        }

        Ok(ToolOutput {
            content: truncated,
            metadata: Some(metadata),
        })
    }
}

/// Convert HTML to plain text using html2text.
///
/// `html2text` can panic on some real-world HTML documents, so the rendering
/// runs on a dedicated OS thread via
/// [`crate::masterfetch::extractor::run_html2text_isolated`], keeping any
/// panic off async runtime threads. Falls back to a simple tag stripper if
/// html2text errors or panics.
/// This is the legacy fallback when `readability-rs` cannot extract an article.
fn html_to_text(html: &str) -> String {
    match crate::masterfetch::extractor::run_html2text_isolated(html.to_string()) {
        Ok(text) => text,
        _ => {
            // Fallback: strip tags manually when html2text fails or panics
            strip_tags(html)
        }
    }
}

/// Minimal fallback tag stripper for when html2text fails.
///
/// Re-exported from [`ragent_types::html::strip_tags`] (DUPPLAN.md Milestone F).
pub use ragent_types::html::strip_tags;
