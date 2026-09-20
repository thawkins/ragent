//! Tests for the `/plugins test` isolated harness (spec `plugins` T-015;
//! FR-013, FR-026).

use std::path::PathBuf;
use std::time::Duration;

use ragent_plugins::{
    HarnessReport, StepOutcome, StoreLedger, parse_test_command, render_report, run_test_command,
    sample_for_schema, store_dirs_at, test_plugin,
};
use serde_json::{Value as JsonValue, json};

static SEQ: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);

/// RAII sandboxed temp tree rooted at `target/temp/` (AGENTS.md: no `/tmp`).
struct TempTree(PathBuf);

impl TempTree {
    fn new(name: &str) -> Self {
        let unique = SEQ.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(format!(
            "../../target/temp/plugins-test/harness-{name}-{}-{unique}",
            std::process::id()
        ));
        std::fs::create_dir_all(&path).expect("temp tree creatable");
        Self(path)
    }

    /// The project store (`<tree>/.ragent/plugins/`).
    fn store(&self) -> PathBuf {
        self.0.join(".ragent/plugins")
    }

    /// Store dirs pinned to this tree's project store (no global leg).
    fn dirs(&self) -> ragent_plugins::StoreDirs {
        store_dirs_at(&self.0, Some(&self.store()), None)
    }
}

impl Drop for TempTree {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// Write a Codex-dialect plugin with an explicit id and an inline entry.
fn write_plugin(tree: &TempTree, id: &str, entry: &str) {
    write_plugin_manifest(
        tree,
        id,
        &format!(r#"{{ "id": "{id}", "name": "{id}", "version": "1.0.0", "entry": "index.js" }}"#),
        entry,
    );
}

/// Write a plugin with a caller-supplied manifest body and entry source.
fn write_plugin_manifest(tree: &TempTree, id: &str, manifest: &str, entry: &str) {
    let plugin_dir = tree.store().join(id);
    std::fs::create_dir_all(&plugin_dir).expect("plugin dir creatable");
    std::fs::write(plugin_dir.join("codex-plugin.json"), manifest).expect("manifest writable");
    std::fs::write(plugin_dir.join("index.js"), entry).expect("entry writable");
}

fn config() -> ragent_config::PluginsConfig {
    ragent_config::PluginsConfig::default()
}

/// The step with `name` in the report, or panic.
fn step<'a>(report: &'a HarnessReport, name: &str) -> &'a ragent_plugins::HarnessStep {
    report
        .steps
        .iter()
        .find(|s| s.name == name)
        .unwrap_or_else(|| panic!("step '{name}' missing from {:?}", report.steps))
}

// ── FR-013: happy path ─────────────────────────────────────────────────────

#[test]
fn harness_runs_discovery_validation_version_entry_and_sample_invocation() {
    let tree = TempTree::new("happy");
    write_plugin(
        &tree,
        "codex-weather",
        r#"
        ragent.register_tool({
          name: "get_weather",
          description: "Get weather by city",
          parameters: { type: "object", properties: { city: { type: "string" } }, required: ["city"] },
          handler: function (args) { return { content: "sunny in " + args.city }; }
        });
        "#,
    );

    let report = test_plugin(tree.dirs(), &config(), "codex-weather");

    assert_eq!(report.steps.len(), 5, "steps: {:?}", report.steps);
    assert_eq!(step(&report, "discovery").outcome, StepOutcome::Pass);
    assert_eq!(
        step(&report, "manifest validation").outcome,
        StepOutcome::Pass
    );
    assert_eq!(step(&report, "version check").outcome, StepOutcome::Pass);
    assert_eq!(step(&report, "entry execution").outcome, StepOutcome::Pass);
    assert_eq!(
        step(&report, "sample invocation get_weather").outcome,
        StepOutcome::Pass
    );
    assert!(report.passed());

    let rendered = render_report(&report);
    assert!(rendered.contains("From: /plugins test codex-weather"));
    assert!(rendered.contains("[ ok ] sample invocation get_weather"));
    assert!(
        rendered.contains("args: {\"city\":\"sample\"}"),
        "schema-valid args shown: {rendered}"
    );
    assert!(rendered.contains("sunny in sample"));
    assert!(rendered.contains("[ ok ] harness complete"));
}

