//! Structure-aware code search tool (`agentgrep`).
//!
//! `agentgrep` is a `grep`-like tool that also returns file structure metadata
//! (function list, line ranges, symbol displacement) and can truncate results
//! based on what the session has already read. It is intended as a richer
//! alternative to `grep` when the model needs to understand code context around
//! matches.

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use super::{Tool, ToolContext, ToolOutput};
use crate::codeindex_utils::{busy_output, codeindex_not_available, with_retry};

/// Structure-aware code search tool.
pub struct AgentGrepTool;

/// Search mode for `agentgrep`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
enum AgentGrepMode {
    /// Literal regex search with symbol context.
    #[default]
    Grep,
    /// Return a file outline (symbols only).
    Outline,
    /// Smart mode: search by query, rank by symbol relevance.
    Smart,
    /// Find files by name / glob.
    Find,
}

impl AgentGrepMode {
    fn from_str(s: &str) -> Option<Self> {
        match s {
            "grep" => Some(Self::Grep),
            "outline" => Some(Self::Outline),
            "smart" => Some(Self::Smart),
            "find" => Some(Self::Find),
            _ => None,
        }
    }
}

#[async_trait::async_trait]
impl Tool for AgentGrepTool {
    fn name(&self) -> &'static str {
        "agentgrep"
    }

    fn description(&self) -> &'static str {
        "Structure-aware code search. Returns grep-style matches enriched with symbol \
         boundaries, file outlines, and displacement from already-read regions. \
         Modes: `grep` (regex search with context), `outline` (symbols only), \
         `smart` (ranked semantic-ish search via code index), `find` (files by glob). \
         USE THIS instead of `grep` when you need to know which function/struct a match \
         belongs to, or to skip regions you have already read."
    }

    fn parameters_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "mode": {
                    "type": "string",
                    "description": "Search mode: grep, outline, smart, find",
                    "enum": ["grep", "outline", "smart", "find"],
                    "default": "grep"
                },
                "query": {
                    "type": "string",
                    "description": "Search query: regex for grep/smart, optional filter for outline/find"
                },
                "path": {
                    "type": "string",
                    "description": "Directory or file to search in (default: working directory)"
                },
                "glob": {
                    "type": "string",
                    "description": "Glob pattern to restrict files (e.g. '*.rs', '**/*.ts')"
                },
                "max_regions": {
                    "type": "integer",
                    "description": "Maximum symbol regions to return per file (default: 10)",
                    "default": 10
                },
                "max_files": {
                    "type": "integer",
                    "description": "Maximum files to return (default: 20)",
                    "default": 20
                },
                "max_matches_per_region": {
                    "type": "integer",
                    "description": "Maximum matching lines to include inside each region (default: 5)",
                    "default": 5
                },
                "full_region": {
                    "type": "boolean",
                    "description": "When true, include the full symbol body text (default: false)"
                },
                "include_already_read": {
                    "type": "boolean",
                    "description": "When true, include regions already read in this session (default: false)"
                }
            },
            "required": ["query"],
            "additionalProperties": false
        })
    }

    fn permission_category(&self) -> &'static str {
        "codeindex:read"
    }

    async fn execute(&self, input: Value, ctx: &ToolContext) -> Result<ToolOutput> {
        let idx = match &ctx.code_index {
            Some(idx) => idx,
            None => {
                return Ok(codeindex_not_available(
                    "Use `grep` as a fallback tool.",
                    &["grep"],
                ));
            }
        };

        let mode = input["mode"]
            .as_str()
            .and_then(AgentGrepMode::from_str)
            .unwrap_or_default();

        let query = input["query"]
            .as_str()
            .context("Missing required 'query' parameter")?;

        let path = input["path"].as_str().map(String::from);
        let glob = input["glob"].as_str().map(String::from);
        let max_regions = input["max_regions"].as_u64().map_or(10, |n| n.max(1)) as usize;
        let max_files = input["max_files"].as_u64().map_or(20, |n| n.max(1)) as usize;
        let max_matches_per_region = input["max_matches_per_region"]
            .as_u64()
            .map_or(5, |n| n.max(1)) as usize;
        let full_region = input["full_region"].as_bool().unwrap_or(false);
        let include_already_read = input["include_already_read"].as_bool().unwrap_or(false);

        let root = if let Some(p) = &path {
            resolve_path(&ctx.working_dir, p)
        } else {
            ctx.working_dir.clone()
        };

        // Gather already-read absolute file paths and line ranges from session history.
        let read_regions = if include_already_read {
            ReadRegions::default()
        } else {
            read_regions_from_ctx(ctx, &root)
        };

        match mode {
            AgentGrepMode::Grep => {
                run_grep_mode(
                    idx,
                    query,
                    &root,
                    glob.as_deref(),
                    max_regions,
                    max_files,
                    max_matches_per_region,
                    full_region,
                    &read_regions,
                )
                .await
            }
            AgentGrepMode::Outline => {
                run_outline_mode(
                    idx,
                    query,
                    &root,
                    glob.as_deref(),
                    max_regions,
                    max_files,
                    full_region,
                    &read_regions,
                )
                .await
            }
            AgentGrepMode::Smart => {
                run_smart_mode(
                    idx,
                    query,
                    &root,
                    glob.as_deref(),
                    max_regions,
                    max_files,
                    max_matches_per_region,
                    full_region,
                    &read_regions,
                )
                .await
            }
            AgentGrepMode::Find => {
                run_find_mode(idx, query, &root, glob.as_deref(), max_files, &read_regions).await
            }
        }
    }
}

