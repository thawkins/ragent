//! External tests for `tests` from `crates/ragent-telemetry/src/sensitive.rs`
//!
//! Relocated from the inline `#[cfg(test)]` module.

use ragent_telemetry::sensitive::{
    MAX_ATTR_VALUE_LEN, REDACTED, looks_sensitive, sanitize_attr_value,
};

// ── Safe values pass through ──────────────────────────────────────────

#[test]
fn test_safe_model_name_passes() {
    assert_eq!(
        sanitize_attr_value("claude-sonnet-4-20250514"),
        "claude-sonnet-4-20250514"
    );
}

#[test]
fn test_safe_ollama_model_with_colons_passes() {
    // Ollama models are "name:tag" — must NOT be flagged.
    assert_eq!(sanitize_attr_value("qwen3:1.7b"), "qwen3:1.7b");
    assert_eq!(sanitize_attr_value("llama3.2:latest"), "llama3.2:latest");
}

#[test]
fn test_safe_provider_name_passes() {
    assert_eq!(sanitize_attr_value("anthropic"), "anthropic");
    assert_eq!(sanitize_attr_value("ollama"), "ollama");
    assert_eq!(sanitize_attr_value("generic_openai"), "generic_openai");
}

#[test]
fn test_safe_tool_name_passes() {
    assert_eq!(sanitize_attr_value("bash"), "bash");
    assert_eq!(sanitize_attr_value("read"), "read");
    assert_eq!(sanitize_attr_value("codeindex_search"), "codeindex_search");
}

#[test]
fn test_safe_session_id_passes() {
    // Session ids are typically UUIDs or short hex strings.
    assert_eq!(
        sanitize_attr_value("550e8400-e29b-41d4-a716-446655440000"),
        "550e8400-e29b-41d4-a716-446655440000"
    );
}

#[test]
fn test_safe_component_name_passes() {
    assert_eq!(sanitize_attr_value("tool"), "tool");
    assert_eq!(sanitize_attr_value("llm"), "llm");
    assert_eq!(sanitize_attr_value("coordinator"), "coordinator");
}

#[test]
fn test_empty_string_passes() {
    assert_eq!(sanitize_attr_value(""), "");
}

// ── API keys are redacted ─────────────────────────────────────────────

#[test]
fn test_openai_sk_prefix_redacted() {
    assert_eq!(sanitize_attr_value("sk-proj-abc123def456ghi789"), REDACTED);
    assert_eq!(sanitize_attr_value("sk-abc123"), REDACTED);
    assert_eq!(sanitize_attr_value("sk_abc123"), REDACTED);
}

#[test]
fn test_bearer_token_redacted() {
    assert_eq!(sanitize_attr_value("Bearer abc123def456"), REDACTED);
    assert_eq!(sanitize_attr_value("bearer abc"), REDACTED);
}

#[test]
fn test_github_tokens_redacted() {
    assert_eq!(sanitize_attr_value("ghp_abc123def456ghi789"), REDACTED);
    assert_eq!(sanitize_attr_value("gho_abc123def456ghi789"), REDACTED);
    assert_eq!(sanitize_attr_value("ghs_abc123def456ghi789"), REDACTED);
    assert_eq!(sanitize_attr_value("ghr_abc123def456ghi789"), REDACTED);
    assert_eq!(sanitize_attr_value("github_pat_abc123def456"), REDACTED);
}

#[test]
fn test_slack_tokens_redacted() {
    assert_eq!(sanitize_attr_value("xoxb-abc123def456-ghi789"), REDACTED);
    assert_eq!(sanitize_attr_value("xoxp-abc123def456"), REDACTED);
    assert_eq!(sanitize_attr_value("xoxs-abc123def456"), REDACTED);
}

#[test]
fn test_aws_access_key_redacted() {
    assert_eq!(sanitize_attr_value("AKIAIOSFODNN7EXAMPLE"), REDACTED);
    assert_eq!(sanitize_attr_value("AKIA1234567890ABCDEF"), REDACTED);
}

#[test]
fn test_google_api_key_redacted() {
    assert_eq!(
        sanitize_attr_value("AIzaSyDabc123def456ghi789jkl"),
        REDACTED
    );
}

#[test]
fn test_google_oauth_token_redacted() {
    assert_eq!(sanitize_attr_value("ya29.abc123def456ghi789"), REDACTED);
}

