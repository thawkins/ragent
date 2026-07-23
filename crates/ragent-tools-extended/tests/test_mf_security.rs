//! Unit tests for `masterfetch::security` — SSRF protection (T-028, FR-019,
//! NFR-003).
//!
//! Covers every SSRF bypass vector enumerated in the spec:
//! - Private IP ranges (10/8, 172.16/12, 192.168/16, 0/8)
//! - Loopback (127/8, `::1`)
//! - Link-local / cloud metadata (169.254/16, `metadata.google.internal`,
//!   `metadata.azure.com`, `fe80::/10`)
//! - Alternate IP notations (octal, hex, decimal, short-form, mixed)
//! - Localhost hostnames (`localhost`, `local`)
//! - DNS rebinding services (`.nip.io`, `.sslip.io`, `.xip.io`, `.nip.name`,
//!   `.1u.ms`)
//! - Backslash characters (CVE-2025-0454)
//! - Blocked schemes (file, ftp, gopher, data, javascript, vbscript, about,
//!   chrome, blob, ws, wss)
//! - Bracketed IPv6 hosts (valid + invalid)
//! - IPv4-mapped IPv6 (`::ffff:a.b.c.d`)
//! - Multicast / reserved (224/4, 240/4, `ff00::/8`)
//! - Non-ASCII hosts, empty/missing host, oversized URL
//!
//! Plus valid-URL sanity checks and public-range boundary edge cases.

use ragent_tools_extended::masterfetch::security::{MAX_URL_LEN, SecurityError, validate_url};

// ===========================================================================
// Valid URLs — must pass
// ===========================================================================

#[test]
fn test_valid_https_url() {
    assert!(validate_url("https://example.com").is_ok());
}

#[test]
fn test_valid_http_url() {
    assert!(validate_url("http://example.com").is_ok());
}

#[test]
fn test_valid_url_with_port() {
    assert!(validate_url("https://example.com:443/path").is_ok());
}

#[test]
fn test_valid_url_with_non_default_port() {
    assert!(validate_url("http://example.com:8080/path").is_ok());
}

#[test]
fn test_valid_url_with_query() {
    assert!(validate_url("https://example.com/search?q=test&page=1").is_ok());
}

#[test]
fn test_valid_url_with_fragment() {
    assert!(validate_url("https://example.com/page#section").is_ok());
}

#[test]
fn test_valid_public_ip() {
    // 93.184.216.34 is example.com's public IP — not in any private range.
    assert!(validate_url("http://93.184.216.34").is_ok());
}

#[test]
fn test_valid_public_ipv6() {
    // A public IPv6 address (Cloudflare's documentation range).
    assert!(validate_url("https://[2606:2800:220:1:248:1893:25c8:1946]").is_ok());
}

#[test]
fn test_valid_url_with_path_and_query() {
    assert!(validate_url("https://example.com/a/b/c?x=1&y=2").is_ok());
}

// ===========================================================================
// Empty / too long
// ===========================================================================

#[test]
fn test_empty_url_rejected() {
    assert_eq!(validate_url(""), Err(SecurityError::Empty));
}

#[test]
fn test_oversized_url_rejected() {
    // Build a URL strictly longer than MAX_URL_LEN.
    let long = format!("https://example.com/{}", "a".repeat(MAX_URL_LEN));
    assert!(long.len() > MAX_URL_LEN);
    assert!(matches!(
        validate_url(&long),
        Err(SecurityError::TooLong(_))
    ));
}

#[test]
fn test_url_at_max_length_accepted_if_valid() {
    // A valid URL whose length is exactly MAX_URL_LEN should not be rejected
    // for being too long (it may still be rejected for other reasons, but not
    // TooLong). We craft a URL padded with path characters to hit the limit.
    let prefix = "https://example.com/";
    let pad = MAX_URL_LEN - prefix.len();
    let url = format!("{prefix}{}", "a".repeat(pad));
    assert_eq!(url.len(), MAX_URL_LEN);
    let res = validate_url(&url);
    assert!(
        !matches!(res, Err(SecurityError::TooLong(_))),
        "URL at exactly MAX_URL_LEN should not be TooLong, got {res:?}"
    );
}

