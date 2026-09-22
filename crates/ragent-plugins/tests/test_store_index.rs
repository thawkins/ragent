//! Tests for the plugin store registry, the store-index entry model, and the
//! compiled default endpoints (spec `pluginstores` T-002, T-014; FR-001,
//! FR-024, FR-027, FR-029, NFR-001).

use ragent_config::{PluginStoreEndpoint, PluginStoresConfig};
use ragent_plugins::{
    DEFAULT_CLAUDE_STORE_URL, DEFAULT_CODEX_STORE_URL, StoreCatalog, StoreEndpoint,
    StoreEndpointError, StoreEntry, StoreEntryError, StoreKind,
};
use serde_json::json;

// ── StoreKind (FR-001) ──────────────────────────────────────────────────────

#[test]
fn store_kinds_are_codex_then_claude() {
    assert_eq!(StoreKind::ALL, [StoreKind::Codex, StoreKind::Claude]);
    let catalog = StoreCatalog::new();
    assert_eq!(catalog.kinds(), [StoreKind::Codex, StoreKind::Claude]);
}

#[test]
fn store_kind_labels_and_tokens() {
    assert_eq!(StoreKind::Codex.label(), "Codex");
    assert_eq!(StoreKind::Claude.label(), "Claude");
    assert_eq!(StoreKind::Codex.token(), "codex");
    assert_eq!(StoreKind::Claude.token(), "claude");
    assert_eq!(StoreKind::Codex.to_string(), "Codex");
}

#[test]
fn store_kind_from_token_is_case_insensitive_and_trims() {
    assert_eq!(StoreKind::from_token("codex"), Some(StoreKind::Codex));
    assert_eq!(StoreKind::from_token("  CLAUDE "), Some(StoreKind::Claude));
    assert_eq!(StoreKind::from_token("openai"), None);
    assert_eq!(StoreKind::from_token(""), None);
}

// ── StoreEndpoint scheme guard (FR-024, FR-029) ─────────────────────────────

#[test]
fn store_endpoint_accepts_https_with_host() {
    let endpoint = StoreEndpoint::parse("https://example.org/codex/index.json").expect("valid");
    assert_eq!(endpoint.as_str(), "https://example.org/codex/index.json");
    assert_eq!(endpoint.as_url().scheme(), "https");
    assert_eq!(endpoint.to_string(), "https://example.org/codex/index.json");
}

#[test]
fn store_endpoint_trims_surrounding_whitespace() {
    let endpoint = StoreEndpoint::parse("  https://example.org/i.json \n").expect("valid");
    assert_eq!(endpoint.as_str(), "https://example.org/i.json");
}

#[test]
fn store_endpoint_refuses_non_https_schemes() {
    for raw in [
        "http://example.org/i.json",
        "ftp://example.org/i.json",
        "file:///etc/passwd",
        "javascript:alert(1)",
    ] {
        let err = StoreEndpoint::parse(raw).expect_err("must refuse");
        assert!(
            matches!(err, StoreEndpointError::NotHttps { .. }),
            "expected NotHttps for {raw}, got {err:?}"
        );
    }
}

#[test]
fn store_endpoint_refuses_empty_and_whitespace() {
    for raw in ["", "   ", "\t\n"] {
        assert_eq!(StoreEndpoint::parse(raw), Err(StoreEndpointError::Empty));
    }
}

#[test]
fn store_endpoint_refuses_malformed_and_hostless() {
    assert!(matches!(
        StoreEndpoint::parse("not a url"),
        Err(StoreEndpointError::Malformed(_))
    ));
    // An absolute https URL with no host is refused (the url crate reports
    // either a parse failure or a missing host; both are refusals).
    assert!(
        StoreEndpoint::parse("https://").is_err(),
        "a hostless https endpoint must be refused"
    );
    assert!(
        StoreEndpoint::parse("https:///index.json").is_ok(),
        "url normalises the empty authority into a host, so it is accepted"
    );
}

