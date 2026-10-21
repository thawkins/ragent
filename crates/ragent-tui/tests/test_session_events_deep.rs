//! Deep coverage for `src/lib.rs`, `src/app/session_ops.rs`,
//! `src/app/event_handler.rs`, and `src/app/connector.rs`.
//!
//! These exercise the remaining public surfaces the existing suites leave
//! untouched: the `lib.rs` test-wrapper deadline/should-render helpers and the
//! `App`/`StderrSpool` re-exports; the connector-catalogue and plugin-store
//! panel state helpers in `session_ops.rs`; the event-handler arms whose
//! variants the other suites never construct (the Copilot device-flow result,
//! `RouterClassification`, `ServiceStartError`, `ModelDownloadProgress`, the
//! non-current-session guards); and the `connector.rs` dispatch glue reached
//! through `/connectors <sub>`.
//!
//! Tests only - no production logic is touched. Every test drives a real public
//! API or the real `handle_event` / `execute_slash_command` / key path; no
//! network and no real TTY (rendering, where needed, goes through `TestBackend`).
//! Panels are installed directly into the `App` state rather than through the
//! launch helpers, so no background catalogue/store fetch is ever started.

use std::sync::Arc;
use std::time::{Duration, Instant};

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::Terminal;
use ratatui::backend::TestBackend;

use ragent_agent::event::Event;
use ragent_connectors::{
    CatalogueBrowser, ConnectorAuthShape, ConnectorDescriptor, ConnectorId, ConnectorServer,
};
use ragent_plugins::{StoreEntry, StoreError, StoreIndex, StoreKind};
use ragent_tui::App;
use ragent_tui::app::{
    ConnectorCatalogueInstallResult, ConnectorEnrichResult, PluginStoreBrowser,
    PluginStoreInstallResult, PluginStoreStatus, StorePane,
};
use ragent_tui::input::handle_key;

#[path = "support/mod.rs"]
mod support;

// ===========================================================================
// shared fixtures
// ===========================================================================

/// An app with a live primary session.
fn app_with_session(session: &str) -> App {
    let mut app = support::make_app();
    app.session_id = Some(session.to_string());
    app.needs_redraw = false;
    app
}

fn key(code: KeyCode) -> KeyEvent {
    KeyEvent::new(code, KeyModifiers::NONE)
}

fn server() -> ConnectorServer {
    ConnectorServer {
        id: "main".to_string(),
        transport: "stdio".to_string(),
        command: Some("echo".to_string()),
        args: Vec::new(),
        env: Default::default(),
        url: None,
        headers: Default::default(),
    }
}

/// A connector-catalogue descriptor with a blank description.
fn connector(id: &str, category: &str) -> ConnectorDescriptor {
    ConnectorDescriptor {
        id: ConnectorId::new(id).expect("valid id"),
        name: id.to_string(),
        description: String::new(),
        category: category.to_string(),
        tags: Vec::new(),
        source: format!("git+https://github.com/acme/connectors#v1:{id}"),
        marketplace: String::new(),
        provenance: Default::default(),
        auth: ConnectorAuthShape::None,
        auth_scope: Vec::new(),
        credential: None,
        servers: vec![server()],
        unsupported: Vec::new(),
    }
}

/// A plugin-store entry.
fn store_entry(id: &str, description: &str) -> StoreEntry {
    StoreEntry {
        id: id.to_string(),
        name: id.to_string(),
        version: "1.0.0".to_string(),
        source: format!("https://example.org/{id}.zip"),
        description: description.to_string(),
        dialect: None,
        tags: Vec::new(),
        homepage: None,
        marketplace: String::new(),
        category: None,
    }
}

/// An app with the connector-catalogue panel open and `entries` loaded. The
/// browser is installed directly (no off-loop fetch is started).
fn open_connector_catalogue(entries: Vec<ConnectorDescriptor>) -> App {
    let mut app = support::make_app();
    let mut browser = CatalogueBrowser::new("", false);
    browser.set_entries(entries, 0);
    app.connector_store = Some(browser);
    app
}

/// An app with the plugin-store panel open for `kind` and `entries` loaded. The
/// browser is installed directly (no off-loop fetch is started).
fn open_plugin_store(kind: StoreKind, entries: Vec<StoreEntry>) -> App {
    let mut app = support::make_app();
    let mut browser = PluginStoreBrowser::new(kind, "", false);
    browser.set_entries(entries);
    app.plugin_store = Some(browser);
    app
}

// ===========================================================================
// lib.rs - re-exports and the deadline / render test wrappers
// ===========================================================================

#[test]
fn lib_reexports_app_and_stderr_spool_are_nameable() {
    let app: App = support::make_app();
    assert!(app.session_id.is_none());
    // Naming the crate-root `StderrSpool` re-export proves it is public; it is a
    // `ragent_types::stderr_spool::Spool` alias.
    let _spool: Option<ragent_tui::StderrSpool> = None;
}

#[test]
fn compute_next_deadline_test_idle_caps_at_idle_redraw_interval() {
    let app = support::make_app();
    let last_draw = Instant::now();
    let deadline = ragent_tui::compute_next_deadline_test(&app, last_draw);
    assert!(
        deadline > Instant::now(),
        "the deadline must be in the future"
    );
    assert!(
        deadline <= last_draw + Duration::from_secs(5),
        "an idle app must not schedule further out than the idle cap"
    );
}

#[test]
fn compute_next_deadline_test_permission_countdown_wakes_within_a_second() {
    let mut app = support::make_app();
    app.permission_queue
        .push_back(ragent_agent::permission::PermissionRequest {
            id: "p1".to_string(),
            session_id: "s1".to_string(),
            permission: "bash".to_string(),
            patterns: vec!["ls".to_string()],
            metadata: serde_json::json!({ "timeout_secs": 120u64 }),
            tool_call_id: None,
        });
    let deadline = ragent_tui::compute_next_deadline_test(&app, Instant::now());
    assert!(
        deadline <= Instant::now() + Duration::from_millis(1100),
        "a live permission countdown must wake within a second"
    );
}

