//! Tests for catalogue fetch, cache, and normalisation providers (spec
//! `connectors` T-005; FR-024, FR-025, FR-031).

use std::path::PathBuf;
use std::time::Duration;

use ragent_connectors::{
    CACHE_DIR, CatalogueCache, CatalogueEndpoint, CatalogueFetcher, CatalogueKind, CatalogueLimits,
    ConnectorAuthShape, ConnectorCatalogue, ConnectorError, FixtureCatalogueFetcher,
    NetworkCatalogueFetcher, normalise_transport, parse_catalogue, provider_for,
};

static SEQ: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);

/// RAII sandboxed temp tree rooted at `target/temp/` (AGENTS.md: no `/tmp`).
struct TempTree(PathBuf);

impl TempTree {
    fn new(name: &str) -> Self {
        let unique = SEQ.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(format!(
            "../../target/temp/connectors-test/fetch-{name}-{}-{unique}",
            std::process::id()
        ));
        std::fs::create_dir_all(&path).expect("temp tree creatable");
        Self(path)
    }
}

impl Drop for TempTree {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn endpoint(url: &str) -> CatalogueEndpoint {
    CatalogueEndpoint::parse(url).expect("test endpoint is a valid https URL")
}

fn claude_origin() -> url::Url {
    url::Url::parse("https://api.example.test/catalogue/index.json").expect("valid url")
}

/// A two-entry catalogue: one expressible remote server and one unexpressible
/// (`grpc`) entry (FR-025).
const TWO_ENTRY_CATALOGUE: &str = r#"{
  "servers": [
    {
      "slug": "echo",
      "name": "Echo",
      "one_liner": "Echoes your input.",
      "categories": ["developer", "utility"],
      "works_with": ["claude", "claude-code"],
      "remote": {
        "url": "https://mcp.example.test/v1/mcp",
        "transport": "streamable-http",
        "is_authless": true
      }
    },
    {
      "slug": "unsupported",
      "name": "Unsupported",
      "categories": ["developer"],
      "remote": {
        "url": "https://mcp.example.test/grpc",
        "transport": "grpc",
        "is_authless": true
      }
    }
  ]
}"#;

// ── Provider normalisation (FR-024, FR-025) ─────────────────────────────────

#[test]
fn two_entry_catalogue_with_one_unexpressible_returns_one_and_skips_one() {
    let provider = provider_for(CatalogueKind::Claude);
    let parsed = parse_catalogue(&*provider, TWO_ENTRY_CATALOGUE.as_bytes(), &claude_origin())
        .expect("well-formed document parses");

    assert_eq!(parsed.connectors.len(), 1, "one expressible connector");
    assert_eq!(parsed.skipped, 1, "the unexpressible entry is skipped");

    let echo = &parsed.connectors[0];
    assert_eq!(echo.id.as_str(), "echo");
    assert_eq!(echo.name, "Echo");
    assert_eq!(echo.description, "Echoes your input.");
    assert_eq!(echo.category, "developer");
    assert!(echo.tags.contains(&"developer".to_string()));
    assert!(echo.tags.contains(&"utility".to_string()));
    assert!(echo.tags.contains(&"claude-code".to_string()));
    assert_eq!(echo.servers.len(), 1);
    assert_eq!(echo.servers[0].transport, "http");
    assert_eq!(
        echo.servers[0].url.as_deref(),
        Some("https://mcp.example.test/v1/mcp")
    );
    // A `streamable-http` + authless remote infers the `none` auth shape.
    assert_eq!(echo.auth, ConnectorAuthShape::None);
}

#[test]
fn claude_provider_reads_authenticated_remote_as_oauth() {
    let doc = r#"{
      "servers": [
        {
          "slug": "google-drive",
          "display_name": "Google Drive",
          "one_liner": "Access Drive files.",
          "categories": ["productivity"],
          "remote": {
            "url": "https://drivemcp.example.test/mcp",
            "transport": "streamable-http",
            "is_authless": false,
            "auth_posture": "auth_required"
          }
        }
      ]
    }"#;
    let provider = provider_for(CatalogueKind::Claude);
    let parsed = parse_catalogue(&*provider, doc.as_bytes(), &claude_origin()).expect("parses");
    assert_eq!(parsed.connectors.len(), 1);
    assert_eq!(parsed.connectors[0].auth, ConnectorAuthShape::Oauth);
    assert_eq!(parsed.connectors[0].name, "Google Drive");
}

