//! Agent-side plugin bridge tests (spec `plugins` FR-029, FR-030): the skills registry
//! picks up enabled-plugin skill directories, and the MCP bridge merges plugin
//! servers with the configured set.
//!
//! Every test is offline and rooted under `target/temp/` (no `/tmp`).

use std::collections::{BTreeSet, HashMap};
use std::path::{Path, PathBuf};

use ragent_agent::plugin::plugin_mcp_servers;
use ragent_agent::skill::effective_skill_dirs;
use ragent_config::{McpServerConfig, McpTransport};

static SEQ: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);

struct TempTree(PathBuf);

impl TempTree {
    fn new(name: &str) -> Self {
        let unique = SEQ.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(format!(
            "../../target/temp/plugins-test/agent-bridge-{name}-{}-{unique}",
            std::process::id()
        ));
        std::fs::create_dir_all(&path).expect("temp tree creatable");
        Self(path)
    }

    fn store(&self) -> PathBuf {
        self.0.join(".ragent/plugins")
    }
}

impl Drop for TempTree {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// Stage a skill+MCP plugin under `<tree>/.ragent/plugins/mongodb`.
fn stage_plugin(tree: &TempTree) -> PathBuf {
    let plugin = tree.store().join("mongodb");
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

fn enable(tree: &TempTree, plugin_id: &str) {
    let store = tree.store();
    let mut ledger = ragent_plugins::StoreLedger::load(&store);
    ledger.state_mut(plugin_id).enabled = true;
    ledger.save(&store).expect("ledger save");
}

#[test]
fn effective_skill_dirs_appends_enabled_plugin_skill_dirs() {
    let tree = TempTree::new("skills");
    let plugin = stage_plugin(&tree);

    let configured = vec!["/configured/skills".to_string()];
    // Disabled: no plugin dirs contribute, but config dirs are preserved.
    let before = effective_skill_dirs(&tree.0, &configured);
    assert_eq!(before, configured);

    enable(&tree, "mongodb");
    let after = effective_skill_dirs(&tree.0, &configured);
    assert_eq!(after[0], "/configured/skills");
    assert!(
        after.contains(&plugin.join("skills").to_string_lossy().into_owned()),
        "plugin skills dir appended: {after:?}"
    );
}

#[test]
fn plugin_mcp_servers_merges_and_config_wins() {
    let tree = TempTree::new("mcp");
    stage_plugin(&tree);
    enable(&tree, "mongodb");

    // A configured server whose id matches the bridged plugin id must win.
    let mut configured: HashMap<String, McpServerConfig> = HashMap::new();
    configured.insert(
        "mongodb.mongodb".to_string(),
        McpServerConfig {
            type_: McpTransport::Stdio,
            command: Some("configured-runner".to_string()),
            ..McpServerConfig::default()
        },
    );
    configured.insert(
        "plain".to_string(),
        McpServerConfig {
            type_: McpTransport::Stdio,
            command: Some("plain-runner".to_string()),
            ..McpServerConfig::default()
        },
    );

    let merged = plugin_mcp_servers(&tree.0, &configured);
    let by_id: HashMap<&str, &McpServerConfig> =
        merged.iter().map(|(id, c)| (id.as_str(), c)).collect();
    assert_eq!(merged.len(), 2, "config wins, no duplicate: {merged:?}");
    assert_eq!(
        by_id["mongodb.mongodb"].command.as_deref(),
        Some("configured-runner")
    );
    assert_eq!(by_id["plain"].command.as_deref(), Some("plain-runner"));
}

#[test]
fn plugin_mcp_servers_contributes_when_not_configured() {
    let tree = TempTree::new("mcp-add");
    stage_plugin(&tree);
    enable(&tree, "mongodb");

    let merged = plugin_mcp_servers(&tree.0, &HashMap::new());
    let entry = merged
        .iter()
        .find(|(id, _)| id == "mongodb.mongodb")
        .expect("plugin server bridged");
    assert_eq!(entry.1.command.as_deref(), Some("npx"));
    assert_eq!(
        entry.1.args,
        vec!["-y".to_string(), "mongodb-mcp-server".to_string()]
    );
    // The declared transport is kept verbatim; an entry with no `type` stays on
    // the default stdio transport.
    assert_eq!(entry.1.type_, McpTransport::Stdio);
    assert_eq!(entry.1.url, None);
}

#[test]
fn skill_registry_load_includes_plugin_skills() {
    use ragent_agent::skill::SkillRegistry;

    let tree = TempTree::new("registry");
    stage_plugin(&tree);
    enable(&tree, "mongodb");

    let registry = SkillRegistry::load(Path::new(&tree.0), &[]);
    assert!(
        registry.get("mongodb-setup").is_some(),
        "plugin skill discovered via SkillRegistry::load: {:?}",
        registry
            .list_all()
            .iter()
            .map(|s| &s.name)
            .collect::<Vec<_>>()
    );
}

// ── FR-031: plugin-contributed slash commands ────────────────────────────────

/// Stage a Claude plugin whose commands live in the `commands/` directory.
fn stage_command_plugin(tree: &TempTree) -> PathBuf {
    let plugin = tree.store().join("commit-commands");
    std::fs::create_dir_all(plugin.join(".claude-plugin")).expect("manifest dir");
    std::fs::create_dir_all(plugin.join("commands")).expect("commands dir");
    std::fs::write(
        plugin.join(".claude-plugin/plugin.json"),
        r#"{ "name": "commit-commands", "version": "1.0.0" }"#,
    )
    .expect("manifest");
    std::fs::write(
        plugin.join("commands/commit.md"),
        "---\ndescription: Create a git commit\n---\n\nCommit: $ARGUMENTS\n",
    )
    .expect("commit command");
    std::fs::write(
        plugin.join("commands/clean_gone.md"),
        "---\ndescription: Clean up gone branches\n---\n\nClean up.\n",
    )
    .expect("clean command");
    plugin
}

#[test]
fn plugin_commands_discovers_prompt_commands_from_the_directory() {
    let tree = TempTree::new("cmd-dir");
    stage_command_plugin(&tree);
    enable(&tree, "commit-commands");

    let commands = ragent_agent::plugin::plugin_commands(&tree.0, &BTreeSet::new());
    let names: Vec<&str> = commands.iter().map(|c| c.name.as_str()).collect();
    assert_eq!(names, vec!["clean_gone", "commit"]);

    let commit = commands.iter().find(|c| c.name == "commit").unwrap();
    assert_eq!(commit.trigger, "commit");
    assert_eq!(commit.description, "Create a git commit");
    assert!(
        commit
            .prompt
            .as_deref()
            .unwrap()
            .contains("Commit: $ARGUMENTS")
    );
    assert_eq!(commit.plugin_id, "commit-commands");
}

#[test]
fn plugin_commands_are_inert_until_the_plugin_is_enabled() {
    let tree = TempTree::new("cmd-disabled");
    stage_command_plugin(&tree);
    // Not enabled: a disabled plugin contributes nothing (FR-016).
    let commands = ragent_agent::plugin::plugin_commands(&tree.0, &BTreeSet::new());
    assert!(commands.is_empty(), "disabled plugin contributes nothing");
}

#[test]
fn plugin_command_colliding_with_a_builtin_is_namespaced() {
    let tree = TempTree::new("cmd-collide");
    stage_command_plugin(&tree);
    enable(&tree, "commit-commands");

    let mut existing = BTreeSet::new();
    existing.insert("commit".to_string());

    let commands = ragent_agent::plugin::plugin_commands(&tree.0, &existing);
    let commit = commands.iter().find(|c| c.name == "commit").unwrap();
    assert_eq!(commit.trigger, "plugin:commit-commands:commit");
    let clean = commands.iter().find(|c| c.name == "clean_gone").unwrap();
    assert_eq!(clean.trigger, "clean_gone");
}

// ── FR-032: plugin-contributed agent profiles ────────────────────────────────

/// Stage a plugin contributing a markdown agent profile under `agents/`.
fn stage_agent_plugin(tree: &TempTree) -> PathBuf {
    let plugin = tree.store().join("review-pack");
    std::fs::create_dir_all(plugin.join(".claude-plugin")).expect("manifest dir");
    std::fs::create_dir_all(plugin.join("agents")).expect("agents dir");
    std::fs::write(
        plugin.join(".claude-plugin/plugin.json"),
        r#"{ "name": "review-pack", "version": "1.0.0" }"#,
    )
    .expect("manifest");
    // A Claude-dialect profile: YAML frontmatter, markdown body.
    std::fs::write(
        plugin.join("agents/security-reviewer.md"),
        "---\nname: security-reviewer\ndescription: Reviews for security issues\nmode: subagent\n---\n\nYou review code for security issues.\n",
    )
    .expect("profile");
    plugin
}

#[test]
fn plugin_agent_profile_loads_with_yaml_frontmatter() {
    let tree = TempTree::new("agents");
    stage_agent_plugin(&tree);

    // Disabled: nothing contributes.
    let before = ragent_agent::agent::custom::load_custom_agents(&tree.0);
    assert!(
        before
            .0
            .iter()
            .all(|d| d.agent_info.name != "security-reviewer"),
        "disabled plugin contributes no agent"
    );

    enable(&tree, "review-pack");
    let (agents, diagnostics) = ragent_agent::agent::custom::load_custom_agents(&tree.0);
    let agent = agents
        .iter()
        .find(|d| d.agent_info.name == "security-reviewer")
        .unwrap_or_else(|| panic!("plugin agent loaded; diagnostics: {diagnostics:?}"));
    assert!(
        agent
            .agent_info
            .prompt
            .as_deref()
            .is_some_and(|p| p.contains("security issues")),
        "YAML profile body became the system prompt"
    );
}