// ===========================================================================
// Backslash (CVE-2025-0454)
// ===========================================================================

#[test]
fn test_backslash_in_path_rejected() {
    assert_eq!(
        validate_url("https://example.com/path\\to"),
        Err(SecurityError::Backslash)
    );
}

#[test]
fn test_backslash_in_host_rejected() {
    // The literal backslash is caught before parsing.
    assert_eq!(
        validate_url("https://example\\.com"),
        Err(SecurityError::Backslash)
    );
}

#[test]
fn test_backslash_in_query_rejected() {
    assert_eq!(
        validate_url("https://example.com/?q=a\\b"),
        Err(SecurityError::Backslash)
    );
}

#[test]
fn test_backslash_alone_rejected() {
    assert_eq!(validate_url("\\"), Err(SecurityError::Backslash));
}

// ===========================================================================
// Blocked schemes — every entry in BLOCKED_SCHEMES
// ===========================================================================

#[test]
fn test_file_scheme_rejected() {
    assert!(matches!(
        validate_url("file:///etc/passwd"),
        Err(SecurityError::BlockedScheme(s)) if s == "file"
    ));
}

#[test]
fn test_ftp_scheme_rejected() {
    assert!(matches!(
        validate_url("ftp://example.com/file"),
        Err(SecurityError::BlockedScheme(s)) if s == "ftp"
    ));
}

#[test]
fn test_gopher_scheme_rejected() {
    assert!(matches!(
        validate_url("gopher://example.com"),
        Err(SecurityError::BlockedScheme(s)) if s == "gopher"
    ));
}

#[test]
fn test_data_scheme_rejected() {
    assert!(matches!(
        validate_url("data:text/html,<h1>hi</h1>"),
        Err(SecurityError::BlockedScheme(s)) if s == "data"
    ));
}

#[test]
fn test_javascript_scheme_rejected() {
    assert!(matches!(
        validate_url("javascript:alert(1)"),
        Err(SecurityError::BlockedScheme(s)) if s == "javascript"
    ));
}

#[test]
fn test_vbscript_scheme_rejected() {
    assert!(matches!(
        validate_url("vbscript:msgbox(1)"),
        Err(SecurityError::BlockedScheme(s)) if s == "vbscript"
    ));
}

#[test]
fn test_about_scheme_rejected() {
    assert!(matches!(
        validate_url("about:blank"),
        Err(SecurityError::BlockedScheme(s)) if s == "about"
    ));
}

#[test]
fn test_chrome_scheme_rejected() {
    assert!(matches!(
        validate_url("chrome://settings"),
        Err(SecurityError::BlockedScheme(s)) if s == "chrome"
    ));
}

#[test]
fn test_blob_scheme_rejected() {
    // `blob:` URLs have no host and a non-http scheme — blocked.
    assert!(matches!(
        validate_url("blob:https://example.com/uuid"),
        Err(SecurityError::BlockedScheme(_))
    ));
}

#[test]
fn test_ws_scheme_rejected() {
    assert!(matches!(
        validate_url("ws://example.com/socket"),
        Err(SecurityError::BlockedScheme(s)) if s == "ws"
    ));
}

#[test]
fn test_wss_scheme_rejected() {
    assert!(matches!(
        validate_url("wss://example.com/socket"),
        Err(SecurityError::BlockedScheme(s)) if s == "wss"
    ));
}

#[test]
fn test_unknown_scheme_rejected() {
    // Any scheme that is neither http, https, nor in BLOCKED_SCHEMES is still
    // rejected (the validator only permits http/https).
    assert!(matches!(
        validate_url("custom://example.com"),
        Err(SecurityError::BlockedScheme(_))
    ));
}

// ===========================================================================
// Localhost hostnames
// ===========================================================================

#[test]
fn test_localhost_hostname_rejected() {
    assert!(matches!(
        validate_url("http://localhost"),
        Err(SecurityError::Localhost(_))
    ));
}

#[test]
fn test_localhost_with_port_rejected() {
    assert!(matches!(
        validate_url("http://localhost:8080"),
        Err(SecurityError::Localhost(_))
    ));
}

