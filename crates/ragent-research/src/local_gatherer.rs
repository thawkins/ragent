//! Local-gathering phase for the research system (FR-006, FR-008, FR-009).
//!
//! This module implements the orchestration logic that turns a research
//! topic into a list of [`Source::Local`] and [`Source::Spec`] entries by
//! scanning the local filesystem for relevant files. The actual filesystem
//! and ripgrep calls are made through the [`LocalTool`] trait abstraction
//! so the gatherer can be unit-tested with in-memory fakes and reused
//! from any integration context (TUI agent loop, CLI, HTTP endpoint).
//!
//! ## Flow
//!
//! 1. [`LocalGatherer::gather`] enumerates candidate files using
//!    [`LocalTool::glob`] against the project root and the optional
//!    `--sources-dir` directory (FR-019).
//! 2. For each candidate it checks for keyword matches via
//!    [`LocalTool::grep`] (case-insensitive, term OR).
//! 3. Files with at least one match are read via [`LocalTool::read`] and
//!    turned into [`Source::Local`] entries with synthetic supporting
//!    paths `sources/local-NN.md`.
//! 4. Prior specs under `specs/` are scanned separately via
//!    [`LocalTool::list_specs`] and become [`Source::Spec`] entries with
//!    a one-line relevance note derived from the spec title.
//!
//! ## Reuse, not reimplementation
//!
//! Per the spec constraints, the gatherer does **not** reimplement file
//! discovery or grep - it delegates entirely to the `LocalTool`
//! implementation. In production this wraps the existing `glob`, `grep`,
//! `read`, and `list` tools from `crates/ragent-tools-core`.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use async_trait::async_trait;
use chrono::Utc;
use futures::stream::StreamExt;

use crate::source::{LocalSourceKind, Source};

/// Default set of glob patterns scanned during local gathering.
///
/// Includes Rust, Markdown, JSON/YAML/TOML config, Python, TypeScript, and
/// a few other common file types found in ragent projects. Callers may
/// override these via `LocalGatherer::with_globs`.
pub const DEFAULT_GLOBS: &[&str] = &[
    "**/*.rs",
    "**/*.md",
    "**/*.toml",
    "**/*.json",
    "**/*.yaml",
    "**/*.yml",
    "**/*.py",
    "**/*.ts",
    "**/*.tsx",
    "**/*.js",
    "**/*.c",
    "**/*.cpp",
    "**/*.h",
    "**/*.hpp",
    "**/*.go",
    "**/*.java",
];

/// Maximum number of local sources to capture in a single gather call.
pub const DEFAULT_MAX_LOCAL_SOURCES: usize = 10;

/// Trait abstracting the existing `glob` / `grep` / `read` / `list` tools
/// used by [`LocalGatherer`].
#[async_trait]
pub trait LocalTool: Send + Sync {
    /// Glob a project-relative pattern and return matching file paths.
    async fn glob(&self, project_root: &Path, pattern: &str) -> anyhow::Result<Vec<PathBuf>>;

    /// Run a case-insensitive search for any of `terms` in `path` and
    /// return the matching line numbers (1-based) plus the matched lines
    /// themselves.
    async fn grep(&self, path: &Path, terms: &[String]) -> anyhow::Result<Vec<GrepMatch>>;

    /// Read the full contents of `path` as a UTF-8 string.
    async fn read(&self, path: &Path) -> anyhow::Result<String>;

    /// List spec directories directly under `specs/`. Used for FR-009
    /// cross-referencing of prior specs.
    async fn list_specs(&self, project_root: &Path) -> anyhow::Result<Vec<String>>;

    /// Read the first 30-ish lines of a SPEC.md and return the title
    /// (text of the first `#` heading) if one can be found.
    async fn spec_title(&self, project_root: &Path, spec_id: &str) -> anyhow::Result<String>;
}

/// A single matching line from [`LocalTool::grep`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GrepMatch {
    /// 1-based line number within the file.
    pub line: usize,
    /// The full line text (without trailing newline).
    pub text: String,
}

/// Errors emitted by [`LocalGatherer`].
#[derive(Debug, thiserror::Error)]
pub enum LocalGatherError {
    /// The configured source limit was zero - there is nothing to gather.
    #[error("local gatherer called with max_local_sources = 0")]
    ZeroLimit,
    /// An empty topic (zero keyword terms) was supplied.
    #[error("local gatherer called with no keyword terms")]
    NoTerms,
}

