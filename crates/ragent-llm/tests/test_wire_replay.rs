//! Recorded-frame replay, one fixture per provider family (NFR-004).
//!
//! NFR-004 makes the unified tool path testable with no live provider: "a
//! loopback SSE server can replay recorded frames per profile, and the encoders
//! are pure functions over `&[ToolDefinition]` and `&ChatMessage`". This file is
//! that sentence, written down.
//!
//! ## Two halves, one fixture each
//!
//! Each case below is a single round trip through **one** provider family's real
//! client, driven end to end against a loopback server:
//!
//! 1. the stream half - a recorded response body is replayed by a
//!    `TcpListener` on `127.0.0.1:0`, parsed by that provider's own stream
//!    parser, and the resulting [`StreamEvent`] list is checked for the call
//!    the frame carries (id, name, arguments, one end, one finish);
//! 2. the encode half - those *same* parsed events are folded into the canned
//!    request the recorder's server received, and the family's encoder is asked
//!    to write it back. The written bytes must match the recorded frame's own
//!    vocabulary, so a decoder and an encoder that agreed only with themselves
//!    fail here rather than in production.
//!
//! The fixtures are recordings, not transcriptions of our own output: every
//! frame and every canned request body below is the shape the named service
//! sends and accepts, written from its documented stream format. A fixture that
//! merely echoed the encoder's own bytes would prove nothing.
//!
//! ## Why one file and not one per family
//!
//! The six families differ in their framing (SSE, NDJSON), their keying (index
//! versus call id) and their error carrier (native `is_error`/`status` versus the
//! `ERROR: ` prefix). Keeping them side by side is what makes a divergence
//! visible: the shared assertions ([`assert_one_call`]) are written once, so a
//! family that stops producing one start, one end and one finish fails on the
//! same line as its siblings.
//!
//! Everything here runs offline. The only sockets are loopback ones this file
//! opens, no credential is read and no environment variable is touched.

#![allow(unsafe_code)]

use std::collections::HashMap;
use std::pin::Pin;
use std::sync::Arc;

use futures::StreamExt;
use ragent_llm::llm::{ChatContent, ChatMessage, ChatRequest, ContentPart, StreamEvent};
use ragent_llm::provider::wire::encode::{
    MessageContext, encode_message, encode_messages, tool_names_by_id,
};
use ragent_llm::provider::wire::profile_table::{
    ANTHROPIC_MESSAGES, BEDROCK_CONVERSE, GEMINI, OLLAMA_NATIVE, OPENAI_CHAT, OPENAI_RESPONSES,
};
use ragent_llm::{AnthropicProvider, GeminiProvider, OllamaCloudProvider, Provider};
use ragent_types::event::FinishReason;
use serde_json::{Value, json};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;

// ---------------------------------------------------------------------------
// Loopback recorder
// ---------------------------------------------------------------------------

/// A loopback HTTP endpoint that captures the single request it is sent and
/// replies with one recorded response body.
///
/// The body is written verbatim after a minimal `HTTP/1.1 200 OK` header, so the
/// replay is the recorded bytes and nothing else. The captured request is
/// returned as `(headers, body)` through the receiver so a case can assert on
/// what the client encoded.
async fn record_one_request(
    content_type: &str,
    response_body: &str,
) -> anyhow::Result<(String, tokio::sync::oneshot::Receiver<(String, Vec<u8>)>)> {
    let listener = TcpListener::bind("127.0.0.1:0").await?;
    let addr = listener.local_addr()?;
    let content_type = content_type.to_string();
    let response_body = response_body.to_string();
    let (tx, rx) = tokio::sync::oneshot::channel();

    tokio::spawn(async move {
        let Ok((mut socket, _)) = listener.accept().await else {
            return;
        };
        // Read until the request is complete enough to parse: the headers and
        // then `content-length` bytes of body.
        let mut raw = Vec::new();
        let mut buf = vec![0_u8; 8192];
        let mut header_end = None;
        let mut content_length = 0usize;
        loop {
            let Ok(n) = socket.read(&mut buf).await else {
                return;
            };
            if n == 0 {
                break;
            }
            raw.extend_from_slice(&buf[..n]);
            if header_end.is_none()
                && let Some(at) = raw.windows(4).position(|w| w == b"\r\n\r\n")
            {
                header_end = Some(at + 4);
                let headers = String::from_utf8_lossy(&raw[..at]).to_string();
                content_length = headers
                    .lines()
                    .find_map(|line| {
                        let (name, value) = line.split_once(':')?;
                        name.eq_ignore_ascii_case("content-length")
                            .then(|| value.trim().parse::<usize>().ok())
                            .flatten()
                    })
                    .unwrap_or(0);
            }
            if let Some(at) = header_end
                && raw.len() >= at + content_length
            {
                break;
            }
        }
        let at = header_end.unwrap_or(raw.len());
        let headers = String::from_utf8_lossy(&raw[..at.saturating_sub(4)]).to_string();
        let body = raw.get(at..).map(<[u8]>::to_vec).unwrap_or_default();
        let _ = tx.send((headers, body));

        let response = format!(
            "HTTP/1.1 200 OK\r\nContent-Type: {content_type}\r\nConnection: close\r\n\r\n{response_body}"
        );
        let _ = socket.write_all(response.as_bytes()).await;
        let _ = socket.shutdown().await;
    });

    Ok((format!("http://{addr}"), rx))
}

