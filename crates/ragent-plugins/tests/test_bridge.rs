//! Plugin bridge tests (spec `plugins` FR-029, FR-030): enabled plugins contribute
//! skill directories and MCP servers to the session.
//!
//! Every test is offline and rooted under `target/temp/` (no `/tmp`, per
//! AGENTS.md).

use std::path::{Path, PathBuf};

use ragent_config::{McpServerConfig, McpTransport};
use ragent_plugins::{
    StoreDirs, extract_mcp_servers, extract_skill_dirs, plugin_agent_files, plugin_mcp_servers,
    plugin_skill_dirs, plugin_skill_names, scanned_plugin_agent_files, scanned_plugin_hooks,
    scanned_plugin_mcp_servers, scanned_plugin_skill_dirs, store_dirs_at,
};

static SEQ: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);

/// RAII scratch tree under `target/temp/`.
struct TempTree(PathBuf);

impl TempTree {
    fn new(name: &str) -> Self {
        let unique = SEQ.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(format!(
            "../../target/temp/plugins-test/bridge-{name}-{}-{unique}",
            std::process::id()
        ));
        std::fs::create_dir_all(&path).expect("temp tree creatable");
        Self(path)
    }

    fn store(&self) -> PathBuf {
        self.0.join(".ragent/plugins")
    }

    fn dirs(&self) -> StoreDirs {
        store_dirs_at(&self.0, Some(&self.store()), None)
    }
}

impl Drop for TempTree {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// Stage a Claude-dialect skill+MCP plugin under `parent/mongodb`.
fn stage_plugin(parent: &Path) -> PathBuf {
    let plugin = parent.join("mongodb");
    std::fs::create_dir_all(plugin.join(".claude-plugin")).expect("manifest dir");
    std::fs::create_dir_all(plugin.join("skills/mongodb-setup")).expect("skills dir");
    std::fs::write(
        plugin.join(".claude-plugin/plugin.json"),
        r#"{
            "name": "mongodb",
            "version": "1.2.1",
            "skills": "./skills/",
            "mcpServers": "./mcp.json"
        }"#,
    )
    .expect("manifest");
    std::fs::write(
        plugin.join("skills/mongodb-setup/SKILL.md"),
        "---\nname: mongodb-setup\ndescription: Set up MongoDB\n---\n\nBody.\n",
    )
    .expect("skill");
    std::fs::write(
        plugin.join("mcp.json"),
        r#"{ "mcpServers": { "mongodb": { "command": "npx", "args": ["-y", "mongodb-mcp-server"] } } }"#,
    )
    .expect("mcp config");
    plugin
}

/// Enable a plugin discovered in `dirs` by writing the store ledger.
fn enable(dirs: &StoreDirs, plugin_id: &str) {
    let store = dirs.project.as_ref().expect("project store");
    let mut ledger = ragent_plugins::StoreLedger::load(store);
    ledger.state_mut(plugin_id).enabled = true;
    ledger.save(store).expect("ledger save");
}

// ── Manifest extraction helpers ─────────────────────────────────────────────

#[test]
fn extract_skill_dirs_accepts_string_and_array() {
    assert_eq!(
        extract_skill_dirs(Some(&serde_json::json!("./skills/"))),
        vec!["./skills/".to_string()]
    );
    assert_eq!(
        extract_skill_dirs(Some(&serde_json::json!(["./a", "./b"]))),
        vec!["./a".to_string(), "./b".to_string()]
    );
    assert!(extract_skill_dirs(Some(&serde_json::json!({ "dir": "x" }))).is_empty());
    assert!(extract_skill_dirs(None).is_empty());
}

#[test]
fn extract_mcp_servers_maps_object_entries_and_counts_unbridged() {
    let raw = serde_json::json!({
        "a": { "command": "npx", "args": ["-y", "pkg"], "env": { "K": "V" } },
        "b": 42
    });
    let (servers, unbridged) = extract_mcp_servers(Some(&raw));
    assert_eq!(unbridged, 1);
    assert_eq!(servers.len(), 1);
    assert_eq!(servers[0].id, "a");
    assert_eq!(servers[0].command.as_deref(), Some("npx"));
    assert_eq!(servers[0].args, vec!["-y".to_string(), "pkg".to_string()]);
    assert_eq!(servers[0].env.get("K").map(String::as_str), Some("V"));
}

// ── Path resolution ─────────────────────────────────────────────────────────

#[test]
fn plugin_skill_dirs_resolve_relative_and_refuse_escape() {
    let root = Path::new("/plugins/mongodb");
    let resolved = plugin_skill_dirs(
        root,
        &[
            "./skills/".to_string(),
            "../escape".to_string(),
            "/abs".to_string(),
        ],
    );
    assert_eq!(resolved, vec![PathBuf::from("/plugins/mongodb/skills")]);
}

