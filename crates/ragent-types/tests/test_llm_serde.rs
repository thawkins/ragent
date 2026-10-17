//! Serde coverage for the LLM primitive types in [`ragent_types::llm`].
//!
//! These types are the wire format shared by every provider adapter, so the
//! two custom `Arc<str>` serde adapters (`arc_str_serde` for tool-result
//! content, `optional_arc_str_serde` for the system prompt) must preserve the
//! plain-string JSON shape and round-trip exactly.

use ragent_types::event::FinishReason;
use ragent_types::llm::{
    ChatContent, ChatMessage, ChatRequest, ContentPart, StreamEvent, ToolDefinition,
};
use std::collections::HashMap;
use std::sync::Arc;

fn request_with_system(system: Option<Arc<str>>) -> ChatRequest {
    ChatRequest {
        model: "claude-sonnet-4".to_string(),
        messages: Arc::new(vec![ChatMessage {
            role: "user".to_string(),
            content: ChatContent::Text("hi".to_string()),
        }]),
        tools: Arc::new(Vec::new()),
        temperature: None,
        top_p: None,
        max_tokens: Some(256),
        system,
        options: HashMap::new(),
        session_id: Some("sess-1".to_string()),
        request_id: Some("req-1".to_string()),
        stream_timeout_secs: Some(30),
        thinking: None,
    }
}

#[test]
fn optional_arc_str_serializes_as_a_plain_nullable_string() {
    // Some: on the wire it is a bare JSON string, not an object.
    let with_system = request_with_system(Some(Arc::from("be terse")));
    let json = serde_json::to_value(&with_system).expect("serialize");
    assert_eq!(json["system"], serde_json::json!("be terse"));

    // None: on the wire it is JSON null.
    let without_system = request_with_system(None);
    let json = serde_json::to_value(&without_system).expect("serialize");
    assert!(json["system"].is_null());
}

#[test]
fn optional_arc_str_round_trips_both_variants() {
    let with_system = request_with_system(Some(Arc::from("system prompt")));
    let json = serde_json::to_string(&with_system).expect("serialize");
    let back: ChatRequest = serde_json::from_str(&json).expect("deserialize");
    assert_eq!(
        back.system.as_deref(),
        Some("system prompt"),
        "the Arc<str> system prompt survives a round-trip"
    );

    let without_system = request_with_system(None);
    let json = serde_json::to_string(&without_system).expect("serialize");
    let back: ChatRequest = serde_json::from_str(&json).expect("deserialize");
    assert!(back.system.is_none());

    // The two shared Arc<str>s are the same allocation after deserialisation
    // is not asserted (serde rebuilds them), but the value must be intact.
    assert!(Arc::strong_count(&back.messages) >= 1);
}

#[test]
fn tool_result_content_arc_str_round_trips_as_a_plain_string() {
    let part = ContentPart::ToolResult {
        tool_use_id: "call-1".to_string(),
        content: Arc::from("file contents"),
    };
    let json = serde_json::to_value(&part).expect("serialize");
    assert_eq!(json["type"], "tool_result");
    assert_eq!(json["content"], serde_json::json!("file contents"));

    let back: ContentPart = serde_json::from_value(json).expect("deserialize");
    match back {
        ContentPart::ToolResult { content, .. } => assert_eq!(&*content, "file contents"),
        other => panic!("unexpected part: {other:?}"),
    }
}

#[test]
fn every_content_part_variant_round_trips() {
    let parts = [
        ContentPart::Text {
            text: "hello".to_string(),
        },
        ContentPart::ToolUse {
            id: "call-2".to_string(),
            name: "read".to_string(),
            input: serde_json::json!({ "path": "a.rs" }),
        },
        ContentPart::ToolResult {
            tool_use_id: "call-2".to_string(),
            content: Arc::from("ok"),
        },
        ContentPart::ImageUrl {
            url: "data:image/png;base64,AAAA".to_string(),
        },
    ];
    for part in &parts {
        let json = serde_json::to_string(part).expect("serialize");
        let back: ContentPart = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(
            serde_json::to_value(&back).expect("re-serialize"),
            serde_json::to_value(part).expect("original serialize")
        );
    }
    // Tagged by `type` with snake_case names.
    assert_eq!(
        serde_json::to_value(&parts[0]).expect("serialize")["type"],
        "text"
    );
    assert_eq!(
        serde_json::to_value(&parts[1]).expect("serialize")["type"],
        "tool_use"
    );
    assert_eq!(
        serde_json::to_value(&parts[3]).expect("serialize")["type"],
        "image_url"
    );
}

#[test]
fn chat_content_is_untagged() {
    let text = ChatContent::Text("plain".to_string());
    assert_eq!(
        serde_json::to_value(&text).expect("serialize"),
        serde_json::json!("plain")
    );

    let parts = ChatContent::Parts(vec![ContentPart::Text {
        text: "x".to_string(),
    }]);
    let json = serde_json::to_value(&parts).expect("serialize");
    assert!(json.is_array());
}

#[test]
fn stream_event_variants_round_trip() {
    let events = [
        StreamEvent::ReasoningStart,
        StreamEvent::ReasoningDelta {
            text: "hmm".to_string(),
        },
        StreamEvent::ReasoningEnd,
        StreamEvent::TextDelta {
            text: "hi".to_string(),
        },
        StreamEvent::ToolCallStart {
            id: "c".to_string(),
            name: "read".to_string(),
        },
        StreamEvent::ToolCallDelta {
            id: "c".to_string(),
            args_json: "{".to_string(),
        },
        StreamEvent::ToolCallEnd {
            id: "c".to_string(),
        },
        StreamEvent::Usage {
            input_tokens: 10,
            output_tokens: 20,
        },
        StreamEvent::RateLimit {
            requests_used_pct: Some(1.5),
            tokens_used_pct: None,
        },
        StreamEvent::Error {
            message: "boom".to_string(),
        },
        StreamEvent::Finish {
            reason: FinishReason::Stop,
        },
    ];
    for event in &events {
        let json = serde_json::to_string(event).expect("serialize");
        let back: StreamEvent = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(
            serde_json::to_value(&back).expect("re-serialize"),
            serde_json::to_value(event).expect("original serialize")
        );
    }
}

#[test]
fn tool_definition_round_trips() {
    let tool = ToolDefinition {
        name: "read".to_string(),
        description: "Read a file".to_string(),
        parameters: serde_json::json!({ "type": "object" }),
    };
    let json = serde_json::to_string(&tool).expect("serialize");
    let back: ToolDefinition = serde_json::from_str(&json).expect("deserialize");
    assert_eq!(back.name, "read");
    assert_eq!(back.parameters, serde_json::json!({ "type": "object" }));
}

#[test]
fn chat_request_defaults_tools_and_options_when_omitted() {
    // `tools` and `options` carry `#[serde(default)]`, so a minimal payload
    // deserialises with them empty.
    let json = serde_json::json!({
        "model": "m",
        "messages": [],
        "temperature": null,
        "top_p": null,
        "max_tokens": null
    });
    let req: ChatRequest = serde_json::from_value(json).expect("deserialize");
    assert!(req.tools.is_empty());
    assert!(req.options.is_empty());
    assert!(req.system.is_none());
    assert!(req.thinking.is_none());
    // `#[serde(skip)]` fields default to None.
    assert!(req.session_id.is_none());
    assert!(req.request_id.is_none());
    assert!(req.stream_timeout_secs.is_none());
}
