//! FR-014's transmit side across every declared profile (T-017).
//!
//! The canonical `ContentPart::ToolResult` carries an `is_error` flag derived
//! from the recorded [`ToolCallStatus`]. Where a family's declared profile
//! transmits that flag natively the flag reaches the wire and the content is
//! untouched; where it does not, the content takes the literal `ERROR: ` prefix
//! and nothing else changes.
//!
//! The per-carrier half of that rule is pinned in
//! `test_wire_encode_message.rs`, one case per carrier. This file adds the three
//! things that file cannot see:
//!
//! 1. **The marking decision is the profile's, stated once.** Every format in
//!    `ToolFormat::ALL` answers `transmits_is_error_natively` with its declared
//!    message profile's own field, so no dialect can hold a second opinion.
//! 2. **A dialect that assembles its own `messages` array still marks.** Local
//!    Ollama adds a `tool_name` key the OpenAI `role:"tool"` shape has no place
//!    for, so it cannot use the carrier; before this task it therefore sent a
//!    failed tool result with no marker at all - the one path on which an error
//!    was indistinguishable from a success.
//! 3. **The marker is exactly six bytes.** `ERROR: ` and not `ERROR:`: the space
//!    is part of the literal FR-014 names, and the model reads the text.
//!
//! [`ToolCallStatus`]: ragent_types::message::ToolCallStatus

use std::collections::HashMap;
use std::pin::Pin;
use std::sync::Arc;

use futures::StreamExt;
use ragent_llm::llm::{ChatContent, ChatMessage, ChatRequest, ContentPart, StreamEvent};
use ragent_llm::provider::tool_cache::{ToolFormat, cached_tools};
use ragent_llm::provider::wire::encode::{
    ERROR_PREFIX, MessageContext, encode_message, error_marked, tool_names_by_id,
};
use ragent_llm::provider::wire::profile_table::{OLLAMA_NATIVE, OPENAI_CHAT};
use ragent_llm::{OllamaProvider, Provider};
use ragent_types::ToolDefinition;
use serde_json::{Value, json};
use tokio::io::AsyncWriteExt;
use tokio::net::TcpListener;

/// A canonical tool result answering `call_ab12`.
fn tool_result(is_error: bool, text: &str) -> ContentPart {
    ContentPart::ToolResult {
        tool_use_id: "call_ab12".to_string(),
        content: Arc::from(text),
        is_error,
    }
}

fn message(role: &str, parts: Vec<ContentPart>) -> ChatMessage {
    ChatMessage {
        role: role.to_string(),
        content: ChatContent::Parts(parts),
    }
}

/// The canonical pair a carrier needs its id-to-name map from.
fn conversation() -> Vec<ChatMessage> {
    vec![
        ChatMessage {
            role: "user".to_string(),
            content: ChatContent::Text("read notes.txt".to_string()),
        },
        message(
            "assistant",
            vec![ContentPart::ToolUse {
                id: "call_ab12".to_string(),
                name: "Read".to_string(),
                input: json!({ "path": "notes.txt" }),
            }],
        ),
        message("user", vec![tool_result(false, "alpha")]),
    ]
}

// ---------------------------------------------------------------------------
// 1. The decision is the profile's
// ---------------------------------------------------------------------------

/// Every format answers the marking question with its declared message
/// profile's `supports_is_error`, and nothing else.
#[test]
fn test_every_format_takes_the_marking_rule_from_its_declared_profile() {
    assert_eq!(
        ToolFormat::ALL.len(),
        6,
        "one case per format; a seventh is a contract change and should read the reason"
    );

    for format in ToolFormat::ALL {
        assert_eq!(
            format.transmits_is_error_natively(),
            format.message_profile().supports_is_error,
            "{format:?}: the marking rule is the declared profile's, never a second opinion"
        );
    }

    // The two families that carry the flag, and the four that need the marker.
    // Named explicitly so flipping one profile's field is a red test rather than
    // a silent wire change.
    let native: Vec<ToolFormat> = ToolFormat::ALL
        .iter()
        .copied()
        .filter(|f| f.transmits_is_error_natively())
        .collect();
    assert_eq!(
        native,
        vec![ToolFormat::Anthropic, ToolFormat::Bedrock],
        "Anthropic's `tool_result.is_error` and Converse's `toolResult.status` \
         are the only native flags"
    );
}

/// A format with no native flag prefixes an error, and leaves a success and a
/// non-error alone.
#[test]
fn test_a_format_without_a_native_flag_prefixes_exactly_an_error() {
    for format in ToolFormat::ALL {
        let marked = format.result_content("Permission denied", true);
        let clean = format.result_content("Permission denied", false);

        if format.transmits_is_error_natively() {
            assert_eq!(
                marked, "Permission denied",
                "{format:?} sends the flag itself, so the text must not be edited"
            );
        } else {
            assert_eq!(
                marked,
                format!("{ERROR_PREFIX}Permission denied"),
                "{format:?} has no flag, so the content takes the marker"
            );
        }
        assert_eq!(
            clean, "Permission denied",
            "{format:?}: a successful result is never edited (FR-014)"
        );
    }
}

