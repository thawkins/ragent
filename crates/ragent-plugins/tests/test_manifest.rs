//! Tests for manifest parsing, normalisation, and the host-API version check
//! (spec `plugins` T-003; FR-002, FR-019, FR-025).

use std::path::{Path, PathBuf};

use ragent_plugins::{
    HOST_API_VERSION, PluginDialect, PluginError, UNSUP_DESKTOP_MOUNTS, UNSUP_DESKTOP_WINDOW,
    UNSUP_EXEC, UNSUP_FS, UNSUP_MCP, UNSUP_SKILLS, check_api_version, derive_id,
    parse_claude_manifest, parse_codex_manifest, parse_manifest, parse_plugin_dir,
};

fn root() -> PathBuf {
    PathBuf::from("/plugins/codex-weather")
}

fn codex_rel() -> &'static Path {
    Path::new("codex-plugin.json")
}

// ── Codex manifest parsing (FR-002) ─────────────────────────────────────────

#[test]
fn parses_minimal_codex_manifest() {
    let parsed = parse_codex_manifest(
        &root(),
        codex_rel(),
        br#"{
            "name": "Weather",
            "version": "1.2.0",
            "entry": "index.js"
        }"#,
    )
    .expect("should parse");

    let d = &parsed.descriptor;
    assert_eq!(d.id, "weather");
    assert_eq!(d.name, "Weather");
    assert_eq!(d.version, "1.2.0");
    assert_eq!(d.dialect, PluginDialect::Codex);
    assert_eq!(d.entry, Some(root().join("index.js")));
    assert!(d.requested_permissions.is_empty());
    assert_eq!(d.api_version, HOST_API_VERSION);
    assert!(d.unsupported_capabilities.is_empty());
    assert_eq!(d.manifest_path, root().join("codex-plugin.json"));
    assert_eq!(d.root, root());
    assert!(parsed.tools.is_empty());
    assert!(parsed.commands.is_empty());
}

#[test]
fn parses_full_codex_manifest_with_tools_commands_permissions() {
    let parsed = parse_codex_manifest(
        &root(),
        codex_rel(),
        br#"{
            "name": "Weather",
            "version": "1.2.0",
            "id": "codex-weather",
            "entry": "index.js",
            "api_version": 1,
            "tools": [
                {
                    "name": "get_weather",
                    "description": "Get weather for a city",
                    "parameters": {
                        "type": "object",
                        "properties": { "city": { "type": "string" } },
                        "required": ["city"]
                    }
                }
            ],
            "commands": [
                { "name": "weather", "description": "Weather lookup", "usage": "/weather <city>" }
            ],
            "permissions": { "network": ["outbound", "dns"] },
            "unknown_future_field": { "anything": true }
        }"#,
    )
    .expect("should parse; unknown fields tolerated");

    let d = &parsed.descriptor;
    assert_eq!(d.id, "codex-weather");
    assert_eq!(
        d.requested_permissions,
        vec!["network.outbound", "network.dns"]
    );
    assert_eq!(d.api_version, 1);

    assert_eq!(parsed.tools.len(), 1);
    assert_eq!(parsed.tools[0].name, "get_weather");
    assert_eq!(parsed.tools[0].description, "Get weather for a city");
    assert_eq!(
        parsed.tools[0].parameters["properties"]["city"]["type"],
        "string"
    );

    assert_eq!(parsed.commands.len(), 1);
    assert_eq!(parsed.commands[0].decl.name, "weather");
    assert_eq!(
        parsed.commands[0].decl.usage.as_deref(),
        Some("/weather <city>")
    );
}

#[test]
fn codex_fs_and_exec_permissions_are_recorded_unsupported() {
    let parsed = parse_codex_manifest(
        &root(),
        codex_rel(),
        br#"{
            "name": "w", "version": "1", "entry": "i.js",
            "permissions": { "fs": ["read:/"], "exec": ["sh"] }
        }"#,
    )
    .expect("should parse");

    assert_eq!(
        parsed.descriptor.unsupported_capabilities,
        vec![UNSUP_FS.to_string(), UNSUP_EXEC.to_string()]
    );
    assert!(parsed.descriptor.requested_permissions.is_empty());
}