#[test]
fn test_localhost_with_path_rejected() {
    assert!(matches!(
        validate_url("http://localhost/admin"),
        Err(SecurityError::Localhost(_))
    ));
}

#[test]
fn test_local_hostname_rejected() {
    assert!(matches!(
        validate_url("http://local"),
        Err(SecurityError::Localhost(_))
    ));
}

#[test]
fn test_uppercase_localhost_rejected() {
    // Host comparison is case-insensitive.
    assert!(matches!(
        validate_url("http://LOCALHOST"),
        Err(SecurityError::Localhost(_))
    ));
}

#[test]
fn test_mixed_case_localhost_rejected() {
    assert!(matches!(
        validate_url("http://LocalHost"),
        Err(SecurityError::Localhost(_))
    ));
}

// ===========================================================================
// Loopback IPs — 127.0.0.0/8 and ::1
// ===========================================================================

#[test]
fn test_127_0_0_1_rejected() {
    assert!(matches!(
        validate_url("http://127.0.0.1"),
        Err(SecurityError::PrivateRange(_))
    ));
}

#[test]
fn test_127_1_short_form_rejected() {
    // 127.1 is short-form for 127.0.0.1
    assert!(matches!(
        validate_url("http://127.1"),
        Err(SecurityError::PrivateRange(_))
    ));
}

#[test]
fn test_127_255_255_254_rejected() {
    // Top of the 127/8 loopback range.
    assert!(matches!(
        validate_url("http://127.255.255.254"),
        Err(SecurityError::PrivateRange(_))
    ));
}

#[test]
fn test_127_0_0_1_with_port_rejected() {
    assert!(matches!(
        validate_url("http://127.0.0.1:8080"),
        Err(SecurityError::PrivateRange(_))
    ));
}

#[test]
fn test_ipv6_loopback_rejected() {
    assert!(matches!(
        validate_url("https://[::1]"),
        Err(SecurityError::PrivateRange(_))
    ));
}

#[test]
fn test_ipv6_loopback_with_port_rejected() {
    assert!(matches!(
        validate_url("https://[::1]:8080"),
        Err(SecurityError::PrivateRange(_))
    ));
}

// ===========================================================================
// Private ranges — 10/8, 172.16/12, 192.168/16, 0/8
// ===========================================================================

#[test]
fn test_10_0_0_1_rejected() {
    assert!(matches!(
        validate_url("http://10.0.0.1"),
        Err(SecurityError::PrivateRange(_))
    ));
}

#[test]
fn test_10_255_255_255_rejected() {
    assert!(matches!(
        validate_url("http://10.255.255.255"),
        Err(SecurityError::PrivateRange(_))
    ));
}

#[test]
fn test_172_16_0_1_rejected() {
    // Start of the 172.16.0.0/12 private range.
    assert!(matches!(
        validate_url("http://172.16.0.1"),
        Err(SecurityError::PrivateRange(_))
    ));
}

#[test]
fn test_172_31_255_255_rejected() {
    // End of the 172.16.0.0/12 private range.
    assert!(matches!(
        validate_url("http://172.31.255.255"),
        Err(SecurityError::PrivateRange(_))
    ));
}

#[test]
fn test_192_168_1_1_rejected() {
    assert!(matches!(
        validate_url("http://192.168.1.1"),
        Err(SecurityError::PrivateRange(_))
    ));
}

#[test]
fn test_192_168_0_0_rejected() {
    assert!(matches!(
        validate_url("http://192.168.0.0"),
        Err(SecurityError::PrivateRange(_))
    ));
}

#[test]
fn test_0_0_0_0_rejected() {
    // 0.0.0.0/8 — "this network"
    assert!(matches!(
        validate_url("http://0.0.0.0"),
        Err(SecurityError::PrivateRange(_))
    ));
}

#[test]
fn test_0_1_2_3_rejected() {
    assert!(matches!(
        validate_url("http://0.1.2.3"),
        Err(SecurityError::PrivateRange(_))
    ));
}

// ===========================================================================
// Link-local / cloud metadata
// ===========================================================================

#[test]
fn test_169_254_169_254_rejected() {
    // AWS / GCP / Azure cloud metadata IP.
    assert!(matches!(
        validate_url("http://169.254.169.254"),
        Err(SecurityError::PrivateRange(_))
    ));
}

