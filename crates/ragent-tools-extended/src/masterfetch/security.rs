//! SSRF protection and URL validation for the masterfetch toolset.
//!
//! Implements FR-019 and NFR-003.
//!
//! This module ports Hound's `security.py`. The [`validate_url`] function is a
//! pure, testable function that rejects URLs targeting private/internal network
//! ranges, localhost, cloud metadata endpoints, DNS rebinding services, blocked
//! schemes, and backslash characters (CVE-2025-0454). Alternate IP notations
//! (octal, hex, decimal, short-form) are normalised before checking so that
//! attackers cannot bypass range checks with `0177.0.0.1` or `0x7f.0.0.1`.
//!
//! No DNS resolution is performed — the check is against the hostname/IP
//! literal in the URL, matching Hound's approach. This means a domain that
//! *resolves* to a private IP is not caught here; that is an accepted
//! limitation of the HTTP-only integrated runtime (no DNS rebinding via
//! resolver).
//!
//! # Example
//!
//! ```
//! use ragent_tools_extended::masterfetch::security::validate_url;
//!
//! assert!(validate_url("https://example.com").is_ok());
//! assert!(validate_url("http://127.0.0.1").is_err());
//! assert!(validate_url("file:///etc/passwd").is_err());
//! ```

use thiserror::Error;

/// Maximum allowed URL length (8 192 characters). Longer URLs are rejected
/// before parsing to prevent memory-exhaustion attacks.
pub const MAX_URL_LEN: usize = 8_192;

/// Blocked URL schemes that must never be fetched.
const BLOCKED_SCHEMES: &[&str] = &[
    "file",
    "ftp",
    "gopher",
    "data",
    "javascript",
    "vbscript",
    "about",
    "chrome",
    "blob",
    "ws",
    "wss",
];

/// DNS rebinding service suffixes. Any hostname ending in one of these is
/// rejected because the service can resolve to an arbitrary IP (including
/// private ranges) at query time.
const DNS_REBINDING_SUFFIXES: &[&str] = &[".nip.io", ".sslip.io", ".xip.io", ".nip.name", ".1u.ms"];

/// Cloud metadata endpoint hostnames that must never be fetched.
const CLOUD_METADATA_HOSTS: &[&str] = &["metadata.google.internal", "metadata.azure.com"];

/// Cloud metadata endpoint IP literal `169.254.169.254`.
const CLOUD_METADATA_IP: &str = "169.254.169.254";

/// Error returned when a URL fails SSRF / security validation.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum SecurityError {
    /// URL is empty or not a valid string.
    #[error("URL is empty")]
    Empty,
    /// URL exceeds the maximum allowed length ({0} > {MAX_URL_LEN}).
    #[error("URL exceeds maximum length of {MAX_URL_LEN} characters ({0} chars)")]
    TooLong(usize),
    /// URL contains a backslash character (CVE-2025-0454 parser confusion).
    #[error("URL contains backslash character (CVE-2025-0454)")]
    Backslash,
    /// URL failed to parse.
    #[error("URL parse error: {0}")]
    Parse(String),
    /// URL uses a blocked or non-HTTP scheme.
    #[error("Blocked scheme: '{0}' — only http and https are allowed")]
    BlockedScheme(String),
    /// Host is empty or missing.
    #[error("URL has no host")]
    NoHost,
    /// Host contains non-ASCII characters.
    #[error("Host contains non-ASCII characters: '{0}'")]
    NonAsciiHost(String),
    /// Bracketed host is not a valid IPv6 address.
    #[error("Bracketed host is not valid IPv6: '{0}'")]
    InvalidBracketedIpv6(String),
    /// Host is localhost or a localhost variant.
    #[error("Host is localhost: '{0}'")]
    Localhost(String),
    /// Host is a cloud metadata endpoint.
    #[error("Host is a cloud metadata endpoint: '{0}'")]
    CloudMetadata(String),
    /// Host is a DNS rebinding service.
    #[error("Host uses DNS rebinding service: '{0}'")]
    DnsRebinding(String),
    /// IP address is in a private / reserved network range.
    #[error("IP {0} is in a private or reserved network range")]
    PrivateRange(String),
}

/// Result alias for security validation.
pub type SecurityResult = Result<(), SecurityError>;