#[test]
fn codex_manifest_with_mcp_servers_section_is_bridged() {
    // FR-028: an inline `mcpServers` object is bridged to a server list, so it
    // is no longer reported as an unsupported capability.
    let parsed = parse_codex_manifest(
        &root(),
        codex_rel(),
        br#"{
            "name": "w", "version": "1", "entry": "i.js",
            "mcpServers": { "fetch": { "command": "npx" } }
        }"#,
    )
    .expect("should parse");
    assert!(parsed.descriptor.unsupported_capabilities.is_empty());
    assert_eq!(parsed.mcp_servers.len(), 1);
    assert_eq!(parsed.mcp_servers[0].id, "fetch");
    assert_eq!(parsed.mcp_servers[0].command.as_deref(), Some("npx"));
}

#[test]
fn codex_manifest_unbridgeable_mcp_section_stays_unsupported() {
    // A present-but-unbridgeable MCP shape (here an array) keeps the FR-025
    // label rather than being silently dropped.
    let parsed = parse_codex_manifest(
        &root(),
        codex_rel(),
        br#"{
            "name": "w", "version": "1", "entry": "i.js",
            "mcpServers": [ "not", "an", "object" ]
        }"#,
    )
    .expect("should parse");
    assert_eq!(
        parsed.descriptor.unsupported_capabilities,
        vec![UNSUP_MCP.to_string()]
    );
}

#[test]
fn codex_manifest_rejects_invalid_json_with_position() {
    let err = parse_codex_manifest(&root(), codex_rel(), b"{\"name\": \"w\", ").unwrap_err();
    let PluginError::ManifestParse { manifest, detail } = err else {
        panic!("expected ManifestParse, got {err:?}");
    };
    assert_eq!(manifest, root().join("codex-plugin.json"));
    assert!(
        detail.contains("line") && detail.contains("column"),
        "parse detail should carry the JSON error position, got: {detail}"
    );
}

#[test]
fn codex_manifest_rejects_empty_entry() {
    let err = parse_codex_manifest(
        &root(),
        codex_rel(),
        br#"{"name": "w", "version": "1", "entry": "  "}"#,
    )
    .unwrap_err();
    let PluginError::ManifestParse { detail, .. } = err else {
        panic!("expected ManifestParse, got {err:?}");
    };
    assert!(
        detail.contains("entry"),
        "detail should name the entry field"
    );
}

// ── Claude manifest parsing (FR-002) ────────────────────────────────────────

#[test]
fn parses_minimal_claude_manifest() {
    let parsed = parse_claude_manifest(
        &root(),
        Path::new(".claude-plugin/plugin.json"),
        br#"{ "name": "todo-tools", "main": "index.js" }"#,
    )
    .expect("should parse");

    let d = &parsed.descriptor;
    assert_eq!(d.id, "todo-tools");
    assert_eq!(d.version, "0.0.0");
    assert_eq!(d.dialect, PluginDialect::Claude);
    assert_eq!(d.entry, Some(root().join("index.js")));
    assert_eq!(d.api_version, HOST_API_VERSION);
    assert!(d.unsupported_capabilities.is_empty());
}

#[test]
fn parses_claude_manifest_with_tools_commands_permissions() {
    let parsed = parse_claude_manifest(
        &root(),
        Path::new("claude-plugin.json"),
        br#"{
            "name": "Claude Todo",
            "version": "2.0",
            "id": "claude-todo",
            "entry": "index.js",
            "tools": [ { "name": "add_todo", "description": "Add a todo item" } ],
            "commands": [ { "name": "todo-add", "description": "Add todo" } ],
            "permissions": ["filesystem.read_workspace"]
        }"#,
    )
    .expect("should parse");

    let d = &parsed.descriptor;
    assert_eq!(d.id, "claude-todo");
    assert_eq!(d.version, "2.0");
    assert_eq!(d.requested_permissions, vec!["filesystem.read_workspace"]);
    assert_eq!(parsed.tools.len(), 1);
    assert_eq!(parsed.tools[0].parameters, serde_json::Value::Null);
    assert_eq!(parsed.commands.len(), 1);
    assert_eq!(parsed.commands[0].decl.usage, None);
}

