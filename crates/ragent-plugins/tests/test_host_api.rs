//! Tests for the versioned `ragent` host-API bridge (spec `plugins` T-008;
//! FR-004, FR-018, FR-020, FR-027).

use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use ragent_plugins::{
    HostApiInstall, HostCalls, PermissionGate, PluginLogLine, PluginMessage, RuntimePool,
    SandboxBudget, V1_CAPABILITIES, capability_permission_key, read_within,
};
use serde_json::json;

fn budget() -> SandboxBudget {
    SandboxBudget {
        memory_bytes: 16 * 1024 * 1024,
        deadline: Duration::from_secs(2),
        stack_bytes: 256 * 1024,
    }
}

/// RAII sandboxed temp tree rooted at `target/temp/` (AGENTS.md: no `/tmp`).
struct TempTree(PathBuf);

impl TempTree {
    fn new(name: &str) -> Self {
        static SEQ: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
        let unique = SEQ.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(format!(
            "../../target/temp/plugins-test/host-api-{name}-{}-{unique}",
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

/// Checkout, install, eval one script; returns the sink for assertions.
fn run_script_with(install: HostApiInstall, script: &str) -> HostCalls {
    let pool = RuntimePool::new();
    let ctx = pool.checkout(budget()).expect("checkout");
    ctx.install_host_api(&install).expect("install host api");
    ctx.eval(script).expect("script succeeds");
    install.calls()
}

fn gate_allowing(allowed: &'static [&'static str]) -> PermissionGate {
    Arc::new(move |cap: &str| allowed.contains(&cap))
}

fn typeof_all(install: &HostApiInstall) -> String {
    let pool = RuntimePool::new();
    let ctx = pool.checkout(budget()).expect("checkout");
    ctx.install_host_api(install).expect("install host api");
    ctx.eval_to_string(
        "[ ragent.register_tool, ragent.register_command, ragent.config, ragent.message, \
         ragent.log, ragent.plugin, ]
            .map((x) => typeof x)
            .join(',')",
    )
    .expect("eval")
}

// ── FR-004: full surface ────────────────────────────────────────────────────

#[test]
fn installs_the_full_v1_surface_without_a_gate() {
    let install = HostApiInstall::new("codex-weather", PathBuf::from("/plugins/x"));
    let result = typeof_all(&install);
    assert_eq!(
        result,
        V1_CAPABILITIES
            .iter()
            .map(|cap| {
                match *cap {
                    "plugin.read_text_file" | "config" | "message" => "object",
                    _ => "function",
                }
            })
            .collect::<Vec<_>>()
            .join(","),
        "every v1 capability present in declaration order"
    );
}

#[test]
fn api_version_and_plugin_id_are_always_present() {
    let install = HostApiInstall::new("claude-todo", PathBuf::from("/plugins/claude-todo"));
    let pool = RuntimePool::new();
    let ctx = pool.checkout(budget()).expect("checkout");
    ctx.install_host_api(&install).expect("install");
    let probe: String = ctx
        .eval_to_string("String(ragent.api_version) + '|' + ragent.plugin_id")
        .expect("eval");
    assert_eq!(probe, "1|claude-todo");
}

// ── FR-020: permission gate ─────────────────────────────────────────────────

#[test]
fn ungranted_capabilities_are_absent_from_the_injected_object() {
    let install = HostApiInstall::new("codex-weather", PathBuf::from("/plugins/x"))
        .with_gate(gate_allowing(&["tools", "message"]));
    let result = typeof_all(&install);
    assert_eq!(
        result, "function,undefined,undefined,object,undefined,undefined",
        "only tools + message survive the gate"
    );
}

#[test]
fn gate_receives_each_v1_capability_name_exactly_once() {
    let seen = Arc::new(std::sync::Mutex::new(Vec::new()));
    let seen_for_gate = Arc::clone(&seen);
    let gate: PermissionGate = Arc::new(move |cap: &str| {
        seen_for_gate
            .lock()
            .expect("gate log lock")
            .push(cap.to_string());
        true
    });
    let install = HostApiInstall::new("codex-weather", PathBuf::from("/plugins/x")).with_gate(gate);
    let _ = typeof_all(&install);
    let seen = seen.lock().expect("gate log lock").clone();
    // The gate is consulted once per v1 capability, in install order.
    let install_order: Vec<&str> = vec![
        "config",
        "message",
        "log",
        "tools",
        "commands",
        "plugin.read_text_file",
    ];
    assert_eq!(
        seen, install_order,
        "gate consulted exactly once per v1 capability"
    );
    // And every consulted name is a declared v1 capability.
    assert!(seen.iter().all(|c| V1_CAPABILITIES.contains(&c.as_str())));
}

#[test]
fn denied_tools_capability_records_no_registrations() {
    let install = HostApiInstall::new("codex-weather", PathBuf::from("/plugins/x"))
        .with_gate(gate_allowing(&["message"]));
    let calls = run_script_with(
        install,
        "if (typeof ragent.register_tool === 'undefined') { \
            ragent.message.info('no tools capability'); \
        }",
    );
    assert_eq!(
        calls.messages(),
        vec![PluginMessage {
            level: "info".into(),
            text: "no tools capability".into()
        }]
    );
    assert!(calls.tools().is_empty(), "register_tool never existed");
}

#[test]
fn capability_permission_key_is_scoped_to_domain_plugin_and_id() {
    assert_eq!(
        capability_permission_key("codex-weather", "tools"),
        "plugin:codex-weather:tools"
    );
}

// ── registrations (recorded for T-010 / T-012) ──────────────────────────────

#[test]
fn register_tool_and_command_record_declarations_with_json_schemas() {
    let install = HostApiInstall::new("codex-weather", PathBuf::from("/plugins/x"));
    let calls = run_script_with(
        install,
        r#"ragent.register_tool({
           name: "get_weather",
           description: "Look up weather",
           parameters: { type: "object", properties: { city: { type: "string" } } }
       });
       ragent.register_command({
           name: "weather",
           description: "Weather lookup",
           usage: "/weather <city>"
       });"#,
    );
    let tools = calls.tools();
    assert_eq!(tools.len(), 1);
    assert_eq!(tools[0].name, "get_weather");
    assert_eq!(tools[0].description, "Look up weather");
    assert_eq!(
        tools[0].parameters,
        json!({ "type": "object", "properties": { "city": { "type": "string" } } })
    );
    let commands = calls.commands();
    assert_eq!(commands.len(), 1);
    assert_eq!(commands[0].name, "weather");
    assert_eq!(commands[0].usage.as_deref(), Some("/weather <city>"));
}

// ── config.get (FR-004) ─────────────────────────────────────────────────────

#[test]
fn config_get_returns_plugin_scoped_json_text_and_undefined_for_missing_keys() {
    let mut config = std::collections::BTreeMap::new();
    config.insert("units".to_string(), json!("metric"));
    config.insert("limits".to_string(), json!({ "per_day": 100 }));
    let install =
        HostApiInstall::new("codex-weather", PathBuf::from("/plugins/x")).with_config(config);
    let calls = run_script_with(
        install,
        "ragent.message.info(String(ragent.config.get('units')) + '|' + \
         String(ragent.config.get('limits')) + '|' + String(ragent.config.get('nope')))",
    );
    let messages = calls.messages();
    assert_eq!(messages.len(), 1);
    let text = &messages[0].text;
    assert_eq!(text, "\"metric\"|{\"per_day\":100}|undefined");
    // And plugin code parses the JSON text itself.
    let calls = run_script_with(
        HostApiInstall::new("codex-weather", PathBuf::from("/plugins/x")).with_config(
            std::collections::BTreeMap::from([("limits".to_string(), json!({ "per_day": 100 }))]),
        ),
        "ragent.message.info(String(JSON.parse(ragent.config.get('limits')).per_day))",
    );
    assert_eq!(calls.messages()[0].text, "100");
}

// ── message + log sinks (FR-020, FR-027) ────────────────────────────────────

#[test]
fn message_levels_and_log_lines_are_recorded_in_the_sink() {
    let install = HostApiInstall::new("codex-weather", PathBuf::from("/plugins/x"));
    let calls = run_script_with(
        install,
        r#"ragent.message.info("hello");
       ragent.message.warn("careful");
       ragent.message.error("broken");
       ragent.log("debug", "detail line");"#,
    );
    assert_eq!(
        calls.messages(),
        vec![
            PluginMessage {
                level: "info".into(),
                text: "hello".into()
            },
            PluginMessage {
                level: "warn".into(),
                text: "careful".into()
            },
            PluginMessage {
                level: "error".into(),
                text: "broken".into()
            },
        ]
    );
    assert_eq!(
        calls.logs(),
        vec![PluginLogLine {
            level: "debug".into(),
            text: "detail line".into()
        }]
    );
}

// ── plugin.read_text_file (FR-018) ──────────────────────────────────────────

#[test]
fn read_text_file_reads_inside_the_plugin_root() {
    let tree = TempTree::new("read-ok");
    std::fs::create_dir_all(tree.0.join("data")).expect("subdir");
    std::fs::write(tree.0.join("data/info.txt"), "inside the sandbox").expect("written");
    let install = HostApiInstall::new("codex-weather", tree.0.clone());
    let calls = run_script_with(
        install,
        "ragent.message.info(ragent.plugin.read_text_file('data/info.txt'))",
    );
    assert_eq!(calls.messages()[0].text, "inside the sandbox");
}

#[test]
fn read_text_file_refuses_escapes_with_a_caught_exception() {
    let tree = TempTree::new("read-escape");
    std::fs::write(tree.0.join("ok.txt"), "ok").expect("written");
    let install = HostApiInstall::new("codex-weather", tree.0.clone());
    let calls = run_script_with(
        install,
        r#"let seen = [];
       for (const attempt of ["../outside.txt", "/etc/passwd"]) {
           try { ragent.plugin.read_text_file(attempt); }
           catch (e) { seen.push("caught"); }
       }
       ragent.message.info(seen.join(","));"#,
    );
    assert_eq!(calls.messages()[0].text, "caught,caught");
}

#[test]
fn read_text_file_uncaught_escape_surfaces_as_a_script_error_not_a_panic() {
    let tree = TempTree::new("read-escape-uncaught");
    let install = HostApiInstall::new("codex-weather", tree.0.clone());
    let pool = RuntimePool::new();
    let ctx = pool.checkout(budget()).expect("checkout");
    ctx.install_host_api(&install).expect("install");
    let err = ctx
        .eval("ragent.plugin.read_text_file('../escape.txt')")
        .expect_err("escape must fail");
    match err {
        ragent_plugins::PluginError::Script { detail, .. } => {
            assert!(detail.contains("escapes the plugin directory"), "{detail}");
        }
        other => panic!("expected Script error, got {other:?}"),
    }
}

// ── read_within (pure helper used by the bridge) ────────────────────────────

#[test]
fn read_within_refuses_absolute_and_parent_paths_and_reads_children() {
    let tree = TempTree::new("read-within");
    std::fs::create_dir_all(tree.0.join("sub")).expect("subdir");
    std::fs::write(tree.0.join("sub/file.txt"), "payload").expect("written");

    assert_eq!(
        read_within(&tree.0, "sub/file.txt").expect("child read"),
        "payload"
    );
    // Dot segments that stay inside are fine.
    assert_eq!(
        read_within(&tree.0, "sub/./file.txt").expect("dot inside"),
        "payload"
    );

    let err = read_within(&tree.0, "../leak.txt").expect_err("parent refused");
    assert!(err.contains("escapes the plugin directory"), "{err}");
    let err = read_within(&tree.0, "/abs.txt").expect_err("absolute refused");
    assert!(err.contains("absolute path refused"), "{err}");
    let err = read_within(&tree.0, "sub/../../leak.txt").expect_err("mid-path refused");
    assert!(err.contains("escapes the plugin directory"), "{err}");
    let err = read_within(&tree.0, "sub/missing.txt").expect_err("io error surfaced");
    assert!(err.contains("plugin.read_text_file"), "{err}");
}

// ── FR-018: no ambient host access ──────────────────────────────────────────

#[test]
fn installed_context_still_exposes_no_ambient_host_apis() {
    let install = HostApiInstall::new("codex-weather", PathBuf::from("/plugins/x"));
    let pool = RuntimePool::new();
    let ctx = pool.checkout(budget()).expect("checkout");
    ctx.install_host_api(&install).expect("install");
    let kinds: String = ctx
        .eval_to_string(
            "['std','os','print','require','fetch','process','Deno','Bun','XMLHttpRequest']\
             .map((n) => typeof globalThis[n]).join(',')",
        )
        .expect("eval");
    assert_eq!(
        kinds,
        "undefined,undefined,undefined,undefined,undefined,undefined,undefined,undefined,undefined"
    );
}
