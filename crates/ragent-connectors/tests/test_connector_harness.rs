//! Tests for the `/connectors test` isolated harness (spec `connectors` T-013;
//! FR-015, FR-016): isolated connect, one sample invocation, per-step
//! `[ ok ]`/`[fail]` reporting with wall-clock time, and teardown that never
//! touches the live session.
//!
//! Every test is offline and rooted under `target/temp/` (no `/tmp`, per
//! AGENTS.md). A fake [`McpProbe`] records the connect/invoke/disconnect calls
//! and can be told to fail a named server or tool, so the per-step,
//! containment, and teardown behaviour is asserted without a real MCP process.

mod support;

use support::TempTree;

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;
use std::sync::Mutex;

use async_trait::async_trait;
use ragent_config::ConnectorsConfig;
use ragent_connectors::{
    ConnectorAuthShape, ConnectorDescriptor, ConnectorId, ConnectorServer, HarnessError,
    InMemoryCredentialStore, MANIFEST_FILE, MapEnv, McpProbe, ProbeTool, StepOutcome, StoreLedger,
    render_report, run_connector_subcommand_async, run_harness, sample_for_schema, store_secret,
    test_connector,
};
use serde_json::{Value as JsonValue, json};

/// A descriptor with the given id and servers.
fn descriptor(id: &str, servers: Vec<ConnectorServer>) -> ConnectorDescriptor {
    ConnectorDescriptor {
        id: ConnectorId::new(id).expect("valid id"),
        name: id.to_string(),
        description: String::new(),
        category: String::new(),
        tags: Vec::new(),
        source: String::new(),
        provenance: Default::default(),
        auth: ConnectorAuthShape::None,
        auth_scope: Vec::new(),
        credential: None,
        servers,
        unsupported: Vec::new(),
    }
}

/// A stdio server with the given id and command.
fn stdio(id: &str, command: &str) -> ConnectorServer {
    ConnectorServer {
        id: id.to_string(),
        transport: "stdio".to_string(),
        command: Some(command.to_string()),
        args: Vec::new(),
        env: Default::default(),
        url: None,
        headers: Default::default(),
    }
}

/// Write a descriptor as a connector manifest in `store/<id>/connector.json`.
fn write_connector(store: &Path, descriptor: &ConnectorDescriptor) {
    let dir = store.join(descriptor.id.as_str());
    std::fs::create_dir_all(&dir).expect("connector dir creatable");
    ragent_connectors::write_manifest(&dir, descriptor).expect("manifest writable");
    assert!(dir.join(MANIFEST_FILE).is_file());
}

/// One advertised tool with a JSON schema.
fn tool(name: &str, parameters: JsonValue) -> ProbeTool {
    ProbeTool {
        name: name.to_string(),
        parameters,
    }
}

/// A fake probe: records calls, returns per-server tools, fails named ids.
#[derive(Default)]
struct FakeProbe {
    fail_connect: BTreeSet<String>,
    fail_call: BTreeSet<String>,
    tools_for: BTreeMap<String, Vec<ProbeTool>>,
    connects: Mutex<Vec<String>>,
    calls: Mutex<Vec<(String, String, JsonValue)>>,
    disconnects: Mutex<Vec<String>>,
}

impl FakeProbe {
    /// Tools for `server_id`, defaulting to one schema-bearing tool so a
    /// successful connect always has something to invoke.
    fn tools(&self, server_id: &str) -> Vec<ProbeTool> {
        self.tools_for.get(server_id).cloned().unwrap_or_else(|| {
            vec![tool(
                "ping",
                json!({ "type": "object", "properties": { "note": { "type": "string" } } }),
            )]
        })
    }

    fn connect_calls(&self) -> Vec<String> {
        self.connects.lock().expect("not poisoned").clone()
    }

    fn call_calls(&self) -> Vec<(String, String, JsonValue)> {
        self.calls.lock().expect("not poisoned").clone()
    }

    fn disconnect_calls(&self) -> Vec<String> {
        self.disconnects.lock().expect("not poisoned").clone()
    }
}