#[test]
fn test_169_254_0_1_rejected() {
    // Link-local range.
    assert!(matches!(
        validate_url("http://169.254.0.1"),
        Err(SecurityError::PrivateRange(_))
    ));
}

#[test]
fn test_169_254_255_255_rejected() {
    // Top of 169.254.0.0/16.
    assert!(matches!(
        validate_url("http://169.254.255.255"),
        Err(SecurityError::PrivateRange(_))
    ));
}

#[test]
fn test_metadata_google_internal_rejected() {
    assert!(matches!(
        validate_url("http://metadata.google.internal"),
        Err(SecurityError::CloudMetadata(_))
    ));
}

#[test]
fn test_metadata_google_internal_with_path_rejected() {
    assert!(matches!(
        validate_url("http://metadata.google.internal/computeMetadata/v1/"),
        Err(SecurityError::CloudMetadata(_))
    ));
}

#[test]
fn test_metadata_azure_com_rejected() {
    assert!(matches!(
        validate_url("http://metadata.azure.com"),
        Err(SecurityError::CloudMetadata(_))
    ));
}

#[test]
fn test_metadata_azure_com_case_insensitive_rejected() {
    assert!(matches!(
        validate_url("http://METADATA.AZURE.COM"),
        Err(SecurityError::CloudMetadata(_))
    ));
}

#[test]
fn test_ipv6_link_local_rejected() {
    assert!(matches!(
        validate_url("https://[fe80::1]"),
        Err(SecurityError::PrivateRange(_))
    ));
}

#[test]
fn test_ipv6_unspecified_rejected() {
    assert!(matches!(
        validate_url("https://[::]"),
        Err(SecurityError::PrivateRange(_))
    ));
}

#[test]
fn test_ipv6_unique_local_rejected() {
    // fc00::/7 unique local address.
    assert!(matches!(
        validate_url("https://[fd00::1]"),
        Err(SecurityError::PrivateRange(_))
    ));
}

// ===========================================================================
// DNS rebinding services
// ===========================================================================

#[test]
fn test_nip_io_rejected() {
    assert!(matches!(
        validate_url("http://127.0.0.1.nip.io"),
        Err(SecurityError::DnsRebinding(_))
    ));
}

#[test]
fn test_sslip_io_rejected() {
    assert!(matches!(
        validate_url("http://10.0.0.1.sslip.io"),
        Err(SecurityError::DnsRebinding(_))
    ));
}

#[test]
fn test_xip_io_rejected() {
    assert!(matches!(
        validate_url("http://192.168.1.1.xip.io"),
        Err(SecurityError::DnsRebinding(_))
    ));
}

#[test]
fn test_nip_name_rejected() {
    assert!(matches!(
        validate_url("http://127.0.0.1.nip.name"),
        Err(SecurityError::DnsRebinding(_))
    ));
}

#[test]
fn test_1u_ms_rejected() {
    assert!(matches!(
        validate_url("http://127.0.0.1.1u.ms"),
        Err(SecurityError::DnsRebinding(_))
    ));
}

#[test]
fn test_dns_rebinding_with_path_rejected() {
    assert!(matches!(
        validate_url("http://10.0.0.1.sslip.io/path?q=1"),
        Err(SecurityError::DnsRebinding(_))
    ));
}

// ===========================================================================
// Multicast / reserved
// ===========================================================================

#[test]
fn test_multicast_224_0_0_1_rejected() {
    assert!(matches!(
        validate_url("http://224.0.0.1"),
        Err(SecurityError::PrivateRange(_))
    ));
}

#[test]
fn test_multicast_239_255_255_255_rejected() {
    // Top of 224.0.0.0/4 multicast range.
    assert!(matches!(
        validate_url("http://239.255.255.255"),
        Err(SecurityError::PrivateRange(_))
    ));
}

#[test]
fn test_reserved_240_0_0_1_rejected() {
    assert!(matches!(
        validate_url("http://240.0.0.1"),
        Err(SecurityError::PrivateRange(_))
    ));
}

