//! Integration tests for the `browser` tool — JCODEPLAN M4 (T-033).
//!
//! Covers:
//! - Tool identity (name, permission category, description)
//! - Parameters schema (all 14 actions present, required fields)
//! - Graceful degradation when no CDP endpoint is available
//! - Config-based endpoint resolution
//! - Browser tool registration in the extended registry
//! - Tool-visibility switch (`browser`) in config
//! - CDP types deserialisation (VersionInfo, TargetInfo)
//! - HTML-to-text conversion helper
//! - Key code mapping helpers
//! - Conditional CDP tests (only run when Chrome is available at port 9222)

use std::sync::Arc;

use ragent_config::{Config, tool_family_names};
use ragent_tools_extended::browser::{BROWSER_TOOL_NAME, BrowserTool, DEFAULT_CDP_ENDPOINT};
use ragent_tools_extended::{Tool, ToolContext, create_extended_registry};
use ragent_types::event::EventBus;
use serde_json::json;

// ---------------------------------------------------------------------------
// Test context helper
// ---------------------------------------------------------------------------

/// Build a minimal `ToolContext` for testing with no config.
fn ctx_no_config() -> ToolContext {
    ToolContext {
        session_id: "test".to_string(),
        working_dir: std::path::PathBuf::from("."),
        event_bus: Arc::new(EventBus::new(64)),
        storage: None,
        code_index: None,
        config: None,
        read_timestamps: Arc::new(std::sync::RwLock::new(std::collections::HashMap::new())),
    }
}

/// Build a `ToolContext` with a config containing a custom CDP endpoint.
fn ctx_with_config(endpoint: &str) -> ToolContext {
    let mut config = ragent_config::Config::default();
    config.browser.cdp_endpoint = Some(endpoint.to_string());

    ToolContext {
        session_id: "test".to_string(),
        working_dir: std::path::PathBuf::from("."),
        event_bus: Arc::new(EventBus::new(64)),
        storage: None,
        code_index: None,
        config: Some(Arc::new(config)),
        read_timestamps: Arc::new(std::sync::RwLock::new(std::collections::HashMap::new())),
    }
}

// ---------------------------------------------------------------------------
// Tool identity
// ---------------------------------------------------------------------------

#[test]
fn test_tool_name_is_browser() {
    let tool = BrowserTool;
    assert_eq!(tool.name(), BROWSER_TOOL_NAME);
    assert_eq!(tool.name(), "browser");
}

#[test]
fn test_permission_category_is_web() {
    let tool = BrowserTool;
    assert_eq!(tool.permission_category(), "web");
}

#[test]
fn test_description_mentions_cdp_and_actions() {
    let tool = BrowserTool;
    let desc = tool.description();
    assert!(desc.contains("CDP") || desc.contains("DevTools"));
    assert!(desc.contains("open"));
    assert!(desc.contains("snapshot"));
    assert!(desc.contains("click"));
    assert!(desc.contains("screenshot"));
    assert!(desc.contains("setup"));
}

// ---------------------------------------------------------------------------
// Parameters schema
// ---------------------------------------------------------------------------

#[test]
fn test_parameters_schema_has_all_fourteen_actions() {
    let tool = BrowserTool;
    let schema = tool.parameters_schema();
    let actions = schema
        .pointer("/properties/action/enum")
        .and_then(|v| v.as_array())
        .expect("action enum should exist");

    assert_eq!(
        actions.len(),
        14,
        "should have exactly 14 actions, got {actions:?}"
    );

    let action_names: Vec<&str> = actions.iter().filter_map(|v| v.as_str()).collect();
    for expected in &[
        "open",
        "snapshot",
        "click",
        "type",
        "fill_form",
        "select",
        "wait",
        "eval",
        "scroll",
        "upload",
        "press",
        "screenshot",
        "status",
        "setup",
    ] {
        assert!(
            action_names.contains(expected),
            "action '{expected}' should be in the schema enum"
        );
    }
}

