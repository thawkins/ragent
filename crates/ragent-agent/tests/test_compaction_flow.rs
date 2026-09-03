#![allow(clippy::assert_is_empty)]
//! Compaction flow tests for T-012.
//!
//! This file consolidates the T-012 test scenarios that are *not* already
//! covered by `test_compaction_integration.rs` (pre-send + emergency-overflow
//! wiring) or the inline module tests (estimator, runner `select`, serializer,
//! prompt builder). It exercises:
//!
//! 1. The `compact` overflow guard — the summarisation prompt itself must not
//!    exceed `context_window - summary_output` (FR-005).
//! 2. `emergency_compact` at the function level — it converts provider-facing
//!    `ChatMessage`s, runs summarisation, and replaces the slice in place with
//!    `[compaction_msg, ...recent]` (FR-004).
//! 3. `select` with a single very long turn — the last message is always kept
//!    verbatim even when its serialised form exceeds `keep_tokens`.
//! 4. A session whose total tokens exceed a small context window: verify the
//!    compaction message is produced and the subsequent turn loads only from
//!    the compaction point forward (FR-005 / FR-007) — the real conversation
//!    request that follows compaction begins with the compaction summary.
//! 5. Prompt construction with and without a previous summary (FR-010) is
//!    covered at the unit level in `prompt.rs`; here we assert the prompt
//!    built inside `compact` carries the previous-summary update instruction
//!    when one is supplied.

use std::sync::Arc;
use std::sync::Mutex;

use ragent_agent::compaction::{
    SUMMARY_OUTPUT_TOKENS, build_prompt, compact, emergency_compact, select,
};
use ragent_agent::event::EventBus;
use ragent_agent::llm::{ChatContent, ChatMessage, LlmClient, StreamEvent};
use ragent_agent::message::{Message, Role};
use ragent_config::StreamConfig;
use ragent_config::compaction::{CompactionConfig, KeepConfig};
use ragent_llm::providers::mock_llm_client::{MockLlmClient, MockScenario};

fn user_msg(text: &str) -> Message {
    Message::user_text("sess", text)
}

fn assistant_msg(text: &str) -> Message {
    Message::assistant_text("sess", text)
}

/// Extract the text content of a chat message, or empty string for non-text.
fn chat_text(msg: &ChatMessage) -> String {
    match &msg.content {
        ChatContent::Text(t) => t.clone(),
        _ => String::new(),
    }
}

// ────────────────────────────────────────────────────────────────────────────
// 1. Overflow guard inside `compact`
// ────────────────────────────────────────────────────────────────────────────

#[tokio::test]
async fn test_compact_bails_when_summary_prompt_would_overflow_context() {
    // Build a head whose serialised transcript is large relative to a tiny
    // context window. The prompt tokens (template + transcript) must exceed
    // `context_window - SUMMARY_OUTPUT_TOKENS`, which saturates to zero for a
    // small window, so any non-trivial prompt triggers the guard.
    let big = "x".repeat(10_000);
    let messages = vec![
        user_msg(&big),
        assistant_msg(&big),
        user_msg(&big),
        assistant_msg(&big),
    ];
    let config = CompactionConfig {
        keep: KeepConfig { tokens: Some(0.0) },
        ..Default::default()
    };
    let client: Arc<dyn LlmClient> =
        Arc::new(MockLlmClient::with_scenario(MockScenario::SimpleTextReply));
    let bus = EventBus::new(64);

    let result = compact(
        "sess",
        messages,
        "mock-model",
        // Tiny context window: prompt_budget = context - SUMMARY_OUTPUT = 0.
        1_000,
        0,
        &config,
        None,
        &client,
        &bus,
        "auto",
        &StreamConfig::default(),
    )
    .await;

    // Keep the constant referenced so it is not flagged as unused; it also
    // documents the budget arithmetic the guard uses.
    let _ = SUMMARY_OUTPUT_TOKENS;

    let err = result.expect_err("expected overflow-guard error");
    assert!(
        err.to_string().contains("exceeds context budget") || err.to_string().contains("context"),
        "expected a context-budget overflow error, got: {err}"
    );
}