#[test]
fn compute_next_deadline_test_status_expiry_shortens_the_wake() {
    let mut app = support::make_app();
    app.status_set_at = Some(Instant::now());
    let deadline = ragent_tui::compute_next_deadline_test(&app, Instant::now());
    assert!(
        deadline <= Instant::now() + Duration::from_millis(2100),
        "the status auto-expiry must bound the wake"
    );
}

#[test]
fn compute_next_deadline_test_run_cost_banner_shortens_the_wake() {
    let mut app = support::make_app();
    app.run_cost_banner_at = Some(Instant::now());
    let deadline = ragent_tui::compute_next_deadline_test(&app, Instant::now());
    assert!(
        deadline <= Instant::now() + Duration::from_secs(16),
        "the run-cost banner auto-dismiss must bound the wake"
    );
}

#[test]
fn compute_next_deadline_test_housekeeping_window_bounds_the_idle_wake() {
    let mut app = support::make_app();
    app.jobs_last_poll = Instant::now();
    let due_at = app.housekeeping_due_at();
    let deadline = ragent_tui::compute_next_deadline_test(&app, Instant::now());
    assert!(deadline <= due_at);
}

#[test]
fn compute_next_deadline_test_pending_message_group_bounds_the_wake() {
    let mut app = support::make_app();
    app.messages
        .push(ragent_types::message::Message::assistant_text("s1", "hi"));
    app.message_cache_dirty_from = 0;
    let deadline = ragent_tui::compute_next_deadline_test(&app, Instant::now());
    assert!(
        deadline <= Instant::now() + Duration::from_secs(5),
        "a pending message group must schedule a repaint wake"
    );
}

#[test]
fn should_render_is_false_when_clean_and_true_when_dirty() {
    let mut app = support::make_app();
    app.needs_redraw = false;
    assert!(!ragent_tui::should_render(&app, Instant::now()));

    app.needs_redraw = true;
    assert!(ragent_tui::should_render(&app, Instant::now()));
}

#[test]
fn should_render_safety_interval_paints_a_pending_message_group() {
    let mut app = support::make_app();
    app.needs_redraw = false;
    app.messages
        .push(ragent_types::message::Message::assistant_text("s1", "hi"));
    app.message_cache_dirty_from = 0;
    let past = Instant::now()
        .checked_sub(Duration::from_secs(3))
        .expect("monotonic clock newer than 3s");
    assert!(
        ragent_tui::should_render(&app, past),
        "a pending group must paint at the safety interval"
    );
}

#[test]
fn should_render_safety_interval_paints_a_live_countdown() {
    let mut app = support::make_app();
    app.needs_redraw = false;
    app.permission_queue
        .push_back(ragent_agent::permission::PermissionRequest {
            id: "p1".to_string(),
            session_id: "s1".to_string(),
            permission: "bash".to_string(),
            patterns: vec!["ls".to_string()],
            metadata: serde_json::json!({ "timeout_secs": 120u64 }),
            tool_call_id: None,
        });
    let past = Instant::now()
        .checked_sub(Duration::from_secs(3))
        .expect("monotonic clock newer than 3s");
    assert!(
        ragent_tui::should_render(&app, past),
        "a live countdown must paint at the safety interval"
    );
}

// ===========================================================================
// session_ops.rs - connector-catalogue panel helpers
// ===========================================================================

#[test]
fn connector_catalogue_search_box_edit_and_clear_paths() {
    let mut app = open_connector_catalogue(vec![
        connector("github", "developer"),
        connector("slack", "communication"),
    ]);

    app.connector_catalogue_push_char('g');
    app.connector_catalogue_push_char('i');
    let browser = app.connector_store.as_ref().expect("panel open");
    assert_eq!(browser.query, "gi");
    assert!(app.needs_redraw);

    assert!(app.connector_catalogue_edit_or_close());
    assert!(app.connector_catalogue_edit_or_close());
    assert!(!app.connector_catalogue_edit_or_close());
    assert!(
        app.connector_store.is_none(),
        "the panel closes on empty Esc"
    );
}

#[test]
fn connector_catalogue_edit_or_close_on_no_panel_is_false() {
    let mut app = support::make_app();
    assert!(!app.connector_catalogue_edit_or_close());
}

#[test]
fn connector_catalogue_focus_toggle_and_nav_moves_apply_category() {
    let mut app = open_connector_catalogue(vec![
        connector("github", "developer"),
        connector("slack", "communication"),
    ]);
    assert!(!app.connector_catalogue_nav_focused());

    app.connector_catalogue_focus_next();
    assert!(app.connector_catalogue_nav_focused());

    app.connector_catalogue_category_move_down();
    app.connector_catalogue_category_confirm();
    assert!(!app.connector_catalogue_nav_focused());
    assert!(app.needs_redraw);

    app.connector_catalogue_category_move_up();
    app.connector_catalogue_clear_category();
    assert_eq!(
        app.connector_store
            .as_ref()
            .expect("panel open")
            .active_category(),
        "ALL"
    );
}

#[test]
fn connector_catalogue_result_moves_clamp_on_a_single_row() {
    let mut app = open_connector_catalogue(vec![connector("only", "x")]);
    app.connector_catalogue_move_down();
    app.connector_catalogue_move_up();
    assert_eq!(
        app.connector_store.as_ref().expect("panel open").cursor,
        0,
        "a single row clamps both directions"
    );
}

#[test]
fn connector_catalogue_next_category_cycles_and_returns_to_all() {
    let mut app = open_connector_catalogue(vec![
        connector("github", "developer"),
        connector("slack", "communication"),
    ]);
    let mut saw_non_all = false;
    for _ in 0..6 {
        app.connector_catalogue_next_category();
        let label = app
            .connector_store
            .as_ref()
            .expect("panel open")
            .active_category()
            .to_string();
        if label != "ALL" {
            saw_non_all = true;
        }
    }
    assert!(saw_non_all, "the cycle visits declared categories");
    assert!(app.needs_redraw);
}

