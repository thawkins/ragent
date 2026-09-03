//! Goal-based autonomous stop hook — evaluator model call + goal management.
//!
//! This module implements FR-011 from the piegap spec: a goal-based stop hook
//! that evaluates a user-defined goal condition after each agent turn and
//! halts autonomous execution when the goal is satisfied.
//!
//! # Overview
//!
//! The goal system allows users to set a high-level goal (e.g., "stop when all
//! tests pass" or "stop when the feature is implemented") and have the agent
//! automatically evaluate whether the goal has been met after each turn. This
//! enables more autonomous operation while maintaining safety through explicit
//! goal satisfaction checking.
//!
//! # Architecture
//!
//! ```text
//! User sets goal → GoalCondition stored in session
//!                      ↓
//! After each turn → GoalEvaluator calls LLM
//!                      ↓
//! Goal satisfied? → Yes: halt autonomous execution
//!                   No: continue
//! ```
//!
//! # Goal Structure
//!
//! Each goal consists of:
//! - **Description**: Natural-language description of the goal condition
//! - **Created at**: Timestamp when the goal was set
//! - **Evaluation count**: Number of times the goal has been evaluated
//! - **Last evaluated**: Timestamp of the most recent evaluation
//! - **Satisfied**: Whether the goal has been marked as satisfied

use chrono::{DateTime, Utc};
use futures::StreamExt;
use serde::{Deserialize, Serialize};
use std::sync::Arc;

use crate::agent::ModelRef;
use crate::llm::{ChatContent, ChatMessage, ChatRequest, LlmClient, StreamEvent};
use ragent_types::message::{Message, MessagePart, Role};

/// A goal condition that can be evaluated by the LLM.
///
/// Goals are set by the user via `/goal set <description>` and evaluated
/// after each agent turn to determine if autonomous execution should halt.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GoalCondition {
    /// Natural-language description of the goal condition.
    pub description: String,
    /// When the goal was created.
    pub created_at: DateTime<Utc>,
    /// Number of times the goal has been evaluated.
    pub evaluation_count: u32,
    /// When the goal was last evaluated.
    pub last_evaluated: Option<DateTime<Utc>>,
    /// Whether the goal has been satisfied.
    pub satisfied: bool,
    /// Optional reasoning from the LLM about why the goal is/isn't satisfied.
    pub last_reasoning: Option<String>,
}

impl GoalCondition {
    /// Creates a new goal with the given description.
    pub fn new(description: impl Into<String>) -> Self {
        Self {
            description: description.into(),
            created_at: Utc::now(),
            evaluation_count: 0,
            last_evaluated: None,
            satisfied: false,
            last_reasoning: None,
        }
    }

    /// Records an evaluation of the goal.
    pub fn record_evaluation(&mut self, satisfied: bool, reasoning: Option<String>) {
        self.evaluation_count = self.evaluation_count.saturating_add(1);
        self.last_evaluated = Some(Utc::now());
        self.satisfied = satisfied;
        self.last_reasoning = reasoning;
    }

    /// Returns a summary of the goal for display.
    pub fn summary(&self) -> String {
        let status = if self.satisfied {
            "✓ satisfied"
        } else {
            "✗ not satisfied"
        };
        format!(
            "Goal: {}\nStatus: {}\nEvaluated: {} times\nLast: {}",
            self.description,
            status,
            self.evaluation_count,
            self.last_evaluated
                .map(|t| t.format("%Y-%m-%d %H:%M:%S UTC").to_string())
                .unwrap_or_else(|| "never".to_string())
        )
    }
}

/// Result of evaluating a goal condition.
#[derive(Debug, Clone)]
pub struct GoalEvaluation {
    /// Whether the goal is satisfied.
    pub satisfied: bool,
    /// Reasoning from the LLM about the evaluation.
    pub reasoning: String,
    /// Confidence score (0.0–1.0) if provided by the evaluator.
    pub confidence: Option<f64>,
}

/// Evaluator that checks if a goal condition is satisfied.
///
/// The evaluator uses a lightweight LLM call to assess whether the current
/// state satisfies the goal condition. It is designed to be called after each
/// agent turn during autonomous execution.
pub struct GoalEvaluator {
    /// The model to use for evaluation.
    model_ref: ModelRef,
    /// The LLM client for making evaluation calls.
    client: Arc<dyn LlmClient>,
}

impl GoalEvaluator {
    /// Creates a new goal evaluator with the given model and client.
    pub fn new(model_ref: ModelRef, client: Arc<dyn LlmClient>) -> Self {
        Self { model_ref, client }
    }