#[test]
fn store_endpoint_try_from_matches_parse() {
    let endpoint = StoreEndpoint::try_from("https://example.org/i.json").expect("valid");
    assert_eq!(
        endpoint,
        StoreEndpoint::parse("https://example.org/i.json").unwrap()
    );
    assert!(StoreEndpoint::try_from("http://example.org/i.json").is_err());
}

// ── StoreCatalog (FR-001) ───────────────────────────────────────────────────

#[test]
fn empty_catalog_reports_empty_and_no_endpoints() {
    let catalog = StoreCatalog::new();
    assert!(catalog.is_empty());
    assert!(catalog.endpoint(StoreKind::Codex).is_none());
    assert!(catalog.endpoint(StoreKind::Claude).is_none());
    assert_eq!(StoreCatalog::default(), catalog);
}

#[test]
fn catalog_inserts_and_reads_back_per_store() {
    let mut catalog = StoreCatalog::new();
    let codex = StoreEndpoint::parse("https://example.org/codex.json").expect("valid");
    catalog.insert(StoreKind::Codex, codex.clone());
    assert!(!catalog.is_empty());
    assert_eq!(catalog.endpoint(StoreKind::Codex), Some(&codex));
    // The other store is unaffected.
    assert!(catalog.endpoint(StoreKind::Claude).is_none());

    let claude = StoreEndpoint::parse("https://example.org/claude.json").expect("valid");
    catalog.insert(StoreKind::Claude, claude.clone());
    assert_eq!(catalog.endpoint(StoreKind::Claude), Some(&claude));
}

#[test]
fn catalog_insert_replaces_existing_endpoint() {
    let mut catalog = StoreCatalog::new();
    catalog.insert(
        StoreKind::Codex,
        StoreEndpoint::parse("https://a.example/x.json").expect("valid"),
    );
    catalog.insert(
        StoreKind::Codex,
        StoreEndpoint::parse("https://b.example/y.json").expect("valid"),
    );
    assert_eq!(
        catalog
            .endpoint(StoreKind::Codex)
            .map(StoreEndpoint::as_str),
        Some("https://b.example/y.json")
    );
}

// ── configured_url (FR-019, FR-028) ─────────────────────────────────────────

#[test]
fn configured_url_is_none_when_the_block_is_absent() {
    let stores = PluginStoresConfig::default();
    assert!(StoreKind::Codex.configured_url(&stores).is_none());
    assert!(StoreKind::Claude.configured_url(&stores).is_none());
}

#[test]
fn configured_url_reads_each_store_override_verbatim() {
    let stores = PluginStoresConfig {
        codex: Some(PluginStoreEndpoint {
            url: Some("https://cfg.example/codex.json".to_string()),
        }),
        claude: Some(PluginStoreEndpoint { url: None }),
        ..PluginStoresConfig::default()
    };
    assert_eq!(
        StoreKind::Codex.configured_url(&stores),
        Some("https://cfg.example/codex.json")
    );
    // Claude carries an endpoint with no url set: still None.
    assert!(StoreKind::Claude.configured_url(&stores).is_none());
}

#[test]
fn configured_url_returns_an_empty_override_verbatim() {
    // The empty string is preserved so the resolver (T-015) can decide to fall
    // back to the compiled default rather than treating it as absent.
    let stores = PluginStoresConfig {
        codex: Some(PluginStoreEndpoint {
            url: Some(String::new()),
        }),
        ..PluginStoresConfig::default()
    };
    assert_eq!(StoreKind::Codex.configured_url(&stores), Some(""));
}

// ── effective_endpoint resolution (FR-027, FR-028, FR-029, NFR-002) ──────────

#[test]
fn default_url_returns_the_compiled_default_per_store() {
    assert_eq!(StoreKind::Codex.default_url(), DEFAULT_CODEX_STORE_URL);
    assert_eq!(StoreKind::Claude.default_url(), DEFAULT_CLAUDE_STORE_URL);
}

