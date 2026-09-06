//! Research sub-command CLI parser and rendering helpers.
//!
//! This module implements the `ragent research` command surface:
//!
//! - `ResearchCliCommand` — a lightweight clap-free parser that maps the
//!   `ragent research <verb> [args]` shell syntax into a structured command.
//! - `render_*` helpers that turn research items, search results, and session
//!   events into the JSON/terminal formats consumed by the TUI and scripts.
//!
//! The parser is intentionally hand-written rather than using `clap` directly
//! so the same command grammar can be reused by the TUI slash command parser
//! without pulling in heavy CLI machinery.

use std::collections::HashMap;

use crate::run_config::{Depth, OutputFormat, ResearchMode, Tier};

/// Parsed `ragent research` sub-command.
///
/// Each variant corresponds to one user-facing verb. Fields use plain
/// `Option<String>` and `Option<u32>` rather than strongly-typed enums so
/// invalid values can be reported with the exact flag that produced them.
#[derive(Debug, Clone, PartialEq, Eq)]
#[allow(clippy::large_enum_variant)]
pub enum ResearchCliCommand {
    /// `ragent research create <name> [topic] [flags]`
    Create {
        /// URL-safe research item name (required).
        name: String,
        /// Free-form topic derived from trailing positional arguments.
        topic: String,
        /// `--from-url <URL>` seed pages.
        from_urls: Vec<String>,
        /// `--from-file <PATH>` seed documents.
        from_files: Vec<String>,
        /// Optional FR-010 `--iterations N` override.
        iterations: Option<u32>,
        /// Optional FR-011 `--depth shallow|standard|deep`.
        depth: Option<String>,
        /// Optional FR-001 `--tier light|full|dissertation`.
        tier: Option<String>,
        /// Optional FR-001 `--mode tiered|supervisor|competitive` (specs/opendeepresearch).
        mode: Option<String>,
        /// `--summarization-model <provider:model>` — lightweight model used
        /// to summarize each fetched webpage before synthesis and vault storage
        /// (FR-002, FR-010). When omitted the configured default model is used.
        summarization_model: Option<String>,
        /// `--research-model <provider:model>` — model used by research agents /
        /// sub-topic workers (FR-013). When omitted the configured default model
        /// is used.
        research_model: Option<String>,
        /// `--compression-model <provider:model>` — model used to compress or
        /// summarize intermediate findings (FR-013). When omitted the configured
        /// default model is used.
        compression_model: Option<String>,
        /// `--final-report-model <provider:model>` — model used to write the final
        /// report (FR-013). When omitted the configured default model is used.
        final_report_model: Option<String>,
        /// `--max-concurrent-research-units N` — maximum parallel researcher agents
        /// in supervisor/competitive modes (FR-007, FR-012). When omitted the
        /// configured default or the crate-level default is used.
        max_concurrent_research_units: Option<usize>,
        /// `--clarify` / `--no-clarify` — ask a single clarifying question
        /// before web searches when the topic is ambiguous (FR-005, FR-017).
        /// Defaults to enabled; use `--no-clarify` to disable.
        clarify: Option<bool>,
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
        /// `--local-concurrently N` — override the maximum number of local
        /// candidate scoring/spec-scan tasks that run in parallel. The
        /// default is `ragent_research::DEFAULT_LOCAL_CONCURRENCY` (8); `0`
        /// is clamped up to `1`. Larger values reduce wall-clock latency on
        /// large projects at the cost of more in-flight file handles.
        local_concurrency: Option<usize>,
        /// `--use-local` — enable the local-file scanning phase.
        use_local: bool,
        /// `--use-specs` — enable the prior-spec cross-reference phase.
        use_specs: bool,
        /// `--use-low-relevance` — keep web sources that would otherwise be
        /// filtered out as low-relevance. By default the web-gathering phase
        /// discards sources whose query-match ratio falls below the "Low"
        /// threshold; this flag disables that filter so every fetched page is
        /// retained regardless of relevance score.
        use_low_relevance: bool,
        /// `--no-papers` — disable scholarly search engines (e.g. OpenAlex)
        /// during the web-gathering phase. When set, hits from scholarly
        /// backends are filtered out so only general web search results are
        /// captured.
        no_papers: bool,
        /// `--use-pdf` — allow PDF documents returned by web search or
        /// `--from-url` to be captured as sources. By default PDF web sources
        /// are skipped because they require extra extraction time and are often
        /// paywalled or large.
        use_pdf: bool,
        /// `--fetch-timeout-secs N` — override the per-page fetch timeout.
        /// Pages that take longer than this are treated as a fetch failure so
        /// one slow URL cannot stall the whole gather pass. The default is
        /// 30 seconds.
        fetch_timeout_secs: Option<u64>,
        /// `--web-phase-timeout-secs N` / `--web-time N` — optional
        /// wall-clock timeout for the entire web-gathering phase (Milestone
        /// H-001). When the deadline passes, everything gathered so far is
        /// ingested and the run proceeds to analysis/synthesis. `0` disables
        /// the timeout.
        web_phase_timeout_secs: Option<u64>,
        /// `--local-phase-timeout-secs N` — optional wall-clock timeout for the
        /// entire local-gathering phase (Milestone H-001). When set, the phase
        /// is aborted if it exceeds `N` seconds and a diagnostic is emitted so
        /// a slow filesystem scan cannot stall the session. When `None`, no
        /// phase-level timeout is applied.
        local_phase_timeout_secs: Option<u64>,
        /// `--search-max-retries N` — maximum retry attempts for a failed
        /// sub-query search (Milestone H-002). Defaults to 2. `0` disables
        /// retries.
        search_max_retries: Option<u32>,
        /// `--search-retry-base-delay-ms N` — base delay in milliseconds for
        /// the first search-retry backoff (Milestone H-002). Subsequent
        /// retries double this value. Defaults to 200 ms.
        search_retry_base_delay_ms: Option<u64>,
        /// `--search-circuit-breaker-threshold N` — number of consecutive
        /// failed sub-query searches before the circuit breaker opens and
        /// further searches for the current run are skipped (Milestone H-003).
        /// Defaults to 3. `0` disables the breaker.
        search_circuit_breaker_threshold: Option<u32>,
        /// `--max-web-results N` — override the maximum number of web sources
        /// to capture.
        max_web_results: Option<usize>,
        /// `--max-search-calls N` — hard cap on the total number of web-search
        /// calls the run may issue, shared across all supervisor/competitive
        /// researchers and gather passes. When `None`, no cap is applied.
        max_search_calls: Option<usize>,
        /// `--max-local-sources N` — override the maximum number of in-project
        /// local sources to capture.
        max_local_sources: Option<usize>,
        /// `--max-synthesis-sources N` — override the maximum number of sources
        /// sent to the LLM synthesis engine.
        max_synthesis_sources: Option<usize>,
        /// `--brief <text>` — explicit research brief (FR-004 brief context).
        brief: Option<String>,
        /// `--evaluate` — run the deterministic self-evaluation scorecard and
        /// append it to the assembled report (FR-008 / T-015).
        evaluate: bool,
    },
    /// `ragent research open <name>`
    Open {
        /// URL-safe research item name.
        name: String,
    },
    /// `ragent research list [--all] [--json]`
    List {
        /// `--all` — include archived items.
        all: bool,
        /// `--json` — emit JSON instead of a human-readable table.
        json: bool,
    },
    /// `ragent research show <name> [--json]`
    Show {
        /// URL-safe research item name.
        name: String,
        /// `--json` — emit JSON instead of a human-readable summary.
        json: bool,
    },
    /// `ragent research search <query> [--json]`
    Search {
        /// Free-form search query.
        query: String,
        /// `--json` — emit JSON instead of a human-readable table.
        json: bool,
    },
    /// `ragent research delete <name> [--yes]`
    Delete {
        /// URL-safe research item name.
        name: String,
        /// `--yes` — skip the confirmation prompt.
        yes: bool,
    },
    /// `ragent research archive <name>`
    Archive {
        /// URL-safe research item name.
        name: String,
    },
    /// `ragent research resume <name>`
    Resume {
        /// URL-safe research item name.
        name: String,
    },
    /// `ragent research continue <name> [--message <text>]`
    Continue {
        /// URL-safe research item name.
        name: String,
        /// Optional follow-up message for the resumed session.
        message: Option<String>,
    },
    /// `ragent research update <name>` — replay the invocation recorded in
    /// the item's frontmatter and overwrite `RESEARCH.md`.
    Update {
        /// URL-safe research item name.
        name: String,
    },
    /// `ragent research cluster <name> [--force]`
    Cluster {
        /// URL-safe research item name.
        name: String,
        /// `--force` — regenerate cluster files even if they already exist.
        force: bool,
    },
    /// `ragent research export <name> [--output <path>]`
    Export {
        /// URL-safe research item name.
        name: String,
        /// `--output <path>` destination file/directory.
        output: Option<String>,
    },
    /// `ragent research import <path> [--name <name>]`
    Import {
        /// Source file or directory to import.
        path: String,
        /// Optional override for the item name.
        name: Option<String>,
    },
    /// `ragent research config` — show effective research defaults.
    Config,
    /// `ragent research help`
    Help,
    /// Unknown/unsupported sub-command.
    Unknown(String),
}

