//! Tests for the injectable store-index fetch seam and the offline fixture
//! fetcher (spec `pluginstores` T-019; FR-032, FR-033, FR-037, NFR-003, NFR-004).
//!
//! Every test here runs offline: the fixture fetcher answers from in-memory
//! bytes and never contacts a store. The one production-path test points at a
//! refused loopback endpoint so the default HTTPS fetcher's contained error
//! arrives without any external network dependency (NFR-003).

use std::time::Duration;

use ragent_config::PluginStoresConfig;
use ragent_plugins::{
    DEFAULT_CLAUDE_STORE_URL, DEFAULT_CODEX_STORE_URL, FetchLimits, FixtureStoreFetcher,
    NetworkStoreFetcher, StoreEndpoint, StoreError, StoreIndexFetcher, StoreKind, default_fetcher,
};

/// Resolve `kind`'s effective endpoint with no `plugins.stores` block, so it is
/// the compiled default (FR-027, FR-033).
fn default_endpoint(kind: StoreKind) -> StoreEndpoint {
    kind.effective_endpoint_with_source(&PluginStoresConfig::default())
        .expect("a compiled default is a valid endpoint")
        .0
}

/// The compiled default index bytes for `kind`, as the fixture fetcher serves them.
fn default_index(kind: StoreKind) -> ragent_plugins::StoreIndex {
    FixtureStoreFetcher::new()
        .with_default_endpoints()
        .fetch_index(kind, &default_endpoint(kind), &FetchLimits::default())
        .unwrap_or_else(|e| panic!("fixture index for {}: {e}", kind.token()))
}

// ── The fixture fetcher serves both compiled defaults (FR-032, NFR-004) ──────

#[test]
fn the_fixture_fetcher_serves_both_compiled_default_endpoints() {
    let fetcher = FixtureStoreFetcher::new().with_default_endpoints();
    assert_eq!(
        fetcher.endpoints(),
        vec![DEFAULT_CLAUDE_STORE_URL, DEFAULT_CODEX_STORE_URL],
        "both compiled defaults are registered (sorted)"
    );
    // Neither store's default is left unverified (NFR-004).
    for kind in StoreKind::ALL {
        let index = fetcher
            .fetch_index(kind, &default_endpoint(kind), &FetchLimits::default())
            .unwrap_or_else(|e| panic!("{} default served: {e}", kind.token()));
        assert!(
            !index.entries.is_empty(),
            "{} default fixture carries entries",
            kind.token()
        );
        assert_eq!(index.skipped, 0, "no fixture entry is malformed");
    }
}

#[test]
fn the_codex_fixture_matches_the_compiled_default_and_an_installed_fixture() {
    // With no stores block the effective endpoint is the compiled Codex default.
    let endpoint = default_endpoint(StoreKind::Codex);
    assert_eq!(endpoint.as_str(), DEFAULT_CODEX_STORE_URL);

    let index = default_index(StoreKind::Codex);
    assert_eq!(index.store.as_deref(), Some("codex"));

    // One entry's id matches the installed fixture under assets/plugins/fixtures/.
    let weather = index
        .entries
        .iter()
        .find(|e| e.id == "codex-weather")
        .expect("codex-weather entry present");
    assert_eq!(weather.name, "Codex Weather");
    assert_eq!(weather.version, "1.2.0");
    assert!(weather.source.starts_with("https://"));
    assert_eq!(
        weather.tags,
        vec!["weather".to_string(), "http".to_string()]
    );

    // Another entry omits every optional field (FR-003).
    let time = index
        .entries
        .iter()
        .find(|e| e.id == "codex-time")
        .expect("codex-time entry present");
    assert!(time.description.is_empty());
    assert!(time.tags.is_empty());
    assert!(time.homepage.is_none());
}

