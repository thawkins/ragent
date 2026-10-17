//! Additional hermetic coverage for the auto-updater (`ragent_agent::updater`).
//!
//! Covers the version-comparison edge cases and the SEC-043 refusal paths of
//! [`download_and_replace_verified`], which fail *before* any network request
//! when the expected digest is absent or malformed.

use ragent_agent::updater::{
    CURRENT_VERSION, download_and_replace, download_and_replace_verified, is_newer,
};

#[test]
fn is_newer_handles_prerelease_and_prefix_variants() {
    // Equal with and without the `v` prefix.
    assert!(!is_newer("v1.2.3", "1.2.3"));
    // Major / minor / patch each dominate.
    assert!(is_newer("2.0.0", "1.9.9"));
    assert!(is_newer("1.3.0", "1.2.9"));
    assert!(is_newer("1.2.4", "1.2.3"));
    // The prerelease string participates in the tuple comparison, so an empty
    // prerelease sorts *below* a non-empty one (conservative: a bare release is
    // not treated as newer than its own prerelease).
    assert!(!is_newer("1.2.3", "1.2.3-beta.1"));
    // Garbage parses to (0,0,0,...) and never beats a real version.
    assert!(!is_newer("not-a-version", "1.0.0"));
}

#[test]
fn current_version_is_non_empty_semver_like() {
    assert_ne!(CURRENT_VERSION, "");
    assert_eq!(CURRENT_VERSION, env!("CARGO_PKG_VERSION"));
}

#[tokio::test]
async fn download_and_replace_verified_refuses_a_malformed_digest() {
    let err = download_and_replace_verified("http://127.0.0.1:1/none", Some("not-hex"))
        .await
        .expect_err("a malformed digest must be refused before any download");
    assert!(
        err.to_string()
            .contains("not a valid 64-character hex digest"),
        "err: {err}"
    );
}

#[tokio::test]
async fn download_and_replace_verified_refuses_a_short_digest() {
    let err = download_and_replace_verified("http://127.0.0.1:1/none", Some("abcd"))
        .await
        .expect_err("a short digest must be refused");
    assert!(
        err.to_string().contains("64-character hex digest"),
        "err: {err}"
    );
}

#[tokio::test]
async fn download_and_replace_accepts_the_sha256_prefix_form() {
    // A valid `sha256:`-prefixed digest of the wrong value must pass parsing but
    // fail at the download stage (unreachable host), proving the prefix is
    // stripped and the digest validated without a live server.
    let err = download_and_replace_verified(
        "http://127.0.0.1:1/none",
        Some("sha256:0000000000000000000000000000000000000000000000000000000000000000"),
    )
    .await
    .expect_err("unreachable host must fail");
    // The failure must be an HTTP/transport error, not a digest-validation error.
    let msg = err.to_string();
    assert!(
        !msg.contains("hex digest"),
        "digest should have been accepted: {msg}"
    );
}

#[tokio::test]
async fn download_and_replace_without_a_digest_errors() {
    // `download_and_replace` resolves the digest from `RAGENT_UPDATE_SHA256`.
    // When that variable is unset (the normal test environment) the call is
    // refused up-front. Skip the assertion if the ambient env happens to set it.
    if std::env::var("RAGENT_UPDATE_SHA256").is_ok() {
        return;
    }
    let err = download_and_replace("http://127.0.0.1:1/none")
        .await
        .expect_err("no digest must be refused");
    assert!(
        err.to_string().contains("no expected SHA-256 digest"),
        "err: {err}"
    );
}
