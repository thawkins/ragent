use std::sync::LazyLock;

use regex::Regex;

static SECRET_PATTERN: LazyLock<Regex> = LazyLock::new(|| {
    #[allow(clippy::expect_used)]
    Regex::new(r"(sk-[a-zA-Z0-9]{20,}|key-[a-zA-Z0-9]{20,}|Bearer [a-zA-Z0-9\-]{20,})")
        .expect("valid regex pattern")
});

/// Redacts sensitive data such as API keys, secret keys, and bearer tokens
/// from the given text, replacing each match with `[REDACTED]`.
///
/// The function recognises patterns of the form `sk-…`, `key-…`, and
/// `Bearer …` where the secret portion is at least 20 alphanumeric
/// characters long.
///
/// # Examples
///
/// ```rust
/// use ragent_core::sanitize::redact_secrets;
///
/// let input = "Authorization: Bearer abcdefghijklmnopqrstuvwxyz";
/// let cleaned = redact_secrets(input);
/// assert_eq!(cleaned, "Authorization: [REDACTED]");
/// assert!(!cleaned.contains("abcdefghijklmnopqrstuvwxyz"));
/// ```
pub fn redact_secrets(msg: &str) -> String {
    SECRET_PATTERN.replace_all(msg, "[REDACTED]").into_owned()
}
