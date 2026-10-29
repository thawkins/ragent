//! Tests for the shared schema-property validator
//! (`ragent_agent::tool::param_schema`) and for its wiring into the
//! sub-agent tool family plus `bg`.
//!
//! Spec `agentssetup` FR-002 / FR-004 / FR-038: a tool that declares
//! `additionalProperties: false` must refuse a call carrying a property its
//! schema does not declare, naming the offending key, rather than ignoring it.
//! FR-002 additionally requires that a *declared* parameter is honoured or
//! refused - never silently dropped.

use std::path::PathBuf;
use std::sync::Arc;

use ragent_agent::event::EventBus;
use ragent_agent::tool::bg::BgTool;
use ragent_agent::tool::cancel_agent::CancelAgentTool;
use ragent_agent::tool::list_agents::ListAgentsTool;
use ragent_agent::tool::new_agent::NewAgentTool;
use ragent_agent::tool::param_schema::{
    closed_property_names, declared_properties, validate_tool_input,
};
use ragent_agent::tool::wait_agents::WaitAgentsTool;
use ragent_agent::tool::{Tool, ToolContext};
use serde_json::json;

/// A `ToolContext` with no services wired in: every tool under test refuses a
/// bad call shape *before* it looks for a manager or service, so the refusal
/// is what the assertion sees.
fn base_ctx() -> ToolContext {
    ToolContext {
        session_id: "parent".to_string(),
        working_dir: PathBuf::from("target/temp"),
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
        cached_team_dir: Arc::new(std::sync::Mutex::new(None)),
        permission_checker: None,
        tool_registry: ToolContext::default_tool_registry(),
        read_timestamps: Arc::new(std::sync::RwLock::new(std::collections::HashMap::new())),
        canonical_cache: Arc::new(ragent_tools_core::CanonicalPathCache::new()),
    }
}

// ---------------------------------------------------------------------------
// The shared validator itself
// ---------------------------------------------------------------------------

#[test]
fn test_closed_property_names_reads_the_declared_set() {
    let schema = json!({
        "type": "object",
        "properties": { "b": { "type": "boolean" }, "a": { "type": "string" } },
        "additionalProperties": false
    });
    let mut names = closed_property_names(&schema).expect("closed schema");
    names.sort();
    assert_eq!(names, vec!["a".to_string(), "b".to_string()]);
}

#[test]
fn test_closed_property_names_is_none_for_an_open_schema() {
    // additionalProperties: true - arbitrary keys are allowed.
    let open = json!({
        "type": "object",
        "properties": { "a": { "type": "string" } },
        "additionalProperties": true
    });
    assert!(closed_property_names(&open).is_none());

    // No properties table at all: nothing to validate against.
    assert!(closed_property_names(&json!({ "type": "object" })).is_none());

    // A non-object schema (a bare string) is not a parameter contract.
    assert!(closed_property_names(&json!("string")).is_none());

    // An object schema that declares properties but no `additionalProperties`
    // and no `type: object` makes no closure promise.
    let untyped = json!({ "properties": { "a": { "type": "string" } } });
    assert!(closed_property_names(&untyped).is_none());
}

#[test]
fn test_validate_tool_input_accepts_declared_keys_only() {
    let schema = json!({
        "type": "object",
        "properties": { "agent": { "type": "string" } },
        "additionalProperties": false
    });

    assert!(validate_tool_input(&schema, &json!({ "agent": "explore" })).is_ok());
    // An empty input carries no unknown key (missing required parameters are
    // each tool's own check, not this validator's).
    assert!(validate_tool_input(&schema, &json!({})).is_ok());
    // A non-object input is not a parameter object; do not invent a refusal.
    assert!(validate_tool_input(&schema, &json!(null)).is_ok());
}

#[test]
fn test_validate_tool_input_names_every_unknown_key() {
    let schema = json!({
        "type": "object",
        "properties": { "agent": { "type": "string" } },
        "additionalProperties": false
    });

    let err = validate_tool_input(
        &schema,
        &json!({ "backgound": true, "model_override": "x" }),
    )
    .expect_err("unknown keys must be refused");
    let message = err.to_string();

    assert!(message.contains("Unknown parameter(s)"), "{message}");
    assert!(message.contains("`backgound`"), "{message}");
    assert!(message.contains("`model_override`"), "{message}");
    // The declared surface is listed so the caller can correct the call.
    assert!(message.contains("`agent`"), "{message}");
    // Keys are sorted, so the message is stable for a given call.
    assert!(
        message.find("backgound") < message.find("model_override"),
        "keys are not sorted: {message}"
    );
}