#[test]
fn connector_catalogue_install_selected_refuses_installed() {
    let mut app = open_connector_catalogue(vec![connector("github", "developer")]);
    app.connector_store
        .as_mut()
        .expect("panel open")
        .set_installed(std::iter::once("github".to_string()).collect());
    app.connector_catalogue_install_selected();
    let notice = app
        .connector_store
        .as_ref()
        .expect("panel")
        .last_install
        .clone()
        .unwrap_or_default();
    assert!(notice.contains("already installed"), "got: {notice}");
}

#[test]
fn connector_catalogue_install_selected_on_empty_results_is_neutral() {
    let mut app = open_connector_catalogue(Vec::new());
    app.connector_catalogue_install_selected();
    let notice = app
        .connector_store
        .as_ref()
        .expect("panel")
        .last_install
        .clone()
        .unwrap_or_default();
    assert!(notice.contains("no result"), "got: {notice}");
}

#[test]
fn connector_catalogue_poll_install_result_applies_success_and_failure() {
    let mut app = open_connector_catalogue(vec![connector("github", "developer")]);

    *app.connector_catalogue_install_result.lock().expect("slot") =
        Some(ConnectorCatalogueInstallResult {
            id: "github".to_string(),
            notice: "install failed: boom".to_string(),
            report: "[err] install failed: boom".to_string(),
            succeeded: false,
        });
    app.poll_connector_catalogue_install_result();
    assert!(app.needs_redraw);

    *app.connector_catalogue_install_result.lock().expect("slot") =
        Some(ConnectorCatalogueInstallResult {
            id: "slack".to_string(),
            notice: "installed slack".to_string(),
            report: "installed slack".to_string(),
            succeeded: true,
        });
    app.poll_connector_catalogue_install_result();
    assert_eq!(
        app.connector_store
            .as_ref()
            .expect("panel")
            .last_install
            .as_deref(),
        Some("installed slack")
    );
}

#[test]
fn connector_catalogue_poll_install_result_noop_when_empty_slot() {
    let mut app = open_connector_catalogue(vec![connector("github", "developer")]);
    app.needs_redraw = false;
    app.poll_connector_catalogue_install_result();
    assert!(!app.needs_redraw, "an empty slot is a no-op");
}

#[test]
fn refresh_connector_catalogue_installed_is_noop_without_panel() {
    let mut app = support::make_app();
    app.needs_redraw = false;
    app.refresh_connector_catalogue_installed();
    assert!(!app.needs_redraw);
}

#[test]
fn refresh_connector_catalogue_installed_redraws_with_panel() {
    let mut app = open_connector_catalogue(vec![connector("github", "developer")]);
    app.needs_redraw = false;
    app.refresh_connector_catalogue_installed();
    assert!(app.needs_redraw);
}

#[test]
fn connector_catalogue_poll_result_records_refused_marketplace() {
    use ragent_tui::app::ConnectorCatalogueResult;

    let mut app = open_connector_catalogue(Vec::new());
    *app.connector_catalogue_result.lock().expect("slot") = Some(ConnectorCatalogueResult {
        outcome: Ok(vec![connector("github", "developer")]),
        skipped: 1,
        refused_marketplace: Some("https://evil.example/marketplace.json".to_string()),
    });
    app.poll_connector_catalogue_result();
    assert!(
        app.log_entries
            .iter()
            .any(|e| e.message.contains("skipped")),
        "a refused marketplace is surfaced as a warning log"
    );
    assert!(app.needs_redraw);
}

#[test]
fn connector_catalogue_poll_result_failure_sets_panel_failed_status() {
    use ragent_tui::app::ConnectorCatalogueResult;

    let mut app = open_connector_catalogue(Vec::new());
    *app.connector_catalogue_result.lock().expect("slot") = Some(ConnectorCatalogueResult {
        outcome: Err("network down".to_string()),
        skipped: 0,
        refused_marketplace: None,
    });
    app.poll_connector_catalogue_result();
    assert!(
        matches!(
            app.connector_store.as_ref().expect("panel open").status,
            ragent_tui::app::ConnectorBrowseStatus::Failed(_)
        ),
        "a failed fetch leaves the panel in the failed state"
    );
}

#[test]
fn connector_catalogue_poll_result_without_panel_does_not_redraw() {
    use ragent_tui::app::ConnectorCatalogueResult;

    let mut app = support::make_app();
    app.needs_redraw = false;
    *app.connector_catalogue_result.lock().expect("slot") = Some(ConnectorCatalogueResult {
        outcome: Ok(vec![connector("github", "developer")]),
        skipped: 0,
        refused_marketplace: None,
    });
    app.poll_connector_catalogue_result();
    assert!(
        !app.needs_redraw,
        "a late result never fills a closed browser"
    );
}

#[test]
fn connector_catalogue_poll_enrichment_applies_matching_generation() {
    use ragent_connectors::catalogue_enrich::{ManifestMeta, ManifestUpdate};

    let mut app = open_connector_catalogue(vec![connector("github", "developer")]);
    let generation = app
        .connector_enrich_generation
        .load(std::sync::atomic::Ordering::SeqCst);
    app.connector_enrich_result
        .lock()
        .expect("slot")
        .push(ConnectorEnrichResult::Update {
            generation,
            update: ManifestUpdate {
                id: "github".to_string(),
                meta: ManifestMeta {
                    version: Some("2.0.0".to_string()),
                    description: Some("filled in".to_string()),
                },
            },
        });
    app.connector_enrich_result
        .lock()
        .expect("slot")
        .push(ConnectorEnrichResult::Finished { generation });
    app.poll_connector_enrichment();
    assert!(app.needs_redraw);
    assert!(
        app.connector_store
            .as_ref()
            .expect("panel open")
            .all
            .iter()
            .any(|e| e.id.as_str() == "github" && e.description == "filled in"),
        "the enrichment filled the blank description"
    );
}

