//! Tests for manifest parsing, normalisation, and the host-API version check
//! (spec `plugins` T-003; FR-002, FR-019, FR-025).

use std::path::{Path, PathBuf};

use ragent_plugins::{
    HOST_API_VERSION, PluginDialect, PluginError, UNSUP_DESKTOP_MOUNTS, UNSUP_DESKTOP_WINDOW,
    UNSUP_EXEC, UNSUP_FS, UNSUP_MCP, check_api_version, derive_id, parse_claude_manifest,
    parse_codex_manifest, parse_manifest, parse_plugin_dir,
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
    assert_eq!(d.entry, root().join("index.js"));
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
    assert_eq!(parsed.commands[0].name, "weather");
    assert_eq!(parsed.commands[0].usage.as_deref(), Some("/weather <city>"));
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
fn codex_manifest_with_mcp_servers_section_is_recorded_unsupported() {
    let parsed = parse_codex_manifest(
        &root(),
        codex_rel(),
        br#"{
            "name": "w", "version": "1", "entry": "i.js",
            "mcpServers": { "fetch": { "command": "npx" } }
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
    assert_eq!(d.entry, root().join("index.js"));
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
    assert_eq!(parsed.commands[0].usage, None);
}

#[test]
fn claude_desktop_server_entry_is_resolved() {
    let parsed = parse_claude_manifest(
        &root(),
        Path::new(".claude-plugin/plugin.json"),
        br#"{ "name": "ext", "server": { "entry": "dist/server.js" } }"#,
    )
    .expect("should parse");
    assert_eq!(parsed.descriptor.entry, root().join("dist/server.js"));
}

#[test]
fn claude_mcp_section_is_recorded_unsupported_never_dropped() {
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
    assert_eq!(
        parsed.descriptor.unsupported_capabilities,
        vec![UNSUP_MCP.to_string()]
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
fn claude_manifest_rejects_missing_entry() {
    let err = parse_claude_manifest(
        &root(),
        Path::new(".claude-plugin/plugin.json"),
        br#"{ "name": "no-entry" }"#,
    )
    .unwrap_err();
    let PluginError::ManifestParse { detail, .. } = err else {
        panic!("expected ManifestParse, got {err:?}");
    };
    assert!(
        detail.contains("no entry point"),
        "detail should explain the missing entry, got: {detail}"
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
    assert_eq!(parsed.descriptor.entry, dir.join("index.js"));
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