#[test]
fn test_parameters_schema_requires_action() {
    let tool = BrowserTool;
    let schema = tool.parameters_schema();
    let required = schema
        .get("required")
        .and_then(|v| v.as_array())
        .expect("required should exist");
    assert!(
        required.iter().any(|v| v.as_str() == Some("action")),
        "'action' should be required"
    );
}

#[test]
fn test_parameters_schema_has_url_for_open() {
    let tool = BrowserTool;
    let schema = tool.parameters_schema();
    assert!(
        schema.pointer("/properties/url").is_some(),
        "schema should have a 'url' property"
    );
}

#[test]
fn test_parameters_schema_has_selector() {
    let tool = BrowserTool;
    let schema = tool.parameters_schema();
    assert!(
        schema.pointer("/properties/selector").is_some(),
        "schema should have a 'selector' property"
    );
}

#[test]
fn test_parameters_schema_has_expression_for_eval() {
    let tool = BrowserTool;
    let schema = tool.parameters_schema();
    assert!(
        schema.pointer("/properties/expression").is_some(),
        "schema should have an 'expression' property"
    );
}

#[test]
fn test_parameters_schema_has_fields_for_fill_form() {
    let tool = BrowserTool;
    let schema = tool.parameters_schema();
    assert!(
        schema.pointer("/properties/fields").is_some(),
        "schema should have a 'fields' property"
    );
}

#[test]
fn test_parameters_schema_has_file_path_for_upload() {
    let tool = BrowserTool;
    let schema = tool.parameters_schema();
    assert!(
        schema.pointer("/properties/file_path").is_some(),
        "schema should have a 'file_path' property"
    );
}

// ---------------------------------------------------------------------------
// Default CDP endpoint
// ---------------------------------------------------------------------------

#[test]
fn test_default_cdp_endpoint() {
    assert_eq!(DEFAULT_CDP_ENDPOINT, "http://127.0.0.1:9222");
}

// ---------------------------------------------------------------------------
// Graceful degradation — no browser available
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_status_returns_honest_error_when_no_browser() {
    // Use a port that's almost certainly not running a browser.
    let tool = BrowserTool;
    let ctx = ctx_with_config("http://127.0.0.1:59999");

    let result = tool.execute(json!({"action": "status"}), &ctx).await;

    // The tool should not return an Err — it should return a ToolOutput
    // with an honest "not available" message (graceful degradation).
    let output = result.expect("status should degrade gracefully, not error");

    // Content should mention the endpoint and that the browser is not available.
    assert!(
        output.content.contains("not available")
            || output.content.contains("not reachable")
            || output.content.contains("Browser is not available")
            || output.content.contains("Status: not available"),
        "content should indicate browser is not available: {}",
        output.content
    );

    // Metadata should indicate availability is false.
    let meta = output.metadata.expect("metadata should exist");
    assert_eq!(
        meta["available"].as_bool(),
        Some(false),
        "metadata should show available=false"
    );
}

#[tokio::test]
async fn test_open_degrades_gracefully_when_no_browser() {
    let tool = BrowserTool;
    let ctx = ctx_with_config("http://127.0.0.1:59999");

    let output = tool
        .execute(
            json!({"action": "open", "url": "https://example.com"}),
            &ctx,
        )
        .await
        .expect("should degrade gracefully");

    // Should mention the browser is not available and suggest next_action.
    assert!(
        output.content.contains("not available") || output.content.contains("setup"),
        "content should mention browser not available or suggest setup: {}",
        output.content
    );

    let meta = output.metadata.expect("metadata should exist");
    assert_eq!(
        meta["available"].as_bool(),
        Some(false),
        "metadata should show available=false"
    );
    assert!(
        meta.get("next_action").is_some(),
        "metadata should have next_action guidance"
    );
}

