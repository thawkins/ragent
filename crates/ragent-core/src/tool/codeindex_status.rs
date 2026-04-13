//! Codebase index status tool.

use anyhow::Result;
use serde_json::{Value, json};

use super::{Tool, ToolContext, ToolOutput};

/// Show status and statistics of the codebase index.
pub struct CodeIndexStatusTool;

fn not_available() -> ToolOutput {
    ToolOutput {
        content: "Code index is not available. It may be disabled or not yet initialised. \
                  Use `/codeindex on` to enable it."
            .to_string(),
        metadata: Some(json!({
            "error": "codeindex_disabled",
            "enabled": false
        })),
    }
}

#[async_trait::async_trait]
impl Tool for CodeIndexStatusTool {
    fn name(&self) -> &'static str {
        "codeindex_status"
    }

    fn description(&self) -> &'static str {
        "Show the current status and statistics of the codebase index — \
         files indexed, symbols extracted, languages, index size, and timestamps."
    }

    fn parameters_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {},
            "additionalProperties": false
        })
    }

    fn permission_category(&self) -> &'static str {
        "codeindex:read"
    }

    async fn execute(&self, _input: Value, ctx: &ToolContext) -> Result<ToolOutput> {
        let idx = match &ctx.code_index {
            Some(idx) => idx,
            None => return Ok(not_available()),
        };

        let stats = idx.status()?;

        let mut output = String::from("## Code Index Status\n\n");
        output.push_str(&format!("Files indexed:  {}\n", stats.files_indexed));
        output.push_str(&format!("Total symbols:  {}\n", stats.total_symbols));
        output.push_str(&format!(
            "Total size:     {:.1} KB\n",
            stats.total_bytes as f64 / 1024.0
        ));

        if !stats.languages.is_empty() {
            output.push_str("Languages:      ");
            for (i, (lang, count)) in stats.languages.iter().enumerate() {
                if i > 0 {
                    output.push_str(", ");
                }
                output.push_str(&format!("{lang} ({count})"));
            }
            output.push('\n');
        }

        if let Some(ts) = &stats.last_full_index {
            output.push_str(&format!("Last full:      {ts}\n"));
        }
        if let Some(ts) = &stats.last_incremental_update {
            output.push_str(&format!("Last incremental: {ts}\n"));
        }
        output.push_str(&format!(
            "Index size:     {:.1} KB\n",
            stats.index_size_bytes as f64 / 1024.0
        ));

        Ok(ToolOutput {
            content: output,
            metadata: Some(json!({
                "enabled": true,
                "files_indexed": stats.files_indexed,
                "total_symbols": stats.total_symbols,
                "index_size_bytes": stats.index_size_bytes,
            })),
        })
    }
}
