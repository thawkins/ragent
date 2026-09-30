//! Cross-provider vocabulary helpers for the VCS tools.
//!
//! GitHub and GitLab spell the same concept differently. The one that matters
//! at the tool boundary is the issue/pull-request `state` filter
//! (ANTIPAT.md I-2):
//!
//! | Canonical | GitHub accepts | GitLab issues | GitLab MRs |
//! |-----------|----------------|---------------|------------|
//! | `open`    | `open`         | `opened`      | `opened`   |
//! | `closed`  | `closed`       | `closed`      | `closed`   |
//! | `merged`  | n/a            | n/a           | `merged`   |
//! | `all`     | `all`          | `all`         | `all`      |
//!
//! To keep callers from having to know which provider they are talking to,
//! every state-bearing tool routes its `state` input through [`normalize_issue_state`]
//! so both `open` and `opened` are accepted everywhere, and `merged` is mapped
//! to `opened` on providers that do not support it.

/// Normalise an issue / pull-request `state` input to GitHub's native spelling.
///
/// GitHub natively uses `open`, so `opened` (GitLab's spelling) is accepted and
/// translated. `merged` is a GitLab-MR-only state and has no GitHub equivalent,
/// so it falls back to `open` (an unrecognised state would otherwise be rejected
/// by the API).
#[must_use]
pub fn normalize_issue_state(input: &str) -> &'static str {
    match input {
        "opened" | "open" => "open",
        "closed" => "closed",
        "merged" => "open",
        _ => "all",
    }
}

/// Normalise an issue / merge-request `state` input to GitLab's native spelling.
///
/// GitLab uses `opened`, so `open` (GitHub's spelling) is accepted and
/// translated. `merged` is only meaningful for merge requests; for issues it
/// falls back to `opened` (GitLab's issues endpoint rejects `merged`).
#[must_use]
pub fn normalize_gitlab_state(input: &str) -> &'static str {
    match input {
        "open" | "opened" => "opened",
        "closed" => "closed",
        "merged" => "merged",
        _ => "all",
    }
}

/// Normalise an issue `state` input to GitLab's native spelling.
///
/// Same as [`normalize_gitlab_state`] except that `merged` has no meaning for
/// issues and maps to `opened`.
#[must_use]
pub fn normalize_gitlab_issue_state(input: &str) -> &'static str {
    match input {
        "open" | "opened" | "merged" => "opened",
        "closed" => "closed",
        _ => "all",
    }
}
