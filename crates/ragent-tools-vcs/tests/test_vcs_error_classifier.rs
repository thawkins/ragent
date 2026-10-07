//! Integration tests for the shared VCS error classifier (T-308).
//!
//! The single shape rule lives in `ragent_tools_vcs::vcs_error::classify_status`
//! and is used by both the GitHub and GitLab clients.

use ragent_tools_vcs::vcs_error::{VcsStatus, classify_status};

#[test]
fn test_classify_status_429_is_rate_limited() {
    assert_eq!(classify_status(429, None), VcsStatus::RateLimited);
    // A 429 is a rate limit regardless of any remaining-quota header.
    assert_eq!(classify_status(429, Some(100)), VcsStatus::RateLimited);
}

#[test]
fn test_classify_status_403_without_exhausted_quota_is_permission_denied() {
    // No header (GitLab sends none): permission denial.
    assert_eq!(classify_status(403, None), VcsStatus::PermissionDenied);
    // Quota still available: permission denial (ANTIPAT I-5).
    assert_eq!(
        classify_status(403, Some(5000)),
        VcsStatus::PermissionDenied
    );
}

#[test]
fn test_classify_status_403_with_exhausted_quota_is_rate_limited() {
    assert_eq!(classify_status(403, Some(0)), VcsStatus::RateLimited);
}

#[test]
fn test_classify_status_401_is_unauthorized() {
    assert_eq!(classify_status(401, None), VcsStatus::Unauthorized);
}

#[test]
fn test_classify_status_success_and_other_are_other() {
    assert_eq!(classify_status(200, None), VcsStatus::Other);
    assert_eq!(classify_status(404, None), VcsStatus::Other);
    assert_eq!(classify_status(500, None), VcsStatus::Other);
}