#[test]
fn connector_catalogue_poll_enrichment_drops_stale_generation() {
    use ragent_connectors::catalogue_enrich::{ManifestMeta, ManifestUpdate};

    let mut app = open_connector_catalogue(vec![connector("github", "developer")]);
    let stale = app
        .connector_enrich_generation
        .load(std::sync::atomic::Ordering::SeqCst)
        .wrapping_sub(1);
    app.connector_enrich_result
        .lock()
        .expect("slot")
        .push(ConnectorEnrichResult::Update {
            generation: stale,
            update: ManifestUpdate {
                id: "github".to_string(),
                meta: ManifestMeta {
                    version: None,
                    description: Some("stale description".to_string()),
                },
            },
        });
    app.poll_connector_enrichment();
    assert!(
        !app.connector_store
            .as_ref()
            .expect("panel open")
            .all
            .iter()
            .any(|e| e.description == "stale description"),
        "a stale-generation update is dropped"
    );
}

#[test]
fn connector_catalogue_poll_enrichment_empty_queue_is_noop() {
    let mut app = open_connector_catalogue(vec![connector("github", "developer")]);
    app.needs_redraw = false;
    app.poll_connector_enrichment();
    assert!(!app.needs_redraw);
}

// ===========================================================================
// session_ops.rs - plugin-store panel helpers
// ===========================================================================

#[test]
fn plugin_store_push_char_and_clear_search_round_trip() {
    let mut app = open_plugin_store(
        StoreKind::Codex,
        vec![store_entry("alpha", "one"), store_entry("beta", "two")],
    );
    app.plugin_store_push_char('a');
    app.plugin_store_push_char('l');
    assert_eq!(app.plugin_store.as_ref().expect("panel open").query, "al");
    assert!(app.needs_redraw);

    app.plugin_store_clear_search();
    assert_eq!(app.plugin_store.as_ref().expect("panel open").query, "");
}

#[test]
fn plugin_store_clear_search_on_empty_query_is_noop() {
    let mut app = open_plugin_store(StoreKind::Codex, vec![store_entry("alpha", "one")]);
    app.needs_redraw = false;
    app.plugin_store_clear_search();
    assert!(!app.needs_redraw, "clearing an empty query is a no-op");
}

#[test]
fn plugin_store_focus_next_and_prev_cycle_the_three_panes() {
    let mut app = open_plugin_store(StoreKind::Codex, vec![store_entry("alpha", "one")]);
    assert_eq!(
        app.plugin_store.as_ref().expect("panel").focus,
        StorePane::Results
    );
    app.plugin_store_focus_next();
    assert_eq!(
        app.plugin_store.as_ref().expect("panel").focus,
        StorePane::Categories
    );
    app.plugin_store_focus_prev();
    assert_eq!(
        app.plugin_store.as_ref().expect("panel").focus,
        StorePane::Results
    );
    assert!(app.needs_redraw);
}

#[test]
fn plugin_store_category_and_marketplace_nav_helpers_apply() {
    let mut app = open_plugin_store(
        StoreKind::Codex,
        vec![store_entry("alpha", "one"), store_entry("beta", "two")],
    );
    app.plugin_store_category_move_down();
    app.plugin_store_category_move_up();
    app.plugin_store_category_confirm();
    app.plugin_store_clear_category();

    app.plugin_store_marketplace_move_down();
    app.plugin_store_marketplace_move_up();
    app.plugin_store_marketplace_confirm();
    app.plugin_store_clear_marketplace();
    assert!(app.needs_redraw);
}

#[test]
fn plugin_store_install_selected_refuses_installed_and_notifies_empty() {
    let mut app = open_plugin_store(StoreKind::Codex, vec![store_entry("alpha", "one")]);
    app.plugin_store
        .as_mut()
        .expect("panel open")
        .set_installed(std::iter::once("alpha".to_string()).collect());
    app.plugin_store_install_selected();
    let notice = app
        .plugin_store
        .as_ref()
        .expect("panel")
        .last_install
        .clone()
        .unwrap_or_default();
    assert!(notice.contains("already installed"), "got: {notice}");

    let mut empty = open_plugin_store(StoreKind::Codex, Vec::new());
    empty.plugin_store_install_selected();
    let notice = empty
        .plugin_store
        .as_ref()
        .expect("panel")
        .last_install
        .clone()
        .unwrap_or_default();
    assert!(notice.contains("no result"), "got: {notice}");
}

#[test]
fn plugin_store_install_selected_without_panel_is_noop() {
    let mut app = support::make_app();
    app.needs_redraw = false;
    app.plugin_store_install_selected();
    assert!(!app.needs_redraw);
}

#[test]
fn plugin_store_poll_install_result_applies_and_refreshes() {
    let mut app = open_plugin_store(StoreKind::Codex, vec![store_entry("alpha", "one")]);
    *app.plugin_store_install_result.lock().expect("slot") = Some(PluginStoreInstallResult {
        kind: StoreKind::Codex,
        id: "alpha".to_string(),
        notice: "installed alpha".to_string(),
        report: "installed alpha".to_string(),
        succeeded: true,
    });
    app.poll_plugin_store_install_result();
    assert_eq!(
        app.plugin_store
            .as_ref()
            .expect("panel")
            .last_install
            .as_deref(),
        Some("installed alpha")
    );
}

