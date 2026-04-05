//! File discovery tool using glob patterns.
//!
//! Provides [`GlobTool`], which recursively searches directories for files
//! matching a glob pattern (e.g., `**/*.rs`), skipping hidden and generated directories.
//!
//! The directory walk is parallelised with Rayon when the entry count is large,
//! reducing latency on big codebases.

use anyhow::{Context, Result};
use rayon::prelude::*;
use serde_json::{Value, json};
use std::path::{Path, PathBuf};
use std::sync::Arc;

use super::{Tool, ToolContext, ToolOutput};

/// Finds files matching a glob pattern by recursively walking directories.
///
/// Hidden entries and common generated directories (`node_modules`, `target`, etc.)
/// are skipped. Results are capped at 1 000 matches.
pub struct GlobTool;

#[async_trait::async_trait]
impl Tool for GlobTool {
    fn name(&self) -> &'static str {
        "glob"
    }

    /// Returns a human-readable description of what the tool does.
    fn description(&self) -> &'static str {
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

    fn permission_category(&self) -> &'static str {
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
        let base_dir = input["path"].as_str().map_or_else(
            || ctx.working_dir.clone(),
            |p| resolve_path(&ctx.working_dir, p),
        );

        let glob = globset::GlobBuilder::new(pattern)
            .case_insensitive(false)
            .build()
            .with_context(|| format!("Invalid glob pattern: {pattern}"))?;
        let matcher = glob.compile_matcher();

        let mut match_results = Vec::new();
        const MAX_MATCHES: usize = 1000;

        collect_matches(
            &base_dir,
            &base_dir,
            &matcher,
            &mut match_results,
            MAX_MATCHES,
        )?;

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
/// Uses Rayon for parallel directory walking to accelerate large trees.
/// IO errors on individual entries are silently skipped.
fn collect_matches(
    root: &Path,
    dir: &Path,
    matcher: &globset::GlobMatcher,
    results: &mut Vec<String>,
    max: usize,
) -> Result<()> {
    // Gather all entries in this directory
    let entries: Vec<_> = match std::fs::read_dir(dir) {
        Ok(rd) => rd.filter_map(|e| e.ok()).collect(),
        Err(_) => return Ok(()),
    };

    let root = Arc::new(root.to_path_buf());
    let matcher = Arc::new(matcher.clone());
    let max_results = max;

    // Partition entries into files and sub-directories
    let mut subdirs: Vec<PathBuf> = Vec::new();
    let mut matched: Vec<String> = Vec::new();

    for entry in &entries {
        let path = entry.path();
        let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
        if name.starts_with('.') {
            continue;
        }
        if path.is_dir() {
            if !matches!(
                name,
                "node_modules" | "target" | "__pycache__" | "dist" | "build"
            ) {
                subdirs.push(path);
            }
        } else if let Ok(rel) = path.strip_prefix(root.as_path())
            && matcher.is_match(rel)
        {
            matched.push(rel.display().to_string());
        }
    }

    // Collect matching files from this level
    for m in matched {
        if results.len() >= max_results {
            return Ok(());
        }
        results.push(m);
    }

    // Walk sub-directories in parallel when there are many of them
    if subdirs.len() > 4 {
        let parallel_results: Vec<Vec<String>> = subdirs
            .par_iter()
            .map(|sub| {
                let mut local = Vec::new();
                let _ = collect_matches(sub, sub, matcher.as_ref(), &mut local, max_results);
                local
            })
            .collect();
        for batch in parallel_results {
            for item in batch {
                if results.len() >= max_results {
                    return Ok(());
                }
                results.push(item);
            }
        }
    } else {
        for sub in &subdirs {
            if results.len() >= max_results {
                return Ok(());
            }
            collect_matches(sub, sub, matcher.as_ref(), results, max_results)?;
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
