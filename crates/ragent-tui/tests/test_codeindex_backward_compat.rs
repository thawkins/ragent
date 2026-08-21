//! Backward-compatibility verification for existing `/codeindex` sub-commands
//! (spec graphCI, T-028).
//!
//! This test verifies that the existing `/codeindex` sub-commands — `on`,
//! `off`, `show`, `lang`, `reindex`, `rebuild`, `help` — and the default
//! usage string are still present and unchanged after the graph extension
//! was added.  The new graph sub-commands (`graph`, `explain`, `path`,
//! `communities`, `godnodes`) are additive and do not replace any existing
//! sub-command (FR-005, FR-021).
//!
//! The test works by reading the slash.rs source file and asserting that
//! all existing match arms are still present in the `"codeindex" =>` block.

use std::path::PathBuf;

fn workspace_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent() // crates/
        .and_then(|p| p.parent()) // workspace root
        .map(|p| p.to_path_buf())
        .unwrap_or_else(|| PathBuf::from(".."))
}

fn slash_rs_path() -> PathBuf {
    workspace_root().join("crates/ragent-tui/src/app/slash.rs")
}

fn state_rs_path() -> PathBuf {
    workspace_root().join("crates/ragent-tui/src/app/state.rs")
}

#[test]
fn test_existing_codeindex_subcommands_present() {
    let source = std::fs::read_to_string(slash_rs_path()).expect("read slash.rs");

    // The codeindex match block must contain all existing sub-command match
    // arms.  These patterns are specific enough that they only appear in the
    // codeindex block.  We verify the block exists by checking for the
    // `"codeindex" => {` marker, then verify each sub-command arm.
    assert!(
        source.contains("\"codeindex\" => {"),
        "codeindex match arm not found"
    );

    let existing_subcommands = [
        "\"on\" | \"enable\" =>",
        "\"off\" | \"disable\" =>",
        "\"show\" | \"status\" | \"\" =>",
        "\"lang\" | \"languages\" =>",
        "\"reindex\" =>",
        "\"rebuild\" =>",
        "\"help\" =>",
    ];

    for pattern in &existing_subcommands {
        assert!(
            source.contains(pattern),
            "Existing sub-command pattern not found in slash.rs: {pattern}"
        );
    }

    // Also verify the default fallback arm exists in the codeindex block.
    assert!(
        source.contains("\"Usage: `/codeindex "),
        "Usage fallback not found in codeindex block"
    );
}

#[test]
fn test_help_text_preserves_existing_rows() {
    let source = std::fs::read_to_string(slash_rs_path()).expect("read slash.rs");

    // The help text must contain all existing sub-command lines. Each
    // line uses the form `/codeindex <cmd>` — <description>; we verify
    // the command and its description both appear in the source.
    let existing_help_rows = [
        ("`/codeindex on`", "Enable codebase indexing"),
        ("`/codeindex off`", "Disable codebase indexing"),
        (
            "`/codeindex show`",
            "Show index and graph status & statistics",
        ),
        ("`/codeindex lang`", "List supported languages"),
        ("`/codeindex reindex`", "Trigger a full re-index"),
        ("`/codeindex rebuild`", "Rebuild FTS index from SQLite"),
        ("`/codeindex help`", "Show this help"),
    ];

    for (cmd, desc) in &existing_help_rows {
        assert!(
            source.contains(cmd),
            "Existing help command not found in slash.rs: {cmd}"
        );
        assert!(
            source.contains(desc),
            "Existing help description not found in slash.rs: {desc}"
        );
    }
}