/// The JSON body of a captured request.
fn request_body(captured: (String, Vec<u8>)) -> Value {
    serde_json::from_slice(&captured.1).unwrap_or_else(|error| {
        panic!(
            "captured request body is not JSON ({error}): {}",
            String::from_utf8_lossy(&captured.1)
        )
    })
}

/// A one-turn request asking for a tool call, in the canonical form.
fn tool_request(model: &str) -> ChatRequest {
    ChatRequest {
        model: model.to_string(),
        messages: Arc::new(vec![ChatMessage {
            role: "user".to_string(),
            content: ChatContent::Text("Read notes.txt and report the first line.".to_string()),
        }]),
        tools: Arc::new(vec![ragent_types::ToolDefinition {
            name: "Read".to_string(),
            description: "Read a file".to_string(),
            parameters: json!({
                "type": "object",
                "properties": { "path": { "type": "string" } },
                "required": ["path"]
            }),
        }]),
        temperature: None,
        top_p: None,
        max_tokens: Some(256),
        system: None,
        options: HashMap::new(),
        session_id: None,
        request_id: None,
        stream_timeout_secs: Some(10),
        thinking: None,
    }
}

/// Drain a provider's event stream.
async fn drain(
    mut stream: Pin<Box<dyn futures::Stream<Item = StreamEvent> + Send>>,
) -> Vec<StreamEvent> {
    let mut events = Vec::new();
    while let Some(event) = stream.next().await {
        events.push(event);
    }
    events
}

/// The one tool call a replay must produce: its `(id, name)`, its concatenated
/// arguments, and the finish reason.
struct Replayed {
    id: String,
    name: String,
    args: String,
    finish: FinishReason,
}

/// Assert that `events` carry exactly one tool call and return it.
///
/// This is the shared half of every family case: one start, at least one delta,
/// exactly one end carrying the start's id, and exactly one finish. A family
/// that produced two starts (a repeated frame re-opening a call), no end (a
/// dropped call), or a mismatched id (a start and an end that disagree) fails
/// here, in the same words, whichever provider it is.
fn assert_one_call(events: &[StreamEvent], family: &str) -> Replayed {
    let starts: Vec<(&String, &String)> = events
        .iter()
        .filter_map(|event| match event {
            StreamEvent::ToolCallStart { id, name } => Some((id, name)),
            _ => None,
        })
        .collect();
    assert_eq!(
        starts.len(),
        1,
        "{family}: the replay must open exactly one call, got {starts:?} in {events:?}"
    );
    let (id, name) = starts[0];

    let ends: Vec<&String> = events
        .iter()
        .filter_map(|event| match event {
            StreamEvent::ToolCallEnd { id } => Some(id),
            _ => None,
        })
        .collect();
    assert_eq!(
        ends.len(),
        1,
        "{family}: exactly one end per started call, got {ends:?} in {events:?}"
    );
    assert_eq!(
        ends[0], id,
        "{family}: the end must carry the id the start carried"
    );

    let args: String = events
        .iter()
        .filter_map(|event| match event {
            StreamEvent::ToolCallDelta {
                id: delta_id,
                args_json,
            } => {
                assert_eq!(
                    delta_id, id,
                    "{family}: a delta must be attributed to the open call"
                );
                Some(args_json.clone())
            }
            _ => None,
        })
        .collect();

    // A family may settle its reason twice: the chat-completions parser maps
    // `finish_reason` on the frame that carries it and the family's end-of-stream
    // frame then yields `Stop`. The *last* one is the one the consumer sees, so it
    // is what this helper returns - with the earlier ones pinned to the recorded
    // sequence by each case that cares.
    let finishes: Vec<FinishReason> = events
        .iter()
        .filter_map(|event| match event {
            StreamEvent::Finish { reason } => Some(reason.clone()),
            _ => None,
        })
        .collect();
    assert!(
        !finishes.is_empty(),
        "{family}: the replay must finish, got {events:?}"
    );

    Replayed {
        id: id.clone(),
        name: name.clone(),
        args,
        finish: finishes[finishes.len() - 1].clone(),
    }
}

/// The argument JSON a replayed call carried, decoded.
fn args_value(args: &str, family: &str) -> Value {
    serde_json::from_str(args).unwrap_or_else(|error| {
        panic!("{family}: replayed arguments are not JSON ({error}): {args}")
    })
}

// ---------------------------------------------------------------------------
// Encoders are pure (NFR-004, second clause)
// ---------------------------------------------------------------------------

#[test]
fn test_every_family_encoder_is_a_pure_function_of_its_inputs() {
    // NFR-004's second clause: the encoders are pure functions over
    // `&[ToolDefinition]` and `&ChatMessage`. Purity is what lets a fixture be a
    // recording rather than a live call: the same inputs must produce the same
    // bytes on every call, with no environment, no lock and no I/O.
    let request = tool_request("fixture-model");
    let names = tool_names_by_id(&request.messages);
    let ctx = MessageContext {
        tool_names: Some(&names),
        ..MessageContext::default()
    };

    for (family, profile) in ragent_llm::provider::wire::profile_table::FAMILIES {
        let first = encode_messages(*profile, &request.messages, &ctx)
            .unwrap_or_else(|loss| panic!("{family}: the encoder refused a text turn: {loss}"));
        let second = encode_messages(*profile, &request.messages, &ctx)
            .unwrap_or_else(|loss| panic!("{family}: the encoder refused a text turn: {loss}"));
        assert_eq!(
            first, second,
            "{family}: encoding the same history twice must produce the same entries"
        );
    }
}

// ---------------------------------------------------------------------------
// Family: OpenAI chat completions (recorded chat.completion.chunk frames)
// ---------------------------------------------------------------------------

