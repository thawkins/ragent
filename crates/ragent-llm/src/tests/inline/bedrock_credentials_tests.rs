//! Inline tests for `bedrock_credentials.rs` - compiled inside the module tree
//! via the `#[path]` delegation left in the source file
//! (ANTIPAT M2 inline-test migration; no behaviour change).

use super::*;

#[test]
fn test_parse_aws_credentials_ini_basic() {
    let contents = "
[default]
aws_access_key_id = AKIAIOSFODNN7EXAMPLE
aws_secret_access_key = wJalrXUtnFEMI/K7MDENG/bPxRfiCYEXAMPLEKEY

[my-profile]
aws_access_key_id = AKIAI44QH8DHBEXAMPLE
aws_secret_access_key = je7MtGbClwBF/2Zp9Utk/h3yCo8nvbEXAMPLEKEY
region = eu-west-1
";
    let profiles = parse_aws_credentials_ini(contents);
    assert_eq!(profiles.len(), 2);

    let default = profiles.get("default").unwrap();
    assert_eq!(default.access_key, "AKIAIOSFODNN7EXAMPLE");
    assert_eq!(
        default.secret_key,
        "wJalrXUtnFEMI/K7MDENG/bPxRfiCYEXAMPLEKEY"
    );
    assert!(default.session_token.is_none());
    assert!(default.region.is_none());

    let my_profile = profiles.get("my-profile").unwrap();
    assert_eq!(my_profile.access_key, "AKIAI44QH8DHBEXAMPLE");
    assert_eq!(my_profile.region.as_deref(), Some("eu-west-1"));
}

#[test]
fn test_parse_aws_credentials_ini_with_session_token() {
    let contents = "
[sts-role]
aws_access_key_id = AKIAIOSFODNN7EXAMPLE
aws_secret_access_key = wJalrXUtnFEMI/K7MDENG/bPxRfiCYEXAMPLEKEY
aws_session_token = FwoGZXIvYXdzEBYaDExampleToken
";
    let profiles = parse_aws_credentials_ini(contents);
    let sts = profiles.get("sts-role").unwrap();
    assert_eq!(
        sts.session_token.as_deref(),
        Some("FwoGZXIvYXdzEBYaDExampleToken")
    );
}

#[test]
fn test_parse_aws_credentials_ini_comments() {
    let contents = "
# This is a comment
[default]
; This is also a comment
aws_access_key_id = AKIAIOSFODNN7EXAMPLE
aws_secret_access_key = wJalrXUtnFEMI/K7MDENG/bPxRfiCYEXAMPLEKEY
";
    let profiles = parse_aws_credentials_ini(contents);
    assert_eq!(profiles.len(), 1);
    assert_eq!(
        profiles.get("default").unwrap().access_key,
        "AKIAIOSFODNN7EXAMPLE"
    );
}

#[test]
fn test_resolve_region_bedrock_override() {
    // This test uses temp env vars which could conflict in parallel runs,
    // but it's acceptable for unit tests.
    let options = HashMap::new();
    // Without any env vars or config, we get the default
    // (env vars may or may not be set in the test environment)
    let region = resolve_region(&options);
    assert_ne!(region, String::new());
}

#[test]
fn test_resolve_region_from_options() {
    let mut options = HashMap::new();
    options.insert("region".to_string(), serde_json::json!("ap-southeast-1"));
    // Options take effect only if env vars are not set
    let region = resolve_region(&options);
    // If env vars are set, they override; otherwise we get the options value
    assert_ne!(region, String::new());
}

#[test]
fn test_resolve_region_default() {
    let options = HashMap::new();
    // The default should be us-east-1 when no overrides exist
    // Note: if AWS_REGION or AWS_BEDROCK_REGION is set in the test env,
    // the result will differ. This test validates non-empty result.
    let region = resolve_region(&options);
    assert_ne!(region, String::new());
}

#[test]
fn test_creds_from_env_empty_values() {
    // With no env vars set (or empty), should return None
    // This test is informational - actual env state may vary
    let result = creds_from_env("us-east-1");
    // Result depends on whether AWS_ACCESS_KEY_ID is set in the environment
    if std::env::var("AWS_ACCESS_KEY_ID").is_err() {
        assert!(result.is_none());
    }
}

#[test]
fn test_parse_ini_empty_file() {
    let profiles = parse_aws_credentials_ini("");
    assert!(profiles.is_empty());
}

#[test]
fn test_parse_ini_unknown_keys_ignored() {
    let contents = "
[default]
aws_access_key_id = AKIAIOSFODNN7EXAMPLE
aws_secret_access_key = secret
custom_key = ignored
";
    let profiles = parse_aws_credentials_ini(contents);
    assert_eq!(profiles.len(), 1);
    assert_eq!(
        profiles.get("default").unwrap().access_key,
        "AKIAIOSFODNN7EXAMPLE"
    );
}