/// Validate a URL against SSRF and security rules (FR-019).
///
/// This is a pure function — no network I/O, no DNS resolution. It checks the
/// URL string and its parsed components against the security rules defined in
/// the module documentation.
///
/// # Errors
///
/// Returns [`SecurityError`] if the URL fails any validation check:
/// - Empty or too long
/// - Contains backslash
/// - Blocked scheme
/// - No host
/// - Non-ASCII host
/// - Invalid bracketed IPv6
/// - Localhost
/// - Cloud metadata endpoint
/// - DNS rebinding service
/// - Private/reserved IP range
pub fn validate_url(raw: &str) -> SecurityResult {
    // 1. Reject empty URLs.
    if raw.is_empty() {
        return Err(SecurityError::Empty);
    }

    // 2. Reject oversized URLs.
    if raw.len() > MAX_URL_LEN {
        return Err(SecurityError::TooLong(raw.len()));
    }

    // 3. Reject backslash characters (CVE-2025-0454).
    if raw.contains('\\') {
        return Err(SecurityError::Backslash);
    }

    // 4. Parse the URL.
    let parsed = url::Url::parse(raw).map_err(|e| SecurityError::Parse(e.to_string()))?;

    // 5. Check scheme — reject blocked, accept only http/https.
    let scheme = parsed.scheme();
    if BLOCKED_SCHEMES.contains(&scheme) {
        return Err(SecurityError::BlockedScheme(scheme.to_string()));
    }
    if scheme != "http" && scheme != "https" {
        return Err(SecurityError::BlockedScheme(scheme.to_string()));
    }

    // 6. Extract the host using the typed `Host` enum from the `url` crate.
    //    This gives us `Domain(&str)`, `Ipv4(Ipv4Addr)`, or `Ipv6(Ipv6Addr)`
    //    directly — no manual bracket-stripping needed for IPv6 literals.
    let host = match parsed.host() {
        Some(url::Host::Domain(d)) => HostKind::Domain(d.to_string()),
        Some(url::Host::Ipv4(addr)) => HostKind::Ip(std::net::IpAddr::V4(addr)),
        Some(url::Host::Ipv6(addr)) => HostKind::Ip(std::net::IpAddr::V6(addr)),
        None => return Err(SecurityError::NoHost),
    };

    // 7. Reject non-ASCII domain hosts.
    match &host {
        HostKind::Domain(d) => {
            if d.is_empty() {
                return Err(SecurityError::NoHost);
            }
            if !d.is_ascii() {
                return Err(SecurityError::NonAsciiHost(d.clone()));
            }
        }
        HostKind::Ip(_) => {} // IPs are always ASCII
    }

    validate_host(host)
}

/// Internal enum representing the kind of host extracted from a parsed URL.
enum HostKind {
    /// A domain name (e.g. `example.com`).
    Domain(String),
    /// An IP address (IPv4 or IPv6), already parsed by the `url` crate.
    Ip(std::net::IpAddr),
}

/// Environment variable that disables private/reserved IP and localhost checks.
///
/// **For integration tests only.** When set, SSRF validation still enforces
/// scheme, backslash, URL length, DNS rebinding suffixes, and cloud metadata
/// hosts, but it allows loopback/private IP literals and the `localhost`
/// hostname so the fetch pipeline can be exercised against a local test server.
const SSRF_TEST_SKIP_VAR: &str = "RAGENT_TOOLS_EXTENDED_TEST_NO_SSRF";

/// Returns `true` when the test-only SSRF bypass is enabled.
fn ssrf_test_skip() -> bool {
    std::env::var(SSRF_TEST_SKIP_VAR).is_ok_and(|v| !v.is_empty())
}

/// Validate a hostname — either a domain name or an IP literal — against
/// SSRF rules (FR-019).
///
/// # Errors
///
/// Returns [`SecurityError`] for localhost, cloud metadata, DNS rebinding,
/// private/reserved IP ranges, or alternate-notation private IPs.
fn validate_host(host: HostKind) -> SecurityResult {
    match host {
        HostKind::Ip(ip) => {
            if ssrf_test_skip() {
                return Ok(());
            }
            // The `url` crate already parsed this as a valid IP — check ranges.
            check_ip_range(&ip)
        }
        HostKind::Domain(domain) => {
            let host_lower = domain.to_ascii_lowercase();

            // 8. Reject localhost hostnames.
            if !ssrf_test_skip() && (host_lower == "localhost" || host_lower == "local") {
                return Err(SecurityError::Localhost(domain));
            }

            // 9. Reject cloud metadata endpoints.
            if CLOUD_METADATA_HOSTS.contains(&host_lower.as_str()) {
                return Err(SecurityError::CloudMetadata(domain));
            }

            // 10. Reject DNS rebinding service suffixes.
            for suffix in DNS_REBINDING_SUFFIXES {
                if host_lower.ends_with(suffix) {
                    return Err(SecurityError::DnsRebinding(domain));
                }
            }

            // 11. Try to parse the domain as an IP in alternate notation
            //     (octal, hex, decimal, short-form). The `url` crate would have
            //     returned an `Ipv4`/`Ipv6` Host variant for standard notation,
            //     so we only need to check alternate notations here.
            if let Some(normalised_ip) = normalise_ip_notation(&host_lower) {
                let ip_addr = std::net::IpAddr::V4(normalised_ip);
                check_ip_range(&ip_addr)?;
            }
            // If it's not an IP in any notation, it's a regular hostname — pass.

            Ok(())
        }
    }
}