impl ResearchCliCommand {
    /// Parse a full `ragent research ...` command string.
    ///
    /// Splits on whitespace, respecting double-quoted arguments, then
    /// dispatches to the verb-specific parser.
    #[must_use]
    pub fn parse(input: &str) -> Self {
        let tokens = split_args(input);
        let args: Vec<&str> = tokens.iter().map(String::as_str).collect();
        if args.is_empty() {
            return Self::Unknown(String::new());
        }
        match args[0] {
            "create" => Self::parse_create(&args[1..]),
            "list" => Self::parse_list(&args[1..]),
            "show" => Self::parse_show(&args[1..]),
            "search" => Self::parse_search(&args[1..]),
            "open" => Self::parse_open(&args[1..]),
            "delete" => Self::parse_delete(&args[1..]),
            "archive" => Self::parse_archive(&args[1..]),
            "resume" => Self::parse_resume(&args[1..]),
            "continue" => Self::parse_continue(&args[1..]),
            "update" => Self::parse_update(&args[1..]),
            "cluster" => Self::parse_cluster(&args[1..]),
            "export" => Self::parse_export(&args[1..]),
            "import" => Self::parse_import(&args[1..]),
            "config" => Self::Config,
            "help" | "--help" | "-h" => Self::Help,
            other => Self::Unknown(other.to_string()),
        }
    }