#[test]
fn native_provider_parses_connectors_array_and_mcp_servers_object() {
    // The native shape: a `connectors` array with an `mcpServers` object.
    let doc = r#"{
      "connectors": [
        {
          "id": "native-one",
          "name": "Native One",
          "category": "data",
          "mcpServers": {
            "main": { "command": "/bin/true" }
          }
        }
      ]
    }"#;
    let provider = provider_for(CatalogueKind::Claude);
    let parsed = parse_catalogue(&*provider, doc.as_bytes(), &claude_origin()).expect("parses");
    assert_eq!(parsed.connectors.len(), 1);
    assert_eq!(parsed.connectors[0].id.as_str(), "native-one");
    assert_eq!(parsed.connectors[0].category, "data");
    assert_eq!(parsed.connectors[0].servers[0].transport, "stdio");
    assert_eq!(
        parsed.connectors[0].servers[0].command.as_deref(),
        Some("/bin/true")
    );
}

#[test]
fn native_provider_accepts_a_bare_top_level_array() {
    let doc = r#"[
      { "id": "one", "servers": [ { "id": "main", "command": "/bin/true" } ] }
    ]"#;
    let provider = provider_for(CatalogueKind::Claude);
    let parsed = parse_catalogue(&*provider, doc.as_bytes(), &claude_origin()).expect("parses");
    assert_eq!(parsed.connectors.len(), 1);
    assert_eq!(parsed.skipped, 0);
}

#[test]
fn malformed_entry_is_skipped_not_fatal() {
    // Entry 2 has no server definition at all; entry 1 is fine.
    let doc = r#"{
      "connectors": [
        { "id": "good", "servers": [ { "id": "main", "command": "/bin/true" } ] },
        { "id": "bad" }
      ]
    }"#;
    let provider = provider_for(CatalogueKind::Claude);
    let parsed = parse_catalogue(&*provider, doc.as_bytes(), &claude_origin()).expect("parses");
    assert_eq!(parsed.connectors.len(), 1);
    assert_eq!(parsed.skipped, 1);
}

#[test]
fn malformed_json_names_the_parse_cause() {
    let provider = provider_for(CatalogueKind::Claude);
    let err = parse_catalogue(&*provider, b"{ not json", &claude_origin())
        .expect_err("malformed JSON is refused");
    match err {
        ConnectorError::CatalogueParse { detail } => {
            assert!(!detail.is_empty(), "the parse cause is named");
        }
        other => panic!("expected CatalogueParse, got {other:?}"),
    }
}

