//! Integration tests for [`McpToolWrapper`] and the MCP config-validation
//! helpers in `ragent_agent::mcp`.
//!
//! These tests exercise the wrapper's ragent-name mangling, its `Tool` trait
//! accessors (`name`, `description`, `parameters_schema`, `permission_category`,
//! `mcp_wrapper_info`), and its `execute` refusal paths (a disabled server, an
//! absent server) without ever starting a live MCP process. The config
//! validation edge cases cover the `validate_mcp_config` stdio/HTTP branches.

use std::sync::Arc;

use ragent_agent::mcp::{McpClient, McpStatus, validate_mcp_config};
use ragent_agent::tool::{McpToolWrapper, Tool, ToolContext};
use ragent_config::{McpServerConfig, McpTransport};
use serde_json::json;
use tokio::sync::RwLock;

/// A client holding a single registered-but-disabled server, with no transport.
async fn client_with_disabled_server(id: &str) -> Arc<RwLock<McpClient>> {
    let mut client = McpClient::new();
    client
        .connect(
            id,
            McpServerConfig {
                disabled: true,
                ..McpServerConfig::default()
            },
        )
        .await
        .expect("register disabled server");
    Arc::new(RwLock::new(client))
}

/// A `ToolContext` with no external services, sufficient for wrapper execute.
fn test_ctx() -> ToolContext {
    ToolContext {
        session_id: "test-session".to_string(),
        working_dir: std::path::PathBuf::from("target/temp"),
        event_bus: Arc::new(ragent_types::event::EventBus::new(16)),
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
        cached_team_dir: Arc::new(std::sync::Mutex::new(None)),
        permission_checker: None,
        tool_registry: ToolContext::default_tool_registry(),
        read_timestamps: Arc::new(std::sync::RwLock::new(std::collections::HashMap::new())),
        canonical_cache: Arc::new(ragent_tools_core::CanonicalPathCache::new()),
    }
}

// -- ragent_name_for mangling -------------------------------------------------

#[test]
fn ragent_name_for_mangles_all_separators() {
    // `-`, `.` and `/` are replaced by `_` in both the server and tool segment.
    assert_eq!(
        McpToolWrapper::ragent_name_for("srv-a.b/c", "tool-x.y/z"),
        "mcp_srv_a_b_c_tool_x_y_z"
    );
}

#[test]
fn ragent_name_for_leaves_clean_names_untouched() {
    assert_eq!(
        McpToolWrapper::ragent_name_for("filesystem", "read_file"),
        "mcp_filesystem_read_file"
    );
}

// -- Tool trait accessors -----------------------------------------------------

#[tokio::test]
async fn wrapper_accessors_expose_name_description_and_category() {
    let client = Arc::new(RwLock::new(McpClient::new()));
    let wrapper = McpToolWrapper::new(
        "srv-a",
        "do-thing",
        "Does a thing",
        json!({"type": "object"}),
        client,
    );

    assert_eq!(wrapper.name(), "mcp_srv_a_do_thing");
    assert_eq!(wrapper.description(), "Does a thing");
    assert_eq!(wrapper.permission_category(), "mcp");
}

#[test]
fn wrapper_parameters_schema_defaults_additional_properties_false() {
    let client = Arc::new(RwLock::new(McpClient::new()));
    let wrapper = McpToolWrapper::new(
        "srv",
        "tool",
        "d",
        json!({"type": "object", "properties": {"a": {"type": "string"}}}),
        client,
    );

    let schema = wrapper.parameters_schema();
    assert_eq!(schema["additionalProperties"], json!(false));
    // The original schema is preserved alongside the injected key.
    assert_eq!(schema["properties"]["a"]["type"], "string");
}

#[test]
fn wrapper_parameters_schema_leaves_non_object_alone() {
    let client = Arc::new(RwLock::new(McpClient::new()));
    let wrapper = McpToolWrapper::new("srv", "tool", "d", json!(true), client);

    // A non-object schema has no place to attach `additionalProperties`.
    assert_eq!(wrapper.parameters_schema(), json!(true));
}

#[test]
fn wrapper_mcp_wrapper_info_returns_server_and_tool_pair() {
    let client = Arc::new(RwLock::new(McpClient::new()));
    let wrapper = McpToolWrapper::new(
        "my-server",
        "my-tool",
        "d",
        json!({"type": "object"}),
        client,
    );

    assert_eq!(wrapper.mcp_wrapper_info(), Some(("my-server", "my-tool")));
}

// -- execute refusal paths ----------------------------------------------------

