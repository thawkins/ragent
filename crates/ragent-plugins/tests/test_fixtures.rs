//! Fixture-driven acceptance tests for the plugin system (spec `plugins`
//! T-018): the repository fixture plugins under `assets/plugins/fixtures/` are
//! walked end to end against the crate API, covering acceptance criteria 1-9,
//! 13, and 14 plus the NFR timing checks (Performance section).
//!
//! The fixtures are the same artifacts the manual TESTPLAN walk uses, so a
//! regression here means the documented walk would also fail. Each test stages
//! a fixture copy into a sandboxed temp store (`target/temp/`, per AGENTS.md)
//! via the real `add` path, then drives discovery / enable / disable / test.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, Instant};

use ragent_config::PluginsConfig;
use ragent_plugins::{
    HarnessReport, LifecycleState, PluginCommandAdapter, PluginSession, PluginSurface,
    PluginToolAdapter, StepOutcome, StoreDirs, StoreLedger, add, run_test_command, scan_dirs,
    store_dirs_at, test_plugin,
};
use ragent_tools_core::Tool;
use serde_json::json;

static SEQ: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);

/// Workspace fixture root: `assets/plugins/fixtures/`.
fn fixtures_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../assets/plugins/fixtures")
}

/// RAII sandboxed temp tree rooted at `target/temp/` (AGENTS.md: no `/tmp`).
struct TempTree(PathBuf);