/// Resolve a possibly-relative path against the working directory.
fn resolve_path(working_dir: &Path, p: &str) -> PathBuf {
    let path = PathBuf::from(p);
    if path.is_absolute() {
        path
    } else {
        working_dir.join(path)
    }
}

/// Set of already-read file paths and line numbers.
#[derive(Debug, Default, Clone)]
struct ReadRegions {
    files: HashSet<PathBuf>,
    lines: HashMap<PathBuf, HashSet<u32>>,
}

impl ReadRegions {
    fn is_file_read(&self, path: &Path) -> bool {
        self.files.contains(path)
    }

    fn is_line_read(&self, path: &Path, line: u32) -> bool {
        self.lines.get(path).is_some_and(|set| set.contains(&line))
    }

    fn any_line_read(&self, path: &Path, start: u32, end: u32) -> bool {
        self.lines
            .get(path)
            .is_some_and(|set| (start..=end).any(|l| set.contains(&l)))
    }
}

fn read_regions_from_ctx(ctx: &ToolContext, root: &Path) -> ReadRegions {
    let mut regions = ReadRegions::default();
    let read_lock = ctx
        .read_timestamps
        .read()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    for path in read_lock.keys() {
        let canonical = if path.exists() {
            path.canonicalize().unwrap_or_else(|_| path.clone())
        } else {
            path.clone()
        };
        // Track whole-file reads as well as line ranges. Since the read tool
        // doesn't currently expose exact line ranges, we treat a read of the
        // file as having read lines 1..=100000 as a conservative approximation.
        regions.files.insert(canonical.clone());
        let mut lines = HashSet::new();
        for l in 1..=100_000 {
            lines.insert(l);
        }
        regions.lines.insert(canonical, lines);
    }

    // Also consider the requested root path itself as a read anchor if it is a file.
    if root.is_file() {
        let canonical = root.canonicalize().unwrap_or_else(|_| root.to_path_buf());
        regions.files.insert(canonical.clone());
        let mut lines = HashSet::new();
        for l in 1..=100_000 {
            lines.insert(l);
        }
        regions.lines.insert(canonical, lines);
    }
    regions
}

/// A region is a symbol boundary containing one or more matches.
#[derive(Debug, Clone)]
struct Region {
    symbol_name: String,
    symbol_kind: String,
    start_line: u32,
    end_line: u32,
    matches: Vec<(u32, String)>,
    signature: Option<String>,
    doc: Option<String>,
}

