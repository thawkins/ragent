//! Tests for test_sanitize.rs

use ragent_core::sanitize::{
    clear_secret_registry, redact_secrets, register_secret, seed_secrets, unregister_secret,
};

/// Build a fake token at runtime by repeating a character sequence.
/// The prefix and the generated suffix are never stored together as a literal,
/// so secret scanners cannot flag them in source.
fn fake_token(prefix: &str, len: usize) -> String {
    const CHARS: &[u8] = b"abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789";
    let suffix: String = (0..len)
        .map(|i| CHARS[i % CHARS.len()] as char)
        .collect();
    format!("{prefix}{suffix}")
}

/// Encode bytes as base64url (no padding) — used to build fake JWTs at runtime.
fn base64_url_encode(data: &[u8]) -> String {
    const TABLE: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_";
    let mut out = String::new();
    for chunk in data.chunks(3) {
        let b0 = chunk[0] as usize;
        let b1 = chunk.get(1).copied().unwrap_or(0) as usize;
        let b2 = chunk.get(2).copied().unwrap_or(0) as usize;
        out.push(TABLE[(b0 >> 2)] as char);
        out.push(TABLE[((b0 & 3) << 4) | (b1 >> 4)] as char);
        if chunk.len() > 1 { out.push(TABLE[((b1 & 0xf) << 2) | (b2 >> 6)] as char); }
        if chunk.len() > 2 { out.push(TABLE[b2 & 0x3f] as char); }
    }
    out
}


/// Build a fake numeric-suffixed token.
fn fake_numeric_token(prefix: &str, len: usize) -> String {
    let suffix: String = (0..len)
        .map(|i| (b'0' + (i % 10) as u8) as char)
        .collect();
    format!("{prefix}{suffix}")
}

// ── sk- keys ─────────────────────────────────────────────────────

#[test]
fn test_redact_sk_key() {
    let token = fake_token("sk-", 30);
    let input = format!("My API key is {token}");
    let result = redact_secrets(&input);
    assert!(result.contains("[REDACTED]"));
    assert!(!result.contains("sk-"));
}

#[test]
fn test_redact_long_sk_key() {
    let token = fake_token("sk-", 40);
    let input = format!("token: {token}");
    let result = redact_secrets(&input);
    assert!(result.contains("[REDACTED]"));
    assert!(!result.contains("sk-"));
}

// ── key- keys ────────────────────────────────────────────────────

#[test]
fn test_redact_key_prefix() {
    let token = fake_token("key-", 30);
    let input = format!("Using {token} for auth");
    let result = redact_secrets(&input);
    assert!(result.contains("[REDACTED]"));
    assert!(!result.contains("key-"));
}

// ── Bearer tokens ────────────────────────────────────────────────

#[test]
fn test_redact_bearer_token() {
    let token = fake_token("", 30);
    let input = format!("Authorization: Bearer {token}");
    let result = redact_secrets(&input);
    assert!(result.contains("[REDACTED]"));
    assert!(!result.contains("Bearer a"));
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
    let t1 = fake_token("sk-", 25);
    let t2 = fake_token("key-", 25);
    let input = format!("key1={t1} key2={t2}");
    let result = redact_secrets(&input);
    assert!(!result.contains("sk-"));
    assert!(!result.contains("key-a"));
    assert_eq!(result.matches("[REDACTED]").count(), 2);
}

// ── Secret at start and end of string ────────────────────────────

#[test]
fn test_redact_at_boundaries() {
    let t = fake_token("sk-", 30);
    let start = format!("{t} is my key");
    let result = redact_secrets(&start);
    assert!(result.starts_with("[REDACTED]"));

    let end = format!("my key is {t}");
    let result = redact_secrets(&end);
    assert!(result.ends_with("[REDACTED]"));
}

// ── Preserves surrounding text ───────────────────────────────────

#[test]
fn test_redact_preserves_context() {
    let token = fake_token("sk-", 30);
    let input = format!("Error: authentication failed with token {token}. Please retry.");
    let result = redact_secrets(&input);
    assert!(result.contains("Error: authentication failed with token"));
    assert!(result.contains(". Please retry."));
    assert!(result.contains("[REDACTED]"));
}

// ── JWT tokens ───────────────────────────────────────────────────

#[test]
fn test_redact_bearer_jwt() {
    // Build a fake JWT dynamically: base64url(header).base64url(payload).signature
    let header = base64_url_encode(b"{\"alg\":\"HS256\",\"typ\":\"JWT\"}");
    let payload = base64_url_encode(b"{\"sub\":\"1234567890\"}");
    let sig = fake_token("", 40);
    let input = format!("Authorization: Bearer {header}.{payload}.{sig}");
    let result = redact_secrets(&input);
    assert!(
        result.contains("[REDACTED]"),
        "JWT should be redacted: {result}"
    );
    assert!(!result.contains(&header));
}

