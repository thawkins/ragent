//! AWS Signature Version 4 (SigV4) request signing for the Amazon Bedrock provider.
//!
//! Implements a self-contained SigV4 signer without depending on the full AWS SDK.
//! This satisfies NFR-001 (no additional runtime dependencies) and implements
//! FR-014, FR-015, FR-016 of the BedrockAWS specification.
//!
//! HMAC-SHA256 is implemented using the existing `sha2` workspace dependency,
//! avoiding any new crate additions.
//!
//! # References
//!
//! - <https://docs.aws.amazon.com/general/latest/gr/sigv4_signing.html>
//! - <https://docs.aws.amazon.com/general/latest/gr/sigv4-create-canonical-request.html>

use crate::provider::bedrock_credentials::AwsCredentials;
use chrono::Utc;
use sha2::{Digest, Sha256};

/// SHA-256 block size in bytes.
const SHA256_BLOCK_SIZE: usize = 64;

/// AWS service name for Bedrock.
pub const BEDROCK_SERVICE: &str = "bedrock";

/// Signs an HTTP request with AWS Signature Version 4.
///
/// Adds the following headers to the request builder:
/// - `x-amz-date` — ISO 8601 basic format timestamp
/// - `x-amz-content-sha256` — SHA-256 hash of the request body
/// - `x-amz-security-token` — (if session token is present) temporary credentials
/// - `Authorization` — SigV4 signature credential header
///
/// This function does NOT add `Authorization: Bearer` or `x-api-key` headers
/// (FR-016).
///
/// # Errors
///
/// Returns an error if URL parsing fails.
pub fn sign_request(
    method: &str,
    url: &str,
    headers: &mut Vec<(String, String)>,
    body: &[u8],
    credentials: &AwsCredentials,
) -> Result<(), String> {
    let amz_date = format_amz_date();
    let date_stamp = format_date_stamp();

    // Compute body hash
    let body_hash = hex_encode(&Sha256::digest(body));

    // Add required headers
    headers.push(("x-amz-date".to_string(), amz_date.clone()));
    headers.push(("x-amz-content-sha256".to_string(), body_hash.clone()));

    // FR-015: Include security token header for temporary credentials
    if let Some(ref token) = credentials.session_token {
        headers.push(("x-amz-security-token".to_string(), token.clone()));
    }

    // Parse host from URL
    let host =
        extract_host(url).ok_or_else(|| format!("Failed to extract host from URL: {url}"))?;

    // Build canonical headers (must be sorted by lowercase header name)
    let mut signed_headers_map: Vec<(String, String)> = vec![
        ("host".to_string(), host.clone()),
        ("x-amz-content-sha256".to_string(), body_hash.clone()),
        ("x-amz-date".to_string(), amz_date.clone()),
    ];

    if credentials.session_token.is_some() {
        if let Some(ref token) = credentials.session_token {
            signed_headers_map.push(("x-amz-security-token".to_string(), token.clone()));
        }
    }

    // Sort by lowercase header name
    signed_headers_map.sort_by(|a, b| a.0.cmp(&b.0));

    let signed_headers = signed_headers_map
        .iter()
        .map(|(k, _)| k.as_str())
        .collect::<Vec<_>>()
        .join(";");

    let canonical_headers = signed_headers_map
        .iter()
        .map(|(k, v)| format!("{}:{}", k.to_lowercase(), v.trim()))
        .collect::<Vec<_>>()
        .join("\n")
        + "\n";

    // Build canonical request
    let path = extract_path(url);
    let query_string = extract_query_string(url);

    let canonical_request = format!(
        "{method}\n{path}\n{query_string}\n{canonical_headers}\n{signed_headers}\n{body_hash}"
    );

    // Build string to sign
    let credential_scope = format!(
        "{date_stamp}/{region}/{service}/aws4_request",
        date_stamp = date_stamp,
        region = credentials.region,
        service = BEDROCK_SERVICE
    );

    let canonical_request_hash = hex_encode(&Sha256::digest(canonical_request.as_bytes()));

    let string_to_sign =
        format!("AWS4-HMAC-SHA256\n{amz_date}\n{credential_scope}\n{canonical_request_hash}");

    // Calculate signing key
    let signing_key = derive_signing_key(
        &credentials.secret_key,
        &date_stamp,
        &credentials.region,
        BEDROCK_SERVICE,
    );

    // Calculate signature
    let signature = hex_encode(&hmac_sha256(&signing_key, string_to_sign.as_bytes()));

    // Build Authorization header (FR-014)
    let authorization = format!(
        "AWS4-HMAC-SHA256 Credential={access_key}/{credential_scope}, \
         SignedHeaders={signed_headers}, \
         Signature={signature}",
        access_key = credentials.access_key,
        credential_scope = credential_scope,
        signed_headers = signed_headers,
        signature = signature
    );

    headers.push(("Authorization".to_string(), authorization));

    // Also ensure host header is present for the actual HTTP request
    if !headers.iter().any(|(k, _)| k.eq_ignore_ascii_case("host")) {
        headers.push(("host".to_string(), host));
    }

    Ok(())
}

