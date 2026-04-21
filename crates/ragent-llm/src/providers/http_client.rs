//! HTTP client configuration for LLM providers.
//!
//! This module provides a centralized way to configure HTTP clients with
//! appropriate settings for concurrent sub-agent execution, including:
//! - Connection pool limits to prevent HTTP/2 race conditions
//! - Request/response timeouts
//! - HTTP/2 configuration
//! - Retry logic with exponential backoff

use std::time::Duration;

use anyhow::Result;
use reqwest::Client;
use tracing::warn;

/// Maximum connections per host.
const MAX_IDLE_PER_HOST: usize = 8;

/// Connection timeout in seconds.
const CONNECT_TIMEOUT_SECS: u64 = 30;

/// Request timeout in seconds.
const REQUEST_TIMEOUT_SECS: u64 = 120;

/// Creates a properly configured HTTP client for LLM provider communication.
///
/// The client is configured with:
/// - Connection pool limits to prevent HTTP/2 race conditions
/// - Timeouts for connection establishment and requests
/// - TCP keep-alive for long-running connections
///
/// # Errors
///
/// Returns an error if the HTTP client builder fails.
///
/// # Examples
///
/// ```
/// use ragent_llm::provider::http_client::create_http_client;
///
/// let client = create_http_client();
/// ```
#[must_use]
pub fn create_http_client() -> Client {
    Client::builder()
        .pool_max_idle_per_host(MAX_IDLE_PER_HOST)
        .pool_idle_timeout(Duration::from_secs(90))
        .connect_timeout(Duration::from_secs(CONNECT_TIMEOUT_SECS))
        .timeout(Duration::from_secs(REQUEST_TIMEOUT_SECS))
        .tcp_keepalive(Duration::from_secs(60))
        .build()
        .unwrap_or_else(|e| {
            warn!(error = %e, "Failed to build HTTP client with custom settings, using defaults");
            Client::new()
        })
}

/// Creates an HTTP client for streaming LLM responses.
///
/// Similar to [`create_http_client`] but **without** a global request timeout.
/// Streaming responses can run for minutes; the per-chunk timeout is managed
/// by each provider individually (via `tokio::time::timeout` on each chunk).
/// The global reqwest timeout would otherwise kill long-running streams after
/// 120 seconds, causing "error decoding response body" failures.
#[must_use]
pub fn create_streaming_http_client() -> Client {
    Client::builder()
        .pool_max_idle_per_host(MAX_IDLE_PER_HOST)
        .pool_idle_timeout(Duration::from_secs(90))
        .connect_timeout(Duration::from_secs(CONNECT_TIMEOUT_SECS))
        .tcp_keepalive(Duration::from_secs(60))
        .build()
        .unwrap_or_else(|e| {
            warn!(error = %e, "Failed to build streaming HTTP client, using defaults");
            Client::new()
        })
}

/// Executes an HTTP request with retry logic for transient failures.
///
/// Retries on:
/// - 5xx server errors
/// - Connection errors
/// - Timeout errors
///
/// Uses exponential backoff: 1s, 2s, 4s, 8s (max 4 retries)
///
/// # Errors
///
/// Returns an error if all retries are exhausted or the error is not retryable.
///
/// # Examples
///
/// ```no_run
/// use ragent_llm::provider::http_client::execute_with_retry;
/// use reqwest::Client;
///
/// async fn example() -> anyhow::Result<reqwest::Response> {
///     let client = Client::new();
///     let response = execute_with_retry(
///         || async {
///             client.get("https://api.example.com/data").send().await
///         },
///         4,
///     ).await?;
///     Ok(response)
/// }
/// ```
pub async fn execute_with_retry<F, Fut>(
    request_fn: F,
    max_retries: u32,
) -> Result<reqwest::Response>
where
    F: Fn() -> Fut,
    Fut: std::future::Future<Output = Result<reqwest::Response, reqwest::Error>>,
{
    let mut last_error = None;

    for attempt in 0..=max_retries {
        if attempt > 0 {
            let delay = Duration::from_millis(500 * (1 << (attempt - 1).min(4)));
            tracing::debug!(
                attempt = attempt,
                max_retries = max_retries,
                delay_ms = delay.as_millis() as u64,
                "Retrying HTTP request after transient failure"
            );
            tokio::time::sleep(delay).await;
        }

        match request_fn().await {
            Ok(response) => {
                let status = response.status();

                // Check if we should retry based on status code
                if status.is_server_error() && attempt < max_retries {
                    let body = response.text().await.unwrap_or_default();
                    last_error = Some(anyhow::anyhow!(
                        "HTTP {} (attempt {}): {}",
                        status,
                        attempt + 1,
                        body
                    ));
                    continue;
                }

                // For client errors, don't retry
                if status.is_client_error() {
                    let body = response.text().await.unwrap_or_default();
                    return Err(anyhow::anyhow!("HTTP {} (not retryable): {}", status, body));
                }

                return Ok(response);
            }
            Err(e) if is_retryable_error(&e) && attempt < max_retries => {
                last_error = Some(anyhow::anyhow!("Request failed: {}", e));
                continue;
            }
            Err(e) => {
                return Err(anyhow::anyhow!("Request failed (not retryable): {}", e));
            }
        }
    }

    Err(last_error.unwrap_or_else(|| anyhow::anyhow!("All retries exhausted")))
}

/// Determines if an error is retryable.
///
/// Retryable errors include:
/// - Connection errors
/// - Timeout errors
/// - HTTP/2 protocol errors
/// - Body/decode errors (e.g. truncated response)
fn is_retryable_error(error: &reqwest::Error) -> bool {
    if error.is_timeout() {
        return true;
    }

    if error.is_connect() {
        return true;
    }

    // Body decoding errors are transient (truncated response, broken encoding)
    if error.is_body() || error.is_decode() {
        return true;
    }

    // Check for HTTP/2 protocol errors or other transient failures in the source chain
    if let Some(source) = error.source() {
        let source_str = source.to_string().to_lowercase();
        if source_str.contains("http2")
            || source_str.contains("h2")
            || source_str.contains("connection reset")
            || source_str.contains("broken pipe")
        {
            return true;
        }
    }

    // Check status code if available
    if let Some(status) = error.status() {
        return status.is_server_error() || status == reqwest::StatusCode::TOO_MANY_REQUESTS;
    }

    false
}

use std::error::Error;
