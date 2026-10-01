//! Tests for the `/connectors` management subcommands (spec `connectors` T-011;
//! FR-009, FR-010, FR-012, FR-013, FR-014, FR-018, FR-019, FR-021, FR-022,
//! FR-025, FR-039, FR-041).
//!
//! The pure renderers ([`render_list`], [`render_search`]) and the shared
//! management report wording are exercised directly; the async dispatcher
//! ([`run_connector_subcommand_env`]) is driven through a fake
//! [`ConnectorCommandEnv`] so the subcommand routing and the master-switch guard
//! are asserted with no live session.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use async_trait::async_trait;
use ragent_config::{ConnectorsConfig, McpServerConfig};
use ragent_connectors::{
    ALL_CATEGORY, AuthOutcome, AuthOutcomeError, AuthState, CategoryError, CategoryFilter,
    ConnectReport, ConnectorCommandEnv, ConnectorDescriptor, ConnectorError,
    ConnectorLifecycleState, ConnectorStatus, DisableReport, LifecycleError, ListInput, McpProbe,
    ProbeTool, ServerReport, ServerState, StoreDirs, StoreLedger, ToolCount, auth_report,
    connect_report, disable_report, disconnect_report, enable_report, render_list, render_search,
    resolve_filter, run_connector_subcommand_env,
};

static SEQ: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);

/// RAII sandboxed temp tree rooted at `target/temp/` (AGENTS.md: no `/tmp`).
struct TempTree(PathBuf);

impl TempTree {
    fn new(name: &str) -> Self {
        let unique = SEQ.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(format!(
            "../../target/temp/connectors-test/management-{name}-{}-{unique}",
            std::process::id()
        ));
        let store = path.join(".ragent").join("connectors");
        std::fs::create_dir_all(store.join("gdrive")).expect("store creatable");
        std::fs::write(
            store.join("gdrive").join("connector.json"),
            r#"{ "id": "gdrive", "name": "Google Drive", "category": "productivity",
                 "auth": "token", "credential": "GDRIVE_TOKEN",
                 "servers": [ { "id": "main", "transport": "stdio", "command": "/bin/true" } ] }"#,
        )
        .expect("manifest writable");
        Self(path)
    }

    fn store(&self) -> PathBuf {
        self.0.join(".ragent").join("connectors")
    }

    fn dirs(&self) -> StoreDirs {
        StoreDirs {
            project: Some(self.store()),
            global: None,
        }
    }
}

impl Drop for TempTree {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// A descriptor with the given id and category (no servers needed to list it).
fn descriptor(id: &str, name: &str, category: &str) -> ConnectorDescriptor {
    serde_json::from_str(&format!(
        r#"{{ "id": "{id}", "name": "{name}", "category": "{category}",
              "servers": [ {{ "id": "main", "transport": "stdio", "command": "/bin/true" }} ] }}"#
    ))
    .expect("descriptor parses")
}

/// A live status snapshot for one connector.
fn status(
    id: &str,
    state: ConnectorLifecycleState,
    auth: AuthState,
    tools: usize,
) -> ConnectorStatus {
    ConnectorStatus {
        id: id.to_string(),
        name: format!("{id} name"),
        state,
        auth,
        error: None,
        servers: vec![ServerReport {
            server_id: format!("{id}.main"),
            state: ServerState::Connected,
            tools: (0..tools)
                .map(|i| format!("mcp_{id}_main_tool{i}"))
                .collect(),
            error: None,
        }],
    }
}

// -- render_list (FR-009, FR-025, FR-039, FR-041) -----------------------------

#[test]
fn list_renders_one_row_per_connector_with_counts() {
    let tree = TempTree::new("list-rows");
    let dirs = tree.dirs();
    let statuses = vec![status(
        "gdrive",
        ConnectorLifecycleState::Connected,
        AuthState::Satisfied,
        3,
    )];
    let input = ListInput {
        dirs: &dirs,
        statuses: &statuses,
    };
    let report = render_list(&input, &CategoryFilter::all(), false).expect("filter known");

    assert!(report.starts_with("From: /connectors list"), "{report}");
    assert!(report.contains("gdrive"), "{report}");
    assert!(report.contains("Google Drive"), "{report}");
    assert!(report.contains("category productivity"), "{report}");
    assert!(report.contains("state connected"), "{report}");
    assert!(report.contains("auth authenticated"), "{report}");
    assert!(report.contains("1 server(s), 3 tool(s)"), "{report}");
    assert!(report.contains("Total: 1 connector(s)"), "{report}");
    assert!(report.contains("category ALL (1 of 1)"), "{report}");
    assert!(report.is_ascii(), "{report}");
}

