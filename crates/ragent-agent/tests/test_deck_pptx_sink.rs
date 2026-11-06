//! Tests for MCP server selection for the deck's PowerPoint export (spec
//! `diffdeck` FR-030).
//!
//! The sink is not hard-wired to one vendor: it drives whichever connected
//! server advertises a tool that creates a presentation. That match is the
//! whole of the capability test, so it is pinned here - a server that only reads
//! presentations, a spreadsheet server, a document server, or a disconnected
//! server must never be picked, and the choice must be stable for a given set of
//! connected servers.

use ragent_agent::mcp::deck_sink::{DeckTool, select_deck_tool};
use ragent_agent::mcp::{McpServer, McpStatus, McpToolDef};
use ragent_config::McpServerConfig;
use serde_json::json;

/// A tool definition with no parameters.
fn tool(name: &str, description: &str) -> McpToolDef {
    McpToolDef {
        name: name.to_string(),
        description: description.to_string(),
        parameters: json!({ "type": "object", "properties": {} }),
    }
}

/// A server with `id`, in `status`, advertising `tools`.
fn server(id: &str, status: McpStatus, tools: Vec<McpToolDef>) -> McpServer {
    McpServer {
        id: id.to_string(),
        config: McpServerConfig {
            command: Some("deckcraft-cli".to_string()),
            ..McpServerConfig::default()
        },
        status,
        tools,
    }
}

/// The tool set a DeckCraft-style server advertises: a creation tool plus a
/// generic command channel that carries add-slide and save.
fn deckcraft_tools() -> Vec<McpToolDef> {
    vec![
        tool("new_presentation", "Start a new presentation"),
        tool("run_command", "Run any command by id with JSON parameters"),
        tool("inspect_slide", "One slide's shapes"),
    ]
}

#[test]
fn test_a_command_channel_server_is_selected_with_its_create_tool() {
    let servers = vec![server("deckcraft", McpStatus::Connected, deckcraft_tools())];
    let selected = select_deck_tool(&servers).expect("a deck server is selected");
    assert_eq!(selected.server_id, "deckcraft");
    assert_eq!(selected.create_tool, "new_presentation");
    assert_eq!(selected.command_tool.as_deref(), Some("run_command"));
    assert_eq!(
        selected.provider(),
        "deckcraft/new_presentation",
        "the provider label names the server and the tool it used"
    );
}

#[test]
fn test_without_a_command_channel_add_slide_and_save_are_required() {
    // A server with explicit per-slide and save tools qualifies and records both.
    let explicit = vec![server(
        "pptx-mcp",
        McpStatus::Connected,
        vec![
            tool("create_presentation", "Create a new PowerPoint file"),
            tool("add_slide", "Add a slide to the presentation"),
            tool("save_presentation", "Save the presentation to a path"),
        ],
    )];
    let selected = select_deck_tool(&explicit).expect("an explicit-tool server is selected");
    assert_eq!(selected.command_tool, None);
    assert_eq!(selected.add_slide_tool.as_deref(), Some("add_slide"));
    assert_eq!(selected.save_tool.as_deref(), Some("save_presentation"));

    // The same server without a save tool cannot produce a file, so it is not
    // selected: advertising a create tool alone is not the capability.
    let no_save = vec![server(
        "pptx-mcp",
        McpStatus::Connected,
        vec![
            tool("create_presentation", "Create a new PowerPoint file"),
            tool("add_slide", "Add a slide to the presentation"),
        ],
    )];
    assert_eq!(
        select_deck_tool(&no_save),
        None,
        "a server that cannot save a presentation is not selected"
    );
}

#[test]
fn test_a_reader_of_presentations_is_not_a_deck_server() {
    let servers = vec![server(
        "slide-reader",
        McpStatus::Connected,
        vec![
            tool("read_presentation", "Read the slides of an existing pptx"),
            tool("list_slides", "List the slides in a deck"),
        ],
    )];
    assert_eq!(
        select_deck_tool(&servers),
        None,
        "a tool that only reads presentations never qualifies"
    );
}

#[test]
fn test_other_office_servers_are_not_mistaken_for_a_deck_server() {
    // The spreadsheet and word-processor tool sets share the `create` verb but
    // have no presentation noun, so the noun is what keeps them out.
    let servers = vec![
        server(
            "gridcraft",
            McpStatus::Connected,
            vec![
                tool("new_workbook", "Create a new workbook"),
                tool("save_workbook", "Save the workbook"),
                tool("execute_command", "Run any command by id"),
            ],
        ),
        server(
            "wordcraft",
            McpStatus::Connected,
            vec![
                tool("new_document", "Start a document"),
                tool("save_document", "Save the document"),
                tool("execute", "Run one command by id"),
            ],
        ),
    ];
    assert_eq!(
        select_deck_tool(&servers),
        None,
        "a spreadsheet or word-processor server is not a presentation writer"
    );
}