#[test]
fn plugin_store_install_result_for_other_kind_does_not_touch_panel() {
    let mut app = open_plugin_store(StoreKind::Codex, vec![store_entry("alpha", "one")]);
    *app.plugin_store_install_result.lock().expect("slot") = Some(PluginStoreInstallResult {
        kind: StoreKind::Claude,
        id: "alpha".to_string(),
        notice: "installed alpha".to_string(),
        report: "installed alpha".to_string(),
        succeeded: false,
    });
    app.poll_plugin_store_install_result();
    assert!(
        app.needs_redraw,
        "the report still reaches the message window"
    );
    assert!(
        app.plugin_store
            .as_ref()
            .expect("panel")
            .last_install
            .is_none(),
        "a Claude result does not fill the Codex panel's footer"
    );
}

#[test]
fn plugin_store_refresh_installed_noop_without_panel() {
    let mut app = support::make_app();
    app.needs_redraw = false;
    app.refresh_plugin_store_installed();
    assert!(!app.needs_redraw);
}

#[test]
fn plugin_store_refresh_installed_redraws_with_panel() {
    let mut app = open_plugin_store(StoreKind::Codex, vec![store_entry("alpha", "one")]);
    app.needs_redraw = false;
    app.refresh_plugin_store_installed();
    assert!(app.needs_redraw);
}

#[test]
fn plugin_store_edit_or_close_backspaces_then_closes() {
    let mut app = open_plugin_store(StoreKind::Codex, vec![store_entry("alpha", "one")]);
    app.plugin_store_push_char('x');
    assert!(app.plugin_store_edit_or_close());
    assert!(!app.plugin_store_edit_or_close());
    assert!(app.plugin_store.is_none());
}

#[test]
fn plugin_store_edit_or_close_without_panel_is_false() {
    let mut app = support::make_app();
    assert!(!app.plugin_store_edit_or_close());
}

#[test]
fn plugin_store_close_is_idempotent() {
    let mut app = open_plugin_store(StoreKind::Codex, vec![store_entry("alpha", "one")]);
    app.close_plugin_store();
    app.needs_redraw = false;
    app.close_plugin_store();
    assert!(!app.needs_redraw, "closing a closed panel is a no-op");
}

#[test]
fn plugin_store_poll_result_empty_index_and_failed_fetch() {
    use ragent_tui::app::PluginStoreFetchResult;

    let mut app = open_plugin_store(StoreKind::Codex, Vec::new());
    *app.plugin_store_result.lock().expect("slot") = Some(PluginStoreFetchResult {
        kind: StoreKind::Codex,
        outcome: Ok(StoreIndex::default()),
    });
    app.poll_plugin_store_result();
    assert_eq!(
        app.plugin_store.as_ref().expect("panel").status,
        PluginStoreStatus::Empty
    );

    *app.plugin_store_result.lock().expect("slot") = Some(PluginStoreFetchResult {
        kind: StoreKind::Codex,
        outcome: Err(StoreError::Network {
            detail: "refused".to_string(),
        }),
    });
    app.poll_plugin_store_result();
    assert!(matches!(
        app.plugin_store.as_ref().expect("panel").status,
        PluginStoreStatus::Failed(_)
    ));
}

#[test]
fn plugin_store_poll_result_stale_kind_is_ignored() {
    use ragent_tui::app::PluginStoreFetchResult;

    let mut app = open_plugin_store(StoreKind::Codex, vec![store_entry("alpha", "one")]);
    assert_eq!(
        app.plugin_store.as_ref().expect("panel").status,
        PluginStoreStatus::Ready
    );
    *app.plugin_store_result.lock().expect("slot") = Some(PluginStoreFetchResult {
        kind: StoreKind::Claude,
        outcome: Err(StoreError::Network {
            detail: "should be ignored".to_string(),
        }),
    });
    app.poll_plugin_store_result();
    assert_eq!(
        app.plugin_store.as_ref().expect("panel").status,
        PluginStoreStatus::Ready,
        "a result for the other store is ignored"
    );
}

#[test]
fn plugin_store_poll_result_noop_when_empty_slot() {
    let mut app = open_plugin_store(StoreKind::Codex, Vec::new());
    app.needs_redraw = false;
    app.poll_plugin_store_result();
    assert!(!app.needs_redraw);
}

// ===========================================================================
// session_ops.rs - rollback / websearch polls, clear dialogs, poisoned lock
// ===========================================================================

#[test]
fn poll_rollback_result_handles_all_three_outcomes() {
    let mut app = support::make_app();

    *app.rollback_result.lock().expect("slot") = Some(Ok(true));
    app.poll_rollback_result();
    assert!(app.status.contains("rollback complete"));

    *app.rollback_result.lock().expect("slot") = Some(Ok(false));
    app.poll_rollback_result();
    assert!(app.status.contains("nothing to restore"));

    *app.rollback_result.lock().expect("slot") = Some(Err("io error".to_string()));
    app.poll_rollback_result();
    assert!(app.status.contains("rollback failed"));
    assert!(app.needs_redraw);
}

#[test]
fn poll_rollback_result_empty_slot_is_noop() {
    let mut app = support::make_app();
    app.needs_redraw = false;
    app.poll_rollback_result();
    assert!(!app.needs_redraw);
}

#[test]
fn poll_rollback_result_recovers_a_poisoned_slot_and_applies_it() {
    // Poison `rollback_result` by panicking while holding its lock, then
    // confirm the poll's `recover_poisoned` path still applies the retained
    // value.
    let mut app = support::make_app();
    let slot = Arc::clone(&app.rollback_result);
    let _ = std::thread::spawn(move || {
        let mut guard = slot.lock().expect("lock");
        *guard = Some(Ok(true));
        panic!("poison the rollback slot");
    })
    .join();

    app.poll_rollback_result();
    assert!(
        app.status.contains("rollback complete"),
        "a poisoned slot is recovered and applied, got: {}",
        app.status
    );
}

