//! External tests for the compaction token estimator and trigger (T-005).
//!
//! These exercise the pure estimator/trigger logic from
//! `ragent_agent::compaction::estimator`. The module is compiled into the
//! crate's module tree via the `#[path]` attribute in `estimator.rs` so it can
//! access the crate-private items directly. It lives under `tests/inline/` so
//! cargo does not also try to compile it as a standalone integration test.

use ragent_config::compaction::CompactionConfig;
use ragent_types::llm::{ChatContent, ChatMessage, ToolDefinition};

use crate::compaction::estimator::{
    compaction_threshold, effective_request_tokens, estimate_message_tokens,
    estimate_request_tokens, estimate_text_tokens, evaluate_trigger,
};

fn user_msg(text: &str) -> ChatMessage {
    ChatMessage {
        role: "user".to_string(),
        content: ChatContent::Text(text.to_string()),
    }
}

fn assistant_msg(text: &str) -> ChatMessage {
    ChatMessage {
        role: "assistant".to_string(),
        content: ChatContent::Text(text.to_string()),
    }
}

#[test]
fn test_estimate_text_tokens_rounds() {
    // 4 chars -> 1 token, 8 chars -> 2 tokens, 0 -> 0.
    assert_eq!(estimate_text_tokens(""), 0);
    assert_eq!(estimate_text_tokens("abcd"), 1);
    assert_eq!(estimate_text_tokens("abcdefgh"), 2);
}

#[test]
fn test_estimate_message_tokens_includes_overhead() {
    let msg = user_msg("abcdefgh"); // role 4 + content 8 = 12 bytes -> round(12/4)=3 + 10 overhead
    assert_eq!(estimate_message_tokens(&msg), 13);
}

#[test]
fn test_estimate_request_tokens_sums_components() {
    let system = "abcdabcd"; // 8 bytes -> round(8/4)=2 tokens
    let messages = vec![user_msg("abcd"), assistant_msg("abcd")];
    let tools: Vec<ToolDefinition> = vec![];
    let total = estimate_request_tokens(Some(system), &messages, &tools);
    // system 2 + user_msg: round((4+4)/4)=2 + 10 = 12
    //        + assistant_msg: round((9+4)/4)=3 + 10 = 13  => 2 + 12 + 13 = 27
    assert_eq!(total, 27);
}

#[test]
fn test_estimate_request_tokens_with_tools() {
    let tool = ToolDefinition {
        name: "read".to_string(),
        description: "Read a file".to_string(),
        parameters: serde_json::json!({"type": "object"}),
    };
    // tools contribute name+desc+params+60 bytes; with the tiny schema above
    // that is 4 (name) + 11 (desc) + 17 (params `{"type":"object"}`) + 60 = 92
    // bytes -> round(92/4) = 23 tokens.
    let total = estimate_request_tokens(None, &[], &[tool]);
    assert_eq!(total, 23);
}

#[test]
fn test_effective_request_tokens_prefers_reported() {
    // Reported > 0 wins.
    assert_eq!(effective_request_tokens(500, 1234), 1234);
    // Zero reported falls back to estimate.
    assert_eq!(effective_request_tokens(500, 0), 500);
}

#[test]
fn test_compaction_threshold_subtracts_max() {
    // max(output, buffer) = max(1000, 20000) = 20000
    assert_eq!(compaction_threshold(100_000, 1_000, 20_000, None), 80_000);
    // output larger than buffer
    assert_eq!(compaction_threshold(100_000, 30_000, 20_000, None), 70_000);
    // saturates at zero
    assert_eq!(compaction_threshold(1_000, 30_000, 20_000, None), 0);
}

#[test]
fn test_compaction_threshold_uses_percentage_when_set() {
    // 80% of a 100k window fires at 80k regardless of buffer.
    assert_eq!(
        compaction_threshold(100_000, 1_000, 20_000, Some(0.8)),
        80_000
    );
    // 80% of a 32k window fires at 25.6k — NOT `window - buffer` (12k).
    assert_eq!(
        compaction_threshold(32_000, 1_000, 20_000, Some(0.8)),
        25_600
    );
    // Out-of-range fractions fall back to the buffer model.
    assert_eq!(
        compaction_threshold(100_000, 1_000, 20_000, Some(2.0)),
        80_000
    );
}

#[test]
fn test_evaluate_trigger_fires_when_over_threshold() {
    let config = CompactionConfig {
        buffer: 20_000,
        ..Default::default()
    };
    // context 100k, output 8k, buffer 20k -> threshold 80k.
    let decision = evaluate_trigger(&config, 90_000, 0, 100_000, 8_000);
    assert!(decision.should_compact);
    assert_eq!(decision.threshold, 80_000);
    assert_eq!(decision.effective_tokens, 90_000);
}

#[test]
fn test_evaluate_trigger_no_fire_when_under_threshold() {
    let config = CompactionConfig::default();
    let decision = evaluate_trigger(&config, 10_000, 0, 100_000, 8_000);
    assert!(!decision.should_compact);
}

#[test]
fn test_evaluate_trigger_prefers_reported_tokens() {
    let config = CompactionConfig {
        buffer: 20_000,
        ..Default::default()
    };
    // Estimate is small (1k) but provider reported 95k -> should fire.
    let decision = evaluate_trigger(&config, 1_000, 95_000, 100_000, 8_000);
    assert!(decision.should_compact);
    assert_eq!(decision.effective_tokens, 95_000);
    assert_eq!(decision.estimated_tokens, 1_000);
}

#[test]
fn test_evaluate_trigger_boundary_not_inclusive() {
    let config = CompactionConfig {
        buffer: 20_000,
        ..Default::default()
    };
    // effective == threshold -> not > threshold, so no fire.
    let decision = evaluate_trigger(&config, 80_000, 0, 100_000, 8_000);
    assert!(!decision.should_compact);
}

#[test]
fn test_evaluate_trigger_honors_percentage_threshold() {
    // User configured 80% (migrated from `compression.auto_threshold: 0.8`).
    // On a 32k window the percentage threshold is 25.6k, not the buffer-based
    // 12k (`window - buffer`) that would fire far too early.
    let config = CompactionConfig {
        threshold: Some(0.8),
        buffer: 20_000,
        ..Default::default()
    };
    // 20k of 32k = 62.5% usage — below the 80% trigger.
    let below = evaluate_trigger(&config, 20_000, 0, 32_000, 0);
    assert!(!below.should_compact);
    assert_eq!(below.threshold, 25_600);
    // 30k of 32k = 93.75% usage — above the 80% trigger.
    let above = evaluate_trigger(&config, 30_000, 0, 32_000, 0);
    assert!(above.should_compact);
    assert_eq!(above.threshold, 25_600);
}
