//! Non-JS (skill-only / MCP-only) plugin plugins (spec `plugins`; FR-002,
//! FR-007, FR-008, FR-013, FR-025).
//!
//! The official Claude marketplace ships packages that carry no JavaScript entry
//! point — a `skills/` folder of `SKILL.md` files and/or an `mcpServers`
//! section. These must install, list, load, and pass `/plugins test` without
//! executing any entry point, and without being reported as a parse error.
//!
//! Every test is offline and rooted under `target/temp/` (no `/tmp`, per
//! AGENTS.md).

use std::path::{Path, PathBuf};

use ragent_plugins::{
    LifecycleState, PluginManager, StepOutcome, StoreDirs, add, scan_dirs, store_dirs_at,
    test_plugin,
};

static SEQ: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);

/// RAII scratch tree under `target/temp/`.
struct TempTree(PathBuf);

impl TempTree {
    fn new(name: &str) -> Self {
        let unique = SEQ.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(format!(
            "../../target/temp/plugins-test/non-js-{name}-{}-{unique}",
            std::process::id()
        ));
        std::fs::create_dir_all(&path).expect("temp tree creatable");
        Self(path)
    }

    /// The project store (`<tree>/.ragent/plugins/`).
    fn store(&self) -> PathBuf {
        self.0.join(".ragent/plugins")
    }

    fn dirs(&self) -> StoreDirs {
        store_dirs_at(&self.0, Some(&self.store()), None)
    }

    fn manager(&self) -> PluginManager {
        PluginManager::new(self.dirs(), ragent_config::PluginsConfig::default())
    }
}

impl Drop for TempTree {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// A Claude-dialect manifest with no JavaScript entry point, mirroring the
/// shape of the official `mongodb` marketplace plugin.
const SKILL_ONLY_MANIFEST: &str = r#"{
  "name": "mongodb",
  "displayName": "MongoDB (Self-Managed MCP)",
  "version": "1.2.1",
  "description": "Official Claude plugin for MongoDB (MCP Server + Skills).",
  "skills": "./skills/",
  "mcpServers": "./mcp.json"
}"#;

const MCP_JSON: &str = r#"{ "mcpServers": { "mongodb": { "command": "npx", "args": ["-y", "mongodb-mcp-server"] } } }"#;

/// Write the non-JS Claude plugin directory under `parent/mongodb` and return it.
fn stage_skill_only(parent: &Path) -> PathBuf {
    let plugin = parent.join("mongodb");
    std::fs::create_dir_all(plugin.join(".claude-plugin")).expect("manifest dir");
    std::fs::create_dir_all(plugin.join("skills/mongodb-setup")).expect("skills dir");
    std::fs::write(
        plugin.join(".claude-plugin/plugin.json"),
        SKILL_ONLY_MANIFEST,
    )
    .expect("manifest");
    std::fs::write(
        plugin.join("skills/mongodb-setup/SKILL.md"),
        "---\nname: mongodb-setup\ndescription: Set up MongoDB\n---\n\nUse the mongodb MCP server.\n",
    )
    .expect("skill");
    std::fs::write(plugin.join("mcp.json"), MCP_JSON).expect("mcp config");
    plugin
}

// ── Install (FR-007, FR-010) ────────────────────────────────────────────────

#[test]
fn add_installs_a_skill_only_claude_plugin() {
    let tree = TempTree::new("install");
    let source = stage_skill_only(&tree.0.join("src"));

    let outcome = add(&tree.dirs(), &tree.0, source.to_str().expect("utf8"), false)
        .expect("a manifest with no entry point installs");

    assert_eq!(outcome.parsed.descriptor.id, "mongodb");
    assert_eq!(
        outcome.parsed.descriptor.entry, None,
        "a non-JS plugin has no entry point"
    );
    // FR-028: the skills and MCP sections are bridged, so they are no longer
    // reported as unsupported capabilities.
    assert!(
        outcome
            .parsed
            .descriptor
            .unsupported_capabilities
            .is_empty(),
        "the bridged sections are not unsupported: {:?}",
        outcome.parsed.descriptor.unsupported_capabilities
    );
    assert_eq!(outcome.parsed.skills, vec!["./skills/".to_string()]);
    assert!(outcome.installed_dir.join("skills").is_dir());

    // It scans as a discovered, enabled plugin (FR-007): add records the flag.
    let found = scan_dirs(tree.dirs());
    assert_eq!(found.len(), 1);
    assert_eq!(
        found[0].outcome.as_ref().expect("parsed").descriptor.id,
        "mongodb"
    );
    assert!(found[0].enabled);
}

// ── Load (FR-008) ───────────────────────────────────────────────────────────

#[test]
fn enable_loads_a_skill_only_plugin_inertly() {
    let tree = TempTree::new("enable");
    stage_skill_only(&tree.store());

    let mut manager = tree.manager();
    let report = manager.enable("mongodb").expect("enable succeeds");

    assert_eq!(report.state, LifecycleState::Loaded);
    assert_eq!(report.error, None);
    assert!(report.tools.is_empty());
    assert!(report.commands.is_empty());

    // No JavaScript ran: there is no live sandbox context and no messages.
    let loaded = manager.get("mongodb").expect("tracked");
    assert!(loaded.context().is_none());
    assert!(loaded.calls.messages().is_empty());
}

// ── Harness (FR-013) ────────────────────────────────────────────────────────

#[test]
fn harness_passes_a_skill_only_plugin_without_executing_an_entry() {
    let tree = TempTree::new("harness");
    stage_skill_only(&tree.store());

    let report = test_plugin(
        tree.dirs(),
        &ragent_config::PluginsConfig::default(),
        "mongodb",
    );

    assert!(report.passed(), "no step should fail: {report:?}");
    let entry_step = report
        .steps
        .iter()
        .find(|s| s.name == "entry execution")
        .expect("the entry step is reported");
    assert_eq!(entry_step.outcome, StepOutcome::Pass);
    assert_eq!(
        entry_step.detail.as_deref(),
        Some("no entry point (non-JS plugin)")
    );
}