/// One candidate file discovered during the local-gathering phase.
#[derive(Debug, Clone, PartialEq, Eq)]
struct LocalCandidate {
    path: PathBuf,
    /// `LocalSourceKind::InProject` for project-root matches,
    /// `LocalSourceKind::Extra` for `--sources-dir` matches.
    kind: LocalSourceKind,
}

/// Default number of concurrent local candidate scoring/spec-scan tasks.
pub const DEFAULT_LOCAL_CONCURRENCY: usize = 8;

/// Configuration knobs for [`LocalGatherer`].
#[derive(Debug, Clone)]
pub struct LocalGatherConfig {
    /// Glob patterns to scan. Defaults to [`DEFAULT_GLOBS`].
    pub globs: Vec<String>,
    /// Maximum number of `Source::Local` entries to emit. Defaults to
    /// [`DEFAULT_MAX_LOCAL_SOURCES`].
    pub max_local_sources: usize,
    /// Keyword terms derived from the research topic (split on whitespace,
    /// lowercased, deduped, length >= 2 chars).
    pub terms: Vec<String>,
    /// When `true`, skip the spec cross-reference pass that produces
    /// [`Source::Spec`] entries. Defaults to `false`.
    pub skip_specs: bool,
    /// Maximum number of concurrent candidate scoring or spec-scan tasks.
    /// Defaults to [`DEFAULT_LOCAL_CONCURRENCY`] (8). `0` is clamped up to `1`.
    pub local_concurrency: usize,
}

impl Default for LocalGatherConfig {
    fn default() -> Self {
        Self {
            globs: DEFAULT_GLOBS
                .iter()
                .map(std::string::ToString::to_string)
                .collect(),
            max_local_sources: DEFAULT_MAX_LOCAL_SOURCES,
            terms: Vec::new(),
            skip_specs: false,
            local_concurrency: DEFAULT_LOCAL_CONCURRENCY,
        }
    }
}

/// Orchestrates a single local-gathering pass for one research topic.
#[derive(Clone)]
pub struct LocalGatherer {
    tool: Arc<dyn LocalTool>,
}

impl std::fmt::Debug for LocalGatherer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("LocalGatherer").finish_non_exhaustive()
    }
}

impl LocalGatherer {
    /// Construct a new gatherer that delegates filesystem operations to
    /// `tool`.
    pub fn new(tool: Arc<dyn LocalTool>) -> Self {
        Self { tool }
    }

    /// Gather local sources for `topic` from `project_root`.
    ///
    /// `extra_sources_dir` is the optional FR-019 `--sources-dir` path; any
    /// file matches from there are tagged [`LocalSourceKind::Extra`] in
    /// the references index.
    pub async fn gather(
        &self,
        project_root: &Path,
        topic: &str,
        extra_sources_dir: Option<&Path>,
        config: &LocalGatherConfig,
    ) -> Result<Vec<Source>, LocalGatherError> {
        if config.max_local_sources == 0 {
            return Err(LocalGatherError::ZeroLimit);
        }
        let terms = derive_terms(topic, &config.terms);
        if terms.is_empty() {
            return Err(LocalGatherError::NoTerms);
        }

        tracing::info!(
            project_root = %project_root.display(),
            terms = ?terms,
            max = config.max_local_sources,
            extra_dir = ?extra_sources_dir,
            "research: starting local-gathering phase"
        );

        // 1. Enumerate candidate files via globs.
        let mut candidates = self
            .enumerate_candidates(project_root, &config.globs, LocalSourceKind::InProject)
            .await;
        if let Some(extra) = extra_sources_dir {
            let extra_candidates = self
                .enumerate_candidates(extra, &config.globs, LocalSourceKind::Extra)
                .await;
            candidates.extend(extra_candidates);
        }
        // Deduplicate by canonical path while preserving insertion order.
        let mut seen = std::collections::HashSet::new();
        candidates.retain(|c| seen.insert(c.path.clone()));

        // 2. Score candidates by grep hits and keep the top N.
        let concurrency = config.local_concurrency.max(1);
        tracing::info!(
            candidate_count = candidates.len(),
            concurrency,
            "research: scoring local candidates with bounded concurrency"
        );
        let scored = self
            .score_candidates(&candidates, &terms, concurrency)
            .await;
        let mut sources = Vec::new();
        for (index, (candidate, matches, matched_terms)) in scored
            .into_iter()
            .take(config.max_local_sources)
            .enumerate()
        {
            let body_path = local_body_path(index);
            // Read the file so we can embed an excerpt in the supporting file
            // and so the relevance note can show the *first* matching line
            // instead of just a keyword count.
            let raw_body = match self.tool.read(&candidate.path).await {
                Ok(b) => b,
                Err(e) => {
                    tracing::warn!(
                        path = %candidate.path.display(),
                        error = %e,
                        "research: read failed during local gathering; capturing matches only"
                    );
                    String::new()
                }
            };
            let excerpt = build_local_excerpt(&raw_body, &matches, MAX_LOCAL_EXCERPT_LINES);
            let relevance = build_relevance_note(&matched_terms, &matches);
            tracing::info!(
                path = %candidate.path.display(),
                kind = ?candidate.kind,
                matches = matches.len(),
                matched_terms = ?matched_terms,
                body_chars = excerpt.chars().count(),
                "research: captured local source"
            );
            sources.push(Source::Local {
                path: candidate.path.display().to_string(),
                kind: candidate.kind,
                captured_at: Utc::now(),
                body_path,
                relevance,
                body: excerpt,
            });
        }

        // 3. Cross-reference prior specs (FR-009). Skipped when the caller
        // set `skip_specs: true` (e.g. via `--no-specs`).
        let spec_sources = if config.skip_specs {
            tracing::info!(
                project_root = %project_root.display(),
                "research: skipping spec cross-reference (skip_specs=true)"
            );
            Vec::new()
        } else {
            self.gather_specs(project_root, &terms, config.max_local_sources, concurrency)
                .await
        };
        sources.extend(spec_sources);

        tracing::info!(
            count = sources.len(),
            "research: local-gathering phase complete"
        );
        Ok(sources)
    }

