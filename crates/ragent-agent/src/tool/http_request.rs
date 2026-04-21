//! Full HTTP client tool.
//!
//! Provides [`HttpRequestTool`], which performs an HTTP request (any method)
//! with optional headers and a request body, returning the response status,
//! headers, and body.  Backed by the [`reqwest`] async HTTP client.

use anyhow::{Context, Result};
use reqwest::header::{HeaderMap, HeaderName, HeaderValue};
use serde_json::{Value, json};
use std::str::FromStr as _;
use std::time::Duration;

use super::{Tool, ToolContext, ToolOutput};

const DEFAULT_TIMEOUT_SECS: u64 = 30;
const MAX_BODY_BYTES: usize = 1024 * 1024; // 1 MiB response cap

/// Perform an HTTP request with configurable method, headers, and body.
pub struct HttpRequestTool;

#[async_trait::async_trait]
impl Tool for HttpRequestTool {
    fn name(&self) -> &'static str {
        "http_request"
    }

    fn description(&self) -> &'static str {
        "Perform an HTTP request (GET, POST, PUT, PATCH, DELETE, HEAD, OPTIONS). \
         Returns the response status code, selected response headers, and the \
         response body (truncated at 1 MiB). For simple web page fetching prefer \
         'webfetch'; use this tool when you need full control over method/headers/body."
    }

    fn parameters_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "url": {
                    "type": "string",
                    "description": "Full URL to request (including scheme, e.g. https://...)"
                },
                "method": {
                    "type": "string",
                    "description": "HTTP method (default: GET)",
                    "enum": ["GET", "POST", "PUT", "PATCH", "DELETE", "HEAD", "OPTIONS"]
                },
                "headers": {
                    "type": "object",
                    "description": "Additional request headers as a key-value map",
                    "additionalProperties": { "type": "string" }
                },
                "body": {
                    "type": "string",
                    "description": "Request body (for POST/PUT/PATCH)"
                },
                "timeout": {
                    "type": "integer",
                    "description": "Timeout in seconds (default: 30)"
                }
            },
            "required": ["url"]
        })
    }

    fn permission_category(&self) -> &'static str {
        "network:fetch"
    }

    async fn execute(&self, input: Value, _ctx: &ToolContext) -> Result<ToolOutput> {
        let url = input["url"]
            .as_str()
            .context("Missing required 'url' parameter")?;
        let method_str = input["method"].as_str().unwrap_or("GET").to_uppercase();
        let timeout_secs = input["timeout"].as_u64().unwrap_or(DEFAULT_TIMEOUT_SECS);

        let method = reqwest::Method::from_str(&method_str)
            .with_context(|| format!("Invalid HTTP method: {method_str}"))?;

        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(timeout_secs))
            .build()
            .context("Failed to build HTTP client")?;

        let mut request = client.request(method, url);

        // Apply custom headers
        if let Some(headers_obj) = input["headers"].as_object() {
            let mut header_map = HeaderMap::new();
            for (k, v) in headers_obj {
                if let Some(val_str) = v.as_str() {
                    let name = HeaderName::from_str(k)
                        .with_context(|| format!("Invalid header name: {k}"))?;
                    let value = HeaderValue::from_str(val_str)
                        .with_context(|| format!("Invalid header value for {k}"))?;
                    header_map.insert(name, value);
                }
            }
            request = request.headers(header_map);
        }

        // Apply body
        if let Some(body_str) = input["body"].as_str() {
            request = request.body(body_str.to_string());
        }

        let response = request
            .send()
            .await
            .with_context(|| format!("HTTP request to {url} failed"))?;

        let status = response.status();
        let status_code = status.as_u16();

        // Collect a few useful response headers
        let content_type = response
            .headers()
            .get("content-type")
            .and_then(|v| v.to_str().ok())
            .unwrap_or("")
            .to_string();
        let content_length = response
            .headers()
            .get("content-length")
            .and_then(|v| v.to_str().ok())
            .and_then(|s| s.parse::<u64>().ok());

        let bytes = response
            .bytes()
            .await
            .with_context(|| "Failed to read response body")?;

        let truncated = bytes.len() > MAX_BODY_BYTES;
        let body_slice = &bytes[..bytes.len().min(MAX_BODY_BYTES)];
        let body = String::from_utf8_lossy(body_slice).to_string();

        let mut content = format!(
            "HTTP {status_code} {}\n",
            status.canonical_reason().unwrap_or("")
        );
        content.push_str(&format!("Content-Type: {content_type}\n"));
        if let Some(cl) = content_length {
            content.push_str(&format!("Content-Length: {cl}\n"));
        }
        content.push('\n');
        content.push_str(&body);
        if truncated {
            content.push_str("\n\n[Response truncated at 1 MiB]");
        }

        let line_count = content.lines().count();

        Ok(ToolOutput {
            content,
            metadata: Some(json!({
                "http_status": status_code,
                "content_type": content_type,
                "byte_count": bytes.len(),
                "truncated": truncated,
                "line_count": line_count,
            })),
        })
    }
}
