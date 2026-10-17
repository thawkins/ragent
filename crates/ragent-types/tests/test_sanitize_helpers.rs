//! Coverage for the remaining public redaction helpers in
//! [`ragent_types::sanitize`] that the existing redaction tests do not reach:
//! `is_secret_json_key`, `mask_key`, `redact_json_value`, `mask_edges`, and the
//! empty-secret registry edges.

use ragent_types::sanitize::{
    REDACTION_PLACEHOLDER, is_secret_json_key, mask_edges, mask_key, redact_json_value,
    register_secret, seed_secrets, unregister_secret,
};
use serde_json::json;

#[test]
fn exact_secret_keys_are_case_insensitive() {
    for key in [
        "token",
        "secret",
        "password",
        "api_key",
        "apikey",
        "webhook_url",
        "client_id",
        "client_secret",
        "access_token",
        "refresh_token",
        "bot_token",
        "private_key",
        "credentials",
        "openalex_email",
        "gitlab",
    ] {
        assert!(is_secret_json_key(key), "{key} must be secret");
        assert!(
            is_secret_json_key(&key.to_uppercase()),
            "uppercase {key} must be secret"
        );
    }
}

#[test]
fn secret_suffixes_are_matched_but_not_innocent_words() {
    for key in [
        "foo_key",
        "foo_token",
        "foo_secret",
        "foo_password",
        "API_KEY",
        "my_refresh_token",
    ] {
        assert!(is_secret_json_key(key), "{key} must be secret");
    }
    // The suffix form is deliberately conservative: `keywords` and
    // `token_budget` are not caught.
    assert!(!is_secret_json_key("keywords"));
    assert!(!is_secret_json_key("token_budget"));
    assert!(!is_secret_json_key("monkey"));
    assert!(!is_secret_json_key(""));
    assert!(!is_secret_json_key("name"));
}

#[test]
fn mask_key_keeps_only_the_last_four_characters() {
    assert_eq!(mask_key("sk-1234567890abcd"), "...abcd");
    assert_eq!(mask_key("abc"), "...abc");
    assert_eq!(mask_key("abcd"), "...abcd");
    assert_eq!(mask_key(""), "(none)");
    // Multi-byte: the last four characters are kept whole.
    assert_eq!(
        mask_key("\u{e9}\u{e9}\u{e9}\u{e9}\u{e9}"),
        "...\u{e9}\u{e9}\u{e9}\u{e9}"
    );
}

#[test]
fn redact_json_value_walks_nested_arrays_and_objects() {
    let secret = "sk-live-REDACTJSON0123456789";
    register_secret(secret);
    let mut value = json!({
        "outer": {
            "nested": [ "clean", secret ],
            "headers": { "Authorization": format!("Bearer {secret}") }
        },
        "count": 3,
        "flag": true
    });
    redact_json_value(&mut value, 8);
    let rendered = serde_json::to_string(&value).expect("serialize");
    assert!(
        !rendered.contains(secret),
        "the registered secret must not survive redaction: {rendered}"
    );
    // Non-string leaves are left untouched.
    assert_eq!(value["count"], json!(3));
    assert_eq!(value["flag"], json!(true));
    assert_eq!(value["outer"]["nested"][0], json!("clean"));
    // The registered secret inside the header string is replaced, leaving the
    // surrounding "Bearer " scheme prefix intact.
    let auth = value["outer"]["headers"]["Authorization"]
        .as_str()
        .expect("Authorization is a string");
    assert!(auth.contains(REDACTION_PLACEHOLDER), "got: {auth}");
    unregister_secret(secret);
}

#[test]
fn redact_json_value_stops_below_the_depth_limit() {
    let secret = "sk-live-DEPTHLIMIT012345678";
    register_secret(secret);
    // depth 0 is the root object; its `inner` value is at depth 1, and the
    // string is at depth 2. A max_depth of 1 leaves the deep string untouched.
    let mut value = json!({ "inner": { "deep": secret } });
    redact_json_value(&mut value, 1);
    assert_eq!(value["inner"]["deep"], json!(secret));

    // A generous depth redacts it.
    let mut value = json!({ "inner": { "deep": secret } });
    redact_json_value(&mut value, 8);
    assert_eq!(value["inner"]["deep"], json!(REDACTION_PLACEHOLDER));
    unregister_secret(secret);
}

#[test]
fn mask_edges_keeps_a_consistent_shape_for_each_band() {
    // Empty input is empty.
    assert_eq!(mask_edges("", 4, 12), "");
    // Length <= threshold masks fully.
    assert_eq!(mask_edges("abc", 4, 12), "***");
    assert_eq!(mask_edges("123456789012", 4, 12), "************");
    // Above threshold keeps both edges.
    assert_eq!(mask_edges("user@example.com", 2, 6), "us************om");
    assert_eq!(mask_edges("abcdefghijklmnop", 4, 8), "abcd********mnop");
}

#[test]
fn empty_secrets_are_ignored_by_register_and_seed() {
    // Registering the empty string is a no-op (and must not panic).
    register_secret("");
    seed_secrets(vec![String::new(), "seeded-value".to_string()]);
    // The seeded value is redacted after seeding; the empty one never matches
    // anything meaningful.
    let cleaned = ragent_types::sanitize::redact_secrets("contains seeded-value here");
    assert!(!cleaned.contains("seeded-value"));
    unregister_secret("seeded-value");
}