#[test]
fn plugin_skill_names_lists_skill_subdirectories_only() {
    let tree = TempTree::new("skill-names");
    let plugin = stage_plugin(&tree.0.join("src"));
    // A directory with no `SKILL.md` contributes no skill name.
    std::fs::create_dir_all(plugin.join("skills/empty")).expect("empty skill dir");

    let names = plugin_skill_names(&plugin, &["./skills/".to_string()]);
    assert_eq!(names, vec!["mongodb-setup".to_string()]);

    // A missing directory (or no declaration) contributes nothing.
    assert!(plugin_skill_names(&plugin, &["./absent/".to_string()]).is_empty());
    assert!(plugin_skill_names(&plugin, &[]).is_empty());
}

// ── MCP mapping ─────────────────────────────────────────────────────────────

#[test]
fn plugin_mcp_servers_bridges_inline_object() {
    let root = Path::new("/plugins/x");
    let servers = vec![ragent_plugins::PluginMcpServer {
        id: "db".to_string(),
        command: Some("db-server".to_string()),
        args: vec!["--stdio".to_string()],
        env: std::collections::BTreeMap::new(),
        url: None,
        headers: std::collections::BTreeMap::new(),
        transport: "stdio".to_string(),
    }];
    let mapped = plugin_mcp_servers("x", root, &servers, None);
    assert_eq!(mapped.len(), 1);
    assert_eq!(mapped[0].0, "x.db");
    assert_eq!(mapped[0].1.command.as_deref(), Some("db-server"));
    assert_eq!(mapped[0].1.type_, McpTransport::Stdio);
}

#[test]
fn plugin_mcp_servers_reads_external_file() {
    let tree = TempTree::new("external");
    let plugin = stage_plugin(&tree.0.join("src"));

    // Parse the manifest to obtain the raw `mcpServers` string and root.
    let parsed = ragent_plugins::parse_plugin_dir(&plugin)
        .expect("readable")
        .expect("recognised");
    let mapped = plugin_mcp_servers(
        &parsed.descriptor.id,
        &parsed.descriptor.root,
        &parsed.mcp_servers,
        parsed.raw_mcp.as_ref(),
    );
    assert_eq!(mapped.len(), 1);
    assert_eq!(mapped[0].0, "mongodb.mongodb");
    assert_eq!(mapped[0].1.command.as_deref(), Some("npx"));
    assert_eq!(
        mapped[0].1.args,
        vec!["-y".to_string(), "mongodb-mcp-server".to_string()]
    );
    assert_eq!(mapped[0].1.type_, McpTransport::Stdio);
    assert_eq!(mapped[0].1.url, None);
}

// ── Store-level bridge (enabled filter) ─────────────────────────────────────

#[test]
fn scanned_plugin_skill_dirs_only_include_enabled_plugins() {
    let tree = TempTree::new("skills");
    let plugin = stage_plugin(&tree.store());

    // Disabled (not yet enabled): contributes nothing (FR-016).
    assert!(scanned_plugin_skill_dirs(&tree.dirs()).is_empty());

    enable(&tree.dirs(), "mongodb");
    let dirs = scanned_plugin_skill_dirs(&tree.dirs());
    assert_eq!(dirs, vec![plugin.join("skills")]);
}

#[test]
fn scanned_plugin_mcp_servers_prefix_ids_and_only_include_enabled() {
    let tree = TempTree::new("mcp");
    stage_plugin(&tree.store());

    assert!(scanned_plugin_mcp_servers(&tree.dirs()).is_empty());

    enable(&tree.dirs(), "mongodb");
    let servers = scanned_plugin_mcp_servers(&tree.dirs());
    assert_eq!(servers.len(), 1);
    assert_eq!(servers[0].0, "mongodb.mongodb");
    let expected = McpServerConfig {
        type_: McpTransport::Stdio,
        command: Some("npx".to_string()),
        args: vec!["-y".to_string(), "mongodb-mcp-server".to_string()],
        ..McpServerConfig::default()
    };
    assert_eq!(servers[0].1.command, expected.command);
    assert_eq!(servers[0].1.args, expected.args);
}

/// The `/plugins list` report groups MCP servers under the plugin that declares
/// them, so the attributed view must carry the owning plugin id even when the
/// bridged id alone would be ambiguous.
#[test]
fn scanned_plugin_mcp_contributions_attribute_each_server_to_its_plugin() {
    let tree = TempTree::new("mcp-contributions");
    stage_plugin(&tree.store());

    assert!(
        ragent_plugins::scanned_plugin_mcp_contributions(&tree.dirs()).is_empty(),
        "a disabled plugin contributes nothing"
    );

    enable(&tree.dirs(), "mongodb");
    let contributions = ragent_plugins::scanned_plugin_mcp_contributions(&tree.dirs());
    assert_eq!(contributions.len(), 1);
    assert_eq!(contributions[0].plugin_id, "mongodb");
    assert_eq!(contributions[0].server_id, "mongodb.mongodb");
    assert_eq!(contributions[0].config.command.as_deref(), Some("npx"));

    // The deduplicated view stays consistent with the attributed one.
    let deduped = scanned_plugin_mcp_servers(&tree.dirs());
    assert_eq!(deduped.len(), contributions.len());
    assert_eq!(deduped[0].0, contributions[0].server_id);
}

