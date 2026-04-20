//! AIWiki ingestion tool for agents.
//!
//! Provides [`AiwikiIngestTool`], which ingests documents into the AIWiki
//! knowledge base. Supports files, directories, and the raw/ folder.

use anyhow::Result;
use serde_json::{Value, json};
use std::path::Path;

use super::{Tool, ToolContext, ToolOutput};

/// Ingest documents into the AIWiki knowledge base.
pub struct AiwikiIngestTool;

/// Build a "not available" response when AIWiki is not initialized.
fn not_initialized() -> ToolOutput {
    ToolOutput {
        content:
            "AIWiki is not initialized. Run `/aiwiki init` first to create the wiki structure."
                .to_string(),
        metadata: Some(json!({
            "error": "aiwiki_not_initialized",
            "initialized": false
        })),
    }
}

/// Build a "disabled" response when AIWiki is disabled.
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
impl Tool for AiwikiIngestTool {
    fn name(&self) -> &'static str {
        "aiwiki_ingest"
    }

    fn description(&self) -> &'static str {
        "Ingest documents into the AIWiki knowledge base. \
         Can ingest a single file, a directory (recursively), \
         or scan the aiwiki/raw/ folder. Supported formats: \
         Markdown (.md), Plain text (.txt), PDF (.pdf), Word (.docx), \
         OpenDocument (.odt). After ingestion, run `/aiwiki sync` to process."
    }

    fn parameters_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "path": {
                    "type": "string",
                    "description": "Path to file or directory to ingest. If omitted, scans aiwiki/raw/ folder."
                },
                "move_file": {
                    "type": "boolean",
                    "description": "Move the source file to raw/ instead of copying (default: false)",
                    "default": false
                },
                "subdirectory": {
                    "type": "string",
                    "description": "Store files in a subdirectory within raw/ (e.g., 'documents', 'references')"
                }
            },
            "additionalProperties": false
        })
    }

    fn permission_category(&self) -> &'static str {
        "aiwiki:write"
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
                });
            }
        };

        if !wiki.config.enabled {
            return Ok(disabled());
        }

        let move_file = input["move_file"].as_bool().unwrap_or(false);
        let subdirectory = input["subdirectory"].as_str().map(String::from);

        let options = if let Some(subdir) = subdirectory {
            aiwiki::IngestOptions::move_file()
                .with_move(move_file)
                .with_subdirectory(subdir)
        } else {
            aiwiki::IngestOptions::move_file().with_move(move_file)
        };

        // Determine what to ingest
        let result = if let Some(path_str) = input["path"].as_str() {
            let path = ctx.working_dir.join(path_str);

            if !path.exists() {
                return Ok(ToolOutput {
                    content: format!("Path not found: {}", path.display()),
                    metadata: Some(json!({"error": "path_not_found", "path": path_str})),
                });
            }

            let metadata = tokio::fs::metadata(&path).await?;

            if metadata.is_file() {
                // Ingest single file
                let result = aiwiki::ingest_file(&wiki, &path, options).await?;
                format_ingestion_result(&result)
            } else if metadata.is_dir() {
                // Ingest directory
                let results = aiwiki::scan_directory(&wiki, &path, options, true).await?;
                format_directory_result(&results, &path)
            } else {
                return Ok(ToolOutput {
                    content: format!("Unsupported path type: {}", path.display()),
                    metadata: Some(json!({"error": "unsupported_path_type"})),
                });
            }
        } else {
            // Scan raw/ directory
            let options = aiwiki::IngestOptions::default();
            let results = aiwiki::ingest_raw_directory(&wiki, options).await?;
            format_raw_scan_result(&results)
        };

        Ok(result)
    }
}

/// Format a single file ingestion result.
fn format_ingestion_result(result: &aiwiki::IngestionResult) -> ToolOutput {
    let content = format!(
        "## File Ingested Successfully\n\n\
         **File:** {}\n\
         **Stored at:** {}\n\
         **Type:** {:?}\n\
         **Size:** {} bytes\n\
         **Hash:** {}\n\
         **Text extracted:** {}",
        result.source_path.display(),
        result.stored_path.display(),
        result.doc_type,
        result.size_bytes,
        result.hash,
        if result.text_extracted { "Yes" } else { "No" }
    );

    ToolOutput {
        content,
        metadata: Some(json!({
            "stored_path": result.stored_path.to_string_lossy().to_string(),
            "doc_type": format!("{:?}", result.doc_type),
            "size_bytes": result.size_bytes,
            "hash": &result.hash,
            "text_extracted": result.text_extracted
        })),
    }
}

/// Format a directory ingestion result.
fn format_directory_result(results: &[aiwiki::IngestionResult], source_dir: &Path) -> ToolOutput {
    if results.is_empty() {
        return ToolOutput {
            content: format!(
                "No supported files found in directory: {}",
                source_dir.display()
            ),
            metadata: Some(json!({"ingested_count": 0})),
        };
    }

    let mut content = format!("## Directory Ingested: {}\n\n", source_dir.display());
    content.push_str(&format!("**Files ingested:** {}\n\n", results.len()));

    for result in results.iter().take(10) {
        content.push_str(&format!(
            "- `{}` → `{}` ({:?})\n",
            result
                .source_path
                .file_name()
                .unwrap_or_default()
                .to_string_lossy(),
            result
                .stored_path
                .file_name()
                .unwrap_or_default()
                .to_string_lossy(),
            result.doc_type
        ));
    }

    if results.len() > 10 {
        content.push_str(&format!("\n_... and {} more files_\n", results.len() - 10));
    }

    content.push_str("\nRun `/aiwiki sync` to process these files into wiki pages.");

    ToolOutput {
        content,
        metadata: Some(json!({
            "ingested_count": results.len(),
            "source_dir": source_dir.to_string_lossy().to_string()
        })),
    }
}

/// Format a raw/ directory scan result.
fn format_raw_scan_result(results: &[aiwiki::IngestionResult]) -> ToolOutput {
    if results.is_empty() {
        return ToolOutput {
            content: "No new or modified files found in aiwiki/raw/.\n\n\
                      The wiki is up to date. Run `/aiwiki sync` to regenerate pages if needed."
                .to_string(),
            metadata: Some(json!({"new_count": 0, "modified_count": 0})),
        };
    }

    let mut content = format!(
        "## Raw Directory Scan Complete\n\n\
         **New/modified files found:** {}\n\n",
        results.len()
    );

    for result in results.iter().take(10) {
        content.push_str(&format!(
            "- `{}` ({:?}, {} bytes)\n",
            result
                .stored_path
                .file_name()
                .unwrap_or_default()
                .to_string_lossy(),
            result.doc_type,
            result.size_bytes
        ));
    }

    if results.len() > 10 {
        content.push_str(&format!("\n_... and {} more files_\n", results.len() - 10));
    }

    content.push_str("\nRun `/aiwiki sync` to process these files into wiki pages.");

    ToolOutput {
        content,
        metadata: Some(json!({
            "new_modified_count": results.len()
        })),
    }
}
