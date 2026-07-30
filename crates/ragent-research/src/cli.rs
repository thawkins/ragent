//! CLI helpers for the `ragent research …` sub-commands (T-034, T-035).
//!
//! Provides parse, build-help, and JSON progress-emit helpers used by
//! `src/main.rs` to dispatch the `ragent research <subcommand>` family.

use crate::research_name::ResearchNameError;

/// Parsed `ragent research <subcommand>` arguments.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ResearchCliCommand {
    /// `ragent research help` — show the help table.
    Help,
    /// `ragent research create <name> [topic] [--from-url <URL>] [--from-file <PATH>] [--iterations N] [--depth shallow|standard|deep] [--format report|executive-summary|comparison-table|source-bibliography|imrad] [--sources-dir <path>] [--template <name>] [--fetch-concurrently N] [--use-local] [--use-specs]` — run a gathering session.
    Create {
        /// Validated research name (or raw string if validation hasn't run).
        name: String,
        /// Free-form topic description. Optional when `--from-url` or
        /// `--from-file` is supplied; in that case the fetched/extracted
        /// content becomes the research subject.
        topic: String,
        /// `--from-url <URL>`: fetch the URL and use its content as the research
        /// subject in place of (or alongside) an explicit topic. The fetched
        /// page is captured as the primary web source; the normal web-search
        /// phase still runs using the derived topic.
        from_url: Option<String>,
        /// `--from-file <PATH>`: extract the local document and use its content
        /// as the research subject in place of (or alongside) an explicit
        /// topic. The extracted content is captured as the primary
        /// `Source::Other`; the normal web-search phase still runs using the
        /// derived topic.
        from_file: Option<String>,
        /// Optional FR-010 `--iterations N` override.
        iterations: Option<u32>,
        /// Optional FR-011 `--depth shallow|standard|deep`.
        depth: Option<String>,
        /// Optional FR-012 `--format <artifact>`.
        format: Option<String>,
        /// Optional FR-019 `--sources-dir <path>`.
        sources_dir: Option<String>,
        /// Optional FR-020 `--template <name>`.
        template: Option<String>,
        /// `--fetch-concurrently N` — override the maximum number of candidate
        /// pages fetched in parallel during the web-gathering phase. The
        /// default is `ragent_research::DEFAULT_FETCH_CONCURRENCY` (10); `0`
        /// is clamped up to `1`. Larger values reduce wall-clock latency when
        /// a search returns many hits, at the cost of more in-flight HTTP
        /// connections.
        fetch_concurrency: Option<usize>,
        /// `--use-local` — enable the local-file scanning phase.
        use_local: bool,
        /// `--use-specs` — enable the prior-spec cross-reference phase.
        use_specs: bool,
    },
    /// `ragent research continue <name> [message]` — resume an in-progress item (T-012).
    Continue {
        /// Research name.
        name: String,
        /// Optional follow-up requirement to add to the plan (T-014).
        message: Option<String>,
    },
    /// `ragent research list` — list every item.
    List {
        /// `--all` includes archived items.
        all: bool,
    },
    /// `ragent research open <name>` — print the absolute path of `RESEARCH.md`.
    Open {
        /// Research name.
        name: String,
    },
    /// `ragent research search <query>` — full-text search.
    Search {
        /// Search query string.
        query: String,
    },
    /// `ragent research show <name>` — print metadata.
    Show {
        /// Research name.
        name: String,
    },
    /// `ragent research delete <name>` — remove the item.
    Delete {
        /// Research name.
        name: String,
        /// `--yes` to skip confirmation.
        yes: bool,
    },
    /// `ragent research archive <name>` — mark as archived.
    Archive {
        /// Research name.
        name: String,
    },
    /// Unknown / malformed sub-command — preserves the raw input for an
    /// error message.
    Unknown(String),
}

impl ResearchCliCommand {
    /// Parse `ragent research …` arguments. The first positional argument is
    /// the subcommand; remaining arguments are subcommand-specific.
    #[must_use]
    pub fn parse(args: &str) -> Self {
        let trimmed = args.trim();
        if trimmed.is_empty() {
            return Self::Help;
        }
        let mut parts = trimmed.split_whitespace();
        let sub = parts.next().unwrap_or("");
        let rest: Vec<&str> = parts.collect();
        match sub {
            "help" | "-h" | "--help" => Self::Help,
            "create" => Self::parse_create(&rest),
            "continue" => Self::parse_continue(&rest),
            "list" | "ls" => {
                let all = rest.contains(&"--all");
                Self::List { all }
            }
            "open" => Self::parse_open(&rest),
            "search" => {
                let query = rest.join(" ");
                Self::Search { query }
            }
            "show" => Self::parse_show(&rest),
            "delete" | "rm" => Self::parse_delete(&rest),
            "archive" => Self::Archive {
                name: rest.join(" "),
            },
            other => {
                // Treat as `create <name> <topic…>` if it looks like a name.
                let name = other.to_string();
                let topic = rest.join(" ");
                if topic.is_empty() {
                    Self::Unknown(name)
                } else {
                    Self::Create {
                        name,
                        topic,
                        from_url: None,
                        from_file: None,
                        iterations: None,
                        depth: None,
                        format: None,
                        sources_dir: None,
                        template: None,
                        fetch_concurrency: None,
                        use_local: false,
                        use_specs: false,
                    }
                }
            }
        }
    }

