//! Inline tests for `http.rs` - compiled inside the module tree
//! via the `#[path]` delegation left in the source file
//! (ANTIPAT M2 inline-test migration; no behaviour change).

use super::*;

#[test]
fn parse_tool_list_extracts_tools() {
    let value = serde_json::json!({
        "tools": [
            {
                "name": "echo",
                "description": "Echoes input",
                "inputSchema": {"type": "object", "properties": {"msg": {"type": "string"}}}
            }
        ]
    });
    let tools = parse_tool_list(value);
    assert_eq!(tools.len(), 1);
    assert_eq!(tools[0].name, "echo");
    assert_eq!(tools[0].description, "Echoes input");
}

#[test]
fn parse_tool_list_handles_empty_result() {
    let tools = parse_tool_list(serde_json::json!({}));
    assert!(tools.is_empty());
}

#[test]
fn new_client_starts_connected() {
    let client = HttpMcpClient::new("http://localhost:9999", HashMap::new());
    assert!(!client.is_disconnected());
}

#[test]
fn unwrap_sse_frame_joins_multiline_data_fields() {
    // The SSE grammar concatenates the `data:` lines of one event with
    // `\n`; a server that splits a payload across lines must round-trip.
    let frame = "event: message\ndata: {\"jsonrpc\":\ndata: \"2.0\",\"result\":1}\n\n";
    assert_eq!(
        unwrap_sse_frame(frame),
        "{\"jsonrpc\":\n\"2.0\",\"result\":1}"
    );
}

#[test]
fn unwrap_sse_frame_keeps_bare_json_and_single_data_line() {
    let bare = "{\"result\": 1}";
    assert_eq!(unwrap_sse_frame(bare), bare);
    let framed = "event: message\ndata: {\"result\": 1}\n\n";
    assert_eq!(unwrap_sse_frame(framed), "{\"result\": 1}");
}
