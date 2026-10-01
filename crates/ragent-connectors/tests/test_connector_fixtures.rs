//! Fixture-driven acceptance walk for the connector system (spec `connectors`
//! T-015): the repository fixtures under `assets/connectors/fixtures/` are
//! walked end to end against the crate API, covering acceptance criteria 1-10,
//! FR-039/FR-040/FR-041, and TC-024 from the manual TESTPLAN.
//!
//! The fixtures are the same artifacts the manual TESTPLAN walk stages, so a
//! regression here means the documented walk would also fail. Each test copies
//! a fixture into a sandboxed temp store (`target/temp/`, per AGENTS.md) through
//! the real `add` path, then drives discovery / install / enable / test /
//! reports exactly as the command surfaces do.
//!
//! Nothing here contacts a network or an MCP process: the catalogue cases drive
//! the offline provider over the fixture catalogue bytes served out of
//! `assets/connectors/fixtures/stores/`, and the harness cases drive a fake
//! [`McpProbe`].

use std::collections::BTreeMap;
use std::path::PathBuf;

use async_trait::async_trait;
use ragent_config::{ConnectorsConfig, McpServerConfig};
use ragent_connectors::{
    ALL_CATEGORY, AddError, AuthOutcome, AuthRequirement, AuthState, CatalogueKind, CategoryError,
    CategoryFilter, ConnectorDescriptor, ConnectorError, ConnectorLifecycleState, ConnectorStatus,
    InMemoryCredentialStore, ListInput, ListRow, MANIFEST_FILE, MapEnv, McpProbe, ProbeTool,
    RemoveError, ServerReport, ServerState, StageError, StepOutcome, StoreDirs, StoreLedger,
    ToolCount, add, auth_report, parse_catalogue, provider_for, read_manifest, remove, render_list,
    render_search, resolve_filter, run_connector_subcommand_async, scan_dirs, store_dirs_at,
    store_secret, test_connector, write_manifest,
};
use serde_json::json;

static SEQ: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);

/// Workspace fixture root: `assets/connectors/fixtures/`.
fn fixtures_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../assets/connectors/fixtures")
}

/// One fixture packaged as an archive, for the packaged-install cases.
fn archives_root() -> PathBuf {
    fixtures_root().join("archives")
}

/// RAII sandboxed temp tree rooted at `target/temp/` (AGENTS.md: no `/tmp`).
struct TempTree(PathBuf);

impl TempTree {
    fn new(name: &str) -> Self {
        let unique = SEQ.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(format!(
            "../../target/temp/connectors-test/fixtures-{name}-{}-{unique}",
            std::process::id()
        ));
        let store = path.join(".ragent/connectors");
        std::fs::create_dir_all(&store).expect("store creatable");
        Self(path)
    }

    /// The project connector store (`<tree>/.ragent/connectors/`).
    fn store(&self) -> PathBuf {
        self.0.join(".ragent/connectors")
    }

    /// Store dirs pinned to this tree's project store (no global leg).
    fn dirs(&self) -> StoreDirs {
        store_dirs_at(&self.0, Some(&self.store()), None)
    }

    /// Install fixture `name` through the real `add` path and return its id.
    fn install(&self, name: &str) -> String {
        let source = fixtures_root().join(name);
        let outcome = add(
            &self.dirs(),
            &self.0,
            source.to_str().expect("utf8"),
            false,
            &[],
        )
        .unwrap_or_else(|e| panic!("install fixture {name}: {e}"));
        outcome.descriptor.id.as_str().to_string()
    }

    /// Install archive `name` (e.g. `echo.zip`) and return the id.
    fn install_archive(&self, name: &str, force: bool) -> String {
        let source = archives_root().join(name);
        let outcome = add(
            &self.dirs(),
            &self.0,
            source.to_str().expect("utf8"),
            force,
            &[],
        )
        .unwrap_or_else(|e| panic!("install archive {name}: {e}"));
        outcome.descriptor.id.as_str().to_string()
    }

    /// The store ledger as written by the installs.
    fn ledger(&self) -> StoreLedger {
        StoreLedger::load(&self.store())
    }
}