#[test]
fn the_claude_fixture_matches_the_compiled_default_and_an_installed_fixture() {
    let endpoint = default_endpoint(StoreKind::Claude);
    assert_eq!(endpoint.as_str(), DEFAULT_CLAUDE_STORE_URL);

    let index = default_index(StoreKind::Claude);
    assert_eq!(index.store.as_deref(), Some("claude"));

    let todo = index
        .entries
        .iter()
        .find(|e| e.id == "claude-todo")
        .expect("claude-todo entry present");
    assert_eq!(todo.name, "Claude Todo");
    assert_eq!(todo.dialect.as_deref(), Some("claude"));
    assert!(todo.source.starts_with("https://"));

    let time = index
        .entries
        .iter()
        .find(|e| e.id == "claude-time")
        .expect("claude-time entry present");
    assert!(time.description.is_empty());
    assert!(time.tags.is_empty());
}

// ── Unknown / oversized / malformed endpoints are contained (FR-025) ─────────

#[test]
fn an_endpoint_with_no_registered_fixture_is_a_contained_error() {
    let fetcher = FixtureStoreFetcher::new();
    let endpoint = StoreEndpoint::parse("https://example.org/nope.json").expect("valid endpoint");
    let err = fetcher
        .fetch_index(StoreKind::Codex, &endpoint, &FetchLimits::default())
        .expect_err("no fixture is registered");
    match err {
        StoreError::Network { detail } => assert!(detail.contains("no offline fixture")),
        other => panic!("expected a contained Network error, got {other:?}"),
    }
}

#[test]
fn an_oversized_fixture_is_refused_with_the_byte_ceiling() {
    let endpoint = StoreEndpoint::parse(DEFAULT_CODEX_STORE_URL).expect("valid endpoint");
    let fetcher = FixtureStoreFetcher::new().with_default_endpoints();
    // A ceiling far below the fixture size aborts before parsing (FR-025).
    let limits = FetchLimits::new(Duration::from_secs(5), 4);
    let err = fetcher
        .fetch_index(StoreKind::Codex, &endpoint, &limits)
        .expect_err("the fixture exceeds the ceiling");
    assert_eq!(err, StoreError::TooLarge { limit: 4 });
}

#[test]
fn a_malformed_fixture_body_is_a_contained_error() {
    let endpoint = StoreEndpoint::parse("https://example.org/bad.json").expect("valid endpoint");
    let fetcher = FixtureStoreFetcher::new().with_index("https://example.org/bad.json", b"{ nope");
    let err = fetcher
        .fetch_index(StoreKind::Codex, &endpoint, &FetchLimits::default())
        .expect_err("malformed JSON is refused");
    assert!(matches!(err, StoreError::MalformedJson { .. }));
}

#[test]
fn a_registered_fixture_is_served_for_its_exact_endpoint_only() {
    let endpoint = StoreEndpoint::parse("https://example.org/a.json").expect("valid endpoint");
    let bytes = br#"{"plugins":[{"id":"a","name":"A","version":"1","source":"https://x/a.zip"}]}"#;
    let fetcher = FixtureStoreFetcher::new().with_index("https://example.org/a.json", &bytes[..]);
    let index = fetcher
        .fetch_index(StoreKind::Codex, &endpoint, &FetchLimits::default())
        .expect("registered fixture served");
    assert_eq!(index.entries.len(), 1);
    assert_eq!(index.entries[0].id, "a");
}

// ── The seam is production-transparent (FR-037) ──────────────────────────────

#[test]
fn the_default_fetcher_is_the_network_fetcher() {
    // The production default is the HTTPS fetcher; a refused loopback endpoint
    // yields a contained error with no external network dependency (FR-037,
    // NFR-003).
    let endpoint = StoreEndpoint::parse("https://127.0.0.1:9/index.json").expect("valid endpoint");
    let limits = FetchLimits::new(Duration::from_millis(2000), FetchLimits::DEFAULT_MAX_BYTES);
    let err = default_fetcher()
        .fetch_index(StoreKind::Codex, &endpoint, &limits)
        .expect_err("the loopback endpoint refuses the connection");
    assert!(
        matches!(err, StoreError::Network { .. } | StoreError::Timeout { .. }),
        "a contained network failure, got {err:?}"
    );
}