#[async_trait]
impl McpProbe for FakeProbe {
    async fn probe_connect(
        &mut self,
        server_id: &str,
        _config: ragent_config::McpServerConfig,
    ) -> Result<Vec<ProbeTool>, String> {
        self.connects
            .lock()
            .expect("not poisoned")
            .push(server_id.to_string());
        if self.fail_connect.contains(server_id) {
            return Err(format!("server '{server_id}' could not be started"));
        }
        Ok(self.tools(server_id))
    }

    async fn probe_call(
        &mut self,
        server_id: &str,
        tool: &str,
        args: JsonValue,
    ) -> Result<String, String> {
        self.calls.lock().expect("not poisoned").push((
            server_id.to_string(),
            tool.to_string(),
            args,
        ));
        if self.fail_call.contains(server_id) {
            return Err("server rejected the call: bad request".to_string());
        }
        Ok("{\"ok\":true}".to_string())
    }

    async fn probe_disconnect(&mut self, server_id: &str) -> Result<(), String> {
        self.disconnects
            .lock()
            .expect("not poisoned")
            .push(server_id.to_string());
        Ok(())
    }
}

/// The empty environment for the auth seam.
fn no_env() -> MapEnv {
    MapEnv::new()
}

/// The step with `name`, or panic.
fn step<'a>(
    report: &'a ragent_connectors::HarnessReport,
    name: &str,
) -> &'a ragent_connectors::HarnessStep {
    report
        .step(name)
        .unwrap_or_else(|| panic!("step '{name}' missing from {:?}", report.steps))
}

// -- FR-015: happy path -----------------------------------------------------

#[tokio::test]
async fn harness_connects_invokes_one_tool_and_disconnects() {
    let tree = TempTree::new("happy");
    write_connector(
        &tree.store(),
        &descriptor("echo", vec![stdio("main", "echo")]),
    );

    let credentials = InMemoryCredentialStore::new();
    let env_source = no_env();
    let mut probe = FakeProbe::default();

    let report = test_connector(
        tree.dirs(),
        &ConnectorsConfig::default(),
        &credentials,
        &env_source,
        &mut probe,
        "echo",
    )
    .await;

    assert!(report.passed(), "{:?}", report.steps);
    assert_eq!(step(&report, "discovery").outcome, StepOutcome::Pass);
    assert_eq!(step(&report, "validation").outcome, StepOutcome::Pass);
    assert_eq!(step(&report, "auth").outcome, StepOutcome::Pass);
    assert_eq!(
        step(&report, "connect echo.main").outcome,
        StepOutcome::Pass
    );
    assert_eq!(
        step(&report, "sample invocation ping on echo.main").outcome,
        StepOutcome::Pass
    );
    assert_eq!(
        step(&report, "disconnect echo.main").outcome,
        StepOutcome::Pass
    );

    // The isolated connection was connected, invoked once, and torn down.
    assert_eq!(probe.connect_calls(), vec!["echo.main".to_string()]);
    assert_eq!(probe.disconnect_calls(), vec!["echo.main".to_string()]);
    let calls = probe.call_calls();
    assert_eq!(calls.len(), 1, "{calls:?}");
    assert_eq!(calls[0].0, "echo.main");
    assert_eq!(calls[0].1, "ping");
    assert_eq!(calls[0].2, json!({ "note": "sample" }));

    let rendered = render_report(&report);
    assert!(rendered.contains("From: /connectors test echo"));
    assert!(rendered.contains("[ ok ] connect echo.main"));
    assert!(rendered.contains("[ ok ] sample invocation ping on echo.main"));
    assert!(rendered.contains("live session untouched"));
}

