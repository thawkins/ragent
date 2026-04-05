//! Text search tool for file contents.
//!
//! Provides [`GrepTool`], which searches files for a regex or literal pattern
//! using the ripgrep internal library crates:
//!
//! - [`ignore`] — directory walking that honours `.gitignore`, `.ignore`, and
//!   custom include/exclude glob patterns.
//! - [`grep_regex`] — regex matcher backed by the `regex` crate.
//! - [`grep_searcher`] — fast line-oriented searcher with encoding detection
//!   and automatic binary-file skipping.
//!
//! Results are capped at 500 matches and include the relative file path, line
//! number, and matching line content.

use anyhow::{Context, Result};
use grep_regex::RegexMatcherBuilder;
use grep_searcher::{Searcher, SearcherBuilder, Sink, SinkMatch};
use ignore::WalkBuilder;
use serde_json::{Value, json};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use super::{Tool, ToolContext, ToolOutput};

/// Maximum number of matching lines returned before results are truncated.
const MAX_RESULTS: usize = 500;

/// Searches file contents for a regex pattern across a directory tree.
///
/// Uses the ripgrep library crates for correct `.gitignore` handling, fast
/// line-oriented searching, automatic binary-file skipping, and real regex
/// support. Results are capped at [`MAX_RESULTS`] matches.
pub struct GrepTool;

#[async_trait::async_trait]
impl Tool for GrepTool {
    fn name(&self) -> &'static str {
        "grep"
    }

    fn description(&self) -> &'static str {
        "Search file contents for a regex pattern using ripgrep. \
         Respects .gitignore rules. Returns matching lines with file path \
         and line number. Supports regex, case-insensitive search, and \
         file-type glob filtering."
    }

    fn parameters_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "pattern": {
                    "type": "string",
                    "description": "Regex pattern to search for (Rust regex syntax)"
                },
                "path": {
                    "type": "string",
                    "description": "Directory or file to search in (default: working directory)"
                },
                "include": {
                    "type": "string",
                    "description": "Glob pattern to restrict which files are searched (e.g. '*.rs', '**/*.ts')"
                },
                "exclude": {
                    "type": "string",
                    "description": "Glob pattern of files/directories to exclude"
                },
                "case_insensitive": {
                    "type": "boolean",
                    "description": "Case-insensitive matching (default: false)"
                },
                "multiline": {
                    "type": "boolean",
                    "description": "Enable multiline mode — ^ and $ match line boundaries (default: false)"
                },
                "max_results": {
                    "type": "integer",
                    "description": "Maximum number of matches to return (default: 500, max: 500)"
                }
            },
            "required": ["pattern"]
        })
    }

    fn permission_category(&self) -> &'static str {
        "file:read"
    }

    /// Searches file contents for a regex pattern using the ripgrep library.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The `pattern` parameter is missing or not a valid regex.
    /// - The search path cannot be accessed.
    async fn execute(&self, input: Value, ctx: &ToolContext) -> Result<ToolOutput> {
        let pattern = input["pattern"]
            .as_str()
            .context("Missing required 'pattern' parameter")?;

        let search_path = input["path"].as_str().map_or_else(
            || ctx.working_dir.clone(),
            |p| resolve_path(&ctx.working_dir, p),
        );

        let include_glob = input["include"].as_str().map(str::to_owned);
        let exclude_glob = input["exclude"].as_str().map(str::to_owned);
        let case_insensitive = input["case_insensitive"].as_bool().unwrap_or(false);
        let multiline = input["multiline"].as_bool().unwrap_or(false);
        let max_results = input["max_results"]
            .as_u64()
            .map_or(MAX_RESULTS, |n| (n as usize).min(MAX_RESULTS));

        // Build the regex matcher — validates the pattern early before spawning
        let matcher = RegexMatcherBuilder::new()
            .case_insensitive(case_insensitive)
            .multi_line(multiline)
            .build(pattern)
            .with_context(|| format!("Invalid regex pattern: '{pattern}'"))?;

        // Shared accumulators used from the Sink callback on the blocking thread
        let results: Arc<Mutex<Vec<String>>> = Arc::new(Mutex::new(Vec::new()));
        let files_searched: Arc<Mutex<u64>> = Arc::new(Mutex::new(0));
        let truncated: Arc<Mutex<bool>> = Arc::new(Mutex::new(false));

        let results_bg = Arc::clone(&results);
        let files_bg = Arc::clone(&files_searched);
        let truncated_bg = Arc::clone(&truncated);
        let search_path_bg = search_path.clone();
        let pattern_owned = pattern.to_owned();

        tokio::task::spawn_blocking(move || {
            // Build the walker with .gitignore / .ignore support.
            // Keep hidden(false) so dot-files like .eslintrc are searchable, but
            // filter the .git directory itself to avoid searching VCS internals.
            let mut walk_builder = WalkBuilder::new(&search_path_bg);
            walk_builder
                .hidden(false)
                .git_ignore(true)
                .git_global(true)
                .git_exclude(true)
                .ignore(true)
                .filter_entry(|e| e.file_name() != ".git");

            // Apply include/exclude glob overrides
            if include_glob.is_some() || exclude_glob.is_some() {
                let mut ob = ignore::overrides::OverrideBuilder::new(&search_path_bg);
                if let Some(ref inc) = include_glob {
                    // Positive glob — only match these files
                    let _ = ob.add(inc);
                }
                if let Some(ref exc) = exclude_glob {
                    // Negative glob — exclude matching files
                    let neg = format!("!{exc}");
                    let _ = ob.add(&neg);
                }
                if let Ok(ov) = ob.build() {
                    walk_builder.overrides(ov);
                }
            }

            let mut searcher = SearcherBuilder::new()
                .binary_detection(grep_searcher::BinaryDetection::quit(b'\x00'))
                .line_number(true)
                .build();

            for entry in walk_builder.build().flatten() {
                // Stop walking if already at limit
                {
                    let r = results_bg
                        .lock()
                        .unwrap_or_else(std::sync::PoisonError::into_inner);
                    if r.len() >= max_results {
                        *truncated_bg
                            .lock()
                            .unwrap_or_else(std::sync::PoisonError::into_inner) = true;
                        break;
                    }
                }

                // Only search regular files
                if !entry.file_type().is_some_and(|ft| ft.is_file()) {
                    continue;
                }

                let path = entry.path().to_path_buf();
                *files_bg
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner) += 1;

                let sink = CollectSink {
                    path: &path,
                    base: &search_path_bg,
                    results: &results_bg,
                    truncated: &truncated_bg,
                    max_results,
                };

                // Per-file errors (binary, permission denied) are silently ignored
                let _ = searcher.search_path(&matcher, &path, sink);
            }

            // Keep borrow checker happy — pattern_owned is moved here for lifetime
            drop(pattern_owned);
        })
        .await
        .context("Grep search task panicked")?;

        let results = Arc::try_unwrap(results)
            .map_err(|_| anyhow::anyhow!("results Arc still has other owners"))?
            .into_inner()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let files_searched = *files_searched
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let truncated = *truncated
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);

        if results.is_empty() {
            Ok(ToolOutput {
                content: format!(
                    "No matches found for '{}' in {} ({} files searched)",
                    pattern,
                    search_path.display(),
                    files_searched
                ),
                metadata: None,
            })
        } else {
            let match_count = results.len();
            let content = results.join("\n");
            let summary = format!(
                "{} match{} in {} file{} searched{}",
                match_count,
                if match_count == 1 { "" } else { "es" },
                files_searched,
                if files_searched == 1 { "" } else { "s" },
                if truncated {
                    " (results truncated at limit)"
                } else {
                    ""
                },
            );
            Ok(ToolOutput {
                content: format!("{summary}\n\n{content}"),
                metadata: Some(json!({
                    "matches": match_count,
                    "files_searched": files_searched,
                    "truncated": truncated,
                    "pattern": pattern,
                })),
            })
        }
    }
}

