//! Tests for test_sanitize.rs

use ragent_core::sanitize::{
    clear_secret_registry, redact_secrets, register_secret, seed_secrets, unregister_secret,
};

// ── sk- keys ─────────────────────────────────────────────────────

#[test]
fn test_redact_sk_key() {
    let input = "My API key is sk-abcdefghijklmnopqrstuvwxyz1234";
    let result = redact_secrets(input);
    assert!(result.contains("[REDACTED]"));
    assert!(!result.contains("sk-abcdef"));
}

#[test]
fn test_redact_long_sk_key() {
    let input = "token: sk-1234567890abcdefghijklmnopqrstuvwxyzABCDEF";
    let result = redact_secrets(input);
    assert!(result.contains("[REDACTED]"));
    assert!(!result.contains("sk-1234"));
}

// ── key- keys ────────────────────────────────────────────────────

#[test]
fn test_redact_key_prefix() {
    let input = "Using key-abcdefghijklmnopqrstuvwxyz for auth";
    let result = redact_secrets(input);
    assert!(result.contains("[REDACTED]"));
    assert!(!result.contains("key-abcdef"));
}

// ── Bearer tokens ────────────────────────────────────────────────

#[test]
fn test_redact_bearer_token() {
    let input = "Authorization: Bearer abcdefghijklmnopqrstuvwxyz-1234";
    let result = redact_secrets(input);
    assert!(result.contains("[REDACTED]"));
    assert!(!result.contains("Bearer abcdef"));
}

// ── Non-secret text ──────────────────────────────────────────────

#[test]
fn test_no_redaction_for_normal_text() {
    let input = "Hello, this is a normal message with no secrets.";
    let result = redact_secrets(input);
    assert_eq!(result, input);
}

#[test]
fn test_no_redaction_for_short_sk() {
    // sk- followed by less than 20 chars should not be redacted
    let input = "sk-short";
    let result = redact_secrets(input);
    assert_eq!(result, input);
}

// ── Multiple secrets in one string ───────────────────────────────

#[test]
fn test_redact_multiple_secrets() {
    let input = "key1=sk-aaaaaaaaaabbbbbbbbbbcccccc key2=key-ddddddddddeeeeeeeeeefffff";
    let result = redact_secrets(input);
    assert!(!result.contains("sk-aaa"));
    assert!(!result.contains("key-ddd"));
    // Should have two [REDACTED] tokens
    assert_eq!(result.matches("[REDACTED]").count(), 2);
}

// ── Secret at start and end of string ────────────────────────────

#[test]
fn test_redact_at_boundaries() {
    let start = "sk-abcdefghijklmnopqrstuvwxyz is my key";
    let result = redact_secrets(start);
    assert!(result.starts_with("[REDACTED]"));

    let end = "my key is sk-abcdefghijklmnopqrstuvwxyz";
    let result = redact_secrets(end);
    assert!(result.ends_with("[REDACTED]"));
}

// ── Preserves surrounding text ───────────────────────────────────

#[test]
fn test_redact_preserves_context() {
    let input =
        "Error: authentication failed with token sk-abcdefghijklmnopqrstuvwxyz. Please retry.";
    let result = redact_secrets(input);
    assert!(result.contains("Error: authentication failed with token"));
    assert!(result.contains(". Please retry."));
    assert!(result.contains("[REDACTED]"));
}

// ── JWT tokens ───────────────────────────────────────────────────

#[test]
fn test_redact_bearer_jwt() {
    let input = "Authorization: Bearer FAKE_JWT_SIGNATURE_FOR_TESTS";
    let result = redact_secrets(input);
    assert!(result.contains("[REDACTED]"), "JWT should be redacted: {result}");
    assert!(!result.contains("eyJhbGci"));
}

// ── Tokens with underscores ──────────────────────────────────────

#[test]
fn test_redact_sk_with_underscores() {
    let input = "FAKE_SK_LIVE_TOKEN_FOR_TESTS";
    let result = redact_secrets(input);
    assert!(result.contains("[REDACTED]"), "sk_live_ key should be redacted: {result}");
    assert!(!result.contains("sk_live_"));
}

