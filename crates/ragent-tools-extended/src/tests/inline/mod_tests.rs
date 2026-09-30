//! Inline tests for `mod.rs` - compiled inside the module tree
//! via the `#[path]` delegation left in the source file
//! (ANTIPAT M2 inline-test migration; no behaviour change).

use super::*;

#[test]
fn test_browser_tool_name() {
    let tool = BrowserTool;
    assert_eq!(tool.name(), "browser");
}

#[test]
fn test_browser_tool_permission_category() {
    let tool = BrowserTool;
    assert_eq!(tool.permission_category(), "web");
}

#[test]
fn test_parameters_schema_has_all_actions() {
    let tool = BrowserTool;
    let schema = tool.parameters_schema();
    let actions = schema
        .pointer("/properties/action/enum")
        .and_then(Value::as_array)
        .expect("action enum should exist");

    let action_names: Vec<&str> = actions.iter().filter_map(Value::as_str).collect();

    assert!(action_names.contains(&"open"));
    assert!(action_names.contains(&"snapshot"));
    assert!(action_names.contains(&"click"));
    assert!(action_names.contains(&"type"));
    assert!(action_names.contains(&"fill_form"));
    assert!(action_names.contains(&"select"));
    assert!(action_names.contains(&"wait"));
    assert!(action_names.contains(&"eval"));
    assert!(action_names.contains(&"scroll"));
    assert!(action_names.contains(&"upload"));
    assert!(action_names.contains(&"press"));
    assert!(action_names.contains(&"screenshot"));
    assert!(action_names.contains(&"status"));
    assert!(action_names.contains(&"setup"));
}

#[test]
fn test_parameters_schema_requires_action() {
    let tool = BrowserTool;
    let schema = tool.parameters_schema();
    let required = schema
        .get("required")
        .and_then(Value::as_array)
        .expect("required should exist");
    assert!(required.iter().any(|v| v.as_str() == Some("action")));
}

#[test]
fn test_default_cdp_endpoint() {
    assert_eq!(DEFAULT_CDP_ENDPOINT, "http://127.0.0.1:9222");
}

#[test]
fn test_format_browser_result_snapshot() {
    let value = json!({
        "title": "Example",
        "url": "https://example.com",
        "text": "Hello world",
        "html_length": 100,
        "node_id": 1,
    });
    let text = format_browser_result("snapshot", &value);
    assert!(text.contains("Example"));
    assert!(text.contains("Hello world"));
}

#[test]
fn test_format_browser_result_open() {
    let value = json!({
        "url": "https://example.com",
        "loaded": true,
    });
    let text = format_browser_result("open", &value);
    assert!(text.contains("https://example.com"));
    assert!(text.contains("true"));
}

#[test]
fn test_format_browser_result_click() {
    let value = json!({ "selector": "#button", "clicked": true });
    let text = format_browser_result("click", &value);
    assert!(text.contains("#button"));
}

#[test]
fn test_format_browser_result_eval() {
    let value = json!({ "expression": "1+1", "result": 2 });
    let text = format_browser_result("eval", &value);
    assert!(text.contains('2'));
}

#[test]
fn test_format_browser_result_screenshot() {
    let value = json!({
        "data_length": 1024,
        "full_page": true,
        "format": "png",
    });
    let text = format_browser_result("screenshot", &value);
    assert!(text.contains("1024"));
    assert!(text.contains("true"));
}

#[test]
fn test_format_browser_result_status_available() {
    let value = json!({
        "browser": "Chrome/131.0",
        "page_targets": 2,
        "available": true,
    });
    let text = format_browser_result("status", &value);
    assert!(text.contains("Chrome/131.0"));
    assert!(text.contains("available"));
}

#[test]
fn test_format_browser_result_setup() {
    let value = json!({
        "status": "launched",
        "http_endpoint": "http://127.0.0.1:9222",
        "browser": "Chrome/131.0",
    });
    let text = format_browser_result("setup", &value);
    assert!(text.contains("launched"));
    assert!(text.contains("Chrome/131.0"));
}
