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
use crate::masterfetch::http::DEFAULT_TIMEOUT_SECS;
use crate::masterfetch::security::validate_url;

const MAX_BODY_BYTES: usize = 1024 * 1024; // 1 MiB response cap

/// Perform an HTTP request with configurable method, headers, and body.
pub struct HttpRequestTool;

/// Headers a caller may not set on an `http_request`.
///
/// SEC-tools-extended-004 (SECTASKS T-056): routing and credential headers.
const DENIED_REQUEST_HEADERS: &[&str] = &[
    "host",
    "cookie",
    "cookie2",
    "authorization",
    "proxy-authorization",
    "proxy-authenticate",
    "proxy-connection",
    "connection",
    "transfer-encoding",
    "upgrade",
    "te",
    "trailer",
    "x-forwarded-for",
    "x-forwarded-host",
    "x-forwarded-proto",
    "x-real-ip",
];

/// Whether `name` is a routing/credential header the caller may not set.
#[must_use]
fn is_denied_request_header(name: &str) -> bool {
    let lower = name.trim().to_ascii_lowercase();
    DENIED_REQUEST_HEADERS.contains(&lower.as_str()) || lower.starts_with("proxy-")
}

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
            "required": ["url"],
            "additionalProperties": false
        })
    }

    fn permission_category(&self) -> &'static str {
        "network:fetch"
    }

    async fn execute(&self, input: Value, _ctx: &ToolContext) -> Result<ToolOutput> {
        let url = input["url"]
            .as_str()
            .context("Missing required 'url' parameter")?;

        // C-001: Enforce SSRF / URL safety guard before any outbound request.
        validate_url(url).with_context(|| format!("URL failed security validation: {url}"))?;

        let method_str = input["method"].as_str().unwrap_or("GET").to_uppercase();
        let timeout_secs = input["timeout"].as_u64().unwrap_or(DEFAULT_TIMEOUT_SECS);

        let method = reqwest::Method::from_str(&method_str)
            .with_context(|| format!("Invalid HTTP method: {method_str}"))?;

        // M-014: reuse the shared reqwest client singleton (connection pool +
        // TLS session cache) instead of building a fresh client per call. The
        // per-request timeout is applied via `RequestBuilder`. Note this also
        // means requests inherit the shared client's behaviour: a 5-redirect
        // limit, transparent gzip/deflate decompression, and the versioned
        // masterfetch `User-Agent` header.
        let client =
            crate::masterfetch::http::shared_client().context("Failed to build HTTP client")?;

        let mut request = client
            .request(method, url)
            .timeout(Duration::from_secs(timeout_secs));

        // Apply custom headers.
        //
        // SEC-tools-extended-004 (SECTASKS T-056): reject caller-supplied
        // routing/credential headers. `Host` re-targets the request to a
        // virtual host behind the validated IP, `Cookie`/`Authorization`
        // forward credentials to an attacker-chosen origin, and the
        // `Proxy-*`/hop-by-hop headers rewrite the request path. The tool's
        // SSRF check validates the URL, not these.
        if let Some(headers_obj) = input["headers"].as_object() {
            let mut header_map = HeaderMap::new();
            for (k, v) in headers_obj {
                if let Some(val_str) = v.as_str() {
                    if is_denied_request_header(k) {
                        anyhow::bail!(
                            "header '{k}' is not permitted: it controls request routing or \
                             carries credentials"
                        );
                    }
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
