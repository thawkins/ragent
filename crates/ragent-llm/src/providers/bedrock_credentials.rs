//! AWS credential resolution for the Amazon Bedrock provider.
//!
//! Resolves AWS credentials from the standard provider chain:
//! 1. Environment variables (`AWS_ACCESS_KEY_ID` + `AWS_SECRET_ACCESS_KEY`)
//! 2. Named profile in `~/.aws/credentials` (via `AWS_PROFILE` or config option)
//! 3. IAM instance metadata (EC2/ECS) - optional, best-effort
//!
//! Implements FR-001, FR-002, FR-003 of the BedrockAWS specification.
//!
//! Audit T-111: the credential env reads below intentionally use `std::env`
//! directly. This is the AWS provider-chain module named in the audit's
//! documented allow-list ("a documented allow-list may remain for AWS
//! SDK-style reads"): `AWS_PROFILE` / `AWS_REGION` / `AWS_SHARED_CREDENTIALS_FILE`
//! are chain *selectors*, not credentials, and the key/session pair follows the
//! official AWS resolution order, including a blank-as-unset policy that is
//! verified inline below.

use anyhow::{Context, Result, bail};
use std::collections::HashMap;
use std::path::PathBuf;

/// Resolved AWS credentials for SigV4 request signing.
///
/// `Debug` is implemented by hand: the derived form would print `secret_key`
/// and `session_token` verbatim into any `tracing::debug!(creds = ?..)` field
/// (SEC-ragent-llm-007 / SECTASKS T-062).
#[derive(Clone)]
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

impl std::fmt::Debug for AwsCredentials {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AwsCredentials")
            .field("access_key", &self.access_key)
            .field("secret_key", &"[REDACTED]")
            .field(
                "session_token",
                &self.session_token.as_ref().map(|_| "[REDACTED]"),
            )
            .field("region", &self.region)
            .finish()
    }
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
    // Canonical blank-as-unset, trimmed credential read (audit T-111).
    let access_key = ragent_config::credential_env::read_credential_env("AWS_ACCESS_KEY_ID")?;
    let secret_key = ragent_config::credential_env::read_credential_env("AWS_SECRET_ACCESS_KEY")?;

    let session_token = ragent_config::credential_env::read_credential_env("AWS_SESSION_TOKEN");

    Some(AwsCredentials {
        access_key,
        secret_key,
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

/// Parses an AWS credentials INI file into a map of profile name -> credentials.
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
#[path = "../tests/inline/bedrock_credentials_tests.rs"]
mod tests;
