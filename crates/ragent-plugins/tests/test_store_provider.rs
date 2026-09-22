//! Tests for the per-store index providers ([`StoreProvider`]): the Codex and
//! Claude marketplace transforms that normalise a vendor catalogue into the
//! internal store-entry model (spec `pluginstores`; FR-003, FR-027, FR-029).
//!
//! The fixtures here mirror the real vendor document shapes (captured from
//! `openai/plugins` and `anthropics/claude-plugins-official`) so the transform is
//! proven against the actual schemas: `name`-as-id, optional `version`, and a
//! `source` that is a repo-relative string or a `local`/`url`/`git-subdir`
//! object. Everything is offline — the bytes are inline and no store is touched.

use ragent_plugins::{
    DEFAULT_CLAUDE_STORE_URL, DEFAULT_CODEX_STORE_URL, FixtureStoreFetcher, StoreEndpoint,
    StoreIndexFetcher, StoreKind, provider_for,
};

/// The parsed origin URL of a store's compiled default endpoint.
fn origin(url: &str) -> reqwest::Url {
    StoreEndpoint::parse(url)
        .expect("the compiled default is a valid endpoint")
        .as_url()
        .clone()
}

/// A realistic Codex (`openai/plugins`) marketplace document: a mix of `local`,
/// `url`, and `git-subdir` sources plus one entry with a `products` policy array.
const CODEX_MARKETPLACE: &[u8] = br#"{
  "name": "openai-curated",
  "interface": { "displayName": "Codex official" },
  "plugins": [
    {
      "name": "linear",
      "source": { "source": "local", "path": "./plugins/linear" },
      "policy": { "installation": "AVAILABLE" },
      "category": "Productivity"
    },
    {
      "name": "crowdstrike-falcon-foundry",
      "source": { "source": "url", "url": "https://github.com/CrowdStrike/foundry-skills.git" },
      "policy": { "installation": "AVAILABLE" }
    },
    {
      "name": "amazon-location-service",
      "source": {
        "source": "git-subdir",
        "url": "https://github.com/awslabs/agent-plugins.git",
        "path": "plugins/amazon-location-service",
        "ref": "main",
        "sha": "9898ddc4"
      }
    },
    {
      "name": "no-source-plugin",
      "policy": { "installation": "AVAILABLE" }
    }
  ]
}"#;

/// A realistic Claude (`claude-plugins-official`) marketplace document.
const CLAUDE_MARKETPLACE: &[u8] = br#"{
  "$schema": "https://anthropic.com/claude-code/marketplace.schema.json",
  "name": "claude-plugins-official",
  "owner": { "name": "Anthropic" },
  "plugins": [
    {
      "name": "agent-sdk-dev",
      "description": "Development kit for the Claude Agent SDK",
      "source": "./plugins/agent-sdk-dev",
      "category": "development"
    },
    {
      "name": "42crunch-api-security-testing",
      "description": "Automate API security",
      "category": "security",
      "source": {
        "source": "git-subdir",
        "url": "https://github.com/42Crunch-AI/claude-plugins.git",
        "path": "plugins/api-security-testing",
        "ref": "v1.5.5",
        "sha": "30287f5e"
      },
      "homepage": "https://42crunch.com"
    },
    {
      "name": "clangd-lsp",
      "version": "1.0.0",
      "source": "./plugins/clangd-lsp",
      "category": "development"
    },
    {
      "name": "tls-only-plugin",
      "source": { "source": "url", "url": "git@github.com:private/repo.git" }
    }
  ]
}"#;

/// The provider for a store reports its own kind.
#[test]
fn provider_for_reports_each_stores_kind() {
    assert_eq!(provider_for(StoreKind::Codex).kind(), StoreKind::Codex);
    assert_eq!(provider_for(StoreKind::Claude).kind(), StoreKind::Claude);
}

// ── Codex transform ─────────────────────────────────────────────────────────

