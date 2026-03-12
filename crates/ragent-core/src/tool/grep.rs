//! Text search tool for file contents.
//!
//! Provides [`GrepTool`], which searches files for a text pattern, returning
//! matching lines with file paths and line numbers. Supports optional file-type
//! filtering and case-insensitive search.

use anyhow::{Context, Result};
use serde_json::{Value, json};
use std::fmt::Write as _;
use std::path::{Path, PathBuf};

use super::{Tool, ToolContext, ToolOutput};

/// Searches file contents for a text pattern across a directory tree.
///
/// Binary files are automatically skipped, and results are capped at 500 matches.
/// Hidden entries and common generated directories are excluded from the search.
pub struct GrepTool;

#[async_trait::async_trait]
impl Tool for GrepTool {
    fn name(&self) -> &str {
        "grep"
    }

    fn description(&self) -> &str {
        "Search file contents for a pattern. Uses simple string matching. \
         Returns matching lines with file path and line number."
    }

    fn parameters_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "pattern": {
                    "type": "string",
                    "description": "Text pattern to search for"
                },
                "path": {
                    "type": "string",
                    "description": "Directory to search in (default: working directory)"
                },
                "include": {
                    "type": "string",
                    "description": "File glob pattern to include (e.g. '*.rs')"
                },
                "case_insensitive": {
                    "type": "boolean",
                    "description": "Case insensitive search (default: false)"
                }
            },
            "required": ["pattern"]
        })
    }

    fn permission_category(&self) -> &str {
        "file:read"
    }

    async fn execute(&self, input: Value, ctx: &ToolContext) -> Result<ToolOutput> {
        let pattern = input["pattern"]
            .as_str()
            .context("Missing required 'pattern' parameter")?;
        let search_dir = input["path"]
            .as_str()
            .map(|p| resolve_path(&ctx.working_dir, p))
            .unwrap_or_else(|| ctx.working_dir.clone());
        let include_glob = input["include"].as_str();
        let case_insensitive = input["case_insensitive"].as_bool().unwrap_or(false);

        let glob_matcher = include_glob.and_then(|g| {
            globset::GlobBuilder::new(g)
                .case_insensitive(false)
                .build()
                .ok()
                .map(|glob| glob.compile_matcher())
        });

        let search_pattern = if case_insensitive {
            pattern.to_lowercase()
        } else {
            pattern.to_string()
        };

        let mut results = Vec::new();
        let mut files_searched = 0u64;
        const MAX_RESULTS: usize = 500;

        search_directory(
            &search_dir,
            &search_pattern,
            case_insensitive,
            &glob_matcher,
            &mut results,
            &mut files_searched,
            MAX_RESULTS,
        )?;

        if results.is_empty() {
            Ok(ToolOutput {
                content: format!(
                    "No matches found for '{}' in {} ({} files searched)",
                    pattern,
                    search_dir.display(),
                    files_searched
                ),
                metadata: None,
            })
        } else {
            let truncated = results.len() >= MAX_RESULTS;
            let content = results.join("\n");
            let summary = format!(
                "{} match{} in {} files searched{}",
                results.len(),
                if results.len() == 1 { "" } else { "es" },
                files_searched,
                if truncated {
                    " (results truncated)"
                } else {
                    ""
                }
            );
            Ok(ToolOutput {
                content: format!("{}\n\n{}", summary, content),
                metadata: Some(json!({
                    "matches": results.len(),
                    "files_searched": files_searched,
                    "truncated": truncated,
                })),
            })
        }
    }
}

fn search_directory(
    dir: &Path,
    pattern: &str,
    case_insensitive: bool,
    glob_matcher: &Option<globset::GlobMatcher>,
    results: &mut Vec<String>,
    files_searched: &mut u64,
    max_results: usize,
) -> Result<()> {
    let entries = match std::fs::read_dir(dir) {
        Ok(entries) => entries,
        Err(_) => return Ok(()),
    };

    for entry in entries {
        if results.len() >= max_results {
            break;
        }
        let entry = match entry {
            Ok(e) => e,
            Err(_) => continue,
        };
        let path = entry.path();

        // Skip hidden files/directories
        if path
            .file_name()
            .and_then(|n| n.to_str())
            .is_some_and(|n| n.starts_with('.'))
        {
            continue;
        }

        if path.is_dir() {
            // Skip common non-source directories
            let dir_name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
            if matches!(
                dir_name,
                "node_modules" | "target" | ".git" | "__pycache__" | "dist" | "build"
            ) {
                continue;
            }
            search_directory(
                &path,
                pattern,
                case_insensitive,
                glob_matcher,
                results,
                files_searched,
                max_results,
            )?;
        } else if path.is_file() {
            // Check glob filter
            if let Some(matcher) = glob_matcher
                && !matcher.is_match(&path)
            {
                continue;
            }

            // Skip binary files (check first 512 bytes)
            if let Ok(sample) = std::fs::read(&path) {
                if sample.len() > 512 {
                    if sample[..512].contains(&0) {
                        continue;
                    }
                } else if sample.contains(&0) {
                    continue;
                }
            }

            *files_searched += 1;

            if let Ok(content) = std::fs::read_to_string(&path) {
                let mut buf = String::new();
                for (line_num, line) in content.lines().enumerate() {
                    if results.len() >= max_results {
                        break;
                    }
                    let matches = if case_insensitive {
                        line.to_lowercase().contains(pattern)
                    } else {
                        line.contains(pattern)
                    };
                    if matches {
                        buf.clear();
                        let _ = write!(buf, "{}:{}:{}", path.display(), line_num + 1, line.trim());
                        results.push(buf.clone());
                    }
                }
            }
        }
    }
    Ok(())
}

fn resolve_path(working_dir: &Path, path_str: &str) -> PathBuf {
    let p = PathBuf::from(path_str);
    if p.is_absolute() {
        p
    } else {
        working_dir.join(p)
    }
}