#[test]
fn claude_desktop_server_entry_is_resolved() {
    let parsed = parse_claude_manifest(
        &root(),
        Path::new(".claude-plugin/plugin.json"),
        br#"{ "name": "ext", "server": { "entry": "dist/server.js" } }"#,
    )
    .expect("should parse");
    assert_eq!(parsed.descriptor.entry, Some(root().join("dist/server.js")));
}

#[test]
fn claude_mcp_section_is_bridged_never_dropped() {
    // FR-028: a Claude `mcp_servers` object is bridged, so the section is not
    // reported as unsupported.
    let parsed = parse_claude_manifest(
        &root(),
        Path::new(".claude-plugin/plugin.json"),
        br#"{
            "name": "weather-mcp",
            "entry": "index.js",
            "mcp_servers": { "weather": { "transport": "stdio", "command": "node" } }
        }"#,
    )
    .expect("should parse");
    assert!(parsed.descriptor.unsupported_capabilities.is_empty());
    assert_eq!(parsed.mcp_servers.len(), 1);
    assert_eq!(parsed.mcp_servers[0].id, "weather");
    assert_eq!(parsed.mcp_servers[0].command.as_deref(), Some("node"));
    assert_eq!(parsed.mcp_servers[0].transport, "stdio");
}

#[test]
fn claude_external_mcp_file_is_bridged() {
    // The Claude marketplace `"mcpServers": "./mcp.json"` shape: the string is
    // carried through on `raw_mcp` for the bridge to resolve, and is not
    // reported unsupported.
    let parsed = parse_claude_manifest(
        &root(),
        Path::new(".claude-plugin/plugin.json"),
        br#"{
            "name": "mongodb",
            "skills": "./skills/",
            "mcpServers": "./mcp.json"
        }"#,
    )
    .expect("should parse");
    assert!(parsed.descriptor.unsupported_capabilities.is_empty());
    assert!(parsed.mcp_servers.is_empty());
    assert_eq!(
        parsed.raw_mcp,
        Some(serde_json::Value::String("./mcp.json".to_string()))
    );
    assert_eq!(parsed.skills, vec!["./skills/".to_string()]);
}

#[test]
fn claude_unbridgeable_skills_section_is_recorded_unsupported() {
    let parsed = parse_claude_manifest(
        &root(),
        Path::new(".claude-plugin/plugin.json"),
        br#"{ "name": "bad-skills", "skills": { "dir": "./skills" } }"#,
    )
    .expect("should parse");
    assert_eq!(
        parsed.descriptor.unsupported_capabilities,
        vec![UNSUP_SKILLS.to_string()]
    );
}

#[test]
fn claude_desktop_sections_are_recorded_unsupported() {
    let parsed = parse_claude_manifest(
        &root(),
        Path::new("claude-plugin.json"),
        br#"{
            "name": "desktop-ext", "entry": "main.js",
            "mounts": ["/home/user/data"],
            "window": { "width": 800 },
            "capabilities": ["tools", "screen_capture", "log"]
        }"#,
    )
    .expect("should parse");
    assert_eq!(
        parsed.descriptor.unsupported_capabilities,
        vec![
            UNSUP_DESKTOP_MOUNTS.to_string(),
            UNSUP_DESKTOP_WINDOW.to_string(),
            "screen_capture".to_string(),
        ]
    );
}

#[test]
fn claude_manifest_without_entry_is_a_non_js_plugin() {
    // A manifest with no entry/main/server.entry is a skill-only or MCP-only
    // plugin: it parses with `entry = None` and contributes nothing executable.
    let parsed = parse_claude_manifest(
        &root(),
        Path::new(".claude-plugin/plugin.json"),
        br#"{
            "name": "mongodb",
            "version": "1.2.1",
            "skills": "./skills/",
            "mcpServers": "./mcp.json"
        }"#,
    )
    .expect("a manifest with no entry point is accepted as a non-JS plugin");
    assert_eq!(parsed.descriptor.entry, None);
    assert_eq!(parsed.descriptor.id, "mongodb");
    // FR-028: the skills and MCP sections are bridged, so nothing is unsupported.
    assert!(parsed.descriptor.unsupported_capabilities.is_empty());
    assert_eq!(parsed.skills, vec!["./skills/".to_string()]);
}