#[test]
fn unrecognised_shape_is_refused_with_a_named_reason() {
    let provider = provider_for(CatalogueKind::Claude);
    let err = parse_catalogue(&*provider, br#"{"something_else": []}"#, &claude_origin())
        .expect_err("a foreign shape is refused");
    match err {
        ConnectorError::CatalogueShape { detail } => {
            assert!(
                detail.contains("servers"),
                "the reason names the expected array: {detail}"
            );
        }
        other => panic!("expected CatalogueShape, got {other:?}"),
    }
}

#[test]
fn transport_spellings_normalise_and_unknowns_do_not() {
    assert_eq!(normalise_transport("streamable-http"), "http");
    assert_eq!(normalise_transport("streamable_http"), "http");
    assert_eq!(normalise_transport("STREAMABLE-HTTP"), "http");
    assert_eq!(normalise_transport("http"), "http");
    assert_eq!(normalise_transport("sse"), "sse");
    assert_eq!(normalise_transport(""), "stdio");
    assert_eq!(
        normalise_transport("grpc"),
        "grpc",
        "a foreign transport stays unexpressible"
    );
}

// ── Bounded download (FR-031) ───────────────────────────────────────────────

#[test]
fn read_capped_passes_a_body_under_the_cap() {
    let body = b"hello world";
    let bytes = ragent_connectors::read_capped(&body[..], 1024).expect("under cap");
    assert_eq!(bytes, body);
}

#[test]
fn read_capped_aborts_a_body_over_the_cap() {
    let body = vec![b'x'; 1024];
    let err = ragent_connectors::read_capped(&body[..], 100).expect_err("over cap");
    match err {
        ConnectorError::CatalogueTooLarge { limit } => assert_eq!(limit, 100),
        other => panic!("expected CatalogueTooLarge, got {other:?}"),
    }
}

#[test]
fn limits_from_config_maps_zero_to_default_and_reads_ttl() {
    let limits = CatalogueLimits::from_config(0, 0, 0);
    assert_eq!(limits.timeout, CatalogueLimits::DEFAULT_TIMEOUT);
    assert_eq!(limits.max_bytes, CatalogueLimits::DEFAULT_MAX_BYTES);
    assert_eq!(
        limits.cache_ttl,
        Duration::ZERO,
        "zero TTL disables the cache"
    );

    let limits = CatalogueLimits::from_config(1500, 4096, 60);
    assert_eq!(limits.timeout, Duration::from_millis(1500));
    assert_eq!(limits.max_bytes, 4096);
    assert_eq!(limits.cache_ttl, Duration::from_secs(60));
}

// ── Offline fixture fetcher (NFR-003) ───────────────────────────────────────

#[test]
fn fixture_fetcher_serves_registered_bytes_and_refuses_unknown_endpoints() {
    let ep = endpoint("https://api.example.test/index.json");
    let fetcher = FixtureCatalogueFetcher::new()
        .with_index("https://api.example.test/index.json", TWO_ENTRY_CATALOGUE);
    let limits = CatalogueLimits::default();
    let parsed = fetcher
        .fetch(CatalogueKind::Claude, &ep, &limits)
        .expect("registered endpoint is served");
    assert_eq!(parsed.connectors.len(), 1);

    let other = endpoint("https://api.example.test/other.json");
    let err = fetcher
        .fetch(CatalogueKind::Claude, &other, &limits)
        .expect_err("an unregistered endpoint is refused");
    assert!(matches!(err, ConnectorError::CatalogueFetch { .. }));
}

#[test]
fn fixture_fetcher_enforces_the_byte_ceiling() {
    let ep = endpoint("https://api.example.test/index.json");
    let fetcher = FixtureCatalogueFetcher::new()
        .with_index("https://api.example.test/index.json", "x".repeat(2048));
    let limits = CatalogueLimits::new(Duration::from_secs(1), 100, Duration::ZERO);
    let err = fetcher
        .fetch(CatalogueKind::Claude, &ep, &limits)
        .expect_err("oversize fixture is refused");
    assert!(matches!(err, ConnectorError::CatalogueTooLarge { .. }));
}

// ── Cache behaviour (FR-031) ────────────────────────────────────────────────

fn catalogue_with(endpoint_url: &str, fetched_at: u64) -> ConnectorCatalogue {
    let provider = provider_for(CatalogueKind::Claude);
    let parsed = parse_catalogue(&*provider, TWO_ENTRY_CATALOGUE.as_bytes(), &claude_origin())
        .expect("parses");
    ConnectorCatalogue {
        endpoint: endpoint_url.to_string(),
        connectors: parsed.connectors,
        skipped: parsed.skipped,
        fetched_at,
    }
}

#[test]
fn cache_round_trips_a_catalogue_within_ttl() {
    let tree = TempTree::new("cache-roundtrip");
    let cache = CatalogueCache::new(tree.0.join(CACHE_DIR));
    let ep = "https://api.example.test/index.json";
    let catalogue = catalogue_with(ep, 1_000);
    cache
        .store(CatalogueKind::Claude, &catalogue)
        .expect("cache write succeeds");

    let loaded = cache
        .load(CatalogueKind::Claude, ep, Duration::from_secs(3600), 1_100)
        .expect("within ttl");
    assert_eq!(loaded.connectors.len(), 1);
    assert_eq!(loaded.endpoint, ep);
}

#[test]
fn cache_entry_expires_after_ttl() {
    let tree = TempTree::new("cache-expiry");
    let cache = CatalogueCache::new(tree.0.join(CACHE_DIR));
    let ep = "https://api.example.test/index.json";
    cache
        .store(CatalogueKind::Claude, &catalogue_with(ep, 1_000))
        .expect("cache write succeeds");
    assert!(
        cache
            .load(CatalogueKind::Claude, ep, Duration::from_secs(10), 1_020)
            .is_none(),
        "an entry older than the ttl is a miss"
    );
}

#[test]
fn disabled_cache_never_serves() {
    let tree = TempTree::new("cache-disabled");
    let cache = CatalogueCache::new(tree.0.join(CACHE_DIR));
    let ep = "https://api.example.test/index.json";
    cache
        .store(CatalogueKind::Claude, &catalogue_with(ep, 1_000))
        .expect("cache write succeeds");
    assert!(
        cache
            .load(CatalogueKind::Claude, ep, Duration::ZERO, 1_000)
            .is_none()
    );
}

#[test]
fn cache_ignores_a_file_for_a_different_endpoint() {
    let tree = TempTree::new("cache-endpoint-mismatch");
    let cache = CatalogueCache::new(tree.0.join(CACHE_DIR));
    let ep = "https://api.example.test/index.json";
    cache
        .store(
            CatalogueKind::Claude,
            &catalogue_with("https://other.test/x.json", 1_000),
        )
        .expect("cache write succeeds");
    assert!(
        cache
            .load(CatalogueKind::Claude, ep, Duration::from_secs(3600), 1_000)
            .is_none()
    );
}

#[test]
fn fetch_catalogue_serves_from_cache_then_refetches_when_expired() {
    let tree = TempTree::new("cache-serve");
    let cache = CatalogueCache::new(tree.0.join(CACHE_DIR));
    let ep = endpoint("https://api.example.test/index.json");
    let fetcher = FixtureCatalogueFetcher::new()
        .with_index("https://api.example.test/index.json", TWO_ENTRY_CATALOGUE);
    let limits = CatalogueLimits::new(
        Duration::from_secs(1),
        CatalogueLimits::DEFAULT_MAX_BYTES,
        Duration::from_secs(600),
    );

    let first = ragent_connectors::fetch_catalogue(
        &fetcher,
        CatalogueKind::Claude,
        &ep,
        &limits,
        Some(&cache),
        1_000,
    )
    .expect("first fetch");
    assert!(!first.from_cache, "the first fetch is live");

    let second = ragent_connectors::fetch_catalogue(
        &fetcher,
        CatalogueKind::Claude,
        &ep,
        &limits,
        Some(&cache),
        1_030,
    )
    .expect("second fetch");
    assert!(
        second.from_cache,
        "a fresh cache entry is served without a network round trip"
    );

    let third = ragent_connectors::fetch_catalogue(
        &fetcher,
        CatalogueKind::Claude,
        &ep,
        &limits,
        Some(&cache),
        2_000,
    )
    .expect("third fetch");
    assert!(
        !third.from_cache,
        "an expired cache entry triggers a refetch"
    );
}

#[test]
fn a_failed_fetch_leaves_a_previously_cached_catalogue_untouched() {
    let tree = TempTree::new("cache-untouched");
    let cache = CatalogueCache::new(tree.0.join(CACHE_DIR));
    let ep = endpoint("https://api.example.test/index.json");
    let good = FixtureCatalogueFetcher::new()
        .with_index("https://api.example.test/index.json", TWO_ENTRY_CATALOGUE);
    let limits = CatalogueLimits::new(
        Duration::from_secs(1),
        CatalogueLimits::DEFAULT_MAX_BYTES,
        Duration::from_secs(600),
    );

    // Prime the cache at t=1000.
    let primed = ragent_connectors::fetch_catalogue(
        &good,
        CatalogueKind::Claude,
        &ep,
        &limits,
        Some(&cache),
        1_000,
    )
    .expect("priming fetch");
    assert!(!primed.from_cache);
    let cache_file = cache.path_for(CatalogueKind::Claude, ep.as_str());
    let before = std::fs::metadata(&cache_file)
        .expect("cache file exists")
        .len();

    // A stale lookup (well past the ttl) attempts a live fetch; a fetcher that
    // fails (unknown endpoint) must return the error and not touch the cache.
    let failing = FixtureCatalogueFetcher::new();
    let err = ragent_connectors::fetch_catalogue(
        &failing,
        CatalogueKind::Claude,
        &ep,
        &limits,
        Some(&cache),
        999_000,
    )
    .expect_err("the failing fetcher returns an error");
    assert!(matches!(err, ConnectorError::CatalogueFetch { .. }));

    let after = std::fs::metadata(&cache_file)
        .expect("cache file still exists")
        .len();
    assert_eq!(
        before, after,
        "a refused fetch leaves the cached catalogue untouched"
    );
    // The primed catalogue is still intact and loads within its ttl window.
    assert!(
        cache
            .load(
                CatalogueKind::Claude,
                ep.as_str(),
                Duration::from_secs(600),
                1_000
            )
            .is_some()
    );
}

#[test]
fn parse_failure_does_not_replace_an_existing_cache_entry() {
    let tree = TempTree::new("cache-parse-failure");
    let cache = CatalogueCache::new(tree.0.join(CACHE_DIR));
    let ep = endpoint("https://api.example.test/index.json");
    let limits = CatalogueLimits::new(
        Duration::from_secs(1),
        CatalogueLimits::DEFAULT_MAX_BYTES,
        Duration::from_secs(600),
    );

    let good = FixtureCatalogueFetcher::new()
        .with_index("https://api.example.test/index.json", TWO_ENTRY_CATALOGUE);
    ragent_connectors::fetch_catalogue(
        &good,
        CatalogueKind::Claude,
        &ep,
        &limits,
        Some(&cache),
        1_000,
    )
    .expect("priming fetch");

    // A stale lookup forces a live fetch; a malformed document from the same
    // endpoint must be refused before the cache is written.
    let malformed = FixtureCatalogueFetcher::new()
        .with_index("https://api.example.test/index.json", "{ truncated");
    let err = ragent_connectors::fetch_catalogue(
        &malformed,
        CatalogueKind::Claude,
        &ep,
        &limits,
        Some(&cache),
        999_000,
    )
    .expect_err("malformed document is refused");
    assert!(matches!(err, ConnectorError::CatalogueParse { .. }));

    // The primed catalogue is still served (it is untouched by the failed fetch).
    let cached = cache
        .load(
            CatalogueKind::Claude,
            ep.as_str(),
            Duration::from_secs(600),
            1_000,
        )
        .expect("the previously cached catalogue is still served");
    assert_eq!(cached.connectors.len(), 1);
    assert_eq!(
        cached.fetched_at, 1_000,
        "the cached entry is the original, not the bad document"
    );
}

#[test]
fn network_fetcher_is_the_default_and_implements_the_seam() {
    // The default fetcher value is a live network fetcher; constructing it and
    // the type-check below prove it satisfies the `CatalogueFetcher` seam.
    let fetcher: Box<dyn ragent_connectors::CatalogueFetcher> =
        ragent_connectors::default_fetcher();
    let _ = fetcher;
    fn assert_fetcher<T: ragent_connectors::CatalogueFetcher>() {}
    assert_fetcher::<NetworkCatalogueFetcher>();
}

// ── Live-shape verification (opportunistic golden test) ─────────────────────

/// If the recorded Claude connector-directory feed is present under
/// `target/temp/` (produced by a manual `curl` during development), parse it to
/// confirm the provider understands the real production shape end to end. The
/// test is a no-op when the file is absent, so it never depends on the network.
#[test]
fn parses_recorded_claude_feed_when_present() {
    let path =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../target/temp/claude-connectors.json");
    let Ok(bytes) = std::fs::read(&path) else {
        return;
    };
    let origin = url::Url::parse("https://api.anthropic.com/api/directory/servers").expect("url");
    let provider = provider_for(CatalogueKind::Claude);
    let parsed = parse_catalogue(&*provider, &bytes, &origin).expect("the real feed parses");
    // The real feed lists ~100 connectors per page; the vast majority normalise.
    assert!(
        parsed.connectors.len() >= 50,
        "expected many connectors from the real feed, got {}",
        parsed.connectors.len()
    );
    // A handful of entries are legitimately inexpressible (a URL that is only
    // supplied by the user at connect time, or a server id that is a hostname
    // with a path); they are skipped and counted, never fatal (FR-025).
    assert!(
        parsed.skipped < 10,
        "only a minority of real-feed entries should be inexpressible, got {}",
        parsed.skipped
    );
    // The feed's remote transport is `streamable-http`, which must normalise to
    // the `http` transport ragent speaks.
    let with_http = parsed
        .connectors
        .iter()
        .filter(|c| c.servers.iter().any(|s| s.transport == "http"))
        .count();
    assert!(
        with_http > 0,
        "at least one connector uses the normalised http transport"
    );
    // No connector advertises more than one server from this feed, and every
    // server carries a usable id.
    for connector in &parsed.connectors {
        assert!(!connector.servers.is_empty());
        for server in &connector.servers {
            assert!(!server.id.trim().is_empty(), "every server has an id");
        }
    }
}