    fn parse_create(rest: &[&str]) -> Self {
        // Parse: ragent research create <name> <topic> [--from-url <URL>]
        //        [--from-file <PATH>] [--iterations N] [--depth shallow|standard|deep]
        //        [--format <artifact>] [--sources-dir <path>] [--template <name>]
        //        [--fetch-concurrently N] [--use-local] [--use-specs]
        let mut i = 0;
        let mut name: Option<String> = None;
        let mut topic_words: Vec<&str> = Vec::new();
        let mut from_url: Option<String> = None;
        let mut from_file: Option<String> = None;
        let mut iterations: Option<u32> = None;
        let mut depth: Option<String> = None;
        let mut format: Option<String> = None;
        let mut sources_dir: Option<String> = None;
        let mut template: Option<String> = None;
        let mut fetch_concurrency: Option<usize> = None;
        let mut use_local = false;
        let mut use_specs = false;
        while i < rest.len() {
            let arg = rest[i];
            match arg {
                "--from-url" => {
                    if let Some(v) = rest.get(i + 1) {
                        from_url = Some((*v).to_string());
                        i += 2;
                    } else {
                        i += 1;
                    }
                }
                "--from-file" => {
                    if let Some(v) = rest.get(i + 1) {
                        from_file = Some((*v).to_string());
                        i += 2;
                    } else {
                        i += 1;
                    }
                }
                "--iterations" => {
                    if let Some(v) = rest.get(i + 1) {
                        iterations = v.parse().ok();
                        i += 2;
                    } else {
                        i += 1;
                    }
                }
                "--depth" => {
                    if let Some(v) = rest.get(i + 1) {
                        depth = Some((*v).to_string());
                        i += 2;
                    } else {
                        i += 1;
                    }
                }
                "--format" => {
                    if let Some(v) = rest.get(i + 1) {
                        format = Some((*v).to_string());
                        i += 2;
                    } else {
                        i += 1;
                    }
                }
                "--sources-dir" => {
                    if let Some(v) = rest.get(i + 1) {
                        sources_dir = Some((*v).to_string());
                        i += 2;
                    } else {
                        i += 1;
                    }
                }
                "--template" => {
                    if let Some(v) = rest.get(i + 1) {
                        template = Some((*v).to_string());
                        i += 2;
                    } else {
                        i += 1;
                    }
                }
                "--fetch-concurrently" => {
                    if let Some(v) = rest.get(i + 1) {
                        fetch_concurrency = v.parse().ok();
                        i += 2;
                    } else {
                        i += 1;
                    }
                }
                "--use-local" => {
                    use_local = true;
                    i += 1;
                }
                "--use-specs" => {
                    use_specs = true;
                    i += 1;
                }
                _ => {
                    if name.is_none() {
                        name = Some(arg.to_string());
                    } else {
                        topic_words.push(arg);
                    }
                    i += 1;
                }
            }
        }
        let Some(name) = name else {
            return Self::Unknown("create".to_string());
        };
        let topic = topic_words.join(" ");
        Self::Create {
            name,
            topic,
            from_url,
            from_file,
            iterations,
            depth,
            format,
            sources_dir,
            template,
            fetch_concurrency,
            use_local,
            use_specs,
        }
    }

    fn parse_continue(rest: &[&str]) -> Self {
        let name = rest
            .first()
            .map(std::string::ToString::to_string)
            .unwrap_or_default();
        if name.is_empty() {
            return Self::Unknown("continue".to_string());
        }
        let message = if rest.len() > 1 {
            Some(rest[1..].join(" "))
        } else {
            None
        };
        Self::Continue { name, message }
    }
    fn parse_open(rest: &[&str]) -> Self {
        let name = rest
            .iter()
            .find(|a| !a.starts_with("--"))
            .map(std::string::ToString::to_string)
            .unwrap_or_default();
        if name.is_empty() {
            Self::Unknown("open".to_string())
        } else {
            Self::Open { name }
        }
    }

    fn parse_show(rest: &[&str]) -> Self {
        let name = rest
            .iter()
            .find(|a| !a.starts_with("--"))
            .map(std::string::ToString::to_string)
            .unwrap_or_default();
        if name.is_empty() {
            Self::Unknown("show".to_string())
        } else {
            Self::Show { name }
        }
    }