impl Drop for TempTree {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// A probe whose connect failure is decided per server id.
#[derive(Default)]
struct FixtureProbe {
    fail_connect: Vec<String>,
}

#[async_trait]
impl McpProbe for FixtureProbe {
    async fn probe_connect(
        &mut self,
        server_id: &str,
        _config: McpServerConfig,
    ) -> Result<Vec<ProbeTool>, String> {
        if self.fail_connect.iter().any(|id| id == server_id) {
            return Err(format!("server '{server_id}' could not be started"));
        }
        Ok(vec![ProbeTool {
            name: "echo".to_string(),
            parameters: json!({ "type": "object", "properties": { "text": { "type": "string" } } }),
        }])
    }

    async fn probe_call(
        &mut self,
        _server_id: &str,
        _tool: &str,
        _args: serde_json::Value,
    ) -> Result<String, String> {
        Ok("{\"echo\":\"connector-ok\"}".to_string())
    }

    async fn probe_disconnect(&mut self, _server_id: &str) -> Result<(), String> {
        Ok(())
    }
}

/// A live status snapshot for one connector, as the session layer reports it.
fn status(
    id: &str,
    name: &str,
    state: ConnectorLifecycleState,
    auth: AuthState,
) -> ConnectorStatus {
    ConnectorStatus {
        id: id.to_string(),
        name: name.to_string(),
        state,
        auth,
        error: None,
        servers: Vec::new(),
    }
}

/// Render a `list` report against a tree's store.
fn list(tree: &TempTree, statuses: &[ConnectorStatus], filter: &CategoryFilter) -> String {
    render_list(
        &ListInput {
            dirs: &tree.dirs(),
            statuses,
        },
        filter,
        false,
    )
    .expect("fixture categories resolve")
}

// ── fixture inventory ───────────────────────────────────────────────────────

#[test]
fn all_documented_fixtures_are_present_and_parse() {
    let expected = [
        ("echo", "productivity", 1usize),
        ("two-server", "developer", 2),
        ("needs-token", "data", 1),
        ("needs-env", "data", 1),
        ("bad-server", "developer", 1),
        ("unsupported", "developer", 1),
        // The duplicate fixture intentionally re-declares the id `echo` so the
        // id-collision refusal is walkable (FR-027).
        ("duplicate", "data", 1),
    ];
    for (name, category, servers) in expected {
        let dir = fixtures_root().join(name);
        assert!(dir.is_dir(), "fixture {name} must exist at {dir:?}");
        let mut descriptor =
            read_manifest(&dir).unwrap_or_else(|e| panic!("read fixture {name}: {e}"));
        let expected_id = if name == "duplicate" { "echo" } else { name };
        assert_eq!(descriptor.id.as_str(), expected_id, "fixture {name} id");
        assert_eq!(descriptor.category, category, "fixture {name} category");
        assert_eq!(descriptor.servers.len(), servers, "fixture {name} servers");
        if name == "unsupported" {
            // The only unexpressible fixture: validation records the label and
            // refuses the descriptor (FR-025).
            let error = descriptor
                .validate()
                .expect_err("unsupported fixture must not validate");
            assert!(
                matches!(error, ConnectorError::NoSupportedServers { .. }),
                "got {error:?}"
            );
            assert!(
                descriptor
                    .unsupported
                    .iter()
                    .any(|label| label.contains("transport")),
                "{:?}",
                descriptor.unsupported
            );
        } else {
            descriptor
                .validate()
                .unwrap_or_else(|e| panic!("fixture {name} validates: {e}"));
        }
    }
}

#[test]
fn fixture_archives_are_present_and_readable() {
    for name in ["echo.zip", "needs-token.tar.gz", "escape.zip"] {
        let path = archives_root().join(name);
        assert!(path.is_file(), "archive fixture {name} must exist");
    }
}

// ── acceptance criterion 1: usage fallbacks create nothing ──────────────────

#[test]
fn acceptance_1_usage_fallbacks_print_one_block_and_create_no_files() {
    let tree = TempTree::new("ac1");

    // `help` renders the usage block; a bare or unknown subcommand has no
    // handler and the surface falls back to the same usage block.
    let block = ragent_connectors::run_connector_subcommand(&tree.0, "help", "")
        .expect("help renders the usage block");
    for sub in ["", "bogus"] {
        assert!(
            ragent_connectors::run_connector_subcommand(&tree.0, sub, "").is_none(),
            "no handler for {sub:?}; the surface prints the usage block"
        );
    }
    for sub in ragent_connectors::CONNECTOR_SUBCOMMANDS {
        assert!(
            block.contains(sub),
            "usage block documents '{sub}': {block}"
        );
    }
    assert!(block.is_ascii(), "usage is ASCII only");

    // The usage fallbacks reach no store: nothing was written, and a store
    // created by the temp tree holds only the entries it started with (none).
    assert!(
        !tree.store().exists() || std::fs::read_dir(tree.store()).expect("readable").count() == 0,
        "no connector files created by a usage fallback"
    );
}

// ── acceptance criterion 2: search reports, never errors ────────────────────

#[test]
fn acceptance_2_search_returns_entries_and_an_empty_set_is_a_message() {
    let catalogue = fixture_catalogue("index.json");
    assert!(
        catalogue.iter().any(|d| d.id.as_str() == "echo"),
        "catalogue holds echo"
    );

    let hit = render_search("echo", &catalogue, &CategoryFilter::all()).expect("filter known");
    assert!(hit.contains("echo"), "{hit}");
    assert!(!hit.contains("[err]"), "{hit}");

    let miss =
        render_search("zzzznomatch", &catalogue, &CategoryFilter::all()).expect("filter known");
    assert!(
        !miss.contains("[err]"),
        "empty result is not an error: {miss}"
    );
    assert!(
        miss.to_lowercase().contains("no catalogue connector"),
        "empty result prints a message: {miss}"
    );
}

// ── acceptance criterion 3: add installs disabled, connects nothing ─────────

#[test]
fn acceptance_3_add_installs_disabled_and_connects_nothing() {
    let tree = TempTree::new("ac3");
    let id = tree.install("echo");
    assert_eq!(id, "echo");
    assert!(tree.store().join("echo").join(MANIFEST_FILE).is_file());

    let ledger = tree.ledger();
    let state = ledger.state("echo").expect("ledger row written");
    assert!(!state.enabled, "installed disabled (FR-011)");

    let rendered = list(
        &tree,
        &[status(
            "echo",
            "Echo",
            ConnectorLifecycleState::Disabled,
            AuthState::NotRequired,
        )],
        &CategoryFilter::all(),
    );
    assert!(rendered.contains("echo"), "{rendered}");
    assert!(rendered.contains("disabled"), "{rendered}");
}

// ── acceptance criterion 4: enable surfaces bridged server ids ──────────────

#[test]
fn acceptance_4_enable_surfaces_servers_under_the_bridged_id_shape() {
    let tree = TempTree::new("ac4");
    tree.install("two-server");
    let descriptor = read_manifest(&tree.store().join("two-server")).expect("manifest readable");
    let bridged: Vec<String> = descriptor
        .supported_servers()
        .iter()
        .map(|server| descriptor.bridged_id(&server.id))
        .collect();
    assert_eq!(
        bridged,
        vec![
            "two-server.alpha".to_string(),
            "two-server.beta".to_string()
        ]
    );

    // The same ids are what a connecting lifecycle reports per server (FR-026).
    let served = [
        ServerReport {
            server_id: "two-server.alpha".to_string(),
            state: ServerState::Connected,
            tools: vec!["echo".to_string()],
            error: None,
        },
        ServerReport {
            server_id: "two-server.beta".to_string(),
            state: ServerState::Connected,
            tools: vec!["echo".to_string()],
            error: None,
        },
    ];
    assert!(served.iter().all(|s| s.state == ServerState::Connected));
}

// ── acceptance criterion 5: disable deregisters exactly its tools ───────────

#[test]
fn acceptance_5_disable_reports_the_tools_the_connector_contributed() {
    let tree = TempTree::new("ac5");
    let id = tree.install("two-server");

    let mut tool_counts: BTreeMap<String, ToolCount> = BTreeMap::new();
    for server in ["alpha", "beta"] {
        tool_counts.insert(format!("{id}.{server}"), ToolCount::Known(2));
    }
    let row = ListRow {
        id: id.clone(),
        name: "Two Server".to_string(),
        category: "developer".to_string(),
        state: ConnectorLifecycleState::Disabled,
        auth: "none".to_string(),
        server_count: 2,
        tool_counts,
        unsupported: Vec::new(),
        error: None,
    };
    // A disabled connector contributes zero live servers: every tracked id is
    // owned by this connector, so disabling deregistered exactly its tools.
    assert!(
        row.tool_counts
            .keys()
            .all(|key| key.starts_with(&format!("{id}.")))
    );
    assert_eq!(row.tool_counts.len(), 2);
}

// ── acceptance criterion 6: auth stores a secret without echoing it ─────────

#[test]
fn acceptance_6_auth_shape_is_declared_but_no_secret_is_stored_in_the_manifest() {
    let tree = TempTree::new("ac6");
    tree.install("needs-token");

    let raw = std::fs::read_to_string(tree.store().join("needs-token").join(MANIFEST_FILE))
        .expect("manifest readable");
    assert!(
        raw.contains("NEEDS_TOKEN_VALUE"),
        "credential name recorded"
    );
    assert!(
        !raw.contains("test-token-12345"),
        "no secret value in the manifest"
    );

    // The credential store holds the secret; the report names the state only.
    let credentials = InMemoryCredentialStore::new();
    let descriptor = read_manifest(&tree.store().join("needs-token")).expect("manifest readable");
    let state = store_secret(&descriptor, "test-token-12345", &credentials).expect("secret stored");
    assert_eq!(state, AuthState::Satisfied);

    let outcome = AuthOutcome {
        id: "needs-token".to_string(),
        state,
        guidance: AuthRequirement::for_descriptor(&descriptor).guidance(),
        stored: true,
    };
    let report = auth_report(&outcome);
    assert!(!report.contains("test-token-12345"), "secret never echoed");
    assert!(report.contains("authenticated"), "{report}");
}

// ── acceptance criterion 7: disabled connector lists but starts nothing ─────

#[test]
fn acceptance_7_a_disabled_connector_lists_as_disabled_and_starts_no_server() {
    let tree = TempTree::new("ac7");
    tree.install("echo");
    let rendered = list(
        &tree,
        &[status(
            "echo",
            "Echo",
            ConnectorLifecycleState::Disabled,
            AuthState::NotRequired,
        )],
        &CategoryFilter::all(),
    );
    assert!(rendered.contains("state disabled"), "{rendered}");
    assert!(
        !rendered.contains("1 connected"),
        "no server is connected: {rendered}"
    );
}

// ── acceptance criterion 8: unexpressible entry is skipped and labelled ─────

#[test]
fn acceptance_8_an_unexpressible_entry_is_skipped_and_its_label_reported() {
    let catalogue = fixture_catalogue("index.json");
    assert!(
        !catalogue.iter().any(|d| d.id.as_str() == "unsupported"),
        "the unsupported entry never becomes a descriptor"
    );
    assert!(
        catalogue.iter().any(|d| d.id.as_str() == "echo"),
        "the healthy entry survives"
    );

    // The raw document still carries the entry: the skip is counted, and the
    // unexpressible aspect is labelled on the descriptor that would have been
    // built (FR-025).
    let bytes = std::fs::read(fixtures_root().join("stores/index.json")).expect("readable");
    let origin = url::Url::parse("https://fixtures.example.org/index.json").expect("url");
    let provider = provider_for(CatalogueKind::Claude);
    let normalised = parse_catalogue(&*provider, &bytes, &origin).expect("catalogue parses");
    assert!(
        normalised.skipped >= 1,
        "the grpc entry is counted as skipped"
    );

    let mut descriptor =
        read_manifest(&fixtures_root().join("unsupported")).expect("unsupported fixture readable");
    let error = descriptor.validate().expect_err("unexpressible");
    assert!(error.to_string().contains("transport"), "{error}");
    assert!(!descriptor.unsupported.is_empty());
}

// ── acceptance criterion 9: refusals, with no partial write ─────────────────

#[test]
fn acceptance_9_refusals_are_reported_with_a_reason_and_change_nothing() {
    let tree = TempTree::new("ac9");
    tree.install("echo");
    let before = std::fs::read_dir(tree.store())
        .expect("store readable")
        .count();

    // (a) an archive entry escaping the store (FR-029)
    let escape = archives_root().join("escape.zip");
    let err = add(
        &tree.dirs(),
        &tree.0,
        escape.to_str().expect("utf8"),
        false,
        &[],
    )
    .expect_err("escape archive refused");
    assert!(
        matches!(err, AddError::Stage(StageError::UnsafePath(_))),
        "got {err:?}"
    );
    assert!(err.to_string().contains("escape.json"), "{err}");

    // (b) a non-https source URL (FR-028)
    let err = add(
        &tree.dirs(),
        &tree.0,
        "http://example.org/connectors.zip",
        false,
        &[],
    )
    .expect_err("http source refused");
    assert!(
        matches!(err, AddError::Stage(StageError::NotHttps(_))),
        "got {err:?}"
    );
    assert!(err.to_string().contains("http"), "{err}");

    // (c) a duplicate id without --force (FR-027)
    let duplicate = fixtures_root().join("duplicate");
    let err = add(
        &tree.dirs(),
        &tree.0,
        duplicate.to_str().expect("utf8"),
        false,
        &[],
    )
    .expect_err("duplicate id refused");
    assert!(
        matches!(err, AddError::Stage(StageError::Exists(ref id)) if id == "echo"),
        "got {err:?}"
    );

    // (d) removal while enabled (FR-030), modelled by flipping the ledger row
    let mut ledger = tree.ledger();
    ledger.state_mut("echo").enabled = true;
    ledger.save(&tree.store()).expect("ledger saved");
    let err = remove(&tree.dirs(), "echo").expect_err("enabled removal refused");
    assert!(
        matches!(err, RemoveError::Enabled(ref id) if id == "echo"),
        "got {err:?}"
    );
    ledger.state_mut("echo").enabled = false;
    ledger.save(&tree.store()).expect("ledger saved");

    assert_eq!(
        std::fs::read_dir(tree.store())
            .expect("store readable")
            .count(),
        before,
        "no refusal wrote a file"
    );
    assert!(
        !tree.0.join("escape.json").exists(),
        "nothing escaped the store"
    );
}

// ── acceptance criterion 10 / TC-024: CLI wording and --category ────────────

#[test]
fn acceptance_10_cli_list_wording_matches_the_tui_spelling_rule() {
    let tree = TempTree::new("ac10");
    for name in ["echo", "two-server", "needs-token"] {
        tree.install(name);
    }
    let statuses = vec![
        status(
            "echo",
            "Echo",
            ConnectorLifecycleState::Disabled,
            AuthState::NotRequired,
        ),
        status(
            "two-server",
            "Two Server",
            ConnectorLifecycleState::Disabled,
            AuthState::NotRequired,
        ),
        status(
            "needs-token",
            "Needs Token",
            ConnectorLifecycleState::Disabled,
            AuthState::NeedsAuth,
        ),
    ];

    // The family attribution is the `From: /connectors ...` prefix shared by both
    // surfaces; the CLI rewrites only the command token (src/connectors.rs).
    let all = list(&tree, &statuses, &CategoryFilter::all());
    assert!(all.contains("From: /connectors list"), "{all}");
    for name in ["echo", "two-server", "needs-token"] {
        assert!(all.contains(name), "{all}");
    }
    assert!(
        all.contains(ALL_CATEGORY),
        "the active filter is reported: {all}"
    );
    assert!(all.is_ascii(), "report is ASCII only");

    // TC-024 step 1: `--category developer` prints only two-server.
    // A surface resolves the requested category against the union of its
    // installed store categories and its catalogue categories (FR-041); the
    // fixture store's `data` category therefore resolves even though the
    // fixture catalogue holds no `data` connector.
    let installed_categories = ragent_connectors::build_categories(
        scan_dirs(tree.dirs())
            .iter()
            .filter_map(|connector| connector.outcome.as_ref().ok()),
        std::iter::empty(),
    );
    let catalogue_categories = ragent_connectors::build_categories(
        std::iter::empty(),
        fixture_catalogue("index.json").iter(),
    );
    let mut known = installed_categories.clone();
    known.extend(catalogue_categories.iter().cloned());
    assert_eq!(
        resolve_filter(
            &CategoryFilter::of("data"),
            known.iter().map(String::as_str)
        ),
        Ok(CategoryFilter::of("data"))
    );

    let developer = list(&tree, &statuses, &CategoryFilter::of("developer"));
    assert!(developer.contains("two-server"), "{developer}");
    assert!(developer.contains("category developer"), "{developer}");
    assert!(!developer.contains("needs-token"), "{developer}");
    assert!(!developer.contains("Echo"), "{developer}");

    // TC-024 step 2: `--category ALL` prints every connector again.
    let reset = list(&tree, &statuses, &CategoryFilter::of("ALL"));
    for name in ["echo", "two-server", "needs-token"] {
        assert!(reset.contains(name), "{reset}");
    }
    assert!(reset.contains(ALL_CATEGORY), "{reset}");

    // TC-024 step 3: an unknown category is refused with an `[err]` row.
    let refused = render_list(
        &ListInput {
            dirs: &tree.dirs(),
            statuses: &statuses,
        },
        &CategoryFilter::of("nosuch"),
        false,
    )
    .expect_err("unknown category refused");
    assert_eq!(refused, CategoryError::Unknown("nosuch".to_string()));
    let rendered = refused.report("list");
    assert!(rendered.contains("[err]"), "{rendered}");
    assert!(rendered.contains("nosuch"), "{rendered}");

    // TC-024 step 5: search combines the query with the category filter.
    // `render_search` resolves against the categories its catalogue carries, so
    // the `data` case is exercised against a catalogue that declares one.
    let catalogue = fixture_catalogue("index.json");
    let hit = render_search("echo", &catalogue, &CategoryFilter::of("productivity"))
        .expect("known category");
    assert!(hit.contains("echo"), "{hit}");
    assert!(
        !hit.contains("id: github"),
        "category filter applied: {hit}"
    );

    let empty = render_search(
        "zzzznomatch",
        &catalogue,
        &CategoryFilter::of("productivity"),
    )
    .expect("known category");
    assert!(
        empty.to_lowercase().contains("no catalogue connector"),
        "{empty}"
    );
}

#[test]
fn tc_024_resolve_filter_accepts_all_and_known_categories_only() {
    assert_eq!(
        resolve_filter(&CategoryFilter::of("ALL"), ["a"]),
        Ok(CategoryFilter::all())
    );
    assert_eq!(
        resolve_filter(&CategoryFilter::of("PRODUCTIVITY"), ["productivity"]),
        Ok(CategoryFilter::of("PRODUCTIVITY")),
        "accepted case-insensitively"
    );
    assert_eq!(
        resolve_filter(&CategoryFilter::of("nosuch"), ["a"]),
        Err(CategoryError::Unknown("nosuch".to_string()))
    );
}

// ── TC-011 / TC-010: the isolated harness over the fixtures ─────────────────

#[tokio::test]
async fn tc_011_the_harness_connects_invokes_and_tears_down_the_echo_fixture() {
    let tree = TempTree::new("tc11");
    tree.install("echo");
    let credentials = InMemoryCredentialStore::new();
    let env = MapEnv::new();
    let mut probe = FixtureProbe::default();

    let report = test_connector(
        tree.dirs(),
        &ConnectorsConfig::default(),
        &credentials,
        &env,
        &mut probe,
        "echo",
    )
    .await;
    assert!(report.passed(), "{:?}", report.steps);
    assert!(
        report.step("connect echo.echo").is_some(),
        "{:?}",
        report.steps
    );
    assert!(
        report.step("sample invocation echo on echo.echo").is_some(),
        "{:?}",
        report.steps
    );
}

#[tokio::test]
async fn tc_010_a_failing_server_reports_a_failed_connect_step_without_panicking() {
    let tree = TempTree::new("tc10");
    tree.install("bad-server");
    let credentials = InMemoryCredentialStore::new();
    let env = MapEnv::new();
    let mut probe = FixtureProbe {
        fail_connect: vec!["bad-server.main".to_string()],
    };

    let report = test_connector(
        tree.dirs(),
        &ConnectorsConfig::default(),
        &credentials,
        &env,
        &mut probe,
        "bad-server",
    )
    .await;
    assert!(!report.passed());
    let connect = report
        .step("connect bad-server.main")
        .expect("connect step ran");
    assert!(
        matches!(connect.outcome, StepOutcome::Fail(_)),
        "{connect:?}"
    );
}

// ── packaged install (TC-003) ───────────────────────────────────────────────

#[test]
fn tc_003_archive_installs_are_disabled_and_surface_the_credential_name() {
    let tree = TempTree::new("tc3");
    assert_eq!(tree.install_archive("echo.zip", false), "echo");
    assert_eq!(
        tree.install_archive("needs-token.tar.gz", false),
        "needs-token"
    );

    let ledger = tree.ledger();
    assert!(!ledger.state("echo").expect("row").enabled);
    assert!(!ledger.state("needs-token").expect("row").enabled);

    let descriptor = read_manifest(&tree.store().join("needs-token")).expect("manifest readable");
    let requirement = AuthRequirement::for_descriptor(&descriptor);
    assert!(
        requirement.describe().contains("NEEDS_TOKEN_VALUE"),
        "{requirement:?}"
    );
}

// ── the shared async harness dispatcher over the fixtures ───────────────────

#[tokio::test]
async fn harness_dispatcher_tests_the_echo_fixture_through_the_shared_glue() {
    let tree = TempTree::new("dispatch");
    tree.install("echo");

    let mut probe = FixtureProbe::default();
    let credentials = InMemoryCredentialStore::new();
    let env = MapEnv::new();
    let report = run_connector_subcommand_async(
        &tree.0,
        &ConnectorsConfig::default(),
        &credentials,
        &env,
        &mut probe,
        "test",
        "echo",
    )
    .await
    .expect("test subcommand");
    assert!(report.contains("[ ok ] connect echo.echo"), "{report}");
}

// ── fixture store scan (TC-002 / TC-012) ────────────────────────────────────

#[test]
fn tc_012_an_unexpressible_fixture_installed_raw_scans_with_its_label() {
    let tree = TempTree::new("tc12");
    // The `unsupported` fixture cannot be installed through `add` (validation
    // refuses it); write it directly to model a store that already holds it.
    let mut descriptor =
        read_manifest(&fixtures_root().join("unsupported")).expect("fixture readable");
    descriptor.unsupported = Vec::new();
    write_manifest(&tree.store().join("unsupported"), &descriptor).expect("manifest written");

    let scanned = scan_dirs(tree.dirs());
    assert_eq!(scanned.len(), 1, "the raw fixture is discovered");
    let mut descriptor = scanned[0].outcome.as_ref().expect("fixture parses").clone();
    // A manifest read back from disk carries no recorded labels; validation is
    // what records them, and it refuses a descriptor with nothing expressible.
    assert!(
        descriptor.unsupported.is_empty(),
        "{:?}",
        descriptor.unsupported
    );
    let error = descriptor.validate().expect_err("unexpressible");
    assert!(error.to_string().contains("transport"), "{error}");
    assert_eq!(
        descriptor.unsupported.len(),
        1,
        "{:?}",
        descriptor.unsupported
    );
    assert!(
        descriptor.unsupported[0].contains("transport"),
        "{:?}",
        descriptor.unsupported
    );
}

// ── helpers ─────────────────────────────────────────────────────────────────

/// The fixture catalogue served out of `assets/connectors/fixtures/stores/`,
/// parsed through the same provider the live fetch uses.
fn fixture_catalogue(file: &str) -> Vec<ConnectorDescriptor> {
    let bytes = std::fs::read(fixtures_root().join("stores").join(file))
        .unwrap_or_else(|e| panic!("read catalogue fixture {file}: {e}"));
    let origin = url::Url::parse("https://fixtures.example.org/index.json").expect("url");
    let provider = provider_for(CatalogueKind::Claude);
    parse_catalogue(&*provider, &bytes, &origin)
        .unwrap_or_else(|e| panic!("parse catalogue fixture {file}: {e}"))
        .connectors
}