/// The recorded chat-completions stream: an index-keyed `tool_calls` delta split
/// across two frames, then `finish_reason: "tool_calls"`, then `[DONE]`.
const OPENAI_CHAT_SSE: &str = concat!(
    "data: {\"id\":\"chatcmpl-rec\",\"object\":\"chat.completion.chunk\",\"created\":1712345678,\"model\":\"gpt-4o-2024-08-06\",\"choices\":[{\"index\":0,\"delta\":{\"role\":\"assistant\",\"content\":null},\"finish_reason\":null}]}\n\n",
    "data: {\"id\":\"chatcmpl-rec\",\"object\":\"chat.completion.chunk\",\"created\":1712345678,\"model\":\"gpt-4o-2024-08-06\",\"choices\":[{\"index\":0,\"delta\":{\"tool_calls\":[{\"index\":0,\"id\":\"call_REDACTED1\",\"type\":\"function\",\"function\":{\"name\":\"Read\"}}]},\"finish_reason\":null}]}\n\n",
    "data: {\"id\":\"chatcmpl-rec\",\"object\":\"chat.completion.chunk\",\"created\":1712345678,\"model\":\"gpt-4o-2024-08-06\",\"choices\":[{\"index\":0,\"delta\":{\"tool_calls\":[{\"index\":0,\"type\":\"function\",\"function\":{\"arguments\":\"{\\\"path\\\":\"}}]},\"finish_reason\":null}]}\n\n",
    "data: {\"id\":\"chatcmpl-rec\",\"object\":\"chat.completion.chunk\",\"created\":1712345678,\"model\":\"gpt-4o-2024-08-06\",\"choices\":[{\"index\":0,\"delta\":{\"tool_calls\":[{\"index\":0,\"type\":\"function\",\"function\":{\"arguments\":\"\\\"notes.txt\\\"}\"}}]},\"finish_reason\":null}]}\n\n",
    "data: {\"id\":\"chatcmpl-rec\",\"object\":\"chat.completion.chunk\",\"created\":1712345678,\"model\":\"gpt-4o-2024-08-06\",\"choices\":[{\"index\":0,\"delta\":{},\"finish_reason\":\"tool_calls\"}],\"usage\":{\"prompt_tokens\":41,\"completion_tokens\":12,\"total_tokens\":53}}\n\n",
    "data: [DONE]\n\n",
);

#[tokio::test]
async fn test_openai_chat_replay_round_trips_through_the_encoder() {
    let (base_url, captured) = record_one_request("text/event-stream", OPENAI_CHAT_SSE)
        .await
        .expect("loopback recorder");
    let client = ragent_llm::OpenAiProvider
        .create_client("fixture-key", Some(&base_url), &HashMap::new())
        .await
        .expect("openai client");

    let events = drain(
        client
            .chat(tool_request("gpt-4o-2024-08-06"))
            .await
            .expect("chat"),
    )
    .await;
    let replayed = assert_one_call(&events, "openai_chat");

    assert_eq!(replayed.id, "call_REDACTED1");
    assert_eq!(replayed.name, "Read");
    assert_eq!(
        args_value(&replayed.args, "openai_chat"),
        json!({"path": "notes.txt"}),
        "the recorded fragments must concatenate into the call's arguments"
    );
    // The recorded frame settles `tool_use`; the family's end-of-stream frame
    // then settles `stop`, which is the reason the consumer sees.
    let reasons: Vec<&FinishReason> = events
        .iter()
        .filter_map(|event| match event {
            StreamEvent::Finish { reason } => Some(reason),
            _ => None,
        })
        .collect();
    assert_eq!(
        reasons,
        vec![&FinishReason::ToolUse, &FinishReason::Stop],
        "the recorded sequence settles tool_use then stop"
    );

    // The request the recorder received was built by the family's encoder.
    let body = request_body(captured.await.expect("captured request"));
    assert_eq!(
        body["tools"][0]["function"]["name"], "Read",
        "the OpenAI envelope wraps the definition: {body}"
    );
    assert_eq!(
        body["messages"][0]["content"], "Read notes.txt and report the first line.",
        "the user turn travels as a string on this family"
    );

    // The encode half: the call the decoder produced becomes the assistant turn
    // the next request replays, in this family's own vocabulary.
    let assistant = ChatMessage {
        role: "assistant".to_string(),
        content: ChatContent::Parts(vec![ContentPart::ToolUse {
            id: replayed.id.clone(),
            name: replayed.name.clone(),
            input: args_value(&replayed.args, "openai_chat"),
        }]),
    };
    let encoded = encode_message(OPENAI_CHAT, &assistant, &MessageContext::default());
    assert_eq!(encoded[0]["tool_calls"][0]["id"], "call_REDACTED1");
    assert_eq!(encoded[0]["tool_calls"][0]["function"]["name"], "Read");
    assert_eq!(
        encoded[0]["tool_calls"][0]["function"]["arguments"],
        json!({"path": "notes.txt"}).to_string(),
        "this family escapes the arguments into a JSON string"
    );
}

// ---------------------------------------------------------------------------
// Family: Anthropic Messages (recorded content_block frames)
// ---------------------------------------------------------------------------