#[test]
fn list_without_a_session_shows_the_store_enable_state() {
    let tree = TempTree::new("list-offline");
    let dirs = tree.dirs();
    let input = ListInput {
        dirs: &dirs,
        statuses: &[],
    };

    // A fresh connector is disabled and, with no live session, unknown tools.
    let report = render_list(&input, &CategoryFilter::all(), false).expect("filter known");
    assert!(report.contains("state disabled"), "{report}");
    assert!(report.contains("auth needs auth"), "{report}");
    assert!(report.contains("1 server(s), ? tool(s)"), "{report}");

    // Enable it in the store ledger: the discovery-only render reflects that.
    let mut ledger = StoreLedger::load(&tree.store());
    ledger.state_mut("gdrive").enabled = true;
    ledger.save(&tree.store()).expect("ledger writable");
    let report = render_list(&input, &CategoryFilter::all(), false).expect("filter known");
    assert!(report.contains("state enabled"), "{report}");
}

#[test]
fn list_verbose_appends_the_per_server_tool_counts() {
    let tree = TempTree::new("list-verbose");
    let dirs = tree.dirs();
    let statuses = vec![status(
        "gdrive",
        ConnectorLifecycleState::Connected,
        AuthState::Satisfied,
        2,
    )];
    let input = ListInput {
        dirs: &dirs,
        statuses: &statuses,
    };
    let report = render_list(&input, &CategoryFilter::all(), true).expect("filter known");
    assert!(report.contains("gdrive.main (2)"), "{report}");
}

#[test]
fn list_reports_unsupported_labels_and_errors() {
    let tree = TempTree::new("list-unsupported");
    let dirs = tree.dirs();
    let mut descriptor = descriptor("gdrive", "Google Drive", "productivity");
    descriptor.unsupported = vec!["transport 'carrier-pigeon'".to_string()];
    let mut status = status(
        "gdrive",
        ConnectorLifecycleState::Errored,
        AuthState::Failed,
        0,
    );
    status.error = Some("auth failed".to_string());
    let input = ListInput {
        dirs: &dirs,
        statuses: &[status],
    };
    // The unsupported block reads the *scanned* descriptor; write it to the
    // store so the scan sees it.
    std::fs::write(
        tree.store().join("gdrive").join("connector.json"),
        serde_json::to_string_pretty(&descriptor).expect("serialisable"),
    )
    .expect("manifest writable");

    let report = render_list(&input, &CategoryFilter::all(), false).expect("filter known");
    assert!(report.contains("Unsupported capabilities:"), "{report}");
    assert!(report.contains("carrier-pigeon"), "{report}");
    assert!(report.contains("Errors:"), "{report}");
    assert!(report.contains("auth failed"), "{report}");
}

#[test]
fn list_category_filter_restricts_rows_and_reports_the_count() {
    let tree = TempTree::new("list-category");
    for (id, category) in [("gdrive", "productivity"), ("slack", "communication")] {
        let dir = tree.store().join(id);
        std::fs::create_dir_all(&dir).expect("connector dir");
        std::fs::write(
            dir.join("connector.json"),
            serde_json::to_string(&descriptor(id, id, category)).expect("serialisable"),
        )
        .expect("manifest writable");
    }
    let dirs = tree.dirs();
    let input = ListInput {
        dirs: &dirs,
        statuses: &[],
    };

    let filtered =
        render_list(&input, &CategoryFilter::parse("productivity"), false).expect("filter known");
    assert!(
        filtered.contains("category productivity (1 of 2)"),
        "{filtered}"
    );
    assert!(filtered.contains("- gdrive:"), "{filtered}");
    assert!(!filtered.contains("- slack:"), "{filtered}");

    // An unknown category is refused and changes no state (FR-041): the renderer
    // reports the `CategoryError` so the caller renders the family's `[err]` row.
    let unknown = render_list(&input, &CategoryFilter::parse("nonexistent"), false)
        .expect_err("an unknown category must be refused");
    assert_eq!(unknown, CategoryError::Unknown("nonexistent".to_string()));
    assert_eq!(unknown.to_string(), "unknown category nonexistent");
}

