#![allow(clippy::assert_is_empty)]
//! Integration tests for `masterfetch::urlnorm` — URL normalisation (T-029,
//! FR-027, NFR-003).
//!
//! Covers: host lowercasing, default port stripping, trailing slash removal,
//! tracking-parameter stripping, idempotency, and dedup.

use ragent_tools_extended::masterfetch::urlnorm::{dedup_urls, normalise_url};

// ---------------------------------------------------------------------------
// Host lowercasing
// ---------------------------------------------------------------------------

#[test]
fn test_host_lowercased() {
    let norm = normalise_url("https://Example.COM/path").unwrap();
    assert_eq!(norm, "https://example.com/path");
}

#[test]
fn test_scheme_lowercased() {
    let norm = normalise_url("HTTPS://example.com/path").unwrap();
    assert_eq!(norm, "https://example.com/path");
}

#[test]
fn test_mixed_case_host_and_scheme() {
    let norm = normalise_url("HTTP://ExAmPlE.CoM/Path").unwrap();
    assert_eq!(norm, "http://example.com/Path");
}

// ---------------------------------------------------------------------------
// Default port stripping
// ---------------------------------------------------------------------------

#[test]
fn test_http_default_port_80_stripped() {
    let norm = normalise_url("http://example.com:80/path").unwrap();
    assert_eq!(norm, "http://example.com/path");
}

#[test]
fn test_https_default_port_443_stripped() {
    let norm = normalise_url("https://example.com:443/path").unwrap();
    assert_eq!(norm, "https://example.com/path");
}

#[test]
fn test_non_default_port_preserved() {
    let norm = normalise_url("https://example.com:8443/path").unwrap();
    assert_eq!(norm, "https://example.com:8443/path");
}

#[test]
fn test_http_non_default_port_preserved() {
    let norm = normalise_url("http://example.com:8080/path").unwrap();
    assert_eq!(norm, "http://example.com:8080/path");
}

// ---------------------------------------------------------------------------
// Trailing slash removal
// ---------------------------------------------------------------------------

#[test]
fn test_trailing_slash_stripped() {
    let norm = normalise_url("https://example.com/path/").unwrap();
    assert_eq!(norm, "https://example.com/path");
}

#[test]
fn test_root_path_slash_preserved() {
    let norm = normalise_url("https://example.com/").unwrap();
    assert_eq!(norm, "https://example.com/");
}

#[test]
fn test_no_path_gets_root() {
    let norm = normalise_url("https://example.com").unwrap();
    assert_eq!(norm, "https://example.com/");
}

#[test]
fn test_multiple_trailing_slashes_stripped() {
    let norm = normalise_url("https://example.com/path///").unwrap();
    assert_eq!(norm, "https://example.com/path");
}

#[test]
fn test_deep_path_trailing_slash_stripped() {
    let norm = normalise_url("https://example.com/a/b/c/d/").unwrap();
    assert_eq!(norm, "https://example.com/a/b/c/d");
}

#[test]
fn test_path_without_trailing_slash_unchanged() {
    let norm = normalise_url("https://example.com/path").unwrap();
    assert_eq!(norm, "https://example.com/path");
}

// ---------------------------------------------------------------------------
// Tracking parameter stripping
// ---------------------------------------------------------------------------

#[test]
fn test_utm_source_stripped() {
    let norm = normalise_url("https://example.com?utm_source=foo").unwrap();
    assert_eq!(norm, "https://example.com/");
}

#[test]
fn test_utm_medium_stripped() {
    let norm = normalise_url("https://example.com?utm_medium=social").unwrap();
    assert_eq!(norm, "https://example.com/");
}

#[test]
fn test_utm_campaign_stripped() {
    let norm = normalise_url("https://example.com?utm_campaign=launch").unwrap();
    assert_eq!(norm, "https://example.com/");
}

#[test]
fn test_utm_content_stripped() {
    let norm = normalise_url("https://example.com?utm_content=header").unwrap();
    assert_eq!(norm, "https://example.com/");
}

#[test]
fn test_utm_term_stripped() {
    let norm = normalise_url("https://example.com?utm_term=rust").unwrap();
    assert_eq!(norm, "https://example.com/");
}

#[test]
fn test_fbclid_stripped() {
    let norm = normalise_url("https://example.com?fbclid=abc123").unwrap();
    assert_eq!(norm, "https://example.com/");
}

#[test]
fn test_gclid_stripped() {
    let norm = normalise_url("https://example.com?gclid=xyz456").unwrap();
    assert_eq!(norm, "https://example.com/");
}

#[test]
fn test_ref_stripped() {
    let norm = normalise_url("https://example.com?ref=newsletter").unwrap();
    assert_eq!(norm, "https://example.com/");
}

#[test]
fn test_ga_stripped() {
    let norm = normalise_url("https://example.com?_ga=GA1.2.123").unwrap();
    assert_eq!(norm, "https://example.com/");
}

#[test]
fn test_mc_cid_stripped() {
    let norm = normalise_url("https://example.com?mc_cid=abc").unwrap();
    assert_eq!(norm, "https://example.com/");
}

#[test]
fn test_mc_eid_stripped() {
    let norm = normalise_url("https://example.com?mc_eid=def").unwrap();
    assert_eq!(norm, "https://example.com/");
}

#[test]
fn test_utm_case_insensitive_stripped() {
    let norm = normalise_url("https://example.com?UTM_SOURCE=foo").unwrap();
    assert_eq!(norm, "https://example.com/");
}