#[tokio::test]
async fn harness_uses_the_tool_schema_for_sample_arguments() {
    let tree = TempTree::new("schema");
    write_connector(
        &tree.store(),
        &descriptor("schema", vec![stdio("s", "echo")]),
    );

    let mut probe = FakeProbe::default();
    probe.tools_for.insert(
        "schema.s".to_string(),
        vec![tool(
            "add",
            json!({
                "type": "object",
                "properties": {
                    "fixed": { "const": "pinned" },
                    "count": { "type": "integer", "default": 7 },
                    "maybe": { "type": "string", "enum": ["alpha", "beta"] }
                }
            }),
        )],
    );

    let credentials = InMemoryCredentialStore::new();
    let env_source = no_env();
    let report = test_connector(
        tree.dirs(),
        &ConnectorsConfig::default(),
        &credentials,
        &env_source,
        &mut probe,
        "schema",
    )
    .await;

    assert!(report.passed(), "{:?}", report.steps);
    let calls = probe.call_calls();
    assert_eq!(calls.len(), 1);
    assert_eq!(
        calls[0].2,
        json!({ "fixed": "pinned", "count": 7, "maybe": "alpha" })
    );
}

#[tokio::test]
async fn harness_exercises_every_server_of_a_multi_server_connector() {
    let tree = TempTree::new("two-servers");
    write_connector(
        &tree.store(),
        &descriptor("pair", vec![stdio("a", "echo"), stdio("b", "echo")]),
    );

    let credentials = InMemoryCredentialStore::new();
    let env_source = no_env();
    let mut probe = FakeProbe::default();
    let report = test_connector(
        tree.dirs(),
        &ConnectorsConfig::default(),
        &credentials,
        &env_source,
        &mut probe,
        "pair",
    )
    .await;

    assert!(report.passed(), "{:?}", report.steps);
    assert_eq!(
        probe.connect_calls(),
        vec!["pair.a".to_string(), "pair.b".to_string()]
    );
    assert_eq!(
        probe.disconnect_calls(),
        vec!["pair.a".to_string(), "pair.b".to_string()]
    );
    assert_eq!(probe.call_calls().len(), 2);
}

// -- FR-016: failure containment --------------------------------------------

#[tokio::test]
async fn harness_reports_connect_failure_and_makes_no_change_to_the_session() {
    let tree = TempTree::new("bad-server");
    write_connector(
        &tree.store(),
        &descriptor("bad", vec![stdio("main", "definitely-not-a-real-command")]),
    );

    let credentials = InMemoryCredentialStore::new();
    let env_source = no_env();
    let mut probe = FakeProbe::default();
    probe.fail_connect.insert("bad.main".to_string());

    let report = test_connector(
        tree.dirs(),
        &ConnectorsConfig::default(),
        &credentials,
        &env_source,
        &mut probe,
        "bad",
    )
    .await;

    assert!(!report.passed());
    match &step(&report, "connect bad.main").outcome {
        StepOutcome::Fail(cause) => assert!(cause.contains("could not be started"), "{cause}"),
        other => panic!("expected connect failure, got {other:?}"),
    }
    // A failed connect is never invoked and never disconnected (nothing to
    // tear down), so the live session sees no change.
    assert!(probe.call_calls().is_empty());
    assert!(probe.disconnect_calls().is_empty());

    let rendered = render_report(&report);
    assert!(rendered.contains("[fail] connect bad.main"));
    assert!(rendered.contains("live session untouched"));
}

#[tokio::test]
async fn harness_reports_tool_failure_with_the_server_message_and_still_disconnects() {
    let tree = TempTree::new("bad-tool");
    write_connector(
        &tree.store(),
        &descriptor("chatty", vec![stdio("main", "echo")]),
    );

    let credentials = InMemoryCredentialStore::new();
    let env_source = no_env();
    let mut probe = FakeProbe::default();
    probe.fail_call.insert("chatty.main".to_string());

    let report = test_connector(
        tree.dirs(),
        &ConnectorsConfig::default(),
        &credentials,
        &env_source,
        &mut probe,
        "chatty",
    )
    .await;

    assert!(!report.passed());
    match &step(&report, "sample invocation ping on chatty.main").outcome {
        StepOutcome::Fail(cause) => {
            assert!(cause.contains("server rejected the call"), "{cause}")
        }
        other => panic!("expected tool failure, got {other:?}"),
    }
    // The connection is still torn down after a failed invocation.
    assert_eq!(probe.disconnect_calls(), vec!["chatty.main".to_string()]);
}

