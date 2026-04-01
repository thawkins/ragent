//! Web content fetching tool.
//!
//! Provides [`WebFetchTool`], which fetches the content of a URL via HTTP GET,
//! optionally converting HTML to plain text. Supports configurable timeout and
//! maximum content length.

use anyhow::{Context, Result, bail};
use serde_json::{Value, json};

use super::{Tool, ToolContext, ToolOutput};

/// Fetches web content from a URL, with optional HTML-to-text conversion.
pub struct WebFetchTool;

const DEFAULT_MAX_LENGTH: usize = 50_000;
const DEFAULT_TIMEOUT_SECS: u64 = 30;
const MAX_REDIRECTS: usize = 5;
const TEXT_WIDTH: usize = 120;
const USER_AGENT: &str = "ragent/0.1 (https://github.com/thawkins/ragent)";

#[async_trait::async_trait]
impl Tool for WebFetchTool {
    fn name(&self) -> &str {
        "webfetch"
    }

    /// # Errors
    ///
    /// Returns an error if the description string cannot be converted or returned.
    fn description(&self) -> &str {
        "Fetch the content of a URL via HTTP GET. HTML is automatically converted \
         to plain text unless format is set to 'raw'. Supports timeout and max \
         content length."
    }

    fn parameters_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "url": {
                    "type": "string",
                    "description": "The URL to fetch"
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
            "required": ["url"]
        })
    }

    /// # Errors
    ///
    /// Returns an error if the category string cannot be converted or returned.
    fn permission_category(&self) -> &str {
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

        // Validate URL scheme
        if !url.starts_with("http://") && !url.starts_with("https://") {
            bail!(
                "Only http:// and https:// URLs are supported. The provided URL '{}' uses an unsupported scheme.",
                url
            );
        }

        let format = input["format"].as_str().unwrap_or("text");
        let max_length = input["max_length"]
            .as_u64()
            .map(|v| v as usize)
            .unwrap_or(DEFAULT_MAX_LENGTH);
        let timeout_secs = input["timeout"].as_u64().unwrap_or(DEFAULT_TIMEOUT_SECS);

        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(timeout_secs))
            .redirect(reqwest::redirect::Policy::limited(MAX_REDIRECTS))
            .user_agent(USER_AGENT)
            .build()
            .context("Failed to build HTTP client")?;

        let response = client
            .get(url)
            .send()
            .await
            .with_context(|| format!("Failed to fetch URL: {}", url))?;

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

        let body = response
            .text()
            .await
            .with_context(|| format!("Failed to read response body from: {}", url))?;

        let is_html =
            content_type.contains("text/html") || content_type.contains("application/xhtml");

        let processed = if is_html && format != "raw" {
            html_to_text(&body)
        } else {
            body
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

        let lines = truncated.lines().count();

        Ok(ToolOutput {
            content: truncated,
            metadata: Some(json!({
                "url": url,
                "status": status,
                "content_type": content_type,
                "content_length": content_length,
                "lines": lines,
            })),
        })
    }
}

/// Convert HTML to plain text using html2text.
fn html_to_text(html: &str) -> String {
    html2text::from_read(html.as_bytes(), TEXT_WIDTH).unwrap_or_else(|_| {
        // Fallback: strip tags manually
        strip_tags(html)
    })
}

/// Minimal fallback tag stripper for when html2text fails.
fn strip_tags(html: &str) -> String {
    let mut result = String::with_capacity(html.len());
    let mut in_tag = false;
    for ch in html.chars() {
        match ch {
            '<' => in_tag = true,
            '>' => in_tag = false,
            _ if !in_tag => result.push(ch),
            _ => {}
        }
    }
    result
}
