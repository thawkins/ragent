//! Integration test: VCR cassette record/playback with a mock provider.
//!
//! **Spec:** `rig` — T-015 / FR-026.
//!
//! This test records a [`MockCompletionModel`] response to a temporary cassette
//! and then replays it through [`VcrClient`] without touching the inner client.
//! It verifies that cassettes produce identical event streams and that missing
//! interactions fail deterministically.

use futures::StreamExt;
use ragent_llm::llm::{ChatRequest, LlmClient, StreamEvent};
use ragent_rig::testing::{MockResponse, build_mock_llm_client};
use ragent_rig::vcr::{VcrClient, VcrMode};
use ragent_types::event::FinishReason;
use ragent_types::llm::{ChatContent, ChatMessage};
use std::collections::HashMap;
use std::sync::Arc;

fn make_request(text: &str) -> ChatRequest {
    ChatRequest {
        model: "rig-vcr-mock".to_owned(),
        messages: Arc::new(vec![ChatMessage {
            role: "user".to_owned(),
            content: ChatContent::Text(text.to_owned()),
        }]),
        tools: Arc::new(Vec::new()),
        temperature: None,
        top_p: None,
        max_tokens: None,
        system: None,
        options: HashMap::new(),
        session_id: None,
        request_id: None,
        stream_timeout_secs: None,
        thinking: None,
    }
}

#[tokio::test]
async fn vcr_records_mock_response_and_replays_it() {
    let dir = tempfile::tempdir().unwrap();
    let cassette = dir.path().join("interaction.json");

    let inner = build_mock_llm_client("rig-vcr-mock", MockResponse::text("hello from cassette"));
    let recorder = VcrClient::new(Box::new(inner), VcrMode::Record(cassette.clone()))
        .await
        .unwrap();

    let mut stream = recorder.chat(make_request("hi")).await.unwrap();
    let mut events = Vec::new();
    while let Some(ev) = stream.next().await {
        events.push(ev);
    }
    assert!(
        events
            .iter()
            .any(|e| matches!(e, StreamEvent::TextDelta { text } if text == "hello from cassette")),
        "recorded events should contain the mock text"
    );
    assert!(
        events.iter().any(|e| matches!(
            e,
            StreamEvent::Finish {
                reason: FinishReason::Stop
            }
        )),
        "recorded events should end with Finish(Stop)"
    );

    // Replay from the saved cassette with a panicking inner client to prove
    // playback does not touch the provider.
    let player = VcrClient::new(
        Box::new(PanickingClient),
        VcrMode::Playback(cassette.clone()),
    )
    .await
    .unwrap();
    let mut replayed = Vec::new();
    let mut stream = player.chat(make_request("hi")).await.unwrap();
    while let Some(ev) = stream.next().await {
        replayed.push(ev);
    }
    assert_eq!(replayed.len(), events.len());
    for (a, b) in replayed.iter().zip(events.iter()) {
        assert_eq!(format!("{a:?}"), format!("{b:?}"));
    }
}

#[tokio::test]
async fn vcr_playback_errors_for_unknown_request() {
    let dir = tempfile::tempdir().unwrap();
    let cassette = dir.path().join("empty.json");
    tokio::fs::write(&cassette, br#"{"version":1,"interactions":[]}"#)
        .await
        .unwrap();

    let player = VcrClient::new(
        Box::new(PanickingClient),
        VcrMode::Playback(cassette.clone()),
    )
    .await
    .unwrap();
    let result = player.chat(make_request("not in cassette")).await;
    assert!(
        result.is_err(),
        "unknown request should fail in playback mode"
    );
    match result {
        Err(e) => assert!(
            e.to_string().contains("No cassette interaction matched"),
            "error should mention missing interaction: {e}"
        ),
        Ok(_) => panic!("expected an error"),
    }
}

struct PanickingClient;

#[async_trait::async_trait]
impl LlmClient for PanickingClient {
    async fn chat(
        &self,
        _request: ChatRequest,
    ) -> anyhow::Result<std::pin::Pin<Box<dyn futures::Stream<Item = StreamEvent> + Send>>> {
        panic!("playback should not call the inner client")
    }
}