#[tokio::test]
async fn harness_reports_a_connector_that_advertises_no_tool_without_failing() {
    let tree = TempTree::new("no-tools");
    write_connector(
        &tree.store(),
        &descriptor("bare", vec![stdio("main", "echo")]),
    );

    let credentials = InMemoryCredentialStore::new();
    let env_source = no_env();
    let mut probe = FakeProbe::default();
    probe.tools_for.insert("bare.main".to_string(), Vec::new());

    let report = test_connector(
        tree.dirs(),
        &ConnectorsConfig::default(),
        &credentials,
        &env_source,
        &mut probe,
        "bare",
    )
    .await;

    assert!(report.passed(), "{:?}", report.steps);
    assert!(probe.call_calls().is_empty());
    let detail = step(&report, "sample invocation on bare.main")
        .detail
        .clone()
        .unwrap_or_default();
    assert!(detail.contains("advertises no tool"), "{detail}");
}

#[tokio::test]
async fn harness_reports_unknown_connector_as_discovery_failure() {
    let tree = TempTree::new("unknown");
    let credentials = InMemoryCredentialStore::new();
    let env_source = no_env();
    let mut probe = FakeProbe::default();

    let report = test_connector(
        tree.dirs(),
        &ConnectorsConfig::default(),
        &credentials,
        &env_source,
        &mut probe,
        "nope",
    )
    .await;

    assert_eq!(report.steps.len(), 1);
    match &report.steps[0].outcome {
        StepOutcome::Fail(cause) => assert!(cause.contains("unknown connector: nope"), "{cause}"),
        other => panic!("expected discovery failure, got {other:?}"),
    }
    assert!(!report.passed());
}

#[tokio::test]
async fn harness_refuses_when_the_declared_credential_is_missing() {
    let tree = TempTree::new("needs-auth");
    let mut d = descriptor("gated", vec![stdio("main", "echo")]);
    d.auth = ConnectorAuthShape::Token;
    d.credential = Some("GATED_TOKEN".to_string());
    write_connector(&tree.store(), &d);

    let credentials = InMemoryCredentialStore::new();
    let env_source = no_env();
    let mut probe = FakeProbe::default();
    let report = test_connector(
        tree.dirs(),
        &ConnectorsConfig::default(),
        &credentials,
        &env_source,
        &mut probe,
        "gated",
    )
    .await;

    assert!(!report.passed());
    match &step(&report, "auth").outcome {
        StepOutcome::Fail(cause) => assert!(cause.contains("auth failed"), "{cause}"),
        other => panic!("expected auth failure, got {other:?}"),
    }
    // An unsatisfied auth gate refuses the connect entirely (FR-022, FR-032).
    assert!(probe.connect_calls().is_empty());
}

#[tokio::test]
async fn harness_connects_a_token_connector_once_the_secret_is_stored() {
    let tree = TempTree::new("authed");
    let mut d = descriptor("gated", vec![stdio("main", "echo")]);
    d.auth = ConnectorAuthShape::Token;
    d.credential = Some("GATED_TOKEN".to_string());
    write_connector(&tree.store(), &d);

    let credentials = InMemoryCredentialStore::new();
    store_secret(&d, "s3cr3t-value", &credentials).expect("secret stored");
    let env_source = no_env();
    let mut probe = FakeProbe::default();
    let report = test_connector(
        tree.dirs(),
        &ConnectorsConfig::default(),
        &credentials,
        &env_source,
        &mut probe,
        "gated",
    )
    .await;

    assert!(report.passed(), "{:?}", report.steps);
    assert_eq!(probe.connect_calls(), vec!["gated.main".to_string()]);
}

