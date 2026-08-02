//! Sensitive-data guard for metric attributes and resource attributes (FR-034).
//!
//! FR-034: "The system shall not record sensitive data (API keys, file
//! contents, user prompts) as metric attributes or resource attributes."
//!
//! This module provides [`sanitize_attr_value`], a defensive filter that
//! inspects a candidate attribute value string and replaces it with the
//! [`REDACTED`] sentinel when it matches a known sensitive pattern. The
//! guard is applied at the point where [`InstrumentRegistry`](crate::instruments::InstrumentRegistry)
//! builds attribute key/value pairs (the `attr_*` helpers) and where the
//! [`CardinalityCache`](crate::cardinality::CardinalityCache) resolves
//! attribute sets, so that a caller that accidentally passes an API key,
//! a file path containing secrets, or a chunk of file/prompt content can
//! never leak that data into an exported metric.
//!
//! # What counts as "sensitive"
//!
//! The guard uses a conservative deny-list rather than attempting to
//! understand the semantics of every field. A value is redacted if **any**
//! of the following are true:
//!
//! - It looks like a common API-key prefix (`sk-`, `sk_`, `Bearer `, `ghp_`,
//!   `gho_`, `github_pat_`, `xoxb-`, `AKIA`, `AIza`).
//! - It contains a `:` separator between two long alphanumeric runs (the
//!   classic `username:password` or `key:secret` shape).
//! - It contains a newline, tab, or carriage return — legitimate model,
//!   provider, tool, and session identifiers never contain whitespace
//!   beyond simple spaces; multi-line values indicate file content or a
//!   pasted prompt.
//! - It exceeds 256 characters — legitimate low-cardinality identifiers
//!   are short; a very long value suggests a pasted blob.
//! - It contains an `=` followed by a long base64-like run (a common
//!   shape for inline tokens / credentials).
//!
//! The guard is intentionally over-cautious: false positives (redacting a
//! genuinely long model name) only collapse the value into a single
//! `redacted` bucket, which is the same behaviour already used by the
//! cardinality cap (FR-035) for overflow values.

/// Sentinel value substituted in place of a sensitive attribute value (FR-034).
///
/// Distinct from [`crate::cardinality::UNKNOWN_BUCKET`] (`"unknown"`) so that
/// logs and exported metrics can distinguish "cardinality overflow" from
/// "sensitive data was redacted".
pub const REDACTED: &str = "redacted";

/// Maximum number of characters in a legitimate attribute value before the
/// guard treats it as suspicious (FR-034).
///
/// Model names, provider ids, tool names, and session ids are all short
/// identifiers. A value longer than this is almost certainly a pasted blob
/// (file content, prompt text, a long token).
pub const MAX_ATTR_VALUE_LEN: usize = 256;