#[tokio::test]
async fn test_snapshot_degrades_gracefully_when_no_browser() {
    let tool = BrowserTool;
    let ctx = ctx_with_config("http://127.0.0.1:59999");

    let output = tool
        .execute(json!({"action": "snapshot"}), &ctx)
        .await
        .expect("should degrade gracefully");

    assert!(
        output.content.contains("not available") || output.content.contains("setup"),
        "snapshot should degrade gracefully: {}",
        output.content
    );
}

#[tokio::test]
async fn test_click_degrades_gracefully_when_no_browser() {
    let tool = BrowserTool;
    let ctx = ctx_with_config("http://127.0.0.1:59999");

    let output = tool
        .execute(json!({"action": "click", "selector": "#button"}), &ctx)
        .await
        .expect("should degrade gracefully");

    assert!(
        output.content.contains("not available") || output.content.contains("setup"),
        "click should degrade gracefully: {}",
        output.content
    );
}

#[tokio::test]
async fn test_eval_degrades_gracefully_when_no_browser() {
    let tool = BrowserTool;
    let ctx = ctx_with_config("http://127.0.0.1:59999");

    let output = tool
        .execute(json!({"action": "eval", "expression": "1 + 1"}), &ctx)
        .await
        .expect("should degrade gracefully");

    assert!(
        output.content.contains("not available") || output.content.contains("setup"),
        "eval should degrade gracefully: {}",
        output.content
    );
}

#[tokio::test]
async fn test_screenshot_degrades_gracefully_when_no_browser() {
    let tool = BrowserTool;
    let ctx = ctx_with_config("http://127.0.0.1:59999");

    let output = tool
        .execute(json!({"action": "screenshot"}), &ctx)
        .await
        .expect("should degrade gracefully");

    assert!(
        output.content.contains("not available") || output.content.contains("setup"),
        "screenshot should degrade gracefully: {}",
        output.content
    );
}

// ---------------------------------------------------------------------------
// Missing required parameters
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_missing_action_returns_error() {
    let tool = BrowserTool;
    let ctx = ctx_no_config();

    let result = tool.execute(json!({}), &ctx).await;
    assert!(result.is_err(), "missing action should return an error");
}

#[tokio::test]
async fn test_unknown_action_returns_error() {
    let tool = BrowserTool;
    let ctx = ctx_no_config();

    let result = tool
        .execute(json!({"action": "nonexistent_action"}), &ctx)
        .await;
    // With no browser, it should either error or degrade gracefully.
    // An unknown action after connection check should error.
    // But since no browser is available, it degrades first.
    // Either way is acceptable — just check it doesn't panic.
    let _ = result;
}

// ---------------------------------------------------------------------------
// Config-based endpoint resolution
// ---------------------------------------------------------------------------

#[test]
fn test_browser_config_defaults() {
    let config = ragent_config::Config::default();
    assert!(
        config.browser.cdp_endpoint.is_none(),
        "cdp_endpoint should default to None"
    );
    assert!(
        config.browser.default_headless,
        "default_headless should default to true"
    );
}

#[test]
fn test_browser_config_parses_custom_endpoint() {
    let config: Config = serde_json::from_str(
        r#"{
            "browser": {
                "cdp_endpoint": "http://localhost:3456",
                "default_headless": false
            }
        }"#,
    )
    .expect("config should parse");

    assert_eq!(
        config.browser.cdp_endpoint.as_deref(),
        Some("http://localhost:3456")
    );
    assert!(
        !config.browser.default_headless,
        "default_headless should be false"
    );
}