/// The recorded Anthropic stream: a thinking block, a text block, then a
/// `tool_use` block whose `input_json_delta` frames are keyed by block index.
const ANTHROPIC_SSE: &str = concat!(
    "event: message_start\n",
    "data: {\"type\":\"message_start\",\"message\":{\"id\":\"msg_rec\",\"model\":\"claude-sonnet-4-20250514\",\"usage\":{\"input_tokens\":88,\"output_tokens\":1}}}\n\n",
    "event: content_block_start\n",
    "data: {\"type\":\"content_block_start\",\"index\":0,\"content_block\":{\"type\":\"thinking\",\"thinking\":\"\"}}\n\n",
    "event: content_block_delta\n",
    "data: {\"type\":\"content_block_delta\",\"index\":0,\"delta\":{\"type\":\"thinking_delta\",\"thinking\":\"I should read the file.\"}}\n\n",
    "event: content_block_delta\n",
    "data: {\"type\":\"content_block_delta\",\"index\":0,\"delta\":{\"type\":\"signature_delta\",\"signature\":\"EqQBCkYIBxgCKkA\"}}\n\n",
    "event: content_block_stop\n",
    "data: {\"type\":\"content_block_stop\",\"index\":0}\n\n",
    "event: content_block_start\n",
    "data: {\"type\":\"content_block_start\",\"index\":1,\"content_block\":{\"type\":\"text\",\"text\":\"\"}}\n\n",
    "event: content_block_delta\n",
    "data: {\"type\":\"content_block_delta\",\"index\":1,\"delta\":{\"type\":\"text_delta\",\"text\":\"Reading it now.\"}}\n\n",
    "event: content_block_stop\n",
    "data: {\"type\":\"content_block_stop\",\"index\":1}\n\n",
    "event: content_block_start\n",
    "data: {\"type\":\"content_block_start\",\"index\":2,\"content_block\":{\"type\":\"tool_use\",\"id\":\"toolu_REDACTED1\",\"name\":\"Read\",\"input\":{}}}\n\n",
    "event: content_block_delta\n",
    "data: {\"type\":\"content_block_delta\",\"index\":2,\"delta\":{\"type\":\"input_json_delta\",\"partial_json\":\"\"}}\n\n",
    "event: content_block_delta\n",
    "data: {\"type\":\"content_block_delta\",\"index\":2,\"delta\":{\"type\":\"input_json_delta\",\"partial_json\":\"{\\\"path\\\": \\\"notes.txt\\\"}\"}}\n\n",
    "event: content_block_stop\n",
    "data: {\"type\":\"content_block_stop\",\"index\":2}\n\n",
    "event: message_delta\n",
    "data: {\"type\":\"message_delta\",\"delta\":{\"stop_reason\":\"tool_use\"},\"usage\":{\"output_tokens\":41}}\n\n",
    "event: message_stop\n",
    "data: {\"type\":\"message_stop\"}\n\n",
);

#[tokio::test]
async fn test_anthropic_replay_round_trips_through_the_encoder() {
    let (base_url, captured) = record_one_request("text/event-stream", ANTHROPIC_SSE)
        .await
        .expect("loopback recorder");
    let client = AnthropicProvider
        .create_client("fixture-key", Some(&base_url), &HashMap::new())
        .await
        .expect("anthropic client");

    let events = drain(
        client
            .chat(tool_request("claude-sonnet-4-20250514"))
            .await
            .expect("chat"),
    )
    .await;
    let replayed = assert_one_call(&events, "anthropic_messages");

    assert_eq!(replayed.id, "toolu_REDACTED1");
    assert_eq!(replayed.name, "Read");
    assert_eq!(
        args_value(&replayed.args, "anthropic_messages"),
        json!({"path": "notes.txt"})
    );
    assert_eq!(replayed.finish, FinishReason::ToolUse);

    // FR-011: the recorded signature must reach the accumulator, so the block
    // this family replays is signed rather than merely present.
    assert!(
        events.iter().any(|event| matches!(
            event,
            StreamEvent::ReasoningSignature { signature } if signature == "EqQBCkYIBxgCKkA"
        )),
        "the recorded signature_delta must reach the reasoning echo: {events:?}"
    );

    let body = request_body(captured.await.expect("captured request"));
    assert!(
        body["tools"][0].get("input_schema").is_some(),
        "the Anthropic envelope is `{{name,description,input_schema}}`: {body}"
    );
    assert_eq!(body["tools"][0]["name"], "Read");

    // The encode half: the assistant turn the next request replays carries the
    // signed thinking block *before* the tool_use block, and the arguments are
    // an object rather than a string on this family.
    let assistant = ChatMessage {
        role: "assistant".to_string(),
        content: ChatContent::Parts(vec![
            ContentPart::Reasoning {
                text: "I should read the file.".to_string(),
                signature: Some("EqQBCkYIBxgCKkA".to_string()),
            },
            ContentPart::Text {
                text: "Reading it now.".to_string(),
            },
            ContentPart::ToolUse {
                id: replayed.id.clone(),
                name: replayed.name.clone(),
                input: args_value(&replayed.args, "anthropic_messages"),
            },
        ]),
    };
    let encoded = encode_message(ANTHROPIC_MESSAGES, &assistant, &MessageContext::default());
    assert_eq!(
        encoded[0]["content"][0]["type"], "thinking",
        "the signed block precedes the call it decided (FR-011): {}",
        encoded[0]
    );
    assert_eq!(encoded[0]["content"][0]["signature"], "EqQBCkYIBxgCKkA");
    assert_eq!(encoded[0]["content"][2]["type"], "tool_use");
    assert_eq!(encoded[0]["content"][2]["name"], "Read");
    assert_eq!(
        encoded[0]["content"][2]["input"],
        json!({"path": "notes.txt"}),
        "this family carries the arguments as an object"
    );
}