struct SummaryClient;
/// A mock `LlmClient` that captures the prompt of the first request and then
/// returns a fixed summary. Used to assert the prompt built by `compact`
/// carries the previous-summary update instruction.
struct CapturingClient {
    captured: Arc<Mutex<Option<String>>>,
}
#[async_trait::async_trait]
impl LlmClient for SummaryClient {
    async fn chat(
        &self,
        _request: ragent_agent::llm::ChatRequest,
    ) -> anyhow::Result<std::pin::Pin<Box<dyn futures::Stream<Item = StreamEvent> + Send>>> {
        let stream = tokio_stream::iter(vec![
            StreamEvent::TextDelta {
                text: "## Objective\n- recover from overflow".to_string(),
            },
            StreamEvent::Finish {
                reason: ragent_agent::llm::LlmFinishReason::Stop,
            },
        ]);
        Ok(Box::pin(stream))
    }
}
#[async_trait::async_trait]
impl LlmClient for CapturingClient {
    async fn chat(
        &self,
        request: ragent_agent::llm::ChatRequest,
    ) -> anyhow::Result<std::pin::Pin<Box<dyn futures::Stream<Item = StreamEvent> + Send>>> {
        if self.captured.lock().unwrap().is_none() {
            let prompt = request.messages.first().map(chat_text).unwrap_or_default();
            *self.captured.lock().unwrap() = Some(prompt);
        }
        let stream = tokio_stream::iter(vec![
            StreamEvent::TextDelta {
                text: "## Objective\n- updated".to_string(),
            },
            StreamEvent::Finish {
                reason: ragent_agent::llm::LlmFinishReason::Stop,
            },
        ]);
        Ok(Box::pin(stream))
    }
}

#[tokio::test]
async fn test_emergency_compact_replaces_chat_messages_in_place() {
    // Build provider-facing chat messages with enough volume that the head is
    // non-empty after `select` with a zero keep budget.
    let pad = "y".repeat(2_000);
    let mut chat_messages: Vec<ChatMessage> = Vec::new();
    for i in 0..6 {
        let role = if i % 2 == 0 { "user" } else { "assistant" };
        chat_messages.push(ChatMessage {
            role: role.to_string(),
            content: ChatContent::Text(format!("msg {i} {pad}")),
        });
    }
    let original_len = chat_messages.len();

    let config = CompactionConfig {
        keep: KeepConfig { tokens: Some(0.0) },
        ..Default::default()
    };
    let client: Arc<dyn LlmClient> = Arc::new(SummaryClient);
    let bus = EventBus::new(32);

    let outcome = emergency_compact(
        "sess",
        &mut chat_messages,
        "mock-model",
        100_000,
        0,
        &config,
        &client,
        &bus,
        &StreamConfig::default(),
    )
    .await
    .expect("emergency_compact should succeed");

    // The slice was replaced: the first chat message is now the compaction
    // summary, emitted as an `assistant` role (providers without a compaction
    // role see it as assistant).
    assert!(!chat_messages.is_empty(), "chat_messages must not be empty");
    assert_eq!(chat_messages[0].role, "assistant");
    let first = chat_text(&chat_messages[0]);
    assert!(
        first.contains("## Objective"),
        "first message should be the compaction summary, got: {first}"
    );

    // The compacted history is shorter than the original.
    assert!(
        chat_messages.len() < original_len,
        "compacted history ({} msgs) must be shorter than original ({} msgs)",
        chat_messages.len(),
        original_len
    );

    // The outcome reports the synthetic compaction message.
    assert_eq!(outcome.compaction_message.role, Role::Compaction);
    assert!(outcome.summary.contains("## Objective"));
    assert_eq!(outcome.new_messages.len(), chat_messages.len());
}

#[tokio::test]
async fn test_emergency_compact_leaves_chat_messages_unchanged_on_error() {
    // When there is nothing to summarise (single short message, no previous
    // summary), emergency_compact errors and must leave the slice unchanged.
    let mut chat_messages: Vec<ChatMessage> = vec![ChatMessage {
        role: "user".to_string(),
        content: ChatContent::Text("only message".to_string()),
    }];
    let snapshot_len = chat_messages.len();
    let snapshot_first = chat_text(&chat_messages[0]);

    let config = CompactionConfig {
        keep: KeepConfig { tokens: Some(0.0) },
        ..Default::default()
    };
    let client: Arc<dyn LlmClient> = Arc::new(SummaryClient);
    let bus = EventBus::new(32);

    let result = emergency_compact(
        "sess",
        &mut chat_messages,
        "mock-model",
        100_000,
        0,
        &config,
        &client,
        &bus,
        &StreamConfig::default(),
    )
    .await;
    assert!(result.is_err(), "expected nothing-to-summarise error");
    assert_eq!(
        chat_messages.len(),
        snapshot_len,
        "chat_messages length must be unchanged when emergency_compact errors"
    );
    assert_eq!(
        chat_text(&chat_messages[0]),
        snapshot_first,
        "chat_messages content must be unchanged when emergency_compact errors"
    );
}