#[test]
fn list_refuses_an_unknown_category_but_all_and_installed_categories_apply() {
    let tree = TempTree::new("list-unknown-category");
    for (id, category) in [("gdrive", "productivity"), ("slack", "communication")] {
        let dir = tree.store().join(id);
        std::fs::create_dir_all(&dir).expect("connector dir");
        std::fs::write(
            dir.join("connector.json"),
            serde_json::to_string(&descriptor(id, id, category)).expect("serialisable"),
        )
        .expect("manifest writable");
    }
    let dirs = tree.dirs();
    let input = ListInput {
        dirs: &dirs,
        statuses: &[],
    };

    // `ALL` (any case) clears the filter and shows every row (FR-040).
    for sentinel in [ALL_CATEGORY, "all"] {
        let all = render_list(&input, &CategoryFilter::parse(sentinel), false)
            .expect("ALL is always known");
        assert!(all.contains("category ALL (2 of 2)"), "{all}");
        assert!(
            all.contains("- gdrive:") && all.contains("- slack:"),
            "{all}"
        );
    }

    // A known category applies case-insensitively (FR-039).
    let known = render_list(&input, &CategoryFilter::parse("PRODUCTIVITY"), false)
        .expect("a known category applies");
    assert!(known.contains("category PRODUCTIVITY (1 of 2)"), "{known}");
    assert!(known.contains("- gdrive:"), "{known}");

    // An unknown category is refused; the `[err]` row names it and is not a
    // silent empty result (FR-041).
    let refused = render_list(&input, &CategoryFilter::parse("nosuch"), false)
        .expect_err("an unknown category must be refused");
    let report = refused.report("list");
    assert!(report.starts_with("From: /connectors list"), "{report}");
    assert!(
        report.contains("[err] Unknown category `nosuch`."),
        "{report}"
    );
}

#[test]
fn list_with_no_connectors_installed_is_a_message() {
    let tree = TempTree::new("list-empty");
    let empty_dirs = StoreDirs {
        project: Some(tree.0.join("no-such-store")),
        global: None,
    };
    let input = ListInput {
        dirs: &empty_dirs,
        statuses: &[],
    };
    let report = render_list(&input, &CategoryFilter::all(), false).expect("filter known");
    assert!(report.contains("No connectors installed"), "{report}");
    assert!(report.contains("Total: 0 connector(s)"), "{report}");
    assert!(!report.contains("[err]"), "{report}");
}

#[test]
fn list_reports_a_broken_manifest_as_its_own_err_row() {
    let tree = TempTree::new("list-broken");
    std::fs::create_dir_all(tree.store().join("broken")).expect("dir");
    std::fs::write(
        tree.store().join("broken").join("connector.json"),
        b"{ not json",
    )
    .expect("manifest writable");
    let dirs = tree.dirs();
    let input = ListInput {
        dirs: &dirs,
        statuses: &[],
    };
    let report = render_list(&input, &CategoryFilter::all(), false).expect("filter known");
    assert!(
        report.contains("Unreadable connector directories:"),
        "{report}"
    );
    assert!(report.contains("- broken: [err]"), "{report}");
}

// -- render_search (FR-010, FR-025, FR-041) -----------------------------------