#[test]
fn codex_manifest_without_entry_is_a_non_js_plugin() {
    let parsed = parse_codex_manifest(
        &root(),
        codex_rel(),
        br#"{ "name": "skill-only", "version": "1", "skills": "./skills" }"#,
    )
    .expect("a Codex manifest with no entry is a non-JS plugin");
    assert_eq!(parsed.descriptor.entry, None);
}

#[test]
fn manifest_with_blank_entry_still_errors() {
    // An explicitly declared but blank entry is a malformed manifest, not a
    // non-JS plugin: the author intended an entry point and got it wrong.
    let err = parse_claude_manifest(
        &root(),
        Path::new(".claude-plugin/plugin.json"),
        br#"{ "name": "blank", "entry": "   " }"#,
    )
    .unwrap_err();
    let PluginError::ManifestParse { detail, .. } = err else {
        panic!("expected ManifestParse, got {err:?}");
    };
    assert!(
        detail.contains("empty"),
        "detail should explain the empty entry, got: {detail}"
    );
}

#[test]
fn claude_manifest_rejects_invalid_json_with_position() {
    let err =
        parse_claude_manifest(&root(), Path::new("claude-plugin.json"), b"not json").unwrap_err();
    let PluginError::ManifestParse { detail, .. } = err else {
        panic!("expected ManifestParse, got {err:?}");
    };
    assert!(detail.contains("line") && detail.contains("column"));
}

// ── Dispatch and filesystem parsing ─────────────────────────────────────────

#[test]
fn parse_manifest_dispatches_on_dialect() {
    let matched = ragent_plugins::DialectMatch {
        dialect: PluginDialect::Codex,
        manifest_rel: PathBuf::from("plugin.json"),
    };
    let parsed = parse_manifest(
        &root(),
        &matched,
        br#"{"codex": true, "name": "w", "version": "1", "entry": "i.js"}"#,
    )
    .expect("should parse");
    assert_eq!(parsed.descriptor.dialect, PluginDialect::Codex);
    assert_eq!(parsed.descriptor.manifest_path, root().join("plugin.json"));
}

#[test]
fn parse_plugin_dir_end_to_end_with_nested_claude_manifest() {
    static SEQ: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
    let unique = SEQ.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(format!(
        "../../target/temp/plugins-test/manifest-e2e-{}-{unique}",
        std::process::id()
    ));
    std::fs::create_dir_all(dir.join(".claude-plugin")).expect("fixture creatable");
    std::fs::write(
        dir.join(".claude-plugin/plugin.json"),
        r#"{ "name": "Todo", "version": "3.1", "main": "index.js" }"#,
    )
    .expect("fixture writable");
    std::fs::write(dir.join("index.js"), "// entry").expect("fixture writable");

    let parsed = parse_plugin_dir(&dir)
        .expect("no error")
        .expect("should be a plugin");
    assert_eq!(parsed.descriptor.dialect, PluginDialect::Claude);
    assert_eq!(parsed.descriptor.id, "todo");
    assert_eq!(parsed.descriptor.version, "3.1");
    assert_eq!(parsed.descriptor.entry, Some(dir.join("index.js")));
    assert_eq!(
        parsed.descriptor.manifest_path,
        dir.join(".claude-plugin/plugin.json")
    );

    std::fs::remove_dir_all(&dir).expect("cleanup");
}

#[test]
fn parse_plugin_dir_returns_none_for_non_plugin_directory() {
    static SEQ: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(100);
    let unique = SEQ.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(format!(
        "../../target/temp/plugins-test/manifest-none-{}-{unique}",
        std::process::id()
    ));
    std::fs::create_dir_all(&dir).expect("fixture creatable");
    std::fs::write(dir.join("README.md"), "hi").expect("fixture writable");

    assert_eq!(parse_plugin_dir(&dir).expect("no error"), None);
    std::fs::remove_dir_all(&dir).expect("cleanup");
}

