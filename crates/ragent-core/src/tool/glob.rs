//! File discovery tool using glob patterns.
//!
//! Provides [`GlobTool`], which recursively searches directories for files
//! matching a glob pattern (e.g., `**/*.rs`), skipping hidden and generated directories.

use anyhow::{Context, Result};
use serde_json::{Value, json};
use std::path::{Path, PathBuf};

use super::{Tool, ToolContext, ToolOutput};

/// Finds files matching a glob pattern by recursively walking directories.
///
/// Hidden entries and common generated directories (`node_modules`, `target`, etc.)
/// are skipped. Results are capped at 1 000 matches.
pub struct GlobTool;

#[async_trait::async_trait]
impl Tool for GlobTool {
    fn name(&self) -> &str {
        "glob"
    }

    /// Returns a human-readable description of what the tool does.
    fn description(&self) -> &str {
        "Find files matching a glob pattern. Recursively searches directories."
    }

    fn parameters_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "pattern": {
                    "type": "string",
                    "description": "Glob pattern to match (e.g. '**/*.rs', 'src/**/*.ts')"
                },
                "path": {
                    "type": "string",
                    "description": "Base directory (default: working directory)"
                }
            },
            "required": ["pattern"]
        })
    }

    fn permission_category(&self) -> &str {
        "file:read"
    }

    /// Finds files matching a glob pattern.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The `pattern` parameter is missing or invalid
    /// - The glob pattern is malformed or cannot be compiled
    async fn execute(&self, input: Value, ctx: &ToolContext) -> Result<ToolOutput> {
        let pattern = input["pattern"]
            .as_str()
            .context("Missing required 'pattern' parameter")?;
        let base_dir = input["path"]
            .as_str()
            .map(|p| resolve_path(&ctx.working_dir, p))
            .unwrap_or_else(|| ctx.working_dir.clone());

        let glob = globset::GlobBuilder::new(pattern)
            .case_insensitive(false)
            .build()
            .with_context(|| format!("Invalid glob pattern: {}", pattern))?;
        let matcher = glob.compile_matcher();

        let mut match_results = Vec::new();
        const MAX_MATCHES: usize = 1000;

        collect_matches(&base_dir, &base_dir, &matcher, &mut match_results, MAX_MATCHES)?;

        match_results.sort();

        if match_results.is_empty() {
            Ok(ToolOutput {
                content: format!("No files matching '{}' in {}", pattern, base_dir.display()),
                metadata: None,
            })
        } else {
            let truncated = match_results.len() >= MAX_MATCHES;
            let content = match_results.join("\n");
            Ok(ToolOutput {
                content: format!(
                    "{} file{} found{}\n\n{}",
                    match_results.len(),
                    if match_results.len() == 1 { "" } else { "s" },
                    if truncated { " (truncated)" } else { "" },
                    content
                ),
                metadata: Some(json!({
                    "count": match_results.len(),
                    "truncated": truncated,
                })),
            })
        }
    }
}

/// Recursively collects file paths matching a glob pattern.
///
/// # Errors
///
/// Returns an error if a directory cannot be read due to permission issues.
/// IO errors on individual entries are silently skipped.
fn collect_matches(
    root: &Path,
    dir: &Path,
    matcher: &globset::GlobMatcher,
    results: &mut Vec<String>,
    max: usize,
) -> Result<()> {
    let entries = match std::fs::read_dir(dir) {
        Ok(entries) => entries,
        Err(_) => return Ok(()),
    };

    for entry in entries {
        if results.len() >= max {
            break;
        }
        let entry = match entry {
            Ok(e) => e,
            Err(_) => continue,
        };
        let path = entry.path();

        // Skip hidden entries
        if path
            .file_name()
            .and_then(|n| n.to_str())
            .is_some_and(|n| n.starts_with('.'))
        {
            continue;
        }

        if path.is_dir() {
            let dir_name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
            if matches!(
                                  dir_name,
                                  "node_modules" | "target" | "__pycache__" | "dist" | "build"
                              ) {
                                  continue;
                              }
                              collect_matches(root, &path, matcher, results, max)?;
                          } else {
                              // Match relative path against glob
                              if let Ok(rel) = path.strip_prefix(root)
                                  && matcher.is_match(rel)
                              {
                                  results.push(rel.display().to_string());
                              }
                          }
                      }
                      Ok(())
                }
/// Resolves a path string to an absolute `PathBuf` relative to the working directory.
fn resolve_path(working_dir: &Path, path_str: &str) -> PathBuf {
    let p = PathBuf::from(path_str);
    if p.is_absolute() {
        p
    } else {
        working_dir.join(p)
    }
}
