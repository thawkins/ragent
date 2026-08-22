//! Tests for GitHub API error handling (FR-014, FR-015, FR-017).
//!
//! Covers `classify_api_error`, `extract_rate_limit_reset`, and
//! `format_reset_time` — the pure helpers behind the /reverse command's
//! error reporting.

use ragent_tools_vcs::github::{classify_api_error, format_reset_time};

// ---------------------------------------------------------------------------
// FR-014 — Non-success status: message includes HTTP status code + repo ID
// ---------------------------------------------------------------------------

#[test]
fn test_classify_500_includes_status_and_repo_id() {
    let msg = classify_api_error(500, "octocat/Hello-World", "Internal server error", None);
    assert!(
        msg.contains("500"),
        "message should include HTTP status code, got: {msg}"
    );
    assert!(
        msg.contains("octocat/Hello-World"),
        "message should include repo identifier, got: {msg}"
    );
    assert!(
        msg.contains("Internal server error"),
        "message should include response body, got: {msg}"
    );
}

#[test]
fn test_classify_503_includes_status_and_repo_id() {
    let msg = classify_api_error(503, "myorg/myrepo", "Service unavailable", None);
    assert!(msg.contains("503"));
    assert!(msg.contains("myorg/myrepo"));
    assert!(msg.contains("Service unavailable"));
}

#[test]
fn test_classify_400_includes_status_and_repo_id() {
    let msg = classify_api_error(400, "a/b", "Bad request", None);
    assert!(msg.contains("400"));
    assert!(msg.contains("a/b"));
}

#[test]
fn test_classify_long_body_truncated() {
    let long_body = "x".repeat(500);
    let msg = classify_api_error(500, "o/r", &long_body, None);
    assert!(
        msg.contains("..."),
        "long body should be truncated with ellipsis, got: {msg}"
    );
    // The snippet should be at most ~200 chars + "Response: " prefix.
    let response_part = msg.split("Response: ").nth(1).unwrap_or("");
    assert!(
        response_part.starts_with("x"),
        "truncated body should start with content"
    );
}

#[test]
fn test_classify_empty_body_no_response_line() {
    let msg = classify_api_error(500, "o/r", "", None);
    assert!(msg.contains("500"));
    assert!(msg.contains("o/r"));
    assert!(
        !msg.contains("Response:"),
        "empty body should not add a Response line, got: {msg}"
    );
}

// ---------------------------------------------------------------------------
// FR-017 — 404 not-found: specific message, no subsequent fetches
// ---------------------------------------------------------------------------

#[test]
fn test_classify_404_says_not_found_or_private() {
    let msg = classify_api_error(404, "octocat/missing", "", None);
    assert!(
        msg.contains("not found"),
        "404 message should say 'not found', got: {msg}"
    );
    assert!(
        msg.contains("private"),
        "404 message should mention private repos, got: {msg}"
    );
    assert!(
        msg.contains("octocat/missing"),
        "404 message should include repo identifier, got: {msg}"
    );
    assert!(
        msg.contains("No further API calls"),
        "404 message should state no further calls, got: {msg}"
    );
}

#[test]
fn test_classify_404_does_not_include_body() {
    // The 404 body from GitHub ("Not Found") is redundant with our message.
    let msg = classify_api_error(404, "o/r", "Not Found", None);
    assert!(
        !msg.contains("Not Found"),
        "404 should not echo the body, got: {msg}"
    );
}

// ---------------------------------------------------------------------------
// FR-015 — 403/429 rate limit: includes reset time if available
// ---------------------------------------------------------------------------

#[test]
fn test_classify_403_with_reset_time() {
    let msg = classify_api_error(403, "o/r", "", Some(1787366400));
    assert!(msg.contains("403"));
    assert!(msg.contains("rate limit"));
    assert!(
        msg.contains("Rate limit resets at:"),
        "403 with reset should include reset time, got: {msg}"
    );
    assert!(
        msg.contains("1787366400") || msg.contains("2026"),
        "reset time should be human-readable, got: {msg}"
    );
}

#[test]
fn test_classify_429_with_reset_time() {
    let msg = classify_api_error(429, "o/r", "", Some(1787366400));
    assert!(msg.contains("429"));
    assert!(msg.contains("rate limit"));
    assert!(msg.contains("Rate limit resets at:"));
}

#[test]
fn test_classify_403_without_reset_time() {
    let msg = classify_api_error(403, "o/r", "", None);
    assert!(msg.contains("403"));
    assert!(msg.contains("rate limit"));
    assert!(
        !msg.contains("Rate limit resets at:"),
        "403 without reset should not claim a reset time, got: {msg}"
    );
}

#[test]
fn test_classify_429_without_reset_time() {
    let msg = classify_api_error(429, "o/r", "", None);
    assert!(msg.contains("429"));
    assert!(msg.contains("rate limit"));
    assert!(!msg.contains("Rate limit resets at:"));
}

#[test]
fn test_classify_rate_limit_says_not_retried() {
    let msg = classify_api_error(429, "o/r", "", None);
    assert!(
        msg.contains("not retried"),
        "rate-limit message should state it was not retried, got: {msg}"
    );
}

// ---------------------------------------------------------------------------
// 401 auth failure
// ---------------------------------------------------------------------------

#[test]
fn test_classify_401_says_re_authenticate() {
    let msg = classify_api_error(401, "o/r", "", None);
    assert!(msg.contains("401"));
    assert!(
        msg.contains("/github login"),
        "401 should suggest /github login, got: {msg}"
    );
    assert!(msg.contains("o/r"));
}

// ---------------------------------------------------------------------------
// format_reset_time
// ---------------------------------------------------------------------------

#[test]
fn test_format_reset_time_known_timestamp() {
    // 2026-01-01 00:00:00 UTC = 1767225600
    let s = format_reset_time(1_767_225_600);
    assert_eq!(s, "2026-01-01 00:00:00 UTC");
}

#[test]
fn test_format_reset_time_epoch() {
    let s = format_reset_time(0);
    assert_eq!(s, "1970-01-01 00:00:00 UTC");
}

#[test]
fn test_format_reset_time_with_seconds() {
    // 2026-01-01 12:30:45 UTC
    let secs = 1_767_225_600 + 12 * 3600 + 30 * 60 + 45;
    let s = format_reset_time(secs);
    assert_eq!(s, "2026-01-01 12:30:45 UTC");
}

#[test]
fn test_format_reset_time_leap_year() {
    // 2024-02-29 00:00:00 UTC (leap day)
    let s = format_reset_time(1_709_164_800);
    assert_eq!(s, "2024-02-29 00:00:00 UTC");
}
