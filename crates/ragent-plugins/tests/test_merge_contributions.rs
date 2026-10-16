//! Tests for the manifest/runtime contribution merge (spec `plugins` FR-005,
//! FR-004, FR-024).
//!
//! A tool declared in the manifest and registered at runtime under the same name
//! is one contribution, not a `name-collision`; a genuine duplicate within either
//! source is preserved so the registration path still rejects it.

use ragent_plugins::{
    CommandSource, PluginCommandDecl, PluginCommandDef, PluginToolDecl, merge_contributed_commands,
    merge_contributed_tools,
};
use serde_json::json;

fn tool(name: &str, description: &str) -> PluginToolDecl {
    PluginToolDecl {
        name: name.to_string(),
        description: description.to_string(),
        parameters: json!({ "type": "object" }),
    }
}

fn command(name: &str) -> PluginCommandDef {
    PluginCommandDef::inline(PluginCommandDecl {
        name: name.to_string(),
        description: "d".to_string(),
        usage: None,
    })
}

fn command_decl(name: &str) -> PluginCommandDecl {
    PluginCommandDecl {
        name: name.to_string(),
        description: "d".to_string(),
        usage: None,
    }
}

#[test]
fn manifest_tool_and_matching_runtime_registration_merge_to_one() {
    let merged = merge_contributed_tools(
        vec![tool("get_weather", "declared")],
        vec![tool("get_weather", "runtime")],
    );
    assert_eq!(merged.len(), 1, "one merged contribution: {merged:?}");
    // The manifest metadata wins; the runtime call supplies only the handler.
    assert_eq!(merged[0].description, "declared");
}

#[test]
fn a_runtime_only_tool_is_appended_after_the_manifest_tools() {
    let merged =
        merge_contributed_tools(vec![tool("declared", "d")], vec![tool("registered", "d")]);
    let names: Vec<&str> = merged.iter().map(|d| d.name.as_str()).collect();
    assert_eq!(names, vec!["declared", "registered"]);
}

#[test]
fn a_duplicate_within_the_manifest_is_preserved_for_the_collision_path() {
    let merged = merge_contributed_tools(
        vec![tool("twice", "first"), tool("twice", "second")],
        Vec::new(),
    );
    assert_eq!(merged.len(), 2, "a manifest duplicate survives the merge");
}

#[test]
fn a_duplicate_within_the_runtime_registrations_is_preserved() {
    let merged = merge_contributed_tools(
        Vec::new(),
        vec![tool("twice", "first"), tool("twice", "second")],
    );
    assert_eq!(merged.len(), 2, "two register_tool calls still collide");
}

#[test]
fn manifest_command_and_matching_runtime_registration_merge_to_one() {
    let merged =
        merge_contributed_commands(vec![command("todo-add")], vec![command_decl("todo-add")]);
    assert_eq!(merged.len(), 1, "one merged command: {merged:?}");
    // The manifest entry is kept (declared-first precedence).
    assert!(matches!(merged[0].source, CommandSource::Inline));
}

#[test]
fn a_runtime_only_command_is_appended_as_inline() {
    let merged = merge_contributed_commands(Vec::new(), vec![command_decl("todo-add")]);
    assert_eq!(merged.len(), 1);
    assert!(matches!(merged[0].source, CommandSource::Inline));
}

#[test]
fn a_duplicate_within_the_manifest_commands_is_preserved() {
    let merged = merge_contributed_commands(vec![command("twice"), command("twice")], Vec::new());
    assert_eq!(merged.len(), 2, "a command duplicate survives the merge");
}