    fn parse_create(rest: &[&str]) -> Self {
        let mut i = 0;
        let mut name: Option<String> = None;
        let mut topic_words: Vec<&str> = Vec::new();
        let mut from_urls: Vec<String> = Vec::new();
        let mut from_files: Vec<String> = Vec::new();
        let mut iterations: Option<u32> = None;
        let mut depth: Option<String> = None;
        let mut tier: Option<String> = None;
        let mut mode: Option<String> = None;
        let mut summarization_model: Option<String> = None;
        let mut research_model: Option<String> = None;
        let mut compression_model: Option<String> = None;
        let mut final_report_model: Option<String> = None;
        let mut max_concurrent_research_units: Option<usize> = None;
        let mut clarify: Option<bool> = None;
        let mut format: Option<String> = None;
        let mut sources_dir: Option<String> = None;
        let mut template: Option<String> = None;
        let mut fetch_concurrency: Option<usize> = None;
        let mut fetch_timeout_secs: Option<u64> = None;
        let mut local_concurrency: Option<usize> = None;
        let mut web_phase_timeout_secs: Option<u64> = None;
        let mut local_phase_timeout_secs: Option<u64> = None;
        let mut search_max_retries: Option<u32> = None;
        let mut search_retry_base_delay_ms: Option<u64> = None;
        let mut search_circuit_breaker_threshold: Option<u32> = None;
        let mut use_local = false;
        let mut use_specs = false;
        let mut use_low_relevance = false;
        let mut no_papers = false;
        let mut use_pdf = false;
        let mut max_web_results: Option<usize> = None;
        let mut max_search_calls: Option<usize> = None;
        let mut max_local_sources: Option<usize> = None;
        let mut max_synthesis_sources: Option<usize> = None;
        let mut brief: Option<String> = None;
        let mut evaluate = false;

        while i < rest.len() {
            let arg = rest[i];
            match arg {
                "--from-url"
                | "--from-urls"
                | "--from-file"
                | "--from-files"
                | "--iterations"
                | "--depth"
                | "--tier"
                | "--mode"
                | "--summarization-model"
                | "--research-model"
                | "--compression-model"
                | "--final-report-model"
                | "--max-concurrent-research-units"
                | "--format"
                | "--sources-dir"
                | "--template"
                | "--fetch-concurrently"
                | "--local-concurrently"
                | "--fetch-timeout-secs"
                | "--web-phase-timeout-secs"
                | "--web-time"
                | "--local-phase-timeout-secs"
                | "--search-max-retries"
                | "--search-retry-base-delay-ms"
                | "--search-circuit-breaker-threshold"
                | "--max-web-results"
                | "--max-search-calls"
                | "--max-local-sources"
                | "--max-synthesis-sources"
                | "--brief" => {
                    i += 1;
                    if let Some(v) = rest.get(i) {
                        match arg {
                            "--from-url" | "--from-urls" => from_urls.push((*v).to_string()),
                            "--from-file" | "--from-files" => {
                                from_files.push((*v).to_string());
                            }
                            "--iterations" => iterations = v.parse().ok(),
                            "--depth" => depth = Some((*v).to_string()),
                            "--tier" => tier = Some((*v).to_string()),
                            "--mode" => mode = Some((*v).to_string()),
                            "--summarization-model" => {
                                summarization_model = Some((*v).to_string());
                            }
                            "--research-model" => research_model = Some((*v).to_string()),
                            "--compression-model" => compression_model = Some((*v).to_string()),
                            "--final-report-model" => {
                                final_report_model = Some((*v).to_string());
                            }
                            "--max-concurrent-research-units" => {
                                max_concurrent_research_units = v.parse().ok();
                            }
                            "--format" => format = Some((*v).to_string()),
                            "--sources-dir" => sources_dir = Some((*v).to_string()),
                            "--template" => template = Some((*v).to_string()),
                            "--fetch-concurrently" => fetch_concurrency = v.parse().ok(),
                            "--local-concurrently" => local_concurrency = v.parse().ok(),
                            "--fetch-timeout-secs" => fetch_timeout_secs = v.parse().ok(),
                            "--web-phase-timeout-secs" | "--web-time" => {
                                web_phase_timeout_secs = v.parse().ok();
                            }
                            "--local-phase-timeout-secs" => {
                                local_phase_timeout_secs = v.parse().ok();
                            }
                            "--search-max-retries" => search_max_retries = v.parse().ok(),
                            "--search-retry-base-delay-ms" => {
                                search_retry_base_delay_ms = v.parse().ok();
                            }
                            "--search-circuit-breaker-threshold" => {
                                search_circuit_breaker_threshold = v.parse().ok();
                            }
                            "--max-web-results" => max_web_results = v.parse().ok(),
                            "--max-search-calls" => max_search_calls = v.parse().ok(),
                            "--max-local-sources" => max_local_sources = v.parse().ok(),
                            "--max-synthesis-sources" => max_synthesis_sources = v.parse().ok(),
                            "--brief" => brief = Some((*v).to_string()),
                            _ => unreachable!(),
                        }
                    }
                }
                "--use-local" => use_local = true,
                "--use-specs" => use_specs = true,
                "--use-low-relevance" => use_low_relevance = true,
                "--no-papers" => no_papers = true,
                "--use-pdf" => use_pdf = true,
                "--clarify" => clarify = Some(true),
                "--no-clarify" => clarify = Some(false),
                "--evaluate" => evaluate = true,
                _ => {
                    if name.is_none() {
                        name = Some(arg.to_string());
                    } else {
                        topic_words.push(arg);
                    }
                }
            }
            i += 1;
        }
        let Some(name) = name else {
            return Self::Unknown("create".to_string());
        };
        let topic = topic_words.join(" ");
        Self::Create {
            name,
            topic,
            from_urls,
            from_files,
            iterations,
            depth,
            tier,
            mode,
            summarization_model,
            research_model,
            compression_model,
            final_report_model,
            max_concurrent_research_units,
            clarify,
            format,
            sources_dir,
            template,
            fetch_concurrency,
            use_local,
            use_specs,
            use_low_relevance,
            no_papers,
            use_pdf,
            fetch_timeout_secs,
            local_concurrency,
            web_phase_timeout_secs,
            local_phase_timeout_secs,
            search_max_retries,
            search_retry_base_delay_ms,
            search_circuit_breaker_threshold,
            max_web_results,
            max_search_calls,
            max_local_sources,
            max_synthesis_sources,
            brief,
            evaluate,
        }
    }

    fn is_flag(arg: &str) -> bool {
        arg.starts_with('-')
    }

    fn first_positional(rest: &[&str]) -> Option<String> {
        rest.iter()
            .find(|a| !Self::is_flag(a))
            .map(std::string::ToString::to_string)
    }

    fn parse_list(rest: &[&str]) -> Self {
        Self::List {
            all: rest.contains(&"--all"),
            json: rest.contains(&"--json"),
        }
    }

    fn parse_show(rest: &[&str]) -> Self {
        Self::Show {
            name: Self::first_positional(rest).unwrap_or_default(),
            json: rest.contains(&"--json"),
        }
    }

    fn parse_search(rest: &[&str]) -> Self {
        let query = rest
            .iter()
            .filter(|a| !Self::is_flag(a))
            .copied()
            .collect::<Vec<_>>()
            .join(" ");
        Self::Search {
            query,
            json: rest.contains(&"--json"),
        }
    }

    fn parse_open(rest: &[&str]) -> Self {
        Self::Open {
            name: Self::first_positional(rest).unwrap_or_default(),
        }
    }

    fn parse_delete(rest: &[&str]) -> Self {
        Self::Delete {
            name: Self::first_positional(rest).unwrap_or_default(),
            yes: rest.contains(&"--yes"),
        }
    }

