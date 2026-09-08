#![allow(clippy::assert_is_empty)]
//! Regression tests for the Gemini streaming tool-call extraction (F2).
//!
//! Gemini's final frame commonly carries BOTH `finishReason` and the
//! `functionCall` part. The parser used to process the finish reason first,
//! clear the pending buffer, and only then buffer the tool call — which was
//! then never emitted. The fix parses parts before the finish-reason flush
//! and adds an end-of-stream flush, so the call is emitted exactly once.
//!
//! These tests drive the Gemini SSE parser through a local mock HTTP server.

use std::collections::HashMap;
use std::sync::Arc;

use futures::StreamExt;
use ragent_llm::llm::{ChatContent, ChatMessage, ChatRequest, StreamEvent};
use ragent_llm::{GeminiProvider, Provider};
use tokio::io::AsyncWriteExt;
use tokio::net::TcpListener;

fn make_request(model: &str) -> ChatRequest {
    ChatRequest {
        model: model.to_string(),
        messages: Arc::new(vec![ChatMessage {
            role: "user".to_string(),
            content: ChatContent::Text("Call the weather tool.".to_string()),
        }]),
        tools: Arc::new(vec![]),
        temperature: None,
        top_p: None,
        max_tokens: Some(128),
        system: None,
        options: HashMap::new(),
        session_id: None,
        request_id: None,
        stream_timeout_secs: Some(5),
        thinking: None,
    }
}

async fn spawn_gemini_sse_server(response_body: String) -> anyhow::Result<String> {
    let listener = TcpListener::bind("127.0.0.1:0").await?;
    let addr = listener.local_addr()?;

    tokio::spawn(async move {
        let Ok((mut socket, _)) = listener.accept().await else {
            return;
        };

        let response = format!(
            "HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nConnection: close\r\n\r\n{response_body}"
        );
        let _ = socket.write_all(response.as_bytes()).await;
        let _ = socket.shutdown().await;
    });

    Ok(format!("http://{addr}"))
}

fn drain_events(events: &[StreamEvent]) -> (Vec<(String, String)>, String, usize) {
    let starts: Vec<(String, String)> = events
        .iter()
        .filter_map(|e| match e {
            StreamEvent::ToolCallStart { id, name } => Some((id.clone(), name.clone())),
            _ => None,
        })
        .collect();
    let args: String = events
        .iter()
        .filter_map(|e| match e {
            StreamEvent::ToolCallDelta { args_json, .. } => Some(args_json.clone()),
            _ => None,
        })
        .collect();
    let ends = events
        .iter()
        .filter(|e| matches!(e, StreamEvent::ToolCallEnd { .. }))
        .count();
    (starts, args, ends)
}

#[tokio::test]
async fn test_gemini_final_chunk_function_call_with_finish_reason_survives() {
    // The final frame carries both finishReason and the functionCall part:
    // the exact shape the old ordering lost.
    let sse = concat!(
        "{\"candidates\":[{\"content\":{\"parts\":[{\"text\":\"Checking\"}]}}]}\n\n",
        "{\"candidates\":[{\"content\":{\"parts\":[{\"functionCall\":{\"name\":\"get_weather\",\"args\":{\"location\":\"London\"}}}]},\"finishReason\":\"STOP\"}],\"usageMetadata\":{\"promptTokenCount\":10,\"candidatesTokenCount\":5}}\n\n",
        ""
    );

    let url = spawn_gemini_sse_server(sse.to_string())
        .await
        .expect("server");
    let client = GeminiProvider
        .create_client("", Some(&url), &HashMap::new())
        .await
        .expect("client");

    let mut stream = client
        .chat(make_request("gemini-2.0-flash"))
        .await
        .expect("chat");
    let mut events = Vec::new();
    while let Some(event) = stream.next().await {
        events.push(event);
    }

    let (starts, args, ends) = drain_events(&events);
    assert_eq!(
        starts.len(),
        1,
        "final-chunk functionCall must be emitted exactly once; got {starts:?}"
    );
    assert_eq!(starts[0].1, "get_weather");
    assert!(
        args.contains("London"),
        "args should contain London: {args}"
    );
    assert_eq!(ends, 1, "expected one ToolCallEnd");
    assert!(
        events
            .iter()
            .any(|e| matches!(e, StreamEvent::Finish { .. })),
        "the finish event should still be emitted"
    );
}

#[tokio::test]
async fn test_gemini_no_duplicate_flush_when_call_already_emitted() {
    // The call arrives in a frame WITHOUT a finish reason, then the stream
    // ends with a finish-reason frame. The end-of-stream flush must not
    // re-emit the same call.
    let sse = concat!(
        "{\"candidates\":[{\"content\":{\"parts\":[{\"functionCall\":{\"name\":\"read_file\",\"args\":{\"path\":\"a.rs\"}}}]}}]}\n\n",
        "{\"candidates\":[{\"finishReason\":\"STOP\"}]}\n\n",
        ""
    );

    let url = spawn_gemini_sse_server(sse.to_string())
        .await
        .expect("server");
    let client = GeminiProvider
        .create_client("", Some(&url), &HashMap::new())
        .await
        .expect("client");

    let mut stream = client
        .chat(make_request("gemini-2.0-flash"))
        .await
        .expect("chat");
    let mut events = Vec::new();
    while let Some(event) = stream.next().await {
        events.push(event);
    }

    let (starts, args, ends) = drain_events(&events);
    assert_eq!(starts.len(), 1, "exactly one start; got {starts:?}");
    assert_eq!(starts[0].1, "read_file");
    assert!(args.contains("a.rs"));
    assert_eq!(ends, 1, "exactly one end");
}

#[tokio::test]
async fn test_gemini_call_in_stream_without_finish_reason_is_flushed() {
    // F2: a stream that ends without any finishReason frame previously
    // dropped the buffered call entirely.
    let sse = concat!(
        "{\"candidates\":[{\"content\":{\"parts\":[{\"functionCall\":{\"name\":\"bash\",\"args\":{\"command\":\"ls\"}}}]}}]}\n\n",
        ""
    );

    let url = spawn_gemini_sse_server(sse.to_string())
        .await
        .expect("server");
    let client = GeminiProvider
        .create_client("", Some(&url), &HashMap::new())
        .await
        .expect("client");

    let mut stream = client
        .chat(make_request("gemini-2.0-flash"))
        .await
        .expect("chat");
    let mut events = Vec::new();
    while let Some(event) = stream.next().await {
        events.push(event);
    }

    let (starts, args, ends) = drain_events(&events);
    assert_eq!(starts.len(), 1, "end-of-stream flush must emit the call");
    assert_eq!(starts[0].1, "bash");
    assert!(args.contains("command"));
    assert_eq!(ends, 1);
}