#[test]
fn test_redact_sk_test_key() {
    let input = "key: FAKE_SK_TEST_TOKEN_FOR_TESTS";
    let result = redact_secrets(input);
    assert!(result.contains("[REDACTED]"), "sk_test_ key should be redacted: {result}");
}

// ── GitHub tokens ────────────────────────────────────────────────

#[test]
fn test_redact_github_personal_token() {
    let input = "GITHUB_TOKEN=FAKE_GHP_TOKEN_FOR_TESTS";
    let result = redact_secrets(input);
    assert!(result.contains("[REDACTED]"), "GitHub PAT should be redacted: {result}");
    assert!(!result.contains("ghp_"));
}

#[test]
fn test_redact_github_oauth_token() {
    let input = "token: FAKE_GHO_TOKEN_FOR_TESTS";
    let result = redact_secrets(input);
    assert!(result.contains("[REDACTED]"), "GitHub OAuth token should be redacted: {result}");
}

// ── Slack tokens ─────────────────────────────────────────────────

#[test]
fn test_redact_slack_bot_token() {
    let input = "SLACK_TOKEN=FAKE_XOXB_TOKEN_FOR_TESTS";
    let result = redact_secrets(input);
    assert!(result.contains("[REDACTED]"), "Slack bot token should be redacted: {result}");
    assert!(!result.contains("xoxb-"));
}

// ── AWS access keys ──────────────────────────────────────────────

#[test]
fn test_redact_aws_access_key() {
    let input = "AWS_ACCESS_KEY_ID=FAKE_AKIA_TOKEN_FOR_TESTS";
    let result = redact_secrets(input);
    assert!(result.contains("[REDACTED]"), "AWS key should be redacted: {result}");
    assert!(!result.contains("AKIAIOSF"));
}

// ── Generic token= assignments ───────────────────────────────────

#[test]
fn test_redact_token_assignment() {
    let input = "https://api.example.com?token=a1b2c3d4e5f6g7h8i9j0k1l2m3n4o5p6";
    let result = redact_secrets(input);
    assert!(result.contains("[REDACTED]"), "token= value should be redacted: {result}");
    assert!(!result.contains("a1b2c3d4"));
}

#[test]
fn test_redact_apikey_assignment() {
    let input = "apikey=abcdefghijklmnopqrstuvwx";
    let result = redact_secrets(input);
    assert!(result.contains("[REDACTED]"), "apikey= value should be redacted: {result}");
}

#[test]
fn test_redact_password_assignment() {
    let input = "config: password=Super_Secret_P4ssw0rd_12345";
    let result = redact_secrets(input);
    assert!(result.contains("[REDACTED]"), "password= value should be redacted: {result}");
}

// ── No false positives ──────────────────────────────────────────

#[test]
fn test_no_redact_normal_code() {
    let input = "let token = get_token(); if token.is_empty() { bail!(\"no token\"); }";
    let result = redact_secrets(input);
    assert_eq!(result, input, "Normal code should not be redacted");
}

#[test]
fn test_no_redact_short_key_prefix() {
    let input = "key-abc";
    let result = redact_secrets(input);
    assert_eq!(result, input, "Short key- prefix should not be redacted");
}

#[test]
fn test_no_redact_short_bearer() {
    let input = "Bearer short";
    let result = redact_secrets(input);
    assert_eq!(result, input, "Short bearer value should not be redacted");
}

// ── Secret registry: exact-match redaction ───────────────────────

#[test]
fn test_registry_exact_match() {
    let secret = "my-custom-nonstandard-secret-12345";
    register_secret(secret);
    let result = redact_secrets(&format!("using {secret} in request"));
    assert!(
        result.contains("[REDACTED]"),
        "Registered secret should be redacted: {result}"
    );
    assert!(!result.contains(secret));
    // Cleanup
    unregister_secret(secret);
}

#[test]
fn test_registry_unregister() {
    let secret = "ephemeral-token-abc-xyz-67890";
    register_secret(secret);
    unregister_secret(secret);
    let result = redact_secrets(secret);
    // After unregister, should NOT be redacted (unless regex also matches)
    assert_eq!(result, secret, "Unregistered secret should not be redacted");
}

