//! Coverage for the permission-engine helpers that previously had no test
//! references: `extract_resource_from_input`, `is_hardwired_auto_approved_tool`
//! and `is_interactive_tool`.

use ragent_agent::session::permissions::{
    extract_resource_from_input_for_test, is_hardwired_auto_approved_tool_for_test,
    is_interactive_tool,
};
use serde_json::json;

#[test]
fn test_extract_resource_prefers_path() {
    let input = json!({ "path": "src/main.rs", "command": "ignored" });
    assert_eq!(
        extract_resource_from_input_for_test(&input, "read"),
        "src/main.rs"
    );
}

#[test]
fn test_extract_resource_falls_back_through_known_keys() {
    assert_eq!(
        extract_resource_from_input_for_test(&json!({ "command": "ls" }), "bash"),
        "ls"
    );
    assert_eq!(
        extract_resource_from_input_for_test(&json!({ "url": "https://x" }), "webfetch"),
        "https://x"
    );
    assert_eq!(
        extract_resource_from_input_for_test(&json!({ "pattern": "fn main" }), "grep"),
        "fn main"
    );
    assert_eq!(
        extract_resource_from_input_for_test(&json!({ "query": "rust tui" }), "memory_recall"),
        "rust tui"
    );
}

#[test]
fn test_extract_resource_falls_back_to_tool_name() {
    assert_eq!(
        extract_resource_from_input_for_test(&json!({}), "mystery_tool"),
        "tool:mystery_tool"
    );
    // A non-string path value is ignored, not coerced.
    assert_eq!(
        extract_resource_from_input_for_test(&json!({ "path": 42 }), "read"),
        "tool:read"
    );
}

#[test]
fn test_hardwired_auto_approved_covers_expected_families() {
    for tool in [
        "codeindex_search",
        "codeindex_status",
        "team_create",
        "new_agent",
        "agent_complete",
        "task_list",
        "ask_user",
        "model_info",
        "ragent_info",
        "tool_info",
        "commands_info",
    ] {
        assert!(
            is_hardwired_auto_approved_tool_for_test(tool),
            "{tool} must be hardwired auto-approved"
        );
    }
}

#[test]
fn test_hardwired_auto_approved_rejects_untrusted_tools() {
    for tool in ["bash", "write", "edit", "rm", "read", "glob"] {
        assert!(
            !is_hardwired_auto_approved_tool_for_test(tool),
            "{tool} must NOT be hardwired auto-approved"
        );
    }
}

#[test]
fn test_is_interactive_tool_only_ask_user() {
    assert!(is_interactive_tool("ask_user"));
    assert!(!is_interactive_tool("bash"));
    assert!(!is_interactive_tool("read"));
}