#[tokio::test]
async fn harness_reports_unsupported_capabilities_and_keeps_running() {
    let tree = TempTree::new("unsupported");
    // One expressible server plus one with an unknown transport: the
    // unsupported label is recorded but the harness still runs the good server.
    let mut bad = stdio("legacy", "echo");
    bad.transport = "carrier-pigeon".to_string();
    write_connector(
        &tree.store(),
        &descriptor("mixed", vec![stdio("good", "echo"), bad]),
    );

    let credentials = InMemoryCredentialStore::new();
    let env_source = no_env();
    let mut probe = FakeProbe::default();
    let report = test_connector(
        tree.dirs(),
        &ConnectorsConfig::default(),
        &credentials,
        &env_source,
        &mut probe,
        "mixed",
    )
    .await;

    assert!(report.passed(), "{:?}", report.steps);
    assert_eq!(report.unsupported.len(), 1, "{:?}", report.unsupported);
    assert!(report.unsupported[0].contains("server transport"));
    let rendered = render_report(&report);
    assert!(rendered.contains("Unsupported capabilities:"));
}

// -- FR-015: the harness never touches the live session or the store --------

#[tokio::test]
async fn harness_never_writes_the_store_ledger() {
    let tree = TempTree::new("isolation");
    write_connector(
        &tree.store(),
        &descriptor("echo", vec![stdio("main", "echo")]),
    );

    let credentials = InMemoryCredentialStore::new();
    let env_source = no_env();
    let mut probe = FakeProbe::default();
    let report = test_connector(
        tree.dirs(),
        &ConnectorsConfig::default(),
        &credentials,
        &env_source,
        &mut probe,
        "echo",
    )
    .await;
    assert!(report.passed(), "{:?}", report.steps);

    let ledger = StoreLedger::load(&tree.store());
    assert!(
        ledger.state("echo").is_none(),
        "the harness must not write ledger rows"
    );
    assert!(
        !tree.store().join("_state.json").exists(),
        "the harness must not create a ledger file"
    );
}

#[tokio::test]
async fn run_harness_refuses_when_the_subsystem_is_disabled() {
    let tree = TempTree::new("disabled");
    write_connector(
        &tree.store(),
        &descriptor("echo", vec![stdio("main", "echo")]),
    );

    let config = ConnectorsConfig {
        enabled: false,
        ..ConnectorsConfig::default()
    };
    let credentials = InMemoryCredentialStore::new();
    let env_source = no_env();
    let mut probe = FakeProbe::default();

    let outcome = run_harness(
        &tree.0,
        &config,
        &credentials,
        &env_source,
        &mut probe,
        "echo",
    )
    .await;

    assert_eq!(outcome, Err(HarnessError::Disabled));
    assert!(probe.connect_calls().is_empty());
    let rendered = HarnessError::Disabled.report("echo");
    assert!(rendered.contains("[err]"), "{rendered}");
    assert!(rendered.contains("disabled"), "{rendered}");
}

// -- FR-015: the shared async dispatcher ------------------------------------

#[tokio::test]
async fn async_dispatcher_runs_test_and_ignores_other_subcommands() {
    let tree = TempTree::new("dispatch");
    write_connector(
        &tree.store(),
        &descriptor("echo", vec![stdio("main", "echo")]),
    );

    // A non-`test` subcommand is not this dispatcher's business.
    let credentials = InMemoryCredentialStore::new();
    let env_source = no_env();
    let mut probe = FakeProbe::default();
    assert!(
        run_connector_subcommand_async(
            &tree.0,
            &ConnectorsConfig::default(),
            &credentials,
            &env_source,
            &mut probe,
            "list",
            "",
        )
        .await
        .is_none()
    );

    let report = run_connector_subcommand_async(
        &tree.0,
        &ConnectorsConfig::default(),
        &credentials,
        &env_source,
        &mut probe,
        "test",
        "echo",
    )
    .await
    .expect("test subcommand");
    assert!(report.contains("[ ok ] disconnect echo.main"), "{report}");
    assert!(report.contains("harness complete"), "{report}");
}