    async fn enumerate_candidates(
        &self,
        root: &Path,
        globs: &[String],
        kind: LocalSourceKind,
    ) -> Vec<LocalCandidate> {
        let mut out = Vec::new();
        for pattern in globs {
            match self.tool.glob(root, pattern).await {
                Ok(paths) => {
                    for path in paths {
                        // Trust the glob tool to return only files. A
                        // redundant `is_file()` syscall here would make
                        // the gatherer untestable against the in-memory
                        // fake used by the test suite, and in production
                        // `glob` already filters directories.
                        out.push(LocalCandidate { path, kind });
                    }
                }
                Err(e) => {
                    tracing::warn!(
                        pattern,
                        root = %root.display(),
                        error = %e,
                        "research: glob failed during local gathering; skipping pattern"
                    );
                }
            }
        }
        out
    }
}

/// A candidate paired with its grep matches and matched terms.
type ScoredCandidate = (LocalCandidate, Vec<GrepMatch>, Vec<String>);

impl LocalGatherer {
    async fn score_candidates(
        &self,
        candidates: &[LocalCandidate],
        terms: &[String],
        concurrency: usize,
    ) -> Vec<ScoredCandidate> {
        let concurrency = concurrency.max(1);
        let scored: Vec<Option<ScoredCandidate>> =
            futures::stream::iter(candidates.iter().cloned())
                .map(|candidate| async move {
                    match self.tool.grep(&candidate.path, terms).await {
                        Ok(matches) if !matches.is_empty() => {
                            let matched_terms = collect_matched_terms(&matches, terms);
                            Some((candidate, matches, matched_terms))
                        }
                        Ok(_) => None,
                        Err(e) => {
                            tracing::warn!(
                                path = %candidate.path.display(),
                                error = %e,
                                "research: grep failed during local gathering; skipping file"
                            );
                            None
                        }
                    }
                })
                .buffer_unordered(concurrency)
                .collect()
                .await;

        let mut scored: Vec<ScoredCandidate> = scored.into_iter().flatten().collect();
        // Stable sort: highest match-count first, then by path for determinism.
        scored.sort_by(|a, b| {
            b.1.len()
                .cmp(&a.1.len())
                .then_with(|| a.0.path.cmp(&b.0.path))
        });
        scored
    }