#[test]
fn a_network_fetcher_instance_delegates_to_the_http_path() {
    // `NetworkStoreFetcher` is the concrete production fetcher; pointing it at a
    // refused port exercises the same contained-failure contract as above.
    let endpoint = StoreEndpoint::parse("https://127.0.0.1:9/index.json").expect("valid endpoint");
    let limits = FetchLimits::new(Duration::from_millis(2000), FetchLimits::DEFAULT_MAX_BYTES);
    let err = NetworkStoreFetcher
        .fetch_index(StoreKind::Codex, &endpoint, &limits)
        .expect_err("the loopback endpoint refuses the connection");
    assert!(matches!(
        err,
        StoreError::Network { .. } | StoreError::Timeout { .. }
    ));
}

#[test]
fn a_closure_is_a_valid_fetch_seam() {
    // Any `Fn(&StoreEndpoint, &FetchLimits) -> Result<StoreIndex, StoreError>`
    // is a fetch seam, so a test can inject a bespoke implementation without a
    // dedicated type (the blanket impl).
    let seam: Box<dyn StoreIndexFetcher> = Box::new(
        |_kind: StoreKind,
         _endpoint: &StoreEndpoint,
         _limits: &FetchLimits|
         -> Result<_, StoreError> {
            Ok(ragent_plugins::StoreIndex {
                store: Some("injected".to_string()),
                entries: Vec::new(),
                skipped: 0,
            })
        },
    );
    let endpoint = StoreEndpoint::parse(DEFAULT_CLAUDE_STORE_URL).expect("valid endpoint");
    let index = seam
        .fetch_index(StoreKind::Claude, &endpoint, &FetchLimits::default())
        .expect("closure seam serves an index");
    assert_eq!(index.store.as_deref(), Some("injected"));
}
// ── The on-disk store fixtures parse and match the compiled defaults ─────────

/// The on-disk fixture store-index path for `kind`
/// (`assets/plugins/fixtures/stores/<token>.json`).
fn on_disk_fixture(kind: StoreKind) -> std::path::PathBuf {
    std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../assets/plugins/fixtures/stores")
        .join(format!("{}.json", kind.token()))
}

#[test]
fn the_on_disk_store_fixtures_are_valid_and_match_the_compiled_defaults() {
    for kind in StoreKind::ALL {
        let path = on_disk_fixture(kind);
        let bytes =
            std::fs::read(&path).unwrap_or_else(|e| panic!("read fixture {}: {e}", path.display()));
        let disk = ragent_plugins::StoreIndex::from_bytes(&bytes)
            .unwrap_or_else(|e| panic!("parse fixture {}: {e}", path.display()));
        assert_eq!(
            disk.skipped,
            0,
            "{} on-disk fixture has no malformed entries",
            kind.token()
        );
        assert!(
            !disk.entries.is_empty(),
            "{} on-disk fixture carries entries",
            kind.token()
        );

        // The on-disk fixture and the compiled-default fixture must not drift.
        let compiled = default_index(kind);
        let disk_ids: Vec<&str> = disk.entries.iter().map(|e| e.id.as_str()).collect();
        let compiled_ids: Vec<&str> = compiled.entries.iter().map(|e| e.id.as_str()).collect();
        assert_eq!(
            disk_ids,
            compiled_ids,
            "{} on-disk and compiled-default fixtures list the same entries",
            kind.token()
        );

        // Each carries an installed-fixture id and an entry with no optional fields.
        assert!(disk.entries.iter().any(|e| e.description.is_empty()));
        assert!(disk.entries.iter().any(|e| !e.source.is_empty()));
    }
}