    fn parse_delete(rest: &[&str]) -> Self {
        let yes = rest.iter().any(|a| *a == "--yes" || *a == "-y");
        let name = rest
            .iter()
            .find(|a| !a.starts_with("--") && !a.starts_with('-'))
            .map(std::string::ToString::to_string)
            .unwrap_or_default();
        if name.is_empty() {
            Self::Unknown("delete".to_string())
        } else {
            Self::Delete { name, yes }
        }
    }

    /// Build the static help message shown by `ragent research help`.
    #[must_use]
    pub const fn build_help_message() -> &'static str {
        "ragent research — manage research items under research/\n\
               \n\
               USAGE:\n\
                 ragent research <SUBCOMMAND> [ARGS]\n\
               \n\
               SUBCOMMANDS:\n\
                                   create <name> [topic] [--from-url <URL>] [--iterations N] [--depth shallow|standard|deep]\n\
                                         [--format report|executive-summary|comparison-table|source-bibliography|imrad]\n\
                                         [--sources-dir <path>] [--template <name>] [--fetch-concurrently N] [--use-local] [--use-specs]\n\
                                         Run an information-gathering session and write RESEARCH.md.\n\
                                         --from-url            Fetch the URL and use its content as the research subject\n\
                                                               in place of an explicit topic. The page is captured as\n\
                                                               the primary source; web search still runs.\n\
                                         --iterations          Override the default maximum number of iterations.\n\
                                         --depth               Choose a preset: shallow, standard, or deep (default: standard).\n\
                                         --format              Select the output artifact format. Values: report, executive-summary, comparison-table, source-bibliography, imrad (default: report).\n\
                                         --fetch-concurrently  Override the maximum number of candidate pages fetched\n\
                                                               in parallel during the web-gathering phase (default 10).\n\
                                         --use-local           Enable local-file scanning (in-project + extras).\n\
                                         --use-specs           Enable prior-spec cross-referencing.\n\
                   continue <name> [message] Resume an in-progress research item.\n\
                   list [--all]                  List every research item.\n\
                 open <name>                   Print the absolute path of RESEARCH.md.\n\
                 search <query>                Full-text search across all RESEARCH.md.\n\
                 show <name>                   Print metadata for a single item.\n\
                 delete <name> [--yes]         Remove a research item (prompts unless --yes).\n\
                 archive <name>                Mark a research item as archived.\n\
                 help                          Show this message.\n\
               \n\
               Output: by default `ragent research` emits machine-readable JSON lines\n\
               prefixed with `ragent-research:` so callers can pipe the output\n\
               through `jq` or other tools."
    }
    /// `true` if this is a usage-error variant (i.e. parse succeeded but the
    /// caller passed wrong args).
    #[must_use]
    pub const fn is_usage_error(&self) -> bool {
        matches!(self, Self::Unknown(_))
    }
}