#[test]
fn search_matches_id_name_category_and_tags() {
    let mut tagged = descriptor("gdrive", "Google Drive", "productivity");
    tagged.tags = vec!["files".to_string(), "cloud".to_string()];
    tagged.auth = ragent_connectors::ConnectorAuthShape::Token;
    tagged.credential = Some("GDRIVE_TOKEN".to_string());
    let catalogue = vec![tagged, descriptor("slack", "Slack", "communication")];

    for query in ["gdrive", "drive", "productivity", "cloud"] {
        let report =
            render_search(query, &catalogue, &CategoryFilter::all()).expect("filter known");
        assert!(
            report.contains("- gdrive:"),
            "query `{query}` must match gdrive: {report}"
        );
    }

    let report = render_search("gdrive", &catalogue, &CategoryFilter::all()).expect("filter known");
    assert!(report.starts_with("From: /connectors search"), "{report}");
    assert!(report.contains("tags files, cloud"), "{report}");
    assert!(report.contains("auth token (GDRIVE_TOKEN)"), "{report}");
    assert!(
        report.contains("Total: 1 matching connector(s)"),
        "{report}"
    );
    assert!(report.is_ascii(), "{report}");
}

#[test]
fn search_empty_result_is_a_message_not_an_error() {
    let catalogue = vec![descriptor("gdrive", "Google Drive", "productivity")];
    let report =
        render_search("nothing-matches", &catalogue, &CategoryFilter::all()).expect("filter known");
    assert!(
        report.contains("No catalogue connector matches"),
        "{report}"
    );
    assert!(!report.contains("[err]"), "{report}");
}

#[test]
fn search_category_filter_restricts_the_result_set() {
    let catalogue = vec![
        descriptor("gdrive", "Google Drive", "productivity"),
        descriptor("slack", "Slack", "communication"),
    ];
    let report = render_search("a", &catalogue, &CategoryFilter::parse("communication"))
        .expect("filter known");
    assert!(report.contains("- slack:"), "{report}");
    assert!(!report.contains("- gdrive:"), "{report}");
    assert!(report.contains("category communication"), "{report}");

    // A filtered search that matches nothing under a known category is an empty
    // message, never an error (FR-041).
    let empty = render_search(
        "gdrive",
        &catalogue,
        &CategoryFilter::parse("communication"),
    )
    .expect("filter known");
    assert!(
        empty.contains("No catalogue connector matches \"gdrive\" in category communication."),
        "{empty}"
    );
    assert!(!empty.contains("[err]"), "{empty}");

    // A category the fetched catalogue does not declare is refused (FR-041).
    let refused = render_search("slack", &catalogue, &CategoryFilter::parse("nosuch"))
        .expect_err("an unknown category must be refused");
    assert_eq!(refused, CategoryError::Unknown("nosuch".to_string()));
    let rendered = refused.report("search");
    assert!(
        rendered.starts_with("From: /connectors search"),
        "{rendered}"
    );
    assert!(
        rendered.contains("[err] Unknown category `nosuch`."),
        "{rendered}"
    );
    assert!(rendered.is_ascii(), "{rendered}");
}

#[test]
fn resolve_filter_accepts_all_and_known_categories_only() {
    let known = ["productivity", "communication"];

    // `ALL` and a blank value clear the filter (FR-040).
    for sentinel in [ALL_CATEGORY, "all", "", "  "] {
        let filter =
            resolve_filter(&CategoryFilter::parse(sentinel), known).expect("ALL is always known");
        assert!(filter.is_all(), "`{sentinel}` must clear the filter");
    }

    // A known category resolves case-insensitively and keeps the supplied
    // spelling (FR-039).
    let known_filter = resolve_filter(&CategoryFilter::parse("PRODUCTIVITY"), known)
        .expect("a known category resolves");
    assert_eq!(known_filter.label(), "PRODUCTIVITY");

    // A value naming no known category is refused (FR-041).
    assert_eq!(
        resolve_filter(&CategoryFilter::parse("nosuch"), known),
        Err(CategoryError::Unknown("nosuch".to_string()))
    );
}

#[test]
fn search_reports_unsupported_labels() {
    let mut broken = descriptor("broken", "Broken", "developer");
    broken.unsupported = vec!["transport 'carrier-pigeon'".to_string()];
    let report = render_search("broken", &[broken], &CategoryFilter::all()).expect("filter known");
    assert!(
        report.contains("unsupported transport 'carrier-pigeon'"),
        "{report}"
    );
}

// -- report wording (FR-012, FR-013, FR-014, FR-019) --------------------------