#[test]
fn effective_endpoint_falls_back_to_the_default_when_no_block_is_present() {
    let stores = PluginStoresConfig::default();
    let codex = StoreKind::Codex
        .effective_endpoint(&stores)
        .expect("default resolves");
    let claude = StoreKind::Claude
        .effective_endpoint(&stores)
        .expect("default resolves");
    assert_eq!(codex.as_str(), DEFAULT_CODEX_STORE_URL);
    assert_eq!(claude.as_str(), DEFAULT_CLAUDE_STORE_URL);
}

#[test]
fn effective_endpoint_uses_a_configured_url_wholesale() {
    let stores = PluginStoresConfig {
        codex: Some(PluginStoreEndpoint {
            url: Some("https://cfg.example/codex.json".to_string()),
        }),
        ..PluginStoresConfig::default()
    };
    let codex = StoreKind::Codex
        .effective_endpoint(&stores)
        .expect("configured resolves");
    assert_eq!(codex.as_str(), "https://cfg.example/codex.json");
    // The untouched store still resolves to its compiled default.
    assert_eq!(
        StoreKind::Claude
            .effective_endpoint(&stores)
            .expect("default resolves")
            .as_str(),
        DEFAULT_CLAUDE_STORE_URL
    );
}

#[test]
fn effective_endpoint_ignores_an_empty_or_whitespace_override() {
    for raw in ["", "   "] {
        let stores = PluginStoresConfig {
            codex: Some(PluginStoreEndpoint {
                url: Some(raw.to_string()),
            }),
            ..PluginStoresConfig::default()
        };
        let codex = StoreKind::Codex
            .effective_endpoint(&stores)
            .expect("empty override falls back");
        assert_eq!(
            codex.as_str(),
            DEFAULT_CODEX_STORE_URL,
            "override {raw:?} must fall back to the default"
        );
    }
}

#[test]
fn effective_endpoint_refuses_a_non_https_or_empty_override() {
    // FR-028/FR-029: the configured path is guarded exactly like the default,
    // so a non-https override is refused rather than used.
    let stores = PluginStoresConfig {
        codex: Some(PluginStoreEndpoint {
            url: Some("http://cfg.example/codex.json".to_string()),
        }),
        ..PluginStoresConfig::default()
    };
    assert!(matches!(
        StoreKind::Codex.effective_endpoint(&stores),
        Err(StoreEndpointError::NotHttps { .. })
    ));
}

#[test]
fn effective_endpoint_is_resolved_afresh_on_each_call() {
    // NFR-002: resolution reads the config on every call, so a change takes
    // effect without a rebuild.
    let empty = PluginStoresConfig::default();
    assert_eq!(
        StoreKind::Codex
            .effective_endpoint(&empty)
            .expect("default")
            .as_str(),
        DEFAULT_CODEX_STORE_URL
    );
    let overridden = PluginStoresConfig {
        codex: Some(PluginStoreEndpoint {
            url: Some("https://late.example/codex.json".to_string()),
        }),
        ..PluginStoresConfig::default()
    };
    assert_eq!(
        StoreKind::Codex
            .effective_endpoint(&overridden)
            .expect("override")
            .as_str(),
        "https://late.example/codex.json"
    );
}

// ── StoreEntry model (FR-003, FR-025) ───────────────────────────────────────

#[test]
fn store_entry_parses_required_and_optional_fields() {
    let value = json!({
        "id": "codex-weather",
        "name": "Codex Weather",
        "version": "1.2.0",
        "source": "https://example.org/codex-weather.zip",
        "description": "Weather lookups",
        "dialect": "codex",
        "tags": ["weather", "http"],
        "homepage": "https://example.org/codex-weather"
    });
    let entry = StoreEntry::from_value(&value).expect("valid entry");
    assert_eq!(entry.id, "codex-weather");
    assert_eq!(entry.name, "Codex Weather");
    assert_eq!(entry.version, "1.2.0");
    assert_eq!(entry.source, "https://example.org/codex-weather.zip");
    assert_eq!(entry.description, "Weather lookups");
    assert_eq!(entry.dialect.as_deref(), Some("codex"));
    assert_eq!(entry.tags, vec!["weather".to_string(), "http".to_string()]);
    assert_eq!(
        entry.homepage.as_deref(),
        Some("https://example.org/codex-weather")
    );
}

