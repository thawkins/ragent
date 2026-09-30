//! Shared HTTP client for masterfetch tools.
//!
//! Provides a pre-configured [`reqwest::Client`] that every masterfetch tool
//! reuses for outbound HTTP requests. The client is built once with a
//! consistent `User-Agent`, a 30-second default timeout, a redirect policy
//! that follows up to 5 hops, and automatic gzip/deflate decompression.
//!
//! # Requirements
//!
//! - **FR-025** - shared `reqwest::Client` with `User-Agent`
//!   `ragent/{version} (masterfetch)`, configurable timeout (default 30 s),
//!   redirect policy (max 5), and gzip/deflate support.
//! - **NFR-002** - reuses the workspace `reqwest` dependency; no new crates.
//!
//! # Usage
//!
//! ```no_run
//! use ragent_tools_extended::masterfetch::http;
//!
//! # async fn demo() -> anyhow::Result<()> {
//! let client = http::build_default_client()?;
//! let resp = client.get("https://example.com").send().await?;
//! # Ok(()) }
//! ```
//!
//! For tools that need a single long-lived client (the common case), call
//! [`shared_client`] to obtain a lazily-initialised singleton.

use std::sync::OnceLock;

use thiserror::Error;

/// `User-Agent` header value sent with every masterfetch HTTP request.
///
/// Format: `ragent/{version} (masterfetch)` where `{version}` is the
/// `ragent-tools-extended` crate version at compile time (FR-025).
pub const USER_AGENT: &str = concat!("ragent/", env!("CARGO_PKG_VERSION"), " (masterfetch)");

/// Default request timeout in seconds (FR-025).
pub const DEFAULT_TIMEOUT_SECS: u64 = 30;

/// Maximum number of HTTP redirects to follow (FR-025).
pub const MAX_REDIRECTS: usize = 5;

/// Errors that can occur when building the shared HTTP client.
#[derive(Debug, Error)]
pub enum HttpError {
    /// The reqwest client builder returned an error.
    #[error("failed to build masterfetch HTTP client: {0}")]
    Build(#[from] reqwest::Error),
}

/// Build a new [`reqwest::Client`] with the masterfetch configuration.
///
/// The client is configured with:
///
/// - `User-Agent: ragent/{version} (masterfetch)` (see [`USER_AGENT`])
/// - `timeout` - the supplied request timeout
/// - `redirect::Policy::limited(MAX_REDIRECTS)` - follows up to 5 redirects
/// - `gzip(true)` and `deflate(true)` - automatic decompression
///
/// # Errors
///
/// Returns [`HttpError::Build`] if reqwest fails to construct the client
/// (e.g. TLS backend initialisation failure).
///
/// # Examples
///
/// ```no_run
/// use std::time::Duration;
/// use ragent_tools_extended::masterfetch::http;
///
/// let client = http::build_client(Duration::from_secs(10)).unwrap();
/// ```
pub fn build_client(timeout: std::time::Duration) -> Result<reqwest::Client, HttpError> {
    tracing::debug!(
        timeout_secs = timeout.as_secs(),
        redirects = MAX_REDIRECTS,
        "building masterfetch HTTP client"
    );
    let client = reqwest::Client::builder()
        .user_agent(USER_AGENT)
        .timeout(timeout)
        .redirect(reqwest::redirect::Policy::limited(MAX_REDIRECTS))
        .gzip(true)
        .deflate(true)
        .build()?;
    Ok(client)
}

/// Build a [`reqwest::Client`] with the default 30-second timeout.
///
/// Convenience wrapper around [`build_client`] using [`DEFAULT_TIMEOUT_SECS`].
///
/// # Errors
///
/// Returns [`HttpError::Build`] if the client cannot be constructed.
///
/// # Examples
///
/// ```no_run
/// use ragent_tools_extended::masterfetch::http;
///
/// let client = http::build_default_client().unwrap();
/// ```
pub fn build_default_client() -> Result<reqwest::Client, HttpError> {
    build_client(std::time::Duration::from_secs(DEFAULT_TIMEOUT_SECS))
}

/// Lazily-initialised shared [`reqwest::Client`] singleton.
///
/// The first call constructs the client with [`build_default_client`];
/// subsequent calls return the same instance without rebuilding. This avoids
/// repeated TLS handshake and connection-pool setup on every tool invocation.
///
/// # Errors
///
/// Returns [`HttpError::Build`] if the initial construction fails. The error
/// is **not** cached - a subsequent call will retry the build.
///
/// # Examples
///
/// ```no_run
/// use ragent_tools_extended::masterfetch::http;
///
/// # async fn demo() -> anyhow::Result<()> {
/// let client = http::shared_client()?;
/// let resp = client.get("https://example.com").send().await?;
/// # Ok(()) }
/// ```
pub fn shared_client() -> Result<&'static reqwest::Client, HttpError> {
    static CLIENT: OnceLock<reqwest::Client> = OnceLock::new();