#[test]
fn harness_uses_declared_parameter_defaults_and_const() {
    let tree = TempTree::new("schema-defaults");
    write_plugin(
        &tree,
        "defaults",
        r#"
        ragent.register_tool({
          name: "t",
          description: "d",
          parameters: { type: "object", properties: {
            fixed: { const: "pinned" },
            count: { type: "integer", default: 7 },
            maybe: { type: "string", enum: ["alpha", "beta"] }
          } },
          handler: function (a) { return { content: JSON.stringify(a) }; }
        });
        "#,
    );

    let report = test_plugin(tree.dirs(), &config(), "defaults");
    assert!(report.passed(), "{:?}", report.steps);
    let detail = step(&report, "sample invocation t")
        .detail
        .clone()
        .unwrap_or_default();
    assert!(detail.contains("\"fixed\":\"pinned\""), "{detail}");
    assert!(detail.contains("\"count\":7"), "{detail}");
    assert!(detail.contains("\"maybe\":\"alpha\""), "{detail}");
}

#[test]
fn harness_invokes_a_manifest_declared_tool_once() {
    let tree = TempTree::new("manifest-tool");
    write_plugin_manifest(
        &tree,
        "manifested",
        r#"{
            "id": "manifested", "name": "manifested", "version": "1.0.0", "entry": "index.js",
            "tools": [ { "name": "add_todo", "description": "add", "parameters": { "type": "object" } } ]
        }"#,
        r#"globalThis.add_todo = function (a) { return { content: "added" }; };"#,
    );

    let report = test_plugin(tree.dirs(), &config(), "manifested");
    assert!(report.passed(), "{:?}", report.steps);
    let invocations = report
        .steps
        .iter()
        .filter(|s| s.name.starts_with("sample invocation"))
        .count();
    assert_eq!(invocations, 1, "{:?}", report.steps);
}

#[test]
fn harness_deduplicates_manifest_and_registered_tool_of_same_name() {
    let tree = TempTree::new("dedupe");
    write_plugin_manifest(
        &tree,
        "dupe",
        r#"{
            "id": "dupe", "name": "dupe", "version": "1.0.0", "entry": "index.js",
            "tools": [ { "name": "same", "description": "declared", "parameters": { "type": "object" } } ]
        }"#,
        r#"ragent.register_tool({ name: "same", description: "registered", parameters: { type: "object" }, handler: function (a) { return { content: "ok" }; } });"#,
    );

    let report = test_plugin(tree.dirs(), &config(), "dupe");
    assert!(report.passed(), "{:?}", report.steps);
    let invocations = report
        .steps
        .iter()
        .filter(|s| s.name == "sample invocation same")
        .count();
    assert_eq!(invocations, 1, "{:?}", report.steps);
}

#[test]
fn harness_captures_plugin_messages() {
    let tree = TempTree::new("messages");
    write_plugin(
        &tree,
        "chatty",
        r#"ragent.message.info("hello from plugin"); ragent.message.warn("careful");"#,
    );

    let report = test_plugin(tree.dirs(), &config(), "chatty");
    assert!(report.passed(), "{:?}", report.steps);
    assert_eq!(report.messages.len(), 2, "{:?}", report.messages);
    assert_eq!(report.messages[0].level, "info");
    assert_eq!(report.messages[0].text, "hello from plugin");
    assert_eq!(report.messages[1].level, "warn");

    let rendered = render_report(&report);
    assert!(rendered.contains("Captured messages (2):"));
    assert!(rendered.contains("info: hello from plugin"));
}

// ── FR-013/FR-026: failure containment ─────────────────────────────────────

#[test]
fn harness_reports_invalid_manifest_at_validation() {
    let tree = TempTree::new("bad-manifest");
    write_plugin_manifest(&tree, "broken", "{ not json", "0;");

    let report = test_plugin(tree.dirs(), &config(), "broken");
    assert_eq!(step(&report, "discovery").outcome, StepOutcome::Pass);
    assert!(matches!(
        step(&report, "manifest validation").outcome,
        StepOutcome::Fail(_)
    ));
    assert!(
        step(&report, "manifest validation").outcome != StepOutcome::Pass,
        "stops after validation failure"
    );
    assert!(!report.passed());
    let rendered = render_report(&report);
    assert!(rendered.contains("[fail] manifest validation"));
    assert!(rendered.contains("[fail] harness aborted"));
}