#[test]
fn test_jwt_prefix_redacted() {
    assert_eq!(sanitize_attr_value("jwt abc123.def456.ghi789"), REDACTED);
}

// ── "key:secret" shape ────────────────────────────────────────────────

#[test]
fn test_username_password_shape_redacted() {
    assert_eq!(sanitize_attr_value("user:secretpassword123"), REDACTED);
    assert_eq!(sanitize_attr_value("admin:password1234"), REDACTED);
}

#[test]
fn test_key_secret_shape_redacted() {
    assert_eq!(sanitize_attr_value("keyid:secretvalue1234"), REDACTED);
}

#[test]
fn test_short_parts_not_flagged() {
    // Both parts < 4 chars → not flagged (too short to be a credential).
    assert_eq!(sanitize_attr_value("a:bcd"), "a:bcd");
    assert_eq!(sanitize_attr_value("ab:cd"), "ab:cd");
}

#[test]
fn test_dots_in_parts_not_flagged() {
    // Dots indicate a version-like value, not a credential.
    assert_eq!(sanitize_attr_value("1.7b:latest"), "1.7b:latest");
}

// ── Whitespace / file content ─────────────────────────────────────────

#[test]
fn test_newline_redacted() {
    assert_eq!(sanitize_attr_value("line1\nline2"), REDACTED);
    assert_eq!(sanitize_attr_value("hello\r\nworld"), REDACTED);
}

#[test]
fn test_tab_redacted() {
    assert_eq!(sanitize_attr_value("col1\tcol2"), REDACTED);
}

#[test]
fn test_spaces_only_not_redacted() {
    // Plain spaces are fine (e.g. "Model Router").
    assert_eq!(sanitize_attr_value("Model Router"), "Model Router");
}

// ── Length guard ────────────────────────────────────────────────────────

#[test]
fn test_long_value_redacted() {
    let long = "a".repeat(MAX_ATTR_VALUE_LEN + 1);
    assert_eq!(sanitize_attr_value(&long), REDACTED);
}

#[test]
fn test_max_length_value_passes() {
    let max = "a".repeat(MAX_ATTR_VALUE_LEN);
    assert_eq!(sanitize_attr_value(&max), max);
}

// ── Inline token=... shape ─────────────��───────────────────────────────

#[test]
fn test_inline_token_redacted() {
    assert_eq!(
        sanitize_attr_value("token=abc123def456ghi789jkl012"),
        REDACTED
    );
    assert_eq!(
        sanitize_attr_value("key=abcdefghijklmnopqrstuvwx"),
        REDACTED
    );
}

#[test]
fn test_short_inline_value_not_redacted() {
    // < 20 base64 chars after '=' → not flagged.
    assert_eq!(sanitize_attr_value("a=short"), "a=short");
}

// ── File content / prompt text ─────────────────────────────────────────

#[test]
fn test_file_content_redacted() {
    let content = "use std::collections::HashMap;\nfn main() { println!(\"hi\"); }";
    assert_eq!(sanitize_attr_value(content), REDACTED);
}

#[test]
fn test_prompt_text_redacted() {
    let prompt = "Please explain how lifetimes work in Rust and give examples.";
    // Single line, < 256 chars, no prefix, no colon → passes (it's a
    // legitimate-looking string). This is a limitation of the deny-list
    // approach: a short single-line prompt with no sensitive shape is
    // indistinguishable from a long tool name. The cardinality cap
    // (FR-035) is the second layer of defence here.
    assert_eq!(sanitize_attr_value(prompt), prompt);
}

#[test]
fn test_multiline_prompt_redacted() {
    let prompt = "Line one of the prompt.\nLine two of the prompt.\nLine three.";
    assert_eq!(sanitize_attr_value(prompt), REDACTED);
}

// ── looks_sensitive predicate ─────────────────────────────────────────

#[test]
fn test_looks_sensitive_true_for_keys() {
    assert!(looks_sensitive("sk-abc123"));
    assert!(looks_sensitive("Bearer abc"));
    assert!(looks_sensitive("ghp_abc123def456"));
}

#[test]
fn test_looks_sensitive_false_for_safe() {
    assert!(!looks_sensitive("claude-sonnet-4-20250514"));
    assert!(!looks_sensitive("ollama"));
    assert!(!looks_sensitive("qwen3:1.7b"));
    assert!(!looks_sensitive("bash"));
}