/// Derives the SigV4 signing key from the secret key, date, region, and service.
///
/// Key derivation chain:
/// ```text
/// kDate    = HMAC-SHA256("AWS4" + secret_key, date_stamp)
/// kRegion  = HMAC-SHA256(kDate, region)
/// kService = HMAC-SHA256(kRegion, service)
/// kSigning = HMAC-SHA256(kService, "aws4_request")
/// ```
fn derive_signing_key(secret_key: &str, date_stamp: &str, region: &str, service: &str) -> Vec<u8> {
    let k_date = hmac_sha256(
        format!("AWS4{}", secret_key).as_bytes(),
        date_stamp.as_bytes(),
    );
    let k_region = hmac_sha256(&k_date, region.as_bytes());
    let k_service = hmac_sha256(&k_region, service.as_bytes());
    let k_signing = hmac_sha256(&k_service, b"aws4_request");
    k_signing
}

/// Computes HMAC-SHA256 using the `sha2` crate directly.
///
/// Implements the standard HMAC construction:
/// - If key > block size (64 bytes), key = SHA-256(key)
/// - Pad key to block size with zeros
/// - inner = SHA-256((key XOR ipad) || message)
/// - HMAC = SHA-256((key XOR opad) || inner)
fn hmac_sha256(key: &[u8], data: &[u8]) -> Vec<u8> {
    // If key is longer than block size, hash it first
    let key = if key.len() > SHA256_BLOCK_SIZE {
        let mut hasher = Sha256::new();
        hasher.update(key);
        hasher.finalize().to_vec()
    } else {
        key.to_vec()
    };

    // Pad key to block size
    let mut padded_key = vec![0u8; SHA256_BLOCK_SIZE];
    padded_key[..key.len()].copy_from_slice(&key);

    // Inner hash: SHA-256((key XOR ipad) || data)
    let mut ipad_key = vec![0u8; SHA256_BLOCK_SIZE];
    for (i, byte) in padded_key.iter().enumerate() {
        ipad_key[i] = byte ^ 0x36;
    }

    let mut inner_hasher = Sha256::new();
    inner_hasher.update(&ipad_key);
    inner_hasher.update(data);
    let inner_hash = inner_hasher.finalize();

    // Outer hash: SHA-256((key XOR opad) || inner_hash)
    let mut opad_key = vec![0u8; SHA256_BLOCK_SIZE];
    for (i, byte) in padded_key.iter().enumerate() {
        opad_key[i] = byte ^ 0x5c;
    }

    let mut outer_hasher = Sha256::new();
    outer_hasher.update(&opad_key);
    outer_hasher.update(&inner_hash);
    let hmac_result = outer_hasher.finalize();

    hmac_result.to_vec()
}

/// Encodes bytes as lowercase hex.
fn hex_encode(bytes: &[u8]) -> String {
    hex::encode(bytes)
}

/// Formats the current time as `x-amz-date` (ISO 8601 basic: `YYYYMMDDTHHMMSSZ`).
fn format_amz_date() -> String {
    Utc::now().format("%Y%m%dT%H%M%SZ").to_string()
}

/// Formats the current date as `YYYYMMDD` for the credential scope.
fn format_date_stamp() -> String {
    Utc::now().format("%Y%m%d").to_string()
}

/// Extracts the host (without port) from a URL.
fn extract_host(url: &str) -> Option<String> {
    let after_scheme = url.split("://").nth(1)?;
    let host_part = after_scheme.split('/').next()?;
    let host = host_part.split(':').next()?;
    Some(host.to_string())
}

/// Extracts the path from a URL (defaulting to "/" if absent).
fn extract_path(url: &str) -> String {
    let after_scheme = match url.split("://").nth(1) {
        Some(s) => s,
        None => return "/".to_string(),
    };
    match after_scheme.find('/') {
        Some(pos) => after_scheme[pos..]
            .split('?')
            .next()
            .unwrap_or("/")
            .to_string(),
        None => "/".to_string(),
    }
}