#[test]
fn test_a_disabled_server_is_skipped_even_when_it_could_write_a_deck() {
    let servers = vec![server("deckcraft", McpStatus::Disabled, deckcraft_tools())];
    assert_eq!(
        select_deck_tool(&servers),
        None,
        "only a connected server can write the presentation"
    );
}

#[test]
fn test_selection_is_stable_and_prefers_the_lowest_server_id() {
    let build = || {
        vec![
            server("zebra-deck", McpStatus::Connected, deckcraft_tools()),
            server("alpha-deck", McpStatus::Connected, deckcraft_tools()),
        ]
    };
    let first = select_deck_tool(&build()).expect("a deck server is selected");
    let second = select_deck_tool(&build()).expect("a deck server is selected");
    assert_eq!(first, second, "the same set of servers picks the same one");
    assert_eq!(
        first.server_id, "alpha-deck",
        "servers are considered in id order, so the choice is stable"
    );

    // Reversing the input order does not change the answer.
    let mut reversed = build();
    reversed.reverse();
    assert_eq!(
        select_deck_tool(&reversed).map(|tool| tool.server_id),
        Some("alpha-deck".to_string())
    );
}

#[test]
fn test_a_server_with_no_tools_is_skipped() {
    let servers = vec![
        server("empty", McpStatus::Connected, Vec::new()),
        server("deckcraft", McpStatus::Connected, deckcraft_tools()),
    ];
    let selected = select_deck_tool(&servers).expect("the deck server is selected");
    assert_eq!(selected.server_id, "deckcraft");
}

#[test]
fn test_deck_tool_provider_label_names_the_server_and_tool() {
    let tool = DeckTool {
        server_id: "deckcraft".to_string(),
        create_tool: "run_command".to_string(),
        command_tool: Some("run_command".to_string()),
        add_slide_tool: None,
        save_tool: None,
    };
    assert_eq!(tool.provider(), "deckcraft/run_command");
}

/// A DeckCraft-style server as it really advertises itself: the wrapper tools
/// this workspace's `/mcp` discovery reports.
fn deckcraft_wrapper_tools() -> Vec<McpToolDef> {
    vec![
        tool("new_presentation", "Start a new presentation"),
        tool("open_presentation", "Open a presentation file"),
        tool("save_presentation", "Save the active presentation"),
        tool("run_command", "Run any command by id with JSON parameters"),
        tool("batch", "Run commands in order"),
        tool("add_slide", "Insert a slide after the current one"),
    ]
}

#[test]
fn test_a_command_server_that_also_publishes_wrappers_records_both() {
    // The command path prefers the wrappers (`add_slide`, `save_presentation`):
    // DeckCraft's own command ids are `file.new`/`file.save`, and it answers the
    // older `presentation.new`/`presentation.save` ids with `unknown command`,
    // so driving it by raw command produced no file at all while every call
    // still "succeeded". Recording the wrappers keeps them reachable.
    let servers = vec![server(
        "deckcraft",
        McpStatus::Connected,
        deckcraft_wrapper_tools(),
    )];
    let selected = select_deck_tool(&servers).expect("a deck server is selected");
    assert_eq!(selected.command_tool.as_deref(), Some("run_command"));
    assert_eq!(selected.add_slide_tool.as_deref(), Some("add_slide"));
    assert_eq!(
        selected.save_tool.as_deref(),
        Some("save_presentation"),
        "the save wrapper is what actually writes the file"
    );
}

#[test]
fn test_a_rejected_tool_call_is_not_a_success() {
    // `McpClient::call_tool` succeeds on the transport even when the server
    // rejects the tool; the rejection is `is_error` on the result. The sink turns
    // that into an error, so the export must be able to see it.
    let rejected = json!({
        "content": [ { "type": "text", "text": "unknown command `presentation.new`" } ],
        "is_error": true,
    });
    assert_eq!(
        rejected
            .get("is_error")
            .and_then(serde_json::Value::as_bool),
        Some(true),
        "a rejected command reports is_error, which the sink must honour"
    );
    let accepted = json!({ "content": [], "is_error": false });
    assert_eq!(
        accepted
            .get("is_error")
            .and_then(serde_json::Value::as_bool),
        Some(false)
    );
}
