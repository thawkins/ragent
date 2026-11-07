//! Validation for operator-supplied instance URLs.
//!
//! A forge instance base URL (`GITLAB_URL`, a recorded `instance_url`, an
//! explicit host from a reverse-engineered identifier) is sent a
//! `PRIVATE-TOKEN` header on every API call. An unvalidated value from the
//! environment therefore decides where a credential goes: `http://` downgrades
//! the token to cleartext, and any host whatsoever receives it.
//!
//! There are three separate resolution sites in the workspace
//! (`ragent_tools_vcs::gitlab::auth::load_config`, the project-scaffold remote
//! helper, and the TUI `/spec reverse` resolver) and they must agree, so the
//! check lives here once.

use std::net::IpAddr;

/// A URL scheme an instance base URL may use.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InstanceScheme {
    /// `https://` - the only scheme that keeps the token confidential. Required
    /// for any host other than loopback.
    Https,
    /// `http://` - accepted only for a loopback host, where the traffic never
    /// leaves the machine (a locally-run GitLab container).
    HttpLoopback,
}

/// The reason an instance URL was refused.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InstanceUrlError {
    /// The value is empty or whitespace only.
    Empty,
    /// The value does not parse as an absolute URL.
    NotAbsolute(String),
    /// The scheme is neither `https` nor `http`.
    UnsupportedScheme(String),
    /// `http://` was used for a non-loopback host.
    InsecureScheme(String),
    /// The URL carries userinfo (`https://user:pw@host`). Credentials must come
    /// from the token store, never from a URL that is logged and echoed.
    CredentialsInUrl,
    /// The URL names no host.
    MissingHost,
}

impl std::fmt::Display for InstanceUrlError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Empty => f.write_str("the URL is empty"),
            Self::NotAbsolute(v) => {
                write!(
                    f,
                    "`{v}` is not an absolute URL (expected https://host[/path])"
                )
            }
            Self::UnsupportedScheme(s) => {
                write!(f, "scheme `{s}` is not supported (expected https)")
            }
            Self::InsecureScheme(_) => f.write_str(
                "`http://` is only allowed for a loopback host; use https:// for a remote \
                 instance, because the instance token travels in a header",
            ),
            Self::CredentialsInUrl => {
                f.write_str("the URL embeds credentials; use the token store instead")
            }
            Self::MissingHost => f.write_str("the URL names no host"),
        }
    }
}

impl std::error::Error for InstanceUrlError {}

/// Validate an operator-supplied instance base URL and return its normalised
/// form (trailing `/` stripped, no query, no fragment).
///
/// Accepts `https://` for any host and `http://` for a loopback host only.
///
/// # Errors
///
/// Returns [`InstanceUrlError`] when the value is empty, not an absolute URL,
/// uses another scheme, uses `http://` for a remote host, embeds userinfo, or
/// names no host.
///
/// # Examples
///
/// ```
/// use ragent_types::url_validation::validate_instance_url;
///
/// assert_eq!(
///     validate_instance_url("https://gitlab.example.com/api/v4/").unwrap(),
///     "https://gitlab.example.com/api/v4"
/// );
/// assert!(validate_instance_url("http://gitlab.example.com").is_err());
/// assert!(validate_instance_url("http://127.0.0.1:8080").is_ok());
/// ```
pub fn validate_instance_url(raw: &str) -> Result<String, InstanceUrlError> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Err(InstanceUrlError::Empty);
    }
    // `url::Url` accepts a relative value only with a base, so a parse failure
    // here means "not absolute"; distinguish the empty case above first so the
    // message is useful.
    let parsed =
        url::Url::parse(trimmed).map_err(|_| InstanceUrlError::NotAbsolute(trimmed.to_string()))?;

    match parsed.scheme() {
        "https" => {}
        "http" => {
            let host = parsed.host_str().ok_or(InstanceUrlError::MissingHost)?;
            if !is_loopback_host(host) {
                return Err(InstanceUrlError::InsecureScheme(trimmed.to_string()));
            }
        }
        other => return Err(InstanceUrlError::UnsupportedScheme(other.to_string())),
    }

    if !parsed.username().is_empty() || parsed.password().is_some() {
        return Err(InstanceUrlError::CredentialsInUrl);
    }
    if parsed.host_str().is_none_or(str::is_empty) {
        return Err(InstanceUrlError::MissingHost);
    }

    // Normalise: drop any query/fragment and the trailing slash, so the value
    // can be concatenated with an API path without producing a `//`.
    let mut normalised = parsed.clone();
    normalised.set_query(None);
    normalised.set_fragment(None);
    Ok(normalised.as_str().trim_end_matches('/').to_string())
}