#[test]
fn poll_websearch_test_result_appends_report_and_sets_status() {
    let mut app = app_with_session("s1");
    *app.websearch_test_result.lock().expect("slot") =
        Some("websearch engine report body".to_string());
    app.poll_websearch_test_result();
    assert_eq!(app.status, "websearch: engine test complete");
    let text = app
        .messages
        .last()
        .map(ragent_types::message::Message::text_content)
        .unwrap_or_default();
    assert!(text.contains("websearch engine report body"));
}

#[test]
fn poll_websearch_test_result_empty_slot_is_noop() {
    let mut app = support::make_app();
    app.needs_redraw = false;
    app.poll_websearch_test_result();
    assert!(!app.needs_redraw);
}

#[test]
fn queue_clear_confirm_toggle_flips_yes_no() {
    use ragent_tui::app::{QUEUE_CLEAR_CONFIRM_NO, QUEUE_CLEAR_CONFIRM_YES};

    let mut app = support::make_app();
    app.queue_clear_confirm_selected = QUEUE_CLEAR_CONFIRM_NO;
    assert!(!app.queue_clear_confirm_is_yes());
    app.queue_clear_confirm_toggle();
    assert!(app.queue_clear_confirm_is_yes());
    assert_eq!(app.queue_clear_confirm_selected, QUEUE_CLEAR_CONFIRM_YES);
    app.queue_clear_confirm_toggle();
    assert_eq!(app.queue_clear_confirm_selected, QUEUE_CLEAR_CONFIRM_NO);
    assert!(app.needs_redraw);
}

#[test]
fn memory_clear_confirm_open_defaults_no_and_toggles() {
    let mut app = support::make_app();
    app.open_memory_clear_confirm();
    assert!(app.memory_clear_confirm_open);
    assert!(!app.memory_clear_confirm_is_yes(), "defaults to No");
    app.memory_clear_confirm_toggle();
    assert!(app.memory_clear_confirm_is_yes());
    app.memory_clear_confirm_toggle();
    assert!(!app.memory_clear_confirm_is_yes());
}

// ===========================================================================
// event_handler.rs - the remaining Event arms
// ===========================================================================

#[tokio::test]
async fn copilot_device_flow_complete_persists_token_and_opens_model_picker() {
    use ragent_tui::app::ProviderSetupStep;

    let mut app = app_with_session("s1");
    app.handle_event(Event::CopilotDeviceFlowComplete {
        token: "gho_secret".to_string(),
        api_base: "https://api.githubcopilot.com".to_string(),
    })
    .await;
    assert!(matches!(
        app.provider_setup,
        Some(ProviderSetupStep::SelectModel { ref provider_id, .. }) if provider_id == "copilot"
    ));
    assert!(
        app.log_entries
            .iter()
            .any(|e| e.message.contains("Copilot authorised")),
        "the authorization is logged"
    );
    assert!(
        app.storage
            .get_provider_auth("copilot")
            .ok()
            .flatten()
            .is_some(),
        "the token is persisted"
    );
}

#[tokio::test]
async fn copilot_device_flow_complete_clears_disabled_flag_setting() {
    let mut app = app_with_session("s1");
    app.storage
        .set_setting("provider_copilot_disabled", "true")
        .expect("seed disabled flag");
    app.handle_event(Event::CopilotDeviceFlowComplete {
        token: "t".to_string(),
        api_base: "https://api".to_string(),
    })
    .await;
    assert_eq!(
        app.storage
            .get_setting("provider_copilot_disabled")
            .ok()
            .flatten(),
        None,
        "the disabled flag is cleared on success"
    );
}

#[tokio::test]
async fn router_classification_logs_the_bucket_selection() {
    let mut app = app_with_session("s1");
    app.handle_event(Event::RouterClassification {
        session_id: "s1".to_string(),
        tier: "standard".to_string(),
        requested_tier: Some("cheap".to_string()),
        model: "anthropic:claude-3-5-haiku".to_string(),
        composite_score: 0.42,
        prompt: "classify me".to_string(),
        dimensions: vec![("complexity".to_string(), 0.5)],
    })
    .await;
    assert!(
        app.log_entries
            .iter()
            .any(|e| e.message.to_lowercase().contains("router")),
        "the routing decision reaches the log"
    );
}

#[tokio::test]
async fn service_start_error_surfaces_a_failure() {
    let mut app = app_with_session("s1");
    app.handle_event(Event::ServiceStartError {
        session_id: "s1".to_string(),
        service: "ollama".to_string(),
        command_path: "/usr/bin/ollama".to_string(),
        stdout: String::new(),
        stderr: "boom".to_string(),
        error: "failed to start".to_string(),
    })
    .await;
    assert!(app.needs_redraw);
}

#[tokio::test]
async fn model_download_progress_updates_the_download_state() {
    let mut app = app_with_session("s1");
    app.handle_event(Event::ModelDownloadStarted {
        provider_id: "ollama".to_string(),
        model_id: "llama3".to_string(),
        session_id: "s1".to_string(),
    })
    .await;
    assert!(app.model_download_state.is_some());

    app.handle_event(Event::ModelDownloadProgress {
        provider_id: "ollama".to_string(),
        model_id: "llama3".to_string(),
        session_id: "s1".to_string(),
        percent: 42.0,
    })
    .await;
    let state = app.model_download_state.as_ref().expect("download live");
    assert!((state.percent - 42.0).abs() < f32::EPSILON);
}

#[tokio::test]
async fn permission_requested_for_other_session_is_ignored() {
    let mut app = app_with_session("s1");
    app.handle_event(Event::PermissionRequested {
        session_id: "other".to_string(),
        request_id: "r-other".to_string(),
        permission: "bash".to_string(),
        description: "run".to_string(),
        options: Vec::new(),
    })
    .await;
    assert!(
        app.permission_queue.is_empty(),
        "a permission request for a non-current session is ignored"
    );
}

#[tokio::test]
async fn agent_switched_updates_agent_name_for_current_session() {
    let mut app = app_with_session("s1");
    app.handle_event(Event::AgentSwitched {
        session_id: "s1".to_string(),
        from: "coder".to_string(),
        to: "architect".to_string(),
    })
    .await;
    assert_eq!(app.agent_name, "architect");
}