#[test]
fn test_reserved_255_255_255_255_rejected() {
    // Top of 240.0.0.0/4 reserved range (limited broadcast).
    assert!(matches!(
        validate_url("http://255.255.255.255"),
        Err(SecurityError::PrivateRange(_))
    ));
}

#[test]
fn test_ipv6_multicast_rejected() {
    // ff00::/8 multicast.
    assert!(matches!(
        validate_url("https://[ff02::1]"),
        Err(SecurityError::PrivateRange(_))
    ));
}

// ===========================================================================
// Alternate IP notations — octal, hex, decimal, short-form, mixed
// ===========================================================================

#[test]
fn test_octal_notation_rejected() {
    // 0177.0.0.1 = 127.0.0.1 in octal
    assert!(matches!(
        validate_url("http://0177.0.0.1"),
        Err(SecurityError::PrivateRange(_))
    ));
}

#[test]
fn test_octal_0o_prefix_notation_rejected_as_parse_error() {
    // The `url` crate does not recognise the `0o` octal prefix as a valid IPv4
    // literal, so `0o177.0.0.1` fails at the parse stage. Either way it cannot
    // bypass SSRF protection — the request never reaches a private host.
    assert!(matches!(
        validate_url("http://0o177.0.0.1"),
        Err(SecurityError::Parse(_))
    ));
}

#[test]
fn test_hex_notation_rejected() {
    // 0x7f.0.0.1 = 127.0.0.1 in hex
    assert!(matches!(
        validate_url("http://0x7f.0.0.1"),
        Err(SecurityError::PrivateRange(_))
    ));
}

#[test]
fn test_hex_uppercase_notation_rejected() {
    // 0X7F.0.0.1 = 127.0.0.1 in hex (uppercase prefix + digits)
    assert!(matches!(
        validate_url("http://0X7F.0.0.1"),
        Err(SecurityError::PrivateRange(_))
    ));
}

#[test]
fn test_decimal_notation_rejected() {
    // 2130706433 = 127.0.0.1 as a single 32-bit integer
    assert!(matches!(
        validate_url("http://2130706433"),
        Err(SecurityError::PrivateRange(_))
    ));
}

#[test]
fn test_hex_single_integer_notation_rejected() {
    // 0x7f000001 = 127.0.0.1 as a single hex integer
    assert!(matches!(
        validate_url("http://0x7f000001"),
        Err(SecurityError::PrivateRange(_))
    ));
}

#[test]
fn test_hex_octal_mix_rejected() {
    // 0x7f.0.0177.1 — hex + octal mix = 127.0.0.1
    assert!(matches!(
        validate_url("http://0x7f.0.0177.1"),
        Err(SecurityError::PrivateRange(_))
    ));
}

#[test]
fn test_decimal_short_form_two_parts_rejected() {
    // 127.1 → 127.0.0.1
    assert!(matches!(
        validate_url("http://127.1"),
        Err(SecurityError::PrivateRange(_))
    ));
}

#[test]
fn test_decimal_short_form_three_parts_rejected() {
    // 127.0.1 → 127.0.0.1
    assert!(matches!(
        validate_url("http://127.0.1"),
        Err(SecurityError::PrivateRange(_))
    ));
}

#[test]
fn test_octal_private_10_rejected() {
    // 012.0.0.1 = 10.0.0.1 in octal (012 octal = 10 decimal)
    assert!(matches!(
        validate_url("http://012.0.0.1"),
        Err(SecurityError::PrivateRange(_))
    ));
}

#[test]
fn test_hex_private_192_168_rejected() {
    // 0xc0.0xa8.0x01.0x01 = 192.168.1.1 in hex
    assert!(matches!(
        validate_url("http://0xc0.0xa8.0x01.0x01"),
        Err(SecurityError::PrivateRange(_))
    ));
}

#[test]
fn test_decimal_notation_10_rejected() {
    // 167772161 = 10.0.0.1 as a single 32-bit integer
    assert!(matches!(
        validate_url("http://167772161"),
        Err(SecurityError::PrivateRange(_))
    ));
}

// ===========================================================================
// IPv4-mapped IPv6 (::ffff:a.b.c.d)
// ===========================================================================