    fn parse_archive(rest: &[&str]) -> Self {
        Self::Archive {
            name: Self::first_positional(rest).unwrap_or_default(),
        }
    }

    fn parse_resume(rest: &[&str]) -> Self {
        Self::Resume {
            name: Self::first_positional(rest).unwrap_or_default(),
        }
    }

    fn parse_continue(rest: &[&str]) -> Self {
        let mut message: Option<String> = None;
        let mut i = 0;
        while i < rest.len() {
            if rest[i] == "--message" {
                i += 1;
                if let Some(v) = rest.get(i) {
                    message = Some((*v).to_string());
                }
            }
            i += 1;
        }
        Self::Continue {
            name: Self::first_positional(rest).unwrap_or_default(),
            message,
        }
    }

    /// Parse `update <name>`. Extra arguments are tolerated and ignored so
    /// the recorded CLI/TUI/HTTP invocation forms replay unchanged.
    fn parse_update(rest: &[&str]) -> Self {
        Self::Update {
            name: Self::first_positional(rest).unwrap_or_default(),
        }
    }

    fn parse_cluster(rest: &[&str]) -> Self {
        Self::Cluster {
            name: Self::first_positional(rest).unwrap_or_default(),
            force: rest.contains(&"--force"),
        }
    }

    fn parse_export(rest: &[&str]) -> Self {
        let mut output: Option<String> = None;
        let mut i = 0;
        while i < rest.len() {
            if rest[i] == "--output" {
                i += 1;
                if let Some(v) = rest.get(i) {
                    output = Some((*v).to_string());
                }
            }
            i += 1;
        }
        Self::Export {
            name: Self::first_positional(rest).unwrap_or_default(),
            output,
        }
    }

    fn parse_import(rest: &[&str]) -> Self {
        let mut name: Option<String> = None;
        let mut i = 0;
        while i < rest.len() {
            if rest[i] == "--name" {
                i += 1;
                if let Some(v) = rest.get(i) {
                    name = Some((*v).to_string());
                }
            }
            i += 1;
        }
        Self::Import {
            path: Self::first_positional(rest).unwrap_or_default(),
            name,
        }
    }

    /// Build the help message shown for `/research help`.
    #[must_use]
    pub fn build_help_message() -> String {
        r"Available `ragent research` commands:

  create <name> [topic] [flags]   Start a new research run
  list [--all] [--json]          List research items
  show <name> [--json]           Show one research item
  open <name>                     Open a research item in the viewer
  search <query> [--json]        Search research items
  delete <name> [--yes]           Delete a research item
  archive <name>                  Archive a research item
  resume <name>                  Resume an in-progress run
  continue <name> [--message ...] Continue a completed run with a follow-up
  update <name>                   Replay the recorded invocation for a run
  cluster <name> [--force]        Extract concept clusters for a run
  export <name> [--output path]   Export a research item
  import <path> [--name name]    Import a research item
  config                         Show effective research defaults
  help                           Show this help message

Common create flags:
  --mode tiered|supervisor|competitive (competitive implies --format comparison-table)
  --tier light|full|dissertation
  --depth shallow|standard|deep
  --format report|executive-summary|comparison-table|source-bibliography|imrad
  --research-model, --compression-model, --final-report-model <provider:model>
  --max-concurrent-research-units N
  --summarization-model <provider:model>
  --use-local, --use-specs, --use-low-relevance, --no-papers, --use-pdf
"
        .to_string()
    }
}

/// Split a command line into tokens, respecting double-quoted strings.
fn split_args(input: &str) -> Vec<String> {
    let mut tokens = Vec::new();
    let mut current = String::new();
    let mut in_quotes = false;
    for ch in input.chars() {
        match ch {
            '"' => in_quotes = !in_quotes,
            ' ' | '\t' if !in_quotes => {
                if !current.is_empty() {
                    tokens.push(std::mem::take(&mut current));
                }
            }
            _ => current.push(ch),
        }
    }
    if !current.is_empty() {
        tokens.push(current);
    }
    tokens
}