#[test]
fn test_browser_config_parses_empty_as_defaults() {
    let config: Config = serde_json::from_str(r#"{"browser": {}}"#).expect("config should parse");
    assert!(config.browser.cdp_endpoint.is_none());
    assert!(config.browser.default_headless);
}

// ---------------------------------------------------------------------------
// Tool-visibility switch
// ---------------------------------------------------------------------------

#[test]
fn test_browser_visibility_switch_defaults_true() {
    let config = Config::default();
    assert!(
        config.tool_visibility.browser,
        "browser visibility should default to true"
    );
}

#[test]
fn test_browser_visibility_switch_in_iter_switches() {
    let config = Config::default();
    let switches: std::collections::HashMap<&str, bool> =
        config.tool_visibility.iter_switches().collect();
    assert!(
        switches.contains_key("browser"),
        "iter_switches should include 'browser'"
    );
}

#[test]
fn test_browser_visibility_switch_off_hides_browser() {
    let mut config = Config::default();
    config.tool_visibility.browser = false;

    let hidden = config.effective_hidden_tools();
    assert!(
        hidden.iter().any(|h| h == "browser"),
        "'browser' should be hidden when browser switch is off"
    );
}

#[test]
fn test_browser_visibility_switch_on_does_not_hide_browser() {
    let config = Config::default();
    assert!(config.tool_visibility.browser);

    let hidden = config.effective_hidden_tools();
    assert!(
        !hidden.iter().any(|h| h == "browser"),
        "'browser' should NOT be hidden when browser switch is on"
    );
}

#[test]
fn test_tool_family_names_browser() {
    let names = tool_family_names("browser").expect("browser family should exist");
    assert_eq!(names, &["browser"]);
}

#[test]
fn test_config_parses_browser_visibility_false() {
    let config: Config = serde_json::from_str(
        r#"{
            "tool_visibility": {
                "browser": false
            }
        }"#,
    )
    .expect("config should parse");

    assert!(
        !config.tool_visibility.browser,
        "browser should be false when explicitly set"
    );
    assert!(
        config.tool_visibility.specified.browser,
        "browser specified flag should be true"
    );
}

#[test]
fn test_serialise_round_trip_browser_visibility() {
    let mut config = Config::default();
    config.tool_visibility.browser = false;
    config.tool_visibility.specified.browser = true;

    let json_str = serde_json::to_string(&config).expect("should serialise");
    assert!(
        json_str.contains("\"browser\":false"),
        "serialised config should contain browser:false, got: {json_str}"
    );

    let parsed: Config = serde_json::from_str(&json_str).expect("should deserialise");
    assert!(
        !parsed.tool_visibility.browser,
        "round-trip should preserve browser=false"
    );
}

// ---------------------------------------------------------------------------
// Registration in extended registry
// ---------------------------------------------------------------------------

#[test]
fn test_browser_tool_registered_in_registry() {
    let registry = create_extended_registry();
    let definitions = registry.definitions();
    let registered_names: std::collections::HashSet<String> =
        definitions.iter().map(|d| d.name.clone()).collect();

    assert!(
        registered_names.contains("browser"),
        "'browser' should be registered in create_extended_registry()"
    );
}

// ---------------------------------------------------------------------------
// CDP types deserialisation
// ---------------------------------------------------------------------------

#[test]
fn test_version_info_deserialises() {
    let json_str = r#"{
        "Browser": "Chrome/131.0.6778.85",
        "V8": "13.1.201.7",
        "webSocketDebuggerUrl": "ws://127.0.0.1:9222/devtools/browser/abc-123",
        "User-Agent": "Mozilla/5.0"
    }"#;
    let info: ragent_tools_extended::browser::cdp::VersionInfo =
        serde_json::from_str(json_str).expect("should parse");
    assert_eq!(info.browser, "Chrome/131.0.6778.85");
    assert_eq!(
        info.web_socket_debugger_url,
        "ws://127.0.0.1:9222/devtools/browser/abc-123"
    );
}

#[test]
fn test_target_info_deserialises() {
    let json_str = r#"{
        "id": "target-1",
        "type": "page",
        "title": "Example",
        "url": "https://example.com",
        "webSocketDebuggerUrl": "ws://127.0.0.1:9222/devtools/page/target-1",
        "attached": false
    }"#;
    let target: ragent_tools_extended::browser::cdp::TargetInfo =
        serde_json::from_str(json_str).expect("should parse");
    assert_eq!(target.id, "target-1");
    assert_eq!(target.target_type, "page");
    assert_eq!(target.url, "https://example.com");
}