// ── Tokens with underscores ──────────────────────────────────────

#[test]
fn test_redact_sk_with_underscores() {
    let token = fake_token("sk_live_", 30);
    let result = redact_secrets(&token);
    assert!(
        result.contains("[REDACTED]"),
        "sk_live_ key should be redacted: {result}"
    );
    assert!(!result.contains("sk_live_"));
}

#[test]
fn test_redact_sk_test_key() {
    let token = fake_token("sk_test_", 25);
    let input = format!("key: {token}");
    let result = redact_secrets(&input);
    assert!(
        result.contains("[REDACTED]"),
        "sk_test_ key should be redacted: {result}"
    );
}

// ── GitHub tokens ────────────────────────────────────────────────

#[test]
fn test_redact_github_personal_token() {
    let token = fake_token("ghp_", 30);
    let input = format!("GITHUB_TOKEN={token}");
    let result = redact_secrets(&input);
    assert!(
        result.contains("[REDACTED]"),
        "GitHub PAT should be redacted: {result}"
    );
    assert!(!result.contains("ghp_"));
}

#[test]
fn test_redact_github_oauth_token() {
    let token = fake_token("gho_", 30);
    let input = format!("token: {token}");
    let result = redact_secrets(&input);
    assert!(
        result.contains("[REDACTED]"),
        "GitHub OAuth token should be redacted: {result}"
    );
}

// ── Slack tokens ─────────────────────────────────────────────────

#[test]
fn test_redact_slack_bot_token() {
    let token = fake_token("xoxb-", 35);
    let input = format!("SLACK_TOKEN={token}");
    let result = redact_secrets(&input);
    assert!(
        result.contains("[REDACTED]"),
        "Slack bot token should be redacted: {result}"
    );
    assert!(!result.contains("xoxb-"));
}

// ── AWS access keys ──────────────────────────────────────────────

#[test]
fn test_redact_aws_access_key() {
    // AWS keys are uppercase alphanumeric after the AKIA prefix
    let suffix: String = (0..16).map(|i| (b'A' + (i % 26)) as char).collect();
    let token = format!("AKIA{suffix}");
    let input = format!("AWS_ACCESS_KEY_ID={token}");
    let result = redact_secrets(&input);
    assert!(
        result.contains("[REDACTED]"),
        "AWS key should be redacted: {result}"
    );
    assert!(!result.contains("AKIA"));
}

// ── Generic token= assignments ───────────────────────────────────

#[test]
fn test_redact_token_assignment() {
    let token = fake_token("", 32);
    let input = format!("https://api.example.com?token={token}");
    let result = redact_secrets(&input);
    assert!(
        result.contains("[REDACTED]"),
        "token= value should be redacted: {result}"
    );
}

#[test]
fn test_redact_apikey_assignment() {
    let token = fake_token("", 26);
    let input = format!("apikey={token}");
    let result = redact_secrets(&input);
    assert!(
        result.contains("[REDACTED]"),
        "apikey= value should be redacted: {result}"
    );
}

#[test]
fn test_redact_password_assignment() {
    let token = fake_token("", 20);
    let input = format!("config: password={token}");
    let result = redact_secrets(&input);
    assert!(
        result.contains("[REDACTED]"),
        "password= value should be redacted: {result}"
    );
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
    assert_eq!(
        result, "normal text",
        "Empty secret should not affect output"
    );
}

