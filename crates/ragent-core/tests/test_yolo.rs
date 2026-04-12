//! Tests for the YOLO mode toggle and its effect on validation.

use ragent_core::yolo;
use serial_test::serial;

#[test]
#[serial]
fn test_yolo_default_disabled() {
    // Reset to known state
    yolo::set_enabled(false);
    assert!(!yolo::is_enabled());
}

#[test]
#[serial]
fn test_yolo_enable_disable() {
    yolo::set_enabled(true);
    assert!(yolo::is_enabled());
    yolo::set_enabled(false);
    assert!(!yolo::is_enabled());
}

#[test]
#[serial]
fn test_yolo_toggle() {
    yolo::set_enabled(false);
    let state = yolo::toggle();
    assert!(state, "toggle from false should return true");
    assert!(yolo::is_enabled());

    let state = yolo::toggle();
    assert!(!state, "toggle from true should return false");
    assert!(!yolo::is_enabled());
}

#[test]
#[serial]
fn test_yolo_toggle_idempotent_double() {
    yolo::set_enabled(false);
    yolo::toggle();
    yolo::toggle();
    assert!(
        !yolo::is_enabled(),
        "double toggle returns to original state"
    );
}

// --- Integration: bash denied patterns ---

use ragent_core::event::EventBus;
use ragent_core::tool::{Tool, ToolContext};
use std::path::PathBuf;
use std::sync::Arc;

fn make_ctx() -> ToolContext {
    ToolContext {
        session_id: "test-yolo".to_string(),
        working_dir: PathBuf::from("/tmp"),
        event_bus: Arc::new(EventBus::new(16)),
        storage: None,
        task_manager: None,
        lsp_manager: None,
        active_model: None,
        team_context: None,
        team_manager: None,
    }
}

#[tokio::test]
#[serial]
async fn test_yolo_allows_denied_bash_pattern() {
    yolo::set_enabled(true);

    let tool = ragent_core::tool::bash::BashTool;
    // "echo rm -rf /" contains the denied substring "rm -rf /"
    // In YOLO mode the pattern check should be bypassed.
    let result = tool
        .execute(
            serde_json::json!({"command": "echo rm -rf /", "timeout": 2}),
            &make_ctx(),
        )
        .await;

    let output = result.expect("should not be rejected in YOLO mode");
    assert!(
        !output.content.contains("Command rejected"),
        "YOLO mode should bypass denied pattern check, got: {}",
        output.content
    );

    yolo::set_enabled(false);
}

#[tokio::test]
#[serial]
async fn test_yolo_off_rejects_denied_bash_pattern() {
    yolo::set_enabled(false);

    let tool = ragent_core::tool::bash::BashTool;
    let result = tool
        .execute(
            serde_json::json!({"command": "rm -rf /", "timeout": 2}),
            &make_ctx(),
        )
        .await;

    assert!(
        result.is_err(),
        "should reject denied pattern when YOLO is off"
    );
}

// --- Integration: MCP config validation ---

#[test]
#[serial]
fn test_yolo_allows_mcp_metacharacters() {
    use ragent_core::config::{McpServerConfig, McpTransport};
    use ragent_core::mcp::validate_mcp_config;

    yolo::set_enabled(true);

    let config = McpServerConfig {
        type_: McpTransport::Stdio,
        command: Some("evil; rm -rf /".to_string()),
        args: Vec::new(),
        url: None,
        env: std::collections::HashMap::new(),
        disabled: false,
    };
    let result = validate_mcp_config("test-server", &config);
    assert!(
        result.is_ok(),
        "YOLO should skip MCP validation, got: {:?}",
        result
    );

    yolo::set_enabled(false);
}

#[test]
#[serial]
fn test_yolo_off_rejects_mcp_metacharacters() {
    use ragent_core::config::{McpServerConfig, McpTransport};
    use ragent_core::mcp::validate_mcp_config;

    yolo::set_enabled(false);

    let config = McpServerConfig {
        type_: McpTransport::Stdio,
        command: Some("evil; rm -rf /".to_string()),
        args: Vec::new(),
        url: None,
        env: std::collections::HashMap::new(),
        disabled: false,
    };
    let result = validate_mcp_config("test-server", &config);
    assert!(
        result.is_err(),
        "should reject metacharacters when YOLO is off"
    );
}