#[tokio::test]
async fn async_dispatcher_renders_the_usage_error_for_a_missing_id() {
    let tree = TempTree::new("missing-id");
    let credentials = InMemoryCredentialStore::new();
    let env_source = no_env();
    let mut probe = FakeProbe::default();
    let report = run_connector_subcommand_async(
        &tree.0,
        &ConnectorsConfig::default(),
        &credentials,
        &env_source,
        &mut probe,
        "test",
        "",
    )
    .await
    .expect("test subcommand");
    assert!(report.contains("[err] Missing argument."), "{report}");
}

// -- sample argument generation ---------------------------------------------

#[test]
fn sample_for_schema_generates_typed_values() {
    assert_eq!(
        sample_for_schema(&json!({ "type": "string" })),
        json!("sample")
    );
    assert_eq!(sample_for_schema(&json!({ "type": "integer" })), json!(0));
    assert_eq!(sample_for_schema(&json!({ "type": "number" })), json!(0.0));
    assert_eq!(
        sample_for_schema(&json!({ "type": "boolean" })),
        json!(false)
    );
    assert_eq!(
        sample_for_schema(&json!({ "const": "fixed" })),
        json!("fixed")
    );
    assert_eq!(
        sample_for_schema(&json!({ "type": "string", "default": "d" })),
        json!("d")
    );
    assert_eq!(
        sample_for_schema(&json!({ "type": "string", "enum": ["a", "b"] })),
        json!("a")
    );
    assert_eq!(
        sample_for_schema(&json!({ "type": ["integer", "null"] })),
        json!(0)
    );
}

#[test]
fn sample_for_schema_recurses_into_objects_and_arrays() {
    let schema = json!({
        "type": "object",
        "properties": {
            "items": { "type": "array", "items": { "type": "string" } },
            "nested": { "type": "object", "properties": { "n": { "type": "integer" } } }
        }
    });
    assert_eq!(
        sample_for_schema(&schema),
        json!({ "items": ["sample"], "nested": { "n": 0 } })
    );
}

#[test]
fn harness_report_passed_and_step_helpers_are_consistent() {
    let report = ragent_connectors::HarnessReport {
        connector_id: "x".to_string(),
        steps: vec![ragent_connectors::HarnessStep {
            name: "discovery".to_string(),
            outcome: StepOutcome::Pass,
            elapsed: std::time::Duration::from_millis(1),
            detail: None,
        }],
        unsupported: Vec::new(),
    };
    assert!(report.passed());
    assert!(report.step("discovery").is_some());
    assert!(report.step("missing").is_none());
}

/// A `BTreeMap`-keyed tools map for the fake probe stays `Send`: assert the
/// probe trait object is usable from the dispatcher signature.
#[test]
fn probe_trait_is_object_safe_and_send() {
    fn assert_send<T: Send>() {}
    assert_send::<FakeProbe>();
    let _: &dyn McpProbe = &FakeProbe::default();
}

#[tokio::test]
async fn async_dispatcher_resolves_a_display_name_reference_to_the_installed_id() {
    let tree = TempTree::new("dispatch-name");
    // The connector id is `echo` but its display name is `Echo Probe`, so the
    // reference `"Echo Probe"` must resolve to the installed id `echo`.
    let mut d = descriptor("echo", vec![stdio("main", "echo")]);
    d.name = "Echo Probe".to_string();
    write_connector(&tree.store(), &d);

    let credentials = InMemoryCredentialStore::new();
    let env_source = no_env();
    let mut probe = FakeProbe::default();
    let report = run_connector_subcommand_async(
        &tree.0,
        &ConnectorsConfig::default(),
        &credentials,
        &env_source,
        &mut probe,
        "test",
        "Echo Probe",
    )
    .await
    .expect("test subcommand");
    // Resolution reached the harness (which then runs the `echo` connector),
    // rather than reporting an unknown connector under the display name.
    assert!(report.contains("harness complete"), "{report}");
    assert!(!report.contains("unknown connector"), "{report}");
}