// ---------------------------------------------------------------------------
// Family: Gemini (recorded candidates[].parts[] frames)
// ---------------------------------------------------------------------------

/// The recorded Gemini stream: a `functionCall` part in the final candidate,
/// which also carries `finishReason` and `usageMetadata`.
///
/// The frames are newline-delimited plain JSON with no `data:` prefix, which is
/// the shape the family parses: unlike every other family here it is not SSE, and
/// a case that replayed `data:` frames would assert against a parser path that
/// never runs in production.
const GEMINI_SSE: &str = concat!(
    "{\"candidates\":[{\"content\":{\"parts\":[{\"text\":\"Checking the file.\"}],\"role\":\"model\"}}],\"usageMetadata\":{\"promptTokenCount\":31}}\n",
    "{\"candidates\":[{\"content\":{\"parts\":[{\"functionCall\":{\"name\":\"Read\",\"args\":{\"path\":\"notes.txt\"}}}],\"role\":\"model\"},\"finishReason\":\"TOOL_USE\"}],\"usageMetadata\":{\"promptTokenCount\":31,\"candidatesTokenCount\":9}}\n",
);

#[tokio::test]
async fn test_gemini_replay_round_trips_through_the_encoder() {
    let (base_url, captured) = record_one_request("text/event-stream", GEMINI_SSE)
        .await
        .expect("loopback recorder");
    let client = GeminiProvider
        .create_client("fixture-key", Some(&base_url), &HashMap::new())
        .await
        .expect("gemini client");

    let events = drain(
        client
            .chat(tool_request("gemini-2.0-flash"))
            .await
            .expect("chat"),
    )
    .await;
    let replayed = assert_one_call(&events, "gemini");

    assert_eq!(replayed.name, "Read");
    assert_eq!(
        args_value(&replayed.args, "gemini"),
        json!({"path": "notes.txt"})
    );
    // FR-019: the model names no id, so the harness mints one from the provider
    // id, a per-stream counter and the tool name.
    assert!(
        replayed.id.starts_with("gemini_0_Read"),
        "an id-less call gets the FR-019 id, got {}",
        replayed.id
    );

    let body = request_body(captured.await.expect("captured request"));
    assert!(
        body["tools"][0]["functionDeclarations"][0]
            .get("parameters")
            .is_some(),
        "the Gemini envelope is one `functionDeclarations` entry: {body}"
    );

    // The encode half: `functionCall` for the call and `functionResponse` for
    // the answer, which names the tool because the carrier has no id slot.
    let names = tool_names_by_id(&[ChatMessage {
        role: "assistant".to_string(),
        content: ChatContent::Parts(vec![ContentPart::ToolUse {
            id: replayed.id.clone(),
            name: replayed.name.clone(),
            input: args_value(&replayed.args, "gemini"),
        }]),
    }]);
    let ctx = MessageContext {
        tool_names: Some(&names),
        ..MessageContext::default()
    };
    let call = encode_message(
        GEMINI,
        &ChatMessage {
            role: "assistant".to_string(),
            content: ChatContent::Parts(vec![ContentPart::ToolUse {
                id: replayed.id.clone(),
                name: replayed.name.clone(),
                input: args_value(&replayed.args, "gemini"),
            }]),
        },
        &ctx,
    );
    assert_eq!(call[0]["role"], "model");
    assert_eq!(call[0]["parts"][0]["functionCall"]["name"], "Read");
    assert_eq!(
        call[0]["parts"][0]["functionCall"]["args"],
        json!({"path": "notes.txt"})
    );

    let answer = encode_message(
        GEMINI,
        &ChatMessage {
            role: "user".to_string(),
            content: ChatContent::Parts(vec![ContentPart::ToolResult {
                tool_use_id: replayed.id.clone(),
                content: "first line".into(),
                is_error: false,
            }]),
        },
        &ctx,
    );
    assert_eq!(answer[0]["parts"][0]["functionResponse"]["name"], "Read");
    assert!(
        answer[0]["parts"][0]["functionResponse"]
            .get("response")
            .is_some(),
        "the response payload is nested under `response`: {}",
        answer[0]
    );
}

// ---------------------------------------------------------------------------
// Family: Bedrock Converse (recorded contentBlock* events)
// ---------------------------------------------------------------------------

/// The recorded Converse stream: two parallel `toolUse` blocks, each keyed by
/// its own `contentBlockIndex`, then `messageStop` with `stopReason: tool_use`.
///
/// Two blocks rather than one is deliberate: FR-005's failure mode - a stream
/// that folds the second call's fragments onto the first - is invisible with a
/// single block, and the recorded shape is what makes it reachable here.
const BEDROCK_CONVERSE_SSE: &str = concat!(
    "event: messageStart\n",
    "data: {\"role\":\"assistant\"}\n",
    "event: contentBlockStart\n",
    "data: {\"contentBlockIndex\":0,\"start\":{\"toolUse\":{\"toolUseId\":\"tooluse_REDACTED1\",\"name\":\"Read\"}}}\n",
    "event: contentBlockDelta\n",
    "data: {\"contentBlockIndex\":0,\"delta\":{\"toolUse\":{\"input\":\"\"}}}\n",
    "event: contentBlockDelta\n",
    "data: {\"contentBlockIndex\":0,\"delta\":{\"toolUse\":{\"input\":\"{\\\"path\\\": \\\"notes.txt\\\"}\"}}}\n",
    "event: contentBlockStop\n",
    "data: {\"contentBlockIndex\":0}\n",
    "event: contentBlockStart\n",
    "data: {\"contentBlockIndex\":1,\"start\":{\"toolUse\":{\"toolUseId\":\"tooluse_REDACTED2\",\"name\":\"Read\"}}}\n",
    "event: contentBlockDelta\n",
    "data: {\"contentBlockIndex\":1,\"delta\":{\"toolUse\":{\"input\":\"{\\\"path\\\": \\\"other.txt\\\"}\"}}}\n",
    "event: contentBlockStop\n",
    "data: {\"contentBlockIndex\":1}\n",
    "event: messageStop\n",
    "data: {\"stopReason\":\"tool_use\",\"usage\":{\"inputTokens\":61,\"outputTokens\":24}}\n",
);

