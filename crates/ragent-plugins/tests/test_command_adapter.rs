//! Tests for `PluginCommandAdapter` and `PluginManager::execute_command`
//! (spec `plugins` T-012; FR-004, FR-024, FR-026).

use std::collections::BTreeSet;
use std::path::PathBuf;

use ragent_plugins::{
    LifecycleState, PluginCommandAdapter, PluginCommandDecl, PluginCommandDef, PluginError,
    PluginManager, StoreLedger, dispatch_command_sandbox, store_dirs_at,
};

static SEQ: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);

/// RAII sandboxed temp tree rooted at `target/temp/` (AGENTS.md: no `/tmp`).
struct TempTree(PathBuf);

impl TempTree {
    fn new(name: &str) -> Self {
        let unique = SEQ.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(format!(
            "../../target/temp/plugins-test/command-adapter-{name}-{}-{unique}",
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

/// Enable `id` and register its commands into the provided sink via the
/// collision-checked registration path.
fn load_and_register(
    manager: &PluginManager,
    id: &str,
    existing: &BTreeSet<String>,
) -> (Vec<String>, Vec<std::sync::Arc<PluginCommandAdapter>>) {
    let mut registered = Vec::new();
    let names = manager
        .register_plugin_commands(id, existing, |adapter| registered.push(adapter))
        .expect("registration succeeds");
    (names, registered)
}

// ── FR-004: adapter metadata surface ───────────────────────────────────────

#[test]
fn adapter_surfaces_declared_name_description_and_usage() {
    let adapter = PluginCommandAdapter::new(
        "codex-weather",
        PluginCommandDef::inline(PluginCommandDecl {
            name: "weather".to_string(),
            description: "Show weather for a city".to_string(),
            usage: Some("/weather <city>".to_string()),
        }),
    );

    assert_eq!(adapter.name(), "weather");
    assert_eq!(adapter.plugin_id(), "codex-weather");
    assert_eq!(adapter.description(), "Show weather for a city");
    assert_eq!(adapter.usage(), Some("/weather <city>"));
}

#[test]
fn adapter_without_usage_reports_none() {
    let adapter = PluginCommandAdapter::new(
        "bare",
        PluginCommandDef::inline(PluginCommandDecl {
            name: "bare".to_string(),
            description: "d".to_string(),
            usage: None,
        }),
    );
    assert_eq!(adapter.usage(), None);
}

#[test]
fn manifest_declared_commands_are_contributed_without_register_calls() {
    let tree = TempTree::new("manifest-cmds");
    let plugin_dir = tree.store().join("weather");
    std::fs::create_dir_all(&plugin_dir).expect("plugin dir creatable");
    std::fs::write(
        plugin_dir.join("codex-plugin.json"),
        r#"{ "id": "weather", "name": "weather", "version": "1.0.0", "entry": "index.js",
             "commands": [ { "name": "forecast", "description": "3-day outlook", "usage": "/forecast <city>" } ] }"#,
    )
    .expect("manifest writable");
    std::fs::write(
        plugin_dir.join("index.js"),
        r#"globalThis.__ragent_commands = globalThis.__ragent_commands || {};
           __ragent_commands.forecast = function(args){ return "sunny " + args; };"#,
    )
    .expect("entry writable");
    let mut manager = tree.manager();
    assert_eq!(
        manager.enable("weather").expect("enable").state,
        LifecycleState::Loaded
    );

    let (names, adapters) = load_and_register(&manager, "weather", &BTreeSet::new());
    assert_eq!(names, vec!["forecast".to_string()]);
    assert_eq!(adapters[0].usage(), Some("/forecast <city>"));

    let (result, _) = manager.execute_command("forecast", "Berlin");
    assert_eq!(result.expect("manifest command invocable"), "sunny Berlin");
}

#[test]
fn register_command_stashes_handler_and_declaration_together() {
    let tree = TempTree::new("register-stash");
    write_plugin(
        &tree,
        "todo",
        r#"ragent.register_command({ name: "todo-add", description: "Add a todo", usage: "/todo-add <text>",
             handler: function(text){ return "added: " + text; } });"#,
    );
    let mut manager = tree.manager();
    manager.enable("todo").expect("enable");

    let (names, _) = load_and_register(&manager, "todo", &BTreeSet::new());
    assert_eq!(names, vec!["todo-add".to_string()]);

    let (result, _) = manager.execute_command("todo-add", "buy milk");
    assert_eq!(result.expect("handler dispatched"), "added: buy milk");
}

// ── FR-024: collision rejection ────────────────────────────────────────────

#[test]
fn registration_rejects_collision_with_builtin_slash_command() {
    let tree = TempTree::new("cmd-collision-builtin");
    write_plugin(
        &tree,
        "specs",
        r#"ragent.register_command({ name: "spec", description: "plugin spec command", handler: function(a){ return "x"; } });"#,
    );
    let mut manager = tree.manager();
    assert_eq!(
        manager.enable("specs").expect("enable").state,
        LifecycleState::Loaded
    );

    // Built-in `SLASH_COMMANDS` list includes `spec` (FR-024 collision
    // surface: built-ins plus other plugins' commands).
    let builtins: BTreeSet<String> = ["plugins".to_string(), "spec".to_string()]
        .into_iter()
        .collect();

    let result = manager.register_plugin_commands("specs", &builtins, |_| {
        panic!("nothing must be registered on collision")
    });
    assert!(
        matches!(&result, Err(PluginError::NameCollision(name)) if name == "spec"),
        "collision reports the clashing trigger; got {result:?}"
    );
}

#[test]
fn registration_accepts_distinct_names_and_registers_all() {
    let tree = TempTree::new("cmd-register-clean");
    write_plugin(
        &tree,
        "multi",
        r#"
        ragent.register_command({ name: "alpha", description: "d", handler: function(a){ return "a"; } });
        ragent.register_command({ name: "beta", description: "d", handler: function(a){ return "b"; } });
        "#,
    );
    let mut manager = tree.manager();
    manager.enable("multi").expect("enable");

    let builtins: BTreeSet<String> = std::iter::once("spec".to_string()).collect();
    let (names, adapters) = load_and_register(&manager, "multi", &builtins);
    assert_eq!(names, vec!["alpha".to_string(), "beta".to_string()]);
    assert_eq!(adapters.len(), 2);

    let all = manager.registered_plugin_command_names();
    assert!(all.contains("alpha"));
    assert!(all.contains("beta"));
}

#[test]
fn registration_rejects_collision_with_other_plugin_same_name() {
    let tree = TempTree::new("cmd-collision-plugin");
    write_plugin(
        &tree,
        "late",
        r#"ragent.register_command({ name: "ping", description: "plugin ping", handler: function(a){ return "pong"; } });"#,
    );
    let mut manager = tree.manager();
    manager.enable("late").expect("loads");

    // Another plugin already holds the `ping` trigger (FR-024).
    let existing: BTreeSet<String> = std::iter::once("ping".to_string()).collect();
    let mut registered = 0usize;
    let result = manager.register_plugin_commands("late", &existing, |_| registered += 1);
    assert!(
        matches!(&result, Err(PluginError::NameCollision(name)) if name == "ping"),
        "cross-plugin collision rejected; got {result:?}"
    );
    assert_eq!(registered, 0, "all-or-nothing: nothing registered");
}

#[test]
fn duplicate_declaration_within_one_plugin_is_rejected() {
    let tree = TempTree::new("cmd-collision-self");
    write_plugin(
        &tree,
        "dupe",
        r#"
        ragent.register_command({ name: "twice", description: "first", handler: function(a){ return 1; } });
        ragent.register_command({ name: "twice", description: "second", handler: function(a){ return 2; } });
        "#,
    );
    let mut manager = tree.manager();
    manager.enable("dupe").expect("loads with two declarations");

    let mut counted = 0usize;
    let result = manager.register_plugin_commands("dupe", &BTreeSet::new(), |_| counted += 1);
    assert!(
        matches!(&result, Err(PluginError::NameCollision(name)) if name == "twice"),
        "duplicate within a plugin is a collision; got {result:?}"
    );
    assert_eq!(counted, 0, "all-or-nothing: nothing registered");
}

#[test]
fn registering_unloaded_or_errored_plugin_fails_without_panicking() {
    let tree = TempTree::new("cmd-register-errored");
    write_plugin(&tree, "fails", r#"throw new Error("entry blew up");"#);
    let mut manager = tree.manager();
    let report = manager.enable("fails").expect("enable returns a report");
    assert_eq!(report.state, LifecycleState::Errored);

    let result = manager.register_plugin_commands("fails", &BTreeSet::new(), |_| {
        panic!("errored plugins register nothing")
    });
    assert!(
        matches!(
            &result,
            Err(PluginError::Script { .. }) | Err(PluginError::UnknownPlugin(_))
        ),
        "errored plugin registration is refused; got {result:?}"
    );

    let unknown = manager.register_plugin_commands("ghost", &BTreeSet::new(), |_| {});
    assert!(matches!(unknown, Err(PluginError::UnknownPlugin(_))));
}

// ── FR-004/FR-026: invocation, marshalling, containment ───────────────────

#[test]
fn argument_string_reaches_the_handler_verbatim() {
    let tree = TempTree::new("cmd-args");
    write_plugin(
        &tree,
        "echo",
        r#"ragent.register_command({ name: "echo", description: "d", handler: function(args){ return "got:[" + args + "]"; } });"#,
    );
    let mut manager = tree.manager();
    manager.enable("echo").expect("enable");

    let (result, unloaded) = manager.execute_command("echo", "  spaced   text  ");
    assert!(!unloaded);
    assert_eq!(
        result.expect("execution succeeds"),
        "got:[  spaced   text  ]",
        "the argument string is not trimmed or re-parsed"
    );
}

#[test]
fn returned_text_is_the_message_window_payload() {
    let tree = TempTree::new("cmd-text");
    write_plugin(
        &tree,
        "say",
        r#"ragent.register_command({ name: "say", description: "d", handler: function(a){ return "hello world"; } });"#,
    );
    let mut manager = tree.manager();
    manager.enable("say").expect("enable");

    let (result, _) = manager.execute_command("say", "");
    assert_eq!(result.expect("text return"), "hello world");
}

#[test]
fn undefined_and_object_returns_become_empty_or_json_text() {
    let tree = TempTree::new("cmd-marshal");
    write_plugin(
        &tree,
        "forms",
        r#"
        ragent.register_command({ name: "silent", description: "d", handler: function(a){ } });
        ragent.register_command({ name: "obj", description: "d", handler: function(a){ return { n: 42 }; } });
        "#,
    );
    let mut manager = tree.manager();
    manager.enable("forms").expect("enable");

    let (silent, _) = manager.execute_command("silent", "");
    assert_eq!(silent.expect("undefined return yields empty text"), "");

    let (obj, _) = manager.execute_command("obj", "");
    let text = obj.expect("object return serialises");
    let parsed: serde_json::Value = serde_json::from_str(&text).expect("JSON text");
    assert_eq!(parsed["n"], 42);
}

#[test]
fn invocation_failure_is_recorded_in_telemetry() {
    let tree = TempTree::new("cmd-telemetry");
    write_plugin(
        &tree,
        "flaky",
        r#"ragent.register_command({ name: "go", description: "d", handler: function(a){ throw new Error("nope"); } });"#,
    );
    let mut manager = tree.manager();
    manager.enable("flaky").expect("enable");

    let (result, _) = manager.execute_command("go", "");
    assert!(result.is_err());

    let ledger = StoreLedger::load(&tree.store());
    let counters = &ledger.state("flaky").expect("row").counters;
    assert_eq!(counters.tool_invocations, 1);
    assert_eq!(counters.tool_failures, 1);
}

#[test]
fn thrown_command_exception_is_contained() {
    let tree = TempTree::new("cmd-throw");
    write_plugin(
        &tree,
        "boom",
        r#"ragent.register_command({ name: "go", description: "d", handler: function(a){ throw new Error("kaboom"); } });"#,
    );
    let mut manager = tree.manager();
    manager.enable("boom").expect("enable");

    let (result, _) = manager.execute_command("go", "");
    match result {
        Err(PluginError::Script { plugin, detail }) => {
            assert_eq!(plugin, "boom");
            assert!(
                detail.contains("kaboom"),
                "exception message carried; got {detail}"
            );
        }
        other => panic!("uncaught handler exception fails the call; got {other:?}"),
    }
}

#[test]
fn infinite_loop_command_times_out_without_hanging() {
    let tree = TempTree::new("cmd-timeout");
    write_plugin(
        &tree,
        "slow",
        r#"ragent.register_command({ name: "spin", description: "d", handler: function(a){ while(true){} } });"#,
    );
    let mut manager = tree.manager();
    manager.enable("slow").expect("enable");

    // The dispatch wrapper rearms the interrupt per invocation, so an
    // infinite handler trips within this call's own budget window (default
    // `max_execution_ms` in `PluginsConfig`) — the call must return rather
    // than hang (FR-026). The wall-clock bound is generous for slow CI; the
    // contract is "finite".
    let started = std::time::Instant::now();
    let (result, _) = manager.execute_command("spin", "");
    assert!(started.elapsed() < std::time::Duration::from_secs(30));
    assert!(
        matches!(result, Err(PluginError::Timeout)),
        "got {result:?}"
    );
}

#[test]
fn missing_command_handler_fails_with_clear_error() {
    let tree = TempTree::new("cmd-nohandler");
    write_plugin(
        &tree,
        "decl",
        r#"ragent.register_command({ name: "registered", description: "d" });"#,
    );
    let mut manager = tree.manager();
    manager.enable("decl").expect("enable");

    let (result, _) = manager.execute_command("registered", "");
    match result {
        Err(PluginError::Script { detail, .. }) => {
            assert!(
                detail.contains("no handler registered for command"),
                "missing handler named; got {detail}"
            );
        }
        other => panic!("declaration without handler fails cleanly; got {other:?}"),
    }
}

#[test]
fn unknown_command_name_is_unknown_plugin_error() {
    let tree = TempTree::new("cmd-unknown");
    write_plugin(
        &tree,
        "here",
        r#"ragent.register_command({ name: "x", description: "d", handler: function(a){ return "x"; } });"#,
    );
    let mut manager = tree.manager();
    manager.enable("here").expect("enable");

    let (result, unloaded) = manager.execute_command("no-such-command", "");
    assert!(!unloaded, "unknown names never touch telemetry");
    assert!(matches!(result, Err(PluginError::UnknownPlugin(_))));
}

#[test]
fn errored_plugin_commands_are_not_invocable() {
    let tree = TempTree::new("cmd-inert");
    write_plugin(
        &tree,
        "fragile",
        r#"ragent.register_command({ name: "fragile", description: "d", handler: function(a){ return "ok"; } });
           throw new Error("entry failed after registering");"#,
    );
    let mut manager = tree.manager();
    let report = manager.enable("fragile").expect("enable returns a report");
    assert_eq!(report.state, LifecycleState::Errored);

    let (result, _) = manager.execute_command("fragile", "");
    assert!(
        matches!(
            result,
            Err(PluginError::Script { .. }) | Err(PluginError::UnknownPlugin(_))
        ),
        "commands from an errored plugin stay inert; got {result:?}"
    );
}

// ── direct sandbox dispatch (used by the T-015 harness too) ───────────────

#[test]
fn dispatch_command_sandbox_invokes_stashed_handlers_directly() {
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
            r#"ragent.register_command({ name: "shout", description: "d",
               handler: function(args){ return args.toUpperCase(); } });"#,
        )
        .expect("registration script runs");

    let text = dispatch_command_sandbox(&context, "direct", "shout", "make me loud")
        .expect("dispatch works on a bare sandbox");
    assert_eq!(text, "MAKE ME LOUD");
}