#[allow(clippy::too_many_arguments)]
async fn run_grep_mode(
    idx: &ragent_codeindex::CodeIndex,
    query: &str,
    root: &Path,
    glob: Option<&str>,
    max_regions: usize,
    max_files: usize,
    max_matches_per_region: usize,
    full_region: bool,
    read_regions: &ReadRegions,
) -> Result<ToolOutput> {
    // First collect raw grep matches using ripgrep for speed and regex fidelity.
    let raw_matches = grep_files(root, query, glob).await?;

    if raw_matches.is_empty() {
        return Ok(ToolOutput {
            content: format!("No matches found for '{query}'."),
            metadata: Some(json!({"total_results": 0})),
        });
    }

    // Group raw matches by file, then load symbols for those files and build regions.
    let mut by_file: HashMap<PathBuf, Vec<(u32, String)>> = HashMap::new();
    for (path, line, text) in raw_matches {
        by_file.entry(path).or_default().push((line, text));
    }

    let mut file_regions: Vec<(PathBuf, Vec<Region>)> = Vec::new();
    for (path, matches) in by_file.iter() {
        if read_regions.is_file_read(path)
            && !matches
                .iter()
                .any(|(line, _)| !read_regions.is_line_read(path, *line))
        {
            continue;
        }

        let rel = relative_path(root, path);
        let symbols = load_symbols_for_file(idx, &rel).await?;
        let mut regions = build_regions(matches, &symbols, max_matches_per_region);
        // Sort by number of matches descending and truncate.
        regions.sort_by_key(|b| std::cmp::Reverse(b.matches.len()));
        regions.truncate(max_regions);
        if !regions.is_empty() {
            file_regions.push((path.clone(), regions));
        }
    }

    file_regions.sort_by(|a, b| {
        let a_score: usize = a.1.iter().map(|r| r.matches.len()).sum();
        let b_score: usize = b.1.iter().map(|r| r.matches.len()).sum();
        b_score.cmp(&a_score)
    });
    file_regions.truncate(max_files);

    let rendered = render_regions(&file_regions, root, full_region);
    let total_matches = file_regions
        .iter()
        .map(|(_, rs)| rs.iter().map(|r| r.matches.len()).sum::<usize>())
        .sum::<usize>();

    Ok(ToolOutput {
        content: rendered,
        metadata: Some(json!({
            "total_results": total_matches,
            "files": file_regions.len(),
            "regions": file_regions.iter().map(|(_, rs)| rs.len()).sum::<usize>(),
        })),
    })
}

#[allow(clippy::too_many_arguments)]
async fn run_outline_mode(
    idx: &ragent_codeindex::CodeIndex,
    query: &str,
    root: &Path,
    glob: Option<&str>,
    max_regions: usize,
    max_files: usize,
    full_region: bool,
    read_regions: &ReadRegions,
) -> Result<ToolOutput> {
    let filter = build_symbol_filter(query, glob);
    let symbols = match with_retry(|| idx.try_symbols(&filter)).await? {
        Some(s) => s,
        None => return Ok(busy_output("agentgrep")),
    };

    let mut by_file: HashMap<PathBuf, Vec<ragent_codeindex::types::Symbol>> = HashMap::new();
    for sym in symbols {
        let path = root.join(sym.file_path().unwrap_or_default());
        by_file.entry(path).or_default().push(sym);
    }

    let mut file_regions: Vec<(PathBuf, Vec<Region>)> = Vec::new();
    for (path, mut syms) in by_file {
        if read_regions.is_file_read(&path) {
            syms.retain(|s| !read_regions.any_line_read(&path, s.start_line, s.end_line));
        }
        syms.truncate(max_regions);
        let regions = syms.into_iter().map(symbol_to_region).collect::<Vec<_>>();
        if !regions.is_empty() {
            file_regions.push((path, regions));
        }
    }

    file_regions.truncate(max_files);
    let rendered = render_regions(&file_regions, root, full_region);

    Ok(ToolOutput {
        content: rendered,
        metadata: Some(json!({
            "total_results": file_regions.iter().map(|(_, rs)| rs.len()).sum::<usize>(),
            "files": file_regions.len(),
        })),
    })
}