fn connect_outcome(state: ConnectorLifecycleState) -> ConnectReport {
    ConnectReport {
        id: "gdrive".to_string(),
        state,
        auth: AuthState::Satisfied,
        servers: vec![ServerReport {
            server_id: "gdrive.main".to_string(),
            state: ServerState::Connected,
            tools: vec!["mcp_gdrive_main_read".to_string()],
            error: None,
        }],
        refused: Vec::new(),
        error: None,
    }
}

#[test]
fn enable_and_connect_reports_state_the_outcome() {
    let report = enable_report(&connect_outcome(ConnectorLifecycleState::Connected));
    assert!(
        report.starts_with("From: /connectors enable gdrive"),
        "{report}"
    );
    assert!(report.contains("state connected"), "{report}");
    assert!(report.contains("1 server(s) connected"), "{report}");
    assert!(report.contains("1 tool(s) exposed"), "{report}");
    assert!(
        report.contains("- gdrive.main: connected, 1 tool(s)"),
        "{report}"
    );

    let report = connect_report(&connect_outcome(ConnectorLifecycleState::Connected));
    assert!(
        report.starts_with("From: /connectors connect gdrive"),
        "{report}"
    );
    assert!(report.contains("state connected"), "{report}");
}

#[test]
fn disable_and_disconnect_reports_confirm_the_teardown_counts() {
    let outcome = DisableReport {
        id: "gdrive".to_string(),
        servers_disconnected: 2,
        tools_deregistered: 7,
    };
    let report = disable_report(&outcome);
    assert!(
        report.starts_with("From: /connectors disable gdrive"),
        "{report}"
    );
    assert!(report.contains("disconnected 2 server(s)"), "{report}");
    assert!(report.contains("deregistered 7 tool(s)"), "{report}");

    let report = disconnect_report(&outcome);
    assert!(
        report.starts_with("From: /connectors disconnect gdrive"),
        "{report}"
    );
    assert!(report.contains("still enabled"), "{report}");
}

#[test]
fn auth_report_never_echoes_a_secret() {
    let outcome = AuthOutcome {
        id: "gdrive".to_string(),
        state: AuthState::NeedsAuth,
        guidance: "run `/connectors auth gdrive` and paste a token (stored as GDRIVE_TOKEN)"
            .to_string(),
        stored: false,
    };
    let report = auth_report(&outcome);
    assert!(
        report.starts_with("From: /connectors auth gdrive"),
        "{report}"
    );
    assert!(report.contains("auth state needs auth"), "{report}");
    assert!(report.contains("GDRIVE_TOKEN"), "{report}");
    assert!(!report.contains("s3cr3t"), "{report}");
}

// -- the async dispatcher (FR-009, FR-010, FR-021) ----------------------------

/// A fake session environment: every lifecycle transition succeeds and the
/// catalogue is a fixed fixture.
struct FakeEnv {
    dirs: StoreDirs,
    config: ConnectorsConfig,
    statuses: Vec<ConnectorStatus>,
    catalogue: Result<Vec<ConnectorDescriptor>, String>,
    enabled_calls: Vec<String>,
    disabled_calls: Vec<String>,
    connected_calls: Vec<String>,
    disconnected_calls: Vec<String>,
    auth_calls: Vec<String>,
}

impl FakeEnv {
    fn new(dirs: StoreDirs) -> Self {
        Self {
            dirs,
            config: ConnectorsConfig::default(),
            statuses: Vec::new(),
            catalogue: Ok(Vec::new()),
            enabled_calls: Vec::new(),
            disabled_calls: Vec::new(),
            connected_calls: Vec::new(),
            disconnected_calls: Vec::new(),
            auth_calls: Vec::new(),
        }
    }
}

#[async_trait]
impl ConnectorCommandEnv for FakeEnv {
    fn config(&self) -> &ConnectorsConfig {
        &self.config
    }

    fn dirs(&self) -> &StoreDirs {
        &self.dirs
    }

    fn statuses(&self) -> Vec<ConnectorStatus> {
        self.statuses.clone()
    }

