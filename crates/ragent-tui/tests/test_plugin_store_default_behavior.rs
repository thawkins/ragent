//! Behaviour tests for the store browser driven by a store index served from a
//! compiled default endpoint (spec `pluginstores` T-020; FR-032, FR-034, FR-035,
//! NFR-004).
//!
//! Each test replaces the app's production HTTPS fetcher with an offline
//! [`FixtureStoreFetcher`], launches the off-loop fetch with no
//! `plugins.stores` block (so the effective endpoint is the compiled default),
//! and drains the delivery slot exactly as the event loop does. The fetched
//! entries then drive the browser's result set, in-memory filtering, block-cursor
//! movement, and the `ENTER` install action.
//!
//! Every test is deterministic and offline: no live store is contacted, no plugin
//! is downloaded, and the only install (in the `ENTER` test) copies a local
//! fixture directory under `target/temp/` through the real `add` path (NFR-003).
//! Both stores are exercised explicitly so neither compiled default is left
//! unverified (NFR-004).

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use serde_json::json;

use ragent_config::{PluginStoreEndpoint, PluginStoresConfig};
use ragent_plugins::{
    DEFAULT_CLAUDE_STORE_URL, DEFAULT_CODEX_STORE_URL, FixtureStoreFetcher, StoreEndpoint,
    StoreKind,
};
use ragent_tui::App;
use ragent_tui::app::{PluginStoreBrowser, PluginStoreStatus};
use ragent_tui::input::handle_key;

#[path = "support/mod.rs"]
mod support;

static TEMP_SEQ: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);

/// A unique scratch directory under `target/temp/` (no `/tmp`, per AGENTS.md).
fn temp_dir(name: &str) -> PathBuf {
    let unique = TEMP_SEQ.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(format!(
        "../../target/temp/plugin-store-default/{name}-{}-{unique}",
        std::process::id()
    ));
    std::fs::create_dir_all(&path).expect("temp dir creatable");
    path
}

fn key(code: KeyCode) -> KeyEvent {
    KeyEvent::new(code, KeyModifiers::NONE)
}

/// The compiled default endpoint for `kind`, resolved with no `plugins.stores`
/// block present (FR-033).
fn default_endpoint(kind: StoreKind) -> StoreEndpoint {
    kind.effective_endpoint(&PluginStoresConfig::default())
        .expect("the compiled default is a valid endpoint")
}

/// An app whose fetch seam is `fetcher` and whose project store is `cwd`
/// (when supplied), with the browse panel for `kind` open and no fetch started.
fn app_with_seam(kind: StoreKind, fetcher: FixtureStoreFetcher, cwd: Option<PathBuf>) -> App {
    let mut app = support::make_app();
    app.set_plugin_store_fetcher(Arc::new(fetcher));
    if let Some(cwd) = cwd {
        app.cwd_path = cwd;
    }
    app.plugin_store = Some(PluginStoreBrowser::new(kind, "", false));
    app
}

/// An app whose seam serves exactly the compiled-default fixture for both stores.
fn default_fixture_app(kind: StoreKind) -> App {
    app_with_seam(
        kind,
        FixtureStoreFetcher::new().with_default_endpoints(),
        None,
    )
}

