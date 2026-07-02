//! Integration tests for Ollama (OpenAI-compatible) streaming tool-call handling.

use std::collections::HashMap;
use std::pin::Pin;
use std::sync::Arc;

use futures::StreamExt;
use ragent_llm::llm::{ChatContent, ChatMessage, ChatRequest, StreamEvent, ToolDefinition};
use ragent_llm::{OllamaProvider, Provider};
use serde_json::{Value, json};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;

fn make_request(model: &str, tools: Vec<ToolDefinition>) -> ChatRequest {
    ChatRequest {
        model: model.to_string(),
        messages: Arc::new(vec![ChatMessage {
            role: "user".to_string(),
            content: ChatContent::Text("Use the get_weather tool for London.".to_string()),
        }]),
        tools: Arc::new(tools),
        temperature: None,
        top_p: None,
        max_tokens: Some(128),
        system: Some(std::sync::Arc::from("system")),
        options: HashMap::new(),
        session_id: None,
        request_id: None,
        stream_timeout_secs: Some(5),
        thinking: None,
    }
}

async fn spawn_ollama_sse_server(response_body: String) -> anyhow::Result<String> {
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

#[tokio::test]
async fn test_ollama_request_includes_tool_choice_and_stream_options() {
    let tools = vec![ToolDefinition {
        name: "get_weather".to_string(),
        description: "Get weather".to_string(),
        parameters: json!({
            "type": "object",
            "properties": {
                "location": {"type": "string"}
            },
            "required": ["location"]
        }),
    }];

    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let url = format!("http://{addr}");

    let (tx, rx) = tokio::sync::oneshot::channel::<Value>();
    tokio::spawn(async move {
        let Ok((mut socket, _)) = listener.accept().await else {
            return;
        };
        let mut buf = vec![0_u8; 4096];
        let Ok(n) = socket.read(&mut buf).await else {
            return;
        };
        let request = String::from_utf8_lossy(&buf[..n]);
        let header_end = request.find("\r\n\r\n").unwrap_or(request.len());
        let body_bytes = &buf[header_end + 4..n];
        let body: Value = serde_json::from_slice(body_bytes).unwrap_or(Value::Null);
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
    let request = make_request("ornith:latest", tools);
    let _stream = client.chat(request).await.expect("chat");
    let body = rx.await.expect("body");

    assert_eq!(
        body["tool_choice"],
        json!("auto"),
        "tool_choice should be auto"
    );
    assert_eq!(
        body["stream_options"],
        json!({"include_usage": true}),
        "stream_options should request usage"
    );
    assert!(
        body["tools"]
            .as_array()
            .map(|a| !a.is_empty())
            .unwrap_or(false)
    );
}

#[tokio::test]
async fn test_ollama_parses_reasoning_and_tool_calls_from_stream() {
    // Simulates the Ornith streaming response observed in the wild: reasoning
    // tokens interleaved, then a single tool_calls frame, then finish.
    let sse = concat!(
        "data: {\"choices\":[{\"index\":0,\"delta\":{\"role\":\"assistant\",\"content\":\"\",\"reasoning\":\"The user wants\"}}]}\n\n",
        "data: {\"choices\":[{\"index\":0,\"delta\":{\"content\":\"\",\"reasoning\":\" weather in London.\"}}]}\n\n",
        "data: {\"choices\":[{\"index\":0,\"delta\":{\"content\":\"\",\"tool_calls\":[{\"id\":\"call_abc\",\"index\":0,\"type\":\"function\",\"function\":{\"name\":\"get_weather\",\"arguments\":\"{\\\"location\\\":\\\"London\\\"}\"}}]}}]}\n\n",
        "data: {\"choices\":[{\"index\":0,\"delta\":{\"content\":\"\"},\"finish_reason\":\"tool_calls\"}]}\n\n",
        "data: [DONE]\n\n"
    );

    let url = spawn_ollama_sse_server(sse.to_string())
        .await
        .expect("server");
    let client = OllamaProvider::with_url(&url)
        .create_client("", None, &HashMap::new())
        .await
        .expect("client");

    let request = make_request(
        "ornith:latest",
        vec![ToolDefinition {
            name: "get_weather".to_string(),
            description: "Get weather".to_string(),
            parameters: json!({"type":"object","properties":{"location":{"type":"string"}},"required":["location"]}),
        }],
    );

    let mut stream: Pin<Box<dyn futures::Stream<Item = StreamEvent> + Send>> =
        client.chat(request).await.expect("chat");

    let mut events = Vec::new();
    while let Some(event) = stream.next().await {
        events.push(event);
    }

    let reasoning_text: String = events
        .iter()
        .filter_map(|e| match e {
            StreamEvent::ReasoningDelta { text } => Some(text.clone()),
            _ => None,
        })
        .collect();
    assert!(
        reasoning_text.contains("The user wants"),
        "reasoning deltas should be emitted; got: {reasoning_text}"
    );

    let tool_starts: Vec<_> = events
        .iter()
        .filter_map(|e| match e {
            StreamEvent::ToolCallStart { id, name } => Some((id.clone(), name.clone())),
            _ => None,
        })
        .collect();
    assert_eq!(tool_starts.len(), 1, "expected one tool call start");
    assert_eq!(tool_starts[0].1, "get_weather");

    let args: String = events
        .iter()
        .filter_map(|e| match e {
            StreamEvent::ToolCallDelta { args_json, .. } => Some(args_json.clone()),
            _ => None,
        })
        .collect();
    assert!(
        args.contains("London"),
        "tool args should contain London; got {args}"
    );

    let finishes: Vec<_> = events
        .iter()
        .filter(|e| matches!(e, StreamEvent::Finish { .. }))
        .collect();
    assert_eq!(finishes.len(), 1, "expected a finish event");
}