    async fn enable(&mut self, id: &str) -> Result<ConnectReport, LifecycleError> {
        self.enabled_calls.push(id.to_string());
        Ok(connect_outcome(ConnectorLifecycleState::Connected))
    }

    async fn disable(&mut self, id: &str) -> Result<DisableReport, LifecycleError> {
        self.disabled_calls.push(id.to_string());
        Ok(DisableReport {
            id: id.to_string(),
            servers_disconnected: 1,
            tools_deregistered: 2,
        })
    }

    async fn connect(&mut self, id: &str) -> Result<ConnectReport, LifecycleError> {
        self.connected_calls.push(id.to_string());
        Ok(connect_outcome(ConnectorLifecycleState::Connected))
    }

    async fn disconnect(&mut self, id: &str) -> Result<DisableReport, LifecycleError> {
        self.disconnected_calls.push(id.to_string());
        Ok(DisableReport {
            id: id.to_string(),
            servers_disconnected: 1,
            tools_deregistered: 2,
        })
    }

    async fn auth(&mut self, id: &str) -> Result<AuthOutcome, AuthOutcomeError> {
        self.auth_calls.push(id.to_string());
        Ok(AuthOutcome {
            id: id.to_string(),
            state: AuthState::NeedsAuth,
            guidance: "store the credential".to_string(),
            stored: false,
        })
    }

    async fn search_catalogue(&mut self) -> Result<Vec<ConnectorDescriptor>, String> {
        self.catalogue.clone()
    }
}

/// A probe that is never called by the management subcommands.
struct NeverProbe;

#[async_trait]
impl McpProbe for NeverProbe {
    async fn probe_connect(
        &mut self,
        _server_id: &str,
        _config: McpServerConfig,
    ) -> Result<Vec<ProbeTool>, String> {
        panic!("the management subcommands must not probe a server");
    }

    async fn probe_call(
        &mut self,
        _server_id: &str,
        _tool: &str,
        _args: serde_json::Value,
    ) -> Result<String, String> {
        panic!("the management subcommands must not invoke a tool");
    }

    async fn probe_disconnect(&mut self, _server_id: &str) -> Result<(), String> {
        panic!("the management subcommands must not disconnect a server");
    }
}

#[tokio::test]
async fn dispatcher_routes_each_management_subcommand() {
    let tree = TempTree::new("dispatch-routes");
    let mut env = FakeEnv::new(tree.dirs());
    env.statuses = vec![status(
        "gdrive",
        ConnectorLifecycleState::Connected,
        AuthState::Satisfied,
        1,
    )];
    env.catalogue = Ok(vec![descriptor("gdrive", "Google Drive", "productivity")]);
    let mut probe = NeverProbe;

    let list = run_connector_subcommand_env(&tree.0, &mut env, &mut probe, "list", "")
        .await
        .expect("list report");
    assert!(list.starts_with("From: /connectors list"), "{list}");

    let search = run_connector_subcommand_env(&tree.0, &mut env, &mut probe, "search", "drive")
        .await
        .expect("search report");
    assert!(search.contains("- gdrive:"), "{search}");

    // The `--category` argument is honoured end to end (FR-041): a known
    // category filters, `ALL` clears, an unknown value renders an `[err]` row.
    let filtered = run_connector_subcommand_env(
        &tree.0,
        &mut env,
        &mut probe,
        "list",
        "--category productivity",
    )
    .await
    .expect("filtered list report");
    assert!(
        filtered.contains("category productivity (1 of 1)"),
        "{filtered}"
    );

    let all = run_connector_subcommand_env(&tree.0, &mut env, &mut probe, "list", "--category ALL")
        .await
        .expect("ALL list report");
    assert!(all.contains("category ALL (1 of 1)"), "{all}");

    let refused =
        run_connector_subcommand_env(&tree.0, &mut env, &mut probe, "list", "--category nosuch")
            .await
            .expect("refusal report");
    assert!(
        refused.contains("[err] Unknown category `nosuch`."),
        "{refused}"
    );
    assert!(refused.starts_with("From: /connectors list"), "{refused}");

    let searched = run_connector_subcommand_env(
        &tree.0,
        &mut env,
        &mut probe,
        "search",
        "drive --category productivity",
    )
    .await
    .expect("filtered search report");
    assert!(
        searched.contains("category productivity (1 catalogue entries)"),
        "{searched}"
    );

    let refused_search = run_connector_subcommand_env(
        &tree.0,
        &mut env,
        &mut probe,
        "search",
        "drive --category nosuch",
    )
    .await
    .expect("refusal report");
    assert!(
        refused_search.contains("[err] Unknown category `nosuch`."),
        "{refused_search}"
    );

    for (sub, id) in [
        ("enable", "gdrive"),
        ("disable", "gdrive"),
        ("connect", "gdrive"),
        ("disconnect", "gdrive"),
        ("auth", "gdrive"),
    ] {
        let report = run_connector_subcommand_env(&tree.0, &mut env, &mut probe, sub, id)
            .await
            .unwrap_or_else(|| panic!("`{sub}` must render a report"));
        assert!(
            report.starts_with(&format!("From: /connectors {sub}")),
            "`{sub}` must attribute its output: {report}"
        );
        assert!(report.is_ascii(), "`{sub}` must be ASCII: {report}");
    }

    assert_eq!(env.enabled_calls, vec!["gdrive"]);
    assert_eq!(env.disabled_calls, vec!["gdrive"]);
    assert_eq!(env.connected_calls, vec!["gdrive"]);
    assert_eq!(env.disconnected_calls, vec!["gdrive"]);
    assert_eq!(env.auth_calls, vec!["gdrive"]);
}