#[test]
fn codex_provider_normalises_the_vendor_marketplace() {
    let provider = provider_for(StoreKind::Codex);
    let index = provider
        .parse_index(CODEX_MARKETPLACE, &origin(DEFAULT_CODEX_STORE_URL))
        .expect("the codex marketplace parses");

    // Three entries carry a resolvable source; the fourth has none and is skipped.
    assert_eq!(index.entries.len(), 3, "three installable entries");
    assert_eq!(index.skipped, 1, "the source-less entry is skipped");

    let linear = index
        .entries
        .iter()
        .find(|e| e.id == "linear")
        .expect("linear entry present");
    // The id is the entry `name`; the version defaults when the vendor omits it.
    assert_eq!(linear.name, "linear");
    assert_eq!(linear.version, "0");
    assert_eq!(linear.dialect.as_deref(), Some("codex"));
    // A `local` path resolves against the marketplace repository root as an
    // installable git source (a bare `https` directory URL is not installable).
    assert_eq!(
        linear.source,
        "git+https://github.com/openai/plugins#main:plugins/linear"
    );

    let crowdstrike = index
        .entries
        .iter()
        .find(|e| e.id == "crowdstrike-falcon-foundry")
        .expect("crowdstrike entry present");
    // A whole-repo `url` source becomes a git source at HEAD, sans `.git`.
    assert_eq!(
        crowdstrike.source,
        "git+https://github.com/CrowdStrike/foundry-skills#HEAD"
    );

    let amazon = index
        .entries
        .iter()
        .find(|e| e.id == "amazon-location-service")
        .expect("amazon entry present");
    // A `git-subdir` source becomes `git+<url>#<ref>:<path>`.
    assert_eq!(
        amazon.source,
        "git+https://github.com/awslabs/agent-plugins#main:plugins/amazon-location-service"
    );
}

// ── Claude transform ────────────────────────────────────────────────────────

#[test]
fn claude_provider_normalises_the_vendor_marketplace() {
    let provider = provider_for(StoreKind::Claude);
    let index = provider
        .parse_index(CLAUDE_MARKETPLACE, &origin(DEFAULT_CLAUDE_STORE_URL))
        .expect("the claude marketplace parses");

    assert_eq!(index.entries.len(), 3, "three installable entries");
    assert_eq!(index.skipped, 1, "the ssh-remote entry is skipped");

    let sdk = index
        .entries
        .iter()
        .find(|e| e.id == "agent-sdk-dev")
        .expect("agent-sdk-dev entry present");
    // A repo-relative string source resolves against the marketplace repo root.
    assert_eq!(
        sdk.source,
        "git+https://github.com/anthropics/claude-plugins-official#main:plugins/agent-sdk-dev"
    );
    assert_eq!(sdk.description, "Development kit for the Claude Agent SDK");
    assert_eq!(sdk.dialect.as_deref(), Some("claude"));

    let security = index
        .entries
        .iter()
        .find(|e| e.id == "42crunch-api-security-testing")
        .expect("security entry present");
    assert_eq!(
        security.source,
        "git+https://github.com/42Crunch-AI/claude-plugins#v1.5.5:plugins/api-security-testing"
    );
    assert_eq!(security.homepage.as_deref(), Some("https://42crunch.com"));

    // A declared version is preserved verbatim.
    let clangd = index
        .entries
        .iter()
        .find(|e| e.id == "clangd-lsp")
        .expect("clangd entry present");
    assert_eq!(clangd.version, "1.0.0");
}

// ── Native documents stay strict ────────────────────────────────────────────

#[test]
fn a_native_index_is_parsed_through_the_strict_path() {
    // Any entry carrying an `id` marks the document as a native ragent index, so
    // the strict validator runs and a malformed native entry is still skipped
    // exactly as before (FR-003).
    let native = br#"{"store":"codex","plugins":[
        {"id":"a","name":"A","version":"1","source":"https://x/a.zip"},
        {"name":"no id","version":"1","source":"https://x/b.zip"}
    ]}"#;
    let index = provider_for(StoreKind::Codex)
        .parse_index(native, &origin(DEFAULT_CODEX_STORE_URL))
        .expect("native index parses");
    assert_eq!(index.store.as_deref(), Some("codex"));
    assert_eq!(index.entries.len(), 1);
    assert_eq!(index.entries[0].id, "a");
    assert_eq!(index.skipped, 1);
}

// ── Containment: malformed documents and unusable sources ───────────────────

#[test]
fn a_document_without_a_plugins_array_is_a_shape_error() {
    let err = provider_for(StoreKind::Claude)
        .parse_index(
            br#"{"name":"claude-plugins-official"}"#,
            &origin(DEFAULT_CLAUDE_STORE_URL),
        )
        .expect_err("no plugins array is a shape error");
    assert!(
        matches!(err, ragent_plugins::StoreError::MalformedShape { .. }),
        "expected MalformedShape, got {err:?}"
    );
}

#[test]
fn malformed_json_is_a_contained_error() {
    let err = provider_for(StoreKind::Codex)
        .parse_index(b"{ not json", &origin(DEFAULT_CODEX_STORE_URL))
        .expect_err("malformed JSON is refused");
    assert!(matches!(
        err,
        ragent_plugins::StoreError::MalformedJson { .. }
    ));
}