#[allow(clippy::too_many_arguments)]
async fn run_smart_mode(
    idx: &ragent_codeindex::CodeIndex,
    query: &str,
    root: &Path,
    glob: Option<&str>,
    max_regions: usize,
    max_files: usize,
    max_matches_per_region: usize,
    full_region: bool,
    read_regions: &ReadRegions,
) -> Result<ToolOutput> {
    // Smart mode: start with FTS search over symbols, then fall back to grep if too few results.
    let file_pattern = glob.map(|g| g.to_string());
    let search_query = ragent_codeindex::types::SearchQuery {
        query: query.to_string(),
        kind: None,
        language: None,
        file_pattern,
        max_results: max_files * 4,
        include_body: false,
    };

    let mut fts_results = match with_retry(|| idx.try_search(&search_query)).await? {
        Some(r) => r,
        None => return Ok(busy_output("agentgrep")),
    };

    if fts_results.len() < max_files {
        // Supplement with literal grep.
        let raw = grep_files(root, query, glob).await?;
        let mut seen: HashSet<(PathBuf, u32)> = fts_results
            .iter()
            .map(|r| (root.join(&r.file_path), r.line))
            .collect();
        for (path, line, _text) in raw {
            if seen.insert((path.clone(), line)) {
                let rel = relative_path(root, &path);
                fts_results.push(ragent_codeindex::search::SearchResult {
                    symbol_name: String::new(),
                    qualified_name: String::new(),
                    kind: String::new(),
                    file_path: rel.display().to_string(),
                    line,
                    end_line: line,
                    score: 0.5,
                    signature: String::new(),
                    doc_snippet: String::new(),
                });
            }
        }
    }

    // Group by file.
    let mut by_file: HashMap<PathBuf, Vec<(u32, String)>> = HashMap::new();
    for r in fts_results {
        let path = root.join(&r.file_path);
        let text = line_text(&path, r.line).await.unwrap_or_default();
        by_file.entry(path).or_default().push((r.line, text));
    }

    let mut file_regions: Vec<(PathBuf, Vec<Region>)> = Vec::new();
    for (path, matches) in by_file.iter() {
        if read_regions.is_file_read(path)
            && !matches
                .iter()
                .any(|(line, _)| !read_regions.is_line_read(path, *line))
        {
            continue;
        }
        let rel = relative_path(root, path);
        let symbols = load_symbols_for_file(idx, &rel).await?;
        let mut regions = build_regions(matches, &symbols, max_matches_per_region);
        regions.sort_by_key(|b| std::cmp::Reverse(b.matches.len()));
        regions.truncate(max_regions);
        if !regions.is_empty() {
            file_regions.push((path.clone(), regions));
        }
    }

    file_regions.sort_by(|a, b| {
        let a_score: usize = a.1.iter().map(|r| r.matches.len()).sum();
        let b_score: usize = b.1.iter().map(|r| r.matches.len()).sum();
        b_score.cmp(&a_score)
    });
    file_regions.truncate(max_files);

    let rendered = render_regions(&file_regions, root, full_region);
    let total_matches = file_regions
        .iter()
        .map(|(_, rs)| rs.iter().map(|r| r.matches.len()).sum::<usize>())
        .sum::<usize>();

    Ok(ToolOutput {
        content: rendered,
        metadata: Some(json!({
            "total_results": total_matches,
            "files": file_regions.len(),
            "regions": file_regions.iter().map(|(_, rs)| rs.len()).sum::<usize>(),
        })),
    })
}

async fn run_find_mode(
    idx: &ragent_codeindex::CodeIndex,
    query: &str,
    root: &Path,
    glob: Option<&str>,
    max_files: usize,
    read_regions: &ReadRegions,
) -> Result<ToolOutput> {
    // Find files by glob/query. Prefer symbol/file FTS if the query looks like a symbol.
    let filter = build_symbol_filter(query, glob);
    let symbols = match with_retry(|| idx.try_symbols(&filter)).await? {
        Some(s) => s,
        None => return Ok(busy_output("agentgrep")),
    };

    let mut paths: Vec<PathBuf> = symbols
        .iter()
        .filter_map(|s| s.file_path().map(|p| root.join(p)))
        .collect();
    paths.sort();
    paths.dedup();

    // If no symbol matches, fall back to globbing.
    if paths.is_empty() {
        let pattern = glob.unwrap_or(query);
        let full_pattern = if PathBuf::from(pattern).is_absolute() {
            pattern.to_string()
        } else {
            format!("{}/**/{}", root.display(), pattern)
        };
        for path in glob::glob(&full_pattern)?.flatten() {
            paths.push(path);
        }
    }

    paths.truncate(max_files);
    paths.retain(|p| !read_regions.is_file_read(p));

    let lines: Vec<String> = paths
        .iter()
        .map(|p| relative_path(root, p).display().to_string())
        .collect();
    let content = if lines.is_empty() {
        format!(
            "No files found for '{}'{}",
            query,
            glob.map(|g| format!(" with glob '{g}'"))
                .unwrap_or_default()
        )
    } else {
        lines.join("\n")
    };

    Ok(ToolOutput {
        content,
        metadata: Some(json!({"total_results": paths.len()})),
    })
}