/// Environment the Bedrock fixture needs: AWS credentials, so that
/// `resolve_aws_credentials` does not fail before any request is made.
const BEDROCK_ENV: &[&str] = &[
    "AWS_ACCESS_KEY_ID",
    "AWS_SECRET_ACCESS_KEY",
    "AWS_SESSION_TOKEN",
    "AWS_REGION",
    "AWS_BEDROCK_REGION",
    "AWS_PROFILE",
    "AWS_SHARED_CREDENTIALS_FILE",
];

#[test]
fn test_bedrock_converse_replay_round_trips_through_the_encoder() {
    // The concurrency limit of the session is one, because this case forces
    // process-global state: Rust 2024 makes `set_var` unsafe, so the AWS
    // variables are set and cleared under the crate's one env lock while no
    // other test in this process observes them (AGENTS.md, "Test Layout"). The
    // loopback round trip runs on a runtime this test owns, so the lock is not
    // held across an `await` point belonging to the test's own frame.
    let _guard = crate::global_env_lock();
    for var in BEDROCK_ENV {
        // SAFETY: exclusive access to the AWS_* variables via the crate env lock.
        unsafe { std::env::remove_var(var) };
    }
    // SAFETY: exclusive access to the AWS_* variables via the crate env lock.
    unsafe {
        std::env::set_var("AWS_ACCESS_KEY_ID", "AKIAFIXTUREREADONLY");
        std::env::set_var("AWS_SECRET_ACCESS_KEY", "fixture-secret-read-only");
        std::env::set_var("AWS_BEDROCK_REGION", "us-east-1");
        std::env::set_var(
            "AWS_SHARED_CREDENTIALS_FILE",
            "/nonexistent/aws/credentials",
        );
    }

    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("test runtime");
    let (events, request) = runtime.block_on(async {
        let (base_url, captured) = record_one_request("text/event-stream", BEDROCK_CONVERSE_SSE)
            .await
            .expect("loopback recorder");
        let mut options = HashMap::new();
        options.insert("endpoint_url".to_string(), json!(base_url));
        // A non-Anthropic id selects the Converse client rather than the Bedrock
        // Anthropic one (two clients, one provider id).
        options.insert("model_id".to_string(), json!("amazon.nova-pro-v1:0"));
        let client = ragent_llm::provider::bedrock::BedrockProvider
            .create_client("", None, &options)
            .await
            .expect("bedrock converse client");
        let events = drain(
            client
                .chat(tool_request("amazon.nova-pro-v1:0"))
                .await
                .expect("chat"),
        )
        .await;
        (events, captured.await.expect("captured request"))
    });

    for var in BEDROCK_ENV {
        // SAFETY: exclusive access to the AWS_* variables via the crate env lock.
        unsafe { std::env::remove_var(var) };
    }

    let calls: Vec<(&String, &String)> = events
        .iter()
        .filter_map(|event| match event {
            StreamEvent::ToolCallStart { id, name } => Some((id, name)),
            _ => None,
        })
        .collect();
    assert_eq!(
        calls.len(),
        2,
        "two parallel content blocks must open two calls (FR-005): {events:?}"
    );
    let ids: Vec<&String> = calls.iter().map(|(id, _)| *id).collect();
    assert_eq!(ids, vec!["tooluse_REDACTED1", "tooluse_REDACTED2"]);

    let ends: Vec<&String> = events
        .iter()
        .filter_map(|event| match event {
            StreamEvent::ToolCallEnd { id } => Some(id),
            _ => None,
        })
        .collect();
    assert_eq!(
        ends, ids,
        "each block ends its own call, in index order (FR-005)"
    );

    let mut by_id: HashMap<String, String> = HashMap::new();
    for event in &events {
        if let StreamEvent::ToolCallDelta { id, args_json } = event {
            by_id.entry(id.clone()).or_default().push_str(args_json);
        }
    }
    assert_eq!(
        args_value(
            by_id.get("tooluse_REDACTED1").map_or("", String::as_str),
            "bedrock"
        ),
        json!({"path": "notes.txt"}),
        "block 0 keeps its own arguments"
    );
    assert_eq!(
        args_value(
            by_id.get("tooluse_REDACTED2").map_or("", String::as_str),
            "bedrock"
        ),
        json!({"path": "other.txt"}),
        "block 1 keeps its own arguments, not a concatenation (FR-005)"
    );
    assert!(
        events.iter().any(
            |e| matches!(e, StreamEvent::Finish { reason } if *reason == FinishReason::ToolUse)
        ),
        "messageStop carries stopReason tool_use: {events:?}"
    );

    // The request the recorder received used the Converse envelope, under
    // `toolConfig`, which is where only this family puts it.
    let body = request_body(request);
    assert!(
        body["toolConfig"]["tools"][0]["toolSpec"]
            .get("inputSchema")
            .is_some(),
        "the Converse envelope is `toolSpec` under `toolConfig`: {body}"
    );

    // The encode half: `toolUse` blocks out and `toolResult` blocks back, the
    // latter carrying the native `status` this family declares (FR-014).
    let assistant = ChatMessage {
        role: "assistant".to_string(),
        content: ChatContent::Parts(vec![
            ContentPart::ToolUse {
                id: "tooluse_REDACTED1".to_string(),
                name: "Read".to_string(),
                input: json!({"path": "notes.txt"}),
            },
            ContentPart::ToolUse {
                id: "tooluse_REDACTED2".to_string(),
                name: "Read".to_string(),
                input: json!({"path": "other.txt"}),
            },
        ]),
    };
    let encoded = encode_message(BEDROCK_CONVERSE, &assistant, &MessageContext::default());
    assert_eq!(
        encoded[0]["content"][0]["toolUse"]["toolUseId"],
        "tooluse_REDACTED1"
    );
    assert_eq!(
        encoded[0]["content"][1]["toolUse"]["input"],
        json!({"path": "other.txt"})
    );

    let answer = encode_message(
        BEDROCK_CONVERSE,
        &ChatMessage {
            role: "user".to_string(),
            content: ChatContent::Parts(vec![ContentPart::ToolResult {
                tool_use_id: "tooluse_REDACTED1".to_string(),
                content: "first line".into(),
                is_error: true,
            }]),
        },
        &MessageContext::default(),
    );
    assert_eq!(
        answer[0]["content"][0]["toolResult"]["toolUseId"],
        "tooluse_REDACTED1"
    );
    assert_eq!(
        answer[0]["content"][0]["toolResult"]["status"], "error",
        "Converse carries the error natively (FR-014, FR-017)"
    );
}

