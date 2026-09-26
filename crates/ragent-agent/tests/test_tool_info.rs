//! Integration tests for the `tool_info` and `commands_info` tools.
//!
//! Verifies both tools are registered, require no permission category
//! (`"none"`), return valid JSON, and that `tool_info`'s snapshot matches the
//! registry it was fetched from (names, counts, visibility). `commands_info`
//! covers the builtin catalog and the (live) plugin-command resolution.

use ragent_agent::event::EventBus;
use ragent_agent::tool::{ToolContext, create_default_registry};
use serde_json::{Value, json};
use std::path::PathBuf;
use std::sync::Arc;

fn registry_ctx() -> ToolContext {
    ToolContext {
        session_id: "session-1".to_string(),
        working_dir: PathBuf::from("/tmp"),
        event_bus: Arc::new(EventBus::new(16)),
        storage: None,
        agent_manager: None,
        active_model: None,
        provider_registry: None,
        team_context: None,
        team_manager: None,
        code_index: None,
        bg_service: None,
        spec_manager: None,
        active_spec_id: None,
        config: None,
        allowed_roots: Vec::new(),
        read_timestamps: Arc::new(std::sync::RwLock::new(std::collections::HashMap::new())),
        cached_team_dir: Arc::new(std::sync::Mutex::new(None)),
        canonical_cache: Arc::new(ragent_tools_core::CanonicalPathCache::new()),
        tool_registry: Arc::new(create_default_registry()),
    }
}

fn parse_output_json(output: &ragent_agent::tool::ToolOutput) -> Value {
    serde_json::from_str(&output.content)
        .unwrap_or_else(|e| panic!("tool output should be valid JSON: {e}\n{}", output.content))
}

// ---------------------------------------------------------------------------
// tool_info
// ---------------------------------------------------------------------------

#[test]
fn test_tool_info_is_registered() {
    let registry = create_default_registry();
    let tool = registry.get("tool_info").expect("tool_info registered");
    assert_eq!(tool.name(), "tool_info");
    assert_eq!(tool.permission_category(), "none");
}

#[tokio::test]
async fn test_tool_info_returns_valid_json_with_counts_and_every_tool() {
    let registry = create_default_registry();
    let tool = Arc::clone(&registry.get("tool_info").expect("tool_info registered"));
    // The context must carry the same registry the tool is being executed
    // against, so the snapshot reflects this registry (not an empty default).
    let mut ctx = registry_ctx();
    let registry_handle = Arc::new(create_default_registry());
    // Ensure the tool itself is present in the registry it reports.
    assert!(registry_handle.get("tool_info").is_some());
    ctx.tool_registry = registry_handle.clone();

    let output = tool.execute(json!({}), &ctx).await.expect("execute ok");
    let parsed = parse_output_json(&output);

    let names = registry_handle.list();
    let expected_total = names.len() + registry_handle.hidden().len();
    assert_eq!(parsed["total"].as_u64().unwrap() as usize, expected_total);
    let visible = parsed["visible"].as_u64().unwrap() as usize;
    assert_eq!(
        visible + parsed["hidden"].as_u64().unwrap() as usize,
        expected_total
    );

    let tools: Vec<&Value> = parsed["tools"]
        .as_array()
        .expect("tools array")
        .iter()
        .collect();
    assert_eq!(tools.len(), expected_total);
    // Sorted by name and exactly the registry's full set (visible + hidden).
    let reported_names: Vec<&str> = tools.iter().map(|t| t["name"].as_str().unwrap()).collect();
    let mut expected: Vec<String> = names;
    expected.extend(registry_handle.hidden());
    expected.sort();
    assert_eq!(
        reported_names,
        expected.iter().map(String::as_str).collect::<Vec<_>>()
    );

    // Each entry carries a schema and a permission category.
    assert!(tools.iter().all(|t| t["permission_category"].is_string()));
    assert!(tools.iter().all(|t| t["parameters"].is_object()));
}

#[tokio::test]
async fn test_tool_info_marks_hidden_and_reports_categories() {
    let registry_handle = Arc::new(create_default_registry());
    registry_handle.set_hidden(&["read".to_string(), "write".to_string()]);
    let tool = registry_handle
        .get("tool_info")
        .expect("tool_info registered");
    let mut ctx = registry_ctx();
    ctx.tool_registry = registry_handle;

    let output = tool.execute(json!({}), &ctx).await.expect("execute ok");
    let parsed = parse_output_json(&output);
    let tools = parsed["tools"].as_array().expect("tools array");

    let read_entry = tools
        .iter()
        .find(|t| t["name"] == "read")
        .expect("read present");
    assert_eq!(read_entry["hidden"], true);
    assert_eq!(parsed["hidden"], 2);

    let tool_info_entry = tools
        .iter()
        .find(|t| t["name"] == "tool_info")
        .expect("tool_info present");
    assert_eq!(tool_info_entry["permission_category"], "none");
    assert_eq!(tool_info_entry["source"], "internal");
}

// ---------------------------------------------------------------------------
// commands_info
// ---------------------------------------------------------------------------

#[test]
fn test_commands_info_is_registered() {
    let registry = create_default_registry();
    let tool = registry
        .get("commands_info")
        .expect("commands_info registered");
    assert_eq!(tool.name(), "commands_info");
    assert_eq!(tool.permission_category(), "none");
}

#[tokio::test]
async fn test_commands_info_returns_builtin_catalog_and_plugin_fields() {
    let registry = create_default_registry();
    let tool = registry
        .get("commands_info")
        .expect("commands_info registered");
    let ctx = registry_ctx();

    let output = tool.execute(json!({}), &ctx).await.expect("execute ok");
    let parsed = parse_output_json(&output);

    let builtin = parsed["builtin"].as_array().expect("builtin array");
    assert!(!builtin.is_empty(), "builtin catalog must not be empty");
    // Every entry exposes the schema the tool advertises.
    assert!(builtin.iter().all(|c| {
        c["trigger"].is_string()
            && c["description"].is_string()
            && c["subcommands"].is_array()
            && c["flags"].is_array()
    }));

    let plugin_commands = parsed["plugin_commands"]
        .as_array()
        .expect("plugin_commands array");
    assert!(plugin_commands.iter().all(|c| {
        c["plugin_id"].is_string()
            && c["name"].is_string()
            && c["trigger"].is_string()
            && c["description"].is_string()
            && c["is_prompt_command"].is_boolean()
    }));

    assert_eq!(
        parsed["total"].as_u64().unwrap() as usize,
        builtin.len() + plugin_commands.len()
    );
}

#[tokio::test]
async fn test_commands_info_matches_the_tui_slash_command_table() {
    // Drift guard: the static catalog in `command_catalog` must list every
    // trigger the TUI exposes via `SLASH_COMMANDS`.
    let registry = create_default_registry();
    let tool = registry
        .get("commands_info")
        .expect("commands_info registered");
    let ctx = registry_ctx();

    let output = tool.execute(json!({}), &ctx).await.expect("execute ok");
    let parsed = parse_output_json(&output);
    let builtin: Vec<&str> = parsed["builtin"]
        .as_array()
        .expect("builtin array")
        .iter()
        .map(|c| c["trigger"].as_str().unwrap())
        .collect();

    for expected in [
        "plugins",
        "mcp",
        "new",
        "research",
        "spec",
        "tools",
        "help",
        "queue",
        "telemetry",
        "codeindex",
        "team",
        "teams",
        "swarm",
        "spawn",
    ] {
        assert!(
            builtin.contains(&expected),
            "missing builtin trigger {expected}"
        );
    }
}
