//! Shared HTTP client for masterfetch tools.
//!
//! Provides a pre-configured [`reqwest::Client`] that every masterfetch tool
//! reuses for outbound HTTP requests. The client is built once with a
//! consistent `User-Agent`, a 30-second default timeout, a redirect policy
//! that follows up to 5 hops, and automatic gzip/deflate decompression.
//!
//! # Requirements
//!
//! - **FR-025** — shared `reqwest::Client` with `User-Agent`
//!   `ragent/{version} (masterfetch)`, configurable timeout (default 30 s),
//!   redirect policy (max 5), and gzip/deflate support.
//! - **NFR-002** — reuses the workspace `reqwest` dependency; no new crates.
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
/// - `timeout` — the supplied request timeout
/// - `redirect::Policy::limited(MAX_REDIRECTS)` — follows up to 5 redirects
/// - `gzip(true)` and `deflate(true)` — automatic decompression
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
/// is **not** cached — a subsequent call will retry the build.
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
    // a duplicate client, but only the first inserted is retained — the
    // others are dropped. This is acceptable: the extra build is cheap and
    // happens at most once per concurrent first-call race.
    let _ = CLIENT.set(client);
    Ok(CLIENT
        .get()
        .expect("client was just set or is present from a racing caller"))
}