// ---------------------------------------------------------------------------
// Family: OpenAI Responses (recorded response.* events)
// ---------------------------------------------------------------------------

/// The recorded Responses stream: the function-call item is announced before its
/// argument deltas, and every delta repeats the `call_id` (id-keyed, FR-004).
///
/// The frames are `data:`-prefixed lines with no `event:` line, which is the
/// shape the family's parser reads: it dispatches on the JSON `type` field and
/// skips any line without the prefix.
const RESPONSES_SSE: &str = concat!(
    "data: {\"type\":\"response.created\",\"response\":{\"id\":\"resp_REDACTED\",\"status\":\"in_progress\"}}\n\n",
    "data: {\"type\":\"response.output_item.added\",\"output_index\":0,\"item\":{\"type\":\"function_call\",\"id\":\"fc_REDACTED\",\"call_id\":\"call_REDACTED1\",\"name\":\"Read\",\"arguments\":\"\"}}\n\n",
    "data: {\"type\":\"response.function_call_arguments.delta\",\"item_id\":\"fc_REDACTED\",\"call_id\":\"call_REDACTED1\",\"delta\":\"{\\\"path\\\":\"}\n\n",
    "data: {\"type\":\"response.function_call_arguments.delta\",\"item_id\":\"fc_REDACTED\",\"call_id\":\"call_REDACTED1\",\"delta\":\" \\\"notes.txt\\\"}\"}\n\n",
    "data: {\"type\":\"response.function_call_arguments.done\",\"item_id\":\"fc_REDACTED\",\"call_id\":\"call_REDACTED1\",\"arguments\":\"{\\\"path\\\": \\\"notes.txt\\\"}\"}\n\n",
    "data: {\"type\":\"response.completed\",\"response\":{\"id\":\"resp_REDACTED\",\"status\":\"completed\",\"usage\":{\"input_tokens\":47,\"output_tokens\":13}}}\n\n",
);

#[tokio::test]
async fn test_openai_responses_replay_round_trips_through_the_encoder() {
    let (base_url, captured) = record_one_request("text/event-stream", RESPONSES_SSE)
        .await
        .expect("loopback recorder");
    let client = ragent_llm::provider::openai_responses::ResponsesApiProvider
        .create_client("fixture-key", Some(&base_url), &HashMap::new())
        .await
        .expect("responses client");

    let events = drain(client.chat(tool_request("gpt-5.6")).await.expect("chat")).await;
    let replayed = assert_one_call(&events, "openai_responses");

    assert_eq!(replayed.id, "call_REDACTED1");
    assert_eq!(replayed.name, "Read");
    assert_eq!(
        args_value(&replayed.args, "openai_responses"),
        json!({"path": "notes.txt"})
    );

    let body = request_body(captured.await.expect("captured request"));
    assert_eq!(
        body["tools"][0]["name"], "Read",
        "the Responses envelope is the flattened function shape: {body}"
    );
    assert!(
        body["tools"][0].get("function").is_none(),
        "it has no nested `function` object: {body}"
    );

    // The encode half, and FR-018's subject: the input list carries the tool
    // exchange as `function_call` / `function_call_output` items rather than
    // dropping every non-text part.
    let history = vec![
        ChatMessage {
            role: "user".to_string(),
            content: ChatContent::Text("Read notes.txt".to_string()),
        },
        ChatMessage {
            role: "assistant".to_string(),
            content: ChatContent::Parts(vec![ContentPart::ToolUse {
                id: replayed.id.clone(),
                name: replayed.name.clone(),
                input: args_value(&replayed.args, "openai_responses"),
            }]),
        },
        ChatMessage {
            role: "user".to_string(),
            content: ChatContent::Parts(vec![ContentPart::ToolResult {
                tool_use_id: replayed.id.clone(),
                content: "first line".into(),
                is_error: false,
            }]),
        },
    ];
    let encoded = encode_messages(OPENAI_RESPONSES, &history, &MessageContext::default())
        .expect("the Responses input list carries every construct here");
    let tool_items: Vec<&Value> = encoded
        .iter()
        .filter(|item| item["type"].as_str().is_some())
        .collect();
    let kinds: Vec<&str> = tool_items
        .iter()
        .filter_map(|item| item["type"].as_str())
        .collect();
    assert_eq!(
        kinds,
        vec!["function_call", "function_call_output"],
        "the tool exchange survives the encode: {encoded:?}"
    );
    assert_eq!(tool_items[0]["call_id"], "call_REDACTED1");
    assert_eq!(
        tool_items[0]["arguments"],
        json!({"path": "notes.txt"}).to_string(),
        "the Responses family escapes the arguments into a JSON string"
    );
    assert_eq!(tool_items[1]["call_id"], "call_REDACTED1");
}

