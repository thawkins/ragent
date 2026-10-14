//! Shared networking guards for the ragent surface crates.
//!
//! The `/plugins` and `/connectors` command families were built as parallel
//! surfaces and had drifted into byte-identical copies of two security
//! primitives: an `https`-only redirect policy (so a fetch cannot be
//! downgraded to a plaintext mirror) and an `https`-endpoint validator (so a
//! catalogue/index endpoint is never representable unless it is an absolute
//! `https` URL with a host). Each now has a single implementation here.
//!
//! [`endpoint::parse_https_endpoint`] returns the neutral
//! [`endpoint::EndpointError`]; each crate maps it into its own error type, so
//! the public error enums (and their messages) are unchanged.

/// Maximum `https`-only redirect hops followed before a fetch is refused.
///
/// A single named constant so the three call sites (connectors catalogue fetch,
/// plugins store fetch, surface manifest fetch) cannot drift.
pub const MAX_REDIRECT_HOPS: usize = 5;

/// A [`reqwest`] redirect policy that follows only `https` targets, up to
/// [`MAX_REDIRECT_HOPS`] hops.
///
/// A plaintext downgrade (or any non-`https` location) is stopped, so an
/// endpoint cannot be bypassed by a redirect chain.
#[must_use]
pub fn https_only_redirects() -> reqwest::redirect::Policy {
    reqwest::redirect::Policy::custom(|attempt| {
        if attempt.url().scheme() != "https" {
            attempt.stop()
        } else if attempt.previous().len() >= MAX_REDIRECT_HOPS {
            attempt.error("too many redirects")
        } else {
            attempt.follow()
        }
    })
}

/// Shared `https`-endpoint validation.
pub mod endpoint {
    use url::Url;

    /// A refused `https` endpoint, independent of the calling crate's error
    /// taxonomy. Each crate maps this into its own public error enum.
    #[derive(Debug, Clone, PartialEq, Eq)]
    pub enum EndpointError {
        /// The endpoint string was empty or whitespace-only.
        Empty,
        /// The endpoint string could not be parsed as a URL.
        Malformed(String),
        /// The endpoint used a scheme other than `https`.
        NotHttps {
            /// The refused scheme.
            scheme: String,
        },
        /// The endpoint parsed but carried no host.
        MissingHost,
    }

    /// Parse and validate an absolute `https` URL that carries a host.
    ///
    /// The input is trimmed; an empty or whitespace-only string yields
    /// [`EndpointError::Empty`]. Any other failure names the offending scheme,
    /// the malformed input, or the missing host.
    ///
    /// # Errors
    ///
    /// Returns an [`EndpointError`] when the input is empty, is not a parseable
    /// URL, is not `https`, or carries no host.
    pub fn parse_https_endpoint(raw: &str) -> Result<Url, EndpointError> {
        let trimmed = raw.trim();
        if trimmed.is_empty() {
            return Err(EndpointError::Empty);
        }
        let url = Url::parse(trimmed).map_err(|e| EndpointError::Malformed(e.to_string()))?;
        if url.scheme() != "https" {
            return Err(EndpointError::NotHttps {
                scheme: url.scheme().to_string(),
            });
        }
        if url.host_str().is_none() {
            return Err(EndpointError::MissingHost);
        }
        Ok(url)
    }
}
