//! Integration tests for the `input_queue_capacity` config field
//! (spec `inputqueue` T-011 / FR-015).

use ragent_config::{Config, DEFAULT_INPUT_QUEUE_CAPACITY};

#[test]
fn default_capacity_is_32() {
    assert_eq!(DEFAULT_INPUT_QUEUE_CAPACITY, 32);
    assert_eq!(Config::default().effective_input_queue_capacity(), 32);
}

#[test]
fn unset_capacity_falls_back_to_the_default() {
    let config = Config::default();
    assert!(config.input_queue_capacity.is_none());
    assert_eq!(
        config.effective_input_queue_capacity(),
        DEFAULT_INPUT_QUEUE_CAPACITY
    );
}

#[test]
fn configured_capacity_is_used() {
    let mut config = Config::default();
    config.input_queue_capacity = Some(8);
    assert_eq!(config.effective_input_queue_capacity(), 8);
}

#[test]
fn capacity_is_clamped_to_the_valid_range() {
    let mut zero = Config::default();
    zero.input_queue_capacity = Some(0);
    assert_eq!(zero.effective_input_queue_capacity(), 1);

    let mut too_big = Config::default();
    too_big.input_queue_capacity = Some(500);
    assert_eq!(
        too_big.effective_input_queue_capacity(),
        99,
        "the counter renders two digits, so the capacity clamps at 99"
    );
}

#[test]
fn parses_input_queue_capacity_from_json() {
    let json = r#"{ "input_queue_capacity": 5 }"#;
    let config: Config = serde_json::from_str(json).expect("parse config");
    assert_eq!(config.input_queue_capacity, Some(5));
    assert_eq!(config.effective_input_queue_capacity(), 5);
}

#[test]
fn omitted_input_queue_capacity_parses_as_none() {
    let config: Config = serde_json::from_str(r#"{ "default_agent": "coder" }"#).expect("parse");
    assert!(config.input_queue_capacity.is_none());
}

#[test]
fn overlay_capacity_overrides_base() {
    let base = Config::default();
    let mut overlay = Config::default();
    overlay.input_queue_capacity = Some(12);

    let merged = Config::merge(base, overlay);
    assert_eq!(merged.effective_input_queue_capacity(), 12);
}

#[test]
fn absent_overlay_capacity_preserves_base() {
    let mut base = Config::default();
    base.input_queue_capacity = Some(7);

    let merged = Config::merge(base, Config::default());
    assert_eq!(merged.input_queue_capacity, Some(7));
}

#[test]
fn serialised_default_omits_input_queue_capacity() {
    let json = serde_json::to_value(Config::default()).expect("serialise config");
    assert!(
        json.get("input_queue_capacity").is_none(),
        "an unset capacity must be omitted from ragent.json"
    );
}

#[test]
fn roundtrip_preserves_input_queue_capacity() {
    let mut config = Config::default();
    config.input_queue_capacity = Some(20);

    let json = serde_json::to_string(&config).expect("serialise config");
    let decoded: Config = serde_json::from_str(&json).expect("deserialise config");
    assert_eq!(decoded.input_queue_capacity, Some(20));
}
