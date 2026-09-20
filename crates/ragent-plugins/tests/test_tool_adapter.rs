//! Tests for `PluginToolAdapter` and `PluginManager::execute_tool`
//! (spec `plugins` T-010; FR-005, FR-015, FR-024, FR-026).

use std::collections::BTreeSet;
use std::path::PathBuf;

use ragent_plugins::{
    LifecycleState, PluginError, PluginManager, PluginToolAdapter, PluginToolDecl, StoreLedger,
    dispatch_sandbox, plugin_tool_name, store_dirs_at,
};
use ragent_tools_core::Tool;
use serde_json::{Value as JsonValue, json};

static SEQ: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);

/// RAII sandboxed temp tree rooted at `target/temp/` (AGENTS.md: no `/tmp`).
struct TempTree(PathBuf);

impl TempTree {
    fn new(name: &str) -> Self {
        let unique = SEQ.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(format!(
            "../../target/temp/plugins-test/tool-adapter-{name}-{}-{unique}",
            std::process::id()
        ));
        std::fs::create_dir_all(&path).expect("temp tree creatable");
        Self(path)
    }

    /// The project store (`<tree>/.ragent/plugins/`).
    fn store(&self) -> PathBuf {
        self.0.join(".ragent/plugins")
    }

    /// A fresh manager bound to this tree's project store (no global leg).
    fn manager(&self) -> PluginManager {
        PluginManager::new(
            store_dirs_at(&self.0, Some(&self.store()), None),
            ragent_config::PluginsConfig::default(),
        )
    }
}

