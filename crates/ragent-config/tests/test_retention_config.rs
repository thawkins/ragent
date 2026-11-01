//! Tests for the `storage.retention` configuration block: defaults, JSON
//! parsing, and validation.

#![forbid(unsafe_code)]

use ragent_config::retention::{DEFAULT_MESSAGE_DAYS, RetentionConfig, StorageConfig};

/// The default config leaves retention off, so the startup prune never runs
/// unless the operator opts in.
#[test]
fn retention_defaults_to_disabled_with_fourteen_days() {
    let cfg = RetentionConfig::default();
    assert!(!cfg.enabled, "retention must be opt-in");
    assert_eq!(cfg.message_days, DEFAULT_MESSAGE_DAYS);
    assert_eq!(cfg.message_days, 14);
    assert!(cfg.is_default());
}

/// A config block parsed from JSON enables retention and sets the window.
#[test]
fn retention_parses_from_the_storage_section() {
    let cfg: StorageConfig =
        serde_json::from_str(r#"{ "retention": { "enabled": true, "message_days": 14 } }"#)
            .expect("parse");
    assert!(cfg.retention.enabled);
    assert_eq!(cfg.retention.message_days, 14);
    assert!(!cfg.is_default());
}

/// A `storage` section naming only `enabled` keeps the 14-day default.
#[test]
fn retention_section_defaults_the_window() {
    let cfg: StorageConfig =
        serde_json::from_str(r#"{ "retention": { "enabled": true } }"#).expect("parse");
    assert!(cfg.retention.enabled);
    assert_eq!(cfg.retention.message_days, 14);
}

/// A zero-day window is rejected by validation rather than emptying the store.
#[test]
fn retention_rejects_a_zero_day_window() {
    let cfg = RetentionConfig {
        enabled: true,
        message_days: 0,
    };
    let problems = cfg.validate();
    assert_eq!(problems.len(), 1);
    assert!(problems[0].contains("message_days"));
}

/// A one-day window validates cleanly.
#[test]
fn retention_accepts_a_positive_window() {
    let cfg = RetentionConfig {
        enabled: true,
        message_days: 1,
    };
    assert_eq!(cfg.validate(), Vec::<String>::new());
}

/// A config file with no `storage` section keeps retention disabled and omits
/// the key on the way back out, so an untouched config stays untouched.
#[test]
fn absent_storage_section_is_disabled_and_not_serialised() {
    let cfg: ragent_config::config::Config =
        serde_json::from_str("{}").expect("parse empty config");
    assert!(!cfg.storage.retention.enabled);
    let back = serde_json::to_value(&cfg).expect("serialise");
    assert!(
        back.get("storage").is_none(),
        "the storage key must be omitted at its default state"
    );
}