#[test]
fn test_registry_combined_with_regex() {
    let custom = "my-internal-service-token-value";
    register_secret(custom);
    let sk = fake_token("sk-", 30);
    let input = format!("custom={custom} standard={sk}");
    let result = redact_secrets(&input);
    assert!(!result.contains(custom), "Custom secret should be redacted");
    assert!(
        !result.contains("sk-"),
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
    let header = base64_url_encode(b"{\"alg\":\"RS256\"}");
    let payload = base64_url_encode(b"{\"iss\":\"example.com\"}");
    let sig = fake_token("", 40);
    let jwt = format!("{header}.{payload}.{sig}");
    let input = format!("Authorization: Bearer {jwt}");
    let result = redact_secrets(&input);
    assert!(
        result.contains("[REDACTED]"),
        "3-segment JWT should be redacted: {result}"
    );
    assert!(!result.contains(&header));
}

#[test]
fn test_redact_jwt_with_underscores_and_dashes() {
    let header = base64_url_encode(b"{\"alg\":\"HS256\"}");
    let payload = base64_url_encode(b"{\"sub\":\"1234567890\",\"admin\":true}");
    let sig = fake_token("", 43);
    let jwt = format!("{header}.{payload}.{sig}");
    let input = format!("Bearer {jwt}");
    let result = redact_secrets(&input);
    assert!(
        result.contains("[REDACTED]"),
        "JWT with mixed chars should be redacted: {result}"
    );
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
    assert_eq!(
        result, input,
        "Plain base64 without secret prefix should not be redacted"
    );
}

#[test]
fn test_no_redact_url_with_short_params() {
    let input = "https://api.example.com?page=5&limit=100";
    let result = redact_secrets(input);
    assert_eq!(
        result, input,
        "URL with short params should not be redacted"
    );
}

// ── Property-style edge cases: tokens with underscores ──────────

#[test]
fn test_redact_github_server_token() {
    let token = fake_token("ghs_", 35);
    let result = redact_secrets(&token);
    assert!(
        result.contains("[REDACTED]"),
        "ghs_ token should be redacted: {result}"
    );
}

#[test]
fn test_redact_github_user_token() {
    let token = fake_token("ghu_", 30);
    let result = redact_secrets(&token);
    assert!(
        result.contains("[REDACTED]"),
        "ghu_ token should be redacted: {result}"
    );
}

#[test]
fn test_redact_github_refresh_token() {
    let token = fake_token("ghr_", 30);
    let result = redact_secrets(&token);
    assert!(
        result.contains("[REDACTED]"),
        "ghr_ token should be redacted: {result}"
    );
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
    assert_eq!(
        result, input,
        "Code variable named 'token' should not be redacted"
    );
}

#[test]
fn test_no_redact_password_short_value() {
    // password= with a short value (< 16 chars)
    let input = "password=abc123";
    let result = redact_secrets(input);
    assert_eq!(
        result, input,
        "Short password= value should not be redacted"
    );
}

#[test]
fn test_redact_secret_assignment_with_equals() {
    let token = fake_token("", 26);
    let input = format!("secret={token}");
    let result = redact_secrets(&input);
    assert!(
        result.contains("[REDACTED]"),
        "secret= with long value should be redacted: {result}"
    );
}

#[test]
fn test_no_redact_api_key_short() {
    let input = "api_key=short";
    let result = redact_secrets(input);
    assert_eq!(
        result, input,
        "api_key= with short value should not be redacted"
    );
}

#[test]
fn test_redact_api_key_long() {
    let token = fake_token("", 26);
    let input = format!("api_key={token}");
    let result = redact_secrets(&input);
    assert!(
        result.contains("[REDACTED]"),
        "api_key= with long value should be redacted: {result}"
    );
}

// ── Multi-secret stress ─────────────────────────────────────────

#[test]
fn test_redact_many_mixed_secrets() {
    let sk = fake_token("sk-", 25);
    let jwt_h = base64_url_encode(b"{\"alg\":\"HS256\"}");
    let jwt_p = base64_url_encode(b"{\"sub\":\"123\"}");
    let jwt = format!("Bearer {jwt_h}.{jwt_p}.longSignatureValue");
    let ghp = fake_token("ghp_", 30);
    let akia_suffix: String = (0..16).map(|i| (b'A' + (i % 26)) as char).collect();
    let akia = format!("AKIA{akia_suffix}");
    let xoxb = fake_token("xoxb-", 35);
    let tok = fake_token("", 28);
    let input = format!("{sk} {jwt} {ghp} {akia} {xoxb} token={tok}");
    let result = redact_secrets(&input);
    assert!(!result.contains("sk-"), "sk- should be redacted");
    assert!(!result.contains("Bearer"), "Bearer should be redacted");
    assert!(!result.contains("ghp_"), "GitHub token should be redacted");
    assert!(!result.contains("AKIA"), "AWS key should be redacted");
    assert!(!result.contains("xoxb-"), "Slack token should be redacted");
    assert!(!result.contains("token="), "token= should be redacted");
    assert!(
        result.matches("[REDACTED]").count() >= 5,
        "Should have at least 5 redactions"
    );
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
    let token = fake_token("sk-", 30);
    let input = format!("认证: {token} 完了");
    let result = redact_secrets(&input);
    assert!(
        result.contains("[REDACTED]"),
        "Secret among unicode should be redacted: {result}"
    );
    assert!(
        result.contains("认证:"),
        "Surrounding unicode should be preserved"
    );
    assert!(
        result.contains("完了"),
        "Surrounding unicode should be preserved"
    );
}