// ── Host-API version check (FR-019) ─────────────────────────────────────────

#[test]
fn api_version_check_accepts_equal_and_lower() {
    check_api_version(1, HOST_API_VERSION).expect("v1 plugin on v1 host");
    check_api_version(0, HOST_API_VERSION).expect("v0 plugin on v1 host");
}

#[test]
fn api_version_check_refuses_newer() {
    let err = check_api_version(99, HOST_API_VERSION).unwrap_err();
    assert_eq!(err.declared, 99);
    assert_eq!(err.host, HOST_API_VERSION);
    assert!(
        err.to_string().contains("v99") && err.to_string().contains("v1"),
        "mismatch message should name both versions, got: {err}"
    );
}

// ── Id derivation ───────────────────────────────────────────────────────────

#[test]
fn derive_id_normalises_display_names() {
    assert_eq!(derive_id("Weather Tools"), "weather-tools");
    assert_eq!(derive_id("claude_todo!"), "claude-todo");
    assert_eq!(derive_id("  spaced  out  name "), "spaced-out-name");
    assert_eq!(derive_id("!!!"), "unnamed-plugin");
    assert_eq!(derive_id(""), "unnamed-plugin");
}

// ── ParsedManifest round-trip ───────────────────────────────────────────────

#[test]
fn parsed_manifest_tool_decls_serialise_for_downstream_tasks() {
    let parsed = parse_codex_manifest(
        &root(),
        codex_rel(),
        br#"{
            "name": "w", "version": "1", "entry": "i.js",
            "tools": [ { "name": "t", "description": "d", "parameters": { "type": "object" } } ]
        }"#,
    )
    .expect("should parse");
    let json = serde_json::to_string(&parsed.tools[0]).expect("decl serialises");
    assert!(json.contains("\"name\":\"t\""));
    assert!(json.contains("\"parameters\":{\"type\":\"object\"}"));
}

// ── FR-031: prompt commands from the `commands/` directory ───────────────────

/// A temp plugin tree under `target/temp/` (no `/tmp`).
struct TempDir(PathBuf);