/// Classify the scheme of a validated instance URL (for callers that want to
/// report `http://` explicitly).
#[must_use]
pub fn instance_scheme(validated: &str) -> InstanceScheme {
    if validated.starts_with("http://") {
        InstanceScheme::HttpLoopback
    } else {
        InstanceScheme::Https
    }
}

/// Return `true` when `host` names the local machine.
///
/// `localhost`, a `*.localhost` name, or a loopback IP literal. A DNS name that
/// merely resolves to a loopback address is not accepted: resolution can
/// change between validation and use.
fn is_loopback_host(host: &str) -> bool {
    let bare = host.trim_matches(['[', ']']);
    if bare.eq_ignore_ascii_case("localhost") || bare.to_ascii_lowercase().ends_with(".localhost") {
        return true;
    }
    bare.parse::<IpAddr>().is_ok_and(|ip| ip.is_loopback())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_https_remote_is_accepted_and_normalised() {
        assert_eq!(
            validate_instance_url("  https://gitlab.example.com/api/v4/  ").unwrap(),
            "https://gitlab.example.com/api/v4"
        );
        assert_eq!(
            validate_instance_url("https://gitlab.com").unwrap(),
            "https://gitlab.com"
        );
    }

    #[test]
    fn test_http_is_refused_for_a_remote_host() {
        let err = validate_instance_url("http://gitlab.example.com").unwrap_err();
        assert!(matches!(err, InstanceUrlError::InsecureScheme(_)));
        // The message names the fix, not just the failure.
        assert!(err.to_string().contains("https"));
    }

    #[test]
    fn test_http_is_allowed_for_loopback() {
        for url in [
            "http://localhost:8080",
            "http://127.0.0.1:3000",
            "http://[::1]:8080",
            "http://gitlab.localhost",
        ] {
            assert!(
                validate_instance_url(url).is_ok(),
                "{url} should be accepted"
            );
        }
    }

    #[test]
    fn test_other_schemes_are_refused() {
        for url in [
            "ftp://example.com",
            "file:///etc/passwd",
            "git://example.com",
        ] {
            assert!(
                matches!(
                    validate_instance_url(url),
                    Err(InstanceUrlError::UnsupportedScheme(_))
                ),
                "{url} should be refused"
            );
        }
    }

    #[test]
    fn test_userinfo_is_refused() {
        assert_eq!(
            validate_instance_url("https://user:pw@gitlab.example.com"),
            Err(InstanceUrlError::CredentialsInUrl)
        );
        assert_eq!(
            validate_instance_url("https://token@gitlab.example.com"),
            Err(InstanceUrlError::CredentialsInUrl)
        );
    }

    #[test]
    fn test_empty_and_relative_are_refused() {
        assert_eq!(validate_instance_url(""), Err(InstanceUrlError::Empty));
        assert_eq!(validate_instance_url("   "), Err(InstanceUrlError::Empty));
        assert!(matches!(
            validate_instance_url("gitlab.example.com"),
            Err(InstanceUrlError::NotAbsolute(_))
        ));
        assert!(matches!(
            validate_instance_url("/api/v4"),
            Err(InstanceUrlError::NotAbsolute(_))
        ));
    }

    #[test]
    fn test_query_and_fragment_are_rejected_from_the_base() {
        assert_eq!(
            validate_instance_url("https://gitlab.example.com/base?x=1#frag").unwrap(),
            "https://gitlab.example.com/base"
        );
    }

    #[test]
    fn test_instance_scheme_classification() {
        assert_eq!(instance_scheme("https://gitlab.com"), InstanceScheme::Https);
        assert_eq!(
            instance_scheme("http://localhost:8080"),
            InstanceScheme::HttpLoopback
        );
    }
}
