use std::collections::HashSet;
use std::sync::{LazyLock, RwLock};

use regex::Regex;

/// Matches common secret patterns:
///
/// - `sk-` / `sk_live_` / `sk_test_` prefixed keys (`OpenAI`, Stripe, etc.)
/// - `key-` prefixed keys
/// - `Bearer` tokens (including JWTs with dots)
/// - `ghp_` / `gho_` / `ghs_` / `ghu_` / `ghr_` GitHub tokens
/// - `xoxb-` / `xoxp-` Slack tokens
/// - `AKIA` AWS access key IDs
/// - Generic long base64-like tokens following `token=`, `apikey=`, `api_key=`,
///   `secret=`, or `password=`
static SECRET_PATTERN: LazyLock<Regex> = LazyLock::new(|| {
    #[allow(clippy::expect_used)]
    Regex::new(concat!(
        r"(",
        // OpenAI / Stripe sk- keys (may contain underscores, hyphens)
        r"sk[-_][a-zA-Z0-9_\-]{20,}",
        r"|",
        // key- prefixed keys
        r"key-[a-zA-Z0-9_\-]{20,}",
        r"|",
        // Bearer tokens including JWTs (contain dots, underscores, hyphens)
        r"Bearer\s+[a-zA-Z0-9_\-\.]{20,}",
        r"|",
        // GitHub personal / OAuth / server / user / refresh tokens
        r"gh[pousr]_[a-zA-Z0-9]{20,}",
        r"|",
        // Slack tokens
        r"xox[bp]-[a-zA-Z0-9\-]{20,}",
        r"|",
        // AWS access key IDs (start with AKIA)
        r"AKIA[A-Z0-9]{16,}",
        r"|",
        // Generic token/apikey/secret/password assignments in URLs or configs
        r"(?:token|apikey|api_key|secret|password)=[a-zA-Z0-9_\-\.]{16,}",
        r")",
    ))
    .expect("valid regex pattern")
});

/// Global in-memory registry of known secret values.
///
/// Secrets registered here are redacted by exact substring match in
/// [`redact_secrets`], complementing the regex-based pattern matching.
/// The registry is seeded from the database on startup and updated
/// whenever provider credentials change.
static SECRET_REGISTRY: LazyLock<RwLock<HashSet<String>>> =
    LazyLock::new(|| RwLock::new(HashSet::new()));

/// Registers a secret value for exact-match redaction.
///
/// Empty strings are ignored. The secret is stored in a global in-memory
/// registry and will be matched by [`redact_secrets`].
///
/// # Examples
///
/// ```rust
/// use ragent_types::sanitize::{register_secret, redact_secrets};
///
/// register_secret("my-custom-secret-value");
/// let cleaned = redact_secrets("token is my-custom-secret-value here");
/// assert!(!cleaned.contains("my-custom-secret-value"));
/// ```
pub fn register_secret(secret: &str) {
    if secret.is_empty() {
        return;
    }
    if let Ok(mut registry) = SECRET_REGISTRY.write() {
        registry.insert(secret.to_string());
    }
}

/// Removes a secret value from the exact-match redaction registry.
///
/// # Examples
///
/// ```rust
/// use ragent_types::sanitize::{register_secret, unregister_secret, redact_secrets};
///
/// register_secret("temp-secret");
/// unregister_secret("temp-secret");
/// let result = redact_secrets("temp-secret");
/// assert_eq!(result, "temp-secret");
/// ```
pub fn unregister_secret(secret: &str) {
    if let Ok(mut registry) = SECRET_REGISTRY.write() {
        registry.remove(secret);
    }
}

/// Clears all secrets from the exact-match redaction registry.
pub fn clear_secret_registry() {
    if let Ok(mut registry) = SECRET_REGISTRY.write() {
        registry.clear();
    }
}

/// Seeds the secret registry with multiple values at once.
///
/// Useful at startup to bulk-load secrets from the database or
/// environment variables.
pub fn seed_secrets(secrets: impl IntoIterator<Item = String>) {
    if let Ok(mut registry) = SECRET_REGISTRY.write() {
        for s in secrets {
            if !s.is_empty() {
                registry.insert(s);
            }
        }
    }
}

/// Redacts sensitive data such as API keys, secret keys, and bearer tokens
/// from the given text, replacing each match with `[REDACTED]`.
///
/// Applies two layers of redaction:
/// 1. **Exact match** — any secret registered via [`register_secret`] or
///    [`seed_secrets`] is replaced by substring match.
/// 2. **Regex match** — common patterns (`sk-…`, `Bearer …`, etc.) are
///    caught by a static regex.
///
/// # Examples
///
/// ```rust
/// use ragent_types::sanitize::redact_secrets;
///
/// let input = "Authorization: Bearer abcdefghijklmnopqrstuvwxyz";
/// let cleaned = redact_secrets(input);
/// assert_eq!(cleaned, "Authorization: [REDACTED]");
/// assert!(!cleaned.contains("abcdefghijklmnopqrstuvwxyz"));
/// ```
pub fn redact_secrets(msg: &str) -> String {
    let mut result = msg.to_string();

    // Layer 1: exact-match registered secrets (longest first to avoid
    // partial replacements when one secret is a substring of another).
    if let Ok(registry) = SECRET_REGISTRY.read()
        && !registry.is_empty()
    {
        let mut secrets: Vec<&str> = registry.iter().map(String::as_str).collect();
        secrets.sort_by(|a, b| b.len().cmp(&a.len()));
        for secret in secrets {
            if result.contains(secret) {
                result = result.replace(secret, "[REDACTED]");
            }
        }
    }

    // Layer 2: regex pattern matching for common secret formats.
    SECRET_PATTERN
        .replace_all(&result, "[REDACTED]")
        .into_owned()
}