#[test]
fn test_validate_tool_input_skips_an_open_schema() {
    let open = json!({
        "type": "object",
        "properties": { "agent": { "type": "string" } },
        "additionalProperties": true
    });
    assert!(validate_tool_input(&open, &json!({ "anything": 1 })).is_ok());
}

#[test]
fn test_declared_properties_exposes_the_table() {
    let schema = json!({ "properties": { "a": { "type": "string" } } });
    let props = declared_properties(&schema).expect("properties table");
    assert!(props.contains_key("a"));

    assert!(declared_properties(&json!({ "type": "object" })).is_none());
}

// ---------------------------------------------------------------------------
// Wiring: every tool that declares `additionalProperties: false` refuses an
// undeclared property (FR-004, FR-038), before any service lookup.
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_new_agent_refuses_unknown_property() {
    let tool = NewAgentTool;
    assert!(
        closed_property_names(&tool.parameters_schema()).is_some(),
        "new_agent must declare a closed property set for this test to mean anything"
    );

    let err = tool
        .execute(
            json!({ "agent": "explore", "task": "x", "backgound": true }),
            &base_ctx(),
        )
        .await
        .expect_err("a misspelled `background` must be refused");
    let message = err.to_string();
    assert!(message.contains("backgound"), "{message}");
    // Refused as a bad call shape, not reported as a missing environment.
    assert!(
        !message.contains("AgentManager"),
        "the shape refusal must come first: {message}"
    );
}

#[tokio::test]
async fn test_cancel_agent_refuses_unknown_property() {
    let tool = CancelAgentTool;
    let err = tool
        .execute(json!({ "task_id": "t-1", "force": true }), &base_ctx())
        .await
        .expect_err("an undeclared `force` must be refused");
    assert!(err.to_string().contains("force"), "{err}");
}

#[tokio::test]
async fn test_list_agents_refuses_unknown_property() {
    let tool = ListAgentsTool;
    let err = tool
        .execute(json!({ "staus": "running" }), &base_ctx())
        .await
        .expect_err("a misspelled `status` must be refused");
    assert!(err.to_string().contains("staus"), "{err}");
}

#[tokio::test]
async fn test_wait_agents_refuses_unknown_property() {
    let tool = WaitAgentsTool;
    let err = tool
        .execute(json!({ "task_ids": [], "timeout": 5 }), &base_ctx())
        .await
        .expect_err("`timeout` is `wait_agents`'s neighbour's spelling and must be refused here");
    assert!(err.to_string().contains("timeout"), "{err}");
}

#[tokio::test]
async fn test_bg_refuses_unknown_property() {
    let tool = BgTool;
    let err = tool
        .execute(json!({ "action": "list", "limt": 5 }), &base_ctx())
        .await
        .expect_err("a misspelled `limit` must be refused");
    assert!(err.to_string().contains("limt"), "{err}");
}

// ---------------------------------------------------------------------------
// FR-002: a *declared* parameter is honoured or refused, never ignored.
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_bg_refuses_a_declared_parameter_the_action_does_not_read() {
    let tool = BgTool;

    let err = tool
        .execute(
            json!({ "action": "status", "task_id": "t-1", "lines": 10 }),
            &base_ctx(),
        )
        .await
        .expect_err("`lines` on `status` must be refused, not ignored");
    let message = err.to_string();
    assert!(
        message.contains("not accepted by bg action 'status'"),
        "{message}"
    );
    assert!(message.contains("`lines`"), "{message}");
    assert!(message.contains("`task_id`"), "{message}");

    let err = tool
        .execute(
            json!({ "action": "spawn", "command": "sleep 1", "timeout": 5 }),
            &base_ctx(),
        )
        .await
        .expect_err("`timeout` on `spawn` must be refused, not ignored");
    assert!(err.to_string().contains("'spawn'"), "{err}");
}

#[tokio::test]
async fn test_bg_accepts_every_parameter_its_action_reads() {
    // No bg service in the context, so a well-formed call reaches the service
    // lookup and fails there - proof the parameter check let it through.
    let tool = BgTool;
    let err = tool
        .execute(
            json!({
                "action": "cleanup",
                "older_than_minutes": 30,
                "completed_only": true,
                "session_id": "parent"
            }),
            &base_ctx(),
        )
        .await
        .expect_err("no bg service is wired into this context");
    assert!(
        err.to_string().contains("reason_code: service_unavailable"),
        "{err}"
    );
}