impl TempDir {
    fn new(name: &str) -> Self {
        let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(format!(
            "../../target/temp/manifest-cmd-{name}-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&path);
        std::fs::create_dir_all(&path).expect("temp dir creatable");
        Self(path)
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

#[test]
fn claude_commands_directory_yields_prompt_commands() {
    let dir = TempDir::new("dir");
    std::fs::create_dir_all(dir.0.join("commands")).expect("commands dir");
    std::fs::write(
        dir.0.join("commands/commit.md"),
        "---\ndescription: Create a git commit\nallowed-tools: Bash(git add:*)\n---\n\nCommit body.\n",
    )
    .expect("command file");
    std::fs::write(dir.0.join("commands/plain.md"), "No frontmatter body.\n").expect("plain file");
    std::fs::create_dir_all(dir.0.join(".claude-plugin")).expect("manifest dir");
    std::fs::write(
        dir.0.join(".claude-plugin/plugin.json"),
        r#"{ "name": "commit-commands" }"#,
    )
    .expect("manifest");

    let parsed = parse_plugin_dir(&dir.0)
        .expect("parses")
        .expect("recognised");
    assert_eq!(parsed.commands.len(), 2);

    let commit = parsed
        .commands
        .iter()
        .find(|c| c.decl.name == "commit")
        .expect("commit discovered");
    assert_eq!(commit.decl.description, "Create a git commit");
    // The body is everything after the closing delimiter; the blank line that
    // separated it from the frontmatter is preserved.
    assert_eq!(commit.prompt(), Some("\nCommit body.\n"));
    // Unknown frontmatter keys are ignored.
    assert_eq!(commit.decl.usage, None);

    let plain = parsed
        .commands
        .iter()
        .find(|c| c.decl.name == "plain")
        .expect("plain discovered");
    assert_eq!(plain.decl.description, "");
    assert_eq!(plain.prompt(), Some("No frontmatter body.\n"));
}

#[test]
fn claude_manifest_commands_array_with_file_is_a_prompt_command() {
    let dir = TempDir::new("decl-file");
    std::fs::create_dir_all(dir.0.join("commands")).expect("commands dir");
    std::fs::write(dir.0.join("commands/deploy.md"), "Deploy now.\n").expect("command file");
    std::fs::create_dir_all(dir.0.join(".claude-plugin")).expect("manifest dir");
    std::fs::write(
        dir.0.join(".claude-plugin/plugin.json"),
        r#"{ "name": "deployer", "commands": [ { "name": "deploy", "file": "deploy.md" } ] }"#,
    )
    .expect("manifest");

    let parsed = parse_plugin_dir(&dir.0)
        .expect("parses")
        .expect("recognised");
    assert_eq!(parsed.commands.len(), 1);
    assert_eq!(parsed.commands[0].decl.name, "deploy");
    assert_eq!(parsed.commands[0].prompt(), Some("Deploy now.\n"));
}

#[test]
fn manifest_commands_array_object_without_file_stays_inline() {
    let dir = TempDir::new("inline");
    std::fs::create_dir_all(dir.0.join(".claude-plugin")).expect("manifest dir");
    std::fs::write(
        dir.0.join(".claude-plugin/plugin.json"),
        r#"{ "name": "inline-plugin", "commands": [ { "name": "go", "description": "run it" } ] }"#,
    )
    .expect("manifest");

    let parsed = parse_plugin_dir(&dir.0)
        .expect("parses")
        .expect("recognised");
    assert_eq!(parsed.commands.len(), 1);
    assert_eq!(parsed.commands[0].decl.name, "go");
    assert!(parsed.commands[0].prompt().is_none(), "inline command");
}

// ── FR-032: agent profiles (`agents/` directory + `agents` section) ──────────

#[test]
fn claude_agents_directory_yields_profiles() {
    let dir = TempDir::new("agents-dir");
    std::fs::create_dir_all(dir.0.join("agents")).expect("agents dir");
    std::fs::write(
        dir.0.join("agents/reviewer.md"),
        "---\nname: reviewer\n---\nBody.\n",
    )
    .expect("profile");
    std::fs::create_dir_all(dir.0.join(".claude-plugin")).expect("manifest dir");
    std::fs::write(
        dir.0.join(".claude-plugin/plugin.json"),
        r#"{ "name": "reviewer-pack" }"#,
    )
    .expect("manifest");

    let parsed = parse_plugin_dir(&dir.0)
        .expect("parses")
        .expect("recognised");
    assert_eq!(parsed.agents, vec!["agents/reviewer.md".to_string()]);
    assert!(
        !parsed
            .descriptor
            .unsupported_capabilities
            .contains(&"plugin agents".to_string()),
        "bridged agents are not reported unsupported"
    );
}

#[test]
fn manifest_agents_array_merges_with_directory_scan() {
    let dir = TempDir::new("agents-merge");
    std::fs::create_dir_all(dir.0.join("agents")).expect("agents dir");
    std::fs::write(dir.0.join("agents/one.md"), "---\nname: one\n---\nBody.\n").expect("profile");
    std::fs::create_dir_all(dir.0.join(".claude-plugin")).expect("manifest dir");
    std::fs::write(
        dir.0.join(".claude-plugin/plugin.json"),
        r#"{ "name": "pack", "agents": ["agents/two.md"] }"#,
    )
    .expect("manifest");

    let parsed = parse_plugin_dir(&dir.0)
        .expect("parses")
        .expect("recognised");
    assert_eq!(
        parsed.agents,
        vec!["agents/two.md".to_string(), "agents/one.md".to_string()]
    );
}

#[test]
fn manifest_agents_wrong_shape_is_unsupported() {
    let parsed = parse_codex_manifest(
        &root(),
        codex_rel(),
        br#"{ "name": "Weather", "version": "1.0.0", "agents": { "bad": true } }"#,
    )
    .expect("should parse");
    assert!(parsed.agents.is_empty());
    assert!(
        parsed
            .descriptor
            .unsupported_capabilities
            .contains(&"plugin agents".to_string())
    );
}

// ── FR-033: hooks (`hooks` section normalisation) ───────────────────────────

#[test]
fn extract_hooks_reads_object_array_and_flat_shapes() {
    // Object of trigger -> command string.
    let object = serde_json::json!({ "PreToolUse": "./check.sh" });
    let hooks = ragent_plugins::extract_hooks("p", root().as_path(), Some(&object));
    assert_eq!(hooks.len(), 1);
    assert_eq!(hooks[0].plugin_id, "p");
    assert_eq!(hooks[0].trigger, "PreToolUse");
    assert_eq!(hooks[0].command, "./check.sh");
    assert_eq!(hooks[0].timeout_secs, None);
    assert_eq!(hooks[0].plugin_root, root());

    // Object of trigger -> array of mixed entries (Claude hooks.json shape).
    let nested = serde_json::json!({
        "PostToolUse": [
            "logger.sh",
            { "command_path": "./audit.sh", "timeout_secs": 5 }
        ]
    });
    let hooks = ragent_plugins::extract_hooks("p", root().as_path(), Some(&nested));
    assert_eq!(hooks.len(), 2);
    assert_eq!(hooks[0].command, "logger.sh");
    assert_eq!(hooks[1].command, "./audit.sh");
    assert_eq!(hooks[1].timeout_secs, Some(5));

    // Flat array of {trigger, command} (ragent.json shape).
    let flat = serde_json::json!([{ "trigger": "on_session_start", "command": "echo hi" }]);
    let hooks = ragent_plugins::extract_hooks("p", root().as_path(), Some(&flat));
    assert_eq!(hooks.len(), 1);
    assert_eq!(hooks[0].trigger, "on_session_start");
    assert_eq!(hooks[0].command, "echo hi");
}

#[test]
fn extract_hooks_reads_claude_group_shape_with_matchers() {
    // The official Claude `hooks/hooks.json` shape: each trigger maps to a
    // group `{matcher, hooks: [{type, command|command_path, timeout}]}`.
    let raw = serde_json::json!({
        "PostToolUse": [
            {
                "hooks": [
                    { "type": "command", "command": "check.sh" },
                    { "type": "command", "command_path": "${CLAUDE_PLUGIN_ROOT}/hooks/x.py" }
                ],
                "matcher": "Edit|Write|MultiEdit"
            },
            {
                "hooks": [
                    {
                        "type": "command",
                        "command": "review.sh",
                        "if": "Bash(git commit:*)",
                        "asyncRewake": true
                    }
                ],
                "matcher": "Bash"
            }
        ]
    });
    let hooks = ragent_plugins::extract_hooks("sg", root().as_path(), Some(&raw));
    assert_eq!(hooks.len(), 3);
    assert_eq!(hooks[0].matcher.as_deref(), Some("Edit|Write|MultiEdit"));
    assert_eq!(hooks[0].command, "check.sh");
    assert_eq!(hooks[1].command, "${CLAUDE_PLUGIN_ROOT}/hooks/x.py");
    assert_eq!(hooks[1].matcher.as_deref(), Some("Edit|Write|MultiEdit"));
    assert_eq!(hooks[2].command, "review.sh");
    assert_eq!(hooks[2].matcher.as_deref(), Some("Bash(git commit:*)"));
}

#[test]
fn extract_hooks_accepts_claude_timeout_and_skips_non_command_types() {
    let raw = serde_json::json!({
        "SessionStart": [
            { "hooks": [
                { "type": "command", "command": "boot.sh", "timeout": 180 },
                { "type": "prompt", "command": "not-a-command" }
            ] }
        ]
    });
    let hooks = ragent_plugins::extract_hooks("p", root().as_path(), Some(&raw));
    assert_eq!(hooks.len(), 1, "non-command entry types are skipped");
    assert_eq!(hooks[0].timeout_secs, Some(180));
}

#[test]
fn extract_hooks_star_matcher_is_omitted() {
    let raw = serde_json::json!({ "SessionStart": [ { "hooks": [
        { "type": "command", "command": "a.sh" }
    ], "matcher": "*" } ] });
    let hooks = ragent_plugins::extract_hooks("p", root().as_path(), Some(&raw));
    assert_eq!(
        hooks[0].matcher, None,
        "`*` matches everything, so it is dropped"
    );
}

#[test]
fn manifest_reads_claude_hooks_json_file() {
    let dir = tempfile::tempdir().expect("tempdir");
    let root = dir.path();
    std::fs::create_dir_all(root.join("hooks")).expect("mkdir");
    std::fs::create_dir_all(root.join(".claude-plugin")).expect("mkdir");
    std::fs::write(
        root.join("hooks/hooks.json"),
        br#"{
            "description": "demo",
            "hooks": {
                "SessionStart": [
                    { "hooks": [
                        { "type": "command", "command": "boot.sh", "timeout": 180 }
                    ] }
                ],
                "Stop": [
                    { "hooks": [
                        { "type": "command", "command": "review.sh", "asyncRewake": true }
                    ] }
                ]
            }
        }"#,
    )
    .expect("write");
    std::fs::write(
        root.join(".claude-plugin/plugin.json"),
        br#"{ "name": "sg", "version": "2.0.8" }"#,
    )
    .expect("write manifest");

    let parsed = parse_plugin_dir(root).expect("parse").expect("recognised");
    assert_eq!(parsed.hooks.len(), 2, "hooks/hooks.json triggers bridged");
    let triggers: Vec<&str> = parsed.hooks.iter().map(|h| h.trigger.as_str()).collect();
    assert!(triggers.contains(&"SessionStart"));
    assert!(triggers.contains(&"Stop"));
    let start = parsed
        .hooks
        .iter()
        .find(|h| h.trigger == "SessionStart")
        .expect("SessionStart");
    assert_eq!(start.timeout_secs, Some(180));
    assert_eq!(start.plugin_root, root);
    assert!(
        parsed
            .descriptor
            .unsupported_capabilities
            .iter()
            .all(|c| c != "plugin hooks"),
    );
}

#[test]
fn manifest_reads_root_hooks_json_file() {
    let dir = tempfile::tempdir().expect("tempdir");
    let root = dir.path();
    std::fs::create_dir_all(root.join(".claude-plugin")).expect("mkdir");
    std::fs::write(
        root.join("hooks.json"),
        br#"{ "hooks": { "PreToolUse": "./guard.sh" } }"#,
    )
    .expect("write");
    std::fs::write(
        root.join(".claude-plugin/plugin.json"),
        br#"{ "name": "p", "version": "1.0.0" }"#,
    )
    .expect("write manifest");

    let parsed = parse_plugin_dir(root).expect("parse").expect("recognised");
    assert_eq!(parsed.hooks.len(), 1);
    assert_eq!(parsed.hooks[0].trigger, "PreToolUse");
    assert_eq!(parsed.hooks[0].command, "./guard.sh");
}

#[test]
fn manifest_hooks_wrong_shape_is_unsupported() {
    let parsed = parse_codex_manifest(
        &root(),
        codex_rel(),
        br#"{ "name": "Weather", "version": "1.0.0", "hooks": 7 }"#,
    )
    .expect("should parse");
    assert!(parsed.hooks.is_empty());
    assert!(
        parsed
            .descriptor
            .unsupported_capabilities
            .contains(&"plugin hooks".to_string())
    );
}

#[test]
fn claude_manifest_bridges_hooks_declaration() {
    let parsed = parse_claude_manifest(
        Path::new("/plugins/pack"),
        Path::new(".claude-plugin/plugin.json"),
        br#"{
            "name": "pack",
            "hooks": { "PreToolUse": "./guard.sh", "PreCompact": "./snap.sh" }
        }"#,
    )
    .expect("should parse");
    assert_eq!(parsed.hooks.len(), 2);
    assert!(
        parsed
            .descriptor
            .unsupported_capabilities
            .iter()
            .all(|c| c != "plugin hooks"),
        "bridged hooks are not reported unsupported"
    );
}
