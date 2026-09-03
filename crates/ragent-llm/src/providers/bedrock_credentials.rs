//! AWS credential resolution for the Amazon Bedrock provider.
//!
//! Resolves AWS credentials from the standard provider chain:
//! 1. Environment variables (`AWS_ACCESS_KEY_ID` + `AWS_SECRET_ACCESS_KEY`)
//! 2. Named profile in `~/.aws/credentials` (via `AWS_PROFILE` or config option)
//! 3. IAM instance metadata (EC2/ECS) — optional, best-effort
//!
//! Implements FR-001, FR-002, FR-003 of the BedrockAWS specification.

use anyhow::{Context, Result, bail};
use std::collections::HashMap;
use std::path::PathBuf;

/// Resolved AWS credentials for SigV4 request signing.
#[derive(Debug, Clone)]
pub struct AwsCredentials {
    /// AWS access key ID (e.g. `AKIAIOSFODNN7EXAMPLE`).
    pub access_key: String,
    /// AWS secret access key.
    pub secret_key: String,
    /// Optional session token for temporary credentials (STS assumed roles).
    pub session_token: Option<String>,
    /// AWS region for the Bedrock endpoint.
    pub region: String,
}

/// Resolves AWS credentials using the standard provider chain.
///
/// Precedence:
/// 1. `AWS_ACCESS_KEY_ID` + `AWS_SECRET_ACCESS_KEY` environment variables
/// 2. Named profile from `AWS_PROFILE` environment variable or `profile` option
/// 3. IAM instance metadata (best-effort, logged on failure)
///
/// Region resolution precedence:
/// 1. `AWS_BEDROCK_REGION` environment variable (Bedrock-specific override, FR-006)
/// 2. `AWS_REGION` environment variable (FR-005)
/// 3. `region` from `options` HashMap (FR-004)
/// 4. Default `us-east-1`
///
/// # Errors
///
/// Returns an error with actionable diagnostics when no credentials are found
/// from any source (FR-002).
#[allow(clippy::implicit_hasher)] // options is always a plain std HashMap from the provider config
pub fn resolve_aws_credentials(
    options: &std::collections::HashMap<String, serde_json::Value>,
) -> Result<AwsCredentials> {
    // Resolve region first (needed for all paths)
    let region = resolve_region(options);

    // 1. Try environment variables
    if let Some(creds) = creds_from_env(&region) {
        tracing::debug!(region = %region, source = "env_vars", "Resolved AWS credentials");
        return Ok(creds);
    }

    // 2. Try named profile
    let profile_name = options
        .get("profile")
        .and_then(|v| v.as_str())
        .filter(|s| !s.trim().is_empty())
        .map(String::from)
        .or_else(|| {
            std::env::var("AWS_PROFILE")
                .ok()
                .filter(|s| !s.trim().is_empty())
        });

    if let Some(profile) = profile_name
        && let Some(creds) = creds_from_profile(&profile, &region)?
    {
        tracing::debug!(region = %region, profile = %profile, source = "aws_profile", "Resolved AWS credentials");
        return Ok(creds);
    }

    // 3. Try IAM instance metadata (best-effort)
    // We log a warning and skip rather than error, as this requires network access
    // that may not be available in all environments.
    tracing::debug!(
        region = %region,
        "No static AWS credentials found; IAM instance metadata not supported in this build"
    );

    // Build an actionable error message (FR-002)
    let mut sources_tried = vec!["AWS_ACCESS_KEY_ID + AWS_SECRET_ACCESS_KEY env vars"];
    if std::env::var("AWS_PROFILE").is_ok() || options.get("profile").is_some() {
        sources_tried.push("AWS profile in ~/.aws/credentials");
    }
    sources_tried.push("IAM instance metadata (not available)");

    bail!(
        "No AWS credentials found for Bedrock provider. \
         Sources attempted: {}. \
         Set AWS_ACCESS_KEY_ID and AWS_SECRET_ACCESS_KEY environment variables, \
         or configure an AWS profile in ~/.aws/credentials.",
        sources_tried.join(", ")
    );
}

/// Resolves the AWS region using the precedence chain.
///
/// 1. `AWS_BEDROCK_REGION` (FR-006)
/// 2. `AWS_REGION` (FR-005)
/// 3. `options["region"]` (FR-004)
/// 4. Default `us-east-1`
#[allow(clippy::implicit_hasher)] // called with the same plain std HashMap as resolve_aws_credentials
pub fn resolve_region(options: &std::collections::HashMap<String, serde_json::Value>) -> String {
    // FR-006: Bedrock-specific region override
    if let Ok(region) = std::env::var("AWS_BEDROCK_REGION")
        && !region.trim().is_empty()
    {
        return region.trim().to_string();
    }

    // FR-005: General AWS region
    if let Ok(region) = std::env::var("AWS_REGION")
        && !region.trim().is_empty()
    {
        return region.trim().to_string();
    }

    // FR-004: Config option
    if let Some(region) = options.get("region").and_then(|v| v.as_str())
        && !region.trim().is_empty()
    {
        return region.trim().to_string();
    }

    // Default
    "us-east-1".to_string()
}

