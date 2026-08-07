//! External tests for the compaction runner (T-007).
//!
//! Compiled into the crate's module tree via the `#[path]` attribute in
//! `runner.rs` so they can access crate-private items. Lives under
//! `tests/inline/` so cargo does not also compile it as a standalone
//! integration test.

use std::sync::Arc;

use ragent_config::{CompactionConfig, StreamConfig};
use ragent_llm::llm::{ChatRequest, LlmClient, StreamEvent};
use ragent_llm::providers::mock_llm_client::{MockLlmClient, MockScenario};
use ragent_types::event::EventBus;
use ragent_types::message::{Message, Role};

use crate::compaction::runner::{
    build_compaction_message, build_summary_request, compact, select, summarize_via_client,
};

fn user_msg(text: &str) -> Message {
    Message::user_text("sess", text)
}

fn assistant_msg(text: &str) -> Message {
    Message::assistant_text("sess", text)
}

#[test]
fn test_select_keeps_recent_tail_within_budget() {
    // keep.tokens default = 0.20; on a 100k window that is 20k tokens. Each
    // short message is ~1-3 tokens, so all five messages fit in the recent tail
    // and the head is empty.
    let messages = vec![
        user_msg("aaaa"),
        assistant_msg("bbbb"),
        user_msg("cccc"),
        assistant_msg("dddd"),
        user_msg("eeee"),
    ];
    let config = CompactionConfig::default();
    let split = select(&messages, &config, 100_000);
    assert!(split.head_messages.is_empty());
    assert_eq!(split.recent_messages.len(), 5);
    assert!(split.recent_tokens > 0);
}

#[test]
fn test_select_splits_when_budget_exceeded() {
    // A tiny keep fraction forces a split: only the last message fits.
    let messages = vec![user_msg("aaaa"), assistant_msg("bbbb"), user_msg("cccc")];
    let config = CompactionConfig {
        keep: ragent_config::compaction::KeepConfig { tokens: Some(0.0) },
        ..Default::default()
    };
    let split = select(&messages, &config, 100_000);
    // With a zero budget, only the last message is kept verbatim.
    assert_eq!(split.recent_messages.len(), 1);
    assert_eq!(split.recent_messages[0].text_content(), "cccc");
    assert_eq!(split.head_messages.len(), 2);
}

#[test]
fn test_select_drops_compaction_messages() {
    let mut messages = vec![user_msg("aaaa"), assistant_msg("bbbb"), user_msg("cccc")];
    // Insert a compaction message in the middle; select should ignore it.
    messages.insert(1, build_compaction_message("sess", "old summary"));
    let config = CompactionConfig::default();
    let split = select(&messages, &config, 100_000);
    // The compaction message is dropped from both head and recent.
    assert!(
        split
            .recent_messages
            .iter()
            .all(|m| m.role != Role::Compaction)
    );
    assert!(
        split
            .head_messages
            .iter()
            .all(|m| m.role != Role::Compaction)
    );
}

#[test]
fn test_select_empty_history() {
    let config = CompactionConfig::default();
    let split = select(&[], &config, 100_000);
    assert!(split.head_messages.is_empty());
    assert!(split.recent_messages.is_empty());
    assert_eq!(split.recent_tokens, 0);
}

#[test]
fn test_select_respects_fraction_on_small_window() {
    // keep fraction 0.20 on a 1000-token window gives 200 tokens. With messages
    // that sum to more than 200 tokens, only the last message should be kept.
    let messages = vec![
        user_msg("a".repeat(400).as_str()),
        assistant_msg("b".repeat(400).as_str()),
        user_msg("c".repeat(400).as_str()),
    ];
    let config = CompactionConfig::default();
    let split = select(&messages, &config, 1_000);
    assert_eq!(split.recent_messages.len(), 1);
    assert_eq!(split.head_messages.len(), 2);
}

#[test]
fn test_build_compaction_message_has_compaction_role() {
    let msg = build_compaction_message("sess", "## Objective\n- do thing");
    assert_eq!(msg.role, Role::Compaction);
    assert_eq!(msg.text_content(), "## Objective\n- do thing");
    assert_eq!(msg.session_id, "sess");
}

#[test]
fn test_build_summary_request_is_single_user_message_no_tools() {
    let req = build_summary_request("claude-sonnet", "summarise this", 4096, Some(120));
    assert_eq!(req.model, "claude-sonnet");
    assert_eq!(req.messages.len(), 1);
    assert_eq!(req.messages[0].role, "user");
    assert!(req.tools.is_empty());
    assert_eq!(req.max_tokens, Some(4096));
    assert!(req.system.is_none());
    assert_eq!(req.stream_timeout_secs, Some(120));
}

#[tokio::test]
async fn test_summarize_via_client_collects_text_deltas() {
    let client: Arc<dyn LlmClient> =
        Arc::new(MockLlmClient::with_scenario(MockScenario::SimpleTextReply));
    let request = build_summary_request("mock-model", "summarise", 4096, None);
    let summary = summarize_via_client(
        &client,
        request,
        &StreamConfig::default(),
        &EventBus::new(8),
        "sess",
    )
    .await
    .unwrap();
    // SimpleTextReply emits "Hello, world from the mock LLM client. Bye!\n".
    assert!(summary.contains("Hello, world from the mock LLM client"));
}

/// A mock LlmClient that emits an Error event, used to verify the error path.
struct ErrorClient;
#[async_trait::async_trait]
impl LlmClient for ErrorClient {
    async fn chat(
        &self,
        _request: ChatRequest,
    ) -> anyhow::Result<std::pin::Pin<Box<dyn futures::Stream<Item = StreamEvent> + Send>>> {
        let stream = tokio_stream::iter(vec![StreamEvent::Error {
            message: "boom".to_string(),
        }]);
        Ok(Box::pin(stream))
    }
}