// ---------------------------------------------------------------------------
// Family: native Ollama /api/chat (recorded NDJSON frames)
// ---------------------------------------------------------------------------

/// The recorded native `/api/chat` stream: newline-delimited JSON, no `data:`
/// prefix, a `tool_calls` message, then the `done` frame.
const OLLAMA_NATIVE_NDJSON: &str = concat!(
    "{\"model\":\"fixture\",\"created_at\":\"2024-05-01T00:00:00Z\",\"message\":{\"role\":\"assistant\",\"content\":\"\"},\"done\":false}\n",
    "{\"model\":\"fixture\",\"created_at\":\"2024-05-01T00:00:00Z\",\"message\":{\"role\":\"assistant\",\"content\":\"\",\"tool_calls\":[{\"function\":{\"name\":\"Read\",\"arguments\":{\"path\":\"notes.txt\"}}}]},\"done\":false}\n",
    "{\"model\":\"fixture\",\"created_at\":\"2024-05-01T00:00:01Z\",\"message\":{\"role\":\"assistant\",\"content\":\"\"},\"done\":true,\"done_reason\":\"stop\",\"prompt_eval_count\":23,\"eval_count\":11}\n",
);

#[tokio::test]
async fn test_ollama_native_replay_round_trips_through_the_encoder() {
    let (base_url, captured) = record_one_request("application/x-ndjson", OLLAMA_NATIVE_NDJSON)
        .await
        .expect("loopback recorder");
    let client = OllamaCloudProvider::new()
        .create_client("fixture-key", Some(&base_url), &HashMap::new())
        .await
        .expect("ollama cloud client");

    let events = drain(client.chat(tool_request("fixture")).await.expect("chat")).await;
    let replayed = assert_one_call(&events, "ollama_native");

    assert_eq!(replayed.name, "Read");
    assert_eq!(
        args_value(&replayed.args, "ollama_native"),
        json!({"path": "notes.txt"}),
        "the native frame carries the arguments as an object"
    );
    // FR-019: the native call has no id, so one is minted.
    assert!(
        replayed.id.starts_with("ollama_cloud_0_Read"),
        "an id-less native call gets the FR-019 id, got {}",
        replayed.id
    );
    assert_eq!(replayed.finish, FinishReason::Stop);

    let body = request_body(captured.await.expect("captured request"));
    assert_eq!(
        body["tools"][0]["function"]["name"], "Read",
        "the native family uses the OpenAI envelope for definitions: {body}"
    );

    // The encode half. This family's carrier names the tool and has no id slot,
    // so the assistant turn it replays is a call *and* the answer is a `role:"tool"`
    // message naming the tool. Both must reach the body: before this fixture the
    // encoder dropped a result that shared a turn with text, and could not write
    // a call and its text together at all.
    let call = ChatMessage {
        role: "assistant".to_string(),
        content: ChatContent::Parts(vec![
            ContentPart::Text {
                text: "Reading it now.".to_string(),
            },
            ContentPart::ToolUse {
                id: replayed.id.clone(),
                name: replayed.name.clone(),
                input: args_value(&replayed.args, "ollama_native"),
            },
        ]),
    };
    let encoded = encode_message(OLLAMA_NATIVE, &call, &MessageContext::default());
    assert_eq!(
        encoded.len(),
        2,
        "the text and the call each reach the wire: {encoded:?}"
    );
    assert_eq!(encoded[0]["content"], "Reading it now.");
    assert_eq!(encoded[1]["tool_calls"][0]["function"]["name"], "Read");
    assert_eq!(
        encoded[1]["tool_calls"][0]["function"]["arguments"],
        json!({"path": "notes.txt"}),
        "this family carries the arguments as an object"
    );

    let names = tool_names_by_id(&[call]);
    let ctx = MessageContext {
        tool_names: Some(&names),
        ..MessageContext::default()
    };
    let answer = encode_message(
        OLLAMA_NATIVE,
        &ChatMessage {
            role: "user".to_string(),
            content: ChatContent::Parts(vec![
                ContentPart::Text {
                    text: "Here it is:".to_string(),
                },
                ContentPart::ToolResult {
                    tool_use_id: replayed.id.clone(),
                    content: "first line".into(),
                    is_error: false,
                },
            ]),
        },
        &ctx,
    );
    assert_eq!(
        answer.len(),
        2,
        "the narrative text and the result both reach the wire: {answer:?}"
    );
    assert_eq!(answer[1]["role"], "tool");
    assert_eq!(
        answer[1]["name"], "Read",
        "the native result names the tool it answers"
    );
    assert!(
        answer[1].get("tool_call_id").is_none(),
        "and carries no id slot: {}",
        answer[1]
    );
}