/// Render a [`SessionEvent`] as a single machine-readable JSON line on stdout
/// (T-035). The prefix `ragent-research:` makes the line easy to grep in a
/// mixed transcript.
#[must_use]
pub fn render_session_event_json(event: &crate::session::SessionEvent) -> String {
    use crate::session::SessionEvent;
    let (kind, payload) = match event {
        SessionEvent::Phase { phase } => ("phase", serde_json::json!({ "phase": phase.as_str() })),
        SessionEvent::QueriesDecomposed { queries } => {
            ("queries", serde_json::json!({ "queries": queries }))
        }
        SessionEvent::WebCaptured {
            url,
            title,
            search_tool,
            search_engine,
        } => (
            "web",
            serde_json::json!({
                "url": url,
                "title": title,
                "search_tool": search_tool,
                "search_engine": search_engine,
            }),
        ),
        SessionEvent::FromUrlBodyPreview { url, body_preview } => (
            "from_url_body_preview",
            serde_json::json!({ "url": url, "body_preview": body_preview }),
        ),
        SessionEvent::FromFileBodyPreview { path, body_preview } => (
            "from_file_body_preview",
            serde_json::json!({ "path": path, "body_preview": body_preview }),
        ),
        SessionEvent::LocalCaptured { path, score } => {
            ("local", serde_json::json!({ "path": path, "score": score }))
        }
        SessionEvent::SpecCaptured { spec_id } => {
            ("spec", serde_json::json!({ "spec_id": spec_id }))
        }
        SessionEvent::WebSearchFailed { error } => {
            ("web_error", serde_json::json!({ "error": error }))
        }
        SessionEvent::WebFetchFailed { url, error } => (
            "web_fetch_error",
            serde_json::json!({ "url": url, "error": error }),
        ),
        SessionEvent::PlanUpdated { sub_questions } => (
            "plan_updated",
            serde_json::json!({ "sub_questions": sub_questions }),
        ),
        SessionEvent::SubQuestionStatusChanged { id, status } => (
            "sub_question_status_changed",
            serde_json::json!({ "id": id, "status": status }),
        ),
        SessionEvent::SourceFailed { source, error } => (
            "source_failed",
            serde_json::json!({
                "source": source,
                "error": error,
            }),
        ),
        SessionEvent::CriticResult { score, gaps } => (
            "critic",
            serde_json::json!({
                "score": score,
                "gaps": gaps,
            }),
        ),
        SessionEvent::VerificationResult { passed, issues } => (
            "verification",
            serde_json::json!({
                "passed": passed,
                "issues": issues,
            }),
        ),
        SessionEvent::IterationCompleted { iteration, score } => (
            "iteration_completed",
            serde_json::json!({
                "iteration": iteration,
                "score": score,
            }),
        ),
        SessionEvent::FollowUpQueries { queries } => (
            "follow_up_queries",
            serde_json::json!({ "queries": queries }),
        ),
        SessionEvent::SynthesizeResult { outcome, detail } => (
            "synthesize",
            serde_json::json!({
                "outcome": outcome.as_str(),
                "detail": detail,
            }),
        ),
        SessionEvent::Done {
            total_sources,
            pdf_count,
            youtube_count,
            excluded_count,
        } => (
            "done",
            serde_json::json!({
                "total_sources": total_sources,
                "pdf_count": pdf_count,
                "youtube_count": youtube_count,
                "excluded_count": excluded_count,
            }),
        ),
        SessionEvent::ConfigSnapshot {
            output_format,
            depth,
            iterations,
            from_url,
            from_file,
        } => (
            "config",
            serde_json::json!({
                "output_format": output_format,
                "depth": depth,
                "iterations": iterations,
                "from_url": from_url,
                "from_file": from_file,
            }),
        ),
    };
    format!(
        "ragent-research: {}",
        serde_json::to_string(&serde_json::json!({
            "kind": kind,
            "payload": payload,
        }))
        .unwrap_or_else(|_| "{}".to_string())
    )
}

/// Pretty-print a research item (T-034: `ragent research show`).
#[must_use]
pub fn render_show_output(
    name: &str,
    title: &str,
    topic: &str,
    status: &str,
    created: &str,
    modified: &str,
    sources: &[(String, String, String, String)],
) -> String {
    let mut out = String::new();
    out.push_str(&format!("Research item: {name}\n"));
    out.push_str(&format!("Title:         {title}\n"));
    out.push_str(&format!("Topic:         {topic}\n"));
    out.push_str(&format!("Status:        {status}\n"));
    out.push_str(&format!("Created (UTC): {created}\n"));
    out.push_str(&format!("Modified (UTC):{modified}\n"));
    out.push_str(&format!("\nReferences ({}):\n", sources.len()));
    for (i, (kind, path, title, captured)) in sources.iter().enumerate() {
        out.push_str(&format!(
            "  #{i:>2}  [{kind:<11}] {path:<32}  {title}  ({captured})\n",
        ));
    }
    out
}

/// Pretty-print a research list (T-034: `ragent research list`).
#[must_use]
pub fn render_list_output(rows: &[(String, String, String, String, String)]) -> String {
    let mut out = String::new();
    out.push_str(&format!(
        "{:<22} {:<32} {:<11} {:<24} {:<24}\n",
        "NAME", "TITLE", "STATUS", "CREATED", "MODIFIED"
    ));
    out.push_str(&"-".repeat(120));
    out.push('\n');
    for (name, title, status, created, modified) in rows {
        out.push_str(&format!(
            "{:<22} {:<32} {:<11} {:<24} {:<24}\n",
            truncate(name, 22),
            truncate(title, 32),
            truncate(status, 11),
            truncate(created, 24),
            truncate(modified, 24),
        ));
    }
    out
}

fn truncate(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        s.to_string()
    } else {
        let mut t: String = s.chars().take(max.saturating_sub(1)).collect();
        t.push('…');
        t
    }
}

/// Pretty-print a search result list (T-030).
#[must_use]
pub fn render_search_output(hits: &[(String, String, String)]) -> String {
    let mut out = String::new();
    for (name, title, snippet) in hits {
        out.push_str(&format!("• {name} — {title}\n    {snippet}\n"));
    }
    if out.is_empty() {
        out.push_str("(no matches)\n");
    }
    out
}

/// Convenience: map a [`ResearchNameError`] to a user-facing message.
#[must_use]
pub fn explain_name_error(err: &ResearchNameError) -> String {
    err.to_string()
}

// ── Filesystem-backed LocalTool for the CLI ───────────────────────────────

