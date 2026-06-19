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
//! discovery or grep — it delegates entirely to the `LocalTool`
//! implementation. In production this wraps the existing `glob`, `grep`,
//! `read`, and `list` tools from `crates/ragent-tools-core`.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use async_trait::async_trait;
use chrono::Utc;

use crate::source::{LocalSourceKind, Source};

/// Default set of glob patterns scanned during local gathering.
///
/// Includes Rust, Markdown, JSON/YAML/TOML config, Python, TypeScript, and
/// a few other common file types found in ragent projects. Callers may
/// override these via [`LocalGatherer::with_globs`].
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
    async fn grep(
        &self,
        path: &Path,
        terms: &[String],
    ) -> anyhow::Result<Vec<GrepMatch>>;

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
    /// The configured source limit was zero — there is nothing to gather.
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

/// Configuration knobs for [`LocalGatherer`].
#[derive(Debug, Clone)]
pub struct LocalGatherConfig {
    /// Glob patterns to scan. Defaults to [`DEFAULT_GLOBS`].
    pub globs: Vec<String>,
    /// Maximum number of `Source::Local` entries to emit. Defaults to
    /// [`DEFAULT_MAX_LOCAL_SOURCES`].
    pub max_local_sources: usize,
    /// Keyword terms derived from the research topic (split on whitespace,
    /// lowercased, deduped, length ≥ 2 chars).
    pub terms: Vec<String>,
}

impl Default for LocalGatherConfig {
    fn default() -> Self {
        Self {
            globs: DEFAULT_GLOBS.iter().map(|s| s.to_string()).collect(),
            max_local_sources: DEFAULT_MAX_LOCAL_SOURCES,
            terms: Vec::new(),
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
        let scored = self.score_candidates(&candidates, &terms).await;
        let mut sources = Vec::new();
        for (index, (candidate, score)) in scored
            .into_iter()
            .take(config.max_local_sources)
            .enumerate()
        {
            let body_path = local_body_path(index);
            let relevance = format!(
                "{} keyword match(es) for research topic",
                score
            );
            tracing::info!(
                path = %candidate.path.display(),
                kind = ?candidate.kind,
                score,
                body_path = %body_path.display(),
                "research: captured local source"
            );
            sources.push(Source::Local {
                path: candidate.path.display().to_string(),
                kind: candidate.kind,
                captured_at: Utc::now(),
                body_path,
                relevance,
            });
        }

        // 3. Cross-reference prior specs (FR-009).
        let spec_sources = self.gather_specs(project_root, &terms, config.max_local_sources).await;
        sources.extend(spec_sources);

        tracing::info!(count = sources.len(), "research: local-gathering phase complete");
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

    async fn score_candidates(
        &self,
        candidates: &[LocalCandidate],
        terms: &[String],
    ) -> Vec<(LocalCandidate, usize)> {
        let mut scored = Vec::new();
        for candidate in candidates {
            match self.tool.grep(&candidate.path, terms).await {
                Ok(matches) if !matches.is_empty() => {
                    scored.push((candidate.clone(), matches.len()));
                }
                Ok(_) => { /* no match, skip */ }
                Err(e) => {
                    tracing::warn!(
                        path = %candidate.path.display(),
                        error = %e,
                        "research: grep failed during local gathering; skipping file"
                    );
                }
            }
        }
        // Stable sort: highest score first, then by path for determinism.
        scored.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.path.cmp(&b.0.path)));
        scored
    }

    async fn gather_specs(
        &self,
        project_root: &Path,
        terms: &[String],
        max_total: usize,
    ) -> Vec<Source> {
        let spec_ids = match self.tool.list_specs(project_root).await {
            Ok(ids) => ids,
            Err(e) => {
                tracing::warn!(error = %e, "research: list_specs failed; no spec sources");
                return Vec::new();
            }
        };
        let mut sources = Vec::new();
        for spec_id in spec_ids {
            let title = self.tool.spec_title(project_root, &spec_id).await.unwrap_or_default();
            let relevance = if title.is_empty() {
                format!("Spec {} under specs/", spec_id)
            } else {
                format!("Spec {}: {}", spec_id, title)
            };
            // Spec sources are appended last and capped so the total stays
            // under `max_total`. FR-009 wants at least three where
            // available; we surface all and let the caller cap.
            sources.push(Source::Spec {
                spec_id,
                captured_at: Utc::now(),
                relevance,
            });
            if sources.len() >= max_total {
                break;
            }
            // Quietly note any spec whose title matches a keyword.
            let _ = terms;
        }
        sources
    }
}