#[test]
fn harness_refuses_newer_host_api_version() {
    let tree = TempTree::new("version");
    write_plugin_manifest(
        &tree,
        "futuristic",
        r#"{ "id": "futuristic", "name": "futuristic", "version": "1.0.0", "entry": "index.js", "api_version": 99 }"#,
        "0;",
    );

    let report = test_plugin(tree.dirs(), &config(), "futuristic");
    assert_eq!(
        step(&report, "manifest validation").outcome,
        StepOutcome::Pass
    );
    match &step(&report, "version check").outcome {
        StepOutcome::Fail(cause) => assert!(cause.contains("v99"), "{cause}"),
        other => panic!("expected version mismatch, got {other:?}"),
    }
    // Entry execution never runs after a refused version.
    assert!(report.steps.iter().all(|s| s.name != "entry execution"));
}

#[test]
fn harness_contains_entry_exception_and_reports_cause() {
    let tree = TempTree::new("entry-throw");
    write_plugin(&tree, "thrower", "throw new Error('entry exploded');");

    let report = test_plugin(tree.dirs(), &config(), "thrower");
    match &step(&report, "entry execution").outcome {
        StepOutcome::Fail(cause) => assert!(cause.contains("entry exploded"), "{cause}"),
        other => panic!("expected entry failure, got {other:?}"),
    }
    assert!(!report.passed());
}

#[test]
fn harness_contains_infinite_loop_entry_within_entry_budget() {
    let tree = TempTree::new("loop");
    write_plugin(&tree, "loop-forever", "while (true) {}");

    let mut cfg = config();
    cfg.max_entry_ms = 300;
    let started = std::time::Instant::now();
    let report = test_plugin(tree.dirs(), &cfg, "loop-forever");
    let elapsed = started.elapsed();

    match &step(&report, "entry execution").outcome {
        StepOutcome::Fail(cause) => assert!(cause.contains("timeout"), "{cause}"),
        other => panic!("expected timeout failure, got {other:?}"),
    }
    assert!(
        elapsed < Duration::from_secs(10),
        "harness must return promptly; took {elapsed:?}"
    );
}

#[test]
fn harness_reports_tool_exception_as_failed_sample_invocation() {
    let tree = TempTree::new("tool-throw");
    write_plugin(
        &tree,
        "tool-thrower",
        r#"ragent.register_tool({ name: "boom", description: "d", parameters: { type: "object" }, handler: function (a) { throw new Error('tool exploded'); } });"#,
    );

    let report = test_plugin(tree.dirs(), &config(), "tool-thrower");
    assert_eq!(step(&report, "entry execution").outcome, StepOutcome::Pass);
    match &step(&report, "sample invocation boom").outcome {
        StepOutcome::Fail(cause) => assert!(cause.contains("tool exploded"), "{cause}"),
        other => panic!("expected tool failure, got {other:?}"),
    }
}

#[test]
fn harness_reports_unknown_plugin_as_discovery_failure() {
    let tree = TempTree::new("unknown");
    let report = test_plugin(tree.dirs(), &config(), "nope");
    assert_eq!(report.steps.len(), 1);
    match &report.steps[0].outcome {
        StepOutcome::Fail(cause) => assert!(cause.contains("unknown plugin: nope"), "{cause}"),
        other => panic!("expected discovery failure, got {other:?}"),
    }
    assert!(!report.passed());
}

// ── FR-013: isolation — live session and store untouched ───────────────────

#[test]
fn harness_does_not_write_the_store_ledger_or_enable_the_plugin() {
    let tree = TempTree::new("isolation");
    write_plugin(
        &tree,
        "codex-weather",
        r#"ragent.register_tool({ name: "get_weather", description: "d", parameters: { type: "object" }, handler: function (a) { return { content: "ok" }; } });"#,
    );

    let report = test_plugin(tree.dirs(), &config(), "codex-weather");
    assert!(report.passed(), "{:?}", report.steps);

    // The plugin remains disabled and no ledger state was persisted for it.
    let ledger = StoreLedger::load(&tree.store());
    let enabled = ledger.state("codex-weather").is_some_and(|s| s.enabled);
    assert!(!enabled, "harness must not enable the plugin");
    assert!(
        ledger.state("codex-weather").is_none(),
        "harness must not write ledger rows"
    );
    assert!(
        !tree.store().join("_state.json").exists(),
        "harness must not create a ledger file"
    );
}