#[test]
fn test_tracking_params_stripped_keep_others() {
    let norm = normalise_url("https://example.com?utm_source=foo&keep=1&fbclid=x").unwrap();
    assert_eq!(norm, "https://example.com/?keep=1");
}

#[test]
fn test_all_tracking_params_stripped() {
    let norm = normalise_url(
        "https://example.com?utm_source=a&utm_medium=b&utm_campaign=c&fbclid=d&gclid=e&ref=f&_ga=g&mc_cid=h&mc_eid=i&keep=1",
    )
    .unwrap();
    assert_eq!(norm, "https://example.com/?keep=1");
}

#[test]
fn test_non_tracking_param_preserved() {
    let norm = normalise_url("https://example.com?q=rust&page=2").unwrap();
    assert_eq!(norm, "https://example.com/?q=rust&page=2");
}

// ---------------------------------------------------------------------------
// Combined normalisation
// ---------------------------------------------------------------------------

#[test]
fn test_combined_host_port_slash_tracking() {
    let norm = normalise_url("https://Example.com:443/path/?utm_source=x&keep=1").unwrap();
    assert_eq!(norm, "https://example.com/path?keep=1");
}

#[test]
fn test_combined_all_normalisations() {
    let norm =
        normalise_url("HTTP://API.Example.COM:80/v1/posts/?utm_campaign=launch&fbclid=abc&id=42")
            .unwrap();
    assert_eq!(norm, "http://api.example.com/v1/posts?id=42");
}

// ---------------------------------------------------------------------------
// Idempotency
// ---------------------------------------------------------------------------

#[test]
fn test_idempotent_simple() {
    let once = normalise_url("http://example.com/a/").unwrap();
    let twice = normalise_url(&once).unwrap();
    assert_eq!(once, twice);
}

#[test]
fn test_idempotent_with_tracking() {
    let once = normalise_url("https://Example.com:443/path/?utm_source=foo&keep=1").unwrap();
    let twice = normalise_url(&once).unwrap();
    assert_eq!(once, twice);
}

#[test]
fn test_idempotent_already_normalised() {
    let url = "https://example.com/path?keep=1";
    let once = normalise_url(url).unwrap();
    assert_eq!(once, url);
}

// ---------------------------------------------------------------------------
// Edge cases
// ---------------------------------------------------------------------------

#[test]
fn test_fragment_preserved() {
    let norm = normalise_url("https://example.com/path#section").unwrap();
    assert_eq!(norm, "https://example.com/path#section");
}

#[test]
fn test_fragment_with_trailing_slash() {
    let norm = normalise_url("https://example.com/path/#section").unwrap();
    assert_eq!(norm, "https://example.com/path#section");
}

#[test]
fn test_query_with_trailing_slash() {
    let norm = normalise_url("https://example.com/path/?keep=1").unwrap();
    assert_eq!(norm, "https://example.com/path?keep=1");
}

#[test]
fn test_ipv4_host() {
    let norm = normalise_url("https://93.184.216.34/path/").unwrap();
    assert_eq!(norm, "https://93.184.216.34/path");
}

#[test]
fn test_just_host() {
    let norm = normalise_url("https://example.com").unwrap();
    assert_eq!(norm, "https://example.com/");
}

#[test]
fn test_empty_query_after_tracking_stripped_no_trailing_question() {
    let norm = normalise_url("https://example.com/path?utm_source=foo").unwrap();
    assert!(!norm.ends_with('?'));
    assert_eq!(norm, "https://example.com/path");
}

#[test]
fn test_multiple_values_same_key_preserved() {
    // Non-tracking keys with multiple values should be preserved.
    let norm = normalise_url("https://example.com?tag=a&tag=b").unwrap();
    assert_eq!(norm, "https://example.com/?tag=a&tag=b");
}

// ---------------------------------------------------------------------------
// Error cases
// ---------------------------------------------------------------------------

#[test]
fn test_relative_url_rejected() {
    assert!(normalise_url("/path/to/page").is_err());
}

#[test]
fn test_empty_url_rejected() {
    assert!(normalise_url("").is_err());
}

#[test]
fn test_invalid_url_rejected() {
    assert!(normalise_url("not a url at all").is_err());
}

// ---------------------------------------------------------------------------
// dedup_urls
// ---------------------------------------------------------------------------

#[test]
fn test_dedup_removes_duplicates() {
    let urls = vec![
        "https://example.com/page/",
        "https://example.com/page", // dup of #1
        "https://other.com",
    ];
    let deduped = dedup_urls(&urls);
    assert_eq!(
        deduped,
        vec!["https://example.com/page", "https://other.com/"]
    );
}

#[test]
fn test_dedup_preserves_order() {
    let urls = vec![
        "https://b.com",
        "https://a.com",
        "https://b.com/", // dup of #1
    ];
    let deduped = dedup_urls(&urls);
    assert_eq!(deduped, vec!["https://b.com/", "https://a.com/"]);
}

#[test]
fn test_dedup_tracking_params_collapse() {
    let urls = vec![
        "https://example.com?utm_source=x&id=1",
        "https://example.com?id=1", // same after normalisation
    ];
    let deduped = dedup_urls(&urls);
    assert_eq!(deduped, vec!["https://example.com/?id=1"]);
}

#[test]
fn test_dedup_skips_invalid() {
    let urls = vec!["not a url", "https://example.com", "also bad"];
    let deduped = dedup_urls(&urls);
    assert_eq!(deduped, vec!["https://example.com/"]);
}

#[test]
fn test_dedup_empty_input() {
    let deduped = dedup_urls(&[]);
    assert!(deduped.is_empty());
}