#[test]
fn test_registry_seed_multiple() {
    let secrets = vec![
        "seed-secret-aaaa-1111".to_string(),
        "seed-secret-bbbb-2222".to_string(),
    ];
    seed_secrets(secrets.clone());
    for s in &secrets {
        let result = redact_secrets(&format!("value: {s}"));
        assert!(
            result.contains("[REDACTED]"),
            "Seeded secret should be redacted: {result}"
        );
    }
    // Cleanup
    for s in &secrets {
        unregister_secret(s);
    }
}

#[test]
fn test_registry_empty_string_ignored() {
    register_secret("");
    let result = redact_secrets("normal text");
    assert_eq!(result, "normal text", "Empty secret should not affect output");
}

#[test]
fn test_registry_combined_with_regex() {
    // Register a custom secret AND have a regex-matchable secret
    let custom = "my-internal-service-token-value";
    register_secret(custom);
    let input = format!("custom={custom} standard=sk-abcdefghijklmnopqrstuvwxyz");
    let result = redact_secrets(&input);
    assert!(!result.contains(custom), "Custom secret should be redacted");
    assert!(
        !result.contains("sk-abcdef"),
        "Standard pattern should also be redacted"
    );
    assert_eq!(
        result.matches("[REDACTED]").count(),
        2,
        "Should have two redactions"
    );
    unregister_secret(custom);
}

#[test]
fn test_registry_clear_all() {
    register_secret("clear-test-secret-aaa");
    register_secret("clear-test-secret-bbb");
    clear_secret_registry();
    let result = redact_secrets("clear-test-secret-aaa clear-test-secret-bbb");
    assert_eq!(
        result, "clear-test-secret-aaa clear-test-secret-bbb",
        "Cleared registry should not redact"
    );
}

// ── Property-style edge cases: JWT variants ─────────────────────

#[test]
fn test_redact_jwt_three_segments() {
    // Standard JWT: header.payload.signature
    let jwt = "FAKE_JWT_RS256_FOR_TESTS";
    let input = format!("Authorization: Bearer {jwt}");
    let result = redact_secrets(&input);
    assert!(result.contains("[REDACTED]"), "3-segment JWT should be redacted: {result}");
    assert!(!result.contains("eyJhbGci"));
}

#[test]
fn test_redact_jwt_with_underscores_and_dashes() {
    let jwt = "FAKE_JWT_ADMIN_FOR_TESTS";
    let input = format!("Bearer {jwt}");
    let result = redact_secrets(&input);
    assert!(result.contains("[REDACTED]"), "JWT with mixed chars should be redacted: {result}");
}

#[test]
fn test_no_redact_bearer_short_token() {
    // Bearer followed by < 20 chars — should NOT be redacted
    let input = "Bearer abc123";
    let result = redact_secrets(input);
    assert_eq!(result, input, "Short bearer token should not be redacted");
}

// ── Property-style edge cases: base64-like strings ──────────────

#[test]
fn test_no_redact_plain_base64_string() {
    // Raw base64 that doesn't match any secret pattern
    let input = "data: aGVsbG8gd29ybGQ=";
    let result = redact_secrets(input);
    assert_eq!(result, input, "Plain base64 without secret prefix should not be redacted");
}

#[test]
fn test_no_redact_url_with_short_params() {
    let input = "https://api.example.com?page=5&limit=100";
    let result = redact_secrets(input);
    assert_eq!(result, input, "URL with short params should not be redacted");
}

// ── Property-style edge cases: tokens with underscores ──────────

#[test]
fn test_redact_github_server_token() {
    let input = "FAKE_GHS_TOKEN_FOR_TESTS";
    let result = redact_secrets(input);
    assert!(result.contains("[REDACTED]"), "ghs_ token should be redacted: {result}");
}

#[test]
fn test_redact_github_user_token() {
    let input = "FAKE_GHU_TOKEN_FOR_TESTS";
    let result = redact_secrets(input);
    assert!(result.contains("[REDACTED]"), "ghu_ token should be redacted: {result}");
}

#[test]
fn test_redact_github_refresh_token() {
    let input = "FAKE_GHR_TOKEN_FOR_TESTS";
    let result = redact_secrets(input);
    assert!(result.contains("[REDACTED]"), "ghr_ token should be redacted: {result}");
}

// ── Boundary / false-positive checks ────────────────────────────

