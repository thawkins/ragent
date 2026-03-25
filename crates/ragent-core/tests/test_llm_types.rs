//! Tests for test_llm_types.rs

use ragent_core::llm::*;
use serde_json::json;

// ── StreamEvent serde ────────────────────────────────────────────

#[test]
fn test_stream_event_serde_all_variants() {
    let events = vec![
        StreamEvent::ReasoningStart,
        StreamEvent::ReasoningDelta {
            text: "thinking...".into(),
        },
        StreamEvent::ReasoningEnd,
        StreamEvent::TextDelta {
            text: "hello".into(),
        },
        StreamEvent::ToolCallStart {
            id: "call-1".into(),
            name: "read".into(),
        },
        StreamEvent::ToolCallDelta {
            id: "call-1".into(),
            args_json: r#"{"path":"test"}"#.into(),
        },
        StreamEvent::ToolCallEnd {
            id: "call-1".into(),
        },
        StreamEvent::Usage {
            input_tokens: 100,
            output_tokens: 50,
        },
        StreamEvent::Error {
            message: "rate limit".into(),
        },
        StreamEvent::Finish {
            reason: ragent_core::event::FinishReason::Stop,
        },
    ];

    for event in &events {
        let json = serde_json::to_string(event).unwrap();
        let deserialized: StreamEvent = serde_json::from_str(&json).unwrap();
        let json2 = serde_json::to_string(&deserialized).unwrap();
        assert_eq!(json, json2, "StreamEvent roundtrip failed for: {:?}", event);
    }
}

// ── ChatRequest serde ────────────────────────────────────────────

#[test]
fn test_chat_request_serde() {
    let request = ChatRequest {
        model: "claude-sonnet-4-20250514".into(),
        messages: vec![
            ChatMessage {
                role: "user".into(),
                content: ChatContent::Text("Hello!".into()),
            },
            ChatMessage {
                role: "assistant".into(),
                content: ChatContent::Text("Hi there!".into()),
            },
        ],
        tools: vec![ToolDefinition {
            name: "read".into(),
            description: "Read a file".into(),
            parameters: json!({"type": "object", "properties": {"path": {"type": "string"}}}),
        }],
        temperature: Some(0.7),
        top_p: None,
        max_tokens: Some(4096),
        system: Some("You are helpful.".into()),
        options: Default::default(),
    };

    let json = serde_json::to_string(&request).unwrap();
    let deserialized: ChatRequest = serde_json::from_str(&json).unwrap();

    assert_eq!(deserialized.model, "claude-sonnet-4-20250514");
    assert_eq!(deserialized.messages.len(), 2);
    assert_eq!(deserialized.tools.len(), 1);
    assert_eq!(deserialized.temperature, Some(0.7));
    assert_eq!(deserialized.max_tokens, Some(4096));
    assert_eq!(deserialized.system.as_deref(), Some("You are helpful."));
}

// ── ChatContent variants ─────────────────────────────────────────

#[test]
fn test_chat_content_text() {
    let content = ChatContent::Text("simple text".into());
    let json = serde_json::to_string(&content).unwrap();
    assert_eq!(json, r#""simple text""#);
}

#[test]
fn test_chat_content_parts() {
    let content = ChatContent::Parts(vec![
        ContentPart::Text {
            text: "before tool".into(),
        },
        ContentPart::ToolUse {
            id: "call-1".into(),
            name: "read".into(),
            input: json!({"path": "test.rs"}),
        },
        ContentPart::ToolResult {
            tool_use_id: "call-1".into(),
            content: "file contents".into(),
        },
    ]);

    let json = serde_json::to_string(&content).unwrap();
    let deserialized: ChatContent = serde_json::from_str(&json).unwrap();

    match deserialized {
        ChatContent::Parts(parts) => {
            assert_eq!(parts.len(), 3);
            match &parts[0] {
                ContentPart::Text { text } => assert_eq!(text, "before tool"),
                _ => panic!("Expected Text"),
            }
            match &parts[1] {
                ContentPart::ToolUse { id, name, input } => {
                    assert_eq!(id, "call-1");
                    assert_eq!(name, "read");
                    assert_eq!(input["path"], "test.rs");
                }
                _ => panic!("Expected ToolUse"),
            }
            match &parts[2] {
                ContentPart::ToolResult {
                    tool_use_id,
                    content,
                } => {
                    assert_eq!(tool_use_id, "call-1");
                    assert_eq!(content, "file contents");
                }
                _ => panic!("Expected ToolResult"),
            }
        }
        _ => panic!("Expected Parts variant"),
    }
}

// ── ChatMessage ──────────────────────────────────────────────────

#[test]
fn test_chat_message_serde() {
    let msg = ChatMessage {
        role: "user".into(),
        content: ChatContent::Text("What is Rust?".into()),
    };

    let json = serde_json::to_string(&msg).unwrap();
    let deserialized: ChatMessage = serde_json::from_str(&json).unwrap();
    assert_eq!(deserialized.role, "user");
    match deserialized.content {
        ChatContent::Text(text) => assert_eq!(text, "What is Rust?"),
        _ => panic!("Expected Text"),
    }
}

// ── ToolDefinition ───────────────────────────────────────────────

#[test]
fn test_tool_definition_serde() {
    let def = ToolDefinition {
        name: "bash".into(),
        description: "Execute a shell command".into(),
        parameters: json!({
            "type": "object",
            "properties": {
                "command": {"type": "string"}
            },
            "required": ["command"]
        }),
    };

    let json = serde_json::to_string(&def).unwrap();
    let deserialized: ToolDefinition = serde_json::from_str(&json).unwrap();
    assert_eq!(deserialized.name, "bash");
    assert_eq!(deserialized.description, "Execute a shell command");
    assert!(deserialized.parameters["required"].is_array());
}

// ── ChatRequest defaults ─────────────────────────────────────────

#[test]
fn test_chat_request_minimal() {
    let json = r#"{
        "model": "test-model",
        "messages": [{"role": "user", "content": "hi"}]
    }"#;

    let request: ChatRequest = serde_json::from_str(json).unwrap();
    assert_eq!(request.model, "test-model");
    assert!(request.tools.is_empty());
    assert!(request.temperature.is_none());
    assert!(request.top_p.is_none());
    assert!(request.max_tokens.is_none());
    assert!(request.system.is_none());
    assert!(request.options.is_empty());
}