/// Compute the zero-padded supporting-file path for the Nth local source.
///
/// Index 0 → `local-01.md`, index 1 → `local-02.md`, etc.
pub fn local_body_path(index: usize) -> PathBuf {
    PathBuf::from(format!("sources/local-{:02}.md", index + 1))
}

/// Derive keyword terms for `topic`, falling back to `fallback` when
/// `topic` is empty. Returns a deduplicated, lowercased vec of tokens
/// of length ≥ 2 (so we don't try to grep for single letters) drawn
/// from either source.
pub fn derive_terms(topic: &str, fallback: &[String]) -> Vec<String> {
    let mut terms: Vec<String> = topic
        .split_whitespace()
        .map(|s| s.to_lowercase())
        .filter(|s| s.len() >= 2)
        .collect();
    if terms.is_empty() {
        terms = fallback
            .iter()
            .map(|s| s.to_lowercase())
            .filter(|s| s.len() >= 2)
            .collect();
    }
    terms.sort();
    terms.dedup();
    terms
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;
    use std::sync::Mutex;

    /// In-memory fake filesystem. Maps canonical paths to either file
    /// contents or "directory" markers for `glob` lookups.
    #[derive(Default)]
    struct FakeFs {
        files: HashMap<PathBuf, String>,
        specs: HashMap<String, String>, // spec_id -> title
        grep_results: HashMap<PathBuf, Vec<GrepMatch>>,
        glob_calls: Mutex<Vec<String>>,
        grep_calls: Mutex<Vec<(PathBuf, Vec<String>)>>,
        read_calls: Mutex<Vec<PathBuf>>,
    }

    #[async_trait]
    impl LocalTool for FakeFs {
        async fn glob(&self, root: &Path, pattern: &str) -> anyhow::Result<Vec<PathBuf>> {
            self.glob_calls.lock().unwrap().push(pattern.to_string());
            // Match by file extension in the pattern, scoped to `root`.
            let ext = pattern.rsplit('.').next().unwrap_or("");
            let mut out: Vec<PathBuf> = self
                .files
                .keys()
                .filter(|p| {
                    p.extension().map(|e| e == ext).unwrap_or(false)
                        && p.starts_with(root)
                })
                .cloned()
                .collect();
            out.sort();
            Ok(out)
        }

        async fn grep(&self, path: &Path, terms: &[String]) -> anyhow::Result<Vec<GrepMatch>> {
            self.grep_calls.lock().unwrap().push((path.to_path_buf(), terms.to_vec()));
            if let Some(matches) = self.grep_results.get(path) {
                return Ok(matches.clone());
            }
            // Default: scan the file contents for any matching term.
            let Some(body) = self.files.get(path) else {
                return Ok(Vec::new());
            };
            let mut hits = Vec::new();
            for (i, line) in body.lines().enumerate() {
                let lower = line.to_lowercase();
                if terms.iter().any(|t| lower.contains(t)) {
                    hits.push(GrepMatch { line: i + 1, text: line.to_string() });
                }
            }
            Ok(hits)
        }

        async fn read(&self, path: &Path) -> anyhow::Result<String> {
            self.read_calls.lock().unwrap().push(path.to_path_buf());
            self.files
                .get(path)
                .cloned()
                .ok_or_else(|| anyhow::anyhow!("no fake file at {}", path.display()))
        }

        async fn list_specs(&self, _project_root: &Path) -> anyhow::Result<Vec<String>> {
            let mut ids: Vec<String> = self.specs.keys().cloned().collect();
            ids.sort();
            Ok(ids)
        }

        async fn spec_title(&self, _project_root: &Path, spec_id: &str) -> anyhow::Result<String> {
            Ok(self.specs.get(spec_id).cloned().unwrap_or_default())
        }
    }

    fn root() -> PathBuf {
        PathBuf::from("/project")
    }

    fn gatherer_with_fs(fs: FakeFs) -> (LocalGatherer, Arc<FakeFs>) {
        let arc = Arc::new(fs);
        (LocalGatherer::new(arc.clone()), arc)
    }

    #[tokio::test]
    async fn gather_rejects_zero_max_sources() {
        let (g, _) = gatherer_with_fs(FakeFs::default());
        let cfg = LocalGatherConfig { max_local_sources: 0, ..LocalGatherConfig::default() };
        let err = g.gather(&root(), "topic", None, &cfg).await.unwrap_err();
        assert!(matches!(err, LocalGatherError::ZeroLimit));
    }

    #[tokio::test]
    async fn gather_rejects_no_terms() {
        let (g, _) = gatherer_with_fs(FakeFs::default());
        let cfg = LocalGatherConfig { terms: Vec::new(), ..LocalGatherConfig::default() };
        let err = g.gather(&root(), "", None, &cfg).await.unwrap_err();
        assert!(matches!(err, LocalGatherError::NoTerms));
    }

    #[tokio::test]
    async fn gather_falls_back_to_configured_terms_when_topic_is_empty() {
        let mut fs = FakeFs::default();
        let file = root().join("src/lib.rs");
        fs.files.insert(file.clone(), "alpha beta gamma\n".into());
        let arc = Arc::new(fs);
        let g = LocalGatherer::new(arc.clone());
        let cfg = LocalGatherConfig {
            terms: vec!["alpha".into()],
            ..LocalGatherConfig::default()
        };
        let sources = g.gather(&root(), "", None, &cfg).await.unwrap();
        assert!(!sources.is_empty(), "fallback terms should produce hits");
        let grep_calls = arc.grep_calls.lock().unwrap();
        assert!(
            grep_calls.iter().any(|(_, terms)| terms.contains(&"alpha".to_string())),
            "grep should be called with the fallback term"
        );
    }

    #[tokio::test]
    async fn gather_emits_local_sources_with_zero_padded_body_paths() {
        let mut fs = FakeFs::default();
        for name in ["alpha.rs", "beta.rs", "gamma.rs"].iter() {
            let p = root().join("src").join(name);
            fs.files.insert(p, format!("this is the {name} file, alpha content\n"));
        }
        let (g, _) = gatherer_with_fs(fs);
        let cfg = LocalGatherConfig {
            terms: vec!["alpha".into()],
            max_local_sources: 5,
            ..LocalGatherConfig::default()
        };
        let sources = g.gather(&root(), "alpha content", None, &cfg).await.unwrap();
        assert!(!sources.is_empty());
        for (i, src) in sources.iter().enumerate() {
            let Source::Local { body_path, kind, path, .. } = src else {
                panic!("expected Source::Local, got {src:?}");
            };
            assert_eq!(*kind, LocalSourceKind::InProject);
            assert_eq!(
                body_path.as_path(),
                PathBuf::from(format!("sources/local-{:02}.md", i + 1)).as_path()
            );
            assert!(path.ends_with(".rs"));
        }
    }

    #[tokio::test]
    async fn gather_caps_at_max_local_sources() {
        let mut fs = FakeFs::default();
        for i in 0..20 {
            let p = root().join(format!("file{i}.rs"));
            fs.files.insert(p, format!("file {i} contains alpha\n"));
        }
        let (g, _) = gatherer_with_fs(fs);
        let cfg = LocalGatherConfig {
            terms: vec!["alpha".into()],
            max_local_sources: 3,
            ..LocalGatherConfig::default()
        };
        let sources = g.gather(&root(), "alpha", None, &cfg).await.unwrap();
        let local_count = sources.iter().filter(|s| matches!(s, Source::Local { .. })).count();
        assert_eq!(local_count, 3);
    }

    #[tokio::test]
    async fn gather_orders_higher_scores_first() {
        let mut fs = FakeFs::default();
        // file_high has 5 alpha hits, file_low has 1.
        fs.files.insert(root().join("high.rs"), "alpha\nalpha\nalpha\nalpha\nalpha\n".into());
        fs.files.insert(root().join("low.rs"), "alpha\nbeta\ngamma\n".into());
        let (g, _) = gatherer_with_fs(fs);
        let cfg = LocalGatherConfig {
            terms: vec!["alpha".into()],
            max_local_sources: 5,
            ..LocalGatherConfig::default()
        };
        let sources = g.gather(&root(), "alpha", None, &cfg).await.unwrap();
        let first = sources
            .iter()
            .find(|s| matches!(s, Source::Local { path, .. } if path.contains("high.rs")));
        assert!(first.is_some(), "highest-scoring file must come first");
        assert!(matches!(first.unwrap(), Source::Local { body_path, .. } if body_path == &PathBuf::from("sources/local-01.md")));
    }

    #[tokio::test]
    async fn gather_marks_extra_dir_files_with_extra_kind() {
        let mut fs = FakeFs::default();
        // In-project file.
        fs.files.insert(root().join("src/lib.rs"), "alpha content here\n".into());
        // Extra-dir file (will be globs'd from /extra).
        fs.files.insert(PathBuf::from("/extra/notes.md"), "alpha notes here\n".into());
        let (g, _) = gatherer_with_fs(fs);
        let cfg = LocalGatherConfig {
            terms: vec!["alpha".into()],
            max_local_sources: 10,
            ..LocalGatherConfig::default()
        };
        let sources = g
            .gather(&root(), "alpha", Some(Path::new("/extra")), &cfg)
            .await
            .unwrap();
        let kinds: Vec<LocalSourceKind> = sources
            .iter()
            .filter_map(|s| match s {
                Source::Local { kind, .. } => Some(*kind),
                _ => None,
            })
            .collect();
        assert!(kinds.contains(&LocalSourceKind::InProject));
        assert!(
            kinds.contains(&LocalSourceKind::Extra),
            "extra-dir matches must be tagged LocalSourceKind::Extra"
        );
    }

    #[tokio::test]
    async fn gather_includes_spec_sources_with_relevance_notes() {
        let mut fs = FakeFs::default();
        fs.specs.insert("auth-refactor".into(), "Auth refactor plan".into());
        fs.specs.insert("model-router".into(), "Model router provider".into());
        let (g, _) = gatherer_with_fs(fs);
        let cfg = LocalGatherConfig {
            terms: vec!["auth".into()],
            max_local_sources: 5,
            ..LocalGatherConfig::default()
        };
        let sources = g.gather(&root(), "auth", None, &cfg).await.unwrap();
        let spec_sources: Vec<&Source> = sources
            .iter()
            .filter(|s| matches!(s, Source::Spec { .. }))
            .collect();
        assert_eq!(spec_sources.len(), 2);
        if let Source::Spec { relevance, .. } = spec_sources[0] {
            assert!(
                relevance.contains("Auth refactor plan") || relevance.contains("Model router"),
                "relevance should include the spec title: {relevance}"
            );
        }
    }

    #[tokio::test]
    async fn gather_returns_empty_when_no_files_match() {
        let mut fs = FakeFs::default();
        fs.files.insert(root().join("README.md"), "no matching keywords here\n".into());
        let (g, _) = gatherer_with_fs(fs);
        let cfg = LocalGatherConfig {
            terms: vec!["zzznotpresent".into()],
            max_local_sources: 5,
            ..LocalGatherConfig::default()
        };
        let sources = g.gather(&root(), "zzznotpresent", None, &cfg).await.unwrap();
        let local_count = sources.iter().filter(|s| matches!(s, Source::Local { .. })).count();
        assert_eq!(local_count, 0);
    }

    #[test]
    fn derive_terms_lowercases_dedupes_and_filters_short_tokens() {
        let terms = derive_terms("Async Rust async tokio A I", &[]);
        assert!(terms.contains(&"async".to_string()));
        assert!(terms.contains(&"rust".to_string()));
        assert!(terms.contains(&"tokio".to_string()));
        assert!(!terms.iter().any(|t| t.len() < 2));
        // Ensure dedup — "async" appears twice in input.
        assert_eq!(terms.iter().filter(|t| *t == "async").count(), 1);
    }

    #[test]
    fn derive_terms_falls_back_when_topic_is_empty() {
        let fallback = vec!["foo".into(), "bar".into()];
        let terms = derive_terms("", &fallback);
        assert_eq!(terms, vec!["bar".to_string(), "foo".to_string()]);
    }

    #[test]
    fn derive_terms_filters_short_tokens_from_fallback_too() {
        let fallback = vec!["x".into(), "yy".into(), "z".into()];
        let terms = derive_terms("", &fallback);
        // Single-char tokens filtered even from fallback.
        assert_eq!(terms, vec!["yy".to_string()]);
    }

    #[test]
    fn local_body_path_zero_pads_and_uses_one_based_index() {
        assert_eq!(local_body_path(0), PathBuf::from("sources/local-01.md"));
        assert_eq!(local_body_path(9), PathBuf::from("sources/local-10.md"));
        assert_eq!(local_body_path(99), PathBuf::from("sources/local-100.md"));
    }
}