    /// Evaluates whether the goal condition is satisfied given the current context.
    ///
    /// The evaluation uses a structured prompt that asks the LLM to:
    /// 1. Review the goal description
    /// 2. Examine the conversation history and tool results
    /// 3. Determine if the goal has been achieved
    /// 4. Provide reasoning for the decision
    ///
    /// # Arguments
    ///
    /// * `goal` - The goal condition to evaluate
    /// * `context` - Summary of the conversation context (recent messages, tool results)
    ///
    /// # Returns
    ///
    /// Returns a `GoalEvaluation` with the satisfaction status and reasoning.
    pub async fn evaluate(
        &self,
        goal: &GoalCondition,
        context: &str,
    ) -> anyhow::Result<GoalEvaluation> {
        let system_prompt = r"You are a goal evaluator for an AI coding agent. Your task is to determine whether a user-defined goal has been satisfied based on the conversation history and tool results.

Evaluate the goal objectively and conservatively. Only mark the goal as satisfied if there is clear evidence that the condition has been met. If there is any doubt, err on the side of continuing execution.

Respond in the following format:
SATISFIED: <YES or NO>
CONFIDENCE: <0.0 to 1.0>
REASONING: <Your explanation>";

        let user_prompt = format!(
            r"GOAL: {}

CONTEXT:
{}

Evaluate whether the goal has been satisfied. Be conservative - only mark as satisfied if there is clear evidence.",
            goal.description, context
        );

        let messages = vec![
            ChatMessage {
                role: "system".to_string(),
                content: ChatContent::Text(system_prompt.to_string()),
            },
            ChatMessage {
                role: "user".to_string(),
                content: ChatContent::Text(user_prompt),
            },
        ];

        let request = ChatRequest {
            model: self.model_ref.model_id.clone(),
            messages: Arc::new(messages),
            temperature: Some(0.1), // Low temperature for consistent evaluation
            top_p: None,
            max_tokens: Some(500),
            system: Some(Arc::from("")),
            options: std::collections::HashMap::new(),
            session_id: None,
            request_id: None,
            stream_timeout_secs: None,
            thinking: None,
            tools: Arc::new(Vec::new()),
        };

        // Call the LLM and collect the streaming response
        let mut stream = self.client.chat(request).await?;
        let mut full_text = String::new();

        while let Some(event) = stream.next().await {
            match event {
                StreamEvent::TextDelta { text } => {
                    full_text.push_str(&text);
                }
                StreamEvent::Error { message, .. } => {
                    return Err(anyhow::anyhow!("LLM error: {}", message));
                }
                _ => {}
            }
        }

        self.parse_evaluation(&full_text)
    }

    /// Parses the LLM response into a structured evaluation.
    fn parse_evaluation(&self, text: &str) -> anyhow::Result<GoalEvaluation> {
        let mut satisfied = false;
        let mut confidence = None;
        let mut reasoning = String::new();

        for line in text.lines() {
            let line = line.trim();
            if line.starts_with("SATISFIED:") {
                let value = line.strip_prefix("SATISFIED:").unwrap_or("").trim();
                satisfied = value.eq_ignore_ascii_case("YES") || value.eq_ignore_ascii_case("TRUE");
            } else if line.starts_with("CONFIDENCE:") {
                let value = line.strip_prefix("CONFIDENCE:").unwrap_or("").trim();
                confidence = value.parse::<f64>().ok();
            } else if line.starts_with("REASONING:") {
                reasoning = line
                    .strip_prefix("REASONING:")
                    .unwrap_or("")
                    .trim()
                    .to_string();
            } else if !reasoning.is_empty() {
                reasoning.push('\n');
                reasoning.push_str(line);
            }
        }

        // If no reasoning was found, use the full text
        if reasoning.is_empty() {
            reasoning = text.trim().to_string();
        }

        Ok(GoalEvaluation {
            satisfied,
            reasoning,
            confidence,
        })
    }
}