#[test]
fn test_help_text_includes_new_graph_rows() {
    let source = std::fs::read_to_string(slash_rs_path()).expect("read slash.rs");

    // The help text must also contain the new graph sub-command lines.
    // We verify the command and description both appear in the source.
    let new_help_rows = [
        ("`/codeindex graph build`", "Build the semantic edge graph"),
        (
            "`/codeindex graph export`",
            "Export graph.json and GRAPH_REPORT.md",
        ),
        (
            "`/codeindex explain <symbol>`",
            "Show a symbol's connections",
        ),
        (
            "`/codeindex path <A> <B>`",
            "Shortest path between two symbols",
        ),
        ("`/codeindex communities`", "List detected communities"),
        ("`/codeindex godnodes`", "Top-N highest-degree symbols"),
    ];

    for (cmd, desc) in &new_help_rows {
        assert!(
            source.contains(cmd),
            "New graph help command not found in slash.rs: {cmd}"
        );
        assert!(
            source.contains(desc),
            "New graph help description not found in slash.rs: {desc}"
        );
    }
}

#[test]
fn test_help_text_preserves_existing_tool_list() {
    let source = std::fs::read_to_string(slash_rs_path()).expect("read slash.rs");

    // The help text must still list all existing codeindex tools.
    let existing_tools = [
        "`codeindex_search`",
        "`codeindex_symbols`",
        "`codeindex_references`",
        "`codeindex_dependencies`",
        "`codeindex_status`",
        "`codeindex_reindex`",
    ];

    for tool in &existing_tools {
        assert!(
            source.contains(tool),
            "Existing tool not found in help text: {tool}"
        );
    }
}

#[test]
fn test_usage_string_includes_existing_subcommands() {
    let source = std::fs::read_to_string(slash_rs_path()).expect("read slash.rs");

    // Find the main usage string in the codeindex block's `_ =>` fallback.
    // We use a marker that is specific to the fallback usage (not the
    // `graph` sub-command's own usage string) to ensure we find the right one.
    let usage_marker = "Usage: `/codeindex on|off";
    let usage_start = source
        .find(usage_marker)
        .expect("found main usage string in codeindex fallback");

    // Extract a reasonable window around the usage string.
    let window = &source[usage_start..usage_start + 300];

    // The usage string must include all existing sub-command names.
    let existing_usage_parts = ["on", "off", "show", "lang", "reindex", "rebuild", "help"];

    for part in &existing_usage_parts {
        assert!(
            window.contains(part),
            "Existing sub-command not found in usage string: {part}"
        );
    }
}

#[test]
fn test_no_new_top_level_slash_command() {
    // FR-022: No new top-level slash command. The only entry point for graph
    // features is `/codeindex <subcommand>`. Verify that there is no
    // top-level `"graph"`, `"explain"`, `"path"`, `"communities"`, or
    // `"godnodes"` match arm outside the codeindex block.
    // Check the SlashCommandDef list in state.rs for top-level commands.
    let state_source = std::fs::read_to_string(state_rs_path()).expect("read state.rs");

    // There should be no SlashCommandDef with trigger "graph", "explain",
    // "path", "communities", or "godnodes".
    for cmd in &["graph", "explain", "path", "communities", "godnodes"] {
        let pattern = format!("trigger: \"{cmd}\"");
        assert!(
            !state_source.contains(&pattern),
            "Found a top-level SlashCommandDef that should not exist: {cmd}"
        );
    }
}

#[test]
fn test_slash_command_def_description_updated() {
    let source = std::fs::read_to_string(state_rs_path()).expect("read state.rs");

    // The SlashCommandDef description for codeindex should mention both
    // existing and new sub-commands.
    assert!(
        source.contains("trigger: \"codeindex\""),
        "codeindex SlashCommandDef not found"
    );

    // Find the description line for codeindex.
    let trigger_pos = source
        .find("trigger: \"codeindex\"")
        .expect("found codeindex trigger");
    let desc_region = &source[trigger_pos..trigger_pos + 300];

    // Must still include existing sub-commands.
    for existing in &["on", "off", "show", "lang", "reindex", "rebuild", "help"] {
        assert!(
            desc_region.contains(existing),
            "Existing sub-command not in SlashCommandDef description: {existing}"
        );
    }

    // Must include new sub-commands.
    for new_cmd in &["graph", "explain", "path", "communities", "godnodes"] {
        assert!(
            desc_region.contains(new_cmd),
            "New sub-command not in SlashCommandDef description: {new_cmd}"
        );
    }
}
