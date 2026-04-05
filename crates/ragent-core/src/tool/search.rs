//! Code search tool — a model-friendly alias for the `grep` tool.
//!
//! Some LLMs (especially smaller open-weight models) hallucinate a generic
//! `search` or `Search` tool when they need to look for symbols or text in a
//! codebase.  [`SearchTool`] accepts the parameters those models typically
//! emit (`query`, `path`, `max_results`) and forwards them to the same
//! ripgrep-backed search engine used by [`super::grep::GrepTool`].
//!
//! This prevents "Unknown tool: search" errors while giving the model a
//! working path to the same capability.

use anyhow::{Context, Result};
use grep_regex::RegexMatcherBuilder;
use grep_searcher::{Searcher, SearcherBuilder, Sink, SinkMatch};
use ignore::WalkBuilder;
use serde_json::{Value, json};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use super::{Tool, ToolContext, ToolOutput};

/// Maximum matches returned by a single search call.
const MAX_RESULTS: usize = 500;

/// A model-friendly code search tool backed by ripgrep.
///
/// Accepts `query`, `path`, and `max_results` — the parameter names most
/// LLMs produce when they imagine a generic "search codebase" tool.
pub struct SearchTool;

#[async_trait::async_trait]
impl Tool for SearchTool {
    fn name(&self) -> &'static str {
        "search"
    }

    fn description(&self) -> &'static str {
        "Search for text or code patterns in the codebase. \
         Accepts a plain-text or regex query and an optional path. \
         Returns matching lines with file path and line number. \
         Use 'grep' for full regex/flag control; 'search' is a quick alias."
    }

    fn parameters_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "query": {
                    "type": "string",
                    "description": "Text or regex pattern to search for"
                },
                "path": {
                    "type": "string",
                    "description": "Directory or file to search in (default: working directory)"
                },
                "include": {
                    "type": "string",
                    "description": "Glob pattern to restrict which files are searched (e.g. '*.rs')"
                },
                "case_insensitive": {
                    "type": "boolean",
                    "description": "Case-insensitive search (default: false)"
                },
                "max_results": {
                    "type": "integer",
                    "description": "Maximum number of matches to return (default: 100)"
                }
            },
            "required": ["query"]
        })
    }

    fn permission_category(&self) -> &'static str {
        "file:read"
    }

    /// Executes a code search.
    ///
    /// # Errors
    ///
    /// Returns an error if `query` is missing or is an invalid regex.
    async fn execute(&self, input: Value, ctx: &ToolContext) -> Result<ToolOutput> {
        let query = input["query"]
            .as_str()
            .context("Missing required 'query' parameter")?;

        let base_dir = input["path"].as_str().map_or_else(
            || ctx.working_dir.clone(),
            |p| resolve_path(&ctx.working_dir, p),
        );

        let include_glob = input["include"].as_str();
        let case_insensitive = input["case_insensitive"].as_bool().unwrap_or(false);
        let max_results = input["max_results"]
            .as_u64()
            .map(|n| n.min(MAX_RESULTS as u64) as usize)
            .unwrap_or(100);

        // Build a case-sensitive or case-insensitive regex matcher
        let matcher = RegexMatcherBuilder::new()
            .case_insensitive(case_insensitive)
            .build(query)
            .with_context(|| format!("Invalid search pattern: {query}"))?;

        let results: Arc<Mutex<Vec<String>>> = Arc::new(Mutex::new(Vec::new()));
        let results_clone = Arc::clone(&results);
        let max = max_results;

        let mut walker = WalkBuilder::new(&base_dir);
        walker
            .hidden(true)
            .git_ignore(true)
            .git_global(true)
            .git_exclude(true);

        if let Some(glob) = include_glob {
            let mut override_builder = ignore::overrides::OverrideBuilder::new(&base_dir);
            override_builder
                .add(glob)
                .with_context(|| format!("Invalid include glob: {glob}"))?;
            let overrides = override_builder
                .build()
                .context("Failed to build include filter")?;
            walker.overrides(overrides);
        }

        let mut searcher = SearcherBuilder::new().build();

        for entry in walker.build().flatten() {
            if entry.file_type().is_some_and(|t| !t.is_file()) {
                continue;
            }
            let path = entry.path().to_path_buf();
            let rel = path
                .strip_prefix(&base_dir)
                .unwrap_or(&path)
                .display()
                .to_string();

            let results_ref = Arc::clone(&results_clone);
            let _ = searcher.search_path(
                &matcher,
                &path,
                SearchSink {
                    path: rel,
                    results: results_ref,
                    max,
                },
            );

            if results_clone.lock().expect("lock").len() >= max {
                break;
            }
        }

        let lines = Arc::try_unwrap(results)
            .expect("single owner")
            .into_inner()
            .expect("lock");
        let truncated = lines.len() >= max_results;
        if lines.is_empty() {
            return Ok(ToolOutput {
                content: format!("No matches for '{query}' in {}", base_dir.display()),
                metadata: None,
            });
        }
        Ok(ToolOutput {
            content: format!(
                "{} match{}{}\n\n{}",
                lines.len(),
                if lines.len() == 1 { "" } else { "es" },
                if truncated { " (truncated)" } else { "" },
                lines.join("\n"),
            ),
            metadata: Some(json!({
                "count": lines.len(),
                "truncated": truncated,
            })),
        })
    }
}

/// ripgrep sink that collects formatted `path:line: content` strings.
struct SearchSink {
    path: String,
    results: Arc<Mutex<Vec<String>>>,
    max: usize,
}

impl Sink for SearchSink {
    type Error = std::io::Error;

    fn matched(&mut self, _searcher: &Searcher, mat: &SinkMatch<'_>) -> Result<bool, Self::Error> {
        let line = String::from_utf8_lossy(mat.bytes()).trim_end().to_string();
        let entry = format!("{}:{}: {}", self.path, mat.line_number().unwrap_or(0), line);
        let mut lock = self.results.lock().expect("lock");
        lock.push(entry);
        Ok(lock.len() < self.max)
    }
}

/// Resolve a path string against the working directory.
fn resolve_path(working_dir: &Path, path_str: &str) -> PathBuf {
    let p = PathBuf::from(path_str);
    if p.is_absolute() {
        p
    } else {
        working_dir.join(p)
    }
}