#[test]
fn test_no_redact_word_skeleton() {
    // "skeleton" starts with "sk" but shouldn't match sk- pattern
    let input = "loading skeleton component";
    let result = redact_secrets(input);
    assert_eq!(result, input, "'skeleton' should not be redacted");
}

#[test]
fn test_no_redact_word_skip() {
    let input = "skip this test";
    let result = redact_secrets(input);
    assert_eq!(result, input, "'skip' should not be redacted");
}

#[test]
fn test_no_redact_token_in_code() {
    // 'token' as a variable name, not an assignment with long value
    let input = "let token = compute_hash(input);";
    let result = redact_secrets(input);
    assert_eq!(result, input, "Code variable named 'token' should not be redacted");
}

#[test]
fn test_no_redact_password_short_value() {
    // password= with a short value (< 16 chars)
    let input = "password=abc123";
    let result = redact_secrets(input);
    assert_eq!(result, input, "Short password= value should not be redacted");
}

#[test]
fn test_redact_secret_assignment_with_equals() {
    let input = "secret=a1b2c3d4e5f6g7h8i9j0abcd";
    let result = redact_secrets(input);
    assert!(result.contains("[REDACTED]"), "secret= with long value should be redacted: {result}");
}

#[test]
fn test_no_redact_api_key_short() {
    let input = "api_key=short";
    let result = redact_secrets(input);
    assert_eq!(result, input, "api_key= with short value should not be redacted");
}

#[test]
fn test_redact_api_key_long() {
    let input = "api_key=a1b2c3d4e5f6g7h8i9j0k1l2";
    let result = redact_secrets(input);
    assert!(result.contains("[REDACTED]"), "api_key= with long value should be redacted: {result}");
}

// ── Multi-secret stress ─────────────────────────────────────────

#[test]
fn test_redact_many_mixed_secrets() {
    let input = concat!(
        "sk-aaaaaaaaaaaaaaaaaaaaaaaaa ",
        "Bearer FAKE_JWT_PAYLOAD_FOR_TESTS_here ",
        "ghp_aBcDeFgHiJkLmNoPqRsTuVwXyZ ",
        "FAKE_AKIA_TOKEN_FOR_TESTS123 ",
        "FAKE_XOXB_TOKEN_FOR_TESTS ",
        "token=abcdefghijklmnop12345678"
    );
    let result = redact_secrets(input);
    assert!(!result.contains("sk-aaa"), "sk- should be redacted");
    assert!(!result.contains("eyJhbGci"), "JWT should be redacted");
    assert!(!result.contains("ghp_"), "GitHub token should be redacted");
    assert!(!result.contains("AKIAIOSF"), "AWS key should be redacted");
    assert!(!result.contains("xoxb-"), "Slack token should be redacted");
    assert!(!result.contains("abcdefghijklmnop12345678"), "token= should be redacted");
    assert!(result.matches("[REDACTED]").count() >= 5, "Should have at least 5 redactions");
}

// ── Registry longest-first ordering ─────────────────────────────

#[test]
fn test_registry_longest_first() {
    let short = "short-secret-aaa";
    let long = "short-secret-aaa-extended-version";
    register_secret(short);
    register_secret(long);
    let input = format!("value: {long}");
    let result = redact_secrets(&input);
    // Should replace the long one fully, not leave "-extended-version" behind
    assert!(
        !result.contains("extended"),
        "Longest-first replacement should replace whole long secret: {result}"
    );
    unregister_secret(short);
    unregister_secret(long);
}

// ── Unicode / binary safety ─────────────────────────────────────

#[test]
fn test_no_redact_unicode_text() {
    let input = "日本語テスト：スキルが正しく動作します 🚀";
    let result = redact_secrets(input);
    assert_eq!(result, input, "Unicode text should pass through unchanged");
}

#[test]
fn test_redact_secret_surrounded_by_unicode() {
    let input = "认证: sk-abcdefghijklmnopqrstuvwxyz1234 完了";
    let result = redact_secrets(input);
    assert!(result.contains("[REDACTED]"), "Secret among unicode should be redacted: {result}");
    assert!(result.contains("认证:"), "Surrounding unicode should be preserved");
    assert!(result.contains("完了"), "Surrounding unicode should be preserved");
}