/// Render a list of research items as a fixed-width human-readable table.
///
/// Each tuple is `(name, title, topic, status, created, modified)`. Long
/// fields are truncated to their column width so rows stay aligned. Use
/// [`render_list_output_json`] for machine-readable output.
#[must_use]
pub fn render_list_output(items: &[(String, String, String, String, String, String)]) -> String {
    if items.is_empty() {
        return "(no research items)\n".to_string();
    }
    let mut out = String::new();
    out.push_str(&format!(
        "{:<22} {:<32} {:<11} {:<24} {:<24}\n",
        "NAME", "TITLE", "STATUS", "CREATED", "MODIFIED"
    ));
    out.push_str(&"-".repeat(120));
    out.push('\n');
    for (name, title, _topic, status, created, modified) in items {
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

/// Render a list of research items as a JSON array (one object per item).
///
/// Each tuple is `(name, title, topic, status, created, modified)`.
#[must_use]
pub fn render_list_output_json(
    items: &[(String, String, String, String, String, String)],
) -> String {
    let rows: Vec<serde_json::Value> = items
        .iter()
        .map(|(name, title, topic, status, created, modified)| {
            serde_json::json!({
                "name": name,
                "title": title,
                "topic": topic,
                "status": status,
                "created": created,
                "modified": modified,
            })
        })
        .collect();
    serde_json::to_string(&rows).unwrap_or_else(|_| "[]".to_string())
}

/// Render search results as a human-readable bullet list.
///
/// Each tuple is `(name, title, snippet)`. Use [`render_search_output_json`]
/// for machine-readable output.
#[must_use]
pub fn render_search_output(results: &[(String, String, String)]) -> String {
    let mut out = String::new();
    for (name, title, snippet) in results {
        out.push_str(&format!("* {name} - {title}\n    {snippet}\n"));
    }
    if out.is_empty() {
        out.push_str("(no matches)\n");
    }
    out
}

/// Render search results as a JSON array.
///
/// Each tuple is `(name, title, snippet, path)`.
#[must_use]
pub fn render_search_output_json(results: &[(String, String, String, String)]) -> String {
    let rows: Vec<serde_json::Value> = results
        .iter()
        .map(|(name, title, snippet, path)| {
            serde_json::json!({
                "name": name,
                "title": title,
                "snippet": snippet,
                "path": path,
            })
        })
        .collect();
    serde_json::to_string(&rows).unwrap_or_else(|_| "[]".to_string())
}

/// Truncate a string to at most `max` characters, appending `...` when cut.
fn truncate(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        return s.to_string();
    }
    let mut t: String = s.chars().take(max.saturating_sub(3)).collect();
    t.push_str("...");
    t
}

/// Render a single research item as a JSON object.
#[must_use]
pub fn render_show_output_json(
    name: &str,
    title: &str,
    topic: &str,
    status: &str,
    created: &str,
    modified: &str,
    sources: &[(String, String, String, String, Option<String>)],
) -> String {
    let sources_json: Vec<serde_json::Value> = sources
        .iter()
        .map(|(id, url, title, kind, preview)| {
            let mut obj = serde_json::json!({
                "id": id,
                "url": url,
                "title": title,
                "kind": kind,
            });
            if let Some(p) = preview {
                obj["preview"] = serde_json::json!(p);
            }
            obj
        })
        .collect();
    serde_json::to_string(&serde_json::json!({
        "name": name,
        "title": title,
        "topic": topic,
        "status": status,
        "created": created,
        "modified": modified,
        "sources": sources_json,
    }))
    .unwrap_or_else(|_| "{}".to_string())
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
    sources: &[(String, String, String, String, Option<String>)],
) -> String {
    let mut out = String::new();
    out.push_str(&format!("Research item: {name}\n"));
    out.push_str(&format!("Title:         {title}\n"));
    out.push_str(&format!("Topic:         {topic}\n"));
    out.push_str(&format!("Status:        {status}\n"));
    out.push_str(&format!("Created:       {created}\n"));
    out.push_str(&format!("Modified:      {modified}\n"));
    out.push_str("Sources:\n");
    for (id, url, title, kind, preview) in sources {
        out.push_str(&format!("  [{kind}] {id}: {title} ({url})\n"));
        if let Some(p) = preview {
            out.push_str(&format!("    preview: {p}\n"));
        }
    }
    out
}

/// Convert a [`SessionEvent`](crate::session::SessionEvent) into a compact JSON
/// object with a `kind`/`payload` shape.
#[must_use]
pub fn session_event_json(event: &crate::session::SessionEvent) -> String {
    use crate::session::{AnalysisEvent, SessionEvent, SynthesisEvent};
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
            body_preview,
            language,
            oa_recovery,
            media_type,
        } => {
            let mut payload = serde_json::Map::new();
            payload.insert("url".into(), serde_json::json!(url));
            payload.insert("title".into(), serde_json::json!(title));
            payload.insert("search_tool".into(), serde_json::json!(search_tool));
            payload.insert("search_engine".into(), serde_json::json!(search_engine));
            payload.insert("body_preview".into(), serde_json::json!(body_preview));
            payload.insert("language".into(), serde_json::json!(language));
            payload.insert("oa_recovery".into(), serde_json::json!(oa_recovery));
            payload.insert("media_type".into(), serde_json::json!(media_type));
            ("web_captured", serde_json::Value::Object(payload))
        }
        SessionEvent::WebSearchFailed { error } => {
            ("web_search_failed", serde_json::json!({ "error": error }))
        }
        SessionEvent::WebFetchFailed { url, error } => (
            "web_fetch_failed",
            serde_json::json!({ "url": url, "error": error }),
        ),
        SessionEvent::WebSourceExcluded { url, reason } => (
            "web_source_excluded",
            serde_json::json!({ "url": url, "reason": reason }),
        ),
        SessionEvent::FromUrlBodyPreview { url, body_preview } => (
            "from_url_body_preview",
            serde_json::json!({ "url": url, "body_preview": body_preview }),
        ),
        SessionEvent::FromFileBodyPreview { path, body_preview } => (
            "from_file_body_preview",
            serde_json::json!({ "path": path, "body_preview": body_preview }),
        ),
        SessionEvent::LocalCaptured { path, score } => (
            "local_captured",
            serde_json::json!({ "path": path, "score": score }),
        ),
        SessionEvent::SpecCaptured { spec_id } => {
            ("spec_captured", serde_json::json!({ "spec_id": spec_id }))
        }
        SessionEvent::SourceFailed { source, error } => (
            "source_failed",
            serde_json::json!({ "source": source, "error": error }),
        ),
        SessionEvent::NeedsClarification { question } => (
            "needs_clarification",
            serde_json::json!({ "question": question }),
        ),
        SessionEvent::PlanUpdated { sub_questions } => (
            "plan_updated",
            serde_json::json!({ "sub_questions": sub_questions }),
        ),
        SessionEvent::SubQuestionStatusChanged { id, status } => (
            "sub_question_status_changed",
            serde_json::json!({ "id": id, "status": status }),
        ),
        SessionEvent::VerificationResult { passed, issues } => (
            "verification_result",
            serde_json::json!({ "passed": passed, "issues": issues }),
        ),
        SessionEvent::IterationCompleted { iteration, score } => (
            "iteration_completed",
            serde_json::json!({ "iteration": iteration, "score": score }),
        ),
        SessionEvent::FollowUpQueries { queries } => (
            "follow_up_queries",
            serde_json::json!({ "queries": queries }),
        ),
        SessionEvent::Analysis(AnalysisEvent::EvidenceDigest { digest }) => {
            ("evidence_digest", serde_json::json!({ "digest": digest }))
        }
        SessionEvent::Analysis(AnalysisEvent::CorpusCritic { report }) => {
            ("corpus_critic", serde_json::json!({ "report": report }))
        }
        SessionEvent::Analysis(AnalysisEvent::GapFetch { result }) => {
            ("gap_fetch", serde_json::json!({ "result": result }))
        }
        SessionEvent::Analysis(AnalysisEvent::TripleDraft { draft }) => {
            ("triple_draft", serde_json::json!({ "draft": draft }))
        }
        SessionEvent::Analysis(AnalysisEvent::ContradictionGraph {
            edges,
            sources_scanned,
        }) => (
            "contradiction_graph",
            serde_json::json!({
                "edges": edges,
                "sources_scanned": sources_scanned,
            }),
        ),
        SessionEvent::Analysis(AnalysisEvent::LociAnalysis {
            loci,
            sources_scanned,
        }) => (
            "loci_analysis",
            serde_json::json!({
                "loci": loci,
                "sources_scanned": sources_scanned,              }),
        ),
        SessionEvent::Analysis(AnalysisEvent::DepthInvestigation { investigations }) => (
            "depth_investigation",
            serde_json::json!({ "investigations": investigations }),
        ),
        SessionEvent::Analysis(AnalysisEvent::CrossLocusReconcile { reconcile }) => (
            "cross_locus_reconcile",
            serde_json::json!({ "reconcile": reconcile }),
        ),
        SessionEvent::Analysis(AnalysisEvent::SourceTensions { tensions }) => (
            "source_tensions",
            serde_json::json!({ "tensions": tensions }),
        ),
        SessionEvent::Synthesis(SynthesisEvent::SynthesizeResult { outcome, detail }) => (
            "synthesize_result",
            serde_json::json!({
                "outcome": outcome.as_str(),
                "detail": detail,
            }),
        ),
        SessionEvent::Synthesis(SynthesisEvent::SynthesisAudit { audit }) => {
            ("synthesis_audit", serde_json::json!({ "audit": audit }))
        }
        SessionEvent::Synthesis(SynthesisEvent::CriticResult { score, gaps }) => (
            "critic_result",
            serde_json::json!({ "score": score, "gaps": gaps }),
        ),
        SessionEvent::Synthesis(SynthesisEvent::SurgicalPatch { result }) => {
            ("surgical_patch", serde_json::json!({ "result": result }))
        }
        SessionEvent::Synthesis(SynthesisEvent::CiteCheck { result }) => {
            ("citation_check", serde_json::json!({ "result": result }))
        }
        SessionEvent::Synthesis(SynthesisEvent::Polish { result }) => {
            ("polish", serde_json::json!({ "result": result }))
        }
        SessionEvent::Synthesis(SynthesisEvent::ReadabilityAudit { result }) => {
            ("readability_audit", serde_json::json!({ "result": result }))
        }
        SessionEvent::Synthesis(SynthesisEvent::Evaluation { scorecard }) => (
            "evaluation_scorecard",
            serde_json::json!({
                "quality": scorecard.quality,
                "relevance": scorecard.relevance,
                "groundedness": scorecard.groundedness,
                "completeness": scorecard.completeness,
                "structure": scorecard.structure,
                "overall": scorecard.overall,
                "rationale": scorecard.rationale,
                "error": scorecard.error,
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
        SessionEvent::RunStep {
            step,
            status,
            detail,
        } => (
            "run_step",
            serde_json::json!({ "step": step, "status": status, "detail": detail }),
        ),
        SessionEvent::TierDone {
            completed,
            skipped,
            failed,
        } => (
            "tier_done",
            serde_json::json!({
                "completed": completed,
                "skipped": skipped,
                "failed": failed,
            }),
        ),
        SessionEvent::SupervisorPlanUpdated { sub_topics } => (
            "supervisor_plan_updated",
            serde_json::json!({ "sub_topics": sub_topics }),
        ),
        SessionEvent::CompetitiveEntities {
            entities,
            criteria,
            inferred,
        } => (
            "competitive_entities",
            serde_json::json!({
                "entities": entities,
                "criteria": criteria,
                "inferred": inferred,
            }),
        ),
        SessionEvent::ResearcherSpawned { id, sub_topic } => (
            "researcher_spawned",
            serde_json::json!({ "id": id, "sub_topic": sub_topic }),
        ),
        SessionEvent::ResearcherProgress {
            id,
            status,
            detail,
            sources_found,
        } => (
            "researcher_progress",
            serde_json::json!({
                "id": id,
                "status": status,
                "detail": detail,
                "sources_found": sources_found,
            }),
        ),
        SessionEvent::ResearcherNote { id, note } => (
            "researcher_note",
            serde_json::json!({ "id": id, "note": note }),
        ),
        SessionEvent::ResearcherCompleted { id, summary } => (
            "researcher_completed",
            serde_json::json!({ "id": id, "summary": summary }),
        ),
        SessionEvent::SupervisorMerged { findings_count } => (
            "supervisor_merged",
            serde_json::json!({ "findings_count": findings_count }),
        ),
        SessionEvent::ConfigSnapshot {
            mode,
            output_format,
            depth,
            iterations,
            tier,
            from_urls,
            from_files,
        } => (
            "config",
            serde_json::json!({
                "mode": mode,
                "output_format": output_format,
                "depth": depth,
                "iterations": iterations,
                "tier": tier,
                "from_urls": from_urls,
                "from_files": from_files,
            }),
        ),
    };
    serde_json::to_string(&serde_json::json!({
        "kind": kind,
        "payload": payload,
    }))
    .unwrap_or_else(|_| "{}".to_string())
}

/// Render a [`SessionEvent`](crate::session::SessionEvent) as a single
/// machine-readable JSON line on stdout (T-035). The prefix `ragent-research:`
/// makes the line easy to grep in a mixed transcript.
#[must_use]
pub fn render_session_event_json(event: &crate::session::SessionEvent) -> String {
    format!("ragent-research: {}", session_event_json(event))
}

/// Parse a `<name>` / `<status>` filter pair from a `search` query string.
///
/// Supports the grammar:
///
/// - `status:archived` — limit to archived items.
/// - `name:foo` — substring match on item name.
/// - Any other token matches against title/topic.
#[must_use]
pub fn parse_search_filters(query: &str) -> SearchFilters {
    let mut filters = SearchFilters::default();
    for token in query.split_whitespace() {
        if let Some(rest) = token.strip_prefix("status:") {
            filters.status = Some(rest.to_string());
        } else if let Some(rest) = token.strip_prefix("name:") {
            filters.name = Some(rest.to_string());
        } else {
            filters.text.push(token.to_string());
        }
    }
    filters
}

/// Parsed filters from a `search` query.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SearchFilters {
    /// Substring filter on item name.
    pub name: Option<String>,
    /// Exact status filter (matches `ResearchStatus::as_str`).
    pub status: Option<String>,
    /// Free-form text tokens matched against title and topic.
    pub text: Vec<String>,
}

/// Build a deterministic hash map from a list of research item names to their
/// index positions in `research/INDEX.md` (T-038). Used by the HTTP API to
/// resolve item order without re-parsing the index file.
#[must_use]
pub fn build_index_name_map(index_content: &str) -> HashMap<String, usize> {
    let mut map = HashMap::new();
    let mut position = 0usize;
    for line in index_content.lines() {
        if let Some(stripped) = line.strip_prefix("- ") {
            if let Some(name) = stripped.split(':').next() {
                map.insert(name.trim().to_string(), position);
                position += 1;
            }
        }
    }
    map
}

/// Parse an `OutputFormat` from a CLI `--format` value, returning the default
/// report format for unknown values so callers can log a warning instead of
/// failing the whole run.
#[must_use]
pub fn parse_output_format(s: &str) -> OutputFormat {
    OutputFormat::parse(s).unwrap_or(OutputFormat::Report)
}

/// Parse a `Depth` from a CLI `--depth` value. Returns `None` for unknown
/// values so the caller can decide whether to fall back to the default depth.
#[must_use]
pub fn parse_depth(s: &str) -> Option<Depth> {
    Depth::parse(s)
}

/// Parse a `Tier` from a CLI `--tier` value. Returns `None` for unknown values
/// so the caller can fall back to the default tier.
#[must_use]
pub fn parse_tier(s: &str) -> Option<Tier> {
    Tier::parse(s)
}

/// Parse a `ResearchMode` from a CLI `--mode` value. Returns `None` for unknown
/// values so the caller can fall back to the default mode.
#[must_use]
pub fn parse_mode(s: &str) -> Option<ResearchMode> {
    ResearchMode::parse(s)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_create_basic() {
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
    fn parse_create_with_flags() {
        let cmd = ResearchCliCommand::parse(
            "create bar topic --use-local --use-specs --use-low-relevance --no-papers --use-pdf",
        );
        match cmd {
            ResearchCliCommand::Create {
                name,
                topic,
                use_local,
                use_specs,
                use_low_relevance,
                no_papers,
                use_pdf,
                ..
            } => {
                assert_eq!(name, "bar");
                assert_eq!(topic, "topic");
                assert!(use_local);
                assert!(use_specs);
                assert!(use_low_relevance);
                assert!(no_papers);
                assert!(use_pdf);
            }
            other => panic!("unexpected variant: {other:?}"),
        }
    }

    #[test]
    fn parse_create_with_model_flags() {
        let cmd = ResearchCliCommand::parse(
            "create baz topic --research-model openai:gpt-4.1 --compression-model openai:gpt-4.1-mini --final-report-model anthropic:claude-sonnet-4",
        );
        match cmd {
            ResearchCliCommand::Create {
                name,
                topic,
                research_model,
                compression_model,
                final_report_model,
                ..
            } => {
                assert_eq!(name, "baz");
                assert_eq!(topic, "topic");
                assert_eq!(research_model.as_deref(), Some("openai:gpt-4.1"));
                assert_eq!(compression_model.as_deref(), Some("openai:gpt-4.1-mini"));
                assert_eq!(
                    final_report_model.as_deref(),
                    Some("anthropic:claude-sonnet-4")
                );
            }
            other => panic!("unexpected variant: {other:?}"),
        }
    }

    #[test]
    fn parse_create_with_max_concurrent_research_units() {
        let cmd = ResearchCliCommand::parse("create qux topic --max-concurrent-research-units 4");
        match cmd {
            ResearchCliCommand::Create {
                name,
                topic,
                max_concurrent_research_units,
                ..
            } => {
                assert_eq!(name, "qux");
                assert_eq!(topic, "topic");
                assert_eq!(max_concurrent_research_units, Some(4));
            }
            other => panic!("unexpected variant: {other:?}"),
        }
    }

    #[test]
    fn parse_create_with_mode() {
        let cmd = ResearchCliCommand::parse("create comp topic --mode competitive");
        match cmd {
            ResearchCliCommand::Create {
                name, topic, mode, ..
            } => {
                assert_eq!(name, "comp");
                assert_eq!(topic, "topic");
                assert_eq!(mode.as_deref(), Some("competitive"));
            }
            other => panic!("unexpected variant: {other:?}"),
        }
    }

    #[test]
    fn parse_list_json() {
        assert_eq!(
            ResearchCliCommand::parse("list --json"),
            ResearchCliCommand::List {
                all: false,
                json: true
            }
        );
        assert_eq!(
            ResearchCliCommand::parse("list --all"),
            ResearchCliCommand::List {
                all: true,
                json: false
            }
        );
    }

    #[test]
    fn parse_show_defaults_to_json_false() {
        match ResearchCliCommand::parse("show my-item") {
            ResearchCliCommand::Show { name, json } => {
                assert_eq!(name, "my-item");
                assert!(!json);
            }
            other => panic!("unexpected variant: {other:?}"),
        }
    }

    #[test]
    fn parse_search_joins_positional_words() {
        match ResearchCliCommand::parse("search rust async runtimes") {
            ResearchCliCommand::Search { query, json } => {
                assert_eq!(query, "rust async runtimes");
                assert!(!json);
            }
            other => panic!("unexpected variant: {other:?}"),
        }
    }

    #[test]
    fn parse_delete_requires_name() {
        match ResearchCliCommand::parse("delete doomed") {
            ResearchCliCommand::Delete { name, .. } => assert_eq!(name, "doomed"),
            other => panic!("unexpected variant: {other:?}"),
        }
    }

    #[test]
    fn parse_export_with_output() {
        match ResearchCliCommand::parse("export item --output /tmp/out") {
            ResearchCliCommand::Export { name, output } => {
                assert_eq!(name, "item");
                assert_eq!(output.as_deref(), Some("/tmp/out"));
            }
            other => panic!("unexpected variant: {other:?}"),
        }
    }

    #[test]
    fn parse_import_with_name_override() {
        match ResearchCliCommand::parse("import /tmp/item.md --name renamed") {
            ResearchCliCommand::Import { path, name } => {
                assert_eq!(path, "/tmp/item.md");
                assert_eq!(name.as_deref(), Some("renamed"));
            }
            other => panic!("unexpected variant: {other:?}"),
        }
    }

    #[test]
    fn parse_unknown_verb() {
        assert_eq!(
            ResearchCliCommand::parse("frobnicate"),
            ResearchCliCommand::Unknown("frobnicate".to_string())
        );
    }

    #[test]
    fn split_args_respects_quotes() {
        assert_eq!(
            split_args("create a \"two words\" --flag value"),
            vec!["create", "a", "two words", "--flag", "value"]
        );
    }

    #[test]
    fn render_list_output_table_has_aligned_header() {
        let out = render_list_output(&[(
            "foo".to_string(),
            "Foo".to_string(),
            "topic".to_string(),
            "complete".to_string(),
            "2026-01-01T00:00:00+00:00".to_string(),
            "2026-01-02T00:00:00+00:00".to_string(),
        )]);
        assert!(out.contains("NAME"));
        assert!(out.contains("STATUS"));
        assert!(out.contains("CREATED"));
        assert!(out.contains("foo"));
        assert!(out.contains("complete"));
        assert!(!out.contains('{'));
    }

    #[test]
    fn render_list_output_empty_shows_message() {
        assert_eq!(render_list_output(&[]), "(no research items)\n");
    }

    #[test]
    fn render_list_output_json_is_valid_json() {
        let json = render_list_output_json(&[(
            "foo".to_string(),
            "Foo".to_string(),
            "topic".to_string(),
            "complete".to_string(),
            "2026-01-01T00:00:00+00:00".to_string(),
            "2026-01-02T00:00:00+00:00".to_string(),
        )]);
        let parsed: serde_json::Value = serde_json::from_str(&json).expect("valid JSON");
        assert_eq!(parsed[0]["name"].as_str(), Some("foo"));
        assert_eq!(parsed[0]["topic"].as_str(), Some("topic"));
    }

    #[test]
    fn render_show_output_includes_sources() {
        let out = render_show_output(
            "foo",
            "Foo",
            "topic",
            "complete",
            "2024-01-01",
            "2024-01-02",
            &[(
                "s1".to_string(),
                "https://x".to_string(),
                "X".to_string(),
                "web".to_string(),
                None,
            )],
        );
        assert!(out.contains("Research item: foo"));
        assert!(out.contains("[web] s1: X (https://x)"));
    }

    #[test]
    fn render_search_output_bullet_list() {
        let out =
            render_search_output(&[("foo".to_string(), "Foo".to_string(), "snippet".to_string())]);
        assert!(out.contains("* foo - Foo"));
        assert!(out.contains("snippet"));
    }

    #[test]
    fn render_search_output_empty_shows_message() {
        assert_eq!(render_search_output(&[]), "(no matches)\n");
    }

    #[test]
    fn render_search_output_json_is_valid_json() {
        let json = render_search_output_json(&[(
            "foo".to_string(),
            "Foo".to_string(),
            "snippet".to_string(),
            "research/foo/RESEARCH.md".to_string(),
        )]);
        let parsed: serde_json::Value = serde_json::from_str(&json).expect("valid JSON");
        assert_eq!(parsed[0]["path"].as_str(), Some("research/foo/RESEARCH.md"));
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
        assert!(t.ends_with("..."));
    }

    #[test]
    fn parse_search_filters_extracts_status_and_name() {
        let f = parse_search_filters("status:complete name:rust async");
        assert_eq!(f.status.as_deref(), Some("complete"));
        assert_eq!(f.name.as_deref(), Some("rust"));
        assert_eq!(f.text, vec!["async"]);
    }

    #[test]
    fn parse_output_format_defaults_unknown_to_report() {
        assert_eq!(parse_output_format("weird"), OutputFormat::Report);
        assert_eq!(parse_output_format("imrad"), OutputFormat::Imrad);
    }

    #[test]
    fn parse_depth_returns_none_for_unknown() {
        assert!(parse_depth("deep").is_some());
        assert!(parse_depth("weird").is_none());
    }

    #[test]
    fn parse_tier_returns_none_for_unknown() {
        assert!(parse_tier("full").is_some());
        assert!(parse_tier("weird").is_none());
    }

    #[test]
    fn parse_mode_returns_none_for_unknown() {
        assert!(parse_mode("supervisor").is_some());
        assert!(parse_mode("weird").is_none());
    }

    #[test]
    fn build_index_name_map_assigns_positions() {
        let map = build_index_name_map("- alpha: title\n- beta: other\n");
        assert_eq!(map.get("alpha"), Some(&0));
        assert_eq!(map.get("beta"), Some(&1));
    }

    #[test]
    fn session_event_json_round_trips() {
        let event = crate::session::SessionEvent::Phase {
            phase: crate::session::SessionPhase::Web,
        };
        let json = session_event_json(&event);
        let parsed: serde_json::Value = serde_json::from_str(&json).expect("valid JSON");
        assert_eq!(parsed["kind"].as_str(), Some("phase"));
    }

    #[test]
    fn render_session_event_json_adds_prefix() {
        let event = crate::session::SessionEvent::Phase {
            phase: crate::session::SessionPhase::Web,
        };
        let rendered = render_session_event_json(&event);
        assert!(rendered.starts_with("ragent-research: "));
    }
}

#[test]
fn parse_create_evaluate_flag() {
    let cmd = ResearchCliCommand::parse("create eval-topic topic --evaluate");
    match cmd {
        ResearchCliCommand::Create {
            name,
            topic,
            evaluate,
            ..
        } => {
            assert_eq!(name, "eval-topic");
            assert_eq!(topic, "topic");
            assert!(evaluate);
        }
        other => panic!("unexpected variant: {other:?}"),
    }
}