    async fn gather_specs(
        &self,
        project_root: &Path,
        terms: &[String],
        max_total: usize,
        concurrency: usize,
    ) -> Vec<Source> {
        let spec_ids = match self.tool.list_specs(project_root).await {
            Ok(ids) => ids,
            Err(e) => {
                tracing::warn!(error = %e, "research: list_specs failed; no spec sources");
                return Vec::new();
            }
        };

        let concurrency = concurrency.max(1);
        let scored: Vec<(isize, String, String)> = futures::stream::iter(spec_ids)
            .map(|spec_id| async move {
                let title = self
                    .tool
                    .spec_title(project_root, &spec_id)
                    .await
                    .unwrap_or_default();
                let haystack = format!("{} {}", spec_id.to_lowercase(), title.to_lowercase());
                let score = terms
                    .iter()
                    .map(|term| if haystack.contains(term) { 1 } else { 0 })
                    .sum::<isize>();
                (score, spec_id, title)
            })
            .buffer_unordered(concurrency)
            .collect()
            .await;

        // Split specs into those matching a keyword and the rest.
        let mut matched = Vec::new();
        let mut rest = Vec::new();
        for (score, spec_id, title) in scored {
            if score > 0 {
                matched.push((spec_id, title));
            } else {
                rest.push((spec_id, title));
            }
        }

        // FR-009: prefer relevant specs, but fall back to all specs when the
        // filter is too restrictive (fewer than 3 matches) so research still
        // has useful cross-references.
        let selected: Vec<_> = if matched.len() >= 3 {
            matched
        } else {
            let mut combined = matched;
            combined.extend(rest);
            combined
        };

        let mut sources = Vec::new();
        for (spec_id, title) in selected.into_iter().take(max_total) {
            let relevance = if title.is_empty() {
                format!("Spec {spec_id} under specs/")
            } else {
                format!("Spec {spec_id}: {title}")
            };
            sources.push(Source::Spec {
                spec_id,
                captured_at: Utc::now(),
                relevance,
            });
        }
        sources
    }
}

/// Maximum number of lines to include in a local source excerpt. Keeps the
/// supporting file small and the synthesis prompt focused on the most
/// relevant context around each keyword match.
pub const MAX_LOCAL_EXCERPT_LINES: usize = 30;

/// Compute the zero-padded supporting-file path for the Nth local source.
///
/// Index 0 -> `local-01.md`, index 1 -> `local-02.md`, etc.
#[must_use]
pub fn local_body_path(index: usize) -> PathBuf {
    PathBuf::from(format!("sources/local-{:02}.md", index + 1))
}

/// Build the `body` field for a [`Source::Local`] entry.
///
/// Produces a markdown excerpt that:
/// - Header line showing the file path and match count.
/// - Up to `MAX_LOCAL_EXCERPT_LINES` matching lines plus surrounding context
///   (one line of context on either side when available).
/// - A trailing ellipsis marker if there were more matches.
///
/// When the body is empty (read failed) we fall back to the matched lines
/// only, since those are still informative.
#[must_use]
pub fn build_local_excerpt(body: &str, matches: &[GrepMatch], max_lines: usize) -> String {
    if matches.is_empty() {
        return String::new();
    }
    let lines: Vec<(usize, &str)> = body.lines().enumerate().collect();
    // Defensive: clamp the requested max to at least one line so we never
    // accidentally produce an empty excerpt when the caller asks for 0.
    let max_lines = max_lines.max(1);
    // Build a set of match line numbers (1-based, like GrepMatch.line).
    let mut match_set: std::collections::BTreeSet<usize> = matches.iter().map(|m| m.line).collect();
    // For each match, also include one line of context on either side
    // (when available). Context lines are tagged so we don't render the
    // exact line twice if it's both a match and a context neighbour.
    let mut context_lines: Vec<usize> = Vec::new();
    for m in matches {
        if m.line > 1 {
            context_lines.push(m.line - 1);
        }
        context_lines.push(m.line + 1);
    }
    for c in context_lines {
        if c >= 1 {
            match_set.insert(c);
        }
    }

    let mut out = String::new();
    let total_matches = matches.len();
    out.push_str(&format!("Excerpt -> {total_matches} keyword match(es)\n\n"));
    let mut included = 0usize;
    let mut last_emitted: Option<usize> = None;
    for (idx, text) in &lines {
        let one_based = idx + 1;
        if !match_set.contains(&one_based) {
            continue;
        }
        if let Some(prev) = last_emitted
            && one_based > prev + 1
        {
            out.push_str("...\n");
        }
        let marker = if matches.iter().any(|m| m.line == one_based) {
            ">"
        } else {
            " "
        };
        // Truncate the per-line text so a single very long line doesn't
        // dominate the excerpt.
        let line_text: String = text.chars().take(200).collect();
        out.push_str(&format!("{marker} {one_based:>4}: {line_text}\n"));
        last_emitted = Some(one_based);
        included += 1;
        if included >= max_lines {
            break;
        }
    }
    if total_matches > included {
        out.push_str(&format!(
            "\n... ({remaining} more match(es) elided)\n",
            remaining = total_matches.saturating_sub(included)
        ));
    }
    out
}