impl TempTree {
    fn new(name: &str) -> Self {
        let unique = SEQ.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(format!(
            "../../target/temp/plugins-test/fixtures-{name}-{}-{unique}",
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
    fn dirs(&self) -> StoreDirs {
        store_dirs_at(&self.0, Some(&self.store()), None)
    }

    /// Install fixture `name` through the real `add` path and return its id.
    fn install(&self, name: &str) -> String {
        let source = fixtures_root().join(name);
        let outcome = add(&self.dirs(), &self.0, source.to_str().expect("utf8"), false)
            .unwrap_or_else(|e| panic!("install fixture {name}: {e}"));
        outcome.parsed.descriptor.id
    }
}

impl Drop for TempTree {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// A test surface recording registrations/deregistrations.
#[derive(Default)]
struct TestSurface {
    tools: BTreeSet<String>,
    commands: BTreeSet<String>,
    deregistered_tools: Vec<String>,
    deregistered_commands: Vec<String>,
}

impl PluginSurface for TestSurface {
    fn existing_tool_names(&self) -> BTreeSet<String> {
        self.tools.clone()
    }

    fn existing_command_names(&self) -> BTreeSet<String> {
        self.commands.clone()
    }

    fn register_tool(&mut self, adapter: Arc<PluginToolAdapter>) {
        self.tools.insert(adapter.name().to_string());
    }

    fn register_command(&mut self, adapter: Arc<PluginCommandAdapter>) {
        self.commands.insert(adapter.name().to_string());
    }

    fn deregister_tool(&mut self, registry_name: &str) {
        self.tools.remove(registry_name);
        self.deregistered_tools.push(registry_name.to_string());
    }

    fn deregister_command(&mut self, trigger: &str) {
        self.commands.remove(trigger);
        self.deregistered_commands.push(trigger.to_string());
    }
}

fn harness_step<'a>(report: &'a HarnessReport, name: &str) -> &'a ragent_plugins::HarnessStep {
    report
        .steps
        .iter()
        .find(|s| s.name == name)
        .unwrap_or_else(|| panic!("step '{name}' missing from {:?}", report.steps))
}

// ── fixture inventory ───────────────────────────────────────────────────────

#[test]
fn all_documented_fixtures_are_present_and_recognised() {
    let expected = [
        ("codex-weather", "codex"),
        ("claude-todo", "claude"),
        ("loop-forever", "codex"),
        ("escape-attempt", "codex"),
        ("future-api", "codex"),
        ("claude-mcp", "claude"),
        ("collider", "codex"),
    ];
    for (name, dialect) in expected {
        let root = fixtures_root().join(name);
        assert!(root.is_dir(), "fixture {name} must exist at {root:?}");
        let matched = ragent_plugins::detect_dialect(&root)
            .unwrap_or_else(|e| panic!("detect {name}: {e}"))
            .unwrap_or_else(|| panic!("fixture {name} must be recognised"));
        assert_eq!(matched.dialect.to_string(), dialect, "fixture {name}");
    }
    // The malformed fixture is intentionally unparseable but still recognised.
    let bad = fixtures_root().join("bad-manifest");
    assert!(bad.is_dir(), "fixture bad-manifest must exist");
    assert!(
        ragent_plugins::parse_plugin_dir(&bad).is_err(),
        "bad-manifest must fail to parse"
    );
}

// ── acceptance criterion 1: install both dialects, listed disabled ──────────

#[test]
fn acceptance_1_installs_both_dialects_and_lists_them_disabled() {
    let tree = TempTree::new("ac1");
    let codex = tree.install("codex-weather");
    let claude = tree.install("claude-todo");
    assert_eq!(codex, "codex-weather");
    assert_eq!(claude, "claude-todo");

    let found = scan_dirs(tree.dirs());
    assert_eq!(found.len(), 2, "both fixtures discovered");
    assert!(found.iter().all(|p| !p.enabled), "disabled until enabled");

    let versions: BTreeSet<String> = found
        .iter()
        .map(|p| {
            p.outcome
                .as_ref()
                .expect("parsed")
                .descriptor
                .version
                .clone()
        })
        .collect();
    assert!(versions.contains("1.2.0"), "codex version: {versions:?}");
    assert!(versions.contains("0.3.0"), "claude version: {versions:?}");
}

// ── acceptance criterion 2: enable loads, tool returns canned JSON ──────────

#[test]
fn acceptance_2_enable_loads_and_tool_returns_canned_json() {
    let tree = TempTree::new("ac2");
    tree.install("codex-weather");

    let mut surface = TestSurface::default();
    let mut session = PluginSession::start(tree.dirs(), PluginsConfig::default(), &mut surface);
    let outcome = session
        .enable("codex-weather", &mut surface)
        .expect("enable");
    assert_eq!(outcome.report.state, LifecycleState::Loaded);
    assert_eq!(
        session.manager().state_of("codex-weather"),
        LifecycleState::Loaded
    );
    assert!(surface.tools.contains("plugin_codex-weather_get_weather"));

    let (result, _) = session.manager_mut().execute_tool(
        "plugin_codex-weather_get_weather",
        json!({ "city": "Helsinki" }),
    );
    let output = result.expect("tool dispatch");
    assert!(
        output.content.contains("\"city\":\"Helsinki\""),
        "{}",
        output.content
    );
    assert!(output.content.contains("clear skies"), "{}", output.content);
}

// ── acceptance criterion 3: disable deregisters the tool ────────────────────

#[test]
fn acceptance_3_disable_deregisters_the_tool() {
    let tree = TempTree::new("ac3");
    tree.install("codex-weather");
    let mut surface = TestSurface::default();
    let mut session = PluginSession::start(tree.dirs(), PluginsConfig::default(), &mut surface);
    session
        .enable("codex-weather", &mut surface)
        .expect("enable");

    let disabled = session
        .disable("codex-weather", &mut surface)
        .expect("disable");
    assert_eq!(disabled.tools_deregistered, 1);
    assert_eq!(disabled.commands_deregistered, 0);
    assert!(!surface.tools.contains("plugin_codex-weather_get_weather"));
    assert_eq!(
        session.manager().state_of("codex-weather"),
        LifecycleState::Disabled
    );

    // A direct invocation now reports an unknown tool, not a plugin execution.
    let (result, _) = session.manager_mut().execute_tool(
        "plugin_codex-weather_get_weather",
        json!({ "city": "Oslo" }),
    );
    assert!(result.is_err(), "disabled plugin tool must be unavailable");
}

// ── acceptance criterion 4: isolated harness on the Claude fixture ──────────

#[test]
fn acceptance_4_harness_passes_every_step_without_touching_the_session() {
    let tree = TempTree::new("ac4");
    tree.install("claude-todo");

    let report = test_plugin(tree.dirs(), &PluginsConfig::default(), "claude-todo");
    assert!(report.passed(), "report: {:?}", report.steps);
    for step in [
        "manifest validation",
        "version check",
        "entry execution",
        "sample invocation add_todo",
    ] {
        let step = harness_step(&report, step);
        assert_eq!(step.outcome, StepOutcome::Pass, "{step:?}");
    }
    // The manifest and sample-invocation steps carry human-readable detail.
    assert!(
        harness_step(&report, "manifest validation")
            .detail
            .is_some()
    );
    let sample = harness_step(&report, "sample invocation add_todo");
    assert!(
        sample
            .detail
            .as_deref()
            .unwrap_or_default()
            .contains("args:"),
        "sample invocation shows generated args: {sample:?}"
    );

    // Nothing was registered and the store ledger was not written (FR-013).
    assert!(!tree.store().join("_state.json").exists());
    let found = scan_dirs(tree.dirs());
    assert!(!found[0].enabled, "harness must not enable the plugin");
}

// ── acceptance criterion 5: infinite loop contained by the entry budget ─────

#[test]
fn acceptance_5_infinite_loop_is_contained_by_the_entry_budget() {
    let tree = TempTree::new("ac5");
    tree.install("loop-forever");

    let config = PluginsConfig {
        max_entry_ms: 400,
        ..PluginsConfig::default()
    };
    let mut surface = TestSurface::default();
    let mut session = PluginSession::start(tree.dirs(), config, &mut surface);

    let started = Instant::now();
    let outcome = session
        .enable("loop-forever", &mut surface)
        .expect("enable returns a report");
    let elapsed = started.elapsed();

    assert_eq!(outcome.report.state, LifecycleState::Errored);
    let cause = outcome.report.error.clone().unwrap_or_default();
    assert!(cause.contains("entry"), "cause names entry: {cause}");
    assert!(cause.contains("timeout"), "cause names timeout: {cause}");
    assert!(
        elapsed < Duration::from_secs(5),
        "contained promptly; took {elapsed:?}"
    );
    assert!(
        surface.tools.is_empty(),
        "errored plugin contributes nothing"
    );
}

// ── acceptance criterion 6: filesystem escape denied ────────────────────────

#[test]
fn acceptance_6_filesystem_escape_is_denied() {
    let tree = TempTree::new("ac6");
    tree.install("escape-attempt");
    // Sentinel outside the plugin directory but inside the tree.
    let outside = tree.0.join("outside.txt");
    std::fs::write(&outside, "SENTINEL-DO-NOT-READ").expect("sentinel writable");

    let mut surface = TestSurface::default();
    let mut session = PluginSession::start(tree.dirs(), PluginsConfig::default(), &mut surface);
    session
        .enable("escape-attempt", &mut surface)
        .expect("enable");

    let (result, _) = session
        .manager_mut()
        .execute_tool("plugin_escape-attempt_read_outside", json!({}));
    let output = result.expect("handler returns its refusal record");
    assert!(
        output.content.contains("\"escaped\":false"),
        "the escape attempt must be refused: {}",
        output.content
    );
    // The sentinel is untouched; nothing was read into the result.
    assert_eq!(
        std::fs::read_to_string(&outside).expect("sentinel readable"),
        "SENTINEL-DO-NOT-READ"
    );
    assert!(!output.content.contains("SENTINEL-DO-NOT-READ"));
}

// ── acceptance criterion 7: help / bare / bogus share one usage block ───────

#[test]
fn acceptance_7_usage_block_is_shared_and_ascii_only() {
    let bare = ragent_plugins::render_help("");
    let help = ragent_plugins::render_help("help");
    let bogus = ragent_plugins::render_help("bogus");

    // All three carry the identical usage body (FR-014).
    let body = bare.split_once("\n\n").expect("attribution + body").1;
    assert!(help.ends_with(body), "help reuse the body");
    assert!(bogus.ends_with(body), "bogus reuse the body");
    for sub in ["list", "add", "remove", "enable", "disable", "test", "help"] {
        assert!(body.contains(sub), "usage documents `{sub}`");
    }
    assert!(
        body.contains("https://"),
        "usage documents the URL source form"
    );
    assert!(body.is_ascii(), "usage block must be ASCII only");

    // Rendering is pure: it creates no files under a scratch tree (FR-014).
    let tree = TempTree::new("ac7");
    let before = dir_entries(&tree.0);
    let _ = ragent_plugins::render_help("");
    let _ = ragent_plugins::render_help("help");
    let _ = ragent_plugins::render_help("bogus");
    assert_eq!(before, dir_entries(&tree.0), "help must create no files");
}

/// Recursively list every entry under `root` (sorted), for no-file-creation
/// assertions.
fn dir_entries(root: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    let mut stack = vec![root.to_path_buf()];
    while let Some(dir) = stack.pop() {
        let Ok(read) = std::fs::read_dir(&dir) else {
            continue;
        };
        for entry in read.flatten() {
            let path = entry.path();
            out.push(path.clone());
            if path.is_dir() {
                stack.push(path);
            }
        }
    }
    out.sort();
    out
}

// ── acceptance criterion 8: `plugins.enabled: false` is inert ───────────────

#[test]
fn acceptance_8_master_switch_disables_the_subsystem() {
    let tree = TempTree::new("ac8");
    tree.install("codex-weather");
    // Mark it enabled in the ledger so an inert session is demonstrably not
    // loading it.
    let mut ledger = StoreLedger::load(&tree.store());
    ledger.state_mut("codex-weather").enabled = true;
    ledger.save(&tree.store()).expect("ledger save");

    let config = PluginsConfig {
        enabled: false,
        ..PluginsConfig::default()
    };
    let mut surface = TestSurface::default();
    let mut session = PluginSession::start(tree.dirs(), config.clone(), &mut surface);

    assert!(session.reports().is_empty(), "no discovery/loading runs");
    assert!(surface.tools.is_empty(), "nothing registered");

    // Control subcommands report the disabled subsystem and change no state.
    for sub in ["list", "enable", "disable"] {
        let report =
            ragent_plugins::run_control_command(&mut session, &mut surface, sub, "codex-weather")
                .expect("control subcommand handled");
        assert!(report.contains("[err]"), "{sub}: {report}");
        assert!(report.contains("disabled"), "{sub}: {report}");
    }
}

// ── acceptance criterion 9: newer host-API version refused ──────────────────

#[test]
fn acceptance_9_newer_api_version_is_refused_at_enable_and_test() {
    let tree = TempTree::new("ac9");
    tree.install("future-api");

    let mut surface = TestSurface::default();
    let mut session = PluginSession::start(tree.dirs(), PluginsConfig::default(), &mut surface);
    let outcome = session.enable("future-api", &mut surface).expect("enable");
    assert_eq!(outcome.report.state, LifecycleState::Errored);
    let cause = outcome.report.error.clone().unwrap_or_default();
    assert!(cause.contains("v99"), "names the declared version: {cause}");

    let report = test_plugin(tree.dirs(), &PluginsConfig::default(), "future-api");
    match &harness_step(&report, "version check").outcome {
        StepOutcome::Fail(cause) => assert!(cause.contains("v99"), "{cause}"),
        other => panic!("expected version refusal, got {other:?}"),
    }
    assert!(
        report.steps.iter().all(|s| s.name != "entry execution"),
        "the entry must never run"
    );
}

// ── TC-013: unsupported capabilities reported, not dropped ──────────────────

#[test]
fn tc_013_unsupported_capabilities_are_reported() {
    let tree = TempTree::new("tc13");
    tree.install("claude-mcp");

    let report = test_plugin(tree.dirs(), &PluginsConfig::default(), "claude-mcp");
    assert!(
        report
            .unsupported
            .contains(&"mcp server transports".to_string()),
        "unsupported labels: {:?}",
        report.unsupported
    );
    let rendered = ragent_plugins::render_report(&report);
    assert!(rendered.contains("Unsupported capabilities:"), "{rendered}");
    assert!(rendered.contains("mcp server transports"), "{rendered}");
}

// ── TC-014: duplicate tool name is a real registry collision ────────────────

#[test]
fn tc_014_duplicate_tool_name_is_rejected_and_leaves_the_surface_intact() {
    let tree = TempTree::new("tc14");
    tree.install("collider");

    // Seed the surface with the built-in `bash` tool: the plugin must never be
    // able to displace it.
    let mut surface = TestSurface::default();
    surface.tools.insert("bash".to_string());
    let mut session = PluginSession::start(tree.dirs(), PluginsConfig::default(), &mut surface);
    let outcome = session.enable("collider", &mut surface).expect("enable");

    // The duplicate declaration is refused; the plugin contributes nothing.
    assert!(
        outcome.registration.error.is_some(),
        "collision must be reported: {outcome:?}"
    );
    assert_eq!(
        session.manager().state_of("collider"),
        LifecycleState::Errored
    );
    assert!(!surface.tools.contains("plugin_collider_bash"));
    assert!(surface.tools.contains("bash"), "built-in bash is untouched");
}

// ── NFR: discovery under 2 s for 50 installed plugins ───────────────────────

#[test]
fn nfr_discovery_of_fifty_plugins_is_under_two_seconds() {
    let tree = TempTree::new("nfr-discovery");
    let dirs = tree.dirs();
    let store = tree.store();
    for index in 0..50 {
        let dir = store.join(format!("bulk-{index}"));
        std::fs::create_dir_all(&dir).expect("plugin dir");
        std::fs::write(
            dir.join("codex-plugin.json"),
            format!(
                r#"{{ "id": "bulk-{index}", "name": "Bulk {index}", "version": "1.0.0",
                     "entry": "index.js",
                     "tools": [{{ "name": "t", "description": "d", "parameters": {{ "type": "object" }} }}] }}"#
            ),
        )
        .expect("manifest");
        std::fs::write(dir.join("index.js"), "// no execution on discovery").expect("entry");
    }