#[test]
fn test_ipv4_mapped_ipv6_loopback_rejected() {
    assert!(matches!(
        validate_url("https://[::ffff:127.0.0.1]"),
        Err(SecurityError::PrivateRange(_))
    ));
}

#[test]
fn test_ipv4_mapped_ipv6_private_rejected() {
    assert!(matches!(
        validate_url("https://[::ffff:10.0.0.1]"),
        Err(SecurityError::PrivateRange(_))
    ));
}

#[test]
fn test_ipv4_mapped_ipv6_metadata_rejected() {
    // ::ffff:169.254.169.254 — cloud metadata via IPv4-mapped IPv6.
    assert!(matches!(
        validate_url("https://[::ffff:169.254.169.254]"),
        Err(SecurityError::PrivateRange(_))
    ));
}

#[test]
fn test_ipv4_mapped_ipv6_public_ok() {
    assert!(validate_url("https://[::ffff:93.184.216.34]").is_ok());
}

// ===========================================================================
// Bracketed IPv6 hosts
// ===========================================================================

#[test]
fn test_bracketed_ipv6_public_ok() {
    assert!(validate_url("https://[2606:2800:220:1:248:1893:25c8:1946]").is_ok());
}

#[test]
fn test_bracketed_ipv6_with_port_ok() {
    assert!(validate_url("https://[2606:2800:220:1:248:1893:25c8:1946]:8443/").is_ok());
}

#[test]
fn test_bracketed_ipv6_loopback_rejected() {
    assert!(matches!(
        validate_url("https://[::1]"),
        Err(SecurityError::PrivateRange(_))
    ));
}

// ===========================================================================
// Non-ASCII host
// ===========================================================================

#[test]
fn test_non_ascii_host_does_not_panic() {
    // The url crate applies IDNA / punycode encoding to internationalised
    // domain names, so an ASCII punycode host may result. We only require that
    // the validator does not panic and returns a deterministic result.
    let result = validate_url("https://例え.com");
    let _ = result;
}

// ===========================================================================
// No host
// ===========================================================================

#[test]
fn test_no_host_rejected() {
    // "http://" with no host — the url crate rejects this at parse time.
    assert!(matches!(
        validate_url("http://"),
        Err(SecurityError::Parse(_))
    ));
}

// ===========================================================================
// Valid edge cases — public-range boundaries
// ===========================================================================

#[test]
fn test_172_15_public_ok() {
    // 172.15.x.x is just below the 172.16.0.0/12 private range — public.
    assert!(validate_url("http://172.15.0.1").is_ok());
}

#[test]
fn test_172_32_public_ok() {
    // 172.32.x.x is just above the 172.16.0.0/12 private range — public.
    assert!(validate_url("http://172.32.0.1").is_ok());
}

#[test]
fn test_172_16_boundary_rejected() {
    // Exactly the start of the private range.
    assert!(matches!(
        validate_url("http://172.16.0.0"),
        Err(SecurityError::PrivateRange(_))
    ));
}

#[test]
fn test_172_31_boundary_rejected() {
    // Exactly the end of the private range.
    assert!(matches!(
        validate_url("http://172.31.255.255"),
        Err(SecurityError::PrivateRange(_))
    ));
}

#[test]
fn test_11_x_public_ok() {
    // 11.x.x.x is just above 10.0.0.0/8 — public.
    assert!(validate_url("http://11.0.0.1").is_ok());
}

#[test]
fn test_126_x_public_ok() {
    // 126.x.x.x is just below 127.0.0.0/8 — public.
    assert!(validate_url("http://126.0.0.1").is_ok());
}

#[test]
fn test_128_x_public_ok() {
    // 128.x.x.x is just above 127.0.0.0/8 — public.
    assert!(validate_url("http://128.0.0.1").is_ok());
}

#[test]
fn test_223_x_public_ok() {
    // 223.x.x.x is just below 224.0.0.0/4 multicast — public.
    assert!(validate_url("http://223.0.0.1").is_ok());
}

#[test]
fn test_1_1_1_1_public_ok() {
    // Cloudflare's public resolver — a well-known public IP.
    assert!(validate_url("http://1.1.1.1").is_ok());
}