impl Drop for TempTree {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// Write a Codex-dialect plugin into the tree's project store with explicit
/// id so `manager.enable(id)` matches (derived ids come from `name`).
fn write_plugin(tree: &TempTree, id: &str, entry: &str) {
    let plugin_dir = tree.store().join(id);
    std::fs::create_dir_all(&plugin_dir).expect("plugin dir creatable");
    std::fs::write(
        plugin_dir.join("codex-plugin.json"),
        format!(r#"{{ "id": "{id}", "name": "{id}", "version": "1.0.0", "entry": "index.js" }}"#),
    )
    .expect("manifest writable");
    std::fs::write(plugin_dir.join("index.js"), entry).expect("entry writable");
}

/// Enable and load `id`, then register its tools into the provided sink via
/// the collision-checked registration path.
fn load_and_register(
    manager: &PluginManager,
    id: &str,
    existing: &BTreeSet<String>,
) -> (Vec<String>, Vec<std::sync::Arc<PluginToolAdapter>>) {
    let mut registered = Vec::new();
    let names = manager
        .register_plugin_tools(id, existing, |adapter| registered.push(adapter))
        .expect("registration succeeds");
    (names, registered)
}

// ── FR-005: adapter metadata surface ───────────────────────────────────────

#[test]
fn adapter_surfaces_declared_name_description_and_schema() {
    let adapter = PluginToolAdapter::new(
        "codex-weather",
        PluginToolDecl {
            name: "get_weather".to_string(),
            description: "Get weather by city".to_string(),
            parameters: json!({
                "type": "object",
                "properties": { "city": { "type": "string" } },
                "required": ["city"]
            }),
        },
    );

    assert_eq!(adapter.name(), "plugin_codex-weather_get_weather");
    assert_eq!(adapter.description(), "Get weather by city");
    let schema = adapter.parameters_schema();
    assert_eq!(
        schema
            .pointer("/properties/city/type")
            .and_then(JsonValue::as_str),
        Some("string")
    );
    // Permission domain: plugin:<id> (A5).
    assert_eq!(adapter.permission_category(), "plugin:codex-weather");
    assert_eq!(adapter.plugin_id(), "codex-weather");
    assert_eq!(adapter.tool_name(), "get_weather");
}

#[test]
fn adapter_without_declared_schema_falls_back_to_object_schema() {
    let adapter = PluginToolAdapter::new(
        "p",
        PluginToolDecl {
            name: "bare".to_string(),
            description: String::new(),
            parameters: JsonValue::Null,
        },
    );
    assert_eq!(adapter.parameters_schema(), json!({ "type": "object" }));
}

#[test]
fn plugin_tool_name_formats_registry_names() {
    assert_eq!(
        plugin_tool_name("claude-todo", "add"),
        "plugin_claude-todo_add"
    );
}

// ── FR-024: collision rejection ────────────────────────────────────────────

#[test]
fn registration_rejects_collision_with_builtin() {
    let tree = TempTree::new("collision-builtin");
    write_plugin(
        &tree,
        "codex-bash",
        r#"ragent.register_tool({ name: "execute", description: "x", parameters: {}, handler: function(a){ return "y"; } });"#,
    );
    let mut manager = tree.manager();
    assert_eq!(
        manager.enable("codex-bash").expect("enable").state,
        LifecycleState::Loaded
    );

    let builtins: BTreeSet<String> = ["read".to_string(), "bash".to_string()]
        .into_iter()
        .collect();

    // Direct: name colliding with the builtin set is rejected.
    let result = manager.register_plugin_tools(
        "codex-bash",
        &builtins
            .iter()
            .cloned()
            .chain(["plugin_codex-bash_execute".to_string()])
            .collect(),
        |_| panic!("nothing must be registered on collision"),
    );
    assert!(
        matches!(&result, Err(PluginError::NameCollision(name)) if name == "plugin_codex-bash_execute"),
        "collision reports the clashing name; got {result:?}"
    );
}

#[test]
fn registration_accepts_distinct_names_and_registers_all() {
    let tree = TempTree::new("register-clean");
    write_plugin(
        &tree,
        "multi",
        r#"
        ragent.register_tool({ name: "one", description: "d", parameters: {}, handler: function(a){ return 1; } });
        ragent.register_tool({ name: "two", description: "d", parameters: {}, handler: function(a){ return 2; } });
        "#,
    );
    let mut manager = tree.manager();
    assert_eq!(
        manager.enable("multi").expect("enable").state,
        LifecycleState::Loaded
    );
    let builtins: BTreeSet<String> = ["read".to_string(), "bash".to_string()]
        .into_iter()
        .collect();

    let (names, adapters) = load_and_register(&manager, "multi", &builtins);
    assert_eq!(
        names,
        vec![
            "plugin_multi_one".to_string(),
            "plugin_multi_two".to_string()
        ]
    );
    assert_eq!(adapters.len(), 2);
    // Registry list reflects the in-session plugin tool names now.
    let all = manager.registered_plugin_tool_names();
    assert!(all.contains("plugin_multi_one"));
    assert!(all.contains("plugin_multi_two"));
}

#[test]
fn registration_rejects_collision_with_other_plugin_same_name() {
    let tree = TempTree::new("collision-plugin");
    // Deliberately colliding full names: same tool name under the same
    // registry prefix cannot happen across *different* plugin ids, so the
    // cross-plugin case is exercised by pre-seeding a name as if another
    // plugin already registered it.
    write_plugin(
        &tree,
        "late",
        r#"ragent.register_tool({ name: "ping", description: "b", parameters: {}, handler: function(a){ return "b"; } });"#,
    );
    let mut manager = tree.manager();
    manager.enable("late").expect("loads");

    let mut existing: BTreeSet<String> = manager.registered_plugin_tool_names();
    // Another plugin already holds `plugin_late_ping`'s slot (FR-024).
    existing.insert("plugin_late_ping".to_string());

    let mut registered = 0usize;
    let result = manager.register_plugin_tools("late", &existing, |_| registered += 1);
    assert!(
        matches!(&result, Err(PluginError::NameCollision(name)) if name == "plugin_late_ping"),
        "cross-plugin collision rejected; got {result:?}"
    );
    assert_eq!(registered, 0, "all-or-nothing: nothing registered");
}

#[test]
fn duplicate_declaration_within_one_plugin_is_rejected() {
    let tree = TempTree::new("collision-self");
    write_plugin(
        &tree,
        "dupe",
        r#"
        ragent.register_tool({ name: "twice", description: "first", parameters: {}, handler: function(a){ return 1; } });
        ragent.register_tool({ name: "twice", description: "second", parameters: {}, handler: function(a){ return 2; } });
        "#,
    );
    let mut manager = tree.manager();
    manager.enable("dupe").expect("loads with two declarations");

    let mut counted = 0usize;
    let result = manager.register_plugin_tools("dupe", &BTreeSet::new(), |_| counted += 1);
    assert!(
        matches!(&result, Err(PluginError::NameCollision(name)) if name == "plugin_dupe_twice"),
        "duplicate within a plugin is a collision; got {result:?}"
    );
    assert_eq!(counted, 0, "all-or-nothing: nothing registered");
}

#[test]
fn registering_unloaded_or_errored_plugin_fails_without_panicking() {
    let tree = TempTree::new("register-errored");
    write_plugin(&tree, "fails", r#"throw new Error("entry blew up");"#);
    let mut manager = tree.manager();
    let report = manager.enable("fails").expect("enable returns a report");
    assert_eq!(report.state, LifecycleState::Errored);

    let result = manager.register_plugin_tools("fails", &BTreeSet::new(), |_| {
        panic!("errored plugins register nothing")
    });
    assert!(
        matches!(
            &result,
            Err(PluginError::Script { .. }) | Err(PluginError::UnknownPlugin(_))
        ),
        "errored plugin registration is refused; got {result:?}"
    );

    let unknown = manager.register_plugin_tools("ghost", &BTreeSet::new(), |_| {});
    assert!(matches!(unknown, Err(PluginError::UnknownPlugin(_))));
}

// ── FR-005/FR-015/FR-026: execution and marshalling ───────────────────────

#[test]
fn handler_receives_json_args_and_bare_string_passes_through() {
    let tree = TempTree::new("exec-string");
    write_plugin(
        &tree,
        "echo",
        r#"ragent.register_tool({ name: "say", description: "echo", parameters: {}, handler: function(args){ return "said:" + args.word; } });"#,
    );
    let mut manager = tree.manager();
    assert_eq!(
        manager.enable("echo").expect("enable").state,
        LifecycleState::Loaded
    );

    let (result, unloaded) = manager.execute_tool("plugin_echo_say", json!({ "word": "hello" }));
    assert!(!unloaded);
    let output = result.expect("execution succeeds");
    assert_eq!(output.content, "said:hello");
    assert_eq!(output.metadata, None);

    // Telemetry recorded one successful invocation.
    let ledger = StoreLedger::load(&tree.store());
    let counters = &ledger.state("echo").expect("row").counters;
    assert_eq!(counters.tool_invocations, 1);
    assert_eq!(counters.tool_failures, 0);
}

#[test]
fn object_return_follows_content_contract() {
    let tree = TempTree::new("exec-contract");
    write_plugin(
        &tree,
        "svc",
        r#"
        ragent.register_tool({ name: "ok", description: "d", parameters: {}, handler: function(a){ return { content: "done", isError: false }; } });
        ragent.register_tool({ name: "fail", description: "d", parameters: {}, handler: function(a){ return { content: "it broke", isError: true }; } });
        "#,
    );
    let mut manager = tree.manager();
    manager.enable("svc").expect("enable");

    let (ok_result, _) = manager.execute_tool("plugin_svc_ok", json!({}));
    let ok_out = ok_result.expect("content contract success");
    assert_eq!(ok_out.content, "done");
    assert_eq!(
        ok_out.metadata,
        Some(json!({ "content": "done", "isError": false }))
    );

    let (fail_result, _) = manager.execute_tool("plugin_svc_fail", json!({}));
    match fail_result {
        Err(PluginError::Script { plugin, detail }) => {
            assert_eq!(plugin, "svc");
            assert_eq!(detail, "it broke");
        }
        other => panic!("isError: true fails the tool call; got {other:?}"),
    }
}

#[test]
fn arbitrary_json_return_becomes_content_and_metadata() {
    let tree = TempTree::new("exec-json");
    write_plugin(
        &tree,
        "data",
        r#"ragent.register_tool({ name: "fetch", description: "d", parameters: {}, handler: function(a){ return { city: a.city, temp: 21 }; } });"#,
    );
    let mut manager = tree.manager();
    manager.enable("data").expect("enable");

    let (result, _) = manager.execute_tool("plugin_data_fetch", json!({ "city": "Berlin" }));
    let output = result.expect("execution succeeds");
    let parsed: JsonValue =
        serde_json::from_str(&output.content).expect("content carries the JSON document");
    assert_eq!(parsed["city"], "Berlin");
    assert_eq!(parsed["temp"], 21);
    assert_eq!(
        output.metadata,
        Some(json!({ "city": "Berlin", "temp": 21 }))
    );
}

#[test]
fn undefined_and_null_returns_yield_empty_content() {
    let tree = TempTree::new("exec-empty");
    write_plugin(
        &tree,
        "void",
        r#"
        ragent.register_tool({ name: "nothing", description: "d", parameters: {}, handler: function(a){ } });
        ragent.register_tool({ name: "null", description: "d", parameters: {}, handler: function(a){ return null; } });
        "#,
    );
    let mut manager = tree.manager();
    manager.enable("void").expect("enable");

    for name in ["plugin_void_nothing", "plugin_void_null"] {
        let (result, _) = manager.execute_tool(name, json!({}));
        let output = result.expect("void returns succeed");
        assert_eq!(output.content, "");
        assert_eq!(output.metadata, None);
    }
}

#[test]
fn thrown_handler_exception_is_contained_as_tool_failure() {
    let tree = TempTree::new("exec-throw");
    write_plugin(
        &tree,
        "boom",
        r#"ragent.register_tool({ name: "go", description: "d", parameters: {}, handler: function(a){ throw new Error("kaboom"); } });"#,
    );
    let mut manager = tree.manager();
    manager.enable("boom").expect("enable");

    let (result, _) = manager.execute_tool("plugin_boom_go", json!({}));
    match result {
        Err(PluginError::Script { plugin, detail }) => {
            assert_eq!(plugin, "boom");
            assert!(
                detail.contains("kaboom"),
                "exception message carried; got {detail}"
            );
        }
        other => panic!("uncaught handler exception fails the tool call; got {other:?}"),
    }
}

#[test]
fn unserialisable_return_fails_the_call_not_the_process() {
    let tree = TempTree::new("exec-cyclic");
    write_plugin(
        &tree,
        "cyc",
        r#"ragent.register_tool({ name: "loop", description: "d", parameters: {}, handler: function(a){ const o = {}; o.self = o; return o; } });"#,
    );
    let mut manager = tree.manager();
    manager.enable("cyc").expect("enable");

    let (result, _) = manager.execute_tool("plugin_cyc_loop", json!({}));
    match result {
        Err(PluginError::Script { detail, .. }) => {
            assert!(
                detail.contains("not JSON-serialisable"),
                "serialisation error reported; got {detail}"
            );
        }
        other => panic!("JSON.stringify throw is contained; got {other:?}"),
    }
}

#[test]
fn infinite_loop_handler_times_out_without_hanging() {
    let tree = TempTree::new("exec-timeout");
    write_plugin(
        &tree,
        "slow",
        r#"ragent.register_tool({ name: "spin", description: "d", parameters: {}, handler: function(a){ while(true){} } });"#,
    );
    let mut manager = tree.manager();
    manager.enable("slow").expect("enable");
    // The dispatch wrapper rearms the interrupt per invocation, so an
    // infinite handler trips within this call's own budget window (default
    // `max_execution_ms` in `PluginsConfig`) — the call must return rather
    // than hang (FR-026). The wall-clock bound is generous for slow CI; the
    // contract is "finite".
    let started = std::time::Instant::now();
    let (result, _unloaded) = manager.execute_tool("plugin_slow_spin", json!({}));
    assert!(started.elapsed() < std::time::Duration::from_secs(30));
    assert!(
        matches!(result, Err(PluginError::Timeout)),
        "got {result:?}"
    );
    // One timeout failure counts toward the auto-unload threshold (default
    // 3); the plugin is still loaded — only the third consecutive failure
    // unloads. Assert the failure was recorded against telemetry instead.
    let tracked = manager.get("slow").expect("tracked");
    let ledger = StoreLedger::load(&tracked.store);
    let counters = &ledger.state("slow").expect("row").counters;
    assert_eq!(counters.tool_failures, 1);
    assert_eq!(counters.consecutive_failures, 1);
}

#[test]
fn missing_handler_fails_with_clear_error() {
    let tree = TempTree::new("exec-nohandler");
    write_plugin(
        &tree,
        "decl",
        r#"ragent.register_tool({ name: "registered", description: "d", parameters: {} });"#,
    );
    let mut manager = tree.manager();
    manager.enable("decl").expect("enable");

    let (result, _) = manager.execute_tool("plugin_decl_registered", json!({}));
    match result {
        Err(PluginError::Script { detail, .. }) => {
            assert!(
                detail.contains("no handler registered for tool"),
                "missing handler named; got {detail}"
            );
        }
        other => panic!("declaration without handler fails cleanly; got {other:?}"),
    }
}

#[test]
fn unknown_registry_name_is_unknown_plugin_error() {
    let tree = TempTree::new("exec-unknown");
    write_plugin(
        &tree,
        "here",
        r#"ragent.register_tool({ name: "x", description: "d", parameters: {}, handler: function(a){ return 1; } });"#,
    );
    let mut manager = tree.manager();
    manager.enable("here").expect("enable");

    let (result, unloaded) = manager.execute_tool("plugin_elsewhere_x", json!({}));
    assert!(!unloaded, "unknown names never touch telemetry");
    assert!(matches!(result, Err(PluginError::UnknownPlugin(_))));
}

#[test]
fn consecutive_handler_failures_trip_auto_unload() {
    let tree = TempTree::new("exec-autounload");
    write_plugin(
        &tree,
        "flaky",
        r#"ragent.register_tool({ name: "go", description: "d", parameters: {}, handler: function(a){ throw new Error("nope"); } });"#,
    );
    let mut manager = tree.manager().with_auto_unload_threshold(3);
    manager.enable("flaky").expect("enable");

    for attempt in 1..=2 {
        let (result, unloaded) = manager.execute_tool("plugin_flaky_go", json!({}));
        assert!(result.is_err());
        assert!(!unloaded, "attempt {attempt} must not trip the threshold");
    }
    let (result, unloaded) = manager.execute_tool("plugin_flaky_go", json!({}));
    assert!(result.is_err());
    assert!(unloaded, "third consecutive failure trips auto-unload");

    // The plugin is now errored and its sandbox dropped; further dispatches
    // fail cleanly.
    let tracked = manager.get("flaky").expect("still tracked");
    assert_eq!(tracked.state, LifecycleState::Errored);
    assert!(tracked.context().is_none());
    let (stale, _) = manager.execute_tool("plugin_flaky_go", json!({}));
    match stale {
        Err(PluginError::Script { .. }) | Err(PluginError::UnknownPlugin(_)) => {}
        other => panic!("stale dispatch after auto-unload is contained; got {other:?}"),
    }
}

#[test]
fn reset_on_success_keeps_consecutive_counter_down() {
    let tree = TempTree::new("exec-reset");
    write_plugin(
        &tree,
        "mixed",
        r#"
        let calls = 0;
        ragent.register_tool({ name: "flap", description: "d", parameters: {}, handler: function(a){ calls += 1; if (calls === 2) { throw new Error("one bad"); } return "ok" + calls; } });
        "#,
    );
    let mut manager = tree.manager().with_auto_unload_threshold(2);
    manager.enable("mixed").expect("enable");

    let (r1, _) = manager.execute_tool("plugin_mixed_flap", json!({}));
    let (r2, _) = manager.execute_tool("plugin_mixed_flap", json!({}));
    let (r3, unloaded) = manager.execute_tool("plugin_mixed_flap", json!({}));
    assert!(r1.is_ok() && r2.is_err() && r3.is_ok());
    assert!(!unloaded, "success reset keeps the threshold away");
}

// ── direct sandbox dispatch (used by the T-015 harness too) ───────────────

#[test]
fn dispatch_sandbox_invokes_stashed_handlers_directly() {
    let pool = ragent_plugins::RuntimePool::new();
    let context = pool
        .checkout(ragent_plugins::SandboxBudget::default())
        .expect("sandbox allocatable");
    let install = ragent_plugins::HostApiInstall::new("direct", PathBuf::new());
    context
        .install_host_api(&install)
        .expect("host api installs");
    context
        .eval(
            r#"ragent.register_tool({ name: "double", description: "d", parameters: {},
               handler: function(args){ return args.n * 2; } });"#,
        )
        .expect("registration script runs");

    let output = dispatch_sandbox(&context, "direct", "double", &json!({ "n": 21 }))
        .expect("dispatch works on a bare sandbox");
    let parsed: JsonValue = serde_json::from_str(&output.content).expect("numeric return JSON");
    assert_eq!(parsed, 42);
}