fn build_symbol_filter(query: &str, glob: Option<&str>) -> ragent_codeindex::types::SymbolFilter {
    let name = if query.is_empty() || glob.is_some_and(|_| false) {
        None
    } else {
        Some(query.to_string())
    };
    let mut file_path = glob.map(|g| {
        // Strip leading wildcards to make a substring filter.
        g.trim_start_matches("*/")
            .trim_start_matches('*')
            .to_string()
    });
    // If query is a path fragment, also filter by it.
    if query.contains('/') || query.contains('.') {
        file_path = Some(query.to_string());
    }
    ragent_codeindex::types::SymbolFilter {
        name,
        kind: None,
        file_path,
        language: None,
        visibility: None,
        limit: Some(200),
    }
}

async fn load_symbols_for_file(
    idx: &ragent_codeindex::CodeIndex,
    rel_path: &Path,
) -> Result<Vec<ragent_codeindex::types::Symbol>> {
    let filter = ragent_codeindex::types::SymbolFilter {
        name: None,
        kind: None,
        file_path: Some(rel_path.display().to_string()),
        language: None,
        visibility: None,
        limit: Some(200),
    };
    let symbols = match with_retry(|| idx.try_symbols(&filter)).await? {
        Some(s) => s,
        None => return Ok(Vec::new()),
    };
    Ok(symbols)
}

fn build_regions(
    matches: &[(u32, String)],
    symbols: &[ragent_codeindex::types::Symbol],
    max_matches_per_region: usize,
) -> Vec<Region> {
    let mut regions: Vec<Region> = Vec::new();
    for sym in symbols {
        let contained: Vec<(u32, String)> = matches
            .iter()
            .filter(|(line, _)| *line >= sym.start_line && *line <= sym.end_line)
            .cloned()
            .collect();
        if contained.is_empty() {
            continue;
        }
        // Check if any match already exists in another region with the exact same
        // boundaries; if so, prefer the more specific (smaller) symbol.
        let already = regions.iter().any(|r| {
            r.start_line == sym.start_line
                && r.end_line == sym.end_line
                && contained.iter().all(|m| r.matches.contains(m))
        });
        if already {
            continue;
        }
        regions.push(Region {
            symbol_name: sym.name.clone(),
            symbol_kind: sym.kind.to_string(),
            start_line: sym.start_line,
            end_line: sym.end_line,
            matches: contained.into_iter().take(max_matches_per_region).collect(),
            signature: sym.signature.clone(),
            doc: sym.doc_comment.clone(),
        });
    }

    // Any matches not inside a symbol become a "toplevel" region.
    let covered: HashSet<u32> = regions
        .iter()
        .flat_map(|r| r.start_line..=r.end_line)
        .collect();
    let uncovered: Vec<(u32, String)> = matches
        .iter()
        .filter(|(line, _)| !covered.contains(line))
        .cloned()
        .collect();
    if !uncovered.is_empty() {
        // Determine the span of uncovered matches.
        let min_line = uncovered.iter().map(|m| m.0).min().unwrap_or(1);
        let max_line = uncovered.iter().map(|m| m.0).max().unwrap_or(min_line);
        regions.push(Region {
            symbol_name: "toplevel".to_string(),
            symbol_kind: "file".to_string(),
            start_line: min_line,
            end_line: max_line,
            matches: uncovered.into_iter().take(max_matches_per_region).collect(),
            signature: None,
            doc: None,
        });
    }

    regions
}

fn symbol_to_region(sym: ragent_codeindex::types::Symbol) -> Region {
    Region {
        symbol_name: sym.name.clone(),
        symbol_kind: sym.kind.to_string(),
        start_line: sym.start_line,
        end_line: sym.end_line,
        matches: Vec::new(),
        signature: sym.signature.clone(),
        doc: sym.doc_comment.clone(),
    }
}

