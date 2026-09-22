//! Tests for the store-index fetch and parse (spec `pluginstores` T-003;
//! FR-003, FR-013, FR-016, FR-023, FR-024, FR-025).

use std::io::Cursor;
use std::time::Duration;

use ragent_config::PluginStoresConfig;
use ragent_plugins::{
    FetchLimits, StoreEndpoint, StoreEndpointError, StoreError, StoreIndex, StoreKind, store_label,
};

fn spec_index() -> Vec<u8> {
    br#"{
      "store": "codex",
      "plugins": [
        {
          "id": "codex-weather",
          "name": "Codex Weather",
          "version": "1.2.0",
          "description": "Weather lookups",
          "source": "https://example.org/codex-weather.zip",
          "dialect": "codex",
          "tags": ["weather", "http"],
          "homepage": "https://example.org/codex-weather"
        },
        {
          "id": "codex-time",
          "name": "Codex Time",
          "version": "0.9.1",
          "source": "https://example.org/codex-time.zip"
        }
      ]
    }"#
    .to_vec()
}

// ── StoreIndex parse: shapes (FR-003) ───────────────────────────────────────

#[test]
fn parses_the_spec_object_shape_and_captures_the_store_token() {
    let index = StoreIndex::from_bytes(&spec_index()).expect("valid index");
    assert_eq!(index.store.as_deref(), Some("codex"));
    assert_eq!(index.entries.len(), 2);
    assert_eq!(index.skipped, 0);
    let first = &index.entries[0];
    assert_eq!(first.id, "codex-weather");
    assert_eq!(first.name, "Codex Weather");
    assert_eq!(first.version, "1.2.0");
    assert_eq!(first.description, "Weather lookups");
    assert_eq!(first.source, "https://example.org/codex-weather.zip");
    assert_eq!(first.tags, vec!["weather".to_string(), "http".to_string()]);
    // The second entry omits optional fields and is still accepted (FR-003).
    assert_eq!(index.entries[1].id, "codex-time");
    assert!(index.entries[1].description.is_empty());
    assert!(index.entries[1].tags.is_empty());
}

#[test]
fn parses_a_bare_top_level_array() {
    let body = br#"[{"id":"a","name":"A","version":"1","source":"https://x/a.zip"}]"#;
    let index = StoreIndex::from_bytes(body).expect("valid");
    assert!(index.store.is_none());
    assert_eq!(index.entries.len(), 1);
    assert_eq!(index.entries[0].id, "a");
}