#[tokio::test]
async fn dispatcher_malformed_arguments_render_an_err_row_and_change_no_state() {
    let tree = TempTree::new("dispatch-malformed");
    let mut env = FakeEnv::new(tree.dirs());
    let mut probe = NeverProbe;

    let report = run_connector_subcommand_env(&tree.0, &mut env, &mut probe, "enable", "")
        .await
        .expect("err report");
    assert!(report.contains("[err] Missing argument."), "{report}");
    assert!(report.starts_with("From: /connectors enable"), "{report}");
    assert!(
        env.enabled_calls.is_empty(),
        "no state change on a malformed arg"
    );

    let report = run_connector_subcommand_env(&tree.0, &mut env, &mut probe, "search", "")
        .await
        .expect("err report");
    assert!(report.contains("[err] Missing argument."), "{report}");
}

#[tokio::test]
async fn dispatcher_honours_the_master_switch() {
    let tree = TempTree::new("dispatch-disabled");
    let mut env = FakeEnv::new(tree.dirs());
    env.config = ConnectorsConfig {
        enabled: false,
        ..ConnectorsConfig::default()
    };
    let mut probe = NeverProbe;

    for (sub, rest) in [("list", ""), ("search", "drive"), ("enable", "gdrive")] {
        let report = run_connector_subcommand_env(&tree.0, &mut env, &mut probe, sub, rest)
            .await
            .unwrap_or_else(|| panic!("`{sub}` must render a report"));
        assert!(
            report.contains("[err]") && report.contains("disabled"),
            "`{sub}` must report the disabled subsystem: {report}"
        );
    }
    assert!(env.enabled_calls.is_empty());

    // `help` stays available while disabled (FR-021).
    let help = run_connector_subcommand_env(&tree.0, &mut env, &mut probe, "help", "")
        .await
        .expect("help renders");
    assert!(help.contains("command reference"), "{help}");
}

#[tokio::test]
async fn dispatcher_returns_none_for_non_management_subcommands() {
    let tree = TempTree::new("dispatch-other");
    let mut env = FakeEnv::new(tree.dirs());
    let mut probe = NeverProbe;

    for (sub, rest) in [
        ("add", "thing"),
        ("remove", "thing"),
        ("stores", ""),
        ("bogus", ""),
    ] {
        assert!(
            run_connector_subcommand_env(&tree.0, &mut env, &mut probe, sub, rest)
                .await
                .is_none(),
            "`{sub}` is not served by this dispatcher"
        );
    }
}

