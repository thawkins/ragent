//! User interaction tool for asking questions.
//!
//! Provides [`QuestionTool`], which publishes a permission-request event to
//! prompt the user for clarification or confirmation during an agent session.

use anyhow::{Context, Result};
use serde_json::{Value, json};

use super::{Tool, ToolContext, ToolOutput};
use crate::event::Event;

/// Asks the user a question by publishing a [`Event::PermissionRequested`] event
/// and returns a pending-response marker with a unique request ID.
pub struct QuestionTool;

#[async_trait::async_trait]
impl Tool for QuestionTool {
    fn name(&self) -> &str {
        "question"
    }

    fn description(&self) -> &str {
        "Ask the user a question and wait for their response. \
         Use this when you need clarification or confirmation."
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

    fn permission_category(&self) -> &str {
        "question"
    }

    async fn execute(&self, input: Value, ctx: &ToolContext) -> Result<ToolOutput> {
        let question = input["question"]
            .as_str()
            .context("Missing 'question' parameter")?;

        let request_id = uuid::Uuid::new_v4().to_string();

        ctx.event_bus.publish(Event::PermissionRequested {
            session_id: ctx.session_id.clone(),
            request_id: request_id.clone(),
            permission: "question".to_string(),
            description: question.to_string(),
        });

        Ok(ToolOutput {
            content: format!("[Question asked: {}] Awaiting user response.", question),
            metadata: Some(json!({
                "request_id": request_id,
                "question": question,
            })),
        })
    }
}