use crate::local_gatherer::{GrepMatch, LocalTool};
use crate::research_name::ResearchName;
use async_trait::async_trait;
use std::path::{Path, PathBuf};
use std::sync::Arc;

/// Filesystem-backed implementation of [`LocalTool`] for use by the
/// `ragent research` CLI when no agent tool registry is available (T-034).
///
/// Behaviour:
/// - [`FsLocalTool::glob`] runs a synchronous glob over `project_root` and
///   returns project-relative paths. The implementation is intentionally
///   simple — it does the standard `walkdir`-style recursion itself so the
///   CLI doesn't need to depend on the `glob` crate.
/// - [`FsLocalTool::grep`] is a line-by-line case-insensitive substring
///   match over any of `terms`.
/// - [`FsLocalTool::read`] reads the file as UTF-8.
/// - [`FsLocalTool::list_specs`] lists the directories directly under
///   `specs/` and returns their base names.
/// - [`FsLocalTool::spec_title`] reads the first `#` heading of
///   `specs/<id>/SPEC.md` if present.
pub struct FsLocalTool;

impl FsLocalTool {
    /// Build a new filesystem-backed local tool.
    #[must_use]
    pub fn new() -> Arc<Self> {
        Arc::new(Self)
    }
}

impl Default for FsLocalTool {
    fn default() -> Self {
        Self
    }
}

#[async_trait]
impl LocalTool for FsLocalTool {
    async fn glob(&self, project_root: &Path, pattern: &str) -> anyhow::Result<Vec<PathBuf>> {
        // Translate a `**/*.ext` style pattern into a walkdir-style scan.
        // We only support two pattern shapes:
        //   1. `**/*.<ext>`     → match every file with that extension
        //   2. `*.<ext>`        → match every file with that extension (1-deep)
        // Anything else returns an empty vec — the gatherer treats that as
        // "no candidates" and moves on.
        let ext = pattern.rsplit('.').next().unwrap_or("");
        let ext = if ext == pattern { "" } else { ext };
        if ext.is_empty() {
            return Ok(Vec::new());
        }
        let mut out: Vec<PathBuf> = Vec::new();
        walk(project_root, project_root, ext, &mut out).await?;
        out.sort();
        Ok(out)
    }

    async fn grep(&self, path: &Path, terms: &[String]) -> anyhow::Result<Vec<GrepMatch>> {
        let body = match tokio::fs::read_to_string(path).await {
            Ok(b) => b,
            Err(_) => return Ok(Vec::new()),
        };
        let mut out = Vec::new();
        for (i, line) in body.lines().enumerate() {
            let lower = line.to_lowercase();
            if terms.iter().any(|t| lower.contains(&t.to_lowercase())) {
                out.push(GrepMatch {
                    line: i + 1,
                    text: line.to_string(),
                });
            }
        }
        Ok(out)
    }

    async fn read(&self, path: &Path) -> anyhow::Result<String> {
        Ok(tokio::fs::read_to_string(path).await?)
    }

    async fn list_specs(&self, project_root: &Path) -> anyhow::Result<Vec<String>> {
        let specs_dir = project_root.join("specs");
        let mut entries = match tokio::fs::read_dir(&specs_dir).await {
            Ok(d) => d,
            Err(_) => return Ok(Vec::new()),
        };
        let mut out = Vec::new();
        while let Some(entry) = entries.next_entry().await? {
            let path = entry.path();
            if !path.is_dir() {
                continue;
            }
            let name = entry.file_name().to_string_lossy().to_string();
            if name.starts_with('_') || name.starts_with('.') {
                continue;
            }
            if ResearchName::try_new(&name).is_ok() {
                out.push(name);
            }
        }
        out.sort();
        Ok(out)
    }

    async fn spec_title(&self, project_root: &Path, spec_id: &str) -> anyhow::Result<String> {
        let path = project_root.join("specs").join(spec_id).join("SPEC.md");
        let body = match tokio::fs::read_to_string(&path).await {
            Ok(b) => b,
            Err(_) => return Ok(String::new()),
        };
        for raw in body.lines() {
            let line = raw.trim();
            if let Some(rest) = line.strip_prefix("# ") {
                return Ok(rest.trim().to_string());
            }
            if let Some(rest) = line.strip_prefix("## ") {
                return Ok(rest.trim().to_string());
            }
        }
        Ok(String::new())
    }
}

