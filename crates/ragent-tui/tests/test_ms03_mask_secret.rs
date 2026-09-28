//! MS-03 regression tests for credential masking in the TUI.
//!
//! SEC-ragent-tui-002 (SECTASKS T-045): the provider-setup dialog rendered the
//! API key and the GitLab PAT verbatim, so a live credential was readable
//! during a screen share, a terminal recording, or a shoulder-surf.

use ragent_tui::layout::mask_secret;

#[test]
fn test_short_secret_is_fully_masked() {
    assert_eq!(mask_secret("abc"), "***");
    assert_eq!(mask_secret("123456789012"), "************");
    assert_eq!(mask_secret(""), "");
}

#[test]
fn test_long_secret_keeps_only_the_ends() {
    let masked = mask_secret("sk-ant-api03-abcdefghijklmnop");
    assert!(masked.starts_with("sk-a"), "prefix kept: {masked}");
    assert!(masked.ends_with("mnop"), "suffix kept: {masked}");
    assert!(!masked.contains("api03"), "middle must be masked: {masked}");
    assert_eq!(
        masked.chars().count(),
        "sk-ant-api03-abcdefghijklmnop".len()
    );
}

#[test]
fn test_gitlab_pat_is_masked() {
    let masked = mask_secret("glpat-abcdefghijklmnopqrst");
    assert!(
        !masked.contains("abcdefghij"),
        "token body must be hidden: {masked}"
    );
    assert!(masked.starts_with("glpa"));
}
