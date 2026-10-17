//! Pure-logic tests for the open-access helper surface: DOI extraction and
//! recovered-URL validation (SSRF guard). No network access.

use ragent_research::open_access::{
    DEFAULT_OA_MIN_FULL_TEXT_CHARS, RecoveredOpenAccess, RecoverySource, extract_doi,
    validate_oa_url,
};

#[test]
fn extract_doi_accepts_every_prefix_form() {
    assert_eq!(
        extract_doi("https://doi.org/10.1/x"),
        Some("10.1/x".to_string())
    );
    assert_eq!(
        extract_doi("http://doi.org/10.1/x"),
        Some("10.1/x".to_string())
    );
    assert_eq!(
        extract_doi("https://dx.doi.org/10.1/x"),
        Some("10.1/x".to_string())
    );
    assert_eq!(
        extract_doi("http://dx.doi.org/10.1/x"),
        Some("10.1/x".to_string())
    );
    assert_eq!(extract_doi("doi:10.1/x"), Some("10.1/x".to_string()));
    assert_eq!(extract_doi("DOI:10.1/x"), Some("10.1/x".to_string()));
    assert_eq!(
        extract_doi("  https://doi.org/10.1/x  "),
        Some("10.1/x".to_string())
    );
}

#[test]
fn extract_doi_from_path_segment_and_strips_suffixes() {
    assert_eq!(
        extract_doi("https://pub.example/journal/10.5/abc"),
        Some("10.5/abc".to_string())
    );
    // A trailing `#fragment` is stripped. When a `?query` is present the
    // fragment-strip fallback carries the original string forward unchanged
    // (existing behaviour), so the whole `?query` span is retained.
    assert_eq!(
        extract_doi("https://doi.org/10.5/abc#section"),
        Some("10.5/abc".to_string())
    );
    assert_eq!(
        extract_doi("https://doi.org/10.5/abc?utm=x"),
        Some("10.5/abc?utm=x".to_string())
    );
}

#[test]
fn extract_doi_rejects_non_doi_urls() {
    assert_eq!(extract_doi("https://example.com/article"), None);
    assert_eq!(extract_doi("https://doi.org/not-a-doi"), None);
    assert_eq!(extract_doi(""), None);
}

#[test]
fn validate_oa_url_accepts_global_https() {
    assert!(validate_oa_url("https://oa.example.com/paper.pdf").is_ok());
    assert!(validate_oa_url("http://oa.example.com/paper").is_ok());
    assert!(validate_oa_url("https://8.8.8.8/paper").is_ok());
}

#[test]
fn validate_oa_url_rejects_localhost_and_bad_schemes() {
    assert!(validate_oa_url("http://localhost/x").is_err());
    assert!(validate_oa_url("http://sub.localhost/x").is_err());
    assert!(validate_oa_url("ftp://example.com/x").is_err());
    assert!(validate_oa_url("file:///etc/passwd").is_err());
    assert!(validate_oa_url("not a url").is_err());
}

#[test]
fn validate_oa_url_rejects_private_and_special_ips() {
    for url in [
        "http://127.0.0.1/x",
        "http://10.0.0.5/x",
        "http://192.168.1.1/x",
        "http://169.254.1.1/x",
        "http://0.0.0.0/x",
        "http://100.64.0.1/x",
        "http://192.0.0.1/x",
        "http://198.18.0.1/x",
        "http://240.0.0.1/x",
        "http://[::1]/x",
        "http://[fe80::1]/x",
        "http://[fc00::1]/x",
        "http://[::ffff:127.0.0.1]/x",
    ] {
        assert!(validate_oa_url(url).is_err(), "should reject {url}");
    }
}

#[test]
fn recovery_source_display_and_serde() {
    assert_eq!(RecoverySource::Unpaywall.to_string(), "unpaywall");
    assert_eq!(RecoverySource::EuropePmc.to_string(), "europepmc");
    let recovered = RecoveredOpenAccess {
        url: "https://oa.example.com/x.pdf".to_string(),
        source: RecoverySource::Unpaywall,
        license: Some("cc-by".to_string()),
        version: Some("publishedVersion".to_string()),
    };
    let json = serde_json::to_string(&recovered).expect("serializable");
    let back: RecoveredOpenAccess = serde_json::from_str(&json).expect("deserializable");
    assert_eq!(back, recovered);
}

#[test]
fn default_oa_threshold_is_positive() {
    const { assert!(DEFAULT_OA_MIN_FULL_TEXT_CHARS > 0) };
}