// ── FR-025: unsupported capabilities reported ──────────────────────────────

#[test]
fn harness_reports_unsupported_capabilities() {
    let tree = TempTree::new("unsupported");
    write_plugin_manifest(
        &tree,
        "legacy",
        r#"{
            "id": "legacy", "name": "legacy", "version": "1.0.0", "entry": "index.js",
            "mcp_servers": { "db": { "command": "db-server" } }
        }"#,
        "0;",
    );

    let report = test_plugin(tree.dirs(), &config(), "legacy");
    assert!(report.passed(), "{:?}", report.steps);
    assert!(!report.unsupported.is_empty());
    let rendered = render_report(&report);
    assert!(rendered.contains("Unsupported capabilities:"));
    assert!(rendered.contains("mcp server transports"), "{rendered}");
}

// ── FR-013: `run_test_command` / parse glue ───────────────────────────────

#[test]
fn run_test_command_refuses_when_subsystem_disabled() {
    let tree = TempTree::new("disabled-subsystem");
    write_plugin(
        &tree,
        "codex-weather",
        r#"ragent.register_tool({ name: "t", description: "d", parameters: { type: "object" }, handler: function (a) { return { content: "ok" }; } });"#,
    );

    let mut cfg = config();
    cfg.enabled = false;
    let rendered =
        run_test_command(tree.dirs(), &cfg, "test", "codex-weather").expect("test subcommand");
    assert!(rendered.contains("[err]"), "{rendered}");
    assert!(rendered.contains("disabled"), "{rendered}");
    // No ledger file was created: disabled subsystem means nothing ran.
    assert!(!tree.store().join("_state.json").exists());
}

#[test]
fn run_test_command_returns_none_for_other_subcommands() {
    let tree = TempTree::new("other-sub");
    assert!(run_test_command(tree.dirs(), &config(), "list", "").is_none());
    assert!(run_test_command(tree.dirs(), &config(), "enable", "x").is_none());
}

#[test]
fn parse_test_command_handles_id_and_missing_id() {
    assert_eq!(parse_test_command("list", ""), None);
    assert_eq!(
        parse_test_command("test", "claude-todo"),
        Some(Ok("claude-todo".to_string()))
    );
    assert_eq!(
        parse_test_command("test", "  claude-todo  extra"),
        Some(Ok("claude-todo".to_string()))
    );
    assert!(matches!(
        parse_test_command("test", "   "),
        Some(Err(ragent_plugins::TestArgError::MissingTestId))
    ));
}

#[test]
fn run_test_command_missing_id_renders_usage_error() {
    let tree = TempTree::new("missing-id");
    let rendered = run_test_command(tree.dirs(), &config(), "test", "").expect("test subcommand");
    assert!(rendered.contains("[err] Missing <pluginid>."), "{rendered}");
    assert!(
        rendered.contains("Usage: `/plugins test <pluginid>`"),
        "{rendered}"
    );
}

// ── sample argument generation ─────────────────────────────────────────────

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
        sample_for_schema(&json!({ "type": "null" })),
        JsonValue::Null
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
        sample_for_schema(&json!({ "type": "string", "examples": ["e1", "e2"] })),
        json!("e1")
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
            "name": { "type": "string" },
            "nested": {
                "type": "object",
                "properties": { "flag": { "type": "boolean" } }
            },
            "items": { "type": "array", "items": { "type": "integer" } },
            "bare_array": { "type": "array" }
        }
    });
    assert_eq!(
        sample_for_schema(&schema),
        json!({
            "name": "sample",
            "nested": { "flag": false },
            "items": [0],
            "bare_array": []
        })
    );
    // A schema with no type defaults to an object placeholder.
    assert_eq!(sample_for_schema(&json!({})), json!({}));
}