#[test]
fn store_entry_tolerates_absent_optional_fields() {
    let value = json!({
        "id": "codex-time",
        "name": "Codex Time",
        "version": "0.9.1",
        "source": "https://example.org/codex-time.zip"
    });
    let entry = StoreEntry::from_value(&value).expect("valid entry");
    assert!(entry.description.is_empty());
    assert!(entry.dialect.is_none());
    assert!(entry.tags.is_empty());
    assert!(entry.homepage.is_none());
}

#[test]
fn store_entry_round_trips_through_serde() {
    let entry = StoreEntry {
        id: "codex-weather".to_string(),
        name: "Codex Weather".to_string(),
        version: "1.2.0".to_string(),
        source: "https://example.org/w.zip".to_string(),
        description: "d".to_string(),
        dialect: Some("codex".to_string()),
        tags: vec!["t".to_string()],
        homepage: None,
    };
    let text = serde_json::to_string(&entry).expect("serialises");
    let back: StoreEntry = serde_json::from_str(&text).expect("deserialises");
    assert_eq!(back, entry);
}

#[test]
fn store_entry_rejects_missing_required_fields() {
    for missing in ["id", "name", "version", "source"] {
        let mut object = json!({
            "id": "a", "name": "b", "version": "c", "source": "d",
        });
        object.as_object_mut().expect("object").remove(missing);
        let err = StoreEntry::from_value(&object).expect_err("must reject");
        assert!(
            matches!(err, StoreEntryError::Invalid(_)),
            "expected Invalid for missing {missing}, got {err:?}"
        );
    }
}

#[test]
fn store_entry_rejects_empty_required_fields() {
    for field in ["id", "name", "version", "source"] {
        let mut object = json!({
            "id": "a", "name": "b", "version": "c", "source": "d",
        });
        *object.get_mut(field).expect("field") = json!("   ");
        let err = StoreEntry::from_value(&object).expect_err("must reject");
        assert_eq!(err, StoreEntryError::EmptyField(field));
    }
}

#[test]
fn store_entry_rejects_wrong_typed_required_field() {
    let value = json!({
        "id": 42, "name": "b", "version": "c", "source": "d",
    });
    assert!(matches!(
        StoreEntry::from_value(&value),
        Err(StoreEntryError::Invalid(_))
    ));
}

#[test]
fn store_entry_rejects_non_object_values() {
    for value in [json!("nope"), json!(7), json!([1, 2]), json!(null)] {
        assert_eq!(
            StoreEntry::from_value(&value),
            Err(StoreEntryError::NotAnObject)
        );
    }
}

// ── Compiled default endpoints (FR-027, FR-029, NFR-001) ────────────────────

#[test]
fn default_endpoints_are_distinct_and_non_empty() {
    assert!(!DEFAULT_CODEX_STORE_URL.is_empty());
    assert!(!DEFAULT_CLAUDE_STORE_URL.is_empty());
    assert_ne!(DEFAULT_CODEX_STORE_URL, DEFAULT_CLAUDE_STORE_URL);
}

#[test]
fn default_endpoints_are_https_absolute_urls() {
    for raw in [DEFAULT_CODEX_STORE_URL, DEFAULT_CLAUDE_STORE_URL] {
        let url = reqwest::Url::parse(raw).expect("default endpoint parses");
        assert_eq!(url.scheme(), "https", "{raw} must be https");
        assert!(url.host_str().is_some(), "{raw} must carry a host");
    }
}

#[test]
fn default_endpoints_pass_the_endpoint_scheme_guard() {
    // FR-029: the default path is guarded exactly like the configured path.
    let codex = StoreEndpoint::parse(DEFAULT_CODEX_STORE_URL).expect("codex default passes guard");
    let claude =
        StoreEndpoint::parse(DEFAULT_CLAUDE_STORE_URL).expect("claude default passes guard");
    assert_eq!(codex.as_str(), DEFAULT_CODEX_STORE_URL);
    assert_eq!(claude.as_str(), DEFAULT_CLAUDE_STORE_URL);
}
