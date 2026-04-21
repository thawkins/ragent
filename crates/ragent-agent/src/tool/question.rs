//! User interaction tool for asking questions.
//!
//! Provides [`QuestionTool`], which publishes a permission-request event to
//! prompt the user for free-text input during an agent session, then blocks
//! until the user submits a response.

use anyhow::{Context, Result};
use serde_json::{Value, json};
use tokio::sync::broadcast::error::RecvError;

use super::{Tool, ToolContext, ToolOutput};
use crate::event::Event;

/// Asks the user a question by publishing a [`Event::PermissionRequested`] event
/// with `permission == "question"` and waits for a [`Event::UserInput`] response.
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
         before proceeding. The user will see a text-input dialog and their response \
         is returned as the result of this tool call."
    }

    fn parameters_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "question": {
                    "type": "string",
                    "description": "The question to ask the user"
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

        let request_id = uuid::Uuid::new_v4().to_string();

        // Subscribe before publishing so we don't miss the reply.
        let mut rx = ctx.event_bus.subscribe();

        ctx.event_bus.publish(Event::PermissionRequested {
            session_id: ctx.session_id.clone(),
            request_id: request_id.clone(),
            permission: "question".to_string(),
            description: question.to_string(),
        });

        // Wait for a matching UserInput event from the TUI.
        let response = loop {
            match rx.recv().await {
                Ok(Event::UserInput {
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