#[tokio::test]
async fn test_wait_agents_refuses_a_non_string_task_id_element() {
    let tool = WaitAgentsTool;
    // An element that is not a string used to be dropped silently, waiting on
    // a different set of tasks than the caller asked for.
    let err = tool
        .execute(json!({ "task_ids": ["ok-id", 7] }), &base_ctx())
        .await
        .expect_err("a non-string task id must be refused");
    let message = err.to_string();
    assert!(message.contains("task_ids[1]"), "{message}");
    assert!(message.contains("expected a string"), "{message}");
}

#[tokio::test]
async fn test_wait_agents_refuses_a_non_array_task_ids() {
    let tool = WaitAgentsTool;
    let err = tool
        .execute(json!({ "task_ids": "t-1" }), &base_ctx())
        .await
        .expect_err("`task_ids` must be an array");
    let message = err.to_string();
    assert!(message.contains("expected an array"), "{message}");
    assert!(message.contains("a string"), "{message}");
}

// ---------------------------------------------------------------------------
// TESTPLAN TC-002: the exact manual-test call shapes, asserted as code so the
// acceptance evidence is reproducible.
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_tc002_new_agent_rejects_the_manual_test_spelling() {
    let tool = NewAgentTool;

    // `backgound` (missing r) - must be named exactly as written.
    let err = tool
        .execute(
            json!({
                "agent": "explore",
                "task": "list the files in src/",
                "backgound": true
            }),
            &base_ctx(),
        )
        .await
        .expect_err("TC-002: `backgound` must be rejected");
    let message = err.to_string();
    assert!(message.contains("backgound"), "TC-002 name: {message}");
    assert!(
        !message.contains("Missing required parameter"),
        "TC-002: the rejection must not be a missing-parameter error: {message}"
    );

    // `timeout_sec` misspelled - the same rejection shape.
    let err = tool
        .execute(
            json!({
                "agent": "explore",
                "task": "list the files in src/",
                "timeout_sec": 60
            }),
            &base_ctx(),
        )
        .await
        .expect_err("TC-002: `timeout_sec` must be rejected with the same shape");
    let message = err.to_string();
    assert!(message.contains("timeout_sec"), "TC-002 name: {message}");
    assert!(
        message.contains("Unknown parameter(s)"),
        "TC-002 shape: {message}"
    );
    assert!(
        !message.contains("Missing required parameter"),
        "TC-002: the rejection must not be a missing-parameter error: {message}"
    );
}

/// `bg` names every refusal a caller could branch on (FR-010): an unknown
/// action, a property the selected action does not read, and a session with no
/// background service wired in.
#[tokio::test]
async fn test_bg_refusals_carry_reason_codes() {
    use ragent_agent::tool::param_schema::reason_in;

    // Unknown action: named with the supported set, not just the bad value.
    // The environment is checked before dispatch, so this case needs a wired
    // service or it would report the missing service instead.
    let storage = ragent_agent::storage::Storage::open_in_memory().expect("storage");
    let _ = storage.create_session("reasons", ".");
    let bus = Arc::new(EventBus::new(16));
    let mut ctx = base_ctx();
    ctx.bg_service = Some(Arc::new(
        ragent_agent::background::BackgroundTaskService::new(Arc::new(storage), bus.clone()),
    ));
    ctx.event_bus = bus;
    let err = BgTool
        .execute(json!({ "action": "teleport" }), &ctx)
        .await
        .expect_err("an unknown action must be refused");
    assert_eq!(reason_in(&err.to_string()), Some("unknown_action"));
    let message = err.to_string();
    assert!(
        message.contains("teleport"),
        "must name the value: {message}"
    );
    assert!(
        message.contains("`spawn`") && message.contains("`cleanup`"),
        "must list the supported actions: {message}"
    );

    // A declared property the action does not read (checked before the
    // environment, so no service is needed here).
    let err = BgTool
        .execute(json!({ "action": "list", "command": "ls" }), &ctx)
        .await
        .expect_err("`command` is not read by `list`");
    assert_eq!(reason_in(&err.to_string()), Some("parameter_not_accepted"));

    // No service: the tool cannot act at all here.
    let err = BgTool
        .execute(json!({ "action": "list" }), &base_ctx())
        .await
        .expect_err("no service must error");
    assert_eq!(reason_in(&err.to_string()), Some("service_unavailable"));
}