/// Returns `true` when `value` matches a known sensitive pattern (FR-034).
///
/// This is the core predicate used by [`sanitize_attr_value`]. It is kept
/// separate so that tests can assert on individual patterns.
///
/// # Arguments
///
/// * `value` — The candidate attribute value to inspect.
///
/// # Examples
///
/// ```
/// use ragent_telemetry::sensitive::looks_sensitive;
///
/// assert!(looks_sensitive("sk-proj-abc123"));
/// assert!(looks_sensitive("Bearer abc123"));
/// assert!(looks_sensitive("user:secretpassword"));
/// assert!(looks_sensitive("line1\nline2"));
/// assert!(!looks_sensitive("claude-sonnet-4-20250514"));
/// ```
#[must_use]
pub fn looks_sensitive(value: &str) -> bool {
    // ── Length guard ──────────────────────────────────────────────────────
    // Legitimate identifiers are short; a very long value is almost
    // certainly a pasted blob (file content, prompt, long token).
    if value.len() > MAX_ATTR_VALUE_LEN {
        return true;
    }

    // ── Whitespace guard ──────────────────────────────────────────────────
    // Newlines, tabs, and carriage returns never appear in legitimate
    // model/provider/tool/session identifiers. Their presence indicates
    // multi-line content (file contents, a pasted prompt).
    if value.contains('\n') || value.contains('\r') || value.contains('\t') {
        return true;
    }

    // ── Known API-key prefixes ────────────────────────────────────────────
    // These are the common prefixes used by the providers ragent supports
    // and by the major cloud platforms. Matching is case-insensitive for
    // the alphabetic prefixes (Bearer, AKIA, AIza, xoxb) because real
    // tokens are case-mixed but the prefix is consistently cased.
    let lower = value.to_ascii_lowercase();
    const PREFIXES: &[&str] = &[
        "sk-",         // OpenAI
        "sk_",         // OpenAI (alternate)
        "bearer ",     // HTTP Authorization header
        "ghp_",        // GitHub personal access token
        "gho_",        // GitHub OAuth token
        "ghs_",        // GitHub server-to-server token
        "ghr_",        // GitHub refresh token
        "github_pat_", // GitHub fine-grained PAT
        "xoxb-",       // Slack bot token
        "xoxp-",       // Slack user token
        "xoxs-",       // Slack signature secret
        "akia",        // AWS access key id (18 chars, starts with AKIA)
        "aiza",        // Google API key (starts with AIza)
        "ya29.",       // Google OAuth2 access token
        "jwt ",        // JWT bearer prefix
    ];
    for prefix in PREFIXES {
        if lower.starts_with(prefix) {
            return true;
        }
    }

    // ── "key:secret" shape ────────────────────────────────────────────────
    // A single colon separating two runs of >=4 alphanumeric chars is the
    // classic credential shape (username:password, key:secret). Legitimate
    // model names can contain colons (e.g. "ollama:qwen3:1.7b") so we only
    // flag it when there are exactly two colon-separated parts and both
    // parts are "dense" (no spaces, no dots, at least 4 chars). This avoids
    // false positives on Ollama model identifiers.
    if let Some(idx) = value.find(':') {
        let rest = &value[idx + 1..];
        // Only flag if there is no further colon (two parts total).
        if !rest.contains(':') {
            let left = &value[..idx];
            let right = rest;
            // Both parts must be at least 4 chars and contain no spaces or
            // dots (dots are common in model versions like "1.7b").
            if left.len() >= 4
                && right.len() >= 4
                && !left.contains(' ')
                && !right.contains(' ')
                && !left.contains('.')
                && !right.contains('.')
            {
                return true;
            }
        }
    }

    // ── Inline "token=..." shape ───────────────────────────────────────────
    // An `=` followed by a run of >=20 base64-like characters is a common
    // shape for inline credentials. Legitimate identifiers never contain
    // `=` at all (OTEL attribute keys/values in ragent are simple strings).
    if let Some(idx) = value.find('=') {
        let after = &value[idx + 1..];
        // Count the base64-like run length immediately after '='.
        let run_len = after
            .chars()
            .take_while(|c| c.is_ascii_alphanumeric() || matches!(c, '+' | '/' | '_' | '-'))
            .count();
        if run_len >= 20 {
            return true;
        }
    }

    false
}

/// Sanitize a candidate attribute value, replacing it with [`REDACTED`] if
/// it matches a sensitive pattern (FR-034).
///
/// This is the primary entry point used by the `attr_*` helpers in
/// [`InstrumentRegistry`](crate::instruments::InstrumentRegistry) and by
/// the resource-attribute builder in
/// [`TelemetrySubsystem`](crate::subsystem::TelemetrySubsystem). It never
/// panics and never blocks — it is a pure string inspection.
///
/// # Arguments
///
/// * `value` — The candidate attribute value.
///
/// # Returns
///
/// The original `value` (as an owned `String`) if it is safe, or
/// [`REDACTED`] if it matched a sensitive pattern.
///
/// # Examples
///
/// ```
/// use ragent_telemetry::sensitive::{sanitize_attr_value, REDACTED};
///
/// // Safe values pass through unchanged.
/// assert_eq!(sanitize_attr_value("claude-sonnet-4-20250514"), "claude-sonnet-4-20250514");
/// assert_eq!(sanitize_attr_value("ollama"), "ollama");
/// assert_eq!(sanitize_attr_value("bash"), "bash");
///
/// // Sensitive values are redacted.
/// assert_eq!(sanitize_attr_value("sk-proj-abc123def456"), REDACTED);
/// assert_eq!(sanitize_attr_value("Bearer abc123"), REDACTED);
/// ```
#[must_use]
pub fn sanitize_attr_value(value: &str) -> String {
    if looks_sensitive(value) {
        REDACTED.to_string()
    } else {
        value.to_string()
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────
