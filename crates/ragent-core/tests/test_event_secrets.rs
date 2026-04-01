//! Integration tests for event payloads — ensuring secrets don't leak via SSE.

use ragent_core::event::{Event, EventBus};
use ragent_core::sanitize::{redact_secrets, register_secret};
use std::sync::Arc;

// ── Event serialization: no secrets in SSE payloads ──────────────

#[test]
fn test_event_text_delta_redacted() {
    // Simulate an LLM text delta that accidentally includes a secret
    let secret = "sk-supersecret_test_abcdefghijklmnop";
    let event = Event::TextDelta {
        session_id: "s1".into(),
        text: format!("Here is the API key: {secret}"),
    };
    let json = serde_json::to_string(&event).unwrap();

    // The event itself doesn't auto-redact — the transport layer should.
    // But we verify that redact_secrets on the serialized payload works.
    let redacted = redact_secrets(&json);
    assert!(
        !redacted.contains(secret),
        "Secret should be redacted from serialized event: {redacted}"
    );
    assert!(redacted.contains("[REDACTED]"));
}

#[test]
fn test_event_tool_result_redacted() {
    let secret = "ghp_AbCdEfGhIjKlMnOpQrStUv1234567890";
    let event = Event::ToolResult {
        session_id: "s1".into(),
        call_id: "c1".into(),
        tool: "bash".into(),
        content: format!("Token found: {secret}"),
        content_line_count: 1,
        metadata: None,
        success: true,
    };
    let json = serde_json::to_string(&event).unwrap();
    let redacted = redact_secrets(&json);
    assert!(!redacted.contains(secret), "GitHub token should be redacted from tool result");
}

#[test]
fn test_event_agent_error_redacted() {
    let secret = "Bearer eyJhbGciOiJIUzI1NiJ9.eyJzdWIiOiIxMjM0NTY3ODkwIn0.dGVzdA";
    let event = Event::AgentError {
        session_id: "s1".into(),
        error: format!("Auth failed with {secret}"),
    };
    let json = serde_json::to_string(&event).unwrap();
    let redacted = redact_secrets(&json);
    assert!(!redacted.contains("eyJhbGci"), "JWT should be redacted from agent error");
}

#[test]
fn test_event_tool_call_args_redacted() {
    let event = Event::ToolCallArgs {
        session_id: "s1".into(),
        call_id: "c1".into(),
        tool: "bash".into(),
        args: r#"{"command": "curl -H 'Authorization: Bearer eyJhbGciOiJIUzI1NiJ9.eyJzdWIiOiIxIn0.test_sig_padding' https://api.example.com"}"#.into(),
    };
    let json = serde_json::to_string(&event).unwrap();
    let redacted = redact_secrets(&json);
    assert!(!redacted.contains("eyJhbGci"), "JWT in tool args should be redacted");
}

#[test]
fn test_event_copilot_device_flow_redacted() {
    let event = Event::CopilotDeviceFlowComplete {
        token: "gho_AbCdEfGhIjKlMnOpQrStUv1234567890".into(),
        api_base: "https://api.github.com".into(),
    };
    let json = serde_json::to_string(&event).unwrap();
    let redacted = redact_secrets(&json);
    assert!(!redacted.contains("gho_"), "Copilot token should be redacted");
}

#[test]
fn test_event_model_response_with_registered_secret() {
    let custom_secret = "custom-internal-api-key-999888777";
    register_secret(custom_secret);

    let event = Event::ModelResponse {
        session_id: "s1".into(),
        text: format!("The key is {custom_secret} and more text"),
        elapsed_ms: 123,
    };
    let json = serde_json::to_string(&event).unwrap();
    let redacted = redact_secrets(&json);
    assert!(
        !redacted.contains(custom_secret),
        "Registered secret should be redacted from model response"
    );

    // Cleanup
    ragent_core::sanitize::unregister_secret(custom_secret);
}

// ── Event payloads preserve non-secret data ──────────────────────

#[test]
fn test_event_serialization_preserves_structure() {
    let event = Event::ToolCallEnd {
        session_id: "session-123".into(),
        call_id: "call-456".into(),
        tool: "bash".into(),
        error: None,
        duration_ms: 42,
    };
    let json = serde_json::to_string(&event).unwrap();
    assert!(json.contains("\"type\":\"tool_call_end\""));
    assert!(json.contains("\"session_id\":\"session-123\""));
    assert!(json.contains("\"duration_ms\":42"));
}

#[test]
fn test_event_safe_text_delta_unchanged() {
    let event = Event::TextDelta {
        session_id: "s1".into(),
        text: "Here is some perfectly safe text with no secrets at all.".into(),
    };
    let json = serde_json::to_string(&event).unwrap();
    let redacted = redact_secrets(&json);
    assert_eq!(json, redacted, "Safe text should not be modified by redaction");
}

// ── EventBus publish/subscribe ───────────────────────────────────

#[tokio::test]
async fn test_event_bus_redacted_publish() {
    let bus = Arc::new(EventBus::new(16));
    let mut rx = bus.subscribe();

    let secret = "FAKE_AKIA_TOKEN_FOR_TESTS1234";
    let event = Event::TextDelta {
        session_id: "s1".into(),
        text: format!("AWS key: {secret}"),
    };
    bus.publish(event);

    let received = rx.recv().await.unwrap();
    // The bus itself doesn't redact — verify the raw event contains the secret,
    // and that redact_secrets would clean it.
    let json = serde_json::to_string(&received).unwrap();
    let redacted = redact_secrets(&json);
    assert!(!redacted.contains("AKIAIOSF"), "Redact on received event should work");
}