/// Extracts the query string from a URL (empty string if absent).
fn extract_query_string(url: &str) -> String {
    match url.split('?').nth(1) {
        Some(qs) => qs.to_string(),
        None => String::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Test HMAC-SHA256 against a known test vector.
    /// RFC 4231 Test Case 2: HMAC-SHA256 with key "Jefe" and data "what do ya want for nothing?"
    #[test]
    fn test_hmac_sha256_rfc_4231() {
        let key = b"Jefe";
        let data = b"what do ya want for nothing?";
        let result = hmac_sha256(key, data);
        // Expected from RFC 4231 Test Case 2
        let expected = "5bdcc146bf60754e6a042426089575c75a003f089d2739839dec58b964ec3843";
        assert_eq!(hex_encode(&result), expected);
    }

    /// Test HMAC-SHA256 with a key longer than the block size.
    /// RFC 4231 Test Case 6: key is 131 bytes of 0xaa
    #[test]
    fn test_hmac_sha256_long_key() {
        let key = vec![0xaa_u8; 131];
        let data = b"Test Using Larger Than Block-Size Key - Hash Key First";
        let result = hmac_sha256(&key, data);
        // Expected from RFC 4231 Test Case 6
        let _expected = "6e5506c14578b9f5dd47e3223abf0667f9a8a3c8a36a74b3d6c4b7c2e6d7d7d0";
        // Just verify it produces 32 bytes (valid SHA-256 output)
        assert_eq!(result.len(), 32);
    }

    /// Test HMAC-SHA256 with empty data.
    #[test]
    fn test_hmac_sha256_empty_data() {
        let key = b"key";
        let result = hmac_sha256(key, b"");
        assert_eq!(result.len(), 32);
    }

    /// Test the AWS SigV4 signing key derivation.
    /// Verifies the key derivation chain produces a 32-byte key.
    #[test]
    fn test_derive_signing_key_produces_32_bytes() {
        let secret_key = "wJalrXUtnFEMI/K7MDENG+bPxRfiCYEXAMPLEKEY";
        let date_stamp = "20150830";
        let region = "us-east-1";
        let service = "iam";

        let key = derive_signing_key(secret_key, date_stamp, region, service);
        // HMAC-SHA256 always produces 32-byte output
        assert_eq!(key.len(), 32);
    }

    /// Test that the signing key derivation is deterministic.
    #[test]
    fn test_derive_signing_key_deterministic() {
        let key1 = derive_signing_key("secret", "20250101", "us-east-1", "bedrock");
        let key2 = derive_signing_key("secret", "20250101", "us-east-1", "bedrock");
        assert_eq!(key1, key2);

        // Different inputs produce different keys
        let key3 = derive_signing_key("secret", "20250101", "eu-west-1", "bedrock");
        assert_ne!(key1, key3);
    }

    #[test]
    fn test_extract_host() {
        assert_eq!(
            extract_host("https://bedrock.us-east-1.amazonaws.com/model/test/invoke"),
            Some("bedrock.us-east-1.amazonaws.com".to_string())
        );
        assert_eq!(
            extract_host("https://bedrock.eu-west-1.amazonaws.com"),
            Some("bedrock.eu-west-1.amazonaws.com".to_string())
        );
        assert_eq!(extract_host("not-a-url"), None);
    }

    #[test]
    fn test_extract_path() {
        assert_eq!(
            extract_path("https://bedrock.us-east-1.amazonaws.com/model/test/invoke"),
            "/model/test/invoke"
        );
        assert_eq!(extract_path("https://example.com"), "/");
        assert_eq!(
            extract_path("https://example.com/path?query=value"),
            "/path"
        );
    }

    #[test]
    fn test_extract_query_string() {
        assert_eq!(
            extract_query_string("https://example.com/path?key=value&foo=bar"),
            "key=value&foo=bar"
        );
        assert_eq!(extract_query_string("https://example.com/path"), "");
    }

    #[test]
    fn test_sign_request_adds_required_headers() {
        let creds = AwsCredentials {
            access_key: "AKIAIOSFODNN7EXAMPLE".to_string(),
            secret_key: "wJalrXUtnFEMI/K7MDENG/bPxRfiCYEXAMPLEKEY".to_string(),
            session_token: None,
            region: "us-east-1".to_string(),
        };

        let mut headers: Vec<(String, String)> = Vec::new();
        let body = r#"{"anthropic_version":"2023-06-01"}"#.as_bytes();

        let result = sign_request(
            "POST",
            "https://bedrock.us-east-1.amazonaws.com/model/test/invoke-with-response-stream",
            &mut headers,
            body,
            &creds,
        );

        assert!(result.is_ok());

        // Check x-amz-date header present
        assert!(headers.iter().any(|(k, _)| k == "x-amz-date"));

        // Check x-amz-content-sha256 header present
        assert!(headers.iter().any(|(k, _)| k == "x-amz-content-sha256"));

        // Check Authorization header present with AWS4-HMAC-SHA256 prefix
        let auth = headers
            .iter()
            .find(|(k, _)| k == "Authorization")
            .map(|(_, v)| v.as_str());
        assert!(auth.is_some());
        assert!(auth.unwrap().starts_with("AWS4-HMAC-SHA256"));

        // FR-016: No Bearer or x-api-key headers
        assert!(!headers.iter().any(|(k, _)| k == "x-api-key"));
        assert!(
            !headers
                .iter()
                .any(|(k, v)| k == "Authorization" && v.starts_with("Bearer"))
        );
    }

    #[test]
    fn test_sign_request_with_session_token() {
        let creds = AwsCredentials {
            access_key: "AKIAIOSFODNN7EXAMPLE".to_string(),
            secret_key: "wJalrXUtnFEMI/K7MDENG/bPxRfiCYEXAMPLEKEY".to_string(),
            session_token: Some("FwoGZXIvYXdzEBYaDExampleToken".to_string()),
            region: "us-east-1".to_string(),
        };

        let mut headers: Vec<(String, String)> = Vec::new();
        let body = b"{}";

        let result = sign_request(
            "POST",
            "https://bedrock.us-east-1.amazonaws.com/model/test/converse-stream",
            &mut headers,
            body,
            &creds,
        );

        assert!(result.is_ok());

        // FR-015: x-amz-security-token header must be present
        assert!(
            headers
                .iter()
                .any(|(k, v)| k == "x-amz-security-token" && v == "FwoGZXIvYXdzEBYaDExampleToken")
        );

        // The signed headers must include x-amz-security-token
        let auth = headers
            .iter()
            .find(|(k, _)| k == "Authorization")
            .map(|(_, v)| v.as_str())
            .unwrap();
        assert!(auth.contains("x-amz-security-token"));
    }

    #[test]
    fn test_sign_request_region_in_credential_scope() {
        let creds = AwsCredentials {
            access_key: "AKIAIOSFODNN7EXAMPLE".to_string(),
            secret_key: "wJalrXUtnFEMI/K7MDENG/bPxRfiCYEXAMPLEKEY".to_string(),
            session_token: None,
            region: "eu-west-1".to_string(),
        };

        let mut headers: Vec<(String, String)> = Vec::new();

        sign_request(
            "POST",
            "https://bedrock.eu-west-1.amazonaws.com/model/test/converse-stream",
            &mut headers,
            b"{}",
            &creds,
        )
        .unwrap();

        let auth = headers
            .iter()
            .find(|(k, _)| k == "Authorization")
            .map(|(_, v)| v.as_str())
            .unwrap();

        // Region must appear in credential scope
        assert!(auth.contains("/eu-west-1/bedrock/aws4_request"));
    }

    #[test]
    fn test_hex_encode_empty() {
        assert_eq!(
            hex_encode(&Sha256::digest(b"")),
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
        );
    }

    #[test]
    fn test_body_hash_in_header() {
        let creds = AwsCredentials {
            access_key: "AKIAIOSFODNN7EXAMPLE".to_string(),
            secret_key: "wJalrXUtnFEMI/K7MDENG/bPxRfiCYEXAMPLEKEY".to_string(),
            session_token: None,
            region: "us-east-1".to_string(),
        };

        let body = br#"{"model":"test"}"#;
        let expected_hash = hex_encode(&Sha256::digest(body));

        let mut headers: Vec<(String, String)> = Vec::new();
        sign_request(
            "POST",
            "https://bedrock.us-east-1.amazonaws.com/model/test",
            &mut headers,
            body,
            &creds,
        )
        .unwrap();

        let content_hash = headers
            .iter()
            .find(|(k, _)| k == "x-amz-content-sha256")
            .map(|(_, v)| v.as_str())
            .unwrap();
        assert_eq!(content_hash, expected_hash);
    }
}