#[tokio::test]
async fn dispatcher_renders_a_fetch_failure_as_an_err_report() {
    let tree = TempTree::new("dispatch-fetch-fail");
    let mut env = FakeEnv::new(tree.dirs());
    env.catalogue = Err("catalogue endpoint must use https, not http".to_string());
    let mut probe = NeverProbe;

    let report = run_connector_subcommand_env(&tree.0, &mut env, &mut probe, "search", "drive")
        .await
        .expect("err report");
    assert!(report.starts_with("From: /connectors search"), "{report}");
    assert!(report.contains("[err]"), "{report}");
    assert!(report.contains("must use https"), "{report}");
}

#[tokio::test]
async fn dispatcher_lifecycle_refusal_is_an_err_report() {
    struct RefusingEnv(FakeEnv);

    #[async_trait]
    impl ConnectorCommandEnv for RefusingEnv {
        fn config(&self) -> &ConnectorsConfig {
            self.0.config()
        }
        fn dirs(&self) -> &StoreDirs {
            self.0.dirs()
        }
        fn statuses(&self) -> Vec<ConnectorStatus> {
            self.0.statuses()
        }
        async fn enable(&mut self, id: &str) -> Result<ConnectReport, LifecycleError> {
            Err(LifecycleError::UnknownConnector(id.to_string()))
        }
        async fn disable(&mut self, id: &str) -> Result<DisableReport, LifecycleError> {
            Err(LifecycleError::UnknownConnector(id.to_string()))
        }
        async fn connect(&mut self, id: &str) -> Result<ConnectReport, LifecycleError> {
            Err(LifecycleError::UnknownConnector(id.to_string()))
        }
        async fn disconnect(&mut self, id: &str) -> Result<DisableReport, LifecycleError> {
            Err(LifecycleError::UnknownConnector(id.to_string()))
        }
        async fn auth(&mut self, id: &str) -> Result<AuthOutcome, AuthOutcomeError> {
            Err(AuthOutcomeError::UnknownConnector(id.to_string()))
        }
    }

    let tree = TempTree::new("dispatch-refusal");
    let dirs = tree.dirs();
    let mut env = RefusingEnv(FakeEnv::new(dirs));
    let mut probe = NeverProbe;

    let report = run_connector_subcommand_env(&tree.0, &mut env, &mut probe, "enable", "ghost")
        .await
        .expect("err report");
    assert!(
        report.starts_with("From: /connectors enable ghost"),
        "{report}"
    );
    assert!(report.contains("[err]"), "{report}");
    assert!(report.contains("unknown connector id ghost"), "{report}");

    let report = run_connector_subcommand_env(&tree.0, &mut env, &mut probe, "auth", "ghost")
        .await
        .expect("err report");
    assert!(report.contains("unknown connector id ghost"), "{report}");
}

// -- ConnectorError/LifecycleError plumbing used above ------------------------

#[test]
fn management_lifecycle_error_plumbs_through_connector_error() {
    let error = LifecycleError::Store(ConnectorError::Io("boom".to_string()));
    assert!(error.to_string().contains("boom"));
}

#[test]
fn tool_count_renders_unknown_as_a_question_mark() {
    assert_eq!(ToolCount::Known(4).render(), "4");
    assert_eq!(ToolCount::Unknown.render(), "?");
}

#[test]
fn list_row_tools_total_is_none_when_any_count_is_unknown() {
    let mut counts = BTreeMap::new();
    counts.insert("a.main".to_string(), ToolCount::Known(2));
    counts.insert("a.other".to_string(), ToolCount::Unknown);
    let row = ragent_connectors::ListRow {
        id: "a".to_string(),
        name: "A".to_string(),
        category: "developer".to_string(),
        state: ConnectorLifecycleState::Connected,
        auth: "none".to_string(),
        server_count: 2,
        tool_counts: counts,
        unsupported: Vec::new(),
        error: None,
    };
    assert_eq!(row.tools_total(), None);
}

/// Unused helper kept for parity with the store-scan tests: asserts the store
/// path the fixture builds is the one the dispatcher reads.
#[test]
fn temp_tree_store_matches_the_dispatcher_dirs() {
    let tree = TempTree::new("store-parity");
    assert_eq!(tree.dirs().project.as_deref(), Some(tree.store().as_path()));
    assert!(Path::new(&tree.store()).is_dir());
}
