use std::borrow::Cow;
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
/// - Generic long base64-like tokens following a `token` / `apikey` / `api_key` /
///   `secret` / `password` key. This group is case-insensitive and accepts an
///   optional quote and either `=` or `:`, so `API_KEY=…`, `"token": "…"` and
///   `token: …` all match (FUNC-007).
static SECRET_PATTERN: LazyLock<Regex> = LazyLock::new(|| {
    #[allow(clippy::expect_used)]
    Regex::new(concat!(
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
        // Generic token/apikey/secret/password assignments in URLs and configs.
        // Capture group 1 holds the key + separator so the replacement keeps the
        // key and only blanks the value (FUNC-007). Case-insensitive, with an
        // optional quote around the key and value and either `=` or `:` as the
        // separator, so `API_KEY=…`, `"token": "…"` and `token: …` all match.
        // The value charset includes `/`, `+`, `=` so base64/base64url/JWT
        // payloads redact fully instead of leaking the tail past the first
        // excluded byte. The 16-char value floor keeps innocuous prose out.
        r#"(?i:((?:api[_-]?key|token|secret|password)["']?\s*[:=]\s*["']?)[a-zA-Z0-9_\-\./+=]{16,})"#,
    ))
    .expect("valid regex pattern")
});

/// Global in-memory registry of known secret values.
///
/// Secrets registered here are redacted by exact substring match in
/// [`redact_secrets`], complementing the regex-based pattern matching.
/// The registry is seeded from the database on startup and updated
/// whenever provider credentials change.
///
/// PERF-055: the registry is kept sorted longest-first on insert so
/// [`redact_secrets_cow`] does not have to collect and sort it on every call.
static SECRET_REGISTRY: LazyLock<RwLock<Vec<String>>> = LazyLock::new(|| RwLock::new(Vec::new()));

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
    let mut registry = registry_write();
    register_secret_inner(&mut registry, secret);
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
    registry_write().retain(|s| s != secret);
}

/// Clears all secrets from the exact-match redaction registry.
pub fn clear_secret_registry() {
    registry_write().clear();
}

/// Seeds the secret registry with multiple values at once.
///
/// Useful at startup to bulk-load secrets from the database or
/// environment variables.
pub fn seed_secrets(secrets: impl IntoIterator<Item = String>) {
    let mut registry = registry_write();
    for s in secrets {
        if !s.is_empty() {
            registry.push(s);
        }
    }
    sort_by_len_desc(&mut registry);
}

/// Acquire a write guard on the registry, recovering from poison.
///
/// A panic while a writer held the lock poisons it; recovering the inner value
/// keeps the registry usable instead of turning every later write into a silent
/// no-op (FUNC-008).
fn registry_write() -> std::sync::RwLockWriteGuard<'static, Vec<String>> {
    SECRET_REGISTRY
        .write()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}

/// Acquire a read guard on the registry, recovering from poison.
///
/// Recovers the inner value on poison so exact-match redaction still consults
/// the registered secrets — failing closed instead of leaking them (FUNC-006).
fn registry_read() -> std::sync::RwLockReadGuard<'static, Vec<String>> {
    SECRET_REGISTRY
        .read()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}

/// Insert `secret` (if new) and re-sort longest-first.
fn register_secret_inner(registry: &mut Vec<String>, secret: &str) {
    if registry.iter().any(|s| s == secret) {
        return;
    }
    registry.push(secret.to_string());
    sort_by_len_desc(registry);
}

/// Sort secrets by descending length so a shorter secret that is a substring
/// of a longer one cannot partially replace it first.
fn sort_by_len_desc(registry: &mut [String]) {
    registry.sort_by(|a, b| b.len().cmp(&a.len()));
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
    redact_secrets_cow(msg).into_owned()
}

/// Cow-returning variant of [`redact_secrets`] (PERF-055).
///
/// Returns [`Cow::Borrowed`] without allocating when the message contains no
/// registered secret and does not match the secret regex — the common case for
/// streaming events. Callers on hot paths (e.g. SSE serialisation) should use
/// this and keep the borrowed slice rather than materialising a `String`.
///
/// # Examples
///
/// ```rust
/// use std::borrow::Cow;
/// use ragent_types::sanitize::redact_secrets_cow;
///
/// let clean = redact_secrets_cow("nothing sensitive here");
/// assert!(matches!(clean, Cow::Borrowed(_)));
/// ```
pub fn redact_secrets_cow(msg: &str) -> Cow<'_, str> {
    // Fail closed: a poisoned lock still holds intact data, so recover the
    // guard rather than treating the registry as empty (which would skip the
    // exact-match layer and leak registered secrets) (FUNC-006).
    let registry_empty = registry_read().is_empty();

    if registry_empty && !SECRET_PATTERN.is_match(msg) {
        return Cow::Borrowed(msg);
    }

    Cow::Owned(redact_secrets_owned(msg, registry_empty))
}

/// Apply both redaction layers, allocating the result.
fn redact_secrets_owned(msg: &str, registry_empty: bool) -> String {
    let mut result: Cow<'_, str> = Cow::Borrowed(msg);

    // Layer 1: exact-match registered secrets, already longest-first.
    if !registry_empty {
        let registry = registry_read();
        for secret in registry.iter() {
            if result.contains(secret.as_str()) {
                let replaced = match result {
                    Cow::Borrowed(text) => text.replace(secret.as_str(), "[REDACTED]"),
                    Cow::Owned(text) => text.replace(secret.as_str(), "[REDACTED]"),
                };
                result = Cow::Owned(replaced);
            }
        }
    }

    // Layer 2: regex pattern matching for common secret formats. The generic
    // token/apikey/secret/password arm captures the key + separator as group 1
    // and re-inserts it (`${1}`) so JSON/config structure survives redaction;
    // for every other arm group 1 is empty.
    SECRET_PATTERN
        .replace_all(&result, "${1}[REDACTED]")
        .into_owned()
}
