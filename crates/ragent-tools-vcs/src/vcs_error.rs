//! Shared VCS HTTP error classification (T-308).
//!
//! The GitHub and GitLab clients both map a non-success HTTP response onto the
//! same four outcome shapes (rate limit, permission denial, authentication
//! failure, other error). The *shape* rule - including GitHub's "403 is a rate
//! limit only when `x-ratelimit-remaining: 0`" carve-out - lives here once; each
//! client supplies its own provider-specific message wording around the
//! classified shape.

/// The shared outcome shape of a non-success VCS API response.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VcsStatus {
    /// Too many requests: HTTP 429, or a 403 whose rate-limit quota is exhausted.
    RateLimited,
    /// HTTP 403 without an exhausted rate-limit quota - an ordinary scope denial.
    PermissionDenied,
    /// HTTP 401 - the token is missing, expired, or invalid.
    Unauthorized,
    /// Any other status, including successful ones.
    Other,
}

/// Classify an HTTP status into a shared [`VcsStatus`] shape.
///
/// `rate_remaining` is the parsed `x-ratelimit-remaining` header value, if the
/// provider sends one (GitHub; GitLab sends none). A 403 is treated as a rate
/// limit only when that value is exactly `0`, otherwise it is an ordinary
/// permission denial (ANTIPAT.md I-5).
///
/// # Examples
///
/// ```
/// use ragent_tools_vcs::vcs_error::{VcsStatus, classify_status};
///
/// assert_eq!(classify_status(429, None), VcsStatus::RateLimited);
/// assert_eq!(classify_status(403, None), VcsStatus::PermissionDenied);
/// assert_eq!(classify_status(403, Some(0)), VcsStatus::RateLimited);
/// assert_eq!(classify_status(401, None), VcsStatus::Unauthorized);
/// assert_eq!(classify_status(500, None), VcsStatus::Other);
/// assert_eq!(classify_status(200, None), VcsStatus::Other);
/// ```
#[must_use]
pub const fn classify_status(status: u16, rate_remaining: Option<u32>) -> VcsStatus {
    match status {
        429 => VcsStatus::RateLimited,
        403 => match rate_remaining {
            Some(0) => VcsStatus::RateLimited,
            _ => VcsStatus::PermissionDenied,
        },
        401 => VcsStatus::Unauthorized,
        _ => VcsStatus::Other,
    }
}
