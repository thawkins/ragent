//! Environment-driven coverage for `bedrock_credentials`.
//!
//! `std::env::set_var` / `remove_var` are `unsafe` in Rust 2024; this test
//! target opts back in explicitly and mutates the `AWS_*` / `AWS_SHARED_
//! CREDENTIALS_FILE` variables only while a process-wide mutex guard is held.
//! Each integration-test file is its own process, so no other test binary
//! observes these mutations.

#![allow(unsafe_code)]

use ragent_llm::providers::bedrock_credentials::{resolve_aws_credentials, resolve_region};
use std::collections::HashMap;
use std::io::Write;

/// The process-wide env lock (see `tests/all.rs`); one binary per crate
/// shares one process, so every env-mutating file contends on this lock.
fn env_lock() -> std::sync::MutexGuard<'static, ()> {
    crate::global_env_lock()
}

const AWS_VARS: &[&str] = &[
    "AWS_ACCESS_KEY_ID",
    "AWS_SECRET_ACCESS_KEY",
    "AWS_SESSION_TOKEN",
    "AWS_REGION",
    "AWS_BEDROCK_REGION",
    "AWS_PROFILE",
    "AWS_SHARED_CREDENTIALS_FILE",
];

fn clear_aws_env() {
    for var in AWS_VARS {
        // SAFETY: exclusive access to the AWS_* variables via ENV_LOCK.
        unsafe { std::env::remove_var(var) };
    }
}

#[test]
fn test_creds_from_env_roundtrip() {
    let _guard = env_lock();
    clear_aws_env();

    // SAFETY: exclusive access to the AWS_* variables via ENV_LOCK.
    unsafe {
        std::env::set_var("AWS_ACCESS_KEY_ID", "AKIATESTKEY");
        std::env::set_var("AWS_SECRET_ACCESS_KEY", "secretvalue");
        std::env::set_var("AWS_SESSION_TOKEN", "sessiontoken");
        std::env::set_var("AWS_BEDROCK_REGION", "eu-central-1");
    }

    let creds = resolve_aws_credentials(&HashMap::new()).expect("env creds");
    assert_eq!(creds.access_key, "AKIATESTKEY");
    assert_eq!(creds.secret_key, "secretvalue");
    assert_eq!(creds.session_token.as_deref(), Some("sessiontoken"));
    assert_eq!(creds.region, "eu-central-1");

    clear_aws_env();
}

#[test]
fn test_resolve_region_precedence_env_over_options() {
    let _guard = env_lock();
    clear_aws_env();

    let mut options = HashMap::new();
    options.insert("region".to_string(), serde_json::json!("ap-south-1"));

    // Options win when no env override is set.
    assert_eq!(resolve_region(&options), "ap-south-1");

    // AWS_REGION beats options.
    // SAFETY: exclusive access via ENV_LOCK.
    unsafe { std::env::set_var("AWS_REGION", "us-west-2") };
    assert_eq!(resolve_region(&options), "us-west-2");

    // AWS_BEDROCK_REGION beats AWS_REGION.
    // SAFETY: exclusive access via ENV_LOCK.
    unsafe { std::env::set_var("AWS_BEDROCK_REGION", "us-gov-west-1") };
    assert_eq!(resolve_region(&options), "us-gov-west-1");

    clear_aws_env();

    // Default when nothing is set.
    assert_eq!(resolve_region(&HashMap::new()), "us-east-1");
}

#[test]
fn test_resolve_aws_credentials_profile_from_file() {
    let _guard = env_lock();
    clear_aws_env();

    let mut file = tempfile::NamedTempFile::new().expect("tempfile");
    file.write_all(
        b"[work]\naws_access_key_id = AKIAPROFILE\naws_secret_access_key = profilesecret\nregion = eu-west-3\n",
    )
    .expect("write");
    file.flush().expect("flush");

    // Point the resolver at the temp credentials file and select the profile.
    // SAFETY: exclusive access via ENV_LOCK.
    unsafe {
        std::env::set_var(
            "AWS_SHARED_CREDENTIALS_FILE",
            file.path().to_string_lossy().to_string(),
        );
    }
    let mut options = HashMap::new();
    options.insert("profile".to_string(), serde_json::json!("work"));

    let creds = resolve_aws_credentials(&options).expect("profile creds");
    assert_eq!(creds.access_key, "AKIAPROFILE");
    assert_eq!(creds.secret_key, "profilesecret");
    assert_eq!(creds.region, "eu-west-3");

    clear_aws_env();
}

#[test]
fn test_resolve_aws_credentials_errors_when_no_source() {
    let _guard = env_lock();
    clear_aws_env();

    // Point the credentials file at a nonexistent path so the profile path
    // cannot accidentally succeed on the developer's machine.
    // SAFETY: exclusive access via ENV_LOCK.
    unsafe {
        std::env::set_var(
            "AWS_SHARED_CREDENTIALS_FILE",
            "/nonexistent/aws/credentials",
        );
    }
    let mut options = HashMap::new();
    options.insert("region".to_string(), serde_json::json!("us-east-1"));

    let err = resolve_aws_credentials(&options)
        .expect_err("should fail without credentials")
        .to_string();
    assert!(err.contains("No AWS credentials found"), "error: {err}");
    assert!(!err.contains("AKIA"), "error must not leak keys: {err}");

    clear_aws_env();
}