#[test]
fn accepts_an_empty_plugins_array() {
    let index = StoreIndex::from_bytes(br#"{"plugins":[]}"#).expect("valid");
    assert!(index.entries.is_empty());
    assert_eq!(index.skipped, 0);
}

#[test]
fn rejects_an_object_without_a_plugins_array() {
    // An object that omits "plugins" entirely is a shape error (FR-013).
    let err = StoreIndex::from_bytes(br#"{"store":"codex"}"#).expect_err("must reject");
    assert!(matches!(err, StoreError::MalformedShape { .. }));
}

#[test]
fn accepts_an_object_with_only_a_store_token_and_no_plugins() {
    let body = br#"{"store":"codex","plugins":[]}"#;
    let index = StoreIndex::from_bytes(body).expect("valid");
    assert_eq!(index.store.as_deref(), Some("codex"));
    assert!(index.entries.is_empty());
}

#[test]
fn rejects_plugins_that_is_not_an_array() {
    let err = StoreIndex::from_bytes(br#"{"plugins":{"id":"a"}}"#).expect_err("must reject");
    assert!(matches!(err, StoreError::MalformedShape { .. }));
}

#[test]
fn rejects_malformed_json_with_a_detail() {
    let err = StoreIndex::from_bytes(b"{ not json").expect_err("must reject");
    match err {
        StoreError::MalformedJson { detail } => assert!(!detail.is_empty()),
        other => panic!("expected MalformedJson, got {other:?}"),
    }
}

#[test]
fn rejects_non_object_non_array_roots() {
    for body in [b"42".as_slice(), b"\"text\"", b"null", b"true"] {
        let err = StoreIndex::from_bytes(body).expect_err("must reject");
        assert!(
            matches!(err, StoreError::MalformedShape { .. }),
            "expected MalformedShape for {body:?}, got {err:?}"
        );
    }
}

// ── Per-entry validation: skip and count (FR-013, FR-025) ───────────────────

#[test]
fn skips_malformed_entries_and_counts_them() {
    let body = br#"{
      "plugins": [
        {"id":"ok","name":"OK","version":"1","source":"https://x/ok.zip"},
        {"name":"no id","version":"1","source":"https://x/z.zip"},
        {"id":"b","name":"B","version":"1"},
        {"id":"","name":"empty id","version":"1","source":"https://x/c.zip"},
        42,
        "a string",
        {"id":"ok2","name":"OK2","version":"2","source":"https://x/ok2.zip"}
      ]
    }"#;
    let index = StoreIndex::from_bytes(body).expect("valid index");
    assert_eq!(index.entries.len(), 2);
    assert_eq!(
        index
            .entries
            .iter()
            .map(|e| e.id.as_str())
            .collect::<Vec<_>>(),
        vec!["ok", "ok2"]
    );
    assert_eq!(index.skipped, 5);
}

#[test]
fn a_single_malformed_entry_does_not_fail_the_whole_index() {
    let body = br#"{"plugins":[{"name":"no id"}]}"#;
    let index = StoreIndex::from_bytes(body).expect("still a valid index");
    assert!(index.entries.is_empty());
    assert_eq!(index.skipped, 1);
}

// ── read_capped: the streamed byte ceiling (FR-013, FR-025) ─────────────────

#[test]
fn read_capped_returns_the_full_body_under_the_ceiling() {
    let body = vec![7_u8; 1000];
    let out = ragent_plugins::read_capped(Cursor::new(body.clone()), 2000).expect("under cap");
    assert_eq!(out, body);
}

#[test]
fn read_capped_accepts_a_body_exactly_at_the_ceiling() {
    let body = vec![1_u8; 1024];
    let out = ragent_plugins::read_capped(Cursor::new(body.clone()), 1024).expect("at cap");
    assert_eq!(out, body);
}

#[test]
fn read_capped_aborts_past_the_ceiling() {
    let body = vec![0_u8; 5000];
    let err = ragent_plugins::read_capped(Cursor::new(body), 1024).expect_err("over cap");
    assert_eq!(err, StoreError::TooLarge { limit: 1024 });
}

#[test]
fn read_capped_handles_an_empty_body() {
    let out = ragent_plugins::read_capped(Cursor::new(Vec::<u8>::new()), 10).expect("empty");
    assert!(out.is_empty());
}

// ── FetchLimits (FR-016; config wiring) ─────────────────────────────────────

#[test]
fn fetch_limits_defaults_are_the_compiled_values() {
    let limits = FetchLimits::default();
    assert_eq!(limits.timeout, Duration::from_millis(10_000));
    assert_eq!(limits.max_bytes, 2 * 1024 * 1024);
    assert_eq!(
        FetchLimits::new(FetchLimits::DEFAULT_TIMEOUT, FetchLimits::DEFAULT_MAX_BYTES),
        limits
    );
}

#[test]
fn fetch_limits_from_millis_maps_zero_onto_the_default() {
    let limits = FetchLimits::from_millis(0, 0);
    assert_eq!(limits, FetchLimits::default());
    let custom = FetchLimits::from_millis(1500, 4096);
    assert_eq!(custom.timeout, Duration::from_millis(1500));
    assert_eq!(custom.max_bytes, 4096);
}

#[test]
fn fetch_limits_are_derived_from_the_stores_config() {
    let cfg = PluginStoresConfig {
        timeout_ms: 2500,
        max_index_bytes: 8192,
        ..PluginStoresConfig::default()
    };
    let limits = FetchLimits::from(&cfg);
    assert_eq!(limits.timeout, Duration::from_millis(2500));
    assert_eq!(limits.max_bytes, 8192);
    // The compiled-default config maps onto the compiled limits.
    let defaults = FetchLimits::from(&PluginStoresConfig::default());
    assert_eq!(defaults, FetchLimits::default());
}

// ── Failure containment: endpoint guard (FR-024, FR-025) ────────────────────

#[test]
fn endpoint_errors_convert_into_store_errors() {
    // A non-https endpoint is refused before a fetch can start, and the refusal
    // is a contained StoreError (never a panic).
    let refused = StoreEndpoint::parse("http://example.org/i.json").expect_err("refused");
    let err: StoreError = refused.into();
    assert!(matches!(err, StoreError::Endpoint(_)));
    assert!(matches!(
        StoreEndpoint::parse(""),
        Err(StoreEndpointError::Empty)
    ));
}

#[test]
fn store_labels_name_each_store() {
    use ragent_plugins::StoreKind;
    assert_eq!(store_label(StoreKind::Codex), "Codex");
    assert_eq!(store_label(StoreKind::Claude), "Claude");
}

// ── Live fetch failure containment (FR-013, FR-024, FR-025) ─────────────────

#[test]
fn fetch_reports_a_network_failure_without_panicking() {
    // A host that cannot resolve (RFC 6761 `.invalid`) fails the connection.
    // The result is a contained StoreError, never a panic (FR-025). No plugin
    // code runs and no store is touched (FR-023).
    let endpoint =
        StoreEndpoint::parse("https://store.invalid.example/index.json").expect("https endpoint");
    let limits = FetchLimits::new(Duration::from_millis(2_000), 2 * 1024 * 1024);
    let err = ragent_plugins::fetch_index(StoreKind::Codex, &endpoint, &limits)
        .expect_err("network must fail");
    assert!(
        matches!(err, StoreError::Network { .. } | StoreError::Timeout { .. }),
        "expected a contained network/timeout error, got {err:?}"
    );
}

// ── The byte cap is enforced before JSON parse (FR-013, FR-025) ─────────────

#[test]
fn an_oversized_body_never_reaches_the_parser() {
    // A valid-looking index padded past the ceiling is refused by the reader,
    // so the oversized payload is reported as TooLarge, not parsed.
    let mut body = spec_index();
    body.resize(4096, b' ');
    let err = ragent_plugins::read_capped(Cursor::new(body), 1024).expect_err("over cap");
    assert_eq!(err, StoreError::TooLarge { limit: 1024 });
}
