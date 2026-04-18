//! AIWiki export tool for agents.
//!
//! Provides [`AiwikiExportTool`], which exports the AIWiki knowledge base
//! to various formats including single markdown and Obsidian vault.

use anyhow::Result;
use serde_json::{Value, json};

use super::{Tool, ToolContext, ToolOutput};

/// Export the AIWiki knowledge base to various formats.
pub struct AiwikiExportTool;

/// Build a "not initialized" response.
fn not_initialized() -> ToolOutput {
    ToolOutput {
        content: "AIWiki is not initialized. Run `/aiwiki init` first to create the wiki structure."
            .to_string(),
        metadata: Some(json!({
            "error": "aiwiki_not_initialized",
            "initialized": false
        })),
    }
}

/// Build a "disabled" response.
fn disabled() -> ToolOutput {
    ToolOutput {
        content: "AIWiki is currently disabled. Run `/aiwiki on` to enable it.".to_string(),
        metadata: Some(json!({
            "error": "aiwiki_disabled",
            "enabled": false
        })),
    }
}

#[async_trait::async_trait]
impl Tool for AiwikiExportTool {
    fn name(&self) -> &'static str {
        "aiwiki_export"
    }

    fn description(&self) -> &'static str {
        "Export the AIWiki knowledge base to various formats. \
         Supports exporting as a single combined markdown file, \
         or as an Obsidian-compatible vault. Use this to backup, \
         share, or migrate your wiki content."
    }

    fn parameters_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "format": {
                    "type": "string",
                    "description": "Export format",
                    "enum": ["single_markdown", "obsidian"],
                    "default": "single_markdown"
                },
                "output_path": {
                    "type": "string",
                    "description": "Output file path for single_markdown, or directory for obsidian"
                }
            },
            "required": ["format"],
            "additionalProperties": false
        })
    }

    fn permission_category(&self) -> &'static str {
        "aiwiki:read"
    }

    async fn execute(&self, input: Value, ctx: &ToolContext) -> Result<ToolOutput> {
        // Check if AIWiki exists
        if !aiwiki::Aiwiki::exists(&ctx.working_dir) {
            return Ok(not_initialized());
        }

        let wiki = match aiwiki::Aiwiki::new(&ctx.working_dir).await {
            Ok(w) => w,
            Err(e) => {
                return Ok(ToolOutput {
                    content: format!("Failed to load AIWiki: {}", e),
                    metadata: Some(json!({"error": e.to_string()})),
                })
            }
        };

        if !wiki.config.enabled {
            return Ok(disabled());
        }

        let format = input["format"].as_str().unwrap_or("single_markdown");
        
        match format {
            "single_markdown" => {
                let output_path = input["output_path"].as_str()
                    .map(|p| ctx.working_dir.join(p))
                    .unwrap_or_else(|| ctx.working_dir.join("aiwiki_export.md"));
                
                match aiwiki::export_single_markdown(&wiki, &output_path).await {
                    Ok(count) => {
                        Ok(ToolOutput {
                            content: format!(
                                "## Export Complete ✅\n\n\
                                 Exported {} pages to:\n\
                                 `{}`\n\n\
                                 Format: Single Markdown File",
                                count,
                                output_path.display()
                            ),
                            metadata: Some(json!({
                                "exported_count": count,
                                "output_path": output_path.to_string_lossy().to_string(),
                                "format": "single_markdown"
                            })),
                        })
                    }
                    Err(e) => {
                        Ok(ToolOutput {
                            content: format!("Export failed: {}", e),
                            metadata: Some(json!({"error": e.to_string()})),
                        })
                    }
                }
            }
            "obsidian" => {
                let output_dir = input["output_path"].as_str()
                    .map(|p| ctx.working_dir.join(p))
                    .unwrap_or_else(|| ctx.working_dir.join("aiwiki_obsidian"));
                
                match aiwiki::export_obsidian_vault(&wiki, &output_dir).await {
                    Ok(count) => {
                        Ok(ToolOutput {
                            content: format!(
                                "## Obsidian Vault Export Complete ✅\n\n\
                                 Exported {} pages to:\n\
                                 `{}`\n\n\
                                 Format: Obsidian-Compatible Vault\n\n\
                                 ## Usage\n\n\
                                 1. Open this folder as a vault in Obsidian\n\
                                 2. All your wiki pages are ready to use\n\
                                 3. Graph view and backlinks are available",
                                count,
                                output_dir.display()
                            ),
                            metadata: Some(json!({
                                "exported_count": count,
                                "output_path": output_dir.to_string_lossy().to_string(),
                                "format": "obsidian"
                            })),
                        })
                    }
                    Err(e) => {
                        Ok(ToolOutput {
                            content: format!("Export failed: {}", e),
                            metadata: Some(json!({"error": e.to_string()})),
                        })
                    }
                }
            }
            _ => {
                Ok(ToolOutput {
                    content: format!("Unknown export format: {}", format),
                    metadata: Some(json!({"error": "unknown_format"})),
                })
            }
        }
    }
}