#[test]
fn entries_with_unusable_sources_are_skipped_not_fatal() {
    let doc = br#"{"plugins":[
        {"name":"ftp","source":{"source":"url","url":"ftp://example.org/x.git"}},
        {"name":"empty","source":"   "},
        {"name":"ssh","source":"git@github.com:private/repo.git"},
        {"name":"good","source":"./plugins/good"}
    ]}"#;
    let index = provider_for(StoreKind::Claude)
        .parse_index(doc, &origin(DEFAULT_CLAUDE_STORE_URL))
        .expect("the document still parses");
    assert_eq!(index.entries.len(), 1, "only the resolvable entry survives");
    assert_eq!(index.entries[0].id, "good");
    assert_eq!(index.skipped, 3);
}

// ── The fixture seam routes through the provider ────────────────────────────

#[test]
fn the_fixture_seam_normalises_a_vendor_shaped_index() {
    // A fixture with no `id` fields is a vendor document; the fixture fetcher
    // must route it through the store's provider so it is still normalised.
    let fetcher = FixtureStoreFetcher::new().with_index(DEFAULT_CODEX_STORE_URL, CODEX_MARKETPLACE);
    let endpoint = StoreEndpoint::parse(DEFAULT_CODEX_STORE_URL).expect("valid endpoint");
    let index = fetcher
        .fetch_index(
            StoreKind::Codex,
            &endpoint,
            &ragent_plugins::FetchLimits::default(),
        )
        .expect("the vendor fixture is served and normalised");
    assert_eq!(index.entries.len(), 3);
    assert!(index.entries.iter().any(|e| e.id == "linear"));
}

#[test]
fn the_fixture_seam_keeps_serving_the_native_default_fixture() {
    // The bundled default fixtures are native (they carry `id`s), so the seam
    // still parses them strictly and both stores still populate (FR-032).
    let fetcher = FixtureStoreFetcher::new().with_default_endpoints();
    for kind in StoreKind::ALL {
        let endpoint = kind
            .effective_endpoint(&ragent_config::PluginStoresConfig::default())
            .expect("compiled default is valid");
        let index = fetcher
            .fetch_index(kind, &endpoint, &ragent_plugins::FetchLimits::default())
            .unwrap_or_else(|e| panic!("{} default served: {e}", kind.token()));
        assert!(
            !index.entries.is_empty(),
            "{} default carries entries",
            kind.token()
        );
    }
}

// ── Repo-relative anchoring requires a GitHub origin ─────────────────────────

#[test]
fn repo_relative_dirs_resolve_from_a_github_tree_origin() {
    // A marketplace served from a GitHub HTML URL (e.g. a manually added store)
    // still anchors repo-relative entries at the repository root.
    let doc =
        br#"{"plugins":[{"name":"security-guidance","source":"./plugins/security-guidance"}]}"#;
    // A bare repository URL carries no ref, so the entry installs at HEAD.
    let origin = reqwest::Url::parse("https://github.com/anthropics/claude-plugins-official")
        .expect("a valid github url");
    let index = provider_for(StoreKind::Claude)
        .parse_index(doc, &origin)
        .expect("the document parses");
    assert_eq!(index.entries.len(), 1);
    assert_eq!(
        index.entries[0].source,
        "git+https://github.com/anthropics/claude-plugins-official#HEAD:plugins/security-guidance"
    );

    let tree_origin = reqwest::Url::parse(
        "https://github.com/anthropics/claude-plugins-official/tree/main/.claude-plugin",
    )
    .expect("a valid github tree url");
    let index = provider_for(StoreKind::Claude)
        .parse_index(doc, &tree_origin)
        .expect("the document parses");
    assert_eq!(
        index.entries[0].source,
        "git+https://github.com/anthropics/claude-plugins-official#main:plugins/security-guidance"
    );
}

#[test]
fn repo_relative_sources_are_skipped_without_a_github_origin() {
    // A directory listing with no GitHub repository behind it cannot host a
    // repo-relative entry: the entry is skipped rather than pointed at a
    // non-existent `https` directory the install pipeline rejects.
    let doc = br#"{"plugins":[{"name":"x","source":"./plugins/x"}]}"#;
    let origin = reqwest::Url::parse("https://example.com/marketplace/").expect("a valid url");
    let index = provider_for(StoreKind::Claude)
        .parse_index(doc, &origin)
        .expect("the document still parses");
    assert!(index.entries.is_empty(), "no installable entry survives");
    assert_eq!(index.skipped, 1);
}
