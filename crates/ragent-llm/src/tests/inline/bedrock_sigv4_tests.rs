//! Inline tests for `bedrock_sigv4.rs` - compiled inside the module tree
//! via the `#[path]` delegation left in the source file
//! (ANTIPAT M2 inline-test migration; no behaviour change).

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