#[tokio::test]
async fn wrapper_execute_on_disabled_server_is_refused() {
    let client = client_with_disabled_server("off-server").await;
    let wrapper = McpToolWrapper::new("off-server", "tool", "d", json!({"type": "object"}), client);

    let err = wrapper
        .execute(json!({}), &test_ctx())
        .await
        .expect_err("a disabled server must refuse the call");
    let message = err.to_string();
    assert!(
        message.contains("is disabled") && message.contains("off-server"),
        "unexpected error: {message}"
    );
}

#[tokio::test]
async fn wrapper_execute_for_absent_server_is_refused_by_the_ledger_probe() {
    // No server is registered for this id: the client holds no record, so the
    // wrapper falls back to its default config probe. A default config is not
    // disabled, so the call is attempted and the client reports it as absent.
    let client = Arc::new(RwLock::new(McpClient::new()));
    let wrapper = McpToolWrapper::new("ghost", "tool", "d", json!({"type": "object"}), client);

    let err = wrapper
        .execute(json!({}), &test_ctx())
        .await
        .expect_err("an absent server cannot be called");
    assert!(
        err.to_string().contains("is not connected")
            || err.to_string().contains("No connected MCP server"),
        "unexpected error: {err}"
    );
}

// -- validate_mcp_config edge cases -------------------------------------------

#[test]
fn validate_stdio_requires_a_command() {
    let config = McpServerConfig {
        type_: McpTransport::Stdio,
        command: None,
        ..McpServerConfig::default()
    };
    let err = validate_mcp_config("srv", &config).expect_err("missing command must fail");
    assert!(err.to_string().contains("requires a 'command'"));
}

#[test]
fn validate_stdio_rejects_empty_command() {
    let config = McpServerConfig {
        type_: McpTransport::Stdio,
        command: Some("   ".to_string()),
        ..McpServerConfig::default()
    };
    let err = validate_mcp_config("srv", &config).expect_err("blank command must fail");
    assert!(err.to_string().contains("must not be empty"));
}

#[test]
fn validate_stdio_rejects_shell_metacharacters() {
    let config = McpServerConfig {
        type_: McpTransport::Stdio,
        command: Some("echo; rm -rf /".to_string()),
        ..McpServerConfig::default()
    };
    let err = validate_mcp_config("srv", &config).expect_err("metacharacter must fail");
    assert!(err.to_string().contains("disallowed shell metacharacter"));
}

#[test]
fn validate_stdio_rejects_a_missing_command_path() {
    let config = McpServerConfig {
        type_: McpTransport::Stdio,
        command: Some("/nonexistent/path/to/mcp-server".to_string()),
        ..McpServerConfig::default()
    };
    let err = validate_mcp_config("srv", &config).expect_err("missing path must fail");
    assert!(err.to_string().contains("does not exist"));
}

#[test]
fn validate_http_requires_a_url() {
    let config = McpServerConfig {
        type_: McpTransport::Http,
        url: None,
        ..McpServerConfig::default()
    };
    let err = validate_mcp_config("srv", &config).expect_err("missing url must fail");
    assert!(err.to_string().contains("requires a 'url'"));
}

#[test]
fn validate_http_rejects_a_non_http_scheme() {
    let config = McpServerConfig {
        type_: McpTransport::Http,
        url: Some("ftp://example.invalid".to_string()),
        ..McpServerConfig::default()
    };
    let err = validate_mcp_config("srv", &config).expect_err("non-http url must fail");
    assert!(err.to_string().contains("must start with http://"));
}

#[test]
fn validate_http_accepts_a_well_formed_url() {
    let config = McpServerConfig {
        type_: McpTransport::Http,
        url: Some("https://example.invalid/mcp".to_string()),
        ..McpServerConfig::default()
    };
    validate_mcp_config("srv", &config).expect("a valid https url must pass");
}

// -- server list lifecycle ----------------------------------------------------

#[tokio::test]
async fn disconnect_marks_a_registered_server_disabled() {
    let mut client = McpClient::new();
    client
        .connect(
            "srv-a",
            McpServerConfig {
                disabled: true,
                ..McpServerConfig::default()
            },
        )
        .await
        .expect("register disabled server");
    assert_eq!(client.servers().len(), 1);
    assert_eq!(client.servers()[0].status, McpStatus::Disabled);

    // Disconnecting a server with no live connection is a no-op that still
    // succeeds, and the server entry remains listed as disabled.
    client
        .disconnect("srv-a")
        .await
        .expect("disconnect must succeed");
    assert_eq!(client.servers()[0].status, McpStatus::Disabled);
}

#[test]
fn mcp_status_display_covers_every_variant() {
    assert_eq!(McpStatus::Connected.to_string(), "connected");
    assert_eq!(McpStatus::Disabled.to_string(), "disabled");
    assert_eq!(McpStatus::NeedsAuth.to_string(), "needs auth");
    assert_eq!(
        McpStatus::Failed {
            error: "boom".to_string()
        }
        .to_string(),
        "failed: boom"
    );
}