/// Check an IP address against private, reserved, and dangerous ranges.
///
/// # Errors
///
/// Returns [`SecurityError::PrivateRange`] if the IP is in a blocked range.
fn check_ip_range(ip: &std::net::IpAddr) -> SecurityResult {
    match ip {
        std::net::IpAddr::V4(v4) => check_ipv4_range(*v4),
        std::net::IpAddr::V6(v6) => check_ipv6_range(*v6),
    }
}

/// Check an IPv4 address against private/reserved ranges (FR-019).
///
/// Blocked ranges:
/// - `0.0.0.0/8` (this network)
/// - `10.0.0.0/8` (private)
/// - `127.0.0.0/8` (loopback)
/// - `169.254.0.0/16` (link-local, includes cloud metadata `169.254.169.254`)
/// - `172.16.0.0/12` (private)
/// - `192.168.0.0/16` (private)
/// - `224.0.0.0/4` (multicast)
/// - `240.0.0.0/4` (reserved)
fn check_ipv4_range(ip: std::net::Ipv4Addr) -> SecurityResult {
    let octets = ip.octets();

    let is_private = octets[0] == 10 // 10.0.0.0/8
        || (octets[0] == 172 && (octets[1] & 0xf0) == 16) // 172.16.0.0/12
        || (octets[0] == 192 && octets[1] == 168) // 192.168.0.0/16
        || (octets[0] == 127) // 127.0.0.0/8
        || (octets[0] == 0) // 0.0.0.0/8
        || (octets[0] == 169 && octets[1] == 254) // 169.254.0.0/16
        || (octets[0] & 0xf0) == 224 // 224.0.0.0/4 (multicast)
        || (octets[0] & 0xf0) == 240; // 240.0.0.0/4 (reserved)

    if is_private {
        return Err(SecurityError::PrivateRange(ip.to_string()));
    }

    Ok(())
}

/// Check an IPv6 address against private/reserved ranges (FR-019).
///
/// Blocked ranges:
/// - `::1/128` (loopback)
/// - `::/128` (unspecified)
/// - `fc00::/7` (unique local)
/// - `fe80::/10` (link-local)
/// - `ff00::/8` (multicast)
/// - IPv4-mapped IPv6 (`::ffff:a.b.c.d`) — the embedded IPv4 is checked too
fn check_ipv6_range(ip: std::net::Ipv6Addr) -> SecurityResult {
    let segments = ip.segments();

    // ::1 loopback
    if ip == std::net::Ipv6Addr::LOCALHOST {
        return Err(SecurityError::PrivateRange(ip.to_string()));
    }

    // :: unspecified
    if ip == std::net::Ipv6Addr::UNSPECIFIED {
        return Err(SecurityError::PrivateRange(ip.to_string()));
    }

    // fc00::/7 (unique local) — top 7 bits
    if (segments[0] & 0xfe00) == 0xfc00 {
        return Err(SecurityError::PrivateRange(ip.to_string()));
    }

    // fe80::/10 (link-local) — top 10 bits
    if (segments[0] & 0xffc0) == 0xfe80 {
        return Err(SecurityError::PrivateRange(ip.to_string()));
    }

    // ff00::/8 (multicast) — top 8 bits
    if (segments[0] & 0xff00) == 0xff00 {
        return Err(SecurityError::PrivateRange(ip.to_string()));
    }

    // IPv4-mapped IPv6: ::ffff:a.b.c.d — check the embedded IPv4 address.
    if let Some(v4) = ip.to_ipv4_mapped() {
        return check_ipv4_range(v4);
    }

    Ok(())
}