/// Drive the off-loop fetch for `kind` against `stores`, then apply the result.
async fn drive_fetch(app: &mut App, kind: StoreKind, stores: &PluginStoresConfig) {
    app.spawn_plugin_store_fetch_with_stores(kind, stores);

    let mut delivered = false;
    for _ in 0..300 {
        if app
            .plugin_store_result
            .lock()
            .map(|guard| guard.is_some())
            .unwrap_or(false)
        {
            delivered = true;
            break;
        }
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
    assert!(delivered, "the off-loop fixture fetch delivered a result");

    app.poll_plugin_store_result();
}

/// Bounded wait for the off-loop install worker to deposit its result, then
/// drain it through the poll.
fn drain_install(app: &mut App) {
    let mut delivered = false;
    for _ in 0..500 {
        if app
            .plugin_store_install_result
            .lock()
            .map(|guard| guard.is_some())
            .unwrap_or(false)
        {
            delivered = true;
            break;
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    assert!(delivered, "the off-loop install delivered its result");
    app.poll_plugin_store_install_result();
}

/// Build a store-index JSON document with `entries` under the store token.
fn index_bytes(token: &str, entries: Vec<serde_json::Value>) -> Vec<u8> {
    serde_json::to_vec(&json!({ "store": token, "plugins": entries })).expect("index json")
}

/// An installed-fixture entry (id matches a fixture under `assets/plugins/`).
fn rich_index(kind: StoreKind, count: usize) -> Vec<u8> {
    let entries = (0..count)
        .map(|i| {
            json!({
                "id": format!("{}-e{i}", kind.token()),
                "name": format!("Entry {i}"),
                "version": "1.0.0",
                "source": format!("https://example.org/{}-e{i}.zip", kind.token()),
            })
        })
        .collect();
    index_bytes(kind.token(), entries)
}

/// A dialect-appropriate local plugin source directory under `root` with `id`.
fn plugin_source(root: &Path, kind: StoreKind, id: &str) -> PathBuf {
    let plugin = root.join(format!("{id}-src"));
    let manifest = match kind {
        StoreKind::Codex => plugin.join("codex-plugin.json"),
        StoreKind::Claude => plugin.join(".claude-plugin/plugin.json"),
    };
    std::fs::create_dir_all(manifest.parent().expect("manifest parent")).expect("plugin dir");
    std::fs::write(
        &manifest,
        format!(r#"{{ "id": "{id}", "name": "{id}", "version": "1.0.0", "entry": "index.js" }}"#),
    )
    .expect("manifest");
    std::fs::write(plugin.join("index.js"), "// entry").expect("entry");
    plugin
}

/// The entry id in the compiled-default fixture whose id matches an installed
/// fixture under `assets/plugins/fixtures/`.
fn installed_fixture_id(kind: StoreKind) -> &'static str {
    match kind {
        StoreKind::Codex => "codex-weather",
        StoreKind::Claude => "claude-todo",
    }
}

/// The entry id in the compiled-default fixture that omits every optional field.
fn tolerant_entry_id(kind: StoreKind) -> &'static str {
    match kind {
        StoreKind::Codex => "codex-time",
        StoreKind::Claude => "claude-time",
    }
}

// -- The default endpoint populates the result set (FR-033, FR-034, NFR-004) --

#[tokio::test]
async fn the_compiled_default_endpoint_populates_all_and_filtered_for_both_stores() {
    for kind in StoreKind::ALL {
        // With no `plugins.stores` block the effective endpoint is the compiled
        // default for the requested store (FR-033).
        let expected = match kind {
            StoreKind::Codex => DEFAULT_CODEX_STORE_URL,
            StoreKind::Claude => DEFAULT_CLAUDE_STORE_URL,
        };
        assert_eq!(default_endpoint(kind).as_str(), expected);

        let mut app = default_fixture_app(kind);
        drive_fetch(&mut app, kind, &PluginStoresConfig::default()).await;

        let browser = app.plugin_store.as_ref().expect("panel open");
        assert_eq!(
            browser.status,
            PluginStoreStatus::Ready,
            "{} default fixture populated the panel (FR-032)",
            kind.token()
        );
        assert!(
            !browser.all.is_empty(),
            "{} default fixture populated `all`",
            kind.token()
        );
        // An empty query matches every entry, so `all` and `filtered` agree.
        assert_eq!(
            browser.filtered.len(),
            browser.all.len(),
            "{}: every entry survives the empty-query filter (FR-034)",
            kind.token()
        );
        assert!(
            browser
                .all
                .iter()
                .any(|e| e.id == installed_fixture_id(kind)),
            "{}: the installed-fixture entry loaded (FR-034)",
            kind.token()
        );
        assert!(
            browser.all.iter().any(|e| e.id == tolerant_entry_id(kind)),
            "{}: the tolerant entry without optional fields also loaded",
            kind.token()
        );
        // Reading never installs anything (FR-022, NFR-003).
        assert!(
            app.plugin_store_install_result.lock().unwrap().is_none(),
            "{}: populating the panel installs nothing",
            kind.token()
        );
    }
}

// -- Typing filters the default-sourced entries in memory (FR-034) ------------

#[tokio::test]
async fn typing_filters_the_default_endpoint_entries_in_memory_for_both_stores() {
    for kind in StoreKind::ALL {
        let mut app = default_fixture_app(kind);
        drive_fetch(&mut app, kind, &PluginStoresConfig::default()).await;
        let total = app.plugin_store.as_ref().expect("panel").all.len();

        // A query matching exactly the installed-fixture entry narrows the set.
        let needle = match kind {
            StoreKind::Codex => "weather",
            StoreKind::Claude => "todo",
        };
        app.plugin_store
            .as_mut()
            .expect("panel")
            .set_query(needle.to_string());
        {
            let browser = app.plugin_store.as_ref().expect("panel");
            assert_eq!(
                browser.filtered.len(),
                1,
                "{}: `{needle}` matches one default-sourced entry",
                kind.token()
            );
            assert_eq!(
                browser.all[browser.filtered[0]].id,
                installed_fixture_id(kind),
                "{}: the surviving entry is the expected one",
                kind.token()
            );
            assert_eq!(browser.cursor, 0, "the cursor resets to the first survivor");
            assert!(
                browser.all.len() == total,
                "{}: filtering is in memory -- `all` is unchanged (no re-fetch)",
                kind.token()
            );
        }

        // A query that matches nothing yields an explicit empty result set.
        app.plugin_store
            .as_mut()
            .expect("panel")
            .set_query("zzzz-no-match".to_string());
        assert!(
            !app.plugin_store.as_ref().expect("panel").has_results(),
            "{}: a non-matching query leaves no results (FR-017)",
            kind.token()
        );

        // Clearing the query widens the set back to the full index.
        app.plugin_store
            .as_mut()
            .expect("panel")
            .set_query(String::new());
        assert_eq!(
            app.plugin_store.as_ref().expect("panel").filtered.len(),
            total,
            "{}: an empty query restores the full default-sourced set",
            kind.token()
        );
    }
}

// -- The block cursor moves and clamps within the entries (FR-034) ------------

#[tokio::test]
async fn the_block_cursor_moves_and_clamps_within_default_endpoint_entries_for_both_stores() {
    for kind in StoreKind::ALL {
        // A richer fixture at the compiled default endpoint gives the cursor room
        // to move and clamp.
        let fetcher = FixtureStoreFetcher::new()
            .with_default_endpoints()
            .with_index(kind.default_url(), rich_index(kind, 4));
        let mut app = app_with_seam(kind, fetcher, None);
        drive_fetch(&mut app, kind, &PluginStoresConfig::default()).await;

        assert_eq!(
            app.plugin_store.as_ref().expect("panel").filtered.len(),
            4,
            "{}: the richer default fixture loaded four entries",
            kind.token()
        );

        // Down to the last entry, then one more (clamps, no wrap).
        for _ in 0..3 {
            app.plugin_store_move_down();
        }
        assert_eq!(app.plugin_store.as_ref().expect("panel").cursor, 3);
        assert_eq!(
            app.plugin_store
                .as_ref()
                .expect("panel")
                .selected()
                .map(|e| e.id.as_str()),
            Some(format!("{}-e3", kind.token()).as_str()),
            "{}: the cursor rests on the last default-sourced entry",
            kind.token()
        );
        app.plugin_store_move_down();
        assert_eq!(
            app.plugin_store.as_ref().expect("panel").cursor,
            3,
            "{}: Down at the bottom clamps",
            kind.token()
        );

        // Up to the first entry, then one more (clamps, no wrap).
        for _ in 0..3 {
            app.plugin_store_move_up();
        }
        assert_eq!(app.plugin_store.as_ref().expect("panel").cursor, 0);
        app.plugin_store_move_up();
        assert_eq!(
            app.plugin_store.as_ref().expect("panel").cursor,
            0,
            "{}: Up at the top clamps",
            kind.token()
        );
        assert_eq!(
            app.plugin_store
                .as_ref()
                .expect("panel")
                .selected()
                .map(|e| e.id.as_str()),
            Some(format!("{}-e0", kind.token()).as_str()),
            "{}: the cursor rests on the first default-sourced entry",
            kind.token()
        );
    }
}

// -- ENTER installs the default-sourced entry (FR-034, NFR-004) ---------------

#[tokio::test]
async fn enter_installs_the_default_sourced_entry_for_both_stores() {
    for kind in StoreKind::ALL {
        let temp = temp_dir(&format!("install-{}", kind.token()));
        let id = format!("{}-local", kind.token());
        let source = plugin_source(&temp, kind, &id);

        // The compiled default endpoint serves an entry whose source is the
        // local fixture directory, so ENTER installs offline through the real
        // `add` path (NFR-003).
        let entries = vec![
            json!({
                "id": format!("{}-remote", kind.token()),
                "name": "Remote",
                "version": "1.0.0",
                "source": "https://example.org/remote.zip",
            }),
            json!({
                "id": id,
                "name": "Local",
                "version": "1.0.0",
                "source": source.to_string_lossy(),
            }),
        ];
        let fetcher = FixtureStoreFetcher::new()
            .with_default_endpoints()
            .with_index(kind.default_url(), index_bytes(kind.token(), entries));

        let mut app = app_with_seam(kind, fetcher, Some(temp.clone()));
        app.session_id = Some("s1".to_string());
        drive_fetch(&mut app, kind, &PluginStoresConfig::default()).await;

        // Move the block cursor onto the default-sourced local entry and install
        // it with ENTER.
        app.plugin_store_move_down();
        assert_eq!(
            app.plugin_store
                .as_ref()
                .expect("panel")
                .selected()
                .map(|e| e.id.clone()),
            Some(id.clone()),
            "{}: the cursor is on the default-sourced entry",
            kind.token()
        );

        let action = handle_key(&mut app, key(KeyCode::Enter));
        assert!(action.is_none(), "ENTER is routed to the panel");

        drain_install(&mut app);

        let browser = app.plugin_store.as_ref().expect("panel open");
        assert!(
            browser.is_installed(&id),
            "{}: the installed entry joins the installed set so the row re-colours (FR-011)",
            kind.token()
        );
        let report = app
            .messages
            .last()
            .map(|m| m.text_content())
            .unwrap_or_default();
        assert!(
            report.contains(&format!("Installed plugin `{id}`")),
            "{}: the message window reports the installed id (FR-006), got: {report:?}",
            kind.token()
        );
        assert!(
            temp.join(".ragent/plugins").join(&id).is_dir(),
            "{}: the entry was installed into the project store",
            kind.token()
        );
    }
}

// -- A configured URL is the endpoint the browser fetches (FR-035, NFR-004) ---

#[tokio::test]
async fn a_configured_url_is_the_endpoint_the_browser_fetches_for_both_stores() {
    for kind in StoreKind::ALL {
        let configured = format!("https://cfg.example/{}/index.json", kind.token());
        let configured_id = format!("{}-cfg", kind.token());
        let configured_bytes = index_bytes(
            kind.token(),
            vec![json!({
                "id": configured_id,
                "name": "Configured",
                "version": "1.0.0",
                "source": "https://cfg.example/plugin.zip",
            })],
        );
        // The seam serves BOTH the compiled default (its own entries) and the
        // configured endpoint (a distinct entry), so the test can prove which
        // one the browser fetched.
        let fetcher = FixtureStoreFetcher::new()
            .with_default_endpoints()
            .with_index(&configured, configured_bytes);

        let stores = PluginStoresConfig {
            codex: (kind == StoreKind::Codex).then(|| PluginStoreEndpoint {
                url: Some(configured.clone()),
            }),
            claude: (kind == StoreKind::Claude).then(|| PluginStoreEndpoint {
                url: Some(configured.clone()),
            }),
            ..PluginStoresConfig::default()
        };
        assert_eq!(
            kind.effective_endpoint(&stores)
                .expect("configured https endpoint accepted")
                .as_str(),
            configured,
            "{}: the configured URL is the effective endpoint (FR-035)",
            kind.token()
        );

        let mut app = app_with_seam(kind, fetcher, None);
        drive_fetch(&mut app, kind, &stores).await;

        let browser = app.plugin_store.as_ref().expect("panel open");
        assert_eq!(browser.status, PluginStoreStatus::Ready);
        assert!(
            browser.all.iter().any(|e| e.id == configured_id),
            "{}: the browser fetched the configured endpoint's entries (FR-035)",
            kind.token()
        );
        assert!(
            !browser
                .all
                .iter()
                .any(|e| e.id == installed_fixture_id(kind)),
            "{}: the compiled-default entries were NOT fetched -- the override won wholesale",
            kind.token()
        );
    }
}