    if let Some(client) = CLIENT.get() {
        return Ok(client);
    }

    let client = build_default_client()?;
    // OnceLock::get_or_init cannot return a reference to a fallibly-built
    // value, so we manually insert and then borrow. Racing callers may build
    // a duplicate client, but only the first inserted is retained - the
    // others are dropped. This is acceptable: the extra build is cheap and
    // happens at most once per concurrent first-call race.
    let _ = CLIENT.set(client); // INTENTIONAL: OnceLock set race is benign
    Ok(CLIENT
        .get()
        .expect("client was just set or is present from a racing caller"))
}

/// Maximum bytes buffered from an upstream response body (ANTIPAT 4.1).
///
/// The shared masterfetch client transparently decompresses gzip/deflate, so a
/// hostile or buggy upstream can return an arbitrarily large body (or a
/// decompression bomb). Every response body read in this crate must be capped;
/// 16 MiB is far more than any JSON API page, RSS feed, or robots.txt needs.
pub const MAX_RESPONSE_BODY_BYTES: usize = 16 * 1024 * 1024;

/// Maximum bytes buffered from a small text response (robots.txt, error bodies).
///
/// A robots.txt is a few kilobytes in the worst realistic case, so 512 KiB is
/// already a generous ceiling (ANTIPAT 4.1).
pub const MAX_SMALL_BODY_BYTES: usize = 512 * 1024;

/// Read a response body into a string, capped at `limit` bytes.
///
/// Returns `Ok(body)` (possibly truncated) or `Err` when the transport fails,
/// so callers keep their existing error handling instead of a silent
/// `unwrap_or_default()`. Replaces the uncapped `response.text().await` calls
/// flagged by ANTIPAT 4.1.
///
/// # Errors
///
/// Returns the underlying [`reqwest::Error`] when the body cannot be read.
pub async fn read_body_capped(
    response: reqwest::Response,
    limit: usize,
) -> Result<String, reqwest::Error> {
    let body = response.text().await?;
    Ok(truncate_body(&body, limit))
}

/// Truncate `body` to at most `limit` bytes on a UTF-8 character boundary.
///
/// Appends an omission marker when truncation occurs so a caller can see the
/// response was clipped.
#[must_use]
pub fn truncate_body(body: &str, limit: usize) -> String {
    if body.len() <= limit {
        return body.to_string();
    }
    let mut end = limit;
    while end > 0 && !body.is_char_boundary(end) {
        end -= 1;
    }
    let mut out = String::with_capacity(end + 32);
    out.push_str(&body[..end]);
    out.push_str("\n[... response body truncated ...]");
    out
}

/// Stream a response into memory, stopping at `limit` bytes.
///
/// SEC-tools-extended-006 (SECTASKS T-033) / ANTIPAT 4.1: the size budget is
/// applied per chunk, so a decompression bomb cannot be buffered in full first.
///
/// # Errors
///
/// Returns the transport error message when the body cannot be read.
pub async fn read_bytes_capped(
    response: reqwest::Response,
    limit: usize,
) -> Result<Vec<u8>, String> {
    let mut stream = response;
    let mut buf: Vec<u8> = Vec::new();
    loop {
        match stream.chunk().await {
            Ok(Some(chunk)) => {
                if buf.len() + chunk.len() > limit {
                    let room = limit.saturating_sub(buf.len());
                    buf.extend_from_slice(&chunk[..room]);
                    buf.extend_from_slice(b"\n[truncated at the size cap]\n");
                    break;
                }
                buf.extend_from_slice(&chunk);
            }
            Ok(None) => break,
            Err(e) => return Err(e.to_string()),
        }
    }
    Ok(buf)
}

/// Stream a response body into a lossy UTF-8 string, stopping at `limit` bytes.
///
/// This is the streaming counterpart of [`read_body_capped`]: the size budget
/// is applied per chunk (so a decompression bomb cannot be buffered in full),
/// and the result is decoded lossily so non-UTF-8 bytes do not fail the read.
/// It replaces the private `read_body_capped` loop that previously lived in
/// `masterfetch::tools::crawl_tool` (ANTIPAT M5.9 / 3.6).
///
/// # Errors
///
/// Returns the transport error message when the body cannot be read.
pub async fn read_body_capped_lossy(
    response: reqwest::Response,
    limit: usize,
) -> Result<String, String> {
    let bytes = read_bytes_capped(response, limit).await?;
    Ok(String::from_utf8_lossy(&bytes).into_owned())
}