#[tokio::test]
async fn test_summarize_via_client_propagates_error_event() {
    let client: Arc<dyn LlmClient> = Arc::new(ErrorClient);
    let request = build_summary_request("mock", "x", 4096, Some(60));
    let result = summarize_via_client(
        &client,
        request,
        &StreamConfig::default(),
        &EventBus::new(8),
        "sess",
    )
    .await;
    assert!(result.is_err());
    assert!(
        result
            .unwrap_err()
            .to_string()
            .contains("compaction summarisation failed")
    );
}

#[tokio::test]
async fn test_compact_replaces_history_with_summary_and_recent() {
    // Build a history large enough to force a head/recent split when
    // keep_tokens is small, so we exercise both the summarisation call and the
    // verbatim-tail preservation.
    let messages = vec![
        user_msg("earliest user message about rust"),
        assistant_msg("earliest assistant reply explaining ownership"),
        user_msg("middle user question about lifetimes"),
        assistant_msg("middle assistant reply with examples"),
        user_msg("latest user asking to fix a bug"),
    ];
    let config = CompactionConfig {
        keep: ragent_config::compaction::KeepConfig { tokens: Some(0.0) },
        ..Default::default()
    };
    let client: Arc<dyn LlmClient> =
        Arc::new(MockLlmClient::with_scenario(MockScenario::SimpleTextReply));
    let bus = EventBus::new(64);

    let outcome = compact(
        "sess",
        messages,
        "mock-model",
        100_000,
        8_000,
        &config,
        None,
        &client,
        &bus,
        "auto",
        &StreamConfig::default(),
    )
    .await
    .expect("compact should succeed");

    // The new message list starts with a compaction message.
    assert_eq!(outcome.new_messages[0].role, Role::Compaction);
    assert_eq!(outcome.compaction_message.role, Role::Compaction);
    // The summary is the mock client's canned text (trimmed).
    assert!(
        outcome
            .summary
            .contains("Hello, world from the mock LLM client")
    );
    // The verbatim recent tail follows the compaction message.
    assert!(outcome.new_messages.len() > 1);
    assert!(
        outcome
            .new_messages
            .iter()
            .skip(1)
            .any(|m| m.text_content().contains("latest user asking to fix a bug"))
    );
    // The earliest messages are no longer present verbatim (they were
    // summarised into the compaction message).
    assert!(outcome.new_messages.iter().skip(1).all(|m| {
        !m.text_content()
            .contains("earliest user message about rust")
    }));
}

#[tokio::test]
async fn test_compact_bails_when_nothing_to_summarise() {
    // A single small message with a zero keep budget: head is empty, no
    // previous summary -> nothing to summarise.
    let messages = vec![user_msg("only message")];
    let config = CompactionConfig {
        keep: ragent_config::compaction::KeepConfig { tokens: Some(0.0) },
        ..Default::default()
    };
    let client: Arc<dyn LlmClient> =
        Arc::new(MockLlmClient::with_scenario(MockScenario::SimpleTextReply));
    let bus = EventBus::new(64);
    let result = compact(
        "sess",
        messages,
        "mock-model",
        100_000,
        8_000,
        &config,
        None,
        &client,
        &bus,
        "auto",
        &StreamConfig::default(),
    )
    .await;
    assert!(result.is_err());
    assert!(
        result
            .unwrap_err()
            .to_string()
            .contains("nothing to summarise")
    );
}

#[tokio::test]
async fn test_compact_bails_on_empty_summary() {
    // MockScenario::Empty emits no text deltas, just a Finish.
    let messages = vec![
        user_msg("first"),
        assistant_msg("second"),
        user_msg("third"),
    ];
    let config = CompactionConfig {
        keep: ragent_config::compaction::KeepConfig { tokens: Some(0.0) },
        ..Default::default()
    };
    let client: Arc<dyn LlmClient> = Arc::new(MockLlmClient::with_scenario(MockScenario::Empty));
    let bus = EventBus::new(64);
    let result = compact(
        "sess",
        messages,
        "mock-model",
        100_000,
        8_000,
        &config,
        None,
        &client,
        &bus,
        "auto",
        &StreamConfig::default(),
    )
    .await;
    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("empty summary"));
}

#[tokio::test]
async fn test_compact_publishes_started_and_finished_events() {
    let messages = vec![
        user_msg("first message"),
        assistant_msg("second message"),
        user_msg("third message"),
    ];
    let config = CompactionConfig {
        keep: ragent_config::compaction::KeepConfig { tokens: Some(0.0) },
        ..Default::default()
    };
    let client: Arc<dyn LlmClient> =
        Arc::new(MockLlmClient::with_scenario(MockScenario::SimpleTextReply));
    let bus = EventBus::new(64);
    let mut rx = bus.subscribe();

    let _ = compact(
        "sess",
        messages,
        "mock-model",
        100_000,
        8_000,
        &config,
        None,
        &client,
        &bus,
        "auto",
        &StreamConfig::default(),
    )
    .await
    .unwrap();

    // Drain events and confirm both CompressionStarted and CompressionFinished
    // were published.
    let mut saw_started = false;
    let mut saw_finished = false;
    while let Ok(event) = rx.try_recv() {
        match event {
            ragent_types::event::Event::CompressionStarted { .. } => saw_started = true,
            ragent_types::event::Event::CompressionFinished { .. } => saw_finished = true,
            _ => {}
        }
    }
    assert!(saw_started, "expected CompressionStarted event");
    assert!(saw_finished, "expected CompressionFinished event");
}
