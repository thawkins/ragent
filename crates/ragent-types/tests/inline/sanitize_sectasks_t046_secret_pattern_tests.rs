//! Inline tests for `sanitize.rs` - compiled inside the module tree
//! via the `#[path]` delegation left in the source file
//! (ANTIPAT M2 inline-test migration; no behaviour change).

use super::*;

/// SEC-ragent-types-004 (SECTASKS T-046): the pattern layer must cover the
/// credential shapes that were previously unregistered.
#[test]
fn redacts_gitlab_pat_and_env_sourced_credentials() {
    let gitlab = "glpat-abcdefghijklmnopqrst";
    let out = redact_secrets(&format!("token is {gitlab} here"));
    assert!(!out.contains(gitlab), "GitLab PAT leaked: {out}");

    let hf = "hf_abcdefghijklmnopqrstuvwx";
    let out = redact_secrets(&format!("use {hf} for the model"));
    assert!(!out.contains(hf), "Hugging Face token leaked: {out}");

    let npm = "npm_abcdefghijklmnopqrstuvwx";
    let out = redact_secrets(&format!("auth {npm}"));
    assert!(!out.contains(npm), "npm token leaked: {out}");
}

/// A bare `?key=` query parameter (the Google Gemini form) must redact.
#[test]
fn redacts_query_string_key_parameter() {
    let url = "https://generativelanguage.googleapis.com/v1beta/models?key=AIzaSyABCDEFGHIJKLMNOP";
    let out = redact_secrets(url);
    assert!(
        !out.contains("AIzaSyABCDEFGHIJKLMNOP"),
        "query key leaked: {out}"
    );
}
