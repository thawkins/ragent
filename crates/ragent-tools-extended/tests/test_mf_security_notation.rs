//! Deterministic tests for `masterfetch::security::validate_url` alternate-IP
//! notation normalisation (SEC/FR-019).
//!
//! This test binary deliberately does NOT set the
//! `RAGENT_TOOLS_EXTENDED_TEST_NO_SSRF` bypass, so the private/reserved range
//! checks are exercised for real. Alternate notations (octal, hex, decimal,
//! short-form, and mixed) are normalised by the production code and must be
//! rejected when they resolve into a private/reserved range.

#![allow(unsafe_code)]

use ragent_tools_extended::masterfetch::security::validate_url;

/// Run `body` with the process-wide env lock held and the SSRF test bypass
/// force-disabled, restoring the previous value afterwards.
///
/// The bypass variable is process-global. Before consolidation the fetch tests
/// (which enable it for loopback servers) and these security tests were
/// separate binaries, so the bypass could never leak into a validation test.
/// A consolidated binary shares one process, so both families serialise on the
/// lock in `tests/all.rs` and this guard clears the variable while it is held.
fn without_ssrf_bypass<T>(body: impl FnOnce() -> T) -> T {
    let _lock = crate::global_env_lock();
    let saved = std::env::var_os("RAGENT_TOOLS_EXTENDED_TEST_NO_SSRF");
    // SAFETY: the variable is process-global; the lock above serialises this
    // mutation against every other test that reads or writes it, and the value
    // is restored below.
    unsafe { std::env::remove_var("RAGENT_TOOLS_EXTENDED_TEST_NO_SSRF") };
    let out = body();
    match saved {
        Some(prev) => unsafe { std::env::set_var("RAGENT_TOOLS_EXTENDED_TEST_NO_SSRF", prev) },
        None => unsafe { std::env::remove_var("RAGENT_TOOLS_EXTENDED_TEST_NO_SSRF") },
    }
    out
}

// -- octal ----------------------------------------------------------------

#[test]
fn notation_octal_leading_zero_loopback_rejected() {
    without_ssrf_bypass(|| {
        // 0177.0.0.1 == 127.0.0.1
        assert!(validate_url("http://0177.0.0.1/").is_err());
    });
}

#[test]
fn notation_octal_explicit_prefix_loopback_rejected() {
    without_ssrf_bypass(|| {
        assert!(validate_url("http://0o177.0.0.1/").is_err());
    });
}

// -- hex ------------------------------------------------------------------

#[test]
fn notation_hex_loopback_rejected() {
    without_ssrf_bypass(|| {
        assert!(validate_url("http://0x7f.0.0.1/").is_err());
    });
}

#[test]
fn notation_hex_uppercase_prefix_loopback_rejected() {
    without_ssrf_bypass(|| {
        assert!(validate_url("http://0X7F.0.0.1/").is_err());
    });
}

// -- single-integer -------------------------------------------------------

#[test]
fn notation_single_integer_decimal_loopback_rejected() {
    without_ssrf_bypass(|| {
        // 2130706433 == 127.0.0.1
        assert!(validate_url("http://2130706433/").is_err());
    });
}

#[test]
fn notation_single_integer_hex_loopback_rejected() {
    without_ssrf_bypass(|| {
        assert!(validate_url("http://0x7f000001/").is_err());
    });
}

// -- short form -----------------------------------------------------------

#[test]
fn notation_short_form_two_part_loopback_rejected() {
    without_ssrf_bypass(|| {
        // 127.1 -> 127.0.0.1
        assert!(validate_url("http://127.1/").is_err());
    });
}

#[test]
fn notation_short_form_three_part_loopback_rejected() {
    without_ssrf_bypass(|| {
        // 127.0.1 -> 127.0.0.1
        assert!(validate_url("http://127.0.1/").is_err());
    });
}

// -- private ranges in alternate notation ---------------------------------

#[test]
fn notation_octal_private_10_rejected() {
    without_ssrf_bypass(|| {
        // 012 == 10 -> 10.0.0.1
        assert!(validate_url("http://012.0.0.1/").is_err());
    });
}

#[test]
fn notation_hex_private_192_168_rejected() {
    without_ssrf_bypass(|| {
        assert!(validate_url("http://0xc0.0xa8.0.1/").is_err());
    });
}

// -- invalid alternate notations parse as a plain host (allowed) ---------

#[test]
fn notation_five_parts_is_not_an_ip_and_allowed() {
    without_ssrf_bypass(|| {
        // More than four dotted parts is not an IP in any notation -> treated as a
        // regular hostname (which is fine; DNS would fail).
        assert!(validate_url("http://1.2.3.4.5.example/").is_ok());
    });
}

#[test]
fn notation_empty_part_is_not_an_ip_and_allowed() {
    without_ssrf_bypass(|| {
        assert!(validate_url("http://1..2.example/").is_ok());
    });
}

#[test]
fn notation_octet_over_255_not_private_and_allowed() {
    without_ssrf_bypass(|| {
        // A four-part form with an octet > 255 is not a valid IP literal, so it is
        // not flagged as a private range.
        assert!(validate_url("http://999.0.0.1.example/").is_ok());
    });
}

// -- hostname / scheme guards (no private-IP interaction) ----------------

#[test]
fn host_localhost_variants_rejected() {
    without_ssrf_bypass(|| {
        assert!(validate_url("http://localhost/").is_err());
        assert!(validate_url("http://local/").is_err());
        assert!(validate_url("http://localhost.localdomain/").is_err());
    });
}

#[test]
fn host_trailing_dot_localhost_rejected() {
    without_ssrf_bypass(|| {
        assert!(validate_url("http://localhost./").is_err());
    });
}

#[test]
fn host_lookalike_youtube_is_allowed_security_wise() {
    without_ssrf_bypass(|| {
        // Security-wise a lookalike host is an ordinary public hostname.
        assert!(validate_url("https://notyoutube.com/").is_ok());
    });
}

#[test]
fn scheme_ftp_and_ws_and_data_rejected() {
    without_ssrf_bypass(|| {
        assert!(validate_url("ftp://example.com/").is_err());
        assert!(validate_url("ws://example.com/").is_err());
        assert!(validate_url("wss://example.com/").is_err());
        assert!(validate_url("data:text/plain,hi").is_err());
    });
}
