//! ANTIPAT M0.8 regression tests: config secrets and the enforced denylist.
//!
//! - H-2: `Config` carried a derived `Debug`, so any `{:?}`/`tracing::debug!`
//!   printed plaintext API keys, tokens, and webhook URLs.
//! - H-3: `dir_lists::load_from_config` returned an empty `DirLists` when the
//!   config could not be loaded, silently dropping the mandatory built-in
//!   system-directory denylist (fail-open).

use ragent_config::Config;

/// A `{:?}` of a config holding secrets must not print them.
#[test]
fn test_config_debug_redacts_api_keys() {
    let mut config = Config::default();
    config.tavily_api_key = Some("tvly-abcdef0123456789".to_string());
    config.exa_api_key = Some("exa-abcdef0123456789".to_string());
    config.langsearch_api_key = Some("ls-abcdef0123456789".to_string());
    config.perplexity_api_key = Some("pplx-abcdef0123456789".to_string());
    config.serper_api_key = Some("s-abcdef0123456789".to_string());

    let rendered = format!("{config:?}");

    for secret in [
        "tvly-abcdef0123456789",
        "exa-abcdef0123456789",
        "ls-abcdef0123456789",
        "pplx-abcdef0123456789",
        "s-abcdef0123456789",
    ] {
        assert!(
            !rendered.contains(secret),
            "Debug output leaked {secret}: {rendered}"
        );
    }
    // The field names are still visible and the secrets are masked, so the
    // output stays useful for debugging without leaking credentials.
    assert!(
        rendered.contains("tavily_api_key") && rendered.contains("[REDACTED]"),
        "Debug output should show the field with a redaction marker: {rendered}"
    );
}

/// A config with no secrets still renders its non-secret fields.
#[test]
fn test_config_debug_keeps_non_secret_fields() {
    let mut config = Config::default();
    config.default_agent = "coder".to_string();

    let rendered = format!("{config:?}");

    assert!(
        rendered.contains("coder"),
        "Debug output must keep non-secret values: {rendered}"
    );
}

/// The built-in denylist is a non-empty floor that the merge must preserve.
///
/// H-3's regression risk is that a config-load failure returns a default
/// (empty) enforced denylist. `load_from_config` is environment-dependent, so
/// the invariant tested here is the constant itself plus the merge behaviour:
/// every built-in entry is present in a config that adds no denylist entries.
#[test]
fn test_builtin_denylist_is_non_empty_and_merged() {
    let builtins = ragent_config::dir_lists::BUILTIN_DENYLIST;
    assert!(
        !builtins.is_empty(),
        "the built-in denylist must never be empty: it is the enforced floor"
    );

    // A config with no `dirs.denylist` must still yield every built-in entry.
    let lists = ragent_config::dir_lists::merged_dir_lists_for(&Config::default());
    for pattern in builtins {
        assert!(
            lists.denylist.iter().any(|d| d == pattern),
            "built-in denylist entry {pattern} must always be enforced"
        );
    }
}