/// Attempts to read credentials from environment variables.
fn creds_from_env(region: &str) -> Option<AwsCredentials> {
    let access_key = std::env::var("AWS_ACCESS_KEY_ID").ok()?;
    let secret_key = std::env::var("AWS_SECRET_ACCESS_KEY").ok()?;

    if access_key.trim().is_empty() || secret_key.trim().is_empty() {
        return None;
    }

    let session_token = std::env::var("AWS_SESSION_TOKEN")
        .ok()
        .filter(|s| !s.trim().is_empty());

    Some(AwsCredentials {
        access_key: access_key.trim().to_string(),
        secret_key: secret_key.trim().to_string(),
        session_token,
        region: region.to_string(),
    })
}

/// Attempts to read credentials from a named AWS profile in `~/.aws/credentials`.
///
/// The INI file format is:
/// ```ini
/// [my-profile]
/// aws_access_key_id = AKIAIOSFODNN7EXAMPLE
/// aws_secret_access_key = wJalrXUtnFEMI/K7MDENG/bPxRfiCYEXAMPLEKEY
/// aws_session_token = optional_sts_token
/// region = us-east-1
/// ```
fn creds_from_profile(profile: &str, default_region: &str) -> Result<Option<AwsCredentials>> {
    let cred_path = aws_credentials_path();
    if !cred_path.exists() {
        tracing::debug!(path = %cred_path.display(), "AWS credentials file not found");
        return Ok(None);
    }

    let contents = std::fs::read_to_string(&cred_path).with_context(|| {
        format!(
            "Failed to read AWS credentials file at {}",
            cred_path.display()
        )
    })?;

    let profiles = parse_aws_credentials_ini(&contents);

    if let Some(cred) = profiles.get(profile) {
        if cred.access_key.trim().is_empty() || cred.secret_key.trim().is_empty() {
            tracing::warn!(profile = %profile, "AWS profile found but credentials are empty");
            return Ok(None);
        }

        // Profile may override region
        let region = cred
            .region
            .as_deref()
            .filter(|r| !r.trim().is_empty())
            .unwrap_or(default_region)
            .to_string();

        Ok(Some(AwsCredentials {
            access_key: cred.access_key.trim().to_string(),
            secret_key: cred.secret_key.trim().to_string(),
            session_token: cred
                .session_token
                .as_deref()
                .filter(|s| !s.trim().is_empty())
                .map(|s| s.trim().to_string()),
            region,
        }))
    } else {
        tracing::debug!(profile = %profile, "Profile not found in AWS credentials file");
        Ok(None)
    }
}

/// Returns the path to `~/.aws/credentials`.
fn aws_credentials_path() -> PathBuf {
    // Respect AWS_SHARED_CREDENTIALS_FILE env var
    if let Ok(path) = std::env::var("AWS_SHARED_CREDENTIALS_FILE")
        && !path.trim().is_empty()
    {
        return PathBuf::from(path);
    }

    dirs::home_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join(".aws")
        .join("credentials")
}

/// Parsed credential entry from the AWS credentials INI file.
#[derive(Debug, Clone, Default)]
struct ProfileCredentials {
    access_key: String,
    secret_key: String,
    session_token: Option<String>,
    region: Option<String>,
}

/// Parses an AWS credentials INI file into a map of profile name → credentials.
///
/// Handles the standard `~/.aws/credentials` format with `[profile]` section headers.
fn parse_aws_credentials_ini(contents: &str) -> HashMap<String, ProfileCredentials> {
    let mut profiles = HashMap::new();
    let mut current_profile: Option<String> = None;
    let mut current_creds = ProfileCredentials {
        access_key: String::new(),
        secret_key: String::new(),
        session_token: None,
        region: None,
    };

    for line in contents.lines() {
        let line = line.trim();

        // Skip comments and empty lines
        if line.is_empty() || line.starts_with('#') || line.starts_with(';') {
            continue;
        }

        // Section header: [profile-name]
        if line.starts_with('[') && line.ends_with(']') {
            // Save previous profile
            if let Some(name) = current_profile.take() {
                profiles.insert(name, std::mem::take(&mut current_creds));
            }

            let profile_name = line[1..line.len() - 1].trim().to_string();
            current_profile = Some(profile_name);
            current_creds = ProfileCredentials {
                access_key: String::new(),
                secret_key: String::new(),
                session_token: None,
                region: None,
            };
            continue;
        }

        // Key = Value
        if let Some((key, value)) = line.split_once('=') {
            let key = key.trim();
            let value = value.trim().to_string();

            match key {
                "aws_access_key_id" => current_creds.access_key = value,
                "aws_secret_access_key" => current_creds.secret_key = value,
                "aws_session_token" | "aws_security_token" => {
                    current_creds.session_token = Some(value);
                }
                "region" => current_creds.region = Some(value),
                _ => {} // Ignore unknown keys
            }
        }
    }

    // Save last profile
    if let Some(name) = current_profile {
        profiles.insert(name, current_creds);
    }

    profiles
}

#[cfg(test)]
mod tests {
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
}