/// The marker is the six bytes FR-014 names, and `error_marked` is the one
/// place they are added.
#[test]
fn test_the_marker_is_the_literal_fr014_names() {
    assert_eq!(ERROR_PREFIX, "ERROR: ", "the space is part of the literal");
    assert_eq!(error_marked("boom", true), "ERROR: boom");
    assert_eq!(error_marked("boom", false), "boom");
    assert_eq!(
        error_marked("", true),
        "ERROR: ",
        "an empty failure is still an error result, not a blank success"
    );
}

// ---------------------------------------------------------------------------
// 2. A dialect that assembles its own messages array
// ---------------------------------------------------------------------------

/// The shared encoder marks the result for a caller that names its format,
/// even where the carrier it uses is not that format's default one.
///
/// This is the local-Ollama shape: the `role:"tool"` carrier with a
/// `tool_name` key, marked by `ToolFormat::OpenAi`'s rule. (Native Ollama packs
/// its own `messages` array and only the end-to-end request case reaches it.)
#[test]
fn test_a_named_format_marks_the_result_the_encoder_writes() {
    let messages = conversation();
    let names = tool_names_by_id(&messages);
    let ctx = MessageContext {
        tool_names: Some(&names),
        mirror_tool_name: true,
        error_marking: Some(ToolFormat::OpenAi),
        ..MessageContext::default()
    };

    let out = encode_message(
        OPENAI_CHAT,
        &message("tool", vec![tool_result(true, "Permission denied")]),
        &ctx,
    );
    assert_eq!(out[0]["role"], "tool");
    assert_eq!(
        out[0]["tool_name"], "Read",
        "the dialect's extra key survives"
    );
    assert_eq!(
        out[0]["content"], "ERROR: Permission denied",
        "FR-014: a named format without a native flag must still mark the error"
    );

    // The default (no named format) is unchanged: the carrier's own profile
    // decides, which for `OPENAI_CHAT` is also not-native, so the same bytes.
    let plain = MessageContext {
        tool_names: Some(&names),
        mirror_tool_name: true,
        ..MessageContext::default()
    };
    let out = encode_message(
        OPENAI_CHAT,
        &message("tool", vec![tool_result(true, "Permission denied")]),
        &plain,
    );
    assert_eq!(out[0]["content"], "ERROR: Permission denied");

    // The two are one derivation, not two copies that happen to agree: a family
    // whose declared profile carries the flag writes the content untouched on
    // BOTH paths. That is what makes the named format a statement of the rule
    // rather than an override of it.
    let native_ctx = MessageContext {
        error_marking: Some(ToolFormat::Anthropic),
        ..MessageContext::default()
    };
    let out = encode_message(
        OPENAI_CHAT,
        &message("tool", vec![tool_result(true, "Permission denied")]),
        &native_ctx,
    );
    assert_eq!(
        out[0]["content"], "Permission denied",
        "a format with a native flag must not be marked, even on a \
         non-native carrier - so the marking follows the named profile"
    );

    // And a success is not marked on either path.
    let out = encode_message(
        OPENAI_CHAT,
        &message("tool", vec![tool_result(false, "alpha")]),
        &ctx,
    );
    assert_eq!(out[0]["content"], "alpha");

    // Native Ollama's own carrier marks by its own profile, which agrees.
    let out = encode_message(
        OLLAMA_NATIVE,
        &message("user", vec![tool_result(true, "Permission denied")]),
        &ctx,
    );
    assert_eq!(out[0]["content"], "ERROR: Permission denied");
}

// ---------------------------------------------------------------------------
// 4. A dialect that rewrites tool names
// ---------------------------------------------------------------------------

/// A family's definition-time name rewrite does not leak into its messages.
///
/// HuggingFace prefixes every tool name (`t_`) inside its definition envelope,
/// because one endpoint hosts many models and a name collision would make a call
/// ambiguous. Its *messages* are OpenAI's, where a tool call carries no name and
/// a result is keyed by the id the model already returned, so carrying the
/// prefix into the history would edit bytes for no gain and could only break the
/// pairing the model relies on. The encoder therefore reads
/// `message_name_prefix`, not `name_prefix`, and this case pins that.
#[test]
fn test_a_definition_name_rewrite_does_not_reach_the_messages() {
    let tools = [ToolDefinition {
        name: "Read".to_string(),
        description: "Read a file".to_string(),
        parameters: json!({ "type": "object" }),
    }];
    let definitions = cached_tools(ToolFormat::HuggingFace, &tools)
        .openai_tools_array()
        .to_string();
    assert!(
        definitions.contains("t_Read"),
        "the definition envelope carries the rewrite: {definitions}"
    );

    assert_eq!(
        ToolFormat::HuggingFace.message_name_prefix(),
        None,
        "the message layer is OpenAI's, which has no name rewrite"
    );
    for format in ToolFormat::ALL {
        assert_eq!(
            format.message_name_prefix(),
            None,
            "{format:?}: no family rewrites a name inside its message shape"
        );
    }
}

