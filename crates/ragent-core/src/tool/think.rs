//! Reasoning note tool.

use anyhow::{Context, Result};
use serde_json::{Value, json};

use super::{Tool, ToolContext, ToolOutput};

/// Records a short reasoning note for the current session.
pub struct ThinkTool;

#[async_trait::async_trait]
impl Tool for ThinkTool {
    fn name(&self) -> &'static str {
        "think"
    }

    fn description(&self) -> &'static str {
        "Record a short reasoning note without changing project state."
    }

    fn parameters_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "thought": {
                    "type": "string",
                    "description": "Short reasoning note"
                }
            },
            "required": ["thought"],
            "additionalProperties": false
        })
    }

    fn permission_category(&self) -> &'static str {
        "think:record"
    }

    async fn execute(&self, input: Value, ctx: &ToolContext) -> Result<ToolOutput> {
        let thought = input["thought"]
            .as_str()
            .context("Missing required 'thought' parameter")?;

        ctx.event_bus.publish(crate::event::Event::ReasoningDelta {
            session_id: ctx.session_id.clone(),
            text: thought.to_string(),
        });

        Ok(ToolOutput {
            content: String::new(),
            metadata: Some(json!({"thinking": true})),
        })
    }
}