async fn walk(root: &Path, dir: &Path, ext: &str, out: &mut Vec<PathBuf>) -> anyhow::Result<()> {
    let mut entries = match tokio::fs::read_dir(dir).await {
        Ok(d) => d,
        Err(_) => return Ok(()),
    };
    while let Some(entry) = entries.next_entry().await? {
        let path = entry.path();
        let file_type = match entry.file_type().await {
            Ok(ft) => ft,
            Err(_) => continue,
        };
        if file_type.is_dir() {
            // Skip the research/ output directory and obvious junk so the
            // gatherer doesn't index its own previous outputs.
            let name = entry.file_name().to_string_lossy().to_string();
            if name == "research"
                || name == "target"
                || name == ".git"
                || name == "node_modules"
                || name.starts_with('.')
            {
                continue;
            }
            Box::pin(walk(root, &path, ext, out)).await?;
        } else if file_type.is_file() {
            let matches_ext = entry
                .file_name()
                .to_string_lossy()
                .rsplit('.')
                .next()
                .is_some_and(|e| e == ext);
            if matches_ext && let Ok(rel) = path.strip_prefix(root) {
                out.push(rel.to_path_buf());
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_help() {
        assert_eq!(ResearchCliCommand::parse("help"), ResearchCliCommand::Help);
        assert_eq!(ResearchCliCommand::parse(""), ResearchCliCommand::Help);
    }

    #[test]
    fn parse_create_with_topic() {
        let cmd = ResearchCliCommand::parse("create rust-async async/await idioms in stable Rust");
        match cmd {
            ResearchCliCommand::Create {
                name,
                topic,
                sources_dir,
                template,
                use_local,
                use_specs,
                ..
            } => {
                assert_eq!(name, "rust-async");
                assert_eq!(topic, "async/await idioms in stable Rust");
                assert!(sources_dir.is_none());
                assert!(template.is_none());
                assert!(!use_local);
                assert!(!use_specs);
            }
            other => panic!("unexpected variant: {other:?}"),
        }
    }

    #[test]
    fn parse_create_with_sources_dir_and_template() {
        let cmd = ResearchCliCommand::parse(
            "create foo topic words --sources-dir /tmp/notes --template deepdive",
        );
        match cmd {
            ResearchCliCommand::Create {
                name,
                topic,
                sources_dir,
                template,
                use_local,
                use_specs,
                ..
            } => {
                assert_eq!(name, "foo");
                assert_eq!(topic, "topic words");
                assert_eq!(sources_dir.as_deref(), Some("/tmp/notes"));
                assert_eq!(template.as_deref(), Some("deepdive"));
                assert!(!use_local);
                assert!(!use_specs);
            }
            other => panic!("unexpected variant: {other:?}"),
        }
    }

    #[test]
    fn parse_create_with_use_local_flag() {
        let cmd = ResearchCliCommand::parse("create foo a topic --use-local");
        match cmd {
            ResearchCliCommand::Create {
                name,
                topic,
                use_local,
                use_specs,
                ..
            } => {
                assert_eq!(name, "foo");
                assert_eq!(topic, "a topic");
                assert!(use_local);
                assert!(!use_specs);
            }
            other => panic!("unexpected variant: {other:?}"),
        }
    }

    #[test]
    fn parse_create_with_use_specs_flag() {
        let cmd = ResearchCliCommand::parse("create foo a topic --use-specs");
        match cmd {
            ResearchCliCommand::Create {
                name,
                topic,
                use_local,
                use_specs,
                ..
            } => {
                assert_eq!(name, "foo");
                assert_eq!(topic, "a topic");
                assert!(!use_local);
                assert!(use_specs);
            }
            other => panic!("unexpected variant: {other:?}"),
        }
    }

    #[test]
    fn parse_create_with_both_no_flags() {
        let cmd = ResearchCliCommand::parse(
            "create foo a topic --use-local --use-specs --sources-dir /tmp/x",
        );
        match cmd {
            ResearchCliCommand::Create {
                name,
                topic,
                sources_dir,
                use_local,
                use_specs,
                ..
            } => {
                assert_eq!(name, "foo");
                assert_eq!(topic, "a topic");
                assert_eq!(sources_dir.as_deref(), Some("/tmp/x"));
                assert!(use_local);
                assert!(use_specs);
            }
            other => panic!("unexpected variant: {other:?}"),
        }
    }
    #[test]
    fn parse_list_all() {
        let cmd = ResearchCliCommand::parse("list --all");
        assert!(matches!(cmd, ResearchCliCommand::List { all: true }));
    }

    #[test]
    fn parse_list_default() {
        let cmd = ResearchCliCommand::parse("list");
        assert!(matches!(cmd, ResearchCliCommand::List { all: false }));
    }

    #[test]
    fn parse_open() {
        let cmd = ResearchCliCommand::parse("open rust-async");
        assert!(matches!(cmd, ResearchCliCommand::Open { ref name } if name == "rust-async"));
    }

    #[test]
    fn parse_search() {
        let cmd = ResearchCliCommand::parse("search async patterns");
        assert!(
            matches!(cmd, ResearchCliCommand::Search { ref query } if query == "async patterns")
        );
    }

    #[test]
    fn parse_show() {
        let cmd = ResearchCliCommand::parse("show foo");
        assert!(matches!(cmd, ResearchCliCommand::Show { ref name } if name == "foo"));
    }

    #[test]
    fn parse_delete_with_yes() {
        let cmd = ResearchCliCommand::parse("delete foo --yes");
        assert!(matches!(cmd, ResearchCliCommand::Delete { ref name, yes: true } if name == "foo"));
    }

    #[test]
    fn parse_create_with_iterations_depth_format() {
        let cmd = ResearchCliCommand::parse(
            "create foo topic words --iterations 5 --depth deep --format executive-summary",
        );
        match cmd {
            ResearchCliCommand::Create {
                name,
                topic,
                iterations,
                depth,
                format,
                ..
            } => {
                assert_eq!(name, "foo");
                assert_eq!(topic, "topic words");
                assert_eq!(iterations, Some(5));
                assert_eq!(depth.as_deref(), Some("deep"));
                assert_eq!(format.as_deref(), Some("executive-summary"));
            }
            other => panic!("unexpected variant: {other:?}"),
        }
    }

    #[test]
    fn parse_create_with_imrad_format() {
        let cmd = ResearchCliCommand::parse("create foo topic words --format imrad");
        match cmd {
            ResearchCliCommand::Create {
                name,
                topic,
                format,
                ..
            } => {
                assert_eq!(name, "foo");
                assert_eq!(topic, "topic words");
                assert_eq!(format.as_deref(), Some("imrad"));
            }
            other => panic!("unexpected variant: {other:?}"),
        }
    }

    #[test]
    fn parse_create_with_fetch_concurrently() {
        let cmd = ResearchCliCommand::parse("create foo topic words --fetch-concurrently 20");
        match cmd {
            ResearchCliCommand::Create {
                name,
                topic,
                fetch_concurrency,
                ..
            } => {
                assert_eq!(name, "foo");
                assert_eq!(topic, "topic words");
                assert_eq!(fetch_concurrency, Some(20));
            }
            other => panic!("unexpected variant: {other:?}"),
        }
    }

    #[test]
    fn parse_create_without_fetch_concurrently_defaults_to_none() {
        let cmd = ResearchCliCommand::parse("create foo topic words");
        match cmd {
            ResearchCliCommand::Create {
                fetch_concurrency, ..
            } => {
                assert_eq!(fetch_concurrency, None);
            }
            other => panic!("unexpected variant: {other:?}"),
        }
    }

    #[test]
    fn parse_create_ignores_non_numeric_fetch_concurrently() {
        // A non-numeric value leaves fetch_concurrency as None and is
        // consumed as the flag's argument (not pushed into the topic).
        let cmd = ResearchCliCommand::parse("create foo topic words --fetch-concurrently abc");
        match cmd {
            ResearchCliCommand::Create {
                name,
                topic,
                fetch_concurrency,
                ..
            } => {
                assert_eq!(name, "foo");
                assert_eq!(topic, "topic words");
                assert_eq!(fetch_concurrency, None);
            }
            other => panic!("unexpected variant: {other:?}"),
        }
    }

    #[test]
    fn parse_continue_with_message() {
        let cmd = ResearchCliCommand::parse("continue rust-async focus on async-std");
        match cmd {
            ResearchCliCommand::Continue { name, message } => {
                assert_eq!(name, "rust-async");
                assert_eq!(message.as_deref(), Some("focus on async-std"));
            }
            other => panic!("unexpected variant: {other:?}"),
        }
    }
    #[test]
    fn parse_continue_without_message() {
        let cmd = ResearchCliCommand::parse("continue foo");
        assert!(
            matches!(cmd, ResearchCliCommand::Continue { ref name, message: None } if name == "foo")
        );
    }

    #[test]
    fn parse_archive() {
        let cmd = ResearchCliCommand::parse("archive foo");
        assert!(matches!(cmd, ResearchCliCommand::Archive { ref name } if name == "foo"));
    }

    #[test]
    fn parse_implicit_create_when_unknown_subcommand() {
        let cmd = ResearchCliCommand::parse("foo bar baz");
        match cmd {
            ResearchCliCommand::Create {
                name,
                topic,
                use_local,
                use_specs,
                ..
            } => {
                assert_eq!(name, "foo");
                assert_eq!(topic, "bar baz");
                assert!(!use_local);
                assert!(!use_specs);
            }
            other => panic!("unexpected variant: {other:?}"),
        }
    }
    #[test]
    fn parse_create_with_from_url_and_no_topic() {
        let cmd = ResearchCliCommand::parse("create myitem --from-url https://example.com/article");
        match cmd {
            ResearchCliCommand::Create {
                name,
                topic,
                from_url,
                ..
            } => {
                assert_eq!(name, "myitem");
                assert_eq!(topic, "");
                assert_eq!(from_url.as_deref(), Some("https://example.com/article"));
            }
            other => panic!("unexpected variant: {other:?}"),
        }
    }
    #[test]
    fn parse_create_with_from_url_and_topic() {
        let cmd = ResearchCliCommand::parse(
            "create myitem rust async --from-url https://example.com/article",
        );
        match cmd {
            ResearchCliCommand::Create {
                name,
                topic,
                from_url,
                ..
            } => {
                assert_eq!(name, "myitem");
                assert_eq!(topic, "rust async");
                assert_eq!(from_url.as_deref(), Some("https://example.com/article"));
            }
            other => panic!("unexpected variant: {other:?}"),
        }
    }
    #[test]
    fn parse_create_with_from_url_before_other_flags() {
        let cmd = ResearchCliCommand::parse(
            "create myitem --from-url https://example.com --use-local --iterations 3",
        );
        match cmd {
            ResearchCliCommand::Create {
                name,
                topic,
                from_url,
                use_local,
                iterations,
                ..
            } => {
                assert_eq!(name, "myitem");
                assert_eq!(topic, "");
                assert_eq!(from_url.as_deref(), Some("https://example.com"));
                assert!(use_local);
                assert_eq!(iterations, Some(3));
            }
            other => panic!("unexpected variant: {other:?}"),
        }
    }
    #[test]
    fn help_message_documents_from_url_flag() {
        let h = ResearchCliCommand::build_help_message();
        assert!(h.contains("--from-url"), "help missing `--from-url`: {h}");
    }
    #[test]
    fn help_message_contains_documented_subcommands() {
        let h = ResearchCliCommand::build_help_message();
        for sub in [
            "create", "continue", "list", "open", "search", "show", "delete", "archive",
        ] {
            assert!(h.contains(sub), "help missing `{sub}`");
        }
    }

    #[test]
    fn help_message_lists_imrad_format_option() {
        let h = ResearchCliCommand::build_help_message();
        assert!(
            h.contains("imrad"),
            "help missing `imrad` format option: {h}"
        );
    }

    #[test]
    fn render_session_event_json_for_phase() {
        let event = crate::session::SessionEvent::Phase {
            phase: crate::session::SessionPhase::Web,
        };
        let line = render_session_event_json(&event);
        assert!(line.starts_with("ragent-research:"));
        assert!(line.contains("\"web\""));
    }

    #[test]
    fn render_session_event_json_for_plan_updated() {
        let event = crate::session::SessionEvent::PlanUpdated {
            sub_questions: vec!["q1".into(), "q2".into()],
        };
        let line = render_session_event_json(&event);
        assert!(line.contains("\"plan_updated\""));
        assert!(line.contains("\"q1\""));
    }

    #[test]
    fn render_session_event_json_for_source_failed() {
        let event = crate::session::SessionEvent::SourceFailed {
            source: Some("https://example.com".into()),
            error: "timeout".into(),
        };
        let line = render_session_event_json(&event);
        assert!(line.contains("\"source_failed\""));
        assert!(line.contains("\"timeout\""));
    }

    #[test]
    fn render_session_event_json_for_critic_and_verification() {
        let event = crate::session::SessionEvent::CriticResult {
            score: Some(72),
            gaps: vec!["missing citation".into()],
        };
        let line = render_session_event_json(&event);
        assert!(line.contains("\"critic\""));
        assert!(line.contains("72"));

        let event = crate::session::SessionEvent::VerificationResult {
            passed: false,
            issues: vec!["claim X unsupported".into()],
        };
        let line = render_session_event_json(&event);
        assert!(line.contains("\"verification\""));
        assert!(line.contains("false"));
    }

    #[test]
    fn render_show_output_includes_metadata() {
        let out = render_show_output(
            "rust-async",
            "Rust Async",
            "async/await idioms",
            "complete",
            "2024-01-15T10:30:00Z",
            "2024-01-15T10:31:00Z",
            &[(
                "web".into(),
                "https://example.com".into(),
                "Example".into(),
                "2024-01-15T10:31:00Z".into(),
            )],
        );
        assert!(out.contains("rust-async"));
        assert!(out.contains("Rust Async"));
        assert!(out.contains("https://example.com"));
    }

    #[test]
    fn render_list_output_handles_empty() {
        let out = render_list_output(&[]);
        assert!(out.contains("NAME"));
    }

    #[test]
    fn truncate_short_string_passes_through() {
        assert_eq!(truncate("abc", 5), "abc");
    }

    #[test]
    fn truncate_long_string_ellipsises() {
        let s = "a".repeat(20);
        let t = truncate(&s, 5);
        assert_eq!(t.chars().count(), 5);
        assert!(t.ends_with('…'));
    }
}
