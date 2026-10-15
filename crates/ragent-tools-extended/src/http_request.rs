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

/// The only request headers a caller may set on an `http_request`.
///
/// SEC-035: this was previously a deny-list, so any credential header not named
/// (`x-api-key`, `Private-Token`, `x-goog-api-key`, ...) was forwarded - and a
/// cross-host redirect could carry it to a third party. An allowlist is the only
/// safe shape: everything else (routing, hop-by-hop, and credential headers) is
/// refused.
const ALLOWED_REQUEST_HEADERS: &[&str] =
    &["accept", "accept-language", "content-type", "user-agent"];

/// Whether `name` is a request header the caller may set.
#[must_use]
fn is_allowed_request_header(name: &str) -> bool {
    ALLOWED_REQUEST_HEADERS.contains(&name.trim().to_ascii_lowercase().as_str())
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
        // SEC-035: use a no-redirect client so a caller-supplied header (or the
        // request body on a 307/308) cannot be replayed against a second host the
        // URL redirects to.
        let client = reqwest::Client::builder()
            .user_agent(crate::masterfetch::http::USER_AGENT)
            .timeout(Duration::from_secs(timeout_secs))
            .redirect(reqwest::redirect::Policy::none())
            .build()
            .context("Failed to build HTTP client")?;

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
                    if !is_allowed_request_header(k) {
                        anyhow::bail!(
                            "header '{k}' is not permitted (allowed: {})",
                            ALLOWED_REQUEST_HEADERS.join(", ")
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

        // SEC-036: stream the body, stopping at the cap, instead of buffering the
        // whole (possibly transparently-decompressed) response first.
        let mut response = response;
        let mut buf: Vec<u8> = Vec::new();
        let mut truncated = false;
        while let Some(chunk) = response
            .chunk()
            .await
            .with_context(|| "Failed to read response body")?
        {
            if buf.len() + chunk.len() > MAX_BODY_BYTES {
                let room = MAX_BODY_BYTES.saturating_sub(buf.len());
                buf.extend_from_slice(&chunk[..room]);
                truncated = true;
                break;
            }
            buf.extend_from_slice(&chunk);
        }
        let body = String::from_utf8_lossy(&buf).to_string();

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
                "byte_count": buf.len(),
                "truncated": truncated,
                "line_count": line_count,
            })),
        })
    }
}