/// Builds a context summary from recent messages for goal evaluation.
///
/// This function extracts the most relevant information from the conversation
/// history to help the evaluator understand what has been accomplished.
///
/// # Performance (FR-012)
///
/// The context string is built in a **single pass**:
/// - The output buffer is pre-sized to `max_bytes` (capped at 8 KiB) to
///   avoid repeated reallocations.
/// - Text parts are written directly into the buffer with `push_str`,
///   avoiding the intermediate `Vec<&str>` + `join` that the previous
///   implementation allocated for every message.
/// - The `"[role] text\n"` framing is written with individual `push`
///   calls instead of `format!`, eliminating a per-message `String`
///   allocation.
/// - `max_bytes` is the total byte budget for the evaluation context. The
///   counter (`byte_count`) tracks bytes consumed, not Unicode scalar
///   values, so the budget is a hard byte limit.
pub fn build_evaluation_context(messages: &[Message], max_bytes: usize) -> String {
    // FR-012: pre-size the buffer to avoid repeated reallocations.
    let cap = max_bytes.min(8 * 1024);
    let mut context = String::with_capacity(cap);
    let mut byte_count = 0usize;

    // Start from the most recent messages and work backwards so the
    // evaluator sees the latest context first.
    for msg in messages.iter().rev() {
        if byte_count >= max_bytes {
            break;
        }

        let role_str = match msg.role {
            Role::User => "User",
            Role::Assistant => "Assistant",
            Role::Compaction => "System",
        };

        // FR-012: write text parts directly into the output buffer,
        // avoiding an intermediate Vec and a per-message format! call.
        let mut wrote_any = false;

        for part in &msg.parts {
            let MessagePart::Text { text } = part else {
                continue;
            };
            if text.is_empty() {
                continue;
            }

            if !wrote_any {
                // First text part for this message — write the role prefix.
                context.push('[');
                context.push_str(role_str);
                context.push_str("] ");
                byte_count += role_str.len() + 4; // "[]" + " " + "\n"
                wrote_any = true;
            } else {
                // Separator between concatenated text parts.
                context.push(' ');
                byte_count += 1;
            }

            // Truncate to the remaining budget, ending at a char boundary.
            let remaining = max_bytes.saturating_sub(byte_count);
            if remaining == 0 {
                break;
            }
            let chunk = if text.len() <= remaining {
                text.as_str()
            } else {
                // Walk back to the nearest UTF-8 char boundary.
                let mut end = remaining;
                while end > 0 && !text.is_char_boundary(end) {
                    end -= 1;
                }
                &text[..end]
            };
            context.push_str(chunk);
            byte_count += chunk.len();

            if byte_count >= max_bytes {
                break;
            }
        }

        if wrote_any {
            context.push('\n');
        }
    }

    context
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_goal_new() {
        let goal = GoalCondition::new("All tests pass");
        assert_eq!(goal.description, "All tests pass");
        assert!(!goal.satisfied);
        assert_eq!(goal.evaluation_count, 0);
        assert!(goal.last_evaluated.is_none());
    }

    #[test]
    fn test_goal_record_evaluation() {
        let mut goal = GoalCondition::new("Tests pass");
        goal.record_evaluation(true, Some("All 10 tests passed".to_string()));

        assert!(goal.satisfied);
        assert_eq!(goal.evaluation_count, 1);
        assert!(goal.last_evaluated.is_some());
        assert_eq!(goal.last_reasoning, Some("All 10 tests passed".to_string()));
    }

    #[test]
    fn test_goal_summary() {
        let goal = GoalCondition::new("Feature complete");
        let summary = goal.summary();
        assert!(summary.contains("Feature complete"));
        assert!(summary.contains("not satisfied"));
    }

    #[test]
    fn test_parse_evaluation_yes() {
        // Test the parsing logic directly
        let text = "SATISFIED: YES\nCONFIDENCE: 0.95\nREASONING: All tests have passed and the feature is implemented.";

        let mut satisfied = false;
        let mut confidence = None;
        let mut reasoning = String::new();

        for line in text.lines() {
            let line = line.trim();
            if line.starts_with("SATISFIED:") {
                let value = line.strip_prefix("SATISFIED:").unwrap_or("").trim();
                satisfied = value.eq_ignore_ascii_case("YES");
            } else if line.starts_with("CONFIDENCE:") {
                let value = line.strip_prefix("CONFIDENCE:").unwrap_or("").trim();
                confidence = value.parse::<f64>().ok();
            } else if line.starts_with("REASONING:") {
                reasoning = line
                    .strip_prefix("REASONING:")
                    .unwrap_or("")
                    .trim()
                    .to_string();
            }
        }

        assert!(satisfied);
        assert_eq!(confidence, Some(0.95));
        assert!(reasoning.contains("All tests"));
    }

    #[test]
    fn test_build_context() {
        let messages = vec![
            Message::new(
                "session-1",
                Role::User,
                vec![MessagePart::Text {
                    text: "Run the tests".to_string(),
                }],
            ),
            Message::new(
                "session-1",
                Role::Assistant,
                vec![MessagePart::Text {
                    text: "Running cargo test...".to_string(),
                }],
            ),
        ];

        let context = build_evaluation_context(&messages, 1000);
        assert!(context.contains("User"));
        assert!(context.contains("Assistant"));
        assert!(context.contains("Run the tests"));
    }
}