/// Build the `relevance` note for a [`Source::Local`] entry.
///
/// Replaces the legacy "X keyword match(es) for research topic" string with a
/// more informative note: the matched keywords (truncated to 3) plus a short
/// snippet of the first matching line so the user can see *why* the file is
/// relevant without opening it.
pub fn build_relevance_note(matched_terms: &[String], matches: &[GrepMatch]) -> String {
    if matches.is_empty() {
        return "matched no keyword terms (relevance tag retained)".into();
    }
    let terms_str = if matched_terms.is_empty() {
        "keyword match(es)".to_string()
    } else {
        let shown: Vec<&str> = matched_terms.iter().take(3).map(String::as_str).collect();
        if matched_terms.len() > 3 {
            format!("{}, ...(+{})", shown.join(", "), matched_terms.len() - 3)
        } else {
            shown.join(", ")
        }
    };
    let first = matches
        .first()
        .map_or("", |m| m.text.trim())
        .chars()
        .take(120)
        .collect::<String>();
    if first.is_empty() {
        format!("{} match(es) on: {}", matches.len(), terms_str)
    } else {
        format!(
            "{} match(es) on: {} - \"{}\"",
            matches.len(),
            terms_str,
            first
        )
    }
}

/// Walk the grep matches and figure out which of `terms` actually appeared.
/// Returns a deduplicated, lowercase vec preserving first-seen order.
#[must_use]
pub fn collect_matched_terms(matches: &[GrepMatch], terms: &[String]) -> Vec<String> {
    // Pre-lowercase and filter terms once so the inner loop avoids
    // redundant `to_lowercase()` allocations (PERF-LOC-01). Terms may
    // arrive in mixed case (e.g. from test inputs), so we lower-case
    // them here rather than assuming they are already lowercased.
    let lower_terms: Vec<String> = terms
        .iter()
        .map(|t| t.to_lowercase())
        .filter(|t| t.len() >= 2)
        .collect();
    let lower_terms: Vec<&str> = lower_terms.iter().map(|s| s.as_str()).collect();
    let mut seen: std::collections::HashSet<&str> = std::collections::HashSet::new();
    let mut out: Vec<String> = Vec::new();
    for m in matches {
        // One lowercase allocation per match line instead of per (match, term) pair.
        let lower = m.text.to_lowercase();
        for &t in &lower_terms {
            if lower.contains(t) && seen.insert(t) {
                out.push(t.to_string());
            }
        }
    }
    out
}

/// Derive keyword terms for `topic`, falling back to `fallback` when
/// `topic` is empty. Returns a deduplicated, lowercased vec of tokens
/// of length >= 2 (so we don't try to grep for single letters) drawn
/// from either source.
#[must_use]
pub fn derive_terms(topic: &str, fallback: &[String]) -> Vec<String> {
    // Replace ASCII punctuation (except apostrophes in contractions) with spaces
    // so tokens like "async/await," become "async" and "await".
    let cleaned: String = topic
        .chars()
        .map(|c| {
            if c.is_ascii_punctuation() && c != '\'' {
                ' '
            } else {
                c
            }
        })
        .collect();
    let mut terms: Vec<String> = cleaned
        .split_whitespace()
        .map(str::to_lowercase)
        .filter(|s| s.len() >= 2)
        .collect();
    if terms.is_empty() {
        let cleaned_fallback: String = fallback
            .iter()
            .map(|s| {
                s.chars()
                    .map(|c| {
                        if c.is_ascii_punctuation() && c != '\'' {
                            ' '
                        } else {
                            c
                        }
                    })
                    .collect::<String>()
            })
            .collect::<Vec<_>>()
            .join(" ");
        terms = cleaned_fallback
            .split_whitespace()
            .map(str::to_lowercase)
            .filter(|s| s.len() >= 2)
            .collect();
    }
    terms.sort();
    terms.dedup();
    terms
}

#[cfg(test)]
#[path = "../tests/inline/local_gatherer_tests.rs"]
mod tests;