// ── FR-032: agents bridge ───────────────────────────────────────────────────

#[test]
fn plugin_agent_files_resolve_bare_names_and_refuse_escape() {
    let root = Path::new("/plugins/pack");
    let resolved = plugin_agent_files(
        root,
        &["reviewer".to_string(), "agents/audit.md".to_string()],
    );
    assert_eq!(
        resolved,
        vec![
            root.join("agents/reviewer.md"),
            root.join("agents/audit.md"),
        ]
    );
    // `..` escape and absolute paths are dropped.
    assert!(plugin_agent_files(root, &["../../etc/passwd.md".to_string()]).is_empty());
    assert!(plugin_agent_files(root, &["/abs.md".to_string()]).is_empty());
}

#[test]
fn scanned_plugin_agent_files_only_from_enabled_plugins() {
    let tree = TempTree::new("agents");
    let store = tree.store();
    std::fs::create_dir_all(store.join("pack/.claude-plugin")).expect("manifest dir");
    std::fs::create_dir_all(store.join("pack/agents")).expect("agent dir");
    std::fs::write(
        store.join("pack/.claude-plugin/plugin.json"),
        r#"{ "name": "pack" }"#,
    )
    .expect("manifest");
    std::fs::write(
        store.join("pack/agents/reviewer.md"),
        "---\nname: reviewer\n---\nBody.\n",
    )
    .expect("profile");

    let dirs = tree.dirs();
    assert!(
        scanned_plugin_agent_files(&dirs).is_empty(),
        "disabled plugins contribute nothing"
    );

    enable(&dirs, "pack");
    let files = scanned_plugin_agent_files(&dirs);
    assert_eq!(files.len(), 1);
    assert!(files[0].ends_with("pack/agents/reviewer.md"));
}

// ── FR-033: hooks bridge ────────────────────────────────────────────────────

#[test]
fn scanned_plugin_hooks_only_from_enabled_plugins() {
    let tree = TempTree::new("hooks");
    let store = tree.store();
    std::fs::create_dir_all(store.join("guard/.claude-plugin")).expect("manifest dir");
    std::fs::write(
        store.join("guard/.claude-plugin/plugin.json"),
        r#"{ "name": "guard", "hooks": { "PreToolUse": "./guard.sh" } }"#,
    )
    .expect("manifest");

    let dirs = tree.dirs();
    assert!(scanned_plugin_hooks(&dirs).is_empty(), "disabled -> inert");

    enable(&dirs, "guard");
    let hooks = scanned_plugin_hooks(&dirs);
    assert_eq!(hooks.len(), 1);
    assert_eq!(hooks[0].plugin_id, "guard");
    assert_eq!(hooks[0].trigger, "PreToolUse");
    assert_eq!(hooks[0].command, "./guard.sh");
}

#[test]
fn scanned_plugin_hooks_reads_hooks_json_file() {
    let tree = TempTree::new("hooks-file");
    let store = tree.store();
    let plugin = store.join("sg");
    std::fs::create_dir_all(plugin.join(".claude-plugin")).expect("manifest dir");
    std::fs::create_dir_all(plugin.join("hooks")).expect("hooks dir");
    std::fs::write(
        plugin.join(".claude-plugin/plugin.json"),
        r#"{ "name": "sg", "version": "2.0.8" }"#,
    )
    .expect("manifest");
    std::fs::write(
        plugin.join("hooks/hooks.json"),
        r#"{
            "hooks": {
                "SessionStart": [
                    { "hooks": [
                        { "type": "command", "command": "bash \"${CLAUDE_PLUGIN_ROOT}/hooks/boot.sh\"", "timeout": 180 }
                    ] }
                ],
                "PostToolUse": [
                    {
                        "hooks": [
                            { "type": "command", "command": "bash \"${CLAUDE_PLUGIN_ROOT}/hooks/check.py\"", "timeout": 30 }
                        ],
                        "matcher": "Edit|Write|MultiEdit"
                    }
                ]
            }
        }"#,
    )
    .expect("hooks file");

    let dirs = tree.dirs();
    assert!(scanned_plugin_hooks(&dirs).is_empty(), "disabled -> inert");

    enable(&dirs, "sg");
    let hooks = scanned_plugin_hooks(&dirs);
    assert_eq!(
        hooks.len(),
        2,
        "hooks/hooks.json bridged for enabled plugin"
    );
    let post = hooks
        .iter()
        .find(|h| h.trigger == "PostToolUse")
        .expect("PostToolUse hook");
    assert_eq!(post.matcher.as_deref(), Some("Edit|Write|MultiEdit"));
    assert_eq!(post.timeout_secs, Some(30));
    assert_eq!(post.plugin_root, plugin);
    let start = hooks
        .iter()
        .find(|h| h.trigger == "SessionStart")
        .expect("SessionStart hook");
    assert_eq!(start.timeout_secs, Some(180));
}