/// [`Sink`] implementation that collects matching lines into a shared `Vec<String>`.
///
/// Each match is formatted as `relative/path:line_number:line_content`.
struct CollectSink<'a> {
    path: &'a Path,
    base: &'a Path,
    results: &'a Arc<Mutex<Vec<String>>>,
    truncated: &'a Arc<Mutex<bool>>,
    max_results: usize,
}

impl Sink for CollectSink<'_> {
    type Error = std::io::Error;

    fn matched(&mut self, _searcher: &Searcher, mat: &SinkMatch<'_>) -> Result<bool, Self::Error> {
        let mut results = self
            .results
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);

        if results.len() >= self.max_results {
            *self
                .truncated
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner) = true;
            return Ok(false); // stop searching this file
        }

        let line_num = mat.line_number().unwrap_or(0);
        let line = std::str::from_utf8(mat.bytes())
            .unwrap_or("")
            .trim_end_matches(['\n', '\r']);

        // Relative path for cleaner output
        let display_path = self
            .path
            .strip_prefix(self.base)
            .unwrap_or(self.path)
            .display()
            .to_string();

        results.push(format!("{display_path}:{line_num}:{line}"));
        Ok(true)
    }
}

/// Resolves a path string to an absolute [`PathBuf`] relative to the working directory.
fn resolve_path(working_dir: &Path, path_str: &str) -> PathBuf {
    let p = PathBuf::from(path_str);
    if p.is_absolute() {
        p
    } else {
        working_dir.join(p)
    }
}