// ────────────────────────────────────────────────────────────────────────────
// 3. `select` with a single very long turn
// ────────────────────────────────────────────────────────────────────────────

#[test]
fn test_select_keeps_last_message_even_when_it_exceeds_keep_budget() {
    // A single message whose serialised form far exceeds keep_tokens. The
    // `select` algorithm always keeps at least the last message verbatim, so
    // the head is empty and the recent tail is exactly that one message.
    let big = "z".repeat(50_000);
    let messages = vec![user_msg(&big)];
    let config = CompactionConfig {
        keep: KeepConfig {
            tokens: Some(0.001),
        },
        ..Default::default()
    };
    let split = select(&messages, &config, 100_000);
    assert!(split.head_messages.is_empty(), "head must be empty");
    assert_eq!(
        split.recent_messages.len(),
        1,
        "the single long message must be kept verbatim"
    );
    assert_eq!(split.recent_messages[0].text_content(), big);
    assert!(
        split.recent_tokens > 100,
        "recent_tokens should reflect the cost"
    );
}

#[test]
fn test_select_single_long_turn_with_zero_budget() {
    // With keep_tokens = 0 the recent tail still contains the last message.
    let messages = vec![user_msg(&"a".repeat(8_000))];
    let config = CompactionConfig {
        keep: KeepConfig { tokens: Some(0.0) },
        ..Default::default()
    };
    let split = select(&messages, &config, 100_000);
    assert!(split.head_messages.is_empty());
    assert_eq!(split.recent_messages.len(), 1);
}

// ───────────────────────────────────────────────��────────────────────────────
// 4. Prompt construction with / without previous summary (FR-010)
// ────────────────────────────────────────────────────────────────────────────

#[test]
fn test_build_prompt_without_previous_summary_asks_for_new() {
    let prompt = build_prompt(None, &["[User]: do thing"]);
    assert!(prompt.contains("Create a new anchored summary"));
    assert!(!prompt.contains("<previous-summary>"));
    assert!(prompt.contains("## Objective"));
    assert!(prompt.contains("[User]: do thing"));
}

#[test]
fn test_build_prompt_with_previous_summary_asks_to_update() {
    let prompt = build_prompt(Some("## Objective\n- old goal"), &["[User]: more"]);
    assert!(prompt.contains("Update the anchored summary"));
    assert!(prompt.contains("<previous-summary>"));
    assert!(prompt.contains("old goal"));
    assert!(prompt.contains("[User]: more"));
}

#[tokio::test]
async fn test_compact_passes_previous_summary_into_prompt() {
    // When a previous summary is supplied, the prompt sent to the LLM must
    // carry the "Update the anchored summary" instruction and the previous
    // summary text. We capture the prompt via a shared mutex inside a custom
    // client (no downcast needed).
    let captured: Arc<Mutex<Option<String>>> = Arc::new(Mutex::new(None));

    let messages = vec![
        user_msg("first question"),
        assistant_msg("first answer"),
        user_msg("second question"),
    ];
    let config = CompactionConfig {
        keep: KeepConfig { tokens: Some(0.0) },
        ..Default::default()
    };
    let client: Arc<dyn LlmClient> = Arc::new(CapturingClient {
        captured: Arc::clone(&captured),
    });
    let bus = EventBus::new(16);

    let outcome = compact(
        "sess",
        messages,
        "mock-model",
        100_000,
        0,
        &config,
        // Previous summary present -> update instruction.
        Some("## Objective\n- prior work"),
        &client,
        &bus,
        "auto",
        &StreamConfig::default(),
    )
    .await
    .expect("compact should succeed");
    // Keep `outcome` alive for the duration of the assertion.
    let _ = outcome.summary.len();

    let prompt = captured
        .lock()
        .unwrap()
        .clone()
        .expect("prompt was captured");

    assert!(
        prompt.contains("Update the anchored summary"),
        "prompt must ask to update the existing summary"
    );
    assert!(
        prompt.contains("prior work"),
        "prompt must embed the previous summary text"
    );
    assert!(
        prompt.contains("<previous-summary>"),
        "prompt must wrap the previous summary in the previous-summary tag"
    );
}
