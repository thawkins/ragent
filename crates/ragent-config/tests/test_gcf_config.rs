//! Integration tests for the `gcf` config block (spec `gcf`, FR-001, FR-008).
//!
//! These tests verify:
//! - `GcfConfig` defaults to disabled (FR-001)
//! - The `gcf` block is omitted from serialised output while disabled
//! - The `gcf` block parses when present (`{"enabled": true}`)
//! - An empty `gcf` block parses to the default
//! - Serde round-trip preserves the enabled state
//! - `Config::merge` uses last-wins semantics for the `gcf` block
//! - Malformed `gcf` values produce clear parse errors (FR-008)

use ragent_config::{Config, GcfConfig};

// ---------------------------------------------------------------------------
// Defaults (FR-001)
// ---------------------------------------------------------------------------

#[test]
fn default_gcf_config_is_disabled() {
    let cfg = GcfConfig::default();
    assert!(!cfg.enabled, "GCF encoding must default to off (FR-001)");
    assert!(
        cfg.is_default(),
        "default GcfConfig should report is_default() true"
    );
}

#[test]
fn top_level_config_defaults_gcf_when_absent() {
    let config: Config = serde_json::from_str("{}").expect("parse");
    assert_eq!(
        config.gcf,
        GcfConfig::default(),
        "absent gcf block -> default"
    );
    assert!(
        !config.gcf.enabled,
        "absent gcf block means GCF off (FR-001)"
    );
}

#[test]
fn serialised_default_config_omits_gcf_block() {
    // skip_serializing_if = "GcfConfig::is_default": a default config must not
    // carry a `gcf` key, so existing config files keep their intent.
    let config = Config::default();
    let json = serde_json::to_string_pretty(&config).unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
    assert!(
        parsed.get("gcf").is_none(),
        "default config should omit the gcf block, but found: {:?}",
        parsed.get("gcf")
    );
}

// ---------------------------------------------------------------------------
// Parsing when present
// ---------------------------------------------------------------------------

#[test]
fn top_level_config_parses_enabled_gcf_block() {
    let json = r#"{"gcf": {"enabled": true}}"#;
    let config: Config = serde_json::from_str(json).expect("parse");
    assert!(config.gcf.enabled, "gcf.enabled=true must parse");
}

#[test]
fn top_level_config_parses_explicitly_disabled_gcf_block() {
    let json = r#"{"gcf": {"enabled": false}}"#;
    let config: Config = serde_json::from_str(json).expect("parse");
    assert!(!config.gcf.enabled);
}

#[test]
fn gcf_config_deserializes_empty_block() {
    let config: Config = serde_json::from_str(r#"{"gcf": {}}"#).expect("parse");
    assert!(
        config.gcf.is_default(),
        "empty gcf block should parse to the default"
    );
    assert_eq!(config.gcf, GcfConfig::default());
}

// ---------------------------------------------------------------------------
// Round-trip (FR-008)
// ---------------------------------------------------------------------------

#[test]
fn gcf_config_roundtrip_preserves_enabled() {
    let original = GcfConfig { enabled: true };
    let json = serde_json::to_string(&original).unwrap();
    assert!(
        json.contains("enabled"),
        "serialised GcfConfig carries enabled"
    );
    let restored: GcfConfig = serde_json::from_str(&json).unwrap();
    assert_eq!(restored, original);
    assert!(restored.enabled);
}

#[test]
fn full_config_roundtrip_preserves_enabled_gcf() {
    let mut config = Config::default();
    config.gcf.enabled = true;

    let json = serde_json::to_string_pretty(&config).unwrap();
    let restored: Config = serde_json::from_str(&json).unwrap();
    assert!(restored.gcf.enabled, "round-trip must preserve gcf.enabled");

    // The serialised JSON carries the gcf key with enabled true.
    let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
    assert_eq!(parsed["gcf"]["enabled"], serde_json::Value::Bool(true));
}

#[test]
fn full_config_roundtrip_omits_disabled_gcf() {
    let config = Config::default(); // gcf disabled
    let json = serde_json::to_string_pretty(&config).unwrap();
    let restored: Config = serde_json::from_str(&json).unwrap();
    assert_eq!(restored.gcf, GcfConfig::default());
    assert!(!restored.gcf.enabled);
}

// ---------------------------------------------------------------------------
// Merge semantics (last-wins, mirrors compaction)
// ---------------------------------------------------------------------------

#[test]
fn config_merge_gcf_overlay_wins_over_base() {
    let mut base = Config::default();
    base.gcf.enabled = false;
    let mut overlay = Config::default();
    overlay.gcf.enabled = true;

    let merged = Config::merge(base, overlay);
    assert!(
        merged.gcf.enabled,
        "overlay enabling GCF must win (last-wins)"
    );
}

#[test]
fn config_merge_gcf_overlay_off_respected() {
    // /gcf off in a higher-precedence config must win over a base that
    // enabled GCF (NOT OR semantics).
    let mut base = Config::default();
    base.gcf.enabled = true;
    let mut overlay = Config::default();
    overlay.gcf.enabled = false;

    let merged = Config::merge(base, overlay);
    assert!(
        !merged.gcf.enabled,
        "overlay disabling GCF must win (last-wins)"
    );
}

// ---------------------------------------------------------------------------
// Malformed values (FR-008)
// ---------------------------------------------------------------------------

#[test]
fn malformed_gcf_enabled_type_is_rejected() {
    // `enabled` must be a boolean; a string is a type mismatch.
    let result: Result<Config, _> = serde_json::from_str(r#"{"gcf": {"enabled": "yes"}}"#);
    let err = result.expect_err("string enabled must be rejected");
    let msg = err.to_string();
    assert!(
        msg.contains("enabled") || msg.contains("expected"),
        "error should identify the offending field, got: {msg}"
    );
}

#[test]
fn malformed_gcf_block_type_is_rejected() {
    // The `gcf` section itself must be an object; a bare string is invalid.
    let result: Result<Config, _> = serde_json::from_str(r#"{"gcf": "on"}"#);
    let err = result.expect_err("non-object gcf block must be rejected");
    let msg = err.to_string();
    assert!(
        msg.contains("gcf") || msg.contains("struct") || msg.contains("expected"),
        "error should mention the gcf section or the type mismatch, got: {msg}"
    );
}

#[test]
fn malformed_gcf_block_preserves_line_information() {
    // FR-008 parity with the shared parse diagnostics: the error text for a
    // malformed gcf block carries a line number, not "line 0".
    let json = "{\n  \"gcf\": \"on\"\n}";
    let result: Result<Config, _> = serde_json::from_str(json);
    let err = result.expect_err("non-object gcf block must be rejected");
    let msg = err.to_string();
    assert!(
        msg.contains("line 2") || msg.contains("line 3") || msg.contains("line 4"),
        "error should carry a real line position, got: {msg}"
    );
}