/// Normalise alternate IP notations to a canonical IPv4 address.
///
/// Handles:
/// - **Octal**: `0177.0.0.1` → `127.0.0.1` (leading `0` or `0o` prefix)
/// - **Hex**: `0x7f.0.0.1` → `127.0.0.1` (leading `0x` prefix)
/// - **Decimal**: `2130706433` → `127.0.0.1` (single 32-bit integer)
/// - **Short-form**: `127.1` → `127.0.0.1` (fewer than 4 octets; last octet
///   absorbs the remaining value)
///
/// Returns `Some(Ipv4Addr)` if the host is an IP in an alternate notation,
/// `None` if it is a regular hostname.
fn normalise_ip_notation(host: &str) -> Option<std::net::Ipv4Addr> {
    // Try single-integer decimal notation: "2130706433" → 127.0.0.1
    if !host.contains('.') && !host.contains(':') {
        if let Ok(n) = host.parse::<u32>() {
            return Some(std::net::Ipv4Addr::from(n));
        }
        // Also try hex single-integer: "0x7f000001"
        if let Some(n) = host
            .strip_prefix("0x")
            .or_else(|| host.strip_prefix("0X"))
            .and_then(|hex_part| u32::from_str_radix(hex_part, 16).ok())
        {
            return Some(std::net::Ipv4Addr::from(n));
        }
        return None;
    }

    // Try dotted notation with alternate bases.
    let parts: Vec<&str> = host.split('.').collect();
    if parts.is_empty() || parts.len() > 4 {
        return None;
    }

    // All parts must be parseable in some base for this to be an IP.
    let mut octets: [u8; 4] = [0, 0, 0, 0];
    let mut parsed: Vec<u32> = Vec::with_capacity(parts.len());

    for part in &parts {
        let v = parse_ip_octet(part)?;
        parsed.push(v);
    }

    // Short-form: if fewer than 4 parts, the last part holds the remaining
    // value. E.g. "127.1" → [127, 1] → [127, 0, 0, 1].
    // "127.0x10001" → [127, 65537] → [127, 0, 0, 1].
    if parsed.len() == 4 {
        // Standard 4-octet form.
        for (i, &v) in parsed.iter().enumerate() {
            if v > 255 {
                // An individual octet > 255 in 4-part notation is invalid.
                return None;
            }
            octets[i] = v as u8;
        }
    } else if parsed.len() < 4 {
        // Short-form: last part absorbs remaining bytes.
        let last = *parsed.last()?;
        let last_u32 = last;
        let mut remaining = last_u32;
        let mut byte_idx = 3;
        // Unpack the last value into up to (5 - len) bytes.
        let absorbing_bytes = 4usize.saturating_sub(parsed.len() - 1);
        for _ in 0..absorbing_bytes {
            if byte_idx < 2 {
                // Not enough room — invalid.
                break;
            }
            octets[byte_idx] = (remaining & 0xFF) as u8;
            remaining >>= 8;
            if remaining == 0 {
                break;
            }
            byte_idx = byte_idx.saturating_sub(1);
        }
        if remaining != 0 {
            return None;
        }
        // Fill the leading octets.
        for (i, &v) in parsed.iter().rev().skip(1).enumerate() {
            if v > 255 {
                return None;
            }
            let idx = 3usize.saturating_sub(i + 1);
            if idx < 4 {
                octets[idx] = v as u8;
            }
        }
    } else {
        return None;
    }

    Some(std::net::Ipv4Addr::new(
        octets[0], octets[1], octets[2], octets[3],
    ))
}

/// Parse a single IP octet in decimal, octal, or hex notation.
///
/// - `127` → `127` (decimal)
/// - `0177` or `0o177` → `127` (octal)
/// - `0x7f` → `127` (hex)
///
/// Returns `Some(u32)` if parseable, `None` otherwise. The value may exceed 255
/// for short-form notation (e.g. `0x10001` for `65537`).
fn parse_ip_octet(part: &str) -> Option<u32> {
    if part.is_empty() {
        return None;
    }

    // Hex: 0x prefix
    if let Some(hex) = part.strip_prefix("0x").or_else(|| part.strip_prefix("0X")) {
        return u32::from_str_radix(hex, 16).ok();
    }

    // Octal: 0o prefix or leading 0 with all digits 0-7
    if let Some(oct) = part.strip_prefix("0o").or_else(|| part.strip_prefix("0O")) {
        return u32::from_str_radix(oct, 8).ok();
    }
    if part.len() > 1 && part.starts_with('0') && part.chars().all(|c| c.is_ascii_digit()) {
        // Leading-zero octal notation: "0177"
        return u32::from_str_radix(part, 8).ok();
    }

    // Decimal
    part.parse::<u32>().ok()
}