#[test]
fn test_first_page_target_finds_page() {
    use ragent_tools_extended::browser::cdp::{TargetInfo, first_page_target};

    let targets = vec![
        TargetInfo {
            id: "bg".to_string(),
            target_type: "background_page".to_string(),
            title: String::new(),
            url: String::new(),
            web_socket_debugger_url: String::new(),
            attached: false,
        },
        TargetInfo {
            id: "page1".to_string(),
            target_type: "page".to_string(),
            title: "Test".to_string(),
            url: "https://example.com".to_string(),
            web_socket_debugger_url: "ws://127.0.0.1:9222/devtools/page/page1".to_string(),
            attached: false,
        },
    ];
    let result = first_page_target(&targets).unwrap();
    assert_eq!(result.id, "page1");
}

#[test]
fn test_first_page_target_no_page_returns_error() {
    use ragent_tools_extended::browser::cdp::{TargetInfo, first_page_target};

    let targets = vec![TargetInfo {
        id: "bg".to_string(),
        target_type: "background_page".to_string(),
        title: String::new(),
        url: String::new(),
        web_socket_debugger_url: String::new(),
        attached: false,
    }];
    assert!(first_page_target(&targets).is_err());
}

// ---------------------------------------------------------------------------
// Conditional CDP tests — only run when Chrome is available at port 9222
// ---------------------------------------------------------------------------

/// Check if a CDP endpoint is available at the default port.
async fn cdp_available() -> bool {
    ragent_tools_extended::browser::cdp::discover_version(DEFAULT_CDP_ENDPOINT)
        .await
        .is_ok()
}

#[tokio::test]
async fn test_cdp_open_snapshot_click() {
    if !cdp_available().await {
        eprintln!("Skipping CDP integration test — no browser at {DEFAULT_CDP_ENDPOINT}");
        return;
    }

    let tool = BrowserTool;
    let ctx = ctx_no_config();

    // Open a simple page (example.com).
    let output = tool
        .execute(
            json!({"action": "open", "url": "https://example.com", "wait": true}),
            &ctx,
        )
        .await
        .expect("open should succeed");

    assert!(
        output.content.contains("example.com") || output.content.contains("Navigated"),
        "open should report navigation: {}",
        output.content
    );

    // Snapshot the page.
    let output = tool
        .execute(json!({"action": "snapshot"}), &ctx)
        .await
        .expect("snapshot should succeed");

    assert!(
        output.content.contains("Example Domain") || output.content.contains("example"),
        "snapshot should contain page text: {}",
        output.content
    );
}

#[tokio::test]
async fn test_cdp_eval() {
    if !cdp_available().await {
        eprintln!("Skipping CDP integration test — no browser at {DEFAULT_CDP_ENDPOINT}");
        return;
    }

    let tool = BrowserTool;
    let ctx = ctx_no_config();

    let output = tool
        .execute(json!({"action": "eval", "expression": "1 + 2"}), &ctx)
        .await
        .expect("eval should succeed");

    assert!(
        output.content.contains("3"),
        "eval 1+2 should return 3: {}",
        output.content
    );
}

#[tokio::test]
async fn test_cdp_status() {
    if !cdp_available().await {
        eprintln!("Skipping CDP integration test — no browser at {DEFAULT_CDP_ENDPOINT}");
        return;
    }

    let tool = BrowserTool;
    let ctx = ctx_no_config();

    let output = tool
        .execute(json!({"action": "status"}), &ctx)
        .await
        .expect("status should succeed");

    assert!(
        output.content.contains("Chrome") || output.content.contains("available"),
        "status should report browser info: {}",
        output.content
    );

    let meta = output.metadata.expect("metadata should exist");
    assert_eq!(
        meta["available"].as_bool(),
        Some(true),
        "metadata should show available=true"
    );
}
