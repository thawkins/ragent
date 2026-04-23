//! User interaction tool for asking questions.
//!
//! Provides [`QuestionTool`], which publishes a dedicated question event to
//! prompt the user for input during an agent session, then blocks until the
//! user submits a response.

use anyhow::{Context, Result};
use serde_json::{Value, json};
use tokio::sync::broadcast::error::RecvError;

use super::{Tool, ToolContext, ToolOutput};
use crate::event::Event;

/// Asks the user a question by publishing a [`Event::QuestionRequested`] event
/// and waits for a [`Event::QuestionAnswered`] response.
pub struct QuestionTool;

#[async_trait::async_trait]
impl Tool for QuestionTool {
    fn name(&self) -> &'static str {
        "question"
    }

          /// # Errors
          ///
          /// Returns an error if the description string cannot be converted or returned.
          fn description(&self) -> &'static str {
              "Ask the user a question and wait for their typed response. \
               Use this when you need clarification, prioritisation help, or confirmation \
               before proceeding. \
               \
               When you need a choice from a fixed set, provide the optional `options` \
               parameter as an array of strings (e.g. [\"Yes\", \"No\", \"Skip\"]). \
               The user will see a multiple-choice dialog instead of a free-text input, \
               and their selection is returned as the result. \
               If `options` is omitted the user sees a plain text-input dialog."
          }
    fn parameters_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "question": {
                    "type": "string",
                    "description": "The question to ask the user"
                },
                "options": {
                    "type": "array",
                    "items": { "type": "string" },
                    "description": "Optional multiple-choice options. When provided, the user selects one instead of typing free text."
                }
            },
            "required": ["question"]
        })
    }

    /// # Errors
    ///
    /// Returns an error if the category string cannot be converted or returned.
    fn permission_category(&self) -> &'static str {
        "question"
    }

    /// Publishes a question event and blocks until the user submits a response.
    ///
    /// # Errors
    ///
    /// Returns an error if the `question` parameter is missing or the user
    /// dismisses the dialog without answering.
    async fn execute(&self, input: Value, ctx: &ToolContext) -> Result<ToolOutput> {
        let question = input["question"]
            .as_str()
            .context("Missing required 'question' parameter")?;

        let options: Vec<String> = input["options"]
            .as_array()
            .map(|arr| {
                arr.iter()
                    .filter_map(|v| v.as_str().map(String::from))
                    .collect()
            })
            .unwrap_or_default();

        let request_id = uuid::Uuid::new_v4().to_string();

        // Subscribe before publishing so we don't miss the reply.
        let mut rx = ctx.event_bus.subscribe();

        ctx.event_bus.publish(Event::QuestionRequested {
            session_id: ctx.session_id.clone(),
            request_id: request_id.clone(),
            question: question.to_string(),
            options,
        });

        // Wait for a matching QuestionAnswered event from the TUI.
        let response = loop {
            match rx.recv().await {
                Ok(Event::QuestionAnswered {
                    session_id: ref s,
                    request_id: ref rid,
                    response: ref r,
                }) if s == &ctx.session_id && rid == &request_id => {
                    break r.clone();
                }
                Ok(_) => {
                    // Ignore unrelated events.
                }
                Err(RecvError::Lagged(_)) => {
                    // Some events were dropped; keep waiting.
                }
                Err(RecvError::Closed) => {
                    anyhow::bail!("Event bus closed while waiting for user response");
                }
            }
        };

        Ok(ToolOutput {
            content: response.clone(),
            metadata: Some(json!({
                "request_id": request_id,
                "question": question,
                "response": response,
            })),
        })
    }
}