/// The name rewrite still applies where a profile *does* name a tool on a
/// result: a caller that asks for one gets the rewritten name, which is what
/// keeps the request's definitions and the history's names in step.
#[test]
fn test_a_caller_that_names_a_tool_still_gets_the_rewrite_applied() {
    let messages = conversation();
    let names = tool_names_by_id(&messages);
    let ctx = MessageContext {
        tool_names: Some(&names),
        mirror_tool_name: true,
        name_prefix: Some("t_"),
        ..MessageContext::default()
    };
    let out = encode_message(
        OPENAI_CHAT,
        &message("tool", vec![tool_result(true, "Permission denied")]),
        &ctx,
    );
    assert_eq!(out[0]["tool_name"], "Read");
    assert_eq!(
        out[0]["content"], "ERROR: Permission denied",
        "the rewrite is a naming concern and must not touch the marking"
    );

    // The same prefix does rewrite the name inside an assistant tool call, which
    // is the one place the history does carry a name.
    let out = encode_message(
        OPENAI_CHAT,
        &message(
            "assistant",
            vec![ContentPart::ToolUse {
                id: "call_ab12".to_string(),
                name: "Read".to_string(),
                input: json!({}),
            }],
        ),
        &ctx,
    );
    assert_eq!(out[0]["tool_calls"][0]["function"]["name"], "t_Read");
}

// ---------------------------------------------------------------------------
// 3. The provider that cannot use the carrier
// ---------------------------------------------------------------------------
///
/// The request is captured off the wire by a throw-away HTTP server, because
/// the point of the case is the *bytes* the provider sends: the marking happens
/// inside the dialect's message packing, so a unit test of the encoder would
/// not reach it. A successful result in the same request stays untouched, which
/// is what makes this a flag test rather than a "content was rewritten" test.
#[tokio::test]
async fn test_local_ollama_marks_a_failed_result_on_the_wire() {
    let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind");
    let addr = listener.local_addr().expect("addr");
    let url = format!("http://{addr}");

    let (tx, rx) = tokio::sync::oneshot::channel::<Value>();
    tokio::spawn(async move {
        let Ok((mut socket, _)) = listener.accept().await else {
            return;
        };
        let mut buf = vec![0_u8; 8192];
        let n = match tokio::io::AsyncReadExt::read(&mut socket, &mut buf).await {
            Ok(n) => n,
            Err(_) => return,
        };
        let request = String::from_utf8_lossy(&buf[..n]).to_string();
        let header_end = request.find("\r\n\r\n").unwrap_or(request.len());
        let body: Value = serde_json::from_str(&request[header_end + 4..]).unwrap_or(Value::Null);
        let _ = tx.send(body);
        let response =
            b"HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nConnection: close\r\n\r\n";
        let _ = socket.write_all(response).await;
        let _ = socket.shutdown().await;
    });

    let client = OllamaProvider::with_url(&url)
        .create_client("", None, &HashMap::new())
        .await
        .expect("client");

    let messages = vec![
        ChatMessage {
            role: "user".to_string(),
            content: ChatContent::Text("read both files".to_string()),
        },
        message(
            "assistant",
            vec![
                ContentPart::ToolUse {
                    id: "call_ok".to_string(),
                    name: "Read".to_string(),
                    input: json!({ "path": "notes.txt" }),
                },
                ContentPart::ToolUse {
                    id: "call_bad".to_string(),
                    name: "Read".to_string(),
                    input: json!({ "path": "missing.txt" }),
                },
            ],
        ),
        message(
            "user",
            vec![
                ContentPart::ToolResult {
                    tool_use_id: "call_ok".to_string(),
                    content: Arc::from("alpha"),
                    is_error: false,
                },
                ContentPart::ToolResult {
                    tool_use_id: "call_bad".to_string(),
                    content: Arc::from("No such file"),
                    is_error: true,
                },
            ],
        ),
    ];
    let request = ChatRequest {
        model: "ornith:latest".to_string(),
        messages: Arc::new(messages),
        tools: Arc::new(Vec::new()),
        temperature: None,
        top_p: None,
        max_tokens: Some(64),
        system: None,
        options: HashMap::new(),
        session_id: None,
        request_id: None,
        stream_timeout_secs: Some(5),
        thinking: None,
    };

    let mut stream: Pin<Box<dyn futures::Stream<Item = StreamEvent> + Send>> =
        client.chat(request).await.expect("chat");
    while stream.next().await.is_some() {}

    let body = rx.await.expect("captured body");
    let sent = body["messages"]
        .as_array()
        .expect("the dialect writes a messages array")
        .iter()
        .filter(|m| m["role"] == "tool")
        .collect::<Vec<_>>();
    assert_eq!(sent.len(), 2, "one tool message per result");

    let content = |call_id: &str| -> String {
        sent.iter()
            .find(|m| m["tool_call_id"] == call_id)
            .and_then(|m| m["content"].as_str())
            .unwrap_or_default()
            .to_string()
    };
    assert_eq!(
        content("call_bad"),
        "ERROR: No such file",
        "FR-014: local Ollama has no native flag, so the failure carries the marker"
    );
    assert_eq!(
        content("call_ok"),
        "alpha",
        "FR-014: the success in the same request is byte-identical"
    );
}
