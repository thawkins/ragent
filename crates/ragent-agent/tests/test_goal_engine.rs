//! Tests-only coverage for the goal-driven loop evaluator and context builder
//! (`ragent_agent::goal`).
//!
//! A hermetic scripted [`LlmClient`] returns canned `SATISFIED/CONFIDENCE/
//! REASONING` replies so [`GoalEvaluator::evaluate`] and its parser are fully
//! deterministic - no network, no provider credentials.

use std::pin::Pin;
use std::sync::Arc;

use anyhow::Result;
use futures::stream;
use ragent_agent::agent::ModelRef;
use ragent_agent::goal::{GoalCondition, GoalEvaluator, build_evaluation_context};
use ragent_agent::llm::{ChatRequest, LlmClient, StreamEvent};
use ragent_agent::message::{Message, MessagePart, Role};

/// An [`LlmClient`] that replays one canned text body (or an error).
struct ScriptedClient {
    reply: String,
    fail: bool,
}

#[async_trait::async_trait]
impl LlmClient for ScriptedClient {
    async fn chat(
        &self,
        _request: ChatRequest,
    ) -> Result<Pin<Box<dyn futures::Stream<Item = StreamEvent> + Send>>> {
        if self.fail {
            return Err(anyhow::anyhow!("provider unavailable"));
        }
        let events = vec![
            StreamEvent::TextDelta {
                text: self.reply.clone(),
            },
            StreamEvent::Finish {
                reason: ragent_agent::llm::LlmFinishReason::Stop,
            },
        ];
        Ok(Box::pin(stream::iter(events)))
    }
}

fn model_ref() -> ModelRef {
    ModelRef {
        provider_id: "ollama".to_string(),
        model_id: "qwen3:latest".to_string(),
    }
}

fn evaluator(reply: &str, fail: bool) -> GoalEvaluator {
    GoalEvaluator::new(
        model_ref(),
        Arc::new(ScriptedClient {
            reply: reply.to_string(),
            fail,
        }),
    )
}

#[test]
fn goal_condition_summary_and_record_evaluation() {
    let mut goal = GoalCondition::new("all tests pass");
    assert!(!goal.satisfied);
    assert_eq!(goal.evaluation_count, 0);
    let unsat = goal.summary();
    assert!(unsat.contains("[x] not satisfied"), "summary: {unsat}");
    assert!(unsat.contains("never"), "summary: {unsat}");

    goal.record_evaluation(true, Some("green".to_string()));
    assert!(goal.satisfied);
    assert_eq!(goal.evaluation_count, 1);
    let sat = goal.summary();
    assert!(sat.contains("[ok] satisfied"), "summary: {sat}");
    assert!(!sat.contains("never"), "last_evaluated should be stamped");
}

#[tokio::test]
async fn evaluate_parses_satisfied_yes() {
    let ev = evaluator(
        "SATISFIED: YES\nCONFIDENCE: 0.92\nREASONING: build is green",
        false,
    );
    let out = ev
        .evaluate(&GoalCondition::new("g"), "context")
        .await
        .expect("evaluate");
    assert!(out.satisfied);
    assert_eq!(out.confidence, Some(0.92));
    assert!(out.reasoning.contains("build is green"));
}

#[tokio::test]
async fn evaluate_parses_satisfied_false() {
    let ev = evaluator("SATISFIED: NO\nCONFIDENCE: 0.1\nREASONING: not yet", false);
    let out = ev
        .evaluate(&GoalCondition::new("g"), "")
        .await
        .expect("eval");
    assert!(!out.satisfied);
    assert_eq!(out.confidence, Some(0.1));
    assert!(out.reasoning.contains("not yet"));
}

#[tokio::test]
async fn evaluate_falls_back_to_raw_text_when_unstructured() {
    let ev = evaluator("the goal looks basically done to me", false);
    let out = ev
        .evaluate(&GoalCondition::new("g"), "")
        .await
        .expect("eval");
    assert!(!out.satisfied);
    assert!(out.confidence.is_none());
    assert!(out.reasoning.contains("basically done"));
}

#[tokio::test]
async fn evaluate_propagates_provider_error() {
    let ev = evaluator("ignored", true);
    let err = ev
        .evaluate(&GoalCondition::new("g"), "")
        .await
        .expect_err("provider error must propagate");
    assert!(err.to_string().contains("provider unavailable"));
}

#[test]
fn build_evaluation_context_includes_recent_messages_within_budget() {
    let messages = vec![
        Message::new(
            "s1",
            Role::User,
            vec![MessagePart::Text {
                text: "first user turn".to_string(),
            }],
        ),
        Message::new(
            "s1",
            Role::Assistant,
            vec![MessagePart::Text {
                text: "assistant reply with detail".to_string(),
            }],
        ),
    ];

    let ctx = build_evaluation_context(&messages, 4096);
    assert!(ctx.contains("[User] first user turn"), "ctx: {ctx}");
    assert!(
        ctx.contains("[Assistant] assistant reply with detail"),
        "ctx: {ctx}"
    );
}

#[test]
fn build_evaluation_context_respects_a_tight_byte_budget() {
    let big = "x".repeat(1000);
    let messages = vec![Message::new(
        "s1",
        Role::User,
        vec![MessagePart::Text { text: big }],
    )];

    let ctx = build_evaluation_context(&messages, 32);
    assert!(ctx.len() <= 32, "byte budget exceeded: {} bytes", ctx.len());
}

#[test]
fn build_evaluation_context_on_empty_history_is_empty() {
    assert_eq!(build_evaluation_context(&[], 128), "");
}