    let started = Instant::now();
    let found = scan_dirs(dirs);
    let elapsed = started.elapsed();

    assert_eq!(found.len(), 50);
    assert!(found.iter().all(|p| !p.enabled), "discovery never enables");
    assert!(
        elapsed < Duration::from_secs(2),
        "discovery of 50 plugins took {elapsed:?}, over the 2 s budget"
    );
}

// ── NFR: `/plugins test` on a 10-tool plugin under 15 s ─────────────────────

#[test]
fn nfr_test_of_ten_tool_plugin_is_under_fifteen_seconds() {
    let tree = TempTree::new("nfr-test");
    let dir = tree.store().join("many-tools");
    std::fs::create_dir_all(&dir).expect("plugin dir");
    let tools: Vec<String> = (0..10)
        .map(|i| {
            format!(
                r#"{{ "name": "tool_{i}", "description": "tool {i}", "parameters": {{ "type": "object", "properties": {{ "n": {{ "type": "integer" }} }} }} }}"#
            )
        })
        .collect();
    std::fs::write(
        dir.join("codex-plugin.json"),
        format!(
            r#"{{ "id": "many-tools", "name": "many-tools", "version": "1.0.0",
                 "entry": "index.js", "tools": [{}] }}"#,
            tools.join(", ")
        ),
    )
    .expect("manifest");
    let handlers: Vec<String> = (0..10)
        .map(|i| format!("function tool_{i}(a) {{ return {{ ok: true }}; }}"))
        .collect();
    std::fs::write(dir.join("index.js"), handlers.join("\n")).expect("entry");

    let started = Instant::now();
    let rendered = run_test_command(tree.dirs(), &PluginsConfig::default(), "test", "many-tools")
        .expect("test subcommand");
    let elapsed = started.elapsed();

    assert!(rendered.contains("[ ok ] harness complete"), "{rendered}");
    for i in 0..10 {
        assert!(
            rendered.contains(&format!("sample invocation tool_{i}")),
            "tool_{i} must be sampled: {rendered}"
        );
    }
    assert!(
        elapsed < Duration::from_secs(15),
        "10-tool harness took {elapsed:?}, over the 15 s budget"
    );
}