#[tokio::test]
async fn agent_switched_for_other_session_is_ignored() {
    let mut app = app_with_session("s1");
    let before = app.agent_name.clone();
    app.handle_event(Event::AgentSwitched {
        session_id: "other".to_string(),
        from: "coder".to_string(),
        to: "architect".to_string(),
    })
    .await;
    assert_eq!(app.agent_name, before);
}

#[tokio::test]
async fn shell_cwd_changed_records_the_shell_directory() {
    let mut app = app_with_session("s1");
    app.handle_event(Event::ShellCwdChanged {
        session_id: "s1".to_string(),
        cwd: "/tmp/work".to_string(),
    })
    .await;
    assert_eq!(app.shell_cwd.as_deref(), Some("/tmp/work"));
}

#[tokio::test]
async fn session_created_sets_id_and_logs_only_when_unset() {
    let mut app = support::make_app();
    app.handle_event(Event::SessionCreated {
        session_id: "created-session".to_string(),
    })
    .await;
    assert_eq!(app.session_id.as_deref(), Some("created-session"));
    assert!(
        app.log_entries
            .iter()
            .any(|e| e.message.contains("session created")),
        "the first SessionCreated is logged"
    );

    let first_id = app.session_id.clone();
    app.handle_event(Event::SessionCreated {
        session_id: "ignored-session".to_string(),
    })
    .await;
    assert_eq!(app.session_id, first_id, "the id is not overwritten");
}

#[tokio::test]
async fn token_usage_accumulates_and_updates_input_tokens() {
    let mut app = app_with_session("s1");
    app.handle_event(Event::TokenUsage {
        session_id: "s1".to_string(),
        input_tokens: 100,
        output_tokens: 40,
    })
    .await;
    assert_eq!(app.last_input_tokens, 100);
    assert_eq!(app.token_usage, (100, 40));

    app.handle_event(Event::TokenUsage {
        session_id: "s1".to_string(),
        input_tokens: 50,
        output_tokens: 10,
    })
    .await;
    assert_eq!(app.token_usage, (150, 50));
}

#[tokio::test]
async fn loop_terminated_renders_a_banner_with_verification_and_reason() {
    let mut app = app_with_session("s1");
    app.handle_event(Event::LoopTerminated {
        session_id: "s1".to_string(),
        status: "error".to_string(),
        iterations: 3,
        verification: Some("tests passed".to_string()),
        reason: Some("budget exhausted".to_string()),
    })
    .await;
    let text = app
        .messages
        .last()
        .map(ragent_types::message::Message::text_content)
        .unwrap_or_default();
    assert!(text.contains("tests passed"));
    assert!(text.contains("budget exhausted"));
}

#[tokio::test]
async fn loop_terminated_for_other_session_is_ignored() {
    let mut app = app_with_session("s1");
    let before = app.messages.len();
    app.handle_event(Event::LoopTerminated {
        session_id: "other".to_string(),
        status: "completed".to_string(),
        iterations: 1,
        verification: None,
        reason: None,
    })
    .await;
    assert_eq!(app.messages.len(), before);
}

#[tokio::test]
async fn subagent_start_registers_a_task() {
    let mut app = app_with_session("s1");
    app.handle_event(Event::SubagentStart {
        session_id: "s1".to_string(),
        task_id: "t1".to_string(),
        child_session_id: "child-1".to_string(),
        agent: "explore".to_string(),
        task: "look around".to_string(),
        background: true,
    })
    .await;
    assert!(
        app.active_tasks.iter().any(|t| t.id == "t1"),
        "the sub-agent task is tracked"
    );
}

#[tokio::test]
async fn subagent_complete_marks_the_task_completed() {
    let mut app = app_with_session("s1");
    app.handle_event(Event::SubagentStart {
        session_id: "s1".to_string(),
        task_id: "t1".to_string(),
        child_session_id: "child-1".to_string(),
        agent: "explore".to_string(),
        task: "look around".to_string(),
        background: true,
    })
    .await;
    app.handle_event(Event::SubagentComplete {
        session_id: "s1".to_string(),
        task_id: "t1".to_string(),
        child_session_id: "child-1".to_string(),
        summary: "done".to_string(),
        success: true,
        duration_ms: 12,
        finish_reason: "stop".to_string(),
    })
    .await;
    assert!(
        !app.active_tasks.iter().any(|t| t.id == "t1"),
        "a completed sub-agent is removed from the live task list"
    );
    assert!(
        app.log_entries
            .iter()
            .any(|e| e.message.contains("Task completed")),
        "the completion is logged"
    );
}

#[tokio::test]
async fn teammate_p2p_message_records_counts_on_both_ends() {
    let mut app = app_with_session("s1");
    app.handle_event(Event::TeammateP2PMessage {
        session_id: "s1".to_string(),
        team_name: "alpha".to_string(),
        from: "tm-001".to_string(),
        to: "tm-002".to_string(),
        message_type: "message".to_string(),
        preview: "hi".to_string(),
    })
    .await;
    assert_eq!(app.team_message_counts.get("tm-001").copied(), Some((1, 0)));
    assert_eq!(app.team_message_counts.get("tm-002").copied(), Some((0, 1)));
}

#[tokio::test]
async fn user_input_event_sets_working_status_for_current_session() {
    let mut app = app_with_session("s1");
    app.handle_event(Event::UserInput {
        session_id: "s1".to_string(),
        request_id: "req".to_string(),
        response: "the answer".to_string(),
    })
    .await;
    assert!(app.status.contains("processing"), "got: {}", app.status);
}