fn render_regions(
    file_regions: &[(PathBuf, Vec<Region>)],
    root: &Path,
    full_region: bool,
) -> String {
    let mut out = String::new();
    for (path, regions) in file_regions {
        let rel = relative_path(root, path);
        out.push_str(&format!("\n── {} ──\n", rel.display()));
        for region in regions {
            out.push_str(&format!(
                "  [{}] `{}` (L{}-{})",
                region.symbol_kind, region.symbol_name, region.start_line, region.end_line
            ));
            if let Some(sig) = &region.signature {
                out.push_str(&format!("\n    signature: {sig}"));
            }
            if let Some(doc) = &region.doc {
                let truncated = ragent_types::truncate_bytes(doc, 120);
                out.push_str(&format!("\n    doc: {truncated}"));
            }
            if full_region {
                if let Ok(text) = std::fs::read_to_string(path) {
                    let lines: Vec<&str> = text.lines().collect();
                    let start = (region.start_line as usize).saturating_sub(1);
                    let end = (region.end_line as usize).min(lines.len());
                    out.push_str("\n    ---");
                    for line in &lines[start..end] {
                        out.push_str(&format!("\n    {line}"));
                    }
                    out.push_str("\n    ---");
                }
            } else if !region.matches.is_empty() {
                out.push_str("\n    matches:");
                for (line, text) in &region.matches {
                    out.push_str(&format!("\n      L{line}: {text}"));
                }
            }
            out.push('\n');
        }
    }
    if out.is_empty() {
        "No matching regions found.".to_string()
    } else {
        out.trim_start().to_string()
    }
}

fn relative_path(root: &Path, path: &Path) -> PathBuf {
    path.strip_prefix(root).unwrap_or(path).to_path_buf()
}

async fn line_text(path: &Path, line: u32) -> Option<String> {
    let text = tokio::fs::read_to_string(path).await.ok()?;
    text.lines()
        .nth((line as usize).saturating_sub(1))
        .map(String::from)
}

async fn grep_files(
    root: &Path,
    query: &str,
    glob: Option<&str>,
) -> Result<Vec<(PathBuf, u32, String)>> {
    let mut matches = Vec::new();
    use grep_regex::RegexMatcherBuilder;
    use grep_searcher::{Searcher, SearcherBuilder, Sink, SinkMatch};
    use ignore::WalkBuilder;

    let matcher = RegexMatcherBuilder::new()
        .case_insensitive(false)
        .multi_line(false)
        .build(query)
        .with_context(|| format!("Invalid regex pattern: '{query}'"))?;

    let mut walk_builder = WalkBuilder::new(root);
    walk_builder
        .hidden(false)
        .git_ignore(true)
        .git_global(true)
        .git_exclude(true)
        .ignore(true)
        .filter_entry(|e| e.file_name() != ".git");

    if let Some(g) = glob {
        let mut ob = ignore::overrides::OverrideBuilder::new(root);
        let _ = ob.add(g);
        if let Ok(ov) = ob.build() {
            walk_builder.overrides(ov);
        }
    }

    struct CollectSink<'a> {
        base: &'a Path,
        path: &'a Path,
        matches: &'a mut Vec<(PathBuf, u32, String)>,
        limit: usize,
    }

    impl Sink for CollectSink<'_> {
        type Error = std::io::Error;
        fn matched(
            &mut self,
            _searcher: &Searcher,
            mat: &SinkMatch<'_>,
        ) -> Result<bool, Self::Error> {
            if self.matches.len() >= self.limit {
                return Ok(false);
            }
            let line_num = mat.line_number().unwrap_or(0) as u32;
            let line = std::str::from_utf8(mat.bytes())
                .unwrap_or("")
                .trim_end_matches(['\n', '\r'])
                .to_string();
            let display_path = self
                .path
                .strip_prefix(self.base)
                .unwrap_or(self.path)
                .to_path_buf();
            self.matches.push((display_path, line_num, line));
            Ok(true)
        }
    }

    let mut searcher = SearcherBuilder::new()
        .binary_detection(grep_searcher::BinaryDetection::quit(b'\0'))
        .line_number(true)
        .build();

    for entry in walk_builder.build().flatten() {
        if !entry.file_type().is_some_and(|ft| ft.is_file()) {
            continue;
        }
        let path = entry.path();
        let mut sink = CollectSink {
            base: root,
            path,
            matches: &mut matches,
            limit: 10_000,
        };
        let _ = searcher.search_path(&matcher, path, &mut sink);
    }

    // Convert display paths back to absolute.
    let absolute: Vec<(PathBuf, u32, String)> = matches
        .into_iter()
        .map(|(rel, line, text)| (root.join(rel), line, text))
        .collect();
    Ok(absolute)
}

// Extension trait to get file_path from Symbol (it exists as an inherent method in newer codeindex).
trait SymbolExt {
    fn file_path(&self) -> Option<&str>;
}

impl SymbolExt for ragent_codeindex::types::Symbol {
    fn file_path(&self) -> Option<&str> {
        // The Symbol struct has file_id but not file_path. We approximate by requiring callers
        // to resolve via the root. Returning None keeps the API simple here.
        None
    }
}