#[tokio::test]
async fn memory_stored_marks_memory_cache_dirty() {
    let mut app = app_with_session("s1");
    app.memory_cache_dirty = false;
    app.handle_event(Event::MemoryStored {
        session_id: "s1".to_string(),
        id: 1,
        category: "fact".to_string(),
    })
    .await;
    assert!(app.memory_cache_dirty);
}

#[tokio::test]
async fn provider_loading_finished_advances_a_waiting_dialog() {
    use ragent_tui::app::ProviderSetupStep;

    let mut app = app_with_session("s1");
    app.provider_setup = Some(ProviderSetupStep::LoadingModels {
        provider_id: "ollama".to_string(),
        provider_name: "Ollama".to_string(),
    });
    app.handle_event(Event::ProviderLoadingStarted {
        provider_id: "ollama".to_string(),
        provider_name: "Ollama".to_string(),
    })
    .await;
    assert!(app.model_loading_state.is_some());

    app.handle_event(Event::ProviderLoadingFinished {
        provider_id: "ollama".to_string(),
        provider_name: "Ollama".to_string(),
        models: Vec::new(),
        error: Some("refused".to_string()),
    })
    .await;
    assert!(app.model_loading_state.is_none());
    assert!(matches!(
        app.provider_setup,
        Some(ProviderSetupStep::SelectModel { ref provider_id, .. }) if provider_id == "ollama"
    ));
}

#[tokio::test]
async fn background_task_completed_updates_status() {
    let mut app = app_with_session("s1");
    app.handle_event(Event::BackgroundTaskSpawned {
        session_id: "s1".to_string(),
        task_id: "bg-1".to_string(),
        command: "sleep 1".to_string(),
    })
    .await;
    app.handle_event(Event::BackgroundTaskCompleted {
        session_id: "s1".to_string(),
        task_id: "bg-1".to_string(),
        status: "completed".to_string(),
        exit_code: Some(0),
    })
    .await;
    if let Some(task) = app.bg_tasks.iter().find(|t| t.id == "bg-1") {
        assert_eq!(task.status, "completed");
    }
}

// ===========================================================================
// connector.rs - the `/connectors <sub>` dispatch glue
// ===========================================================================

#[tokio::test]
async fn connectors_bare_and_unknown_render_usage_through_the_dispatcher() {
    let mut app = support::make_app();
    app.session_id = Some("test-session".to_string());

    for invocation in ["/connectors", "/connectors bogus"] {
        app.execute_slash_command(invocation).await;
        let text = app
            .messages
            .last()
            .map(ragent_types::message::Message::text_content)
            .unwrap_or_default();
        assert!(
            text.contains("From: /connectors"),
            "{invocation} must render the usage block, got: {text}"
        );
    }
}

#[tokio::test]
async fn connectors_management_subcommand_without_live_client_reports_refusal() {
    // A management transition with no live MCP client and no published session
    // must report the refusal rather than panicking; no network is touched.
    let mut app = support::make_app();
    app.session_id = Some("test-session".to_string());
    app.execute_slash_command("/connectors enable demo").await;
    let text = app
        .messages
        .last()
        .map(ragent_types::message::Message::text_content)
        .unwrap_or_default();
    assert!(
        text.contains("From: /connectors enable"),
        "the enable attempt renders an attributed report, got: {text}"
    );
}

// ===========================================================================
// Real key routing through the panels and a TestBackend render
// ===========================================================================

#[tokio::test]
async fn plugin_store_panel_swallows_keys_via_handle_key() {
    let mut app = open_plugin_store(
        StoreKind::Codex,
        vec![store_entry("alpha", "one"), store_entry("beta", "two")],
    );
    let _ = handle_key(&mut app, key(KeyCode::Down)).await;
    let _ = handle_key(&mut app, key(KeyCode::Tab)).await;
    assert_eq!(
        app.plugin_store.as_ref().expect("panel").focus,
        StorePane::Categories
    );
    let _ = handle_key(&mut app, key(KeyCode::Char('a'))).await;
    assert_eq!(app.plugin_store.as_ref().expect("panel").query, "a");
}

#[tokio::test]
async fn connector_catalogue_panel_swallows_keys_via_handle_key() {
    let mut app = open_connector_catalogue(vec![
        connector("github", "developer"),
        connector("slack", "communication"),
    ]);
    let _ = handle_key(&mut app, key(KeyCode::Tab)).await;
    assert!(app.connector_catalogue_nav_focused());
    let _ = handle_key(&mut app, key(KeyCode::Char('g'))).await;
    assert_eq!(app.connector_store.as_ref().expect("panel").query, "g");
}

#[tokio::test]
async fn render_frame_with_connector_panel_open_has_unicode_borders() {
    let mut app = open_connector_catalogue(vec![connector("github", "developer")]);
    let backend = TestBackend::new(100, 30);
    let mut terminal = Terminal::new(backend).expect("test terminal");
    terminal
        .draw(|frame| ragent_tui::layout::render(frame, &mut app))
        .expect("draw");
    // The panel frames itself with the default Unicode box-drawing glyphs; the
    // surrounding app chrome (logo) may carry other glyphs, so scope the check
    // to the panel's own border.
    let area = app.connector_store_area;
    assert!(
        area.width > 0 && area.height > 0,
        "the panel painted an area"
    );
    let buffer = terminal.backend().buffer().clone();
    let right = area.x + area.width.saturating_sub(1);
    assert_eq!(
        buffer[(area.x, area.y)].symbol(),
        "\u{250c}",
        "top-left border corner"
    );
    assert_eq!(
        buffer[(right, area.y)].symbol(),
        "\u{2510}",
        "top-right border corner"
    );
    let bottom = area.y + area.height.saturating_sub(1);
    assert_eq!(
        buffer[(area.x, bottom)].symbol(),
        "\u{2514}",
        "bottom-left border corner"
    );
    for y in area.y + 1..bottom {
        assert_eq!(
            buffer[(area.x, y)].symbol(),
            "\u{2502}",
            "left border rule at y={y}"
        );
    }
}
